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
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use wolf_game::atlas::{self, Atlas, TextQuad};
use wolf_game::{choice, key, Game};
use wolf_render::{Renderer, SpriteInstance, TextItem, LOGICAL_H, LOGICAL_W};

/// One frame at 60 Hz.
const FRAME_TIME: Duration = Duration::from_micros(16_667);

/// Glyph size for text pictures and the runtime choice panel, in logical pixels.
///
/// WOLF's default font is a 24 px bitmap glyph set and the title screens are laid out
/// for it, so anything much smaller renders a menu that looks empty at 1x.
const TEXT_PX: f32 = 22.0;

/// Runs the title screen in a window until the user closes it (or `frames` pass).
///
/// A non-zero frame budget makes the window self-closing, which is how the interactive
/// path gets smoke-tested: the same code that a player drives also has to survive an
/// unattended run without panicking or wedging the event loop.
pub fn run(root: &std::path::Path, frames: Option<u64>, boot: bool) -> std::process::ExitCode {
    let mut game = match Game::new(root) {
        Ok(game) => game,
        Err(err) => {
            eprintln!("cannot load the game data: {err}");
            return std::process::ExitCode::FAILURE;
        }
    };
    // The original boots into the TitleMap map, whose autorun event plays the three
    // publisher/developer splashes, draws the title art and only then calls the menu
    // event 42 (docs/M1_CLOSURE.md). --menu-only skips straight to the menu.
    let started = if boot {
        game.boot().map_err(|err| err.to_string())
    } else {
        game.start_title().map_err(|err| err.to_string())
    };
    if let Err(err) = started {
        eprintln!("{err}");
        return std::process::ExitCode::FAILURE;
    }

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
        window: None,
        renderer: None,
        uploaded_images: usize::MAX,
        frame_limit: frames,
        next_frame: Instant::now(),
        fps_frames: 0,
        fps_since: Instant::now(),
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
    if !app.game.diagnostics.is_empty() {
        println!("diagnostics seen while playing:");
        for (text, count) in app.game.diagnostics.iter().take(10) {
            println!("  {count:>6}x {text}");
        }
    }
    std::process::ExitCode::SUCCESS
}

struct App {
    game: Game,
    atlas: Atlas,
    sprites: Vec<SpriteInstance>,
    texts: Vec<TextQuad>,
    held: HashSet<u32>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    /// Atlas image count at the last upload; a change means new artwork was packed.
    uploaded_images: usize,
    /// Quit after this many frames (None = run until the window closes).
    frame_limit: Option<u64>,
    next_frame: Instant,
    fps_frames: u32,
    fps_since: Instant,
}

impl App {
    fn bits(&self) -> u32 {
        self.held.iter().copied().fold(0, |acc, bit| acc | bit)
    }

    /// Advances one frame: latch input, run the script, rebuild the draw list.
    fn step(&mut self) {
        self.game.key_bits = self.bits();
        let outcome = self.game.frame();
        if outcome.idle && !self.game.screen.is_empty() {
            // Nothing is running any more: keep presenting the last frame rather than
            // tearing the window down, so the capture stays inspectable.
        }
        self.rebuild();
    }

    /// Rebuilds the draw list: script pictures, the runtime choice panel, and text.
    ///
    /// The panel is appended last so it composites over every script layer, which is
    /// what the engine does: a choice prompt is an engine overlay, not a picture slot.
    fn rebuild(&mut self) {
        let quads = self.atlas.sprites(&self.game.screen, &mut self.game.assets);
        let mut sprites: Vec<SpriteInstance> = quads
            .iter()
            .map(|quad| SpriteInstance::new(quad.rect, quad.uv).with_alpha(quad.alpha))
            .collect();
        let mut texts = atlas::texts(&self.game.screen);
        if let Some(choices) = &self.game.choices {
            let options = choices.options.clone();
            let cursor = self.game.choice_cursor();
            let (panel, rows) = choice::overlay(
                &mut self.atlas,
                &mut self.game.assets,
                &options,
                cursor,
            );
            sprites.extend(
                panel
                    .iter()
                    .map(|quad| SpriteInstance::new(quad.rect, quad.uv).with_alpha(quad.alpha)),
            );
            texts.extend(rows);
        }
        self.sprites = sprites;
        self.texts = texts;
    }

    fn build_text_items(&self) -> Vec<TextItem> {
        self.texts
            .iter()
            .map(|text| TextItem::new(text.text.clone(), text.rect, TEXT_PX).with_alpha(text.alpha))
            .collect()
    }

    fn upload_if_needed(&mut self, renderer: &mut Renderer) {
        if self.atlas.len() == self.uploaded_images {
            return;
        }
        let [width, height] = self.atlas.size();
        // The atlas is sent to the GPU only when the packed image set changes, so a steady
        // screen costs one instance-buffer write per frame and zero texture uploads.
        if renderer.set_atlas(self.atlas.pixels(), width, height).is_ok() {
            self.uploaded_images = self.atlas.len();
        }
    }

    fn refresh_title(&self, renderer: &Renderer) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let scaler = renderer.scaler();
        let text_count = atlas::texts(&self.game.screen).len();
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
            .with_inner_size(PhysicalSize::new(1920u32, 1080u32));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                eprintln!("could not create a window: {err}");
                event_loop.exit();
                return;
            }
        };
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
        let info = renderer.adapter_info();
        println!(
            "play: {} ({:?}, backend {:?}) | {}x{} logical",
            info.name, info.device_type, info.backend, LOGICAL_W, LOGICAL_H
        );
        self.upload_if_needed(&mut renderer);
        // The original game keeps its TTFs next to Game.exe, and the font covers the
        // Chinese patch text that the bitmap font would render as boxes.
        match crate::find_font(self.game.root()) {
            Some((path, bytes)) => match renderer.set_font(&bytes) {
                Ok(()) => println!("font: {}", path.display()),
                Err(err) => eprintln!("font rejected ({}): {err}", path.display()),
            },
            None => eprintln!("no font found; text will not be drawn"),
        }
        renderer.draw(&self.sprites);
        let items = self.build_text_items();
        renderer.draw_text(&items);
        self.window = Some(window);
        self.renderer = Some(renderer);
        self.next_frame = Instant::now() + FRAME_TIME;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
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
                    renderer.draw(&self.sprites);
                    let items = self.build_text_items();
                    renderer.draw_text(&items);
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
            self.step();
            if self.frame_limit.is_some_and(|limit| self.game.stats.frames >= limit) {
                println!("reached the frame limit after {} frame(s)", self.game.stats.frames);
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
