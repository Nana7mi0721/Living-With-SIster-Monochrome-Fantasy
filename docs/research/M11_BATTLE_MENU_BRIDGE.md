# M11 task-85 — real battle command menu -> CE242 command-index bridge

Repo: E:/Study/github program/imoto life. HEAD at task start: 22de104.
Write scope: crates/wolf-game/src/battle_wiring.rs, crates/wolf-game/tests/m11_battle_menu_bridge.rs,
docs/research/M11_BATTLE_MENU_BRIDGE.md. game.rs/db.rs/exec.rs untouched. No commit/push.

## 1. The gap, in one paragraph

The player-facing battle menu (CE188 -> CE191 -> CE192 -> CE152 -> CE65 -> CE157) registers the
chosen command into the command list (physical CData type14). The rebuilt battle driver in
crates/wolf-game/src/battle_wiring.rs does NOT read that list; it consumes a *successive sequence
of CE242 command indexes* 1..8 (CE242 self-var 600, SELF_ARG_BASE). The only data contract between
them is that the **registered command ROW (1-based) equals the CE242 command index**; each index
dispatches a distinct CE242 action body that drains a distinct party pool:

  index -> body pc start (empirical, tasks 49/57/63) -> party pool (200034x)
  1 -> pc145 (body ends ~pc415..474) -> 2000345 (first member)
  2 -> pc476                        -> 2000345
  3 -> pc761 (body ends ~pc866..967) -> 2000346 (Kana)
  4 -> pc972                        -> 2000346
  5 -> pc1057                       -> 2000347 (Yui)
  6 -> pc1134                       -> 2000348 (Kasumi)
  7 -> pc1196                       -> 2000347
  8 -> pc1405                       -> (none)
  Dispatch heads: 1/2/3 -> pc143, 4/5/6 -> pc970, 7 -> pc1194, 8 -> pc1403.
  Pools are seeded by CE239 pc546 = 5, pc554 = 4, pc555 = 3, pc556 = 2.

There is **no** command-code -> index map in the data (codes {25 防御, 26 蓄力, 30 強斬, 29 反撃,
0} are disjoint from indexes 1..8). None is invented here; only the row==index contract is used.

## 2. Why the live menu cannot supply a >1 sequence (precise blocker)

The CE157 producer (コマンド登録, run0, 175 cmds) writes the chosen command into a command row, but
never advances コマンド入力位置 (CData logical type15 / physical type14; input position lives at
CData(20,73,0)):

- pc121 gates the cursor advance on `self1600023 == 0`.
- On the live path self1600023 is set to **100** (pc93 / pc100, resolved-member-id >= 100), so the
  gate fails and the advance at **pc124** (self1600017 += 1) / **pc125** (CData(20,73) = self1600017)
  is skipped.
- The alternative advance branch at pc168/169 (guarded by self1600002 == 1) is unreachable because
  the instance terminates around pc152.

Consequence: every selection overwrites command **row 1**, コマンド入力位置 stays **0**, CE194
(行動順の計算) reads CData(20,73) at pc7 into self1600030 and loops pc13 c179 that many times, i.e.
**zero**, so the CE188 engine executes **no** command. This is exactly the task-67 finding and it is
reproduced live in this task's test A (below).

## 3. What was added to battle_wiring.rs (engine half)

The engine already consumed a successive command-index sequence via
`Game::set_battle_command_schedule(Vec<i32>)` + `Game::set_battle_round_loop(true)` (task-57/63).
task-85 adds a *real registration harvester* so the driver can in principle lift indexes straight off
the CE157 command list, without touching game.rs:

- `pub struct MenuSelection { activator: i32, skill: i32, row: i32, command_index: i32 }`.
- `pub fn harvest_menu_registrations(&mut self, vm: &Vm) -> usize` — scans live `vm.instances()`
  for `event == EVENT_COMMAND_REGISTER (157)`, de-dupes by instance id (BTreeSet), reads
  self1600000 (発動者番号, REG_ACTIVATOR), self1600001 (発動技能 code, REG_SKILL), self1600017
  (command row, REG_ROW), records a MenuSelection with `command_index = row.clamp(1,8)`, and
  enqueues that index into a `menu_queue: VecDeque<i32>`. Called once per `tick()`.
- `pub fn set_menu_command_queue(&mut self, commands: Vec<i32>)` — programmatic equivalent
  (each entry clamped through `command_index_for_row`).
