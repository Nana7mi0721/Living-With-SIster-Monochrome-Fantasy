#!/usr/bin/env bash
# M0 环境门槛 / CI 入口：编译 C# 并跑 headless 冒烟、存档导入、Oracle 对拍与
# GDScript 表现层测试。
# 用法： tools/godot_check.sh                       全 headless（默认并行加速）
#        LWsmf_GODOT_JOBS=1 tools/godot_check.sh    严格串行模式
#        LWsmf_GATE_FAST=1 tools/godot_check.sh     跳过两个 M13 终局长回放（迭代用，合并前仍跑完整门禁）
#        LWsmf_RENDER=1 tools/godot_check.sh        额外用真实渲染器跑 101/150 像素校验
# 退出码 0 = 引擎+C#+场景+存档+Oracle 全部通过。
set -euo pipefail

IMO="$(cd "$(dirname "$0")/.." && pwd)"
# Per-machine overrides (GODOT, DOTNET, ...) live in an untracked file so no local path is committed.
if [ -f "$IMO/tools/local.env.sh" ]; then
  # shellcheck disable=SC1091
  . "$IMO/tools/local.env.sh"
fi
GODOT="${GODOT:-${GODOT_BIN:-$(command -v godot 2>/dev/null || command -v Godot_v4.7.2-stable_mono_win64_console.exe 2>/dev/null || true)}}"
DOTNET="${DOTNET:-${DOTNET_BIN:-$(command -v dotnet 2>/dev/null || true)}}"
PYTHON="${PYTHON:-${PYTHON_BIN:-$(command -v python3 2>/dev/null || command -v python 2>/dev/null || true)}}"
# Isolate the gate from the player profile: 220/221/222 persist system data beside the save
# slot, and a previous run's file would change the first-run route branches (Ce253 day 3).
# GameSessionController.DefaultSavePath honours LWsmf_SAVE_DIR.
ROOT_TEMP="$(mktemp -d)"
trap 'rm -rf "$ROOT_TEMP"' EXIT
export LWsmf_SAVE_DIR="${LWsmf_SAVE_DIR:-$ROOT_TEMP/default_save}"
mkdir -p "$LWsmf_SAVE_DIR" "$IMO/reports"
if [ -n "$DOTNET" ]; then
  DOTNET_DIR="$(cd "$(dirname "$DOTNET")" 2>/dev/null && pwd || dirname "$DOTNET")"
  export DOTNET
  export DOTNET_ROOT="${DOTNET_ROOT:-$DOTNET_DIR}"
  export PATH="$DOTNET_DIR:$PATH"
fi
export LWsmf_QUIET_HUD="${LWsmf_QUIET_HUD:-1}"
# A Roslyn compiler server started by `--build-solutions` inherits Godot's stdio, so Godot
# blocks until that server idles out (10 minutes on a cold start). Compile in-process.
export UseSharedCompilation=false MSBUILDDISABLENODEREUSE=1 DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1
default_jobs=8
case "$(uname -s 2>/dev/null || echo "")" in
  MINGW*|MSYS*|CYGWIN*) default_jobs=4 ;;
esac
JOBS="${LWsmf_GODOT_JOBS:-$default_jobs}"
if [ "${LWsmf_SERIAL:-0}" = "1" ]; then
  JOBS=1
fi

if [ -z "$GODOT" ] || [ ! -x "$GODOT" ]; then
  echo "godot not found: set GODOT=/path/to/Godot_v4.7.2-stable_mono_win64_console.exe" >&2
  exit 2
fi
if [ -z "$DOTNET" ] || { [ ! -x "$DOTNET" ] && ! command -v "$DOTNET" >/dev/null 2>&1; }; then
  echo "dotnet not found: set DOTNET=/path/to/dotnet" >&2
  exit 2
fi
if [ -z "$PYTHON" ] || ! command -v "$PYTHON" >/dev/null 2>&1; then
  echo "python not found: set PYTHON=/path/to/python3" >&2
  exit 2
fi

echo "[1/38] versions"
"$GODOT" --headless --version | head -1

echo "[preflight] converter schema audit"
if ! out_validate="$(cd "$IMO" && "$PYTHON" -m converter validate --root work/all --out game_data --reports reports 2>&1)"; then
  echo "$out_validate"
  exit 1
