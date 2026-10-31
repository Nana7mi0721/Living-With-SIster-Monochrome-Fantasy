//! M7 task-38: day-4 end-to-end audit with NO injection.
//!
//! Goal of task-38 was to prove day4 real battle settlement -> evening -> day5 08:30
//! *without* test-injecting the CE244 settlement variables (2000350/2000352/2000343),
//! driving the round through the task-34 rebuilt wiring (crates/wolf-game/src/battle_wiring.rs:
//! start the command phase CE242 on the 0->1 edge of the battle-active flag 2000340 while
//! 2000350 is idle).
//!
//! This test replays the genuine schedule from a new game to day 4 08:30 (same helper
//! pattern as m6_multiday / m7_day5_schedule), lets the retail data force day 4 daytime into
//! CE253 地域貢献 -> CE239, and lets the rebuilt wiring fire. It then ASSERTS THE EXACT EDGE
//! WHERE THE SHIPPED DATA BLOCKS THE ROUND, and PASSES on that proven chain.
//!
//! ROOT CAUSE (see docs/research/M7_DAY4_PLAYABLE.md for the full trace):
//!   In a from-scratch replay the party battle power 2000028 is only 132. CE253's pc183
//!   sub-branch treats 100..149 as a DEAD ZONE that passes command 0 (a valid 1..3 command
//!   needs 2000028 >= 200), so it takes pc187 case2 -> pc188 210[500239,...] with arg3=0 and passes
//!   a non-action command: 2000354 is NEVER set to a valid 1..3 (it stays 0 for the whole
//!   battle). The wiring faithfully forwards vars[354]=0 as the CE242 arg0 (self1600000=0),
//!   so CE242 fails its pc143 gate "self1600000 IN {1,2,3}", takes the pc210 DLC/追加コマンド
//!   ELSE branch, and never reaches pc201 CALL 500244 (CE244) / pc207 CALL 500243 / pc208
//!   SET 2000350=2. So 2000350 sticks at 1 and CE239 parks forever at pc13.
//!
//! NOTE the Lead's task-31 hypothesis (2000349==1 vs CE244 self1600010 blocks settlement)
//! is FALSIFIED: CLI dump --event 242 --self 600=1 with --var 2000349={0,1,11} all settle
//! (v352=1) identically; 2000349 is written only by CE239 pc564 and CE241 pc78 and is a
//! red herring. CE244's win block pc310-317 is reached linearly.
//!
//! WHY TASK-34 PASSES: crates/wolf-game/tests/m7_battle_playable.rs loads SaveData01 whose
//! party battle power 2000028 = 1815 (>= 300); CE253 then takes the high-power pc174 branch
//! and passes a valid command, so CE242 acts and CE244 settles. SaveData01 is not day 4.
//!
//! NO TEST-ONLY INJECTION of 2000350/2000352/2000343 (or any settlement state) is performed.
//! The only data seeding is the task-19 battle database seed (party stats / command slots /
//! item stock) plus one cosmetic command-menu section-header label, exactly as task-34.
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

