//! M9 W3-c follow-up: event contact boxes, player start, contact triggers.
//!
//! The tile-only movement core (docs/research/M9_MOVEMENT.md) treated every
//! active event as blocking its own tile and never fired touch triggers. This
//! slice settles what the page collision-width/height fields actually do and
//! wires player-touch and event-touch to the leader's steps.
//!
//! Slice report: docs/research/M9_MAP_CONTACTS.md.

use std::path::PathBuf;

use wolf_format::command::{RouteData, Slot};
use wolf_format::mps::{Map, MapEvent, MapEventPage};
use wolf_game::map_runtime::{MapRuntime, Terrain, TriggerRequest};
use wolf_game::Game;
use wolf_vm::{ProgramSet, Vm};

/// The data root: WOLF_DATA_ROOT when set, otherwise work/all falling back to work/full.
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

fn started_runtime(map: &Map) -> Option<(MapRuntime, Vm)> {
    let mut runtime = runtime_for(map)?;
    let mut vm = Vm::new(ProgramSet::default());
    runtime.start_pending(&mut vm);
    Some((runtime, vm))
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

fn map_with_page(width: u32, height: u32, id: u32, x: u32, y: u32, page: MapEventPage) -> Map {
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
            name: "contact".to_owned(),
            x,
            y,
            pages: vec![page],
        }],
        footer: 0x66,
    }
}

#[test]
fn real_contact_boxes_use_the_half_tile_extension_unit() {
    let Some(map) = read_map("SampleMapB.mps") else {
        eprintln!("skipping: no SampleMapB.mps");
        return;
    };
    let Some((runtime, _vm)) = started_runtime(&map) else {
        eprintln!("skipping: no TileSetData.dat");
        return;
    };
    // Event 0 is the encounter event at (7, 11), square hit box, X=8 Y=6.
    // Base 1x1 plus 8 half-tiles each side gives the inclusive tile rectangle
    // x 3..=11, y 8..=14.
    assert_eq!(runtime.contact_box(0), Some((3, 8, 11, 14)));
    // Event 2 is the teleport event at (11, 26), square, X=4 Y=0 -> x 9..=13, y 26.
    assert_eq!(runtime.contact_box(2), Some((9, 26, 13, 26)));
}

#[test]
fn walking_into_the_encounter_box_fires_player_touch_and_still_moves() {
    let Some(map) = read_map("SampleMapB.mps") else {
        eprintln!("skipping: no SampleMapB.mps");
        return;
    };
    let Some((mut runtime, _vm)) = started_runtime(&map) else {
        eprintln!("skipping: no TileSetData.dat");
        return;
    };
    runtime.place_leader(7, 7, None);
    let (moved, requests) = runtime.try_move_leader_contacts(2); // down into y=8
    assert!(
        moved,
        "an image-less encounter event must not block movement"
    );
    assert_eq!(runtime.leader().map(|l| (l.x, l.y)), Some((7, 8)));
    assert_eq!(requests, vec![TriggerRequest { event: 0, page: 0 }]);

    // A step that stays outside the contact box fires nothing.
    runtime.place_leader(7, 5, None);
    let (moved, requests) = runtime.try_move_leader_contacts(2); // down to y=6
    assert!(moved);
    assert!(requests.is_empty());
}

#[test]
fn player_touch_on_a_passable_body_fires_only_when_entering() {
    let Some(map) = read_map("SampleMapB.mps") else {
        eprintln!("skipping: no SampleMapB.mps");
        return;
    };
    let Some((mut runtime, _vm)) = started_runtime(&map) else {
        eprintln!("skipping: no TileSetData.dat");
        return;
    };
    runtime.place_leader(8, 26, None);
    let (moved, requests) = runtime.try_move_leader_contacts(6); // right into x=9
    assert!(moved);
    assert_eq!(requests, vec![TriggerRequest { event: 2, page: 0 }]);

    // A second step stays inside the box, so player-touch does not repeat.
    let (moved, requests) = runtime.try_move_leader_contacts(6);
    assert!(moved);
    assert!(requests.is_empty());
}

#[test]
fn a_blocked_step_into_an_event_tile_still_fires_player_touch() {
    // Base hit box is the event's own tile. An event without slip-through blocks
    // it even when it has no image, but the blocked attempt still fires touch.
    let map = map_with_page(5, 1, 7, 2, 0, blank_page(3, 0, false, 0, 0));
    let mut runtime = MapRuntime::collect(&map);
    let mut vm = Vm::new(ProgramSet::default());
    runtime.start_pending(&mut vm);
    runtime.place_leader(1, 0, None);
    let (moved, requests) = runtime.try_move_leader_contacts(6);
    assert!(!moved, "the event's own tile blocks movement");
    assert_eq!(runtime.leader().map(|l| (l.x, l.y)), Some((1, 0)));
    assert_eq!(requests, vec![TriggerRequest { event: 7, page: 0 }]);

    // Slip-through lets the leader enter the tile; entering still fires touch.
    let map = map_with_page(5, 1, 7, 2, 0, blank_page(3, 0x08, true, 0, 0));
    let mut runtime = MapRuntime::collect(&map);
    let mut vm = Vm::new(ProgramSet::default());
    runtime.start_pending(&mut vm);
    runtime.place_leader(1, 0, None);
    let (moved, requests) = runtime.try_move_leader_contacts(6);
    assert!(moved, "slip-through lets the leader enter the event tile");
    assert_eq!(runtime.leader().map(|l| (l.x, l.y)), Some((2, 0)));
    assert_eq!(requests, vec![TriggerRequest { event: 7, page: 0 }]);
}

#[test]
fn event_touch_repeats_while_overlapping_a_passable_event() {
    let map = map_with_page(5, 1, 7, 2, 0, blank_page(4, 0x08, true, 0, 0));
    let mut runtime = MapRuntime::collect(&map);
    let mut vm = Vm::new(ProgramSet::default());
    runtime.start_pending(&mut vm);
    runtime.place_leader(1, 0, None);
    let (moved, requests) = runtime.try_move_leader_contacts(6);
    assert!(moved, "slip-through lets the leader enter the event tile");
    assert_eq!(requests, vec![TriggerRequest { event: 7, page: 0 }]);
    // Still overlapping: event touch fires again.
    assert_eq!(
        runtime.try_contact_triggers((2, 0), (2, 0)),
        vec![TriggerRequest { event: 7, page: 0 }]
    );
}

#[test]
fn game_records_the_player_start_separately_from_the_live_leader() {
    let Some(mut game) = Game::new(data_root()).ok() else {
        eprintln!("skipping: no data root");
        return;
    };
    assert!(game.player_start().is_none());
    game.set_player_start(10, 12, 2);
    let start = game.player_start().expect("start must be recorded");
    assert_eq!((start.x, start.y), (10, 12));
    assert_eq!((start.precise_x, start.precise_y), (20, 24));
    assert_eq!(start.direction, Some(2));
}
