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
LOSSLESS=false
BROTLI=false
FONT_OPT=""
TEMPLATE_OPT=""
OFFICIAL_TEMPLATE=false
GODOT_BIN="${GODOT_BIN:-${GODOT:-$(command -v godot 2>/dev/null || command -v Godot 2>/dev/null || true)}}"
DOTNET_BIN="${DOTNET_BIN:-${DOTNET:-$(command -v dotnet 2>/dev/null || true)}}"
PYTHON_BIN="${PYTHON_BIN:-${PYTHON:-$(command -v python3 2>/dev/null || command -v python 2>/dev/null || true)}}"
UNSIGNED=1
DRY_RUN=false
FORCE=false
ALLOW_PLACEHOLDER=false

usage() {
  cat <<'EOF'
Usage: tools/package_ios.sh [options]

  --version V        package version (default: 0.1.0)
  --out-dir DIR      output directory (default: dist/ios/<version>)
  --game-data DIR    optional game_data directory to inject after build
  --work-all DIR     optional work/all directory (requires --game-data)
  --lossless         repack media.pak with pixel-exact lossless WebP when injecting data
  --slim             repack media.pak with WebP slimming when injecting data
  --brotli           compress large game_data JSONs as .json.br when injecting data
  --font FILE        optional Lolita.ttf to place at <app>/work/all/Lolita.ttf
  --template FILE    optional custom iOS ios.zip export template (auto-detected if present)
  --official-template  force Godot's official iOS template even if a custom template exists
  --godot PATH       Godot 4.7.2 Mono executable
  --dotnet PATH      dotnet 8 executable
  --unsigned         build unsigned .app -> .ipa (default & required)
  --dry-run          check macOS toolchain & preset without exporting
  --force            replace existing output directory under dist/ios or work/out
  --allow-placeholder  allow placeholder launcher icons if _publish artwork is absent
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
    --lossless)  LOSSLESS=true; shift ;;
    --slim)      SLIM=true; shift ;;
    --brotli)    BROTLI=true; shift ;;
    --font)      need_arg "$@"; FONT_OPT="$2"; shift 2 ;;
    --template)  need_arg "$@"; TEMPLATE_OPT="$2"; shift 2 ;;
    --official-template) OFFICIAL_TEMPLATE=true; shift ;;
    --godot)     need_arg "$@"; GODOT_BIN="$2"; shift 2 ;;
    --dotnet)    need_arg "$@"; DOTNET_BIN="$2"; shift 2 ;;
    --unsigned)  UNSIGNED=1; shift ;;
    --dry-run)   DRY_RUN=true; shift ;;
    --force)     FORCE=true; shift ;;
    --allow-placeholder) ALLOW_PLACEHOLDER=true; shift ;;
    -h|--help)   usage; exit 0 ;;
    *) echo "ERROR: unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [ -n "$TEMPLATE_OPT" ] && [ "$OFFICIAL_TEMPLATE" = true ]; then
  echo "ERROR: --template and --official-template are mutually exclusive" >&2
  exit 2
fi
if [ -z "$TEMPLATE_OPT" ] && [ "$OFFICIAL_TEMPLATE" != true ] && [ "${LWSMF_OFFICIAL_TEMPLATE:-0}" != "1" ]; then
  for _tpl in \
    "$ROOT/work/out/templates/ios/ios.zip" \
    "$ROOT/dist/templates/ios/ios.zip"
  do
    if [ -s "$_tpl" ]; then
      TEMPLATE_OPT="$_tpl"
      echo "== auto-detected custom iOS template: $TEMPLATE_OPT =="
      break
    fi
  done
fi

HOST_OS="$(uname -s 2>/dev/null || echo unknown)"
if { [ -n "$GAME_DATA_DIR" ] && [ -z "$WORK_ALL_DIR" ]; } || { [ -z "$GAME_DATA_DIR" ] && [ -n "$WORK_ALL_DIR" ]; }; then
  echo "ERROR: --game-data and --work-all must be specified together" >&2
  exit 2
fi
if [ -z "$FONT_OPT" ] && [ -n "$WORK_ALL_DIR" ]; then
  if [ -n "${LWSMF_FONT_PATH:-}" ] && [ -f "$LWSMF_FONT_PATH" ]; then
    FONT_OPT="$LWSMF_FONT_PATH"
  else
    for dir in "$WORK_ALL_DIR" "$WORK_ALL_DIR/Data"; do
      for name in "Lolita.ttf" "LOLITA.TTF" "lolita.ttf"; do
        if [ -f "$dir/$name" ]; then
          FONT_OPT="$dir/$name"
          break 2
        fi
      done
    done
  fi
