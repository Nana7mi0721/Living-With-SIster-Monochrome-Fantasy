//! M7 W2-b: the H `word_effect` text layer becomes visible once a move's fade time
//! is honoured. See docs/research/M7_HWORD_VISIBLE.md.
//!
//! Every H text common event draws its glyph strip, then immediately "parks" the same
//! layer with a `150 Picture` **move** (cmd 1) that carries the fade time in raw slot 2
//! and *no* animation tail. Common event 350, pc26/pc27:
//!
//!   pc26  op150 [4096,4000,0,1,1,1,255,1600010,1600011,100,0] "hword/word_effect38.png"
//!         -> draw slot 4000, opacity 255, at self[10],self[11]
//!   pc27  op150 [65537,4000,100,0,0,1,0,-10,10,150,0]
//!         -> move slot 4000: raw[2]=100 = 100-frame fade, target opacity 0, scale 150
//!   pc28  op180 [180]   -> wait 180 frames (the 100-frame fade-out must run inside it)
//!
//! Before the fix `Transition::from_command` only read the explicit tail
//! (`cmd.animate`, raw slots 13..). With no tail it returned None, so the move teleported
//! straight to opacity 0 in the same frame the draw happened: not one frame of the glyph
//! strip was ever lit (measured max alpha 0.000). The fix falls back to raw slot 2 (WOLF's
//! picture fade time) as the transition length, and clears the draw-style fade-in ramp so
//! the transition's own opacity interpolation drives alpha.
//!
//! This test pins the visible result: EV350 is scheduled by setting its gate var 118, and
//! slot 4000's `hword/word_effect38.png` alpha climbs to ~0.99 and then interpolates to 0
//! over 100 frames. A guard render of the word layer is written to
//! work/out/m7_hword_visible.png and must contain non-black glyph pixels.

use std::path::{Path, PathBuf};

use wolf_game::atlas::{Atlas, SpriteQuad};
use wolf_game::Game;

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".to_owned());
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        p
    } else {
        repo_root().join(p)
    }
}

fn save_path() -> PathBuf {
    repo_root().join("work/retail_backup/Save/SaveData08.sav")
}

fn out_path() -> PathBuf {
    repo_root().join("work/out/m7_hword_visible.png")
}

fn open_game() -> Option<Game> {
    let mut game = match Game::new(data_root()) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipping m7_hword_visible: cannot open game data ({e})");
            return None;
        }
    };
    game.auto_advance = true;
    game.auto_choice = true;
    if let Err(e) = game.load_save(&save_path()) {
        eprintln!("skipping m7_hword_visible: cannot load retail save ({e})");
        return None;
    }
    game.start_engine_events();
    Some(game)
}

/// Blits the given sprite quads into a 1024x576 RGBA canvas by sampling the packed atlas,
/// writes it as a PNG, and returns the number of non-black pixels written. This is the
/// CPU equivalent of the GPU capture pass (wolf-cli's render path needs a GPU adapter,
/// which CI does not guarantee); it exists only so the doc has a pixel-level artifact.
fn save_quads_png(atlas: &Atlas, quads: &[SpriteQuad], out: &Path) -> usize {
    let [aw, ah] = atlas.size();
    let px = atlas.pixels();
    let w = 1024usize;
    let h = 576usize;
    let mut canvas = vec![0u8; w * h * 4];
    let mut lit = 0usize;
    for q in quads {
        let [x0, y0, qw, qh] = q.rect;
        let [u0, v0, u1, v1] = q.uv;
        let iw = qw.max(1.0);
        let ih = qh.max(1.0);
        let xs = x0.floor().max(0.0) as i32;
        let ys = y0.floor().max(0.0) as i32;
        let xe = (x0 + qw).ceil() as i32;
        let ye = (y0 + qh).ceil() as i32;
        for py in ys..ye {
            for pxx in xs..xe {
                if pxx < 0 || py < 0 || pxx as usize >= w || py as usize >= h {
                    continue;
                }
                let fx = ((pxx as f32 - x0) / iw).clamp(0.0, 0.9999);
                let fy = ((py as f32 - y0) / ih).clamp(0.0, 0.9999);
                let u = u0 + (u1 - u0) * fx;
                let v = v0 + (v1 - v0) * fy;
                let tx = ((u * aw as f32) as i32).clamp(0, aw as i32 - 1);
                let ty = ((v * ah as f32) as i32).clamp(0, ah as i32 - 1);
                let si = (ty as usize * aw as usize + tx as usize) * 4;
                let (sr, sg, sb, sa) = (px[si], px[si + 1], px[si + 2], px[si + 3]);
                let a = (sa as f32 / 255.0) * q.alpha;
                let di = (py as usize * w + pxx as usize) * 4;
                canvas[di] = (sr as f32 * q.tint[0]).clamp(0.0, 255.0) as u8;
                canvas[di + 1] = (sg as f32 * q.tint[1]).clamp(0.0, 255.0) as u8;
                canvas[di + 2] = (sb as f32 * q.tint[2]).clamp(0.0, 255.0) as u8;
                canvas[di + 3] = (a * 255.0).clamp(0.0, 255.0) as u8;
                if canvas[di] | canvas[di + 1] | canvas[di + 2] != 0 {
                    lit += 1;
                }
            }
        }
    }
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    image::save_buffer(out, &canvas, w as u32, h as u32, image::ColorType::Rgba8)
        .expect("write word-layer png");
    lit
}

