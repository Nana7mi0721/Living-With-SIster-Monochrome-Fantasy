#!/usr/bin/env python3
"""Localize Windows PE RT_VERSION (VS_VERSIONINFO) language IDs and update PE CheckSum.

Godot Windows exports write RT_VERSION resources with US English (0x0409)
language identifiers. This pure-stdlib utility patches the PE resource directory,
StringFileInfo StringTable keys (e.g. 040904b0 -> 080404b0), and VarFileInfo
Translation pairs (0x0409 -> 0x0804), then recomputes the PE Optional Header
CheckSum without shifting sections or touching an appended Godot PCK.
"""
from __future__ import annotations

import argparse
import os
import shutil
import struct
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

RT_VERSION = 16
DEFAULT_SOURCE_LANG = 0x0409
DEFAULT_TARGET_LANG = 0x0804
PE32_MAGIC = 0x10B
PE32_PLUS_MAGIC = 0x20B
VS_FFI_SIGNATURE = 0xFEEF04BD
IMAGE_RESOURCE_NAME_IS_STRING = 0x80000000
IMAGE_RESOURCE_DATA_IS_DIRECTORY = 0x80000000


class PEFormatError(ValueError):
    """Raised when an input file is not a valid PE executable with RT_VERSION."""


@dataclass
class PEInfo:
    e_lfanew: int
    magic: int
    checksum_offset: int
    original_checksum: int
    rsrc_dir_rva: int
    rsrc_dir_size: int
    sections: list[tuple[int, int, int, int]]

    def rva_to_file_offset(self, rva: int, size: int = 1) -> int:
        for va, vsz, raw_ptr, raw_size in self.sections:
            span = max(vsz, raw_size)
            if va <= rva < va + span:
                rel = rva - va
                if raw_ptr == 0 or rel + size > raw_size:
                    raise PEFormatError(f"RVA {rva:#x} (size={size}) exceeds raw section bounds")
                return raw_ptr + rel
        raise PEFormatError(f"RVA {rva:#x} does not map to any PE section")


@dataclass
class VersionResourceRef:
    dir_offset: int
    lang_entry_offset: int
    lang_id: int
    data_entry_offset: int
    data_rva: int
    data_offset: int
    data_size: int


@dataclass
class VersionNode:
    start_off: int
    end_off: int
    key_off: int
    key: str
    w_type: int
    w_value_len: int
    value_off: int
    value_bytes_len: int
    value: bytes
    children: list[VersionNode] = field(default_factory=list)


@dataclass
class PatchResult:
    file_size: int
    source_lang: int
    target_lang: int
    original_langs: list[int]
    patched_dir_entries: int
    patched_string_tables: int
    patched_translations: int
    patched_versions: int
    original_checksum: int
    new_checksum: int
    changed: bool
    check_issues: list[str] = field(default_factory=list)


def align4(value: int) -> int:
    return (value + 3) & ~3


def parse_lang_id(raw: str) -> int:
    text = raw.strip()
    base = 16 if text.lower().startswith("0x") or (len(text) == 4 and all(c in "0123456789abcdefABCDEF" for c in text)) else 10
    try:
        val = int(text, base)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"invalid language ID: {raw!r}") from exc
    if not (0 <= val <= 0xFFFF):
        raise argparse.ArgumentTypeError(f"language ID out of uint16 range: {raw!r}")
    return val


def parse_version_quad(raw: str) -> tuple[int, int, int, int]:
    parts = raw.strip().split(".")
    if len(parts) != 4:
        raise ValueError(f"version must have 4 dot-separated components (got {raw!r})")
    nums: list[int] = []
    for part in parts:
        val = int(part, 10)
        if not (0 <= val <= 0xFFFF):
            raise ValueError(f"version component out of uint16 range in {raw!r}")
        nums.append(val)
    return nums[0], nums[1], nums[2], nums[3]


def compute_pe_checksum(data: bytes | bytearray, checksum_offset: int) -> int:
    """Compute standard Windows PE CheckSum (carry-folded uint16 sum + file size)."""
    if checksum_offset < 0 or checksum_offset + 4 > len(data):
        raise PEFormatError("checksum offset out of range")

    w0, w1 = struct.unpack_from("<HH", data, checksum_offset)
    even_len = len(data) & ~1
    mv = memoryview(data)[:even_len]
    if sys.byteorder == "little":
        total = sum(mv.cast("H"))
    else:
        total = sum(w for (w,) in struct.iter_unpack("<H", mv))
    if len(data) & 1:
        total += data[-1]
    total -= w0 + w1

    while total > 0xFFFF:
        total = (total & 0xFFFF) + (total >> 16)
    return (total + len(data)) & 0xFFFFFFFF


