#!/usr/bin/env python3
"""Inject LWSMF runtime data (`game_data/` + `work/all/media.pak` + schemas) into an unsigned iOS IPA.

Pure-Python ZIP manipulation: works on Windows without macOS or external zip CLI,
preserves original Unix permissions & symlinks via `ZipInfo.external_attr`, and
verifies output ZIP CRCs before committing the atomic write.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
import zipfile
from datetime import datetime
from pathlib import Path, PurePosixPath

GAME_DATA_FILES = ("manifest.json", "programs.json", "database.json", "map_tiles.json")
PROJECT_FILES = ("CDataBase.project", "DataBase.project", "SysDatabase.project")
STORED_EXTENSIONS = {".ipa", ".zip", ".pak", ".ogg", ".mp3", ".wav", ".webp", ".png", ".jpg", ".jpeg"}


def fail(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)


def safe_member_name(name: str) -> str:
    if not name:
        fail("IPA contains an empty archive member name")
    if "\x00" in name:
        fail("IPA contains NUL in archive member name")
    if "\\" in name:
        fail(f"IPA contains backslash in archive member name: {name}")
    if name.startswith("/"):
        fail(f"IPA contains absolute path: {name}")
    parts = PurePosixPath(name.rstrip("/")).parts
    if not parts:
        fail(f"IPA contains empty normalized path: {name}")
    if parts[0] == ".." or any(part == ".." for part in parts):
        fail(f"IPA contains path traversal: {name}")
    if re.match(r"^[A-Za-z]:", parts[0]):
        fail(f"IPA contains drive-qualified path: {name}")
    return name


def app_prefix_from_names(names: list[str]) -> str:
    apps: set[str] = set()
    for name in names:
        parts = name.rstrip("/").split("/")
        if len(parts) >= 2 and parts[0] == "Payload" and parts[1].endswith(".app"):
            apps.add(parts[1])
    if not apps:
        fail("IPA has no Payload/*.app bundle")
    if len(apps) > 1:
        fail("IPA has multiple Payload/*.app bundles: " + ", ".join(sorted(apps)))
    return f"Payload/{next(iter(apps))}"


def clone_zipinfo(info: zipfile.ZipInfo) -> zipfile.ZipInfo:
    clone = zipfile.ZipInfo(info.filename, date_time=info.date_time)
    clone.compress_type = (
        info.compress_type
        if info.compress_type in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED)
        else zipfile.ZIP_DEFLATED
    )
    clone.external_attr = info.external_attr
    clone.internal_attr = info.internal_attr
    clone.create_system = info.create_system
    clone.comment = info.comment
    clone.flag_bits = info.flag_bits & 0x800
    return clone


def data_zipinfo(arcname: str, is_dir: bool, timestamp: float, mode: int) -> zipfile.ZipInfo:
    try:
        dt = datetime.fromtimestamp(timestamp)
        date_time = (min(2107, max(1980, dt.year)), dt.month, dt.day, dt.hour, dt.minute, dt.second)
    except (OSError, OverflowError, ValueError):
        date_time = (1980, 1, 1, 0, 0, 0)
    suffix = "/" if is_dir else ""
    info = zipfile.ZipInfo(arcname + suffix, date_time=date_time)
    info.create_system = 3
    info.external_attr = (mode & 0xFFFF) << 16
    if is_dir:
        info.external_attr |= 0x10
        info.compress_type = zipfile.ZIP_STORED
    elif Path(arcname).suffix.lower() in STORED_EXTENSIONS:
        info.compress_type = zipfile.ZIP_STORED
    else:
        info.compress_type = zipfile.ZIP_DEFLATED
    return info


def collect_payload_files(
    app_prefix: str,
    game_data_dir: Path,
    work_all_dir: Path,
    media_pak_override: Path | None,
    font_path: Path | None,
) -> tuple[dict[str, tuple[bytes, float, int]], set[str]]:
    gd = game_data_dir.resolve()
    wa = work_all_dir.resolve()
    files: dict[str, tuple[bytes, float, int]] = {}
    dirs = {
        f"{app_prefix}/game_data",
        f"{app_prefix}/work",
        f"{app_prefix}/work/all",
        f"{app_prefix}/work/all/BasicData",
    }

    for name in GAME_DATA_FILES:
        src = gd / name
        if not src.is_file() or src.stat().st_size <= 0:
            fail(f"missing or empty game_data/{name}: {src}")
        st = src.stat()
        if name in ("programs.json", "database.json", "map_tiles.json"):
            raw = json.dumps(json.loads(src.read_text(encoding="utf-8")), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        else:
            raw = src.read_bytes()
        files[f"{app_prefix}/game_data/{name}"] = (raw, st.st_mtime, stat.S_IFREG | (st.st_mode & 0o777))

    for name in PROJECT_FILES:
        src = wa / "BasicData" / name
        if not src.is_file() or src.stat().st_size <= 0:
            fail(f"missing or empty BasicData/{name}: {src}")
        st = src.stat()
        files[f"{app_prefix}/work/all/BasicData/{name}"] = (src.read_bytes(), st.st_mtime, stat.S_IFREG | (st.st_mode & 0o777))

    pak_src = (media_pak_override or (wa / "media.pak")).resolve()
    if not pak_src.is_file() or pak_src.stat().st_size <= 0:
        fail(f"missing or empty media.pak: {pak_src}")
    st = pak_src.stat()
    files[f"{app_prefix}/work/all/media.pak"] = (pak_src.read_bytes(), st.st_mtime, stat.S_IFREG | (st.st_mode & 0o777))

    if font_path is not None:
        f = font_path.resolve()
        if not f.is_file() or f.stat().st_size <= 0:
            fail(f"font file not found or empty: {font_path}")
        st = f.stat()
        files[f"{app_prefix}/work/all/Lolita.ttf"] = (f.read_bytes(), st.st_mtime, stat.S_IFREG | (st.st_mode & 0o777))

    return files, dirs


def inject(
    ipa: Path,
    game_data_dir: Path,
    work_all_dir: Path,
    output: Path,
    media_pak_override: Path | None,
    font: Path | None,
    force: bool,
    dry_run: bool = False,
) -> Path:
    source = ipa.resolve()
    if not source.is_file() or not zipfile.is_zipfile(source):
        fail(f"input IPA is missing or not a valid ZIP archive: {ipa}")

    destination = output.resolve()
    if destination == source and not dry_run:
        fail("output IPA must differ from input IPA")
    if destination.exists() and not force and not dry_run:
        fail(f"output already exists (pass --force): {destination}")

    with zipfile.ZipFile(source, "r") as archive:
        infos = archive.infolist()
        names = [safe_member_name(info.filename) for info in infos]
        app_prefix = app_prefix_from_names(names)
        payload_files, payload_dirs = collect_payload_files(
            app_prefix, game_data_dir, work_all_dir, media_pak_override, font
        )

        if dry_run:
            total = sum(len(v[0]) for v in payload_files.values())
            print(f"IOS_INJECT_DRY_RUN app={app_prefix} files={len(payload_files)} payload_bytes={total} out={destination}")
            return destination

        destination.parent.mkdir(parents=True, exist_ok=True)
        temp = destination.with_name(destination.name + ".tmp")
        if temp.exists():
            temp.unlink()

        replace_prefixes = (f"{app_prefix}/game_data", f"{app_prefix}/work")
        seen: set[str] = set()
        try:
            with zipfile.ZipFile(temp, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6, allowZip64=True) as out_zip:
                for info, name in zip(infos, names):
                    norm = name.rstrip("/")
                    if any(norm == p or norm.startswith(p + "/") for p in replace_prefixes):
                        continue
                    if name in seen:
                        fail(f"duplicate archive member in input IPA: {name}")
                    seen.add(name)
                    out_zip.writestr(clone_zipinfo(info), archive.read(info))

                now_ts = datetime.now().timestamp()
                for d in sorted(payload_dirs, key=str.lower):
                    out_zip.writestr(data_zipinfo(d, True, now_ts, stat.S_IFDIR | 0o755), b"")
                for arcname in sorted(payload_files, key=str.lower):
                    raw, mtime, mode = payload_files[arcname]
                    out_zip.writestr(data_zipinfo(arcname, False, mtime, mode), raw)

            with zipfile.ZipFile(temp, "r") as check_zip:
                check_names = [safe_member_name(i.filename) for i in check_zip.infolist()]
                if app_prefix_from_names(check_names) != app_prefix:
                    fail("output IPA bundle mismatch")
                for req in (f"{app_prefix}/game_data/programs.json", f"{app_prefix}/work/all/media.pak"):
                    if req not in check_names:
                        fail(f"output IPA missing injected entry: {req}")
                bad = check_zip.testzip()
                if bad is not None:
                    fail(f"output IPA CRC check failed at: {bad}")

            os.replace(temp, destination)
        finally:
            if temp.exists():
                temp.unlink()

    sha = hashlib.sha256(destination.read_bytes()).hexdigest()
    print(f"IOS_INJECT_OK out={destination} sha256={sha}")
    return destination


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("ipa", type=Path, help="Engine-only unsigned IPA")
    parser.add_argument("--game-data", type=Path, default=Path("game_data"))
    parser.add_argument("--work-all", type=Path, default=Path("work/all"))
    parser.add_argument("--media-pak", type=Path, default=None)
    parser.add_argument("--font", type=Path, default=None)
    parser.add_argument("--out", type=Path, default=None)
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)

    out = args.out
    if out is None:
        stem = args.ipa.name[:-4] if args.ipa.name.lower().endswith(".ipa") else args.ipa.name
        out = args.ipa.with_name(f"{stem}-full.ipa")

    inject(args.ipa, args.game_data, args.work_all, out, args.media_pak, args.font, args.force, args.dry_run)
    return 0


if __name__ == "__main__":
    sys.exit(main())
