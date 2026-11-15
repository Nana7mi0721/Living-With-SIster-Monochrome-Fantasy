"""解析 DataBase.dat, CDataBase.dat, SysDatabase.dat, Game.dat, TileSetData.dat, MapTree.dat。"""
from __future__ import annotations

import os
from ..reader import Reader, WolfEof

DB_MAGIC = b"\x00\x57\x00\x00\x4F\x4C"
DB_MAGIC_TYPE = b"FM\x00"
DB_VERSION = 0xC2
DB_FOOTER = 0xC2
DB_TYPE_SEPARATOR = b"\xFE\xFF\xFF\xFF"
STRING_START = 2000
INT_START = 1000

GAME_MAGIC = b"\x00\x57\x00\x00\x4F\x4C\x00\x46\x4D"
GAME_FOOTER = 0xC3

TILE_MAGIC = b"\x00\x57\x00\x00\x4F\x4C"
TILE_MAGIC_TYPE = b"FM\x00"
TILE_VERSION_210 = 0xD2
TILE_VERSION_209 = 0xD1
TILE_FOOTER = 0xCF
TILE_SEPARATOR = 0xFF

MAPTREE_MAGIC = b"\x00" * 10
MAPTREE_VERSION = 0x90
MAPTREE_FOOTER = 0xBC


def parse_game_dat(path: str) -> tuple[dict, list]:
    with open(path, "rb") as f:
        data = f.read()
    r = Reader(data)
    magic = r.take(len(GAME_MAGIC))
    if magic != GAME_MAGIC:
        raise ValueError(f"Game.dat bad magic: {magic!r}")
    version_header = r.u8()
    u8_len = r.u32()
    u8_settings = list(r.take(u8_len))
    string_count = r.u32()
    strings = [r.t_str() for _ in range(string_count)]
    recorded_size = r.u32()
    unknown_size = r.u32()
    u16_len = r.u32()
    u16_settings = [r.u16() for _ in range(u16_len)]
    random_a = r.u32()
    random_b = r.u32()
    static_randoms = list(r.take(r.remaining() - 1))
    footer = r.u8()
    if footer != GAME_FOOTER:
        raise ValueError(f"Game.dat bad footer: 0x{footer:02X}")
    return {
        "version_header": version_header,
        "u8_settings": u8_settings,
        "strings": strings,
        "recorded_size": recorded_size,
        "u16_settings": u16_settings,
        "random_a": random_a,
        "random_b": random_b,
        "static_randoms": static_randoms,
    }, r.warnings


def parse_database_file(path: str) -> tuple[dict, list]:
    with open(path, "rb") as f:
        data = f.read()
    r = Reader(data)
    magic = r.take(len(DB_MAGIC))
    if magic != DB_MAGIC:
        raise ValueError(f"{path} bad magic: {magic!r}")
    version_header = r.u8()
    magic_type = r.take(len(DB_MAGIC_TYPE))
    if magic_type != DB_MAGIC_TYPE:
        raise ValueError(f"{path} bad magic type: {magic_type!r}")
    version = r.u8()
    if version != DB_VERSION:
        raise ValueError(f"{path} unsupported version: 0x{version:02X}")

    type_count = r.u32()
    types = []
    for _ in range(type_count):
        sep = r.take(4)
        if sep != DB_TYPE_SEPARATOR:
            raise ValueError(f"{path} bad type separator: {sep!r}")
        data_id_method = r.u32()
        field_count = r.u32()
        if data_id_method == 0x0001D4C0:
            _ = r.t_str()
        fields_raw = [r.u32() for _ in range(field_count)]
        fields = []
        for raw in fields_raw:
            if raw >= STRING_START:
                fields.append({"kind": "str", "index": raw - STRING_START, "raw": raw})
            elif raw >= INT_START:
                fields.append({"kind": "num", "index": raw - INT_START, "raw": raw})
            else:
                fields.append({"kind": "absent", "index": raw, "raw": raw})

        row_count = r.u32()
        num_cols = sum(1 for f in fields if f["kind"] != "str")
        str_cols = sum(1 for f in fields if f["kind"] == "str")

        rows = []
        for _ in range(row_count):
            nums = [r.i32() for _ in range(num_cols)]
            strs = [r.t_str() for _ in range(str_cols)]
            rows.append({"numbers": nums, "strings": strs})

        types.append({
            "data_id_method": data_id_method,
            "fields": fields,
            "row_count": row_count,
            "rows": rows,
        })

    footer = r.u8()
    if footer != DB_FOOTER:
        raise ValueError(f"{path} bad footer: 0x{footer:02X}")

    return {
        "version": version,
        "type_count": type_count,
        "types": types,
    }, r.warnings


