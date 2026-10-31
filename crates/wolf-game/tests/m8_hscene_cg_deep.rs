//! M10 W2 - a TEST-ONLY constructed save that satisfies the full deep H CG gate set, and
//! proof of how far ev33 -> CE333 -> CE246 / CE328 / CE402 advances from it.
//!
//! Context (M9_HSCENE_CG_RUN.md, tests/m8_hscene_cg_run.rs): the documented deep H CG gate is
//! the conjunction {var157==13 & var104==0 & var105==0 & var4>=21 & var102>=80}. No single
//! retail save satisfies it, so the deep branch was never driven end-to-end. This file builds a
//! synthetic save (from SaveData07, mutated ONLY in memory then written to work/out) that opens
//! every gate simultaneously, and measures the real bytecode's advance from it.
//!
//! Findings pinned here (all empirical, live VM):
//! 1. The synthetic save round-trips exactly (vars 0..800 persist through Game::save_to/load_save).
//! 2. CE402 started on the synthetic save runs its DEEP body: max pc >= 300 of 512, the
//!    騎乗位系カットイン child CE410 is entered, 14 h_s08 pose/effect files + h_bg_001/h_bg_011/
//!    h_frame are drawn, 0 warnings - deterministic across repeats.
//! 3. CE402 on the plain SaveData07 (var4=19, var102=59 fail the gate) aborts immediately:
//!    no CE410, no h_s08, max pc tiny - the gate set is NECESSARY.
//! 4. ev33 -> CE333 never reaches the deep branch even with every gate open: CE333 is a leaf
//!    CG-drawing body, so var460/ev33 renders the H CG cluster but does NOT enter CE246/328/402.
//! 5. CE246 started stateless stalls at pc116 on its pc117 var151>=1 early gate - the pc1943
//!    var157==13 block (the only place CE246 calls CE333 / CE346) is unreachable that way.
//! 6. CE328 started stateless on the synthetic save completes its OWN body (max pc > 1000 of
//!    1025) but does NOT take its pc986 騎乗位/夢精 sub-branch, so CE402 stays uninstantiated.
//!
//! Run:   WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_cg_deep
//! Data:  work/all; saves work/retail_backup/Save/SaveData01..08.sav.
//! Exit:  the only persistence written is work/out/m10_deep_synth.sav (gitignored); no retail
//!        save is mutated.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_format::mps::{Map, MapEventPage};
use wolf_game::Game;

// ---------------------------------------------------------------------------------------
// Paths and data loading (mirror m8_hscene_cg_run.rs).
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

