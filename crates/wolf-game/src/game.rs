//! The game layer: one VM, one picture screen, one asset database, one frame at a time.
//!
//! [`Game`] is the piece that turns the interpreter into something a window can draw. It
//! implements [`Host`] itself (through a short-lived borrow shim, because the VM needs
//! the host while living inside the game), so every WOLF callback - pictures, sounds,
//! database reads, messages, choices - lands in typed state the renderer can consume.
//!
//! Two rules keep this honest:
//!
//! * Nothing here invents game state. If the VM asks for something we do not model yet,
//!   the request is recorded as a diagnostic and dropped; the frame still runs. A port
//!   that panics on the first unknown opcode never gets past the title screen.
//! * The frame boundary is explicit. Input is latched at the top of [`Game::frame`];
//!   everything the VM produced during the frame is readable when it returns. That makes
//!   a recorded session replayable, which is how the save-compat work will be verified.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use std::sync::Arc;

use wolf_format::commonevent::CommonEventData;
use wolf_format::mps::Map;
use wolf_vm::{
    ChoiceReply, DbOp, Host, LogLevel, PictureCmd, Program, ProgramSet, SoundCmd, StepReport,
    Vm,
};

use crate::assets::Assets;
use crate::map_runtime::MapRuntime;
use crate::picture::{PictureKind, Screen, ANCHOR_CENTRE, SCREEN_H, SCREEN_W};

/// The event the game opens with in this build (the "original title screen A").
pub const TITLE_EVENT: u32 = 42;

/// The map the original boots into (SysDatabase type 0, row 0).
pub const BOOT_MAP: &str = "MapData/TitleMap.mps";

/// The autorun event on [`BOOT_MAP`] that plays the splashes and reaches the title.
pub const BOOT_EVENT: u32 = 2;

/// One visible message window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// Text as the script produced it, escape-expanded, with the @-face prefix removed.
    pub text: String,
    /// Face graphic id from the @-prefix, when the script carries one.
    pub face: Option<i32>,
    /// Frames this message has been on screen.
    pub frames: u32,
}

/// A pending choice prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choices {
    /// The option labels, in order.
    pub options: Vec<String>,
    /// Frames this prompt has been on screen.
    pub frames: u32,
}

/// Counters that make a headless run comparable to an interactive one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameStats {
    /// Frames advanced.
    pub frames: u64,
    /// Instructions executed across all coroutines.
    pub instructions: u64,
    /// Most coroutines alive at the end of any frame.
    pub peak_instances: usize,
    /// Picture commands applied.
    pub pictures: u64,
    /// Sound commands seen.
    pub sounds: u64,
    /// Database accesses seen.
    pub databases: u64,
    /// Messages shown.
    pub messages: u64,
    /// Choice prompts shown.
    pub choices: u64,
    /// Warnings and errors logged by the interpreter.
    pub warnings: u64,
    /// Requests this layer does not model yet (deduplicated in [`Game::diagnostics`]).
    pub unmodelled: u64,
}

/// What one frame produced.
#[derive(Clone, Debug, Default)]
pub struct FrameOutcome {
    /// The interpreter's own per-frame report.
    pub report: StepReport,
    /// Pictures live at the end of the frame.
    pub pictures: usize,
    /// Sounds requested during the frame (the audio layer consumes these).
    pub sounds: usize,
    /// True once nothing is running any more.
    pub idle: bool,
}

/// Why the game could not be created.
#[derive(Debug)]
pub enum GameError {
    /// A data file could not be read.
    Io {
        /// Path that failed.
        path: PathBuf,
        /// Underlying error.
        source: std::io::Error,
    },
    /// A data file parsed incorrectly.
    Format(wolf_format::Error),
    /// The title event does not exist in this build.
    MissingEvent(u32),
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "cannot read {}: {source}", path.display()),
            Self::Format(err) => write!(f, "malformed game data: {err}"),
            Self::MissingEvent(id) => write!(f, "common event {id} is not in this build"),
        }
    }
}

impl std::error::Error for GameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Format(err) => Some(err),
            Self::MissingEvent(_) => None,
        }
    }
}

