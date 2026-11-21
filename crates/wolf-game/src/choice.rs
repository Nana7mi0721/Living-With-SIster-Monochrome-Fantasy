//! The runtime choice panel: WOLF draws opcode `102`'s options itself.
//!
//! A WOLF script never draws its own menu frame for a choice prompt. `102` hands the
//! option strings to the engine, and the engine moves the cursor, paints the window
//! skin and returns a 0-based index. Keeping that split is what makes a script written
//! for the engine work here unchanged: the measured title event draws only its
//! background (one `150` command) and then parks on a `102` with three options, so
//! without a runtime panel the port shows a title screen with no menu at all.
//!
//! Geometry is inferred, not measured: the skin comes from
//! `SystemFile/WindowBase.png`, the grey panel the game's SysDatabase
//! "ウィンドウ画像 / ×選択肢ウィンドウ画像" row names (see [`crate::window`]), and the panel
//! is centred on the 1024x576 screen. The original game's exact placement is still
//! unknown (open item H-MENU-1 in docs/UNVERIFIED.md), so every number here is a named
//! constant a future measurement can replace in one place.

use crate::assets::Assets;
use crate::atlas::{Atlas, SpriteQuad, TextQuad};
use crate::message::{self, Colours, WhiteColours};
use crate::picture::{SCREEN_H, SCREEN_W};
use crate::window::{self, WindowPiece};

/// Distance in pixels between two option baselines.
pub const LINE_HEIGHT: f32 = 30.0;
/// Padding between the option text and the window frame.
pub const PADDING: f32 = 14.0;
/// Fallback pixel size for option text, used by tools and tests that have no VM.
///
/// The live size is Sys8, which common event 48 writes as 20 and which WOLF shares
/// between messages, choices and text pictures (work/yado.txt lines 56-79); the game
/// passes it in through [layout_at_sized] via [crate::game::Game::font_px]. This
/// constant must stay equal to that default, and it is the single source of truth for
/// sizing when no VM exists: the frame is measured with the same advance the glyphs are
/// painted with, so the two can never drift again. They used to: the panel budgeted
/// 16 px per character while the text pass drew 22 px glyphs, so the nine-character
/// opening option 微风吹拂而过的麦田 needed 198 px inside a 144 px content box and
/// wrapped, and its second line was overdrawn by the row below.
pub const TEXT_PX: f32 = 20.0;
/// Width reserved for the cursor marker and its trailing gap, in pixels.
///
/// Every row reserves the same column whether or not it is selected, which is what
/// keeps the option text aligned: one wide glyph plus one space is about 1.5 em.
pub const MARKER_PX: f32 = TEXT_PX * 1.5;
/// Extra width added to the measured line, absorbing glyph side bearings and rounding.
pub const WIDTH_SLACK: f32 = 4.0;
/// Smallest panel width, so a one-letter option is not a 16 px sliver.
pub const MIN_WIDTH: f32 = 120.0;

/// Where the panel and its option lines sit on the logical screen.
#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceLayout {
    /// Panel rectangle, `[x, y, w, h]` in logical pixels.
    pub rect: [f32; 4],
    /// One rectangle per option, top to bottom.
    pub lines: Vec<[f32; 4]>,
    /// Rectangle of the highlighted option (the cursor row).
    pub cursor: [f32; 4],
}

/// Whether a glyph is full width, i.e. one em wide at any size.
///
/// These are the CJK, kana, hangul, full-width form and CJK-symbol planes plus the
/// supplementary ideographic planes. Everything else is treated as half width, which is
/// right for Latin, digits and the ASCII punctuation WOLF menus use.
fn is_wide(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x20000..=0x3FFFD
    )
}

/// Advance width of one character at [`TEXT_PX`], in pixels.
/// Advance of one character at a given font size, in pixels.
///
/// Full-width glyphs (CJK, kana, hangul, full-width forms) take one em; everything
/// else takes half an em. The message window draws at Sys8 rather than [`TEXT_PX`],
/// so the advance has to be a function of the font size, not a constant.
pub fn advance_at(ch: char, font_px: f32) -> f32 {
    if is_wide(ch) {
        font_px
    } else {
        font_px * 0.5
    }
}

