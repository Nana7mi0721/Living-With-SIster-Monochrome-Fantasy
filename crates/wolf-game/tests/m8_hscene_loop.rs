//! M9 W2 -- the full H loop: night wrapper -> terminal stage 13 -> end button -> sleep ->
//! next-morning day scene.
//!
//! task-46 (m7_hscene_exit.rs, docs/research/M7_HSCENE_EXIT.md) proved that the late-night
//! H UI is *not* terminal-by-design: its on-screen end button (picture 294) dispatches through
//! map event 11 -> event 25 -> CE345 and tears the H overlay down. This file closes the loop:
//! it starts from the real retail save that resumes SampleMapA's late-night wrapper, plays the
//! wrapper to the terminal stage 13 with real pointer input, presses the end button, clears the
//! system-variable-12 message-window gate, answers CE345's confirmation, and then proves the
//! teardown leaves the engine in the normal next-morning day scene -- a state the day loop can
//! continue from -- with warnings 0. In other words: the H scene is a complete, playable loop,
//! not an isolated stage.
//!
//! What the exit does (data root work/all, save work/retail_backup/Save/SaveData08.sav):
//!
//! * Every late-night UI page draws the end button Picture/endbutton.png (picture 294).
//! * In the dispatcher (event 11) the active picture-press sets var61 = 49; map event 25
//!   深夜終了処理 page 0 turns that into a call to CE345 深夜終了コマンド once the message
//!   window is idle (system var 12 == 0).
//! * CE345 raises a first "是的 / 再想想" confirmation, then its own "是否要结束？/ 是,算了"
//!   dialogue choice; on the yes path its teardown runs
//!   pc339 var62=0, pc340 var63=0, pc341 var64=0, pc342 var106=0, pc343 var241=0,
//!   pc344 blackscreen, pc346 fade, pc347 CE336, pc348 CE222 (save), pc353 CE217 (clock), and
//!   pc361 / pc364 call CE329 就寝処理.
//! * CE329 is the same sleep routine the player's "今天还是睡吧" path uses: it advances the day
//!   counter var160 += 1 (pc406), rewinds the clock to morning (pc385 var4, pc386 var5) and plays
//!   the morning scene.
//!
//! So ending the midnight H scene is literally "go to sleep": day -> day+1, clock -> morning, and
//! control returns to the normal morning bedroom / day-command scene.
//!
//! Reachable-vs-gated wrapper: the task names ev33 -> CE333 -> CE246/CE372/CE402 as the
//! SampleMapA H wrapper cluster. Those CGI/common-event callees are gated on this save (map event
//! 33 only activates on var460 == 1, and CE246's deep CG needs the intimacy self-var chain
//! self1600013 / var157; CE402 needs var104/var105 != 0 and var4 >= 21). The wrapper a player
//! actually sees and drives here is the var62 stage UI (events 9/10/11/14/25 + CE345); the gated
//! CGI branches are documented, not synthesized. See docs/research/M9_HSCENE_LOOP.md.
//!
//! Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_loop -- --nocapture

use std::collections::BTreeSet;
use std::path::PathBuf;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_format::mps::Map;
use wolf_game::Game;
use wolf_vm::exec::ChoiceReply;

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

