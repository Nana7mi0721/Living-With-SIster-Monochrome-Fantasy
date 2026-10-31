# M7 · W1 battle playable — rebuilt command-phase wiring (task-34)

Status: implemented and green.
Scope: three production files touched — **NEW** `crates/wolf-game/src/battle_wiring.rs`,
a minimal hook in `crates/wolf-game/src/game.rs`; test **NEW** `crates/wolf-game/tests/m7_battle_playable.rs`;
this report **NEW**. No edit to `exec.rs`, `battle_seed.rs`, `db.rs` or any other test. No commit/push.

Run:

```
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_battle_playable -- --nocapture
# test result: ok. 1 passed; 0 failed
```

Result line produced by the test:

```
RESULT: ce253=true ce239=true ce242=true ce243=true ce241=true rounds=1
        battle_active_f=Some(520) v350_seq=[0, 1, 0, 2, 0] v352=1 v343=1
        max239pc=1099 max242pc=1048 warnings=0 unanswered250=0
```

i.e. a **real battle round** now runs and settles: `2000350` walks the full
`0 → 1 → (CE244 clears 0) → 2 → 0` three-state machine, CE244 writes
`2000352 = 1` and `2000343 = 1`, CE243 (enemy action) and CE241 (turn start) both run,
warnings are 0, no `250` read is unanswered, and CE239 returns (its coroutine is gone at
the end of the run).

---

## 1. Why the battle never ran before (recap)

The battle turn is a variable-driven state machine held in normal variable **2000350**
(flat slot 350):

| 2000350 | phase | event that runs | event run_condition |
|---|---|---|---|
| 1 | wait for player command | **CE242** ┣コマンド処理 (command processing) | `2000350 == 1` |
| 2 | execute the turn | **CE243** ┣敵の行動 + **CE241** ┣ターン開始処理 | both `2000350 == 2` |
| 0 | idle | — | — |

The bootstrap is circular and un-closable from the data alone (proven in
`docs/research/M7_BATTLE_FULL.md`, task-20, and `docs/BATTLE.md` §5.3.1):

* the **only** writer that sets `2000350 = 1` is CE242 itself (`pc14`);
* CE242's own start gate is `2000350 == 1`;
* no event in the whole 550-event corpus issues `210 500242`, and no script writes 242
  into a variable.

So CE242 can only start when `2000350` is already 1, but only CE242 can make it 1.
CE239 — the real battle caller — raises the battle-active flag **2000340** `0 → 1` at
`pc695`, but never touches `2000350`. The state machine therefore freezes at
`2000350 = 0`, CE244 never settles, and CE239 deadlocks in its end-judgment loop
(case2 `pc876–1103` spins until `2000343 == 1`).

## 2. The fix: rebuilt engine wiring (BATTLE.md §5.3.1 option B)

Option B: the engine starts the command phase on the rising edge of the battle-active
flag, then hands CE242 the command the caller already selected. This is an **engine
wiring addition**, explicitly labelled as rebuilt (the data has no such call).

### 2.1 New module — `crates/wolf-game/src/battle_wiring.rs`

The whole hook is one public type plus one `tick`. Constants (flat normal-variable slots):

```rust
const VAR_COMMAND_PHASE: usize = 350;   // 2000350 three-state command/action machine
const VAR_BATTLE_ACTIVE: usize = 340;   // 2000340 raised by CE239 pc695
const VAR_BATTLE_DECIDED: usize = 343;  // 2000343 set by CE244
const VAR_CHOSEN_COMMAND: usize = 354;  // 2000354 command CE239 got from its caller
const EVENT_COMMAND_PHASE: u32 = 242;   // CE242 ┣コマンド処理
```

`BattleWiring::tick` (called once per frame, before the VM steps):

```rust
pub fn tick(&mut self, vm: &mut Vm) -> Option<u32> {
    if !self.enabled { self.prev_battle_active = vm.vars.get(VAR_BATTLE_ACTIVE) != 0; return None; }
    let battle_active = vm.vars.get(VAR_BATTLE_ACTIVE) != 0;
    let rising = battle_active && !self.prev_battle_active;   // 0 -> 1 edge only
    self.prev_battle_active = battle_active;
    if !rising { return None; }
    // Do not disturb a running command phase, and do not restart after CE244 decided.
    if vm.vars.get(VAR_COMMAND_PHASE) != 0 || vm.vars.get(VAR_BATTLE_DECIDED) != 0 {
        return None;
    }
    let command = vm.vars.get(VAR_CHOSEN_COMMAND);            // the real caller's command
    if let Some(id) = vm.start_event(EVENT_COMMAND_PHASE) {   // start CE242
        if let Some(inst) = vm.instance_mut(id) {
            inst.self_vars.set(SELF_ARG_BASE, command);        // self1600000 = 選択コマンド
        }
        self.started_rounds = self.started_rounds.saturating_add(1);
        return Some(EVENT_COMMAND_PHASE);
    }
    None
}
```