def parse_pe(data: bytes | bytearray) -> PEInfo:
    if len(data) < 64:
        raise PEFormatError("file too small to be a PE executable")
    if data[0:2] != b"MZ":
        raise PEFormatError("missing MZ DOS header signature")

    e_lfanew = struct.unpack_from("<I", data, 0x3C)[0]
    if e_lfanew < 64 or e_lfanew + 24 > len(data):
        raise PEFormatError(f"invalid e_lfanew offset: {e_lfanew:#x}")
    if data[e_lfanew : e_lfanew + 4] != b"PE\x00\x00":
        raise PEFormatError("missing PE\\0\\0 signature")

    coff_off = e_lfanew + 4
    num_sections = struct.unpack_from("<H", data, coff_off + 2)[0]
    opt_size = struct.unpack_from("<H", data, coff_off + 16)[0]
    if num_sections == 0:
        raise PEFormatError("PE has zero sections")

    opt_off = coff_off + 20
    if opt_off + opt_size > len(data) or opt_size < 96:
        raise PEFormatError(f"invalid Optional Header size: {opt_size}")

    magic = struct.unpack_from("<H", data, opt_off)[0]
    if magic == PE32_MAGIC:
        num_rva_off = opt_off + 92
        data_dir_off = opt_off + 96
    elif magic == PE32_PLUS_MAGIC:
        num_rva_off = opt_off + 108
        data_dir_off = opt_off + 112
    else:
        raise PEFormatError(f"unsupported PE Optional Header magic: {magic:#x}")

    if opt_off + opt_size < data_dir_off + 24:
        raise PEFormatError("Optional Header too small for IMAGE_DIRECTORY_ENTRY_RESOURCE")

    num_rva = struct.unpack_from("<I", data, num_rva_off)[0]
    if num_rva <= 2:
        raise PEFormatError("PE has no IMAGE_DIRECTORY_ENTRY_RESOURCE (DataDirectory[2])")

    checksum_offset = opt_off + 64
    original_checksum = struct.unpack_from("<I", data, checksum_offset)[0]
    rsrc_dir_rva, rsrc_dir_size = struct.unpack_from("<II", data, data_dir_off + 2 * 8)
    if rsrc_dir_rva == 0 or rsrc_dir_size == 0:
        raise PEFormatError("PE resource directory DataDirectory[2] is empty")

    sec_table_off = opt_off + opt_size
    if sec_table_off + num_sections * 40 > len(data):
        raise PEFormatError("PE section header table truncated")

    sections: list[tuple[int, int, int, int]] = []
    for i in range(num_sections):
        off = sec_table_off + i * 40
        vsz, va, raw_size, raw_ptr = struct.unpack_from("<IIII", data, off + 8)
        sections.append((va, vsz, raw_ptr, raw_size))

    return PEInfo(
        e_lfanew=e_lfanew,
        magic=magic,
        checksum_offset=checksum_offset,
        original_checksum=original_checksum,
        rsrc_dir_rva=rsrc_dir_rva,
        rsrc_dir_size=rsrc_dir_size,
        sections=sections,
    )


def _read_resource_dir_entries(
    data: bytes | bytearray, dir_offset: int
) -> tuple[int, int, list[tuple[int, int, int]]]:
    if dir_offset < 0 or dir_offset + 16 > len(data):
        raise PEFormatError(f"resource directory out of bounds: {dir_offset:#x}")
    num_named, num_id = struct.unpack_from("<HH", data, dir_offset + 12)
    total = num_named + num_id
    entries_start = dir_offset + 16
    if entries_start + total * 8 > len(data):
        raise PEFormatError(f"resource directory entries truncated at {dir_offset:#x}")
    entries: list[tuple[int, int, int]] = []
    for i in range(total):
        entry_off = entries_start + i * 8
        name_or_id, off_to_data = struct.unpack_from("<II", data, entry_off)
        entries.append((entry_off, name_or_id, off_to_data))
    return num_named, num_id, entries


