//! M9 integration task-64: one continuous new-game route from title boot to day 5 08:30.
//!
//! The route is the shipped player path, not synthetic map starts:
//!   boot -> TitleMap event 2 (splash/title) -> CE42 menu [1,2,3] -> option 1
//!        -> op130 Map003 (60,34) -> Map003 ev1 op130 SampleMapA (62,38)
//!        -> SampleMapA opening prompt -> bedroom day 1 18:30
//!        -> natural night menu (hover sister + Decide, CE245) -> CE329 -> next 08:30
//!        -> CE251 daytime command (hover + press, CE252 task) -> dinner -> evening
//!        -> repeated through day 4 -> real CE253 / CE239 battle settlement
//!        -> real dinner -> natural night menu -> day 5 08:30.
//!
//! Real inputs only. The documented injections are:
//!   * the task-19 battle database seed (party stats, command slots, item stock) plus
//!     one cosmetic command-menu header label, exactly as m7_day4_playable does;
//!   * the first-dinner TIPS flag var295 (only if still set) and the ev4 gate var68 are
//!     normalized before the natural night gesture, exactly as m6_night_sleep does.
//! No settlement variable (2000340/2000343/2000350/2000352) is ever injected.
//!
//! Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m9_full_route -- --nocapture

use std::path::PathBuf;

use wolf_game::key;
use wolf_game::Game;

const TITLE_TO_MAP003: &str = "130 Teleport: MapData/Map003.mps at (60, 34) target -1 options 0x1";
const MAP003_TO_SAMPLE: &str =
    "130 Teleport: MapData/SampleMapA.mps at (62, 38) target -1 options 0x1";

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
            eprintln!("skip m9_full_route: {err}");
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
    println!("m9_full_route day {day}: CE251 rows {rows:?}, selecting {label:?} at row {row}");
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

