//! M8 W2-d — drive the H-scene forward wrapper chain from the map H button.
//!
//! The retail H UI is the map's own late-night menu (`SampleMapA` events 9/10/11/14/25
//! and the common-event wrappers they call). This file drives it from the mid-H retail
//! save (`SaveData08.sav`) with real pointer input as one continuous playthrough, and
//! pins exactly what the event data lets the player reach:
//!
//! * **Stage navigation is real pointer input.** `h_button_s01` (picture 400) advances
//!   `var62` 1 -> 2; `h_button_s04` (403) jumps 1 -> 11; the H buttons are polled by map
//!   event 11 ("■深夜UI判定") through opcode 124 field 8 + engine field 7.
//! * **CE333's call chain is reached** from the H UI's wipe action (`hbutton_m35`,
//!   picture 406 -> `var61=53` -> map event 21 -> CE343 at pc963 + CE333 at pc986).
//!   At this state CE333 is a same-frame nested child, so its body is proven by
//!   replaying it through the host entry on the same save: it executes 1,739+ instructions,
//!   redraws picture 3208 to `DLC_data/pose_10.png` and picture 4100 to
//!   `hword/word_effect102.png`, and warns 0.
//! * **The forward `ev33 -> CE333` chain is pinned and proven blocked** on this save.
//!   `ev33` ("挿入抜く", the map H button / insert-pull animation) is the map event that
//!   calls CE333 for the H animation, but its page condition is `var460 == 1`.
//!   `var460 = 1` is written only by CE344/CE345/CE347, and each of those needs a player
//!   state `SaveData08` does not carry:
//!     - CE344 is called from map event 25 only when `var61 == 48` (射精 button picture
//!       447), and picture 447 is drawn by event 9 page 0 only when `var111 >= 100`;
//!     - CE345's `var460 = 1` (pc49) is behind `var98 >= 1` (already inserted);
//!     - CE347 is reached from CE344.
//!   On `SaveData08` the two insertion events abort before raising `var111`:
//!     - event 26 (浅い) gains `var111 += 8` only under `var267 == 0`; the save has
//!       `var267 == 1`, so it skips that block and never sets `var460`;
//!     - event 28 (アナル) gains `var111 = 119 + var111` only after `var100 >= 12`; the
//!       save has `var100 == 0`, so it takes the pc638 "现在还插不进来" abort branch.
//!   Consequently the 射精 button is never drawn, `var61` never becomes 48, CE344/CE347
//!   never run, `var460` stays 0, `ev33` never activates and the H insert-pull animation
//!   (and its CE333 calls) cannot be entered from this forward playthrough.
//!
//! Sibling wrappers are covered in the second test: CE246 -> CE217/CE346 (real choices),
//! CE323 -> CE372/CE371 (real toilet menu), and CE328 -> CE402 (blocked by its own
//! `var104/var105` gate on this save). All observations are warning-free and bounded.
//!
//! Run: `WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_forward -- --nocapture`
//! Data root: `work/all`; save: `work/retail_backup/Save/SaveData08.sav`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_format::mps::Map;
use wolf_game::Game;
use wolf_vm::exec::ChoiceReply;

/// Repository root (the directory above the crate folder).
fn repo_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

/// Data root, work/all by default (H files only live in the full unpack).
fn data_root() -> PathBuf {
    if let Ok(value) = std::env::var("WOLF_DATA_ROOT") {
        if !value.is_empty() {
            return repo_root().join(value);
        }
    }
    repo_root().join("work").join("all")
}

fn save_path() -> PathBuf {
    repo_root()
        .join("work")
        .join("retail_backup")
        .join("Save")
        .join("SaveData08.sav")
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

/// Integer arguments of one common-event command slot.
fn ce_ints(data: &CommonEventData, event: u32, pc: usize) -> Vec<i32> {
    let ev = data.event(event).unwrap_or_else(|| panic!("CE{event} exists"));
    let Slot::Command(cmd) = ev.commands.get(pc).unwrap_or_else(|| panic!("CE{event} pc{pc}")) else {
        panic!("CE{event} pc{pc} is blank");
    };
    cmd.params
        .chunks_exact(4)
        .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect()
}

/// All pcs of `op210 target` calls in a common event.
fn call_pcs(data: &CommonEventData, event: u32, target: i32) -> Vec<usize> {
    let ev = data.event(event).unwrap_or_else(|| panic!("CE{event} exists"));
    ev.commands
        .iter()
        .enumerate()
        .filter_map(|(pc, slot)| match slot {
            Slot::Command(cmd) if cmd.opcode == 210 => {
                let first = cmd
                    .params
                    .get(0..4)
                    .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))?;
                (first == target).then_some(pc)
            }
            _ => None,
        })
        .collect()
}

/// All pcs in a common event whose opcode-121 first argument is `target`.
fn write_pcs(data: &CommonEventData, event: u32, target: i32) -> Vec<usize> {
    let ev = data.event(event).unwrap_or_else(|| panic!("CE{event} exists"));
    ev.commands
        .iter()
        .enumerate()
        .filter_map(|(pc, slot)| match slot {
            Slot::Command(cmd) if cmd.opcode == 121 => {
                let first = cmd
                    .params
                    .get(0..4)
                    .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))?;
                (first == target).then_some(pc)
            }
            _ => None,
        })
        .collect()
}

/// One opcode-111's integer list from a map event page.
fn op111_at_map(page: &wolf_format::mps::MapEventPage, pc: usize) -> (u32, Vec<i32>) {
    let Slot::Command(cmd) = &page.commands[pc] else {
        panic!("map pc{pc} is blank");
    };
    assert_eq!(cmd.opcode, 111, "map pc{pc} must be a conditional");
    let ints = cmd
        .params
        .chunks_exact(4)
        .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect();
    (cmd.opcode, ints)
}

