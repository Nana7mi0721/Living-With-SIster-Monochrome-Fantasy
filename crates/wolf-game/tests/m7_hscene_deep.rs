//! W2-d: real-save reachability of the deep H branch (CE333 / CE346) inside CE246.
//!
//! Investigation result encoded here:
//!   * CE346 (H cut-in) IS reachable end-to-end from the real SaveData08 save,
//!     via 性骚扰 -> 让我看裸体 -> 让我看更多些 -> 看破, with NO fabricated state.
//!     The cutin event really starts and draws visible cutin/cutin_*.png layers.
//!   * CE333 (the 深夜パート失敗ルート / 妹爆発 BADEND) is NOT reachable from any
//!     retail save: it needs v23 == 0 (and v157 != 13) to fall through to the
//!     ELSE branch at pc1980. Every retail SaveData01..08 has v23 >= 6, so the
//!     three success cases at pc1951 always match. Forcing v23 = 0 (a state no
//!     real save has) reproduces the CE333 branch, pinning the exact blocker.
//!
//! Note: op210 CALLEV child events (e.g. CE346/CE333) are NOT recorded in
//! vm.started_events; they are observed as live instances via vm.instances()
//! plus their on-screen side effects (cutin pictures / explosions / var166).
use std::{collections::BTreeSet, path::PathBuf};
use wolf_game::Game;
use wolf_save::SaveFile;
use wolf_vm::exec::ChoiceReply;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}
fn data_root() -> PathBuf {
    match std::env::var("WOLF_DATA_ROOT") {
        Ok(value) if !value.is_empty() => {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                repo_root().join(path)
            }
        }
        _ => repo_root().join("work/all"),
    }
}
fn save_path(n: u32) -> PathBuf {
    repo_root().join(format!("work/retail_backup/Save/SaveData{n:02}.sav"))
}

struct DeepRun {
    deep_reached: bool,
    ce346_seen: bool,
    ce346_maxpc: usize,
    ce346_frames: u32,
    cutin_visible: Vec<(i32, String, i32)>, // (picture number, file, opacity)
    var166: i32,
    explosion_seen: bool,
    warnings: u64,
}

/// Drive CE246 (スキンシップ) from a real save: pick 性骚扰, then the chosen
/// deep sub-option, then auto-pick 让我看更多些 / 看破 / 发射 as they appear.
/// `force_v23` is used ONLY by the blocker test to prove what CE333 needs.
fn drive(save: u32, deep_index: usize, force_v23: Option<i32>) -> DeepRun {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.auto_advance = true;
    game.auto_choice = false;
    game.load_save(&save_path(save)).expect("load_save");
    if let Some(v) = force_v23 {
        game.vm.vars.set(23, v);
    }
    game.start_event(246);

    let mut deep_reached = false;
    let mut ce346_seen = false;
    let mut ce346_maxpc = 0usize;
    let mut ce346_frames = 0u32;
    let mut cutin: BTreeSet<(i32, String, i32)> = BTreeSet::new();
    let mut explosion_seen = false;
    let mut last = 0u32;

    for f in 0..80_000u32 {
        if let Some(choice) = &game.choices {
            let opts = choice.options.clone();
            if opts.iter().any(|o| o == "小鸟飞飞") {
                deep_reached = true;
            }
            let idx: usize = if let Some(i) = opts.iter().position(|o| o == "性骚扰") {
                i
            } else if opts.iter().any(|o| o == "小鸟飞飞") {
                deep_index.min(opts.len() - 1)
            } else {
                opts.iter()
                    .position(|o| o == "让我看更多些")
                    .unwrap_or_default()
            };
            game.reply(ChoiceReply::Index(idx));
        }
        game.frame();

        for inst in game.vm.instances() {
            if inst.event == 346 {
                ce346_seen = true;
                ce346_maxpc = ce346_maxpc.max(inst.pc);
                ce346_frames += 1;
            }
        }
        for p in game.screen.iter() {
            let file = p.file.to_lowercase();
            if file.contains("explosion") {
                explosion_seen = true;
            }
            if file.contains("cutin") && p.opacity > 0 {
                cutin.insert((p.number, p.file.clone(), p.opacity));
            }
        }
        if let Some(inst) = game.vm.instances().iter().find(|i| i.event == 246) {
            last = f;
            let _ = inst;
        } else if deep_reached && f > last + 2 {
            break;
        }
    }

    DeepRun {
        deep_reached,
        ce346_seen,
        ce346_maxpc,
        ce346_frames,
        cutin_visible: cutin.into_iter().collect(),
        var166: game.vm.vars.get(166),
        explosion_seen,
        warnings: game.stats.warnings,
    }
}