/// The running game.
pub struct Game {
    /// The interpreter.
    pub vm: Vm,
    /// Live pictures, in WOLF draw order.
    pub screen: Screen,
    /// Decoded assets, cached by WOLF asset name.
    pub assets: Assets,
    /// Keys held this frame (see crate::key).
    pub key_bits: u32,
    /// Sounds requested during the current frame.
    pub sounds: Vec<SoundCmd>,
    /// Message windows on screen.
    pub messages: Vec<Message>,
    /// Pending choice prompt.
    pub choices: Option<Choices>,
    /// Counters.
    pub stats: GameStats,
    /// Deduplicated diagnostics, keyed by message, valued by hit count.
    pub diagnostics: BTreeMap<String, u64>,
    /// Queued answers for choice prompts (front is delivered first).
    pub replies: std::collections::VecDeque<ChoiceReply>,
    /// When true, messages dismiss immediately.
    pub auto_advance: bool,
    /// When true, a choice prompt answers itself with the highlighted option.
    pub auto_choice: bool,
    /// The automatic pages of the map the game booted into.
    pub runtime: MapRuntime,
    /// Mouse position in logical pixels, [-1, -1] when the pointer is outside.
    pub mouse: [i32; 2],
    /// Mouse buttons held this frame: bit 0 left, bit 1 right, bit 2 middle.
    pub mouse_buttons: u32,
    /// Which option the player has highlighted in the current choice prompt.
    choice_cursor: usize,
    /// Keys held on the previous frame, so a choice sees presses and not holds.
    previous_keys: u32,
    /// Mouse buttons held on the previous frame, so a press can be told from a hold.
    previous_mouse_buttons: u32,
    root: PathBuf,
}

