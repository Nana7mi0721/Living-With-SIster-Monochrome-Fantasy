//! The `150` Picture layer system.
//!
//! WOLF's pictures are not "one image on screen": every picture number is a persistent
//! slot with its own position, scale, opacity, blend mode and animation state, mutated in
//! place by later `150` instructions. The title screen alone issues 3,261 picture
//! operations in 200 frames (measured with `wolf-cli run --event 42`), and `150` is
//! the third most common opcode in the game (16,716 sites), so this store - not the
//! interpreter - is where most of the visual work lands.
//!
//! # Field layout (measured, not quoted)
//!
//! docs/OPCODE_SPEC.md section 5.20 describes the editor's argument order. Dumping the real
//! instructions out of work/full/BasicData/CommonEvent.dat shows a different but consistent
//! order for the long form (argc >= 11, after the type/number words):
//!
//! | args[..] | meaning | evidence |
//! |---|---|---|
//! | 0 | unknown flag word (0 everywhere except the title background, which uses 30) | ev42 pc20 |
//! | 1,2 | window width/height - only meaningful for the window picture kinds | ev42 draws a 300x200 window at (855,200) |
//! | 3 | constant 1 in every sample | |
//! | 4 | opacity, 0..255 | always 255 on the title path |
//! | 5 | x | pc43 feeds self-variables here |
//! | 6 | y | |
//! | 7 | uniform scale in percent, 100 = 1.0 | picture 12501 draws text at 100 |
//! | 8 | blend mode | 0 everywhere on the title path |
//! | 9.. | per-kind extras (window decoration flags, animation patterns, ...) | kept verbatim |
//!
//! The decisive sample is ev42 pc37: [0, -2, 0, 1, 1, 1, 255, 0, 0, 100, 0]
//! -> type 0 (file), number -2, window 1x1, opacity 255, x=0, y=0, scale=100, blend=0.
//! Reading x/y from args[0]/args[1] instead (as the editor docs suggest for short forms)
//! would put the 1024x576 title image at scale 1% - a 10x6 pixel dot.
//!
//! # The head word carries an anchor code in bits 12..14
//!
//! Grouping the 13,637 long-form draws of this game by that nibble shows the x/y values
//! are not free coordinates but are measured from an anchor:
//!
//! | bits | count | x seen | y seen | reading |
//! |---|---|---|---|---|
//! | 0x0000 | 8,911 | x=0 (5,782x) | y=0 (5,047x) | top-left, x/y is the corner |
//! | 0x1000 | 4,393 | x=512 (788x) | y=288 (216x) | centred on x/y |
//! | 0x2000 | 84 | x=384 (57x) | y=570 | unverified |
//! | 0x3000 | 105 | x=1000 | y=60 | unverified |
//! | 0x4000 | 67 | x=1024 | y=570 | unverified |
//! | 0x5000 | 8 | x=512 | y=0/100 | unverified |
//! | 0x6000 | 69 | x=800 | y=576 | unverified |
//!
//! 512 is exactly 1024/2 and 288 is exactly 576/2, i.e. the logical screen centre. The
//! decisive check is the retail title screen: TitleMap event 2 draws SystemFile/Button01.png
//! (606x35) with the 0x1000 flag at (512, 520). Anchored at the corner that banner would
//! cover x 512..1118 (clipped at the right edge) and y 520..555; the real screenshot shows
//! its ink at x 232..794, y 504..535 - which is the banner centred on (512, 520), with the
//! transparent margin of the PNG accounting for the remaining pixels. So 0x1000 anchors
//! BOTH axes on the centre.
//!
//! Only 0x0000 and 0x1000 are implemented; the other nibbles keep the corner behaviour and
//! are listed as unverified in docs/UNVERIFIED.md, because no title-screen instruction uses
//! them and guessing would move live pictures to the wrong place.
//!
//! # Negative numbers are whole-screen layers
//!
//! Picture numbers below zero are the screen-level pictures. Every instance in this game
//! draws a 1024x576 image with x=y=0 and scale=100, which is exactly the game's logical
//! screen (see GameDat::screen_size), so they are treated as full-screen layers and their
//! position/scale fields are ignored. This is hypothesis H-PIC-1 in docs/UNVERIFIED.md: it
//! must be confirmed against the retail game before M2 assets are considered final,
//! because if it is wrong the title screen renders at the wrong size rather than not at all.

use std::collections::BTreeMap;

use wolf_vm::{PictureAction, PictureCmd};

/// The sentinel WOLF stores in a `150 Move` argument that must not change.
///
/// Only ever seen at the fields listed in the module docs (146/127/474/166 occurrences in
/// this game), never as a real coordinate.
pub const UNCHANGED: i32 = -1_000_000;

/// Logical screen width used when a picture is a whole-screen layer.
pub const SCREEN_W: u32 = 1024;
/// Logical screen height used when a picture is a whole-screen layer.
pub const SCREEN_H: u32 = 576;

