# M10 — SavePart completeness audit

Status: **complete (M9 W5 residual)**. One concrete load-side bug was found and fixed in
`crates/wolf-game/src/savegame.rs`; every other gap below is a property of the shipped
save format and is pinned by a test rather than "fixed" by inventing new bytes.

## 1. What the format actually carries

A full WOLF save is five parts. `Game::save_to` (`crates/wolf-game/src/game.rs:517-582`)
builds parts 2, 3 and 5 from live state and copies parts 1 and 4 from the template save
(`base_save`, i.e. `Save/System.sav` or the last file loaded). `savegame::apply`
(`crates/wolf-game/src/savegame.rs:86-140`) is the inverse, and it is the narrower half.

| Runtime state | Save region | Restored by `apply`? | Evidence |
|---|---|---|---|
| 800 normal variables (incl. clock 4/5, day 160, stats 170–174) | Part3 row 0 (`2000000+n`) | yes | `savegame.rs:92-97`; test `every_serialized_field_round_trips` |
| Play time (frames) | Part2 `vars3_21[0]` | yes | `savegame.rs:130-132` |
| Sys3/Sys4 choice-window anchor | Part2 `vars3_21[9]/[10]` | yes | `savegame.rs:122-128` |
| CData numeric/text cells | variable database (`save.vardb`) | yes | `apply_saved_database`, `savegame.rs:120` |
| CData row counts set by 250 row -2 | variable database row counts | **fixed here** | `savegame.rs:102-118` |
| Picture table (number, text/file/window flags, anchor, x/y, opacity, scale incl. sign, blend, sprite-sheet grid/pattern, window size, last fade length) | Part5 events | yes | `picture_from_record` / `picture_to_record`; test `every_serialized_field_round_trips` |
| Part1 map snapshot / Part4 object tables | carried from template | copied verbatim, **not interpreted** | `Game::save_to`; test asserts `written.part1/part4 == template` |
| Common-event self variables and strings | Part3 self-var/self-string groups | **no — written only** | test `common_event_self_variables_are_written_but_not_restored` |
| Picture `hidden` flag | **no field in Part5** (`var2/var3` always 0) | **no** | test `a_hidden_picture_has_no_save_bit_and_reloads_visible` |
| Map id, tile map, leader/event placement, map switches | **no region on this path** | **no** | test `the_map_tile_layer_and_leader_are_not_part_of_this_save_path` |
| System variables other than Sys3/4 | **no region** | **no** | test `system_variables_outside_the_anchor_pair_have_no_save_region` |
| Extension row `100+n` without a 250 row-count write | not in the template vardb | **no (counted unmodelled)** | test `an_extension_row_written_without_a_row_count_is_refused`; `M8_TEMPLATE_SAVE.md` caveats 1–2 |

## 2. The concrete bug that was fixed

A run-time `250` write to row `-2` (`ROW_COUNT_SET`) grows a table before a script writes
the appended rows. `Databases::write_into` (`crates/wolf-game/src/db.rs:627-676`) applies
`reconcile_row_counts`, so the saved vardb correctly gained the extra rows (e.g. CData
type 0 grew from its 17 `.dat` rows to 101).

On load, however, `apply_saved_database` copied cells but never set the row-count overlay.
`Databases::row_count` (`crates/wolf-game/src/db.rs:248-268`) therefore still answered the
stale static height (17), even though the save held 101 rows. Consequences:

* a later `ROW_COUNT_QUERY` (row -1) or a list loop bound undercounted the table;
* `row_exists` said rows 17..100 did not exist, so script reads of the restored cells were
  treated as out-of-range even though the engine cells were present.

Fix (in scope, `crates/wolf-game/src/savegame.rs:102-118`): before overlaying the database,
walk the saved vardb types and restore each CData type whose saved row count differs from
the current one with `Databases::set_row_count(Container::CData, db_type, saved.rows.len())`.
Only CData travels in a save (SysDatabase/DataBase are engine configuration), and only
tables whose height changed are touched.

Safety check: all nine retail saves have the same row count for all 30 types as the static
tables of both `work/full` and `work/all` (probed while developing this test), so retail
loads are byte-behaviour neutral. The full `cargo test -q -p wolf-game` suite stays green.

