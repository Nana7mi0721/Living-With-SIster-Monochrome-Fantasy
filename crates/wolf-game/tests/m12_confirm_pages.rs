//! M12 retail-scope item: `confirm` pages vs the picture hit boxes.
//!
//! `docs/MAP_AUDIT.md` §9 listed one open question: a map event page with trigger 0 is
//! a *confirm* page, but the shipped game is point-and-click and the player sees
//! pictures, not event tiles, so "event-grid click" and "picture hover" looked like two
//! competing hit-test systems. This test settles what the six confirm pages in
//! SampleMapA actually are.
//!
//! Measured result: five of the six are empty page-selector stubs. They exist only so a
//! `v302 == 1` condition makes a higher, empty page active and thereby stops the event's
//! parallel page; nothing is ever meant to click them. The sixth (ev24 page 6) is the
//! only confirm page with commands, and its last condition is `self0 == 99`; no page in
//! any map and no common event writes magic `1_000_000` at all, so the page is
//! debug-editor-only and unreachable in the retail flow.
//!
//! The actual click input path is opcode 124 on pictures, whose hit box is the picture's
//! drawn destination rectangle (`Game::picture_hover`, already covered by
//! `m8_hover_anchor.rs`). The event tiles at (0..10, 33/35) sit underneath full-screen
//! background layers, not under dedicated HUD hit areas.
//!
//! Skips when the unpacked data root is absent.

use std::fs;
use std::path::{Path, PathBuf};

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_format::mps::Map;

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        repo_root().join(path)
    }
}

fn load_map(root: &Path, name: &str) -> Option<Map> {
    let path = root.join("MapData").join(name);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("skipping: cannot read {}: {err}", path.display());
            return None;
        }
    };
    Some(Map::parse(&bytes).expect("parse map"))
}

fn load_common_events(root: &Path) -> Option<CommonEventData> {
    let path = root.join("BasicData").join("CommonEvent.dat");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("skipping: cannot read {}: {err}", path.display());
            return None;
        }
    };
    Some(CommonEventData::parse(&bytes).expect("parse CommonEvent.dat"))
}

/// WOLF magic `1_000_000 + n` is the event-local self variable n. The confirm page's
/// last condition compares self0 with 99.
const MAGIC_SELF0: u32 = 1_000_000;

fn int_args(cmd: &wolf_format::command::Command) -> Vec<i32> {
    cmd.params
        .chunks_exact(4)
        .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect()
}

/// A command writes a numeric destination for `121` (SetVariable), `122` (SetString) or
/// `124` (SetVariableEx); `250`'s first argument is a container id, not a magic value.
fn writes_magic(cmd: &wolf_format::command::Command, magic: u32) -> bool {
    let ints = int_args(cmd);
    matches!(cmd.opcode, 121 | 122 | 124) && ints.first() == Some(&(magic as i32))
}

#[test]
fn the_six_confirm_pages_are_selector_stubs_not_click_targets() {
    let root = data_root();
    let Some(map) = load_map(&root, "SampleMapA.mps") else {
        return;
    };

    // The six confirm pages recorded in docs/HANDOFF_M12.md §5.4 / MAP_AUDIT.md.
    let expected = [(11u32, 1usize), (14, 1), (18, 1), (21, 1), (22, 3), (24, 6)];
    for (id, page_index) in expected {
        let event = map
            .events
            .iter()
            .find(|event| event.id == id)
            .unwrap_or_else(|| panic!("SampleMapA event {id} missing"));
        let page = event
            .pages
            .get(page_index)
            .unwrap_or_else(|| panic!("event {id} has no page {page_index}"));
        assert_eq!(
            page.trigger, 0,
            "event {id} page {page_index} must be a confirm page"
        );

        let real_commands = page
            .commands
            .iter()
            .filter(|slot| matches!(slot, Slot::Command(cmd) if cmd.opcode != 0))
            .count();
        if id == 24 {
            assert!(
                real_commands > 100,
                "ev24 page 6 is the real exception handler, got {real_commands} commands"
            );
        } else {
            assert_eq!(
                real_commands, 0,
                "event {id} confirm page must be an empty selector stub, got {real_commands} commands"
            );
        }
    }

    // Page priority explains why the empty pages exist. With v302 == 1 the highest
    // matching page is the empty confirm page, so the parallel page 0 stops running;
    // with v302 == 0 the parallel page is active again. Nothing clicks the stub.
    let select = |v302: i32| {
        move |variable: u32| -> i32 {
            match variable {
                2_000_302 => v302,
                // The parallel pages' other gates: v62 >= 1, v168 == 0, v364 == 0,
                // v60 == 0, v138 in range and v139 == 0.
                2_000_062 => 1,
                _ => 0,
            }
        }
    };
    for (id, page_index) in [(11u32, 1usize), (14, 1), (18, 1), (21, 1), (22, 3)] {
        let event = map.events.iter().find(|event| event.id == id).unwrap();
        assert_eq!(
            event.active_page_index(select(0)),
            Some(0),
            "event {id} runs its parallel page while v302 == 0"
        );
        assert_eq!(
            event.active_page_index(select(1)),
            Some(page_index),
            "event {id} switches to its empty confirm page while v302 == 1"
        );
    }

    // ev24 page 6 is gated on v60 == 4, v124 >= 1 and self0 == 99. With self0 left at 0
    // the highest matching page is the autorun page 5; only a debugger value reaches 6.
    let event24 = map.events.iter().find(|event| event.id == 24).unwrap();
    let select24 = |self0: i32| {
        move |variable: u32| -> i32 {
            match variable {
                2_000_060 => 4,
                2_000_124 => 1,
                MAGIC_SELF0 => self0,
                _ => 0,
            }
        }
    };
    assert_eq!(
        event24.active_page_index(select24(0)),
        Some(5),
        "ev24's normal exception path is autorun page 5"
    );
    assert_eq!(
        event24.active_page_index(select24(99)),
        Some(6),
        "ev24's confirm page 6 is only selected with self0 == 99"
    );
}

