//! Regression tests for the wolf-game Host answers that opcode 124 SetVariableEx
//! reads. The interpreter half is pinned by crates/wolf-vm/tests/opcode_124_long.rs;
//! these tests drive the *real* Game host (not a mock) through synthetic programs so
//! the host-side field table cannot silently regress.
//!
//! Slice: docs/research/M7_HOST_STATE.md. Evidence spec: docs/OPCODE_124.md.

use std::path::PathBuf;
use std::sync::Arc;

use wolf_format::command::{Command, Slot, Tail};
use wolf_game::{Game, Picture};
use wolf_vm::Program;

/// Build one command slot. param_count counts the opcode byte plus every arg, so the
/// interpreter's argc is args.len() (this is what makes 0x3000 take the short form).
fn cmd(opcode: u32, args: &[i32], strings: &[&str]) -> Slot {
    let mut params = Vec::new();
    for a in args {
        params.extend_from_slice(&a.to_le_bytes());
    }
    Slot::Command(Box::new(Command {
        param_count: (args.len() + 1) as u8,
        opcode,
        params,
        branch_depth: 0,
        strings: strings.iter().map(|s| (*s).to_owned()).collect(),
        tail: Tail::None,
    }))
}

/// The real data root: WOLF_DATA_ROOT when set, otherwise work/all (this checkout's
/// merged data set) falling back to work/full.
fn data_root() -> PathBuf {
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    if let Ok(value) = std::env::var("WOLF_DATA_ROOT") {
        if !value.is_empty() {
            let path = PathBuf::from(value);
            return if path.is_absolute() { path } else { base.join(path) };
        }
    }
    let all = base.join("work").join("all");
    if all.join("BasicData").join("CommonEvent.dat").is_file() {
        return all;
    }
    base.join("work").join("full")
}

/// Real Game, or None when the data set is absent (the test then skips instead of
/// failing on a machine without work/).
fn open_game() -> Option<Game> {
    Game::new(data_root()).ok()
}

/// Run one synthetic program to completion on the real host for a single frame.
fn run_once(game: &mut Game, id: u32, title: &str, commands: Vec<Slot>) {
    // Parallel auto events would write normal variables of their own; disable them so
    // the only writer to the variables we assert is our synthetic program.
    game.auto_events.set_enabled(false);
    let program = Program::from_slots(id, title, commands);
    assert!(
        game.vm.start_program(Arc::new(program)).is_some(),
        "synthetic program should start"
    );
    let _ = game.frame();
}

#[test]
fn engine_state_reports_the_mouse_and_neutral_none_sentinels_through_opcode_124() {
    let Some(mut game) = open_game() else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    // Warm-up frame: retail computes fields 11/12 as "current mouse - position at the
    // end of the previous frame"; the warm-up gives the real host that previous frame.
    game.mouse = [100, 100];
    run_once(&mut game, 899, "host warm-up", vec![cmd(172, &[], &[])]);

    game.mouse = [640, 300];
    game.mouse_buttons = 1; // left button down this frame, up last frame
    // Seed the destinations with a poison value so "wrote 0 / wrote -1" is
    // distinguishable from "host returned None and left the destination alone".
    for index in [600usize, 601, 602, 603, 604, 605] {
        game.vm.vars.set(index, 424_242);
    }
    let commands = vec![
        // 0x3000 argc 3: [dst, sub, field]
        cmd(124, &[2_000_600, 0x3000, 20], &[]), // mouse screen X
        cmd(124, &[2_000_601, 0x3000, 21], &[]), // mouse screen Y
        cmd(124, &[2_000_602, 0x3000, 17], &[]), // this common event id: neutral 0
        cmd(124, &[2_000_603, 0x3000, 11], &[]), // mouse X travel since last frame
        cmd(124, &[2_000_604, 0x3000, 12], &[]), // mouse Y travel since last frame
        cmd(124, &[2_000_605, 0x3000, 7], &[]),  // left button tri-state
        cmd(172, &[], &[]),
    ];
    run_once(&mut game, 900, "host engine state", commands);
    assert_eq!(game.vm.vars.get(600), 640, "field 20 = mouse X");
    assert_eq!(game.vm.vars.get(601), 300, "field 21 = mouse Y");
    assert_eq!(game.vm.vars.get(602), 0, "field 17 answers neutral 0");
    assert_eq!(
        game.vm.vars.get(603),
        540,
        "field 11 = mouse X travel (640-100)"
    );
    assert_eq!(
        game.vm.vars.get(604),
        200,
        "field 12 = mouse Y travel (300-100)"
    );
    assert_eq!(game.vm.vars.get(605), 2, "field 7 = just pressed = 2");
    assert_eq!(game.stats.warnings, 0, "known fields emit no warnings");
}

