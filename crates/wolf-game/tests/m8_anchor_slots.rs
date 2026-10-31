//! Regression matrix for the 150 picture anchors and the long-form slots [2]/[5].
//!
//! Anchors: 0 top-left, 1 centre, 2 bottom-left, 3 top-right, 4 bottom-right,
//! 5 top-centre, 6 bottom-centre; code 7 falls back to top-left.
//!
//! Raw slots: [2] -> args[0] is the fade-in length, [5] -> args[3] is the one-based
//! sprite-sheet cell. Both are already consumed by merge_args; these tests lock the
//! behaviour so H-PIC-2 cannot silently regress.

use wolf_game::picture::{
    Screen, ANCHOR_BOTTOM_CENTRE, ANCHOR_BOTTOM_LEFT, ANCHOR_BOTTOM_RIGHT, ANCHOR_CENTRE,
    ANCHOR_TOP_CENTRE, ANCHOR_TOP_LEFT, ANCHOR_TOP_RIGHT, UNCHANGED,
};
use wolf_vm::{PictureAction, PictureCmd};

/// A draw like the interpreter builds: args start at raw arg[2].
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

fn only_dst(cmd: &PictureCmd, w: u32, h: u32) -> [f32; 4] {
    let mut screen = Screen::new();
    screen.apply(cmd);
    let quads = screen.quads(|_| Some((w, h)));
    assert_eq!(quads.len(), 1, "one picture should produce one quad");
    quads[0].dst
}

#[test]
fn every_anchor_code_places_the_extent_against_its_edges() {
    // (anchor, x, y, w, h, expected dst). The values are the real positions from the
    // shipped scripts: bottom description text at (10,570), right HUD text at x=140,
    // the verified 101 window at (1024,570), 718x700 album art at x=512 and the
    // 482x225 menu icons at y=576.
    let cases: &[(u8, i32, i32, u32, u32, [f32; 4])] = &[
        (
            ANCHOR_TOP_LEFT,
            512,
            520,
            606,
            35,
            [512.0, 520.0, 606.0, 35.0],
        ),
        (
            ANCHOR_CENTRE,
            512,
            520,
            606,
            35,
            [209.0, 502.5, 606.0, 35.0],
        ),
        (
            ANCHOR_BOTTOM_LEFT,
            10,
            570,
            100,
            40,
            [10.0, 530.0, 100.0, 40.0],
        ),
        (ANCHOR_TOP_RIGHT, 140, 7, 100, 40, [40.0, 7.0, 100.0, 40.0]),
        (
            ANCHOR_BOTTOM_RIGHT,
            1024,
            570,
            100,
            40,
            [924.0, 530.0, 100.0, 40.0],
        ),
        (
            ANCHOR_TOP_CENTRE,
            512,
            100,
            718,
            700,
            [153.0, 100.0, 718.0, 700.0],
        ),
        (
            ANCHOR_BOTTOM_CENTRE,
            800,
            576,
            482,
            225,
            [559.0, 351.0, 482.0, 225.0],
        ),
        (7, 100, 50, 20, 10, [100.0, 50.0, 20.0, 10.0]),
    ];
    for &(anchor, x, y, w, h, expected) in cases {
        let cmd = draw(1, 0, "a.png", &[0, 1, 1, 1, 255, x, y, 100, 0], anchor);
        assert_eq!(only_dst(&cmd, w, h), expected, "anchor {anchor}");
    }
}

#[test]
fn a_bottom_centred_icon_stays_on_screen_where_the_old_corner_rule_lost_it() {
    // CE251 draws 482x225 day-menu icons at (800,576) with anchor 6. Top-left would put
    // the picture at y 576..801, entirely below the 576-pixel screen, so quads would
    // drop it; bottom-centre puts it at 351..576.
    let mut screen = Screen::new();
    screen.apply(&draw(
        1210,
        0,
        "1.4.0/SD_tetsudai2.png",
        &[0, 1, 1, 1, 255, 800, 576, 100, 0],
        ANCHOR_BOTTOM_CENTRE,
    ));
    let quads = screen.quads(|_| Some((482, 225)));
    assert_eq!(quads.len(), 1, "the icon must be visible");
    assert_eq!(quads[0].dst, [559.0, 351.0, 482.0, 225.0]);
}

#[test]
fn raw_slot_two_is_the_fade_in_length() {
    let mut screen = Screen::new();
    screen.apply(&draw(
        1,
        0,
        "a.png",
        &[20, 1, 1, 1, 255, 0, 0, 100, 0],
        ANCHOR_TOP_LEFT,
    ));
    let picture = screen.get(1).expect("picture");
    assert_eq!(picture.fade, 20, "slot [2] is the fade length");
    assert_eq!(picture.alpha(), 0.0, "a faded picture starts transparent");

    for _ in 0..5 {
        screen.tick();
    }
    assert!(
        (screen.get(1).expect("picture").alpha() - 0.25).abs() < 1e-6,
        "5 of 20 frames is a quarter opacity"
    );
    for _ in 0..15 {
        screen.tick();
    }
    assert_eq!(screen.get(1).expect("picture").alpha(), 1.0);

    // Slot [2] = 0 means the picture appears at once.
    screen.apply(&draw(
        2,
        0,
        "a.png",
        &[0, 1, 1, 1, 255, 0, 0, 100, 0],
        ANCHOR_TOP_LEFT,
    ));
    assert_eq!(screen.get(2).expect("picture").fade, 0);
    assert_eq!(screen.get(2).expect("picture").alpha(), 1.0);
}

#[test]
fn raw_slot_five_is_the_one_based_sprite_sheet_cell() {
    let mut screen = Screen::new();
    // 6 x 2 sheet, cell 2 -> the second column of the first row.
    screen.apply(&draw(
        7,
        0,
        "stand.png",
        &[0, 6, 2, 2, 255, 0, 0, 100, 0],
        ANCHOR_TOP_LEFT,
    ));
    let picture = screen.get(7).expect("picture");
    assert_eq!(picture.split, (6, 2));
    assert_eq!(picture.pattern_index, 2);
    let quads = screen.quads(|_| Some((2400, 1300)));
    assert_eq!(quads[0].src, [400, 0, 400, 650]);

    // Cell 7 is the first column of the second row.
    screen.apply(&draw(
        8,
        0,
        "stand.png",
        &[0, 6, 2, 7, 255, 0, 0, 100, 0],
        ANCHOR_TOP_LEFT,
    ));
    let quads = screen.quads(|_| Some((2400, 1300)));
    let cell7 = quads
        .iter()
        .find(|quad| quad.number == 8)
        .expect("picture 8");
    assert_eq!(cell7.src, [0, 650, 400, 650]);

    // The UNCHANGED sentinel keeps the default whole-image cell 1.
    screen.apply(&draw(
        9,
        0,
        "stand.png",
        &[0, 6, 2, UNCHANGED, 255, 0, 0, 100, 0],
        ANCHOR_TOP_LEFT,
    ));
    assert_eq!(screen.get(9).expect("picture").pattern_index, 1);
}
