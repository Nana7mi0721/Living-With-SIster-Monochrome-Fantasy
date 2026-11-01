//! M13 P0-B — DLC mainline CE452 (DLC story) full EP chain, the DLC final battle
//! CE446, the DLC ending CE447 (endroll @172), and the second mainline hot-spring
//! subjugation chain CE424-CE428.
//!
//! WHY SYNTHETIC STATE
//! docs/BASELINE.json records that all eight retail saves carry var619 = 0 (and
//! var466/492/493 = 0): no retail save has any DLC progress, so the DLC cannot be
//! reached by playing a save. It must be proven from the scripts plus a synthetic
//! state, exactly as docs/HANDOFF_M12.md section 9.3 P0-B prescribes.
//!
//! HOW CE452 IS DRIVEN
//! CE452 is one giant dispatcher on its self variable slot 10 (self10). The retail
//! caller is CE251 @pc 2498, the weekday "去温泉工作" menu entry, which invokes it
//! as 210 CommonEvent [500452, 0] -> zero numeric args -> self10 = 0. Inside CE452:
//!   * @pc 2 writes var619 = self10 (the DLC chapter counter), then
//!   * ten sequential 111 blocks compare self10 against 0..39 and jump to the
//!     matching EP body (EP number == self10),
//!   * @pc 6087 does self10 += 1 and the event ends.
//! So one invocation plays exactly ONE EP. To play EP n we start CE452 and seed
//! self10 = n; var619 must come out equal to n and the pc must reach that EP's
//! marker. The seed uses self group 6 slot 10 = flat self-vars index 600 + 10 = 610.
//!
//! ENTRY WIRING (M13 common-self fix)
//! The only in-data caller passes zero numeric args, i.e. it does NOT seed self10.
//! Before the M13 common-self fix each invocation of the event got a fresh all-zero
//! self-vars bank, so self10 was reset to 0 every day and var619 never advanced
//! (an organic playthrough only ever replayed EP0). With the fix, a common event's
//! self variables persist across separate start_event invocations: self10 now
//! accumulates 0->1->... and var619 advances naturally instead of resetting to 0.
//! `dlc452_self10_persists_across_calls` proves it without injecting self10: it
//! carries var619 through EP0..EP25 (0->25). Past EP26 the script gates on other
//! DLC state (e.g. self11) that a fresh game does not have, so a bare repeated
//! zero-arg call alone cannot reach EP36 - a data-control-flow property, separate
//! from the self-vars reset bug this fix removes.
//!
//! Run: cargo test -q -p wolf-game --test m13_dlc_452.
//! Full 37-EP sweep (ignored by default):
//!   cargo test -q -p wolf-game --test m13_dlc_452 -- --ignored dlc452_full_ep_matrix

use std::path::PathBuf;
use wolf_format::command::Slot;
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

struct EpRun {
    var619: i32,
    maxpc: i32,
    ended: bool,
    warnings: u64,
    messages: u64,
    instructions: u64,
}

/// Play one EP of CE452 by seeding self10 = n, advancing until the event ends.
fn run_ep(n: i32, frames: usize) -> EpRun {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(game.start_event(452), "start_event(452) returned false");
    let id = game
        .vm
        .instances()
        .iter()
        .find(|i| i.event == 452)
        .expect("CE452 instance")
        .id;
    // self group 6 slot 10 -> flat self-vars index 600 + 10.
    game.vm.instance_mut(id).unwrap().self_vars.set(610, n);

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
        messages: game.stats.messages,
        instructions: game.stats.instructions,
    }
}

/// True when event FROM's instruction stream contains 210 CallCommonEvent
/// targeting TARGET (first u32 param of opcode 210, little-endian).
fn calls_common_event(game: &Game, from: u32, target: u32) -> bool {
    let Some(program) = game.vm.programs.event(from) else {
        return false;
    };
    for slot in &program.slots {
        if let Slot::Command(c) = slot {
            if c.opcode == 210 && c.params.len() >= 4 {
                let t = u32::from_le_bytes([c.params[0], c.params[1], c.params[2], c.params[3]]);
                if t == 500_000 + target {
                    return true;
                }
            }
        }
    }
    false
}

