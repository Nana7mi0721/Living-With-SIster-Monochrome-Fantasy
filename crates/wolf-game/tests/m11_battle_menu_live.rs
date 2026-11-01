//! M11 task-98: the real player battle command menu -> CE157 -> CE242, harvested in
//! ordinary map state (no battle-drive) and consumed by the rebuilt command-phase drive with
//! NO manual command-queue injection.
//!
//! ## What this proves
//!
//! The shipped player-facing command menu line CE188 -> CE190 -> CE191 -> CE192 -> CE152 ->
//! CE65 -> CE157 writes each confirmed command into the command-processing list (CData type14)
//! and spawns a CE157 (コマンド登録) coroutine. BattleWiring::harvest_menu_registrations reads
//! those committed rows back out of the command-processing list (CData type14) - NOT the
//! transient CE157 self-var self1600017, which is only assigned at pc80-84 and has already been
//! advanced by the time a step boundary is observed - and derives the CE242 command index from
//! the ROW (command_index = row.clamp(1,8)); the engine therefore lifts the real menu output
//! into its command-index queue without any player-input faking.
//!
//! Previously the harvester ran only after the drive gate in BattleWiring::tick, so it never
//! saw a menu confirmation made in ordinary map state (drive == false).  Task-98 moves the
//! harvest above the drive gate, so the real menu confirmations are captured while the player
//! is simply choosing commands, and the very same BattleWiring instance later drains them when
//! the CE253 battle dispatch auto-arms the rebuilt drive.
//!
//! The test drives one Game instance through both phases:
//!   A) real two-member battle menu (start CE188, real OK keys), NO CE253 / NO set_battle_wiring
//!      / NO set_battle_menu_command_queue -> the queue is populated purely by the harvester;
//!   B) real CE253 -> CE239 BOSS battle with the default auto-arm (again no wiring flag, no
//!      queue injection) -> CE242 command rounds run and the battle settles.
//!
//! Test-only: no production file is touched except the shared harvester fix; the retail save on
//! disk is loaded into memory and only in-memory CData cells are patched.

use std::collections::BTreeSet;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MenuRow {
    row: i32,
    activator: i32,
    code: i32,
}

/// Real two-member party fixture (same cells as the M11 P0 bridge).  Only database cells are
/// patched; the retail save on disk is never edited.
fn build_fixture(game: &mut Game) -> usize {
    let seeded = game.seed_battle_databases();
    assert!(seeded > 0, "battle DB seed must land");

    game.databases
        .set_num(Container::CData, PARTY_COUNT_TYPE, 4, 0, 2);
    game.databases
        .set_num(Container::CData, PARTY_TABLE_TYPE, 0, 2, MEMBER2_ROW);

    let stats = [
        (20, 100),
        (21, 100),
        (22, 50),
        (23, 50),
        (24, 20),
        (25, 15),
        (26, 10),
        (27, 30),
        (28, 30),
        (9, 20),
        (10, 10),
        (4, 10),
    ];
    for (c, v) in stats {
        game.databases
            .set_num(Container::CData, STATUS_TYPE, MEMBER2_ROW, c, v);
    }
    let commands = [4, 3, 6, 7, 0, 0, 0, 0];
    for (i, v) in commands.iter().enumerate() {
        game.databases.set_num(
            Container::CData,
            STATUS_TYPE,
            MEMBER2_ROW,
            47 + i as i32,
            *v,
        );
    }
    seeded
}

fn open_fixture_game() -> Game {
    let mut game = Game::new(data_root()).unwrap();
    game.load_save(&save01()).unwrap();
    // Synthetic battle-driver fixture: clear the retail save's common-event self banks so
    // the live-menu → CE242 round flow is exercised from a clean, save-independent store.
    game.vm.clear_common_self();
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    build_fixture(&mut game);
    game
}

/// All materialised command rows (col 0 >= 0), in row order.
fn menu_rows(game: &Game) -> Vec<MenuRow> {
    (0..16)
        .filter_map(|r| {
            let activator = game.databases.num(Container::CData, CMD_LIST_TYPE, r, 0)?;
            if activator < 0 {
                return None;
            }
            Some(MenuRow {
                row: r,
                activator,
                code: game
                    .databases
                    .num(Container::CData, CMD_LIST_TYPE, r, 3)
                    .unwrap_or(-1),
            })
        })
        .collect()
}

fn pools(game: &Game) -> [i32; 4] {
    [
        game.vm.vars.get(345),
        game.vm.vars.get(346),
        game.vm.vars.get(347),
        game.vm.vars.get(348),
    ]
}

fn is_live_event(game: &Game, id: u32) -> bool {
    game.vm
        .instances()
        .iter()
        .any(|inst| format!("{:?}", inst.event) == id.to_string())
}

