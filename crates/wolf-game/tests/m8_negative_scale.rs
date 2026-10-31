//! M9 W5-b: a negative picture scale is a horizontal mirror, not a hidden picture.
//!
//! Evidence: the official WOLF editor help says a negative zoom flips the image
//! (work/out/yadot_picture.html line 1547), and common event 245 draws its seven
//! 482x225 night-menu reaction pictures with scale -100 at anchor 6 (x=240, y=576).
//! The renderer cannot carry a reversed UV, so Screen::quads emits one column per
//! source pixel in reverse order; these tests pin that mapping and the rectangle
//! that the hit test uses.

use wolf_game::game::Game;
use wolf_game::picture::{Picture, Screen, ANCHOR_BOTTOM_CENTRE, SCALE_ONE};
use wolf_vm::{PictureAction, PictureCmd};

const REACTION_FILE: &str = "1.4.0/SD_menu_010.png";

fn file_picture(number: i32, scale: i32) -> Picture {
    let mut picture = Picture::new(number);
    picture.file = REACTION_FILE.to_owned();
    picture.scale = scale;
    picture
}

fn open_game() -> Option<Game> {
    let root = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".to_owned());
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(root);
    Game::new(&root).ok()
}

/// A mirrored 482x225 reaction icon still occupies the bottom-centre rectangle.
#[test]
fn a_negative_scale_keeps_the_bottom_centre_rectangle_and_mirrors_the_columns() {
    let mut positive = file_picture(900, SCALE_ONE);
    positive.anchor = ANCHOR_BOTTOM_CENTRE;
    positive.x = 240;
    positive.y = 576;
    let mut negative = file_picture(901, -SCALE_ONE);
    negative.anchor = ANCHOR_BOTTOM_CENTRE;
    negative.x = 240;
    negative.y = 576;

    let positive_rect = positive.destination(482.0, 225.0);
    let negative_rect = negative.destination(482.0, 225.0);
    assert_eq!(
        positive_rect, negative_rect,
        "the sign must not move the rectangle"
    );
    assert_eq!(negative_rect, [-1.0, 351.0, 482.0, 225.0]);

    let mut screen = Screen::new();
    screen.insert(negative);
    let quads = screen.quads(|name| (name == REACTION_FILE).then_some((482, 225)));
    assert_eq!(
        quads.len(),
        481,
        "one source column is clipped at the left edge"
    );
    assert_eq!(quads[0].src, [0, 0, 1, 225]);
    assert_eq!(quads[0].dst, [480.0, 351.0, 1.0, 225.0]);
    let last = quads.last().expect("last column");
    assert_eq!(last.src, [480, 0, 1, 225]);
    assert_eq!(last.dst, [0.0, 351.0, 1.0, 225.0]);
    for quad in &quads {
        assert_eq!(quad.src[2], 1, "each quad is one source column");
        assert_eq!(quad.dst[2], 1.0, "each quad is one destination pixel");
        assert_eq!(
            quad.src[0] as f32 + quad.dst[0],
            480.0,
            "column i lands at mirrored position width-1-i"
        );
    }
}

/// The exact PictureCmd shape common event 245 applies to slot 1220.
#[test]
fn the_ce245_reaction_command_draws_the_mirrored_icon_on_screen() {
    let mut screen = Screen::new();
    screen.apply(&PictureCmd {
        number: 1220,
        kind: 0,
        action: PictureAction::Draw,
        file: REACTION_FILE.to_owned(),
        text: String::new(),
        // args start at raw slot 2: [fade, windowW, windowH, cell, opacity, x, y, scale, blend]
        args: vec![0, 0, 0, 1, 255, 240, 576, -100, 0],
        animate: Vec::new(),
        anchor: ANCHOR_BOTTOM_CENTRE,
    });
    let picture = screen.get(1220).expect("picture 1220");
    assert_eq!(
        (picture.x, picture.y, picture.scale, picture.anchor),
        (240, 576, -100, ANCHOR_BOTTOM_CENTRE)
    );
    assert_eq!(
        picture.destination(482.0, 225.0),
        [-1.0, 351.0, 482.0, 225.0]
    );

    let quads = screen.quads(|name| (name == REACTION_FILE).then_some((482, 225)));
    assert_eq!(quads.len(), 481);
    assert_eq!(quads[0].src, [0, 0, 1, 225]);
    assert_eq!(quads[0].dst, [480.0, 351.0, 1.0, 225.0]);
    assert_eq!(quads.last().expect("last").dst, [0.0, 351.0, 1.0, 225.0]);
}

/// A mirrored sprite-sheet picture flips only the selected cell, never the sheet.
#[test]
fn a_mirrored_sprite_sheet_flips_the_selected_cell_only() {
    let mut picture = file_picture(90, -SCALE_ONE);
    picture.file = "Picture/stand_imouto_paja2.png".to_owned();
    picture.split = (6, 2);
    picture.pattern_index = 2;
    picture.x = 0;
    picture.y = 0;
    let mut screen = Screen::new();
    screen.insert(picture);
    let quads =
        screen.quads(|name| (name == "Picture/stand_imouto_paja2.png").then_some((2400, 1300)));
    assert_eq!(quads.len(), 400, "cell 2 is 400 pixels wide");
    assert_eq!(quads[0].src, [400, 0, 1, 650]);
    assert_eq!(quads[0].dst, [399.0, 0.0, 1.0, 650.0]);
    let last = quads.last().expect("last column");
    assert_eq!(last.src, [799, 0, 1, 650]);
    assert_eq!(last.dst, [0.0, 0.0, 1.0, 650.0]);
}

/// The hit test sees the same mirrored rectangle the draw covers.
#[test]
fn the_mirrored_rectangle_is_what_the_hover_test_uses() {
    let Some(mut game) = open_game() else {
        return;
    };
    let mut picture = file_picture(1220, -SCALE_ONE);
    picture.anchor = ANCHOR_BOTTOM_CENTRE;
    picture.x = 240;
    picture.y = 576;
    game.screen.insert(picture);
    game.mouse = [400, 400];
    assert_eq!(game.picture_hover(1220), 1, "inside the mirrored band");
    game.mouse = [600, 400];
    assert_eq!(
        game.picture_hover(1220),
        0,
        "right of the mirrored rectangle"
    );
}

/// Zero still means invisible; only a negative magnitude is a mirror.
#[test]
fn a_zero_scale_remains_invisible() {
    let mut screen = Screen::new();
    screen.insert(file_picture(902, 0));
    assert!(screen
        .quads(|name| (name == REACTION_FILE).then_some((482, 225)))
        .is_empty());
}
