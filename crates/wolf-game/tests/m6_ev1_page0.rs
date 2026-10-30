//! M6 ev1 page 0: the DAY/evening sister click (SampleMapA event 1 page 0) can be entered
//! naturally and the pointer click synthesized with no output-variable injection. The click
//! writes var68 = 1, which ev4 page 0 (メニュー処理) consumes to call CE245 (夜コマンドメニュー).
//!
//! Relationship to the night page (m6_ev1_page2.rs)
//! ------------------------------------------------
//! At deep night (var64 == 1) ev2 妹クリック判定 page 1 REDRAWS picture 120 as a 120x80
//! <SQUARE> hit box centred at (140,340), because the real sister sprite is not on screen
//! then. Page 0 is different: it is active in the ordinary day/evening state where the
//! sister SPRITE itself is drawn, so there is no redraw — page 0 hit-tests the actual
//! sprite. This test documents and exercises that case.
//!
//! State and hit box (measured)
//! ----------------------------
//! Reached by the same real day loop as m6_night_menu.rs / m6_mainline.rs: new game ->
//! day-1 night (今天还是睡吧) -> day 2 morning -> one 学习会 -> dinner -> day-2 19:30
//! bedroom. There var69 == 1 (the flag ニューゲーム処理 ev0 idx841 sets once and never clears),
//! var64 == 0, var65 == 0, var25 == var26 == var302 == 0 — so ev1 page 0's condition holds
//! while page 1 (var65==1) and page 2 (var64==1) do not; the highest satisfied page is 0.
//! Picture 120 is the real sister sprite. The retail MSVC-LCG PRNG (crates/wolf-vm/src/rng.rs)
//! picks the pose here: this build draws Picture/imouto04.png centre-anchored at (388,368),
//! whereas the old xorshift placeholder drew imouto02.png top-left at (240,340). The test
//! therefore hovers the sprite's actual centre and does not pin the random-dependent pose.
//! ev2 cannot interfere: its page 0 needs var65 == 1 and its page 1 needs var64 == 1 &&
//! var167 == 0; both are false here, so picture 120 stays the sprite (no <SQUARE>).
//!
//! Input chain (bytecode refs in work/maps_full.json SampleMapA.mps ev1 page 0):
//!   * idx5 op124 [self100,0x4000,120,8] = picture 120 field 8 = hover test -> self var0 (slot 100);
//!     idx6 gates self100 == 1; idx8 op123 [self101,16] polls the keys (mask 16);
//!     idx9 compares the poll to 10 (CODE_OK / Decide); idx11 op121 [2000068,1,0,0] writes var68 = 1.
//!   * ev4 page 0 (auto, cond var68 == 1 && var69 == 1 && var62 == 0) reaches pc335
//!     op210 [500245] -> CE245; its pc1 hard-breaks when var295 == 1 (first-dinner TIPS), so a
//!     fast-forwarded run must normalize var295 = 0 like a player who closed the tip.
//!
//! Boundary: only the minimum is seeded — nothing is done to ev1; var68 is NEVER injected,
//! it is asserted 0 before the gesture and observed 1 after. var63 = 1 is seeded only on
//! day 1 to drive the canonical day-loop sleep branch into day 2 (the m6_mainline technique);
//! it is page 2's flag, not page 0's output, and is 0 again at 19:30.

use std::path::PathBuf;

use wolf_game::Game;

/// Data root. Same resolution as the other wolf-game integration tests.
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

/// Hover a screen point for a few frames, then left-click (press a few frames, release).
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

/// Reach the day-2 19:30 bedroom by the real day loop (identical to m6_night_menu.rs). The
/// only seed is var63 = 1 on day 1, which drives the canonical first-night sleep branch into
/// day 2; it is not page 0's output and is 0 again by 19:30.
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

    // Day-1 night: seed the sleep-branch flag, choose 今天还是睡吧 (index 2), confirm 是的.
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

    // Day 2: auto-answer, spend the day with the first command (学习会 at 512,230), then let
    // the evening scene settle to the 19:30 bedroom.
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
        if game.vm.instances().iter().all(|instance| instance.event != 251) {
            break;
        }
    }
    for _ in 0..1200 {
        game.frame();
    }
    game.auto_choice = false;
    Some(game)
}

fn menu_picture_numbers(game: &Game) -> Vec<i32> {
    game.screen
        .iter()
        .map(|picture| picture.number)
        .filter(|number| (1200..=1213).contains(number))
        .collect()
}

