//! M9 W3-c: map event trigger dispatch core.
//!
//! The runtime already knows which page of an event is active; this slice turns
//! that ownership metadata into a testable contact query. It covers the two
//! player-driven trigger kinds that can be proven from shipped data:
//! action-key (trigger 0) and player-touch (trigger 3). Event-touch (trigger 4)
//! is decoded but never fires because no shipped page uses it and the official
//! hitbox geometry for its quarter-tile overlap is not specified.
//!
//! Slice: docs/research/M9_MAP_TRIGGERS.md.

use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::mps::{Map, MapEvent, MapEventPage};
use wolf_game::map_runtime::{MapRuntime, Terrain, Trigger, TriggerRequest};
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

fn runtime_for(map: &Map, tileset_index: u32) -> Option<MapRuntime> {
    let tilesets = read_tilesets()?;
    let set = tilesets.get(tileset_index)?;
    let mut runtime = MapRuntime::collect(map);
    runtime.set_terrain(Terrain::from_map(map, set));
    Some(runtime)
}

fn blank_page(trigger: u8, behavior_options: u8, has_image: bool) -> MapEventPage {
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
        route: wolf_format::command::RouteData {
            behavior_options,
            ..Default::default()
        },
        commands: vec![Slot::Blank],
        features: 3,
        shadow_graphic: 0,
        collision_width: 0,
        collision_height: 0,
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
            name: "trigger".to_owned(),
            x,
            y,
            pages: vec![page],
        }],
        footer: 0x66,
    }
}

#[test]
fn action_key_activates_only_the_smallest_overlapping_event_on_a_passable_body() {
    let Some(map) = read_map("SampleMapA.mps") else {
        eprintln!("skipping: no SampleMapA.mps");
        return;
    };
    let Some(mut runtime) = runtime_for(&map, map.tileset_index) else {
        eprintln!("skipping: no TileSetData.dat");
        return;
    };
    let mut vm = Vm::new(ProgramSet::default());
    // SampleMapA events 11/14/18/21/22 expose their action-key page 1 only once
    // global variable 302 is 1; page 0 is a parallel page with a different guard.
    vm.vars.set(302, 1);
    runtime.start_pending(&mut vm);

    let actor = runtime.actor(11).expect("event 11 is on the map");
    assert_eq!((actor.x, actor.y), (3, 35));
    assert_eq!(
        actor.active_page,
        Some(1),
        "the action page must be selected"
    );

    // Body chip is autotile 1 with passability 0, and the page has no image, so
    // the official implicit rule requires standing ON the event.
    assert_eq!(
        runtime.try_trigger((3, 35), 2, true, false),
        vec![TriggerRequest { event: 11, page: 1 }]
    );
    assert!(
        runtime.try_trigger((2, 35), 6, true, false).is_empty(),
        "a passable, image-less event cannot be started from the adjacent tile"
    );
}

#[test]
fn player_touch_on_a_solid_body_starts_from_the_adjacent_tile() {
    let Some(mut map) = read_map("SampleMapB.mps") else {
        eprintln!("skipping: no SampleMapB.mps");
        return;
    };
    // The real SampleMapB tileset marks (8, 11) solid (all four entry directions
    // blocked). Put the trigger-3 event there so the body is genuinely solid.
    map.events[0].x = 8;
    map.events[0].y = 11;
    let Some(mut runtime) = runtime_for(&map, map.tileset_index) else {
        eprintln!("skipping: no TileSetData.dat");
        return;
    };
    let mut vm = Vm::new(ProgramSet::default());
    runtime.start_pending(&mut vm);

    let actor = runtime.actor(0).expect("event 0 is on the map");
    assert_eq!((actor.x, actor.y), (8, 11));
    assert_eq!(actor.active_page, Some(0));
    assert!(
        runtime
            .terrain()
            .expect("real terrain installed")
            .is_solid_cell(8, 11),
        "SampleMapB (8, 11) is the measured solid tile"
    );

    assert_eq!(
        runtime.try_trigger((7, 11), 6, false, true),
        vec![TriggerRequest { event: 0, page: 0 }]
    );
    assert!(
        runtime.try_trigger((5, 11), 6, false, true).is_empty(),
        "two tiles away is out of contact"
    );
    assert!(
        runtime.try_trigger((7, 11), 6, true, false).is_empty(),
        "a player-touch page must not answer an action press"
    );
}

