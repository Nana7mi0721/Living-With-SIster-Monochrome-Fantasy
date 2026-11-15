//! M7 H-mode / intimacy state probe: derive the variable state that the mid-H
//! retail save carries, and pin down why CE333 (the H CG common event) cannot be
//! opened from CE246's 性骚扰 deep branch by a pure variable-state setter.
//!
//! Background (docs/research/M7_HMODE_STATE.md):
//!   * Task-29 proved a non-synthetic H entry: day-2 19:30 bedroom -> night command
//!     menu -> 身体接触 -> CE246 six-way skinship menu. Light options start CE217 (HUD)
//!     and draw hscene/header_bar.jpg, but CE333 (CG) and CE346 (cut-in) stay gated by
//!     CE246's deep 性骚扰 branch.
//!   * This file diffs a fresh day-2 evening against work/retail_backup/Save/SaveData08.sav
//!     (a real mid-H snapshot) to tabulate the elevated intimacy variables, then drives
//!     CE246's deep 性骚扰 -> 小鸟飞飞 (deep option 4) branch to its very end and shows
//!     CE333 is still NOT started.
//!   * The precise remaining gate is CE246 pc1943 "IF var157 == 13". Global var157 has
//!     no literal-13 writer anywhere in the data; it only reaches 13 by copying an
//!     event-local SELF variable (raw 1_600_013, the self-variable magic range) inside
//!     CE282 pc342 or CE329 pc663. A global variable-state loader cannot reproduce it.
//!
//! Harness notes / exact commands:
//!   * cargo test -q -p wolf-game --test m7_hmode_state
//!   * Data root defaults to the extracted retail tree work/all (WOLF_DATA_ROOT overrides).
//!   * The mid-H save path is work/retail_backup/Save/SaveData08.sav (repo-relative).
//!   * Game::load_save restores globals + pictures, NOT coroutines, so after loading we
//!     re-run start_engine_events() and start_event(246) to resume the skinship menu.
//!
//! Variable semantics confirmed by reading the VM this range:
//!   crates/wolf-vm/src/magic.rs:30-36 (SELF_BASE 1_000_000, VAR_BASE 2_000_000;
//!     self range 1_000_000..=1_999_999, ordinary vars 2_000_000..=2_999_999).
//!   crates/wolf-vm/src/exec.rs:1672-1691 (opcode 121 = [target, a1, a2, bits]:
//!     Magic::decode(target) store, a1/a2 through eval_num, so a1 in the self range
//!     is a self-variable copy, a1 below 1_000_000 is a literal).
//!   crates/wolf-vm/src/exec.rs:1489-1503 (opcode 111 compare; op 2 ==).
//!   crates/wolf-vm/src/vars.rs:72-77 (Variables::get/set take a usize index).

use std::path::PathBuf;

use wolf_game::Game;

/// Same data-root resolution as the other M6/M7 tests: default to work/all, overridable
/// with WOLF_DATA_ROOT (relative paths resolve against the repo root = CARGO_MANIFEST_DIR/../..).
fn repo_root() -> PathBuf {
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    base
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".to_owned());
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return path;
    }
    repo_root().join(path)
}

fn mid_h_save() -> PathBuf {
    repo_root().join("work/retail_backup/Save/SaveData08.sav")
}

fn open_game() -> Option<Game> {
    match Game::new(data_root()) {
        Ok(game) => Some(game),
        Err(err) => {
            eprintln!("skipping: {err}");
            None
        }
    }
}

fn decide(game: &mut Game) {
    game.key_bits = wolf_game::key::OK;
    game.frame();
    game.key_bits = 0;
}

fn click_at(game: &mut Game, x: i32, y: i32) {
    game.mouse = [x, y];
    for _ in 0..10 {
        game.frame();
    }
    game.mouse_buttons = 1;
    for _ in 0..5 {
        game.frame();
    }
    game.mouse_buttons = 0;
}

/// Drive a fresh new game through day 1 into the day-2 19:30 bedroom. Mirrors the
/// canonical loop used by m6_night_menu.rs / m6_mainline.rs; the day-2 evening state
/// (measured this range) is bit-identical to the state at the CE246 skinship menu,
/// because merely opening the night command menu does not touch the intimacy variables.

