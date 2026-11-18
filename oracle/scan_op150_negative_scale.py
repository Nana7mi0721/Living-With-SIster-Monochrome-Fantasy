#!/usr/bin/env python3
"""Full opcode 150 negative-scale scan for the retail content set.

The -1000000 value is WOLF's "field unchanged" sentinel, never a mirror. Every
other negative scale must be an image/window picture; text kinds (2 and 4) are
rejected because PictureStage mirrors the whole holder and would reverse glyphs
(R7-NEW-03).
"""
from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path

UNCHANGED = -1_000_000
EXPECTED = {
    "total": 20425,
    "sentinel": 474,
    "negative": 63,
    "draw": 57,
    "move": 6,
    "text": 0,
    "by_scale": {-100: 53, -200: 7, -1300: 3},
}


def signed32(value: int) -> int:
    return value - 2**32 if value >= 2**31 else value


def scan_slots(slots, scope: str, event_id: int, sites: list[dict], counts: Counter) -> None:
    for slot in slots:
        if int(slot.get("op", -1)) != 150:
            continue
        counts["total"] += 1
        params = slot.get("params", [])
        scale = signed32(int(params[9])) if len(params) > 9 else 100
        if scale == UNCHANGED:
            counts["sentinel"] += 1
            continue
        if scale >= 0:
            continue
        counts["negative"] += 1
        head = int(params[0]) if params else 0
        action = head & 0xF
        kind = (head >> 4) & 7
        anchor = (head >> 12) & 7
        counts["draw" if action == 0 else "move" if action == 1 else "other_action"] += 1
        counts["text" if kind in (2, 4) else "image"] += 1
        counts[f"scale_{scale}"] += 1
        strings = slot.get("strings") or []
        sites.append({
            "scope": scope,
            "event_id": event_id,
            "pc": int(slot.get("pc", -1)),
            "action": action,
            "kind": kind,
            "anchor": anchor,
            "scale": scale,
            "number": signed32(int(params[1])) if len(params) > 1 else None,
            "file": strings[0] if strings else "",
        })


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--programs", default="game_data/programs.json")
    parser.add_argument("--report", default="reports/op150_negative_scale_scan.json")
    args = parser.parse_args()

    source = Path(args.programs)
    data = json.loads(source.read_text(encoding="utf-8"))
    sites: list[dict] = []
    counts: Counter = Counter()

    for event in data.get("common_events", []):
        scan_slots(event.get("slots", []), "ce", int(event.get("id", -1)), sites, counts)
    for map_data in data.get("maps", []):
        map_id = int(map_data.get("id", -1))
        for event in map_data.get("events", []):
            scan_slots(event.get("slots", []), f"map{map_id}", int(event.get("id", -1)), sites, counts)

    failures: list[str] = []
    for key in ("total", "sentinel", "negative", "draw", "move", "text"):
        if counts[key] != EXPECTED[key]:
            failures.append(f"{key}={counts[key]} expected {EXPECTED[key]}")
    for scale, expected in EXPECTED["by_scale"].items():
        actual = counts[f"scale_{scale}"]
        if actual != expected:
            failures.append(f"scale {scale}={actual} expected {expected}")
    for site in sites:
        if site["action"] not in (0, 1):
            failures.append(f"{site['scope']} ev{site['event_id']} pc{site['pc']}: action={site['action']}")
        if site["kind"] not in (0, 3):
            failures.append(f"{site['scope']} ev{site['event_id']} pc{site['pc']}: kind={site['kind']} text/window-string mirror")
        if site["action"] == 0 and site["kind"] == 0 and not site["file"]:
            failures.append(f"{site['scope']} ev{site['event_id']} pc{site['pc']}: missing source file")

    report = {
        "source": str(source),
        "total_op150": counts["total"],
        "sentinel_unchanged": counts["sentinel"],
        "negative_scales": counts["negative"],
        "negative_by_action": {"draw": counts["draw"], "move": counts["move"], "other": counts["other_action"]},
        "negative_by_kind": {"image": counts["image"], "text": counts["text"]},
        "negative_by_scale": {str(k): counts[f"scale_{k}"] for k in EXPECTED["by_scale"]},
        "sites": sites,
        "failures": failures,
    }
    report_path = Path(args.report)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    if failures:
        for failure in failures:
            print("OP150_NEGATIVE_SCALE_SCAN_FAIL " + failure)
        return 1
    print(
        "OP150_NEGATIVE_SCALE_SCAN_OK "
        f"total={counts['total']} sentinel={counts['sentinel']} negative={counts['negative']} "
        f"draw={counts['draw']} move={counts['move']} text={counts['text']} "
        f"m100={counts['scale_-100']} m200={counts['scale_-200']} m1300={counts['scale_-1300']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
