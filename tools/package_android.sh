#!/usr/bin/env bash
# Package LWSMF (Godot 4.7.2 .NET) as an Android arm64 APK.
# Supports `--dry-run` preflight planning without running Godot export.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="0.1.0"
SLIM=false
BUILD_MODE="debug"
KEYSTORE_OPT=""
KS_USER_OPT="androiddebugkey"
KS_PASS_OPT="android"
KEY_PASS_OPT=""
FONT_OPT=""
TEMPLATE_OPT=""
MEDIA_PAK_OPT=""
OUT_DIR=""
DRY_RUN=false
FORCE=false

GODOT_BIN="${GODOT_BIN:-${GODOT:-$(command -v godot 2>/dev/null || command -v Godot_v4.7.2-stable_mono_win64_console.exe 2>/dev/null || true)}}"
DOTNET_BIN="${DOTNET_BIN:-${DOTNET:-$(command -v dotnet 2>/dev/null || true)}}"
PYTHON_BIN="${PYTHON_BIN:-${PYTHON:-$(command -v python3 2>/dev/null || command -v python 2>/dev/null || true)}}"

usage() {
  cat <<'EOF'
Usage: tools/package_android.sh [options]

  --version V       package version (default: 0.1.0)
  --slim            repack media.pak with WebP slimming before APK staging
  --media-pak FILE  use a prebuilt (for example pruned) media.pak
  --debug           export debug APK with auto/supplied debug keystore (default)
  --release         export release APK (requires --keystore)
  --keystore FILE   Android keystore path (.keystore / .jks)
  --ks-user NAME    keystore alias (default: androiddebugkey)
  --ks-pass PASS    keystore password (default: android)
  --key-pass PASS   private-key password (default: same as --ks-pass)
  --font FILE       optional Lolita.ttf to bundle into mobile_data
  --template FILE   optional custom Android APK template
  --out-dir DIR     output directory (default: dist/android/<ver> or dist/android-slim/<ver>)
  --dry-run         run preflight env check + payload plan only (no APK build)
  --force           replace existing output APK
  -h, --help        show this help
EOF
}

need_arg() {
  if [ "$#" -lt 2 ]; then
    echo "ERROR: option requires a value: $1" >&2
    exit 2
  fi
}

to_win_path() {
  local input="$1"
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

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)  need_arg "$@"; VERSION="$2"; shift 2 ;;
    --slim)     SLIM=true; shift ;;
    --debug)    BUILD_MODE="debug"; shift ;;
    --release)  BUILD_MODE="release"; shift ;;
    --keystore) need_arg "$@"; KEYSTORE_OPT="$2"; shift 2 ;;
    --ks-user)  need_arg "$@"; KS_USER_OPT="$2"; shift 2 ;;
    --ks-pass)  need_arg "$@"; KS_PASS_OPT="$2"; shift 2 ;;
    --key-pass) need_arg "$@"; KEY_PASS_OPT="$2"; shift 2 ;;
    --font)     need_arg "$@"; FONT_OPT="$2"; shift 2 ;;
    --template) need_arg "$@"; TEMPLATE_OPT="$2"; shift 2 ;;
    --media-pak) need_arg "$@"; MEDIA_PAK_OPT="$2"; shift 2 ;;
    --out-dir)  need_arg "$@"; OUT_DIR="$2"; shift 2 ;;
    --dry-run)  DRY_RUN=true; shift ;;
    --force)    FORCE=true; shift ;;
    -h|--help)  usage; exit 0 ;;
    *) echo "ERROR: unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

[ -n "$PYTHON_BIN" ] || { echo "ERROR: python not found" >&2; exit 2; }
if [ "$SLIM" = true ] && [ -n "$MEDIA_PAK_OPT" ]; then
  echo "ERROR: --slim and --media-pak are mutually exclusive" >&2
  exit 2
fi
if [ -n "$MEDIA_PAK_OPT" ] && [ ! -f "$(to_win_path "$MEDIA_PAK_OPT")" ]; then
  echo "ERROR: media.pak not found: $MEDIA_PAK_OPT" >&2
  exit 2
fi

if [ -z "$FONT_OPT" ]; then
  if [ -n "${LWSMF_FONT_PATH:-}" ] && [ -f "$LWSMF_FONT_PATH" ]; then
    FONT_OPT="$LWSMF_FONT_PATH"
  else
    for dir in "$ROOT/work/all" "$ROOT/work/all/Data"; do
      for name in "Lolita.ttf" "LOLITA.TTF" "lolita.ttf"; do
        if [ -f "$dir/$name" ]; then
          FONT_OPT="$dir/$name"
          break 2
        fi
      done
    done
  fi
