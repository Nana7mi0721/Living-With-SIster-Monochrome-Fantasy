#!/usr/bin/env bash
# M5-A: build and stage the Windows x64 release package.
# Run from anywhere; paths with spaces are quoted. Does not overwrite user files:
# an existing dist/Lwsmf-win64 is moved to work/out/dist-backup-<timestamp> first.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

GODOT_BIN="${GODOT_BIN:-${GODOT:-/e/Study/tools/godot-4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe}}"
DOTNET_BIN="${DOTNET_BIN:-${DOTNET:-/e/Study/tools/dotnet8/dotnet.exe}}"
PYTHON_BIN="${PYTHON_BIN:-${PYTHON:-python}}"
export DOTNET_ROOT="${DOTNET_ROOT:-E:/Study/tools/dotnet8}"
export DOTNET="${DOTNET:-$DOTNET_BIN}"
export PATH="$(dirname "$DOTNET_BIN"):$PATH"
export DOTNET_CLI_TELEMETRY_OPTOUT=1
export DOTNET_NOLOGO=1

OUT="$ROOT/dist/Lwsmf-win64"
STAGE="$ROOT/dist/.Lwsmf-win64.staging.$$"
EXPORT_DIR="$ROOT/godot/build/release"
LOG_DIR="$ROOT/work/out"
EXPORT_LOG="$LOG_DIR/package_release_export.log"
BUILD_LOG="$LOG_DIR/package_release_build.log"

DEMO=false
if [ "${1:-}" = "--demo" ]; then
  DEMO=true
elif [ "$#" -gt 0 ]; then
  echo "usage: tools/package_release.sh [--demo]" >&2
  exit 2
fi

SLN="$ROOT/godot/Lwsmf.sln"
SLN_CREATED=false

cleanup() {
  if [ -n "${STAGE:-}" ] && [ -d "$STAGE" ]; then
    rm -rf "$STAGE"
  fi
  if [ "$SLN_CREATED" = true ]; then
    rm -f "$SLN"
  fi
}
trap cleanup EXIT

for tool in "$GODOT_BIN" "$DOTNET_BIN"; do
  if [ ! -x "$tool" ]; then
    echo "ERROR: required executable not found: $tool" >&2
    exit 2
  fi
done
if ! command -v "$PYTHON_BIN" >/dev/null 2>&1; then
  echo "ERROR: python not found (set PYTHON_BIN=...)" >&2
  exit 2
fi

for required in game_data work/all; do
  if [ ! -d "$ROOT/$required" ]; then
    echo "ERROR: $required/ not found; release package must be self-contained." >&2
    exit 2
  fi
done

mkdir -p "$LOG_DIR" "$ROOT/dist"
mkdir -p "$STAGE/game"

if [ ! -f "$SLN" ]; then
  echo "== creating temporary solution for Godot .NET export =="
  "$DOTNET_BIN" new sln --name Lwsmf --output "$ROOT/godot" >/dev/null
  "$DOTNET_BIN" sln "$SLN" add "$ROOT/godot/Lwsmf.csproj" >/dev/null
  SLN_CREATED=true
fi

echo "== dotnet build =="
"$DOTNET_BIN" build "$ROOT/godot/Lwsmf.csproj" -v:minimal 2>&1 | tee "$BUILD_LOG"

echo "== Godot release export =="
rm -rf "$EXPORT_DIR"
mkdir -p "$EXPORT_DIR"
"$GODOT_BIN" --headless --path "$ROOT/godot" --export-release "Windows Desktop" 2>&1 | tee "$EXPORT_LOG"

if [ ! -f "$EXPORT_DIR/Lwsmf.exe" ]; then
  echo "ERROR: export did not produce $EXPORT_DIR/Lwsmf.exe" >&2
  exit 1
fi
if grep -qE "no solution file was found|Sdk not found|Export .NET Project: Failed" "$EXPORT_LOG"; then
  echo "ERROR: export log reports a .NET export failure; cannot produce a C# release." >&2
  exit 1
fi

echo "EXPORT_PRESET_OK preset=Windows Desktop export_path=$EXPORT_DIR/Lwsmf.exe"

