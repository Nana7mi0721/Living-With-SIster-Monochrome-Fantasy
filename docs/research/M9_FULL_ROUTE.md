# M9 full route: title boot to day 5 08:30

Task-64 integration test. One continuous, real-input new-game route is driven from the
shipped title screen through the day schedule to day 5 08:30, including the real day-4
CE253 / CE239 battle settlement. Test-only: only
`crates/wolf-game/tests/m9_full_route.rs` was added; no production source was changed.

## Command and result

```
cargo test -q -p wolf-game --test m9_full_route -- --nocapture
```

Result: `test result: ok. 1 passed; 0 failed; 0 ignored; ... finished in 2.08s`.

Milestone output (verbatim, abbreviated):

```
m9_full_route milestone 1: title boot -> Map003 -> SampleMapA, day 1 18:30 (frames=2006, instructions=68292)
m9_full_route milestone 2: natural sleep day 1 -> day 2 08:30
m9_full_route day 2: CE251 rows [(0, "\f[20]给店里帮忙"), (1, "\f[20]防御练习"), (2, "\f[20]秘密特训")], selecting "\f[20]给店里帮忙" at row 0
m9_full_route milestone 3: day 2 CE251 "\f[20]给店里帮忙" -> day 3 08:30
m9_full_route day 3: CE251 rows [(0, "\f[20]给店里帮忙"), (1, "\f[20]学习会"), (2, "\f[20]防御练习")], selecting "\f[20]学习会" at row 1
m9_full_route milestone 4: day 3 CE251 "\f[20]学习会" -> day 4 08:30
m9_full_route UNANSWERED250: x1: 250 DatabaseAccess: CData (5, 16, 0) flags 0x11000 labels "┗ 状態リスト"/"" unanswered
m9_full_route milestone 5 battle: ce253=true ce239=true ce242=true ce243=false ce241=false rounds=1 active_f=Some(707) v350=[0, 1, 0] v352_peak=1 v343_peak=1 max239pc=745 max242pc=471 alive=false warnings=0 unanswered250=1
m9_full_route PASS: title -> Map003 -> SampleMapA -> days 1..4 -> real CE253/CE239 settlement (2000352=1, 2000343=1) -> day 5 08:30; warnings=0, frames=8855, instructions=456004
```

## Route (test line refs in `crates/wolf-game/tests/m9_full_route.rs`)

| Milestone | Input / mechanism | Asserted state |
|---|---|---|
| 1. title boot | `game.boot()` (`crates/wolf-game/src/game.rs:679`), wait 800 frames for `title01`, one OK opens CE42, choose option 1 (`cursor 0`), then real opening prompt option 0 | exact diagnostics `130 Teleport: MapData/Map003.mps at (60, 34) ...` and `130 Teleport: MapData/SampleMapA.mps at (62, 38) ...`; opening options [广阔的战场, 广袤的森林, 微风吹拂而过的麦田, 有人在等我的家]; day var160=1, var4=18, var5=30, warnings=0, screen>=20 |
| 2. sleep day 1 | natural night menu: hover picture 120 sister + Decide, CE245, click 今天先休息吧 (512,450), confirm 是 | day 2 08:30, var160=2, var6=1, warnings=0 |
| 3. day 2 | CE251 daytime command, hover row + press; selected row 0 给店里帮忙 | clock 17:00 (var4=17, var5=0), energy var14 spent, evening settles 18:30, warnings=0; natural sleep to day 3 08:30, var6=2 |
| 4. day 3 | CE251 daytime command, selected row 1 学习会 (the study action) | clock 17:00, energy spent, evening settles, warnings=0; natural sleep to day 4 08:30, var160=4, var6=3 |
| 5. day 4 battle | real CE253 -> CE239 with `seed_battle_databases()` + `set_battle_wiring(true)`; alternating OK | ce253/ce239/ce242 started; CE239 max pc 745; battle active frame 707; v350_seq [0,1,0]; v352_peak=1; v343_peak=1; CE239 returns; warnings=0; only the cosmetic CData (5,16,0) label read is unanswered |
| 6. evening and day 5 | post-battle dinner helper, then natural night menu | var160=5, var6=4 (Friday), var4=8, var5=30, var340=0, var343=0, warnings=0 |

Test entry point: `fn the_continuous_new_game_route_reaches_day_five_0830` at
`crates/wolf-game/tests/m9_full_route.rs:370`. Natural night helper at line 92; CE251
day helper at line 201; post-battle dinner helper at line 286; battle block begins at
line 484; final day-5 assertions at line 661 onward.

## Injected values (minimal, all documented)

1. `seed_battle_databases()` at test line 488: the task-19 static battle table
   (`crates/wolf-game/src/battle_seed.rs:166`) writes 26 numeric CData cells: party
   count type18 row4 col0=1, enemy count type18 row69 col0=1, command slots 47..54,
   leader stats, item type7 row0 col0=1, and three starter weapons. Plus 8 type19
   command-menu text labels. This is data seeding, not settlement state.
2. `CData type5 row16 col0 = "┗ 状態リスト"` at test line 494: one cosmetic battle
   menu section header that a fresh save leaves blank; it is the only unanswered-250
   read and is filtered explicitly.
3. Night helper normalizes `var295` to 0 only when it is 1 (undismissed first-dinner
   TIPS flag) and always sets `var68=0` (ev4 gate), exactly as `m6_night_sleep` does
   (`crates/wolf-game/tests/m6_night_sleep.rs:219-221`).

No settlement variable is injected at any point: before the battle the test asserts
`var340==0`, `var350==0`, `var352==0`, `var343==0` (test lines 498-501).

## Data root

The test defaults to `work/all` (the full extraction with `BasicData/Game.dat` and
`MapData/TitleMap.mps`), honours `WOLF_DATA_ROOT`, and falls back to `work/full` only if
`work/all/BasicData` is missing. `work/all` has no `Save/System.sav`, which is fine: the
route starts a new game through `boot()` and battle data comes from the static seed table.

## Honest residual / divergence

* The battle trace observed `ce243=false` and `ce241=false` on this continuous natural-
  sleep route, while the standalone `m7_day4_playable` harness observes them true. The
  task-64 acceptance target is the real settlement (`v352 in {1,-1}`, `v343=1`), which is
  met here: CE244 settles 2000352=1 and 2000343=1, CE239 returns and the route reaches
  day 5. The CE243/CE241 difference is recorded rather than forced; the test does not
  assert those two flags.
* The CE251 rows are the retail PRNG roll. Day 2 draws 给店里帮忙/防御练习/秘密特训
  (no 学习会), day 3 draws 给店里帮忙/学习会/防御练习 and the test selects the 学习会
  study action. The helper falls back to the first CE252 task if no 学习会 is drawn, and
  prints the actual rows for auditability.
* The day-1 18:30 bedroom has no sister sprite (picture 120) yet; the natural helper
  lets the real dinner script advance until picture 120 appears before the hover-click,
  instead of falling back to the var63 sister-click flag used by m7_day4_playable.
