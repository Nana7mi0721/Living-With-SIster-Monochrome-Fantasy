//! Regression matrix for opcode 124 SetVariableEx (WOLF "variable op +"), the
//! long form [dst, sub, object, field] and the short form [dst, sub, field] that
//! only 0x3000 uses.
//!
//! Evidence spec: docs/OPCODE_124.md. This slice: docs/research/M7_OPCODE_124.md.
//!
//! The interpreter owns dispatch and the argc/field split; the per-attribute
//! answers come from the Host (engine_state / character_state / picture_state).
//! These tests pin the interpreter half so a refactor cannot silently drop a sub,
//! mis-split the arguments, start writing where the original writes nothing, or
//! turn the query into a blocking instruction.

use wolf_format::command::{Command, Slot, Tail};
use wolf_format::commonevent::{CommonEvent, CommonEventData};
use wolf_vm::program::NO_RETURN_VALUE;
use wolf_vm::{Host, LogLevel, ProgramSet, Vm};

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

/// Records every 124 query and serves canned answers keyed by the query, in the
/// same shape the real hosts use: None means "no opinion", which the interpreter
/// must turn into "leave the destination untouched".
#[derive(Default)]
struct Probe {
    pictures: Vec<(i32, i32)>,
    characters: Vec<(i32, i32)>,
    engines: Vec<i32>,
    logs: Vec<(LogLevel, String)>,
    picture_answers: Vec<(i32, i32, Option<i32>)>,
    character_answers: Vec<(i32, i32, Option<i32>)>,
    engine_answers: Vec<(i32, Option<i32>)>,
}

impl Probe {
    fn answer_picture(mut self, number: i32, field: i32, value: i32) -> Self {
        self.picture_answers.push((number, field, Some(value)));
        self
    }
    fn answer_character(mut self, target: i32, field: i32, value: i32) -> Self {
        self.character_answers.push((target, field, Some(value)));
        self
    }
    fn answer_engine(mut self, field: i32, value: i32) -> Self {
        self.engine_answers.push((field, Some(value)));
        self
    }
}

impl Host for Probe {
    fn picture_state(&mut self, number: i32, field: i32) -> Option<i32> {
        self.pictures.push((number, field));
        self.picture_answers
            .iter()
            .find(|(n, f, _)| *n == number && *f == field)
            .and_then(|(_, _, value)| *value)
    }
    fn character_state(&mut self, target: i32, field: i32) -> Option<i32> {
        self.characters.push((target, field));
        self.character_answers
            .iter()
            .find(|(t, f, _)| *t == target && *f == field)
            .and_then(|(_, _, value)| *value)
    }
    fn engine_state(&mut self, field: i32) -> Option<i32> {
        self.engines.push(field);
        self.engine_answers
            .iter()
            .find(|(f, _)| *f == field)
            .and_then(|(_, value)| *value)
    }
    fn log(&mut self, level: LogLevel, message: &str) {
        self.logs.push((level, message.to_owned()));
    }
}

#[test]
fn picture_subops_read_the_requested_field_and_leave_unknowns_untouched() {
    let commands = vec![
        // 0x4000 long form: [dst, sub, picture number, field]. 12004 is a real
        // five-digit picture number; it must reach the host verbatim, never be
        // truncated to a byte.
        cmd(124, &[2_000_000, 0x4000, 12_004, 8], &[]),
        // 0x4400 has its own field space but is plumbed identically. The picture
        // number here is a self variable, so the object operand must go through
        // eval_num before the host sees it.
        cmd(121, &[1_600_021, 300, 0, 0], &[]),
        cmd(124, &[2_000_001, 0x4400, 1_600_021, 2], &[]),
        // An unanswered field must leave the destination alone, not write zero.
        cmd(121, &[2_000_002, 7, 0, 0], &[]),
        cmd(124, &[2_000_002, 0x4000, 803, 77], &[]),
    ];
    let mut vm = vm_with(vec![event(10, "picture queries", commands)]);
    vm.start_event(10);
    let mut host = Probe::default()
        .answer_picture(12_004, 8, 1)
        .answer_picture(300, 2, 150);
    let report = vm.step_frame(&mut host);
    assert_eq!(vm.vars.get(0), 1, "hover answer for the five-digit picture");
    assert_eq!(vm.vars.get(1), 150, "0x4400 routed like 0x4000 for a variable object");
    assert_eq!(vm.vars.get(2), 7, "unanswered field keeps the old value");
    assert_eq!(host.pictures, vec![(12_004, 8), (300, 2), (803, 77)]);
    assert_eq!(report.running, 0);
    assert!(host.logs.is_empty(), "no diagnostic for known subs: {:?}", host.logs);
}

#[test]
fn character_subops_evaluate_the_object_and_share_one_field_space() {
    let commands = vec![
        cmd(121, &[2_000_000, 42, 0, 0], &[]),
        // obj is a normal variable reference: its value (42) is what the host sees.
        cmd(124, &[2_000_001, 0x1000, 2_000_000, 6], &[]),
        // obj -2 is the party-leader sentinel; it survives eval_num unchanged.
        cmd(124, &[2_000_002, 0x1000, -2, 7], &[]),
        // 0x1200 is the map-zoom variant of 0x1000: same field space, same host call.
        cmd(124, &[2_000_003, 0x1200, -2, 7], &[]),
    ];
    let mut vm = vm_with(vec![event(11, "character queries", commands)]);
    vm.start_event(11);
    let mut host = Probe::default()
        .answer_character(42, 6, 640)
        .answer_character(-2, 7, 520);
    vm.step_frame(&mut host);
    assert_eq!(vm.vars.get(1), 640, "obj read through eval_num");
    assert_eq!(vm.vars.get(2), 520, "negative sentinel passed through");
    assert_eq!(vm.vars.get(3), 520, "0x1200 answers like 0x1000");
    assert_eq!(host.characters, vec![(42, 6), (-2, 7), (-2, 7)]);
}

