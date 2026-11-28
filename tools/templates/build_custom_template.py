#!/usr/bin/env python3
"""Build custom 2D/GLES3 + Mono Godot 4.7.2 export templates from source.

Supports Windows (x86_64), Android (arm64), and iOS (arm64). Uses
`tools/templates/custom_2d_mono.py` as the shared SCons profile and writes
ready-to-use export templates (`windows_release_x86_64.exe`,
`android_release.apk`, or `ios.zip`).
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
DEFAULT_PROFILE = SCRIPT_DIR / "custom_2d_mono.py"

PLATFORM_FLAGS: dict[str, list[str]] = {
    "windows": [
        "platform=windows",
        "arch=x86_64",
        "d3d12=no",
        "sdl=no",
        "use_static_cpp=yes",
    ],
    "android": [
        "platform=android",
        "arch=arm64",
        "lto=thin",
        "swappy=no",
        "generate_apk=yes",
    ],
    "ios": [
        "platform=ios",
        "arch=arm64",
        "metal=no",
    ],
}

EXPECTED_ARTIFACTS: dict[str, str] = {
    "windows": "windows_release_x86_64.exe",
    "android": "android_release.apk",
    "ios": "ios.zip",
}


def build_scons_args(platform_name: str, jobs: int) -> list[str]:
    if platform_name not in PLATFORM_FLAGS:
        raise ValueError(f"unsupported platform: {platform_name}")
    return [
        f"-j{max(1, jobs)}",
        "target=template_release",
        "module_mono_enabled=yes",
        *PLATFORM_FLAGS[platform_name],
    ]


def find_built_binary(bin_dir: Path, platform_name: str) -> Path:
    if platform_name == "windows":
        patterns = ("godot.windows.template_release.x86_64*.exe", "*windows*template_release*x86_64*.exe")
    elif platform_name == "android":
        # Godot's generateGodotMonoTemplates outputs android_monoRelease.apk when module_mono_enabled=yes.
        patterns = ("android_monoRelease.apk", "android_release.apk", "android_mono*Release*.apk", "*android*release*.apk")
    elif platform_name == "ios":
        patterns = ("libgodot.ios.template_release.arm64*.a", "*ios*template_release*arm64*.a")
    else:
        raise ValueError(f"unsupported platform: {platform_name}")

    for pat in patterns:
        matches = sorted(
            p
            for p in bin_dir.glob(pat)
            if p.is_file() and ".console." not in p.name and "-unsigned." not in p.name
        )
        if matches:
            return matches[0]
    existing = sorted(p.name for p in bin_dir.iterdir()) if bin_dir.is_dir() else []
    raise FileNotFoundError(
        f"No built {platform_name} template binary found in {bin_dir} (patterns={patterns}, existing={existing})"
    )


def inject_ios_static_lib(base_ios_zip: Path, custom_lib_a: Path, out_ios_zip: Path) -> int:
    """Replace release arm64 libgodot*.a entries inside an official ios.zip template."""
    if not base_ios_zip.is_file() or not zipfile.is_zipfile(base_ios_zip):
        raise FileNotFoundError(f"base ios.zip not found or invalid: {base_ios_zip}")
    if not custom_lib_a.is_file():
        raise FileNotFoundError(f"custom iOS static library not found: {custom_lib_a}")

    lib_bytes = custom_lib_a.read_bytes()
    out_ios_zip.parent.mkdir(parents=True, exist_ok=True)
    tmp_zip = out_ios_zip.with_name(out_ios_zip.name + ".tmp")
    if tmp_zip.exists():
        tmp_zip.unlink()

    replaced = 0
    try:
        with zipfile.ZipFile(base_ios_zip, "r") as zin, zipfile.ZipFile(
            tmp_zip, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6
        ) as zout:
            for info in zin.infolist():
                clone = zipfile.ZipInfo(info.filename, info.date_time)
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

                low = info.filename.lower()
                is_release_arm64_lib = (
                    low.endswith(".a")
                    and "release" in low
                    and ("arm64" in low or "ios-arm64" in low)
                    and "simulator" not in low
                    and "libgodot" in low
                )
                if is_release_arm64_lib:
                    zout.writestr(clone, lib_bytes)
                    replaced += 1
                else:
                    zout.writestr(clone, zin.read(info))

        if replaced == 0:
            raise RuntimeError(f"No release arm64 libgodot*.a entry matched inside {base_ios_zip}")
        os.replace(tmp_zip, out_ios_zip)
    finally:
        if tmp_zip.exists():
            tmp_zip.unlink()
    return replaced


def run_build(
    platform_name: str,
    godot_src: Path,
    out_dir: Path,
    profile_path: Path,
    jobs: int,
    base_ios_zip: Path | None,
    dry_run: bool,
) -> Path:
    args = build_scons_args(platform_name, jobs)
    out_artifact = out_dir / EXPECTED_ARTIFACTS[platform_name]

    if dry_run:
        print(f"CUSTOM_TEMPLATE_DRY_RUN platform={platform_name} scons={' '.join(args)} out={out_artifact}")
        return out_artifact

    if not godot_src.is_dir() or not (godot_src / "SConstruct").is_file():
        raise FileNotFoundError(f"Godot source tree with SConstruct not found: {godot_src}")
    if not profile_path.is_file():
        raise FileNotFoundError(f"SCons profile not found: {profile_path}")

    scons_bin = shutil.which("scons")
    if not scons_bin:
        raise RuntimeError("scons not found on PATH (pip install scons)")

    shutil.copy2(profile_path, godot_src / "custom.py")
    subprocess.run([scons_bin, *args], cwd=godot_src, check=True)

    built = find_built_binary(godot_src / "bin", platform_name)
    out_dir.mkdir(parents=True, exist_ok=True)
    if platform_name == "ios":
        if base_ios_zip is None:
            raise ValueError("--base-ios-zip is required when packaging custom iOS template into ios.zip")
        strip_bin = shutil.which("strip")
        if strip_bin:
            subprocess.run([strip_bin, "-S", "-x", str(built)], check=True)
        replaced = inject_ios_static_lib(base_ios_zip, built, out_artifact)
        print(f"CUSTOM_TEMPLATE_OK platform=ios replaced={replaced} out={out_artifact} bytes={out_artifact.stat().st_size}")
    else:
        shutil.copy2(built, out_artifact)
        print(f"CUSTOM_TEMPLATE_OK platform={platform_name} out={out_artifact} bytes={out_artifact.stat().st_size}")
    return out_artifact


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", required=True, choices=("windows", "android", "ios"))
    parser.add_argument("--godot-src", type=Path, required=True, help="Path to Godot 4.7.2-stable source directory")
    parser.add_argument("--out", type=Path, required=True, help="Output directory for built template artifact")
    parser.add_argument("--profile", type=Path, default=DEFAULT_PROFILE, help="SCons custom.py profile path")
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    parser.add_argument("--base-ios-zip", type=Path, default=None, help="Official 4.7.2.stable.mono ios.zip base (iOS only)")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)

    run_build(
        platform_name=args.platform,
        godot_src=args.godot_src,
        out_dir=args.out,
        profile_path=args.profile,
        jobs=args.jobs,
        base_ios_zip=args.base_ios_zip,
        dry_run=args.dry_run,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
