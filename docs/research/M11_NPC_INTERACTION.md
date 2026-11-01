# M11 P1: held-direction continuous walking and NPC action-key interaction

Repo: E:/Study/github program/imoto life
HEAD at slice: 5ef6b1f
Scope: crates/wolf-game/src/map_runtime.rs, crates/wolf-game/src/game.rs,
crates/wolf-game/tests/m11_npc_interaction.rs, this file. No commit or push.

## Result

The engine-side input core now has both halves the task asked for:

* Held-direction continuous walking. A direction press takes one whole-tile step
  immediately, then repeats one step every WALK_STEP_FRAMES = 8 frames while the
  direction stays held. The step uses the existing eight-way movement core.
* ActionKey interaction. A fresh confirm press asks the map runtime which active
  action-key page the leader's tile and facing reach, and starts that page. This
  reuses the official WOLF contact rule; it does not invent a second targeting
  model.
* Both paths are frame-based and headless: no rendering, animation, NPC route
  movement, or pathfinding is added.

## Retail scope (M11 round 3 correction)

The shipped game is point-and-click: the six registered maps have 115 event pages
with empty `graphic`/`graphic_chip_id`, and interaction runs through 150 Picture
hotspots plus 124 hover checks (`docs/MAP_AUDIT.md:233-268`). Across all 9 `.mps`
files (134 pages), 133 pages use fixed `route_mode` and every page has route length
0; the 73 scripted 201 Move commands only do bomb/approach effects in the isolated
Map002 sample and invisible load/new-game positioning in Map003/Map031. This
module is therefore engine-generic input coverage, not a shipped NPC walking or
walk-up dialogue gap; there is no retail walking animation to port.

## Frame input contract

All code is in crates/wolf-game/src/game.rs.

| item | line | behaviour |
|------|------|-----------|
| pub const WALK_STEP_FRAMES: u32 = 8 | 72 | 2 px/frame x 1 frame/step at the default Game.dat move speed, 16 px tile => one tile per eight frames. |
| walk_last_direction: i32 | 319 | Last direction sampled from the host. |
| walk_wait: u32 | 321 | Frames remaining before the next repeat attempt. |
| initial values 0 / 0 | 478-479 | No direction latched at construction. |
| pub fn interact_action_key(&mut self) -> Vec<TriggerRequest> | 830 | Action-key half (see below). |
| fn advance_player_input(&mut self) | 854 | Called once per Game::frame before handle_choice_input and the VM step. |
| self.advance_player_input() call | 1367 | Inside Game::frame, after auto_events.sync and battle_wiring.tick, before handle_choice_input; a page started here is visible to the same frame's VM step. |
| switch_map resets 0 / 0 | 1003-1004 | A map transfer starts a fresh walking context even when the same key is held. |

The per-frame algorithm is:

1. pressed = key_bits & !previous_keys. Only a fresh edge counts as an action press.
2. dialogue_open = choices.is_some() || !messages.is_empty().
3. If not dialogue_open, a leader exists, and pressed contains key::OK, call
   interact_action_key first. If it started any page, reset the walk cadence and
   return; the action wins over walking on that frame.
4. held = key::code(key_bits & key::DIRECTIONS). If dialogue is open, no direction
   is held, or no leader is placed, reset walk_last_direction/walk_wait and return.
5. If held differs from walk_last_direction, latch the new direction and set
   walk_wait = 0 so the first step is immediate.
6. If walk_wait == 0, call runtime.try_move_leader_contacts_8(held), start every
   returned player-touch page, and set walk_wait = 7. Otherwise decrement walk_wait.

Consequences:

* Step frames for a continuously held direction are 1, 9, 17, 25, ... (the first
  frame of the press is already a full step). In the 25-frame test that is four
  tiles, not three.
* A changed direction or a release/press restarts the immediate step.
* A blocked step still runs the movement core, which updates facing before the
  collision answer; a tap into a wall therefore turns in place.
* Messages and choices suppress both interaction and walking, so a dialogue page
  never walks the leader out of the conversation.
* Only the direction bits are passed to key::code for walking; a confirm press
  that finds no NPC does not block a simultaneous direction.
* key::code and key::DIRECTIONS are in crates/wolf-game/src/key.rs:119 and :59.
  TILE_PX = 16 is crates/wolf-game/src/tilemap.rs:29.

### Cadence evidence

docs/FORMAT_SPEC.md:248 records Game.dat u16_settings idx 1..14 as the
custom_move_speed_hero_allies_N / custom_move_speed_events_N pairs:
(2,1) (4,2) (8,4) (16,8) (32,16) (64,32) (128,64). The default pair is
(2 pixels per frame, 1 frame per step). With TILE_PX = 16 px, 16/2 = 8 frames per
tile. The movement core is whole-tile only, so the eight-frame divisor is the
only cadence this slice models; it deliberately does not render the two
intermediate pixel positions.

## Action-key interaction

crates/wolf-game/src/game.rs:830 interact_action_key:

1. Return empty when a message or choice window is open, or when no leader is
   placed.
2. Read leader tile x/y and direction (direction defaults to 0 when unset).
3. Ask runtime.try_trigger((x,y), facing, action_pressed=true, touch=false).
4. Start every returned request through runtime.start_trigger_page(...,&mut vm)
   and return the requests for diagnostics/tests.