const DAY_COMMAND_LABELS: [&str; 6] = [
    "给店里帮忙", "学习会", "基础锻炼", "攻击练习", "防御练习", "秘密特训",
];
const DAY_ROW_PICS: [i32; 5] = [1201, 1203, 1205, 1207, 1209];

fn day_row_texts(game: &Game) -> Vec<String> {
    DAY_ROW_PICS
        .iter()
        .map(|&number| game.screen.get(number).map(|p| p.text.clone()).unwrap_or_default())
        .collect()
}

fn evening_work(game: &Game) -> bool {
    game.vm
        .instances()
        .iter()
        .any(|instance| matches!(instance.event, 251 | 300 | 301 | 326 | 406))
}

fn day2_evening_game() -> Option<Game> {
    let mut game = open_game()?;
    game.auto_advance = true;
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.start_map_event_by_name("SampleMapA", 0)
        .expect("the new-game entry point exists");
    for _ in 0..1500 {
        game.frame();
    }
    game.auto_choice = false;
    game.vm.vars.set(63, 1);
    for _ in 0..120 {
        game.frame();
        if game.choices.is_some() {
            break;
        }
    }
    game.set_choice_cursor(2);
    decide(&mut game);
    for _ in 0..120 {
        game.frame();
        if game.choices.is_some() {
            break;
        }
    }
    game.set_choice_cursor(0);
    decide(&mut game);
    for _ in 0..400 {
        game.frame();
        if game.vm.vars.get(160) == 2 && game.vm.vars.get(4) == 8 && game.vm.vars.get(5) >= 30 {
            break;
        }
    }
    game.auto_choice = true;
    // The drawn day commands are a retail-PRNG roll; pick the first CE252 daytime
    // task row (地区贡献 would route into the battle).
    let mut chosen: Option<(usize, String)> = None;
    for _ in 0..1200 {
        game.frame();
        for (row, text) in day_row_texts(&game).iter().enumerate() {
            if DAY_COMMAND_LABELS.iter().any(|label| text.contains(label)) {
                chosen = Some((row, text.clone()));
                break;
            }
        }
        if chosen.is_some() {
            break;
        }
    }
    let (row, _label) = chosen.expect("CE251 draws a CE252 daytime command");
    click_at(&mut game, 512, 230 + 50 * row as i32);
    for _ in 0..2500 {
        game.frame();
        if game.vm.instances().iter().all(|instance| instance.event != 251) {
            break;
        }
    }
    for _ in 0..6000 {
        game.frame();
        if !evening_work(&game) && game.vm.vars.get(4) >= 18 {
            break;
        }
    }
    game.auto_choice = false;
    Some(game)
}

/// The derived H-mode variable table: (variable id, fresh day-2 evening, retail SaveData08).
/// Values are read from work/all and work/retail_backup/Save/SaveData08.sav; both columns
/// are asserted exactly so a data/loader drift fails loudly.
const H_MODE_VARS: &[(usize, i32, i32)] = &[
    (7, 3029, 7586),
    (8, 0, 25),
    (13, 60, 120),
    // opcode-111 reference semantics: CE226 pc15 111[18,1600012,1600010,1] now clamps
    // var14 to var13 (fresh day-2 35, was the literal-bug 115) and var16 shifted 4->3.
    (14, 35, 75),
    (16, 3, 2),
    (18, 22, 60),
    (20, 75, 90),
    // op121's argc-5 form operates on a contiguous slot block; the day loop's
    // range clears now leave 20 instead of the old single-slot 18.
    (21, 20, 688),
    // CE38 pc2 sets the 600-slot block 2000001..=2000601 to zero, so a fresh
    // file starts at 0, not the old single-slot template's -1.
    (22, 0, 375),
    (23, 5, 9),
    (28, 105, 1815),
    (62, 0, 1),
    (64, 0, 1),
    (70, 0, 1),
    (102, 0, 120),
    (137, 1, 8),
    (138, 0, 50),
    (150, 0, 44),
    (157, 1, 12),
    (160, 2, 90),
    (200, 0, 506),
    (452, 0, 10),
    (365, 0, 1737),
];

fn le32(params: &[u8], at: usize) -> i64 {
    if at + 4 > params.len() {
        return -1;
    }
    i32::from_le_bytes([params[at], params[at + 1], params[at + 2], params[at + 3]]) as i64
}

