//! M6 battle-real line probe (task-8 / task-12).
//!
//! Goal: drive the REAL battle line (CE188 -> CE189 -> CE190 -> CE191 -> CE192 -> CE65 ->
//! CE157 command selection) from a real loaded save with genuine player data, without the
//! legacy CE242/CE243 seeded path (2000350 two-state scheduler).
//!
//! Verified reachable depth and the boundary are documented in
//! docs/research/M6_BATTLE_REAL.md.  With a battle-ready party row seeded at the CORRECT
//! 主人公ステータス columns, CE143 copies real HP/maxHP/action into CData type10 戦闘一時ステータス
//! (confirmed: type10 row0 = [0,100,100,...]).  The line still does NOT complete a turn or
//! reach CE244 settlement: CE152's command-availability logic marks the real attack command
//! rows (item codes 26/25) unavailable in CData type19 col2, leaving only the two sentinel
//! rows (-2 何もしない / -1) selectable, so CE244 (sole writer of 2000352/2000343) is never hit.
use std::path::PathBuf;

use wolf_game::Game;
use wolf_game::db::Container;

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let p = PathBuf::from(raw);
    if p.is_absolute() { return p; }
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop(); b.pop();
    b.join(p)
}
fn save01() -> PathBuf {
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop(); b.pop();
    b.join("work/retail_backup/Save/SaveData01.sav")
}

/// Seed exactly what a genuine party record carries, using the CORRECT
/// CData type0 主人公ステータス columns: 20 maxHP, 21 HP, 22 maxMP, 23 MP, 24 melee,
/// 25 ranged, 26 defense, 27 max action, 28 action, 47..54 コマンド1..8.
/// This mirrors real player data; it does not fabricate menu rows (CData type 19).
fn seed_party(game: &mut Game) {
    game.databases.set_num(Container::CData, 18, 4, 0, 1); // 基本システム用変数 row4 = party count
    let cmds = [1i32, 2, 3, 4, 0, 0, 0, 0];
    for (i, v) in cmds.iter().enumerate() {
        game.databases.set_num(Container::CData, 0, 12, 47 + i as i32, *v); // コマンド1..8
    }
    for (c, v) in [
        (20, 100), // max HP
        (21, 100), // HP
        (22, 50),  // max MP
        (23, 50),  // MP
        (24, 20),  // melee attack
        (25, 15),  // ranged attack
        (26, 10),  // defense
        (27, 30),  // max action
        (28, 30),  // action
        (9, 20),
        (10, 10),
        (4, 10),   // level
    ] {
        game.databases.set_num(Container::CData, 0, 12, c, v);
    }
}

fn inst_stack(game: &Game) -> String {
    game.vm.instances().into_iter().map(|i| format!("{:?}@{} ", i.event, i.pc)).collect()
}

