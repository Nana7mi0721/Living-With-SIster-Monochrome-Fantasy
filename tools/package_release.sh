#!/usr/bin/env bash
# Build and stage the self-contained Windows x64 release package (`--slim` / `--demo`).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

GODOT_BIN="${GODOT_BIN:-${GODOT:-$(command -v godot 2>/dev/null || command -v Godot_v4.7.2-stable_mono_win64_console.exe 2>/dev/null || true)}}"
DOTNET_BIN="${DOTNET_BIN:-${DOTNET:-$(command -v dotnet 2>/dev/null || true)}}"
PYTHON_BIN="${PYTHON_BIN:-${PYTHON:-$(command -v python3 2>/dev/null || command -v python 2>/dev/null || true)}}"

if [ -z "$GODOT_BIN" ] || [ ! -x "$GODOT_BIN" ]; then
  echo "ERROR: Godot 4.7.2 Mono console executable not found. Set GODOT_BIN=/path/to/Godot_console.exe" >&2
  exit 2
fi
if [ -z "$DOTNET_BIN" ] || [ ! -x "$DOTNET_BIN" ]; then
  echo "ERROR: dotnet executable not found. Set DOTNET_BIN=/path/to/dotnet or add it to PATH" >&2
  exit 2
fi
if [ -z "$PYTHON_BIN" ] || ! command -v "$PYTHON_BIN" >/dev/null 2>&1; then
  echo "ERROR: python not found (set PYTHON_BIN=...)" >&2
  exit 2
fi

export DOTNET="${DOTNET:-$DOTNET_BIN}"
export DOTNET_ROOT="${DOTNET_ROOT:-$(dirname "$DOTNET_BIN")}"
export PATH="$(dirname "$DOTNET_BIN"):$PATH"
export DOTNET_CLI_TELEMETRY_OPTOUT=1
export DOTNET_NOLOGO=1

DEMO=false
SLIM=false
for arg in "$@"; do
  case "$arg" in
    --demo) DEMO=true ;;
    --slim) SLIM=true ;;
    *)
      echo "usage: tools/package_release.sh [--demo] [--slim]" >&2
      exit 2
      ;;
  esac
done

PKG_NAME="Lwsmf-win64"
ZIP_NAME="Lwsmf-win64.zip"
if [ "$SLIM" = true ]; then
  PKG_NAME="Lwsmf-win64-slim"
  ZIP_NAME="Lwsmf-win64-slim.zip"
fi

OUT="$ROOT/dist/$PKG_NAME"
ZIP_OUT="$ROOT/dist/$ZIP_NAME"
STAGE="$ROOT/dist/.$PKG_NAME.staging.$$"
EXPORT_DIR="$ROOT/godot/build/release"
LOG_DIR="$ROOT/work/out"
EXPORT_LOG="$LOG_DIR/package_release_export.log"
BUILD_LOG="$LOG_DIR/package_release_build.log"
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

for required in game_data work/all; do
  if [ ! -d "$ROOT/$required" ]; then
    echo "ERROR: $required/ not found; release package must be self-contained." >&2
    exit 2
  fi
done
if [ ! -f "$ROOT/work/all/media.pak" ]; then
  echo "ERROR: work/all/media.pak not found." >&2
  exit 2
fi

mkdir -p "$LOG_DIR" "$ROOT/dist" "$STAGE/game"

if [ ! -f "$SLN" ]; then
  echo "== creating temporary solution for Godot .NET export =="
  "$DOTNET_BIN" new sln --name Lwsmf --output "$ROOT/godot" >/dev/null
  "$DOTNET_BIN" sln "$SLN" add "$ROOT/godot/Lwsmf.csproj" >/dev/null
  SLN_CREATED=true
fi

echo "== dotnet build (-c Release) =="
"$DOTNET_BIN" build "$ROOT/godot/Lwsmf.csproj" -c Release -v:minimal 2>&1 | tee "$BUILD_LOG"

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

echo "== staging game binaries =="
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
find "$STAGE/game" -type f -name '*.pdb' -delete

to_win_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  else
    printf '%s' "$1"
  fi
}

echo "== staging compact game_data =="
mkdir -p "$STAGE/game_data"
SRC_GD_WIN="$(to_win_path "$ROOT/game_data")"
DST_GD_WIN="$(to_win_path "$STAGE/game_data")"
"$PYTHON_BIN" - "$SRC_GD_WIN" "$DST_GD_WIN" <<'PY'
import json, sys
from pathlib import Path

