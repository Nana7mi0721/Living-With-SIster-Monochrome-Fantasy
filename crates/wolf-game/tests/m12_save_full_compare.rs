//! M12: full segment-by-segment comparison of the retail saves through the game layer.
//!
//! The M5/M8 save work proved the raw serializer is the exact inverse of the parser
//! (`crates/wolf-save/tests/write.rs`) and that the *live* fields survive a game-level
//! round trip (`save_roundtrip.rs`). What was still open is the segment-level question:
//! does `Game::load_save -> Game::save_to` reproduce an official save file byte for
//! byte? Before M12 it did not: Parts 1-4, the variable database and Part7 were already
//! identical after decryption, but Part5 (the picture table) differed because
//! `picture_from_record` only restored the fields the renderer uses and
//! `picture_to_record` then zeroed the tone/effect/animation fields it does not model,
//! and re-encoded text pictures from the approximated decoded text.
//!
//! M12 keeps the loaded `SavePart5_1` record on `Picture::saved_record` as the write
//! baseline, overwrites only the fields the live picture owns, and reuses the saved
//! `md1` byte stream while a text picture's text is unchanged. With that in place the
//! whole file - every segment after decryption and the encrypted bytes on disk - is
//! reproduced exactly for all eight retail saves.
//!
//! The retail saves are copyrighted, so the test skips when they are absent. Set
//! `WOLF_RETAIL_SAVE_DIR` to a `Save` directory that holds `SaveDataNN.sav`.

use std::path::{Path, PathBuf};

use wolf_game::Game;
use wolf_save::SaveFile;

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

fn retail_save_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("WOLF_RETAIL_SAVE_DIR") {
        let dir = PathBuf::from(dir);
        let dir = if dir.is_absolute() {
            dir
        } else {
            repo_root().join(dir)
        };
        return dir.is_dir().then_some(dir);
    }
    let dir = repo_root().join("work/retail_backup/Save");
    dir.is_dir().then_some(dir)
}

/// Scratch directory under `work/out`, like the other save tests. The system temp
/// directory is not writable in this project's sandboxed test runs.
fn out_dir(tag: &str) -> PathBuf {
    let dir = repo_root()
        .join("work/out")
        .join(format!("m12_save_compare_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn open_game() -> Option<Game> {
    match Game::new(data_root()) {
        Ok(game) => Some(game),
        Err(err) => {
            eprintln!("skipping: {err}");
            None
        }
    }
}

fn save_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(name)
}

#[test]
fn every_retail_save_round_trips_byte_for_byte_through_the_game() {
    let Some(retail) = retail_save_dir() else {
        eprintln!("skipping: no retail save directory");
        return;
    };
    let out = out_dir("roundtrip");
    let mut checked = 0usize;
    for slot in 1..=8 {
        let name = format!("SaveData{slot:02}.sav");
        let source = save_path(&retail, &name);
        let original = std::fs::read(&source)
            .unwrap_or_else(|err| panic!("cannot read {}: {err}", source.display()));
        let Some(mut game) = open_game() else { return };
        game.load_save(&source)
            .unwrap_or_else(|err| panic!("load {name}: {err}"));

        let written_path = out.join(&name);
        game.save_to(&written_path)
            .unwrap_or_else(|err| panic!("save {name}: {err}"));
        let written = std::fs::read(&written_path).expect("read written save");

        // Whole-file equality is the strongest form of the segment comparison: the
        // header, checksum and every segment after decryption, plus the cipher stream
        // that covers them, are identical.
        assert_eq!(
            written, original,
            "{name}: Game::load_save -> Game::save_to must reproduce the official file"
        );

        // Name the segments as a guard against a future format change making the whole
        // file equal for an accidental reason (e.g. a zero-length segment).
        let parsed = SaveFile::parse(&original).expect("parse retail save");
        let names: Vec<&str> = parsed.chunks().iter().map(|chunk| chunk.name).collect();
        assert_eq!(
            names,
            [
                "SavePart1",
                "SavePart2",
                "SavePart3",
                "SavePart4",
                "SavePart5",
                "VariableDatabase",
                "SavePart7",
            ],
            "{name}: every segment is present in the comparison"
        );
        checked += 1;
    }
    let _ = std::fs::remove_dir_all(&out);
    assert_eq!(checked, 8, "all eight retail saves were compared");
    eprintln!("m12 save compare: {checked} retail saves byte-identical through the game layer");
}

/// The baseline must not freeze the live picture. A script that moves a loaded picture
/// still writes the new position; only the unmodelled banks come from the saved record.
#[test]
fn a_moved_loaded_picture_writes_the_new_position_and_keeps_the_unmodelled_banks() {
    let Some(retail) = retail_save_dir() else {
        eprintln!("skipping: no retail save directory");
        return;
    };
    let Some(mut game) = open_game() else { return };
    let source = save_path(&retail, "SaveData01.sav");
    game.load_save(&source).expect("load SaveData01");

    let original =
        SaveFile::parse(&std::fs::read(&source).expect("read")).expect("parse retail save");
    let before = original
        .part5
        .events
        .iter()
        .find(|record| record.var1 == 90)
        .expect("SaveData01 carries picture 90");

    // A 150 Move lands in `Picture::x`; this is the same engine-level change.
    let mut picture = game.screen.get(90).expect("picture 90 loaded").clone();
    picture.x += 7;
    game.screen.insert(picture);

    let out = out_dir("moved");
    let written_path = out.join("SaveData01.sav");
    game.save_to(&written_path).expect("save");
    let written = SaveFile::parse(&std::fs::read(&written_path).expect("read back"))
        .expect("parse written save");
    let after = written
        .part5
        .events
        .iter()
        .find(|record| record.var1 == 90)
        .expect("written save carries picture 90");

    assert_eq!(
        after.vars7_18[0],
        before.vars7_18[0] + 7,
        "the moved x must be written"
    );
    assert_eq!(
        after.vars19_36, before.vars19_36,
        "the tone/effect bank the runtime does not model must be preserved"
    );
    assert_eq!(
        after.vars55_60, before.vars55_60,
        "the animation tail must be preserved"
    );
    assert_eq!(
        after.md1, before.md1,
        "the picture name byte stream must be unchanged"
    );

    let mut reloaded = open_game().expect("fresh game");
    reloaded.load_save(&written_path).expect("reload");
    assert_eq!(
        reloaded.screen.get(90).map(|picture| picture.x),
        Some(before.vars7_18[0] as i32 + 7),
        "the moved position survives a reload"
    );
    let _ = std::fs::remove_dir_all(&out);
}
