//! Semantics regression for opcode 111 (Condition), value-operand half.
//!
//! docs/research/M7_OPCODE_111.md records the static evidence: 356 of 22695
//! conditions (1.57%) compare a variable against *another variable*, e.g. CE26
//! compares a 300 call result with the CSelf argument, CE109 tests flag masks,
//! CE64 compares scroll flags and CE244 tests the display / win-lose gates.
//! Those 356 sites only make sense if the value operand is resolved through
//! eval_num. At HEAD 1d36ebe the interpreter still reads it as a raw integer
//! (exec.rs:2207), so the tests below pin the *current* literal behaviour and
//! mark the intended reference behaviour as ignored until that one-line patch is
//! applied. The patch is deliberately not applied in this task because
//! crates/wolf-vm/src/exec.rs is owned by another change (Lead coordination).

use wolf_format::command::{Command, Slot, Tail};
use wolf_format::commonevent::{CommonEvent, CommonEventData};
use wolf_vm::program::NO_RETURN_VALUE;
use wolf_vm::{Host, ProgramSet, Vm};

/// Build one command slot. param_count counts the opcode byte plus every arg, so
/// the interpreter's argc is args.len().
fn cmd(opcode: u32, args: &[i32], strings: &[&str]) -> Slot {
    let mut params = Vec::new();
    for a in args {
        params.extend_from_slice(&a.to_le_bytes());
    }
    Slot::Command(Box::new(Command {
        param_count: (args.len() + 1) as u8,
        opcode,
        params,
        branch_depth: 0,
        strings: strings.iter().map(|s| (*s).to_owned()).collect(),
        tail: Tail::None,
    }))
}

fn event(id: u32, title: &str, commands: Vec<Slot>) -> CommonEvent {
    CommonEvent {
        id,
        raw_condition: 0,
        condition_variable: 0,
        condition_value: 0,
        arg_num_count: 0,
        arg_str_count: 0,
        title: title.to_owned(),
        commands,
        memo: String::new(),
        arg_names: Vec::new(),
        arg_special_spec: Vec::new(),
        arg_string_pages: Vec::new(),
        arg_value_pages: Vec::new(),
        default_values: Vec::new(),
        color: 0,
        self_variable_names: Vec::new(),
        return_name: String::new(),
        return_value_id: NO_RETURN_VALUE,
    }
}

fn vm_with(events: Vec<CommonEvent>) -> Vm {
    let mut data = CommonEventData {
        version_header: 0x55,
        version: 0xC9,
        events,
        footer: 0x90,
    };
    data.events.sort_by_key(|e| e.id);
    Vm::new(ProgramSet::from_common_events(&data))
}

/// A Host with no opinions; the tests only observe interpreter control flow.
#[derive(Default)]
struct Silent;

impl Host for Silent {}

/// Run one synthetic event to completion and return the Vm so var values can be
/// inspected. seeds are applied before the first step.
fn run(commands: Vec<Slot>, seeds: &[(usize, i32)]) -> Vm {
    let mut vm = vm_with(vec![event(1, "condition", commands)]);
    for (index, value) in seeds {
        vm.vars.set(*index, *value);
    }
    vm.start_event(1);
    let mut host = Silent;
    vm.step_frame(&mut host);
    vm
}

/// A one-case condition block shaped like the verified if/then pattern:
///   dest = else_value
///   if (var OP value) { dest = then_value }
/// The 401 marker opens the then body, 499 closes the block.
fn guarded(var: i32, value: i32, operator: i32, dest: usize, then_value: i32, else_value: i32) -> Vec<Slot> {
    let dest_magic = 2_000_000 + dest as i32;
    vec![
        cmd(121, &[dest_magic, else_value, 0, 0], &[]),
        cmd(111, &[1, var, value, operator], &[]),
        cmd(401, &[1], &[]),
        cmd(121, &[dest_magic, then_value, 0, 0], &[]),
        cmd(499, &[], &[]),
    ]
}

#[test]
fn a_literal_value_operand_is_compared_as_an_integer() {
    // if (v0 > 5) { v2 = 1 } else { v2 = 2 }
    let taken = run(guarded(2_000_000, 5, 0, 2, 1, 2), &[(0, 9)]);
    assert_eq!(taken.vars.get(2), 1, "9 > 5 takes the then body");
    let skipped = run(guarded(2_000_000, 5, 0, 2, 1, 2), &[(0, 1)]);
    assert_eq!(skipped.vars.get(2), 2, "1 > 5 leaves the else value");
}

#[test]
fn a_variable_valued_operand_is_currently_read_as_a_literal() {
    // Both variables hold 5, so a resolved comparison (v0 == v1) is true.
    // The current interpreter compares 5 with the raw integer 2_000_001 and
    // takes the else branch. This pins the bug; it will flip to 1 after the
    // proposed eval_num patch.
    let vm = run(guarded(2_000_000, 2_000_001, 2, 3, 1, 2), &[(0, 5), (1, 5)]);
    assert_eq!(vm.vars.get(0), 5);
    assert_eq!(vm.vars.get(1), 5);
    assert_eq!(
        vm.vars.get(3),
        2,
        "raw literal semantics compares 5 with 2000001, not with v1"
    );

    // Control: the same equality works when the right side is a real literal.
    let control = run(guarded(2_000_000, 5, 2, 3, 1, 2), &[(0, 5)]);
    assert_eq!(control.vars.get(3), 1, "v0 == literal 5 is true");
}

