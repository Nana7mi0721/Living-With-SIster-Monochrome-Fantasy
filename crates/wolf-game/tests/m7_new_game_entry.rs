//! W4: the real retail new-game entry.
//!
//! A new game is not a single opcode. The retail player path is:
//!
//!   boot -> TitleMap.mps event 2 (splashes + title art)
//!        -> confirm -> CE42 "オリジナルタイトル画面Ａ" menu ["1","2","3"]
//!        -> answer option 1 -> CE42 pc118 op130 [-1, 60, 34, 3, 1]
//!             (target = hero, x = 60, y = 34, map id = 3 = Map003.mps, options = 1)
//!        -> Map003.mps event 1 "ニューゲーム処理" pc28 op130 [-1, 62, 38, 1, 1]
//!             (target = hero, x = 62, y = 38, map id = 1 = SampleMapA.mps, options = 1)
//!        -> SampleMapA.mps event 0 new-game processing / opening.
//!
//! MapTree.dat is NOT part of this: it has 50 entries but stores only parent/map_id
//! (no x/y), and no engine code reads it. The coordinates come from the two opcode
//! 130 operands above, which is why this test pins them rather than MapTree.

use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_format::mps::Map;
use wolf_game::key;
use wolf_game::Game;

const TITLE_TO_MAP003: &str = "130 Teleport: MapData/Map003.mps at (60, 34) target -1 options 0x1";
const MAP003_TO_SAMPLE: &str =
    "130 Teleport: MapData/SampleMapA.mps at (62, 38) target -1 options 0x1";

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

fn teleport_operands(slot: &Slot) -> Option<[i32; 5]> {
    let Slot::Command(cmd) = slot else {
        return None;
    };
    if cmd.opcode != 130 {
        return None;
    }
    let words: Vec<i32> = cmd
        .params
        .chunks_exact(4)
        .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    words.try_into().ok()
}

fn teleports(commands: &[Slot]) -> Vec<(usize, [i32; 5])> {
    commands
        .iter()
        .enumerate()
        .filter_map(|(pc, slot)| teleport_operands(slot).map(|args| (pc, args)))
        .collect()
}

/// Deterministic: the two hops live at fixed instruction slots in the shipped data.
#[test]
fn the_new_game_hops_have_exact_static_operands() {
    let root = data_root();

    let Ok(ce_bytes) = std::fs::read(root.join("BasicData").join("CommonEvent.dat")) else {
        eprintln!("skipping: CommonEvent.dat missing");
        return;
    };
    let ce = CommonEventData::parse(&ce_bytes).expect("CommonEvent.dat parses");
    let ce42 = ce.event(42).expect("CE42 exists");
    assert!(
        ce42.title.contains("タイトル"),
        "CE42 should be the sample title screen, got {:?}",
        ce42.title
    );
    let menu: Vec<String> = ce42
        .commands
        .iter()
        .find_map(|slot| match slot {
            Slot::Command(cmd) if cmd.opcode == 102 => Some(cmd.strings.clone()),
            _ => None,
        })
        .expect("CE42 draws a choice menu");
    assert_eq!(menu, ["1", "2", "3"], "the retail title menu options");
    assert_eq!(
        teleports(&ce42.commands),
        vec![(118, [-1, 60, 34, 3, 1])],
        "CE42 pc118 is the single title -> Map003 hop"
    );

    let Ok(map_bytes) = std::fs::read(root.join("MapData").join("Map003.mps")) else {
        eprintln!("skipping: Map003.mps missing");
        return;
    };
    let map003 = Map::parse(&map_bytes).expect("Map003 parses");
    let ev1 = map003
        .events
        .iter()
        .find(|event| event.id == 1)
        .expect("Map003 has the new-game processing event");
    assert!(
        ev1.name.contains("ニューゲーム"),
        "Map003 ev1 should be new-game processing, got {:?}",
        ev1.name
    );
    assert_eq!(
        teleports(&ev1.pages[0].commands),
        vec![(28, [-1, 62, 38, 1, 1])],
        "Map003 ev1 pc28 is the single Map003 -> SampleMapA hop"
    );

    // Honest boundary: MapTree carries no coordinates, so it is not the source of
    // (62, 38); the operands above are the only authored start position.
    let Ok(tree_bytes) = std::fs::read(root.join("BasicData").join("MapTree.dat")) else {
        eprintln!("skipping: MapTree.dat missing");
        return;
    };
    let tree = wolf_format::maptree::MapTree::parse(&tree_bytes).expect("MapTree parses");
    assert_eq!(tree.entries.len(), 50);
    assert!(tree.children_of(0).contains(&3), "Map003 is a root child");
}

/// The whole route as the engine runs it: boot the title, answer the real CE42 menu
/// with option 1, and watch the two opcode 130 hops land in SampleMapA. Nothing here
/// calls start_map_event_by_name, so a pass proves the retail chain, not a synthetic
/// start at Map003 event 1.
#[test]
fn the_retail_title_menu_boots_a_new_game_through_map003_into_sample_map_a() {
    let Some(mut game) = open_game() else {
        return;
    };
    game.boot().expect("the boot map has its autorun event");

    for _ in 0..800 {
        game.frame();
    }
    assert!(
        game.screen
            .iter()
            .any(|picture| picture.file.contains("title01")),
        "the boot chain did not reach the title art"
    );
    assert!(
        !game
            .diagnostics
            .keys()
            .any(|line| line.contains("MapData/Map003.mps")),
        "a new game must not start before the title menu is answered"
    );

    // Confirm opens CE42's sample title menu (three example options).
    game.key_bits = key::OK;
    game.frame();
    game.key_bits = 0;
    for _ in 0..240 {
        game.frame();
    }
    let menu = game.choices.as_ref().expect("the title menu is open");
    assert_eq!(
        menu.options,
        ["1", "2", "3"],
        "the retail title menu options"
    );

    // Answer option 1 = new game, exactly the player's action.
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    for _ in 0..120 {
        game.frame();
        if game.choices.is_none() {
            break;
        }
    }
    game.auto_choice = false;
    game.auto_advance = true;
    let mut opening_prompt: Option<Vec<String>> = None;
    for _ in 0..900 {
        game.frame();
        if let Some(choices) = game.choices.as_ref() {
            opening_prompt = Some(choices.options.clone());
            break;
        }
    }

    let diagnostics: Vec<String> = game.diagnostics.keys().cloned().collect();
    assert!(
        diagnostics.iter().any(|line| line == TITLE_TO_MAP003),
        "missing the exact CE42 -> Map003 hop {TITLE_TO_MAP003:?}; got {diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(|line| line == MAP003_TO_SAMPLE),
        "missing the exact Map003 -> SampleMapA hop {MAP003_TO_SAMPLE:?}; got {diagnostics:?}"
    );

    let expected: Vec<String> = [
        "广阔的战场",
        "广袤的森林",
        "微风吹拂而过的麦田",
        "有人在等我的家",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(
        opening_prompt,
        Some(expected),
        "the new game did not reach SampleMapA's opening prompt"
    );
    assert_eq!(
        game.stats.warnings, 0,
        "the new-game route produced warnings"
    );
    assert!(
        game.stats.instructions > 10_000,
        "the title path plus the opening runs a real script: {} instructions",
        game.stats.instructions
    );
    let stale: Vec<&str> = game
        .screen
        .iter()
        .filter(|picture| picture.number < 0 && picture.file.contains("TitleGraphic"))
        .map(|picture| picture.file.as_str())
        .collect();
    assert!(
        stale.is_empty(),
        "title art survived the map switch: {stale:?}"
    );
}
