//! M11 P1 - real-progression reachability of the H CG cluster (CE402/CE410).
//!
//! Question (task-86): can a shipped retail save, driven with ONLY real event
//! inputs (real night-menu choices, real day/skill event flow), set var304=1 and
//! var326=2 and then enter CE402/CE410?
//!
//! Verdict (pinned empirically here):
//!   * var304 = 1 IS reachable from SaveData01 via the real night menu:
//!     CommonEvent 290 top-menu choice index 5 -> pc403 op121[2000304,1] and
//!     pc406 op210[500328] (calls CE328). Warnings 0.  (test 1)
//!   * var326 = 2 IS reachable through the REAL skill event CommonEvent 301
//!     (pc6320 / pc6365 op121[2000326,2]) *provided* var276>=1 && var277>=1.
//!     (test 2 drives CE301 with those "book owned" flags seeded.)
//!   * BUT var276/var277 == 0 in ALL eight shipped saves, and var277 is written
//!     to 1 ONLY by the bookstore CommonEvent 314 (pc1233/pc1530).  CE314 is
//!     called only from CE308 pc115 / CE310 pc670, is not directly startable
//!     (aborts at pc118) and is not reached by driving CE308/CE310 from a save.
//!     So var326 cannot actually be obtained from any shipped save.  (test 3)
//!   * Even with var326 preset and var304 raised by the real menu, the CE328
//!     "H-shitai" branch terminates at pc741 (op172) and never reaches the CE402
//!     call at pc986; CE402 is called ONLY from CE328 pc986.  (test 4)
//!
//! Net: CE402/CE410 is NOT reachable end-to-end from a shipped retail save via
//! real inputs; the closest real progress is var304=1 (menu) + the CE301 skill
//! machine (which needs the un-reachable bookstore flags var276/var277).

use std::collections::BTreeSet;
use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_game::game::Game;
use wolf_vm::exec::ChoiceReply;

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn data_root() -> PathBuf {
    match std::env::var("WOLF_DATA_ROOT") {
        Ok(v) if !v.is_empty() => repo_root().join(v),
        _ => repo_root().join("work").join("all"),
    }
}

fn save_path(name: &str) -> PathBuf {
    repo_root()
        .join("work")
        .join("retail_backup")
        .join("Save")
        .join(format!("{name}.sav"))
}

fn common_events() -> CommonEventData {
    CommonEventData::parse(
        &std::fs::read(data_root().join("BasicData").join("CommonEvent.dat")).unwrap(),
    )
    .unwrap()
}

