//! Integration test: ".wolf" archives open and extract byte-exactly.
//!
//! Expected numbers come from "docs/FORMAT_SPEC.md" section 8 and from the
//! archives measured there. Both tests need the retail game data, so they skip
//! when it is missing: point WOLF_GAME_DIR at the game's Data folder and
//! WOLF_DATA_ROOT at the unpacked reference tree (default work/full).

use std::path::PathBuf;

use wolf_format::dxarchive::DxArchive;
use wolf_format::Error;

/// Steam install of "Living With Sister Monochrome Fantasy".
///
/// The folder really is spelled "SIster" on disk, so keep the odd casing.
const DEFAULT_GAME_DIR: &str =
    r"E:\SteamLibrary\steamapps\common\Living With SIster Monochrome Fantasy\Data";

/// Data root, resolved the way "wolf-cli" does it (see tests/tileset.rs).
fn data_root() -> PathBuf {
    let raw = match std::env::var("WOLF_DATA_ROOT") {
        Ok(value) if !value.is_empty() => PathBuf::from(value),
        _ => PathBuf::from("work/full"),
    };
    if raw.is_absolute() {
        return raw;
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .and_then(|crates| crates.parent())
        .unwrap_or(&manifest);
    let candidate = workspace.join(&raw);
    if candidate.is_dir() {
        candidate
    } else {
        raw
    }
}

/// Folder holding the retail *.wolf files, or None when it is not installed.
fn game_dir() -> Option<PathBuf> {
    let raw = match std::env::var("WOLF_GAME_DIR") {
        Ok(value) if !value.is_empty() => PathBuf::from(value),
        _ => PathBuf::from(DEFAULT_GAME_DIR),
    };
    if raw.is_dir() {
        Some(raw)
    } else {
        None
    }
}

/// Open one archive from the game folder.
fn open(name: &str) -> Option<DxArchive> {
    let Some(dir) = game_dir() else {
        eprintln!("skipping: no game data, set WOLF_GAME_DIR (looked in {DEFAULT_GAME_DIR})");
        return None;
    };
    let path = dir.join(name);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("skipping: cannot read {}: {error}", path.display());
            return None;
        }
    };
    Some(
        DxArchive::open(&bytes).unwrap_or_else(|error| panic!("{} opens: {error}", path.display())),
    )
}

/// Compare the first differing byte, if any.
fn first_difference(left: &[u8], right: &[u8]) -> Option<usize> {
    left.iter()
        .zip(right.iter())
        .position(|(a, b)| a != b)
        .or_else(|| {
            if left.len() == right.len() {
                None
            } else {
                Some(left.len().min(right.len()))
            }
        })
}
/// Open an archive and assert the header facts shared by every v8 WOLF file.
fn expect_archive(archive_name: &str, expected_files: usize) -> Option<DxArchive> {
    let archive = open(archive_name)?;
    assert_eq!(archive.version(), 8, "{archive_name} version");
    assert_eq!(
        archive.char_code_format(),
        932,
        "{archive_name} CharCodeFormat"
    );
    assert_eq!(archive.flags(), 0x013A_0000, "{archive_name} Flags");
    assert_eq!(
        archive.crypt_version(),
        0x013A,
        "{archive_name} crypt version"
    );
    assert_eq!(
        archive.huffman_encode_kb(),
        12,
        "{archive_name} HuffmanEncodeKB"
    );
    assert_eq!(
        archive.data_start_address(),
        64,
        "{archive_name} DataStartAddress"
    );
    assert!(archive.is_encrypted(), "{archive_name} is XOR obfuscated");
    assert_eq!(
        archive.entries().len(),
        expected_files,
        "{archive_name} file count"
    );
    assert!(
        archive
            .entries()
            .iter()
            .all(|entry| entry.directory.is_empty()),
        "{archive_name} is a flat root directory"
    );
    Some(archive)
}

/// Byte-compare every entry against the unpacked reference tree.
fn compare_against_reference(archive: &DxArchive, unpacked: &str) {
    let root = data_root().join(unpacked);
    if !root.is_dir() {
        eprintln!(
            "skipping comparison: no reference tree at {}",
            root.display()
        );
        return;
    }
    let mut checked = 0usize;
    for entry in archive.entries() {
        let path = entry.path();
        let reference_path = root.join(&path);
        let reference = match std::fs::read(&reference_path) {
            Ok(bytes) => bytes,
            Err(error) => panic!("read {}: {error}", reference_path.display()),
        };
        let extracted = archive
            .read(entry)
            .unwrap_or_else(|error| panic!("extract {path}: {error}"));
        assert_eq!(
            extracted.len() as u64,
            entry.size,
            "{path} extracted length matches the directory entry"
        );
        if let Some(index) = first_difference(&extracted, &reference) {
            panic!(
                "{path} differs from the reference at byte {index}: extracted {} byte(s), reference {} byte(s)",
                extracted.len(),
                reference.len()
            );
        }
        checked += 1;
    }
    assert_eq!(
        checked,
        archive.entries().len(),
        "every entry of {unpacked} was compared"
    );
}

