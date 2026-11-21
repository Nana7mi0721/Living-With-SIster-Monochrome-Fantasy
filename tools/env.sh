#!/usr/bin/env bash
# Optional developer helper for tools/ and Rust toolchain setup.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export LWSMF_ROOT="$ROOT"
T="$ROOT/tools"
export PATH="$T/bin:$T/cmake/bin:$T/ffmpeg/bin:${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
echo "[env] rustc $(rustc --version 2>/dev/null | cut -d' ' -f2) | rg $(rg --version 2>/dev/null | head -1 | cut -d' ' -f2)"
