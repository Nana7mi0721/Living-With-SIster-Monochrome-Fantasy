#!/usr/bin/env python3
"""Stage and verify Android mobile_data payloads and exported APKs for LWSMF.

Adapts the proven APK packaging pattern to LWSMF's data contract:
- Compacted JSON files (`game_data/{manifest,programs,database,map_tiles}.json`)
  are staged with `.blob` suffixes so Godot packs them cleanly as raw payloads.
- Database schemas (`work/all/BasicData/*.project`) are staged with `.blob`.
- `work/all/media.pak` is staged with `.blob.cfb` (so Android AssetManager keeps
  it STORED/uncompressed rather than deflating 400+ MB on APK build) and marked
  `"direct_res": true` in `mobile_data_manifest.json` for zero-copy `res://`
  reading without unpacking into `user://`.
- `mobile_data_manifest.json` records payload_digest & staging_version.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import struct
import sys
import zipfile
from pathlib import Path, PurePosixPath

SCRIPT_DIR = Path(__file__).resolve().parent
TOOLS_DIR = SCRIPT_DIR.parent
REPO_ROOT = SCRIPT_DIR.parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import brotli_json  # noqa: E402
import pack_media  # noqa: E402

MANIFEST_FILENAME = "mobile_data_manifest.json"
BLOB_SUFFIX = ".blob"
MEDIA_BLOB_SUFFIX = ".blob.cfb"
FONT_PAK_NAMES = {"lolita.ttf", "data/lolita.ttf"}
GAME_DATA_FILES = ("manifest.json", "programs.json", "database.json", "map_tiles.json")
BROTLI_GAME_DATA_FILES = set(brotli_json.BROTLI_JSON_NAMES)
PROJECT_FILES = ("CDataBase.project", "DataBase.project", "SysDatabase.project")
FORBIDDEN_SAVE_RE = re.compile(r"^(savedata.*\.sav|save_\d+\.json|quicksave\.json|system\.json)(\.bak)?$", re.I)
FORBIDDEN_APK_PREFIXES = (
    "addons/godot_ai/",
    "res://tests/",
    "res://tests_gd/",
    "assets/mobile_data/work/all/save/",
)


def fail(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)


def is_within(path: Path, parent: Path) -> bool:
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def ensure_safe_dest(dest: Path, allow_temp: bool = False) -> Path:
    if dest.is_symlink():
        fail(f"refusing symlink staging destination: {dest}")
    resolved = dest.resolve()
    allowed_mobile = (REPO_ROOT / "godot" / "mobile_data").resolve()
    allowed_work_out = (REPO_ROOT / "work" / "out").resolve()
    if resolved == allowed_mobile:
        return resolved
    if resolved != allowed_work_out and is_within(resolved, allowed_work_out):
        return resolved
    if allow_temp:
        return resolved
    fail(f"refusing destination outside godot/mobile_data or work/out: {dest}")
    raise AssertionError("unreachable")


def pak_has_font(pak_path: Path) -> bool:
    try:
        names = pack_media.read_pak_index(pak_path)
    except (OSError, ValueError, struct.error, IndexError):
        return False
    return any(n.lstrip("/") in FONT_PAK_NAMES for n in names)


def apk_pak_has_font(zf: zipfile.ZipFile, entry: str) -> bool:
    try:
        with zf.open(entry, "r") as f:
            header = f.read(pack_media.HEADER_SIZE)
            if len(header) < pack_media.HEADER_SIZE or header[:8] != pack_media.MAGIC:
                return False
            ver, flags, count, index_offset, index_size = struct.unpack("<HHIQQ", header[8:])
            if ver != pack_media.VERSION or flags != 0:
                return False
            f.seek(index_offset)
            enc_idx = f.read(index_size)
            if len(enc_idx) != index_size:
                return False
            idx = pack_media.xor_bytes(enc_idx, index_offset)
            pos = 0
            for _ in range(count):
                plen, _off, _sz, _crc = struct.unpack_from("<HQQI", idx, pos)
                pos += 22
                rel = idx[pos : pos + plen].decode("utf-8", errors="replace").replace("\\", "/").lstrip("/").lower()
                pos += plen
                if rel in FONT_PAK_NAMES:
                    return True
            return False
    except (OSError, ValueError, struct.error, IndexError, zipfile.BadZipFile):
        return False


def collect_plan(
    game_data_dir: Path,
    work_all_dir: Path,
    media_pak_override: Path | None = None,
    font_path: Path | None = None,
    allow_missing_font: bool = False,
) -> list[tuple[str, Path, bool]]:
    """Return list of (logical_rel_posix, source_path, compact_json)."""
    items: list[tuple[str, Path, bool]] = []
    gd = game_data_dir.resolve()
    wa = work_all_dir.resolve()
    for name in GAME_DATA_FILES:
        src = gd / name
        if not src.is_file() or src.stat().st_size <= 0:
            fail(f"missing or empty game_data file: {src}")
        compact = name in ("programs.json", "database.json", "map_tiles.json")
        items.append((f"game_data/{name}", src, compact))

    for name in PROJECT_FILES:
        src = wa / "BasicData" / name
        if not src.is_file() or src.stat().st_size <= 0:
            fail(f"missing or empty project schema: {src}")
        items.append((f"work/all/BasicData/{name}", src, False))

    pak_src = (media_pak_override or (wa / "media.pak")).resolve()
    if not pak_src.is_file() or pak_src.stat().st_size <= 0:
        fail(f"missing or empty media.pak: {pak_src}")
    items.append(("work/all/media.pak", pak_src, False))

    if font_path is not None:
        f = font_path.resolve()
        if not f.is_file() or f.stat().st_size <= 0:
            fail(f"font file missing or empty: {font_path}")
        items.append(("work/all/Lolita.ttf", f, False))
    elif not allow_missing_font and not pak_has_font(pak_src):
        fail(
            "Lolita.ttf missing: pass --font /path/to/Lolita.ttf, include Lolita.ttf in media.pak, "
            "or pass --allow-missing-font"
        )

    return items


def _compact_json_bytes(src: Path) -> bytes:
    return brotli_json.compact_json_bytes(src)


def payload_digest(items: list[tuple[str, Path, bool]], use_brotli: bool = False) -> str:
    h = hashlib.sha256()
    if use_brotli:
        h.update(b"brotli=1\0")
    for logical, src, compact in sorted(items, key=lambda kv: kv[0]):
        h.update(logical.encode("utf-8") + b"\0")
        if compact:
            raw = _compact_json_bytes(src)
            h.update(brotli_json.compress_bytes(raw, quality=11) if use_brotli else raw)
        else:
            with src.open("rb") as f:
                for chunk in iter(lambda: f.read(1 << 20), b""):
                    h.update(chunk)
        h.update(b"\0")
    return h.hexdigest()


def cmd_plan(args: argparse.Namespace) -> int:
    items = collect_plan(
        args.game_data,
        args.work_all,
        args.media_pak,
        args.font,
        allow_missing_font=args.allow_missing_font,
    )
    total_raw = sum(p.stat().st_size for _, p, _ in items)
    media_override = str(args.media_pak) if args.media_pak else "-"
    print(f"ANDROID_DATA_PLAN entries={len(items)} source_bytes={total_raw} brotli={args.brotli} media_override={media_override}")
    for logical, src, compact in items:
        direct_res = logical.endswith("media.pak")
        logical_out = (logical + ".br") if (args.brotli and compact) else logical
        suffix = MEDIA_BLOB_SUFFIX if direct_res else BLOB_SUFFIX
        print(
            f"  {logical_out}{suffix:10s} <- {src.name} "
            f"({src.stat().st_size / 1024 / 1024:.2f} MB, compact_json={compact}, brotli={args.brotli and compact}, direct_res={direct_res})"
        )
    return 0


def cmd_stage(args: argparse.Namespace) -> int:
    safe_dest = ensure_safe_dest(args.dest, allow_temp=args.allow_temp)
    items = collect_plan(
        args.game_data,
        args.work_all,
        args.media_pak,
        args.font,
        allow_missing_font=args.allow_missing_font,
    )
    digest = payload_digest(items, use_brotli=args.brotli)
    staging_version = f"{args.version.strip()}+{digest[:16]}"

    if safe_dest.exists():
        shutil.rmtree(safe_dest)
    safe_dest.mkdir(parents=True, exist_ok=True)

    staged_files: list[dict[str, object]] = []
    for logical, src, compact in items:
        if FORBIDDEN_SAVE_RE.match(PurePosixPath(logical).name):
            fail(f"refusing player save file in mobile stage: {logical}")
        direct_res = logical.endswith("media.pak")
        logical_out = (logical + ".br") if (args.brotli and compact) else logical
        suffix = MEDIA_BLOB_SUFFIX if direct_res else BLOB_SUFFIX
        rel_out = logical_out + suffix
        dst = safe_dest / rel_out
        dst.parent.mkdir(parents=True, exist_ok=True)
        if compact:
            raw = _compact_json_bytes(src)
            if args.brotli:
                raw = brotli_json.compress_bytes(raw, quality=11)
            dst.write_bytes(raw)
        else:
            shutil.copy2(src, dst)
        staged_files.append(
            {
                "logical": logical_out,
                "staged": rel_out,
                "bytes": dst.stat().st_size,
                "direct_res": direct_res,
            }
        )

    manifest_obj = {
        "schema": "lwsmf.mobile_data",
        "schema_version": 1,
        "version": args.version.strip(),
        "staging_version": staging_version,
        "payload_digest": digest,
        "files": staged_files,
    }
    manifest_tmp = safe_dest / (MANIFEST_FILENAME + ".tmp")
    manifest_tmp.write_text(json.dumps(manifest_obj, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    os.replace(manifest_tmp, safe_dest / MANIFEST_FILENAME)

    total_bytes = sum(int(f["bytes"]) for f in staged_files)
    print(f"ANDROID_DATA_STAGE_OK dest={safe_dest} version={staging_version} files={len(staged_files)} bytes={total_bytes}")
    return 0


def cmd_verify_apk(args: argparse.Namespace) -> int:
    apk = args.apk.resolve()
    if not apk.is_file() or not zipfile.is_zipfile(apk):
        fail(f"not a valid APK/ZIP archive: {apk}")

    errors: list[str] = []
    with zipfile.ZipFile(apk, "r") as zf:
        infos = {info.filename: info for info in zf.infolist()}
        required = [
            f"assets/mobile_data/{MANIFEST_FILENAME}",
            f"assets/mobile_data/work/all/media.pak{MEDIA_BLOB_SUFFIX}",
            *(f"assets/mobile_data/work/all/BasicData/{name}{BLOB_SUFFIX}" for name in PROJECT_FILES),
        ]
        for req in required:
            if req not in infos or infos[req].file_size <= 0:
                errors.append(f"missing or empty APK asset: {req}")
        for name in GAME_DATA_FILES:
            plain = f"assets/mobile_data/game_data/{name}{BLOB_SUFFIX}"
            br = f"assets/mobile_data/game_data/{name}.br{BLOB_SUFFIX}"
            has_plain = plain in infos and infos[plain].file_size > 0
            has_br = (name in BROTLI_GAME_DATA_FILES) and (br in infos and infos[br].file_size > 0)
            if not has_plain and not has_br:
                errors.append(f"missing or empty APK asset: {plain}")

        pak_entry = f"assets/mobile_data/work/all/media.pak{MEDIA_BLOB_SUFFIX}"
        if pak_entry in infos and infos[pak_entry].compress_type != zipfile.ZIP_STORED:
            errors.append(f"{pak_entry} must use ZIP_STORED (got {infos[pak_entry].compress_type})")

        font_entry = f"assets/mobile_data/work/all/Lolita.ttf{BLOB_SUFFIX}"
        has_font_blob = font_entry in infos and infos[font_entry].file_size > 0
        has_pak_font = (
            pak_entry in infos
            and infos[pak_entry].file_size > 0
            and apk_pak_has_font(zf, pak_entry)
        )
        if not getattr(args, "allow_missing_font", False) and not has_font_blob and not has_pak_font:
            errors.append(f"missing Lolita.ttf: neither {font_entry} nor lolita.ttf inside {pak_entry}")

        has_lwsmf_dll = any(
            name.startswith("assets/.godot/mono/publish/")
            and name.endswith("/Lwsmf.dll")
            and infos[name].file_size > 0
            for name in infos
        )
        if not has_lwsmf_dll:
            errors.append("APK missing non-empty assets/.godot/mono/publish/*/Lwsmf.dll")

        has_arm64_lib = any(name.startswith("lib/arm64-v8a/") and name.endswith(".so") for name in infos)
        if not has_arm64_lib:
            errors.append("APK has no lib/arm64-v8a/*.so native libraries")

        for name in infos:
            low = name.lower()
            for pref in FORBIDDEN_APK_PREFIXES:
                if pref in low:
                    errors.append(f"forbidden APK entry matching {pref}: {name}")
            if low.endswith(".pdb"):
                errors.append(f"forbidden debug symbol file in APK: {name}")
            base = PurePosixPath(name).name
            if FORBIDDEN_SAVE_RE.match(base):
                errors.append(f"forbidden player save slot in APK: {name}")

    if errors:
        print("ANDROID_APK_VERIFY_FAIL")
        for err in errors:
            print(f"- {err}")
        return 1

    print(f"ANDROID_APK_VERIFY_OK apk={apk.name} entries={len(infos)}")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    for name in ("plan", "stage"):
        p = sub.add_parser(name)
        p.add_argument("--game-data", type=Path, default=REPO_ROOT / "game_data")
        p.add_argument("--work-all", type=Path, default=REPO_ROOT / "work/all")
        p.add_argument("--media-pak", type=Path, default=None, help="Optional slim media.pak override")
        p.add_argument("--font", type=Path, default=None, help="Optional Lolita.ttf path")
        p.add_argument("--allow-missing-font", action="store_true", help="Allow staging without Lolita.ttf")
        p.add_argument("--brotli", action="store_true", help="Compress large game_data JSONs as .json.br")
        if name == "stage":
            p.add_argument("--version", default="0.1.0")
            p.add_argument("--dest", type=Path, default=REPO_ROOT / "godot/mobile_data")
            p.add_argument("--allow-temp", action="store_true")

    pv = sub.add_parser("verify-apk")
    pv.add_argument("apk", type=Path)
    pv.add_argument("--allow-missing-font", action="store_true", help="Allow APK verification without Lolita.ttf")

    args = parser.parse_args(argv)
    if args.command == "plan":
        return cmd_plan(args)
    if args.command == "stage":
        return cmd_stage(args)
    return cmd_verify_apk(args)


if __name__ == "__main__":
    sys.exit(main())