impl Game {
    /// Loads the data needed to run events and builds an idle VM.
    ///
    /// # Errors
    ///
    /// Fails when Game.dat or CommonEvent.dat is missing or malformed, because without
    /// them no event can run at all.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, GameError> {
        let root = root.into();
        let basic = if root.join("BasicData").is_dir() {
            root.join("BasicData")
        } else {
            root.clone()
        };
        let events = read_events(&basic.join("CommonEvent.dat"))?;
        let programs = ProgramSet::from_common_events(&events);
        Ok(Self {
            vm: Vm::new(programs),
            screen: Screen::new(),
            assets: Assets::new(&root),
            key_bits: 0,
            sounds: Vec::new(),
            messages: Vec::new(),
            choices: None,
            stats: GameStats::default(),
            diagnostics: BTreeMap::new(),
            replies: std::collections::VecDeque::new(),
            auto_advance: false,
            auto_choice: false,
            runtime: MapRuntime::new(),
            mouse: [-1, -1],
            mouse_buttons: 0,
            choice_cursor: 0,
            previous_keys: 0,
            previous_mouse_buttons: 0,
            root,
        })
    }

    /// The data root this game was created from.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Starts one common event as a coroutine. Returns false when the id is unknown.
    pub fn start_event(&mut self, id: u32) -> bool {
        self.vm.start_event(id).is_some()
    }

    /// Boots the game the way the original does: the autorun event on the title map.
    ///
    /// Common event 42 is *not* the entry point. The measured boot chain is
    /// [`BOOT_MAP`] event 2 -> common event 549 (three splashes) -> a key wait ->
    /// common event 42 (the three-option menu), which is why starting at 42 shows the
    /// menu over a title graphic the original has not drawn yet.
    ///
    /// # Errors
    ///
    /// Fails when the map file is missing or malformed, or when it has no event 2.
    pub fn boot(&mut self) -> Result<(), GameError> {
        let bytes = self.read(BOOT_MAP)?;
        let map = Map::parse(&bytes).map_err(GameError::Format)?;
        self.start_pages(&map, Some(BOOT_EVENT));
        if self.start_map_event(&map, BOOT_EVENT) {
            Ok(())
        } else {
            Err(GameError::MissingEvent(BOOT_EVENT))
        }
    }

    /// Starts one map event first page as a coroutine.
    ///
    /// A map page is an ordinary instruction stream, so it compiles through the same
    /// jump-table precompiler the common events use. Only the first page runs: later
    /// pages need the page-switch machinery that M2 adds.
    /// Start every automatic page of `map` and remember them.
    ///
    /// The boot map's own event is started separately by `start_map_event`,
    /// because `BOOT_EVENT` is the title script and must run exactly once
    /// (an autorun page would be started by `MapRuntime` too, and starting it
    /// twice is a behaviour change nobody asked for).
    pub fn start_pages(&mut self, map: &Map, skip: Option<u32>) -> usize {
        self.runtime = MapRuntime::collect_except(map, skip);
        self.runtime.start_pending(&mut self.vm)
    }

    /// Start one event of a named map, collecting that map's other pages too.
    ///
    /// This is what a new game does once the title menu answers: it switches to
    /// SampleMapA and runs event 0, the opening story script. The event itself
    /// is skipped by the page collector so its coroutine is only started once.
    ///
    /// # Errors
    ///
    /// Fails when the map file is missing or malformed, or when the map has no
    /// event with that id.
    pub fn start_map_event_by_name(
        &mut self,
        map_name: &str,
        event_id: u32,
    ) -> Result<(), GameError> {
        let name = if map_name.contains('.') {
            map_name.to_owned()
        } else {
            format!("MapData/{map_name}.mps")
        };
        let bytes = self.read(&name)?;
        let map = Map::parse(&bytes).map_err(GameError::Format)?;
        self.start_pages(&map, Some(event_id));
        if self.start_map_event(&map, event_id) {
            Ok(())
        } else {
            Err(GameError::MissingEvent(event_id))
        }
    }

    pub fn start_map_event(&mut self, map: &Map, event_id: u32) -> bool {
        let Some(event) = map.events.iter().find(|event| event.id == event_id) else {
            return false;
        };
        let Some(page) = event.pages.first() else {
            return false;
        };
        let program = Program::from_slots(event.id, &event.name, page.commands.clone());
        self.vm.start_program(Arc::new(program)).is_some()
    }

    fn read(&self, name: &str) -> Result<Vec<u8>, GameError> {
        let relative: std::path::PathBuf = name.split('/').collect();
        let path = self.root.join(relative);
        std::fs::read(&path).map_err(|source| GameError::Io { path, source })
    }

    /// Starts the title screen event.
    ///
    /// # Errors
    ///
    /// Fails when this build has no event 42.
    pub fn start_title(&mut self) -> Result<(), GameError> {
        if self.start_event(TITLE_EVENT) {
            Ok(())
        } else {
            Err(GameError::MissingEvent(TITLE_EVENT))
        }
    }

    /// Queues an answer for the next choice prompt.
    pub fn reply(&mut self, reply: ChoiceReply) {
        self.replies.push_back(reply);
    }

    /// Highlighted option of the current choice prompt (0 when none is open).
    #[must_use]
    pub fn choice_cursor(&self) -> usize {
        self.choice_cursor
    }

    /// Where the engine puts the current choice window, from system variables 3 (X) and
    /// 4 (Y).
    ///
    /// WOLF has no centred default: [`102`] paints its window at exactly these two
    /// numbers (docs/OPCODE_102.md). The title event sets them to (512, 1000), which is
    /// why the original title screen shows no menu box at all.
    #[must_use]
    pub fn choice_anchor(&self) -> (i32, i32) {
        (self.vm.vars.get_sys(3), self.vm.vars.get_sys(4))
    }

    /// Preselects an option of the current choice prompt.
    ///
    /// Headless captures use this together with [`Self::auto_choice`] to take a
    /// specific branch without a keyboard.
    pub fn set_choice_cursor(&mut self, index: usize) {
        self.choice_cursor = index;
    }

    /// Applies this frame's key state to the open choice prompt, if there is one.
    ///
    /// WOLF's runtime owns choice navigation: opcode 102 hands the runtime a list of
    /// options and blocks the coroutine, and the runtime is what moves the cursor and
    /// returns an index. The port has to do the same, otherwise a script that opens a
    /// menu simply stops - the measured title screen does exactly that, advancing 45
    /// instructions and then waiting forever.
    fn handle_choice_input(&mut self) {
        let Some(choices) = &self.choices else {
            self.choice_cursor = 0;
            return;
        };
        let count = choices.options.len();
        if count == 0 {
            return;
        }
        // Only fresh presses count: holding a key must not scroll a menu at 60 steps a
        // second, which is what a level-triggered read would do.
        let pressed = self.key_bits & !self.previous_keys;
        if pressed & crate::key::UP != 0 {
            self.choice_cursor = (self.choice_cursor + count - 1) % count;
        }
        if pressed & crate::key::DOWN != 0 {
            self.choice_cursor = (self.choice_cursor + 1) % count;
        }
        self.choice_cursor = self.choice_cursor.min(count - 1);
        if pressed & crate::key::OK != 0 {
            self.reply(ChoiceReply::Index(self.choice_cursor));
            self.choice_cursor = 0;
        } else if pressed & crate::key::CANCEL != 0 {
            self.reply(ChoiceReply::Cancel);
            self.choice_cursor = 0;
        }
    }

    /// Advances one frame at 1/60 s.
    pub fn frame(&mut self) -> FrameOutcome {
        self.sounds.clear();
        self.handle_choice_input();
        let mut report = self.vm.step_frame(&mut GameHost {
            screen: &mut self.screen,
            assets: &mut self.assets,
            sounds: &mut self.sounds,
            messages: &mut self.messages,
            choices: &mut self.choices,
            replies: &mut self.replies,
            diagnostics: &mut self.diagnostics,
            stats: &mut self.stats,
            key_bits: self.key_bits,
            auto_advance: self.auto_advance,
            auto_choice: self.auto_choice,
            cursor: self.choice_cursor,
            mouse: self.mouse,
            mouse_buttons: self.mouse_buttons,
            previous_mouse_buttons: self.previous_mouse_buttons,
        });
        // Age the UI state after the VM ran, so a message shown this frame is 0 frames old.
        for message in &mut self.messages {
            message.frames = message.frames.saturating_add(1);
        }
        if let Some(choices) = &mut self.choices {
            choices.frames = choices.frames.saturating_add(1);
        }
        self.screen.tick();
        self.previous_keys = self.key_bits;
        // Automatic pages are restarted here, i.e. at the end of a frame, so a
        // parallel page that just ended comes back on the next tick.
        self.runtime.start_pending(&mut self.vm);
        self.previous_mouse_buttons = self.mouse_buttons;
        self.stats.frames += 1;
        self.stats.instructions += report.steps as u64;
        self.stats.peak_instances = self.stats.peak_instances.max(report.running);
        let outcome = FrameOutcome {
            pictures: self.screen.len(),
            sounds: self.sounds.len(),
            // A coroutine parked on a menu is alive but does no work, and that is what
            // a renderer needs to know: idle means this frame executed nothing.
            idle: report.steps == 0,
            report: StepReport::default(),
        };
        report.notes.clear();
        // Keep the interpreter note list empty: everything worth keeping went through
        // Host::log, which deduplicates. A per-frame Vec would otherwise grow forever.
        FrameOutcome {
            report,
            ..outcome
        }
    }
}