/// Width the text pass will need to draw `text` on one line, in pixels.
#[must_use]
pub fn text_width(text: &str) -> f32 {
    text_width_at(text, TEXT_PX)
}

/// Width `text` needs when the text pass draws it at `font_px`, in pixels.
pub fn text_width_at(text: &str, font_px: f32) -> f32 {
    text.chars().map(|ch| advance_at(ch, font_px)).sum()
}

/// Width an option actually needs after its display markup is resolved.
///
/// The panel used to measure the raw string, so `\f[12]` counted as five visible
/// characters and the frame widened around codes nobody sees. Measuring the styled
/// runs keeps the frame and the glyphs on the same arithmetic.
/// [styled_width] with live variable/database resolution.
fn styled_width_with(text: &str, font_px: f32, vars: Option<&crate::textpic::Ctx<'_>>) -> f32 {
    message::inline_runs_with(text, font_px, &WhiteColours, vars)
        .iter()
        .map(|run| run.rect[2])
        .sum()
}

/// Panel size in logical pixels, rounded so every row lands on a whole pixel.
///
/// The width is the widest option measured at the size the text pass actually draws,
/// plus the cursor column and the frame padding. A flat per-character estimate is what
/// broke CJK: a full-width glyph is one em, not half of one.
fn size(options: &[String], font_px: f32) -> (f32, f32) {
    size_with(options, font_px, None)
}

/// [size] with live variable/database resolution.
fn size_with(
    options: &[String],
    font_px: f32,
    vars: Option<&crate::textpic::Ctx<'_>>,
) -> (f32, f32) {
    let widest = options
        .iter()
        .map(|option| styled_width_with(option, font_px, vars))
        .fold(0.0_f32, f32::max);
    let marker = font_px * 1.5;
    let width = (widest + marker + PADDING * 2.0 + WIDTH_SLACK).max(MIN_WIDTH);
    let rows = options.len().max(1) as f32;
    (width.round(), (rows * LINE_HEIGHT + PADDING * 2.0).round())
}

/// Builds rows and cursor rectangle for a panel whose top-left corner is `(x, y)`.
fn build(options: &[String], cursor: usize, x: f32, y: f32, font_px: f32) -> ChoiceLayout {
    build_with(options, cursor, x, y, font_px, None)
}

/// [build] with live variable/database resolution.
fn build_with(
    options: &[String],
    cursor: usize,
    x: f32,
    y: f32,
    font_px: f32,
    vars: Option<&crate::textpic::Ctx<'_>>,
) -> ChoiceLayout {
    let (width, height) = size_with(options, font_px, vars);
    let rect = [x, y, width, height];
    let mut lines = Vec::with_capacity(options.len());
    for (index, _) in options.iter().enumerate() {
        lines.push([
            rect[0] + PADDING,
            rect[1] + PADDING + index as f32 * LINE_HEIGHT,
            rect[2] - PADDING * 2.0,
            LINE_HEIGHT,
        ]);
    }
    let cursor_rect = lines
        .get(cursor)
        .copied()
        .or_else(|| lines.first().copied())
        .unwrap_or([rect[0], rect[1], rect[2], LINE_HEIGHT]);
    ChoiceLayout {
        rect,
        lines,
        cursor: cursor_rect,
    }
}

/// Lays out a choice panel for `options` with `cursor` highlighted, centred on the
/// logical screen.
///
/// This is the layout for a host with no engine anchor (tests, offline tools). The
/// game itself goes through [`layout_at`] with the script's own system variables,
/// because the original engine never centres a choice window on its own.
#[must_use]
pub fn layout(options: &[String], cursor: usize) -> ChoiceLayout {
    let (width, height) = size(options, TEXT_PX);
    let x = ((SCREEN_W as f32) - width) / 2.0;
    let y = ((SCREEN_H as f32) - height) / 2.0;
    build(options, cursor, x.round(), y.round(), TEXT_PX)
}

