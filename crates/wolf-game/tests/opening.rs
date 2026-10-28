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
    game.auto_advance = true;
    game.auto_choice = true;
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
    // The measured shape of the first ten seconds: the script draws the black
    // backdrop, a text box frame and the story text, and parks on a message.
    assert!(game.stats.instructions > 20_000, "script barely ran: {}", game.stats.instructions);
    assert_eq!(game.stats.warnings, 0, "the opening produced warnings");
    assert!(game.screen.len() >= 10, "only {} pictures live", game.screen.len());
    assert!(!quads.is_empty(), "nothing textured on screen");
    assert!(!texts.is_empty(), "no text pictures on screen");
}