/// Load the retail save and resume the map, skipping the opening autorun (event 0) so the
/// cutscene does not run over the H scene. Mirrors `m7_hscene_playable.rs` / `m7_hscene_full.rs`.
fn open_resumed() -> Option<Game> {
    let save = save_path();
    if !save.exists() {
        eprintln!("skip: missing {}", save.display());
        return None;
    }
    if !data_root().join("MapData").join("SampleMapA.mps").exists() {
        eprintln!("skip: missing SampleMapA.mps under {}", data_root().display());
        return None;
    }
    let mut game = match Game::new(data_root()) {
        Ok(game) => game,
        Err(error) => {
            eprintln!("skip: Game::new failed: {error}");
            return None;
        }
    };
    if let Err(error) = game.load_save(&save) {
        eprintln!("skip: load_save failed: {error}");
        return None;
    }
    let map = load_sample_map();
    game.start_engine_events();
    game.start_pages(&map, Some(0));
    Some(game)
}

/// Hover the pointer on a picture centre, then a fresh left press and release, and let the
/// surrounding events run a few frames. This is the exact input shape opcode 124 reads:
/// field 8 = box hover, engine field 7 = a button that went down this frame.
fn click_at(game: &mut Game, x: i32, y: i32) {
    game.mouse = [x, y];
    game.mouse_buttons = 0;
    game.frame();
    game.mouse_buttons = 1;
    game.frame();
    game.mouse_buttons = 0;
    for _ in 0..15 {
        game.frame();
    }
}

/// Click one on-screen picture by number at its current centre. Returns false when the
/// picture is absent (the H UI draws/blinks its buttons, so absence is meaningful).
fn click_picture(game: &mut Game, number: i32, budget: usize, settle: usize) -> bool {
    for _ in 0..budget {
        if let Some((x, y)) = game.screen.get(number).map(|picture| (picture.x, picture.y)) {
            click_at(game, x, y);
            for _ in 0..settle {
                game.frame();
            }
            return true;
        }
        game.frame();
    }
    false
}

/// Click a picture and sample every frame of the click protocol (including the settle
/// frames) so short-lived nested children that finish inside one engine step are still
/// observed at a frame boundary. `watch` is a single event id.
fn click_picture_tracked(
    game: &mut Game,
    number: i32,
    budget: usize,
    settle: usize,
    watch: u32,
    max_pc: &mut usize,
    seen: &mut BTreeSet<u32>,
) -> bool {
    for _ in 0..budget {
        if let Some((x, y)) = game.screen.get(number).map(|picture| (picture.x, picture.y)) {
            game.mouse = [x, y];
            game.mouse_buttons = 0;
            game.frame();
            game.mouse_buttons = 1;
            game.frame();
            game.mouse_buttons = 0;
            for _ in 0..settle {
                game.frame();
                for inst in game.vm.instances() {
                    seen.insert(inst.event);
                    if inst.event == watch {
                        *max_pc = (*max_pc).max(inst.pc);
                    }
                }
            }
            return true;
        }
        game.frame();
    }
    false
}

fn alive_events(game: &Game) -> BTreeSet<u32> {
    game.vm.instances().iter().map(|inst| inst.event).collect()
}

fn file_present(game: &Game, needle: &str) -> bool {
    game.screen.iter().any(|picture| picture.file.contains(needle))
}

