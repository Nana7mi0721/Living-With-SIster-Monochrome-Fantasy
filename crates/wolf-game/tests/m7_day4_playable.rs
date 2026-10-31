//! M7 task-38 / M8 W1-c: day-4 real battle settlement -> evening -> day 5 08:30 with
//! NO settlement injection.
//!
//! This test replays the genuine schedule from a new game to day 4 08:30 (same helper
//! pattern as m6_multiday / m7_day5_schedule), lets the retail data force day 4 daytime
//! into CE253 地域貢献 -> CE239, lets the rebuilt wiring fire, and then plays the real
//! evening/night menu through CE329 就寝処理 to day 5.
//!
//! WHAT CHANGED (M8 W1-c): the earlier task-38 run proved the round deadlocked because
//! the wiring forwarded 2000354 (the enemy/battle type the caller selected) as CE242's
//! arg0. CE242's arg0 (`self1600000`) is the **acting character slot**: pc143 takes the
//! main-party action block for 1..3 and the `DLC用追加処理` branch otherwise. With the
//! leader slot 1 the same from-scratch day-4 replay now settles: CE244 writes
//! 2000352 = 1 and 2000343 = 1, CE239 returns, CE251 finishes at 17:00, the dinner and
//! the natural night menu run, and day 5 08:30 is reached. See
//! crate::battle_wiring (src/battle_wiring.rs) and docs/research/M8_BATTLE_SETTLE.md.
//!
//! Party power 2000028 is still only 132 at day 4 (a from-scratch weak party), which is
//! why CE253's low-power branch passes command 0 in 2000354; that value is a valid
//! battle context (it selects the enemy row block in CE242 pc91..pc129), not the
//! acting-slot argument.
//!
//! NO TEST-ONLY INJECTION of 2000350/2000352/2000343 (or any settlement state) is
//! performed. The only data seeding is the task-19 battle database seed (party stats /
//! command slots / item stock) plus one cosmetic command-menu section-header label.
//! The night menu is opened with a real hover-sister + Decide gesture; only the
//! first-dinner TIPS flag var295 (if still set) and the ev4 gate var68 are normalized,
//! exactly as m6_night_sleep does.
//!
//! Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_day4_playable -- --nocapture

use std::path::PathBuf;

use wolf_game::Game;