## 3. Tests

`crates/wolf-game/tests/m8_save_parts.rs` (6 tests):

1. `every_serialized_field_round_trips` — 800 normal variables including the clock, day and
   170–174 stats; play time; Sys3/Sys4; one numeric and one text CData cell; a row-count
   resize to 101 with cells in extension row 100; five pictures (text, spritesheet, window,
   mirrored scale `-100`, zero scale); Part1/Part4 template identity. Mutates the live game,
   loads into a fresh `Game`, and asserts every value and the restored row count.
2. `common_event_self_variables_are_written_but_not_restored` — starts CE244, sets a self
   variable and a self string, proves both are present in the written Part3, then proves a
   fresh load starts CE244 with the default value.
3. `a_hidden_picture_has_no_save_bit_and_reloads_visible` — a `hidden` picture is written
   (slot survives) but reloads visible.
4. `the_map_tile_layer_and_leader_are_not_part_of_this_save_path` — a started map has a tile
   layer live, but the reloaded game has `tilemap == None` and no leader.
5. `system_variables_outside_the_anchor_pair_have_no_save_region` — Sys0/Sys5 do not survive.
6. `an_extension_row_written_without_a_row_count_is_refused` — writing row 100 without a
   resize increments `stats.unmodelled`, leaves the saved table at 17 rows, and the cell is
   absent after reload.

Commands and results (Windows, repo root):

```
cargo test -q -p wolf-game --test m8_save_parts
# 6 passed; 0 failed

cargo test -q -p wolf-game --lib
# 231 passed; 0 failed

cargo test -q -p wolf-game
# every test binary green
```

## 4. Residual gaps (documented, not invented)

* **Common-event self variables.** `Game::save_to` writes them when an instance is alive
  (`game.rs:533-548`), but `load_into` cannot restore them: a fresh `Game` has no
  coroutines, and starting an event later creates a new instance with zeroed `SelfVars`.
  A correct fix needs an event-id keyed self-variable bank applied when events start, which
  is a VM/Game architecture change, not a serializer fix. Left as a gap.
* **Hidden pictures.** SavePart5 has no hidden bit in this build (`var2/var3` are always 0,
  and `picture_to_record` writes neither), so a `150 Hide` slot reloads visible. There is no
  retail evidence for which unused word could carry the flag, so none was invented.
* **Map/object state.** Part1 exists in a full save but `Game` does not model a map snapshot;
  part1 is template-carried and ignored on load. Tile map, leader placement, switches and
  event positions rebuild only when a map is started, so a load lands on an empty runtime.
* **System variables.** Only Sys3/Sys4 live in Part2. `sys12` is an engine per-frame flag;
  the other system-variable slots are transient and have no save region (documented in
  `docs/SAVE_SPEC.md:459-462`).
* **Picture presentation state.** `extras`, `tone`, `effects`, `color_corr`, `shift`,
  `shake/zoom/breath/pattern`, `age`, `transition` and the active fade are not restored
  into the live `Picture` by `picture_from_record`. Retail SavePart5 **does** carry a
  picture-state tail bank (M12 task-103 measured it and preserves it byte-for-byte via
  `Picture::saved_record` — see `docs/research/M12_SAVE_FULL_COMPARE.md`), but this port
  does not interpret those words yet. A loaded save therefore redraws the picture
  positions and static appearance but not a mid-effect animation, and a load→save keeps
  the saved tail values rather than simulating them.
* **DB containers.** SysDatabase and DataBase cells are excluded by design (engine
  configuration vs the player's 可変DB), see `docs/research/M8_TEMPLATE_SAVE.md §6`.
* **Extension rows without a resize.** A script that writes `100+n` without first growing
  the table is refused and counted unmodelled; this is the existing M8 caveat, exercised
  through the live game by test 6.

No other production file was modified. The only source change for this task is the row-count
restore in `crates/wolf-game/src/savegame.rs`; `Game::save_to`, `map_runtime.rs` and the VM
were left untouched because their gaps need architecture changes, not serializer patches.
