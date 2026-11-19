#!/usr/bin/env python3
"""Read-only static audit scanner for imoto-life's game_data/programs.json.

Aligned with converter/export/programs.py & converter/parse/commonevent.py
(schema "wolf.programs", version 1). Zero third-party dependencies.

Produces both a concise Markdown summary and a detailed JSON report covering:
  1. RunCondition == 2 (and gated 0) common events: loop / wait / linear classification (BUG-25)
  2. High-risk VM opcodes: 176 (ContinueLoop), 211 (Reserve), 125 (AutoKeys),
     151 (ChangeColor frames magic), 130 (Teleport magic), 140 (Sound magic),
     180 (Wait <= 0), and 121 (SetVariable arg3 bitfields & 0x600/0x700 ops)
  3. Opcode 124 (State Query) (sub, field) distribution & gap highlights
  4. Opcode 150 (Picture): number < 0 / >= 12000 backdrop stretch check (with
     optional pure-stdlib image dimension sniffing via --assets), 0x0F00_0000
     param-mask collision check, plus 160/161/162 Transition & 290 Effect counts

Usage:
  python3 audit_programs_static.py \
      --programs game_data/programs.json \
      --assets work/Data \
      --out-dir reports/static_audit
"""
from __future__ import annotations

import argparse
import json
import struct
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

UNCHANGED_SENTINEL = -1_000_000
SCREEN_SIZE = (1024, 576)

# Magic ranges from godot/core_cs/Magic.cs & WOLF RPG DB magic conventions
SELF_BASE = 1_000_000
VAR_BASE = 2_000_000
STR_BASE = 3_000_000
SYS_BASE = 9_000_000
DB_MAGIC_MIN = 1_000_000_000

ENGINE_STATE_IMPLEMENTED_IN_VM = {18}
ENGINE_STATE_IMPLEMENTED_IN_HOST = {7, 8, 9}
ENGINE_STATE_KNOWN_GAPS = {10, 11, 12, 17, 20, 21}


def signed32(value: int) -> int:
    v = int(value)
    return v - 2**32 if v >= 2**31 else v


def unsigned32(value: int) -> int:
    return int(value) & 0xFFFF_FFFF


def is_var_magic(raw: int) -> bool:
    """Return True if raw integer looks like a WOLF variable/self/sys/DB magic."""
    v = signed32(raw)
    if v < 0:
        return False
    body = v & 0x0FFF_FFFF
    if (
        (SELF_BASE <= body < 4_000_000)
        or (SYS_BASE <= body <= 9_999_999)
        or (v >= DB_MAGIC_MIN)
    ):
        return True
    return False


def sniff_image_size(path: Path) -> tuple[int, int] | None:
    """Read (width, height) from PNG, JPEG, or BMP headers using stdlib only."""
    try:
        with path.open("rb") as f:
            head = f.read(32)
            if len(head) < 24:
                return None
            # PNG
            if head[:8] == b"\x89PNG\r\n\x1a\n" and head[12:16] == b"IHDR":
                w, h = struct.unpack(">II", head[16:24])
                return (w, h)
            # BMP
            if head[:2] == b"BM" and len(head) >= 26:
                w, h = struct.unpack("<ii", head[18:26])
                return (abs(w), abs(h))
            # JPEG
            if head[:2] == b"\xff\xd8":
                f.seek(2)
                while True:
                    marker_start = f.read(1)
                    if not marker_start:
                        return None
                    if marker_start != b"\xff":
                        continue
                    marker = f.read(1)
                    while marker == b"\xff":
                        marker = f.read(1)
                    if not marker:
                        return None
                    m = marker[0]
                    if m in (0xD8, 0xD9, 0x01) or (0xD0 <= m <= 0xD7):
                        continue
                    seg_len_bytes = f.read(2)
                    if len(seg_len_bytes) < 2:
                        return None
                    seg_len = struct.unpack(">H", seg_len_bytes)[0]
                    if m in (
                        0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7,
                        0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF,
                    ):
                        sof = f.read(5)
                        if len(sof) < 5:
                            return None
                        h, w = struct.unpack(">HH", sof[1:5])
                        return (w, h)
                    f.seek(max(0, seg_len - 2), 1)
    except OSError:
        return None
    return None


