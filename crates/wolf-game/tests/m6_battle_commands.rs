//! M6 task-13: make a REAL battle command selectable in the CE188/CE192/CE65 line.
//!
//! Finding (see docs/research/M6_BATTLE_COMMANDS.md):
//!   CE152 (戦闘コマンド一覧算出) marks each command row usable only after it passes
//!   the "発動アイテム要する?[消費]" check at pc89..pc95. CE152 reads the skill row's
//!   item-requirement cell (Data type0 col20 / label 発動アイテム要する?[消費]); that
//!   column is absent from the shipped Data type0 (field_count 18) so the read leaves
//!   the destination variable at 0, CE152 takes the ">= 0" branch and then reads the
//!   party's stock of item 0 from CData type7 (所持アイテム個数) row0 col0. With an
//!   empty inventory that is 0, so pc93 (count <= 0) fires and every real attack row is
//!   written back as unusable (CData type21 usability flag = 0).
//!
//!   Seeding the party inventory — CData type7 (所持アイテム個数) row0 col0 = 1 —
//!   flips every real attack row (command codes 26/25/30/29) selectable (type19 col2=1),
//!   a genuine data prerequisite, not a fabricated menu row. Confirming a row then
//!   drives CE192 pc208 -> CE20 コマンド処理 -> CE157/CE158 target selection.
//!
//! Boundary (pinned by this test, not faked):
//!   * The full turn never settles from CE188 alone: CE188 (X◆戦闘処理) does not call
//!     CE244; only CE239 (pc201..), CE241, CE242, CE243 call CE244 (settlement writer
//!     of 2000352/2000343). Reaching CE244 needs the CE239 battle-caller context.
//!   * After a command is confirmed the target-selection window (CE157 味方欄 / CE158
//!     敵欄) returns without committing a target under plain OK input, so CE192 loops
//!     back to the command list; the elapsed-turn counter 2000349 does not advance.
//!   * CE239 driven straight from a loaded save stalls at pc13 (it needs a live map /
//!     battle presentation context - diagnostic "201 SetMoveRoute ... no map layer").
//!
//! The test therefore asserts the deepest honest milestone: with only the inventory
//! seed a real attack row becomes selectable, and without it none does.

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

/// Party + hero command slots, exactly as CE190/CE143 expect them. This is the
/// precondition M6_BATTLE_REAL.md already documents; it does not touch the command
/// availability logic under test.
fn seed_party(game: &mut Game) {
    game.databases.set_num(Container::CData, 18, 4, 0, 1); // party count = 1
    let cmds = [3, 4, 6, 7, 0, 0, 0, 0]; // command ids -> Data type6 rows
    for (i, v) in cmds.iter().enumerate() {
        game.databases
            .set_num(Container::CData, 0, 12, 47 + i as i32, *v);
    }
    for (c, v) in [
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
    ] {
        game.databases.set_num(Container::CData, 0, 12, c, v);
    }
}

const REAL_ATTACK_CODES: [i32; 4] = [26, 25, 30, 29];

struct Outcome {
    real_selectable: bool,
    reached_ce152: bool,
    reached_ce192: bool,
    reached_target_window: bool,
    saw_ce244: bool,
}

fn run(with_inventory: bool, frames: usize) -> Outcome {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let _ = game.start_event(188);

    let mut out = Outcome {
        real_selectable: false,
        reached_ce152: false,
        reached_ce192: false,
        reached_target_window: false,
        saw_ce244: false,
    };

    for f in 0..frames {
        if (1..=60).contains(&f) {
            seed_party(&mut game);
        }
        if with_inventory {
            // CData type7 = 所持アイテム個数; row 0 = item 0; col 0 = count.
            game.databases.set_num(Container::CData, 7, 0, 0, 1);
        }
        // Plain "Decide" pulse; the shop/battle windows only need the press edge.
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();

        for inst in game.vm.instances() {
            match format!("{:?}", inst.event).as_str() {
                "152" => out.reached_ce152 = true,
                "192" => out.reached_ce192 = true,
                "157" | "158" => out.reached_target_window = true,
                "244" => out.saw_ce244 = true,
                _ => {}
            }
        }
        // A real attack row is selectable when a 万能ウィンドウ row carries one of the
        // real command codes and its 選択可能 cell is 1.
        for row in 0..8 {
            let code = game
                .databases
                .num(Container::CData, 19, row, 1)
                .unwrap_or(0);
            let selectable = game
                .databases
                .num(Container::CData, 19, row, 2)
                .unwrap_or(0);
            if REAL_ATTACK_CODES.contains(&code) && selectable == 1 {
                out.real_selectable = true;
            }
        }
    }
    out
}

#[test]
fn m6_battle_commands_real_attack_selectable_with_inventory() {
    // Control: party + command slots but an empty inventory -> real attacks unusable.
    let control = run(false, 200);
    // With one copy of item 0 -> every real attack row becomes selectable.
    let seeded = run(true, 200);

    assert!(
        control.reached_ce192,
        "control run must reach CE192 味方コマンド選択 (party/command seed is a precondition)"
    );
    assert!(
        !control.real_selectable,
        "control run (empty inventory) must NOT expose a selectable real attack row"
    );
    assert!(
        seeded.reached_ce152,
        "seeded run must reach CE152 戦闘コマンド一覧算出"
    );
    assert!(
        seeded.reached_ce192,
        "seeded run must reach CE192 味方コマンド選択"
    );
    assert!(
        seeded.real_selectable,
        "with item 0 in stock a real attack row (code 26/25/30/29) must be selectable"
    );
    // Honest boundary: a full turn settlement is not reachable from CE188.
    assert!(
        !seeded.saw_ce244,
        "CE188 line must not call CE244 (structural boundary)"
    );
    assert!(!control.saw_ce244, "control must not reach CE244 either");

    eprintln!(
        "m6_battle_commands: control(selectable={}) seeded(selectable={}, ce152={}, ce192={}, target_win={}, ce244={})",
        control.real_selectable,
        seeded.real_selectable,
        seeded.reached_ce152,
        seeded.reached_ce192,
        seeded.reached_target_window,
        seeded.saw_ce244,
    );
}
