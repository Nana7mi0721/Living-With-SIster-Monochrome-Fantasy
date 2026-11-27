#!/usr/bin/env python3
"""Pure-Python builder for WOLFMPK1 media packs (`media.pak`).

Byte-exact wire format matching `godot/presentation/MediaPakReader.cs`:
- 32-byte header: magic `WOLFMPK1`, uint16 version=1, uint16 flags=0,
  uint32 count, uint64 indexOffset (16-byte aligned), uint64 indexSize.
- 16-byte-aligned file bodies XORed with the splitmix64 offset keystream and
  tagged with standard zlib CRC-32 of the plaintext bytes.
- Trailing encrypted index: (uint16 pathLen, uint64 offset, uint64 size,
  uint32 crc32, utf8 path) per entry, XORed from indexOffset.

Supports both a loose retail asset directory (`--root`) and repacking an
existing `media.pak` (`--from-pak`), with optional `--slim` mixed-fidelity
WebP/OGG conversion or `--lossless` pixel-exact WebP repacking.
"""
from __future__ import annotations

import argparse
import concurrent.futures
import io
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path, PurePosixPath

MAGIC = b"WOLFMPK1"
VERSION = 1
HEADER_SIZE = 32
DATA_ALIGNMENT = 16
STREAM_BLOCK_SIZE = 64
GOLDEN = 0x9E3779B97F4A7C15
MASK64 = 0xFFFFFFFFFFFFFFFF

SEED = bytes(
    [
        0x9F, 0x2E, 0x4B, 0xD7, 0x18, 0x63, 0xA5, 0x0C,
        0xE4, 0x71, 0x35, 0x8A, 0xC2, 0x5D, 0x90, 0x26,
        0x6B, 0xF3, 0x04, 0xBE, 0x57, 0xA1, 0xD8, 0x39,
        0x82, 0x4C, 0xEF, 0x16, 0x75, 0xB0, 0x2D, 0x9A,
    ]
)
SEED_WORDS = struct.unpack("<4Q", SEED)

MEDIA_EXTENSIONS = {
    ".png",
    ".jpg",
    ".jpeg",
    ".bmp",
    ".webp",
    ".ogg",
    ".mp3",
    ".wav",
    ".ico",
    ".ttf",
    ".otf",
}

TEXTURE_EXTENSIONS = (".png", ".jpg", ".jpeg", ".bmp", ".webp")
AUDIO_EXTENSIONS = (".ogg", ".mp3", ".wav")
REF_MEDIA_EXTENSIONS = set(TEXTURE_EXTENSIONS) | set(AUDIO_EXTENSIONS)
WOLF_CTRL_RE = re.compile(
    r"\\(?:cself|self|cdb|udb|sdb|sys|font|space|sp|ax|ay|mx|my|at|[vscfmi-])\[",
    re.I,
)


def _norm_asset_key(raw: str) -> str:
    key = raw.strip().replace("\\", "/").lower()
    while key.startswith("./") or key.startswith("../"):
        key = key[2:] if key.startswith("./") else key[3:]
    return key.lstrip("/")


def _load_game_data_json(path: Path) -> object:
    br_path = Path(str(path) + ".br")
    chosen = path if path.is_file() else (br_path if br_path.is_file() else None)
    if chosen is None:
        raise FileNotFoundError(f"missing game_data artifact: {path} (or {br_path})")
    if chosen.suffix.lower() == ".br":
        tools_dir = Path(__file__).resolve().parent
        if str(tools_dir) not in sys.path:
            sys.path.insert(0, str(tools_dir))
        import brotli_json

        return json.loads(brotli_json.decompress_bytes(chosen.read_bytes()).decode("utf-8"))
    return json.loads(chosen.read_text(encoding="utf-8"))


def _extract_media_refs(node: object, sink: set[str]) -> None:
    if isinstance(node, dict):
        for value in node.values():
            _extract_media_refs(value, sink)
    elif isinstance(node, list):
        for item in node:
            _extract_media_refs(item, sink)
    elif isinstance(node, str):
        raw = node.strip()
        norm = _norm_asset_key(raw)
        if not norm or norm.startswith("<") or norm.startswith("save/") or "%" in raw or WOLF_CTRL_RE.search(raw):
            return
        if PurePosixPath(norm).suffix in REF_MEDIA_EXTENSIONS:
            sink.add(raw.replace("\\", "/"))


