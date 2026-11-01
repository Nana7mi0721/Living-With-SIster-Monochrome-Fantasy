//! M9 task-68: the natural day-4 battle is a genuine multi-round fight.
//!
//! Under correct opcode-111 value-operand (reference) semantics the continuous-route day-4
//! battle no longer one-shots the enemy. With the round loop enabled 2000350 cycles
//! 0 -> 1 -> 2 -> 0 FOUR times before CE244 settles through its enemy-gauge<=0 arm: CE241
//! (turn start) and CE243 (enemy action) both run every round. This replaces the old
//! single-round artifact (the literal-operand bug mis-sized the gauge so the first command
//! zeroed it before phase 2 was ever reached).
//!
//! Measured (WOLF_DATA_ROOT=work/all): v350=[0,1,2,0,1,2,0,1,2,0,1,0], settle frame 1516,
//! 2000349=4 (four command rounds: three full 0->1->2 cycles + a final 0->1->0), 2000342 min 0
//! (enemy gauge drained), 2000343=1, 2000352=1 (victory). CE241 live for 258 frames, CE243 for
//! 294 frames. Task-79 proved this trajectory is byte-reproducible across process restarts (5x,
//! identical trace) — the exact values below are locked as a determinism regression.
//!
//! Two probes in this file:
//!  1. the natural continuous route above (test-only trace of live event ids, phase, enemy
//!     gauge, CE241/CE243 presence and CE244 result writes), asserting the exact locked
//!     trajectory v350=[0,1,2,0,1,2,0,1,2,0,1,0] settling at frame 1516 with v349=4 and
//!     pools [3,2,3,2], CE241/CE243 running before settle;
//!  2. the task-57 BOSS path with set_battle_round_loop(true) as an independent multi-round
//!     control.
//!
//! Test-only: no production edits, no commit/push.
//! Run: cargo test -q -p wolf-game --test m9_battle_round_gap -- --nocapture
use std::path::PathBuf;

use wolf_game::key;
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

/// The continuous route runs on the full work/all data root. WOLF_DATA_ROOT wins; when
/// work/all is missing BasicData we fall back to work/full so the test still says why.
fn data_root() -> PathBuf {
    if let Ok(raw) = std::env::var("WOLF_DATA_ROOT") {
        return resolve_root(&raw);
    }
    let all = resolve_root("work/all");
    if all.join("BasicData").is_dir() {
        all
    } else {
        resolve_root("work/full")
    }
}

fn open_game() -> Option<Game> {
    match Game::new(data_root()) {
        Ok(game) => Some(game),
        Err(err) => {
            eprintln!("skip m9_battle_round_gap: {err}");
            None
        }
    }
}

