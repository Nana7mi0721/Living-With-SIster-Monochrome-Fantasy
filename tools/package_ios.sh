#!/usr/bin/env bash
# Export and build an unsigned iOS arm64 IPA (macOS + Xcode only).
# On Windows/CI-artifact workflows, build the engine-only IPA on macOS and
# inject local game_data/media.pak via tools/inject_ios_data.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="0.1.0"
OUT_DIR=""
GAME_DATA_DIR=""
WORK_ALL_DIR=""
SLIM=false
FONT_OPT=""
GODOT_BIN="${GODOT_BIN:-${GODOT:-$(command -v godot 2>/dev/null || command -v Godot 2>/dev/null || true)}}"
DOTNET_BIN="${DOTNET_BIN:-${DOTNET:-$(command -v dotnet 2>/dev/null || true)}}"
PYTHON_BIN="${PYTHON_BIN:-${PYTHON:-$(command -v python3 2>/dev/null || command -v python 2>/dev/null || true)}}"
UNSIGNED=1
DRY_RUN=false
FORCE=false

usage() {
  cat <<'EOF'
Usage: tools/package_ios.sh [options]

  --version V        package version (default: 0.1.0)
  --out-dir DIR      output directory (default: dist/ios/<version>)
  --game-data DIR    optional game_data directory to inject after build
  --work-all DIR     optional work/all directory (requires --game-data)
  --slim             repack media.pak with WebP slimming when injecting data
  --font FILE        optional Lolita.ttf to place at <app>/work/all/Lolita.ttf
  --godot PATH       Godot 4.7.2 Mono executable
  --dotnet PATH      dotnet 8 executable
  --unsigned         build unsigned .app -> .ipa (default & required)
  --dry-run          check macOS toolchain & preset without exporting
  --force            replace existing output directory under dist/ios or work/out
  -h, --help         show this help
EOF
}

need_arg() {
  if [ "$#" -lt 2 ]; then
    echo "ERROR: option requires a value: $1" >&2
    exit 2
  fi
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)   need_arg "$@"; VERSION="$2"; shift 2 ;;
    --out-dir)   need_arg "$@"; OUT_DIR="$2"; shift 2 ;;
    --game-data) need_arg "$@"; GAME_DATA_DIR="$2"; shift 2 ;;
    --work-all)  need_arg "$@"; WORK_ALL_DIR="$2"; shift 2 ;;
    --slim)      SLIM=true; shift ;;
    --font)      need_arg "$@"; FONT_OPT="$2"; shift 2 ;;
    --godot)     need_arg "$@"; GODOT_BIN="$2"; shift 2 ;;
    --dotnet)    need_arg "$@"; DOTNET_BIN="$2"; shift 2 ;;
    --unsigned)  UNSIGNED=1; shift ;;
    --dry-run)   DRY_RUN=true; shift ;;
    --force)     FORCE=true; shift ;;
    -h|--help)   usage; exit 0 ;;
    *) echo "ERROR: unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

HOST_OS="$(uname -s 2>/dev/null || echo unknown)"
if [ "$HOST_OS" != "Darwin" ]; then
  echo "ERROR: tools/package_ios.sh requires macOS with Xcode (current host: $HOST_OS)." >&2
  echo "       On Windows, trigger .github/workflows/ios-unsigned.yml for the engine IPA, then run:" >&2
  echo "         bash tools/inject_ios_data.sh <engine-only.ipa> --game-data game_data --work-all work/all [--out <full.ipa>]" >&2
  exit 1
fi

[ -n "$PYTHON_BIN" ] || { echo "ERROR: python3 not found" >&2; exit 2; }
[ -n "$OUT_DIR" ] || OUT_DIR="$ROOT/dist/ios/$VERSION"

if [ "$DRY_RUN" = true ]; then
  "$PYTHON_BIN" "$ROOT/tools/mobile/env_check.py" --target ios --strict
  echo "IOS_PACKAGE_DRY_RUN_OK out=$OUT_DIR"
  exit 0
fi

"$PYTHON_BIN" "$ROOT/tools/mobile/env_check.py" --target ios --strict
[ -n "$GODOT_BIN" ] && [ -x "$GODOT_BIN" ] || { echo "ERROR: Godot 4.7.2 Mono not found" >&2; exit 2; }
[ -n "$DOTNET_BIN" ] && [ -x "$DOTNET_BIN" ] || { echo "ERROR: dotnet 8 not found" >&2; exit 2; }
export DOTNET="${DOTNET:-$DOTNET_BIN}"
export DOTNET_ROOT="${DOTNET_ROOT:-$(dirname "$DOTNET_BIN")}"
export PATH="$(dirname "$DOTNET_BIN"):$PATH"

