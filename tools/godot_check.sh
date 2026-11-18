#!/usr/bin/env bash
# M0 环境门槛 / CI 入口：编译 C# 并跑 headless 冒烟、存档导入、Oracle 对拍与
# GDScript 表现层测试。
# 用法： tools/godot_check.sh            全 headless（CI）
#        LWsmf_RENDER=1 tools/godot_check.sh  额外用真实渲染器跑 101 窗口像素校验
# 退出码 0 = 引擎+C#+场景+存档+Oracle 全部通过。
set -euo pipefail

GODOT="${GODOT:-/e/Study/tools/godot-4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe}"
IMO="$(cd "$(dirname "$0")/.." && pwd)"
DOTNET="${DOTNET:-/e/Study/tools/dotnet8/dotnet}"
export DOTNET_ROOT="${DOTNET_ROOT:-E:/Study/tools/dotnet8}"
export PATH="/e/Study/tools/dotnet8:$PATH"

if [ ! -x "$GODOT" ]; then
  echo "godot not found: $GODOT" >&2
  exit 2
fi

echo "[1/34] versions"
"$GODOT" --headless --version | head -1

echo "[preflight] converter schema audit"
if ! out_validate="$(cd "$IMO" && python -m converter validate --root work/all --out game_data --reports reports 2>&1)"; then
  echo "$out_validate"
  exit 1
fi
echo "$out_validate"
echo "$out_validate" | grep -q "DATABASE_SCHEMA_AUDIT_OK=True"

echo "[2/34] build solutions"
"$GODOT" --headless --path "$IMO/godot" --build-solutions --quit

echo "[3/34] smoke test"
out="$("$GODOT" --headless --path "$IMO/godot" res://tests/Smoke.tscn 2>&1)"
echo "$out"
echo "$out" | grep -q SMOKE_OK
echo "$out" | grep -q DATABASE_TEST_OK
echo "$out" | grep -q AUTO_EVENTS_TEST_OK
echo "$out" | grep -q BATTLE_COMMAND_TEST_OK
echo "$out" | grep -q BATTLE_WIRING_TEST_OK
echo "$out" | grep -q BATTLE_TARGET_TEST_OK
echo "$out" | grep -q BATTLE_SKILL_TARGET_TEST_OK
echo "$out" | grep -q BATTLE_MENU_BRIDGE_TEST_OK
echo "$out" | grep -q BATTLE_MATRIX_TEST_OK
echo "$out" | grep -q BATTLE_ESCAPE_TEST_OK
echo "$out" | grep -q H_EVENT_GATE_TEST_OK
echo "$out" | grep -q CONTENT_MATRIX_TEST_OK

echo "[4/34] save import test"
out_save="$("$GODOT" --headless --path "$IMO/godot" res://tests/SaveImportTest.tscn 2>&1)"
echo "$out_save"
echo "$out_save" | grep -q SAVE_IMPORT_OK
echo "$out_save" | grep -q SAVE_IMPORT_VALIDATION_OK

echo "[5/34] oracle trace diff (cli)"
python "$IMO/oracle/diff_trace.py" \
  "$IMO/oracle/fixtures/ce42_reference_trace.json" \
  "$IMO/oracle/fixtures/ce42_reference_trace.json" \
  --report "$IMO/reports/trace_diff.json"

echo "[6/34] godot trace diff test"
out_trace="$("$GODOT" --headless --path "$IMO/godot" res://tests/TraceDiffTest.tscn 2>&1)"
echo "$out_trace"
echo "$out_trace" | grep -q TRACE_DIFF_TEST_OK

echo "[7/34] title event test"
out_title="$("$GODOT" --headless --path "$IMO/godot" res://tests/TitleTest.tscn 2>&1)"
echo "$out_title"
echo "$out_title" | grep -q TITLE_TEST_OK

echo "[8/34] new game event test"
out_new="$("$GODOT" --headless --path "$IMO/godot" res://tests/NewGameTest.tscn 2>&1)"
echo "$out_new"
echo "$out_new" | grep -q NEW_GAME_TEST_OK

echo "[9/34] presentation playthrough test"
out_pres="$("$GODOT" --headless --path "$IMO/godot" res://tests/PresentationTest.tscn 2>&1)"
echo "$out_pres"
echo "$out_pres" | grep -q PRESENTATION_TEST_OK

