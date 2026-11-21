#!/usr/bin/env bash
# Inject LWSMF game_data and media.pak into an unsigned iOS IPA (Windows/macOS/Linux).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PYTHON_BIN="${PYTHON_BIN:-${PYTHON:-$(command -v python3 2>/dev/null || command -v python 2>/dev/null || true)}}"
if [ -z "$PYTHON_BIN" ]; then
  echo "ERROR: python not found (set PYTHON_BIN=...)" >&2
  exit 2
fi

exec "$PYTHON_BIN" "$ROOT/tools/mobile/ios_inject_data.py" "$@"
