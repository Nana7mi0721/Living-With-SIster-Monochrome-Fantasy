#!/usr/bin/env python3
"""Deprecated compatibility wrapper; use tools/release_audit.py instead.

The old no-assets copyright audit was removed when D-53 made the release package
self-contained. This wrapper preserves the old command path for existing scripts.
"""
try:
    from .release_audit import main
except ImportError:  # direct script execution: python tools/copyright_audit.py
    from release_audit import main

if __name__ == "__main__":
    raise SystemExit(main())
