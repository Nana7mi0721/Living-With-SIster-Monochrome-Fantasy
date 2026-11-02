//! M11 P0 - real two-member party fixture -> live command menu -> CE242 bridge.
//!
//! The task-89 cursor research proved the live menu producer (CE188 -> CE190 -> CE191
//! -> CE192 -> CE157) is not broken: every real confirm advances the コマンド入力位置
//! cursor and writes a distinct row into the command processing list.  What was still
//! missing was a genuine *two-member* party, so the earlier fixture only ever produced
//! one row with a real action code and the rest were empty slots.
//!
//! This test builds the smallest real two-member party in the databases (party count 2,
//! member 2 registered in パーティー情報, a second 主人公ステータス row with the same
//! stat block and its own command columns), drives the real CE188 menu with real OK key
//! input, and pins two successive registrations with two distinct real action codes
//! (26 = 蓄力, 25 = 防御) in rows 1 and 2.  It then feeds the harvested command-index
//! values into the BattleWiring menu queue and runs a real CE253 -> CE239 BOSS, asserting
//! four distinct CE242 command bodies, two drained action-point pools, and a settled
//! battle (v352 in {1,-1}, v343 = 1, warnings = 0).
//!
//! Test-only: no production file is modified.  The fixture never touches the retail save
//! on disk; it copies the save game into memory and patches the in-memory CData cells.

use std::collections::BTreeSet;
use std::path::PathBuf;
use wolf_game::db::Container;
use wolf_game::Game;

/// コマンド処理リスト (logical 15 -> physical type 14).
const CMD_LIST_TYPE: i32 = 14;
/// 基本システム用変数 (logical 20 -> physical type 18); row 73 col 0 is the real cursor.
const INPUT_POS_TYPE: i32 = 18;
/// The cell old probes wrongly read: physical type 20 row 73 is always 0.
const WRONG_POS_TYPE: i32 = 20;
/// パーティー人数 lives in physical type 18 row 4 col 0.
const PARTY_COUNT_TYPE: i32 = 18;
/// パーティー情報 (logical 3 read / 6 write).
const PARTY_TABLE_TYPE: i32 = 6;
/// 主人公ステータス rows.
const STATUS_TYPE: i32 = 0;
const LEADER_ROW: i32 = 12;
const MEMBER2_ROW: i32 = 13;
/// Action codes that CE152 can really select (蓄力/防御/強斬/反撃).
const REAL_ACTION_CODES: [i32; 4] = [26, 25, 30, 29];
/// Four real command bodies on the BOSS command board.
const EXPECTED_FIRST_PCS: [i32; 4] = [338, 645, 792, 996];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MenuRow {
    row: i32,
    /// 発動者番号 / slot in col 0 (0 for member 1, 1 for member 2).
    activator: i32,
    /// Materialised action field in col 3; -1 means an empty slot.
    code: i32,
}

#[derive(Clone, Debug)]
struct RoundObs {
    index: u64,
    first_pc: i32,
    arg0: i32,
    pre: [i32; 4],
    post: [i32; 4],
}

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

/// Real two-member party fixture.  Only database cells are touched; the retail save on
/// disk is never edited.  Returns the number of battle cells seeded by the production
/// seed table.
fn build_fixture(game: &mut Game) -> usize {
    let seeded = game.seed_battle_databases();
    assert!(seeded > 0, "battle DB seed must land");

    // Party count 2 and a second member row registered in パーティー情報.
    game.databases
        .set_num(Container::CData, PARTY_COUNT_TYPE, 4, 0, 2);
    game.databases
        .set_num(Container::CData, PARTY_TABLE_TYPE, 0, 2, MEMBER2_ROW);

    // Give member 2 the same complete stat block as the leader, plus its own command
    // columns 47..54 so CE143 materialises two allies with full HP.
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
    // Read-load fix removes the old spurious 506 coroutine; pin this fixture's event RNG
    // so its intended deterministic sequence is preserved.
    game.vm.rng.set_event_seed(9);
    // Synthetic battle fixture: clear the save's restored common-event self banks so
    // they cannot perturb the deterministic command-round count driven below.
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

/// Drive the real CE253 -> CE239 BOSS with the harvested menu queue.  The round loop is
/// enabled and a 4000 HP boss gauge keeps the fight alive long enough for four command
/// rounds.  Returns one observation per CE242 round; settlement values are captured on
/// the exact frame v343 becomes 1, before any post-battle drain can reset them.
fn drive_bridge(queue: &[i32]) -> (Vec<RoundObs>, (i32, i32), u64, u64, u64) {
    let mut game = open_fixture_game();
    game.databases
        .set_text(Container::CData, 5, 16, 0, "┗ 状態リスト");
    let _ = game.start_engine_events();
    game.set_battle_wiring(true);
    game.set_battle_round_loop(true);
    game.set_battle_menu_command_queue(queue.to_vec());
    game.vm.vars.set(445, 1);
    game.vm.vars.set(446, 4000);
    assert!(
        game.vm.start_event_fresh(253).is_some(),
        "CE253 battle must start"
    );

    let mut rounds: Vec<RoundObs> = Vec::new();
    let mut current: Option<RoundObs> = None;
    let mut last_rounds = 0u64;
    let mut settle = (0, 0);
    let mut settled = false;

    for f in 0..40000usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();

        let started = game.battle_wiring_rounds();
        if started > last_rounds {
            if let Some(mut prev) = current.take() {
                prev.post = pools(&game);
                rounds.push(prev);
            }
            last_rounds = started;
            let p = pools(&game);
            current = Some(RoundObs {
                index: started,
                first_pc: 0,
                arg0: 0,
                pre: p,
                post: p,
            });
        }

        if let Some(cur) = current.as_mut() {
            if cur.first_pc == 0 {
                for inst in game.vm.instances() {
                    if format!("{:?}", inst.event) == "242" {
                        let pc = inst.pc as i32;
                        if pc > 143 {
                            cur.first_pc = pc;
                            cur.arg0 = inst.self_vars.get(600);
                            break;
                        }
                    }
                }
            }
            cur.post = pools(&game);
        }

        if game.vm.vars.get(343) == 1 && game.vm.vars.get(350) == 0 {
            if let Some(mut cur) = current.take() {
                cur.post = pools(&game);
                rounds.push(cur);
            }
            settle = (game.vm.vars.get(352), game.vm.vars.get(343));
            settled = true;
            break;
        }
    }

    assert!(settled, "CE244 must settle the BOSS battle");
    (
        rounds,
        settle,
        game.battle_wiring_rounds(),
        game.battle_wiring_turn_starts(),
        game.stats.warnings,
    )
}

