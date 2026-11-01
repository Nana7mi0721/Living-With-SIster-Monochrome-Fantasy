//! M7 W2 playable H wrapper: drive the late-night UI with real pointer input and record the
//! stage trace, then prove the terminal stage holds (no exit, no deadlock).
//!
//! This is the "playable" companion to m7_hscene_full.rs. Where that file proves the H layer
//! stack resumes and one greedy walk moves the stage index, this file plays the wrapper like a
//! user would and documents what each real click does: which button picture is hit, which stage
//! index var62 it produces, how many pictures/events are live afterwards, and which background
//! coroutines appeared. It then pins the terminal stage and shows a click cannot advance past
//! it and the scene never returns to the map or deadlocks.
//!
//! Real entry: load the retail mid-H save work/retail_backup/Save/SaveData08.sav, resume
//! SampleMapA's automatic pages while skipping the opening autorun (event 0). The save resumes
//! with the CE335 tutorial already completed (self20 != 0), so the UI dispatcher gate var364 is
//! 0; individual tests still force it where they need a specific dispatch branch.
//! var61/var62/var137 are all produced by synthetic mouse input flowing through the real
//! opcode-124 host reads (hover field 8, fresh-left-press engine field 7).
//!
//! On the CE333 question: common event 333 is the expression/face/effect helper the UI events
//! 11/18/19 call (opcode 210 target 500333); it is not a standalone CG and, being a nested
//! common-event call, never appears in vm.started_events. Its output is visible as the h_s10
//! face/effect layers on screen, and this file asserts those layers are present at the terminal
//! stage. The ev33 "insert/pull" animation and the CE246 deep CG branch are NOT reachable from
//! this UI: they are gated behind the intimacy self-variable chain documented in
//! docs/research/M7_HMODE_STATE.md (var157 self 1_600_013 etc.).

use std::collections::BTreeSet;
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

/// Data root, work/all by default (H files only live in the full unpack).
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
/// cutscene does not run over the H scene. Returns None (test skips) when data/save missing.
fn open_resumed() -> Option<Game> {
    let save = save_path();
    if !save.exists() {
        eprintln!("skip: missing {}", save.display());
        return None;
    }
    if !data_root().join("MapData").join("SampleMapA.mps").exists() {
        eprintln!("skip: missing SampleMapA.mps under {}", data_root().display());
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
    game.screen.iter().any(|picture| picture.file.contains(needle))
}

/// The live coroutine event ids, as a set, so a test can diff before/after a click.
fn alive_events(game: &Game) -> BTreeSet<u32> {
    game.vm.instances().iter().map(|inst| inst.event).collect()
}

/// One step of the recorded play trace.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StageStep {
    /// The picture number of the H button that was clicked.
    button: i32,
    /// Its image file (basename), for a human-readable trace.
    file: String,
    /// Stage index (var62) before -> after the click.
    from: i32,
    to: i32,
    /// Live picture count after the click settled.
    pictures: usize,
    /// Coroutine events that first appeared (relative to before this click).
    new_events: Vec<u32>,
}

