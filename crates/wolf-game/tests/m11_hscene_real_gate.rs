//! M11 W2 - H deep CG real-save gate matrix (CE328 -> CE402 -> CE410).
//!
//! Context: earlier milestones proved (a) the deep H CG body CE402 / CE410 is gated by the
//! conjunction {var157==13 & var104==0 & var105==0 & var4>=21 & var102>=80 & var16>=1 &
//! var22>=180 & var326>=2 & var364==0} (m8_hscene_cg_deep.rs: a synthetic save with those
//! values drives CE402 to pc509 and enters CE410, 0 warnings), and (b) no single retail
//! save holds that conjunction. This file makes the negative result explicit and auditable:
//! a per-save gate matrix over SaveData01..08, the writer trace for every gate variable, and
//! the empirical live-VM behaviour when CE402 / CE328 are started on each retail save.
//!
//! HONEST VERDICT pinned here: NO shipped retail save - and therefore no real player operation
//! that only reads such a save - can reach CE402 / CE410. Two gate variables are 0 in ALL eight
//! saves:
//!   * var304 == 0 in every save  -> CE328 (the UNIQUE caller of CE402, pc986) never enters.
//!   * var326 == 0 in every save  -> CE402's own body gate (pc6 var326>=2) never passes.
//!
//! Every one of these variables DOES have a normal gameplay writer (traced below, so this is
//! NOT a data bug), but no snapshot captures the required state. Only a synthetic,
//! engine-injected save opens it - which is why the deep branch is reproducible only in
//! m8_hscene_cg_deep.rs, never from the day/night loop.
//!
//! Run:   WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_hscene_real_gate
//! Data:  work/all; saves work/retail_backup/Save/SaveData01..08.sav.
//! Exit:  reads retail saves only; the sole write is work/out/m11_real_gate_synth.sav (gitignored).

use std::collections::BTreeSet;
use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_format::mps::{Map, MapEventPage};
use wolf_game::Game;

// ---------------------------------------------------------------------------------------
// Paths and data loading (mirror m8_hscene_cg_deep.rs).
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
    repo_root()
        .join("work")
        .join("out")
        .join("m11_real_gate_synth.sav")
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
// Bytecode readers.
// ---------------------------------------------------------------------------------------

fn words(cmd: &wolf_format::command::Command) -> Vec<i32> {
    cmd.params
        .chunks_exact(4)
        .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect()
}

// Every op210 call site (target event id) anywhere in CommonEvent.dat.
fn call_sites(data: &CommonEventData, target: i32) -> Vec<(u32, usize)> {
    let mut out = Vec::new();
    for event in &data.events {
        for (pc, slot) in event.commands.iter().enumerate() {
            if let Slot::Command(cmd) = slot {
                if cmd.opcode == 210 && words(cmd).first().copied() == Some(target) {
                    out.push((event.id, pc));
                }
            }
        }
    }
    out
}

// Every op121 write site (first word == target variable address) in a common event.
fn write_sites(data: &CommonEventData, event: u32, target: i32) -> Vec<(usize, Vec<i32>)> {
    let ev = data
        .event(event)
        .unwrap_or_else(|| panic!("CE{event} exists"));
    let mut out = Vec::new();
    for (pc, slot) in ev.commands.iter().enumerate() {
        if let Slot::Command(cmd) = slot {
            if cmd.opcode == 121 {
                let ints = words(cmd);
                if ints.first().copied() == Some(target) {
                    out.push((pc, ints[1..].to_vec()));
                }
            }
        }
    }
    out
}

// Every op121 write of target across ALL common events, as "CE{id} pc{pc}".
fn ce_var_writers(data: &CommonEventData, target: i32) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for event in &data.events {
        for (pc, _) in write_sites(data, event.id, target) {
            out.insert(format!("CE{} pc{}", event.id, pc));
        }
    }
    out
}

// Every op121 write of target across ALL map pages, as "MAP EV{id} p{page} pc{pc}".
fn map_var_writers(map: &Map, target: i32) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for event in &map.events {
        for (page, ev_page) in event.pages.iter().enumerate() {
            for (pc, slot) in ev_page.commands.iter().enumerate() {
                if let Slot::Command(cmd) = slot {
                    if cmd.opcode == 121 && words(cmd).first().copied() == Some(target) {
                        out.insert(format!("MAP EV{} p{} pc{}", event.id, page, pc));
                    }
                }
            }
        }
    }
    out
}