def _resolve_live_key(key: str, available: set[str]) -> str | None:
    if key in available:
        return key
    pp = PurePosixPath(key)
    suffix = pp.suffix
    stem = pp.with_suffix("").as_posix()
    exts = TEXTURE_EXTENSIONS if suffix in TEXTURE_EXTENSIONS else AUDIO_EXTENSIONS if suffix in AUDIO_EXTENSIONS else ()
    for ext in exts:
        cand = stem + ext
        if cand in available:
            return cand
    if suffix in TEXTURE_EXTENSIONS and key.startswith("picture/word_effect"):
        alias = "hword/" + pp.name
        if alias in available:
            return alias
        alias_stem = PurePosixPath(alias).with_suffix("").as_posix()
        for ext in TEXTURE_EXTENSIONS:
            cand = alias_stem + ext
            if cand in available:
                return cand
    return None


def collect_live_asset_keys(game_data_dir: Path, available_keys: set[str]) -> set[str]:
    """Compute the live media whitelist from game_data JSONs plus SystemFile/*."""
    refs: set[str] = set()
    for fname in ("programs.json", "database.json", "map_tiles.json"):
        _extract_media_refs(_load_game_data_json(game_data_dir / fname), refs)
    manifest_path = game_data_dir / "manifest.json"
    if manifest_path.is_file():
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        for item in manifest.get("assets") or []:
            if isinstance(item, dict):
                logical = item.get("logical") or item.get("path") or item.get("source") or ""
                if isinstance(logical, str) and logical.strip():
                    refs.add(logical)

    live: set[str] = {k for k in available_keys if k.startswith("systemfile/")}
    for ref in refs:
        hit = _resolve_live_key(_norm_asset_key(ref), available_keys)
        if hit is not None:
            live.add(hit)
    return live


def _stream_block(block: int) -> bytes:
    state = (block * GOLDEN) & MASK64
    for seed_word in SEED_WORDS:
        state = (((state + seed_word) & MASK64) * GOLDEN) & MASK64
        state ^= state >> 29
    out = bytearray(STREAM_BLOCK_SIZE)
    for offset in range(0, STREAM_BLOCK_SIZE, 8):
        state = (state + GOLDEN) & MASK64
        val = state
        val = (((val ^ (val >> 30)) & MASK64) * 0xBF58476D1CE4E5B9) & MASK64
        val = (((val ^ (val >> 27)) & MASK64) * 0x94D049BB133111EB) & MASK64
        val ^= val >> 31
        struct.pack_into("<Q", out, offset, val)
    return bytes(out)


def xor_bytes(data: bytes | bytearray, offset: int) -> bytes:
    """Apply the WOLFMPK1 offset-indexed splitmix64 XOR stream."""
    n = len(data)
    if n == 0:
        return b""
    out = bytearray(data)
    first_block = offset // STREAM_BLOCK_SIZE
    last_block = (offset + n - 1) // STREAM_BLOCK_SIZE
    pad_front = offset % STREAM_BLOCK_SIZE
    # Concatenate enough keystream blocks, then XOR in uint64 chunks via int.from_bytes
    stream = b"".join(_stream_block(b) for b in range(first_block, last_block + 1))
    key = stream[pad_front : pad_front + n]
    val = int.from_bytes(out, "little") ^ int.from_bytes(key, "little")
    return val.to_bytes(n, "little")


def read_pak_index(pak_path: Path) -> set[str]:
    """Read only the lowercase logical path TOC from an existing WOLFMPK1 file."""
    with pak_path.open("rb") as f:
        header = f.read(HEADER_SIZE)
        if len(header) < HEADER_SIZE or header[:8] != MAGIC:
            raise ValueError(f"{pak_path} is not a valid WOLFMPK1 pack")
        ver, flags, count, index_offset, index_size = struct.unpack("<HHIQQ", header[8:])
        if ver != VERSION or flags != 0:
            raise ValueError(f"unsupported header version={ver} flags={flags}")
        f.seek(index_offset)
        idx = xor_bytes(f.read(index_size), index_offset)
        names: set[str] = set()
        pos = 0
        for _ in range(count):
            plen, _off, _sz, _crc = struct.unpack_from("<HQQI", idx, pos)
            pos += 22
            rel = idx[pos : pos + plen].decode("utf-8").replace("\\", "/").lower()
            pos += plen
            names.add(rel)
        return names


