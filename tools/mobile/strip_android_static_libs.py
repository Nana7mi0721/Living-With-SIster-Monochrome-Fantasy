#!/usr/bin/env python3
"""Remove compile-only Mono static archives from an exported Android APK."""
from __future__ import annotations

import argparse
import shutil
import zipfile
from pathlib import Path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args(argv)

    source = args.source.resolve()
    output = args.output.resolve()
    if source == output:
        raise SystemExit("source and output must differ")
    output.parent.mkdir(parents=True, exist_ok=True)

    removed_count = 0
    removed_bytes = 0
    with zipfile.ZipFile(source, "r") as src, zipfile.ZipFile(
        output, "w", compression=zipfile.ZIP_DEFLATED, allowZip64=True
    ) as dst:
        for info in src.infolist():
            if info.filename.startswith("lib/") and info.filename.endswith(".a"):
                removed_count += 1
                removed_bytes += info.file_size
                continue

            out_info = zipfile.ZipInfo(info.filename, info.date_time)
            out_info.compress_type = info.compress_type
            out_info.comment = info.comment
            out_info.extra = info.extra
            out_info.internal_attr = info.internal_attr
            out_info.external_attr = info.external_attr
            out_info.create_system = info.create_system
            out_info.flag_bits = info.flag_bits & ~0x08

            with src.open(info, "r") as source_handle, dst.open(out_info, "w") as target_handle:
                shutil.copyfileobj(source_handle, target_handle, length=1024 * 1024)

    print(f"REMOVED_STATIC_ARCHIVES count={removed_count} bytes={removed_bytes} output={output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
