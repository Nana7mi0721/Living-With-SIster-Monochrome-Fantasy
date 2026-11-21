//! Battle database seed: the CData cells a fresh game never initialises, so the
//! real battle command line can run (see docs/research/M7_BATTLE_SEED.md).
//!
//! A new game in this build never writes four runtime tables the battle events read:
//!
//! * `CData type 0` row 12 (party-leader status row) columns 47..54 (コマンド1..8) — only
//!   CE26/CE77/CE78 write them, and CE26 has no caller in the shipped graph.
//! * `CData type 7` row 0 column 0 (所持アイテム個数, item 0 count) — CE152 marks every
//!   real attack unusable while this is 0 (see docs/research/M6_BATTLE_COMMANDS.md).
//! * `CData type 8` rows 0..2 column 0 (所持武器個数) — empty in the bundled saves.
//! * `CData type 18` row 4 column 0 (the party count CE190 places allies from) — 0.
//! * `CData type 18` row 69 column 0 (the enemy count CE190/CE162 read) — 0, so the
//!   enemy loop is empty and CE162 敵対象選択実行 spins forever on its wait key.
//!
//! None of these cells is written by `Databases::load` (they live only in a save's
//! variable database) and none is written by the shipped CE26 bootstrap, so a game
//! started without a hand-edited save has an empty command list. `seed` writes the
//! minimal new-game party template as a data precondition; it does not fabricate a
//! menu row — CE152 still decides usability from these values.
//!
//! The values mirror the proven `seed_party` block in
//! crates/wolf-game/tests/m6_battle_commands.rs and the shipped `System.sav` layout
//! (probe: type0 row12 c47..c50 already 0, c51.. absent; type7 row0 c0 = 0; type8
//! rows all 0; type18 row4 all 0 — so a save merge would be a no-op and the constants
//! below are what actually make the line boot).

use crate::db::{Container, Databases};
use crate::Game;

/// One numeric `CData` cell seeded into the engine cell layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// `CData` table id.
    pub db_type: i32,
    /// Row inside that table.
    pub row: i32,
    /// Column inside that row.
    pub column: i32,
    /// Value to write when the cell is not already meaningful.
    pub value: i32,
}

/// コマンド1..8 for the party leader: `Data type 6` row ids of the real battle commands
/// (蓄力/防御/强斩/反击/普通攻击/empty...). CE143 copies type0 row12 cols 47..54 to the
/// battle scratch table, and CE152 walks them to build the command window.
pub const COMMAND_SLOTS: [Cell; 8] = [
    Cell {
        db_type: 0,
        row: 12,
        column: 47,
        value: 3,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 48,
        value: 4,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 49,
        value: 6,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 50,
        value: 7,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 51,
        value: 0,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 52,
        value: 0,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 53,
        value: 0,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 54,
        value: 0,
    },
];

/// Leader stat block CE143 copies into the battle scratch row (maxHP, HP, attack,
/// defence, ...). A fresh game leaves it 0, so the copied battle HP would be 0.
pub const LEADER_STATS: [Cell; 12] = [
    Cell {
        db_type: 0,
        row: 12,
        column: 20,
        value: 100,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 21,
        value: 100,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 22,
        value: 50,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 23,
        value: 50,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 24,
        value: 20,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 25,
        value: 15,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 26,
        value: 10,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 27,
        value: 30,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 28,
        value: 30,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 9,
        value: 20,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 10,
        value: 10,
    },
    Cell {
        db_type: 0,
        row: 12,
        column: 4,
        value: 10,
    },
];

/// One copy of item 0, so CE152 pc91..pc93 sees a non-zero stock and keeps real attacks
/// usable. Without it every real command row is written back as unusable.
pub const ITEM_ZERO: Cell = Cell {
    db_type: 7,
    row: 0,
    column: 0,
    value: 1,
};

/// Starter weapon stock (所持武器個数) rows 0..2, so equipment lookups are not empty.
pub const STARTER_WEAPONS: [Cell; 3] = [
    Cell {
        db_type: 8,
        row: 0,
        column: 0,
        value: 1,
    },
    Cell {
        db_type: 8,
        row: 1,
        column: 0,
        value: 1,
    },
    Cell {
        db_type: 8,
        row: 2,
        column: 0,
        value: 1,
    },
];

/// Party-member count (type 18 row 4 column 0) CE190 places allies from.
pub const PARTY_COUNT: Cell = Cell {
    db_type: 18,
    row: 4,
    column: 0,
    value: 1,
};

/// Enemy count (type 18 row 69 column 0). CE190 pc36 copies it to the enemy loop
/// counter and CE162 pc12 reads it before it will place/select a target. A fresh
/// game leaves it 0, which makes CE190's enemy loop empty and CE162 spin
/// pc49<->pc50 forever. One enemy is enough for a single-target selection.
pub const ENEMY_COUNT: Cell = Cell {
    db_type: 18,
    row: 69,
    column: 0,
    value: 1,
};

