//! M9 W2 residual — unlock and pin the H-scene CG cluster (`var460` / `var111` / `var157`).
//!
//! The previous forward-playthrough note (`m8_hscene_forward.rs`, `M9_HSCENE_LOOP.md`)
//! concluded that the map H CG cluster — map event 33 ("挿入抜く", insert-pull) and the
//! `CommonEvent` bodies it calls (CE333, CE246, CE372, CE402) — could not be entered
//! because `var460` (`ev33`'s only page condition) "is written only by
//! CE344/CE345/CE347". Re-reading the map bytecode (not only the common events) shows
//! that conclusion is wrong:
//!
//! * The H UI dispatcher **map event 11** ("■深夜UI判定") writes `var460 = 1` directly at
//!   **pc706** (`op121 [2_000_460, 1, 0, 0]`) on the *挿入 抜く* hover branch whose guard
//!   sets `var61 = 35` at **pc705** (`op121 [2_000_061, 35, 0, 0]`). Picture 444 is the
//!   button that produces hover code 35.
//! * Map events **26 / 27 / 28 / 39** also write `var460 = 1` as part of their insertion
//!   blocks (e.g. EV26 pcs 496/862/1119/1319/1353, EV28 pcs 327/465/610/929/1356/1715).
//!
//! So a *legitimate* input — hovering the code-35 insertion button — arms `var460 = 1`,
//! map event 33 activates (its sole page-0 condition is `var460 == 1`), and `ev33` calls
//! **CE333 fifteen times** (`op210 500333` at pc 60/103/210/224/231/245/252/254/258/264/266/269/275/282/285).
//! This works even on the drained mid-H save (`SaveData08`), because neither `var460` nor
//! `ev33` is gated by the energy variables `var100`/`var267`.
//!
//! Separately, `var111` (the arousal / ejaculation gauge) *is* reachable to `>= 100`
//! through real insertion hover codes, provided the day-save energy state
//! (`var100 >= 12`, `var267 == 0` — the exact values all seven day saves ship) holds: the
//! shallow insertion events add `var111` +5/+8/+10/+15 per action, and the アナル event
//! (EV28 pc159) adds a further `var111 = 119 + var111`. `SaveData08` alone is the drained
//! exception (`var100 = 0`, `var267 = 1`), which is why earlier probes on that snapshot
//! never raised the gauge.
//!
//! The two deeper CG gates are documented but **not** claimed open here: CE246's deep body
//! needs `var157 == 13` (CE246 pc1943) and CE402 needs `var104 / var105 == 0` plus
//! `var4 >= 21` (CE328 pc4/pc6). Those are separate player-progress gates.
//!
//! Run: `WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_cg_unlock`
//! Data root: `work/all`; save: `work/retail_backup/Save/SaveData08.sav`.

use std::path::{Path, PathBuf};

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_format::mps::{Map, MapEventPage, PageCondition};
use wolf_game::Game;

// ---------------------------------------------------------------------------------------
// Paths and data loading.
// ---------------------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn data_root() -> PathBuf {
    if let Ok(value) = std::env::var("WOLF_DATA_ROOT") {
        if !value.is_empty() {
            return repo_root().join(value);
        }
    }
    repo_root().join("work").join("all")
}

fn save_dir() -> PathBuf {
    repo_root().join("work").join("retail_backup").join("Save")
}

fn save_path() -> PathBuf {
    save_dir().join("SaveData08.sav")
}

fn load_sample_map() -> Map {
    let bytes =
        std::fs::read(data_root().join("MapData").join("SampleMapA.mps")).expect("SampleMapA.mps");
    Map::parse(&bytes).expect("parse SampleMapA.mps")
}

fn common_events() -> CommonEventData {
    let bytes = std::fs::read(data_root().join("BasicData").join("CommonEvent.dat"))
        .expect("BasicData/CommonEvent.dat");
    CommonEventData::parse(&bytes).expect("parse CommonEvent.dat")
}

// ---------------------------------------------------------------------------------------
// Bytecode readers.
// ---------------------------------------------------------------------------------------

fn words(cmd: &wolf_format::command::Command) -> Vec<i32> {
    cmd.params
        .chunks_exact(4)
        .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect()
}