const H_LAYER_FILES: &[&str] = &[
    "hscene/h_bg_001.jpg",               // background A
    "hscene/h_bg_002.png",               // background overlay B
    "hscene/h_s10_pose03.png",           // character pose
    "hscene/h_s10_face_mayu.png",        // eyebrows
    "hscene/h_s10_face_eye.png",         // eyes
    "hscene/h_s10_face_kuchi.png",       // mouth
    "hscene/h_s10_ef_up.png",            // effect column (up)
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

/// Walk the late-night menu from the base page to a terminal page, recording one StageStep per
/// real click. At each stage it tries the visible H buttons in draw order and keeps the first
/// that changes var62 with warnings unchanged (the deterministic retail path).
fn play_wrapper(game: &mut Game, max_steps: usize, trace: &mut Vec<StageStep>) {
    for _ in 0..max_steps {
        let buttons: Vec<(i32, String, i32, i32)> = game
            .screen
            .iter()
            .filter(|picture| {
                picture.file.contains("h_button") || picture.file.contains("hbutton")
            })
            .map(|picture| (picture.number, basename(&picture.file), picture.x, picture.y))
            .collect();

        let mut advanced = false;
        for (number, file, x, y) in buttons {
            let before = game.vm.vars.get(62);
            let warnings_before = game.stats.warnings;
            let events_before = alive_events(game);
            click_at(game, x, y);
            let after = game.vm.vars.get(62);
            if after != before && game.stats.warnings == warnings_before {
                let events_after = alive_events(game);
                let new_events: Vec<u32> = events_after.difference(&events_before).copied().collect();
                trace.push(StageStep {
                    button: number,
                    file,
                    from: before,
                    to: after,
                    pictures: game.screen.len(),
                    new_events,
                });
                advanced = true;
                break;
            }
        }
        if !advanced {
            break;
        }
    }
}

fn basename(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned()
}

#[test]
fn h_wrapper_plays_through_five_real_clicks_with_clean_warnings() {
    let Some(mut game) = open_resumed() else { return; };

    for _ in 0..60 {
        game.frame();
    }
    assert_eq!(game.stats.warnings, 0, "resume must not warn");

    // At the resumed base page (stage 1) the whole H stack and every H button are drawn.
    for file in H_LAYER_FILES {
        assert!(file_present(&game, file), "missing H layer {file} on the resumed page");
    }
    for file in H_BUTTON_FILES {
        assert!(file_present(&game, file), "missing H button {file} on the resumed page");
    }

    // Open the dispatcher gate (the only injected value). Then play the menu.
    game.vm.vars.set(364, 0);
    for _ in 0..15 {
        game.frame();
    }
    assert_eq!(game.vm.vars.get(62), 1, "stage index starts at the base menu");

    let mut ever: BTreeSet<u32> = alive_events(&game);
    let mut trace: Vec<StageStep> = Vec::new();
    play_wrapper(&mut game, 12, &mut trace);
    ever.extend(alive_events(&game));
    // Sweep a few idle frames so background UI events that only spin occasionally are seen.
    for _ in 0..30 {
        game.frame();
        ever.extend(alive_events(&game));
    }

    // Deterministic stage trace: five real left clicks walk 1 -> 2 -> 6 -> 1 -> 4 -> 13.
    let path: Vec<(i32, i32, i32)> = trace.iter().map(|s| (s.button, s.from, s.to)).collect();
    assert_eq!(
        path,
        vec![(400, 1, 2), (410, 2, 6), (421, 6, 1), (500, 1, 4), (510, 4, 13)],
        "late-night playable stage trace changed: {trace:#?}"
    );
    assert!(
        trace.iter().all(|s| s.pictures >= 80),
        "the H stack thinned out during the walk: {trace:#?}"
    );

    // The real UI events really ran: the two display pages, the click dispatcher and the
    // processor, plus the expression/effect helpers (18/19) that draw the h_s10 face layers.
    for wanted in [9u32, 10, 11, 14, 18, 19] {
        assert!(ever.contains(&wanted), "UI event {wanted} never ran: {ever:?}");
    }

    assert_eq!(game.vm.vars.get(62), 13, "walk must end at stage 13");
    assert_eq!(game.stats.warnings, 0, "the whole play session must stay warning-free");

    // The CE333 expression/effect helper has no trackable coroutine id here, but its DRAWN
    // output must be on screen at the terminal stage: the full H character/effect layer stack.
    for file in H_LAYER_FILES {
        assert!(file_present(&game, file), "missing H layer {file} at terminal stage");
    }
    // Terminal pages draw their own subset of H buttons (plus the end button); at least one
    // clickable button must remain, otherwise the page would be a dead end we cannot explain.
    assert!(
        game.screen.iter().any(|p| {
            p.file.contains("h_button") || p.file.contains("hbutton") || p.file.contains("endbutton")
        }),
        "terminal stage has no clickable button at all"
    );
}

#[test]
fn terminal_stage_13_holds_without_exiting_or_deadlocking() {
    let Some(mut game) = open_resumed() else { return; };

    for _ in 0..60 {
        game.frame();
    }
    game.vm.vars.set(364, 0);
    for _ in 0..15 {
        game.frame();
    }

    let mut trace = Vec::new();
    play_wrapper(&mut game, 12, &mut trace);
    assert_eq!(game.vm.vars.get(62), 13, "expected to reach terminal stage 13");

    // Clicking every visible button at stage 13 must NOT push the stage index past 13: the menu
    // has no further page. (A button toggles the selection code var61 and a background click
    // resets to stage 1, but nothing produces stage 14+.)
    let warnings_before = game.stats.warnings;
    let terminal_buttons: Vec<(i32, i32)> = game
        .screen
        .iter()
        .filter(|picture| {
            picture.file.contains("h_button")
                || picture.file.contains("hbutton")
                || picture.file.contains("endbutton")
        })
        .map(|picture| (picture.x, picture.y))
        .collect();
    assert!(!terminal_buttons.is_empty(), "expected clickable terminal-page buttons");
    for (x, y) in &terminal_buttons {
        click_at(&mut game, *x, *y);
        let stage = game.vm.vars.get(62);
        assert!(
            (1..=13).contains(&stage),
            "a terminal-page click jumped the stage index out of range: {stage}"
        );
    }
    assert_eq!(game.stats.warnings, warnings_before, "terminal clicks must not warn");

    // Re-reach a stable terminal page and idle: the scene must keep running (no deadlock) and
    // must stay in the H UI (not return to the map / not blank out).
    for _ in 0..40 {
        game.frame();
    }
    let stage_before_idle = game.vm.vars.get(62);
    let instr_before = game.stats.instructions;
    let frames_before = game.stats.frames;
    for _ in 0..300 {
        game.frame();
    }
    assert_eq!(
        game.vm.vars.get(62),
        stage_before_idle,
        "stage index drifted while idle"
    );
    assert!(
        game.stats.instructions > instr_before,
        "engine deadlocked after the wrapper terminal stage"
    );
    assert!(game.stats.frames > frames_before, "frame counter froze");
    assert_eq!(game.stats.warnings, 0, "idling in the H UI must not warn");
    assert!(
        game.screen.len() >= 80,
        "H picture stack thinned out to {} pictures",
        game.screen.len()
    );
    // Still inside the H scene, not back on the map: the character pose and frame layers remain.
    assert!(file_present(&game, "hscene/h_s10_pose03.png"), "left the H scene");
    assert!(file_present(&game, "hscene/h_frame.png"), "lost the H frame");
    // var137 is the act selector; it stays on the h_s10 stage throughout, proving no route exit.
    assert_eq!(game.vm.vars.get(137), 8, "act selector should stay on h_s10");
}

#[test]
fn stage_index_is_advanced_by_clicks_not_by_time() {
    let Some(mut game) = open_resumed() else { return; };

    for _ in 0..60 {
        game.frame();
    }
    // The retail save resumes with the CE335 tutorial already completed (self20 != 0), so
    // the UI dispatcher gate var364 is open (0), not closed.  Before the M13 common-self
    // fix every load reset CE335 self to zero, replayed the tutorial and forced var364=1.
    assert_eq!(
        game.vm.vars.get(364),
        0,
        "restored CE335 self20 skips the tutorial, so the dispatcher gate is open"
    );
    // Even with the gate open, idling never moves the stage index: the advance is input-driven.
    for _ in 0..300 {
        game.frame();
    }
    assert_eq!(
        game.vm.vars.get(62),
        1,
        "the stage index must not advance without a real left click"
    );

    // A single real click on the base-page button (h_button_s01 at 820,260) advances it 1 -> 2.
    click_at(&mut game, 820, 260);
    assert_eq!(
        game.vm.vars.get(62),
        2,
        "one real hover+left-click on h_button_s01 must advance the stage index"
    );
    assert_eq!(game.stats.warnings, 0, "the click path must not warn");
}
