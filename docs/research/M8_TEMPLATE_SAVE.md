# M8 W4-b — a data root without `Save/System.sav` is now saveable

Status: **DONE** (acceptance tests green, all existing save tests still green).
Worktree branch `worktree-agent-a68b5f7f7486fb7f5`, based on local master `eb83b23`.
Deliverables:

- `crates/wolf-save/src/lib.rs` — `SaveFile::new_game_template()` (empty database),
  `SaveFile::new_game_template_with_vardb()` (caller schema), plus unit tests.
- `crates/wolf-game/src/db.rs` — `Databases::template_vardb()`, the static CData schema
  export.
- `crates/wolf-game/src/game.rs` — the `base_save` fallback in `Game::new` only.
- `crates/wolf-game/tests/m8_template_save.rs` — the acceptance test.
- this document.

No save-format bytes changed: the nine retail saves still rebuild byte-for-byte
(`crates/wolf-save/tests/*`, 9/9 checked, see §5).

## 1. The gap

`Game::new` used to do

```rust
let base_save = crate::savegame::read_save(&root.join("Save").join("System.sav")).ok();
```

`work/all` ships the game data but no `Save/` directory, so `base_save` stayed
`None` and `Game::save_to` refused with
`220 SaveLoad: no template save (Save/System.sav is missing)`. A game started on
that root could be played but never saved: a full save needs Part1 (map snapshot +
object table) and Part4 (object-table continuation), which the running game does
not maintain — they are copied from the template save.

## 2. `SaveFile::new_game_template()` (wolf-save)

A public, infallible constructor that builds a complete in-memory `SaveFile` at
`file_version = 0x8C`. It never reads game data. Every segment is at its
version-gate default and is encoded by the existing per-part writers, so the typed
views and the stored chunk bytes are identical by construction.

| Segment | Defaults |
|---|---|
| header | `raw[6] = 0x55` (UTF-8); seeds 0; checksum recomputed from the body |
| fixed header | `START_BYTE = 0x19`; name `"New Game"` (NUL-terminated MemData<WORD>); version `0x8C` |
| SavePart1 | `var1..var3 = 1/0/0`; 64×36; `map_tiles` **empty → the `0xFFFFFFFF` no-snapshot sentinel**; empty chip image; 31 empty auto-tile names; 192 zero switch words; 0 objects; 11-word tail `[0,0,0,0,100,100,0,0,0,0,100]`; empty vars3 |
| SavePart2 | every `>= 0x8C` block present (`block100`, `md1_md2`, `bgm`, `block109`, `mds7_9`, `block137`, `block8a`), all zero/empty; `vars3_21 = [0; 19]` |
| SavePart3 | `var2 = 0` groups; `var3 = 1` row = the 800 normal variables (all 0); `var4 = 0` mds1; `var5 = 550` lists × 100 words; `var6 = 550` slots × 5 NUL strings |
| SavePart4 | one all-default lead object (`var1 = 0xFFFFFFFE`, `var6 = 0xFFFFFFFF`, `md1 = [0]`), 0 further objects, empty vars2/vars3 |
| SavePart5 | `var1 = 0`, 0 picture records |
| VariableDatabase | `unknown = 0x6F`; `new_game_template()` ships `type_count = 0`, `new_game_template_with_vardb(db)` ships the caller database's types/rows verbatim (see §2.1) |
| SavePart7 | the one-byte gate `[0]` (no writer existed; the constructor stores it verbatim, as `rebuild()` expects) |
| trailer | `END_BYTE = 0x19` |

Why these shapes:

- Part1 must supply the 31 auto-tile entries and 11 tail words at 0x8C or its own
  encoder rejects the segment; an empty `map_tiles` is the documented sentinel, not
  a missing field.
- Part3's encoder **errors when `var6 == 0`** (parse treats it as corruption), so a
  template needs at least one slot; 550 groups is the widest event table the retail
  saves use (`SaveData05/08`), which covers all 550 common events of this build.
  Row 0 with 800 words is what makes normal variables survive `save_to`/`load_into`.
- Part4's lead object is not optional in the format.
- SavePart7 is not rebuilt from its typed view; `SaveFile::rebuild` copies
  `chunk_bytes(PART7_ID)`, so the constructor lays out the chunk table and the
  decrypted buffer exactly as `parse` would.

Proven invariants (unit tests in `lib.rs`):

- `save.serialize() == save.rebuild()`;
- `SaveFile::parse(&save.to_bytes_rebuilt())` succeeds, `checksum_ok()` is true,
  and every typed part equals the constructed one;
- `parsed.to_bytes()` and `parsed.to_bytes_rebuilt()` both reproduce the same bytes.

### 2.1 The template variable database

An empty `VariableDatabase` is enough for Part1/Part2/Part3/Part4/Part5 but cannot
store a database cell: `VariableDatabase::set_num`
returns false when the type, row or field id is absent, and
`Databases::write_into` counts each refusal as `unmodelled`. So the constructor is
split in two:

