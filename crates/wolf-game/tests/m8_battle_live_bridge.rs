//! M9 task-63: bridge the live CE65 battle menu selection into the rebuilt battle driver.
//!
//! The player-facing battle command line (CE188 -> CE191 -> CE192 -> CE152 -> CE65 ->
//! CE157) really accepts input and registers the chosen command in the physical CData
//! table labelled [x コマンド処理リスト] (runtime type 15 -> physical type 14):
//!   col0 = 発動者番号, col1 = 発動敏捷性, col2 = 受け側番号, col3 = 発動技能.
//!
//! The rebuilt driver (src/battle_wiring.rs) starts CE242 with a command index 1..8 in
//! self-var 600 selecting which CE242 action body (= which party member / action slot)
//! runs. CData type 14 row == that command index (1-based).
//!
//! Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_battle_live_bridge -- --nocapture

use std::path::PathBuf;

use wolf_game::db::Container;
use wolf_game::Game;

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        return p;
    }
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop();
    b.pop();
    b.join(p)
}

fn save01() -> PathBuf {
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop();
    b.pop();
    b.join("work/retail_backup/Save/SaveData01.sav")
}

fn event_id<E: std::fmt::Debug>(e: &E) -> String {
    format!("{:?}", e)
}

const NON_DEFAULT_CODE: i32 = 30;
const SELECTABLE_CODES: [i32; 4] = [26, 25, 30, 29];
const VAR_ACTIVE: usize = 340;
const VAR_PHASE: usize = 350;
const VAR_DECIDED: usize = 343;
const VAR_RESULT: usize = 352;
const MEMBER_POOLS: [usize; 4] = [345, 346, 347, 348];

struct BattleRun {
    pre_pools: Vec<i32>,
    post_pools: Vec<i32>,
    ce242: bool,
    max_ce242_pc: usize,
    v350_seq: Vec<i32>,
    v352: i32,
    v343: i32,
    warnings: u64,
    settled: bool,
}

fn run_battle_with_schedule(schedule: Vec<i32>) -> BattleRun {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    // Read-load fix removes the old spurious 506 coroutine; pin this fixture's event RNG
    // so its intended deterministic sequence is preserved.
    game.vm.rng.set_event_seed(2);
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(game.seed_battle_databases() > 0, "battle cells must seed");
    game.databases.set_text(Container::CData, 5, 16, 0, "state list");
    let _ = game.start_engine_events();
    game.set_battle_wiring(true);
    game.set_battle_command_schedule(schedule);
    let _ = game.start_event(253);

    let mut ce242 = false;
    let mut max_ce242_pc = 0usize;
    let mut v350_seq: Vec<i32> = Vec::new();
    let mut last_phase = game.vm.vars.get(VAR_PHASE);
    let mut post_pools: Vec<i32> = MEMBER_POOLS.iter().map(|&i| game.vm.vars.get(i)).collect();
    let mut pre_pools: Vec<i32> = post_pools.clone();
    let mut settled = false;
    let mut saw_battle = false;

    for f in 0..20000 {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();
        for inst in game.vm.instances() {
            if event_id(&inst.event) == "242" {
                ce242 = true;
                if inst.pc > max_ce242_pc {
                    max_ce242_pc = inst.pc;
                }
            }
        }
        let phase = game.vm.vars.get(VAR_PHASE);
        if phase != last_phase {
            v350_seq.push(phase);
            last_phase = phase;
        }
        if !saw_battle && game.vm.vars.get(VAR_ACTIVE) != 0 {
            saw_battle = true;
            pre_pools = MEMBER_POOLS.iter().map(|&i| game.vm.vars.get(i)).collect();
        }
        if game.vm.vars.get(VAR_DECIDED) == 1 && game.vm.vars.get(VAR_PHASE) == 0 {
            settled = true;
            post_pools = MEMBER_POOLS.iter().map(|&i| game.vm.vars.get(i)).collect();
            break;
        }
    }
    if !settled {
        post_pools = MEMBER_POOLS.iter().map(|&i| game.vm.vars.get(i)).collect();
    }

    BattleRun {
        pre_pools,
        post_pools,
        ce242,
        max_ce242_pc,
        v350_seq,
        v352: game.vm.vars.get(VAR_RESULT),
        v343: game.vm.vars.get(VAR_DECIDED),
        warnings: game.stats.warnings,
        settled,
    }
}

