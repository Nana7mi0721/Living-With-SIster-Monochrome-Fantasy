//! M11 task-95: arm the rebuilt engine battle drive by default on the natural day-4 route.
//!
//! ## The gap this closes
//!
//! Before task-95 the game built BattleWiring disabled and only the tests called
//! Game::set_battle_wiring(true) / set_battle_round_loop(true). On the shipped natural
//! route the day-4 story forces CE253 (地域貢献) -> CE239 (戦闘呼び出し); CE239 raises the
//! battle-active flag 2000340 but nothing starts the command phase CE242 (its only writer of
//! 2000350=1 is itself), so CE239 parks at pc13 with 2000350 stuck at 0 and the clock never
//! leaves 08:30. This test proves the default (no wiring flag) now settles that battle.
//!
//! ## What changed (production)
//!
//! BattleWiring now auto-arms on the shipped battle dispatch event CE253 while 2000340
//! rises, and runs the multi-round loop (the day-4 fight needs two command rounds to settle).
//! An explicit Game::set_battle_wiring(false) is a hard opt-out and any explicit
//! set_battle_wiring(true) keeps the old opt-in behaviour (see src/battle_wiring.rs and
//! docs/research/M11_BATTLE_AUTOSTART.md).
//!
//! Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_battle_autostart -- --nocapture

use std::path::PathBuf;

use wolf_game::db::Container;
use wolf_game::Game;

// ---------------------------------------------------------------------------------------
// Data root / save helpers
// ---------------------------------------------------------------------------------------

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

fn retail_save(name: &str) -> PathBuf {
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop();
    b.pop();
    b.join("work/retail_backup/Save").join(name)
}

fn open_game() -> Option<Game> {
    match Game::new(data_root()) {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("skip m11_battle_autostart: {e}");
            None
        }
    }
}

// ---------------------------------------------------------------------------------------
// Input / menu helpers (mirror m7_day4_playable)
// ---------------------------------------------------------------------------------------

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

fn evname<E: std::fmt::Debug>(e: &E) -> String {
    format!("{:?}", e)
}

// ---------------------------------------------------------------------------------------
// Real day-loop drivers (from m7_day4_playable)
// ---------------------------------------------------------------------------------------

