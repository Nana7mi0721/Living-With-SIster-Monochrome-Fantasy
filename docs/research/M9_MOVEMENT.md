# M9 W3-b - headless player movement and passability core

Status: implemented. `MapRuntime` now carries a directional tile-passability grid
and exposes one-tile cardinal movement; `Game` installs the shipped tileset grid on
every real map switch and forwards two public movement methods. Proof is
`crates/wolf-game/tests/m8_movement.rs` (6 tests) plus the untouched
`m8_character_state` regression (3 tests).

Files changed (task-52 write scope only):

- `crates/wolf-game/src/map_runtime.rs` - `Terrain`, map bounds, movement core.
- `crates/wolf-game/src/game.rs` - terrain installation and public wrappers.
- `crates/wolf-game/tests/m8_movement.rs` (new) - movement, bounds, walls, events.
- `docs/research/M9_MOVEMENT.md` (this file).

No commit, no push. `crates/wolf-vm/src/rng.rs` was already modified by another
teammate and was left untouched.

## 1. What the shipped data actually carries

### 1.1 Tile passability is in TileSetData, not in the map

`crates/wolf-format/src/tileset.rs:111` defines `TileSet` and line 125 stores
`pub passability: Vec<TilePassability>`. `TileSetData::parse`
(`crates/wolf-format/src/tileset.rs:52`) reads, per tileset, the two names, the 31
autotile names, `0xFF`, a `u32` tag count + tag bytes, `0xFF`, then a `u32`
passability count and that many little-endian `u32` passability words.

`crates/wolf-format/src/tileset.rs:180` `TilePassability` decodes the bitfield in
`docs/FORMAT_SPEC.md:1353` section 7.3:

    bit 0      counter_enabled
    bit 1      square
    bit 2      quarter_tile (bits 4..7 are corner passability)
    bit 3      star
    bits 4..7  if quarter_tile: top_left/top_right/bottom_left/bottom_right passable
               else:             upwards/rightwards/leftwards/downwards NOT passable
    bits 8..13 reserved (bits 8..9 carry real values; 10..13 are zero)
    bit 14     downarrow
    bit 15     triangle
    upper u16  always zero

`TilePassability::blocked(direction)` (`crates/wolf-format/src/tileset.rs:253`) maps
0 up, 1 right, 2 left, 3 down; anything else answers `None`.

The six shipped tilesets and their passability counts (`docs/FORMAT_SPEC.md:1379`
section 7.4) are exactly:

    index 0  2376   index 1  2376   index 2  2376
    index 3   216   index 4  2376   index 5  1648

SampleMapA, Map002, Map003, Map024, Map031 and TitleMap use tileset 4
(kiri_project.png); Dungeon and SampleMapB use tileset 0. These counts were
re-verified directly against `work/all/BasicData/TileSetData.dat` (version 210).

### 1.2 Tile word -> passability index

A map tile word is not a direct passability index. The measured rule
(`work/_yado_0600.txt:535`: chip numbers are numbered from the tileset's top-left;
chip 0 transparent, 1..31 autotile, 32+ base chip) is:

    autotile id = raw / 100_000
    chip id     = autotile id when > 0, otherwise raw itself
    passability = tileset.passability[chip id]

Example: the ubiquitous SampleMapA word `104444` is autotile id 1 with corner
digits 4444, so it looks up passability[1] = 0 = fully passable. A raw base chip
such as 464 is looked up directly. `Terrain::from_map` implements exactly this
(`crates/wolf-game/src/map_runtime.rs:213`).

All three tile layers contribute. The implementation ORs their direction masks
into one mask per cell. A quarter tile is reduced to whole-tile sides by requiring
BOTH corners on a side to be non-passable; this is a documented approximation, not
the retail sub-tile walk.

### 1.3 Event blocking data

`crates/wolf-format/src/mps.rs:331` defines `MapEventPage`; lines 365-367 carry
`collision_width` and `collision_height` (bytes, parsed at 415-416). The editor
description calls them a collision-range extension in x/y, range 0-250.

Census over all 134 shipped event pages (all maps):

    130 pages: (collision_width, collision_height) = (0, 0)
      1 page : (3, 0)    Dungeon.mps event 0, page trigger 3 (player touch)
      1 page : (33, 22)  Dungeon.mps event 2, page trigger 3
      1 page : (8, 6)    SampleMapB.mps event 0, page trigger 3
      1 page : (4, 0)    SampleMapB.mps event 2, page trigger 3

Every non-zero box is on a trigger-3 touch page; every trigger page the leader can
walk into in the normal maps has (0,0). No format note or source gives the exact
unit/origin semantics of these extensions, so the movement core blocks an event's
own tile only. This is explicit residual uncertainty, not silent support.

