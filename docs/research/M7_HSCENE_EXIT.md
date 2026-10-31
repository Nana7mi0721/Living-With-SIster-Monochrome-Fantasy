# M7 W2 — H scene termination: the real exit from terminal stage 13

Companion to `crates/wolf-game/tests/m7_hscene_exit.rs` (3 tests, green in ~1.0–2.2 s).
This file closes the last open H-scene boundary in `docs/HANDOFF_M7.md`: the note that
"terminal stage 13 does not exit to the map, clicks only toggle `var61` / reset stage 1",
i.e. that the late-night H UI might be **terminal-by-design**. It is not. The H scene has a
real, explicit exit and it fires from stage 13 with **0 warnings**. The earlier reading was an
**input-harness artifact**, not a game property.

- Test: `crates/wolf-game/tests/m7_hscene_exit.rs` — `end_button_exits_h_scene_from_terminal_stage_13`,
  `real_navigation_to_stage_13_then_end_button_exits`, `end_button_exit_is_gated_on_the_message_window`.
- Data root: `work/all` (H files only live in the full unpack). Save: `work/retail_backup/Save/SaveData08.sav`.
- Build/run: `WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_hscene_exit -- --nocapture`.

## Result in one line

The H scene is **not** terminal-by-design. Every late-night UI page draws an on-screen end
button (`Picture/endbutton.png`, picture **294**). Pressing it makes the dispatcher event 11
write `var61 = 49`; map event **25** `深夜終了処理` page 0 turns that into a call to common
event **CE345** `深夜終了コマンド`, whose teardown resets `var62 = 0`, `var63 = 0`, `var64 = 0`
(clears the pyjama flag), fades to black, saves through `CE222` and returns the clock through
`CE217`. The dispatch is gated on **system variable 12 (9000012) == 0** — the message-window
visible flag — so the button must be pressed with the message window idle, and `CE345`'s own
"要结束么？" dialogue must be advanced for the teardown to reach its reset pcs.

## The full exit chain (SampleMapA + CommonEvent.dat, work/all)

| step | locus | instruction | effect |
|---|---|---|---|
| 1 | event 9 pages 1..10 (stages 1..10) / event 10 pages 1..6 (stages 11..16) | `op150` picture **294** `Picture/endbutton.png` | the end button is on every late-night UI page |
| 2 | event 11 `■深夜UI判定` page 0 | `op124` hover 294 -> `op121 [2000061, 49, 0, 0]` | end button writes `var61 = 49` |
| 3 | event 25 `深夜終了処理` page 0 (conds `var140==0 & var60==0 & var364==0 & var123==0`) | `pc9 op111 [1,9000012,0,2]`; `pc11 op111 [1,2000061,49,2]`; `pc15 op111 [1,2000060,2,1]` | gate: message window closed + `var61==49` + `var60<2` |
| 4 | event 25 page 0 `pc21 op121 [2000062,1,0,0]`, `pc23 op210 [345,...]` | stage back to 1, then call **CE345** |
| 5 | CE345 `深夜終了コマンド` (380 slots) | `pc38` msg "\E要结束么？" -> `pc39 op102 [2]` choice [是,算了] -> big dialogue/choices | scripted end sequence (needs advancing) |
| 6 | CE345 teardown | `pc339 var62=0`, `pc340 var63=0`, `pc341 var64=0`, `pc342 var106=0`, `pc343 var241=0`, `pc344` blackscreen, `pc346` pic290 fade, `pc347 op210 CE336`, `pc348 op210 CE222[2,13,0]` (save), `pc353 op210 CE217` (clock) | H state cleared, scene ends by fade-to-black + save |

Static writer audit (all `CommonEvent.dat` events + `SampleMapA.mps`): `var62 = 0` is written only by
CE320 pc4, CE332 pc8, CE340 pc3927, CE341 pc2039, CE345 pc339, EV24 pc25/44, and it is exactly the
"left the H UI menu" value the sleep gate (`EV4` page 4, `var63==1 & var62==0 & var464==0 & var60==0`)
waits for. There is **no** `ReturnToTitle`/`map-change`/`teleport` inside the act-stage events
9/10/11/14/18/19/21 — the only in-map way out of the H UI is this end-button -> event 25 -> CE345 path.
(`EV4` page 1 pc25 `op174` ReturnToTitle needs `var298==1`; `EV24` pages 4/5/6 ReturnToTitle needs
`var60>=4`; both are sleep/bad-end states, not the act stages.)

