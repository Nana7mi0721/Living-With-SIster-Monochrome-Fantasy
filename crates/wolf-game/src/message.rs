//! The 101 Message window: where the engine draws its text, and how much of it.
//!
//! WOLF splits the message window in two, and this module is deliberately only the
//! second half:
//!
//! * the FRAME (the nine-slice skin, the face chip) is drawn by common event 63, which
//!   the engine starts for the whole run and which paints ordinary 150 pictures into
//!   picture slots 19500 and up whenever Sys12 says a message is up. Nothing here draws
//!   a frame, and nothing should: the author owns that geometry.
//! * the TEXT is drawn by the engine itself, and its geometry comes from the system
//!   variables that the same event initialises before jumping back to its caller.
//!
//! Every law below is measured; docs/OPCODE_101.md carries the bytecode evidence.
//!
//! * [已实测] The coordinate scale is the screen divided by the engine virtual screen:
//!   rateX = width / 32, rateY = height / 24 (common event 48 divides Sys116/Sys117 by
//!   exactly those numbers; common event 63 multiplies back and stores 1024 and 576).
//! * [已实测] Text origin = Sys1/Sys2 = (2*rateX, 17*rateY + rateY/10) = (64, 410) on a
//!   1024x576 screen. The nudge is rateY/10 (24 / 10 = 2). An earlier note here read 412 and
//!   blamed a /5 nudge, but that capture predated the 121 operator fix (0x3000 was remainder).
//! * [已实测] Font size = Sys8, and this game sets 20.
//! * [已实测] Line pitch = Sys8 + Game.dat text_line_spacing (6) = 26.
//! * [已实测] Typewriter speed = Sys9 characters per second: 500 at boot (common event
//!   48), 120 after common event 269 slows it down. Sys10 holds 9 idle, 1 while showing.
//! * [未实测] The per-frame reveal uses a fractional accumulator of Sys9/60 per frame;
//!   the manual only says X characters per second, and no frame-exact sample exists.
//! * [已实测] The pause cursor is drawn by the ENGINE at (Sys1 + Sys5, Sys2 + Sys6) =
//!   (576, 554), which is inside the frame, bottom right of the text origin.
//! * [未实测] The right padding of the wrap box: wrapping uses the frame right edge
//!   (the screen edge, 1024) minus the text origin, which is the documented hard cap.
//! * [已实测] A bare LF byte is the line break; 3,915 of them exist in real messages.

use crate::choice;

/// Frames per second the engine counts the typewriter speed in.
pub const FPS: f32 = 60.0;

/// Sys8 as common event 48 leaves it in this game.
pub const DEFAULT_FONT_PX: f32 = 20.0;

/// Game.dat text_line_spacing, added to Sys8 to get the line pitch.
pub const LINE_GAP: f32 = 6.0;

/// Sys9 as common event 48 leaves it: characters per second.
pub const DEFAULT_CHARS_PER_SECOND: i32 = 500;

/// Sys10 as common event 63 leaves it while a message is up.
pub const DEFAULT_PREROLL: i32 = 9;

/// The virtual screen the coordinate scale divides by: 32 steps across, 24 down.
pub const SCREEN_STEPS: (f32, f32) = (32.0, 24.0);

/// Picture slot common event 63 keeps the message-window frame in (its self[20]).
///
/// This is a SLOT NUMBER, not a file name: Picture/19500 does not exist, and the frame
/// is an ordinary 150 window-type draw with the skin SysDatabase answers for the
/// [250, 18, 0, 24] read. When the slot is live its rectangle is the box the text must
/// wrap inside, which is why this module still reads it.
pub const FRAME_PICTURE: i32 = 19500;

/// Gap between the last glyph of the text and an auto-placed pause cursor, in pixels.
pub const CURSOR_GAP: f32 = 6.0;

/// The engine pause cursor: `SystemFile/Text_Pause.png`.
///
/// [已实测] The file is 17x90 and its opaque pixels fall into six 17x15 bands, each a
/// 16x10 downward triangle whose top edge sits at y 3, 2, 1, 0, 1, 3 inside its band: a
/// six-frame bob, not the three 17x30 frames an earlier note guessed. No script draws
/// it - common event 63 only draws slots 19500..19503 - so the engine owns it.
pub const CURSOR_FILE: &str = "SystemFile/Text_Pause.png";
/// Width of one cursor frame, in pixels.
pub const CURSOR_FRAME_W: u32 = 17;
/// Height of one cursor frame, in pixels.
pub const CURSOR_FRAME_H: u32 = 15;
/// Frames in the cursor bob animation.
pub const CURSOR_FRAMES: u32 = 6;
/// Frames of the message clock per cursor animation step (未验证 rhythm).
pub const CURSOR_STEP_FRAMES: u32 = 8;
/// Trace id for the cursor quad; never a picture slot, which are all >= -100.
pub const CURSOR_SLOT: i32 = -100_000;

