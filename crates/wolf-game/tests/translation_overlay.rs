//! The optional text overlay, checked against the real extracted data.
//!
//! These tests read work/all (or WOLF_DATA_ROOT) and are skipped when the retail
//! tree is absent, matching the other data-dependent tests in this crate.

use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_game::translation::TextOverlay;

fn data_root() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(root) = std::env::var_os("WOLF_DATA_ROOT") {
        candidates.push(PathBuf::from(root));
    }
    candidates.push(PathBuf::from("../../work/all"));
    candidates.push(PathBuf::from("work/all"));
    candidates
        .into_iter()
        .find(|root| root.join("BasicData/CommonEvent.dat").is_file())
}

#[test]
fn rewrites_a_real_message_slot_and_reports_the_count() {
    let Some(root) = data_root() else {
        eprintln!("skip: no extracted data root");
        return;
    };
    let bytes = std::fs::read(root.join("BasicData/CommonEvent.dat")).expect("CommonEvent.dat");
    let mut events = CommonEventData::parse(&bytes).expect("parse");
    // CE280 slot 8 is the first 101 message ("\\E……呼……稍微休息会儿吧。").
    let overlay = TextOverlay::parse("280\t8\t\\\\E测试覆盖文本\n");
    assert_eq!(overlay.len(), 1);
    assert_eq!(overlay.apply(&mut events), 1);

    let event = events.events.iter().find(|event| event.id == 280).expect("CE280");
    match &event.commands[8] {
        Slot::Command(command) => {
            assert_eq!(command.opcode, 101);
            assert_eq!(command.strings[0], "\\E测试覆盖文本");
        }
        other => panic!("slot 8 should be a command, got {other:?}"),
    }
}

#[test]
fn rows_that_do_not_match_a_101_slot_are_ignored() {
    let Some(root) = data_root() else {
        eprintln!("skip: no extracted data root");
        return;
    };
    let bytes = std::fs::read(root.join("BasicData/CommonEvent.dat")).expect("CommonEvent.dat");
    let mut events = CommonEventData::parse(&bytes).expect("parse");
    let overlay = TextOverlay::parse("9999\t0\tno such event\n280\t99999\tout of range\n");
    assert_eq!(overlay.apply(&mut events), 0);
}
