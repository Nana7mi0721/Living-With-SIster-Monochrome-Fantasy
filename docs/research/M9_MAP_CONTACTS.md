# M9 W3-c follow-up: event contact boxes, player start, contact triggers

Status: implemented and tested on HEAD 22148d8 plus the task-71 working tree. No
commit, no push. Task-71 closed the two largest M9 movement limits recorded in
docs/HANDOFF_M9.md:140-141 ("event collision box unit undefined, only blocks the
active page's own tile; no player start coordinate") and added the missing
player-touch/event-touch dispatch.

## 1. What the .mps page really stores

Every map-event page has two bytes decoded by the format layer as
`collision_width: u8` and `collision_height: u8`
(crates/wolf-format/src/mps.rs:365-367, parser crates/wolf-format/src/mps.rs:372-454).
The editor labels them 接触範囲拡張 X / Y ("contact-range extension X/Y"), range
0-250. No official source states their numeric unit. WodiLib's
`HitExtendRange` only declares `Range(0,255)` / `SafetyRange(0,250)`; the
editor help (04eventwindowB §9) says the extension applies to 決定キー /
プレイヤー接触 / イベント接触 and is affected by the square-hitbox option, but gives
no unit.

The page behaviour byte is the WOLF editor's option word, stored in
`RouteData.behavior_options` (crates/wolf-format/src/command.rs:60, read at
crates/wolf-format/src/command.rs:74). WodiLib MapEventPageOption.cs defines:
0x01 idle animation, 0x02 move animation, 0x04 fixed direction, 0x08 slip-through,
0x10 above hero, 0x20 square hitbox, 0x40 half-step up.

### Census of all 134 shipped pages

130 pages carry (0,0). The only four non-zero collision boxes are:

| map | event | tile | trigger | cw | ch | behavior | image |
|---|---|---|---|---|---|---|---|
| Dungeon.mps | ev0 入口場所移動 | (7,29) | 3 player-touch | 3 | 0 | 0x23 | none |
| Dungeon.mps | ev2 敵エンカウント処理 | (21,13) | 3 player-touch | 33 | 22 | 0x23 | none |
| SampleMapB.mps | ev0 敵エンカウントEV | (7,11) | 3 player-touch | 8 | 6 | 0x23 | none |
| SampleMapB.mps | ev2 Aへの場所移動 | (11,26) | 3 player-touch | 4 | 0 | 0x23 | none |

All four pages are 0x23 = idle animation + move animation + square hitbox; slip-through
(0x08) is OFF; `graphic` is empty and `graphic_chip_id` is 0xFFFFFFFF, i.e. they
draw no image. Their page conditions are unconditional, so they are active by
default.

## 2. Empirical unit decision: these are contact ranges, not movement boxes

Applying the extension as a physical movement block is disproven by the data:

* Dungeon ev2 cw33/ch22 is an encounter trigger on a 40x30 map. A half-tile-unit
  box would cover about 35x23 cells, i.e. most of the dungeon; a movement block
  there makes the map unplayable.
* SampleMapB ev0 cw8/ch6 is an encounter trigger at the mouth of a 1-tile
  corridor. Its extension would wall off the encounter room itself.

Therefore the extension belongs to contact/trigger selection. The event body's
base hit box (1x1 with square ON, 1 wide x 0.5 high with square OFF) remains the
movement block, and both variants lie inside the event's own tile, so the
movement block reduces to the own tile. Slip-through pages are walkable. This is
implemented by `event_occupies` in
crates/wolf-game/src/map_runtime.rs:791-801 (block if active page, not
slip-through, same tile).

The contact rectangle is base hit box plus `collision_width/height` extended on
each side. The unit chosen empirically is the **half-tile (8 px per side)**, because
WOLF's own spatial quantum is the half tile (leader `precise_x = 2 * tile`; the
editor has a 半歩 placement option), and it reproduces the demo's purpose:

