//! M13 — DLC CE452 EP26–EP36: why an *artificially* driven chain stalls at EP26,
//! and proof that a normally-progressed party runs the full natural chain EP0..EP36.
//!
//! CONTEXT (see docs/research/M13_DLC_452.md and docs/research/M13_DLC_452_EP26_36.md)
//! CE452 is one giant dispatcher on its self slot 10 (self10 == EP number). One
//! invocation plays exactly one EP, writes var619 = self10 @pc2, and does
//! self10 += 1 @pc6087. With the M13 common-self persistence fix a repeated
//! zero-arg start_event_resuming(452) advances var619 0 -> 36 through EP0..EP36,
//! as long as the party's battle stats are realistic.
//!
//! THE EP26 STALL IS A PROBE ARTIFACT — NOT A DATA GATE, NOT A PORT BUG
//! EP26 (@pc4165) runs an embedded battle via 210 CommonEvent [500239, ...] @pc4184
//! whose return value lands in self11; @pc4185 gates on self11 >= 1 (win -> skip the
//! 負け penalty scene @4189..4226; lose -> play it, and its party-wipe helper
//! 500226 @pc4223 tears every event instance down before @pc6087 can do
//! self10 += 1, freezing var619 at 26). The outcome is decided by the party's
//! combat power vs the enemy gauge. THIS story battle uses a fixed, high enemy
//! gauge (~4500), so it REQUIRES a normally leveled party.
//!
//! The isolated probe used to start CE452 directly without ever running the
//! day-battle / leveling cycle, so its party combat power was the bare
//! seed_battle_databases default of ~15, far below 4500 -> it always lost EP26.
//! Pre-seeding realistic party stats (var 2000040..45 and 2000170..175) makes the
//! same battle a win. That is the ONLY setup needed; it models a leveled party and
//! is a legitimate battle precondition.
//!
//! var395 (magic 2000395) is NOT the cause and MUST NOT be reset:
//!   * It is a PERMANENT skill-unlock flag (由衣 learned 献身 / self-sacrifice), set
//!     to 2 at dump idx2362 (end of the EP16 -> EP17 stretch) and by 301 idx2240,
//!     and NEVER written back to 0 by any event.
//!   * Readers: status screen 234 idx210 (shows the skill icon), album 510/513, and
//!     the battle branches 243 idx905 / 244 idx88 (which add the 献身 skill to a fight).
//!   * An engine-side reset of var395 at battle end would permanently erase the
//!     skill (status/album icon gone, the EP17 learn-scene gate at 452 idx2306
//!     reverses). => NO engine change is correct or needed.
//!
//! Run (default, fast, deterministic):
//!   cargo test -q -p wolf-game --test m13_dlc_452_ep26_36
//! Long evidence (ignored by default):
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

/// self group 6 slot 10 -> flat self-vars index 600 + 10 (holds the EP number).
const SELF10: usize = 610;
/// self group 6 slot 11 -> flat self-vars index 600 + 11 (the embedded-battle
/// return value each EP's 210 CommonEvent writes).
const SELF11: usize = 611;
/// GLOBAL var 395 (magic 2000395): the PERMANENT 献身 skill-unlock flag. It is 2
/// from the end of EP16 onward and must never be reset.
const VAR395: usize = 395;

/// Party combat-stat input slots consumed by the battle helper event 500239 (CE239):
/// var 2000040..45 feed each member's power; var 2000170..175 feed the leader
/// strength used by the 献身 branch. Seeding these models a leveled party.
const PARTY_STAT_VARS: [usize; 12] = [40, 41, 42, 43, 44, 45, 170, 171, 172, 173, 174, 175];

/// Default party-stat value used for the leveled-party fixtures below. Verified:
/// any modest value (300 / 3000 / 30000) makes the EP26 fixed-gauge battle a win,
/// while the bare seed_battle_databases default (~15) loses it.
const LEVELED_STAT: i32 = 300;

fn seed_party_stats(game: &mut Game, val: i32) {
    for k in PARTY_STAT_VARS {
        game.vm.vars.set(k, val);
    }
}

struct EpRun {
    var619: i32,
    maxpc: i32,
    ended: bool,
    warnings: u64,
    self10_after: i32,
    self11: i32,
}