#[test]
fn basic_data_extracts_byte_exactly() {
    let Some(archive) = expect_archive("BasicData.wolf", 149) else {
        return;
    };
    assert!(archive.entry("Game.dat").is_some(), "Game.dat is listed");
    assert!(
        archive.entry("icon128.png").is_some(),
        "icon128.png is listed"
    );
    assert!(archive.entry("apple.png").is_some(), "apple.png is listed");
    assert!(
        archive.entry("no_such_file.dat").is_none(),
        "unknown names resolve to None"
    );
    compare_against_reference(&archive, "BasicData");
}

#[test]
fn map_data_extracts_byte_exactly() {
    let Some(archive) = expect_archive("MapData.wolf", 9) else {
        return;
    };
    assert!(
        archive.entry("Dungeon.mps").is_some(),
        "Dungeon.mps is listed"
    );
    compare_against_reference(&archive, "MapData");
}

#[test]
fn system_file_extracts_byte_exactly() {
    let Some(archive) = expect_archive("SystemFile.wolf", 32) else {
        return;
    };
    compare_against_reference(&archive, "SystemFile");
}

#[test]
fn rejects_a_foreign_header() {
    let error = DxArchive::open(&[0u8; 64]).expect_err("zeroed bytes are not a DX archive");
    assert!(
        matches!(error, Error::Magic { .. }),
        "expected a magic mismatch, got {error:?}"
    );
}

#[test]
fn rejects_version_seven() {
    let mut bytes = [0u8; 64];
    bytes[0..2].copy_from_slice(&0x5844u16.to_le_bytes());
    bytes[2..4].copy_from_slice(&7u16.to_le_bytes());
    let error = DxArchive::open(&bytes).expect_err("version 7 is not readable");
    match error {
        Error::Field { what, msg, .. } => {
            assert_eq!(what, "DXArchive head");
            assert!(
                msg.contains("version"),
                "message mentions the version: {msg}"
            );
        }
        other => panic!("expected a field error, got {other:?}"),
    }
}
#[test]
fn scene_and_word_extract_byte_exactly() {
    for (archive_name, unpacked, expected) in [
        ("scene.wolf", "scene", 327usize),
        ("word.wolf", "word", 119),
    ] {
        let Some(archive) = expect_archive(archive_name, expected) else {
            return;
        };
        compare_against_reference(&archive, unpacked);
    }
}

/// Open every archive in the game folder and read every entry.
///
/// Ignored by default because it decompresses a few hundred megabytes, but it
/// is the broadest single check that the four storage branches all work.
#[test]
#[ignore = "reads every archive in the game folder, including 250 MB ones"]
fn every_archive_reads() {
    let Some(dir) = game_dir() else {
        eprintln!("skipping: no game data, set WOLF_GAME_DIR (looked in {DEFAULT_GAME_DIR})");
        return;
    };
    let mut archives = Vec::new();
    for item in std::fs::read_dir(&dir).expect("game folder is readable") {
        let path = item.expect("directory entry").path();
        if path.extension().is_some_and(|ext| ext == "wolf") {
            archives.push(path);
        }
    }
    archives.sort();
    assert!(
        !archives.is_empty(),
        "no .wolf files under {}",
        dir.display()
    );

    let mut files = 0usize;
    let mut bytes = 0u64;
    for path in archives {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let raw = std::fs::read(&path).expect("archive is readable");
        let archive = DxArchive::open(&raw).unwrap_or_else(|error| panic!("{name} opens: {error}"));
        assert_eq!(archive.version(), 8, "{name} version");
        assert_eq!(archive.char_code_format(), 932, "{name} CharCodeFormat");
        for entry in archive.entries() {
            let data = archive
                .read(entry)
                .unwrap_or_else(|error| panic!("{name}: extract {}: {error}", entry.path()));
            assert_eq!(
                data.len() as u64,
                entry.size,
                "{name}: {} length",
                entry.path()
            );
            files += 1;
            bytes += entry.size;
        }
        eprintln!("{name}: {} entries", archive.entries().len());
    }
    eprintln!("read {files} file(s), {bytes} byte(s)");
    assert!(files > 4000, "expected more than 4000 files, saw {files}");
}
