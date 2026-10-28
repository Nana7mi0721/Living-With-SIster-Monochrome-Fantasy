//! Integration test: every shipped `.mps` map parses byte-exactly.
//!
//! Expected numbers come from `docs/BASELINE.json` (generated from
//! `work/maps_full.json`). The test skips itself when the data tree is absent so
//! the crate stays testable without the game files.

use std::collections::BTreeMap;
use std::path::PathBuf;

use wolf_format::mps::{decode_tile, Map, Tile};

/// One row of the expected per-map table in `docs/BASELINE.json`.
struct Expect {
    file: &'static str,
    width: u32,
    height: u32,
    tileset: u32,
    has_tile_data: bool,
    events: usize,
    pages: usize,
    commands: usize,
}

const EXPECTED: [Expect; 9] = [
    Expect {
        file: "Dungeon.mps",
        width: 40,
        height: 30,
        tileset: 0,
        has_tile_data: true,
        events: 5,
        pages: 6,
        commands: 79,
    },
    Expect {
        file: "Map002.mps",
        width: 100,
        height: 30,
        tileset: 4,
        has_tile_data: true,
        events: 5,
        pages: 7,
        commands: 416,
    },
    Expect {
        file: "Map003.mps",
        width: 70,
        height: 170,
        tileset: 4,
        has_tile_data: true,
        events: 2,
        pages: 2,
        commands: 45,
    },
    Expect {
        file: "Map024.mps",
        width: 53,
        height: 30,
        tileset: 4,
        has_tile_data: false,
        events: 1,
        pages: 1,
        commands: 3,
    },
    Expect {
        file: "Map031.mps",
        width: 53,
        height: 30,
        tileset: 0,
        has_tile_data: false,
        events: 1,
        pages: 1,
        commands: 63,
    },
    Expect {
        file: "SampleMapA.mps",
        width: 64,
        height: 36,
        tileset: 4,
        has_tile_data: true,
        events: 40,
        pages: 93,
        commands: 31083,
    },
    Expect {
        file: "SampleMapA_1.mps",
        width: 64,
        height: 36,
        tileset: 4,
        has_tile_data: true,
        events: 12,
        pages: 17,
        commands: 5752,
    },
    Expect {
        file: "SampleMapB.mps",
        width: 30,
        height: 30,
        tileset: 0,
        has_tile_data: true,
        events: 6,
        pages: 6,
        commands: 36,
    },
    Expect {
        file: "TitleMap.mps",
        width: 53,
        height: 30,
        tileset: 4,
        has_tile_data: true,
        events: 1,
        pages: 1,
        commands: 133,
    },
];

/// Data root, resolved the way `wolf-cli` does it.
///
/// `cargo test` runs with the crate directory as the working directory, so a
/// relative root (the default `work/full`) is resolved against the workspace
/// root instead; an absolute `WOLF_DATA_ROOT` is used verbatim.
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

fn load(name: &str) -> Option<Vec<u8>> {
    let path = data_root().join("MapData").join(name);
    std::fs::read(path).ok()
}

