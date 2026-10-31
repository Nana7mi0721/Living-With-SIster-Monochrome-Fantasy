//! M11 P1 - CE328 topic/branch enumeration and the real path to CE402.
//!
//! Question (task-94): can any real-input path from a SHIPPED retail save drive
//! CommonEvent 328 (the nightly 深夜 family-meeting / sleep menu) to its
//! pc986 op210[500402] call, i.e. instantiate CE402 -> CE410 (the H CG cluster)?
//!
//! Answer pinned here (all empirical, warnings == 0 everywhere):
//!   * The CE328 topic menu (pc173 op102, 4 options 梦想的话题/无关紧要的话题/
//!     色情的话题/小鸟飞飞！) is real and drivable, but it only opens when the day
//!     handler leaves var6 == 5 (Friday) AND var4 <= 3 (early month) at pc102/pc104
//!     (CE328 pc106 jumps to the 家族会議 body pc126).  No shipped save sits in
//!     that calendar window at CE328 time, and even when forced into it, every
//!     topic index (0..3) diverges through pc303..668 -> pc749 -> pc755 -> ... ->
//!     pc843 (お風呂を0にすること) -> pc1023 END.  It NEVER reaches pc986.
//!   * The var304 >= 1 night-intimacy entry (pc15 -> pc17 Hしたい -> pc90 body)
//!     short-circuits through pc727/pc731 to pc740 op210[500332] = CE332 then
//!     pc741 op172 END.  CE332 pc125 sets var304 = 0.  pc986 is never reached.
//!   * pc986 is reachable ONLY from the 悪戯へ label body (pc862) which is entered
//!     through the 通常就寝 (pc745) body's dispatch pc827..pc861.  That body is
//!     entered only via a 就寝 jump to label pc744 (-> pc745): pc122 or pc721.
//!     pc721 is dead - pc719 op499 closes the 合宿 block and jumps straight to
//!     pc749, skipping pc721.  pc122 sits in the var6 == 4 (Thursday) & var4 > 20
//!     branch (pc111 -> pc115 else -> pc121 -> pc122), a calendar window disjoint
//!     from the Friday family-meeting window, and the 745 body is additionally
//!     gated at pc746 on var18 >= 50.  No shipped save is in that Thursday/late
//!     month window when CE328 runs (the day handler recomputes var4/var6 each
//!     night), so pc986/CE402 is not reachable from any shipped save.
//!   * CE402's only caller is CE328 pc986 (asserted statically).  Directly driving
//!     CE402 is the only reproducible way to reach CE410 - consistent with
//!     m11_hscene_real_gate.rs / m11_hscene_bookstore.rs.
//!
//! Net verdict: NO real-input path from a shipped save reaches CE402 / CE410; the
//! missing gate is the date-driven 就寝 (pc122) route plus the var18 >= 50 sleep
//! gate, neither of which the shipped saves present.  The topic branches are a
//! dead end that terminates at pc1023 (take-a-bath reset).

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

fn op210_callers(ce: &CommonEventData, target: i32) -> BTreeSet<(u32, u32)> {
    let mut out = BTreeSet::new();
    for ev in ce.events.iter() {
        for (pc, slot) in ev.commands.iter().enumerate() {
            if let Slot::Command(c) = slot {
                if c.opcode == 210 && words(c).first().copied() == Some(target) {
                    out.insert((ev.id, pc as u32));
                }
            }
        }
    }
    out
}

fn op121_writers_val(ce: &CommonEventData, target: i32, val: i32) -> BTreeSet<(u32, u32)> {
    let mut out = BTreeSet::new();
    for ev in ce.events.iter() {
        for (pc, slot) in ev.commands.iter().enumerate() {
            if let Slot::Command(c) = slot {
                if c.opcode == 121 {
                    let w = words(c);
                    if w.first().copied() == Some(target) && w.get(1).copied() == Some(val) {
                        out.insert((ev.id, pc as u32));
                    }
                }
            }
        }
    }
    out
}

struct Track {
    seen: BTreeSet<u32>,
    max_pc: BTreeMap<u32, usize>,
    ce328_pcs: BTreeSet<usize>,
    menus: Vec<(usize, usize, usize)>,
    warnings: u64,
}