- `SaveFile::new_game_template()` keeps the empty database for wolf-save-only users.
- `SaveFile::new_game_template_with_vardb(vardb) -> Result<SaveFile>` embeds a
  caller-supplied database. It refuses a database whose `row_count` disagrees with
  `rows` or whose row index is not its position, instead of writing a file that
  cannot be parsed back.

`wolf-game` builds the supplied database from the static `BasicData/CDataBase.dat`
it already loads (`Databases::template_vardb()`):

| Save field | Source |
|---|---|
| `unknown` | `0x6F`, as every retail database stores |
| type count / order | one `VariableType` per CData type, in .dat order (30 in this build) |
| `unknown` (per type) | `-2`, the shape every retail type uses (`m_dis` + `m_fieldCount` follow) |
| `dis` | the .dat `data_id_method`; measured to equal the retail `m_dis` on every type (`0:1`, `7:20002`, `8:20003`, `9:20004`, `17:30000`, …) |
| `field_count` / `config` | the .dat field table in logical column order, each entry its **raw** position word (`1000 + n` numeric, `2000 + n` textual), exactly what a retail save carries |
| `row_count` / `rows` | every row of the static table; every cell is populated, numeric from `Row::numbers`, text from `Row::strings` re-encoded as the `MemData<DWORD>` body (NUL appended; an empty editor string is the one-byte `[0]` body retail uses) |

Only CData is exported: `SysDatabase.dat` is engine configuration and `DataBase.dat`
is the editor's fixed table, neither of which belongs in a save (§6.2). Runtime
row-count resizes and engine-owned cells are not baked into the template; they go
through the existing `Databases::write_into` / `reconcile_row_counts` path when
`Game::save_to` runs, exactly as they did with a retail `System.sav`.

## 3. `Game::new` fallback (wolf-game)

Only the `base_save` initializer changed. When `Save/System.sav` cannot be read:

- absent → `220 SaveLoad: <root>/Save/System.sav is absent; using a synthesized new-game template`
- present but malformed → `220 SaveLoad: cannot read <path> (<error>); using a synthesized new-game template`

In both cases the synthesized template carries `Databases::template_vardb()`, so
runtime CData cells are storable from the first save. The note goes into
`Game::diagnostics` with hit count 1 — **not** through `Host::log`, so
`GameStats::warnings` is untouched.

A `System.sav` that *parses* but has an empty variable database (the file written by
the first W4-b build has `type_count = 0`) gets the same static schema filled in via
`SaveFile::set_vardb`, with one diagnostic. Without this an old synthesized
`System.sav` would keep silently dropping every database cell. A retail save has 30
types and is not touched: the retail path stays byte-identical.

`base_save` is always `Some` after `Game::new`, and a root that has a valid retail
`System.sav` still uses it unchanged.

## 4. Acceptance tests

`crates/wolf-game/tests/m8_template_save.rs::a_data_root_without_system_sav_saves_and_reloads`

- root: `WOLF_DATA_ROOT` when set, else `<repo>/work/all`; skips when
  `BasicData/CommonEvent.dat` is absent.
- `Game::new(root)` → `base_save.is_some()`; when `Save/System.sav` is absent, the
  synthesized-template diagnostic is asserted present.
- starts the engine events, runs **5 frames** (play time 5).
- writes the 800 normal variables (mixed signs/range), Sys3/Sys4 = 700/355, and
  three live pictures (Text / spritesheet File / WindowFile).
- `Game::save_to(<repo>/work/out/m8_template_save_<pid>/new_game.sav)` succeeds (the
  old code failed with "no template save").
- reads the file back with `savegame::read_save`: play time, anchor, all 800
  normal variables and the 3 picture records are in the file.
- mutates the live game, then a **fresh** `Game::new(root)` +
  `savegame::load_into` restores: `LoadedSave { pictures: 3, normal_vars: 800,
  playtime_frames: 5, choice_anchor: (700, 355) }`, the 800-variable snapshot, the
  picture snapshot, and play time.
- asserts no diagnostic in either game contains `"no template save"` and
  `stats.warnings == 0` in both.

`crates/wolf-game/tests/m7_save_parts.rs::every_save_part_a_played_game_writes_round_trips`
is the database-cell witness. On a root without `System.sav` it used to fail at
`CData (0,0,5)` (the write was counted as unmodelled and the reload read the static
`CDataBase` value 1 instead of 4242). It now writes CData type 0 row 0 column 5
(numeric = 4242) and column 0 (text `テスト名前`), saves, reloads and asserts both
cells come back. The same test still passes on `work/full` (retail `System.sav`),
where it always passed.

