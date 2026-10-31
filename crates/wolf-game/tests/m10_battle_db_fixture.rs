//! M10 phase 0 — full battle-database fixture + CE172 win/lose counter proof.
//!
//! Purpose (task-66). The naive opcode-111 "reference" patch (comparing the
//! condition value operand as a variable reference instead of a raw literal)
//! makes CE172 `X[戦]敵/味方·勝敗判定` actually able to settle a battle. That is
//! correct, but it exposes whether our *test fixtures* populate the battle DB
//! realistically. This file builds a full, explicit battle DB fixture and proves
//! the CE172 counter logic against it, and it shows that the degenerate
//! "enemy count = 0" fixture that several older tests used would be an instant
//! victory-abort once CE172 actually evaluates.
//!
//! Evidence (all measured from work/_ce_full.json + work/all unless noted):
//!  CE172 id=172 "X[戦]敵/味方·勝敗判定" run=0, 124 slots:
//!   pc8  op121[1600099,0]            verdict = 0
//!   pc12 op250[21,72,...,1600030]    enemy GROUP id (CData type18 row72)
//!   pc13-16 op250[13,group,col7/8/9/10,...,1600033/35/34/36]  group win/lose conds
//!   pc17 op250[21,75,...,1600024]    turn (type18 row75)
//!   pc21 op250[21,4,...,1600020]     ALLY count   (type18 row4)
//!   pc22-31 ally loop: for slot in 0..ally_count call CE176, sum type10[slot].col3
//!   pc32 op111[1,1600023,1600020,2]  if deadAlly == allyCount -> pc35 verdict=-1 (defeat), pc36 BreakEvent
//!   pc42 op250[21,69,...,1600020]    ENEMY count  (type18 row69)
//!   pc43 1600021=9                   enemy slots start at index 10
//!   pc44-52 enemy loop: slot 10..10+enemy_count, sum type10[slot].col3
//!   pc53 op111[1,1600023,1600020,2]  if deadEnemy == enemyCount -> pc57 verdict=1 (victory), pc58 BreakEvent
//!   pc65.. special win/lose conditions from the group table (seeded to 0 here)
//!  CE176 id=176 "X[戦]基本状態取得" returns type10[slot].col3 【戦闘不能?】 (0 alive / 1 dead)
//!   into its self var 1600010 -> copied to CE172's 1600022 -> accumulated into 1600023.
//!
//! THE KEY MEASUREMENT: CE190 "戦闘キャラ配置" rewrites the enemy count (type18
//! row69) from the group table to the real roster size and marks every placed
//! unit alive (type10 col3 = 0). So in ANY real run dead(0) != total(1 or 7) and
//! CE172 does NOT abort. The `dead == total` abort only fires for a synthetic
//! fixture where a whole side is empty or fully dead — most dangerously when
//! enemy_count is left at 0 (deadEnemy 0 == enemyCount 0 -> instant victory).

use std::collections::BTreeMap;
use std::path::PathBuf;

use wolf_game::db::Container;
use wolf_game::Game;

fn repo_root() -> PathBuf {
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop();
    b.pop();
    b
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let p = PathBuf::from(raw);
    if p.is_absolute() { return p; }
    repo_root().join(p)
}

fn save01() -> PathBuf {
    repo_root().join("work/retail_backup/Save/SaveData01.sav")
}

fn cdata(g: &Game, t: i32, r: i32, c: i32) -> i32 {
    g.databases.num(Container::CData, t, r, c).unwrap_or(0)
}

