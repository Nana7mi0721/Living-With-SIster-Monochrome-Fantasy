"""Diff the Rust and C# main-route traces (wolf.route).

Compares every step's (frame, map_id, event_id, pc, op), then its integer vars,
string vars and host effects. Reports the first divergence with enough context
to reproduce it.
"""

from __future__ import annotations

import argparse
import gzip
import json
import sys
from pathlib import Path


def load_document(path):
    if path.suffix == ".gz":
        with gzip.open(path, "rt", encoding="utf-8") as handle:
            return json.load(handle)
    return json.loads(path.read_text(encoding="utf-8"))


def normalize_effects(effects):
    """Keep the full effect payload: a missing key and an empty string are distinct.

    This must stay aligned with oracle/diff_trace.py (R5-P2-02).
    """
    return [dict(effect) for effect in effects]


def diff_steps(base, target):
    for index in range(min(len(base), len(target))):
        b = base[index]
        t = target[index]
        for field in ("frame", "map_id", "event_id", "pc", "op"):
            if b.get(field) != t.get(field):
                return {
                    "index": index,
                    "field": field,
                    "rust": b.get(field),
                    "csharp": t.get(field),
                    "rust_step": b,
                    "csharp_step": t,
                }
        b_vars = {str(k): v for k, v in b.get("vars", {}).items()}
        t_vars = {str(k): v for k, v in t.get("vars", {}).items()}
        if b_vars != t_vars:
            keys = sorted(set(b_vars) | set(t_vars))
            changed = {
                k: [b_vars.get(k), t_vars.get(k)]
                for k in keys
                if b_vars.get(k) != t_vars.get(k)
            }
            return {
                "index": index,
                "field": "vars",
                "rust": b_vars,
                "csharp": t_vars,
                "changed": changed,
                "rust_step": b,
                "csharp_step": t,
            }
        b_strs = {str(k): str(v) for k, v in b.get("str_vars", {}).items()}
        t_strs = {str(k): str(v) for k, v in t.get("str_vars", {}).items()}
        if b_strs != t_strs:
            return {
                "index": index,
                "field": "str_vars",
                "rust": b_strs,
                "csharp": t_strs,
                "rust_step": b,
                "csharp_step": t,
            }
        b_eff = normalize_effects(b.get("effects", []))
        t_eff = normalize_effects(t.get("effects", []))
        if b_eff != t_eff:
            return {
                "index": index,
                "field": "effects",
                "rust": b_eff,
                "csharp": t_eff,
                "rust_step": b,
                "csharp_step": t,
            }
    if len(base) != len(target):
        return {
            "index": min(len(base), len(target)),
            "field": "step_count",
            "rust": len(base),
            "csharp": len(target),
            "rust_step": None,
            "csharp_step": None,
        }
    return None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("rust")
    parser.add_argument("csharp")
    parser.add_argument("--report")
    args = parser.parse_args()

    base_doc = load_document(Path(args.rust))
    target_doc = load_document(Path(args.csharp))
    base = base_doc.get("steps", [])
    target = target_doc.get("steps", [])
    result = diff_steps(base, target)

    header = {
        "schema": "wolf.route.diff",
        "schema_version": 1,
        "rust_steps": len(base),
        "csharp_steps": len(target),
        "rust_frames": base_doc.get("frames"),
        "csharp_frames": target_doc.get("frames"),
        "matched_steps": len(base) if result is None else result["index"],
        "ok": result is None,
        "divergence": result,
    }
    if args.report:
        Path(args.report).write_text(
            json.dumps(header, ensure_ascii=False, indent=2), encoding="utf-8"
        )

    if result is None:
        print(
            f"ROUTE_DIFF_OK: {len(base)}/{len(target)} steps matched 100% "
            f"(frames {header['rust_frames']}/{header['csharp_frames']})"
        )
        return 0

    print(
        f"ROUTE_DIFF_FAIL: {result['index']}/{min(len(base), len(target))} steps matched; "
        f"field={result['field']}"
    )
    print(f"  rust:   {json.dumps(result['rust'])[:2000]}")
    print(f"  csharp: {json.dumps(result['csharp'])[:2000]}")
    if result.get("changed") is not None:
        print(f"  changed: {json.dumps(result['changed'])[:2000]}")
    if result.get("rust_step"):
        print("  rust step:   " + json.dumps(result["rust_step"])[:2000])
    if result.get("csharp_step"):
        print("  csharp step: " + json.dumps(result["csharp_step"])[:2000])
    return 1


if __name__ == "__main__":
    sys.exit(main())