/// Run a standalone event and report (maxpc, warnings, messages, instructions).
fn run_event(id: u32, frames: usize) -> (i32, u64, u64, u64) {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(game.start_event(id), "start_event({id}) returned false");
    let mut maxpc: i32 = -1;
    for _ in 0..frames {
        game.key_bits = wolf_game::key::OK;
        game.frame();
        game.key_bits = 0;
        for i in game.vm.instances() {
            if i.event == id && (i.pc as i32) > maxpc {
                maxpc = i.pc as i32;
            }
        }
    }
    (
        maxpc,
        game.stats.warnings,
        game.stats.messages,
        game.stats.instructions,
    )
}

// ---------------------------------------------------------------------------
// Entry wiring: CE251 (weekday "去温泉工作") dispatches into the DLC chain.
// ---------------------------------------------------------------------------

#[test]
fn dlc452_entry_wiring_from_ce251() {
    let game = Game::new(data_root()).expect("Game::new");
    // The weekday menu entry @pc 2498 calls CE452 with zero args (self10 = 0).
    assert!(
        calls_common_event(&game, 251, 452),
        "CE251 must call CE452 (500452), the hot-spring-work entry"
    );
    assert!(calls_common_event(&game, 251, 446), "CE251 must call CE446 (DLC final battle)");
    assert!(calls_common_event(&game, 251, 447), "CE251 must call CE447 (DLC ending)");
    assert!(calls_common_event(&game, 251, 426), "CE251 must call CE426 (hot-spring subjugation)");

    let ep0 = run_ep(0, 4000);
    assert_eq!(ep0.var619, 0, "entry passes self10=0 -> var619 must be 0");
    assert!(ep0.maxpc >= EP_PC[0], "EP0 body must be reached");
}

// ---------------------------------------------------------------------------
// CE452 dispatch: EP number == self10, and var619 == self10.
// ---------------------------------------------------------------------------

#[test]
fn dlc452_dispatch_ep0_to_ep5() {
    for n in 0..=5 {
        let r = run_ep(n, 4000);
        assert_eq!(r.var619, n, "EP{n}: var619 must equal self10({n})");
        assert!(r.ended, "EP{n}: event must terminate");
        assert!(
            r.maxpc >= EP_PC[n as usize],
            "EP{n}: pc {} must reach EP marker {}",
            r.maxpc,
            EP_PC[n as usize]
        );
        assert_eq!(r.warnings, 0, "EP{n}: no interpreter warnings");
    }
}

// ---------------------------------------------------------------------------
// DLC final battle CE446 and ending CE447 (@172 endroll).
// ---------------------------------------------------------------------------

#[test]
fn dlc446_final_battle_runs_clean() {
    let (maxpc, warnings, messages, instructions) = run_event(446, 4000);
    assert!(maxpc > 0, "CE446 must execute");
    assert_eq!(warnings, 0, "CE446: no interpreter warnings");
    assert!(messages > 0, "CE446 must show dialogue");
    assert!(instructions > 1000, "CE446 must run a substantial script");
}

#[test]
fn dlc447_ending_reaches_endroll() {
    // pc 172 is the endroll marker of the DLC ending.
    let (maxpc, warnings, messages, _instructions) = run_event(447, 4000);
    assert_eq!(warnings, 0, "CE447: no interpreter warnings");
    assert!(messages > 0, "CE447 must show dialogue");
    assert!(maxpc >= 172, "CE447 must reach the endroll @172 (got pc {maxpc})");
}

// ---------------------------------------------------------------------------
// Second mainline hot-spring subjugation chain CE424-CE428.
// ---------------------------------------------------------------------------

#[test]
fn second_mainline_ce424_to_ce428_run_clean() {
    for id in [424u32, 425, 426, 427, 428] {
        let (maxpc, warnings, messages, instructions) = run_event(id, 4000);
        assert!(maxpc > 0, "CE{id} must execute");
        assert_eq!(warnings, 0, "CE{id}: no interpreter warnings");
        assert!(messages > 0, "CE{id} must show dialogue");
        assert!(instructions > 100, "CE{id} must run its script");
    }
}