if [ "$DEMO" = true ]; then
  echo "DEMO: export succeeded; stopping before staging."
  exit 0
fi

echo "== staging game files =="
cp -a "$EXPORT_DIR/Lwsmf.exe" "$STAGE/game/"
if [ -f "$EXPORT_DIR/Lwsmf.pck" ]; then
  cp -a "$EXPORT_DIR/Lwsmf.pck" "$STAGE/game/"
fi
shopt -s nullglob
for extra in "$EXPORT_DIR"/*; do
  base="$(basename "$extra")"
  if [ "$base" = "Lwsmf.exe" ] || [ "$base" = "Lwsmf.pck" ]; then
    continue
  fi
  cp -a "$extra" "$STAGE/game/"
done

echo "== staging game data =="
if [ ! -d "$ROOT/game_data" ]; then
  echo "ERROR: game_data/ not found; run converter/cli.py stage all first." >&2
  exit 1
fi
cp -a "$ROOT/game_data" "$STAGE/game_data"

echo "== staging runtime assets =="
if [ ! -d "$ROOT/work/all" ]; then
  echo "ERROR: work/all/ not found; the release package requires runtime assets." >&2
  exit 1
fi
mkdir -p "$STAGE/work"
cp -a "$ROOT/work/all" "$STAGE/work/all"
rm -rf "$STAGE/work/all/save"

echo "== staging converter =="
if [ -d "$ROOT/converter" ]; then
  cp -a "$ROOT/converter" "$STAGE/converter"
  find "$STAGE/converter" -type d -name __pycache__ -prune -exec rm -rf {} +
  find "$STAGE/converter" -type f -name '*.pyc' -delete
  rm -rf "$STAGE/converter/tests"
else
  echo "WARNING: converter/ not found; package will ship without converter." >&2
fi

cat > "$STAGE/README.txt" <<'EOF'
LWSMF Remake - Windows x64 发布包
=================================

本发布包自带重制版程序、已转换的 game_data 与运行所需素材，正常游玩无需安装 Python，也无需先转换。
如需重新转换或排查数据，可使用本包内的本地转换器。

目录结构
--------
game\Lwsmf.exe      重制版主程序（Godot 4.7.2 .NET / Windows x64）
game\Lwsmf.pck      游戏数据包
game\data_Lwsmf_windows_x86_64\  .NET 运行时与程序集（必须与 Lwsmf.exe 同目录）
game_data\          已转换的游戏数据（manifest/programs/database/map_tiles）
work\all\          运行所需素材（BasicData/MapChip/MapData/SystemFile；不含 save）
converter\          可选：本地数据转换器（Python 源码）
reports\            可选：转换报告

首次运行
--------
直接运行：

   game\Lwsmf.exe

如果 game_data 缺失或不完整，程序会打开首次运行向导；也可以手动执行下面的重新转换步骤。

重新转换（可选）
----------------
1. 安装 Python 3.10+，确保命令行可运行 python。
2. 准备原版 Data 目录（该目录应直接包含 BasicData\ 与 MapData\，不是游戏安装根目录）。
3. 在本发布包根目录（与 game、converter 同级）打开命令提示符，执行：

   converter\convert.bat --root "D:\你的原版\Data" --out game_data --reports reports

   也可以先 cd 到本目录后运行 converter\convert.bat，按提示填写路径。
4. 看到 CONVERT_ALL_OK 后，重新运行 game\Lwsmf.exe。

转换器只读取你本机指定的原版目录，在本地写入 game_data\ 与 reports\，不会上传或分发原版文件。
如果提示缺少 BasicData\Game.dat 等文件，说明 --root 指向的不是解包后的 Data 目录。
EOF

echo "== swapping package into dist =="
if [ -d "$OUT" ]; then
  BACKUP="$LOG_DIR/dist-backup-$(date +%Y%m%d-%H%M%S)-$$"
  mv "$OUT" "$BACKUP"
  echo "existing dist moved to: $BACKUP"
fi
mv "$STAGE" "$OUT"
STAGE=""

echo "== release audit =="
"$PYTHON_BIN" "$ROOT/tools/release_audit.py" --dist "$OUT"

echo "RELEASE_PACKAGE_OK dist=$OUT"
