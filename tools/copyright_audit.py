#!/usr/bin/env python3
"""Copyright boundary audit for a staged release directory.

Scans dist/Lwsmf-win64 recursively for retail-format extensions and for files
whose bytes match the source/assets fingerprints recorded in game_data/manifest.json.
This script never writes into the release directory.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
from pathlib import Path

FORBIDDEN_EXTS = {
    ".dat", ".mps", ".wolf", ".sav",
    ".png", ".jpg", ".jpeg", ".webp", ".bmp",
    ".ogg", ".wav", ".mp3",
    ".ttf", ".otf", ".ttc",
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_fingerprints(manifest_path: Path) -> dict[int, list[tuple[str, str]]]:
    with manifest_path.open("r", encoding="utf-8") as handle:
        manifest = json.load(handle)
    entries: list[tuple[str, str, int]] = []
    source = manifest.get("source") or {}
    for item in source.get("files") or []:
        entries.append((str(item.get("path", "?")), str(item.get("sha256", "")), int(item.get("bytes", -1))))
    for item in manifest.get("assets") or []:
        label = str(item.get("logical") or item.get("source") or "?")
        entries.append((label, str(item.get("sha256", "")), int(item.get("bytes", -1))))

    by_size: dict[int, list[tuple[str, str]]] = {}
    for label, digest, size in entries:
        digest = digest.strip().lower()
        if size < 0 or len(digest) != 64:
            continue
        by_size.setdefault(size, []).append((digest, label))
    return by_size


def main(argv: list[str] | None = None) -> int:
    repo = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description="Audit a release directory against retail fingerprints.")
    parser.add_argument("--dist", default="dist/Lwsmf-win64", help="release directory (default: dist/Lwsmf-win64)")
    parser.add_argument("--manifest", default=None, help="manifest.json path (default: repo game_data/manifest.json)")
    args = parser.parse_args(argv)

    dist = Path(args.dist)
    if not dist.is_absolute():
        dist = repo / dist
    dist = dist.resolve()
    if not dist.is_dir():
        print(f"COPYRIGHT_AUDIT_ERROR: release directory not found: {dist}", file=sys.stderr)
        return 2

    manifest_path = Path(args.manifest) if args.manifest else repo / "game_data" / "manifest.json"
    if not manifest_path.is_absolute():
        manifest_path = repo / manifest_path
    manifest_path = manifest_path.resolve()
    if not manifest_path.is_file():
        print(f"COPYRIGHT_AUDIT_ERROR: manifest not found: {manifest_path}", file=sys.stderr)
        return 2
    try:
        by_size = load_fingerprints(manifest_path)
    except (OSError, ValueError, TypeError, json.JSONDecodeError) as exc:
        print(f"COPYRIGHT_AUDIT_ERROR: cannot read manifest {manifest_path}: {exc}", file=sys.stderr)
        return 2

    files = 0
    total_bytes = 0
    violations: list[str] = []
    for dirpath, dirnames, filenames in os.walk(dist):
        dirnames.sort()
        filenames.sort()
        for name in filenames:
            path = Path(dirpath) / name
            try:
                stat = path.stat()
            except OSError as exc:
                print(f"COPYRIGHT_AUDIT_ERROR: cannot stat {path}: {exc}", file=sys.stderr)
                return 2
            if not path.is_file():
                continue
            files += 1
            total_bytes += stat.st_size
            rel = path.relative_to(dist).as_posix()
            if path.suffix.lower() in FORBIDDEN_EXTS:
                violations.append(f"FORBIDDEN_EXT {rel} size={stat.st_size}")
                continue
            candidates = by_size.get(stat.st_size)
            if candidates:
                try:
                    digest = sha256_file(path)
                except OSError as exc:
                    print(f"COPYRIGHT_AUDIT_ERROR: cannot hash {path}: {exc}", file=sys.stderr)
                    return 2
                for want, label in candidates:
                    if digest == want:
                        violations.append(f"FORBIDDEN_HASH {rel} size={stat.st_size} matches={label}")
                        break

    if violations:
        print(f"COPYRIGHT_AUDIT_FAIL files={files} bytes={total_bytes} forbidden={len(violations)}")
        for violation in violations:
            print(violation)
        return 1

    print(f"COPYRIGHT_AUDIT_OK files={files} bytes={total_bytes} forbidden=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
