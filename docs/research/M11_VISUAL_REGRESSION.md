# M11 deterministic visual regression suite

Date: 2026-05-19
Status: implemented at HEAD 5ef6b1f (local only; not committed)
Owner: m6-training / task-92

## Goal

Turn the ad-hoc capture screenshots used during M8-M11 into a repeatable smoke
suite. One integration test renders five fixed scenes through the real capture
path and pins each frame's geometry, population counts, counters and non-black
pixel count. A rendering regression should fail the suite with the scene name and
the measured number instead of requiring a human to eyeball a screenshot.

## Harness decision

The capture pipeline lives inside the wolf-cli binary, not behind a library API:

- crates/wolf-cli/src/main.rs:30 imports Renderer, SpriteInstance, TextItem,
  LOGICAL_H and LOGICAL_W into the binary.
- build_capture_frame (crates/wolf-cli/src/main.rs:2058) and
  render_capture_frame (crates/wolf-cli/src/main.rs:2133) are private functions.
- run_capture (crates/wolf-cli/src/main.rs:2293) owns the whole Game -> CaptureFrame
  -> Renderer -> PNG path and writes the human-readable report.

The suite therefore spawns the freshly built binary for each scene, using
env!("CARGO_BIN_EXE_wolf-cli"), parses the report, then independently decodes the
PNG with the image crate and recounts the non-black pixels. It exercises the same
production path the CLI uses.

## Test file

crates/wolf-cli/tests/m11_visual_regression.rs (new, five tests)

- title_opening_frame_matches_its_baseline
- story_map_idle_frame_matches_its_baseline
- day_command_menu_frame_matches_its_baseline
- battle_frame_matches_its_baseline
- h_overlay_frame_matches_its_baseline

The tests share a process-wide Mutex because overlapping offscreen wgpu work is
not safe in this harness. Each test first checks that the external fixtures exist;
if the data root or the retail save is missing it prints a skip notice and returns
so the workspace test run stays green on machines without the game data.

Data root: work/all by default, or the WOLF_DATA_ROOT environment variable.
Retail save fixture: work/retail_backup/Save/SaveData08.sav.

## Scenes and exact commands

Run from the repository root with the debug CLI already built. The commands below
are exactly what the test issues (except that the test passes an absolute --out
path under work/out).

    ./target/debug/wolf-cli.exe --root work/all capture --frames 636 --out work/out/m11_visual_title.png

    ./target/debug/wolf-cli.exe --root work/all capture --map SampleMapA --frames 1200 --answer 0 --answer-delay 1 --out work/out/m11_visual_map_idle.png

    ./target/debug/wolf-cli.exe --root work/all capture --event 251 --frames 120 --out work/out/m11_visual_day_command.png

    ./target/debug/wolf-cli.exe --root work/all capture --event 253 --frames 1200 --answer 0 --answer-delay 1 --out work/out/m11_visual_battle.png

    ./target/debug/wolf-cli.exe --root work/all capture --load-save work/retail_backup/Save/SaveData08.sav --frames 60 --out work/out/m11_visual_h_overlay.png

Scene rationale:

- title: boot map TitleMap.mps event 2 at frame 636 is the finished title/opening
  art (91.3% non-black). Earlier frames are the logo/splash (8.4% at frame 120,
  17.8% at frame 600) and are not the title screen.
- map_idle: SampleMapA with auto-answered passage runs the opening dream sequence
  and settles in the day-1 18:30 bedroom story map (92.3% non-black, 1200 frames).
- day_command: common event 251 is the day-command menu; frame 120 shows the
  three-option menu and its background.
- battle: common event 253 at 1200 frames with answer 0 and delay 1 reaches the
  battle frame. The report proves the real chain: coro 14 event 253 pc 192 waits
  Child(232), and coro 232 event 239 pc 13 waits Frames(1).
- h_overlay: loading retail SaveData08.sav and running 60 frames captures an
  in-game H overlay frame (42 atlas images, 34.0% atlas fill).

## Committed baselines

Table 1 - geometry and population counts.