def find_version_resources(data: bytes | bytearray, pe: PEInfo) -> list[VersionResourceRef]:
    rsrc_base = pe.rva_to_file_offset(pe.rsrc_dir_rva, min(16, pe.rsrc_dir_size))
    _, _, lvl1 = _read_resource_dir_entries(data, rsrc_base)
    refs: list[VersionResourceRef] = []

    for _, type_id, off1 in lvl1:
        if (type_id & IMAGE_RESOURCE_NAME_IS_STRING) or type_id != RT_VERSION:
            continue
        if not (off1 & IMAGE_RESOURCE_DATA_IS_DIRECTORY):
            raise PEFormatError("RT_VERSION level 1 entry does not point to a subdirectory")
        dir2_off = rsrc_base + (off1 & 0x7FFFFFFF)
        _, _, lvl2 = _read_resource_dir_entries(data, dir2_off)
        for _, _, off2 in lvl2:
            if not (off2 & IMAGE_RESOURCE_DATA_IS_DIRECTORY):
                raise PEFormatError("RT_VERSION level 2 entry does not point to a subdirectory")
            dir3_off = rsrc_base + (off2 & 0x7FFFFFFF)
            _, _, lvl3 = _read_resource_dir_entries(data, dir3_off)
            for lang_entry_off, lang_raw, off3 in lvl3:
                if off3 & IMAGE_RESOURCE_DATA_IS_DIRECTORY:
                    raise PEFormatError("RT_VERSION level 3 entry unexpectedly points to a directory")
                if lang_raw & IMAGE_RESOURCE_NAME_IS_STRING:
                    continue
                data_entry_off = rsrc_base + off3
                if data_entry_off + 16 > len(data):
                    raise PEFormatError("IMAGE_RESOURCE_DATA_ENTRY out of bounds")
                data_rva, data_size, _, _ = struct.unpack_from("<IIII", data, data_entry_off)
                data_off = pe.rva_to_file_offset(data_rva, data_size)
                refs.append(
                    VersionResourceRef(
                        dir_offset=dir3_off,
                        lang_entry_offset=lang_entry_off,
                        lang_id=lang_raw & 0xFFFF,
                        data_entry_offset=data_entry_off,
                        data_rva=data_rva,
                        data_offset=data_off,
                        data_size=data_size,
                    )
                )
    return refs


def parse_version_tree(blob: bytes | bytearray, offset: int = 0, limit: int | None = None) -> VersionNode:
    end_limit = len(blob) if limit is None else limit
    if offset + 6 > end_limit:
        raise PEFormatError(f"VS_VERSIONINFO node header truncated at {offset:#x}")
    w_len, w_val_len, w_type = struct.unpack_from("<HHH", blob, offset)
    node_end = offset + w_len
    if w_len < 6 or node_end > end_limit:
        raise PEFormatError(f"invalid VS_VERSIONINFO node wLength={w_len} at {offset:#x}")

    key_off = offset + 6
    pos = key_off
    key_end = -1
    while pos + 2 <= node_end:
        if blob[pos] == 0 and blob[pos + 1] == 0:
            key_end = pos
            break
        pos += 2
    if key_end < 0:
        raise PEFormatError(f"unterminated VS_VERSIONINFO szKey at {offset:#x}")
    key = bytes(blob[key_off:key_end]).decode("utf-16-le")
    pos = key_end + 2

    val_off = align4(pos)
    if w_val_len == 0:
        val_bytes_len = 0
    elif w_type == 1:
        # String node values store wValueLength in UTF-16 code units (WCHARs).
        val_bytes_len = w_val_len * 2
    else:
        val_bytes_len = w_val_len

    if val_off + val_bytes_len > node_end:
        raise PEFormatError(
            f"VS_VERSIONINFO value exceeds node bounds for key={key!r} "
            f"(val_off={val_off}, len={val_bytes_len}, node_end={node_end})"
        )
    value = bytes(blob[val_off : val_off + val_bytes_len])

    child_pos = align4(val_off + val_bytes_len)
    children: list[VersionNode] = []
    while child_pos < node_end:
        child = parse_version_tree(blob, child_pos, node_end)
        children.append(child)
        child_pos = align4(child.end_off)

    return VersionNode(
        start_off=offset,
        end_off=node_end,
        key_off=key_off,
        key=key,
        w_type=w_type,
        w_value_len=w_val_len,
        value_off=val_off,
        value_bytes_len=val_bytes_len,
        value=value,
        children=children,
    )


