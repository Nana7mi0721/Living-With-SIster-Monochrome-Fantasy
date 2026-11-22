#!/usr/bin/env python3
"""Lint docs/godot/DECISIONS.md numbering/section placement and docs/README.md links."""
from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote

ROW_RE = re.compile(r"^\|\s*`?(D-(\d+))`?\s*\|")
MD_LINK_RE = re.compile(r"\[[^\]]+\]\(([^)]+)\)")


def lint_decisions(decisions_path: Path) -> list[str]:
    errors: list[str] = []
    if not decisions_path.is_file():
        return [f"missing {decisions_path}"]
    section = ""
    seen: dict[str, int] = {}
    prev_num = 0
    prev_id = ""
    in_fence = False
    for lineno, line in enumerate(decisions_path.read_text(encoding="utf-8").splitlines(), 1):
        stripped = line.strip()
        if stripped.startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if stripped.startswith("## "):
            section = stripped
            continue
        m = ROW_RE.match(stripped)
        if not m:
            continue
        did = m.group(1)
        dnum = int(m.group(2))
        if not section.startswith("## 1."):
            errors.append(f"{decisions_path.name}:{lineno}: {did} placed under '{section}' instead of '## 1.'")
        if did in seen:
            errors.append(f"{decisions_path.name}:{lineno}: duplicate decision id {did} (first at line {seen[did]})")
        else:
            seen[did] = lineno
        if dnum <= prev_num:
            errors.append(f"{decisions_path.name}:{lineno}: decision id {did} out of order after {prev_id}")
        prev_num = dnum
        prev_id = did
    return errors


def lint_readme_links(readme_path: Path) -> list[str]:
    errors: list[str] = []
    if not readme_path.is_file():
        return [f"missing {readme_path}"]
    in_fence = False
    for lineno, line in enumerate(readme_path.read_text(encoding="utf-8").splitlines(), 1):
        if line.strip().startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        for target in MD_LINK_RE.findall(line):
            clean = target.split("#", 1)[0].strip()
            if not clean or "://" in clean or clean.startswith("mailto:"):
                continue
            resolved = (readme_path.parent / unquote(clean)).resolve()
            if not resolved.exists():
                errors.append(f"{readme_path.name}:{lineno}: broken link '{target}' -> {resolved}")
    return errors


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    errors = lint_decisions(repo_root / "docs" / "godot" / "DECISIONS.md")
    errors.extend(lint_readme_links(repo_root / "docs" / "README.md"))
    if errors:
        print("DECISIONS_LINT_FAIL")
        for err in errors:
            print(f"- {err}")
        return 1
    print("DECISIONS_LINT_OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