fi
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
PRESET_FILE="$ROOT/godot/export_presets.cfg"
PRESET_BAK="$OUT_DIR/export_presets.ios.$$.cfg"
ICON_OUT="$ROOT/work/out/release_icons.ios.$$"
ICON_STAGE="$ROOT/godot/assets/boot/generated"
ICON_STAGE_CREATED=false
cleanup() {
  if [ -f "$PRESET_BAK" ]; then
    cp -f "$PRESET_BAK" "$PRESET_FILE"
    rm -f "$PRESET_BAK"
  fi
  if [ "$ICON_STAGE_CREATED" = true ]; then
    rm -rf "$ICON_STAGE"
  fi
  rm -rf "$ICON_OUT"
  if [ "$SLN_CREATED" = true ]; then
    rm -f "$SLN" "$ROOT/godot/Lwsmf.slnx"
  fi
}
trap cleanup EXIT

VERSION_CODE="$("$PYTHON_BIN" - "$VERSION" <<'PY'
import re
import sys

match = re.match(r"^(\d+)\.(\d+)\.(\d+)", sys.argv[1])
print(int(match.group(1)) * 10000 + int(match.group(2)) * 100 + int(match.group(3)) if match else 1)
PY
)"

echo "== preparing iOS icons =="
[ ! -d "$ICON_STAGE" ] && ICON_STAGE_CREATED=true
ICON_ARGS=(
  --source-dir "${LWSMF_PUBLISH_DIR:-$ROOT/work/all/_publish}"
  --out-dir "$ICON_OUT"
  --stage-res "$ICON_STAGE"
)
if [ "$ALLOW_PLACEHOLDER" = true ] || [ "${LWSMF_ALLOW_PLACEHOLDER_ICONS:-0}" = "1" ]; then
  ICON_ARGS+=(--allow-placeholder)
fi
"$PYTHON_BIN" "$ROOT/tools/sync_release_icons.py" "${ICON_ARGS[@]}"

cp -f "$PRESET_FILE" "$PRESET_BAK"
PATCH_SETS=(
  --set "application/short_version=$VERSION"
  --set "application/version=$VERSION_CODE"
)
if [ -n "$TEMPLATE_OPT" ]; then
  [ -f "$TEMPLATE_OPT" ] || { echo "ERROR: custom iOS template not found: $TEMPLATE_OPT" >&2; exit 2; }
  PATCH_SETS+=(--set "custom_template/release=$(cd "$(dirname "$TEMPLATE_OPT")" && pwd)/$(basename "$TEMPLATE_OPT")")
fi
"$PYTHON_BIN" "$ROOT/tools/mobile/export_preset_template.py" \
  --preset-file "$PRESET_FILE" \
  --preset "iOS" \
  "${PATCH_SETS[@]}"

if [ ! -f "$SLN" ]; then
  "$DOTNET_BIN" new sln --format sln --name Lwsmf --output "$ROOT/godot" >/dev/null 2>&1 \
    || "$DOTNET_BIN" new sln --name Lwsmf --output "$ROOT/godot" >/dev/null
  SLN_CREATED=true
  "$DOTNET_BIN" sln "$SLN" add "$ROOT/godot/Lwsmf.csproj" >/dev/null
fi

echo "== importing & exporting Godot iOS project =="
"$GODOT_BIN" --headless --path "$ROOT/godot" --import --quit || true
EXPORT_TARGET="$OUT_DIR/Lwsmf.xcodeproj"
"$GODOT_BIN" --headless --path "$ROOT/godot" --export-release "iOS" "$EXPORT_TARGET"

XCODEPROJ="$(find "$OUT_DIR" -maxdepth 3 -type d -name '*.xcodeproj' | head -n1 || true)"
[ -n "$XCODEPROJ" ] || { echo "ERROR: .xcodeproj not found under $OUT_DIR" >&2; exit 1; }

echo "== verifying iOS launch screen SplashImage.imageset =="
SPLASH_SET="$(find "$OUT_DIR" -type d -name 'SplashImage.imageset' | head -n1 || true)"
if [ -z "$SPLASH_SET" ]; then
  echo "ERROR: SplashImage.imageset missing in exported iOS project under $OUT_DIR; custom launch screen did not export." >&2
  exit 1
fi
"$PYTHON_BIN" - "$SPLASH_SET" "$ROOT/godot/assets/boot/splash_static.png" <<'PY'
import hashlib, sys
from pathlib import Path

imageset = Path(sys.argv[1])
expected_src = Path(sys.argv[2])
pngs = sorted(p for p in imageset.glob("*.png") if p.is_file() and p.stat().st_size > 0)
if not pngs:
    print(f"ERROR: SplashImage.imageset has no PNG files: {imageset}", file=sys.stderr)
    raise SystemExit(1)
