#!/usr/bin/env python3
"""Remove compile-only libraries, align, and sign an exported Android APK."""
from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path, PurePosixPath

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import env_check  # noqa: E402
import strip_android_static_libs  # noqa: E402

MEDIA_ENTRY = "assets/mobile_data/work/all/media.pak.blob.cfb"
PAGE_SIZE_KB = 16
ZIP_ALIGNMENT = 4


def fail(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)


def _tool_name(name: str) -> str:
    return name + ".exe" if os.name == "nt" else name


def _version_key(path: Path) -> tuple[int, ...]:
    match = re.fullmatch(r"(\d+(?:\.\d+)*)", path.name)
    return tuple(int(part) for part in match.group(1).split(".")) if match else ()


def find_build_tools(
    android_sdk: Path | None = None,
    build_tools: Path | None = None,
) -> tuple[Path, Path]:
    if build_tools is not None:
        candidates = [build_tools.resolve()]
    else:
        discovered_sdk = env_check.find_android_sdk()
        sdk = android_sdk or (Path(discovered_sdk) if discovered_sdk else None)
        if sdk is None:
            fail("Android SDK not found; set ANDROID_HOME/ANDROID_SDK_ROOT or pass --android-sdk")
        root = sdk / "build-tools"
        if not root.is_dir():
            fail(f"Android build-tools directory not found: {root}")
        candidates = sorted((p for p in root.iterdir() if p.is_dir()), key=_version_key, reverse=True)

    for candidate in candidates:
        zipalign = candidate / _tool_name("zipalign")
        apksigner_jar = candidate / "lib" / "apksigner.jar"
        if zipalign.is_file() and apksigner_jar.is_file():
            return zipalign, apksigner_jar
    fail("no Android build-tools version contains both zipalign and lib/apksigner.jar")
    raise AssertionError("unreachable")


def find_java(java: str | None = None) -> str:
    candidate = java or env_check.find_jdk_tool("java")
    if candidate is None:
        fail("Java not found; set JAVA_HOME or pass --java")
    if Path(candidate).is_file():
        return str(Path(candidate).resolve())
    found = shutil.which(candidate)
    if found:
        return found
    fail(f"Java executable not found: {candidate}")
    raise AssertionError("unreachable")


def _run(command: list[str], label: str) -> None:
    try:
        proc = subprocess.run(
            command,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
        )
    except OSError as exc:
        fail(f"{label} could not start: {exc}")
    if proc.returncode == 0:
        return
    detail = (proc.stderr or proc.stdout or "").strip()
    fail(f"{label} failed with exit {proc.returncode}: {detail}")


def verify_apk_layout(apk: Path, *, require_stripped: bool = True) -> None:
    if not apk.is_file() or not zipfile.is_zipfile(apk):
        fail(f"not a valid APK archive: {apk}")

    errors: list[str] = []
    with zipfile.ZipFile(apk, "r") as archive:
        infos = {info.filename: info for info in archive.infolist()}
        if require_stripped:
            static_archives = [
                name
                for name in infos
                if PurePosixPath(name).suffix == ".a" and name.startswith("lib/")
            ]
            if static_archives:
                errors.append(f"static archives remain: {', '.join(sorted(static_archives))}")

        media_info = infos.get(MEDIA_ENTRY)
        if media_info is None or media_info.file_size <= 0:
            errors.append(f"missing or empty media payload: {MEDIA_ENTRY}")
        elif media_info.compress_type != zipfile.ZIP_STORED:
            errors.append(f"media payload must use ZIP_STORED: {MEDIA_ENTRY}")

        if not any(name.startswith("lib/") and name.endswith(".so") for name in infos):
            errors.append("APK contains no native .so libraries")
        if "AndroidManifest.xml" not in infos or "classes.dex" not in infos:
            errors.append("APK is missing AndroidManifest.xml or classes.dex")

    if errors:
        fail("; ".join(errors))


