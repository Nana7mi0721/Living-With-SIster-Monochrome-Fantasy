//! M11 regression: the CN title save/load patch stays inside its panel and the screen.
//!
//! docs/research/M9_WINDOW_SKIN.md recorded #12500 at raw (855, 200) with a 300x200
//! box and warned that 855 + 300 = 1155 exceeded the 1024-wide screen. That arithmetic
//! treated (855, 200) as the top-left corner. The shipped command carries anchor code
//! 1 (centre), so Picture::destination places the panel at (705, 100) and its right
//! edge is 1005, not 1155. The patch is therefore a data placement that already fits;
//! there is no clipping bug in window.rs or atlas.rs.
//!
//! These tests run the real save/load screen draw event (CE94) from the shipped
//! work/all data and pin:
//!
//!   * #12500 is a type-4 window at (855, 200), centre anchor, 300x200 box;
//!     its destination is [705, 100, 300, 200] (right 1005, bottom 300).
//!   * every nine-slice panel piece and every real atlas quad for #12500 lies
//!     inside that destination and inside 1024x576;
//!   * #12501 is top-left text at (715, 115); all seven text runs lie inside the
//!     panel and the screen.
//!
//! Capture used as visual evidence:
//!
//!     ./target/debug/wolf-cli.exe --root work/all capture --event 94 --frames 120 \
//!         --out work/out/task87_event94.png
//!
//! The screenshot shows the dark panel's right edge at x=1005 with a 19 px black
//! margin, and the Chinese patch text well inside it.

use std::path::PathBuf;

use wolf_game::atlas::Atlas;
use wolf_game::game::Game;
use wolf_game::picture::{PictureKind, ANCHOR_CENTRE, ANCHOR_TOP_LEFT};
use wolf_game::{picture, window};

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        base.pop();
        base.pop();
        base.join(path)
    }
}

fn open_game() -> Option<Game> {
    Game::new(data_root()).ok()
}

/// Run the real save/load screen event far enough that the CN patch is live and
/// the title/load-screen script has settled on its final frame.
fn run_save_load_screen(game: &mut Game) {
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(game.start_event(94), "CE94 save/load screen starts");
    for _ in 0..120 {
        game.frame();
    }
}

fn assert_inside(rect: [f32; 4], outer: [f32; 4], what: &str) {
    assert!(
        rect[0] >= outer[0] - 0.001
            && rect[1] >= outer[1] - 0.001
            && rect[0] + rect[2] <= outer[0] + outer[2] + 0.001
            && rect[1] + rect[3] <= outer[1] + outer[3] + 0.001,
        "{what} {rect:?} escapes {outer:?}"
    );
}

fn assert_inside_screen(rect: [f32; 4], what: &str) {
    assert!(
        rect[0] >= -0.001
            && rect[1] >= -0.001
            && rect[0] + rect[2] <= picture::SCREEN_W as f32 + 0.001
            && rect[1] + rect[3] <= picture::SCREEN_H as f32 + 0.001,
        "{what} {rect:?} escapes the 1024x576 screen"
    );
}

#[test]
fn the_cn_patch_panel_is_centred_so_its_right_edge_is_1005_not_1155() {
    let Some(mut game) = open_game() else {
        eprintln!("skipping: work/all data root is unavailable");
        return;
    };
    run_save_load_screen(&mut game);

    let panel = game
        .screen
        .get(12500)
        .expect("CE94 leaves the CN patch window #12500 live");
    assert_eq!(panel.kind, PictureKind::WindowString);
    assert_eq!((panel.x, panel.y), (855, 200), "raw picture origin");
    assert_eq!(
        panel.anchor, ANCHOR_CENTRE,
        "the command's anchor nibble is 1"
    );
    assert_eq!(panel.window, Some((300, 200)), "explicit 300x200 box");

    let dst = panel.destination(300.0, 200.0);
    assert_eq!(dst, [705.0, 100.0, 300.0, 200.0]);
    assert_eq!(dst[0] + dst[2], 1005.0, "right edge is 1005, not 1155");
    assert_inside_screen(dst, "panel destination");

    let skin = panel.window_skin().to_owned();
    let (sw, sh) = game
        .assets
        .texture_size(&skin)
        .expect("the patch skin loads from the shipped data");
    assert_eq!((sw, sh), (18, 18), "skin is the measured 18x18 nine-slice");
    let pieces = window::nine_slice(dst, sw, sh);
    assert_eq!(pieces.len(), 9, "a real box always yields nine pieces");
    for (index, piece) in pieces.iter().enumerate() {
        assert_inside(piece.rect, dst, &format!("skin piece {index}"));
        assert_inside_screen(piece.rect, &format!("skin piece {index}"));
    }
}

