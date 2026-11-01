//! M13 P2 — runtime reachability sampling for the 111 random events.
//!
//! Companion to docs/research/M13_RANDOM_EVENTS.md and
//! docs/research/M13_CONTENT_MATRIX.md. Run with:
//!
//!   WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m13_random_events
//!
//! Group A  : structural — every literal CommonEvent / CallByName target resolves.
//! Group B  : umbrella pools (CE277/278/279/280) dispatch into the expected event block.
//! Group C  : named day/dinner/DLC/seal blocks start and render their headline line.
//! Group D  : cross-check the +key gate triples quoted from M13_CONTENT_MATRIX.
//! Group E  : #[ignore] full sweep of every umbrella-pool branch.
//!
//! Every driven run also asserts stats.warnings == 0, so a single unknown opcode,
//! missing picture, or bad variable warns here first.

use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_game::Game;
use wolf_vm::Magic;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap()
        .to_path_buf()
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".to_string());
    let p = PathBuf::from(raw);
    if p.is_absolute() { p } else { repo_root().join(p) }
}

fn new_game() -> Game {
    Game::new(&data_root()).expect("Game::new")
}

/// Decode a command argument (little-endian i32 at param index i).
fn carg(cmd: &wolf_format::command::Command, i: usize) -> i32 {
    let b = &cmd.params;
    let o = i * 4;
    i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn clean(t: &str) -> String {
    let t = t.trim();
    t.strip_prefix("\\E").unwrap_or(t).trim().to_string()
}

struct Run {
    messages: Vec<String>,
    warnings: u64,
    ended: bool,
}

/// Drive one event. selfvars are (flat self-var slot, value) pairs; the common
/// selector self#n lives at flat slot 600+n (COMMON_SELF_BASE + SELF_ARG_BASE).
/// pc optionally overrides the entry pc to jump straight into a block.
fn drive(game: &mut Game, ce: u32, selfvars: &[(usize, i32)], pc: Option<usize>, max_frames: usize) -> Run {
    let iid = game.vm.start_event(ce)
        .unwrap_or_else(|| panic!("start_event CE{ce} returned None"));
    {
        let inst = game.vm.instance_mut(iid).expect("instance");
        for &(slot, val) in selfvars {
            inst.self_vars.set(slot, val);
        }
        if let Some(pc) = pc {
            inst.pc = pc;
        }
    }
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
            if !c.is_empty() && messages.last() != Some(&c) {
                messages.push(c);
            }
        }
        if game.vm.instance(iid).is_none() {
            ended = true;
            break;
        }
    }
    Run { messages, warnings: game.stats.warnings, ended }
}

fn assert_reached(label: &str, run: &Run, expected: &str) {
    let hit = run.messages.iter().any(|m| m.contains(expected));
    assert!(
        hit,
        "{label}: did not render expected substring {expected:?}\n  warnings={} ended={} messages={:?}",
        run.warnings, run.ended, run.messages
    );
    assert_eq!(run.warnings, 0, "{label}: warnings while running; messages={:?}", run.messages);
}

// ---------------------------------------------------------------------------
// Group A — structural: no live call target is missing.
// ---------------------------------------------------------------------------
// Known dormant data defects inside CE270 ('×┣[メッセ—ジ]表示処理', an unused
// '×' helper with no callers): its op300 calls the editor-only names
// 'ズ—ム'/'マップをズーム' (em-dash typo, no such event). Never executed in play,
// but recorded here so a regression elsewhere is still caught.
const DEAD_ALLOW: &[(u32, usize)] = &[
    (270, 631), (270, 637), (270, 705), (270, 711),
    (270, 2065), (270, 2071), (270, 2112), (270, 2119),
];

