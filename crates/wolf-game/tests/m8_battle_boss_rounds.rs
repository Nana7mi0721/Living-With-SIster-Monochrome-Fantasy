//! M9 task-57: production multi-round battle driver on the BOSS path.
//!
//! ## Why the BOSS path
//!
//! The shipped enemy gauge (2000342) is seeded to party-power/8 (CE239 pc195), so the very
//! first command round zeroes it and CE244 always settles in round 1 — there is nothing for a
//! round loop to chain. CE239 has a BOSS branch at pc202 (`IF 2000445 == 1`) which instead
//! boosts the enemy gauge from 2000446 (pc204). With a gauge that survives the first command
//! round the fight genuinely needs more than one round.
//!
//! ## What the round-loop driver does (all with real events, no faked state)
//!
//! Each CE242 slot body performs one ally action, calls CE243 (enemy action) and parks
//! 2000350 = 2. CE241 (`┣ターン開始処理`) is the only normal-path writer that resets
//! 2000350 2 -> 0 (its pc628) — but it is a boot-once `ENGINE_AUTO_START` event, so the
//! shipped flow just sits at phase 2 forever after round 1. `Game::set_battle_round_loop(true)`
//! lets `BattleWiring::tick` chain rounds by: (a) starting the next CE242 command when the
//! battle starts, (b) starting the REAL CE241 turn-start once phase reaches 2 with no
//! 241/242/243 instance alive, and (c) starting the next CE242 once CE241 has reset the phase
//! to 0. The command schedule rotates through real command bodies, so different members act.
//!
//! ## What this test proves
//!
//! Starting from the real SaveData01 battle chain (CE253 -> CE239) with a BOSS gauge seeded
//! from 2000446:
//!  * >= 2 real CE242 command rounds run (`battle_wiring_rounds`),
//!  * CE241 fires between rounds and resets 2000350 2 -> 0 (observed in the phase sequence),
//!  * distinct member pools (2000345/2000346) are drained by the different commands,
//!  * the battle settles with 2000352 in {1,-1}, 2000343 = 1 and 0 script warnings.
//!
//! Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_battle_boss_rounds -- --nocapture

use std::path::PathBuf;

use wolf_game::Game;

/// 2000446 seed for the BOSS branch (CE239 pc204). Chosen so the enemy gauge survives round 1
/// but is exhausted within a handful of rounds — the battle ends in a real victory, not by
/// starving the party pools.
const BOSS_GAUGE: i32 = 4000;

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let p = PathBuf::from(raw);
    if p.is_absolute() { return p; }
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop(); b.pop();
    b.join(p)
}

fn save01() -> PathBuf {
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop(); b.pop();
    b.join("work/retail_backup/Save/SaveData01.sav")
}

/// Open a real battle with the BOSS context seeded before the battle chain runs.
fn open_boss_battle(round_loop: bool) -> Game {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    // Read-load fix removes the old spurious 506 coroutine; pin this fixture's event RNG
    // so its intended deterministic sequence is preserved.
    game.vm.rng.set_event_seed(2);
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(game.seed_battle_databases() > 0, "seed battle cells");
    game.databases.set_text(wolf_game::db::Container::CData, 5, 16, 0, "┗ 状態リスト");
    let _ = game.start_engine_events();
    game.set_battle_wiring(true);
    game.set_battle_round_loop(round_loop);
    // BOSS context: CE239 pc202 reads 2000445, pc204 boosts the gauge from 2000446.
    game.vm.vars.set(445, 1);
    game.vm.vars.set(446, BOSS_GAUGE);
    let _ = game.start_event(253);
    game
}

fn pools(game: &Game) -> [i32; 4] {
    [game.vm.vars.get(345), game.vm.vars.get(346), game.vm.vars.get(347), game.vm.vars.get(348)]
}

#[test]
fn boss_battle_runs_multiple_rounds_and_settles() {
    let mut game = open_boss_battle(true);
    let mut v350_seq: Vec<i32> = Vec::new();
    let mut last350 = i32::MIN;
    let mut max242_pc = 0i32;
    let mut pre = [0i32; 4];
    let mut have_pre = false;
    let mut settled = false;
    let mut v352 = 0;
    let mut v343 = 0;
    for f in 0..40000usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();
        if !have_pre && game.vm.vars.get(340) != 0 { pre = pools(&game); have_pre = true; }
        for inst in game.vm.instances() {
            if inst.event == 242 && inst.pc as i32 > max242_pc { max242_pc = inst.pc as i32; }
        }
        let v350 = game.vm.vars.get(350);
        if v350 != last350 { v350_seq.push(v350); last350 = v350; }
        v352 = game.vm.vars.get(352);
        v343 = game.vm.vars.get(343);
        if v343 == 1 && v350 == 0 { settled = true; break; }
    }
    for f in 0..1200usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();
    }
    let rounds = game.battle_wiring_rounds();
    let turn_starts = game.battle_wiring_turn_starts();
    let post = pools(&game);
    println!("BOSS v350_seq={:?}", v350_seq);
    println!("BOSS pre={:?} post={:?} rounds={rounds} turn_starts={turn_starts} max242_pc={max242_pc} v352={v352} v343={v343} settled={settled} warnings={}",
        pre, post, game.stats.warnings);
    assert!(have_pre, "battle must become active");
    assert!(settled, "BOSS battle must settle");
    assert!(rounds >= 2, "must run >= 2 real CE242 command rounds, got {rounds}");
    assert!(turn_starts >= 1, "CE241 turn-start must fire between rounds, got {turn_starts}");
    // CE241 reset: the phase sequence must contain a 2 -> 0 transition.
    assert!(v350_seq.windows(2).any(|w| w[0] == 2 && w[1] == 0),
        "phase must be reset 2 -> 0 by CE241, seq={v350_seq:?}");
    // Distinct member pools: the rotating command schedule must drain at least two members.
    assert_eq!(pre, [5, 4, 3, 2], "CE239 must seed the four pools 5/4/3/2, got {pre:?}");
    let drained: Vec<usize> = (0..4).filter(|&i| post[i] < pre[i]).collect();
    assert!(drained.len() >= 2, ">= 2 distinct member pools must be drained, pre={pre:?} post={post:?}");
    assert_eq!(v343, 1, "battle decided");
    assert!(v352 == 1 || v352 == -1, "real result, got {v352}");
    assert_eq!(game.stats.warnings, 0, "no warnings");
}