#[test]
fn the_cn_patch_text_runs_stay_inside_the_panel() {
    let Some(mut game) = open_game() else {
        eprintln!("skipping: work/all data root is unavailable");
        return;
    };
    run_save_load_screen(&mut game);

    let panel = game.screen.get(12500).expect("#12500 panel");
    let panel_dst = panel.destination(300.0, 200.0);

    let text = game.screen.get(12501).expect("#12501 patch text picture");
    assert_eq!(text.kind, PictureKind::Text);
    assert_eq!((text.x, text.y), (715, 115), "raw text origin");
    assert_eq!(
        text.anchor, ANCHOR_TOP_LEFT,
        "text picture is top-left anchored"
    );
    assert_eq!(
        text.window,
        Some((1, 1)),
        "1x1 is the self-sizing sentinel, not a panel box"
    );

    let runs: Vec<_> = game
        .text_quads()
        .into_iter()
        .filter(|quad| quad.number == 12501)
        .collect();
    assert_eq!(runs.len(), 7, "the shipped string lays out as seven lines");
    assert_eq!(runs[0].text, "欢迎来到Ver2.0DLC！");
    assert_eq!(runs[6].text, "重新保存至合适的位置。");

    for (index, run) in runs.iter().enumerate() {
        assert_inside(run.rect, panel_dst, &format!("text run {index}"));
        assert_inside_screen(run.rect, &format!("text run {index}"));
    }
    let rightmost = runs
        .iter()
        .map(|run| run.rect[0] + run.rect[2])
        .fold(f32::MIN, f32::max);
    let bottom = runs
        .iter()
        .map(|run| run.rect[1] + run.rect[3])
        .fold(f32::MIN, f32::max);
    assert!(
        rightmost <= panel_dst[0] + panel_dst[2],
        "rightmost {rightmost}"
    );
    assert!(bottom <= panel_dst[1] + panel_dst[3], "bottom {bottom}");
    assert!(rightmost <= picture::SCREEN_W as f32);
    assert!(bottom <= picture::SCREEN_H as f32);
}

#[test]
fn the_real_atlas_draws_the_cn_patch_panel_with_quads_inside_the_screen() {
    let Some(mut game) = open_game() else {
        eprintln!("skipping: work/all data root is unavailable");
        return;
    };
    run_save_load_screen(&mut game);

    let panel = game.screen.get(12500).expect("#12500 panel");
    let panel_dst = panel.destination(300.0, 200.0);

    let mut atlas = Atlas::new();
    let quads = atlas.sprites(&game.screen, &mut game.assets);
    let panel_quads: Vec<_> = quads.iter().filter(|quad| quad.number == 12500).collect();
    assert_eq!(panel_quads.len(), 9, "the panel is nine atlas quads");
    for (index, quad) in panel_quads.iter().enumerate() {
        assert_inside(quad.rect, panel_dst, &format!("atlas quad {index}"));
        assert_inside_screen(quad.rect, &format!("atlas quad {index}"));
    }
    assert!(
        panel_quads
            .iter()
            .all(|quad| quad.rect[0] + quad.rect[2] <= 1024.0),
        "no atlas quad for the patch crosses x=1024"
    );
}
