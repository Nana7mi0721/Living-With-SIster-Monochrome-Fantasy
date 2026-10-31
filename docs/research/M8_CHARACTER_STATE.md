# M8 W3-a — real map/actor coordinates behind `character_state`

Date: 2026-10-31. Base: branch `worktree-agent-afe08b54f3679529c` at `1d36ebe`
(the M7 second batch `b9e0d36`, which contains `docs/research/M7_HOST_STATE.md`,
`docs/research/M7_OPCODE_124.md` and `tests/m7_host_state.rs`, is **not merged into
this branch**; the field semantics below come from `docs/OPCODE_124.md §3`, which is
present, and those M7 docs were read from `master` read-only).
Data root: `work/all` (`WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/all'`).
Field spec: `docs/OPCODE_124.md §3.2`. Map data: `docs/MAP_AUDIT.md`.

## 1. Conclusion

`Host::character_state` is no longer a stub. It answers the measured `0x1000` /
`0x1200` field space from a real placement layer:

| target | meaning | answer |
|---|---|---|
| `>= 0` | map event id | tile x/y from the `.mps` event record, facing from the **active page**, screen coords, event id |
| `-2` | party leader | the engine placement (see §3), same field space; event id `-1` |
| `-1` | running event | `None` (the host has no interpreter instance context; not fabricated) |
| `-3..=-7` | party members 1..5 | `None` (no party placement layer in this port) |
| unknown event id | — | `None` (the interpreter then leaves the destination untouched) |

`None` still means "no answer": the interpreter keeps the destination variable's old
value, which is the original engine's behaviour for a missing object.

Measured map-event data now flowing through 124 (both verified by the new test):

| evidence | map event | placement | facing |
|---|---|---|---|
| `SampleMapA.mps` event 13 (`デバック用だよ`, unconditional parallel) | 13 | (63, 35) | 2 |
| `Map003.mps` event 1 (`ニューゲーム処理`) | 1 | (30, 17) | 2 |
| `Map003.mps` event 0 (`画像消去`, page wants `v0 == 1`) | 0 | (29, 17) | none (no active page) |
| `SampleMapA.mps` event 1 (`妹クリック判定（夜`) | 1 | (63, 0) | 2 |

## 2. Field mapping

`0x1200` is the same field space with the map-zoom modifier. This game's zoom is 100%,
so fields 8/9 answer exactly like 6/7 (docs/OPCODE_124.md §4).

| field | value | how it is derived |
|---|---|---|
| 0 | tile x | `.mps` `MapEvent.x`; leader `precise_x / 2` |
| 1 | tile y | `.mps` `MapEvent.y`; leader `precise_y / 2` |
| 2 | precise x (0.5 tile) | map events `x * 2`; leader stores the half-tile value |
| 3 | precise y | same |
| 4 | height px | 0 — no jump/height state exists in the map or the port |
| 5 | facing 1..9 | `MapEventPage.direction` of the **active** page; `None` while no page holds |
| 6 | screen x (centre) | `x * 16 + 8` (camera fixed at the map origin) |
| 7 | screen y (feet) | `(y + 1) * 16` |
| 8 | zoom screen x | same as 6 (zoom = 100%) |
| 9 | zoom screen y | same as 7 |
| 10 | event id | the map event id; `-1` for the leader (not a map event) |

The screen-coordinate formulas are the geometry of a 16x16 tile with the camera at
`(0, 0)` (`crates/wolf-game/src/tilemap.rs`, `TILE_PX = 16`; `Game::tile_quads` uses
the same fixed camera). They are inferred from the field names (中央/足下) plus tile
geometry, not from a runtime screenshot; see §5.

Page directions are stored per page and read through the currently active page, so a
page switch changes field 5 on the next frame. Measured values in the shipped maps are
2 (down) and 9 (up-right); `MapEventPage`'s doc comment still says "0-7", which the raw
bytes contradict (nine `SampleMapA_1` pages carry 9).

## 3. Where a leader position comes from (and what is not invented)

The shipped maps have **no player start coordinate**: `MapTree.dat` has no x/y or start
field, the `.mps` event record has no start flag, and the registered maps place no
walkable player actor (docs/MAP_AUDIT.md §1.4, §5.1, §7.5). The game is point-and-click.
Therefore a leader position is **engine state**, and this port only reports one after
the engine has placed it:

1. `Game::place_leader(x, y, direction)` — the explicit engine hook (tile coordinates,
   WOLF numpad facing). Used by debug/new-game wiring and by the acceptance test.
