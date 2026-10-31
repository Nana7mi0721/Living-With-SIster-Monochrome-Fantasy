# M7 W2 full — resume the retail mid-H save and drive the late-night UI

Status: green. Test file: `crates/wolf-game/tests/m7_hscene_full.rs` (2 tests, ~2.0 s,
deterministic). Data root: `work/all`. Save: `work/retail_backup/Save/SaveData08.sav`.
Baseline HEAD: 1d36ebe.

## What this proves

Two independent facts, both reached from real data + a real retail save — no H-stage or menu
variable is forced:

1. Loading `SaveData08.sav` and resuming the map's automatic pages (skipping the opening
   autorun, event 0) rebuilds the complete H picture stack the retail game draws and keeps
   running with zero warnings.
2. The late-night UI **stage index is the global variable var62**. Its click dispatcher is map
   event 11 "■深夜UI判定" page 0, gated by `var364 == 0`. The retail save was taken with
   `var364 == 1` (gate closed). Clearing that **single documented gate** lets real hover +
   left-click input walk var62 across several menu pages (1 -> 2 -> 6 -> 1 -> 4 -> 13),
   warnings stay 0, no deadlock.

Everything else (var61 selection, var62 stage, var137 act selector, the drawn poses/faces)
is produced by synthetic mouse input flowing through the real opcode-124 / opcode-123 host
reads — the same reads the retail client executes.

## How the scene is entered

The shortest faithful entry is "resume a mid-H retail save straight into the map", not a
scripted map event:

```rust
let mut game = Game::new(data_root())?;          // Game::new, crates/wolf-game/src/game.rs
game.load_save(&save)?;                           // restores vars + pictures, NOT coroutines
let map = Map::parse(&std::fs::read(data_root().join("MapData/SampleMapA.mps"))?)?;
game.start_engine_events();                       // game.rs:506
game.start_pages(&map, Some(0));                  // game.rs:566 — starts all automatic
                                                  // pages except event 0 (the opening
                                                  // dream autorun that would overwrite v137)
```

- `Map::parse` comes from `wolf_format::mps` (wolf-format is nameable from the
  integration test).
- `start_pages(&map, Some(0))` evaluates every page's conditions against the just-loaded
  vars and starts the matching parallel/autorun pages — i.e. exactly the pages the retail
  frame had live at save time.
- Skipping **event 0** matters: if the opening autorun is allowed to run it raises a dream
  choice and rewrites `var137` (8 -> 2), a *fake* stage change. Skipping it keeps the H act
  selector at its true value (var137 == 8, the h_s10 stage).

After 120 frames the scene holds: ~88-90 pictures, warnings 0, no opening choice. Recorded
state: `v62=1, v61=0, v137=8, v60=0, v138=50, v64=1, v364=1, v295=0, v63=0, v262=0`. Live
events include 1,2,13,14,15,16,17,18,19,20,23,26,27,28,29,31,35,37,39,63,334,335,337,352,
355,356,357,506 (events 9 and 11 are still dormant while `var364 == 1`).

### Layer stack asserted (test 1)

```
hscene/h_bg_001.jpg            background A
hscene/h_bg_002.png            background overlay B
hscene/h_s10_pose03.png        character pose
hscene/h_s10_face_mayu.png     eyebrows
hscene/h_s10_face_eye.png      eyes
hscene/h_s10_face_kuchi.png    mouth
hscene/h_s10_ef_syasei03.png   effect (body fluid)
hscene/h_s10_action_vibrator2.png  prop/action layer
hscene/h_frame.png             H frame
hword/word_effect98.png        h-word overlay
Picture/h_button_s01/s02/s04/s06/s07.png   H button HUD
```

`screen.len() >= 80` is required; both the H-layer files and the H-button files must be
present. The second 300-frame soak asserts `instructions` keeps increasing and warnings stay
0 — the idle H scene is alive, not deadlocked.

## How the UI advances (real input)

The dispatcher is map event 11 "■深夜UI判定", page 0. Its page condition is

```
PageCondition{variable: 2000062, operator: 1(==), value: 1}   // var62 == 1  (base menu)
PageCondition{variable: 2000168, operator: 0(?), value: 0}     // var168 == 0
PageCondition{variable: 2000364, operator: 0, value: 0}        // var364 == 0  <-- gate
PageCondition{variable: 2000481, ... disabled}
```

It then polls each button picture exactly the way the host exposes input:

```
op124 [1100000, 0x4000, <pic>, 8]     ; hover  -> var1100000  (picture_state field 8)
op111 [17, 1100000, 1, 2]             ; if var1100000 == 1     (pointer is over the button)
op124 [1100001, 0x3000, 7]            ; engine_state field 7 -> var1100001 (left button)
op111 [1, 1100001, 2, 2]              ; if fresh press this frame (state 2)
op212 <label>                         ; jump to the action handler
```

