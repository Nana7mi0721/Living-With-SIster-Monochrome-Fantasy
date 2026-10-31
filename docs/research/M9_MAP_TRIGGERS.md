# M9 W3-c: Map Event Trigger Dispatch Core

Status: implemented and green. Slice of task-52 follow-up task-58. No commit; the
Lead integrates. Only crates/wolf-game/src/map_runtime.rs, this slice's test file,
and this document were touched. game.rs was deliberately not changed.

## Result

The runtime now has a testable contact query. Given the party leader tile, a WOLF
facing, an action-button flag and a touch flag, it returns the event/page pairs
the player's contact would run:

    MapRuntime::try_trigger(
        leader: (i32, i32),
        facing: i32,
        action_pressed: bool,
        touch: bool,
    ) -> Vec<TriggerRequest { event: u32, page: usize }>

It covers the two trigger kinds that can be proven from shipped data and the
official editor manual: action key (byte 0) and player touch (byte 3). Event
touch (byte 4) is decoded but never fires, because no shipped page uses it and
its exact quarter-tile hitbox geometry is not specified.

Verification:

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_map_triggers -- --nocapture
    running 5 tests
    test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; finished in 0.02s

    cargo check -q -p wolf-game
    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_movement -- --nocapture
    running 6 tests ... ok. 6 passed; 0 failed
    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_character_state -- --nocapture
    running 3 tests ... ok. 3 passed; 0 failed

cargo check exits 0. The only warnings in the crate are the pre-existing
battle_wiring dead-code warnings.

## Trigger code table

| byte | editor/manual name | Trigger enum | shipped pages | fired by this core | evidence |
|---|---|---|---|---|---|
| 0 | 決定キーで実行 (action key) | ActionKey | 14 | yes | crates/wolf-format/src/mps.rs:344-345; docs/MAP_AUDIT.md:78-79; work/_yado_0600.txt:284-296 |
| 1 | 自動実行 (autorun) | Autorun | 26 | no (start_pending already starts it) | same |
| 2 | 並列実行 (parallel) | Parallel | 90 | no (start_pending already starts it) | same |
| 3 | プレイヤーから接触 (player touch) | PlayerTouch | 4 | yes | same |
| 4 | イベントと接触 (event touch) | EventTouch | 0 | no, decoded only | work/_yado_0600.txt:284-296; editor help |

The census is 134 pages over 73 events in the 9 shipped maps: 14 confirm,
26 autorun, 90 parallel, 4 player-touch, 0 event-touch. The four player-touch
pages are exactly the four pages with non-zero collision boxes.

Before this slice, Trigger::from_byte mapped byte 4 to Unknown(4), whose
is_automatic() is true. That was a latent bug: an unseen trigger 4 would have
been started as an automatic page. The enum now has EventTouch; it is neither
automatic nor looping.

## Page record fields and option bits

crates/wolf-format/src/mps.rs:330-370 defines MapEventPage. The parser reads the
fields in this order at crates/wolf-format/src/mps.rs:372-435:

    page magic 0x79
    u32 graphic_chip_id
    t_str graphic
    u8 direction, u8 frame, u8 opacity, u8 display_type
    u8 trigger
    u8[4] trigger_switches (condition flags, not switch ids)
    u32[4] trigger_variables
    i32[4] trigger_values
    RouteData route
    u32 command_count, Slot[] commands
    u32 features
    u8 shadow_graphic
    u8 collision_width
    u8 collision_height
    optional pageTransfer byte when features > 3
    footer 0x7A

All 134 shipped pages have features == 3, so no pageTransfer byte occurs.
collision_width/height are documented only as "Collision range extension in x/y,
0-250" (crates/wolf-format/src/mps.rs:364-367); their numeric unit is not
documented anywhere found (manual, editor help, WodiLib).

