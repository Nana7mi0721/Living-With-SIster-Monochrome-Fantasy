//! Regression matrix for opcode 121 compound assignment over a slot range.
//!
//! The retail Game.exe handler at 0x4aea70 treats arg[4] as an *inclusive last
//! offset* when arg[3] bit 0x10000 is set: the loop at 0x4af1b0/0x4afd58 is
//! "for esi in 0..=arg4". CE94 pc87 uses it to divide the four-slot window
//! 1600022..=1600025 (x/y/width/height) by ten. These tests pin the interpreter
//! half so the range optimisation cannot regress into a single-slot write and
//! so a malformed short form cannot read past its argument list.

use wolf_format::command::{Command, Slot, Tail};
use wolf_format::commonevent::{CommonEvent, CommonEventData};
use wolf_vm::program::NO_RETURN_VALUE;
use wolf_vm::{Host, ProgramSet, Vm};

struct NoHost;
impl Host for NoHost {}

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

/// The four-slot CE94 division: [1600022, 10, 0, 0x10400, 3] must divide
/// 1600022..=1600025 by ten, not just 1600022.
#[test]
fn inclusive_last_offset_divides_every_slot() {
    let e = event(
        1,
        "op121 range divide",
        vec![cmd(121, &[2_000_000, 10, 0, 0x1_0400, 3], &[])],
    );
    let mut vm = vm_with(vec![e]);
    vm.vars.set(0, 100);
    vm.vars.set(1, 200);
    vm.vars.set(2, 300);
    vm.vars.set(3, 400);
    vm.start_event(1).unwrap();
    vm.step_frame(&mut NoHost);
    assert_eq!(vm.vars.get(0), 10);
    assert_eq!(vm.vars.get(1), 20);
    assert_eq!(vm.vars.get(2), 30);
    assert_eq!(vm.vars.get(3), 40);
}

/// Without bit 0x10000 the fifth argument is ignored; this preserves the
/// ordinary single-slot form emitted by most of the 17,258 call sites.
#[test]
fn without_the_range_bit_only_the_first_slot_is_written() {
    let e = event(
        1,
        "op121 single",
        vec![cmd(121, &[2_000_000, 10, 0, 0x0_0400, 3], &[])],
    );
    let mut vm = vm_with(vec![e]);
    vm.vars.set(0, 100);
    vm.vars.set(1, 200);
    vm.vars.set(2, 300);
    vm.vars.set(3, 400);
    vm.start_event(1).unwrap();
    vm.step_frame(&mut NoHost);
    assert_eq!(vm.vars.get(0), 10);
    assert_eq!(vm.vars.get(1), 200, "slot 1 must be untouched");
    assert_eq!(vm.vars.get(2), 300);
    assert_eq!(vm.vars.get(3), 400);
}

/// A large range still terminates at arg4: assign 7 to all 601 slots.
#[test]
fn a_large_range_writes_arg4_plus_one_slots() {
    let e = event(
        1,
        "op121 range large",
        vec![cmd(121, &[2_000_000, 7, 0, 0x1_0000, 600], &[])],
    );
    let mut vm = vm_with(vec![e]);
    for i in 0..=600 {
        vm.vars.set(i as usize, 1);
    }
    vm.start_event(1).unwrap();
    vm.step_frame(&mut NoHost);
    for i in 0..=600 {
        assert_eq!(vm.vars.get(i as usize), 7, "slot {i} in the range");
    }
    assert_eq!(vm.vars.get(601), 0, "slot after the range must be untouched");
}

/// A short form that sets bit 0x10000 but omits arg[4] reads a missing argument
/// as zero (docs/OPCODE_SPEC.md §10), so it degrades to the single-slot form
/// instead of panicking or walking off the argument list.
#[test]
fn a_range_bit_without_arg4_degrades_to_one_slot() {
    let e = event(
        1,
        "op121 short",
        vec![cmd(121, &[2_000_000, 7, 0, 0x1_0000], &[])],
    );
    let mut vm = vm_with(vec![e]);
    vm.vars.set(0, 1);
    vm.vars.set(1, 1);
    vm.start_event(1).unwrap();
    vm.step_frame(&mut NoHost);
    assert_eq!(vm.vars.get(0), 7);
    assert_eq!(vm.vars.get(1), 1);
}
