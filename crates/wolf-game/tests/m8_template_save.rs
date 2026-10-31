//! M8 W4-b: a data root with no `Save/System.sav` must still be saveable.
//!
//! `work/all` ships the game data but no `Save/` directory, which used to leave
//! `Game::base_save` as `None`; `Game::save_to` then refused with
//! "220 SaveLoad: no template save" and a game started there could never save.
//!
//! `Game::new` now synthesizes a structurally valid new-game template
//! ([`wolf_save::SaveFile::new_game_template`]) when System.sav cannot be read,
//! and records exactly one diagnostic instead of a warning. This test plays a few
//! frames, saves, and reloads the file in a fresh game, checking the three dynamic
//! parts the running game owns: play time (Part2), normal variables (Part3 row 0)
//! and the live picture layer (Part5).

use std::path::{Path, PathBuf};

use wolf_game::game::Game;
use wolf_game::picture::{Picture, PictureKind};
use wolf_game::savegame;

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

/// The data root under test: `WOLF_DATA_ROOT` when set, else `work/all`.
fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".to_owned());
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        repo_root().join(path)
    }
}

fn have_data(root: &Path) -> bool {
    root.join("BasicData").join("CommonEvent.dat").is_file()
}

/// One live picture reduced to the fields SavePart5 stores.
#[derive(Debug, PartialEq, Eq)]
struct Pic {
    number: i32,
    kind: String,
    file: String,
    text: String,
    x: i32,
    y: i32,
    opacity: i32,
    scale: i32,
    blend: i32,
    anchor: u8,
    split: (u32, u32),
    pattern_index: u32,
    window: Option<(i32, i32)>,
}

fn snapshot(game: &Game) -> Vec<Pic> {
    game.screen
        .iter()
        .map(|picture| Pic {
            number: picture.number,
            kind: format!("{:?}", picture.kind),
            file: picture.file.clone(),
            text: picture.text.clone(),
            x: picture.x,
            y: picture.y,
            opacity: picture.opacity,
            scale: picture.scale,
            blend: picture.blend,
            anchor: picture.anchor,
            split: picture.split,
            pattern_index: picture.pattern_index,
            window: picture.window,
        })
        .collect()
}

fn normal_vars(game: &Game) -> Vec<i32> {
    (0..800).map(|index| game.vm.vars.get(index)).collect()
}

/// No diagnostic may still say the game has no template, and the interpreter must
/// not have logged a warning while this path ran.
fn assert_no_template_refusal(game: &Game, when: &str) {
    assert_eq!(
        game.stats.warnings,
        0,
        "{when}: the synthesized template must not produce interpreter warnings: {:?}",
        game.diagnostics.keys().collect::<Vec<_>>()
    );
    for message in game.diagnostics.keys() {
        assert!(
            !message.contains("no template save"),
            "{when}: a save must never report a missing template: {message}"
        );
    }
}

