//! M8 W3-a: the map-actor coordinate layer behind opcode 124 `0x1000` / `0x1200`.
//!
//! These tests drive the *real* `Game` host (never a mock) through synthetic programs
//! whose only instruction is `124 SetVariableEx`, against the shipped maps:
//!
//! * a map event (`obj >= 0`) reports the tile x/y from its `.mps` placement and the
//!   facing of its currently active page;
//! * the party leader (`obj == -2`) reports the engine placement it was given, and
//!   that placement is compared against the real map event above;
//! * a query the map truly cannot answer (no such event, no active page, leader not
//!   placed) returns `None`, which the interpreter turns into "leave the destination
//!   untouched" - the same behaviour as the original engine for a missing object.
//!
//! Slice: docs/research/M8_CHARACTER_STATE.md. Field table: docs/OPCODE_124.md §3.2.
//! Skipped when the unpacked data is absent, like every other integration test.

use std::path::PathBuf;
use std::sync::Arc;

use wolf_format::command::{Command, Slot, Tail};
use wolf_format::mps::Map;
use wolf_game::Game;
use wolf_vm::Program;

/// Build one command slot. `param_count` counts the opcode byte plus every argument,
/// so the interpreter's argc is `args.len()` (this is what makes `0x3000` short-form).
fn cmd(opcode: u32, args: &[i32]) -> Slot {
    let mut params = Vec::with_capacity(args.len() * 4);
    for arg in args {
        params.extend_from_slice(&arg.to_le_bytes());
    }
    Slot::Command(Box::new(Command {
        param_count: (args.len() + 1) as u8,
        opcode,
        params,
        branch_depth: 0,
        strings: Vec::new(),
        tail: Tail::None,
    }))
}

/// The data root: `WOLF_DATA_ROOT` when set, otherwise `work/all` (the merged data
/// set this checkout ships with) falling back to `work/full`.
fn data_root() -> PathBuf {
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    if let Ok(value) = std::env::var("WOLF_DATA_ROOT") {
        if !value.is_empty() {
            let path = PathBuf::from(value);
            // Tests run with cwd = the crate directory, so a relative override
            // (WOLF_DATA_ROOT=work/all) must resolve against the repo root.
            return if path.is_absolute() {
                path
            } else {
                base.join(path)
            };
        }
    }
    let all = base.join("work").join("all");
    if all.join("BasicData").join("CommonEvent.dat").is_file() {
        return all;
    }
    base.join("work").join("full")
}

fn open_game() -> Option<Game> {
    Game::new(data_root()).ok()
}

fn read_map(name: &str) -> Option<Map> {
    let bytes = std::fs::read(data_root().join("MapData").join(name)).ok()?;
    Map::parse(&bytes).ok()
}

/// Run one synthetic program on the real host for exactly one frame.
fn run_once(game: &mut Game, id: u32, title: &str, commands: Vec<Slot>) {
    // The common-event scheduler would write normal variables of its own; the map
    // runtime still runs, because page selection is what supplies the facing.
    game.auto_events.set_enabled(false);
    let program = Program::from_slots(id, title, commands);
    assert!(
        game.vm.start_program(Arc::new(program)).is_some(),
        "synthetic program should start"
    );
    let _ = game.frame();
}

fn poison(game: &mut Game, indices: &[usize]) {
    for &index in indices {
        game.vm.vars.set(index, 424_242);
    }
}