/// Advance the given number of frames, answering CE328's choice menus by the
/// observed instance pc (the pc just after op102).  Answers not listed default 0.
fn drive(g: &mut Game, answers: &BTreeMap<usize, usize>, frames: usize) -> Track {
    let mut t = Track {
        seen: BTreeSet::new(),
        max_pc: BTreeMap::new(),
        ce328_pcs: BTreeSet::new(),
        menus: Vec::new(),
        warnings: 0,
    };
    let mut answered_regions: BTreeSet<usize> = BTreeSet::new();
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
                t.ce328_pcs.insert(pc);
            }
        }
        if let Some(ch) = &g.choices {
            if ch.frames >= 2 {
                let nopts = ch.options.len();
                let pc = g
                    .vm
                    .instances()
                    .iter()
                    .find(|i| i.event == 328)
                    .map(|i| i.pc as usize)
                    .unwrap_or(9999);
                let ans = answers.get(&pc).copied().unwrap_or(0);
                if answered_regions.insert(pc) {
                    t.menus.push((pc, nopts, ans));
                }
                let _ = g.reply(ChoiceReply::Index(ans));
            }
        }
    }
    t.warnings = g.stats.warnings;
    t
}

fn open_with(name: &str, seed: &[(usize, i32)]) -> Game {
    let mut g = Game::new(data_root()).unwrap();
    g.load_save(&save_path(name)).unwrap();
    for &(k, v) in seed {
        g.vm.vars.set(k, v);
    }
    g.auto_advance = true;
    g.auto_choice = false;
    g.auto_choice_delay = 2;
    g.set_choice_cursor(0);
    g.start_event(328);
    g
}

/// Test 1: the four topic branches each diverge to the bath reset (pc1023) and
/// never reach the H CG call pc986/CE402.  Requires the Friday+early-month window
/// (var6 == 5, var4 <= 3) to even open the topic menu.
#[test]
fn topic_menu_four_branches_diverge_to_bath_reset_not_h_cg() {
    let ce = common_events();
    let e = ce.events.iter().find(|e| e.id == 328).unwrap();
    assert_eq!(words(cmd_ref(e, 173))[0], 20, "topic menu declared 4 opts");
    assert_eq!(
        cmd_ref(e, 173).strings,
        vec![
            "梦想的话题".to_string(),
            "无关紧要的话题".to_string(),
            "色情的话题".to_string(),
            "小鸟飞飞！".to_string()
        ]
    );

    for topic in 0usize..4 {
        let mut g = open_with("SaveData01", &[(304, 0), (6, 5), (4, 3)]);
        let mut answers: BTreeMap<usize, usize> = BTreeMap::new();
        answers.insert(174, topic);
        answers.insert(173, topic);
        let t = drive(&mut g, &answers, 12000);

        assert_eq!(t.warnings, 0, "topic {topic}: warnings");
        let menu_pcs: Vec<usize> = t.menus.iter().map(|(p, _, _)| *p).collect();
        assert!(
            menu_pcs.contains(&174),
            "topic {topic}: expected topic menu at pc174, menus={:?}",
            t.menus
        );
        assert!(t.ce328_pcs.contains(&579), "topic {topic}: pc579 CG-select");
        assert!(t.ce328_pcs.contains(&661), "topic {topic}: pc661 yes/no");
        assert!(
            t.ce328_pcs.contains(&1023),
            "topic {topic}: expected pc1023 bath reset terminal"
        );
        assert!(!t.ce328_pcs.contains(&986), "topic {topic}: must not hit pc986");
        assert!(!t.seen.contains(&402), "topic {topic}: CE402 must not spawn");
        assert!(!t.seen.contains(&410), "topic {topic}: CE410 must not spawn");
        assert_eq!(
            t.max_pc.get(&328).copied().unwrap_or(0),
            1023,
            "topic {topic}: CE328 max pc should be the bath-reset end"
        );
    }
}

/// Test 2: the var304 >= 1 night-intimacy entry (real CE245/CE290 output) takes
/// the Hしたい body and STOPS at CE332 -> pc741 END; pc986/CE402 is never reached.
#[test]
fn var304_entry_short_circuits_to_ce332_and_never_h_cg() {
    let ce = common_events();
    let e = ce.events.iter().find(|e| e.id == 328).unwrap();
    assert_eq!(words(cmd_ref(e, 740))[0], 500332);
    assert_eq!(cmd_ref(e, 741).opcode, 172);
    for v304 in [1i32, 2] {
        let mut g = open_with("SaveData01", &[(304, v304)]);
        let t = drive(&mut g, &BTreeMap::new(), 12000);
        assert_eq!(t.warnings, 0, "v304={v304}: warnings");
        assert!(
            t.seen.contains(&332),
            "v304={v304}: CE332 must be called on the Hしたい branch"
        );
        assert!(
            !t.ce328_pcs.contains(&986),
            "v304={v304}: must NOT reach pc986"
        );
        assert!(!t.seen.contains(&402), "v304={v304}: no CE402");
        assert_eq!(
            t.max_pc.get(&328).copied().unwrap_or(0),
            741,
            "v304={v304}: CE328 ends at pc741 op172 after CE332"
        );
    }
}