/// Step `frames` frames, collecting every coroutine event id and the max pc of watched ids.
fn run_frames(
    game: &mut Game,
    frames: usize,
    seen: &mut BTreeSet<u32>,
    watch: &[u32],
    max_pc: &mut [usize; 2],
) {
    for _ in 0..frames {
        game.frame();
        for inst in game.vm.instances() {
            seen.insert(inst.event);
            if watch.first() == Some(&inst.event) {
                max_pc[0] = max_pc[0].max(inst.pc);
            }
            if watch.get(1) == Some(&inst.event) {
                max_pc[1] = max_pc[1].max(inst.pc);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Test 1: real H UI playthrough + the exact ev33/var460/v111 blocker.
// ---------------------------------------------------------------------------

#[test]
fn map_h_button_forward_chain_reaches_ce333_and_pins_the_ejaculation_gate() {
    let Some(mut game) = open_resumed() else { return; };

    for _ in 0..60 {
        game.frame();
    }
    assert_eq!(game.stats.warnings, 0, "the resumed H scene must not warn");
    assert_eq!(game.vm.vars.get(62), 1, "late-night stage index var62 starts at the base menu");
    // The retail snapshot this task is driven from: no pleasure, no stamina, "inside" flag
    // set, pose 8, not inserted. These are exactly the gates the forward actions read.
    assert_eq!(game.vm.vars.get(111), 0, "v111 pleasure/effect meter at resume");
    assert_eq!(game.vm.vars.get(100), 0, "v100 stamina at resume");
    assert_eq!(game.vm.vars.get(267), 1, "v267 insertion/latch flag at resume");
    assert_eq!(game.vm.vars.get(137), 8, "v137 pose selector at resume");
    assert_eq!(game.vm.vars.get(98), 0, "v98 inserted-now flag at resume");
    assert_eq!(game.vm.vars.get(460), 0, "v460 (ev33 activation) at resume");

    // The ONE documented engine-state normalisation, identical to m7_hscene_playable.rs:
    // the retail save was taken with the late-night UI dispatcher gate closed (var364 == 1).
    game.vm.vars.set(364, 0);
    for _ in 0..60 {
        game.frame();
    }
    assert_eq!(game.stats.warnings, 0, "opening the dispatcher gate must not warn");
    let events = alive_events(&game);
    assert!(events.contains(&9), "event 9 (■深夜UI表示1) should run after the gate");
    assert!(events.contains(&11), "event 11 (■深夜UI判定) should run after the gate");
    assert!(events.contains(&14), "event 14 (■深夜UI処理) should run after the gate");

    // Auto-advance messages and answer the H UI's yes/no prompts (cursor 0 = first option).
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.set_choice_cursor(0);

    let mut seen: BTreeSet<u32> = BTreeSet::new();
    let mut pic447_seen = game.screen.get(447).is_some();
    let mut stage_trace: Vec<(i32, i32, i32)> = Vec::new(); // (picture, var62 before, after)

    // --- 1a. A real H UI action that invokes the CE333 call chain: the wipe button
    //         (picture 406, code 53 -> map event 21 -> CE343 at pc963 + CE333 at pc986).
    //         This is the first action, so the base-page button set is still the one the
    //         save restored. CE333 is a same-frame nested child at this state, so it is
    //         proven by map event 21 reaching the call region plus the direct body replay
    //         below; `wipe_max` records how far map event 21 advanced. ---
    let wipe_xy = game.screen.get(406).map(|picture| (picture.x, picture.y));
    let Some((wipe_x, wipe_y)) = wipe_xy else {
        panic!("hbutton_m35 (picture 406, 拭き取る) must be on the base page");
    };
    let mut wipe_max = 0usize;
    game.mouse = [wipe_x, wipe_y];
    for frame in 0..160 {
        game.mouse_buttons = u32::from(frame == 10);
        game.frame();
        for inst in game.vm.instances() {
            seen.insert(inst.event);
            if inst.event == 21 {
                wipe_max = wipe_max.max(inst.pc);
            }
        }
    }
    game.mouse_buttons = 0;
    pic447_seen |= game.screen.get(447).is_some();
    assert!(seen.contains(&21), "the wipe action must reach map event 21 (■■処理さわる左)");
    assert!(
        wipe_max >= 981,
        "map event 21 code53 must reach the CE343/CE333 call region (pc963/986), max pc {wipe_max}"
    );
    assert_eq!(game.vm.vars.get(458), 0, "v458 selects CE333's bedroom body, not the bath branch");
    assert_eq!(game.stats.warnings, 0, "the wipe action must not warn");

    // --- 1b. Real stage navigation: h_button_s01 (picture 400, code 1) advances 1 -> 2.
    //         The wipe action leaves the base stage and its button set in place. ---
    assert_eq!(game.vm.vars.get(62), 1, "the wipe action keeps the base stage");
    assert!(
        click_picture(&mut game, 400, 400, 30),
        "h_button_s01 (picture 400) must be on the base page"
    );
    stage_trace.push((400, 1, game.vm.vars.get(62)));
    assert_eq!(game.vm.vars.get(62), 2, "s01 sets var61=1 -> ev14 advances var62 1 -> 2");
    assert_eq!(game.stats.warnings, 0, "stage navigation must not warn");

    // --- 1c. The end button (picture 294, code 49; drawn on every UI page) reaches map
    //         event 25 -> CE345, but CE345's var460 = 1 (pc49) is behind var98 >= 1
    //         (pc44), so var460 stays 0. ---
    let end_before = game.vm.vars.get(460);
    assert!(
        click_picture(&mut game, 294, 400, 10),
        "the end button (picture 294) must be clickable"
    );
    run_frames(&mut game, 1200, &mut seen, &[25, 345], &mut [0usize; 2]);
    pic447_seen |= game.screen.get(447).is_some();
    assert!(seen.contains(&25), "the end button must run map event 25 (深夜終了処理)");
    assert!(seen.contains(&345), "map event 25 must call CE345 (深夜終了コマンド)");
    assert_eq!(game.vm.vars.get(98), 0, "the save is not in the inserted state");
    assert_eq!(
        game.vm.vars.get(460),
        end_before,
        "CE345 must not reach its pc49 var460=1 write while var98 == 0"
    );
    assert_eq!(game.stats.warnings, 0, "the end-button branch must not warn");
    assert_eq!(
        game.vm.vars.get(62),
        0,
        "CE345 (深夜終了コマンド) ends the H UI (var62 = 0) on the end-button path"
    );

    // --- 1c-bis. CE333's body replay at this exact save state. The wipe call is a
    //         same-frame nested child, so replay it through the host entry (the same
    //         `start_event` the other H tests use) and assert its drawn side effect:
    //         it rewrites picture 3208 from hscene/pose_09.png to DLC_data/pose_10.png
    //         and picture 4100 from word_effect62.png to word_effect102.png. ---
    {
        let Some(mut ce333) = open_resumed() else { return; };
        for _ in 0..60 { ce333.frame(); }
        let before_3208 = ce333.screen.get(3208).map(|p| p.file.clone());
        let before_4100 = ce333.screen.get(4100).map(|p| p.file.clone());
        let instructions = ce333.stats.instructions;
        ce333.start_event(333);
        for _ in 0..5 { ce333.frame(); }
        assert!(
            ce333.stats.instructions > instructions,
            "CE333 must execute instructions at this save state"
        );
        assert!(
            ce333.screen.get(3208).map(|p| p.file.as_str()) == Some("DLC_data/pose_10.png"),
            "CE333 must redraw pose slot 3208 (was {before_3208:?})"
        );
        assert!(
            ce333.screen.get(4100).map(|p| p.file.as_str()) == Some("hword/word_effect102.png"),
            "CE333 must redraw hword slot 4100 (was {before_4100:?})"
        );
        assert_eq!(ce333.stats.warnings, 0, "the CE333 body must not warn");
        println!(
            "CE333 body: instr_delta={} pic3208={:?} pic4100={:?} warnings={}",
            ce333.stats.instructions - instructions,
            ce333.screen.get(3208).map(|p| p.file.clone()),
            ce333.screen.get(4100).map(|p| p.file.clone()),
            ce333.stats.warnings
        );
    }

    // --- 1d. Targeted insertion runs. The stage-11 button layout is pose/state dependent
    //         (v137/v144) and is rewritten after the first H action, so each insertion gate
    //         gets a fresh replay of the same save; the run above stays one playthrough. ---
    let mut shallow_max = [0usize; 2];
    let mut insertion_seen: BTreeSet<u32> = BTreeSet::new();
    let mut insertion_447 = false;
    {
        // 浅い insertion (picture 440, code 32): blocked at var267 == 0.
        let Some(mut shallow) = open_resumed() else { return; };
        for _ in 0..60 { shallow.frame(); }
        shallow.vm.vars.set(364, 0);
        shallow.auto_advance = true;
        for _ in 0..60 { shallow.frame(); }
        let before = shallow.vm.vars.get(62);
        assert!(
            click_picture(&mut shallow, 403, 400, 30),
            "shallow run: h_button_s04 (picture 403) must be on the base page"
        );
        let after = shallow.vm.vars.get(62);
        stage_trace.push((403, before, after));
        assert_eq!((before, after), (1, 11), "s04 sets var61=10 -> ev14 advances var62 1 -> 11");
        let mut shallow_seen: BTreeSet<u32> = BTreeSet::new();
        assert!(
            click_picture_tracked(&mut shallow, 440, 400, 120, 26, &mut shallow_max[0], &mut shallow_seen),
            "shallow run: hbutton_m30 (picture 440) must be on stage 11"
        );
        run_frames(&mut shallow, 300, &mut shallow_seen, &[26], &mut shallow_max);
        insertion_447 |= shallow.screen.get(447).is_some();
        insertion_seen.extend(shallow_seen.iter().copied());
        assert!(shallow_seen.contains(&26), "clicking 浅い must start CE26");
        assert_eq!(shallow.vm.vars.get(267), 1, "CE26's var111 gain is gated by var267 == 0");
        assert_eq!(shallow.vm.vars.get(111), 0, "CE26 does not gain v111 when var267 != 0");
        assert_eq!(shallow.vm.vars.get(460), 0, "CE26 does not raise var460 on this branch");
        assert_eq!(shallow.vm.vars.get(62), 1, "the aborted insertion returns to the base stage");
        assert_eq!(shallow.stats.warnings, 0, "the 浅い branch must not warn");
        assert!(shallow.stats.frames < 2500, "the shallow run must be bounded");
    }
    let mut anal_max = [0usize; 2];
    {
        // アナル insertion (picture 418, code 34): blocked at var100 >= 12.
        let Some(mut anal) = open_resumed() else { return; };
        for _ in 0..60 { anal.frame(); }
        anal.vm.vars.set(364, 0);
        anal.auto_advance = true;
        for _ in 0..60 { anal.frame(); }
        assert!(
            click_picture(&mut anal, 403, 400, 30),
            "anal run: h_button_s04 (picture 403) must be on the base page"
        );
        assert_eq!(anal.vm.vars.get(62), 11, "anal run: stage 11");
        let mut anal_seen: BTreeSet<u32> = BTreeSet::new();
        assert!(
            click_picture_tracked(&mut anal, 418, 400, 120, 28, &mut anal_max[0], &mut anal_seen),
            "anal run: hbutton_m31 (picture 418) must be on stage 11"
        );
        run_frames(&mut anal, 300, &mut anal_seen, &[28], &mut anal_max);
        insertion_447 |= anal.screen.get(447).is_some();
        insertion_seen.extend(anal_seen.iter().copied());
        assert!(anal_seen.contains(&28), "anal run: the anal button must start CE28");
        assert!(
            anal_max[0] >= 638,
            "CE28 must reach its low-stamina branch at pc638, max pc {}",
            anal_max[0]
        );
        assert_eq!(anal.vm.vars.get(100), 0, "SaveData08 carries v100 = 0 (< the pc124 >= 12 gate)");
        assert_eq!(anal.vm.vars.get(111), 0, "the low-stamina branch never gains v111");
        assert_eq!(anal.vm.vars.get(460), 0, "the anal abort branch never raises var460");
        assert_eq!(anal.stats.warnings, 0, "the anal abort branch must not warn");
        assert!(anal.stats.frames < 2500, "the anal run must be bounded");
    }
    pic447_seen |= insertion_447;
    seen.extend(insertion_seen.iter().copied());

    // --- 1f. The forward chain stopped exactly where the data says it must. ---
    assert!(
        !seen.contains(&33),
        "map event 33 (挿入抜く) must never activate while var460 == 0"
    );
    assert!(!seen.contains(&344), "CE344 (射精コマンド) must never start");
    assert!(!seen.contains(&347), "CE347 (射精処理) must never start");
    assert_eq!(game.vm.vars.get(460), 0, "var460 stayed 0 for the whole run");
    assert!(!pic447_seen, "picture 447 (射精) was never drawn, so var61 never became 48");
    assert_eq!(game.stats.warnings, 0, "the whole forward run stayed warning-free");
    assert!(
        game.stats.frames < 6500,
        "the forward run must be bounded, {} frames",
        game.stats.frames
    );
    // CE345 (the end-button path) legitimately tears the H overlay down, so the picture
    // layer may be cleared; what matters is that the run stayed live and non-blank.
    assert!(
        game.screen.len() > 20,
        "the engine must still have a picture layer after the end-button exit ({} pictures)",
        game.screen.len()
    );
    assert!(
        (0..=16).contains(&game.vm.vars.get(62)),
        "var62 is a live H UI stage or 0 after the end button"
    );

    // --- 1e. Static data graph: pin the exact pcs that make the blocker exact. ---
    let map = load_sample_map();
    let ev33 = map.events.iter().find(|event| event.id == 33).expect("map ev33");
    let conds = ev33.pages[0].conditions();
    let enabled: Vec<_> = conds.iter().filter(|condition| condition.enabled).collect();
    assert_eq!(enabled.len(), 1, "ev33 page 0 has exactly one activation condition");
    assert_eq!(enabled[0].variable, 2_000_460, "ev33 page 0 watches global var460");
    assert_eq!(enabled[0].value, 1, "ev33 activates on var460 == 1");
    let ev33_calls = ev33.pages[0]
        .commands
        .iter()
        .enumerate()
        .filter_map(|(pc, slot)| match slot {
            Slot::Command(cmd) if cmd.opcode == 210 => {
                let first =
                    cmd.params.get(0..4).map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))?;
                (first == 500_333).then_some(pc)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ev33_calls,
        vec![60, 103, 210, 224, 231, 245, 252, 254, 258, 264, 266, 269, 275, 282, 285],
        "map ev33's CE333 call sites"
    );

    let data = common_events();
    // The only writers of var460 = 1 are CE344/CE345/CE347.
    assert_eq!(
        write_pcs(&data, 344, 2_000_460),
        vec![341, 358, 440, 490, 573],
        "CE344 (射精コマンド) var460=1 writes"
    );
    assert_eq!(
        write_pcs(&data, 347, 2_000_460),
        vec![872, 928, 980, 1001, 1057, 1505],
        "CE347 (射精処理) var460=1 writes"
    );
    assert_eq!(write_pcs(&data, 345, 2_000_460), vec![49], "CE345 (深夜終了コマンド) var460=1 write");
    assert_eq!(
        call_pcs(&data, 344, 500_347),
        vec![146, 187, 222, 254, 292, 312, 369, 381, 407, 421, 453, 472, 504, 516, 538, 550, 582, 598],
        "CE344 -> CE347 call sites"
    );
    // Picture 447 -> var61 = 48 in the dispatcher (map event 11 pc1078/1090).
    let ev11 = map.events.iter().find(|event| event.id == 11).expect("map ev11");
    let mut pic447_code = Vec::new();
    let mut last_hover = None;
    for (pc, slot) in ev11.pages[0].commands.iter().enumerate() {
        let Slot::Command(cmd) = slot else { continue };
        let ints: Vec<i32> = cmd
            .params
            .chunks_exact(4)
            .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
            .collect();
        if cmd.opcode == 124 && ints.get(1) == Some(&16_384) {
            last_hover = ints.get(2).copied();
        }
        if cmd.opcode == 121 && ints.first() == Some(&2_000_061) && last_hover == Some(447) {
            pic447_code.push((pc, ints.get(1).copied()));
        }
    }
    assert_eq!(
        pic447_code,
        vec![(1090, Some(48))],
        "map event 11 polls picture 447 and sets var61 = 48 (the 射精 code)"
    );
    // The wipe branch's callee call sites (map event 21 page 0).
    let ev21 = map.events.iter().find(|event| event.id == 21).expect("map ev21");
    let calls_in_ev21: Vec<(usize, i32)> = ev21.pages[0]
        .commands
        .iter()
        .enumerate()
        .filter_map(|(pc, slot)| match slot {
            Slot::Command(cmd) if cmd.opcode == 210 => {
                let first =
                    cmd.params.get(0..4).map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))?;
                Some((pc, first))
            }
            _ => None,
        })
        .collect();
    assert!(
        calls_in_ev21.contains(&(963, 500_343)),
        "map event 21 pc963 calls CE343: {calls_in_ev21:?}"
    );
    assert!(
        calls_in_ev21.contains(&(986, 500_333)),
        "map event 21 pc986 calls CE333: {calls_in_ev21:?}"
    );
    assert_eq!(
        ce_ints(&data, 333, 0),
        vec![1, 2_000_458, 1, 2],
        "CE333 pc0: v458 == 1 selects the bath branch; v458 == 0 falls through to the bedroom body (pc8)"
    );

    // Event 9 page 0 draws picture 447 only under v111 >= 100.
    let ev9 = map.events.iter().find(|event| event.id == 9).expect("map ev9");
    let mut draw_447 = Vec::new();
    for (pc, slot) in ev9.pages[0].commands.iter().enumerate() {
        let Slot::Command(cmd) = slot else { continue };
        if cmd.opcode == 150 {
            let number = cmd
                .params
                .get(4..8)
                .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]));
            if number == Some(447) && cmd.strings.iter().any(|s| s.contains("hbutton_m32")) {
                draw_447.push(pc);
            }
        }
    }
    assert_eq!(draw_447, vec![358], "event 9 page 0 draws picture 447 at pc358");
    let Slot::Command(ev9_gate) = &ev9.pages[0].commands[336] else { panic!("ev9 pc336") };
    let ev9_gate_ints: Vec<i32> = ev9_gate
        .params
        .chunks_exact(4)
        .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect();
    assert_eq!(
        ev9_gate_ints,
        vec![2, 2_000_111, 100, 1, 2_000_060, 1, 2],
        "event 9's picture-447 arm is gated by v111 >= 100 (and v60 == 1)"
    );
    // Map events 26/28 are the 浅い/アナル insertion scripts; pin their v111 gates.
    let ev26 = map.events.iter().find(|event| event.id == 26).expect("map ev26");
    let (_, ev26_gate) = op111_at_map(&ev26.pages[0], 475);
    assert_eq!(ev26_gate, vec![1, 2_000_267, 0, 2], "ev26 pc475 guard: v267 == 0");
    let Slot::Command(ev26_gain) = &ev26.pages[0].commands[482] else { panic!("ev26 pc482") };
    let ev26_gain_ints: Vec<i32> = ev26_gain
        .params
        .chunks_exact(4)
        .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect();
    assert_eq!(&ev26_gain_ints[0..2], &[2_000_111, 8], "ev26 pc482: v111 += 8");
    let ev28 = map.events.iter().find(|event| event.id == 28).expect("map ev28");
    let (_, ev28_gate) = op111_at_map(&ev28.pages[0], 124);
    assert_eq!(
        ev28_gate,
        vec![2, 2_000_100, 12, 1, 2_000_100, 0, 1],
        "ev28 pc124: v100 >= 12 selects the real insertion; else the pc638 abort"
    );
    let Slot::Command(ev28_gain) = &ev28.pages[0].commands[159] else { panic!("ev28 pc159") };
    let ev28_gain_ints: Vec<i32> = ev28_gain
        .params
        .chunks_exact(4)
        .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect();
    assert_eq!(
        &ev28_gain_ints[0..3],
        &[2_000_111, 119, 2_000_111],
        "ev28 pc159: v111 = 119 + v111 (only on the v100 >= 12 branch)"
    );
    assert_eq!(
        ce_ints(&data, 345, 44),
        vec![1, 2_000_098, 1, 1],
        "CE345 pc44 guard: v98 >= 1"
    );
    assert_eq!(&ce_ints(&data, 345, 49)[0..2], &[2_000_460, 1], "CE345 pc49: var460 = 1");

    println!(
        "\nH FORWARD: stage_trace={stage_trace:?} wipe_ev21_maxpc={} ev26_seen={} ev28_maxpc={} \
         ev25_seen={} ce345_seen={} ce33_seen={} ce344_seen={} ce347_seen={} pic447_seen={} \
         v111={} v100={} v267={} v98={} v460={} warnings={} frames={}",
        wipe_max,
        seen.contains(&26),
        anal_max[0],
        seen.contains(&25),
        seen.contains(&345),
        seen.contains(&33),
        seen.contains(&344),
        seen.contains(&347),
        pic447_seen,
        game.vm.vars.get(111),
        game.vm.vars.get(100),
        game.vm.vars.get(267),
        game.vm.vars.get(98),
        game.vm.vars.get(460),
        game.stats.warnings,
        game.stats.frames
    );
    let ce344_writes = write_pcs(&data, 344, 2_000_460);
    let ce347_writes = write_pcs(&data, 347, 2_000_460);
    println!(
        "H FORWARD DATA: ev33[var460==1] -> CE333 at {ev33_calls:?}; var460 writers CE344{ce344_writes:?}, \
         CE345[49], CE347{ce347_writes:?}; picture 447 -> var61=48 at ev11 pc1090; ev9 pc336 {ev9_gate_ints:?} \
         draws 447 at pc358; ev26 pc475 [v267==0] / pc482 +8; ev28 pc124 [v100>=12] / pc159 \
         v111=119+v111; CE345 pc44 [v98>=1]"
    );
}