fi
echo "$out_validate"
echo "$out_validate" | grep -q "DATABASE_SCHEMA_AUDIT_OK=True"

echo "[preflight] decisions & docs link lint"
"$PYTHON" "$IMO/tools/lint_decisions.py"

echo "[preflight] op150 negative-scale full scan"
"$PYTHON" "$IMO/oracle/scan_op150_negative_scale.py" \
  --programs "$IMO/game_data/programs.json" \
  --report "$IMO/reports/op150_negative_scale_scan.json"

echo "[2/38] build core library"
# Godot's headless --build-solutions can hang during shutdown after the C#
# compilation itself reports success, so invoke the SDK build directly.
# Build both Release (CI/headless) and Debug (GUI / editor play) so neither goes stale.
"$DOTNET" build "$IMO/godot/Lwsmf.csproj" -c Release
"$DOTNET" build "$IMO/godot/Lwsmf.csproj" -c Debug

# The GDScript presentation layer (W2) resolves other scripts by class_name, so the global
# class cache has to exist before any tests_gd scene runs in parallel.
# Isolate editor config dirs so host EditorSettings (update checks, ADB, concurrent editor runs) cannot block --quit.
echo "[13/38] gdscript class cache"
mkdir -p "$ROOT_TEMP/appdata" "$ROOT_TEMP/localappdata" "$ROOT_TEMP/xdg_cfg" "$ROOT_TEMP/xdg_data"
APPDATA="$ROOT_TEMP/appdata" LOCALAPPDATA="$ROOT_TEMP/localappdata" \
  XDG_CONFIG_HOME="$ROOT_TEMP/xdg_cfg" XDG_DATA_HOME="$ROOT_TEMP/xdg_data" \
  "$PYTHON" "$IMO/tools/run_godot_test.py" \
  --godot "$GODOT" --project "$IMO/godot" \
  --timeout 60 --quit-after 0 --no-check-only --quiet-pass \
  --log "$IMO/reports/gd_class_cache.log" \
  -- --import --quit

run_godot_marker() {
  local label="$1"
  local scene="$2"
  local marker="$3"
  local log="$4"
  local timeout_sec="${5:-60}"
  local t0=$SECONDS
  if ! "$PYTHON" "$IMO/tools/run_godot_test.py" \
      --godot "$GODOT" --project "$IMO/godot" \
      --timeout "$timeout_sec" --quit-after 300 --quiet-pass \
      --marker "$marker" --log "$log" \
      "$scene"; then
    printf '[%s] FAILED (%ds)\n' "$label" "$((SECONDS - t0))"
    return 1
  fi
  printf '[%s] %s (%ds)\n' "$label" "$marker" "$((SECONDS - t0))"
}

# ---- individual job entrypoints (run in isolated LWsmf_SAVE_DIR) -----------

job_m13_slot() {
  local slot="$1"
  local t0=$SECONDS
  M13_SLOT="$slot" M13_DAYS=30 "$PYTHON" "$IMO/tools/run_godot_test.py" \
    --godot "$GODOT" --project "$IMO/godot" \
    --timeout 180 --quit-after 0 --quiet-pass \
    --marker "M13_MAINLINE_ENDINGS_TEST_OK slot=$slot" \
    --log "$IMO/reports/m13_mainline_endings_slot${slot}.log" \
    res://tests_gd/M13MainlineEndingsTest.tscn
  echo "[M13 slot=$slot] OK ($((SECONDS - t0))s)"
}

job_35_m4_route() {
  local t0=$SECONDS
  "$PYTHON" "$IMO/tools/run_godot_test.py" \
    --godot "$GODOT" --project "$IMO/godot" \
    --timeout 90 --quit-after 300 --quiet-pass \
    --marker M4_FULL_ROUTE_TRACE_TEST_OK \
    --log "$IMO/reports/m4_full_route_test.log" \
    res://tests/M4FullRouteTraceTest.tscn
  "$PYTHON" "$IMO/oracle/diff_route.py" \
    "$IMO/oracle/fixtures/m4_full_route_rust_trace.json.gz" \
    "$IMO/reports/csharp_m4_route_trace.json" \
    --report "$IMO/reports/m4_route_diff.json"
  echo "[35/38] full M4 route trace diff OK ($((SECONDS - t0))s)"
}

