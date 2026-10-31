//! M9 W3-b: the headless player movement and passability core.
//!
//! These tests drive the real `Game` host for the shipped maps plus a synthetic
//! map/runtime for the cases the shipped SampleMapA cannot express (it has no
//! blocked tile anywhere and no event in its middle). Slice:
//! docs/research/M9_MOVEMENT.md.
//!
//! What is covered:
//! * one-tile free movement with `precise_x/precise_y = 2 * tile` and facing;
//! * map-edge blocking, event/person blocking and solid-tile blocking;
//! * a blocked step still turns the leader, exactly as a direction press does;
//! * explicit placement is unchanged (no regression to the M8 place_leader hook).
//!
//! Skipped when the unpacked data is absent, like every other integration test.

use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::mps::{Map, MapEvent, MapEventPage};
use wolf_game::map_runtime::{MapRuntime, Terrain};
use wolf_game::Game;
use wolf_vm::{Host, ProgramSet, Vm};

/// The data root: `WOLF_DATA_ROOT` when set, otherwise `work/all` (the merged data
/// set this checkout ships with) falling back to `work/full`.
fn data_root() -> PathBuf {
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    if let Ok(value) = std::env::var("WOLF_DATA_ROOT") {
        if !value.is_empty() {
            let path = PathBuf::from(value);
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

fn read_tilesets() -> Option<wolf_format::tileset::TileSetData> {
    let bytes = std::fs::read(data_root().join("BasicData").join("TileSetData.dat"))
        .or_else(|_| std::fs::read(data_root().join("TileSetData.dat")))
        .ok()?;
    wolf_format::tileset::TileSetData::parse(&bytes).ok()
}

fn blank_page(trigger: u8) -> MapEventPage {
    MapEventPage {
        graphic_chip_id: 0,
        graphic: String::new(),
        direction: 2,
        frame: 0,
        opacity: 255,
        display_type: 0,
        trigger,
        trigger_switches: [0; 4],
        trigger_variables: [0; 4],
        trigger_values: [0; 4],
        route: Default::default(),
        commands: vec![Slot::Blank],
        features: 3,
        shadow_graphic: 0,
        collision_width: 0,
        collision_height: 0,
        footer: 0x7A,
    }
}

fn map_with_one_event(width: u32, height: u32, id: u32, x: u32, y: u32) -> Map {
    Map {
        version_header: 0,
        version: 0,
        name: String::new(),
        tileset_index: 0,
        width,
        height,
        has_tile_data: false,
        layers: Vec::new(),
        events: vec![MapEvent {
            id,
            name: "blocker".to_owned(),
            x,
            y,
            pages: vec![blank_page(2)],
        }],
        footer: 0x66,
    }
}

struct Quiet;

impl Host for Quiet {}

#[test]
fn leader_steps_one_tile_and_keeps_precise_coordinates_exact() {
    let Some(mut game) = open_game() else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    let Some(map) = read_map("SampleMapA.mps") else {
        eprintln!("skipping: no SampleMapA.mps");
        return;
    };
    game.start_pages(&map, None);
    // The whole of SampleMapA is passable (every chip is autotile 1, passability 0),
    // so this exercises the free-step path on real map data.
    game.place_leader(30, 20, 2);
    let start = game.runtime.leader().expect("leader placed");
    assert_eq!(
        (
            start.x,
            start.y,
            start.precise_x,
            start.precise_y,
            start.direction
        ),
        (30, 20, 60, 40, Some(2))
    );

    assert!(game.try_move_leader(6), "step right should succeed");
    let right = game.runtime.leader().expect("leader placed");
    assert_eq!((right.x, right.y), (31, 20));
    assert_eq!((right.precise_x, right.precise_y), (62, 40));
    assert_eq!(right.direction, Some(6), "facing follows the step");

    assert!(game.try_move_leader(2), "step down should succeed");
    let down = game.runtime.leader().expect("leader placed");
    assert_eq!((down.x, down.y), (31, 21));
    assert_eq!((down.precise_x, down.precise_y), (62, 42));
    assert_eq!(down.direction, Some(2));

    assert!(
        !game.try_move_leader(1),
        "a diagonal is not a one-tile step"
    );
    assert_eq!(game.runtime.leader().unwrap(), down);
}

#[test]
fn map_edge_and_event_occupancy_block_the_step() {
    let Some(mut game) = open_game() else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    let Some(map) = read_map("SampleMapA.mps") else {
        eprintln!("skipping: no SampleMapA.mps");
        return;
    };
    game.start_pages(&map, None);

    // Map edge: (0,0) cannot move up or left. The blocked attempt still turns.
    game.place_leader(0, 0, 2);
    assert!(!game.try_move_leader(4), "left of the map is blocked");
    let left = game.runtime.leader().unwrap();
    assert_eq!((left.x, left.y), (0, 0));
    assert_eq!(left.direction, Some(4), "a blocked step still turns");
    assert!(!game.try_move_leader(8), "above the map is blocked");
    assert_eq!((left.x, left.y), (0, 0));
    assert_eq!(game.runtime.leader().unwrap().direction, Some(8));

    // Event 13 is a live parallel event on tile (63,35); step into it from (62,35).
    // This is the "person blocks" case.
    game.place_leader(62, 35, 6);
    assert!(
        !game.try_move_leader(6),
        "a live map event occupies (63,35)"
    );
    let blocked = game.runtime.leader().unwrap();
    assert_eq!((blocked.x, blocked.y), (62, 35));
    assert_eq!(blocked.direction, Some(6));
}

#[test]
fn a_solid_tile_blocks_the_step() {
    let mut runtime = MapRuntime::new();
    let mut terrain = Terrain::new(4, 4);
    terrain.set_tile_blocked(2, 1, true);
    runtime.set_terrain(terrain);
    runtime.place_leader(1, 1, None);

    assert!(!runtime.try_move_leader(6), "the solid tile blocks entry");
    let blocked = runtime.leader().unwrap();
    assert_eq!((blocked.x, blocked.y), (1, 1));
    assert_eq!((blocked.precise_x, blocked.precise_y), (2, 2));
    assert_eq!(blocked.direction, Some(6));

    assert!(runtime.try_move_leader(2), "the tile below is free");
    assert_eq!(runtime.leader().unwrap().x, 1);
    assert_eq!(runtime.leader().unwrap().y, 2);
    assert!(runtime.try_move_leader(6), "the diagonal-free tile is free");
    assert_eq!(runtime.leader().unwrap().x, 2);
    assert_eq!(runtime.leader().unwrap().y, 2);
}

#[test]
fn an_active_map_event_blocks_its_tile() {
    let map = map_with_one_event(4, 4, 7, 2, 1);
    let mut runtime = MapRuntime::collect(&map);
    let mut vm = Vm::new(ProgramSet::default());
    let mut host = Quiet;
    runtime.start_pending(&mut vm);
    assert!(
        runtime
            .actor(7)
            .expect("event placement")
            .active_page
            .is_some(),
        "the blank page is active, so the event is on the map"
    );
    runtime.place_leader(1, 1, None);

    assert!(!runtime.try_move_leader(6), "the event tile is occupied");
    let blocked = runtime.leader().unwrap();
    assert_eq!((blocked.x, blocked.y), (1, 1));
    assert_eq!(blocked.direction, Some(6));

    assert!(runtime.try_move_leader(2), "the tile below is empty");
    assert_eq!(
        (runtime.leader().unwrap().x, runtime.leader().unwrap().y),
        (1, 2)
    );
    let _ = vm.step_frame(&mut host);
}

#[test]
fn place_leader_is_unchanged_and_precise_origins_move_by_whole_tiles() {
    let mut runtime = MapRuntime::new();
    runtime.set_terrain(Terrain::new(64, 40));
    // The M8 contract: whole-tile placement stores precise = tile*2 and the facing.
    runtime.place_leader(11, 12, Some(8));
    let placed = runtime.leader().unwrap();
    assert_eq!((placed.x, placed.y), (11, 12));
    assert_eq!((placed.precise_x, placed.precise_y), (22, 24));
    assert_eq!(placed.direction, Some(8));

    // A precise (half-tile) placement keeps its odd offset when moving by a tile.
    runtime.place_leader_precise(63, 39, Some(2));
    let precise = runtime.leader().unwrap();
    assert_eq!((precise.x, precise.y), (31, 19));
    assert_eq!((precise.precise_x, precise.precise_y), (63, 39));
    assert!(runtime.try_move_leader(6));
    let moved = runtime.leader().unwrap();
    assert_eq!((moved.x, moved.y), (32, 19));
    assert_eq!((moved.precise_x, moved.precise_y), (65, 39));
    assert!(runtime.try_move_leader(2));
    let down = runtime.leader().unwrap();
    assert_eq!((down.x, down.y), (32, 20));
    assert_eq!((down.precise_x, down.precise_y), (65, 41));
}

#[test]
fn the_real_tileset_passability_grid_marks_dungeon_walls_and_samplemapa_nothing() {
    let Some(dungeon) = read_map("Dungeon.mps") else {
        eprintln!("skipping: no Dungeon.mps");
        return;
    };
    let Some(sample) = read_map("SampleMapA.mps") else {
        eprintln!("skipping: no SampleMapA.mps");
        return;
    };
    let Some(tilesets) = read_tilesets() else {
        eprintln!("skipping: no TileSetData.dat");
        return;
    };
    let dungeon_set = tilesets
        .get(dungeon.tileset_index)
        .expect("Dungeon tileset must exist");
    let sample_set = tilesets
        .get(sample.tileset_index)
        .expect("SampleMapA tileset must exist");
    let terrain = Terrain::from_map(&dungeon, dungeon_set);
    assert_eq!((terrain.width, terrain.height), (40, 30));

    // Measured over work/all: Dungeon has 348 cells with at least one blocked
    // direction; SampleMapA has none. This is real tileset data, not a fixture.
    let mut blocked_cells = 0;
    let mut anchor: Option<(i32, i32, u8)> = None;
    for y in 0..terrain.height {
        for x in 0..terrain.width {
            let mut any_blocked = false;
            for step in 0..4u8 {
                if terrain.is_blocked(x, y, step) {
                    any_blocked = true;
                    let (dx, dy) = match step {
                        0 => (0, 1),
                        1 => (-1, 0),
                        2 => (1, 0),
                        _ => (0, -1),
                    };
                    let start = (x + dx, y + dy);
                    if start.0 >= 0
                        && start.1 >= 0
                        && start.0 < terrain.width
                        && start.1 < terrain.height
                        && anchor.is_none()
                    {
                        anchor = Some((start.0, start.1, step));
                    }
                }
            }
            if any_blocked {
                blocked_cells += 1;
            }
        }
    }
    assert_eq!(blocked_cells, 348, "Dungeon passability grid changed");

    let sample_terrain = Terrain::from_map(&sample, sample_set);
    let sample_blocked: usize = (0..sample_terrain.height)
        .flat_map(|y| (0..sample_terrain.width).map(move |x| (x, y)))
        .map(|(x, y)| {
            (0..4u8)
                .filter(|step| sample_terrain.is_blocked(x, y, *step))
                .count()
        })
        .sum();
    assert_eq!(sample_blocked, 0, "SampleMapA must stay fully passable");

    // Drive the movement core against one real Dungeon wall.
    let (start_x, start_y, step) = anchor.expect("Dungeon has an approachable wall");
    let direction = match step {
        0 => 8,
        1 => 6,
        2 => 4,
        _ => 2,
    };
    let mut runtime = MapRuntime::new();
    runtime.set_terrain(terrain);
    runtime.place_leader(start_x, start_y, None);
    assert!(
        !runtime.try_move_leader(direction),
        "a real Dungeon wall tile must block the step"
    );
    let stayed = runtime.leader().unwrap();
    assert_eq!((stayed.x, stayed.y), (start_x, start_y));
}