#[test]
fn a_data_root_without_system_sav_saves_and_reloads() {
    let root = data_root();
    if !have_data(&root) {
        eprintln!("skipping: no game data at {}", root.display());
        return;
    }
    let system_sav = root.join("Save").join("System.sav");
    let synthesized_expected = !system_sav.is_file();

    let mut game = Game::new(&root).expect("open the data root");
    assert!(
        game.base_save.is_some(),
        "Game::new must synthesize a write template when {} is absent",
        system_sav.display()
    );
    if synthesized_expected {
        assert!(
            game.diagnostics
                .keys()
                .any(|message| message.contains("synthesized new-game template")),
            "the fallback must record one diagnostic; got {:?}",
            game.diagnostics.keys().collect::<Vec<_>>()
        );
    } else {
        eprintln!(
            "note: {} exists, so the synthesized fallback is not exercised",
            system_sav.display()
        );
    }
    // The template itself is a valid save: rebuilding and reparsing it must work.
    let template = game.base_save.clone().expect("template");
    template
        .to_bytes_rebuilt()
        .expect("the synthesized template must encode");

    // Run a few frames first, so the play clock and the engine state below are the
    // kind of state a real session carries, not a freshly constructed Game.
    game.start_engine_events();
    for _ in 0..5 {
        game.frame();
    }
    let frames_played = game.playtime_frames();
    assert_eq!(frames_played, 5, "five frames of play time");

    // Part3 row 0: the 800 normal variables, mixing signs and the full range.
    for index in 0..800usize {
        let value = match index % 5 {
            0 => index as i32,
            1 => -(index as i32),
            2 => 0,
            3 => 1_000_000 - index as i32,
            _ => (index as i32 * 37) % 60_000 - 30_000,
        };
        game.vm.vars.set(index, value);
    }
    // Part2: the choice-window anchor (words 9/10).
    game.vm.vars.set_sys(3, 700);
    game.vm.vars.set_sys(4, 355);

    // Part5: one text, one spritesheet file and one window record.
    let mut text = Picture::new(6200);
    text.kind = PictureKind::Text;
    text.text = "\\E\\f[24]\\c[9]21:40".to_owned();
    text.x = 140;
    text.y = 7;
    text.anchor = 3;

    let mut sprite = Picture::new(90);
    sprite.kind = PictureKind::File;
    sprite.file = "Picture/stand_imouto_paja2.png".to_owned();
    sprite.split = (6, 2);
    sprite.pattern_index = 2;
    sprite.x = 300;
    sprite.y = 120;
    sprite.blend = 2;

    let mut window = Picture::new(6233);
    window.kind = PictureKind::WindowFile;
    window.file = "SystemFile/WindowBase.png".to_owned();
    window.window = Some((150, 14));
    window.x = 64;
    window.y = 408;
    window.scale = 62;

    game.screen.clear();
    game.screen.insert(text);
    game.screen.insert(sprite);
    game.screen.insert(window);

    let vars_before = normal_vars(&game);
    let pics_before = snapshot(&game);

    let out_dir = repo_root()
        .join("work")
        .join("out")
        .join(format!("m8_template_save_{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).expect("create the private out directory");
    let out = out_dir.join("new_game.sav");
    let bytes = game
        .save_to(&out)
        .expect("a game from a root without System.sav must save");
    assert!(bytes > 0, "save_to wrote {bytes} bytes");
    assert_no_template_refusal(&game, "after save_to");

    // The written file is a real save: read it back and check each dynamic part.
    let written = savegame::read_save(&out).expect("read back the written save");
    assert_eq!(
        written.playtime_frames(),
        Some(frames_played),
        "Part2 word 0 is the play time"
    );
    assert_eq!(written.part2.vars3_21[9], 700, "Part2 word 9 is Sys3");
    assert_eq!(written.part2.vars3_21[10], 355, "Part2 word 10 is Sys4");
    for (index, expected) in vars_before.iter().enumerate() {
        assert_eq!(
            written.part3.normal_var(index),
            Some(*expected),
            "Part3 normal_var {index}"
        );
    }
    assert_eq!(
        written.part5.events.len(),
        3,
        "Part5 holds the three live pictures"
    );

    // Mutate the live game so a reload cannot pass by accident.
    for index in 0..800 {
        game.vm.vars.set(index, 123_456 + index as i32);
    }
    game.set_playtime_frames(0);
    game.vm.vars.set_sys(3, 0);
    game.vm.vars.set_sys(4, 0);
    game.screen.clear();

    let mut reloaded = Game::new(&root).expect("a fresh game for the reload");
    let loaded = savegame::load_into(&mut reloaded, &out).expect("load the written save");
    assert_eq!(
        loaded,
        savegame::LoadedSave {
            pictures: 3,
            normal_vars: 800,
            playtime_frames: frames_played,
            choice_anchor: (700, 355),
        },
        "LoadedSave reports every dynamic part it restored"
    );
    assert_eq!(
        normal_vars(&reloaded),
        vars_before,
        "Part3 normal variables round-trip"
    );
    assert_eq!(
        snapshot(&reloaded),
        pics_before,
        "Part5 picture records round-trip"
    );
    assert_eq!(
        reloaded.playtime_frames(),
        frames_played,
        "Part2 play time round-trips"
    );
    assert_eq!(
        (reloaded.vm.vars.get_sys(3), reloaded.vm.vars.get_sys(4)),
        (700, 355),
        "Part2 Sys3/Sys4 round-trip"
    );
    assert_no_template_refusal(&reloaded, "after load_into");

    let _ = std::fs::remove_dir_all(&out_dir);
}
