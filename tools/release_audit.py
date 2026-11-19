#!/usr/bin/env python3
"""Release payload audit for a staged LWSMF release directory.

A release package is self-contained: game binaries/runtime, converted
game_data, and the work/all runtime asset tree staged beside game/. This audit
checks the required layout and that every manifest asset is present. It does not
reject retail-format extensions or fingerprint-matching files; the distribution
policy is defined by docs/godot/DECISIONS.md D-53 (which supersedes D-11).
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REQUIRED_FILES = [
    "game/Lwsmf.exe",
    "game/Lwsmf.pck",
    "game/data_Lwsmf_windows_x86_64/Lwsmf.dll",
    "game_data/manifest.json",
    "game_data/programs.json",
    "game_data/database.json",
    "game_data/map_tiles.json",
    "work/all/BasicData/Game.dat",
    "work/all/BasicData/CommonEvent.dat",
    "work/all/BasicData/DataBase.dat",
    "work/all/BasicData/CDataBase.dat",
    "work/all/BasicData/SysDatabase.dat",
    "work/all/BasicData/TileSetData.dat",
    "work/all/BasicData/MapTree.dat",
    "converter/cli.py",
    "converter/convert.bat",
    "README.txt",
]

REQUIRED_ASSET_DIRS = [
    "work/all/BasicData",
    "work/all/MapChip",
    "work/all/MapData",
    "work/all/SystemFile",
]


def _nonempty_file(path: Path) -> bool:
    return path.is_file() and path.stat().st_size > 0


def _logical_asset_path(item: dict) -> str:
    value = item.get("logical") or item.get("path") or item.get("source") or ""
    return str(value).replace("\\", "/").lstrip("/")


def audit(dist: Path) -> tuple[list[str], dict[str, int]]:
    errors: list[str] = []
    stats = {"files": 0, "bytes": 0, "assets": 0, "asset_bytes": 0, "mps": 0}

    if not dist.is_dir():
        return [f"dist directory not found: {dist}"], stats

    for rel in REQUIRED_FILES:
        path = dist / rel
        if not _nonempty_file(path):
            errors.append(f"missing or empty: {rel}")

    for rel in REQUIRED_ASSET_DIRS:
        directory = dist / rel
        if not directory.is_dir() or not any(p.is_file() for p in directory.rglob("*")):
            errors.append(f"missing or empty asset directory: {rel}")

    save_dir = dist / "work/all/save"
    if save_dir.exists():
        errors.append("work/all/save must not be shipped (player save data)")

    manifest_path = dist / "game_data/manifest.json"
    manifest: dict = {}
    if manifest_path.is_file():
        try:
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        except Exception as exc:  # noqa: BLE001 - report malformed manifests cleanly
            errors.append(f"invalid game_data/manifest.json: {exc}")
            manifest = {}

    assets = manifest.get("assets") or []
    for item in assets:
        if not isinstance(item, dict):
            continue
        logical = _logical_asset_path(item)
        if not logical:
            continue
        source = dist / "work/all" / logical
        if not _nonempty_file(source):
            errors.append(f"manifest asset missing: work/all/{logical}")
            continue
        stats["assets"] += 1
        stats["asset_bytes"] += source.stat().st_size

    artifacts = manifest.get("artifacts") or {}
    for name, item in artifacts.items():
        if not isinstance(item, dict):
            continue
        rel = str(item.get("path") or "").replace("\\", "/").lstrip("/")
        if not rel:
            continue
        path = dist / "game_data" / rel
        if not _nonempty_file(path):
            errors.append(f"manifest artifact missing: game_data/{rel}")
            continue
        expected = item.get("bytes")
        if isinstance(expected, int) and path.stat().st_size != expected:
            errors.append(
                f"manifest artifact size mismatch: game_data/{rel} "
                f"expected={expected} actual={path.stat().st_size}"
            )

    map_data = dist / "work/all/MapData"
    if map_data.is_dir():
        stats["mps"] = sum(1 for p in map_data.rglob("*.mps") if p.is_file())
        if stats["mps"] == 0:
            errors.append("work/all/MapData has no .mps map files")

    for path in dist.rglob("*"):
        if path.is_file():
            stats["files"] += 1
            stats["bytes"] += path.stat().st_size

    return errors, stats


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Audit a staged LWSMF release directory")
    parser.add_argument("--dist", default="dist/Lwsmf-win64", help="release directory to audit")
    args = parser.parse_args(argv)

    dist = Path(args.dist).resolve()
    errors, stats = audit(dist)
    if errors:
        print("RELEASE_AUDIT_FAIL")
        for error in errors:
            print(f"- {error}")
        return 1

    print(
        "RELEASE_AUDIT_OK "
        f"files={stats['files']} bytes={stats['bytes']} "
        f"assets={stats['assets']} asset_bytes={stats['asset_bytes']} mps={stats['mps']}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