fn finish_day_after_battle(game: &mut Game, day: i32) {
    let mut finished = false;
    for f in 0..4000 {
        game.key_bits = if f % 2 == 0 { key::OK } else { 0 };
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
    game.key_bits = 0;
    assert!(
        finished,
        "day {day}: daytime command never returned after the battle"
    );
    assert_eq!(
        game.stats.warnings, 0,
        "day {day}: warnings after the battle"
    );
    assert_eq!(game.vm.vars.get(160), day);
    assert!(
        game.vm.vars.get(4) >= 16,
        "day {day}: expected the clock past 15:00, got {}:{:02}",
        game.vm.vars.get(4),
        game.vm.vars.get(5)
    );
    let mut dinner_done = false;
    for f in 0..6000 {
        game.key_bits = if f % 2 == 0 { key::OK } else { 0 };
        game.frame();
        let alive300 = game
            .vm
            .instances()
            .iter()
            .any(|instance| instance.event == 300);
        let alive326 = game
            .vm
            .instances()
            .iter()
            .any(|instance| instance.event == 326);
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

fn evname<E: std::fmt::Debug>(event: &E) -> String {
    format!("{event:?}")
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
    v350_seq: Vec<i32>,
    v352_peak: i32,
    v343_peak: i32,
    max_ce239_pc: i32,
    max_ce242_pc: i32,
    ce239_alive_at_end: bool,
    warnings: u64,
    unanswered_250: usize,
}

#[test]
fn the_continuous_new_game_route_reaches_day_five_0830() {
    let Some(mut game) = open_game() else { return };

    // ---- Milestone 1: title boot -> CE42 -> Map003 -> SampleMapA bedroom ----
    game.boot().expect("the boot map has its autorun event");
    for _ in 0..800 {
        game.frame();
    }
    assert!(
        game.screen
            .iter()
            .any(|picture| picture.file.contains("title01")),
        "the boot chain did not reach the title art"
    );
    game.key_bits = key::OK;
    game.frame();
    game.key_bits = 0;
    for _ in 0..240 {
        game.frame();
    }
    let title_menu = game.choices.as_ref().expect("the title menu is open");
    assert_eq!(
        title_menu.options,
        ["1", "2", "3"],
        "the retail title menu options"
    );
    game.set_choice_cursor(0); // option 1 = new game
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
    let mut opening_prompt: Option<Vec<String>> = None;
    for _ in 0..900 {
        game.frame();
        if let Some(choices) = game.choices.as_ref() {
            opening_prompt = Some(choices.options.clone());
            break;
        }
    }
    let expected_opening: Vec<String> = [
        "广阔的战场",
        "广袤的森林",
        "微风吹拂而过的麦田",
        "有人在等我的家",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    assert_eq!(
        opening_prompt,
        Some(expected_opening),
        "SampleMapA opening prompt"
    );
    let diagnostics: Vec<String> = game.diagnostics.keys().cloned().collect();
    assert!(
        diagnostics.iter().any(|line| line == TITLE_TO_MAP003),
        "missing the exact CE42 -> Map003 hop {TITLE_TO_MAP003:?}; got {diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(|line| line == MAP003_TO_SAMPLE),
        "missing the exact Map003 -> SampleMapA hop {MAP003_TO_SAMPLE:?}; got {diagnostics:?}"
    );

    // Answer the real opening prompt (first option) and let the bedroom settle at
    // day 1 18:30. auto_choice + cursor 0 is the documented player input.
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
    assert!(day1, "bedroom day 1 18:30 never settled");
    assert_eq!(game.stats.warnings, 0, "title/new-game route warnings");
    assert!(
        game.screen.len() >= 20,
        "bedroom and HUD drawn: {} pictures",
        game.screen.len()
    );
    println!(
        "m9_full_route milestone 1: title boot -> Map003 -> SampleMapA, day 1 18:30 (frames={}, instructions={})",
        game.stats.frames, game.stats.instructions
    );

    // ---- Milestone 2: natural night menu -> CE329 -> day 2 08:30 ------------
    sleep_naturally_into_next_morning(&mut game, 1);
    assert_eq!(game.vm.vars.get(6), 1, "day 2 weekday = Tuesday");
    println!("m9_full_route milestone 2: natural sleep day 1 -> day 2 08:30");

    // ---- Milestone 3: CE251 study/task day -> natural sleep -> day 3 08:30 ---
    let day2_label = spend_day_choosing_row(&mut game, 2);
    sleep_naturally_into_next_morning(&mut game, 2);
    assert_eq!(game.vm.vars.get(6), 2, "day 3 weekday = Wednesday");
    println!("m9_full_route milestone 3: day 2 CE251 {day2_label:?} -> day 3 08:30");

    // ---- Milestone 4: CE251 study/task day -> natural sleep -> day 4 08:30 ---
    let day3_label = spend_day_choosing_row(&mut game, 3);
    sleep_naturally_into_next_morning(&mut game, 3);
    assert_eq!(game.vm.vars.get(160), 4, "reached day 4");
    assert_eq!(game.vm.vars.get(6), 3, "day 4 weekday = Thursday");
    assert_eq!(game.vm.vars.get(4), 8, "day 4 08:30");
    assert_eq!(game.vm.vars.get(5), 30, "day 4 08:30");
    println!("m9_full_route milestone 4: day 3 CE251 {day3_label:?} -> day 4 08:30");

    // ---- Milestone 5: the real day-4 CE253 / CE239 battle settlement --------
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let seeded = game.seed_battle_databases();
    assert!(
        seeded > 0,
        "seed_battle_databases must seed the battle cells"
    );
    game.databases
        .set_text(wolf_game::db::Container::CData, 5, 16, 0, "┗ 状態リスト");
    let pre_power = game.vm.vars.get(28);
    let _ = game.start_engine_events();
    game.set_battle_wiring(true);
    // opcode-111 reference semantics: enemy gauge now spans multiple command rounds;
    // enable the round loop so the day-4 battle can settle.
    game.set_battle_round_loop(true);
    assert_eq!(
        game.vm.vars.get(340),
        0,
        "pre-battle 2000340 idle (no injection)"
    );
    assert_eq!(
        game.vm.vars.get(350),
        0,
        "pre-battle 2000350 idle (no injection)"
    );
    assert_eq!(
        game.vm.vars.get(352),
        0,
        "pre-battle 2000352 zero (no injection)"
    );
    assert_eq!(
        game.vm.vars.get(343),
        0,
        "pre-battle 2000343 zero (no injection)"
    );

    let mut trace = BattleTrace::default();
    let mut last350 = i32::MIN;
    let mut settled = false;
    for frame in 0..40000 {
        game.key_bits = if frame % 2 == 0 { key::OK } else { 0 };
        game.frame();
        let v340 = game.vm.vars.get(340);
        let v350 = game.vm.vars.get(350);
        let v352 = game.vm.vars.get(352);
        let v343 = game.vm.vars.get(343);
        if v340 != 0 && trace.battle_active_frame.is_none() {
            trace.battle_active_frame = Some(frame);
        }
        if v350 != last350 {
            trace.v350_seq.push(v350);
            last350 = v350;
        }
        if v352.abs() > trace.v352_peak.abs() {
            trace.v352_peak = v352;
        }
        trace.v343_peak = trace.v343_peak.max(v343);
        trace.wiring_rounds = game.battle_wiring_rounds();
        for instance in game.vm.instances() {
            match evname(&instance.event).as_str() {
                "253" => trace.ce253_started = true,
                "239" => {
                    trace.ce239_started = true;
                    trace.max_ce239_pc = trace.max_ce239_pc.max(instance.pc as i32);
                }
                "242" => {
                    trace.ce242_started = true;
                    trace.max_ce242_pc = trace.max_ce242_pc.max(instance.pc as i32);
                }
                "243" => trace.ce243_started = true,
                "241" => trace.ce241_started = true,
                _ => {}
            }
        }
        if v343 == 1 && v350 == 0 {
            settled = true;
            break;
        }
    }
    if settled {
        for frame in 0..1600 {
            game.key_bits = if frame % 2 == 0 { key::OK } else { 0 };
            game.frame();
            if game.vm.vars.get(352).abs() > trace.v352_peak.abs() {
                trace.v352_peak = game.vm.vars.get(352);
            }
            trace.v343_peak = trace.v343_peak.max(game.vm.vars.get(343));
            for instance in game.vm.instances() {
                if evname(&instance.event) == "242" {
                    trace.max_ce242_pc = trace.max_ce242_pc.max(instance.pc as i32);
                }
            }
            if !has_event(&game, 239) {
                break;
            }
        }
        game.key_bits = 0;
    }
    trace.ce239_alive_at_end = has_event(&game, 239);
    trace.warnings = game.stats.warnings;
    let unanswered: Vec<String> = game
        .diagnostics
        .iter()
        .filter(|(message, _)| message.contains("unanswered"))
        .map(|(message, count)| format!("x{count}: {message}"))
        .collect();
    trace.unanswered_250 = unanswered.len();
    for message in &unanswered {
        println!("m9_full_route UNANSWERED250: {message}");
    }
    println!(
        "m9_full_route milestone 5 battle: ce253={} ce239={} ce242={} ce243={} ce241={} rounds={} active_f={:?} v350={:?} v352_peak={} v343_peak={} max239pc={} max242pc={} alive={} warnings={} unanswered250={}",
        trace.ce253_started,
        trace.ce239_started,
        trace.ce242_started,
        trace.ce243_started,
        trace.ce241_started,
        trace.wiring_rounds,
        trace.battle_active_frame,
        trace.v350_seq,
        trace.v352_peak,
        trace.v343_peak,
        trace.max_ce239_pc,
        trace.max_ce242_pc,
        trace.ce239_alive_at_end,
        trace.warnings,
        trace.unanswered_250
    );
    assert!(trace.ce253_started, "day 4 must dispatch CE253");
    assert!(trace.ce239_started, "CE253 must call the real battle CE239");
    assert!(
        trace.max_ce239_pc >= 700,
        "CE239 must reach battle staging, max pc {}",
        trace.max_ce239_pc
    );
    assert!(trace.battle_active_frame.is_some(), "2000340 must rise");
    assert!(
        trace.wiring_rounds >= 1,
        "the rebuilt wiring must start CE242"
    );
    assert!(trace.ce242_started, "CE242 command phase must run");
    // opcode-111 reference semantics raises the party power to 201 (var28), which
    // crosses the CE253 >=200 threshold and selects the HIGH (harder) branch; the
    // low-branch instakill was a literal-operand artifact. This branch fights a
    // genuine multi-round battle that the fresh party loses (2000352 = -1).
    assert!(
        pre_power >= 200,
        "party power {pre_power} takes the CE253 HIGH branch"
    );
    assert!(
        trace.max_ce242_pc >= 200,
        "CE242 must reach the CE244 call region, max pc {}",
        trace.max_ce242_pc
    );
    assert!(settled, "the day-4 round must settle");
    assert!(
        trace.v350_seq.first() == Some(&0) && trace.v350_seq.contains(&1),
        "2000350 starts idle and enters the command phase, got {:?}",
        trace.v350_seq
    );
    assert!(
        trace.v352_peak == 1 || trace.v352_peak == -1,
        "CE244 must settle: 2000352 in {{1,-1}}, got {}",
        trace.v352_peak
    );
    assert_eq!(trace.v343_peak, 1, "CE244 must set 2000343 = 1");
    assert!(
        !trace.ce239_alive_at_end,
        "CE239 must return after settlement"
    );
    assert_eq!(trace.warnings, 0, "no warnings through the day-4 battle");
    assert!(
        trace.unanswered_250 <= 1,
        "at most the cosmetic menu label read is unanswered"
    );
    for message in &unanswered {
        assert!(
            message.contains("CData (5, 16, 0)"),
            "unexpected unanswered read: {message}"
        );
    }

    // ---- Milestone 6: post-battle dinner -> natural night -> day 5 08:30 ----
    finish_day_after_battle(&mut game, 4);
    sleep_naturally_into_next_morning(&mut game, 4);
    assert_eq!(game.vm.vars.get(340), 0, "post-battle 2000340 idle again");
    assert_eq!(
        game.vm.vars.get(343),
        0,
        "CE239 clears 2000343 when it returns"
    );
    assert_eq!(game.vm.vars.get(160), 5, "day 5 counter");
    assert_eq!(game.vm.vars.get(6), 4, "day 5 is Friday");
    assert_eq!(game.vm.vars.get(4), 8, "day 5 08:30");
    assert_eq!(game.vm.vars.get(5), 30, "day 5 08:30");
    assert_eq!(game.stats.warnings, 0, "the whole route is warning-free");
    println!(
        "m9_full_route PASS: title -> Map003 -> SampleMapA -> days 1..4 -> real CE253/CE239 settlement (2000352={}, 2000343=1) -> day 5 08:30; warnings=0, frames={}, instructions={}",
        trace.v352_peak, game.stats.frames, game.stats.instructions
    );
}
