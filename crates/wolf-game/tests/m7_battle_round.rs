//! M7 W1-a (task-15): attempt a REAL battle round through the genuine event graph
//! and pin exactly where CE244 settlement becomes unreachable.
//!
//! Two real, engine-driven scenarios (no fabricated window rows, no legacy CE242/CE243
//! seeding):
//!
//!   A. CE188 (X◆戦闘処理) driven directly with documented data preconditions
//!      (party/command slots + stock of item 0). The real line boots all the way:
//!      CE189 初期化 -> CE190 配置 -> CE143 主人公DB→戦闘DBコピー (type10 row0 HP/maxHP
//!      copied) -> CE191 行動内容 -> CE192 味方コマンド選択 -> CE152 戦闘コマンド一覧
//!      -> a REAL attack row (code 26/25/30/29) becomes selectable -> CE157/CE158 target
//!      windows open. But CE188 never calls CE244 (settlement), so no amount of input
//!      produces 2000352/2000343 from this line.
//!
//!   B. CE239 (■戦闘呼び出し) driven in a full session (start_engine_events, which
//!      auto-starts all ENGINE_ALWAYS events and the 2000350-gated CE242/CE243). CE239
//!      reaches its battle-active case (2000340 = 1, 2000349 = 1) but the three-state
//!      command machine never leaves idle: 2000350 stays 0 the whole run, so CE242 and
//!      CE243 never start, and CE244 (the SOLE writer of 2000352/2000343) is never called.
//!      The bootstrap is circular: only CE242 pc14 writes 2000350 = 1, and CE242 only
//!      runs when 2000350 == 1.
//!
//! See docs/research/M7_BATTLE_ROUND.md for the full command/variable evidence.

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

/// Documented data preconditions (see M6_BATTLE_REAL.md / M6_BATTLE_COMMANDS.md):
/// party count, the party-leader command slots CE143 copies, its stats, and one copy of
/// item 0 so CE152 marks real attacks usable. These are data the real save/flags would
/// have supplied; they are not fabricated menu rows.
fn seed_party(game: &mut Game) {
    game.databases.set_num(Container::CData, 18, 4, 0, 1); // party count = 1
    for (i, v) in [3, 4, 6, 7, 0, 0, 0, 0].iter().enumerate() {
        game.databases
            .set_num(Container::CData, 0, 12, 47 + i as i32, *v);
    }
    for (c, v) in [
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
    ] {
        game.databases.set_num(Container::CData, 0, 12, c, v);
    }
    game.databases.set_num(Container::CData, 7, 0, 0, 1); // 所持アイテム個数 item0 = 1
}

const REAL_ATTACK_CODES: [i32; 4] = [26, 25, 30, 29];

struct LineA {
    reached_ce143: bool,
    reached_ce152: bool,
    reached_ce192: bool,
    reached_target_window: bool,
    real_attack_selectable: bool,
    copy_maxhp: i32,
    copy_hp: i32,
    max_ce188_pc: i32,
    reached_turn_process: bool,
    saw_ce244: bool,
    warnings: u64,
}

