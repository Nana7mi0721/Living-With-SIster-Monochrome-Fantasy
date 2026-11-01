//! M12: the real battle command menu's cancel/back path, with no command injection.
//!
//! The M11 menu test proved that two real member confirmations register two command
//! rows (`CData` type 14) and that the harvester lifts them into CE242. This file
//! covers the other half of the player input: what the menu does when the player
//! cancels, and that a cancel never becomes a command.
//!
//! Measured semantics of the shipped menu (CE188 -> CE191 行動内容のセット -> CE192
//! 味方コマンド選択 -> CE65 万能ウィンドウ選択実行):
//!
//! * CE65 returns -100 for a cancel at either window. CE191 loops and reopens the same
//!   window (`1600015 != -100` is its continue test); nothing is registered.
//! * CE65 returns -300 for the sub key / left / right. CE191 treats that as "leave this
//!   window" and continues to CE192's per-member command selection.
//! * CE192 treats -100 (and -300) as "back out" (`1600099 = -1` then BreakEvent), so a
//!   cancel at the command list returns without writing a command row. CE157 is only
//!   started by `CE192 pc208` after `pc137` returned a real selection - never by the
//!   cancel branch.
//!
//! The test drives one real `Game` with a two-member fixture built only from database
//! cells (the same fixture as `m11_battle_menu_live.rs`) and never calls
//! `set_battle_wiring`, `set_battle_round_loop` or `set_battle_menu_command_queue`.
//! Retail saves are copyrighted; the test skips when they are absent.

use std::path::PathBuf;

use wolf_game::db::Container;
use wolf_game::Game;

/// コマンド処理リスト (logical 15 -> physical type 14).
const CMD_LIST_TYPE: i32 = 14;
/// パーティー人数 lives in physical type 18 row 4 col 0.
const PARTY_COUNT_TYPE: i32 = 18;
/// パーティー情報 (logical 3 read / 6 write).
const PARTY_TABLE_TYPE: i32 = 6;
/// 主人公ステータス rows.
const STATUS_TYPE: i32 = 0;
const MEMBER2_ROW: i32 = 13;

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        repo_root().join(path)
    }
}

fn save01() -> PathBuf {
    if let Ok(dir) = std::env::var("WOLF_RETAIL_SAVE_DIR") {
        let dir = PathBuf::from(dir);
        let dir = if dir.is_absolute() {
            dir
        } else {
            repo_root().join(dir)
        };
        return dir.join("SaveData01.sav");
    }
    repo_root().join("work/retail_backup/Save/SaveData01.sav")
}

fn open_fixture_game() -> Option<Game> {
    if !save01().is_file() {
        eprintln!("skipping: no retail SaveData01.sav");
        return None;
    }
    let mut game = match Game::new(data_root()) {
        Ok(game) => game,
        Err(err) => {
            eprintln!("skipping: {err}");
            return None;
        }
    };
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;

    // Real two-member party fixture (same cells as m11_battle_menu_live.rs). Only
    // database cells are patched; the retail file on disk is never edited.
    assert!(game.seed_battle_databases() > 0, "battle DB seed must land");
    game.databases
        .set_num(Container::CData, PARTY_COUNT_TYPE, 4, 0, 2);
    game.databases
        .set_num(Container::CData, PARTY_TABLE_TYPE, 0, 2, MEMBER2_ROW);
    for (column, value) in [(20, 100), (21, 100), (22, 50), (23, 50), (24, 20), (25, 15)] {
        game.databases
            .set_num(Container::CData, STATUS_TYPE, MEMBER2_ROW, column, value);
    }
    for (index, value) in [4, 3, 6, 7, 0, 0, 0, 0].iter().enumerate() {
        game.databases.set_num(
            Container::CData,
            STATUS_TYPE,
            MEMBER2_ROW,
            47 + index as i32,
            *value,
        );
    }
    Some(game)
}

