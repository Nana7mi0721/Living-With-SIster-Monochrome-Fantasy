//! M6 EV1 page 2: the deep-night sister click (妹クリック判定（夜, SampleMapA event 1 page 2)
//! writes its page-2 output flag var63 = 1 when the player hovers picture 120 and presses
//! the Decide key, and that flag opens ev4 page 4 (深夜睡眠の三択: 恶作剧 / 轻轻把被子盖好 /
//! 今天还是睡吧).
//!
//! Reaching the page without injecting var63
//! -----------------------------------------
//! docs/research/EV1_NIGHT_SISTER.md §8 establishes that page 2's UNIQUE condition is
//! var64 == 1 (plus var25/var26/var302 == 0); var63 is page 2's OUTPUT, not its input.
//! var64 is written by CE328 (就寝前パジャマ) as the "deep night" branch flag. The real day
//! loop that would set it (CE245/CE290 night menu -> CE328 -> 睡衣/熬夜) is long and its
//! post-pc700 path is still WIP (docs/research/M6_NIGHT_FLOW.md), so this test reaches the
//! day-2 19:30 bedroom with the same real day loop as m6_night_menu.rs and then applies the
//! documented MINIMAL page-2 precondition directly: var64 = 1, var25 = var26 = var302 = 0,
//! leaving var63 = 0 so the observed var63 = 1 is genuinely produced by the click.
//!
//! Why the hover target is (140,340), not the daytime (240,340)
//! -----------------------------------------------------------
//! At 19:30 with var64 == 0, picture 120 is the sister sprite Picture/imouto02.png at
//! (240,340) and ev1 page 0 (var69 == 1) polls it. Setting var64 == 1 selects ev1 page 2
//! (verified: the active ev1 page becomes 2) AND activates ev2 (妹クリック判定) page 1, whose
//! condition var64 == 1 && var167 == 0 makes it REDRAW picture 120 as a 120x80 <SQUARE>
//! hit box centred at (140,340) (op150 [4144,120,0,120,80,1,0,140,340,100,...]). ev1 page 2
//! then hit-tests that same slot 120. So in the page-2 state picture 120 lives at (140,340),
//! anchor = ANCHOR_CENTRE, and hovering (240,340) misses the box ([80,200)x[300,380)); the
//! correct hover is the picture's centre (140,340). This is synthesizable, so no blocker.
//!
//! Input path exercised (bytecode refs in work/maps_full.json SampleMapA.mps ev1):
//!   * page 2 idx135 op124 [self100,0x4000,120,8] = picture 120 field 8 = hover test;
//!     idx136 gates hover == 1; idx138 op123 [self101,16] polls the keys (mask 16);
//!     idx139 compares to 10 (CODE_OK / Decide); idx141 op121 [2000063,1,0,0] writes var63 = 1.
//!   * ev4 page 4 (trigger 2) gates on var63 == 1 && var62 == 0 && var464 == 0 && var60 == 0
//!     and raises the 深夜睡眠 three-way choice; its pc91 op121 [2000064,0,0,0] clears the
//!     deep-night flag only once the player picks an option (after this test stops).

use std::path::PathBuf;

use wolf_game::Game;

/// Data root. Same resolution as the other wolf-game integration tests: default to the
/// extracted retail tree the Lead pins for M6, overridable with WOLF_DATA_ROOT.
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

/// Press and release the Decide key for one frame.
fn decide(game: &mut Game) {
    game.key_bits = wolf_game::key::OK;
    game.frame();
    game.key_bits = 0;
}

/// Hover a screen point for a few frames, then left-click (press a few frames, release),
/// the way a player drives the picture buttons.
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

