//! M10 save completeness: what a full save actually round-trips, and where the
//! template-carried format stops short of the live game state.
//!
//! The suite is a superset of the M7 part coverage. The first test drives every field
//! this port serialises -- the 800 normal variables (including the clock, the day
//! counter and the 170..174 stats), play time, the Sys3/Sys4 choice anchor, the
//! variable-database cells and a run-time row-count resize, and the picture table
//! (text, spritesheet, window, mirrored and zero-scale records) -- and checks each one
//! after a load into a fresh `Game`.
//!
//! The remaining tests cover these behaviours and the deliberate gaps documented in
//! docs/research/M10_SAVE_COMPLETENESS.md:
//!
//! * a common event's self variables are written to Part3 *and* restored, so they
//!   persist across separate invocations of the same event (M13 common-self fix);
//! * a picture hidden with `150 Hide` has no bit in Part5 and reloads visible;
//! * the map, tile map and leader placement have no snapshot in this template path;
//! * a system variable outside the Sys3/Sys4 pair has no save region at all;
//! * an extension row `100 + n` written without a row-count write is refused as
//!   unmodelled (the M8 template caveat, exercised through the live game).

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

fn open_game() -> Option<Game> {
    match Game::new(data_root()) {
        Ok(game) => Some(game),
        Err(err) => {
            eprintln!("skipping: cannot open {}: {err}", data_root().display());
            None
        }
    }
}

