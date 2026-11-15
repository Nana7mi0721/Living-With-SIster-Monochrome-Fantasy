//! M12 UI layer-composition tests: UI-13 host bands.
//!
//! `wolf-cli` is a binary-only crate, so a normal integration test cannot reach the
//! private display-list types in `src/play.rs`. Including that file as a module compiles
//! the exact `build_scene_layers`/sort helpers the running `App` uses. The three stubs
//! below stand in for the `crate::` items `play::run` mentions; these tests never call
//! `run`, so they stay inert.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use wolf_game::atlas::{Atlas, SpriteQuad, TextQuad};
use wolf_game::window::{self, LayerBand};
use wolf_game::Game;

/// Mirror of the private `Entry` enum in `src/main.rs`.
enum Entry<'a> {
    Boot,
    Menu,
    Script { map: &'a str, event: u32 },
    Save(&'a Path),
    CommonEvent(u32),
}

fn install_font_metrics(_game: &mut Game, _root: &Path) {}

fn find_font(_root: &Path) -> Option<(PathBuf, Vec<u8>)> {
    None
}

#[path = "../src/play.rs"]
mod play;

fn sprite(number: i32) -> SpriteQuad {
    SpriteQuad {
        number,
        rect: [0.0, 0.0, 10.0, 10.0],
        uv: [0.0, 0.0, 0.1, 0.1],
        alpha: 1.0,
        tint: [1.0, 1.0, 1.0],
    }
}

fn text(number: i32, text: &str) -> TextQuad {
    TextQuad {
        number,
        text: text.to_owned(),
        rect: [0.0, 0.0, 10.0, 10.0],
        alpha: 1.0,
        font_px: 16.0,
        colour: [255, 255, 255],
        outline: false,
    }
}

fn layered_sprite(band: LayerBand, number: i32, seq: usize) -> play::LayeredSprite {
    play::LayeredSprite {
        band,
        number,
        seq,
        quad: sprite(number),
    }
}

fn layered_text(band: LayerBand, number: i32, seq: usize, body: &str) -> play::LayeredText {
    play::LayeredText {
        band,
        number,
        seq,
        quad: text(number, body),
    }
}

#[test]
fn choice_panel_sorts_above_nameplate_and_every_scene_layer() {
    let mut sprites = vec![
        layered_sprite(LayerBand::Cursor, -100_000, 5),
        layered_sprite(LayerBand::ChoicePanel, window::PANEL_PICTURE, 4),
        layered_sprite(LayerBand::Nameplate, window::PANEL_PICTURE, 3),
        layered_sprite(LayerBand::MessageFrame, 19_500, 2),
        layered_sprite(LayerBand::ScenePicture, 500, 1),
        layered_sprite(LayerBand::Map, -10_000, 0),
    ];
    play::sort_layered_sprites(&mut sprites);
    let bands: Vec<LayerBand> = sprites.iter().map(|entry| entry.band).collect();
    assert_eq!(
        bands,
        vec![
            LayerBand::Map,
            LayerBand::ScenePicture,
            LayerBand::MessageFrame,
            LayerBand::Nameplate,
            LayerBand::ChoicePanel,
            LayerBand::Cursor,
        ]
    );
    let choice = bands
        .iter()
        .position(|band| *band == LayerBand::ChoicePanel)
        .expect("choice panel present");
    let nameplate = bands
        .iter()
        .position(|band| *band == LayerBand::Nameplate)
        .expect("nameplate present");
    assert!(choice > nameplate, "choice panel must cover the nameplate");
}

#[test]
fn message_text_sorts_below_choice_text_but_above_scene_text() {
    let mut texts = vec![
        layered_text(LayerBand::ChoiceText, window::PANEL_PICTURE, 4, "choice"),
        layered_text(LayerBand::Nameplate, window::PANEL_PICTURE, 3, "name"),
        layered_text(LayerBand::MessageText, 19_500, 2, "message later"),
        layered_text(LayerBand::MessageText, 19_500, 1, "message first"),
        layered_text(LayerBand::SceneText, 1601, 0, "scene"),
    ];
    play::sort_layered_texts(&mut texts);
    let bands: Vec<LayerBand> = texts.iter().map(|entry| entry.band).collect();
    assert_eq!(
        bands,
        vec![
            LayerBand::SceneText,
            LayerBand::MessageText,
            LayerBand::MessageText,
            LayerBand::Nameplate,
            LayerBand::ChoiceText,
        ]
    );
    // Same band, same picture number: the sequence decides, so the message's own
    // styled runs keep their layout order instead of being reordered by the sort.
    assert_eq!(texts[1].seq, 1);
    assert_eq!(texts[2].seq, 2);
    assert_eq!(texts[1].quad.text, "message first");
    assert_eq!(texts[2].quad.text, "message later");
}

#[test]
fn scene_bands_merge_each_present_band_once_in_rank_order() {
    let sprites = vec![
        layered_sprite(LayerBand::Cursor, -100_000, 5),
        layered_sprite(LayerBand::Map, -10_000, 0),
        layered_sprite(LayerBand::MessageFrame, 19_500, 2),
        layered_sprite(LayerBand::ScenePicture, 500, 1),
        layered_sprite(LayerBand::ChoicePanel, window::PANEL_PICTURE, 6),
    ];
    let texts = vec![
        layered_text(LayerBand::ChoiceText, window::PANEL_PICTURE, 7, "choice"),
        layered_text(LayerBand::MessageText, 19_500, 3, "message"),
        layered_text(LayerBand::Nameplate, window::PANEL_PICTURE, 4, "name"),
        layered_text(LayerBand::SceneText, 1601, 2, "scene"),
    ];
    assert_eq!(
        play::scene_bands(&sprites, &texts),
        vec![
            LayerBand::Map,
            LayerBand::ScenePicture,
            LayerBand::SceneText,
            LayerBand::MessageFrame,
            LayerBand::MessageText,
            LayerBand::Nameplate,
            LayerBand::ChoicePanel,
            LayerBand::ChoiceText,
            LayerBand::Cursor,
        ]
    );
    assert_eq!(play::scene_bands(&[], &[]), vec![LayerBand::Map]);
}

#[test]
fn scene_pictures_in_the_same_band_keep_picture_number_order() {
    let mut sprites = vec![
        layered_sprite(LayerBand::ScenePicture, 500, 0),
        layered_sprite(LayerBand::ScenePicture, 10, 1),
    ];
    play::sort_layered_sprites(&mut sprites);
    assert_eq!(sprites[0].number, 10);
    assert_eq!(sprites[1].number, 500);
}

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates/wolf-cli -> crates
    path.pop(); // crates -> repository root
    path
}