def resolve_asset_file(assets_root: Path | None, rel: str) -> Path | None:
    if assets_root is None or not rel:
        return None
    norm = rel.replace("\\", "/").lstrip("/")
    direct = assets_root / norm
    if direct.is_file():
        return direct
    # Case-insensitive fallback on Linux/macOS
    parts = norm.split("/")
    cur = assets_root
    for part in parts:
        if not cur.is_dir():
            return None
        match = None
        low = part.lower()
        for child in cur.iterdir():
            if child.name.lower() == low:
                match = child
                break
        if match is None:
            return None
        cur = match
    return cur if cur.is_file() else None


def extract_run_condition(ev: dict[str, Any]) -> tuple[int, int, int, int]:
    """Read [run_cond, op, var, val] from converter/parse/commonevent.py schema."""
    rc_list = ev.get("run_condition_op_var_val")
    if isinstance(rc_list, list) and len(rc_list) >= 4:
        return (int(rc_list[0]), int(rc_list[1]), int(rc_list[2]), signed32(rc_list[3]))
    return (
        int(ev.get("run_condition", 0)),
        int(ev.get("condition_operator", 0)),
        int(ev.get("condition_variable", 0)),
        signed32(ev.get("condition_value", 0)),
    )


def iter_all_programs(data: dict[str, Any]):
    for ev in data.get("common_events", []):
        yield "ce", int(ev.get("id", -1)), str(ev.get("title", "")), ev
    for m in data.get("maps", []):
        map_id = int(m.get("id", -1))
        map_file = str(m.get("file", f"map{map_id}"))
        for ev in m.get("events", []):
            yield f"map{map_id}({map_file})", int(ev.get("id", -1)), str(ev.get("title", "")), ev


