//! M9 hover hit-testing: the opcode 124 field 8 probe must use the picture's real
//! destination rectangle, not a centre-only special case.
//!
//! WOLF pictures carry an anchor code in bits 12..14 of the op150 head word. The renderer
//! honours seven codes (top-left, centre, bottom-left, top-right, bottom-right, top-centre,
//! bottom-centre). The headless hover used to subtract half width/height only for the
//! centre code, so a bottom-centre icon anchored at the screen bottom was hit-tested below
//! the visible band and every other anchor behaved like top-left. These tests pin the
//! corrected geometry and prove a real day-command menu icon is still clickable.

use std::path::PathBuf;
use wolf_game::picture::{
    Picture, PictureKind, ANCHOR_BOTTOM_CENTRE, ANCHOR_BOTTOM_LEFT, ANCHOR_BOTTOM_RIGHT,
    ANCHOR_CENTRE, ANCHOR_TOP_CENTRE, ANCHOR_TOP_LEFT, ANCHOR_TOP_RIGHT,
};
use wolf_game::Game;

fn repo_root() -> PathBuf {
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop();
    b.pop();
    b
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".to_owned());
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        return p;
    }
    repo_root().join(p)
}

fn open_game() -> Option<Game> {
    match Game::new(data_root()) {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("skip: cannot open data root {:?}: {e}", data_root());
            None
        }
    }
}

/// A window-kind picture with a known size, anchor and position. It needs no disk texture,
/// so the hit box is fully controlled by the test.
fn synthetic(number: i32, anchor: u8, x: i32, y: i32, w: i32, h: i32) -> Picture {
    let mut p = Picture::new(number);
    p.kind = PictureKind::WindowFile;
    p.window = Some((w, h));
    p.x = x;
    p.y = y;
    p.anchor = anchor;
    p
}

fn assert_hit(game: &mut Game, number: i32, x: i32, y: i32, want: i32, what: &str) {
    game.mouse = [x, y];
    let got = game.picture_hover(number);
    assert_eq!(got, want, "{what} at ({x},{y})");
}

#[test]
fn every_anchor_code_hit_tests_the_drawn_rectangle() {
    let Some(mut game) = open_game() else { return };
    let number = 900;
    // anchor, x, y, destination rect the renderer draws, a point inside, a point outside.
    let cases = [
        (
            ANCHOR_TOP_LEFT,
            100,
            100,
            [100.0, 100.0, 100.0, 40.0],
            [150, 120],
            [99, 120],
        ),
        (
            ANCHOR_CENTRE,
            512,
            288,
            [462.0, 268.0, 100.0, 40.0],
            [512, 288],
            [461, 288],
        ),
        (
            ANCHOR_BOTTOM_LEFT,
            100,
            200,
            [100.0, 160.0, 100.0, 40.0],
            [150, 180],
            [150, 159],
        ),
        (
            ANCHOR_TOP_RIGHT,
            900,
            100,
            [800.0, 100.0, 100.0, 40.0],
            [850, 120],
            [901, 120],
        ),
        (
            ANCHOR_BOTTOM_RIGHT,
            900,
            200,
            [800.0, 160.0, 100.0, 40.0],
            [850, 180],
            [799, 180],
        ),
        (
            ANCHOR_TOP_CENTRE,
            512,
            100,
            [462.0, 100.0, 100.0, 40.0],
            [512, 120],
            [461, 120],
        ),
        (
            ANCHOR_BOTTOM_CENTRE,
            512,
            400,
            [462.0, 360.0, 100.0, 40.0],
            [512, 380],
            [512, 359],
        ),
    ];
    for (anchor, x, y, dst, inside, outside) in cases {
        game.screen.insert(synthetic(number, anchor, x, y, 100, 40));
        let drawn = game
            .screen
            .get(number)
            .expect("picture inserted")
            .destination(100.0, 40.0);
        assert_eq!(drawn, dst, "anchor {anchor} destination");
        assert_hit(
            &mut game,
            number,
            inside[0],
            inside[1],
            1,
            &format!("anchor {anchor} inside"),
        );
        assert_hit(
            &mut game,
            number,
            outside[0],
            outside[1],
            0,
            &format!("anchor {anchor} outside"),
        );
    }
}

