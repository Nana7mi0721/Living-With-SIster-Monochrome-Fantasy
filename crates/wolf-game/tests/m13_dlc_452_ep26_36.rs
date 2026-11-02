//! M13 — DLC CE452 EP26–EP36: why the *natural* zero-arg chain stops at EP26,
//! and a fixture that drives the full chain EP0..EP36.
//!
//! CONTEXT (see docs/research/M13_DLC_452.md and docs/research/M13_DLC_452_EP26_36.md)
//! CE452 is one giant dispatcher on its self slot 10 (self10 == EP number). One
//! invocation plays exactly one EP, writes var619 = self10 @pc2, and does
//! self10 += 1 @pc6087. With the M13 common-self persistence fix a bare repeated
//! zero-arg start_event_resuming(452) advances var619 0 -> 25 naturally (EP0..EP25),
//! but then stops dead at EP26. This file pins down WHY and proves the whole chain
//! is reachable.
//!
//! WHY EP26 STOPS — STALE var395 BATTLE FLAG (not a data gate on self11)
//! EP26 (@pc4165) runs an embedded battle via 210 CommonEvent [500239, ...] @pc4184
//! whose 24-bit return value lands in self11; @pc4185 gates on self11 >= 1
//! (win -> skip the 負け penalty scene @4189..4226; lose -> play it). On an
//! accumulated single-game run that battle resolves as a LOSS (self11 = -1), the
//! 負け branch hits a party-wipe game-over (@pc4223 -> helper 500226), which clears
//! EVERY event instance (instances == 0, i.e. the whole event chain is torn down)
//! before @pc6087 can do self10 += 1. So var619 freezes at 26 and every later call
//! replays EP26.
//!
//! The single deciding variable is the GLOBAL var395 (magic 2000395):
//!   * EP17 (画竜点睛) sets var395 = 2 at dump idx 2362, as part of 由衣 stat-boost
//!     story beat, and NOTHING in any event script ever writes var395 back to 0
//!     (verified by scanning every decompiled event: the only writers of 2000395 are
//!     452 idx2362 and 301 idx2240, both = 2).
//!   * Event 243 (敵の行動 / enemy action) and 244 (終了判定 / end judgment) branch on
//!     var395 == 2, so a stale 2 makes the EP26..EP36 battles resolve as losses.
//! A fresh game has var395 = 0, so seeding self10 = 26 in a fresh game WINS EP26
//! (self11 = 1) and every EP26..EP36 completes. Clearing var395 = 0 before each EP
//! also lets the *natural* single-game chain run all the way to EP36 (var619 = 36).
//!
//! So EP26 is NOT a script-level data gate on self11: it is the ordinary battle,
//! mis-resolving because of the leaked var395==2 flag. Whether the retail engine
//! clears var395 elsewhere (engine-native battle reset) or handles the 負け branch
//! without a game-over is a production/battle-engine question, reported to the Lead;
//! it is outside this test write scope. This test only measures the content.
//!
//! Run (default, fast, deterministic):
//!   cargo test -q -p wolf-game --test m13_dlc_452_ep26_36
//! Long chain evidence (ignored by default):
//!   cargo test -q -p wolf-game --test m13_dlc_452_ep26_36 -- --ignored --nocapture

use std::path::PathBuf;

use wolf_game::Game;

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        p
    } else {
        repo_root().join(p)
    }
}

/// EP marker pc for EP 0..=36 (comment positions in the decompiled 452 dump).
const EP_PC: [i32; 37] = [
    5, 240, 402, 525, 656, 718, 825, 968, 1148, 1235, 1316, 1460, 1561, 1690, 1767, 2111, 2186,
    2381, 2733, 2834, 2914, 3445, 3484, 3700, 3838, 4027, 4165, 4513, 4596, 4750, 4889, 4998,
    5178, 5305, 5547, 5670, 5869,
];

/// self group 6 slot 10 -> flat self-vars index 600 + 10.
const SELF10: usize = 610;
/// self group 6 slot 11 -> flat self-vars index 600 + 11 (the embedded-battle
/// return value each EP 210 CommonEvent writes).
const SELF11: usize = 611;
/// GLOBAL var 395 (magic 2000395): the stale battle flag that gates EP26+.
const VAR395: usize = 395;

struct EpRun {
    var619: i32,
    maxpc: i32,
    ended: bool,
    warnings: u64,
    self10_after: i32,
    self11: i32,
}

