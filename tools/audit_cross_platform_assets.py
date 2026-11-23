#!/usr/bin/env python3
"""Offline cross-platform asset consistency auditor for LWSMF.

Verifies parity between desktop loose assets (`work/all`), packed media (`media.pak`),
optional 2x assets (`media_hd.pak`), and static references in `game_data/`.
"""
from __future__ import annotations

import argparse
import io
import json
import math
import os
import re
import struct
import sys
import zlib
from collections import defaultdict
from pathlib import Path, PurePosixPath
from typing import Any

TOOLS_DIR = Path(__file__).resolve().parent
REPO_ROOT = TOOLS_DIR.parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import brotli_json  # noqa: E402
import pack_media  # noqa: E402

DIR_PRIORITY: list[str] = [
    "systemgraphic",
    "hword",
    "systemfile",
    "picture",
    "dlc_data",
    "cg",
    "hscene",
    "cutin",
    "1.4.0",
    "kagura",
    "charachip",
    "ficon",
    "battleeffect",
    "enemygraphic",
    "mapchip",
]
DIR_RANK = {name: idx for idx, name in enumerate(DIR_PRIORITY)}

TEXTURE_EXTENSIONS = (".png", ".jpg", ".jpeg", ".bmp", ".webp")
AUDIO_EXTENSIONS = (".ogg", ".mp3", ".wav")
REF_MEDIA_EXTENSIONS = set(TEXTURE_EXTENSIONS) | set(AUDIO_EXTENSIONS)
ALL_MEDIA_EXTENSIONS = REF_MEDIA_EXTENSIONS | {".ico", ".ttf", ".otf"}
FONT_NAMES = {"lolita.ttf", "data/lolita.ttf"}

WOLF_CTRL_RE = re.compile(
    r"\\(?:cself|self|cdb|udb|sdb|sys|font|space|sp|ax|ay|mx|my|at|[vscfmi-])\[",
    re.I,
)


def norm_key(raw: str) -> str:
    key = raw.strip().replace("\\", "/").lower()
    while key.startswith("./") or key.startswith("../"):
        key = key[2:] if key.startswith("./") else key[3:]
    return key.lstrip("/")


def dir_rank(low_rel: str) -> int:
    parent = low_rel.rsplit("/", 1)[0] if "/" in low_rel else ""
    return DIR_RANK.get(parent, len(DIR_PRIORITY))


def pick_deterministic_winner(paths: list[str]) -> str:
    return min(paths, key=lambda p: (dir_rank(p), p))


def read_pak_toc(pak_path: Path) -> dict[str, tuple[str, int, int, int]]:
    """Return lowercase_path -> (canonical_path, offset, size, crc) from a WOLFMPK1 pack."""
    with pak_path.open("rb") as f:
        header = f.read(pack_media.HEADER_SIZE)
        if len(header) < pack_media.HEADER_SIZE or header[:8] != pack_media.MAGIC:
            raise ValueError(f"{pak_path} is not a valid WOLFMPK1 pack")
        ver, flags, count, index_offset, index_size = struct.unpack("<HHIQQ", header[8:])
        if ver != pack_media.VERSION or flags != 0:
            raise ValueError(f"unsupported WOLFMPK1 header version={ver} flags={flags}")
        f.seek(index_offset)
        enc_idx = f.read(index_size)
        if len(enc_idx) != index_size:
            raise ValueError(f"truncated index in {pak_path}")
        idx = pack_media.xor_bytes(enc_idx, index_offset)
        toc: dict[str, tuple[str, int, int, int]] = {}
        pos = 0
        for _ in range(count):
            plen, off, sz, crc = struct.unpack_from("<HQQI", idx, pos)
            pos += 22
            canonical = idx[pos : pos + plen].decode("utf-8")
            pos += plen
            toc[norm_key(canonical)] = (canonical, off, sz, crc)
        return toc


def read_pak_entry_bytes(pak_path: Path, entry: tuple[str, int, int, int]) -> bytes:
    canonical, off, sz, crc = entry
    with pak_path.open("rb") as f:
        f.seek(off)
        plain = pack_media.xor_bytes(f.read(sz), off)
    actual = zlib.crc32(plain) & 0xFFFFFFFF
    if actual != crc:
        raise ValueError(f"CRC mismatch on {canonical}: expected {crc:08X}, got {actual:08X}")
    return plain


def collect_loose_media(work_all: Path) -> dict[str, str]:
    """Return lowercase_rel -> canonical_rel for non-BasicData/non-save media files."""
    out: dict[str, str] = {}
    if not work_all.is_dir():
        return out
    for dirpath, dirnames, filenames in os.walk(work_all):
        dirnames[:] = [d for d in dirnames if d.lower() not in ("basicdata", "save")]
        for name in filenames:
            full = Path(dirpath) / name
            if full.suffix.lower() not in ALL_MEDIA_EXTENSIONS:
                continue
            rel = full.relative_to(work_all).as_posix()
            low = norm_key(rel)
            if low.startswith("basicdata/") or low.startswith("save/"):
                continue
            out[low] = rel
    return out


