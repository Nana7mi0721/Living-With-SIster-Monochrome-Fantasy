//! The scheduler and the opcode dispatch (docs/OPCODE_SPEC.md §5–§8).
//!
//! # Coroutines, not calls
//!
//! WOLF runs up to 108 event *instances* concurrently (docs/OPCODE_SPEC.md §7.1):
//! the 442 background common events of this game alone are 108 in practice, plus
//! every map event page and every menu. Each one is an [Instance] with its own
//! `pc` and self variables, and one frame advances all of them once.
//!
//! `210 CommonEvent` therefore does **not** push a Rust call frame: it spawns a
//! child [Instance] and marks the caller `WaitState::Child`. Three consequences
//! are worth spelling out because they are easy to get wrong:
//!
//! * recursion is bounded by the *instance list*, not by the machine stack, so a
//!   menu that calls back into its caller (real data does this) cannot overflow;
//! * the child's self variables are its own — the parent's `V6-0` is untouched;
//! * `172 TerminateEvent` inside the child ends only the child.
//!
//! # Blocking
//!
//! docs/OPCODE_SPEC.md §7.2 lists which instructions stop a coroutine for a frame
//! or until the player acts. This module encodes that table directly: a blocking
//! opcode sets [WaitState] and returns [Signal::Blocked]; everything else falls
//! through to `pc + 1` in the same frame.
//!
//! # Budget
//!
//! `170 StartLoop` is an unconditional loop and the data contains loops that only
//! leave via a variable condition. A bug in that condition must not hang the
//! process, so the frame has an instruction budget ([Vm::with_budget],
//! 200,000 by default — roughly 6x the whole title closure). Hitting it just ends
//! the frame early; the next frame resumes at the same `pc`.

use std::sync::Arc;

use wolf_format::command::{Command, RouteCommand, Slot, Tail};

use crate::magic::{Magic, MagicKind};
use crate::program::{arg, argc, BlockKind, Control, Program, ProgramSet};
use crate::rng::WolfRng;
use crate::text::{self, EscapeBank, VarLookup};
use crate::vars::{SelfVars, Variables};

/// Identifies an event (a `CommonEvent.dat` row).
pub type EventId = u32;
/// Identifies a running coroutine.
pub type InstanceId = u32;

/// The `-1000000` sentinel used by `150 Picture` for "leave this field alone".
pub const PICTURE_UNCHANGED: i32 = -1_000_000;

/// Default instructions per frame across all instances.
pub const DEFAULT_BUDGET: usize = 200_000;
/// Hard cap on concurrently running coroutines; a runaway `210` chain stops here.
pub const MAX_INSTANCES: usize = 4_096;

/// Severity for [Host::log].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Per-instruction detail (only with tracing enabled).
    Trace,
    /// State changes worth seeing in a trace.
    Info,
    /// Something the original would tolerate but we had to guess at.
    Warn,
    /// Data we cannot execute; the coroutine keeps going where possible.
    Error,
}

/// What a coroutine is waiting for.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum WaitState {
    /// Runnable.
    #[default]
    None,
    /// `180 Wait`: this many frames, counting down.
    Frames(u32),
    /// `101 Message`: until the player dismisses it.
    Message,
    /// `102 ShowChoices`: until a choice arrives; the block names the case table.
    Choice { block: usize },
    /// `210`/`300`: until the child coroutine ends.
    Child(InstanceId),
}

/// One open loop of a coroutine (docs/OPCODE_SPEC.md §4.6).
#[derive(Clone, Copy, Debug)]
pub struct LoopCtx {
    /// Index into [Program::blocks].
    pub block: usize,
    /// Remaining iterations; `-1` for an unconditional loop (`170`/`176`).
    pub remaining: i64,
}

/// A running coroutine.
#[derive(Clone, Debug)]
pub struct Instance {
    /// Unique within the [Vm].
    pub id: InstanceId,
    /// The coroutine that called us, if any.
    pub parent: Option<InstanceId>,
    /// Event id, for traces.
    pub event: EventId,
    /// Precompiled program.
    pub program: Arc<Program>,
    /// Next instruction.
    pub pc: usize,
    /// `V0-0 .. V9-99`, private to this coroutine.
    pub self_vars: SelfVars,
    /// What we are waiting for.
    pub wait: WaitState,
    /// Open loops, innermost last.
    pub loops: Vec<LoopCtx>,
    /// Where to write this coroutine's return value when it ends.
    pub return_to: Option<i32>,
}

impl Instance {
    /// Fresh coroutine at pc 0.
    #[must_use]
    pub fn new(id: InstanceId, parent: Option<InstanceId>, program: Arc<Program>) -> Self {
        let event = program.id;
        Self {
            id,
            parent,
            event,
            program,
            pc: 0,
            self_vars: SelfVars::new(),
            wait: WaitState::None,
            loops: Vec::new(),
            return_to: None,
        }
    }

    /// True when this coroutine can be advanced this frame.
    #[must_use]
    pub fn is_runnable(&self) -> bool {
        self.wait == WaitState::None
    }
}

impl VarLookup for Instance {
    fn lookup_num(&self, bank: EscapeBank, index: usize) -> i32 {
        match bank {
            EscapeBank::SelfVar => self.self_vars.get(index),
            _ => 0,
        }
    }

    fn lookup_str(&self, bank: EscapeBank, index: usize) -> String {
        match bank {
            EscapeBank::SelfVar => self.self_vars.get_str(index).to_owned(),
            _ => String::new(),
        }
    }
}

/// Reads that span the self bank and the global banks.
struct Lookup<'a> {
    inst: &'a Instance,
    vars: &'a Variables,
}

impl VarLookup for Lookup<'_> {
    fn lookup_num(&self, bank: EscapeBank, index: usize) -> i32 {
        match bank {
            EscapeBank::SelfVar => self.inst.self_vars.get(index),
            EscapeBank::Var => self.vars.get(index),
            EscapeBank::SysVar => self.vars.get_sys(index),
            // A numeric read of a string bank is a data error; WOLF yields 0.
            EscapeBank::StrVar => 0,
        }
    }

    fn lookup_str(&self, bank: EscapeBank, index: usize) -> String {
        match bank {
            EscapeBank::SelfVar => self.inst.self_vars.get_str(index).to_owned(),
            EscapeBank::Var => String::new(),
            EscapeBank::StrVar => self.vars.get_str(index).to_owned(),
            EscapeBank::SysVar => self.vars.get_sys_str(index).to_owned(),
        }
    }
}

/// A decoded `150 Picture` instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureCmd {
    /// Picture slot (WOLF calls it the "picture number").
    pub number: i32,
    /// Picture type: 0 file, 1 file-from-string-var, 2 text, 3 window-file, 4 window-string.
    pub kind: u8,
    /// What to do with the slot.
    pub action: PictureAction,
    /// Resolved file name (types 0/3) — escapes already expanded.
    pub file: String,
    /// Resolved text (types 2/4).
    pub text: String,
    /// x, y, scale-x, scale-y, opacity, rotation, blend, and the remaining slots.
    pub args: Vec<i32>,
    /// Where args measures x/y from: 0 = top-left corner, 1 = centre (bit 0x1000 of the
    /// head word). See crates/wolf-game/src/picture.rs for the measurement behind this.
    pub anchor: u8,
    /// Trailing animation slots for the long form (docs/OPCODE_SPEC.md §5.31).
    pub animate: Vec<i32>,
}

/// The `cmd` nibble of a `150 Picture` instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureAction {
    /// `cmd 0` — create/draw.
    Draw,
    /// `cmd 1` — move or animate an existing picture.
    Move,
    /// `cmd 2` — erase.
    Erase,
    /// `cmd 3` — short form action.
    Set,
}

/// Which mixer channel a `140 PlaySound` instruction addressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundChannel {
    /// Background music.
    Bgm,
    /// Sound effect.
    Se,
}

/// A decoded `140 PlaySound` instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundCmd {
    /// BGM or SE.
    pub channel: SoundChannel,
    /// Play or stop.
    pub action: SoundAction,
    /// File name, escapes expanded (empty for a stop).
    pub file: String,
    /// Volume, 0..=100.
    pub volume: i32,
    /// Fade length in frames.
    pub frames: i32,
}

/// What a `140 PlaySound` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundAction {
    /// Start playing.
    Play,
    /// Stop, optionally fading.
    Stop,
}

/// A decoded `250 DatabaseAccess` instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbOp {
    /// Database type (0..=24).
    pub db_type: i32,
    /// Row id.
    pub data_id: i32,
    /// Column id.
    pub item_id: i32,
    /// Raw addressing bitfield (bit 16 marks a write, docs/OPCODE_SPEC.md §5.44).
    pub flags: i32,
    /// Value to write, for the 5-argument form.
    pub value: Option<i32>,
}

/// What stopped (or did not stop) a coroutine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    /// Ran until the frame's instruction budget was exhausted.
    Continue,
    /// Set a [WaitState]; nothing more this frame.
    Blocked,
    /// The coroutine ended (pc ran off the end, `172`, or `173`).
    Finished,
    /// `210 CommonEvent`: spawn a child and block.
    Call {
        /// Event to call.
        target: EventId,
        /// Arguments, already evaluated.
        args: Vec<i32>,
        /// Where the child's return value goes, if the call asked for one.
        return_to: Option<i32>,
    },
    /// `300 CallByName`: same, but resolved through the event title.
    CallByName {
        /// Event title.
        name: String,
        /// Arguments, already evaluated.
        args: Vec<i32>,
        /// Where the child's return value goes, if the call asked for one.
        return_to: Option<i32>,
    },
    /// `174 ReturnToTitle`.
    ReturnToTitle,
    /// `175 EndGame`.
    EndGame,
    /// Data we could not execute; the frame continues with other coroutines.
    Error(String),
}

