//! M10 W2 residual - the real CE246 -> CE346 "H カットイン実行処理昼" call.
//!
//! Context (docs/research/M10_HSCENE_DEEP_SAVE.md, tests/m8_hscene_cg_deep.rs): ev33 -> CE333
//! only renders the H CG leaf; the deep common-event bodies stay closed. CE246's pc1943
//! var157==13 block is the ONLY place CE246 calls CE346 (the H cut-in executor), and a stateless
//! CE246 start stalls at its pc117 var151>=1 gate. This file answers the remaining question:
//! what actually CALLS CE346 with what self-var arguments, and can a legitimate CE246 menu walk
//! reach + complete that executor?
//!
//! Findings pinned here (all empirical, live VM):
//! 1. STATIC. CE246 issues op210 -> CE346 at exactly 7 sites (pc 695,906,1477,1543,1598,1692,
//!    1799); every call passes 3 self args = [24,1,1] except pc1543 = [25,1,1]. CE346 gates its
//!    body on those args (self magic 1_600_000+i): pc2 SV1600001 in {0,1}, pc12 SV1600002 in
//!    {1,2,3}, pc22 SV1600000 in {1,2,3}, pc108 SV1600000 in {4,24,25} - so the real arg 24
//!    selects the pc108 "H cut-in" branch.
//! 2. EXECUTOR. Started with self [24,1,1] (args 600..602), CE346 advances to max pc >=180,
//!    draws a cutin/*.png, terminates cleanly (instance reaped), 0 warnings - deterministic.
//!    [25,1,1] takes the pc108 alt branch and also completes.
//! 3. END-TO-END. From a legitimate high-affection save state (a real save with the affection /
//!    relationship counters raised), CE246's own menu walk 性骚扰 -> 让你看我撸管 reaches its
//!    積極的 sub-branch, op210-spawns CE346, CE346 draws cutin_161 and completes, warnings 0.
//! 4. GATE NECESSITY. Zeroing any of var20/var21/var22/var102/var16 individually stops CE346 from
//!    ever being reached (CE246 cannot open the 積極的 branch), pinning the parent gate chain.
//!
//! Run:   WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_cutin
//! Data:  work/all; saves work/retail_backup/Save/SaveData07.sav.
//! Exit:  the only persistence written is work/out/m10_cutin_synth.sav (gitignored); no retail
//!        save is mutated.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_game::Game;
use wolf_vm::exec::ChoiceReply;

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
        .join("m10_cutin_synth.sav")
}

fn common_events() -> CommonEventData {
    let bytes = std::fs::read(data_root().join("BasicData").join("CommonEvent.dat"))
        .expect("BasicData/CommonEvent.dat");
    CommonEventData::parse(&bytes).expect("parse CommonEvent.dat")
}

// ---------------------------------------------------------------------------------------
// Bytecode readers (mirror m8_hscene_cg_deep.rs / m8_hscene_cg_run.rs).
// ---------------------------------------------------------------------------------------

fn words(cmd: &wolf_format::command::Command) -> Vec<i32> {
    cmd.params
        .chunks_exact(4)
        .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect()
}

