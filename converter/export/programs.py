"""导出 programs.json（CE + 地图事件）。"""
from __future__ import annotations

import os

from .. import schema
from ..parse import commonevent, mps

MAP_SUFFIX = ".mps"


def _map_id(name: str, index: int) -> int:
    stem = os.path.splitext(name)[0]
    digits = "".join(ch for ch in stem if ch.isdigit())
    return int(digits) if digits else 1000 + index


def build(root: str) -> tuple:
    warnings = []
    ce = commonevent.load(os.path.join(root, "BasicData", "CommonEvent.dat"))
    warnings += [{"where": "CommonEvent.dat", **w} for w in ce.get("warnings", [])]

    maps = []
    map_dir = os.path.join(root, "MapData")
    names = sorted(n for n in os.listdir(map_dir) if n.lower().endswith(MAP_SUFFIX))
    for i, name in enumerate(names):
        m = mps.load_map(os.path.join(map_dir, name), _map_id(name, i))
        warnings += [{"where": name, **w} for w in m.get("warnings", [])]
        maps.append({
            "id": m["id"], "file": m["file"], "width": m["width"], "height": m["height"],
            "tileset_id": m["tileset_id"], "event_count": m.get("event_count"),
            "page_count": m.get("page_count"), "events": m["events"],
        })

    ce_progs = ce["events"]
    map_progs = [ev for m in maps for ev in m["events"]]
    ce_stats = schema.count_slots(ce_progs)
    map_stats = schema.count_slots(map_progs)
    ops = {}
    for p in ce_progs + map_progs:
        for s in p.get("slots") or []:
            if s.get("op"):
                ops[s["op"]] = ops.get(s["op"], 0) + 1

    doc = schema.envelope(schema.SCHEMA_PROGRAMS)
    doc.update({
        "common_events": ce_progs,
        "maps": maps,
        "totals": {
            "common_events": ce_stats["events"], "map_events": map_stats["events"],
            "maps": len(maps), "slots": ce_stats["slots"] + map_stats["slots"],
            "blank_slots": ce_stats["blank_slots"] + map_stats["blank_slots"],
            "real_instructions": ce_stats["real_instructions"] + map_stats["real_instructions"],
            "distinct_opcodes": len(ops),
        },
        "opcode_histogram": {str(k): ops[k] for k in sorted(ops)},
        "unknown_opcodes": {str(k): v for k, v in sorted((ce.get("unknown_opcodes") or {}).items())},
    })
    return doc, warnings


def run(args) -> int:
    doc, warnings = build(args.root)
    info = schema.write_artifact(doc, os.path.join(args.out, args.programs_name), args.programs_name)
    os.makedirs(args.reports, exist_ok=True)
    if warnings:
        schema.write_artifact({"schema": "wolf.warnings", "schema_version": 1,
                               "items": warnings},
                              os.path.join(args.reports, "conversion_warnings.json"),
                              "conversion_warnings.json")
    import json
    print(json.dumps({"written": info, "totals": doc["totals"],
                      "warnings": len(warnings)}, ensure_ascii=False))
    return 0
