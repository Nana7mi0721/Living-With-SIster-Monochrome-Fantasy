//! M13 P0-C — H-branch end-to-end verification (CE310 / CE323 / CE326 / CE335).
//!
//! Companion to docs/research/M13_H_ENDTOEND.md and the threshold matrix in
//! docs/research/M13_CONTENT_MATRIX.md §2.  task-8 (m13_h_thresholds.rs) already pins the
//! *byte-level* H-gate thresholds; this file drives the actual H branches end-to-end and
//! documents, test by test, what is reachable naturally vs what needs a state fixture.
//!
//! Tests:
//!   1. ce310_weekend_h_endtoend    — the 周末 "想要色色" branch: three deterministic
//!      outcomes (H consummated / too tired / not in the mood) driven from a fresh VM.
//!   2. ce323_toilet_voyeur_e2e     — the 厕所 (CE323) 偷袭 route driven to its H/witness
//!      scene; the full 插入 branch is documented as fixture-gated (see .md §3).
//!   3. ce323_breakin_gate_conformance — decoded gate bytes for the 闯入 route @1055-1062
//!      and live first-true boundary flip for 信頼(var21) at 200 vs 199.
//!   4. ce326_restaurant_gate_gap   — the 餐厅/台所 H gate (CE326 @801-805) actually reads
//!      var424>=3 ∧ var18>=100 ∧ var16>=1 and has NO 妹性欲>=60 precondition: this proves
//!      the guide-vs-script gap recorded in M13_CONTENT_MATRIX.md.
//!   5. ce335_night_renderer        — CE335 is the 深夜 CG/expression renderer + tutorial,
//!      not a trigger; drive it and confirm it renders the tips and raises no warnings.
//!
//! Run:  WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m13_h_endtoend

use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_game::Game;
use wolf_vm::exec::ChoiceReply;

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn data_root() -> PathBuf {
    match std::env::var("WOLF_DATA_ROOT") {
        Ok(v) if !v.is_empty() => repo_root().join(v),
        _ => repo_root().join("work").join("all"),
    }
}

fn common_events() -> CommonEventData {
    let bytes = std::fs::read(data_root().join("BasicData").join("CommonEvent.dat"))
        .expect("BasicData/CommonEvent.dat");
    CommonEventData::parse(&bytes).expect("parse CommonEvent.dat")
}