#[test]
fn h_mode_var_table_is_derived_from_the_retail_save() {
    // Fresh day-2 evening baseline (real day loop, warning-free).
    let Some(fresh) = day2_evening_game() else {
        return;
    };
    assert_eq!(fresh.stats.warnings, 0, "fresh day-2 evening is warning-free");
    assert_eq!(fresh.vm.vars.get(160), 2, "fresh baseline is day 2");


    // Mid-H retail snapshot.
    let Some(mut retail) = open_game() else {
        return;
    };
    let save = mid_h_save();
    if !save.exists() {
        eprintln!("skipping: {} missing", save.display());
        return;
    }
    retail.load_save(&save).expect("load the mid-H SaveData08 snapshot");
    assert_eq!(retail.stats.warnings, 0, "loading the retail save is warning-free");

    for (id, fresh_expected, retail_expected) in H_MODE_VARS {
        assert_eq!(
            fresh.vm.vars.get(*id),
            *fresh_expected,
            "fresh day-2 var{id} mismatch"
        );
        assert_eq!(
            retail.vm.vars.get(*id),
            *retail_expected,
            "retail SaveData08 var{id} mismatch"
        );
    }

    // The intimacy gates that are non-positive on a fresh file (var22 = 0, and
    // var70/var102/var150/var200/var452 = 0), which is exactly why the deep 性骚扰
    // branch returns early before it can reach the CE333 tail. (var23 is a small
    // positive 5 fresh and is not a "<= 0" default.)
    for id in [22usize, 70, 102, 150, 200, 452] {
        assert!(
            fresh.vm.vars.get(id) <= 0,
            "fresh var{id} should be a non-positive blocking default, got {}",
            fresh.vm.vars.get(id)
        );
        assert!(
            retail.vm.vars.get(id) > fresh.vm.vars.get(id),
            "retail var{id} must be elevated above the fresh default"
        );
    }
    // And the CE333 gate variable itself is different: 1 fresh, 12 in the mid-H save.
    assert_eq!(fresh.vm.vars.get(157), 1, "fresh var157");
    assert_eq!(retail.vm.vars.get(157), 12, "retail var157 is 12, not the required 13");
}

/// Drive the CE246 性骚扰 -> 小鸟飞飞 (deep option 4) branch to completion after loading
/// the mid-H snapshot, optionally forcing the global CE333 gate. Returns
/// (ever_333, ever_346, max_pc_246, final_pc_246_option, warnings, var157_after).
fn drive_deep_branch(force_gate: bool) -> Option<(bool, bool, usize, Option<usize>, u64, i32)> {
    let mut game = open_game()?;
    let save = mid_h_save();
    if !save.exists() {
        eprintln!("skipping: {} missing", save.display());
        return None;
    }
    game.load_save(&save).expect("load mid-H snapshot");
    game.start_engine_events();
    game.start_event(246);
    for _ in 0..400 {
        game.frame();
        if game.choices.is_some() {
            break;
        }
    }
    // Six-way skinship menu: [摸摸头, 揉肩, 拥抱, 亲吻, 挠痒痒, 性骚扰].
    assert_eq!(
        game.choices.as_ref().map(|c| c.options.len()),
        Some(6),
        "CE246 six-way skinship menu"
    );
    game.set_choice_cursor(5); // 性骚扰
    decide(&mut game);
    for _ in 0..400 {
        game.frame();
        if game.choices.is_some() {
            break;
        }
    }
    assert!(
        game.choices.is_some(),
        "性骚扰 opens the deep five-way submenu"
    );
    game.set_choice_cursor(4); // 小鸟飞飞 (deep option 4)
    decide(&mut game);

    game.auto_advance = true;
    game.auto_choice = true;
    game.set_choice_cursor(0);
    game.auto_choice_delay = 1;

    let mut max_pc = 0usize;
    let mut ever_333 = false;
    let mut ever_346 = false;
    for _ in 0..3000 {
        if force_gate {
            game.vm.vars.set(157, 13);
        }
        game.frame();
        if let Some(pc) = game
            .vm
            .instances()
            .iter()
            .find(|i| i.event == 246)
            .map(|i| i.pc)
        {
            if pc > max_pc {
                max_pc = pc;
            }
        }
        for inst in game.vm.instances() {
            if inst.event == 333 {
                ever_333 = true;
            }
            if inst.event == 346 {
                ever_346 = true;
            }
        }
    }
    let final_pc = game
        .vm
        .instances()
        .iter()
        .find(|i| i.event == 246)
        .map(|i| i.pc);
    Some((
        ever_333,
        ever_346,
        max_pc,
        final_pc,
        game.stats.warnings,
        game.vm.vars.get(157),
    ))
}

