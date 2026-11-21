//! M11 P1 (task-96) — the REAL CE328 深夜 routine reaches its single CE402
//! caller (pc986 `op210[500402]`) and CE402 then drives CE410 to completion.
//!
//! This is deliberately *not* a `start_event(402)` harness.  We load a retail
//! save, apply a synthetic but calendar-consistent fixture (var6=4 Thursday,
//! var4=21 21:00, plus CE402's relationship/H gates) and start CommonEvent 328.
//! Everything after that is CE328's own control flow:
//!
//!   693 合宿処理 → 694 (var304<1) → 715..721 就寝 → 745 通常就寝
//!   → 746 (var18<50 false path) → 845 (var494!=2) → 850 (var160 not 83/84)
//!   → 969..985 → 986 op210[500402]
//!
//! CE402 runs to its last slot (pc511); along the way it calls CE410 (騎乗位系
//! カットイン) several times, which itself runs to its last slot (pc247).
//!
//! Frame granularity: the interpreter is stepped with `Vm::with_budget(..., 1)`
//! so one instruction retires per frame.  That changes *when* a frame boundary
//! lands, never which instruction executes — the trace is a faithful, repeatable
//! rendering of CE328's real branch.  A second test pins the CE402 var23 gate
//! (var23≤5 aborts at pc52 → pc54), which is why the first test seeds var23=6.

use std::collections::BTreeSet;
use std::path::PathBuf;

use wolf_game::game::Game;
use wolf_vm::exec::{ChoiceReply, Vm};

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn data_root() -> PathBuf {
    match std::env::var("WOLF_DATA_ROOT") {
        Ok(v) if !v.is_empty() => repo_root().join(v),
        _ => repo_root().join("work").join("all"),
    }
}

fn save_path(name: &str) -> PathBuf {
    repo_root()
        .join("work")
        .join("retail_backup")
        .join("Save")
        .join(format!("{name}.sav"))
}

/// Calendar-consistent fixture for the 21:00 Thursday 就寝 route.  var18 is
/// kept below 50 so pc746 takes its else branch; var160 is 90 so pc850 is false
/// and its else body (969..997) — the one that holds the CE402 call — executes.
fn seed_deep(var23: i32) -> Vec<(usize, i32)> {
    vec![
        (304, 0),
        (6, 4),
        (4, 21),
        (5, 0),
        (18, 20),
        (16, 1),
        (22, 180),
        (326, 2),
        (102, 80),
        (103, 2),
        (23, var23),
        (151, 0),
        (20, 61),
        (265, 0),
        (160, 90),
        (545, 1),
        (494, 0),
    ]
}

struct DeepRun {
    frames: usize,
    ce328_seq: Vec<usize>,
    ce402_seq: Vec<usize>,
    ce410_seq: Vec<usize>,
    menus: Vec<(usize, usize, usize)>,
    cg_files: BTreeSet<String>,
    vars_at_402: Vec<(usize, i32)>,
    var64_end: i32,
    warnings: u64,
}

fn open_budget_one(name: &str, seed: &[(usize, i32)]) -> Game {
    let mut g = Game::new(data_root()).unwrap();
    g.load_save(&save_path(name)).unwrap();
    for &(k, v) in seed {
        g.vm.vars.set(k, v);
    }
    g.auto_advance = true;
    g.auto_choice = false;
    g.auto_choice_delay = 2;
    g.set_choice_cursor(0);
    // Swap in the one-instruction budget *after* the save is loaded and vars are
    // seeded, keeping the exact same variable bank and program set.
    let programs = g.vm.programs.clone();
    let vars = g.vm.vars.clone();
    g.vm = Vm::with_budget(programs, 1);
    g.vm.vars = vars;
    g.start_event(328);
    g
}

fn run_deep(var23: i32, frame_budget: usize) -> DeepRun {
    let mut g = open_budget_one("SaveData01", &seed_deep(var23));
    let mut out = DeepRun {
        frames: 0,
        ce328_seq: Vec::new(),
        ce402_seq: Vec::new(),
        ce410_seq: Vec::new(),
        menus: Vec::new(),
        cg_files: BTreeSet::new(),
        vars_at_402: Vec::new(),
        var64_end: 0,
        warnings: 0,
    };
    let mut prev328: Option<usize> = None;
    let mut prev402: Option<usize> = None;
    let mut prev410: Option<usize> = None;
    let mut answered: BTreeSet<usize> = BTreeSet::new();

    for f in 0..frame_budget {
        out.frames = f;
        g.frame();

        // CE328 head trace.
        if let Some(pc) =
            g.vm.instances()
                .iter()
                .find(|i| i.event == 328)
                .map(|i| i.pc)
        {
            if prev328 != Some(pc) {
                out.ce328_seq.push(pc);
                prev328 = Some(pc);
            }
        }
        // CE402 trace.
        if out.ce402_seq.is_empty() && g.vm.instances().iter().any(|i| i.event == 402) {
            for k in [
                4usize, 5, 6, 16, 18, 20, 22, 23, 102, 103, 160, 265, 304, 326, 494, 545,
            ] {
                out.vars_at_402.push((k, g.vm.vars.get(k)));
            }
        }
        if let Some(pc) =
            g.vm.instances()
                .iter()
                .find(|i| i.event == 402)
                .map(|i| i.pc)
        {
            if prev402 != Some(pc) {
                out.ce402_seq.push(pc);
                prev402 = Some(pc);
            }
        }
        // CE410 trace.
        if let Some(pc) =
            g.vm.instances()
                .iter()
                .find(|i| i.event == 410)
                .map(|i| i.pc)
        {
            if prev410 != Some(pc) {
                out.ce410_seq.push(pc);
                prev410 = Some(pc);
            }
        }

        // While a CG event owns the screen, snapshot every visible image name.
        let cg_active =
            g.vm.instances()
                .iter()
                .any(|i| i.event == 402 || i.event == 410);
        if cg_active {
            for q in g.screen.quads(|_| Some((1, 1))) {
                if q.alpha > 0.5 && !q.file.is_empty() && q.file != "cg/blackscreen.jpg" {
                    out.cg_files.insert(q.file.clone());
                }
            }
        }

        // Answer every choice with option 0 immediately, exactly as the trace
        // walk does.  Menus are recorded once per event pc.
        if let Some(ch) = &g.choices {
            let nopts = ch.options.len();
            let pc =
                g.vm.instances()
                    .iter()
                    .find(|i| i.event == 328)
                    .map(|i| i.pc)
                    .unwrap_or(usize::MAX);
            if answered.insert(pc) {
                out.menus.push((pc, nopts, 0));
            }
            g.reply(ChoiceReply::Index(0));
        }

        // Done once CE410 has retired and CE328 itself has run out.
        if !out.ce410_seq.is_empty() && !g.vm.instances().iter().any(|i| i.event == 328) {
            break;
        }
    }
    out.var64_end = g.vm.vars.get(64);
    out.warnings = g.stats.warnings;
    out
}