/// Play one EP of CE452 in a FRESH game by seeding self10 = n. Optional var395 and
/// optional leveled party stats let a caller reproduce the battle outcome cases.
fn run_ep(n: i32, var395: Option<i32>, stats: Option<i32>, frames: usize) -> EpRun {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    game.seed_battle_databases();
    if let Some(v) = var395 {
        game.vm.vars.set(VAR395, v);
    }
    if let Some(s) = stats {
        seed_party_stats(&mut game, s);
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

/// Advance one natural (zero-arg) DLC episode in a shared game. var395 and the
/// party stats are left exactly as the game currently holds them — this is the
/// authentic accumulated run, no per-EP reset.
fn play_one_natural(game: &mut Game) -> (bool, i32, i32, i32) {
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
// (1) EP26..EP36 are each reachable and clean in a fresh game with a leveled party.
// ---------------------------------------------------------------------------

#[test]
fn dlc452_ep26_36_each_reachable_fresh() {
    for n in 26..=36i32 {
        let r = run_ep(n, None, Some(LEVELED_STAT), 9000);
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
// (2) The EP26 embedded-battle outcome tracks the PARTY STATS, not var395. Holding
//     the permanent flag at its natural post-EP16 value (2), a degenerate party
//     loses and a leveled party wins — the exact opposite of a var395 "reset" hack.
// ---------------------------------------------------------------------------

#[test]
fn dlc452_ep26_battle_outcome_tracks_party_stats_not_var395() {
    // (a) var395 == 2 (natural after EP16) + degenerate/default stats -> LOSS, stalls.
    let low = run_ep(26, Some(2), None, 9000);
    assert_eq!(low.var619, 26);
    assert_eq!(
        low.self11, -1,
        "degenerate party must lose the fixed-gauge EP26 battle, got self11={}",
        low.self11
    );
    assert_eq!(low.self10_after, 26, "a loss must NOT advance self10");
    assert!(
        low.maxpc <= 4226,
        "a loss aborts inside the 負け branch (end_pc 4226), got pc {}",
        low.maxpc
    );

    // (b) var395 == 2 (same permanent flag) + leveled stats -> WIN, advances.
    let strong = run_ep(26, Some(2), Some(LEVELED_STAT), 9000);
    assert_eq!(strong.var619, 26);
    assert_eq!(
        strong.self11, 1,
        "a leveled party must win EP26 even with 献身 (var395=2), got self11={}",
        strong.self11
    );
    assert_eq!(strong.self10_after, 27, "a win advances self10 26 -> 27");
    assert!(
        strong.maxpc >= 4510,
        "a win reaches the EP26 win scene @4510, got pc {}",
        strong.maxpc
    );
    assert!(strong.ended, "the winning EP26 must terminate normally");
}

// ---------------------------------------------------------------------------
// (3) LONG — the headline proof the Lead asked for: in ONE game, with a leveled
//     party seeded once and var395 NEVER touched (no var395=0 hack), the natural
//     zero-arg chain runs EP0..EP36 and var619 reaches 36. var395 is 0 before EP16
//     and the permanent 2 from EP16 onward — proving it is a skill flag, not a
//     transient battle flag that needs clearing.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "long: full 37-EP natural chain with a leveled party (no var395 hack)"]
fn dlc452_natural_chain_to_ep36_with_leveled_party() {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    game.seed_battle_databases();
    // Model a normally-progressed (leveled) party once. var395 is NOT touched.
    seed_party_stats(&mut game, LEVELED_STAT);

    println!("ep  maxpc  var619  self10_after  self11  var395");
    let mut ep = 0i32;
    loop {
        let (ended, maxpc, s10, s11) = play_one_natural(&mut game);
        let var619 = game.vm.vars.get(619);
        let v395 = game.vm.vars.get(VAR395);
        println!("{ep:<3} {maxpc:<6} {var619:<7} {s10:<13} {s11:<6} {v395}");
        assert!(ended, "EP{ep}: must terminate");
        assert_eq!(var619, ep, "EP{ep}: var619 must equal {ep}");
        assert_eq!(s10, ep + 1, "EP{ep}: self10 must advance to {}", ep + 1);
        assert!(
            maxpc >= EP_PC[ep as usize],
            "EP{ep}: pc {maxpc} must reach EP marker {}",
            EP_PC[ep as usize]
        );
        assert_eq!(game.stats.warnings, 0, "EP{ep}: no interpreter warnings");
        if ep <= 15 {
            assert_eq!(v395, 0, "EP{ep}: var395 must still be 0 before the 献身 unlock");
        } else {
            assert_eq!(v395, 2, "EP{ep}: var395 must be the permanent 献身 flag (2)");
        }
        if ep >= 36 {
            break;
        }
        ep += 1;
    }
    assert_eq!(ep, 36, "the natural chain must reach EP36");
    assert_eq!(game.vm.vars.get(619), 36, "var619 must reach 36");
    assert_eq!(
        game.vm.vars.get(VAR395),
        2,
        "var395 stays the permanent skill flag 2 (never reset)"
    );
}

// ---------------------------------------------------------------------------
// (4) LONG — diagnostic: WITHOUT leveling the party (bare seed_battle_databases,
//     party power ~15), the same natural chain stalls exactly at EP26. This pins
//     the failure to the unleveled party, not to var395 (which is already 2 here).
// ---------------------------------------------------------------------------

#[test]
#[ignore = "long: demonstrates the probe artifact — an unleveled party stalls at EP26"]
fn dlc452_ep26_stall_is_unleveled_party_artifact() {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    game.seed_battle_databases(); // NOTE: no party stats -> degenerate party (~15 power)

    let mut last_ok_ep = -1i32;
    let mut ep = 0i32;
    let mut stall = None;
    loop {
        let (ended, maxpc, s10, s11) = play_one_natural(&mut game);
        if !ended || s10 != ep + 1 {
            stall = Some((ep, maxpc, s10, s11, game.vm.vars.get(619)));
            break;
        }
        last_ok_ep = ep;
        ep += 1;
        if ep > 36 {
            break;
        }
    }
    let (stall_ep, maxpc, s10, s11, var619) =
        stall.expect("unleveled chain must stall before EP36");
    eprintln!(
        "unleveled party: last completed EP{last_ok_ep}; stalled at EP{stall_ep}          maxpc={maxpc} self10_after={s10} self11={s11} var619={var619}; var395={}",
        game.vm.vars.get(VAR395)
    );
    assert_eq!(stall_ep, 26, "an unleveled party stalls exactly at EP26");
    assert_eq!(last_ok_ep, 25, "EP0..EP25 complete without needing battle strength");
    assert_eq!(var619, 26, "var619 reached 26 before stalling");
    assert_eq!(s10, 26, "self10 is stuck at 26 (no +1 tail)");
    assert_eq!(s11, -1, "the EP26 embedded battle was lost");
    assert_eq!(
        game.vm.vars.get(VAR395),
        2,
        "var395==2 is present but is NOT the cause — the party stats are"
    );
}
