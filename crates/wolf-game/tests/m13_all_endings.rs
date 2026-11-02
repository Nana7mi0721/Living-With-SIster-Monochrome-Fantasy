//! M13 P2 — remaining seven endings: executable verification.
//!
//! Companion to docs/research/M13_ALL_ENDINGS.md. Each ending is proven two ways:
//!   * a static check that the documented gate triples really exist at the cited
//!     runtime pc, and
//!   * a runtime drive that injects the satisfying gate state, jumps to the block
//!     headline and asserts the ending text renders with stats.warnings == 0.

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
    let t = t.strip_prefix('\u{5C}').unwrap_or(t).trim();
    t.strip_prefix('E').unwrap_or(t).trim().to_string()
}

fn cond_triples(program: &wolf_vm::Program, pc: usize) -> Option<Vec<(i32, i32, i32)>> {
    let Some(Slot::Command(cmd)) = program.slots.get(pc) else { return None };
    if cmd.opcode != 111 { return None; }
    let count = (carg(cmd, 0) & 0x0F) as usize;
    Some((0..count).map(|i| (carg(cmd, 1 + 3 * i), carg(cmd, 2 + 3 * i), carg(cmd, 3 + 3 * i))).collect())
}

fn msg_pc(program: &wolf_vm::Program, needle: &str) -> usize {
    for (pc, slot) in program.slots.iter().enumerate() {
        if let Slot::Command(cmd) = slot {
            if cmd.opcode == 101 && cmd.strings.join("").contains(needle) {
                return pc;
            }
        }
    }
    panic!("message containing {needle:?} not found");
}

fn gate_pc(program: &wolf_vm::Program, want: &[(i32, i32, i32)]) -> usize {
    for pc in 0..program.slots.len() {
        if cond_triples(program, pc).as_deref() == Some(want) {
            return pc;
        }
    }
    panic!("gate {want:?} not found");
}

struct Run {
    messages: Vec<String>,
    warnings: u64,
    ended: bool,
}

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

// ---------------------------------------------------------------------------
// 1. 妹妹怀孕 — CE405.  Good arm: var21(信頼) >= 300 (idx55). Bad arm: else.
// ---------------------------------------------------------------------------
const PREG_GATE: &[(i32, i32, i32)] = &[(2_000_021, 300, 1)];

#[test]
fn pregnancy_good_end() {
    let mut g = new_game();
    let pc = { let p = g.vm.programs.event(405).unwrap(); gate_pc(p, PREG_GATE) };
    eprintln!("CE405 pregnancy gate @pc={pc}");
    let run = drive(&mut g, 405, &[], &[(21, 350)], Some(pc), 6000);
    assert_reached("pregnancy good", &run, "怀孕END");
    assert!(run.messages.iter().any(|m| m.contains("good end")), "missing good-end line: {:?}", run.messages);
}

#[test]
fn pregnancy_bad_end() {
    let mut g = new_game();
    let pc = { let p = g.vm.programs.event(405).unwrap(); gate_pc(p, PREG_GATE) };
    let run = drive(&mut g, 405, &[], &[(21, 100)], Some(pc), 6000);
    assert_reached("pregnancy bad", &run, "不被期待的怀孕");
}

// ---------------------------------------------------------------------------
// 2. 参军 / 返家 — CE326. Gate: 信頼(var21)<=5 (idx36) 且 機嫌(var23)<=1 (idx38);
//    then var151>=1 (idx54) => 兵役, else => 返家.
// ---------------------------------------------------------------------------
const TRUST_GATE: &[(i32, i32, i32)] = &[(2_000_021, 5, 3)];
const MOOD_GATE: &[(i32, i32, i32)] = &[(2_000_023, 1, 3)];
const ARMY_GATE: &[(i32, i32, i32)] = &[(2_000_151, 1, 1)];

#[test]
fn military_service_end() {
    let mut g = new_game();
    let pc = { let p = g.vm.programs.event(326).unwrap(); gate_pc(p, TRUST_GATE) };
    { let p = g.vm.programs.event(326).unwrap();
      assert!(cond_triples(p, gate_pc(p, MOOD_GATE)).is_some());
      assert!(cond_triples(p, gate_pc(p, ARMY_GATE)).is_some()); }
    eprintln!("CE326 trust gate @pc={pc}");
    let run = drive(&mut g, 326, &[], &[(21, 3), (23, 0), (151, 1)], Some(pc), 6000);
    assert_reached("military 兵役", &run, "兵役");
}

#[test]
fn return_home_end() {
    let mut g = new_game();
    let pc = { let p = g.vm.programs.event(326).unwrap(); gate_pc(p, TRUST_GATE) };
    let run = drive(&mut g, 326, &[], &[(21, 3), (23, 0), (151, 0)], Some(pc), 6000);
    assert_reached("返家", &run, "返家");
}

