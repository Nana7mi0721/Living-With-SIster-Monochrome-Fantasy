//! M11 P1: held-direction walking and NPC action-key interaction.
//!
//! The shipped game is point-and-click, but the map runtime already has exact
//! tile movement and contact boxes. These tests pin the frame-level input contract:
//! a held direction takes one whole-tile step immediately and then one every
//! `WALK_STEP_FRAMES` frames, a blocked tap still turns the leader, and a fresh
//! confirm press starts the adjacent active action-key event.
//!
//! Synthetic maps are used because the shipped SampleMapA is all-passable and has
//! no image-bearing action event in its open floor. Skipped when the unpacked data
//! root is absent, like the other integration tests.

use std::path::PathBuf;

use wolf_format::command::{Command, RouteData, Slot, Tail};
use wolf_format::mps::{Map, MapEvent, MapEventPage};
use wolf_game::Game;

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

fn page(trigger: u8, behavior_options: u8, image: bool, commands: Vec<Slot>) -> MapEventPage {
    MapEventPage {
        graphic_chip_id: if image { 1 } else { u32::MAX },
        graphic: if image {
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
            ..RouteData::default()
        },
        commands,
        features: 3,
        shadow_graphic: 0,
        collision_width: 0,
        collision_height: 0,
        footer: 0x7A,
    }
}

fn free_map(width: u32, height: u32) -> Map {
    Map {
        version_header: 0,
        version: 0,
        name: String::new(),
        tileset_index: 0,
        width,
        height,
        has_tile_data: false,
        layers: Vec::new(),
        events: Vec::new(),
        footer: 0x66,
    }
}

fn map_with_event(width: u32, height: u32, id: u32, x: u32, y: u32, page: MapEventPage) -> Map {
    let mut map = free_map(width, height);
    map.events.push(MapEvent {
        id,
        name: "talker".to_owned(),
        x,
        y,
        pages: vec![page],
    });
    map
}

fn game_with(map: &Map) -> Option<Game> {
    let mut game = open_game()?;
    game.auto_events.set_enabled(false);
    game.start_pages(map, None);
    Some(game)
}

#[test]
fn held_direction_walks_one_tile_immediately_and_then_every_eight_frames() {
    let map = free_map(20, 20);
    let Some(mut game) = game_with(&map) else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    game.place_leader(2, 2, 2);
    game.key_bits = wolf_game::key::RIGHT;
    for _ in 0..25 {
        game.frame();
    }
    let leader = game.runtime.leader().expect("leader remains placed");
    // 25 frames = steps on frames 1, 9, 17 and 25, then no fifth step yet.
    assert_eq!((leader.x, leader.y), (6, 2), "four right steps");
    assert_eq!((leader.precise_x, leader.precise_y), (12, 4));
    assert_eq!(
        leader.direction,
        Some(6),
        "facing follows the held direction"
    );
}

#[test]
fn a_blocked_tap_turns_in_place_and_holding_into_a_wall_never_moves() {
    let map = free_map(20, 20);
    let Some(mut game) = game_with(&map) else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    game.place_leader(0, 0, 2);
    game.key_bits = wolf_game::key::LEFT;
    for _ in 0..20 {
        game.frame();
    }
    let leader = game.runtime.leader().expect("leader remains placed");
    assert_eq!((leader.x, leader.y), (0, 0), "map edge blocks the step");
    assert_eq!((leader.precise_x, leader.precise_y), (0, 0));
    assert_eq!(leader.direction, Some(4), "the tap still turns the leader");

    game.key_bits = wolf_game::key::UP;
    game.frame();
    let leader = game.runtime.leader().expect("leader remains placed");
    assert_eq!((leader.x, leader.y), (0, 0));
    assert_eq!(leader.direction, Some(8), "the second blocked tap turns up");
}

#[test]
fn a_fresh_confirm_press_starts_an_adjacent_image_action_event() {
    // The page is an image event with slip-through OFF, so the official rule lets
    // the player talk to it from the adjacent tile. The command writes global
    // variable 123 = 1; reading it after the frame proves the page actually ran.
    let mut talk = page(0, 0x10, true, vec![cmd(121, &[2_000_123, 1, 0, 0])]);
    talk.route.behavior_options = 0x10;
    let map = map_with_event(20, 20, 7, 2, 1, talk);
    let Some(mut game) = game_with(&map) else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    game.place_leader(1, 1, 6);
    assert_eq!(game.vm.vars.get(123), 0);

    game.key_bits = wolf_game::key::OK;
    game.frame();
    game.key_bits = 0;
    assert_eq!(
        game.vm.vars.get(123),
        1,
        "the action page ran through opcode 121"
    );
}

#[test]
fn the_action_key_ignores_the_wrong_facing_and_a_passable_body_needs_overlap() {
    let mut talk = page(0, 0x10, true, vec![cmd(121, &[2_000_124, 1, 0, 0])]);
    talk.route.behavior_options = 0x10;
    let map = map_with_event(20, 20, 7, 2, 1, talk);
    let Some(mut game) = game_with(&map) else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    game.place_leader(1, 1, 4);
    assert!(game.interact_action_key().is_empty(), "facing away");
    assert_eq!(game.vm.vars.get(124), 0);

    // A no-image event on a passable tile does not reach from the adjacent tile:
    // the body itself has to be stood on.
    let no_image = page(0, 0, false, vec![cmd(121, &[2_000_125, 1, 0, 0])]);
    let map = map_with_event(20, 20, 8, 5, 5, no_image);
    let Some(mut game) = game_with(&map) else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    game.place_leader(4, 5, 6);
    assert!(
        game.interact_action_key().is_empty(),
        "no-image event adjacent must not fire"
    );
    game.place_leader(5, 5, 2);
    assert_eq!(
        game.interact_action_key().len(),
        1,
        "standing on the event fires"
    );
}

#[test]
fn diagonal_held_input_uses_the_eight_way_step_core() {
    let map = free_map(20, 20);
    let Some(mut game) = game_with(&map) else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    game.place_leader(4, 4, 2);
    game.key_bits = wolf_game::key::UP | wolf_game::key::RIGHT;
    game.frame();
    let leader = game.runtime.leader().expect("leader remains placed");
    assert_eq!((leader.x, leader.y), (5, 3), "one diagonal tile");
    assert_eq!((leader.precise_x, leader.precise_y), (10, 6));
    assert_eq!(leader.direction, Some(9), "up-right facing");
}
