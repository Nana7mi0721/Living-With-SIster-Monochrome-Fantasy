//! M9 W2 - drive the real H CG cluster end-to-end from the proven var460 / ev33 path.
//!
//! The predecessor note (M9_HSCENE_CG_UNLOCK.md, test m8_hscene_cg_unlock.rs) proved a
//! legitimate input - hovering the code-35 'insert-pull' button (picture 444) - makes map
//! event 11 write var460 = 1 (pc705 sets var61 = 35, pc706 sets var460 = 1), arming map
//! event 33 (whose sole page-0 condition is var460 == 1). Event 33 then calls one common
//! event - CE333 - fifteen times (op210 500333 at pcs 60/103/210/224/231/245/252/254/
//! 258/264/266/269/275/282/285).
//!
//! This file answers the open question 'what does that call chain actually drive, and can we
//! reach the deep CE246 / CE328 / CE402 branch from it?' Two facts settle it, one empirical,
//! one static:
//!
//! 1. Empirical (live): running ev33 on the three retail saves a player would really hold
//!    there - the drained mid-H save SaveData08 and the two day saves the task names
//!    (SaveData07, SaveData04) - makes CE333 DRAW THE H CG PICTURE CLUSTER: >= 20 distinct
//!    picture files including hscene/h_bg_001.jpg, hscene/h_frame.png, the pose sheet
//!    selected by var137 (h_s01..h_s10), DLC_data/pose_10.png, cg/bg_14_sunset_a and
//!    cg/blackscreen.jpg, with var460 reaching 1, ev33 running and 0 warnings.
//!
//! 2. Static (bytecode): CE333 has ZERO op210 calls - it is a leaf CG-drawing body (401
//!    op150 picture draws). ev33's only callee is 500333 (= CE333). So ev33 -> CE333 does
//!    NOT reach CE246 / CE328 / CE402 at all; those are separate menu-entry common events.
//!    CE246 calls CE333 in the REVERSE direction, and only at pc1983, inside the
//!    var157 == 13 block opened at pc1943.
//!
//! The deep CG branch is therefore not an ev33 destination. Its gates are also mutually
//! exclusive across the retail save set: the only save carrying the CE246 var157 == 13 gate
//! is SaveData07, and SaveData07 fails CE402's inner gate (var4 = 19 < 21, var102 = 59 <
//! 80); the saves that clear CE328's var104/var105 == 0 entry (01/02/03/06/07/08) do not
//! carry var157 == 13. No single real save satisfies
//! {var157 == 13 & var104 == 0 & var105 == 0 & var4 >= 21 & var102 >= 80}, which the last
//! test pins explicitly.
//!
//! Run:  WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_cg_run
//! Data: work/all; saves work/retail_backup/Save/SaveData01..08.sav.
//! Screenshot of the rendered cluster: work/out/m9_hscene_cg_run.png (SaveData08 H scene,
//! 89.4% non-black) - see docs/research/M9_HSCENE_CG_RUN.md.

use std::collections::BTreeSet;
use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_format::mps::{Map, MapEventPage, PageCondition};
use wolf_game::Game;

// ---------------------------------------------------------------------------------------
// Paths and data loading.
// ---------------------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn data_root() -> PathBuf {
    if let Ok(value) = std::env::var("WOLF_DATA_ROOT") {
        if !value.is_empty() {
            return repo_root().join(value);
        }
    }
    repo_root().join("work").join("all")
}

fn save_dir() -> PathBuf {
    repo_root().join("work").join("retail_backup").join("Save")
}

fn load_sample_map() -> Map {
    let bytes =
        std::fs::read(data_root().join("MapData").join("SampleMapA.mps")).expect("SampleMapA.mps");
    Map::parse(&bytes).expect("parse SampleMapA.mps")
}

fn common_events() -> CommonEventData {
    let bytes = std::fs::read(data_root().join("BasicData").join("CommonEvent.dat"))
        .expect("BasicData/CommonEvent.dat");
    CommonEventData::parse(&bytes).expect("parse CommonEvent.dat")
}

