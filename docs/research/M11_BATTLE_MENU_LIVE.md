# M11 task-98 — Real player battle menu -> CE157 -> CE242 (no command injection)

Status: DONE. Test `crates/wolf-game/tests/m11_battle_menu_live.rs` is green; one shared
production fix in `crates/wolf-game/src/battle_wiring.rs` (+ a one-line call-site signature
update in `game.rs`). No commit.

## The gap this task closed

Before task-98 the rebuilt battle drive could already settle a real CE253 -> CE239 fight and
cycle per-member CE242 command rounds (task-93 `m11_party_live_bridge.rs`), but the command
sequence it consumed had to be *injected* through
`Game::set_battle_menu_command_queue([1,2,3,4])`. The actual player-facing menu line
(CE188 -> CE190 -> CE191 -> CE192 -> CE152 -> CE65 -> CE157 コマンド登録) writes each confirmed
command into the command-processing list and spawns a CE157 instance, but the harvester that was
supposed to turn those confirmations into CE242 command indexes never saw them in ordinary map
state:

```
BattleWiring::tick()  (before task-98)
  ... compute drive ...
  if !drive { return None; }              // <- returns here in map state
  ...
  self.harvest_menu_registrations(vm);    // <- unreachable unless a battle drive is armed
```

The player chooses commands in the *map* state, before the CE253 battle dispatch arms the drive,
so `drive == false` and the harvester never ran. That is the exact "missing trigger": the queue
stayed empty and the drive fell back to its default rotating schedule, which cannot reflect the
player's real per-member picks.

## The fix

Minimal, in `battle_wiring.rs`:

1. **Move the harvest above the drive gate.** `tick()` now calls
   `self.harvest_menu_registrations(vm, db)` *before* `if !drive { return None; }`. The harvester
   is idempotent (dedup by row index) and a no-op when no CE157 is live, so non-menu ticks are
   unaffected and every shipped battle path stays byte-for-byte identical.
2. **Read the committed command table, not the transient CE157 self-var.** The harvester now
   reads `CData type14` (論理 コマンド処理リスト) rows directly: `activator = col 0`,
   `ability code = col 3`, for `row in 1..=8`, skipping rows whose col 0 is negative/absent.
   Each newly seen row is pushed onto the driver queue as `row.clamp(1,8)` (row index == CE242
   command index by contract).

Because `harvest_menu_registrations` now needs the databases it gained a `db: &Databases`
parameter, and `tick` is now `tick(&mut self, vm: &mut Vm, db: &Databases)`; the single caller
`Game::frame` (game.rs L1381) was updated to pass `&self.databases`.

## Root cause of the first (wrong) attempt: a stale self-var read

The first version of the harvester read the CE157 instance's self-var `self1600017`
(`SELF_ARG_BASE + 17`) as the command row. It produced a queue of `[1, 1]`: both confirmations
looked like row 1.

Instrumenting the live instances showed why: **every CE157 instance is caught parked at pc64**
with `activator = 0,1,2,...`, `skill = 26/25/0...`, and `row = 0`. In CE157 bytecode:

- pc13 reads コマンド入力位置 (CData 18,73) into self1600017;
- pc14 increments it; pc16 clears CData(14, self17, 0);
- **pc80-84** re-reads the cursor into self1600017 -> *this* is the row the confirm writes;
- pc107-120 writes CData type14 row=self1600017 cols 0..3
  (activator / self1600018 / target / ability code);
- pc121 gate self1600023 == 0;
- pc124 increments self1600017; pc125 writes the cursor back.

So at a step boundary the instance is typically at pc64, *before* pc80-84 assigns the row;
self1600017 still holds its default 0, and `command_index_for_row(0) = clamp(0,1,8) = 1` for
every instance. self1600017 is a moving value across the coroutine's execution and cannot be
sampled reliably from `tick()`. The authoritative record is the committed CData type14 row that
pc107-120 wrote -- which is exactly what the final harvester reads.

(Hence the earlier M9 task-67 "CE157 only ever registers row 1" folklore was wrong: the producer
does advance the cursor and writes distinct rows 1..7. M11 round-2 task-89 retracted it; this
test reconfirms it by reading two distinct rows.)

## What the test proves (no injection)

`natural_menu_confirmations_harvest_into_ce242_rounds` drives a single `Game` through two phases
and never calls `set_battle_wiring`, `set_battle_round_loop` or
`set_battle_menu_command_queue` for its assertions:

- **Phase A — the real menu.** Open the retail save (`work/retail_backup/Save/SaveData01.sav`),
  seed a 2-member party, `start_event(188)`, then feed real OK keys until at least two command
  rows exist. The real line registers **two distinct rows**:
  `MenuRow{row:1, activator:0, code:26}` and `MenuRow{row:2, activator:1, code:25}`;
  2 CE157 instances spawn; the harvester records `harvested=[1,2]`.
- **Phase B — the real battle.** Same Game, arm the shipped engine events, set BOSS gauge
  (v445=1, v446=4000), `start_event(253)` (地域貢献 dispatch), feed OK keys, and let the
  default auto-arm drive. The queued harvested rows are consumed front-first by CE242.

Observed (stable):

```
menu_rows   = [ {row:1, activator:0, code:26}, {row:2, activator:1, code:25} ]
harvested   = [1, 2]            ce157_starts = 2
battle: rounds = 3
        first_pcs = [338, 645, 792]      (three distinct CE242 bodies)
        arg0s     = [1, 2, 3]            (starts with the two harvested rows [1,2])
        pool_pre  = [[5,4,3,2],[4,4,3,2],[3,4,3,2]]
        final_pools = [3, 3, 3, 2]       (>=2 member pools drained)
        settle    = (2000352 = 1, 2000343 = 1)
        turn_starts = 2                  warnings = 0
```

Assertions: battle settles (v343==1 && v350==0); rounds >= 2; turn_starts >= 1; the first
`min(len)` CE242 `self1600000` args equal the harvested rows (arg0s start `[1,2]`); at least two
distinct CE242 bodies ran (multi-member); pool 2000345 drained from 5 to <=3 before the settled
round; at least two of the four pools were drained across the fight; settlement `2000352 in {1,-1}`
and `2000343 == 1`; `warnings == 0`.

The third round (`arg0 = 3`) comes from the rotating default schedule after the two harvested
queue entries drain -- expected, since Phase A only confirms two members. The point of the main
assertion is that the *first* rounds' command indexes are precisely the rows the player's real
menu committed, with no injection.

## Scope / boundaries

- Production change is confined to `crates/wolf-game/src/battle_wiring.rs` (+ the one-line
  `tick` call-site in `game.rs`). No CLI / map_runtime edits. No commit.
- Harvesting is gated on a live CE157, so it cannot leak stale rows from an earlier fight into a
  later battle; dedup is by row index, and entering a new menu resets nothing because rows are
  re-written (row 0 col 0 goes negative between fights via CE157 pc16, so a fresh menu re-arms
  rows 1..n).
- This is still rebuilt wiring (the data ships no `210 500242` edge), as documented in the
  battle_wiring module header; the harvester only *reads* what the real menu wrote.
