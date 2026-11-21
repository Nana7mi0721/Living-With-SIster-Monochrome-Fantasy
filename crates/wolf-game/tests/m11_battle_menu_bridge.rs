//! M11 task-85: the real per-member battle command menu -> CE242 command-index bridge.
//!
//! ## The gap (recap of M9 tasks 63/67)
//! The player-facing battle menu line CE188 -> CE191 -> CE192 -> CE152 -> CE65 -> CE157
//! registers the chosen command into the command list (physical CData type14). The rebuilt
//! driver (crates/wolf-game/src/battle_wiring.rs) does NOT read that table; it consumes a
//! successive sequence of command indexes 1..8 (self-var 600 of CE242), where the command
//! index equals the registered command ROW and each index dispatches a distinct CE242 action
//! body draining a distinct party pool (2000345/346/347/348).
//!
//! Task-67 pinned the blocker: the CE157 producer writes command row 1 but never advances
//! コマンド入力位置 (CData 20,73). Its cursor advance (pc124-125) sits behind the pc121 gate
//! self1600023 == 0, but the live path sets self1600023 = 100 (pc93/pc100), so the advance is
//! skipped and every selection overwrites row 1. CE194 (行動順の計算) loops コマンド入力位置
//! times, i.e. 0, so the CE188 engine executes zero commands. The live menu therefore confirms
//! exactly ONE command (row 1 / index 1).
//!
//! ## What this test proves
//!  A) real_menu_registers_only_row_one_and_never_advances_input_position
//!     Driven with real keys through CE188, the shipped menu registers a selectable ability
//!     code into CData type14 row 1 only, コマンド入力位置 stays 0, CE242 never runs in the menu
//!     line, and no warnings are raised. This is the reproducible blocker: the producer cannot
//!     hand the bridge a >1 index sequence.
//!  B) successive_command_indexes_run_distinct_ce242_bodies_and_settle
//!     The engine half already accepts a successive command-index sequence
//!     (Game::set_battle_command_schedule + set_battle_round_loop). On the real BOSS battle
//!     (CE253 -> CE239, gauge survives round 1) a schedule [1, 3] runs >= 2 real CE242 rounds,
//!     reaches two DISTINCT CE242 action bodies (index 1 -> pc<=474, index 3 -> pc>=761), drains
//!     two distinct member pools, chains the REAL CE241 turn-start (2000350 2 -> 0) between
//!     rounds, and the real CE239 battle settles with 2000352 in {1,-1}, 2000343 = 1, warnings 0.
//!
//! Together they bound the gap: the engine consumes a real successive sequence faithfully, and
//! the only missing piece is the CE157 producer cursor advance (data/VM layer, out of this file's
//! write scope). See docs/research/M11_BATTLE_MENU_BRIDGE.md.
//!
//! Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_battle_menu_bridge -- --nocapture

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

const SELECTABLE_CODES: [i32; 4] = [26, 25, 30, 29];
const TARGET_CODE: i32 = 30; // 強斬 -- a real selectable, non-default ability code
const BOSS_GAUGE: i32 = 4000;

fn pools(game: &Game) -> [i32; 4] {
    [
        game.vm.vars.get(345),
        game.vm.vars.get(346),
        game.vm.vars.get(347),
        game.vm.vars.get(348),
    ]
}

