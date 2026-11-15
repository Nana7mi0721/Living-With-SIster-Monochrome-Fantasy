"""Oracle Trace Diff tool (M0).

Compares two execution traces against the specification in oracle/TRACE_CONTRACT.md.
Validates step alignment, variables, and side effects.
Exits 0 on identical match, 1 on divergence.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional


@dataclass
class Divergence:
    step: int
    field: str
    expected: Any
    actual: Any
    context_expected: List[Dict[str, Any]] = field(default_factory=list)
    context_actual: List[Dict[str, Any]] = field(default_factory=list)


@dataclass
class TraceDiffResult:
    ok: bool
    total_expected: int
    total_actual: int
    matched_steps: int
    divergence: Optional[Divergence] = None
    header_diff: Optional[str] = None


def normalize_effects(effects: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Keep the full effect payload: a missing key and an empty string are distinct.

    This must stay aligned with oracle/diff_route.py so the single-event and route
    oracles enforce the same effect contract (R5-P2-02).
    """
    return [dict(eff) for eff in effects]


def diff_traces(base_data: Dict[str, Any], target_data: Dict[str, Any]) -> TraceDiffResult:
    """Compare two trace dictionaries according to TRACE_CONTRACT.md."""
    # 1. Header validations
    if base_data.get("schema") != "wolf.trace" or target_data.get("schema") != "wolf.trace":
        return TraceDiffResult(
            ok=False,
            total_expected=0,
            total_actual=0,
            matched_steps=0,
            header_diff=f"Invalid schema: base={base_data.get('schema')}, target={target_data.get('schema')}",
        )

    if base_data.get("schema_version") != target_data.get("schema_version"):
        return TraceDiffResult(
            ok=False,
            total_expected=0,
            total_actual=0,
            matched_steps=0,
            header_diff=f"Schema version mismatch: base={base_data.get('schema_version')}, target={target_data.get('schema_version')}",
        )

    if base_data.get("event_id") != target_data.get("event_id"):
        return TraceDiffResult(
            ok=False,
            total_expected=0,
            total_actual=0,
            matched_steps=0,
            header_diff=f"Event ID mismatch: base={base_data.get('event_id')}, target={target_data.get('event_id')}",
        )

    base_steps: List[Dict[str, Any]] = base_data.get("steps", [])
    target_steps: List[Dict[str, Any]] = target_data.get("steps", [])

    total_expected = len(base_steps)
    total_actual = len(target_steps)
    matched_steps = 0

    max_steps = max(total_expected, total_actual)
    for i in range(max_steps):
        if i >= total_expected:
            div = Divergence(
                step=i + 1,
                field="length",
                expected="<end of steps>",
                actual=f"extra step pc={target_steps[i].get('pc')} op={target_steps[i].get('op')}",
            )
            return TraceDiffResult(
                ok=False,
                total_expected=total_expected,
                total_actual=total_actual,
                matched_steps=matched_steps,
                divergence=div,
            )

        if i >= total_actual:
            div = Divergence(
                step=i + 1,
                field="length",
                expected=f"step pc={base_steps[i].get('pc')} op={base_steps[i].get('op')}",
                actual="<truncated / end of steps>",
            )
            return TraceDiffResult(
                ok=False,
                total_expected=total_expected,
                total_actual=total_actual,
                matched_steps=matched_steps,
                divergence=div,
            )

        b_step = base_steps[i]
        t_step = target_steps[i]

        # Check step identity
        for key in ["frame", "event_id", "pc", "op"]:
            if b_step.get(key) != t_step.get(key):
                ctx_start = max(0, i - 5)
                ctx_end = min(total_expected, i + 6)
                div = Divergence(
                    step=i + 1,
                    field=key,
                    expected=b_step.get(key),
                    actual=t_step.get(key),
                    context_expected=base_steps[ctx_start:ctx_end],
                    context_actual=target_steps[max(0, i - 5) : min(total_actual, i + 6)],
                )
                return TraceDiffResult(
                    ok=False,
                    total_expected=total_expected,
                    total_actual=total_actual,
                    matched_steps=matched_steps,
                    divergence=div,
                )

        # Check vars (string addr -> int32 value)
        b_vars = {str(k): int(v) for k, v in b_step.get("vars", {}).items()}
        t_vars = {str(k): int(v) for k, v in t_step.get("vars", {}).items()}
        if b_vars != t_vars:
            ctx_start = max(0, i - 5)
            ctx_end = min(total_expected, i + 6)
            div = Divergence(
                step=i + 1,
                field="vars",
                expected=b_vars,
                actual=t_vars,
                context_expected=base_steps[ctx_start:ctx_end],
                context_actual=target_steps[max(0, i - 5) : min(total_actual, i + 6)],
            )
            return TraceDiffResult(
                ok=False,
                total_expected=total_expected,
                total_actual=total_actual,
                matched_steps=matched_steps,
                divergence=div,
            )

        # Check string vars (address -> string value)
        b_strs = {str(k): str(v) for k, v in b_step.get("str_vars", {}).items()}
        t_strs = {str(k): str(v) for k, v in t_step.get("str_vars", {}).items()}
        if b_strs != t_strs:
            ctx_start = max(0, i - 5)
            ctx_end = min(total_expected, i + 6)
            div = Divergence(
                step=i + 1,
                field="str_vars",
                expected=b_strs,
                actual=t_strs,
                context_expected=base_steps[ctx_start:ctx_end],
                context_actual=target_steps[max(0, i - 5) : min(total_actual, i + 6)],
            )
            return TraceDiffResult(
                ok=False,
                total_expected=total_expected,
                total_actual=total_actual,
                matched_steps=matched_steps,
                divergence=div,
            )

        # Check effects
        b_eff = normalize_effects(b_step.get("effects", []))
        t_eff = normalize_effects(t_step.get("effects", []))
        if b_eff != t_eff:
            ctx_start = max(0, i - 5)
            ctx_end = min(total_expected, i + 6)
            div = Divergence(
                step=i + 1,
                field="effects",
                expected=b_eff,
                actual=t_eff,
                context_expected=base_steps[ctx_start:ctx_end],
                context_actual=target_steps[max(0, i - 5) : min(total_actual, i + 6)],
            )
            return TraceDiffResult(
                ok=False,
                total_expected=total_expected,
                total_actual=total_actual,
                matched_steps=matched_steps,
                divergence=div,
            )

        matched_steps += 1

    return TraceDiffResult(
        ok=True,
        total_expected=total_expected,
        total_actual=total_actual,
        matched_steps=matched_steps,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Diff two Wolf trace files against TRACE_CONTRACT.md")
    parser.add_argument("base", type=Path, help="Base/Reference trace JSON file")
    parser.add_argument("target", type=Path, help="Target trace JSON file")
    parser.add_argument("--report", type=Path, default=None, help="Path to write JSON diff report")
    args = parser.parse_args()

    if not args.base.exists():
        print(f"Error: base file not found: {args.base}", file=sys.stderr)
        return 2
    if not args.target.exists():
        print(f"Error: target file not found: {args.target}", file=sys.stderr)
        return 2

    with open(args.base, "r", encoding="utf-8") as f:
        base_data = json.load(f)
    with open(args.target, "r", encoding="utf-8") as f:
        target_data = json.load(f)

    res = diff_traces(base_data, target_data)

    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        report_data = {
            "ok": res.ok,
            "base": str(args.base),
            "target": str(args.target),
            "total_expected": res.total_expected,
            "total_actual": res.total_actual,
            "matched_steps": res.matched_steps,
            "header_diff": res.header_diff,
            "divergence": asdict(res.divergence) if res.divergence else None,
        }
        with open(args.report, "w", encoding="utf-8") as f:
            json.dump(report_data, f, indent=2, ensure_ascii=False)

    if res.ok:
        print(f"TRACE_DIFF_OK: {res.matched_steps}/{res.total_expected} steps matched 100% (event_id={base_data.get('event_id')})")
        return 0
    else:
        print("TRACE_DIFF_FAIL: Trace divergence detected!", file=sys.stderr)
        if res.header_diff:
            print(f"  Header mismatch: {res.header_diff}", file=sys.stderr)
        if res.divergence:
            d = res.divergence
            print(f"  Divergence at Step #{d.step} on field '{d.field}':", file=sys.stderr)
            print(f"    Expected: {d.expected}", file=sys.stderr)
            print(f"    Actual:   {d.actual}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