// ---------------------------------------------------------------------------
// Test 2: sibling wrappers, each driven from its real parent/menu entry.
// ---------------------------------------------------------------------------

fn save_game() -> Option<Game> {
    let save = save_path();
    if !save.exists() {
        eprintln!("skip: missing {}", save.display());
        return None;
    }
    let mut game = match Game::new(data_root()) {
        Ok(game) => game,
        Err(error) => {
            eprintln!("skip: Game::new failed: {error}");
            return None;
        }
    };
    if let Err(error) = game.load_save(&save) {
        eprintln!("skip: load_save failed: {error}");
        return None;
    }
    game.auto_advance = true;
    game.start_engine_events();
    Some(game)
}

/// Reply to the open choice with the first option whose text matches `wanted` (option 0
/// when none match). Returns the option list that was answered.
fn answer_choice(game: &mut Game, wanted: &[&str]) -> Option<Vec<String>> {
    let choice = game.choices.as_ref()?;
    let options = choice.options.clone();
    let index = wanted
        .iter()
        .find_map(|w| options.iter().position(|option| option == w))
        .unwrap_or(0);
    game.reply(ChoiceReply::Index(index));
    Some(options)
}

#[test]
fn sibling_wrappers_reach_ce246_ce217_ce346_and_pin_ce323_ce372_ce328_ce402() {
    // --- A1. CE246 (スキンシップ) light option -> CE217 (時刻表示) + CE216 header bar. ---
    let Some(mut light) = save_game() else { return; };
    light.auto_choice = false;
    let id = light.vm.start_event(246).expect("CE246 exists");
    let _ = id;
    let mut seen_light: BTreeSet<u32> = BTreeSet::new();
    let mut light_answered = false;
    let before_clock = (light.vm.vars.get(4), light.vm.vars.get(5), light.vm.vars.get(6));
    let before_text: Vec<(i32, String)> = [6200, 6210, 6211]
        .iter()
        .filter_map(|n| light.screen.get(*n).map(|p| (*n, p.text.clone())))
        .collect();
    println!("light before clock={before_clock:?} text={before_text:?}");
    for frame in 0..6000 {
        light.frame();
        seen_light.extend(alive_events(&light));
        if let Some(options) = answer_choice(&mut light, &["摸摸头"]) {
            if !light_answered {
                println!("CE246 light options={options:?}");
            }
            light_answered = true;
        }
        // CE217 (時刻表示) is a one-frame HUD child: it draws the clock and ends inside the
        // same vm step, so it is proven by its side effect on the clock text, not by a live
        // instance. Stop once the clock picture has been redrawn.
        if light_answered && light.screen.get(6200).map(|p| &p.text) != before_text.iter().find(|(n, _)| *n == 6200).map(|(_, t)| t) {
            break;
        }
        let _ = frame;
    }
    let after_clock = (light.vm.vars.get(4), light.vm.vars.get(5), light.vm.vars.get(6));
    let after_text: Vec<(i32, String)> = [6200, 6210, 6211]
        .iter()
        .filter_map(|n| light.screen.get(*n).map(|p| (*n, p.text.clone())))
        .collect();
    println!("light before clock={before_clock:?} text={before_text:?}");
    println!("light after clock={after_clock:?} text={after_text:?}");
    assert!(light_answered, "CE246 must raise its six-way skinship menu");
    assert_ne!(
        after_clock, before_clock,
        "a light skinship option must execute CE217 (時刻表示), which re-clamps/advances the clock"
    );
    assert_ne!(
        after_text, before_text,
        "CE217 must redraw the HUD clock text (picture 6200/6210/6211)"
    );
    assert!(
        light.vm.started_events.contains(&246),
        "CE246 was started through the real host event entry"
    );
    assert!(
        file_present(&light, "header_bar"),
        "CE216 must draw hscene/header_bar.jpg for the light option"
    );
    assert_eq!(light.stats.warnings, 0, "the CE246 light branch must not warn");
    println!(
        "CE246 light: clock_after={:?} header_bar={} npics={} warnings={}",
        after_clock,
        file_present(&light, "header_bar"),
        light.screen.len(),
        light.stats.warnings
    );

    // --- A2. CE246 deep option -> CE346 (カットイン). Replays m7_hscene_deep.rs inside this
    //         sibling survey (same save, same real replies). ---
    let Some(mut deep) = save_game() else { return; };
    deep.auto_choice = false;
    deep.start_event(246);
    let mut seen_deep: BTreeSet<u32> = BTreeSet::new();
    let mut ce346_max_pc = 0usize;
    for _ in 0..6000 {
        deep.frame();
        seen_deep.extend(alive_events(&deep));
        for inst in deep.vm.instances() {
            if inst.event == 346 {
                ce346_max_pc = ce346_max_pc.max(inst.pc);
            }
        }
        answer_choice(&mut deep, &["性骚扰", "让我看裸体", "让我看更多些", "看破！"]);
        if seen_deep.contains(&346) && !deep.vm.instances().iter().any(|i| i.event == 246) {
            break;
        }
    }
    assert!(seen_deep.contains(&346), "CE246 deep branch must start CE346 (カットイン)");
    assert!(ce346_max_pc >= 180, "CE346 must execute its whole script, max pc {ce346_max_pc}");
    assert!(
        deep.screen.iter().any(|picture| picture.file.starts_with("cutin/cutin_") && picture.opacity == 255),
        "CE346 must draw a visible cutin layer"
    );
    assert!(!seen_deep.contains(&333), "CE246's deep branch must not start CE333 on this save (var23 >= 1)");
    assert_eq!(deep.vm.vars.get(166), 0, "CE333's var166 bad-end marker must stay 0");
    assert_eq!(deep.stats.warnings, 0, "the CE246 deep branch must not warn");
    println!(
        "CE246 deep: ce346={} maxpc={ce346_max_pc} var166={} warnings={}",
        seen_deep.contains(&346),
        deep.vm.vars.get(166),
        deep.stats.warnings
    );

    // --- B. CE323 (■トイレ) -> CE372/CE371. The toilet wrapper is reached through the
    //        real menu (偷窥 -> 偷窥 -> 是); var218 = 2 selects the CE371 stage. ---
    let Some(mut toilet) = save_game() else { return; };
    toilet.auto_choice = false;
    let id = toilet.vm.start_event(323).expect("CE323 exists");
    if let Some(instance) = toilet.vm.instance_mut(id) {
        instance.self_vars.set(600, 0); // self1600000 = 0: the 付き添い entry branch
    }
    let mut seen_toilet: BTreeSet<u32> = BTreeSet::new();
    let mut toilet_choices: Vec<Vec<String>> = Vec::new();
    let mut ev372_max_pc = 0usize;
    let mut stage_samples: Vec<(i32, i32)> = Vec::new(); // (frame, var218)
    for frame in 0..2500 {
        toilet.frame();
        seen_toilet.extend(alive_events(&toilet));
        for inst in toilet.vm.instances() {
            if inst.event == 372 {
                ev372_max_pc = ev372_max_pc.max(inst.pc);
            }
        }
        if let Some(options) = answer_choice(&mut toilet, &["偷窥", "是"]) {
            if toilet_choices.last() != Some(&options) {
                toilet_choices.push(options);
            }
        }
        if frame % 500 == 0 {
            stage_samples.push((frame as i32, toilet.vm.vars.get(218)));
        }
    }
    assert!(seen_toilet.contains(&323), "CE323 must run");
    assert!(seen_toilet.contains(&372), "the toilet menu must call CE372");
    assert!(seen_toilet.contains(&371), "CE372 must call CE371 (トイレタイマー)");
    assert!(ev372_max_pc >= 30, "CE372 must execute its stage body, max pc {ev372_max_pc}");
    assert_eq!(
        toilet.vm.vars.get(218),
        2,
        "CE372 [arg0=0, arg1=1] selects var218 = 2 and CE371 loops on that stage"
    );
    assert!(
        toilet.screen.iter().any(|picture| picture.file.contains("cutin/cutin_")),
        "CE372 must draw its cutin layer"
    );
    assert_eq!(toilet.stats.warnings, 0, "the CE323/CE372 wrapper must not warn");
    println!(
        "CE323/CE372: choices={toilet_choices:?} seen372={} ev372_maxpc={ev372_max_pc} v218={} \
         stage_samples={stage_samples:?} warnings={}",
        seen_toilet.contains(&372),
        toilet.vm.vars.get(218),
        toilet.stats.warnings
    );

    // --- C. CE328 (就寝前パジャマ) -> CE402: the wrapper exits before the CE402 call on
    //        this save. Pin the exact gate. ---
    let Some(mut bedtime) = save_game() else { return; };
    bedtime.auto_choice = true;
    bedtime.auto_choice_delay = 2;
    bedtime.set_choice_cursor(0);
    let v104 = bedtime.vm.vars.get(104);
    let v105 = bedtime.vm.vars.get(105);
    let v4 = bedtime.vm.vars.get(4);
    bedtime.start_event(328);
    let mut seen_bedtime: BTreeSet<u32> = BTreeSet::new();
    for _ in 0..2500 {
        bedtime.frame();
        seen_bedtime.extend(alive_events(&bedtime));
    }
    assert!(seen_bedtime.contains(&328), "CE328 must run");
    assert!(seen_bedtime.contains(&435), "CE328 must call CE435 before its early return");
    assert!(
        !seen_bedtime.contains(&402),
        "CE328 must NOT reach CE402 while var104/var105 are 0"
    );
    assert_eq!((v104, v105), (0, 0), "SaveData08 pajama flags gate CE328 at pc4/pc6");
    assert_eq!(
        v4, 0,
        "SaveData08 clock is midnight; CE402's own pc4 gate needs var4 >= 21 anyway"
    );
    assert_eq!(bedtime.stats.warnings, 0, "the CE328 wrapper must not warn");
    println!(
        "CE328/CE402: v104={v104} v105={v105} seen402={} seen435={} warnings={}",
        seen_bedtime.contains(&402),
        seen_bedtime.contains(&435),
        bedtime.stats.warnings
    );

    // --- D. Static call-graph evidence for the sibling wrappers. ---
    let data = common_events();
    assert_eq!(call_pcs(&data, 328, 500_402), vec![986], "CE328 -> CE402 call site");
    assert!(
        write_pcs(&data, 402, 2_000_137).contains(&136),
        "CE402 sets the pose selector (v137=6) at pc136"
    );
    // CE328's early-return gate: pc4 v105==0, pc6 v104==0 -> CE435, pc9 BreakEvent.
    assert_eq!(ce_ints(&data, 328, 4), vec![1, 2_000_105, 0, 2], "CE328 pc4: v105 == 0 early-exit arm");
    assert_eq!(ce_ints(&data, 328, 6), vec![1, 2_000_104, 0, 2], "CE328 pc6: v104 == 0 -> CE435");
    // CE402's own gates: v16 >= 1, v22 >= 180, v4 >= 21, v326 == 2, v102 >= 80.
    assert_eq!(ce_ints(&data, 402, 0), vec![17, 2_000_016, 1, 1], "CE402 pc0: v16 >= 1");
    assert_eq!(ce_ints(&data, 402, 2), vec![17, 2_000_022, 180, 1], "CE402 pc2: v22 >= 180");
    assert_eq!(ce_ints(&data, 402, 4), vec![17, 2_000_004, 21, 1], "CE402 pc4: v4 >= 21");
    assert_eq!(ce_ints(&data, 402, 6), vec![17, 2_000_326, 2, 1], "CE402 pc6: v326 == 2");
    assert_eq!(ce_ints(&data, 402, 8), vec![17, 2_000_102, 80, 1], "CE402 pc8: v102 >= 80");
    // CE323's real parent edges and CE372's stage table.
    let ce372_calls = call_pcs(&data, 323, 500_372);
    for pc in [120, 232, 240] {
        assert!(ce372_calls.contains(&pc), "CE323 -> CE372 call site pc{pc}: {ce372_calls:?}");
    }
    assert!(ce372_calls.len() >= 40, "CE323 polls many CE372 branches: {} sites", ce372_calls.len());
    let ce374_calls = call_pcs(&data, 323, 500_374);
    assert!(ce374_calls.contains(&194), "CE323 -> CE374 call sites include pc194: {ce374_calls:?}");
    assert_eq!(
        write_pcs(&data, 372, 2_000_218),
        vec![14, 22, 28, 42, 48, 59, 82, 90, 99, 110, 119, 128, 139, 149],
        "CE372 sets the var218 stage index at these pcs"
    );
    println!(
        "SIBLING DATA: CE328->CE402 pc986; CE323->CE372 pcs {:?}; CE323->CE374 {} sites; \
         CE372 var218 writes {:?}; CE246->CE217 {} call sites",
        ce372_calls,
        ce374_calls.len(),
        write_pcs(&data, 372, 2_000_218),
        call_pcs(&data, 246, 500_217).len()
    );
}
