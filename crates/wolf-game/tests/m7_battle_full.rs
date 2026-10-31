//! M7 W1 (task-20): drive a REAL battle turn to CE244 settlement, and pin the exact
//! edge that blocks a menu-driven turn in the shipped event graph.
//!
//! Findings (full command/variable evidence in docs/research/M7_BATTLE_FULL.md):
//!
//!   * CE244 (終了判定) is the SOLE writer of 2000352 (勝ち負け) and 2000343 (戦闘終了).
//!     A settlement therefore requires CE244 to run.
//!   * The three-state command machine (2000350: 1=コマンド処理中 CE242, 2=実行 CE243+CE241,
//!     0=idle) has NO external entry: 2000350 is set to 1 ONLY by CE242 pc14 itself, and
//!     CE242 requires (run_condition) 2000350 == 1 to start -> circular bootstrap. No event
//!     in the 550-event graph calls 500242/500243 nor writes 2000350 = 1 from outside CE242.
//!   * Therefore the ONLY reachable path to CE244 settlement is to start the command-phase
//!     script CE242 directly with the player's chosen command pre-seeded (self 1600000).
//!     Scenario 1 proves that, once started, CE242 -> CE244 -> 2000352 = 1 -> 2000350 0->1->2->0
//!     is a genuine, engine-driven settlement round (CE244 really executes).
//!   * Scenario 2 drives the REAL battle caller CE239 with the battle-active / hot-spring
//!     flag 2000495 = 2: CE239 executes its pc795 `210 500244` edge to CE244, yet CE244 does
//!     NOT write 2000352/2000343 in this unprepared state, and CE239 falls into its
//!     end-judgment deadlock (case2 waits on 2000343). So CE239's own graph cannot settle.
//!
//! Runs with: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_battle_full -- --nocapture

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

fn event_num<E: std::fmt::Debug>(e: &E) -> String {
    format!("{:?}", e)
}

/// Set self flat index `idx` on the first live instance of `event`.
fn set_self_of(game: &mut Game, event: u32, idx: usize, value: i32) -> bool {
    let want = event.to_string();
    let id = game
        .vm
        .instances()
        .iter()
        .find(|i| event_num(&i.event) == want)
        .map(|i| i.id);
    if let Some(id) = id {
        if let Some(inst) = game.vm.instance_mut(id) {
            inst.self_vars.set(idx, value);
            return true;
        }
    }
    false
}

const REAL_ATTACK_CODES: [i32; 4] = [26, 25, 30, 29];

struct Ce188 {
    reached_ce143: bool,
    copy_maxhp: i32,
    copy_hp: i32,
    reached_ce152: bool,
    reached_ce192: bool,
    reached_target_window: bool, // CE157/CE158 target windows opened
    real_attack_selectable: bool,
    max_ce65_pc: i32,
    ce65_left_spin: bool, // saw CE65 pc > 140 (past the pc78..140 command-window spin)
    unanswered_250: usize,
    warnings: u64,
}

/// Scenario 0: the REAL CE188 battle line. Boots CE189->CE190->CE143->CE191->CE192->CE152
/// ->CE65 with the task-19 seed, proving a real attack is selectable and CE65 progresses
/// through the command window. CE188 NEVER calls CE244, so this line cannot settle — the
/// settlement lives only in the CE242 command-phase line (Scenario 1).
fn run_ce188_line(frames: usize) -> Ce188 {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let _ = game.seed_battle_databases();
    let _ = game.start_event(188);
    let mut a = Ce188 {
        reached_ce143: false,
        copy_maxhp: -1,
        copy_hp: -1,
        reached_ce152: false,
        reached_ce192: false,
        reached_target_window: false,
        real_attack_selectable: false,
        max_ce65_pc: 0,
        ce65_left_spin: false,
        unanswered_250: 0,
        warnings: 0,
    };
    for f in 0..frames {
        if (1..=60).contains(&f) {
            let _ = game.seed_battle_databases();
        }
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();
        for inst in game.vm.instances() {
            match event_num(&inst.event).as_str() {
                "143" => a.reached_ce143 = true,
                "152" => a.reached_ce152 = true,
                "192" => a.reached_ce192 = true,
                "157" | "158" => a.reached_target_window = true,
                "65" => {
                    let pc = inst.pc as i32;
                    if pc > a.max_ce65_pc {
                        a.max_ce65_pc = pc;
                    }
                    if pc > 140 {
                        a.ce65_left_spin = true;
                    }
                }
                _ => {}
            }
        }
        if a.reached_ce143 && a.copy_hp < 0 {
            a.copy_maxhp = game.databases.num(Container::CData, 10, 0, 1).unwrap_or(-1);
            a.copy_hp = game.databases.num(Container::CData, 10, 0, 2).unwrap_or(-1);
        }
        for row in 0..8 {
            let code = game.databases.num(Container::CData, 19, row, 1).unwrap_or(0);
            let sel = game.databases.num(Container::CData, 19, row, 2).unwrap_or(0);
            if REAL_ATTACK_CODES.contains(&code) && sel == 1 {
                a.real_attack_selectable = true;
            }
        }
    }
    a.unanswered_250 = game
        .diagnostics
        .iter()
        .filter(|(msg, _)| msg.contains("unanswered"))
        .count();
    a.warnings = game.stats.warnings;
    a
}

