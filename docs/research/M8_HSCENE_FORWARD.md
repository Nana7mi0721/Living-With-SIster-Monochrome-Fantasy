# M8 W2-d — H forward wrapper chain from the map H button (one continuous playthrough)

Status: **delivered** — `crates/wolf-game/tests/m8_hscene_forward.rs` (2 tests, green in ~4.6 s,
warnings 0). Data root **`work/all`**, retail save **`work/retail_backup/Save/SaveData08.sav`**.
Test-only work: no production file was changed. Not committed / not pushed.

Command (the acceptance command):

```
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_forward -- --nocapture
```

## 0. Result in one line

The real `SampleMapA` late-night H UI is driven by genuine pointer input (stage navigation
`var62 1 -> 2`, `1 -> 11`, the wipe/skin action, and the end button), and the forward
`ev33 -> CE333` chain is **pinned and proven blocked on this save**: `ev33` ("挿入抜く",
the map H button) needs `var460 == 1`, every writer of `var460 = 1` needs a player state
`SaveData08` does not carry (`v100 = 0`, `v267 = 1`, `v98 = 0`, `v111 = 0`), so the
射精 button (picture 447) is never drawn, `var61` never becomes 48, CE344/CE347 never run,
and map event 33 never activates. CE333's **call chain** is nevertheless reached from the
H UI's wipe action (map event 21 pc963/986) and its body is proven by a direct replay at
this save state. Sibling wrappers are pinned: CE246 -> CE217/CE346 is reachable, CE323 ->
CE372/CE371 is reachable (stage-selected, `var218 = 2`), CE328 -> CE402 is blocked before
the call. Everything is warning-free and frame-bounded.

## 1. Observed output (`--nocapture`; only the long CE323->CE372 pc list is elided with `...`)

```
running 2 tests
light before clock=(0, 42, 6) text=[(6200, "\\f[24]0:42"), (6210, "\\c[2]\\f[18]周日"), (6211, "\\f[18]90\\f[14]日")]
CE246 light options=["摸摸头", "揉肩", "拥抱", "亲吻", "挠痒痒", "性骚扰"]
light before clock=(0, 42, 6) text=[(6200, "\\f[24]0:42"), (6210, "\\c[2]\\f[18]周日"), (6211, "\\f[18]90\\f[14]日")]
light after clock=(0, 52, 6) text=[(6200, "\\f[24]0:52"), (6210, "\\c[2]\\f[18]周日"), (6211, "\\f[18]90\\f[14]天")]
CE246 light: clock_after=(0, 52, 6) header_bar=true npics=91 warnings=0
CE246 deep: ce346=true maxpc=180 var166=0 warnings=0
CE323/CE372: choices=[["偷窥", "什么都不做"], ["偷窥", "闯入"], ["是", "否"]] seen372=true ev372_maxpc=37 v218=2 stage_samples=[(0, 0), (500, 2), (1000, 2), (1500, 2), (2000, 2)] warnings=0
CE328/CE402: v104=0 v105=0 seen402=false seen435=true warnings=0
SIBLING DATA: CE328->CE402 pc986; CE323->CE372 pcs [120, 232, 240, 337, ...]; CE323->CE374 131 sites; CE372 var218 writes [14, 22, 28, 42, 48, 59, 82, 90, 99, 110, 119, 128, 139, 149]; CE246->CE217 11 call sites
CE333 body: instr_delta=1739 pic3208=Some("DLC_data/pose_10.png") pic4100=Some("hword/word_effect102.png") warnings=0
H FORWARD: stage_trace=[(400, 1, 2), (403, 1, 11)] wipe_ev21_maxpc=1072 ev26_seen=true ev28_maxpc=1848 ev25_seen=true ce345_seen=true ce33_seen=false ce344_seen=false ce347_seen=false pic447_seen=false v111=0 v100=70 v267=0 v98=0 v460=0 warnings=0 frames=1554
H FORWARD DATA: ev33[var460==1] -> CE333 at [60, 103, 210, 224, 231, 245, 252, 254, 258, 264, 266, 269, 275, 282, 285]; var460 writers CE344[341, 358, 440, 490, 573], CE345[49], CE347[872, 928, 980, 1001, 1057, 1505]; picture 447 -> var61=48 at ev11 pc1090; ev9 pc336 [2, 2000111, 100, 1, 2000060, 1, 2] draws 447 at pc358; ev26 pc475 [v267==0] / pc482 +8; ev28 pc124 [v100>=12] / pc159 v111=119+v111; CE345 pc44 [v98>=1]
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.25s  # ~4.6-6.3 s across runs
```

