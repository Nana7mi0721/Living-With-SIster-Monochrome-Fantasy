//! M7 W1 target selection: the production battle seed carries the enemy count
//! (CData type 18 row 69 column 0), so CE190 戦闘キャラ配置 lays out a drawable
//! enemy and CE162 敵対象選択実行 can select it.
//!
//! CE162 pc12 reads type18 row69 into its enemy count. When that count is 0 the
//! event never leaves the pc49 <-> pc50 wait loop; with row69 = 1 it walks
//! pc12 -> pc34 -> pc49 -> pc50 -> pc55 -> pc57 and exits with enemy index 0
//! selected. The real-target caller CE159 技能選択実行 still does not spawn in
//! CE188-line rounds, which is the remaining gameplay gap (see
//! docs/research/M7_BATTLE_TARGET_SELECT.md).

use std::collections::BTreeSet;
use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_game::db::Container;
use wolf_game::key;
use wolf_game::Game;

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        return p;
    }
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    base.join(p)
}

fn cdata(g: &Game, db_type: i32, row: i32, column: i32) -> i32 {
    g.databases
        .num(Container::CData, db_type, row, column)
        .unwrap_or(-9999)
}

fn instance_pc(g: &Game, event: &str) -> Option<i32> {
    for i in g.vm.instances() {
        if format!("{:?}", i.event) == event {
            return Some(i.pc as i32);
        }
    }
    None
}

fn has_instance(g: &Game, event: &str) -> bool {
    g.vm
        .instances()
        .iter()
        .any(|i| format!("{:?}", i.event) == event)
}

fn seeded_game() -> Game {
    let mut g = Game::new(data_root()).expect("Game::new");
    g.seed_battle_databases();
    g.auto_advance = true;
    g.auto_choice = true;
    g.auto_choice_delay = 1;
    g
}

fn drive(g: &mut Game, frames: usize) {
    for f in 0..frames {
        g.key_bits = if f % 2 == 0 { key::OK } else { 0 };
        g.frame();
    }
}

/// Static contract: CE162 reads type18 row69, and the production seed now fills it.
#[test]
fn production_seed_carries_the_enemy_count_ce162_reads() {
    let bytes = std::fs::read(data_root().join("BasicData/CommonEvent.dat")).expect("CommonEvent.dat");
    let events = CommonEventData::parse(&bytes).expect("parse CommonEvent.dat");
    let ce162 = events.event(162).expect("CE162");
    assert!(ce162.title.contains("敵対象選択実行"), "CE162 title={}", ce162.title);

    let mut reads_row69 = false;
    for slot in &ce162.commands {
        if let Slot::Command(c) = slot {
            if c.opcode == 250 {
                let w: Vec<i32> = c
                    .params
                    .chunks_exact(4)
                    .map(|x| i32::from_le_bytes([x[0], x[1], x[2], x[3]]))
                    .collect();
                if w.len() >= 5 && w[0] == 20 && w[1] == 69 && w[2] == 0 {
                    reads_row69 = true;
                }
            }
        }
    }
    assert!(reads_row69, "CE162 pc12 must read CData type18 row69 col0");

    let mut g = seeded_game();
    assert_eq!(cdata(&g, 18, 4, 0), 1, "party count is seeded");
    assert_eq!(cdata(&g, 18, 69, 0), 1, "production seed now writes the enemy count");

    g.start_event(190);
    drive(&mut g, 600);
    assert_eq!(cdata(&g, 18, 69, 0), 1, "CE190 preserves the enemy count");
    assert!(cdata(&g, 10, 0, 2) > 0, "CE144 copied a live enemy HP row into type10");
    assert_eq!(cdata(&g, 18, 35, 0), 512, "CE175 enemy x");
    // CE175 pc72 is one op121 with an inclusive last offset of 3, so it scales
    // all four consecutive coordinates (x, y, x-scale, y-scale) by 1/10, not
    // just x. The old value -2880 came from treating it as a single slot; -288
    // is the correctly scaled y that the interpreter now stores.
    assert_eq!(cdata(&g, 18, 36, 0), -288, "CE175 enemy y");
    // pc72 scales the whole contiguous block (x, y, x-scale, y-scale) by 1/10.
    assert_eq!(cdata(&g, 18, 37, 0), 512, "CE175 enemy draw x scale");
    assert_eq!(cdata(&g, 18, 38, 0), 312, "CE175 enemy draw y scale");
    assert_eq!(g.stats.warnings, 0);
}

/// Seeded target selection: CE162 reaches pc57 and exits, enemy index 0 selected.
#[test]
fn seeded_enemy_count_lets_ce162_select_enemy_index_zero() {
    let mut g = seeded_game();
    g.start_event(190);
    drive(&mut g, 600);
    assert_eq!(cdata(&g, 18, 69, 0), 1);

    assert!(g.start_event(162), "CE162 starts");
    let mut pcs = BTreeSet::new();
    for f in 0..300 {
        g.key_bits = if f % 2 == 0 { key::OK } else { 0 };
        g.frame();
        if let Some(pc) = instance_pc(&g, "162") {
            pcs.insert(pc);
        }
    }
    assert!(pcs.contains(&12), "CE162 must read the enemy count at pc12, saw {pcs:?}");
    assert!(pcs.contains(&57), "CE162 must reach pc57 (enemy index 0 selected), saw {pcs:?}");
    assert!(!has_instance(&g, "162"), "CE162 exits after a successful selection");
    assert_eq!(g.stats.warnings, 0);
}

/// Control: clearing just the enemy-count cell reproduces the pc49/pc50 spin.
#[test]
fn without_enemy_count_ce162_spins_at_pc49_pc50() {
    let mut g = seeded_game();
    g.databases.set_num(Container::CData, 18, 69, 0, 0);
    assert_eq!(cdata(&g, 18, 69, 0), 0);

    g.start_event(190);
    drive(&mut g, 400);
    assert_eq!(cdata(&g, 18, 69, 0), 0, "CE190 cannot invent an enemy count");
    assert!(cdata(&g, 10, 0, 2) > 0, "CE144 still lands one type10 enemy row");

    assert!(g.start_event(162), "CE162 starts");
    let mut saw57 = false;
    let mut max_pc = -1;
    for f in 0..240 {
        g.key_bits = if f % 2 == 0 { key::OK } else { 0 };
        g.frame();
        if let Some(pc) = instance_pc(&g, "162") {
            max_pc = max_pc.max(pc);
            if pc == 57 {
                saw57 = true;
            }
        }
    }
    assert!(!saw57, "CE162 must not reach pc57 without an enemy count");
    assert!(has_instance(&g, "162"), "CE162 stays active, spinning pc49/pc50");
    let pc = instance_pc(&g, "162").expect("CE162 still running");
    assert!(pc == 49 || pc == 50, "control CE162 stuck at pc49/pc50, got pc{pc} (max {max_pc})");
    assert_eq!(g.stats.warnings, 0);
}