#[test]
fn a_magic_shaped_operand_matches_only_when_the_left_side_holds_the_raw_integer() {
    // The operand 1_600_010 is the self-variable address for slot 610, but the
    // current interpreter treats it as the integer 1_600_010. It "matches" only
    // when v0 literally contains that integer.
    let matching = run(guarded(2_000_000, 1_600_010, 2, 4, 1, 2), &[(0, 1_600_010)]);
    assert_eq!(matching.vars.get(0), 1_600_010);
    assert_eq!(matching.vars.get(4), 1, "literal 1600010 == literal 1600010");
}

#[test]
fn the_ce244_display_gate_and_win_lose_compare_fall_through_under_literal_semantics() {
    // Synthetic reconstruction of the two decisive CE244 conditions.
    //
    // CE244 pc3 assigns 11 to the self slot addressed by 1_600_010; its pc36 is
    // 111 [1, 2_000_349, 1_600_010, 2] = "if (var349 == that self slot)". With
    // var349 seeded to 11 the resolved comparison is TRUE, but raw-literal
    // semantics compares 11 with 1_600_010 and falls through.
    //
    // CE244 pc44 is 111 [17, 2_000_192, 2_000_193, 0] = "if (var192 > var193)".
    // With 5 and 3 the resolved comparison is TRUE, but raw-literal semantics
    // compares 5 with 2_000_193 and falls through.
    let commands = vec![
        cmd(121, &[1_600_010, 11, 0, 0], &[]), // self slot 610 = 11
        cmd(121, &[2_000_005, 2, 0, 0], &[]),
        cmd(111, &[1, 2_000_349, 1_600_010, 2], &[]),
        cmd(401, &[1], &[]),
        cmd(121, &[2_000_005, 1, 0, 0], &[]),
        cmd(499, &[], &[]),
        cmd(121, &[2_000_006, 2, 0, 0], &[]),
        cmd(111, &[1, 2_000_192, 2_000_193, 0], &[]),
        cmd(401, &[1], &[]),
        cmd(121, &[2_000_006, 1, 0, 0], &[]),
        cmd(499, &[], &[]),
        cmd(121, &[2_000_007, 1_600_010, 0, 0], &[]), // copy self slot 610 -> v7
    ];
    let vm = run(commands, &[(349, 11), (192, 5), (193, 3)]);
    assert_eq!(vm.vars.get(349), 11, "var349 equals the self slot value");
    assert_eq!(vm.vars.get(7), 11, "the self slot really is 11");
    assert_eq!(vm.vars.get(5), 2, "pc36-like display gate falls through");
    assert_eq!(vm.vars.get(6), 2, "pc44-like win/lose compare falls through");

    // Task-31 day4 wiring: var349 is 1 while the self slot is 11 (default) or
    // 5 (set by event 20). Raw-literal compares 1 with 1_600_010; a resolved
    // compare would test 1 == 11 (or 5). BOTH are false, so pc36 falls through
    // under either semantics and is NOT the CE244 settlement blocker. The real
    // decider is pc79 (2000354 == 11 or 16). This case is kept because the
    // operand is still a variable reference by intent.
    let real = run(
        vec![
            cmd(121, &[1_600_010, 11, 0, 0], &[]),
            cmd(121, &[2_000_005, 2, 0, 0], &[]),
            cmd(111, &[1, 2_000_349, 1_600_010, 2], &[]),
            cmd(401, &[1], &[]),
            cmd(121, &[2_000_005, 1, 0, 0], &[]),
            cmd(499, &[], &[]),
            cmd(121, &[2_000_007, 1_600_010, 0, 0], &[]),
        ],
        &[(349, 1)],
    );
    assert_eq!(real.vars.get(349), 1);
    assert_eq!(real.vars.get(7), 11);
    assert_eq!(
        real.vars.get(5),
        2,
        "1 != 11, so even reference semantics cannot enter pc36 on task-31 data"
    );
}

/// Documents the behaviour that the proposed eval_num patch produces. Kept
/// ignored until the patch lands; then remove the ignore and this test must pass.
#[test]
#[ignore = "requires the proposed opcode-111 value-operand patch (docs/research/M7_OPCODE_111.md)"]
fn reference_valued_operands_resolve_before_comparison() {
    let vm = run(guarded(2_000_000, 2_000_001, 2, 3, 1, 2), &[(0, 5), (1, 5)]);
    assert_eq!(vm.vars.get(3), 1, "v0 == v1 once the operand is evaluated");

    let commands = vec![
        cmd(121, &[1_600_010, 11, 0, 0], &[]),
        cmd(121, &[2_000_005, 2, 0, 0], &[]),
        cmd(111, &[1, 2_000_349, 1_600_010, 2], &[]),
        cmd(401, &[1], &[]),
        cmd(121, &[2_000_005, 1, 0, 0], &[]),
        cmd(499, &[], &[]),
    ];
    let vm = run(commands, &[(349, 11)]);
    assert_eq!(vm.vars.get(5), 1, "var349 == self slot once resolved");
}