/// Which cursor frame a message clock lands on.
#[must_use]
pub fn cursor_frame(frames: u32) -> u32 {
    (frames / CURSOR_STEP_FRAMES) % CURSOR_FRAMES
}

/// The coordinate scale (rateX, rateY) for a screen size.
#[must_use]
pub fn coordinate_scale(screen_w: f32, screen_h: f32) -> (f32, f32) {
    (
        (screen_w / SCREEN_STEPS.0).max(1.0),
        (screen_h / SCREEN_STEPS.1).max(1.0),
    )
}

/// The text origin a boot-time initialiser leaves in Sys1/Sys2.
///
/// Sys1 = 2*rateX, then Sys2 = 17*rateY followed by a Ver3.00 nudge of the same rate. The
/// nudge is rateY/10: with rateY = 24 it is 2 px, and the running game now reads Sys2 = 410
/// (the YADOT-WOLF transcript of common event 63 publishes the identical `Sys2 += CSelf89 / 10`
/// line). Both terms are rounded, so the 1024x576 origin is (64, 410).
#[must_use]
pub fn default_origin(screen_w: f32, screen_h: f32) -> (f32, f32) {
    let (rate_x, rate_y) = coordinate_scale(screen_w, screen_h);
    (
        (2.0 * rate_x).round(),
        // Integer division, like the bytecode operator: 24 / 10 is 2, not 2.4 rounded up.
        (17.0 * rate_y + (rate_y / 10.0).trunc()).round(),
    )
}

/// Line advance in pixels for a font size: Sys8 + Game.dat text_line_spacing.
#[must_use]
pub fn line_height(font_px: f32) -> f32 {
    font_px + LINE_GAP
}

/// The system variables the message window reads.
///
/// Filled from Game by the caller; kept as plain numbers so this module stays
/// independent of the VM and can be tested without one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vars {
    /// Sys1 - text origin X in absolute screen pixels.
    pub x: f32,
    /// Sys2 - text origin Y in absolute screen pixels.
    pub y: f32,
    /// Sys8 - font size in pixels; non-positive falls back to DEFAULT_FONT_PX.
    pub font_px: f32,
    /// Sys9 - typewriter speed in characters per second; zero or less shows it all.
    pub chars_per_second: i32,
    /// Sys10 - frames of pre-roll before typing starts.
    pub preroll: i32,
    /// Wrap width in pixels, or None to wrap at the frame right edge.
    pub width: Option<f32>,
    /// (Sys5, Sys6) - the pause cursor position relative to the origin, when both >= 0.
    pub cursor_offset: Option<(f32, f32)>,
    /// Screen width in pixels, used when width is None.
    pub screen_w: f32,
}

impl Vars {
    /// The message-window variables a boot-time initialiser leaves behind.
    #[must_use]
    pub fn defaults(screen_w: f32, screen_h: f32) -> Self {
        let (x, y) = default_origin(screen_w, screen_h);
        Self {
            x,
            y,
            font_px: DEFAULT_FONT_PX,
            chars_per_second: DEFAULT_CHARS_PER_SECOND,
            preroll: DEFAULT_PREROLL,
            width: None,
            cursor_offset: None,
            screen_w,
        }
    }

    /// The wrap width actually used, in pixels.
    #[must_use]
    pub fn wrap_width(&self) -> f32 {
        self.width.unwrap_or(self.screen_w - self.x)
    }

    /// The font size actually used, in pixels.
    #[must_use]
    pub fn font_px(&self) -> f32 {
        if self.font_px > 0.0 {
            self.font_px
        } else {
            DEFAULT_FONT_PX
        }
    }
}

/// One line of message text and where it goes.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The characters this row shows, already stripped of WOLF markup.
    pub text: String,
    /// Destination rectangle, [x, y, w, h] in logical pixels.
    pub rect: [f32; 4],
}

/// What the text pass should draw for the current message.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Visible rows, top to bottom. A partially typed row is already truncated.
    pub rows: Vec<Row>,
    /// Characters revealed so far, counting line breaks.
    pub typed: usize,
    /// Characters in the whole message, counting line breaks.
    pub total: usize,
    /// The message is still typing.
    pub typing: bool,
    /// Where to draw the pause cursor, when the message is complete.
    pub cursor: Option<[f32; 2]>,
    /// Font size the rows are drawn at.
    pub font_px: f32,
    /// Line advance in pixels.
    pub line_height: f32,
}