fn event_pc(game: &Game, id: u32) -> Option<usize> {
    game.vm
        .instances()
        .iter()
        .find(|instance| format!("{:?}", instance.event) == id.to_string())
        .map(|instance| instance.pc)
}

fn live_event(game: &Game, id: u32) -> bool {
    event_pc(game, id).is_some()
}

fn ce157_live(game: &Game) -> bool {
    live_event(game, 157)
}

/// A live event's own self variable, or `None` when the event is not running.
fn event_self(game: &Game, id: u32, slot: usize) -> Option<i32> {
    game.vm
        .instances()
        .iter()
        .find(|instance| format!("{:?}", instance.event) == id.to_string())
        .map(|instance| instance.self_vars.get(slot))
}

/// The materialised command rows: `(row, activator, ability code)` for col 0 >= 0.
fn command_rows(game: &Game) -> Vec<(i32, i32, i32)> {
    (0..16)
        .filter_map(|row| {
            let activator = game.databases.num(Container::CData, CMD_LIST_TYPE, row, 0)?;
            (activator >= 0).then(|| {
                (
                    row,
                    activator,
                    game.databases
                        .num(Container::CData, CMD_LIST_TYPE, row, 3)
                        .unwrap_or(-1),
                )
            })
        })
        .collect()
}

/// One frame with a key held.
fn step(game: &mut Game, key: u32) {
    game.key_bits = key;
    game.frame();
    game.key_bits = 0;
}

/// True on the first frame of a fresh CE65 selection loop (pc78 is the loop head).
fn fresh_selection(previous: Option<usize>, current: Option<usize>) -> bool {
    current == Some(78) && previous != Some(78) && previous != Some(79)
}

#[test]
fn cancel_at_the_action_window_reopens_without_registering() {
    let Some(mut game) = open_fixture_game() else {
        return;
    };
    assert!(game.start_event(188), "CE188 battle entry must start");

    // Step through the command path until CE191's outer window is open. CE65 is the
    // 万能ウィンドウ selector; CE192 has not started yet.
    let mut saw_ce157 = false;
    let mut ce157_starts = 0usize;
    let mut reached = false;
    for frame in 0..600usize {
        let live = ce157_live(&game);
        if live && !saw_ce157 {
            ce157_starts += 1;
        }
        saw_ce157 = live;
        let ce65 = event_pc(&game, 65);
        if ce65 == Some(78) && !live_event(&game, 192) {
            reached = true;
            break;
        }
        step(&mut game, if frame % 2 == 0 { wolf_game::key::OK } else { 0 });
    }
    assert!(reached, "the real action window (CE191 -> CE65) never opened");
    assert_eq!(ce157_starts, 0, "no command may be registered before the window opens");
    assert!(command_rows(&game).is_empty(), "command list must still be empty");

    // Cancel once at the selector. The real engine closes the window and CE191's loop
    // reopens it; the important part is that no command row and no CE157 appear.
    step(&mut game, wolf_game::key::CANCEL);
    let mut reopened = false;
    let mut saw_cancel_return = false;
    let mut saw_ce157 = false;
    let mut ce157_starts = 0usize;
    for _ in 0..60usize {
        step(&mut game, 0);
        if live_event(&game, 65) {
            reopened = true;
        }
        // CE191 receives CE65's result in self1600015; -100 is the cancel code.
        if event_self(&game, 191, 615) == Some(-100) {
            saw_cancel_return = true;
        }
        let live = ce157_live(&game);
        if live && !saw_ce157 {
            ce157_starts += 1;
        }
        saw_ce157 = live;
    }

    assert!(
        saw_cancel_return,
        "CE65 must return -100 to CE191's self1600015 on the cancel path"
    );
    assert!(reopened, "the action window must be reopened after a cancel");
    assert!(!live_event(&game, 192), "a cancel must not advance to CE192");
    assert_eq!(ce157_starts, 0, "a cancel must never start a command registration");
    assert!(
        command_rows(&game).is_empty(),
        "a cancel must not write a command row: {:?}",
        command_rows(&game)
    );
    assert_eq!(game.vm.vars.get(99), 0, "the cancel path leaves the battle flag idle");
    assert_eq!(game.stats.warnings, 0, "the cancel path must not warn");
}