fn synth_path() -> PathBuf {
    repo_root().join("work").join("out").join("m10_deep_synth.sav")
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
// Bytecode readers (mirror m8_hscene_cg_run.rs / m8_hscene_cg_unlock.rs).
// ---------------------------------------------------------------------------------------

fn words(cmd: &wolf_format::command::Command) -> Vec<i32> {
    cmd.params
        .chunks_exact(4)
        .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect()
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

// ---------------------------------------------------------------------------------------
// Synthetic save construction (TEST-ONLY; mutations live in memory + work/out only).
// ---------------------------------------------------------------------------------------

/// The documented deep gate values the synthetic save sets, in (var index, value) order.
const DEEP_GATE: [(usize, i32); 9] = [
    (157, 13), // CE246 deep-body gate
    (104, 0),  // CE328 entry gate
    (105, 0),  // CE328 entry gate
    (4, 21),   // CE402 inner gate (age/day)
    (102, 80), // CE402 inner gate
    (16, 1),   // CE402 inner gate
    (22, 180), // CE402 inner gate
    (326, 2),  // CE402 inner gate
    (364, 0),  // dispatcher gate (documented normalization)
];

fn build_synth_save() -> PathBuf {
    let path = synth_path();
    let source = save_dir().join("SaveData07.sav");
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&source)
        .unwrap_or_else(|error| panic!("load_save({}): {error}", source.display()));
    for (index, value) in DEEP_GATE {
        game.vm.vars.set(index, value);
    }
    let bytes = game.save_to(&path).expect("save_to synthetic save");
    assert!(bytes > 0, "synthetic save wrote {bytes} bytes");
    assert!(path.exists(), "synthetic save exists at {}", path.display());
    path
}

fn open_on(path: &PathBuf) -> Option<Game> {
    if !path.exists() {
        eprintln!("skip: missing {}", path.display());
        return None;
    }
    let mut game = match Game::new(data_root()) {
        Ok(game) => game,
        Err(error) => {
            eprintln!("skip: Game::new failed: {error}");
            return None;
        }
    };
    if let Err(error) = game.load_save(path) {
        eprintln!("skip: load_save({}) failed: {error}", path.display());
        return None;
    }
    let map = load_sample_map();
    game.start_engine_events();
    game.start_pages(&map, Some(0));
    Some(game)
}

fn open_synth() -> Option<Game> {
    open_on(&build_synth_save())
}

/// Load the synthetic save WITHOUT starting the engine's background common events or map
/// pages. The deep H common events are driven directly, so no background coroutine can
/// clobber the gate variables mid-frame - this isolates "does CE402's own body run when the
/// gate holds", which is what the deep-save proof is about.
fn open_synth_bare() -> Option<Game> {
    let path = build_synth_save();
    if !path.exists() {
        return None;
    }
    let mut game = Game::new(data_root()).ok()?;
    game.load_save(&path).ok()?;
    Some(game)
}

fn open_bare(path: &PathBuf) -> Option<Game> {
    let mut game = Game::new(data_root()).ok()?;
    game.load_save(path).ok()?;
    Some(game)
}

fn drive(game: &mut Game, frames: usize) {
    for _ in 0..frames {
        game.frame();
    }
}

// ---------------------------------------------------------------------------------------
// Deep-run measurement: max pc reached per common event + which events ran + files drawn.
// ---------------------------------------------------------------------------------------

struct DeepRun {
    max_pc: BTreeMap<u32, usize>,
    seen: BTreeSet<u32>,
    files: BTreeSet<String>,
    warnings: u64,
}

impl DeepRun {
    fn max_pc_of(&self, event: u32) -> usize {
        self.max_pc.get(&event).copied().unwrap_or(0)
    }
    fn h_s08_files(&self) -> usize {
        self.files.iter().filter(|file| file.contains("h_s08")).count()
    }
}

fn run_started_event(game: &mut Game, event: u32, frames: usize) -> DeepRun {
    assert!(game.start_event(event), "start_event({event}) spawns a coroutine");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.set_choice_cursor(0);

    let mut max_pc: BTreeMap<u32, usize> = BTreeMap::new();
    let mut seen: BTreeSet<u32> = BTreeSet::new();
    let mut files: BTreeSet<String> = BTreeSet::new();
    for _ in 0..frames {
        game.frame();
        for instance in game.vm.instances() {
            seen.insert(instance.event);
            let slot = max_pc.entry(instance.event).or_insert(0);
            if instance.pc > *slot {
                *slot = instance.pc;
            }
        }
        for picture in game.screen.iter() {
            if !picture.file.is_empty() {
                files.insert(picture.file.clone());
            }
        }
    }
    DeepRun { max_pc, seen, files, warnings: game.stats.warnings }
}

// ---------------------------------------------------------------------------------------
// ev33 / CE333 cluster runner (mirror m8_hscene_cg_run.rs run_ev33_cg_cluster).
// ---------------------------------------------------------------------------------------

struct CgRun {
    peak_460: i32,
    saw_ev33: bool,
    files: BTreeSet<String>,
    warnings: u64,
}

fn run_ev33_cluster(game: &mut Game, frames: usize) -> CgRun {
    game.vm.vars.set(364, 0);
    game.vm.vars.set(62, 1);
    game.vm.vars.set(111, 0);
    game.vm.vars.set(460, 0);
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.set_choice_cursor(0);
    drive(game, 4);

    game.vm.vars.set(61, 35); // real EV11 dispatcher output for hovering picture 444

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

// ---------------------------------------------------------------------------------------
// Test 1 - the synthetic save opens every gate and round-trips exactly; the static graph
//          documents the same gate set.
// ---------------------------------------------------------------------------------------

#[test]
fn synthetic_save_opens_the_full_deep_gate_and_round_trips() {
    let path = build_synth_save();

    // Round-trip through the on-disk save (not the in-memory build).
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&path).expect("reload synthetic save");
    let get = |n| game.vm.vars.get(n);
    assert_eq!(get(157), 13, "synthetic var157 == 13 (CE246 deep gate)");
    assert_eq!(get(104), 0, "synthetic var104 == 0 (CE328 entry gate)");
    assert_eq!(get(105), 0, "synthetic var105 == 0 (CE328 entry gate)");
    assert_eq!(get(4), 21, "synthetic var4 == 21 (CE402 inner gate)");
    assert_eq!(get(102), 80, "synthetic var102 == 80 (CE402 inner gate)");
    assert_eq!(get(16), 1, "synthetic var16 >= 1 (CE402 inner gate)");
    assert_eq!(get(22), 180, "synthetic var22 >= 180 (CE402 inner gate)");
    assert_eq!(get(326), 2, "synthetic var326 >= 2 (CE402 inner gate)");
    assert_eq!(get(364), 0, "synthetic var364 == 0 (dispatcher gate)");

    // The gate really is a gate: plain SaveData07 (the synth's source) fails it.
    let mut retail = Game::new(data_root()).expect("Game::new");
    retail.load_save(&save_dir().join("SaveData07.sav")).expect("SaveData07");
    assert_eq!(retail.vm.vars.get(4), 19, "SaveData07 var4 = 19 < 21");
    assert_eq!(retail.vm.vars.get(102), 59, "SaveData07 var102 = 59 < 80");

    // Static: CE402's deep-body gates.
    let data = common_events();
    for (var, value, op) in [
        (2_000_016, 1, 1),
        (2_000_022, 180, 1),
        (2_000_004, 21, 1),
        (2_000_326, 2, 1),
        (2_000_102, 80, 1),
    ] {
        assert!(
            ce_cond_triples(&data, 402)
                .iter()
                .any(|(_, v, val, o)| *v == var && *val == value && *o == op),
            "CE402 gates its body on var{} {} (op{op})",
            var - 2_000_000,
            value
        );
    }
    // Static: CE402's 騎乗位系カットイン child CE410 call sites.
    let cutin_pcs = ce_cmd_pcs(&data, 402, 210, 500_410);
    assert!(
        cutin_pcs.len() >= 10,
        "CE402 calls CE410 (騎乗位系カットイン) at {cutin_pcs:?}"
    );
    // Static: CE328 enters only when var105/var104 are 0, and calls CE402 at pc986.
    assert!(ce_cond_triples(&data, 328)
        .iter()
        .any(|(pc, v, val, o)| *pc == 4 && *v == 2_000_105 && *val == 0 && *o == 2));
    assert!(ce_cond_triples(&data, 328)
        .iter()
        .any(|(pc, v, val, o)| *pc == 6 && *v == 2_000_104 && *val == 0 && *o == 2));
    assert_eq!(ce_cmd_pcs(&data, 328, 210, 500_402), vec![986], "CE328 calls CE402 at pc986");

    // Static: ev33's only callee is CE333 (a leaf), so the map path never enters the deep branch.
    let map = load_sample_map();
    let ev33 = map_event(&map, 33);
    let targets: BTreeSet<i32> = map_cmd_firsts(&ev33.pages[0], 210).into_iter().collect();
    assert_eq!(targets, BTreeSet::from([500_333]), "ev33's only callee is CE333");
}

// ---------------------------------------------------------------------------------------
// Test 2 - CE402 on the synthetic save runs its deep cutin body (deterministic).
// ---------------------------------------------------------------------------------------

#[test]
fn ce402_runs_its_deep_cutin_body_on_the_synthetic_save() {
    let mut profiles = Vec::new();
    for _ in 0..2 {
        let Some(mut game) = open_synth_bare() else { panic!("synthetic save must load") };
        let run = run_started_event(&mut game, 402, 1500);
        profiles.push(run);
    }

    for (n, run) in profiles.iter().enumerate() {
        let label = format!("CE402 synth run#{}", n + 1);
        assert_eq!(run.warnings, 0, "{label}: no engine warnings");
        assert!(run.seen.contains(&402), "{label}: CE402 ran");
        assert!(
            run.max_pc_of(402) >= 300,
            "{label}: CE402 reaches deep pc (got {} of 512)",
            run.max_pc_of(402)
        );
        assert!(run.seen.contains(&410), "{label}: CE410 cutin child entered");
        assert!(run.max_pc_of(410) >= 1, "{label}: CE410 advanced");
        assert!(
            run.h_s08_files() >= 10,
            "{label}: >= 10 h_s08 pose/effect files drawn (got {})",
            run.h_s08_files()
        );
        assert!(
            run.files.iter().any(|file| file.contains("hscene/h_bg_001")),
            "{label}: H CG background h_bg_001 drawn"
        );
        assert!(
            run.files.iter().any(|file| file.contains("hscene/h_frame")),
            "{label}: H CG frame h_frame drawn"
        );
    }

    // Determinism: same files and same CE402 max pc across the two runs.
    assert_eq!(profiles[0].files, profiles[1].files, "file set deterministic across runs");
    assert_eq!(
        profiles[0].max_pc_of(402),
        profiles[1].max_pc_of(402),
        "CE402 max pc deterministic across runs"
    );
}

// ---------------------------------------------------------------------------------------
// Test 3 - the same CE402 start on plain SaveData07 aborts: the gates are NECESSARY.
// ---------------------------------------------------------------------------------------

#[test]
fn ce402_gates_are_necessary_plain_savedata07_aborts() {
    let Some(mut game) = open_bare(&save_dir().join("SaveData07.sav")) else {
        panic!("SaveData07 must load")
    };
    let run = run_started_event(&mut game, 402, 400);

    assert_eq!(run.warnings, 0, "no warnings on the aborted run");
    assert!(
        run.max_pc_of(402) < 20,
        "CE402 aborts at its first gates on SaveData07 (max pc {})",
        run.max_pc_of(402)
    );
    assert!(!run.seen.contains(&410), "CE410 cutin never entered");
    assert_eq!(run.h_s08_files(), 0, "no h_s08 files drawn");
}

// ---------------------------------------------------------------------------------------
// Test 4 - ev33 -> CE333 renders the CG cluster but never enters the deep branch, even with
//          every gate open.
// ---------------------------------------------------------------------------------------

#[test]
fn ev33_and_ce333_never_reach_the_deep_branch_even_when_gates_are_open() {
    let Some(mut game) = open_synth() else { panic!("synthetic save must load") };
    drive(&mut game, 4);
    let run = run_ev33_cluster(&mut game, 400);

    assert_eq!(run.peak_460, 1, "ev11 writes var460 = 1 so ev33 activates");
    assert!(run.saw_ev33, "map event 33 runs");
    assert_eq!(run.warnings, 0, "no warnings");
    assert!(
        run.files.iter().filter(|f| f.contains("hscene/") || f.contains("cg/")).count() >= 12,
        "CE333 still draws the H CG cluster"
    );

    // But the deep branch is untouched: the leaf never instantiates CE246 / CE328 / CE402.
    for deep in [246u32, 328, 402] {
        assert!(
            !game.vm.instances().iter().any(|instance| instance.event == deep),
            "CE{deep} not running after ev33 -> CE333"
        );
    }
    assert!(!run.files.iter().any(|file| file.contains("h_s08")), "no deep h_s08 drawn");
}

// ---------------------------------------------------------------------------------------
// Test 5 - CE246 started stateless cannot reach its var157 == 13 deep block (stalls pc117).
// ---------------------------------------------------------------------------------------

#[test]
fn ce246_stateless_start_cannot_reach_its_deep_block() {
    let Some(mut game) = open_synth_bare() else { panic!("synthetic save must load") };
    let run = run_started_event(&mut game, 246, 1500);

    assert_eq!(run.warnings, 0, "no warnings");
    assert!(run.seen.contains(&246), "CE246 ran");
    assert!(
        run.max_pc_of(246) < 1943,
        "CE246 never reaches its pc1943 var157 deep block (max pc {})",
        run.max_pc_of(246)
    );
    assert!(
        run.max_pc_of(246) <= 120,
        "CE246 stalls at its pc117 var151>=1 early gate (max pc {})",
        run.max_pc_of(246)
    );
    assert!(!run.seen.contains(&333), "CE246's deep CE333 call is not reached");
    assert!(!run.seen.contains(&346), "CE246's deep CE346 cutin call is not reached");
}

// ---------------------------------------------------------------------------------------
// Test 6 - CE328 on the synthetic save completes its own body but skips the CE402 sub-branch.
// ---------------------------------------------------------------------------------------

#[test]
fn ce328_runs_its_body_but_skips_the_ce402_sub_branch() {
    let Some(mut game) = open_synth_bare() else { panic!("synthetic save must load") };
    let run = run_started_event(&mut game, 328, 3000);

    assert_eq!(run.warnings, 0, "no warnings");
    assert!(run.seen.contains(&328), "CE328 ran");
    assert!(
        run.max_pc_of(328) >= 1000,
        "CE328 advances through its own body (max pc {} of 1025)",
        run.max_pc_of(328)
    );
    // The pc986 騎乗位/夢精 sub-branch is the only CE402 entry from CE328; it is not taken here.
    assert!(!run.seen.contains(&402), "CE402 never instantiated from CE328's own run");
    assert_eq!(run.h_s08_files(), 0, "no CE402 h_s08 files drawn from the CE328 path");
}