// ---------------------------------------------------------------------------------------
// Bytecode readers (mirror m8_hscene_cg_unlock.rs).
// ---------------------------------------------------------------------------------------

fn words(cmd: &wolf_format::command::Command) -> Vec<i32> {
    cmd.params
        .chunks_exact(4)
        .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect()
}

fn ce_call_targets(data: &CommonEventData, event: u32) -> Vec<i32> {
    let ev = data.event(event).unwrap_or_else(|| panic!("CE{event} exists"));
    let mut out = Vec::new();
    for slot in &ev.commands {
        if let Slot::Command(cmd) = slot {
            if cmd.opcode == 210 {
                if let Some(first) = words(cmd).into_iter().next() {
                    out.push(first);
                }
            }
        }
    }
    out
}

fn ce_cmd_pcs(data: &CommonEventData, event: u32, op: u32, first: i32) -> Vec<usize> {
    let ev = data.event(event).unwrap_or_else(|| panic!("CE{event} exists"));
    ev.commands
        .iter()
        .enumerate()
        .filter_map(|(pc, slot)| match slot {
            Slot::Command(cmd) if cmd.opcode == op => {
                (words(cmd).into_iter().next()? == first).then_some(pc)
            }
            _ => None,
        })
        .collect()
}

fn ce_opcode_count(data: &CommonEventData, event: u32, op: u32) -> usize {
    let ev = data.event(event).unwrap_or_else(|| panic!("CE{event} exists"));
    ev.commands
        .iter()
        .filter(|slot| matches!(slot, Slot::Command(cmd) if cmd.opcode == op))
        .count()
}

fn ce_cond_triples(data: &CommonEventData, event: u32) -> Vec<(usize, i32, i32, i32)> {
    let ev = data.event(event).unwrap_or_else(|| panic!("CE{event} exists"));
    let mut out = Vec::new();
    for (pc, slot) in ev.commands.iter().enumerate() {
        if let Slot::Command(cmd) = slot {
            if cmd.opcode == 111 {
                let ints = words(cmd);
                if let Some(&count) = ints.first() {
                    for k in 0..count.max(0) as usize {
                        let base = 1 + 3 * k;
                        if let (Some(var), Some(value), Some(op)) =
                            (ints.get(base), ints.get(base + 1), ints.get(base + 2))
                        {
                            out.push((pc, *var, *value, *op));
                        }
                    }
                }
            }
        }
    }
    out
}

fn map_event<'a>(map: &'a Map, id: u32) -> &'a wolf_format::mps::MapEvent {
    map.events
        .iter()
        .find(|event| event.id == id)
        .unwrap_or_else(|| panic!("map event {id} exists"))
}

fn map_cmd_firsts(page: &MapEventPage, op: u32) -> Vec<i32> {
    page.commands
        .iter()
        .filter_map(|slot| match slot {
            Slot::Command(cmd) if cmd.opcode == op => words(cmd).into_iter().next(),
            _ => None,
        })
        .collect()
}

fn enabled_conditions(page: &MapEventPage) -> Vec<PageCondition> {
    page.conditions()
        .into_iter()
        .filter(|condition| condition.enabled)
        .collect()
}

// ---------------------------------------------------------------------------------------
// Live game harness (mirrors m8_hscene_forward.rs / m8_hscene_cg_unlock.rs).
// ---------------------------------------------------------------------------------------

fn open_resumed_on(index: u32) -> Option<Game> {
    let save = save_dir().join(format!("SaveData{index:02}.sav"));
    if !save.exists() {
        eprintln!("skip: missing {}", save.display());
        return None;
    }
    let mut game = match Game::new(data_root()) {
        Ok(game) => game,
        Err(error) => {
            eprintln!("skip: Game::new failed: {error}");
            return None;
        }
    };
    if let Err(error) = game.load_save(&save) {
        eprintln!("skip: load_save({}) failed: {error}", save.display());
        return None;
    }
    let map = load_sample_map();
    game.start_engine_events();
    game.start_pages(&map, Some(0));
    Some(game)
}

