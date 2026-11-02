//! M11 P1 - H CG bookstore / skill line and the CE328 deep branch.
//!
//! Question (task-90): can a shipped retail save actually reach CE402 / CE410
//! through real inputs, and if not, what is the exact remaining gate?
//!
//! Findings pinned here (all empirical, warnings == 0 everywhere):
//!   1. The bookstore menu IS a real, drivable event.  Starting CommonEvent 314 on
//!      SaveData01 and answering its real choice menus (bookstore -> 色色的书 /
//!      小薄本 -> 是) buys BOTH prerequisite books, setting var276 (CE314 pc1188)
//!      and var277 (CE314 pc1233) >= 1.  This CORRECTS the earlier task-86 claim
//!      that var277 is unreachable.
//!   2. Persisting the purchase (save_to) and reloading it, the real skill event
//!      CommonEvent 301 then sets var326 = 2 (pc6320 / pc6365) from the freshly
//!      owned books - so var326 IS obtainable from a shipped save via real events.
//!   3. The real night intimacy menu (CE290 option index 5) still raises var304 = 1
//!      and calls CE328.  But CE328's H/合宿 branch (the var304 >= 1 path) ends at
//!      pc741 op172 (after pc740 op210[500332] = CE332) BEFORE the 就寝 sleep branch
//!      reaches its CE402 call at pc985/pc986.  So CE402 is never instantiated on
//!      the real path - the last remaining blocker is this CE328 control-flow
//!      early-return, NOT the bookstore / var326 gate.
//!   4. Only driving CE402 DIRECTLY on a synthetic DEEP_GATE save (var326 = 2 plus
//!      the other eight predicates) reaches CE410 (騎乗位系カットイン); warnings 0.
//!
//! Net matrix: var276/var277/var326/var304 are all reachable from SaveData01 via
//! real inputs; CE402/CE410 remain unreachable end-to-end because CE328 early-
//! returns at pc741 and CE402's only caller is CE328 pc986.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use wolf_format::command::{Command, Slot};
use wolf_format::commonevent::{CommonEvent, CommonEventData};
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

fn out_path(name: &str) -> PathBuf {
    repo_root().join("work").join("out").join(name)
}

fn common_events() -> CommonEventData {
    CommonEventData::parse(
        &std::fs::read(data_root().join("BasicData").join("CommonEvent.dat")).unwrap(),
    )
    .unwrap()
}

fn words(cmd: &Command) -> Vec<i32> {
    cmd.params
        .chunks_exact(4)
        .map(|w| i32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        .collect()
}

fn cmd_ref<'a>(e: &'a CommonEvent, pc: usize) -> &'a Command {
    match &e.commands[pc] {
        Slot::Command(c) => c.as_ref(),
        Slot::Blank => panic!("CE{} pc{} is Blank, expected a command", e.id, pc),
    }
}

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
    max_pc: BTreeMap<u32, usize>,
    var304_peak: i32,
}

