//! M12 day 6/7: the natural weekend schedule after day 5.
//!
//! The existing day-loop tests stop at day 5 08:30 (`m6_multiday`, `m7_day5_schedule`)
//! because day 4 is forced into the CE253 -> CE239 battle. M12 continues the same
//! played chain:
//!
//!   day 5 (Friday) 08:30 -> two real greetings -> CE251 workday row (秘密特训)
//!     -> 18:30 bedroom -> real CE329 sleep -> day 6 (Saturday) 09:30
//!   day 6 -> CE310 ■休日 menu: 外出 / 自主锻炼 / 处理积攒的家务 / 努力打工 / 悠然自得
//!     -> 自主锻炼 (09:30->12:00) -> 悠然自得 (12:00->15:00)
//!     -> 悠然自得 (15:00->17:00) -> real dinner -> 18:30 bedroom
//!     -> real CE329 sleep -> day 7 (Sunday) weekend menu at 09:30
//!
//! All of it is the shipped data: only the two-member battle fixture and the shared
//! `var63` sister-click flag used by every sleep test are seeded. No time, day,
//! energy or menu value is injected.
//!
//! The weekend menu positions are fixed pictures, not the 230+50*row formula the
//! weekday menu uses: picture 1201/1203/1209 sit at y = 200/250/400 and the day
//! buttons 1200/1202/1208 share those centres. The test clicks the text picture's own
//! coordinate, which is what the player sees.
//!
//! Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m12_day6_weekend -- --nocapture

use std::path::PathBuf;

use wolf_game::db::Container;
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

