//! M7 W1 battle support: CE190 target generation and CE239 / 2000350 scheduler probe.
//!
//! Test-only investigation. These tests pin the retail data facts behind the
//! battle target window and the 2000350 state machine so later wiring work has
//! an executable contract. No production code is modified here.
//!
//! Key measured facts encoded below:
//!  * CE190 (戦闘キャラ配置) reads party count from CData type18 row4 and enemy
//!    count / enemy group from CData type18 rows 69/72 (labels 基本システム用変数).
//!  * The production battle seed now includes type18 row69 = 1 (enemy count), so
//!    CE190's enemy branch lays out one enemy and CE162 (敵対象選択実行) terminates
//!    through pc57 with enemy index 0 selected. Clearing just that cell reproduces
//!    the pc49/pc50 spin that a fresh game used to show.
//!  * Variable 2000350 (magic 2000350 -> plain variable index 350) has exactly
//!    one writer of value 1: CE242 pc14. No command anywhere calls CE242
//!    (no op210 target 500242). CE239 only writes 0 (pc98/pc1043). Therefore
//!    the natural CE239 route never leaves 0; injecting CE242 directly shows the
//!    0 -> 1 -> 0 -> 2 -> 0 cycle and CE244 win settlement (2000352=1, 2000343=1).

use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_game::db::Container;
use wolf_game::key;
use wolf_game::Game;

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        return p;
    }
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    base.join(p)
}

fn event_data() -> CommonEventData {
    let bytes = std::fs::read(data_root().join("BasicData/CommonEvent.dat")).expect("CommonEvent.dat");
    CommonEventData::parse(&bytes).expect("parse CommonEvent.dat")
}