#[test]
fn no_shipped_script_writes_event_self0() {
    let root = data_root();
    let Some(map) = load_map(&root, "SampleMapA.mps") else {
        return;
    };
    let Some(events) = load_common_events(&root) else {
        return;
    };

    // Any write to magic 1_000_000 anywhere in the data would make the "debug only"
    // conclusion uncertain. Scan the whole common-event file plus every map's pages.
    for event in &events.events {
        for (pc, slot) in event.commands.iter().enumerate() {
            let Slot::Command(cmd) = slot else { continue };
            assert!(
                !writes_magic(cmd, MAGIC_SELF0),
                "common event {} pc{pc} writes event self0: {cmd:?}",
                event.id
            );
        }
    }
    for entry in fs::read_dir(root.join("MapData")).expect("read MapData") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".mps") {
            continue;
        }
        let bytes = fs::read(entry.path()).expect("read map");
        let parsed = Map::parse(&bytes).expect("parse map");
        for event in &parsed.events {
            for (page_index, page) in event.pages.iter().enumerate() {
                for (pc, slot) in page.commands.iter().enumerate() {
                    let Slot::Command(cmd) = slot else { continue };
                    assert!(
                        !writes_magic(cmd, MAGIC_SELF0),
                        "{name} ev{} page{page_index} pc{pc} writes event self0: {cmd:?}",
                        event.id
                    );
                }
            }
        }
    }

    // The six confirm pages belong to this map; the scan above covered it as well.
    assert!(
        map.events.iter().any(|event| event.id == 24),
        "SampleMapA is the audited map"
    );
}
/// The event grid and the picture hit boxes are two different coordinate systems, and
/// the retail click path only uses the second. Loading SaveData01 shows what actually
/// sits under the six event tiles: the full-screen background layers (-10, -7, -1) and,
/// at ev21's tile only, the visible sub button picture 107. This is the observation that
/// closes MAP_AUDIT.md §9 item 4 - a confirm page is not a click target.
#[test]
fn the_confirm_tiles_sit_under_picture_hitboxes_not_under_event_grid_clicks() {
    let root = data_root();
    if load_map(&root, "SampleMapA.mps").is_none() {
        return;
    }
    let retail = repo_root().join("work/retail_backup/Save/SaveData01.sav");
    if !retail.is_file() {
        eprintln!("skipping: no retail SaveData01.sav");
        return;
    }
    let mut game = match wolf_game::Game::new(root) {
        Ok(game) => game,
        Err(err) => {
            eprintln!("skipping: {err}");
            return;
        }
    };
    game.load_save(&retail).expect("load SaveData01");
    let numbers: Vec<i32> = game.screen.iter().map(|picture| picture.number).collect();

    for (label, tile_x, tile_y, extra) in [
        ("ev18", 0, 33, None),
        ("ev21", 2, 33, Some(107)),
        ("ev11", 3, 35, None),
        ("ev14", 4, 35, None),
        ("ev22", 7, 35, None),
        ("ev24", 10, 35, None),
    ] {
        game.mouse = [tile_x * 16 + 8, tile_y * 16 + 8];
        // A click at the event tile is seen by 124 as a hover over the picture layer.
        for screen_layer in [-10, -7, -1] {
            assert_eq!(
                game.picture_hover(screen_layer),
                1,
                "{label}: screen layer {screen_layer} must cover the event tile"
            );
        }
        let over: Vec<i32> = numbers
            .iter()
            .copied()
            .filter(|number| *number >= 0 && game.picture_hover(*number) == 1)
            .collect();
        assert_eq!(
            over,
            extra.into_iter().collect::<Vec<_>>(),
            "{label}: only the visible picture at that pixel may claim the click"
        );
    }

    // Picture 107 is the bottom-left sub button: its rectangle is the hit area the
    // player uses, and it does not follow the event tile grid.
    let button = game.screen.get(107).expect("sub button picture 107");
    assert_eq!(
        (button.x, button.y),
        (20, 505),
        "retail sub-button placement"
    );
    game.mouse = [20, 505];
    assert_eq!(
        game.picture_hover(107),
        1,
        "the drawn rectangle is the hit box"
    );
    game.mouse = [19, 505];
    assert_eq!(
        game.picture_hover(107),
        0,
        "one pixel left is outside the picture"
    );
}