#[test]
fn real_two_member_party_menu_feeds_the_ce242_bridge() {
    // ---- Part A: real two-member party through the live CE188 menu ----
    let mut menu_game = open_fixture_game();
    assert_eq!(
        menu_game
            .databases
            .num(Container::CData, PARTY_COUNT_TYPE, 4, 0),
        Some(2),
        "party count must be two"
    );
    assert_eq!(
        menu_game
            .databases
            .num(Container::CData, PARTY_TABLE_TYPE, 0, 1),
        Some(LEADER_ROW),
        "member 1 must remain the leader row"
    );
    assert_eq!(
        menu_game
            .databases
            .num(Container::CData, PARTY_TABLE_TYPE, 0, 2),
        Some(MEMBER2_ROW),
        "member 2 must be registered"
    );

    assert!(menu_game.start_event(188), "CE188 battle entry must start");
    menu_game.frame();

    let mut best_rows = menu_rows(&menu_game);
    let mut best_n = best_rows.len();
    let mut best_inpos = menu_game
        .databases
        .num(Container::CData, INPUT_POS_TYPE, 73, 0)
        .unwrap_or(-1);
    let mut last_growth = 0usize;
    let mut ce190_starts = 0u32;
    let mut ce191_starts = 0u32;
    let mut ce192_starts = 0u32;
    let mut ce157_starts = 0u32;
    let (mut saw190, mut saw191, mut saw192, mut saw157) = (false, false, false, false);

    for f in 0..2000usize {
        let has190 = is_live_event(&menu_game, 190);
        let has191 = is_live_event(&menu_game, 191);
        let has192 = is_live_event(&menu_game, 192);
        let has157 = is_live_event(&menu_game, 157);
        if has190 && !saw190 {
            ce190_starts += 1;
        }
        if has191 && !saw191 {
            ce191_starts += 1;
        }
        if has192 && !saw192 {
            ce192_starts += 1;
        }
        if has157 && !saw157 {
            ce157_starts += 1;
        }
        saw190 = has190;
        saw191 = has191;
        saw192 = has192;
        saw157 = has157;

        menu_game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        menu_game.frame();

        let rows = menu_rows(&menu_game);
        if rows.len() > best_n {
            best_n = rows.len();
            best_rows = rows;
            best_inpos = menu_game
                .databases
                .num(Container::CData, INPUT_POS_TYPE, 73, 0)
                .unwrap_or(-1);
            last_growth = f;
        }
        if best_n >= 2 && f.saturating_sub(last_growth) > 80 {
            break;
        }
    }

    assert!(
        best_n >= 2,
        "expected at least two registered menu rows, got {best_n}: {best_rows:?}"
    );
    assert_eq!(
        best_inpos,
        1 + best_n as i32,
        "the real cursor must advance once per registration"
    );
    assert!(ce190_starts >= 1, "CE190 must run");
    assert!(ce191_starts >= 1, "CE191 must loop over party members");
    assert!(ce192_starts >= 2, "each member gets a CE192 command window");
    assert!(
        ce157_starts >= 2,
        "each confirm must run CE157 registration"
    );

    let mut activators = BTreeSet::new();
    for (i, row) in best_rows.iter().enumerate() {
        let expected_row = i as i32 + 1;
        assert_eq!(
            row.row, expected_row,
            "registered rows must be contiguous: {best_rows:?}"
        );
        assert_eq!(
            row.activator,
            expected_row - 1,
            "row {expected_row} must carry party slot {}: {row:?}",
            expected_row - 1
        );
        assert!(
            row.code >= 0,
            "registered row {expected_row} must not be a -1 sentinel: {row:?}"
        );
        activators.insert(row.activator);
    }
    assert_eq!(
        activators.len(),
        best_n,
        "each registered row must belong to a distinct slot"
    );

    let real_rows: Vec<MenuRow> = best_rows
        .iter()
        .filter(|row| REAL_ACTION_CODES.contains(&row.code))
        .copied()
        .collect();
    assert!(
        real_rows.len() >= 2,
        "two-member fixture must produce at least two real action rows, got {real_rows:?}"
    );
    assert_eq!(
        real_rows[0],
        MenuRow {
            row: 1,
            activator: 0,
            code: 26
        }
    );
    assert_eq!(
        real_rows[1],
        MenuRow {
            row: 2,
            activator: 1,
            code: 25
        }
    );
    assert_ne!(
        real_rows[0].code, real_rows[1].code,
        "the two real confirmations must use distinct action codes"
    );

    // CE143 must have materialised both allies with the fixture's full HP.
    assert_eq!(
        menu_game
            .databases
            .num(Container::CData, CMD_LIST_TYPE, 0, 0),
        Some(-1),
        "row 0 of the command list is empty; real rows start at row 1"
    );
    assert_eq!(
        menu_game.databases.num(Container::CData, 10, 0, 1),
        Some(100)
    );
    assert_eq!(
        menu_game.databases.num(Container::CData, 10, 0, 2),
        Some(100)
    );
    assert_eq!(
        menu_game.databases.num(Container::CData, 10, 1, 1),
        Some(100)
    );
    assert_eq!(
        menu_game.databases.num(Container::CData, 10, 1, 2),
        Some(100)
    );
    assert_eq!(
        menu_game.stats.warnings, 0,
        "live two-member menu must not warn"
    );

    // The old trap cell stays zero while the real cursor moved.
    assert_eq!(
        menu_game
            .databases
            .num(Container::CData, WRONG_POS_TYPE, 73, 0)
            .unwrap_or(-1),
        0
    );

    // ---- Part B: hand the harvested rows to the CE242 bridge ----
    // The first four registered rows map row -> command index 1..4.  Rows 1 and 2 are the
    // two real confirmations; rows 3 and 4 are the subsequent registrations the fixture
    // also produced and are still genuine menu rows.
    let queue: Vec<i32> = best_rows
        .iter()
        .take(4)
        .map(|row| row.row.clamp(1, 8))
        .collect();
    assert_eq!(queue, vec![1, 2, 3, 4], "harvested command indexes");
    assert_eq!(real_rows[0].row.clamp(1, 8), queue[0]);
    assert_eq!(real_rows[1].row.clamp(1, 8), queue[1]);

    let (rounds, settle, started_rounds, turn_starts, warnings) = drive_bridge(&queue);
    assert_eq!(started_rounds, 4, "bridge must start four command rounds");
    assert_eq!(
        turn_starts, 3,
        "three CE241 turn-start phases between four commands"
    );
    assert_eq!(
        rounds.len(),
        4,
        "one observable CE242 body per command round: {rounds:?}"
    );

    let first_pcs: Vec<i32> = rounds.iter().map(|r| r.first_pc).collect();
    let args: Vec<i32> = rounds.iter().map(|r| r.arg0).collect();
    assert_eq!(
        first_pcs,
        EXPECTED_FIRST_PCS.to_vec(),
        "distinct CE242 bodies"
    );
    assert_eq!(
        args,
        vec![1, 2, 3, 4],
        "menu queue must be consumed front-first"
    );
    assert_eq!(
        first_pcs.iter().copied().collect::<BTreeSet<_>>().len(),
        4,
        "four distinct CE242 bodies must run"
    );

    assert_eq!(
        rounds[0].pre,
        [5, 4, 3, 2],
        "BOSS starts with four full pools"
    );
    let final_post = rounds.last().unwrap().post;
    assert_eq!(final_post, [3, 2, 3, 2], "two action-point pools drained");
    let drained = (0..4)
        .filter(|i| final_post[*i] < rounds[0].pre[*i])
        .count();
    assert!(
        drained >= 2,
        "expected >=2 distinct pools drained, got {drained}: {rounds:?}"
    );
    assert_eq!(settle, (1, 1), "CE244 must record v352=1 and v343=1");
    assert_eq!(warnings, 0, "battle must not warn");

    eprintln!(
        "m11_party_live_bridge: registered_rows={} real_rows={:?} crawl_ce190={} ce191={} ce192={} ce157={}",
        best_n,
        real_rows,
        ce190_starts,
        ce191_starts,
        ce192_starts,
        ce157_starts
    );
    for obs in &rounds {
        eprintln!(
            "  round={} arg0={} first_pc={} pre={:?} post={:?}",
            obs.index, obs.arg0, obs.first_pc, obs.pre, obs.post
        );
    }
    eprintln!(
        "  bridge settle={:?} started_rounds={} turn_starts={} warnings={}",
        settle, started_rounds, turn_starts, warnings
    );
}
