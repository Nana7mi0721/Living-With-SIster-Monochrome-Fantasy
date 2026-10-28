//! Integration test: the three database files parse byte-exactly.
//!
//! Expected numbers come from `docs/FORMAT_SPEC.md` §6.3 and `docs/BASELINE.json`.

use std::path::PathBuf;

use wolf_format::database::{Database, FieldKind, RowValue};

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

fn load(name: &str) -> Option<Vec<u8>> {
    std::fs::read(data_root().join("BasicData").join(name)).ok()
}

/// Walk a row's field table in logical column order, pulling from the numeric
/// and string cursors as the table says.
fn columns<'a>(
    db_type: &wolf_format::database::DatabaseType,
    row: &'a wolf_format::database::Row,
) -> Vec<RowValue<'a>> {
    (0..db_type.fields.len())
        .filter_map(|column| db_type.value(row, column))
        .collect()
}

#[test]
fn database_files_match_their_baseline_shape() {
    // name, type count, type-0 field count, type-0 numeric fields, type-0 string
    // fields, type-0 row count — §6.3.
    let expected: [(&str, usize, usize, usize, usize, usize); 3] = [
        ("DataBase.dat", 40, 18, 14, 4, 44),
        ("CDataBase.dat", 30, 51, 46, 5, 17),
        ("SysDatabase.dat", 28, 8, 4, 4, 50),
    ];

    let mut loaded_any = false;
    for (name, types, fields, numbers, strings, rows) in expected {
        let Some(data) = load(name) else {
            eprintln!("skipping {name}: no data under {}", data_root().display());
            continue;
        };
        loaded_any = true;
        let parsed = Database::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(parsed.version, 0xC2, "{name}: version");
        assert_eq!(parsed.footer, 0xC2, "{name}: footer");
        assert_eq!(parsed.types.len(), types, "{name}: type count");

        let first = &parsed.types[0];
        assert_eq!(first.fields.len(), fields, "{name}: type 0 field count");
        assert_eq!(
            first.numeric_columns(),
            numbers,
            "{name}: type 0 numeric fields"
        );
        assert_eq!(
            first.string_columns(),
            strings,
            "{name}: type 0 string fields"
        );
        assert_eq!(first.rows.len(), rows, "{name}: type 0 row count");

        // Every row must carry exactly the columns the field table declares.
        for (index, row) in first.rows.iter().enumerate() {
            assert_eq!(
                row.numbers.len(),
                numbers,
                "{name}: row {index} numeric columns"
            );
            assert_eq!(
                row.strings.len(),
                strings,
                "{name}: row {index} string columns"
            );
        }
        // And every other type's rows must be consistent with its own table.
        for (type_index, db_type) in parsed.types.iter().enumerate() {
            for (row_index, row) in db_type.rows.iter().enumerate() {
                assert_eq!(
                    row.numbers.len(),
                    db_type.numeric_columns(),
                    "{name}: type {type_index} row {row_index} numeric columns"
                );
                assert_eq!(
                    row.strings.len(),
                    db_type.string_columns(),
                    "{name}: type {type_index} row {row_index} string columns"
                );
            }
        }
    }
    assert!(
        loaded_any,
        "no database files under {}",
        data_root().display()
    );
}

#[test]
fn data_base_type_zero_has_the_documented_field_table() {
    let Some(data) = load("DataBase.dat") else {
        eprintln!("skipping: no data under {}", data_root().display());
        return;
    };
    let parsed = Database::parse(&data).expect("DataBase.dat parses");
    let first = &parsed.types[0];

    assert_eq!(first.data_id_method, 1, "type 0 data id method");
    let raw: Vec<u32> = first.fields.iter().map(FieldKind::raw).collect();
    assert_eq!(
        raw,
        vec![
            2000, 2001, 1000, 1001, 1002, 1003, 1004, 1005, 2002, 1006, 2003, 1007, 1008, 1009,
            1010, 1011, 1012, 1013
        ],
        "type 0 field positions"
    );
    assert_eq!(first.fields[0], FieldKind::Str(0));
    assert_eq!(first.fields[2], FieldKind::Num(0));
    assert_eq!(first.fields[17], FieldKind::Num(13));

    // Measured first row, §6.3.
    let row = columns(first, &first.rows[0]);
    assert_eq!(
        row,
        vec![
            RowValue::Str("什么都不做"),
            RowValue::Str(""),
            RowValue::Num(0),
            RowValue::Num(10),
            RowValue::Num(0),
            RowValue::Num(0),
            RowValue::Num(100),
            RowValue::Num(0),
            RowValue::Str(""),
            RowValue::Num(0),
            RowValue::Str(""),
            RowValue::Num(0),
            RowValue::Num(0),
            RowValue::Num(0),
            RowValue::Num(0),
            RowValue::Num(0),
            RowValue::Num(100),
            RowValue::Num(0),
        ],
        "type 0 row 0"
    );
}

#[test]
fn sys_database_type_zero_is_the_map_list() {
    let Some(data) = load("SysDatabase.dat") else {
        eprintln!("skipping: no data under {}", data_root().display());
        return;
    };
    let parsed = Database::parse(&data).expect("SysDatabase.dat parses");
    let first = &parsed.types[0];
    assert_eq!(first.data_id_method, 0, "type 0 data id method");
    assert_eq!(
        first.fields.iter().map(FieldKind::raw).collect::<Vec<_>>(),
        vec![2000, 1000, 1001, 1002, 1003, 2001, 2002, 2003],
        "type 0 field positions"
    );

    // 9 .mps files ship on disk, but only these 6 are registered — and in this
    // order, which is neither the file order nor the alphabetical order.
    let settings = parsed.map_settings();
    let paths: Vec<&str> = settings.iter().map(|s| s.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "MapData/TitleMap.mps",
            "MapData/SampleMapA.mps",
            "MapData/SampleMapA_1.mps",
            "MapData/Map003.mps",
            "MapData/Map024.mps",
            "MapData/Map031.mps",
        ],
        "registered maps"
    );
    for setting in &settings {
        assert_eq!(
            setting.numbers.len(),
            4,
            "{}: numeric columns",
            setting.path
        );
        assert_eq!(setting.strings.len(), 4, "{}: string columns", setting.path);
        assert_eq!(
            setting.strings[0], setting.path,
            "{}: path is string column 0",
            setting.path
        );
    }
    // Row 0 of the table: the title map, with -1 sentinels in the three ids.
    assert_eq!(settings[0].numbers, vec![-1, -1, 0, -1], "TitleMap row");
    assert_eq!(settings[1].numbers, vec![-2, -1, 0, 2], "SampleMapA row");

    // The other 44 rows are empty slots and must not appear in the list.
    assert_eq!(first.rows.len(), 50, "type 0 row count");
    let empty = first
        .rows
        .iter()
        .filter(|row| row.strings.first().is_none_or(String::is_empty))
        .count();
    assert_eq!(empty, 44, "unregistered rows");
}

#[test]
fn databases_with_no_types_still_parse() {
    // A truncated header must fail loudly rather than yield an empty database.
    let empty = Database::parse(&[]);
    assert!(empty.is_err(), "empty buffer must not parse");
    let header_only = Database::parse(&[
        0x00, 0x57, 0x00, 0x00, 0x4F, 0x4C, 0x55, 0x46, 0x4D, 0x00, 0xC2,
    ]);
    assert!(
        header_only.is_err(),
        "header without a count must not parse"
    );
}