fn drive(game: &mut Game, frames: usize) {
    for _ in 0..frames {
        game.frame();
    }
}

struct CgRun {
    peak_460: i32,
    saw_ev33: bool,
    files: BTreeSet<String>,
    warnings: u64,
}

fn run_ev33_cg_cluster(game: &mut Game, frames: usize) -> CgRun {
    game.vm.vars.set(364, 0);
    game.vm.vars.set(62, 1);
    game.vm.vars.set(111, 0);
    game.vm.vars.set(460, 0);
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.set_choice_cursor(0);
    drive(game, 4);

    game.vm.vars.set(61, 35);

    let mut peak_460 = 0;
    let mut saw_ev33 = false;
    let mut files: BTreeSet<String> = BTreeSet::new();
    for _ in 0..frames {
        game.frame();
        peak_460 = peak_460.max(game.vm.vars.get(460));
        if game.vm.instances().iter().any(|instance| instance.event == 33) {
            saw_ev33 = true;
        }
        for picture in game.screen.iter() {
            if !picture.file.is_empty() {
                files.insert(picture.file.clone());
            }
        }
    }
    CgRun { peak_460, saw_ev33, files, warnings: game.stats.warnings }
}

fn has_file_containing(files: &BTreeSet<String>, needle: &str) -> bool {
    files.iter().any(|file| file.contains(needle))
}

fn h_scene_file_count(files: &BTreeSet<String>) -> usize {
    files
        .iter()
        .filter(|file| {
            file.contains("hscene/") || file.contains("cg/") || file.contains("DLC_data/")
        })
        .count()
}

fn assert_cg_cluster_rendered(label: &str, run: &CgRun) {
    assert_eq!(run.peak_460, 1, "{label}: var460 reaches 1 (ev33 page condition)");
    assert!(run.saw_ev33, "{label}: map event 33 runs");
    assert_eq!(run.warnings, 0, "{label}: no engine warnings");
    assert!(
        has_file_containing(&run.files, "hscene/h_bg_001"),
        "{label}: CG background hscene/h_bg_001 drawn"
    );
    assert!(
        has_file_containing(&run.files, "hscene/h_frame"),
        "{label}: CG frame hscene/h_frame drawn"
    );
    assert!(
        has_file_containing(&run.files, "hscene/h_s"),
        "{label}: a CE333 pose sheet hscene/h_sNN drawn"
    );
    assert!(
        has_file_containing(&run.files, "DLC_data/pose_10"),
        "{label}: DLC pose_10 drawn"
    );
    assert!(
        h_scene_file_count(&run.files) >= 12,
        "{label}: at least 12 distinct H CG files drawn (got {})",
        h_scene_file_count(&run.files)
    );
}

// ---------------------------------------------------------------------------------------
// Test 1 - live: ev33 -> CE333 draws the H CG cluster on the drained mid-H save.
// ---------------------------------------------------------------------------------------

#[test]
fn ev33_renders_the_h_cg_cluster_on_the_mid_h_save() {
    let Some(mut game) = open_resumed_on(8) else { return };
    drive(&mut game, 60);
    assert_eq!(game.vm.vars.get(62), 1, "SaveData08 starts on H UI stage 1");

    let run = run_ev33_cg_cluster(&mut game, 160);
    assert_cg_cluster_rendered("SaveData08", &run);

    assert_eq!(game.vm.vars.get(137), 8, "SaveData08 stage selector var137 == 8");
    assert!(
        has_file_containing(&run.files, "hscene/h_s10"),
        "SaveData08 draws the h_s10 pose sheet for var137 == 8"
    );
    assert!(!game.vm.instances().is_empty(), "engine instances still running");
    assert_eq!(game.vm.vars.get(62), 1, "H UI still on stage 1 after the CG overlay");
}

// ---------------------------------------------------------------------------------------
// Test 2 - live: the same cluster renders on the two day saves the task names.
// ---------------------------------------------------------------------------------------