fn run_ce188_line(frames: usize) -> LineA {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let _ = game.start_event(188);

    let mut a = LineA {
        reached_ce143: false,
        reached_ce152: false,
        reached_ce192: false,
        reached_target_window: false,
        real_attack_selectable: false,
        copy_maxhp: -1,
        copy_hp: -1,
        max_ce188_pc: 0,
        reached_turn_process: false,
        saw_ce244: false,
        warnings: 0,
    };

    for f in 0..frames {
        if (1..=60).contains(&f) {
            seed_party(&mut game);
        }
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();

        for inst in game.vm.instances() {
            match format!("{:?}", inst.event).as_str() {
                "143" => a.reached_ce143 = true,
                "152" => a.reached_ce152 = true,
                "192" => a.reached_ce192 = true,
                "157" | "158" => a.reached_target_window = true,
                "244" => a.saw_ce244 = true,
                _ => {}
            }
            if format!("{:?}", inst.event) == "188" && (inst.pc as i32) > a.max_ce188_pc {
                a.max_ce188_pc = inst.pc as i32;
                if a.max_ce188_pc >= 116 {
                    a.reached_turn_process = true;
                }
            }
        }
        // Capture the exact CE143 copy: type10 row0 col1=maxHP, col2=HP.
        if a.reached_ce143 && a.copy_hp < 0 {
            a.copy_maxhp = game.databases.num(Container::CData, 10, 0, 1).unwrap_or(-1);
            a.copy_hp = game.databases.num(Container::CData, 10, 0, 2).unwrap_or(-1);
        }
        // Real attack row selectable = a 万能ウィンドウ row with a real attack code and
        // 選択可能(col2)=1.
        for row in 0..8 {
            let code = game
                .databases
                .num(Container::CData, 19, row, 1)
                .unwrap_or(0);
            let selectable = game
                .databases
                .num(Container::CData, 19, row, 2)
                .unwrap_or(0);
            if REAL_ATTACK_CODES.contains(&code) && selectable == 1 {
                a.real_attack_selectable = true;
            }
        }
    }
    a.warnings = game.stats.warnings;
    a
}

struct LineB {
    saw_battle_active: bool, // 2000340 == 1
    saw_turn_set: bool,      // 2000349 == 1
    ce242_started: bool,
    ce243_started: bool,
    saw_ce244: bool,
    v350_max: i32,
    v352: i32,
    v343: i32,
    max_ce239_pc: i32,
    entered_end_judgment: bool, // CE239 case2 (pc >= 876)
    warnings: u64,
}

fn run_ce239_line(frames: usize) -> LineB {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    // Full real session: this auto-starts ENGINE_ALWAYS + the 2000350-gated CE242/CE243.
    let _ = game.start_engine_events();
    let _ = game.start_event(239);

    let mut b = LineB {
        saw_battle_active: false,
        saw_turn_set: false,
        ce242_started: false,
        ce243_started: false,
        saw_ce244: false,
        v350_max: 0,
        v352: 0,
        v343: 0,
        max_ce239_pc: 0,
        entered_end_judgment: false,
        warnings: 0,
    };

    for f in 0..frames {
        if (1..=60).contains(&f) {
            seed_party(&mut game);
        }
        // Give the battle-caller its enemy id (self1600000 = 6) once CE239 exists.
        let ids: Vec<(u32, i32)> = game
            .vm
            .instances()
            .iter()
            .map(|i| {
                (
                    i.id,
                    if format!("{:?}", i.event) == "239" {
                        i.pc as i32
                    } else {
                        -1
                    },
                )
            })
            .collect();
        for (id, pc) in ids {
            if pc >= 0 {
                if let Some(inst) = game.vm.instance_mut(id) {
                    if inst.self_vars.get(600usize) == 0 {
                        inst.self_vars.set(600usize, 6); // enemy id
                        inst.self_vars.set(601usize, 1);
                        inst.self_vars.set(602usize, 7);
                        inst.self_vars.set(603usize, 0);
                    }
                }
            }
        }
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();

        for inst in game.vm.instances() {
            match format!("{:?}", inst.event).as_str() {
                "242" => b.ce242_started = true,
                "243" => b.ce243_started = true,
                "244" => b.saw_ce244 = true,
                "239" => {
                    let pc = inst.pc as i32;
                    if pc > b.max_ce239_pc {
                        b.max_ce239_pc = pc;
                    }
                    if pc >= 876 {
                        b.entered_end_judgment = true;
                    }
                }
                _ => {}
            }
        }
        let v340 = game.vm.vars.get(340);
        let v349 = game.vm.vars.get(349);
        let v350 = game.vm.vars.get(350);
        if v340 == 1 {
            b.saw_battle_active = true;
        }
        if v349 == 1 {
            b.saw_turn_set = true;
        }
        if v350 > b.v350_max {
            b.v350_max = v350;
        }
    }
    b.v352 = game.vm.vars.get(352);
    b.v343 = game.vm.vars.get(343);
    b.warnings = game.stats.warnings;
    b
}

