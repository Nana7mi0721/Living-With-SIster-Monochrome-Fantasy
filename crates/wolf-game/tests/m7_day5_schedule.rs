//! W3 day5 schedule: reach day 5 08:30 from a faithful played day-4 state.
//!
//! Real schedule path exercised here:
//!   new game -> day1 18:30 -> (real CE329 sleep) -> day2 -> ... -> day4 08:30
//!   day4 daytime is FORCED by the retail data into CE253 地域貢献 -> CE239 battle
//!   (the day-4 workday menu never draws a selectable non-battle row; see
//!    docs/research/M7_DAY5_SCHEDULE.md and m6_week_loop.rs).
//!
//! Since M11 task-95 the rebuilt battle wiring is auto-armed on the shipped CE253
//! dispatch, so the forced day-4 fight settles naturally: no settlement variable is
//! injected any more (the old circular 2000350 bootstrap is gone).  The battle is
//! followed by the real evening dinner chain (CE300/CE326), the real CE329 就寝
//! (day+1, weekday+1), CE317 早朝 (08:30) and the day/weekday counters.
//!
//! The only seed is var63=1 in the shared sleep helper: var63 is the "sister click"
//! flag that opens the night sleep prompt.  The post-battle evening is shorter than
//! the normal menu route, so the bedroom is reached at 18:00 rather than 19:30.

use std::path::PathBuf;
use wolf_game::db::Container;
use wolf_game::Game;

fn resolve_root(raw: &str) -> PathBuf {
    let p = PathBuf::from(raw);
    if p.is_absolute() { return p; }
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    base.join(p)
}

/// work/full carries the bundled Save/System.sav template; work/all is the large
/// extraction but lacks it.  Honors WOLF_DATA_ROOT for CI.
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
        Ok(g) => Some(g),
        Err(e) => { eprintln!("skip m7_day5_schedule: {e}"); None }
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
/// 08:30.  Mirrors m6_mainline::the_bedroom_sleep_menu_choice_rolls_over_to_a_new_day.
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

