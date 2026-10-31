//! M7 W4 map-graph edge cases.
//!
//! Three behaviours the generic map audit did not pin:
//!
//! 1. Map002's 4 teleports store the map id in the *fourth* operand (0 = TitleMap),
//!    not in the first: the leading -2 is the move *target* (the player), while the
//!    x/y coordinates are read from variable 7 and self[105] (var8 - 4).
//! 2. Map024 and Map031 ship no tile layer (has_tile_data == false -> 0 layers), so
//!    they render over whatever is already on screen instead of painting a floor.
//! 3. The MapTree root's children (map ids 3/31/24) are the only new-game ordering the
//!    data gives; the actual start coordinate lives in Map003 event 1 (opcode 130 to
//!    SampleMapA at 62,38), and no engine code reads MapTree.dat yet.

use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::maptree::MapTree;
use wolf_format::mps::Map;
use wolf_game::{Atlas, Game};
use wolf_vm::magic::{Magic, MagicKind};

const OPCODE_TELEPORT: u32 = 130;

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/full".to_owned());
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return path;
    }
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    base.join(path)
}

fn open_game() -> Option<Game> {
    match Game::new(data_root()) {
        Ok(game) => Some(game),
        Err(err) => {
            eprintln!("skipping: {err}");
            None
        }
    }
}

fn load_map(name: &str) -> Option<Map> {
    let path = data_root().join("MapData").join(name);
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipping: {name} not present at {}", path.display());
        return None;
    };
    Some(Map::parse(&bytes).expect("map parses"))
}

fn teleport_args(slot: &Slot) -> Option<[i32; 5]> {
    let Slot::Command(cmd) = slot else {
        return None;
    };
    if cmd.opcode != OPCODE_TELEPORT {
        return None;
    }
    let words: Vec<i32> = cmd
        .params
        .chunks_exact(4)
        .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    words.try_into().ok()
}

/// (event id, page index, pc, raw teleport args).
fn teleport_sites(map: &Map) -> Vec<(u32, usize, usize, [i32; 5])> {
    let mut sites = Vec::new();
    for event in &map.events {
        for (page_index, page) in event.pages.iter().enumerate() {
            for (pc, slot) in page.commands.iter().enumerate() {
                if let Some(args) = teleport_args(slot) {
                    sites.push((event.id, page_index, pc, args));
                }
            }
        }
    }
    sites
}

fn run_until_teleport(game: &mut Game, frames: usize) -> Option<String> {
    for _ in 0..frames {
        let _ = game.frame();
        if let Some((key, _)) = game
            .diagnostics
            .iter()
            .find(|(key, _)| key.starts_with("130 Teleport:"))
        {
            return Some(key.clone());
        }
    }
    None
}

#[test]
fn map002_teleport_uses_an_ordinary_map_id_and_variable_coordinates() {
    let Some(map) = load_map("Map002.mps") else {
        return;
    };
    let sites = teleport_sites(&map);
    assert_eq!(
        sites,
        vec![
            (3, 0, 7, [-2, 2_000_007, 1_100_005, 0, 1]),
            (3, 0, 10, [-2, 2_000_007, 1_100_005, 0, 1]),
            (5, 0, 7, [-2, 2_000_007, 1_100_005, 0, 1]),
            (5, 0, 10, [-2, 2_000_007, 1_100_005, 0, 1]),
        ],
        "four identical teleports, all on Map002 events 3 and 5"
    );

    for (_, _, _, args) in &sites {
        // arg0 is the target, not the destination map.
        assert_eq!(args[0], -2, "target = -2 (the player)");
        assert_eq!(args[3], 0, "destination map id is operand 3 = 0");
        assert_eq!(args[4], 1, "options");
        let x = Magic::decode(args[1]);
        assert_eq!(x.kind(), MagicKind::Var);
        assert_eq!(x.index(), 7, "x comes from global variable 7");
        let y = Magic::decode(args[2]);
        assert_eq!(y.kind(), MagicKind::SelfVar);
        assert_eq!(y.index(), 105, "y comes from self[105] (var8 - 4)");
    }

    let Some(mut game) = open_game() else {
        return;
    };
    assert_eq!(
        game.databases.map_path(0),
        Some("MapData/TitleMap.mps"),
        "map id 0 is TitleMap"
    );
    assert_eq!(
        game.databases.map_path(-2),
        None,
        "-2 cannot be a map id: map_path rejects negatives"
    );

    game.start_map_event_by_name("Map002", 3)
        .expect("Map002 event 3 starts");
    let key = run_until_teleport(&mut game, 120).expect("Map002 queues its teleport");
    assert!(
        key.contains("MapData/TitleMap.mps"),
        "teleport should resolve to TitleMap, got: {key}"
    );
    assert!(!key.contains("no map registered"), "unexpected drop: {key}");
    println!("Map002 teleport diagnostic: {key}");
}

#[test]
fn map024_and_map031_have_no_tile_layer_so_they_draw_no_tile_quads() {
    for name in ["Map024.mps", "Map031.mps"] {
        let Some(map) = load_map(name) else {
            return;
        };
        assert!(!map.has_tile_data, "{name} has no tile block");
        assert!(map.layers.is_empty(), "{name} decodes to zero tile layers");

        let Some(mut game) = open_game() else {
            return;
        };
        // Do not step frames: Map031's load event would try to teleport to an
        // unregistered map id (27) and muddy this pure tile-layer check.
        game.start_pages(&map, None);
        let tilemap = game
            .tilemap
            .as_ref()
            .unwrap_or_else(|| panic!("{name} tileset is registered"));
        assert_eq!(
            tilemap.layer_count(),
            0,
            "{name} must expose zero tile layers"
        );
        let mut atlas = Atlas::new();
        assert!(
            game.tile_quads(&mut atlas).is_empty(),
            "{name} must emit no tile quads"
        );
    }
}

#[test]
fn maptree_names_the_new_game_hook_and_map003_is_the_real_start_coordinate() {
    let path = data_root().join("BasicData").join("MapTree.dat");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipping: MapTree.dat not present at {}", path.display());
        return;
    };
    let tree = MapTree::parse(&bytes).expect("MapTree.dat parses");
    assert_eq!(tree.entries.len(), 50, "measured entry count");
    let children = tree.children_of(0);
    for id in [3u32, 31, 24] {
        assert!(
            children.contains(&id),
            "map {id} is a direct child of the root"
        );
        assert!(tree.contains_map(id), "map {id} is present");
    }

    // MapTree stores only parent/map_id (no x/y). The new-game hook is therefore the
    // autorun event on the map it orders first: map id 3 = Map003, event 1
    // "ニューゲーム処理", opcode 130 [-1, 62, 38, 1, 1].
    let Some(map003) = load_map("Map003.mps") else {
        return;
    };
    assert_eq!(
        teleport_sites(&map003),
        vec![(1, 0, 28, [-1, 62, 38, 1, 1])],
        "Map003 event 1 is the single new-game teleport"
    );

    let Some(mut game) = open_game() else {
        return;
    };
    game.start_map_event_by_name("Map003", 1)
        .expect("Map003 event 1 starts");
    let key = run_until_teleport(&mut game, 300).expect("new-game event teleports");
    assert!(
        key.contains("MapData/SampleMapA.mps"),
        "new game should land in SampleMapA, got: {key}"
    );
    println!("Map003 new-game teleport diagnostic: {key}");
}