job_27_main_route() {
  local t0=$SECONDS
  "$PYTHON" "$IMO/tools/run_godot_test.py" \
    --godot "$GODOT" --project "$IMO/godot" \
    --timeout 90 --quit-after 300 --quiet-pass \
    --marker ROUTE_TRACE_TEST_OK \
    --log "$IMO/reports/route_trace_test.log" \
    res://tests/RouteTraceTest.tscn
  "$PYTHON" "$IMO/oracle/diff_route.py" \
    "$IMO/oracle/fixtures/main_route_rust_trace.json.gz" \
    "$IMO/reports/csharp_route_trace.json" \
    --report "$IMO/reports/route_trace_diff.json"
  echo "[27/38] main route trace diff OK ($((SECONDS - t0))s)"
}

job_19_oracle_step() {
  local t0=$SECONDS
  "$PYTHON" "$IMO/tools/run_godot_test.py" \
    --godot "$GODOT" --project "$IMO/godot" \
    --timeout 60 --quit-after 300 --quiet-pass \
    --marker ORACLE_STEP_TRACE_OK \
    --log "$IMO/reports/oracle_step_trace_test.log" \
    res://tests/OracleStepTraceTest.tscn
  "$PYTHON" "$IMO/oracle/diff_trace.py" \
    "$IMO/oracle/fixtures/samplemapa_event0_418_rust_trace.json" \
    "$IMO/reports/csharp_step_trace.json" \
    --report "$IMO/reports/step_trace_diff.json"
  echo "[19/38] oracle step trace OK ($((SECONDS - t0))s)"
}

job_03_smoke() {
  local t0=$SECONDS
  local -a marker_args=()
  for m in \
    SMOKE_OK DATABASE_TEST_OK AUTO_EVENTS_TEST_OK BATTLE_COMMAND_TEST_OK \
    BATTLE_WIRING_TEST_OK BATTLE_TARGET_TEST_OK BATTLE_SKILL_TARGET_TEST_OK \
    BATTLE_MENU_BRIDGE_TEST_OK BATTLE_MATRIX_TEST_OK BATTLE_ESCAPE_TEST_OK \
    H_EVENT_GATE_TEST_OK CONTENT_MATRIX_TEST_OK M13_ALL_ENDINGS_TEST_OK \
    M13_RANDOM_EVENTS_TEST_OK OP150_NEGATIVE_SCALE_SCAN_OK
  do
    marker_args+=(--marker "$m")
  done
  "$PYTHON" "$IMO/tools/run_godot_test.py" \
    --godot "$GODOT" --project "$IMO/godot" \
    --timeout 60 --quit-after 300 --quiet-pass \
    "${marker_args[@]}" \
    --log "$IMO/reports/smoke.log" \
    res://tests/Smoke.tscn
  echo "[3/38] smoke test OK ($((SECONDS - t0))s)"
}

job_04_save_import() {
  local t0=$SECONDS
  "$PYTHON" "$IMO/tools/run_godot_test.py" \
    --godot "$GODOT" --project "$IMO/godot" \
    --timeout 60 --quit-after 300 --quiet-pass \
    --marker SAVE_IMPORT_OK \
    --marker SAVE_IMPORT_VALIDATION_OK \
    --log "$IMO/reports/save_import_test.log" \
    res://tests/SaveImportTest.tscn
  echo "[4/38] save import test OK ($((SECONDS - t0))s)"
}

job_05_trace_diff_cli() {
  local t0=$SECONDS
  "$PYTHON" "$IMO/oracle/diff_trace.py" \
    "$IMO/oracle/fixtures/ce42_reference_trace.json" \
    "$IMO/oracle/fixtures/ce42_reference_trace.json" \
    --report "$IMO/reports/trace_diff.json"
  echo "[5/38] oracle trace diff (cli) OK ($((SECONDS - t0))s)"
}

