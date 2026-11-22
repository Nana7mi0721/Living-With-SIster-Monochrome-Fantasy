#!/usr/bin/env python3
"""Patch one Godot export preset option atomically in godot/export_presets.cfg.

Godot has no CLI flag to override custom_template/release or keystore values per
invocation. Packaging scripts back up export_presets.cfg, patch cleanly, and
restore the original on EXIT.
"""
from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path


def fail(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)


def _preset_sections(lines: list[str]) -> list[tuple[int, int, str]]:
    sections: list[tuple[int, int, str]] = []
    start = -1
    name = ""
    for i, line in enumerate(lines):
        if line.startswith("[") and line.rstrip().endswith("]"):
            if start >= 0:
                sections.append((start, i, name))
            start = i
            name = line.strip()[1:-1]
    if start >= 0:
        sections.append((start, len(lines), name))
    return sections


def patch_option(path: Path, preset_name: str, key: str, value: str) -> None:
    lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
    sections = _preset_sections(lines)

    preset_header = ""
    for start, end, header in sections:
        if header.startswith("preset.") and not header.endswith(".options"):
            if any(line.strip() == f'name="{preset_name}"' for line in lines[start:end]):
                preset_header = header
                break
    if not preset_header:
        fail(f"export preset not found in {path}: {preset_name}")

    options_header = f"{preset_header}.options"
    opt_start = opt_end = -1
    for start, end, header in sections:
        if header == options_header:
            opt_start, opt_end = start, end
            break
    if opt_start < 0:
        fail(f"missing [{options_header}] section in {path}")

    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    replacement = f'{key}="{escaped}"\n'
    for idx in range(opt_start + 1, opt_end):
        if lines[idx].lstrip().startswith(f"{key}="):
            lines[idx] = replacement
            break
    else:
        lines.insert(opt_end, replacement)

    tmp = path.with_name(path.name + ".tmp")
    with open(tmp, "w", encoding="utf-8", newline="\n") as f:
        f.write("".join(lines))
    os.replace(tmp, path)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preset-file", required=True, type=Path)
    parser.add_argument("--preset", required=True)
    parser.add_argument("--set", dest="assignments", action="append", required=True, metavar="KEY=VALUE")
    args = parser.parse_args(argv)

    target = args.preset_file.resolve()
    for item in args.assignments:
        if "=" not in item:
            fail(f"expected KEY=VALUE, got: {item}")
        key, val = item.split("=", 1)
        patch_option(target, args.preset, key.strip(), val.strip())
    return 0


if __name__ == "__main__":
    sys.exit(main())
