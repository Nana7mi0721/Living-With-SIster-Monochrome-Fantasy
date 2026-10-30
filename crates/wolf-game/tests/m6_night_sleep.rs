//! M6 full natural sleep: from the day-2 19:30 bedroom the player opens the night
//! menu with a real mouse gesture, chooses 今天先休息吧 -> 是, and CE329 (就寝処理)
//! rolls the clock over to day 3 08:30 — with no scripted stop of the CALL chain.
//!
//! This is the regression for the engine defect documented in
//! docs/research/CE328_ABORT.md: CE329 writes var69 = 0 at pc41, which is an
//! activation term of map event 4 page 0 (メニュー処理, Autorun) — the event whose
//! 210 CALL stack currently owns CE329. The per-frame page re-selection used to
//! stop that page and every descendant mid-call, so CE329 died before pc385 and
//! the day never advanced. crates/wolf-game/src/map_runtime.rs now defers a page
//! switch while the page's coroutine is blocked in WaitState::Child (a 210/300
//! call), letting the page command list run to completion as WOLF does.
//!
//! The only state normalization is var295 = 0: the fast-forwarded first dinner
//! leaves the skill TIPS flag set, and ev4 page0 pc1 hard-breaks on it; a player
//! who opened and closed the status screen has var295 = 0. Nothing seeds var63 or
//! var64: the click itself drives the real event chain.

use std::path::PathBuf;

use wolf_game::Game;

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".to_owned());
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

fn decide(game: &mut Game) {
    game.key_bits = wolf_game::key::OK;
    game.frame();
    game.key_bits = 0;
}

fn click_at(game: &mut Game, x: i32, y: i32) {
    game.mouse = [x, y];
    for _ in 0..10 {
        game.frame();
    }
    game.mouse_buttons = 1;
    for _ in 0..5 {
        game.frame();
    }
    game.mouse_buttons = 0;
}

fn day2_evening_game() -> Option<Game> {
    let mut game = open_game()?;
    game.auto_advance = true;
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.start_map_event_by_name("SampleMapA", 0)
        .expect("the new-game entry point exists");
    for _ in 0..1500 {
        game.frame();
    }

    // Day-1 night: the sister click leaves var63 = 1; choose 今天还是睡吧 -> 是的.
    game.auto_choice = false;
    game.vm.vars.set(63, 1);
    for _ in 0..120 {
        game.frame();
        if game.choices.is_some() {
            break;
        }
    }
    game.set_choice_cursor(2);
    decide(&mut game);
    for _ in 0..120 {
        game.frame();
        if game.choices.is_some() {
            break;
        }
    }
    game.set_choice_cursor(0);
    decide(&mut game);
    for _ in 0..400 {
        game.frame();
        if game.vm.vars.get(160) == 2 && game.vm.vars.get(4) == 8 && game.vm.vars.get(5) >= 30 {
            break;
        }
    }

    // Day 2: 学习会 spends the day, the dinner scene settles to 19:30.
    game.auto_choice = true;
    for _ in 0..350 {
        game.frame();
        if game
            .screen
            .get(1201)
            .map(|picture| !picture.text.is_empty())
            .unwrap_or(false)
        {
            break;
        }
    }
    click_at(&mut game, 512, 230);
    for _ in 0..1500 {
        game.frame();
        if game
            .vm
            .instances()
            .iter()
            .all(|instance| instance.event != 251)
        {
            break;
        }
    }
    for _ in 0..1200 {
        game.frame();
    }
    game.auto_choice = false;
    Some(game)
}

#[test]
fn the_natural_night_menu_sleep_rolls_over_to_day_three() {
    let Some(mut game) = day2_evening_game() else {
        return;
    };
    assert_eq!(game.vm.vars.get(160), 2);
    assert_eq!(game.vm.vars.get(4), 19);
    assert_eq!(game.vm.vars.get(5), 30);
    assert_eq!(game.stats.warnings, 0);

    // Normalize the undismissed first-dinner tip, then open the night menu with the
    // real hover-sister + Decide gesture (no var63/64 injection).
    assert_eq!(game.vm.vars.get(295), 1);
    game.vm.vars.set(295, 0);
    game.vm.vars.set(68, 0);
    // Hover the actual sister sprite; the retail PRNG picks its pose/centre.
    let sister = game
        .screen
        .get(120)
        .expect("the sister sprite is picture 120");
    game.mouse = [sister.x, sister.y];
    for _ in 0..8 {
        game.frame();
    }
    decide(&mut game);
    for _ in 0..40 {
        game.frame();
    }
    assert!(
        game.vm
            .instances()
            .iter()
            .any(|instance| instance.event == 245),
        "the night command menu is open"
    );

    // 今天先休息吧 -> 是.
    click_at(&mut game, 512, 450);
    let mut options: Option<Vec<String>> = None;
    for _ in 0..400 {
        game.frame();
        if let Some(choices) = &game.choices {
            options = Some(choices.options.clone());
            break;
        }
    }
    assert_eq!(
        options.as_deref(),
        Some(["是".to_owned(), "算了".to_owned()].as_slice()),
        "the rest command raises the confirm prompt"
    );
    game.set_choice_cursor(0);
    decide(&mut game);

    // CE329 就寝処理 must survive its own var69 = 0 write and reach the rollover at pc385+.
    let mut arrived = false;
    for _ in 0..2000 {
        game.frame();
        if game.vm.vars.get(160) == 3 && game.vm.vars.get(4) == 8 && game.vm.vars.get(5) >= 30 {
            arrived = true;
            break;
        }
    }
    assert!(
        arrived,
        "the natural sleep reaches day 3 08:30 without the CALL-chain teardown"
    );
    assert_eq!(
        game.stats.warnings, 0,
        "the whole natural sleep is warning-free"
    );
    assert_eq!(
        game.vm.vars.get(160),
        3,
        "the day counter advanced to day 3"
    );
    assert_eq!(game.vm.vars.get(6), 2, "weekday var6 is 2 (Wednesday)");
    assert_eq!(game.vm.vars.get(4), 8, "the morning starts at 8");
    assert_eq!(game.vm.vars.get(5), 30, "and 30 minutes");
    assert!(
        game.screen.len() > 20,
        "the day-3 morning scene is drawn: {} pictures",
        game.screen.len()
    );
}