// ---------------------------------------------------------------------------------------------
// A) Real menu blocker: only row 1 / command index 1 is ever registered.
// ---------------------------------------------------------------------------------------------
#[test]
fn real_menu_registers_only_row_one_and_never_advances_input_position() {
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

    let mut registered_rows: Vec<i32> = Vec::new();
    let mut ce242_in_menu = false;
    let mut v350 = 0i32;

    for f in 0..900 {
        // Navigate the CE65 cursor to the row holding TARGET_CODE, then confirm (real keys).
        let mut cursor: Option<i32> = None;
        for inst in game.vm.instances() {
            if inst.event == 65 {
                cursor = Some(inst.self_vars.get(611));
            }
            if inst.event == 242 {
                ce242_in_menu = true;
            }
        }
        let mut target_row: Option<i32> = None;
        for row in 0..8 {
            let code = game
                .databases
                .num(Container::CData, 19, row, 1)
                .unwrap_or(-1);
            let selectable = game
                .databases
                .num(Container::CData, 19, row, 2)
                .unwrap_or(0);
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
        v350 = game.vm.vars.get(350);
        // Collect every command-list row that now holds the target code.
        registered_rows.clear();
        for row in 0..16 {
            if game.databases.num(Container::CData, 14, row, 3) == Some(TARGET_CODE) {
                registered_rows.push(row);
            }
        }
        if !registered_rows.is_empty() {
            break;
        }
    }

    let input_pos = game
        .databases
        .num(Container::CData, 20, 73, 0)
        .unwrap_or(-999);
    eprintln!(
        "menu: registered_rows={:?} input_pos={} ce242={} v350={} warnings={}",
        registered_rows, input_pos, ce242_in_menu, v350, game.stats.warnings
    );

    assert_eq!(
        registered_rows,
        vec![1],
        "the shipped menu registers the command into row 1 only (command index 1)"
    );
    assert_eq!(
        input_pos, 0,
        "コマンド入力位置 stays 0: the CE157 producer never advances its cursor"
    );
    assert!(!ce242_in_menu, "the CE188 menu line never starts CE242");
    assert_eq!(v350, 0, "command phase stays idle in the menu line");
    assert_eq!(game.stats.warnings, 0, "no warnings in the menu line");
}

// ---------------------------------------------------------------------------------------------
// B) Engine half: a successive command-index sequence drives distinct CE242 bodies/pools and
//    the real CE239 battle settles.
// ---------------------------------------------------------------------------------------------
#[test]
fn successive_command_indexes_run_distinct_ce242_bodies_and_settle() {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    // Read-load fix removes the old spurious 506 coroutine; pin this fixture's event RNG
    // so its intended deterministic sequence is preserved.
    game.vm.rng.set_event_seed(2);
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(game.seed_battle_databases() > 0, "seed battle cells");
    game.databases
        .set_text(Container::CData, 5, 16, 0, "┗ 状態リスト");
    let _ = game.start_engine_events();
    game.set_battle_wiring(true);
    game.set_battle_round_loop(true);
    // A successive sequence of command indexes (the shape the menu bridge delivers): member 1
    // attack (index 1) then Kana attack (index 3) -- two distinct bodies / two distinct pools.
    game.set_battle_command_schedule(vec![1, 3]);
    // BOSS context so the enemy gauge survives round 1 (CE239 pc202/204).
    game.vm.vars.set(445, 1);
    game.vm.vars.set(446, BOSS_GAUGE);
    let _ = game.start_event(253);

    let mut v350_seq: Vec<i32> = Vec::new();
    let mut last350 = i32::MIN;
    let mut pre = [0i32; 4];
    let mut have_pre = false;
    let mut settled = false;
    let mut v352 = 0;
    let mut v343 = 0;

    // Per-CE242-instance max pc, to prove the different command indexes reach different bodies.
    let mut cur_round_id: Option<u32> = None;
    let mut cur_round_max = 0i32;
    let mut round_body_max: Vec<i32> = Vec::new();

    for f in 0..40000usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();

        if !have_pre && game.vm.vars.get(340) != 0 {
            pre = pools(&game);
            have_pre = true;
        }
        // Track the live CE242 instance: a new id starts a new command round.
        let mut alive_242: Option<(u32, i32)> = None;
        for inst in game.vm.instances() {
            if inst.event == 242 {
                let pc = inst.pc as i32;
                alive_242 = Some(match alive_242 {
                    Some((id, m)) if id == inst.id => (id, m.max(pc)),
                    Some(_) => (inst.id, pc),
                    None => (inst.id, pc),
                });
            }
        }
        if let Some((id, pc)) = alive_242 {
            match cur_round_id {
                Some(prev) if prev != id => {
                    round_body_max.push(cur_round_max);
                    cur_round_id = Some(id);
                    cur_round_max = pc;
                }
                None => {
                    cur_round_id = Some(id);
                    cur_round_max = pc;
                }
                _ => {
                    cur_round_max = cur_round_max.max(pc);
                }
            }
        }

        let v350 = game.vm.vars.get(350);
        if v350 != last350 {
            v350_seq.push(v350);
            last350 = v350;
        }
        v352 = game.vm.vars.get(352);
        v343 = game.vm.vars.get(343);
        if v343 == 1 && v350 == 0 {
            settled = true;
            break;
        }
    }
    if cur_round_id.is_some() {
        round_body_max.push(cur_round_max);
    }
    // Drain the settlement tail.
    for f in 0..1200usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();
    }

    let rounds = game.battle_wiring_rounds();
    let turn_starts = game.battle_wiring_turn_starts();
    let post = pools(&game);
    eprintln!(
        "engine-bridge: v350_seq={:?} pre={:?} post={:?} rounds={} turn_starts={} round_body_max={:?} v352={} v343={} settled={} warnings={}",
        v350_seq, pre, post, rounds, turn_starts, round_body_max, v352, v343, settled, game.stats.warnings
    );

    assert!(have_pre, "battle must become active");
    assert!(settled, "the real BOSS battle must settle");
    assert!(
        rounds >= 2,
        "must run >= 2 real CE242 command rounds, got {}",
        rounds
    );
    assert!(
        turn_starts >= 1,
        "the REAL CE241 turn-start must fire between rounds, got {}",
        turn_starts
    );
    assert!(
        v350_seq.windows(2).any(|w| w[0] == 2 && w[1] == 0),
        "phase must be reset 2 -> 0 by CE241, seq={:?}",
        v350_seq
    );
    assert_eq!(
        pre,
        [5, 4, 3, 2],
        "CE239 must seed the four pools 5/4/3/2, got {:?}",
        pre
    );
    // Distinct CE242 bodies: index 1 body ends <= pc474, index 3 body runs to >= pc761.
    let has_low_body = round_body_max.iter().any(|&p| p > 200 && p <= 600);
    let has_high_body = round_body_max.iter().any(|&p| p >= 761);
    assert!(
        has_low_body && has_high_body,
        "successive indexes must reach two distinct CE242 bodies (low<=600 and high>=761), got {:?}", round_body_max
    );
    // Distinct member pools: the two bodies drain different pools.
    let drained: Vec<usize> = (0..4).filter(|&i| post[i] < pre[i]).collect();
    assert!(
        drained.len() >= 2,
        ">= 2 distinct member pools must be drained, pre={:?} post={:?}",
        pre,
        post
    );
    assert_eq!(v343, 1, "battle must be decided");
    assert!(
        v352 == 1 || v352 == -1,
        "real CE244 result in {{1,-1}}, got {}",
        v352
    );
    assert_eq!(game.stats.warnings, 0, "no engine warnings");
}