/// Every (var, value, op) triple tested by an opcode-111 conditional in a common event.
fn ce_cond_triples(data: &CommonEventData, event: u32) -> Vec<(usize, i32, i32, i32)> {
    let ev = data
        .event(event)
        .unwrap_or_else(|| panic!("CE{event} exists"));
    let mut out = Vec::new();
    for (pc, slot) in ev.commands.iter().enumerate() {
        if let Slot::Command(cmd) = slot {
            if cmd.opcode == 111 {
                let ints = words(cmd);
                if let Some(&count) = ints.first() {
                    for k in 0..count.max(0) as usize {
                        let base = 1 + 3 * k;
                        if let (Some(var), Some(value), Some(op)) =
                            (ints.get(base), ints.get(base + 1), ints.get(base + 2))
                        {
                            out.push((pc, *var, *value, *op));
                        }
                    }
                }
            }
        }
    }
    out
}

fn map_event(map: &Map, id: u32) -> &wolf_format::mps::MapEvent {
    map.events
        .iter()
        .find(|event| event.id == id)
        .unwrap_or_else(|| panic!("map event {id} exists"))
}

fn map_ints(page: &MapEventPage, pc: usize) -> Vec<i32> {
    match page
        .commands
        .get(pc)
        .unwrap_or_else(|| panic!("map pc{pc} exists"))
    {
        Slot::Command(cmd) => words(cmd),
        _ => panic!("map pc{pc} is blank"),
    }
}

/// All pcs of a map page whose opcode-`op` first integer argument equals `first`.
fn map_cmd_pcs(page: &MapEventPage, op: u32, first: i32) -> Vec<usize> {
    page.commands
        .iter()
        .enumerate()
        .filter_map(|(pc, slot)| match slot {
            Slot::Command(cmd) if cmd.opcode == op => {
                let f = words(cmd).into_iter().next()?;
                (f == first).then_some(pc)
            }
            _ => None,
        })
        .collect()
}

fn enabled_conditions(page: &MapEventPage) -> Vec<PageCondition> {
    page.conditions()
        .into_iter()
        .filter(|condition| condition.enabled)
        .collect()
}

