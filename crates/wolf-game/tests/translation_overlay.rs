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

    let event = events
        .events
        .iter()
        .find(|event| event.id == 280)
        .expect("CE280");
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

fn unescape(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut chars = field.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// The whole generated table must apply to the real data without a single
/// dropped row; a mismatch means we wrote an event/pc pair that has no 101
/// message, and that translation would silently never reach the screen.
#[test]
fn every_generated_overlay_row_applies_to_a_real_message() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../work/translation/final/wolf_override.tsv");
    let Ok(body) = std::fs::read_to_string(&path) else {
        eprintln!("skip: no generated overlay at {}", path.display());
        return;
    };
    let Some(root) = data_root() else {
        eprintln!("skip: no extracted data root");
        return;
    };
    let bytes = std::fs::read(root.join("BasicData/CommonEvent.dat")).expect("CommonEvent.dat");
    let mut events = CommonEventData::parse(&bytes).expect("parse");
    let overlay = TextOverlay::parse(&body);
    assert!(
        overlay.len() > 7000,
        "expected the full generated table, got {}",
        overlay.len()
    );
    assert_eq!(
        overlay.apply(&mut events),
        overlay.len(),
        "every override row must land on an existing 101 message"
    );
}

/// Every generated override row must be made only of WOLF's own escapes: the
/// tokeniser leaves an unknown escape as literal text, which would reach the
/// screen. This is the end-to-end guard for the MV -> WOLF mapping.
#[test]
fn every_generated_overlay_row_tokenises_without_literal_escapes() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../work/translation/final/wolf_override.tsv");
    let Ok(body) = std::fs::read_to_string(&path) else {
        eprintln!("skip: no generated overlay at {}", path.display());
        return;
    };
    let mut checked = 0usize;
    for (lineno, line) in body.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.splitn(3, '\t');
        let (_event, _pc, raw) = (cols.next(), cols.next(), cols.next());
        let Some(raw) = raw else {
            panic!("line {}: not event<TAB>pc<TAB>text", lineno + 1);
        };
        let text = unescape(raw);
        for token in wolf_format::messagetext::tokenize(&text) {
            if let wolf_format::messagetext::TextToken::Text(fragment) = token {
                assert!(
                    !fragment.contains('\\'),
                    "line {}: literal escape leaked into text: {fragment:?}\n{text}",
                    lineno + 1
                );
            }
        }
        checked += 1;
    }
    assert!(
        checked > 7000,
        "expected the full generated table, checked {checked}"
    );
}