/// Lays out the panel at the engine's own choice-window anchor, `None` when it would
/// not be visible.
///
/// WOLF places `102`'s window at system variables 3 (X) and 4 (Y), measured in
/// docs/OPCODE_102.md. The title event parks that window at (512, 1000) on a
/// 576-pixel-tall screen, i.e. deliberately off screen: `None` is then the faithful
/// answer, because the engine would paint nothing either. Any other anchor is taken as
/// the top-left corner, exactly like the manual describes.
#[must_use]
pub fn layout_at(options: &[String], cursor: usize, x: i32, y: i32) -> Option<ChoiceLayout> {
    layout_at_sized(options, cursor, x, y, TEXT_PX)
}

/// layout_at with the engine's live font size (Sys8) instead of the fallback.
#[must_use]
pub fn layout_at_sized(
    options: &[String],
    cursor: usize,
    x: i32,
    y: i32,
    font_px: f32,
) -> Option<ChoiceLayout> {
    layout_at_sized_with(options, cursor, x, y, font_px, None)
}

/// [layout_at_sized] with live variable/database resolution.
///
/// A 102 option may contain \cdb, \v or \cself escapes. Measuring the raw source
/// would size the panel around invisible control codes; resolving first keeps the frame
/// and the painted glyphs on the same arithmetic, so long options never truncate.
pub fn layout_at_sized_with(
    options: &[String],
    cursor: usize,
    x: i32,
    y: i32,
    font_px: f32,
    vars: Option<&crate::textpic::Ctx<'_>>,
) -> Option<ChoiceLayout> {
    if options.is_empty() {
        return None;
    }
    let panel = build_with(options, cursor, x as f32, y as f32, font_px, vars);
    let on_screen = panel.rect[0] < SCREEN_W as f32
        && panel.rect[1] < SCREEN_H as f32
        && panel.rect[0] + panel.rect[2] > 0.0
        && panel.rect[1] + panel.rect[3] > 0.0;
    if on_screen {
        Some(panel)
    } else {
        None
    }
}

/// The nine window pieces that paint `layout`'s frame.
#[must_use]
pub fn skin_pieces(layout: &ChoiceLayout, skin_w: u32, skin_h: u32) -> Vec<WindowPiece> {
    window::nine_slice(layout.rect, skin_w, skin_h)
}

/// Builds the panel draw list: window pieces first, then one text row per option.
///
/// The selected row is marked with a leading angle quote because the port has no
/// cursor sprite wired up yet; the sprite in `SystemGraphic/cursol.png` would be the
/// faithful choice.
#[allow(clippy::too_many_arguments)]
fn draw(
    atlas: &mut Atlas,
    assets: &mut Assets,
    options: &[String],
    cursor: usize,
    layout: &ChoiceLayout,
    font_px: f32,
    colours: &dyn Colours,
    vars: Option<&crate::textpic::Ctx<'_>>,
) -> (Vec<SpriteQuad>, Vec<TextQuad>) {
    let mut sprites = Vec::new();
    if let Some(skin) = atlas.add_asset(assets, window::SKIN) {
        for piece in skin_pieces(layout, skin.w, skin.h) {
            sprites.push(SpriteQuad {
                number: window::PANEL_PICTURE,
                rect: piece.rect,
                uv: skin.sub_uv(atlas.size()[0], atlas.size()[1], piece.src),
                alpha: 1.0,
                tint: [1.0, 1.0, 1.0],
            });
        }
    }
    let marker_px = font_px * 1.5;
    let mut texts = Vec::new();
    for (index, option) in options.iter().enumerate() {
        let line = layout.lines[index];
        let marker = if index == cursor { "▶" } else { "" };
        if !marker.is_empty() {
            texts.push(TextQuad {
                number: window::PANEL_PICTURE,
                text: marker.to_owned(),
                rect: [line[0], line[1], marker_px, LINE_HEIGHT],
                alpha: 1.0,
                font_px: 0.0,
                colour: [255, 255, 255],
                outline: false,
            });
        }
        let mut x = line[0] + marker_px;
        for run in message::inline_runs_with(option, font_px, colours, vars) {
            let top = line[1] + (font_px - run.font_px).max(0.0);
            texts.push(TextQuad {
                number: window::PANEL_PICTURE,
                text: run.text,
                // The shaper's real advance can exceed the half/whole-em estimate by a
                // side bearing; one em of slack stops the last glyph wrapping.
                rect: [x, top, run.rect[2] + run.font_px, LINE_HEIGHT],
                alpha: 1.0,
                font_px: run.font_px,
                colour: run.colour.unwrap_or([255, 255, 255]),
                outline: run.outline,
            });
            x += run.rect[2];
        }
    }
    (sprites, texts)
}