# Serialize tests that share Godot user:// so they cannot race on AppData files.
job_user_dir_lane() {
  run_godot_marker "14/38" "res://tests_gd/InputRouterTest.tscn" INPUT_ROUTER_TEST_OK "$IMO/reports/gd_InputRouterTest.log"
  run_godot_marker "15/38" "res://tests_gd/LayerOrderTest.tscn" LAYER_ORDER_TEST_OK "$IMO/reports/gd_LayerOrderTest.log"
  run_godot_marker "16/38" "res://tests_gd/AssetDBTest.tscn" ASSETDB_TEST_OK "$IMO/reports/gd_AssetDBTest.log"
  if [ "${LWsmf_RENDER:-0}" = "1" ]; then
    local shot
    LWsmf_RENDER=1 "$PYTHON" "$IMO/tools/run_godot_test.py" \
      --godot "$GODOT" --project "$IMO/godot" \
      --timeout 60 --quit-after 300 --quiet-pass \
      --rendering-driver opengl3 \
      --marker DIALOG_DRAW_TEST_OK \
      --log "$IMO/reports/gd_DialogDrawTest.log" \
      res://tests_gd/DialogDrawTest.tscn
    shot="$(sed -n 's/^DIALOG_DRAW_SCREENSHOT //p' "$IMO/reports/gd_DialogDrawTest.log" | head -1)"
    if [ -n "$shot" ] && [ -f "$shot" ]; then cp "$shot" "$IMO/reports/dialog_draw.png"; fi
    echo "[17/38] DIALOG_DRAW_TEST_OK (windowed)"

    local shot_line shot_before shot_after
    LWsmf_RENDER=1 "$PYTHON" "$IMO/tools/run_godot_test.py" \
      --godot "$GODOT" --project "$IMO/godot" \
      --timeout 60 --quit-after 300 --quiet-pass \
      --rendering-driver opengl3 \
      --marker PICTURE_ROUNDTRIP_TEST_OK \
      --log "$IMO/reports/gd_PictureRoundtripTest.log" \
      res://tests_gd/PictureRoundtripTest.tscn
    shot_line="$(sed -n 's/^PICTURE_ROUNDTRIP_SCREENSHOT //p' "$IMO/reports/gd_PictureRoundtripTest.log" | head -1)"
    shot_before="${shot_line%%|*}"
    shot_after="${shot_line#*|}"
    if [ -n "$shot_before" ] && [ -f "$shot_before" ]; then cp "$shot_before" "$IMO/reports/picture_roundtrip.png"; fi
    if [ -n "$shot_after" ] && [ -f "$shot_after" ]; then cp "$shot_after" "$IMO/reports/picture_roundtrip_replay.png"; fi
    echo "[18/38] PICTURE_ROUNDTRIP_TEST_OK (windowed)"
  else
    run_godot_marker "17/38" "res://tests_gd/DialogDrawTest.tscn" DIALOG_DRAW_TEST_OK "$IMO/reports/gd_DialogDrawTest.log"
    run_godot_marker "18/38" "res://tests_gd/PictureRoundtripTest.tscn" PICTURE_ROUNDTRIP_TEST_OK "$IMO/reports/gd_PictureRoundtripTest.log"
  fi
  run_godot_marker "20/38" "res://tests_gd/GameRootTest.tscn" GAME_ROOT_TEST_OK "$IMO/reports/gd_GameRootTest.log"
  # Optional 2x pack contract; reports OK immediately when LWsmf_HD_PAK is unset.
  run_godot_marker "HD pack" "res://tests_gd/HDPackTest.tscn" HD_PACK_TEST_OK "$IMO/reports/gd_HDPackTest.log"
  run_godot_marker "36/38" "res://tests_gd/SetupWizardTest.tscn" SETUP_WIZARD_TEST_OK "$IMO/reports/gd_SetupWizardTest.log"
}

run_job_wrapper() {
  local idx="$1"
  local label="$2"
  shift 2
  local job_save="$ROOT_TEMP/save_$idx"
  local job_log="$ROOT_TEMP/job_$idx.log"
  mkdir -p "$job_save"
  local t0=$SECONDS rc=0
  # The job body must run outside any `if`/`||` context: bash disables `set -e` there, which
  # let a failing diff or grep in the middle of a job function be reported as success.
  set +e
  ( set -e; LWsmf_SAVE_DIR="$job_save" "$@" ) > "$job_log" 2>&1
  rc=$?
  set -e
  echo "$((SECONDS - t0)) $label" > "$ROOT_TEMP/time_$idx"
  if [ "$rc" -eq 0 ]; then
    cat "$job_log"
    return 0
  fi
  echo "=== JOB FAILED: $label (exit $rc) ==="
  cat "$job_log"
  return "$rc"
}

declare -a JOB_LABELS=()
declare -a JOB_CMDS=()

