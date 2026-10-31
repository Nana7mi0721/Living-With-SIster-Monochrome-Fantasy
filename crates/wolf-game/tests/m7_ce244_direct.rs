//! W1 direct settlement harness for common event 244 (battle "finish judgement").
//!
//! The shipped graph never reaches CE244 (only CE239/241/242/243 call it, and CE242
//! is gated behind 2000350 == 1 which only CE242 writes). These tests enter CE244
//! directly with Game::start_event(244) under crafted state and pin the settlement
//! gates plus the resulting 2000352 (outcome) and 2000343 (settled flag).
//!
//! Observed on real data root work/all, HEAD 1d36ebe:
//!   * CE244 finishes in a single Game::frame() (about 49 steps), warnings 0, no diags.
//!   * pc79 on 2000354: 11 -> label 省略 -> 2000352=1 (win);
//!     16 -> falls into 献身/敗北 -> 2000352=-1 (lose);
//!     anything else -> pc258 -> 勝利 (261) -> 省略 (311) -> 2000352=1 (win).
//!   * Every terminal also writes 2000343=1 and 2000350=0.
//!   * CE244 never reads 2000350; a bare Vm with 2000350=1 still wins and zeroes it.
//!   * pc36 (2000349 == self 1600010) and pc44 (2000192 vs 2000193) are dead in this
//!     interpreter because the 111 value operand is compared as a raw literal, so a
//!     variable magic on the right never matches. Forcing 2000349=1600010 enters the
//!     branch; pc44 then compares 2000192 against literal 2000193 and always takes
//!     the else (败北/lose) arm.

use std::path::PathBuf;

use wolf_game::Game;
use wolf_vm::{Host, Vm};

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        return p;
    }
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    base.join(p)
}

fn open_game() -> Option<Game> {
    Game::new(data_root()).ok()
}

struct NoopHost;
impl Host for NoopHost {}

const OUTCOME: usize = 352;   // 2000352
const SETTLED: usize = 343;   // 2000343
const BOOTSTRAP: usize = 350; // 2000350
const BATTLE_FLAG: usize = 354; // 2000354

fn settle(flag: i32) -> Option<(i32, i32, i32, u64, usize)> {
    let mut game = open_game()?;
    game.vm.vars.set(BATTLE_FLAG, flag);
    assert!(game.start_event(244), "common event 244 must exist");
    let out = game.frame();
    Some((
        game.vm.vars.get(OUTCOME),
        game.vm.vars.get(SETTLED),
        game.vm.vars.get(BOOTSTRAP),
        game.stats.warnings,
        out.report.steps,
    ))
}

#[test]
fn ce244_settles_a_win_when_entered_with_no_extra_state() {
    let Some(mut game) = open_game() else {
        eprintln!("skipping: no game data at {:?}", data_root());
        return;
    };
    assert!(game.start_event(244), "common event 244 must exist");
    let out = game.frame();

    assert_eq!(out.report.running, 0, "CE244 must finish within the frame");
    assert!(out.report.finished >= 1, "CE244 must report a finished coroutine");
    assert!(out.report.steps > 0, "CE244 must execute instructions");

    assert_eq!(game.vm.vars.get(OUTCOME), 1, "zero extra state is a win");
    assert_eq!(game.vm.vars.get(SETTLED), 1, "settlement flag set");
    assert_eq!(game.vm.vars.get(BOOTSTRAP), 0, "CE244 zeroes 2000350");
    assert_eq!(game.stats.warnings, 0, "no engine warnings");
    assert!(game.diagnostics.is_empty(), "no diagnostics: {:?}", game.diagnostics);
}

#[test]
fn ce244_settles_a_loss_when_the_defeat_flag_is_set() {
    let Some((outcome, settled, bootstrap, warnings, steps)) = settle(16) else {
        eprintln!("skipping: no game data at {:?}", data_root());
        return;
    };
    assert!(steps > 0, "CE244 must execute instructions");
    assert_eq!(outcome, -1, "2000354 == 16 routes into the defeat arm");
    assert_eq!(settled, 1, "settled flag set on the loss arm");
    assert_eq!(bootstrap, 0, "CE244 zeroes 2000350 on the loss arm");
    assert_eq!(warnings, 0);
}