def parse_tileset_file(path: str) -> tuple[list, list]:
    with open(path, "rb") as f:
        data = f.read()
    r = Reader(data)
    magic = r.take(len(TILE_MAGIC))
    if magic != TILE_MAGIC:
        raise ValueError(f"{path} bad magic: {magic!r}")
    _version_header = r.u8()
    magic_type = r.take(len(TILE_MAGIC_TYPE))
    if magic_type != TILE_MAGIC_TYPE:
        raise ValueError(f"{path} bad magic type: {magic_type!r}")
    version = r.u8()
    if version not in (TILE_VERSION_210, TILE_VERSION_209):
        raise ValueError(f"{path} bad version: 0x{version:02X}")

    count = r.u32()
    slots = 31 if version == TILE_VERSION_210 else 15
    tilesets = []
    for _ in range(count):
        name = r.t_str()
        base_image = r.t_str()
        autotiles = []
        for _ in range(slots):
            raw = r.t_str_bytes()
            autotiles.append(raw.decode("utf-8", "replace"))
        sep1 = r.u8()
        if sep1 != TILE_SEPARATOR:
            raise ValueError(f"{path} bad sep1: 0x{sep1:02X}")
        tag_count = r.u32()
        tags = list(r.take(tag_count))
        sep2 = r.u8()
        if sep2 != TILE_SEPARATOR:
            raise ValueError(f"{path} bad sep2: 0x{sep2:02X}")
        passable_count = r.u32()
        passability = [r.u32() for _ in range(passable_count)]
        tilesets.append({
            "name": name,
            "base_image": base_image,
            "autotiles": autotiles,
            "tags": tags,
            "passability": passability,
        })

    footer = r.u8()
    if footer != TILE_FOOTER:
        raise ValueError(f"{path} bad footer: 0x{footer:02X}")

    return tilesets, r.warnings


def parse_maptree_file(path: str) -> tuple[dict, list]:
    with open(path, "rb") as f:
        data = f.read()
    r = Reader(data)
    pad = r.take(10)
    if pad != MAPTREE_MAGIC:
        raise ValueError(f"{path} bad header pad")
    version = r.u8()
    if version != MAPTREE_VERSION:
        raise ValueError(f"{path} bad version: 0x{version:02X}")
    entry_count = r.u32()
    entries = []
    for _ in range(entry_count):
        parent = r.i32()
        map_id = r.u32()
        entries.append({"parent": parent, "map_id": map_id})
    footer = r.u8()
    if footer != MAPTREE_FOOTER:
        raise ValueError(f"{path} bad footer: 0x{footer:02X}")
    return {
        "version": version,
        "entry_count": entry_count,
        "entries": entries,
    }, r.warnings


def load_database(root: str) -> dict:
    warnings = []

    game, w = parse_game_dat(os.path.join(root, "BasicData", "Game.dat"))
    warnings.extend(w)

    database, w = parse_database_file(os.path.join(root, "BasicData", "DataBase.dat"))
    warnings.extend(w)

    cdata, w = parse_database_file(os.path.join(root, "BasicData", "CDataBase.dat"))
    warnings.extend(w)

    system, w = parse_database_file(os.path.join(root, "BasicData", "SysDatabase.dat"))
    warnings.extend(w)

    tilesets, w = parse_tileset_file(os.path.join(root, "BasicData", "TileSetData.dat"))
    warnings.extend(w)

    map_tree, w = parse_maptree_file(os.path.join(root, "BasicData", "MapTree.dat"))
    warnings.extend(w)

    return {
        "game": game,
        "database": database,
        "cdata": cdata,
        "system": system,
        "tilesets": tilesets,
        "map_tree": map_tree,
        "warnings": warnings,
    }