#[test]
fn cancel_in_the_member_command_window_backs_out_and_a_later_confirm_still_registers() {
    let Some(mut game) = open_fixture_game() else {
        return;
    };
    assert!(game.start_event(188), "CE188 battle entry must start");

    // Reach CE192 (味方コマンド選択) through the real CE191 window.
    let mut saw_ce157 = false;
    let mut ce157_starts = 0usize;
    let mut reached = false;
    for frame in 0..1200usize {
        let live = ce157_live(&game);
        if live && !saw_ce157 {
            ce157_starts += 1;
        }
        saw_ce157 = live;
        if live_event(&game, 192) {
            reached = true;
            break;
        }
        step(&mut game, if frame % 2 == 0 { wolf_game::key::OK } else { 0 });
    }
    assert!(reached, "the real member command selection (CE192) never started");
    assert_eq!(ce157_starts, 0, "no row may exist before the first confirmation");
    assert!(
        command_rows(&game).is_empty(),
        "CE192 opens with an empty command list"
    );

    // The window is parked and waits: without input it must stay open, so the later
    // cancel is the only thing that can end it.
    for _ in 0..30usize {
        step(&mut game, 0);
    }
    assert!(
        live_event(&game, 192),
        "CE192 must keep waiting at the command window with no input"
    );
    assert!(command_rows(&game).is_empty(), "waiting must not register a row");

    // Cancel at the first fresh CE65 selection loop that belongs to CE192.
    let mut last_ce65 = None;
    let mut cancel_sent = false;
    let mut ended = false;
    for _ in 0..240usize {
        let ce65 = event_pc(&game, 65);
        if !cancel_sent && fresh_selection(last_ce65, ce65) && live_event(&game, 192) {
            step(&mut game, wolf_game::key::CANCEL);
            cancel_sent = true;
        } else {
            step(&mut game, 0);
        }
        last_ce65 = event_pc(&game, 65);
        if cancel_sent && !live_event(&game, 192) {
            ended = true;
            break;
        }
    }
    assert!(cancel_sent, "the CE192 command window never reached a pollable state");
    assert!(ended, "a cancel must back out of the member command window");
    assert!(
        command_rows(&game).is_empty(),
        "backing out must not write a command row: {:?}",
        command_rows(&game)
    );
    assert!(
        !ce157_live(&game),
        "backing out must not leave a command registration alive"
    );

    // Recover: the menu is still usable, and a later two-member confirmation registers
    // exactly two distinct rows and starts CE157 twice - no injection anywhere.
    let mut saw_ce157 = false;
    let mut ce157_starts = 0usize;
    let mut registered = false;
    for frame in 0..4000usize {
        let live = ce157_live(&game);
        if live && !saw_ce157 {
            ce157_starts += 1;
        }
        saw_ce157 = live;
        step(&mut game, if frame % 2 == 0 { wolf_game::key::OK } else { 0 });
        if command_rows(&game).len() >= 2 {
            registered = true;
            break;
        }
    }
    let rows = command_rows(&game);
    assert!(
        registered,
        "after the cancel the menu must still register two rows, got {rows:?}"
    );
    assert_eq!(
        rows,
        vec![(1, 0, 26), (2, 1, 25)],
        "the two member confirmations keep their retail rows and ability codes"
    );
    assert_eq!(
        ce157_starts, 2,
        "each real confirmation starts exactly one CE157 registration"
    );
    assert_eq!(game.stats.warnings, 0, "the whole cancel/confirm flow must not warn");
}