RouteData.behavior_options is the page options byte (crates/wolf-format/src/command.rs:60,
read at :74; layout in docs/FORMAT_SPEC.md:926). WodiLib MapEventPageOption.cs
(https://raw.githubusercontent.com/kameske/WodiLib/master/WodiLib/WodiLib/Map/Model/MapEventPageOption.cs)
gives the exact bits:

    0x01 wait animation on
    0x02 move animation on
    0x04 fixed direction
    0x08 skip through
    0x10 above hero
    0x20 square hitbox (1x1 instead of 1 x 0.5)
    0x40 place half step up

A page has an image iff graphic is non-empty OR graphic_chip_id != 0xFFFFFFFF.

## Real collision/contact census

The four non-zero collision page boxes are all trigger 3:

| map | event (tile) | cw/ch | body tile mask | nearest solid neighbour |
|---|---|---|---|---|
| Dungeon.mps | ev0 (7, 29) | 3 / 0 | 0x0 passable | none adjacent |
| Dungeon.mps | ev2 (21, 13) | 33 / 22 | 0x0 passable | (21, 12) mask 0xF |
| SampleMapB.mps | ev0 (7, 11) | 8 / 6 | 0x0 passable | (8, 11) mask 0xF |
| SampleMapB.mps | ev2 (11, 26) | 4 / 0 | 0x0 passable | none adjacent |

All other 130 pages have collision_width == collision_height == 0.
SampleMapA, SampleMapA_1, Map002, Map003, Map024, Map031 and TitleMap have no
non-zero box at all.

Important correction to an earlier measurement: the first tile word read from a
map header (first_pixel) is layer 0 index 0 in the Rust parser
(crates/wolf-format/src/mps.rs:150-159). A naive binary probe that starts reading
layer 0 after that word shifts every cell by one. With the correct alignment the
four event bodies above are passable; the solid cells are neighbours. The earlier
claim that SampleMapB ev0 sat on a solid tile was one cell off. The Rust Terrain
and the corrected probe agree.

## Implemented contact rules

All rules are applied to the event's currently active page only; the caller must
run start_pending first so page conditions are evaluated.

Action key (trigger 0):
- Candidates are active ActionKey pages that the leader contacts.
- Only the smallest event id is returned (official rule for overlapping action
  events).
- Contact is overlap (leader tile == event tile) when the event must be stood on,
  otherwise overlap or the event tile directly in front of the leader.

Player touch (trigger 3):
- Candidates are active PlayerTouch pages that the leader contacts.
- Every candidate is returned in ascending event id order (official rule).
- The same overlap/front contact rule is used.

Whether the event must be stood on:
- if the page has an image: requires overlap iff skip-through bit 0x08 is set;
  an image with skip-through off is treated as solid for contact, so adjacent
  contact is allowed;
- if the page has no image: requires overlap iff the body tile is passable in
  Terrain; an impassable body allows adjacent contact.

Event touch (trigger 4):
- Decoded as EventTouch; never emitted by try_trigger. No shipped page uses it and
  the official quarter-tile hitbox rule cannot be implemented without the
  undocumented numeric hitbox geometry.

Facing uses WOLF direction codes: 2 down, 4 left, 6 right, 8 up. step_delta maps
the internal step order (0 up, 1 right, 2 left, 3 down) to (dx, dy).

## API and source references

crates/wolf-game/src/map_runtime.rs:
- 54 EventTouch variant
- 67 from_byte(4) => EventTouch
- 80 Trigger::byte EventTouch => 4
- 123 PageState.behavior_options, 125 PageState.has_image, 130 skip_through()
- 206 Terrain struct; 353 is_solid_cell; 371 is_passable_cell
- 378 TriggerRequest struct
- 600 try_trigger
- 653 player_contacts
- 964 step_delta
- 687 try_move_leader / 704 move_leader (task-52 movement API, unchanged)

crates/wolf-format/src/mps.rs:330-370 MapEventPage, 372-435 parser.
crates/wolf-format/src/command.rs:49-65 RouteData, 69-99 reader.
docs/FORMAT_SPEC.md:925-931 page layout and options byte.
docs/FORMAT_SPEC.md:1353 section 7.3 passability word bits; :1379 section 7.4
tileset sizes.

## Tests

crates/wolf-game/tests/m8_map_triggers.rs contains five tests:

1. action_key_activates_only_the_smallest_overlapping_event_on_a_passable_body
   Loads real SampleMapA, sets global var 302 to 1, calls start_pending, then
   checks event 11 at (3,35) is active page 1 and returns TriggerRequest {11,1};
   the adjacent tile (2,35) returns nothing because the body is passable and the
   page has no image.
2. player_touch_on_a_solid_body_starts_from_the_adjacent_tile
   Loads real SampleMapB terrain and moves event 0 onto the measured solid cell
   (8,11). Asserts Terrain::is_solid_cell(8,11), then (7,11) facing right with
   touch returns TriggerRequest {0,0}; two tiles away and an action press return
   nothing.
3. player_touch_on_a_passable_body_requires_standing_on_it
   Real SampleMapB event 2 at passable (11,26): standing on it returns {2,0};
   adjacent (10,26) returns nothing.
4. trigger_kind_and_option_bits_decide_the_contact_reach
   Synthetic pages exercise the option byte: image + 0x23 (square hitbox, no
   skip-through) contacts from the front; image + 0x2B (skip-through set) only
   from overlap; no image on a passable synthetic cell only from overlap;
   trigger 4 returns nothing even with both flags.
5. no_shipped_page_uses_event_touch_but_the_kind_is_decoded
   Asserts Trigger::from_byte(4) == EventTouch, not automatic, byte() == 4, and
   scans all 9 shipped maps asserting 134 pages and no trigger byte 4.

## Supported and deliberately missing

Supported:
- action-key dispatch from the active page with smallest-id rule;
- player-touch dispatch from the active page with ascending-id rule;
- body-tile passability gate using the real TileSetData passability grid;
- image, skip-through and square-hitbox option bits are decoded and the two
  contact-relevant ones are used;
- exact event/page identity and deterministic ordering;
- does not disturb task-52 movement/passability API.

Missing or deliberately limited:
- collision_width/collision_height range extension is not applied. Its unit is
  unproven; implementing it would invent geometry. Every non-zero shipped box is
  a trigger-3 page, so this slice is complete for action-key and passable-body
  player-touch but cannot claim full touch-range parity.
- event touch (byte 4) is decoded but not fired. No shipped page uses it; exact
  quarter-tile hitbox overlap is unproven.
- player-touch official rule says the hero X coordinate must exactly match the
  event X. This core works in whole tiles, so it collapses that to same-tile
  overlap; precise half-tile X does not exist in the current leader model.
- the gate rule that default movement width 1 tile automatically extends contact
  judgment 0.5 tile vertically/horizontally is not modelled. It needs half-tile
  leader/hitbox geometry that task-52 does not implement.
- no rendering, no script execution, no event movement, no event-to-player touch;
  this is a pure dispatch query. The caller must run start_pending first.
- action-key dispatch does not model event images with no image but non-zero
  collision box. No shipped action page has one.

## Dead ends and lessons

- The first probe computed event-body masks with the wrong layer alignment: it
  read layer 0 starting after first_pixel instead of including first_pixel at
  index 0. This shifted the map row by one cell and produced a false "solid event
  body" result. The Rust parser at mps.rs:150-159 is the authority; the corrected
  probe and Terrain agree.
- Treating the four non-zero collision boxes as evidence of solid event bodies
  was wrong. A non-zero collision box is a range extension, not body passability;
  all four bodies are passable.
- There is no real trigger-3 event standing on an impassable tile in shipped
  data, so the solid-body adjacent-contact branch is tested by moving a real
  trigger-3 event onto a real solid tile in the real SampleMapB terrain. That is
  the strongest available proof without fabricating a map.
- A naive assumption that byte 4 would fall through Unknown was unsafe because
  Unknown.is_automatic() is true. The enum now has an explicit EventTouch.

## Unproven items

- Exact numeric unit/geometry of collision_width and collision_height.
- Event-touch quarter-tile hitbox overlap; official manual says any one quarter
  of the hero hitbox counts, but no numeric conversion exists.
- Player-touch exact-X rule at half-tile precision.
- Automatic half-tile contact extension from the default movement-width rule.
- Behaviour of action-key contact when an event has an image with skip-through on
  and a solid body, beyond the simple overlap rule derived from the manual.
- Full event pass-through/square/half-step interaction with movement collision;
  the option bits are decoded but only the contact-relevant subset is used.

## Environment

Repo: E:/Study/github program/imoto life. Data root: work/all.
Tests resolve WOLF_DATA_ROOT relative to the repo root. No commit or push was
made. Permanent scratch files
crates/wolf-format/examples/opscan.rs,
crates/wolf-format/tests/_scratch_cdata.rs and
crates/wolf-format/tests/_scratch_event.rs are untouched.
