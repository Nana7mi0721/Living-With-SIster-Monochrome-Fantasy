//! GPU text pass for the 1024x576 logical framebuffer.
//!
//! WHY glyphon + cosmic-text instead of a hand-rolled bitmap or SDF font path:
//! WOLF text is runtime-shaped, proportional CJK. The retail game ships its own
//! CJK TTF files (07yasashi.ttf, Lolita.ttf, MSGOTHIC.TTF, ...) and the CN patch
//! mixes Japanese and Simplified-Chinese strings inside the same archive, so no
//! glyph atlas can be baked at build time -- glyphs have to be rasterised from
//! the player's own font copy at whatever pixel size a picture asks for.
//! cosmic-text gives UAX#14 line breaking, bidi and swash rasterisation; glyphon
//! gives the instanced-quad atlas path on top of it; and glyphon 0.12.0 depends
//! on wgpu 30.0.0 while this crate is on 30.0.1, so exactly one wgpu is linked
//! (see docs/ARCHITECTURE.md section 3).
//!
//! WHY the font arrives as bytes and never as a path: all game assets, fonts
//! included, are read at runtime from the player's own install
//! (docs/ARCHITECTURE.md section 4). A hard-coded path here would make the
//! renderer depend on one machine's layout, and the font file names differ
//! between the retail JP build and the CN patch.
//!
//! WHAT WOLF actually asks for (measured against CommonEvent 94, the save/load
//! screen): a text picture is a rectangle plus a string, and the string already
//! has every WOLF escape (font, colour, outline, newline) expanded by the data
//! layer, so this module lays out plain text only. Pictures of type 2 routinely
//! carry a 1x1 rectangle because WOLF itself sizes them from the text, which is
//! why `layout_box` derives the width when the rectangle is degenerate.

use glyphon::cosmic_text::Align;
use glyphon::{
    fontdb, Attrs, Buffer, Cache, Color, Family, FontSystem, Metrics, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer as GlyphonTextRenderer, Viewport,
    Wrap,
};
use wgpu::{Device, MultisampleState, Queue, RenderPass, TextureFormat};

/// U+FFFD, substituted for every character the loaded font has no glyph for.
///
/// WHY substitute instead of dropping the character: a dropped CJK glyph turns
/// text into silent nonsense, while a visible replacement character makes a
/// missing-glyph regression obvious in a screenshot diff.
pub const REPLACEMENT_CHAR: char = '\u{FFFD}';

/// Line pitch as a multiple of the font size, matching WOLF's default row
/// height (it draws message text on a fixed pitch, not on the font's own
/// ascender+descender box).
pub const LINE_HEIGHT_FACTOR: f32 = 1.25;

/// Smallest font size the pass will honour (guards `Metrics` against 0).
pub const MIN_SIZE_PX: f32 = 6.0;

/// Largest font size the pass will honour (a 1024x576 canvas cannot show more).
pub const MAX_SIZE_PX: f32 = 256.0;

/// A rectangle whose width or height is at most this many logical pixels is
/// treated as "size me from my content".
///
/// WHY: WOLF text pictures are emitted with a 1x1 rectangle (CommonEvent 94,
/// picture 12501 = 1x1 at (715,115) carrying a seven-line notice) because the
/// engine sizes the window from the text; a renderer that trusted the rectangle
/// would clip that picture to a single pixel.
pub const AUTO_BOX_THRESHOLD: f32 = 2.0;

/// Locale handed to cosmic-text.
///
/// WHY zh-CN: Han unification makes the same code point pick different glyph
/// shapes in Japanese and Simplified-Chinese fonts, and this install is the CN
/// patch on top of the Japanese base game, so the Chinese shapes are the ones
/// the patch author saw.
const LOCALE: &str = "zh-CN";

/// Failure modes of the text pass.
#[derive(Debug)]
pub enum TextError {
    /// `TextRenderer::set_font` was handed an empty byte slice.
    EmptyFont,
    /// The font bytes parsed but the database exposes no face for them.
    FontRejected,
    /// `TextRenderer::prepare` ran before a font was set.
    NoFont,
    /// Glyph layout / atlas upload failed (message is glyphon's own).
    Prepare(String),
    /// Encoding the glyph draw call failed.
    Render(String),
}

