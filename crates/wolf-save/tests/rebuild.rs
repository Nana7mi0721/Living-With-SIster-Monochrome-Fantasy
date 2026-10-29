//! Full-file rebuild: parse -> rebuild (typed serializers) -> encrypt must
//! reproduce each of the nine retail saves byte for byte.
//!
//! Two writers exist and this file pins the difference:
//!
//! * SaveFile::serialize copies every segment's original bytes; it only
//!   re-derives the header, the name, the version and the checksum.
//! * SaveFile::rebuild re-encodes SavePart1/2/3/4/5 from their typed views and
//!   copies only VariableDatabase and SavePart7, which have no writer yet.
//!
//! Both must agree with the file on disk, and a mutated typed field must survive
//! the second one.

use std::path::{Path, PathBuf};

use wolf_save::SaveFile;

/// Every save this game ships, in the order the game numbers its slots.
const FILES: &[&str] = &[
    "SaveData01.sav",
    "SaveData02.sav",
    "SaveData03.sav",
    "SaveData04.sav",
    "SaveData05.sav",
    "SaveData06.sav",
    "SaveData07.sav",
    "SaveData08.sav",
    "System.sav",
];

/// The retail save directory, or None when it is not on this machine.
///
/// work/retail_backup/Save holds the untouched copies checked into the work
/// tree; WOLF_GAME_DIR/Save is the live install. Both hold the same nine files.
fn save_dir() -> Option<PathBuf> {
    let candidates = [
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("work")
            .join("retail_backup")
            .join("Save"),
        std::env::var("WOLF_GAME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(
                    r"E:\SteamLibrary\steamapps\common\Living With SIster Monochrome Fantasy",
                )
            })
            .join("Save"),
    ];
    for dir in candidates {
        if dir.is_dir() {
            return Some(dir);
        }
    }
    eprintln!("skipping: no save directory found (set WOLF_GAME_DIR to the install directory)");
    None
}

/// Every save on disk as (name, original bytes, parsed).
fn all_saves() -> Vec<(&'static str, Vec<u8>, SaveFile)> {
    let Some(dir) = save_dir() else {
        return Vec::new();
    };
    FILES
        .iter()
        .map(|name| {
            let path = dir.join(name);
            let data = std::fs::read(&path)
                .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
            let save = SaveFile::parse(&data)
                .unwrap_or_else(|err| panic!("{} must parse: {err}", path.display()));
            (*name, data, save)
        })
        .collect()
}

/// Where two buffers first disagree, for a readable assertion message.
fn first_difference(left: &[u8], right: &[u8]) -> Option<usize> {
    if left.len() != right.len() {
        let shared = left.len().min(right.len());
        for at in 0..shared {
            if left[at] != right[at] {
                return Some(at);
            }
        }
        return Some(shared);
    }
    left.iter().zip(right).position(|(a, b)| a != b)
}

/// The gate: every retail save survives a typed rebuild unchanged.
#[test]
fn every_retail_save_rebuilds_to_the_same_bytes() {
    let saves = all_saves();
    if saves.is_empty() {
        return;
    }
    for (name, original, save) in &saves {
        let rebuilt = save
            .to_bytes_rebuilt()
            .unwrap_or_else(|err| panic!("{name} rebuild failed: {err}"));
        if let Some(at) = first_difference(original, &rebuilt) {
            panic!(
                "{name}: rebuilt file differs at offset {at:#X} ({} vs {} byte(s))",
                original.len(),
                rebuilt.len()
            );
        }
        assert_eq!(rebuilt.len(), original.len(), "{name}: length");
    }
    assert_eq!(saves.len(), 9);
}

/// The copy-everything writer and the rebuild writer must agree.
#[test]
fn rebuild_matches_serialize_and_to_bytes() {
    let saves = all_saves();
    if saves.is_empty() {
        return;
    }
    for (name, original, save) in &saves {
        let copied = save.serialize().expect("serialize");
        let rebuilt = save.rebuild().expect("rebuild");
        assert_eq!(
            copied,
            rebuilt,
            "{name}: serialize and rebuild disagree ({} vs {} byte(s))",
            copied.len(),
            rebuilt.len()
        );
        assert_eq!(copied.len(), original.len(), "{name}: length");
        assert_eq!(
            &save.to_bytes().expect("to_bytes"),
            original,
            "{name}: to_bytes"
        );
    }
}