fn ce_cond_triples(data: &CommonEventData, event: u32) -> Vec<(usize, i32, i32, i32)> {
    let ev = data
        .event(event)
        .unwrap_or_else(|| panic!("CE{event} exists"));
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

fn map_event(map: &Map, id: u32) -> &wolf_format::mps::MapEvent {
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
// op111 comparison semantics (exec.rs compare(); op1 = >=, op2 = ==, ...).
// ---------------------------------------------------------------------------------------

fn compare(lhs: i32, rhs: i32, operator: i32) -> bool {
    match operator {
        0 => lhs > rhs,
        1 => lhs >= rhs,
        2 => lhs == rhs,
        3 => lhs <= rhs,
        4 => lhs < rhs,
        5 => lhs != rhs,
        6 => (lhs & rhs) != 0,
        _ => lhs == rhs,
    }
}

// ---------------------------------------------------------------------------------------
// The documented deep gate set (identical to m8_hscene_cg_deep.rs DEEP_GATE) as predicates.
// (var index, operator, value).
// ---------------------------------------------------------------------------------------

// CE402 body + CE328-entry + dispatcher conjunction, i.e. the necessary-and-sufficient gate
// set proven by m8_hscene_cg_deep.rs (a save holding all nine drives CE402 -> CE410).
const DEEP_GATE: [(usize, i32, i32); 9] = [
    (157, 2, 13), // CE246 deep-body chapter gate (op2 ==)
    (104, 2, 0),  // CE328 entry gate (op2 ==)
    (105, 2, 0),  // CE328 entry gate (op2 ==)
    (4, 1, 21),   // CE402 inner gate (op1 >=)
    (102, 1, 80), // CE402 inner gate (op1 >=)
    (16, 1, 1),   // CE402 inner gate (op1 >=)
    (22, 1, 180), // CE402 inner gate (op1 >=)
    (326, 1, 2),  // CE402 inner gate (op1 >=) - learned 騎乗位 skill
    (364, 2, 0),  // dispatcher gate (documented normalization, op2 ==)
];

// CE328 entry conjunction (pc4/6/15/20): var105==0 & var104==0 & var304>=1 & var4 in {18,19,20}.
fn ce328_entry_ok(v: &SaveVals) -> bool {
    v.v105 == 0 && v.v104 == 0 && v.v304 >= 1 && (18..=20).contains(&v.v4)
}

// ---------------------------------------------------------------------------------------
// Per-save variable snapshot.
// ---------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct SaveVals {
    v4: i32,
    v16: i32,
    v22: i32,
    v102: i32,
    v104: i32,
    v105: i32,
    v157: i32,
    v304: i32,
    v326: i32,
    v364: i32,
}

fn load_vals(n: u32) -> Option<SaveVals> {
    let path = save_dir().join(format!("SaveData{n:02}.sav"));
    let mut game = Game::new(data_root()).ok()?;
    game.load_save(&path).ok()?;
    let g = |i: usize| game.vm.vars.get(i);
    Some(SaveVals {
        v4: g(4),
        v16: g(16),
        v22: g(22),
        v102: g(102),
        v104: g(104),
        v105: g(105),
        v157: g(157),
        v304: g(304),
        v326: g(326),
        v364: g(364),
    })
}

fn body_passes(v: &SaveVals) -> Vec<bool> {
    DEEP_GATE
        .iter()
        .map(|&(var, op, val)| {
            let lhs = match var {
                157 => v.v157,
                104 => v.v104,
                105 => v.v105,
                4 => v.v4,
                102 => v.v102,
                16 => v.v16,
                22 => v.v22,
                326 => v.v326,
                364 => v.v364,
                _ => unreachable!(),
            };
            compare(lhs, val, op)
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// Live-VM driving helpers.
// ---------------------------------------------------------------------------------------

fn open_bare(n: u32) -> Option<Game> {
    let path = save_dir().join(format!("SaveData{n:02}.sav"));
    let mut game = Game::new(data_root()).ok()?;
    game.load_save(&path).ok()?;
    Some(game)
}

fn drive_started(game: &mut Game, event: u32, frames: usize) -> (usize, bool, u64) {
    assert!(
        game.start_event(event),
        "start_event({event}) spawns a coroutine"
    );
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.set_choice_cursor(0);
    let mut max_pc = 0usize;
    let mut saw_410 = false;
    for _ in 0..frames {
        game.frame();
        for instance in game.vm.instances() {
            if instance.event == event && instance.pc > max_pc {
                max_pc = instance.pc;
            }
            if instance.event == 410 {
                saw_410 = true;
            }
        }
    }
    (max_pc, saw_410, game.stats.warnings)
}

fn build_synth_save() -> PathBuf {
    let path = synth_path();
    let source = save_dir().join("SaveData07.sav");
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&source)
        .unwrap_or_else(|error| panic!("load_save({}): {error}", source.display()));
    for (index, _op, value) in DEEP_GATE {
        game.vm.vars.set(index, value);
    }
    std::fs::create_dir_all(path.parent().unwrap()).ok();
    let bytes = game.save_to(&path).expect("save_to synthetic save");
    assert!(
        bytes > 0 && path.exists(),
        "synthetic save written to {}",
        path.display()
    );
    path
}

// =======================================================================================
// Test 1 - static graph: the gate set, the single CE402 entry, and every gate-var writer.
// =======================================================================================

#[test]
fn gate_graph_and_writers_are_as_documented() {
    let data = common_events();

    // (1) CE402's own body gates (all op1 >=).
    for (var, value) in [
        (2_000_016, 1),
        (2_000_022, 180),
        (2_000_004, 21),
        (2_000_326, 2),
        (2_000_102, 80),
    ] {
        assert!(
            ce_cond_triples(&data, 402)
                .iter()
                .any(|(_, v, val, o)| *v == var && *val == value && *o == 1),
            "CE402 body gates var{} >= {value}",
            var - 2_000_000
        );
    }

    // (2) CE402 -> CE410 (騎乗位系カットイン) at >= 10 sites, all inside CE402.
    let cutin = call_sites(&data, 500_410);
    assert!(
        cutin.iter().all(|(ev, _)| *ev == 402) && cutin.len() >= 10,
        "CE402 calls CE410 at {} sites, all inside CE402",
        cutin.len()
    );

    // (3) CE328 is the UNIQUE caller of CE402, at pc986.
    assert_eq!(
        call_sites(&data, 500_402),
        vec![(328, 986)],
        "CE402 has exactly one caller: CE328 pc986"
    );

    // (4) CE328 entry conjunction (pc4/6/15/20).
    let e328 = ce_cond_triples(&data, 328);
    assert!(
        e328.iter()
            .any(|(pc, v, val, o)| *pc == 4 && *v == 2_000_105 && *val == 0 && *o == 2),
        "CE328 entry var105 == 0 at pc4"
    );
    assert!(
        e328.iter()
            .any(|(pc, v, val, o)| *pc == 6 && *v == 2_000_104 && *val == 0 && *o == 2),
        "CE328 entry var104 == 0 at pc6"
    );
    assert!(
        e328.iter()
            .any(|(pc, v, val, o)| *pc == 15 && *v == 2_000_304 && *val == 1 && *o == 1),
        "CE328 entry var304 >= 1 at pc15"
    );
    assert!(
        e328.iter()
            .filter(|(pc, v, _, _)| *pc == 20 && *v == 2_000_004)
            .count()
            == 3,
        "CE328 entry var4 in {{18,19,20}} at pc20 (three op2 arms)"
    );

    // (5) var326 (learned 騎乗位) is written ONLY by CE301, at pc6320 / pc6365, always to 2.
    assert_eq!(
        ce_var_writers(&data, 2_000_326),
        BTreeSet::from(["CE301 pc6320".to_string(), "CE301 pc6365".to_string()]),
        "var326 has exactly two writers, both in CE301"
    );
    let w326 = write_sites(&data, 301, 2_000_326);
    assert!(
        w326.iter().all(|(_, ints)| ints == &vec![2, 0, 0]),
        "CE301 sets var326 = 2"
    );
    assert!(
        ce_cond_triples(&data, 301)
            .iter()
            .any(|(_, v, val, o)| *v == 2_000_326 && *val == 0 && *o == 2),
        "CE301 guards its 騎乗位 grant on var326 == 0"
    );
    assert!(
        map_var_writers(&load_sample_map(), 2_000_326).is_empty(),
        "no map writes var326"
    );

    // (6) var304 (intimate-route flag) is set to 1 by the two night-menu routes only.
    assert!(
        write_sites(&data, 245, 2_000_304)
            .iter()
            .any(|(pc, ints)| *pc == 779 && ints == &vec![1, 0, 0]),
        "CE245 pc779 sets var304 = 1 (intimate route)"
    );
    assert!(
        write_sites(&data, 290, 2_000_304)
            .iter()
            .any(|(pc, ints)| *pc == 403 && ints == &vec![1, 0, 0]),
        "CE290 pc403 sets var304 = 1 (intimate route)"
    );
    assert!(
        write_sites(&data, 332, 2_000_304)
            .iter()
            .any(|(pc, ints)| *pc == 125 && ints == &vec![0, 0, 0]),
        "CE332 pc125 clears var304 = 0 (night reset)"
    );

    // (7) ev33 still only ever calls the CE333 leaf (so the map path cannot reach the deep node).
    let map = load_sample_map();
    let ev33_targets: BTreeSet<i32> = map_cmd_firsts(&map_event(&map, 33).pages[0], 210)
        .into_iter()
        .collect();
    assert_eq!(
        ev33_targets,
        BTreeSet::from([500_333]),
        "ev33's only callee is CE333"
    );
}

// =======================================================================================
// Test 2 - per-save gate matrix over SaveData01..08.
// =======================================================================================

#[test]
fn gate_matrix_over_the_eight_retail_saves_is_all_fail() {
    // Raw matrix (v4,v16,v22,v102,v104,v105,v157,v304,v326,v364) - pins the inputs.
    type GateRow = (i32, i32, i32, i32, i32, i32, i32, i32, i32, i32);
    let expected: [GateRow; 8] = [
        (21, 4, 347, 120, 0, 0, 12, 0, 0, 0), // SaveData01
        (18, 4, 0, 12, 0, 0, 1, 0, 0, 0),     // SaveData02
        (22, 4, 102, 52, 0, 0, 10, 0, 0, 0),  // SaveData03
        (21, 3, 122, 90, 1, 2, 12, 0, 0, 0),  // SaveData04
        (18, 3, 121, 59, 1, 3, 12, 0, 0, 0),  // SaveData05
        (18, 2, 157, 33, 0, 0, 8, 0, 0, 0),   // SaveData06
        (19, 2, 213, 59, 0, 0, 13, 0, 0, 0),  // SaveData07
        (0, 2, 375, 120, 0, 0, 12, 0, 0, 0),  // SaveData08
    ];

    // Body predicate pass counts (out of DEEP_GATE.len()==9) measured from those values.
    let expected_pass = [7usize, 4, 5, 4, 2, 4, 6, 6];

    let mut vals = Vec::new();
    for n in 1..=8u32 {
        let v = load_vals(n).unwrap_or_else(|| panic!("SaveData{n:02}.sav must load"));
        let e = expected[(n - 1) as usize];
        let got = (
            v.v4, v.v16, v.v22, v.v102, v.v104, v.v105, v.v157, v.v304, v.v326, v.v364,
        );
        assert_eq!(got, e, "SaveData{n:02}.sav variable snapshot");
        vals.push(v);
    }

    // Universal blockers: var304 == 0 (entry) and var326 == 0 (body) in EVERY save.
    for (n, v) in vals.iter().enumerate() {
        assert_eq!(
            v.v304,
            0,
            "SaveData{:02}: var304 == 0 (CE328 never enters)",
            n + 1
        );
        assert_eq!(
            v.v326,
            0,
            "SaveData{:02}: var326 == 0 (CE402 body gate var326>=2 fails)",
            n + 1
        );
    }

    // CE328 entry fails for EVERY save (var304==0 universal); the var4-range candidates
    // {02,06,07} are the only ones that would otherwise qualify on var104/105/var4.
    let mut entry_candidates = Vec::new();
    for (n, v) in vals.iter().enumerate() {
        assert!(
            !ce328_entry_ok(v),
            "SaveData{:02} cannot enter CE328 (var304==0)",
            n + 1
        );
        if v.v105 == 0 && v.v104 == 0 && (18..=20).contains(&v.v4) {
            entry_candidates.push(n + 1);
        }
    }
    assert_eq!(
        entry_candidates,
        vec![2, 6, 7],
        "var4-in-range entry candidates are SaveData02/06/07"
    );

    // Full DEEP_GATE conjunction is NEVER satisfied; the pass-count vector matches.
    let mut best = (0usize, 0usize); // (pass_count, save_number)
    for (n, v) in vals.iter().enumerate() {
        let passes = body_passes(v);
        let count = passes.iter().filter(|p| **p).count();
        assert_eq!(
            count,
            expected_pass[n],
            "SaveData{:02} DEEP_GATE pass count",
            n + 1
        );
        assert!(
            count < DEEP_GATE.len(),
            "SaveData{:02} does NOT satisfy the full deep gate",
            n + 1
        );
        if count > best.0 {
            best = (count, n + 1);
        }
    }
    assert_eq!(
        best,
        (7, 1),
        "closest save is SaveData01 with 7/9 predicates"
    );

    // Quantify the closest save: fails exactly {var157==13, var326>=2}.
    let s01 = vals[0];
    let fails: Vec<usize> = body_passes(&s01)
        .iter()
        .enumerate()
        .filter(|(_, p)| !**p)
        .map(|(i, _)| DEEP_GATE[i].0)
        .collect();
    assert_eq!(
        fails,
        vec![157, 326],
        "SaveData01 fails exactly var157 and var326"
    );
}

// =======================================================================================
// Test 3 - live VM: CE402 aborts on every retail save; CE328 runs its body but never
//          instantiates CE402.
// =======================================================================================

#[test]
fn ce402_aborts_and_ce328_skips_the_deep_branch_on_every_retail_save() {
    for n in 1..=8u32 {
        let Some(mut game) = open_bare(n) else {
            panic!("SaveData{n:02}.sav must load")
        };
        let (max402, saw410, warn402) = drive_started(&mut game, 402, 300);
        assert_eq!(warn402, 0, "SaveData{n:02}: CE402 run has no warnings");
        assert!(
            max402 < 20,
            "SaveData{n:02}: CE402 aborts at its first gate (max pc {max402})"
        );
        assert!(
            !saw410,
            "SaveData{n:02}: CE410 never entered from a bare CE402 start"
        );

        let Some(mut game) = open_bare(n) else {
            panic!("SaveData{n:02}.sav must load")
        };
        let (max328, saw410, warn328) = drive_started(&mut game, 328, 3000);
        assert_eq!(warn328, 0, "SaveData{n:02}: CE328 run has no warnings");
        assert!(
            max328 >= 1000,
            "SaveData{n:02}: CE328 advances its own body (max pc {max328})"
        );
        assert!(
            !saw410,
            "SaveData{n:02}: CE328 never reaches the pc986 CE402 sub-branch"
        );
    }
}

// =======================================================================================
// Test 4 - sufficiency: a synthetic save holding exactly DEEP_GATE DOES drive CE402 -> CE410.
//          Proves the only missing ingredient is the state, not a data/logic defect.
// =======================================================================================

#[test]
fn synthetic_deep_gate_drives_ce402_to_ce410_proving_state_is_the_only_blocker() {
    let path = build_synth_save();
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&path).expect("reload synthetic save");
    for (var, op, value) in DEEP_GATE {
        let lhs = game.vm.vars.get(var);
        assert!(
            compare(lhs, value, op),
            "synthetic var{var} satisfies the gate (op{op} {value})"
        );
    }
    drop(game);

    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&path).expect("reload synthetic save");
    // Read-load fix removes the old spurious 506 coroutine; pin this fixture's event RNG
    // so its intended deterministic sequence is preserved.
    game.vm.rng.set_event_seed(6);
    let (max402, saw410, warnings) = drive_started(&mut game, 402, 1500);
    assert_eq!(warnings, 0, "synthetic CE402 run has no engine warnings");
    assert!(
        max402 >= 300,
        "synthetic CE402 runs its deep body (max pc {max402} of 512)"
    );
    assert!(
        saw410,
        "synthetic CE402 enters the CE410 騎乗位系カットイン child"
    );
    std::fs::remove_file(&path).ok();
}