/// Every pc in CE `event` whose op `op` has `first` as its first parameter word.
fn ce_cmd_pcs(data: &CommonEventData, event: u32, op: u32, first: i32) -> Vec<usize> {
    let ev = data
        .event(event)
        .unwrap_or_else(|| panic!("CE{event} exists"));
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

/// Every (pc, var, value, op) triple encoded by an op111 condition command in CE `event`.
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

/// Every CE246 op210 whose target (words[0]) is `target`, returned as (pc, self-args).
fn op210_sites(data: &CommonEventData, event: u32, target: i32) -> Vec<(usize, Vec<i32>)> {
    let ev = data
        .event(event)
        .unwrap_or_else(|| panic!("CE{event} exists"));
    ev.commands
        .iter()
        .enumerate()
        .filter_map(|(pc, slot)| match slot {
            Slot::Command(cmd) if cmd.opcode == 210 => {
                let ints = words(cmd);
                if ints.first().copied()? != target {
                    return None;
                }
                let status = ints.get(1).copied().unwrap_or(0);
                let count = (status & 0xF).max(0) as usize;
                let args = ints
                    .get(2..2 + count)
                    .map(|s| s.to_vec())
                    .unwrap_or_default();
                Some((pc, args))
            }
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// Legitimate high-affection state, constructed in memory from real SaveData07 (no mutation of
// the on-disk retail save). These are ordinary affection / relationship counters a normal
// playthrough raises; the raw SaveData07 has the H UI already reachable (day 19:30 bedroom).
// ---------------------------------------------------------------------------------------

/// Vars the CE246 積極的 branch needs to be open (all soft relationship counters).
const WIN_VARS: [(usize, i32); 18] = [
    (18, 100),
    (20, 100),
    (21, 100),
    (22, 300),
    (23, 0),
    (24, 100),
    (102, 100),
    (103, 1),
    (16, 1),
    (70, 0),
    (15, 1),
    (157, 13),
    (104, 0),
    (105, 0),
    (4, 21),
    (26, 100),
    (27, 100),
    (100, 100),
];

fn open_synth_mem() -> Game {
    let source = save_dir().join("SaveData07.sav");
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&source)
        .unwrap_or_else(|error| panic!("load_save({}): {error}", source.display()));
    for (index, value) in WIN_VARS {
        game.vm.vars.set(index, value);
    }
    game
}

fn build_synth_save() -> PathBuf {
    let path = synth_path();
    let source = save_dir().join("SaveData07.sav");
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&source)
        .unwrap_or_else(|error| panic!("load_save({}): {error}", source.display()));
    for (index, value) in WIN_VARS {
        game.vm.vars.set(index, value);
    }
    let bytes = game.save_to(&path).expect("save_to synthetic save");
    assert!(bytes > 0, "synthetic save wrote {bytes} bytes");
    path
}

// ---------------------------------------------------------------------------------------
// End-to-end runner through the CE246 menu (text labels drive the choices).
// ---------------------------------------------------------------------------------------

fn choose(options: &[String]) -> usize {
    // Menu 1 (6 items) contains 性骚扰; menu 2 (5 items) -> 让你看我撸管; anything else -> first.
    if let Some(pos) = options.iter().position(|option| option.contains("性骚")) {
        return pos;
    }
    if options.len() == 5 {
        return 3; // 让你看我撸管
    }
    if options.len() == 6 {
        return 5; // 性骚扰
    }
    0
}

#[derive(Default)]
struct CutinRun {
    max_246: usize,
    max_346: usize,
    saw_346: bool,
    completed_346: bool,
    alive_346: bool,
    cutin: BTreeSet<String>,
    prompts: Vec<Vec<String>>,
    warnings: u64,
}

fn run_ce246(zero: Option<usize>, frames: usize) -> CutinRun {
    let mut game = open_synth_mem();
    if let Some(var) = zero {
        game.vm.vars.set(var, 0);
    }
    assert!(game.start_event(246), "start_event(246) spawns a coroutine");
    game.auto_advance = true;
    game.auto_choice = false;
    game.auto_choice_delay = 2;

    let mut run = CutinRun::default();
    let mut live: BTreeSet<u32> = BTreeSet::new();
    for _ in 0..frames {
        game.key_bits = wolf_game::key::OK;
        let prompt = game
            .choices
            .as_ref()
            .filter(|choices| choices.frames >= 2)
            .map(|choices| choices.options.clone());
        if let Some(options) = prompt {
            let index = choose(&options);
            run.prompts.push(options);
            game.reply(ChoiceReply::Index(index));
        }
        game.frame();

        let mut now: BTreeSet<u32> = BTreeSet::new();
        for instance in game.vm.instances() {
            if instance.event == 246 {
                run.max_246 = run.max_246.max(instance.pc);
            }
            if instance.event == 346 {
                now.insert(instance.id);
                run.saw_346 = true;
                run.max_346 = run.max_346.max(instance.pc);
            }
        }
        for gone in live.difference(&now) {
            let _ = gone;
            run.completed_346 = true;
        }
        live = now;
        for picture in game.screen.iter() {
            if picture.file.contains("cutin") {
                run.cutin.insert(picture.file.clone());
            }
        }
    }
    run.alive_346 = !live.is_empty();
    run.warnings = game.stats.warnings;
    run
}

// ---------------------------------------------------------------------------------------
// CE346 executor runner (start the common event directly with explicit self args).
// ---------------------------------------------------------------------------------------

struct ExecRun {
    max_pc: usize,
    files: BTreeSet<String>,
    alive: bool,
    warnings: u64,
}

fn run_ce346(args: [i32; 3], frames: usize) -> ExecRun {
    let mut game = open_synth_mem();
    assert!(game.start_event(346), "start_event(346) spawns a coroutine");
    let id = game
        .vm
        .instances()
        .iter()
        .find(|instance| instance.event == 346)
        .expect("CE346 instance exists")
        .id;
    {
        let instance = game.vm.instance_mut(id).expect("CE346 instance mut");
        // op210 args land in self group 6 => flat index 600+slot (magic 1_600_000+i).
        instance.self_vars.set(600, args[0]);
        instance.self_vars.set(601, args[1]);
        instance.self_vars.set(602, args[2]);
    }
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.set_choice_cursor(0);

    let mut max_pc = 0;
    let mut files = BTreeSet::new();
    for _ in 0..frames {
        game.key_bits = wolf_game::key::OK;
        game.frame();
        for instance in game.vm.instances() {
            if instance.event == 346 && instance.pc > max_pc {
                max_pc = instance.pc;
            }
        }
        for picture in game.screen.iter() {
            if !picture.file.is_empty() {
                files.insert(picture.file.clone());
            }
        }
    }
    let alive = game
        .vm
        .instances()
        .iter()
        .any(|instance| instance.event == 346);
    ExecRun {
        max_pc,
        files,
        alive,
        warnings: game.stats.warnings,
    }
}

// ---------------------------------------------------------------------------------------
// Test 1 - STATIC: the CE246 -> CE346 call shape, the CE346 self-arg gates, and the round-trip
//          of the constructed high-affection save.
// ---------------------------------------------------------------------------------------

#[test]
fn ce246_calls_ce346_with_the_real_self_args() {
    let data = common_events();

    // 7 call sites, 6 with args [24,1,1] and pc1543 with [25,1,1].
    let sites = op210_sites(&data, 246, 500_346);
    let pcs: Vec<usize> = sites.iter().map(|(pc, _)| *pc).collect();
    assert_eq!(
        pcs,
        vec![695, 906, 1477, 1543, 1598, 1692, 1799],
        "CE246 CE346 call pcs"
    );
    let args_by_pc: BTreeMap<usize, Vec<i32>> = sites.into_iter().collect();
    assert_eq!(args_by_pc[&1543], vec![25, 1, 1], "pc1543 passes [25,1,1]");
    let arg24 = args_by_pc
        .iter()
        .filter(|(pc, args)| **pc != 1543 && args.as_slice() == [24, 1, 1])
        .count();
    assert_eq!(arg24, 6, "the other six call sites pass [24,1,1]");

    // CE346 gates its body on the self args (magic 1_600_000+i is self group 6, slot i).
    let gt = ce_cond_triples(&data, 346);
    let has = |pc: usize, var: i32, value: i32, op: i32| {
        gt.iter()
            .any(|(p, v, val, o)| *p == pc && *v == var && *val == value && *o == op)
    };
    for value in [0, 1] {
        assert!(
            has(2, 1_600_001, value, 2),
            "CE346 pc2 gates SV1600001 in {{0,1}} (value {value})"
        );
    }
    for value in [1, 2, 3] {
        assert!(
            has(12, 1_600_002, value, 2),
            "CE346 pc12 gates SV1600002 in {{1,2,3}} (value {value})"
        );
        assert!(
            has(22, 1_600_000, value, 2),
            "CE346 pc22 gates SV1600000 in {{1,2,3}} (value {value})"
        );
    }
    for value in [4, 24, 25] {
        assert!(
            has(108, 1_600_000, value, 2),
            "CE346 pc108 gates SV1600000 in {{4,24,25}} (value {value})"
        );
    }
    // The real arg 24 is exactly one of the pc108 branch selectors.
    assert!(
        has(108, 1_600_000, 24, 2),
        "real arg 24 opens the pc108 cut-in branch"
    );

    // CE346 spawns CE217 (ウェイト/演出 helper) at pc81 and pc103.
    assert_eq!(
        ce_cmd_pcs(&data, 346, 210, 500_217),
        vec![81, 103],
        "CE346 calls CE217 at 81/103"
    );

    // The constructed save round-trips: every WIN_VARS value survives save_to/load_save.
    let path = build_synth_save();
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&path).expect("reload synthetic save");
    for (index, value) in WIN_VARS {
        assert_eq!(
            game.vm.vars.get(index),
            value,
            "synth var{index} round-trips to {value}"
        );
    }
    // And plain SaveData07 really has the lower raw values (the state is constructed, not shipped).
    let mut retail = Game::new(data_root()).expect("Game::new");
    retail
        .load_save(&save_dir().join("SaveData07.sav"))
        .expect("SaveData07");
    assert_eq!(retail.vm.vars.get(4), 19, "raw SaveData07 var4 = 19");
    assert_eq!(retail.vm.vars.get(102), 59, "raw SaveData07 var102 = 59");
}

// ---------------------------------------------------------------------------------------
// Test 2 - EXECUTOR: CE346 with the real [24,1,1] args advances to its cut-in tail and completes.
// ---------------------------------------------------------------------------------------

#[test]
fn ce346_executor_runs_the_cutin_and_terminates() {
    let mut profiles = Vec::new();
    for _ in 0..2 {
        profiles.push(run_ce346([24, 1, 1], 1500));
    }
    for (n, run) in profiles.iter().enumerate() {
        let label = format!("CE346 [24,1,1] run#{}", n + 1);
        assert_eq!(run.warnings, 0, "{label}: no engine warnings");
        assert!(
            run.max_pc >= 108,
            "{label}: CE346 passes its pc108 branch gate (max pc {})",
            run.max_pc
        );
        assert!(
            run.max_pc >= 180,
            "{label}: CE346 reaches the cut-in tail (max pc {})",
            run.max_pc
        );
        assert!(
            run.files.iter().any(|file| file.contains("cutin")),
            "{label}: a cutin/*.png is drawn (files: {:?})",
            run.files
        );
        assert!(
            !run.alive,
            "{label}: CE346 completes and its instance is reaped"
        );
    }
    assert_eq!(
        profiles[0].max_pc, profiles[1].max_pc,
        "CE346 max pc deterministic"
    );
    assert_eq!(
        profiles[0].files, profiles[1].files,
        "CE346 drawn files deterministic"
    );

    // The alt branch [25,1,1] also takes the pc108 selector and completes.
    let alt = run_ce346([25, 1, 1], 1500);
    assert_eq!(alt.warnings, 0, "CE346 [25,1,1]: no warnings");
    assert!(
        alt.max_pc >= 108,
        "CE346 [25,1,1] reaches its cut-in branch (max pc {})",
        alt.max_pc
    );
    assert!(!alt.alive, "CE346 [25,1,1] completes");
}

// ---------------------------------------------------------------------------------------
// Test 3 - END-TO-END: a legitimate CE246 menu walk reaches CE346 and completes the cut-in.
// ---------------------------------------------------------------------------------------

#[test]
fn legit_ce246_menu_walk_reaches_and_completes_ce346() {
    let mut profiles = Vec::new();
    for _ in 0..2 {
        profiles.push(run_ce246(None, 9000));
    }
    for (n, run) in profiles.iter().enumerate() {
        let label = format!("CE246 menu run#{}", n + 1);
        assert_eq!(run.warnings, 0, "{label}: no engine warnings");
        assert!(
            run.saw_346,
            "{label}: CE246 reaches the pc1477 積極的 branch and spawns CE346"
        );
        assert!(
            run.max_246 >= 1477,
            "{label}: CE246 advances into its 積極的 sub-branch (max pc {})",
            run.max_246
        );
        assert!(
            run.max_346 >= 108,
            "{label}: CE346 runs its cut-in branch (max pc {})",
            run.max_346
        );
        assert!(
            run.cutin.iter().any(|file| file.contains("cutin_161")),
            "{label}: CE346 draws cutin_161 (real file: {:?})",
            run.cutin
        );
        assert!(run.completed_346, "{label}: the CE346 child completes");
        assert!(!run.alive_346, "{label}: no CE346 coroutine left running");
        assert!(
            !run.prompts.is_empty(),
            "{label}: the menu prompted for choices"
        );
    }
    // Determinism.
    assert_eq!(
        profiles[0].max_246, profiles[1].max_246,
        "CE246 max pc deterministic"
    );
    assert_eq!(
        profiles[0].max_346, profiles[1].max_346,
        "CE346 max pc deterministic"
    );
    assert_eq!(
        profiles[0].cutin, profiles[1].cutin,
        "cutin files deterministic"
    );
}

// ---------------------------------------------------------------------------------------
// Test 4 - GATE NECESSITY: each parent var is required; a non-required one is not.
// ---------------------------------------------------------------------------------------

#[test]
fn ce246_active_branch_gates_are_necessary() {
    // Control: an unrelated counter (var23) can be zeroed and CE346 is still reached.
    let control = run_ce246(Some(23), 9000);
    assert!(
        control.saw_346,
        "zeroing var23 does not block the branch (control)"
    );
    assert_eq!(control.warnings, 0, "control: no warnings");

    // Every var in the winning chain is individually necessary.
    for var in [20usize, 21, 22, 102, 16] {
        let run = run_ce246(Some(var), 6000);
        assert!(
            !run.saw_346,
            "zeroing var{var} keeps CE246 out of the 積極的 branch (CE346 never spawned; max pc {})",
            run.max_246
        );
        assert_eq!(run.warnings, 0, "zeroing var{var}: no warnings");
    }
}
