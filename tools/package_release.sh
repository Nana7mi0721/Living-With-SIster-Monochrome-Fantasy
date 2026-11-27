#!/usr/bin/env bash
# Build and stage the self-contained Windows x64 release package (`--slim` / `--lossless` / `--demo`).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

to_posix_path() {
  local input="$1"
  if [ -z "$input" ]; then
    return 0
  fi
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -u "$input"
  elif [[ "$input" =~ ^([A-Za-z]):[/\\](.*)$ ]]; then
    local drive="${BASH_REMATCH[1],,}"
    local rest="${BASH_REMATCH[2]//\\//}"
    printf '/%s/%s' "$drive" "$rest"
  else
    printf '%s' "${input//\\//}"
  fi
}

to_win_path() {
  local input="$1"
  if [ -z "$input" ]; then
    return 0
  fi
  if [[ "$input" != /* && ! "$input" =~ ^[A-Za-z]:[/\\] ]]; then
    input="$ROOT/$input"
  fi
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -m "$input"
  elif [[ "$input" =~ ^/([a-zA-Z])/(.*)$ ]]; then
    printf '%s:/%s' "${BASH_REMATCH[1]^^}" "${BASH_REMATCH[2]}"
  else
    printf '%s' "$input"
  fi
}

if [ -f "$ROOT/tools/local.env.sh" ]; then
  _LOCAL_GODOT=""
  _LOCAL_DOTNET=""
  eval "$(
    # shellcheck disable=SC1090
    source "$ROOT/tools/local.env.sh" >/dev/null 2>&1 || true
    printf '_LOCAL_GODOT=%q\n_LOCAL_DOTNET=%q\n' "${GODOT:-}" "${DOTNET:-}"
  )"
  [ -z "${GODOT_BIN:-}" ] && [ -z "${GODOT:-}" ] && [ -n "$_LOCAL_GODOT" ] && GODOT_BIN="$_LOCAL_GODOT"
  [ -z "${DOTNET_BIN:-}" ] && [ -z "${DOTNET:-}" ] && [ -n "$_LOCAL_DOTNET" ] && DOTNET_BIN="$_LOCAL_DOTNET"
fi

GODOT_BIN="$(to_posix_path "${GODOT_BIN:-${GODOT:-$(command -v godot 2>/dev/null || command -v Godot_v4.7.2-stable_mono_win64_console.exe 2>/dev/null || true)}}")"
DOTNET_BIN="$(to_posix_path "${DOTNET_BIN:-${DOTNET:-$(command -v dotnet 2>/dev/null || true)}}")"
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

export DOTNET="$(to_win_path "$DOTNET_BIN")"
export DOTNET_ROOT="${DOTNET_ROOT:-$(to_win_path "$(dirname "$DOTNET_BIN")")}"
export PATH="$(dirname "$DOTNET_BIN"):$PATH"
export DOTNET_CLI_TELEMETRY_OPTOUT=1
export DOTNET_NOLOGO=1
export UseSharedCompilation=false
export MSBUILDDISABLENODEREUSE=1

VERSION="0.1.0"
WINDOWS_APP_NAME="妹相随～黑白世界的缤纷冒险～"
DEMO=false
SLIM=false
LOSSLESS=false
HD=false
BROTLI=false
INSTALLER=false
ALLOW_PLACEHOLDER=false
TEMPLATE_OPT=""
OFFICIAL_TEMPLATE=false
MEDIA_PAK_OPT=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --demo) DEMO=true; ALLOW_PLACEHOLDER=true; shift ;;
    --slim) SLIM=true; shift ;;
    --lossless) LOSSLESS=true; shift ;;
    --hd) HD=true; shift ;;
    --brotli) BROTLI=true; shift ;;
    --installer) INSTALLER=true; shift ;;
    --allow-placeholder) ALLOW_PLACEHOLDER=true; shift ;;
    --official-template) OFFICIAL_TEMPLATE=true; shift ;;
    --version)
      if [ "$#" -lt 2 ]; then
        echo "ERROR: --version requires a value" >&2
        exit 2
      fi
      VERSION="$2"
      shift 2
      ;;
    --template)
      if [ "$#" -lt 2 ]; then
        echo "ERROR: --template requires a file path" >&2
        exit 2
      fi
      TEMPLATE_OPT="$2"
      shift 2
      ;;
    --media-pak)
      if [ "$#" -lt 2 ]; then
        echo "ERROR: --media-pak requires a file path" >&2
        exit 2
      fi
      MEDIA_PAK_OPT="$2"
      shift 2
      ;;
    *)
      echo "usage: tools/package_release.sh [--demo] [--slim|--lossless] [--media-pak FILE] [--hd] [--brotli] [--installer] [--allow-placeholder] [--version VER] [--template FILE|--official-template]" >&2
      exit 2
      ;;
  esac
done

if [ "$SLIM" = true ] && [ "$LOSSLESS" = true ]; then
  echo "ERROR: --slim and --lossless are mutually exclusive" >&2
  exit 2
fi
if [ -n "$TEMPLATE_OPT" ] && [ "$OFFICIAL_TEMPLATE" = true ]; then
  echo "ERROR: --template and --official-template are mutually exclusive" >&2
  exit 2
fi
if [ -z "$TEMPLATE_OPT" ] && [ "$OFFICIAL_TEMPLATE" != true ] && [ "${LWSMF_OFFICIAL_TEMPLATE:-0}" != "1" ]; then
  for _tpl in \
    "$ROOT/work/out/templates/windows/windows_release_x86_64.exe" \
    "$ROOT/dist/templates/windows/windows_release_x86_64.exe"
  do
    if [ -s "$_tpl" ]; then
      TEMPLATE_OPT="$_tpl"
      echo "== auto-detected custom Windows template: $TEMPLATE_OPT =="
      break
    fi
  done
fi
if [ -n "$MEDIA_PAK_OPT" ] && { [ "$SLIM" = true ] || [ "$LOSSLESS" = true ]; }; then
  echo "ERROR: --media-pak cannot be combined with --slim or --lossless" >&2
  exit 2
fi
if [ -n "$MEDIA_PAK_OPT" ] && [ ! -f "$MEDIA_PAK_OPT" ]; then
  echo "ERROR: media.pak not found: $MEDIA_PAK_OPT" >&2
  exit 2
fi

PKG_NAME="LWSMF-${VERSION}-windows-x64"
if [ "$SLIM" = true ]; then
  PKG_NAME="${PKG_NAME}-slim"
fi
if [ "$HD" = true ]; then
  PKG_NAME="$PKG_NAME-hd"
fi
ZIP_NAME="${PKG_NAME}.zip"

OUT="$ROOT/dist/$PKG_NAME"
ZIP_OUT="$ROOT/dist/$ZIP_NAME"
STAGE="$ROOT/dist/.$PKG_NAME.staging.$$"
EXPORT_DIR="$ROOT/godot/build/release"
LOG_DIR="$ROOT/work/out"
EXPORT_LOG="$LOG_DIR/package_release_export.log"
BUILD_LOG="$LOG_DIR/package_release_build.log"
SLN="$ROOT/godot/Lwsmf.sln"
SLN_CREATED=false
PRESET_FILE="$ROOT/godot/export_presets.cfg"
PRESET_BAK="$LOG_DIR/export_presets.win.$$.cfg"
PROJECT_FILE="$ROOT/godot/project.godot"
PROJECT_BAK="$LOG_DIR/project.win.$$.godot"
ICON_OUT="$LOG_DIR/release_icons.win.$$"
ICON_STAGE="$ROOT/godot/assets/boot/generated"
ICON_STAGE_CREATED=false
MEDIA_OVERLAY="$LOG_DIR/media_overlay.win.$$"

cleanup() {
  if [ -n "${STAGE:-}" ] && [ -d "$STAGE" ]; then
    rm -rf "$STAGE"
  fi
  if [ -f "$PRESET_BAK" ]; then
    cp -f "$PRESET_BAK" "$PRESET_FILE"
    rm -f "$PRESET_BAK"
  fi
  if [ -f "$PROJECT_BAK" ]; then
    cp -f "$PROJECT_BAK" "$PROJECT_FILE"
    rm -f "$PROJECT_BAK"
  fi
  if [ "$ICON_STAGE_CREATED" = true ]; then
    rm -rf "$ICON_STAGE"
  fi
  rm -rf "$ICON_OUT" "$MEDIA_OVERLAY"
  if [ "$SLN_CREATED" = true ]; then
    rm -f "$SLN" "$ROOT/godot/Lwsmf.slnx"
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
  "$DOTNET_BIN" new sln --format sln --name Lwsmf --output "$ROOT/godot" >/dev/null 2>&1 \
    || "$DOTNET_BIN" new sln --name Lwsmf --output "$ROOT/godot" >/dev/null
  SLN_CREATED=true
  "$DOTNET_BIN" sln "$SLN" add "$ROOT/godot/Lwsmf.csproj" >/dev/null
fi

normalize_integrity() {
  local target="$1"
  [ -e "$target" ] || return 0
  command -v icacls.exe >/dev/null 2>&1 || return 0
  local win_target
  win_target="$(to_win_path "$target")"
  for _ in 1 2 3; do
    if MSYS2_ARG_CONV_EXCL='*' icacls.exe "$win_target" /setintegritylevel M /T >/dev/null 2>&1; then
      echo "INTEGRITY_OK $target"
      return 0
    fi
    sleep 1
  done
  local user="${USERNAME:-${USER:-}}"
  if [ -n "$user" ]; then
    MSYS2_ARG_CONV_EXCL='*' icacls.exe "$win_target" /grant "${user}:(OI)(CI)F" >/dev/null 2>&1 || true
  fi
  for _ in 1 2 3; do
    if MSYS2_ARG_CONV_EXCL='*' icacls.exe "$win_target" /setintegritylevel M /T >/dev/null 2>&1; then
      echo "INTEGRITY_OK $target"
      return 0
    fi
    sleep 1
  done
  echo "WARN: unable to normalize NTFS integrity level for $target" >&2
}

echo "== dotnet build (-c Release) =="
"$DOTNET_BIN" build "$ROOT/godot/Lwsmf.csproj" -c Release -v:minimal 2>&1 | tee "$BUILD_LOG"

WIN_VERSION="$("$PYTHON_BIN" - "$VERSION" <<'PY'
import re
import sys

match = re.match(r"^(\d+)\.(\d+)\.(\d+)(?:\.(\d+))?", sys.argv[1])
print(".".join(match.groups(default="0")) if match else "0.0.0.0")
PY
)"

echo "== preparing Windows icons and installer artwork =="
[ ! -d "$ICON_STAGE" ] && ICON_STAGE_CREATED=true
ICON_ARGS=(
  --source-dir "$(to_win_path "${LWSMF_PUBLISH_DIR:-$ROOT/work/all/_publish}")"
  --out-dir "$(to_win_path "$ICON_OUT")"
  --stage-res "$(to_win_path "$ICON_STAGE")"
)
if [ "$ALLOW_PLACEHOLDER" = true ] || [ "${LWSMF_ALLOW_PLACEHOLDER_ICONS:-0}" = "1" ]; then
  ICON_ARGS+=(--allow-placeholder)
fi
"$PYTHON_BIN" "$(to_win_path "$ROOT/tools/sync_release_icons.py")" "${ICON_ARGS[@]}"

cp -f "$PROJECT_FILE" "$PROJECT_BAK"
"$PYTHON_BIN" - "$(to_win_path "$PROJECT_FILE")" <<'PY'
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
text = re.sub(r"(?m)^config/icon=.*\r?\n", "", text)
line = 'config/icon="res://assets/boot/generated/icon_pc_taskbar.png"\n'
if "[application]\n" not in text:
    raise SystemExit("project.godot is missing [application]")
text = text.replace("[application]\n", "[application]\n\n" + line, 1)
path.write_text(text, encoding="utf-8")
PY

cp -f "$PRESET_FILE" "$PRESET_BAK"
PATCH_SETS=(
  --set "application/file_version=$WIN_VERSION"
  --set "application/product_version=$WIN_VERSION"
  --set "application/company_name=Inusuku"
  --set "application/product_name=$WINDOWS_APP_NAME"
  --set "application/file_description=$WINDOWS_APP_NAME"
  --set "application/copyright=Copyright (C) 2026 Inusuku. All rights reserved."
)
if [ -n "$TEMPLATE_OPT" ]; then
  TEMPLATE_WIN="$(to_win_path "$TEMPLATE_OPT")"
  if [ ! -f "$TEMPLATE_WIN" ] && [ ! -f "$TEMPLATE_OPT" ]; then
    echo "ERROR: custom release template not found: $TEMPLATE_OPT" >&2
    exit 2
  fi
  PATCH_SETS+=(--set "custom_template/release=$TEMPLATE_WIN")
fi
"$PYTHON_BIN" "$(to_win_path "$ROOT/tools/mobile/export_preset_template.py")" \
  --preset-file "$(to_win_path "$PRESET_FILE")" \
  --preset "Windows Desktop" \
  "${PATCH_SETS[@]}"

echo "== Godot release export =="
if command -v timeout >/dev/null 2>&1; then
  timeout 60 "$GODOT_BIN" --headless --path "$ROOT/godot" --import --quit \
    >"$LOG_DIR/package_release_import.log" 2>&1 \
    || echo "WARN: Godot import prewarm did not exit cleanly; continuing (log: $LOG_DIR/package_release_import.log)" >&2
else
  "$GODOT_BIN" --headless --path "$ROOT/godot" --import --quit \
    >"$LOG_DIR/package_release_import.log" 2>&1 || true
fi
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

echo "== localizing Lwsmf.exe Windows version resource (0x0804) =="
"$PYTHON_BIN" "$(to_win_path "$ROOT/tools/mobile/windows_pe.py")" "$(to_win_path "$STAGE/game/Lwsmf.exe")"

echo "== staging compact game_data (brotli=$BROTLI) =="
mkdir -p "$STAGE/game_data"
SRC_GD_WIN="$(to_win_path "$ROOT/game_data")"
DST_GD_WIN="$(to_win_path "$STAGE/game_data")"
GD_ARGS=(--src "$SRC_GD_WIN" --dst "$DST_GD_WIN")
[ "$BROTLI" = true ] && GD_ARGS+=(--brotli)
"$PYTHON_BIN" "$(to_win_path "$ROOT/tools/brotli_json.py")" "${GD_ARGS[@]}"

echo "== staging minimal runtime assets =="
mkdir -p "$STAGE/work/all/BasicData"
cp -a "$ROOT/work/all/BasicData/"*.project "$STAGE/work/all/BasicData/"
FOUND_FONT=""
if [ -n "${LWSMF_FONT_PATH:-}" ] && [ -f "$LWSMF_FONT_PATH" ]; then
  FOUND_FONT="$LWSMF_FONT_PATH"
else
  for dir in "$ROOT/work/all" "$ROOT/work/all/Data" "$ROOT/work/out/retail_pairing/game"; do
    for name in "Lolita.ttf" "LOLITA.TTF" "lolita.ttf"; do
      if [ -f "$dir/$name" ]; then
        FOUND_FONT="$dir/$name"
        break 2
      fi
    done
  done
fi
if [ -n "$FOUND_FONT" ]; then
  cp -a "$FOUND_FONT" "$STAGE/work/all/Lolita.ttf"
else
  PAK_WIN="$(to_win_path "$ROOT/work/all/media.pak")"
  TOOLS_WIN="$(to_win_path "$ROOT/tools")"
  HAS_PAK_FONT="$("$PYTHON_BIN" - "$PAK_WIN" "$TOOLS_WIN" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[2])
from pack_media import read_pak_index

p = Path(sys.argv[1])
if not p.is_file():
    print("0")
    raise SystemExit(0)
names = read_pak_index(p)
print("1" if any(n.endswith("lolita.ttf") for n in names) else "0")
PY
)"
  if [ "$HAS_PAK_FONT" != "1" ] && [ "${LWSMF_ALLOW_MISSING_FONT:-0}" != "1" ]; then
    echo "ERROR: Lolita.ttf not found in work/all, Data, retail_pairing, or media.pak. Set LWSMF_FONT_PATH=/path/to/Lolita.ttf (or LWSMF_ALLOW_MISSING_FONT=1)." >&2
    exit 3
  fi
fi

rm -rf "$MEDIA_OVERLAY"
mkdir -p "$MEDIA_OVERLAY/SystemFile"
if [ -d "$ROOT/work/all/SystemFile" ]; then
  cp -a "$ROOT/work/all/SystemFile/." "$MEDIA_OVERLAY/SystemFile/"
fi
MEDIA_EXTRA_ARGS=(--extra-root "$(to_win_path "$MEDIA_OVERLAY")")

if [ -n "$MEDIA_PAK_OPT" ]; then
  echo "== copying prebuilt media.pak ($MEDIA_PAK_OPT) =="
  cp -a "$MEDIA_PAK_OPT" "$STAGE/work/all/media.pak"
elif [ "$LOSSLESS" = true ]; then
  echo "== building true-lossless media.pak =="
  "$PYTHON_BIN" "$(to_win_path "$ROOT/tools/pack_media.py")" \
    --from-pak "$(to_win_path "$ROOT/work/all/media.pak")" \
    --out "$(to_win_path "$STAGE/work/all/media.pak")" \
    --lossless \
    "${MEDIA_EXTRA_ARGS[@]}"
elif [ "$SLIM" = true ]; then
  echo "== slimming media.pak =="
  "$PYTHON_BIN" "$(to_win_path "$ROOT/tools/pack_media.py")" \
    --from-pak "$(to_win_path "$ROOT/work/all/media.pak")" \
    --out "$(to_win_path "$STAGE/work/all/media.pak")" \
    --slim \
    "${MEDIA_EXTRA_ARGS[@]}"
else
  "$PYTHON_BIN" "$(to_win_path "$ROOT/tools/pack_media.py")" \
    --from-pak "$(to_win_path "$ROOT/work/all/media.pak")" \
    --out "$(to_win_path "$STAGE/work/all/media.pak")" \
    "${MEDIA_EXTRA_ARGS[@]}"
fi

if [ "$HD" = true ]; then
  echo "== packing 2x media_hd.pak =="
  if [ ! -f "$ROOT/work/hd/hd_manifest.json" ]; then
    echo "ERROR: --hd needs work/hd/hd_manifest.json (run tools/upscale_assets.py first)." >&2
    exit 3
  fi
  "$PYTHON_BIN" "$(to_win_path "$ROOT/tools/pack_media.py")" \
    --hd-root "$(to_win_path "$ROOT/work/hd")" \
    --out "$(to_win_path "$STAGE/work/all/media_hd.pak")"
fi

echo "== staging converter =="
if [ -d "$ROOT/converter" ]; then
  cp -a "$ROOT/converter" "$STAGE/converter"
  find "$STAGE/converter" -type d -name __pycache__ -prune -exec rm -rf {} +
  find "$STAGE/converter" -type f -name '*.pyc' -delete
  rm -rf "$STAGE/converter/tests"
fi
if [ -f "$ROOT/docs/BASELINE.json" ]; then
  mkdir -p "$STAGE/docs"
  cp -a "$ROOT/docs/BASELINE.json" "$STAGE/docs/BASELINE.json"
fi

cat > "$STAGE/README.txt" <<'EOF'
妹相随～黑白世界的缤纷冒险～ - Windows x64 发布包
===================================================

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
按键说明.txt                     完整快捷键与窗口/帧率行为说明

快捷键
------
Enter / Z / 鼠标左键   确认 / 对话推进 / 地图交互
Esc / X / 鼠标右键     取消 / 返回
F4                     窗口尺寸循环（1024×576 → 2x → … → 全屏 → 最小）
Shift+F4               切换缩放与过滤（Integer / Fit-Nearest / Fit-Linear）
F5                     帧率循环（60 / 30 / 15 fps）
Shift+F5               开关标题栏实时帧率信息
F6 / F7                快速保存 / 读取（user://quicksave.json，不覆盖槽 1 自动存档）
F8                     存档导入 / 导出
F9                     消息回顾（Backlog）
F10                    导出诊断快照
F11                    全屏 ↔ 最小窗口（1024×576）
F12                    返回标题确认
F2 / F3                游玩统计浮层 / 运行时调试浮层
方向键 / W A S D        移动与选择光标（回顾界面 W/S 滚动）

重新转换（可选）
----------------
1. 安装 Python 3.10+。
2. 准备原版解包后的 Data 目录（直接包含 BasicData\ 与 MapData\）。
3. 在本发布包根目录执行：
   converter\convert.bat --root "<原版Data目录>" --out game_data --reports reports
EOF

cp "$ROOT/docs/godot/HOTKEYS.md" "$STAGE/按键说明.txt"

echo "== pre-swap release audit =="
"$PYTHON_BIN" "$(to_win_path "$ROOT/tools/release_audit.py")" --dist "$(to_win_path "$STAGE")"

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
pak_hd = out_dir / "work/all/media_hd.pak"
for item in (exe, pak, pak_hd):
    if item.is_file():
        sha_lines.append(f"{sha256_file(item)}  {out_dir.name}/{item.relative_to(out_dir).as_posix()}")
sha_file = zip_path.parent / "SHA256SUMS.txt"
sha_file.write_text("\n".join(sha_lines) + "\n", encoding="utf-8")

# Size ledger
pck = out_dir / "game/Lwsmf.pck"
buckets = {
    "game/Lwsmf.exe": (out_dir / "game/Lwsmf.exe").stat().st_size,
    "game/Lwsmf.pck": pck.stat().st_size if pck.is_file() else 0,
    "game/data_Lwsmf_windows_x86_64": sum(p.stat().st_size for p in (out_dir / "game/data_Lwsmf_windows_x86_64").rglob("*") if p.is_file()),
    "game_data": sum(p.stat().st_size for p in (out_dir / "game_data").rglob("*") if p.is_file()),
    "work/all/media.pak": pak.stat().st_size,
    "work/all/other": sum(p.stat().st_size for p in (out_dir / "work/all").rglob("*") if p.is_file() and p != pak),
    "converter+README+按键说明": sum(p.stat().st_size for p in (out_dir / "converter").rglob("*") if p.is_file()) + (out_dir / "README.txt").stat().st_size + (out_dir / "按键说明.txt").stat().st_size,
}
total = sum(buckets.values())
print("=== SIZE LEDGER ===")
for k, v in buckets.items():
    print(f"  {k:34s}: {v/1024/1024:8.2f} MB")
print(f"  {'TOTAL UNCOMPRESSED':34s}: {total/1024/1024:8.2f} MB")
print(f"  {'ZIP ARCHIVE':34s}: {zip_path.stat().st_size/1024/1024:8.2f} MB")
PY

if [ "$INSTALLER" = true ]; then
  echo "== building Windows Inno Setup LZMA2 installer =="
  ISCC_BIN="$(to_posix_path "${ISCC_BIN:-}")"
  if [ -z "$ISCC_BIN" ]; then
    for candidate in "$ROOT"/work/out/inno/tool-*/ISCC.exe; do
      if [ -x "$candidate" ]; then
        ISCC_BIN="$candidate"
        break
      fi
    done
  fi
  if [ -z "$ISCC_BIN" ]; then
    ISCC_BIN="$(command -v ISCC.exe 2>/dev/null || command -v iscc 2>/dev/null || true)"
  fi
  if [ -z "$ISCC_BIN" ]; then
    for candidate in \
      "${LOCALAPPDATA:-}/Programs/Inno Setup 6/ISCC.exe" \
      "${LOCALAPPDATA:-}/InnoSetup6/ISCC.exe" \
      "${LOCALAPPDATA:-}/Inno Setup 6/ISCC.exe" \
      "/c/Program Files (x86)/Inno Setup 6/ISCC.exe" \
      "/c/Program Files/Inno Setup 6/ISCC.exe" \
      "/c/Program Files (x86)/Inno Setup 7/ISCC.exe" \
      "/c/Program Files/Inno Setup 7/ISCC.exe"
    do
      if [ -n "$candidate" ] && [ -x "$candidate" ]; then
        ISCC_BIN="$candidate"
        break
      fi
    done
  fi
  if [ -z "$ISCC_BIN" ] || [ ! -x "$ISCC_BIN" ]; then
    echo "ERROR: ISCC.exe not found; install Inno Setup 6.4+ or set ISCC_BIN=/path/to/ISCC.exe" >&2
    exit 2
  fi
  ISCC_DIR="$(dirname "$ISCC_BIN")"
  EXTRA_LANGUAGES=""
  CHINESE_ISL=""
  for candidate in \
    "$(to_posix_path "${LWSMF_INNO_CHINESE_ISL:-}")" \
    "$ISCC_DIR/Languages/ChineseSimplified.isl" \
    "$ISCC_DIR/ChineseSimplified.isl" \
    "$ROOT/work/out/inno/ChineseSimplified.isl"
  do
    if [ -n "$candidate" ] && [ -f "$candidate" ]; then
      CHINESE_ISL="$candidate"
      break
    fi
  done
  if [ -z "$CHINESE_ISL" ]; then
    echo "ERROR: ChineseSimplified.isl not found; place it in work/out/inno/ChineseSimplified.isl or set LWSMF_INNO_CHINESE_ISL." >&2
    exit 2
  fi
  CHINESE_ISL_WIN="$(to_win_path "$CHINESE_ISL")"
  EXTRA_LANGUAGES="Name: \"chinesesimplified\"; MessagesFile: \"$CHINESE_ISL_WIN\""
  ISS_PATH="$LOG_DIR/lwsmf-${PKG_NAME}.iss"
  TMPL_WIN="$(to_win_path "$ROOT/tools/installer/lwsmf.iss.tmpl")"
  ISS_WIN="$(to_win_path "$ISS_PATH")"
  DIST_WIN="$(to_win_path "$ROOT/dist")"
  STAGE_WIN="$(to_win_path "$OUT")"
  SETUP_BASE="${PKG_NAME}-setup"
  SETUP_ICON_WIN="$(to_win_path "$ICON_OUT/installer/setup_icon.ico")"
  SETUP_WELCOME_WIN="$(to_win_path "$ICON_OUT/installer/setup_welcome.png")"
  SETUP_FINISH_WIN="$(to_win_path "$ICON_OUT/installer/setup_finish.png")"
  SETUP_SMALL_WIN="$(to_win_path "$ICON_OUT/installer/setup_small.png")"
  for asset in \
    "$ICON_OUT/installer/setup_icon.ico" \
    "$ICON_OUT/installer/setup_welcome.png" \
    "$ICON_OUT/installer/setup_finish.png" \
    "$ICON_OUT/installer/setup_small.png"
  do
    if [ ! -f "$asset" ]; then
      echo "ERROR: installer artwork missing: $asset" >&2
      exit 3
    fi
  done
  "$PYTHON_BIN" - "$TMPL_WIN" "$ISS_WIN" "$WINDOWS_APP_NAME" "$VERSION" "$STAGE_WIN" "$DIST_WIN" "$SETUP_BASE" "$EXTRA_LANGUAGES" "$SETUP_ICON_WIN" "$SETUP_WELCOME_WIN" "$SETUP_FINISH_WIN" "$SETUP_SMALL_WIN" <<'PY'
import sys
from pathlib import Path
from PIL import Image

tmpl = Path(sys.argv[1]).read_text(encoding="utf-8")
extra_langs = sys.argv[8]
with Image.open(sys.argv[10]) as welcome_image:
    welcome_width, welcome_height = welcome_image.size
with Image.open(sys.argv[11]) as finish_image:
    finish_width, finish_height = finish_image.size
rendered = (
    tmpl.replace("@APP_NAME@", sys.argv[3])
    .replace("@APP_VERSION@", sys.argv[4])
    .replace("@STAGE_DIR@", sys.argv[5].replace("/", "\\"))
    .replace("@OUTPUT_DIR@", sys.argv[6].replace("/", "\\"))
    .replace("@OUTPUT_BASE_FILENAME@", sys.argv[7])
    .replace("@SETUP_ICON@", sys.argv[9].replace("/", "\\"))
    .replace("@SETUP_WELCOME@", sys.argv[10].replace("/", "\\"))
    .replace("@SETUP_FINISH@", sys.argv[11].replace("/", "\\"))
    .replace("@SETUP_SMALL@", sys.argv[12].replace("/", "\\"))
    .replace("@WELCOME_POSTER_WIDTH@", str(welcome_width))
    .replace("@WELCOME_POSTER_HEIGHT@", str(welcome_height))
    .replace("@FINISH_POSTER_WIDTH@", str(finish_width))
    .replace("@FINISH_POSTER_HEIGHT@", str(finish_height))
    .replace("\n@EXTRA_LANGUAGES@\n", f"\n{extra_langs}\n" if extra_langs else "\n")
    .replace("@EXTRA_LANGUAGES@", extra_langs)
)
Path(sys.argv[2]).write_text(rendered, encoding="utf-8")
PY
  ISCC_WIN="$(to_win_path "$ISCC_BIN")"
  WORK_OUT_WIN="$(to_win_path "$ROOT/work/out")"
  if [[ "$ISCC_WIN" == "$WORK_OUT_WIN"* ]]; then
    normalize_integrity "$(dirname "$ISCC_BIN")"
  fi
  "$ISCC_BIN" "$ISS_WIN"
  echo "INSTALLER_OK exe=$ROOT/dist/${SETUP_BASE}.exe"
  if [ -f "$ROOT/dist/${SETUP_BASE}.exe" ]; then
    SETUP_SHA="$("$PYTHON_BIN" - "$ROOT/dist/${SETUP_BASE}.exe" <<'PY'
import hashlib
import sys
from pathlib import Path

h = hashlib.sha256()
with Path(sys.argv[1]).open("rb") as handle:
    for chunk in iter(lambda: handle.read(1 << 20), b""):
        h.update(chunk)
print(h.hexdigest())
PY
)"
    printf '%s  %s\n' "$SETUP_SHA" "${SETUP_BASE}.exe" >> "$ROOT/dist/SHA256SUMS.txt"
  fi
fi

echo "== normalizing NTFS integrity level =="
normalize_integrity "$ROOT/dist"

echo "RELEASE_PACKAGE_OK dist=$OUT zip=$ZIP_OUT"