`SELF_ARG_BASE` is `wolf_vm::magic::SELF_ARG_BASE` = index of magic `1_600_000` = 600.
That is exactly where the VM's own `spawn_child` puts a `210` call's first argument
(`exec.rs:1194-1198`), so this is the same argument plumbing a real call edge would use,
not a special case.

### 2.2 Minimal hook in `crates/wolf-game/src/game.rs`

* module declaration (explicit path, so `lib.rs` needs no change; `game.rs:48-53`):

```rust
#[path = "battle_wiring.rs"]
mod battle_wiring;
use battle_wiring::BattleWiring;
```

* struct field (`game.rs:296-297`): `battle_wiring: BattleWiring`, initialised
  `BattleWiring::new()` (disabled) in the constructor (`game.rs:388`).
* one call in `Game::frame`, right after the conditional events are synced and before
  `vm.step_frame` (`game.rs:1065-1068`):

```rust
self.auto_events.sync(&mut self.vm);
// Rebuilt battle wiring (M7 task-34): start the command phase on the battle-active edge.
self.battle_wiring.tick(&mut self.vm);
self.handle_choice_input();
```

* two façade methods (`game.rs:537-545`): `Game::set_battle_wiring(bool)` and
  `Game::battle_wiring_rounds() -> u64`.

Wiring is **off by default**. A production boot flips it on with one
`game.set_battle_wiring(true)`; the existing capture/regression suites that assert
`2000350` stays 0 on an unprepared battle are unaffected.

### 2.3 Where the command argument comes from

CE242's single argument is `選択コマンド` (the chosen command), which it reads as
`self1600000`. The real battle caller already resolved it:

* CE253 (the day-4 地域貢献 battle wrapper) calls CE239 at `pc174`:
  `210 [500239, …, 6, 1, 7, 1600020, 1600030]`. Its `pc173` sets its own `1600020 = 1`
  just before, so CE239 receives command `1` in `self1600003`.
* CE239 stores that command in **2000354**: `pc104` (`2000354 = self1600003`) in one
  branch, `pc798` (`2000354 = self1600003`) on the normal non-hot-spring path.
  Observed live: `2000354 = 1` by frame 519, before the battle-active edge at 520.

So the wiring reads `2000354` and passes it as CE242's `self1600000`. This is the value
the player-menu would have produced — it is supplied by the real caller, **not fabricated**.
(Verified by the earlier control run: passing **no** argument leaves `self1600000 = 0`;
CE242 then fails `pc143` `IF self1600000 in {1,2,3}` and ends without reaching
`pc201 CALL 500244` / `pc208 2000350 = 2`, so `2000350` only ever reached 1.)

## 3. The test — `crates/wolf-game/tests/m7_battle_playable.rs`

Setup:

1. `Game::new(data_root)`, `load_save(work/retail_backup/Save/SaveData01.sav)`,
   `auto_advance = auto_choice = true`, `auto_choice_delay = 1`.
2. `seed_battle_databases()` — the task-19 seed (25 cells: party count, command slots
   コマンド1..8, item/weapon stock; `statistics` on 主人公ステータス + 戦闘時一時ステータス).
3. one cosmetic data completion (see §3.1): CData `(5, 16, 0) = "┗ 状態リスト"`.
4. `start_engine_events()` — CE241 turn-start + the `run==2` scheduler (CE240 戦力処理).
5. `set_battle_wiring(true)` — the wiring under test.
6. `start_event(253)` — the real day-4 battle wrapper. CE253 is exactly the event that
   the day-4 `CE251` quest row calls at `pc2466` (`210 [500253, 0]`, inside the
   `2000048 == 7` case selected at CE251 `pc2460-2466`). The wrapper's own `102` choice
   at `pc34` is answered by `auto_choice` (option 0 = the 地域貢献 battle branch).

The frame loop presses OK on even frames, records `2000340/350/352/343`, instance PCs,
start counts and diagnostics, stops when `2000343 == 1 && 2000350 == 0`, then drains
800 frames so CE242's tail and CE239's victory branch finish.

