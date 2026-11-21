//! M7 W2-a: prove an H scene advances interactively through its opcode-123
//! poll gates purely from `game.key_bits` (the OK/decide key), with no
//! prod-code changes. See docs/research/M7_HSCENE_ADVANCE.md.
//!
//! Every opcode-123 poll in the H common events (CE346 / CE371 / CE410) is the
//! same shape:
//!   121 [target, 1, 0, 256]        ; write the key code to a scratch self var
//!   123 [target, 17]               ; target <- host.key_code_auto(mask)
//!   111 [2, target, 0, 2, target, 10, 2]   ; key==0 -> retry, key==10(OK) -> continue
//! So holding the OK bit in `game.key_bits` walks *past* the poll
//! (continue branch, short wait), while key_bits==0 parks on the retry branch
//! (50-frame wait loop). That pc divergence is the observable proof the scene
//! advanced because of real key input, not because a variable was poked.

use std::collections::BTreeSet;
use std::path::PathBuf;

use wolf_game::Game;

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".to_owned());
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        p
    } else {
        repo_root().join(p)
    }
}

fn save_path() -> PathBuf {
    repo_root().join("work/retail_backup/Save/SaveData08.sav")
}

fn open_game() -> Option<Game> {
    match Game::new(data_root()) {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("skipping m7_hscene_advance: cannot open game data ({e})");
            None
        }
    }
}

/// One opcode-123 poll gate: an event, the (self) call arguments that route
/// execution into the poll block, optional extra global seeds, and the two pcs
/// the poll branches to (retry when key==0, continue when key==OK).
struct PollCase {
    label: &'static str,
    event: u32,
    args: [i32; 3],
    extra_vars: &'static [(usize, i32)],
    retry_pc: i32,
    cont_pc: i32,
}

/// Run one event for `frames` frames, holding the OK key every frame when
/// `inject` is true, and return the set of pcs its instance visited.
fn run_case(case: &PollCase, inject: bool, frames: usize) -> BTreeSet<i32> {
    let mut game = open_game().expect("game data must load for m7 test");
    game.auto_advance = true;
    game.auto_choice = true;
    game.load_save(&save_path()).expect("load retail save");
    game.start_engine_events();
    let id = game.vm.start_event(case.event).expect("start event");
    if let Some(inst) = game.vm.instance_mut(id) {
        for (k, v) in case.args.iter().enumerate() {
            inst.self_vars.set(600 + k, *v);
        }
    }
    for (addr, val) in case.extra_vars {
        game.vm.vars.set(*addr, *val);
    }
    let mut visited = BTreeSet::new();
    for _ in 0..frames {
        game.key_bits = if inject { wolf_game::key::OK } else { 0 };
        game.frame();
        if let Some(inst) = game.vm.instances().iter().find(|i| i.event == case.event) {
            visited.insert(inst.pc as i32);
        } else {
            break;
        }
    }
    visited
}

const CASES: &[PollCase] = &[
    // ---- CE346 (2 polls) -------------------------------------------------
    // args [2,1,1] -> block 4/5 poll @ pc48; retry pc52, continue pc55
    PollCase {
        label: "CE346 args[2,1,1] poll#1 pc48",
        event: 346,
        args: [2, 1, 1],
        extra_vars: &[],
        retry_pc: 52,
        cont_pc: 55,
    },
    // args [4,0,1] -> block 11/12 poll @ pc119; retry pc123, continue pc126
    PollCase {
        label: "CE346 args[4,0,1] poll#2 pc119",
        event: 346,
        args: [4, 0, 1],
        extra_vars: &[],
        retry_pc: 123,
        cont_pc: 126,
    },
    // ---- CE371 (7 polls, one per global 2000218 stage) -------------------
    PollCase {
        label: "CE371 var218=1 poll pc74",
        event: 371,
        args: [1, 0, 0],
        extra_vars: &[(218, 1)],
        retry_pc: 79,
        cont_pc: 83,
    },
    PollCase {
        label: "CE371 var218=2 poll pc157",
        event: 371,
        args: [1, 0, 0],
        extra_vars: &[(218, 2)],
        retry_pc: 162,
        cont_pc: 166,
    },
    PollCase {
        label: "CE371 var218=3 poll pc239",
        event: 371,
        args: [1, 0, 0],
        extra_vars: &[(218, 3)],
        retry_pc: 244,
        cont_pc: 248,
    },
    PollCase {
        label: "CE371 var218=4 poll pc326",
        event: 371,
        args: [1, 0, 0],
        extra_vars: &[(218, 4)],
        retry_pc: 331,
        cont_pc: 335,
    },
    PollCase {
        label: "CE371 var218=5 poll pc412",
        event: 371,
        args: [1, 0, 0],
        extra_vars: &[(218, 5)],
        retry_pc: 417,
        cont_pc: 421,
    },
    PollCase {
        label: "CE371 var218=6 poll pc502",
        event: 371,
        args: [1, 0, 0],
        extra_vars: &[(218, 6)],
        retry_pc: 507,
        cont_pc: 511,
    },
    PollCase {
        label: "CE371 var218=7 poll pc586",
        event: 371,
        args: [1, 0, 0],
        extra_vars: &[(218, 7)],
        retry_pc: 591,
        cont_pc: 595,
    },
    // ---- CE410 (5 polls, one per arg[0] 1..5) ----------------------------
    PollCase {
        label: "CE410 args[1,0,0] poll pc40",
        event: 410,
        args: [1, 0, 0],
        extra_vars: &[(143, 0), (144, 0)],
        retry_pc: 44,
        cont_pc: 47,
    },
    PollCase {
        label: "CE410 args[2,0,0] poll pc87",
        event: 410,
        args: [2, 0, 0],
        extra_vars: &[(143, 0), (144, 0)],
        retry_pc: 91,
        cont_pc: 94,
    },
    PollCase {
        label: "CE410 args[3,0,0] poll pc134",
        event: 410,
        args: [3, 0, 0],
        extra_vars: &[(143, 0), (144, 0)],
        retry_pc: 138,
        cont_pc: 141,
    },
    PollCase {
        label: "CE410 args[4,0,0] poll pc177",
        event: 410,
        args: [4, 0, 0],
        extra_vars: &[(143, 0), (144, 0)],
        retry_pc: 181,
        cont_pc: 184,
    },
    PollCase {
        label: "CE410 args[5,0,0] poll pc221",
        event: 410,
        args: [5, 0, 0],
        extra_vars: &[(143, 0), (144, 0)],
        retry_pc: 225,
        cont_pc: 228,
    },
];

