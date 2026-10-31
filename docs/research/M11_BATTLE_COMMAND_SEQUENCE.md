# M11 - Real multi-member battle command sequence (task-83)

## 1. Scope

This note documents the proof that the rebuilt battle driver can run a genuine
multi-round, multi-member command sequence through the REAL CE239 battle chain,
not just a single default action. It also records the exact evidence and the
remaining honesty boundaries.

New files for task-83 (no production edits):

- crates/wolf-game/tests/m11_battle_command_sequence.rs (418 lines, 2 tests)
- docs/research/M11_BATTLE_COMMAND_SEQUENCE.md (this file)

Production behaviour is unchanged: src/battle_wiring.rs and src/game.rs were not
modified by this task. The test only calls the existing public switches
Game::set_battle_wiring, Game::set_battle_round_loop and
Game::set_battle_command_schedule.

## 2. How to run

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_battle_command_sequence -- --nocapture

Result on HEAD 71803f986c137e48b7b114bd4dba7b8f750b7300:

    running 2 tests
    ... both tests pass ...
    test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.49s

Related regressions were also run and stayed green:

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_battle_boss_rounds --test m8_battle_party --test m9_battle_round_gap -- --nocapture

    m8_battle_boss_rounds: 1 passed
    m8_battle_party:        2 passed
    m9_battle_round_gap:    2 passed
    total:                  5 passed, 0 failed

## 3. Why the BOSS path is required

A normal CE239 battle seeds the enemy gauge 2000342 from party power / 8
(CE239 pc195), so the first command round always zeroes the gauge and CE244
settles immediately. There is then no second command round to observe.

CE239 has a BOSS branch at pc202 that tests 2000445 == 1, and pc204 boosts the
enemy gauge from 2000446. The test seeds 2000445 = 1 and 2000446 = 4000 (the same
fixture as tests/m8_battle_boss_rounds.rs:40 and :69-71). The gauge survives one
round, so the rebuilt round loop must chain several REAL CE242 command rounds
before CE244 can settle.

## 4. Test setup (real events only)

The helper drive() is crates/wolf-game/tests/m11_battle_command_sequence.rs:118.
It performs:

1. Game::new(work/all) then load_save(work/retail_backup/Save/SaveData01.sav).
2. seed_battle_databases() > 0, then set CData (5,16,0) to the section header
   "┗ 状態リスト" (the same cosmetic cell used by m8_battle_boss_rounds.rs:65).
3. start_engine_events(); set_battle_wiring(true); set_battle_round_loop(true).
4. set_battle_command_schedule(schedule.to_vec()) with the requested indices.
5. Seed the BOSS branch: vm.vars.set(445, 1); vm.vars.set(446, 4000).
6. start_event(253) to enter the real CE253 battle chain.

The test never writes 2000350, 2000352 or 2000343. The phase variable 2000350 is
written by CE242 (1), CE243 / the real CE241 at pc628 (2 -> 0), and CE244 at
settlement, exactly as authored. The settlement variables are read, never seeded.

The command indices passed to CE242 are the data-level equivalent of a menu
selection: CE242's arg0 is self1600000 (flat self slot 600), the selected command
index 1..8. The test asserts that the live CE242 instance really receives each
scheduled index, so this is not a global default action.

## 5. CE242 command-body map

Source of the ranges: the CE242 block decode in src/battle_wiring.rs module docs
(lines 30-40) and tests/m8_battle_party.rs:151-160.

| command | first-PC body [lo, hi) | party action-point pool |
|---------|------------------------|--------------------------|
| 1 | [145, 475)   | 2000345 (member 1) |
| 2 | [476, 761)   | 2000345 (member 1) |
| 3 | [761, 970)   | 2000346 (member 2) |
| 4 | [972, 1057)  | 2000346 (member 2) |
| 5 | [1057, 1134) | 2000347 (member 3) |
| 6 | [1134, 1194) | 2000348 (member 4) |
| 7 | [1196, 1403) | 2000347 (member 3) |
| 8 | [1405, 1436) | none (display/generic body) |

All four party pools are seeded by CE239 for a normal (non-DLC) battle: 2000345
through 2000348 = 5, 4, 3, 2. The single-member branch that would zero 346..348
is gated on 2000598, which only DLC event CE452 writes.

