# M7 — Battle database seed (`Game::seed_battle_databases`)

Status: implemented and green (task-19). Files:
`crates/wolf-game/src/battle_seed.rs` (new), `crates/wolf-game/src/game.rs` (module hook +
`Game::seed_battle_databases`), `crates/wolf-game/src/db.rs` (`Databases::seed_numbers` helper),
`crates/wolf-game/tests/m7_battle_seed.rs` (new test).

## Why a seed is needed

A new game in this build never initialises four `CData` runtime tables the battle events
read. None of them lives in a `.dat` file (they are save-variable cells), and the only
bootstrap that would fill them (CE26 / CE77 / CE78 writing コマンド1..8) has **no caller**
in the shipped event graph. So a battle started without a hand-edited save has an empty
command list and every real attack is unusable.

| Cell | Meaning | Shipped value | Seed |
| --- | --- | --- | --- |
| `CData type 0` row 12 col 47..54 | コマンド1..8 (Data type 6 row ids for the leader) | `0,0,0,0` / absent | `3,4,6,7,0,0,0,0` |
| `CData type 0` row 12 col 20..28, 9,10,4 | leader stat block CE143 copies into battle scratch | 0 (c28..c32 carry unrelated 20000/30000 in saves) | maxHP 100 / HP 100 / ... |
| `CData type 7` row 0 col 0 | 所持アイテム個数 (item 0 count) | 0 | 1 |
| `CData type 8` row 0..2 col 0 | 所持武器個数 (weapon stock) | 0 | 1 each |
| `CData type 18` row 4 col 0 | party-member count CE190 places allies from | 0 | 1 |

Command ids `3,4,6,7` map through `Data type 6` col2 to real attack action rows
`蓄力=26, 防御=25, 强斩=30, 反击=29` (`docs/research/M6_BATTLE_COMMANDS.md`). The item-0
cell is the exact gate CE152 pc89..pc95 reads (`┣所持アイテム個数` row 1600014 col0): while
it is 0, pc93 marks every real attack row unusable (type21 flag 0).

## Evidence that these cells are empty in the bundled saves

Probe over `work/full/Save/system.sav`, `work/retail_backup/Save/System.sav` and
`SaveData01.sav` (all 30 vardb types) showed, for the cells above:

* `type0 row12 c45..c50 = 0`, `c51..c60 = absent`; only `c28..c32 = 20000/30000` are set.
* `type7 row0 c0 = 0`, `type8 rows 0..9 c0 = 0`.
* `type18 row4 = all zero` (row17 carried 15, but that is not the party count).

So merging from `System.sav` would be a no-op for exactly the cells the battle line needs;
the seed writes the static new-game party template instead. The values are the ones the
already-green `seed_party` block in `crates/wolf-game/tests/m6_battle_commands.rs`
(and `m7_battle_round.rs`) uses to bootstrap the CE188 line.

## API

```rust
impl Game {
    /// Writes the battle-only CData cells; returns the cell count (25).
    pub fn seed_battle_databases(&mut self) -> usize { battle_seed::seed(self) }
}
```

`battle_seed::seed_cells()` returns the table (party count, 8 command slots, 12 stats, item 0,
3 weapons = 25 cells); `seed()` groups them by `(db_type,row)` and writes through
`Databases::seed_numbers`. The module is declared inside `game.rs` with
`#[path = "battle_seed.rs"] mod battle_seed;` so `lib.rs` needs no change.

**Deliberately not automatic.** `Game::new` and `start_event(188/239)` do **not** call it:
`m6_battle_commands.rs` asserts that *without* the item seed a real attack is unselectable
(control run). Seeding is therefore an explicit battle-entry action / test call.

## Proof (from `Game::new` alone, no save loaded)

`cargo test -q -p wolf-game --test m7_battle_seed -- --nocapture` → **ok. 1 passed**:

```
m7_battle_seed: control(selectable=false) seeded(selectable=true, ce152=true, ce192=true, target_win=true, warnings=0, unanswered=0)
```

The test:
1. asserts `seed_battle_databases() == 25` and reads back the seeded cells
   (`type18 r4 c0 = 1`, `type0 r12 c47 = 3`, `c48 = 4`, `type7 r0 c0 = 1`, `type8 r0 c0 = 1`);
2. control (`Game::new` + `start_event(188)`, 300 frames): reaches CE192 but
   `real_selectable == false` (empty item gate);
3. seeded (`seed_battle_databases()` then `start_event(188)`, 300 frames): reaches
   CE152 + CE192 + the CE157/CE158 target window, `real_selectable == true`,
   `warnings == 0`, `250 unanswered == 0`, `peak_instances >= 3` (no deadlock).

Regression checks (both still green after the change):
`cargo test -q -p wolf-game --test m6_battle_commands --test m7_battle_round` → 2 passed.

## Boundary (honest, not fixed here)

This seed unblocks **command selection** (CE152 → CE192 → CE157/CE158). It does not make a
full battle turn settle: from CE188 the turn counter 2000349 never advances and CE244
(settlement writer of 2000352/2000343) is never reached — that circular bootstrap on
2000350 is pinned in `docs/HANDOFF_M7.md` and `M7_BATTLE_ROUND`. The `250 unanswered == 0`
bar is met for the CE188 segment; the CE239 battle-caller context (where the docs measured
the 250 unanswered reads) is still unreachable in this build.
