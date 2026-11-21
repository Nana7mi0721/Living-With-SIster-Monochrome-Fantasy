//! M13 P1-E — H threshold matrix: runtime-executable verification.
//!
//! Verifies the six H-gate groups documented in docs/research/M13_CONTENT_MATRIX.md §2
//! against the shipped BasicData/CommonEvent.dat, and demonstrates the boundary
//! behaviour ("threshold-1 does NOT take the branch, ==threshold DOES") three ways:
//!
//!   1. gate_bytecode_conformance — decodes the real opcode-111 command at every
//!      documented pc and pins its exact integer payload (var / value / operator).
//!   2. gate_branch_boundary_semantics — evaluates those same decoded op111 commands
//!      with the engine compare() semantics over the live VM variable store
//!      (game.vm.vars), and asserts the selected branch index flips exactly at each
//!      threshold (threshold-1 -> fallback, ==threshold -> primary branch).
//!   3. bathroom_trust_gate_spawns_ce360_at_threshold — a real end-to-end drive of
//!      CommonEvent 318 (■お風呂): with 信頼(var21)=100 the "两人一起洗" choice enters the
//!      H body CE360; with 99 it is refused and CE360 is never spawned.
//!
//! ## Why "first true wins", not "AND"
//! OPCODE_SPEC.md §5.9 and crates/wolf-vm/src/program.rs:561 condition_layout_ok:
//! an opcode-111 command carries 1 + 3*cond_count int args (arg0 & 0x0F == cond_count),
//! and the VM opens a BlockKind::Condition block: the cond_count conditions are a
//! first-true dispatch chain (case id 1..=cond_count, else at opcode 420). So a gate whose
//! payload is [2, v21,200,1, v21,0,1] means "if var21>=200 -> case1; elif var21>=0 ->
//! case2", NOT "var21>=200 && var21>=0". The primitive thresholds in the matrix are exact;
//! multi-condition gates are ordered tiers and the boundary that matters is the one where
//! the selected branch changes. The tests below assert that flip.
//!
//! Run:  WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m13_h_thresholds
//! Data: work/all (BasicData/CommonEvent.dat); save work/retail_backup/Save/SaveData01.sav.

use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_game::Game;
use wolf_vm::exec::ChoiceReply;

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
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

fn common_events() -> CommonEventData {
    let bytes = std::fs::read(data_root().join("BasicData").join("CommonEvent.dat"))
        .expect("BasicData/CommonEvent.dat");
    CommonEventData::parse(&bytes).expect("parse CommonEvent.dat")
}