fn sleep_into_next_morning(game: &mut Game, day: i32) {
    let expected = day + 1;
    game.auto_choice = false;
    game.vm.vars.set(63, 1);
    let options = wait_for_choice(game, 500, "sleep menu");
    assert_eq!(options, SLEEP_OPTIONS, "day {day}: unexpected sleep menu");
    game.set_choice_cursor(2); // 今天还是睡吧
    decide(game);
    let options = wait_for_choice(game, 500, "sleep confirm");
    assert_eq!(options, CONFIRM_OPTIONS, "day {day}: unexpected confirm menu");
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
        "day {day}: expected the clock to move past 15:00, got {}:{:02}",
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

fn sleep_naturally_into_next_morning(game: &mut Game, day: i32) {
    let expected = day + 1;
    game.auto_choice = false;
    if game.vm.vars.get(295) == 1 {
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

// ---------------------------------------------------------------------------------------
// Battle trace
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct BattleTrace {
    ce253_started: bool,
    ce239_started: bool,
    ce242_started: bool,
    ce243_started: bool,
    ce241_started: bool,
    wiring_rounds: u64,
    battle_active_frame: Option<usize>,
    v350_seq: Vec<i32>,
    v352_peak: i32,
    v343_peak: i32,
    max_ce239_pc: i32,
    max_ce242_pc: i32,
    ce239_alive_at_end: bool,
    ce239_stuck_pc: Option<usize>,
    warnings: u64,
    unanswered_250: usize,
}

fn trace_battle(game: &mut Game, budget: usize) -> (BattleTrace, bool) {
    let mut t = BattleTrace::default();
    let mut last350 = i32::MIN;
    let mut settled = false;
    for f in 0..budget {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        let v340 = game.vm.vars.get(340);
        let v350 = game.vm.vars.get(350);
        let v352 = game.vm.vars.get(352);
        let v343 = game.vm.vars.get(343);
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
                    t.max_ce239_pc = t.max_ce239_pc.max(inst.pc as i32);
                }
                "242" => {
                    t.ce242_started = true;
                    t.max_ce242_pc = t.max_ce242_pc.max(inst.pc as i32);
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
    if settled {
        for f in 0..2000 {
            game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
            game.frame();
            if !has_event(game, 239) {
                break;
            }
        }
    }
    game.key_bits = 0;
    t.ce239_alive_at_end = has_event(game, 239);
    t.ce239_stuck_pc = event_pc(game, 239);
    t.warnings = game.stats.warnings;
    t.unanswered_250 = game
        .diagnostics
        .iter()
        .filter(|(msg, _)| msg.contains("unanswered"))
        .count();
    (t, settled)
}

// ---------------------------------------------------------------------------------------
// (a) Natural day-4 battle settles WITHOUT ever calling set_battle_wiring.
// ---------------------------------------------------------------------------------------

#[test]
fn natural_day4_battle_settles_without_wiring_flag() {
    let Some(mut game) = open_game() else { return };
    game.auto_advance = true;
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;

    // Drive the genuine new-game schedule to day 4 08:30 (same entry as m7_day4_playable),
    // with no test-only shortcuts and NO set_battle_wiring flag.
    game.start_map_event_by_name("SampleMapA", 0).expect("entry event");
    for _ in 0..1600 {
        game.frame();
    }
    assert_eq!(game.stats.warnings, 0, "day1 intro is warning-free");
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
    assert_eq!(game.vm.vars.get(160), 4, "reached day 4");
    assert_eq!(game.vm.vars.get(6), 3, "day4 weekday = Thursday");
    assert_eq!(game.vm.vars.get(4), 8, "day4 08:30");
    assert_eq!(game.vm.vars.get(5), 30, "day4 08:30");

    // ---- day 4 daytime: real battle, DEFAULT production settings (NO wiring flag) ----
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let seeded = game.seed_battle_databases();
    assert!(seeded > 0, "seed_battle_databases must seed the battle cells");
    game.databases
        .set_text(Container::CData, 5, 16, 0, "┗ 状態リスト");
    let _ = game.start_engine_events(); // production boot analogue; NOT a battle flag

    // The whole point: the rebuilt drive must be armed with NO set_battle_wiring call.
    assert!(
        game.battle_wiring_auto_armed(),
        "the natural-battle auto-arm must default to ON"
    );
    assert_eq!(game.vm.vars.get(340), 0, "pre-battle 2000340 idle");
    assert_eq!(game.vm.vars.get(350), 0, "pre-battle 2000350 idle");
    assert_eq!(game.vm.vars.get(352), 0, "pre-battle 2000352 idle");
    assert_eq!(game.vm.vars.get(343), 0, "pre-battle 2000343 idle");

    let (t, settled) = trace_battle(&mut game, 2400);

    println!(
        "AUTOSTART (a) RESULT: ce253={} ce239={} ce242={} ce243={} ce241={} rounds={} \
         battle_active_f={:?} v350_seq={:?} v352_peak={} v343_peak={} max239pc={} max242pc={} \
         ce239_alive={} ce239_pc={:?} warnings={} unanswered250={}",
        t.ce253_started, t.ce239_started, t.ce242_started, t.ce243_started, t.ce241_started,
        t.wiring_rounds, t.battle_active_frame, t.v350_seq, t.v352_peak, t.v343_peak,
        t.max_ce239_pc, t.max_ce242_pc, t.ce239_alive_at_end, t.ce239_stuck_pc,
        t.warnings, t.unanswered_250
    );

    assert!(t.ce253_started, "day4 must dispatch CE253 地域貢献");
    assert!(t.ce239_started, "CE253 must call the real battle event CE239");
    assert!(t.max_ce239_pc >= 700, "CE239 must reach its battle-start staging (>=pc700)");
    assert!(t.battle_active_frame.is_some(), "2000340 must rise");
    assert!(
        t.wiring_rounds >= 1,
        "the auto-armed drive must start CE242 with NO set_battle_wiring call"
    );
    assert!(t.ce242_started, "CE242 command phase must run");
    assert!(settled, "the natural day-4 battle must settle");
    assert!(
        t.v350_seq.first() == Some(&0) && t.v350_seq.contains(&1),
        "2000350 must start idle and enter the command phase, got {:?}",
        t.v350_seq
    );
    assert!(t.v352_peak == 1 || t.v352_peak == -1, "CE244 must settle (2000352 in {{1,-1}}), got {}", t.v352_peak);
    assert_eq!(t.v343_peak, 1, "CE244 must set 2000343 = 1");
    assert!(!t.ce239_alive_at_end, "CE239 must return after settlement, pc {:?}", t.ce239_stuck_pc);
    assert_eq!(t.warnings, 0, "no warnings in the day-4 battle");
    assert!(t.unanswered_250 <= 1, "at most the one cosmetic menu-label read");

    // ---- rest of day 4 and the natural night sleep reach day 5 ----
    finish_day_after_battle(&mut game, 4);
    sleep_naturally_into_next_morning(&mut game, 4);
    assert_eq!(game.vm.vars.get(340), 0, "post-battle 2000340 idle again");
    assert_eq!(game.vm.vars.get(343), 0, "CE239 clears 2000343 when it returns");
    assert_eq!(game.vm.vars.get(160), 5, "day 5 counter");
    assert_eq!(game.vm.vars.get(6), 4, "day 5 is Friday");
    assert_eq!(game.vm.vars.get(4), 8, "day 5 08:30");
    assert_eq!(game.vm.vars.get(5), 30, "day 5 08:30");
    assert_eq!(game.stats.warnings, 0, "the whole day4->day5 run is warning-free");
    eprintln!(
        "m11_battle_autostart [A]: natural day-4 CE253->CE239 settled with NO set_battle_wiring \
         (rounds={}, 2000352={}, 2000343=1) -> day 5 08:30; warnings={}, frames={}",
        t.wiring_rounds, t.v352_peak, game.stats.warnings, game.stats.frames
    );
}

// ---------------------------------------------------------------------------------------
// Two-member party fixture (mirrors m11_party_live_bridge) for test (b).
// ---------------------------------------------------------------------------------------

const PARTY_COUNT_TYPE: i32 = 18;
const PARTY_TABLE_TYPE: i32 = 6;
const STATUS_TYPE: i32 = 0;
const MEMBER2_ROW: i32 = 13;

fn build_fixture(game: &mut Game) -> usize {
    let seeded = game.seed_battle_databases();
    assert!(seeded > 0, "battle DB seed must land");
    game.databases
        .set_num(Container::CData, PARTY_COUNT_TYPE, 4, 0, 2);
    game.databases
        .set_num(Container::CData, PARTY_TABLE_TYPE, 0, 2, MEMBER2_ROW);
    let stats = [
        (20, 100),
        (21, 100),
        (22, 50),
        (23, 50),
        (24, 20),
        (25, 15),
        (26, 10),
        (27, 30),
        (28, 30),
        (9, 20),
        (10, 10),
        (4, 10),
    ];
    for (c, v) in stats {
        game.databases
            .set_num(Container::CData, STATUS_TYPE, MEMBER2_ROW, c, v);
    }
    let commands = [4, 3, 6, 7, 0, 0, 0, 0];
    for (i, v) in commands.iter().enumerate() {
        game.databases.set_num(
            Container::CData,
            STATUS_TYPE,
            MEMBER2_ROW,
            47 + i as i32,
            *v,
        );
    }
    seeded
}

fn open_fixture_game() -> Game {
    let mut game = Game::new(data_root()).unwrap();
    game.load_save(&retail_save("SaveData01.sav")).unwrap();
    // Synthetic battle-driver fixture: the retail save's common-event self banks are
    // irrelevant here and, once restored, perturb the deterministic multi-round queue.
    // Start from a clean self store so the driver is exercised on its own merits.
    game.vm.clear_common_self();
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    build_fixture(&mut game);
    // read-load fix removes the old spurious 506 coroutine; pin event RNG so this
    // fixture's intended four-round queue sequence is deterministic (seed 9 keeps 4 rounds).
    game.vm.rng.set_event_seed(9);
    game
}

fn pools(game: &Game) -> [i32; 4] {
    [
        game.vm.vars.get(345),
        game.vm.vars.get(346),
        game.vm.vars.get(347),
        game.vm.vars.get(348),
    ]
}

// ---------------------------------------------------------------------------------------
// (b) Two-member fixture command queue dispatches distinct CE242 bodies under auto drive.
// ---------------------------------------------------------------------------------------

#[test]
fn two_member_queue_dispatches_distinct_bodies_under_auto_drive() {
    let mut game = open_fixture_game();
    game.databases
        .set_text(Container::CData, 5, 16, 0, "┗ 状態リスト");
    let _ = game.start_engine_events();
    // Queue of four real registered-row indexes; NO set_battle_wiring / set_battle_round_loop.
    game.set_battle_menu_command_queue(vec![1, 2, 3, 4]);
    // Genuine BOSS path (CE239 pc202: 2000445 == 1 boosts the enemy gauge from 2000446) so the
    // fight survives long enough for four command rounds. These are data cells, not settlement.
    game.vm.vars.set(445, 1);
    game.vm.vars.set(446, 4000);
    assert!(game.battle_wiring_auto_armed(), "auto-arm default ON");
    // CE253 is a synthetic battle-driver probe here, not a save-fidelity round trip:
    // start it fresh so any persisted self bank in SaveData01 cannot perturb the
    // deterministic multi-round queue this test asserts on.
    assert!(
        game.vm.start_event_fresh(253).is_some(),
        "CE253 battle must start"
    );

    let mut recs: Vec<(i32, i32)> = Vec::new(); // per-round (max pc reached, arg0 command index)
    let mut pool_pre: Vec<[i32; 4]> = Vec::new();
    let mut last_rounds = 0u64;
    let mut settle = (0, 0);
    let mut settled = false;
    for f in 0..40000usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        let started = game.battle_wiring_rounds();
        if started > last_rounds {
            last_rounds = started;
            pool_pre.push(pools(&game));
            recs.push((0, 0));
        }
        if let Some(slot) = recs.last_mut() {
            for inst in game.vm.instances() {
                if evname(&inst.event) == "242" {
                    let pc = inst.pc as i32;
                    // Record the FIRST pc past the pc143 dispatch head = the case body entry
                    // (145 / 476 / 761 / 972 for command indexes 1/2/3/4).
                    if slot.0 == 0 && pc > 143 {
                        slot.0 = pc;
                    }
                    let a = inst.self_vars.get(600);
                    if a != 0 {
                        slot.1 = a;
                    }
                }
            }
        }
        if game.vm.vars.get(343) == 1 && game.vm.vars.get(350) == 0 {
            settle = (game.vm.vars.get(352), game.vm.vars.get(343));
            settled = true;
            break;
        }
    }
    let first_pcs: Vec<i32> = recs.iter().map(|r| r.0).collect();
    let arg0s: Vec<i32> = recs.iter().map(|r| r.1).collect();
    let final_pools = pools(&game);
    let warnings = game.stats.warnings;

    println!(
        "AUTOSTART (b) RESULT: rounds={} first_pcs={:?} arg0s={:?} pre_pools={:?} final_pools={:?} \
         settle={:?} warnings={}",
        game.battle_wiring_rounds(), first_pcs, arg0s, pool_pre, final_pools, settle, warnings
    );

    assert!(settled, "the automatic drive must settle the BOSS battle");
    assert_eq!(game.battle_wiring_rounds(), 4, "four queued command rounds must run");
    assert_eq!(first_pcs.len(), 4, "four command rounds captured");
    let mut uniq = first_pcs.clone();
    uniq.sort_unstable();
    uniq.dedup();
    assert_eq!(
        uniq.len(),
        4,
        "each queued index must reach a DISTINCT CE242 command body, got {:?}",
        first_pcs
    );
    assert!(first_pcs.iter().all(|&p| p > 143), "each body advanced past dispatch ({:?})", first_pcs);
    assert_eq!(arg0s, vec![1, 2, 3, 4], "the queue is consumed front-first under auto drive");
    assert_eq!(
        final_pools,
        [3, 2, 3, 2],
        "pools 2000345 and 2000346 must drain (multi-member data under the auto drive)"
    );
    assert!(settle.0 == 1 || settle.0 == -1, "real settlement result, got {:?}", settle);
    assert_eq!(settle.1, 1, "2000343 decided");
    assert_eq!(warnings, 0, "no warnings under the auto drive");
}

// ---------------------------------------------------------------------------------------
// (c) Explicit disable remains honored even with the natural CE253 battle resident.
// ---------------------------------------------------------------------------------------

#[test]
fn explicit_disable_overrides_auto_arm() {
    let mut game = Game::new(data_root()).unwrap();
    game.load_save(&retail_save("SaveData01.sav")).unwrap();
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let seeded = game.seed_battle_databases();
    assert!(seeded > 0);
    // read-load fix removes the old spurious 506 coroutine; pin event RNG so CE253's
    // random 工作 branch deterministically reaches the resident battle (seed 2 does).
    game.vm.rng.set_event_seed(2);
    game.databases
        .set_text(Container::CData, 5, 16, 0, "┗ 状態リスト");
    let _ = game.start_engine_events();
    // Hard opt-out: the auto-arm must NOT drive the shipped battle.
    game.set_battle_wiring(false);
    assert!(game.start_event(253), "CE253 battle starts");

    let mut max350 = 0;
    let mut battle_active = false;
    for f in 0..2000usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        if game.vm.vars.get(340) != 0 {
            battle_active = true;
        }
        max350 = max350.max(game.vm.vars.get(350));
    }
    let rounds = game.battle_wiring_rounds();
    let v352 = game.vm.vars.get(352);
    let v343 = game.vm.vars.get(343);
    println!(
        "AUTOSTART (c) RESULT: battle_active={battle_active} max350={max350} rounds={rounds} \
         v352={v352} v343={v343} warnings={}",
        game.stats.warnings
    );
    assert!(battle_active, "CE253/CE239 must reach the battle-active flag");
    assert_eq!(rounds, 0, "explicit disable must not start CE242");
    assert_eq!(max350, 0, "2000350 must stay 0 (shipped stall preserved)");
    assert_eq!(v352, 0, "no settlement under explicit disable");
    assert_eq!(v343, 0, "no decision under explicit disable");
    assert_eq!(game.stats.warnings, 0, "no warnings");
}
