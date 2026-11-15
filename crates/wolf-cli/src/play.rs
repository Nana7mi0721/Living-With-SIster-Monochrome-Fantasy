//! `wolf-cli play` - the interactive title screen.
//!
//! This is the first command that puts the reimplementation on a real screen: it runs
//! event 42 through the interpreter, feeds the resulting picture stack through the atlas
//! and the wgpu renderer, and maps the keyboard onto WOLF's key bitmask so the game's own
//! input polling (opcode 123) sees a player.
//!
//! Frame pacing is a fixed 1/60 s grid rather than "as fast as the GPU allows" because the
//! script counts frames: opcode 180 waits N frames, animations advance per frame, and the
//! title menu's cursor speed is expressed in frames. Running the VM faster than the clock
//! would make menus unusable; running it slower would make animations crawl.
//!
//! The window is 1024x576 logical pixels scaled by the largest whole integer factor that
//! fits (with black bars), so a 1920x1080 desktop shows a crisp 1:1 or 1.875x image
//! instead of a blurry fractional upscale - see `wolf_render::Scaler`.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use wolf_game::atlas::{Atlas, SpriteQuad, TextQuad};
use wolf_game::window::{self, LayerBand};
use wolf_game::{key, Game};
use wolf_render::{Renderer, Scaler, SceneLayer, SpriteInstance, TextItem, LOGICAL_H, LOGICAL_W};
use wolf_vm::rng::{resolve_session_event_seed, EVENT_SEED_ENV};

/// One frame at 60 Hz.
const FRAME_TIME: Duration = Duration::from_micros(16_667);

/// One sprite submission with its semantic band and append sequence.
///
/// Host UI (choice window, nameplate, message cursor) has no script picture slot, so
/// `SpriteQuad::number` cannot say where it belongs. Keeping the band beside the quad
/// lets the interactive and offscreen capture paths sort one common display list by
/// `(band, picture_number, sequence)` instead of trusting append order.
#[derive(Clone, Debug)]
pub(crate) struct LayeredSprite {
    pub band: LayerBand,
    pub number: i32,
    pub seq: usize,
    pub quad: SpriteQuad,
}

/// One text submission with its semantic band and append sequence.
#[derive(Clone, Debug)]
pub(crate) struct LayeredText {
    pub band: LayerBand,
    pub number: i32,
    pub seq: usize,
    pub quad: TextQuad,
}

impl LayeredSprite {
    fn script(quad: SpriteQuad, seq: usize) -> Self {
        Self {
            band: window::picture_band(quad.number),
            number: quad.number,
            seq,
            quad,
        }
    }

    fn host(band: LayerBand, quad: SpriteQuad, seq: usize) -> Self {
        Self {
            band,
            number: quad.number,
            seq,
            quad,
        }
    }
}

impl LayeredText {
    fn script(quad: TextQuad, seq: usize) -> Self {
        Self {
            band: LayerBand::SceneText,
            number: quad.number,
            seq,
            quad,
        }
    }

    fn host(band: LayerBand, quad: TextQuad, seq: usize) -> Self {
        Self {
            band,
            number: quad.number,
            seq,
            quad,
        }
    }
}

/// Stable `(band, picture_number, sequence)` sort used by every submission path.
pub(crate) fn sort_layered_sprites(entries: &mut [LayeredSprite]) {
    entries.sort_by_key(|entry| (entry.band.rank(), entry.number, entry.seq));
}

/// Stable `(band, picture_number, sequence)` sort for the glyph pass.
pub(crate) fn sort_layered_texts(entries: &mut [LayeredText]) {
    entries.sort_by_key(|entry| (entry.band.rank(), entry.number, entry.seq));
}

/// Turns one display-list text entry into the renderer's glyph item.
///
/// Text pictures may mix sizes, colours and outlines inside one string; the
/// layout already split each style change into its own quad.
pub(crate) fn text_item(entry: &LayeredText, default_font_px: f32) -> TextItem {
    let text = &entry.quad;
    let size = if text.font_px > 0.0 {
        text.font_px
    } else {
        default_font_px
    };
    let mut item = TextItem::new(text.text.clone(), text.rect, size)
        .with_alpha(text.alpha)
        .with_colour(text.colour);
    if text.outline {
        item = item.with_outline(2, [0, 0, 0]);
    }
    item
}

