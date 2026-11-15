//! M12 UI input tests: the front end's wheel and mouse-to-key mapping.
//!
//! `wolf-cli` is a binary-only crate (no `lib.rs`), so a normal integration test
//! cannot reach the private helpers in `src/play.rs`. Including that file as a module
//! compiles the exact helper functions the running `App` uses - and therefore the
//! `#[cfg(test)]` tests declared beside them - without creating a window. The three
//! stubs below stand in for the `crate::` items `play::run` mentions; these tests never
//! call `run`, so they stay inert.
//!
//! Covered here (through `play::tests`): UI-01 wheel notch +1/-1 and ordinary UP/DOWN
//! bits, UI-02 right button -> CANCEL and never OK, horizontal wheel ignored, and
//! focus loss clearing every latched key/button/wheel value.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

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
