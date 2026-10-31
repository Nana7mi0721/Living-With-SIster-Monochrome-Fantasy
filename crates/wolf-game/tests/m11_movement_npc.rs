//! M10 residual: eight-direction movement, diagonal collision and NPC queries.
//!
//! The core slice (docs/research/M9_MOVEMENT.md, M9_MAP_CONTACTS.md) kept
//! try_move_leader/move_leader cardinal-only. This file exercises the added
//! try_move_leader_8 / move_leader_8 / try_move_leader_contacts_8 entry points
//! plus the events_at/event_at/npc_at/contact_events_at queries.
//!
//! Slice report: docs/research/M11_MOVEMENT_NPC.md.

use std::path::PathBuf;

use wolf_format::command::{RouteData, Slot};
use wolf_format::mps::{Map, MapEvent, MapEventPage};
use wolf_game::map_runtime::{MapRuntime, Terrain, TriggerRequest};
use wolf_vm::{ProgramSet, Vm};

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

fn runtime_for(map: &Map) -> Option<MapRuntime> {
    let tilesets = read_tilesets()?;
    let set = tilesets.get(map.tileset_index)?;
    let mut runtime = MapRuntime::collect(map);
    runtime.set_terrain(Terrain::from_map(map, set));
    Some(runtime)
}

fn blank_page(
    trigger: u8,
    behavior_options: u8,
    has_image: bool,
    collision_width: u8,
    collision_height: u8,
) -> MapEventPage {
    MapEventPage {
        graphic_chip_id: if has_image { 1 } else { 0xFFFF_FFFF },
        graphic: if has_image {
            "hero.png".to_owned()
        } else {
            String::new()
        },
        direction: 2,
        frame: 0,
        opacity: 255,
        display_type: 0,
        trigger,
        trigger_switches: [0; 4],
        trigger_variables: [0; 4],
        trigger_values: [0; 4],
        route: RouteData {
            behavior_options,
            ..Default::default()
        },
        commands: vec![Slot::Blank],
        features: 3,
        shadow_graphic: 0,
        collision_width,
        collision_height,
        footer: 0x7A,
    }
}

fn event(id: u32, x: u32, y: u32, page: MapEventPage) -> MapEvent {
    MapEvent {
        id,
        name: format!("event{id}"),
        x,
        y,
        pages: vec![page],
    }
}

fn map_with_events(width: u32, height: u32, events: Vec<MapEvent>) -> Map {
    Map {
        version_header: 0,
        version: 0,
        name: String::new(),
        tileset_index: 0,
        width,
        height,
        has_tile_data: false,
        layers: Vec::new(),
        events,
        footer: 0x66,
    }
}

fn started(runtime: &mut MapRuntime) -> Vm {
    let mut vm = Vm::new(ProgramSet::default());
    runtime.start_pending(&mut vm);
    vm
}

#[test]
fn eight_direction_steps_use_wolf_keypad_facing_and_precise_units() {
    let mut runtime = MapRuntime::new();
    runtime.set_terrain(Terrain::new(5, 5));

    // direction, expected tile, expected precise (half-tiles), expected facing.
    let cases = [
        (8, 2, 1, 4, 2, 8),
        (9, 3, 1, 6, 2, 9),
        (6, 3, 2, 6, 4, 6),
        (3, 3, 3, 6, 6, 3),
        (2, 2, 3, 4, 6, 2),
        (1, 1, 3, 2, 6, 1),
        (4, 1, 2, 2, 4, 4),
        (7, 1, 1, 2, 2, 7),
    ];
    for (direction, x, y, precise_x, precise_y, facing) in cases {
        runtime.place_leader(2, 2, None);
        assert!(
            runtime.try_move_leader_8(direction),
            "direction {direction} should step from an open tile"
        );
        let leader = runtime.leader().unwrap();
        assert_eq!((leader.x, leader.y), (x, y), "direction {direction} tile");
        assert_eq!(
            (leader.precise_x, leader.precise_y),
            (precise_x, precise_y),
            "direction {direction} precise"
        );
        assert_eq!(
            leader.direction,
            Some(facing),
            "direction {direction} facing"
        );
    }

    // The axis form accepts one diagonal and reports the matching facing.
    runtime.place_leader(2, 2, None);
    assert!(runtime.move_leader_8(1, 1));
    let leader = runtime.leader().unwrap();
    assert_eq!((leader.x, leader.y, leader.direction), (3, 3, Some(3)));
    assert_eq!((leader.precise_x, leader.precise_y), (6, 6));

    // Non-unit and zero deltas are rejected without moving or turning.
    runtime.place_leader(2, 2, Some(4));
    assert!(!runtime.move_leader_8(2, 0));
    assert!(!runtime.move_leader_8(0, 0));
    let unchanged = runtime.leader().unwrap();
    assert_eq!(
        (unchanged.x, unchanged.y, unchanged.direction),
        (2, 2, Some(4))
    );
}

