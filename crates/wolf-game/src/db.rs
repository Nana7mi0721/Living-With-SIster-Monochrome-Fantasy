//! The three WOLF databases, plus the cells the engine fills in by itself.
//!
//! Opcode 250 is the only database opcode this game uses: a probe over all 550
//! common events finds 2,592 [250] instructions and **no 251/252/253 at all**, so
//! no script ever writes a database. The argument count histogram is
//! {5: 2571, 4: 16}, i.e. [type, row, column, flags, destination]; the 16
//! four-argument sites have no destination and are pure side-effect reads.
//!
//! Which file a given type means is still being settled by docs/OPCODE_250.md,
//! so the routing below is **provisional** and deliberately forgiving: a numeric
//! read tries CDataBase then DataBase, a string read tries SysDatabase, then
//! CDataBase, then DataBase. What is not provisional is that the engine owns a
//! handful of cells no script writes -- see `Databases::apply_engine_defaults`.
//!
//! Evidence for the engine-owned cells:
//!
//! * `250 [18, 97, 0, ...]` and `250 [18, 98, 0, ...]` are read by common event
//!   48 (基本システム自動初期化) and by common event 63 as 座標倍率X/Y. CDataBase
//!   type 18 rows 97/98 carry the author own note in their string column --
//!   `この値は「自動システム初期化」処理でセットされます`, "this value is set by the
//!   automatic system initialisation" -- while their numeric column is 0 in the
//!   file. So the engine computes it at boot and the file never holds it.
//! * `250 [18, 0, 24, ...]` is read by common event 63 pc25 into the picture file
//!   name of the message-window frame. `SystemFile/WindowBase.png` is the skin
//!   `crate::window` already nine-slices, and it is the only plausible answer:
//!   DataBase type 18 has zero columns in this game, so the cell cannot come from
//!   a file at all.
//!
//! The coordinate scale is NOT the tile size: it is the screen size divided by the
//! engine's virtual screen, 1024x576 / 32x24. docs/OPCODE_101.md section 3.1 proves
//! it twice over: common event 48 divides the screen system variables by 32 and 24
//! (Sys116/32, Sys117/24), and common event 63 multiplies the result back and
//! stores 32*rateX = 1024 and 24*rateY = 576 into its own self[42]/self[43].
//! With rateX = 32 and rateY = 24 common event 63 places the message frame at
//! (64, 408), 960x168, flush with the bottom-right corner of the screen -- exactly
//! where a message box belongs, and 2 px above the text origin it computes
//! (Sys1 = 64, Sys2 = 410). Feeding it the tile size 16 instead lands Sys2 at 278,
//! i.e. above the frame the same event just drew; that was the bug fixed here.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use wolf_format::database::{Database, RowValue};
use wolf_format::sysdatabase::SysDatabase;

/// `CDataBase` type the 座標倍率 (coordinate scale) lives in.
pub const COORDINATE_SCALE_TYPE: i32 = 18;
/// Row holding the X scale.
pub const COORDINATE_SCALE_X_ROW: i32 = 97;
/// Row holding the Y scale.
pub const COORDINATE_SCALE_Y_ROW: i32 = 98;
/// `CDataBase` type the message-window skin name is read from.
pub const WINDOW_SKIN_TYPE: i32 = 18;
/// Row of the message-window skin name.
pub const WINDOW_SKIN_ROW: i32 = 0;
/// Column of the message-window skin name (docs/OPCODE_250.md, ev63 pc25).
pub const WINDOW_SKIN_COLUMN: i32 = 24;
/// The skin the engine uses for the system message window.
pub const WINDOW_SKIN_FILE: &str = "SystemFile/WindowBase.png";
/// Screen size assumed when Game.dat cannot be read.
pub const DEFAULT_SCREEN: (u32, u32) = (1024, 576);
/// The engine's virtual screen, in units of one coordinate step.
pub const SCREEN_STEPS: (u32, u32) = (32, 24);

/// The coordinate scale pair (rateX, rateY) for a screen size: one step is one
/// 32nd of the width and one 24th of the height (docs/OPCODE_101.md section 3.1).
pub fn coordinate_scale(screen: (u32, u32)) -> (i32, i32) {
    let divisor = |value: u32, steps: u32| i32::try_from((value / steps).max(1)).unwrap_or(1);
    (
        divisor(screen.0, SCREEN_STEPS.0),
        divisor(screen.1, SCREEN_STEPS.1),
    )
}

