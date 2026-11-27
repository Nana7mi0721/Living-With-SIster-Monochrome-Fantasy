#!/usr/bin/env python3
"""Interactive TUI & CLI release packager for LWSMF (Windows / Android / iOS).

Uses true-lossless compressed media (`--lossless`) as the standard release baseline
and intentionally omits lossy `--slim` presets.
"""
from __future__ import annotations

import argparse
import dataclasses
import hashlib
import io
import json
import os
import plistlib
import re
import shlex
import shutil
import subprocess
import sys
import time
import zipfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

TOOLS_DIR = Path(__file__).resolve().parent
REPO_ROOT = TOOLS_DIR.parent
MOBILE_DIR = TOOLS_DIR / "mobile"
for _p in (str(TOOLS_DIR), str(MOBILE_DIR)):
    if _p not in sys.path:
        sys.path.insert(0, _p)

import env_check  # noqa: E402
import github_ios_ci  # noqa: E402
import pack_media  # noqa: E402

try:
    from rich import box
    from rich.console import Console
    from rich.panel import Panel
    from rich.rule import Rule
    from rich.table import Table
    from rich.text import Text

    HAS_RICH = True
except ImportError:  # pragma: no cover - graceful fallback without rich
    HAS_RICH = False
    Console = None  # type: ignore[assignment]

CONFIG_PATH = REPO_ROOT / "work/out/release_tui_config.json"
PUBLISH_FILES = (
    "icon_mobile.jpg",
    "icon_pc.png",
    "icon_pc_dock.png",
    "icon_pc_corner.png",
    "setup_welcome.png",
    "setup_finish.png",
)
VALID_MEDIA_MODES = ("lossless", "copy")
VALID_ANDROID_MODES = ("debug", "release")


@dataclass
class ReleaseConfig:
    version: str = "0.1.0"
    media_mode: str = "lossless"
    brotli: bool = True
    win_installer: bool = True
    include_hd: bool = False
    use_custom_templates: bool = True
    android_mode: str = "debug"
    android_keystore: str = ""
    android_ks_user: str = "androiddebugkey"
    android_ks_pass: str = "android"
    android_key_pass: str = ""
    ios_engine_ipa: str = ""
    allow_placeholder_icons: bool = False

    def normalized(self) -> "ReleaseConfig":
        ver = self.version.strip() or "0.1.0"
        mm = self.media_mode if self.media_mode in VALID_MEDIA_MODES else "lossless"
        am = self.android_mode if self.android_mode in VALID_ANDROID_MODES else "debug"
        return ReleaseConfig(
            version=ver,
            media_mode=mm,
            brotli=bool(self.brotli),
            win_installer=bool(self.win_installer),
            include_hd=bool(self.include_hd),
            use_custom_templates=bool(self.use_custom_templates),
            android_mode=am,
            android_keystore=self.android_keystore.strip(),
            android_ks_user=self.android_ks_user.strip() or "androiddebugkey",
            android_ks_pass=self.android_ks_pass,
            android_key_pass=self.android_key_pass,
            ios_engine_ipa=self.ios_engine_ipa.strip(),
            allow_placeholder_icons=bool(self.allow_placeholder_icons),
        )


def load_config(path: Path = CONFIG_PATH) -> ReleaseConfig:
    if not path.is_file():
        return ReleaseConfig()
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(raw, dict):
            return ReleaseConfig()
        valid_fields = {f.name for f in dataclasses.fields(ReleaseConfig)}
        filtered = {k: v for k, v in raw.items() if k in valid_fields}
        return ReleaseConfig(**filtered).normalized()
    except (OSError, ValueError, TypeError):
        return ReleaseConfig()