/// Every cell `seed` writes, in a stable order.
pub fn seed_cells() -> Vec<Cell> {
    let mut cells = Vec::with_capacity(26);
    cells.push(PARTY_COUNT);
    cells.push(ENEMY_COUNT);
    cells.extend_from_slice(&COMMAND_SLOTS);
    cells.extend_from_slice(&LEADER_STATS);
    cells.push(ITEM_ZERO);
    cells.extend_from_slice(&STARTER_WEAPONS);
    cells
}

/// One **text** `CData` cell seeded into the engine text layer.
///
/// Numeric cells go through [`seed`]'s arithmetic store; text only makes sense on a
/// string field, so it has its own tiny table for the command-menu labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextCell {
    /// `CData` table id.
    pub db_type: i32,
    /// Row inside that table.
    pub row: i32,
    /// Column inside that row.
    pub column: i32,
    /// The string to write when the cell is not already meaningful.
    pub value: &'static str,
}

/// Row index -> command-name label for the battle command window (万能ウィンドウ一時DB,
/// `CData type 19` column 0 「項目文」).
///
/// CE152 戦闘コマンド一覧算出 rebuilds this scratch table from `Data type 6` column 0
/// 「コマンド名」 (docs/research/M7_BATTLE_MENU_DB.md). These eight rows mirror, in slot
/// order, what CE152 produces from [`COMMAND_SLOTS`]: slot 1..4 are the real attacks
/// 蓄力/防御/强斩/反击 and slots 5..8 fall back to 普通攻撃. The strings are the same
/// values CE152 reads, so seeding them here exercises the exact label path CE64 draws.
pub const COMMAND_MENU_TEXT: [TextCell; 8] = [
    TextCell {
        db_type: 19,
        row: 0,
        column: 0,
        value: "蓄力",
    },
    TextCell {
        db_type: 19,
        row: 1,
        column: 0,
        value: "防御",
    },
    TextCell {
        db_type: 19,
        row: 2,
        column: 0,
        value: "强斩",
    },
    TextCell {
        db_type: 19,
        row: 3,
        column: 0,
        value: "反击",
    },
    TextCell {
        db_type: 19,
        row: 4,
        column: 0,
        value: "普通攻击",
    },
    TextCell {
        db_type: 19,
        row: 5,
        column: 0,
        value: "普通攻击",
    },
    TextCell {
        db_type: 19,
        row: 6,
        column: 0,
        value: "普通攻击",
    },
    TextCell {
        db_type: 19,
        row: 7,
        column: 0,
        value: "普通攻击",
    },
];

/// Writes [`COMMAND_MENU_TEXT`] into the engine text layer. Returns how many labels landed.
///
/// This is the **data half** of battle-menu localisation. The engine still drops these
/// labels when CE152 rebuilds the scratch table (the 250 write path coerces the string
/// operand of an argc-5 write to a number — exec.rs opcode 250; see
/// docs/research/M7_BATTLE_MENU_DB.md), so the visible window stays blank until that
/// engine fix lands. Keeping the labels here means the fix only needs the write path,
/// not more data.
#[allow(dead_code)]
pub fn seed_menu_text(game: &mut Game) -> usize {
    let mut written = 0;
    for cell in COMMAND_MENU_TEXT {
        game.databases.set_text(
            Container::CData,
            cell.db_type,
            cell.row,
            cell.column,
            cell.value,
        );
        written += 1;
    }
    written
}

/// Writes the battle seed into `game.databases`. Returns how many cells were written.
///
/// Idempotent: calling it again overwrites the same cells with the same values. It is
/// deliberately **not** called from `Game::new` or `start_event`: the m6 control test
/// (`m6_battle_commands.rs`) asserts an empty inventory leaves real attacks unselectable,
/// so seeding stays behind this explicit entry point.
pub fn seed(game: &mut Game) -> usize {
    seed_databases(&mut game.databases)
}

/// Writes the same battle seed into a bare database host. The headless route
/// runner drives the VM without a [`Game`], so it shares this exact data table
/// instead of duplicating the 26 numeric cells and 8 menu labels.
pub fn seed_databases(databases: &mut Databases) -> usize {
    use std::collections::BTreeMap;
    let mut rows: BTreeMap<(i32, i32), Vec<(i32, i32)>> = BTreeMap::new();
    for cell in seed_cells() {
        rows.entry((cell.db_type, cell.row))
            .or_default()
            .push((cell.column, cell.value));
    }
    let mut written = 0;
    for ((db_type, row), columns) in rows {
        written += databases.seed_numbers(Container::CData, db_type, row, &columns);
    }
    // The command-menu labels are the string half of the same table. They do not add to
    // the returned numeric-cell count (kept at 26 so m7_battle_seed's documented
    // "1 party + 1 enemy + 8 slots + 12 stats + 1 item + 3 weapons" invariant stays exact).
    for cell in COMMAND_MENU_TEXT {
        databases.set_text(
            Container::CData,
            cell.db_type,
            cell.row,
            cell.column,
            cell.value,
        );
    }
    written
}