fn decide(game: &mut Game) {
    game.key_bits = key::OK;
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

fn has_event(game: &Game, event: u32) -> bool {
    game.vm
        .instances()
        .iter()
        .any(|instance| instance.event == event)
}

/// Open the night menu with the real hover-sister + Decide gesture, then
/// 今天先休息吧 -> 是 and stop at the next morning 08:30. Mirrors m6_night_sleep.
fn sleep_naturally_into_next_morning(game: &mut Game, day: i32) {
    let expected = day + 1;
    game.auto_choice = false;
    if game.vm.vars.get(295) == 1 {
        game.vm.vars.set(295, 0);
    }
    game.vm.vars.set(68, 0);
    // At the day-1 18:30 bedroom the sister sprite is not up yet; let the real dinner
    // script settle the evening (auto-answering its own prompts) until picture 120 appears.
    game.auto_choice = true;
    for _ in 0..1200 {
        if game.screen.get(120).is_some() {
            break;
        }
        game.frame();
    }
    game.auto_choice = false;
    let sister = game.screen.get(120).unwrap_or_else(|| {
        panic!(
            "day {day}: sister sprite picture 120 missing at {}:{:02}",
            game.vm.vars.get(4),
            game.vm.vars.get(5)
        )
    });
    game.mouse = [sister.x, sister.y];
    for _ in 0..8 {
        game.frame();
    }
    decide(game);
    for _ in 0..40 {
        game.frame();
    }
    assert!(
        has_event(game, 245),
        "day {day}: the night command menu is open"
    );

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
    assert!(
        game.screen.len() > 20,
        "day {expected}: morning scene drawn"
    );
}

const DAY_COMMAND_LABELS: [&str; 6] = [
    "给店里帮忙",
    "学习会",
    "基础锻炼",
    "攻击练习",
    "防御练习",
    "秘密特训",
];
const DAY_ROW_PICS: [i32; 5] = [1201, 1203, 1205, 1207, 1209];

fn day_row_texts(game: &Game) -> Vec<String> {
    DAY_ROW_PICS
        .iter()
        .map(|&number| {
            game.screen
                .get(number)
                .map(|picture| picture.text.clone())
                .unwrap_or_default()
        })
        .collect()
}

fn evening_work(game: &Game) -> bool {
    game.vm
        .instances()
        .iter()
        .any(|instance| matches!(instance.event, 251 | 300 | 301 | 326 | 406))
}

/// Hover and press the CE251 daytime command. Prefer the 学习会 study action when the
/// retail PRNG draws it, otherwise pick the first drawn CE252 task like a player would.
/// Returns the label that was actually selected.
fn spend_day_choosing_row(game: &mut Game, day: i32) -> String {
    game.auto_choice = true;
    let mut rows: Vec<(usize, String)> = Vec::new();
    for _ in 0..1200 {
        game.frame();
        rows = day_row_texts(game)
            .iter()
            .enumerate()
            .filter(|(_, text)| DAY_COMMAND_LABELS.iter().any(|label| text.contains(label)))
            .map(|(row, text)| (row, text.clone()))
            .collect();
        if !rows.is_empty() {
            break;
        }
    }
    assert!(
        !rows.is_empty(),
        "day {day}: CE251 draws a CE252 daytime command"
    );
    let (row, label) = rows
        .iter()
        .find(|(_, text)| text.contains("学习会"))
        .cloned()
        .unwrap_or_else(|| rows[0].clone());
    println!(
        "m9_battle_round_gap day {day}: CE251 rows {rows:?}, selecting {label:?} at row {row}"
    );
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
        "day {day}: daytime command never returned ({label})"
    );
    assert_eq!(game.stats.warnings, 0, "day {day}: warnings during day");
    assert_eq!(game.vm.vars.get(4), 17, "day {day}: expected 17:00");
    assert_eq!(game.vm.vars.get(5), 0, "day {day}: expected 17:00");
    assert_eq!(game.vm.vars.get(160), day);
    assert!(
        game.vm.vars.get(14) < energy_before,
        "day {day}: energy not spent"
    );
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
    assert_eq!(
        game.vm.vars.get(5),
        30,
        "day {day}: evening settles on the half hour"
    );
    assert_eq!(game.stats.warnings, 0, "day {day}: warnings in evening");
    label
}

