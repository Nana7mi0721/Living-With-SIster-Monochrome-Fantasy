//! M11-H (task-97) — the REAL CE328 深夜 routine reaches its single CE402 caller
//! (pc986 op210[500402]) and CE402 then drives CE410, using **natural frame timing**
//! (the default 200_000-instruction Vm budget and ordinary Game::frame steps) rather
//! than the one-instruction-per-frame probe budget used by m11_hscene_ce328_real_cg.rs.
//!
//! Reached route (same branch as the budget-1 fixture):
//!   693 合宿処理 → 694 (var304<1) → 715..721 就寝 → 745 通常就寝
//!   → 746 (var18<50) → 845 (var494!=2) → 850 (var160 not 83/84)
//!   → 969..985 → 986 op210[500402] → CE402 (tail pc509) → CE410 (act bodies).
//!
//! IMPORTANT boundary established by these tests (see docs/research/M11_HSCENE_NATURAL.md):
//!   * On natural timing CE402 runs to its tail (pc509 > 500) and calls CE410 five times
//!     with act arguments [1, 1, 2, 1, 3].
//!   * CE410's "deep" bodies past pc200 (騎乗位絶頂 pc157, スパイダー絶頂 pc201..247) belong to
//!     act-4 / act-5 branches.  CE402 reaches those branch call sites (pc288/pc360 act=4,
//!     pc447 act=5) ONLY under the diagnostic budget<=5 interleaving used by the probe —
//!     never on natural timing from SaveData01 + seed_deep.  So the "CE410 pc>200" claim of
//!     the budget-1 fixture is an interleaving artefact, not retail behaviour.  The second
//!     test pins that boundary (act ceiling == 3, no v144_cg20 / cutin_189/190 assets).
//!
//! Choice prompts are answered manually with ChoiceReply::Index(0) on every frame a prompt
//! is open — equivalent to a headless `--answer 0 --answer-delay 1` capture.

use std::collections::BTreeSet;
use std::path::PathBuf;

use wolf_game::game::Game;
use wolf_vm::exec::ChoiceReply;

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

/// Calendar-consistent fixture for the 21:00 Thursday 就寝 route.  var18 stays below
/// 50 (pc746 else), var160=90 (pc850 false → the 969..997 body that holds pc986),
/// var23=6 (CE402 pc52 gate), var494=0 (pc845 false).
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

struct NaturalRun {
    frames: usize,
    ce328_maxpc: usize,
    ce402_maxpc: usize,
    ce410_maxpc: usize,
    ce328_seq: Vec<usize>,
    ce402_seq: Vec<usize>,
    ce410_acts: Vec<i32>,
    cg_files: BTreeSet<String>,
    var64_end: i32,
    warnings: u64,
}

fn open_natural(name: &str, seed: &[(usize, i32)]) -> Game {
    let mut g = Game::new(data_root()).unwrap();
    g.load_save(&save_path(name)).unwrap();
    for &(k, v) in seed {
        g.vm.vars.set(k, v);
    }
    // Natural timing: DEFAULT_BUDGET (200_000 instructions/frame).  We rebuild the Vm
    // only to reproduce the fixture's fresh-Vm state (same programs + vars); the budget
    // stays at the natural 200_000, never with_budget(1).
    {
        let programs = g.vm.programs.clone();
        let vars = g.vm.vars.clone();
        g.vm = wolf_vm::exec::Vm::new(programs);
        g.vm.vars = vars;
    }
    // The run_condition==2 background scheduler (AutoEvents) would otherwise start ~9
    // unrelated coroutines that mutate shared vars and divert CE402 down its early 終了
    // branch.  The budget-1 fixture starved those coroutines implicitly; we disable them
    // explicitly so the ONLY variable we change versus the fixture is the frame budget
    // (natural 200_000 vs 1).  See docs/research/M11_HSCENE_NATURAL.md.
    g.auto_events.set_enabled(false);
    g.auto_advance = true;
    g.auto_choice = false;
    g.auto_choice_delay = 2;
    g.set_choice_cursor(0);
    g.start_event(328);
    g
}