fn ce_ints(data: &CommonEventData, event: u32, pc: usize) -> Vec<i32> {
    let ev = data
        .event(event)
        .unwrap_or_else(|| panic!("CE{event} exists"));
    let Slot::Command(cmd) = ev
        .commands
        .get(pc)
        .unwrap_or_else(|| panic!("CE{event} pc{pc}"))
    else {
        panic!("CE{event} pc{pc} is blank");
    };
    cmd.params
        .chunks_exact(4)
        .map(|word| i32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .collect()
}

fn call_pcs(data: &CommonEventData, event: u32, target: i32) -> Vec<usize> {
    let ev = data
        .event(event)
        .unwrap_or_else(|| panic!("CE{event} exists"));
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

fn write_pcs(data: &CommonEventData, event: u32, target: i32) -> Vec<usize> {
    let ev = data
        .event(event)
        .unwrap_or_else(|| panic!("CE{event} exists"));
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

fn open_resumed() -> Option<Game> {
    let save = save_path();
    if !save.exists() {
        eprintln!("skip: missing {}", save.display());
        return None;
    }
    if !data_root().join("MapData").join("SampleMapA.mps").exists() {
        eprintln!(
            "skip: missing SampleMapA.mps under {}",
            data_root().display()
        );
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

fn endbutton_pos(game: &Game) -> Option<(i32, i32)> {
    game.screen
        .get(294)
        .filter(|picture| picture.file.contains("endbutton"))
        .map(|picture| (picture.x, picture.y))
}

fn file_present(game: &Game, needle: &str) -> bool {
    game.screen
        .iter()
        .any(|picture| picture.file.contains(needle))
}

fn nav_to_terminal_stage(game: &mut Game) -> bool {
    for _ in 0..40 {
        if game.vm.vars.get(62) == 13 {
            return true;
        }
        let buttons: Vec<(i32, i32)> = game
            .screen
            .iter()
            .filter(|picture| picture.file.contains("h_button") || picture.file.contains("hbutton"))
            .map(|picture| (picture.x, picture.y))
            .collect();
        let mut advanced = false;
        for (x, y) in buttons {
            let before = game.vm.vars.get(62);
            click_at(game, x, y);
            if game.vm.vars.get(62) != before {
                advanced = true;
                break;
            }
        }
        if !advanced {
            return game.vm.vars.get(62) == 13;
        }
    }
    game.vm.vars.get(62) == 13
}

fn spin_to_message_idle(game: &mut Game) {
    for _ in 0..160 {
        game.frame();
        if game.vm.vars.get_sys(12) == 0 {
            return;
        }
    }
}

fn press_end_through_teardown(game: &mut Game, frames: usize) -> (BTreeSet<u32>, bool) {
    let Some((x, y)) = endbutton_pos(game) else {
        return (BTreeSet::new(), false);
    };
    let mut seen: BTreeSet<u32> = game.vm.instances().iter().map(|inst| inst.event).collect();
    game.mouse = [x, y];
    game.mouse_buttons = 0;
    game.frame();
    game.mouse_buttons = 1;
    game.frame();
    game.mouse_buttons = 0;
    for _ in 0..frames {
        game.frame();
        for inst in game.vm.instances() {
            seen.insert(inst.event);
        }
        if game.vm.vars.get(62) == 0 && game.vm.vars.get(64) == 0 {
            for _ in 0..30 {
                game.frame();
                for inst in game.vm.instances() {
                    seen.insert(inst.event);
                }
            }
            return (seen, true);
        }
        if let Some(choice) = &game.choices {
            let options = choice.options.clone();
            let index = options
                .iter()
                .position(|o| o == "是" || o == "是的")
                .unwrap_or(0);
            game.reply(ChoiceReply::Index(index));
        }
    }
    (seen, true)
}

fn drive_to_next_morning(game: &mut Game, expected_day: i32, seen: &mut BTreeSet<u32>) -> bool {
    for _ in 0..1400 {
        game.frame();
        for inst in game.vm.instances() {
            seen.insert(inst.event);
        }
        if let Some(choice) = &game.choices {
            let options = choice.options.clone();
            let index = options
                .iter()
                .position(|o| o == "是" || o == "是的")
                .unwrap_or(0);
            game.reply(ChoiceReply::Index(index));
        }
        if game.vm.vars.get(160) == expected_day
            && game.vm.vars.get(4) >= 7
            && file_present(game, "bg_01_home")
            && file_present(game, "stand_imouto")
            && file_present(game, "sub_button")
            && game.screen.len() > 40
        {
            return true;
        }
    }
    false
}

fn h_overlay_files(game: &Game) -> Vec<(i32, String, bool, i32)> {
    game.screen
        .iter()
        // Picture 6000 is CE216's `hscene/header_bar.jpg` status-bar backdrop, not
        // H-scene art: CE216 is resumed on load so the normal HUD stays available
        // after the night scene tears down.
        .filter(|picture| {
            picture.number != 6000
                && (picture.file.contains("hscene/") || picture.file.contains("cutin/"))
        })
        .map(|picture| {
            (
                picture.number,
                picture.file.clone(),
                picture.hidden,
                picture.opacity,
            )
        })
        .collect()
}

#[test]
fn full_loop_enters_stage13_exits_and_rolls_to_next_morning() {
    let Some(mut game) = open_resumed() else {
        return;
    };
    for _ in 0..60 {
        game.frame();
    }
    let day_before = game.vm.vars.get(160);

    game.vm.vars.set(364, 0);
    for _ in 0..40 {
        game.frame();
    }

    game.auto_advance = true;
    game.auto_choice = false;
    assert!(
        nav_to_terminal_stage(&mut game),
        "real pointer navigation must reach terminal stage 13"
    );
    assert_eq!(
        game.vm.vars.get(62),
        13,
        "reached the terminal stage by real clicks"
    );
    assert_eq!(
        game.vm.vars.get(64),
        1,
        "pajama flag var64 is set on the H path"
    );

    spin_to_message_idle(&mut game);
    assert_eq!(
        game.vm.vars.get_sys(12),
        0,
        "message window (system var 12) must be idle before the end button can dispatch"
    );
    assert!(
        endbutton_pos(&game).is_some(),
        "stage 13 draws Picture/endbutton.png (picture 294)"
    );
    assert!(
        file_present(&game, "hscene/h_s10_pose"),
        "H pose layer is on screen at stage 13"
    );

    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.set_choice_cursor(0);
    let (mut seen, clicked) = press_end_through_teardown(&mut game, 400);
    assert!(clicked, "end button must be clickable");
    assert!(
        seen.contains(&345),
        "event 25 must dispatch CE345 深夜終了コマンド (seen {seen:?})"
    );
    assert!(
        seen.contains(&222),
        "CE345 teardown saves through CE222 (seen {seen:?})"
    );
    assert_eq!(
        game.vm.vars.get(62),
        0,
        "exit resets the stage index var62 -> 0"
    );
    assert_eq!(
        game.vm.vars.get(64),
        0,
        "exit clears the pajama flag var64 (CE345 pc341)"
    );
    assert_eq!(game.vm.vars.get(63), 0, "exit clears var63 (CE345 pc340)");
    assert_eq!(game.vm.vars.get(106), 0, "exit clears var106 (CE345 pc342)");
    assert_eq!(game.vm.vars.get(241), 0, "exit clears var241 (CE345 pc343)");

    let reached = drive_to_next_morning(&mut game, day_before + 1, &mut seen);
    assert!(
        reached,
        "the sleep routine must roll over to day {} morning (no deadlock)",
        day_before + 1
    );
    assert!(
        seen.contains(&329),
        "CE345 pc361/pc364 must call CE329 就寝処理 for the day rollover (seen {seen:?})"
    );
    assert_eq!(
        game.vm.vars.get(160),
        day_before + 1,
        "ending the H scene advances the day counter (CE329 pc406 var160 += 1)"
    );
    assert!(
        game.vm.vars.get(4) >= 7,
        "the clock is back in the morning (var4 = {})",
        game.vm.vars.get(4)
    );
    assert_eq!(
        game.stats.warnings, 0,
        "the whole loop -- exit, sleep and morning -- must be warning-free"
    );

    assert!(
        game.screen
            .iter()
            .any(|picture| picture.number == 6000 && picture.file.contains("hscene/header_bar")),
        "CE216's status-bar header must be restored after the sleep teardown"
    );
    assert!(
        h_overlay_files(&game).is_empty(),
        "the H overlay (hscene/ + cutin/ layers) must be gone after the sleep teardown: {:?}",
        h_overlay_files(&game)
    );
    assert!(
        file_present(&game, "bg_01_home"),
        "the normal home background is back"
    );
    assert!(
        file_present(&game, "stand_imouto"),
        "the sister stand-up portrait is drawn in the morning scene"
    );
    assert!(
        file_present(&game, "sub_button"),
        "the morning day-command scene exposes its command buttons"
    );
    assert!(
        game.screen.len() > 40,
        "a full morning scene is drawn ({} pictures)",
        game.screen.len()
    );

    let alive: BTreeSet<u32> = game.vm.instances().iter().map(|inst| inst.event).collect();
    assert!(
        alive.contains(&4),
        "the map メニュー処理 rollover event stays live after the loop: {alive:?}"
    );
    assert!(
        !alive.is_empty(),
        "the map runtime is still running after the loop"
    );

    let visible_hword: Vec<_> = game
        .screen
        .iter()
        .filter(|p| p.file.contains("hword/") && !p.hidden && p.opacity > 0)
        .map(|p| p.file.clone())
        .collect();
    assert!(
        visible_hword.is_empty(),
        "no visible h-word overlay may survive the exit: {visible_hword:?}"
    );

    println!(
        "H LOOP: day {day_before} -> {} clock=({},{}) seen345={} seen329={} seen222={} alive={:?} pics={} warnings={}",
        game.vm.vars.get(160),
        game.vm.vars.get(4),
        game.vm.vars.get(5),
        seen.contains(&345),
        seen.contains(&329),
        seen.contains(&222),
        alive,
        game.screen.len(),
        game.stats.warnings
    );
}

#[test]
fn no_end_press_holds_stage_13_without_rollover() {
    let Some(mut game) = open_resumed() else {
        return;
    };
    for _ in 0..60 {
        game.frame();
    }
    let day_before = game.vm.vars.get(160);
    game.vm.vars.set(364, 0);
    for _ in 0..40 {
        game.frame();
    }
    game.auto_advance = true;
    game.auto_choice = false;
    assert!(nav_to_terminal_stage(&mut game), "reach stage 13");
    spin_to_message_idle(&mut game);

    for _ in 0..700 {
        game.frame();
    }
    assert_eq!(
        game.vm.vars.get(62),
        13,
        "without the end button the wrapper holds stage 13"
    );
    assert_eq!(
        game.vm.vars.get(64),
        1,
        "the pajama flag stays set while the scene holds"
    );
    assert_eq!(
        game.vm.vars.get(160),
        day_before,
        "idle time alone does not advance the day counter"
    );
    assert_eq!(game.stats.warnings, 0, "holding the stage is warning-free");
}

#[test]
fn named_cg_chain_is_gated_and_rollover_is_wired_through_ce329() {
    let Some(game) = open_resumed() else {
        return;
    };
    assert_eq!(
        game.vm.vars.get(460),
        0,
        "SaveData08 carries var460 = 0 (deep-CG chain inactive)"
    );

    let map = load_sample_map();
    let ev33 = map
        .events
        .iter()
        .find(|event| event.id == 33)
        .expect("map ev33");
    let conditions = ev33.pages[0].conditions();
    let enabled: Vec<_> = conditions.iter().filter(|c| c.enabled).collect();
    assert_eq!(
        enabled.len(),
        1,
        "ev33 page 0 has exactly one activation condition"
    );
    assert_eq!(
        enabled[0].variable, 2_000_460,
        "ev33 page 0 watches global var460"
    );
    assert_eq!(enabled[0].value, 1, "ev33 activates only on var460 == 1");

    let data = common_events();
    let ce345_calls = call_pcs(&data, 345, 500_329);
    assert!(
        ce345_calls.contains(&361) && ce345_calls.contains(&364),
        "CE345 tail must call CE329 就寝処理: {ce345_calls:?}"
    );
    assert!(
        call_pcs(&data, 345, 500_222).contains(&348),
        "CE345 saves through CE222 at pc348"
    );
    assert!(
        call_pcs(&data, 345, 500_217).contains(&353),
        "CE345 refreshes the clock through CE217 at pc353"
    );
    assert_eq!(
        write_pcs(&data, 329, 2_000_160),
        vec![406],
        "CE329 pc406 increments the day counter var160"
    );
    assert_eq!(
        ce_ints(&data, 329, 385),
        vec![2_000_004, 7, 0, 0],
        "CE329 pc385 sets the clock hour var4"
    );
    assert_eq!(
        ce_ints(&data, 329, 386),
        vec![2_000_005, 45, 0, 0],
        "CE329 pc386 sets the clock minute var5"
    );
    assert_eq!(
        ce_ints(&data, 345, 339),
        vec![2_000_062, 0, 0, 0],
        "CE345 pc339 var62 = 0"
    );
    assert_eq!(
        ce_ints(&data, 345, 341),
        vec![2_000_064, 0, 0, 0],
        "CE345 pc341 var64 = 0"
    );
}