#[test]
fn engine_short_form_reads_host_state_and_field_18_from_the_interpreter() {
    let commands = vec![
        // argc 3 short form: [dst, 0x3000, field], no object slot.
        cmd(124, &[2_000_000, 0x3000, 20], &[]),
        cmd(124, &[2_000_001, 0x3000, 21], &[]),
        // field 18 => the running event number, answered by the interpreter itself
        // (+500000 for a common event), never delegated to the host.
        cmd(124, &[2_000_002, 0x3000, 18], &[]),
        cmd(121, &[2_000_003, 7, 0, 0], &[]),
        cmd(124, &[2_000_003, 0x3000, 99], &[]),
    ];
    let mut vm = vm_with(vec![event(55, "engine queries", commands)]);
    vm.start_event(55);
    let mut host = Probe::default().answer_engine(20, 640).answer_engine(21, 360);
    vm.step_frame(&mut host);
    assert_eq!(vm.vars.get(0), 640);
    assert_eq!(vm.vars.get(1), 360);
    assert_eq!(vm.vars.get(2), 500_055, "event 55 + 500000");
    assert_eq!(vm.vars.get(3), 7, "unanswered engine field keeps the old value");
    assert_eq!(
        host.engines,
        vec![20, 21, 99],
        "field 18 is answered locally, never sent to the host"
    );
}

#[test]
fn an_unknown_sub_keeps_the_destination_and_logs_one_info_line() {
    let commands = vec![
        cmd(121, &[2_000_000, 7, 0, 0], &[]),
        // 0x2000 (type 2, screen-coordinate lookups) has zero sites in this game.
        cmd(124, &[2_000_000, 0x2000, 1, 2], &[]),
    ];
    let mut vm = vm_with(vec![event(12, "unknown sub", commands)]);
    vm.start_event(12);
    let mut host = Probe::default();
    vm.step_frame(&mut host);
    assert_eq!(vm.vars.get(0), 7, "unimplemented sub leaves the destination alone");
    assert_eq!(host.logs.len(), 1, "exactly one diagnostic");
    assert_eq!(host.logs[0].0, LogLevel::Info);
    assert!(
        host.logs[0].1.contains("0x2000"),
        "diagnostic names the sub: {}",
        host.logs[0].1
    );
}

#[test]
fn the_destination_magic_routes_to_normal_system_and_self_banks() {
    let commands = vec![
        cmd(124, &[2_000_000, 0x3000, 20], &[]),  // normal variable 0
        cmd(124, &[9_000_005, 0x3000, 20], &[]),   // system variable 5
        cmd(124, &[1_600_002, 0x3000, 20], &[]),   // self variable 2 of this coroutine
        cmd(121, &[2_000_001, 1_600_002, 0, 0], &[]), // copy self 2 into normal 1
    ];
    let mut vm = vm_with(vec![event(13, "destination banks", commands)]);
    vm.start_event(13);
    let mut host = Probe::default().answer_engine(20, 111);
    vm.step_frame(&mut host);
    assert_eq!(vm.vars.get(0), 111, "normal bank");
    assert_eq!(vm.vars.get_sys(5), 111, "system bank");
    assert_eq!(vm.vars.get(1), 111, "self bank was written and copied out");
}

#[test]
fn an_unanswered_query_leaves_a_self_destination_untouched() {
    let commands = vec![
        cmd(121, &[1_600_005, 9, 0, 0], &[]),
        cmd(124, &[1_600_005, 0x4000, 12_004, 8], &[]), // the host has no answer
        cmd(121, &[2_000_000, 1_600_005, 0, 0], &[]),   // copy self 5 out for viewing
    ];
    let mut vm = vm_with(vec![event(14, "self untouched", commands)]);
    vm.start_event(14);
    let mut host = Probe::default();
    vm.step_frame(&mut host);
    assert_eq!(host.pictures, vec![(12_004, 8)], "the query was still attempted");
    assert_eq!(vm.vars.get(0), 9, "self destination untouched when the host is silent");
}

#[test]
fn a_run_of_124_queries_finishes_in_a_single_frame() {
    let commands = vec![
        cmd(124, &[2_000_000, 0x3000, 20], &[]),
        cmd(124, &[2_000_001, 0x3000, 21], &[]),
        cmd(124, &[2_000_002, 0x3000, 20], &[]),
        cmd(124, &[2_000_003, 0x3000, 21], &[]),
        cmd(124, &[2_000_004, 0x3000, 20], &[]),
        cmd(124, &[2_000_005, 0x3000, 21], &[]),
        cmd(172, &[], &[]),
    ];
    let mut vm = vm_with(vec![event(15, "no blocking", commands)]);
    vm.start_event(15);
    let mut host = Probe::default().answer_engine(20, 1).answer_engine(21, 2);
    let report = vm.step_frame(&mut host);
    assert_eq!(report.running, 0, "the whole event ran out in one frame");
    assert_eq!(report.finished, 1);
    assert_eq!(vm.vars.get(5), 2, "every query in the run executed");
}