The targeting rule lives in MapRuntime and is the official one
(crates/wolf-game/src/map_runtime.rs:646 try_trigger and :755 player_contacts):

* Standing exactly on the event always contacts it.
* An image-bearing event with slip-through OFF is reachable from the adjacent
  tile in the facing direction.
* An image-bearing event with slip-through ON, or a no-image event on a passable
  tile, must be stood on; it is not reachable from the adjacent tile.
* A no-image event on an impassable tile is reachable from the adjacent tile.
* When several action events overlap, the smallest event id wins.
* Action targeting uses cardinal facing_step only, so a diagonal facing does not
  extend action reach. This is the existing M11 P1-4 boundary.

The launch half is crates/wolf-game/src/map_runtime.rs:707
start_trigger_page(&mut self, request: TriggerRequest, vm: &mut Vm) -> bool. It
requires the request to name the event's current active page, refuses to start a
second live instance, runs the page with its self-variable bank, and increments
starts/starts_on_page/restarts. That makes holding a key safe: it cannot clone a
running conversation coroutine.

## Tests

New file crates/wolf-game/tests/m11_npc_interaction.rs (5 tests, synthetic maps +
the real Game when WOLF_DATA_ROOT is present; skipped otherwise):

| test | proves |
|------|--------|
| held_direction_walks_one_tile_immediately_and_then_every_eight_frames | leader (2,2) facing down, hold RIGHT for 25 frames => tile (6,2), precise (12,4), facing 6: exactly four repeated steps at frames 1/9/17/25. |
| a_blocked_tap_turns_in_place_and_holding_into_a_wall_never_moves | at the map corner, holding LEFT for 20 frames leaves tile and precise at (0,0) but facing becomes 4; a following UP tap turns to 8. |
| a_fresh_confirm_press_starts_an_adjacent_image_action_event | an image event with behavior_options 0x10 (slip-through off) at (2,1), leader at (1,1) facing right; key_bits = OK then frame executes its opcode 121 and leaves global variable 123 = 1. |
| the_action_key_ignores_the_wrong_facing_and_a_passable_body_needs_overlap | facing away does not reach the event; a no-image event on a passable tile does not reach from adjacent, but standing on it yields exactly one request. |
| diagonal_held_input_uses_the_eight_way_step_core | holding UP|RIGHT for one frame moves one diagonal tile to (5,3), precise (10,6), facing 9. |

Observed commands and counts:

```text
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_npc_interaction -- --nocapture
test result: ok. 5 passed; 0 failed; 0 ignored

WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_movement_npc -- --nocapture
test result: ok. 7 passed; 0 failed

WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_movement -- --nocapture
test result: ok. 6 passed; 0 failed

WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_map_contacts -- --nocapture
test result: ok. 6 passed; 0 failed

WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_map_triggers -- --nocapture
test result: ok. 5 passed; 0 failed

WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_character_state -- --nocapture
test result: ok. 3 passed; 0 failed
```

## Workspace verification

`WOLF_DATA_ROOT=work/all cargo test -q --workspace --no-fail-fast -j 2` ran
119 test binaries: **775 passed, 1 failed**. The only failure is the pre-existing
`-p wolf-game --test m9_full_route` test
`the_continuous_new_game_route_reaches_day_five_0830` at
crates/wolf-game/tests/m9_full_route.rs:645: actual v350_seq
[0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0] vs expected
[0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 0], and v352_peak -1 vs expected 1. This is
not caused by this slice: restoring both changed files to their HEAD versions
(`git show HEAD:crates/wolf-game/src/game.rs` and
`git show HEAD:crates/wolf-game/src/map_runtime.rs`) makes the same test fail
identically at frames=1880 with the same 13-entry v350_seq. Full log:
work/out/task91_workspace_serial.log.

A later rerun with that single test skipped was blocked by an unrelated untracked
test owned by another workstream, crates/wolf-game/tests/m11_party_live_bridge.rs,
which did not compile (`PARTY_COUNT_TYPE` unresolved at lines 88 and 249). That
file is outside this task's write scope and was not changed here. All six
task-relevant binaries above stayed green in the same worktree.

## Supported cases

* Held cardinal or diagonal direction: one whole tile immediately, repeat every
  eight frames, using the already-proven eight-way collision rules (bounds,
  directional terrain mask, event occupancy, no corner cutting).
* Direction change and release/re-press: immediate step; blocked taps still turn.
* Fresh ActionKey on an adjacent active image event with slip-through off.
* Fresh ActionKey on the event tile for passable no-image events.
* Action pages started through start_trigger_page cannot be double-started while
  their VM instance is live.

## Not implemented / limits

* Whole-tile motion only: no sub-tile pixel animation, walking animation frames,
  or move-route playback for the leader or NPCs.
* No pathfinding, no auto-repeat of the action key while it stays held, and no
  mouse/touch movement. Only the fresh OK edge is an interaction trigger.
* Action-key reach stays cardinal even though movement supports diagonals.
* Event-touch (trigger 4) remains decoded-only; no shipped page uses it.
* NPCs do not move or animate; only their static active page is consulted.
* The synthetic tests require WOLF_DATA_ROOT for Game construction; without the
  data root they report as skipped rather than failing.
