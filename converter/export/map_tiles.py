"""导出 map_tiles.json：地图三层图块 + tileset 通行度原始位（W3 Day 3）。

只有地图的**呈现与通行**需要这个产物；programs.json 仍然不携带图块。
图块层是 u32 小端，编成 base64 以控制 JSON 体积：9 张地图合计约 100 KB。

层布局（docs/FORMAT_SPEC.md §5.3）：layer0 地表 → layer1 装饰 → layer2 顶层，
每层 width*height 个 u4、行优先、层优先。图块值解码见 §5.4：
autotile_id = raw / 100000，>0 时低四位是四角模式，否则 raw 就是基础图块号。
"""
from __future__ import annotations

import base64
import json
import os
import struct

from .. import schema
from ..parse import database, mps
from .programs import _map_id

MAP_SUFFIX = ".mps"


def _b64_u32(values) -> str:
    return base64.b64encode(struct.pack("<%dI" % len(values), *values)).decode("ascii")


def build(root: str) -> tuple:
    warnings = []
    tilesets_raw, w = database.parse_tileset_file(
        os.path.join(root, "BasicData", "TileSetData.dat")
    )
    warnings += [{"where": "TileSetData.dat", **x} for x in w]

    tilesets = []
    for index, ts in enumerate(tilesets_raw):
        tilesets.append({
            "index": index,
            "name": ts["name"],
            "base_image": ts["base_image"],
            "autotiles": list(ts["autotiles"]),
            "tags": list(ts["tags"]),
            "passability": _b64_u32(ts["passability"]),
        })

    maps = []
    map_dir = os.path.join(root, "MapData")
    names = sorted(n for n in os.listdir(map_dir) if n.lower().endswith(MAP_SUFFIX))
    for i, name in enumerate(names):
        m = mps.load_map(os.path.join(map_dir, name), _map_id(name, i))
        warnings += [{"where": name, **x} for x in m.get("warnings", [])]
        layers = m.get("layers") or []
        maps.append({
            "id": m["id"],
            "file": m["file"],
            "width": m["width"],
            "height": m["height"],
            "tileset_id": m["tileset_id"],
            "has_tile_data": m["has_tile_data"],
            "layers": [_b64_u32(layer) for layer in layers],
        })

    doc = schema.envelope(schema.SCHEMA_MAP_TILES)
    doc.update({"tilesets": tilesets, "maps": maps})
    return doc, warnings


def run(args) -> int:
    doc, warnings = build(args.root)
    info = schema.write_artifact(
        doc, os.path.join(args.out, args.map_tiles_name), args.map_tiles_name
    )
    if warnings:
        os.makedirs(args.reports, exist_ok=True)
        schema.write_artifact(
            {"schema": "wolf.warnings", "schema_version": 1, "items": warnings},
            os.path.join(args.reports, "map_tiles_warnings.json"),
            "map_tiles_warnings.json",
        )
    print(json.dumps({"written": info, "tilesets": len(doc["tilesets"]),
                      "maps": len(doc["maps"]), "warnings": len(warnings)},
                     ensure_ascii=False))
    return 0
