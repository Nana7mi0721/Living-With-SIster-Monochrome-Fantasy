#!/usr/bin/env python3
"""Offline unit tests for the HD art pipeline helpers (needs Pillow and numpy)."""

from __future__ import annotations

import contextlib
import io
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))

try:
    from PIL import Image

    import upscale_assets
    import verify_hd_manifest
except ImportError as exc:  # pragma: no cover - dependency gate
    raise SystemExit(f"HD_TOOLS_TEST_SKIPPED missing dependency: {exc}")


def _save(path: Path, size: tuple[int, int], mode: str = "RGB") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    Image.new(mode, size, (10, 20, 30, 255) if mode == "RGBA" else (10, 20, 30)).save(path)


def _write_pack(
    base: Path,
    live_lines: list[str],
    entries: dict[str, dict],
    skipped: list[dict] | None = None,
) -> dict[str, str]:
    hd = base / "hd"
    hd.mkdir(parents=True, exist_ok=True)
    (hd / "hd_manifest.json").write_text(json.dumps({"entries": entries}), encoding="utf-8")
    manifest = base / "media_manifest.txt"
    manifest.write_text("\n".join(live_lines) + "\n", encoding="utf-8")
    skipped_path = base / "skipped.json"
    skipped_path.write_text(json.dumps(skipped or []), encoding="utf-8")
    return {
        "--hd": str(hd),
        "--root": str(base / "retail"),
        "--manifest": str(manifest),
        "--skipped": str(skipped_path),
    }


def _run_verify(paths: dict[str, str]) -> tuple[int, str]:
    argv = [item for pair in paths.items() for item in pair]
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        rc = verify_hd_manifest.main(argv)
    return rc, out.getvalue()


class VerifyHdManifestTest(unittest.TestCase):
    def test_clean_pack_passes_and_never_prints_fail(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            _save(base / "retail" / "UI" / "a.png", (4, 4))
            _save(base / "hd" / "ui" / "a.png", (8, 8))
            paths = _write_pack(
                base,
                ["# header comment", "", "UI\\a.png"],
                {"ui/a.png": {"path": "ui/a.png", "scale": 2.0}},
            )
            rc, text = _run_verify(paths)
            self.assertEqual(rc, 0)
            self.assertIn("HD_MANIFEST_VERIFY_OK", text)
            self.assertNotIn("HD_MANIFEST_VERIFY_FAIL", text)

    def test_degraded_alpha_fails_with_fail_marker(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            src = base / "retail" / "cg" / "b.png"
            src.parent.mkdir(parents=True)
            image = Image.new("RGBA", (4, 4), (10, 20, 30, 255))
            image.putpixel((0, 0), (10, 20, 30, 0))
            image.save(src)
            _save(base / "hd" / "cg" / "b.png", (8, 8), "RGB")
            paths = _write_pack(
                base, ["cg/b.png"], {"cg/b.png": {"path": "cg/b.png", "scale": 2.0}}
            )
            rc, text = _run_verify(paths)
            self.assertEqual(rc, 1)
            self.assertIn("degraded: cg/b.png: alpha lost", text)
            self.assertIn("HD_MANIFEST_VERIFY_FAIL", text)
            self.assertNotIn("HD_MANIFEST_VERIFY_OK", text)

    def test_uncovered_live_entry_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            paths = _write_pack(base, ["cg/missing.png"], {})
            rc, text = _run_verify(paths)
            self.assertEqual(rc, 1)
            self.assertIn("uncovered=1", text)
            self.assertIn("HD_MANIFEST_VERIFY_FAIL", text)

    def test_mixed_case_source_resolves_on_case_sensitive_trees(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            _save(base / "retail" / "Picture" / "Stand_A.PNG", (4, 4))
            _save(base / "hd" / "picture" / "stand_a.png", (8, 8))
            paths = _write_pack(
                base,
                ["Picture/Stand_A.PNG"],
                {"picture/stand_a.png": {"path": "picture/stand_a.png", "scale": 2.0}},
            )
            rc, text = _run_verify(paths)
            self.assertEqual(rc, 0, text)
            self.assertIn("mismatch=0", text)

    def test_comment_lines_are_not_live_entries(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            manifest = Path(tmp) / "m.txt"
            manifest.write_text("# note.png\n  \n#x/y.png\nA\\b.png\ndata.bin\n", encoding="utf-8")
            self.assertEqual(verify_hd_manifest.read_live_manifest(manifest), ["A/b.png"])

    def test_malformed_manifest_entries_fail_cleanly(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            paths = _write_pack(base, [], {})
            (base / "hd" / "hd_manifest.json").write_text('{"entries": null}', encoding="utf-8")
            rc, text = _run_verify(paths)
            self.assertEqual(rc, 1)
            self.assertIn("HD_MANIFEST_VERIFY_FAIL", text)


class UpscaleAssetsHelpersTest(unittest.TestCase):
    def test_engine_dir_resolution_has_no_builtin_default(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            with mock.patch.dict(os.environ, {}, clear=False) as env, mock.patch.object(
                upscale_assets.shutil, "which", return_value=None
            ):
                env.pop(upscale_assets.ENGINE_ENV, None)
                self.assertEqual(upscale_assets.resolve_engine_dir(""), "")
                self.assertEqual(upscale_assets.resolve_engine_dir("cli"), "cli")
                env[upscale_assets.ENGINE_ENV] = tmp
                self.assertEqual(upscale_assets.resolve_engine_dir(""), tmp)
                self.assertEqual(upscale_assets.resolve_engine_dir("cli"), "cli")
            with mock.patch.dict(os.environ, {upscale_assets.ENGINE_ENV: ""}), mock.patch.object(
                upscale_assets.shutil, "which", return_value=str(Path(tmp) / "engine.exe")
            ):
                self.assertEqual(
                    upscale_assets.resolve_engine_dir(""), str(Path(tmp).resolve())
                )

    def test_write_text_atomic_creates_parents_and_leaves_no_tmp(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / "work" / "out" / "hd_manifest.json"
            upscale_assets.write_text_atomic(target, "one")
            upscale_assets.write_text_atomic(target, "two")
            self.assertEqual(target.read_text(encoding="utf-8"), "two")
            self.assertEqual([p.name for p in target.parent.iterdir()], ["hd_manifest.json"])

    def test_encode_best_removes_stale_sibling_of_other_format(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out_dir = Path(tmp) / "cg"
            out_dir.mkdir()
            (out_dir / "x.png").write_bytes(b"stale")
            (out_dir / "x.webp").write_bytes(b"stale")
            image = Image.new("RGB", (2000, 2000), (5, 5, 5))
            best, _ = upscale_assets.encode_best(image, out_dir, "x")
            self.assertEqual(best.suffix, ".webp")
            self.assertEqual(sorted(p.name for p in out_dir.iterdir()), ["x.webp"])

    def test_denoise_for_jpg_background_is_conservative(self) -> None:
        self.assertEqual(upscale_assets.denoise_for(Path("CG/bg.jpg")), 0)
        self.assertEqual(upscale_assets.denoise_for(Path("Picture/a.png")), -1)


if __name__ == "__main__":
    unittest.main()
