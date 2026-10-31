//! M7 battle seed: prove the production `Game::seed_battle_databases` fills the
//! battle-only `CData` cells a fresh game never initialises, so a real attack becomes
//! selectable in the CE152/CE192 line — from `Game::new` alone, no hand-edited save.
//!
//! See docs/research/M7_BATTLE_SEED.md for the exact cells and the evidence behind the
//! values. The m6 control test (`m6_battle_commands.rs`) still proves that *without*
//! the item-0 seed a real attack is unselectable; here the same run starts from a plain
//! `Game::new` and reaches the selectable state only after the seed.

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

const REAL_ATTACK_CODES: [i32; 4] = [26, 25, 30, 29];

/// Total of the interpreter's `250 unanswered` diagnostics (label-drift reads the
/// engine could not answer). The M7 acceptance bar is 0 for the battle segment.
fn unanswered(game: &Game) -> u64 {
    game.diagnostics
        .iter()
        .filter(|(key, _)| key.contains("unanswered"))
        .map(|(_, value)| *value)
        .sum()
}

struct Outcome {
    real_selectable: bool,
    reached_ce152: bool,
    reached_ce192: bool,
    reached_target_window: bool,
    warnings: u64,
    unanswered: u64,
    peak_instances: usize,
}

fn run(with_seed: bool, frames: usize) -> Outcome {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    if with_seed {
        game.seed_battle_databases();
    }
    let _ = game.start_event(188);

    let mut out = Outcome {
        real_selectable: false,
        reached_ce152: false,
        reached_ce192: false,
        reached_target_window: false,
        warnings: 0,
        unanswered: 0,
        peak_instances: 0,
    };

    for f in 0..frames {
        game.key_bits = if f % 2 == 0 {
            wolf_game::key::OK
        } else {
            0
        };
        let _ = game.frame();
        for inst in game.vm.instances() {
            match format!("{:?}", inst.event).as_str() {
                "152" => out.reached_ce152 = true,
                "192" => out.reached_ce192 = true,
                "157" | "158" => out.reached_target_window = true,
                _ => {}
            }
        }
        for row in 0..8 {
            let code = game.databases.num(Container::CData, 19, row, 1).unwrap_or(0);
            let selectable = game.databases.num(Container::CData, 19, row, 2).unwrap_or(0);
            if REAL_ATTACK_CODES.contains(&code) && selectable == 1 {
                out.real_selectable = true;
            }
        }
    }
    out.warnings = game.stats.warnings;
    out.unanswered = unanswered(&game);
    out.peak_instances = game.stats.peak_instances;
    out
}

#[test]
fn m7_battle_seed_makes_a_real_attack_selectable() {
    // 1. The seed writes exactly the documented cells.
    let mut game = Game::new(data_root()).expect("Game::new");
    let written = game.seed_battle_databases();
    assert_eq!(
        written, 26,
        "seed writes 1 party + 1 enemy + 8 command slots + 12 stats + 1 item + 3 weapons"
    );
    assert_eq!(
        game.databases.num(Container::CData, 18, 4, 0),
        Some(1),
        "party count (type18 row4 col0)"
    );
    assert_eq!(
        game.databases.num(Container::CData, 18, 69, 0),
        Some(1),
        "enemy count (type18 row69 col0)"
    );
    assert_eq!(
        game.databases.num(Container::CData, 0, 12, 47),
        Some(3),
        "command slot 1 (type0 row12 col47)"
    );
    assert_eq!(
        game.databases.num(Container::CData, 0, 12, 48),
        Some(4),
        "command slot 2 (type0 row12 col48)"
    );
    assert_eq!(
        game.databases.num(Container::CData, 7, 0, 0),
        Some(1),
        "item 0 stock (type7 row0 col0)"
    );
    assert_eq!(
        game.databases.num(Container::CData, 8, 0, 0),
        Some(1),
        "weapon 0 stock (type8 row0 col0)"
    );
    drop(game);

    // 2. Control: no seed -> the item gate keeps every real attack unselectable.
    let control = run(false, 300);
    // 3. Seeded: a real attack becomes selectable, with no errors or unanswered reads.
    let seeded = run(true, 300);

    assert!(
        control.reached_ce192,
        "control must still reach CE192 味方コマンド選択 (party/command precondition)"
    );
    assert!(
        !control.real_selectable,
        "without the seed an empty inventory must leave real attacks unselectable"
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
        "with the seed a real attack (code 26/25/30/29) must be selectable"
    );
    assert_eq!(seeded.warnings, 0, "seeded battle segment must log no warnings");
    assert_eq!(
        seeded.unanswered, 0,
        "seeded battle segment must leave no 250 unanswered database read"
    );
    assert!(
        seeded.peak_instances >= 3,
        "the battle line must keep coroutines alive (no deadlock)"
    );

    eprintln!(
        "m7_battle_seed: control(selectable={}) seeded(selectable={}, ce152={}, ce192={}, target_win={}, warnings={}, unanswered={})",
        control.real_selectable,
        seeded.real_selectable,
        seeded.reached_ce152,
        seeded.reached_ce192,
        seeded.reached_target_window,
        seeded.warnings,
        seeded.unanswered,
    );
}