def read_pak_entries(pak_path: Path) -> list[tuple[str, bytes]]:
    """Read and verify all decrypted entries from an existing WOLFMPK1 file."""
    with pak_path.open("rb") as f:
        header = f.read(HEADER_SIZE)
        if len(header) < HEADER_SIZE or header[:8] != MAGIC:
            raise ValueError(f"{pak_path} is not a valid WOLFMPK1 pack")
        ver, flags, count, index_offset, index_size = struct.unpack("<HHIQQ", header[8:])
        if ver != VERSION or flags != 0:
            raise ValueError(f"unsupported header version={ver} flags={flags}")
        f.seek(index_offset)
        idx = xor_bytes(f.read(index_size), index_offset)

        items: list[tuple[str, bytes]] = []
        pos = 0
        for _ in range(count):
            plen, off, sz, crc = struct.unpack_from("<HQQI", idx, pos)
            pos += 22
            rel = idx[pos : pos + plen].decode("utf-8")
            pos += plen
            f.seek(off)
            plain = xor_bytes(f.read(sz), off)
            actual = zlib.crc32(plain) & 0xFFFFFFFF
            if actual != crc:
                raise ValueError(f"CRC mismatch on {rel}: expected {crc:08X}, got {actual:08X}")
            items.append((rel, plain))
        return items


def collect_loose_entries(root: Path) -> list[tuple[str, Path]]:
    """Collect media assets from an unpacked retail Data directory."""
    if not root.is_dir():
        raise FileNotFoundError(f"asset root not found: {root}")
    items: list[tuple[str, Path]] = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d.lower() != "save"]
        for name in filenames:
            full = Path(dirpath) / name
            rel = full.relative_to(root).as_posix()
            # Skip BasicData/* except icon/image assets inside BasicData only when requested;
            # CDataBase/DataBase/SysDatabase *.dat/*.project stay outside media.pak.
            if rel.lower().startswith("basicdata/"):
                continue
            if full.suffix.lower() not in MEDIA_EXTENSIONS:
                continue
            items.append((rel, full))
    items.sort(key=lambda kv: kv[0].lower())
    return items


def _slim_entry(rel: str, raw: bytes, quality_effects: int, ffmpeg_bin: str | None) -> tuple[str, bytes, str]:
    """Transcode one entry if slimming saves >= 5% while keeping format fidelity."""
    low = rel.lower()
    ext = os.path.splitext(low)[1]

    if ext in (".png", ".jpg", ".jpeg", ".bmp"):
        try:
            from PIL import Image
        except ImportError:
            return rel, raw, "kept"

        try:
            with Image.open(io.BytesIO(raw)) as img:
                out = io.BytesIO()
                if ext == ".png" and low.startswith("battleeffect/"):
                    img.save(out, format="WEBP", quality=quality_effects, method=4)
                elif ext == ".png":
                    img.save(out, format="WEBP", lossless=True, method=4)
                else:
                    if img.mode not in ("RGB", "RGBA"):
                        img = img.convert("RGB")
                    img.save(out, format="WEBP", quality=88, method=4)
                cand = out.getvalue()
                if len(cand) <= int(len(raw) * 0.95):
                    return rel, cand, "webp"
        except Exception:
            return rel, raw, "kept"

    elif ext == ".wav" and ffmpeg_bin and b"smpl" not in raw and b"cue " not in raw:
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / "in.wav"
            dst = Path(tmp) / "out.ogg"
            src.write_bytes(raw)
            proc = subprocess.run(
                [ffmpeg_bin, "-y", "-v", "error", "-i", str(src), "-c:a", "libvorbis", "-q:a", "5", str(dst)],
                capture_output=True,
                check=False,
            )
            if proc.returncode == 0 and dst.is_file():
                cand = dst.read_bytes()
                if len(cand) <= int(len(raw) * 0.95):
                    return rel, cand, "ogg"

    return rel, raw, "kept"