fn drive_real_menu_to_code(target_code: i32) -> (Option<i32>, i32, i32, i32, bool, i32, u64) {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    // Read-load fix removes the old spurious 506 coroutine; pin this fixture's event RNG
    // so its intended deterministic sequence is preserved.
    game.vm.rng.set_event_seed(2);
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(game.seed_battle_databases() > 0, "battle cells must seed");
    let _ = game.start_event(188);

    let mut registered_row: Option<i32> = None;
    let mut registered_code = 0i32;
    let mut ce242_seen = false;
    let mut v350 = 0i32;

    for f in 0..500 {
        let mut cursor: Option<i32> = None;
        for inst in game.vm.instances() {
            match event_id(&inst.event).as_str() {
                "65" => cursor = Some(inst.self_vars.get(611)),
                "242" => ce242_seen = true,
                _ => {}
            }
        }
        let mut target_row: Option<i32> = None;
        for row in 0..8 {
            let code = game.databases.num(Container::CData, 19, row, 1).unwrap_or(-1);
            let selectable = game.databases.num(Container::CData, 19, row, 2).unwrap_or(0);
            if SELECTABLE_CODES.contains(&code) && selectable == 1 && code == target_code {
                target_row = Some(row);
            }
        }
        game.key_bits = match cursor {
            Some(cur) => match target_row {
                Some(t) if cur < t => wolf_game::key::DOWN,
                Some(t) if cur > t => wolf_game::key::UP,
                Some(_) => wolf_game::key::OK,
                None => wolf_game::key::OK,
            },
            None => {
                if f % 2 == 0 {
                    wolf_game::key::OK
                } else {
                    0
                }
            }
        };
        let _ = game.frame();
        v350 = game.vm.vars.get(VAR_PHASE);
        for row in 0..16 {
            if game.databases.num(Container::CData, 14, row, 3) == Some(target_code) {
                registered_row = Some(row);
                registered_code = target_code;
            }
        }
        if registered_row.is_some() {
            break;
        }
    }

    let activator = registered_row
        .and_then(|r| game.databases.num(Container::CData, 14, r, 0))
        .unwrap_or(-999);
    let input_pos = game.databases.num(Container::CData, 20, 73, 0).unwrap_or(-999);
    (registered_row, registered_code, activator, input_pos, ce242_seen, v350, game.stats.warnings)
}

#[test]
fn live_menu_selection_feeds_the_driver_and_settles() {
    let (registered_row, registered_code, activator, input_pos, ce242_in_menu, v350_menu, warn_menu) =
        drive_real_menu_to_code(NON_DEFAULT_CODE);

    eprintln!(
        "menu: registered_row={registered_row:?} code={registered_code} activator={activator} input_pos={input_pos} ce242={ce242_in_menu} v350={v350_menu} warnings={warn_menu}"
    );

    let row = registered_row.expect("the real menu must register the chosen command");
    assert_eq!(row, 1, "the shipped single-input menu registers row1 (command index 1)");
    assert_eq!(registered_code, NON_DEFAULT_CODE, "registered 発動技能 code");
    assert_eq!(activator, 0, "発動者番号 col0 is 0 for the first input");
    assert_eq!(input_pos, 0, "CData type20 row73 コマンド入力位置 stays 0");
    assert!(!ce242_in_menu, "the CE188 menu line never starts CE242");
    assert_eq!(v350_menu, 0, "command phase stays idle in the menu line");
    assert_eq!(warn_menu, 0, "no warnings in the menu line");
    assert!(
        registered_code > 8,
        "the command CODE lives outside the CE242 index domain 1..8; bridge it via the ROW, not the code"
    );

    let run = run_battle_with_schedule(vec![row]);
    eprintln!(
        "bridge: ce242={} max_ce242_pc={} v350_seq={:?} pre={:?} post={:?} v352={} v343={} settled={} warnings={}",
        run.ce242, run.max_ce242_pc, run.v350_seq, run.pre_pools, run.post_pools, run.v352, run.v343, run.settled, run.warnings
    );

    assert!(run.ce242, "the bridge must start CE242 from the battle-active edge");
    assert!(run.max_ce242_pc >= 200, "CE242 must reach its action body, got pc {}", run.max_ce242_pc);
    assert!(
        run.post_pools[0] < run.pre_pools[0],
        "command index {} must drain pool 2000345: pre {:?} post {:?}",
        row, run.pre_pools, run.post_pools
    );
    assert!(run.v352 == 1 || run.v352 == -1, "CE244 must settle 2000352 in {{1,-1}}, got {}", run.v352);
    assert_eq!(run.v343, 1, "CE244 must set 2000343 = 1");
    assert!(run.settled, "the battle must settle");
    assert_eq!(run.warnings, 0, "no engine warnings allowed");
}

#[test]
fn driver_honours_nondefault_index_but_the_menu_cannot_supply_it() {
    let run = run_battle_with_schedule(vec![3]);
    eprintln!(
        "index3: ce242={} max_ce242_pc={} v350_seq={:?} pre={:?} post={:?} v352={} v343={} warnings={}",
        run.ce242, run.max_ce242_pc, run.v350_seq, run.pre_pools, run.post_pools, run.v352, run.v343, run.warnings
    );

    assert!(run.ce242, "CE242 must start");
    assert!(run.max_ce242_pc >= 761, "index 3 must dispatch to the slot-3 CE242 body (>= pc761), got pc {}", run.max_ce242_pc);
    assert!(
        run.post_pools[1] < run.pre_pools[1],
        "command index 3 must drain pool 2000346: pre {:?} post {:?}",
        run.pre_pools, run.post_pools
    );
    assert!(run.v352 == 1 || run.v352 == -1, "battle must still settle, got v352={}", run.v352);
    assert_eq!(run.v343, 1);
    assert_eq!(run.warnings, 0);

    let (row, _code, _act, input_pos, _c, _v, _w) = drive_real_menu_to_code(NON_DEFAULT_CODE);
    assert_eq!(row, Some(1), "the real menu can only register row 1, i.e. command index 1");
    assert_eq!(input_pos, 0, "コマンド入力位置 stays 0: no shipped multi-input path to index > 1 (documented gap)");
}
