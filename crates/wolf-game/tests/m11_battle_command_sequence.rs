//! M10 task-83: real multi-member battle command sequence on the BOSS path.
//!
//! This test drives the REAL battle entry chain CE253 -> CE239 with the rebuilt
//! command-phase wiring and its round loop enabled. The wiring starts the real CE242
//! command scripts in a caller-supplied index sequence; each index selects that
//! command's own CE242 body and drains that member's action-point pool. Between rounds
//! the driver starts the REAL CE241 turn-start event, whose pc628 is the normal-path
//! writer that resets 2000350 from 2 back to 0. The battle then settles through CE244
//! (v343 = 1 and v352 in {1, -1}) with zero script warnings.
//!
//! The test does NOT write 2000350 / 2000352 / 2000343 and does NOT fabricate a menu.
//! It only supplies command indices through Game::set_battle_command_schedule, exactly
//! the hand-off a real menu selection would perform. See src/battle_wiring.rs for the
//! rebuilt-wiring honesty contract.
//!
//! The BOSS context (2000445 = 1, 2000446 = 4000) is required: a normal battle seeds
//! its enemy gauge from party power / 8, so the first command always settles it and
//! there is no second round to observe. CE239 pc202/pc204 implement that BOSS branch.
//!
//! Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_battle_command_sequence -- --nocapture

use std::path::PathBuf;

use wolf_game::Game;

/// One CE242 command body: command index, first-PC window [lo, hi), and the party
/// action-point slot it drains. Derived from the CE242 block decode in
/// src/battle_wiring.rs (module docs) and tests/m8_battle_party.rs EXPECT table.
#[derive(Debug, Clone, Copy)]
struct BodyInfo {
    cmd: i32,
    first_lo: i32,
    first_hi_excl: i32,
    pool: usize,
}

const BODIES: [BodyInfo; 8] = [
    BodyInfo { cmd: 1, first_lo: 145, first_hi_excl: 475, pool: 0 },
    BodyInfo { cmd: 2, first_lo: 476, first_hi_excl: 761, pool: 0 },
    BodyInfo { cmd: 3, first_lo: 761, first_hi_excl: 970, pool: 1 },
    BodyInfo { cmd: 4, first_lo: 972, first_hi_excl: 1057, pool: 1 },
    BodyInfo { cmd: 5, first_lo: 1057, first_hi_excl: 1134, pool: 2 },
    BodyInfo { cmd: 6, first_lo: 1134, first_hi_excl: 1194, pool: 3 },
    BodyInfo { cmd: 7, first_lo: 1196, first_hi_excl: 1403, pool: 2 },
    BodyInfo { cmd: 8, first_lo: 1405, first_hi_excl: 1436, pool: 4 },
];

fn body(cmd: i32) -> BodyInfo {
    *BODIES
        .iter()
        .find(|b| b.cmd == cmd)
        .unwrap_or_else(|| panic!("unknown command index {cmd}"))
}

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

fn pools(game: &Game) -> [i32; 4] {
    [
        game.vm.vars.get(345),
        game.vm.vars.get(346),
        game.vm.vars.get(347),
        game.vm.vars.get(348),
    ]
}

fn count_event(game: &Game, event: u32) -> usize {
    game.vm.instances().iter().filter(|i| i.event == event).count()
}

#[derive(Debug, Clone)]
struct RoundRecord {
    index: i32,
    arg0: i32,
    first_pc: i32,
    pre: [i32; 4],
    post: [i32; 4],
    start_frame: usize,
    end_frame: usize,
}

#[derive(Debug)]
struct Trace {
    rounds: Vec<RoundRecord>,
    phase_seq: Vec<i32>,
    rounds_started: u64,
    turn_starts: u64,
    settle_frame: usize,
    settle_v352: i32,
    settle_v343: i32,
    warnings: u64,
    ce241_live_frames: usize,
    ce242_live_frames: usize,
    ce243_live_frames: usize,
    ce239_seen: bool,
    ce253_seen: bool,
    ce244_seen: bool,
}