#[test]
fn engine_travel_is_zero_without_movement_and_signed_when_moving_back() {
    let Some(mut game) = open_game() else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    game.mouse = [640, 300];
    run_once(&mut game, 910, "travel warm-up", vec![cmd(172, &[], &[])]);

    // Same pointer position as last frame: both travel answers must be exactly 0
    // (retail subtracts the stored previous position, so this is not a sentinel).
    for index in [630usize, 631] {
        game.vm.vars.set(index, 424_242);
    }
    run_once(
        &mut game,
        911,
        "travel still",
        vec![
            cmd(124, &[2_000_630, 0x3000, 11], &[]),
            cmd(124, &[2_000_631, 0x3000, 12], &[]),
            cmd(172, &[], &[]),
        ],
    );
    assert_eq!(game.vm.vars.get(630), 0, "no horizontal travel");
    assert_eq!(game.vm.vars.get(631), 0, "no vertical travel");

    // Moving left/up produces negative values, which is why -1 can never be used
    // as a "no answer" sentinel here.
    game.mouse = [10, 20];
    for index in [632usize, 633] {
        game.vm.vars.set(index, 424_242);
    }
    run_once(
        &mut game,
        912,
        "travel back",
        vec![
            cmd(124, &[2_000_632, 0x3000, 11], &[]),
            cmd(124, &[2_000_633, 0x3000, 12], &[]),
            cmd(172, &[], &[]),
        ],
    );
    assert_eq!(game.vm.vars.get(632), -630, "field 11 is signed (10-640)");
    assert_eq!(game.vm.vars.get(633), -280, "field 12 is signed (20-300)");
    assert_eq!(game.stats.warnings, 0, "known fields emit no warnings");
}

#[test]
fn picture_state_field_five_answers_zero_for_live_pictures_only() {
    let Some(mut game) = open_game() else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    let mut picture = Picture::new(12_004);
    picture.x = 111;
    picture.y = 222;
    picture.scale = 150;
    picture.extras = vec![7, 8];
    game.screen.insert(picture);
    for index in [610usize, 611, 612, 613, 614, 615, 616] {
        game.vm.vars.set(index, 424_242);
    }
    let commands = vec![
        cmd(124, &[2_000_610, 0x4000, 12_004, 0], &[]), // X
        cmd(124, &[2_000_611, 0x4000, 12_004, 1], &[]), // Y
        cmd(124, &[2_000_612, 0x4000, 12_004, 2], &[]), // scale
        cmd(124, &[2_000_613, 0x4000, 12_004, 4], &[]), // pattern / extras[0]
        cmd(124, &[2_000_614, 0x4000, 12_004, 5], &[]), // field 5: 0 for a live picture
        cmd(124, &[2_000_615, 0x4000, 12_004, 700], &[]), // unknown field: None
        cmd(124, &[2_000_616, 0x4000, 99_999, 5], &[]), // field 5, missing picture: None
        cmd(172, &[], &[]),
    ];
    run_once(&mut game, 901, "host picture state", commands);
    assert_eq!(game.vm.vars.get(610), 111, "field 0 = picture X");
    assert_eq!(game.vm.vars.get(611), 222, "field 1 = picture Y");
    assert_eq!(game.vm.vars.get(612), 150, "field 2 = picture scale");
    assert_eq!(game.vm.vars.get(613), 7, "field 4 = extras[0]");
    assert_eq!(game.vm.vars.get(614), 0, "field 5 answers 0 for a live picture");
    assert_eq!(
        game.vm.vars.get(615),
        424_242,
        "unknown picture field leaves the destination alone"
    );
    assert_eq!(
        game.vm.vars.get(616),
        424_242,
        "field 5 does not answer for a picture that does not exist"
    );
    assert_eq!(game.stats.warnings, 0, "known fields emit no warnings");
}

#[test]
fn character_state_stays_none_so_no_coordinates_are_fabricated() {
    let Some(mut game) = open_game() else {
        eprintln!("skipping: no WOLF data root");
        return;
    };
    // The port has no map/character layer: mps coords and page directions are parsed
    // but the Map is discarded after start_pages, so character_state returns None.
    // Pin that: a query must leave the destination untouched rather than invent a 0.
    for index in [620usize, 621] {
        game.vm.vars.set(index, 424_242);
    }
    let commands = vec![
        cmd(124, &[2_000_620, 0x1000, -2, 6], &[]), // leader screen X
        cmd(124, &[2_000_621, 0x1000, -2, 5], &[]), // leader direction
        cmd(172, &[], &[]),
    ];
    run_once(&mut game, 902, "host character state", commands);
    assert_eq!(
        game.vm.vars.get(620),
        424_242,
        "character field 6 is unsupported and leaves the destination alone"
    );
    assert_eq!(
        game.vm.vars.get(621),
        424_242,
        "character field 5 is unsupported and leaves the destination alone"
    );
}