def scan_programs(data: dict[str, Any], assets_root: Path | None) -> dict[str, Any]:
    # 1. AutoEvents (RunCondition == 2)
    auto_events: list[dict[str, Any]] = []
    for ev in data.get("common_events", []):
        rc, op, var, val = extract_run_condition(ev)
        if rc != 2:
            continue
        slots = ev.get("slots") or []
        ops = [int(s.get("op", 0)) for s in slots if int(s.get("op", 0)) != 0]
        has_loop = any(o in (170, 179) for o in ops)
        has_wait = any(o == 180 for o in ops)
        has_break = any(o in (172, 173) for o in ops)
        max_wait = 0
        for s in slots:
            if int(s.get("op", 0)) == 180:
                params = s.get("params") or []
                if params:
                    w = signed32(params[0])
                    if not is_var_magic(w) and w > max_wait:
                        max_wait = w
        if has_loop:
            classification = "has_internal_loop"
        elif max_wait >= 99999 or has_break:
            classification = "one_shot_or_long_wait_suspect"
        else:
            classification = "no_loop_linear_BUG25_candidate"
        auto_events.append({
            "id": int(ev.get("id", -1)),
            "title": str(ev.get("title", "")),
            "operator": op,
            "variable": var,
            "value": val,
            "real_instructions": len(ops),
            "has_loop_170_179": has_loop,
            "has_wait_180": has_wait,
            "max_literal_wait": max_wait,
            "has_break_172": has_break,
            "classification": classification,
        })

    # 2. High-risk VM opcodes
    op176_sites: list[dict[str, Any]] = []
    op211_sites: list[dict[str, Any]] = []
    op125_sites: list[dict[str, Any]] = []
    op151_sites: list[dict[str, Any]] = []
    op130_magic_sites: list[dict[str, Any]] = []
    op140_magic_sites: list[dict[str, Any]] = []
    op180_nonpos_sites: list[dict[str, Any]] = []
    op121_stats: Counter = Counter()
    op121_flag_samples: dict[str, list[dict[str, Any]]] = defaultdict(list)

    # 3. Opcode 124
    op124_counts: Counter = Counter()
    op124_samples: dict[str, list[dict[str, Any]]] = defaultdict(list)

    # 4. Opcode 150, 160/161/162, 290
    op150_total = 0
    op150_backdrop_sites: list[dict[str, Any]] = []
    op150_mask_hits: list[dict[str, Any]] = []
    op150_blend_counts: Counter = Counter()
    op150_fade_nonzero = 0
    op16x_counts: Counter = Counter()
    op290_kind_counts: Counter = Counter()
    asset_size_cache: dict[str, tuple[int, int] | None] = {}

    def record_sample(bucket: dict[str, list[dict[str, Any]]], key: str, item: dict[str, Any], limit: int = 8) -> None:
        if len(bucket[key]) < limit:
            bucket[key].append(item)

    for scope, eid, title, ev in iter_all_programs(data):
        slots = ev.get("slots") or []
        loop_depth = 0
        for s in slots:
            op = int(s.get("op", 0))
            if op == 0:
                continue
            pc = int(s.get("pc", -1))
            params = s.get("params") or []
            strings = s.get("strings") or []

            if op in (170, 179):
                loop_depth += 1
            elif op == 498:
                loop_depth = max(0, loop_depth - 1)

            if op == 176:
                op176_sites.append({
                    "scope": scope, "event_id": eid, "title": title, "pc": pc,
                    "loop_depth_excluding_176": loop_depth,
                    "params": params,
                })

            elif op == 211:
                op211_sites.append({
                    "scope": scope, "event_id": eid, "title": title, "pc": pc,
                    "raw_target": signed32(params[0]) if params else None,
                    "param_count": len(params),
                    "params": [signed32(p) for p in params[:6]],
                    "strings": strings[:4],
                })

            elif op == 125:
                op125_sites.append({
                    "scope": scope, "event_id": eid, "title": title, "pc": pc,
                    "mask": unsigned32(params[0]) if params else 0,
                    "params": params,
                })

            elif op == 151:
                color = signed32(params[0]) if len(params) > 0 else 0
                frames = signed32(params[1]) if len(params) > 1 else 0
                op151_sites.append({
                    "scope": scope, "event_id": eid, "title": title, "pc": pc,
                    "color_raw": color,
                    "frames_raw": frames,
                    "frames_is_magic": is_var_magic(frames),
                })

            elif op == 130:
                magic_idx = [i for i in range(min(4, len(params))) if is_var_magic(params[i])]
                if magic_idx:
                    op130_magic_sites.append({
                        "scope": scope, "event_id": eid, "title": title, "pc": pc,
                        "magic_param_indices": magic_idx,
                        "params": [signed32(p) for p in params[:5]],
                    })

            elif op == 140:
                check_indices = [i for i in (1, 4, 5, 6) if i < len(params) and is_var_magic(params[i])]
                if check_indices:
                    op140_magic_sites.append({
                        "scope": scope, "event_id": eid, "title": title, "pc": pc,
                        "magic_param_indices": check_indices,
                        "params": [signed32(p) for p in params[:7]],
                        "file": strings[0] if strings else "",
                    })

            elif op == 180:
                frames = signed32(params[0]) if params else 1
                if not is_var_magic(frames) and frames <= 0:
                    op180_nonpos_sites.append({
                        "scope": scope, "event_id": eid, "title": title, "pc": pc,
                        "frames": frames,
                    })

            elif op == 121:
                op121_stats["total"] += 1
                bits = unsigned32(params[3]) if len(params) > 3 else 0
                flags = {
                    "bit_0x01_clamp": (bits & 0x01) != 0,
                    "bit_0x02_f64_real": (bits & 0x02) != 0,
                    "bit_0x04_raw_rhs_a": (bits & 0x04) != 0,
                    "bit_0x08_raw_rhs_b": (bits & 0x08) != 0,
                    "bit_0x10_ptr_target": (bits & 0x10) != 0,
                    "bit_0x20_ptr_a": (bits & 0x20) != 0,
                    "bit_0x40_ptr_b": (bits & 0x40) != 0,
                    "bit_0x10000_batch": (bits & 0x10000) != 0,
                    "compound_0x600_min_floor": (bits & 0x0F00) == 0x600,
                    "compound_0x700_max_ceil": (bits & 0x0F00) == 0x700,
                }
                for fname, active in flags.items():
                    if active:
                        op121_stats[fname] += 1
                        record_sample(op121_flag_samples, fname, {
                            "scope": scope, "event_id": eid, "pc": pc,
                            "bits_hex": f"0x{bits:X}",
                            "params": [signed32(p) for p in params[:5]],
                        })

            elif op == 124:
                sub = unsigned32(params[1]) if len(params) > 1 else 0
                field = signed32(params[3]) if len(params) >= 4 else (signed32(params[2]) if len(params) > 2 else 0)
                key = f"0x{sub:04X}:{field}"
                op124_counts[key] += 1
                record_sample(op124_samples, key, {
                    "scope": scope, "event_id": eid, "title": title, "pc": pc,
                    "target": signed32(params[0]) if params else 0,
                    "arg2": signed32(params[2]) if len(params) > 2 else None,
                }, limit=5)

            elif op == 150:
                op150_total += 1
                head = unsigned32(params[0]) if params else 0
                action = head & 0x0F
                kind = (head >> 4) & 0x07
                blend = (head >> 8) & 0x0F
                op150_blend_counts[blend] += 1
                num = signed32(params[1]) if len(params) > 1 else 0
                file_str = strings[0] if strings else ""
                # Duration/fade frames typically in args[8] (params[10]) or args[9] depending on action
                if len(params) > 10 and signed32(params[10]) > 0 and not is_var_magic(params[10]):
                    op150_fade_nonzero += 1

                # Check 0x0F00_0000 mask collision in params[2..] (EventVm.cs:791)
                for idx, raw_p in enumerate(params[2:]):
                    u = unsigned32(raw_p)
                    if (u & 0x0F00_0000) != 0:
                        s_val = signed32(raw_p)
                        op150_mask_hits.append({
                            "scope": scope, "event_id": eid, "title": title, "pc": pc,
                            "arg_index": idx,
                            "raw_signed": s_val,
                            "raw_hex": f"0x{u:08X}",
                            "likely_db_magic": s_val >= DB_MAGIC_MIN,
                            "action": action,
                            "kind": kind,
                        })

                # Check number < 0 or >= 12000 (not variable magic)
                if not is_var_magic(num) and (num < 0 or num >= 12000):
                    img_size = None
                    if kind in (0, 3) and file_str:
                        if file_str not in asset_size_cache:
                            resolved = resolve_asset_file(assets_root, file_str)
                            asset_size_cache[file_str] = sniff_image_size(resolved) if resolved else None
                        img_size = asset_size_cache[file_str]
                    op150_backdrop_sites.append({
                        "scope": scope, "event_id": eid, "title": title, "pc": pc,
                        "number": num,
                        "action": action,
                        "kind": kind,
                        "file": file_str,
                        "x": signed32(params[7]) if len(params) > 7 else None,
                        "y": signed32(params[8]) if len(params) > 8 else None,
                        "scale": signed32(params[9]) if len(params) > 9 else None,
                        "image_size": list(img_size) if img_size else None,
                        "non_fullscreen_asset": (img_size is not None and img_size != list(SCREEN_SIZE) and img_size != SCREEN_SIZE),
                    })

            elif op in (160, 161, 162):
                op16x_counts[op] += 1

            elif op == 290:
                eff_head = signed32(params[0]) if params else 0
                eff_kind = eff_head >> 4 if not is_var_magic(eff_head) else -1
                op290_kind_counts[eff_kind] += 1

    # Summarize 124 gaps
    op124_summary = []
    for key, cnt in op124_counts.most_common():
        sub_s, field_s = key.split(":")
        sub_v = int(sub_s, 16)
        field_v = int(field_s)
        status = "ok"
        if sub_v == 0x3000:
            if field_v in ENGINE_STATE_IMPLEMENTED_IN_HOST:
                status = "implemented_host"
            elif field_v in ENGINE_STATE_IMPLEMENTED_IN_VM:
                status = "partial_vm_no_parallel_distinction"
            else:
                status = "GAP_returns_0"
        elif sub_v in (0x4000, 0x4400) and field_v in (2, 3):
            status = "GAP_returns_scale_not_px"
        elif sub_v in (0x1000, 0x1200):
            status = "GAP_character_state_null"
        op124_summary.append({
            "sub": sub_s,
            "field": field_v,
            "count": cnt,
            "status": status,
            "samples": op124_samples[key],
        })

    return {
        "totals": data.get("totals", {}),
        "scan1_auto_events_rc2": {
            "total": len(auto_events),
            "by_classification": dict(Counter(e["classification"] for e in auto_events)),
            "events": auto_events,
        },
        "scan2_vm_opcodes": {
            "op176_continue_loop": {"count": len(op176_sites), "sites": op176_sites},
            "op211_reserve": {"count": len(op211_sites), "sites": op211_sites},
            "op125_auto_keys": {"count": len(op125_sites), "sites": op125_sites},
            "op151_change_color": {
                "count": len(op151_sites),
                "frames_is_magic_count": sum(1 for x in op151_sites if x["frames_is_magic"]),
                "sites": op151_sites,
            },
            "op130_teleport_magic_params": {"count": len(op130_magic_sites), "sites": op130_magic_sites[:100]},
            "op140_sound_magic_params": {"count": len(op140_magic_sites), "sites": op140_magic_sites[:100]},
            "op180_wait_nonpositive": {"count": len(op180_nonpos_sites), "sites": op180_nonpos_sites[:100]},
            "op121_bitfields": {
                "counts": dict(op121_stats),
                "samples": dict(op121_flag_samples),
            },
        },
        "scan3_op124_state_queries": {
            "total": sum(op124_counts.values()),
            "breakdown": op124_summary,
        },
        "scan4_op150_and_effects": {
            "op150_total": op150_total,
            "op150_blend_counts": {str(k): v for k, v in sorted(op150_blend_counts.items())},
            "op150_fade_nonzero_approx": op150_fade_nonzero,
            "op150_mask_0x0F000000_hits": {
                "total": len(op150_mask_hits),
                "likely_db_magic": sum(1 for x in op150_mask_hits if x["likely_db_magic"]),
                "samples": op150_mask_hits[:100],
            },
            "op150_backdrop_num_lt0_or_ge12000": {
                "total": len(op150_backdrop_sites),
                "non_fullscreen_assets": sum(1 for x in op150_backdrop_sites if x["non_fullscreen_asset"]),
                "sites": op150_backdrop_sites[:200],
            },
            "op16x_transitions": {str(k): v for k, v in sorted(op16x_counts.items())},
            "op290_effects_by_kind": {str(k): v for k, v in sorted(op290_kind_counts.items())},
        },
    }


