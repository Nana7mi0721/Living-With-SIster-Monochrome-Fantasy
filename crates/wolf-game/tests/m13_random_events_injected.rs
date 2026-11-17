//! M13 P2 — 18 condition-injection cases (16 DLC random events + 特4 two blocks)
//! and their 15 in-event opcode-111 gate payloads, all loaded from the shared fixture
//! oracle/fixtures/content_matrix_injected.json. C# godot/tests/ContentMatrixTest.cs
//! reads the same file, so the two runtimes can no longer drift apart (R10-A2).
//!
//!   WOLF_DATA_ROOT=work/all cargo test -p wolf-game --test m13_random_events_injected -- --nocapture

use std::path::PathBuf;

use serde_json::Value;
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

fn fixture() -> Value {
    let path = repo_root().join("oracle/fixtures/content_matrix_injected.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).expect("content matrix fixture json")
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
    for frame in 0..max_frames {
        // UI-06 uses fresh-press edge detection; holding OK every frame would
        // only advance the first message. Pulse it every other frame.
        game.key_bits = if frame % 2 == 0 { wolf_game::key::OK } else { 0 };
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
    selfs: Vec<(usize, i32)>,
    globals: Vec<(i32, i32)>,
    pc: usize,
    max_frames: usize,
    expected: String,
    label: String,
    gate: String,
    alias_of: String,
}

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap_or_else(|| panic!("expected integer, got {v}")) as i32
}

fn pairs(value: &Value) -> Vec<(i32, i32)> {
    value.as_array().expect("pair array").iter()
        .map(|p| (int(&p[0]), int(&p[1])))
        .collect()
}

fn load_cases(root: &Value) -> Vec<Case> {
    root["cases"].as_array().expect("cases").iter().map(|c| Case {
        ce: int(&c["ce"]) as u32,
        selfs: pairs(&c["self"]).into_iter().map(|(a, b)| (a as usize, b)).collect(),
        globals: pairs(&c["globals"]),
        pc: int(&c["pc"]) as usize,
        max_frames: c["max_frames"].as_u64().unwrap_or(1200) as usize,
        expected: c["expected"].as_str().expect("expected").to_string(),
        label: c["label"].as_str().expect("label").to_string(),
        gate: c["gate"].as_str().unwrap_or("").to_string(),
        alias_of: c["alias_of"].as_str().unwrap_or("").to_string(),
    }).collect()
}

#[test]
fn injected_cases_reach_expected_block() {
    let root = fixture();
    let cases = load_cases(&root);
    assert_eq!(cases.len(), 18, "expected 18 cases");
    let unique = cases.iter().filter(|c| c.alias_of.is_empty()).count();
    assert_eq!(unique, 17, "expected 17 unique script blocks");
    for alias in cases.iter().filter(|c| !c.alias_of.is_empty()) {
        let base = cases.iter().find(|c| c.label == alias.alias_of)
            .unwrap_or_else(|| panic!("{}: alias target {} missing", alias.label, alias.alias_of));
        assert_eq!((alias.ce, alias.pc, &alias.expected), (base.ce, base.pc, &base.expected),
            "{}: aliased row diverged from {}", alias.label, alias.alias_of);
        assert_eq!(alias.selfs, base.selfs, "{}: self vars diverge", alias.label);
        assert_eq!(alias.globals, base.globals, "{}: globals diverge", alias.label);
    }
    for c in &cases {
        let mut game = new_game();
        let run = drive(&mut game, c.ce, &c.selfs, &c.globals, Some(c.pc), c.max_frames);
        eprintln!("OK {:?} CE{} pc={} gate={}", c.label, c.ce, c.pc, c.gate);
        assert_reached(&c.label, &run, &c.expected);
    }
}

#[test]
fn injected_gate_triples_present() {
    let root = fixture();
    let game = new_game();
    let progs = &game.vm.programs;
    let gates = root["gates"].as_array().expect("gates");
    assert_eq!(gates.len(), 15, "expected 15 op111 gate payloads");
    for g in gates {
        let ce = int(&g["ce"]) as u32;
        let pc = int(&g["pc"]) as usize;
        let want: Vec<(i32, i32, i32)> = g["triples"].as_array().expect("triples").iter()
            .map(|t| (int(&t[0]), int(&t[1]), int(&t[2])))
            .collect();
        let prog = progs.event(ce).unwrap_or_else(|| panic!("CE{ce} missing"));
        let got = cond_triples(prog, pc).unwrap_or_else(|| panic!("CE{ce}@{pc} not op111"));
        assert_eq!(got, want, "CE{ce}@{pc} gate triples differ");
    }
}
