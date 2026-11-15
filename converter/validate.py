"""对账：把导出结果与 docs/BASELINE.json 逐项比对，不符即失败（exit 1）。"""
from __future__ import annotations

import json
import os

from . import schema


def _opcode_hist(programs: list) -> dict:
    hist = {}
    for p in programs:
        for s in p.get("slots") or []:
            op = s.get("op") or 0
            if op:
                hist[op] = hist.get(op, 0) + 1
    return hist


def _check(checks: list, name: str, got, want) -> None:
    ok = got == want
    checks.append({"name": name, "got": got, "want": want, "ok": ok})


def reconcile(doc: dict, baseline: dict) -> dict:
    checks = []
    ce = doc.get("common_events") or []
    maps = doc.get("maps") or []
    map_progs = [ev for m in maps for ev in m.get("events") or []]

    ce_stats = schema.count_slots(ce)
    me_stats = schema.count_slots(map_progs)
    b_ce = baseline["common_events"]
    b_map = baseline["map_events"]

    _check(checks, "ce.count", ce_stats["events"], b_ce["count"])
    _check(checks, "ce.slots", ce_stats["slots"], b_ce["slots"])
    _check(checks, "ce.blank", ce_stats["blank_slots"], b_ce["blank"])
    _check(checks, "ce.real", ce_stats["real_instructions"], b_ce["real"])
    _check(checks, "ce.distinct_opcodes", ce_stats["distinct_opcodes"], b_ce["distinct_opcodes"])

    cond = {}
    for p in ce:
        rc = (p.get("run_condition_op_var_val") or [0])[0]
        cond[str(rc)] = cond.get(str(rc), 0) + 1
    _check(checks, "ce.run_condition", cond, {str(k): v for k, v in b_ce["run_condition"].items()})

    got_ops = {str(k): v for k, v in _opcode_hist(ce).items()}
    want_ops = {str(k): v for k, v in baseline.get("ce_opcodes", {}).items()}
    _check(checks, "ce.opcodes", got_ops, want_ops)

    total_map_events = sum(len({e["id"] for e in m.get("events") or []}) for m in maps)
    _check(checks, "map.count", total_map_events, b_map["count"])
    _check(checks, "map.pages", me_stats["events"], b_map["pages"])
    _check(checks, "map.slots", me_stats["slots"], b_map["slots"])
    _check(checks, "map.blank", me_stats["blank_slots"], b_map["blank"])
    _check(checks, "map.real", me_stats["real_instructions"], b_map["real"])
    _check(checks, "map.distinct_opcodes", me_stats["distinct_opcodes"], b_map["distinct_opcodes"])
    _check(checks, "maps.files", len(maps), len(b_map["per_map"]))

    for m in maps:
        name = m.get("file")
        want = (b_map.get("per_map") or {}).get(name)
        if want is None:
            checks.append({"name": f"map.{name}", "got": "present", "want": "absent-in-baseline", "ok": False})
            continue
        evs = m.get("events") or []
        st = schema.count_slots(evs)
        _check(checks, f"map.{name}.w", m.get("width"), want["w"])
        _check(checks, f"map.{name}.h", m.get("height"), want["h"])
        _check(checks, f"map.{name}.tileset", m.get("tileset_id"), want["tileset"])
        _check(checks, f"map.{name}.events", len({e["id"] for e in evs}), want["events"])
        _check(checks, f"map.{name}.pages", len(evs), want["pages"])
        _check(checks, f"map.{name}.commands", st["slots"], want["commands"])

    total_real = ce_stats["real_instructions"] + me_stats["real_instructions"]
    _check(checks, "total_real_instructions", total_real, baseline["total_real_instructions"])

    failed = [c for c in checks if not c["ok"]]
    return {
        "schema": "wolf.reconcile", "schema_version": 1,
        "ok": not failed, "checks": checks, "failed": failed,
        "unknown_opcodes": doc.get("unknown_opcodes") or {},
    }


def run(args) -> int:
    with open(os.path.join(args.out, args.programs_name), "r", encoding="utf-8") as f:
        doc = json.load(f)
    with open(os.path.join("docs", "BASELINE.json"), "r", encoding="utf-8") as f:
        baseline = json.load(f)
    report = reconcile(doc, baseline)
    os.makedirs(args.reports, exist_ok=True)
    schema.write_artifact(report, os.path.join(args.reports, "reconcile.json"), "reconcile.json")
    for c in report["failed"]:
        print(f"FAIL {c['name']}: got {c['got']!r} want {c['want']!r}")
    print(f"{len(report['checks']) - len(report['failed'])}/{len(report['checks'])} checks passed")
    return 0 if report["ok"] else 1