#[test]
fn daytime_sister_sprite_click_writes_var68_and_opens_the_night_command_menu() {
    let Some(mut game) = day2_evening_game() else { return };

    // --- The day/evening bedroom state, warning-free ---------------------------------
    assert_eq!(game.vm.vars.get(160), 2, "the day counter is day 2");
    assert_eq!(game.vm.vars.get(4), 19, "the clock is 19:00");
    assert_eq!(game.vm.vars.get(5), 30, "and 19:30");
    assert_eq!(game.stats.warnings, 0, "the evening scene is warning-free");

    // ev1 page 0's condition holds; the higher pages' conditions do not, so page 0 is active.
    assert_eq!(game.vm.vars.get(69), 1, "page-0 condition var69 == 1 (day/evening flag)");
    assert_eq!(game.vm.vars.get(64), 0, "page-2 condition var64 == 0");
    assert_eq!(game.vm.vars.get(65), 0, "page-1 condition var65 == 0");
    assert_eq!(game.vm.vars.get(25), 0, "page-0 condition var25 == 0");
    assert_eq!(game.vm.vars.get(26), 0, "page-0 condition var26 == 0");
    assert_eq!(game.vm.vars.get(302), 0, "page-0 condition var302 == 0");

    // The expected output var68 is NOT injected: it must start at 0.
    assert_eq!(game.vm.vars.get(68), 0, "the page-0 output flag var68 starts at 0 (not injected)");
    assert!(game.choices.is_none(), "no prompt before the gesture");

    // --- Hit box: the real sister sprite, not a redrawn square ------------------------
    let sister = game
        .screen
        .get(120)
        .expect("the sister sprite is picture 120");
    // The retail PRNG picks which sister sprite is on screen (imouto04 centre-anchored
    // here; the old xorshift placeholder picked imouto02 top-left). Pin the hit box to
    // whatever the real sprite currently is instead of a random-dependent coordinate.
    let sister_center = (sister.x, sister.y);
    assert_eq!(
        sister.anchor, 1,
        "the sister sprite is centre-anchored (no <SQUARE> redraw)"
    );
    assert_eq!(sister.scale, 100, "the sprite draws at scale 100");
    assert_eq!(sister.kind as u8, 0, "picture 120 is a sprite, not a solid <SQUARE>");
    assert!(
        sister.file.contains("imouto"),
        "picture 120 is a sister sprite (imouto*.png), got {:?}",
        sister.file
    );
    assert!(!sister.hidden, "the sprite is visible to hover");

    // --- Documented normalisation, then the natural gesture ---------------------------
    // Clear the stale first-dinner tip so the downstream ev4 page 0 can run past its pc1
    // break (a player who closed the status/skill screen leaves var295 == 0).
    assert_eq!(game.vm.vars.get(295), 1, "the first-dinner tip flag is set on the fast-forward");
    game.vm.vars.set(295, 0);
    game.vm.vars.set(68, 0);

    // Hover the sister sprite for ~8 frames, then one Decide frame.
    game.mouse = [sister_center.0, sister_center.1];
    for _ in 0..8 {
        game.frame();
    }
    decide(&mut game);

    // ev1 page 0 idx11 wrote var68 = 1 — the click was synthesized naturally.
    assert_eq!(
        game.vm.vars.get(68),
        1,
        "hovering the sister sprite + Decide writes the page-0 output flag var68 = 1"
    );
    assert_eq!(game.vm.vars.get(63), 0, "page 2's flag var63 was not touched");
    assert_eq!(game.stats.warnings, 0, "the click frame is warning-free");

    // --- Real downstream: ev4 page 0 consumes var68 and calls CE245 (夜コマンドメニュー) --
    // Let ev4 page 0 run its CE245 call and CE245 draw the five button rows.
    for _ in 0..40 {
        game.frame();
    }
    assert_eq!(game.vm.vars.get(68), 1, "var68 stays set");
    assert!(
        game.vm
            .instances()
            .iter()
            .any(|instance| instance.event == 245),
        "ev4 page 0 reached op210 [500245]: the CE245 night command menu coroutine is alive"
    );
    let numbers = menu_picture_numbers(&game);
    for expected in [1200, 1201, 1202, 1203, 1204, 1205, 1206, 1207, 1212, 1213] {
        assert!(
            numbers.contains(&expected),
            "menu picture {expected} is drawn downstream; got {numbers:?}"
        );
    }
    assert_eq!(game.stats.warnings, 0, "the whole flow is warning-free");
}