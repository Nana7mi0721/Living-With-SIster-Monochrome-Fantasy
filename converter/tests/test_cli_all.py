"""converter.cli 的 `all` 阶段测试（不读取真实 50MB 数据）。"""

from __future__ import annotations

import contextlib
import io
import os
import shutil
import tempfile
import unittest
from unittest import mock

from converter import cli


def _touch(path: str) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as handle:
        handle.write(b"")


def _make_data_root(base: str, missing: tuple[str, ...] = ()) -> str:
    """构造只含必需占位文件的合法 Data 目录。"""
    root = os.path.join(base, "Data")
    for rel in cli.REQUIRED_BASICDATA:
        if rel in missing:
            continue
        _touch(os.path.join(root, *rel.split("/")))
    os.makedirs(os.path.join(root, "MapData"), exist_ok=True)
    return root


class TestAllStage(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.base = self._tmp.name
        self.root = _make_data_root(self.base)
        self.out = os.path.join(self.base, "out")
        self.reports = os.path.join(self.base, "reports")

    def _run(self, argv: list[str]) -> tuple[int, str, str]:
        stdout = io.StringIO()
        stderr = io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            code = cli.main(argv)
        return code, stdout.getvalue(), stderr.getvalue()

    def _fake_stages(self, calls: list[tuple[str, str, str, str]], codes=None):
        codes = codes or {}

        def make(name: str):
            def fake(args) -> int:
                calls.append((name, args.root, args.out, args.reports))
                return codes.get(name, 0)

            return fake

        return mock.patch.dict(cli.STAGES, {name: make(name) for name in cli.ALL_STAGES})

    def _argv(self) -> list[str]:
        return [
            "all",
            "--root",
            self.root,
            "--out",
            self.out,
            "--reports",
            self.reports,
        ]

    def test_all_runs_stages_in_order_and_prints_marker(self) -> None:
        calls: list[tuple[str, str, str, str]] = []
        with self._fake_stages(calls):
            code, stdout, stderr = self._run(self._argv())

        self.assertEqual(code, 0)
        self.assertEqual([call[0] for call in calls], list(cli.ALL_STAGES))
        self.assertEqual([call[1:] for call in calls], [(self.root, self.out, self.reports)] * 5)
        self.assertIn("CONVERT_ALL_OK ", stdout)
        self.assertIn(
            f"CONVERT_ALL_OK root={self.root} out={self.out} reports={self.reports}",
            stdout,
        )
        self.assertIn("[1/5] programs ...", stdout)
        self.assertIn("[5/5] validate OK", stdout)
        self.assertEqual(stderr, "")

    def test_all_stops_at_first_failure(self) -> None:
        calls: list[tuple[str, str, str, str]] = []
        with self._fake_stages(calls, codes={"database": 7}):
            code, stdout, stderr = self._run(self._argv())

        self.assertEqual(code, 7)
        self.assertEqual([call[0] for call in calls], ["programs", "database"])
        self.assertNotIn("CONVERT_ALL_OK", stdout)
        self.assertIn("database FAILED (exit 7)", stderr)
        self.assertNotIn("map-tiles", stderr)

    def test_all_negative_stage_code_becomes_nonzero(self) -> None:
        calls: list[tuple[str, str, str, str]] = []
        with self._fake_stages(calls, codes={"programs": -2}):
            code, stdout, stderr = self._run(self._argv())

        self.assertEqual(code, 1)
        self.assertEqual([call[0] for call in calls], ["programs"])
        self.assertNotIn("CONVERT_ALL_OK", stdout)
        self.assertIn("programs FAILED (exit -2)", stderr)

    def test_all_missing_root_returns_2_without_running_stages(self) -> None:
        calls: list[tuple[str, str, str, str]] = []
        missing_root = os.path.join(self.base, "does-not-exist")
        with self._fake_stages(calls):
            code, stdout, stderr = self._run(
                ["all", "--root", missing_root, "--out", self.out, "--reports", self.reports]
            )

        self.assertEqual(code, 2)
        self.assertEqual(calls, [])
        self.assertIn("root directory not found", stderr)
        self.assertIn(missing_root, stderr)
        self.assertNotIn("CONVERT_ALL_OK", stdout)

    def test_all_missing_basicdata_returns_2_and_names_file(self) -> None:
        missing_base = self.base + "-missing"
        root = _make_data_root(missing_base, missing=("BasicData/CDataBase.dat",))
        self.addCleanup(shutil.rmtree, missing_base, ignore_errors=True)
        calls: list[tuple[str, str, str, str]] = []
        with self._fake_stages(calls):
            code, stdout, stderr = self._run(
                ["all", "--root", root, "--out", self.out, "--reports", self.reports]
            )

        self.assertEqual(code, 2)
        self.assertEqual(calls, [])
        self.assertIn("missing required BasicData file(s)", stderr)
        self.assertIn("BasicData/CDataBase.dat", stderr)
        self.assertNotIn("CONVERT_ALL_OK", stdout)

    def test_all_unwritable_out_returns_2_without_running_stages(self) -> None:
        blocker = os.path.join(self.base, "out-is-a-file")
        with open(blocker, "wb") as handle:
            handle.write(b"x")
        calls: list[tuple[str, str, str, str]] = []
        with self._fake_stages(calls):
            code, stdout, stderr = self._run(
                ["all", "--root", self.root, "--out", blocker, "--reports", self.reports]
            )

        self.assertEqual(code, 2)
        self.assertEqual(calls, [])
        self.assertIn("out directory not writable", stderr)
        self.assertNotIn("CONVERT_ALL_OK", stdout)

    def test_all_unwritable_reports_returns_2_without_running_stages(self) -> None:
        blocker = os.path.join(self.base, "reports-is-a-file")
        with open(blocker, "wb") as handle:
            handle.write(b"x")
        calls: list[tuple[str, str, str, str]] = []
        with self._fake_stages(calls):
            code, stdout, stderr = self._run(
                ["all", "--root", self.root, "--out", self.out, "--reports", blocker]
            )

        self.assertEqual(code, 2)
        self.assertEqual(calls, [])
        self.assertIn("reports directory not writable", stderr)
        self.assertNotIn("CONVERT_ALL_OK", stdout)

    def test_all_does_not_swallow_stage_exception(self) -> None:
        def boom(args) -> int:
            raise RuntimeError("stage exploded")

        with mock.patch.dict(cli.STAGES, {"programs": boom}):
            with self.assertRaises(RuntimeError):
                self._run(self._argv())


if __name__ == "__main__":
    unittest.main()
