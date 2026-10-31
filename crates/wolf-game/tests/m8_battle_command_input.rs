//! M9 task-50: real battle command input probe.
//!
//! Two facts are pinned here, both test-only and data-faithful:
//!
//! 1. The player-facing battle command line CE188 -> CE191 -> CE192 -> CE152 -> CE65 ->
//!    CE157 really accepts pointer/key input. Driving the CE65 万能ウィンドウ with DOWN/OK
//!    selects a NON-default row (防御, Data type6 row4, command code 25). CE157 registers
//!    that choice in the physical CData table whose editor label is
//!    [×┣ コマンド処理リスト]: row = コマンド入力位置 (= 1 here), column 3 = 発動技能 = 25.
//!    The runtime type id in the script is 15; Databases::resolve_labels maps it to the
//!    physical .dat type 14 by the type label, so the test reads CData type 14.
//!
//! 2. The separate CE253 -> CE239 battle wrapper settles through the rebuilt engine
//!    wiring (CE242 command phase -> CE244 settlement), but CE239 never opens the CE188
//!    menu line: CE191/CE192/CE65 never start. The selected command id is therefore
//!    registered by the menu line only; it is NOT what CE242 consumes. Variable 2000496
//!    stays 0 in both scenarios - it is a Yukari-only subchoice code, not the command id.
//!
//! Run: cargo test -q -p wolf-game --test m8_battle_command_input -- --nocapture

use std::path::PathBuf;

use wolf_game::db::Container;
use wolf_game::Game;

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        return p;
    }
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop();
    b.pop();
    b.join(p)
}

fn save01() -> PathBuf {
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop();
    b.pop();
    b.join("work/retail_backup/Save/SaveData01.sav")
}

fn event_id<E: std::fmt::Debug>(e: &E) -> String {
    format!("{:?}", e)
}

/// The command row code CE65 should select: 防御. Data type6 row4 carries code 25.
const DEFENSE_CODE: i32 = 25;

/// Command codes that a seeded inventory makes usable; CE19 col2 must be 1 for these.
const REAL_ATTACK_CODES: [i32; 4] = [26, 25, 30, 29];

#[test]
fn real_ce188_command_window_registers_a_chosen_non_default_command() {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;

    let seeded = game.seed_battle_databases();
    assert!(
        seeded > 0,
        "seed_battle_databases must load the battle command cells"
    );

    let _ = game.start_event(188);

    let mut saw_command_menu = false;
    let mut selectable_codes: Vec<i32> = Vec::new();
    let mut selected_code = 0i32;
    let mut registered_row: Option<i32> = None;
    let mut saw_ce157 = false;
    let mut saw_ce192 = false;
    let mut saw_ce65 = false;
    let mut saw_ce242 = false;
    let mut saw_ce244 = false;
    let mut v496_peak = 0i32;

    for f in 0..500 {
        let mut cursor: Option<i32> = None;
        for inst in game.vm.instances() {
            match event_id(&inst.event).as_str() {
                "65" => {
                    saw_ce65 = true;
                    cursor = Some(inst.self_vars.get(611));
                }
                "192" => saw_ce192 = true,
                "157" => saw_ce157 = true,
                "242" => saw_ce242 = true,
                "244" => saw_ce244 = true,
                _ => {}
            }
        }

        // CData type19 is the 万能ウィンドウ temporary table: column 1 = item code,
        // column 2 = selectable.  Select the row whose code is 防御 (25).
        let mut target_row: Option<i32> = None;
        for row in 0..8 {
            let code = game
                .databases
                .num(Container::CData, 19, row, 1)
                .unwrap_or(-1);
            let selectable = game
                .databases
                .num(Container::CData, 19, row, 2)
                .unwrap_or(0);
            if REAL_ATTACK_CODES.contains(&code) && selectable == 1 {
                saw_command_menu = true;
                if !selectable_codes.contains(&code) {
                    selectable_codes.push(code);
                }
                if code == DEFENSE_CODE {
                    target_row = Some(row);
                }
            }
        }

        // Top menu (战斗/逃跑) has no real-attack row: OK picks the highlighted 战斗.
        // Command menu: steer the cursor to the 防御 row, then confirm.
        game.key_bits = match cursor {
            Some(cur) => match target_row {
                Some(target) if cur < target => wolf_game::key::DOWN,
                Some(target) if cur > target => wolf_game::key::UP,
                Some(_) => wolf_game::key::OK,
                None => wolf_game::key::OK,
            },
            None => {
                if f % 2 == 0 {
                    wolf_game::key::OK
                } else {
                    0
                }
            }
        };

        let _ = game.frame();
        v496_peak = v496_peak.max(game.vm.vars.get(496).abs());

        // CE157 writes the chosen action to the physical CData table labelled
        // [×┣ コマンド処理リスト]. Runtime type id 15 resolves to physical type 14.
        for row in 0..16 {
            if game.databases.num(Container::CData, 14, row, 3) == Some(DEFENSE_CODE) {
                registered_row = Some(row);
                selected_code = DEFENSE_CODE;
            }
        }
        if registered_row.is_some() {
            break;
        }
    }

    eprintln!(
        "menu_seen={saw_command_menu} ce65={saw_ce65} ce192={saw_ce192} ce157={saw_ce157} codes={selectable_codes:?} selected={selected_code} registered_row={registered_row:?} ce242={saw_ce242} ce244={saw_ce244} v496_peak={v496_peak} v352={} v343={} v350={} warnings={}",
        game.vm.vars.get(352),
        game.vm.vars.get(343),
        game.vm.vars.get(350),
        game.stats.warnings,
    );

    assert!(saw_ce65, "CE65 万能ウィンドウ選択実行 must run");
    assert!(saw_ce192, "CE192 味方コマンド選択 must run");
    assert!(
        saw_command_menu,
        "CE152 must publish selectable real-attack command rows"
    );
    assert!(
        selectable_codes.contains(&DEFENSE_CODE),
        "防御 (code 25) must be selectable, got {selectable_codes:?}"
    );
    assert!(
        saw_ce157,
        "CE157 コマンド登録 must run after the menu choice"
    );
    assert_eq!(
        registered_row,
        Some(1),
        "the chosen command must land at CData type14 row1 col3 (コマンド入力位置 = 1)"
    );
    assert_eq!(
        selected_code, DEFENSE_CODE,
        "the registered 発動技能 must be 防御 code 25"
    );
    assert_eq!(
        v496_peak, 0,
        "2000496 is not the chosen command and must stay 0"
    );
    assert!(
        !saw_ce242,
        "CE242 is not reachable from the CE188 menu line"
    );
    assert!(
        !saw_ce244,
        "CE244 settlement is not reachable from the CE188 menu line"
    );
    assert_eq!(
        game.vm.vars.get(352),
        0,
        "v352 must stay 0 - CE244 never settles here"
    );
    assert_eq!(
        game.vm.vars.get(343),
        0,
        "v343 must stay 0 - CE244 never settles here"
    );
    assert_eq!(
        game.vm.vars.get(350),
        0,
        "v350 command phase must stay idle in the menu line"
    );
    assert_eq!(game.stats.warnings, 0, "no engine warnings allowed");
}