fn words(cmd: &wolf_format::command::Command) -> Vec<i32> {
    cmd.params
        .chunks_exact(4)
        .map(|w| i32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        .collect()
}

// opcode-111 operator semantics, identical to crates/wolf-vm/src/exec.rs:1493 compare().
fn compare(lhs: i32, rhs: i32, operator: i32) -> bool {
    match operator {
        0 => lhs > rhs,
        1 => lhs >= rhs,
        2 => lhs == rhs,
        3 => lhs <= rhs,
        4 => lhs < rhs,
        5 => lhs != rhs,
        6 => (lhs & rhs) != 0,
        _ => lhs == rhs,
    }
}

// (variable id without the 2_000_000 base, value, operator) triples of one op111 payload.
fn decode_conditions(ints: &[i32]) -> Vec<(usize, i32, i32)> {
    let count = (ints[0] & 0x0F) as usize;
    let mut out = Vec::with_capacity(count);
    for k in 0..count {
        let b = 1 + 3 * k;
        let var = ints[b] as usize - 2_000_000;
        out.push((var, ints[b + 1], ints[b + 2]));
    }
    out
}

// First-true branch index of the decoded op111 chain (None = all false -> else / skip).
fn selected_case(ints: &[i32], var_of: impl Fn(usize) -> i32) -> Option<usize> {
    decode_conditions(ints)
        .iter()
        .position(|&(var, val, op)| compare(var_of(var), val, op))
}

fn op111_ints(data: &CommonEventData, event: u32, pc: usize) -> Vec<i32> {
    let ev = data
        .event(event)
        .unwrap_or_else(|| panic!("CE{event} exists"));
    match ev.commands.get(pc) {
        Some(Slot::Command(cmd)) => {
            assert_eq!(
                cmd.opcode, 111,
                "CE{event}@{pc} must be opcode 111 (VariableCondition)"
            );
            let ints = words(cmd);
            // Loading invariant from OPCODE_SPEC 5.9 / program.rs:561.
            assert_eq!(
                ints.len(),
                1 + 3 * ((ints[0] & 0x0F) as usize),
                "CE{event}@{pc} op111 payload length must be 1 + 3*cond_count"
            );
            ints
        }
        _ => panic!("CE{event}@{pc} is missing or blank"),
    }
}

// ----------------------------------------------------------------------------------------
// Test 1 — exact bytecode conformance for every documented H gate.
// ----------------------------------------------------------------------------------------

#[test]
fn gate_bytecode_conformance() {
    let data = common_events();
    // (event, pc, exact intArgs payload from the shipped CommonEvent.dat)
    let expected: &[(u32, usize, &[i32])] = &[
        // --- 周末 H : CE310 @159-165 (four separate single-condition menu gates) ---
        (310, 159, &[17, 2_000_016, 1, 1]),  // 精力 var16 >= 1
        (310, 161, &[1, 2_000_022, 200, 1]), // 性興味 var22 >= 200
        (310, 163, &[17, 2_000_018, 40, 1]), // 性欲 var18 >= 40
        (310, 165, &[17, 2_000_424, 3, 1]),  // 挿入経験 var424 >= 3
        // --- 泳池/海边 : CE310 @731-743 ---
        (310, 731, &[1, 2_000_265, 1, 2]), // 処女喪失 var265 == 1
        (310, 733, &[1, 2_000_455, 1, 1]), // 起こした回数 var455 >= 1
        (310, 735, &[1, 2_000_424, 1, 1]), // 挿入経験 var424 >= 1
        (310, 739, &[17, 2_000_022, 120, 1]), // 性興味 var22 >= 120
        (310, 741, &[17, 2_000_102, 30, 1]), // 発情度 var102 >= 30
        (310, 1099, &[1, 2_000_102, 60, 1]), // 発情度 var102 >= 60
        // --- 浴室 : CE318 (信頼 tiers 100/120/150; 欲求不満/発情度 2-case) ---
        (318, 213, &[17, 2_000_021, 100, 1]), // 信頼 var21 >= 100
        (318, 217, &[17, 2_000_021, 100, 1]),
        (318, 227, &[17, 2_000_021, 120, 1]), // 信頼 var21 >= 120
        (318, 237, &[17, 2_000_021, 150, 1]), // 信頼 var21 >= 150
        (318, 338, &[2, 2_000_103, 2, 1, 2_000_102, 75, 1]),
        (318, 415, &[2, 2_000_103, 2, 1, 2_000_102, 75, 1]),
        // --- 厕所 : CE323 ---
        (323, 1055, &[2, 2_000_021, 200, 1, 2_000_021, 0, 1]), // 信頼 >=200 (fallback >=0)
        (323, 1058, &[2, 2_000_023, 5, 1, 2_000_023, 0, 1]),   // 機嫌 >=5
        (
            323,
            1060,
            &[3, 2_000_022, 250, 1, 2_000_022, 150, 1, 2_000_022, 0, 1],
        ),
        (323, 1062, &[2, 2_000_222, 0, 2, 2_000_222, 1, 1]), // おしっこ観察 var222: ==0 / >=1
        (323, 1849, &[2, 2_000_131, 10, 1, 2_000_131, 0, 1]), // 妹絶頂 var131 >= 10
        (323, 1913, &[2, 2_000_022, 200, 1, 2_000_022, 0, 1]),
        // --- 夜袭 : CE335 ---
        (335, 3100, &[2, 2_000_022, 200, 1, 2_000_022, 0, 1]), // 性興味 >=200
        (335, 3416, &[2, 2_000_022, 200, 1, 2_000_022, 0, 1]),
        (335, 3444, &[2, 2_000_022, 200, 1, 2_000_022, 0, 1]),
        (335, 3470, &[2, 2_000_022, 200, 1, 2_000_022, 0, 1]),
        (335, 3102, &[2, 2_000_102, 60, 1, 2_000_102, 0, 1]), // 発情度 >=60
        (335, 3230, &[2, 2_000_102, 60, 1, 2_000_102, 0, 1]),
        (335, 3457, &[2, 2_000_102, 60, 1, 2_000_102, 0, 1]),
        (335, 2561, &[2, 2_000_102, 40, 1, 2_000_102, 0, 1]), // 発情度 >=40
        (335, 2703, &[2, 2_000_102, 40, 1, 2_000_102, 0, 1]),
        (335, 3063, &[2, 2_000_102, 40, 1, 2_000_102, 0, 1]),
        (335, 3418, &[2, 2_000_102, 40, 1, 2_000_102, 0, 1]),
        (335, 3446, &[2, 2_000_102, 40, 1, 2_000_102, 0, 1]),
        (335, 3472, &[2, 2_000_102, 40, 1, 2_000_102, 0, 1]),
        (335, 3429, &[2, 2_000_102, 50, 1, 2_000_102, 0, 1]), // 発情度 >=50
        (335, 3483, &[2, 2_000_102, 70, 1, 2_000_102, 0, 1]), // 発情度 >=70
    ];

    for &(event, pc, want) in expected {
        let got = op111_ints(&data, event, pc);
        assert_eq!(
            got, want,
            "CE{event}@{pc} op111 payload must equal the documented gate bytes"
        );
    }
    eprintln!(
        "gate_bytecode_conformance: {} gate commands pinned exactly",
        expected.len()
    );
}

// ----------------------------------------------------------------------------------------
// Test 2 — branch boundary semantics over the live VM variable store.
// ----------------------------------------------------------------------------------------

// One boundary point: variable assignments + the branch index expected to be selected.
struct Point {
    // (var id, value to write) pairs.
    set: Vec<(usize, i32)>,
    // Expected first-true branch index (None = all false -> else / skip).
    expect: Option<usize>,
}

fn assert_gate_boundaries(
    game: &mut Game,
    data: &CommonEventData,
    event: u32,
    pc: usize,
    points: &[Point],
) {
    let ints = op111_ints(data, event, pc);
    for point in points {
        // Reset every referenced variable to a neutral 0, then apply this point.
        for &(var, _, _) in &decode_conditions(&ints) {
            game.vm.vars.set(var, 0);
        }
        for &(var, val) in &point.set {
            game.vm.vars.set(var, val);
        }
        let got = selected_case(&ints, |v| game.vm.vars.get(v));
        assert_eq!(
            got, point.expect,
            "CE{event}@{pc}: branch with vars {:?} (live Vm store) must be {:?}",
            point.set, point.expect
        );
    }
}

#[test]
fn gate_branch_boundary_semantics() {
    let data = common_events();
    let mut game = Game::new(data_root()).expect("Game::new for live Vm variable store");

    // 周末 H — single-condition threshold gates: threshold-1 -> false (else), == -> case0.
    assert_gate_boundaries(
        &mut game,
        &data,
        310,
        159,
        &[
            Point {
                set: vec![(16, 0)],
                expect: None,
            },
            Point {
                set: vec![(16, 1)],
                expect: Some(0),
            },
        ],
    );
    assert_gate_boundaries(
        &mut game,
        &data,
        310,
        161,
        &[
            Point {
                set: vec![(22, 199)],
                expect: None,
            },
            Point {
                set: vec![(22, 200)],
                expect: Some(0),
            },
        ],
    );
    assert_gate_boundaries(
        &mut game,
        &data,
        310,
        163,
        &[
            Point {
                set: vec![(18, 39)],
                expect: None,
            },
            Point {
                set: vec![(18, 40)],
                expect: Some(0),
            },
        ],
    );
    assert_gate_boundaries(
        &mut game,
        &data,
        310,
        165,
        &[
            Point {
                set: vec![(424, 2)],
                expect: None,
            },
            Point {
                set: vec![(424, 3)],
                expect: Some(0),
            },
        ],
    );

    // 泳池/海边.
    assert_gate_boundaries(
        &mut game,
        &data,
        310,
        731,
        &[
            Point {
                set: vec![(265, 0)],
                expect: None,
            },
            Point {
                set: vec![(265, 1)],
                expect: Some(0),
            },
        ],
    );
    assert_gate_boundaries(
        &mut game,
        &data,
        310,
        733,
        &[
            Point {
                set: vec![(455, 0)],
                expect: None,
            },
            Point {
                set: vec![(455, 1)],
                expect: Some(0),
            },
        ],
    );
    assert_gate_boundaries(
        &mut game,
        &data,
        310,
        735,
        &[
            Point {
                set: vec![(424, 0)],
                expect: None,
            },
            Point {
                set: vec![(424, 1)],
                expect: Some(0),
            },
        ],
    );
    assert_gate_boundaries(
        &mut game,
        &data,
        310,
        739,
        &[
            Point {
                set: vec![(22, 119)],
                expect: None,
            },
            Point {
                set: vec![(22, 120)],
                expect: Some(0),
            },
        ],
    );
    assert_gate_boundaries(
        &mut game,
        &data,
        310,
        741,
        &[
            Point {
                set: vec![(102, 29)],
                expect: None,
            },
            Point {
                set: vec![(102, 30)],
                expect: Some(0),
            },
        ],
    );
    assert_gate_boundaries(
        &mut game,
        &data,
        310,
        1099,
        &[
            Point {
                set: vec![(102, 59)],
                expect: None,
            },
            Point {
                set: vec![(102, 60)],
                expect: Some(0),
            },
        ],
    );

    // 浴室 — 信頼 tiers 100 / 120 / 150 are three single-condition thresholds.
    for (pc, thr) in [(213usize, 100i32), (217, 100), (227, 120), (237, 150)] {
        assert_gate_boundaries(
            &mut game,
            &data,
            318,
            pc,
            &[
                Point {
                    set: vec![(21, thr - 1)],
                    expect: None,
                },
                Point {
                    set: vec![(21, thr)],
                    expect: Some(0),
                },
            ],
        );
    }
    // 浴室 @338/@415 — 2-case chain: 欲求不満>=2 (case0) else 発情度>=75 (case1) else none.
    for pc in [338usize, 415] {
        assert_gate_boundaries(
            &mut game,
            &data,
            318,
            pc,
            &[
                Point {
                    set: vec![(103, 1), (102, 74)],
                    expect: None,
                },
                Point {
                    set: vec![(103, 2), (102, 74)],
                    expect: Some(0),
                },
                Point {
                    set: vec![(103, 1), (102, 75)],
                    expect: Some(1),
                },
            ],
        );
    }

    // 厕所.
    assert_gate_boundaries(
        &mut game,
        &data,
        323,
        1055,
        &[
            Point {
                set: vec![(21, 199)],
                expect: Some(1),
            },
            Point {
                set: vec![(21, 200)],
                expect: Some(0),
            },
        ],
    );
    assert_gate_boundaries(
        &mut game,
        &data,
        323,
        1058,
        &[
            Point {
                set: vec![(23, 4)],
                expect: Some(1),
            },
            Point {
                set: vec![(23, 5)],
                expect: Some(0),
            },
        ],
    );
    assert_gate_boundaries(
        &mut game,
        &data,
        323,
        1060,
        &[
            Point {
                set: vec![(22, 0)],
                expect: Some(2),
            },
            Point {
                set: vec![(22, 149)],
                expect: Some(2),
            },
            Point {
                set: vec![(22, 150)],
                expect: Some(1),
            },
            Point {
                set: vec![(22, 249)],
                expect: Some(1),
            },
            Point {
                set: vec![(22, 250)],
                expect: Some(0),
            },
        ],
    );
    assert_gate_boundaries(
        &mut game,
        &data,
        323,
        1062,
        &[
            Point {
                set: vec![(222, 0)],
                expect: Some(0),
            },
            Point {
                set: vec![(222, 1)],
                expect: Some(1),
            },
        ],
    );
    assert_gate_boundaries(
        &mut game,
        &data,
        323,
        1849,
        &[
            Point {
                set: vec![(131, 9)],
                expect: Some(1),
            },
            Point {
                set: vec![(131, 10)],
                expect: Some(0),
            },
        ],
    );
    assert_gate_boundaries(
        &mut game,
        &data,
        323,
        1913,
        &[
            Point {
                set: vec![(22, 199)],
                expect: Some(1),
            },
            Point {
                set: vec![(22, 200)],
                expect: Some(0),
            },
        ],
    );

    // 夜袭 — 性興味 >=200 and 発情度 40 / 50 / 60 / 70 four tiers.
    for pc in [3100usize, 3416, 3444, 3470] {
        assert_gate_boundaries(
            &mut game,
            &data,
            335,
            pc,
            &[
                Point {
                    set: vec![(22, 199)],
                    expect: Some(1),
                },
                Point {
                    set: vec![(22, 200)],
                    expect: Some(0),
                },
            ],
        );
    }
    for (pcs, thr) in [
        (vec![3102usize, 3230, 3457], 60i32),
        (vec![2561usize, 2703, 3063, 3418, 3446, 3472], 40i32),
        (vec![3429usize], 50i32),
        (vec![3483usize], 70i32),
    ] {
        for pc in pcs {
            assert_gate_boundaries(
                &mut game,
                &data,
                335,
                pc,
                &[
                    Point {
                        set: vec![(102, thr - 1)],
                        expect: Some(1),
                    },
                    Point {
                        set: vec![(102, thr)],
                        expect: Some(0),
                    },
                ],
            );
        }
    }

    assert_eq!(game.stats.warnings, 0, "boundary evaluation must not warn");
    eprintln!(
        "gate_branch_boundary_semantics: all documented thresholds flip the branch as expected"
    );
}

// ----------------------------------------------------------------------------------------
// Test 3 — real end-to-end drive: CE318 信頼 threshold spawns the H body CE360.
// ----------------------------------------------------------------------------------------

// Drive CommonEvent 318 (■お風呂) from SaveData01, choosing "两人一起洗" at the bath
// menu, with 信頼(var21) = trust. Returns (CE360 spawned?, warnings).
fn drive_ce318_bath(trust: i32) -> (bool, u64) {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save_path("SaveData01"))
        .expect("load SaveData01");
    game.vm.clear_common_self();
    game.auto_advance = true;
    game.auto_choice = false;
    game.set_choice_cursor(0);
    // Own every pajama so the pre-bath pajama-selection loop terminates immediately.
    for v in 80..=87 {
        game.vm.vars.set(v, 1);
    }
    game.vm.vars.set(1600010, 0);
    game.vm.vars.set(21, trust);

    game.start_event(318);
    let mut saw_ce360 = false;
    for _ in 0..3000 {
        game.frame();
        if game.vm.instances().iter().any(|i| i.event == 360) {
            saw_ce360 = true;
        }
        if let Some(choice) = &game.choices {
            let options = choice.options.clone();
            let index = options
                .iter()
                .position(|o| o.contains("两人一起洗"))
                .unwrap_or(0);
            game.reply(ChoiceReply::Index(index));
        }
    }
    (saw_ce360, game.stats.warnings)
}

#[test]
fn bathroom_trust_gate_spawns_ce360_at_threshold() {
    let (at_threshold, warn_ok) = drive_ce318_bath(100);
    assert!(
        at_threshold,
        "信頼=100 must let 两人一起洗 enter the H body CE360"
    );
    assert_eq!(warn_ok, 0, "CE318 drive at 信頼=100 must not warn");

    let (below, warn_below) = drive_ce318_bath(99);
    assert!(
        !below,
        "信頼=99 must NOT spawn CE360 (差 1 就不进入 H 分支)"
    );
    assert_eq!(warn_below, 0, "CE318 drive at 信頼=99 must not warn");

    eprintln!(
        "bathroom_trust_gate_spawns_ce360: 信頼=100 -> CE360={at_threshold}, 信頼=99 -> CE360={below}"
    );
}