/// Open the real SaveData01 battle chain with the BOSS gauge, enable the rebuilt
/// wiring and round loop, and drive it with the given CE242 command-index schedule.
fn drive(schedule: &[i32]) -> Trace {
    assert!(!schedule.is_empty(), "schedule must not be empty");
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    // Read-load fix removes the old spurious 506 coroutine; pin this fixture's event RNG
    // so its intended deterministic sequence is preserved.
    game.vm.rng.set_event_seed(9);
    // Synthetic battle-driver fixture: clear the retail save's common-event self banks so
    // the multi-round driver is exercised from a clean, save-independent self store.
    game.vm.clear_common_self();
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(
        game.seed_battle_databases() > 0,
        "seed_battle_databases must seed battle CData"
    );
    game.databases
        .set_text(wolf_game::db::Container::CData, 5, 16, 0, "┗ 状態リスト");
    let _ = game.start_engine_events();
    game.set_battle_wiring(true);
    game.set_battle_round_loop(true);
    game.set_battle_command_schedule(schedule.to_vec());
    // BOSS branch: CE239 pc202 reads 2000445, pc204 boosts the enemy gauge from 2000446.
    game.vm.vars.set(445, 1);
    game.vm.vars.set(446, 4000);
    assert!(game.start_event(253), "start real CE253 battle chain");

    let mut rounds: Vec<RoundRecord> = Vec::new();
    let mut last_rounds = 0u64;
    let mut last_phase = i32::MIN;
    let mut phase_seq: Vec<i32> = Vec::new();
    let mut settle_frame = 0usize;
    let mut settle_v352 = 0i32;
    let mut settle_v343 = 0i32;
    let mut settled = false;
    let mut ce241_live_frames = 0usize;
    let mut ce242_live_frames = 0usize;
    let mut ce243_live_frames = 0usize;
    let mut ce239_seen = false;
    let mut ce253_seen = false;
    let mut ce244_seen = false;

    for f in 0..40000usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();

        // A new CE242 command round was opened by the rebuilt wiring.
        let started = game.battle_wiring_rounds();
        if started != last_rounds {
            if let Some(last) = rounds.last_mut() {
                last.post = pools(&game);
                last.end_frame = f;
            }
            let index = schedule[(started as usize - 1) % schedule.len()];
            let now = pools(&game);
            rounds.push(RoundRecord {
                index,
                arg0: 0,
                first_pc: -1,
                pre: now,
                post: now,
                start_frame: f,
                end_frame: f,
            });
            last_rounds = started;
        }

        // Per-round: record CE242 arg0 (self1600000 = selected command index) and the
        // FIRST PC above the dispatch head. The first PC is the reliable body
        // discriminator; the maximum PC is not, because after its own action body CE242
        // continues through shared command-processing code (e.g. the cmd-1 run reaches
        // pc 967 long after its body ends at pc 415).
        let ce242_here = count_event(&game, 242);
        if ce242_here > 0 {
            ce242_live_frames += ce242_here;
            if let Some(last) = rounds.last_mut() {
                for inst in game.vm.instances() {
                    if inst.event == 242 {
                        if last.arg0 == 0 {
                            last.arg0 = inst.self_vars.get(600);
                        }
                        let pc = inst.pc as i32;
                        if last.first_pc < 0 && pc > 143 {
                            last.first_pc = pc;
                        }
                    }
                }
            }
        }
        ce241_live_frames += count_event(&game, 241);
        ce243_live_frames += count_event(&game, 243);
        ce239_seen |= count_event(&game, 239) > 0;
        ce253_seen |= count_event(&game, 253) > 0;
        ce244_seen |= count_event(&game, 244) > 0;

        let phase = game.vm.vars.get(350);
        if phase != last_phase {
            phase_seq.push(phase);
            last_phase = phase;
        }
        settle_v352 = game.vm.vars.get(352);
        settle_v343 = game.vm.vars.get(343);
        if settle_v343 == 1 && phase == 0 {
            settled = true;
            settle_frame = f;
            if let Some(last) = rounds.last_mut() {
                last.post = pools(&game);
                last.end_frame = f;
            }
            break;
        }
    }
    assert!(settled, "battle must settle (real CE244 result) within 40000 frames");

    // Drain the CE239 end sequence after settlement so warnings/return paths run.
    for f in 0..600usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();
    }

    Trace {
        rounds,
        phase_seq,
        rounds_started: game.battle_wiring_rounds(),
        turn_starts: game.battle_wiring_turn_starts(),
        settle_frame,
        settle_v352,
        settle_v343,
        warnings: game.stats.warnings,
        ce241_live_frames,
        ce242_live_frames,
        ce243_live_frames,
        ce239_seen,
        ce253_seen,
        ce244_seen,
    }
}