add_job() {
  JOB_LABELS+=("$1")
  shift
  local IFS=$'\x1f'
  JOB_CMDS+=("$*")
}

# Schedule longest tests first to minimize tail latency across worker slots.
if [ "${LWsmf_GATE_FAST:-0}" = "1" ]; then
  echo "[M13] skipped: LWsmf_GATE_FAST=1 (two ~1 min day-90 endgame replays; run the full gate before merging)"
else
  for m13_slot in 1 8; do
    sav_rel="work/retail_backup/Save/SaveData0${m13_slot}.sav"
    if [ -f "$IMO/$sav_rel" ]; then
      add_job "M13 slot $m13_slot" job_m13_slot "$m13_slot"
    else
      echo "[M13 slot=$m13_slot] skipped: $sav_rel missing"
    fi
  done
fi
add_job "32/38 Ce253DayRouteTest" run_godot_marker "32/38" "res://tests_gd/Ce253DayRouteTest.tscn" CE253_DAY_ROUTE_TEST_OK "$IMO/reports/ce253_day_route.log"
add_job "35/38 M4FullRouteTrace" job_35_m4_route
add_job "31/38 ColdStartDayTest" run_godot_marker "31/38" "res://tests_gd/ColdStartDayTest.tscn" COLD_START_DAY_TEST_OK "$IMO/reports/cold_start_day.log"
add_job "27/38 RouteTrace" job_27_main_route
add_job "03/38 Smoke" job_03_smoke
add_job "user:// lane (14-18,20,36)" job_user_dir_lane
add_job "04/38 SaveImport" job_04_save_import
add_job "05/38 TraceDiffCli" job_05_trace_diff_cli
add_job "06/38 TraceDiffTest" run_godot_marker "6/38" "res://tests/TraceDiffTest.tscn" TRACE_DIFF_TEST_OK "$IMO/reports/trace_diff_test.log"
add_job "07/38 TitleTest" run_godot_marker "7/38" "res://tests/TitleTest.tscn" TITLE_TEST_OK "$IMO/reports/title_test.log"
add_job "08/38 NewGameTest" run_godot_marker "8/38" "res://tests/NewGameTest.tscn" NEW_GAME_TEST_OK "$IMO/reports/new_game_test.log"
add_job "09/38 PresentationTest" run_godot_marker "9/38" "res://tests/PresentationTest.tscn" PRESENTATION_TEST_OK "$IMO/reports/presentation_test.log"
add_job "10/38 MapRuntimeTest" run_godot_marker "10/38" "res://tests/MapRuntimeTest.tscn" MAP_RUNTIME_TEST_OK "$IMO/reports/map_runtime_test.log"
add_job "11/38 SaveLoadTest" run_godot_marker "11/38" "res://tests/SaveLoadTest.tscn" SAVE_LOAD_TEST_OK "$IMO/reports/save_load_test.log"
add_job "12/38 MapMessageTest" run_godot_marker "12/38" "res://tests/MapMessageTest.tscn" MAP_MESSAGE_TEST_OK "$IMO/reports/map_message_test.log"
add_job "19/38 OracleStepTrace" job_19_oracle_step
add_job "21/38 TerrainTest" run_godot_marker "21/38" "res://tests/TerrainTest.tscn" TERRAIN_TEST_OK "$IMO/reports/terrain_test.log"
add_job "22/38 MapLayerTest" run_godot_marker "22/38" "res://tests_gd/MapLayerTest.tscn" MAP_LAYER_TEST_OK "$IMO/reports/gd_MapLayerTest.log"
add_job "23/38 SessionParityTest" run_godot_marker "23/38" "res://tests/SessionParityTest.tscn" SESSION_PARITY_TEST_OK "$IMO/reports/session_parity_test.log"
add_job "24/38 AudioManagerTest" run_godot_marker "24/38" "res://tests_gd/AudioManagerTest.tscn" AUDIO_MANAGER_TEST_OK "$IMO/reports/audio_manager_test.log"
add_job "25/38 ClockTest" run_godot_marker "25/38" "res://tests/ClockTest.tscn" CLOCK_TEST_OK "$IMO/reports/clock_test.log"
add_job "26/38 SleepSoakTest" run_godot_marker "26/38" "res://tests/SleepSoakTest.tscn" SLEEP_SOAK_TEST_OK "$IMO/reports/sleep_soak.log"
add_job "28/38 PictureStateTest" run_godot_marker "28/38" "res://tests_gd/PictureStateTest.tscn" PICTURE_STATE_TEST_OK "$IMO/reports/picture_state_test.log"
add_job "29/38 DayMenuTest" run_godot_marker "29/38" "res://tests_gd/DayMenuTest.tscn" DAY_MENU_TEST_OK "$IMO/reports/day_menu_test.log"
add_job "30/38 WeekendMenuTest" run_godot_marker "30/38" "res://tests_gd/WeekendMenuTest.tscn" WEEKEND_MENU_TEST_OK "$IMO/reports/weekend_menu_test.log"
add_job "33/38 HSceneTest" run_godot_marker "33/38" "res://tests_gd/HSceneTest.tscn" H_SCENE_TEST_OK "$IMO/reports/h_scene_test.log"
add_job "34/38 SignalStressTest" run_godot_marker "34/38" "res://tests_gd/SignalStressTest.tscn" SIGNAL_STRESS_TEST_OK "$IMO/reports/signal_stress_test.log"

