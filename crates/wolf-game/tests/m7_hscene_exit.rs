//! M7 W2-d follow-up — the H scene's real termination path from the terminal stage.
//!
//! The M7 note in \`docs/HANDOFF_M7.md\` left one H-scene boundary open: "terminal stage 13
//! does not exit to the map, clicks only toggle var61 / reset stage 1", i.e. the late-night
//! H UI looked terminal-by-design. This test proves that reading is an **artifact of the
//! input harness, not the game**: the H scene has a real, explicit exit, and it is reachable
//! from the terminal stage 13.
//!
//! The exit, all in \`SampleMapA\` (data root \`work/all\`, save \`SaveData08.sav\`):
//!
//! * Every late-night UI page (event 9 pages = stages 1..10, event 10 pages = stages 11..16)
//!   draws the on-screen end button \`Picture/endbutton.png\` (picture 294).
//! * Hovering/pressing that button makes the dispatcher event 11 write \`var61 = 49\`
//!   (\`■深夜UI判定\`, the same \`var61\` selector the other H buttons use).
//! * \`var61 = 49\` is *not* handled by the stage-hop processor (event 14); it is handled by
//!   event 25 \`深夜終了処理\` page 0 (\`var140==0 & var60==0 & var364==0 & var123==0\` — confirmed
//!   all zero at stage 13). Its inner branch is gated on **system variable 12 == 0**
//!   (\`9000012\`): \`pc9 op111 [1,9000012,0,2]\`. \`9000012\` is the message-window-visible flag
//!   (\`CE63\` pc70 reads it as "メッセージウィンドウ表示", pc179/194/206 as "closed"), so the
//!   exit cannot fire while a text window is open — the button must be pressed with the
//!   message window idle.
//! * When the gate passes, event 25 pc23 calls \`CE345\` \`深夜終了コマンド\`, whose teardown
//!   (\`pc339 var62=0, pc340 var63=0, pc341 var64=0\`, blackscreen fade, \`CE222\` save,
//!   \`CE217\` clock) leaves the H UI. \`CE345\` is a *scripted* sequence with a first "要结束么？"
//!   yes/no choice, so its dialogue must be advanced for the teardown to reach pc339.
//!
//! Verdict: **the H scene is NOT terminal-by-design.** The exit exists (end button ->
//! event 11 \`var61=49\` -> event 25 page 0 -> \`CE345\`) and fires from stage 13 with warnings 0.
//! The earlier "no exit" observation came from (a) pressing the button while the message
//! window was still open (\`9000012 != 0\` -> event 25 pc9 gate skipped) and (b) never
//! advancing \`CE345\`'s blocking dialogue. See \`docs/research/M7_HSCENE_EXIT.md\`.
//!
//! Run: \`WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_hscene_exit -- --nocapture\`

use std::collections::BTreeSet;
use std::path::PathBuf;

use wolf_format::mps::Map;
use wolf_game::Game;

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

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

/// Load the retail mid-H save and resume the map, skipping the opening autorun (event 0),
/// exactly like \`m7_hscene_playable.rs\` / \`m8_hscene_forward.rs\`.
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

/// The exact opcode-124 input shape: hover a picture centre, then a fresh left press/release,
/// then let the surrounding events react for a few frames.
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

fn endbutton_pos(game: &Game) -> Option<(i32, i32)> {
    game.screen
        .get(294)
        .filter(|picture| picture.file.contains("endbutton"))
        .map(|picture| (picture.x, picture.y))
}

fn file_present(game: &Game, needle: &str) -> bool {
    game.screen
        .iter()
        .any(|picture| picture.file.contains(needle))
}

/// Drive the real H UI to stage 13 by clicking whichever on-screen \`h_button\` advances
/// \`var62\`, stopping as soon as stage 13 is reached. Mirrors the dynamic click the retail
/// menus are built around; the forward path is 1 -> 2 -> 6 -> 1 -> 4 -> 13 on \`SaveData08\`.
fn nav_to_terminal_stage(game: &mut Game) -> bool {
    for _ in 0..40 {
        if game.vm.vars.get(62) == 13 {
            return true;
        }
        let buttons: Vec<(i32, i32)> = game
            .screen
            .iter()
            .filter(|picture| picture.file.contains("h_button") || picture.file.contains("hbutton"))
            .map(|picture| (picture.x, picture.y))
            .collect();
        let mut advanced = false;
        for (x, y) in buttons {
            let before = game.vm.vars.get(62);
            click_at(game, x, y);
            if game.vm.vars.get(62) != before {
                advanced = true;
                break;
            }
        }
        if !advanced {
            return game.vm.vars.get(62) == 13;
        }
    }
    game.vm.vars.get(62) == 13
}

/// Press the end button and run up to \`frames\` frames, collecting every live event id.
fn press_end(game: &mut Game, frames: usize) -> (BTreeSet<u32>, bool) {
    let Some((x, y)) = endbutton_pos(game) else {
        return (BTreeSet::new(), false);
    };
    let mut seen: BTreeSet<u32> = BTreeSet::new();
    for inst in game.vm.instances() {
        seen.insert(inst.event);
    }
    click_at(game, x, y);
    for _ in 0..frames {
        game.frame();
        for inst in game.vm.instances() {
            seen.insert(inst.event);
        }
    }
    (seen, true)
}

fn settle_frames(game: &mut Game, frames: usize) {
    for _ in 0..frames {
        game.frame();
    }
}

// ---------------------------------------------------------------------------
// Test 1: seed the terminal stage directly and prove the end button tears H down.
//         (deterministic; isolates the exit mechanism from navigation timing)
// ---------------------------------------------------------------------------