| candidate unit | SampleMapB ev0 cells | Dungeon ev2 cells | verdict |
|---|---|---|---|
| 1 px | own cell only | 15 (10 pass/5 solid) | ignores X/Y almost entirely |
| 2 px (1/8 tile) | 9 cells | 63 | too small for a dungeon-wide encounter |
| 4 px (1/4 tile) | 15 | 187 | plausible but small |
| **8 px (1/2 tile)** | **63** | **759** | matches room/dungeon-wide encounter zones |

With the chosen 8 px unit the computed inclusive tile rectangles are:

* SampleMapB ev0 (7,11) cw8/ch6, square 1x1: x 3..=11, y 8..=14.
* SampleMapB ev2 (11,26) cw4/ch0: x 9..=13, y 26..=26.
* Dungeon ev2 (21,13) cw33/ch22: approximately x 4..=38, y 2..=24.
* Dungeon ev0 (7,29) cw3/ch0: approximately x 6..=8, y 29..=29.

The exact official unit is still UNPROVEN; the alternatives above are recorded so
a later data-driven correction is cheap. The code isolates the constant in
crates/wolf-game/src/map_runtime.rs:815 `EXTENSION_UNIT_PX: i32 = 8`.

## 3. Player start coordinate

No map header carries a start coordinate. The map format ends its header at
(width, height, event_count) and the tile block (crates/wolf-format/src/mps.rs:100-205);
`MapEvent::read` stores only x/y placement and pages
(crates/wolf-format/src/mps.rs:247-291). MapTree.dat holds only parent + map_id and
is never read by wolf-game (docs/MAP_AUDIT.md §7.5,
docs/research/M7_NEW_GAME_ENTRY.md).

The authoritative start is the operand of the hero-targeted opcode 130 transfer
that enters the map (target -1, map, x, y, options; options bit 0 = precise
half-tile coordinates). Measured hero transfers in shipped data:

| source | 130 operands | meaning |
|---|---|---|
| TitleMap ev2 -> CE42 pc118 | [-1,60,34,3,1] | hero to Map003 at precise (60,34) |
| Map003 ev1 pc28 | [-1,62,38,1,1] | hero to SampleMapA at precise (62,38) = tile (31,19) |
| Dungeon ev0 | [-1,34,12,1,32] | hero to Map003 (Dungeon is an unregistered demo map) |
| SampleMapB ev2 | [-1,15,1,1,0] | hero to Map001 |
| Map031 ev0 | [-1,22,18,27,16] | hero to map27 |

Implementation:

* `MapRuntime::player_start: Option<LeaderPlacement>` at
  crates/wolf-game/src/map_runtime.rs:425.
* `MapRuntime::set_player_start(x,y,Option<i32>)` at
  crates/wolf-game/src/map_runtime.rs:577 and
  `set_player_start_precise(precise_x,precise_y,Option<i32>)` at
  crates/wolf-game/src/map_runtime.rs:582 record the start and place the leader.
* `MapRuntime::player_start()` at crates/wolf-game/src/map_runtime.rs:590.
* `Game::switch_map` now calls these when a hero transfer arrives
  (crates/wolf-game/src/game.rs:913-921), so the arrival coordinate is the recorded
  start.
* `Game::set_player_start(x,y,direction)` at crates/wolf-game/src/game.rs:789 and
  `Game::player_start()` at crates/wolf-game/src/game.rs:795 expose it to the host.

A bare 130 carries no facing; the start keeps `direction: None` exactly like the
leader placement.

## 4. Player-touch and event-touch firing

New API in crates/wolf-game/src/map_runtime.rs:

```rust
pub fn contact_box(&self, event: u32) -> Option<(i32, i32, i32, i32)>
pub fn try_contact_triggers(&self, from: (i32,i32), to: (i32,i32)) -> Vec<TriggerRequest>
pub fn try_move_leader_contacts(&mut self, direction: i32) -> (bool, Vec<TriggerRequest>)
```