/// Result of advancing one coroutine.
#[derive(Clone, Debug)]
pub struct BlockResult {
    /// How it stopped.
    pub signal: Signal,
    /// Instructions executed.
    pub steps: usize,
}

/// What the platform layer has to do for the interpreter.
///
/// Every method has a default: `wolf-cli` only overrides `log` and `show_message`
/// and gets a fully working trace host, which is how the M1 acceptance test is
/// driven.
pub trait Host {
    /// `101 Message`: display `text` and keep it up until
    /// [Host::message_dismissed] returns true. `face` is the `@n` speaker prefix
    /// (20 messages carry one, docs/OPCODE_SPEC.md §5.3).
    fn show_message(&mut self, text: &str, face: Option<i32>) {
        let _ = (text, face);
    }

    /// Has the player advanced the message? Polled once per frame.
    fn message_dismissed(&mut self) -> bool {
        true
    }

    /// `105 CloseMessage`: take the message window down without waiting.
    fn close_message(&mut self) {}

    /// `102 ShowChoices`: present the options and keep them up until
    /// [Host::choice_made] answers.
    fn show_choices(&mut self, options: &[String]) {
        let _ = options;
    }

    /// Polled once per frame while a `102` is blocking.
    fn choice_made(&mut self) -> Option<ChoiceReply> {
        Some(ChoiceReply::Index(0))
    }

    /// `150 Picture`.
    fn picture(&mut self, cmd: &PictureCmd) {
        let _ = cmd;
    }

    /// `140 PlaySound`.
    fn sound(&mut self, cmd: &SoundCmd) {
        let _ = cmd;
    }

    /// `250 DatabaseAccess`. Reads are unimplemented until wolf-game owns a
    /// database; until then the host just records the access.
    fn database(&mut self, op: &DbOp) {
        let _ = op;
    }

    /// `201 SetMoveRoute`.
    fn move_route(&mut self, target: i32, route: &[RouteCommand]) {
        let _ = (target, route);
    }

    /// `130 Teleport`.
    fn teleport(&mut self, map: i32, x: i32, y: i32, direction: i32) {
        let _ = (map, x, y, direction);
    }

    /// `160 Transition`.
    fn transition(&mut self, kind: i32, flags: i32) {
        let _ = (kind, flags);
    }

    /// `270 Party` (add/remove/reorder party members).
    fn party(&mut self, args: &[i32]) {
        let _ = args;
    }

    /// `281 Scroll` (camera scroll).
    fn scroll(&mut self, args: &[i32]) {
        let _ = args;
    }

    /// `290 Effect` (screen effects).
    fn effect(&mut self, kind: i32, duration: i32, args: &[i32]) {
        let _ = (kind, duration, args);
    }

    /// `220 SaveLoad`, `221`, `222`.
    fn save_load(&mut self, args: &[i32]) {
        let _ = args;
    }

    /// `211 CommonEventReserve`: schedule an event to start after this one ends.
    fn reserve(&mut self, event: EventId) {
        let _ = event;
    }

    /// Reads one attribute of a picture for `124 SetVariableEx` (`0x4000` / `0x4400`).
    ///
    /// Returns `None` when the host has no opinion, which makes the interpreter leave
    /// the destination variable untouched - the same behaviour the original engine has
    /// for a picture number that does not exist (docs/OPCODE_124.md §3.1).
    fn picture_state(&mut self, number: i32, field: i32) -> Option<i32> {
        let _ = (number, field);
        None
    }

    /// Reads one attribute of a map event or party member for `124` (`0x1000` / `0x1200`).
    ///
    /// The map layer only arrives in M2, so the default is "unknown".
    fn character_state(&mut self, target: i32, field: i32) -> Option<i32> {
        let _ = (target, field);
        None
    }

    /// Reads one engine attribute for `124` (`0x3000`): mouse, map id, BGM id, ...
    ///
    /// `field 18` (the event being processed) is answered by the interpreter itself, so
    /// hosts never have to know their own event id.
    fn engine_state(&mut self, field: i32) -> Option<i32> {
        let _ = field;
        None
    }

    /// Key code a script reads for opcode 123, given the poll's key `mask`.
    ///
    /// The value is a CODE, not a bit pattern: 10 decide, 11 cancel, 12 sub, and the
    /// numpad direction codes (2 down, 4 left, 6 right, 8 up), 0 when nothing is
    /// held. The evidence is the developers' own debug comments next to the
    /// comparisons in this game (crates/wolf-game/src/key.rs lists them).
    ///
    /// What the mask selects is still unknown, which is why it is handed to the host
    /// instead of being applied here: one poll can yield six different codes.
    /// Headless hosts return 0, which reads exactly like "nobody is touching the
    /// keyboard".
    fn key_code(&mut self, mask: u32) -> i32 {
        let _ = mask;
        0
    }

    /// `106 DebugMessage`.
    fn debug_message(&mut self, text: &str) {
        let _ = text;
    }

    /// `107 ClearDebugMessage`.
    fn clear_debug(&mut self) {}

    /// `174 ReturnToTitle`.
    fn return_to_title(&mut self) {}

    /// `175 EndGame`.
    fn end_game(&mut self) {}

    /// `1000 ProFeature`.
    fn pro_feature(&mut self, kind: i32, milliseconds: i32) {
        let _ = (kind, milliseconds);
    }

    /// Any diagnostic the interpreter wants to surface.
    fn log(&mut self, level: LogLevel, message: &str) {
        let _ = (level, message);
    }
}

/// Answer to a blocking `102 ShowChoices`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceReply {
    /// Option index, 0-based.
    Index(usize),
    /// The player cancelled out of the window.
    Cancel,
}

/// Aggregate result of one frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StepReport {
    /// Frame counter (starts at 1).
    pub frame: u64,
    /// Instructions executed across all coroutines.
    pub steps: usize,
    /// Coroutines alive at the end of the frame.
    pub running: usize,
    /// Coroutines that ended during this frame.
    pub finished: usize,
    /// Diagnostics produced this frame.
    pub notes: Vec<String>,
}

/// The interpreter.
pub struct Vm {
    /// Every precompiled common event.
    pub programs: ProgramSet,
    /// The global variable banks.
    pub vars: Variables,
    /// Running coroutines, in spawn order.
    instances: Vec<Instance>,
    next_id: InstanceId,
    frame: u64,
    budget: usize,
    /// Random source for 121's 0x6000 operator (docs/OPCODE_SPEC.md §3.2).
    pub rng: WolfRng,
}

impl Vm {
    /// A VM over a precompiled event set.
    #[must_use]
    pub fn new(programs: ProgramSet) -> Self {
        Self {
            programs,
            vars: Variables::new(),
            instances: Vec::new(),
            next_id: 1,
            frame: 0,
            budget: DEFAULT_BUDGET,
            rng: WolfRng::new(),
        }
    }

    /// Same, with a custom per-frame instruction budget.
    #[must_use]
    pub fn with_budget(programs: ProgramSet, budget: usize) -> Self {
        let mut vm = Self::new(programs);
        vm.budget = budget.max(1);
        vm
    }

    /// Frames elapsed.
    #[must_use]
    pub const fn frame(&self) -> u64 {
        self.frame
    }

    /// Running coroutines.
    #[must_use]
    pub fn instances(&self) -> &[Instance] {
        &self.instances
    }

    /// One coroutine by id.
    #[must_use]
    pub fn instance(&self, id: InstanceId) -> Option<&Instance> {
        self.instances.iter().find(|i| i.id == id)
    }

    /// Load an event into a fresh coroutine. Returns `None` when the id is unknown.
    pub fn start_event(&mut self, event: EventId) -> Option<InstanceId> {
        self.start_event_with(event, None)
    }