impl core::fmt::Display for TextError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyFont => f.write_str("font bytes are empty"),
            Self::FontRejected => f.write_str("font parsed but exposes no usable face"),
            Self::NoFont => f.write_str("no font has been loaded into the text pass"),
            Self::Prepare(e) => write!(f, "glyph preparation failed: {e}"),
            Self::Render(e) => write!(f, "glyph render failed: {e}"),
        }
    }
}

impl std::error::Error for TextError {}

/// Horizontal alignment of a text picture.
///
/// WOLF pictures are left-aligned by default; the other two exist because menu
/// pictures on the title/save screens are right- or centre-aligned in their box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    /// Flush left (WOLF default).
    #[default]
    Left,
    /// Centred inside the box.
    Center,
    /// Flush right.
    Right,
}

impl TextAlign {
    const fn to_cosmic(self) -> Align {
        match self {
            Self::Left => Align::Left,
            Self::Center => Align::Center,
            Self::Right => Align::Right,
        }
    }
}

/// One text picture to draw this frame.
///
/// This is to text what `crate::SpriteInstance` is to sprites: a flat
/// description that owns no GPU state, so the game layer can build the list from
/// WOLF pictures without knowing that glyphon exists.
#[derive(Clone, Debug, PartialEq)]
pub struct TextItem {
    /// UTF-8 text with WOLF escapes already expanded (newlines are real newlines).
    pub text: String,
    /// Destination rectangle in logical pixels: [x, y, w, h] from the top-left
    /// of the 1024x576 canvas.
    pub rect: [f32; 4],
    /// Opacity multiplier in 0.0..=1.0 (WOLF picture alpha).
    pub alpha: f32,
    /// Font size in logical pixels.
    pub size_px: f32,
    /// Horizontal alignment inside `rect`.
    pub align: TextAlign,
}

impl TextItem {
    /// A left-aligned, fully opaque item.
    #[must_use]
    pub fn new(text: impl Into<String>, rect: [f32; 4], size_px: f32) -> Self {
        Self {
            text: text.into(),
            rect,
            alpha: 1.0,
            size_px,
            align: TextAlign::Left,
        }
    }

    /// Set opacity (clamped to 0.0..=1.0 at prepare time).
    #[must_use]
    pub fn with_alpha(mut self, alpha: f32) -> Self {
        self.alpha = alpha;
        self
    }

    /// Set horizontal alignment.
    #[must_use]
    pub fn with_align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }
}

/// Resolved placement of one `TextItem` inside the framebuffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutBox {
    /// Rounded left edge in logical pixels (may be negative: glyphon clips).
    pub left: f32,
    /// Rounded top edge in logical pixels (may be negative).
    pub top: f32,
    /// Width of the layout box in logical pixels, always >= 1.
    pub width: f32,
    /// Height of the layout box in logical pixels, always >= 1.
    pub height: f32,
    /// Integer scissor rectangle [left, top, right, bottom], half-open, already
    /// intersected with the framebuffer.
    pub clip: [i32; 4],
}

/// Resolve a WOLF picture rectangle into a layout box.
///
/// Returns `None` when the item can never be visible (non-finite values,
/// zero-sized canvas, or a rectangle entirely outside the canvas) so the caller
/// can skip it instead of asking the GPU for an empty scissor.
///
/// WHY round the origin: text on a 1024x576 pixel-art canvas that starts on a
/// fractional pixel gets resampled by the integer blit and turns into grey
/// mush; WOLF coordinates are integral anyway.
///
/// WHY clip: glyphon rasterises outside the viewport quite happily, but its
/// scissor is expressed in integer target pixels, and negative bounds make the
/// scissor disagree with the glyph positions it computed, so the box is
/// intersected here instead.
#[must_use]
pub fn layout_box(rect: [f32; 4], screen: [u32; 2]) -> Option<LayoutBox> {
    let [x, y, w, h] = rect;
    if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()) {
        return None;
    }
    if screen[0] == 0 || screen[1] == 0 {
        return None;
    }
    let (screen_w, screen_h) = (screen[0] as f32, screen[1] as f32);

    let left = x.round();
    let top = y.round();
    let width = if w <= AUTO_BOX_THRESHOLD {
        (screen_w - left).max(1.0)
    } else {
        w.round().max(1.0)
    };
    let height = if h <= AUTO_BOX_THRESHOLD {
        (screen_h - top).max(1.0)
    } else {
        h.round().max(1.0)
    };

    let clip_left = left.max(0.0);
    let clip_top = top.max(0.0);
    let clip_right = (left + width).min(screen_w);
    let clip_bottom = (top + height).min(screen_h);
    if clip_right <= clip_left || clip_bottom <= clip_top {
        return None;
    }

    Some(LayoutBox {
        left,
        top,
        width,
        height,
        clip: [
            clip_left.floor() as i32,
            clip_top.floor() as i32,
            clip_right.ceil() as i32,
            clip_bottom.ceil() as i32,
        ],
    })
}