/// How many characters of a message are visible at a given frame.
///
/// The engine accumulates Sys9/60 characters per frame and draws the integer part, so
/// the count after N frames is floor(N * Sys9 / 60). None means "all of them": a speed of
/// zero or less shows the message at once.
#[must_use]
pub fn revealed(frames: u32, chars_per_second: i32, preroll: i32) -> Option<usize> {
    if chars_per_second <= 0 {
        return None;
    }
    let preroll = preroll.max(0) as u32;
    if frames < preroll {
        return Some(0);
    }
    let elapsed = f64::from(frames - preroll);
    let whole = (elapsed * f64::from(chars_per_second) / f64::from(FPS)).floor();
    usize::try_from(whole as u64).ok()
}

/// Wrap text to width pixels at font_px, honouring explicit line breaks.
///
/// WOLF message markup uses a real newline for a forced break, and its text is
/// overwhelmingly CJK, which has no spaces to break on: this wraps between any two
/// characters. A space that lands on the break is dropped, so an ASCII sentence does not
/// start its next line with one.
#[must_use]
pub fn wrap(text: &str, width: f32, font_px: f32) -> Vec<String> {
    let width = width.max(font_px);
    let mut rows: Vec<String> = Vec::new();
    for paragraph in text.split('\n') {
        let mut row = String::new();
        let mut used = 0.0_f32;
        for ch in paragraph.chars() {
            let advance = choice::advance_at(ch, font_px);
            if used + advance > width && !row.is_empty() {
                rows.push(std::mem::take(&mut row));
                used = 0.0;
                if ch == ' ' {
                    continue;
                }
            }
            row.push(ch);
            used += advance;
        }
        rows.push(row);
    }
    rows
}