def _lossless_entry(rel: str, raw: bytes) -> tuple[str, bytes, str]:
    """Repack PNG/BMP to pixel-exact lossless WebP when it is smaller."""
    ext = os.path.splitext(rel.lower())[1]
    if ext not in (".png", ".bmp"):
        return rel, raw, "kept"
    # Skip redundant re-encoding when repacking an existing lossless media.pak.
    if len(raw) >= 12 and raw[:4] == b"RIFF" and raw[8:12] == b"WEBP":
        return rel, raw, "kept"
    try:
        from PIL import Image
    except ImportError as exc:
        raise RuntimeError("Pillow is required for --lossless mode (pip install pillow)") from exc

    try:
        with Image.open(io.BytesIO(raw)) as img:
            if getattr(img, "n_frames", 1) != 1 or img.mode in ("I", "I;16", "I;16B", "I;16L", "F"):
                return rel, raw, "kept"
            has_alpha = "A" in img.getbands() or "transparency" in img.info
            mode = "RGBA" if has_alpha else "RGB"
            source = img.convert(mode)
            expected_pixels = source.tobytes()
            out = io.BytesIO()
            save_args = {
                "format": "WEBP",
                "lossless": True,
                "quality": 100,
                "method": 6,
                "exact": True,
            }
            icc_profile = img.info.get("icc_profile")
            if icc_profile:
                save_args["icc_profile"] = icc_profile
            source.save(out, **save_args)
            candidate = out.getvalue()
            if len(candidate) >= len(raw):
                return rel, raw, "kept"
        with Image.open(io.BytesIO(candidate)) as check:
            if check.convert(mode).tobytes() != expected_pixels:
                return rel, raw, "kept"
        return rel, candidate, "webp_lossless"
    except Exception:
        return rel, raw, "kept"