/// Anchor code 0: x/y name the picture top-left corner.
pub const ANCHOR_TOP_LEFT: u8 = 0;
/// Anchor code 1 (head bit 0x1000): x/y name the picture centre.
pub const ANCHOR_CENTRE: u8 = 1;
/// Scale field value that means "no scaling" (percent).
pub const SCALE_ONE: i32 = 100;

/// Field indices inside PictureCmd::args, from the table in the module docs.
pub mod field {
    /// Unknown flag word; 0 except for the title background (30).
    pub const FLAG: usize = 0;
    /// Fade-in length in frames, same slot as FLAG. 0 means the picture appears at once.
    /// Evidence: every splash draw in the boot chain carries 20 here and the retail
    /// disclaimer frame is exactly trunc(255 * age / 20) of the source art.
    pub const FADE_IN: usize = 0;
    /// Window width, for the window picture kinds.
    pub const WINDOW_W: usize = 1;
    /// Window height, for the window picture kinds.
    pub const WINDOW_H: usize = 2;
    /// Constant 1 in every sample.
    pub const RESERVED: usize = 3;
    /// Opacity, 0..=255.
    pub const OPACITY: usize = 4;
    /// X position, in logical pixels.
    pub const X: usize = 5;
    /// Y position, in logical pixels.
    pub const Y: usize = 6;
    /// Uniform scale in percent (100 = 1.0).
    pub const SCALE: usize = 7;
    /// Blend mode.
    pub const BLEND: usize = 8;
    /// First field that is kind-specific.
    pub const EXTRAS: usize = 9;
    /// Red channel of the base tone, percent (100 = neutral).
    ///
    /// docs/OPCODE_290.md section 3.1 numbers these slots 15..17 in the raw
    /// instruction; PictureCmd::args starts at raw arg 2, so they land at 13..15
    /// here. Only the long form (raw argc 18 or more) carries them at all.
    pub const TONE_R: usize = 13;
    /// Green channel of the base tone, percent.
    pub const TONE_G: usize = 14;
    /// Blue channel of the base tone, percent.
    pub const TONE_B: usize = 15;
}

/// The five picture kinds, decoded from `(arg0 >> 4) & 7` by the interpreter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureKind {
    /// 0: a file named literally.
    File,
    /// 1: a file name read out of a string variable.
    FileString,
    /// 2: text drawn straight into the picture.
    Text,
    /// 3: a window (frame graphic) around a file.
    WindowFile,
    /// 4: a window around text.
    WindowString,
}

impl PictureKind {
    /// Maps the interpreter's already-masked type code (0..=4) onto a kind.
    ///
    /// Unknown codes fall back to PictureKind::File rather than failing: an unrecognised
    /// picture must still occupy its slot, otherwise later `150 Move` instructions
    /// would silently target nothing.
    #[must_use]
    pub fn from_code(code: u8) -> Self {
        match code {
            1 => Self::FileString,
            2 => Self::Text,
            3 => Self::WindowFile,
            4 => Self::WindowString,
            _ => Self::File,
        }
    }

    /// True when the picture draws text rather than a texture.
    #[must_use]
    pub fn is_text(self) -> bool {
        matches!(self, Self::Text | Self::WindowString)
    }
}

/// Per-channel draw brightness in percent: 100 is neutral, 0 is black.
///
/// This is the shape DxLib SetDrawBright takes, and the measurements in docs/OPCODE_290.md
/// force it: the retail title banner is its own artwork times 222/255 on every channel,
/// independent of the pixels behind it.
pub type Tone = [i32; 3];

/// The tone a picture has before any effect touches it.
pub const TONE_NEUTRAL: Tone = [SCALE_ONE, SCALE_ONE, SCALE_ONE];

/// Effect kind 0 (head nibble): single flash, the colour jumps to the target and decays back.
pub const EFFECT_FLASH: u8 = 0;
/// Effect kind 1: cancel the running flash and return to the base tone.
pub const EFFECT_OFF: u8 = 1;
/// Effect kind 5: automatic flash, the same ramp repeated for as long as the picture lives.
pub const EFFECT_BLINK: u8 = 5;

/// One running `290` colour effect on a picture.
///
/// The official documentation insists an effect is processed "independently of what the
/// picture command specified", which is why this lives in a list beside the `150` fields
/// instead of mutating them: when the effect ends the picture has to fall back to its own
/// base tone, not to neutral. Keeping the base tone on Picture and only the phase here is
/// the same split, and it is also what makes the boot banner testable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PictureEffect {
    /// Effect type: the high nibble of the head word.
    pub kind: u8,
    /// Frames in one cycle, never zero because the ramp divides by it.
    pub duration: u32,
    /// Per-channel target offset in percent, straight from args 4..6.
    pub target: Tone,
    /// Frames elapsed, i.e. the phase inside the cycle.
    pub age: u32,
}

impl PictureEffect {
    /// Builds an effect at phase zero.
    #[must_use]
    pub fn new(kind: u8, duration: i32, target: Tone) -> Self {
        Self {
            kind,
            duration: duration.max(1) as u32,
            target,
            age: 0,
        }
    }