#[test]
fn the_leader_reports_the_placed_map_events_tile_and_direction() {
    let Some(mut game) = open_game() else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    let Some(map) = read_map("SampleMapA.mps") else {
        eprintln!("skipping: no SampleMapA.mps");
        return;
    };
    game.start_pages(&map, None);

    // Event 13 is the map's unconditional parallel hook (two slots), so it has an
    // active page without any variable being set. Its placement and facing are real
    // .mps data: (63, 35), every shipped SampleMapA page faces 2 (down).
    let event = map
        .events
        .iter()
        .find(|event| event.id == 13)
        .expect("SampleMapA shipped without event 13");
    assert_eq!((event.x, event.y), (63, 35), "measured placement changed");
    let page = event.pages.first().expect("event 13 has no page");
    let direction = i32::from(page.direction);
    assert_eq!(direction, 2, "measured facing changed");

    // Place the party leader on the same tile. This is the engine placement hook
    // (Game::place_leader) a new-game/debug path uses; no coordinate is invented.
    game.place_leader(event.x as i32, event.y as i32, direction);

    poison(&mut game, &(640..=670).collect::<Vec<_>>());
    let commands = vec![
        // The map event itself: tile x/y, facing, event id.
        cmd(124, &[2_000_640, 0x1000, 13, 0]),
        cmd(124, &[2_000_641, 0x1000, 13, 1]),
        cmd(124, &[2_000_642, 0x1000, 13, 5]),
        cmd(124, &[2_000_643, 0x1000, 13, 10]),
        // The leader: the same tile, facing, precise coords, height, screen coords.
        cmd(124, &[2_000_650, 0x1000, -2, 0]),
        cmd(124, &[2_000_651, 0x1000, -2, 1]),
        cmd(124, &[2_000_652, 0x1000, -2, 2]),
        cmd(124, &[2_000_653, 0x1000, -2, 3]),
        cmd(124, &[2_000_654, 0x1000, -2, 4]),
        cmd(124, &[2_000_655, 0x1000, -2, 5]),
        cmd(124, &[2_000_656, 0x1000, -2, 6]),
        cmd(124, &[2_000_657, 0x1000, -2, 7]),
        cmd(124, &[2_000_658, 0x1000, -2, 8]),
        cmd(124, &[2_000_659, 0x1000, -2, 9]),
        cmd(124, &[2_000_660, 0x1000, -2, 10]),
        // 0x1200 shares the field space; zoom is 100% in this game, so field 7 is
        // the same answer.
        cmd(124, &[2_000_661, 0x1200, -2, 7]),
        // A map event that does not exist: the destination must stay untouched.
        cmd(124, &[2_000_670, 0x1000, 9999, 0]),
        cmd(172, &[]),
    ];
    run_once(&mut game, 910, "m8 character state", commands);

    assert_eq!(game.vm.vars.get(640), 63, "event tile x");
    assert_eq!(game.vm.vars.get(641), 35, "event tile y");
    assert_eq!(
        game.vm.vars.get(642),
        2,
        "event facing from the active page"
    );
    assert_eq!(game.vm.vars.get(643), 13, "event id");
    assert_eq!(
        game.vm.vars.get(650),
        63,
        "leader tile x equals the placed map event's x"
    );
    assert_eq!(
        game.vm.vars.get(651),
        35,
        "leader tile y equals the placed map event's y"
    );
    assert_eq!(game.vm.vars.get(652), 126, "precise x = tile * 2");
    assert_eq!(game.vm.vars.get(653), 70, "precise y = tile * 2");
    assert_eq!(game.vm.vars.get(654), 0, "no jump height");
    assert_eq!(
        game.vm.vars.get(655),
        2,
        "leader facing equals the placed map event's facing"
    );
    assert_eq!(game.vm.vars.get(656), 63 * 16 + 8, "screen centre x");
    assert_eq!(game.vm.vars.get(657), (35 + 1) * 16, "screen feet y");
    assert_eq!(game.vm.vars.get(658), 63 * 16 + 8, "zoom x (100% here)");
    assert_eq!(game.vm.vars.get(659), (35 + 1) * 16, "zoom y (100% here)");
    assert_eq!(game.vm.vars.get(660), -1, "a party member has no event id");
    assert_eq!(game.vm.vars.get(661), (35 + 1) * 16, "0x1200 field 7");
    assert_eq!(
        game.vm.vars.get(670),
        424_242,
        "an absent map event leaves the destination alone"
    );
    assert_eq!(
        game.stats.warnings, 0,
        "known character fields warn nothing"
    );
}

#[test]
fn the_new_game_transfer_places_the_leader_on_the_destination_tile() {
    let Some(mut game) = open_game() else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    // The real new-game chain: Map003 event 1 teleports the hero to SampleMapA with
    // `130 [-1, 62, 38, 1, 1]` (MAP_AUDIT §7.2, pc28). Options bit 0 marks precise
    // (half-tile) coordinates: the map is 64x36, so (62, 38) only fits as tile
    // (31, 19). This proves the placement comes from real script data, not the test.
    game.auto_advance = true;
    if let Err(err) = game.start_map_event_by_name("Map003", 1) {
        eprintln!("skipping: cannot start the new-game map: {err}");
        return;
    }
    let mut reached = false;
    for _ in 0..600 {
        let _ = game.frame();
        if game
            .diagnostics
            .keys()
            .any(|line| line.contains("130 Teleport: MapData/SampleMapA.mps"))
        {
            reached = true;
            break;
        }
    }
    assert!(reached, "the new-game transfer never fired");

    poison(&mut game, &(720..=726).collect::<Vec<_>>());
    let commands = vec![
        cmd(124, &[2_000_720, 0x1000, -2, 0]),
        cmd(124, &[2_000_721, 0x1000, -2, 1]),
        cmd(124, &[2_000_722, 0x1000, -2, 2]),
        cmd(124, &[2_000_723, 0x1000, -2, 3]),
        cmd(124, &[2_000_724, 0x1000, -2, 5]),
        cmd(124, &[2_000_725, 0x1000, 1, 0]),
        cmd(124, &[2_000_726, 0x1000, 1, 1]),
        cmd(172, &[]),
    ];
    run_once(&mut game, 914, "m8 transfer placement", commands);
    assert_eq!(
        game.vm.vars.get(720),
        31,
        "leader tile x from the precise 130 destination (62 / 2)"
    );
    assert_eq!(
        game.vm.vars.get(721),
        19,
        "leader tile y from the precise 130 destination (38 / 2)"
    );
    assert_eq!(
        game.vm.vars.get(722),
        62,
        "precise x is the raw 130 operand"
    );
    assert_eq!(
        game.vm.vars.get(723),
        38,
        "precise y is the raw 130 operand"
    );
    assert_eq!(
        game.vm.vars.get(724),
        424_242,
        "a transfer carries no facing, so field 5 stays unanswered"
    );
    assert_eq!(game.vm.vars.get(725), 63, "SampleMapA event 1 tile x");
    assert_eq!(game.vm.vars.get(726), 0, "SampleMapA event 1 tile y");
    assert_eq!(game.stats.warnings, 0);
}

