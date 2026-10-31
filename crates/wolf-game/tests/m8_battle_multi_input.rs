//! M9 task-67: does the shipped CE188 command-menu line register more than one command?
//!
//! The live battle command line is CE188 -> CE191 (戦闘メンバー選択処理, an internal loop
//! over members) -> CE192 (味方コマンド選択画面) -> CE152 -> CE65 -> CE157 (コマンド登録).
//! CE157 writes the chosen action into the command list (logical type 15 / physical type 14)
//! and is the ONLY writer of the cursor CData(20,73,0) "コマンド入力位置".
//!
//! The multi-command CONSUMER already exists: CE194 (行動順の計算) reads
//! CData(20,73,0) into self1600030 (pc7) and loops that many times (pc13 c179) over the
//! command rows, i.e. the number of registered commands == コマンド入力位置. CE195 then
//! executes the sorted actions.
//!
//! Verdict proven here (static + runtime): the PRODUCER never advances コマンド入力位置.
//! Every selection overwrites command row 1 and the cursor stays 0 forever, so CE194 would
//! loop zero times and the CE188 engine would execute zero commands (it is dead in the
//! shipped graph). The live menu registers exactly ONE command (row1 / command index 1).
//! See docs/research/M9_BATTLE_MULTI_INPUT.md for the CE157 branch/gate analysis and the
//! minimal (non-hardcoded) fix proposal.
//!
//! Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_battle_multi_input -- --nocapture

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

fn open_game() -> Game {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(game.seed_battle_databases() > 0, "battle cells must seed");
    game
}

const TARGET_CODE: i32 = 30; // 強斬 — a selectable, non-default ability code
const SELECTABLE_CODES: [i32; 4] = [26, 25, 30, 29];

struct MenuObserve {
    rows_with_selectable: Vec<i32>,
    registered_row: Option<i32>,
    registered_code: i32,
    max_input_pos: i32,
    final_input_pos: i32,
    ce188: bool,
    ce191: bool,
    ce192: bool,
    ce194: bool,
    ce242: bool,
    warnings: u64,
}