#[test]
fn a_solid_diagonal_target_blocks_the_step_but_still_turns() {
    let mut runtime = MapRuntime::new();
    let mut terrain = Terrain::new(5, 5);
    terrain.set_tile_blocked(3, 3, true);
    runtime.set_terrain(terrain);
    runtime.place_leader(2, 2, None);

    assert!(
        !runtime.try_move_leader_8(3),
        "solid target blocks diagonal"
    );
    let leader = runtime.leader().unwrap();
    assert_eq!((leader.x, leader.y), (2, 2));
    assert_eq!((leader.precise_x, leader.precise_y), (4, 4));
    assert_eq!(leader.direction, Some(3), "blocked step still turns");

    // A diagonal out of the map bounds is blocked the same way.
    runtime.place_leader(0, 0, None);
    assert!(!runtime.try_move_leader_8(7));
    assert_eq!(runtime.leader().unwrap().direction, Some(7));
}

#[test]
fn either_solid_orthogonal_corner_blocks_a_diagonal() {
    // Horizontal neighbour (3,2) is solid: the path to (3,3) would cut its corner.
    let mut runtime = MapRuntime::new();
    let mut terrain = Terrain::new(5, 5);
    terrain.set_tile_blocked(3, 2, true);
    runtime.set_terrain(terrain);
    runtime.place_leader(2, 2, None);
    assert!(
        !runtime.try_move_leader_8(3),
        "a solid horizontal corner blocks down-right"
    );
    assert_eq!(runtime.leader().map(|l| (l.x, l.y)), Some((2, 2)));
    assert_eq!(runtime.leader().unwrap().direction, Some(3));

    // Vertical neighbour (2,3) is solid: same diagonal is blocked from the other side.
    let mut runtime = MapRuntime::new();
    let mut terrain = Terrain::new(5, 5);
    terrain.set_tile_blocked(2, 3, true);
    runtime.set_terrain(terrain);
    runtime.place_leader(2, 2, None);
    assert!(
        !runtime.try_move_leader_8(3),
        "a solid vertical corner blocks down-right"
    );
    assert_eq!(runtime.leader().map(|l| (l.x, l.y)), Some((2, 2)));

    // With both corners free the same diagonal succeeds.
    let mut runtime = MapRuntime::new();
    runtime.set_terrain(Terrain::new(5, 5));
    runtime.place_leader(2, 2, None);
    assert!(runtime.try_move_leader_8(3));
    assert_eq!(runtime.leader().map(|l| (l.x, l.y)), Some((3, 3)));
}

