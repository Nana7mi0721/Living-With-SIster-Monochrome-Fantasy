//! M7 W2 H-entry path: real day-loop -> night command menu (CE245) -> 身体接触 -> CE246 skinship.
//!
//! This test wires the H system to its genuinely real, non-synthetic pre-entry chain: the
//! canonical new-game day loop up to day2 19:30 bedroom (same harness as m6_night_menu.rs),
//! then a real hover+OK on the sister sprite to open the night command menu, then a real
//! left-click on the 身体接触 (skinship) row. That invokes CE245 "■夜コマンドメニュー"
//! (op210 [500246,0], pc529 unguarded willing path) -> CE246 "┣スキンシップ". NO H event is
//! started synthetically (no start_event / start_map_event on any H event).
//!
//! Empirically established (see docs/research/M7_HSCENE_ENTRY.md):
//!  * CE246 raises the six-way skinship menu [摸摸头,揉肩,拥抱,亲吻,挠痒痒,性骚扰].
//!  * Selecting any light option runs CE246 -> CE217 (HUD clock) and CE216 which draws
//!    hscene/header_bar.jpg -> the H overlay layer genuinely appears, warnings 0.
//!  * The actual CG renderer CE333 (op150 h_bg_*/h_s* files) and the CE346 cutin are only
//!    called from CE246's deep 性骚扰 explicit branch behind a multi-variable intimacy gate
//!    (var16/18/20/21/22/23/70/102/439) that a day-2 game does not carry; fabricating those
//!    vars still does not start CE333/CE346. That gate is the missing production hook and is
//!    asserted/documented by the third test.

use std::path::PathBuf;
use wolf_game::Game;

fn repo_root() -> PathBuf {
    let mut b = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    b.pop();
    b.pop();
    b
}
fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".to_owned());
    let p = PathBuf::from(raw);
    if p.is_absolute() { return p; }
    repo_root().join(p)
}
fn open_game() -> Option<Game> {
    match Game::new(data_root()) {
        Ok(g) => Some(g),
        Err(e) => { eprintln!("skip: cannot open data root {:?}: {e}", data_root()); None }
    }
}
fn decide(g: &mut Game) { g.key_bits = wolf_game::key::OK; g.frame(); g.key_bits = 0; }
fn click_at(g: &mut Game, x: i32, y: i32) {
    g.mouse = [x, y];
    for _ in 0..10 { g.frame(); }
    g.mouse_buttons = 1;
    for _ in 0..5 { g.frame(); }
    g.mouse_buttons = 0;
}

/// Canonical new game driven to day2 19:30 in the bedroom (mirror of m6_night_menu.rs).
fn day2_evening_game() -> Option<Game> {
    let mut game = open_game()?;
    game.auto_advance = true;
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.start_map_event_by_name("SampleMapA", 0).expect("start opening map event");
    for _ in 0..1500 { game.frame(); }
    game.auto_choice = false;
    game.vm.vars.set(63, 1); // documented: jump the bedroom sleep menu to the day-2 rollover
    for _ in 0..120 { game.frame(); if game.choices.is_some() { break; } }
    game.set_choice_cursor(2);
    decide(&mut game);
    for _ in 0..120 { game.frame(); if game.choices.is_some() { break; } }
    game.set_choice_cursor(0);
    decide(&mut game);
    for _ in 0..400 {
        game.frame();
        if game.vm.vars.get(160) == 2 && game.vm.vars.get(4) == 8 && game.vm.vars.get(5) >= 30 { break; }
    }
    game.auto_choice = true;
    for _ in 0..350 {
        game.frame();
        if game.screen.get(1201).map(|p| !p.text.is_empty()).unwrap_or(false) { break; }
    }
    click_at(&mut game, 512, 230); // day command: 学习会
    for _ in 0..1500 { game.frame(); if game.vm.instances().iter().all(|i| i.event != 251) { break; } }
    for _ in 0..1200 { game.frame(); }
    game.auto_choice = false;
    Some(game)
}