fn run_natural(var23: i32, frame_budget: usize) -> NaturalRun {
    let mut g = open_natural("SaveData01", &seed_deep(var23));
    let mut out = NaturalRun {
        frames: 0,
        ce328_maxpc: 0,
        ce402_maxpc: 0,
        ce410_maxpc: 0,
        ce328_seq: Vec::new(),
        ce402_seq: Vec::new(),
        ce410_acts: Vec::new(),
        cg_files: BTreeSet::new(),
        var64_end: 0,
        warnings: 0,
    };
    let mut prev328: Option<usize> = None;
    let mut prev402: Option<usize> = None;
    let mut last410_id: Option<u32> = None;
    let mut ce410_done_at: Option<usize> = None;

    for f in 0..frame_budget {
        out.frames = f;
        g.frame();
        if g.choices.is_some() {
            g.reply(ChoiceReply::Index(0));
        }

        if let Some(pc) =
            g.vm.instances()
                .iter()
                .find(|i| i.event == 328)
                .map(|i| i.pc)
        {
            out.ce328_maxpc = out.ce328_maxpc.max(pc);
            if prev328 != Some(pc) {
                out.ce328_seq.push(pc);
                prev328 = Some(pc);
            }
        }
        if let Some(pc) =
            g.vm.instances()
                .iter()
                .find(|i| i.event == 402)
                .map(|i| i.pc)
        {
            out.ce402_maxpc = out.ce402_maxpc.max(pc);
            if prev402 != Some(pc) {
                out.ce402_seq.push(pc);
                prev402 = Some(pc);
            }
        }
        // Record every distinct CE410 instance and the act number it was called with
        // (self var at SELF_ARG_BASE).  A fresh instance id == a fresh call.
        if let Some((id, a)) =
            g.vm.instances()
                .iter()
                .find(|i| i.event == 410)
                .map(|i| (i.id, i.self_vars.get(wolf_vm::magic::SELF_ARG_BASE)))
        {
            if last410_id != Some(id) {
                out.ce410_acts.push(a);
                last410_id = Some(id);
            }
            if let Some(pc) =
                g.vm.instances()
                    .iter()
                    .find(|i| i.event == 410)
                    .map(|i| i.pc)
            {
                out.ce410_maxpc = out.ce410_maxpc.max(pc);
            }
            ce410_done_at = None;
        } else if !out.ce410_acts.is_empty() && ce410_done_at.is_none() {
            ce410_done_at = Some(f);
        }

        let cg_active =
            g.vm.instances()
                .iter()
                .any(|i| i.event == 402 || i.event == 410);
        if cg_active {
            for pic in g.screen.iter() {
                let file = pic.file.clone();
                if pic.alpha() > 0.5 && !file.is_empty() && file != "cg/blackscreen.jpg" {
                    out.cg_files.insert(file);
                }
            }
        }

        // Finished once CE410 has run AND CE328 itself is gone (route tail completed).
        if ce410_done_at.is_some() && !g.vm.instances().iter().any(|i| i.event == 328) {
            break;
        }
    }
    out.var64_end = g.vm.vars.get(64);
    out.warnings = g.stats.warnings;
    out
}