    /// Builds an effect from the interpreter raw `290` argument list.
    ///
    /// The list is head, duration, picture A, picture B, then the three channel targets, so
    /// the targets are simply args 4..6. The interpreter hands over the whole instruction,
    /// which is why this module never has to look at the opcode encoding itself.
    #[must_use]
    pub fn from_args(kind: u8, duration: i32, args: &[i32]) -> Self {
        let channel = |index: usize| args.get(index).copied().unwrap_or(0);
        Self::new(kind, duration, [channel(4), channel(5), channel(6)])
    }

    /// True when the effect asks for no colour change at all.
    #[must_use]
    pub fn is_neutral(&self) -> bool {
        self.target == [0, 0, 0]
    }

    /// True once a one-shot effect has run its course; type 5 never finishes.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.kind != EFFECT_BLINK && self.age >= self.duration
    }

    /// The channel multiplier at the current phase, scaled by the cycle length.
    ///
    /// Returned as percent-times-duration so the caller divides exactly once and the whole
    /// chain stays in integers, which keeps the result reproducible frame for frame. The
    /// ramp is the one docs/OPCODE_290.md section 5.2 derives from the measurements: the
    /// colour changes with no wait, then walks linearly back to neutral over `duration`.
    #[must_use]
    pub fn percent_times_duration(&self, channel: usize) -> i64 {
        let from = i64::from(SCALE_ONE + self.target[channel]);
        let span = i64::from(self.duration);
        let phase = i64::from(self.age % self.duration);
        from * (span - phase) + i64::from(SCALE_ONE) * phase
    }
}
/// One live picture slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    /// Slot number; negative values are whole-screen layers.
    pub number: i32,
    /// What the picture draws.
    pub kind: PictureKind,
    /// Asset path, already escape-expanded. Empty for text kinds.
    pub file: String,
    /// Text content, already escape-expanded. Empty for image kinds.
    pub text: String,
    /// Left edge, logical pixels.
    pub x: i32,
    /// Top edge, logical pixels.
    pub y: i32,
    /// Uniform scale in percent.
    pub scale: i32,
    /// Opacity, 0..=255.
    pub opacity: i32,
    /// Blend mode, passed to the renderer unchanged.
    pub blend: i32,
    /// Window size for the window kinds.
    pub window: Option<(i32, i32)>,
    /// Anchor code from the head word bits 12..14: 0 top-left, 1 centre.
    pub anchor: u8,
    /// Kind-specific trailing arguments, kept verbatim for the animation layer.
    pub extras: Vec<i32>,
    /// Fade-in length in frames (0 = instant).
    pub fade: u32,
    /// Frames since the picture first appeared, for animation timing.
    pub age: u32,
    /// Base per-channel brightness in percent, from the 150 colour slots.
    pub tone: Tone,
    /// Running 290 colour effects, applied on top of tone.
    pub effects: Vec<PictureEffect>,
}

impl Picture {
    /// A picture at the WOLF defaults: origin at the top-left, 100% scale, opaque.
    #[must_use]
    pub fn new(number: i32) -> Self {
        Self {
            number,
            kind: PictureKind::File,
            file: String::new(),
            text: String::new(),
            x: 0,
            y: 0,
            scale: SCALE_ONE,
            opacity: 255,
            blend: 0,
            window: None,
            anchor: ANCHOR_TOP_LEFT,
            extras: Vec::new(),
            fade: 0,
            age: 0,
            tone: TONE_NEUTRAL,
            effects: Vec::new(),
        }
    }

    /// True for the whole-screen layers (picture numbers below zero).
    #[must_use]
    pub fn is_screen_layer(&self) -> bool {
        self.number < 0
    }

    /// Opacity as 0.0..=1.0, clamped the way the renderer wants it.
    ///
    /// A picture with a fade-in ramp is multiplied by its progress through that ramp, which
    /// is how the boot chain's splashes appear: the retail disclaimer frame is 63/255 of the
    /// source art at age 5 of a 20-frame fade, and 5/20 * 255 = 63.75.
    #[must_use]
    pub fn alpha(&self) -> f32 {
        let opacity = (self.opacity.clamp(0, 255) as f32) / 255.0;
        if self.fade == 0 {
            return opacity;
        }
        let progress = (self.age as f32 / self.fade as f32).min(1.0);
        opacity * progress
    }
    /// The per-channel draw brightness this picture currently asks for, 0..=255.
    ///
    /// Two layers, in this order: the base tone from the 150 colour slots, then every
    /// live 290 effect multiplying it. DxLib expresses the result as SetDrawBright, which
    /// scales the colour channels of whatever is drawn and never the alpha, so alpha()
    /// stays independent of this and the banner still blends at its own opacity.
    #[must_use]
    pub fn tone_byte(&self) -> [u8; 3] {
        let mut out = [0u8; 3];
        for (channel, slot) in out.iter_mut().enumerate() {
            let mut value = i64::from(percent_to_byte(self.tone[channel]));
            for effect in &self.effects {
                let numerator = effect.percent_times_duration(channel);
                let denominator = i64::from(SCALE_ONE) * i64::from(effect.duration);
                value = (value * numerator + denominator / 2) / denominator;
            }
            *slot = value.clamp(0, 255) as u8;
        }
        out
    }