fn read_events(path: &Path) -> Result<CommonEventData, GameError> {
    let bytes = std::fs::read(path).map_err(|source| GameError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    CommonEventData::parse(&bytes).map_err(GameError::Format)
}

/// Per-frame borrow shim: the VM lives inside Game, so the host cannot borrow Game.
struct GameHost<'a> {
    screen: &'a mut Screen,
    assets: &'a mut Assets,
    sounds: &'a mut Vec<SoundCmd>,
    messages: &'a mut Vec<Message>,
    choices: &'a mut Option<Choices>,
    replies: &'a mut std::collections::VecDeque<ChoiceReply>,
    diagnostics: &'a mut BTreeMap<String, u64>,
    stats: &'a mut GameStats,
    key_bits: u32,
    auto_advance: bool,
    auto_choice: bool,
    cursor: usize,
    mouse: [i32; 2],
    mouse_buttons: u32,
    previous_mouse_buttons: u32,
}

impl GameHost<'_> {
    fn note(&mut self, text: String) {
        *self.diagnostics.entry(text).or_insert(0) += 1;
        self.stats.unmodelled += 1;
    }

    /// 1 when the pointer is over the picture, 0 when not, -1 when there is no such
    /// picture (docs/OPCODE_124.md field 8).
    ///
    /// The test is a plain axis-aligned box that includes transparent pixels, as the
    /// engine documentation says: a picture hit area is its rectangle, not its alpha.
    fn hover(&mut self, number: i32) -> i32 {
        let Some(picture) = self.screen.get(number) else {
            return -1;
        };
        let scale = f32::from(picture.scale.max(1) as i16) / 100.0;
        let size = match picture.window {
            Some((width, height)) if width > crate::window::MIN_BOX && height > crate::window::MIN_BOX => {
                (width as f32 * scale, height as f32 * scale)
            }
            _ if picture.is_screen_layer() => (SCREEN_W as f32, SCREEN_H as f32),
            _ => {
                let Some((width, height)) = self.assets.texture_size(&picture.file) else {
                    return -1;
                };
                (width as f32 * scale, height as f32 * scale)
            }
        };
        let mut left = picture.x as f32;
        let mut top = picture.y as f32;
        if picture.anchor == ANCHOR_CENTRE {
            left -= size.0 / 2.0;
            top -= size.1 / 2.0;
        }
        let (px, py) = (self.mouse[0] as f32, self.mouse[1] as f32);
        let inside_screen = self.mouse[0] >= 0
            && self.mouse[1] >= 0
            && px < SCREEN_W as f32
            && py < SCREEN_H as f32;
        let over = inside_screen
            && px >= left
            && px < left + size.0
            && py >= top
            && py < top + size.1;
        i32::from(over)
    }
}