#[test]
fn m7_battle_round_real_line_and_ce244_blocker() {
    // ---- Scenario A: the real CE188 battle line boots and reaches command selection ----
    let a = run_ce188_line(400);
    assert!(
        a.reached_ce143,
        "CE188 line must run CE143 主人公DB→戦闘DBコピー"
    );
    assert_eq!(
        a.copy_maxhp, 100,
        "CE143 must copy maxHP into type10 row0 col1"
    );
    assert_eq!(a.copy_hp, 100, "CE143 must copy HP into type10 row0 col2");
    assert!(
        a.reached_ce152,
        "CE188 line must reach CE152 戦闘コマンド一覧算出"
    );
    assert!(
        a.reached_ce192,
        "CE188 line must reach CE192 味方コマンド選択"
    );
    assert!(
        a.real_attack_selectable,
        "a real attack row (code 26/25/30/29) must be selectable in the command window"
    );
    assert!(
        a.reached_target_window,
        "confirming a command must open the CE157/CE158 target-selection window"
    );
    assert!(
        !a.saw_ce244,
        "CE188 never calls CE244 — settlement is structurally out of this line"
    );
    assert!(
        a.warnings == 0,
        "scenario A must run with zero warnings, got {}",
        a.warnings
    );

    // ---- Scenario B: the real CE239 battle-caller line — pin the 2000350 deadlock ----
    let b = run_ce239_line(1600);
    assert!(
        b.saw_battle_active,
        "CE239 must reach its battle-active case (2000340 = 1)"
    );
    assert!(
        b.saw_turn_set,
        "CE239 must set the elapsed-turn counter 2000349 = 1"
    );
    // The crux: the 3-state command machine never leaves idle.
    assert_eq!(
        b.v350_max, 0,
        "2000350 (コマンド処理中) must stay 0 — no state ever sets it to 1"
    );
    assert!(
        !b.ce242_started,
        "CE242 must never start (it needs 2000350 == 1)"
    );
    assert!(
        !b.ce243_started,
        "CE243 must never start (it needs 2000350 == 2)"
    );
    assert!(
        !b.saw_ce244,
        "CE244 settlement must never run (only 239/241/242/243 call it)"
    );
    assert_eq!(
        b.v352, 0,
        "2000352 (勝ち負け) stays 0 — CE244 never writes it"
    );
    assert_eq!(
        b.v343, 0,
        "2000343 (戦闘終了) stays 0 — CE244 never writes it"
    );
    assert!(
        !b.entered_end_judgment,
        "CE239 must not enter its end-judgment case2 (it blocks waiting on 2000343)"
    );
    assert!(
        b.warnings == 0,
        "scenario B must run with zero warnings, got {}",
        b.warnings
    );

    eprintln!(
        "M7_BATTLE_ROUND: A(max188pc={}, ce143={}, copyHP={}/{}, ce152={}, ce192={}, target_win={}, real_attack_selectable={}, ce244={}, warns={}) | B(battle_active={}, turn_set={}, v350_max={}, ce242={}, ce243={}, ce244={}, v352={}, v343={}, max239pc={}, warns={})",
        a.max_ce188_pc, a.reached_ce143, a.copy_hp, a.copy_maxhp, a.reached_ce152, a.reached_ce192,
        a.reached_target_window, a.real_attack_selectable, a.saw_ce244, a.warnings,
        b.saw_battle_active, b.saw_turn_set, b.v350_max, b.ce242_started, b.ce243_started,
        b.saw_ce244, b.v352, b.v343, b.max_ce239_pc, b.warnings
    );
}