- `pub fn menu_selections(&self)`, `pub fn harvested_command_indexes(&self)`,
  `pub fn menu_queue_len(&self)` — introspection.
- `next_command()` now drains `menu_queue` front-first, then falls back to the existing rotating
  `command_schedule` (default [1, 3]). When the queue is empty this is byte-for-byte the old default.
- Module-doc section "## Menu -> CE242 bridge (M11, task-85)" records all of the above and the fact
  that the shipped producer writes only row 1.
- 3 unit tests in `#[cfg(test)] mod tests`: menu_queue_drains_before_schedule,
  command_index_for_row_clamps_to_board, set_menu_command_queue_clamps_entries.

Because `mod battle_wiring` is declared *inside* game.rs (`#[path="battle_wiring.rs"] mod
battle_wiring;`, game.rs:50-51), integration tests cannot reach these methods directly. The single
public entry point the Game layer must expose to make the harvester usable in production is:

  ```rust
  // in crates/wolf-game/src/game.rs (NOT edited here -- out of write scope)
  pub fn set_battle_menu_command_queue(&mut self, commands: Vec<i32>) {
      self.battle_wiring.set_menu_command_queue(commands);
  }
  ```

This is a request to the Game-layer owner, not applied.

## 4. Tests and exact observed values

File: crates/wolf-game/tests/m11_battle_menu_bridge.rs

### A) real_menu_registers_only_row_one_and_never_advances_input_position
Loads SaveData01, seeds battle cells, `start_event(188)`, drives the CE65 cursor to the row holding
code 30 (強斬) with real DOWN/UP/OK keys, then scans the command list.
Observed: `registered_rows=[1] input_pos=0 ce242=false v350=0 warnings=0`
=> asserts registered_rows == vec![1], input_pos == 0, CE242 never runs in the menu line, v350 == 0,
warnings == 0. This is the reproducible live blocker: the menu yields a single command, row 1.

### B) successive_command_indexes_run_distinct_ce242_bodies_and_settle
Loads SaveData01, seeds, `set_battle_wiring(true)`, `set_battle_round_loop(true)`,
`set_battle_command_schedule(vec![1, 3])`, BOSS context (vars 445 = 1, 446 = 4000), `start_event(253)`,
then frames with real OK keys while tracking the live CE242 instance ids, v350 sequence and pools.
Observed:
`v350_seq=[0,1,2,0,1,2,0,1,2,0,1,2,0,1,0] pre=[5,4,3,2] post=[2,2,3,2] rounds=5 turn_starts=4
 round_body_max=[338,6,967,792,6,967,338,6,471,792,6,967,338,6,967] v352=1 v343=1 settled=true warnings=0`
=> asserts battle active, settled, rounds >= 2 (5), turn_starts >= 1 (4, the REAL CE241 turn-start),
v350 has 2 -> 0 (CE241 reset), pre == [5,4,3,2], per-round CE242 body pcs include a low body
(338 / 471, index 1) AND a high body (792 / 967, index 3) proving two distinct CE242 bodies, >= 2
distinct member pools drained (2000345 5 -> 2, 2000346 4 -> 2), v343 == 1, v352 in {1,-1} (1),
warnings == 0. (The stray "6" entries are a one-frame spawn pc of alternately-live CE242 instances;
they do not affect the low/high-body assertions.)

## 5. Result and remaining gap

- Engine half: READY. A real successive command-index sequence [1, 3] runs >= 2 real CE242 rounds,
  reaches distinct bodies/pools, chains the genuine CE241 turn-start, and the real CE239 battle
  settles (v352 = 1, v343 = 1, warnings 0). Proven in test B with no faked state.
- Producer half: still BLOCKED at CE157 pc121/pc124-125 (self1600023 == 0 gate never satisfied on
  the live path; input position stays 0). Until that VM/data cursor advance is fixed (or the Game
  layer maps menu selections to CE242 indexes explicitly), the shipped menu can only ever hand the
  bridge index 1. Fixing it is a data/VM-layer change outside this task's write scope; no command-
  code -> index shortcut was invented.

## 6. Verification commands and results

- `cargo test -q -p wolf-game --lib` => 234 passed, 0 failed (includes the 3 new battle_wiring
  unit tests).
- `WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_battle_menu_bridge -- --nocapture`
  => 2 passed, 0 failed.
- Regressions: `m8_battle_boss_rounds` 1 passed, `m8_battle_live_bridge` 1 passed,
  `m7_day4_playable` 1 passed, `m7_battle_playable` 1 passed.

No commit/push performed.