// ---------------------------------------------------------------------------
// 3. 妹妹病亡（症状恶化链）— CE277 dispatch self0 in {4,5,6}.
//    self0=4 => 轻症 arm; self0=5 => 重症 arm (var151=1, var23=-1).
// ---------------------------------------------------------------------------
const SICK_DISPATCH: &[(i32, i32, i32)] = &[(1_600_000, 4, 2), (1_600_000, 5, 2), (1_600_000, 6, 2)];

#[test]
fn sister_sickness_mild_arm() {
    let mut g = new_game();
    let pc = { let p = g.vm.programs.event(277).unwrap(); gate_pc(p, SICK_DISPATCH) };
    let run = drive(&mut g, 277, &[(600, 4)], &[], Some(pc), 3000);
    assert_reached("sick mild 症状", &run, "妹妹的身体状况好像不太好");
}

#[test]
fn sister_sickness_severe_arm_side_effects() {
    // The multi-arm self0 selector at the gate does not reliably select arm 2/3
    // (case_target pairing differs from the editor order), so — like m13_random —
    // jump straight to the target block headline and drive from there.
    let mut g = new_game();
    let pc = { let p = g.vm.programs.event(277).unwrap(); msg_pc(p, "今天特别的色呢") };
    // These 121 commands are compound (bits 0x100/0x200 = +=/=), so assert deltas.
    let a151 = g.vm.vars.get(151);
    let a23 = g.vm.vars.get(23);
    let a18 = g.vm.vars.get(18);
    let a102 = g.vm.vars.get(102);
    let run = drive(&mut g, 277, &[], &[], Some(pc), 3000);
    assert!(!run.messages.is_empty(), "severe arm rendered nothing: {:?}", run.messages);
    let v151 = g.vm.vars.get(151);
    let v23 = g.vm.vars.get(23);
    let v18 = g.vm.vars.get(18);
    let v102 = g.vm.vars.get(102);
    eprintln!(
        "severe arm deltas: var151 {a151}->{v151}(+1), var23 {a23}->{v23}(-1), var18 {a18}->{v18}(-5), var102 {a102}->{v102}(-10), msgs={:?}",
        run.messages
    );
    assert_eq!(run.warnings, 0, "warnings in severe arm: {:?}", run.messages);
    assert_eq!(v151, a151 + 1, "severe arm should add 1 to var151 (feeds CE326 兵役 branch)");
    assert_eq!(v18, a18 - 5, "severe arm should subtract 5 from var18");
    assert_eq!(v102, a102 - 10, "severe arm should subtract 10 from var102");
    // var23(機嫌) is set to -1 at pc123, but a later step in the same event recomputes
    // it back to 0 (verified by frame trace: v23 = -1 at pc125, 0 by event end), so the
    // final value is not a stable assertion; only the persistent deltas above are.
    let _ = (a23, v23);
}

// ---------------------------------------------------------------------------
// 4/5. 暗影 / 魔界 — CE305 stage25 battle-result branch:
//    self30 == 1 => 阴影 END; self30 <= -1 => 魔界 BAD END.
// ---------------------------------------------------------------------------
const SHADOW_GATE: &[(i32, i32, i32)] = &[(1_600_030, 1, 2), (1_600_030, -1, 3)];
// The same (self30==1 | self30<=-1) triple also appears at an early battle-result
// branch (pc~433); the stage25 station is the one we want. Dump index == runtime pc
// (confirmed by farmer dispatch idx1193 == pc1193), so pin it explicitly.
const SHADOW_PC: usize = 2915;

fn shadow_gate_pc(g: &Game) -> usize {
    let p = g.vm.programs.event(305).unwrap();
    assert_eq!(
        cond_triples(p, SHADOW_PC).as_deref(),
        Some(SHADOW_GATE),
        "CE305 dump idx2915 is not the shadow/makai gate"
    );
    SHADOW_PC
}

#[test]
fn shadow_end() {
    let mut g = new_game();
    let pc = shadow_gate_pc(&g);
    let run = drive(&mut g, 305, &[(630, 1)], &[], Some(pc), 4000);
    assert_reached("阴影 END", &run, "阴影");
}

#[test]
fn makai_end() {
    let mut g = new_game();
    let pc = shadow_gate_pc(&g);
    let run = drive(&mut g, 305, &[(630, -1)], &[], Some(pc), 4000);
    assert_reached("魔界 END", &run, "魔界");
}

