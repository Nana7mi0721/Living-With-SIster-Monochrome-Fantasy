# M11-H — natural-frame CE328 → CE402 → CE410 H chain + save+event capture

Task-97. HEAD workspace after the M11 batch. Data root `work/all`, saves under
`work/retail_backup/Save/`. This document records the natural-timing proof, the exact
boundary that the older budget-1 fixture hid, the new `--load-save + --event` capture
combo, and the H-scene screenshot it produces.

## 1. What was proven

`crates/wolf-game/tests/m11_hscene_ce328_natural.rs` (2 tests, green in ~0.43 s):

```
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_hscene_ce328_natural -- --nocapture
SEQ_328_tail=[716, 717, 718, 972, 974, 977, 987, 994, 1023]
SEQ_402_tail=[503, 507, 509]
CE410_ACTS=[1, 1, 2, 1, 3] maxpc410=149
natural_chain: frames=3667 maxpc328=1023 maxpc402=509 acts=[1,1,2,1,3] cg_files=22
natural_ce410_ceiling: acts=[1,1,2,1,3] max_act=3 maxpc410=149 deep_assets=[]
test result: ok. 2 passed; 0 failed
```

The whole chain CE328 → CE402 → CE410 runs on **natural frame timing**: the default
`DEFAULT_BUDGET = 200_000` instructions per `Game::frame` (`crates/wolf-vm/src/exec.rs:54`),
ordinary `Game::frame` steps, **no `Vm::with_budget(1)`**. Reached route (pinned by
`SEQ_328_tail`): `693 合宿処理 → 694 (var304<1) → 715..721 就寝 → 745 通常就寝 → 746
(var18<50) → 845 (var494!=2) → 850 → 969..985 → pc986 op210[500402] → CE402 (tail pc509)
→ CE410 ×5 acts [1,1,2,1,3]`. 22 distinct H CG files were live (cutin_183/184/187,
h_bg_001/011, h_frame, h_s08_* …), warnings = 0, frames = 3667, var64 stays 0.

(`pc986` itself is never observed by frame-boundary sampling — the call and its whole
CE402 child execute inside a single natural frame; the tail `pc1023` with CE402 live is
the sampling-independent proof the call happened. Assertions therefore check route tail /
act list / CG file set / warnings, never a sampled max-pc equality.)

## 2. Root cause: AutoEvents interleaving, not the frame budget

The old fixture `m11_hscene_ce328_real_cg.rs` reached the deep chain only with
`Vm::with_budget(programs, 1)`. That is an **interleaving artefact**, not retail behaviour:

* `Game::frame` begins with `self.auto_events.sync(&mut self.vm)`
  (`crates/wolf-game/src/game.rs:1378`), where `AutoEvents` (`crates/wolf-game/src/auto_events.rs`)
  starts every `run_condition == 2` common event whose header variable condition is true.
  With SaveData01 + the seeded vars this starts ~9 unrelated background coroutines
  (max concurrent instances measured 9).
* At a 1-instruction budget those coroutines **starve** (CE328 monopolises the budget each
  frame) and cannot mutate shared vars. At any realistic budget (>=5) they run interleaved
  and divert CE402 down its early `終了` branch — CE402 is never called, `maxpc402 = 0`.
* Disabling the scheduler explicitly (`g.auto_events.set_enabled(false)`) and keeping the
  **natural 200_000 budget** reproduces the deep chain. So the single variable that matters
  versus the budget-1 fixture is the presence of the background scheduler, not the budget.

The test's `open_natural` therefore: `Game::new` → `load_save(SaveData01)` → seed
`seed_deep(var23=6)` → rebuild a fresh `Vm::new(programs)` preserving `vars` (same programs,
budget stays 200_000) → `auto_events.set_enabled(false)` → `auto_advance = true` →
`auto_choice = false`, `auto_choice_delay = 2`, cursor 0 → `start_event(328)`. Choice
prompts are answered manually with `ChoiceReply::Index(0)` whenever `game.choices` is open.

`seed_deep(var23)` (calendar-consistent fixture for the 21:00 Thursday 就寝 route):
`(304,0),(6,4),(4,21),(5,0),(18,20),(16,1),(22,180),(326,2),(102,80),(103,2),(23,var23),
(151,0),(20,61),(265,0),(160,90),(545,1),(494,0)` — var18<50 (pc746 else), var160=90
(pc850 false → the 969..997 body holding pc986), var23=6 (CE402 pc52 gate), var494=0.

## 3. The exact residual boundary (CE410 act > 3 / pc > 200)

Under natural timing CE402 **exits at pc208** (BGM fade, the おやすみ ending after act 3) and
jumps straight to its tail (`SEQ_402_tail=[503, 507, 509]`). It never reaches the act-4 /
act-5 CE410 call sites:

| CE410 caller in CE402 | act |
|---|---|
| pc165, pc173, pc195, pc308 | 1 |
| pc177 | 2 |
| pc198, pc311 | 3 |
| pc288, pc360 | 4 |
| pc447 | 5 |

CE410 routes on its self var `SELF_ARG_BASE` (1600000): block head=16 cases (1→pc18 素股,
2→pc60 素股絶頂, 3→pc111 素股射精) and block head=155 cases (1→pc157 騎乗位絶頂,
2→pc201 スパイダー騎乗位絶頂, 3→pc245 act 4/5/6). The **pc>200 bodies (pc157..172, pc201..247)
therefore belong only to act 4 / act 5**, which natural timing never reaches.