Note: the `v100=70 / v267=0` in the `H FORWARD` line are the **post-exit** values after the
end button ran CE329/CE336 (sleep teardown), not the gate values at the insertion attempts;
the gate assertions are made inside each insertion run (`v100 == 0`, `v267 == 1`).

## 2. Harness and the one documented normalisation

Both tests use the exact recipe of `m7_hscene_full.rs` / `m7_hscene_playable.rs`
(`crates/wolf-game/tests/m8_hscene_forward.rs:155` `open_resumed`):

```rust
let mut game = Game::new(data_root())?;            // work/all
game.load_save(SaveData08.sav)?;                    // vars + pictures, no coroutines
game.start_engine_events();                         // game.rs:574
game.start_pages(&map, Some(0));                    // game.rs:634 — skip opening event 0
```

The only engine-state normalisation is the one already documented by
`docs/research/M7_HSCENE_PLAYABLE.md`: the retail save was taken with the late-night UI
dispatcher gate closed, so `var364 = 0` is set once (test file line 250). Everything else is
produced by real hover + fresh-left-press input through opcode 124
(`picture_state` game.rs:2001, `engine_state` game.rs:2077, `hover` game.rs:1520,
`mouse_state` game.rs:1557), or by real `start_event`/choice replies, exactly as the
existing tests do (`start_event` game.rs:536; m7_hmode_state.rs and m7_hscene_deep.rs use
`start_event(246)`; m7_hscene_advance.rs uses `start_event(371/410)`).

No `game.vm.vars.set` is used to advance any stage, gate or event; the only variables the
test writes are `var364 = 0` (documented gate) and the self-vars that select a real common
event branch when a wrapper is entered directly from its parent call signature
(`start_event(323)` + `self1600000 = 0`, the 付き添い entry CE247 itself uses).

Click protocol (test file:185 / 199 / 216): move the pointer, settle one frame, hold
`mouse_buttons = 1` for one frame, release, then settle. `click_picture_tracked`
(test file:216) samples every frame of the click so **same-frame nested children** are
still observed at a frame boundary.

## 3. The map H button: what `ev33` is and what activates it

`SampleMapA.mps` event 33 is **"挿入抜く"** (insert/pull), a parallel page with one
activation condition: **`global var460 == 1`** (verified in the test at line 527-533;
`MapEventPage::conditions` mps.rs:461). Its 336-command page calls **CE333** at 15 pcs
(line 534-549):

```
ev33 -> 500333 at pc 60, 103, 210, 224, 231, 245, 252, 254, 258, 264, 266, 269, 275, 282, 285
```

It ends by setting `var62 = 1` (pc333) and `var460 = 0` (pc334). In other words the
"map H button" is a one-shot map animation that must be armed by `var460 = 1`.

### 3.1 Who can arm `var460`

Scanning `BasicData/CommonEvent.dat` and every map event page, the **only** writers of
`global var460 = 1` are:

| writer | pcs | how it is reached |
|---|---|---|
| CE344 射精コマンド | 341, 358, 440, 490, 573 | called only from map event 25 pc5 (test line 550-556) |
| CE345 深夜終了コマンド | 49 | called from map event 25 pc23 (end button, `var61=49`) |
| CE347 射精処理 | 872, 928, 980, 1001, 1057, 1505 | called by CE344 at 18 pcs (test line 550-556) |

Map event 25 page 0 runs only when `var61 == 48` (射精) or `var61 == 49` (end) and
`var140==0 & var60==0 & var364==0 & var123==0`. `var61 = 48` is written by map event 11
only after hovering **picture 447** (the 射精 button; ev11 pc1078 hover -> pc1090
`var61 = 48`), and picture 447 is drawn by event 9 page 0 only under **`var111 >= 100`**
(ev9 pc336 `[2, 2000111, 100, 1, 2000060, 1, 2]`; draw at pc358).

