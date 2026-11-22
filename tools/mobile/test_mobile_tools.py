#!/usr/bin/env python3
"""Unit tests for LWSMF mobile packaging helpers (tools/mobile/*)."""
from __future__ import annotations

import json
import stat
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

MOBILE_DIR = Path(__file__).resolve().parent
if str(MOBILE_DIR) not in sys.path:
    sys.path.insert(0, str(MOBILE_DIR))

import android_data  # noqa: E402
import env_check  # noqa: E402
import export_preset_template  # noqa: E402
import ios_inject_data  # noqa: E402


def _make_fake_payload(root: Path) -> tuple[Path, Path]:
    gd = root / "game_data"
    wa = root / "work/all"
    bd = wa / "BasicData"
    gd.mkdir(parents=True)
    bd.mkdir(parents=True)

    (gd / "manifest.json").write_text('{"schema":"lwsmf.manifest","assets":[]}\n', encoding="utf-8")
    (gd / "programs.json").write_text('{\n  "common_events": []\n}\n', encoding="utf-8")
    (gd / "database.json").write_text('{\n  "types": []\n}\n', encoding="utf-8")
    (gd / "map_tiles.json").write_text('{\n  "maps": []\n}\n', encoding="utf-8")
    for proj in android_data.PROJECT_FILES:
        (bd / proj).write_text("<Project/>\n", encoding="utf-8")
    (wa / "media.pak").write_bytes(b"WOLFMPK1" + b"\x00" * 64)
    return gd, wa


def _build_valid_apk(apk_path: Path, staged_dir: Path, skip_entry: str | None = None) -> None:
    with zipfile.ZipFile(apk_path, "w") as zf:
        for p in sorted(staged_dir.rglob("*")):
            if not p.is_file():
                continue
            rel = p.relative_to(staged_dir).as_posix()
            entry_name = f"assets/mobile_data/{rel}"
            if entry_name == skip_entry:
                continue
            ctype = zipfile.ZIP_STORED if rel.endswith(".blob.cfb") else zipfile.ZIP_DEFLATED
            zf.writestr(entry_name, p.read_bytes(), compress_type=ctype)
        if skip_entry != "assets/.godot/mono/publish/arm64/Lwsmf.dll":
            zf.writestr("assets/.godot/mono/publish/arm64/Lwsmf.dll", b"MZ_STUB", compress_type=zipfile.ZIP_DEFLATED)
        zf.writestr("lib/arm64-v8a/libgodot_android.so", b"\x7fELF", compress_type=zipfile.ZIP_DEFLATED)