def serialize_version_tree(node: VersionNode) -> bytes:
    buf = bytearray(6)
    key_bytes = node.key.encode("utf-16-le") + b"\x00\x00"
    buf.extend(key_bytes)
    while len(buf) % 4:
        buf.append(0)

    if node.value:
        buf.extend(node.value)
        w_val_len = len(node.value) // 2 if node.w_type == 1 else len(node.value)
    else:
        w_val_len = 0

    for child in node.children:
        while len(buf) % 4:
            buf.append(0)
        buf.extend(serialize_version_tree(child))

    struct.pack_into("<HHH", buf, 0, len(buf), w_val_len, node.w_type)
    return bytes(buf)


def _sort_level3_id_entries_if_needed(buf: bytearray, dir_offset: int) -> None:
    num_named, num_id = struct.unpack_from("<HH", buf, dir_offset + 12)
    if num_id <= 1:
        return
    id_start = dir_offset + 16 + num_named * 8
    entries = [struct.unpack_from("<II", buf, id_start + i * 8) for i in range(num_id)]
    sorted_entries = sorted(entries, key=lambda item: item[0])
    if sorted_entries != entries:
        for i, (nid, off) in enumerate(sorted_entries):
            struct.pack_into("<II", buf, id_start + i * 8, nid, off)


def _format_version_value_bytes(ver_str: str) -> bytes:
    return ver_str.encode("utf-16-le") + b"\x00\x00"


def _patch_fixed_file_info(
    value: bytes,
    file_ver: tuple[int, int, int, int] | None,
    prod_ver: tuple[int, int, int, int] | None,
) -> tuple[bytes, int]:
    if len(value) < 52:
        raise PEFormatError("VS_FIXEDFILEINFO smaller than 52 bytes")
    sig = struct.unpack_from("<I", value, 0)[0]
    if sig != VS_FFI_SIGNATURE:
        raise PEFormatError(f"invalid VS_FIXEDFILEINFO signature: {sig:#x}")
    arr = bytearray(value)
    changes = 0
    if file_ver is not None:
        ms = (file_ver[0] << 16) | file_ver[1]
        ls = (file_ver[2] << 16) | file_ver[3]
        if struct.unpack_from("<II", arr, 8) != (ms, ls):
            struct.pack_into("<II", arr, 8, ms, ls)
            changes += 1
    if prod_ver is not None:
        ms = (prod_ver[0] << 16) | prod_ver[1]
        ls = (prod_ver[2] << 16) | prod_ver[3]
        if struct.unpack_from("<II", arr, 16) != (ms, ls):
            struct.pack_into("<II", arr, 16, ms, ls)
            changes += 1
    return bytes(arr), changes