`var61 = 49` comes from the end button (picture 294, ev11 pc173), but CE345 reaches its
`var460 = 1` write (pc49) only through the pc44 guard **`var98 >= 1`** (already inserted).

## 4. Proven forward playthrough (test 1, lines 283-716)

| step | real input | observed |
|---|---|---|
| resume + gate | `start_pages(Some(0))`, `var364=0` | `var62=1`, `v111=0`, `v100=0`, `v267=1`, `v137=8`, `v98=0`, `v460=0`, warnings 0 |
| wipe / skin action | real click **picture 406** (`hbutton_m35`, 130,350) | map event 21 reaches **pc1072**; its code-53 branch calls CE343 (pc963) and CE333 (pc986); warnings 0 |
| stage nav | real click **picture 400** (`h_button_s01`) | `var62: 1 -> 2` (`stage_trace (400,1,2)`) |
| end button (from the new page) | real click **picture 294** (`endbutton.png`) | map event 25 -> CE345 observed; `var460` stays 0; CE345 ends the H UI (`var62 -> 0`); warnings 0 |
| 浅い insertion (fresh run) | real click **picture 440** (`hbutton_m30`, stage 11) | map event 26 seen; `v267=1` so its `v111 += 8` block is skipped; `v111=0`, `v460=0`, back to stage 1; warnings 0 |
| アナル insertion (fresh run) | real click **picture 418** (`hbutton_m31`, stage 11) | map event 28 reaches **pc1848** via the pc638 low-stamina abort; `v100=0 < 12`; `v111=0`, `v460=0`; warnings 0 |

Stage 11 is reached from the base page by clicking picture 403 (`h_button_s04`, code 10 ->
ev14 var62 = 11). The two insertion probes use fresh replays of the same save because the
stage-11 button layout is pose/state dependent (`v137/v144`) and is rewritten after the
first H action; the first run above remains one continuous playthrough.

### 4.1 Why the forward chain stops here (exact gates)

| gate | site | SaveData08 | consequence |
|---|---|---|---|
| `ev33` activation | `SampleMapA ev33 page0` `var460 == 1` | `v460 = 0` | ev33 never starts |
| 射精 button drawn | map ev9 p0 pc336 `v111 >= 100` | `v111 = 0` | picture 447 never drawn |
| 浅い insertion gain | map ev26 pc475 `v267 == 0`, pc482 `v111 += 8` | `v267 = 1` | gain skipped |
| アナル insertion gain | map ev28 pc124 `v100 >= 12` -> pc159 `v111 = 119 + v111` | `v100 = 0` | pc638 abort branch (`v100 >= 0` case) |
| CE345 var460 write | CE345 pc44 `var98 >= 1`, pc49 `var460 = 1` | `v98 = 0` | write skipped |

Consequently `var61` never becomes 48, map ev25 never calls CE344, CE347 never runs,
`var460` stays 0, and **`ev33` (the map H button) never activates**. CE333 can still be
*invoked* (see §5) but the insert/pull animation that `ev33` drives is unreachable from
this forward state.

### 4.2 CE333's body at this save state

The wipe call into CE333 is a same-frame nested child (map event 21 advances through the
CE333 call in one frame), so the test proves the callee by replaying it through the host
entry on the same save (test lines 391-423):

* CE333 pc0 is `[1, 2000458, 1, 2]`: `v458 == 1` selects the bath branch (pc2-7); with
  `v458 = 0` execution falls through to the bedroom body at pc8 (block end = pc7).
* `SaveData08` has `v458 = 0`; the body still executes 1,739 instructions in one frame and
  redraws **picture 3208 -> `DLC_data/pose_10.png`** and **picture 4100 ->
  `hword/word_effect102.png`**.
* `v458 = 1` is written only by CE361 (■お風呂えっち, pc162/166); the bedroom save does not
  carry it. This is recorded, not forced.

## 5. Sibling wrappers (test 2, lines 756-980)