/// Test 3: from EVERY shipped save, both the var304=0 and var304=1 entries never
/// reach pc986 / CE402 / CE410 (empirical gate).  The real day handler recomputes
/// the weekday/day-of-month so the 就寝 (pc122) route is never presented.
#[test]
fn no_shipped_save_reaches_the_ce402_call() {
    let saves = [
        "SaveData01", "SaveData02", "SaveData03", "SaveData04", "SaveData05", "SaveData06",
        "SaveData07", "SaveData08",
    ];
    for name in saves {
        for v304 in [0i32, 1] {
            let mut g = open_with(name, &[(304, v304)]);
            let t = drive(&mut g, &BTreeMap::new(), 12000);
            assert_eq!(t.warnings, 0, "{name} v304={v304}: warnings");
            assert!(
                !t.ce328_pcs.contains(&986),
                "{name} v304={v304}: must NOT reach pc986 (max_pc={:?})",
                t.max_pc.get(&328)
            );
            assert!(!t.seen.contains(&402), "{name} v304={v304}: no CE402");
            assert!(!t.seen.contains(&410), "{name} v304={v304}: no CE410");
        }
    }
}

/// Test 4: static pin of the exact gate graph that stands between CE328 and the
/// CE402 call.
#[test]
fn gate_graph_pins_the_only_route_to_the_ce402_call() {
    let ce = common_events();
    let e = ce.events.iter().find(|e| e.id == 328).unwrap();

    assert_eq!(
        op210_callers(&ce, 500402),
        [(328u32, 986u32)].into_iter().collect::<BTreeSet<_>>(),
        "CE402 must have exactly one caller: CE328 pc986"
    );
    assert_eq!(cmd_ref(e, 986).opcode, 210);
    assert_eq!(words(cmd_ref(e, 986))[0], 500402);

    assert_eq!(cmd_ref(e, 862).opcode, 212);
    assert_eq!(cmd_ref(e, 862).strings, vec!["悪戯へ".to_string()]);
    assert_eq!(cmd_ref(e, 744).opcode, 212);
    assert_eq!(cmd_ref(e, 744).strings, vec!["就寝".to_string()]);
    assert_eq!(cmd_ref(e, 122).opcode, 213);
    assert_eq!(cmd_ref(e, 122).strings, vec!["就寝".to_string()]);
    assert_eq!(cmd_ref(e, 721).opcode, 213);
    assert_eq!(cmd_ref(e, 721).strings, vec!["就寝".to_string()]);
    assert_eq!(cmd_ref(e, 719).opcode, 499);
    assert_eq!(cmd_ref(e, 746).opcode, 111);
    assert_eq!(words(cmd_ref(e, 746)), vec![17, 2000018, 50, 1]);

    assert_eq!(cmd_ref(e, 102).opcode, 111);
    assert_eq!(words(cmd_ref(e, 102))[1..3], [2000006, 5]);
    assert_eq!(cmd_ref(e, 104).opcode, 111);
    assert_eq!(words(cmd_ref(e, 104))[1..3], [2000004, 3]);
    assert_eq!(cmd_ref(e, 106).opcode, 213);
    assert_eq!(cmd_ref(e, 106).strings, vec!["家族会議".to_string()]);
    assert_eq!(cmd_ref(e, 111).opcode, 111);
    assert_eq!(words(cmd_ref(e, 111))[1..3], [2000006, 4]);
    assert_eq!(cmd_ref(e, 115).opcode, 111);
    assert_eq!(words(cmd_ref(e, 115))[1..3], [2000004, 20]);

    assert_eq!(cmd_ref(e, 15).opcode, 111);
    assert_eq!(words(cmd_ref(e, 15))[1..3], [2000304, 1]);

    let ce332 = ce.events.iter().find(|e| e.id == 332).unwrap();
    assert_eq!(cmd_ref(ce332, 125).opcode, 121);
    assert_eq!(words(cmd_ref(ce332, 125))[0..2], [2000304, 0]);
    let ce332_calls = op210_callers(&ce, 500332);
    assert!(ce332_calls.contains(&(328, 711)), "CE332 called at CE328 pc711");
    assert!(ce332_calls.contains(&(328, 740)), "CE332 called at CE328 pc740");

    let v304_one = op121_writers_val(&ce, 2000304, 1);
    assert_eq!(
        v304_one,
        [(245u32, 779u32), (290u32, 403u32)]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
}
