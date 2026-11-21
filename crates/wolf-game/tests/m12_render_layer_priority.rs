//! M12 render regressions: semantic layer bands for host and script UI.
//!
//! UI-13 asks for one composition order across the whole frame:
//!
//!   选择弹窗 > 消息框 > 场景 UI > 地图/背景
//!
//! WOLF script pictures carry only a signed number, and the old ascending-number sort
//! mixed host panels into the scene. window::LayerBand turns the retail order into named
//! ranks (Map=0 .. Cursor=40). atlas::Atlas::sprites now sorts its script quads by
//! (band rank, picture number), so two script pictures cannot invert the semantic order
//! just because a panel happens to have a larger number than the content behind it.
//!
//! These tests pin the public order and the one collision that had to be removed:
//! window::PANEL_PICTURE used to be -100, exactly TITLE_BACKDROP_LAYER, so a host panel
//! sorting by number could be mistaken for the title backdrop (or vice versa). It is now
//! a trace-only sentinel well below every real negative layer.

use wolf_game::assets::Assets;
use wolf_game::atlas::Atlas;
use wolf_game::picture::{Screen, ANCHOR_TOP_LEFT, TITLE_BACKDROP_LAYER};
use wolf_game::window::{self, LayerBand};
use wolf_vm::exec::PictureAction;
use wolf_vm::PictureCmd;

fn picture(number: i32, file: &str, args: &[i32]) -> PictureCmd {
    PictureCmd {
        number,
        kind: 3,
        action: PictureAction::Draw,
        file: file.to_owned(),
        text: String::new(),
        args: args.to_vec(),
        animate: Vec::new(),
        anchor: ANCHOR_TOP_LEFT,
    }
}

#[test]
fn layer_bands_match_the_retail_composition_order() {
    let order = [
        LayerBand::Map,
        LayerBand::ScenePicture,
        LayerBand::SceneText,
        LayerBand::MessageFrame,
        LayerBand::MessageText,
        LayerBand::Nameplate,
        LayerBand::ChoicePanel,
        LayerBand::ChoiceText,
        LayerBand::Cursor,
    ];
    for pair in order.windows(2) {
        assert!(
            pair[0].rank() < pair[1].rank(),
            "{:?} ({}) must stay below {:?} ({})",
            pair[0],
            pair[0].rank(),
            pair[1],
            pair[1].rank()
        );
    }
    assert_eq!(
        LayerBand::Map.rank(),
        0,
        "map/background is the bottom band"
    );
    assert_eq!(LayerBand::Cursor.rank(), 40, "the cursor is always on top");
    assert!(
        LayerBand::MessageFrame.rank() < LayerBand::ChoicePanel.rank(),
        "a choice popup must cover the message frame"
    );
}

#[test]
fn panel_sentinel_does_not_collide_with_the_title_backdrop() {
    const {
        assert!(window::PANEL_PICTURE < TITLE_BACKDROP_LAYER);
        assert!(window::PANEL_PICTURE < -2);
    }
    assert_ne!(
        window::PANEL_PICTURE,
        TITLE_BACKDROP_LAYER,
        "a host panel sentinel must never equal a real script layer"
    );
}

#[test]
fn title_backdrop_constants_agree() {
    // game::TITLE_BACKDROP builds the backdrop; picture::TITLE_BACKDROP_LAYER protects it
    // from clear_screen_layers. If the two ever drift, the protection silently targets the
    // wrong slot and UI-05 can come back.
    assert_eq!(
        wolf_game::game::TITLE_BACKDROP,
        wolf_game::picture::TITLE_BACKDROP_LAYER,
        "the title backdrop slot must be the same value in game.rs and picture.rs"
    );
}

#[test]
fn picture_band_marks_the_message_frame_slots_as_message_frame() {
    // Common event 63 draws the message frame into 19500..19503. It is a script picture,
    // but it is engine message chrome and must not sort with room art or character poses.
    let frame = wolf_game::message::FRAME_PICTURE;
    for number in frame..=frame + 3 {
        assert_eq!(
            window::picture_band(number),
            LayerBand::MessageFrame,
            "slot {number}"
        );
    }
    assert_eq!(window::picture_band(frame - 1), LayerBand::ScenePicture);
    assert_eq!(window::picture_band(frame + 4), LayerBand::ScenePicture);
}

#[test]
fn picture_band_separates_whole_screen_layers_from_scene_pictures() {
    for number in [-100, -2, -1] {
        assert_eq!(
            window::picture_band(number),
            LayerBand::Map,
            "negative full-screen layer {number} belongs to the map band"
        );
    }
    for number in [0, 10, 1600, 50000] {
        assert_eq!(
            window::picture_band(number),
            LayerBand::ScenePicture,
            "positive picture {number} belongs to the scene band"
        );
    }
}

#[test]
fn atlas_orders_the_title_backdrop_before_positive_scene_pictures() {
    let mut screen = Screen::new();
    // -100 is the title backdrop, -2 the title art; both are whole-screen layers.
    screen.apply(&picture(
        -100,
        "<SQUARE>",
        &[0, 1024, 576, 1, 255, 0, 0, 100, 0],
    ));
    screen.apply(&picture(
        -2,
        "<SQUARE>",
        &[0, 1024, 576, 1, 255, 0, 0, 100, 0],
    ));
    // A scene picture with a smaller number must still draw above the backdrop.
    screen.apply(&picture(
        10,
        "<SQUARE>",
        &[0, 100, 80, 1, 255, 700, 100, 100, 0],
    ));
    screen.apply(&picture(
        500,
        "<SQUARE>",
        &[0, 100, 80, 1, 255, 800, 100, 100, 0],
    ));

    let mut atlas = Atlas::with_size(1024, 1024);
    let mut assets = Assets::new("no-such-root");
    let quads = atlas.sprites(&screen, &mut assets);
    let numbers: Vec<i32> = quads.iter().map(|quad| quad.number).collect();
    assert_eq!(
        numbers,
        vec![-100, -2, 10, 500],
        "whole-screen layers first, then scene pictures by number"
    );
}