#[test]
fn the_outcome_is_decided_only_by_2000354() {
    // 11 is the explicit jump-to-省略 case, 16 the 献身/敗北 case, everything else
    // falls through 勝利 into 省略. All non-16 values are wins.
    let cases: [(i32, i32); 7] = [
        (0, 1),
        (7, 1),
        (11, 1),
        (13, 1),
        (16, -1),
        (20, 1),
        (255, 1),
    ];
    for (flag, want) in cases {
        let Some((outcome, settled, _bootstrap, warnings, _steps)) = settle(flag) else {
            eprintln!("skipping: no game data at {:?}", data_root());
            return;
        };
        assert_eq!(outcome, want, "2000354={flag} expects outcome {want}");
        assert_eq!(settled, 1, "2000354={flag} must set 2000343=1");
        assert_eq!(warnings, 0, "2000354={flag} produced warnings");
    }
}

#[test]
fn the_win_lose_display_branch_opens_only_on_the_raw_self_magic() {
    // pc36 is "2000349 == self 1600010". The right operand is compared as a raw
    // literal, so the real self value (11) never matches and the display branch
    // stays dead (fall-through win). Writing the magic 1600010 into 2000349 forces
    // the branch; pc44 then compares two variables and takes the else, so outcome
    // becomes the defeat arm (-1).
    let cases: [(i32, i32); 2] = [(11, 1), (1_600_010, -1)];
    for (probe, want) in cases {
        let Some(mut game) = open_game() else {
            eprintln!("skipping: no game data at {:?}", data_root());
            return;
        };
        game.vm.vars.set(349, probe);
        assert!(game.start_event(244));
        let _ = game.frame();
        assert_eq!(
            game.vm.vars.get(OUTCOME),
            want,
            "2000349={probe} expects outcome {want}"
        );
        assert_eq!(game.vm.vars.get(SETTLED), 1);
        assert_eq!(game.stats.warnings, 0);
    }
}

#[test]
fn ce244_ignores_the_2000350_bootstrap_and_zeroes_it() {
    // A bare Vm has no auto-event scheduler, isolating CE244 from the caller chain.
    // Even with 2000350 pre-set to 1, CE244 settles to a win and clears it.
    let Some(game) = open_game() else {
        eprintln!("skipping: no game data at {:?}", data_root());
        return;
    };
    let programs = game.vm.programs.clone();
    let mut vm = Vm::with_budget(programs, 1_000_000);
    vm.vars.set(BOOTSTRAP, 1);
    assert!(vm.start_event(244).is_some(), "event 244 must start");
    let mut host = NoopHost;
    for _ in 0..64 {
        let still_running = vm
            .instances()
            .iter()
            .any(|inst| format!("{:?}", inst.event) == "244");
        if !still_running {
            break;
        }
        let _ = vm.step_frame(&mut host);
    }
    assert_eq!(vm.vars.get(OUTCOME), 1, "bootstrap must not change the outcome");
    assert_eq!(vm.vars.get(SETTLED), 1);
    assert_eq!(vm.vars.get(BOOTSTRAP), 0, "CE244 clears the bootstrap variable");
}

#[test]
fn seeding_the_battle_databases_then_settling_still_yields_the_same_outcome() {
    // The task entry path calls seed_battle_databases() first (command slots, item and
    // weapon stock). CE244 settlement never reads those cells: seeding then entering
    // CE244 still produces the plain win, so the decisive context is only 2000354.
    let Some(mut game) = open_game() else {
        eprintln!("skipping: no game data at {:?}", data_root());
        return;
    };
    let cells = game.seed_battle_databases();
    assert!(cells > 0, "battle seed must write cells");
    assert!(game.start_event(244), "common event 244 must exist");
    let _ = game.frame();
    assert_eq!(game.vm.vars.get(OUTCOME), 1);
    assert_eq!(game.vm.vars.get(SETTLED), 1);
    assert_eq!(game.stats.warnings, 0);
}