| Scene | Entry / route | Frames | Peak coroutines | Live pictures | Textured quads | Text pictures | Atlas images | Text items/glyphs/lines |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| title | boot map (TitleMap.mps event 2) | 636 | 6 | 2 | 19082 | 0 | 18 | 0/0/0 |
| map_idle | MapData/SampleMapA.mps 64x36 tiles = 1024x576, 3 layers, 40 events | 1200 | 12 | 49 | 27661 | 33 | 29 | 33/76/33 |
| day_command | common event 251 | 120 | 6 | 9 | 4 | 3 | 2 | 3/13/3 |
| battle | common event 253 | 1200 | 7 | 49 | 958 | 30 | 17 | 30/52/30 |
| h_overlay | save file work/retail_backup/Save/SaveData08.sav | 60 | 7 | 86 | 257 | 17 | 42 | 17/59/17 |

Table 2 - pixel result and engine counters. Instruction, picture-op, sound and db
counts are recorded for diagnosis but are not asserted, because a benign engine
change can move them without changing the picture.

| Scene | Non-black pixels | Ratio | Atlas fill | Instructions | Picture ops | Sounds | DB accesses | Messages | Choices | Warnings |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| title | 538282 / 589824 | 91.3% | 4.1% | 15194 | 16 | 0 | 636 | 0 | 0 | 0 |
| map_idle | 544405 / 589824 | 92.3% | 8.3% | 86261 | 4038 | 8 | 989 | 39 | 5 | 0 |
| day_command | 552254 / 589824 | 93.6% | 3.6% | 9372 | 187 | 0 | 666 | 4 | 0 | 0 |
| battle | 522838 / 589824 | 88.6% | 8.4% | 34080 | 485 | 5 | 825 | 26 | 1 | 0 |
| h_overlay | 527568 / 589824 | 89.4% | 34.0% | 1062 | 45 | 0 | 0 | 0 | 0 | 0 |

Every scene reports choice none open and zero warnings.

### h_overlay baseline update (M13 common-event self restore, anchored 2026-11-01)

The h_overlay baseline was re-pinned when the M13 common-event self-variable
persistence fix began restoring each event's CSelf bank from SaveData08's Part3 on
load. The re-pinned frame is the **faithful** one: it reproduces exactly the picture
table the save itself stored in Part5.

Evidence (SaveData08.sav Part5 records):

- pic 3208 = "hscene/pose_09.png" at (176, 464) — the re-pinned frame renders
  pose_09 at (176, 464); the old zero-self baseline rendered "DLC_data/pose_10.png"
  (wrong, the saved record is pose_09).
- pic 4100 = "hword/word_effect62.png" at (530, 315), opacity 0 — the re-pinned
  frame renders exactly this (invisible); the old baseline rendered
  "hword/word_effect102.png" at (213, 132) with opacity 0.41, i.e. an onomatopoeia
  the save does not show.

So the old baseline (peak 9, quads 258, atlas 43, messages 7, non-black 527264,
pose_10 / word_effect102 alpha 0.41) encoded the pre-fix bug where common-event self
variables were reset to zero on load. The corrected values are peak 7, quads 257,
atlas 42, messages 0, non-black 527568. Text/picture counters (86 pictures,
17 text pictures, 17/59/17 text items/glyphs/lines), frames 60, choice none and
warnings 0 are unchanged. If this baseline ever moves again, first diff the frame's
picture table against SaveData08 Part5 rather than trusting the counters.

### Fixture environment isolation

`capture_scene` in `crates/wolf-cli/tests/m11_visual_regression.rs` now calls
`command.env_remove("WOLF_GAME_DIR")` before spawning `wolf-cli`.  The baselines are
captured with the game's own fonts; when another in-process test has set
`WOLF_GAME_DIR` to a bare save directory with no TTF, the child inherits it and all
text silently disappears (`text_items` 0, a smaller non-black count), which was the
real cause of the earlier day_command / battle / map_idle mismatches — not the
self-variable fix.

Reference PNG artifacts from this machine (work/out is gitignored):

| Scene | File | Bytes | MD5 |
|---|---|---:|---|
| title | work/out/m11_visual_title.png | 521147 | 791a462d3d565d78d7b21c77eaec5736 |
| map_idle | work/out/m11_visual_map_idle.png | 399757 | 87a67c1d1e1382fbe6548daf2746c800 |
| day_command | work/out/m11_visual_day_command.png | 488291 | dbc48c329ba451826a2d7a8bd4eb835a |
| battle | work/out/m11_visual_battle.png | 243901 | 99ecfd364378ec8c99bdfa3a81dbffec |
| h_overlay | work/out/m11_visual_h_overlay.png | 448437 | 4ab78cf2e5efaa3dcc89f20321c271f0 |

