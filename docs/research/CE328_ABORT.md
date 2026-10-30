# CE328 map-context abort — root cause (M6 night flow)

Status: investigation complete (task-5, teammate m6-night2). Lead applied the fix in the same session.

**Implementation note (Lead)**: `crates/wolf-game/src/map_runtime.rs:328-348` now defers only the *deactivation* of a page whose coroutine is blocked in `WaitState::Child`, i.e. `want.is_none()` plus the live instance's wait check. The initially proposed unconditional deferral was too broad: it deadlocked the day-3 dinner (a page-to-page switch that legitimately has to happen while a child is alive). Regression: `crates/wolf-game/tests/m6_night_sleep.rs::the_natural_night_menu_sleep_rolls_over_to_day_three` now drives hover-sister → 今天先休息吧 → 是 → day3 08:30 with only `var295=0` normalization, and all M6 tests stay green.

## Verdict (short)

The reported symptom "CE328 aborts before pc700, var64 stays 0, var69 cleared, returns to day2 19:30" is a mis-attribution.

- **CE328 does not abort at all.** In both the standalone chain and the live map chain, CE328 runs past pc700 and is parked at **pc994**, i.e. blocked inside `210 CALL 500329` at pc993 (the CE329 "就寝処理" call). The `var64 = 1` branches (pc700/pc727, reached only when `v304 >= 1`; and the hour branches gated by `v18 >= 50`) are **never taken**, because in the map run `v304 = 0` and `v18 = 22`. `var64 = 0` is therefore correct for the route both contexts actually take — it is not evidence of an abort.
- **The real failure is inside CE329.** CE329 (called from CE328 pc993) parks at a `180 [60]` wait at pc35. On the frame that wait expires it executes pc36..pc60. That block **writes `var69 = 0`** at pc41. `var69 == 1` is part of the activation condition of the map event that *owns the whole call stack* (map event 4 "メニュー処理", page 0, trigger = Autorun). The moment `var69` flips to 0, the engine's once-per-frame page re-selection replaces that event's page and calls `Vm::stop_instance` on it; `stop_instance` kills the instance **and every descendant**, which is CE245 → CE327 → CE328 → CE329. CE329 is destroyed at pc≈41, long before its day-rollover code at pc385 (`var4 = 7`), pc386 (`var5 = 45`), pc406 (`var160 = 1`) and pc436/440 (`CALL CE317` morning setup). Day stays `v160 = 2` / 19:30, `var69 = 0`.

So the **precise missing trigger is the page-activation write `var69 = 0` at CE329 pc41**, which invalidates its own ancestor page condition `v69 == 1` while that ancestor is blocked inside the `210` call chain.

## The call stack (all coroutines blocked at the same time)

`210 CALL` is a *blocking* call: the caller's `WaitState` becomes `WaitState::Child(child)` (`crates/wolf-vm/src/exec.rs:1209`; variant `Child(InstanceId)` at `exec.rs:98-99`). The night menu therefore exists as a suspended descendant stack. At the abort frame the stack was:

```
map event 4 "メニュー処理" page 0   pc=336   WaitState::Child(CE245)   [AUTORUN]
   └ CE245 "■夜コマンドメニュー"   pc=821   WaitState::Child(CE327)   (210 CALL 500327 at pc820, then 172 BreakEvent at pc821)
       └ CE327 "■休息"             pc=30    WaitState::Child(CE328)   (210 CALL 500328 at pc29)
           └ CE328 "┣就寝前パジャマ" pc=994   WaitState::Child(CE329)   (210 CALL 500329 at pc993 → pc994)
               └ CE329 "┗就寝処理"   pc=36    WaitState::Frames(60)     (180 [60] at pc35; pc36 = next op)
```