fn live_ids(game: &Game) -> Vec<u32> {
    let mut ids: Vec<u32> = game.vm.instances().iter().map(|i| i.event).collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

#[derive(Default)]
struct RoundTrace {
    v350_seq: Vec<i32>,
    phase_frames: Vec<(usize, i32, i32)>,
    battle_active_frame: Option<usize>,
    ce253: bool,
    ce239: bool,
    ce242: bool,
    ce241_frames: usize,
    ce241_first_frame: Option<usize>,
    ce241_max_pc: i32,
    ce243_frames: usize,
    ce243_first_frame: Option<usize>,
    ce243_max_pc: i32,
    ce243_frame_list: Vec<usize>,
    tail_live: Vec<(usize, Vec<u32>)>,
    ce241_frame_list: Vec<usize>,
    ce244_frames: usize,
    ce244_first_frame: Option<usize>,
    ce244_first_pc: i32,
    ce244_max_pc: i32,
    first_244_live: Vec<u32>,
    max_ce239_pc: i32,
    max_ce242_pc: i32,
    v342_min: i32,
    enemy_gauge_zero_frame: Option<usize>,
    result_write_frame: Option<usize>,
    decided_frame: Option<usize>,
    settle_frame: Option<usize>,
    live_at_result: Vec<u32>,
    v349_at_settle: i32,
    v350_at_settle: i32,
    v352_at_settle: i32,
    v343_at_settle: i32,
    pools_at_settle: [i32; 4],
    live_ids_seen: std::collections::BTreeSet<u32>,
    settled: bool,
}

fn scan_round(game: &Game, frame: usize, trace: &mut RoundTrace) {
    if game.vm.vars.get(340) != 0 && trace.battle_active_frame.is_none() {
        trace.battle_active_frame = Some(frame);
    }
    let live = live_ids(game);
    if (1590..=1610).contains(&frame) {
        trace.tail_live.push((frame, live.clone()));
    }
    for &id in &live {
        trace.live_ids_seen.insert(id);
    }
    if live.contains(&253) {
        trace.ce253 = true;
    }
    if live.contains(&239) {
        trace.ce239 = true;
    }
    if live.contains(&242) {
        trace.ce242 = true;
    }
    for inst in game.vm.instances() {
        match inst.event {
            239 => trace.max_ce239_pc = trace.max_ce239_pc.max(inst.pc as i32),
            242 => trace.max_ce242_pc = trace.max_ce242_pc.max(inst.pc as i32),
            241 => {
                trace.ce241_frames += 1;
                trace.ce241_frame_list.push(frame);
                trace.ce241_first_frame.get_or_insert(frame);
                trace.ce241_max_pc = trace.ce241_max_pc.max(inst.pc as i32);
            }
            243 => {
                trace.ce243_frames += 1;
                trace.ce243_frame_list.push(frame);
                trace.ce243_first_frame.get_or_insert(frame);
                trace.ce243_max_pc = trace.ce243_max_pc.max(inst.pc as i32);
            }
            244 => {
                trace.ce244_frames += 1;
                if trace.ce244_first_frame.is_none() {
                    trace.ce244_first_frame = Some(frame);
                    trace.ce244_first_pc = inst.pc as i32;
                    trace.first_244_live = live.clone();
                }
                trace.ce244_max_pc = trace.ce244_max_pc.max(inst.pc as i32);
            }
            _ => {}
        }
    }
    if trace.battle_active_frame.is_some() {
        let v342 = game.vm.vars.get(342);
        trace.v342_min = trace.v342_min.min(v342);
        if v342 <= 0 && trace.enemy_gauge_zero_frame.is_none() {
            trace.enemy_gauge_zero_frame = Some(frame);
        }
    }
    let v352 = game.vm.vars.get(352);
    if v352 != 0 && trace.result_write_frame.is_none() {
        trace.result_write_frame = Some(frame);
        trace.live_at_result = live.clone();
    }
    if game.vm.vars.get(343) == 1 && trace.decided_frame.is_none() {
        trace.decided_frame = Some(frame);
    }
}

fn run_natural_day_four_battle() -> Option<Game> {
    let mut game = open_game()?;
    game.boot().expect("the boot map has its autorun event");
    for _ in 0..800 {
        game.frame();
    }
    assert!(game.screen.iter().any(|p| p.file.contains("title01")));
    game.key_bits = key::OK;
    game.frame();
    game.key_bits = 0;
    for _ in 0..240 {
        game.frame();
    }
    let title_menu = game.choices.as_ref().expect("title menu");
    assert_eq!(title_menu.options, ["1", "2", "3"]);
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    for _ in 0..120 {
        game.frame();
        if game.choices.is_none() {
            break;
        }
    }
    game.auto_choice = false;
    game.auto_advance = true;
    let mut opening: Option<Vec<String>> = None;
    for _ in 0..900 {
        game.frame();
        if let Some(choices) = game.choices.as_ref() {
            opening = Some(choices.options.clone());
            break;
        }
    }
    assert_eq!(opening.map(|v| v.len()), Some(4), "opening prompt");
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    let mut day1 = false;
    for _ in 0..2600 {
        game.frame();
        if game.vm.vars.get(160) == 1 && game.vm.vars.get(4) == 18 && game.vm.vars.get(5) == 30 {
            day1 = true;
            break;
        }
    }
    assert!(day1, "day1 18:30");
    assert_eq!(game.stats.warnings, 0);
    sleep_naturally_into_next_morning(&mut game, 1);
    let _ = spend_day_choosing_row(&mut game, 2);
    sleep_naturally_into_next_morning(&mut game, 2);
    let _ = spend_day_choosing_row(&mut game, 3);
    sleep_naturally_into_next_morning(&mut game, 3);
    assert_eq!(game.vm.vars.get(160), 4);
    assert_eq!(game.vm.vars.get(6), 3);
    assert_eq!(game.vm.vars.get(4), 8);
    assert_eq!(game.vm.vars.get(5), 30);
    Some(game)
}

#[test]
fn natural_day_four_battle_runs_genuine_multi_round_and_settles() {
    let Some(mut game) = run_natural_day_four_battle() else {
        return;
    };
    // The boot path already ran start_engine_events; the boot-once CE241 copy is gone.
    let engine_after_boot = game.start_engine_events();
    assert_eq!(
        engine_after_boot, 0,
        "engine events were already started by boot()"
    );
    let started_before = game.vm.started_events.clone();
    assert!(
        started_before.contains(&241),
        "CE241 is started once by boot()"
    );
    assert!(
        !started_before.contains(&243),
        "CE243 has no reason to run before a battle"
    );

    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(game.seed_battle_databases() > 0);
    game.databases
        .set_text(wolf_game::db::Container::CData, 5, 16, 0, "┗ 状態リスト");
    game.start_engine_events();
    game.set_battle_wiring(true);
    // opcode-111 reference semantics: enable the round loop so the day-4 battle can run
    // the genuine multi-round fight instead of the old single-round instakill artifact.
    game.set_battle_round_loop(true);
    assert_eq!(game.vm.vars.get(340), 0);
    assert_eq!(game.vm.vars.get(350), 0);
    assert_eq!(game.vm.vars.get(352), 0);
    assert_eq!(game.vm.vars.get(343), 0);

    let mut trace = RoundTrace {
        v342_min: i32::MAX,
        ..Default::default()
    };
    let mut last350 = i32::MIN;
    for frame in 0..4000 {
        game.key_bits = if frame % 2 == 0 { key::OK } else { 0 };
        game.frame();
        let v340 = game.vm.vars.get(340);
        let v350 = game.vm.vars.get(350);
        if v340 != 0 && trace.battle_active_frame.is_none() {
            trace.battle_active_frame = Some(frame);
        }
        if v350 != last350 {
            trace.phase_frames.push((frame, last350, v350));
            trace.v350_seq.push(v350);
            last350 = v350;
        }
        scan_round(&game, frame, &mut trace);
        if game.vm.vars.get(343) == 1 && v350 == 0 {
            trace.settled = true;
            trace.settle_frame = Some(frame);
            trace.v349_at_settle = game.vm.vars.get(349);
            trace.v350_at_settle = v350;
            trace.v352_at_settle = game.vm.vars.get(352);
            trace.v343_at_settle = game.vm.vars.get(343);
            trace.pools_at_settle = [
                game.vm.vars.get(345),
                game.vm.vars.get(346),
                game.vm.vars.get(347),
                game.vm.vars.get(348),
            ];
            break;
        }
    }
    if trace.settled {
        for frame in 0..1600 {
            game.key_bits = if frame % 2 == 0 { key::OK } else { 0 };
            game.frame();
            let v350 = game.vm.vars.get(350);
            if v350 != last350 {
                trace.phase_frames.push((1600 + frame, last350, v350));
                trace.v350_seq.push(v350);
                last350 = v350;
            }
            scan_round(&game, 1600 + frame, &mut trace);
            if !has_event(&game, 239) {
                break;
            }
        }
        game.key_bits = 0;
    }
    let started_after = game.vm.started_events.clone();
    let newly_started: Vec<u32> = started_after.difference(&started_before).copied().collect();

    println!(
        "m9_battle_round_gap NATURAL: v350={:?} phases={:?} active_f={:?} ce253={} ce239={} ce242={} ce241_frames={} ce243_frames={} ce244_frames={} first244_f={:?} first244_pc={} first244_live={:?} max239pc={} max242pc={} v342_min={} gauge_zero_f={:?} result_write_f={:?} decided_f={:?} settle_f={:?} v349={} pools={:?} newly_started={:?} started_before_241={} started_after_241={} started_before_243={} started_after_243={} started_before_244={} started_after_244={} ce243_first={:?} ce243_last={:?} ce243_frames={} tail_live={:?} v352_settle={} v343_settle={} warnings={}",
        trace.v350_seq,
        trace.phase_frames,
        trace.battle_active_frame,
        trace.ce253,
        trace.ce239,
        trace.ce242,
        trace.ce241_frames,
        trace.ce243_frames,
        trace.ce244_frames,
        trace.ce244_first_frame,
        trace.ce244_first_pc,
        trace.first_244_live,
        trace.max_ce239_pc,
        trace.max_ce242_pc,
        trace.v342_min,
        trace.enemy_gauge_zero_frame,
        trace.result_write_frame,
        trace.decided_frame,
        trace.settle_frame,
        trace.v349_at_settle,
        trace.pools_at_settle,
        newly_started,
        started_before.contains(&241),
        started_after.contains(&241),
        started_before.contains(&243),
        started_after.contains(&243),
        started_before.contains(&244),
        started_after.contains(&244),
        trace.ce243_first_frame,
        trace.ce243_frame_list.last(),
        trace.ce243_frames,
        trace.tail_live,
        trace.v352_at_settle,
        trace.v343_at_settle,
        game.stats.warnings
    );

    assert!(
        trace.ce253 && trace.ce239 && trace.ce242,
        "CE253/239/242 dispatch chain"
    );
    assert!(trace.battle_active_frame.is_some(), "2000340 rose");
    // Task-79 determinism lock: this natural battle trajectory is byte-reproducible across
    // repeated process runs (verified 5x with an identical trace/md5), so pin exact values
    // instead of the old structural shape guards.
    let settle = trace.settle_frame.expect("settled frame");
    let main_phases: Vec<i32> = trace
        .phase_frames
        .iter()
        .filter(|(f, _, _)| *f <= settle)
        .map(|(_, _, v)| *v)
        .collect();
    assert_eq!(
        main_phases,
        vec![0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 0],
        "2000350 exact trajectory: four command rounds (0->1->2 x3 then 0->1->0 settle)"
    );
    // Frame indices shift 134 frames earlier than the pre-M13 baseline because persisted
    // common-event self (CE251 etc.) advances the day-flow clock; the phase trajectory,
    // round count and pools above are unchanged.
    assert_eq!(settle, 1382, "CE244 settles on the exact deterministic frame");
    let command_rounds = main_phases.iter().filter(|v| **v == 1).count();
    let action_phases = main_phases.iter().filter(|v| **v == 2).count();
    assert_eq!(command_rounds, 4, "exactly four command rounds");
    assert_eq!(action_phases, 3, "exactly three enemy-action phases");
    assert_eq!(trace.v349_at_settle, 4, "2000349 = four command rounds elapsed");
    assert_eq!(
        trace.pools_at_settle,
        [3, 2, 3, 2],
        "member pools (2000345/346/347/348) at settle"
    );
    assert_eq!(trace.ce241_frames, 258, "CE241 turn-start live-frame count");
    assert_eq!(trace.ce243_frames, 294, "CE243 enemy-action live-frame count");
    assert_eq!(
        trace.ce243_first_frame,
        Some(641),
        "CE243 first observed frame"
    );
    assert_eq!(
        trace.result_write_frame,
        Some(settle),
        "CE244 result and decided flag land on the same decisive frame"
    );
    assert_eq!(
        trace.enemy_gauge_zero_frame,
        Some(settle),
        "2000342 drained to <= 0 on the settle frame"
    );
    assert_eq!(trace.v342_min, 0, "enemy gauge drained to exactly 0");
    assert_eq!(trace.max_ce239_pc, 1099, "CE239 reaches its cleanup tail pc");
    assert_eq!(trace.max_ce242_pc, 967, "CE242 reaches the slot-3 body pc");
    // Deterministic victory: the reference-semantics day-4 fight always ends 2000352 = 1 (win);
    // this is NOT a {1,-1} coin flip (proven stable across process restarts by task-79).
    assert_eq!(trace.v352_at_settle, 1, "CE244 writes 2000352 = 1 (win)");
    assert_eq!(trace.v343_at_settle, 1, "battle-decided flag");
    assert!(!has_event(&game, 239), "CE239 returned");
    assert_eq!(game.stats.warnings, 0, "no warnings");
}

fn save01() -> PathBuf {
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    base.join("work/retail_backup/Save/SaveData01.sav")
}

fn open_boss_battle() -> Option<Game> {
    let save = save01();
    if !save.is_file() {
        eprintln!(
            "skip m9_battle_round_gap BOSS control: {} missing",
            save.display()
        );
        return None;
    }
    let mut game = Game::new(data_root()).ok()?;
    game.load_save(&save).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(game.seed_battle_databases() > 0);
    game.databases
        .set_text(wolf_game::db::Container::CData, 5, 16, 0, "┗ 状態リスト");
    game.start_engine_events();
    game.set_battle_wiring(true);
    game.set_battle_round_loop(true);
    game.vm.vars.set(445, 1);
    game.vm.vars.set(446, 4000);
    assert!(game.start_event(253));
    Some(game)
}

#[test]
fn boss_multi_round_path_observes_ce243_and_ce241() {
    let Some(mut game) = open_boss_battle() else {
        return;
    };
    let mut trace = RoundTrace {
        v342_min: i32::MAX,
        ..Default::default()
    };
    let mut last350 = i32::MIN;
    let mut settled = false;
    for frame in 0..40000 {
        game.key_bits = if frame % 2 == 0 { key::OK } else { 0 };
        game.frame();
        let v350 = game.vm.vars.get(350);
        if v350 != last350 {
            trace.phase_frames.push((frame, last350, v350));
            trace.v350_seq.push(v350);
            last350 = v350;
        }
        scan_round(&game, frame, &mut trace);
        if game.vm.vars.get(343) == 1 && v350 == 0 {
            trace.settled = true;
            trace.settle_frame = Some(frame);
            trace.v350_at_settle = v350;
            trace.v352_at_settle = game.vm.vars.get(352);
            trace.v343_at_settle = game.vm.vars.get(343);
            trace.v349_at_settle = game.vm.vars.get(349);
            settled = true;
            break;
        }
    }
    if settled {
        for frame in 0..1200 {
            game.key_bits = if frame % 2 == 0 { key::OK } else { 0 };
            game.frame();
            scan_round(&game, 40000 + frame, &mut trace);
        }
        game.key_bits = 0;
    }
    let rounds = game.battle_wiring_rounds();
    let turn_starts = game.battle_wiring_turn_starts();
    let started_after = game.vm.started_events.clone();
    println!(
        "m9_battle_round_gap BOSS: v350={:?} rounds={} turn_starts={} ce241_frames={} ce241_first_f={:?} ce243_first_f={:?} ce243_last_f={:?} ce243_frames={} ce244_frames={} v342_min={} v352_settle={} v343_settle={} v352_now={} v343_now={} settled={} warnings={}",
        trace.v350_seq,
        rounds,
        turn_starts,
        trace.ce241_frames,
        trace.ce241_first_frame,
        trace.ce243_first_frame,
        trace.ce243_frame_list.last(),
        trace.ce243_frames,
        trace.ce244_frames,
        trace.v342_min,
        trace.v352_at_settle,
        trace.v343_at_settle,
        game.vm.vars.get(352),
        game.vm.vars.get(343),
        trace.settled,
        game.stats.warnings
    );
    assert!(settled, "BOSS battle settled");
    assert!(rounds >= 2, "multi-round driver, got {rounds}");
    assert!(
        turn_starts >= 1,
        "real CE241 turn-start fired, got {turn_starts}"
    );
    assert!(trace.v350_seq.contains(&2), "phase 2 entered");
    assert!(trace.ce243_frames > 0, "CE243 enemy action observed");
    assert!(trace.ce241_frames > 0, "CE241 turn-start observed alive");
    assert!(started_after.contains(&243) && started_after.contains(&241));
    assert!(
        trace.v352_at_settle == 1 || trace.v352_at_settle == -1,
        "settle result {}",
        trace.v352_at_settle
    );
    assert_eq!(trace.v343_at_settle, 1);
    assert_eq!(game.stats.warnings, 0);
}