/// Each segment encoder on its own, compared with the segment bytes on disk.
#[test]
fn each_segment_encoder_reproduces_its_parsed_bytes() {
    let saves = all_saves();
    if saves.is_empty() {
        return;
    }
    for (name, _original, save) in &saves {
        let version = save.file_version;
        let cases: [(&str, Vec<u8>, &[u8]); 5] = [
            (
                "SavePart1",
                save.part1.to_bytes(version).expect("part1"),
                save.chunk_bytes(wolf_save::PART1_ID).expect("part1 bytes"),
            ),
            (
                "SavePart2",
                save.part2.to_bytes(version).expect("part2"),
                save.chunk_bytes(wolf_save::PART2_ID).expect("part2 bytes"),
            ),
            (
                "SavePart3",
                save.part3.to_bytes_with_version(version).expect("part3"),
                save.chunk_bytes(wolf_save::PART3_ID).expect("part3 bytes"),
            ),
            (
                "SavePart4",
                save.part4.to_bytes(version).expect("part4"),
                save.chunk_bytes(wolf_save::PART4_ID).expect("part4 bytes"),
            ),
            (
                "SavePart5",
                save.part5.to_bytes(version).expect("part5"),
                save.chunk_bytes(wolf_save::PART5_ID).expect("part5 bytes"),
            ),
        ];
        for (label, encoded, parsed) in cases {
            if let Some(at) = first_difference(parsed, &encoded) {
                panic!(
                    "{name} {label}: encoder differs at offset {at:#X} ({} vs {} byte(s))",
                    parsed.len(),
                    encoded.len()
                );
            }
        }
    }
}

/// Mutating a SavePart2 word and a SavePart5 event field survives a full write.
#[test]
fn mutations_in_part2_and_part5_survive_a_rebuild() {
    let saves = all_saves();
    if saves.is_empty() {
        return;
    }
    let (name, _original, base) = &saves[0];

    // SavePart2: the play time, word 0 of vars3_21. That is also the field
    // SaveFile::set_playtime_frames patches in place, so the two paths must
    // agree on the bytes they produce.
    let mut save = base.clone();
    let before = save.playtime_frames().expect("play time");
    let edited = before.wrapping_add(1234);
    let mut part2 = save.part2.clone();
    part2.vars3_21[0] = edited;
    save.set_part2(&part2).expect("set_part2");
    assert_eq!(save.playtime_frames(), Some(edited));
    let bytes = save.to_bytes_rebuilt().expect("rebuild after part2 edit");
    let fresh = SaveFile::parse(&bytes).expect("reparse after part2 edit");
    assert_eq!(fresh.playtime_frames(), Some(edited), "{name}: play time");
    assert_eq!(fresh.name, base.name, "{name}: name unchanged");
    assert!(fresh.checksum_ok(), "{name}: checksum after part2 edit");

    // SavePart5: one word inside one event's runtime state.
    let mut save = base.clone();
    assert!(
        !save.part5.events.is_empty(),
        "{name}: expected event states"
    );
    let slot = save.part5.events.len() / 2;
    let mut part5 = save.part5.clone();
    let old = part5.events[slot].vars7_18[0];
    part5.events[slot].vars7_18[0] = 0x5A5A_5A5A;
    save.set_part5(&part5).expect("set_part5");
    assert_eq!(save.part5.events[slot].vars7_18[0], 0x5A5A_5A5A);
    let bytes = save.to_bytes_rebuilt().expect("rebuild after part5 edit");
    let fresh = SaveFile::parse(&bytes).expect("reparse after part5 edit");
    assert_eq!(
        fresh.part5.events.len(),
        part5.events.len(),
        "{name}: event count"
    );
    assert_eq!(
        fresh.part5.events[slot].vars7_18[0], 0x5A5A_5A5A,
        "{name}: edited word (was {old:#010X})"
    );
    assert_eq!(
        fresh.part5.events[slot].var1, part5.events[slot].var1,
        "{name}: event id"
    );

    // Reverting the edit gets the original bytes back, which is the strongest
    // statement that the serializer has no hidden state.
    let mut reverted = fresh.clone();
    let mut back = reverted.part5.clone();
    back.events[slot].vars7_18[0] = old;
    reverted.set_part5(&back).expect("set_part5 revert");
    let bytes = reverted.to_bytes_rebuilt().expect("rebuild after revert");
    assert_eq!(&bytes, _original, "{name}: revert must restore the file");
}