/// Submits the display list as one renderer layer per semantic band.
///
/// The inputs must already be sorted by `(band, picture_number, sequence)`;
/// entries are grouped by band without reordering, so the renderer receives
/// ascending bands and clips each layer to the next one instead of relying on
/// one append order for every kind of UI.
pub(crate) fn scene_bands(sprites: &[LayeredSprite], texts: &[LayeredText]) -> Vec<LayerBand> {
    let mut bands: Vec<LayerBand> = Vec::new();
    for band in sprites
        .iter()
        .map(|entry| entry.band)
        .chain(texts.iter().map(|entry| entry.band))
    {
        if !bands.contains(&band) {
            bands.push(band);
        }
    }
    if bands.is_empty() {
        bands.push(LayerBand::Map);
    }
    bands.sort_by_key(|band| band.rank());
    bands
}

pub(crate) fn draw_scene_layers(
    renderer: &mut Renderer,
    sprites: &[LayeredSprite],
    texts: &[LayeredText],
    default_font_px: f32,
) {
    let bands = scene_bands(sprites, texts);

    let sprite_layers: Vec<Vec<SpriteInstance>> = bands
        .iter()
        .map(|band| {
            sprites
                .iter()
                .filter(|entry| entry.band == *band)
                .map(|entry| App::tint_instance(&entry.quad))
                .collect()
        })
        .collect();
    let text_layers: Vec<Vec<TextItem>> = bands
        .iter()
        .map(|band| {
            texts
                .iter()
                .filter(|entry| entry.band == *band)
                .map(|entry| text_item(entry, default_font_px))
                .collect()
        })
        .collect();

    let layers: Vec<SceneLayer<'_>> = bands
        .iter()
        .enumerate()
        .map(|(index, _)| SceneLayer {
            sprites: &sprite_layers[index],
            texts: &text_layers[index],
        })
        .collect();
    renderer.draw_layers(&layers);
}

/// Builds the complete sprite/text display list for one frame.
///
/// This is the single composition point for the live window and the offscreen
/// capture path. Script pictures are classified through `window::picture_band`;
/// host UI passes its band explicitly, so a choice panel stays above a nameplate
/// even though both are drawn from the same `WindowBase.png` skin and both carry
/// `PANEL_PICTURE` as their trace number.
pub(crate) fn build_scene_layers(
    game: &mut Game,
    atlas: &mut Atlas,
) -> (Vec<LayeredSprite>, Vec<LayeredText>) {
    let mut sprites = Vec::new();
    let mut texts = Vec::new();

    // Tilemap first; its quads use TILE_NUMBER[-10000] and classify as Map. Script
    // pictures follow in atlas paint order, keeping their picture number as tie-break.
    for quad in game.tile_quads(atlas) {
        let seq = sprites.len();
        sprites.push(LayeredSprite::script(quad, seq));
    }
    for quad in game.picture_quads(atlas) {
        let seq = sprites.len();
        sprites.push(LayeredSprite::script(quad, seq));
    }
    for quad in game.text_quads() {
        let seq = texts.len();
        texts.push(LayeredText::script(quad, seq));
    }

    // A WOLF script never draws its own choice frame: the engine paints a window skin
    // and moves a cursor over it at system variables 3/4. It is host UI, not a script
    // picture, so the band is explicit rather than derived from PANEL_PICTURE.
    if let Some((panel, rows)) = game.choice_overlay(atlas) {
        for quad in panel {
            let seq = sprites.len();
            sprites.push(LayeredSprite::host(LayerBand::ChoicePanel, quad, seq));
        }
        for quad in rows {
            let seq = texts.len();
            texts.push(LayeredText::host(LayerBand::ChoiceText, quad, seq));
        }
    }

    // The 101 pause cursor is engine art drawn from SystemFile/Text_Pause.png; it is
    // above every panel and text layer in the retail composition.
    let newest = game.messages.last().map(|message| message.frames);
    if let Some(frames) = newest {
        if let Some(layout) = game.message_layout() {
            if let Some(at) = layout.cursor {
                let frame = wolf_game::message::cursor_frame(frames);
                let src = [
                    0,
                    frame * wolf_game::message::CURSOR_FRAME_H,
                    wolf_game::message::CURSOR_FRAME_W,
                    wolf_game::message::CURSOR_FRAME_H,
                ];
                let dst = [
                    at[0],
                    at[1],
                    wolf_game::message::CURSOR_FRAME_W as f32,
                    wolf_game::message::CURSOR_FRAME_H as f32,
                ];
                if let Some(quad) = atlas.sprite_from(
                    &mut game.assets,
                    wolf_game::message::CURSOR_FILE,
                    wolf_game::message::CURSOR_SLOT,
                    src,
                    dst,
                    1.0,
                ) {
                    let seq = sprites.len();
                    sprites.push(LayeredSprite::host(LayerBand::Cursor, quad, seq));
                }
            }
        }
    }

    // Speaker tab for a 101 message with a face: one nine-slice of WindowBase.png,
    // painted above the message frame but below a modal choice.
    if let Some((plate, _name, _font)) = game.message_nameplate() {
        for piece in wolf_game::window::nine_slice(plate, 18, 18) {
            if let Some(quad) = atlas.sprite_from(
                &mut game.assets,
                wolf_game::window::SKIN,
                wolf_game::window::PANEL_PICTURE,
                piece.src,
                piece.rect,
                1.0,
            ) {
                let seq = sprites.len();
                sprites.push(LayeredSprite::host(LayerBand::Nameplate, quad, seq));
            }
        }
    }

    // The 101 message glyphs are engine text, not picture text. Their frame is the
    // 19500 picture group; keeping the glyphs in the MessageText band makes message
    // text stay under a choice prompt while remaining above ordinary scene text.
    if let Some(layout) = game.message_layout() {
        let vars = game.message_vars();
        for run in &layout.runs {
            // Keep the renderer's wrap box at the whole row remainder: the layout
            // measures with the half-em rule, the shaper uses the real (wider)
            // advance, and re-wrapping there clips the tail of the line.
            let mut rect = run.rect;
            let remaining = (vars.wrap_width() - (run.rect[0] - vars.x)).max(run.rect[2]);
            rect[2] = remaining + 2.0;
            let quad = TextQuad {
                number: wolf_game::message::FRAME_PICTURE,
                text: run.text.clone(),
                rect,
                alpha: 1.0,
                font_px: run.font_px,
                colour: run.colour.unwrap_or(wolf_game::message::DEFAULT_COLOUR),
                outline: run.outline,
            };
            let seq = texts.len();
            texts.push(LayeredText::host(LayerBand::MessageText, quad, seq));
        }
    }

    // Speaker name, above the plate it belongs to. This is text, but its band keeps
    // it under a modal choice in the same text pass.
    if let Some((plate, name, font)) = game.message_nameplate() {
        let rect = [
            plate[0] + font * 0.5,
            plate[1] + (plate[3] - font * 1.25) * 0.5,
            (plate[2] - font * 0.25).max(font),
            font * 1.25,
        ];
        let quad = TextQuad {
            number: wolf_game::window::PANEL_PICTURE,
            text: name,
            rect,
            alpha: 1.0,
            font_px: font,
            colour: [255, 255, 255],
            outline: true,
        };
        let seq = texts.len();
        texts.push(LayeredText::host(LayerBand::Nameplate, quad, seq));
    }

    sort_layered_sprites(&mut sprites);
    sort_layered_texts(&mut texts);
    (sprites, texts)
}

