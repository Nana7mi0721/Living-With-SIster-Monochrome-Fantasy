//! M7 battle command-menu DB: prove the production `Game::seed_battle_databases`
//! supplies the command-name text the 万能ウィンドウ (CE65/CE64) reads, and that the
//! seeded real battle segment still reaches CE152/CE192 with no warnings.
//!
//! The strings live in `CData type 19` column 0 (万能ウィンドウ一時DB「項目文」). CE152
//! 戦闘コマンド一覧算出 is the event that normally builds them: it reads `Data type 6`
//! column 0「コマンド名」 and writes the same column 0 of type 19 (docs/research/
//! M7_BATTLE_MENU_DB.md). `battle_seed::seed` pre-fills both halves so a fresh game has
//! the exact text, not just the selectable codes.
//!
//! This file also pins the engine fix (task-39): the interpreter's opcode-250 write
//! path used to flatten the *value operand* of an argc-5 write to a number, so CE152's
//! own rebuild wrote blank command names. The write path now keeps a string that a
//! prior 250 read stored in the operand's self-variable string bank
//! (crates/wolf-vm/src/exec.rs opcode-250 write branch), so CE152 writes the real
//! per-slot labels; the attack_labels capture below proves it explicitly.

use std::path::PathBuf;

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

/// The command-name labels CE152 copies out of `Data type 6` column 0 for the four real
/// attacks seeded into slots 1..4, plus 普通攻撃 for the empty tail slots.
const EXPECTED_LABELS: [&str; 8] = [
    "蓄力",
    "防御",
    "强斩",
    "反击",
    "普通攻击",
    "普通攻击",
    "普通攻击",
    "普通攻击",
];

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
    post_run_labels: Vec<String>,
    attack_labels: Vec<String>,
}

fn run(with_seed: bool, frames: usize) -> Outcome {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    if with_seed {
        let written = game.seed_battle_databases();
        assert_eq!(written, 26, "seed still reports the 26 numeric cells");
    } else {
        // Party-only roster: under opcode-111 reference semantics CE172 pc21/pc32
        // needs a nonzero ally count (type18 row4 col0) or it aborts 0==0 before
        // CE192. Seed just the party; item/weapon/command cells stay empty so a
        // real attack is still unselectable in the control run.
        game.databases
            .set_num(wolf_game::db::Container::CData, 18, 4, 0, 1);
        // One enemy (type18 row69 col0): 0 enemies makes CE172 pc42/pc53 see 0==0 and
        // victory-abort before CE192; one enemy keeps the line alive. Not a seed that
        // makes real attacks selectable.
        game.databases
            .set_num(wolf_game::db::Container::CData, 18, 69, 0, 1);
        for (col, val) in [
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
            game.databases
                .set_num(wolf_game::db::Container::CData, 0, 12, col, val);
        }
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
        post_run_labels: Vec::new(),
        attack_labels: Vec::new(),
    };

    for f in 0..frames {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
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
            let code = game
                .databases
                .num(wolf_game::db::Container::CData, 19, row, 1)
                .unwrap_or(0);
            let selectable = game
                .databases
                .num(wolf_game::db::Container::CData, 19, row, 2)
                .unwrap_or(0);
            if REAL_ATTACK_CODES.contains(&code) && selectable == 1 {
                out.real_selectable = true;
                if out.attack_labels.is_empty() {
                    let labels: Vec<String> = (0..8)
                        .map(|row| {
                            game.databases
                                .text(wolf_game::db::Container::CData, 19, row, 0)
                                .unwrap_or("")
                                .to_string()
                        })
                        .collect();
                    // Capture the first frame CE152 has filled the four real-attack
                    // labels (row0..3 = 蓄力/防御/强斩/反击).
                    if labels.first().map(String::as_str) == Some("蓄力")
                        && labels.get(1).map(String::as_str) == Some("防御")
                        && labels.get(2).map(String::as_str) == Some("强斩")
                        && labels.get(3).map(String::as_str) == Some("反击")
                    {
                        out.attack_labels = labels;
                    }
                }
            }
        }
    }
    out.warnings = game.stats.warnings;
    out.unanswered = unanswered(&game);
    out.peak_instances = game.stats.peak_instances;
    out.post_run_labels = (0..8)
        .map(|row| {
            game.databases
                .text(wolf_game::db::Container::CData, 19, row, 0)
                .unwrap_or("")
                .to_string()
        })
        .collect();
    out
}

