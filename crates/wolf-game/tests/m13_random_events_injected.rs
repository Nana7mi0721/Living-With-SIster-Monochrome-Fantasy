//! M13 P2 — 17 condition-injection random events: inject gate state, drive, assert.
//!
//! Companion to docs/research/M13_RANDOM_EVENTS_INJECTED.md. The 17 entries are
//! exactly the "条件注入" tier of docs/research/M13_CONTENT_MATRIX.md §1.2
//! (16 DLC + 特4 seal). For each we record the gate variable + threshold,
//! inject the satisfying value(s), jump pc to the known block headline and
//! assert the expected line renders with stats.warnings == 0.
//!
//!   WOLF_DATA_ROOT=work/all cargo test -p wolf-game --test m13_random_events_injected -- --nocapture

use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_game::Game;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".to_string());
    let p = PathBuf::from(raw);
    if p.is_absolute() { p } else { repo_root().join(p) }
}

fn new_game() -> Game {
    Game::new(&data_root()).expect("Game::new")
}

fn carg(cmd: &wolf_format::command::Command, i: usize) -> i32 {
    let b = &cmd.params;
    let o = i * 4;
    i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn clean(t: &str) -> String {
    let t = t.trim();
    t.strip_prefix("\\E").unwrap_or(t).trim().to_string()
}

/// Decode the gate triples of an op111 conditional at a given pc.
fn cond_triples(program: &wolf_vm::Program, pc: usize) -> Option<Vec<(i32, i32, i32)>> {
    let Some(Slot::Command(cmd)) = program.slots.get(pc) else { return None };
    if cmd.opcode != 111 { return None; }
    let head = carg(cmd, 0);
    let count = (head & 0x0F) as usize;
    Some((0..count).map(|i| (carg(cmd, 1 + 3 * i), carg(cmd, 2 + 3 * i), carg(cmd, 3 + 3 * i))).collect())
}

struct Run {
    messages: Vec<String>,
    warnings: u64,
    ended: bool,
}

/// Drive one event: set self vars (flat slot), global vars, optionally override
/// entry pc, then advance dumping every message. Auto-choices are taken.
fn drive(
    game: &mut Game,
    ce: u32,
    selfs: &[(usize, i32)],
    globals: &[(i32, i32)],
    pc: Option<usize>,
    max_frames: usize,
) -> Run {
    let iid = game.vm.start_event(ce).unwrap_or_else(|| panic!("start_event CE{ce} returned None"));
    {
        let inst = game.vm.instance_mut(iid).expect("instance");
        for &(slot, val) in selfs { inst.self_vars.set(slot, val); }
        if let Some(pc) = pc { inst.pc = pc; }
    }
    for &(v, val) in globals { game.vm.vars.set(v as usize, val); }
    game.auto_advance = false;
    game.auto_choice = true;
    game.auto_choice_delay = 0;

    let mut messages: Vec<String> = Vec::new();
    let mut ended = false;
    for _ in 0..max_frames {
        game.key_bits = wolf_game::key::OK;
        game.frame();
        game.key_bits = 0;
        for m in &game.messages {
            let c = clean(&m.text);
            if !c.is_empty() && messages.last() != Some(&c) { messages.push(c); }
        }
        if game.vm.instance(iid).is_none() { ended = true; break; }
    }
    Run { messages, warnings: game.stats.warnings, ended }
}

fn assert_reached(label: &str, run: &Run, expected: &str) {
    let hit = run.messages.iter().any(|m| m.contains(expected));
    assert!(
        hit,
        "{label}: expected substring {expected:?} not rendered\n  warnings={} ended={} messages={:?}",
        run.warnings, run.ended, run.messages
    );
    assert_eq!(run.warnings, 0, "{label}: warnings while running; messages={:?}", run.messages);
}

struct Case {
    ce: u32,
    selfs: &'static [(usize, i32)],
    globals: &'static [(i32, i32)],
    pc: usize,
    expected: &'static str,
    label: &'static str,
    gate: &'static str,
}

/// The 16 DLC random events (#10/28/39/45/54/58/59/62/63/65/69/76/80/84/101/111).
/// Gate = DLC seed var619==1 plus (for selector-gated blocks) the self selector.
const CASES: &[Case] = &[
    // #10 紫桑的慰问  CE460@5 ユカリさん差し入れ : self#10==1 (@1), DLC
    Case { ce: 460, selfs: &[(610, 1)], globals: &[(619, 1)], pc: 5, expected: "慰劳品", label: "#10 紫桑的慰问", gate: "self#10==1 (@1); DLC var619" },
    // #28 和优衣一起探病 CE465@108 看病 : 由衣好感 var42>=10 (@97), var593==1 (@95), DLC
    Case { ce: 465, selfs: &[], globals: &[(42, 20), (593, 1), (619, 1)], pc: 104, expected: "由衣，你没事吧？", label: "#28 和优衣一起探病", gate: "var42(由衣好感)>=10 (@97), var593==1 (@95); DLC" },
    // #39 自己去卖胖次 CE452@581 EP3 : DLC
    Case { ce: 452, selfs: &[], globals: &[(619, 1)], pc: 581, expected: "向熊镇", label: "#39 自己去卖胖次", gate: "DLC var619 (EP3)" },
    // #45 撸猫 CE461@1081 看板猫 : self#10∈4..7 (@1079), self#20∈0..2 (@1082), DLC
    Case { ce: 461, selfs: &[(610, 4), (620, 0)], globals: &[(619, 1)], pc: 1081, expected: "小猫", label: "#45 撸猫", gate: "self#10 in 4..7 (@1079), self#20 in 0..2 (@1082); DLC" },
    // #54 吃烤曼德拉草 CE452@2983 EP20 : DLC
    Case { ce: 452, selfs: &[], globals: &[(619, 1)], pc: 2983, expected: "烤曼陀罗草", label: "#54 吃烤曼德拉草", gate: "DLC var619 (EP20)" },
    // #58 和加奈优衣外面吃饭 CE461@1355 買い出しの日 : DLC
    Case { ce: 461, selfs: &[], globals: &[(619, 1)], pc: 1355, expected: "东西基本买好了", label: "#58 和加奈优衣外面吃饭", gate: "DLC var619 (買い出し)" },
    // #59 被不良顾客缠上 CE461@1314 たちの悪い客 : self#10∈1..3 (@1320), DLC
    Case { ce: 461, selfs: &[(610, 1)], globals: &[(619, 1)], pc: 1314, expected: "被坏心眼的客人缠上了", label: "#59 被不良顾客缠上", gate: "self#10 in 1..3 (@1320); DLC" },
    // #62 遇紫桑换衣服 CE452@4750 EP29 : DLC
    Case { ce: 452, selfs: &[], globals: &[(619, 1)], pc: 4750, expected: "由香莉姐还在睡", label: "#62 遇紫桑换衣服", gate: "DLC var619 (EP29)" },
    // #63 和小霞出去转换心情 CE461@258 カスミ : var565==1 (@256), self#23∈0..2 (@259), DLC
    Case { ce: 461, selfs: &[(623, 0)], globals: &[(565, 1), (619, 1)], pc: 258, expected: "和香澄美一起寻找素材了", label: "#63 和小霞出去转换心情", gate: "var565==1 (@256), self#23 in 0..2 (@259); DLC" },
    // #65 抱优衣拿东西 CE461@43 ユイ : var573∈1..3 (@41), self#22∈0..2 (@44), DLC
    Case { ce: 461, selfs: &[(622, 0)], globals: &[(573, 1), (619, 1)], pc: 43, expected: "与由衣的亲密度上升了", label: "#65 抱优衣拿东西", gate: "var573 in 1..3 (@41), self#22 in 0..2 (@44); DLC" },
    // #69 和优衣去面包店 CE449@45 デススコーン討伐 : DLC
    Case { ce: 449, selfs: &[], globals: &[(619, 1)], pc: 45, expected: "源头很可能是", label: "#69 和优衣去面包店", gate: "DLC var619 (討伐前段)" },
    // #76 被说成恐怖组织 CE461@1355 低評判分支 @1409 : 評判 var11<30, DLC
    Case { ce: 461, selfs: &[], globals: &[(11, 10), (619, 1)], pc: 1409, expected: "用炸弹把委托人都炸飞了", label: "#76 被说成恐怖组织", gate: "評判 var11<30 (低評價分支 @1374 tiers); DLC" },
    // #80 和加奈一起跑步 CE461@460 カナ : self#21∈0..3 (@461), DLC
    Case { ce: 461, selfs: &[(621, 0)], globals: &[(619, 1)], pc: 460, expected: "和加奈一起锻炼了", label: "#80 和加奈一起跑步", gate: "self#21 in 0..3 (@461); DLC" },
    // #84 大家一起吸猫 CE461@1081 看板猫 (同 #45) : self#10∈4..7 (@1079), self#20∈0..2 (@1082), DLC
    Case { ce: 461, selfs: &[(610, 4), (620, 0)], globals: &[(619, 1)], pc: 1081, expected: "喵——", label: "#84 大家一起吸猫", gate: "self#10 in 4..7 (@1079), self#20 in 0..2 (@1082); DLC" },
    // #101 遇小霞狩猎目标 CE461@258 カスミ (同 #63) : var565==1 (@256), self#23∈0..2 (@259), DLC
    Case { ce: 461, selfs: &[(623, 0)], globals: &[(565, 1), (619, 1)], pc: 258, expected: "和香澄美一起寻找素材了", label: "#101 遇小霞狩猎目标", gate: "var565==1 (@256), self#23 in 0..2 (@259); DLC" },
    // #111 忙碌的一天 CE461@1433 今日は忙しかった : self#10∈8..10 (@1431), DLC
    Case { ce: 461, selfs: &[(610, 8)], globals: &[(619, 1)], pc: 1433, expected: "今天可真忙啊", label: "#111 忙碌的一天", gate: "self#10 in 8..10 (@1431); DLC" },
];

#[test]
fn injected_dlc_events_reach_expected_block() {
    for c in CASES {
        let mut game = new_game();
        let run = drive(&mut game, c.ce, c.selfs, c.globals, Some(c.pc), 1200);
        eprintln!("OK {:?} CE{} pc={} gate={}", c.label, c.ce, c.pc, c.gate);
        assert_reached(c.label, &run, c.expected);
    }
    assert_eq!(CASES.len(), 16, "expected 16 DLC cases");
}

// ---------------------------------------------------------------------------
// 特4 海豹清除事件 (gate var533 = 初见晓胜利). Two sub-blocks.
// ---------------------------------------------------------------------------
#[test]
fn special4_seal_clear_reachable() {
    // CE437 讨伐前会話 @22 -> @24; gate var535==0 (@10), self#11==0 (@15).
    let mut g1 = new_game();
    let r1 = drive(&mut g1, 437, &[(611, 0)], &[(533, 1), (535, 0)], Some(22), 600);
    assert_reached("特4 CE437 アザラシ討伐", &r1, "海豹驱除作战");

    // CE438 アザラシイベント エネミー @82 -> @85; gate var449>=1 (@32).
    let mut g2 = new_game();
    let r2 = drive(&mut g2, 438, &[], &[(533, 1), (449, 1)], Some(82), 600);
    assert_reached("特4 CE438 アザラシイベント", &r2, "海豹跃出水面");
}

// ---------------------------------------------------------------------------
// Static cross-check: the recorded gate triples are actually present at the
// cited pcs (so the injection values/thresholds correspond to real conditions).
// ---------------------------------------------------------------------------
#[test]
fn injected_gate_triples_present() {
    let game = new_game();
    let progs = &game.vm.programs;
    let check = |ce: u32, pc: usize, want: &[(i32, i32, i32)]| {
        let prog = progs.event(ce).unwrap_or_else(|| panic!("CE{ce} missing"));
        let got = cond_triples(prog, pc).unwrap_or_else(|| panic!("CE{ce}@{pc} not op111"));
        assert_eq!(got.as_slice(), want, "CE{ce}@{pc} gate triples differ");
    };
    // op: 0 >  1 >=  2 ==  3 <=  4 <  5 !=
    check(460, 1, &[(1_600_010, 1, 2)]);              // #10 self#10==1
    check(465, 97, &[(2_000_042, 10, 1), ]);
    check(465, 95, &[(2_000_593, 1, 2)]);
    check(461, 1079, &[(1_600_010, 4, 2), (1_600_010, 5, 2), (1_600_010, 6, 2), (1_600_010, 7, 2)]);
    check(461, 1082, &[(1_600_020, 0, 2), (1_600_020, 1, 2), (1_600_020, 2, 2)]);
    check(461, 1320, &[(1_600_010, 1, 2), (1_600_010, 2, 2), (1_600_010, 3, 2)]);
    check(461, 256, &[(2_000_565, 1, 2)]);
    check(461, 259, &[(1_600_023, 0, 2), (1_600_023, 1, 2), (1_600_023, 2, 2)]);
    check(461, 41, &[(2_000_573, 1, 2), (2_000_573, 2, 2), (2_000_573, 3, 2)]);
    check(461, 44, &[(1_600_022, 0, 2), (1_600_022, 1, 2), (1_600_022, 2, 2)]);
    check(461, 1374, &[(2_000_011, 90, 1), (2_000_011, 60, 1), (2_000_011, 30, 1), (2_000_011, 0, 1)]);
    check(461, 461, &[(1_600_021, 0, 2), (1_600_021, 1, 2), (1_600_021, 2, 2), (1_600_021, 3, 1)]);
    check(461, 1431, &[(1_600_010, 8, 2), (1_600_010, 9, 2), (1_600_010, 10, 2)]);
    check(437, 10, &[(2_000_535, 0, 2)]);
    check(438, 32, &[(2_000_449, 1, 1)]);
}