/// Runs the title screen in a window until the user closes it (or `frames` pass).
///
/// A non-zero frame budget makes the window self-closing, which is how the interactive
/// path gets smoke-tested: the same code that a player drives also has to survive an
/// unattended run without panicking or wedging the event loop.
pub fn run(
    root: &std::path::Path,
    frames: Option<u64>,
    entry: crate::Entry<'_>,
) -> std::process::ExitCode {
    let launched_at = Instant::now();
    let mut game = match Game::new(root) {
        Ok(game) => game,
        Err(err) => {
            eprintln!("cannot load the game data: {err}");
            return std::process::ExitCode::FAILURE;
        }
    };
    // Retail seeds the event PRNG from GetTickCount at startup, so two launches
    // do not produce the same random sequence. The library keeps a fixed seed
    // for deterministic tests; only the interactive front end opts into the
    // clock. WOLF_EVENT_SEED still wins so a captured retail tick can be replayed.
    let seed_override = std::env::var(EVENT_SEED_ENV).ok();
    let clock_ticks = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u32);
    let event_seed = resolve_session_event_seed(seed_override.as_deref(), clock_ticks);
    game.vm.rng.set_event_seed(event_seed);
    println!(
        "startup: event RNG seed {event_seed} ({})",
        if seed_override.is_some() {
            EVENT_SEED_ENV
        } else {
            "clock"
        }
    );
    // The packaged game is launched by double-clicking the executable, which never
    // reaches the --audio flag parser. A front end meant for a player should always try
    // to open the mixer; the engine falls back to a silent null backend when the machine
    // has no usable output device, so this stays safe on CI.
    game.enable_audio();
    crate::install_font_metrics(&mut game, root);
    println!(
        "startup: data loaded in {:.0} ms",
        launched_at.elapsed().as_secs_f64() * 1000.0
    );
    // The same entry-point rules `capture` uses: default boot (the TitleMap autorun
    // event and its three splashes), --menu-only for common event 42, --map-run for a
    // map script, --event for one common event, --load-save for a save file.
    let (started, label) = match entry {
        crate::Entry::Boot => (
            game.boot().map_err(|err| err.to_string()),
            "boot map".to_owned(),
        ),
        crate::Entry::Menu => (
            game.start_title().map_err(|err| err.to_string()),
            "common event 42".to_owned(),
        ),
        crate::Entry::Script { map, event } => (
            game.start_map_event_by_name(map, event)
                .map_err(|err| err.to_string()),
            format!("map {map} event {event}"),
        ),
        crate::Entry::CommonEvent(id) => {
            game.start_engine_events();
            let started = if game.start_event(id) {
                Ok(())
            } else {
                Err(format!("no common event with id {id}"))
            };
            (started, format!("common event {id}"))
        }
        crate::Entry::Save(path) => {
            let label = format!("save {}", path.display());
            let started = game
                .load_save(path)
                .map(|_| ())
                .map_err(|err| err.to_string());
            (started, label)
        }
    };
    if let Err(err) = started {
        eprintln!("{err}");
        return std::process::ExitCode::FAILURE;
    }

    println!("play: {label}");
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(err) => {
            eprintln!("cannot create an event loop: {err}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let mut app = App {
        game,
        atlas: Atlas::new(),
        sprites: Vec::new(),
        texts: Vec::new(),
        held: HashSet::new(),
        wheel: 0,
        wheel_dir: 0,
        mouse: [-1, -1],
        mouse_buttons: 0,
        window: None,
        renderer: None,
        uploaded_images: usize::MAX,
        frame_limit: frames,
        next_frame: Instant::now(),
        fps_frames: 0,
        fps_since: Instant::now(),
        launched_at,
        window_ready: None,
        step_times: Vec::new(),
    };
    if let Err(err) = event_loop.run_app(&mut app) {
        eprintln!("event loop failed: {err}");
        return std::process::ExitCode::FAILURE;
    }
    println!(
        "exit: {} frame(s), {} instruction(s), {} picture op(s), {} warning(s)",
        app.game.stats.frames,
        app.game.stats.instructions,
        app.game.stats.pictures,
        app.game.stats.warnings
    );
    let (images, bytes) = app.game.asset_cache_stats();
    let (p50, p95, max, mean) = step_summary(&app.step_times);
    println!(
        "perf: total {:.0} ms | startup window {:.0} ms | step p50 {p50:.2} ms, p95 {p95:.2} ms, max {max:.2} ms, mean {mean:.2} ms | assets {images} image(s), {:.1} MiB",
        app.launched_at.elapsed().as_secs_f64() * 1000.0,
        app.window_ready.unwrap_or_default().as_secs_f64() * 1000.0,
        bytes as f64 / (1024.0 * 1024.0)
    );
    if !app.game.diagnostics.is_empty() {
        println!("diagnostics seen while playing:");
        for (text, count) in app.game.diagnostics.iter().take(10) {
            println!("  {count:>6}x {text}");
        }
    }
    std::process::ExitCode::SUCCESS
}

