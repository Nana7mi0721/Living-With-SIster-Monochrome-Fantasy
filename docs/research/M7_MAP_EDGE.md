# M7 W4 — Map-graph edge cases

Status: measured (HEAD `1d36ebe`, data root `work/all` / `work/full`).
Owner: m6-night. Scope: read-only except `docs/research/M7_MAP_EDGE.md`,
`crates/wolf-game/tests/m7_map_edge.rs` and a doc comment in
`crates/wolf-game/src/tilemap.rs`. No commit.

This note closes the three open items in `docs/MAP_AUDIT.md` §9 that the generic
audit could not decide from bytes alone:

1. does `130` with a literal `-2` mean "map id -2"?
2. do the tile-less maps `Map024`/`Map031` render black or keep the previous frame?
3. is `MapTree.dat` the new-game entry point, and where are the start coordinates?

Short answers: **(1)** no, the destination map id is `0` (TitleMap); `-2` is the
move *target*. **(2)** they paint no floor; a cold capture is fully black.
**(3)** `MapTree.dat` only orders maps (root children 3/31/24) and the engine does
not read it; the real new-game start is `Map003` event 1 → teleport to
SampleMapA at (62, 38).

## Reproduction

```
cd 'E:/Study/github program/imoto life'
source tools/env.sh
# this investigation's tests:
cargo test -q -p wolf-game --test m7_map_edge -- --nocapture
# captures (the --map value is relative to --root, so it needs the MapData/ prefix):
cargo run -q -p wolf-cli -- --root work/all capture --map MapData/Map024.mps --out work/out/m7_map024.png --frames 60
cargo run -q -p wolf-cli -- --root work/all capture --map MapData/Map031.mps --out work/out/m7_map031.png --frames 60
```

The temporary `--example tmp_map002` probe that first dumped these maps was removed after
writing this report; every assertion it produced now lives in `m7_map_edge.rs`.

Observed test output:

```
running 3 tests
Map002 teleport diagnostic: 130 Teleport: MapData/TitleMap.mps at (2000007, 1100005) target -2 options 0x1
Map003 new-game teleport diagnostic: 130 Teleport: MapData/SampleMapA.mps at (62, 38) target -1 options 0x1
test result: ok. 3 passed; 0 failed; ...
```

## 1. Map002 `130` — `-2` is the target, destination is map id 0

`130`'s operand order is `[target, x, y, map, options]`
(`crates/wolf-vm/src/exec.rs:1792-1800` calls `host.teleport(arg0, arg3, arg1, arg2, arg4)`;
`crates/wolf-game/src/game.rs:1728-1736` pushes `MapRequest{target, map, x, y, options}`).
`Map002.mps` contains exactly four teleports, all with the identical raw args
`[-2, 2000007, 1100005, 0, 1]`, at event 3 page 0 pc 7 and pc 10, and event 5
page 0 pc 7 and pc 10 (matches `crates/wolf-format/tests/map_audit.rs:539-542`):

* `target = -2` — the player/hero, **not** the destination map.
* `map = arg3 = 0` — `SysDatabase` type 0 row 0 = `MapData/TitleMap.mps`
  (`crates/wolf-format/src/sysdatabase.rs:399-434`; registered rows are
  `0=TitleMap, 1=SampleMapA, 2=SampleMapA_1, 3=Map003, 24=Map024, 31=Map031`).
* `x = 2000007` → `Magic::decode` = `MagicKind::Var` index **7** (global var 7).
* `y = 1100005` → `Magic::decode` = `MagicKind::SelfVar` index **105**
  (self group 1 slot 5, i.e. the value `self[105] = var8 - 4` computed just above
  at pc 2).

So **`MAP_AUDIT.md` §9 #2 ("130 的 -2 目的地") is a misparse**: `-2` is the
target operand; the destination is an ordinary map id `0`. Runtime confirms it:
starting `Map002` event 3 raises exactly
`130 Teleport: MapData/TitleMap.mps at (2000007, 1100005) target -2 options 0x1`.

`crates/wolf-game/src/db.rs:392 map_path(id)` rejects negatives
(`usize::try_from(id)`), so `map_path(-2) == None`; its doc comment claiming
"-1 is map 1, -2 is map 0" is stale and dead — the shipped data never stores a
negative map id.

Two further engine facts the diagnostic exposes (both worth knowing):

* The coordinates are logged **verbatim as magic operands** `(2000007, 1100005)`:
  `crates/wolf-game/src/game.rs:1100-1120` only uses `request.map` (for `switch_map`)
  and formats `request.x/request.y/request.target/request.options` straight into the
  diagnostic. It never resolves the magic numbers, and `switch_map` takes no position.
  So this engine performs a **map switch only** — it does not place the player at the
  decoded variable coordinates. The destination map's own events (here TitleMap event 2)
  decide everything else.
* Because the x/y are variable-typed, the on-disk "-2 destination" reading in the audit
  is the only reason this looked exotic; it is a plain map switch at runtime.

## 2. Map024 / Map031 — no tile layer, so no floor is painted

Both maps ship the `0xFFFFFFFF` "no tile block" marker: `has_tile_data == false`,
`layers` is empty, `layer_count() == 0`, `Map::tile()` returns `None`
(`crates/wolf-format/src/mps.rs`; asserted for both maps in
`crates/wolf-format/tests/map_audit.rs:191-198`). Height/event facts:

| map | size | tileset | has_tile | layers | events |
|-----|------|---------|----------|--------|--------|
| `Map024.mps` | 53x30 | 4 | false | 0 | 1 — `画像消去` (image erase), 3 cmds, no teleport |
| `Map031.mps` | 53x30 | 0 | false | 0 | 1 — `ロード時の自動処理` (on-load), 63 cmds, one teleport |