fn count_phase_resets(seq: &[i32]) -> usize {
    seq.windows(2).filter(|w| w[0] == 2 && w[1] == 0).count()
}

fn assert_rounds(trace: &Trace, expected: &[i32]) {
    assert!(
        trace.rounds.len() >= expected.len(),
        "expected at least {} command rounds, got {}",
        expected.len(),
        trace.rounds.len()
    );
    let start = [5i32, 4, 3, 2];
    assert_eq!(trace.rounds[0].pre, start, "CE239 must seed pools 5/4/3/2");
    for (i, rec) in trace.rounds.iter().take(expected.len()).enumerate() {
        let want = expected[i];
        let info = body(want);
        assert_eq!(
            rec.index, want,
            "round {i}: scheduled index must be {want}, got {}",
            rec.index
        );
        assert_eq!(
            rec.arg0, want,
            "round {i}: CE242 arg0 (self1600000) must equal the scheduled index {want}, got {}",
            rec.arg0
        );
        assert!(
            rec.first_pc >= info.first_lo && rec.first_pc < info.first_hi_excl,
            "round {i} cmd {want}: CE242 first pc {} is not inside body [{},{}); record={rec:?}",
            rec.first_pc,
            info.first_lo,
            info.first_hi_excl
        );
        if i > 0 {
            assert_eq!(
                rec.pre,
                trace.rounds[i - 1].post,
                "round {i}: pre pools must equal previous round post pools"
            );
        }
        for slot in 0..4 {
            let want_pool = if slot == info.pool {
                rec.pre[slot] - 1
            } else {
                rec.pre[slot]
            };
            assert_eq!(
                rec.post[slot], want_pool,
                "round {i} cmd {want}: pool {} must be {} (pre {:?} post {:?})",
                [345, 346, 347, 348][slot],
                want_pool,
                rec.pre,
                rec.post
            );
        }
    }
}

fn assert_common_real_event_evidence(trace: &Trace) {
    assert!(trace.ce239_seen, "real CE239 battle caller must be live");
    assert!(trace.ce253_seen, "real CE253 battle chain must be live");
    assert!(trace.ce242_live_frames > 0, "real CE242 command scripts must run");
    assert!(trace.ce243_live_frames > 0, "real CE243 enemy-action scripts must run");
    assert!(
        trace.ce241_live_frames > 0,
        "real CE241 turn-start must run between rounds"
    );
    assert_eq!(
        trace.turn_starts,
        trace.rounds_started.saturating_sub(1),
        "one real CE241 turn-start per inter-round gap: rounds={} turn_starts={}",
        trace.rounds_started,
        trace.turn_starts
    );
    // Every inter-round gap needs one CE241 reset (2 -> 0). The final settlement may add
    // one more 2 -> 0 (CE244 victory arm), but depending on the last command's path the
    // final transition can instead be 1 -> 0, so accept turn_starts or turn_starts + 1.
    let resets = count_phase_resets(&trace.phase_seq);
    assert!(
        resets >= trace.turn_starts as usize,
        "each inter-round gap must reset 2000350 2 -> 0 via CE241: turn_starts={} resets={} seq={:?}",
        trace.turn_starts,
        resets,
        trace.phase_seq
    );
    assert!(
        resets <= trace.turn_starts as usize + 1,
        "at most one extra settlement reset beyond the CE241 turns: turn_starts={} resets={} seq={:?}",
        trace.turn_starts,
        resets,
        trace.phase_seq
    );
    assert_eq!(trace.rounds.len() as u64, trace.rounds_started);
    assert_eq!(trace.settle_v343, 1, "CE244 must decide the battle");
    assert!(
        trace.settle_v352 == 1 || trace.settle_v352 == -1,
        "CE244 must write a real result 2000352 in {{1,-1}}, got {}",
        trace.settle_v352
    );
    assert_eq!(trace.warnings, 0, "no script warnings allowed");
}