/// After the day-4 battle settles: let CE251 finish (16:00 on the forced route), then
/// CE300 walks to 17:00 and runs the real CE326 dinner, which ends around 18:00.
fn finish_day_after_battle(game: &mut Game, day: i32) {
    let mut finished = false;
    for f in 0..4000 {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
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
    for f in 0..6000 {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
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

/// Click the first workday command button (row0 鍛錬) and let the day run through
/// 17:00 to the 19:30 bedroom.  Days 2 and 3 only (day 4 is the forced battle).

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
    for _ in 0..10 { game.frame(); }
    game.mouse_buttons = 1;
    for _ in 0..5 { game.frame(); }
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
        if !evening_work(&game) && game.vm.vars.get(4) >= 18 {
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

#[test]
fn day_five_morning_is_reachable_after_settling_the_forced_day4_battle() {
    let Some(mut game) = open_game() else { return };

    // ---- days 1..4 08:30 on the real schedule --------------------------------
    game.auto_advance = true;
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.start_map_event_by_name("SampleMapA", 0).expect("entry event");
    for _ in 0..1600 { game.frame(); }
    assert_eq!(game.stats.warnings, 0);
    assert_eq!(game.vm.vars.get(4), 18, "day1 should start at 18:30");
    assert_eq!(game.vm.vars.get(5), 30);
    assert_eq!(game.vm.vars.get(160), 1, "day1 counter");

    sleep_into_next_morning(&mut game, 1); // day2 08:30, weekday 1 (Tue)
    assert_eq!(game.vm.vars.get(6), 1);
    spend_day_choosing_row(&mut game, 2);
    sleep_into_next_morning(&mut game, 2); // day3 08:30, weekday 2 (Wed)
    assert_eq!(game.vm.vars.get(6), 2);
    spend_day_choosing_row(&mut game, 3);
    sleep_into_next_morning(&mut game, 3); // day4 08:30, weekday 3 (Thu)

    assert_eq!(game.stats.warnings, 0);
    assert_eq!(game.vm.vars.get(160), 4, "reached day 4");
    assert_eq!(game.vm.vars.get(6), 3, "day4 weekday = Thursday");
    assert_eq!(game.vm.vars.get(4), 8, "day4 08:30");
    assert_eq!(game.vm.vars.get(5), 30, "day4 08:30");
    assert!(game.screen.len() > 20, "day4 screen should be populated");

    // ---- day 4 daytime: forced CE253 -> CE239, settled by the auto-armed drive ----
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;

    // Real battle database seed (party stats / command slots / item stock) and the one
    // cosmetic command-menu section-header label a fresh save leaves blank.  No
    // settlement variable is seeded.
    let seeded = game.seed_battle_databases();
    assert!(seeded > 0, "seed_battle_databases must seed the battle cells");
    game.databases
        .set_text(Container::CData, 5, 16, 0, "┗ 状態リスト");
    let _ = game.start_engine_events();

    let mut saw_253 = false;
    let mut saw_239 = false;
    let mut saw_242 = false;
    let mut saw_training_menu = false;
    let mut settled = false;
    let mut v352_peak: i32 = 0;
    let mut v343_peak: i32 = 0;
    for f in 0..4000 {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        if has_event(&game, 253) { saw_253 = true; }
        if has_event(&game, 239) { saw_239 = true; }
        if has_event(&game, 242) { saw_242 = true; }
        if let Some(choice) = &game.choices {
            if choice.options.len() == 5 { saw_training_menu = true; }
        }
        let v352 = game.vm.vars.get(352);
        if v352.abs() > v352_peak.abs() { v352_peak = v352; }
        v343_peak = v343_peak.max(game.vm.vars.get(343));
        if game.vm.vars.get(343) == 1 && game.vm.vars.get(350) == 0 {
            settled = true;
            break;
        }
    }
    // Let CE239 drain its victory sequence before returning to CE253.
    if settled {
        for f in 0..2000 {
            game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
            game.frame();
            if !has_event(&game, 239) { break; }
        }
    }
    game.key_bits = 0;
    assert!(saw_253, "day4 should dispatch CE253 地域貢献");
    assert!(saw_239, "CE253 should call the real battle event CE239");
    assert!(saw_242, "the auto-armed drive must start the CE242 command phase");
    assert!(!saw_training_menu, "day4 must not draw the 5-row workday menu");
    assert!(settled, "the day-4 fight must settle without injection");
    assert!(
        v352_peak == 1 || v352_peak == -1,
        "CE244 must settle 2000352 in {{1,-1}}, got {v352_peak}"
    );
    assert_eq!(v343_peak, 1, "CE244 must set 2000343 = 1");
    assert!(!has_event(&game, 239), "CE239 must return after settlement");
    assert_eq!(game.stats.warnings, 0, "no warnings through the day-4 battle");

    // ---- real evening chain after the battle returns -------------------------
    finish_day_after_battle(&mut game, 4);

    // ---- real CE329 sleep from the evening bedroom -> day5 08:30 --------------
    // sleep_into_next_morning seeds var63 (the sister click) and drives the genuine
    // CE329 就寝 + CE317 早朝 chain; the script-random settle hour (18:30 or 19:30)
    // does not matter because CE329/CE317 rewrite date and time absolutely.
    sleep_into_next_morning(&mut game, 4); // day5 08:30, weekday 4 (Fri)

    assert_eq!(game.stats.warnings, 0, "no warnings on day5");
    assert_eq!(game.vm.vars.get(160), 5, "reached day 5");
    assert_eq!(game.vm.vars.get(6), 4, "day5 weekday = Friday");
    assert_eq!(game.vm.vars.get(4), 8, "day5 08:30");
    assert_eq!(game.vm.vars.get(5), 30, "day5 08:30");
    assert!(game.screen.len() > 20, "day5 screen should be populated");
    assert!(
        game.vm.instances().iter().any(|i| i.event == 317 || i.event == 329 || i.event == 4),
        "morning schedule instances should be live on day5"
    );

    eprintln!(
        "m7_day5_schedule: day1->day5 08:30 OK; var160=5 var6=4 (Fri) var4=8 var5=30; \
         warnings=0; frames={} instructions={}",
        game.stats.frames, game.stats.instructions
    );
}