#[test]
fn every_map_parses_to_its_baseline_numbers() {
    if load(EXPECTED[0].file).is_none() {
        eprintln!("skipping: no map data under {}", data_root().display());
        return;
    }

    let mut total_events = 0;
    let mut total_pages = 0;
    let mut total_commands = 0;
    let mut all_stats = wolf_format::command::SlotStats::default();
    let mut switch_bytes: BTreeMap<u8, usize> = BTreeMap::new();
    let mut triggers: BTreeMap<u8, usize> = BTreeMap::new();

    for expect in &EXPECTED {
        let data = load(expect.file).expect("map file present");
        let map = Map::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", expect.file));

        assert_eq!(map.version_header, 0x55, "{}: version header", expect.file);
        assert_eq!(map.version, 0x66, "{}: version", expect.file);
        assert_eq!(map.footer, 0x66, "{}: footer", expect.file);
        assert_eq!(map.name, "なし", "{}: map name", expect.file);
        assert_eq!(map.width, expect.width, "{}: width", expect.file);
        assert_eq!(map.height, expect.height, "{}: height", expect.file);
        assert_eq!(
            map.tileset_index, expect.tileset,
            "{}: tileset index",
            expect.file
        );
        assert_eq!(
            map.has_tile_data, expect.has_tile_data,
            "{}: tile layers",
            expect.file
        );
        assert_eq!(map.events.len(), expect.events, "{}: events", expect.file);

        if map.has_tile_data {
            assert_eq!(map.layers.len(), 3, "{}: layer count", expect.file);
            let pixels = (map.width as usize) * (map.height as usize);
            for (index, layer) in map.layers.iter().enumerate() {
                assert_eq!(layer.len(), pixels, "{}: layer {index} length", expect.file);
            }
            // Spot-check the accessor against the raw layer.
            assert_eq!(
                map.tile(0, 0, 0),
                map.layers[0].first().copied(),
                "{}: tile accessor",
                expect.file
            );
            assert_eq!(
                map.tile(0, map.width, 0),
                None,
                "{}: out-of-range x",
                expect.file
            );
            assert_eq!(
                map.tile(0, 0, map.height),
                None,
                "{}: out-of-range y",
                expect.file
            );
            assert_eq!(map.tile(3, 0, 0), None, "{}: no layer 3", expect.file);
        } else {
            assert!(map.layers.is_empty(), "{}: layers", expect.file);
        }

        let pages: usize = map.events.iter().map(|event| event.pages.len()).sum();
        let commands: usize = map
            .events
            .iter()
            .flat_map(|event| &event.pages)
            .map(|page| page.commands.len())
            .sum();
        assert_eq!(pages, expect.pages, "{}: pages", expect.file);
        assert_eq!(commands, expect.commands, "{}: commands", expect.file);

        for event in &map.events {
            for page in &event.pages {
                // The feature word is 3 in every shipped page, so the optional
                // page-transfer byte never appears.
                assert_eq!(page.features, 3, "{}: page features", expect.file);
                // Condition bytes are kept raw. The low nibble is the "enabled"
                // flag, so it is only ever 0 or 1; the high nibble is the
                // comparison operator. The full histogram is checked after the
                // totals, and the decoded meaning in
                // `page_conditions_are_variable_comparisons_without_switch_ids`.
                for &byte in &page.trigger_switches {
                    assert!(
                        byte & 0x0F <= 1,
                        "{}: switch byte {byte:#04X} has a flag nibble > 1",
                        expect.file
                    );
                    *switch_bytes.entry(byte).or_default() += 1;
                }
                *triggers.entry(page.trigger).or_default() += 1;
                assert!(
                    matches!(page.footer, 0x7A | 0x00),
                    "{}: page footer {:#04X}",
                    expect.file,
                    page.footer
                );
            }
        }

        all_stats.merge(&map.stats());
        total_events += map.events.len();
        total_pages += pages;
        total_commands += commands;
    }

    assert_eq!(total_events, 73, "total events");
    assert_eq!(total_pages, 134, "total pages");
    assert_eq!(total_commands, 37_610, "total slots");
    assert_eq!(all_stats.slots, 37_610, "stats slots");
    assert_eq!(all_stats.blank(), 7_052, "blank slots");
    assert_eq!(all_stats.real(), 30_558, "real slots");
    assert_eq!(all_stats.distinct_opcodes(), 40, "distinct opcodes");

    // Measured over all 134 pages: the 536 condition bytes take exactly seven
    // values, and only trigger conditions 0-3 appear (4 = event-touch is unused
    // in this game, so the loader must not assume the whole documented range).
    assert_eq!(
        switch_bytes.into_iter().collect::<Vec<_>>(),
        vec![
            (0x10, 2),
            (0x11, 25),
            (0x20, 324),
            (0x21, 173),
            (0x31, 3),
            (0x50, 2),
            (0x51, 7),
        ],
        "condition byte histogram"
    );
    assert_eq!(
        triggers.into_iter().collect::<Vec<_>>(),
        vec![(0, 14), (1, 26), (2, 90), (3, 4)],
        "trigger condition histogram"
    );
}

