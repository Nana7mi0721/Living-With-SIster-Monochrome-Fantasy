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

/// Which of the three database files a `250` instruction addresses.
///
/// The value comes from `(flags >> 8) & 3` and is verified on 2,195 labelled sites
/// with zero exceptions (docs/OPCODE_250.md section 3.2): 0 = CDataBase,
/// 1 = SysDatabase, 2 = DataBase. Testing bit 9 instead would misread every
/// SysDatabase access, and testing bit 12 would misread all of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Container {
    /// The game's variable database (可変DB), `BasicData/CDataBase.dat`.
    #[default]
    CData,
    /// The engine's system database, `BasicData/SysDatabase.dat`.
    Sys,
    /// The editor's fixed database, `BasicData/DataBase.dat`.
    Data,
}

impl Container {
    /// Decodes the container from a `250` flags word.
    #[must_use]
    pub fn from_flags(flags: i32) -> Self {
        match (flags >> 8) & 3 {
            1 => Container::Sys,
            2 => Container::Data,
            _ => Container::CData,
        }
    }
}

/// The interpreter decodes `(flags >> 8) & 3` itself, so the game layer borrows that
/// decoding rather than repeating it; the two enums stay in step through this impl.
impl From<wolf_vm::DbContainer> for Container {
    fn from(value: wolf_vm::DbContainer) -> Self {
        match value {
            wolf_vm::DbContainer::CData => Container::CData,
            wolf_vm::DbContainer::Sys => Container::Sys,
            wolf_vm::DbContainer::Data => Container::Data,
        }
    }
}

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
    /// Engine-owned numeric cells, keyed (container, type, row, column).
    cells: BTreeMap<(Container, i32, i32, i32), i32>,
    /// Engine-owned string cells, same key.
    texts: BTreeMap<(Container, i32, i32, i32), String>,
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
        // The .project companions hold the editor labels, and `250` puts those labels in
        // its string arguments. They are what `resolve_labels` needs to map a script's
        // type id onto the right table row; without them only the numeric id is usable.
        if let (Some(table), Some(bytes)) = (databases.data.as_mut(), read("DataBase.project")) {
            table.apply_project_names(&bytes);
        }
        if let (Some(table), Some(bytes)) = (databases.cdata.as_mut(), read("CDataBase.project")) {
            table.apply_project_names(&bytes);
        }
        if let (Some(table), Some(bytes)) = (databases.sys.as_mut(), read("SysDatabase.project")) {
            table.apply_project_names(&bytes);
        }
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
        self.set_num(
            Container::CData,
            COORDINATE_SCALE_TYPE,
            COORDINATE_SCALE_X_ROW,
            0,
            scale_x,
        );
        self.set_num(
            Container::CData,
            COORDINATE_SCALE_TYPE,
            COORDINATE_SCALE_Y_ROW,
            0,
            scale_y,
        );
        self.set_text(
            Container::Data,
            WINDOW_SKIN_TYPE,
            WINDOW_SKIN_ROW,
            WINDOW_SKIN_COLUMN,
            WINDOW_SKIN_FILE,
        );
    }

    /// True when no database file could be read; the engine cells may still exist.
    pub fn is_empty(&self) -> bool {
        self.data.is_none() && self.cdata.is_none() && self.sys.is_none()
    }

    /// Row count of `db_type` in `container`.
    ///
    /// SysDatabase answers through its own API; the two .dat containers share the type
    /// numbering but not the type meanings, so a caller must say which one it means.
    pub fn row_count(&self, container: Container, db_type: i32) -> Option<usize> {
        match container {
            Container::Sys => self
                .sys
                .as_ref()
                .and_then(|table| Some(table.row_count(usize::try_from(db_type).ok()?))),
            _ => self
                .table(container)
                .and_then(|table| table.types.get(usize::try_from(db_type).ok()?))
                .map(|table| table.rows.len()),
        }
    }

    /// Numeric read: engine cells win, then the addressed container.
    pub fn num(&self, container: Container, db_type: i32, row: i32, column: i32) -> Option<i32> {
        if let Some(value) = self.cells.get(&(container, db_type, row, column)) {
            return Some(*value);
        }
        if row < 0 || column < 0 {
            return None;
        }
        let (row, column) = (row as usize, column as usize);
        self.table(container)
            .and_then(|table| numeric_cell(table, db_type, row, column))
    }

    /// String read: engine cells win, then the addressed container.
    pub fn text(&self, container: Container, db_type: i32, row: i32, column: i32) -> Option<&str> {
        if let Some(value) = self.texts.get(&(container, db_type, row, column)) {
            return Some(value);
        }
        if row < 0 || column < 0 {
            return None;
        }
        let (row, column) = (row as usize, column as usize);
        match container {
            Container::Sys => self
                .sys
                .as_ref()
                .and_then(|table| table.text(db_type as usize, row, column)),
            _ => self
                .table(container)
                .and_then(|table| string_cell(table, db_type, row, column)),
        }
    }

    /// Overwrites a numeric cell in the engine layer.
    pub fn set_num(
        &mut self,
        container: Container,
        db_type: i32,
        row: i32,
        column: i32,
        value: i32,
    ) {
        self.cells.insert((container, db_type, row, column), value);
    }

    /// Overwrites a string cell in the engine layer.
    pub fn set_text(
        &mut self,
        container: Container,
        db_type: i32,
        row: i32,
        column: i32,
        value: &str,
    ) {
        self.texts
            .insert((container, db_type, row, column), value.to_owned());
    }

    /// Resolves the editor labels a `250` command carries onto real table indices.
    ///
    /// The runtime type id is **not** always the table index. The eight `システム設定` /
    /// `装備可能欄数` reads in this game pass type 18 / column 22 (ev48 pc158, ev85 pc29,
    /// ev93 pc38, ev102 pc38, ev109 pc14, ev123 pc15, ev147 pc21, ev185 pc49) while the
    /// cell the author means is `DataBase` type 17 column 22, value 5: the editor counts
    /// the empty separator type 14 in its own numbering, so the ids drift. When a table
    /// has labels the names are therefore authoritative; a table without a `.project`
    /// (or a site whose labels do not resolve) keeps the numeric pair unchanged.
    pub fn resolve_labels(
        &self,
        container: Container,
        db_type: i32,
        column: i32,
        type_label: Option<&str>,
        field_label: Option<&str>,
    ) -> (i32, i32) {
        let Some(table) = self.table(container) else {
            return (db_type, column);
        };
        let named_type = type_label
            .filter(|label| !label.is_empty())
            .and_then(|label| {
                (0..table.types.len()).find(|index| table.type_name(*index) == Some(label))
            })
            .map(|index| index as i32)
            .unwrap_or(db_type);
        let named_column = field_label
            .filter(|label| !label.is_empty())
            .and_then(|label| {
                let index = usize::try_from(named_type).ok()?;
                let fields = table.types.get(index)?.fields.len();
                (0..fields).find(|column| table.field_name(index, *column) == Some(label))
            })
            .map(|index| index as i32)
            .unwrap_or(column);
        (named_type, named_column)
    }

    /// The .dat table behind one container; SysDatabase has its own type.
    fn table(&self, container: Container) -> Option<&Database> {
        match container {
            Container::CData => self.cdata.as_ref(),
            Container::Data => self.data.as_ref(),
            Container::Sys => None,
        }
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
        assert_eq!(
            databases.num(Container::CData, COORDINATE_SCALE_TYPE, 97, 0),
            Some(32)
        );
        assert_eq!(
            databases.num(Container::CData, COORDINATE_SCALE_TYPE, 98, 0),
            Some(24)
        );
        assert_eq!(
            databases.text(
                Container::Data,
                WINDOW_SKIN_TYPE,
                WINDOW_SKIN_ROW,
                WINDOW_SKIN_COLUMN
            ),
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
        assert_eq!(databases.num(Container::CData, 16, 0, 2), None);
        assert_eq!(databases.text(Container::Data, 16, 0, 2), None);
        assert_eq!(databases.row_count(Container::CData, 18), None);
        assert_eq!(databases.row_count(Container::Sys, 24), None);
    }

    #[test]
    fn the_engine_layer_wins_over_the_file() {
        let mut databases = Databases::with_engine_defaults(DEFAULT_SCREEN);
        databases.set_num(
            Container::CData,
            COORDINATE_SCALE_TYPE,
            COORDINATE_SCALE_X_ROW,
            0,
            24,
        );
        assert_eq!(
            databases.num(Container::CData, COORDINATE_SCALE_TYPE, 97, 0),
            Some(24)
        );
    }

    #[test]
    fn sysdatabase_decodes_the_face_graphic_type() {
        // Type id 24 of a `250` read is SysDatabase type 24 unchanged: 12 rows of
        // face-graphic names, `Picture/face01.png` at row 2.
        let Some(root) = have_data() else { return };
        let databases = Databases::load(&root.join("BasicData"), DEFAULT_SCREEN);
        assert!(!databases.is_empty());
        assert_eq!(
            databases.text(Container::Sys, 24, 2, 0),
            Some("Picture/face01.png")
        );
        assert_eq!(
            databases.text(Container::Sys, 24, 4, 0),
            Some("DLC_data/member_20_s.png")
        );
    }

    #[test]
    fn the_real_databases_still_answer_the_engine_cells() {
        let Some(root) = have_data() else { return };
        let databases = Databases::load(&root.join("BasicData"), DEFAULT_SCREEN);
        // CDataBase type 18 stores 0 in rows 97/98; the engine value must win.
        assert_eq!(
            databases.num(Container::CData, COORDINATE_SCALE_TYPE, 97, 0),
            Some(32)
        );
        assert_eq!(
            databases.num(Container::CData, COORDINATE_SCALE_TYPE, 98, 0),
            Some(24)
        );
        assert_eq!(
            databases.row_count(Container::CData, COORDINATE_SCALE_TYPE),
            Some(100)
        );
        // A file-backed cell still resolves in its own container.
        assert_eq!(
            databases.text(Container::Data, 16, 0, 2),
            Some("SystemFile/WindowBase.png"),
            "DataBase type 16 row 0 column 2 must still be reachable"
        );
    }

    #[test]
    fn the_interpreter_container_maps_onto_the_right_table() {
        // The VM decodes (flags >> 8) & 3 itself; this impl is the only bridge, so a
        // mismatch here would silently route 250 reads into the wrong .dat file.
        assert_eq!(
            Container::from(wolf_vm::DbContainer::CData),
            Container::CData
        );
        assert_eq!(Container::from(wolf_vm::DbContainer::Sys), Container::Sys);
        assert_eq!(Container::from(wolf_vm::DbContainer::Data), Container::Data);
    }

    #[test]
    fn the_equipment_slot_label_resolves_onto_the_admin_type() {
        // This is the site the label route exists for: eight reads pass type 18 / column
        // 22 and mean DataBase type 17 column 22, because the editor numbering counts the
        // separator type 14 differently from the file. Without labels the read returns
        // nothing and the scripts believe there are zero equipment slots.
        let Some(root) = have_data() else { return };
        let databases = Databases::load(&root.join("BasicData"), DEFAULT_SCREEN);
        let (ty, column) = databases.resolve_labels(
            Container::Data,
            18,
            22,
            Some("システム設定"),
            Some("装備可能欄数"),
        );
        assert_eq!((ty, column), (17, 22), "labels must land on DataBase type 17");
        assert_eq!(databases.num(Container::Data, ty, 0, column), Some(5));
        // A raw read of the numeric pair the script carries answers nothing, which is why
        // the labels are not optional here.
        assert_eq!(databases.num(Container::Data, 18, 0, 22), None);
    }

    #[test]
    fn an_unresolvable_label_keeps_the_numeric_pair() {
        let Some(root) = have_data() else { return };
        let databases = Databases::load(&root.join("BasicData"), DEFAULT_SCREEN);
        assert_eq!(
            databases.resolve_labels(Container::Data, 16, 2, None, None),
            (16, 2)
        );
        assert_eq!(
            databases.resolve_labels(Container::Data, 16, 2, Some("存在しない型"), Some("存在しない列")),
            (16, 2)
        );
        // Sys is answered by SysDatabase's own name table, but resolve_labels is a
        // .dat-container operation; it must leave a Sys call alone rather than guess.
        assert_eq!(databases.resolve_labels(Container::Sys, 6, 0, None, None), (6, 0));
    }
}