`Game::start_pages` still builds a `TileMap` (the tilesets exist), but
`TileMap::layer_count()` is 0 and `TileMap::quads()` yields no quads
(`crates/wolf-game/src/tilemap.rs:91-107`). The render path has **no black fallback
fill**; `switch_map` (`crates/wolf-game/src/game.rs:663-673`) only clears negative
screen layers. Therefore a tile-less map shows whatever picture layers survive from
before, and a cold capture (renderer clears to black, no prior pictures) is fully black.

Measured captures (`work/out/`, both 1024x576):

```
work/out/m7_map024.png   nonblack_px=0 (0.000%)  distinct_colors=1  bg=(0,0,0)
work/out/m7_map031.png   nonblack_px=0 (0.000%)  distinct_colors=1  bg=(0,0,0)
```

Verdict: tile-less maps render **black from a cold start / nothing of their own**;
they do not synthesize a floor and (in this engine) do not restore a saved background.
The phrase "preserves background" is only true to the extent that prior positive
picture layers are left in place — the map itself contributes zero pixels.

Extra edge found while capturing: `Map031`'s on-load event teleports to map id **27**
(`op130 [-1, 22, 18, 27, 16]`, `crates/wolf-format/tests/map_audit.rs:320`). Row 27 of
`SysDatabase` type 0 is not one of the six registered maps and its path text is empty,
so `map_path(27)` yields an empty string and the load fails with the observed
diagnostic `130 Teleport: cannot read work/all\: ... (os error 3)`. This is a dangling
destination in the shipped data, not a decoder bug.

## 3. MapTree.dat — new-game ordering, and where the start really is

`BasicData/MapTree.dat` parses to 50 entries
(`crates/wolf-format/src/maptree.rs`; `MapTreeEntry{parent:i32, map_id:u32}`).
The root is `parent == -1`; `children_of(0)` = `[3, 31, 24]` (also documented in
`docs/MAP_AUDIT.md` §9). The file carries **only parent/map_id** — there is no
x/y start coordinate anywhere in it.

The engine does **not** consume `MapTree.dat`: a grep of `crates/wolf-game` finds zero
references to `MapTree`. Game boot is fixed to `BOOT_MAP = "MapData/TitleMap.mps"`,
`BOOT_EVENT = 2` (splashes → title); there is no new-game / MapTree-driven path yet.

**Blocker (explicit):** "MapTree as new-game entry" is not implementable from this file
alone — the ordering exists but the coordinates do not, and no engine hook reads it.

**The real new-game start**, verified at runtime and usable as the hook target:
map id **3 = `Map003.mps`**, event 1 `ニューゲーム処理` (autorun, 43 cmds) ends at pc 28
with `op130 [-1, 62, 38, 1, 1]` — target `-1` (hero), `x=62`, `y=38`, map id `1` =
`SampleMapA.mps`, options `1`. Starting `Map003` event 1 raises
`130 Teleport: MapData/SampleMapA.mps at (62, 38) target -1 options 0x1`. So a new
game = "load map id 3, run its event 1", which lands the player in SampleMapA at (62, 38).

## 4. Dead ends / corrections recorded

* `MAP_AUDIT.md` §9 #2 "130 的 -2 目的地" — **wrong**: `-2` is target, destination map
  id is `0` (TitleMap). Corrected here and pinned by `m7_map_edge.rs`.
* `db.rs:386` `map_path` doc comment ("-1 is map 1, -2 is map 0") — stale; negatives are
  rejected and never appear as map ids in the data.
* `MapTree.dat` new-game entry — **blocked**: no coordinate field and no engine reader.
* `Map031` load-time teleport to map id 27 — **dangling**: empty `SysDatabase` row.
  **Fixed in M8 W4-c**: `db::map_path` treats an empty path cell as unregistered, so the
  teleport is skipped with one `130 Teleport: no map registered for id 27` diagnostic
  instead of a failed read of the data root; pinned by
  `m7_map_edge::a_dangling_map_id_is_skipped_without_a_read_error`.
* `switch_map` / `130` do **not** position the player: coordinates are variable-typed
  magic operands that the host logs but never resolves; only the map switch happens.

## 5. Test + evidence

`crates/wolf-game/tests/m7_map_edge.rs` (4 tests, all green):

* `map002_teleport_uses_an_ordinary_map_id_and_variable_coordinates` — pins the four
  sites, checks `map_path(0)==Some("MapData/TitleMap.mps")`, `map_path(-2)==None`, and
  runs `Map002` event 3 until the `130 Teleport: MapData/TitleMap.mps ...` diagnostic.
* `map024_and_map031_have_no_tile_layer_so_they_draw_no_tile_quads` — `has_tile_data`
  false, `layers` empty, `tilemap.layer_count()==0`, `tile_quads` empty (no frames
  stepped, so Map031's dangling teleport cannot interfere).
* `maptree_names_the_new_game_hook_and_map003_is_the_real_start_coordinate` — MapTree
  has 50 entries and root children 3/31/24; `Map003` event 1 is the single teleport
  `[-1,62,38,1,1]`; runtime reaches `130 Teleport: MapData/SampleMapA.mps ...`.
* `a_dangling_map_id_is_skipped_without_a_read_error` — starts Map031's real load event,
  sees `130 Teleport: no map registered for id 27`, no "cannot read" diagnostic, 0
  warnings.

Screens: `work/out/m7_map024.png`, `work/out/m7_map031.png` (both black, 0 non-black px).