#[test]
fn all_call_targets_resolve() {
    let game = new_game();
    let progs = &game.vm.programs;
    let mut checked = 0usize;
    let mut dynamic = 0usize;
    let mut allowed = 0usize;
    let mut dead: Vec<String> = Vec::new();

    for id in 0u32..2000 {
        let Some(prog) = progs.event(id) else { continue };
        for (pc, slot) in prog.slots.iter().enumerate() {
            let Slot::Command(cmd) = slot else { continue };
            match cmd.opcode {
                210 => {
                    let raw = carg(cmd, 0);
                    if Magic::decode(raw).is_storage() { dynamic += 1; continue; }
                    let ev = match raw {
                        500_000..=599_999 => Some((raw - 500_000) as u32),
                        600_000..=699_999 => Some((raw - 600_000) as u32),
                        _ => None,
                    };
                    let Some(ev) = ev else {
                        dead.push(format!("CE{id}@{pc} op210 literal target {raw} unsupported"));
                        continue;
                    };
                    checked += 1;
                    if progs.event(ev).is_none() {
                        dead.push(format!("CE{id}@{pc} op210 -> event {ev} MISSING"));
                    }
                }
                211 => {
                    let raw = carg(cmd, 0);
                    if Magic::decode(raw).is_storage() { dynamic += 1; continue; }
                    let ev = match raw {
                        500_000..=599_999 => (raw - 500_000) as u32,
                        600_000..=699_999 => (raw - 600_000) as u32,
                        other => other.max(0) as u32,
                    };
                    checked += 1;
                    if ev != 0 && progs.event(ev).is_none() {
                        dead.push(format!("CE{id}@{pc} op211 -> event {ev} MISSING"));
                    }
                }
                300 => {
                    let name = cmd.strings.first().cloned().unwrap_or_default();
                    if name.is_empty() || name.contains('\\') || name.contains("cself") || name.contains('{') {
                        dynamic += 1;
                        continue;
                    }
                    checked += 1;
                    if progs.id_by_name(&name).is_none() {
                        if DEAD_ALLOW.contains(&(id, pc)) {
                            allowed += 1;
                        } else {
                            dead.push(format!("CE{id}@{pc} op300 name '{name}' UNRESOLVED"));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    eprintln!("call-targets: checked={checked} dynamic(skipped)={dynamic} allowed(dormant)={allowed} dead={}", dead.len());
    assert!(dead.is_empty(), "{} dead call target(s):\n{}", dead.len(), dead.join("\n"));
}

// ---------------------------------------------------------------------------
// Group B — umbrella pool dispatches into the expected block.
// ---------------------------------------------------------------------------
#[test]
fn pool_dispatch_reaches_expected_block() {
    let cases: &[(u32, usize, i32, usize, &str, &str)] = &[
        (277, 0, 8, 1, "摔了一跤受伤了", "CE277 #66 怪我"),
        (277, 0, 6, 1, "肚子饿了", "CE277 #99 空腹"),
        (277, 0, 11, 1, "感觉没啥干劲", "CE277 #71 無気力"),
        (277, 0, 14, 1, "小指撞到桌角了", "CE277 #108 足をぶつける"),
        (278, 10, 1, 11, "竟然是熊头", "CE278 #15 クマヘッド稽古"),
        (279, 0, 3, 1, "亲亲我我的时间", "CE279 #5/#33 いちゃつく"),
        (279, 0, 4, 1, "留的作业很难", "CE279 #94 勉強教える"),
        (280, 0, 4, 0, "吃东西得再买一点", "CE280 #110 どか食い気絶部"),
    ];
    for &(ce, sel_fn, sel_val, cond_pc, expected, label) in cases {
        let mut game = new_game();
        let run = drive(&mut game, ce, &[(600 + sel_fn, sel_val)], Some(cond_pc), 2000);
        assert_reached(label, &run, expected);
    }
}

// ---------------------------------------------------------------------------
// Group C — named day/dinner/DLC/seal blocks reachable by pc jump.
// pc == the slot index of the headline Message command.
// ---------------------------------------------------------------------------
#[test]
fn block_reachable_pc_jump_samples() {
    let cases: &[(u32, usize, &str, &str)] = &[
        (301, 1145, "留下来继续训练了", "CE301 #43 居残り訓練"),
        (326, 1145, "感觉食欲爆发", "CE326 #79 食欲大爆发"),
        (326, 1323, "好好吃！！！", "CE326 #14 料理大成功"),
        (326, 1378, "要珍惜粮食", "CE326 #97 倹約"),
        (326, 1412, "超级像鸡鸡", "CE326 #98 倹約(胡萝卜)"),
        (449, 45, "源头很可能是", "CE449 #69 デススコーン討伐"),
        (461, 1355, "看啊，那些家伙", "CE461 #58/#76 買い出し"),
        (465, 104, "由衣，你没事吧？", "CE465 #28 看病"),
        (437, 0, "海豹驱除作战", "CE437 アザラシ討伐"),
        (438, 5, "海豹跃出水面", "CE438 アザラシイベント"),
        (317, 163, "比平时还有精神", "CE317 #52 早朝処理"),
        (317, 330, "干劲下降了", "CE317 #51 早朝処理"),
        (250, 26, "午休有妹妹送慰问品", "CE250 #37 差し入れ"),
        (248, 257, "泡杯茶吧", "CE248 #57/#70 お茶"),
        (312, 53, "得到点心了", "CE312 #102 バイト"),
    ];
    for &(ce, pc, expected, label) in cases {
        let mut game = new_game();
        let run = drive(&mut game, ce, &[], Some(pc), 800);
        assert_reached(label, &run, expected);
    }
}

// ---------------------------------------------------------------------------
// Group D — cross-check the gate triples quoted from M13_CONTENT_MATRIX.
// op: 0 >  1 >=  2 ==  3 <=  4 <  5 !=  (crates/wolf-vm/src/exec.rs).
// ---------------------------------------------------------------------------
fn cond_triples(program: &wolf_vm::Program, pc: usize) -> Vec<(i32, i32, i32)> {
    let Some(Slot::Command(cmd)) = program.slots.get(pc) else {
        panic!("pc {pc} is not a command");
    };
    assert_eq!(cmd.opcode, 111, "pc {pc} opcode {} is not 111", cmd.opcode);
    let head = carg(cmd, 0);
    let count = (head & 0x0F) as usize;
    (0..count).map(|i| (
        carg(cmd, 1 + 3 * i), carg(cmd, 2 + 3 * i), carg(cmd, 3 + 3 * i)
    )).collect()
}

#[test]
fn verified_gate_conditions_present() {
    let game = new_game();
    let progs = &game.vm.programs;
    let check = |ce: u32, pc: usize, want: &[(i32, i32, i32)]| {
        let prog = progs.event(ce).unwrap_or_else(|| panic!("CE{ce} missing"));
        let got = cond_triples(prog, pc);
        assert_eq!(got, want, "CE{ce}@{pc} condition triples differ from M13_CONTENT_MATRIX");
    };
    // 参军 END (CE326@35 Comment 親父帰宅END): 信頼(21)<=5 && 機嫌(23)<=1.
    check(326, 36, &[(2_000_021, 5, 3)]);
    check(326, 38, &[(2_000_023, 1, 3)]);
    // 周末 H (CE310): 精力(16)>=1, 性興味(22)>=200, 性欲(18)>=40, 挿入経験(424)>=3.
    check(310, 159, &[(2_000_016, 1, 1)]);
    check(310, 161, &[(2_000_022, 200, 1)]);
    check(310, 163, &[(2_000_018, 40, 1)]);
    check(310, 165, &[(2_000_424, 3, 1)]);
    // 浴室 H (CE318): single op111 with two ordered tiers -- 欲求不満(103)>=2 (case1)
    // else 発情度(102)>=75 (case2); identical bytes at @338 and @415.
    check(318, 338, &[(2_000_103, 2, 1), (2_000_102, 75, 1)]);
    check(318, 415, &[(2_000_103, 2, 1), (2_000_102, 75, 1)]);
}

// ---------------------------------------------------------------------------
// Group E (heavy, #[ignore]) — sweep every branch of the four umbrella pools.
// ---------------------------------------------------------------------------
#[test]
#[ignore]
fn full_pool_dispatch_sweep() {
    for v in 1..=15i32 {
        let mut game = new_game();
        let run = drive(&mut game, 277, &[(600, v)], Some(1), 2000);
        assert_eq!(run.warnings, 0, "CE277 sel={v} warnings; msgs={:?}", run.messages);
        assert!(run.ended, "CE277 sel={v} did not end");
        eprintln!("CE277 sel={v:2} first={:?}", run.messages.first());
    }
    for v in 1..=10i32 {
        let mut game = new_game();
        let run = drive(&mut game, 279, &[(600, v)], Some(1), 2000);
        assert_eq!(run.warnings, 0, "CE279 sel={v} warnings; msgs={:?}", run.messages);
        eprintln!("CE279 sel={v:2} first={:?}", run.messages.first());
    }
    for v in 1..=4i32 {
        let mut game = new_game();
        let run = drive(&mut game, 280, &[(600, v)], Some(0), 2000);
        assert_eq!(run.warnings, 0, "CE280 sel={v} warnings; msgs={:?}", run.messages);
        eprintln!("CE280 sel={v} first={:?}", run.messages.first());
    }
    for v in 1..=9i32 {
        let mut game = new_game();
        let run = drive(&mut game, 278, &[(610, v)], Some(11), 2000);
        assert_eq!(run.warnings, 0, "CE278 sel={v} warnings; msgs={:?}", run.messages);
        eprintln!("CE278 sel={v} first={:?}", run.messages.first());
    }
}