/// Builds the centred panel: window pieces first, then one text row per option.
pub fn overlay(
    atlas: &mut Atlas,
    assets: &mut Assets,
    options: &[String],
    cursor: usize,
) -> (Vec<SpriteQuad>, Vec<TextQuad>) {
    if options.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let panel = layout(options, cursor);
    draw(
        atlas,
        assets,
        options,
        cursor,
        &panel,
        TEXT_PX,
        &WhiteColours,
        None,
    )
}

/// Builds the engine-anchored draw list, `None` when the window would be invisible.
///
/// Telling the caller that nothing was painted is better than silently dropping a
/// prompt a player still has to answer.
pub fn overlay_at(
    atlas: &mut Atlas,
    assets: &mut Assets,
    options: &[String],
    cursor: usize,
    x: i32,
    y: i32,
) -> Option<(Vec<SpriteQuad>, Vec<TextQuad>)> {
    let panel = layout_at(options, cursor, x, y)?;
    Some(draw(
        atlas,
        assets,
        options,
        cursor,
        &panel,
        TEXT_PX,
        &WhiteColours,
        None,
    ))
}

/// overlay_at with the engine's live font size (Sys8).
pub fn overlay_at_sized(
    atlas: &mut Atlas,
    assets: &mut Assets,
    options: &[String],
    cursor: usize,
    x: i32,
    y: i32,
    font_px: f32,
) -> Option<(Vec<SpriteQuad>, Vec<TextQuad>)> {
    let panel = layout_at_sized(options, cursor, x, y, font_px)?;
    Some(draw(
        atlas,
        assets,
        options,
        cursor,
        &panel,
        font_px,
        &WhiteColours,
        None,
    ))
}

/// overlay_at_sized with the SysDatabase palette, so `\c[n]` and `\E` in an option
/// resolve the same way they do in a message or a text picture.
#[allow(clippy::too_many_arguments)]
pub fn overlay_at_sized_styled(
    atlas: &mut Atlas,
    assets: &mut Assets,
    options: &[String],
    cursor: usize,
    x: i32,
    y: i32,
    font_px: f32,
    colours: &dyn Colours,
) -> Option<(Vec<SpriteQuad>, Vec<TextQuad>)> {
    overlay_at_sized_styled_with(atlas, assets, options, cursor, x, y, font_px, colours, None)
}

