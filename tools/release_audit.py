#!/usr/bin/env python3
"""Release payload & repository hygiene audit for LWSMF.

Usage:
  python tools/release_audit.py --dist dist/Lwsmf-win64
  python tools/release_audit.py --repo-scan
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path, PurePosixPath

try:
    from pack_media import read_pak_entries, read_pak_index
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from pack_media import read_pak_entries, read_pak_index

REQUIRED_FILES = [
    "game/Lwsmf.exe",
    "game/Lwsmf.pck",
    "game/data_Lwsmf_windows_x86_64/Lwsmf.dll",
    "game_data/manifest.json",
    "game_data/programs.json",
    "game_data/database.json",
    "game_data/map_tiles.json",
    "work/all/media.pak",
    "work/all/BasicData/CDataBase.project",
    "work/all/BasicData/DataBase.project",
    "work/all/BasicData/SysDatabase.project",
    "converter/cli.py",
    "converter/convert.bat",
    "README.txt",
]

FORBIDDEN_STAGE_DIRS = [
    "work/all/save",
    "work/all/MapChip",
    "work/all/MapData",
    "work/all/SystemFile",
]

PRIVACY_PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    ("hardcoded E:/Study path", re.compile(r"E:[/\\]Study[/\\]", re.I)),
    ("hardcoded /e/Study path", re.compile(r"/e/Study/", re.I)),
    ("hardcoded SteamLibrary path", re.compile(r"[A-Z]:[/\\]SteamLibrary[/\\]", re.I)),
    ("hardcoded user home path", re.compile(r"(?:C:[/\\]Users[/\\]|/c/Users/)REISEN\b", re.I)),
    ("local username REISEN", re.compile(r"\bREISEN\b")),
    ("old repo name imoto life", re.compile(r"imoto[ _-]life", re.I)),
    ("personal email", re.compile(r"linxin070410@gmail\.com", re.I)),
]

PROBE_EXTENSIONS = ("", ".png", ".jpg", ".bmp", ".jpeg", ".webp", ".ogg", ".mp3", ".wav")
MEDIA_EXTS = {".png", ".jpg", ".jpeg", ".bmp", ".webp", ".ogg", ".mp3", ".wav", ".ico", ".ttf", ".otf"}
WOLF_CTRL_RE = re.compile(
    r"\\(?:cself|self|cdb|udb|sdb|sys|font|space|sp|ax|ay|mx|my|at|[vscfmi-])\[",
    re.I,
)
LEADING_REL_RE = re.compile(r"^(?:\.\./|\./)+")


def _nonempty_file(path: Path) -> bool:
    return path.is_file() and path.stat().st_size > 0


def _logical_asset_path(item: dict) -> str:
    value = item.get("logical") or item.get("path") or item.get("source") or ""
    return str(value).replace("\\", "/").lstrip("/")


def _scan_text_privacy(path: Path, rel_label: str, errors: list[str]) -> None:
    try:
        text = path.read_text(encoding="utf-8", errors="ignore")
    except OSError as exc:
        errors.append(f"unreadable text file {rel_label}: {exc}")
        return
    for lineno, line in enumerate(text.splitlines(), 1):
        for label, pat in PRIVACY_PATTERNS:
            if pat.search(line):
                errors.append(f"privacy/path leak ({label}) at {rel_label}:{lineno}")


def audit(
    dist: Path,
    strict: bool = False,
    missing_out: list[str] | None = None,
) -> tuple[list[str], dict[str, int]]:
    errors: list[str] = []
    stats = {"files": 0, "bytes": 0, "pak_entries": 0, "assets": 0}

    if not dist.is_dir():
        return [f"dist directory not found: {dist}"], stats

    for rel in REQUIRED_FILES:
        if not _nonempty_file(dist / rel):
            errors.append(f"missing or empty: {rel}")

    for rel in FORBIDDEN_STAGE_DIRS:
        if (dist / rel).exists():
            errors.append(f"forbidden directory must not be shipped: {rel}")

    for dat in (dist / "work/all/BasicData").glob("*.dat") if (dist / "work/all/BasicData").is_dir() else []:
        errors.append(f"redundant loose .dat in release: work/all/BasicData/{dat.name}")

    game_dir = dist / "game"
    if game_dir.is_dir():
        for pdb in game_dir.rglob("*.pdb"):
            errors.append(f"debug symbol file shipped: {pdb.relative_to(dist).as_posix()}")

    pck_path = dist / "game/Lwsmf.pck"
    if pck_path.is_file():
        pck_bytes = pck_path.read_bytes()
        for needle in (b"addons/godot_ai", b"res://tests/", b"res://tests_gd/"):
            if needle in pck_bytes:
                errors.append(f"Lwsmf.pck contains excluded dev content matching {needle.decode('utf-8')}")

    pak_paths: set[str] = set()
    pak_file = dist / "work/all/media.pak"
    if pak_file.is_file():
        try:
            entries = read_pak_entries(pak_file)
            stats["pak_entries"] = len(entries)
            pak_paths = {rel.replace("\\", "/").lower() for rel, _ in entries}
        except Exception as exc:  # noqa: BLE001 - report corrupted pack cleanly
            errors.append(f"invalid work/all/media.pak: {exc}")

    manifest_path = dist / "game_data/manifest.json"
    manifest: dict = {}
    if manifest_path.is_file():
        try:
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        except Exception as exc:  # noqa: BLE001
            errors.append(f"invalid game_data/manifest.json: {exc}")
            manifest = {}

    for rel_json in ("programs.json", "database.json", "map_tiles.json"):
        p = dist / "game_data" / rel_json
        if p.is_file():
            try:
                json.loads(p.read_text(encoding="utf-8"))
            except Exception as exc:  # noqa: BLE001
                errors.append(f"invalid game_data/{rel_json}: {exc}")

    for item in manifest.get("assets") or []:
        if not isinstance(item, dict):
            continue
        logical = _logical_asset_path(item)
        if not logical:
            continue
        low = logical.lower()
        if low.startswith("basicdata/"):
            if low.endswith(".project") and not _nonempty_file(dist / "work/all" / logical):
                errors.append(f"manifest project missing: work/all/{logical}")
            stats["assets"] += 1
            continue
        ext = Path(low).suffix
        if ext in MEDIA_EXTS or not ext:
            if not any((low + probe) in pak_paths for probe in PROBE_EXTENSIONS):
                errors.append(f"manifest asset missing in media.pak: {logical}")
                continue
        stats["assets"] += 1

    readme = dist / "README.txt"
    if readme.is_file():
        _scan_text_privacy(readme, "README.txt", errors)
    conv_dir = dist / "converter"
    if conv_dir.is_dir():
        for p in conv_dir.rglob("*"):
            if p.is_file() and p.suffix.lower() in (".py", ".bat", ".md", ".txt"):
                _scan_text_privacy(p, p.relative_to(dist).as_posix(), errors)

    for path in dist.rglob("*"):
        if path.is_file():
            stats["files"] += 1
            stats["bytes"] += path.stat().st_size

    prog_path = dist / "game_data/programs.json"
    if prog_path.is_file() and pak_paths:
        total_refs, miss = audit_program_assets(prog_path, pak_paths)
        stats["program_asset_total"] = total_refs
        stats["program_asset_missing"] = len(miss)
        if missing_out is not None:
            missing_out.extend(miss)
        if strict and miss:
            errors.append(
                f"program assets missing in media.pak ({len(miss)}/{total_refs}): "
                + ", ".join(miss[:10])
            )

    return errors, stats


def audit_program_assets(programs_json: Path, pak_paths: set[str]) -> tuple[int, list[str]]:
    """Cross-check static picture/audio asset references in programs.json against media.pak."""
    data = json.loads(programs_json.read_text(encoding="utf-8"))
    basenames = {PurePosixPath(p).name for p in pak_paths}
    refs: set[str] = set()

    def walk(node: object) -> None:
        if isinstance(node, dict):
            if node.get("op") in (140, 150):
                for raw in node.get("strings") or []:
                    if not isinstance(raw, str):
                        continue
                    norm = raw.strip().replace("\\", "/")
                    low = LEADING_REL_RE.sub("", norm.lower())
                    if (
                        not norm
                        or low.startswith("<")
                        or WOLF_CTRL_RE.search(raw)
                        or "%" in norm
                        or "/" not in norm
                    ):
                        continue
                    if low.startswith("save/"):
                        continue
                    if PurePosixPath(low).suffix in MEDIA_EXTS:
                        refs.add(norm)
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for x in node:
                walk(x)

    walk(data)
    missing: list[str] = []
    for ref in sorted(refs):
        low = LEADING_REL_RE.sub("", ref.lower())
        if low.startswith("save/"):
            continue
        if low in pak_paths:
            continue
        stem = PurePosixPath(low).with_suffix("").as_posix()
        if any((stem + ext) in pak_paths for ext in PROBE_EXTENSIONS if ext):
            continue
        if PurePosixPath(low).name in basenames:
            continue
        missing.append(ref)
    return len(refs), missing


def repo_scan(repo_root: Path) -> list[str]:
    """Scan tracked production, build, tool, and core godot doc files for leaks."""
    errors: list[str] = []
    proc = subprocess.run(
        ["git", "-C", str(repo_root), "ls-files"],
        capture_output=True,
        text=True,
        check=True,
    )
    include_prefixes = (
        "AGENTS.md",
        "README",
        ".github/",
        "godot/core_cs/",
        "godot/presentation/",
        "godot/presentation_gd/",
        "godot/scenes/",
        "godot/export_presets.cfg",
        "godot/project.godot",
        "godot/Lwsmf.csproj",
        "godot/README.md",
        "tools/",
        "docs/godot/",
    )
    skip_exact = {
        "tools/release_audit.py",
        "docs/godot/PRE_RELEASE_ORCHESTRATOR_AUDIT.md",
        "docs/godot/audit/GEMINI_AUDIT_AND_PLAN_2026-11-14.md",
    }
    text_exts = {".cs", ".gd", ".py", ".sh", ".ps1", ".cfg", ".godot", ".csproj", ".yml", ".yaml", ".md", ".txt", ".bat", ".tscn"}
    for line in proc.stdout.splitlines():
        rel = line.strip()
        if not rel or rel in skip_exact:
            continue
        if not rel.startswith(include_prefixes):
            continue
        p = repo_root / rel
        if not p.is_file() or p.suffix.lower() not in text_exts:
            continue
        _scan_text_privacy(p, rel, errors)
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Audit LWSMF release staging or repo privacy hygiene")
    parser.add_argument("--dist", default="dist/Lwsmf-win64", help="release directory to audit")
    parser.add_argument("--repo-scan", action="store_true", help="scan tracked production/tool/core-doc files for leaks")
    parser.add_argument("--asset-audit", action="store_true", help="cross-check game_data/programs.json against work/all/media.pak")
    parser.add_argument("--strict", action="store_true", help="fail non-zero if --asset-audit finds any unreferenced/stub misses")
    parser.add_argument("--hd-root", default="", help="verify a 2x art root (hd_manifest.json) against work/retail_dat")
    args = parser.parse_args(argv)

    if args.hd_root:
        tool = Path(__file__).resolve().parent / "verify_hd_manifest.py"
        rc = subprocess.call([sys.executable, str(tool), "--hd", args.hd_root])
        if rc != 0:
            print("HD_AUDIT_FAIL")
            return 1
        print("HD_AUDIT_OK")
        return 0

    if args.asset_audit:
        repo_root = Path(__file__).resolve().parent.parent
        pak_file = repo_root / "work/all/media.pak"
        prog_file = repo_root / "game_data/programs.json"
        if not pak_file.is_file() or not prog_file.is_file():
            print("ASSET_AUDIT_SKIPPED missing work/all/media.pak or game_data/programs.json")
            return 0
        pak_paths = read_pak_index(pak_file)
        total_refs, missing = audit_program_assets(prog_file, pak_paths)
        print(
            f"ASSET_AUDIT_REPORT total={total_refs} matched={total_refs - len(missing)} "
            f"missing={len(missing)} pak_entries={len(pak_paths)}"
        )
        for ref in missing[:20]:
            print(f"- {ref}")
        return 1 if (args.strict and missing) else 0

    if args.repo_scan:
        repo_root = Path(__file__).resolve().parent.parent
        errors = repo_scan(repo_root)
        if errors:
            print("REPO_PRIVACY_AUDIT_FAIL")
            for err in errors:
                print(f"- {err}")
            return 1
        print("REPO_PRIVACY_AUDIT_OK leaks=0")
        return 0

    dist = Path(args.dist).resolve()
    missing_prog_assets: list[str] = []
    errors, stats = audit(dist, strict=args.strict, missing_out=missing_prog_assets)
    if missing_prog_assets:
        print(
            f"PROGRAM_ASSET_MISSING count={len(missing_prog_assets)} "
            f"total={stats.get('program_asset_total', 0)}"
        )
        for ref in missing_prog_assets[:20]:
            print(f"- {ref}")
    if errors:
        print("RELEASE_AUDIT_FAIL")
        for error in errors:
            print(f"- {error}")
        return 1

    print(
        "RELEASE_AUDIT_OK "
        f"files={stats['files']} bytes={stats['bytes']} "
        f"pak_entries={stats['pak_entries']} assets={stats['assets']} "
        f"program_asset_missing={stats.get('program_asset_missing', 0)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