#[test]
fn the_two_pages_with_a_zero_terminator_are_tolerated() {
    // Dungeon.mps offset 17716 and Map024.mps offset 234 end their last page with
    // 0x00 instead of the documented 0x7A; both must still load.
    for (file, expected_zero_pages) in [("Dungeon.mps", 1), ("Map024.mps", 1)] {
        let Some(data) = load(file) else {
            eprintln!("skipping: no map data under {}", data_root().display());
            return;
        };
        let map = Map::parse(&data).unwrap_or_else(|e| panic!("{file}: {e}"));
        let zero = map
            .events
            .iter()
            .flat_map(|event| &event.pages)
            .filter(|page| page.footer == 0x00)
            .count();
        assert_eq!(zero, expected_zero_pages, "{file}: tolerated pages");
    }
}

#[test]
fn maps_without_tiles_are_flagged() {
    let Some(data) = load("Map024.mps") else {
        eprintln!("skipping: no map data under {}", data_root().display());
        return;
    };
    for file in ["Map024.mps", "Map031.mps"] {
        let data = std::fs::read(data_root().join("MapData").join(file)).expect("map file");
        let map = Map::parse(&data).expect("map parses");
        assert!(!map.has_tile_data, "{file}: has_tile_data");
        assert!(map.layers.is_empty(), "{file}: no layers");
        // The header still declares a size, it just has no pixel data.
        assert_eq!(map.width, 53, "{file}: width");
        assert_eq!(map.height, 30, "{file}: height");
    }
    // Sanity: a tiled map of the same size does have layers.
    let tiled = std::fs::read(data_root().join("MapData").join("TitleMap.mps")).expect("map file");
    let tiled = Map::parse(&tiled).expect("map parses");
    assert!(tiled.has_tile_data);
    let _ = data;
}

#[test]
fn tile_words_decode_as_documented() {
    let Some(data) = load("SampleMapA.mps") else {
        eprintln!("skipping: no map data under {}", data_root().display());
        return;
    };
    let map = Map::parse(&data).expect("map parses");
    // Measured first tile of the surface layer: autotile 1 with all four
    // quadrants set to variant 4 (104444 = 1*100000 + 4444).
    let first = map.tile(0, 0, 0).expect("tile present");
    assert_eq!(first, 104_444, "first surface tile");
    assert_eq!(
        decode_tile(first),
        Tile::Autotile {
            id: 1,
            corners: [4, 4, 4, 4]
        },
        "raw word {first}"
    );
    // TitleMap mixes autotile 1 variant 0 with the same variant-4 tile.
    let title = std::fs::read(data_root().join("MapData").join("TitleMap.mps")).expect("map file");
    let title = Map::parse(&title).expect("map parses");
    let mut saw_variant_zero = false;
    for y in 0..title.height {
        for x in 0..title.width {
            if title.tile(0, x, y) == Some(100_000) {
                saw_variant_zero = true;
            }
        }
    }
    assert!(saw_variant_zero, "TitleMap uses autotile 1 variant 0");
    assert_eq!(
        decode_tile(100_000),
        Tile::Autotile {
            id: 1,
            corners: [0, 0, 0, 0]
        }
    );

    // SampleMapB contains plain base tiles as well as autotiles.
    let data = std::fs::read(data_root().join("MapData").join("SampleMapB.mps")).expect("map file");
    let sample_b = Map::parse(&data).expect("map parses");
    let mut saw_base = false;
    let mut saw_autotile = false;
    for layer in 0..3 {
        for y in 0..sample_b.height {
            for x in 0..sample_b.width {
                let raw = sample_b.tile(layer, x, y).expect("tile present");
                match decode_tile(raw) {
                    Tile::Base(_) => saw_base = true,
                    Tile::Autotile { .. } => saw_autotile = true,
                    Tile::Empty => panic!("no empty tile in this map, got {raw:#010X}"),
                }
            }
        }
    }
    assert!(saw_base, "SampleMapB has base tiles");
    assert!(saw_autotile, "SampleMapB has autotiles");
}

