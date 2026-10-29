//! The 101 Message window: where the engine draws its text, and how much of it.
//!
//! WOLF splits the message window in two, and this module is deliberately only the
//! second half:
//!
//! * the FRAME (the nine-slice skin, the face chip) is drawn by common event 63, which
//!   the engine starts for the whole run and which paints ordinary 150 pictures
//!   (picture 19500 plus the face slots) whenever Sys12 says a message is up. Nothing
//!   here draws a frame, and nothing should: the author owns that geometry.
//! * the TEXT is drawn by the engine itself, and its geometry comes from the system
//!   variables that the same event initialises before jumping back to its caller.
//!
//! Sources: docs/OPCODE_102.md section 4.2 (the Sys1/Sys2 anchoring law),
//! work/full/BasicData/SysDatabase.dat type 6 (the 150 system variable names this
//! module reads), and work/yado.txt (the official YADOT-WOLF manual) for the typewriter
//! and cursor variables.
//!
//! The laws, with their evidence status:
//!
//! * VERIFIED text origin = (Sys1, Sys2) in absolute screen pixels; common event 63
//!   writes 2/50 of the screen width and 27/32 of the screen height into them.
//! * VERIFIED font size = Sys8 (basic font size), the same variable the choice window
//!   and picture text use.
//! * VERIFIED typewriter speed = Sys9 (message speed), in characters per second.
//! * UNVERIFIED Sys10 (message display weight) as frames of pre-roll before typing
//!   starts: the manual calls it a display weight in frames, and only 7 writes of it
//!   exist in the whole game, so the exact rounding is a retail-capture question.
//! * UNVERIFIED the pause-cursor offsets (Sys5/Sys6): the manual documents them as
//!   positions added to Sys1/Sys2 with -1 meaning "let the engine place it", and the
//!   game writes them only 3 and 2 times.
//! * UNVERIFIED the wrap width: this module wraps at the live message-window picture
//!   (19500) when common event 63 has painted it, and at the screen edge otherwise.
//! * UNVERIFIED the line advance: one font size times 30/22, borrowed from the choice
//!   panel, which is the only line pitch this project has measured so far.

use crate::choice;

/// Frames per second the engine counts the typewriter speed in.
pub const FPS: f32 = 60.0;

/// Font size used when Sys8 holds nothing usable.
///
/// Sys8 is written by common event 48 (the boot initialiser) and read by the message
/// window, the choice window and picture text alike; a zero or negative value means the
/// initialiser did not run, so the choice panel size is the honest fallback.
pub const DEFAULT_FONT_PX: f32 = choice::TEXT_PX;

/// Line advance as a multiple of the font size.
///
/// choice::LINE_HEIGHT / choice::TEXT_PX = 30/22. Sharing the ratio keeps a message and
/// a choice panel drawn at the same font size on the same baseline grid.
pub const LINE_SPACING: f32 = choice::LINE_HEIGHT / choice::TEXT_PX;

/// Picture slot common event 63 keeps the message-window frame in (its self[20]).
///
/// The event draws the nine-slice skin there and erases it when Sys12 drops, so when the
/// slot is live its width is the box the text must wrap inside.
pub const FRAME_PICTURE: i32 = 19500;

/// Gap between the last glyph of the text and an auto-placed pause cursor, in pixels.
pub const CURSOR_GAP: f32 = 6.0;

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
    /// Wrap width in pixels, or None to wrap at the screen edge.
    pub width: Option<f32>,
    /// (Sys5, Sys6) - the pause cursor position relative to the origin, when both >= 0.
    pub cursor_offset: Option<(f32, f32)>,
    /// Screen width in pixels, used when width is None.
    pub screen_w: f32,
}

impl Vars {
    /// The message-window variables a fresh engine has before any script runs.
    ///
    /// Sys1/Sys2 at the documented defaults (2/50 and 27/32 of the screen), Sys8 at the
    /// choice panel size, an instant typewriter and no cursor offset.
    #[must_use]
    pub fn defaults(screen_w: f32, screen_h: f32) -> Self {
        Self {
            x: (screen_w * 2.0 / 50.0).round(),
            y: (screen_h * 27.0 / 32.0).round(),
            font_px: DEFAULT_FONT_PX,
            chars_per_second: 0,
            preroll: 0,
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
/// None means "all of them": a typewriter speed of zero or less shows the message at
/// once, which is the state the engine ships in until common event 48 or a script sets
/// Sys9.
#[must_use]
pub fn revealed(frames: u32, chars_per_second: i32, preroll: i32) -> Option<usize> {
    if chars_per_second <= 0 {
        return None;
    }
    let preroll = preroll.max(0) as u32;
    if frames < preroll {
        return Some(0);
    }
    let elapsed = u64::from(frames - preroll);
    let whole = elapsed * chars_per_second as u64 / FPS as u64;
    usize::try_from(whole).ok()
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
    let line_height = (font_px * LINE_SPACING).round();
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
            x: 32.0,
            y: 432.0,
            font_px: 22.0,
            chars_per_second: 0,
            preroll: 0,
            width: Some(220.0),
            cursor_offset: None,
            screen_w: 1024.0,
        }
    }

    #[test]
    fn a_full_width_message_wraps_at_the_font_size() {
        let rows = wrap("一二三四五六七八九十一二三", 220.0, 22.0);
        assert_eq!(rows, vec!["一二三四五六七八九十".to_owned(), "一二三".to_owned()]);
    }

    #[test]
    fn an_explicit_line_break_starts_a_new_row() {
        let rows = wrap("上\n下", 220.0, 22.0);
        assert_eq!(rows, vec!["上".to_owned(), "下".to_owned()]);
    }

    #[test]
    fn rows_advance_by_one_line_pitch() {
        let mut vars = vars();
        vars.width = Some(22.0);
        let layout = layout("甲乙", &vars, 0);
        assert_eq!(layout.rows.len(), 2);
        assert_eq!(layout.rows[0].rect, [32.0, 432.0, 22.0, 30.0]);
        assert_eq!(layout.rows[1].rect[1], 462.0);
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
        vars.width = Some(22.0);
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
        assert_eq!(layout.cursor, Some([32.0 + 44.0 + CURSOR_GAP, 432.0]));
    }

    #[test]
    fn an_explicit_cursor_offset_wins_over_the_auto_placement() {
        let mut vars = vars();
        vars.cursor_offset = Some((16.0, 6.0));
        let layout = layout("甲", &vars, 0);
        assert_eq!(layout.cursor, Some([48.0, 438.0]));
    }

    #[test]
    fn wolf_markup_never_reaches_the_text_pass() {
        let layout = layout("\\E\\c[6]你好", &vars(), 0);
        assert_eq!(layout.rows[0].text, "你好");
        assert_eq!(layout.total, 2);
    }

    #[test]
    fn a_missing_font_size_falls_back_to_the_choice_panel_size() {
        let mut vars = vars();
        vars.font_px = 0.0;
        let layout = layout("甲", &vars, 0);
        assert_eq!(layout.font_px, DEFAULT_FONT_PX);
    }
}
