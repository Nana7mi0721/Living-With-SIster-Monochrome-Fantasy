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

/// `.project` companions carry the labels the `.dat` files lack, one record per
/// type in `.dat` order (§6.8).
///
/// These labels are the keys `docs/OPCODE_250.md` relies on for database reads —
/// `stringArgs[1]` is the type label and `stringArgs[3]` the field label — so
/// they must come from the editor's own file rather than from a hard-coded table.
#[test]
fn project_names_align_with_data_base() {
    let (Some(data), Some(project)) = (load("DataBase.dat"), load("DataBase.project")) else {
        eprintln!(
            "skipping DataBase.project: no data under {}",
            data_root().display()
        );
        return;
    };
    let mut database = Database::parse(&data).expect("DataBase.dat parses");
    // The container on its own has no labels at all.
    assert_eq!(database.type_name(0), None, "the .dat has no type label");
    assert_eq!(
        database.field_name(0, 0),
        None,
        "the .dat has no field label"
    );

    assert_eq!(
        database.apply_project_names(&project),
        40,
        "the companion describes every type of the .dat"
    );
    assert_eq!(
        database.names.named_types(),
        23,
        "17 types are unnamed separators"
    );
    assert_eq!(database.names.named_fields(), 592, "field labels");
    assert_eq!(database.names.named_rows(), 723, "row labels");

    // Every one of the 40 types, in `.dat` order.
    let expected: [Option<&str>; 40] = [
        Some("技能"),
        Some("┗ アニメーション"),
        Some("アイテム"),
        Some("武器"),
        Some("防具"),
        Some("┗装備タイプ"),
        Some("戦闘コマンド"),
        Some("属性名の設定"),
        Some("状態設定"),
        Some("敵キャラ個体デ—タ"),
        Some("┣ 敵行動AI"),
        Some("┣ 属性耐性"),
        Some("┗ 状態耐性"),
        Some("敵グループ"),
        Some("------------------------"),
        Some("用語設定"),
        Some("画像/音声設定"),
        Some("システム設定"),
        None,
        None,
        Some("◇吹き出しアイコン"),
        None,
        None,
        None,
        None,
        Some("ランダム肩書"),
        Some("メッセージウィンドウ種類"),
        Some("探索ふきだし"),
        Some("目標テキスト"),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    let got: Vec<Option<&str>> = (0..40).map(|ty| database.type_name(ty)).collect();
    assert_eq!(got, expected, "type labels");

    // Field and row labels of the tables this port reads.
    assert_eq!(database.field_name(0, 0), Some("技能の名前"), "skill name");
    assert_eq!(database.field_name(0, 1), Some("スキル説明"), "skill help");
    assert_eq!(database.field_name(2, 0), Some("アイテム名"), "item name");
    assert_eq!(database.field_name(0, 18), None, "past the last column");
    assert_eq!(
        database.field_name(0, 2000),
        None,
        "a field id is not a column"
    );
    assert_eq!(
        database.row_name(0, 0),
        Some("什么都不做"),
        "empty skill row"
    );
    assert_eq!(database.row_name(0, 1), Some("观察状况"), "observe row");
    assert_eq!(database.row_name(2, 0), Some("绿茶"), "first item");
    assert_eq!(
        database.row_name(2, 4),
        None,
        "row 4 of the item table is unnamed"
    );
    assert_eq!(database.row_name(0, 44), None, "past the last row");

    // Types 18..=24 and 30..=31 are unnamed `------------------------` separators,
    // and for 18..=24 and 30..=31 the editor's record keeps exactly one field that
    // the exported table dropped, so no field label may be attached there.
    for ty in [18usize, 19, 21, 22, 23, 24, 30, 31] {
        assert_eq!(database.type_name(ty), None, "type {ty} is unnamed");
        assert_eq!(
            database.names.record(ty).map(|record| record.fields.len()),
            Some(1),
            "type {ty} keeps one field in the editor only"
        );
        assert_eq!(
            database.types[ty].fields.len(),
            0,
            "type {ty} has no field in the .dat"
        );
        assert_eq!(
            database.field_name(ty, 0),
            None,
            "type {ty} must not get a label for a column it does not have"
        );
    }
    assert_eq!(database.type_name(40), None, "past the last type");
}

/// The same companion walk on the second database: 30 types, 973 field labels and
/// 723 row labels, and two types where the editor keeps a field the table dropped.
#[test]
fn project_names_align_with_c_data_base() {
    let (Some(data), Some(project)) = (load("CDataBase.dat"), load("CDataBase.project")) else {
        eprintln!(
            "skipping CDataBase.project: no data under {}",
            data_root().display()
        );
        return;
    };
    let mut database = Database::parse(&data).expect("CDataBase.dat parses");
    assert_eq!(
        database.apply_project_names(&project),
        30,
        "every type found"
    );
    assert_eq!(database.names.named_types(), 24, "6 types are unnamed");
    assert_eq!(database.names.named_fields(), 973, "field labels");
    assert_eq!(database.names.named_rows(), 723, "row labels");

    let expected: [Option<&str>; 30] = [
        Some("主人公ステータス"),
        Some("┣ 技能習得Lv"),
        Some("┣ 主人公行動AI"),
        Some("┣ 属性耐性"),
        Some("┣ 状態耐性"),
        Some("┗ 状態リスト"),
        Some("パーティー情報"),
        Some("┣所持アイテム個数"),
        Some("┣所持武器個数"),
        Some("┗所持防具個数"),
        Some("戦闘時一時ステータス"),
        Some("×┣ 属性耐性計算結果"),
        Some("×┣ 状態耐性計算結果"),
        Some("×┣ 状態リスト"),
        Some("×┣ コマンド処理リスト"),
        Some("×┗ お宝リスト"),
        Some("× お店一時処理用"),
        Some("×主人公一時DB"),
        Some("基本システム用変数"),
        Some("万能ウィンドウ一時DB"),
        Some("×ドロップアイテムリスト"),
        Some("メンバー情報"),
        None,
        None,
        None,
        Some("追加キーコンフィグ設定"),
        None,
        Some("カスタム選択肢"),
        None,
        None,
    ];
    let got: Vec<Option<&str>> = (0..30).map(|ty| database.type_name(ty)).collect();
    assert_eq!(got, expected, "type labels");

    assert_eq!(database.field_name(0, 0), Some("头衔1"), "hair style field");
    assert_eq!(database.field_name(1, 0), Some("<能力>覚えるレベル"));
    assert_eq!(database.row_name(0, 0), Some("勇者先生"), "first hero");
    assert_eq!(database.row_name(0, 5), Some("盗贼先生"), "sixth hero");
    assert_eq!(database.row_name(1, 0), None, "type 1 names no row");
    // Types 22 and 26 keep one field in the editor that the .dat dropped.
    for ty in [22usize, 26] {
        assert_eq!(database.type_name(ty), None, "type {ty} is unnamed");
        assert_eq!(database.field_name(ty, 0), None, "type {ty} field label");
    }
}

/// A companion only ever labels the database it belongs to.
#[test]
fn a_companion_for_another_database_names_nothing() {
    let (Some(data), Some(project)) = (load("DataBase.dat"), load("DataBase.project")) else {
        eprintln!(
            "skipping DataBase.project: no data under {}",
            data_root().display()
        );
        return;
    };
    let mut database = Database::parse(&data).expect("DataBase.dat parses");

    // Another game version would carry a different type count; the count is the
    // only thing tying a companion to its .dat, so it is checked first.
    let mut foreign = project.clone();
    foreign[0..4].copy_from_slice(&7u32.to_le_bytes());
    assert_eq!(
        database.apply_project_names(&foreign),
        0,
        "wrong type count"
    );
    assert_eq!(database.type_name(0), None, "no label survives a mismatch");
    assert_eq!(database.names.named_fields(), 0);

    assert_eq!(database.apply_project_names(&[]), 0, "empty companion");

    // The SysDatabase companion has the same shape but not the same type list.
    if let Some(other) = load("SysDatabase.project") {
        assert_eq!(database.apply_project_names(&other), 0, "wrong database");
        assert_eq!(database.type_name(0), None);
    }

    // And a truncated record stops the walk instead of inventing labels.
    let cut = &project[..project.len() / 2];
    let mut database = Database::parse(&data).expect("DataBase.dat parses");
    let partial = database.apply_project_names(cut);
    assert!(
        partial < 40,
        "a truncated companion claims fewer types: {partial}"
    );
    assert_eq!(
        database.type_name(0),
        Some("技能"),
        "the first record still reads"
    );
}
