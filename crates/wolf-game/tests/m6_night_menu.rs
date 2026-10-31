//! M6 night command menu: at the day-2 evening bedroom the player opens the night
//! command menu (common event 245, 夜コマンドメニュー) with a natural mouse gesture —
//! hover the sister sprite (picture 120, Picture/imouto02.png at 240,340) and press
//! the Decide key — and the menu really draws its five button rows.
//!
//! This exercises SampleMapA event 1 page 0 (the night sister-click detector) and the
//! autorun event 4 page 0 (メニュー処理) up to its pc335 -> CE245 call. It deliberately
//! stops at the menu and its 是/算了 confirmation: the CE328 pre-pc700 abort discovered
//! in docs/research/M6_NIGHT_FLOW.md is a separate problem and is NOT solved here.
//!
//! Input path, from the code:
//!   * ev1 page0 pc5 op124 [self100,0x4000,120,8] = picture field 8 = hover test on
//!     picture 120; pc6 gates on hover==1; pc8 op123 [self101,16] polls the keys;
//!     pc9 compares the poll to 10 (CODE_OK / Decide); pc11 writes var68 = 1.
//!   * ev4 page0 runs autorun while var68==1 && var69==1 && var62==0, and at pc335
//!     op210 [500245] calls CE245. BUT pc1 hard-breaks the whole page when
//!     var295==1 (the first-dinner skill TIPS flag set by CE326 pc1648). Normal play
//!     clears it when the player opens the status/skill screen; a straight fast-forward
//!     leaves it 1, so this test normalizes var295 = 0 like a player who closed the tip.
//!
//! Setup reaches the day-2 evening bedroom by the canonical new game followed by the
//! real day loop: day-1 evening -> 今天还是睡吧 (CE329) -> day 2 morning -> one CE252
//! day command -> the evening scene (dinner, CE326) -> the evening bedroom. The sleep at the
//! first night seeds var63 = 1 (the flag the sister-click branch leaves behind), which
//! is how m6_mainline drives the same loop headlessly.

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

/// CE251's day-command rows that route to CE252 (normal daytime tasks), in the
/// (label, var48 id, stat raised) form the menu can produce. The task on row 1 is a
/// retail-PRNG draw, so the helper picks the first CE252 row like a player would.
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

/// The pointer point at the visible centre of a picture. The retail PRNG picks the
/// sister sprite pose, and a top-left-anchored sprite stores its origin, not its
/// centre, in x/y; computing from the real texture size keeps the hover faithful.
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

/// Drive the canonical new game through day 1 into the day-2 evening bedroom.
///
/// Uses auto-choice to answer the character-creation and sleep prompts; the first-night
/// sleep seeds var63 = 1 exactly as the sister-click branch does on a real click, which
/// headless input cannot reproduce from its hit-test. A CE252 day command spends day 2
/// to 17:00, then the evening scene plays out to the evening bedroom. Leaves the game with
/// auto-choice off and auto-advance on.
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

    // Day 2: auto-answer, spend the day with the first CE252 task row, then let the
    // evening scene settle to the bedroom (18:30 or 19:30, a script-random choice).
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

/// From the day-2 evening bedroom, normalize the stale first-dinner tip and open the night
/// menu with the natural hover-sister + Decide gesture. Leaves the game at the open menu.
fn open_night_menu(game: &mut Game) {
    // Documented state normalization: var295==1 means the first-dinner skill TIPS was
    // never dismissed, and ev4 page0 pc1 breaks on it. A player who opened and closed
    // the status/skill screen leaves it 0. Confirm the gate is real before clearing it.
    assert_eq!(
        game.vm.vars.get(295),
        1,
        "the first-dinner tip flag (var295) is set on the fast-forwarded evening"
    );
    game.vm.vars.set(295, 0);
    game.vm.vars.set(68, 0);

    // The natural gesture: hover picture 120 (the sister sprite) at its real centre
    // and press Decide. The retail PRNG decides which sister sprite/pose is drawn
    // (and whether it is centre- or top-left-anchored), so derive the centre from the
    // picture instead of the old placeholder's fixed (240,340).
    let Some(sister_center) = picture_center(game, 120) else {
        panic!("picture 120 has no readable texture");
    };
    game.mouse = [sister_center.0, sister_center.1];
    for _ in 0..8 {
        game.frame();
    }
    decide(game);
    for _ in 0..40 {
        game.frame();
    }
}