#[test]
fn player_touch_on_a_passable_body_requires_standing_on_it() {
    let Some(map) = read_map("SampleMapB.mps") else {
        eprintln!("skipping: no SampleMapB.mps");
        return;
    };
    let Some(mut runtime) = runtime_for(&map, map.tileset_index) else {
        eprintln!("skipping: no TileSetData.dat");
        return;
    };
    let mut vm = Vm::new(ProgramSet::default());
    runtime.start_pending(&mut vm);

    let actor = runtime.actor(2).expect("event 2 is on the map");
    assert_eq!((actor.x, actor.y), (11, 26));
    assert!(
        runtime
            .terrain()
            .expect("real terrain installed")
            .is_passable_cell(11, 26),
        "SampleMapB event 2 sits on a passable tile"
    );

    assert_eq!(
        runtime.try_trigger((11, 26), 2, false, true),
        vec![TriggerRequest { event: 2, page: 0 }]
    );
    assert!(
        runtime.try_trigger((10, 26), 6, false, true).is_empty(),
        "a passable, image-less body must be stood on"
    );
}

#[test]
fn trigger_kind_and_option_bits_decide_the_contact_reach() {
    // Image plus slip-through off: the event is solid for contact purposes, so
    // it can be started from the adjacent tile even on a passable map cell.
    let image_off = map_with_page(4, 4, 7, 2, 1, blank_page(0, 0x23, true));
    let mut runtime = MapRuntime::collect(&image_off);
    let mut vm = Vm::new(ProgramSet::default());
    runtime.start_pending(&mut vm);
    assert_eq!(
        runtime.try_trigger((1, 1), 6, true, false),
        vec![TriggerRequest { event: 7, page: 0 }]
    );

    // Same image, but slip-through on: now the player must overlap the event.
    let image_on = map_with_page(4, 4, 8, 2, 1, blank_page(0, 0x2B, true));
    let mut runtime = MapRuntime::collect(&image_on);
    let mut vm = Vm::new(ProgramSet::default());
    runtime.start_pending(&mut vm);
    assert!(runtime.try_trigger((1, 1), 6, true, false).is_empty());
    assert_eq!(
        runtime.try_trigger((2, 1), 2, true, false),
        vec![TriggerRequest { event: 8, page: 0 }]
    );

    // No image and a passable body: also requires standing on the event.
    let no_image = map_with_page(4, 4, 9, 2, 1, blank_page(0, 0, false));
    let mut runtime = MapRuntime::collect(&no_image);
    let mut vm = Vm::new(ProgramSet::default());
    runtime.start_pending(&mut vm);
    assert!(runtime.try_trigger((1, 1), 6, true, false).is_empty());
    assert_eq!(
        runtime.try_trigger((2, 1), 2, true, false),
        vec![TriggerRequest { event: 9, page: 0 }]
    );

    // Trigger 4 is decoded as EventTouch but is not fired by this tile-only core.
    let event_touch = map_with_page(4, 4, 10, 2, 1, blank_page(4, 0, false));
    let mut runtime = MapRuntime::collect(&event_touch);
    let mut vm = Vm::new(ProgramSet::default());
    runtime.start_pending(&mut vm);
    assert!(runtime.try_trigger((2, 1), 2, true, true).is_empty());
}

#[test]
fn no_shipped_page_uses_event_touch_but_the_kind_is_decoded() {
    assert_eq!(Trigger::from_byte(4), Trigger::EventTouch);
    assert!(!Trigger::from_byte(4).is_automatic());
    assert_eq!(Trigger::EventTouch.byte(), 4);

    let names = [
        "SampleMapA.mps",
        "SampleMapA_1.mps",
        "SampleMapB.mps",
        "Dungeon.mps",
        "Map002.mps",
        "Map003.mps",
        "Map024.mps",
        "Map031.mps",
        "TitleMap.mps",
    ];
    let mut pages = 0usize;
    let mut loaded = 0usize;
    for name in names {
        let Some(map) = read_map(name) else {
            continue;
        };
        loaded += 1;
        for event in &map.events {
            for page in &event.pages {
                pages += 1;
                assert_ne!(
                    page.trigger, 4,
                    "{name} event {} page uses the unproven event-touch trigger",
                    event.id
                );
            }
        }
    }
    assert!(loaded > 0, "skipping: no shipped maps available");
    assert_eq!(pages, 134, "shipped page census changed");
}
