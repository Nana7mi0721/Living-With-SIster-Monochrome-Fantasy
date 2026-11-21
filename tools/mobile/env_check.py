#!/usr/bin/env python3
"""Preflight environment checker for LWSMF Android and iOS packaging.

Usage:
  python tools/mobile/env_check.py --target all
  python tools/mobile/env_check.py --target android --setup-debug-keystore
  python tools/mobile/env_check.py --target android --strict
"""
from __future__ import annotations

import argparse
import os
import platform
import re
import shutil
import subprocess
import sys
from pathlib import Path

REQUIRED_MONO_TEMPLATES = {
    "android": ("android_debug.apk", "android_release.apk", "android_source.zip"),
    "ios": ("ios.zip",),
}
REQUIRED_GAME_DATA = ("manifest.json", "programs.json", "database.json", "map_tiles.json")
REQUIRED_PROJECTS = ("CDataBase.project", "DataBase.project", "SysDatabase.project")


def find_repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def find_godot_bin() -> str | None:
    for var in ("GODOT_BIN", "GODOT"):
        val = os.environ.get(var, "").strip()
        if val and Path(val).is_file():
            return val
    for name in (
        "godot",
        "Godot",
        "Godot_v4.7.2-stable_mono_win64_console.exe",
        "Godot_v4.7.2-stable_mono_win64.exe",
    ):
        found = shutil.which(name)
        if found:
            return found
    return None


def find_dotnet_bin() -> str | None:
    for var in ("DOTNET_BIN", "DOTNET"):
        val = os.environ.get(var, "").strip()
        if val and Path(val).is_file():
            return val
    root = os.environ.get("DOTNET_ROOT", "").strip()
    if root:
        for exe in ("dotnet.exe", "dotnet"):
            cand = Path(root) / exe
            if cand.is_file():
                return str(cand)
    return shutil.which("dotnet")


def godot_templates_dir() -> Path | None:
    override = os.environ.get("GODOT_TEMPLATES_DIR", "").strip()
    if override and Path(override).is_dir():
        return Path(override)
    system = platform.system()
    candidates: list[Path] = []
    if system == "Windows":
        appdata = os.environ.get("APPDATA", "")
        if appdata:
            candidates.append(Path(appdata) / "Godot/export_templates")
        candidates.append(Path.home() / "AppData/Roaming/Godot/export_templates")
    elif system == "Darwin":
        candidates.append(Path.home() / "Library/Application Support/Godot/export_templates")
    else:
        xdg = os.environ.get("XDG_DATA_HOME", str(Path.home() / ".local/share"))
        candidates.append(Path(xdg) / "godot/export_templates")

    for base in candidates:
        mono = base / "4.7.2.stable.mono"
        if mono.is_dir():
            return mono
        stable = base / "4.7.2.stable"
        if stable.is_dir():
            return stable
    return None