def _build_synthetic_ipa(ipa_path: Path, extra_member: str | None = None) -> None:
    with zipfile.ZipFile(ipa_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        dir_info = zipfile.ZipInfo("Payload/Lwsmf.app/")
        dir_info.create_system = 3
        dir_info.external_attr = ((stat.S_IFDIR | 0o755) << 16) | 0x10
        zf.writestr(dir_info, b"")

        exe_info = zipfile.ZipInfo("Payload/Lwsmf.app/Lwsmf")
        exe_info.create_system = 3
        exe_info.external_attr = (stat.S_IFREG | 0o755) << 16
        zf.writestr(exe_info, b"\xcf\xfa\xed\xfe_stub_macho")

        link_info = zipfile.ZipInfo("Payload/Lwsmf.app/Frameworks/Current")
        link_info.create_system = 3
        link_info.external_attr = (stat.S_IFLNK | 0o777) << 16
        zf.writestr(link_info, b"A")

        zf.writestr("Payload/Lwsmf.app/game_data/old.json", b"{}")

        if extra_member:
            zf.writestr(extra_member, b"bad")


class MobileToolsTest(unittest.TestCase):
    def test_android_stage_and_verify_apk(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            gd, wa = _make_fake_payload(t / "src")
            dest = t / "staged_mobile_data"
            rc = android_data.main(
                [
                    "stage",
                    "--game-data",
                    str(gd),
                    "--work-all",
                    str(wa),
                    "--dest",
                    str(dest),
                    "--version",
                    "0.1.0",
                    "--allow-temp",
                ]
            )
            self.assertEqual(rc, 0)
            manifest = json.loads((dest / "mobile_data_manifest.json").read_text(encoding="utf-8"))
            self.assertEqual(manifest["version"], "0.1.0")
            self.assertTrue((dest / "game_data/programs.json.blob").is_file())
            self.assertEqual((dest / "game_data/programs.json.blob").read_text(encoding="utf-8"), '{"common_events":[]}')
            self.assertTrue((dest / "work/all/media.pak.blob.cfb").is_file())

            apk = t / "test.apk"
            _build_valid_apk(apk, dest)
            self.assertEqual(android_data.main(["verify-apk", str(apk)]), 0)

            # Negative case: compressed media.pak.blob.cfb fails verify-apk
            bad_apk = t / "bad_deflate.apk"
            with zipfile.ZipFile(bad_apk, "w", compression=zipfile.ZIP_DEFLATED) as zf:
                with zipfile.ZipFile(apk, "r") as src_apk:
                    for info in src_apk.infolist():
                        zf.writestr(info.filename, src_apk.read(info.filename), compress_type=zipfile.ZIP_DEFLATED)
                zf.writestr("assets/mobile_data/work/all/save/save_01.json", b"{}")
            self.assertEqual(android_data.main(["verify-apk", str(bad_apk)]), 1)

            for required_entry in (
                "assets/mobile_data/game_data/manifest.json.blob",
                "assets/mobile_data/work/all/BasicData/CDataBase.project.blob",
                "assets/mobile_data/work/all/BasicData/SysDatabase.project.blob",
                "assets/.godot/mono/publish/arm64/Lwsmf.dll",
            ):
                missing_apk = t / "missing_entry.apk"
                _build_valid_apk(missing_apk, dest, skip_entry=required_entry)
                self.assertEqual(
                    android_data.main(["verify-apk", str(missing_apk)]),
                    1,
                    f"expected verify-apk to fail when {required_entry} is missing",
                )

    def test_android_safe_dest_rejects_outside(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            outside = Path(tmp) / "outside"
            with self.assertRaises(SystemExit):
                android_data.ensure_safe_dest(outside, allow_temp=False)

    def test_ios_inject_preserves_modes_and_replaces_data(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            gd, wa = _make_fake_payload(t / "src")
            font = t / "Lolita.ttf"
            font.write_bytes(b"TTF_STUB")
            ipa_in = t / "engine-only.ipa"
            ipa_out = t / "full.ipa"
            _build_synthetic_ipa(ipa_in)

            ios_inject_data.inject(
                ipa=ipa_in,
                game_data_dir=gd,
                work_all_dir=wa,
                output=ipa_out,
                media_pak_override=None,
                font=font,
                force=True,
            )

            with zipfile.ZipFile(ipa_out, "r") as zf:
                names = set(zf.namelist())
                self.assertNotIn("Payload/Lwsmf.app/game_data/old.json", names)
                self.assertIn("Payload/Lwsmf.app/game_data/programs.json", names)
                self.assertIn("Payload/Lwsmf.app/work/all/media.pak", names)
                self.assertIn("Payload/Lwsmf.app/work/all/Lolita.ttf", names)
                self.assertEqual(zf.getinfo("Payload/Lwsmf.app/work/all/media.pak").compress_type, zipfile.ZIP_STORED)
                exe_mode = (zf.getinfo("Payload/Lwsmf.app/Lwsmf").external_attr >> 16) & 0o777
                self.assertEqual(exe_mode, 0o755)
                lnk_type = (zf.getinfo("Payload/Lwsmf.app/Frameworks/Current").external_attr >> 16) & stat.S_IFMT(0o170000)
                self.assertEqual(lnk_type, stat.S_IFLNK)

    def test_ios_inject_rejects_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            gd, wa = _make_fake_payload(t / "src")
            bad_ipa = t / "traversal.ipa"
            _build_synthetic_ipa(bad_ipa, extra_member="Payload/Lwsmf.app/../../evil.txt")
            with self.assertRaises(SystemExit):
                ios_inject_data.inject(bad_ipa, gd, wa, t / "out.ipa", None, None, True)

    def test_export_preset_patch_and_env_check_parse(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            cfg = Path(tmp) / "export_presets.cfg"
            cfg.write_text(
                '[preset.0]\nname="Android"\nplatform="Android"\n\n'
                '[preset.0.options]\ncustom_template/debug=""\nkeystore/debug=""\n',
                encoding="utf-8",
            )
            export_preset_template.patch_option(cfg, "Android", "keystore/debug", "/tmp/debug.keystore")
            presets = env_check.parse_export_presets(cfg)
            self.assertEqual(presets["Android"]["keystore/debug"], "/tmp/debug.keystore")

            export_preset_template.patch_option(cfg, "Android", "custom_template/debug", r'C:\Godot\tpl "mono".apk')
            text = cfg.read_text(encoding="utf-8")
            self.assertIn(r'custom_template/debug="C:\\Godot\\tpl \"mono\".apk"', text)

    def test_env_check_blockers_and_sdk_subdirs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            incomplete_sdk = t / "sdk_incomplete"
            (incomplete_sdk / "platform-tools").mkdir(parents=True)
            self.assertFalse(env_check.is_valid_android_sdk(incomplete_sdk))

            valid_sdk = t / "sdk_valid"
            (valid_sdk / "platform-tools").mkdir(parents=True)
            (valid_sdk / "build-tools" / "34.0.0").mkdir(parents=True)
            self.assertTrue(env_check.is_valid_android_sdk(valid_sdk))

            repo = t / "repo"
            (repo / "godot").mkdir(parents=True)
            (repo / "godot/export_presets.cfg").write_text(
                '[preset.0]\nname="Windows Desktop"\nexclude_filter="mobile_data/*"\n'
                '[preset.1]\nname="Android"\nplatform="Android"\narchitectures/arm64-v8a=true\n',
                encoding="utf-8",
            )
            non_mono_tpl = t / "4.7.2.stable"
            non_mono_tpl.mkdir()
            with (
                mock.patch.object(env_check, "find_godot_bin", return_value=None),
                mock.patch.object(env_check, "godot_templates_dir", return_value=non_mono_tpl),
                mock.patch.object(env_check, "find_dotnet_bin", return_value=None),
                mock.patch.object(env_check, "find_jdk_tool", return_value=None),
                mock.patch.object(env_check, "find_android_sdk", return_value=None),
            ):
                rows, blockers = env_check.check_environment(repo, target="android")
                by_label = {label: status for status, label, _ in rows}
                self.assertEqual(by_label["godot"], "MISSING")
                self.assertEqual(by_label["templates_dir"], "MISSING")
                self.assertGreaterEqual(blockers, 5)

            fake_proc = mock.Mock(returncode=1, stdout="", stderr="boom stderr")
            with mock.patch.object(env_check.subprocess, "run", return_value=fake_proc):
                versions, err = env_check.dotnet_sdk_versions("dotnet")
                self.assertEqual(versions, [])
                self.assertIn("boom stderr", err)


if __name__ == "__main__":
    unittest.main()
