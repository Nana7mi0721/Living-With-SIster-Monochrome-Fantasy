#!/usr/bin/env python3
"""Unit tests for LWSMF mobile packaging helpers (tools/mobile/*)."""
from __future__ import annotations

import argparse
import json
import shutil
import stat
import struct
import subprocess
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
import postprocess_android_apk  # noqa: E402
import strip_android_static_libs  # noqa: E402
import windows_pe  # noqa: E402


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


def _build_synthetic_pe32_plus(trailing_pck: bytes = b"", lang_ids: tuple[int, ...] = (0x0409,)) -> bytes:
    """Build a minimal valid PE32+ executable with RT_VERSION (0x0409) + trailing bytes."""
    ffi = bytearray(52)
    struct.pack_into("<IIIIII", ffi, 0, windows_pe.VS_FFI_SIGNATURE, 0x00010000, 0x00000001, 0x00000000, 0x00000001, 0x00000000)

    def str_node(k: str, v: str) -> windows_pe.VersionNode:
        val = v.encode("utf-16-le") + b"\x00\x00"
        return windows_pe.VersionNode(0, 0, 0, k, 1, len(val) // 2, 0, len(val), val)

    st = windows_pe.VersionNode(
        0,
        0,
        0,
        "040904b0",
        1,
        0,
        0,
        0,
        b"",
        [
            str_node("CompanyName", "LWSMF Contributors"),
            str_node("FileVersion", "0.1.0.0"),
            str_node("ProductVersion", "0.1.0.0"),
        ],
    )
    sfi = windows_pe.VersionNode(0, 0, 0, "StringFileInfo", 1, 0, 0, 0, b"", [st])
    var = windows_pe.VersionNode(
        0,
        0,
        0,
        "Translation",
        0,
        4,
        0,
        4,
        struct.pack("<HH", 0x0409, 0x04B0),
    )
    vfi = windows_pe.VersionNode(0, 0, 0, "VarFileInfo", 1, 0, 0, 0, b"", [var])
    vs_root = windows_pe.VersionNode(0, 0, 0, "VS_VERSION_INFO", 0, len(ffi), 0, len(ffi), bytes(ffi), [sfi, vfi])
    vs_blob = windows_pe.serialize_version_tree(vs_root)

    # Resource layout inside .rsrc (RVA 0x2000):
    # lvl1 dir (24B) + lvl2 dir (24B) + lvl3 dir (16B + 8B per language) + data_entry (16B) + vs_blob
    lvl1_off = 0
    lvl2_off = 24
    lvl3_off = 48
    data_entry_off = lvl3_off + 16 + 8 * len(lang_ids)
    vs_off = data_entry_off + 16
    rsrc_rva = 0x2000
    rsrc_buf = bytearray(vs_off + len(vs_blob))

    struct.pack_into("<IIHHHH", rsrc_buf, lvl1_off, 0, 0, 0, 0, 0, 1)
    struct.pack_into("<II", rsrc_buf, lvl1_off + 16, windows_pe.RT_VERSION, 0x80000000 | lvl2_off)

    struct.pack_into("<IIHHHH", rsrc_buf, lvl2_off, 0, 0, 0, 0, 0, 1)
    struct.pack_into("<II", rsrc_buf, lvl2_off + 16, 1, 0x80000000 | lvl3_off)

    struct.pack_into("<IIHHHH", rsrc_buf, lvl3_off, 0, 0, 0, 0, 0, len(lang_ids))
    for index, lang in enumerate(lang_ids):
        struct.pack_into("<II", rsrc_buf, lvl3_off + 16 + index * 8, lang, data_entry_off)

    struct.pack_into("<IIII", rsrc_buf, data_entry_off, rsrc_rva + vs_off, len(vs_blob), 0, 0)
    rsrc_buf[vs_off : vs_off + len(vs_blob)] = vs_blob

    e_lfanew = 0x80
    opt_size = 240  # 112 + 16 * 8
    raw_rsrc_off = 0x400
    pe_buf = bytearray(raw_rsrc_off)
    pe_buf[0:2] = b"MZ"
    struct.pack_into("<I", pe_buf, 0x3C, e_lfanew)
    pe_buf[e_lfanew : e_lfanew + 4] = b"PE\x00\x00"
    coff_off = e_lfanew + 4
    struct.pack_into("<HHIIIHH", pe_buf, coff_off, 0x8664, 1, 0, 0, 0, opt_size, 0x0022)
    opt_off = coff_off + 20
    struct.pack_into("<H", pe_buf, opt_off, windows_pe.PE32_PLUS_MAGIC)
    struct.pack_into("<I", pe_buf, opt_off + 108, 16)
    struct.pack_into("<II", pe_buf, opt_off + 112 + 2 * 8, rsrc_rva, len(rsrc_buf))

    sec_off = opt_off + opt_size
    pe_buf[sec_off : sec_off + 8] = b".rsrc\x00\x00\x00"
    struct.pack_into("<IIII", pe_buf, sec_off + 8, len(rsrc_buf), rsrc_rva, len(rsrc_buf), raw_rsrc_off)

    return bytes(pe_buf) + bytes(rsrc_buf) + trailing_pck


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

    def test_android_strip_static_libs_preserves_media_storage(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            source = t / "source.apk"
            output = t / "output.apk"
            media_name = "assets/mobile_data/work/all/media.pak.blob.cfb"
            with zipfile.ZipFile(source, "w") as zf:
                zf.writestr("lib/arm64-v8a/libmonosgen-2.0.a", b"static", compress_type=zipfile.ZIP_STORED)
                zf.writestr("lib/x86_64/libmonosgen-2.0.a", b"static", compress_type=zipfile.ZIP_STORED)
                zf.writestr("lib/arm64-v8a/libmonosgen-2.0.so", b"\x7fELF", compress_type=zipfile.ZIP_DEFLATED)
                zf.writestr(media_name, b"WOLFMPK1", compress_type=zipfile.ZIP_STORED)

            self.assertEqual(strip_android_static_libs.main([str(source), str(output)]), 0)
            with zipfile.ZipFile(output, "r") as zf:
                names = zf.namelist()
                self.assertNotIn("lib/arm64-v8a/libmonosgen-2.0.a", names)
                self.assertNotIn("lib/x86_64/libmonosgen-2.0.a", names)
                self.assertIn("lib/arm64-v8a/libmonosgen-2.0.so", names)
                self.assertEqual(zf.getinfo(media_name).compress_type, zipfile.ZIP_STORED)

    def test_android_postprocess_aligns_and_signs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            apk = t / "app.apk"
            keystore = t / "debug.keystore"
            keystore.write_bytes(b"keystore")
            media_name = postprocess_android_apk.MEDIA_ENTRY
            with zipfile.ZipFile(apk, "w") as zf:
                zf.writestr("AndroidManifest.xml", b"manifest")
                zf.writestr("classes.dex", b"dex")
                zf.writestr("lib/arm64-v8a/libmonosgen-2.0.a", b"static", compress_type=zipfile.ZIP_STORED)
                zf.writestr("lib/arm64-v8a/libmonosgen-2.0.so", b"\x7fELF", compress_type=zipfile.ZIP_STORED)
                zf.writestr(media_name, b"WOLFMPK1", compress_type=zipfile.ZIP_STORED)

            zipalign = t / "zipalign"
            apksigner_jar = t / "apksigner.jar"
            zipalign.write_bytes(b"")
            apksigner_jar.write_bytes(b"")

            def fake_run(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                if "sign" in command:
                    shutil.copy2(command[-1], command[-2])
                elif "-f" in command:
                    shutil.copy2(command[-2], command[-1])
                return subprocess.CompletedProcess(command, 0, "", "")

            with mock.patch.object(postprocess_android_apk.subprocess, "run", side_effect=fake_run):
                rc = postprocess_android_apk.postprocess(
                    apk=apk,
                    keystore=keystore,
                    ks_user="androiddebugkey",
                    ks_pass="android",
                    key_pass=None,
                    java="java",
                    zipalign=zipalign,
                    apksigner_jar=apksigner_jar,
                )
            self.assertEqual(rc, 0)
            with zipfile.ZipFile(apk, "r") as zf:
                names = zf.namelist()
                self.assertNotIn("lib/arm64-v8a/libmonosgen-2.0.a", names)
                self.assertEqual(zf.getinfo(media_name).compress_type, zipfile.ZIP_STORED)
            self.assertFalse(Path(str(apk) + ".idsig").exists())

    def test_android_postprocess_finds_latest_build_tools(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            build_tools = Path(tmp) / "build-tools"
            for version in ("35.0.0", "36.1.0"):
                root = build_tools / version
                (root / "lib").mkdir(parents=True)
                (root / postprocess_android_apk._tool_name("zipalign")).write_bytes(b"")
                (root / "lib/apksigner.jar").write_bytes(b"")

            zipalign, apksigner_jar = postprocess_android_apk.find_build_tools(
                android_sdk=Path(tmp),
            )
            self.assertEqual(zipalign.parent.name, "36.1.0")
            self.assertEqual(apksigner_jar.parent.parent.name, "36.1.0")

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

    def test_windows_pe_localize_preserves_pck_and_updates_checksum(self) -> None:
        pck_tail = b"GDPC\x02\x00\x00\x00" + bytes(range(256)) + b"ODD"
        pe_bytes = _build_synthetic_pe32_plus(trailing_pck=pck_tail)
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "Lwsmf.exe"
            exe.write_bytes(pe_bytes)

            self.assertEqual(windows_pe.main(["--check", str(exe)]), 1)
            self.assertEqual(windows_pe.main(["--dry-run", str(exe)]), 0)
            self.assertEqual(exe.read_bytes(), pe_bytes)

            self.assertEqual(windows_pe.main([str(exe)]), 0)
            patched = exe.read_bytes()
            self.assertEqual(len(patched), len(pe_bytes))
            self.assertEqual(patched[-len(pck_tail) :], pck_tail)

            pe = windows_pe.parse_pe(patched)
            refs = windows_pe.find_version_resources(patched, pe)
            self.assertEqual([r.lang_id for r in refs], [0x0804])

            blob = patched[refs[0].data_offset : refs[0].data_offset + refs[0].data_size]
            tree = windows_pe.parse_version_tree(blob)
            sfi = next(c for c in tree.children if c.key == "StringFileInfo")
            self.assertEqual(sfi.children[0].key, "080404b0")
            vfi = next(c for c in tree.children if c.key == "VarFileInfo")
            trans = next(c for c in vfi.children if c.key == "Translation")
            self.assertEqual(struct.unpack_from("<HH", trans.value, 0), (0x0804, 0x04B0))

            expected_chk = windows_pe.compute_pe_checksum(patched, pe.checksum_offset)
            self.assertEqual(pe.original_checksum, expected_chk)
            self.assertNotEqual(pe.original_checksum, 0)
            self.assertEqual(windows_pe.main(["--check", str(exe)]), 0)
            self.assertEqual(windows_pe.main([str(exe)]), 0)

    def test_windows_pe_version_override(self) -> None:
        pck_tail = b"GDPC_TAIL_BYTES"
        pe_bytes = _build_synthetic_pe32_plus(trailing_pck=pck_tail)
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "Lwsmf.exe"
            exe.write_bytes(pe_bytes)

            rc = windows_pe.main(
                ["--file-version", "1.2.3.4", "--product-version", "5.6.7.8", str(exe)]
            )
            self.assertEqual(rc, 0)
            patched = exe.read_bytes()
            self.assertEqual(len(patched), len(pe_bytes))
            self.assertEqual(patched[-len(pck_tail) :], pck_tail)

            pe = windows_pe.parse_pe(patched)
            refs = windows_pe.find_version_resources(patched, pe)
            blob = patched[refs[0].data_offset : refs[0].data_offset + refs[0].data_size]
            tree = windows_pe.parse_version_tree(blob)
            self.assertEqual(struct.unpack_from("<IIII", tree.value, 8), (0x00010002, 0x00030004, 0x00050006, 0x00070008))

            self.assertEqual(
                windows_pe.main(
                    ["--check", "--file-version", "1.2.3.4", "--product-version", "5.6.7.8", str(exe)]
                ),
                0,
            )

    def test_windows_pe_rejects_invalid_and_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            bad = t / "not_pe.exe"
            bad.write_bytes(b"NOT_A_PE_BINARY" * 16)
            self.assertEqual(windows_pe.main([str(bad)]), 1)

            pe_bytes = _build_synthetic_pe32_plus()
            trunc = t / "truncated.exe"
            trunc.write_bytes(pe_bytes[:100])
            self.assertEqual(windows_pe.main([str(trunc)]), 1)

            missing = t / "missing.exe"
            self.assertEqual(windows_pe.main([str(missing)]), 1)

            real_exe = t / "real.exe"
            real_exe.write_bytes(pe_bytes)
            with mock.patch.object(Path, "is_symlink", return_value=True):
                self.assertEqual(windows_pe.main([str(real_exe)]), 1)

    def test_windows_pe_parse_lang_id_decimal_and_hex(self) -> None:
        self.assertEqual(windows_pe.parse_lang_id("2052"), 0x0804)
        self.assertEqual(windows_pe.parse_lang_id("0x0804"), 0x0804)
        self.assertEqual(windows_pe.parse_lang_id("1033"), 0x0409)
        self.assertEqual(windows_pe.parse_lang_id("0804"), 804)
        with self.assertRaises(argparse.ArgumentTypeError):
            windows_pe.parse_lang_id("0x10000")
        with self.assertRaises(argparse.ArgumentTypeError):
            windows_pe.parse_lang_id("abcd")

    def test_windows_pe_rejects_resource_past_end_of_file(self) -> None:
        pe_bytes = _build_synthetic_pe32_plus()
        cut = pe_bytes[:-4]
        pe = windows_pe.parse_pe(cut)
        with self.assertRaises(windows_pe.PEFormatError):
            windows_pe.find_version_resources(cut, pe)

    def test_windows_pe_rejects_duplicate_language_entries(self) -> None:
        same = _build_synthetic_pe32_plus(lang_ids=(0x0409, 0x0409))
        with self.assertRaises(windows_pe.PEFormatError):
            windows_pe.find_version_resources(same, windows_pe.parse_pe(same))

        both = _build_synthetic_pe32_plus(lang_ids=(0x0409, 0x0804))
        with tempfile.TemporaryDirectory() as tmp:
            exe = Path(tmp) / "Lwsmf.exe"
            exe.write_bytes(both)
            self.assertEqual(windows_pe.main([str(exe)]), 1)
            self.assertEqual(exe.read_bytes(), both)


if __name__ == "__main__":
    unittest.main()