// ---------------------------------------------------------------------------
// 6. 农民 END — CE305 stage13/14/15 dispatch then var534==2.
// ---------------------------------------------------------------------------
const FARM_DISPATCH: &[(i32, i32, i32)] = &[(1_600_000, 13, 2), (1_600_000, 14, 2), (1_600_000, 15, 2)];

#[test]
fn farmer_end() {
    let mut g = new_game();
    let pc = { let p = g.vm.programs.event(305).unwrap(); gate_pc(p, FARM_DISPATCH) };
    eprintln!("CE305 farmer dispatch @pc={pc}");
    let run = drive(&mut g, 305, &[(600, 13)], &[(534, 2)], Some(pc), 6000);
    assert_reached("农民 END", &run, "农民END");
}

// ---------------------------------------------------------------------------
// 7. 妹妹爆炸 BADEND — CE322 / CE323 / CE359 (identical text).
// ---------------------------------------------------------------------------
fn explosion_case(ce: u32, label: &str) {
    let mut g = new_game();
    let pc = { let p = g.vm.programs.event(ce).unwrap(); msg_pc(p, "妹妹爆炸") };
    let run = drive(&mut g, ce, &[], &[], Some(pc), 400);
    assert_reached(label, &run, "妹妹爆炸");
}

#[test]
fn sister_explosion_badend_ce322() { explosion_case(322, "CE322 explosion"); }
#[test]
fn sister_explosion_badend_ce323() { explosion_case(323, "CE323 explosion"); }
#[test]
fn sister_explosion_badend_ce359() { explosion_case(359, "CE359 explosion"); }

#[test]
fn ce297_oracle_foreshadows_explosion() {
    let mut g = new_game();
    let pc = { let p = g.vm.programs.event(297).unwrap(); msg_pc(p, "积攒了大量魔力") };
    let run = drive(&mut g, 297, &[], &[], Some(pc), 400);
    assert_reached("CE297 oracle", &run, "爆炸");
}

// ---------------------------------------------------------------------------
// Static gate cross-check.
// ---------------------------------------------------------------------------
#[test]
fn ending_gates_present() {
    let g = new_game();
    let p = &g.vm.programs;
    let check = |ce: u32, want: &[(i32, i32, i32)]| {
        let prog = p.event(ce).unwrap_or_else(|| panic!("CE{ce} missing"));
        let pc = gate_pc(prog, want);
        assert_eq!(cond_triples(prog, pc).as_deref(), Some(want), "CE{ce} gate differs");
        eprintln!("CE{ce} gate {want:?} @pc={pc}");
    };
    check(405, PREG_GATE);
    check(326, TRUST_GATE);
    check(326, MOOD_GATE);
    check(326, ARMY_GATE);
    check(277, SICK_DISPATCH);
    check(305, SHADOW_GATE);
    check(305, FARM_DISPATCH);
}

// ---------------------------------------------------------------------------
// Long chains (#[ignore]).
// ---------------------------------------------------------------------------
#[test]
#[ignore = "slow chain CE326 to CE405"]
fn pregnancy_chain_via_ce326() {
    let var105_gate: &[(i32, i32, i32)] = &[(2_000_105, 3, 1)];
    let mut g = new_game();
    let pc = { let p = g.vm.programs.event(326).unwrap(); gate_pc(p, var105_gate) };
    let run = drive(&mut g, 326, &[], &[(105, 3), (21, 350)], Some(pc), 12000);
    assert_reached("CE326 to CE405 good", &run, "怀孕END");
    let mut g2 = new_game();
    let run2 = drive(&mut g2, 326, &[], &[(105, 3), (21, 100)], Some(pc), 12000);
    assert_reached("CE326 to CE405 bad", &run2, "不被期待的怀孕");
}

#[test]
#[ignore = "slow chain CE305 stage25 scene"]
fn ce305_stage25_scene_to_makai() {
    // Drive the real stage25 block: 由香莉 warns of the black mist, then the party
    // fights. Headless never wins the battle, so self30 lands <= -1 and the script
    // falls into the 魔界 BAD END. (The 阴影 END arm, self30 == 1, is covered
    // deterministically by shadow_end, which injects the post-battle result.)
    let stage_gate: &[(i32, i32, i32)] = &[(1_600_000, 25, 2), (1_600_000, 26, 2), (1_600_000, 27, 2)];
    let mut g = new_game();
    let pc = { let p = g.vm.programs.event(305).unwrap(); gate_pc(p, stage_gate) };
    let run = drive(&mut g, 305, &[(600, 25)], &[], Some(pc), 20000);
    assert!(
        run.messages.iter().any(|m| m.contains("黑雾")),
        "stage25 scene missing black-mist setup: {:?}",
        run.messages
    );
    assert_reached("stage25 to 魔界", &run, "魔界");
}