if [ -e "$OUT_DIR" ] && [ "$FORCE" != true ]; then
  echo "ERROR: output dir exists (pass --force): $OUT_DIR" >&2
  exit 2
fi
rm -rf -- "$OUT_DIR"
mkdir -p "$OUT_DIR"

SLN="$ROOT/godot/Lwsmf.sln"
SLN_CREATED=false
cleanup() {
  if [ "$SLN_CREATED" = true ]; then
    rm -f "$SLN"
  fi
}
trap cleanup EXIT

if [ ! -f "$SLN" ]; then
  "$DOTNET_BIN" new sln --name Lwsmf --output "$ROOT/godot" >/dev/null
  "$DOTNET_BIN" sln "$SLN" add "$ROOT/godot/Lwsmf.csproj" >/dev/null
  SLN_CREATED=true
fi

echo "== importing & exporting Godot iOS project =="
"$GODOT_BIN" --headless --path "$ROOT/godot" --import --quit || true
EXPORT_TARGET="$OUT_DIR/Lwsmf.xcodeproj"
"$GODOT_BIN" --headless --path "$ROOT/godot" --export-release "iOS" "$EXPORT_TARGET"

XCODEPROJ="$(find "$OUT_DIR" -maxdepth 3 -type d -name '*.xcodeproj' | head -n1 || true)"
[ -n "$XCODEPROJ" ] || { echo "ERROR: .xcodeproj not found under $OUT_DIR" >&2; exit 1; }

SCHEMES="$(xcodebuild -list -project "$XCODEPROJ" | awk '/Schemes:/{f=1;next} f&&/^[[:space:]]*$/{exit} f{gsub(/^[[:space:]]+|[[:space:]]+$/,""); if(length($0)) print}')"
SCHEME="$(printf '%s\n' "$SCHEMES" | grep -Fx "Lwsmf" || printf '%s\n' "$SCHEMES" | head -n1)"
[ -n "$SCHEME" ] || { echo "ERROR: no Xcode scheme detected" >&2; exit 1; }

DERIVED_DATA="$OUT_DIR/DerivedData"
echo "== building unsigned Release .app (scheme=$SCHEME) =="
xcodebuild \
  -project "$XCODEPROJ" \
  -scheme "$SCHEME" \
  -configuration Release \
  -sdk iphoneos \
  -derivedDataPath "$DERIVED_DATA" \
  CODE_SIGNING_ALLOWED=NO \
  CODE_SIGNING_REQUIRED=NO \
  build

APP_SRC="$(find "$DERIVED_DATA/Build/Products" -type d -name '*.app' | head -n1 || true)"
[ -n "$APP_SRC" ] || { echo "ERROR: .app build output missing" >&2; exit 1; }

STAGE_DIR="$OUT_DIR/ipa-stage"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/Payload"
ditto "$APP_SRC" "$STAGE_DIR/Payload/$(basename "$APP_SRC")"

IPA_NAME="Lwsmf-${VERSION}-ios-arm64-unsigned.ipa"
IPA_PATH="$OUT_DIR/$IPA_NAME"
rm -f "$IPA_PATH"
(cd "$STAGE_DIR" && ditto -c -k --sequesterRsrc --keepParent "Payload" "$IPA_PATH")
rm -rf "$STAGE_DIR" "$DERIVED_DATA"

if [ -n "$GAME_DATA_DIR" ] && [ -n "$WORK_ALL_DIR" ]; then
  INJ_OUT="$OUT_DIR/Lwsmf-${VERSION}-ios-arm64-unsigned-full.ipa"
  INJ_ARGS=("$IPA_PATH" --game-data "$GAME_DATA_DIR" --work-all "$WORK_ALL_DIR" --out "$INJ_OUT" --force)
  if [ "$SLIM" = true ]; then
    SLIM_PAK="$OUT_DIR/media-slim.pak"
    "$PYTHON_BIN" "$ROOT/tools/pack_media.py" --from-pak "$WORK_ALL_DIR/media.pak" --out "$SLIM_PAK" --slim
    INJ_ARGS+=(--media-pak "$SLIM_PAK")
  fi
  [ -n "$FONT_OPT" ] && INJ_ARGS+=(--font "$FONT_OPT")
  "$PYTHON_BIN" "$ROOT/tools/mobile/ios_inject_data.py" "${INJ_ARGS[@]}"
  rm -f "$OUT_DIR/media-slim.pak"
fi

(cd "$OUT_DIR" && shasum -a 256 *.ipa > SHA256SUMS.txt)
echo "IOS_PACKAGE_OK out=$OUT_DIR ipa=$IPA_PATH"