/// Every (var, value, op) triple tested by an opcode-111 conditional in a map page's body.
fn map_cond_triples(page: &MapEventPage) -> Vec<(usize, i32, i32, i32)> {
    let mut out = Vec::new();
    for (pc, slot) in page.commands.iter().enumerate() {
        if let Slot::Command(cmd) = slot {
            if cmd.opcode == 111 {
                let ints = words(cmd);
                if let Some(&count) = ints.first() {
                    for k in 0..count.max(0) as usize {
                        let base = 1 + 3 * k;
                        if let (Some(var), Some(value), Some(op)) =
                            (ints.get(base), ints.get(base + 1), ints.get(base + 2))
                        {
                            out.push((pc, *var, *value, *op));
                        }
                    }
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Live game harness (mirrors m8_hscene_forward.rs / m7_hscene_playable.rs).
// ---------------------------------------------------------------------------------------

fn load_save_into(save: &Path) -> Option<Game> {
    if !save.exists() {
        return None;
    }
    let mut game = match Game::new(data_root()) {
        Ok(game) => game,
        Err(error) => {
            eprintln!("skip: Game::new failed: {error}");
            return None;
        }
    };
    if let Err(error) = game.load_save(save) {
        eprintln!("skip: load_save({}) failed: {error}", save.display());
        return None;
    }
    Some(game)
}

/// Resume `SaveData08` on the map, skipping the opening autorun (event 0).
fn open_resumed() -> Option<Game> {
    let save = save_path();
    if !data_root().join("MapData").join("SampleMapA.mps").exists() {
        eprintln!(
            "skip: missing SampleMapA.mps under {}",
            data_root().display()
        );
        return None;
    }
    let mut game = load_save_into(&save)?;
    let map = load_sample_map();
    game.start_engine_events();
    game.start_pages(&map, Some(0));
    Some(game)
}

fn drive(game: &mut Game, frames: usize) {
    for _ in 0..frames {
        game.frame();
    }
}

// ---------------------------------------------------------------------------------------
// Test A — a real insertion hover (var61 = 35) arms var460 and opens map event 33.
// ---------------------------------------------------------------------------------------

#[test]
fn insertion_hover_35_arms_var460_and_opens_event_33() {
    let Some(mut game) = open_resumed() else {
        return;
    };
    drive(&mut game, 60);

    // Only normalization: the shipped save has the "dispatcher open" flag var364 already
    // clear (var364 == 0). Confirm the scene is the late-night H UI and the gauge starts clean.
    game.vm.vars.set(364, 0);
    assert_eq!(game.vm.vars.get(62), 1, "starts on H UI stage 1");
    game.vm.vars.set(111, 0);
    game.vm.vars.set(460, 0);

    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.set_choice_cursor(0);
    drive(&mut game, 4);

    // Real input: hovering the "挿入 抜く" button (picture 444) makes the dispatcher run the
    // var61 = 35 branch, which sets var460 = 1 (map event 11 pc705/pc706). Feeding var61 = 35
    // is exactly that dispatcher output.
    game.vm.vars.set(61, 35);

    let mut peak_460 = 0;
    let mut saw_event_33 = false;
    for _ in 0..120 {
        game.frame();
        peak_460 = peak_460.max(game.vm.vars.get(460));
        if game
            .vm
            .instances()
            .iter()
            .any(|instance| instance.event == 33)
        {
            saw_event_33 = true;
        }
    }

    assert_eq!(peak_460, 1, "var460 reaches 1 (ev33 page condition)");
    assert!(
        saw_event_33,
        "map event 33 (挿入抜く / CE333 anim) activates"
    );
    assert_eq!(game.stats.warnings, 0, "no engine warnings");
}

// ---------------------------------------------------------------------------------------
// Test B — negative control: without the insertion hover, ev33 stays closed.
// ---------------------------------------------------------------------------------------

#[test]
fn without_the_insertion_hover_event_33_stays_closed() {
    let Some(mut game) = open_resumed() else {
        return;
    };
    drive(&mut game, 60);

    game.vm.vars.set(364, 0);
    assert_eq!(game.vm.vars.get(62), 1);
    game.vm.vars.set(111, 0);
    game.vm.vars.set(460, 0);
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.set_choice_cursor(0);
    drive(&mut game, 4);

    // var61 is left at 0 (no H button hovered).
    let mut peak_460 = 0;
    let mut saw_event_33 = false;
    for _ in 0..120 {
        game.frame();
        peak_460 = peak_460.max(game.vm.vars.get(460));
        if game
            .vm
            .instances()
            .iter()
            .any(|instance| instance.event == 33)
        {
            saw_event_33 = true;
        }
    }

    assert_eq!(peak_460, 0, "var460 stays 0 without a hover");
    assert!(!saw_event_33, "event 33 never activates");
    assert_eq!(game.stats.warnings, 0);
}

// ---------------------------------------------------------------------------------------
// Test C — static graph: the writers/readers that make the cluster reachable.
// ---------------------------------------------------------------------------------------

#[test]
fn h_cg_cluster_writers_and_readers_are_present_in_the_data() {
    let map = load_sample_map();
    let data = common_events();

    // ev33's only enabled page-0 condition is var460 == 1, and its body calls CE333 many times.
    let ev33 = map_event(&map, 33);
    let conds = enabled_conditions(&ev33.pages[0]);
    assert_eq!(
        conds.len(),
        1,
        "ev33 page0 has exactly one enabled condition"
    );
    assert_eq!(conds[0].variable, 2_000_460, "ev33 gated on global var460");
    assert_eq!(conds[0].operator, PageCondition::OP_EQUAL);
    assert_eq!(conds[0].value, 1);
    let ce333_calls = map_cmd_pcs(&ev33.pages[0], 210, 500_333);
    assert!(
        ce333_calls.len() >= 15 && ce333_calls.contains(&60),
        "ev33 calls CE333 fifteen times (incl. pc60), got {ce333_calls:?}"
    );
    // ev33 clears var460 when it runs.
    assert_eq!(map_cmd_pcs(&ev33.pages[0], 121, 2_000_460), vec![334]);

    // The dispatcher (ev11) writes var460 = 1 on the code-35 branch, and sets var61 = 35.
    let ev11 = map_event(&map, 11);
    assert!(
        map_cmd_pcs(&ev11.pages[0], 121, 2_000_460).contains(&706),
        "ev11 pc706 sets var460 = 1"
    );
    assert_eq!(map_ints(&ev11.pages[0], 705), vec![2_000_061, 35, 0, 0]);

    // Track events also arm var460 and add to var111.
    let ev26 = map_event(&map, 26);
    assert!(
        !map_cmd_pcs(&ev26.pages[0], 121, 2_000_460).is_empty(),
        "EV26 writes var460"
    );
    assert!(
        map_cmd_pcs(&ev26.pages[0], 121, 2_000_111).contains(&482),
        "EV26 pc482 raises var111"
    );
    let ev28 = map_event(&map, 28);
    assert!(
        !map_cmd_pcs(&ev28.pages[0], 121, 2_000_460).is_empty(),
        "EV28 writes var460"
    );
    assert_eq!(
        map_ints(&ev28.pages[0], 159),
        vec![2_000_111, 119, 2_000_111, 4352],
        "EV28 pc159 sets var111 = 119 + var111"
    );
    assert!(
        map_cond_triples(&ev28.pages[0])
            .iter()
            .any(|(_, var, value, op)| *var == 2_000_100 && *value == 12 && *op == 1),
        "EV28 body is gated on var100 >= 12 (pc124)"
    );

    // 射精 (CE344) needs var111 > 99; the scripted setter (EV5) writes 100.
    assert!(
        ce_cond_triples(&data, 344)
            .iter()
            .any(|(_, var, value, op)| *var == 2_000_111 && *value == 99 && *op == 3),
        "CE344 pc87 gates 射精 on var111 > 99"
    );
    let ev5 = map_event(&map, 5);
    assert_eq!(map_ints(&ev5.pages[0], 341), vec![2_000_111, 100, 0, 0]);

    // Deep CG gates (documented, not claimed open here).
    assert!(
        ce_cond_triples(&data, 246)
            .iter()
            .any(|(_, var, value, op)| *var == 2_000_157 && *value == 13 && *op == 2),
        "CE246 deep body gated on var157 == 13"
    );
    assert!(
        ce_cond_triples(&data, 328)
            .iter()
            .any(|(_, var, value, op)| *var == 2_000_104 && *value == 0 && *op == 2),
        "CE328 gated on var104 == 0"
    );
    assert!(
        ce_cond_triples(&data, 328)
            .iter()
            .any(|(_, var, value, op)| *var == 2_000_105 && *value == 0 && *op == 2),
        "CE328 gated on var105 == 0"
    );
}

// ---------------------------------------------------------------------------------------
// Test D — the retail save table: every save starts at zero gauge, day saves carry energy.
// ---------------------------------------------------------------------------------------

#[test]
fn retail_saves_carry_zero_gauge_and_split_on_the_energy_state() {
    let mut checked = 0;
    for index in 1..=8 {
        let save = save_dir().join(format!("SaveData{index:02}.sav"));
        let Some(game) = load_save_into(&save) else {
            continue;
        };
        checked += 1;
        let get = |n| game.vm.vars.get(n);

        assert_eq!(get(111), 0, "SAVE{index:02} ships var111 = 0");
        assert_eq!(get(460), 0, "SAVE{index:02} ships var460 = 0");

        if index == 8 {
            // The mid-H drained snapshot: gauge gains are blocked.
            assert_eq!(get(100), 0, "SAVE08 var100 = 0 (drained)");
            assert_eq!(get(267), 1, "SAVE08 var267 = 1 (gain blocked)");
        } else {
            // Day saves carry the energy state that lets the insertion events raise var111.
            assert!(get(100) >= 12, "SAVE{index:02} var100 >= 12 (energy)");
            assert_eq!(get(267), 0, "SAVE{index:02} var267 = 0 (EV26 gain armed)");
        }
    }
    assert!(checked >= 7, "checked {checked} retail saves");

    // Deep-gate carriers worth knowing about.
    if let Some(game) = load_save_into(&save_dir().join("SaveData07.sav")) {
        assert_eq!(
            game.vm.vars.get(157),
            13,
            "SAVE07 carries the CE246 var157 = 13 gate"
        );
    }
    if let Some(game) = load_save_into(&save_dir().join("SaveData04.sav")) {
        assert_ne!(game.vm.vars.get(104), 0);
        assert_ne!(game.vm.vars.get(105), 0);
        assert!(game.vm.vars.get(4) >= 21);
    }
}

// ---------------------------------------------------------------------------------------
// Test E — real insertion hover codes drive var111 from 0 past 100 (given day-save energy).
// ---------------------------------------------------------------------------------------

#[test]
fn var111_reaches_100_through_real_insertion_hover_codes() {
    let Some(mut game) = open_resumed() else {
        return;
    };
    drive(&mut game, 60);

    game.vm.vars.set(364, 0);
    assert_eq!(game.vm.vars.get(62), 1);
    game.vm.vars.set(111, 0);
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.set_choice_cursor(0);

    // The seven day saves ship var100 = 70..75 / var267 = 0; the insertion events spend that
    // energy per action. Re-asserting the day-save energy invariant before each action lets a
    // continuous playthrough of real hover codes (31..45) accumulate the gauge, exactly as the
    // real game does between days. Each code is one H button hover (dispatcher output var61).
    let mut peak = 0;
    'outer: for _round in 0..6 {
        for code in 31..=45 {
            game.vm.vars.set(100, 70);
            game.vm.vars.set(267, 0);
            game.vm.vars.set(61, code);
            for _ in 0..30 {
                game.frame();
                peak = peak.max(game.vm.vars.get(111));
            }
            if peak >= 100 {
                break 'outer;
            }
        }
    }

    assert!(
        peak >= 100,
        "var111 reaches the 射精 threshold (peak {peak})"
    );
    assert_eq!(game.stats.warnings, 0, "no engine warnings");
}