/// Drive the canonical new game through day 1 into the day-2 19:30 bedroom.
///
/// Identical to m6_night_menu.rs: auto-answer character creation and the first-night
/// sleep (seeding var63 = 1 the way the real sister-click branch does), spend day 2 with
/// one 学习会 day command, then let the evening scene settle to the 19:30 bedroom.
/// Leaves the game with auto-choice off and auto-advance on.
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

    // Day-1 night: seed the sister-click flag, choose 今天还是睡吧 (index 2), confirm 是的.
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

    // Day 2: auto-answer, wait for the CE251 day-command button, spend the day with the
    // first command (学习会, at 512,230), then let the evening scene settle.
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
fn hovering_sister_at_deep_night_writes_var63_and_opens_the_sleep_choice() {
    let Some(mut game) = day2_evening_game() else {
        return;
    };

    // --- Baseline: day 2, 19:30, warning-free, page-2 conditions all false -------------
    assert_eq!(game.vm.vars.get(160), 2, "the day counter is day 2");
    assert_eq!(game.vm.vars.get(4), 19, "the clock is 19:00");
    assert_eq!(game.vm.vars.get(5), 30, "and 19:30");
    assert_eq!(game.stats.warnings, 0, "the evening scene is warning-free");
    assert_eq!(game.vm.vars.get(69), 1, "the evening-day flag var69 is set");
    assert_eq!(game.vm.vars.get(64), 0, "var64 (deep-night) is not set yet");
    assert_eq!(game.vm.vars.get(63), 0, "var63 (page-2 output) starts at 0");
    assert_eq!(game.vm.vars.get(25), 0, "page-2 precondition var25 == 0");
    assert_eq!(game.vm.vars.get(26), 0, "page-2 precondition var26 == 0");
    assert_eq!(game.vm.vars.get(302), 0, "page-2 precondition var302 == 0");
    assert!(game.choices.is_none(), "no prompt before the gesture");

    // The daytime sister sprite that ev1 page 0 would poll is picture 120 at (240,340).
    let day_sister = game
        .screen
        .get(120)
        .expect("the sister sprite is picture 120");
    assert_eq!(
        (day_sister.x, day_sister.y),
        (240, 340),
        "at 19:30 with var64 == 0 the sprite sits at (240,340)"
    );

    // --- Apply the documented MINIMAL page-2 precondition -----------------------------
    // Clear the stale first-dinner tip (ev4 page0 would otherwise hard-break), zero the
    // just-clicked flag, then set var64 = 1 and keep var63 = 0 so the write is observed.
    game.vm.vars.set(295, 0);
    game.vm.vars.set(68, 0);
    game.vm.vars.set(64, 1);
    game.vm.vars.set(25, 0);
    game.vm.vars.set(26, 0);
    game.vm.vars.set(302, 0);
    game.vm.vars.set(63, 0);
    // Let ev1 re-select page 2 and ev2 page 1 draw the night hit box on slot 120.
    for _ in 0..4 {
        game.frame();
    }

    assert_eq!(
        game.vm.vars.get(64),
        1,
        "the deep-night flag var64 is injected"
    );
    assert_eq!(game.vm.vars.get(63), 0, "the click has not happened yet");
    assert_eq!(
        game.vm.vars.get(167),
        0,
        "ev2 page 1 is active (var167 == 0)"
    );

    // ev2 page 1 repurposes picture 120 as the deep-night 120x80 hit box centred at (140,340).
    let night_hitbox = game
        .screen
        .get(120)
        .expect("the night hit box is picture 120");
    assert_eq!(
        (night_hitbox.x, night_hitbox.y),
        (140, 340),
        "at deep night picture 120 is the ev2 page-1 box centred at (140,340)"
    );
    assert_eq!(
        night_hitbox.window,
        Some((120, 80)),
        "the deep-night hit box is 120x80"
    );
    assert_eq!(night_hitbox.anchor, 1, "the box uses ANCHOR_CENTRE");
    assert!(
        night_hitbox.file.contains("SQUARE"),
        "picture 120 is the <SQUARE> hit box, got {:?}",
        night_hitbox.file
    );
    assert!(!night_hitbox.hidden, "the hit box is hoverable");

    // --- Natural gesture: hover picture 120's centre, then one Decide frame ------------
    game.mouse = [140, 340];
    for _ in 0..8 {
        game.frame();
    }
    decide(&mut game);

    // ev1 page 2 idx141 wrote var63 = 1.
    assert_eq!(
        game.vm.vars.get(63),
        1,
        "hovering picture 120 + Decide writes the page-2 output flag var63 = 1"
    );

    // --- ev4 page 4 then raises the 深夜睡眠 three-way choice --------------------------
    let mut options: Option<Vec<String>> = None;
    for _ in 0..200 {
        game.frame();
        if let Some(choices) = &game.choices {
            if choices.frames >= 2 {
                options = Some(choices.options.clone());
                break;
            }
        }
    }
    assert_eq!(
        options.as_deref(),
        Some(
            [
                "恶作剧".to_owned(),
                "轻轻把被子盖好".to_owned(),
                "今天还是睡吧".to_owned()
            ]
            .as_slice()
        ),
        "var63 = 1 opens the ev4 page-4 deep-night sleep three-way choice"
    );
    assert_eq!(game.stats.warnings, 0, "the whole flow is warning-free");
}