There is no per-map player start coordinate. `docs/MAP_AUDIT.md:359` section 7.5
records that MapTree has no start x/y and the game is point-and-click; the leader
position is engine state. So movement starts from wherever `place_leader` or a
130 teleport put the leader.

## 2. Implemented API

### 2.1 `Terrain` (crates/wolf-game/src/map_runtime.rs:182)

- `pub width: i32`, `pub height: i32`; private `blocked: Vec<u8>`, one 4-bit
  direction mask per cell in `TilePassability::blocked` order (bit0 up, bit1
  right, bit2 left, bit3 down; set means entering the cell in that direction is
  blocked).
- `pub fn new(width, height) -> Self` - all-passable grid.
- `pub fn from_map(map: &Map, tileset: &TileSet) -> Self` (:213) - decodes every
  layer as in section 1.2.
- `pub fn is_blocked(&self, x, y, step) -> bool` (:278).
- `pub fn set_blocked(&mut self, x, y, step, blocked)` (:295) and
  `pub fn set_tile_blocked(&mut self, x, y, blocked)` (:317) - test/extension
  helpers.

### 2.2 `MapRuntime`

New fields: `width: i32`, `height: i32`, `terrain: Option<Terrain>`. The map
dimensions come from `Map.width/height` in `collect_except`; `set_terrain` adopts
the grid's size as the authoritative bounds.

- `pub fn map_size(&self) -> (i32, i32)` (:497)
- `pub fn set_terrain(&mut self, terrain: Terrain)` (:506)
- `pub fn terrain(&self) -> Option<&Terrain>` (:516)
- `pub fn try_move_leader(&mut self, direction: i32) -> bool` (:526) - WOLF facing
  2 down, 4 left, 6 right, 8 up.
- `pub fn move_leader(&mut self, dx: i32, dy: i32) -> bool` (:543) - exactly one
  non-zero axis; derives facing 2/4/6/8.
- private `move_leader_step` (:550); `event_occupies` (:587);
  `facing_step` (:782), `step_facing` (:793), `delta_step` (:803).

Movement semantics, deliberately minimal:

1. The leader turns first even if the step is blocked, matching the engine rule
   that a direction press turns the character before the collision test.
2. A step is rejected for: target outside `[0,width)x[0,height)`; tile passability
   for that incoming direction; any map event with an active page standing on the
   target tile. No leader => no move.
3. On success the tile x/y changes by one and `precise_x/precise_y` change by 2,
   so an odd half-tile origin is preserved instead of snapped.
4. Diagonals, zero deltas and non-facing direction codes are rejected without
   moving or turning.
5. No diagonal, continuous/half-tile walk, route/scripted movement, NPC movement or
   rendering is added.

### 2.3 `Game` wrappers (crates/wolf-game/src/game.rs)

- Import at line 36: `use crate::map_runtime::{MapRuntime, Terrain};`.
- `start_pages` (:683) now keeps the resolved tileset, builds `TileMap` from
  `tileset + version`, then after `MapRuntime::collect_except` calls
  `self.runtime.set_terrain(Terrain::from_map(map, tileset));` (:693). Every real
  map switch therefore installs real tile passability.
- `pub fn try_move_leader(&mut self, direction: i32) -> bool` (:721)
- `pub fn move_leader(&mut self, dx: i32, dy: i32) -> bool` (:727)

Both are `#[must_use]` and simply delegate to the runtime.

## 3. Real shipped maps: which ones can block at all

Applying the section 1.2 rule to every shipped map (blocked cell = at least one
blocked entry direction; `work/all/BasicData/TileSetData.dat` plus
`work/all/MapData/*.mps`):

    map          tileset  size      blocked cells  chips actually used
    Dungeon      0        40 x 30   348            1,7,16,31,464,465,466,472,473,474
    SampleMapB   0        30 x 30   160            22 chips incl 0,1,2,5,8..11,16..19,31,41,43
    Map002       4       100 x 30     0            1,31
    Map003       4        70 x170     0            1,2,31
    SampleMapA   4        64 x 36     0            1,31
    SampleMapA_1 4        64 x 36     0            1,31
    TitleMap     4        53 x 30     0            1,31
    Map024       -        no tile data
    Map031       -        no tile data

Consequence: the story maps (bedroom SampleMapA, town Map003, Map002) are fully
passable by tile data, so a real tile-wall test is impossible there. The only real
walls are on Dungeon (348 cells) and SampleMapB (160 cells). The test suite uses
Dungeon for the real-data wall plus a synthetic grid for an isolated wall.