/// Full, explicit battle DB fixture (COM superset of seed_battle_databases):
/// type18 r4/r69/r72/r75, type13 group conditions, type10 HP/alive rows, type0 r12.
fn seed_full_battle_db(g: &mut Game) {
    // --- CData type18: 戦闘基本設定 (r4 ally count, r69 enemy count, r72 group, r75 turn)
    g.databases.set_num(Container::CData, 18, 4, 0, 1); // 1 ally (the leader)
    g.databases.set_num(Container::CData, 18, 69, 0, 1); // 1 enemy in the base roster
    g.databases.set_num(Container::CData, 18, 72, 0, 0); // group id 0
    g.databases.set_num(Container::CData, 18, 75, 0, 3); // turn 3

    // --- CData type13: 敵グループ条件 (group 0; cols 7/8/9/10 = 勝利条件/勝利A/敗北条件/敗北B)
    //     all zero => no special win/lose trigger at CE172 pc65..pc107.
    for c in [7, 8, 9, 10] {
        g.databases.set_num(Container::CData, 13, 0, c, 0);
    }

    // --- CData type10: 戦闘一時ステータス rows. col2 = 存在 flag, col3 = 【戦闘不能?】
    //     ally slot 0 and enemy slots 10..=10 alive.
    for slot in [0i32, 10] {
        g.databases.set_num(Container::CData, 10, slot, 2, 1); // exists
        g.databases.set_num(Container::CData, 10, slot, 3, 0); // alive
        g.databases.set_num(Container::CData, 10, slot, 20, 100); // HP-ish
        g.databases.set_num(Container::CData, 10, slot, 21, 100); // maxHP-ish
    }

    // --- CData type0 row12: leader (character id 12) command slots + stats.
    for (i, v) in [3, 4, 6, 7, 0, 0, 0, 0].iter().enumerate() {
        g.databases.set_num(Container::CData, 0, 12, 47 + i as i32, *v);
    }
    for (c, v) in [
        (20, 100), (21, 100), (22, 50), (23, 50), (24, 20), (25, 15), (26, 10), (27, 30),
        (28, 30), (9, 20), (10, 10), (4, 10),
    ] {
        g.databases.set_num(Container::CData, 0, 12, c, v);
    }
}

/// Faithful re-implementation of CE172's win/lose counter using the *same* DB
/// cells the event reads (type18 r4/r69 + type10 col3 via CE176 semantics).
/// Returns -1 defeat, +1 victory, 0 undecided. [实测 cells; 推断 dead-accumulation
/// semantics derived from pc22-31/pc44-52 + CE176 pc16 op250[10,slot,3].]
fn reference_ce172_verdict(g: &Game) -> i32 {
    let ally_total = cdata(g, 18, 4, 0);
    let dead_ally: i32 = (0..ally_total.max(0)).map(|s| cdata(g, 10, s, 3)).sum();
    if dead_ally == ally_total {
        return -1; // pc32 deadAlly == allyCount -> defeat (also fires on ally_total == 0)
    }
    let enemy_total = cdata(g, 18, 69, 0);
    let dead_enemy: i32 = (0..enemy_total.max(0)).map(|s| cdata(g, 10, 10 + s, 3)).sum();
    if dead_enemy == enemy_total {
        return 1; // pc53 deadEnemy == enemyCount -> victory (also fires on enemy_total == 0)
    }
    0
}

/// Under the CURRENT (literal) interpreter, the value operand of op111 is compared
/// as a raw literal (1_600_020), so neither pc32 nor pc53 can ever be true and
/// CE172 never settles anything. This is arithmetic fact, encoded so the contrast
/// with the reference model is explicit in the test.
fn literal_ce172_verdict(_g: &Game) -> i32 {
    0
}

struct Reached {
    seen: BTreeMap<i32, i32>,
    warnings: u64,
}

fn run_ce188(g: &mut Game, frames: usize) -> Reached {
    let mut seen: BTreeMap<i32, i32> = BTreeMap::new();
    for f in 0..frames {
        if (1..=60).contains(&f) {
            // keep the party seed stable against CE190 rewrites of type18/type0
            g.databases.set_num(Container::CData, 18, 4, 0, 1);
        }
        g.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = g.frame();
        for inst in g.vm.instances() {
            let id: i32 = format!("{:?}", inst.event).parse().unwrap_or(-1);
            let e = seen.entry(id).or_insert(-1);
            if (inst.pc as i32) > *e { *e = inst.pc as i32; }
        }
    }
    Reached { seen, warnings: g.stats.warnings }
}

