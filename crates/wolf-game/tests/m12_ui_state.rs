//! M12: the top-bar HUD must stay live after a save load.
//!
//! Regression for UI-10. SavePart5 stores the HUD text pictures (6200/6210/6211
//! for the clock and 6235/6236/6246/6251 for action/energy/desire/money) as
//! literal strings rendered at save time. CE216 and CE217 are linear
//! run_condition == 2 pages that repaint those pictures from the live variables.
//! Loading a save used to start them once, but the edge-triggered scheduler never
//! replayed them, so sleeping changed 2000014 (75 -> 105 -> 115) while the HUD
//! stayed at 75/120.
//!
//! The fix whitelists 216/217 as per-frame presentation pages. These tests pin:
//!   1. loading SaveData01 starts 216/217 and not 506 (DLC eye overlay);
//!   2. after a load->sleep path the HUD text equals the internal variables.
//!
//! Run with WOLF_DATA_ROOT=work/all.

use std::path::PathBuf;

use wolf_game::Game;

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

fn save_path() -> PathBuf {
    repo_root().join("work/retail_backup/Save/SaveData01.sav")
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

fn hud_text(game: &Game, number: i32) -> String {
    let parts: Vec<String> = game
        .text_quads()
        .into_iter()
        .filter(|quad| quad.number == number)
        .map(|quad| quad.text)
        .collect();
    assert!(!parts.is_empty(), "HUD picture {number} must render text");
    parts.concat().trim().to_owned()
}

fn fraction(text: &str) -> (i32, i32) {
    let (left, right) = text
        .split_once('/')
        .unwrap_or_else(|| panic!("not a fraction: {text:?}"));
    let left = left
        .split_whitespace()
        .last()
        .expect("fraction numerator")
        .parse()
        .unwrap_or_else(|_| panic!("bad numerator in {text:?}"));
    let right = right
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("bad denominator in {text:?}"));
    (left, right)
}

fn hud_values(game: &Game) -> (i32, i32, i32, i32, i32, i32) {
    let (action, action_max) = fraction(&hud_text(game, 6235));
    let (energy, energy_max) = fraction(&hud_text(game, 6236));
    let desire = hud_text(game, 6246)
        .trim_end_matches('％')
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("bad desire: {:?}", hud_text(game, 6246)));
    let money_text = hud_text(game, 6251);
    let money = money_text
        .split_whitespace()
        .next()
        .expect("money value")
        .parse()
        .unwrap_or_else(|_| panic!("bad money: {money_text:?}"));
    (action, action_max, energy, energy_max, desire, money)
}

fn load_save01() -> Option<Game> {
    if !save_path().is_file() {
        eprintln!("skipping: {} is absent", save_path().display());
        return None;
    }
    let mut game = open_game()?;
    game.auto_advance = true;
    game.auto_choice = true;
    game.load_save(&save_path())
        .unwrap_or_else(|err| panic!("load SaveData01: {err}"));
    Some(game)
}

#[test]
fn loading_save01_starts_the_hud_events_and_never_the_dlc_overlay() {
    let Some(mut game) = load_save01() else { return };

    let running = game.auto_events.running_ids();
    assert!(
        running.contains(&216),
        "CE216 (status bar) must be resumed on load, running={running:?}"
    );
    assert!(
        running.contains(&217),
        "CE217 (clock) must be resumed on load, running={running:?}"
    );
    assert!(
        !running.contains(&506),
        "CE506 (DLC eye overlay) must not be resumed, running={running:?}"
    );

    // A real frame must keep both HUD pages resident while their condition holds,
    // and must never let the DLC overlay appear.
    for frame in 0..3 {
        game.key_bits = wolf_game::key::bits_for_code(10);
        game.frame();
        assert!(
            !game.vm.instances().iter().any(|instance| instance.event == 506),
            "event 506 must not run after load (frame {frame})"
        );
        assert!(
            !game.screen
                .get(-6)
                .map(|picture| picture.file.contains("dlc_imouto01_eye"))
                .unwrap_or(false),
            "DLC eye overlay must not be drawn (frame {frame})"
        );
    }
    // After each frame the replay may have finished; another frame must start it
    // again (the condition is still true), so verify via the scheduler's own state
    // that the pair remains tracked rather than duplicating.
    assert!(game.auto_events.running_ids().contains(&216));
    assert!(game.auto_events.running_ids().contains(&217));
}

#[test]
fn load_then_sleep_repaints_the_hud_from_the_live_variables() {
    let Some(mut game) = load_save01() else { return };

    // The saved HUD is the pre-sleep snapshot: 90日 21:40, action 75/120.
    assert_eq!(game.vm.vars.get(160), 90, "SaveData01 day");
    let (saved_action, saved_max) = fraction(&hud_text(&game, 6235));
    assert_eq!(saved_action, 75, "SavePart5 snapshot starts at 75");
    assert_eq!(saved_max, 120);

    game.start_engine_events();
    assert!(game.start_event(329), "CE329 (sleep) must exist");
    for _ in 0..1600 {
        game.key_bits = wolf_game::key::bits_for_code(10);
        game.frame();
    }

    // Sleep advances the day and refills all three gauges. The exact values are
    // whatever the event wrote; the point is that the HUD equals them, not the
    // old SavePart5 literals.
    assert_eq!(game.vm.vars.get(160), 91, "sleep advances to day 91");
    let (action, action_max, energy, energy_max, desire, money) = hud_values(&game);
    assert_eq!((action, action_max), (game.vm.vars.get(14), game.vm.vars.get(13)));
    assert_eq!((energy, energy_max), (game.vm.vars.get(16), game.vm.vars.get(15)));
    assert_eq!(desire, game.vm.vars.get(18));
    assert_eq!(money, game.vm.vars.get(7));
    assert_ne!(action, 75, "HUD must not stay at the SavePart5 snapshot");
    assert_eq!(action, game.vm.vars.get(14));
    assert_eq!(action_max, game.vm.vars.get(13));
}