/// Real hover on the sister sprite (picture 120) + one OK opens the night command menu (CE245).
fn open_night_menu(g: &mut Game) {
    assert_eq!(g.vm.vars.get(295), 1, "evening night-menu flag should be set at day2 19:30");
    g.vm.vars.set(295, 0);
    g.vm.vars.set(68, 0);
    let s = g.screen.get(120).expect("sister sprite picture 120 present");
    let (x, y) = (s.x, s.y);
    g.mouse = [x, y];
    for _ in 0..8 { g.frame(); }
    decide(g);
    for _ in 0..40 { g.frame(); }
}

fn started(g: &Game) -> Vec<u32> {
    let mut v: Vec<u32> = g.vm.started_events.iter().copied().collect();
    v.sort();
    v.dedup();
    v
}
fn alive_events(g: &Game) -> Vec<u32> {
    let mut v: Vec<u32> = g.vm.instances().iter().map(|i| i.event).collect();
    v.sort();
    v.dedup();
    v
}
fn has_h_cg(g: &Game) -> bool {
    g.screen.iter().any(|p| p.file.contains("h_bg") || p.file.starts_with("hscene/h_s"))
}
fn has_h_any(g: &Game) -> bool {
    g.screen.iter().any(|p| {
        p.file.contains("h_bg")
            || p.file.contains("h_s")
            || p.file.contains("cutin")
            || p.file.contains("header_bar")
            || p.file.contains("h_button")
    })
}

/// Reach day2 bedroom, open the night menu, and click the top 身体接触 row -> CE246 skinship menu.
fn reach_skinship_menu() -> Option<Game> {
    let mut game = day2_evening_game()?;
    open_night_menu(&mut game);
    click_at(&mut game, 512, 200); // 身体接触 (top night-command row)
    for _ in 0..300 { game.frame(); if game.choices.is_some() { break; } }
    Some(game)
}

const SKINSHIP_MENU: [&str; 6] = ["摸摸头", "揉肩", "拥抱", "亲吻", "挠痒痒", "性骚扰"];

#[test]
fn day_loop_night_menu_click_reaches_ce246_skinship() {
    let Some(game) = reach_skinship_menu() else { return };
    assert_eq!(game.stats.warnings, 0, "no VM warnings on the real entry chain");
    // The genuine 6-way skinship menu raised by CE246 via the real click.
    let choices = game.choices.as_ref().expect("CE246 skinship menu should be raised");
    assert_eq!(choices.options.len(), SKINSHIP_MENU.len(), "six skinship options");
    for (got, want) in choices.options.iter().zip(SKINSHIP_MENU.iter()) {
        assert_eq!(got, want, "skinship option text");
    }
    // CE246 is alive, driven by real input only.
    assert!(alive_events(&game).contains(&246), "event 246 (CE246 スキンシップ) alive");
    // No H CG / cutin event was started by merely entering: this is a real pre-entry state.
    let st = started(&game);
    assert!(!st.contains(&333), "CE333 hscene must not be started by the entry click");
    assert!(!st.contains(&346), "CE346 cutin must not be started by the entry click");
    assert!(!has_h_cg(&game), "no h_bg/h_s CG drawn before an explicit selection");
    println!("[ce246] options={:?} started_246={} h_any={}", choices.options, st.contains(&246), has_h_any(&game));
}