src_dir = Path(sys.argv[1])
dst_dir = Path(sys.argv[2])
written = 0
for f in sorted(src_dir.glob("*.json")):
    if f.name in ("programs.json", "database.json", "map_tiles.json"):
        data = json.loads(f.read_text(encoding="utf-8"))
        (dst_dir / f.name).write_text(json.dumps(data, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    else:
        (dst_dir / f.name).write_bytes(f.read_bytes())
    written += 1
if written == 0:
    raise SystemExit(f"no json files found in {src_dir}")
PY

echo "== staging minimal runtime assets =="
mkdir -p "$STAGE/work/all/BasicData"
cp -a "$ROOT/work/all/BasicData/"*.project "$STAGE/work/all/BasicData/"
if [ -n "${LWSMF_FONT_PATH:-}" ] && [ -f "$LWSMF_FONT_PATH" ]; then
  cp -a "$LWSMF_FONT_PATH" "$STAGE/work/all/Lolita.ttf"
elif [ -f "$ROOT/work/all/Lolita.ttf" ]; then
  cp -a "$ROOT/work/all/Lolita.ttf" "$STAGE/work/all/Lolita.ttf"
fi

if [ "$SLIM" = true ]; then
  echo "== slimming media.pak =="
  "$PYTHON_BIN" "$ROOT/tools/pack_media.py" --from-pak "$ROOT/work/all/media.pak" --out "$STAGE/work/all/media.pak" --slim
else
  cp -a "$ROOT/work/all/media.pak" "$STAGE/work/all/media.pak"
fi

echo "== staging converter =="
if [ -d "$ROOT/converter" ]; then
  cp -a "$ROOT/converter" "$STAGE/converter"
  find "$STAGE/converter" -type d -name __pycache__ -prune -exec rm -rf {} +
  find "$STAGE/converter" -type f -name '*.pyc' -delete
  rm -rf "$STAGE/converter/tests"
fi

cat > "$STAGE/README.txt" <<'EOF'
LWSMF Remake - Windows x64 发布包
=================================

本发布包自带重制版程序、已转换的 game_data 与加密媒体包 media.pak，解压即玩，无需安装 Python。

目录结构
--------
game\Lwsmf.exe                   重制版主程序（Godot 4.7.2 .NET / Windows x64）
game\Lwsmf.pck                   游戏脚本包
game\data_Lwsmf_windows_x86_64\  .NET 8 运行时与程序集
game_data\                       已转换游戏数据（JSON）
work\all\media.pak               加密媒体归档（WOLFMPK1）
work\all\BasicData\*.project     数据库结构描述
converter\                       可选：本地数据转换器（Python 源码）

快捷键
------
Enter / Z / 鼠标左键   确认 / 对话推进 / 地图交互
Esc / X / 鼠标右键     取消 / 返回
F4                     消息回顾（Backlog）
F5 / F9                快速保存 / 读取（user://quicksave.json，不覆盖槽 1 自动存档）
F8                     切换缩放与过滤（Integer / Fit-Nearest / Fit-Linear）
F11                    无边框全屏切换
F12                    返回标题确认
F2 / F3                游玩统计浮层 / 运行时调试浮层

重新转换（可选）
----------------
1. 安装 Python 3.10+。
2. 准备原版解包后的 Data 目录（直接包含 BasicData\ 与 MapData\）。
3. 在本发布包根目录执行：
   converter\convert.bat --root "<原版Data目录>" --out game_data --reports reports
EOF

echo "== pre-swap release audit =="
"$PYTHON_BIN" "$ROOT/tools/release_audit.py" --dist "$STAGE"

echo "== swapping package into dist =="
if [ -d "$OUT" ]; then
  BACKUP="$LOG_DIR/dist-backup-$(date +%Y%m%d-%H%M%S)-$$"
  mv "$OUT" "$BACKUP"
  echo "existing dist moved to: $BACKUP"
fi
mv "$STAGE" "$OUT"
STAGE=""

echo "== creating zip archive & SHA256SUMS =="
rm -f "$ZIP_OUT"
OUT_WIN="$(to_win_path "$OUT")"
ZIP_WIN="$(to_win_path "$ZIP_OUT")"
"$PYTHON_BIN" - "$OUT_WIN" "$ZIP_WIN" <<'PY'
import hashlib, sys, zipfile
from pathlib import Path

out_dir = Path(sys.argv[1])
zip_path = Path(sys.argv[2])
with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as zf:
    for p in sorted(out_dir.rglob("*")):
        if p.is_file():
            zf.write(p, f"{out_dir.name}/{p.relative_to(out_dir).as_posix()}")

def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()

sha_lines = [f"{sha256_file(zip_path)}  {zip_path.name}"]
exe = out_dir / "game/Lwsmf.exe"
pak = out_dir / "work/all/media.pak"
for item in (exe, pak):
    if item.is_file():
        sha_lines.append(f"{sha256_file(item)}  {out_dir.name}/{item.relative_to(out_dir).as_posix()}")
sha_file = zip_path.parent / "SHA256SUMS.txt"
sha_file.write_text("\n".join(sha_lines) + "\n", encoding="utf-8")

# Size ledger
buckets = {
    "game/Lwsmf.exe": (out_dir / "game/Lwsmf.exe").stat().st_size,
    "game/Lwsmf.pck": (out_dir / "game/Lwsmf.pck").stat().st_size,
    "game/data_Lwsmf_windows_x86_64": sum(p.stat().st_size for p in (out_dir / "game/data_Lwsmf_windows_x86_64").rglob("*") if p.is_file()),
    "game_data": sum(p.stat().st_size for p in (out_dir / "game_data").rglob("*") if p.is_file()),
    "work/all/media.pak": pak.stat().st_size,
    "work/all/other": sum(p.stat().st_size for p in (out_dir / "work/all").rglob("*") if p.is_file() and p != pak),
    "converter+README": sum(p.stat().st_size for p in (out_dir / "converter").rglob("*") if p.is_file()) + (out_dir / "README.txt").stat().st_size,
}
total = sum(buckets.values())
print("=== SIZE LEDGER ===")
for k, v in buckets.items():
    print(f"  {k:34s}: {v/1024/1024:8.2f} MB")
print(f"  {'TOTAL UNCOMPRESSED':34s}: {total/1024/1024:8.2f} MB")
print(f"  {'ZIP ARCHIVE':34s}: {zip_path.stat().st_size/1024/1024:8.2f} MB")
PY

echo "RELEASE_PACKAGE_OK dist=$OUT zip=$ZIP_OUT"