// ---------------------------------------------------------------------------
// Full 37-EP sweep (long): emits the per-EP table used by the research doc.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "long: boots 37 full games, one per DLC episode"]
fn dlc452_full_ep_matrix() {
    println!("EP  var619  maxpc  marker  reached  ended  warnings  messages  instructions");
    let mut runs = Vec::new();
    for n in 0..=36i32 {
        let r = run_ep(n, 6000);
        println!(
            "{:<3} {:<7} {:<6} {:<7} {:<8} {:<6} {:<9} {:<9} {}",
            n,
            r.var619,
            r.maxpc,
            EP_PC[n as usize],
            r.maxpc >= EP_PC[n as usize],
            r.ended,
            r.warnings,
            r.messages,
            r.instructions
        );
        runs.push((n, r));
    }
    for (n, r) in runs {
        assert_eq!(r.var619, n, "EP{n}: var619 must equal self10({n})");
        assert!(r.ended, "EP{n}: event must terminate");
        assert!(
            r.maxpc >= EP_PC[n as usize],
            "EP{n}: pc {} never reached EP marker {}",
            r.maxpc,
            EP_PC[n as usize]
        );
        assert_eq!(r.warnings, 0, "EP{n}: no interpreter warnings");
    }
}

// ---------------------------------------------------------------------------
// M13 common-self fix (acceptance E): self10 now persists across separate
// start_event(452) invocations, so the DLC chapter counter var619 advances
// naturally WITHOUT injecting self10. Before the fix every call began with a
// fresh all-zero self-vars bank, so self10 (and thus var619) was stuck at 0.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "long: drives repeated zero-arg CE452 invocations in one game"]
fn dlc452_self10_persists_across_calls() {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;

    // (episode index, var619 observed, self10 in the persisted bank after the call)
    let mut observed: Vec<(i32, i32, i32)> = Vec::new();
    let mut ep = 0i32;
    loop {
        // THE regression being fixed: at entry self10 must already equal the number
        // of EPs played so far, NOT a fresh 0. This is the cross-invocation
        // persistence property, independent of CE452's own control flow below.
        let start_self10 = game.vm.common_self(452).map(|b| b.get(610)).unwrap_or(0);
        assert_eq!(
            start_self10, ep,
            "EP{ep}: self10 at entry must equal {ep} (persisted across calls), not reset to 0"
        );

        // Resume the persisted bank: the retail entry reaches CE452 through opcode 210,
        // whose callee inherits the stored self variables (spawn_child seeding).
        assert!(
            game.vm.start_event_resuming(452).is_some(),
            "start_event(452) returned false on EP{ep}"
        );
        // The retail caller passes zero numeric args; we deliberately do NOT seed
        // self10 - the point is that it accumulates naturally.

        let mut ended = false;
        let mut guard = 0usize;
        while guard < 20_000 {
            guard += 1;
            if !game.vm.instances().iter().any(|i| i.event == 452) {
                ended = true;
                break;
            }
            game.key_bits = wolf_game::key::OK;
            game.frame();
            game.key_bits = 0;
        }
        assert!(ended, "EP{ep}: CE452 must terminate");
        assert_eq!(game.stats.warnings, 0, "EP{ep}: no interpreter warnings");

        let var619 = game.vm.vars.get(619);
        assert_eq!(
            var619, ep,
            "EP{ep}: var619 must equal the persisted self10 ({ep}) - DLC counter advanced naturally"
        );
        let self10_after = game.vm.common_self(452).map(|b| b.get(610)).unwrap_or(-1);
        observed.push((ep, var619, self10_after));

        if self10_after != ep + 1 {
            // This EP's body ended the event before reaching the trailing
            // `self10 += 1` (CE452 dump idx 5760). That is a property of CE452's
            // own control flow (later EPs gate on additional DLC state such as
            // self11), NOT the self-vars reset bug: persistence is already proven
            // by start_self10 == ep at the top of every call.
            eprintln!(
                "CE452 natural accumulation stops after EP{ep}: self10 stayed {self10_after} \
                 (EP body ended before the self10 += 1 tail)"
            );
            break;
        }
        ep += 1;
        if ep > 36 {
            break;
        }
    }
    eprintln!("dlc452 natural zero-arg progression (ep, var619, self10_after): {observed:?}");
    // The fix must carry the DLC counter well past EP0. Prove at least the first
    // 26 episodes (EP0..EP25) now advance naturally with no self10 injection, and
    // that var619 genuinely counted up rather than staying 0.
    assert!(
        observed.len() >= 26,
        "CE452 natural accumulation only reached {} episodes: {observed:?}",
        observed.len()
    );
    assert!(
        observed.last().map(|o| o.1).unwrap_or(-1) >= 25,
        "var619 never advanced past 0: {observed:?}"
    );
}