fn words(cmd: &wolf_format::command::Command) -> Vec<i32> {
    cmd.params
        .chunks_exact(4)
        .map(|w| i32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        .collect()
}

// opcode-111 operator semantics, identical to crates/wolf-vm/src/exec.rs:1493 compare().
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

// (variable id without the 2_000_000 base, value, operator) triples of one op111 payload.
fn decode_conditions(ints: &[i32]) -> Vec<(usize, i32, i32)> {
    let count = (ints[0] & 0x0F) as usize;
    let mut out = Vec::with_capacity(count);
    for k in 0..count {
        let b = 1 + 3 * k;
        let var = ints[b] as usize - 2_000_000;
        out.push((var, ints[b + 1], ints[b + 2]));
    }
    out
}

fn selected_case(ints: &[i32], var_of: impl Fn(usize) -> i32) -> Option<usize> {
    decode_conditions(ints)
        .iter()
        .position(|&(var, val, op)| compare(var_of(var), val, op))
}

fn op111_ints(data: &CommonEventData, event: u32, pc: usize) -> Vec<i32> {
    let ev = data
        .event(event)
        .unwrap_or_else(|| panic!("CE{event} exists"));
    match ev.commands.get(pc) {
        Some(Slot::Command(cmd)) => {
            assert_eq!(
                cmd.opcode, 111,
                "CE{event}@{pc} must be opcode 111 (VariableCondition)"
            );
            let ints = words(cmd);
            assert_eq!(
                ints.len(),
                1 + 3 * ((ints[0] & 0x0F) as usize),
                "CE{event}@{pc} op111 payload length must be 1 + 3*cond_count"
            );
            ints
        }
        _ => panic!("CE{event}@{pc} is missing or blank"),
    }
}

fn new_game() -> Game {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.vm.clear_common_self();
    game.auto_advance = true;
    game.auto_choice = false;
    game.set_choice_cursor(0);
    game
}

fn seed(game: &mut Game, pairs: &[(usize, i32)]) {
    for &(var, value) in pairs {
        game.vm.vars.set(var, value);
    }
}

fn note_message(msgs: &mut Vec<String>, game: &Game) {
    if let Some(msg) = game.messages.last() {
        let text = msg.text.clone();
        if msgs.last().map(|t| *t != text).unwrap_or(true) {
            msgs.push(text);
        }
    }
}

fn track_spawned(spawned: &mut std::collections::BTreeSet<u32>, game: &Game) {
    for inst in game.vm.instances() {
        spawned.insert(inst.event);
    }
}

// ----------------------------------------------------------------------------------------
// Test 1 — 周末 "想要色色" H branch (CE310 inline body).  Three deterministic outcomes.
// ----------------------------------------------------------------------------------------

fn drive_ce310_h(
    seed_pairs: &[(usize, i32)],
) -> (Vec<String>, std::collections::BTreeSet<u32>, Game) {
    let mut game = new_game();
    seed(&mut game, seed_pairs);
    game.start_event(310);
    let mut msgs = Vec::new();
    let mut spawned = std::collections::BTreeSet::new();

    // Frame 1: push OK until the "想要色色" picture button is drawn on the weekend menu.
    let mut button: Option<(i32, i32)> = None;
    for frame in 0..3000 {
        game.key_bits = if frame % 2 == 0 {
            wolf_game::key::OK
        } else {
            0
        };
        game.frame();
        note_message(&mut msgs, &game);
        track_spawned(&mut spawned, &game);
        // The button text picture lives near id 1211 (text layer of pic base 1210) at y=450.
        for id in 1200..1230 {
            if let Some(picture) = game.screen.get(id) {
                if picture.text.contains("想要色色") {
                    button = Some((picture.x, picture.y));
                    break;
                }
            }
        }
        if button.is_some() {
            break;
        }
    }
    let (x, y) = button.expect("CE310 must draw the 想要色色 button when the H gate passes");
    game.key_bits = 0;
    game.mouse = [x, y];
    for _ in 0..10 {
        game.frame();
        note_message(&mut msgs, &game);
        track_spawned(&mut spawned, &game);
    }
    game.mouse_buttons = 1;
    for _ in 0..5 {
        game.frame();
        note_message(&mut msgs, &game);
        track_spawned(&mut spawned, &game);
    }
    game.mouse_buttons = 0;
    game.mouse = [-1, -1];

    // Frame 2: advance the inline H body to the end.
    for frame in 0..5000 {
        game.key_bits = if frame % 2 == 0 {
            wolf_game::key::OK
        } else {
            0
        };
        game.frame();
        note_message(&mut msgs, &game);
        track_spawned(&mut spawned, &game);
        if game.vm.instances().iter().all(|inst| inst.event != 310) {
            break;
        }
    }
    (msgs, spawned, game)
}

#[test]
fn ce310_weekend_h_endtoend() {
    // A — H consummated: 精力16≥1, 性興味22≥200, 性欲18≥40, 挿入経験424≥3, 発情度102≥40,
    //     行動力14≥20, not sick (var19=0).
    let a_seed = &[
        (16, 5),
        (18, 50),
        (22, 300),
        (424, 5),
        (14, 30),
        (102, 100),
        (63, 1),
    ];
    let (msgs, spawned, game) = drive_ce310_h(a_seed);
    assert!(
        msgs.iter().any(|m| m.contains("可以哦")),
        "A: expected 同意 H, got {msgs:?}"
    );
    assert!(
        msgs.iter().any(|m| m.contains("一大早就开始和妹妹色色")),
        "A: expected the consummation line, got {msgs:?}"
    );
    assert_eq!(
        game.vm.vars.get(150),
        1,
        "A: H consummation flag var150 must be set"
    );
    assert_eq!(game.vm.vars.get(424), 6, "A: 挿入経験 var424 must +1");
    assert!(spawned.contains(&226), "A: 行動力変動 CE226 must run");
    assert_eq!(game.stats.warnings, 0, "A: no VM warnings");

    // B — too tired: 行動力14 < 20.
    let b_seed = &[
        (16, 5),
        (18, 50),
        (22, 300),
        (424, 5),
        (14, 10),
        (102, 100),
        (63, 1),
    ];
    let (msgs, _, game) = drive_ce310_h(b_seed);
    assert!(
        msgs.iter().any(|m| m.contains("确实有些累了呢")),
        "B: expected the tired line, got {msgs:?}"
    );
    assert_eq!(game.vm.vars.get(150), 0, "B: no consummation");
    assert_eq!(game.stats.warnings, 0, "B: no VM warnings");

    // C — not in the mood: 発情度102 <= 39.
    let c_seed = &[
        (16, 5),
        (18, 50),
        (22, 300),
        (424, 5),
        (14, 30),
        (102, 39),
        (63, 1),
    ];
    let (msgs, _, game) = drive_ce310_h(c_seed);
    assert!(
        msgs.iter().any(|m| m.contains("抱歉哦，今天没那个心情呢")),
        "C: expected the no-mood line, got {msgs:?}"
    );
    assert_eq!(game.vm.vars.get(150), 0, "C: no consummation");
    assert_eq!(game.stats.warnings, 0, "C: no VM warnings");
}

// ----------------------------------------------------------------------------------------
// Test 2 — 厕所 (CE323) 偷袭 route end-to-end.
// ----------------------------------------------------------------------------------------

fn drive_ce323(
    seed_pairs: &[(usize, i32)],
) -> (Vec<String>, std::collections::BTreeSet<u32>, Game) {
    let mut game = new_game();
    seed(&mut game, seed_pairs);
    game.start_event(323);
    let mut msgs = Vec::new();
    let mut spawned = std::collections::BTreeSet::new();
    for frame in 0..20000 {
        game.key_bits = if frame % 2 == 0 {
            wolf_game::key::OK
        } else {
            0
        };
        game.frame();
        note_message(&mut msgs, &game);
        track_spawned(&mut spawned, &game);
        // CE323's reachable route choices are all idx0 (偷窥 / 偷窥 / 是 / 闯入).
        if game.choices.is_some() {
            game.reply(ChoiceReply::Index(0));
        }
        if frame > 60 && game.vm.instances().iter().all(|inst| inst.event != 323) {
            break;
        }
    }
    (msgs, spawned, game)
}

#[test]
fn ce323_toilet_voyeur_e2e() {
    // Seed the onani-witness branch: 欲求不満 var103>=1 gates entry, おしっこ観察 var221>=3
    // gates the 闯入 option; the other values keep the trust/機嫌/性興味/発情度 gates true.
    let seed_pairs = &[
        (16, 10),
        (21, 300),
        (18, 50),
        (22, 300),
        (102, 100),
        (23, 6),
        (103, 2),
        (221, 3),
    ];
    let (msgs, spawned, game) = drive_ce323(seed_pairs);
    assert!(
        msgs.iter().any(|m| m.contains("就是现在，偷袭！")),
        "CE323: expected the 偷袭 line, got {msgs:?}"
    );
    assert!(
        msgs.iter().any(|m| m.contains("被轰出去了")),
        "CE323: expected the voyeur-witness outcome, got {msgs:?}"
    );
    // The witness scene fires the H CGI/expression helper events (手淫 CE376, トイレ瞬き CE375,
    // トイレCG CE374, plus 深夜1吐息 CE355/356) even though it ends with the girl kicking him out.
    assert!(
        spawned.contains(&376),
        "CE323: 手淫 CE376 must run, spawned={spawned:?}"
    );
    assert!(
        spawned.contains(&355) || spawned.contains(&356),
        "CE323: 深夜吐息 helpers must run"
    );
    assert_eq!(
        game.vm.vars.get(221),
        4,
        "CE323: おしっこ観察 var221 increments on the witnessed scene"
    );
    assert_eq!(game.stats.warnings, 0, "CE323: no VM warnings");
    // NOTE (documented in M13_H_ENDTOEND.md §3): the full 插入 branch (CE505 ■妹トイレH前,
    // 用嘴做 / 插入 choices at pc279/415) is NOT reached by this voyeur route — it needs a
    // state fixture (self-state routing at CE323 @1050 break-in + deeper trust/機嫌 chain).
}

// ----------------------------------------------------------------------------------------
// Test 3 — 厕所 闯入 gate bytecode + live boundary semantics.
// ----------------------------------------------------------------------------------------

#[test]
fn ce323_breakin_gate_conformance() {
    let data = common_events();
    assert_eq!(
        op111_ints(&data, 323, 1055),
        vec![2, 2_000_021, 200, 1, 2_000_021, 0, 1],
        "@1055 信頼 var21>=200 / fallback >=0"
    );
    assert_eq!(
        op111_ints(&data, 323, 1058),
        vec![2, 2_000_023, 5, 1, 2_000_023, 0, 1],
        "@1058 機嫌 var23>=5"
    );
    assert_eq!(
        op111_ints(&data, 323, 1060),
        vec![3, 2_000_022, 250, 1, 2_000_022, 150, 1, 2_000_022, 0, 1],
        "@1060 性興味 var22 tiers 250/150/0"
    );
    assert_eq!(
        op111_ints(&data, 323, 1062),
        vec![2, 2_000_222, 0, 2, 2_000_222, 1, 1],
        "@1062 おしっこ観察 var222 ==0 / >=1"
    );

    // First-true boundary: 信頼 var21=200 -> tier 0; 199 -> fallback tier 1.
    let p1055 = op111_ints(&data, 323, 1055);
    assert_eq!(
        selected_case(&p1055, |v| if v == 21 { 200 } else { 0 }),
        Some(0)
    );
    assert_eq!(
        selected_case(&p1055, |v| if v == 21 { 199 } else { 0 }),
        Some(1)
    );

    // 性興味 var22 tiers flip at each documented threshold.
    let p1060 = op111_ints(&data, 323, 1060);
    assert_eq!(
        selected_case(&p1060, |v| if v == 22 { 250 } else { 0 }),
        Some(0)
    );
    assert_eq!(
        selected_case(&p1060, |v| if v == 22 { 150 } else { 0 }),
        Some(1)
    );
    assert_eq!(selected_case(&p1060, |_| 0), Some(2));
}

// ----------------------------------------------------------------------------------------
// Test 4 — 餐厅/台所 H gate: prove the guide-vs-script gap (no 妹性欲>=60 precondition).
// ----------------------------------------------------------------------------------------

#[test]
fn ce326_restaurant_gate_gap() {
    let data = common_events();
    // @799 comment "DLC追加台所えっち"; the pre-entry gate is three single conditions.
    assert_eq!(
        op111_ints(&data, 326, 801),
        vec![1, 2_000_424, 3, 1],
        "@801 挿入経験 var424>=3"
    );
    assert_eq!(
        op111_ints(&data, 326, 803),
        vec![1, 2_000_018, 100, 1],
        "@803 自己性欲 var18>=100"
    );
    assert_eq!(
        op111_ints(&data, 326, 805),
        vec![1, 2_000_016, 1, 1],
        "@805 精力 var16>=1"
    );
    // @826 (after the choice) is the only 発情度 term and it is >=40, not a >=60 gate.
    assert_eq!(
        op111_ints(&data, 326, 826),
        vec![17, 2_000_102, 40, 1],
        "@826 発情度 var102>=40 (post-choice)"
    );

    // The guide threshold "自身性欲>=100 ∧ 妹性欲>=60" has NO 妹性欲>=60 precondition in the
    // script: scan every op111 in the entry window @799..@815 for a 22/102/103 >= 60 term.
    let event = data.event(326).expect("CE326 exists");
    let mut found_imouto_desire = false;
    for pc in 799..=815 {
        if let Some(Slot::Command(cmd)) = event.commands.get(pc) {
            if cmd.opcode == 111 {
                for (var, value, _op) in decode_conditions(&words(cmd)) {
                    if (var == 22 || var == 102 || var == 103) && value >= 60 {
                        found_imouto_desire = true;
                    }
                }
            }
        }
    }
    assert!(
        !found_imouto_desire,
        "CE326 entry must NOT contain a 妹性欲>=60 precondition (documented guide/script gap)"
    );
    eprintln!(
        "ce326_restaurant_gate_gap: entry gate = var424>=3 ∧ var18>=100 ∧ var16>=1; \
         only 発情度 term is post-choice var102>=40 — guide 妹性欲>=60 has no script counterpart"
    );
}

// ----------------------------------------------------------------------------------------
// Test 5 — CE335 is the 深夜 renderer/tutorial, not an H trigger.
// ----------------------------------------------------------------------------------------

#[test]
fn ce335_night_renderer() {
    let mut game = new_game();
    game.start_event(335);
    let mut msgs = Vec::new();
    for frame in 0..4000 {
        game.key_bits = if frame % 2 == 0 {
            wolf_game::key::OK
        } else {
            0
        };
        game.frame();
        note_message(&mut msgs, &game);
        if frame > 60 && game.vm.instances().iter().all(|inst| inst.event != 335) {
            break;
        }
    }
    assert!(
        msgs.iter()
            .any(|m| m.contains("Tips") && m.contains("深夜")),
        "CE335 must show the 深夜 tips tutorial, got {msgs:?}"
    );
    assert!(
        msgs.iter().any(|m| m.contains("深夜")),
        "CE335 must reference 深夜"
    );
    assert_eq!(game.stats.warnings, 0, "CE335: no VM warnings");
}