/// Play one EP of CE452 in a FRESH game by seeding self10 = n. Optional var395
/// pre-seed lets a caller reproduce the stale-flag failure in isolation.
fn run_ep(n: i32, var395: Option<i32>, frames: usize) -> EpRun {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    game.seed_battle_databases();
    if let Some(v) = var395 {
        game.vm.vars.set(VAR395, v);
    }
    assert!(game.start_event(452), "start_event(452) returned false");
    let id = game
        .vm
        .instances()
        .iter()
        .find(|i| i.event == 452)
        .expect("CE452 instance")
        .id;
    game.vm.instance_mut(id).unwrap().self_vars.set(SELF10, n);

    let mut maxpc: i32 = -1;
    let mut ended = false;
    for f in 0..frames {
        game.key_bits = wolf_game::key::OK;
        game.frame();
        game.key_bits = 0;
        let mut live = false;
        for i in game.vm.instances() {
            if i.event == 452 {
                live = true;
                if (i.pc as i32) > maxpc {
                    maxpc = i.pc as i32;
                }
            }
        }
        if !live && f > 5 {
            ended = true;
            break;
        }
    }
    EpRun {
        var619: game.vm.vars.get(619),
        maxpc,
        ended,
        warnings: game.stats.warnings,
        self10_after: game.vm.common_self(452).map(|b| b.get(SELF10)).unwrap_or(-1),
        self11: game.vm.common_self(452).map(|b| b.get(SELF11)).unwrap_or(-1),
    }
}

/// Advance one natural (zero-arg) DLC episode in a shared game. If reset_var395
/// the stale battle flag is cleared to 0 first (the fixture), otherwise it is left
/// exactly as the previous EP left it (the authentic accumulated run).
fn play_one_natural(game: &mut Game, reset_var395: bool) -> (bool, i32, i32, i32) {
    if reset_var395 {
        game.vm.vars.set(VAR395, 0);
    }
    assert!(
        game.vm.start_event_resuming(452).is_some(),
        "start_event_resuming(452) returned None"
    );
    let mut ended = false;
    let mut maxpc: i32 = -1;
    let mut guard = 0usize;
    while guard < 40_000 {
        guard += 1;
        if !game.vm.instances().iter().any(|i| i.event == 452) {
            ended = true;
            break;
        }
        game.key_bits = wolf_game::key::OK;
        game.frame();
        game.key_bits = 0;
        for i in game.vm.instances() {
            if i.event == 452 && (i.pc as i32) > maxpc {
                maxpc = i.pc as i32;
            }
        }
    }
    let self10_after = game.vm.common_self(452).map(|b| b.get(SELF10)).unwrap_or(-1);
    let self11 = game.vm.common_self(452).map(|b| b.get(SELF11)).unwrap_or(-1);
    (ended, maxpc, self10_after, self11)
}

// ---------------------------------------------------------------------------
// (1) EP26..EP36 are each reachable and clean in a fresh game (var395 == 0).
//     This is the content proof: EP26 is NOT a hard data gate on self11.
// ---------------------------------------------------------------------------

#[test]
fn dlc452_ep26_36_each_reachable_fresh() {
    for n in 26..=36i32 {
        let r = run_ep(n, None, 9000);
        assert_eq!(r.var619, n, "EP{n}: var619 must equal self10({n})");
        assert!(r.ended, "EP{n}: event must terminate");
        assert!(
            r.maxpc >= EP_PC[n as usize],
            "EP{n}: pc {} must reach EP marker {}",
            r.maxpc,
            EP_PC[n as usize]
        );
        assert_eq!(r.warnings, 0, "EP{n}: no interpreter warnings");
        assert_eq!(
            r.self10_after,
            n + 1,
            "EP{n}: self10 must advance to {} after the EP",
            n + 1
        );
    }
}

// ---------------------------------------------------------------------------
// (2) Root cause in isolation: var395 == 2 is the single variable that turns the
//     EP26 embedded battle into a loss and blocks self10 += 1. Everything else is
//     a fresh game, so this is a controlled before/after.
// ---------------------------------------------------------------------------