fn out_dir(tag: &str) -> PathBuf {
    let dir = repo_root()
        .join("work")
        .join("out")
        .join(format!("m8_save_parts_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create the private out directory");
    dir
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

fn sample_pictures() -> Vec<Picture> {
    let mut text = Picture::new(6200);
    text.kind = PictureKind::Text;
    text.text = r"\E\f[24]\c[9]21:40".to_owned();
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

    // The CE245 reaction icon: an anchor-6 file drawn with a -100% horizontal scale.
    let mut mirrored = Picture::new(1220);
    mirrored.kind = PictureKind::File;
    mirrored.file = "Picture/stand_imouto_paja2.png".to_owned();
    mirrored.anchor = 6;
    mirrored.x = 240;
    mirrored.y = 576;
    mirrored.scale = -100;

    // A zero scale keeps its slot in the table even though it draws nothing.
    let mut vanished = Picture::new(777);
    vanished.kind = PictureKind::File;
    vanished.file = "Picture/stand_imouto_paja2.png".to_owned();
    vanished.x = 500;
    vanished.y = 300;
    vanished.scale = 0;

    vec![text, sprite, window, mirrored, vanished]
}

#[test]
fn every_serialized_field_round_trips() {
    let Some(mut game) = open_game() else { return };
    let template = game
        .base_save
        .clone()
        .expect("work/full Save/System.sav is the write template");

    // The 800 normal variables, with the clock (4/5), day (160) and the five stats
    // (170..174) set to values a played day actually carries.
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
    game.vm.vars.set(4, 18); // hour
    game.vm.vars.set(5, 30); // minute
    game.vm.vars.set(160, 1); // day
    for (index, value) in [(170, 35), (171, 6), (172, 5), (173, 9), (174, 5)] {
        game.vm.vars.set(index, value);
    }

    game.set_playtime_frames(987_654);
    game.vm.vars.set_sys(3, 700);
    game.vm.vars.set_sys(4, 355);

    // Variable database: one numeric and one text cell in the shipped rows, then a
    // run-time resize to 101 rows and a cell in the appended extension row.
    game.databases.set_num(Container::CData, 0, 0, 5, 4242);
    game.databases
        .set_text(Container::CData, 0, 0, 0, "テスト名前");
    game.databases.set_row_count(Container::CData, 0, 101);
    game.databases.set_num(Container::CData, 0, 100, 5, 7777);
    game.databases
        .set_text(Container::CData, 0, 100, 0, "拡張行");
    assert_eq!(
        game.databases.row_count(Container::CData, 0),
        Some(101),
        "the live table was resized to 101 rows"
    );

    game.screen.clear();
    for picture in sample_pictures() {
        game.screen.insert(picture);
    }

    let vars_before = normal_vars(&game);
    let pics_before = snapshot(&game);

    let dir = out_dir("roundtrip");
    let out = dir.join("save.sav");
    let bytes = game.save_to(&out).expect("save_to");
    assert!(bytes > 0, "save_to wrote {bytes} bytes");

    let written = savegame::read_save(&out).expect("read back the written save");
    assert_eq!(
        written.part1, template.part1,
        "Part1 map snapshot is carried from the write template"
    );
    assert_eq!(
        written.part4, template.part4,
        "Part4 object tables are carried from the write template"
    );
    assert_eq!(
        written.part2.vars3_21[0], 987_654,
        "Part2 word 0 is play time"
    );
    assert_eq!(written.part2.vars3_21[9], 700, "Part2 word 9 is Sys3");
    assert_eq!(written.part2.vars3_21[10], 355, "Part2 word 10 is Sys4");
    for index in 0..800 {
        assert_eq!(
            written.part3.normal_var(index),
            Some(vars_before[index]),
            "Part3 normal_var {index}"
        );
    }
    assert_eq!(
        written.part5.events.len(),
        5,
        "Part5 holds all five live pictures"
    );
    assert_eq!(
        written.vardb.types[0].rows.len(),
        101,
        "the resize to 101 rows travels in the save variable database"
    );

    // Mutate the live game so a reload cannot pass by accident, then load into a fresh
    // Game and assert every serialised field.
    for index in 0..800 {
        game.vm.vars.set(index, 123_456 + index as i32);
    }
    game.set_playtime_frames(0);
    game.vm.vars.set_sys(3, 0);
    game.vm.vars.set_sys(4, 0);
    game.screen.clear();
    game.databases.set_num(Container::CData, 0, 0, 5, -1);
    game.databases.set_text(Container::CData, 0, 0, 0, "gone");
    game.databases.set_row_count(Container::CData, 0, 17);

    let mut reloaded = open_game().expect("fresh game for the reload");
    let loaded = savegame::load_into(&mut reloaded, &out).expect("load the written save");

    assert_eq!(
        loaded,
        savegame::LoadedSave {
            pictures: 5,
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
        (reloaded.vm.vars.get(4), reloaded.vm.vars.get(5)),
        (18, 30),
        "the clock normal variables round-trip"
    );
    assert_eq!(
        reloaded.vm.vars.get(160),
        1,
        "the day normal variable round-trips"
    );
    assert_eq!(
        [170, 171, 172, 173, 174].map(|index| reloaded.vm.vars.get(index)),
        [35, 6, 5, 9, 5],
        "the five stat normal variables round-trip"
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
    assert_eq!(
        reloaded.databases.row_count(Container::CData, 0),
        Some(101),
        "the saved row count is restored, not the stale .dat height"
    );
    assert_eq!(
        reloaded.databases.num(Container::CData, 0, 100, 5),
        Some(7777),
        "the appended extension-row numeric cell round-trips"
    );
    assert_eq!(
        reloaded.databases.text(Container::CData, 0, 100, 0),
        Some("拡張行"),
        "the appended extension-row text cell round-trips"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn common_event_self_variables_are_written_and_restored() {
    let Some(mut game) = open_game() else { return };

    assert!(game.start_event(244), "CE244 is a real common event");
    let id = game
        .vm
        .instances()
        .iter()
        .find(|instance| instance.event == 244)
        .map(|instance| instance.id)
        .expect("the started CE244 instance");
    {
        let instance = game.vm.instance_mut(id).expect("the CE244 instance");
        instance
            .self_vars
            .set(wolf_vm::program::COMMON_SELF_BASE + 3, 1234);
        instance
            .self_vars
            .set_str(wolf_vm::program::COMMON_SELF_BASE + 5, "held");
    }

    let dir = out_dir("selfvars");
    let out = dir.join("save.sav");
    game.save_to(&out).expect("save_to");
    let written = savegame::read_save(&out).expect("read back the written save");
    assert_eq!(
        written.part3.self_var(244, 3),
        Some(1234),
        "Part3 does store a live common event's self variable"
    );
    assert_eq!(
        written.part3.self_string(244, 0),
        Some("held"),
        "Part3 does store a live common event's self string"
    );

    // A fresh load must restore the bank from Part3 into the persistent store, so a
    // restarted event starts from the saved values instead of a blank bank (the M13
    // common-self fix; this used to be the documented gap).
    let mut reloaded = open_game().expect("fresh game for the reload");
    savegame::load_into(&mut reloaded, &out).expect("load the written save");
    assert!(reloaded.vm.start_event_resuming(244).is_some());
    let first = reloaded
        .vm
        .instances()
        .iter()
        .find(|instance| instance.event == 244)
        .map(|instance| instance.id)
        .expect("the restarted CE244 instance");
    {
        let instance = reloaded.vm.instance_mut(first).expect("the CE244 instance");
        assert_eq!(
            instance.self_vars.get(wolf_vm::program::COMMON_SELF_BASE + 3),
            1234,
            "the saved common-event self variable is restored from Part3"
        );
        assert_eq!(
            instance.self_vars.get_str(wolf_vm::program::COMMON_SELF_BASE + 5),
            "held",
            "the saved common-event self string is restored from Part3"
        );
    }

    // Cross-invocation persistence: retiring this coroutine folds its bank back into the
    // store, so an immediate restart of the same event starts from those values, not 0.
    assert!(
        reloaded.vm.stop_instance(first),
        "stop_instance retires the CE244 coroutine"
    );
    assert!(reloaded.vm.start_event_resuming(244).is_some());
    let second = reloaded
        .vm
        .instances()
        .iter()
        .find(|instance| instance.event == 244)
        .map(|instance| instance.id)
        .expect("the restarted CE244 instance");
    {
        let instance = reloaded.vm.instance_mut(second).expect("the CE244 instance");
        assert_eq!(
            instance.self_vars.get(wolf_vm::program::COMMON_SELF_BASE + 3),
            1234,
            "a common event's self variable persists across invocations"
        );
        assert_eq!(
            instance.self_vars.get_str(wolf_vm::program::COMMON_SELF_BASE + 5),
            "held",
            "a common event's self string persists across invocations"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_hidden_picture_has_no_save_bit_and_reloads_visible() {
    let Some(mut game) = open_game() else { return };

    let mut hidden = Picture::new(901);
    hidden.kind = PictureKind::File;
    hidden.file = "Picture/stand_imouto_paja2.png".to_owned();
    hidden.x = 12;
    hidden.y = 34;
    hidden.hidden = true;
    game.screen.clear();
    game.screen.insert(hidden);

    let dir = out_dir("hidden");
    let out = dir.join("save.sav");
    game.save_to(&out).expect("save_to");
    let written = savegame::read_save(&out).expect("read back the written save");
    assert_eq!(
        written.part5.events.len(),
        1,
        "the hidden slot is still written to the picture table"
    );
    assert_eq!(written.part5.events[0].var1, 901);

    let mut reloaded = open_game().expect("fresh game for the reload");
    savegame::load_into(&mut reloaded, &out).expect("load the written save");
    let picture = reloaded.screen.get(901).expect("the picture slot reloaded");
    assert!(
        !picture.hidden,
        "documented gap: SavePart5 has no hidden bit, so a hidden picture reloads visible"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_map_tile_layer_and_leader_are_not_part_of_this_save_path() {
    let Some(mut game) = open_game() else { return };
    game.start_map_event_by_name("SampleMapA", 0)
        .expect("start the sample map");
    assert!(
        game.tilemap.is_some(),
        "starting a map installs its tile layer in the live game"
    );

    let dir = out_dir("mapsave");
    let out = dir.join("save.sav");
    game.save_to(&out).expect("save_to");

    let mut reloaded = open_game().expect("fresh game for the reload");
    savegame::load_into(&mut reloaded, &out).expect("load the written save");
    assert!(
        reloaded.tilemap.is_none(),
        "documented gap: Part1 carries no map snapshot, so the tile layer is not restored"
    );
    assert!(
        reloaded.runtime.leader().is_none(),
        "documented gap: the leader placement is not restored either"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn system_variables_outside_the_anchor_pair_have_no_save_region() {
    let Some(mut game) = open_game() else { return };
    game.vm.vars.set_sys(0, 77);
    game.vm.vars.set_sys(5, 1234);
    game.vm.vars.set_sys(12, 1);

    let dir = out_dir("sysvars");
    let out = dir.join("save.sav");
    game.save_to(&out).expect("save_to");

    let mut reloaded = open_game().expect("fresh game for the reload");
    savegame::load_into(&mut reloaded, &out).expect("load the written save");
    assert_eq!(
        (reloaded.vm.vars.get_sys(0), reloaded.vm.vars.get_sys(5)),
        (0, 0),
        "documented gap: only Sys3/Sys4 live in Part2; other system variables are transient"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_extension_row_written_without_a_row_count_is_refused() {
    let Some(mut game) = open_game() else { return };
    // 100+n is a run-time slot; without a 250 row-count write the template table has no
    // row 100, so write_into cannot encode it and counts the cell as unmodelled.
    game.databases.set_num(Container::CData, 0, 100, 5, 5150);

    let unmodelled_before = game.stats.unmodelled;
    let dir = out_dir("extrow");
    let out = dir.join("save.sav");
    game.save_to(&out).expect("save_to");
    assert!(
        game.stats.unmodelled > unmodelled_before,
        "the refused extension cell is counted as unmodelled"
    );

    let written = savegame::read_save(&out).expect("read back the written save");
    assert_eq!(
        written.vardb.types[0].rows.len(),
        17,
        "the un-resized table keeps its .dat height in the save"
    );
    assert!(
        written.vardb.types[0]
            .rows
            .iter()
            .all(|row| row.index != 100),
        "the unmodelled extension row is absent from the save"
    );

    let mut reloaded = open_game().expect("fresh game for the reload");
    savegame::load_into(&mut reloaded, &out).expect("load the written save");
    assert!(
        !reloaded.databases.row_exists(Container::CData, 0, 100),
        "documented gap: the un-resized table still ends below row 100"
    );
    assert_eq!(
        reloaded.databases.engine_num(Container::CData, 0, 100, 5),
        None,
        "documented gap: a row written without a resize is not restored"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