def render_markdown(report: dict[str, Any], source: str) -> str:
    s1 = report["scan1_auto_events_rc2"]
    s2 = report["scan2_vm_opcodes"]
    s3 = report["scan3_op124_state_queries"]
    s4 = report["scan4_op150_and_effects"]
    lines: list[str] = [
        "# imoto-life 第一阶段静态摸底扫描报告",
        "",
        f"- **数据源**：`{source}`",
        f"- **总计**：`{json.dumps(report['totals'], ensure_ascii=False)}`",
        "",
        "## 1. `RunCondition == 2` 并行公共事件（BUG-25 电平触发摸底）",
        "",
        f"- 总数：{s1['total']}，分类统计：`{json.dumps(s1['by_classification'], ensure_ascii=False)}`",
        "",
        "| CE ID | 标题 | 条件(op/var/val) | 指令数 | 含170/179循环 | 含180等待 | 分类 |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for ev in s1["events"]:
        lines.append(
            f"| {ev['id']} | {ev['title']} | op={ev['operator']} v={ev['variable']} val={ev['value']} "
            f"| {ev['real_instructions']} | {ev['has_loop_170_179']} | {ev['has_wait_180']} | `{ev['classification']}` |"
        )

    lines += [
        "",
        "## 2. 高危 VM 指令命中统计",
        "",
        "| 检查项 | 命中数 | 说明 |",
        "| --- | --- | --- |",
        f"| `Opcode 176` (ContinueLoop) | {s2['op176_continue_loop']['count']} | 需确认 `Program.cs` 不再当作循环头 |",
        f"| `Opcode 211` (Reserve) | {s2['op211_reserve']['count']} | `VmController.Reserve` 当前为空桩 |",
        f"| `Opcode 125` (AutoKeys) | {s2['op125_auto_keys']['count']} | 当前无 2 帧 TTL 与同帧消费锁 |",
        f"| `Opcode 151` (ChangeColor) | {s2['op151_change_color']['count']} (frames 为 magic: {s2['op151_change_color']['frames_is_magic_count']}) | `frames` 当前未 `EvalNum` |",
        f"| `Opcode 130` (Teleport 含 magic 参数) | {s2['op130_teleport_magic_params']['count']} | 当前四参未 `EvalNum` |",
        f"| `Opcode 140` (Sound 含 magic 参数) | {s2['op140_sound_magic_params']['count']} | 当前 volume/frames/tempo/loopPoint 未 `EvalNum` |",
        f"| `Opcode 180` (Wait <= 0 字面量) | {s2['op180_wait_nonpositive']['count']} | 当前 `Math.Max(1, frames)` 会强行停 1 帧 |",
        "",
        "### `Opcode 121`（SetVariable）位域命中统计",
        "",
        f"`{json.dumps(s2['op121_bitfields']['counts'], ensure_ascii=False)}`",
        "",
        "## 3. `Opcode 124` 状态查询 `(sub, field)` 分布",
        "",
        "| sub | field | 次数 | 当前状态 |",
        "| --- | --- | --- | --- |",
    ]
    for row in s3["breakdown"]:
        lines.append(f"| `{row['sub']}` | {row['field']} | {row['count']} | `{row['status']}` |")

    lines += [
        "",
        "## 4. `Opcode 150` / `160-162` / `290` 渲染与特效摸底",
        "",
        f"- `Opcode 150` 总调用：{s4['op150_total']}；混合模式分布：`{json.dumps(s4['op150_blend_counts'])}`；非零渐变帧数（近似）：{s4['op150_fade_nonzero_approx']}",
        f"- `0x0F00_0000` 掩码命中：{s4['op150_mask_0x0F000000_hits']['total']}（疑似 `1_000_000_000+` 数据库 magic：{s4['op150_mask_0x0F000000_hits']['likely_db_magic']}）",
        f"- `number < 0` 或 `>= 12000` 站点：{s4['op150_backdrop_num_lt0_or_ge12000']['total']}（实测非 `1024x576` 素材：{s4['op150_backdrop_num_lt0_or_ge12000']['non_fullscreen_assets']}）",
        f"- `Opcode 160/161/162` 转场分布：`{json.dumps(s4['op16x_transitions'])}`",
        f"- `Opcode 290` 特效按 `kind` 分布：`{json.dumps(s4['op290_effects_by_kind'])}`",
        "",
    ]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="Read-only static audit for imoto-life programs.json")
    parser.add_argument("--programs", default="game_data/programs.json", help="Path to game_data/programs.json")
    parser.add_argument("--assets", default="", help="Optional path to unpacked retail Data/ directory for image size checks")
    parser.add_argument("--out-dir", default="reports/static_audit", help="Directory to write JSON and Markdown reports")
    args = parser.parse_args()

    src = Path(args.programs)
    if not src.is_file():
        raise SystemExit(f"ERROR: programs.json not found at {src}")

    assets_root = Path(args.assets) if args.assets else None
    if assets_root is not None and not assets_root.is_dir():
        print(f"WARN: --assets directory not found ({assets_root}); skipping image dimension checks")
        assets_root = None

    data = json.loads(src.read_text(encoding="utf-8"))
    report = scan_programs(data, assets_root)

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    json_path = out_dir / "static_audit_report.json"
    md_path = out_dir / "static_audit_summary.md"
    json_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    md_path.write_text(render_markdown(report, str(src)), encoding="utf-8")

    s1 = report["scan1_auto_events_rc2"]
    s2 = report["scan2_vm_opcodes"]
    s3 = report["scan3_op124_state_queries"]
    s4 = report["scan4_op150_and_effects"]
    print(
        "STATIC_AUDIT_OK "
        f"rc2={s1['total']} op176={s2['op176_continue_loop']['count']} "
        f"op211={s2['op211_reserve']['count']} op125={s2['op125_auto_keys']['count']} "
        f"op151={s2['op151_change_color']['count']} op124={s3['total']} "
        f"op150={s4['op150_total']} mask_db={s4['op150_mask_0x0F000000_hits']['likely_db_magic']} "
        f"out={out_dir}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())