/// Replace every character listed in `missing` with `REPLACEMENT_CHAR`.
///
/// Returns the rewritten string and how many characters were replaced. Pure, so
/// it can be tested without a GPU or a font.
#[must_use]
pub fn substitute_missing(text: &str, missing: &[char]) -> (String, usize) {
    if missing.is_empty() {
        return (text.to_owned(), 0);
    }
    let mut out = String::with_capacity(text.len());
    let mut replaced = 0_usize;
    for ch in text.chars() {
        if missing.contains(&ch) {
            out.push(REPLACEMENT_CHAR);
            replaced += 1;
        } else {
            out.push(ch);
        }
    }
    (out, replaced)
}

/// What the last `TextRenderer::prepare` did, for logging and tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextStats {
    /// Items that survived layout (were on screen).
    pub items: usize,
    /// Glyph quads submitted.
    pub glyphs: usize,
    /// Layout lines produced.
    pub lines: usize,
    /// Distinct characters the font could not cover, in first-seen order.
    pub missing: Vec<char>,
    /// Total characters substituted with U+FFFD.
    pub substituted: usize,
}

/// Turn a freshly loaded font blob into a `FontSystem` holding only that font.
///
/// WHY a private font database instead of `FontSystem::new()`: the default
/// constructor loads every system font (slow, and noisy), and more importantly it
/// silently borrows a system fallback for characters the game font lacks --
/// which would turn a real coverage bug into a "looks fine on my machine" render.
/// With a single-font database the platform fallback family names never resolve,
/// so `LayoutGlyph::glyph_id == 0` means what it says.
fn build_font_system(ttf: &[u8]) -> Result<(FontSystem, fontdb::ID, String), TextError> {
    if ttf.is_empty() {
        return Err(TextError::EmptyFont);
    }
    let mut db = fontdb::Database::new();
    db.load_font_data(ttf.to_vec());
    let face = db.faces().next().ok_or(TextError::FontRejected)?;
    let id = face.id;
    let family = face
        .families
        .first()
        .map(|(name, _language)| name.clone())
        .ok_or(TextError::FontRejected)?;
    let system = FontSystem::new_with_locale_and_db(LOCALE.to_string(), db);
    Ok((system, id, family))
}

/// Characters in `buffer` that the primary face could not draw.
fn missing_chars(buffer: &Buffer, font_id: fontdb::ID) -> Vec<char> {
    let mut out: Vec<char> = Vec::new();
    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            // glyph_id 0 is .notdef; a different font_id means cosmic-text
            // silently swapped in another face, which for us is also a miss.
            if glyph.glyph_id != 0 && glyph.font_id == font_id {
                continue;
            }
            let Some(slice) = run.text.get(glyph.start..glyph.end) else {
                continue;
            };
            let Some(ch) = slice.chars().next() else {
                continue;
            };
            if ch.is_whitespace() || ch.is_control() || ch == REPLACEMENT_CHAR {
                continue;
            }
            if !out.contains(&ch) {
                out.push(ch);
            }
        }
    }
    out
}

/// Report which characters of `text` the given font cannot draw.
///
/// Pure CPU work (no GPU): a caller can use it to pick between the fonts a WOLF
/// install ships before building a renderer at all, which is exactly what
/// `examples/text_blit.rs` does.
#[must_use]
pub fn missing_glyphs(font: &[u8], text: &str, size_px: f32) -> Vec<char> {
    let Ok((mut system, font_id, family)) = build_font_system(font) else {
        return Vec::new();
    };
    let size = size_px.clamp(MIN_SIZE_PX, MAX_SIZE_PX);
    let mut buffer = Buffer::new(&mut system, Metrics::new(size, size * LINE_HEIGHT_FACTOR));
    let attrs = Attrs::new()
        .family(Family::Name(&family))
        .color(Color::rgb(255, 255, 255));
    buffer.set_text(text, &attrs, Shaping::Advanced, Some(Align::Left));
    buffer.shape_until_scroll(&mut system, false);
    missing_chars(&buffer, font_id)
}