def save_config(cfg: ReleaseConfig, path: Path = CONFIG_PATH) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(dataclasses.asdict(cfg.normalized()), ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


def parse_local_env_sh(repo_root: Path) -> dict[str, str]:
    """Read untracked tools/local.env.sh exports (GODOT, DOTNET, etc.)."""
    env_file = repo_root / "tools/local.env.sh"
    if not env_file.is_file():
        return {}
    result: dict[str, str] = {}
    pat = re.compile(r'^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+?)\s*$')
    for line in env_file.read_text(encoding="utf-8", errors="replace").splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        m = pat.match(line)
        if not m:
            continue
        key, val = m.group(1), m.group(2).strip().strip('"').strip("'")
        result[key] = _msys_to_native_path(val)
    return result


def _msys_to_native_path(value: str) -> str:
    if os.name == "nt":
        m = re.match(r"^/([a-zA-Z])/(.*)$", value)
        if m:
            return f"{m.group(1).upper()}:/{m.group(2)}"
    return value


def to_bash_path(path: Path | str) -> str:
    raw = str(path).replace("\\", "/")
    if os.name == "nt":
        m = re.match(r"^([a-zA-Z]):/(.*)$", raw)
        if m:
            return f"/{m.group(1).lower()}/{m.group(2)}"
    return raw


def find_bash_bin() -> str | None:
    if os.name == "nt":
        for cand in (
            Path(os.environ.get("PROGRAMFILES", r"C:\Program Files")) / "Git/bin/bash.exe",
            Path(os.environ.get("PROGRAMFILES(X86)", r"C:\Program Files (x86)")) / "Git/bin/bash.exe",
            Path(os.environ.get("LOCALAPPDATA", "")) / "Programs/Git/bin/bash.exe",
        ):
            if cand.is_file():
                return str(cand)
        found = shutil.which("bash")
        if found and "system32" not in found.lower():
            return found
        return found
    return shutil.which("bash")


def find_inno_setup(repo_root: Path) -> tuple[Path | None, Path | None]:
    """Return (ISCC.exe, ChineseSimplified.isl) if installed or staged locally."""
    iscc: Path | None = None
    env_iscc = os.environ.get("ISCC_BIN", "").strip()
    if env_iscc and Path(env_iscc).is_file():
        iscc = Path(env_iscc)

    if iscc is None:
        inno_root = repo_root / "work/out/inno"
        if inno_root.is_dir():
            for cand in sorted(inno_root.glob("tool-*/ISCC.exe"), reverse=True):
                if cand.is_file():
                    iscc = cand
                    break

    if iscc is None:
        which_iscc = shutil.which("ISCC.exe") or shutil.which("iscc")
        if which_iscc:
            iscc = Path(which_iscc)

    if iscc is None and os.name == "nt":
        local_app = os.environ.get("LOCALAPPDATA", "")
        for cand in (
            Path(local_app) / "Programs/Inno Setup 6/ISCC.exe" if local_app else None,
            Path(local_app) / "InnoSetup6/ISCC.exe" if local_app else None,
            Path(r"C:\Program Files (x86)\Inno Setup 6\ISCC.exe"),
            Path(r"C:\Program Files\Inno Setup 6\ISCC.exe"),
            Path(r"C:\Program Files (x86)\Inno Setup 7\ISCC.exe"),
            Path(r"C:\Program Files\Inno Setup 7\ISCC.exe"),
        ):
            if cand and cand.is_file():
                iscc = cand
                break

    isl: Path | None = None
    isl_candidates: list[Path] = []
    env_isl = os.environ.get("LWSMF_INNO_CHINESE_ISL", "").strip()
    if env_isl:
        isl_candidates.append(Path(env_isl))
    if iscc is not None:
        isl_candidates.extend([
            iscc.parent / "Languages/ChineseSimplified.isl",
            iscc.parent / "ChineseSimplified.isl",
        ])
    isl_candidates.append(repo_root / "work/out/inno/ChineseSimplified.isl")
    for cand in isl_candidates:
        if cand.is_file():
            isl = cand
            break
    return iscc, isl


def find_custom_templates(repo_root: Path) -> dict[str, Path | None]:
    win_candidates = [
        repo_root / "work/out/templates/windows/windows_release_x86_64.exe",
        repo_root / "dist/templates/windows/windows_release_x86_64.exe",
    ]
    android_candidates = [
        repo_root / "work/out/templates/android/android_release.apk",
        repo_root / "work/out/android-mono-template/android_monoRelease.apk",
        repo_root / "dist/templates/android/android_release.apk",
    ]
    ios_candidates = [
        repo_root / "work/out/templates/ios/ios.zip",
        repo_root / "dist/templates/ios/ios.zip",
    ]

    def first_existing(paths: list[Path]) -> Path | None:
        for p in paths:
            if p.is_file() and p.stat().st_size > 0:
                return p
        return None

    return {
        "windows": first_existing(win_candidates),
        "android": first_existing(android_candidates),
        "ios": first_existing(ios_candidates),
    }


def find_dotnet_by_major(local_env: dict[str, str], min_major: int, exact_major: int | None = None) -> tuple[str | None, list[str]]:
    """Locate a dotnet binary matching the requested SDK major version."""
    candidates: list[str] = []
    for key in ("ANDROID_DOTNET", "DOTNET_BIN", "DOTNET"):
        val = os.environ.get(key, "").strip()
        if val:
            candidates.append(val)
    which_dn = shutil.which("dotnet")
    if which_dn:
        candidates.append(which_dn)
    if local_env.get("DOTNET"):
        candidates.append(local_env["DOTNET"])
        # Also probe sibling dotnet9/dotnet8 directories beside local.env.sh DOTNET if present.
        parent_parent = Path(local_env["DOTNET"]).resolve().parent.parent
        for sibling in ("dotnet9", "dotnet8", "dotnet10", "dotnet11"):
            for exe in ("dotnet.exe", "dotnet"):
                cand = parent_parent / sibling / exe
                if cand.is_file():
                    candidates.append(str(cand))

    seen: set[str] = set()
    best: tuple[str, list[str]] | None = None
    for raw in candidates:
        p = Path(raw)
        if not p.is_file() and os.name == "nt" and not raw.lower().endswith(".exe"):
            p = Path(raw + ".exe")
        if not p.is_file():
            continue
        key = str(p.resolve()).lower()
        if key in seen:
            continue
        seen.add(key)
        versions, _ = env_check.dotnet_sdk_versions(str(p))
        if not versions:
            continue
        majors = [int(v.split(".", 1)[0]) for v in versions]
        if exact_major is not None and exact_major in majors:
            return str(p), versions
        if any(m >= min_major for m in majors):
            if best is None:
                best = (str(p), versions)
    if best is not None:
        return best
    return None, []


def inspect_publish_dir(repo_root: Path) -> tuple[Path, list[str], list[str]]:
    publish_dir = Path(os.environ.get("LWSMF_PUBLISH_DIR", str(repo_root / "work/all/_publish")))
    present: list[str] = []
    missing: list[str] = []
    for name in PUBLISH_FILES:
        if (publish_dir / name).is_file():
            present.append(name)
        else:
            missing.append(name)
    return publish_dir, present, missing


def inspect_media_pak(pak_path: Path) -> dict[str, Any]:
    if not pak_path.is_file():
        return {"exists": False, "bytes": 0, "entries": 0, "error": "missing"}
    try:
        names = pack_media.read_pak_index(pak_path)
        return {
            "exists": True,
            "bytes": pak_path.stat().st_size,
            "entries": len(names),
            "has_mapchip": any(n.startswith("mapchip/") for n in names),
            "error": "",
        }
    except Exception as exc:  # noqa: BLE001
        return {"exists": True, "bytes": pak_path.stat().st_size, "entries": 0, "error": str(exc)}


def discover_ios_engine_ipas(repo_root: Path) -> list[Path]:
    found: list[Path] = []
    for base in (repo_root / "work/out", repo_root / "dist/ios"):
        if not base.is_dir():
            continue
        for ipa in sorted(base.rglob("*.ipa")):
            if ipa.is_file() and not ipa.name.endswith("-full.ipa"):
                found.append(ipa)
    return found


def inspect_git_info(repo_root: Path) -> dict[str, Any]:
    info: dict[str, Any] = {"branch": "?", "commit": "?", "dirty": 0}
    try:
        info["branch"] = subprocess.check_output(
            ["git", "-C", str(repo_root), "rev-parse", "--abbrev-ref", "HEAD"],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        info["commit"] = subprocess.check_output(
            ["git", "-C", str(repo_root), "rev-parse", "--short", "HEAD"],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        status = subprocess.check_output(
            ["git", "-C", str(repo_root), "status", "--porcelain"],
            text=True,
            stderr=subprocess.DEVNULL,
        )
        info["dirty"] = len([line for line in status.splitlines() if line.strip()])
    except Exception:  # noqa: BLE001
        pass
    return info


def collect_environment(repo_root: Path) -> dict[str, Any]:
    local_env = parse_local_env_sh(repo_root)
    godot = env_check.find_godot_bin()
    if not godot and local_env.get("GODOT") and Path(local_env["GODOT"]).is_file():
        godot = local_env["GODOT"]

    dotnet_win, dotnet_win_vers = find_dotnet_by_major(local_env, min_major=8, exact_major=8)
    dotnet_android, dotnet_android_vers = find_dotnet_by_major(local_env, min_major=9, exact_major=9)
    if dotnet_android is None:
        dotnet_android, dotnet_android_vers = find_dotnet_by_major(local_env, min_major=9)

    iscc, chinese_isl = find_inno_setup(repo_root)
    templates_dir = env_check.godot_templates_dir()
    custom_templates = find_custom_templates(repo_root)
    publish_dir, publish_present, publish_missing = inspect_publish_dir(repo_root)
    media_info = inspect_media_pak(repo_root / "work/all/media.pak")

    gd_ok = all((repo_root / "game_data" / f).is_file() for f in env_check.REQUIRED_GAME_DATA)
    proj_ok = all((repo_root / "work/all/BasicData" / f).is_file() for f in env_check.REQUIRED_PROJECTS)
    font_path = None
    for cand in (
        Path(os.environ.get("LWSMF_FONT_PATH", "")) if os.environ.get("LWSMF_FONT_PATH") else None,
        repo_root / "work/all/Lolita.ttf",
        repo_root / "work/all/Data/Lolita.ttf",
    ):
        if cand and cand.is_file():
            font_path = cand
            break

    hd_ready = (repo_root / "work/hd/hd_manifest.json").is_file()
    loose_media_ready = (repo_root / "work/all/Picture").is_dir() and (repo_root / "work/all/BGM").is_dir()

    try:
        import PIL  # noqa: F401

        pillow_ok = True
    except ImportError:
        pillow_ok = False

    ios_icon_path = publish_dir / "icon_mobile.jpg"
    gh_repo, gh_branch = github_ios_ci.detect_git_repo_and_branch(repo_root)
    gh_token_sources = [label for _, label in github_ios_ci.iter_github_token_candidates(repo_root)]

    return {
        "git": inspect_git_info(repo_root),
        "bash": find_bash_bin(),
        "godot": godot,
        "templates_dir": templates_dir,
        "dotnet_win": dotnet_win,
        "dotnet_win_vers": dotnet_win_vers,
        "dotnet_android": dotnet_android,
        "dotnet_android_vers": dotnet_android_vers,
        "java": env_check.find_jdk_tool("java"),
        "keytool": env_check.find_jdk_tool("keytool"),
        "android_sdk": env_check.find_android_sdk(),
        "debug_keystore": (repo_root / "work/out/android/debug.keystore").is_file(),
        "iscc": iscc,
        "chinese_isl": chinese_isl,
        "custom_templates": custom_templates,
        "publish_dir": publish_dir,
        "publish_present": publish_present,
        "publish_missing": publish_missing,
        "ios_icon_path": ios_icon_path if ios_icon_path.is_file() else None,
        "media_pak": media_info,
        "game_data_ok": gd_ok,
        "projects_ok": proj_ok,
        "font_path": font_path,
        "hd_ready": hd_ready,
        "loose_media_ready": loose_media_ready,
        "pillow_ok": pillow_ok,
        "ios_engine_ipas": discover_ios_engine_ipas(repo_root),
        "github_repo": gh_repo,
        "github_branch": gh_branch,
        "github_token_sources": gh_token_sources,
    }


def fmt_mb(num_bytes: int) -> str:
    return f"{num_bytes / (1024 * 1024):.2f} MiB"


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def build_rebuild_media_command(repo_root: Path, media_mode: str = "lossless") -> list[str]:
    cmd = [
        sys.executable,
        str(repo_root / "tools/pack_media.py"),
        "--root",
        str(repo_root / "work/all"),
        "--prune-game-data",
        str(repo_root / "game_data"),
        "--out",
        str(repo_root / "work/all/media.pak"),
    ]
    if media_mode == "lossless":
        cmd.append("--lossless")
    return cmd


def format_cmd(cmd: list[str]) -> str:
    return " ".join(shlex.quote(arg) if (" " in arg or "\t" in arg) else arg for arg in cmd)


def build_platform_plan(
    target: str,
    cfg: ReleaseConfig,
    env_info: dict[str, Any],
    repo_root: Path,
) -> tuple[list[str], dict[str, str], list[str]]:
    """Construct (command, extra_env, warnings) for windows / android / ios."""
    cfg = cfg.normalized()
    extra_env: dict[str, str] = {}
    warnings: list[str] = []
    bash = env_info.get("bash") or "bash"

    if env_info.get("godot"):
        godot_bash = to_bash_path(str(env_info["godot"]))
        extra_env["GODOT_BIN"] = godot_bash
        extra_env["GODOT"] = godot_bash

    if target == "windows":
        if env_info.get("dotnet_win"):
            dn = str(env_info["dotnet_win"])
            dn_bash = to_bash_path(dn)
            extra_env["DOTNET_BIN"] = dn_bash
            extra_env["DOTNET"] = dn
            extra_env["DOTNET_ROOT"] = str(Path(dn).resolve().parent)
        if env_info.get("iscc"):
            extra_env["ISCC_BIN"] = to_bash_path(env_info["iscc"])
        if env_info.get("chinese_isl"):
            extra_env["LWSMF_INNO_CHINESE_ISL"] = to_bash_path(env_info["chinese_isl"])

        args = [bash, "tools/package_release.sh", "--version", cfg.version]
        if cfg.media_mode == "lossless":
            args.append("--lossless")
        if cfg.brotli:
            args.append("--brotli")
        if cfg.include_hd:
            args.append("--hd")
        if cfg.win_installer:
            if env_info.get("iscc") and env_info.get("chinese_isl"):
                args.append("--installer")
            else:
                warnings.append("未检测到完整的 Inno Setup 或 ChineseSimplified.isl，本次仅生成便携 ZIP")
        win_tpl = env_info["custom_templates"].get("windows") if cfg.use_custom_templates else None
        if win_tpl:
            args.extend(["--template", to_bash_path(win_tpl)])
        elif not cfg.use_custom_templates:
            args.append("--official-template")
        else:
            warnings.append("未找到 Windows 定制 2D Mono 模板，将使用官方 Mono 模板")
        if cfg.allow_placeholder_icons:
            args.append("--allow-placeholder")
        return args, extra_env, warnings

    if target == "android":
        if env_info.get("dotnet_android"):
            dn = str(env_info["dotnet_android"])
            dn_bash = to_bash_path(dn)
            extra_env["DOTNET_BIN"] = dn_bash
            extra_env["ANDROID_DOTNET"] = dn_bash
            extra_env["ANDROID_DOTNET_ROOT"] = str(Path(dn).resolve().parent)

        args = [bash, "tools/package_android.sh", "--version", cfg.version, "--force"]
        if cfg.android_mode == "release":
            args.append("--release")
            if cfg.android_keystore:
                args.extend(["--keystore", to_bash_path(cfg.android_keystore)])
            args.extend(["--ks-user", cfg.android_ks_user, "--ks-pass", cfg.android_ks_pass])
            if cfg.android_key_pass:
                args.extend(["--key-pass", cfg.android_key_pass])
        else:
            args.append("--debug")

        if cfg.media_mode == "lossless":
            args.append("--lossless")
        if cfg.brotli:
            args.append("--brotli")
        and_tpl = env_info["custom_templates"].get("android") if cfg.use_custom_templates else None
        if and_tpl:
            args.extend(["--template", to_bash_path(and_tpl)])
        elif not cfg.use_custom_templates:
            args.append("--official-template")
        else:
            warnings.append("未找到 Android 定制 2D Mono 模板，将使用官方 Mono 模板")
        if cfg.allow_placeholder_icons:
            args.append("--allow-placeholder")
        return args, extra_env, warnings

    if target == "ios":
        ipa_path = Path(cfg.ios_engine_ipa) if cfg.ios_engine_ipa else None
        out_dir = repo_root / "dist/ios" / cfg.version
        out_ipa = out_dir / f"LWSMF-{cfg.version}-ios-arm64-full.ipa"
        args = [
            sys.executable,
            str(repo_root / "tools/mobile/ios_inject_data.py"),
            str(ipa_path) if ipa_path else "<engine-only-unsigned.ipa>",
            "--game-data",
            str(repo_root / "game_data"),
            "--work-all",
            str(repo_root / "work/all"),
            "--out",
            str(out_ipa),
            "--force",
        ]
        if cfg.media_mode == "lossless":
            args.append("--lossless")
        if env_info.get("ios_icon_path"):
            args.extend(["--icon", str(env_info["ios_icon_path"])])
        elif not cfg.allow_placeholder_icons:
            warnings.append("缺失 icon_mobile.jpg，iOS 将保留云端占位图标")
        if env_info.get("font_path"):
            args.extend(["--font", str(env_info["font_path"])])
        if cfg.brotli:
            args.append("--brotli")
        if ipa_path is None or not ipa_path.is_file():
            warnings.append("尚未指定 macOS CI 产出的无签名引擎壳 .ipa，可通过本小程序一键触发或拉取 ios-unsigned.yml")
        return args, extra_env, warnings

    raise ValueError(f"unsupported target: {target}")


def validate_target_readiness(
    target: str,
    cfg: ReleaseConfig,
    env_info: dict[str, Any],
) -> list[str]:
    blockers: list[str] = []
    if not env_info["game_data_ok"]:
        blockers.append("game_data/*.json 不完整")
    if not env_info["projects_ok"]:
        blockers.append("work/all/BasicData/*.project 不完整")
    if not env_info["media_pak"]["exists"] or env_info["media_pak"]["error"]:
        blockers.append("work/all/media.pak 缺失或损坏（可在菜单 [6] 一键按活体清单无损重建）")

    if target in ("windows", "android"):
        if not env_info["bash"]:
            blockers.append("未检测到 Git Bash / bash")
        if not env_info["godot"]:
            blockers.append("未检测到 Godot 4.7.2 Mono 控制台程序")
        if not env_info["templates_dir"]:
            blockers.append("未检测到 Godot 4.7.2.stable.mono 官方导出模板目录")
        if env_info["publish_missing"] and not cfg.allow_placeholder_icons:
            blockers.append(
                f"发布美术母版缺失 ({len(env_info['publish_missing'])}/{len(PUBLISH_FILES)}): "
                + ", ".join(env_info["publish_missing"])
            )

    if target == "windows":
        if not env_info["dotnet_win"]:
            blockers.append("未检测到 .NET 8+ SDK（Windows 导出必需）")
        if cfg.include_hd and not env_info["hd_ready"]:
            blockers.append("已开启 2× HD 选项但 work/hd/hd_manifest.json 不存在")
    elif target == "android":
        if not env_info["dotnet_android"]:
            blockers.append("未检测到 .NET 9+ SDK（Android 导出必需）")
        if not env_info["java"] or not env_info["keytool"]:
            blockers.append("未检测到 JDK (java/keytool)")
        if not env_info["android_sdk"]:
            blockers.append("未检测到 Android SDK (platform-tools & build-tools)")
        if cfg.android_mode == "release" and (not cfg.android_keystore or not Path(cfg.android_keystore).is_file()):
            blockers.append("Android Release 模式需要有效的 --keystore 文件路径")
    elif target == "ios":
        ipa_path = Path(cfg.ios_engine_ipa) if cfg.ios_engine_ipa else None
        if ipa_path is None or not ipa_path.is_file():
            blockers.append("未提供 macOS CI 生成的 iOS 无签名引擎壳 .ipa 文件（可用 --ios-ci-fetch / --ios-ci-trigger 自动获取）")
        if not env_info.get("ios_icon_path") and not cfg.allow_placeholder_icons:
            blockers.append("缺失 work/all/_publish/icon_mobile.jpg（iOS 正式应用图标必需）")
    return blockers


class UI:
    def __init__(self) -> None:
        self.console = Console() if HAS_RICH else None

    def clear(self) -> None:
        if self.console:
            self.console.clear()
        else:
            os.system("cls" if os.name == "nt" else "clear")

    def print(self, *args: Any, **kwargs: Any) -> None:
        if self.console:
            self.console.print(*args, **kwargs)
        else:
            print(*args)

    def render_dashboard(self, cfg: ReleaseConfig, env_info: dict[str, Any]) -> None:
        git_info = env_info["git"]
        dirty_tag = f" [yellow]({git_info['dirty']} 处未提交)[/yellow]" if git_info["dirty"] else " [green](干净)[/green]"
        title_str = (
            f"妹相随～黑白世界的缤纷冒险～ · 三端无损发包控制台  "
            f"[dim]branch={git_info['branch']}@{git_info['commit']}[/dim]{dirty_tag}"
        )

        if not self.console:
            print("=" * 72)
            print(f"LWSMF Release Packager | {git_info['branch']}@{git_info['commit']} | ver={cfg.version}")
            print("=" * 72)
            return

        self.console.print(Panel(title_str, style="bold cyan", box=box.ROUNDED))

        # Status grid (Toolchain + Assets)
        status_table = Table(box=box.SIMPLE_HEAVY, expand=True, show_header=True, header_style="bold white")
        status_table.add_column("检查项", style="cyan", width=22)
        status_table.add_column("状态", width=10)
        status_table.add_column("详情", overflow="fold")

        def badge(ok: bool, warn: bool = False) -> str:
            if ok:
                return "[bold green]OK[/bold green]"
            if warn:
                return "[bold yellow]WARN[/bold yellow]"
            return "[bold red]MISSING[/bold red]"

        status_table.add_row(
            "Godot 4.7.2 Mono",
            badge(bool(env_info["godot"])),
            str(env_info["godot"] or "未找到（配置 GODOT_BIN 或 tools/local.env.sh）"),
        )
        win_dn = env_info["dotnet_win"]
        and_dn = env_info["dotnet_android"]
        status_table.add_row(
            ".NET SDK (Win / And)",
            badge(bool(win_dn and and_dn), warn=bool(win_dn or and_dn)),
            f"Win: {', '.join(env_info['dotnet_win_vers']) or '缺失'}  |  Android: {', '.join(env_info['dotnet_android_vers']) or '缺失'}",
        )
        status_table.add_row(
            "Android SDK & JDK",
            badge(bool(env_info["android_sdk"] and env_info["java"])),
            f"SDK={env_info['android_sdk'] or '缺失'}  |  Keystore={'已生成' if env_info['debug_keystore'] else '首次导出自动生成'}",
        )
        inno_ok = bool(env_info["iscc"] and env_info["chinese_isl"])
        status_table.add_row(
            "Inno Setup (Setup)",
            badge(inno_ok, warn=bool(env_info["iscc"])),
            f"ISCC={env_info['iscc'] or '缺失'}  |  简体中文包={'就绪' if env_info['chinese_isl'] else '缺失'}",
        )

        win_tpl = env_info["custom_templates"].get("windows")
        and_tpl = env_info["custom_templates"].get("android")
        tpl_detail = (
            f"Win: {fmt_mb(win_tpl.stat().st_size) if win_tpl else '官方模板'}  |  "
            f"Android: {fmt_mb(and_tpl.stat().st_size) if and_tpl else '官方模板'}"
        )
        status_table.add_row(
            "定制 2D Mono 模板",
            badge(bool(win_tpl and and_tpl), warn=True),
            tpl_detail,
        )

        mp = env_info["media_pak"]
        mp_detail = (
            f"{fmt_mb(mp['bytes'])} · {mp['entries']} 条"
            + (" (无 MapChip · 已剪枝)" if not mp.get("has_mapchip") else " (含 MapChip)")
            if mp["exists"] and not mp["error"]
            else f"缺失或损坏: {mp['error']}"
        )
        status_table.add_row(
            "媒体包 work/all/media.pak",
            badge(mp["exists"] and not mp["error"]),
            mp_detail,
        )

        pub_miss = env_info["publish_missing"]
        pub_ok = len(pub_miss) == 0
        pub_detail = (
            f"{len(PUBLISH_FILES)}/{len(PUBLISH_FILES)} 完整 ({env_info['publish_dir']})"
            if pub_ok
            else f"缺少 {len(pub_miss)}/{len(PUBLISH_FILES)}: {', '.join(pub_miss)}  (目录: {env_info['publish_dir']})"
        )
        status_table.add_row("发行图标/海报 (_publish)", badge(pub_ok, warn=cfg.allow_placeholder_icons), pub_detail)

        ios_ipa = cfg.ios_engine_ipa or (str(env_info["ios_engine_ipas"][0]) if env_info["ios_engine_ipas"] else "")
        gh_sources = env_info.get("github_token_sources") or []
        gh_note = (
            f"CI 鉴权={' / '.join(gh_sources)}"
            if gh_sources
            else "CI 未鉴权（菜单 [4] 可配置 PAT）"
        )
        status_table.add_row(
            "iOS 引擎壳 (.ipa)",
            badge(bool(ios_ipa and Path(ios_ipa).is_file()), warn=True),
            f"{ios_ipa or '本地未下载（菜单 [4] 可一键触发/拉取 ios-unsigned.yml）'}  |  {gh_note}",
        )
        self.console.print(status_table)

        # Current Config Summary
        mode_label = "真无损 WebP 压缩（标准版，无损像素校验）" if cfg.media_mode == "lossless" else "原字节直通（不重编码）"
        cfg_lines = (
            f"[bold]版本:[/bold] [green]{cfg.version}[/green]   "
            f"[bold]媒体模式:[/bold] [cyan]{mode_label}[/cyan]   "
            f"[bold]Brotli JSON:[/bold] {'[green]开启[/green]' if cfg.brotli else '[yellow]关闭[/yellow]'}   "
            f"[bold]Windows 安装包:[/bold] {'[green]ZIP+Setup[/green]' if cfg.win_installer else '[yellow]仅 ZIP[/yellow]'}\n"
            f"[bold]定制模板:[/bold] {'[green]优先启用[/green]' if cfg.use_custom_templates else '[yellow]官方模板[/yellow]'}   "
            f"[bold]Android 签名:[/bold] [cyan]{cfg.android_mode.upper()}[/cyan]   "
            f"[bold]2× HD 层:[/bold] {'[green]附加[/green]' if cfg.include_hd else '[dim]关闭[/dim]'}   "
            f"[bold]缺失图标占位:[/bold] {'[yellow]允许[/yellow]' if cfg.allow_placeholder_icons else '[green]禁止（严格）[/green]'}"
        )
        self.console.print(Panel(cfg_lines, title="当前发包配置（已禁有损 slim 档）", border_style="blue", box=box.ROUNDED))

    def render_menu(self) -> None:
        items = [
            ("1", "一键三端标准发包（Windows + Android + iOS 数据注入）", "bold green"),
            ("2", "仅导出 Windows x64（便携 ZIP + 可选 Inno Setup 安装程序）", "cyan"),
            ("3", "仅导出 Android arm64 APK（自动剥离静态库 + 16KB 对齐 + v2/v3 签名）", "cyan"),
            ("4", "iOS arm64 打包（一键触发/拉取 GitHub Actions 引擎壳 + 真无损数据与正式图标注入）", "cyan"),
            ("5", "修改发包配置（版本号 / 无损或直通 / Brotli / Setup / 模板 / 签名 / 占位图标）", "yellow"),
            ("6", "从零按活体清单剪枝并重建真无损 work/all/media.pak", "magenta"),
            ("7", "环境与素材深度体检（env_check + 隐私扫描 + _publish 文件清单）", "white"),
            ("8", "校验与查看 dist/ 产物（PE/Setup 烟测 + APK 图标/16KB 验真 + SHA-256）", "white"),
            ("9", "修复工作区 NTFS 完整性级别（Medium IL）", "dim"),
            ("0", "退出", "dim"),
        ]
        if self.console:
            t = Table(box=box.SIMPLE, show_header=False, pad_edge=False)
            t.add_column("Key", style="bold", width=5)
            t.add_column("Action")
            for k, desc, style in items:
                t.add_row(f"[{k}]", f"[{style}]{desc}[/{style}]")
            self.console.print(t)
        else:
            for k, desc, _ in items:
                print(f"  [{k}] {desc}")


def run_streaming_command(cmd: list[str], extra_env: dict[str, str], cwd: Path, label: str, ui: UI) -> tuple[int, float]:
    merged_env = os.environ.copy()
    merged_env.update(extra_env)
    merged_env.setdefault("PYTHONUTF8", "1")
    ui.print(Rule(f"执行: {label}") if HAS_RICH else f"\n=== 执行: {label} ===")
    cmd_str = format_cmd(cmd)
    ui.print(f"[dim]$ {cmd_str}[/dim]" if HAS_RICH else f"$ {cmd_str}")
    t0 = time.monotonic()
    proc = subprocess.run(cmd, cwd=str(cwd), env=merged_env, check=False)
    elapsed = time.monotonic() - t0
    if proc.returncode == 0:
        ui.print(f"[bold green]✓ {label} 完成 ({elapsed:.1f}s)[/bold green]" if HAS_RICH else f"OK: {label} ({elapsed:.1f}s)")
    else:
        ui.print(
            f"[bold red]✗ {label} 失败 (exit={proc.returncode}, {elapsed:.1f}s)[/bold red]"
            if HAS_RICH
            else f"FAIL: {label} (exit={proc.returncode}, {elapsed:.1f}s)"
        )
    return proc.returncode, elapsed


def prompt_text(label: str, default: str = "") -> str:
    suffix = f" [{default}]" if default else ""
    try:
        ans = input(f"{label}{suffix}: ").strip()
    except EOFError:
        return default
    return ans if ans else default


def prompt_yes_no(label: str, default: bool = True) -> bool:
    hint = "Y/n" if default else "y/N"
    try:
        ans = input(f"{label} ({hint}): ").strip().lower()
    except EOFError:
        return default
    if not ans:
        return default
    return ans in ("y", "yes", "1", "true", "是")


def edit_config_interactive(cfg: ReleaseConfig, env_info: dict[str, Any], ui: UI) -> ReleaseConfig:
    while True:
        ui.print(Rule("配置发包参数") if HAS_RICH else "\n--- 配置发包参数 ---")
        options = [
            f"[1] 版本号                  : {cfg.version}",
            f"[2] 媒体压缩模式            : {'真无损 WebP（标准版）' if cfg.media_mode == 'lossless' else '原字节直通'}",
            f"[3] game_data Brotli 压缩   : {'开启' if cfg.brotli else '关闭'}",
            f"[4] Windows Inno Setup 安装 : {'开启（ZIP + Setup）' if cfg.win_installer else '关闭（仅便携 ZIP）'}",
            f"[5] 优先使用定制 2D 模板    : {'开启' if cfg.use_custom_templates else '关闭（官方模板）'}",
            f"[6] 附加 2× HD 媒体包       : {'开启' if cfg.include_hd else '关闭'}",
            f"[7] Android 构建签名模式    : {cfg.android_mode} ({cfg.android_keystore or '自动调试密钥'})",
            f"[8] iOS 引擎壳 IPA 路径     : {cfg.ios_engine_ipa or '未设置'}",
            f"[9] 缺失 _publish 允许占位  : {'允许（仅测试）' if cfg.allow_placeholder_icons else '禁止（正式发包推荐）'}",
            "[0] 保存并返回主菜单",
        ]
        for line in options:
            ui.print(line)

        choice = prompt_text("选择要修改的项", "0")
        if choice == "0":
            cfg = cfg.normalized()
            save_config(cfg)
            return cfg
        if choice == "1":
            cfg.version = prompt_text("输入语义化版本号（如 0.1.0 / 0.1.1）", cfg.version)
        elif choice == "2":
            cfg.media_mode = "copy" if cfg.media_mode == "lossless" else "lossless"
        elif choice == "3":
            cfg.brotli = not cfg.brotli
        elif choice == "4":
            cfg.win_installer = not cfg.win_installer
        elif choice == "5":
            cfg.use_custom_templates = not cfg.use_custom_templates
        elif choice == "6":
            cfg.include_hd = not cfg.include_hd
        elif choice == "7":
            cfg.android_mode = "release" if cfg.android_mode == "debug" else "debug"
            if cfg.android_mode == "release":
                cfg.android_keystore = prompt_text("Release Keystore 路径 (.jks/.keystore)", cfg.android_keystore)
                cfg.android_ks_user = prompt_text("Keystore Alias", cfg.android_ks_user)
                cfg.android_ks_pass = prompt_text("Keystore Password", cfg.android_ks_pass)
                cfg.android_key_pass = prompt_text("Key Password（留空同 Keystore Password）", cfg.android_key_pass)
        elif choice == "8":
            discovered = env_info.get("ios_engine_ipas") or []
            default_ipa = cfg.ios_engine_ipa or (str(discovered[0]) if discovered else "")
            cfg.ios_engine_ipa = prompt_text("输入无签名引擎 IPA 路径（清空可输入 -）", default_ipa)
            if cfg.ios_engine_ipa == "-":
                cfg.ios_engine_ipa = ""
        elif choice == "9":
            cfg.allow_placeholder_icons = not cfg.allow_placeholder_icons
        cfg = cfg.normalized()
        save_config(cfg)


def acquire_ios_ipa_via_ci(
    repo_root: Path,
    cfg: ReleaseConfig,
    ui: UI,
    *,
    mode: str,
    ref: str = "",
) -> Path | None:
    """Invoke tools/mobile/github_ios_ci.py to fetch or trigger+wait+download ios-unsigned.yml IPA."""
    cmd = [
        sys.executable,
        str(repo_root / "tools/mobile/github_ios_ci.py"),
        "--trigger" if mode == "trigger" else "--fetch-latest",
        "--version",
        cfg.version,
    ]
    if ref:
        cmd.extend(["--ref", ref])
    label = (
        f"GitHub Actions 触发并等待 ios-unsigned.yml ({cfg.version})"
        if mode == "trigger"
        else f"GitHub Actions 拉取最新 ios-unsigned.yml 引擎壳 ({cfg.version})"
    )
    ui.print(Rule(f"执行: {label}") if HAS_RICH else f"\n=== 执行: {label} ===")
    ui.print(f"[dim]$ {format_cmd(cmd)}[/dim]" if HAS_RICH else f"$ {format_cmd(cmd)}")
    env = os.environ.copy()
    env.setdefault("PYTHONUTF8", "1")
    t0 = time.monotonic()
    ipa_path: Path | None = None
    proc = subprocess.Popen(
        cmd,
        cwd=str(repo_root),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    assert proc.stdout is not None
    for line in proc.stdout:
        stripped = line.rstrip("\r\n")
        print(stripped, flush=True)
        m = re.match(r"^IOS_CI_IPA_OK\s+path=(.+?)\s+run_id=", stripped)
        if m:
            ipa_path = Path(m.group(1).strip())
    rc = proc.wait()
    elapsed = time.monotonic() - t0
    if rc == 0 and ipa_path is not None and ipa_path.is_file():
        ui.print(
            f"[bold green]✓ 已获取 iOS 引擎壳: {ipa_path} ({elapsed:.1f}s)[/bold green]"
            if HAS_RICH
            else f"OK: {ipa_path} ({elapsed:.1f}s)"
        )
        return ipa_path
    ui.print(
        f"[bold red]✗ 获取 iOS 引擎壳失败 (exit={rc}, {elapsed:.1f}s)[/bold red]"
        if HAS_RICH
        else f"FAIL: GitHub Actions iOS CI (exit={rc})"
    )
    return None


def resolve_ios_engine_ipa_interactive(
    cfg: ReleaseConfig,
    env_info: dict[str, Any],
    repo_root: Path,
    ui: UI,
    *,
    allow_skip: bool,
) -> str:
    """Interactive selector for obtaining or reusing the iOS engine-only IPA."""
    existing = cfg.ios_engine_ipa if (cfg.ios_engine_ipa and Path(cfg.ios_engine_ipa).is_file()) else ""
    if not existing and env_info["ios_engine_ipas"]:
        existing = str(env_info["ios_engine_ipas"][0])
    default_branch = env_info.get("github_branch") or "master"

    while True:
        ui.print(Rule("iOS 引擎壳 (.ipa) 来源选择") if HAS_RICH else "\n--- iOS 引擎壳来源选择 ---")
        ui.print(f"  [1] 从 GitHub Actions 拉取最近一次成功的 ios-unsigned.yml 引擎壳")
        ui.print(f"  [2] 立即触发 GitHub Actions ios-unsigned.yml（分支: {default_branch}，等待构建并下载）")
        ui.print(f"  [3] 使用本地已有引擎壳 .ipa（当前: {existing or '无'}）")
        ui.print(f"  [4] 配置/更新 GitHub Token（仅保存到忽略目录 work/out/github_token.txt）")
        ui.print(f"  [0] {'跳过 iOS，继续其余平台' if allow_skip else '取消返回主菜单'}")
        default_pick = "3" if existing else "1"
        pick = prompt_text("请选择 iOS 引擎壳获取方式", default_pick)

        if pick == "0":
            return ""
        if pick == "1":
            got = acquire_ios_ipa_via_ci(repo_root, cfg, ui, mode="fetch")
            if got:
                cfg.ios_engine_ipa = str(got)
                save_config(cfg)
                return str(got)
        elif pick == "2":
            ref = prompt_text("触发分支或 Tag", default_branch)
            got = acquire_ios_ipa_via_ci(repo_root, cfg, ui, mode="trigger", ref=ref)
            if got:
                cfg.ios_engine_ipa = str(got)
                save_config(cfg)
                return str(got)
        elif pick == "3":
            entered = prompt_text("输入本地引擎壳 .ipa 路径", existing)
            if entered and Path(entered).is_file():
                cfg.ios_engine_ipa = entered
                save_config(cfg)
                return entered
            ui.print(f"[yellow]文件不存在: {entered}[/yellow]" if HAS_RICH else f"File not found: {entered}")
        elif pick == "4":
            pat = prompt_text("粘贴 GitHub PAT（需 actions:read+write 与 contents:read，留空取消）", "")
            if pat:
                saved = github_ios_ci.save_local_github_token(repo_root, pat)
                ui.print(f"[green]✓ Token 已保存至 {saved}（已 .gitignore，不入库）[/green]" if HAS_RICH else f"Saved token to {saved}")
        else:
            ui.print(f"[yellow]无效选项: {pick}[/yellow]" if HAS_RICH else f"Invalid choice: {pick}")


def execute_targets(
    targets: list[str],
    cfg: ReleaseConfig,
    repo_root: Path,
    ui: UI,
    auto_yes: bool = False,
    *,
    prompt_ios_source: bool = False,
) -> int:
    env_info = collect_environment(repo_root)
    run_cfg = dataclasses.replace(cfg)

    # Auto-bind discovered iOS engine IPA if user didn't configure one explicitly.
    if "ios" in targets and not run_cfg.ios_engine_ipa and env_info["ios_engine_ipas"]:
        run_cfg.ios_engine_ipa = str(env_info["ios_engine_ipas"][0])

    # Check missing _publish interactively if not headless.
    if any(t in ("windows", "android") for t in targets) and env_info["publish_missing"] and not run_cfg.allow_placeholder_icons:
        ui.print(
            f"[bold yellow]检测到 {env_info['publish_dir']} 缺少 {len(env_info['publish_missing'])} 个发行素材：[/bold yellow]"
            if HAS_RICH
            else f"Missing {len(env_info['publish_missing'])} publish assets in {env_info['publish_dir']}:"
        )
        for name in env_info["publish_missing"]:
            ui.print(f"  - {name}")
        if not auto_yes:
            if prompt_yes_no("是否仅在本次构建中临时使用占位图标继续？（正式发布请选择 n 并先放入 _publish 素材）", default=False):
                run_cfg.allow_placeholder_icons = True
            else:
                ui.print("已取消本次构建。")
                return 1

    # Interactive GitHub Actions CI / local IPA selection for iOS.
    ios_missing = "ios" in targets and (not run_cfg.ios_engine_ipa or not Path(run_cfg.ios_engine_ipa).is_file())
    if "ios" in targets and not auto_yes and (ios_missing or prompt_ios_source):
        selected_ipa = resolve_ios_engine_ipa_interactive(
            run_cfg,
            env_info,
            repo_root,
            ui,
            allow_skip=len(targets) > 1,
        )
        if selected_ipa:
            run_cfg.ios_engine_ipa = selected_ipa
            cfg.ios_engine_ipa = selected_ipa
            save_config(cfg)
            env_info = collect_environment(repo_root)
        elif len(targets) > 1:
            ui.print("[yellow]已跳过 iOS，继续其余平台。[/yellow]" if HAS_RICH else "Skipping iOS.")
            targets = [t for t in targets if t != "ios"]
        else:
            return 0

    if not targets:
        return 1

    # Validate blockers and prepare plans.
    plans: list[tuple[str, list[str], dict[str, str]]] = []
    for t in targets:
        blockers = validate_target_readiness(t, run_cfg, env_info)
        if blockers:
            ui.print(f"[bold red]{t.upper()} 构建前检查未通过：[/bold red]" if HAS_RICH else f"{t.upper()} blocked:")
            for b in blockers:
                ui.print(f"  - {b}")
            return 2
        cmd, extra_env, warns = build_platform_plan(t, run_cfg, env_info, repo_root)
        for w in warns:
            ui.print(f"[yellow]WARN ({t}): {w}[/yellow]" if HAS_RICH else f"WARN ({t}): {w}")
        plans.append((t, cmd, extra_env))

    ui.print(Rule("构建执行计划") if HAS_RICH else "\n=== 构建执行计划 ===")
    for t, cmd, _ in plans:
        cmd_str = format_cmd(cmd)
        ui.print(f"  [bold cyan]{t.upper():8s}[/bold cyan] -> {cmd_str}" if HAS_RICH else f"  {t.upper():8s} -> {cmd_str}")

    if not auto_yes and not prompt_yes_no("确认开始执行以上发包任务？", default=True):
        ui.print("已取消。")
        return 0

    results: list[tuple[str, int, float]] = []
    for t, cmd, extra_env in plans:
        rc, dt = run_streaming_command(cmd, extra_env, repo_root, f"{t.upper()} 发包", ui)
        results.append((t, rc, dt))
        if rc != 0:
            break

    # Summary
    ui.print(Rule("发包结果汇总") if HAS_RICH else "\n=== 发包结果汇总 ===")
    all_ok = all(rc == 0 for _, rc, _ in results)
    for t, rc, dt in results:
        tag = "[bold green]成功[/bold green]" if rc == 0 else f"[bold red]失败 (exit={rc})[/bold red]"
        ui.print(f"  {t.upper():10s}: {tag if HAS_RICH else ('OK' if rc == 0 else f'FAIL({rc})')}  ({dt:.1f}s)")
    if all_ok:
        show_dist_artifacts(repo_root, ui)
    return 0 if all_ok else 1


def show_dist_artifacts(repo_root: Path, ui: UI) -> None:
    dist_dir = repo_root / "dist"
    if not dist_dir.is_dir():
        ui.print("[dim]dist/ 尚无已导出产物。[/dim]" if HAS_RICH else "dist/ is empty.")
        return

    artifacts = sorted(
        [
            p
            for p in dist_dir.rglob("*")
            if p.is_file() and p.suffix.lower() in (".zip", ".exe", ".apk", ".ipa", ".txt") and p.parent.name != "game"
        ],
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )
    # Keep top-level dist outputs + android/<ver> + ios/<ver> outputs.
    filtered = [p for p in artifacts if len(p.relative_to(dist_dir).parts) <= 3]
    if not filtered:
        ui.print("[dim]dist/ 尚无归档文件。[/dim]" if HAS_RICH else "No release archives in dist/.")
        return

    if HAS_RICH and ui.console:
        t = Table(title="dist/ 发布产物清单", box=box.SIMPLE_HEAVY, expand=True)
        t.add_column("文件", style="cyan")
        t.add_column("大小", justify="right", style="green")
        t.add_column("修改时间", style="dim")
        for p in filtered[:20]:
            rel = p.relative_to(dist_dir).as_posix()
            mtime = time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(p.stat().st_mtime))
            t.add_row(rel, fmt_mb(p.stat().st_size), mtime)
        ui.console.print(t)
    else:
        for p in filtered[:20]:
            print(f"  {p.relative_to(dist_dir).as_posix():45s} {fmt_mb(p.stat().st_size)}")


def verify_android_apk_icons(apk_path: Path) -> tuple[list[str], dict[str, Any]]:
    """Verify Android adaptive icon bounds/background and inspect template/Brotli metadata."""
    issues: list[str] = []
    meta: dict[str, Any] = {
        "libgodot_bytes": 0,
        "brotli_json": False,
        "fg_bbox": None,
        "bg_hex": "",
    }
    if not apk_path.is_file():
        return [f"APK not found: {apk_path}"], meta

    with zipfile.ZipFile(apk_path, "r") as zf:
        names = set(zf.namelist())
        if "lib/arm64-v8a/libgodot_android.so" in names:
            meta["libgodot_bytes"] = zf.getinfo("lib/arm64-v8a/libgodot_android.so").file_size
        meta["brotli_json"] = any(n.endswith(".json.br.blob") for n in names)

        fg_name = "res/mipmap-xxxhdpi-v4/icon_foreground.webp"
        bg_name = "res/mipmap-xxxhdpi-v4/icon_background.webp"
        if fg_name not in names or bg_name not in names:
            issues.append(f"missing xxxhdpi adaptive icon resources in APK ({fg_name} / {bg_name})")
            return issues, meta

        try:
            from PIL import Image
        except ImportError:
            issues.append("Pillow unavailable; skipped Android adaptive icon pixel check")
            return issues, meta

        with Image.open(io.BytesIO(zf.read(fg_name))) as fg:
            bbox = fg.convert("RGBA").getchannel("A").getbbox()
            meta["fg_bbox"] = bbox
            if bbox is None:
                issues.append("Android icon_foreground.webp is fully transparent")
            elif bbox[0] > 72 or bbox[1] > 72 or bbox[2] < 360 or bbox[3] < 360:
                issues.append(f"Android icon_foreground opaque bbox {bbox} does not cover safe visible zone")

        with Image.open(io.BytesIO(zf.read(bg_name))) as bg:
            r, g, b = bg.convert("RGB").getpixel((0, 0))
            meta["bg_hex"] = f"#{r:02X}{g:02X}{b:02X}"
            if min(r, g, b) < 220:
                issues.append(f"Android icon_background too dark: {meta['bg_hex']} (< 220)")

    return issues, meta


def run_windows_setup_smoke(setup_exe: Path, expected_pak: Path, repo_root: Path, ui: UI) -> int:
    """Silently install Setup into a temporary directory under work/out, verify files, and uninstall."""
    if os.name != "nt":
        ui.print("Windows Setup 静默烟测仅适用于 Windows。")
        return 0
    smoke_dir = repo_root / "work/out/setup_smoke_verify"
    if smoke_dir.exists():
        shutil.rmtree(smoke_dir, ignore_errors=True)
    smoke_dir.parent.mkdir(parents=True, exist_ok=True)
    try:
        rc, _ = run_streaming_command(
            [
                str(setup_exe),
                "/VERYSILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                "/CURRENTUSER",
                f"/DIR={smoke_dir}",
            ],
            {},
            repo_root,
            "Windows Setup 静默安装烟测",
            ui,
        )
        if rc != 0:
            return rc

        required = [
            smoke_dir / "game/Lwsmf.exe",
            smoke_dir / "按键说明.txt",
            smoke_dir / "work/all/media.pak",
        ]
        missing = [str(p.relative_to(smoke_dir)) for p in required if not p.is_file() or p.stat().st_size <= 0]
        if missing:
            ui.print(f"[bold red]✗ Setup 烟测缺失文件: {', '.join(missing)}[/bold red]" if HAS_RICH else f"FAIL missing: {missing}")
            return 1

        if expected_pak.is_file():
            got_sha = sha256_file(smoke_dir / "work/all/media.pak")
            exp_sha = sha256_file(expected_pak)
            if got_sha != exp_sha:
                ui.print(f"[bold red]✗ Setup 落盘 media.pak SHA-256 不一致: {got_sha} != {exp_sha}[/bold red]" if HAS_RICH else "FAIL media.pak sha mismatch")
                return 1
            ui.print(f"[green]✓ Setup 静默安装校验通过，media.pak SHA-256 一致 ({got_sha[:16]}...)[/green]" if HAS_RICH else f"OK setup install media.pak={got_sha}")

        unins = smoke_dir / "unins000.exe"
        if unins.is_file():
            rc_un, _ = run_streaming_command(
                [str(unins), "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"],
                {},
                repo_root,
                "Windows Setup 静默卸载烟测",
                ui,
            )
            if rc_un != 0:
                return rc_un
        return 0
    finally:
        if smoke_dir.exists():
            shutil.rmtree(smoke_dir, ignore_errors=True)


def verify_ios_ipa_icons(ipa_path: Path) -> tuple[list[str], dict[str, Any]]:
    """Verify injected loose AppIcon PNGs and Info.plist CFBundleIcons inside an iOS IPA."""
    issues: list[str] = []
    meta: dict[str, Any] = {
        "app_prefix": "",
        "icon_count": 0,
        "has_icon_name": False,
        "brotli_json": False,
    }
    if not ipa_path.is_file():
        return [f"IPA not found: {ipa_path}"], meta

    with zipfile.ZipFile(ipa_path, "r") as zf:
        names = zf.namelist()
        apps = sorted({n.split("/")[1] for n in names if n.startswith("Payload/") and len(n.split("/")) >= 2 and n.split("/")[1].endswith(".app")})
        if not apps:
            return ["IPA missing Payload/*.app bundle"], meta
        app_prefix = f"Payload/{apps[0]}"
        meta["app_prefix"] = app_prefix
        meta["brotli_json"] = f"{app_prefix}/game_data/programs.json.br" in names

        plist_arc = f"{app_prefix}/Info.plist"
        if plist_arc not in names:
            issues.append(f"IPA missing {plist_arc}")
        else:
            plist_data = plistlib.loads(zf.read(plist_arc))
            for key, req_base in (("CFBundleIcons", "AppIcon60x60"), ("CFBundleIcons~ipad", "AppIcon76x76")):
                primary = (plist_data.get(key) or {}).get("CFBundlePrimaryIcon") or {}
                if "CFBundleIconName" in primary:
                    meta["has_icon_name"] = True
                    issues.append(f"Info.plist {key} still sets CFBundleIconName (iOS would load placeholder from Assets.car)")
                if req_base not in (primary.get("CFBundleIconFiles") or []):
                    issues.append(f"Info.plist {key}.CFBundleIconFiles missing {req_base}")

        required_icons = {
            "AppIcon60x60@2x.png": (120, 120),
            "AppIcon60x60@3x.png": (180, 180),
            "AppIcon76x76@2x~ipad.png": (152, 152),
            "AppIcon1024x1024.png": (1024, 1024),
        }
        icon_entries = [n for n in names if n.startswith(f"{app_prefix}/AppIcon") and n.lower().endswith(".png")]
        meta["icon_count"] = len(icon_entries)
        try:
            from PIL import Image
        except ImportError:
            Image = None  # type: ignore[assignment]
        for fname, exp_size in required_icons.items():
            arc = f"{app_prefix}/{fname}"
            if arc not in names:
                issues.append(f"IPA missing formal icon PNG: {fname}")
                continue
            if Image is not None:
                with Image.open(io.BytesIO(zf.read(arc))) as im:
                    if im.size != exp_size:
                        issues.append(f"iOS icon {fname} size {im.size} != {exp_size}")
                    if im.mode != "RGB":
                        issues.append(f"iOS icon {fname} mode {im.mode} != RGB (Apple forbids alpha)")
    return issues, meta


def verify_dist_artifacts(repo_root: Path, cfg: ReleaseConfig, ui: UI, smoke_setup: bool = False) -> int:
    """Run unified post-build verification across existing dist/ artifacts."""
    cfg = cfg.normalized()
    rc_all = 0
    checked_any = False

    win_dir = repo_root / "dist" / f"LWSMF-{cfg.version}-windows-x64"
    win_setup = repo_root / "dist" / f"LWSMF-{cfg.version}-windows-x64-setup.exe"
    if win_dir.is_dir():
        checked_any = True
        rc1, _ = run_streaming_command(
            [sys.executable, str(repo_root / "tools/release_audit.py"), "--dist", str(win_dir)],
            {},
            repo_root,
            "Windows 发布包审计 (release_audit --dist)",
            ui,
        )
        if rc1 != 0:
            rc_all = 1
        win_exe = win_dir / "game/Lwsmf.exe"
        if win_exe.is_file():
            rc2, _ = run_streaming_command(
                [sys.executable, str(repo_root / "tools/mobile/windows_pe.py"), "--check", str(win_exe)],
                {},
                repo_root,
                "Windows PE 版本语言与校验和验真 (windows_pe --check)",
                ui,
            )
            if rc2 != 0:
                rc_all = 1
            tpl_tag = "定制 2D Mono 模板" if win_exe.stat().st_size < 70 * 1024 * 1024 else "官方 Mono 模板"
            ui.print(f"  Windows Lwsmf.exe: {fmt_mb(win_exe.stat().st_size)} ({tpl_tag})")
        if smoke_setup and win_setup.is_file():
            rc_s = run_windows_setup_smoke(win_setup, win_dir / "work/all/media.pak", repo_root, ui)
            if rc_s != 0:
                rc_all = 1

    apk_path = repo_root / "dist/android" / cfg.version / f"LWSMF-{cfg.version}-android-arm64.apk"
    if apk_path.is_file():
        checked_any = True
        rc3, _ = run_streaming_command(
            [sys.executable, str(repo_root / "tools/mobile/android_data.py"), "verify-apk", str(apk_path)],
            {},
            repo_root,
            "Android APK 结构与 16KB 验真 (verify-apk)",
            ui,
        )
        if rc3 != 0:
            rc_all = 1
        icon_issues, meta = verify_android_apk_icons(apk_path)
        so_tag = "定制 2D Mono 模板" if 0 < meta["libgodot_bytes"] < 60 * 1024 * 1024 else "官方 Mono 模板"
        ui.print(
            f"  Android APK 详情: libgodot_android.so={fmt_mb(meta['libgodot_bytes'])} ({so_tag}) | "
            f"Brotli={'开启' if meta['brotli_json'] else '未开启'} | "
            f"图标前景={meta['fg_bbox']} 背景={meta['bg_hex']}"
        )
        if icon_issues:
            rc_all = 1
            for issue in icon_issues:
                ui.print(f"[bold red]✗ Android 图标验真失败: {issue}[/bold red]" if HAS_RICH else f"FAIL: {issue}")
        else:
            ui.print("[bold green]✓ Android 自适应图标验真通过（无启动器黑边）[/bold green]" if HAS_RICH else "OK: Android adaptive icon verified")

    ios_ipa = repo_root / "dist/ios" / cfg.version / f"LWSMF-{cfg.version}-ios-arm64-full.ipa"
    if ios_ipa.is_file():
        checked_any = True
        with zipfile.ZipFile(ios_ipa, "r") as zf:
            bad = zf.testzip()
            names = zf.namelist()
            has_pak = any(n.endswith("/work/all/media.pak") for n in names)
        ios_icon_issues, ios_meta = verify_ios_ipa_icons(ios_ipa)
        if bad is not None or not has_pak:
            rc_all = 1
            ui.print(f"[bold red]✗ iOS IPA 校验失败: bad={bad} has_pak={has_pak}[/bold red]" if HAS_RICH else f"FAIL iOS IPA: bad={bad}")
        else:
            ui.print(
                f"  iOS IPA 详情: {fmt_mb(ios_ipa.stat().st_size)} | "
                f"Brotli={'开启' if ios_meta['brotli_json'] else '未开启'} | "
                f"正式 AppIcon PNG={ios_meta['icon_count']} 个"
            )
            if ios_icon_issues:
                rc_all = 1
                for issue in ios_icon_issues:
                    ui.print(f"[bold red]✗ iOS 图标验真失败: {issue}[/bold red]" if HAS_RICH else f"FAIL iOS icon: {issue}")
            else:
                ui.print("[bold green]✓ iOS IPA ZIP CRC、媒体包与正式 AppIcon 验真通过[/bold green]" if HAS_RICH else "OK: iOS IPA verified")

    if not checked_any:
        ui.print("[yellow]未在 dist/ 中找到当前版本号的导出产物。[/yellow]" if HAS_RICH else "No dist artifacts found for current version.")
    return rc_all


def run_deep_diagnostics(repo_root: Path, ui: UI) -> int:
    env_info = collect_environment(repo_root)
    extra_env: dict[str, str] = {}
    if env_info.get("godot"):
        extra_env["GODOT_BIN"] = str(env_info["godot"])
    if env_info.get("dotnet_android"):
        extra_env["DOTNET_BIN"] = str(env_info["dotnet_android"])

    rc1, _ = run_streaming_command(
        [sys.executable, str(repo_root / "tools/mobile/env_check.py"), "--target", "all"],
        extra_env,
        repo_root,
        "移动端与工具链体检 (env_check)",
        ui,
    )
    rc2, _ = run_streaming_command(
        [sys.executable, str(repo_root / "tools/release_audit.py"), "--repo-scan"],
        {},
        repo_root,
        "仓库隐私与路径审计 (--repo-scan)",
        ui,
    )

    pub_dir, present, missing = inspect_publish_dir(repo_root)
    ui.print(f"\n[bold]_publish 素材目录:[/bold] {pub_dir}" if HAS_RICH else f"\n_publish dir: {pub_dir}")
    for name in PUBLISH_FILES:
        mark = "[green]✓[/green]" if name in present else "[red]✗ 缺失[/red]"
        ui.print(f"  {mark if HAS_RICH else ('OK' if name in present else 'MISSING'):10s} {name}")
    return 0 if (rc1 == 0 and rc2 == 0) else 1


def interactive_loop(repo_root: Path) -> int:
    ui = UI()
    cfg = load_config()
    while True:
        ui.clear()
        env_info = collect_environment(repo_root)
        ui.render_dashboard(cfg, env_info)
        ui.render_menu()
        choice = prompt_text("请输入操作编号", "1")
        if choice == "0":
            ui.print("再见！")
            return 0
        elif choice == "1":
            execute_targets(["windows", "android", "ios"], cfg, repo_root, ui)
        elif choice == "2":
            execute_targets(["windows"], cfg, repo_root, ui)
        elif choice == "3":
            execute_targets(["android"], cfg, repo_root, ui)
        elif choice == "4":
            execute_targets(["ios"], cfg, repo_root, ui, prompt_ios_source=True)
        elif choice == "5":
            cfg = edit_config_interactive(cfg, env_info, ui)
            continue
        elif choice == "6":
            if not env_info["loose_media_ready"]:
                ui.print("[red]错误: work/all/ 缺少解包素材目录，无法从零重建 media.pak。[/red]" if HAS_RICH else "ERROR: loose work/all missing.")
            elif prompt_yes_no("确认按 game_data 活体清单剪枝死资产并重建真无损 work/all/media.pak？", default=True):
                cmd = build_rebuild_media_command(repo_root, cfg.media_mode)
                run_streaming_command(cmd, {}, repo_root, "重建剪枝真无损 media.pak", ui)
        elif choice == "7":
            run_deep_diagnostics(repo_root, ui)
        elif choice == "8":
            show_dist_artifacts(repo_root, ui)
            if prompt_yes_no("是否对当前版本 dist/ 产物执行统一验真（release_audit + PE + APK 图标）？", default=True):
                do_smoke = os.name == "nt" and prompt_yes_no("是否同时执行 Windows Setup 静默安装/卸载烟测？", default=False)
                verify_dist_artifacts(repo_root, cfg, ui, smoke_setup=do_smoke)
            if os.name == "nt" and (repo_root / "dist").is_dir():
                if prompt_yes_no("是否在资源管理器打开 dist/ 目录？", default=False):
                    os.startfile(str(repo_root / "dist"))  # type: ignore[attr-defined]
        elif choice == "9":
            if os.name == "nt":
                run_streaming_command(
                    ["powershell", "-ExecutionPolicy", "Bypass", "-File", str(repo_root / "tools/fix_integrity_level.ps1")],
                    {},
                    repo_root,
                    "修复工作区 NTFS Medium Integrity Level",
                    ui,
                )
            else:
                ui.print("NTFS 完整性修复仅适用于 Windows。")
        else:
            ui.print(f"[yellow]无效选项: {choice}[/yellow]" if HAS_RICH else f"Invalid choice: {choice}")

        try:
            input("\n按回车键返回主菜单...")
        except EOFError:
            return 0


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="LWSMF 交互式三端无损发包控制台 (TUI / CLI)")
    parser.add_argument("--check", action="store_true", help="打印环境与素材状态仪表盘后退出")
    parser.add_argument("--plan", action="store_true", help="仅打印解析后的构建命令，不实际执行")
    parser.add_argument("--build", default="", help="非交互批量构建目标，逗号分隔：windows,android,ios")
    parser.add_argument("--verify", action="store_true", help="对当前版本 dist/ 产物执行统一验真（PE + audit + APK 图标/16KB）")
    parser.add_argument("--smoke-setup", action="store_true", help="与 --verify 或 --build 联用：执行 Windows Setup 静默装卸烟测")
    parser.add_argument("--rebuild-media", action="store_true", help="按活体清单剪枝并重建真无损 work/all/media.pak")
    parser.add_argument("--version", default="", help="覆盖版本号（如 0.1.0）")
    parser.add_argument("--media-mode", choices=VALID_MEDIA_MODES, default="", help="媒体模式: lossless 或 copy")
    parser.add_argument("--android-mode", choices=VALID_ANDROID_MODES, default="", help="Android 签名模式: debug 或 release")
    parser.add_argument("--no-brotli", action="store_true", help="关闭 game_data Brotli 压缩")
    parser.add_argument("--no-installer", action="store_true", help="关闭 Windows Inno Setup 安装包")
    parser.add_argument("--official-templates", action="store_true", help="强制使用官方模板而非定制 2D Mono 模板")
    parser.add_argument("--hd", action="store_true", help="附加 2× HD 媒体包（Windows）")
    parser.add_argument("--ios-ipa", default="", help="指定无签名 iOS 引擎壳 .ipa 路径")
    parser.add_argument("--ios-ci-fetch", action="store_true", help="从 GitHub Actions 拉取最近一次成功的 ios-unsigned.yml 引擎壳")
    parser.add_argument("--ios-ci-trigger", action="store_true", help="触发 GitHub Actions ios-unsigned.yml 并等待拉取引擎壳")
    parser.add_argument("--ios-ci-ref", default="", help="配合 --ios-ci-trigger 使用的分支/Tag（默认当前分支）")
    parser.add_argument("--allow-placeholder", action="store_true", help="允许在缺失 _publish 时使用占位图标")
    parser.add_argument("--yes", action="store_true", help="跳过交互确认直接执行")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    ui = UI()
    cfg = load_config()
    if args.version:
        cfg.version = args.version
    if args.media_mode:
        cfg.media_mode = args.media_mode
    if args.android_mode:
        cfg.android_mode = args.android_mode
    if args.no_brotli:
        cfg.brotli = False
    if args.no_installer:
        cfg.win_installer = False
    if args.official_templates:
        cfg.use_custom_templates = False
    if args.hd:
        cfg.include_hd = True
    if args.ios_ipa:
        cfg.ios_engine_ipa = args.ios_ipa
    if args.allow_placeholder:
        cfg.allow_placeholder_icons = True
    cfg = cfg.normalized()

    if args.ios_ci_fetch and args.ios_ci_trigger:
        print("ERROR: --ios-ci-fetch and --ios-ci-trigger are mutually exclusive", file=sys.stderr)
        return 2

    if args.check:
        env_info = collect_environment(REPO_ROOT)
        ui.render_dashboard(cfg, env_info)
        return 0

    if args.rebuild_media:
        cmd = build_rebuild_media_command(REPO_ROOT, cfg.media_mode)
        rc, _ = run_streaming_command(cmd, {}, REPO_ROOT, "重建剪枝真无损 media.pak", ui)
        return rc

    if args.ios_ci_fetch or args.ios_ci_trigger:
        mode = "trigger" if args.ios_ci_trigger else "fetch"
        got_ipa = acquire_ios_ipa_via_ci(REPO_ROOT, cfg, ui, mode=mode, ref=args.ios_ci_ref)
        if not got_ipa:
            return 1
        cfg.ios_engine_ipa = str(got_ipa)
        save_config(cfg)
        if not args.build and not args.plan and not args.verify:
            return 0

    if args.verify and not args.build and not args.plan:
        return verify_dist_artifacts(REPO_ROOT, cfg, ui, smoke_setup=args.smoke_setup)

    if args.plan or args.build:
        targets = [t.strip().lower() for t in (args.build or "windows,android,ios").split(",") if t.strip()]
        invalid = [t for t in targets if t not in ("windows", "android", "ios")]
        if invalid:
            print(f"ERROR: unknown target(s): {', '.join(invalid)}", file=sys.stderr)
            return 2
        env_info = collect_environment(REPO_ROOT)
        if args.plan:
            ui.render_dashboard(cfg, env_info)
            for t in targets:
                cmd, _, warns = build_platform_plan(t, cfg, env_info, REPO_ROOT)
                blockers = validate_target_readiness(t, cfg, env_info)
                print(f"PLAN {t}: {format_cmd(cmd)}")
                for w in warns:
                    print(f"  WARN: {w}")
                for b in blockers:
                    print(f"  BLOCKER: {b}")
            return 0
        rc = execute_targets(targets, cfg, REPO_ROOT, ui, auto_yes=args.yes)
        if rc == 0 and (args.verify or args.smoke_setup):
            return verify_dist_artifacts(REPO_ROOT, cfg, ui, smoke_setup=args.smoke_setup)
        return rc

    return interactive_loop(REPO_ROOT)


if __name__ == "__main__":
    raise SystemExit(main())