    /// Starts a colour effect, replacing whatever colour effect was already running.
    ///
    /// WOLF keeps one colour state per picture rather than a stack, so a second flash takes
    /// over from the first instead of multiplying with it.
    fn start_effect(&mut self, effect: PictureEffect) {
        self.stop_effects();
        self.effects.push(effect);
    }

    /// Cancels the colour effects, leaving the 150 base tone in charge again.
    fn stop_effects(&mut self) {
        self.effects
            .retain(|effect| !matches!(effect.kind, EFFECT_FLASH | EFFECT_BLINK));
    }
}

/// Converts a percentage tone into the 0..=255 byte the draw call wants.
///
/// Rounds half up and clamps: a tone above 100 percent asks for more than a channel can
/// hold, and DxLib saturates there rather than scaling the whole picture down.
fn percent_to_byte(percent: i32) -> i32 {
    let scaled = i64::from(percent.clamp(0, 200)) * 255;
    ((scaled + 50) / 100).clamp(0, 255) as i32
}
/// A draw request for one picture, already flattened into logical-pixel coordinates.
///
/// The renderer only has to place a textured quad; every WOLF-specific rule (sentinel
/// merging, whole-screen layers, percent scaling) has been applied by Screen::quads.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenQuad {
    /// Picture slot this came from (useful for tracing).
    pub number: i32,
    /// What it draws.
    pub kind: PictureKind,
    /// Asset path for image kinds.
    pub file: String,
    /// Text for text kinds.
    pub text: String,
    /// Source rectangle inside the texture: [x, y, w, h].
    pub src: [u32; 4],
    /// Destination rectangle in logical pixels: [x, y, w, h].
    pub dst: [f32; 4],
    /// Opacity 0.0..=1.0.
    pub alpha: f32,
    /// Per-channel draw brightness, 0..=255, alpha excluded.
    pub tone: [u8; 3],
    /// Blend mode, verbatim.
    pub blend: i32,
}

/// The set of live pictures, keyed by number.
///
/// A BTreeMap is the right container twice over: WOLF composites pictures in ascending
/// number order (so iteration order *is* the draw order, with no sort per frame), and
/// lookups by number are the only access pattern the interpreter has.
#[derive(Clone, Debug, Default)]
pub struct Screen {
    pictures: BTreeMap<i32, Picture>,
}