/// `work/full` carries the bundled Save/System.sav template; work/all is the large
/// extraction but lacks it. Honors WOLF_DATA_ROOT for CI.
fn data_root() -> PathBuf {
    if let Ok(raw) = std::env::var("WOLF_DATA_ROOT") {
        return resolve_root(&raw);
    }
    let full = resolve_root("work/full");
    if full.join("Save").join("System.sav").exists()
        || full.join("save").join("system.sav").exists()
    {
        full
    } else {
        resolve_root("work/all")
    }
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

fn wait_for_choice(game: &mut Game, budget: usize, what: &str) -> Vec<String> {
    for _ in 0..budget {
        game.frame();
        if let Some(choice) = &game.choices {
            if choice.frames >= 2 {
                return choice.options.clone();
            }
        }
    }
    panic!("timed out after {budget} frames waiting for {what}");
}

const SLEEP_OPTIONS: [&str; 3] = ["恶作剧", "轻轻把被子盖好", "今天还是睡吧"];
const CONFIRM_OPTIONS: [&str; 2] = ["是的", "再想想"];

/// Drive the real night menu -> CE329 -> CE317 chain and stop at the next morning
/// 08:30. Mirrors `m7_day5_schedule::sleep_into_next_morning`.
fn sleep_into_next_morning(game: &mut Game, day: i32) {
    let expected = day + 1;
    game.auto_choice = false;
    // var63 is the sister-click flag that opens the sleep prompt (same as m6).
    game.vm.vars.set(63, 1);
    let options = wait_for_choice(game, 500, "sleep menu");
    assert_eq!(
        options,
        SLEEP_OPTIONS.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
        "day {day}: unexpected sleep menu"
    );
    game.set_choice_cursor(2); // 今天还是睡吧
    decide(game);
    let confirm = wait_for_choice(game, 300, "sleep confirm");
    assert_eq!(
        confirm,
        CONFIRM_OPTIONS.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
        "day {day}: unexpected confirm menu"
    );
    game.set_choice_cursor(0); // 是的
    decide(game);
    let mut arrived = false;
    for _ in 0..900 {
        game.frame();
        if game.vm.vars.get(160) == expected
            && game.vm.vars.get(4) == 8
            && game.vm.vars.get(5) >= 30
        {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "day {expected} 08:30 not reached");
    assert_eq!(game.stats.warnings, 0, "day {expected}: warnings appeared");
}

/// Sleep from an earlier evening hour. The weekend days reach the bedroom at 18:30
/// (the 19:30 weekday chain is later), and the sleep prompt there needs a fresh
/// confirm edge: a message is dismissed first, then CE245 offers the three options.
/// This keeps the same real event chain and only feeds OK until the prompt appears.
fn sleep_with_confirm_edge(game: &mut Game, day: i32) -> (i32, i32, i32, i32) {
    game.auto_choice = false;
    game.vm.vars.set(63, 1);
    let mut options = Vec::new();
    for frame in 0..1600 {
        game.key_bits = if frame % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        if let Some(choice) = &game.choices {
            options = choice.options.clone();
            break;
        }
    }
    game.key_bits = 0;
    assert_eq!(
        options,
        SLEEP_OPTIONS.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
        "day {day}: unexpected sleep menu"
    );
    game.set_choice_cursor(2); // 今天还是睡吧
    decide(game);
    let mut confirm = Vec::new();
    for _ in 0..300 {
        game.frame();
        if let Some(choice) = &game.choices {
            confirm = choice.options.clone();
            break;
        }
    }
    assert_eq!(
        confirm,
        CONFIRM_OPTIONS.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
        "day {day}: unexpected confirm menu"
    );
    game.set_choice_cursor(0); // 是的
    decide(game);
    let mut arrived = false;
    for _ in 0..1600 {
        game.frame();
        if game.vm.vars.get(160) == day + 1
            && ((game.vm.vars.get(4) == 8 && game.vm.vars.get(5) >= 30)
                || (game.vm.vars.get(4) >= 9 && game.vm.vars.get(5) >= 30))
        {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "day {} morning not reached", day + 1);
    assert_eq!(game.stats.warnings, 0, "day {}: warnings appeared", day + 1);
    (
        game.vm.vars.get(160),
        game.vm.vars.get(6),
        game.vm.vars.get(4),
        game.vm.vars.get(5),
    )
}

/// After the day-4 battle settles: let CE251 finish (16:00 on the forced route), then
/// CE300 walks to 17:00 and runs the real CE326 dinner, which ends around 18:00.
fn finish_day_after_battle(game: &mut Game, day: i32) {
    let mut finished = false;
    for frame in 0..4000 {
        game.key_bits = if frame % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        if game.vm.instances().iter().all(|i| i.event != 251) {
            finished = true;
            break;
        }
    }
    game.key_bits = 0;
    assert!(finished, "day {day}: daytime command never returned after the battle");
    assert_eq!(game.stats.warnings, 0, "day {day}: warnings after the battle");
    assert_eq!(game.vm.vars.get(160), day);
    assert!(
        game.vm.vars.get(4) >= 16,
        "day {day}: expected the clock past 15:00, got {}:{:02}",
        game.vm.vars.get(4),
        game.vm.vars.get(5)
    );
    let mut dinner_done = false;
    for frame in 0..6000 {
        game.key_bits = if frame % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        let alive300 = game.vm.instances().iter().any(|i| i.event == 300);
        let alive326 = game.vm.instances().iter().any(|i| i.event == 326);
        if !alive300 && !alive326 && game.vm.vars.get(4) >= 18 {
            dinner_done = true;
            break;
        }
    }
    game.key_bits = 0;
    assert!(
        dinner_done,
        "day {day}: dinner never finished (clock {}:{:02})",
        game.vm.vars.get(4),
        game.vm.vars.get(5)
    );
    assert_eq!(game.stats.warnings, 0, "day {day}: warnings in the evening");
}

/// Click the first workday command button (row 0 鍛錬) and let the day run through
/// 17:00 to the 18:30 bedroom. Days 2 and 3, plus day 5.
const DAY_COMMAND_LABELS: [&str; 6] = [
    "给店里帮忙", "学习会", "基础锻炼", "攻击练习", "防御练习", "秘密特训",
];
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

fn spend_day_choosing_row(game: &mut Game, day: i32) {
    game.auto_choice = true;
    // The three drawn day commands are a retail-PRNG roll; 地区贡献 routes into the
    // battle. Pick the first row that is a CE252 daytime task, like a player would.
    let mut chosen: Option<(usize, String)> = None;
    for _ in 0..1200 {
        game.frame();
        for (row, text) in day_row_texts(game).iter().enumerate() {
            if DAY_COMMAND_LABELS.iter().any(|label| text.contains(label)) {
                chosen = Some((row, text.clone()));
                break;
            }
        }
        if chosen.is_some() {
            break;
        }
    }
    let (row, label) = chosen.expect("day: CE251 draws a CE252 daytime command");
    let energy_before = game.vm.vars.get(14);
    let y = 230 + 50 * row as i32;
    game.mouse = [512, y];
    for _ in 0..10 {
        game.frame();
    }
    game.mouse_buttons = 1;
    for _ in 0..5 {
        game.frame();
    }
    game.mouse_buttons = 0;
    let mut finished = false;
    for _ in 0..2500 {
        game.frame();
        if game.vm.instances().iter().all(|i| i.event != 251) {
            finished = true;
            break;
        }
    }
    assert!(finished, "day {day}: daytime command never returned ({label})");
    assert_eq!(game.stats.warnings, 0, "day {day}: warnings during day");
    assert_eq!(game.vm.vars.get(4), 17, "day {day}: expected 17:00");
    assert_eq!(game.vm.vars.get(5), 0, "day {day}: expected 17:00");
    assert_eq!(game.vm.vars.get(160), day);
    assert!(game.vm.vars.get(14) < energy_before, "day {day}: energy not spent");
    let mut evening = false;
    for _ in 0..6000 {
        game.frame();
        if !evening_work(game) && game.vm.vars.get(4) >= 18 {
            evening = true;
            break;
        }
    }
    assert!(
        evening,
        "day {day}: evening not settled (clock {}:{:02})",
        game.vm.vars.get(4),
        game.vm.vars.get(5)
    );
    assert_eq!(game.vm.vars.get(160), day);
    assert_eq!(game.vm.vars.get(5), 30, "day {day}: evening settles on the half hour");
    assert_eq!(game.stats.warnings, 0, "day {day}: warnings in evening");
}

fn has_event(game: &Game, event: u32) -> bool {
    game.vm.instances().iter().any(|i| i.event == event)
}

/// Click the text picture whose label matches `label`. The weekend buttons are fixed
/// pictures (1201/1203/1209 at y = 200/250/400), so the picture's own x/y is the point
/// the player sees and the point 124 hit-tests.
fn click_label_row(game: &mut Game, label: &str) -> (i32, i32) {
    for _ in 0..1200 {
        game.frame();
        for &number in &DAY_ROW_PICS {
            if let Some(picture) = game.screen.get(number) {
                if picture.text.contains(label) {
                    let (x, y) = (picture.x, picture.y);
                    game.mouse = [x, y];
                    for _ in 0..10 {
                        game.frame();
                    }
                    game.mouse_buttons = 1;
                    for _ in 0..5 {
                        game.frame();
                    }
                    game.mouse_buttons = 0;
                    game.mouse = [-1, -1];
                    return (x, y);
                }
            }
        }
    }
    panic!("weekend menu never drew {label}");
}

/// Feed OK until the weekend menu (CE310) is drawn with its five/fewer buttons.
fn wait_for_weekend_menu(game: &mut Game, budget: usize) -> Vec<String> {
    for frame in 0..budget {
        game.key_bits = if frame % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        let rows = day_row_texts(game);
        if rows.iter().any(|text| text.contains("悠然自得")) && has_event(game, 310) {
            game.key_bits = 0;
            return rows;
        }
    }
    game.key_bits = 0;
    panic!("weekend menu (CE310) never appeared");
}

#[test]
fn day_five_work_rolls_into_the_weekend_and_day_seven_menu() {
    let Some(mut game) = open_game() else {
        return;
    };

    // ---- days 1..4 08:30 on the real schedule --------------------------------
    game.auto_advance = true;
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.start_map_event_by_name("SampleMapA", 0).expect("entry event");
    for _ in 0..1600 {
        game.frame();
    }
    assert_eq!(game.stats.warnings, 0);
    assert_eq!(game.vm.vars.get(4), 18, "day1 should start at 18:30");
    assert_eq!(game.vm.vars.get(5), 30);
    assert_eq!(game.vm.vars.get(160), 1, "day1 counter");

    sleep_into_next_morning(&mut game, 1); // day2 08:30, weekday 1 (Tue)
    spend_day_choosing_row(&mut game, 2);
    sleep_into_next_morning(&mut game, 2); // day3 08:30, weekday 2 (Wed)
    spend_day_choosing_row(&mut game, 3);
    sleep_into_next_morning(&mut game, 3); // day4 08:30, weekday 3 (Thu)

    assert_eq!(game.vm.vars.get(160), 4, "reached day 4");
    assert_eq!(game.vm.vars.get(6), 3, "day4 weekday = Thursday");
    assert_eq!(game.vm.vars.get(4), 8, "day4 08:30");
    assert_eq!(game.vm.vars.get(5), 30, "day4 08:30");

    // ---- day 4 forced battle, settled by the auto-armed drive ----------------
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(game.seed_battle_databases() > 0, "battle DB seed must land");
    game.databases
        .set_text(Container::CData, 5, 16, 0, "┗ 状態リスト");
    let _ = game.start_engine_events();

    let mut settled = false;
    for frame in 0..4000 {
        game.key_bits = if frame % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        if game.vm.vars.get(343) == 1 && game.vm.vars.get(350) == 0 {
            settled = true;
            break;
        }
    }
    assert!(settled, "the day-4 fight must settle without injection");
    for frame in 0..2000 {
        game.key_bits = if frame % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        if !has_event(&game, 239) {
            break;
        }
    }
    game.key_bits = 0;
    assert!(!has_event(&game, 239), "CE239 must return after settlement");
    assert_eq!(game.stats.warnings, 0, "no warnings through the day-4 battle");
    finish_day_after_battle(&mut game, 4);
    sleep_into_next_morning(&mut game, 4); // day5 08:30, weekday 4 (Fri)

    assert_eq!(game.vm.vars.get(160), 5, "reached day 5");
    assert_eq!(game.vm.vars.get(6), 4, "day5 weekday = Friday");
    assert_eq!(game.vm.vars.get(4), 8, "day5 08:30");
    assert_eq!(game.vm.vars.get(5), 30, "day5 08:30");

    // ---- day 5: two real greetings, then the workday row --------------------
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let mut greetings = 0usize;
    for frame in 0..2500 {
        game.key_bits = if frame % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        if let Some(choice) = &game.choices {
            if choice.frames == 1 {
                greetings += 1;
                assert_eq!(
                    choice.options.len(),
                    1,
                    "day5 greeting #{greetings} is a single-line story prompt: {:?}",
                    choice.options
                );
            }
        }
        if day_row_texts(&game)
            .iter()
            .any(|text| DAY_COMMAND_LABELS.iter().any(|label| text.contains(label)))
        {
            break;
        }
    }
    game.key_bits = 0;
    assert_eq!(greetings, 2, "day5 opens with the two 早上好 greetings");
    assert_eq!(game.vm.vars.get(4), 8, "greetings leave the clock at 08:30");

    spend_day_choosing_row(&mut game, 5);
    assert_eq!(game.vm.vars.get(160), 5, "day5 work keeps the day counter");
    assert_eq!(game.vm.vars.get(4), 18, "day5 work ends at 18:30");
    assert_eq!(game.vm.vars.get(5), 30, "day5 work ends at 18:30");

    // ---- day 6 (Saturday): CE310 weekend menu, three slots, dinner, sleep ----
    let (day, weekday, hour, minute) = sleep_with_confirm_edge(&mut game, 5);
    assert_eq!((day, weekday, hour, minute), (6, 5, 9, 30), "day 6 is Saturday 09:30");
    assert!(!has_event(&game, 251), "the weekday command menu must not run on Saturday");

    let weekend = wait_for_weekend_menu(&mut game, 2000);
    assert_eq!(
        weekend,
        [
            "\\f[20]外出",
            "\\f[20]自主锻炼",
            "\\f[20]处理积攒的家务",
            "\\f[20]努力打工",
            "\\f[20]悠然自得",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect::<Vec<_>>(),
        "day 6 draws the five weekend commands"
    );
    for (number, y) in [(1201, 200), (1203, 250), (1209, 400)] {
        let picture = game.screen.get(number).expect("weekend text picture");
        assert_eq!(picture.y, y, "weekend picture {number} keeps its retail y");
    }

    let energy_start = game.vm.vars.get(14);
    eprintln!("day6 weekend start energy={energy_start}");

    // First slot: training 09:30 -> 12:00, energy -15.
    let (x, y) = click_label_row(&mut game, "自主锻炼");
    eprintln!("weekend training clicked at ({x},{y})");
    let mut trained = false;
    for frame in 0..3000 {
        game.key_bits = if frame % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        if game.vm.vars.get(4) == 12 && game.vm.vars.get(5) == 0 && has_event(&game, 310) {
            trained = true;
            break;
        }
    }
    game.key_bits = 0;
    assert!(trained, "自主锻炼 must return to the weekend menu at 12:00");
    let after_training = game.vm.vars.get(14);
    assert!(
        after_training < energy_start,
        "自主锻炼 must spend energy ({energy_start} -> {after_training})"
    );
    assert_eq!(game.vm.vars.get(160), 6, "still day 6 after the first slot");

    // Second slot: 悠然自得 12:00 -> 15:00, energy +10.
    let (x, y) = click_label_row(&mut game, "悠然自得");
    eprintln!("weekend relax 1 clicked at ({x},{y})");
    let mut relaxed = false;
    for frame in 0..3000 {
        game.key_bits = if frame % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        if game.vm.vars.get(4) == 15 && game.vm.vars.get(5) == 0 && has_event(&game, 310) {
            relaxed = true;
            break;
        }
    }
    game.key_bits = 0;
    assert!(relaxed, "悠然自得 must return to the weekend menu at 15:00");
    let after_relax = game.vm.vars.get(14);
    eprintln!(
        "weekend energy: start={energy_start} after_training={after_training} after_relax={after_relax}"
    );

    // Third slot: 悠然自得 15:00 -> 17:00, CE310 ends, energy back to the start.
    let (x, y) = click_label_row(&mut game, "悠然自得");
    eprintln!("weekend relax 2 clicked at ({x},{y})");
    let mut ended = false;
    for frame in 0..3000 {
        game.key_bits = if frame % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        if game.vm.vars.get(4) == 17 && !has_event(&game, 310) {
            ended = true;
            break;
        }
    }
    game.key_bits = 0;
    assert!(ended, "the third weekend slot must end CE310 at 17:00");
    let final_energy = game.vm.vars.get(14);
    eprintln!("weekend energy after the third slot={final_energy}");

    // ---- real dinner and the 18:30 bedroom, then day 7 (Sunday) ---------------
    // The dinner is a scripted sequence with its own prompts; answer them like the
    // day-loop helper does, then let the clock reach the bedroom.
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let mut evening = false;
    for _ in 0..8000 {
        game.frame();
        if !evening_work(&game) && game.vm.vars.get(4) >= 18 {
            evening = true;
            break;
        }
    }
    assert!(
        evening,
        "day6 evening not settled (clock {}:{:02})",
        game.vm.vars.get(4),
        game.vm.vars.get(5)
    );
    assert_eq!(game.vm.vars.get(5), 30, "day6 evening settles on the half hour");

    let (day, weekday, hour, minute) = sleep_with_confirm_edge(&mut game, 6);
    assert_eq!((day, weekday, hour, minute), (7, 6, 9, 30), "day 7 is Sunday 09:30");
    let sunday = wait_for_weekend_menu(&mut game, 2000);
    assert_eq!(
        sunday,
        weekend,
        "Sunday draws the same five weekend commands after the 09:30 morning scene"
    );
    assert_eq!(game.vm.vars.get(4), 9, "Sunday menu is on screen at 09:30");
    assert_eq!(game.vm.vars.get(5), 30, "Sunday menu stays at 09:30");
    assert_eq!(game.vm.vars.get(160), 7, "day counter reached 7");
    assert_eq!(game.vm.vars.get(6), 6, "day7 weekday = Sunday");
    assert_eq!(game.stats.warnings, 0, "the whole day5 -> day7 run is warning-free");

    eprintln!(
        "m12_day6_weekend: day5 08:30 -> day6 Sat 09:30 -> day7 Sun 09:30 OK; \
         weekend rows={weekend:?}; energy={energy_start}->{after_training}->{after_relax}->{final_energy}; warnings=0; frames={} instructions={}",
        game.stats.frames, game.stats.instructions
    );
}