//! Pixel-geometry audit for the seven 150 picture anchor codes (task-82).
//!
//! The anchor is the nibble WOLF packs into bits 12..14 of the picture head word.
//! Codes 0..=6 are the seven alignments the editor offers; code 7 never occurs in
//! this game and keeps the top-left fallback.
//!
//! This file is deliberately independent of the representative rectangles in
//! m8_anchor_slots.rs: it sweeps every code across several odd and even extents and
//! several scale percentages (including a negative mirror), then pins the two real
//! artists' positions that first motivated the residual (CE522 top-centre stand art
//! and CE251/CE245 bottom-centre menu icons).

use wolf_game::picture::{
    Picture, Screen, ANCHOR_BOTTOM_CENTRE, ANCHOR_BOTTOM_LEFT, ANCHOR_BOTTOM_RIGHT, ANCHOR_CENTRE,
    ANCHOR_TOP_CENTRE, ANCHOR_TOP_LEFT, ANCHOR_TOP_RIGHT,
};
use wolf_vm::{PictureAction, PictureCmd};

/// A draw command shaped exactly like the interpreter emits one: args start at raw arg 2.
fn draw(number: i32, kind: u8, file: &str, args: &[i32], anchor: u8) -> PictureCmd {
    PictureCmd {
        number,
        kind,
        action: PictureAction::Draw,
        file: file.to_owned(),
        text: String::new(),
        args: args.to_vec(),
        anchor,
        animate: Vec::new(),
    }
}

/// Where Picture::destination puts a w*h extent, with no effects attached.
fn placed(anchor: u8, x: i32, y: i32, scale: i32, w: f32, h: f32) -> [f32; 4] {
    let mut picture = Picture::new(1);
    picture.anchor = anchor;
    picture.x = x;
    picture.y = y;
    picture.scale = scale;
    picture.destination(w, h)
}

/// The fraction of the extent each anchor subtracts from the left and top edges.
///
/// 0 top-left, 1 centre, 2 bottom-left, 3 top-right, 4 bottom-right,
/// 5 top-centre, 6 bottom-centre; 7 is the unused top-left fallback.
fn origin_fractions(anchor: u8) -> (f32, f32) {
    match anchor {
        ANCHOR_CENTRE => (0.5, 0.5),
        ANCHOR_BOTTOM_LEFT => (0.0, 1.0),
        ANCHOR_TOP_RIGHT => (1.0, 0.0),
        ANCHOR_BOTTOM_RIGHT => (1.0, 1.0),
        ANCHOR_TOP_CENTRE => (0.5, 0.0),
        ANCHOR_BOTTOM_CENTRE => (0.5, 1.0),
        _ => (0.0, 0.0),
    }
}