impl Screen {
    /// An empty screen.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of live pictures.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pictures.len()
    }

    /// True when nothing is drawn.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pictures.is_empty()
    }

    /// Looks up one picture by slot number.
    #[must_use]
    pub fn get(&self, number: i32) -> Option<&Picture> {
        self.pictures.get(&number)
    }

    /// Iterates live pictures in draw order (ascending number).
    pub fn iter(&self) -> impl Iterator<Item = &Picture> {
        self.pictures.values()
    }

    /// Drops every picture. Used when returning to the title or loading a save.
    pub fn clear(&mut self) {
        self.pictures.clear();
    }

    /// Advances the animation clock by one frame.
    pub fn tick(&mut self) {
        for picture in self.pictures.values_mut() {
            picture.age = picture.age.saturating_add(1);
            for effect in &mut picture.effects {
                effect.age = effect.age.saturating_add(1);
            }
            // One-shot effects narrate themselves out of existence; type 5 repeats.
            picture.effects.retain(|effect| !effect.is_finished());
        }
    }

    /// Applies one `150` picture command.
    ///
    /// The four actions are not symmetric: Draw and Set build a fresh slot (so UNCHANGED
    /// fields take their defaults), while Move patches an existing one and keeps every
    /// field the command leaves at UNCHANGED. Erase removes the slot.
    ///
    /// Tolerance, matching the interpreter's house rules: a Move for a slot that does not
    /// exist creates it rather than being dropped, because losing the mutation would make
    /// every later move target nothing and the picture would never appear.
    pub fn apply(&mut self, cmd: &PictureCmd) {
        match cmd.action {
            PictureAction::Erase => {
                self.pictures.remove(&cmd.number);
            }
            PictureAction::Draw | PictureAction::Set => {
                let mut picture = Picture::new(cmd.number);
                picture.kind = PictureKind::from_code(cmd.kind);
                picture.file = cmd.file.clone();
                picture.text = cmd.text.clone();
                picture.anchor = cmd.anchor;
                merge_args(&mut picture, &cmd.args);
                self.pictures.insert(cmd.number, picture);
            }
            PictureAction::Move => {
                let picture = self.pictures.entry(cmd.number).or_insert_with(|| {
                    let mut fresh = Picture::new(cmd.number);
                    fresh.kind = PictureKind::from_code(cmd.kind);
                    fresh
                });
                // A Move that carries a file name is how the script swaps the image in place.
                if !cmd.file.is_empty() {
                    picture.file.clone_from(&cmd.file);
                }
                if !cmd.text.is_empty() {
                    picture.text.clone_from(&cmd.text);
                }
                // The anchor rides on every command, Move included: 56 of the 3,364 Move
                // instructions in this game carry the centre flag.
                picture.anchor = cmd.anchor;
                merge_args(picture, &cmd.args);
            }
        }
    }

    /// Applies one 290 effect command, returning false when the type is not implemented.
    ///
    /// args is the interpreter raw argument list, so args 2 and 3 are the two picture
    /// numbers and args 4..6 the per-channel targets. Only the colour family is handled -
    /// types 0 (flash), 1 (cancel) and 5 (automatic flash) - because docs/OPCODE_290.md
    /// sections 2.1, 2.2 and 2.5 show those are the only types whose three trailing args
    /// are colour percentages. The coordinate and scale families need picture fields this
    /// module does not own yet, so they are reported as unhandled instead of guessed at.
    pub fn effect(&mut self, kind: i32, duration: i32, args: &[i32]) -> bool {
        let Ok(kind) = u8::try_from(kind) else {
            return false;
        };
        if !matches!(kind, EFFECT_FLASH | EFFECT_OFF | EFFECT_BLINK) {
            return false;
        }
        let effect = PictureEffect::from_args(kind, duration, args);
        let targets = [args.get(2).copied(), args.get(3).copied()];
        for number in targets.into_iter().flatten() {
            let Some(picture) = self.pictures.get_mut(&number) else {
                continue;
            };
            // Every stopping spelling in the data means the same thing: back to the base
            // tone the picture command set. A cancel, a zero length and an all-zero target
            // are the three ways WOLF writes that.
            if kind == EFFECT_OFF || duration <= 0 || effect.is_neutral() {
                picture.stop_effects();
            } else {
                picture.start_effect(effect);
            }
        }
        true
    }
    /// Flattens the live pictures into draw requests, in draw order.
    ///
    /// tex_size resolves an asset path to its pixel size; returning None skips the
    /// picture, because a quad of unknown size cannot be placed and inventing one would
    /// draw garbage. Text pictures are emitted with an empty source rectangle so the text
    /// layer can pick them up in the same pass.
    #[must_use]
    pub fn quads<F>(&self, mut tex_size: F) -> Vec<ScreenQuad>
    where
        F: FnMut(&str) -> Option<(u32, u32)>,
    {
        let mut out = Vec::with_capacity(self.pictures.len());
        for picture in self.pictures.values() {
            if picture.opacity <= 0 {
                continue;
            }
            // Whole-screen layers ignore x/y/scale entirely: whichever way a negative-number
            // picture is anchored, it must cover the logical screen.
            if picture.is_screen_layer() {
                let src = tex_size(&picture.file)
                    .map_or([0, 0, SCREEN_W, SCREEN_H], |(w, h)| [0, 0, w, h]);
                out.push(ScreenQuad::from(
                    picture,
                    src,
                    [0.0, 0.0, SCREEN_W as f32, SCREEN_H as f32],
                ));
                continue;
            }
            let (src, w, h) = if picture.kind.is_text() {
                let (w, h) = picture.window.unwrap_or((0, 0));
                ([0, 0, 0, 0], w.max(0) as f32, h.max(0) as f32)
            } else {
                let Some((w, h)) = tex_size(&picture.file) else {
                    continue;
                };
                let scale = picture.scale.max(0) as f32 / SCALE_ONE as f32;
                ([0, 0, w, h], w as f32 * scale, h as f32 * scale)
            };
            let dst = place(picture.anchor, picture.x, picture.y, w, h);
            out.push(ScreenQuad::from(picture, src, dst));
        }
        out
    }
}

impl ScreenQuad {
    /// Builds a quad from a picture plus its finished source/destination rectangles.
    fn from(picture: &Picture, src: [u32; 4], dst: [f32; 4]) -> Self {
        Self {
            number: picture.number,
            kind: picture.kind,
            file: picture.file.clone(),
            text: picture.text.clone(),
            src,
            dst,
            alpha: picture.alpha(),
            tone: picture.tone_byte(),
            blend: picture.blend,
        }
    }
}

/// Turns an anchor code plus a w*h extent into the destination rectangle.
///
/// ANCHOR_CENTRE shifts by half the extent on both axes, which is what the retail title
/// banner needs; every other code keeps the corner behaviour and is unverified.
fn place(anchor: u8, x: i32, y: i32, w: f32, h: f32) -> [f32; 4] {
    let mut left = x as f32;
    let mut top = y as f32;
    if anchor == ANCHOR_CENTRE {
        left -= w / 2.0;
        top -= h / 2.0;
    }
    [left, top, w, h]
}