def load_json_artifact(path: Path) -> Any:
    br_path = path if path.suffix.lower() == ".br" else Path(str(path) + ".br")
    if path.is_file() and br_path != path and br_path.is_file():
        chosen = path if path.stat().st_mtime > br_path.stat().st_mtime else br_path
    elif br_path.is_file():
        chosen = br_path
    elif path.is_file():
        chosen = path
    else:
        raise FileNotFoundError(f"missing JSON artifact: {path} (or {br_path})")

    if chosen.suffix.lower() == ".br":
        return json.loads(brotli_json.decompress_bytes(chosen.read_bytes()).decode("utf-8"))
    return json.loads(chosen.read_text(encoding="utf-8"))


def extract_media_references(data: Any) -> set[str]:
    refs: set[str] = set()

    def walk(node: Any) -> None:
        if isinstance(node, dict):
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for item in node:
                walk(item)
        elif isinstance(node, str):
            raw = node.strip()
            norm = norm_key(raw)
            if (
                not norm
                or norm.startswith("<")
                or norm.startswith("save/")
                or "%" in raw
                or WOLF_CTRL_RE.search(raw)
            ):
                return
            if PurePosixPath(norm).suffix in REF_MEDIA_EXTENSIONS:
                refs.add(raw.replace("\\", "/"))

    walk(data)
    return refs


def resolve_key_in_index(key: str, index_keys: set[str] | dict[str, Any]) -> str | None:
    if key in index_keys:
        return key
    stem = PurePosixPath(key).with_suffix("").as_posix()
    suffix = PurePosixPath(key).suffix
    exts = TEXTURE_EXTENSIONS if suffix in TEXTURE_EXTENSIONS else AUDIO_EXTENSIONS if suffix in AUDIO_EXTENSIONS else ()
    for ext in exts:
        cand = stem + ext
        if cand in index_keys:
            return cand
    if suffix in TEXTURE_EXTENSIONS and key.startswith("picture/word_effect"):
        alias = "hword/" + PurePosixPath(key).name
        if alias in index_keys:
            return alias
        alias_stem = PurePosixPath(alias).with_suffix("").as_posix()
        for ext in TEXTURE_EXTENSIONS:
            cand = alias_stem + ext
            if cand in index_keys:
                return cand
    return None


def index_basenames(keys: set[str]) -> dict[tuple[str, str], list[str]]:
    groups: dict[tuple[str, str], list[str]] = defaultdict(list)
    for k in sorted(keys):
        pp = PurePosixPath(k)
        if pp.suffix in TEXTURE_EXTENSIONS:
            groups[("texture", pp.stem)].append(k)
        elif pp.suffix in AUDIO_EXTENSIONS:
            groups[("audio", pp.stem)].append(k)
    return groups


