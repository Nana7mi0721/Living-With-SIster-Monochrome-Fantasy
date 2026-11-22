//! Integration test: `TileSetData.dat` parses byte-exactly.
//!
//! Expected numbers come from `docs/FORMAT_SPEC.md` §7.4 and `docs/BASELINE.json`.

use std::path::PathBuf;

use wolf_format::tileset::{TilePassability, TileSetData};

/// Data root, resolved the way `wolf-cli` does it (see `tests/mps.rs`).
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

fn load_required() -> Vec<u8> {
    let path = data_root().join("BasicData").join("TileSetData.dat");
    std::fs::read(&path).unwrap_or_else(|err| {
        panic!(
            "retail TileSetData.dat missing at {} ({err}); set WOLF_DATA_ROOT to run ignored integration tests",
            path.display()
        )
    })
}

fn push_t_str(buf: &mut Vec<u8>, raw: &[u8]) {
    let len = (raw.len() + 1) as u32;
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(raw);
    buf.push(0);
}

fn build_synthetic_v210(autotile0: &[u8], tags: &[u8], passability: &[u32]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&TileSetData::MAGIC);
    buf.push(0x00);
    buf.extend_from_slice(&TileSetData::MAGIC_TYPE);
    buf.push(TileSetData::VERSION_210);
    buf.extend_from_slice(&1u32.to_le_bytes());

    push_t_str(&mut buf, b"TestSet");
    push_t_str(&mut buf, b"MapChip/base.png");
    push_t_str(&mut buf, autotile0);
    for _ in 1..31 {
        push_t_str(&mut buf, b"");
    }

    buf.push(TileSetData::SEPARATOR);
    buf.extend_from_slice(&(tags.len() as u32).to_le_bytes());
    buf.extend_from_slice(tags);

    buf.push(TileSetData::SEPARATOR);
    buf.extend_from_slice(&(passability.len() as u32).to_le_bytes());
    for &word in passability {
        buf.extend_from_slice(&word.to_le_bytes());
    }

    buf.push(TileSetData::FOOTER);
    buf
}

#[test]
fn parse_synthetic_v210_tileset_preserves_raw_bytes_and_flags() {
    let invalid_utf8 = b"MapChip/\xFFbad.png";
    // bit 0 counter, bit 2 quarter, bit 4+7 directional/quarter flags, bits 8..9 = 2
    let words = [0x0000_0295u32, 0x0000_0112u32];
    let raw = build_synthetic_v210(invalid_utf8, &[0, 7], &words);

    let parsed = TileSetData::parse(&raw).expect("synthetic TileSetData parses");
    assert_eq!(parsed.version, TileSetData::VERSION_210);
    assert_eq!(parsed.footer, TileSetData::FOOTER);
    assert_eq!(parsed.tilesets.len(), 1);
    assert!(parsed.get(1).is_none());

    let ts = parsed.get(0).expect("tileset 0 present");
    assert_eq!(ts.name, "TestSet");
    assert_eq!(ts.base_image, "MapChip/base.png");
    assert_eq!(ts.autotiles.len(), 31);
    assert_eq!(ts.raw_autotile_bytes[0], invalid_utf8);
    assert!(ts.autotiles[0].contains('\u{FFFD}'));
    assert_eq!(ts.tags, vec![0, 7]);
    assert_eq!(ts.passability.len(), 2);

    let p0 = ts.passability[0];
    assert!(p0.counter_enabled);
    assert!(p0.quarter_tile);
    assert!(p0.top_left_passable);
    assert!(p0.bottom_right_passable);
    assert_eq!(p0.na, 2);
    assert_eq!(TilePassability::decode(words[0]), p0);

    let p1 = ts.passability[1];
    assert!(p1.square);
    assert!(!p1.quarter_tile);
    assert!(p1.upwards_not_passable);
    assert_eq!(p1.na, 1);
}

