"""Unit tests for oracle trace diffing (M0)."""

from pathlib import Path
import json
import pytest

from oracle.diff_trace import diff_traces, TraceDiffResult


@pytest.fixture
def fixtures_dir() -> Path:
    return Path(__file__).resolve().parent.parent.parent / "oracle" / "fixtures"


def test_reference_trace_identical(fixtures_dir: Path):
    ref_path = fixtures_dir / "ce42_reference_trace.json"
    with open(ref_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    res = diff_traces(data, data)
    assert res.ok is True
    assert res.matched_steps == 10
    assert res.total_expected == 10
    assert res.divergence is None


def test_reference_trace_divergence(fixtures_dir: Path):
    ref_path = fixtures_dir / "ce42_reference_trace.json"
    corrupt_path = fixtures_dir / "ce42_corrupt_trace.json"
    with open(ref_path, "r", encoding="utf-8") as f:
        ref_data = json.load(f)
    with open(corrupt_path, "r", encoding="utf-8") as f:
        corrupt_data = json.load(f)

    res = diff_traces(ref_data, corrupt_data)
    assert res.ok is False
    assert res.divergence is not None
    assert res.divergence.step == 6
    assert res.divergence.field == "vars"


def test_schema_mismatch():
    base = {"schema": "wrong", "schema_version": 1, "event_id": 42, "steps": []}
    target = {"schema": "wolf.trace", "schema_version": 1, "event_id": 42, "steps": []}
    res = diff_traces(base, target)
    assert res.ok is False
    assert "Invalid schema" in (res.header_diff or "")