#[test]
fn natural_menu_confirmations_harvest_into_ce242_rounds() {
    // ---- Phase A: real two-member battle menu in ordinary map state (drive off) ----
    let mut game = open_fixture_game();
    assert_eq!(
        game.databases.num(Container::CData, PARTY_COUNT_TYPE, 4, 0),
        Some(2),
        "party count must be two"
    );
    // No battle dispatch (CE253) yet and no explicit wiring flag: the drive is OFF while the
    // menu runs, exactly like a real player choosing commands.
    assert!(game.start_event(188), "CE188 battle entry must start");
    game.frame();

    let mut best_rows = menu_rows(&game);
    let (mut saw157, mut ce157_starts) = (false, 0u32);
    for f in 0..1200usize {
        let has157 = is_live_event(&game, 157);
        if has157 && !saw157 {
            ce157_starts += 1;
        }
        saw157 = has157;
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        let rows = menu_rows(&game);
        if rows.len() > best_rows.len() {
            best_rows = rows;
        }
        // A real two-member party confirms once per member; stop as soon as both rows exist so
        // the queue holds exactly the two genuine member commands (no spam registrations).
        if best_rows.len() >= 2 {
            break;
        }
    }
    assert!(
        best_rows.len() >= 2,
        "real menu must register at least two command rows, got {:?}",
        best_rows
    );
    assert!(ce157_starts >= 2, "each confirm must spawn a CE157 registration");
    // The command-list rows the real menu produced, in row order. By the documented contract
    // (row index == CE242 command index) these are the harvested command indexes.
    let harvested: Vec<i32> = best_rows.iter().map(|r| r.row.clamp(1, 8)).collect();
    let mut uniq = harvested.clone();
    uniq.sort_unstable();
    uniq.dedup();
    assert_eq!(
        uniq.len(),
        harvested.len(),
        "registered rows must be distinct: {:?}",
        harvested
    );

    // ---- Phase B: real CE253 -> CE239 battle, default auto-arm, NO queue injection ----
    game.databases
        .set_text(Container::CData, 5, 16, 0, "┗ 状態リスト");
    let _ = game.start_engine_events();
    // Genuine BOSS gauge cells (CE239 pc202: 2000445 == 1 feeds 2000446) so the fight survives
    // long enough for several command rounds. Data cells, not settlement values.
    game.vm.vars.set(445, 1);
    game.vm.vars.set(446, 4000);
    assert!(game.battle_wiring_auto_armed(), "auto-arm default ON");
    assert!(game.start_event(253), "CE253 battle dispatch must start");

    let mut recs: Vec<(i32, i32)> = Vec::new(); // per round: (body pc, arg0 command index)
    let mut pool_pre: Vec<[i32; 4]> = Vec::new();
    let mut last_rounds = 0u64;
    let mut settle = (0, 0);
    let mut settled = false;
    for f in 0..40000usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        let started = game.battle_wiring_rounds();
        if started > last_rounds {
            last_rounds = started;
            pool_pre.push(pools(&game));
            recs.push((0, 0));
        }
        if let Some(slot) = recs.last_mut() {
            for inst in game.vm.instances() {
                if format!("{:?}", inst.event) == "242" {
                    let pc = inst.pc as i32;
                    if slot.0 == 0 && pc > 143 {
                        slot.0 = pc;
                    }
                    let a = inst.self_vars.get(600);
                    if a != 0 {
                        slot.1 = a;
                    }
                }
            }
        }
        if game.vm.vars.get(343) == 1 && game.vm.vars.get(350) == 0 {
            settle = (game.vm.vars.get(352), game.vm.vars.get(343));
            settled = true;
            break;
        }
    }

    let first_pcs: Vec<i32> = recs.iter().map(|r| r.0).collect();
    let arg0s: Vec<i32> = recs.iter().map(|r| r.1).collect();
    let final_pools = pools(&game);
    let rounds = game.battle_wiring_rounds();
    let turn_starts = game.battle_wiring_turn_starts();
    let warnings = game.stats.warnings;

    eprintln!(
        "m11_battle_menu_live: menu_rows={:?} harvested={:?} ce157_starts={}",
        best_rows, harvested, ce157_starts
    );
    eprintln!(
        "  battle: rounds={} first_pcs={:?} arg0s={:?} pool_pre={:?} final_pools={:?} settle={:?} turn_starts={} warnings={}",
        rounds, first_pcs, arg0s, pool_pre, final_pools, settle, turn_starts, warnings
    );

    assert!(settled, "the auto-armed battle must settle");
    assert!(rounds >= 2, "must run >= 2 real CE242 command rounds, got {rounds}");
    assert!(
        turn_starts >= 1,
        "the REAL CE241 turn-start must fire between rounds, got {turn_starts}"
    );
    // KEY ASSERTION: the first CE242 rounds consume the REAL harvested menu indexes front-first.
    // No set_battle_menu_command_queue was called, so any match here can only come from the
    // harvester. The default rotating schedule is [1, 3]; the harvested sequence contains 2, so
    // a match on that position proves the harvest (not the schedule) fed the drive.
    let k = harvested.len().min(arg0s.len());
    assert!(k >= 2, "need at least two harvested rounds, harvested={harvested:?} arg0s={arg0s:?}");
    assert_eq!(
        &arg0s[..k],
        &harvested[..k],
        "CE242 must consume the real harvested menu indexes front-first (harvested={harvested:?}, arg0s={arg0s:?})"
    );
    // Distinct CE242 bodies for the first two rounds (different members -> different bodies).
    let body2 = first_pcs.iter().take(2).copied().collect::<BTreeSet<_>>();
    assert_eq!(body2.len(), 2, "first two rounds must reach distinct CE242 bodies: {first_pcs:?}");
    // Distinct party pools must drain.
    assert_eq!(pool_pre[0], [5, 4, 3, 2], "CE239 must seed four pools 5/4/3/2, got {:?}", pool_pre[0]);
    let drained = (0..4).filter(|i| final_pools[*i] < pool_pre[0][*i]).count();
    assert!(drained >= 2, ">= 2 member pools must drain, pre={:?} post={:?}", pool_pre[0], final_pools);
    assert!(settle.0 == 1 || settle.0 == -1, "real CE244 result in {{1,-1}}, got {:?}", settle);
    assert_eq!(settle.1, 1, "2000343 decided");
    assert_eq!(warnings, 0, "battle must not warn");
}