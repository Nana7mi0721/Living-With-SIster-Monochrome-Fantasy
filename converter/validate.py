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


DATA0_FIELD_LABELS = [
    "技能の名前", "スキル説明", "スキルタイプ", "消費行動力", "戦況影響値", "効果基本値",
    "筋力影響", "精神影響", "使用時文章", "スキル属性", "アニメーション",
    "付与するステータス1", "付与するステータス2",
    "回復させるステータス1", "回復させるステータス2", "回復させるステータス3",
    "相手の防御を", "スキルタイプ",
]
DATA0_FIELD_LAYOUT = [
    ("str", 2000), ("str", 2001), ("num", 1000), ("num", 1001),
    ("num", 1002), ("num", 1003), ("num", 1004), ("num", 1005),
    ("str", 2002), ("num", 1006), ("str", 2003),
    ("num", 1007), ("num", 1008), ("num", 1009), ("num", 1010),
    ("num", 1011), ("num", 1012), ("num", 1013),
]


def _column_values(rows, fields, field_index):
    numeric = [f for f in fields if f.get("kind") == "num"]
    pos = next((i for i, f in enumerate(numeric) if f.get("index") == field_index), None)
    if pos is None:
        return []
    return [row["numbers"][pos] for row in rows
            if isinstance(row.get("numbers"), list) and pos < len(row["numbers"])]


def audit_database(doc: dict) -> dict:
    """Static D-35 audit of Data type0: field layout, row widths, and the two 250 reads."""
    db = doc.get("database") or {}
    types = db.get("types") or []
    fields = (types[0].get("fields") or []) if types else []
    rows = (types[0].get("rows") or []) if types else []
    checks = []

    def check(name, got, want):
        checks.append({"name": name, "got": got, "want": want, "ok": got == want})

    check("data0.present", bool(types), True)
    check("data0.field_count", len(fields), len(DATA0_FIELD_LAYOUT))
    check("data0.row_count", len(rows), 44)
    layout = [(f.get("kind"), f.get("raw")) for f in fields]
    check("data0.field_layout", layout, DATA0_FIELD_LAYOUT)
    numeric_width = len([f for f in fields if f.get("kind") == "num"])
    string_width = len([f for f in fields if f.get("kind") == "str"])
    check("data0.numeric_width", numeric_width, 14)
    check("data0.string_width", string_width, 4)
    rows_consistent = all(
        isinstance(r.get("numbers"), list) and len(r["numbers"]) == numeric_width
        and isinstance(r.get("strings"), list) and len(r["strings"]) == string_width
        for r in rows)
    check("data0.row_widths_consistent", rows_consistent, True)

    col2 = _column_values(rows, fields, 2)
    col3 = _column_values(rows, fields, 3)
    col6 = _column_values(rows, fields, 6)
    facts = {
        "skill_type_col2_all_zero": bool(col2) and all(v == 0 for v in col2),
        "action_cost_col3_values": sorted(set(col3)),
        "col6_values": sorted(set(col6)),
        "action_cost_has_10": 10 in col3,
        "col6_has_100": 100 in col6,
    }
    needs_retail = [
        {
            "lookup": "┏ 効果対象", "project_label_present": False,
            "resolved_field_index": None, "fallback_field_index": 2,
            "fallback_label": "スキルタイプ", "fallback_values_all_zero": facts["skill_type_col2_all_zero"],
            "nonzero_columns": [{"field_index": 3, "label": "消費行動力",
                                 "values": facts["action_cost_col3_values"]}],
            "status": "needs-retail-data",
            "note": "Do not alias 効果対象 to col3 without retail DataBase.dat evidence (D-35).",
        },
        {
            "lookup": "使用可能シーン", "project_label_present": False,
            "resolved_field_index": None, "fallback_field_index": 6,
            "fallback_label": "筋力影響", "values": facts["col6_values"],
            "status": "needs-retail-data",
            "note": "CE159 pc110 reads col6 only because the label is absent; 100 on this column is not proof of the retail scene permission.",
        },
        {
            "case": "BattleSkillTargetTest",
            "reason": "CE159 pc110/pc115 read 250 columns by fallback; test seeds Data 0/2/2 and 0/2/6",
            "status": "needs-retail-data",
        },
    ]
    out = schema.envelope("wolf.database_schema_audit")
    out.update({
        "source": "database.json",
        "database_type_count": db.get("type_count"),
        "data_type0": {
            "field_count": len(fields), "row_count": len(rows),
            "fields": [dict(f, label=DATA0_FIELD_LABELS[i] if i < len(DATA0_FIELD_LABELS) else None)
                       for i, f in enumerate(fields)],
            "facts": facts,
        },
        "needs_retail_data": needs_retail,
        "checks": checks,
        "ok": not [c for c in checks if not c["ok"]],
    })
    return out


def run(args) -> int:
    with open(os.path.join(args.out, args.programs_name), "r", encoding="utf-8") as f:
        doc = json.load(f)
    with open(os.path.join("docs", "BASELINE.json"), "r", encoding="utf-8") as f:
        baseline = json.load(f)
    with open(os.path.join(args.out, args.database_name), "r", encoding="utf-8") as f:
        database = json.load(f)
    report = reconcile(doc, baseline)
    audit = audit_database(database)
    os.makedirs(args.reports, exist_ok=True)
    schema.write_artifact(report, os.path.join(args.reports, "reconcile.json"), "reconcile.json")
    schema.write_artifact(audit, os.path.join(args.reports, "database_schema_audit.json"),
                          "database_schema_audit.json")
    for c in report["failed"]:
        print(f"FAIL {c['name']}: got {c['got']!r} want {c['want']!r}")
    for c in audit["checks"]:
        if not c["ok"]:
            print(f"FAIL {c['name']}: got {c['got']!r} want {c['want']!r}")
    print(f"{len(report['checks']) - len(report['failed'])}/{len(report['checks'])} checks passed")
    d0 = audit["data_type0"]
    print(f"DATABASE_SCHEMA_AUDIT_OK={audit['ok']} fields={d0['field_count']} rows={d0['row_count']} "
          f"needs_retail={len(audit['needs_retail_data'])}")
    return 0 if report["ok"] and audit["ok"] else 1