/// The game databases plus the engine-owned cells layered on top of them.
#[derive(Debug, Default)]
pub struct Databases {
    data: Option<Database>,
    cdata: Option<Database>,
    sys: Option<SysDatabase>,
    /// Engine-owned numeric cells, keyed (type, row, column).
    cells: BTreeMap<(i32, i32, i32), i32>,
    /// Engine-owned string cells, same key.
    texts: BTreeMap<(i32, i32, i32), String>,
}

impl Databases {
    /// Loads `DataBase.dat`, `CDataBase.dat` and `SysDatabase.dat` from `basic`
    /// (the `BasicData` directory), then layers the engine defaults on top.
    ///
    /// A missing or corrupt file is not an error: the game still boots and the
    /// diagnostics report which cells stay unanswered.
    pub fn load(basic: &Path, screen: (u32, u32)) -> Self {
        let read = |name: &str| fs::read(basic.join(name)).ok();
        let mut databases = Databases {
            data: read("DataBase.dat").and_then(|bytes| Database::parse(&bytes).ok()),
            cdata: read("CDataBase.dat").and_then(|bytes| Database::parse(&bytes).ok()),
            sys: read("SysDatabase.dat").and_then(|bytes| SysDatabase::parse(&bytes).ok()),
            ..Databases::default()
        };
        databases.apply_engine_defaults(screen);
        databases
    }

    /// An empty store that still answers the engine-owned cells.
    pub fn with_engine_defaults(screen: (u32, u32)) -> Self {
        let mut databases = Databases::default();
        databases.apply_engine_defaults(screen);
        databases
    }

    /// Fills the cells 自動システム初期化 sets: the coordinate scale pair and the
    /// message-window skin name.
    pub fn apply_engine_defaults(&mut self, screen: (u32, u32)) {
        let (scale_x, scale_y) = coordinate_scale(screen);
        self.set_num(COORDINATE_SCALE_TYPE, COORDINATE_SCALE_X_ROW, 0, scale_x);
        self.set_num(COORDINATE_SCALE_TYPE, COORDINATE_SCALE_Y_ROW, 0, scale_y);
        self.texts.insert(
            (WINDOW_SKIN_TYPE, WINDOW_SKIN_ROW, WINDOW_SKIN_COLUMN),
            WINDOW_SKIN_FILE.to_owned(),
        );
    }

    /// True when no database file could be read; the engine cells may still exist.
    pub fn is_empty(&self) -> bool {
        self.data.is_none() && self.cdata.is_none() && self.sys.is_none()
    }

    /// Row count of `db_type` from the first table that owns the type (CDataBase
    /// first, then DataBase). SysDatabase is not consulted: the two .dat files share
    /// the type numbering but not the type meanings.
    pub fn row_count(&self, db_type: i32) -> Option<usize> {
        self.database(db_type)
            .and_then(|table| table.types.get(db_type as usize))
            .map(|table| table.rows.len())
    }

    /// Numeric read: engine cells win, then CDataBase, then DataBase.
    pub fn num(&self, db_type: i32, row: i32, column: i32) -> Option<i32> {
        if let Some(value) = self.cells.get(&(db_type, row, column)) {
            return Some(*value);
        }
        if row < 0 || column < 0 {
            return None;
        }
        let (row, column) = (row as usize, column as usize);
        self.cdata
            .as_ref()
            .and_then(|table| numeric_cell(table, db_type, row, column))
            .or_else(|| {
                self.data
                    .as_ref()
                    .and_then(|table| numeric_cell(table, db_type, row, column))
            })
    }

    /// String read: engine cells win, then SysDatabase, CDataBase, DataBase.
    pub fn text(&self, db_type: i32, row: i32, column: i32) -> Option<&str> {
        if let Some(value) = self.texts.get(&(db_type, row, column)) {
            return Some(value);
        }
        if row < 0 || column < 0 {
            return None;
        }
        let (row, column) = (row as usize, column as usize);
        self.sys
            .as_ref()
            .and_then(|table| table.text(db_type as usize, row, column))
            .or_else(|| {
                self.cdata
                    .as_ref()
                    .and_then(|table| string_cell(table, db_type, row, column))
            })
            .or_else(|| {
                self.data
                    .as_ref()
                    .and_then(|table| string_cell(table, db_type, row, column))
            })
    }

    /// Overwrites a numeric cell in the engine layer.
    pub fn set_num(&mut self, db_type: i32, row: i32, column: i32, value: i32) {
        self.cells.insert((db_type, row, column), value);
    }