fn words(params: &[u8]) -> Vec<i32> {
    params
        .chunks_exact(4)
        .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

fn var(g: &Game, index: usize) -> i32 {
    g.vm.vars.get(index)
}

fn cdata(g: &Game, db_type: i32, row: i32, column: i32) -> i32 {
    g.databases.num(Container::CData, db_type, row, column).unwrap_or(-9999)
}

fn instance_pc(g: &Game, event: &str) -> Option<i32> {
    for i in g.vm.instances() {
        if format!("{:?}", i.event) == event {
            return Some(i.pc as i32);
        }
    }
    None
}

fn has_instance(g: &Game, event: &str) -> bool {
    g.vm.instances().iter().any(|i| format!("{:?}", i.event) == event)
}

fn seeded_game() -> Game {
    let mut g = Game::new(data_root()).expect("Game::new");
    g.seed_battle_databases();
    g.auto_advance = true;
    g.auto_choice = true;
    g.auto_choice_delay = 1;
    g
}

/// Static proof: CE190's enemy branch reads the type18 rows that the shipped
/// battle seed does not populate.
#[test]
fn ce190_reads_type18_rows_4_69_72_and_production_seed_includes_enemy_count() {
    let d = event_data();
    let ce190 = d.event(190).expect("CE190");
    assert!(ce190.title.contains("戦闘キャラ配置"), "CE190 title={}", ce190.title);

    let mut row4 = false;
    let mut row69 = false;
    let mut row72 = false;
    let mut call_enemy_copy = false;
    let mut call_enemy_layout = false;
    for slot in &ce190.commands {
        if let Slot::Command(c) = slot {
            if c.opcode == 250 {
                let w = words(&c.params);
                if w.len() >= 5 && w[0] == 20 {
                    match w[1] {
                        4 => row4 = true,
                        69 => row69 = true,
                        72 => row72 = true,
                        _ => {}
                    }
                }
            }
            if c.opcode == 300 {
                for s in &c.strings {
                    if s.contains("敵DB→戦闘DBコピー") {
                        call_enemy_copy = true;
                    }
                    if s.contains("敵キャラ_座標算出") {
                        call_enemy_layout = true;
                    }
                }
            }
        }
    }
    assert!(row4, "CE190 must read type18 row4 (party count)");
    assert!(row69, "CE190 must read type18 row69 (enemy count)");
    assert!(row72, "CE190 must read type18 row72 (enemy group id)");
    assert!(call_enemy_copy, "CE190 must call CE144 enemy DB copy");
    assert!(call_enemy_layout, "CE190 must call CE175 enemy layout");

    let probe = seeded_game();
    assert_eq!(cdata(&probe, 18, 4, 0), 1, "shipped seed sets type18 row4 col0 = 1 (party count)");
    assert_eq!(cdata(&probe, 18, 69, 0), 1, "production seed includes type18 row69 (enemy count)");
}

/// Runtime proof: clearing the production seed's type18 row69 makes CE162 spin at
/// pc49/pc50 forever; row69=1 lets CE162 pick enemy index 0 and exit through pc57.
#[test]
fn enemy_count_seed_turns_the_ce162_spin_into_a_selectable_enemy() {
    // Control: production seed with only the enemy-count cell cleared.
    let mut control = seeded_game();
    control.databases.set_num(Container::CData, 18, 69, 0, 0);
    assert_eq!(cdata(&control, 18, 69, 0), 0, "control starts with enemy count 0");
    control.start_event(190);
    for f in 0..400 {
        control.key_bits = if f % 2 == 0 { key::OK } else { 0 };
        control.frame();
    }
    assert_eq!(cdata(&control, 18, 69, 0), 0, "CE190 cannot invent an enemy count");
    assert!(cdata(&control, 10, 0, 2) > 0, "CE144 still lands one enemy row in type10");

    control.start_event(162);
    let mut control_selected = false;
    for f in 0..240 {
        control.key_bits = if f % 2 == 0 { key::OK } else { 0 };
        control.frame();
        if instance_pc(&control, "162") == Some(57) {
            control_selected = true;
        }
    }
    assert!(!control_selected, "CE162 must not reach pc57 without an enemy count");
    assert!(
        has_instance(&control, "162"),
        "CE162 remains active (spinning pc49/pc50) without a target"
    );
    let pc = instance_pc(&control, "162").expect("CE162 still running");
    assert!(pc == 49 || pc == 50, "control CE162 stuck at pc49/pc50, got pc{pc}");
    assert_eq!(control.stats.warnings, 0);

    // Seeded: add ONLY CData type18 row69 = 1.
    let mut seeded = seeded_game();
    seeded.databases.set_num(Container::CData, 18, 69, 0, 1);
    seeded.start_event(190);
    for f in 0..600 {
        seeded.key_bits = if f % 2 == 0 { key::OK } else { 0 };
        seeded.frame();
    }
    assert_eq!(cdata(&seeded, 18, 69, 0), 1, "enemy count survives CE190");
    assert!(cdata(&seeded, 10, 0, 2) > 0, "enemy row in type10 is alive");

    seeded.start_event(162);
    let mut selected = false;
    for f in 0..300 {
        seeded.key_bits = if f % 2 == 0 { key::OK } else { 0 };
        seeded.frame();
        if instance_pc(&seeded, "162") == Some(57) {
            selected = true;
        }
    }
    assert!(selected, "CE162 must reach pc57 (enemy index 0 selected) once row69=1");
    assert!(!has_instance(&seeded, "162"), "CE162 exits after a successful selection");
    assert_eq!(seeded.stats.warnings, 0);
}

/// Static proof of the 2000350 bootstrap: exactly one writer of 1 (CE242 pc14),
/// no caller of CE242, CE239 writes only 0.
#[test]
fn only_ce242_writes_2000350_one_and_nothing_calls_ce242() {
    let d = event_data();
    let mut writers_one = Vec::new();
    let mut writer_zero_ce239 = Vec::new();
    let mut callers_of_242 = Vec::new();
    for id in 0u32..1200 {
        if let Some(ev) = d.event(id) {
            for (pc, slot) in ev.commands.iter().enumerate() {
                if let Slot::Command(c) = slot {
                    let w = words(&c.params);
                    if c.opcode == 121 && w.len() >= 2 && w[0] == 2000350 {
                        if w[1] == 1 {
                            writers_one.push((id, pc));
                        }
                        if id == 239 && w[1] == 0 {
                            writer_zero_ce239.push(pc);
                        }
                    }
                    if c.opcode == 210 && !w.is_empty() && w[0] == 500242 {
                        callers_of_242.push((id, pc));
                    }
                }
            }
        }
    }
    assert_eq!(writers_one, vec![(242, 14)], "only CE242 pc14 writes 2000350=1");
    assert!(callers_of_242.is_empty(), "no retail command calls CE242: {callers_of_242:?}");
    assert!(
        writer_zero_ce239.contains(&98) && writer_zero_ce239.contains(&1043),
        "CE239 clears 2000350 at pc98 and pc1043, got {writer_zero_ce239:?}"
    );
}

fn ce239_slot_probe(slot: i32) -> (i32, i32, i32, i32) {
    let mut g = seeded_game();
    g.start_engine_events();
    g.vm.vars.set(495, 2); // 2000495 = 2 -> hotspring branch
    g.start_event(239);
    let instance = g
        .vm
        .instances()
        .into_iter()
        .find(|i| format!("{:?}", i.event) == "239")
        .map(|i| i.id);
    if let Some(id) = instance {
        if let Some(inst) = g.vm.instance_mut(id) {
            inst.self_vars.set(2, slot);
        }
    }
    let mut max350 = 0;
    for f in 0..250 {
        g.key_bits = if f % 2 == 0 { key::OK } else { 0 };
        g.frame();
        max350 = max350.max(var(&g, 350));
    }
    (max350, var(&g, 350), var(&g, 352), var(&g, 343))
}

/// Natural CE239 route: across every self1600002 slot (0..=8) under the
/// hotspring branch, 2000350 never leaves 0 and 2000352/2000343 stay 0, so the
/// CE244 settlement never fires.
#[test]
fn ce239_slots_never_raise_2000350() {
    for slot in 0..=8 {
        let (max350, final350, v352, v343) = ce239_slot_probe(slot);
        assert_eq!(max350, 0, "slot{slot}: CE239 must never raise 2000350, saw {max350}");
        assert_eq!(final350, 0, "slot{slot}: final 2000350");
        assert_eq!(v352, 0, "slot{slot}: CE244 settlement variable 2000352 must stay 0");
        assert_eq!(v343, 0, "slot{slot}: CE244 completion variable 2000343 must stay 0");
    }
}

/// Direct injection of CE242 (the only 0->1 edge) shows the full cycle is
/// observable when the missing caller exists: 0 -> 1 -> 0 -> 2 -> 0, with
/// CE244 win settlement setting 2000352=1 and 2000343=1.
#[test]
fn injected_ce242_exposes_the_three_state_cycle() {
    let mut g = seeded_game();
    g.vm.vars.set(495, 2); // 2000495 = 2
    g.start_event(242);
    let id = g
        .vm
        .instances()
        .into_iter()
        .find(|i| format!("{:?}", i.event) == "242")
        .map(|i| i.id)
        .expect("CE242 instance");
    g.vm.instance_mut(id).unwrap().self_vars.set(600, 1);

    let mut transitions: Vec<(i32, i32)> = vec![(0, var(&g, 350))];
    let mut prev = var(&g, 350);
    let mut max242 = 0;
    for f in 0..200 {
        g.key_bits = if f % 2 == 0 { key::OK } else { 0 };
        g.frame();
        let v = var(&g, 350);
        if v != prev {
            transitions.push((f, v));
            prev = v;
        }
        if has_instance(&g, "242") {
            max242 = max242.max(instance_pc(&g, "242").unwrap_or(0));
        }
    }

    assert_eq!(transitions.first().map(|x| x.1), Some(0), "cycle starts at 0");
    assert!(transitions.iter().any(|x| x.1 == 1), "cycle must expose state 1: {transitions:?}");
    assert!(transitions.iter().any(|x| x.1 == 2), "cycle must expose state 2: {transitions:?}");
    assert_eq!(var(&g, 350), 0, "cycle settles back to 0");
    assert_eq!(var(&g, 352), 1, "CE244 win path writes 2000352=1");
    assert_eq!(var(&g, 343), 1, "CE244 win path writes 2000343=1");
    assert!(max242 >= 208, "CE242 reached its 2000350=2 exit at pc208, maxpc={max242}");
    assert_eq!(g.stats.warnings, 0);
}
/// Rebuilt wiring path (battle_wiring.rs, off by default): with
/// Game::set_battle_wiring(true), CE239 raising 2000340 0->1 starts CE242 and
/// 2000350 advances 0 -> 1 -> 2. It still does not settle: CE239 leaves
/// 2000349 = 1 while CE244 pc36 needs 2000349 == self1600010 (11 for a command
/// whose 2000354 is not 20/7/13), so CE244 skips its win/lose/draw arms and
/// 2000352/2000343 stay 0.
#[test]
fn battle_wiring_advances_to_phase_two_but_ce244_gate_blocks_settlement() {
    // Static gate: CE244 pc36 is 111 [1, 2000349, 1600010, 2].
    let d = event_data();
    let ce244 = d.event(244).expect("CE244");
    let mut gate = None;
    for slot in &ce244.commands {
        if let Slot::Command(c) = slot {
            if c.opcode == 111 {
                let w = words(&c.params);
                if w.len() >= 3 && w[1] == 2000349 {
                    gate = Some(w[2]);
                }
            }
        }
    }
    assert_eq!(gate, Some(1600010), "CE244 settlement gate is 2000349 == self1600010");

    let mut g = seeded_game();
    g.set_battle_wiring(true);
    g.start_engine_events();
    g.vm.vars.set(495, 2); // 2000495 = 2 hotspring branch
    g.start_event(239);
    // Supply the command argument a real caller would pass (self1600003 -> 2000354).
    if let Some(id) = g
        .vm
        .instances()
        .into_iter()
        .find(|i| format!("{:?}", i.event) == "239")
        .map(|i| i.id)
    {
        g.vm.instance_mut(id).unwrap().self_vars.set(603, 1);
    }
    let mut transitions: Vec<i32> = vec![var(&g, 350)];
    let mut prev = var(&g, 350);
    let mut max340 = 0;
    for f in 0..1500 {
        g.key_bits = if f % 2 == 0 { key::OK } else { 0 };
        g.frame();
        let v = var(&g, 350);
        if v != prev {
            transitions.push(v);
            prev = v;
        }
        max340 = max340.max(var(&g, 340));
    }
    assert_eq!(g.battle_wiring_rounds(), 1, "wiring must start CE242 once on the 0->1 edge");
    assert_eq!(max340, 1, "CE239 raises 2000340");
    assert!(transitions.contains(&1), "phase 1 observed: {transitions:?}");
    assert!(transitions.contains(&2), "phase 2 observed: {transitions:?}");
    assert_eq!(var(&g, 350), 2, "command phase advances to 2 and then cannot leave");
    assert_eq!(var(&g, 352), 0, "CE244 settlement never runs");
    assert_eq!(var(&g, 343), 0, "CE244 decided flag never runs");
    assert_eq!(var(&g, 349), 1, "CE239 leaves 2000349 = 1");
}