/// Merges a command's field words into a picture, honouring the UNCHANGED sentinel.
fn merge_args(picture: &mut Picture, args: &[i32]) {
    if let Some(value) = take(args, field::OPACITY) {
        picture.opacity = value.clamp(0, 255);
    }
    if let Some(value) = take(args, field::X) {
        picture.x = value;
    }
    if let Some(value) = take(args, field::Y) {
        picture.y = value;
    }
    if let Some(value) = take(args, field::SCALE) {
        picture.scale = value;
    }
    if let Some(value) = take(args, field::BLEND) {
        picture.blend = value;
    }
    // Window dimensions only mean something for the window kinds, and the short forms do
    // not carry them at all; a 0x0 window would make the text invisible, so require > 0.
    if let (Some(w), Some(h)) = (take(args, field::WINDOW_W), take(args, field::WINDOW_H)) {
        if w > 0 && h > 0 {
            picture.window = Some((w, h));
        }
    }
    if let Some(value) = take(args, field::FADE_IN) {
        picture.fade = value.clamp(0, 600) as u32;
    }
    if args.len() > field::EXTRAS {
        picture.extras = args[field::EXTRAS..].to_vec();
    }
    // The 150 long form also carries the base colour in the three slots the opcode
    // notes number 15..17; PictureCmd::args starts at raw arg 2, so they land at
    // 13..15 here. The short forms simply do not have them, which leaves the
    // default neutral tone in place.
    if let (Some(r), Some(g), Some(b)) = (
        take(args, field::TONE_R),
        take(args, field::TONE_G),
        take(args, field::TONE_B),
    ) {
        picture.tone = [r.clamp(0, 200), g.clamp(0, 200), b.clamp(0, 200)];
    }
}