    /// Same, with a return target.
    pub fn start_event_with(
        &mut self,
        event: EventId,
        return_to: Option<i32>,
    ) -> Option<InstanceId> {
        let program = Arc::clone(self.programs.event(event)?);
        if self.instances.len() >= MAX_INSTANCES {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        let mut inst = Instance::new(id, None, program);
        inst.return_to = return_to;
        self.instances.push(inst);
        Some(id)
    }

    /// Load an already-compiled program into a fresh coroutine.
    ///
    /// Map-event pages are not common events, so they have no entry in
    /// [ProgramSet]; the caller compiles them with [Program::from_slots] and
    /// hands them over here. Everything downstream (blocks, waits, return
    /// values) behaves exactly as for a common event.
    pub fn start_program(&mut self, program: Arc<Program>) -> Option<InstanceId> {
        if self.instances.len() >= MAX_INSTANCES {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.instances.push(Instance::new(id, None, program));
        Some(id)
    }

    /// Stop one coroutine, and every coroutine it spawned, right now.
    ///
    /// A map event page owns exactly one coroutine, so switching pages has to
    /// retire the old one. Ending the program instead of removing the instance
    /// would keep it alive for the rest of the frame and, worse, would hand its
    /// return value to a parent that never asked for it. Waking the parent is
    /// this function's job: a caller waiting in [WaitState::Child] must not
    /// block forever on a coroutine that no longer exists.
    ///
    /// Returns true when the instance was found.
    pub fn stop_instance(&mut self, id: InstanceId) -> bool {
        if !self.instances.iter().any(|instance| instance.id == id) {
            return false;
        }
        // Descendants die with their ancestor: a child coroutine without its
        // caller has nobody left to return to.
        let mut doomed = vec![id];
        loop {
            let mut grew = false;
            for instance in &self.instances {
                if doomed.contains(&instance.id) {
                    continue;
                }
                let mut parent = instance.parent;
                while let Some(pid) = parent {
                    if doomed.contains(&pid) {
                        doomed.push(instance.id);
                        grew = true;
                        break;
                    }
                    parent = self
                        .instances
                        .iter()
                        .find(|other| other.id == pid)
                        .and_then(|other| other.parent);
                }
            }
            if !grew {
                break;
            }
        }
        // Wake anyone waiting on a doomed coroutine before it disappears.
        for index in 0..self.instances.len() {
            if let WaitState::Child(child) = self.instances[index].wait {
                if doomed.contains(&child) {
                    self.instances[index].wait = WaitState::None;
                }
            }
        }
        self.instances.retain(|instance| !doomed.contains(&instance.id));
        true
    }

    /// Load an event by title. Returns `None` when no title matches.
    pub fn start_event_by_name(&mut self, name: &str) -> Option<InstanceId> {
        let event = self.programs.id_by_name(name)?;
        self.start_event(event)
    }

    /// Advance every coroutine by one frame.
    pub fn step_frame(&mut self, host: &mut dyn Host) -> StepReport {
        self.frame += 1;
        let mut report = StepReport {
            frame: self.frame,
            ..StepReport::default()
        };
        let mut budget = self.budget;

        self.poll_waits(host);
        self.run_instances(host, &mut budget, &mut report);
        self.reap_finished(host);

        report.steps = self.budget - budget.min(self.budget);
        report.running = self.instances.len();
        report
    }

    /// Tick every wait state once, resolving the ones the host answered.
    fn poll_waits(&mut self, host: &mut dyn Host) {
        for i in 0..self.instances.len() {
            let wait = self.instances[i].wait.clone();
            match wait {
                WaitState::None | WaitState::Child(_) => {}
                WaitState::Frames(n) => {
                    self.instances[i].wait = if n <= 1 {
                        WaitState::None
                    } else {
                        WaitState::Frames(n - 1)
                    };
                }
                WaitState::Message => {
                    if host.message_dismissed() {
                        self.instances[i].wait = WaitState::None;
                    }
                }
                WaitState::Choice { block } => {
                    if let Some(reply) = host.choice_made() {
                        let inst = &mut self.instances[i];
                        let len = inst.program.len();
                        let target = match reply {
                            // 102's case ids are option index + 2 (docs/OPCODE_SPEC.md §4.2).
                            ChoiceReply::Index(index) => inst
                                .program
                                .blocks
                                .get(block)
                                .and_then(|b| b.case_target(index as i32 + 2))
                                .or_else(|| {
                                    inst.program.blocks.get(block).map(|b| b.end_target(len))
                                }),
                            ChoiceReply::Cancel => inst
                                .program
                                .blocks
                                .get(block)
                                .and_then(|b| b.cancel_pc)
                                .or_else(|| {
                                    inst.program.blocks.get(block).map(|b| b.end_target(len))
                                }),
                        };
                        if let Some(target) = target {
                            self.instances[i].pc = target;
                        }
                        self.instances[i].wait = WaitState::None;
                    }
                }
            }
        }
    }

    /// Run every runnable coroutine until the frame budget is gone.
    fn run_instances(&mut self, host: &mut dyn Host, budget: &mut usize, report: &mut StepReport) {
        let mut i = 0;
        while i < self.instances.len() {
            if !self.instances[i].is_runnable() {
                i += 1;
                continue;
            }
            if *budget == 0 {
                return;
            }
            let result = {
                let inst = &mut self.instances[i];
                execute(
                    inst,
                    &self.programs,
                    &mut self.vars,
                    host,
                    budget,
                    &mut self.rng,
                )
            };
            report.steps += result.steps;
            match result.signal {
                Signal::Blocked | Signal::Continue => {}
                Signal::Finished => {
                    report.finished += 1;
                }
                Signal::Call {
                    target,
                    args,
                    return_to,
                } => {
                    self.spawn_child(i, target, args, return_to, host, report);
                }
                Signal::CallByName {
                    name,
                    args,
                    return_to,
                } => match self.programs.id_by_name(&name) {
                    Some(target) => self.spawn_child(i, target, args, return_to, host, report),
                    None => {
                        host.log(
                            LogLevel::Error,
                            &format!("300 CallByName: no event titled `{name}`"),
                        );
                        report.notes.push(format!("unknown event name: {name}"));
                        // Non-blocking by design: the original would show an empty
                        // dialogue and continue, which is what falling through does.
                        self.instances[i].wait = WaitState::None;
                    }
                },
                Signal::ReturnToTitle => {
                    host.return_to_title();
                    self.instances[i].wait = WaitState::None;
                    self.instances[i].pc = self.instances[i].program.len();
                }
                Signal::EndGame => {
                    host.end_game();
                    self.instances.clear();
                    return;
                }
                Signal::Error(message) => {
                    host.log(LogLevel::Error, &message);
                    report.notes.push(message);
                    // A data error must not wedge the instance forever.
                    self.instances[i].pc += 1;
                }
            }
            i += 1;
        }
    }

    /// Create the child coroutine of a `210`/`300` and block the caller.
    fn spawn_child(
        &mut self,
        caller: usize,
        target: EventId,
        args: Vec<i32>,
        return_to: Option<i32>,
        host: &mut dyn Host,
        report: &mut StepReport,
    ) {
        let Some(program) = self.programs.event(target).map(Arc::clone) else {
            host.log(
                LogLevel::Error,
                &format!("210 CallCommonEvent: no event with id {target}"),
            );
            report
                .notes
                .push(format!("unknown common event id {target}"));
            return;
        };
        if self.instances.len() >= MAX_INSTANCES {
            host.log(LogLevel::Error, "instance limit reached; call dropped");
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let mut child = Instance::new(id, Some(self.instances[caller].id), program);
        // WOLF passes arguments into the child's self variables starting at
        // V0-0 (docs/OPCODE_SPEC.md §6.1: the argument names are declared by
        // `arg_names`, and the values land in self slots 0..n).
        for (slot, value) in args.iter().enumerate() {
            child.self_vars.set(slot, *value);
        }
        child.return_to = return_to;
        self.instances.push(child);
        self.instances[caller].wait = WaitState::Child(id);
    }

    /// Drop ended coroutines and hand their return values to the parents.
    fn reap_finished(&mut self, host: &mut dyn Host) {
        let finished: Vec<InstanceId> = self
            .instances
            .iter()
            .filter(|i| i.pc >= i.program.len())
            .map(|i| i.id)
            .collect();
        if finished.is_empty() {
            return;
        }
        for id in &finished {
            let Some(pos) = self.instances.iter().position(|i| i.id == *id) else {
                continue;
            };
            let inst = self.instances.remove(pos);
            let parent = inst.parent.and_then(|pid| {
                let idx = self.instances.iter().position(|i| i.id == pid)?;
                if self.instances[idx].wait == WaitState::Child(inst.id) {
                    Some(idx)
                } else {
                    None
                }
            });
            match parent {
                Some(idx) => {
                    // The child's return value lives in the self slot the event
                    // declared (docs/OPCODE_SPEC.md §2.5: `return_value_id`).
                    if let Some(target) = self.instances[idx].return_to.take() {
                        let value = inst.self_vars.get(return_slot(&inst.program) as usize);
                        store_num(&mut self.instances[idx], &mut self.vars, target, value);
                    }
                    self.instances[idx].wait = WaitState::None;
                }
                None => {
                    if inst.parent.is_some() {
                        host.log(
                            LogLevel::Warn,
                            &format!("coroutine {} ended with no waiting parent", inst.id),
                        );
                    }
                }
            }
        }
    }
}

/// The self-variable slot a common event returns its value through.
fn return_slot(program: &Program) -> i32 {
    // `CommonEvent.return_value_id` is not carried on Program; events without an
    // explicit declaration return self slot 0, which is what the caller's
    // argument list would have overwritten anyway.
    let _ = program;
    0
}

/// Read one magic operand.
fn eval_num(raw: i32, inst: &Instance, vars: &Variables) -> i32 {
    let magic = Magic::decode(raw);
    match magic.kind() {
        MagicKind::Literal => magic.index() as i32,
        MagicKind::SelfVar => inst.self_vars.get(magic.index() as usize),
        MagicKind::Var => vars.get(magic.index() as usize),
        MagicKind::SysVar => vars.get_sys(magic.index() as usize),
        // A string bank read as a number is 0 (the editor never does this).
        MagicKind::StrVar => 0,
        // "Opaque" is not a bank at all: it is the negative-sentinel space
        // (-1 keep, -2 whole screen, -1_000_000 unchanged) plus anything above
        // the system bank. Those values are literals and must survive, otherwise
        // picture -2 collapses to 0 and the full-screen layer is lost
        // (docs/M1_CLOSURE.md Q3).
        MagicKind::Opaque => raw,
    }
}

/// Read one magic operand as a string.
fn eval_str(raw: i32, inst: &Instance, vars: &Variables) -> String {
    let magic = Magic::decode(raw);
    match magic.kind() {
        MagicKind::StrVar => vars.get_str(magic.index() as usize).to_owned(),
        MagicKind::SelfVar => inst.self_vars.get_str(magic.index() as usize).to_owned(),
        MagicKind::SysVar => vars.get_sys_str(magic.index() as usize).to_owned(),
        // A numeric bank read as text yields the decimal form, which is what the
        // editor's `cself[n]` expansion does.
        MagicKind::Literal => magic.index().to_string(),
        MagicKind::Var => vars.get(magic.index() as usize).to_string(),
        MagicKind::Opaque => String::new(),
    }
}

/// Write one magic operand.
fn store_num(inst: &mut Instance, vars: &mut Variables, raw: i32, value: i32) {
    let magic = Magic::decode(raw);
    match magic.kind() {
        MagicKind::SelfVar => inst.self_vars.set(magic.index() as usize, value),
        MagicKind::Var => vars.set(magic.index() as usize, value),
        MagicKind::SysVar => vars.set_sys(magic.index() as usize, value),
        // Writing to a literal or a sentinel is a no-op in every WOLF build.
        MagicKind::Literal | MagicKind::StrVar | MagicKind::Opaque => {}
    }
}

/// Write one magic operand as a string.
fn store_str(inst: &mut Instance, vars: &mut Variables, raw: i32, value: &str) {
    let magic = Magic::decode(raw);
    match magic.kind() {
        MagicKind::StrVar => vars.set_str(magic.index() as usize, value),
        MagicKind::SelfVar => inst.self_vars.set_str(magic.index() as usize, value),
        MagicKind::SysVar => vars.set_sys_str(magic.index() as usize, value),
        MagicKind::Literal | MagicKind::Var | MagicKind::Opaque => {}
    }
}

/// Expand escapes in a string using the instance and the globals.
fn expand(raw: &str, inst: &Instance, vars: &Variables) -> String {
    let lookup = Lookup { inst, vars };
    text::expand(raw, &lookup)
}

/// Split the `@n` speaker prefix off a message (docs/OPCODE_SPEC.md §5.3).
fn split_face(text: &str) -> (Option<i32>, &str) {
    let bytes = text.as_bytes();
    if bytes.first() == Some(&b'@') {
        if let Some(digit) = bytes.get(1) {
            if digit.is_ascii_digit() {
                let face = i32::from(digit - b'0');
                return (Some(face), text.get(2..).unwrap_or(""));
            }
        }
    }
    (None, text)
}

/// Binary operators for `121`, docs/OPCODE_SPEC.md §3.2.
fn binary_op(kind: u8, a: i32, b: i32) -> i32 {
    match kind {
        0x0 => a.wrapping_add(b),
        0x1 => a.wrapping_sub(b),
        0x2 => a.wrapping_mul(b),
        0x3 => {
            if b == 0 {
                a
            } else {
                a.wrapping_rem(b)
            }
        }
        // 0x4 is "bitwise not of a" in WolfTL; the operand is unused.
        0x4 => !a,
        0x5 => a & b,
        // 0x6 is undefined in WolfTL but occurs 1,101x here (docs/OPCODE_SPEC.md
        // §3.2). It behaves like a right shift, NOT like addition.
        0x6 => a.wrapping_shr((b & 31) as u32),
        0x7 => a | b,
        0x8 => a ^ b,
        0x9 => a.wrapping_shl((b & 31) as u32),
        _ => a,
    }
}

/// Compound assignment / unary transforms for `121`, docs/OPCODE_SPEC.md §3.1.
fn compound(kind: i32, current: i32, value: i32) -> i32 {
    // Only bits 8..11 select the operation (WolfTL Command.hpp:36-49). 194 of the
    // 17,258 121 instructions also set bits 0..7 (0x104, 0x3302, 0x4 ...), which
    // the editor uses as flags we have not identified; masking them off keeps the
    // documented operation intact instead of falling through to "assign".
    match kind & 0x0F00 {
        0x000 => value,
        0x100 => current.wrapping_add(value),
        0x200 => current.wrapping_sub(value),
        0x300 => current.wrapping_mul(value),
        0x400 => {
            if value == 0 {
                current
            } else {
                current / value
            }
        }
        0x500 => {
            if value == 0 {
                current
            } else {
                current % value
            }
        }
        0x600 => value,
        0x700 => value,
        0x800 => value.abs(),
        0x900 => {
            // Angle from slope, in tenths of a degree.
            let deg = (value as f64).atan2(current as f64).to_degrees();
            (deg * 10.0) as i32
        }
        0xA00 => {
            let deg = (value as f64) / 10.0;
            (deg.to_radians().sin() * 1000.0) as i32
        }
        0xB00 => {
            let deg = (value as f64) / 10.0;
            (deg.to_radians().cos() * 1000.0) as i32
        }
        0xC00 => {
            let root = (value.max(0) as f64).sqrt();
            (root * 1000.0) as i32
        }
        _ => value,
    }
}

/// `111` comparison operators, docs/OPCODE_SPEC.md §3.3.
fn compare(lhs: i32, rhs: i32, operator: i32) -> bool {
    match operator {
        0 => lhs > rhs,
        1 => lhs >= rhs,
        2 => lhs == rhs,
        3 => lhs <= rhs,
        4 => lhs < rhs,
        5 => lhs != rhs,
        6 => (lhs & rhs) != 0,
        // 17/18/19 occur 44x and are undocumented; equality is the least
        // surprising reading and matches the single-condition branches they
        // appear in.
        _ => lhs == rhs,
    }
}

/// Execute one coroutine until it blocks, ends, or the budget runs out.
fn execute(
    inst: &mut Instance,
    _programs: &ProgramSet,
    vars: &mut Variables,
    host: &mut dyn Host,
    budget: &mut usize,
    rng: &mut WolfRng,
) -> BlockResult {
    // Holds the jump table and the instruction stream. Cloning the Arc (not the
    // Program) keeps the `&Command` borrow pointing at this local instead of at
    // `inst`, which the loop mutates on almost every arm.
    let program = Arc::clone(&inst.program);
    let mut steps = 0;
    loop {
        if inst.pc >= program.len() {
            return BlockResult {
                signal: Signal::Finished,
                steps,
            };
        }
        if *budget == 0 {
            return BlockResult {
                signal: Signal::Continue,
                steps,
            };
        }
        *budget -= 1;
        steps += 1;

        let pc = inst.pc;
        let Slot::Command(cmd) = &program.slots[pc] else {
            inst.pc += 1;
            continue;
        };
        let opcode = cmd.opcode;

        // Control flow first: these need the jump table, everything else does not.
        match program.control.get(pc).copied().unwrap_or(Control::None) {
            Control::Block(block) => {
                if let Some(result) = enter_block(inst, &program, vars, host, cmd, block, steps) {
                    return result;
                }
                continue;
            }
            Control::Case { block } => {
                // Reaching a case marker *by executing it* always means control
                // fell through the previous case body into the next marker, and
                // in WOLF that ends the branch: every 401 body runs from its
                // marker to the next 401 (or the closing 499) and must not leak
                // into the following case. Case *entry* never lands here -- it is
                // done by enter_block() / poll_waits() through case_target(),
                // which resume one slot PAST the marker. Getting this wrong makes
                // the title menu run every option at once (docs/M1_CLOSURE.md Q1).
                let block = &program.blocks[block];
                inst.pc = block.end_target(program.len());
                continue;
            }
            Control::LoopEnd { block } => {
                close_loop(inst, block);
                continue;
            }
            Control::LabelJump => {
                let name = expand(cmd.strings.first().map_or("", String::as_str), inst, vars);
                match program.labels.get(&name) {
                    Some(target) => inst.pc = *target,
                    None => {
                        host.log(
                            LogLevel::Warn,
                            &format!("213 JumpLabel: label `{name}` not found; skipping event"),
                        );
                        inst.pc = program.len();
                    }
                }
                continue;
            }
            Control::None => {}
        }

        let signal = match opcode {
            0 => {
                inst.pc += 1;
                None
            }
            99 => {
                // Exclusive branch start marker; nothing to do.
                inst.pc += 1;
                None
            }
            101 => {
                let raw = cmd.strings.first().map_or("", String::as_str);
                let expanded = expand(raw, inst, vars);
                let (face, text) = split_face(&expanded);
                host.show_message(text, face);
                inst.wait = WaitState::Message;
                inst.pc += 1;
                Some(Signal::Blocked)
            }
            102 => {
                let options: Vec<String> = cmd
                    .strings
                    .iter()
                    .map(|option| expand(option, inst, vars))
                    .collect();
                let block = match program.control.get(pc) {
                    Some(Control::Block(index)) => *index,
                    _ => 0,
                };
                host.show_choices(&options);
                inst.wait = WaitState::Choice { block };
                inst.pc += 1;
                Some(Signal::Blocked)
            }
            103 => {
                // Comment marker, 9,707 of them in this data.
                inst.pc += 1;
                None
            }
            105 => {
                host.close_message();
                inst.pc += 1;
                None
            }
            106 => {
                let text = expand(cmd.strings.first().map_or("", String::as_str), inst, vars);
                host.debug_message(&text);
                inst.pc += 1;
                None
            }
            107 => {
                host.clear_debug();
                inst.pc += 1;
                None
            }
            111 => {
                // Handled through Control::Block; kept for the no-table fallback.
                inst.pc += 1;
                None
            }
            112 => {
                inst.pc = eval_string_condition(inst, vars, cmd);
                None
            }
            121 => {
                let target = arg(cmd, 0);
                let a = eval_num(arg(cmd, 1), inst, vars);
                let b = eval_num(arg(cmd, 2), inst, vars);
                let bits = arg(cmd, 3) as u32;
                let bin = ((bits >> 12) & 0x0F) as u8;
                // 0x6000 is a random range: docs/OPCODE_SPEC.md §3.2 calls it
                // undefined in WolfTL, but 1,101 call sites pass (min, max) pairs
                // that only make sense as rand(min, max) — see rng.rs.
                let result = if bin == 6 {
                    rng.range(a, b)
                } else {
                    binary_op(bin, a, b)
                };
                let current = eval_num(target, inst, vars);
                let value = compound((bits & 0x0FFF) as i32, current, result);
                store_num(inst, vars, target, value);
                inst.pc += 1;
                None
            }
            122 => {
                let target = arg(cmd, 0);
                let value = cmd
                    .strings
                    .first()
                    .map_or(String::new(), |raw| expand(raw, inst, vars));
                store_str(inst, vars, target, &value);
                inst.pc += 1;
                None
            }
            123 => {
                // Key-input poll (docs/OPCODE_SPEC.md 5.13). The destination receives
                // the CODE of the held key, not a bit pattern - the game compares it
                // against 10/11/12 and the numpad direction codes, which is why the
                // old "key_bits & mask" model could never take a branch. A headless
                // host reports 0, exactly like an untouched keyboard.
                let target = arg(cmd, 0);
                let mask = arg(cmd, 1) as u32;
                store_num(inst, vars, target, host.key_code(mask));
                inst.pc += 1;
                None
            }
            124 => {
                // 124 SetVariableEx (``変数操作（拡張）``; WolfTL Command.hpp:112) is a pure
                // query: pick a value out of the engine state and store it. It never
                // blocks and never touches the block stack (docs/OPCODE_124.md §0).
                //
                // Shape: argc 4 = [dst, sub, object, field], argc 3 = [dst, sub, field]
                // (only 0x3000 appears in the short form, 278/278 in this game). The
                // object operand is a normal magic value - a picture number can be
                // 5 digits or a variable reference - so it goes through eval_num.
                let target = arg(cmd, 0);
                let sub = arg(cmd, 1);
                let field = if argc(cmd) >= 4 {
                    arg(cmd, 3)
                } else {
                    arg(cmd, 2)
                };
                let value = match sub {
                    0x3000 => {
                        // field 18 is "the event being processed", which only the
                        // interpreter knows; common events report id + 500000.
                        if field == 18 {
                            Some(inst.event as i32 + 500_000)
                        } else {
                            host.engine_state(field)
                        }
                    }
                    0x1000 | 0x1200 => {
                        let object = if argc(cmd) >= 4 {
                            eval_num(arg(cmd, 2), inst, vars)
                        } else {
                            0
                        };
                        host.character_state(object, field)
                    }
                    0x4000 | 0x4400 => {
                        let object = if argc(cmd) >= 4 {
                            eval_num(arg(cmd, 2), inst, vars)
                        } else {
                            0
                        };
                        host.picture_state(object, field)
                    }
                    other => {
                        host.log(
                            LogLevel::Info,
                            &format!("124 SetVariableEx: sub-op {other:#x} unused in this game"),
                        );
                        None
                    }
                };
                // `None` deliberately leaves the destination alone: the original engine does
                // not write anything when the queried object does not exist (§3.1).
                if let Some(value) = value {
                    store_num(inst, vars, target, value);
                }
                inst.pc += 1;
                None
            }
            130 => {
                host.teleport(arg(cmd, 0), arg(cmd, 1), arg(cmd, 2), arg(cmd, 3));
                // The map load is instant in the original; one frame of slack
                // keeps the host from having to re-enter the interpreter.
                inst.wait = WaitState::Frames(1);
                inst.pc += 1;
                Some(Signal::Blocked)
            }
            140 => {
                let bits = arg(cmd, 0);
                let channel = if bits & 0x20 != 0 {
                    SoundChannel::Se
                } else {
                    SoundChannel::Bgm
                };
                let playing = bits >= 0 && bits & 0x0200_0000 != 0;
                let (action, file, volume, frames) = if playing {
                    let file = expand(cmd.strings.first().map_or("", String::as_str), inst, vars);
                    let volume = if argc(cmd) >= 6 { arg(cmd, 1) } else { 100 };
                    (SoundAction::Play, file, volume, 0)
                } else {
                    let frames = if argc(cmd) <= 5 { arg(cmd, 1) } else { 0 };
                    (SoundAction::Stop, String::new(), 0, frames)
                };
                host.sound(&SoundCmd {
                    channel,
                    action,
                    file,
                    volume,
                    frames,
                });
                inst.pc += 1;
                None
            }
            150 => {
                host.picture(&decode_picture(inst, vars, cmd));
                inst.pc += 1;
                None
            }
            151 | 161 => {
                host.log(
                    LogLevel::Warn,
                    &format!("{opcode}: transition variant with undocumented layout; ignored"),
                );
                inst.pc += 1;
                None
            }
            160 => {
                host.transition(arg(cmd, 0), arg(cmd, 1));
                inst.pc += 1;
                None
            }
            162 => {
                host.transition(arg(cmd, 0), arg(cmd, 1));
                inst.wait = WaitState::Frames(1);
                inst.pc += 1;
                Some(Signal::Blocked)
            }
            170 | 176 => {
                // Unconditional loop heads. Control::Block pushed nothing yet:
                // enter_block handles the push, so reaching here means the jump
                // table was not built (a hand-made Program). Treat as fallthrough.
                inst.pc += 1;
                None
            }
            171 => {
                // Break the innermost loop: continue after its 498.
                if let Some(ctx) = inst.loops.pop() {
                    let block = &program.blocks[ctx.block];
                    inst.pc = block.end_target(program.len());
                } else {
                    host.log(LogLevel::Warn, "171 BreakLoop outside any loop");
                    inst.pc += 1;
                }
                None
            }
            172 | 173 => {
                // Both end the coroutine; retiring the pc is what tells the
                // scheduler the instance is done.
                inst.pc = program.len();
                Some(Signal::Finished)
            }
            174 => Some(Signal::ReturnToTitle),
            175 => Some(Signal::EndGame),
            179 => {
                inst.pc += 1;
                None
            }
            180 => {
                let frames = eval_num(arg(cmd, 0), inst, vars).max(1) as u32;
                inst.wait = WaitState::Frames(frames);
                inst.pc += 1;
                Some(Signal::Blocked)
            }
            201 => {
                let target = arg(cmd, 0);
                if let Tail::MoveRoute(route) = &cmd.tail {
                    host.move_route(target, &route.commands);
                }
                inst.pc += 1;
                None
            }
            210 => {
                let mut raw = arg(cmd, 0);
                if Magic::decode(raw).is_storage() {
                    raw = eval_num(raw, inst, vars);
                }
                let target = match raw {
                    500_000..=599_999 => Some((raw - 500_000) as EventId),
                    600_000..=699_999 => Some((raw - 600_000) as EventId),
                    _ => None,
                };
                let Some(target) = target else {
                    host.log(
                        LogLevel::Error,
                        &format!("210 CallCommonEvent: unsupported target {raw}"),
                    );
                    inst.pc += 1;
                    return BlockResult {
                        signal: Signal::Continue,
                        steps,
                    };
                };
                let (args, return_to) = call_arguments(cmd, inst, vars);
                inst.pc += 1;
                Some(Signal::Call {
                    target,
                    args,
                    return_to,
                })
            }
            211 => {
                let mut raw = arg(cmd, 0);
                if Magic::decode(raw).is_storage() {
                    raw = eval_num(raw, inst, vars);
                }
                let target = match raw {
                    500_000..=599_999 => (raw - 500_000) as EventId,
                    600_000..=699_999 => (raw - 600_000) as EventId,
                    _ => raw.max(0) as EventId,
                };
                host.reserve(target);
                inst.pc += 1;
                None
            }
            212 => {
                inst.pc += 1;
                None
            }
            213 => {
                inst.pc += 1;
                None
            }
            220..=222 => {
                let args: Vec<i32> = (0..argc(cmd)).map(|i| arg(cmd, i)).collect();
                host.save_load(&args);
                inst.pc += 1;
                None
            }
            250 => {
                let db_type = arg(cmd, 0);
                let db_type = if Magic::decode(db_type).is_storage() {
                    eval_num(db_type, inst, vars)
                } else {
                    db_type
                };
                let value = if argc(cmd) >= 5 {
                    Some(eval_num(arg(cmd, 4), inst, vars))
                } else {
                    None
                };
                host.database(&DbOp {
                    db_type,
                    data_id: arg(cmd, 1),
                    item_id: arg(cmd, 2),
                    flags: arg(cmd, 3),
                    value,
                });
                inst.pc += 1;
                None
            }
            270 => {
                let args: Vec<i32> = (0..argc(cmd)).map(|i| arg(cmd, i)).collect();
                host.party(&args);
                inst.pc += 1;
                None
            }
            281 => {
                let args: Vec<i32> = (0..argc(cmd)).map(|i| arg(cmd, i)).collect();
                host.scroll(&args);
                inst.pc += 1;
                None
            }
            290 => {
                let kind = arg(cmd, 0) >> 4;
                let duration = arg(cmd, 1);
                let args: Vec<i32> = (0..argc(cmd)).map(|i| arg(cmd, i)).collect();
                host.effect(kind, duration, &args);
                inst.pc += 1;
                None
            }
            300 => {
                let name = expand(cmd.strings.first().map_or("", String::as_str), inst, vars);
                let (args, return_to) = call_arguments(cmd, inst, vars);
                inst.pc += 1;
                Some(Signal::CallByName {
                    name,
                    args,
                    return_to,
                })
            }
            401 | 402 => {
                // Only reachable when the jump table was not built.
                inst.pc += 1;
                None
            }
            420 | 421 => {
                inst.pc += 1;
                None
            }
            498 => {
                inst.pc += 1;
                None
            }
            499 => {
                inst.pc += 1;
                None
            }
            1000 => {
                let kind = arg(cmd, 0) >> 16;
                let milliseconds = arg(cmd, 1);
                host.pro_feature(kind, milliseconds);
                if kind == 5 {
                    inst.wait = WaitState::Frames((milliseconds / 16).max(1) as u32);
                    inst.pc += 1;
                    Some(Signal::Blocked)
                } else {
                    inst.pc += 1;
                    None
                }
            }
            other => {
                host.log(
                    LogLevel::Warn,
                    &format!("unhandled opcode {other} at pc {pc}"),
                );
                inst.pc += 1;
                None
            }
        };

        if let Some(signal) = signal {
            return BlockResult { signal, steps };
        }
    }
}

/// Handle a `111`/`102`/loop head, which needs its block's case table.
fn enter_block(
    inst: &mut Instance,
    program: &Program,
    vars: &mut Variables,
    host: &mut dyn Host,
    cmd: &Command,
    block_index: usize,
    steps: usize,
) -> Option<BlockResult> {
    let block = program.blocks[block_index].clone();
    let len = program.len();
    match block.kind {
        BlockKind::Loop => {
            let remaining = if cmd.opcode == 179 {
                let count = eval_num(arg(cmd, 0), inst, vars);
                if count <= 0 {
                    inst.pc = block.end_target(len);
                    return None;
                }
                i64::from(count)
            } else {
                -1
            };
            inst.loops.push(LoopCtx {
                block: block_index,
                remaining,
            });
            inst.pc += 1;
            None
        }
        BlockKind::Condition => {
            let head = arg(cmd, 0);
            let count = (head & 0x0F) as usize;
            let mut matched = None;
            for i in 0..count {
                let var = arg(cmd, 1 + 3 * i);
                let value = arg(cmd, 2 + 3 * i);
                let operator = arg(cmd, 3 + 3 * i);
                let lhs = eval_num(var, inst, vars);
                if compare(lhs, value, operator) {
                    matched = Some((i + 1) as i32);
                    break;
                }
            }
            let target = match matched {
                Some(case_id) => block.case_target(case_id),
                None => None,
            };
            // Bit 4 of the head nibble is documented as "has else", but this
            // game's 0x10..0x14 shapes disagree with where the 420 markers
            // actually sit (docs/OPCODE_SPEC.md §5.16), so the structural
            // evidence — a 420 belonging to this block — is what we trust.
            let target = target.or(block.else_pc);
            inst.pc = target.unwrap_or_else(|| block.end_target(len));
            None
        }
        BlockKind::Choices => {
            let options: Vec<String> = cmd
                .strings
                .iter()
                .map(|option| expand(option, inst, vars))
                .collect();
            host.show_choices(&options);
            inst.wait = WaitState::Choice { block: block_index };
            inst.pc += 1;
            Some(BlockResult {
                signal: Signal::Blocked,
                steps,
            })
        }
    }
}

/// `498`: count the loop down and close it, or jump back to the body.
fn close_loop(inst: &mut Instance, block_index: usize) {
    let len = inst.program.len();
    let Some(pos) = inst.loops.iter().rposition(|ctx| ctx.block == block_index) else {
        // Orphan 498 (docs/OPCODE_SPEC.md §4.3): behave like a no-op.
        inst.pc += 1;
        return;
    };
    let ctx = &mut inst.loops[pos];
    // A negative count is an endless loop (170/176), which must always jump back;
    // a counted loop (179) runs its body once per unit and exits on the last one.
    let again = if ctx.remaining < 0 {
        true
    } else if ctx.remaining > 1 {
        ctx.remaining -= 1;
        true
    } else {
        false
    };
    if again {
        inst.pc = inst.program.blocks[block_index].head + 1;
    } else {
        inst.loops.truncate(pos);
        inst.pc = inst.program.blocks[block_index].end_target(len);
    }
}

/// `112`: string condition. Layout and operator encoding are the least
/// documented part of the format (docs/OPCODE_SPEC.md §5.27) and this game's
/// data contains no `112` at all, so we implement equality only and log.
fn eval_string_condition(inst: &mut Instance, vars: &mut Variables, cmd: &Command) -> usize {
    let head = arg(cmd, 0);
    let count = (head & 0x0F) as usize;
    let use_else = head & 0x10 != 0;
    let block_index = match inst.program.control.get(inst.pc) {
        Some(Control::Block(index)) => *index,
        _ => return inst.pc + 1,
    };
    let block = inst.program.blocks[block_index].clone();
    let len = inst.program.len();
    let mut matched = None;
    for i in 0..count {
        let var = arg(cmd, 1 + i);
        let lhs = eval_str(var, inst, vars);
        let rhs = cmd
            .strings
            .get(i)
            .map_or(String::new(), |raw| expand(raw, inst, vars));
        if lhs == rhs {
            matched = Some((i + 1) as i32);
            break;
        }
    }
    let target = matched.and_then(|case_id| block.case_target(case_id));
    let target = target.or(if use_else { block.else_pc } else { None });
    target.unwrap_or_else(|| block.end_target(len))
}

/// Evaluate the arguments and return target of a `210`/`300`.
fn call_arguments(cmd: &Command, inst: &Instance, vars: &Variables) -> (Vec<i32>, Option<i32>) {
    let Some(status) = cmd.param_status() else {
        return (Vec::new(), None);
    };
    let count = status.number_count();
    let mut args = Vec::with_capacity(count);
    for i in 0..count {
        args.push(eval_num(arg(cmd, 2 + i), inst, vars));
    }
    let return_to = if status.return_value_enabled() {
        Some(arg(cmd, 2 + count))
    } else {
        None
    };
    (args, return_to)
}

/// Decode a `150 Picture` instruction (docs/OPCODE_SPEC.md §5.31).
/// One `150` argument: evaluate magic storage references, pass flags and sentinels through.
fn picture_arg(raw: i32, inst: &Instance, vars: &Variables) -> i32 {
    // 0x0F00_0000 is the flag nibble Magic::decode strips; those bits are data here.
    if (raw as u32) & 0x0F00_0000 != 0 {
        raw
    } else {
        eval_num(raw, inst, vars)
    }
}

fn decode_picture(inst: &Instance, vars: &Variables, cmd: &Command) -> PictureCmd {
    let head = arg(cmd, 0);
    let kind = ((head >> 4) & 0x07) as u8;
    // Bits 12..14 of the head word anchor the picture: 0 means x/y is the top-left corner,
    // 1 (0x1000) means it is the centre. Measured over 13,637 long-form draws - the 0x1000
    // group sits on x=512 / y=288 (the logical screen centre), and the retail title hint
    // banner only lines up when its 606x35 image is centred on (512, 520).
    let anchor = ((head >> 12) & 0x07) as u8;
    let action = match head & 0x0F {
        0 => PictureAction::Draw,
        1 => PictureAction::Move,
        2 => PictureAction::Erase,
        _ => PictureAction::Set,
    };
    let number = eval_num(arg(cmd, 1), inst, vars);
    let raw = cmd.strings.first().map_or("", String::as_str);
    let expanded = expand(raw, inst, vars);
    let (file, text) = match kind {
        0 | 3 => (expanded, String::new()),
        2 => (String::new(), expanded),
        // Types 1 and 4 name a string variable whose contents are the value.
        1 | 4 => {
            let index: usize = expanded.trim().parse().unwrap_or(0);
            (vars.get_str(index).to_owned(), String::new())
        }
        _ => (String::new(), String::new()),
    };
    // Every argument past the head is a value: the measured title script passes its menu
    // coordinates as self-variable references (picture 0 sits at self[41]/self[42]) rather
    // than constants, so they must be evaluated here or the picture lands at 1,600,041.
    // Flag carriers (top nibble set, e.g. 0x02000000) and the negative sentinels
    // (-1 / -2 / -1000000) are passed through untouched: their high bits are meaningful.
    let args: Vec<i32> = (2..argc(cmd))
        .map(|i| picture_arg(arg(cmd, i), inst, vars))
        .collect();
    let animate = if args.len() > 11 {
        args[11..].to_vec()
    } else {
        Vec::new()
    };
    PictureCmd {
        number,
        kind,
        action,
        file,
        text,
        args,
        anchor,
        animate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wolf_format::command::{Command, Slot, Tail};

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

    fn event(id: u32, title: &str, commands: Vec<Slot>) -> wolf_format::commonevent::CommonEvent {
        wolf_format::commonevent::CommonEvent {
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
            return_value_id: 0,
        }
    }

    fn vm_with(events: Vec<wolf_format::commonevent::CommonEvent>) -> Vm {
        let mut data = wolf_format::commonevent::CommonEventData {
            version_header: 0x55,
            version: 0xC9,
            events,
            footer: 0x90,
        };
        data.events.sort_by_key(|e| e.id);
        Vm::new(ProgramSet::from_common_events(&data))
    }

    #[derive(Default)]
    struct Recorder {
        messages: Vec<String>,
        faces: Vec<Option<i32>>,
        choices: Vec<Vec<String>>,
        pictures: Vec<PictureCmd>,
        sounds: Vec<SoundCmd>,
        logs: Vec<(LogLevel, String)>,
        teleports: Vec<(i32, i32, i32, i32)>,
        dismiss: bool,
        reply: Option<ChoiceReply>,
        // Answers served to 124 picture queries, keyed by (number, field).
        picture_answers: Vec<(i32, i32, Option<i32>)>,
        // Every (number, field) pair 124 actually asked about.
        picture_queries: Vec<(i32, i32)>,
        // Canned engine answers for 124 0x3000, keyed by field.
        engine_answers: Vec<(i32, i32)>,
    }

    impl Host for Recorder {
        fn show_message(&mut self, text: &str, face: Option<i32>) {
            self.messages.push(text.to_owned());
            self.faces.push(face);
        }
        fn message_dismissed(&mut self) -> bool {
            self.dismiss
        }
        fn show_choices(&mut self, options: &[String]) {
            self.choices.push(options.to_vec());
        }
        fn choice_made(&mut self) -> Option<ChoiceReply> {
            self.reply
        }
        fn picture(&mut self, cmd: &PictureCmd) {
            self.pictures.push(cmd.clone());
        }
        fn sound(&mut self, cmd: &SoundCmd) {
            self.sounds.push(cmd.clone());
        }
        fn teleport(&mut self, map: i32, x: i32, y: i32, direction: i32) {
            self.teleports.push((map, x, y, direction));
        }
        fn log(&mut self, level: LogLevel, message: &str) {
            self.logs.push((level, message.to_owned()));
        }
        fn picture_state(&mut self, number: i32, field: i32) -> Option<i32> {
            self.picture_queries.push((number, field));
            self.picture_answers
                .iter()
                .find(|(n, f, _)| *n == number && *f == field)
                .and_then(|(_, _, value)| *value)
        }
        fn engine_state(&mut self, field: i32) -> Option<i32> {
            self.engine_answers
                .iter()
                .find(|(f, _)| *f == field)
                .map(|(_, value)| *value)
        }
    }

    #[test]
    fn opcode_124_writes_picture_and_engine_answers() {
        // 0x4000 reads a picture field: field 8 is the hover test that drives the whole
        // title menu (588 uses game-wide, docs/OPCODE_124.md section 5.3).
        let ev = event(
            42,
            "queries",
            vec![
                cmd(124, &[2_000_000, 0x4000, 12_005, 8], &[]),
                cmd(124, &[2_000_001, 0x4000, 12_005, 9], &[]),
                // argc 3 is the short form and only 0x3000 uses it: [dst, sub, field].
                cmd(124, &[2_000_002, 0x3000, 20], &[]),
                // An unanswered field must leave the destination alone, not write 0:
                // seed it with 7 first, then ask a field nobody answers.
                cmd(121, &[2_000_003, 7, 0, 0], &[]),
                cmd(124, &[2_000_003, 0x4000, 12_005, 77], &[]),
            ],
        );
        let mut vm = vm_with(vec![ev]);
        vm.start_event(42);
        let mut host = Recorder {
            picture_answers: vec![(12_005, 8, Some(1)), (12_005, 9, Some(0))],
            engine_answers: vec![(20, 640)],
            ..Recorder::default()
        };
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 1, "hover answer");
        assert_eq!(vm.vars.get(1), 0, "in-use answer");
        assert_eq!(vm.vars.get(2), 640, "mouse x from the short form");
        assert_eq!(vm.vars.get(3), 7, "unanswered field keeps the old value");
        assert_eq!(
            host.picture_queries,
            vec![(12_005, 8), (12_005, 9), (12_005, 77)]
        );
        // The object operand is a magic value, so a 5-digit picture number arrives
        // verbatim instead of being decoded as a variable reference.
        assert!(host.logs.is_empty(), "unexpected logs");
    }

    #[test]
    fn opcode_124_reports_the_running_event_for_field_18() {
        // Field 18 answers 'which event is running' from the interpreter itself, and
        // common events report id + 500000 so they cannot collide with map events.
        let ev = event(77, "self id", vec![cmd(124, &[2_000_000, 0x3000, 18], &[])]);
        let mut vm = vm_with(vec![ev]);
        vm.start_event(77);
        let mut host = Recorder::default();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 500_077);
    }

    #[test]
    fn arithmetic_assigns_and_compounds() {
        // v[0] = 2 + 3; v[1] = v[0] * 4
        let e = event(
            1,
            "arith",
            vec![
                cmd(121, &[2_000_000, 2, 3, 0], &[]),
                cmd(121, &[2_000_001, 2_000_000, 4, 0x2000], &[]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 5);
        assert_eq!(vm.vars.get(1), 20);
    }

    #[test]
    fn operator_0x6000_is_a_random_range() {
        // 1,101 real instructions use it; before this was understood, every one
        // of them silently took the wrong branch (docs/OPCODE_SPEC.md §3.2).
        let ev = event(
            42,
            "random",
            vec![
                cmd(121, &[2_000_000, 1, 6, 0x6000], &[]),
                cmd(121, &[2_000_001, 0, 2_000_000, 0x0000], &[]),
            ],
        );
        let mut vm = vm_with(vec![ev]);
        vm.start_event(42);
        let mut host = Recorder::default();
        vm.step_frame(&mut host);
        let drawn = vm.vars.get(0);
        assert!((1..=6).contains(&drawn), "not in range: {drawn}");
        assert_eq!(vm.rng.draws(), 1);
        // The 0x0000 binary op is still plain addition, not a random draw.
        assert_eq!(vm.vars.get(1), drawn);
        assert_eq!(vm.rng.draws(), 1);
    }

    #[test]
    fn stray_low_bits_do_not_change_the_operator() {
        // 194 real instructions set undocumented bits 0..7 (0x104, 0x3302, 0x4).
        // 0x104 must still be `+=`, not a plain assignment.
        let ev = event(
            7,
            "bits",
            vec![
                cmd(121, &[2_000_000, 5, 0, 0x0000], &[]),
                cmd(121, &[2_000_000, 3, 0, 0x0104], &[]),
            ],
        );
        let mut vm = vm_with(vec![ev]);
        vm.start_event(7);
        let mut host = Recorder::default();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 8, "expected 5 += 3");
    }
    #[test]
    fn compound_assignment_operators() {
        let e = event(
            1,
            "compound",
            vec![cmd(121, &[2_000_000, 7, 0, 0x000], &[])],
        );
        let mut vm = vm_with(vec![e]);
        vm.vars.set(0, 100);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 7);
    }

    #[test]
    fn division_by_zero_keeps_the_old_value() {
        // v[0] /= (0 + 0): the binary op supplies the divisor, the compound
        // operator divides by it.
        let e = event(1, "divzero", vec![cmd(121, &[2_000_000, 0, 0, 0x400], &[])]);
        let mut vm = vm_with(vec![e]);
        vm.vars.set(0, 33);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 33);
    }

    #[test]
    fn conditional_takes_the_matching_branch() {
        // if (v[0] > 5) { v[1] = 1 } else { v[1] = 2 }
        let e = event(
            1,
            "branch",
            vec![
                cmd(111, &[1, 2_000_000, 5, 0], &[]),
                cmd(121, &[2_000_001, 1, 0, 0], &[]),
                cmd(401, &[1], &[]),
                cmd(121, &[2_000_001, 1, 0, 0], &[]),
                cmd(420, &[0], &[]),
                cmd(121, &[2_000_001, 2, 0, 0], &[]),
                cmd(499, &[], &[]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        vm.vars.set(0, 9);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(1), 1);
    }

    #[test]
    fn conditional_takes_the_else_when_nothing_matches() {
        let e = event(
            1,
            "branch2",
            vec![
                cmd(111, &[1, 2_000_000, 5, 0], &[]),
                cmd(121, &[2_000_001, 1, 0, 0], &[]),
                cmd(401, &[1], &[]),
                cmd(121, &[2_000_001, 1, 0, 0], &[]),
                cmd(420, &[0], &[]),
                cmd(121, &[2_000_001, 2, 0, 0], &[]),
                cmd(499, &[], &[]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        vm.vars.set(0, 1);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(1), 2);
    }

    #[test]
    fn loop_times_runs_exactly_n_times() {
        // v[0] = 0; loop 3 times { v[0] += 1 }
        let e = event(
            1,
            "loop",
            vec![
                cmd(121, &[2_000_000, 0, 0, 0], &[]),
                cmd(179, &[3], &[]),
                cmd(121, &[2_000_000, 2_000_000, 1, 0], &[]),
                cmd(498, &[], &[]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 3);
        assert_eq!(vm.instances().len(), 0);
    }

    #[test]
    fn loop_times_zero_skips_the_body() {
        let e = event(
            1,
            "loop0",
            vec![
                cmd(179, &[0], &[]),
                cmd(121, &[2_000_000, 1, 0, 0], &[]),
                cmd(498, &[], &[]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 0);
    }

    #[test]
    fn break_loop_leaves_the_loop() {
        // loop { v[0] += 1; if v[0] == 2 { break } }
        let e = event(
            1,
            "break",
            vec![
                cmd(170, &[], &[]),
                cmd(121, &[2_000_000, 2_000_000, 1, 0], &[]),
                cmd(111, &[1, 2_000_000, 2, 2], &[]),
                cmd(401, &[1], &[]),
                cmd(171, &[], &[]),
                cmd(499, &[], &[]),
                cmd(498, &[], &[]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 2);
    }

    #[test]
    fn wait_one_blocks_exactly_one_frame() {
        let e = event(
            1,
            "wait",
            vec![cmd(180, &[1], &[]), cmd(121, &[2_000_000, 5, 0, 0], &[])],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 0, "must still be waiting after frame 1");
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 5, "must resume on frame 2");
    }

    #[test]
    fn message_blocks_until_dismissed() {
        let e = event(
            1,
            "msg",
            vec![
                cmd(101, &[], &["@2hello"]),
                cmd(121, &[2_000_000, 1, 0, 0], &[]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(host.messages, vec!["hello".to_owned()]);
        assert_eq!(host.faces, vec![Some(2)]);
        assert_eq!(vm.vars.get(0), 0);
        host.dismiss = true;
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 1);
    }

    #[test]
    fn choice_jumps_to_the_selected_case() {
        // 102 with 2 options: option 1 sets v[0] = 11, option 0 sets v[0] = 10
        let e = event(
            1,
            "choice",
            vec![
                cmd(102, &[2], &["first", "second"]),
                cmd(401, &[2], &[]),
                cmd(121, &[2_000_000, 10, 0, 0], &[]),
                cmd(401, &[3], &[]),
                cmd(121, &[2_000_000, 11, 0, 0], &[]),
                cmd(421, &[0], &[]),
                cmd(499, &[], &[]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder {
            reply: Some(ChoiceReply::Index(1)),
            ..Recorder::default()
        };
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(
            host.choices[0],
            vec!["first".to_owned(), "second".to_owned()]
        );
        // The reply is polled at the start of the next frame, so the jump lands
        // there — same latency the original has when the player clicks.
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 11);
    }

    #[test]
    fn a_case_body_does_not_fall_through_into_the_next_case() {
        // Choosing option 0 must run only the first body. The old interpreter
        // executed the next 401 marker and jumped straight into the sibling
        // case, so the title menu ran every entry at once (M1_CLOSURE Q1).
        let e = event(
            1,
            "no fall-through",
            vec![
                cmd(102, &[2], &["first", "second"]),
                cmd(401, &[2], &[]),
                cmd(121, &[2_000_000, 10, 0, 0], &[]),
                cmd(121, &[2_000_001, 10_000, 0, 0], &[]),
                cmd(401, &[3], &[]),
                cmd(121, &[2_000_001, 22_222, 0, 0], &[]),
                cmd(499, &[], &[]),
                cmd(121, &[2_000_002, 33_333, 0, 0], &[]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder {
            reply: Some(ChoiceReply::Index(0)),
            ..Recorder::default()
        };
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 10, "first case body ran");
        assert_eq!(
            vm.vars.get(1),
            10_000,
            "the next marker ended the branch instead of entering the sibling case"
        );
        // The branch ends at the 499, so the instruction after it still runs.
        assert_eq!(
            vm.vars.get(2),
            33_333,
            "execution continues past the branch"
        );
        assert!(vm.instance(0).is_none(), "event finished");
    }

    #[test]
    fn opaque_operands_keep_their_negative_value() {
        // -2 is the "whole screen" picture sentinel, -1_000_000 means "leave
        // unchanged": both live in the MagicKind::Opaque space and used to decay
        // to 0, which silently turned layer -2 into picture 0 (M1_CLOSURE Q3).
        let e = event(
            1,
            "opaque",
            vec![
                cmd(121, &[2_000_000, -2, 0, 0], &[]),
                cmd(121, &[2_000_001, -1_000_000, 0, 0], &[]),
                cmd(150, &[0, -2, 30, 1, 1, 1, 255, 0, 0, 100, 0], &["a.jpg"]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), -2);
        assert_eq!(vm.vars.get(1), -1_000_000);
        assert_eq!(host.pictures[0].number, -2, "screen-layer picture number");
    }

    #[test]
    fn common_event_call_spawns_a_child_and_blocks_the_caller() {
        let callee = event(
            2,
            "callee",
            vec![cmd(121, &[2_000_000, 7, 0, 0], &[]), cmd(172, &[], &[])],
        );
        let caller = event(
            1,
            "caller",
            vec![
                cmd(210, &[500_002, 0, 0, 0], &[]),
                cmd(121, &[2_000_001, 1, 0, 0], &[]),
            ],
        );
        let mut vm = vm_with(vec![callee, caller]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        // The child is a separate coroutine: it runs on the next frame, and the
        // parent stays blocked until it finishes.
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 7);
        // Parent resumed after the child ended.
        assert_eq!(vm.vars.get(1), 1);
        assert_eq!(vm.instances().len(), 0);
    }

    #[test]
    fn label_jump_lands_on_the_set_label() {
        let e = event(
            1,
            "labels",
            vec![
                cmd(213, &[], &["skip"]),
                cmd(121, &[2_000_000, 1, 0, 0], &[]),
                cmd(212, &[], &["skip"]),
                cmd(121, &[2_000_001, 2, 0, 0], &[]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 0, "the skipped instruction must not run");
        assert_eq!(vm.vars.get(1), 2);
    }

    #[test]
    fn unknown_label_ends_the_event_instead_of_looping() {
        let e = event(
            1,
            "badlabel",
            vec![
                cmd(213, &[], &["nowhere"]),
                cmd(121, &[2_000_000, 1, 0, 0], &[]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 0);
        assert!(host.logs.iter().any(|(l, _)| *l == LogLevel::Warn));
        assert_eq!(vm.instances().len(), 0);
    }

    #[test]
    fn picture_long_form_decodes_position_and_file() {
        let e = event(
            1,
            "picture",
            vec![cmd(
                150,
                &[0x00, 3, 100, 200, 50, 50, 255, 0, 0, 0, 0, 0, 0x0200_0000],
                &["SystemGraphic/TitleGraphic01.jpg"],
            )],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        let picture = &host.pictures[0];
        assert_eq!(picture.number, 3);
        assert_eq!(picture.kind, 0);
        assert_eq!(picture.action, PictureAction::Draw);
        assert_eq!(picture.file, "SystemGraphic/TitleGraphic01.jpg");
        assert_eq!(picture.args[0], 100);
        assert_eq!(picture.args[1], 200);
        assert_eq!(picture.anchor, 0, "0x0000 anchors the top-left corner");
    }

    #[test]
    fn picture_head_bits_12_to_14_carry_the_anchor() {
        // The retail title banner: head 4096 = 0x1000, drawn at the screen centre.
        let e = event(
            1,
            "banner",
            vec![cmd(
                150,
                &[4096, 12001, 0, 1, 1, 1, 255, 512, 520],
                &["SystemFile/Button01.png"],
            )],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(host.pictures[0].anchor, 1, "0x1000 anchors the centre");
        assert_eq!(host.pictures[0].kind, 0);
        assert_eq!(host.pictures[0].action, PictureAction::Draw);
    }

    #[test]
    fn picture_text_form_keeps_the_text() {
        let e = event(
            1,
            "picture_text",
            vec![cmd(150, &[0x20, 1, 0, 0], &["こんにちは"])],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(host.pictures[0].kind, 2);
        assert_eq!(host.pictures[0].text, "こんにちは");
        assert!(host.pictures[0].file.is_empty());
    }

    #[test]
    fn se_play_is_non_blocking_and_decodes_the_channel() {
        let e = event(
            1,
            "se",
            vec![
                cmd(140, &[0x0200_0020, 80, 0, 0, 100, 100], &["SE/click"]),
                cmd(121, &[2_000_000, 1, 0, 0], &[]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(host.sounds[0].channel, SoundChannel::Se);
        assert_eq!(host.sounds[0].action, SoundAction::Play);
        assert_eq!(host.sounds[0].file, "SE/click");
        assert_eq!(host.sounds[0].volume, 80);
        assert_eq!(vm.vars.get(0), 1, "must not block");
    }

    #[test]
    fn bgm_stop_does_not_index_a_missing_string() {
        let e = event(1, "bgmstop", vec![cmd(140, &[-256, 30], &[])]);
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(host.sounds[0].action, SoundAction::Stop);
        assert_eq!(host.sounds[0].channel, SoundChannel::Bgm);
        assert_eq!(host.sounds[0].frames, 30);
    }

    #[test]
    fn teleport_blocks_one_frame() {
        let e = event(
            1,
            "teleport",
            vec![
                cmd(130, &[-1, 60, 34, 3, 1], &[]),
                cmd(121, &[2_000_000, 1, 0, 0], &[]),
            ],
        );
        let mut vm = vm_with(vec![e]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert_eq!(host.teleports, vec![(-1, 60, 34, 3)]);
        assert_eq!(vm.vars.get(0), 0);
        vm.step_frame(&mut host);
        assert_eq!(vm.vars.get(0), 1);
    }

    #[test]
    fn self_variables_are_per_instance() {
        let child = event(
            2,
            "child",
            vec![cmd(121, &[1_600_000, 5, 0, 0], &[]), cmd(172, &[], &[])],
        );
        let caller = event(
            1,
            "caller",
            vec![
                cmd(121, &[1_600_000, 1, 0, 0], &[]),
                cmd(210, &[500_002, 0], &[]),
                cmd(121, &[2_000_000, 1_600_000, 0, 0], &[]),
            ],
        );
        let mut vm = vm_with(vec![child, caller]);
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        vm.step_frame(&mut host);
        // v[0] receives the caller's own V6-0, which the child did not touch.
        assert_eq!(vm.vars.get(0), 1);
    }

    #[test]
    fn contains_its_own_budget_so_an_infinite_loop_cannot_hang() {
        let e = event(1, "spin", vec![cmd(170, &[], &[]), cmd(498, &[], &[])]);
        let mut vm = Vm::with_budget(
            {
                let data = wolf_format::commonevent::CommonEventData {
                    version_header: 0x55,
                    version: 0xC9,
                    events: vec![e],
                    footer: 0x90,
                };
                ProgramSet::from_common_events(&data)
            },
            50,
        );
        let mut host = Recorder::default();
        vm.start_event(1).unwrap();
        let report = vm.step_frame(&mut host);
        assert_eq!(report.steps, 50);
        assert_eq!(vm.instances().len(), 1, "still running, not wedged");
    }

    #[test]
    fn return_to_title_is_surfaced_to_the_host() {
        #[derive(Default)]
        struct TitleHost {
            titled: bool,
        }
        impl Host for TitleHost {
            fn return_to_title(&mut self) {
                self.titled = true;
            }
        }
        let e = event(1, "title", vec![cmd(174, &[], &[])]);
        let mut vm = vm_with(vec![e]);
        let mut host = TitleHost::default();
        vm.start_event(1).unwrap();
        vm.step_frame(&mut host);
        assert!(host.titled);
        assert_eq!(vm.instances().len(), 0);
    }
}
