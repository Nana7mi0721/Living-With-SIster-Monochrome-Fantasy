# M7 W2 — Playable late-night H wrapper (real pointer input)

Companion to `m7_hscene_full.rs` / `docs/research/M7_HSCENE_FULL.md`. That pair proved the retail mid-H save
resumes the full H layer stack and one greedy walk moves the stage index. This file *plays* the
wrapper the way a user does and records what each real click does, then pins the terminal stage.

- Test: `crates/wolf-game/tests/m7_hscene_playable.rs` (3 tests, green in ~4.4 s).
- Screenshot: `work/out/m7_hscene_playable_arrival.png` — 1024x576, 527274/589824 non-black = **89.4 %**.
- Data root: `work/all` (H files only live in the full unpack). Save: `work/retail_backup/Save/SaveData08.sav`.

## Result in one line

From the real retail mid-H save, five real hover+left-click inputs walk the late-night stage
index `var62` **1 -> 2 -> 6 -> 1 -> 4 -> 13** with **0 warnings**; the terminal stage 13 keeps
running (instructions advance), never returns to the map, never deadlocks, and shows the full
h_s10 character/effect layer stack. `var62` has no page past 13: further clicks only toggle the
selection code `var61` / reset to stage 1. The ev33 "insert/pull" animation and the CE246 deep CG
branch are **not** reachable from this UI (blocked by the intimacy self-variable chain, see
`docs/research/M7_HMODE_STATE.md`).

## Real entry recipe (no synthetic H event)

```rust
let mut game = Game::new(data_root())?;
game.load_save(&save)?;          // restores vars + pictures, NOT coroutines
game.start_engine_events();      // the 108 background coroutines (HUD/UI)
game.start_pages(&map, Some(0)); // start SampleMapA automatic pages, SKIP the opening autorun (event 0)
```

Skipping event 0 is essential: `start_pages(&map, None)` (or `Some(u32::MAX)`) also starts the
opening dream-prologue autorun, which steals `var137` (8 -> 2) and shows the four-way dream
choice over the H scene. `Some(0)` starts every *other* automatic page (the H UI events) and
leaves the H state intact (`var137` stays 8, the h_s10 stage).

After the resume the live picture count is ~88 and, at the base page, the full stack is drawn:
`hscene/h_bg_001.jpg`, `h_bg_002.png`, `h_s10_pose03.png`, the three face layers
(`h_s10_face_mayu/eye/kuchi`), `h_s10_ef_up.png`, `h_s10_ef_syasei03.png`,
`h_s10_action_vibrator2.png`, `h_frame.png`, `hword/word_effect98.png`, plus all five
`Picture/h_button_s0*.png` H buttons.

## The one injected value: the dispatcher gate `var364 == 0`

The click dispatcher is map event 11 "■深夜UI判定" page 0, whose page conditions are
`var62 >= 1 && var168 == 0 && var364 == 0`. The retail save was taken with `var364 == 1` (gate
**closed**), so the menu is inert on arrival. Clearing `var364 = 0` is the *only* value any test
writes; `var61`, `var62` and `var137` are all produced by synthetic mouse input flowing through the
real host reads. This is the single documented hook a production H-state loader would need to
set (see recommendation below).

## How a real click advances the stage index

Two opcodes carry the whole interaction (both are ordinary host reads, see
`crates/wolf-vm/src/exec.rs`, opcode 124 -> host key/picture/engine state):

| purpose | opcode form | where |
|---|---|---|
| hover a button picture | `124 [1100000, 0x4000, <pic>, 8]` | ev11 page 0 |
| fresh left press this frame | `124 [1100001, 0x3000, 7]` | ev11 page 0 |
| branch on the result | `111 [17,1100000,1,2]` / `111 [1,1100001,2,2]` | ev11 page 0 |

- `0x4000` + field 8 = picture hover -> `Game::hover` (`crates/wolf-game/src/game.rs:1426`),
  reached through `picture_state` (`game.rs:1907`).
- `0x3000` + field 7 = left-button "pressed this frame" -> `mouse_state` (`game.rs:1463`)
  via `engine_state` (`game.rs:1938`). The previous-button snapshot is updated at the end of
  each frame (`game.rs:1156`), which is why a test must set `mouse_buttons = 1` on one frame and
  drop it the next for a clean "pressed" edge.

Flow: ev11 polls the buttons and, on a hit, writes the button code into `var61` (e.g. 1, 2, 3,
36, 37, 60 ...). Map event 14 "■深夜UI処理" page 0 (`var62 >= 1`) is the processor: it switches
on `var61` to set the next `var62`, draws the next button and resets `var61 = 0`. So
**`var62` is the menu-stage index and `var61` is the selected-button code**. Display pages:
event 9 "■深夜UI表示1" covers `var62` 1..10, event 10 "■深夜UI表示2" covers 11..16.

## Recorded play trace (deterministic)