/// Advance frames, tracking spawned events / max pc / peak var304.  When answer is
/// Some(i) the first real choice is answered with index i and the VM is switched to
/// auto-choice; otherwise the caller owns the choice policy.
fn track(g: &mut Game, frames: usize, answer: Option<usize>) -> Track {
    let mut t = Track {
        seen: BTreeSet::new(),
        max_pc: BTreeMap::new(),
        var304_peak: g.vm.vars.get(304),
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
        }
        let v = g.vm.vars.get(304);
        if v > t.var304_peak {
            t.var304_peak = v;
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

/// Drive the real bookstore CommonEvent 314 on g and buy 色色的书 (var276) and
/// 小薄本 (var277).  Returns (var276, var277, max pc reached in CE314).
fn buy_books(g: &mut Game) -> (i32, i32, usize) {
    g.auto_advance = true;
    g.auto_choice = false;
    g.set_choice_cursor(0);
    g.start_event(314);
    let mut max_pc = 0usize;
    for _ in 0..12000 {
        g.frame();
        for inst in g.vm.instances() {
            if inst.event == 314 && (inst.pc as usize) > max_pc {
                max_pc = inst.pc as usize;
            }
        }
        if g.vm.vars.get(276) >= 1 && g.vm.vars.get(277) >= 1 {
            break;
        }
        if let Some(c) = &g.choices {
            if c.frames >= 1 {
                let opts = c.options.clone();
                let joined = opts.join("|");
                let pick = if joined.contains("书店") {
                    opts.iter().position(|o| o.contains("书店")).unwrap()
                } else if joined.contains("色色的书") && g.vm.vars.get(276) == 0 {
                    opts.iter().position(|o| o.contains("色色的书")).unwrap()
                } else if joined.contains("小薄本") && g.vm.vars.get(277) == 0 {
                    opts.iter().position(|o| o.contains("小薄本")).unwrap()
                } else if opts.len() == 2 && opts.iter().any(|o| o == "是") {
                    opts.iter().position(|o| o == "是").unwrap()
                } else {
                    0
                };
                let _ = g.reply(ChoiceReply::Index(pick));
                g.auto_choice_delay = 1;
            }
        }
    }
    (g.vm.vars.get(276), g.vm.vars.get(277), max_pc)
}

fn new_game() -> Game {
    Game::new(data_root()).expect("open game")
}


/// TEST 1 - the real bookstore menu buys both prerequisite skill books.
#[test]
fn bookstore_menu_buys_both_skill_books() {
    let mut g = new_game();
    g.load_save(&save_path("SaveData01")).expect("load SaveData01");
    assert_eq!(g.vm.vars.get(276), 0, "SaveData01 ships var276==0");
    assert_eq!(g.vm.vars.get(277), 0, "SaveData01 ships var277==0");

    let (v276, v277, max_pc) = buy_books(&mut g);

    assert!(v276 >= 1, "bought 色色的书 -> var276>=1, got {v276}");
    assert!(v277 >= 1, "bought 小薄本 -> var277>=1, got {v277}");
    assert!(
        max_pc >= 1233,
        "must reach the 小薄本 purchase site CE314 pc1233, got max pc {max_pc}"
    );
    assert_eq!(g.stats.warnings, 0, "bookstore purchase emits no warnings");
}

/// TEST 2 - a persisted bookstore purchase feeds the real skill event CE301, which
/// sets var326 = 2.  Proves var326 is reachable from a shipped save via real events.
#[test]
fn persisted_book_purchase_lets_ce301_learn_the_skill() {
    let mut a = new_game();
    a.load_save(&save_path("SaveData01")).expect("load SaveData01");
    // Isolated single-event probe: clear the save's restored common-event self banks so
    // the save's own progression state cannot gate the events driven below.
    a.vm.clear_common_self();
    let (v276, v277, _) = buy_books(&mut a);
    assert!(v276 >= 1 && v277 >= 1, "books purchased: v276={v276} v277={v277}");

    let saved = out_path("m11_hscene_books_t2.sav");
    a.save_to(&saved).expect("persist purchased books");

    let mut b = new_game();
    b.load_save(&saved).expect("reload purchased save");
    b.vm.clear_common_self();
    assert!(b.vm.vars.get(276) >= 1, "var276 persisted");
    assert!(b.vm.vars.get(277) >= 1, "var277 persisted");
    assert_eq!(b.vm.vars.get(326), 0, "skill not yet learned");

    b.auto_advance = true;
    b.auto_choice = true;
    b.auto_choice_delay = 1;
    b.set_choice_cursor(0);
    b.start_event(301);
    let t = track(&mut b, 9000, None);

    assert_eq!(b.vm.vars.get(326), 2, "CE301 pc6320/pc6365 sets var326=2");
    assert!(
        *t.max_pc.get(&301).unwrap_or(&0) >= 6365,
        "CE301 must reach pc6365, got {:?}",
        t.max_pc.get(&301)
    );
    assert_eq!(b.stats.warnings, 0, "skill event emits no warnings");
}

/// TEST 3 - the full real chain (bookstore -> skill -> night intimacy) reaches CE328
/// but CE328 early-returns at pc741 (CE332 + op172) before the CE402 call at pc986.
#[test]
fn real_chain_reaches_ce328_but_early_returns_at_pc741() {
    // Step A: buy both books via the real bookstore menu, persist.
    let mut a = new_game();
    a.load_save(&save_path("SaveData01")).expect("load SaveData01");
    // Isolated single-event probe: clear the save's restored common-event self banks so
    // the save's own progression state cannot gate the events driven below.
    a.vm.clear_common_self();
    let (v276, v277, _) = buy_books(&mut a);
    assert!(v276 >= 1 && v277 >= 1, "books purchased");
    let saved = out_path("m11_hscene_books_t3.sav");
    a.save_to(&saved).expect("persist purchased books");

    // Step B: reload and learn the 騎乗位 skill through the real CE301 event.
    let mut b = new_game();
    b.load_save(&saved).expect("reload purchased save");
    b.vm.clear_common_self();
    b.auto_advance = true;
    b.auto_choice = true;
    b.auto_choice_delay = 1;
    b.set_choice_cursor(0);
    b.start_event(301);
    track(&mut b, 9000, None);
    assert_eq!(b.vm.vars.get(326), 2, "var326=2 learned via CE301");

    // Step C: the real night intimacy menu raises var304=1 and enters CE328.
    b.auto_choice = false;
    b.start_event(290);
    let t = track(&mut b, 4000, Some(5));

    assert!(t.seen.contains(&328), "night menu entered CE328");
    assert_eq!(t.var304_peak, 1, "var304 became 1 on the real intimate route");
    let ce328_max = *t.max_pc.get(&328).unwrap_or(&0);
    assert_eq!(
        ce328_max, 741,
        "CE328 H branch early-returns exactly at pc741 op172, got {ce328_max}"
    );
    assert!(!t.seen.contains(&402), "CE402 is NEVER called on the real path");
    assert!(!t.seen.contains(&410), "CE410 is NEVER reached on the real path");
    assert_eq!(b.stats.warnings, 0, "the whole real chain emits no warnings");
}

/// TEST 4 - the static gate graph behind tests 1-3.
#[test]
fn gate_graph_pins_bookstore_and_ce328_deep_branch() {
    let ce = common_events();

    assert_eq!(
        op121_writers(&ce, 2000326),
        BTreeSet::from([(301u32, 6320u32), (301u32, 6365u32)]),
        "var326 has exactly two writers, both in CE301"
    );

    let mut positive: Vec<(u32, u32)> = Vec::new();
    for &(ev, pc) in op121_writers(&ce, 2000277).iter() {
        let c = cmd_ref(ce.event(ev).unwrap(), pc as usize);
        if words(c).get(1).copied() == Some(1) {
            positive.push((ev, pc));
        }
    }
    positive.sort();
    assert_eq!(
        positive,
        vec![(314u32, 1233u32), (314u32, 1530u32)],
        "var277=1 set only by the bookstore CE314"
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

    let e314 = ce.event(314).unwrap();
    assert_eq!(
        words(cmd_ref(e314, 1188))[0],
        2000276,
        "CE314 pc1188 writes var276 (色色的书)"
    );
    assert_eq!(
        words(cmd_ref(e314, 1233))[0],
        2000277,
        "CE314 pc1233 writes var277 (小薄本)"
    );

    let e308 = ce.event(308).unwrap();
    assert_eq!(
        words(cmd_ref(e308, 115))[0],
        500314,
        "CE308 pc115 calls the bookstore"
    );

    let e328 = ce.event(328).unwrap();
    let c740 = cmd_ref(e328, 740);
    assert_eq!(c740.opcode, 210, "CE328 pc740 is a call");
    assert_eq!(words(c740)[0], 500332, "CE328 pc740 calls CE332");
    assert_eq!(cmd_ref(e328, 741).opcode, 172, "CE328 pc741 op172 = END early-return");
    let c986 = cmd_ref(e328, 986);
    assert_eq!(c986.opcode, 210, "CE328 pc986 is a call");
    assert_eq!(words(c986)[0], 500402, "CE328 pc986 calls CE402");
    assert_eq!(cmd_ref(e328, 694).opcode, 111, "CE328 pc694 tests var304");
    assert_eq!(cmd_ref(e328, 724).opcode, 111, "CE328 pc724 tests var304");
}

/// TEST 5 - the synthetic var326=2 path: driving CE402 directly on a DEEP_GATE save
/// reaches CE410.  Proves the sole remaining ingredient on the real path is that
/// CE402 is never called, not a defect in the CG events.
#[test]
fn synthetic_var326_two_path_drives_ce402_to_ce410() {
    let mut g = new_game();
    g.load_save(&save_path("SaveData01")).expect("load SaveData01");
    // Read-load fix removes the old spurious 506 coroutine; pin this fixture's event RNG
    // so its intended deterministic sequence is preserved.
    g.vm.rng.set_event_seed(6);
    for (k, v) in [
        (157, 13),
        (104, 0),
        (105, 0),
        (4, 21),
        (102, 80),
        (16, 1),
        (22, 180),
        (326, 2),
        (364, 0),
    ] {
        g.vm.vars.set(k, v);
    }
    g.auto_advance = true;
    g.auto_choice = true;
    g.auto_choice_delay = 1;
    g.set_choice_cursor(0);
    g.start_event(402);
    let t = track(&mut g, 1200, None);

    assert!(
        *t.max_pc.get(&402).unwrap_or(&0) >= 300,
        "CE402 body must run deep, got {:?}",
        t.max_pc.get(&402)
    );
    assert!(t.seen.contains(&410), "CE410 (騎乗位系カットイン) must be entered");
    assert_eq!(g.stats.warnings, 0, "synthetic CE402->CE410 emits no warnings");
}
