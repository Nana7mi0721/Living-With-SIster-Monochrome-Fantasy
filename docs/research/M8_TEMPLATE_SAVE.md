# M8 W4-b — a data root without `Save/System.sav` is now saveable

Status: **DONE** (acceptance test green, all existing save tests still green).
Worktree branch `worktree-agent-aba50c33a47294a07`, HEAD `5e996f0` (local master).
Deliverables:

- `crates/wolf-save/src/lib.rs` — `SaveFile::new_game_template()` plus two unit tests.
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
| VariableDatabase | `unknown = 0x6F`, `type_count = 0`, no types/rows |
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

## 3. `Game::new` fallback (wolf-game)

Only the `base_save` initializer changed. When `Save/System.sav` cannot be read:

- absent → `220 SaveLoad: <root>/Save/System.sav is absent; using a synthesized new-game template`
- present but malformed → `220 SaveLoad: cannot read <path> (<error>); using a synthesized new-game template`

The note goes into `Game::diagnostics` with hit count 1 — **not** through
`Host::log`, so `GameStats::warnings` is untouched. `base_save` is always `Some`
after `Game::new`, and a root that has a valid `System.sav` still uses it unchanged
(the retail path is byte-identical).

## 4. Acceptance test

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

## 5. Commands and observed results

```
cargo test -q -p wolf-save
# 2 + 8 + 1 + 6 + 6 passed, 0 failed

WOLF_GAME_DIR='E:/Study/github program/imoto life/work/retail_backup' cargo test -q -p wolf-save -- --nocapture
# checked 9 save file(s); round-tripped 9 save file(s)
# tests/save.rs, rebuild.rs (including every_retail_save_rebuilds_to_the_same_bytes),
# typed.rs, write.rs all green -> every retail save still round-trips byte-for-byte.

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/all' \
  cargo test -q -p wolf-game --test m8_template_save -- --nocapture
# running 1 test ... test result: ok. 1 passed; 0 failed; 0 ignored

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/full' \
WOLF_RETAIL_SAVE_DIR='E:/Study/github program/imoto life/work/retail_backup/Save' \
  cargo test -q -p wolf-game --test m7_save_parts -- --nocapture
# round-tripped 8 retail save(s); 6 had a Part4 distinct from System.sav
# test result: ok. 2 passed

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/full' cargo test -q -p wolf-game --lib
# 225 passed; 0 failed

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/full' cargo test -q -p wolf-game --test m6_mainline
# 7 passed; 0 failed
```

## 6. Format caveats (deliberate, documented)

1. **The synthesized variable database is empty.** `save_to` still writes a valid
   file, but runtime `CData` cells cannot land in a database with no types: it
   records `220 SaveLoad: N database cell(s) written, M cell(s) could not be
   encoded` and increments `stats.unmodelled` (a diagnostic, not a warning). A
   data root with the retail `System.sav` keeps that file's database and does not
   hit this. Character creation/database-driven state therefore does not persist
   on a root that ships no template until a real database is supplied.
2. **Part1 has no map snapshot and no objects; Part4 has no objects.** The sentinel
   is the format's explicit "no snapshot" marker, and the port's `load_into`
   ignores Part1/Part4 (they are template-carried), so the port save/load loop is
   unaffected. An engine that restores map/object state from Part1 would have to
   rebuild it from the map data rather than from this template.
3. **Part2 defaults are zero** except the fields `save_to` writes (play time,
   Sys3/Sys4). Only system-variable snapshots (`vars3_21[6..18]`) read back as 0
   from saves written on this path.
4. **550 event groups.** The retail saves use 500 or 550; 550 is proven loadable
   by `SaveData05/08` and gives every common event of this build a CSelf slot.
   A save written here is ~239 KB in Part3, close to the retail 218–240 KB range.
5. **Cosmetics.** The name is `"New Game"`, the three header seeds are 0 and the
   unused header bytes are 0. The cipher is address/seed based and stores its
   seeds in the plaintext header, so this is valid; it is not byte-equal to any
   retail save by design.

## 7. Scope

Changed only: `crates/wolf-save/src/lib.rs`,
`crates/wolf-game/src/game.rs` (the `base_save` initializer),
`crates/wolf-game/tests/m8_template_save.rs`, this document. No battle/H/map
files, no save-format change for existing files, no commit/push.