/// The four condition bytes of a page are `operator << 4 | enabled` for the
/// *variable* condition stored in the same slot. There is no switch id: the block
/// is a fixed 1 + 4 + 4*4 + 4*4 bytes (WolfTL `Map.hpp:55`), so nothing could hold
/// one, and the operators that actually appear (`>=`, `==`, `<=`, `!=`) are
/// numeric comparisons rather than the on/off test a switch would need.
#[test]
fn page_conditions_are_variable_comparisons_without_switch_ids() {
    if load(EXPECTED[0].file).is_none() {
        eprintln!("skipping: no map data under {}", data_root().display());
        return;
    }

    let mut slots = 0usize;
    let mut enabled = 0usize;
    let mut operators: BTreeMap<u8, usize> = BTreeMap::new();
    let mut enabled_operators: BTreeMap<u8, usize> = BTreeMap::new();
    let mut enabled_per_page: BTreeMap<usize, usize> = BTreeMap::new();
    let mut stale_disabled = 0usize;

    for expect in &EXPECTED {
        let data = load(expect.file).expect("map file present");
        let map = Map::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", expect.file));
        for event in &map.events {
            for page in &event.pages {
                let conditions = page.conditions();
                assert_eq!(conditions.len(), 4, "{}: condition slots", expect.file);
                let mut used = 0;
                for (index, condition) in conditions.iter().enumerate() {
                    // `conditions()` is a pure decode of the raw arrays.
                    let byte = page.trigger_switches[index];
                    assert_eq!(condition.operator, byte >> 4, "{}: operator", expect.file);
                    assert_eq!(
                        condition.enabled,
                        byte & 0x0F == 1,
                        "{}: enabled flag",
                        expect.file
                    );
                    assert_eq!(
                        condition.variable, page.trigger_variables[index],
                        "{}: variable",
                        expect.file
                    );
                    assert_eq!(
                        condition.value, page.trigger_values[index],
                        "{}: value",
                        expect.file
                    );
                    slots += 1;
                    *operators.entry(condition.operator).or_default() += 1;
                    if condition.enabled {
                        used += 1;
                        enabled += 1;
                        *enabled_operators.entry(condition.operator).or_default() += 1;
                    } else if (condition.variable, condition.value) != (1_000_000, 0) {
                        // Disabled slots keep whatever the editor left behind.
                        stale_disabled += 1;
                    }
                }
                *enabled_per_page.entry(used).or_default() += 1;
            }
        }
    }

    assert_eq!(slots, 536, "condition slots over all pages");
    assert_eq!(enabled, 208, "enabled conditions");
    assert_eq!(
        operators.into_iter().collect::<Vec<_>>(),
        vec![(1, 27), (2, 497), (3, 3), (5, 9)],
        "operator histogram over every slot"
    );
    assert_eq!(
        enabled_operators.into_iter().collect::<Vec<_>>(),
        vec![(1, 25), (2, 173), (3, 3), (5, 7)],
        "operator histogram over enabled slots"
    );
    assert_eq!(
        enabled_per_page.into_iter().collect::<Vec<_>>(),
        vec![(0, 19), (1, 68), (2, 15), (3, 18), (4, 14)],
        "enabled conditions per page"
    );
    assert_eq!(stale_disabled, 24, "disabled slots with leftover id/value");
}

