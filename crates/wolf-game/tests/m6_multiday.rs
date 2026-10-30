//! M6 multi-day loop regression: the player-facing day loop really repeats, not just
//! once. Starting from the canonical new game (day 1, 18:30 bedroom), this test drives
//! the SAME real transitions a player performs over and over:
//!
//!   * the night sister page (SampleMapA event 1 / メニュー処理 event 4) -> 今天还是睡吧 ->
//!     就寝処理 (common event 329) rolls the clock to the next morning (08:30);
//!   * the CE251 day-command menu (the first picture button at 512,230) spends the day
//!     (08:30 -> 17:00) and its action changes the relevant stats;
//!   * the evening scene (dinner, CE326) then plays out to the 19:30 bedroom, where the
//!     loop can sleep again.
//!
//! The setup is copied verbatim from
//! m6_mainline.rs::the_bedroom_sleep_menu_choice_rolls_over_to_a_new_day, which proves
//! day 1 -> day 3. This file runs TWO full CE251+dinner+sleep loops (day 1 -> day 2 ->
//! day 3) and then sleeps once more into day 4 08:30, and inserts a live save -> reload at
//! the day-3 19:30 bedroom mid-way through the run.
//!
//! Why day 4 08:30 is the stopping point: on day 4 the morning no longer opens a picture-
//! button day-command menu; it enters CE253 (地域貢献) + CE239 (■戦闘呼び出し, the battle
//! call) and stalls on a non-interactive scripted wait (clock frozen at 15:00, no picture
//! text drawn). Battle is a separate, still-in-progress subsystem (see docs/BATTLE.md), so
//! day-4 daytime is out of scope for the day loop: day 4 08:30 is the furthest checkpoint
//! the day loop itself can guarantee, which is exactly the task minimum ("at least
//! day 1 -> day 4 08:30"), and is why day 5 is not reachable through the generic day
//! command.
//!
//! Facts asserted at every morning checkpoint:
//!   * day counter    var160 == N
//!   * weekday        var6   == N - 1   (day 1 = index 0, so day 2 = 1 = Tuesday, ...)
//!   * clock          var4 == 8 and var5 == 30
//!   * stats.warnings == 0
//!   * a drawn morning scene (screen.len() > 20), which also proves the scheduler never
//!     deadlocked: every wait loop here has a frame budget and panics if the story stops.
//!
//! The sister-click flag var63 is seeded exactly where the event-1 pointer branch writes
//! it on a real click (headless input cannot reproduce that hit-test), the same way
//! m6_mainline and m6_night_menu do it.

use std::path::PathBuf;

use wolf_game::Game;

fn resolve_root(raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return path;
    }
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    base.join(path)
}