| wrapper | entry used | observed |
|---|---|---|
| CE246 「スキンシップ」 light option | `start_event(246)` + real reply `摸摸头` | six-way menu raised; CE216 draws `hscene/header_bar.jpg`; **CE217 (時刻表示) executes** (clock `0:42 -> 0:52`; picture 6200 text `0:42 -> 0:52`, 6211 `日 -> 天`); warnings 0 |
| CE246 deep option | `start_event(246)` + `性骚扰 -> 让我看裸体 -> 让我看更多些 -> 看破！` | **CE346** live to pc180, visible `cutin/cutin_211.png` (opacity 255); CE333 **not** started and `var166 = 0` (the bad-end marker: `v23 = 9`, not 0); warnings 0 |
| CE323 「■トイレ」 | `start_event(323)` + `self1600000 = 0` (付き添い) and real replies `偷窥 -> 偷窥 -> 是` | **CE372** live (max pc 37), **CE371** live, cutin layer drawn; `var218 = 2`; warnings 0 |
| CE328 「┣就寝前パジャマ」 | `start_event(328)` + auto choice | CE435 observed; **CE402 is never reached**; warnings 0 |

### 5.1 CE217 is a same-frame HUD child

CE217 has no waits on the path CE246 selects: it draws the clock and ends inside the same
engine step, so it never appears as a live instance at a frame boundary (the `seen` set of
live instances does not contain 217). Its execution is nevertheless proven by the clock
variables and the redrawn HUD pictures (`6200`, `6210`, `6211`), and its call sites are
pinned statically: CE246 -> CE217 at **11 pcs** (`call_pcs(&data, 246, 500217)`).
This is the same methodological caveat already recorded in `M7_HSCENE_DEEP.md` §4 for
CE333/CE346: op210 children are not in `vm.started_events` and may complete within one
frame.

### 5.2 The toilet wrapper is stage-selected, not one linear 7-stage walk

CE323 calls CE372 with different `self1600001` values and each CE372 invocation sets one
`var218` stage and calls CE371. CE371's stage timer polls `123 [1600030,17]` and its
continue branch loops back to the stage entry (see `M7_HSCENE_ADVANCE.md` §2/§3). In the
deliverable the toilet run holds `var218 = 2` for 2,500 frames (`stage_samples` all 2).
The static table pins CE372's stage writes at
`[14, 22, 42, 48, 59, 82, 90, 99, 110, 119, 128, 139, 149]` (plus pc28 for `var218=6`),
so stages 3..7 are selected by later CE323 calls with different arguments; a single
CE372 call does not advance through all seven.

### 5.3 CE328 -> CE402 is blocked before the call

CE328's own entry exits at pc4/pc6 when `var105 == 0` and `var104 == 0` (SaveData08 has both
0): it calls CE435 (pc8) and hits `172 BreakEvent` at pc9, never reaching the CE402 call at
pc986. In addition CE402's first gates require `v16 >= 1`, `v22 >= 180`, **`v4 >= 21`**,
`v326 == 2`, `v102 >= 80`; the bedroom save is at midnight (`v4 = 0`), so the wrapper would
abort even if the call were reached. Both blockers are asserted statically.

## 6. Static data graph (pinned in the test)

| edge / value | data evidence (test assertions) |
|---|---|
| `ev33` activation | `var460 == 1` (ev33 page0 condition, line 527) |
| `ev33 -> CE333` | 15 op210 sites, line 534-549 |
| `var460 = 1` writers | CE344 {341,358,440,490,573}; CE345 {49}; CE347 {872,928,980,1001,1057,1505} |
| CE344 -> CE347 | 18 call sites {146,187,222,254,292,312,369,381,407,421,453,472,504,516,538,550,582,598} |
| picture 447 -> var61=48 | map ev11 pc1078 hover, pc1090 `var61=48` |
| 射精 button draw gate | map ev9 p0 pc336 `v111>=100`, draw at pc358 |
| ev26 浅い gain gate | pc475 `v267==0`, pc482 `v111 += 8` |
| ev28 アナル gain gate | pc124 `v100>=12`, pc159 `v111 = 119 + v111`; else pc638 abort |
| CE345 end-button write | pc44 `v98>=1`, pc49 `var460=1` |
| CE333 bedroom body | pc0 `v458==1` -> bath pc2; else bedroom body pc8 |
| CE328 -> CE402 | pc986; CE328 pc4/pc6 early-return gates; CE402 pc0/2/4/6/8 gates |
| CE323 -> CE372 | 42 call sites (120, 232, 240, ...) |
| CE323 -> CE374 | 131 sites, including pc194 |
| CE372 stage writes | `var218` at pcs listed above |
| CE246 -> CE217 | 11 call sites |