#[test]
fn end_button_exits_h_scene_from_terminal_stage_13() {
    let Some(mut game) = open_resumed() else {
        return;
    };
    settle_frames(&mut game, 60);

    // The ONE documented engine-state normalisation used across the M7/M8 H tests: the retail
    // save was captured with the late-night dispatcher gate closed (var364 == 1).
    game.vm.vars.set(364, 0);
    settle_frames(&mut game, 40);

    // Minimal terminal-stage precondition: stage index var62 == 13 (event 10 page 2).
    game.vm.vars.set(62, 13);
    game.vm.vars.set(61, 0);
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.set_choice_cursor(0);
    // Auto-advance clears the message window, so the CE63-managed system var 12 goes 0.
    for _ in 0..120 {
        game.frame();
        if game.vm.vars.get_sys(12) == 0 {
            break;
        }
    }

    assert_eq!(game.vm.vars.get(62), 13, "seeded terminal stage index");
    assert_eq!(
        game.vm.vars.get(64),
        1,
        "pajama flag var64 is set on the H path"
    );
    assert_eq!(
        game.vm.vars.get_sys(12),
        0,
        "message window (system var 12) must be idle before the end button can dispatch"
    );
    let (x, y) = endbutton_pos(&game).expect("stage 13 draws Picture/endbutton.png (picture 294)");
    assert!(
        file_present(&game, "hscene/h_s10_pose"),
        "H pose layer is on screen at stage 13"
    );

    let (seen, clicked) = press_end(&mut game, 300);
    assert!(clicked, "end button must be clickable");
    let _ = (x, y);

    assert!(
        seen.contains(&345),
        "event 25 must dispatch CE345 深夜終了コマンド (seen {seen:?})"
    );
    assert!(
        seen.contains(&222),
        "CE345 teardown saves through CE222 (seen {seen:?})"
    );
    assert_eq!(
        game.vm.vars.get(62),
        0,
        "exit resets the stage index var62 13 -> 0"
    );
    assert_eq!(
        game.vm.vars.get(64),
        0,
        "exit clears the pajama flag var64 (CE345 pc341)"
    );
    assert!(
        !file_present(&game, "hscene/h_s10_pose"),
        "the H pose layer is torn down on exit"
    );
    assert_eq!(
        game.stats.warnings, 0,
        "the whole exit sequence must be warning-free"
    );
}

// ---------------------------------------------------------------------------
// Test 2: the real navigation to stage 13, then the end button exits.
// ---------------------------------------------------------------------------

#[test]
fn real_navigation_to_stage_13_then_end_button_exits() {
    let Some(mut game) = open_resumed() else {
        return;
    };
    settle_frames(&mut game, 60);
    game.vm.vars.set(364, 0);
    settle_frames(&mut game, 40);

    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.set_choice_cursor(0);

    assert!(
        nav_to_terminal_stage(&mut game),
        "real pointer navigation must reach stage 13 (var62==13)"
    );
    assert_eq!(
        game.vm.vars.get(62),
        13,
        "reached the terminal stage by real clicks"
    );
    assert_eq!(
        game.vm.vars.get(64),
        1,
        "pajama flag set on the reached terminal stage"
    );

    // With auto-advance on, the message window is idle, so system var 12 == 0.
    for _ in 0..120 {
        game.frame();
        if game.vm.vars.get_sys(12) == 0 {
            break;
        }
    }
    let _ = endbutton_pos(&game).expect("stage 13 still shows the end button");

    let (seen, clicked) = press_end(&mut game, 300);
    assert!(clicked, "end button clickable after real navigation");
    assert!(
        seen.contains(&345),
        "CE345 runs from the reached terminal stage (seen {seen:?})"
    );
    assert_eq!(
        game.vm.vars.get(62),
        0,
        "real-path exit resets var62 13 -> 0"
    );
    assert_eq!(game.vm.vars.get(64), 0, "real-path exit clears var64");
    assert_eq!(game.stats.warnings, 0, "real-path exit is warning-free");
}

// ---------------------------------------------------------------------------
// Test 3: the exit gate — an open message window (system var 12 == 1) blocks the dispatch,
//         which is exactly why the earlier M7 playthrough saw "no exit".
// ---------------------------------------------------------------------------

#[test]
fn end_button_exit_is_gated_on_the_message_window() {
    let Some(mut game) = open_resumed() else {
        return;
    };
    settle_frames(&mut game, 60);
    game.vm.vars.set(364, 0);
    settle_frames(&mut game, 40);

    // No auto-advance: the retail save resumes with a message window open (system var 12 == 1).
    assert!(
        nav_to_terminal_stage(&mut game),
        "reach stage 13 without auto-advance"
    );
    assert_eq!(game.vm.vars.get(62), 13, "at terminal stage 13");
    let sys12_before = game.vm.vars.get_sys(12);
    assert_eq!(
        sys12_before, 1,
        "save resumes with the message window open (system var 12 == 1)"
    );

    let (seen, clicked) = press_end(&mut game, 90);
    assert!(
        clicked,
        "end button still hoverable while the message window is open"
    );
    // Event 11 still records the press (var61 = 49) ...
    assert_eq!(
        game.vm.vars.get(61),
        49,
        "the end button press writes var61=49 through event 11"
    );
    // ... but event 25 page 0 pc9 (9000012 == 0) skips CE345, so the H scene stays put.
    assert!(
        !seen.contains(&345),
        "CE345 must NOT run while system var 12 != 0 (seen {seen:?})"
    );
    assert_eq!(
        game.vm.vars.get(62),
        13,
        "the H scene does not exit while the message window is open"
    );
    assert_eq!(game.stats.warnings, 0, "the blocked press is warning-free");
}