#[test]
fn m7_battle_menu_text_is_seeded_and_the_line_runs_clean() {
    // 1. The seed writes the command-menu labels into 万能ウィンドウ一時DB column 0.
    let mut game = Game::new(data_root()).expect("Game::new");
    let written = game.seed_battle_databases();
    assert_eq!(
        written, 26,
        "numeric seed invariant unchanged (1 party + 1 enemy + 8 slots + 12 stats + 1 item + 3 weapons)"
    );
    for (row, expected) in EXPECTED_LABELS.iter().enumerate() {
        assert_eq!(
            game.databases
                .text(wolf_game::db::Container::CData, 19, row as i32, 0),
            Some(*expected),
            "type19 row {row} column 0 項目文 = {expected}"
        );
    }

    // 2. The strings really come from Data type6 コマンド名 (CE152's own source), so the
    //    seed is data-faithful and not a hardcoded overlay of a different table.
    assert_eq!(
        game.databases.text(wolf_game::db::Container::Data, 6, 3, 0),
        Some("蓄力"),
        "Data type6 row3 コマンド名 drives slot1"
    );
    assert_eq!(
        game.databases.text(wolf_game::db::Container::Data, 6, 4, 0),
        Some("防御"),
        "Data type6 row4 コマンド名 drives slot2"
    );
    assert_eq!(
        game.databases.text(wolf_game::db::Container::Data, 6, 6, 0),
        Some("强斩"),
        "Data type6 row6 コマンド名 drives slot3"
    );
    assert_eq!(
        game.databases.text(wolf_game::db::Container::Data, 6, 7, 0),
        Some("反击"),
        "Data type6 row7 コマンド名 drives slot4"
    );
    drop(game);

    // 3. Control vs seeded: the seeded run reaches the command-menu events and selects a
    //    real attack; the control confirms an empty inventory does not.
    let control = run(false, 300);
    let seeded = run(true, 300);

    assert!(
        control.reached_ce192,
        "control must still reach CE192 味方コマンド選択"
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
    assert!(
        seeded.reached_target_window,
        "the target window (CE157/CE158) must open after a command is chosen"
    );
    // The engine fix is observable: at the frame a real attack becomes selectable, CE152
    // has written the four correct per-slot labels into type 19 column 0 (before the fix
    // these rows were blank because the value operand was coerced to a number).
    assert_eq!(
        seeded.attack_labels.len(),
        8,
        "CE152 must fill all eight command rows while building the menu"
    );
    assert_eq!(
        &seeded.attack_labels[0..4],
        &["蓄力", "防御", "强斩", "反击"],
        "CE152 must write the four real-attack labels, not blanks"
    );
    assert!(
        seeded.post_run_labels.iter().all(|label| !label.is_empty()),
        "no command row may stay blank after CE152 runs: {:?}",
        seeded.post_run_labels
    );
    assert_eq!(
        seeded.warnings, 0,
        "seeded battle segment must log no warnings"
    );
    assert_eq!(
        seeded.unanswered, 0,
        "seeded battle segment must leave no 250 unanswered database read"
    );
    assert!(
        seeded.peak_instances >= 3,
        "the battle line must keep coroutines alive (no deadlock)"
    );

    eprintln!(
        "m7_battle_menu_text: seeded labels={:?} | control(selectable={}) seeded(selectable={}, ce152={}, ce192={}, target_win={}, warnings={}, unanswered={}) | post-run type19 col0={:?} | attack-frame labels={:?}",
        EXPECTED_LABELS,
        control.real_selectable,
        seeded.real_selectable,
        seeded.reached_ce152,
        seeded.reached_ce192,
        seeded.reached_target_window,
        seeded.warnings,
        seeded.unanswered,
        seeded.post_run_labels,
        seeded.attack_labels,
    );
}