def dotnet_workloads(dotnet_bin: str | None) -> tuple[set[str], str]:
    if not dotnet_bin:
        return set(), "dotnet not found"
    env = os.environ.copy()
    env.setdefault("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
    env.setdefault("DOTNET_NOLOGO", "1")
    if "DOTNET_ROOT" not in env:
        env["DOTNET_ROOT"] = str(Path(dotnet_bin).resolve().parent)
    try:
        proc = subprocess.run(
            [dotnet_bin, "workload", "list"],
            capture_output=True,
            text=True,
            timeout=15,
            env=env,
            check=False,
        )
    except Exception as exc:  # noqa: BLE001
        return set(), str(exc)
    out = proc.stdout + "\n" + proc.stderr
    found: set[str] = set()
    for line in out.splitlines():
        token = line.strip().split(" ")[0].lower() if line.strip() else ""
        if token in ("android", "ios", "maccatalyst", "maui-android", "maui-ios"):
            found.add(token)
    return found, ""


def find_android_sdk() -> str | None:
    for var in ("ANDROID_HOME", "ANDROID_SDK_ROOT"):
        val = os.environ.get(var, "").strip()
        if val and Path(val).is_dir():
            return val
    candidates = [
        Path.home() / "AppData/Local/Android/Sdk",
        Path.home() / "Library/Android/sdk",
        Path.home() / "Android/Sdk",
    ]
    for cand in candidates:
        if cand.is_dir():
            return str(cand)
    return None


def find_jdk_tool(name: str) -> str | None:
    java_home = os.environ.get("JAVA_HOME", "").strip()
    if java_home:
        for ext in ("", ".exe"):
            cand = Path(java_home) / "bin" / (name + ext)
            if cand.is_file():
                return str(cand)
    return shutil.which(name)


def parse_export_presets(cfg_path: Path) -> dict[str, dict[str, str]]:
    """Parse godot/export_presets.cfg sections keyed by preset name."""
    if not cfg_path.is_file():
        return {}
    sections: dict[str, dict[str, str]] = {}
    current = ""
    raw_sections: dict[str, dict[str, str]] = {}
    for line in cfg_path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            current = stripped[1:-1]
            raw_sections.setdefault(current, {})
            continue
        if current and "=" in stripped and not stripped.startswith(";"):
            k, v = stripped.split("=", 1)
            raw_sections[current][k.strip()] = v.strip().strip('"')
    for header, kv in raw_sections.items():
        if header.startswith("preset.") and not header.endswith(".options"):
            name = kv.get("name", header)
            opts = raw_sections.get(f"{header}.options", {})
            merged = dict(kv)
            merged.update(opts)
            sections[name] = merged
    return sections


def setup_debug_keystore(repo_root: Path) -> tuple[bool, str]:
    ks_path = repo_root / "work/out/android/debug.keystore"
    if ks_path.is_file() and ks_path.stat().st_size > 0:
        return True, f"already present: {ks_path}"
    keytool = find_jdk_tool("keytool")
    if not keytool:
        return False, "keytool not found (install JDK 17+ / Android Studio JDK)"
    ks_path.parent.mkdir(parents=True, exist_ok=True)
    proc = subprocess.run(
        [
            keytool,
            "-genkeypair",
            "-v",
            "-keystore",
            str(ks_path),
            "-storepass",
            "android",
            "-alias",
            "androiddebugkey",
            "-keypass",
            "android",
            "-keyalg",
            "RSA",
            "-keysize",
            "2048",
            "-validity",
            "10000",
            "-dname",
            "CN=Android Debug,O=Android,C=US",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0 or not ks_path.is_file():
        return False, f"keytool failed: {(proc.stderr or proc.stdout).strip()}"
    return True, f"generated: {ks_path}"


def check_environment(
    repo_root: Path,
    target: str = "all",
    do_setup_keystore: bool = False,
) -> tuple[list[tuple[str, str, str]], int]:
    """Return list of (status, label, detail) and count of missing required items."""
    rows: list[tuple[str, str, str]] = []
    blockers = 0
    host_os = platform.system()

    def add(status: str, label: str, detail: str, blocker: bool = False) -> None:
        nonlocal blockers
        rows.append((status, label, detail))
        if blocker and status == "MISSING":
            blockers += 1

    # 1. Core project & presets
    presets = parse_export_presets(repo_root / "godot/export_presets.cfg")
    win_preset = presets.get("Windows Desktop", {})
    if "mobile_data/*" in win_preset.get("exclude_filter", ""):
        add("OK", "preset:Windows Desktop", "excludes mobile_data/*")
    else:
        add("MISSING", "preset:Windows Desktop", "missing mobile_data/* in exclude_filter", blocker=True)

    if target in ("all", "android"):
        if "Android" in presets and presets["Android"].get("platform") == "Android":
            add("OK", "preset:Android", f"arm64-v8a={presets['Android'].get('architectures/arm64-v8a')}")
        else:
            add("MISSING", "preset:Android", "missing [preset.1] Android in godot/export_presets.cfg", blocker=True)

    if target in ("all", "ios"):
        if "iOS" in presets and presets["iOS"].get("platform") == "iOS":
            add("OK", "preset:iOS", f"export_project_only={presets['iOS'].get('application/export_project_only')}")
        else:
            add("MISSING", "preset:iOS", "missing [preset.2] iOS in godot/export_presets.cfg", blocker=True)

    # 2. Godot & templates
    godot_bin = find_godot_bin()
    add("OK" if godot_bin else "WARN", "godot", godot_bin or "not on PATH (set GODOT_BIN=...)")

    tpl_dir = godot_templates_dir()
    if tpl_dir:
        is_mono = "mono" in tpl_dir.name
        add("OK" if is_mono else "WARN", "templates_dir", str(tpl_dir))
        if target in ("all", "android"):
            missing_a = [f for f in REQUIRED_MONO_TEMPLATES["android"] if not (tpl_dir / f).is_file()]
            if not missing_a:
                add("OK", "templates:android", ", ".join(REQUIRED_MONO_TEMPLATES["android"]))
            else:
                add("MISSING", "templates:android", "missing: " + ", ".join(missing_a), blocker=True)
        if target in ("all", "ios"):
            missing_i = [f for f in REQUIRED_MONO_TEMPLATES["ios"] if not (tpl_dir / f).is_file()]
            if not missing_i:
                add("OK", "templates:ios", "ios.zip present")
            else:
                add("MISSING", "templates:ios", "missing: " + ", ".join(missing_i), blocker=True)
    else:
        add("MISSING", "templates_dir", "4.7.2.stable.mono export templates not found", blocker=True)

    # 3. .NET 8 SDK & workloads
    dotnet_bin = find_dotnet_bin()
    if dotnet_bin:
        add("OK", "dotnet", dotnet_bin)
        workloads, werr = dotnet_workloads(dotnet_bin)
        if werr:
            add("WARN", "dotnet_workloads", f"query failed: {werr}")
        if target in ("all", "android"):
            if "android" in workloads:
                add("OK", "workload:android", "installed")
            else:
                add("MISSING", "workload:android", "missing (run: dotnet workload install android)", blocker=True)
        if target in ("all", "ios"):
            if "ios" in workloads:
                add("OK", "workload:ios", "installed")
            else:
                # On Windows, iOS export is driven via macOS CI workflow; mark MISSING if targeting ios strictly.
                add("MISSING" if target == "ios" and host_os == "Darwin" else "INFO",
                    "workload:ios",
                    "not installed locally (macOS/CI installs via: dotnet workload install ios)",
                    blocker=(target == "ios" and host_os == "Darwin"))
    else:
        add("MISSING", "dotnet", "dotnet 8 SDK not found (set DOTNET_BIN/DOTNET_ROOT)", blocker=True)

    # 4. Android SDK / JDK
    if target in ("all", "android"):
        java_bin = find_jdk_tool("java")
        keytool_bin = find_jdk_tool("keytool")
        add("OK" if java_bin else "MISSING", "jdk:java", java_bin or "not found", blocker=True)
        add("OK" if keytool_bin else "MISSING", "jdk:keytool", keytool_bin or "not found", blocker=True)
        sdk = find_android_sdk()
        if sdk:
            add("OK", "android_sdk", sdk)
        else:
            add("MISSING", "android_sdk", "not found (set ANDROID_HOME or install Android SDK platform-tools/build-tools)", blocker=True)
        if do_setup_keystore:
            ok_ks, msg_ks = setup_debug_keystore(repo_root)
            add("OK" if ok_ks else "MISSING", "debug_keystore", msg_ks, blocker=not ok_ks)
        else:
            ks_path = repo_root / "work/out/android/debug.keystore"
            add("OK" if ks_path.is_file() else "INFO",
                "debug_keystore",
                str(ks_path) if ks_path.is_file() else "not generated yet (run with --setup-debug-keystore)")

    # 5. iOS Xcode / Injector
    if target in ("all", "ios"):
        if host_os == "Darwin":
            xcb = shutil.which("xcodebuild")
            add("OK" if xcb else "MISSING", "ios:xcodebuild", xcb or "Xcode command line tools missing", blocker=True)
        else:
            add("INFO", "ios:host_note", f"host is {host_os}; engine IPA builds on macOS/CI (.github/workflows/ios-unsigned.yml), inject data locally with tools/inject_ios_data.sh")

    # 6. Game payload readiness
    gd_ok = all((repo_root / "game_data" / f).is_file() for f in REQUIRED_GAME_DATA)
    pak_ok = (repo_root / "work/all/media.pak").is_file()
    proj_ok = all((repo_root / "work/all/BasicData" / f).is_file() for f in REQUIRED_PROJECTS)
    add("OK" if gd_ok else "WARN", "payload:game_data", "4/4 JSON present" if gd_ok else "game_data/*.json incomplete")
    add("OK" if pak_ok else "WARN", "payload:media.pak", "present" if pak_ok else "work/all/media.pak missing")
    add("OK" if proj_ok else "WARN", "payload:projects", "3/3 *.project present" if proj_ok else "BasicData/*.project incomplete")
    try:
        import PIL  # noqa: F401
        add("OK", "python:Pillow", "available for --slim WebP")
    except ImportError:
        add("WARN", "python:Pillow", "not installed (--slim WebP disabled)")

    return rows, blockers


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Check Android/iOS build & packaging environment for LWSMF")
    parser.add_argument("--target", choices=("all", "android", "ios"), default="all")
    parser.add_argument("--setup-debug-keystore", action="store_true", help="Generate work/out/android/debug.keystore if missing")
    parser.add_argument("--strict", action="store_true", help="Exit 1 if any target blocker is missing")
    args = parser.parse_args(argv)

    repo_root = find_repo_root()
    rows, blockers = check_environment(repo_root, target=args.target, do_setup_keystore=args.setup_debug_keystore)
    print(f"=== LWSMF MOBILE ENV CHECK (target={args.target}) ===")
    for status, label, detail in rows:
        print(f"  [{status:<7s}] {label:<22s} {detail}")
    print(f"MOBILE_ENV_CHECK blockers={blockers} strict={args.strict}")
    if args.strict and blockers > 0:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