/// Click the first workday command button (row0) and let the day run to 17:00 then the
/// evening scene to the 19:30 bedroom. Days 2 and 3 only (day 4 is the forced battle).
fn spend_day_choosing_row(game: &mut Game, day: i32) {
    game.auto_choice = true;
    let mut drawn = false;
    for _ in 0..700 {
        game.frame();
        if let Some(pic) = game.screen.get(1201) {
            if !pic.text.is_empty() {
                drawn = true;
                break;
            }
        }
    }
    assert!(drawn, "day {day}: workday command menu never drew");
    let energy_before = game.vm.vars.get(14);
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
    for _ in 0..2400 {
        game.frame();
        if game.vm.instances().iter().all(|i| i.event != 251) {
            finished = true;
            break;
        }
    }
    assert!(finished, "day {day}: daytime command never returned");
    assert_eq!(game.stats.warnings, 0, "day {day}: warnings during day");
    assert_eq!(game.vm.vars.get(4), 17, "day {day}: expected 17:00");
    assert_eq!(game.vm.vars.get(5), 0, "day {day}: expected 17:00");
    assert_eq!(game.vm.vars.get(160), day);
    assert!(game.vm.vars.get(14) < energy_before, "day {day}: energy not spent");
    let mut evening = false;
    for _ in 0..2000 {
        game.frame();
        if game.vm.vars.get(4) == 19 && game.vm.vars.get(5) >= 30 {
            evening = true;
            break;
        }
    }
    assert!(evening, "day {day}: 19:30 evening not reached");
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
fn day4_real_battle_reaches_ce239_and_exposes_command_gate() {
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
    assert!(t.max_ce239_pc >= 700, "CE239 must reach its real battle-start staging (>=pc700), got {}", t.max_ce239_pc);
    assert!(t.battle_active_frame.is_some(), "2000340 (battle-active) must rise");
    // ---- 2) The rebuilt wiring fires faithfully and starts CE242 -----------
    assert!(
        t.wiring_rounds >= 1,
        "the rebuilt wiring must start the CE242 command phase on the 2000340 edge (NO injection)"
    );
    assert!(t.ce242_started, "CE242 (command phase) must run");

    // ---- 3) The BLOCKER: the command is invalid, so CE242 never reaches CE244
    // Real day-4 replay party power is below the CE253 threshold; CE253's low-power
    // branch passes no command, so 2000354 stays 0 and CE242's pc143 gate sends it
    // to the DLC/追加コマンド ELSE (pc210) instead of the action block that calls CE244.
    assert!(
        pre_power < 200,
        "day-4 replay party battle power 2000028 ({pre_power}) is below the CE253 valid-command threshold (200)"
    );
    assert_eq!(t.v354_max, 0, "2000354 must stay 0 (no valid command supplied by the low-power CE253 branch)");
    assert!(
        t.max_ce242_pc < 144,
        "CE242 must die before its pc143 action gate (never reach the CE244 call region), max pc {}",
        t.max_ce242_pc
    );
    assert!(!t.ce243_started, "CE243 must never run (CE242 never reached pc207)");
    assert!(
        t.v350_seq.contains(&1) && !t.v350_seq.contains(&2),
        "2000350 must stick at 1 (never advance to the action phase 2), got {:?}",
        t.v350_seq
    );
    // ---- 4) NO settlement happened (proves the gate is upstream of CE244) ---
    assert!(!settled, "the round must NOT settle in this low-power day-4 replay");
    assert_eq!(t.v352_peak, 0, "2000352 must never be written (CE244 not reached)");
    assert_eq!(t.v343_peak, 0, "2000343 must never be written (CE244 not reached)");
    // ---- 5) CE239 is parked at its end-judgment loop waiting for 2000343 -----
    assert!(t.ce239_alive_at_end, "CE239 must still be alive (blocked, never returned)");
    assert_eq!(t.ce239_stuck_pc, Some(13), "CE239 must be parked at pc13 (2000343 deadlock)");
    // ---- 6) Clean engine: no warnings; the only unanswered 250 read is the
    // cosmetic command-menu section-header label (CData 5,16,0) that the DLC/追加コマンド
    // branch tries to draw. It is not part of the battle settlement path.
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

    eprintln!(
        "m7_day4_playable [BLOCKER PROVEN]: day1->day4 real CE253->CE239 reaches battle start; \
         wiring starts CE242 on 2000340 edge, but party power 2000028={pre_power}<200 -> CE253 \
         low-power branch -> 2000354=0 -> CE242 exits at pc143 (max pc {}) before calling CE244; \
         2000350 sticks at {:?}; CE239 parks pc13; no settlement, warnings={}, frames={}",
        t.max_ce242_pc, t.v350_seq, t.warnings, game.stats.frames
    );
}