Map event 4 reaches CE245 through its 45th `210` (call #44, target `500245`) inside page 0; that is the `pc 336` suspension point seen in the trace. (The task hint "ev4 page0 pc340 CE325" does not match the data: the page-0 call table has `500245` at call #44 and the live instance is parked on that call, not on CE325.)

## CE328 command stream around the decision (why var64 is never set)

From `work/_ce_full.json` (CE dump, 550 records; record id = common-event id):

| pc | op | args | meaning |
|----|----|------|---------|
| 693 | 212 | label "就寝処理" | SetLabel |
| 694 | 111 | `[17,2000304,1,1]` | **IF v304 >= 1** |
| 695 | 401 | `[1]` | branch select (then) |
| 700 | 121 | `[2000064,1,0,0]` | **SET var64 = 1** (only on the v304>=1 branch) |
| 712 | 172 | — | BreakEvent |
| 714 | 420 | `[0]` | ELSE |
| 721 | 213 | label "就寝" | SetLabel |
| 724 | 111 | `[1,2000304,1,1]` | IF v304 == 1 |
| 727 | 121 | `[2000064,1,0,0]` | SET var64 = 1 (alternate v304==1 entry) |
| 746 | 111 | `[17,2000018,50,1]` | **IF v18 >= 50** |
| 755 | 111 | `[19,2000004,18,2,2000004,19,2,2000004,20,2]` | IF v4 in {18,19,20} |
| 817 | 121 | `[2000161,0,0,0]` | SET var161 = 0 |
| 825 | 121 | `[2000064,1,0,0]` | SET var64 = 1 (hour-branch path) |
| 827 | 111 | `[1,2000160,83,2]` | IF v160 == 83 |
| 836 | 111 | `[1,2000494,2,2]` | IF v494 == 2 |
| 842 | 210 | `[500331,0]` | CALL CE331 |
| 966 | 210 | `[500331,0]` | CALL CE331 |
| 986 | 210 | `[500402,0]` | CALL CE402 (睡眠測定) |
| 987 | 121 | `[1600020,1,10,24576]` | SET self slot 20 = 1 |
| 988 | 111 | `[17,1600020,1,2]` | IF self20 == 1 |
| 990 | 210 | `[500329,1,1]` | CALL CE329 [1,1] (then path) |
| 993 | 210 | `[500329,1,0]` | CALL CE329 [1,0] (else path) |

In the live map run (and identically in the standalone run) the pre-state is `v304 = 0`, `v18 = 22`, `v4 = 19`, `v5 = 30`, `v160 = 2`. Therefore pc694 is false, pc746 is false (22 < 50), and none of the `var64 = 1` writes execute. Execution falls through the "就寝" label path to pc986 → pc987 → pc988 → pc993 `CALL 500329`; CE328 parks at **pc994** (the return slot of the pc993 call), inside CE329. Both the map run and the fresh-standalone run reach exactly pc994 with the same set of wait-point pcs for CE328 (`{91,96,100,101,972,974,977,994}`). **CE328 is not the divergence.**

## CE329 "就寝処理" command stream (the kill zone)

From `work/_ce_full.json` record 329 (444 cmds):

| pc | op | args | meaning |
|----|----|------|---------|
| 33 | 121 | `[2000061,0,0,0]` | SET var61 = 0 |
| 34 | 103 | — | message "お布団は…リセットされます" |
| 35 | 180 | `[60]` | **WAIT 60 frames** (WaitState::Frames(60)) |
| 36 | 140 | `[-240,20,0,0]` | sound fade out |
| 37-60 | 121 | — | zero a block of state flags |
| 41 | 121 | `[2000069,0,0,65536,4]` | **SET var69 = 0**  ← the trigger |
| 43 | 121 | `[2000100,70,0,0]` | SET var100 = 70 |
| 58 | 121 | `[2000297,1,0,512]` | SET var297 = 1 |
| 385 | 121 | `[2000004,7,...]` | **SET var4 = 7** (hour 07) |
| 386 | 121 | `[2000005,45,...]` | **SET var5 = 45** |
| 406 | 121 | `[2000160,1,...]` | **SET var160 = 1** (day advance) |
| 412/423/427 | 210 | `[500226,...]` | CALL 500226 |
| 436 / 440 | 210 | `[500317,1,1]` / `[500317,1,0]` | CALL CE317 (morning setup) |

The whole day rollover lives at pc385+. CE329 never gets there in map context; it dies at pc≈41.

## Map event 4 page/condition data (SampleMapA.mps, the live map)

The live map is `work/all/MapData/SampleMapA.mps` (the boot/entry trick loads SampleMapA). Its event 4 is **"メニュー処理" at (30,3), 5 pages**:

| page | trigger | cmds | activation condition | contains CALL 500245? |
|------|---------|------|----------------------|-----------------------|
| p0 | **Autorun (1)** | 342 | **`v68 == 1 & v69 == 1 & v62 == 0`** | yes — call #44 (`210 [500245,...]`) |
| p1 | Autorun (1) | 28 | `v298 == 1` | no |
| p2 | Parallel (2) | 66 | `v67 >= 1 & v302 == 0 & v464 == 0` | no |
| p3 | Parallel (2) | 66 | `v57 >= 1 & v302 == 0 & v464 == 0` | no |
| p4 | Parallel (2) | 116 | `v63 == 1 & v62 == 0 & v464 == 0 & v60 == 0` | no |

Five events in this map have a page gated by `v69 == 1` — exactly the five pages that switch when `var69` is cleared:

| event | name | page | trigger | condition |
|-------|------|------|---------|-----------|
| 1 | 妹クリック判定（夜 | p0 | Parallel | `v69==1 & v25==0 & v26==0 & v302==0` |
| **4** | **メニュー処理** | p0 | **Autorun** | **`v68==1 & v69==1 & v62==0`** |
| 29 | パンツアイコン表示 | p0 | Parallel | `v439==0 & v69==1 & v295==0 & v627==0` |
| 34 | スピーカー表示 | p0 | Parallel | `v69==1 & v62==0` |
| 35 | 音量調整 | p0 | Parallel | `v69==1 & v464==0` |

Event 4 page 0 is the parent of the CE245/CE327/CE328/CE329 chain, and its activation condition contains `v69 == 1`.

## Runtime trace (frames f160..f164, after choosing 是 on the rest menu)

Captured by a temporary example (`crates/wolf-game/examples/tmp_ce328.rs`, since deleted) that drives the real game:
`WOLF_DATA_ROOT=work/all cargo run -q -p wolf-game --example tmp_ce328`.
Full output saved at `work/out/ce328_abort_trace.txt`. Each frame records every live instance and each var.

**f163** (one frame before the kill): the full chain is alive and `var69 = 1`:

```
f163 inst=[#12[63:message window]@67  #17803[4:メニュー処理]@336
           #17862[245:■夜コマンドメニュー]@821<17803
           #18099[327:■休息]@30<17862
           #18100[328:┣就寝前パジャマ]@994<18099
           #18910[329:┗就寝処理]@36<18100  ...parallel pages...]
     v4=19 v5=30 v160=2 v69=1 v64=0 v18=22 v304=0 v494=0 v295=0 | 329present=true
```

**f164** (same frame the 60-frame wait expires): `var69` has just been cleared and the chain is gone:

```
f164 inst=[#12[63:message window]@67  #19302[13:...]@0  #19303[15:...]@0  #19304[25:...]@0]
     v4=19 v5=30 v160=2 v69=0 v64=0 v18=22 v304=0 v494=0 v295=0 | 329present=false
     REMOVED [(4,336),(245,821),(327,30),(328,994),(329,36),(1,0),(29,17),(13,0),(15,0),(25,0),(34,0),(35,0)]
     ADDED   [(13,0),(15,0),(25,0)]
     runtime.switches 11 -> 16        <-- exactly +5 page switches = the 5 v69-gated events
     runtime.restarts 18625 -> 18628
f164 CHAIN GONE.
POST day=2 hour=19 min=30 v69=0 v160=2      (day did NOT advance)
```

The +5 `runtime.switches` is decisive: MapRuntime tracks how many times it swapped a page; the only five events whose activation depends on `v69 == 1` are events 4, 1, 29, 34, 35 — an exact match. Collateral effects seen at f164: the same `var69` write is the first term in the "is the player still in bed?" gate of the other parallel events, so they too stopped.

## Why the standalone chain rolls the day over but the map chain does not

- Standalone (`start_event(245)` directly, fresh `Game`, `v4=19,v5=30,v160=2,v6=1,v69=1`): CE245 is a bare coroutine with **no owning map-event page**. `MapRuntime::start_pending` has no slot/instance to re-select or stop for it, so nothing can pre-empt the chain. CE329's pc41 `var69 = 0` has no page-condition feedback loop to react to. CE329 proceeds through pc36..pc60, then 385 (`var4=7`), 386 (`var5=45`), 406 (`var160=1`), 436/440 (`CALL CE317`) and the day becomes day3 08:30.
- Map context: CE245 is a descendant of map event 4 page 0 (Autorun, gated on `v69==1`). The same CE329 `var69 = 0` now flips the *ancestor's* page condition to false. Page re-selection stops the ancestor and the descendant cascade removes the entire coroutine stack before CE329 reaches pc385.

## Root cause (engine-level)

The Rust port re-selects every map event's page once per frame and tears down the running page's coroutine whenever the selected page index changes — unconditionally, even when that coroutine is midway through a `210 CALL` and is merely *waiting on a child*.

- `crates/wolf-game/src/game.rs:1136-1138` — `runtime.start_pending(&mut self.vm)` is called **at the end of every frame** ("Automatic pages are restarted here, i.e. at the end of a frame"), after `vm.step_frame` has already run CE329's zeroing block.
- `crates/wolf-game/src/map_runtime.rs:301-376` — `start_pending`.
  - `:322-324` recompute `wanted` with `select_page` for every event each frame (from `crates/wolf-game/src/map_runtime.rs:380-389`, which calls `conditions_met` at `:392-403`).
  - `:327-340` `if state.active != want { ... }` — on any change it takes the running instance (`:329`) and calls `vm.stop_instance(id)` (`:333`).
- `crates/wolf-vm/src/exec.rs:945-987` — `stop_instance` kills the instance **and all its descendants** (`:949-975`, by walking the parent chain) and wakes parents waiting on a doomed child (`:976-983`).
- `crates/wolf-vm/src/exec.rs:1209` — `210` sets the caller to `WaitState::Child(id)`, so every ancestor in the chain is a suspended `Child` waiter (variant declared at `exec.rs:98-99`, field `Instance.wait` at `exec.rs:127`).

So a variable write performed *inside* a called common event is fed back into the page-selection test of the **event that called it**, and the result is that the engine deletes the very coroutine that is still executing. That is the defect.

The intended WOLF semantics are that a page's command list — including any `210` common-event call it is blocked in — runs to completion; a page-condition change may only swap the page at an idle point. Torn-down-mid-call of a *busy* page is unintended, and the trace shows it is what both costs the rollover and corrupts the state.

**Precise missing trigger, restated for the doc's purpose:** `SET var69 = 0` at **CE329 pc41** (reached only after the `180 [60]` wait at pc35 elapses). This single write invalidates the `v69 == 1` term in **map event 4 "メニュー処理" page 0**'s activation condition, causing `MapRuntime::start_pending` to stop that page's instance and (via `stop_instance`'s descendant cascade) CE245 → CE327 → CE328 → CE329. CE329 never reaches pc385 (`var4=7`) / pc406 (`var160=1`) / pc437 (`CALL CE317`), so the day never advances.

