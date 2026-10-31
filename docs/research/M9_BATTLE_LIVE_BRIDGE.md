# M9 task-63 — Bridging the live CE65 battle menu into the rebuilt battle driver

Status: investigation + minimal implementation + tests complete. Defaults OFF, no commit.
Repo HEAD bcbcfba. Write scope: src/game.rs, src/battle_wiring.rs, tests/m8_battle_live_bridge.rs, this doc.

## 1. Question
Does shipped data map the player's CE65 battle-menu choice to the CE242 command index
(1..8) that the rebuilt driver (src/battle_wiring.rs) must supply in self-var 600?

## 2. Answer: NOT by command code — only by the registered ROW (コマンド入力位置)
The player-facing line CE188 -> CE191 -> CE192 -> CE152 -> CE65 -> CE157 registers the
chosen command into the physical CData table [x コマンド処理リスト] (runtime type id 15 is
resolved by the type label to physical type 14). Schema (docs/BATTLE.md:471):
  col0 = 発動者番号 (activator), col1 = 発動敏捷性, col2 = 受け側番号, col3 = 発動技能.

Empirical probe (temp example, since deleted) driving real keys to each selectable code:
  - selecting 防御 (25)  -> type14 row1 col3 = 25, col0 = 0, col1 = 8900000
  - selecting 強斬 (30)  -> type14 row1 col3 = 30, col0 = 0
  - selecting 蓄力 (26) / 反撃 (29) -> type14 row1 col3 = 26 / 29
  - CData type20 row73 コマンド入力位置 = 0 before AND after; seeding it to -1/0/1/2 did
    NOT change which row is written: the choice ALWAYS lands at type14 row1, col0 = 0.
  => a single menu invocation always registers command slot 1 (activator 0).

Static scan of all 550 events for opcode 250 reading type14/15:
  - CE242 NEVER reads CData type14 (absent from the whole command-phase event).
  - Type14 consumers are CE137 (status display), CE164 (行動実行結果), CE148 (ターン計算),
    CE157 (register). None feeds CE242.
  => the registered 発動技能 CODE (25/26/29/30/0) has disjoint numeric domain from the
     CE242 index (1..8) and is never consumed by it. There is NO data-level code->index map.

The ONLY structural correspondence is:
  CData type14 ROW index (== コマンド入力位置, 1-based)  ==  CE242 command index (self-var 600).
  The menu's single input always yields row 1 => index 1. That is the whole shipped bridge.

## 3. Implementation (minimal, default OFF)
- src/battle_wiring.rs: the legacy single-shot path now takes its command index from the
  schedule (next_command()) instead of the constant ACTING_PARTY_SLOT. Default schedule is
  [1, 3] and the first entry is 1, so the default configuration is byte-for-byte unchanged.
- src/game.rs: new hook pub fn set_battle_command_schedule(&mut self, schedule: Vec<i32>).
  It sets the CE242 command-index schedule; it does not itself enable wiring or the round loop.
- Both set_battle_wiring and set_battle_round_loop remain default OFF.

## 4. Test evidence (crates/wolf-game/tests/m8_battle_live_bridge.rs, 2 tests, green)
Test A  live_menu_selection_feeds_the_driver_and_settles
  - Real keys select the NON-default command 強斬 (code 30):
      menu: registered_row=1 code=30 activator=0 input_pos=0 ce242=false v350=0 warnings=0
    (menu line never starts CE242, command phase idle, code 30 > 8 so it is not an index).
  - Bridge: read the menu-written row (1) -> set_battle_command_schedule([1]) -> real
    CE253 -> CE239 battle:
      ce242=true max_ce242_pc=415 v350_seq=[1,0] pre=[5,4,3,2] post=[4,4,3,2]
      v352=1 v343=1 settled=true warnings=0
    => pool 2000345 (first member) drained, battle settled.

Test B  driver_honours_nondefault_index_but_the_menu_cannot_supply_it
  - set_battle_command_schedule([3]) -> real battle:
      ce242=true max_ce242_pc=866 pre=[5,4,3,2] post=[5,3,3,2] v352=1 v343=1 warnings=0
    => index 3 dispatches the slot-3 CE242 body and drains pool 2000346, still settles.
  - But the real menu can only register row1 / コマンド入力位置 0 => it cannot supply index>1.
    That is the documented gap (no shipped multi-input path).

## 5. CE242 index -> action body -> party pool (empirical, tasks 49/57/63)
Index 1 -> CE242 pc145 body (reaches ~pc415) -> drains pool 2000345
Index 2 -> CE242 pc476 body                  -> drains pool 2000345
Index 3 -> CE242 pc761 body (reaches ~pc866) -> drains pool 2000346
Index 4 -> CE242 pc972 body                  -> drains pool 2000346
Index 5 -> CE242 pc1057 body                 -> drains pool 2000347
Index 6 -> CE242 pc1134 body                 -> drains pool 2000348
Index 7 -> CE242 pc1196 body                 -> drains pool 2000347
Index 8 -> CE242 pc1405 body                 -> drains no pool
Pools are seeded by CE239 pc546 (2000345=5) and pc554-556 (2000346/347/348 = 4/3/2).

## 6. Gap / residual (why full multi-member is still not player-driven)
- The shipped menu registers exactly one row (row1, index1) per input; コマンド入力位置
  stays 0. So a real player can only ever feed command index 1 through this line.
- A genuine multi-member / multi-command player loop needs either (a) a multi-input menu
  that increments コマンド入力位置 and registers successive rows, or (b) an explicit engine
  convention mapping menu rows to members. Neither exists in the shipped data.
- The command CODE -> index mapping does not exist in data and must NOT be invented
  (no hardcoding). The honoured bridge is the ROW, and the driver honours any index.
- Recommended next step: extend CE191/CE192 into a real per-member selection loop that
  writes successive type14 rows, then feed each row to set_battle_command_schedule.

## 7. Commands and results
Build/test:
  WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_battle_live_bridge -- --nocapture
    -> menu: registered_row=1 code=30 activator=0 input_pos=0 ce242=false v350=0 warnings=0
    -> bridge: ce242=true max_ce242_pc=415 v350_seq=[1,0] pre=[5,4,3,2] post=[4,4,3,2] v352=1 v343=1 settled=true warnings=0
    -> index3: ce242=true max_ce242_pc=866 pre=[5,4,3,2] post=[5,3,3,2] v352=1 v343=1 warnings=0
    -> test result: ok. 2 passed.

Regressions (opcode-111 patch ABSENT, i.e. exec.rs at HEAD):
  m8_battle_live_bridge 2 passed
  m8_battle_boss_rounds 1 passed
  m8_battle_party 2 passed
  m7_battle_playable 1 passed
  m7_day4_playable 1 passed
  m7_battle_target_scheduler 6 passed
  (13 test fns total, 0 failed)

  cargo build -q -p wolf-cli ; ./target/debug/wolf-cli.exe verify
    -> PASS — 70 check(s) matched baseline, 0 pending

## 8. Caveats (shared worktree)
- Full cargo test --workspace currently does not compile because another teammate's WIP
  tests/m8_window_skin.rs has E0308 (vs src/atlas.rs:295); not mine, the six suites above
  all compile and pass.
- A teammate toggles the PARKED opcode-111 exec.rs value-operand patch
  (docs/research/M9_OPCODE_111_REGRESSION.md); while applied it transiently reds
  m7_battle_target_scheduler:363 and m6_battle_commands. Green with exec.rs at HEAD.