/// Lay out the current message for the text pass.
///
/// text is the raw message as the script produced it; WOLF markup (colour, outline,
/// face prefix, ruby) is stripped here, because the renderer must not be handed escapes.
#[must_use]
pub fn layout(text: &str, vars: &Vars, frames: u32) -> Layout {
    let font_px = vars.font_px();
    let line_height = line_height(font_px);
    let stripped = wolf_format::messagetext::strip(text);
    let rows = wrap(&stripped, vars.wrap_width(), font_px);
    let total: usize = rows.iter().map(|row| row.chars().count()).sum::<usize>() + rows.len() - 1;
    let typed = revealed(frames, vars.chars_per_second, vars.preroll).map_or(total, |n| n.min(total));
    let mut budget = typed;
    let mut visible: Vec<Row> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            if budget == 0 {
                break;
            }
            budget -= 1;
        }
        let take = row.chars().count().min(budget);
        let shown: String = row.chars().take(take).collect();
        budget -= take;
        visible.push(Row {
            text: shown.clone(),
            rect: [
                vars.x,
                vars.y + index as f32 * line_height,
                vars.wrap_width(),
                line_height,
            ],
        });
        if take < row.chars().count() {
            break;
        }
    }
    let typing = typed < total;
    let cursor = if typing {
        None
    } else {
        Some(match vars.cursor_offset {
            Some((dx, dy)) => [vars.x + dx, vars.y + dy],
            None => match visible.last() {
                Some(row) => [
                    vars.x + choice::text_width_at(&row.text, font_px) + CURSOR_GAP,
                    row.rect[1],
                ],
                None => [vars.x, vars.y],
            },
        })
    };
    Layout {
        rows: visible,
        typed,
        total,
        typing,
        cursor,
        font_px,
        line_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vars {
        Vars {
            x: 64.0,
            y: 410.0,
            font_px: DEFAULT_FONT_PX,
            chars_per_second: 0,
            preroll: 0,
            width: Some(200.0),
            cursor_offset: None,
            screen_w: 1024.0,
        }
    }

    #[test]
    fn the_coordinate_scale_is_the_screen_divided_by_32_and_24() {
        assert_eq!(coordinate_scale(1024.0, 576.0), (32.0, 24.0));
        assert_eq!(coordinate_scale(1280.0, 720.0), (40.0, 30.0));
    }

    #[test]
    fn the_text_origin_is_two_steps_in_and_seventeen_down() {
        assert_eq!(default_origin(1024.0, 576.0), (64.0, 410.0));
        // 1280x720: 2*40 = 80, 17*30 + 30/10 = 513.
        assert_eq!(default_origin(1280.0, 720.0), (80.0, 513.0));
        let vars = Vars::defaults(1024.0, 576.0);
        assert_eq!((vars.x, vars.y), (64.0, 410.0));
        assert_eq!(vars.font_px(), 20.0);
        assert_eq!(vars.chars_per_second, 500);
        assert_eq!(vars.wrap_width(), 960.0);
    }

    #[test]
    fn the_line_pitch_is_the_font_size_plus_the_game_dat_spacing() {
        assert_eq!(line_height(20.0), 26.0);
        assert_eq!(line_height(14.0), 20.0);
    }

    #[test]
    fn a_full_width_message_wraps_at_the_font_size() {
        let rows = wrap("一二三四五六七八九十一二三", 200.0, 20.0);
        assert_eq!(rows, vec!["一二三四五六七八九十".to_owned(), "一二三".to_owned()]);
    }

    #[test]
    fn an_explicit_line_break_starts_a_new_row() {
        let rows = wrap("上\n下", 200.0, 20.0);
        assert_eq!(rows, vec!["上".to_owned(), "下".to_owned()]);
    }

    #[test]
    fn rows_advance_by_one_line_pitch() {
        let mut vars = vars();
        vars.width = Some(20.0);
        let layout = layout("甲乙", &vars, 0);
        assert_eq!(layout.rows.len(), 2);
        assert_eq!(layout.rows[0].rect, [64.0, 410.0, 20.0, 26.0]);
        assert_eq!(layout.rows[1].rect[1], 436.0);
    }

    #[test]
    fn a_zero_speed_typewriter_shows_the_whole_message() {
        assert_eq!(revealed(10, 0, 0), None);
        let layout = layout("你好", &vars(), 0);
        assert_eq!(layout.typed, 2);
        assert!(!layout.typing);
        assert_eq!(layout.rows.len(), 1);
        assert_eq!(layout.rows[0].text, "你好");
    }

    #[test]
    fn a_speed_is_characters_per_second_not_per_frame() {
        assert_eq!(revealed(60, 30, 0), Some(30));
        assert_eq!(revealed(1, 30, 0), Some(0));
        assert_eq!(revealed(59, 30, 0), Some(29));
        // The retail speed of 500 characters per second emits 8 per frame.
        assert_eq!(revealed(1, 500, 0), Some(8));
        assert_eq!(revealed(3, 500, 0), Some(25));
    }

    #[test]
    fn the_preroll_delays_the_first_character() {
        assert_eq!(revealed(3, 60, 5), Some(0));
        assert_eq!(revealed(5, 60, 5), Some(0));
        assert_eq!(revealed(7, 60, 5), Some(2));
    }

    #[test]
    fn a_typing_message_reveals_one_row_at_a_time_and_has_no_cursor() {
        let mut vars = vars();
        vars.chars_per_second = 60;
        vars.width = Some(20.0);
        let layout = layout("甲乙丙", &vars, 1);
        assert_eq!(layout.typed, 1);
        assert!(layout.typing);
        assert_eq!(layout.rows.len(), 1);
        assert_eq!(layout.rows[0].text, "甲");
        assert_eq!(layout.cursor, None);
    }

    #[test]
    fn the_pause_cursor_sits_after_the_last_glyph_when_the_engine_places_it() {
        let layout = layout("甲乙", &vars(), 0);
        assert_eq!(layout.cursor, Some([64.0 + 40.0 + CURSOR_GAP, 410.0]));
    }

    #[test]
    fn an_explicit_cursor_offset_wins_over_the_auto_placement() {
        let mut vars = vars();
        vars.cursor_offset = Some((16.0, 6.0));
        let layout = layout("甲", &vars, 0);
        assert_eq!(layout.cursor, Some([80.0, 416.0]));
    }

    #[test]
    fn the_pause_cursor_walks_six_frames_and_loops() {
        let seen: Vec<u32> = (0..48).map(|f| cursor_frame(f * CURSOR_STEP_FRAMES)).collect();
        assert_eq!(seen[..7], [0, 1, 2, 3, 4, 5, 0]);
        assert!(seen.iter().all(|f| *f < CURSOR_FRAMES));
        // Within one step the frame is steady, so a 60 fps message does not strobe.
        assert_eq!(cursor_frame(0), cursor_frame(CURSOR_STEP_FRAMES - 1));
    }

    #[test]
    fn the_retail_cursor_offset_lands_inside_the_frame() {
        // Sys5 = 512, Sys6 = 144 is what this game writes: 64+512, 410+144.
        let mut vars = vars();
        vars.cursor_offset = Some((512.0, 144.0));
        let layout = layout("甲", &vars, 0);
        assert_eq!(layout.cursor, Some([576.0, 554.0]));
    }

    #[test]
    fn wolf_markup_never_reaches_the_text_pass() {
        let layout = layout("\\E\\c[6]你好", &vars(), 0);
        assert_eq!(layout.rows[0].text, "你好");
        assert_eq!(layout.total, 2);
    }

    #[test]
    fn a_missing_font_size_falls_back_to_the_measured_font_size() {
        let mut vars = vars();
        vars.font_px = 0.0;
        let layout = layout("甲", &vars, 0);
        assert_eq!(layout.font_px, DEFAULT_FONT_PX);
    }
}
