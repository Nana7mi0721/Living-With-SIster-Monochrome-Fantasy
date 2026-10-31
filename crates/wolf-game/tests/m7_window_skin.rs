//! M7 window skin regression: the default window frame is the grey
//! SystemFile/WindowBase.png - a real 18x18 nine-slice - and the 960x168 message box
//! is covered by it with the documented 3 px frame.
//!
//! This turns the UNVERIFIED note H-WIN-1 into a checked fact. H-WIN-1 also guessed the
//! frame came from a *navy gradient*; the actual SystemFile/WindowBase.png is grey (its
//! body pixel is 0x1f2425 with alpha 0xdf), while the navy image is a different file
//! (SystemGraphic/WindowBase.png) that the game does not use. The tests below pin both
//! facts so a future change cannot quietly swap the skin back to the navy sample.

use std::path::PathBuf;

use wolf_game::picture::{Picture, PictureKind, ANCHOR_CENTRE};
use wolf_game::window;

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/full".to_owned());
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        repo_root().join(path)
    }
}

fn pixel(image: &image::RgbaImage, x: u32, y: u32) -> [u8; 4] {
    image.get_pixel(x, y).0
}

#[test]
fn the_default_skin_is_a_grey_18px_asset() {
    let path = data_root().join("SystemFile").join("WindowBase.png");
    if !path.is_file() {
        eprintln!("skipping: {} is missing", path.display());
        return;
    }
    let image = image::open(&path)
        .expect("decode SystemFile/WindowBase.png")
        .to_rgba8();
    assert_eq!(image.dimensions(), (18, 18), "the skin must be an 18x18 nine-slice");
    // Outer corner is fully transparent, then the light bevel, then the transition line:
    // the three rows/cols the 3 px frame cuts.
    assert_eq!(pixel(&image, 0, 0), [255, 255, 255, 0], "outer corner is transparent");
    assert_eq!(pixel(&image, 1, 1), [136, 136, 136, 255], "1 px is the light bevel 0x888888");
    assert_eq!(pixel(&image, 2, 2), [217, 220, 221, 255], "2 px is the transition line 0xd9dcdd");
    // The body the nine-slice stretches: translucent grey, identical in a corner and at
    // the very centre.
    assert_eq!(pixel(&image, 5, 5), [31, 36, 37, 223], "body is the translucent grey 0x1f2425/0xdf");
    assert_eq!(pixel(&image, 9, 9), [31, 36, 37, 223], "the centre of the body is the same grey");
}

#[test]
fn the_navy_sample_asset_is_not_the_default_skin() {
    // The engine constant and the DB constant must agree, and neither may name the navy
    // sample that H-WIN-1 suspected.
    assert_eq!(window::SKIN, "SystemFile/WindowBase.png");
    assert_eq!(wolf_game::db::WINDOW_SKIN_FILE, window::SKIN);
    assert_ne!(
        window::SKIN, "SystemGraphic/WindowBase.png",
        "the navy SystemGraphic sample must not be the default window skin"
    );

    let path = data_root().join("SystemGraphic").join("WindowBase.png");
    if !path.is_file() {
        eprintln!("skipping navy-pixel check: {} is missing", path.display());
        return;
    }
    let image = image::open(&path)
        .expect("decode SystemGraphic/WindowBase.png")
        .to_rgba8();
    assert_eq!(image.dimensions(), (18, 18));
    let [red, green, blue, _] = pixel(&image, 5, 5);
    assert!(
        blue > red && blue > green && blue > 60,
        "SystemGraphic/WindowBase.png is the navy sample (r={red} g={green} b={blue}), not the grey default"
    );
}

#[test]
fn the_message_window_box_is_covered_by_the_18px_nine_slice() {
    // Real geometry: common event 63 draws a WindowFile picture whose box is 960x168,
    // centred at (512,480). Picture::destination turns that into the screen-space box
    // [32,396,960,168] (see picture.rs a_hidden_window_keeps_its_skin_and_box).
    let mut picture = Picture::new(19500);
    picture.kind = PictureKind::WindowFile;
    picture.file = window::SKIN.to_owned();
    picture.window = Some((960, 168));
    picture.x = 512;
    picture.y = 480;
    picture.anchor = ANCHOR_CENTRE;
    let dst = picture.destination(960.0, 168.0);
    assert_eq!(dst, [32.0, 396.0, 960.0, 168.0], "message box destination");

    let skin = data_root().join("SystemFile").join("WindowBase.png");
    if !skin.is_file() {
        eprintln!("skipping: {} is missing", skin.display());
        return;
    }
    let image = image::open(&skin).expect("decode skin").to_rgba8();
    let (skin_w, skin_h) = image.dimensions();

    let pieces = window::nine_slice(dst, skin_w, skin_h);
    assert_eq!(pieces.len(), 9, "an 18x18 skin yields the full nine pieces");

    // 3 px frame, stretched body: the centre is 960-6 = 954 wide and 168-6 = 162 tall.
    assert_eq!(pieces[0].rect, [32.0, 396.0, 3.0, 3.0], "top-left corner");
    assert_eq!(pieces[4].rect, [35.0, 399.0, 954.0, 162.0], "stretched centre");
    assert_eq!(pieces[8].rect, [989.0, 561.0, 3.0, 3.0], "bottom-right corner");

    // Exact coverage: the nine pieces tile the box with no gap and no overlap.
    let dst_area: f32 = pieces.iter().map(|p| p.rect[2] * p.rect[3]).sum();
    assert!(
        (dst_area - 960.0 * 168.0).abs() < 1e-3,
        "the nine pieces cover the box exactly, area {dst_area}"
    );
    for piece in &pieces {
        assert!(piece.rect[0] >= 32.0 && piece.rect[1] >= 396.0, "piece escapes the box: {:?}", piece.rect);
        assert!(
            piece.rect[0] + piece.rect[2] <= 992.0 + 1e-3 && piece.rect[1] + piece.rect[3] <= 564.0 + 1e-3,
            "piece overflows the box: {:?}",
            piece.rect
        );
    }

    // The whole 18x18 skin is used exactly once across the nine source rectangles.
    let src_area: u32 = pieces.iter().map(|p| p.src[2] * p.src[3]).sum();
    assert_eq!(src_area, skin_w * skin_h, "the nine pieces consume every skin texel once");
    assert_eq!(src_area, 18 * 18);
}
