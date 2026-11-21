//! Semantics regression for opcode 250 (DatabaseAccess) writes of string values.
//!
//! Retail CE152 (戦闘コマンド一覧算出) reads Data type6 column 0 コマンド名 into a
//! self variable and then writes that same self variable into CData type19 column 0
//! 項目文. A self variable used this way is a *string* slot: its text lives in the
//! string bank, while its numeric bank is 0. At HEAD 1d36ebe the opcode-250 write
//! path flattened every argc>=5 value operand through eval_num, so CE152 wrote 0
//! over the command names and the 万能ウィンドウ drew blank rows.
//!
//! The fix keeps the operand as a string when it actually carries one: an explicit
//! string-bank operand (MagicKind::StrVar), or a self variable whose string bank was
//! filled by an earlier 250 read (MagicKind::SelfVar + non-empty eval_str). Every
//! other operand, including an ordinary variable holding 5, must stay numeric. The
//! tests below pin both halves so a future change cannot regress the fix (blank
//! command names) or over-apply it (numeric source turned into its decimal string).

use wolf_format::command::{Command, Slot, Tail};
use wolf_format::commonevent::{CommonEvent, CommonEventData};
use wolf_vm::program::NO_RETURN_VALUE;
use wolf_vm::{DbDirection, DbOp, DbValue, Host, ProgramSet, Vm};

/// Build one command slot. param_count counts the opcode byte plus every arg, so the
/// interpreter's argc is args.len().
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

/// Host that records every 250 op and answers every *read* with a fixed string.
#[derive(Default)]
struct DbHost {
    ops: Vec<DbOp>,
    read_answer: Option<DbValue>,
}

impl Host for DbHost {
    fn database(&mut self, op: &DbOp) -> Option<DbValue> {
        self.ops.push(op.clone());
        match op.direction {
            DbDirection::Read => self.read_answer.clone(),
            DbDirection::Write => None,
        }
    }
}

/// Flags for a 250 write: container CData (bits 8..9 = 0), op assign (bits 4..6 = 0),
/// direction bit 12 clear.
const WRITE_FLAGS: i32 = 0x0000;
/// Flags for a 250 read: same container and op, but direction bit 12 set.
const READ_FLAGS: i32 = 0x1000;

fn run_read_then_write(read_to: i32, write_value: i32) -> Vec<DbOp> {
    let commands = vec![
        // CData type19 row0 col0 -> read_to (a self/str slot), host answers "蓄力".
        cmd(250, &[19, 0, 0, READ_FLAGS, read_to], &[]),
        // Write the slot just filled -> CData type19 row5 col0.
        cmd(250, &[19, 5, 0, WRITE_FLAGS, write_value], &[]),
    ];
    let mut vm = vm_with(vec![event(1, "opcode250", commands)]);
    vm.start_event(1);
    let mut host = DbHost {
        read_answer: Some(DbValue::Str("蓄力".to_owned())),
        ..DbHost::default()
    };
    vm.step_frame(&mut host);
    host.ops
}

#[test]
fn a_self_variable_string_bank_is_written_as_a_string() {
    // CE152 shape: read コマンド名 into a self slot, then write that slot to 項目文.
    // Self slot 1_600_009 maps to flat index 609 (group 6 slot 9).
    let ops = run_read_then_write(1_600_009, 1_600_009);
    assert_eq!(ops.len(), 2, "both 250 instructions reach the host");
    assert_eq!(ops[0].direction, DbDirection::Read);
    assert_eq!(ops[0].destination, Some(1_600_009));
    assert_eq!(
        ops[1].source,
        Some(DbValue::Str("蓄力".to_owned())),
        "the self variable holds a string, so the write must carry that text, not 0"
    );
}

#[test]
fn an_explicit_string_variable_is_written_as_a_string() {
    // A string-bank operand (3_000_009) is always text; 122 stores the literal first.
    let commands = vec![
        cmd(122, &[3_000_009], &["习得"]),
        cmd(250, &[19, 6, 0, WRITE_FLAGS, 3_000_009], &[]),
    ];
    let mut vm = vm_with(vec![event(2, "opcode250", commands)]);
    vm.start_event(2);
    let mut host = DbHost::default();
    vm.step_frame(&mut host);
    let last = host.ops.last().expect("write op recorded");
    assert_eq!(last.direction, DbDirection::Write);
    assert_eq!(last.source, Some(DbValue::Str("习得".to_owned())));
}

#[test]
fn an_ordinary_variable_holding_a_number_stays_numeric() {
    // The regression m6-training reported: v1 = 5 has the decimal string "5", but an
    // ordinary variable is never a string source. The write must stay DbValue::Num(5).
    let commands = vec![
        cmd(121, &[2_000_001, 5, 0, 0], &[]), // v1 = 5
        cmd(250, &[18, 51, 0, WRITE_FLAGS, 2_000_001], &[]),
    ];
    let mut vm = vm_with(vec![event(3, "opcode250", commands)]);
    vm.start_event(3);
    let mut host = DbHost::default();
    vm.step_frame(&mut host);
    assert_eq!(
        host.ops.last().and_then(|op| op.source.clone()),
        Some(DbValue::Num(5)),
        "an ordinary variable's numeric value must not become its decimal string"
    );
}

#[test]
fn a_self_variable_without_a_string_bank_stays_numeric() {
    // 121 sets only the numeric bank of self slot 610. With no string bank the write
    // must stay numeric (eval_str is empty, so the discriminator does not fire).
    let commands = vec![
        cmd(121, &[1_600_010, 7, 0, 0], &[]), // self slot 610 = 7 (numeric only)
        cmd(250, &[0, 12, 9, WRITE_FLAGS, 1_600_010], &[]),
    ];
    let mut vm = vm_with(vec![event(4, "opcode250", commands)]);
    vm.start_event(4);
    let mut host = DbHost::default();
    vm.step_frame(&mut host);
    assert_eq!(
        host.ops.last().and_then(|op| op.source.clone()),
        Some(DbValue::Num(7)),
        "a number-only self slot must stay numeric"
    );
}