/// [overlay_at_sized_styled] with live variable/database resolution for option text.
#[allow(clippy::too_many_arguments)]
pub fn overlay_at_sized_styled_with(
    atlas: &mut Atlas,
    assets: &mut Assets,
    options: &[String],
    cursor: usize,
    x: i32,
    y: i32,
    font_px: f32,
    colours: &dyn Colours,
    vars: Option<&crate::textpic::Ctx<'_>>,
) -> Option<(Vec<SpriteQuad>, Vec<TextQuad>)> {
    let panel = layout_at_sized_with(options, cursor, x, y, font_px, vars)?;
    Some(draw(
        atlas, assets, options, cursor, &panel, font_px, colours, vars,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(count: usize) -> Vec<String> {
        (0..count).map(|index| format!("option {index}")).collect()
    }

    #[test]
    fn a_three_option_panel_is_centred_and_stacked() {
        let layout = layout(&options(3), 0);
        assert!((layout.rect[0] + layout.rect[2] / 2.0 - 512.0).abs() <= 1.0);
        assert!((layout.rect[1] + layout.rect[3] / 2.0 - 288.0).abs() <= 1.0);
        assert_eq!(layout.lines.len(), 3);
        assert!(layout.lines[1][1] > layout.lines[0][1]);
        assert_eq!(layout.cursor, layout.lines[0]);
    }

    #[test]
    fn the_cursor_follows_the_selected_row() {
        let panel = layout(&options(3), 2);
        assert_eq!(panel.cursor, panel.lines[2]);
        // Out-of-range cursors clamp to a real row instead of panicking.
        assert_eq!(layout(&options(3), 99).cursor, panel.lines[0]);
    }

    #[test]
    fn a_cjk_option_is_wide_enough_for_the_text_it_draws() {
        // Regression for the clipped opening prompt: a nine-character CJK option is 9 em
        // wide at the size the glyphs are drawn, and the row has to fit the cursor column
        // plus that text or the renderer wraps it into the row below.
        let option = "微风吹拂而过的麦田".to_owned();
        let panel = layout(std::slice::from_ref(&option), 0);
        let content = panel.lines[0][2];
        assert_eq!(text_width(&option), 9.0 * TEXT_PX);
        assert!(
            content >= text_width(&option) + MARKER_PX,
            "row {content} must fit the marker plus {} px of text",
            text_width(&option)
        );
        assert!(panel.rect[0] >= 0.0 && panel.rect[0] + panel.rect[2] <= SCREEN_W as f32);
    }

    #[test]
    fn a_long_option_widens_the_panel() {
        let narrow = layout(&["a".to_owned()], 0);
        let wide = layout(&["a".repeat(40)], 0);
        assert_eq!(narrow.rect[2], MIN_WIDTH);
        assert!(wide.rect[2] > narrow.rect[2]);
    }

    #[test]
    fn an_empty_prompt_draws_nothing() {
        let mut atlas = Atlas::new();
        let mut assets = Assets::new("no-such-root");
        let (sprites, texts) = overlay(&mut atlas, &mut assets, &[], 0);
        assert!(sprites.is_empty() && texts.is_empty());
    }

    #[test]
    fn the_overlay_marks_the_cursor_row() {
        let mut atlas = Atlas::new();
        let mut assets = Assets::new("no-such-root");
        // Without the skin asset on disk only the rows come back: the panel degrades
        // to text instead of failing the frame.
        let (sprites, texts) = overlay(&mut atlas, &mut assets, &options(2), 1);
        assert!(sprites.is_empty(), "no skin available in a bare root");
        // The cursor is its own run so a mixed-size option does not shrink it.
        assert_eq!(texts.len(), 3);
        assert!(!texts[0].text.starts_with('\u{25b6}'));
        assert_eq!(texts[1].text, "\u{25b6}");
        assert_eq!(texts[2].text, "option 1");
    }

    #[test]
    fn option_markup_is_stripped_and_its_size_escape_is_honoured() {
        let options = vec![
            "「是啊。」\u{3000}\\f[12]NORMAL".to_owned(),
            "\\f[20]「我觉得可以再拼一点儿……」\u{3000}\\f[12]HARD".to_owned(),
        ];
        let mut atlas = Atlas::new();
        let mut assets = Assets::new("no-such-root");
        let (_, texts) = overlay(&mut atlas, &mut assets, &options, 0);
        let shown: String = texts.iter().map(|text| text.text.as_str()).collect();
        assert!(
            !shown.contains("\\f["),
            "a control escape must never reach the glyph pass: {shown}"
        );
        assert!(shown.contains("NORMAL") && shown.contains("HARD"));
        assert!(
            texts
                .iter()
                .any(|text| text.text.contains("NORMAL") && (text.font_px - 12.0).abs() < 0.01),
            "the \\f[12] span keeps its own size"
        );
        // The frame is measured from visible glyphs, not from the raw escape bytes.
        let raw_width = text_width_at(&options[1], TEXT_PX);
        let panel = layout(&options, 0);
        assert!(
            panel.rect[2] < raw_width + MARKER_PX + PADDING * 2.0,
            "panel {} should be narrower than a raw-string measurement {raw_width}",
            panel.rect[2]
        );
    }
}