// Drive the real CE188 command-menu line with real keys, selecting TARGET_CODE, and keep
// running well past the first registration so any later multi-row registration would show.
fn drive_ce188_menu() -> MenuObserve {
    let mut game = open_game();
    let _ = game.start_event(188);

    let mut rows_with_selectable: Vec<i32> = Vec::new();
    let mut registered_row: Option<i32> = None;
    let mut registered_code = 0i32;
    let mut max_input_pos = 0i32;
    let mut final_input_pos = 0i32;
    let (mut ce188, mut ce191, mut ce192, mut ce194, mut ce242) =
        (false, false, false, false, false);

    for f in 0..2400 {
        // Track which events are alive.
        let mut cursor: Option<i32> = None;
        for inst in game.vm.instances() {
            match event_id(&inst.event).as_str() {
                "65" => cursor = Some(inst.self_vars.get(611)),
                "188" => ce188 = true,
                "191" => ce191 = true,
                "192" => ce192 = true,
                "194" => ce194 = true,
                "242" => ce242 = true,
                _ => {}
            }
        }

        // Navigate the CE65 cursor to the row holding TARGET_CODE, then confirm.
        let mut target_row: Option<i32> = None;
        for row in 0..8 {
            let code = game.databases.num(Container::CData, 19, row, 1).unwrap_or(-1);
            let selectable = game.databases.num(Container::CData, 19, row, 2).unwrap_or(0);
            if SELECTABLE_CODES.contains(&code) && selectable == 1 && code == TARGET_CODE {
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

        // Record every command row that ever carries a real ability code.
        for row in 0..16 {
            let code = game.databases.num(Container::CData, 14, row, 3).unwrap_or(-1);
            if SELECTABLE_CODES.contains(&code) && !rows_with_selectable.contains(&row) {
                rows_with_selectable.push(row);
            }
            if code == TARGET_CODE {
                registered_row = Some(row);
                registered_code = TARGET_CODE;
            }
        }
        let pos = game.databases.num(Container::CData, 20, 73, 0).unwrap_or(0);
        if pos > max_input_pos {
            max_input_pos = pos;
        }
        final_input_pos = pos;
    }

    MenuObserve {
        rows_with_selectable,
        registered_row,
        registered_code,
        max_input_pos,
        final_input_pos,
        ce188,
        ce191,
        ce192,
        ce194,
        ce242,
        warnings: game.stats.warnings,
    }
}

#[test]
fn live_menu_registers_one_command_and_never_advances_input_position() {
    let obs = drive_ce188_menu();
    eprintln!(
        "menu: rows_with_selectable={:?} registered_row={:?} code={} max_input_pos={} final_input_pos={} ce188={} ce191={} ce192={} ce194={} ce242={} warnings={}",
        obs.rows_with_selectable,
        obs.registered_row,
        obs.registered_code,
        obs.max_input_pos,
        obs.final_input_pos,
        obs.ce188,
        obs.ce191,
        obs.ce192,
        obs.ce194,
        obs.ce242,
        obs.warnings
    );

    // The whole CE188 command-menu line runs.
    assert!(obs.ce188, "CE188 (戦闘処理) must run");
    assert!(obs.ce191, "CE191 (メンバー選択) must run");
    assert!(obs.ce192, "CE192 (味方コマンド選択画面) must run");

    // A real selection registers a command.
    assert!(
        obs.registered_row.is_some(),
        "the real menu must register the chosen command"
    );
    assert_eq!(obs.registered_code, TARGET_CODE, "registered 発動技能 code");

    // ...but ONLY in command row 1 and the cursor never moves.
    assert_eq!(
        obs.rows_with_selectable,
        vec![1],
        "every selection overwrites command row 1; no path writes rows 2..N"
    );
    assert_eq!(
        obs.registered_row,
        Some(1),
        "the shipped single-input menu registers row1 = command index 1"
    );
    assert_eq!(
        obs.max_input_pos, 0,
        "コマンド入力位置 (CData 20,73) never leaves 0 across 2400 frames"
    );
    assert_eq!(obs.final_input_pos, 0, "コマンド入力位置 stays 0 at the end");

    // CE194 is the multi-command consumer; it is never reached by the menu line.
    assert!(
        !obs.ce194,
        "CE194 (行動順の計算, the multi-command consumer) is never started by the menu line"
    );
    assert!(!obs.ce242, "the menu line never starts CE242");
    assert_eq!(obs.warnings, 0, "no engine warnings in the menu line");
}

#[test]
fn ce157_registers_row_one_but_never_advances_input_position() {
    // Pin the exact missing writer. CE157 'コマンド登録' is invoked with self1600002 = 0
    // (the real-registration branch that CE192 pc208 uses). It writes command row 1 but the
    // cursor-advance at pc124-125 is gated behind self1600023 == 0 (false in the live path),
    // so コマンド入力位置 (CData 20,73) stays 0.
    let mut game = open_game();
    let id = game.vm.start_event(157).expect("CE157 must start");
    // Common-event self-vars: magic self16000NN maps to physical index 600 + NN.
    {
        let inst = game.vm.instance_mut(id).expect("CE157 instance");
        inst.self_vars.set(600, 1); // self1600000 = 発動者番号
        inst.self_vars.set(601, 26); // self1600001 = 発動技能 code
        inst.self_vars.set(602, 0); // self1600002 = 0 -> real registration branch
    }

    let mut max_pc = 0usize;
    for _ in 0..120 {
        let _ = game.frame();
        for inst in game.vm.instances() {
            if event_id(&inst.event) == "157" && inst.pc > max_pc {
                max_pc = inst.pc;
            }
        }
    }

    let row1_code = game.databases.num(Container::CData, 14, 1, 3).unwrap_or(-1);
    let row2_code = game.databases.num(Container::CData, 14, 2, 3).unwrap_or(-1);
    let input_pos = game.databases.num(Container::CData, 20, 73, 0).unwrap_or(-1);
    eprintln!(
        "ce157: max_pc={max_pc} row1_code={row1_code} row2_code={row2_code} input_pos={input_pos} warnings={}",
        game.stats.warnings
    );

    assert!(max_pc >= 82, "CE157 must execute its registration body, got pc {max_pc}");
    assert_eq!(row1_code, 26, "CE157 writes the command code into row 1 col3");
    assert_ne!(row2_code, 26, "CE157 never writes a second command row");
    assert_eq!(
        input_pos, 0,
        "コマンド入力位置 stays 0: the cursor-advance writer (pc124-125) never fires"
    );
    assert_eq!(game.stats.warnings, 0, "no engine warnings");
}