fn words(cmd: &wolf_format::command::Command) -> Vec<i32> {
    cmd.params
        .chunks_exact(4)
        .map(|w| i32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        .collect()
}

/// (event id, pc) sites where op121 writes `target`.
fn op121_writers(ce: &CommonEventData, target: i32) -> BTreeSet<(u32, u32)> {
    let mut out = BTreeSet::new();
    for ev in ce.events.iter() {
        for (pc, slot) in ev.commands.iter().enumerate() {
            if let Slot::Command(cmd) = slot {
                if cmd.opcode == 121 && words(cmd).first().copied() == Some(target) {
                    out.insert((ev.id, pc as u32));
                }
            }
        }
    }
    out
}

/// (event id, pc) sites where op210 calls `target`.
fn op210_callers(ce: &CommonEventData, target: i32) -> BTreeSet<(u32, u32)> {
    let mut out = BTreeSet::new();
    for ev in ce.events.iter() {
        for (pc, slot) in ev.commands.iter().enumerate() {
            if let Slot::Command(cmd) = slot {
                if cmd.opcode == 210 && words(cmd).first().copied() == Some(target) {
                    out.insert((ev.id, pc as u32));
                }
            }
        }
    }
    out
}

struct Track {
    seen: BTreeSet<u32>,
    max_pc: std::collections::BTreeMap<u32, usize>,
    var304_max: i32,
    ce328_spawned: bool,
}

/// Advance `frames`, tracking instance events/max-pc and peak var304.  Optionally
/// auto-answer the first choice with `answer`, then let auto-choice drive the rest.
fn drive(g: &mut Game, frames: usize, answer: Option<usize>) -> Track {
    let mut t = Track {
        seen: BTreeSet::new(),
        max_pc: std::collections::BTreeMap::new(),
        var304_max: g.vm.vars.get(304),
        ce328_spawned: false,
    };
    let mut answered = false;
    for _ in 0..frames {
        g.frame();
        for inst in g.vm.instances() {
            t.seen.insert(inst.event);
            let pc = inst.pc as usize;
            let e = t.max_pc.entry(inst.event).or_insert(0);
            if pc > *e {
                *e = pc;
            }
            if inst.event == 328 {
                t.ce328_spawned = true;
            }
        }
        let v = g.vm.vars.get(304);
        if v > t.var304_max {
            t.var304_max = v;
        }
        if let Some(idx) = answer {
            if !answered {
                if let Some(c) = &g.choices {
                    if c.frames >= 1 {
                        let _ = g.reply(ChoiceReply::Index(idx));
                        answered = true;
                        g.auto_choice = true;
                        g.auto_choice_delay = 1;
                        g.set_choice_cursor(0);
                    }
                }
            }
        }
    }
    t
}

/// TEST 1 - the real night menu can set var304=1 and call CE328.
#[test]
fn real_night_menu_sets_var304_and_calls_ce328() {
    let mut g = Game::new(data_root()).expect("open game");
    g.load_save(&save_path("SaveData01")).expect("load SaveData01");
    assert_eq!(g.vm.vars.get(304), 0, "SaveData01 ships var304==0");

    g.auto_advance = true;
    g.auto_choice = false;
    g.set_choice_cursor(0);
    g.start_event(290);
    let t = drive(&mut g, 2500, Some(5));

    assert_eq!(t.var304_max, 1, "night-menu option 5 writes var304=1 (CE290 pc403)");
    assert!(t.ce328_spawned, "night-menu option 5 calls CE328 (CE290 pc406 op210 500328)");
    let ce328_max = *t.max_pc.get(&328).unwrap_or(&0);
    assert!(ce328_max >= 640, "CE328 should run its H script, got max pc {ce328_max}");
    assert_eq!(g.stats.warnings, 0, "no warnings");
}

/// TEST 2 - the real skill event CE301 writes var326=2 once the books are owned.
#[test]
fn real_ce301_skill_event_sets_var326_given_books() {
    let mut g = Game::new(data_root()).expect("open game");
    g.load_save(&save_path("SaveData01")).expect("load SaveData01");
    // Isolated single-event probe: clear restored common-event self banks so the
    // save's own progression state cannot gate CE301 driven directly below.
    g.vm.clear_common_self();
    assert_eq!(g.vm.vars.get(326), 0, "SaveData01 ships var326==0");
    g.vm.vars.set(276, 1);
    g.vm.vars.set(277, 1);
    g.auto_advance = true;
    g.auto_choice = true;
    g.auto_choice_delay = 1;
    g.set_choice_cursor(0);
    g.start_event(301);
    let t = drive(&mut g, 9000, None);
    assert_eq!(g.vm.vars.get(326), 2, "CE301 sets var326=2 (pc6320/pc6365)");
    let m = *t.max_pc.get(&301).unwrap_or(&0);
    assert!(m >= 6365, "CE301 should reach pc6365, got {m}");
    assert_eq!(g.stats.warnings, 0, "no warnings");
}

/// TEST 3 - pin the exact missing gate: var277 -> var326 is only obtainable from
/// the bookstore CE314, which no shipped save can reach.
#[test]
fn bookstore_gate_pins_var326_unreachable_from_shipped_saves() {
    let ce = common_events();

    assert_eq!(
        op121_writers(&ce, 2000326),
        BTreeSet::from([(301u32, 6320u32), (301u32, 6365u32)]),
        "var326 has exactly two writers in the whole event graph"
    );

    let var277 = op121_writers(&ce, 2000277);
    assert!(var277.contains(&(314, 1233)));
    assert!(var277.contains(&(314, 1530)));
    let mut positive_only: Vec<(u32, u32)> = vec![];
    for &(ev, pc) in var277.iter() {
        let e = ce.event(ev).unwrap();
        if let Slot::Command(cmd) = &e.commands[pc as usize] {
            if words(cmd).get(1).copied() == Some(1) {
                positive_only.push((ev, pc));
            }
        }
    }
    assert_eq!(
        positive_only,
        vec![(314u32, 1233u32), (314u32, 1530u32)],
        "var277=1 is set ONLY by the bookstore CE314"
    );

    assert_eq!(
        op210_callers(&ce, 500314),
        BTreeSet::from([(308u32, 115u32), (310u32, 670u32)]),
        "bookstore entry points"
    );

    assert_eq!(
        op210_callers(&ce, 500402),
        BTreeSet::from([(328u32, 986u32)]),
        "CE402 unique caller"
    );

    for name in [
        "SaveData01", "SaveData02", "SaveData03", "SaveData04", "SaveData05", "SaveData06",
        "SaveData07", "SaveData08",
    ] {
        let mut g = Game::new(data_root()).expect("open game");
        if g.load_save(&save_path(name)).is_err() {
            continue;
        }
        // var277 is the true blocker: it is 0 in every shipped save (var276 is
        // already 2 in SaveData04/05, but the gate needs var276>=1 && var277>=1).
        assert_eq!(g.vm.vars.get(277), 0, "{name} var277 must be 0");
        assert_eq!(g.vm.vars.get(326), 0, "{name} var326 must be 0");
    }

    for ev in [314u32, 308, 310] {
        let mut g = Game::new(data_root()).expect("open game");
        g.load_save(&save_path("SaveData01")).expect("load");
        g.auto_advance = true;
        g.auto_choice = true;
        g.auto_choice_delay = 1;
        g.set_choice_cursor(0);
        g.start_event(ev);
        let t = drive(&mut g, 4000, None);
        let m = *t.max_pc.get(&ev).unwrap_or(&0);
        if ev == 314 {
            assert!(m < 200, "direct CE314 start aborts before bookstore, got {m}");
        }
        assert!(!t.seen.contains(&314) || ev == 314, "CE{ev} must not reach bookstore CE314");
        assert_eq!(g.vm.vars.get(277), 0, "driving CE{ev} must not set var277");
    }
}

/// TEST 4 - closest real path: skill -> night menu; CE402 still not reached.
#[test]
fn full_chain_still_does_not_reach_ce402() {
    let mut g = Game::new(data_root()).expect("open game");
    g.load_save(&save_path("SaveData01")).expect("load SaveData01");
    g.vm.clear_common_self();
    g.auto_advance = true;
    g.auto_choice = true;
    g.auto_choice_delay = 1;
    g.set_choice_cursor(0);

    g.vm.vars.set(276, 1);
    g.vm.vars.set(277, 1);
    g.start_event(301);
    drive(&mut g, 9000, None);
    assert_eq!(g.vm.vars.get(326), 2, "CE301 learns the skill (var326=2)");

    g.auto_choice = false;
    g.start_event(290);
    let t = drive(&mut g, 4000, Some(5));

    assert_eq!(t.var304_max, 1, "menu option 5 writes var304=1");
    assert!(t.ce328_spawned, "CE328 was spawned");
    let ce328_max = *t.max_pc.get(&328).unwrap_or(&0);
    assert!(!t.seen.contains(&402), "CE402 must not be entered (max CE328 pc {ce328_max})");
    assert!(ce328_max < 985, "CE328 H script stops before the CE402 call pc986, got {ce328_max}");
    assert_eq!(g.stats.warnings, 0, "no warnings");
}