#[test]
fn placements_follow_the_map_and_the_leader_needs_a_placement() {
    let Some(mut game) = open_game() else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    let Some(map003) = read_map("Map003.mps") else {
        eprintln!("skipping: no Map003.mps");
        return;
    };
    let Some(sample) = read_map("SampleMapA.mps") else {
        eprintln!("skipping: no SampleMapA.mps");
        return;
    };

    // The new-game processor is event 1; skip its coroutine so it cannot teleport
    // away during the query, but its placement is still queryable.
    game.start_pages(&map003, Some(1));
    let event1 = map003
        .events
        .iter()
        .find(|event| event.id == 1)
        .expect("Map003 shipped without event 1");
    let event0 = map003
        .events
        .iter()
        .find(|event| event.id == 0)
        .expect("Map003 shipped without event 0");
    assert_eq!((event1.x, event1.y), (30, 17));
    assert_eq!((event0.x, event0.y), (29, 17));

    poison(&mut game, &(680..=700).collect::<Vec<_>>());
    // No leader placement yet: -2 must leave the destinations untouched.
    let commands = vec![
        cmd(124, &[2_000_680, 0x1000, -2, 0]),
        cmd(124, &[2_000_681, 0x1000, -2, 5]),
        cmd(172, &[]),
    ];
    run_once(&mut game, 911, "m8 leader unplaced", commands);
    assert_eq!(game.vm.vars.get(680), 424_242, "no fabricated leader x");
    assert_eq!(game.vm.vars.get(681), 424_242, "no fabricated facing");
    assert_eq!(game.stats.warnings, 0);

    game.place_leader(30, 17, 2);
    poison(&mut game, &(690..=700).collect::<Vec<_>>());
    let commands = vec![
        cmd(124, &[2_000_690, 0x1000, 1, 0]),
        cmd(124, &[2_000_691, 0x1000, 1, 1]),
        cmd(124, &[2_000_692, 0x1000, 1, 5]),
        cmd(124, &[2_000_693, 0x1000, 1, 10]),
        // Event 0 exists but its only page wants v0 == 1; with v0 = 0 it has no
        // active page, so the placement is real but the facing is unanswered.
        cmd(124, &[2_000_694, 0x1000, 0, 0]),
        cmd(124, &[2_000_695, 0x1000, 0, 1]),
        cmd(124, &[2_000_696, 0x1000, 0, 5]),
        cmd(124, &[2_000_697, 0x1000, -2, 0]),
        cmd(124, &[2_000_698, 0x1000, -2, 1]),
        cmd(172, &[]),
    ];
    run_once(&mut game, 912, "m8 map003 placement", commands);
    assert_eq!(
        game.vm.vars.get(690),
        30,
        "skipped event keeps its placement"
    );
    assert_eq!(game.vm.vars.get(691), 17);
    assert_eq!(
        game.vm.vars.get(692),
        2,
        "page facing survives being skipped"
    );
    assert_eq!(game.vm.vars.get(693), 1, "event id");
    assert_eq!(
        game.vm.vars.get(694),
        29,
        "inactive event still has a placement"
    );
    assert_eq!(game.vm.vars.get(695), 17);
    assert_eq!(
        game.vm.vars.get(696),
        424_242,
        "no active page means no facing to report"
    );
    assert_eq!(game.vm.vars.get(697), 30, "leader x");
    assert_eq!(game.vm.vars.get(698), 17, "leader y");
    assert_eq!(game.stats.warnings, 0);

    // A new map replaces every placement. SampleMapA event 1 is at (63, 0), and the
    // old map's leader position must not leak across the switch.
    game.start_pages(&sample, None);
    poison(&mut game, &(700..=703).collect::<Vec<_>>());
    let commands = vec![
        cmd(124, &[2_000_700, 0x1000, 1, 0]),
        cmd(124, &[2_000_701, 0x1000, 1, 1]),
        cmd(124, &[2_000_702, 0x1000, -2, 0]),
        cmd(124, &[2_000_703, 0x1000, -2, 5]),
        cmd(172, &[]),
    ];
    run_once(&mut game, 913, "m8 placement swap", commands);
    assert_eq!(game.vm.vars.get(700), 63, "SampleMapA event 1 tile x");
    assert_eq!(game.vm.vars.get(701), 0, "SampleMapA event 1 tile y");
    assert_eq!(
        game.vm.vars.get(702),
        424_242,
        "the previous map's leader position must not leak"
    );
    assert_eq!(game.vm.vars.get(703), 424_242);
    assert_eq!(game.stats.warnings, 0);
}