#[test]
fn h_word_effect_move_fade_is_visible() {
    if open_game().is_none() {
        return;
    }

    // Baseline: with var 118 at its saved value EV350 is not scheduled, so the word layer
    // never draws at all. This is the state the old code was stuck in even *after* gating.
    let mut base = open_game().expect("game");
    let mut base_max = 0.0_f32;
    for _ in 0..120 {
        base.frame();
        if let Some(p) = base.screen.get(4000) {
            base_max = base_max.max(p.alpha());
        }
    }
    assert_eq!(
        base_max, 0.0,
        "EV350 word layer must be dark before its gate var is satisfied"
    );

    // Gated run: schedule EV350 (文字エフェクトみぎ, gate 2000118>=1) and watch slot 4000.
    let mut game = open_game().expect("game");
    game.vm.vars.set(118, 1);
    let mut scheduled = false;
    let mut max_alpha = 0.0_f32;
    let mut max_frame = 0usize;
    let mut word_file = String::new();
    let mut png_lit = 0usize;
    let mut word_quad_count = 0usize;
    let mut frames_lit = 0usize;
    let mut saved = false;
    for f in 0..200usize {
        game.frame();
        if game.auto_events.instance(350).is_some() {
            scheduled = true;
        }
        if let Some(p) = game.screen.get(4000) {
            if p.alpha() > 0.01 {
                frames_lit += 1;
            }
            if p.alpha() > max_alpha {
                max_alpha = p.alpha();
                max_frame = f;
                word_file = p.file.clone();
            }
            if !saved && p.alpha() > 0.5 {
                let mut atlas = Atlas::new();
                let quads = game.picture_quads(&mut atlas);
                let word_quads: Vec<SpriteQuad> = quads
                    .iter()
                    .filter(|q| q.number == 4000)
                    .copied()
                    .collect();
                word_quad_count = word_quads.len();
                png_lit = save_quads_png(&atlas, &word_quads, &out_path());
                saved = true;
            }
        }
    }

    println!(
        "m7_hword_visible: scheduled={scheduled} max_alpha={max_alpha:.3} @frame{max_frame} file={word_file:?} frames_lit={frames_lit} word_quads={word_quad_count} png_non_black={png_lit} -> {}",
        out_path().display()
    );

    assert!(scheduled, "EV350 must be scheduled once var118 >= 1");
    assert!(
        max_alpha > 0.9,
        "word_effect38 alpha must reach >0.9 while the fade is in flight, got {max_alpha}"
    );
    assert!(
        word_file.contains("word_effect"),
        "slot 4000 must be the hword glyph strip, got {word_file:?}"
    );
    assert!(
        frames_lit >= 30,
        "the 100-frame fade should keep the word lit for many frames, got {frames_lit}"
    );
    assert!(word_quad_count > 0, "slot 4000 must produce a sprite quad");
    assert!(
        png_lit > 200,
        "the rendered word layer must contain non-black glyph pixels, got {png_lit}"
    );
    assert!(out_path().is_file(), "capture png must be written");
}
