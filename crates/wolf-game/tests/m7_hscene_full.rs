//! M7 W2 full H wrapper: resume the retail mid-H save onto SampleMapA and drive the
//! late-night UI with real mouse clicks.
//!
//! Two things this file proves, both from the real data root (`work/all`) and the retail
//! save `work/retail_backup/Save/SaveData08.sav`:
//!
//! 1. `retail_mid_h_save_resumes_the_full_layer_stack` - loading the save and resuming
//!    the map's automatic pages (skipping the opening autorun, event 0) rebuilds the whole
//!    H picture stack the retail game draws: background, pose, the three face layers, the
//!    effect/action layers, the frame and the H button HUD. It stays live (instructions keep
//!    advancing, warnings stay 0) instead of deadlocking.
//! 2. `real_mouse_clicks_advance_the_late_night_stage_index` - the late-night UI stage
//!    index is global var62. Event 11 "■深夜UI判定" page 0 polls each button picture with
//!    `124 [1100000,0x4000,<pic>,8]` (hover) then `124 [1100001,0x3000,7]` (fresh left
//!    press), and its page condition is gated by `var364 == 0`. The retail save was taken
//!    with `var364 == 1` (dispatcher gate closed); clearing that single gate lets a real
//!    hover + left click walk the stage index across several menu pages with zero warnings.
//!
//! Nothing here forces the stage/menu variables themselves (var61/var62/var137): the only
//! injected value is the documented UI gate `var364 = 0`; everything else is produced by
//! synthetic mouse input going through the real opcode-124/opcode-123 host reads.

use std::path::PathBuf;

use wolf_format::mps::Map;
use wolf_game::Game;

/// Repository root (the directory above the crate folder).
fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

/// Data root, `work/all` by default (H files only live in the full unpack).
fn data_root() -> PathBuf {
    if let Ok(value) = std::env::var("WOLF_DATA_ROOT") {
        if !value.is_empty() {
            return repo_root().join(value);
        }
    }
    repo_root().join("work").join("all")
}

fn save_path() -> PathBuf {
    repo_root()
        .join("work")
        .join("retail_backup")
        .join("Save")
        .join("SaveData08.sav")
}

fn load_sample_map() -> Map {
    let bytes =
        std::fs::read(data_root().join("MapData").join("SampleMapA.mps")).expect("SampleMapA.mps");
    Map::parse(&bytes).expect("parse SampleMapA.mps")
}

/// Load the retail save and resume the map, skipping the opening autorun (event 0) so the
/// cutscene does not run over the H scene. Returns `None` (test skips) when the data or
/// save are unavailable.
fn open_resumed() -> Option<Game> {
    let save = save_path();
    if !save.exists() {
        eprintln!("skip: missing {}", save.display());
        return None;
    }
    if !data_root().join("MapData").join("SampleMapA.mps").exists() {
        eprintln!(
            "skip: missing SampleMapA.mps under {}",
            data_root().display()
        );
        return None;
    }
    let mut game = match Game::new(data_root()) {
        Ok(game) => game,
        Err(error) => {
            eprintln!("skip: Game::new failed: {error}");
            return None;
        }
    };
    if let Err(error) = game.load_save(&save) {
        eprintln!("skip: load_save failed: {error}");
        return None;
    }
    let map = load_sample_map();
    game.start_engine_events();
    game.start_pages(&map, Some(0));
    Some(game)
}

/// Hover the pointer on a picture centre, then a fresh left press and release, and let the
/// surrounding events run a few frames. This is the exact input shape opcode 124 reads:
/// field 8 = box hover, engine field 7 = a button that went down this frame.
fn click_at(game: &mut Game, x: i32, y: i32) {
    game.mouse = [x, y];
    game.mouse_buttons = 0;
    game.frame();
    game.mouse_buttons = 1;
    game.frame();
    game.mouse_buttons = 0;
    for _ in 0..15 {
        game.frame();
    }
}

fn file_present(game: &Game, needle: &str) -> bool {
    game.screen
        .iter()
        .any(|picture| picture.file.contains(needle))
}

const H_LAYER_FILES: &[&str] = &[
    "hscene/h_bg_001.jpg",               // background A
    "hscene/h_bg_002.png",               // background overlay B
    "hscene/h_s10_pose03.png",           // character pose
    "hscene/h_s10_face_mayu.png",        // eyebrows
    "hscene/h_s10_face_eye.png",         // eyes
    "hscene/h_s10_face_kuchi.png",       // mouth
    "hscene/h_s10_ef_syasei03.png",      // effect (body fluid)
    "hscene/h_s10_action_vibrator2.png", // prop/action layer
    "hscene/h_frame.png",                // H frame
    "hword/word_effect98.png",           // h-word overlay
];

const H_BUTTON_FILES: &[&str] = &[
    "Picture/h_button_s01.png",
    "Picture/h_button_s02.png",
    "Picture/h_button_s04.png",
    "Picture/h_button_s06.png",
    "Picture/h_button_s07.png",
];