`crates/wolf-game/src/db.rs::the_static_cdata_schema_becomes_a_save_variable_database`
checks the export directly: 30 types, `config` equal to the field table's raw words,
`dis == data_id_method`, every row positional and fully populated, the static
`勇者先生` string body carries its NUL, and `set_num` accepts a static row while
refusing one past the table. It also drives a `250` row-count write over the
template database: growing by one appends a storable row and shrinking to 2
truncates the tail, with `write_into` reporting no missed cells in both cases.

## 5. Commands and observed results

```
cargo test -q -p wolf-save
# 3 + 8 + 1 + 6 + 6 passed, 0 failed (retail tests skip without WOLF_GAME_DIR)

WOLF_GAME_DIR='E:/Study/github program/imoto life/work/retail_backup' \
  cargo test -q -p wolf-save -- --nocapture
# checked 9 save file(s); round-tripped 9 save file(s)
# tests/save.rs, rebuild.rs (including every_retail_save_rebuilds_to_the_same_bytes),
# typed.rs, write.rs all green -> every retail save still round-trips byte-for-byte.

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/all' \
WOLF_RETAIL_SAVE_DIR='E:/Study/github program/imoto life/work/retail_backup/Save' \
  cargo test -q -p wolf-game --test m7_save_parts -- --nocapture
# round-tripped 8 retail save(s); 8 had a Part4 distinct from System.sav
# test result: ok. 2 passed (including the CData (0,0,5)=4242 round-trip)

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/all' \
  cargo test -q -p wolf-game --test m8_template_save -- --nocapture
# test result: ok. 1 passed (the root under test carries a stale synthesized
#   System.sav with type_count 0; the enrichment branch fills it in)

# The same two tests with System.sav moved aside, so the absent-file fallback is
# the path under test:
#   mv work/all/Save/system.sav ...; run both; mv it back
# m8: ok. 1 passed; m7: ok. 2 passed

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/all' cargo test -q -p wolf-game --lib
# 231 passed; 0 failed

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/full' \
WOLF_RETAIL_SAVE_DIR='E:/Study/github program/imoto life/work/retail_backup/Save' \
  cargo test -q -p wolf-game --test m7_save_parts -- --nocapture
# round-tripped 8 retail save(s); the retail-template path is unchanged
# test result: ok. 2 passed
```

## 6. Format caveats (deliberate, documented)

1. **The template variable database carries CData only.** A synthesized template now
   has the static `CDataBase` schema and values, so runtime `CData` cells are stored
   and `stats.unmodelled` stays 0 for them. `SysDatabase` and `DataBase` cells are
   still excluded by design: a save's variable database is the player's 可変DB, while
   SysDatabase is engine configuration and DataBase is the editor's fixed table (§9;
   `Databases::write_into` skips both). A write the static table has no row for -- for
   example an extension row `100 + n` that no script grew through a row-count write --
   still counts as unmodelled, exactly as it did with a retail template.
2. **Rows beyond the static height need a row-count write.** `set_row_count` (the
   `250` write to row `-2`) still grows/shrinks the template database through
   `reconcile_row_counts`; a script that writes row 100 without resizing first is
   refused. Retail saves behave the same way.
3. **Part1 has no map snapshot and no objects; Part4 has no objects.** The sentinel
   is the format's explicit "no snapshot" marker, and the port's `load_into`
   ignores Part1/Part4 (they are template-carried), so the port save/load loop is
   unaffected. An engine that restores map/object state from Part1 would have to
   rebuild it from the map data rather than from this template.
4. **Part2 defaults are zero** except the fields `save_to` writes (play time,
   Sys3/Sys4). Only system-variable snapshots (`vars3_21[6..18]`) read back as 0
   from saves written on this path.
5. **550 event groups.** The retail saves use 500 or 550; 550 is proven loadable
   by `SaveData05/08` and gives every common event of this build a CSelf slot.
   A save written here is ~239 KB in Part3, close to the retail 218–240 KB range.
6. **Size and cost.** The database adds ~82 KB to the template (retail vardb segment
   is 81,842–81,851 bytes; this build's static export is the same shape). `Game::new`
   builds and encodes it once per construction on a root without a usable
   `System.sav`; the work is a linear walk of 30 types / ~700 rows and is dwarfed by
   the per-frame VM and asset work. `save_to` already cloned `base_save`, so the
   per-save copy grows by the same ~82 KB.
7. **Cosmetics.** The name `"New Game"`, the three header seeds are 0 and the
   unused header bytes are 0. The cipher is address/seed based and stores its
   seeds in the plaintext header, so this is valid; it is not byte-equal to any
   retail save by design. `m_dis` is filled from `data_id_method`, which matches
   every retail type checked (see §2.1).

## 7. Scope

Changed only: `crates/wolf-save/src/lib.rs`,
`crates/wolf-game/src/db.rs` (`Databases::template_vardb` and its test),
`crates/wolf-game/src/game.rs` (the `base_save` initializer),
this document. No battle/H/map files, no save-format change for existing files,
no commit/push.