#[test]
fn ce346_cutin_deep_branch_reachable_from_real_savedata08() {
    // Deep option index 1 == 让我看裸体.
    let run = drive(8, 1, None);
    println!(
        "CE346 deep: deep={} ce346={} maxpc={} frames={} var166={} cutin={:?} warn={}",
        run.deep_reached,
        run.ce346_seen,
        run.ce346_maxpc,
        run.ce346_frames,
        run.var166,
        run.cutin_visible,
        run.warnings,
    );
    assert!(
        run.deep_reached,
        "性骚扰 deep submenu (5 options) must be reachable"
    );
    assert!(
        run.ce346_seen,
        "CE346 (H cut-in) must actually start on the real-gate 裸体 path of SaveData08"
    );
    assert!(
        run.ce346_maxpc > 0,
        "CE346 instance must advance through its program"
    );
    assert!(
        !run.cutin_visible.is_empty(),
        "at least one visible cutin/cutin_*.png layer must be on screen while CE346 runs"
    );
    assert!(
        run.cutin_visible.iter().any(|(_, _, op)| *op == 255),
        "cutin layer must reach full opacity (255)"
    );
    assert_eq!(
        run.warnings, 0,
        "no engine warnings driving the deep H branch"
    );
}

#[test]
fn ce333_requires_v23_zero_which_no_retail_save_has() {
    // (1) No retail save carries v23 == 0 (the value CE333's ELSE needs).
    for n in 1..=8u32 {
        let bytes =
            std::fs::read(save_path(n)).unwrap_or_else(|e| panic!("read SaveData{n:02}: {e}"));
        let save = SaveFile::parse(&bytes).unwrap_or_else(|e| panic!("parse SaveData{n:02}: {e}"));
        let v23 = save
            .normal_var(23)
            .unwrap_or_else(|| panic!("SaveData{n:02} has no var23"));
        assert!(
            v23 >= 6,
            "SaveData{n:02}: v23={v23} expected >=6; CE333 needs v23==0, absent everywhere"
        );
    }

    // (2) Real SaveData08 on 小鸟飞飞 takes a success case; CE333 bad-end is NOT entered.
    let real = drive(8, 4, None);
    println!(
        "CE333 real Save08: deep={} var166={} explosion={}",
        real.deep_reached, real.var166, real.explosion_seen,
    );
    assert!(real.deep_reached, "小鸟飞飞 deep option must be reachable");
    assert_eq!(
        real.var166, 0,
        "real save keeps var166=0 (CE333 branch not entered)"
    );
    assert!(
        !real.explosion_seen,
        "real save shows no 妹爆発 explosion CG"
    );

    // (3) Only v23 == 0 (fabricated, no retail save has it) reaches the CE333 ELSE bad-end.
    let fab = drive(8, 4, Some(0));
    println!(
        "CE333 v23:=0: deep={} var166={} explosion={}",
        fab.deep_reached, fab.var166, fab.explosion_seen,
    );
    assert_eq!(
        fab.var166, 1,
        "var166=1 is written at pc1982 immediately before CE333 pc1983"
    );
    assert!(
        fab.explosion_seen,
        "CE333 bad-end branch draws cg/explosion* cutscenes"
    );
}