Each row is one `click_at` (set mouse, settle 1 frame, hold `mouse_buttons=1` 1 frame, release,
+15 frames): hover+press the named picture at its (x,y) centre.

| # | picture | file | (x,y) | var62 | pictures | new coroutines |
|---|---------|------|-------|-------|----------|----------------|
| 1 | 400 | `h_button_s01.png` | (820,260) | 1 -> 2 | 85 | — |
| 2 | 410 | `hbutton_m01.png` | (895,260) | 2 -> 6 | 84 | — |
| 3 | 421 | `hbutton_m05.png` | (978,305) | 6 -> 1 | 86 | 31, 340, 350 |
| 4 | 500 | `h_button_s07.png` | (205,260) | 1 -> 4 | 88 | — |
| 5 | 510 | `hbutton_m08.png` | (130,260) | 4 -> 13 | 88 | 10 |

Distinct visited stages: {1, 2, 4, 6, 13}. Warnings stay 0 at every step. Coroutines that ran
during the whole session include the two display pages (9, 10), the dispatcher (11), the
processor (14) and the expression/effect helpers 18/19 — asserted in the first test.

## Terminal stage 13

- Stage 13 is drawn by event 10 page 2 (`var62 == 13`); its buttons are `hbutton_m08` (130,260),
  `hbutton_m09`-family and `Picture/endbutton.png` (985,540).
- Clicking every visible terminal button leaves `var62` in `1..=13` (it only toggles `var61` between
  codes 40/49/0, and a background click resets to stage 1). No click produces stage 14+.
- 300 idle frames after the walk: `var62` is unchanged, `game.stats.instructions` keeps rising
  (no deadlock), `game.stats.warnings == 0`, picture count >= 80, and the scene is still the H
  scene (`hscene/h_s10_pose03.png` + `h_frame.png` present, `var137 == 8`). The route does **not**
  exit back to the map; it simply sits at the terminal stage.
- So the wrapper is *playable* (real inputs move it) but its late-night UI has a finite 13-page
  menu; there is no in-UI path from here into the ev33 animation / CE246 deep CG.

## On CE333 / the "CG appears" check

Common event 333 is **not a standalone CG**. The opcode-210 target 500333 (CE333) is called by
UI events 11/18/19 and is the expression/face/effect helper that paints the h_s10 face + effect
layers. Being a nested common-event call it never shows up in `vm.started_events` (a scan of the
whole session finds no tracked 333). Its *drawn output*, however, is on screen and the tests
assert it: the terminal frame carries `h_s10_face_mayu/eye/kuchi`, `h_s10_ef_up`,
`h_s10_ef_syasei03` and `h_s10_action_vibrator2` — i.e. the CE333 expression/effect composite.
The ev33 "挿入抜く" animation and the CE246 deep branch that would start CE333 with a fresh CG are
gated behind the intimacy self-variable chain (var157 self 1_600_013 etc.) documented in
`docs/research/M7_HMODE_STATE.md`; they are not reachable from this UI, and this task does not
synthesize them.

## Screenshot

`work/out/m7_hscene_playable_arrival.png` (rendered headless via wolf-cli, which owns
wolf-render):

```
cargo run -q -p wolf-cli -- --root work/all capture \
  --load-save work/retail_backup/Save/SaveData08.sav --frames 90 \
  --out work/out/m7_hscene_playable_arrival.png
```

1024x576, 89.4 % non-black, 0 warnings. It shows the resumed late-night H scene with its HUD
(触摸 / 道具 / 擦干净 / 插入 / 带上套套 …), the h_s10 character with face + effect layers, and the
status bar — the visual proof the character/effect (CE333) layers composite.

Rendering limitation: `wolf-game` integration tests cannot rasterise (the crate does not depend on
`wolf-render`), so the stage walk is asserted on the picture/coroutine model, not pixels. The
CLI path that can render (`capture --load-save`) runs only the engine coroutines, and the other
CLI path (`capture --map`) runs map pages but cannot seed the `var364` gate and re-runs the
opening autorun, so it photographs the dream-prologue overlay instead of the late-night menu.
Consequently only the arrival frame is captured; the 5-step menu trace is proven by the test.

## Production recommendation (not implemented)

Expose a small H-state loader on `Game` that, from a retail mid-H save, resumes the map with
`start_pages(&map, Some(0))` and sets the single dispatcher gate `var364 = 0`. That is all a
front-end needs to make the late-night H UI interactive; the stage index / button codes then
follow from real pointer events. The deeper ev33/CE246 CG remains gated behind the intimacy
variables covered in `M7_HMODE_STATE.md` and would need those seeded too.

## Commands

```
cargo test -q -p wolf-game --test m7_hscene_playable          # 3 passed, ~4.4 s
```

Tests: `h_wrapper_plays_through_five_real_clicks_with_clean_warnings`,
`terminal_stage_13_holds_without_exiting_or_deadlocking`,
`stage_index_is_advanced_by_clicks_not_by_time`.