## 6. Scenario A - schedule [1, 2, 3, 4]

Command:

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_battle_command_sequence four_round_sequence_maps_each_command_to_its_ce242_body_and_pool -- --nocapture

Observed output:

    schedule [1,2,3,4] settle_frame=1942 rounds=4 turn_starts=3
      phase_seq=[0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0]
      pools=[3, 2, 3, 2] v352=1 v343=1 warnings=0
      ce241_live=499 ce242_live=620 ce243_live=553 ce244_seen=false
      round idx=1 arg0=1 first_pc=338 frames 521..951  pre=[5,4,3,2] post=[4,4,3,2]
      round idx=2 arg0=2 first_pc=645 frames 951..1411 pre=[4,4,3,2] post=[3,4,3,2]
      round idx=3 arg0=3 first_pc=792 frames 1411..1695 pre=[3,4,3,2] post=[3,3,3,2]
      round idx=4 arg0=4 first_pc=996 frames 1695..1942 pre=[3,3,3,2] post=[3,2,3,2]

Interpretation:

- Four separate real CE242 command phases start (rounds = 4) and each receives
  the scheduled index as arg0 (1, 2, 3, 4).
- The first PC of each round (338, 645, 792, 996) lies inside the expected body
  window for command 1, 2, 3 and 4 respectively.
- Command 1 and 2 drain pool 2000345; command 3 and 4 drain pool 2000346. The
  per-round pre/post arrays show exactly one pool losing one point per round,
  and the pre of each later round equals the post of the previous round.
- Three real CE241 turn-starts happen between the four rounds (turn_starts = 3),
  and the phase sequence shows three 2 -> 0 gap resets plus the final settlement
  2 -> 0.
- CE244 settles: v343 = 1, v352 = 1, warnings = 0, no deadlock (settled at frame
  1942 well inside the 40000-frame bound).

## 7. Scenario B - schedule [1, 3, 5, 6] (all four members)

Command:

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_battle_command_sequence four_member_rotation_drains_all_four_action_point_pools -- --nocapture

Observed output (first four rounds plus the settlement summary):

    schedule [1,3,5,6] settle_frame=4064 rounds=9 turn_starts=8
      phase_seq=[0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 0]
      pools=[2, 2, 1, 0] v352=1 v343=1 warnings=0
      ce241_live=1529 ce242_live=1302 ce243_live=1309 ce244_seen=false
      round idx=1 arg0=1 first_pc=338  frames 521..951  pre=[5,4,3,2] post=[4,4,3,2]
      round idx=3 arg0=3 first_pc=792  frames 951..1380 pre=[4,4,3,2] post=[4,3,3,2]
      round idx=5 arg0=5 first_pc=1084 frames 1380..1810 pre=[4,3,3,2] post=[4,3,2,2]
      round idx=6 arg0=6 first_pc=1149 frames 1810..2175 pre=[4,3,2,2] post=[4,3,2,1]

Interpretation:

- The first four rounds select commands 1, 3, 5 and 6, whose first PC values
  (338, 792, 1084, 1149) fall in the expected body windows for commands 1, 3, 5
  and 6.
- Those four commands drain four DISTINCT pools: 2000345, 2000346, 2000347 and
  2000348. After four rounds the pool vector is [4, 3, 2, 1], i.e. every member
  has acted once.
- The round loop then cycles the same schedule again; the battle needs nine real
  command rounds in total and settles at frame 4064 with v352 = 1, v343 = 1,
  warnings = 0. Final pools are [2, 2, 1, 0].
- Eight real CE241 turn-starts occur across the nine rounds (turn_starts = 8).

## 8. What the assertions pin

All line refs are crates/wolf-game/tests/m11_battle_command_sequence.rs.

- assert_rounds at :255 checks, for every requested round:
  - rec.index equals the scheduled command index (:264-271);
  - live CE242 arg0 (self1600000, flat self slot 600) equals that index
    (:272-277);
  - the FIRST CE242 PC above the dispatch head lies in [lo, hi) for that command
    (:278-287);
  - each record's pre pools equal the previous record's post pools (:288-294);
  - exactly the command's own pool loses one point and no other pool changes
    (:295-307).