## System variable 12 (9000012) is the message-window flag

`crates/wolf-vm/src/vars.rs` puts the system banks at magic `9_000_000 + n`, so `9000012` is
`vars.get_sys(12)`. It is maintained by the message engine common event **CE63** ("表示処理"):
`pc70 op111 [1,9000012,1,2]` labels the `==1` case "メッセージウィンドウ表示", and
`pc179/194/206 op111 [1,9000012,0,2]` label `==0` "表示されてない場合/完全に消えたら表示フラグをオフ".
So `9000012 == 1` means *a text message window is open*. Event 25 page 0 pc9 refuses to run CE345
while that is 1. The only readers of 9000012 in the whole dataset are CE63 (this manager) and
CE270 pc86 / CE271 pc89 (menu checks) — nothing writes it from map-event data, so it is purely
engine/message-window state.

## Why the earlier M7 playthrough saw "no exit" (the artifact)

`m7_hscene_playable.rs` did two things that hid the exit:

1. It never enabled `auto_advance` on the terminal stage, so the retail save resumed with a
   message window open (`get_sys(12) == 1`). With `9000012 != 0`, event 25 page 0 pc9 skips the
   `var61==49` branch entirely — the end press reaches event 11 (`var61 = 49`) but never reaches
   CE345, so `var62` stays 13. This test reproduces that exactly
   (`end_button_exit_is_gated_on_the_message_window`).
2. It never advanced `CE345`'s scripted dialogue, so even if the dispatch fired, `CE345` would
   block on its first message ("要结束么？") long before the `pc339 var62=0` teardown.

Settling the message window (auto-advance, or pressing OK a few times) drops `get_sys(12)` to 0 and
the same end button then exits cleanly. On `SaveData08` the real forward navigation is
`1 -> 2 -> 6 -> 1 -> 4 -> 13` (`h_button_s01` etc.), unchanged from `m7_hscene_playable.rs`.

## What the tests pin

1. `end_button_exits_h_scene_from_terminal_stage_13` — seed the minimal terminal precondition
   (`var364=0`, `var62=13`, `auto_advance`), confirm the message window is idle
   (`get_sys(12)==0`) and the H pose layer (`hscene/h_s10_pose`) is on screen, press the end
   button, then assert CE345 + CE222 were entered, `var62 13 -> 0`, `var64 1 -> 0`, the
   `h_s10_pose` layer is gone and `stats.warnings == 0`.
2. `real_navigation_to_stage_13_then_end_button_exits` — same, but reach stage 13 by **real
   hover+click pointer input** (auto-advance on so the message window is idle), then press the
   end button and assert the same `var62 13 -> 0`, `var64 -> 0`, CE345 seen, warnings 0.
3. `end_button_exit_is_gated_on_the_message_window` — reach stage 13 with **no** auto-advance
   (`get_sys(12)==1`), press the end button, and assert `var61 == 49` was written (event 11
   recorded the press) but CE345 did **not** run and `var62` stays 13 — pinning the exact
   `9000012 == 0` gate behind the earlier artifact. Warnings 0.

## Verdict

- **Explicit exit exists.** The H scene is not terminal-by-design: the on-screen end button
  (picture 294) -> event 11 `var61=49` -> event 25 page 0 -> CE345 tears the H UI down and ends
  the scene. Reachable from stage 13; warning-free.
- **Not a missing data gate.** The only precondition the earlier run lacked is *engine state*:
  the message window must be closed (`system var 12 == 0`) and `CE345`'s dialogue advanced.
  No production value is missing — the 9000012 flag is owned by the message engine, not by any
  map/event script.
- **Normalisation used by the tests:** the sole injected engine value is the already-documented
  dispatcher gate `var364 = 0`; test 1 additionally seeds `var62 = 13` to isolate the exit
  mechanism from navigation timing (test 2 exercises the real navigation).