fn data_root() -> PathBuf {
    std::env::var_os("WOLF_DATA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("work").join("all"))
}

fn save_path() -> PathBuf {
    repo_root()
        .join("work")
        .join("retail_backup")
        .join("Save")
        .join("SaveData07.sav")
}

#[test]
fn real_save_display_list_is_sorted_by_band_then_picture_number() {
    let mut game = Game::new(data_root()).expect("work/all game data loads");
    game.load_save(&save_path()).expect("SaveData07 loads");
    let mut atlas = Atlas::new();
    let (sprites, texts) = play::build_scene_layers(&mut game, &mut atlas);
    assert!(!sprites.is_empty(), "the room scene has textured quads");

    let sprite_keys: Vec<(u32, i32)> = sprites
        .iter()
        .map(|entry| (entry.band.rank(), entry.number))
        .collect();
    let mut sorted_sprite_keys = sprite_keys.clone();
    sorted_sprite_keys.sort();
    assert_eq!(
        sprite_keys, sorted_sprite_keys,
        "sprites must be submitted in (band, picture number) order"
    );

    let text_keys: Vec<(u32, i32)> = texts
        .iter()
        .map(|entry| (entry.band.rank(), entry.number))
        .collect();
    let mut sorted_text_keys = text_keys.clone();
    sorted_text_keys.sort();
    assert_eq!(
        text_keys, sorted_text_keys,
        "text must be submitted in (band, picture number) order"
    );
    assert!(
        texts
            .iter()
            .any(|entry| entry.band == LayerBand::SceneText && entry.number == 1601),
        "the room policy labels are script text pictures"
    );
}
