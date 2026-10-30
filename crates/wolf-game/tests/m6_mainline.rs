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