2. `130 Teleport` with `target == -1` (the hero). `Game::frame` turns the queued request
   into a placement on the destination map.

   The transfer's options bit 0 is "precise coordinates", i.e. **half-tiles**:

   * the shipped new-game transfer is `[-1, 62, 38, 1, 1]` (Map003 event 1 pc28) and
     SampleMapA is 64x36 — `(62, 38)` is off the map, `(31, 19)` is on it;
   * the title transfer is `[-1, 60, 34, 3, 1]` (CE42 pc118) and halves to `(30, 17)`,
     which is exactly Map003 event 1's own tile (`ニューゲーム処理` sits at (30, 17)).

   So `place_leader_precise` stores the raw half-tile value (fields 2/3 answer it) and
   truncates `/ 2` for fields 0/1. A transfer carries no direction, so field 5 stays
   unanswered unless the leader already had a facing; the old facing is kept across the
   map change (WOLF keeps the hero's facing; this is inferred, not measured).

`start_pages` builds a fresh runtime, so every map switch drops the previous map's
placements and leader position; the new map's coordinates must never leak across.

## 4. Code path

| file | change |
|---|---|
| `crates/wolf-game/src/map_runtime.rs` | `EventState` keeps `x`, `y` and `runs`; `PageState` keeps `direction`; new `MapActor`, `LeaderPlacement`, `MapRuntime::actor`, `place_leader`, `place_leader_precise`, `leader` |
| `crates/wolf-game/src/game.rs` | `Game::place_leader`; `switch_map` accepts the hero transfer (x, y, options, facing); `GameHost` holds `actors: &MapRuntime`; `character_state` implements the table |
| `crates/wolf-game/src/tilemap.rs` | `tile_centre_px`, `tile_feet_px` + unit test |
| `crates/wolf-game/tests/m8_character_state.rs` | 3 integration tests (real `Game`, real opcode-124 queries) |

`MapRuntime::collect_except` now tracks **every** map event instead of dropping the
skipped boot event and all-blank events. Those keep `runs = false`: their placement and
page selection are available (so 124 can answer), but they never own a coroutine, and
`slots()`/`len()`/`trigger_counts()` still ignore them. This preserves the old
"no instance slot burned on a blank event" behaviour while making the actor layer
complete. `event_count()` consequently returns every event in the map file.

What the runtime retains from the parsed `Map` is the placement layer only: per-event
`x`/`y` and per-page `direction` (plus the already-existing page programs and
condition slots). The tile layers stay owned by `TileMap` and the command bytes by the
compiled `Program`s, so a map switch does not clone the whole `.mps` a second time.

## 5. Proven / not proven

Proven by `crates/wolf-game/tests/m8_character_state.rs` (3 tests, warnings 0):

* map event 13 of SampleMapA reports tile (63, 35), facing 2 (active page), event id 13;
* the leader placed on the same tile reports exactly the same x/y/facing, plus
  precise (126, 70), height 0, screen (1016, 576), `0x1200` field 7 = 576, event id -1;
* an absent event id leaves the destination poison value untouched;
* Map003 event 1 (skipped boot event) keeps its placement (30, 17) and facing 2, while
  event 0 (no active page) answers x/y (29, 17) but leaves field 5 untouched;
* a second `start_pages` replaces placements (SampleMapA event 1 → (63, 0)) and clears
  the previous map's leader position;
* the real new-game chain (`start_map_event_by_name("Map003", 1)`) performs the
  `130 [-1, 62, 38, 1, 1]` transfer and the leader then reports tile (31, 19),
  precise (62, 38), facing unanswered.

Not proven / deliberately out of scope:

1. **Moving actors / half-tile motion**: the port has no movement model. Precise
   coordinates are exact for static placements (`2 * tile`) and for precise transfers;
   a moving character's truncated standard coordinate is not modelled.
2. **Field 5 after a bare transfer**: no facing for a leader that had none. Keeping the
   previous facing is inferred engine behaviour, not measured.
3. **Screen-coordinate offset**: `x*16+8` / `(y+1)*16` with camera `(0,0)` follows tile
   geometry and the field names; there is no retail screenshot/breakpoint comparison.
   Any global screen offset the original might add is not modelled.
4. **Height (field 4)**: always 0; the game never uses a jump height in these maps.
5. **`obj == -1` (running event)** and **party members `-3..-7`**: no placement context
   exists in the host; both return `None`.
6. **Direction code space**: raw page values 2 and 9 are passed through unchanged; the
   parser's "0-7" comment is stale, but no mapping table is invented.

## 6. Commands and results

```
cargo test -q -p wolf-game --lib
# 230 passed; 0 failed

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/all' \
  cargo test -q -p wolf-game --test m8_character_state
# 3 passed; 0 failed

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/all' \
  cargo test -q -p wolf-game --test map_runtime
# 3 passed; 0 failed

cargo test -q -p wolf-vm
# 87 + 11 passed; 0 failed

WOLF_DATA_ROOT='E:/Study/github program/imoto life/work/full' \
  cargo test -q -p wolf-game --test opening --test tile_layer \
  --test m6_mainline --test m6_multiday --test m6_week_loop
# 7 + 1 + 1 + 4 + 1 passed; 0 failed
```

The base branch `1d36ebe` does not contain `tests/m7_host_state.rs` or
`tests/opcode_124_long.rs` (those files are on `master` `b9e0d36`). The M7 host-state
character test was materialised from `master` for one run and removed again:
its `character_state_stays_none_so_no_coordinates_are_fabricated` still passes (a
`Game::new` runtime has no leader, so `-2` field 5/6 return `None`). The other two tests
in that file exercise M7-batch `engine_state`/`picture_state` fields that this base does
not yet have, so they were not run. The mock-host `opcode_124_long` matrix does not
exist on this base; `wolf-vm`'s own 98 tests pass.

Four pre-existing `wolf-game` integration tests fail on this worktree for an
environmental reason only: `m6_battle_commands`, `m6_battle_real`, `m7_battle_round`
and `m7_hscene_advance` build a **repo-relative** path
`<worktree>/work/retail_backup/Save/...`, and this worktree has no `work/` directory
(the task brief says so). They fail with `os error 3` before touching any M8 code; no
M8 test uses those paths (`m8_character_state` honours `WOLF_DATA_ROOT` and skips when
the data is absent).