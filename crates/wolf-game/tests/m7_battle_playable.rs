//! M7 task-34: prove a REAL battle can run and settle with the rebuilt engine wiring.
//!
//! The rebuilt wiring (crates/wolf-game/src/battle_wiring.rs) starts the command-phase
//! script CE242 on the 0 -> 1 edge of the battle-active flag 2000340, while the command
//! phase 2000350 is idle. Without it the battle caller CE239 raises 2000340 but nothing
//! ever sets 2000350 = 1 (CE242 is the only writer and it needs 2000350 == 1 to start),
//! so CE244 never settles and CE239 deadlocks in its end judgment.
//!
//! This test drives the REAL day-4 regional-contribution battle wrapper CE253 (called by
//! the day-4 CE251 row at pc2466) with the task-19 database seed and the new wiring, and
//! asserts that CE244 really settles the battle (2000352 in {1,-1}, 2000343 = 1) with no
//! warnings, without hardcoding 2000350 and without seeding 2000352.
//!
//! Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_battle_playable -- --nocapture

use std::path::PathBuf;

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

#[derive(Default)]
struct Trace {
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
    frames_run: usize,
    warnings: u64,
    unanswered_250: usize,
    ce239_alive_at_end: bool,
}

fn run_playable(frames: usize, verbose: bool) -> (Trace, Game) {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    // Task-19 real battle databases (party count, command slots, item stock).
    let seeded = game.seed_battle_databases();
    assert!(seeded > 0, "seed_battle_databases must seed the battle cells");
    // The command-menu (`万能ウィンドウ`) that CE242 renders reads one section header
    // label from CData (5, 16, 0); a fresh SaveData leaves it blank, so the cosmetic
    // `250 DatabaseAccess` read comes back "unanswered". This is a display label, not a
    // command or target, so completing it is data seeding (like seed_battle_databases),
    // NOT menu input. Seeding it keeps the run's only 250 read resolved.
    game.databases
        .set_text(wolf_game::db::Container::CData, 5, 16, 0, "┗ 状態リスト");
    // Engine coroutines: CE241 (turn start) + the run==2 scheduler that starts CE240
    // (combat-power) once 2000340 becomes active.
    let _ = game.start_engine_events();
    // THE REBUILT WIRING under test: start CE242 on the battle-active edge.
    game.set_battle_wiring(true);
    // Real day-4 battle wrapper (called by CE251 pc2466). Runs its story lead-in, then
    // calls CE239 (pc174..pc191) with the command it chose from the 2000028 thresholds.
    let _ = game.start_event(253);

    let mut t = Trace::default();
    let mut last350 = i32::MIN;
    for f in 0..frames {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();
        t.frames_run = f + 1;
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
            match event_num(&inst.event).as_str() {
                "253" => t.ce253_started = true,
                "239" => {
                    t.ce239_started = true;
                    let pc = inst.pc as i32;
                    if pc > t.max_ce239_pc { t.max_ce239_pc = pc; }
                }
                "242" => {
                    t.ce242_started = true;
                    let pc = inst.pc as i32;
                    if pc > t.max_ce242_pc { t.max_ce242_pc = pc; }
                }
                "243" => t.ce243_started = true,
                "241" => t.ce241_started = true,
                _ => {}
            }
        }
        if verbose && (v350 != 0 || v352 != 0 || v343 != 0 || t.wiring_rounds > 0) {
            let pcs: Vec<String> = game
                .vm
                .instances()
                .iter()
                .filter(|i| matches!(event_num(&i.event).as_str(), "239" | "242" | "243" | "241"))
                .map(|i| format!("{}@{}:{:?}", event_num(&i.event), i.pc, i.wait))
                .collect();
            println!(
                "f{f:5} v340={v340} v350={v350} v352={v352} v343={v343} rounds={} alive=[{}]",
                t.wiring_rounds,
                pcs.join(", ")
            );
        }
        // Stop once the battle is decided AND CE239 has had time to drain its tail.
        if v343 == 1 && v350 == 0 {
            // The 2000350 = 2 action phase (CE242's tail) and CE241's _->0 reset can
            // both happen inside a single Game::frame(), so keep recording during the
            // drain or the transient 2 (and the short-lived CE243) is lost.
            for _ in 0..800 {
                let _ = game.frame();
                t.frames_run += 1;
                let v350b = game.vm.vars.get(350);
                if v350b != last350 {
                    if verbose {
                        println!("f{} (drain) v350={} v352={} v343={}", t.frames_run, v350b, game.vm.vars.get(352), game.vm.vars.get(343));
                    }
                    t.v350_seq.push(v350b);
                    last350 = v350b;
                }
                t.v352_peak = if game.vm.vars.get(352).abs() > t.v352_peak.abs() {
                    game.vm.vars.get(352)
                } else { t.v352_peak };
                t.v343_peak = t.v343_peak.max(game.vm.vars.get(343));
                for inst in game.vm.instances() {
                    match event_num(&inst.event).as_str() {
                        "239" => { let pc = inst.pc as i32; if pc > t.max_ce239_pc { t.max_ce239_pc = pc; } }
                        "242" => { let pc = inst.pc as i32; if pc > t.max_ce242_pc { t.max_ce242_pc = pc; } }
                        "243" => t.ce243_started = true,
                        "241" => t.ce241_started = true,
                        _ => {}
                    }
                }
            }
            break;
        }
    }
    t.unanswered_250 = game
        .diagnostics
        .iter()
        .filter(|(msg, _)| msg.contains("unanswered"))
        .count();
    t.ce239_alive_at_end = game
        .vm
        .instances()
        .iter()
        .any(|i| event_num(&i.event) == "239");
    t.warnings = game.stats.warnings;
    (t, game)
}