The test writes the full capture report beside each image as
work/out/m11_visual_<scene>.log, which makes a failure easy to inspect without
re-running the CLI by hand.

## Assertions

For every scene the test asserts:

- the PNG decodes as exactly 1024x576 (the logical screen);
- the non-black pixel count parsed from the report equals the count obtained by
  independently decoding the written PNG;
- the non-black pixel count equals the baseline;
- frame count and peak coroutine count;
- live picture count, textured quad count and text picture count;
- choice state and option count;
- atlas image count;
- text item, glyph and line counts;
- message op and choice op counters;
- warnings are exactly zero;
- the scene-specific header (entry line for events/saves, map line for the map).

## Determinism

Each scene was captured twice by hand before the suite was written, and the five
metrics matched exactly both times. The suite run reproduced the same numbers
again. The capture path is fixed-step (frames are explicit, no wall clock), and
the atlas/font inputs do not depend on environment, so counts and non-black pixels
are stable on this checkout and this machine.

## Harness gaps and residual risk

1. The capture path lacks a public library entry point, so the test must spawn a
   subprocess. A small library API in wolf-cli or wolf-render would make the suite
   in-process and faster, but that is a production change outside this task.
2. The CLI report is human-readable text with no JSON or quiet mode. The parser
   depends on the current field labels and token order; a report formatting change
   must update parse_metrics.
3. Offscreen rendering needs a working wgpu adapter. The test skips when fixtures
   are missing, not when the GPU/adapter is missing; a headless CI box without an
   adapter will fail rather than skip. If that becomes a problem, detect the
   adapter error and skip explicitly.
4. work/all and the retail save are external assets and are absent from a clean
   checkout, so the whole suite is a skip there. This keeps cargo test green but
   means the suite only guards machines with the game data.
5. The suite pins counts and a single non-black aggregate. A localized visual
   change that preserves all counts and the aggregate could pass. A perceptual or
   exact PNG digest comparison would be stricter; the MD5 values above are a
   reference for such a future check but are not asserted here because encoder
   and driver versions can change pixels legitimately.
6. The battle baseline depends on a route (event 253 plus auto-answer). It is
   deterministic today because the capture is frame-counted and the answer is
   fixed; if the scripted battle intro changes, the baseline must move with it.

## Verification

Targeted command:

    cargo test -q -p wolf-cli --test m11_visual_regression -- --nocapture

Observed: 5 passed; 0 failed (8.47s), with every scene printing its measured
number. A plain rerun after formatting also passed 5/5 in 5.88s.

Full workspace command:

    cargo test -q --workspace --no-fail-fast -j 1

Observed at HEAD 5ef6b1f with the shared worktree dirty from other tasks: 774
tests passed and two targets failed, neither related to this suite.

- m9_full_route::the_continuous_new_game_route_reaches_day_five_0830 failed its
  exact deterministic battle expectation (observed wiring_rounds 2 and
  v350 [0,1,2,0,1,0] on one run; the baseline expects four rounds and
  [0,1,2,0,1,2,0,1,2,0,1,0]). The test file and crates/wolf-game/src/game.rs are
  changed in this shared worktree by other tasks; this suite touches neither.
- save_roundtrip::every_retail_save_survives_a_game_level_save_reload failed once
  with a missing work/out/roundtrip_SaveData06.sav, then passed 1/1 when run alone
  immediately afterwards. That is a shared-work/out race, not a save bug.

A serial run skipping only the unrelated m9_full_route case reported 775 passed
and zero failing assertions; one target (m8_battle_multi_input) could not be
launched because its .exe was locked by another process (Windows os error 32),
confirming that the remaining workspace noise is external contention.

The default parallel workspace run on this machine hit rustc STATUS_STACK_BUFFER_OVERRUN
and a pagefile-too-small error while linking many integration-test binaries at
once. Serial compilation (-j 1) is required in this shared environment. This is a
machine-resource limitation, not a code failure of the visual suite.

## Files

- crates/wolf-cli/tests/m11_visual_regression.rs - new test harness and baselines.
- docs/research/M11_VISUAL_REGRESSION.md - this document.
- work/out/m11_visual_*.png and work/out/m11_visual_*.log - gitignored artifacts.