## 7. File / line references

### Deliverable

* `crates/wolf-game/tests/m8_hscene_forward.rs`
  * test 1 `map_h_button_forward_chain_reaches_ce333_and_pins_the_ejaculation_gate` — line 283
  * resume + documented gate (`var364=0`) — lines 286-316
  * CE333 call chain (wipe, picture 406) — lines 323-353
  * stage nav (picture 400) — lines 355-364
  * end button (picture 294) — lines 366-389
  * CE333 body replay — lines 391-423
  * 浅い / アナル insertion runs (fresh replays) — lines 426-497
  * forward blockers / static graph — lines 499-715
  * test 2 `sibling_wrappers_reach_ce246_ce217_ce346_and_pin_ce323_ce372_ce328_ce402` — line 756
  * `open_resumed` — line 155; `click_picture` — line 199; `click_picture_tracked` — line 216
  * `ce_ints` — line 90; `call_pcs` — line 102; `write_pcs` — line 121; `op111_at_map` — line 140

### Engine / data

* `crates/wolf-game/src/game.rs:536` `start_event`; `:574` `start_engine_events`;
  `:634` `start_pages`; `:1111` `frame`; `:1520` `hover`; `:1557` `mouse_state`;
  `:2001` `picture_state`; `:2077` `engine_state`.
* `crates/wolf-vm/src/exec.rs:1923` opcode 210 (CALLEV); `:2209` opcode 111 branch
  evaluation; `:1440` `compound` (`0x100` = `current + value`).
* `crates/wolf-format/src/mps.rs:461` `MapEventPage::conditions`; `:305`
  `active_page_index`; `:372` page condition decode.
* `crates/wolf-game/src/map_runtime.rs:418` `start_pending`; `:446-467` deferred
  deactivation while a page is blocked in a child call.
* `docs/MAP_AUDIT.md` ev33 row: `v460==1`, parallel, 336 commands.
* `docs/HSCENE.md` §1.2: ev33 is the H-button entry; §1.4: CE333 aborts without the
  expected globals.
* `docs/research/M7_HSCENE_PLAYABLE.md`: the `var364 = 0` gate and the five-click
  var62 walk.
* `docs/research/M7_HSCENE_DEEP.md`: CE346 is reachable from SaveData08; CE333's
  bad-end branch needs `v23 == 0` (all retail saves have `v23 >= 6`).
* `docs/research/M7_HSCENE_ADVANCE.md` §8: CE371/CE410 polls are stage-selected.

## 8. Unverified / boundaries

* The two insertion gates are proven in **fresh replays** of SaveData08, not inside the
  single continuous run, because the stage-11 button layout changes after the first H
  action (`v137/v144`-dependent page branches). The continuous run itself (resume -> wipe ->
  stage nav -> end button) is one session.
* CE333's wrapper call is proven by map event 21 reaching pc1072 (through the pc986 call)
  plus the callee body replay; a frame-boundary CE333 instance is not observable because
  it completes inside one engine step at this state.
* CE217 likewise completes inside one step; its proof is the clock/HUD side effect and the
  static call sites.
* CE333's bath branch (`v458 = 1`, CE361) and CE328's pajama/素股 branch (`v104/v105`,
  `v4 >= 21`) are not reachable from this bedroom midnight save; they are recorded as
  blockers, not driven.
* `picture 447` (射精) and `var111 >= 100` belong to the same blocked forward state; no
  attempt is made to fabricate them.

## 9. Cleanup

Only `work/out/m8_ev9.txt`, `work/out/m8_ev10.txt`, `work/out/m8_ev11.txt`,
`work/out/m8_ev14.txt`, `work/out/m8_ev18.txt`, `work/out/m8_ev21.txt` (event dumps used
while researching) were produced under `work/out/`. No other repository file was modified.