#[test]
fn parse_rejects_invalid_magic_and_footer() {
    let mut bad_magic = build_synthetic_v210(b"", &[0], &[0]);
    bad_magic[0] ^= 0xFF;
    assert!(TileSetData::parse(&bad_magic).is_err());

    let mut bad_footer = build_synthetic_v210(b"", &[0], &[0]);
    *bad_footer.last_mut().expect("non-empty buffer") = 0x00;
    assert!(TileSetData::parse(&bad_footer).is_err());
}

#[test]
#[ignore = "requires retail TileSetData.dat under WOLF_DATA_ROOT or work/full"]
fn tileset_data_matches_the_spec_table() {
    let data = load_required();
    let parsed = TileSetData::parse(&data).expect("TileSetData.dat parses");

    assert_eq!(parsed.version, 210, "container version");
    assert_eq!(parsed.footer, 0xCF, "footer");
    assert_eq!(parsed.tilesets.len(), 6, "tileset count");

    // Name / base image / passability count per tileset, §7.4.
    let expected: [(&str, &str, usize); 6] = [
        ("街", "MapChip/[Base]BaseChip_pipo.png", 2376),
        ("ダンジョン", "MapChip/[Base]BaseChip_pipo.png", 2376),
        ("森", "MapChip/[Base]BaseChip_pipo.png", 2376),
        ("ワールドマップ", "MapChip/[Base]WorldMap_pipo.png", 216),
        ("kiri", "MapChip/kiri_project.png", 2376),
        ("海岸ダンジョン", "MapChip/map_parts01.png", 1648),
    ];
    for (index, (name, base, passable)) in expected.iter().enumerate() {
        let tileset = parsed.get(index as u32).expect("tileset exists");
        assert_eq!(&tileset.name, name, "tileset {index} name");
        assert_eq!(&tileset.base_image, base, "tileset {index} base image");
        assert_eq!(
            tileset.passability.len(),
            *passable,
            "tileset {index} passability count"
        );
        assert_eq!(tileset.tags.len(), *passable, "tileset {index} tag count");
        assert_eq!(
            tileset.autotiles.len(),
            31,
            "tileset {index} autotile slots"
        );
        assert_eq!(
            tileset.raw_autotile_bytes.len(),
            31,
            "tileset {index} raw autotile slots"
        );
    }
    assert!(parsed.get(6).is_none(), "no seventh tileset");

    // Tileset 0 has 15 configured autotiles; the rest of the 31 slots are empty.
    let filled: Vec<&str> = parsed.tilesets[0]
        .autotiles
        .iter()
        .filter(|name| !name.is_empty())
        .map(String::as_str)
        .collect();
    assert_eq!(filled.len(), 15, "tileset 0 configured autotiles");
    assert_eq!(
        filled,
        [
            "MapChip/[A]Grass1-Dirt1_pipo.png",
            "MapChip/[A]Grass1-Grass2_pipo.png",
            "MapChip/[A]Grass1-Grass3_pipo.png",
            "MapChip/[A]Water2_pipo.png",
            "MapChip/[A]Water5_pipo.png",
            "MapChip/[A]Water7_pipo.png",
            "MapChip/[A]WaterFall2_pipo.png",
            "MapChip/[A]Flower_pipo.png",
            "MapChip/[A]Snow_pipo.png",
            "MapChip/[A]Snow_Grass4_pipo.png",
            "MapChip/[A]Snow_Dirt2_pipo.png",
            "MapChip/[A]Ice2_pipo.png",
            "MapChip/[A]Dirt1-Dirt2_pipo.png",
            "MapChip/[A]Wall-Up1_pipo.png",
            "MapChip/[A]Wall-Up2_pipo.png",
        ],
        "tileset 0 autotile list"
    );
}