def write_pak(entries: list[tuple[str, bytes]], out_path: Path) -> tuple[int, int]:
    """Write entries into a valid WOLFMPK1 archive atomically via a temp sibling."""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = out_path.with_suffix(out_path.suffix + ".tmp")

    # Sort deterministically, deduplicating case-insensitively.
    seen: set[str] = set()
    ordered: list[tuple[str, bytes]] = []
    for rel, raw in sorted(entries, key=lambda kv: kv[0].lower()):
        norm = rel.replace("\\", "/").lstrip("/")
        low = norm.lower()
        if low in seen:
            continue
        seen.add(low)
        ordered.append((norm, raw))

    total_raw = 0
    toc_parts: list[bytes] = []

    with tmp_path.open("wb") as f:
        f.write(b"\x00" * HEADER_SIZE)
        cursor = HEADER_SIZE

        for rel, raw in ordered:
            pad = (-cursor) & (DATA_ALIGNMENT - 1)
            if pad:
                f.write(b"\x00" * pad)
                cursor += pad
            crc = zlib.crc32(raw) & 0xFFFFFFFF
            enc = xor_bytes(raw, cursor)
            f.write(enc)
            path_bytes = rel.encode("utf-8")
            toc_parts.append(struct.pack("<HQQI", len(path_bytes), cursor, len(raw), crc) + path_bytes)
            cursor += len(raw)
            total_raw += len(raw)

        pad = (-cursor) & (DATA_ALIGNMENT - 1)
        if pad:
            f.write(b"\x00" * pad)
            cursor += pad

        index_offset = cursor
        index_plain = b"".join(toc_parts)
        index_enc = xor_bytes(index_plain, index_offset)
        f.write(index_enc)

        f.seek(0)
        header = struct.pack(
            "<8sHHIQQ",
            MAGIC,
            VERSION,
            0,
            len(ordered),
            index_offset,
            len(index_plain),
        )
        f.write(header)
        f.flush()
        os.fsync(f.fileno())

    os.replace(tmp_path, out_path)
    return len(ordered), total_raw


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build WOLFMPK1 media.pak with optional slim or lossless image repacking")
    src = parser.add_mutually_exclusive_group(required=True)
    src.add_argument("--root", type=Path, help="Loose asset root directory")
    src.add_argument("--from-pak", type=Path, help="Existing WOLFMPK1 pack to repack")
    src.add_argument("--hd-root", type=Path, help="2x art root (hd_manifest.json + images) for a standalone HD pack")
    parser.add_argument("--out", type=Path, required=True, help="Output media.pak path")
    parser.add_argument(
        "--extra-root",
        type=Path,
        default=None,
        help="Overlay missing media files from a loose directory; existing base paths take precedence",
    )
    parser.add_argument(
        "--prune-game-data",
        type=Path,
        default=None,
        help="Prune unreferenced dead assets using game_data/{programs,database,map_tiles,manifest}.json (keeps SystemFile/*)",
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--slim", action="store_true", help="Mixed-fidelity WebP/OGG slimming & skip MapChip")
    mode.add_argument(
        "--lossless",
        action="store_true",
        help="Pixel-exact lossless WebP repack for PNG/BMP; JPG/audio stay byte-identical & skip MapChip",
    )
    parser.add_argument("--keep-mapchip", action="store_true", help="Retain MapChip/ entries under --slim or --lossless")
    parser.add_argument("--effects-quality", type=int, default=90, help="WebP quality for BattleEffect/* under --slim")
    parser.add_argument("--jobs", type=int, default=max(1, min(8, os.cpu_count() or 4)), help="Worker threads for --slim")
    args = parser.parse_args(argv)

    if args.hd_root and (args.slim or args.lossless or args.extra_root or args.prune_game_data):
        print("PACK_MEDIA_ERROR --slim/--lossless/--extra-root/--prune-game-data cannot be combined with --hd-root", file=sys.stderr)
        return 2

    if args.hd_root:
        manifest = args.hd_root / "hd_manifest.json"
        if not manifest.is_file():
            print(f"PACK_MEDIA_ERROR missing {manifest}", file=sys.stderr)
            return 2
        items: list[tuple[str, bytes]] = [("hd_manifest.json", manifest.read_bytes())]
        for rel, path in collect_loose_entries(args.hd_root):
            if rel != "hd_manifest.json":
                items.append((rel, path.read_bytes()))
        raw_items = items
    elif args.from_pak:
        raw_items = read_pak_entries(args.from_pak)
    else:
        raw_items = [(rel, path.read_bytes()) for rel, path in collect_loose_entries(args.root)]

    extra_added = 0
    if args.extra_root:
        existing = {rel.lower() for rel, _ in raw_items}
        for rel, path in collect_loose_entries(args.extra_root):
            low = rel.lower()
            if low in existing:
                continue
            raw_items.append((rel, path.read_bytes()))
            existing.add(low)
            extra_added += 1

    pruned_count = 0
    if args.prune_game_data:
        available = {rel.replace("\\", "/").lstrip("/").lower() for rel, _ in raw_items}
        live_keys = collect_live_asset_keys(args.prune_game_data, available)
        before_count = len(raw_items)
        raw_items = [
            (rel, data)
            for rel, data in raw_items
            if rel.replace("\\", "/").lstrip("/").lower() in live_keys
        ]
        pruned_count = before_count - len(raw_items)

    if (args.slim or args.lossless) and not args.keep_mapchip:
        raw_items = [(rel, data) for rel, data in raw_items if not rel.lower().startswith("mapchip/")]

    orig_total = sum(len(d) for _, d in raw_items)
    stats = {"webp": 0, "webp_lossless": 0, "ogg": 0, "kept": 0}

    if args.lossless:
        try:
            import PIL.Image  # noqa: F401
        except ImportError:
            print("PACK_MEDIA_ERROR --lossless requires Pillow (pip install pillow)", file=sys.stderr)
            return 2

    if args.slim or args.lossless:
        ffmpeg_bin = shutil.which("ffmpeg")
        worker = (
            (lambda item: _slim_entry(item[0], item[1], args.effects_quality, ffmpeg_bin))
            if args.slim
            else (lambda item: _lossless_entry(item[0], item[1]))
        )
        if args.jobs > 1:
            with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
                results = list(pool.map(worker, raw_items))
        else:
            results = [worker(item) for item in raw_items]
        final_items: list[tuple[str, bytes]] = []
        for rel, data, kind in results:
            final_items.append((rel, data))
            stats[kind] += 1
    else:
        final_items = raw_items
        stats["kept"] = len(final_items)

    count, packed_raw = write_pak(final_items, args.out)
    pak_size = args.out.stat().st_size
    pack_mode = "lossless" if args.lossless else ("slim" if args.slim else "copy")
    print(
        f"PACK_MEDIA_OK entries={count} mode={pack_mode} slim={args.slim} lossless={args.lossless} "
        f"extra={extra_added} pruned={pruned_count} "
        f"orig_bytes={orig_total} packed_bytes={packed_raw} pak_bytes={pak_size} "
        f"webp={stats['webp']} webp_lossless={stats['webp_lossless']} "
        f"ogg={stats['ogg']} kept={stats['kept']}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