struct Round {
    v350_seq: Vec<i32>,
    saw_350_eq_1: bool,
    saw_350_eq_2: bool,
    v352_peak: i32,
    v343_peak: i32,
    v352_final: i32,
    v343_final: i32,
    v349_final: i32,
    frames_run: usize,
    final_v350: i32,
    warnings: u64,
}

/// Scenario 1: start the command-phase script CE242 directly, seed the player's chosen
/// command (self 1600000 = 1) and the battle-active flag (2000495 = 2). Traces the whole
/// 0->1->2->0 command cycle and CE244's settlement writes.
fn run_ce242_settlement(frames: usize) -> Round {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    // Real battle data (party/command slots/item stock) — task-19 helper.
    let _ = game.seed_battle_databases();
    // 2000495 == 2: battle-active / hot-spring flag. CE242 pc15 uses this to skip the
    // 万能ウィンドウ command menu (which needs unresolved CData(19,..) strings) and run the
    // command-1 (ユカリ応急処置) heal branch headlessly.
    game.vm.vars.set(495usize, 2);
    let _ = game.start_event(242);
    // Player chose command 1 -> self 1600000 = 1 (the menu that would set it is skipped).
    let seeded = set_self_of(&mut game, 242, 600, 1);
    assert!(seeded, "CE242 instance must exist to seed self 1600000 = 1");

    let mut r = Round {
        v350_seq: Vec::new(),
        saw_350_eq_1: false,
        saw_350_eq_2: false,
        v352_peak: 0,
        v343_peak: 0,
        v352_final: 0,
        v343_final: 0,
        v349_final: 0,
        frames_run: 0,
        final_v350: 0,
        warnings: 0,
    };
    let mut last = i32::MIN;
    for f in 0..frames {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();
        let v350 = game.vm.vars.get(350);
        let v352 = game.vm.vars.get(352);
        let v343 = game.vm.vars.get(343);
        if v350 != last {
            r.v350_seq.push(v350);
            last = v350;
        }
        if v350 == 1 {
            r.saw_350_eq_1 = true;
        }
        if v350 == 2 {
            r.saw_350_eq_2 = true;
        }
        if v352.abs() > r.v352_peak.abs() {
            r.v352_peak = v352;
        }
        r.v343_peak = r.v343_peak.max(v343);
        r.frames_run = f + 1;
    }
    r.v352_final = game.vm.vars.get(352);
    r.v343_final = game.vm.vars.get(343);
    r.v349_final = game.vm.vars.get(349);
    r.final_v350 = game.vm.vars.get(350);
    r.warnings = game.stats.warnings;
    r
}

struct Entry {
    battle_active: bool,
    turn_set: bool,
    v350_max: i32,
    v352: i32,
    v343: i32,
    max_ce239_pc: i32,
    saw_hotspring_region: bool, // CE239 pc in 751..=796 (2000495==2 case1 incl the pc795 210 500244 edge)
    saw_else_region: bool,      // CE239 pc in 798..=874 (2000495!=2 else body — no CE244 call)
    entered_end_judgment: bool, // CE239 pc >= 876 (deadlock case2)
    warnings: u64,
}