echo "[parallel pool] running ${#JOB_CMDS[@]} functional jobs with JOBS=$JOBS"
declare -a ACTIVE_PIDS=()
failures=0

reap_finished() {
  local -a still=()
  local p
  for p in "${ACTIVE_PIDS[@]}"; do
    if kill -0 "$p" 2>/dev/null; then
      still+=("$p")
    else
      if ! wait "$p"; then
        failures=$((failures + 1))
      fi
    fi
  done
  ACTIVE_PIDS=("${still[@]+"${still[@]}"}")
}

for i in "${!JOB_CMDS[@]}"; do
  while [ "${#ACTIVE_PIDS[@]}" -ge "$JOBS" ]; do
    sleep 0.1
    reap_finished
  done
  IFS=$'\x1f' read -r -a cmd_parts <<< "${JOB_CMDS[$i]}"
  run_job_wrapper "$i" "${JOB_LABELS[$i]}" "${cmd_parts[@]}" &
  ACTIVE_PIDS+=("$!")
done

for p in "${ACTIVE_PIDS[@]+"${ACTIVE_PIDS[@]}"}"; do
  if ! wait "$p"; then
    failures=$((failures + 1))
  fi
done

echo "[timing] slowest parallel jobs (seconds):"
cat "$ROOT_TEMP"/time_* 2>/dev/null | sort -rn | head -6 | sed 's/^/  /'

if [ "$failures" -ne 0 ]; then
  echo "GODOT_GATE_FAILED ($failures parallel job(s) failed)" >&2
  exit 1
fi

# Performance probes always run serially on an uncontended machine after functional jobs finish,
# with LWsmf_SAVE_DIR isolated so they never overwrite user://system.json.
PERF_SAVE_DIR="$ROOT_TEMP/perf_save"
mkdir -p "$PERF_SAVE_DIR"
LWsmf_SAVE_DIR="$PERF_SAVE_DIR" run_godot_marker "37/38" "res://tests/PerformanceProbeTest.tscn" PERFORMANCE_CORE_OK "$IMO/reports/performance_core.log"

echo "[38/38] performance frame probe"
if [ "${LWsmf_RENDER:-0}" = "1" ]; then
  LWsmf_SAVE_DIR="$PERF_SAVE_DIR" LWsmf_PERF_STRICT=1 "$PYTHON" "$IMO/tools/run_godot_test.py" \
    --godot "$GODOT" --project "$IMO/godot" \
    --timeout 60 --quit-after 600 \
    --rendering-driver opengl3 \
    --marker PERFORMANCE_FRAME_OK \
    --log "$IMO/reports/performance_frame.log" \
    res://tests_gd/PerformanceFrameProbe.tscn
else
  LWsmf_SAVE_DIR="$PERF_SAVE_DIR" "$PYTHON" "$IMO/tools/run_godot_test.py" \
    --godot "$GODOT" --project "$IMO/godot" \
    --timeout 60 --quit-after 600 \
    --marker PERFORMANCE_FRAME_OK \
    --log "$IMO/reports/performance_frame.log" \
    res://tests_gd/PerformanceFrameProbe.tscn
fi

echo "GODOT_GATE_OK elapsed=${SECONDS}s"