def postprocess(
    apk: Path,
    keystore: Path,
    ks_user: str,
    ks_pass: str,
    key_pass: str | None,
    java: str,
    zipalign: Path,
    apksigner_jar: Path,
    keep_work: bool = False,
) -> int:
    if apk.is_symlink():
        fail(f"refusing symlink APK: {apk}")
    apk = apk.resolve()
    keystore = keystore.resolve()
    if not apk.is_file():
        fail(f"APK not found: {apk}")
    if not keystore.is_file():
        fail(f"keystore not found: {keystore}")

    verify_apk_layout(apk, require_stripped=False)
    work_dir = Path(tempfile.mkdtemp(prefix=".lwsmf-apk-", dir=str(apk.parent)))
    stripped = work_dir / "stripped.apk"
    aligned = work_dir / "aligned.apk"
    signed = work_dir / "signed.apk"
    password = key_pass or ks_pass

    try:
        print("== removing compile-only static archives ==")
        strip_android_static_libs.main([str(apk), str(stripped)])

        print(f"== aligning APK to {PAGE_SIZE_KB} KB pages ==")
        _run(
            [
                str(zipalign),
                "-f",
                "-P",
                str(PAGE_SIZE_KB),
                str(ZIP_ALIGNMENT),
                str(stripped),
                str(aligned),
            ],
            "zipalign",
        )
        _run(
            [
                str(zipalign),
                "-c",
                "-P",
                str(PAGE_SIZE_KB),
                str(ZIP_ALIGNMENT),
                str(aligned),
            ],
            "zipalign check",
        )

        print("== signing APK (v2/v3) ==")
        _run(
            [
                java,
                "-jar",
                str(apksigner_jar),
                "sign",
                "--ks",
                str(keystore),
                "--ks-key-alias",
                ks_user,
                f"--ks-pass=pass:{ks_pass}",
                f"--key-pass=pass:{password}",
                "--v1-signing-enabled",
                "false",
                "--v2-signing-enabled",
                "true",
                "--v3-signing-enabled",
                "true",
                "--v4-signing-enabled",
                "false",
                "--out",
                str(signed),
                str(aligned),
            ],
            "apksigner sign",
        )
        _run(
            [java, "-jar", str(apksigner_jar), "verify", "--verbose", "--print-certs", str(signed)],
            "apksigner verify",
        )
        _run(
            [
                str(zipalign),
                "-c",
                "-P",
                str(PAGE_SIZE_KB),
                str(ZIP_ALIGNMENT),
                str(signed),
            ],
            "final zipalign check",
        )
        verify_apk_layout(signed)

        os.replace(signed, apk)
        idsig = Path(str(apk) + ".idsig")
        if idsig.is_file():
            idsig.unlink()
    finally:
        if keep_work:
            print(f"ANDROID_APK_POSTPROCESS_WORK={work_dir}")
        else:
            shutil.rmtree(work_dir, ignore_errors=True)

    print(f"ANDROID_APK_POSTPROCESS_OK apk={apk} bytes={apk.stat().st_size}")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("apk", type=Path)
    parser.add_argument("--keystore", required=True, type=Path)
    parser.add_argument("--ks-user", default="androiddebugkey")
    parser.add_argument("--ks-pass", default="android")
    parser.add_argument("--key-pass", default=None)
    parser.add_argument("--java", default=None)
    parser.add_argument("--android-sdk", type=Path, default=None)
    parser.add_argument("--build-tools", type=Path, default=None)
    parser.add_argument("--keep-work", action="store_true")
    args = parser.parse_args(argv)

    zipalign, apksigner_jar = find_build_tools(args.android_sdk, args.build_tools)
    java = find_java(args.java)
    return postprocess(
        apk=args.apk,
        keystore=args.keystore,
        ks_user=args.ks_user,
        ks_pass=args.ks_pass,
        key_pass=args.key_pass,
        java=java,
        zipalign=zipalign,
        apksigner_jar=apksigner_jar,
        keep_work=args.keep_work,
    )


if __name__ == "__main__":
    raise SystemExit(main())