#[test]
fn retail_mid_h_save_resumes_the_full_layer_stack() {
    let Some(mut game) = open_resumed() else {
        return;
    };

    for _ in 0..120 {
        game.frame();
    }

    assert_eq!(game.stats.warnings, 0, "load/resume must not warn");
    assert!(
        game.screen.len() >= 80,
        "expected the full H picture stack, found {} pictures",
        game.screen.len()
    );

    for file in H_LAYER_FILES {
        assert!(file_present(&game, file), "missing H layer {file}");
    }
    for file in H_BUTTON_FILES {
        assert!(file_present(&game, file), "missing H button {file}");
    }

    // The save is a genuine mid-H snapshot: the late-night stage index lives in var62 and is
    // 1 (base menu), while the act selector var137 sits at its h_s10 stage (8).
    //
    // var364 is written by CE335's "チュートリアル" (tutorial) block, which only runs while
    // CE335 self20 == 0.  SaveData08 ships a restored CE335 self bank with self20 != 0
    // (the H scene's real progress state), so the tutorial block is correctly skipped and
    // var364 stays 0 while the full H layer stack still renders.  Before the M13
    // common-event self fix every invocation started from all-zero self, replaying the
    // tutorial block and forcing var364 to 1 - the old assertion here encoded that bug.
    assert_eq!(game.vm.vars.get(62), 1, "late-night stage index var62");
    assert_eq!(game.vm.vars.get(137), 8, "H act selector var137");
    assert_eq!(
        game.vm.vars.get(364),
        0,
        "tutorial gate from restored CE335 self20"
    );

    // No deadlock: the engine keeps making progress and never warns while it idles on the H
    // scene. A stuck coroutine would freeze the instruction counter.
    let before_instr = game.stats.instructions;
    let before_frames = game.stats.frames;
    for _ in 0..300 {
        game.frame();
    }
    assert!(
        game.stats.instructions > before_instr,
        "engine stopped executing ({before_instr} instructions frozen)"
    );
    assert!(
        game.stats.frames > before_frames,
        "frame counter did not advance"
    );
    assert_eq!(
        game.stats.warnings, 0,
        "idling on the H scene must not warn"
    );
    assert!(
        game.screen.len() >= 80,
        "the H stack thinned out to {} pictures",
        game.screen.len()
    );
}

#[test]
fn real_mouse_clicks_advance_the_late_night_stage_index() {
    let Some(mut game) = open_resumed() else {
        return;
    };

    for _ in 0..60 {
        game.frame();
    }

    // Event 11 "■深夜UI判定" page 0 requires var364 == 0 to run.  The retail save resumes
    // with the CE335 tutorial already completed (self20 != 0), so var364 is already 0; set
    // it explicitly anyway to keep this test independent of that detail.
    game.vm.vars.set(364, 0);
    for _ in 0..15 {
        game.frame();
    }

    assert_eq!(game.stats.warnings, 0, "clearing the gate must not warn");
    assert_eq!(
        game.vm.vars.get(62),
        1,
        "stage index starts at the base menu"
    );

    // Walk the menu the way the retail UI does: at each stage click the first visible H
    // button whose real click advances the stage index (var62). Screen iteration order is
    // deterministic, so this reproduces the same trajectory every run.
    let mut trajectory: Vec<(i32, i32)> = Vec::new();
    let started = game.stats.warnings;
    for _step in 0..12 {
        let buttons: Vec<(i32, i32, i32)> = game
            .screen
            .iter()
            .filter(|picture| picture.file.contains("h_button") || picture.file.contains("hbutton"))
            .map(|picture| (picture.number, picture.x, picture.y))
            .collect();

        let mut advanced = false;
        for (number, x, y) in buttons {
            let before = game.vm.vars.get(62);
            click_at(&mut game, x, y);
            let after = game.vm.vars.get(62);
            if after != before && game.stats.warnings == started {
                trajectory.push((number, after));
                advanced = true;
                break;
            }
        }
        if !advanced {
            break;
        }
    }

    assert!(
        trajectory.len() >= 4,
        "expected the stage index to advance through several menu pages, got {trajectory:?}"
    );
    // Deterministic regression: the real-button click path walks 1 -> 2 -> 6 -> 1 -> 4 -> 13.
    assert_eq!(
        trajectory,
        vec![(400, 2), (410, 6), (421, 1), (500, 4), (510, 13)],
        "late-night stage trajectory changed"
    );

    // Distinct stage values prove the index really moved rather than holding still.
    let mut visited: Vec<i32> = trajectory.iter().map(|(_, stage)| *stage).collect();
    visited.sort_unstable();
    visited.dedup();
    assert!(visited.len() >= 4, "stage index visited only {visited:?}");

    assert_eq!(
        game.stats.warnings, started,
        "interactive clicks must not warn"
    );
    assert!(
        game.stats.instructions > 0 && !game.screen.is_empty(),
        "engine deadlocked during menu interaction"
    );
}