/// One page per event is on screen: the *highest* page whose enabled conditions all
/// hold (official editor documentation, `docs/FORMAT_SPEC.md` §5.6.3). The trigger,
/// graphic and commands all belong to that page, which is why 8 of the 25 shipped
/// multi-page events mix triggers between their pages.
#[test]
fn the_highest_matching_page_decides_what_the_event_is() {
    let Some(sample) = load("SampleMapA.mps") else {
        eprintln!("skipping: no map data under {}", data_root().display());
        return;
    };
    let sample = Map::parse(&sample).expect("SampleMapA parses");
    let event = |id: u32| {
        sample
            .events
            .iter()
            .find(|event| event.id == id)
            .unwrap_or_else(|| panic!("SampleMapA has no event {id}"))
    };

    // Event 9: ten pages, page i keyed `var 2000062 == i + 1`, page 0 adding three
    // more conditions. Every state of the variable picks a different page.
    let dispatch = event(9);
    assert_eq!(dispatch.pages.len(), 10);
    let state = |value: i32| {
        let variables = BTreeMap::from([
            (2_000_062u32, value),
            (2_000_140, 0),
            (2_000_164, 0),
            (2_000_364, 0),
        ]);
        let page = dispatch.active_page(|id| variables.get(&id).copied().unwrap_or(0));
        page.map(|page| (page.trigger, page.commands.len()))
    };
    assert_eq!(state(1).map(|(trigger, _)| trigger), Some(2));
    assert_eq!(state(5).map(|(trigger, _)| trigger), Some(2));
    // Out of range: no page matches, so the event is not on the map at all.
    assert_eq!(
        dispatch.active_page_index(|id| if id == 2_000_062 { 11 } else { 0 }),
        None
    );
    assert_eq!(
        dispatch.active_page_index(|id| if id == 2_000_062 { 1 } else { 0 }),
        Some(0)
    );

    // Event 10 continues the same dispatch for states 11..16.
    let continuation = event(10);
    assert_eq!(continuation.pages.len(), 6);
    assert_eq!(
        continuation.active_page_index(|id| if id == 2_000_062 { 16 } else { 0 }),
        Some(5)
    );
    assert_eq!(
        continuation.active_page_index(|id| if id == 2_000_062 { 17 } else { 0 }),
        None
    );

    // Event 8 mixes a parallel fallback page 0 with eight autorun pages keyed
    // `var 1100009 == 1..8`: the same event is a parallel event in one state and an
    // autorun event in another, so the trigger must come from the active page.
    let mixed = event(8);
    assert_eq!(mixed.pages.len(), 9);
    assert_eq!(mixed.pages[0].trigger, 2);
    assert!(mixed.pages[1..].iter().all(|page| page.trigger == 1));
    let fallback = |id: u32| match id {
        2_000_340 => 1,
        2_000_350 | 2_000_035 | 1_100_009 => 0,
        _ => 0,
    };
    assert_eq!(mixed.active_page_index(fallback), Some(0));
    assert_eq!(
        mixed
            .active_page(|id| if id == 1_100_009 { 3 } else { fallback(id) })
            .map(|page| page.trigger),
        Some(1)
    );

    // A write in one page's command stream drives the next page's condition: Dungeon
    // event 3 sets self variable 1100000 = 1, Map002 event 3 sets global 2000009 = 2
    // while its own page needs 1.
    let dungeon = std::fs::read(data_root().join("MapData").join("Dungeon.mps")).expect("map file");
    let dungeon = Map::parse(&dungeon).expect("Dungeon parses");
    let dungeon_event = dungeon
        .events
        .iter()
        .find(|event| event.id == 3)
        .expect("Dungeon event 3");
    assert_eq!(dungeon_event.pages.len(), 2);
    assert!(dungeon_event.pages[0]
        .conditions()
        .iter()
        .all(|c| !c.enabled));
    assert_eq!(dungeon_event.active_page_index(|_| 0), Some(0));
    assert_eq!(
        dungeon_event.active_page_index(|id| if id == 1_100_000 { 1 } else { 0 }),
        Some(1)
    );

    let map002 = std::fs::read(data_root().join("MapData").join("Map002.mps")).expect("map file");
    let map002 = Map::parse(&map002).expect("Map002 parses");
    let page_switch = map002
        .events
        .iter()
        .find(|event| event.id == 3)
        .expect("Map002 event 3");
    assert_eq!(page_switch.pages.len(), 2);
    assert_eq!(
        page_switch.active_page_index(|id| if id == 2_000_009 { 1 } else { 0 }),
        Some(0)
    );
    assert_eq!(
        page_switch.active_page_index(|id| if id == 2_000_009 { 2 } else { 0 }),
        Some(1)
    );
    assert_eq!(page_switch.active_page_index(|_| 0), None);

    // The eight multi-page events whose pages disagree about the trigger.
    let mut mixed_events = Vec::new();
    for expect in &EXPECTED {
        let data = load(expect.file).expect("map file present");
        let map = Map::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", expect.file));
        for event in &map.events {
            let distinct: BTreeMap<u8, ()> =
                event.pages.iter().map(|page| (page.trigger, ())).collect();
            if event.pages.len() > 1 && distinct.len() > 1 {
                mixed_events.push((expect.file, event.id));
            }
        }
    }
    assert_eq!(
        mixed_events,
        vec![
            ("SampleMapA.mps", 4),
            ("SampleMapA.mps", 8),
            ("SampleMapA.mps", 11),
            ("SampleMapA.mps", 14),
            ("SampleMapA.mps", 18),
            ("SampleMapA.mps", 21),
            ("SampleMapA.mps", 22),
            ("SampleMapA.mps", 24),
        ],
        "multi-page events with mixed triggers"
    );
}