#[test]
fn a_solid_event_blocks_the_diagonal_target_and_its_corners() {
    // Event in the destination tile blocks the step.
    let target_event = map_with_events(5, 5, vec![event(7, 3, 3, blank_page(0, 0, false, 0, 0))]);
    let mut runtime = MapRuntime::collect(&target_event);
    let _vm = started(&mut runtime);
    runtime.place_leader(2, 2, None);
    assert!(
        !runtime.try_move_leader_8(3),
        "event target blocks diagonal"
    );
    assert_eq!(runtime.leader().map(|l| (l.x, l.y)), Some((2, 2)));

    // Event on the horizontal corner blocks even though the target is empty.
    let corner_event = map_with_events(5, 5, vec![event(7, 3, 2, blank_page(0, 0, false, 0, 0))]);
    let mut runtime = MapRuntime::collect(&corner_event);
    let _vm = started(&mut runtime);
    runtime.place_leader(2, 2, None);
    assert!(
        !runtime.try_move_leader_8(3),
        "event corner blocks diagonal"
    );

    // Slip-through makes the same event walkable, so the diagonal succeeds.
    let slip = map_with_events(5, 5, vec![event(7, 3, 3, blank_page(0, 0x08, true, 0, 0))]);
    let mut runtime = MapRuntime::collect(&slip);
    let _vm = started(&mut runtime);
    runtime.place_leader(2, 2, None);
    assert!(
        runtime.try_move_leader_8(3),
        "slip-through event is walkable"
    );
    assert_eq!(runtime.leader().map(|l| (l.x, l.y)), Some((3, 3)));
}

#[test]
fn diagonal_movement_respects_real_map_passability() {
    let Some(map) = read_map("SampleMapB.mps") else {
        eprintln!("skipping: no SampleMapB.mps");
        return;
    };
    let Some(mut runtime) = runtime_for(&map) else {
        eprintln!("skipping: no TileSetData.dat");
        return;
    };
    let terrain = runtime.terrain().expect("terrain installed");
    assert!(terrain.is_passable_cell(7, 10), "(7,10) is open floor");
    assert!(terrain.is_solid_cell(8, 11), "(8,11) is a real wall");

    runtime.place_leader(7, 10, None);
    assert!(
        !runtime.try_move_leader_8(3),
        "the real wall at the destination blocks the diagonal"
    );
    assert_eq!(runtime.leader().map(|l| (l.x, l.y)), Some((7, 10)));
}

#[test]
fn npc_and_contact_queries_report_active_event_ids_in_order() {
    let map = map_with_events(
        5,
        3,
        vec![
            event(7, 2, 0, blank_page(0, 0, true, 0, 0)),
            event(3, 2, 0, blank_page(0, 0, false, 0, 0)),
            event(5, 3, 0, blank_page(0, 0, true, 0, 0)),
        ],
    );
    let mut runtime = MapRuntime::collect(&map);
    let _vm = started(&mut runtime);

    assert_eq!(runtime.events_at(2, 0), vec![3, 7]);
    assert_eq!(runtime.event_at(2, 0), Some(3), "smallest id wins");
    assert_eq!(runtime.npc_at(2, 0), Some(7), "image-bearing event only");
    assert_eq!(runtime.npc_at(3, 0), Some(5));
    assert_eq!(runtime.npc_at(2, 1), None);
    assert_eq!(runtime.contact_events_at(2, 0), vec![3, 7]);

    // Real data check: SampleMapB event 0 covers tile (7,8) via its contact box.
    let Some(real_map) = read_map("SampleMapB.mps") else {
        eprintln!("skipping: no SampleMapB.mps");
        return;
    };
    let Some(mut real_runtime) = runtime_for(&real_map) else {
        eprintln!("skipping: no TileSetData.dat");
        return;
    };
    let _vm = started(&mut real_runtime);
    assert!(real_runtime.contact_events_at(7, 8).contains(&0));
    assert!(!real_runtime.contact_events_at(7, 7).contains(&0));
    assert_eq!(real_runtime.event_at(7, 11), Some(0));
}

#[test]
fn a_diagonal_step_still_fires_player_touch_on_a_blocking_event() {
    let map = map_with_events(5, 3, vec![event(7, 2, 1, blank_page(3, 0, false, 0, 0))]);
    let mut runtime = MapRuntime::collect(&map);
    let _vm = started(&mut runtime);
    runtime.place_leader(1, 0, None);

    let (moved, requests) = runtime.try_move_leader_contacts_8(3);
    assert!(!moved, "the event's own tile blocks the diagonal");
    assert_eq!(runtime.leader().map(|l| (l.x, l.y)), Some((1, 0)));
    assert_eq!(runtime.leader().unwrap().direction, Some(3));
    assert_eq!(requests, vec![TriggerRequest { event: 7, page: 0 }]);
}