#[test]
fn dlc452_ep26_stale_var395_is_the_blocker() {
    // var395 == 0 (authentic fresh state): EP26 wins and advances self10 26 -> 27.
    let ok = run_ep(26, Some(0), 9000);
    assert_eq!(ok.var619, 26);
    assert_eq!(ok.self11, 1, "var395=0: battle must be won");
    assert_eq!(ok.self10_after, 27, "var395=0: EP26 must advance self10 to 27");
    assert!(ok.maxpc >= 4510, "var395=0: EP26 win scene @4510 must be reached");
    assert!(ok.ended, "var395=0: event must terminate");

    // var395 == 2 (the leaked EP17 flag): the same EP26 battle resolves as a loss,
    // the 負け branch tears the whole game down before self10 += 1.
    let bad = run_ep(26, Some(2), 9000);
    assert_eq!(bad.var619, 26);
    assert_eq!(bad.self11, -1, "var395=2: battle must resolve as a loss, got {}", bad.self11);
    assert_eq!(
        bad.self10_after, 26,
        "var395=2: EP26 must NOT advance self10 (stuck at 26)"
    );
    assert!(
        bad.maxpc <= 4226,
        "var395=2: must abort inside the 負け branch (end_pc 4226), got pc {}",
        bad.maxpc
    );
}

// ---------------------------------------------------------------------------
// (3) LONG: the authentic natural chain (no fixture) reproduces the leak: it
//     advances var619 0 -> 26 (EP0..EP25), then EP26 stalls because var395 was
//     set to 2 by EP17 and never cleared.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "long: drives the authentic single-game CE452 chain and documents the EP26 stall"]
fn dlc452_natural_chain_stalls_at_ep26() {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    game.seed_battle_databases();

    let mut last_ok_ep = -1i32;
    let mut ep = 0i32;
    let mut stall = None;
    loop {
        let (ended, maxpc, self10_after, self11) = play_one_natural(&mut game, false);
        if !ended || self10_after != ep + 1 {
            stall = Some((ep, maxpc, self10_after, self11, game.vm.vars.get(619)));
            break;
        }
        last_ok_ep = ep;
        ep += 1;
        if ep > 36 {
            break;
        }
    }
    let (stall_ep, maxpc, self10_after, self11, var619) =
        stall.expect("authentic chain must stall (var395 leak) before EP36");
    eprintln!(
        "authentic natural chain: last completed EP{last_ok_ep}; stalled at EP{stall_ep} \
         maxpc={maxpc} self10_after={self10_after} self11={self11} var619={var619}; var395={}",
        game.vm.vars.get(VAR395)
    );
    assert_eq!(stall_ep, 26, "authentic chain must stall exactly at EP26");
    assert_eq!(last_ok_ep, 25, "EP0..EP25 must all complete naturally");
    assert_eq!(var619, 26, "var619 must have reached 26 before stalling");
    assert_eq!(self10_after, 26, "self10 must be stuck at 26 (no +1 tail)");
    assert_eq!(self11, -1, "the EP26 embedded battle must have been lost");
    assert_eq!(
        game.vm.vars.get(VAR395),
        2,
        "the leak source var395 must be 2 (set by EP17, never cleared)"
    );
}

// ---------------------------------------------------------------------------
// (4) LONG: with the stale var395 flag cleared before each EP (the fixture), the
//     SAME single-game chain runs EP0..EP36 and var619 reaches 36. Per-EP records.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "long: full 37-EP natural chain with the var395 fixture"]
fn dlc452_natural_chain_to_ep36_with_var395_fixture() {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    game.seed_battle_databases();

    println!("ep  maxpc  var619  self10_after  self11");
    let mut ep = 0i32;
    loop {
        let (ended, maxpc, self10_after, self11) = play_one_natural(&mut game, true);
        let var619 = game.vm.vars.get(619);
        println!("{ep:<3} {maxpc:<6} {var619:<7} {self10_after:<13} {self11}");
        assert!(ended, "EP{ep}: must terminate");
        assert_eq!(var619, ep, "EP{ep}: var619 must equal {ep}");
        assert_eq!(self10_after, ep + 1, "EP{ep}: self10 must advance to {}", ep + 1);
        assert!(
            maxpc >= EP_PC[ep as usize],
            "EP{ep}: pc {maxpc} must reach EP marker {}",
            EP_PC[ep as usize]
        );
        assert_eq!(game.stats.warnings, 0, "EP{ep}: no interpreter warnings");
        if ep >= 36 {
            break;
        }
        ep += 1;
    }
    assert_eq!(ep, 36, "the fixture chain must reach EP36");
    assert_eq!(game.vm.vars.get(619), 36, "var619 must reach 36");
}