/// The taskbar/title-bar icon, decoded from the same PNG the release icon is generated
/// from. Windows already picks up the embedded exe icon for Explorer; setting it on the
/// window as well keeps the title bar and taskbar consistent when the game is running.
fn window_icon() -> Option<winit::window::Icon> {
    const PNG: &[u8] = include_bytes!("../assets/charicon.png");
    let image = image::load_from_memory_with_format(PNG, image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let (width, height) = image.dimensions();
    winit::window::Icon::from_rgba(image.into_raw(), width, height).ok()
}

struct App {
    game: Game,
    atlas: Atlas,
    sprites: Vec<LayeredSprite>,
    texts: Vec<LayeredText>,
    held: HashSet<u32>,
    /// Scroll wheel turned into WOLF key bits for exactly one frame (see `bits`).
    wheel: u32,
    /// Signed wheel notch for the frame: +1 up, -1 down, 0 when the wheel did not move.
    /// The save/load list polls opcode 123 with mask 771 and compares against these
    /// values, so it cannot be derived from the UP/DOWN key bits above.
    wheel_dir: i32,
    /// Mouse position in game pixels, or (-1, -1) while the cursor is outside the window.
    mouse: [i32; 2],
    /// WOLF mouse button bits: 1 left, 2 right, 4 middle.
    mouse_buttons: u32,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    /// Atlas image count at the last upload; a change means new artwork was packed.
    uploaded_images: usize,
    /// Quit after this many frames (None = run until the window closes).
    frame_limit: Option<u64>,
    next_frame: Instant,
    fps_frames: u32,
    fps_since: Instant,
    /// When the process began loading data, for the startup/perf summary.
    launched_at: Instant,
    /// Time from launch to the first presentable frame, once the renderer exists.
    window_ready: Option<Duration>,
    /// Wall time of each `step()` (script + atlas rebuild), in frame order.
    step_times: Vec<Duration>,
}

/// Combines held keys, the one-frame wheel bits and the mouse buttons into the bitmask
/// written to `Game::key_bits`.
///
/// A held right button is the game's cancel key: the save/load screen (and every other
/// WOLF menu that polls opcode 123) only closes on key code 11, and the port previously
/// reported the right button through `Game::mouse_buttons` alone, which no script reads
/// for that decision. The raw bit survives in `mouse_buttons` so `engine_state(8)`
/// still answers the right-button tri-state.
fn key_bits(held: &HashSet<u32>, wheel: u32, mouse_buttons: u32) -> u32 {
    let mut bits = held.iter().copied().fold(wheel, |acc, bit| acc | bit);
    if mouse_buttons & 2 != 0 {
        bits |= key::CANCEL;
    }
    bits
}

/// Splits a winit scroll delta into the ordinary arrow-key bits and the signed notch the
/// save/load list reads (opcode 123 mask 771).
///
/// Only the vertical component is meaningful: the wheel is horizontal-capable hardware,
/// but this game uses it as up/down. A zero vertical delta leaves both outputs at zero so
/// a horizontal-only event cannot cancel a pending notch.
fn wheel_notch(delta: &MouseScrollDelta) -> (u32, i32) {
    let y = match delta {
        MouseScrollDelta::LineDelta(_, y) => *y,
        MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
    };
    if y > 0.0 {
        (key::UP, 1)
    } else if y < 0.0 {
        (key::DOWN, -1)
    } else {
        (0, 0)
    }
}

/// Drops every latched input when the window loses focus.
///
/// Alt-Tab can happen while a key or mouse button is held down. The window system then
/// never delivers the matching release, and the next frame would keep reporting a stuck
/// right button as key::CANCEL (or keep a direction held). Clearing the latches here
/// means the first frame after focus loss reads zero from key_bits and Game::mouse_buttons.
fn clear_input_latches(
    held: &mut HashSet<u32>,
    mouse_buttons: &mut u32,
    wheel: &mut u32,
    wheel_dir: &mut i32,
) {
    held.clear();
    *mouse_buttons = 0;
    *wheel = 0;
    *wheel_dir = 0;
}

impl App {
    fn bits(&self) -> u32 {
        key_bits(&self.held, self.wheel, self.mouse_buttons)
    }

    /// Advances one frame: latch input, run the script, rebuild the draw list.
    fn step(&mut self) {
        // A left click is WOLF's decide key: the retail title waits on a `123` poll that
        // answers to either mouse or keyboard, and every message/menu loop confirms on it.
        // Injecting OK here keeps the script layer in charge while letting the player click
        // through the title exactly like the original.
        self.game.mouse = self.mouse;
        self.game.mouse_buttons = self.mouse_buttons;
        let click = self.game.left_click_fresh();
        self.game.key_bits = self.bits() | if click { key::OK } else { 0 };
        self.game.wheel_dir = self.wheel_dir;
        let outcome = self.game.frame();
        // A wheel notch is one key press, not a held key: clear both channels after the
        // frame that saw them so a single event can never drive two frames.
        self.wheel = 0;
        self.wheel_dir = 0;
        if outcome.idle && !self.game.screen.is_empty() {
            // Nothing is running any more: keep presenting the last frame rather than
            // tearing the window down, so the capture stays inspectable.
        }
        self.rebuild();
    }

    /// Rebuilds the draw list: script pictures, host choice panel, message chrome and text.
    ///
    /// Every entry carries an explicit `LayerBand`; `build_scene_layers` sorts the
    /// complete list by `(band, picture_number, sequence)` before either renderer
    /// pass sees it, so host UI cannot drift above or below a script layer by append
    /// order alone.
    fn rebuild(&mut self) {
        let (sprites, texts) = build_scene_layers(&mut self.game, &mut self.atlas);
        self.sprites = sprites;
        self.texts = texts;
        for warning in self.atlas.take_warnings() {
            *self.game.diagnostics.entry(warning).or_insert(0) += 1;
        }
    }

    /// Builds a render instance from an atlas quad, carrying its alpha and its picture tone.
    ///
    /// The tone is per picture, not per frame: a 290 flash changes what one picture
    /// multiplies its texels by while everything else on screen keeps drawing at full
    /// brightness, so it has to ride on the instance rather than on a global draw state.
    fn tint_instance(quad: &wolf_game::atlas::SpriteQuad) -> SpriteInstance {
        SpriteInstance::new(quad.rect, quad.uv)
            .with_alpha(quad.alpha)
            .with_tint_rgb(quad.tint[0], quad.tint[1], quad.tint[2])
    }

    fn upload_if_needed(&mut self, renderer: &mut Renderer) {
        if self.atlas.len() == self.uploaded_images {
            return;
        }
        let [width, height] = self.atlas.size();
        // The atlas is sent to the GPU only when the packed image set changes, so a steady
        // screen costs one instance-buffer write per frame and zero texture uploads.
        if renderer
            .set_atlas(self.atlas.pixels(), width, height)
            .is_ok()
        {
            self.uploaded_images = self.atlas.len();
        }
    }

    fn refresh_title(&self, renderer: &Renderer) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let scaler = renderer.scaler();
        let text_count = self.texts.len();
        window.set_title(&format!(
            "imoto-life | {}x{} -> x{} | frame {} | {} quad(s){} | {} fps",
            LOGICAL_W,
            LOGICAL_H,
            scaler.scale,
            self.game.stats.frames,
            self.sprites.len(),
            if text_count > 0 { ", text pending" } else { "" },
            self.fps_frames
        ));
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attributes = Window::default_attributes()
            .with_title("imoto-life | title screen")
            .with_inner_size(PhysicalSize::new(LOGICAL_W, LOGICAL_H))
            .with_window_icon(window_icon());
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                eprintln!("could not create a window: {err}");
                event_loop.exit();
                return;
            }
        };
        println!(
            "startup: window created in {:.0} ms",
            self.launched_at.elapsed().as_secs_f64() * 1000.0
        );
        // Render the first frame before the window is even shown, so the user never sees
        // an empty (black) frame before the title art appears.
        self.step();
        let mut renderer = match Renderer::new(window.clone()) {
            Ok(renderer) => renderer,
            Err(err) => {
                eprintln!("renderer init failed: {err}");
                event_loop.exit();
                return;
            }
        };
        println!(
            "startup: renderer ready in {:.0} ms",
            self.launched_at.elapsed().as_secs_f64() * 1000.0
        );
        let info = renderer.adapter_info();
        println!(
            "play: {} ({:?}, backend {:?}) | {}x{} logical",
            info.name, info.device_type, info.backend, LOGICAL_W, LOGICAL_H
        );
        self.upload_if_needed(&mut renderer);
        println!(
            "startup: assets uploaded in {:.0} ms",
            self.launched_at.elapsed().as_secs_f64() * 1000.0
        );
        // The original game keeps its TTFs next to Game.exe, and the font covers the
        // Chinese patch text that the bitmap font would render as boxes.
        match crate::find_font(self.game.root()) {
            Some((path, bytes)) => match renderer.set_font(&bytes) {
                Ok(()) => println!("font: {}", path.display()),
                Err(err) => eprintln!("font rejected ({}): {err}", path.display()),
            },
            None => eprintln!("no font found; text will not be drawn"),
        }
        println!(
            "startup: font loaded in {:.0} ms",
            self.launched_at.elapsed().as_secs_f64() * 1000.0
        );
        draw_scene_layers(
            &mut renderer,
            &self.sprites,
            &self.texts,
            self.game.font_px(),
        );
        self.window = Some(window);
        self.renderer = Some(renderer);
        self.window_ready = Some(self.launched_at.elapsed());
        println!(
            "startup: window ready in {:.0} ms",
            self.window_ready.unwrap_or_default().as_secs_f64() * 1000.0
        );
        self.next_frame = Instant::now() + FRAME_TIME;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Focused(false) => {
                // Losing focus means releases may never arrive (Alt-Tab with a key or the
                // right button held). Drop every latch so the next step writes zeros to
                // the game instead of a stuck CANCEL/OK/direction.
                clear_input_latches(
                    &mut self.held,
                    &mut self.mouse_buttons,
                    &mut self.wheel,
                    &mut self.wheel_dir,
                );
                // The engine's copy is refreshed at the start of step(); clearing it here
                // as well keeps a frame that runs before the next step from seeing stale
                // key/mouse/cancel state.
                self.game.key_bits = 0;
                self.game.mouse_buttons = 0;
                self.game.wheel_dir = 0;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let bit = wolf_bit(&event.logical_key);
                if event.state == ElementState::Pressed {
                    if matches!(event.logical_key, Key::Named(NamedKey::F10)) {
                        event_loop.exit();
                        return;
                    }
                    if let Some(bit) = bit {
                        self.held.insert(bit);
                    }
                } else if let Some(bit) = bit {
                    self.held.remove(&bit);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                // winit reports physical pixels; the scaler maps them back onto the
                // 1024x576 grid the game draws in, so the pointer lands where the
                // player sees it even with letterboxing or an integer upscale.
                if let Some(window) = &self.window {
                    self.mouse = game_point(window, position.x, position.y);
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.mouse = [-1, -1];
                self.mouse_buttons = 0;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // The game is mouse-first: a wheel notch is an arrow key for ordinary
                // menus, and the save/load list additionally polls the signed notch.
                // Winit reports either whole lines or physical pixels depending on the
                // platform/driver, so treat any positive/negative delta as one notch and
                // let the next `step()` clear it. The horizontal component is ignored.
                let (bits, notch) = wheel_notch(&delta);
                self.wheel |= bits;
                if notch != 0 {
                    self.wheel_dir = notch;
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let bit = match button {
                    MouseButton::Left => 1,
                    MouseButton::Right => 2,
                    MouseButton::Middle => 4,
                    MouseButton::Back | MouseButton::Forward | MouseButton::Other(_) => 0,
                };
                if bit != 0 {
                    if state == ElementState::Pressed {
                        self.mouse_buttons |= bit;
                    } else {
                        self.mouse_buttons &= !bit;
                    }
                }
            }
            WindowEvent::Resized(size) => {
                // Taking the renderer out of the struct keeps the borrow checker happy while
                // the title bar needs fields from both sides.
                if let Some(mut renderer) = self.renderer.take() {
                    renderer.resize(size);
                    self.refresh_title(&renderer);
                    self.renderer = Some(renderer);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(mut renderer) = self.renderer.take() {
                    self.upload_if_needed(&mut renderer);
                    draw_scene_layers(
                        &mut renderer,
                        &self.sprites,
                        &self.texts,
                        self.game.font_px(),
                    );
                    renderer.present();
                    self.renderer = Some(renderer);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if now >= self.next_frame {
            // Catch up at most a few frames if the machine hiccupped; a long stall should
            // not turn into a burst of fast-forwarded script execution.
            let behind = now.duration_since(self.next_frame);
            self.next_frame += FRAME_TIME;
            if behind > FRAME_TIME * 4 {
                self.next_frame = now + FRAME_TIME;
            }
            let started = Instant::now();
            self.step();
            self.step_times.push(started.elapsed());
            if self
                .frame_limit
                .is_some_and(|limit| self.game.stats.frames >= limit)
            {
                println!(
                    "reached the frame limit after {} frame(s)",
                    self.game.stats.frames
                );
                event_loop.exit();
                return;
            }
            self.fps_frames += 1;
            if self.fps_since.elapsed() >= Duration::from_secs(1) {
                if let Some(renderer) = self.renderer.as_ref() {
                    self.refresh_title(renderer);
                }
                self.fps_frames = 0;
                self.fps_since = Instant::now();
            }
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}

/// Maps one physical key onto WOLF's key bits (crates/wolf-game/src/key.rs).
///
/// Several physical keys share a bit on purpose: the original game honours Enter/Z/Space
/// for confirm and Escape/X for cancel, and a port that only accepted one of them would
/// feel broken to anyone used to the original.
/// Maps a physical window position onto the 1024x576 game grid.
///
/// The renderer blits with the largest whole integer scale and centres the result, so the
/// inverse is `(pos - offset) / scale`. A position on the black bar maps to a negative
/// game coordinate, which is also how the engine reports "outside the screen".
/// p50/p95/max/mean of the measured `step()` times, in milliseconds.
fn step_summary(samples: &[Duration]) -> (f64, f64, f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let mut ms: Vec<f64> = samples.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let percentile = |p: f64| {
        let index = ((ms.len() - 1) as f64 * p).round() as usize;
        ms[index]
    };
    let mean = ms.iter().sum::<f64>() / ms.len() as f64;
    (
        percentile(0.50),
        percentile(0.95),
        *ms.last().unwrap_or(&0.0),
        mean,
    )
}

fn game_point(window: &Window, x: f64, y: f64) -> [i32; 2] {
    let size = window.inner_size();
    let scaler = Scaler::new(size.width.max(1), size.height.max(1));
    let scale = f64::from(scaler.scale.max(1));
    [
        ((x - f64::from(scaler.offset[0])) / scale).floor() as i32,
        ((y - f64::from(scaler.offset[1])) / scale).floor() as i32,
    ]
}

fn wolf_bit(key: &Key) -> Option<u32> {
    match key {
        Key::Named(NamedKey::ArrowUp) => Some(key::UP),
        Key::Named(NamedKey::ArrowDown) => Some(key::DOWN),
        Key::Named(NamedKey::ArrowLeft) => Some(key::LEFT),
        Key::Named(NamedKey::ArrowRight) => Some(key::RIGHT),
        Key::Named(NamedKey::Enter | NamedKey::Space) => Some(key::OK),
        Key::Named(NamedKey::Escape | NamedKey::Backspace) => Some(key::CANCEL),
        Key::Named(NamedKey::Shift | NamedKey::Tab) => Some(key::SUB),
        Key::Character(text) => match text.to_lowercase().as_str() {
            "z" | "y" => Some(key::OK),
            "x" => Some(key::CANCEL),
            "c" => Some(key::SUB),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(bits: &[u32]) -> HashSet<u32> {
        bits.iter().copied().collect()
    }

    #[test]
    fn a_wheel_notch_reports_the_arrow_bit_and_the_signed_direction() {
        // UI-01: the save/load list polls 123 with mask 771 and compares against +1/-1,
        // while ordinary menus keep receiving the UP/DOWN key bits.
        assert_eq!(
            wheel_notch(&MouseScrollDelta::LineDelta(0.0, 1.0)),
            (key::UP, 1)
        );
        assert_eq!(
            wheel_notch(&MouseScrollDelta::LineDelta(0.0, -1.0)),
            (key::DOWN, -1)
        );
    }

    #[test]
    fn pixel_deltas_count_as_one_notch_in_their_direction() {
        let up = MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0.0, 120.0));
        let down = MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0.0, -120.0));
        assert_eq!(wheel_notch(&up), (key::UP, 1));
        assert_eq!(wheel_notch(&down), (key::DOWN, -1));
    }

    #[test]
    fn horizontal_wheel_movement_is_ignored() {
        assert_eq!(wheel_notch(&MouseScrollDelta::LineDelta(3.0, 0.0)), (0, 0));
        let pixels = MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(120.0, 0.0));
        assert_eq!(wheel_notch(&pixels), (0, 0));
    }

    #[test]
    fn ordinary_wheel_still_speaks_the_direction_key_codes() {
        // Regression guard: adding the signed notch must not replace the UP/DOWN bits
        // ordinary menus read through key::code.
        let (up, _) = wheel_notch(&MouseScrollDelta::LineDelta(0.0, 1.0));
        let (down, _) = wheel_notch(&MouseScrollDelta::LineDelta(0.0, -1.0));
        assert_eq!(key::code(key_bits(&HashSet::new(), up, 0)), key::CODE_UP);
        assert_eq!(
            key::code(key_bits(&HashSet::new(), down, 0)),
            key::CODE_DOWN
        );
    }

    #[test]
    fn a_held_right_button_is_cancel_and_never_decide() {
        // UI-02: CE131 closes on key code 11; right click must not inject OK.
        let bits = key_bits(&HashSet::new(), 0, 2);
        assert_ne!(bits & key::CANCEL, 0, "right button sets the cancel bit");
        assert_eq!(
            bits & key::OK,
            0,
            "right button must not set the decide bit"
        );
        assert_eq!(key::code(bits), key::CODE_CANCEL);
    }

    #[test]
    fn a_held_right_button_keeps_other_held_keys() {
        let bits = key_bits(&held(&[key::UP]), 0, 2);
        assert_ne!(bits & key::UP, 0);
        assert_eq!(key::code(bits), key::CODE_CANCEL);
    }

    #[test]
    fn left_and_middle_buttons_do_not_become_cancel() {
        assert_eq!(key_bits(&HashSet::new(), 0, 1) & key::CANCEL, 0);
        assert_eq!(key_bits(&HashSet::new(), 0, 4) & key::CANCEL, 0);
    }

    #[test]
    fn losing_focus_drops_every_latched_input() {
        // Alt-Tab while the right button and a direction are held must not leave a
        // sticky CANCEL behind: the next frame has to read zero from key_bits and
        // mouse_buttons.
        let mut held = held(&[key::CANCEL, key::UP, key::SUB]);
        let mut mouse_buttons = 2;
        let mut wheel = key::DOWN;
        let mut wheel_dir = -1;
        assert_ne!(key_bits(&held, wheel, mouse_buttons), 0);

        clear_input_latches(&mut held, &mut mouse_buttons, &mut wheel, &mut wheel_dir);

        assert!(held.is_empty());
        assert_eq!(mouse_buttons, 0);
        assert_eq!(wheel, 0);
        assert_eq!(wheel_dir, 0);
        let bits = key_bits(&held, wheel, mouse_buttons);
        assert_eq!(bits, 0);
        assert_eq!(key::code(bits), key::CODE_NONE);
    }
}
