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

use wolf_format::commonevent::CommonEventData;
use wolf_vm::{
    ChoiceReply, DbOp, Host, LogLevel, PictureCmd, ProgramSet, SoundCmd, StepReport, Vm,
};

use crate::assets::Assets;
use crate::picture::{PictureKind, Screen};

/// The event the game opens with in this build (the "original title screen A").
pub const TITLE_EVENT: u32 = 42;

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
    /// When true, messages dismiss immediately and choices answer index 0.
    pub auto_advance: bool,
    /// Which option the player has highlighted in the current choice prompt.
    choice_cursor: usize,
    /// Keys held on the previous frame, so a choice sees presses and not holds.
    previous_keys: u32,
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
            choice_cursor: 0,
            previous_keys: 0,
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
        self.stats.frames += 1;
        self.stats.instructions += report.steps as u64;
        self.stats.peak_instances = self.stats.peak_instances.max(report.running);
        let outcome = FrameOutcome {
            pictures: self.screen.len(),
            sounds: self.sounds.len(),
            idle: report.running == 0,
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
}

impl GameHost<'_> {
    fn note(&mut self, text: String) {
        *self.diagnostics.entry(text).or_insert(0) += 1;
        self.stats.unmodelled += 1;
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
        if self.auto_advance {
            *self.choices = None;
            return Some(ChoiceReply::Index(0));
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
        self.note(format!(
            "290 Effect: kind {kind} for {duration} frame(s) ({} args) not drawn yet",
            args.len()
        ));
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

    fn key_bits(&mut self) -> u32 {
        self.key_bits
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

    #[test]
    fn a_missing_data_root_is_an_error_not_a_panic() {
        let err = match Game::new("no-such-root") {
            Ok(_) => panic!("loading from a bogus root must fail"),
            Err(err) => err,
        };
        assert!(matches!(err, GameError::Io { .. }), "unexpected: {err}");
    }

    #[test]
    fn the_title_menu_advances_when_the_confirm_key_is_pressed() {
        let root = data_root();
        if !root.join("BasicData").join("CommonEvent.dat").is_file() {
            eprintln!("skipping: no game data at {}", root.display());
            return;
        }
        let mut game = Game::new(&root).expect("game");
        game.start_title().expect("title event");
        for _ in 0..30 {
            game.frame();
        }
        // With no keys held the title script parks on its menu: a handful of instructions
        // and then nothing, which is exactly what the original game does.
        let parked = game.stats.instructions;
        assert!(parked < 200, "the title should park quickly, ran {parked}");
        for _ in 0..30 {
            game.frame();
        }
        assert_eq!(
            game.stats.instructions, parked,
            "a parked title must not keep executing"
        );

        // A fresh confirm press has to wake it up.
        game.key_bits = key::OK;
        game.frame();
        game.key_bits = 0;
        for _ in 0..60 {
            game.frame();
        }
        assert!(
            game.stats.instructions > parked + 50,
            "confirm did not advance the menu: {parked} -> {}",
            game.stats.instructions
        );
    }

    #[test]
    fn the_title_event_runs_200_frames_and_draws_pictures() {
        let root = data_root();
        if !root.join("BasicData").join("CommonEvent.dat").is_file() {
            eprintln!("skipping: no game data at {}", root.display());
            return;
        }
        let mut game = Game::new(&root).expect("game");
        game.auto_advance = true;
        game.start_title().expect("title event");

        let mut peak = 0usize;
        let mut last = FrameOutcome::default();
        for _ in 0..200 {
            last = game.frame();
            peak = peak.max(last.report.running);
        }
        // The measured title closure: thousands of instructions, several coroutines,
        // a full complement of pictures, and zero interpreter warnings.
        assert!(game.stats.instructions > 10_000, "{}", game.stats.instructions);
        assert!(peak >= 2, "peak coroutines {peak}");
        assert!(!game.screen.is_empty(), "no pictures after 200 frames");
        assert_eq!(game.stats.warnings, 0, "diagnostics: {:?}", game.diagnostics);
        for picture in game.screen.iter() {
            eprintln!(
                "  pic {:>6} kind={} file={} text={} at ({},{}) scale {} alpha {:.2} screen-layer {}",
                picture.number,
                picture.kind as u8,
                picture.file,
                picture.text,
                picture.x,
                picture.y,
                picture.scale,
                picture.alpha(),
                picture.is_screen_layer()
            );
        }
        assert!(
            game.screen.iter().any(|picture| picture.file.contains("TitleGraphic")),
            "no title graphic among {:?}",
            game.screen.iter().map(|p| p.file.clone()).collect::<Vec<_>>()
        );
        assert!(game.stats.pictures > 1000, "{} picture ops", game.stats.pictures);
        assert!(last.report.steps > 0 || last.idle, "frame report {last:?}");
    }
}
