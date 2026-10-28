//! End-to-end tests for the offscreen pipeline.
//!
//! These need a working adapter, so they *skip* (with a message) instead of
//! failing when none is available: a headless box without a driver should not
//! turn "no GPU" into "the renderer is broken".
//!
//! What is actually being checked:
//!
//! * a sprite really lands where `rect` says, 1:1, inside a 1024x576 frame;
//! * the uv sub-rect really selects the requested texels (atlas left half white,
//!   right half black -- swapping half/whole uv is an easy bug to ship);
//! * 50% alpha over black yields ~188/255, not 128/255. 128 is what a
//!   gamma-space blend would give; 188 is the sRGB encoding of a *linear* 0.5
//!   average. Pinning 188 is the cheapest proof that the target format and the
//!   blend state composite in linear light.

use wolf_render::{Renderer, SpriteInstance, LOGICAL_H, LOGICAL_W};

const ATLAS_W: u32 = 64;
const ATLAS_H: u32 = 64;

/// Left half white, right half black, RGBA8.
fn checkerboard_atlas() -> Vec<u8> {
    let mut atlas = vec![0u8; (ATLAS_W * ATLAS_H * 4) as usize];
    for y in 0..ATLAS_H {
        for x in 0..ATLAS_W / 2 {
            let i = ((y * ATLAS_W + x) * 4) as usize;
            atlas[i] = 255;
            atlas[i + 1] = 255;
            atlas[i + 2] = 255;
            atlas[i + 3] = 255;
        }
    }
    atlas
}

fn offscreen_or_skip() -> Option<Renderer> {
    match Renderer::new_offscreen() {
        Ok(renderer) => Some(renderer),
        Err(err) => {
            eprintln!("skipping GPU test: {err}");
            None
        }
    }
}

fn pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * LOGICAL_W + x) * 4) as usize;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

fn render(renderer: &mut Renderer, sprites: &[SpriteInstance]) -> Vec<u8> {
    renderer.draw(sprites);
    renderer.present();
    renderer.capture_rgba().expect("readback")
}

#[test]
fn frame_is_exactly_1024x576() {
    let Some(mut renderer) = offscreen_or_skip() else {
        return;
    };
    renderer
        .set_atlas(&checkerboard_atlas(), ATLAS_W, ATLAS_H)
        .unwrap();
    let pixels = render(&mut renderer, &[]);
    assert_eq!(pixels.len(), (LOGICAL_W * LOGICAL_H * 4) as usize);
    // Nothing queued -> clear colour, opaque black.
    assert_eq!(pixel(&pixels, 0, 0), [0, 0, 0, 255]);
    assert_eq!(pixel(&pixels, LOGICAL_W - 1, LOGICAL_H - 1), [0, 0, 0, 255]);
}

#[test]
fn sprite_lands_at_its_rect_and_respects_uv() {
    let Some(mut renderer) = offscreen_or_skip() else {
        return;
    };
    renderer
        .set_atlas(&checkerboard_atlas(), ATLAS_W, ATLAS_H)
        .unwrap();

    // Left half of the atlas (white) into the left half of the frame.
    let left = SpriteInstance::new([0.0, 0.0, 512.0, 576.0], [0.0, 0.0, 0.5, 1.0]);
    // Right half of the atlas (black) into the right half of the frame; it is
    // black, so it stays indistinguishable from the clear colour. If uv were
    // wrong this side would light up.
    let right = SpriteInstance::new([512.0, 0.0, 512.0, 576.0], [0.5, 0.0, 1.0, 1.0]);

    let pixels = render(&mut renderer, &[left, right]);
    assert_eq!(pixel(&pixels, 10, 10), [255, 255, 255, 255]);
    assert_eq!(pixel(&pixels, 511, 575), [255, 255, 255, 255]);
    assert_eq!(pixel(&pixels, 512, 0), [0, 0, 0, 255]);
    assert_eq!(pixel(&pixels, 1023, 300), [0, 0, 0, 255]);
}

#[test]
fn half_alpha_blends_in_linear_space() {
    let Some(mut renderer) = offscreen_or_skip() else {
        return;
    };
    renderer
        .set_atlas(&checkerboard_atlas(), ATLAS_W, ATLAS_H)
        .unwrap();

    let quad = SpriteInstance::new(
        [0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32],
        [0.0, 0.0, 0.5, 1.0],
    )
    .with_alpha(0.5);
    let pixels = render(&mut renderer, &[quad]);

    let [r, g, b, a] = pixel(&pixels, 512, 288);
    for channel in [r, g, b] {
        assert!(
            (186..=189).contains(&channel),
            "expected ~188 (linear blend) but got {channel}; 128 would mean gamma-space blending"
        );
    }
    // Straight alpha composited OVER opaque black ends up fully opaque.
    assert_eq!(a, 255);
}

#[test]
fn empty_batch_does_not_replay_the_previous_frame() {
    let Some(mut renderer) = offscreen_or_skip() else {
        return;
    };
    renderer
        .set_atlas(&checkerboard_atlas(), ATLAS_W, ATLAS_H)
        .unwrap();

    let quad = SpriteInstance::new([0.0, 0.0, 64.0, 64.0], [0.0, 0.0, 0.5, 1.0]);
    let first = render(&mut renderer, &[quad]);
    assert_eq!(pixel(&first, 0, 0), [255, 255, 255, 255]);

    let second = render(&mut renderer, &[]);
    assert_eq!(
        pixel(&second, 0, 0),
        [0, 0, 0, 255],
        "`draw(&[])` must clear the queue instead of replaying the old instances"
    );
    assert_eq!(renderer.instance_count(), 0);
}

#[test]
fn instance_buffer_grows_past_its_initial_capacity() {
    let Some(mut renderer) = offscreen_or_skip() else {
        return;
    };
    renderer
        .set_atlas(&checkerboard_atlas(), ATLAS_W, ATLAS_H)
        .unwrap();

    // Initial capacity is 256; 5000 instances force a couple of reallocations
    // and would silently truncate if the growth path were wrong.
    let sprites = vec![SpriteInstance::new([0.0, 0.0, 16.0, 16.0], [0.0, 0.0, 0.25, 0.25]); 5000];
    let pixels = render(&mut renderer, &sprites);
    assert_eq!(renderer.instance_count(), 5000);
    assert_eq!(pixel(&pixels, 8, 8), [255, 255, 255, 255]);
}