## 4. Supported vs missing

Supported and proven:

- One-tile cardinal movement by facing code or by `(dx,dy)`.
- Map-boundary blocking; a blocked step still updates facing.
- Real tile passability from shipped `TileSetData`, all three layers, full-tile
  directional bits and a whole-tile reduction of quarter tiles.
- Active map-event tile occupancy as an obstacle.
- Leader precise coordinates advance by whole tiles, preserving half-tile origins.
- `place_leader`/`place_leader_precise` behaviour unchanged.

Not implemented, with reasons:

- Diagonal or free (sub-tile) movement: only four cardinal one-tile steps exist.
- Event collision_width/height extension: four touch pages carry non-zero boxes
  (section 1.3) but the unit/origin is undocumented; only the event's own tile is
  blocked.
- Page/collision refresh during a step, moving NPCs, route commands, and scripted
  event blocking: the grid is built once at map entry from map data and active
  pages; no per-frame event-footprint update.
- Rendering, input mapping, animation and map transfer: this is a headless core.
- No player start data exists; callers must place the leader first.
- Tile bits 0/1/3 (counter/square/star), bit14/15 (downarrow/triangle) and the
  reserved `na` bits are ignored. Only directional passability is consumed.
- Quarter-tile corners are approximated by requiring both corners on a side; the
  retail engine's sub-tile walk is not reproduced.

## 5. Tests and commands

Test file: `crates/wolf-game/tests/m8_movement.rs`. Helpers mirror
`m8_character_state.rs` data-root resolution (`WOLF_DATA_ROOT` else `work/all`
else `work/full`) and skip cleanly when data is absent.

Tests:

1. `leader_steps_one_tile_and_keeps_precise_coordinates_exact` - real SampleMapA:
   place (30,20) facing down; right -> (31,20), precise (62,40), facing 6;
   down -> (31,21), precise (62,42), facing 2; diagonal 1 rejected.
2. `map_edge_and_event_occupancy_block_the_step` - (0,0) left/up blocked but facing
   becomes 4 then 8; (62,35) right blocked by event 13 at (63,35), position stays,
   facing 6.
3. `a_solid_tile_blocks_the_step` - synthetic 4x4 terrain, blocked (2,1): right
   rejected at (1,1); down then right succeed.
4. `an_active_map_event_blocks_its_tile` - synthetic map, active event at (2,1):
   right rejected, down allowed.
5. `place_leader_is_unchanged_and_precise_origins_move_by_whole_tiles` -
   place_leader (11,12) facing 8 => precise (22,24); place_leader_precise (63,39)
   facing 2 => tile (31,19); right -> (32,19) precise (65,39); down -> (32,20)
   precise (65,41).
6. `the_real_tileset_passability_grid_marks_dungeon_walls_and_samplemapa_nothing` -
   loads Dungeon and SampleMapA with the real `TileSetData`, asserts Dungeon is
   40x30 with exactly 348 blocked cells and SampleMapA has 0; then finds the first
   approachable blocked Dungeon cell and asserts movement into it fails and stays.

Commands and observed results (run from the repo root, data root `work/all`):

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_movement -- --nocapture
    running 6 tests
    test result: ok. 6 passed; 0 failed; 0 ignored; ... finished in 0.46s

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_character_state -- --nocapture
    running 3 tests
    test result: ok. 3 passed; 0 failed; 0 ignored; ... finished in 0.44s

    cargo test -q -p wolf-game --lib
    running 231 tests
    test result: ok. 231 passed; 0 failed; ... finished in 3.96s

    cargo check -q -p wolf-game
    exit 0 (only pre-existing dead_code warnings in battle_wiring.rs)

The one warning seen is the existing `BattleWiring::set_round_loop`,
`set_command_schedule`, `turn_starts` dead-code warning in
`crates/wolf-game/src/battle_wiring.rs`, unrelated to this task.

## 6. Dead ends and residual uncertainty

- Looking for a wall in SampleMapA/Map003/Map002: dead end. Every cell is chip 1,
  2 or 31, all passability 0. The real map has no collision to demonstrate.
- Using event collision boxes by default: dead end without unit semantics. 130 of
  134 pages are 0x0; the four non-zero pages are touch pages. Blocking the event's
  own tile is the only defensible behaviour now.
- Expecting a map-level start coordinate: none exists (`docs/MAP_AUDIT.md:359`),
  so an unplaced leader cannot move.
- Reading passability from the map file alone: impossible. It lives in
  `TileSetData`, keyed by chip id, not by the raw 32-bit tile word.