fn menu_picture_numbers(game: &Game) -> Vec<i32> {
    game.screen
        .iter()
        .map(|picture| picture.number)
        .filter(|number| (1200..=1213).contains(number))
        .collect()
}

#[test]
fn hovering_the_sister_and_pressing_decide_opens_the_night_command_menu() {
    let Some(mut game) = day2_evening_game() else { return };

    // The bedroom is day 2 in the evening, warning-free, with no menu up and no var64 injected.
    assert_eq!(game.vm.vars.get(160), 2, "the day counter is day 2");
    assert!(
        game.vm.vars.get(4) >= 18,
        "the clock is evening ({}:{})",
        game.vm.vars.get(4),
        game.vm.vars.get(5)
    );
    assert_eq!(game.vm.vars.get(5), 30, "the evening settles on the half hour");
    assert_eq!(game.stats.warnings, 0, "the evening scene is warning-free");
    assert_eq!(game.vm.vars.get(64), 0, "no var64 injection is needed");
    assert_eq!(game.vm.vars.get(69), 1, "the evening-day flag var69 is set");
    assert!(game.choices.is_none(), "no prompt before the gesture");
    assert!(
        menu_picture_numbers(&game).is_empty(),
        "the night command menu is not open yet"
    );

    // The sister sprite that the hit-test polls is picture 120; its pose, anchor and
    // origin all depend on the retail PRNG's pick, so assert the real sprite rather
    // than a fixed coordinate or anchor.
    let sister = game
        .screen
        .iter()
        .find(|picture| picture.number == 120)
        .expect("the sister sprite is picture 120");
    assert!(
        sister.file.contains("imouto"),
        "picture 120 is a sister sprite, got {:?}",
        sister.file
    );
    assert!(!sister.hidden, "the sprite is visible to hover");

    open_night_menu(&mut game);

    // The hover+Decide set var68, which lets autorun ev4 page0 reach its CE245 call.
    assert_eq!(game.vm.vars.get(68), 1, "the sister-click flag var68 is set");
    assert_eq!(game.stats.warnings, 0, "opening the menu is warning-free");
    assert!(
        game.vm
            .instances()
            .iter()
            .any(|instance| instance.event == 245),
        "the event-245 night command menu coroutine is alive"
    );

    // The five button rows are drawn: a base_bottan.png background plus its text label.
    let numbers = menu_picture_numbers(&game);
    for expected in [1200, 1201, 1202, 1203, 1204, 1205, 1206, 1207, 1212, 1213] {
        assert!(
            numbers.contains(&expected),
            "menu picture {expected} is drawn; got {numbers:?}"
        );
    }
    let texts: Vec<&str> = game.screen.iter().map(|picture| picture.text.as_str()).collect();
    for label in ["身体接触", "沟通", "洗澡", "泡茶", "今天先休息吧"] {
        assert!(
            texts.iter().any(|text| text.contains(label)),
            "the menu shows {label}: {texts:?}"
        );
    }
    assert!(game.choices.is_none(), "the open menu is not a 102 prompt");
}

#[test]
fn clicking_rest_in_the_night_menu_raises_the_confirm_choice() {
    let Some(mut game) = day2_evening_game() else { return };
    open_night_menu(&mut game);
    assert!(
        game.vm
            .instances()
            .iter()
            .any(|instance| instance.event == 245),
        "the night command menu is open before the rest click"
    );
    assert_eq!(game.stats.warnings, 0);

    // Click 今天先休息吧 (picture 1212/1213 at 512,450). CE245 asks 是/算了.
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
        "今天先休息吧 raises the 是/算了 confirmation"
    );
    assert_eq!(game.stats.warnings, 0, "the confirmation is warning-free");
    // The CE328 pre-pc700 abort (docs/research/M6_NIGHT_FLOW.md) is out of scope: the
    // test stops at the confirmation and does not press 是.
}