/// Data root. The copied day-chain runs on `work/full` (m6_mainline's default), which is
/// also the root that carries `Save/System.sav` -- the template `Game::save_to` needs for
/// the mid-point save -> reload. `WOLF_DATA_ROOT` overrides; otherwise prefer a root with
/// that template and fall back to `work/all`.
fn data_root() -> PathBuf {
    if let Ok(raw) = std::env::var("WOLF_DATA_ROOT") {
        return resolve_root(&raw);
    }
    let full = resolve_root("work/full");
    let has_template = full.join("Save").join("System.sav").exists()
        || full.join("save").join("system.sav").exists();
    if has_template {
        return full;
    }
    resolve_root("work/all")
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

/// Wait for a fresh choice prompt (at least two frames old, so its option list is settled)
/// and return the options. Panics if none appears within `budget` frames.
fn wait_for_choice(game: &mut Game, budget: usize, what: &str) -> Vec<String> {
    for _ in 0..budget {
        game.frame();
        if let Some(choices) = &game.choices {
            if choices.frames >= 2 {
                return choices.options.clone();
            }
        }
    }
    panic!("timed out after {budget} frames waiting for {what}");
}

const SLEEP_OPTIONS: [&str; 3] = ["恶作剧", "轻轻把被子盖好", "今天还是睡吧"];
const CONFIRM_OPTIONS: [&str; 2] = ["是的", "再想想"];

/// From a 19:30 bedroom on day `day`, seed the sister-click flag and run the real night
/// menu -> CE329 -> next-morning chain. Leaves the game at day `day + 1`, 08:30.
fn sleep_into_next_morning(game: &mut Game, day: i32) {
    let expected_day = day + 1;
    game.auto_choice = false;

    // Seed the flag the event-1 pointer branch writes on a real sister click, then the
    // always-on メニュー処理 (event 4) sleep page opens the three-way night prompt.
    game.vm.vars.set(63, 1);
    let options = wait_for_choice(game, 400, "the three-way sleep prompt");
    assert_eq!(
        options,
        SLEEP_OPTIONS
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        "day {day}: the night prompt is 恶作剧 / 轻轻把被子盖好 / 今天还是睡吧"
    );

    // 今天还是睡吧 (index 2) -> "要睡到早上吗？" yes/no -> 是的 (index 0).
    game.set_choice_cursor(2);
    decide(game);
    let confirm = wait_for_choice(game, 200, "the sleep confirmation prompt");
    assert_eq!(
        confirm,
        CONFIRM_OPTIONS
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        "day {day}: the confirmation prompt is 是的 / 再想想"
    );
    game.set_choice_cursor(0);
    decide(game);

    // CE329 rewinds the clock, advances the day and plays the morning scene.
    let mut arrived = false;
    for _ in 0..800 {
        game.frame();
        if game.vm.vars.get(160) == expected_day
            && game.vm.vars.get(4) == 8
            && game.vm.vars.get(5) >= 30
        {
            arrived = true;
            break;
        }
    }
    assert!(
        arrived,
        "day {expected_day} 08:30 bedroom was reached (no deadlock in CE329)"
    );
    assert_eq!(
        game.stats.warnings, 0,
        "day {expected_day}: the sleep transition is warning-free"
    );
    assert_eq!(
        game.vm.vars.get(160),
        expected_day,
        "the day counter advanced to day {expected_day}"
    );
    assert_eq!(
        game.vm.vars.get(6),
        expected_day - 1,
        "weekday var6 is {expected_day}-1 for day {expected_day}"
    );
    assert_eq!(game.vm.vars.get(4), 8, "the morning starts at 8");
    assert_eq!(game.vm.vars.get(5), 30, "and 30 minutes");
    assert!(
        game.screen.len() > 20,
        "the day {expected_day} morning scene is drawn: {} pictures",
        game.screen.len()
    );
}

/// From an 08:30 morning on day `day`, drive the CE251 day-command menu: hover + press the
/// first picture button at 512,230, let whichever command it is spend the day to 17:00, then
/// play the evening scene out to the 19:30 bedroom. Leaves the game ready for
/// `sleep_into_next_morning`.
///
/// The first button's command rotates by day (day 2 = 学习会 / var48 2, day 3 = 攻击练习 /
/// var48 4), so this asserts the day-independent consequences (17:00, same day, energy
/// spent, a real string label) and, on day 2 only, the exact 学习会 effects that
/// m6_mainline.rs established (+8 study, -20 energy).
fn spend_day_to_evening(game: &mut Game, day: i32) {
    game.auto_choice = true;

    // CE251 draws its first day-command button into picture 1201 once the morning menu
    // opens. Wait for a non-empty label rather than a fixed frame count.
    let mut button_drawn = false;
    let mut first_label = String::new();
    for _ in 0..600 {
        game.frame();
        if let Some(picture) = game.screen.get(1201) {
            if !picture.text.is_empty() {
                first_label = picture.text.clone();
                button_drawn = true;
                break;
            }
        }
    }
    assert!(
        button_drawn,
        "day {day}: CE251 draws its first day-command button (no deadlock)"
    );
    assert!(
        !first_label.is_empty(),
        "day {day}: the day-command label is not empty"
    );
    assert_ne!(
        first_label, "\\f[20]0",
        "day {day}: the day-command label is a string, not the numeric self slot"
    );

    // Hover then press the first button, exactly like the m6_mainline loop: a bare one-frame
    // click only reports the hover. auto-choice answers the confirmation prompt.
    let energy_before = game.vm.vars.get(14);
    let study_before = game.vm.vars.get(171);
    game.mouse = [512, 230];
    for _ in 0..10 {
        game.frame();
    }
    game.mouse_buttons = 1;
    for _ in 0..5 {
        game.frame();
    }
    game.mouse_buttons = 0;
    let mut finished = false;
    for _ in 0..2000 {
        game.frame();
        if game
            .vm
            .instances()
            .iter()
            .all(|instance| instance.event != 251)
        {
            finished = true;
            break;
        }
    }
    assert!(
        finished,
        "day {day}: the CE251 day-command script finishes (no deadlock)"
    );
    assert_eq!(
        game.stats.warnings, 0,
        "day {day}: the day command is warning-free"
    );
    assert_eq!(
        game.vm.vars.get(4),
        17,
        "day {day}: the day command spends the day until 17:00"
    );
    assert_eq!(game.vm.vars.get(5), 0, "day {day}: on the hour");
    assert_eq!(
        game.vm.vars.get(160),
        day,
        "day {day}: the day command does not roll the day over"
    );
    assert!(
        game.vm.vars.get(48) >= 1,
        "day {day}: a real command was chosen (var48={})",
        game.vm.vars.get(48)
    );
    assert!(
        game.vm.vars.get(14) < energy_before,
        "day {day}: the day command spends energy ({} -> {})",
        energy_before,
        game.vm.vars.get(14)
    );
    if day == 2 {
        assert_eq!(
            game.vm.vars.get(48),
            2,
            "day 2: the first button is 学习会 (2)"
        );
        assert_eq!(
            game.vm.vars.get(14),
            energy_before - 20,
            "day 2: 学习会 costs 20 energy"
        );
        assert_eq!(
            game.vm.vars.get(171),
            study_before + 8,
            "day 2: 学习会 raises study by 8"
        );
    }

    // The day does not stop there: the evening scene (dinner, CE326) plays out and the
    // bedroom waits for the player again at 19:30.
    let mut evening = false;
    for _ in 0..1600 {
        game.frame();
        if game.vm.vars.get(4) == 19 && game.vm.vars.get(5) >= 30 {
            evening = true;
            break;
        }
    }
    assert!(
        evening,
        "day {day}: the evening scene reaches the 19:30 bedroom (no deadlock)"
    );
    assert_eq!(
        game.stats.warnings, 0,
        "day {day}: the evening scene is warning-free"
    );
    assert_eq!(
        game.vm.vars.get(160),
        day,
        "day {day}: still day {day} after the evening scene"
    );
    assert_eq!(
        game.vm.vars.get(4),
        19,
        "day {day}: the evening reaches 19:00"
    );
    assert_eq!(game.vm.vars.get(5), 30, "day {day}: and 19:30");
    assert!(
        game.screen.len() > 20,
        "day {day}: the evening bedroom is drawn: {} pictures",
        game.screen.len()
    );
}

/// Save the live game at this bedroom checkpoint, reload it into a fresh `Game`, and prove
/// the day state survived the round trip. `Game::new` reads the same `Save/System.sav`
/// template `save_to` needs; a bare load restores variables and pictures but not the
/// running script coroutines, so the multi-day loop itself continues on the live session.
fn save_reload_checkpoint(game: &mut Game, day: i32, weekday: i32) {
    let mut out = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    out.pop();
    out.pop();
    out.push("work/out/m6_multiday_midpoint.sav");
    let _ = std::fs::remove_file(&out);

    let vars_before: Vec<i32> = (0..800).map(|index| game.vm.vars.get(index)).collect();
    let playtime_before = game.playtime_frames;
    let pictures_before = game.screen.len();

    match game.save_to(&out) {
        Ok(bytes) => {
            assert!(bytes > 0, "the mid-point save wrote some bytes");
            let mut reloaded = Game::new(data_root()).expect("create a game to reload into");
            wolf_game::savegame::load_into(&mut reloaded, &out).expect("load the mid-point save");

            assert_eq!(
                reloaded.vm.vars.get(160),
                day,
                "reloaded day counter is day {day}"
            );
            assert_eq!(
                reloaded.vm.vars.get(6),
                weekday,
                "reloaded weekday is {weekday}"
            );
            assert_eq!(reloaded.vm.vars.get(4), 19, "reloaded clock hour is 19");
            assert_eq!(reloaded.vm.vars.get(5), 30, "reloaded clock minute is 30");
            assert_eq!(reloaded.stats.warnings, 0, "the reload is warning-free");
            assert_eq!(
                (0..800)
                    .map(|index| reloaded.vm.vars.get(index))
                    .collect::<Vec<_>>(),
                vars_before,
                "the 800 normal variables survive save -> reload"
            );
            assert_eq!(
                reloaded.playtime_frames, playtime_before,
                "play time survives save -> reload"
            );
            assert_eq!(
                reloaded.screen.len(),
                pictures_before,
                "the day-{day} 19:30 picture layer survives save -> reload"
            );

            let _ = std::fs::remove_file(&out);
        }
        Err(err) => {
            // No usable template save (e.g. a checkout whose data root lacks
            // Save/System.sav): skip just this leg, the multi-day loop still runs.
            eprintln!("skipping mid-point save -> reload: {err}");
        }
    }
}

#[test]
fn the_day_loop_repeats_through_day_four_with_a_midpoint_save_reload() {
    let Some(mut game) = open_game() else { return };
    game.auto_advance = true;
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;

    // Canonical new-game entry; with auto-choice the character-creation prompts are
    // answered and the story settles into the 18:30 bedroom on day 1.
    game.start_map_event_by_name("SampleMapA", 0)
        .expect("the new-game entry point exists");
    for _ in 0..1500 {
        game.frame();
    }
    assert_eq!(game.stats.warnings, 0, "the opening story is warning-free");
    assert_eq!(game.vm.vars.get(4), 18, "the bedroom clock is 18:00");
    assert_eq!(game.vm.vars.get(5), 30, "and 18:30");
    assert_eq!(game.vm.vars.get(160), 1, "on day 1");

    // Day 1 -> day 2: the first night sleep.
    sleep_into_next_morning(&mut game, 1);

    // Day 2: a full 学习会 day command, the evening scene, then sleep into day 3.
    spend_day_to_evening(&mut game, 2);
    sleep_into_next_morning(&mut game, 2);

    // Day 3: another full day command + evening scene, then -- at the 19:30 bedroom -- save
    // -> reload the live day state mid-way through the run, before continuing the loop.
    spend_day_to_evening(&mut game, 3);
    save_reload_checkpoint(&mut game, 3, 2);

    // Day 3 -> day 4: the third sleep. This is the furthest the day loop itself reaches;
    // day-4 daytime hands off to CE239 battle (out of scope, see the module docs).
    sleep_into_next_morning(&mut game, 3);

    // Final checkpoint: day 4, 08:30, warning-free, with a drawn morning scene. Reaching
    // here means every wait loop above finished inside its budget -- the scheduler never
    // deadlocked -- and the story really ran and re-ran for days.
    let instructions = game.stats.instructions;
    let frames = game.stats.frames;
    assert_eq!(
        game.stats.warnings, 0,
        "the whole multi-day loop is warning-free"
    );
    assert_eq!(game.vm.vars.get(160), 4, "the loop advances to day 4");
    assert_eq!(game.vm.vars.get(6), 3, "and to weekday index 3 (Thursday)");
    assert_eq!(game.vm.vars.get(4), 8, "the fourth morning starts at 8");
    assert_eq!(game.vm.vars.get(5), 30, "and 30 minutes");
    assert!(
        game.screen.len() > 20,
        "the day-4 morning scene is drawn: {} pictures",
        game.screen.len()
    );
    eprintln!("m6_multiday: reached day 4 08:30 in {frames} frames, {instructions} instructions");
    assert!(
        instructions > 200_000,
        "several days of real story ran: {instructions} instructions"
    );
    assert!(frames > 4_500, "the loop really ticked frames: {frames}");
}