def localize_pe_bytes(
    data: bytes,
    *,
    target_lang: int = DEFAULT_TARGET_LANG,
    source_lang: int = DEFAULT_SOURCE_LANG,
    file_version: str | None = None,
    product_version: str | None = None,
    check_only: bool = False,
) -> tuple[bytes, PatchResult]:
    pe = parse_pe(data)
    refs = find_version_resources(data, pe)
    if not refs:
        raise PEFormatError("PE file contains no RT_VERSION resources")

    file_ver_quad = parse_version_quad(file_version) if file_version is not None else None
    prod_ver_quad = parse_version_quad(product_version) if product_version is not None else None

    src_prefix = f"{source_lang:04x}"
    dst_prefix = f"{target_lang:04x}"

    buf = bytearray(data)
    original_langs = [ref.lang_id for ref in refs]
    patched_dir_entries = 0
    patched_string_tables = 0
    patched_translations = 0
    patched_versions = 0
    check_issues: list[str] = []

    matching_refs = [ref for ref in refs if ref.lang_id in (source_lang, target_lang)]
    if not matching_refs:
        langs_str = ", ".join(f"{lang:#06x}" for lang in original_langs)
        raise PEFormatError(
            f"no RT_VERSION resource with source_lang={source_lang:#06x} or target_lang={target_lang:#06x} "
            f"(found: {langs_str})"
        )

    touched_dirs: set[int] = set()
    rebuild_required = file_version is not None or product_version is not None

    for ref in matching_refs:
        if ref.lang_id == source_lang and source_lang != target_lang:
            if check_only:
                check_issues.append(f"resource directory entry still uses {source_lang:#06x}")
            else:
                struct.pack_into("<I", buf, ref.lang_entry_offset, target_lang)
                patched_dir_entries += 1
                touched_dirs.add(ref.dir_offset)

        blob = bytes(buf[ref.data_offset : ref.data_offset + ref.data_size])
        root = parse_version_tree(blob)

        if (file_ver_quad is not None or prod_ver_quad is not None) and root.value:
            new_ffi, ffi_changes = _patch_fixed_file_info(root.value, file_ver_quad, prod_ver_quad)
            if ffi_changes:
                if check_only:
                    check_issues.append("VS_FIXEDFILEINFO version does not match requested version")
                else:
                    root.value = new_ffi
                    patched_versions += ffi_changes

        for top_child in root.children:
            if top_child.key == "StringFileInfo":
                for st in top_child.children:
                    if len(st.key) == 8 and st.key[:4].lower() == src_prefix and source_lang != target_lang:
                        new_key = dst_prefix + st.key[4:].lower()
                        if check_only:
                            check_issues.append(f"StringTable key {st.key!r} != {new_key!r}")
                        else:
                            if not rebuild_required:
                                key_bytes = new_key.encode("utf-16-le")
                                abs_key_off = ref.data_offset + st.key_off
                                if len(key_bytes) != len(st.key.encode("utf-16-le")):
                                    raise PEFormatError("unexpected StringTable key length mismatch")
                                buf[abs_key_off : abs_key_off + len(key_bytes)] = key_bytes
                            st.key = new_key
                            patched_string_tables += 1
                    elif len(st.key) == 8 and st.key[:4].lower() != dst_prefix and check_only:
                        check_issues.append(f"StringTable key {st.key!r} does not start with {dst_prefix!r}")

                    if file_version is not None or product_version is not None:
                        for entry in st.children:
                            if entry.key == "FileVersion" and file_version is not None:
                                expected_val = _format_version_value_bytes(file_version)
                                if entry.value != expected_val:
                                    if check_only:
                                        check_issues.append("StringTable FileVersion mismatch")
                                    else:
                                        entry.value = expected_val
                                        patched_versions += 1
                            elif entry.key == "ProductVersion" and product_version is not None:
                                expected_val = _format_version_value_bytes(product_version)
                                if entry.value != expected_val:
                                    if check_only:
                                        check_issues.append("StringTable ProductVersion mismatch")
                                    else:
                                        entry.value = expected_val
                                        patched_versions += 1

            elif top_child.key == "VarFileInfo":
                for var in top_child.children:
                    if var.key != "Translation" or len(var.value) < 4 or len(var.value) % 4 != 0:
                        continue
                    var_arr = bytearray(var.value)
                    for i in range(0, len(var_arr), 4):
                        lang_w, cp_w = struct.unpack_from("<HH", var_arr, i)
                        if lang_w == source_lang and source_lang != target_lang:
                            if check_only:
                                check_issues.append(f"VarFileInfo Translation still has {source_lang:#06x}")
                            else:
                                struct.pack_into("<HH", var_arr, i, target_lang, cp_w)
                                if not rebuild_required:
                                    abs_pair_off = ref.data_offset + var.value_off + i
                                    struct.pack_into("<HH", buf, abs_pair_off, target_lang, cp_w)
                                patched_translations += 1
                        elif lang_w != target_lang and check_only:
                            check_issues.append(f"VarFileInfo Translation has {lang_w:#06x} != {target_lang:#06x}")
                    var.value = bytes(var_arr)

        if rebuild_required and not check_only:
            rebuilt = serialize_version_tree(root)
            if len(rebuilt) > ref.data_size:
                raise PEFormatError(
                    f"rebuilt VS_VERSIONINFO ({len(rebuilt)} bytes) exceeds existing RT_VERSION slot ({ref.data_size} bytes)"
                )
            padded = rebuilt + b"\x00" * (ref.data_size - len(rebuilt))
            buf[ref.data_offset : ref.data_offset + ref.data_size] = padded
            struct.pack_into("<I", buf, ref.data_entry_offset + 4, len(rebuilt))

    for dir_off in touched_dirs:
        _sort_level3_id_entries_if_needed(buf, dir_off)

    expected_checksum = compute_pe_checksum(buf, pe.checksum_offset)
    if check_only:
        if pe.original_checksum != expected_checksum:
            check_issues.append(
                f"PE CheckSum mismatch: header={pe.original_checksum:#x} expected={expected_checksum:#x}"
            )
        return bytes(buf), PatchResult(
            file_size=len(buf),
            source_lang=source_lang,
            target_lang=target_lang,
            original_langs=original_langs,
            patched_dir_entries=0,
            patched_string_tables=0,
            patched_translations=0,
            patched_versions=0,
            original_checksum=pe.original_checksum,
            new_checksum=expected_checksum,
            changed=False,
            check_issues=check_issues,
        )

    if pe.original_checksum != expected_checksum:
        struct.pack_into("<I", buf, pe.checksum_offset, expected_checksum)

    changed = bytes(buf) != data
    return bytes(buf), PatchResult(
        file_size=len(buf),
        source_lang=source_lang,
        target_lang=target_lang,
        original_langs=original_langs,
        patched_dir_entries=patched_dir_entries,
        patched_string_tables=patched_string_tables,
        patched_translations=patched_translations,
        patched_versions=patched_versions,
        original_checksum=pe.original_checksum,
        new_checksum=expected_checksum,
        changed=changed,
    )