#[test]
fn ev33_renders_the_cg_cluster_on_savedata07_and_savedata04() {
    let mut ran = 0;
    for index in [7u32, 4u32] {
        let Some(mut game) = open_resumed_on(index) else { continue };
        drive(&mut game, 60);
        let run = run_ev33_cg_cluster(&mut game, 160);
        let label = format!("SaveData{index:02}");
        assert_cg_cluster_rendered(&label, &run);
        ran += 1;
    }
    assert_eq!(ran, 2, "both SaveData07 and SaveData04 exist and drive the cluster");
}

// ---------------------------------------------------------------------------------------
// Test 3 - static: ev33's only callee is CE333, and CE333 is a leaf (never calls onward).
// ---------------------------------------------------------------------------------------

#[test]
fn ev33_calls_only_ce333_which_is_a_leaf_cg_body() {
    let map = load_sample_map();
    let data = common_events();

    let ev33 = map_event(&map, 33);
    let conds = enabled_conditions(&ev33.pages[0]);
    assert_eq!(conds.len(), 1, "ev33 page0 has exactly one enabled condition");
    assert_eq!(conds[0].variable, 2_000_460, "ev33 gated on var460");
    assert_eq!(conds[0].operator, PageCondition::OP_EQUAL);
    assert_eq!(conds[0].value, 1);

    let ev33_targets: BTreeSet<i32> = map_cmd_firsts(&ev33.pages[0], 210).into_iter().collect();
    assert_eq!(
        ev33_targets,
        BTreeSet::from([500_333]),
        "ev33's only callee is 500333 (CE333)"
    );
    let ce333_calls: Vec<i32> = map_cmd_firsts(&ev33.pages[0], 210);
    assert!(
        ce333_calls.len() >= 15,
        "ev33 calls CE333 fifteen times (got {})",
        ce333_calls.len()
    );

    let ce333_ops210 = ce_call_targets(&data, 333);
    assert!(
        ce333_ops210.is_empty(),
        "CE333 (500333) is a leaf CG-drawing body: op210 targets {ce333_ops210:?}"
    );
    assert!(
        ce_opcode_count(&data, 333, 150) >= 300,
        "CE333 draws the CG with 300+ op150 picture commands"
    );

    assert!(
        ce_call_targets(&data, 246).contains(&500_333),
        "CE246 also calls CE333 (reverse direction: menu -> leaf)"
    );
    let ce246_ce333_pcs = ce_cmd_pcs(&data, 246, 210, 500_333);
    let ce246_gate_pc = ce_cond_triples(&data, 246)
        .into_iter()
        .find(|(_, var, value, op)| *var == 2_000_157 && *value == 13 && *op == 2)
        .map(|(pc, ..)| pc)
        .expect("CE246 gates its deep body on var157 == 13");
    assert!(
        ce246_ce333_pcs.iter().all(|pc| *pc > ce246_gate_pc),
        "CE246 reaches CE333 only after the var157 == 13 gate (gate pc{ce246_gate_pc}, calls {ce246_ce333_pcs:?})"
    );
    assert_eq!(
        ce246_ce333_pcs,
        vec![1983],
        "CE246 calls CE333 at pc1983 (inside the var157 == 13 block)"
    );

    for deep in [500_246, 500_328, 500_402] {
        assert!(
            !ev33_targets.contains(&deep),
            "ev33 never calls {deep} (CE{}): its only callee is CE333",
            deep - 500_000
        );
    }
}

// ---------------------------------------------------------------------------------------
// Test 4 - static + save table: the deep CG gates are not jointly satisfied by any save.
// ---------------------------------------------------------------------------------------

