//! M7 W2 regression coverage for the run_condition == 2 (auto-event) scheduler
//! driving the H text-effect events end to end.
//!
//! The seven H text-effect (文字エフェクト) events are declared with run_condition == 2
//! in BasicData/CommonEvent.dat, so the AutoEvents scheduler - not a 210 call - starts
//! them. This test pins two facts:
//!   1. AutoEvents tracks all seven and starts each on a genuine false->true edge of its
//!      single variable gate (op >=, value 1). Loading a save restores the running state
//!      from the SavePart5 picture snapshot and does NOT re-trigger condition edges
//!      (prime_latched, task-22); the scheduler is complete; no other code fix was needed.
//!   2. Scheduling 353 (文字エフェクト挿入中) actually renders H content: a new visible
//!      H CG (hscene/h_s10_sonyuu01.png, opacity 255) lands in the picture quads, and the
//!      event re-draws an hword/word_effect*.png text slot.
//!
//! See docs/research/M7_HAUTO_EVENTS.md. Run with WOLF_DATA_ROOT=work/all.

use std::collections::BTreeMap;
use std::path::PathBuf;

use wolf_game::atlas::Atlas;
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

fn open() -> Option<Game> {
    match Game::new(data_root()) {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!(
                "skipping: could not open data root ({}); set WOLF_DATA_ROOT",
                e
            );
            None
        }
    }
}

fn boot() -> Game {
    let mut game = open().expect("data root");
    game.auto_advance = true;
    game.auto_choice = true;
    game.load_save(&save_path()).expect("SaveData08");
    game.start_engine_events();
    game
}

// The seven H text-effect events and the ordinary-variable index of their gate.
// Raw condition variable is 2_000_000 + index; vars.set wants the plain index.
// Every one is run_condition == 2, operator >=, value 1.
const H_TEXT_EVENTS: &[(u32, usize)] = &[
    (350, 118), // 文字エフェクトみぎ
    (351, 119), // 文字エフェクトひだり
    (353, 99),  // 文字エフェクト挿入中
    (354, 124), // 文字エフェクト射精
    (357, 62),  // 文字エフェクトアイテム
    (358, 123), // 文字エフェクト妹絶頂
    (367, 207), // カットイン瞬き
];

#[test]
fn scheduler_tracks_all_seven_h_text_events() {
    if open().is_none() {
        return;
    }
    let mut game = boot();

    let tracked: BTreeMap<u32, (u8, u32, u32)> = game
        .auto_events
        .events()
        .iter()
        .map(|a| (a.id, (a.operator, a.variable, a.value)))
        .collect();
    for &(ev, gate) in H_TEXT_EVENTS {
        let (op, var, value) = tracked
            .get(&ev)
            .copied()
            .unwrap_or_else(|| panic!("H text event {} is not tracked by AutoEvents", ev));
        assert_eq!(op, 1, "event {} gate operator must be >= (1)", ev);
        assert_eq!(var, 2_000_000 + gate as u32, "event {} gate variable", ev);
        assert_eq!(value, 1, "event {} gate value", ev);
    }

    // New semantics (task-22, prime_latched): loading a save must NOT re-trigger a
    // run_condition == 2 event whose condition already holds; the running state comes from
    // the SavePart5 picture snapshot instead. So right after boot NONE of the seven is
    // scheduled - including 357, whose gate var62 >= 1 is already true in SaveData08 (this
    // is exactly the same false-edge suppression that removes the spurious 506 eye overlay).
    for &(ev, _) in H_TEXT_EVENTS {
        assert!(
            game.auto_events.instance(ev).is_none(),
            "event {} must NOT be scheduled merely by a save load (prime suppresses the false edge)",
            ev
        );
    }

    // Drive a genuine 0 -> 1 edge of each gate. Force it to 0 first: a gate that was primed
    // true (e.g. 357's var62) must drop to false and leave no instance, while a gate already
    // 0 stays off. Then set it to 1: the real false -> true edge schedules the event.
    for &(ev, gate) in H_TEXT_EVENTS {
        game.vm.vars.set(gate, 0);
        game.frame();
        assert!(
            game.auto_events.instance(ev).is_none(),
            "event {} must stay off while its gate is 0",
            ev
        );
        game.vm.vars.set(gate, 1);
        game.frame();
        assert!(
            game.auto_events.instance(ev).is_some(),
            "event {} did not schedule on a real 0 -> 1 gate edge",
            ev
        );
    }
    // The running map keeps an entry for every event whose condition currently holds, even
    // after its script ends: confirm all seven are in the running set.
    let running: std::collections::BTreeSet<u32> =
        game.auto_events.running_ids().into_iter().collect();
    for &(ev, _) in H_TEXT_EVENTS {
        assert!(
            running.contains(&ev),
            "event {} missing from running_ids",
            ev
        );
    }
}

#[test]
fn scheduling_ev353_draws_h_text_and_a_visible_h_cg() {
    if open().is_none() {
        return;
    }

    let mut base = boot();
    for _ in 0..200 {
        base.frame();
    }
    let base_quads = base.picture_quads(&mut Atlas::new()).len();
    let base_word_slots: BTreeMap<i32, String> = base
        .screen
        .iter()
        .filter(|p| p.file.to_lowercase().contains("word_effect"))
        .map(|p| (p.number, p.file.clone()))
        .collect();
    assert!(
        base.screen.get(1185).is_none(),
        "CG 1185 must not exist before 353 runs"
    );

    let mut game = boot();
    game.vm.vars.set(99, 1);
    for _ in 0..200 {
        game.frame();
    }
    assert!(
        game.auto_events.instance(353).is_some(),
        "353 was not scheduled after var99=1"
    );

    let cg = game
        .screen
        .get(1185)
        .expect("event 353 should draw picture 1185");
    assert!(
        cg.file.to_lowercase().contains("h_s10_sonyuu01"),
        "expected hscene/h_s10_sonyuu01.png, got {:?}",
        cg.file
    );
    assert!(!cg.hidden, "the insertion CG must be visible, not hidden");
    assert!(
        cg.opacity > 0,
        "the insertion CG must have positive opacity"
    );
    let quads = game.picture_quads(&mut Atlas::new()).len();
    assert!(
        quads > base_quads,
        "scheduling 353 should add a visible quad"
    );

    // The event re-draws the hword/word_effect* text layer, so at least one word slot
    // now holds a different text image than baseline. NOTE: the retail script immediately
    // parks that slot to opacity 0 with a follow-up move whose opacity slot is 0, so it does
    // not stay painted; see docs/research/M7_HAUTO_EVENTS.md for the exact instruction and
    // the picture.rs limitation that keeps the glyph layer at alpha 0 in this engine.
    let changed_text: Vec<(i32, String)> = game
        .screen
        .iter()
        .filter(|p| p.file.to_lowercase().contains("word_effect"))
        .filter(|p| base_word_slots.get(&p.number) != Some(&p.file))
        .map(|p| (p.number, p.file.clone()))
        .collect();
    assert!(
        !changed_text.is_empty(),
        "scheduling 353 should re-draw an hword/word_effect text slot"
    );
}