def _atomic_write_preserve_mode(path: Path, payload: bytes) -> None:
    st_mode = path.stat().st_mode
    tmp_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            dir=path.parent,
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as tf:
            tmp_path = Path(tf.name)
            tf.write(payload)
            tf.flush()
            os.fsync(tf.fileno())
        os.chmod(tmp_path, st_mode)
        os.replace(tmp_path, path)
        tmp_path = None
    finally:
        if tmp_path is not None and tmp_path.exists():
            tmp_path.unlink(missing_ok=True)


def localize_pe_file(
    exe_path: Path,
    *,
    target_lang: int = DEFAULT_TARGET_LANG,
    source_lang: int = DEFAULT_SOURCE_LANG,
    file_version: str | None = None,
    product_version: str | None = None,
    check_only: bool = False,
    dry_run: bool = False,
) -> PatchResult:
    if exe_path.is_symlink():
        raise PEFormatError(f"refusing symlink input: {exe_path}")
    if not exe_path.is_file():
        raise PEFormatError(f"PE executable not found: {exe_path}")

    raw = exe_path.read_bytes()
    patched, result = localize_pe_bytes(
        raw,
        target_lang=target_lang,
        source_lang=source_lang,
        file_version=file_version,
        product_version=product_version,
        check_only=check_only,
    )
    if not check_only and not dry_run and result.changed:
        assert len(patched) == len(raw), "PE localization must preserve exact file size"
        _atomic_write_preserve_mode(exe_path, patched)
    return result


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exe", type=Path, help="Path to Windows PE executable (e.g. Lwsmf.exe)")
    parser.add_argument(
        "--lang",
        type=parse_lang_id,
        default=DEFAULT_TARGET_LANG,
        help="Target RT_VERSION language ID (default: 0x0804 Simplified Chinese)",
    )
    parser.add_argument(
        "--source-lang",
        type=parse_lang_id,
        default=DEFAULT_SOURCE_LANG,
        help="Source RT_VERSION language ID to replace (default: 0x0409 US English)",
    )
    parser.add_argument("--file-version", default=None, help="Optional A.B.C.D file version override")
    parser.add_argument("--product-version", default=None, help="Optional A.B.C.D product version override")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--dry-run", action="store_true", help="Validate and compute patch without writing")
    mode.add_argument("--check", action="store_true", help="Verify language ID and CheckSum; exit 1 if unpatched")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        res = localize_pe_file(
            args.exe,
            target_lang=args.lang,
            source_lang=args.source_lang,
            file_version=args.file_version,
            product_version=args.product_version,
            check_only=args.check,
            dry_run=args.dry_run,
        )
    except (PEFormatError, OSError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    if args.check:
        if res.check_issues:
            print(f"WINDOWS_PE_CHECK_FAIL exe={args.exe.name} issues={len(res.check_issues)}")
            for issue in res.check_issues:
                print(f"- {issue}")
            return 1
        print(
            f"WINDOWS_PE_CHECK_OK exe={args.exe.name} lang={res.target_lang:#06x} "
            f"checksum={res.new_checksum:#x} bytes={res.file_size}"
        )
        return 0

    mode_tag = "DRY_RUN" if args.dry_run else "OK"
    print(
        f"WINDOWS_PE_LOCALIZE_{mode_tag} exe={args.exe.name} lang={res.target_lang:#06x} "
        f"dir={res.patched_dir_entries} str_tables={res.patched_string_tables} "
        f"translations={res.patched_translations} versions={res.patched_versions} "
        f"checksum={res.original_checksum:#x}->{res.new_checksum:#x} bytes={res.file_size}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