/// Scenario 2: the REAL battle caller CE239 driven with the hot-spring flag 2000495 = 2.
/// CE239 executes its own pc795 `210 500244` edge, but CE244 does not settle here.
fn run_ce239_real_entry(frames: usize) -> Entry {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let _ = game.seed_battle_databases();
    game.vm.vars.set(495usize, 2); // hot-spring branch -> CE239 pc749 case1 -> pc795 210 500244
    let _ = game.start_engine_events();
    let _ = game.start_event(239);

    let mut e = Entry {
        battle_active: false,
        turn_set: false,
        v350_max: 0,
        v352: 0,
        v343: 0,
        max_ce239_pc: 0,
        saw_hotspring_region: false,
        saw_else_region: false,
        entered_end_judgment: false,
        warnings: 0,
    };
    for f in 0..frames {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();
        if game.vm.vars.get(340) == 1 {
            e.battle_active = true;
        }
        if game.vm.vars.get(349) == 1 {
            e.turn_set = true;
        }
        e.v350_max = e.v350_max.max(game.vm.vars.get(350));
        for inst in game.vm.instances() {
            if event_num(&inst.event) == "239" {
                let pc = inst.pc as i32;
                if pc > e.max_ce239_pc {
                    e.max_ce239_pc = pc;
                }
                if (751..=796).contains(&pc) {
                    e.saw_hotspring_region = true;
                }
                if (798..=874).contains(&pc) {
                    e.saw_else_region = true;
                }
                if pc >= 876 {
                    e.entered_end_judgment = true;
                }
            }
        }
    }
    e.v352 = game.vm.vars.get(352);
    e.v343 = game.vm.vars.get(343);
    e.warnings = game.stats.warnings;
    e
}

struct Unwind {
    entered_victory_case: bool,
    max_ce239_pc: i32,
    alive_at_end: bool,
    warnings: u64,
}

/// Scenario 3: CE239's end-judgment unwind. Given a REAL CE244 settlement already wrote
/// 2000343 = 1 and 2000340 = 1 (battle active), CE239 enters its case2 victory branch
/// (pc >= 876), passes the pc1099 child, and returns (the instance ends). This isolates the
/// "return to pc1106" half: it works as soon as 2000343 = 1 exists; the ONLY missing link is
/// the engine wiring that starts the command phase (CE242) on a real 2000340 rising edge.
fn run_ce239_unwind(frames: usize) -> Unwind {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let _ = game.seed_battle_databases();
    // Precondition: a completed CE244 settlement (Scenario 1 writes exactly these).
    game.vm.vars.set(340usize, 1); // 2000340 戦闘中 = 1
    game.vm.vars.set(343usize, 1); // 2000343 戦闘終了 = 1
    game.vm.vars.set(352usize, 1); // 2000352 勝ち負け = 1
    let _ = game.start_event(239);
    let mut u = Unwind { entered_victory_case: false, max_ce239_pc: 0, alive_at_end: true, warnings: 0 };
    for _f in 0..frames {
        game.key_bits = wolf_game::key::OK;
        let _ = game.frame();
        let mut alive = false;
        for inst in game.vm.instances() {
            if event_num(&inst.event) == "239" {
                alive = true;
                let pc = inst.pc as i32;
                if pc > u.max_ce239_pc {
                    u.max_ce239_pc = pc;
                }
                if pc >= 876 {
                    u.entered_victory_case = true;
                }
            }
        }
        u.alive_at_end = alive;
        if !alive && _f > 2 {
            break;
        }
    }
    u.warnings = game.stats.warnings;
    u
}