#[test]
fn m6_battle_real_reaches_command_selection() {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;

    let _ = game.start_event(188); // ■戦闘処理 — real line, no CE242/CE243

    let mut reached_ce143 = false;
    let mut reached_ce192 = false;
    let mut reached_ce157 = false;
    let mut selectable_cmd_row = false;
    let mut type10_checked = false;
    let mut t10_hp = 0i32;
    let mut t10_maxhp = 0i32;
    let mut t10_action = 0i32;
    let mut t10_row0 = [0i32; 16];
    let mut enemy_hp = -1i32;
    let mut deepest = String::new();

    for f in 0..300 {
        if (1..=45).contains(&f) { seed_party(&mut game); }

        // Drive the 万能ウィンドウ (CE65) like a player: if the command window is open on
        // its first row (self1==4, cursor self11==0) press DOWN to leave the 何もしない
        // sentinel row; otherwise confirm with OK.  Other windows/frames pulse OK.
        let mut cmdwin = None::<(i32, i32, i32)>; // (self1, self11, self25)
        for i in game.vm.instances() {
            if format!("{:?}", i.event) == "65" {
                cmdwin = Some((i.self_vars.get(601), i.self_vars.get(611), i.self_vars.get(625)));
            }
        }
        game.key_bits = match cmdwin {
            Some((4, 0, rc)) if rc >= 2 => wolf_game::key::DOWN,
            Some((4, _, _)) => wolf_game::key::OK,
            _ => if f % 2 == 0 { wolf_game::key::OK } else { 0 },
        };

        let _ = game.frame();

        let stack = inst_stack(&game);
        for tok in stack.split_whitespace() {
            if tok.starts_with("143@") { reached_ce143 = true; }
            if tok.starts_with("192@") { reached_ce192 = true; }
            if tok.starts_with("157@") { reached_ce157 = true; }
        }
        // Capture the copied battle stats once CE143 has had ~7 frames to write type10.
        if reached_ce143 && f >= 7 && !type10_checked {
            for c in 0..16 { t10_row0[c] = game.databases.num(Container::CData, 10, 0, c as i32).unwrap_or(0); }
            t10_maxhp = t10_row0[1];
            t10_hp = t10_row0[2];
            t10_action = t10_row0[12];
            type10_checked = true;
        }
        // Enemy row 10 (CE144 copies Data type9 into type10 enemy rows) once placed.
        if f >= 30 && enemy_hp < 0 {
            for r in 0..17 {
                let hp = game.databases.num(Container::CData, 10, r, 2).unwrap_or(0);
                if hp > 0 { enemy_hp = hp; break; }
            }
        }
        if cmdwin.is_some() {
            let rc = game.databases.num(Container::CData, 18, 17, 0).unwrap_or(0);
            let sel = game.databases.num(Container::CData, 19, 0, 2).unwrap_or(0);
            if rc >= 1 && sel == 1 { selectable_cmd_row = true; }
        }
        if !stack.is_empty() { deepest = stack; }
    }

    eprintln!("reached_ce143={reached_ce143} reached_ce192={reached_ce192} reached_ce157={reached_ce157} selectable_cmd_row={selectable_cmd_row}");
    eprintln!("type10 row0 = {:?}  (maxHP={t10_maxhp} HP={t10_hp} action={t10_action})", t10_row0);
    eprintln!("enemy type10 HP found = {enemy_hp}");
    eprintln!("warnings={} diagnostics={:?}", game.stats.warnings, game.diagnostics);
    eprintln!("v350={:?} v352={:?} v343={:?}", game.vm.vars.get(350), game.vm.vars.get(352), game.vm.vars.get(343));
    eprintln!("boundary stack = [{deepest}]");

    // 1) CE143/CE144 DO copy real battle stats into CData type10 (correct columns).
    assert!(reached_ce143, "CE143 主人公DB→戦闘DBコピー never ran");
    assert_eq!(t10_maxhp, 100, "type10 row0 maxHP (col1) should be copied from type0 row12 c20");
    assert_eq!(t10_hp, 100, "type10 row0 HP (col2) should be copied from type0 row12 c21");
    // Full copied vector (Lead-measured, reproduced here). Note the seeded type0 c27/c28
    // "action" values land at type10 action-count(13)=100 / body(14)=50 in this build;
    // the semantic "action" slot (col12) stays 0.  HP/maxHP copy is the load-bearing fact.
    assert_eq!(
        t10_row0,
        [0, 100, 100, 0, 0, 0, 0, 0, 20, 10, 0, 0, 0, 100, 50, 0],
        "type10 row0 must equal the measured copied battle-stat vector"
    );
    assert!(enemy_hp > 0, "CE144 should copy a positive HP into a type10 enemy row");

    // 2) The line still reaches player command selection ...
    assert!(reached_ce192, "CE192 味方コマンド選択 never reached");
    assert!(selectable_cmd_row, "no selectable command row ever appeared in the CE192 window");
    assert!(reached_ce157, "CE157 コマンド選択 never reached");

    // 3) ... but the real attack commands are unavailable, so CE244 settlement is never reached.
    assert_eq!(game.vm.vars.get(352), 0, "v352 (勝ち負け) must stay 0 — CE244 not reached");
    assert_eq!(game.vm.vars.get(343), 0, "v343 (戦闘終了) must stay 0 — CE244 not reached");
    assert_eq!(game.stats.warnings, 0, "engine emitted warnings during the battle line");
}
