//! M6 main-line slice: the new-game story on SampleMapA runs on top of the engine's
//! 108-coroutine background system, and the scheduler keeps turning without deadlock.
//!
//! `Game::start_map_event_by_name` is the code path a real new game takes after the
//! title menu answers (`docs/MAP_AUDIT.md` section 7.5). It starts the engine events
//! first, so the 10 always-on coroutines plus the auto-start event are alive before
//! the story script runs. This test drives that exact path for 900 frames and pins the
//! facts the M6 handoff quotes: peak concurrency, instruction progress, the started
//! event set, and a clean diagnostic-free screen full of live pictures.

use std::path::PathBuf;

use wolf_game::game::ENGINE_ALWAYS;
use wolf_game::Game;

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

#[test]
fn new_game_story_runs_on_the_background_scheduler_without_deadlock() {
    let Some(mut game) = open_game() else { return };
    // A headless capture dismisses messages immediately; choices are the one thing it
    // must answer, so pick option 0 after two frames like a player would.
    game.auto_advance = true;
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.start_map_event_by_name("SampleMapA", 0)
        .expect("new-game entry point exists");

    let mut frames_with_progress = 0usize;
    for _ in 0..900 {
        let before = game.stats.instructions;
        game.frame();
        if game.stats.instructions > before {
            frames_with_progress += 1;
        }
    }

    assert_eq!(game.stats.frames, 900);
    assert!(
        frames_with_progress > 600,
        "the scheduler advanced the story on most frames (no deadlock); got {frames_with_progress}"
    );
    assert!(
        game.stats.instructions > 50_000,
        "900 frames execute the story: {} instructions",
        game.stats.instructions
    );
    assert!(
        game.stats.peak_instances >= 9,
        "UI + story coroutines run concurrently: peak {}",
        game.stats.peak_instances
    );
    assert_eq!(game.stats.warnings, 0, "no interpreter warnings");
    assert!(game.screen.len() > 30, "the room and HUD stay drawn: {} pictures", game.screen.len());

    // Every engine-always coroutine was started, plus the auto-start event and the
    // boot-time conditional event 38. The rest of the 97 are variable-gated UI states
    // that a slice does not necessarily visit; the registry itself is what must exist.
    for id in ENGINE_ALWAYS {
        assert!(
            game.vm.started_events.contains(id),
            "ENGINE_ALWAYS event {id} must run in every session"
        );
    }
    assert!(game.vm.started_events.contains(&241), "auto-start event 241 runs");
    assert!(game.vm.started_events.contains(&38), "boot-time conditional event 38 runs");
    assert!(
        game.vm.started_events.len() >= 13,
        "at least 10 always + auto-start + implicit + 38: {}",
        game.vm.started_events.len()
    );
}


#[test]
fn the_title_menu_new_game_path_no_longer_leaves_title_art_over_the_room() {
    let Some(mut game) = open_game() else { return };
    game.auto_advance = true;
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.boot().expect("the boot map has its autorun event");

    // The splash -> key wait -> title menu -> confirm chain needs about 800 frames
    // before a single decide press walks the example title menu into the game.
    for frame in 0..3200 {
        if frame == 800 {
            game.key_bits = wolf_game::key::OK;
        }
        game.frame();
        game.key_bits = 0;
    }

    assert_eq!(game.stats.frames, 3200);
    assert_eq!(game.stats.warnings, 0, "no interpreter warnings on the boot path");
    assert!(
        game.stats.instructions > 100_000,
        "the title path runs the splashes plus the room: {} instructions",
        game.stats.instructions
    );
    assert!(game.screen.len() > 30, "the room and HUD are drawn: {} pictures", game.screen.len());
    // The sample title art lives in a negative (whole-screen) slot. Once the menu
    // answers with its 130, that layer belongs to the old map and must be gone.
    let stale: Vec<String> = game
        .screen
        .iter()
        .filter(|picture| picture.number < 0 && picture.file.contains("TitleGraphic"))
        .map(|picture| picture.file.clone())
        .collect();
    assert!(stale.is_empty(), "title art still on screen: {stale:?}");
}



#[test]
fn clicking_the_reading_icon_in_the_bedroom_runs_the_first_playable_action() {
    let Some(mut game) = open_game() else { return };
    game.auto_advance = true;
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.start_map_event_by_name("Map003", 1)
        .expect("the new-game hook exists");

    // Let the hook teleport into SampleMapA and the room/HUD settle.
    for _ in 0..1500 {
        game.frame();
    }
    assert!(game.screen.len() > 30, "the bedroom is drawn before the click");
    assert_eq!(game.stats.warnings, 0);

    // The bedroom menu is mouse driven: one left click on the 读书 icon
    // (885,240 in the 1024x576 frame) starts the reading action.
    game.auto_advance = false;
    game.mouse = [884, 240];
    game.mouse_buttons = 1;
    game.frame();
    game.mouse_buttons = 0;
    for _ in 0..30 {
        game.frame();
    }

    // The result is drawn as a floating text picture, not a 101 message window.
    let texts: Vec<&str> = game
        .screen
        .iter()
        .map(|picture| picture.text.as_str())
        .chain(game.messages.iter().map(|message| message.text.as_str()))
        .collect();
    assert!(
        texts.iter().any(|text| text.contains("理解")),
        "the reading action reports its effect: {texts:?}"
    );
    assert_eq!(game.stats.warnings, 0);
}



fn click_icon(game: &mut Game, x: i32, y: i32) {
    game.auto_advance = false;
    game.mouse = [x, y];
    game.mouse_buttons = 1;
    game.frame();
    game.mouse_buttons = 0;
    game.auto_advance = true;
    for _ in 0..30 {
        game.frame();
    }
}

#[test]
fn the_bedroom_actions_can_be_played_one_after_another() {
    let Some(mut game) = open_game() else { return };
    game.auto_advance = true;
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.start_map_event_by_name("Map003", 1)
        .expect("the new-game hook exists");
    for _ in 0..1500 {
        game.frame();
    }

    click_icon(&mut game, 884, 240); // 读书
    let after_read: Vec<&str> = game.screen.iter().map(|p| p.text.as_str()).collect();
    assert!(
        after_read.iter().any(|text| text.contains("理解")),
        "the reading action reports its effect: {after_read:?}"
    );

    click_icon(&mut game, 195, 475); // 钻研
    let after_training: Vec<&str> = game.screen.iter().map(|p| p.text.as_str()).collect();
    assert!(
        after_training.iter().any(|text| text.contains("自主训练")),
        "the training action reports its effect: {after_training:?}"
    );
    assert_eq!(game.stats.warnings, 0);
}