#[test]
fn m7_battle_full_settles_via_ce244_and_pins_real_entry_gap() {
    // ---- Scenario 0: real CE188 line reaches the command window with real attacks ----
    let c = run_ce188_line(400);
    eprintln!(
        "SC0 CE188 line: ce143={} copyHP={}/{} ce152={} ce192={} target_win={} selectable={} max65pc={} left_spin={} unanswered250={} warns={}",
        c.reached_ce143, c.copy_hp, c.copy_maxhp, c.reached_ce152, c.reached_ce192,
        c.reached_target_window, c.real_attack_selectable, c.max_ce65_pc, c.ce65_left_spin,
        c.unanswered_250, c.warnings
    );
    assert!(c.reached_ce143, "CE188 line must run CE143 主人公DB→戦闘DBコピー");
    assert_eq!(c.copy_maxhp, 100, "CE143 must copy maxHP into type10 row0 col1");
    assert_eq!(c.copy_hp, 100, "CE143 must copy HP into type10 row0 col2");
    assert!(c.reached_ce152, "CE188 line must reach CE152 戦闘コマンド一覧算出");
    assert!(c.reached_ce192, "CE188 line must reach CE192 味方コマンド選択");
    assert!(c.real_attack_selectable, "a real attack row (code 26/25/30/29) must be selectable");
    assert!(c.max_ce65_pc >= 78, "CE65 万能ウィンドウ must open (pc >= 78), got {}", c.max_ce65_pc);
    assert!(c.warnings == 0, "scenario 0 must run with zero warnings, got {}", c.warnings);

    // ---- Scenario 1: command phase -> CE244 settlement, the reachable round ----
    let r = run_ce242_settlement(200);
    eprintln!(
        "SC1 CE242 round: v350_seq={:?} saw350=1:{} saw350=2:{} v352={} v343={} v349={} final350={} frames={} warns={}",
        r.v350_seq, r.saw_350_eq_1, r.saw_350_eq_2, r.v352_final, r.v343_final, r.v349_final,
        r.final_v350, r.frames_run, r.warnings
    );
    assert!(r.saw_350_eq_1, "CE242 pc14 must drive 2000350 -> 1 (command phase)");
    assert!(r.saw_350_eq_2, "CE242 pc208 must hand off 2000350 -> 2 (execute phase)");
    assert_eq!(r.final_v350, 0, "the turn must return 2000350 to idle (0)");
    assert!(
        r.v352_final == 1 || r.v352_final == -1,
        "CE244 must write a settlement 2000352 in {{1,-1}}, got {}",
        r.v352_final
    );
    assert_eq!(r.v343_final, 1, "CE244 must write 2000343 (戦闘終了) = 1");
    assert_eq!(r.warnings, 0, "scenario 1 must run with zero warnings, got {}", r.warnings);

    // ---- Scenario 2: real caller CE239 reaches CE244 but cannot settle ----
    let e = run_ce239_real_entry(900);
    eprintln!(
        "SC2 CE239 entry: battle_active={} turn_set={} v350_max={} v352={} v343={} max_pc={} hotspring_region={} else_region={} end_judgment={} warns={}",
        e.battle_active, e.turn_set, e.v350_max, e.v352, e.v343, e.max_ce239_pc,
        e.saw_hotspring_region, e.saw_else_region, e.entered_end_judgment, e.warnings
    );
    assert!(e.battle_active, "CE239 must reach its battle-active case (2000340 = 1)");
    assert!(e.turn_set, "CE239 must set 2000349 = 1");
    assert_eq!(e.v350_max, 0, "2000350 must stay 0 — CE242/CE243 never start (bootstrap gap)");
    assert_eq!(e.v352, 0, "CE244 never settles from the real-entry path: 2000352 stays 0");
    assert_eq!(e.v343, 0, "2000343 stays 0 from the real-entry path");
    assert!(
        e.saw_hotspring_region && !e.saw_else_region,
        "CE239 with 2000495==2 must take the hot-spring case1 (pc795 210 500244 edge), not the else body"
    );
    assert!(
        e.max_ce239_pc >= 796,
        "CE239 must run past the pc795 210 500244 edge (max pc {}), proving the real CE244 call site is executed",
        e.max_ce239_pc
    );
    assert!(e.warnings == 0, "scenario 2 must run with zero warnings, got {}", e.warnings);

    // ---- Scenario 3: CE239 end-judgment unwind returns once 2000343 = 1 exists ----
    let u = run_ce239_unwind(1200);
    eprintln!(
        "SC3 CE239 unwind: entered_victory_case={} max_pc={} alive_at_end={} warns={}",
        u.entered_victory_case, u.max_ce239_pc, u.alive_at_end, u.warnings
    );
    assert!(u.entered_victory_case, "CE239 must enter its case2 victory branch (pc >= 876) when 2000343 = 1");
    assert!(
        u.max_ce239_pc >= 1099,
        "CE239 must unwind past the victory display / pc1099 child to its return region, got {}",
        u.max_ce239_pc
    );
    assert!(
        !u.alive_at_end,
        "CE239 must finish (instance gone) after the victory unwind, proving it runs through its return at pc1104/1106"
    );
    assert!(u.warnings == 0, "scenario 3 must run with zero warnings, got {}", u.warnings);
}