The budget≤5 diagnostic run instead starves the scheduler and lets CE402 fall through to the
act-5 site (pc447) → CE410 act 5 → pc247, drawing `v144_cg20`, `cutin_189`, `cutin_190`.
That is why the old fixture's "CE410 pc247" assertion existed — it is an artefact of the
starved-budget interleaving, not a reachable retail state from SaveData01 + seed_deep.

The second test, `natural_timing_ce410_act_ceiling_is_three`, pins this honestly:
`max_act <= 3`, acts {1,2,3} present, no `v144_cg20`/`cutin_189`/`cutin_190` asset, sampled
`ce410_maxpc < 200`. So the residual is documented as a ceiling, not left as a silent pass
or a red assertion.

## 4. CLI: `--load-save` + `--event` now combine (`CaptureEntry::SaveWithEvent`)

Before this task `resolve_capture_entry` dropped `--event` whenever `--load-save` was given
(load-save outranked everything), so a capture could not start a common event on top of a
restored save. Changes in `crates/wolf-cli/src/main.rs` (all inside existing structures; the
exhaustive `Entry` / `play.rs` match is untouched):

* New `CaptureEntry::SaveWithEvent { path: PathBuf, event: u32 }` variant.
* `resolve_capture_entry`: `--load-save` + `--event` → `SaveWithEvent`; `--load-save` alone →
  plain `Save` (unchanged precedence over `--map-run`/`--menu-only`/`--boot`).
* Both `CaptureEntry → Entry` call sites: `SaveWithEvent → Entry::Save(path)`; the capture
  site stashes the id in a local `entry_event` and passes it as a new last arg to
  `run_capture`; the play site drops the id (play drives the save interactively).
* `run_capture(..., entry_event: Option<u32>)`: in the `Entry::Save` arm, after
  `game.load_save(path)` succeeds, if `entry_event` is `Some(id)` it calls
  `game.start_engine_events()` then `game.start_event(id)`, records the new instance for
  `--self` seeding, and labels the run `save file <path> + common event <id>`.
* Unit tests updated: `load_save_outranks_everything` now expects
  `SaveWithEvent{path, event:239}`; new `load_save_alone_is_plain_save` expects `Save`.

`cargo test -q -p wolf-cli` → 9 + 2 + 4 + 5 tests pass.

## 5. H-scene screenshot via the new combo

Because `--event` now survives `--load-save`, the natural route can be captured directly:

```
WOLF_DATA_ROOT=work/all ./target/debug/wolf-cli.exe capture \
  --load-save work/retail_backup/Save/SaveData01.sav \
  --event 328 --auto-events off --auto-input --answer 0 --answer-delay 1 \
  --frames 700 \
  --var 304=0 --var 6=4 --var 4=21 --var 5=0 --var 18=20 --var 16=1 --var 22=180 \
  --var 326=2 --var 102=80 --var 103=2 --var 23=6 --var 151=0 --var 20=61 \
  --var 265=0 --var 160=90 --var 545=1 --var 494=0 \
  --out work/out/m11_hscene_natural.png
```

`--auto-events off` is the CLI equivalent of `auto_events.set_enabled(false)`;
`--auto-input` injects key 10 (OK) every frame (drives the H polls); `--answer 0
--answer-delay 1` answers prompts with index 0. Output:

```
pictures  : 54 live -> 16 textured quad(s), 22 text picture(s)
rendered  : 1024x576, 538666/589824 non-black pixel(s) = 91.3%
stats     : 427 picture op(s), 9 sound(s), 31 message(s), 4 choice(s), 0 warning(s)
asset set : h_bg_001 h_bg_011 h_frame h_s08_chinko h_s08_ef_wet
            h_s08_face_eye01 h_s08_face_kuchi01 h_s08_face_mayu01 h_s08_pose04 cutin_183
```

`work/out/m11_hscene_natural.png` — 1024×576, 91.3% non-black, 54 live pictures, 0 warnings:
a real H frame (HUD, standing pose, h_bg, in-window cutin_183).

A frame sweep (100..3600) shows the H layers are live from ~frame 600 to ~1300
(h_bg_001/011 + h_frame + h_s08_* + cutin_183 at 600; cutin_187 + h_s08_ef_ecstasy at
1000; cutin_183 + h_s08_ef_omorasi at 1200). From ~1400 on the run returns to the daytime
home HUD. Frame 700 was chosen for the deliverable because it is mid-act with the cutin
window open and the most live H layers.

## 6. Files

* `crates/wolf-game/tests/m11_hscene_ce328_natural.rs` — the two natural-timing tests.
* `crates/wolf-cli/src/main.rs` — `CaptureEntry::SaveWithEvent` + `run_capture` event start.
* `docs/research/M11_HSCENE_NATURAL.md` — this document.
* `work/out/m11_hscene_natural.png` (+ `work/out/m11_hscene_natural.log`) — the H screenshot.

Not committed: the Lead integrates. Permanent scratch files
(`crates/wolf-format/examples/opscan.rs`, `crates/wolf-format/tests/_scratch_cdata.rs`,
`crates/wolf-format/tests/_scratch_event.rs`) untouched.