#[test]
fn every_anchor_code_subtracts_the_right_fraction_at_every_size_and_scale() {
    // Odd widths (1, 3, 101) exercise the half-width rounding; 482x225 and 718x700 are
    // the real menu-icon and stand-art extents. 150% and 50% prove the anchor shift uses
    // the SCALED extent, not the raw texture size.
    let anchors: &[u8] = &[
        ANCHOR_TOP_LEFT,
        ANCHOR_CENTRE,
        ANCHOR_BOTTOM_LEFT,
        ANCHOR_TOP_RIGHT,
        ANCHOR_BOTTOM_RIGHT,
        ANCHOR_TOP_CENTRE,
        ANCHOR_BOTTOM_CENTRE,
        7,
    ];
    let widths: &[f32] = &[1.0, 3.0, 7.0, 101.0, 482.0, 718.0];
    let heights: &[f32] = &[1.0, 3.0, 5.0, 35.0, 225.0, 700.0];
    let scales: &[i32] = &[100, 150, 50, -100, -150];

    for &anchor in anchors {
        let (fx, fy) = origin_fractions(anchor);
        for &w in widths {
            for &h in heights {
                for &scale in scales {
                    let got = placed(anchor, 137, 251, scale, w, h);
                    let factor = scale.unsigned_abs() as f32 / 100.0;
                    let extent_w = w * factor;
                    let extent_h = h * factor;
                    let expected = [
                        137.0 - extent_w * fx,
                        251.0 - extent_h * fy,
                        extent_w,
                        extent_h,
                    ];
                    for axis in 0..4 {
                        assert!(
                            (got[axis] - expected[axis]).abs() < 1e-4,
                            "anchor {anchor} {w}x{h} at {scale}%: axis {axis} got {got:?}, expected {expected:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn a_negative_scale_keeps_every_anchors_rectangle_and_only_reverses_the_columns() {
    // Negative percent is a mirror, not a smaller extent: the placed rectangle must be
    // identical to the positive one, while Screen::quads emits one reversed column per
    // source column across that same rectangle.
    for anchor in [
        ANCHOR_TOP_LEFT,
        ANCHOR_CENTRE,
        ANCHOR_BOTTOM_LEFT,
        ANCHOR_TOP_RIGHT,
        ANCHOR_BOTTOM_RIGHT,
        ANCHOR_TOP_CENTRE,
        ANCHOR_BOTTOM_CENTRE,
    ] {
        let mut positive = Picture::new(1);
        positive.anchor = anchor;
        positive.x = 400;
        positive.y = 300;
        positive.scale = 100;
        positive.file = "a.png".to_owned();
        let base = positive.destination(4.0, 2.0);

        let mut negative = positive.clone();
        negative.scale = -100;
        assert!(
            negative.is_mirrored(),
            "anchor {anchor} should report mirrored"
        );
        assert_eq!(
            negative.destination(4.0, 2.0),
            base,
            "anchor {anchor}: the mirror must keep the destination rectangle"
        );

        let mut screen = Screen::new();
        screen.insert(negative);
        let quads = screen.quads(|_| Some((4, 2)));
        assert_eq!(
            quads.len(),
            4,
            "anchor {anchor}: one quad per source column"
        );
        assert_eq!(
            quads[0].src,
            [0, 0, 1, 2],
            "anchor {anchor}: texture column 0 first"
        );
        assert_eq!(
            quads[0].dst,
            [base[0] + base[2] - 1.0, base[1], 1.0, base[3]],
            "anchor {anchor}: column 0 lands at the right edge"
        );
        let last = quads.last().expect("last mirrored column");
        assert_eq!(
            last.src,
            [3, 0, 1, 2],
            "anchor {anchor}: texture column 3 last"
        );
        assert_eq!(
            last.dst,
            [base[0], base[1], 1.0, base[3]],
            "anchor {anchor}: last texture column lands at the left edge"
        );
        for quad in &quads {
            assert!((quad.dst[1] - base[1]).abs() < 1e-4);
            assert!((quad.dst[3] - base[3]).abs() < 1e-4);
        }
    }
}

#[test]
fn real_anchor_five_stand_art_hangs_from_the_top_centre() {
    // CE522 pc157 draws Picture/stand_sub12.png (718x700) at x=512, y=100 with anchor 5.
    // The round-faced stand-in must be horizontally centred and start at y=100.
    let mut screen = Screen::new();
    screen.apply(&draw(
        157,
        0,
        "Picture/stand_sub12.png",
        &[0, 1, 1, 1, 255, 512, 100, 100, 0],
        ANCHOR_TOP_CENTRE,
    ));
    let quads = screen.quads(|_| Some((718, 700)));
    assert_eq!(quads.len(), 1, "anchor 5 art stays on screen");
    assert_eq!(quads[0].dst, [153.0, 100.0, 718.0, 700.0]);
    assert_eq!(screen.get(157).expect("picture").anchor, ANCHOR_TOP_CENTRE);
}

#[test]
fn real_anchor_six_menu_icons_sit_on_the_bottom_edge() {
    // CE251 draws 482x225 day-command icons at (800,576) with anchor 6. Top-left would
    // put them entirely below the 576-pixel screen; bottom-centre lifts them to 351.
    let mut screen = Screen::new();
    screen.apply(&draw(
        1210,
        0,
        "1.4.0/SD_tetsudai2.png",
        &[0, 1, 1, 1, 255, 800, 576, 100, 0],
        ANCHOR_BOTTOM_CENTRE,
    ));
    let quads = screen.quads(|_| Some((482, 225)));
    assert_eq!(quads.len(), 1, "anchor 6 icon is visible");
    assert_eq!(quads[0].dst, [559.0, 351.0, 482.0, 225.0]);
}

#[test]
fn real_ce245_negative_scale_anchor_six_icon_is_a_mirrored_bottom_band() {
    // CE245 pc236/271/... draws pic1220 with anchor 6 at (240,576) and scale -100.
    // Destination is [-1,351,482,225]; the leftmost column is half off-screen, so the
    // on-screen draw list has 481 one-pixel columns, reversed left-to-right.
    let mut screen = Screen::new();
    screen.apply(&draw(
        1220,
        0,
        "1.4.0/SD_menu_013.png",
        &[0, 1, 1, 1, 255, 240, 576, -100, 0],
        ANCHOR_BOTTOM_CENTRE,
    ));
    let picture = screen.get(1220).expect("picture");
    assert!(picture.is_mirrored());
    assert_eq!(
        picture.destination(482.0, 225.0),
        [-1.0, 351.0, 482.0, 225.0]
    );

    let quads = screen.quads(|_| Some((482, 225)));
    assert_eq!(quads.len(), 481, "one column is clipped by the left edge");
    assert_eq!(quads[0].src, [0, 0, 1, 225]);
    assert_eq!(quads[0].dst, [480.0, 351.0, 1.0, 225.0]);
    let last = quads.last().expect("last column");
    assert_eq!(last.src, [480, 0, 1, 225]);
    assert_eq!(last.dst, [0.0, 351.0, 1.0, 225.0]);
    for quad in &quads {
        assert!(
            (quad.src[0] as f32 + quad.dst[0] - 480.0).abs() < 1e-4,
            "source and destination columns must add to the right edge"
        );
        assert_eq!(quad.dst[1], 351.0);
        assert_eq!(quad.dst[3], 225.0);
    }
}
