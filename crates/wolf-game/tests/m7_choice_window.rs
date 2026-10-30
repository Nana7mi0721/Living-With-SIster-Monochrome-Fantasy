//! M7 W5 H-MENU-1: the runtime choice window is painted in headless captures.
//!
//! A WOLF script never draws the frame for an opcode 102 prompt; the engine paints a
//! grey window skin and moves a cursor over it, at the screen coordinates system
//! variables 3 (x) and 4 (y) name (docs/OPCODE_102.md). This test pins the two halves
//! the capture path depends on:
//!
//!   * a live prompt that is on screen lays out exactly one row per option, entirely
//!     inside the 1024x576 viewport, and paints one text quad per option over a
//!     nine-slice skin with no atlas warnings;
//!   * with no prompt open, or with the title event's deliberately off-screen anchor
//!     (512, 1000), nothing is painted, because the engine itself would paint nothing.
//!
//! The six-way prompt is common event 246's top-level menu (摸摸头 / 揉肩 / 拥抱 /
//! 亲吻 / 挠痒痒 / 性骚扰), started bare exactly as capture --event 246 does it. Its
//! 102 is anchored at (512, 192), so it is the natural regression for a visible window.

use std::path::PathBuf;

use wolf_game::atlas::Atlas;
use wolf_game::choice;
use wolf_game::picture::{SCREEN_H, SCREEN_W};
use wolf_game::Game;

/// Data root. Same resolution as the other wolf-game integration tests: default to the
/// extracted retail tree the Lead pins for M6/M7, overridable with WOLF_DATA_ROOT.
fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".to_owned());
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return path;
    }
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    base.join(path)
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

/// Runs frames until an opcode 102 prompt is open, returning false if it never opens.
fn run_until_choice(game: &mut Game, max_frames: usize) -> bool {
    for _ in 0..max_frames {
        game.frame();
        if game.choices.is_some() {
            return true;
        }
    }
    false
}

/// Asserts a rectangle is fully inside the logical screen, naming it in the failure.
fn assert_inside_screen(rect: [f32; 4], what: &str) {
    assert!(rect[0] >= 0.0, "{what} left edge {} is above zero", rect[0]);
    assert!(rect[1] >= 0.0, "{what} top edge {} is above zero", rect[1]);
    assert!(
        rect[0] + rect[2] <= SCREEN_W as f32,
        "{what} right edge {} exceeds {}",
        rect[0] + rect[2],
        SCREEN_W
    );
    assert!(
        rect[1] + rect[3] <= SCREEN_H as f32,
        "{what} bottom edge {} exceeds {}",
        rect[1] + rect[3],
        SCREEN_H
    );
}

#[test]
fn a_live_prompt_paints_one_row_per_option_inside_the_screen() {
    let Some(mut game) = open_game() else { return };
    game.auto_advance = true;
    game.start_engine_events();
    assert!(game.start_event(246), "common event 246 exists");
    assert!(
        run_until_choice(&mut game, 300),
        "event 246 reaches its opcode 102 prompt"
    );

    let choices = game.choices.as_ref().expect("the prompt is open");
    let expected = ["摸摸头", "揉肩", "拥抱", "亲吻", "挠痒痒", "性骚扰"];
    assert_eq!(
        choices.options.len(),
        expected.len(),
        "the six-way prompt keeps all six options"
    );
    for (index, want) in expected.iter().enumerate() {
        assert_eq!(choices.options[index].as_str(), *want, "option {index}");
    }
    let cursor = game.choice_cursor();
    assert_eq!(cursor, 0, "the cursor starts on the first row");
    let (x, y) = game.choice_anchor();
    assert_eq!(
        (x, y),
        (512, 192),
        "the six-way prompt is anchored on screen"
    );

    let font_px = game.font_px();
    let layout = choice::layout_at_sized(&choices.options, cursor, x, y, font_px)
        .expect("an on-screen anchor yields a visible panel");
    assert_eq!(
        layout.lines.len(),
        choices.options.len(),
        "the layout has one row per option"
    );
    assert_inside_screen(layout.rect, "panel");
    for (index, line) in layout.lines.iter().enumerate() {
        assert_inside_screen(*line, &format!("row {index}"));
    }
    assert_inside_screen(layout.cursor, "cursor");

    // The draw list is one text quad per option over a nine-slice window skin, and the
    // skin plus font load without a single atlas warning -- the capture acceptance
    // requires game.stats.warnings == 0 and an empty atlas warning list here.
    let mut atlas = Atlas::new();
    let (panel, rows) = choice::overlay_at_sized(
        &mut atlas,
        &mut game.assets,
        &choices.options,
        cursor,
        x,
        y,
        font_px,
    )
    .expect("the on-screen prompt paints a panel");
    assert!(
        !panel.is_empty(),
        "the window skin paints at least one quad"
    );
    assert_eq!(
        rows.len(),
        choices.options.len(),
        "one text quad per option"
    );
    assert!(
        rows.iter().any(|row| row.text.contains("▶")),
        "the selected row carries the cursor marker"
    );
    assert!(
        atlas.take_warnings().is_empty(),
        "the window skin and font load without atlas warnings"
    );
    assert_eq!(game.stats.warnings, 0, "no engine warnings");
}

#[test]
fn no_choice_draws_nothing_and_the_title_anchor_is_clipped() {
    let Some(mut game) = open_game() else { return };

    // A fresh game has no active prompt: there is nothing to paint.
    assert!(game.choices.is_none(), "a new game opens with no prompt");
    assert!(
        choice::layout_at_sized(&[], 0, 512, 192, 20.0).is_none(),
        "an empty prompt has no rows"
    );

    // The title event's opcode 102 plants its window at (512, 1000) on a 576-pixel-tall
    // screen. The engine paints nothing there, so neither does the capture path; this is
    // the faithful answer the title screen relies on (docs/OPCODE_102.md section 4.2).
    game.auto_advance = true;
    game.start_engine_events();
    assert!(game.start_event(42), "common event 42 exists");
    assert!(
        run_until_choice(&mut game, 600),
        "event 42 reaches its opcode 102 prompt"
    );
    let choices = game.choices.as_ref().expect("the title prompt is open");
    assert_eq!(
        choices.options,
        vec!["1".to_owned(), "2".to_owned(), "3".to_owned()]
    );
    let (x, y) = game.choice_anchor();
    assert_eq!(
        (x, y),
        (512, 1000),
        "the title intentionally parks the window below the screen"
    );
    assert!(
        choice::layout_at_sized(&choices.options, game.choice_cursor(), x, y, game.font_px())
            .is_none(),
        "an off-screen anchor draws no quads"
    );
    assert_eq!(game.stats.warnings, 0, "no engine warnings");
}