def audit_cross_platform(
    game_data_dir: Path,
    work_all_dir: Path,
    media_pak_path: Path,
    hd_pak_path: Path | None = None,
    allow_missing_references: bool = False,
) -> dict[str, Any]:
    errors: list[str] = []
    warnings: list[str] = []

    if not media_pak_path.is_file():
        errors.append(f"media.pak not found: {media_pak_path}")
        return {
            "ok": False,
            "summary": {"errors": len(errors), "warnings": 0},
            "errors": errors,
            "warnings": warnings,
        }

    try:
        pak_toc = read_pak_toc(media_pak_path)
    except Exception as exc:  # noqa: BLE001
        errors.append(f"invalid media.pak: {exc}")
        return {
            "ok": False,
            "summary": {"errors": len(errors), "warnings": 0},
            "errors": errors,
            "warnings": warnings,
        }

    pak_keys = set(pak_toc.keys())
    loose_map = collect_loose_media(work_all_dir)
    loose_keys = set(loose_map.keys())

    # 1. Font presence check (media.pak or loose work_all).
    font_in_pak = sorted(k for k in pak_keys if k in FONT_NAMES)
    font_loose = sorted(k for k in loose_keys if k in FONT_NAMES)
    if not font_in_pak and not font_loose:
        errors.append("Lolita.ttf missing from both media.pak and loose work/all")

    # 2. Loose non-BasicData/non-save media vs media.pak.
    loose_not_in_pak = sorted(
        loose_map[k] for k in (loose_keys - pak_keys) if k not in FONT_NAMES
    )
    for rel in loose_not_in_pak:
        errors.append(f"loose work/all asset missing from media.pak: {rel}")
    pak_not_in_loose = sorted(k for k in (pak_keys - loose_keys) if k not in FONT_NAMES)

    # 3. Basename collisions and loose vs media.pak determinism using AssetDB DIR_PRIORITY.
    pak_basenames = index_basenames(pak_keys)
    loose_basenames = index_basenames(loose_keys)
    combined_basenames = index_basenames(pak_keys | loose_keys)

    collisions: list[dict[str, Any]] = []
    for (kind, stem), paths in sorted(combined_basenames.items()):
        if len(paths) > 1:
            collisions.append(
                {
                    "kind": kind,
                    "basename": stem,
                    "winner": pick_deterministic_winner(paths),
                    "candidates": sorted(paths),
                }
            )

    determinism_mismatches: list[dict[str, Any]] = []
    for key in sorted(set(pak_basenames.keys()) & set(loose_basenames.keys())):
        pak_winner = pick_deterministic_winner(pak_basenames[key])
        loose_winner = pick_deterministic_winner(loose_basenames[key])
        if pak_winner != loose_winner:
            kind, stem = key
            determinism_mismatches.append(
                {
                    "kind": kind,
                    "basename": stem,
                    "loose_winner": loose_winner,
                    "pak_winner": pak_winner,
                }
            )
            errors.append(
                f"basename determinism mismatch ({kind}:{stem}): "
                f"loose='{loose_winner}' != media.pak='{pak_winner}'"
            )

    # 4. Scan game_data/programs.json and game_data/database.json references.
    program_refs: set[str] = set()
    database_refs: set[str] = set()
    for fname, sink in (("programs.json", program_refs), ("database.json", database_refs)):
        fpath = game_data_dir / fname
        try:
            payload = load_json_artifact(fpath)
            sink.update(extract_media_references(payload))
        except Exception as exc:  # noqa: BLE001
            errors.append(f"failed to load {fpath}: {exc}")

    all_refs = sorted(program_refs | database_refs, key=str.lower)
    missing_in_pak_existing: list[str] = []
    missing_refs: list[str] = []
    for ref in all_refs:
        key = norm_key(ref)
        if resolve_key_in_index(key, pak_keys) is not None:
            continue
        if resolve_key_in_index(key, loose_keys) is not None:
            missing_in_pak_existing.append(ref)
            errors.append(f"referenced existing asset missing from media.pak: {ref}")
        else:
            missing_refs.append(ref)
            msg = f"referenced asset missing from media.pak and work/all: {ref}"
            if allow_missing_references:
                warnings.append(msg)
            else:
                errors.append(msg)

    # 5. Optional media_hd.pak vs media.pak dimension/aspect-ratio check.
    hd_checked = 0
    hd_mismatches: list[str] = []
    if hd_pak_path is not None:
        if not hd_pak_path.is_file():
            errors.append(f"hd_pak not found: {hd_pak_path}")
        else:
            try:
                hd_toc = read_pak_toc(hd_pak_path)
            except Exception as exc:  # noqa: BLE001
                errors.append(f"invalid media_hd.pak: {exc}")
                hd_toc = {}
            if "hd_manifest.json" in hd_toc:
                try:
                    from PIL import Image
                except ImportError:
                    Image = None  # type: ignore[assignment]
                    errors.append("Pillow is required to validate media_hd.pak image dimensions")
                try:
                    manifest_raw = read_pak_entry_bytes(hd_pak_path, hd_toc["hd_manifest.json"])
                    manifest_obj = json.loads(manifest_raw.decode("utf-8"))
                    hd_entries = manifest_obj.get("entries") or {}
                    if not isinstance(hd_entries, dict):
                        raise ValueError("hd_manifest.json 'entries' is not an object")
                except Exception as exc:  # noqa: BLE001
                    errors.append(f"unreadable hd_manifest.json in {hd_pak_path}: {exc}")
                    hd_entries = {}

                if Image is not None:
                    for raw_key, item in sorted(hd_entries.items()):
                        if not isinstance(item, dict):
                            continue
                        key = norm_key(str(raw_key))
                        hd_rel = norm_key(str(item.get("path", "")))
                        scale = float(item.get("scale", 2.0))
                        if not hd_rel or hd_rel not in hd_toc:
                            msg = f"HD entry missing in media_hd.pak: {raw_key} -> {hd_rel}"
                            hd_mismatches.append(msg)
                            errors.append(msg)
                            continue
                        base_1x_key = resolve_key_in_index(key, pak_keys)
                        if base_1x_key is None:
                            continue
                        hd_checked += 1
                        try:
                            hd_bytes = read_pak_entry_bytes(hd_pak_path, hd_toc[hd_rel])
                            base_bytes = read_pak_entry_bytes(media_pak_path, pak_toc[base_1x_key])
                            with Image.open(io.BytesIO(hd_bytes)) as img_hd, Image.open(io.BytesIO(base_bytes)) as img_1x:
                                w_hd, h_hd = img_hd.size
                                w_1x, h_1x = img_1x.size
                        except Exception as exc:  # noqa: BLE001
                            msg = f"HD decode failed for {raw_key}: {exc}"
                            hd_mismatches.append(msg)
                            errors.append(msg)
                            continue
                        if w_1x <= 0 or h_1x <= 0 or w_hd <= 0 or h_hd <= 0:
                            msg = f"HD invalid zero dimension for {raw_key}: 1x=({w_1x},{h_1x}) hd=({w_hd},{h_hd})"
                            hd_mismatches.append(msg)
                            errors.append(msg)
                            continue
                        aspect_delta = abs((w_hd / h_hd) - (w_1x / h_1x))
                        width_scale = w_hd / w_1x
                        if aspect_delta >= 1e-3 or not math.isclose(width_scale, scale, rel_tol=0.0, abs_tol=1e-6):
                            msg = (
                                f"HD dimension/aspect mismatch for {raw_key}: "
                                f"1x={w_1x}x{h_1x} hd={w_hd}x{h_hd} scale={scale} "
                                f"(w_ratio={width_scale:.6f}, aspect_delta={aspect_delta:.6f})"
                            )
                            hd_mismatches.append(msg)
                            errors.append(msg)

    summary = {
        "pak_entries": len(pak_keys),
        "loose_entries": len(loose_keys),
        "program_refs": len(program_refs),
        "database_refs": len(database_refs),
        "existing_refs_missing_in_pak": len(missing_in_pak_existing),
        "missing_refs": len(missing_refs),
        "loose_not_in_pak": len(loose_not_in_pak),
        "pak_not_in_loose": len(pak_not_in_loose),
        "basename_collisions": len(collisions),
        "determinism_mismatches": len(determinism_mismatches),
        "hd_checked": hd_checked,
        "hd_mismatches": len(hd_mismatches),
        "font_in_pak": bool(font_in_pak),
        "font_loose": bool(font_loose),
        "warnings": len(warnings),
        "errors": len(errors),
    }
    return {
        "ok": len(errors) == 0,
        "summary": summary,
        "errors": errors,
        "warnings": warnings,
        "collisions": collisions,
        "determinism_mismatches": determinism_mismatches,
        "loose_not_in_pak": loose_not_in_pak,
        "missing_in_pak_existing": missing_in_pak_existing,
        "missing_refs": missing_refs,
        "hd_mismatches": hd_mismatches,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game-data", type=Path, default=REPO_ROOT / "game_data")
    parser.add_argument("--work-all", type=Path, default=REPO_ROOT / "work/all")
    parser.add_argument("--media-pak", type=Path, default=None)
    parser.add_argument("--hd-pak", type=Path, default=None)
    parser.add_argument("--json-out", type=Path, default=None)
    parser.add_argument("--allow-missing-references", action="store_true")
    args = parser.parse_args(argv)

    media_pak = args.media_pak if args.media_pak is not None else (args.work_all / "media.pak")
    report = audit_cross_platform(
        game_data_dir=args.game_data,
        work_all_dir=args.work_all,
        media_pak_path=media_pak,
        hd_pak_path=args.hd_pak,
        allow_missing_references=args.allow_missing_references,
    )

    if args.json_out is not None:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    s = report["summary"]
    tag = "CROSS_PLATFORM_AUDIT_OK" if report["ok"] else "CROSS_PLATFORM_AUDIT_FAIL"
    print(
        f"{tag} pak={s.get('pak_entries', 0)} loose={s.get('loose_entries', 0)} "
        f"refs={s.get('program_refs', 0)}+{s.get('database_refs', 0)} "
        f"collisions={s.get('basename_collisions', 0)} det_err={s.get('determinism_mismatches', 0)} "
        f"loose_only={s.get('loose_not_in_pak', 0)} missing_existing={s.get('existing_refs_missing_in_pak', 0)} "
        f"missing_refs={s.get('missing_refs', 0)} hd={s.get('hd_checked', 0)}/{s.get('hd_mismatches', 0)} "
        f"font_pak={int(bool(s.get('font_in_pak', False)))} font_loose={int(bool(s.get('font_loose', False)))} "
        f"warnings={s.get('warnings', 0)} errors={s.get('errors', 0)}"
    )
    for err in report["errors"][:20]:
        print(f"ERROR: {err}", file=sys.stderr)
    if len(report["errors"]) > 20:
        print(f"... and {len(report['errors']) - 20} more errors", file=sys.stderr)
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