## Minimal fix (implemented — narrowed to deactivation only)

Guard the page-switch teardown so it never aborts a page whose coroutine is currently blocked inside a `210/300` call. In `crates/wolf-game/src/map_runtime.rs`, at the top of the `if state.active != want {` block (`:328`), before the `stop_instance` at `:349`. The shipped guard is narrower than the sketch below: it only defers when `want.is_none()` (the page is being fully deactivated); page-to-page switches keep the old behaviour. See the implementation note at the top for why the broader variant was rejected.

```rust
if state.active != want {
    // A page whose coroutine is parked in a 210/300 CALL is still running its
    // own script (the call is synchronous); WOLF does not abort it mid-call.
    // Defer the page switch until the call returns.
    if let Some(id) = state.instance {
        if let Some(inst) = vm.instance(id) {
            if matches!(inst.wait, wolf_vm::WaitState::Child(_)) {
                continue; // skip this event for now; keep state.active unchanged
            }
        }
    }
    // ... existing teardown (state.self_vars = ..., vm.stop_instance(id), state.active = want, ...)
}
```

Why this is the right minimal fix:
- `WaitState::Child` is exactly the state where the page's coroutine has handed control to a called common event and is still executing its script. Deferring the stop until the call returns matches WOLF's "run the page script to completion" semantics.
- It is principle-based, not hardcoded: it does not name event 4, `var69`, or any magic number. Any page that is merely in `WaitState::Frames`/`None` still swaps normally (so the other `v69`-gated parallel pages stop as before).
- It keeps `active` unchanged while deferring, so no spurious "switches" accounting and no restart.