/// Owns the glyphon atlas + font system and draws `TextItem`s into a render pass.
///
/// One instance per `super::Renderer`. The atlas and the shaped buffers live
/// here so that sprite and text draws can share one render pass, and therefore
/// one load/clear, which is what keeps the 1024x576 canvas compositing right.
pub struct TextRenderer {
    cache: Cache,
    atlas: TextAtlas,
    viewport: Viewport,
    glyphs: GlyphonTextRenderer,
    font_system: FontSystem,
    swash: SwashCache,
    /// Face id of the loaded font; `None` until `set_font`.
    font_id: Option<fontdb::ID>,
    /// Family name to ask cosmic-text for; empty until `set_font`.
    font_family: String,
    /// Logical canvas size the pass renders into.
    screen: [u32; 2],
    /// Shaped buffers for the current frame, indexed by `placed`.
    buffers: Vec<Buffer>,
    /// (buffer index, layout box, alpha byte) for the current frame.
    placed: Vec<(usize, LayoutBox, u8)>,
    /// Statistics from the last successful `prepare`.
    stats: TextStats,
}

impl TextRenderer {
    /// Create the text pass for a framebuffer of `screen_size` in `target_format`.
    ///
    /// `target_format` must match the texture the render pass targets
    /// (`crate::FRAME_FORMAT` for the game canvas), because glyphon bakes it into
    /// its pipeline.
    #[must_use]
    pub fn new(
        device: &Device,
        queue: &Queue,
        target_format: TextureFormat,
        screen_size: [u32; 2],
    ) -> Self {
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, target_format);
        let glyphs =
            GlyphonTextRenderer::new(&mut atlas, device, MultisampleState::default(), None);
        let mut viewport = Viewport::new(device, &cache);
        let screen = [screen_size[0].max(1), screen_size[1].max(1)];
        viewport.update(
            queue,
            Resolution {
                width: screen[0],
                height: screen[1],
            },
        );
        // Empty font database: set_font() installs the real one. Starting empty
        // keeps construction cheap (FontSystem::new() parses ~1000 system faces).
        let empty_db = fontdb::Database::new();
        let font_system = FontSystem::new_with_locale_and_db(LOCALE.to_string(), empty_db);
        Self {
            cache,
            atlas,
            viewport,
            glyphs,
            font_system,
            swash: SwashCache::new(),
            font_id: None,
            font_family: String::new(),
            screen,
            buffers: Vec::new(),
            placed: Vec::new(),
            stats: TextStats::default(),
        }
    }

    /// Install the TTF/OTF blob the pass will shape text with.
    ///
    /// The bytes come from the player's install (see the module docs); nothing is
    /// cached on disk here. On failure the previously loaded font keeps working.
    pub fn set_font(&mut self, ttf: &[u8]) -> Result<(), TextError> {
        let (system, id, family) = build_font_system(ttf)?;
        self.font_system = system;
        self.font_id = Some(id);
        self.font_family = family;
        self.buffers.clear();
        self.placed.clear();
        self.stats = TextStats::default();
        Ok(())
    }

    /// Whether `set_font` has succeeded at least once.
    #[must_use]
    pub const fn has_font(&self) -> bool {
        self.font_id.is_some()
    }

    /// Family name of the loaded font, if any.
    #[must_use]
    pub fn font_family(&self) -> Option<&str> {
        self.font_id.map(|_| self.font_family.as_str())
    }

    /// Change the logical canvas the pass lays text out on.
    ///
    /// WOLF is fixed at 1024x576, so this only matters to tests.
    pub fn set_screen_size(&mut self, queue: &Queue, screen: [u32; 2]) {
        self.screen = [screen[0].max(1), screen[1].max(1)];
        self.viewport.update(
            queue,
            Resolution {
                width: self.screen[0],
                height: self.screen[1],
            },
        );
    }

    /// Shape and upload every item, and remember them for `Self::render`.
    ///
    /// Call before opening the render pass (it only writes to the queue and the
    /// glyph atlas), then `Self::render` inside the pass. Splitting the two is
    /// what lets sprites and text share one pass.
    pub fn prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        items: &[TextItem],
    ) -> Result<TextStats, TextError> {
        let font_id = self.font_id.ok_or(TextError::NoFont)?;
        let family = self.font_family.clone();
        self.buffers.clear();
        self.placed.clear();
        self.stats = TextStats::default();

        for item in items {
            let Some(bx) = layout_box(item.rect, self.screen) else {
                continue;
            };
            let size = if item.size_px.is_finite() {
                item.size_px.clamp(MIN_SIZE_PX, MAX_SIZE_PX)
            } else {
                MIN_SIZE_PX
            };
            let alpha = if item.alpha.is_finite() {
                (item.alpha.clamp(0.0, 1.0) * 255.0).round() as u8
            } else {
                255
            };
            if alpha == 0 || item.text.is_empty() {
                continue;
            }

            let mut buffer = Buffer::new(
                &mut self.font_system,
                Metrics::new(size, size * LINE_HEIGHT_FACTOR),
            );
            buffer.set_size(Some(bx.width), Some(bx.height));
            // CJK has no spaces, so word wrapping alone would overflow the box;
            // WordOrGlyph breaks inside a word only when it has to.
            buffer.set_wrap(Wrap::WordOrGlyph);
            let attrs = Attrs::new()
                .family(Family::Name(&family))
                .color(Color::rgb(255, 255, 255));
            buffer.set_text(
                &item.text,
                &attrs,
                Shaping::Advanced,
                Some(item.align.to_cosmic()),
            );
            buffer.shape_until_scroll(&mut self.font_system, false);

            let mut missing = missing_chars(&buffer, font_id);
            if !missing.is_empty() {
                let (rewritten, replaced) = substitute_missing(&item.text, &missing);
                buffer.set_text(
                    &rewritten,
                    &attrs,
                    Shaping::Advanced,
                    Some(item.align.to_cosmic()),
                );
                buffer.shape_until_scroll(&mut self.font_system, false);
                missing.retain(|ch| !self.stats.missing.contains(ch));
                self.stats.missing.append(&mut missing);
                self.stats.substituted += replaced;
            }

            for run in buffer.layout_runs() {
                self.stats.lines += 1;
                self.stats.glyphs += run.glyphs.len();
            }
            self.stats.items += 1;
            self.buffers.push(buffer);
            self.placed.push((self.buffers.len() - 1, bx, alpha));
        }

        if self.placed.is_empty() {
            return Ok(self.stats.clone());
        }

        let areas: Vec<TextArea<'_>> = self
            .placed
            .iter()
            .map(|&(index, bx, alpha)| TextArea {
                buffer: &self.buffers[index],
                left: bx.left,
                top: bx.top,
                scale: 1.0,
                bounds: TextBounds {
                    left: bx.clip[0],
                    top: bx.clip[1],
                    right: bx.clip[2],
                    bottom: bx.clip[3],
                },
                // TextArea has no alpha of its own in glyphon 0.12, so picture
                // opacity has to ride on the default colour.
                default_color: Color::rgba(255, 255, 255, alpha),
                custom_glyphs: &[],
            })
            .collect();

        self.glyphs
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                areas,
                &mut self.swash,
            )
            .map_err(|err| TextError::Prepare(format!("{err}")))?;

        Ok(self.stats.clone())
    }

    /// Encode the prepared glyphs into an already-open render pass.
    ///
    /// Must share the pass with the sprite batch: whichever pass runs first owns
    /// the load/clear, and a second pass would clear the sprites away.
    pub fn render<'pass>(&'pass self, pass: &mut RenderPass<'pass>) -> Result<(), TextError> {
        if self.placed.is_empty() {
            return Ok(());
        }
        self.glyphs
            .render(&self.atlas, &self.viewport, pass)
            .map_err(|err| TextError::Render(format!("{err}")))
    }

    /// Statistics from the last `Self::prepare`.
    #[must_use]
    pub const fn stats(&self) -> &TextStats {
        &self.stats
    }

    /// Release glyphs the atlas no longer needs.
    ///
    /// WHY it is not called per frame: trimming walks every cached glyph, so a
    /// 0.5 s cadence keeps the atlas from growing under font-size churn without
    /// paying for it on every frame.
    pub fn trim(&mut self) {
        let _ = &self.cache;
        self.atlas.trim();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WOLF ships text pictures with a 1x1 rect (it computes its own box from the
    /// strings), so the only layout decision the renderer gets to make is "grow to
    /// the canvas edge". Picture 12501 of CommonEvent 94 is the real case:
    /// (715,115) 1x1 on 1024x576 must become a 309x461 box.
    #[test]
    fn a_unit_rect_grows_to_the_canvas_edge() {
        let bx = layout_box([715.0, 115.0, 1.0, 1.0], [1024, 576]).expect("on screen");
        assert_eq!(bx.left, 715.0);
        assert_eq!(bx.top, 115.0);
        assert_eq!(bx.width, 309.0);
        assert_eq!(bx.height, 461.0);
        assert_eq!(bx.clip, [715, 115, 1024, 576]);
    }

    #[test]
    fn an_explicit_rect_is_used_verbatim_and_rounded() {
        let bx = layout_box([44.4, 90.6, 512.0, 400.0], [1024, 576]).expect("on screen");
        assert_eq!((bx.left, bx.top), (44.0, 91.0));
        assert_eq!((bx.width, bx.height), (512.0, 400.0));
        assert_eq!(bx.clip, [44, 91, 556, 491]);
    }

    /// Negative WOLF coordinates happen (pictures slide in from off-screen). The
    /// clip is what keeps glyphon's integer scissor in agreement with the layout.
    #[test]
    fn a_partly_offscreen_box_is_clipped_not_moved() {
        let bx = layout_box([-20.0, -10.0, 100.0, 100.0], [1024, 576]).expect("partly visible");
        assert_eq!((bx.left, bx.top), (-20.0, -10.0));
        assert_eq!(bx.clip, [0, 0, 80, 90]);
    }

    #[test]
    fn entirely_offscreen_or_degenerate_boxes_are_dropped() {
        assert!(layout_box([2000.0, 10.0, 100.0, 100.0], [1024, 576]).is_none());
        assert!(layout_box([10.0, 700.0, 100.0, 100.0], [1024, 576]).is_none());
        assert!(layout_box([f32::NAN, 10.0, 100.0, 100.0], [1024, 576]).is_none());
        assert!(layout_box([10.0, 10.0, f32::INFINITY, 100.0], [1024, 576]).is_none());
        assert!(layout_box([10.0, 10.0, 100.0, 100.0], [0, 576]).is_none());
    }

    /// A zero rect is a *request* for the canvas edge, not an empty box, so it must
    /// never collapse to nothing -- otherwise pictures would silently vanish.
    #[test]
    fn a_zero_size_rect_still_yields_at_least_one_pixel() {
        let bx = layout_box([1000.0, 500.0, 0.0, 0.0], [1024, 576]).expect("on screen");
        assert_eq!((bx.width, bx.height), (24.0, 76.0));
        assert!(layout_box([1.0e6, 1.0e6, 0.0, 0.0], [1024, 576]).is_none());
    }

    #[test]
    fn substitution_replaces_every_occurrence_and_counts_it() {
        // The CN DLC notice really does repeat a missing character: 栏 appears in
        // "存档的第一栏" and again in "第一栏已有存档数据时".
        let text = "第一栏已有存档数据时，请先读取该存档。";
        let (out, replaced) = substitute_missing(text, &['栏', '读']);
        assert_eq!(replaced, 2);
        assert_eq!(out, "第一�已有存档数据时，请先�取该存档。");
        assert!(!out.contains('栏') && !out.contains('读'));
    }

    #[test]
    fn substitution_is_the_identity_when_nothing_is_missing() {
        let (out, replaced) = substitute_missing("はじめから", &[]);
        assert_eq!(replaced, 0);
        assert_eq!(out, "はじめから");
    }

    /// Garbage font bytes must be an error, never a panic: the game has to keep
    /// running (with blank pictures) if the player's Data/ is damaged.
    #[test]
    fn empty_font_bytes_are_rejected() {
        assert!(matches!(missing_glyphs(&[], "a", 12.0), v if v.is_empty()));
        let system = build_font_system(&[]);
        assert!(matches!(system, Err(TextError::EmptyFont)));
        let system = build_font_system(&[0_u8; 64]);
        assert!(matches!(system, Err(TextError::FontRejected)));
    }

    /// Size is clamped so a broken picture record cannot ask the shaper for a
    /// 0-pixel or 100k-pixel font (cosmic-text asserts on the former).
    #[test]
    fn font_sizes_are_clamped_into_the_supported_window() {
        assert_eq!(MIN_SIZE_PX.clamp(MIN_SIZE_PX, MAX_SIZE_PX), MIN_SIZE_PX);
        assert_eq!(0.0_f32.clamp(MIN_SIZE_PX, MAX_SIZE_PX), MIN_SIZE_PX);
        assert_eq!(1.0e9_f32.clamp(MIN_SIZE_PX, MAX_SIZE_PX), MAX_SIZE_PX);
    }
}