#[test]
#[ignore = "requires retail TileSetData.dat under WOLF_DATA_ROOT or work/full"]
fn passability_words_keep_their_undocumented_bits() {
    let data = load_required();
    let parsed = TileSetData::parse(&data).expect("TileSetData.dat parses");

    // The spec says bits 8..13 and the upper half must be zero; only the upper
    // half actually is. Bits 8..9 carry values 1, 2 and 3, so they are exposed
    // rather than rejected (see docs/FORMAT_SPEC.md §7.4 TODO(verify)).
    let mut saw_reserved = false;
    for (index, tileset) in parsed.tilesets.iter().enumerate() {
        for (tile, word) in tileset.passability.iter().enumerate() {
            assert_eq!(
                word.raw >> 16,
                0,
                "tileset {index} tile {tile}: upper half must be zero"
            );
            assert!(
                !word.downarrow && !word.triangle,
                "tileset {index} tile {tile}: bits 14/15 are zero in this game"
            );
            if word.na != 0 {
                saw_reserved = true;
                assert!(
                    word.na <= 3,
                    "tileset {index} tile {tile}: reserved bits {:#X}",
                    word.na
                );
            }
            // Decoding must be lossless: the raw word is kept alongside the flags.
            assert_eq!(TilePassability::decode(word.raw), *word);
        }
    }
    assert!(
        saw_reserved,
        "reserved bits 8..9 carry real data in this game's tilesets"
    );
}

#[test]
#[ignore = "requires retail TileSetData.dat under WOLF_DATA_ROOT or work/full"]
fn corrupt_autotile_names_are_kept_as_raw_bytes() {
    let data = load_required();
    let parsed = TileSetData::parse(&data).expect("TileSetData.dat parses");

    // Tileset 5 ships 9 autotile names that are not valid UTF-8 (double-encoded
    // Shift-JIS residue). They must survive as raw bytes and decode lossily.
    let mut invalid = 0;
    for tileset in &parsed.tilesets {
        for (index, raw) in tileset.raw_autotile_bytes.iter().enumerate() {
            if std::str::from_utf8(raw).is_err() {
                invalid += 1;
                assert!(
                    !tileset.autotiles[index].is_empty(),
                    "lossy decode of a corrupt name must not be dropped"
                );
                assert!(
                    tileset.autotiles[index].contains('\u{FFFD}'),
                    "lossy decode must mark the replacement characters"
                );
            }
        }
    }
    assert_eq!(invalid, 9, "corrupt autotile names in tileset 5");

    // A clean tileset round-trips byte for byte.
    for (index, raw) in parsed.tilesets[0].raw_autotile_bytes.iter().enumerate() {
        assert_eq!(
            std::str::from_utf8(raw).map(str::to_owned).ok().as_deref(),
            Some(parsed.tilesets[0].autotiles[index].as_str()),
            "tileset 0 autotile {index} round-trips"
        );
    }
}

#[test]
#[ignore = "requires retail TileSetData.dat under WOLF_DATA_ROOT or work/full"]
fn tags_match_the_measured_distribution() {
    let data = load_required();
    let parsed = TileSetData::parse(&data).expect("TileSetData.dat parses");

    // Tilesets 0-2 use no tags at all; tileset 3 tags 201 tiles with 0 and one
    // tile with each of 1..=15; tilesets 4 and 5 tag every tile with 1..=15.
    for (index, tileset) in parsed.tilesets.iter().take(3).enumerate() {
        assert!(
            tileset.tags.iter().all(|&tag| tag == 0),
            "tileset {index} has no tags"
        );
    }
    let world_map = &parsed.tilesets[3];
    let zeros = world_map.tags.iter().filter(|&&tag| tag == 0).count();
    assert_eq!(zeros, 201, "tileset 3 untagged tiles");
    for tag in 1..=15u8 {
        assert_eq!(
            world_map.tags.iter().filter(|&&t| t == tag).count(),
            1,
            "tileset 3 tag {tag}"
        );
    }
    assert!(parsed.tilesets[4].tags.contains(&8), "tileset 4 uses tag 8");
    assert!(
        parsed.tilesets[5].tags.contains(&15),
        "tileset 5 uses tag 15"
    );
}