Residual risk to verify when applying: this changes the timing of page switches for *any* page blocked in a CALL. Recommend a targeted regression (the M6 night flow + a page that legitimately switches while a short CALL is in flight) and a full `m6_night_menu` run. An equivalent narrower variant is "defer only when the instance has live descendants" (same effect via `inst.wait == Child`).

Alternative (engine-design) fix if the Lead prefers not to touch per-frame selection: make `MapRuntime` refuse to re-select a page while its owned instance is alive and `is_runnable() == false` **and** has a child; i.e. treat "busy in a call" the same as "not switching". Either way the invariant is: *never `stop_instance` a coroutine that is blocked in a blocking call it did not itself abandon.*

## Reproduction recipe (harness used, since removed)

A temporary example ran the real engine sequentially:

1. `Game::new(work/all)`; `start_map_event_by_name("SampleMapA", 0)`; auto-advance through the prologue (~1500 frames); set `var63 = 1` to unlock the night path; answer the two opening choices (今天还是睡吧 → 是) to reach day2 08:30; click the study command at (512,230) and settle.
2. Normalise `var295 = 0`, `var68 = 0`; hover the night-menu picture (240,340) and press OK to enter CE245 via map event 4.
3. Click the sleep option (512,450); the rest-menu choice `["是","否"]` appears; choose 是.
4. Step one frame at a time, printing every instance `id[event:title]@pc<parent>` and `v4,v5,v160,v69,v64,v18,v304,v494,v295,v63`, plus `runtime.switches`/`runtime.restarts` deltas.