echo "[10/34] map runtime test"
out_map="$("$GODOT" --headless --path "$IMO/godot" res://tests/MapRuntimeTest.tscn 2>&1)"
echo "$out_map"
echo "$out_map" | grep -q MAP_RUNTIME_TEST_OK

echo "[11/34] save/load slot test"
out_save="$("$GODOT" --headless --path "$IMO/godot" res://tests/SaveLoadTest.tscn 2>&1)"
echo "$out_save"
echo "$out_save" | grep -q SAVE_LOAD_TEST_OK

echo "[12/34] map message test"
out_msg="$("$GODOT" --headless --path "$IMO/godot" res://tests/MapMessageTest.tscn 2>&1)"
echo "$out_msg"
echo "$out_msg" | grep -q MAP_MESSAGE_TEST_OK

# The GDScript presentation layer (W2) resolves other scripts by class_name, so the global
# class cache has to exist before any tests_gd scene can load.
echo "[13/34] gdscript class cache"
"$GODOT" --headless --path "$IMO/godot" --import --quit

run_gd() {
  local label="$1"
  local scene="$2"
  local marker="$3"
  local log="$IMO/reports/gd_$(basename "$scene" .tscn).log"
  echo "[$label] $marker"
  local out
  out="$("$GODOT" --headless --path "$IMO/godot" "res://tests_gd/$scene" 2>&1)"
  printf '%s\n' "$out" > "$log"
  printf '%s\n' "$out"
  printf '%s\n' "$out" | grep -q "$marker"
}

run_gd "14/34" InputRouterTest.tscn INPUT_ROUTER_TEST_OK
run_gd "15/34" LayerOrderTest.tscn LAYER_ORDER_TEST_OK
run_gd "16/34" AssetDBTest.tscn ASSETDB_TEST_OK

if [ "${LWsmf_RENDER:-0}" = "1" ]; then
  # Visual pass: the dummy renderer --headless selects has no textures, so the dialog
  # test only measures pixels when a real renderer is present.
  echo "[17/34] dialog draw test (windowed)"
  out_dialog="$(LWsmf_RENDER=1 "$GODOT" --path "$IMO/godot" --rendering-driver opengl3 res://tests_gd/DialogDrawTest.tscn 2>&1)"
  printf '%s\n' "$out_dialog" | tee "$IMO/reports/gd_DialogDrawTest.log"
  printf '%s\n' "$out_dialog" | grep -q DIALOG_DRAW_TEST_OK
  shot="$(printf '%s\n' "$out_dialog" | sed -n 's/^DIALOG_DRAW_SCREENSHOT //p' | head -1)"
  if [ -n "$shot" ] && [ -f "$shot" ]; then cp "$shot" "$IMO/reports/dialog_draw.png"; fi
else
  echo "[17/34] dialog draw test (headless layout pass)"
  out_dialog="$("$GODOT" --headless --path "$IMO/godot" res://tests_gd/DialogDrawTest.tscn 2>&1)"
  printf '%s\n' "$out_dialog" | tee "$IMO/reports/gd_DialogDrawTest.log"
  printf '%s\n' "$out_dialog" | grep -q DIALOG_DRAW_TEST_OK
fi

# The Rust reference trace is a committed fixture produced by:
#   cargo run -p wolf-cli -- run --root work/all --map SampleMapA --event 0 --frames 418 \
#     --answer 0 --step-trace reports/rust_step_trace.json
# The gate only regenerates the C# side, so no Rust toolchain is required in CI.
if [ "${LWsmf_RENDER:-0}" = "1" ]; then
  echo "[18/34] picture roundtrip test (windowed)"
  # Pixel proof: the picture stack rebuilt from its stored command list must render
  # identically. The dummy renderer has no textures, so only a real renderer can diff.
  out_pic="$(LWsmf_RENDER=1 "$GODOT" --path "$IMO/godot" --rendering-driver opengl3 res://tests_gd/PictureRoundtripTest.tscn 2>&1)"
  printf '%s\n' "$out_pic" | tee "$IMO/reports/gd_PictureRoundtripTest.log"
  printf '%s\n' "$out_pic" | grep -q PICTURE_ROUNDTRIP_TEST_OK
  shot_line="$(printf '%s\n' "$out_pic" | sed -n 's/^PICTURE_ROUNDTRIP_SCREENSHOT //p' | head -1)"
  shot_before="${shot_line%%|*}"
  shot_after="${shot_line#*|}"
  if [ -n "$shot_before" ] && [ -f "$shot_before" ]; then cp "$shot_before" "$IMO/reports/picture_roundtrip.png"; fi
  if [ -n "$shot_after" ] && [ -f "$shot_after" ]; then cp "$shot_after" "$IMO/reports/picture_roundtrip_replay.png"; fi