/// Reads a field, mapping the UNCHANGED sentinel and out-of-range indices to None.
fn take(args: &[i32], index: usize) -> Option<i32> {
    match args.get(index) {
        Some(&value) if value != UNCHANGED => Some(value),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use wolf_vm::PictureCmd;

    /// Builds a command the way the interpreter does: args start at raw arg[2].
    fn cmd(
        number: i32,
        kind: u8,
        action: PictureAction,
        file: &str,
        text: &str,
        args: &[i32],
    ) -> PictureCmd {
        PictureCmd {
            number,
            kind,
            action,
            file: file.to_owned(),
            text: text.to_owned(),
            args: args.to_vec(),
            animate: Vec::new(),
            anchor: ANCHOR_TOP_LEFT,
        }
    }

    #[test]
    fn place_only_shifts_the_centre_anchor() {
        assert_eq!(
            place(ANCHOR_TOP_LEFT, 512, 520, 606.0, 35.0),
            [512.0, 520.0, 606.0, 35.0]
        );
        assert_eq!(
            place(ANCHOR_CENTRE, 512, 520, 606.0, 35.0),
            [209.0, 502.5, 606.0, 35.0]
        );
        // The unverified codes must behave like the corner, never like the centre.
        assert_eq!(place(3, 100, 50, 20.0, 10.0), [100.0, 50.0, 20.0, 10.0]);
    }

    #[test]
    fn a_centred_picture_places_its_middle_at_xy() {
        let mut screen = Screen::new();
        let mut command = cmd(
            12001,
            0,
            PictureAction::Draw,
            "SystemFile/Button01.png",
            "",
            &[4096, 1, 1, 1, 255, 512, 520, 100, 0],
        );
        command.anchor = ANCHOR_CENTRE;
        screen.apply(&command);
        let quads = screen.quads(|_| Some((606, 35)));
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].dst, [209.0, 502.5, 606.0, 35.0]);
    }

    #[test]
    fn a_move_can_switch_the_anchor() {
        let mut screen = Screen::new();
        screen.apply(&cmd(
            7,
            0,
            PictureAction::Draw,
            "a.png",
            "",
            &[0, 1, 1, 1, 255, 0, 0, 100, 0],
        ));
        let mut moved = cmd(
            7,
            0,
            PictureAction::Move,
            "",
            "",
            &[0, 1, 1, 1, 255, 512, 288, 100, 0],
        );
        moved.anchor = ANCHOR_CENTRE;
        screen.apply(&moved);
        let quads = screen.quads(|_| Some((100, 50)));
        assert_eq!(quads[0].dst, [462.0, 263.0, 100.0, 50.0]);
    }

    /// The real title background command, verbatim from event 42 pc20:
    /// raw [0, -2, 30, 1, 1, 1, 255, 0, 0, 100, 0] with file SystemGraphic/TitleGraphic.jpg,
    /// so args (everything after the type word and the number) is [30,1,1,1,255,0,0,100,0].
    fn title_background() -> PictureCmd {
        cmd(
            -2,
            0,
            PictureAction::Draw,
            "SystemGraphic/TitleGraphic.jpg",
            "",
            &[30, 1, 1, 1, 255, 0, 0, 100, 0],
        )
    }

    #[test]
    fn title_background_lands_where_the_measurement_says() {
        let mut screen = Screen::new();
        screen.apply(&title_background());
        let picture = screen.get(-2).expect("picture");
        assert_eq!(picture.kind, PictureKind::File);
        assert_eq!(picture.file, "SystemGraphic/TitleGraphic.jpg");
        assert_eq!(picture.x, 0);
        assert_eq!(picture.y, 0);
        assert_eq!(picture.scale, 100);
        assert_eq!(picture.opacity, 255);
        assert_eq!(picture.blend, 0);
        // The command carries 30 in the fade slot, so the art ramps in over 30 frames
        // instead of popping; the fully faded state is the steady one to assert here.
        assert_eq!(picture.fade, 30);
        assert_eq!(picture.alpha(), 0.0);
        for _ in 0..30 {
            screen.tick();
        }
        let picture = screen.get(-2).expect("picture");
        assert_eq!(picture.alpha(), 1.0);
        assert!(picture.is_screen_layer());
    }

    #[test]
    fn a_screen_layer_fills_the_logical_screen_whatever_the_texture_size_is() {
        let mut screen = Screen::new();
        screen.apply(&title_background());
        let quads = screen.quads(|_| Some((1024, 576)));
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].src, [0, 0, 1024, 576]);
        assert_eq!(quads[0].dst, [0.0, 0.0, 1024.0, 576.0]);

        // Same picture, undersized texture: still covers the screen (scaled up).
        let quads = screen.quads(|_| Some((512, 288)));
        assert_eq!(quads[0].dst, [0.0, 0.0, 1024.0, 576.0]);

        // Unknown texture: fall back to the logical screen rather than dropping the layer.
        let quads = screen.quads(|_| None);
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].src, [0, 0, 1024, 576]);
    }

    #[test]
    fn move_only_patches_fields_that_are_not_the_unchanged_sentinel() {
        let mut screen = Screen::new();
        screen.apply(&title_background());
        // Move to (100, 200) at 50% and leave opacity and blend alone.
        screen.apply(&cmd(
            -2,
            0,
            PictureAction::Move,
            "",
            "",
            &[
                UNCHANGED, UNCHANGED, UNCHANGED, UNCHANGED, UNCHANGED, 100, 200, 50, UNCHANGED,
            ],
        ));
        let picture = screen.get(-2).expect("picture");
        assert_eq!(picture.x, 100);
        assert_eq!(picture.y, 200);
        assert_eq!(picture.scale, 50);
        assert_eq!(picture.opacity, 255);
        assert_eq!(picture.blend, 0);
        assert_eq!(picture.file, "SystemGraphic/TitleGraphic.jpg");
    }

    #[test]
    fn move_creates_a_missing_slot_instead_of_dropping_the_mutation() {
        let mut screen = Screen::new();
        screen.apply(&cmd(
            7,
            0,
            PictureAction::Move,
            "Kagura/bg.png",
            "",
            &[
                UNCHANGED, UNCHANGED, UNCHANGED, UNCHANGED, UNCHANGED, 10, 20, 100, 0,
            ],
        ));
        let picture = screen.get(7).expect("created by move");
        assert_eq!(picture.x, 10);
        assert_eq!(picture.y, 20);
        assert_eq!(picture.file, "Kagura/bg.png");
    }

    #[test]
    fn erase_removes_the_slot_and_is_idempotent() {
        let mut screen = Screen::new();
        screen.apply(&title_background());
        assert_eq!(screen.len(), 1);
        screen.apply(&cmd(-2, 0, PictureAction::Erase, "", "", &[]));
        assert!(screen.is_empty());
        screen.apply(&cmd(-2, 0, PictureAction::Erase, "", "", &[]));
        assert!(screen.is_empty());
    }

    #[test]
    fn opacity_is_clamped_and_transparent_pictures_are_not_drawn() {
        let mut screen = Screen::new();
        screen.apply(&cmd(
            1,
            0,
            PictureAction::Draw,
            "a.png",
            "",
            &[0, 0, 0, 0, 999, 0, 0, 100, 0],
        ));
        assert_eq!(screen.get(1).expect("picture").opacity, 255);
        screen.apply(&cmd(
            2,
            0,
            PictureAction::Draw,
            "b.png",
            "",
            &[0, 0, 0, 0, -5, 0, 0, 100, 0],
        ));
        assert_eq!(screen.get(2).expect("picture").opacity, 0);
        let quads = screen.quads(|_| Some((16, 16)));
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].number, 1);
    }

    #[test]
    fn a_text_picture_carries_no_source_rect_and_the_window_rect_as_destination() {
        let mut screen = Screen::new();
        // The real notice window: 300x200 at (855, 200), text kind 2.
        screen.apply(&cmd(
            12500,
            2,
            PictureAction::Draw,
            "",
            "hello",
            &[0, 300, 200, 1, 255, 855, 200, 100, 0],
        ));
        let quads = screen.quads(|_| Some((1024, 576)));
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].kind, PictureKind::Text);
        assert_eq!(quads[0].src, [0, 0, 0, 0]);
        assert_eq!(quads[0].dst, [855.0, 200.0, 300.0, 200.0]);
        assert_eq!(quads[0].text, "hello");
    }

    #[test]
    fn an_image_picture_with_an_unknown_texture_is_skipped() {
        let mut screen = Screen::new();
        screen.apply(&cmd(
            3,
            0,
            PictureAction::Draw,
            "missing.png",
            "",
            &[0, 0, 0, 0, 255, 5, 6, 100, 0],
        ));
        assert!(screen.quads(|_| None).is_empty());
        let quads = screen.quads(|_| Some((40, 20)));
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].dst, [5.0, 6.0, 40.0, 20.0]);
    }

    #[test]
    fn images_are_scaled_by_percent() {
        let mut screen = Screen::new();
        screen.apply(&cmd(
            3,
            0,
            PictureAction::Draw,
            "a.png",
            "",
            &[0, 0, 0, 0, 255, 5, 6, 250, 0],
        ));
        let quads = screen.quads(|_| Some((40, 20)));
        assert_eq!(quads[0].dst, [5.0, 6.0, 100.0, 50.0]);
    }

    #[test]
    fn iteration_is_in_ascending_number_order_because_that_is_the_draw_order() {
        let mut screen = Screen::new();
        for number in [5, -2, 12, 0] {
            screen.apply(&cmd(
                number,
                0,
                PictureAction::Draw,
                "a.png",
                "",
                &[0, 0, 0, 0, 255, 0, 0, 100, 0],
            ));
        }
        let order: Vec<i32> = screen.iter().map(|p| p.number).collect();
        assert_eq!(order, vec![-2, 0, 5, 12]);
        let quad_order: Vec<i32> = screen
            .quads(|_| Some((1, 1)))
            .into_iter()
            .map(|q| q.number)
            .collect();
        assert_eq!(quad_order, vec![-2, 0, 5, 12]);
    }

    #[test]
    fn tick_advances_the_animation_clock_and_clear_empties_the_screen() {
        let mut screen = Screen::new();
        screen.apply(&title_background());
        screen.tick();
        screen.tick();
        assert_eq!(screen.get(-2).expect("picture").age, 2);
        screen.clear();
        assert!(screen.is_empty());
    }

    #[test]
    fn draw_replaces_a_previous_picture_rather_than_patching_it() {
        let mut screen = Screen::new();
        screen.apply(&title_background());
        screen.apply(&cmd(
            0,
            0,
            PictureAction::Draw,
            "b.png",
            "",
            &[
                UNCHANGED, UNCHANGED, UNCHANGED, UNCHANGED, UNCHANGED, UNCHANGED, UNCHANGED,
                UNCHANGED, UNCHANGED,
            ],
        ));
        // Draw with everything unchanged means "back to the defaults", not "keep the old".
        let picture = screen.get(0).expect("picture");
        assert_eq!(picture.scale, SCALE_ONE);
        assert_eq!(picture.opacity, 255);
        assert_eq!(picture.file, "b.png");
    }
}