fi

if [ -z "$OUT_DIR" ]; then
  if [ "$SLIM" = true ]; then
    OUT_DIR="$ROOT/dist/android-slim/$VERSION"
  else
    OUT_DIR="$ROOT/dist/android/$VERSION"
  fi
fi

SUFFIX=""
[ "$SLIM" = true ] && SUFFIX="-slim"
APK_NAME="Lwsmf-${VERSION}-android-arm64${SUFFIX}.apk"
APK_OUT="$OUT_DIR/$APK_NAME"

STAGE_DIR="$ROOT/godot/mobile_data"
PRESET_FILE="$ROOT/godot/export_presets.cfg"
PRESET_BAK="$ROOT/work/out/export_presets.android.$$.cfg"
SLIM_PAK="$ROOT/work/out/android/media-slim.pak"
SLN="$ROOT/godot/Lwsmf.sln"
SLN_CREATED=false

cleanup() {
  rm -rf "$STAGE_DIR"
  if [ -f "$PRESET_BAK" ]; then
    cp -f "$PRESET_BAK" "$PRESET_FILE"
    rm -f "$PRESET_BAK"
  fi
  if [ "$SLN_CREATED" = true ]; then
    rm -f "$SLN"
  fi
}
trap cleanup EXIT

echo "== Android preflight check =="
"$PYTHON_BIN" "$(to_win_path "$ROOT/tools/mobile/env_check.py")" --target android

GD_WIN="$(to_win_path "$ROOT/game_data")"
WA_WIN="$(to_win_path "$ROOT/work/all")"

if [ "$DRY_RUN" = true ]; then
  echo "== Android data plan (--dry-run) =="
  PLAN_ARGS=(plan --game-data "$GD_WIN" --work-all "$WA_WIN")
  if [ "$SLIM" = true ]; then
    echo "NOTE: --slim --dry-run estimates from existing media.pak; pack_media.py --slim runs on full build."
  fi
  [ -n "$MEDIA_PAK_OPT" ] && PLAN_ARGS+=(--media-pak "$(to_win_path "$MEDIA_PAK_OPT")")
  [ -n "$FONT_OPT" ] && PLAN_ARGS+=(--font "$(to_win_path "$FONT_OPT")")
  "$PYTHON_BIN" "$(to_win_path "$ROOT/tools/mobile/android_data.py")" "${PLAN_ARGS[@]}"
  echo "ANDROID_PACKAGE_DRY_RUN_OK out=$APK_OUT"
  exit 0
fi

# Strict full-build validation
"$PYTHON_BIN" "$(to_win_path "$ROOT/tools/mobile/env_check.py")" --target android --strict --setup-debug-keystore

[ -n "$GODOT_BIN" ] && [ -x "$GODOT_BIN" ] || { echo "ERROR: Godot 4.7.2 Mono console binary not found" >&2; exit 2; }
[ -n "$DOTNET_BIN" ] && [ -x "$DOTNET_BIN" ] || { echo "ERROR: .NET 9 SDK binary not found (set DOTNET_BIN)" >&2; exit 2; }
export DOTNET="${DOTNET:-$DOTNET_BIN}"
export DOTNET_ROOT="${DOTNET_ROOT:-$(dirname "$DOTNET_BIN")}"
export PATH="$(dirname "$DOTNET_BIN"):$PATH"

if [ -e "$APK_OUT" ] && [ "$FORCE" != true ]; then
  echo "ERROR: APK already exists (pass --force): $APK_OUT" >&2
  exit 2
fi

PAK_ARG=()
if [ "$SLIM" = true ]; then
  echo "== building slim media.pak for Android =="
  mkdir -p "$(dirname "$SLIM_PAK")"
  "$PYTHON_BIN" "$(to_win_path "$ROOT/tools/pack_media.py")" --from-pak "$WA_WIN/media.pak" --out "$(to_win_path "$SLIM_PAK")" --slim
  PAK_ARG=(--media-pak "$(to_win_path "$SLIM_PAK")")
elif [ -n "$MEDIA_PAK_OPT" ]; then
  PAK_ARG=(--media-pak "$(to_win_path "$MEDIA_PAK_OPT")")
fi

