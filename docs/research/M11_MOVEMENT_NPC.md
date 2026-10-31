# M11 P1-4: diagonal movement and NPC/contact queries

Repo: E:/Study/github program/imoto life
HEAD at slice: 71803f9
Scope: crates/wolf-game/src/map_runtime.rs, crates/wolf-game/tests/m11_movement_npc.rs, this file.
game.rs was NOT changed; MapRuntime already owns the movement layer.

## Result

* Added eight-direction single-tile movement to MapRuntime:
  try_move_leader_8, move_leader_8, and the shared move_leader_delta.
* The historical cardinal entry points try_move_leader/move_leader are
  untouched, so m8_movement keeps its four-direction contract.
* A diagonal checks both component entries of the destination tile, both
  orthogonal corner cells, the destination event occupancy, and both orthogonal
  event occupancies. A blocked diagonal still turns the leader.
* Added try_move_leader_contacts_8 so a diagonal step reports the same
  player-touch/event-touch requests as a cardinal step, using the attempted
  destination.
* Added NPC/contact queries: events_at, event_at, npc_at, contact_events_at.
* Tests: m11_movement_npc 7 passed; m8_movement 6 passed; m8_map_contacts 6
  passed; m8_map_triggers 5 passed; cargo check -q -p wolf-game exit 0.

## WOLF facing codes

WOLF uses numeric-keypad directions:

| code | direction | delta (dx, dy) |
|------|-----------|----------------|
| 1 | down-left | (-1, +1) |
| 2 | down | (0, +1) |
| 3 | down-right | (+1, +1) |
| 4 | left | (-1, 0) |
| 6 | right | (+1, 0) |
| 7 | up-left | (-1, -1) |
| 8 | up | (0, -1) |
| 9 | up-right | (+1, -1) |

Evidence: work/_yado_0600.txt:380-384 states the character movement-direction
mask as [1 up + 2 left + 4 right + 8 down + 16 up-left + 32 up-right + 64
down-left + 128 down-right]. The engine therefore has eight movement
directions. The game's basic setting can restrict the player to four directions
(work/_yado_0600.txt:353), but the MapRuntime primitive intentionally supports
the full set so a caller can choose.

## Exact APIs

All in crates/wolf-game/src/map_runtime.rs:

| API | line | behaviour |
|-----|------|-----------|
| pub fn try_move_leader_8(&mut self, direction: i32) -> bool | 765 | One-tile step in any WOLF code 1/2/3/4/6/7/8/9. Returns true only if the leader moved. A blocked step still updates facing. Unknown codes return false without moving or turning. |
| pub fn move_leader_8(&mut self, dx: i32, dy: i32) -> bool | 777 | Axis form. dx and dy must each be -1, 0 or 1 and not both zero; both non-zero is a diagonal. |
| fn move_leader_delta(&mut self, dx, dy, direction) -> bool | 784 | Shared implementation. Turns first, then tests bounds, terrain, and event occupancy. On success tile x/y move one and precise_x/y move dx*2/dy*2 (WOLF half-tile unit). |
| fn diagonal_terrain_blocked(&self, from_x, from_y, to_x, to_y, dx, dy) -> bool | 824 | Destination must be enterable along both component axes. For a diagonal, both orthogonal neighbours must also be enterable along the matching axis, preventing corner cutting. |
| pub fn try_move_leader_contacts_8(&mut self, direction: i32) -> (bool, Vec<TriggerRequest>) | 999 | Diagonal-aware contact wrapper. Computes the attempted destination, performs the move, and returns try_contact_triggers(from, attempted_to). |
| pub fn events_at(&self, x: i32, y: i32) -> Vec<u32> | 1019 | Every map event whose own placement tile is (x,y), ascending event id (the runtime form of the manual's event-ID-at-tile query). |
| pub fn event_at(&self, x: i32, y: i32) -> Option<u32> | 1032 | Smallest event id on (x,y), matching WOLF's overlap rule. |
| pub fn npc_at(&self, x: i32, y: i32) -> Option<u32> | 1042 | Smallest active image-bearing event on (x,y); blank trigger volumes are skipped. |
| pub fn contact_events_at(&self, x: i32, y: i32) -> Vec<u32> | 1062 | Every active event whose contact box (base hit box + collisionWidth/Height extension, see M9_MAP_CONTACTS.md) includes (x,y), ascending id. |
| fn facing_delta(direction: i32) -> Option<(i32,i32)> | 1285 | WOLF code to signed delta, including diagonals. |
| fn delta_facing(dx: i32, dy: i32) -> Option<i32> | 1300 | Signed delta to WOLF facing code. |

Existing APIs unchanged: try_move_leader (cardinal only), move_leader (one axis),
move_leader_step, event_occupies, contact_box, try_contact_triggers,
try_move_leader_contacts, player_start.

## Diagonal collision rule

For a diagonal from (x,y) to (x+dx,y+dy):

1. Destination must be inside the map.
2. Destination terrain must be enterable along the horizontal component and the
   vertical component (Terrain directional mask, docs/FORMAT_SPEC.md §7.3 and
   docs/research/M9_MOVEMENT.md).
3. The two edge-sharing neighbours (x+dx,y) and (x,y+dy) must be enterable along
   the matching axis. This is the no-corner-cutting rule: a diagonal cannot pass
   through a solid wall corner.
4. The destination tile must not be occupied by a non-slip-through active event.
5. Both edge-sharing neighbours must not be occupied by a non-slip-through
   active event either, so a diagonal cannot squeeze through an NPC corner.

The no-corner-cut and orthogonal-NPC rules are conservative policy: WOLF's
manual documents eight movement directions but does not state whether the
original engine permits corner cutting. Keeping diagonals watertight around wall
and NPC corners avoids clipping through map geometry.

All blocked cases still update the leader facing to the attempted direction and
leave tile/precise coordinates unchanged. Invalid direction codes (0, 5, 10+),
zero delta, and deltas larger than one tile do not turn.

## NPC / contact queries

The manual's variable query at work/_yado_0600.txt:409-414 returns the map event
ID at a specified tile, -1 when empty, and the smallest event id when events
overlap. The precise-coordinate note says the queried tile is the event's
top-left tile (work/_yado_0600.txt:415-418). events_at/event_at implement that
tile lookup for the runtime; npc_at narrows it to active image-bearing events;
contact_events_at instead answers which active contact boxes cover a tile.

## Supported cases

* Eight WOLF directions as one tile per call; delta is exact and precise_x/y move
  by 2 half-tiles.
* Blocked diagonal: solid destination, solid horizontal corner, solid vertical
  corner, map edge, blocking event destination, blocking event corner.
* Slip-through events are walkable diagonally.
* Diagonal contact wrapper fires player-touch even when the diagonal step is
  blocked by the event body, because it checks the attempted destination.
* NPC/contact queries on synthetic and real data (SampleMapB event 0 contact
  box covers (7,8), not (7,7); event_at(7,11) is event 0).

## Not implemented / limits

* No continuous walk, auto-repeat, pixel/half-tile stepping, animation, or
  pathfinding; each call is one whole-tile step.
* Diagonal movement is map-event-aware only through event occupancy; it does not
  move NPCs or apply route data.
* The existing action-key try_trigger still uses cardinal facing_step, so a
  diagonal facing does not extend action-key target reach. This slice changes
  movement/facing and contact triggers, not action targeting.
* Diagonal contact geometry uses the same whole-tile contact box as the
  cardinal path; there is no sub-tile sweep test between the two orthogonals.
* The collisionWidth/Height extension unit remains empirical (8px/half-tile) as
  documented in M9_MAP_CONTACTS.md; no official numeric unit was found.
* No game.rs wrapper was added. Game continues to expose the cardinal
  try_move_leader; callers that need eight directions use game.runtime
  (pub field) or MapRuntime directly.

## Verification

Commands, cwd E:/Study/github program/imoto life:

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_movement_npc -- --nocapture
    test result: ok. 7 passed; 0 failed; 0 ignored; ... finished in 0.00s

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_movement -- --nocapture
    test result: ok. 6 passed; 0 failed

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_map_contacts -- --nocapture
    test result: ok. 6 passed; 0 failed

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_map_triggers -- --nocapture
    test result: ok. 5 passed; 0 failed

    cargo check -q -p wolf-game
    exit 0

No commit or push was made.