if not expected_src.is_file():
    print(f"ERROR: custom splash source missing: {expected_src}", file=sys.stderr)
    raise SystemExit(1)

expected_sha = hashlib.sha256(expected_src.read_bytes()).hexdigest()
matched = [p.name for p in pngs if hashlib.sha256(p.read_bytes()).hexdigest() == expected_sha]

if not matched:
    # Godot's iOS exporter re-encodes splash PNGs through libpng, changing file bytes while preserving pixels.
    from PIL import Image, ImageChops

    resampling = getattr(Image, "Resampling", Image).LANCZOS
    with Image.open(expected_src) as src_im:
        src_rgb = src_im.convert("RGB")
        details = []
        for p in pngs:
            with Image.open(p) as out_im:
                out_rgb = out_im.convert("RGB")
                ref = src_rgb if out_rgb.size == src_rgb.size else src_rgb.resize(out_rgb.size, resampling)
                diff = ImageChops.difference(out_rgb, ref)
                hist = diff.histogram()
                # Mean absolute channel diff across all pixels (0 = pixel-exact).
                total_vals = out_rgb.size[0] * out_rgb.size[1] * 3
                mean_diff = sum((i % 256) * count for i, count in enumerate(hist)) / max(1, total_vals)
                details.append(f"{p.name}({out_rgb.size[0]}x{out_rgb.size[1]},mad={mean_diff:.2f})")
                if mean_diff <= 4.0:
                    matched.append(p.name)
    if not matched:
        print(
            f"ERROR: SplashImage.imageset PNGs ({', '.join(details)}) do not match "
            f"{expected_src.name} ({src_rgb.size[0]}x{src_rgb.size[1]}); Godot default splash may have leaked.",
            file=sys.stderr,
        )
        raise SystemExit(1)

print(f"IOS_SPLASH_VERIFY_OK imageset={imageset} matched={','.join(matched)}")
PY

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
  CODE_SIGN_IDENTITY="" \
  DEVELOPMENT_TEAM="" \
  STRIP_INSTALLED_PRODUCT=YES \
  DEPLOYMENT_POSTPROCESSING=YES \
  COPY_PHASE_STRIP=YES \
  DEBUG_INFORMATION_FORMAT=dwarf \
  build

APP_SRC="$(find "$DERIVED_DATA/Build/Products" -type d -name '*.app' | head -n1 || true)"
[ -n "$APP_SRC" ] || { echo "ERROR: .app build output missing" >&2; exit 1; }

STAGE_DIR="$OUT_DIR/ipa-stage"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/Payload"
ditto "$APP_SRC" "$STAGE_DIR/Payload/$(basename "$APP_SRC")"

IPA_NAME="LWSMF-${VERSION}-ios-arm64.ipa"
IPA_PATH="$OUT_DIR/$IPA_NAME"
rm -f "$IPA_PATH"
(cd "$STAGE_DIR" && ditto -c -k --sequesterRsrc --keepParent "Payload" "$IPA_PATH")
rm -rf "$STAGE_DIR" "$DERIVED_DATA"

if [ -n "$GAME_DATA_DIR" ] && [ -n "$WORK_ALL_DIR" ]; then
  INJ_OUT="$OUT_DIR/LWSMF-${VERSION}-ios-arm64-full.ipa"
  INJ_ARGS=("$IPA_PATH" --game-data "$GAME_DATA_DIR" --work-all "$WORK_ALL_DIR" --out "$INJ_OUT" --force)
  if [ "$LOSSLESS" = true ] && [ "$SLIM" = true ]; then
    echo "ERROR: --lossless and --slim are mutually exclusive" >&2
    exit 2
  fi
  if [ "$LOSSLESS" = true ]; then
    INJ_ARGS+=(--lossless)
  elif [ "$SLIM" = true ]; then
    SLIM_PAK="$OUT_DIR/media-slim.pak"
    "$PYTHON_BIN" "$ROOT/tools/pack_media.py" --from-pak "$WORK_ALL_DIR/media.pak" --out "$SLIM_PAK" --slim
    INJ_ARGS+=(--media-pak "$SLIM_PAK")
  fi
  [ -n "$FONT_OPT" ] && INJ_ARGS+=(--font "$FONT_OPT")
  [ "$BROTLI" = true ] && INJ_ARGS+=(--brotli)
  "$PYTHON_BIN" "$ROOT/tools/mobile/ios_inject_data.py" "${INJ_ARGS[@]}"
  rm -f "$OUT_DIR/media-slim.pak" "$OUT_DIR/media-lossless.pak"
fi

(cd "$OUT_DIR" && shasum -a 256 *.ipa > SHA256SUMS.txt)
echo "IOS_PACKAGE_OK out=$OUT_DIR ipa=$IPA_PATH"