echo "== staging godot/mobile_data =="
STAGE_ARGS=(stage --version "$VERSION" --game-data "$GD_WIN" --work-all "$WA_WIN" --dest "$(to_win_path "$STAGE_DIR")")
[ "${#PAK_ARG[@]}" -gt 0 ] && STAGE_ARGS+=("${PAK_ARG[@]}")
[ -n "$FONT_OPT" ] && STAGE_ARGS+=(--font "$(to_win_path "$FONT_OPT")")
"$PYTHON_BIN" "$(to_win_path "$ROOT/tools/mobile/android_data.py")" "${STAGE_ARGS[@]}"

mkdir -p "$ROOT/work/out" "$OUT_DIR"
cp -f "$PRESET_FILE" "$PRESET_BAK"
PATCH_SETS=()
[ -n "$TEMPLATE_OPT" ] && PATCH_SETS+=(--set "custom_template/${BUILD_MODE}=$(to_win_path "$TEMPLATE_OPT")")
if [ "$BUILD_MODE" = "debug" ]; then
  SIGN_KS="${KEYSTORE_OPT:-$ROOT/work/out/android/debug.keystore}"
  PATCH_SETS+=(--set "keystore/debug=$(to_win_path "$SIGN_KS")" --set "keystore/debug_user=$KS_USER_OPT" --set "keystore/debug_password=$KS_PASS_OPT")
else
  [ -n "$KEYSTORE_OPT" ] || { echo "ERROR: --release requires --keystore" >&2; exit 2; }
  SIGN_KS="$KEYSTORE_OPT"
  PATCH_SETS+=(--set "keystore/release=$(to_win_path "$SIGN_KS")" --set "keystore/release_user=$KS_USER_OPT" --set "keystore/release_password=$KS_PASS_OPT")
fi
if [ "${#PATCH_SETS[@]}" -gt 0 ]; then
  "$PYTHON_BIN" "$(to_win_path "$ROOT/tools/mobile/export_preset_template.py")" --preset-file "$(to_win_path "$PRESET_FILE")" --preset "Android" "${PATCH_SETS[@]}"
fi

if [ ! -f "$SLN" ]; then
  "$DOTNET_BIN" new sln --format sln --name Lwsmf --output "$(to_win_path "$ROOT/godot")" >/dev/null
  "$DOTNET_BIN" sln "$(to_win_path "$SLN")" add "$(to_win_path "$ROOT/godot/Lwsmf.csproj")" >/dev/null
  SLN_CREATED=true
fi

echo "== exporting Android APK ($BUILD_MODE) =="
DOTNET_CFG="ExportDebug"
EXPORT_FLAG="--export-debug"
if [ "$BUILD_MODE" = "release" ]; then
  DOTNET_CFG="ExportRelease"
  EXPORT_FLAG="--export-release"
fi
"$DOTNET_BIN" build "$(to_win_path "$ROOT/godot/Lwsmf.csproj")" -c "$DOTNET_CFG" -p:GodotTargetPlatform=android -v:minimal
"$GODOT_BIN" --headless --path "$(to_win_path "$ROOT/godot")" "$EXPORT_FLAG" "Android" "$(to_win_path "$APK_OUT")"

echo "== verifying exported APK =="
"$PYTHON_BIN" "$(to_win_path "$ROOT/tools/mobile/android_data.py")" verify-apk "$(to_win_path "$APK_OUT")"

echo "== trimming, aligning, and signing APK =="
POSTPROCESS_ARGS=(
  --apk "$(to_win_path "$APK_OUT")"
  --keystore "$(to_win_path "$SIGN_KS")"
  --ks-user "$KS_USER_OPT"
  --ks-pass "$KS_PASS_OPT"
)
[ -n "$KEY_PASS_OPT" ] && POSTPROCESS_ARGS+=(--key-pass "$KEY_PASS_OPT")
"$PYTHON_BIN" "$(to_win_path "$ROOT/tools/mobile/postprocess_android_apk.py")" "${POSTPROCESS_ARGS[@]}"

echo "== verifying postprocessed APK =="
"$PYTHON_BIN" "$(to_win_path "$ROOT/tools/mobile/android_data.py")" verify-apk "$(to_win_path "$APK_OUT")"

"$PYTHON_BIN" - "$(to_win_path "$APK_OUT")" <<'PY'
import hashlib, sys
from pathlib import Path
apk = Path(sys.argv[1])
sha = hashlib.sha256()
with apk.open("rb") as handle:
    for chunk in iter(lambda: handle.read(4 * 1024 * 1024), b""):
        sha.update(chunk)
sha = sha.hexdigest()
(apk.parent / "SHA256SUMS.txt").write_text(f"{sha}  {apk.name}\n", encoding="utf-8")
print(f"ANDROID_PACKAGE_OK apk={apk} bytes={apk.stat().st_size} sha256={sha}")
PY