#[test]
fn four_round_sequence_maps_each_command_to_its_ce242_body_and_pool() {
    let trace = drive(&[1, 2, 3, 4]);
    println!(
        "schedule [1,2,3,4] settle_frame={} rounds={} turn_starts={} phase_seq={:?} pools={:?} v352={} v343={} warnings={} ce241_live={} ce242_live={} ce243_live={} ce244_seen={}",
        trace.settle_frame,
        trace.rounds_started,
        trace.turn_starts,
        trace.phase_seq,
        trace.rounds.last().map(|r| r.post).unwrap_or([0; 4]),
        trace.settle_v352,
        trace.settle_v343,
        trace.warnings,
        trace.ce241_live_frames,
        trace.ce242_live_frames,
        trace.ce243_live_frames,
        trace.ce244_seen
    );
    for r in &trace.rounds {
        println!(
            "  round idx={} arg0={} first_pc={} frames {}..{} pre={:?} post={:?}",
            r.index, r.arg0, r.first_pc, r.start_frame, r.end_frame, r.pre, r.post
        );
    }
    assert_eq!(trace.rounds.len(), 4, "the requested four indices must be four command rounds");
    assert_rounds(&trace, &[1, 2, 3, 4]);
    assert_common_real_event_evidence(&trace);
    // Requested sequence: members 1 and 2 act twice each.
    let post = trace.rounds[3].post;
    assert_eq!(post, [3, 2, 3, 2], "the four commands drain pools 345 twice and 346 twice");
}

#[test]
fn four_member_rotation_drains_all_four_action_point_pools() {
    let trace = drive(&[1, 3, 5, 6]);
    println!(
        "schedule [1,3,5,6] settle_frame={} rounds={} turn_starts={} phase_seq={:?} pools={:?} v352={} v343={} warnings={} ce241_live={} ce242_live={} ce243_live={} ce244_seen={}",
        trace.settle_frame,
        trace.rounds_started,
        trace.turn_starts,
        trace.phase_seq,
        trace.rounds.last().map(|r| r.post).unwrap_or([0; 4]),
        trace.settle_v352,
        trace.settle_v343,
        trace.warnings,
        trace.ce241_live_frames,
        trace.ce242_live_frames,
        trace.ce243_live_frames,
        trace.ce244_seen
    );
    for r in trace.rounds.iter().take(4) {
        println!(
            "  round idx={} arg0={} first_pc={} frames {}..{} pre={:?} post={:?}",
            r.index, r.arg0, r.first_pc, r.start_frame, r.end_frame, r.pre, r.post
        );
    }
    assert!(trace.rounds.len() >= 4, "at least the four distinct member rounds must run");
    assert_rounds(&trace, &[1, 3, 5, 6]);
    assert_common_real_event_evidence(&trace);
    // The first four rounds collectively drained all four pools once: [5,4,3,2] -> [4,3,2,1].
    let after_four = trace.rounds[3].post;
    assert_eq!(
        after_four, [4, 3, 2, 1],
        "commands 1/3/5/6 must drain pools 345/346/347/348 respectively"
    );
}