else
  echo "[18/34] picture roundtrip test (headless bookkeeping)"
  out_pic="$("$GODOT" --headless --path "$IMO/godot" res://tests_gd/PictureRoundtripTest.tscn 2>&1)"
  printf '%s\n' "$out_pic" | tee "$IMO/reports/gd_PictureRoundtripTest.log"
  printf '%s\n' "$out_pic" | grep -q PICTURE_ROUNDTRIP_TEST_OK
fi

echo "[19/34] oracle step trace (rust fixture vs csharp)"
out_oracle="$("$GODOT" --headless --path "$IMO/godot" res://tests/OracleStepTraceTest.tscn 2>&1)"
printf '%s\n' "$out_oracle" | tee "$IMO/reports/oracle_step_trace_test.log"
printf '%s\n' "$out_oracle" | grep -q ORACLE_STEP_TRACE_OK
python "$IMO/oracle/diff_trace.py" \
  "$IMO/oracle/fixtures/samplemapa_event0_418_rust_trace.json" \
  "$IMO/reports/csharp_step_trace.json" \
  --report "$IMO/reports/step_trace_diff.json"

echo "[20/34] game root smoke test"
out_root="$("$GODOT" --headless --path "$IMO/godot" res://tests_gd/GameRootTest.tscn 2>&1)"
printf '%s\n' "$out_root" | tee "$IMO/reports/gd_GameRootTest.log"
printf '%s\n' "$out_root" | grep -q GAME_ROOT_TEST_OK

echo "[21/34] terrain test (map_tiles passability oracle)"
out_terrain="$("$GODOT" --headless --path "$IMO/godot" res://tests/TerrainTest.tscn 2>&1)"
printf '%s\n' "$out_terrain" | tee "$IMO/reports/terrain_test.log"
printf '%s\n' "$out_terrain" | grep -q TERRAIN_TEST_OK

echo "[22/34] map layer test (tiles + terrain + click)"
out_maplayer="$("$GODOT" --headless --path "$IMO/godot" res://tests_gd/MapLayerTest.tscn 2>&1)"
printf '%s\n' "$out_maplayer" | tee "$IMO/reports/gd_MapLayerTest.log"
printf '%s\n' "$out_maplayer" | grep -q MAP_LAYER_TEST_OK

echo "[23/34] session parity test (core GameSession vs frozen fixture)"
out_parity="$("$GODOT" --headless --path "$IMO/godot" res://tests/SessionParityTest.tscn 2>&1)"
printf '%s\n' "$out_parity" | tee "$IMO/reports/session_parity_test.log"
printf '%s\n' "$out_parity" | grep -q SESSION_PARITY_TEST_OK

echo "[24/34] audio manager test (BGM/BGS/SE + save snapshot)"
out_audio="$("$GODOT" --headless --path "$IMO/godot" res://tests_gd/AudioManagerTest.tscn 2>&1)"
printf '%s\n' "$out_audio" | tee "$IMO/reports/audio_manager_test.log"
printf '%s\n' "$out_audio" | grep -q AUDIO_MANAGER_TEST_OK

echo "[25/34] clock test (CE217 minute carry + formatting)"
out_clock="$("$GODOT" --headless --path "$IMO/godot" res://tests/ClockTest.tscn 2>&1)"
printf '%s\n' "$out_clock" | tee "$IMO/reports/clock_test.log"
printf '%s\n' "$out_clock" | grep -q CLOCK_TEST_OK

echo "[26/34] thirty-day sleep soak (CE329 -> CE317)"
out_soak="$("$GODOT" --headless --path "$IMO/godot" res://tests/SleepSoakTest.tscn 2>&1)"
printf '%s\n' "$out_soak" | tee "$IMO/reports/sleep_soak.log"
printf '%s\n' "$out_soak" | grep -q SLEEP_SOAK_TEST_OK