    /// The table that owns `db_type`, CDataBase first.
    fn database(&self, db_type: i32) -> Option<&Database> {
        let index = usize::try_from(db_type).ok()?;
        [self.cdata.as_ref(), self.data.as_ref()]
            .into_iter()
            .flatten()
            .find(|table| table.types.get(index).is_some())
    }
}

fn numeric_cell(table: &Database, db_type: i32, row: usize, column: usize) -> Option<i32> {
    let index = usize::try_from(db_type).ok()?;
    let table = table.types.get(index)?;
    let row = table.rows.get(row)?;
    match table.value(row, column)? {
        RowValue::Num(value) => Some(value),
        RowValue::Str(_) => None,
    }
}

fn string_cell(table: &Database, db_type: i32, row: usize, column: usize) -> Option<&str> {
    let index = usize::try_from(db_type).ok()?;
    let table = table.types.get(index)?;
    let row = table.rows.get(row)?;
    match table.value(row, column)? {
        RowValue::Str(text) => Some(text),
        RowValue::Num(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn data_root() -> PathBuf {
        if let Ok(value) = std::env::var("WOLF_DATA_ROOT") {
            if !value.is_empty() {
                return PathBuf::from(value);
            }
        }
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.pop();
        path.push("work");
        path.push("full");
        path
    }

    fn have_data() -> Option<PathBuf> {
        let root = data_root();
        if root.join("BasicData").join("CDataBase.dat").is_file() {
            Some(root)
        } else {
            eprintln!("skipping: no game data at {}", root.display());
            None
        }
    }

    #[test]
    fn the_engine_owns_the_coordinate_scale_and_the_window_skin() {
        let databases = Databases::with_engine_defaults(DEFAULT_SCREEN);
        assert!(databases.is_empty());
        assert_eq!(databases.num(COORDINATE_SCALE_TYPE, 97, 0), Some(32));
        assert_eq!(databases.num(COORDINATE_SCALE_TYPE, 98, 0), Some(24));
        assert_eq!(
            databases.text(WINDOW_SKIN_TYPE, WINDOW_SKIN_ROW, WINDOW_SKIN_COLUMN),
            Some("SystemFile/WindowBase.png")
        );
    }

    #[test]
    fn the_coordinate_scale_divides_the_screen_by_32_and_24() {
        assert_eq!(coordinate_scale((1024, 576)), (32, 24));
        assert_eq!(coordinate_scale((1280, 720, )), (40, 30));
        assert_eq!(coordinate_scale((8, 8)), (1, 1));
    }

    #[test]
    fn an_empty_store_answers_only_the_engine_cells() {
        let databases = Databases::with_engine_defaults(DEFAULT_SCREEN);
        assert_eq!(databases.num(16, 0, 2), None);
        assert_eq!(databases.text(16, 0, 2), None);
        assert_eq!(databases.row_count(18), None);
    }

    #[test]
    fn the_engine_layer_wins_over_the_file() {
        let mut databases = Databases::with_engine_defaults(DEFAULT_SCREEN);
        databases.set_num(COORDINATE_SCALE_TYPE, COORDINATE_SCALE_X_ROW, 0, 24);
        assert_eq!(databases.num(COORDINATE_SCALE_TYPE, 97, 0), Some(24));
    }

    #[test]
    fn sysdatabase_decodes_the_face_graphic_type() {
        // Type id 24 of a `250` read is SysDatabase type 24 unchanged: 12 rows of
        // face-graphic names, `Picture/face01.png` at row 2.
        let Some(root) = have_data() else { return };
        let databases = Databases::load(&root.join("BasicData"), DEFAULT_SCREEN);
        assert!(!databases.is_empty());
        assert_eq!(databases.text(24, 2, 0), Some("Picture/face01.png"));
        assert_eq!(databases.text(24, 4, 0), Some("DLC_data/member_20_s.png"));
    }

    #[test]
    fn the_real_databases_still_answer_the_engine_cells() {
        let Some(root) = have_data() else { return };
        let databases = Databases::load(&root.join("BasicData"), DEFAULT_SCREEN);
        // CDataBase type 18 stores 0 in rows 97/98; the engine value must win.
        assert_eq!(databases.num(COORDINATE_SCALE_TYPE, 97, 0), Some(32));
        assert_eq!(databases.num(COORDINATE_SCALE_TYPE, 98, 0), Some(24));
        assert_eq!(databases.row_count(COORDINATE_SCALE_TYPE), Some(100));
        // A file-backed cell still resolves through the fallback chain.
        assert_eq!(
            databases.text(16, 0, 2),
            Some("SystemFile/WindowBase.png"),
            "DataBase type 16 row 0 column 2 must still be reachable"
        );
    }
}
