#!/usr/bin/env python3
"""Verify the 2x art pack against the live manifest and the retail tree.

Checks, per entry: the packed file exists, its pixel size is exactly
``source size * scale``, and an alpha-bearing source did not lose its channel.
Then reports live images that are neither packed nor explained by the skip log.

Prints ``HD_MANIFEST_VERIFY_OK entries=N missing=0 mismatch=0 degraded=0 ...``
and exits 0 only when every counter is clean; otherwise prints
``HD_MANIFEST_VERIFY_FAIL ...`` with the same counters and exits 1.
"""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass, field
from pathlib import Path

from PIL import Image

TEXTURE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".bmp", ".webp"}


@dataclass
class VerifyReport:
    entries: int = 0
    missing_file: list[str] = field(default_factory=list)
    mismatch: list[str] = field(default_factory=list)
    degraded: list[str] = field(default_factory=list)
    uncovered: list[str] = field(default_factory=list)
    opaque_alpha_dropped: int = 0

    @property
    def ok(self) -> bool:
        return not (self.missing_file or self.mismatch or self.degraded or self.uncovered)


def read_live_manifest(path: Path) -> list[str]:
    """Texture entries of the retail manifest; blank and ``#`` comment lines are ignored."""
    live: list[str] = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        rel = line.replace("\\", "/")
        if Path(rel).suffix.lower() in TEXTURE_EXTENSIONS:
            live.append(rel)
    return live


def verify(hd_root: Path, root: Path, manifest_path: Path, skipped_path: Path) -> VerifyReport:
    manifest = json.loads((hd_root / "hd_manifest.json").read_text(encoding="utf-8"))
    entries = manifest.get("entries", {})
    if not isinstance(entries, dict):
        raise ValueError("hd_manifest.json 'entries' is not an object")

    live = read_live_manifest(manifest_path)
    # Manifest keys are lower-case; retail files keep their on-disk case.
    source_by_lower = {rel.lower(): rel for rel in live}

    report = VerifyReport(entries=len(entries))
    for key, item in entries.items():
        if not isinstance(item, dict):
            report.mismatch.append(f"{key}: entry is not an object")
            continue
        out = hd_root / str(item.get("path", ""))
        src = root / source_by_lower.get(key.lower(), key)
        if not out.is_file():
            report.missing_file.append(key)
            continue
        if not src.is_file():
            report.mismatch.append(f"{key}: source missing")
            continue
        scale = float(item.get("scale", 2.0))
        with Image.open(src) as source, Image.open(out) as packed:
            expected = (round(source.width * scale), round(source.height * scale))
            if packed.size != expected:
                report.mismatch.append(f"{key}: {packed.size} != {expected}")
            if "A" in source.getbands() and "A" not in packed.getbands():
                alpha = source.convert("RGBA").getchannel("A")
                if alpha.getextrema() == (255, 255):
                    report.opaque_alpha_dropped += 1
                else:
                    report.degraded.append(f"{key}: alpha lost")

    explained: set[str] = set()
    if skipped_path.is_file():
        for item in json.loads(skipped_path.read_text(encoding="utf-8")):
            explained.add(str(item.get("path", "")).lower())
    report.uncovered = [
        rel for rel in live if rel.lower() not in entries and rel.lower() not in explained
    ]
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--hd", default="work/hd")
    parser.add_argument("--root", default="work/retail_dat")
    parser.add_argument("--manifest", default="work/media_manifest.txt")
    parser.add_argument("--skipped", default="work/out/hd_skipped.json")
    args = parser.parse_args(argv)

    try:
        report = verify(Path(args.hd), Path(args.root), Path(args.manifest), Path(args.skipped))
    except (OSError, ValueError) as error:
        print(f"HD_MANIFEST_VERIFY_FAIL error={error}")
        return 1

    for label, rows in (
        ("missing_file", report.missing_file),
        ("mismatch", report.mismatch),
        ("degraded", report.degraded),
        ("uncovered", report.uncovered),
    ):
        for row in rows[:10]:
            print(f"{label}: {row}")
    tag = "HD_MANIFEST_VERIFY_OK" if report.ok else "HD_MANIFEST_VERIFY_FAIL"
    print(
        f"{tag} entries={report.entries} missing={len(report.missing_file)} "
        f"mismatch={len(report.mismatch)} degraded={len(report.degraded)} "
        f"uncovered={len(report.uncovered)} opaque_alpha_dropped={report.opaque_alpha_dropped}"
    )
    return 0 if report.ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
