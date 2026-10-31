//! M9 task-49: multi-member party battle — command→member/pool mapping + round-loop regression.
//!
//! ## What this proves
//!
//! CE239 seeds FOUR party action-point pools for a normal battle (variable slots
//! 2000345/2000346/2000347/2000348 = 5/4/3/2; the "single member" branch that zeroes
//! 346..348 is gated on 2000598, which only DLC event CE452 writes). CE242 dispatches on
//! its arg0 `self1600000` to EIGHT distinct command bodies (pc143/970/1194/1403). Each body
//! belongs to one party member and drains exactly that member's pool:
//!
//! * command 1/2  -> bodies pc145/pc476  -> member 1 (pool 2000345)
//! * command 3/4  -> bodies pc761/pc972  -> member 2 (pool 2000346, カナ)
//! * command 5/7  -> bodies pc1057/pc1196 -> member 3 (pool 2000347, ユイ)
//! * command 6    -> body  pc1134        -> member 4 (pool 2000348, カスミ)
//! * command 8    -> body  pc1405        -> no named pool (generic/display body)
//!
//! Test 1 drives a real battle eight times with the wiring OFF and starts CE242 by hand
//! with each command index, then asserts (a) the CE242 program counter really reaches the
//! expected command body and (b) exactly the expected member pool loses one point. This is
//! the data-level proof that the party is multi-member: four different members act via four
//! different commands.
//!
//! ## Genuine multi-round behaviour (opcode-111 reference semantics)
//!
//! Under correct opcode-111 value-operand (reference) semantics the enemy gauge 2000342 and
//! damage are large enough that a command does NOT always one-shot the enemy. Measured with a
//! hand-driven single command (wiring OFF): members 1 and 2 (commands 1-4) still drain the
//! gauge in one hit and settle (2000352 in {1,-1}, 2000343 = 1), while members 3 and 4 and the
//! generic body (commands 5-8) leave the enemy alive (2000352 = 0, 2000343 = 0) — a SECOND
//! command round would be required to finish them. The old "every single command settles"
//! premise was an artifact of the literal-operand bug that mis-sized the gauge. Test 2 shows
//! that the standard CE253 battle (fresh SaveData01 + seeded DBs) still settles in exactly one
//! command round with the round loop off, so the shipped day4/m7 path is unaffected.
//!
//! Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_battle_party -- --nocapture

use std::path::PathBuf;

use wolf_game::Game;

/// Flat self-variable index of CE242's arg0 (`self1600000`); wolf_vm::magic::SELF_ARG_BASE.
const SELF_ARG_BASE: usize = 600;

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

/// Open a fresh game poised inside a real battle, with the wiring OFF and the standard
/// battle databases seeded (party stats, command slots, item stock, section header).
fn open_battle(wiring: bool) -> Game {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let seeded = game.seed_battle_databases();
    assert!(seeded > 0, "seed_battle_databases must seed the battle cells");
    game.databases
        .set_text(wolf_game::db::Container::CData, 5, 16, 0, "┗ 状態リスト");
    let _ = game.start_engine_events();
    game.set_battle_wiring(wiring);
    let _ = game.start_event(253);
    game
}

fn pools(game: &Game) -> [i32; 4] {
    [
        game.vm.vars.get(345),
        game.vm.vars.get(346),
        game.vm.vars.get(347),
        game.vm.vars.get(348),
    ]
}

struct CmdRun {
    started: bool,
    pre: [i32; 4],
    post: [i32; 4],
    max_242_pc: i32,
    v352: i32,
    v343: i32,
    warnings: u64,
}

/// Drive to battle-active, then start CE242 by hand with the given command index and let
/// the single battle settle.
fn run_command(cmd: i32) -> CmdRun {
    let mut game = open_battle(false);
    // Advance to battle-active (2000340 != 0); the wiring is off so CE242 will not auto-run.
    let mut active = false;
    for f in 0..20000usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();
        if game.vm.vars.get(340) != 0 {
            active = true;
            break;
        }
    }
    let mut pre = [0i32; 4];
    let mut started = false;
    let mut max_242_pc = 0i32;
    let mut v352 = 0;
    let mut v343 = 0;
    if active {
        pre = pools(&game);
        if let Some(id) = game.vm.start_event(242) {
            if let Some(inst) = game.vm.instance_mut(id) {
                inst.self_vars.set(SELF_ARG_BASE, cmd);
            }
            started = true;
        }
        for f in 0..20000usize {
            game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
            let _ = game.frame();
            for inst in game.vm.instances() {
                if inst.event == 242 {
                    let pc = inst.pc as i32;
                    if pc > max_242_pc {
                        max_242_pc = pc;
                    }
                }
            }
            v352 = game.vm.vars.get(352);
            v343 = game.vm.vars.get(343);
            if v343 == 1 && game.vm.vars.get(350) == 0 {
                break;
            }
        }
    }
    CmdRun { started, pre, post: pools(&game), max_242_pc, v352, v343, warnings: game.stats.warnings }
}