#[test]
fn ce239_battle_settles_while_the_player_menu_line_never_runs() {
    let mut game = Game::new(data_root()).expect("Game::new");
    game.load_save(&save01()).expect("load SaveData01");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;

    let seeded = game.seed_battle_databases();
    assert!(
        seeded > 0,
        "seed_battle_databases must load the battle command cells"
    );
    game.databases
        .set_text(Container::CData, 5, 16, 0, "┗ 状態リスト");

    let _ = game.start_engine_events();
    game.set_battle_wiring(true);
    let _ = game.start_event(253);

    let mut ce239 = false;
    let mut ce242 = false;
    let mut ce244 = false;
    let mut ce191 = false;
    let mut ce192 = false;
    let mut ce65 = false;
    let mut v352_peak = 0i32;
    let mut v343_peak = 0i32;
    let mut v496_peak = 0i32;
    let mut frames = 0usize;

    let record = |game: &Game,
                  ce239: &mut bool,
                  ce242: &mut bool,
                  ce244: &mut bool,
                  ce191: &mut bool,
                  ce192: &mut bool,
                  ce65: &mut bool,
                  v352_peak: &mut i32,
                  v343_peak: &mut i32,
                  v496_peak: &mut i32| {
        let v352 = game.vm.vars.get(352);
        if v352.abs() > v352_peak.abs() {
            *v352_peak = v352;
        }
        *v343_peak = (*v343_peak).max(game.vm.vars.get(343));
        *v496_peak = (*v496_peak).max(game.vm.vars.get(496).abs());
        for inst in game.vm.instances() {
            match event_id(&inst.event).as_str() {
                "239" => *ce239 = true,
                "242" => *ce242 = true,
                "244" => *ce244 = true,
                "191" => *ce191 = true,
                "192" => *ce192 = true,
                "65" => *ce65 = true,
                _ => {}
            }
        }
    };

    for f in 0..20000 {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        let _ = game.frame();
        frames = f + 1;
        record(
            &game,
            &mut ce239,
            &mut ce242,
            &mut ce244,
            &mut ce191,
            &mut ce192,
            &mut ce65,
            &mut v352_peak,
            &mut v343_peak,
            &mut v496_peak,
        );
        if game.vm.vars.get(343) == 1 && game.vm.vars.get(350) == 0 {
            for _ in 0..800 {
                let _ = game.frame();
                frames += 1;
                record(
                    &game,
                    &mut ce239,
                    &mut ce242,
                    &mut ce244,
                    &mut ce191,
                    &mut ce192,
                    &mut ce65,
                    &mut v352_peak,
                    &mut v343_peak,
                    &mut v496_peak,
                );
            }
            break;
        }
    }

    eprintln!(
        "ce239={ce239} ce242={ce242} ce244_seen={ce244} menu_line: ce191={ce191} ce192={ce192} ce65={ce65} v352_peak={v352_peak} v343_peak={v343_peak} v496_peak={v496_peak} frames={frames} warnings={}",
        game.stats.warnings,
    );

    assert!(ce239, "CE239 battle caller must start from CE253");
    assert!(
        ce242,
        "the rebuilt wiring must start CE242 on the battle-active edge"
    );
    assert!(
        v352_peak == 1 || v352_peak == -1,
        "CE244 must settle 2000352 in {{1,-1}}, got {v352_peak}"
    );
    assert_eq!(v343_peak, 1, "CE244 must set 2000343 = 1");
    assert!(
        !ce191 && !ce192 && !ce65,
        "the CE188/CE191/CE192/CE65 player menu line must never run inside CE239"
    );
    assert_eq!(
        v496_peak, 0,
        "2000496 stays 0 even though the battle settles"
    );
    assert_eq!(game.stats.warnings, 0, "no engine warnings allowed");
}