#[test]
fn h_scene_polls_advance_only_with_the_ok_key() {
    if open_game().is_none() {
        return;
    }
    let frames = 80usize;
    let mut advanced = 0usize;
    for case in CASES {
        let no_key = run_case(case, false, frames);
        let ok_key = run_case(case, true, frames);

        // Without the OK key the poll retries: it visits the retry pc and never
        // reaches the continue pc.
        assert!(
            no_key.contains(&case.retry_pc),
            "{}: key=0 should park on the retry pc {} but visited {:?}",
            case.label,
            case.retry_pc,
            no_key
        );
        assert!(
            !no_key.contains(&case.cont_pc),
            "{}: key=0 must NOT pass the poll (continue pc {}) but visited {:?}",
            case.label,
            case.cont_pc,
            no_key
        );

        // With the OK key held the poll continues: it reaches the continue pc.
        assert!(
            ok_key.contains(&case.cont_pc),
            "{}: key=OK should reach the continue pc {} but visited {:?}",
            case.label,
            case.cont_pc,
            ok_key
        );

        advanced += 1;
    }

    // Warnings must stay at zero throughout.
    let mut last = Game::new(data_root()).expect("game");
    last.auto_advance = true;
    last.auto_choice = true;
    last.load_save(&save_path()).expect("save");
    last.start_engine_events();
    let id = last.vm.start_event(346).expect("start");
    if let Some(inst) = last.vm.instance_mut(id) {
        inst.self_vars.set(600, 2);
        inst.self_vars.set(601, 1);
        inst.self_vars.set(602, 1);
    }
    for _ in 0..frames {
        last.key_bits = wolf_game::key::OK;
        last.frame();
    }
    let warnings_seen = last.stats.warnings;
    // Static H layer sanity: a plain load of SaveData08 (no extra event started)
    // must keep the same >= 39 textured quads the capture-backed render uses.
    let mut static_game = Game::new(data_root()).expect("game");
    static_game.auto_advance = true;
    static_game.auto_choice = true;
    static_game.load_save(&save_path()).expect("save");
    static_game.start_engine_events();
    for _ in 0..30 {
        static_game.frame();
    }
    let mut atlas = wolf_game::atlas::Atlas::new();
    let quads = static_game.picture_quads(&mut atlas).len();
    // Which run_condition==2 H text events actually scheduled under this save.
    let h_text_auto = [350u32, 351, 353, 354, 357, 358, 367];
    let live: Vec<u32> = h_text_auto
        .iter()
        .copied()
        .filter(|e| static_game.vm.instances().iter().any(|i| i.event == *e))
        .collect();

    eprintln!(
        "m7_hscene_advance: {advanced}/{} opcode-123 polls advanced on key_bits=OK only; warnings={warnings_seen}; picture_quads={quads}; live_h_text_auto={live:?}",
        CASES.len()
    );
    assert_eq!(advanced, CASES.len(), "all 14 polls must be exercised");
    assert_eq!(
        warnings_seen, 0,
        "no engine warnings during H-scene advance"
    );
    assert!(
        quads >= 39,
        "H layer must stay >= 39 textured quads, got {quads}"
    );
}
