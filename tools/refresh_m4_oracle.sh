#!/usr/bin/env bash
# 重新生成 oracle/fixtures/m4_full_route_rust_trace.json.gz。
# 仅当新 Rust 轨迹与当前 C# 轨迹逐步 100% 一致时才覆盖夹具：夹具不能被用来“迁就”C# 的偏差。
# 用法： tools/refresh_m4_oracle.sh        需要本机 cargo 与 Godot（见 tools/local.env.sh）
set -euo pipefail

IMO="$(cd "$(dirname "$0")/.." && pwd)"
if [ -f "$IMO/tools/local.env.sh" ]; then
  # shellcheck disable=SC1091
  . "$IMO/tools/local.env.sh"
fi
GODOT="${GODOT:-${GODOT_BIN:-$(command -v godot 2>/dev/null || true)}}"
if [ -z "$GODOT" ] || [ ! -x "$GODOT" ]; then
  echo "godot not found: set GODOT or tools/local.env.sh" >&2
  exit 2
fi
export UseSharedCompilation=false MSBUILDDISABLENODEREUSE=1 LWsmf_QUIET_HUD=1

FIXTURE="$IMO/oracle/fixtures/m4_full_route_rust_trace.json.gz"
RUST_TRACE="$IMO/reports/rust_m4_route_trace.json"
CS_TRACE="$IMO/reports/csharp_m4_route_trace.json"
SAVE_DIR="$(mktemp -d)"
trap 'rm -rf "$SAVE_DIR"' EXIT
mkdir -p "$IMO/reports"

echo "[1/4] build C# solutions"
"$GODOT" --headless --path "$IMO/godot" --build-solutions --quit

echo "[2/4] Rust route-full trace"
rm -f "$RUST_TRACE"
(cd "$IMO" && cargo run --release -q -p wolf-cli --bin wolf-cli -- route-full)
test -s "$RUST_TRACE"

echo "[3/4] C# M4 route trace"
rm -f "$CS_TRACE"
out="$(LWsmf_SAVE_DIR="$SAVE_DIR" "$GODOT" --headless --path "$IMO/godot" res://tests/M4FullRouteTraceTest.tscn 2>&1)"
printf '%s\n' "$out" > "$IMO/reports/m4_full_route_test.log"
printf '%s\n' "$out" | grep -q M4_FULL_ROUTE_TRACE_TEST_OK
test -s "$CS_TRACE"

echo "[4/4] compare, then refresh fixture only on a 100% match"
python "$IMO/oracle/diff_route.py" "$RUST_TRACE" "$CS_TRACE" --report "$IMO/reports/m4_route_refresh_diff.json"
python - "$RUST_TRACE" "$FIXTURE" <<'PY'
import gzip, shutil, sys
src, dst = sys.argv[1], sys.argv[2]
with open(src, "rb") as fin, gzip.open(dst, "wb", compresslevel=9) as fout:
    shutil.copyfileobj(fin, fout)
print("fixture refreshed:", dst)
PY
echo "M4_ORACLE_REFRESH_OK"