The handler writes the selection into **var61** (2000061) — codes 1,2,3,49,53,60,61 ...
— which events 9/10 (draw the next button set) and 14 "■深夜UI処理" consume to advance
**var62**.

Host read mapping (crates/wolf-game/src/game.rs):
- `key_code` 1892 / `key_code_auto` 1899.
- `picture_state` 1907: field 8 => `self.hover(number)`, field 9 => exists.
- `engine_state` 1938: field 7/8/9 => `mouse_state(bit0/1/2)`, 20/21 => mouse x/y.
- `hover` 1426: 1 if the pointer is inside the picture's axis-aligned rect (scale/window/
  texture_size/anchor aware), -1 if absent, else 0.
- `mouse_state` 1463: 0 released, 1 held, 2 pressed-this-frame.
- `previous_mouse_buttons` is refreshed at **end of frame** (game.rs:1156), so a synthetic
  click must be: set mouse -> frame (settle) -> `mouse_buttons=1` -> frame (engine sees
  fresh press = 2) -> `mouse_buttons=0` -> frames.

Opcode mapping (crates/wolf-vm/src/exec.rs): 123 stores `key::code`; 124 is SetVariableEx —
argc4 `[dst,sub,object,field]`, sub `0x3000` = engine_state(field), `0x4000/0x4400` =
picture_state(object,field).

### The var364 gate (the one injected value)

The retail save is a screenshot of the H frame with the late-night **dispatcher overlay
closed**, so `var364 == 1`. Event 11 page 0 (and event 9 page 0) require `var364 == 0`.
Test 2 sets `var364 = 0` once — the documented precondition — and forces nothing else. After
that a real hover + fresh left click on any listed H button drives var61 and event 11 rewrites
var62.

## Measured deterministic trajectory

Starting from `var62 == 1`, test 2 walks the menu the way the UI does: at each stage it
iterates the visible H-button pictures in screen order and clicks the first whose real click
advances var62. Screen iteration order is deterministic, so the trajectory is stable:

```
pic400 h_button_s01 (820,260)   var62: 1 -> 2
pic410 hbutton_m01 (895,260)    var62: 2 -> 6
pic421 hbutton_m05 (978,305)    var62: 6 -> 1
pic500 h_button_s07 (205,260)   var62: 1 -> 4
pic510 hbutton_m08 (130,260)    var62: 4 -> 13
(stop: no H button at var62 == 13 advances it)
```

New action/dialogue events spawned by these clicks: 9, 11, 21, 25, 48. Warnings are 0 at
every step. Test 2 asserts the trajectory equals
`[(400,2),(410,6),(421,1),(500,4),(510,13)]`, that it visited >= 4 distinct stage values, and
that warnings never increased.

For reference, single-click transitions measured in isolation (all warnings 0): pic400
h_button_s01(820,260)->2, pic401 h_button_s02(820,305)->3, pic403 h_button_s04(820,395)->11,
pic500 h_button_s07(205,260)->4, pic501 h_button_s06(205,305)->5; pic405 hbutton_m17(828,440)
no advance; pic406 hbutton_m35(130,350) sets var61=53 without advancing.

## Screenshot

wolf-game tests cannot render (no wolf-render dependency — only `image`). The arrival shot is
produced through the CLI capture path:

```
cargo run -q -p wolf-cli -- --root work/all capture \
  --load-save work/retail_backup/Save/SaveData08.sav --frames 90 \
  --out work/out/m7_hscene_full.png
```

Result: `work/out/m7_hscene_full.png`, 1024x576, **527251 / 589824 non-black = 89.4%**,
0 warnings, 11 running events, 2766 instructions. This matches the locked §5.1 baseline
(>= 89.4% non-black). The CLI cannot set `var364 = 0`, so it captures the arrival/rest H
frame; the interactive menu walk is exercised only inside the Rust test.

## Commands

```
# the deliverable test (default data root work/all; add WOLF_DATA_ROOT=<dir> to override)
cargo test -q -p wolf-game --test m7_hscene_full

# the reference screenshot
cargo run -q -p wolf-cli -- --root work/all capture \
  --load-save work/retail_backup/Save/SaveData08.sav --frames 90 \
  --out work/out/m7_hscene_full.png
```

## Boundaries / notes

- This is a **resume from a real mid-H save**; it is not the full forward path from the day
  loop into H (that is CE246 -> 217/333/346 territory, out of scope here). The save is a
  genuine retail snapshot, so the layer stack and stage machine are the real ones.
- No source file was modified. Only `var364 = 0` is injected (documented UI gate). Everything
  else is produced by real opcode reads + synthetic mouse input.
- Empty pointers / missing data: `open_resumed()` returns `None` with an eprintln skip if the
  save or `SampleMapA.mps` is absent, so the test is safe on roots that lack H files.