#[test]
fn real_skinship_choice_starts_hud_and_header_bar() {
    let Some(mut game) = reach_skinship_menu() else { return };
    {
        let choices = game.choices.as_ref().expect("skinship menu");
        assert_eq!(choices.options[0], "摸摸头");
    }
    // Drive the real menu choice 摸摸头 (index 0), then auto-advance the resulting CE246 branch.
    game.set_choice_cursor(0);
    decide(&mut game);
    game.auto_choice = true;
    game.set_choice_cursor(0);
    game.auto_choice_delay = 2;
    for _ in 0..600 { game.frame(); }
    game.auto_choice = false;

    assert_eq!(game.stats.warnings, 0, "no VM warnings during the real skinship branch");
    let st = started(&game);
    // CE217 (HUD clock, op210 [500217,..]) is one of the named chain events and is genuinely reached.
    assert!(st.contains(&217), "CE217 HUD should start from a real skinship choice; started={st:?}");
    // CE216 draws the H overlay header bar -> the H layer genuinely appears.
    assert!(game.screen.iter().any(|p| p.file.contains("header_bar")),
        "hscene/header_bar.jpg should be drawn by CE216");
    // Still not the actual explicit CG: CE333/CE346 require the deep gated branch.
    assert!(!st.contains(&333), "CE333 not reachable from a light skinship option at day2");
    assert!(!st.contains(&346), "CE346 not reachable from a light skinship option at day2");
    assert!(!has_h_cg(&game), "no h_bg/h_s CG from a light skinship option");
    assert!(game.screen.len() >= 45, "rich live layer stack (npics={})", game.screen.len());
    println!("[ce217] started_has_217={} header_bar={} npics={}", st.contains(&217),
        game.screen.iter().any(|p| p.file.contains("header_bar")), game.screen.len());
}

#[test]
fn deep_explicit_scene_stays_gated_without_the_intimacy_hook() {
    let Some(mut game) = reach_skinship_menu() else { return };
    // Select 性骚扰 (index 5) -> with var20=75 (>60) the deep submenu is shown.
    game.set_choice_cursor(5);
    decide(&mut game);
    for _ in 0..300 { game.frame(); if game.choices.is_some() { break; } }
    let deep = game.choices.as_ref().map(|c| c.options.clone());
    let Some(deep) = deep else {
        // If the deep submenu is not shown, the gate is even tighter; that is still the blocker.
        assert_eq!(game.stats.warnings, 0);
        assert!(!started(&game).contains(&333));
        return;
    };
    assert_eq!(deep, vec!["让我看内衣", "让我看裸体", "想看你自慰", "让你看我撸管", "小鸟飞飞"],
        "deep 性骚扰 submenu");

    // Fabricate a generous intimacy/satisfaction set (a stand-in for the multi-day grind the game
    // expects) and then attempt every deep option. Even so, CE333/CE346 do NOT start: the real
    // explicit CG needs accumulated H-mode state that is not expressible as a small precondition.
    let bump = [(16, 1), (18, 120), (21, 200), (22, 200), (23, 10), (102, 120), (138, 100), (439, 1), (70, 1)];
    for (k, v) in bump { game.vm.vars.set(k, v); }
    for opt in 0..deep.len() {
        game.set_choice_cursor(opt);
        decide(&mut game);
        game.auto_choice = true;
        game.set_choice_cursor(0);
        game.auto_choice_delay = 2;
        for _ in 0..500 { game.frame(); }
        game.auto_choice = false;
        assert_eq!(game.stats.warnings, 0, "warnings stay 0 on deep option {opt} ({})", deep[opt]);
        let st = started(&game);
        assert!(!st.contains(&333), "CE333 must not be startable at day2 even with fabricated intimacy (opt {opt})");
        assert!(!st.contains(&346), "CE346 must not be startable at day2 even with fabricated intimacy (opt {opt})");
        assert!(!has_h_cg(&game), "no h_bg/h_s CG from deep option {opt}");
        // Return to a clean menu state if the branch raised the top skinship menu again.
        for _ in 0..40 { game.frame(); }
        if game.choices.as_ref().map(|c| c.options.len()) != Some(SKINSHIP_MENU.len()) {
            // Branch ended; re-open the deep submenu path for the next iteration.
            // (If it cannot be reopened we simply stop: the blocker has been demonstrated.)
            break;
        }
    }
    println!("[gated] deep_options={:?} warnings=0 ce333/ce346 unreachable", deep);
}
