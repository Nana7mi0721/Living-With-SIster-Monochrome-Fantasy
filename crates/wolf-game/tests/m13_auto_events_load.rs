//! M13: loading a retail save must not replay `run_condition == 2` edges.
//!
//! Before the fix, `savegame::apply` restored the ordinary variables but left
//! `AutoEvents::latched` empty, so the first `frame()` after a load treated every
//! `run==2` event whose condition already held as a false->true edge and started
//! it. Event 506 (`妹トイレ前まばたき`) then drew `DLC_data/dlc_imouto01_eye.png`
//! (picture -6) over the loaded scene, which the retail thumbnail does not show.
//! Root-cause analysis: docs/research/M13_AUTO_EVENTS_SCOPE.md.
//!
//! These tests pin the fixed behaviour: after `load_save`, the first frames must
//! not start 506 or draw -6, while a fresh boot still starts its boot-time events.

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

fn open_game() -> Option<Game> {
    match Game::new(data_root()) {
        Ok(game) => Some(game),
        Err(err) => {
            eprintln!("skipping: {err}");
            None
        }
    }
}

fn eye_overlay_drawn(game: &Game) -> bool {
    game.screen
        .get(-6)
        .map(|picture| picture.file.contains("dlc_imouto01_eye"))
        .unwrap_or(false)
}

#[test]
fn loading_a_retail_save_does_not_start_506_or_draw_the_eye_overlay() {
    let Some(retail) = retail_save_dir() else {
        eprintln!("skipping: no retail save directory");
        return;
    };
    for slot in [1u32, 8] {
        let name = format!("SaveData{slot:02}.sav");
        let source = retail.join(&name);
        if !source.is_file() {
            eprintln!("skipping {name}: absent");
            continue;
        }
        let Some(mut game) = open_game() else { return };
        game.load_save(&source)
            .unwrap_or_else(|err| panic!("load {name}: {err}"));

        // Prime must only suppress re-triggering run==2 condition edges; it must not drop
        // the SavePart5 picture snapshot itself. The HUD header bar (picture 6000,
        // hscene/header_bar.jpg) is restored straight from part5 and must still be present.
        let header = game.screen.get(6000).unwrap_or_else(|| {
            panic!("{name}: HUD header bar (pic 6000) must survive from the SavePart5 snapshot")
        });
        assert!(
            header.file.contains("header_bar"),
            "{name}: pic 6000 should be the HUD header bar, got {:?}",
            header.file
        );

        // The old bug started 506 on the very first frame; watch a handful.
        for frame in 0..8 {
            game.frame();
            assert!(
                !game.vm.instances().iter().any(|instance| instance.event == 506),
                "{name}: event 506 must not be running after load (frame {frame})"
            );
            assert!(
                !eye_overlay_drawn(&game),
                "{name}: DLC eye overlay (pic -6) must not be drawn after load (frame {frame})"
            );
        }
    }
}

#[test]
fn a_fresh_boot_still_starts_its_run_condition_two_events() {
    // No save: variables are default, so event 38 (v160 <= 2) is the only run==2
    // event whose condition holds. The load-only priming must not suppress it.
    let Some(mut game) = open_game() else { return };
    let report = game.auto_events.sync(&mut game.vm);
    assert!(
        report.started >= 1,
        "fresh boot starts at least event 38, got {report:?}"
    );
    assert!(
        game.vm.instances().iter().any(|instance| instance.event == 38),
        "event 38 must start on a fresh boot"
    );
    assert!(
        !eye_overlay_drawn(&game),
        "fresh boot never draws the eye overlay"
    );
}
