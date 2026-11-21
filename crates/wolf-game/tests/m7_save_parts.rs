//! M7 save-part coverage: every part the live game writes must round-trip.
//!
//! A WOLF save is five parts, and a played game touches all of them:
//!
//! * Part1 - map snapshot and object tables. The running game does not keep these, so
//!   `Game::save_to` copies them from the template save (`base_save`); this test proves
//!   they survive a write untouched.
//! * Part2 - the global variable tail: word 0 is play time, words 9/10 are the Sys3/Sys4
//!   choice-window anchor.
//! * Part3 - the 800 normal variables (row 0) plus the self variables/strings of live
//!   common-event instances (covered by the live-play test in m6_mainline.rs).
//! * Part4 - the object-table continuation. Like Part1 it is template-carried, and it
//!   *differs* between the retail saves (1194 vs 1213 bytes), so it is the part that
//!   proves the loaded file - not the old template - becomes the next write's base.
//! * Part5 - the PICTURE table: text, file and window records.
//!
//! The second test is a regression for the gap this investigation found: `load_into`
//! used to leave `game.base_save` pointing at the *previous* template (`System.sav` for
//! a fresh game), so a load -> save cycle silently replaced a loaded save's Part4 with
//! the template's.

use std::path::PathBuf;

use wolf_game::db::Container;
use wolf_game::game::Game;
use wolf_game::picture::{Picture, PictureKind};
use wolf_game::savegame;

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

fn retail_save_dir() -> Option<PathBuf> {
    let dir = std::env::var("WOLF_RETAIL_SAVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| repo_root().join("work").join("retail_backup").join("Save"));
    // Tests run with cwd = the crate directory, so a relative override
    // (WOLF_RETAIL_SAVE_DIR=work/retail_backup/Save) must resolve against the
    // repo root, like WOLF_DATA_ROOT above.
    let dir = if dir.is_absolute() {
        dir
    } else {
        repo_root().join(dir)
    };
    dir.is_dir().then_some(dir)
}

fn open_game() -> Option<Game> {
    match Game::new(data_root()) {
        Ok(game) => Some(game),
        Err(err) => {
            eprintln!("skipping: cannot open {}: {err}", data_root().display());
            None
        }
    }
}

/// One live picture reduced to the fields SavePart5 actually stores.
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

#[test]
fn every_save_part_a_played_game_writes_round_trips() {
    let Some(mut game) = open_game() else { return };
    let template = game
        .base_save
        .clone()
        .expect("work/full Save/System.sav is the write template");

    // Part3 row 0: 800 normal variables, mixing negatives and large values so the sign
    // and the full 32-bit range both have to survive the u32 storage.
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
    // Part2: play time (word 0) and the choice-window anchor Sys3/Sys4 (words 9/10).
    game.set_playtime_frames(987_654);
    game.vm.vars.set_sys(3, 700);
    game.vm.vars.set_sys(4, 355);

    // The variable database travels with the save too: one numeric and one text CData
    // cell (type 0 config column 5 is numeric, column 0 is text).
    game.databases.set_num(Container::CData, 0, 0, 5, 4242);
    game.databases
        .set_text(Container::CData, 0, 0, 0, "テスト名前");

    // Part5: one text record, one spritesheet file record and one window record.
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

    let pics_before = snapshot(&game);
    let vars_before = normal_vars(&game);

    // A private subdirectory keeps this test immune to any other test reusing a
    // shared `work/out` file name (and to a stale artifact left by an earlier run).
    let out_dir = repo_root()
        .join("work")
        .join("out")
        .join(format!("m7_save_parts_roundtrip_{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).expect("create the private out directory");
    let out = out_dir.join("save.sav");
    let bytes = game.save_to(&out).expect("save_to");
    assert!(bytes > 0, "save_to wrote {bytes} bytes");

    // Read the written file back and check each part at the byte level.
    let written = savegame::read_save(&out).expect("read back the written save");
    assert_eq!(
        written.part1, template.part1,
        "Part1 map snapshot must be carried from the write template"
    );
    assert_eq!(
        written.part4, template.part4,
        "Part4 object tables must be carried from the write template"
    );
    assert_eq!(
        written.part2.vars3_21[0], 987_654,
        "Part2 word 0 is the play time"
    );
    assert_eq!(written.part2.vars3_21[9], 700, "Part2 word 9 is Sys3");
    assert_eq!(written.part2.vars3_21[10], 355, "Part2 word 10 is Sys4");
    for (index, expected) in vars_before.iter().enumerate().take(800) {
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
    game.databases.set_num(Container::CData, 0, 0, 5, -1);
    game.databases.set_text(Container::CData, 0, 0, 0, "gone");

    let mut reloaded = open_game().expect("fresh game for the reload");
    let loaded = savegame::load_into(&mut reloaded, &out).expect("load the written save");

    assert_eq!(
        loaded,
        savegame::LoadedSave {
            pictures: 3,
            normal_vars: 800,
            playtime_frames: 987_654,
            choice_anchor: (700, 355),
        },
        "LoadedSave reports every part it restored"
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
        reloaded.playtime_frames, 987_654,
        "Part2 play time round-trips"
    );
    assert_eq!(
        (reloaded.vm.vars.get_sys(3), reloaded.vm.vars.get_sys(4)),
        (700, 355),
        "Part2 Sys3/Sys4 round-trip"
    );
    assert_eq!(
        reloaded.databases.num(Container::CData, 0, 0, 5),
        Some(4242),
        "the numeric CData cell round-trips"
    );
    assert_eq!(
        reloaded.databases.text(Container::CData, 0, 0, 0),
        Some("テスト名前"),
        "the text CData cell round-trips"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn a_loaded_retail_save_keeps_its_object_tables_through_a_save() {
    let Some(dir) = retail_save_dir() else {
        eprintln!("skipping: no retail Save directory (set WOLF_RETAIL_SAVE_DIR)");
        return;
    };
    if !data_root().is_dir() {
        eprintln!("skipping: data root {} is missing", data_root().display());
        return;
    }
    let out_dir = repo_root()
        .join("work")
        .join("out")
        .join(format!("m7_save_parts_retail_{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).expect("create the private out directory");

    let mut checked = 0usize;
    let mut part4_varied = 0usize;
    for slot in 1..=8 {
        let name = format!("SaveData{slot:02}.sav");
        let path = dir.join(&name);
        if !path.is_file() {
            continue;
        }
        let source = savegame::read_save(&path).expect("read retail save");
        let mut game = Game::new(data_root()).expect("game");
        let template = game.base_save.clone().expect("System.sav template");
        savegame::load_into(&mut game, &path).expect("load retail save");
        let out = out_dir.join(format!("m7_save_parts_{name}"));
        game.save_to(&out).expect("save the loaded game");
        let written = savegame::read_save(&out).expect("read the re-written save");

        assert_eq!(
            written.part1, source.part1,
            "{name}: Part1 map snapshot changed on a load -> save cycle"
        );
        assert_eq!(
            written.part4, source.part4,
            "{name}: Part4 object tables changed; a load must become the next write's template"
        );
        if source.part4 != template.part4 {
            part4_varied += 1;
        }

        let _ = std::fs::remove_file(&out);
        checked += 1;
    }

    assert!(checked > 0, "no retail save files were round-tripped");
    assert!(
        part4_varied > 0,
        "at least one retail save must differ from System.sav in Part4 for this regression to bite"
    );
    eprintln!("round-tripped {checked} retail save(s); {part4_varied} had a Part4 distinct from System.sav");
    let _ = std::fs::remove_dir_all(&out_dir);
}