Line refs: `contact_box` 834-839, `try_contact_triggers` 848-879 (PlayerTouch
fires when `inside(to) && !inside(from)`; EventTouch fires whenever `inside(to)`,
so repeated overlap re-fires), `try_move_leader_contacts` 886-901 (uses the
ATTEMPTED destination, so a blocked step into a contact box still fires touch).
Game wrappers: `try_move_leader_contacts` crates/wolf-game/src/game.rs:766 and
`contact_triggers` crates/wolf-game/src/game.rs:775.

Official rules implemented (editor help 04eventwindowB §7/§9 and
01specifi.html §8/§9):

* Player touch: only player -> event; destination entering the contact box fires
  it once.
* Event touch: either direction; any overlap fires, so it repeats while overlapping.
* If body/contact target blocks movement, the blocked attempt still fires touch;
  this matches "body impassable => can start from adjacent / moving toward any
  wall or event inside the contact range".
* Trigger table: 0 action key, 1 autorun, 2 parallel, 3 player touch, 4 event
  touch. Shipped pages: action 14, autorun 26, parallel 90, player-touch 4,
  event-touch 0. Trigger 4 is decoded (`Trigger::EventTouch`) but no retail page
  uses it; tests exercise it synthetically.

Action-key dispatch is unchanged from M9 W3-c
(docs/research/M9_MAP_TRIGGERS.md): `try_trigger` still takes the smallest event
id for action and sorts player-touch candidates by id.

## 5. Files touched (task-71 write scope)

* crates/wolf-game/src/map_runtime.rs: PageState collision_width/collision_height
  and square_hit_box, player_start on MapRuntime, set_player_start* /
  player_start, contact_box / try_contact_triggers / try_move_leader_contacts,
  corrected event_occupies.
* crates/wolf-game/src/game.rs: import new types; contact_triggers,
  try_move_leader_contacts, set_player_start, player_start; switch_map records
  the hero transfer as the player start.
* crates/wolf-game/tests/m8_map_contacts.rs (new, 6 tests).
* docs/research/M9_MAP_CONTACTS.md (this report).

## 6. Verification

Commands run from E:/Study/github program/imoto life with WOLF_DATA_ROOT=work/all:

```
cargo check -q -p wolf-game
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_map_contacts -- --nocapture
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_movement -- --nocapture
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_map_triggers -- --nocapture
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_character_state -- --nocapture
```

Observed results:

* cargo check: exit 0.
* m8_map_contacts: 6 passed, 0 failed.
* m8_movement: 6 passed, 0 failed (kept green; its event-occupancy assertion still
  holds because the base hit box blocks the event's own tile).
* m8_map_triggers: 5 passed, 0 failed.
* m8_character_state: 3 passed, 0 failed.

Caveat: `cargo test -q -p wolf-game --lib` currently fails to compile at
crates/wolf-game/src/savegame.rs:111 with `cannot find type Container in this
scope`; that file is modified in the shared working tree by another task (git
status shows M crates/wolf-game/src/savegame.rs) and is outside task-71's write
scope, so it was not touched.

## 7. Limits / unproven

* Exact numeric unit of collision_width/height is not officially documented; the
  8 px half-tile model is empirical (see section 2) and isolated in one constant.
* Event-touch trigger 4 has no shipped page; the API is implemented and
  synthetically tested, but no retail data exercises it.
* Movement remains a whole-tile step: no diagonal, no continuous walk, no
  animation, no NPC movement. Contact is evaluated on the attempted destination.
* Player start is only as good as the scripts: it is recorded from the hero 130
  operand or an explicit API call; there is no map-header start to read.
* The contact box uses inclusive tile rounding; sub-tile/quarter-tile overlap
  precision (the official "any 1/4 of the hero hitbox" wording) is approximated at
  tile granularity.