Result: chain identical to standalone until CE329's 60-frame wait expires at f164, where `var69` goes 1→0, `runtime.switches` rises by 5, and the chain is removed while the day stays day2 19:30.

Second temporary example parsed every `work/all/MapData/*.mps` with `wolf_format::mps::Map::parse` to locate event 4 "メニュー処理" and print its 5 pages' trigger/conditions/command call targets (table above).

Both temporary examples were deleted after producing the artifacts, per the worktree policy. The full trace remains at `work/out/ce328_abort_trace.txt`.

## Cross-check that the standalone path is a valid reference

EXP2 standalone (`start_event(245)` after `v4=19,v5=30,v160=2,v6=1,v69=1`) also parks CE328 at pc994 and enters CE329; CE329 goes pc36 → (after the 60-frame wait) runs the whole body → `v160=3, v4=7, v5=45` → day3 08:30, CE317 morning setup. This confirms CE328/CE329 logic is correct and that the only difference is the presence/absence of the owning map-event page and the page re-selection feedback.

## Relevant files / lines

- `crates/wolf-game/src/game.rs:1136-1138` — `start_pending` at end of frame.
- `crates/wolf-game/src/game.rs:663-664` — `switch_map` → `runtime.stop_all` (130 teleport; would also cascade-stop, but the killing stop here is from page re-selection, not a teleport — no "130 Teleport" diagnostic fires at f164).
- `crates/wolf-game/src/map_runtime.rs:301-376` — `start_pending`; page change block `:327-340`; `stop_instance` at `:333`.
- `crates/wolf-game/src/map_runtime.rs:380-389` — `select_page`; `:392-403` `conditions_met`.
- `crates/wolf-vm/src/exec.rs:945-987` — `stop_instance` descendant cascade `:949-975`, parent wake `:976-983`.
- `crates/wolf-vm/src/exec.rs:1209` — `210` sets caller `WaitState::Child(id)`; `:98-99` `Child(InstanceId)`; `:127` `Instance.wait`.
- `crates/wolf-vm/src/exec.rs:996-1024` — `step_frame` / `poll_waits` (Frames countdown).
- `crates/wolf-format/src/mps.rs:305-309` — `active_page_index` (highest page whose conditions hold); `:476-483` `conditions_met`; `:495-552` `PageCondition`.
- `work/_ce_full.json` — CE bytecode dump (records 245/327/328/329).
- `work/all/MapData/SampleMapA.mps` — live map; event 4 "メニュー処理".
- `work/out/ce328_abort_trace.txt` — captured per-frame trace.

## Conclusion

Not a data or branch bug in CE328. The engine aborts a map-event page that is *executing* a blocking common-event call, because the nested script writes one of that page's own activation variables (`var69`) and the per-frame page re-selection then cascades `stop_instance` through the call stack. Fix the page-switch teardown to defer while the page's coroutine is in `WaitState::Child` (proposal above). Implementation left to the Lead per task constraints.
