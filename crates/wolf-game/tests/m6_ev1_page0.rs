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
//! day-1 night (今天还是睡吧) -> day 2 morning -> one CE252 day command -> dinner ->
//! the day-2 evening bedroom. There var69 == 1 (the flag ニューゲーム処理 ev0 idx841 sets once and never clears),
//! var64 == 0, var65 == 0, var25 == var26 == var302 == 0 — so ev1 page 0's condition holds
//! while page 1 (var65==1) and page 2 (var64==1) do not; the highest satisfied page is 0.
//! Picture 120 is the real sister sprite. The retail Park-Miller event PRNG
//! (crates/wolf-vm/src/rng.rs) picks the pose here, so the sprite file, anchor and
//! position all vary by draw; the test therefore derives the pointer point from the
//! actual picture (anchor- and texture-aware) and does not pin the pose.
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
//! it is page 2's flag, not page 0's output, and is 0 again in the evening.

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

/// CE251's day-command rows that route to CE252 (normal daytime tasks).
const DAY_COMMANDS: [(&str, i32, Option<usize>); 6] = [
    ("给店里帮忙", 1, None),
    ("学习会", 2, Some(171)),
    ("基础锻炼", 3, Some(174)),
    ("攻击练习", 4, Some(170)),
    ("防御练习", 5, Some(173)),
    ("秘密特训", 6, Some(174)),
];

fn day_command(label: &str) -> Option<(i32, Option<usize>)> {
    DAY_COMMANDS
        .iter()
        .find(|(name, _, _)| label.contains(name))
        .map(|(_, id, stat)| (*id, *stat))
}

const DAY_ROW_PICS: [i32; 5] = [1201, 1203, 1205, 1207, 1209];

fn day_row_texts(game: &Game) -> Vec<String> {
    DAY_ROW_PICS
        .iter()
        .map(|&number| game.screen.get(number).map(|p| p.text.clone()).unwrap_or_default())
        .collect()
}

fn evening_work(game: &Game) -> bool {
    game.vm
        .instances()
        .iter()
        .any(|instance| matches!(instance.event, 251 | 300 | 301 | 326 | 406))
}

/// Pointer point at the visible centre of a picture (anchor- and texture-aware).
fn picture_center(game: &mut Game, number: i32) -> Option<(i32, i32)> {
    let (x, y, anchor, scale, file, window) = {
        let picture = game.screen.get(number)?;
        (
            picture.x,
            picture.y,
            picture.anchor,
            picture.scale.max(1),
            picture.file.clone(),
            picture.window,
        )
    };
    let (width, height) = match window {
        Some((width, height)) if width > 0 && height > 0 => (width, height),
        _ => {
            let (width, height) = game.assets.texture_size(&file)?;
            (width as i32, height as i32)
        }
    };
    let scale = f32::from(scale as i16) / 100.0;
    let (width, height) = (width as f32 * scale, height as f32 * scale);
    let (left, top) = if anchor == wolf_game::picture::ANCHOR_CENTRE {
        (x as f32 - width / 2.0, y as f32 - height / 2.0)
    } else {
        (x as f32, y as f32)
    };
    Some(((left + width / 2.0) as i32, (top + height / 2.0) as i32))
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

/// Reach the day-2 evening bedroom by the real day loop (identical to m6_night_menu.rs). The
/// only seed is var63 = 1 on day 1, which drives the canonical first-night sleep branch into
/// day 2; it is not page 0's output and is 0 again by the evening.
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

    // Day 2: auto-answer, pick the first CE252 task row, spend the day, then let the
    // evening scene settle to the bedroom (18:30 or 19:30 depending on the draw).
    game.auto_choice = true;
    let mut chosen: Option<(usize, String)> = None;
    for _ in 0..1200 {
        game.frame();
        let labels = day_row_texts(&game);
        for (row, text) in labels.iter().enumerate() {
            if day_command(text).is_some() {
                chosen = Some((row, text.clone()));
                break;
            }
        }
        if chosen.is_some() {
            break;
        }
    }
    let (row, _label) = chosen.unwrap_or_else(|| panic!("CE251 draws no CE252 day command"));
    click_at(&mut game, 512, 230 + 50 * row as i32);
    for _ in 0..2500 {
        game.frame();
        if game.vm.instances().iter().all(|instance| instance.event != 251) {
            break;
        }
    }
    let mut evening = false;
    for _ in 0..6000 {
        game.frame();
        if !evening_work(&game) && game.vm.vars.get(4) >= 18 {
            evening = true;
            break;
        }
    }
    assert!(evening, "day 2 evening settles into the bedroom");
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
    assert!(
        game.vm.vars.get(4) >= 18,
        "the clock is evening ({}:{})",
        game.vm.vars.get(4),
        game.vm.vars.get(5)
    );
    assert_eq!(game.vm.vars.get(5), 30, "the evening settles on the half hour");
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
    // The retail PRNG picks which sister sprite/pose is on screen; the pose also
    // decides whether the draw is anchored at its centre or at its top-left. Derive
    // the pointer point from the actual picture so the gesture stays faithful.
    let sister_center = picture_center(&mut game, 120).expect("picture 120 has a texture");
    let sister = game
        .screen
        .get(120)
        .expect("the sister sprite is picture 120");
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