fn assert_no_warnings(run: &DeepRun) {
    assert_eq!(
        run.warnings,
        0,
        "engine warnings must stay at zero: {run:?}",
        run = run.warnings
    );
}

#[test]
fn real_ce328_reaches_ce402_and_drives_ce410_to_completion() {
    let run = run_deep(6, 120_000);

    // CE328 went through the real branch tail and hit pc986 op210[500402].
    for pc in [693usize, 721, 745, 845, 850, 969, 985, 986] {
        assert!(
            run.ce328_seq.contains(&pc),
            "CE328 must execute pc{pc}; tail={:?}",
            run.ce328_seq
                .iter()
                .filter(|p| **p >= 680)
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(
        run.ce328_seq.last().copied(),
        Some(1024),
        "CE328 must run to its final slot"
    );

    // CE402 was entered by that op210 and ran to the end of its 512-slot program.
    assert!(
        !run.ce402_seq.is_empty(),
        "CE402 must be started from CE328 pc986"
    );
    assert_eq!(
        run.ce402_seq.first().copied(),
        Some(0),
        "CE402 must start at pc0"
    );
    assert_eq!(
        run.ce402_seq.last().copied(),
        Some(511),
        "CE402 must reach its final slot (pc511); seq={:?}",
        run.ce402_seq
    );
    assert!(
        run.ce402_seq.contains(&52),
        "CE402 var23 gate at pc52 must pass with var23=6"
    );
    assert!(
        run.ce402_seq.contains(&165),
        "CE402 pc165 is the first CE410 call site"
    );

    // CE410 (騎乗位系カットイン) is spawned by those op210 calls and runs to its end.
    assert!(
        !run.ce410_seq.is_empty(),
        "CE410 must be started from CE402"
    );
    assert_eq!(
        run.ce410_seq.first().copied(),
        Some(0),
        "CE410 must start at pc0"
    );
    assert_eq!(
        run.ce410_seq.last().copied(),
        Some(247),
        "CE410 must reach its final slot (pc247); seq={:?}",
        run.ce410_seq
    );

    // The reachable route to pc986 goes through the pc850-else body (969..985).
    // It does NOT pass pc964 (var64=1) / pc966 (CE331): that is the 悪戯へ branch,
    // which then hits pc968 op420 Case and jumps past the whole 969..997 body.
    assert!(run.ce328_seq.contains(&969) && run.ce328_seq.contains(&985));
    assert!(
        !run.ce328_seq.contains(&964),
        "pc964 (var64=1) must not be on the route into CE402; seq={:?}",
        run.ce328_seq
    );
    assert_eq!(run.var64_end, 0, "the CE402 route must not write var64");

    // Real CG assets were on screen while the CG events ran.
    assert!(
        !run.cg_files.is_empty(),
        "at least one CG image must be visible during CE402/CE410"
    );
    let has_cg = run
        .cg_files
        .iter()
        .any(|f| f.contains("cutin") || f.contains("cg/"));
    assert!(has_cg, "expected a cutin/cg asset; saw {:?}", run.cg_files);

    assert_no_warnings(&run);
    println!(
        "real_ce328: frames={} ce402_maxpc={:?} ce410_maxpc={:?} vars@402={:?} menus={:?} cg_files={:?}",
        run.frames,
        run.ce402_seq.last(),
        run.ce410_seq.last(),
        run.vars_at_402,
        run.menus,
        run.cg_files,
    );
}

#[test]
fn ce402_var23_gate_aborts_at_pc54_when_below_six() {
    let run = run_deep(5, 120_000);
    assert!(
        !run.ce402_seq.is_empty(),
        "CE402 still starts; the gate is inside CE402"
    );
    assert_eq!(
        run.ce402_seq.last().copied(),
        Some(54),
        "var23=5 must abort CE402 at pc54; seq={:?}",
        run.ce402_seq
    );
    assert!(
        !run.ce402_seq.contains(&511),
        "CE402 must not reach its end when var23=5"
    );
    assert!(
        run.ce410_seq.is_empty(),
        "CE410 must never be called when CE402 aborts at pc54"
    );
    assert_no_warnings(&run);
    println!("ce402_var23_gate: ce402_seq={:?}", run.ce402_seq);
}