#[test]
fn deep_cg_gates_are_not_jointly_satisfied_by_any_real_save() {
    let data = common_events();

    assert!(
        ce_cond_triples(&data, 246)
            .iter()
            .any(|(pc, var, value, op)| *pc == 1943 && *var == 2_000_157 && *value == 13 && *op == 2),
        "CE246 pc1943 gates its deep body on var157 == 13"
    );
    assert!(
        ce_cond_triples(&data, 328)
            .iter()
            .any(|(pc, var, value, op)| *pc == 4 && *var == 2_000_105 && *value == 0 && *op == 2),
        "CE328 pc4 gates entry on var105 == 0"
    );
    assert!(
        ce_cond_triples(&data, 328)
            .iter()
            .any(|(pc, var, value, op)| *pc == 6 && *var == 2_000_104 && *value == 0 && *op == 2),
        "CE328 pc6 gates entry on var104 == 0"
    );
    assert!(
        ce_cond_triples(&data, 402)
            .iter()
            .any(|(_, var, value, op)| *var == 2_000_004 && *value == 21 && *op == 1),
        "CE402 gates its body on var4 >= 21"
    );
    assert!(
        ce_cond_triples(&data, 402)
            .iter()
            .any(|(_, var, value, op)| *var == 2_000_102 && *value == 80 && *op == 1),
        "CE402 gates its body on var102 >= 80"
    );
    assert!(
        ce_cond_triples(&data, 402)
            .iter()
            .any(|(_, var, value, op)| *var == 2_000_016 && *value == 1 && *op == 1),
        "CE402 gates its body on var16 >= 1"
    );

    let mut ce246_saves = Vec::new();
    let mut ce328_entry_saves = Vec::new();
    let mut rows = Vec::new();
    for index in 1..=8u32 {
        let save = save_dir().join(format!("SaveData{index:02}.sav"));
        if !save.exists() {
            continue;
        }
        let Ok(mut game) = Game::new(data_root()) else { continue };
        if game.load_save(&save).is_err() {
            continue;
        }
        let get = |n| game.vm.vars.get(n);
        let v157 = get(157);
        let v104 = get(104);
        let v105 = get(105);
        let v4 = get(4);
        let v102 = get(102);
        let v16 = get(16);
        let v22 = get(22);
        let ce246_ok = v157 == 13;
        let ce328_entry_ok = v104 == 0 && v105 == 0;
        let ce402_internal_ok = v16 >= 1 && v22 >= 180 && v4 >= 21 && v102 >= 80;
        if ce246_ok {
            ce246_saves.push(index);
        }
        if ce328_entry_ok {
            ce328_entry_saves.push(index);
        }
        rows.push((index, v157, v104, v105, v4, v102, v16, v22, ce246_ok, ce328_entry_ok, ce402_internal_ok));
    }
    assert!(rows.len() >= 7, "read {} retail saves", rows.len());

    assert_eq!(ce246_saves, vec![7], "only SaveData07 has var157 == 13 (CE246 deep gate)");
    assert!(ce328_entry_saves.contains(&7), "SaveData07 clears the CE328 entry gate");
    assert!(!ce328_entry_saves.contains(&4), "SaveData04 fails var104/var105 == 0 entry");
    assert!(!ce328_entry_saves.contains(&5), "SaveData05 fails var104/var105 == 0 entry");

    let save07 = rows.iter().find(|row| row.0 == 7).expect("SaveData07 row");
    assert!(save07.8, "SaveData07 has var157 == 13");
    assert!(save07.9, "SaveData07 clears the CE328 entry gate");
    assert!(!save07.10, "SaveData07 FAILS CE402 inner gate");
    assert_eq!(save07.4, 19, "SaveData07 var4 = 19 < 21 (CE402 rejects)");
    assert_eq!(save07.5, 59, "SaveData07 var102 = 59 < 80 (CE402 rejects)");

    let joint: Vec<u32> = rows
        .iter()
        .filter(|row| row.8 && row.9 && row.10)
        .map(|row| row.0)
        .collect();
    assert!(
        joint.is_empty(),
        "no single real save satisfies the full deep CG gate set (found {joint:?}); rows = {rows:?}"
    );

    let save04 = rows.iter().find(|row| row.0 == 4).expect("SaveData04 row");
    assert_eq!(save04.2, 1, "SaveData04 var104 = 1 (fails CE328 entry)");
    assert_eq!(save04.3, 2, "SaveData04 var105 = 2 (fails CE328 entry)");
    assert!(save04.4 >= 21, "SaveData04 var4 >= 21 (only CE402 inner gate it satisfies)");
}