#[test]
fn ce333_deep_branch_needs_an_event_local_self_variable_gate() {
    // (a) Static proof: global var157 (magic 2_000_157) has no literal-13 writer.
    //     Its only writers are CE258/CE318/CE329/CE453 with literals {16,2,1} and two
    //     event-local SELF copies (raw 1_600_013) in CE282 pc342 and CE329 pc663.
    let ce_path = data_root().join("BasicData/CommonEvent.dat");
    let bytes = match std::fs::read(&ce_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("skipping: cannot read {}: {err}", ce_path.display());
            return;
        }
    };
    let common = wolf_format::commonevent::CommonEventData::parse(&bytes)
        .expect("parse CommonEvent.dat");
    use wolf_format::command::Slot;
    let mut literal_values: Vec<i64> = Vec::new();
    let mut self_copies: Vec<(usize, usize, i64)> = Vec::new();
    for (event_index, event) in common.events.iter().enumerate() {
        let mut pc = 0usize;
        for slot in &event.commands {
            if let Slot::Command(cmd) = slot {
                if cmd.opcode == 121 && cmd.params.len() >= 16 && le32(&cmd.params, 0) == 2_000_157 {
                    let a1 = le32(&cmd.params, 4);
                    if (1_000_000..2_000_000).contains(&a1) {
                        self_copies.push((event_index + 1, pc, a1));
                    } else {
                        literal_values.push(a1);
                    }
                }
            }
            pc += 1;
        }
    }
    literal_values.sort_unstable();
    literal_values.dedup();
    assert_eq!(
        literal_values,
        vec![1, 2, 16],
        "the only literal values ever stored into global var157 are 1, 2, 16 — never 13"
    );
    assert!(
        !literal_values.contains(&13),
        "no literal writer sets var157 = 13, the CE246 pc1943 gate"
    );
    assert_eq!(
        self_copies,
        vec![(282, 342, 1_600_013), (329, 663, 1_600_013)],
        "var157 reaches 13 only by copying the event-local SELF variable 1_600_013 \
         (self range 1_000_000..2_000_000) inside CE282/CE329"
    );

    // (b) Empirical proof: even from the real mid-H snapshot, driving 性骚扰 -> 小鸟飞飞
    //     to the branch's end (max pc > 2000, event then returns via pc2058 CALL ev245)
    //     never starts CE333 or CE346. Forcing global var157 = 13 each frame changes
    //     nothing, because the gate is fed by an event-local self variable, not a global.
    let Some((ever_333, ever_346, max_pc, final_pc, warnings, var157)) = drive_deep_branch(false)
    else {
        return;
    };
    assert_eq!(warnings, 0, "the deep branch runs warning-free");
    assert!(
        max_pc >= 2000,
        "the deep branch runs to its tail (max pc {max_pc} >= 2000)"
    );
    assert_eq!(
        final_pc, None,
        "CE246 ends cleanly (returns to the night menu via pc2058 CALL ev245); no deadlock"
    );
    assert_eq!(var157, 12, "the mid-H snapshot carries var157 == 12, not 13");
    assert!(!ever_333, "CE333 (H CG) is NOT started by the deep variable state");
    assert!(!ever_346, "CE346 (cut-in) is NOT started by the deep variable state");

    // Forcing the global gate each frame still cannot open CE333 — this is the blocker.
    let Some((forced_333, forced_346, _, _, forced_warnings, forced_var157)) =
        drive_deep_branch(true)
    else {
        return;
    };
    assert_eq!(forced_warnings, 0, "forcing var157 stays warning-free");
    assert_eq!(forced_var157, 13, "the forced global var157 is 13");
    assert!(
        !forced_333 && !forced_346,
        "forcing global var157 = 13 STILL does not start CE333/CE346: the real gate is the \
         event-local self variable 1_600_013 copied inside CE282/CE329"
    );
}