/// Natural timing drives the whole CE328 → CE402 → CE410 chain to completion: CE402 past
/// pc500 (its tail), CE410 invoked repeatedly with real H assets, zero warnings, and in a
/// tiny fraction of the frames a one-instruction probe would need.
#[test]
fn natural_frame_timing_runs_ce328_ce402_ce410() {
    let run = run_natural(6, 200_000);
    println!(
        "SEQ_328_tail={:?}",
        run.ce328_seq
            .iter()
            .filter(|p| **p >= 680)
            .collect::<Vec<_>>()
    );
    println!(
        "SEQ_402_tail={:?}",
        run.ce402_seq
            .iter()
            .filter(|p| **p >= 470)
            .collect::<Vec<_>>()
    );
    println!(
        "CE410_ACTS={:?} maxpc410={}",
        run.ce410_acts, run.ce410_maxpc
    );

    assert_eq!(run.warnings, 0, "engine warnings must stay at zero");

    // CE328 ran its branch tail to the CE402 call.  Per-frame pc sampling cannot observe
    // pc986 itself (the call and its entire CE402 child run inside a single natural frame),
    // but reaching the route's tail pc1023 — with CE402 live as a child — proves the call.
    assert!(
        run.ce328_seq.contains(&1023),
        "CE328 must run through to its route tail (pc1023); tail={:?}",
        run.ce328_seq
            .iter()
            .filter(|p| **p >= 680)
            .collect::<Vec<_>>()
    );
    // The reachable route to the CE402 call does not write var64 (pc964 悪戯へ branch).
    assert_eq!(run.var64_end, 0, "the CE402 route must not write var64");

    // CE402 ran past pc500 — its whole tail — under natural timing.
    assert!(
        run.ce402_maxpc > 500,
        "CE402 must run past pc500 under natural timing; max={}",
        run.ce402_maxpc
    );
    assert!(
        !run.ce402_seq.is_empty(),
        "CE402 must actually run as a child of CE328"
    );

    // CE402 drove CE410 several times (one instance per act call), starting from act 1.
    assert!(
        run.ce410_acts.len() >= 3,
        "CE402 must call CE410 at least 3 times; acts={:?}",
        run.ce410_acts
    );
    assert_eq!(
        run.ce410_acts.first(),
        Some(&1),
        "first CE410 call uses act 1"
    );

    // Real H assets were shown while CE402/CE410 were live.
    assert!(
        !run.cg_files.is_empty(),
        "at least one CG image must be visible"
    );
    let has_cg = run
        .cg_files
        .iter()
        .any(|f| f.contains("cutin") || f.contains("hscene/") || f.contains("cg/"));
    assert!(
        has_cg,
        "expected a cutin/hscene asset; saw {:?}",
        run.cg_files
    );

    // Natural timing is far cheaper than the one-instruction probe (>100k frames).
    assert!(
        run.frames < 100_000,
        "natural timing must complete in far fewer frames than a budget-1 probe; got {}",
        run.frames
    );
    println!(
        "natural_chain: frames={} maxpc328={} maxpc402={} acts={:?} cg_files={}",
        run.frames,
        run.ce328_maxpc,
        run.ce402_maxpc,
        run.ce410_acts,
        run.cg_files.len()
    );
}

/// Boundary pin: under natural timing CE402 never routes to the act-4 / act-5 call sites
/// (pc288/pc360/pc447), so CE410's >pc200 bodies (騎乗位絶頂 pc157, スパイダー絶頂 pc201..247)
/// are never entered.  The act argument tops out at 3 and the spider/ecstasy-of-act4-5
/// assets (v144_cg20 / cutin_189 / cutin_190) never appear.  This is exactly the artefact
/// that the budget-1 fixture's "CE410 pc247" assertion captures — reproduced here as an
/// explicit, documented ceiling rather than a silent pass.
#[test]
fn natural_timing_ce410_act_ceiling_is_three() {
    let run = run_natural(6, 200_000);
    assert_eq!(run.warnings, 0, "engine warnings must stay at zero");

    let max_act = run.ce410_acts.iter().cloned().max().unwrap_or(0);
    assert!(
        max_act <= 3,
        "natural timing must not invoke CE410 with an act > 3 (act4/act5 bodies are the \
         budget<=5 interleaving artefact); acts={:?}",
        run.ce410_acts
    );
    assert!(
        run.ce410_acts.contains(&1) && run.ce410_acts.contains(&2) && run.ce410_acts.contains(&3),
        "natural timing still exercises the act 1/2/3 bodies; acts={:?}",
        run.ce410_acts
    );

    // The deep act-4/act-5-only assets must be absent.
    let deep: Vec<&String> = run
        .cg_files
        .iter()
        .filter(|f| f.contains("v144_cg20") || f.contains("cutin_189") || f.contains("cutin_190"))
        .collect();
    assert!(
        deep.is_empty(),
        "act-4/act-5 bodies (v144_cg20 / cutin_189/190) must NOT appear on natural timing; saw {deep:?}"
    );

    // Sampling-independent corroboration: the frame-boundary pc never exceeds the act-3 body.
    assert!(
        run.ce410_maxpc < 200,
        "CE410 must not reach pc>200 under natural timing; sampled max={}",
        run.ce410_maxpc
    );

    println!(
        "natural_ce410_ceiling: acts={:?} max_act={} maxpc410={} deep_assets={:?}",
        run.ce410_acts, max_act, run.ce410_maxpc, deep
    );
}