Assertions (all hold): CE253/CE239/CE242/CE243/CE241 all start; `2000340` rises;
`wiring_rounds >= 1`; `2000350` sequence contains 1 **and** 2; `2000352 ∈ {1,-1}`;
`2000343 == 1`; `warnings == 0`; `unanswered_250 == 0`; CE239 is no longer alive
(it returned).

### 3.1 The one extra data cell (honest note)

CE242's command-menu rendering (`万能ウィンドウ`) reads one section-header label from
CData `(5, 16, 0)`; a fresh `SaveData01` leaves it blank, so the read logs
`250 DatabaseAccess: CData (5, 16, 0) … "┗ 状態リスト" / "" unanswered`. This is a **display
label**, not a command and not a target. The test completes that single text cell
(`game.databases.set_text(CData, 5, 16, 0, "┗ 状態リスト")`) so the only 250 read in the
run resolves. This is data seeding (same class as `seed_battle_databases`), **not** menu
input, and it changes no branch. Without it the round still settles; only the cosmetic
250 counter stays at 1.

## 4. Before → after state trace (measured, this HEAD)

```
f  521  v340=1 (battle-active edge)  v350=0  v352=0  v343=0  rounds=1
         alive = [239@702:Frames(9), 242@6:Frames(1)]        <- wiring started CE242 this frame
f  522  v350=1  (CE242 pc14)         v352=0  v343=0
         alive = [239@702:Frames(8), 242@996:Child(250)]     <- CE242 renders menu, reaches its chosen branch
...CE242 runs the selected command branch (calls 500244 and the win/lose judge inside CE244)...
f  613  v350=0  v352=1  v343=1                            <- CE244 win site: pc312 clears 350, pc315 352=1, pc316 343=1
         alive = [239@824:Frames(7), 242@1048:None]
f615 (drain)  v350=2  v352=1  v343=1                      <- CE242 tail pc1054 hands the turn to action phase
f616 (drain)  v350=0  v352=1  v343=1                      <- CE241 pc628 clears 350; CE243 enemy action ran in this window
...CE239 sees 2000343==1, takes its case2 victory branch, drains victory/level-up to pc1099 and ends...
ALIVE_AT_END: [63@67:Frames(1)]                           <- CE239 and CE242 both returned; only engine event 63 remains
```

So the per-frame `2000350` sequence is `[0, 1, 0, 2, 0]` — the complete three-state
machine. The `0` at f613 is CE244 clearing `2000350` as part of settlement (its win/lose
blocks start with `2000350 = 0`), then CE242's tail sets `2`, then CE241 clears it.

Settlement detail: `2000352 = 1` is the CE244 **win** site (`pc315`, `pc314 self1600000 = 1`,
`pc316 2000343 = 1`, `pc312 2000350 = 0`). The lose site is `pc371 = -1`; the third
ending is `pc416 = -2` (excluded by design). `2000352` is CE244's unique output — no
other event writes it — so its `1` is proof a real settlement ran.

## 5. Residual diagnostics (all non-warning)

```
x1 : 106 debug line: >>19000 7808 1056 …
x1 : 160/162 Transition: kind -1 flags 65546 (hard cut, no mask)
x5 : 201 SetMoveRoute: 1 command(s) not applied (no map layer yet)
x1 : 270 Party: 1 argument(s) recorded only
```

`warnings = 0`. The five `201 SetMoveRoute` notices are the battle open/close move routes
with no map layer in a headless `Game`; the executor already degrades them to no-ops
(non-blocking). None affects the state machine or the settlement.

## 6. Honest boundaries (what this is and is not)

* **It is** a real battle turn: real caller command (CE253 → CE239 → `2000354`), real
  command-phase script CE242, real CE243 enemy action + CE241 turn start, real CE244
  settlement writing `2000352 = 1` / `2000343 = 1`, CE239 returning.
* **It is not** a replay of CE251's quest-menu row: the test enters CE253 directly (CE253
  is the exact event CE251 `pc2466` calls). The CE251 → CE253 edge is statically proven;
  reproducing the day-4 quest UI that picks that row is separate work (day-level scheduling).
* **The wiring is rebuilt, not data.** No `210 500242` edge exists in the corpus. The
  addition is auditable: a labelled module + `Game::set_battle_wiring` toggle, off by default.
* **No faked input and no seeded outcome.** It does not write `2000350 = 1`, does not seed
  `2000352/2000343`, and does not fabricate a menu selection — the command argument is the
  one the real caller already put in `2000354`.
* **CE188 is untouched.** The parallel CE188/CE191/CE192/CE65 line stays as it was; this
  report is about re-connecting the CE239/CE242/CE243/CE244 subsystem that the data shipped
  orphaned.