#[cfg(test)]
mod tone_tests {
    use super::*;

    /// Pins the ramp that has to reproduce the retail banner tone of 222/255.
    ///
    /// TitleMap event 2 runs 290 with head 80, duration 30 and targets (-50, -50, -50):
    /// type 5, a 30 frame cycle, target 50 percent. The ramp therefore visits
    /// 50 + 50 * phase/30 percent, which converts to 128 + 127 * phase/30 bytes.
    /// A 30 frame cycle can only land on 128, 132, ... 255; 222 itself is unreachable
    /// because it needs phase 22.2. Phase 22 gives 221, the closest the cycle ever comes,
    /// so the assertion allows two bytes of slack rather than pretending the model has
    /// more resolution than the cycle length gives it. The retail screenshot is one
    /// arbitrary frame of that cycle and measures 222, between our 221 and 225.
    #[test]
    fn blink_ramp_reaches_the_retail_banner_tone() {
        let mut picture = Picture::new(12001);
        picture
            .effects
            .push(PictureEffect::new(EFFECT_BLINK, 30, [-50, -50, -50]));

        let mut closest: i32 = 0;
        for age in 0..30 {
            picture.effects[0].age = age;
            let byte = i32::from(picture.tone_byte()[0]);
            if (byte - 222).abs() < (closest - 222).abs() {
                closest = byte;
            }
        }
        assert!(
            (closest - 222).abs() <= 2,
            "closest byte to the retail 222 was {closest}"
        );
        picture.effects[0].age = 22;
        assert_eq!(picture.tone_byte(), [221, 221, 221]);
    }

    /// An effect multiplies the command base tone instead of replacing it.
    #[test]
    fn effect_multiplies_the_command_tone() {
        let mut picture = Picture::new(1);
        picture.tone = [100, 50, 0];
        assert_eq!(picture.tone_byte(), [255, 128, 0]);
        picture
            .effects
            .push(PictureEffect::new(EFFECT_FLASH, 25, [-50, 0, 0]));
        assert_eq!(picture.tone_byte(), [128, 128, 0]);
        assert_eq!(picture.alpha(), 1.0);
    }

    /// A finished one-shot drops out of the list, so the base tone takes over again.
    #[test]
    fn finished_flash_falls_back_to_the_base_tone() {
        let mut screen = Screen::new();
        let mut picture = Picture::new(7);
        picture.tone = [50, 50, 50];
        picture
            .effects
            .push(PictureEffect::new(EFFECT_FLASH, 2, [-50, -50, -50]));
        screen.pictures.insert(7, picture);
        assert_eq!(screen.get(7).unwrap().tone_byte(), [64, 64, 64]);
        screen.tick();
        screen.tick();
        assert!(screen.get(7).unwrap().effects.is_empty());
        assert_eq!(screen.get(7).unwrap().tone_byte(), [128, 128, 128]);
    }
}