/// WOLF reports a mouse button as 0 released, 1 held, 2 pressed this frame.
fn mouse_state(held: bool, held_before: bool) -> i32 {
    if !held {
        0
    } else if held_before {
        1
    } else {
        2
    }
}

impl Host for GameHost<'_> {
    fn show_message(&mut self, text: &str, face: Option<i32>) {
        self.messages.push(Message {
            text: text.to_owned(),
            face,
            frames: 0,
        });
        self.stats.messages += 1;
    }

    fn message_dismissed(&mut self) -> bool {
        // A human dismisses messages with the confirm key; a headless run dismisses them
        // immediately so a scripted capture can walk through a conversation.
        self.auto_advance || self.key_bits & crate::key::OK != 0
    }

    fn close_message(&mut self) {
        self.messages.clear();
    }

    fn show_choices(&mut self, options: &[String]) {
        *self.choices = Some(Choices {
            options: options.to_vec(),
            frames: 0,
        });
        self.stats.choices += 1;
    }

    fn choice_made(&mut self) -> Option<ChoiceReply> {
        if let Some(reply) = self.replies.pop_front() {
            *self.choices = None;
            return Some(reply);
        }
        if self.auto_choice {
            *self.choices = None;
            let index = self.cursor;
            return Some(ChoiceReply::Index(index));
        }
        None
    }

    fn picture(&mut self, cmd: &PictureCmd) {
        self.screen.apply(cmd);
        self.stats.pictures += 1;
        // Warm the image cache for file pictures while the frame is running, so the
        // render path never blocks on a decode: by the time the frame ends, every picture
        // on screen already has its size (and usually its pixels) resident.
        if !PictureKind::from_code(cmd.kind).is_text() && !cmd.file.is_empty() {
            let _ = self.assets.texture_size(&cmd.file);
        }
    }

    fn sound(&mut self, cmd: &SoundCmd) {
        self.sounds.push(cmd.clone());
        self.stats.sounds += 1;
    }

    fn database(&mut self, op: &DbOp) {
        let _ = op;
        self.stats.databases += 1;
    }

    fn move_route(&mut self, target: i32, route: &[wolf_format::command::RouteCommand]) {
        let _ = target;
        self.note(format!(
            "201 SetMoveRoute: {} command(s) not applied (no map layer yet)",
            route.len()
        ));
    }

    fn teleport(&mut self, map: i32, x: i32, y: i32, direction: i32) {
        self.note(format!(
            "130 Teleport: map {map} at ({x},{y}) direction {direction} - map layer arrives in M2"
        ));
    }

    fn transition(&mut self, kind: i32, flags: i32) {
        self.note(format!("160/162 Transition: kind {kind} flags {flags} not drawn yet"));
    }

    fn party(&mut self, args: &[i32]) {
        self.note(format!("270 Party: {} argument(s) recorded only", args.len()));
    }

    fn scroll(&mut self, args: &[i32]) {
        self.note(format!("281 Scroll: {} argument(s) recorded only", args.len()));
    }

    fn effect(&mut self, kind: i32, duration: i32, args: &[i32]) {
        // Colour effects belong to the picture store, not to the interpreter, because they
        // outlive the instruction: a type 5 flash keeps cycling for as long as the picture
        // exists. Everything else is recorded but not drawn - see Screen::effect.
        if !self.screen.effect(kind, duration, args) {
            self.note(format!(
                "290 Effect: kind {kind} for {duration} frame(s) ({} args) not drawn yet",
                args.len()
            ));
        }
    }

    fn save_load(&mut self, args: &[i32]) {
        self.note(format!("220/221/222 SaveLoad: args {args:?} not modelled yet"));
    }

    fn reserve(&mut self, event: wolf_vm::exec::EventId) {
        self.note(format!("211 Reserve: event {event} not scheduled yet"));
    }

    fn debug_message(&mut self, text: &str) {
        *self
            .diagnostics
            .entry(format!("106 debug line: {text}"))
            .or_insert(0) += 1;
    }

    fn clear_debug(&mut self) {}

    fn return_to_title(&mut self) {
        self.note("174 ReturnToTitle: the title closure does not use it yet".to_owned());
    }

    fn end_game(&mut self) {
        self.note("175 EndGame: ignored (a port never closes the window from script)".to_owned());
    }

    fn pro_feature(&mut self, kind: i32, ms: i32) {
        self.note(format!("1000 ProFeature: type {kind} ({ms} ms) not modelled"));
    }

    fn log(&mut self, level: LogLevel, text: &str) {
        match level {
            LogLevel::Warn | LogLevel::Error => {
                self.stats.warnings += 1;
                *self.diagnostics.entry(text.to_owned()).or_insert(0) += 1;
            }
            LogLevel::Info => self.note(text.to_owned()),
            LogLevel::Trace => {}
        }
    }

    fn key_code(&mut self, mask: u32) -> i32 {
        // The mask is deliberately unused: one mask can yield six different codes,
        // see crates/wolf-game/src/key.rs.
        let _ = mask;
        crate::key::code(self.key_bits)
    }

    fn picture_state(&mut self, number: i32, field: i32) -> Option<i32> {
        // Fields 8 and 9 answer for a picture that does not exist either (-1 vs 0),
        // so they run before the existence check below.
        if field == 8 {
            return Some(self.hover(number));
        }
        if field == 9 {
            return Some(i32::from(self.screen.get(number).is_some()));
        }
        let picture = self.screen.get(number)?;
        match field {
            0 => Some(picture.x),
            1 => Some(picture.y),
            2 | 3 => Some(picture.scale),
            4 => picture.extras.first().copied(),
            10 => Some(if PictureKind::from_code(picture.kind as u8).is_text() {
                1
            } else {
                -1
            }),
            _ => None,
        }
    }

    fn character_state(&mut self, target: i32, field: i32) -> Option<i32> {
        // No map layer until M2: leaving the destination alone is honest, while
        // fabricating coordinates would silently break follower logic.
        let _ = (target, field);
        None
    }

    fn engine_state(&mut self, field: i32) -> Option<i32> {
        match field {
            // Static map id and BGM data id: both unset in the port so far.
            0 => Some(0),
            1 => Some(-1),
            7 => Some(mouse_state(
                self.mouse_buttons & 1 != 0,
                self.previous_mouse_buttons & 1 != 0,
            )),
            8 => Some(mouse_state(
                self.mouse_buttons & 2 != 0,
                self.previous_mouse_buttons & 2 != 0,
            )),
            9 => Some(mouse_state(
                self.mouse_buttons & 4 != 0,
                self.previous_mouse_buttons & 4 != 0,
            )),
            10 => Some(0),
            20 => Some(self.mouse[0]),
            21 => Some(self.mouse[1]),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key;

    fn data_root() -> PathBuf {
        if let Ok(value) = std::env::var("WOLF_DATA_ROOT") {
            if !value.is_empty() {
                return PathBuf::from(value);
            }
        }
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.pop();
        path.push("work");
        path.push("full");
        path
    }

    fn have_data() -> Option<PathBuf> {
        let root = data_root();
        if root.join("BasicData").join("CommonEvent.dat").is_file() {
            Some(root)
        } else {
            eprintln!("skipping: no game data at {}", root.display());
            None
        }
    }

    #[test]
    fn a_missing_data_root_is_an_error_not_a_panic() {
        let err = match Game::new("no-such-root") {
            Ok(_) => panic!("loading from a bogus root must fail"),
            Err(err) => err,
        };
        assert!(matches!(err, GameError::Io { .. }), "unexpected: {err}");
    }

    #[test]
    fn the_title_event_parks_on_its_menu_after_one_graphic() {
        let Some(root) = have_data() else { return };
        let mut game = Game::new(&root).expect("game");
        game.start_title().expect("title event");
        let mut peak = 0usize;
        let mut last = FrameOutcome::default();
        for _ in 0..200 {
            last = game.frame();
            peak = peak.max(last.report.running);
        }
        // The real title script: draw one full-screen graphic, start one BGM, open a
        // three-option prompt. Everything the old capture showed beyond that came from a
        // branch fall-through bug that reached the save/load screen chain.
        assert!(
            (30..=120).contains(&game.stats.instructions),
            "{} instruction(s)",
            game.stats.instructions
        );
        assert_eq!(peak, 1, "the title event is a single coroutine");
        assert!(game.choices.is_some(), "the title prompt should be open");
        assert_eq!(game.stats.pictures, 1);
        assert!(last.idle, "a parked title does no work");
        let pictures: Vec<_> = game.screen.iter().collect();
        assert_eq!(pictures.len(), 1);
        assert_eq!(pictures[0].number, -2, "the title layer is the whole screen");
        assert!(pictures[0].is_screen_layer());
        assert!(pictures[0].file.contains("TitleGraphic"));
        assert_eq!(
            game.stats.warnings, 0,
            "diagnostics: {:?}",
            game.diagnostics
        );
    }

    #[test]
    fn the_title_menu_advances_when_the_confirm_key_is_pressed() {
        let Some(root) = have_data() else { return };
        let mut game = Game::new(&root).expect("game");
        game.start_title().expect("title event");
        for _ in 0..30 {
            game.frame();
        }
        // With no keys held the script parks on its menu: a handful of instructions and
        // then nothing, which is what the original game does.
        let parked = game.stats.instructions;
        assert!(parked < 200, "the title should park quickly, ran {parked}");
        for _ in 0..30 {
            game.frame();
        }
        assert_eq!(
            game.stats.instructions, parked,
            "a parked title must not keep executing"
        );

        // A fresh confirm press has to wake it up and close the prompt.
        game.key_bits = key::OK;
        game.frame();
        game.key_bits = 0;
        for _ in 0..60 {
            game.frame();
        }
        assert!(
            game.stats.instructions > parked,
            "confirm did not advance the menu: {parked} -> {}",
            game.stats.instructions
        );
        assert!(game.choices.is_none(), "the prompt should be answered");
    }

    #[test]
    fn booting_reaches_the_title_art_and_confirm_opens_the_menu() {
        let Some(root) = have_data() else { return };
        let mut game = Game::new(&root).expect("game");
        if let Err(err) = game.boot() {
            eprintln!("skipping: {err}");
            return;
        }
        for _ in 0..800 {
            game.frame();
        }
        // Three splashes and then the real title art, which is a picture and not a common
        // event: the boot map event draws it before it calls event 42.
        let art = game
            .screen
            .iter()
            .find(|picture| picture.file.contains("title01"))
            .expect("the title art should be on screen");
        assert_eq!(art.number, 12000);
        assert_eq!((art.x, art.y), (0, 0));
        let banner = game
            .screen
            .iter()
            .find(|picture| picture.file.contains("Button01"))
            .expect("the hint banner should be on screen");
        assert_eq!(banner.anchor, ANCHOR_CENTRE, "head word 4096 centres it");
        assert_eq!((banner.x, banner.y), (512, 520));

        // One confirm press plays the door sound, erases the banner and enters the menu.
        game.key_bits = key::OK;
        game.frame();
        game.key_bits = 0;
        for _ in 0..240 {
            game.frame();
        }
        let choices = game
            .choices
            .as_ref()
            .expect("confirm should land on the title menu");
        assert_eq!(choices.options, ["1", "2", "3"]);
        assert!(
            game.screen
                .iter()
                .all(|picture| !picture.file.contains("Button01")),
            "the hint banner is erased on the way into the menu"
        );
    }
}