#[test]
fn a_bottom_centred_icon_at_the_screen_bottom_is_clickable_in_its_visible_band() {
    let Some(mut game) = open_game() else { return };
    let number = 901;
    // CE245's night menu draws 482x225 icons at y=576 with the bottom-centre anchor.
    game.screen
        .insert(synthetic(number, ANCHOR_BOTTOM_CENTRE, 800, 576, 482, 225));
    // Drawn top is 576-225=351, so the visible band is y in 351..576.
    assert_hit(&mut game, number, 800, 576 - 10, 1, "near the bottom edge");
    assert_hit(
        &mut game,
        number,
        800,
        351 + 10,
        1,
        "near the top of the visible band",
    );
    assert_hit(&mut game, number, 800, 350, 0, "just above the drawn band");
    // The old corner rule put the whole icon at y>=576, so it had no visible hit area at all.
    let drawn_top = game.screen.get(number).unwrap().destination(482.0, 225.0)[1];
    assert!(
        drawn_top < 576.0,
        "bottom-centre draw starts above the screen bottom"
    );
}

#[test]
fn a_top_right_hud_picture_hit_tests_against_the_right_edge() {
    let Some(mut game) = open_game() else { return };
    let number = 902;
    game.screen
        .insert(synthetic(number, ANCHOR_TOP_RIGHT, 1000, 7, 200, 40));
    // Drawn left is 1000-200=800; the right edge is the anchor x.
    assert_hit(&mut game, number, 800, 20, 1, "left edge inside");
    assert_hit(&mut game, number, 999, 20, 1, "right before the edge");
    assert_hit(
        &mut game,
        number,
        1000,
        20,
        0,
        "on the right edge is outside",
    );
    assert_hit(&mut game, number, 799, 20, 0, "left of the drawn box");
}

#[test]
fn a_sprite_sheet_cell_is_hit_tested_at_cell_size_not_sheet_size() {
    let Some(mut game) = open_game() else { return };
    let number = 903;
    // A real skin texture supplies a real texture size without a fake asset entry.
    let Some(full) = game.assets.texture_size("SystemFile/WindowBase.png") else {
        return;
    };
    let mut p = Picture::new(number);
    p.kind = PictureKind::File;
    p.file = "SystemFile/WindowBase.png".to_owned();
    p.split = (2, 1);
    p.anchor = ANCHOR_TOP_LEFT;
    p.x = 100;
    p.y = 100;
    game.screen.insert(p);
    let cell_w = (full.0 / 2) as f32;
    let cell_h = full.1 as f32;
    let dst = game.screen.get(number).unwrap().destination(cell_w, cell_h);
    assert_eq!(dst[2], cell_w, "the cell width is half the sheet");
    assert_hit(&mut game, number, 101, 101, 1, "inside the first cell");
    // A point past the first cell but still inside the full sheet is not inside the cell.
    if cell_w + 100.0 < 1024.0 && full.0 as f32 > cell_w + 1.0 {
        assert_hit(
            &mut game,
            number,
            (100.0 + cell_w + 1.0) as i32,
            101,
            0,
            "past the cell",
        );
    }
}

#[test]
fn a_real_day_command_menu_icon_is_still_clickable() {
    let Some(mut game) = open_game() else { return };
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    assert!(game.start_event(251), "start day command menu event 251");
    for _ in 0..120 {
        game.frame();
    }
    // CE251 draws the three day-command icons 1200/1202/1204 at 512 with y=230/280/330,
    // all centre-anchored. The top icon must still be clickable at its centre and must not
    // claim the row below.
    let drawn: Vec<_> = game
        .screen
        .iter()
        .map(|p| (p.number, p.kind, p.anchor, p.x, p.y))
        .collect();
    assert!(
        drawn
            .iter()
            .any(|(n, _, a, x, y)| *n == 1200 && *a == ANCHOR_CENTRE && *x == 512 && *y == 230),
        "icon 1200: {drawn:?}"
    );
    assert_hit(&mut game, 1200, 512, 230, 1, "day command icon 1200");
    assert_hit(&mut game, 1202, 512, 280, 1, "day command icon 1202");
    assert_hit(
        &mut game,
        1200,
        512,
        280,
        0,
        "icon 1200 must not cover the next row",
    );
    assert_eq!(game.stats.warnings, 0, "warnings: {:?}", game.diagnostics);
}