echo "[27/34] main route trace diff (rust fixture vs csharp)"
out_route="$("$GODOT" --headless --path "$IMO/godot" res://tests/RouteTraceTest.tscn 2>&1)"
printf '%s\n' "$out_route" | tee "$IMO/reports/route_trace_test.log"
printf '%s\n' "$out_route" | grep -q ROUTE_TRACE_TEST_OK
python "$IMO/oracle/diff_route.py" \
  "$IMO/oracle/fixtures/main_route_rust_trace.json.gz" \
  "$IMO/reports/csharp_route_trace.json" \
  --report "$IMO/reports/route_trace_diff.json"

echo "[28/34] picture state test (opcode 124 / CE217 HUD / pointer latch)"
out_picstate="$("$GODOT" --headless --path "$IMO/godot" res://tests_gd/PictureStateTest.tscn 2>&1)"
printf '%s\n' "$out_picstate" | tee "$IMO/reports/picture_state_test.log"
printf '%s\n' "$out_picstate" | grep -q PICTURE_STATE_TEST_OK

echo "[29/34] daytime menu test (CE251 image click / clock / energy)"
out_day="$("$GODOT" --headless --path "$IMO/godot" res://tests_gd/DayMenuTest.tscn 2>&1)"
printf '%s\n' "$out_day" | tee "$IMO/reports/day_menu_test.log"
printf '%s\n' "$out_day" | grep -q DAY_MENU_TEST_OK

echo "[30/34] weekend menu test (CE310 image click / rest / clock / energy)"
out_weekend="$("$GODOT" --headless --path "$IMO/godot" res://tests_gd/WeekendMenuTest.tscn 2>&1)"
printf '%s\n' "$out_weekend" | tee "$IMO/reports/weekend_menu_test.log"
printf '%s\n' "$out_weekend" | grep -q WEEKEND_MENU_TEST_OK

echo "[31/34] cold start route (title -> day 2 08:30)"
out_cold="$("$GODOT" --headless --path "$IMO/godot" res://tests_gd/ColdStartDayTest.tscn 2>&1)"
printf '%s\n' "$out_cold" | tee "$IMO/reports/cold_start_day.log"
printf '%s\n' "$out_cold" | grep -q COLD_START_DAY_TEST_OK

echo "[32/34] natural day line (day 4 CE253 dispatch -> day 5 08:30)"
out_battle="$("$GODOT" --headless --path "$IMO/godot" res://tests_gd/Ce253DayRouteTest.tscn 2>&1)"
printf '%s\n' "$out_battle" | tee "$IMO/reports/ce253_day_route.log"
printf '%s\n' "$out_battle" | grep -q CE253_DAY_ROUTE_TEST_OK

echo "[33/34] H scene end-to-end (CE310 A/B/C/outdoor, CE318, CE323, CE326, CE335)"
out_h="$("$GODOT" --headless --path "$IMO/godot" res://tests_gd/HSceneTest.tscn 2>&1)"
printf '%s\n' "$out_h" | tee "$IMO/reports/h_scene_test.log"
printf '%s\n' "$out_h" | grep -q H_SCENE_TEST_OK

echo "[34/34] signal stress test (ChoicesRequested/EffectRequested marshalling)"
out_signal="$("$GODOT" --headless --path "$IMO/godot" res://tests_gd/SignalStressTest.tscn 2>&1)"
printf '%s\n' "$out_signal" | tee "$IMO/reports/signal_stress_test.log"
printf '%s\n' "$out_signal" | grep -q SIGNAL_STRESS_TEST_OK

# Retail saves are gitignored test data; run the M13 mainline regression only when present.
if [ -f "$IMO/work/retail_backup/Save/SaveData01.sav" ]; then
  echo "[M13] mainline retail-save endings (SaveData01/SaveData08)"
  for slot in 1 8; do
    out_m13="$(M13_SLOT="$slot" M13_DAYS=30 "$GODOT" --headless --path "$IMO/godot" res://tests_gd/M13MainlineEndingsTest.tscn 2>&1)"
    printf '%s\n' "$out_m13" | tee "$IMO/reports/m13_mainline_endings_slot${slot}.log"
    printf '%s\n' "$out_m13" | grep -q "M13_MAINLINE_ENDINGS_TEST_OK slot=$slot"
  done
else
  echo "[M13] skipped: work/retail_backup/Save/SaveData01.sav missing"
fi

echo "GODOT_GATE_OK"
