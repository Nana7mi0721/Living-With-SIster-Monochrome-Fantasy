//! M12 render regressions: the room policy strip must match the retail baseline.
//!
//! UI-12 reported the top-left "方針 / 公会評価" panel as the wrong size, at the wrong
//! layer or covering the HUD. The user capture work/out/ui_bugs/07_room_1930_policy_panels_1027x580.png
//! contains six rows of Windows window chrome above the logical 1024x576 frame, so its
//! panel appears six pixels lower than it is in the game. The correct comparison is the
//! retail screenshot save7.jpg against the Rust capture of SaveData07.sav: both are
//! 1024x576 and both place the 530x84 Picture/policy.png strip at (40, 50).
//!
//! These tests pin the geometry so a future picture/atlas change cannot silently move or
//! rescale it. They also assert the strip stays below the top HUD bar (0..40), which is
//! the "panel must not cover the key HUD" half of the UI-12 acceptance.

use std::path::PathBuf;

use wolf_game::Game;

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        repo_root().join(path)
    }
}

fn save_path(name: &str) -> PathBuf {
    repo_root().join("work/retail_backup/Save").join(name)
}

fn load_save(name: &str) -> Option<Game> {
    let path = save_path(name);
    if !path.is_file() {
        eprintln!("skipping: {} is absent", path.display());
        return None;
    }
    let mut game = match Game::new(data_root()) {
        Ok(game) => game,
        Err(err) => {
            eprintln!("skipping: {err}");
            return None;
        }
    };
    game.load_save(&path)
        .unwrap_or_else(|err| panic!("load {}: {err}", path.display()));
    Some(game)
}

#[test]
fn save07_room_policy_strip_matches_the_retail_geometry() {
    let Some(game) = load_save("SaveData07.sav") else {
        return;
    };

    let panel = game
        .screen
        .get(1600)
        .expect("SaveData07 must restore the room policy strip in slot 1600");
    println!(
        "policy slot: number={} file={:?} kind={:?} x={} y={} scale={} anchor={} window={:?} hidden={} opacity={}",
        panel.number,
        panel.file,
        panel.kind,
        panel.x,
        panel.y,
        panel.scale,
        panel.anchor,
        panel.window,
        panel.hidden,
        panel.opacity
    );
    assert!(
        panel.file.ends_with("Picture/policy.png")
            || panel.file.ends_with("Picture/policy2.png")
            || panel.file.ends_with("Picture/policy3.png"),
        "slot 1600 must be the policy strip, got {:?}",
        panel.file
    );

    let dst = panel.destination(530.0, 84.0);
    assert_eq!(
        [dst[0], dst[1], dst[2], dst[3]],
        [40.0, 50.0, 530.0, 84.0],
        "retail save7 places the 530x84 policy strip at (40, 50); Rust is {dst:?}"
    );
    assert!(dst[1] >= 40.0, "the strip must stay below the top HUD bar");
    assert!(dst[1] + dst[3] <= 576.0, "the strip must stay on screen");
    assert!(
        dst[0] + dst[2] <= 1024.0,
        "the strip must not overhang the right edge"
    );
}

#[test]
fn save07_policy_text_starts_inside_the_strip() {
    let Some(game) = load_save("SaveData07.sav") else {
        return;
    };
    let panel = game.screen.get(1600).expect("policy strip");
    let frame = panel.destination(530.0, 84.0);

    // CE232 puts the labels at y=61/87/106 (left column) and 70/95/96/107 (right
    // columns), all relative to the 530x84 panel at y=50. A text run whose top is
    // outside the panel would either be clipped by the strip or paint over the HUD;
    // checking the left-column labels is enough to pin the relationship.
    for number in [1601, 1602, 1610] {
        let Some(text) = game.screen.get(number) else {
            panic!("CE232 text picture {number} missing after SaveData07 load");
        };
        let dst = text.destination(1.0, 1.0);
        assert!(
            dst[1] >= frame[1] && dst[1] < frame[1] + frame[3],
            "text {number} at y={} must sit inside the strip y={}..{}",
            dst[1],
            frame[1],
            frame[1] + frame[3]
        );
    }
}
