//! M11 P0 — CE157 command-registration cursor (`X[戦]コマンド登録`).
//!
//! This test pins the real behaviour of the command-registration cursor that the
//! task-89 briefing *assumed* was broken. It is not broken: driving the real
//! battle entry `CE188` with real key input on the bundled retail save makes
//! CE191 loop over the ally slots, CE192 open a command window per member, and
//! CE157 write a *distinct* row into the `コマンド処理リスト` (CData logical 15 ->
//! physical **type 14**) for every successive confirm, while the コマンド入力位置
//! counter (logical 20 -> physical **type 18 row 73 col 0**) increments once per
//! registration.
//!
//! Root cause of the "only row 1" folklore (see docs/research/M11_CE157_CURSOR.md):
//! earlier probes read the input-position from physical **type 20** row 73, which
//! is always 0; the real cell is physical **type 18** row 73 col 0. They also broke
//! out of the frame loop the instant the first row appeared, so they never saw the
//! next confirm. There is no VM bug here and no speculative fix is needed.
//!
//! The shipped `CE188` legacy engine is a *stub* in this rewrite (one real party
//! member; the enemy/allies never actually trade damage), so the outer loop keeps
//! re-running — we therefore stop once the registration set stops growing and
//! assert the structural invariant at that peak.

use std::path::PathBuf;
use wolf_game::db::Container;
use wolf_game::Game;

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        return p;
    }
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop();
    b.pop();
    b.join(p)
}

fn save01() -> PathBuf {
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop();
    b.pop();
    b.join("work/retail_backup/Save/SaveData01.sav")
}

const CMD_LIST_TYPE: i32 = 14; // コマンド処理リスト (logical 15)
const INPUT_POS_TYPE: i32 = 18; // 基本システム用変数 (logical 20) — the real cell
const WRONG_POS_TYPE: i32 = 20; // what old probes read: always 0

/// Registered rows: (row, col0 = activator/slot, col3 = action code).
fn registered_rows(game: &Game) -> Vec<(i32, i32, i32)> {
    (0..8)
        .map(|r| {
            (
                r,
                game.databases.num(Container::CData, CMD_LIST_TYPE, r, 0).unwrap_or(-1),
                game.databases.num(Container::CData, CMD_LIST_TYPE, r, 3).unwrap_or(-1),
            )
        })
        .collect()
}

#[test]
fn live_menu_registers_multiple_rows_and_advances_the_cursor() {
    let mut game = Game::new(data_root()).unwrap();
    game.load_save(&save01()).unwrap();
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(game.seed_battle_databases() > 0, "battle DB seed must land");
    assert!(game.start_event(188), "CE188 battle must start");
    game.frame(); // let the engine build CE188's program

    let mut best_rows = registered_rows(&game);
    let mut best_n = best_rows.iter().filter(|t| t.1 >= 0).count();
    let mut best_inpos = game.databases.num(Container::CData, INPUT_POS_TYPE, 73, 0).unwrap_or(-1);
    let mut last_inc = 0usize;
    let mut ce157_starts = 0u32;
    let mut saw157 = false;
    let mut ce192_starts = 0u32;
    let mut saw192 = false;

    for f in 0..2000 {
        let mut h157 = false;
        let mut h192 = false;
        for inst in game.vm.instances() {
            let e = format!("{:?}", inst.event);
            if e == "157" {
                h157 = true;
            }
            if e == "192" {
                h192 = true;
            }
        }
        if h157 && !saw157 {
            ce157_starts += 1;
        }
        saw157 = h157;
        if h192 && !saw192 {
            ce192_starts += 1;
        }
        saw192 = h192;

        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();

        let rows = registered_rows(&game);
        let n = rows.iter().filter(|t| t.1 >= 0).count();
        if n > best_n {
            best_n = n;
            best_rows = rows;
            best_inpos = game.databases.num(Container::CData, INPUT_POS_TYPE, 73, 0).unwrap_or(-1);
            last_inc = f;
        }
        // Stop once the registration set has been stable for a while after it grew.
        if best_n >= 2 && f.saturating_sub(last_inc) > 40 {
            break;
        }
    }

    assert!(
        best_n >= 3,
        "expected >=3 distinct registration rows from successive confirms, got {best_n}: {best_rows:?}"
    );
    assert!(
        ce192_starts >= best_n as u32,
        "each registered row should come from a CE192 member window: starts={ce192_starts} rows={best_n}"
    );
    assert!(ce157_starts >= best_n as u32, "CE157 must be invoked per confirm");

    // Every registered row carries the activator/slot in col0 == row-1, and a
    // materialised action field in col3 (not the -1 sentinel of an empty slot).
    let mut distinct = std::collections::BTreeSet::new();
    for r in 1..=best_n as i32 {
        let (_, c0, c3) = best_rows[r as usize];
        assert_eq!(c0, r - 1, "row {r} activator/slot should be {} got {c0}", r - 1);
        assert!(c3 >= 0, "row {r} should carry a non-sentinel action field, got {c3}");
        distinct.insert(c0);
    }
    assert_eq!(distinct.len(), best_n, "registered rows must be distinct: {best_rows:?}");

    // The cursor advanced exactly once per registration: it starts at 1 after the
    // clear-all and ends at 1 + registrations.
    assert_eq!(
        best_inpos,
        1 + best_n as i32,
        "input position must equal 1 + registrations at the registration peak"
    );

    // The old probe read the wrong physical cell; pin that trap so nobody
    // regresses to type 20 again.
    let wrong = game.databases.num(Container::CData, WRONG_POS_TYPE, 73, 0).unwrap_or(-1);
    assert_eq!(wrong, 0, "physical type 20 row 73 is NOT the command-input position (always 0)");
}

#[test]
fn input_position_is_physical_type_18_row_73() {
    // Minimal, direct witness of the cell identity: after a couple of live
    // registrations the real input-position cell is non-zero while the cell the
    // old probes used stays zero.
    let mut game = Game::new(data_root()).unwrap();
    game.load_save(&save01()).unwrap();
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    assert!(game.seed_battle_databases() > 0);
    assert!(game.start_event(188));

    let mut advanced = false;
    for f in 0..1200 {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        let real = game.databases.num(Container::CData, INPUT_POS_TYPE, 73, 0).unwrap_or(0);
        if real >= 3 {
            advanced = true;
            assert_eq!(
                game.databases.num(Container::CData, WRONG_POS_TYPE, 73, 0).unwrap_or(0),
                0,
                "type 20 row 73 (wrong cell) must stay 0 while type 18 advances to {real}"
            );
            break;
        }
    }
    assert!(advanced, "the real command-input position (type 18 row 73) should advance past 2");
}