fn resolve_root(raw: &str) -> PathBuf {
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        return p;
    }
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    base.join(p)
}

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
        Err(e) => {
            eprintln!("skip m7_day4_playable: {e}");
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

fn has_event(game: &Game, event: u32) -> bool {
    game.vm.instances().iter().any(|i| i.event == event)
}

fn event_pc(game: &Game, event: u32) -> Option<usize> {
    game.vm.instances().iter().find(|i| i.event == event).map(|i| i.pc)
}

/// Drive the real night menu -> CE329 就寝 -> CE317 早朝 chain and stop at the next morning
/// 08:30. Mirrors m6_multiday::sleep_into_next_morning.
fn sleep_into_next_morning(game: &mut Game, day: i32) {
    let expected = day + 1;
    game.auto_choice = false;
    game.vm.vars.set(63, 1); // sister-click flag (same seed the real pointer branch writes)
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
    assert_eq!(game.vm.vars.get(160), expected, "day counter advanced");
    assert_eq!(game.vm.vars.get(6), expected - 1, "weekday advanced");
    assert_eq!(game.vm.vars.get(4), 8, "morning hour");
    assert_eq!(game.vm.vars.get(5), 30, "morning minute");
    assert!(game.screen.len() > 20, "day {expected}: morning scene drawn");
}

/// After the day-4 battle settles: let CE253/CE251 finish (16:00 on the forced route),
/// then CE300 walks the clock to 17:00 and runs the CE326 dinner. Wait for that dinner
/// to finish (no CE300/CE326 alive) and return with the bedroom ready.
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
    // The forced 地域貢献 route froze the clock at 15:00; CE251 resumes at 16:00 (the
    // normal workday menu ends at 17:00). Assert the honest post-battle anchor only.
    assert_eq!(game.vm.vars.get(160), day);
    assert!(
        game.vm.vars.get(4) >= 16,
        "day {day}: expected the clock to move past 15:00, got {}:{:02}",
        game.vm.vars.get(4),
        game.vm.vars.get(5)
    );
    // CE300 advances to 17:00 and runs the real CE326 dinner, which ends around 18:00.
    // Wait for that dinner to finish instead of pinning 19:30 (the forced battle shifts
    // the whole evening one hour earlier than the normal menu route).
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

/// Open the night menu with a real hover-sister + Decide gesture and sleep through the
/// 今天先休息吧 -> 是 confirm (the m6_night_sleep recipe). No var63/64/68 injection drives
/// the click chain; only the first-dinner TIPS flag var295 is normalized if still set.
fn sleep_naturally_into_next_morning(game: &mut Game, day: i32) {
    let expected = day + 1;
    game.auto_choice = false;
    if game.vm.vars.get(295) == 1 {
        // An undismissed first-dinner skill TIPS flag; ev4 page0 pc1 breaks on it.
        game.vm.vars.set(295, 0);
    }
    game.vm.vars.set(68, 0);
    let sister = game
        .screen
        .get(120)
        .expect("the sister sprite is picture 120");
    game.mouse = [sister.x, sister.y];
    for _ in 0..8 {
        game.frame();
    }
    decide(game);
    for _ in 0..40 {
        game.frame();
    }
    assert!(has_event(game, 245), "day {day}: the night command menu is open");

    click_at(game, 512, 450); // 今天先休息吧
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
        "day {day}: the rest command raises the confirm prompt"
    );
    game.set_choice_cursor(0); // 是
    decide(game);

    let mut arrived = false;
    for _ in 0..2400 {
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
    assert_eq!(game.vm.vars.get(160), expected, "day counter advanced");
    assert_eq!(game.vm.vars.get(6), expected - 1, "weekday advanced");
    assert_eq!(game.vm.vars.get(4), 8, "morning hour");
    assert_eq!(game.vm.vars.get(5), 30, "morning minute");
    assert!(game.screen.len() > 20, "day {expected}: morning scene drawn");
}

/// Click the first workday command button (row0) and let the day run to 17:00 then the
/// evening scene to the 19:30 bedroom. Days 2 and 3 only (day 4 is the forced battle).

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

/// True while the day-command / evening machinery still has work in flight.
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

fn evname<E: std::fmt::Debug>(e: &E) -> String {
    format!("{:?}", e)
}

#[derive(Default)]
struct BattleTrace {
    ce253_started: bool,
    ce239_started: bool,
    ce242_started: bool,
    ce243_started: bool,
    ce241_started: bool,
    wiring_rounds: u64,
    battle_active_frame: Option<usize>,
    day4_block_frames: usize,
    v350_seq: Vec<i32>,
    v354_max: i32,
    v352_peak: i32,
    v343_peak: i32,
    max_ce239_pc: i32,
    max_ce242_pc: i32,
    ce239_stuck_pc: Option<usize>,
    ce239_alive_at_end: bool,
    warnings: u64,
    unanswered_250: usize,
}

#[test]
fn day4_real_battle_settles_and_reaches_day5_without_injection() {
    let Some(mut game) = open_game() else { return };

    // ---- days 1..4 08:30 on the genuine schedule ----------------------------
    game.auto_advance = true;
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.start_map_event_by_name("SampleMapA", 0).expect("entry event");
    for _ in 0..1600 {
        game.frame();
    }
    assert_eq!(game.stats.warnings, 0);
    assert_eq!(game.vm.vars.get(4), 18, "day1 starts 18:30");
    assert_eq!(game.vm.vars.get(5), 30);
    assert_eq!(game.vm.vars.get(160), 1, "day1 counter");

    sleep_into_next_morning(&mut game, 1); // day2 08:30, weekday 1
    assert_eq!(game.vm.vars.get(6), 1);
    spend_day_choosing_row(&mut game, 2);
    sleep_into_next_morning(&mut game, 2); // day3 08:30, weekday 2
    assert_eq!(game.vm.vars.get(6), 2);
    spend_day_choosing_row(&mut game, 3);
    sleep_into_next_morning(&mut game, 3); // day4 08:30, weekday 3

    assert_eq!(game.stats.warnings, 0);
    assert_eq!(game.vm.vars.get(160), 4, "reached day 4");
    assert_eq!(game.vm.vars.get(6), 3, "day4 weekday = Thursday");
    assert_eq!(game.vm.vars.get(4), 8, "day4 08:30");
    assert_eq!(game.vm.vars.get(5), 30, "day4 08:30");
    assert!(game.screen.len() > 20, "day4 screen should be populated");

    // ---- day 4 daytime: real battle via the rebuilt wiring, NO injection ----
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;

    // Task-19 battle databases (party stats / command slots / item stock) + the one
    // cosmetic command-menu section-header label that a fresh SaveData leaves blank.
    // These populate CData cells; none is a settlement variable.
    let seeded = game.seed_battle_databases();
    assert!(seeded > 0, "seed_battle_databases must seed the battle cells");
    game.databases
        .set_text(wolf_game::db::Container::CData, 5, 16, 0, "┗ 状態リスト");

    let pre_power = game.vm.vars.get(28); // 2000028 party battle power
    // Engine coroutines: CE241 (turn start) + the run==2 scheduler that starts CE240
    // (戦力処理) once 2000340 becomes active.
    let _ = game.start_engine_events();
    // THE WIRING UNDER TEST: on the battle-active edge start CE242 command phase.
    game.set_battle_wiring(true);

    // NO-INJECTION GUARD: the settlement variables must be pristine before the battle.
    assert_eq!(game.vm.vars.get(340), 0, "pre-battle 2000340 must be idle (no injection)");
    assert_eq!(game.vm.vars.get(350), 0, "pre-battle 2000350 must be idle (no injection)");
    assert_eq!(game.vm.vars.get(352), 0, "pre-battle 2000352 must be 0 (no injection)");
    assert_eq!(game.vm.vars.get(343), 0, "pre-battle 2000343 must be 0 (no injection)");

    let mut t = BattleTrace::default();
    let mut last350 = i32::MIN;
    let mut settled = false;
    for f in 0..1600 {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        t.day4_block_frames = f + 1;
        let v340 = game.vm.vars.get(340);
        let v350 = game.vm.vars.get(350);
        let v352 = game.vm.vars.get(352);
        let v343 = game.vm.vars.get(343);
        let v354 = game.vm.vars.get(354);
        t.v354_max = t.v354_max.max(v354);
        if v340 != 0 && t.battle_active_frame.is_none() {
            t.battle_active_frame = Some(f);
        }
        if v350 != last350 {
            t.v350_seq.push(v350);
            last350 = v350;
        }
        if v352.abs() > t.v352_peak.abs() {
            t.v352_peak = v352;
        }
        t.v343_peak = t.v343_peak.max(v343);
        t.wiring_rounds = game.battle_wiring_rounds();
        for inst in game.vm.instances() {
            match evname(&inst.event).as_str() {
                "253" => t.ce253_started = true,
                "239" => {
                    t.ce239_started = true;
                    let pc = inst.pc as i32;
                    if pc > t.max_ce239_pc {
                        t.max_ce239_pc = pc;
                    }
                }
                "242" => {
                    t.ce242_started = true;
                    let pc = inst.pc as i32;
                    if pc > t.max_ce242_pc {
                        t.max_ce242_pc = pc;
                    }
                }
                "243" => t.ce243_started = true,
                "241" => t.ce241_started = true,
                _ => {}
            }
        }
        if v343 == 1 && v350 == 0 {
            settled = true;
            break;
        }
    }
    // Let CE239 drain its victory sequence: it plays messages and waits on InputKey
    // before returning to CE253. Without this drain the coroutine is still parked at
    // pc745 when the settlement variables first read 1/0.
    if settled {
        for f in 0..1600 {
            game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
            game.frame();
            t.day4_block_frames += 1;
            let v352 = game.vm.vars.get(352);
            if v352.abs() > t.v352_peak.abs() {
                t.v352_peak = v352;
            }
            t.v343_peak = t.v343_peak.max(game.vm.vars.get(343));
            for inst in game.vm.instances() {
                if evname(&inst.event) == "242" {
                    let pc = inst.pc as i32;
                    if pc > t.max_ce242_pc {
                        t.max_ce242_pc = pc;
                    }
                }
            }
            if !has_event(&game, 239) {
                break;
            }
        }
        game.key_bits = 0;
    }
    t.ce239_alive_at_end = has_event(&game, 239);
    t.ce239_stuck_pc = event_pc(&game, 239);
    t.warnings = game.stats.warnings;
    let unanswered_msgs: Vec<String> = game
        .diagnostics
        .iter()
        .filter(|(msg, _)| msg.contains("unanswered"))
        .map(|(msg, n)| format!("x{n}: {msg}"))
        .collect();
    t.unanswered_250 = unanswered_msgs.len();
    for m in &unanswered_msgs {
        println!("UNANSWERED250: {m}");
    }

    println!(
        "\nDAY4 BATTLE RESULT: ce253={} ce239={} ce242={} ce243={} ce241={} rounds={} \
         battle_active_f={:?} v350_seq={:?} v354_max={} v352_peak={} v343_peak={} max239pc={} \
         max242pc={} ce239_alive={} ce239_pc={:?} warnings={} unanswered250={}",
        t.ce253_started, t.ce239_started, t.ce242_started, t.ce243_started, t.ce241_started,
        t.wiring_rounds, t.battle_active_frame, t.v350_seq, t.v354_max, t.v352_peak, t.v343_peak,
        t.max_ce239_pc, t.max_ce242_pc, t.ce239_alive_at_end, t.ce239_stuck_pc,
        t.warnings, t.unanswered_250
    );

    // ---- 1) The REAL day-4 battle dispatch chain fires end to end -----------
    assert!(t.ce253_started, "day4 must dispatch CE253 地域貢献");
    assert!(t.ce239_started, "CE253 must call the real battle event CE239");
    assert!(
        t.max_ce239_pc >= 700,
        "CE239 must reach its real battle-start staging (>=pc700), got {}",
        t.max_ce239_pc
    );
    assert!(t.battle_active_frame.is_some(), "2000340 (battle-active) must rise");
    // ---- 2) The rebuilt wiring fires and starts CE242 with the acting slot --
    assert!(
        t.wiring_rounds >= 1,
        "the rebuilt wiring must start the CE242 command phase on the 2000340 edge (NO injection)"
    );
    assert!(t.ce242_started, "CE242 (command phase) must run");
    // The weak from-scratch party still makes CE253 take its low-power branch, so the
    // battle *context* 2000354 stays 0. That is not the phase argument: CE242's arg0 is
    // the acting slot (1), which is what the wiring supplies.
    assert!(
        pre_power < 200,
        "day-4 replay party battle power 2000028 ({pre_power}) is below the CE253 high branch (200)"
    );
    assert_eq!(
        t.v354_max, 0,
        "2000354 stays 0 on the low-power branch (battle context, not the phase argument)"
    );
    // ---- 3) CE242 takes the main-party action block and CE244 settles -------
    assert!(
        t.max_ce242_pc >= 200,
        "CE242 must reach its action block (>= pc200, the CE244 call region), max pc {}",
        t.max_ce242_pc
    );
    assert!(settled, "the round must settle on the genuine day-4 replay");
    assert!(
        t.v350_seq.first() == Some(&0) && t.v350_seq.contains(&1),
        "2000350 must start idle and enter the command phase, got {:?}",
        t.v350_seq
    );
    assert!(
        t.v352_peak == 1 || t.v352_peak == -1,
        "CE244 must settle: 2000352 in {{1,-1}}, got {}",
        t.v352_peak
    );
    assert_eq!(t.v343_peak, 1, "CE244 must set 2000343 = 1");
    // ---- 4) CE239 returns after the settlement ------------------------------
    assert!(!t.ce239_alive_at_end, "CE239 must return after 2000343 = 1, pc {:?}", t.ce239_stuck_pc);
    // ---- 5) Clean engine: no warnings; any unanswered 250 read can only be the
    // cosmetic command-menu section-header label (CData 5,16,0).
    assert_eq!(t.warnings, 0, "no warnings allowed through the day-4 battle");
    assert!(
        t.unanswered_250 <= 1,
        "at most the one cosmetic menu-label read may be unanswered, got {}",
        t.unanswered_250
    );
    for m in &unanswered_msgs {
        assert!(
            m.contains("CData (5, 16, 0)"),
            "unexpected unanswered 250 read in the battle section: {m}"
        );
    }

    println!(
        "\nDAY4 BATTLE RESULT: ce253={} ce239={} ce242={} ce243={} ce241={} rounds={} \
         battle_active_f={:?} v350_seq={:?} v354_max={} v352_peak={} v343_peak={} max239pc={} \
         max242pc={} ce239_alive={} ce239_pc={:?} warnings={} unanswered250={}",
        t.ce253_started, t.ce239_started, t.ce242_started, t.ce243_started, t.ce241_started,
        t.wiring_rounds, t.battle_active_frame, t.v350_seq, t.v354_max, t.v352_peak, t.v343_peak,
        t.max_ce239_pc, t.max_ce242_pc, t.ce239_alive_at_end, t.ce239_stuck_pc,
        t.warnings, t.unanswered_250
    );

    // ---- 6) The rest of day 4 and the natural night sleep reach day 5 --------
    finish_day_after_battle(&mut game, 4);
    sleep_naturally_into_next_morning(&mut game, 4);

    // Day 5 08:30, Friday. No settlement variable was ever injected.
    assert_eq!(game.vm.vars.get(340), 0, "post-battle 2000340 is idle again");
    assert_eq!(game.vm.vars.get(343), 0, "CE239 clears 2000343 when it returns");
    assert_eq!(game.vm.vars.get(160), 5, "day 5 counter");
    assert_eq!(game.vm.vars.get(6), 4, "day 5 is Friday");
    assert_eq!(game.vm.vars.get(4), 8, "day 5 08:30");
    assert_eq!(game.vm.vars.get(5), 30, "day 5 08:30");
    assert_eq!(game.stats.warnings, 0, "the whole day4->day5 run is warning-free");

    eprintln!(
        "m7_day4_playable [SETTLED]: day1->day4 real CE253->CE239, wiring starts CE242 with the \
         acting slot -> CE244 settles (2000352={}, 2000343=1) -> CE239 returns -> evening -> \
         natural night menu -> day5 08:30; warnings={}, frames={}",
        t.v352_peak, game.stats.warnings, game.stats.frames
    );
}