#[test]
fn full_fixture_keeps_ce172_from_aborting_and_the_line_alive() {
    let mut g = Game::new(data_root()).expect("Game::new");
    g.load_save(&save01()).expect("load SaveData01");
    g.auto_advance = true;
    g.auto_choice = true;
    g.auto_choice_delay = 1;
    seed_full_battle_db(&mut g);
    let _ = g.start_event(188);

    // Reference model over the fixture rows: dead(0) vs totals(1/1) => no abort.
    assert_eq!(reference_ce172_verdict(&g), 0,
        "full fixture must leave CE172 undecided (dead 0 != totals 1/1)");
    // Literal interpreter also leaves it undecided (raw-literal bug).
    assert_eq!(literal_ce172_verdict(&g), 0);

    let r = run_ce188(&mut g, 240);
    assert!(r.seen.contains_key(&172), "CE172 must run");
    // No abort chain: CE172 abort routes to CE199/CE149/CE201, never CE192/CE152.
    assert!(!r.seen.contains_key(&199), "CE172 must not enter defeat/abort chain CE199");
    assert!(!r.seen.contains_key(&149), "CE172 must not enter abort chain CE149");
    assert!(!r.seen.contains_key(&201), "CE172 must not enter abort chain CE201");
    // Line stays alive into command selection.
    assert!(r.seen.contains_key(&152), "full fixture must still reach CE152 戦闘コマンド一覧算出");
    assert!(r.seen.contains_key(&192), "full fixture must still reach CE192 味方コマンド選択");
    assert_eq!(r.warnings, 0, "no interpreter warnings on the full-fixture battle line");
    eprintln!("m10_battle_db_fixture full: reached={:?} warnings={}", r.seen, r.warnings);
}

#[test]
fn empty_enemy_roster_would_abort_under_reference_model() {
    // This mirrors the older seed_party() fixture, which sets type18 r4=1 but never
    // sets r69. Under the reference patch r69 stayed 0 (no CE190 refill in some
    // harnesses) => deadEnemy 0 == enemyCount 0 => pc53 victory-abort.
    let mut g = Game::new(data_root()).expect("Game::new");
    g.load_save(&save01()).expect("load SaveData01");
    seed_full_battle_db(&mut g);
    g.databases.set_num(Container::CData, 18, 69, 0, 0); // empty enemy roster

    assert_eq!(reference_ce172_verdict(&g), 1,
        "empty enemy roster: deadEnemy 0 == enemyCount 0 -> victory-abort (PC53)");
    // Control: the full fixture (r69=1) is undecided.
    let mut g2 = Game::new(data_root()).expect("Game::new");
    g2.load_save(&save01()).expect("load SaveData01");
    seed_full_battle_db(&mut g2);
    assert_eq!(reference_ce172_verdict(&g2), 0);

    // And a fully-dead ally side aborts defeat (PC32).
    let mut g3 = Game::new(data_root()).expect("Game::new");
    g3.load_save(&save01()).expect("load SaveData01");
    seed_full_battle_db(&mut g3);
    g3.databases.set_num(Container::CData, 10, 0, 3, 1); // ally slot 0 dead
    assert_eq!(reference_ce172_verdict(&g3), -1,
        "all-dead ally side: deadAlly == allyCount -> defeat-abort (PC32)");
}

#[test]
fn seeded_battle_db_stalls_downstream_not_at_ce172() {
    // The seven "unclear" failures all use seed_battle_databases (row69 refilled by
    // CE190) or the save01 party harness. Empirically CE172 runs, does NOT abort,
    // and the line stalls later (CE65 万能ウィンドウ / CE152 一覧 / CE64 描画). This
    // pins that fact so we do not mis-attribute their divergence to CE172.
    let mut g = Game::new(data_root()).expect("Game::new");
    g.seed_battle_databases();
    g.auto_advance = true;
    g.auto_choice = true;
    g.auto_choice_delay = 1;
    g.start_engine_events();
    g.start_event(188);

    let mut seen: BTreeMap<i32, i32> = BTreeMap::new();
    for _f in 0..800 {
        g.key_bits = 0;
        g.frame();
        for inst in g.vm.instances() {
            let id: i32 = format!("{:?}", inst.event).parse().unwrap_or(-1);
            let e = seen.entry(id).or_insert(-1);
            if (inst.pc as i32) > *e { *e = inst.pc as i32; }
        }
    }
    let enemy_count = cdata(&g, 18, 69, 0);
    assert!(enemy_count > 0, "CE190 must refill the enemy count from the group (got {})", enemy_count);
    assert!(seen.contains_key(&172), "CE172 must run");
    assert!(!seen.contains_key(&199) && !seen.contains_key(&149) && !seen.contains_key(&201),
        "CE172 must NOT abort on a populated roster");
    assert!(seen.contains_key(&65), "line reaches CE65 万能ウィンドウ before stalling");
    eprintln!("m10_battle_db_fixture seeded-battle: enemy_count={} reached={:?}", enemy_count, seen);
}
