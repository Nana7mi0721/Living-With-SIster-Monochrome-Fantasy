#!/usr/bin/env python3
"""Verify the 2x art pack against the live manifest and the retail tree.

Checks, per entry: the packed file exists, its pixel size is exactly
``source size * scale``, and an alpha-bearing source did not lose its channel.
Then reports live images that are neither packed nor explained by the skip log.

Prints ``HD_MANIFEST_VERIFY_OK entries=N missing=0 mismatch=0 degraded=0``.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from PIL import Image

TEXTURE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".bmp", ".webp"}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--hd", default="work/hd")
    parser.add_argument("--root", default="work/retail_dat")
    parser.add_argument("--manifest", default="work/media_manifest.txt")
    parser.add_argument("--skipped", default="work/out/hd_skipped.json")
    args = parser.parse_args()

    hd_root = Path(args.hd)
    root = Path(args.root)
    manifest = json.loads((hd_root / "hd_manifest.json").read_text(encoding="utf-8"))
    entries: dict = manifest.get("entries", {})

    missing_file: list[str] = []
    mismatch: list[str] = []
    degraded: list[str] = []
    opaque_alpha_dropped = 0
    for key, item in entries.items():
        out = hd_root / str(item.get("path", ""))
        src = root / key
        if not out.is_file():
            missing_file.append(key)
            continue
        if not src.is_file():
            mismatch.append(f"{key}: source missing")
            continue
        scale = float(item.get("scale", 2.0))
        with Image.open(src) as source, Image.open(out) as packed:
            expected = (round(source.width * scale), round(source.height * scale))
            if packed.size != expected:
                mismatch.append(f"{key}: {packed.size} != {expected}")
            if "A" in source.getbands() and "A" not in packed.getbands():
                alpha = source.convert("RGBA").getchannel("A")
                if alpha.getextrema() == (255, 255):
                    opaque_alpha_dropped += 1
                else:
                    degraded.append(f"{key}: alpha lost")

    live = []
    for line in Path(args.manifest).read_text(encoding="utf-8").splitlines():
        rel = line.strip().replace("\\", "/")
        if rel and Path(rel).suffix.lower() in TEXTURE_EXTENSIONS:
            live.append(rel)
    explained = set()
    skipped_path = Path(args.skipped)
    if skipped_path.is_file():
        for item in json.loads(skipped_path.read_text(encoding="utf-8")):
            explained.add(str(item.get("path", "")).lower())
    uncovered = [rel for rel in live if rel.lower() not in entries and rel.lower() not in explained]

    for label, rows in (("missing_file", missing_file), ("mismatch", mismatch), ("degraded", degraded), ("uncovered", uncovered)):
        for row in rows[:10]:
            print(f"{label}: {row}")
    print(
        f"HD_MANIFEST_VERIFY_OK entries={len(entries)} missing={len(missing_file)} "
        f"mismatch={len(mismatch)} degraded={len(degraded)} uncovered={len(uncovered)} "
        f"opaque_alpha_dropped={opaque_alpha_dropped}"
    )
    return 0 if not (missing_file or mismatch or uncovered) else 1


if __name__ == "__main__":
    raise SystemExit(main())