/// Expected: (command, pool index that loses one point or 4 = none, body_pc_lo,
/// body_pc_hi_excl, settles_in_one_round). Members 1/2 hit hard enough to settle; members
/// 3/4 and the generic body leave the enemy alive and need a second round (reference semantics).
const EXPECT: [(i32, usize, i32, i32, bool); 8] = [
    (1, 0, 145, 475, true),
    (2, 0, 476, 761, true),
    (3, 1, 761, 970, true),
    (4, 1, 972, 1057, true),
    (5, 2, 1057, 1134, false),
    (6, 3, 1134, 1194, false),
    (7, 2, 1196, 1403, false),
    (8, 4, 1405, 1436, false), // 4 = no named member pool
];

#[test]
fn multi_member_commands_map_to_distinct_party_pools() {
    // 2000598 is DLC-only, so a normal battle seeds the four-pool branch: 5/4/3/2.
    let init = [5i32, 4, 3, 2];
    for (cmd, pool_idx, lo, hi, settles) in EXPECT {
        let r = run_command(cmd);
        assert!(r.started, "cmd {cmd}: CE242 must start once battle is active");
        assert_eq!(r.pre, init, "cmd {cmd}: CE239 must seed the four party pools 5/4/3/2");
        // (a) the command index really reaches its own command body.
        assert!(
            r.max_242_pc >= lo && r.max_242_pc < hi,
            "cmd {cmd}: CE242 pc {} not inside the expected body [{lo},{hi})",
            r.max_242_pc
        );
        // (b) exactly the expected member pool loses one point (or none for cmd 8).
        for i in 0..4 {
            let expected = if i == pool_idx { init[i] - 1 } else { init[i] };
            assert_eq!(
                r.post[i], expected,
                "cmd {cmd}: pool {} = {} (want {expected}); pre={:?} post={:?}",
                [345, 346, 347, 348][i], r.post[i], r.pre, r.post
            );
        }
        // Settlement depends on the member's damage (opcode-111 reference semantics):
        // members 1/2 settle in one command, members 3/4 and the generic body do not.
        if settles {
            assert_eq!(r.v343, 1, "cmd {cmd}: strong command must decide the battle");
            assert!(r.v352 == 1 || r.v352 == -1, "cmd {cmd}: 2000352 must be a real result, got {}", r.v352);
        } else {
            assert_eq!(r.v343, 0, "cmd {cmd}: weak command leaves the enemy alive (needs a 2nd round)");
            assert_eq!(r.v352, 0, "cmd {cmd}: no decisive result written yet, got {}", r.v352);
        }
        assert_eq!(r.warnings, 0, "cmd {cmd}: no script warnings allowed");
        println!(
            "cmd {cmd}: body_pc={} pool_delta(idx {pool_idx}) pre={:?} post={:?} v352={} v343={} warnings={}",
            r.max_242_pc, r.pre, r.post, r.v352, r.v343, r.warnings
        );
    }
}

#[test]
fn single_shot_wiring_settles_in_one_command_round() {
    // Default wiring is single-shot (round_loop off). A real battle must still settle
    // inside the first command round with exactly one CE242 start and no extra rounds.
    let mut game = open_battle(true);
    let mut v350_seq: Vec<i32> = Vec::new();
    let mut last350 = i32::MIN;
    let mut settled = false;
    let mut v352 = 0;
    let mut v343 = 0;
    for f in 0..8000usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();
        let v350 = game.vm.vars.get(350);
        if v350 != last350 {
            v350_seq.push(v350);
            last350 = v350;
        }
        v352 = game.vm.vars.get(352);
        v343 = game.vm.vars.get(343);
        if v343 == 1 && v350 == 0 {
            settled = true;
            break;
        }
    }
    // Drain the end sequence a little so CE239 can return.
    for f in 0..400usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();
    }
    println!(
        "single_shot default: v350_seq={:?} rounds={} v352={} v343={} warnings={} settled={}",
        v350_seq, game.battle_wiring_rounds(), v352, v343, game.stats.warnings, settled
    );
    assert!(settled, "battle must settle");
    assert_eq!(v343, 1, "battle decided");
    assert!(v352 == 1 || v352 == -1, "real result, got {v352}");
    assert_eq!(game.battle_wiring_rounds(), 1, "shipped battle settles in exactly one command round");
    assert_eq!(*v350_seq.first().unwrap_or(&-1), 0, "command phase starts at 0");
    assert!(v350_seq.contains(&1), "command phase 1 observed");
    assert_eq!(*v350_seq.last().unwrap_or(&-1), 0, "command phase ends back at 0");
    assert_eq!(game.stats.warnings, 0, "no warnings");
}
