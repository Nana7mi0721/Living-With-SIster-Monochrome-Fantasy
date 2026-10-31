//! Semantics regression for opcode 111 (Condition), value-operand half.
//!
//! docs/research/M7_OPCODE_111.md records the static evidence: 356 of 22695
//! conditions (1.57%) compare a variable against *another variable*, e.g. CE26
//! compares a 300 call result with the CSelf argument, CE109 tests flag masks,
//! CE64 compares scroll flags and CE244 tests the display / win-lose gates.
//! Those 356 sites only make sense if the value operand is resolved through
//! eval_num.
//!
//! task-53 (M9 W5) landed that one-line patch in crates/wolf-vm/src/exec.rs: the
//! condition handler now reads
//!   let value = eval_num(arg(cmd, 2 + 3 * i), inst, vars);
//! so a variable / self-variable / system-variable operand is dereferenced
//! before the comparison, while a plain literal (< 1_000_000) or a negative
//! Opaque sentinel is returned unchanged by eval_num. The tests below pin the
//! resolved behaviour and keep a literal-operand control so the 98.43% literal
//! path is protected against regression.

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
    // if (v0 > 5) { v2 = 1 } else { v2 = 2 }. eval_num leaves a < 1_000_000
    // literal unchanged, so the literal path is identical before/after the patch.
    let taken = run(guarded(2_000_000, 5, 0, 2, 1, 2), &[(0, 9)]);
    assert_eq!(taken.vars.get(2), 1, "9 > 5 takes the then body");
    let skipped = run(guarded(2_000_000, 5, 0, 2, 1, 2), &[(0, 1)]);
    assert_eq!(skipped.vars.get(2), 2, "1 > 5 leaves the else value");
}

#[test]
fn a_variable_valued_operand_is_resolved_before_comparison() {
    // Both variables hold 5, so the resolved comparison (v0 == v1) is true and
    // the then body runs. (Before task-53 the interpreter compared 5 with the
    // raw integer 2_000_001 and fell through; that was the bug.)
    let vm = run(guarded(2_000_000, 2_000_001, 2, 3, 1, 2), &[(0, 5), (1, 5)]);
    assert_eq!(vm.vars.get(0), 5);
    assert_eq!(vm.vars.get(1), 5);
    assert_eq!(
        vm.vars.get(3),
        1,
        "v0 == v1 once the value operand is evaluated"
    );

    // Control: equality with a real literal still works.
    let control = run(guarded(2_000_000, 5, 2, 3, 1, 2), &[(0, 5)]);
    assert_eq!(control.vars.get(3), 1, "v0 == literal 5 is true");
}

#[test]
fn a_magic_shaped_operand_is_resolved_to_its_self_variable() {
    // The operand 1_600_010 addresses self slot 610. It is now dereferenced: the
    // comparison sees the slot's *value*, not the address. Seed the slot to 7
    // (via opcode 121, [dst=1_600_010]) and v0 to 7 so v0 == slot.
    let commands = vec![
        cmd(121, &[1_600_010, 7, 0, 0], &[]), // self slot 610 = 7
        cmd(121, &[2_000_006, 2, 0, 0], &[]), // v6 = else
        cmd(111, &[1, 2_000_000, 1_600_010, 2], &[]),
        cmd(401, &[1], &[]),
        cmd(121, &[2_000_006, 1, 0, 0], &[]), // v6 = then
        cmd(499, &[], &[]),
    ];
    let matched = run(commands.clone(), &[(0, 7)]);
    assert_eq!(
        matched.vars.get(6),
        1,
        "v0(7) == self slot 610(7) once the operand is resolved"
    );

    // A raw-integer coincidence no longer matches: v0 = 1_600_010 is compared
    // against the slot value 7, not against the address itself.
    let coincidence = run(commands, &[(0, 1_600_010)]);
    assert_eq!(
        coincidence.vars.get(6),
        2,
        "the address 1_600_010 is no longer compared literally"
    );
}

#[test]
fn the_ce244_display_gate_and_win_lose_compare_resolve_under_reference_semantics() {
    // Synthetic reconstruction of the two decisive CE244 conditions.
    //
    // CE244 pc3 assigns 11 to the self slot addressed by 1_600_010; its pc36 is
    // 111 [1, 2_000_349, 1_600_010, 2] = "if (var349 == that self slot)". With
    // var349 seeded to 11 the resolved comparison is TRUE.
    //
    // CE244 pc44 is 111 [17, 2_000_192, 2_000_193, 0] = "if (var192 > var193)".
    // With 5 and 3 the resolved comparison is TRUE.
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
    assert_eq!(vm.vars.get(5), 1, "pc36-like display gate resolves");
    assert_eq!(vm.vars.get(6), 1, "pc44-like win/lose compare resolves");

    // Task-31 day4 wiring: var349 is 1 while the self slot is 11 (default) or
    // 5 (set by event 20). Under reference semantics the compare tests 1 == 11
    // (or 5): BOTH are false and pc36 still falls through. So pc36 is NOT the
    // CE244 settlement blocker in the real data; the real decider is pc79
    // (2000354 == 11 or 16). This case pins that even after the fix.
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

/// The behaviour the landed patch produces: reference-valued operands resolve
/// before the comparison.
#[test]
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