- assert_common_real_event_evidence at :309 checks:
  - real CE239 and CE253 are live (:310-311);
  - real CE242, CE243 and CE241 coroutines run (:312-320);
  - turn_starts equals rounds_started - 1 (:321-327);
  - the phase sequence has at least one 2 -> 0 reset per inter-round gap, and at
    most one extra settlement reset (:328-345);
  - rounds.len equals rounds_started (:346);
  - CE244 settlement outputs: v343 = 1 and v352 in {1, -1} (:347-353);
  - game.stats.warnings == 0 (:354).
- Test A is at :354; test B is at :386.

## 9. Why FIRST PC, not maximum PC

A command body is not a closed region. After its own action, CE242 falls through
into shared command-processing code. For example the command-1 round in scenario A
starts at pc 338, its own body ends around pc 415, and the same CE242 instance
later reaches pc 792..967 through shared paths. A maximum-PC assertion would
therefore misattribute command 1 to command 3's body window.

The reliable discriminator is the FIRST CE242 PC above the dispatch head (pc > 143)
after the round starts. In both schedules the observed first-PC values fall
cleanly inside the expected body ranges:
command 1 -> 338 (in [145,475));
command 2 -> 645 (in [476,761));
command 3 -> 792 (in [761,970));
command 4 -> 996 (in [972,1057));
command 5 -> 1084 (in [1057,1134));
command 6 -> 1149 (in [1134,1194)).

The live arg0 check is an independent proof: the instance really carries the
scheduled index, not merely a default.

## 10. CE244 is invisible as an instance, but its writes are real

In both runs the printed ce244_seen is false, yet the settlement outputs are
present (v343 = 1, v352 = 1). This is expected and already documented by
tests/m9_battle_round_gap.rs: CE244 is a 432-slot event that can start and finish
in the same frame, and Vm::spawn_child (crates/wolf-vm/src/exec.rs:1163-1209)
does not record started_events. The test therefore treats the variable writes as
the CE244 evidence rather than asserting a visible CE244 instance. This is the
same honesty boundary used by the existing battle tests.

CE241 has a similar caveat: its boot-once ENGINE_AUTO_START instance also exists
from frame 0, so ce241_live_frames alone is not proof of inter-round starts. The
direct proof is battle_wiring_turn_starts (3 gaps in scenario A, 8 in scenario B)
plus the observed 2000350 2 -> 0 resets in the phase sequence.

## 11. Honesty boundaries and residual gaps

- This is REAL CE239 battle data: CE253 -> CE239 -> CE242 / CE243 / CE244, with
  the real CE241 turn-start between rounds. It is not a hand-started CE242 and not
  a fabricated phase variable.
- The rebuilt driver only STARTS events. It does not write 2000350, 2000352 or
  2000343 and does not fake a menu. The command index handed to CE242 is the
  documented equivalent of the menu selection (self1600000); the actual chosen
  ability 2000496 is already seeded by CE239 pc581 to 4 (normal attack).
- The live player-menu path still cannot feed more than one command index: CE157
  writes only command row 1 and never advances the anti-input position, so CE194
  always consumes row 0 (see docs/research/M9_BATTLE_MULTI_INPUT.md). The
  multi-index schedule used here is the rebuilt per-index driver on the BOSS path,
  not a live menu rotation.
- A normal battle still settles in one round because its enemy gauge is party
  power / 8. The multi-round proof therefore uses the BOSS branch (2000445 = 1,
  2000446 = 4000), exactly as m8_battle_boss_rounds does.
- The round count is data/PRNG dependent. Scenario A is pinned at 4 rounds;
  scenario B is asserted as at least 4 distinct-member rounds and currently needs
  9. The test does not require a specific final round count for scenario B, only
  that the first four rounds map to commands 1/3/5/6 and drain all four pools.
- No production source was changed. The permanent scratch files
  (crates/wolf-format/examples/opscan.rs,
  crates/wolf-format/tests/_scratch_cdata.rs,
  crates/wolf-format/tests/_scratch_event.rs) were not touched. No git clean, no
  commit and no push were performed.
