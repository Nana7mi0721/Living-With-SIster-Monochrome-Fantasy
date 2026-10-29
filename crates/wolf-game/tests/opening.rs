//! The opening story script: SampleMapA event 0, run headlessly.
//!
//! This is M2's vertical slice. The script starts with a long linear cutscene
//! (disclaimer, choices, fades, cg pictures) and is the first content that uses
//! the message, choice, save/load and effect paths together, so a regression in
//! any of them shows up here as a change in the instruction count or a warning.

use std::path::PathBuf;

use wolf_game::atlas::{self, Atlas};
use wolf_game::Game;

fn data_root() -> PathBuf {
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    base.join("work/full")
}

#[test]
fn the_opening_script_runs_and_draws_its_pictures() {
    let root = data_root();
    let mut game = match Game::new(&root) {
        Ok(game) => game,
        Err(err) => {
            eprintln!("skipping: {err}");
            return;
        }
    };
    // Messages dismiss themselves, but the opening prompt is HELD: dice the choice and
    // the script walks on into the character creation loop, which spins without a frame
    // wait (measured: 103 million instructions over 600 frames). Holding it is what the
    // retail player sees at the end of the cutscene and what `wolf-cli capture` photographs.
    game.auto_advance = true;
    if let Err(err) = game.start_map_event_by_name("SampleMapA", 0) {
        eprintln!("skipping: cannot start the opening: {err}");
        return;
    }
    for _ in 0..600 {
        let _ = game.frame();
    }
    let mut atlas = Atlas::new();
    let quads = atlas.sprites(&game.screen, &mut game.assets);
    let texts = atlas::texts(&game.screen);
    println!(
        "frames 600 instructions {} pictures {} quads {} texts {} atlas images {} warnings {}",
        game.stats.instructions,
        game.screen.len(),
        quads.len(),
        texts.len(),
        atlas.len(),
        game.stats.warnings
    );
    for picture in game.screen.iter() {
        println!(
            "  pic {:<6} {:?} at ({}, {}) scale {} alpha {:.2} {:?}",
            picture.number,
            picture.kind,
            picture.x,
            picture.y,
            picture.scale,
            picture.alpha(),
            picture.file
        );
    }
    // The measured shape of the first ten seconds, taken from this run rather than
    // guessed: 19,596 instructions, 696 picture operations, no warnings. The script
    // draws a black backdrop, runs the loading screen (common event 221), erases it and
    // parks on the four-option opening prompt.
    assert_eq!(game.stats.warnings, 0, "the opening produced warnings");
    assert!(
        (15_000..200_000).contains(&game.stats.instructions),
        "script ran {} instructions",
        game.stats.instructions
    );
    // The backdrop is the whole-screen layer WOLF numbers -1.
    let backdrop = game.screen.get(-1).expect("black backdrop");
    assert_eq!(backdrop.file, "cg/blackscreen.jpg");
    // The regression this test exists for: common event 221 ends the loading screen with
    // ONE ranged erase, `150 [16777218, 50000, 5, 5, 0, 50003]`. While the picture layer
    // read that as a single-slot erase, the eyecatch (50001), the NOW LOADING text (50002)
    // and the progress bar (50003) stayed on screen for the whole game, because nothing
    // else in the script ever names them.
    for number in 50000..=50003 {
        assert!(
            game.screen.get(number).is_none(),
            "loading picture {number} survived the ranged erase"
        );
    }
    // The prompt itself: the engine choice window is painted by the caller (see
    // wolf_game::choice), so the live prompt is what proves the script parked where the
    // retail player sees the menu.
    let prompt = game.choices.as_ref().expect("the opening prompt is open");
    assert_eq!(
        prompt.options,
        ["广阔的战场", "广袤的森林", "微风吹拂而过的麦田", "有人在等我的家"],
        "the opening prompt changed"
    );
    // Nothing the script draws is a text picture any more - the prompt came through the
    // choice path - so only the textured side of the atlas is asserted.
    assert!(!quads.is_empty(), "nothing textured on screen");
    assert!(texts.is_empty(), "unexpected text pictures: {}", texts.len());
}