#[test]
fn real_day4_battle_settles_with_rebuilt_wiring() {
    let (t, _game) = run_playable(20000, true);
    println!(
        "\nRESULT: ce253={} ce239={} ce242={} ce243={} ce241={} rounds={} battle_active_f={:?} v350_seq={:?} v352={} v343={} max239pc={} max242pc={} warnings={} unanswered250={}",
        t.ce253_started, t.ce239_started, t.ce242_started, t.ce243_started, t.ce241_started,
        t.wiring_rounds, t.battle_active_frame, t.v350_seq, t.v352_peak, t.v343_peak,
        t.max_ce239_pc, t.max_ce242_pc, t.warnings, t.unanswered_250
    );
    // Residual diagnostics (all non-warning): the battle-start `201 SetMoveRoute` sites
    // have no map layer here, one debug print, one hard-cut transition, one Party arg
    // recorded only. None are warnings and none is a `250` database miss.
    println!("DIAGNOSTICS (non-warning residuals):");
    for (msg, n) in &_game.diagnostics {
        println!("  x{n}: {msg}");
    }
    let alive: Vec<String> = _game.vm.instances().iter()
        .map(|i| format!("{}@{}:{:?}", event_num(&i.event), i.pc, i.wait))
        .collect();
    println!("ALIVE_AT_END: [{}]", alive.join(", "));
    assert!(t.ce253_started, "CE253 (real day-4 battle wrapper) must start");
    assert!(t.ce239_started, "CE239 (battle caller) must be reached from CE253");
    assert!(t.battle_active_frame.is_some(), "2000340 (battle-active) must rise");
    assert!(t.wiring_rounds >= 1, "rebuilt wiring must start CE242 on the battle-active edge");
    assert!(t.ce242_started, "CE242 (command phase) must run");
    assert!(
        t.v350_seq.contains(&1),
        "the 2000350 state machine must enter the command phase (1), got {:?}", t.v350_seq
    );
    assert!(
        t.max_ce242_pc >= 200,
        "CE242 must reach its action block (>= pc200, the CE244 call region), max pc {}",
        t.max_ce242_pc
    );
    assert!(
        t.v352_peak == 1 || t.v352_peak == -1,
        "CE244 must settle: 2000352 in {{1,-1}}, got {}", t.v352_peak
    );
    assert_eq!(t.v343_peak, 1, "CE244 must set 2000343 = 1");
    assert_eq!(t.warnings, 0, "no warnings allowed");
    assert_eq!(t.unanswered_250, 0, "no unanswered 250 reads in the battle section");
    assert!(
        !t.ce239_alive_at_end,
        "CE239 must return (leave its end-judgment loop) after 2000343 = 1, max pc was {}",
        t.max_ce239_pc
    );
}
