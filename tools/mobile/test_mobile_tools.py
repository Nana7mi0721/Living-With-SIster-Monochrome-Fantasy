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
TOOLS_DIR = MOBILE_DIR.parent
REPO_ROOT = TOOLS_DIR.parent
if str(MOBILE_DIR) not in sys.path:
    sys.path.insert(0, str(MOBILE_DIR))
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import android_data  # noqa: E402
import audit_cross_platform_assets  # noqa: E402
import brotli_json  # noqa: E402
import env_check  # noqa: E402
import export_preset_template  # noqa: E402
import ios_inject_data  # noqa: E402
import pack_media  # noqa: E402
import postprocess_android_apk  # noqa: E402
import release_audit  # noqa: E402
import release_tui  # noqa: E402
import strip_android_static_libs  # noqa: E402
import sync_boot_assets  # noqa: E402
import sync_release_icons  # noqa: E402
import windows_pe  # noqa: E402


def _make_fake_payload(root: Path, include_font: bool = True) -> tuple[Path, Path]:
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
    pak_entries = [("SystemFile/WindowBase.png", b"PNG_STUB")]
    if include_font:
        pak_entries.append(("Lolita.ttf", b"TTF_STUB"))
    pack_media.write_pak(pak_entries, wa / "media.pak")
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

    def test_brotli_json_android_ios_and_release_audit(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            gd, wa = _make_fake_payload(t / "src")
            staged_gd = t / "staged_gd"
            self.assertEqual(
                brotli_json.main(["--src", str(gd), "--dst", str(staged_gd), "--brotli", "--quality", "4"]),
                0,
            )
            self.assertTrue((staged_gd / "manifest.json").is_file())
            for name in ("programs.json", "database.json", "map_tiles.json"):
                self.assertFalse((staged_gd / name).exists())
                br = staged_gd / f"{name}.br"
                self.assertTrue(br.is_file())
                decoded = json.loads(brotli_json.decompress_bytes(br.read_bytes()).decode("utf-8"))
                self.assertEqual(decoded, json.loads((gd / name).read_text(encoding="utf-8")))

            # Android --brotli staging and verify-apk accept .json.br.blob entries.
            android_stage = t / "android_stage"
            rc = android_data.main(
                [
                    "stage",
                    "--version",
                    "0.2.0",
                    "--game-data",
                    str(gd),
                    "--work-all",
                    str(wa),
                    "--dest",
                    str(android_stage),
                    "--brotli",
                    "--allow-temp",
                ]
            )
            self.assertEqual(rc, 0)
            self.assertTrue((android_stage / "game_data/programs.json.br.blob").is_file())
            apk_path = t / "brotli.apk"
            _build_valid_apk(apk_path, android_stage)
            self.assertEqual(android_data.main(["verify-apk", str(apk_path)]), 0)

            # iOS --brotli injection stores .br entries without re-deflating.
            ipa_in = t / "engine.ipa"
            ipa_out = t / "full-br.ipa"
            _build_synthetic_ipa(ipa_in)
            rc = ios_inject_data.main(
                [
                    str(ipa_in),
                    "--game-data",
                    str(gd),
                    "--work-all",
                    str(wa),
                    "--out",
                    str(ipa_out),
                    "--brotli",
                ]
            )
            self.assertEqual(rc, 0)
            with zipfile.ZipFile(ipa_out, "r") as zf:
                info = zf.getinfo("Payload/Lwsmf.app/game_data/programs.json.br")
                self.assertEqual(info.compress_type, zipfile.ZIP_STORED)

            # release_audit accepts .json.br payloads cleanly.
            dist_dir = t / "dist_br"
            for rel in (
                "game/Lwsmf.exe",
                "game/Lwsmf.pck",
                "game/data_Lwsmf_windows_x86_64/Lwsmf.dll",
                "converter/cli.py",
                "converter/convert.bat",
                "docs/BASELINE.json",
                "README.txt",
                "按键说明.txt",
            ):
                p = dist_dir / rel
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text("stub", encoding="utf-8")
            shutil.copytree(staged_gd, dist_dir / "game_data")
            shutil.copytree(wa, dist_dir / "work/all")
            shutil.copy2(REPO_ROOT / "godot/tests/fixtures/media_pack_tiny.pak", dist_dir / "work/all/media.pak")
            errors, _ = release_audit.audit(dist_dir)
            self.assertEqual(errors, [], f"unexpected release_audit errors: {errors}")

    def test_ios_strip_and_inno_setup_template(self) -> None:
        ios_script = (REPO_ROOT / "tools/package_ios.sh").read_text(encoding="utf-8")
        for flag in (
            "STRIP_INSTALLED_PRODUCT=YES",
            "DEPLOYMENT_POSTPROCESSING=YES",
            "COPY_PHASE_STRIP=YES",
            "DEBUG_INFORMATION_FORMAT=dwarf",
        ):
            self.assertIn(flag, ios_script)

        iss_tmpl = (REPO_ROOT / "tools/installer/lwsmf.iss.tmpl").read_text(encoding="utf-8")
        self.assertIn("Compression=lzma2/ultra64", iss_tmpl)
        self.assertIn("SolidCompression=yes", iss_tmpl)
        self.assertIn("LZMADictionarySize=65536", iss_tmpl)
        self.assertIn("media.pak", iss_tmpl)
        self.assertIn("media_hd.pak", iss_tmpl)
        self.assertIn("nocompression", iss_tmpl)
        self.assertIn("SetupIconFile=@SETUP_ICON@", iss_tmpl)
        self.assertIn("WizardImageFile=@FINISH_POSTER@", iss_tmpl)
        self.assertIn('Source: "@SETUP_PROMO@"', iss_tmpl)
        self.assertIn("PngImage.LoadFromFile", iss_tmpl)
        self.assertIn("WizardSmallImageFile=@SETUP_SMALL@", iss_tmpl)
        self.assertIn("WizardImageStretch=yes", iss_tmpl)
        self.assertIn("WizardImageBackColor=clWhite", iss_tmpl)
        self.assertNotIn("WizardKeepAspectRatio=yes", iss_tmpl)
        self.assertIn("WelcomeBanner.Width := BannerWidth", iss_tmpl)
        self.assertIn("LayoutWelcomePage", iss_tmpl)
        self.assertIn("TextAreaHeight := ScaleY(64)", iss_tmpl)
        self.assertIn("WelcomeTitle.Font.Style := [fsBold]", iss_tmpl)
        self.assertIn("LayoutFinishPage", iss_tmpl)
        self.assertIn("PosterWidth := PosterHeight div 2", iss_tmpl)
        self.assertNotIn("WizardBackImageFile", iss_tmpl)
        self.assertIn("WizardStyle=modern", iss_tmpl)
        self.assertIn("WizardSizePercent=100,115", iss_tmpl)
        self.assertIn("DisableWelcomePage=no", iss_tmpl)
        self.assertIn("DisableReadyPage=yes", iss_tmpl)
        self.assertIn("DisableDirPage=no", iss_tmpl)
        self.assertIn("ShowLanguageDialog=no", iss_tmpl)
        self.assertIn("AppendDefaultDirName=yes", iss_tmpl)
        self.assertIn("DesktopIconCheck := TNewCheckBox.Create", iss_tmpl)
        self.assertIn("Check: ShouldCreateDesktopIcon", iss_tmpl)
        self.assertNotIn("[Tasks]", iss_tmpl)
        self.assertIn("AppPublisher=Inusuku", iss_tmpl)
        self.assertIn("Inusuku. All rights reserved.", iss_tmpl)
        self.assertIn("DefaultDirName={autopf}\\@APP_NAME@", iss_tmpl)
        self.assertNotIn("LWSMF Remake", iss_tmpl)
        self.assertNotIn('Name: "english"', iss_tmpl)
        self.assertIn("@EXTRA_LANGUAGES@", iss_tmpl)

        packaging = (REPO_ROOT / "tools/package_release.sh").read_text(encoding="utf-8")
        self.assertIn("normalize_integrity", packaging)
        self.assertIn("/setintegritylevel M", packaging)
        self.assertIn("MSYS2_ARG_CONV_EXCL='*' icacls.exe", packaging)
        self.assertIn("for _ in 1 2 3", packaging)
        self.assertIn("ChineseSimplified.isl not found", packaging)
        self.assertIn("setup_small.png", packaging)
        self.assertIn("setup_promo.png", packaging)
        self.assertIn("setup_finish_poster.png", packaging)
        self.assertIn("@FINISH_POSTER@", packaging)
        self.assertIn("@SETUP_SMALL@", packaging)
        integrity_tool = (REPO_ROOT / "tools/fix_integrity_level.ps1").read_text(encoding="utf-8")
        self.assertIn("/setintegritylevel M", integrity_tool)
        self.assertIn("for ($attempt = 1; $attempt -le 3; $attempt++)", integrity_tool)

        self.assertEqual(release_audit.repo_scan(REPO_ROOT), [])

    def test_custom_template_builder_and_profile(self) -> None:
        import importlib.util
        import zipfile

        builder_path = REPO_ROOT / "tools/templates/build_custom_template.py"
        spec = importlib.util.spec_from_file_location("build_custom_template", builder_path)
        assert spec is not None and spec.loader is not None
        builder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(builder)

        win_args = builder.build_scons_args("windows", 8)
        self.assertIn("-j8", win_args)
        self.assertIn("target=template_release", win_args)
        self.assertIn("module_mono_enabled=yes", win_args)
        self.assertIn("d3d12=no", win_args)
        self.assertIn("sdl=no", win_args)

        and_args = builder.build_scons_args("android", 4)
        self.assertIn("platform=android", and_args)
        self.assertIn("arch=arm64", and_args)
        self.assertIn("lto=thin", and_args)
        self.assertIn("generate_apk=yes", and_args)

        ios_args = builder.build_scons_args("ios", 4)
        self.assertIn("platform=ios", ios_args)
        self.assertIn("arch=arm64", ios_args)
        self.assertIn("metal=no", ios_args)

        with self.assertRaises(ValueError):
            builder.build_scons_args("linux", 2)

        profile_ns: dict[str, object] = {}
        profile_src = (REPO_ROOT / "tools/templates/custom_2d_mono.py").read_text(encoding="utf-8")
        exec(compile(profile_src, "custom_2d_mono.py", "exec"), profile_ns)
        self.assertEqual(profile_ns.get("target"), "template_release")
        self.assertEqual(profile_ns.get("module_mono_enabled"), "yes")
        self.assertEqual(profile_ns.get("vulkan"), "no")
        self.assertEqual(profile_ns.get("accesskit"), "no")
        for forbidden in ("disable_3d", "disable_physics_3d", "disable_navigation_3d"):
            self.assertNotIn(forbidden, profile_ns)

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            (bin_dir / "godot.windows.template_release.x86_64.console.exe").write_bytes(b"console")
            win_bin = bin_dir / "godot.windows.template_release.x86_64.exe"
            win_bin.write_bytes(b"win")
            self.assertEqual(builder.find_built_binary(bin_dir, "windows"), win_bin)

            and_bin = bin_dir / "android_release.apk"
            and_bin.write_bytes(b"apk")
            self.assertEqual(builder.find_built_binary(bin_dir, "android"), and_bin)

            ios_bin = bin_dir / "libgodot.ios.template_release.arm64.a"
            ios_bin.write_bytes(b"NEW_ARM64_ARCHIVE")
            self.assertEqual(builder.find_built_binary(bin_dir, "ios"), ios_bin)

            base_zip = root / "base_ios.zip"
            with zipfile.ZipFile(base_zip, "w") as zf:
                zf.writestr("ios_xcode/libgodot.ios.release.arm64.a", b"OLD_RELEASE")
                zf.writestr("ios_xcode/libgodot.ios.debug.arm64.a", b"KEEP_DEBUG")
                zf.writestr("ios_xcode/libgodot.ios.release.arm64-simulator.a", b"KEEP_SIM")
                zf.writestr("ios_xcode/Info.plist", b"KEEP_PLIST")

            out_zip = root / "out" / "ios.zip"
            replaced = builder.inject_ios_static_lib(base_zip, ios_bin, out_zip)
            self.assertEqual(replaced, 1)
            with zipfile.ZipFile(out_zip, "r") as zf:
                self.assertEqual(zf.read("ios_xcode/libgodot.ios.release.arm64.a"), b"NEW_ARM64_ARCHIVE")
                self.assertEqual(zf.read("ios_xcode/libgodot.ios.debug.arm64.a"), b"KEEP_DEBUG")
                self.assertEqual(zf.read("ios_xcode/libgodot.ios.release.arm64-simulator.a"), b"KEEP_SIM")
                self.assertEqual(zf.read("ios_xcode/Info.plist"), b"KEEP_PLIST")

            bad_zip = root / "bad_ios.zip"
            with zipfile.ZipFile(bad_zip, "w") as zf:
                zf.writestr("ios_xcode/Info.plist", b"ONLY_PLIST")
            with self.assertRaises(RuntimeError):
                builder.inject_ios_static_lib(bad_zip, ios_bin, root / "out" / "bad.zip")

            self.assertEqual(
                builder.main([
                    "--platform", "windows",
                    "--godot-src", str(root / "src"),
                    "--out", str(root / "dist"),
                    "--dry-run",
                ]),
                0,
            )

        wf_text = (REPO_ROOT / ".github/workflows/custom-templates.yml").read_text(encoding="utf-8")
        for needle in (
            "build-windows:",
            "build-android:",
            "build-ios:",
            "tools/templates/build_custom_template.py",
        ):
            self.assertIn(needle, wf_text)

        for script_rel in ("tools/package_release.sh", "tools/package_ios.sh"):
            script_text = (REPO_ROOT / script_rel).read_text(encoding="utf-8")
            self.assertIn("--template", script_text)
            self.assertIn("custom_template/release", script_text)
            self.assertIn("--format sln", script_text)

        rel_script = (REPO_ROOT / "tools/package_release.sh").read_text(encoding="utf-8")
        iss_tmpl = (REPO_ROOT / "tools/installer/lwsmf.iss.tmpl").read_text(encoding="utf-8")
        self.assertIn("--version", rel_script)
        self.assertIn("ChineseSimplified.isl", rel_script)
        self.assertIn("@EXTRA_LANGUAGES@", iss_tmpl)

    def test_font_enforcement_android_and_ios(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            gd, wa = _make_fake_payload(t / "src", include_font=False)
            dest = t / "staged"
            ipa_in = t / "engine.ipa"
            _build_synthetic_ipa(ipa_in)

            with self.assertRaises(SystemExit):
                android_data.main(["plan", "--game-data", str(gd), "--work-all", str(wa)])
            with self.assertRaises(SystemExit):
                android_data.main(
                    ["stage", "--game-data", str(gd), "--work-all", str(wa), "--dest", str(dest), "--allow-temp"]
                )
            with self.assertRaises(SystemExit):
                ios_inject_data.main(
                    [str(ipa_in), "--game-data", str(gd), "--work-all", str(wa), "--out", str(t / "fail.ipa")]
                )

            # --allow-missing-font succeeds on stage/verify-apk/ios_inject_data, while strict verify-apk fails.
            self.assertEqual(
                android_data.main(
                    [
                        "stage",
                        "--game-data",
                        str(gd),
                        "--work-all",
                        str(wa),
                        "--dest",
                        str(dest),
                        "--allow-temp",
                        "--allow-missing-font",
                    ]
                ),
                0,
            )
            apk = t / "nofont.apk"
            _build_valid_apk(apk, dest)
            self.assertEqual(android_data.main(["verify-apk", str(apk)]), 1)
            self.assertEqual(android_data.main(["verify-apk", str(apk), "--allow-missing-font"]), 0)
            self.assertEqual(
                ios_inject_data.main(
                    [
                        str(ipa_in),
                        "--game-data",
                        str(gd),
                        "--work-all",
                        str(wa),
                        "--out",
                        str(t / "allow.ipa"),
                        "--allow-missing-font",
                    ]
                ),
                0,
            )

            # Passing --font satisfies Android stage + verify-apk without media.pak containing Lolita.ttf.
            font = t / "Lolita.ttf"
            font.write_bytes(b"TTF_FONT")
            dest_font = t / "staged_font"
            self.assertEqual(
                android_data.main(
                    [
                        "stage",
                        "--game-data",
                        str(gd),
                        "--work-all",
                        str(wa),
                        "--dest",
                        str(dest_font),
                        "--font",
                        str(font),
                        "--allow-temp",
                    ]
                ),
                0,
            )
            self.assertTrue((dest_font / "work/all/Lolita.ttf.blob").is_file())
            apk_font = t / "with_font.apk"
            _build_valid_apk(apk_font, dest_font)
            self.assertEqual(android_data.main(["verify-apk", str(apk_font)]), 0)

            # Case-insensitive Data/LOLITA.TTF inside media.pak satisfies both Android and iOS.
            pak_data_font = t / "data_font.pak"
            pack_media.write_pak([("Data/LOLITA.TTF", b"TTF_PAK")], pak_data_font)
            dest_pak = t / "staged_pak_font"
            self.assertEqual(
                android_data.main(
                    [
                        "stage",
                        "--game-data",
                        str(gd),
                        "--work-all",
                        str(wa),
                        "--media-pak",
                        str(pak_data_font),
                        "--dest",
                        str(dest_pak),
                        "--allow-temp",
                    ]
                ),
                0,
            )
            apk_pak = t / "pak_font.apk"
            _build_valid_apk(apk_pak, dest_pak)
            self.assertEqual(android_data.main(["verify-apk", str(apk_pak)]), 0)
            self.assertEqual(
                ios_inject_data.main(
                    [
                        str(ipa_in),
                        "--game-data",
                        str(gd),
                        "--work-all",
                        str(wa),
                        "--media-pak",
                        str(pak_data_font),
                        "--out",
                        str(t / "pak_font.ipa"),
                    ]
                ),
                0,
            )

    def test_audit_cross_platform_assets(self) -> None:
        import io as _io
        from PIL import Image

        def png_bytes(w: int, h: int) -> bytes:
            buf = _io.BytesIO()
            Image.new("RGBA", (w, h), (10, 20, 30, 255)).save(buf, format="PNG")
            return buf.getvalue()

        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            gd = t / "game_data"
            wa = t / "work/all"
            (wa / "SystemFile").mkdir(parents=True)
            (wa / "Picture").mkdir(parents=True)
            gd.mkdir(parents=True)

            charicon_bytes = png_bytes(16, 16)
            face_bytes = png_bytes(32, 16)
            (wa / "SystemFile/charicon.png").write_bytes(charicon_bytes)
            (wa / "Picture/face01.png").write_bytes(face_bytes)
            (wa / "Lolita.ttf").write_bytes(b"TTF")

            (gd / "programs.json").write_text(
                json.dumps({"common_events": [{"slots": [{"op": 140, "strings": ["Picture/face01.png"]}]}]}),
                encoding="utf-8",
            )
            (gd / "database.json").write_text(
                json.dumps({"cdata": {"types": [{"rows": [{"strings": ["SystemFile/charicon.png"]}]}]}}),
                encoding="utf-8",
            )

            pak = wa / "media.pak"
            pack_media.write_pak(
                [
                    ("SystemFile/charicon.png", charicon_bytes),
                    ("Picture/face01.png", face_bytes),
                    ("Lolita.ttf", b"TTF"),
                ],
                pak,
            )

            hd_pak = wa / "media_hd.pak"
            hd_manifest = {"entries": {"picture/face01.png": {"path": "picture/face01.png", "scale": 2.0}}}
            pack_media.write_pak(
                [
                    ("hd_manifest.json", json.dumps(hd_manifest).encode("utf-8")),
                    ("picture/face01.png", png_bytes(64, 32)),
                ],
                hd_pak,
            )

            json_out = t / "report.json"
            rc = audit_cross_platform_assets.main(
                [
                    "--game-data",
                    str(gd),
                    "--work-all",
                    str(wa),
                    "--media-pak",
                    str(pak),
                    "--hd-pak",
                    str(hd_pak),
                    "--json-out",
                    str(json_out),
                ]
            )
            self.assertEqual(rc, 0)
            rep = json.loads(json_out.read_text(encoding="utf-8"))
            self.assertTrue(rep["ok"])
            self.assertEqual(rep["summary"]["hd_checked"], 1)
            self.assertEqual(rep["summary"]["hd_mismatches"], 0)

            # Corrupt HD aspect ratio (64x48 instead of 64x32) fails.
            pack_media.write_pak(
                [
                    ("hd_manifest.json", json.dumps(hd_manifest).encode("utf-8")),
                    ("picture/face01.png", png_bytes(64, 48)),
                ],
                hd_pak,
            )
            self.assertEqual(
                audit_cross_platform_assets.main(
                    ["--game-data", str(gd), "--work-all", str(wa), "--media-pak", str(pak), "--hd-pak", str(hd_pak)]
                ),
                1,
            )

            # Referenced existing loose SystemFile/charicon.png missing from media.pak fails even with --allow-missing-references.
            pack_media.write_pak([("Picture/face01.png", face_bytes), ("Lolita.ttf", b"TTF")], pak)
            self.assertEqual(
                audit_cross_platform_assets.main(
                    [
                        "--game-data",
                        str(gd),
                        "--work-all",
                        str(wa),
                        "--media-pak",
                        str(pak),
                        "--allow-missing-references",
                    ]
                ),
                1,
            )

            # Basename determinism mismatch (loose Picture/dup.png vs pak SystemGraphic/dup.png) fails.
            (wa / "Picture/dup.png").write_bytes(charicon_bytes)
            pack_media.write_pak(
                [
                    ("SystemFile/charicon.png", charicon_bytes),
                    ("Picture/face01.png", face_bytes),
                    ("SystemGraphic/dup.png", charicon_bytes),
                    ("Lolita.ttf", b"TTF"),
                ],
                pak,
            )
            self.assertEqual(
                audit_cross_platform_assets.main(
                    ["--game-data", str(gd), "--work-all", str(wa), "--media-pak", str(pak)]
                ),
                1,
            )

    def test_sync_boot_assets_pillow_and_stdlib_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            # 1024x576 black splash with a centered 200x200 white square logo.
            w, h = 1024, 576
            rgba = bytearray(w * h * 4)
            for i in range(3, len(rgba), 4):
                rgba[i] = 255
            for y in range(188, 388):
                for x in range(412, 612):
                    off = (y * w + x) * 4
                    rgba[off] = 240
                    rgba[off + 1] = 240
                    rgba[off + 2] = 240
            src = t / "splash_static.png"
            sync_boot_assets.write_png_rgba(src, w, h, rgba)

            for label, use_pillow in (("pillow", True), ("stdlib", False)):
                out_dir = t / label
                outputs = sync_boot_assets.sync_boot_assets(src, out_dir=out_dir, use_pillow=use_pillow)
                self.assertEqual(len(outputs), 3)
                icon_path = out_dir / "android_icon_192.png"
                fg_path = out_dir / "android_adaptive_fg_432.png"
                bg_path = out_dir / "android_adaptive_bg_432.png"
                self.assertEqual(sync_boot_assets.read_png_dimensions(icon_path), (192, 192))
                self.assertEqual(sync_boot_assets.read_png_dimensions(fg_path), (432, 432))
                self.assertEqual(sync_boot_assets.read_png_dimensions(bg_path), (432, 432))

                # Adaptive foreground keeps corners transparent outside the 62% safe zone
                # and renders the centered logo at the center pixel.
                fg_w, fg_h, fg_rgba = sync_boot_assets.read_png_rgba(fg_path)
                self.assertEqual((fg_w, fg_h), (432, 432))
                self.assertEqual(fg_rgba[3], 0, f"{label}: top-left corner must be transparent")
                center_off = ((216 * 432) + 216) * 4
                self.assertGreater(fg_rgba[center_off], 200, f"{label}: center logo pixel missing")
                self.assertEqual(fg_rgba[center_off + 3], 255)

                # Adaptive background must be pure black #000000.
                bg_w, bg_h, bg_rgba = sync_boot_assets.read_png_rgba(bg_path)
                self.assertEqual((bg_w, bg_h), (432, 432))
                self.assertEqual(tuple(bg_rgba[:4]), (0, 0, 0, 255))

            # Repo placeholder assets must exist with exact dimensions.
            boot_dir = REPO_ROOT / "godot/assets/boot"
            self.assertEqual(sync_boot_assets.read_png_dimensions(boot_dir / "splash_static.png"), (1024, 576))
            self.assertEqual(sync_boot_assets.read_png_dimensions(boot_dir / "android_icon_192.png"), (192, 192))
            self.assertEqual(sync_boot_assets.read_png_dimensions(boot_dir / "android_adaptive_fg_432.png"), (432, 432))
            self.assertEqual(sync_boot_assets.read_png_dimensions(boot_dir / "android_adaptive_bg_432.png"), (432, 432))

    def test_ios_inject_streams_without_full_read_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            gd, wa = _make_fake_payload(t / "src")
            # Build a multi-MB media.pak (> 2x STREAM_CHUNK_BYTES) to exercise multi-chunk streaming.
            big_pak = t / "big_media.pak"
            pack_media.write_pak(
                [
                    ("Lolita.ttf", b"TTF_STUB"),
                    ("Picture/large.bin", b"\x5a" * (2 * 1024 * 1024 + 137)),
                ],
                big_pak,
            )
            ipa_in = t / "engine.ipa"
            ipa_out = t / "streamed.ipa"
            _build_synthetic_ipa(ipa_in)

            orig_read_bytes = Path.read_bytes

            def guarded_read_bytes(self_path: Path) -> bytes:
                if self_path.resolve() in (big_pak.resolve(), ipa_out.resolve()):
                    raise AssertionError(f"streaming violated: full read_bytes() called on {self_path.name}")
                return orig_read_bytes(self_path)

            with mock.patch.object(Path, "read_bytes", guarded_read_bytes):
                ios_inject_data.inject(
                    ipa=ipa_in,
                    game_data_dir=gd,
                    work_all_dir=wa,
                    output=ipa_out,
                    media_pak_override=big_pak,
                    font=None,
                    force=True,
                )

            self.assertTrue(ipa_out.is_file())
            with zipfile.ZipFile(ipa_out, "r") as zf:
                info = zf.getinfo("Payload/Lwsmf.app/work/all/media.pak")
                self.assertEqual(info.compress_type, zipfile.ZIP_STORED)
                self.assertEqual(info.file_size, big_pak.stat().st_size)
                self.assertIsNone(zf.testzip())

            # Verify chunked sha256_file matches stdlib hashlib.
            import hashlib as _hashlib

            expected_sha = _hashlib.sha256(orig_read_bytes(ipa_out)).hexdigest()
            self.assertEqual(ios_inject_data.sha256_file(ipa_out), expected_sha)

    def test_boot_splash_and_packaging_configs(self) -> None:
        proj_text = (REPO_ROOT / "godot/project.godot").read_text(encoding="utf-8")
        for line in (
            "boot_splash/bg_color=Color(0, 0, 0, 1)",
            'boot_splash/image="res://assets/boot/splash_static.png"',
            "boot_splash/fullsize=false",
            "boot_splash/stretch_mode=1",
            "boot_splash/use_filter=true",
            "boot_splash/show_image=true",
            "boot_splash/minimum_display_time=0",
        ):
            self.assertIn(line, proj_text)

        presets = env_check.parse_export_presets(REPO_ROOT / "godot/export_presets.cfg")
        self.assertEqual(presets["Android"]["launcher_icons/main_192x192"], "res://assets/boot/generated/android_icon_192.png")
        self.assertEqual(
            presets["Android"]["launcher_icons/adaptive_foreground_432x432"],
            "res://assets/boot/generated/android_adaptive_fg_432.png",
        )
        self.assertEqual(
            presets["Android"]["launcher_icons/adaptive_background_432x432"],
            "res://assets/boot/generated/android_adaptive_bg_432.png",
        )
        self.assertEqual(presets["Android"]["package/name"], "妹相随")
        self.assertEqual(presets["Windows Desktop"]["application/icon"], "res://assets/boot/generated/icon_pc.ico")
        self.assertEqual(presets["Windows Desktop"]["application/company_name"], "Inusuku")
        self.assertEqual(presets["Windows Desktop"]["application/product_name"], "妹相随～黑白世界的缤纷冒险～")
        self.assertEqual(presets["Windows Desktop"]["application/file_description"], "妹相随～黑白世界的缤纷冒险～")
        self.assertIn("Inusuku", presets["Windows Desktop"]["application/copyright"])
        self.assertIn("assets/boot/generated/*.bin", presets["Windows Desktop"]["include_filter"])
        self.assertIn("assets/boot/generated/icon_pc_taskbar.png", presets["Windows Desktop"]["include_filter"])
        self.assertNotIn("assets/boot/generated/*.png", presets["Windows Desktop"]["exclude_filter"])
        self.assertEqual(presets["iOS"]["icons/icon_1024x1024"], "res://assets/boot/generated/ios_icon_1024.png")
        self.assertIn("CFBundleDisplayName", presets["iOS"]["application/additional_plist_content"])
        self.assertIn("妹相随", presets["iOS"]["application/additional_plist_content"])
        self.assertEqual(presets["iOS"]["entitlements/increased_memory_limit"], "true")
        self.assertEqual(presets["iOS"]["storyboard/use_launch_screen_storyboard"], "true")
        self.assertEqual(presets["iOS"]["storyboard/image_scale_mode"], "1")
        self.assertEqual(presets["iOS"]["storyboard/custom_image@2x"], "res://assets/boot/splash_static.png")
        self.assertEqual(presets["iOS"]["storyboard/custom_image@3x"], "res://assets/boot/splash_static.png")
        self.assertEqual(presets["iOS"]["storyboard/use_custom_bg_color"], "true")
        self.assertEqual(presets["iOS"]["storyboard/custom_bg_color"], "Color(0, 0, 0, 1)")

        ios_script = (REPO_ROOT / "tools/package_ios.sh").read_text(encoding="utf-8")
        self.assertIn("SplashImage.imageset", ios_script)
        self.assertIn("LWSMF-${VERSION}-ios-arm64.ipa", ios_script)
        self.assertIn("--allow-placeholder", ios_script)

        rel_script = (REPO_ROOT / "tools/package_release.sh").read_text(encoding="utf-8")
        self.assertIn("if pck.is_file()", rel_script)
        self.assertIn("PKG_NAME=\"LWSMF-${VERSION}-windows-x64\"", rel_script)
        self.assertIn("sync_release_icons.py", rel_script)
        self.assertIn("--lossless", rel_script)
        self.assertIn("building true-lossless media.pak", rel_script)
        self.assertIn("--extra-root", rel_script)
        self.assertIn("work/out/inno/ChineseSimplified.isl", rel_script)
        self.assertIn("icon_pc_taskbar.png", rel_script)
        self.assertIn("PROJECT_BAK", rel_script)
        self.assertIn('config/icon="res://assets/boot/generated/icon_pc_taskbar.png"', rel_script)

        android_script = (REPO_ROOT / "tools/package_android.sh").read_text(encoding="utf-8")
        self.assertIn("LWSMF-${VERSION}-android-arm64${SUFFIX}.apk", android_script)
        self.assertIn("APP_LABEL=\"妹相随\"", android_script)
        self.assertIn("new sln --format sln", android_script)
        self.assertIn('|| "$DOTNET_BIN" new sln', android_script)
        self.assertIn("timeout 30", android_script)
        self.assertIn("ANDROID_DOTNET", android_script)
        self.assertIn('export DOTNET="$DOTNET_BIN"', android_script)
        self.assertIn("ANDROID_DOTNET_ROOT", android_script)

        game_root = (REPO_ROOT / "godot/presentation_gd/GameRoot.gd").read_text(encoding="utf-8")
        self.assertIn("icon_pc_dockbar.bin", game_root)
        self.assertIn("icon_pc_corner.bin", game_root)
        self.assertNotIn("icon_pc_dockbar.ico", game_root)
        self.assertNotIn("icon_pc_corner.ico", game_root)
        window_icons = (REPO_ROOT / "godot/presentation/WindowIcons.cs").read_text(encoding="utf-8")
        self.assertIn("IconSmall2", window_icons)
        self.assertIn("WmSetIcon, new IntPtr(IconSmall2)", window_icons)
        self.assertIn("_taskbarIcon", window_icons)
        self.assertIn("CreateIcon(dockbarIco, smallWidth, smallHeight)", window_icons)
        self.assertIn("_refresh_windows_window_icons.call_deferred", game_root)

    def test_sync_boot_assets_from_retail_logo_and_pak(self) -> None:
        import io as _io
        from PIL import Image

        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            buf = _io.BytesIO()
            img = Image.new("RGB", (64, 36), (0, 0, 0))
            for y in range(12, 24):
                for x in range(24, 40):
                    img.putpixel((x, y), (240, 240, 240))
            img.save(buf, format="JPEG")
            jpg_bytes = buf.getvalue()

            # 1. Derive directly from media.pak when loose logo.jpg is absent.
            pak = t / "media.pak"
            pack_media.write_pak([("SystemFile/logo.jpg", jpg_bytes)], pak)
            out_pak = t / "out_pak"
            rc = sync_boot_assets.main(
                ["--from-retail", "--retail-logo", str(t / "missing_logo.jpg"), "--media-pak", str(pak), "--out-dir", str(out_pak)]
            )
            self.assertEqual(rc, 0)
            self.assertEqual(sync_boot_assets.read_png_dimensions(out_pak / "splash_static.png"), (64, 36))
            self.assertEqual(sync_boot_assets.read_png_dimensions(out_pak / "android_icon_192.png"), (192, 192))
            self.assertEqual(sync_boot_assets.read_png_dimensions(out_pak / "android_adaptive_fg_432.png"), (432, 432))
            self.assertEqual(sync_boot_assets.read_png_dimensions(out_pak / "android_adaptive_bg_432.png"), (432, 432))

            # 2. Loose retail SystemFile/logo.jpg takes precedence when present.
            loose_logo = t / "logo.jpg"
            loose_logo.write_bytes(jpg_bytes)
            out_loose = t / "out_loose"
            rc2 = sync_boot_assets.main(
                ["--from-retail", "--retail-logo", str(loose_logo), "--out-dir", str(out_loose)]
            )
            self.assertEqual(rc2, 0)
            self.assertEqual(sync_boot_assets.read_png_dimensions(out_loose / "splash_static.png"), (64, 36))

    def test_sync_release_icons_and_installer_artwork(self) -> None:
        from PIL import Image

        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            source = t / "publish"
            source.mkdir()
            mobile = Image.new("RGB", (64, 64), (24, 26, 32))
            mobile.save(source / "icon_mobile.jpg", format="JPEG")
            for name in ("icon_pc.png", "icon_pc_dockbar.png", "icon_pc_corner.png"):
                img = Image.new("RGBA", (512, 512), (0, 0, 0, 0))
                img.paste((220, 224, 232, 255), (96, 96, 416, 416))
                img.save(source / name, format="PNG")
            Image.new("RGB", (164, 314), (80, 120, 180)).save(source / "setup_poster.png", format="PNG")
            Image.new("RGB", (1536, 864), (80, 120, 180)).save(source / "sisfan_promo.png", format="PNG")
            Image.new("RGB", (520, 1041), (80, 120, 180)).save(source / "TitleGraphic6.jpg", format="JPEG")

            out = t / "out"
            stage = t / "stage"
            rc = sync_release_icons.main(
                [
                    "--source-dir",
                    str(source),
                    "--out-dir",
                    str(out),
                    "--stage-res",
                    str(stage),
                ]
            )
            self.assertEqual(rc, 0)
            self.assertEqual(sync_boot_assets.read_png_dimensions(out / "res/android_icon_192.png"), (192, 192))
            self.assertEqual(sync_boot_assets.read_png_dimensions(out / "res/android_adaptive_fg_432.png"), (432, 432))
            self.assertEqual(sync_boot_assets.read_png_dimensions(out / "res/android_adaptive_bg_432.png"), (432, 432))
            self.assertEqual(sync_boot_assets.read_png_dimensions(out / "res/ios_icon_1024.png"), (1024, 1024))
            self.assertEqual(sync_boot_assets.read_png_dimensions(out / "res/icon_pc_taskbar.png"), (256, 256))
            with Image.open(out / "res/android_adaptive_fg_432.png") as fg:
                bbox = fg.getchannel("A").getbbox()
                self.assertLessEqual(bbox[0], 72)
                self.assertLessEqual(bbox[1], 72)
                self.assertGreaterEqual(bbox[2], 360)
                self.assertGreaterEqual(bbox[3], 360)
            with Image.open(out / "res/android_adaptive_bg_432.png") as bg:
                self.assertGreaterEqual(min(bg.convert("RGB").getpixel((0, 0))), 220)
            self.assertEqual(sync_boot_assets.read_png_dimensions(out / "installer/setup_poster.png"), (164, 314))
            self.assertEqual(sync_boot_assets.read_png_dimensions(out / "installer/setup_promo.png"), (1536, 864))
            with Image.open(out / "installer/setup_finish_poster.png") as finish_poster:
                self.assertEqual(finish_poster.size, (520, 1041))
            self.assertEqual(sync_boot_assets.read_png_dimensions(out / "installer/setup_small.png"), (55, 55))
            self.assertEqual(
                (out / "installer/setup_poster.png").read_bytes(),
                (source / "setup_poster.png").read_bytes(),
            )
            with Image.open(out / "installer/setup_small.png") as small:
                self.assertEqual(small.convert("RGBA").getpixel((0, 0)), (255, 255, 255, 255))
            self.assertEqual(
                (out / "installer/setup_promo.png").read_bytes(),
                (source / "sisfan_promo.png").read_bytes(),
            )
            with Image.open(source / "TitleGraphic6.jpg") as finish_source, Image.open(
                out / "installer/setup_finish_poster.png"
            ) as finish_poster:
                self.assertEqual(finish_poster.convert("RGB").tobytes(), finish_source.convert("RGB").tobytes())
            self.assertFalse((out / "installer/setup_finish_poster.jpg").exists())
            for name in ("icon_pc.ico", "icon_pc_dockbar.ico", "icon_pc_corner.ico"):
                with Image.open(out / "res" / name) as ico:
                    self.assertIn((16, 16), ico.info["sizes"])
                    self.assertIn((256, 256), ico.info["sizes"])
            self.assertEqual(
                (stage / "icon_pc_dockbar.ico").read_bytes(),
                (out / "res/icon_pc_dockbar.ico").read_bytes(),
            )
            self.assertEqual(
                (stage / "icon_pc_taskbar.png").read_bytes(),
                (out / "res/icon_pc_taskbar.png").read_bytes(),
            )
            for name in ("icon_pc_dockbar.bin", "icon_pc_corner.bin"):
                self.assertEqual(
                    (stage / name).read_bytes(),
                    (out / "res" / name).read_bytes(),
                )
                self.assertEqual(
                    (out / "res" / name).read_bytes(),
                    (out / "res" / name.replace(".bin", ".ico")).read_bytes(),
                )
            manifest = json.loads((out / "manifest.json").read_text(encoding="utf-8"))
            self.assertFalse(manifest["placeholder"])

            placeholder_out = t / "placeholder"
            rc2 = sync_release_icons.main(
                [
                    "--source-dir",
                    str(t / "missing"),
                    "--out-dir",
                    str(placeholder_out),
                    "--allow-placeholder",
                ]
            )
            self.assertEqual(rc2, 0)
            manifest2 = json.loads((placeholder_out / "manifest.json").read_text(encoding="utf-8"))
            self.assertTrue(manifest2["placeholder"])

    def test_pack_media_lossless_mode_preserves_pixels(self) -> None:
        from io import BytesIO
        from PIL import Image

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "assets"
            (root / "Picture").mkdir(parents=True)
            (root / "MapChip").mkdir(parents=True)
            extra = Path(tmp) / "extra"
            (extra / "SystemFile").mkdir(parents=True)
            (extra / "Picture").mkdir(parents=True)

            source = Image.new("RGBA", (512, 512), (7, 11, 13, 0))
            for y in range(512):
                for x in range(512):
                    if (x - 256) ** 2 + (y - 256) ** 2 < 220**2:
                        source.putpixel((x, y), ((x * 3) % 256, (y * 5) % 256, (x + y) % 256, 255))
            source.save(root / "Picture/hero.png", format="PNG")

            jpg_bytes = BytesIO()
            Image.new("RGB", (64, 64), (80, 120, 180)).save(jpg_bytes, format="JPEG", quality=90)
            (root / "Picture/background.jpg").write_bytes(jpg_bytes.getvalue())
            Image.new("RGB", (32, 32), (20, 30, 40)).save(root / "MapChip/unused.png", format="PNG")
            Image.new("RGB", (16, 16), (40, 50, 60)).save(extra / "SystemFile/extra.png", format="PNG")
            Image.new("RGB", (16, 16), (250, 10, 20)).save(extra / "Picture/hero.png", format="PNG")

            pak = Path(tmp) / "media-lossless.pak"
            rc = pack_media.main(
                [
                    "--root",
                    str(root),
                    "--extra-root",
                    str(extra),
                    "--out",
                    str(pak),
                    "--lossless",
                    "--jobs",
                    "1",
                ]
            )
            self.assertEqual(rc, 0)

            entries = dict(pack_media.read_pak_entries(pak))
            self.assertNotIn("MapChip/unused.png", entries)
            self.assertEqual(entries["Picture/background.jpg"], jpg_bytes.getvalue())
            self.assertEqual(entries["Picture/hero.png"][:4], b"RIFF")
            self.assertEqual(entries["Picture/hero.png"][8:12], b"WEBP")
            self.assertIn("SystemFile/extra.png", entries)
            with Image.open(BytesIO(entries["Picture/hero.png"])) as decoded:
                self.assertEqual(decoded.convert("RGBA").tobytes(), source.tobytes())

    def test_verify_apk_rejects_leaked_test_assets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            gd, wa = _make_fake_payload(t, include_font=True)
            staged = t / "staged"
            rc = android_data.main(
                ["stage", "--version", "0.1.0", "--game-data", str(gd), "--work-all", str(wa), "--dest", str(staged), "--allow-temp"]
            )
            self.assertEqual(rc, 0)
            apk = t / "leaked_tests.apk"
            _build_valid_apk(apk, staged)
            with zipfile.ZipFile(apk, "a") as zf:
                zf.writestr("assets/tests/smoke.tscn", "[gd_scene]\n")
            self.assertNotEqual(android_data.main(["verify-apk", str(apk)]), 0)

    def test_pack_media_prune_game_data_and_lossless_idempotent(self) -> None:
        from io import BytesIO
        from PIL import Image

        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            gd = t / "game_data"
            wa = t / "work/all"
            gd.mkdir(parents=True)
            (wa / "Picture").mkdir(parents=True)
            (wa / "EnemyGraphic").mkdir(parents=True)
            (wa / "SystemFile").mkdir(parents=True)
            (wa / "MapChip").mkdir(parents=True)

            live_img = Image.new("RGBA", (128, 128), (12, 34, 56, 255))
            live_img.save(wa / "Picture/live.png", format="PNG")
            Image.new("RGB", (64, 64), (99, 0, 0)).save(wa / "EnemyGraphic/dead.png", format="PNG")
            Image.new("RGB", (32, 32), (10, 20, 30)).save(wa / "SystemFile/ui.png", format="PNG")
            Image.new("RGB", (32, 32), (40, 50, 60)).save(wa / "MapChip/tile.png", format="PNG")

            (gd / "programs.json").write_text(
                json.dumps({"common_events": [{"slots": [{"op": 150, "strings": ["Picture/live.png"]}]}]}),
                encoding="utf-8",
            )
            (gd / "database.json").write_text(json.dumps({}), encoding="utf-8")
            (gd / "map_tiles.json").write_text(json.dumps({}), encoding="utf-8")
            (gd / "manifest.json").write_text(json.dumps({"assets": []}), encoding="utf-8")

            pak1 = t / "pruned.pak"
            rc1 = pack_media.main(
                [
                    "--root",
                    str(wa),
                    "--prune-game-data",
                    str(gd),
                    "--lossless",
                    "--out",
                    str(pak1),
                    "--jobs",
                    "1",
                ]
            )
            self.assertEqual(rc1, 0)
            entries1 = dict(pack_media.read_pak_entries(pak1))
            self.assertIn("Picture/live.png", entries1)
            self.assertIn("SystemFile/ui.png", entries1)
            self.assertNotIn("EnemyGraphic/dead.png", entries1)
            self.assertNotIn("MapChip/tile.png", entries1)
            with Image.open(BytesIO(entries1["Picture/live.png"])) as dec:
                self.assertEqual(dec.convert("RGBA").tobytes(), live_img.tobytes())

            # Repacking an already-lossless pak stays byte-identical on WebP entries.
            pak2 = t / "repacked.pak"
            rc2 = pack_media.main(["--from-pak", str(pak1), "--lossless", "--out", str(pak2), "--jobs", "1"])
            self.assertEqual(rc2, 0)
            self.assertEqual(dict(pack_media.read_pak_entries(pak2)), entries1)

    def test_release_tui_config_and_platform_plans(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            cfg_file = t / "cfg.json"
            cfg = release_tui.ReleaseConfig(version="0.2.0", media_mode="lossless", brotli=True, win_installer=True)
            release_tui.save_config(cfg, cfg_file)
            loaded = release_tui.load_config(cfg_file)
            self.assertEqual(loaded.version, "0.2.0")
            self.assertEqual(loaded.media_mode, "lossless")

            # Invalid media_mode falls back to lossless (never lossy slim).
            cfg_file.write_text(json.dumps({"version": "0.1.0", "media_mode": "slim"}), encoding="utf-8")
            sanitized = release_tui.load_config(cfg_file)
            self.assertEqual(sanitized.media_mode, "lossless")

            win_tpl = t / "windows_release_x86_64.exe"
            and_tpl = t / "android_release.apk"
            ipa_in = t / "LWSMF-0.2.0-ios-arm64.ipa"
            for f in (win_tpl, and_tpl, ipa_in):
                f.write_bytes(b"stub")

            fake_env = {
                "bash": "bash",
                "godot": "/fake/godot",
                "templates_dir": t,
                "dotnet_win": "/fake/dotnet8",
                "dotnet_win_vers": ["8.0.428"],
                "dotnet_android": "/fake/dotnet9",
                "dotnet_android_vers": ["9.0.321"],
                "java": "/fake/java",
                "keytool": "/fake/keytool",
                "android_sdk": "/fake/sdk",
                "debug_keystore": True,
                "iscc": t / "ISCC.exe",
                "chinese_isl": t / "ChineseSimplified.isl",
                "custom_templates": {"windows": win_tpl, "android": and_tpl, "ios": None},
                "publish_dir": t / "_publish",
                "publish_present": list(release_tui.PUBLISH_FILES),
                "publish_missing": [],
                "media_pak": {"exists": True, "bytes": 1024, "entries": 10, "has_mapchip": False, "error": ""},
                "game_data_ok": True,
                "projects_ok": True,
                "font_path": t / "Lolita.ttf",
                "hd_ready": False,
                "loose_media_ready": True,
                "pillow_ok": True,
                "ios_engine_ipas": [ipa_in],
            }
            cfg.ios_engine_ipa = str(ipa_in)

            win_cmd, win_env, win_warns = release_tui.build_platform_plan("windows", cfg, fake_env, REPO_ROOT)
            self.assertIn("--lossless", win_cmd)
            self.assertIn("--brotli", win_cmd)
            self.assertIn("--installer", win_cmd)
            self.assertIn("--template", win_cmd)
            self.assertNotIn("--slim", win_cmd)
            self.assertEqual(win_warns, [])
            self.assertEqual(win_env["DOTNET_BIN"], "/fake/dotnet8")

            and_cmd, and_env, and_warns = release_tui.build_platform_plan("android", cfg, fake_env, REPO_ROOT)
            self.assertIn("--lossless", and_cmd)
            self.assertIn("--brotli", and_cmd)
            self.assertIn("--template", and_cmd)
            self.assertNotIn("--slim", and_cmd)
            self.assertEqual(and_warns, [])
            self.assertEqual(and_env["ANDROID_DOTNET"], "/fake/dotnet9")

            ios_cmd, _, ios_warns = release_tui.build_platform_plan("ios", cfg, fake_env, REPO_ROOT)
            self.assertIn("--brotli", ios_cmd)
            self.assertIn("--font", ios_cmd)
            self.assertEqual(ios_warns, [])
            self.assertEqual(release_tui.validate_target_readiness("windows", cfg, fake_env), [])
            self.assertEqual(release_tui.validate_target_readiness("android", cfg, fake_env), [])
            self.assertEqual(release_tui.validate_target_readiness("ios", cfg, fake_env), [])


if __name__ == "__main__":
    unittest.main()
