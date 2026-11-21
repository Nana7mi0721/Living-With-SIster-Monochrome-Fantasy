//! M6 day-loop regression at the day-4 boundary.
//!
//! Task-10 asked whether the verified day loop (which m6_multiday takes to day 4 08:30) can be
//! extended to day 5/6 08:30 by choosing a NON-battle day-command row on day 4 instead of the
//! deterministic first row (internal id 7 -> CE253 地域貢献 -> CE239 battle).
//!
//! Empirically it CANNOT. On the morning of day 4 (weekday index var6 == 3), CE251 does not draw
//! its five-row picture menu (pictures 1201/1203/1205/1207/1209 stay blank and no five-option
//! Choice is offered). Instead it plays a scripted guild-subjugation story: the only choices on
//! the day-4 morning are the story greetings "早上好。" / "嗯，早上好。" (CE251 idx 47-48, 95, 125,
//! 158, 200, 258; single-option cutscene Choices). That story routes through the 地域貢献 branch
//! (CE251 JumpLabel 地域貢献 idx 419 -> SetLabel 地域貢献 idx 2465 -> CommonEvent [500253,0] idx
//! 2466) into CE253, whose combat branch calls CE239 at CE253 idx 174/177/180/185/188/191. CE239
//! is the real battle entry (1108 slots, docs/BATTLE.md:17). With the rebuilt battle wiring
//! OFF (the default) the day-4 story stalls there: the clock freezes and the day never advances.
//! There is no selectable non-battle row on day 4, so day 5 08:30 is unreachable through the
//! real day loop **without** `Game::set_battle_wiring(true)`.
//!
//! This regression test locks the wiring-off behaviour: it drives the real day 1 -> day 4 08:30
//! chain (day-command menu days 2-3, dinner/evening, real CE329 sleep), then asserts that the
//! day-4 daytime is forced into the CE253/CE239 battle with NO day-command menu ever drawn and
//! no clock advance. The wiring-on acceptance run (battle settles, day 5 reached with no
//! injection) is `crates/wolf-game/tests/m7_day4_playable.rs`; the companion audit lives in
//! docs/research/M6_SCHEDULE.md.
//!
//! Facts asserted: day counter var160, weekday var6, clock var4/var5, stats.warnings == 0, and a
//! drawn morning scene (screen.len() > 20). The sister-click flag var63 is seeded only where the
//! event-1 pointer branch writes it, exactly as m6_mainline / m6_night_menu / m6_multiday do.

use std::path::PathBuf;

use wolf_game::Game;

fn resolve_root(raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return path;
    }
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    base.join(path)
}

/// Data root. Honors WOLF_DATA_ROOT (the task runs with work/all); otherwise prefers a root
/// carrying the Save/System.sav template and falls back to work/all / work/full.
fn data_root() -> PathBuf {
    if let Ok(raw) = std::env::var("WOLF_DATA_ROOT") {
        return resolve_root(&raw);
    }
    for candidate in ["work/all", "work/full"] {
        let root = resolve_root(candidate);
        if root.join("Save").join("System.sav").exists()
            || root.join("save").join("system.sav").exists()
        {
            return root;
        }
    }
    resolve_root("work/all")
}

fn open_game() -> Option<Game> {
    match Game::new(data_root()) {
        Ok(game) => Some(game),
        Err(err) => {
            eprintln!("skipping: {err}");
            None
        }
    }
}

fn decide(game: &mut Game) {
    game.key_bits = wolf_game::key::OK;
    game.frame();
    game.key_bits = 0;
}

fn wait_for_choice(game: &mut Game, budget: usize, what: &str) -> Vec<String> {
    for _ in 0..budget {
        game.frame();
        if let Some(choices) = &game.choices {
            if choices.frames >= 2 {
                return choices.options.clone();
            }
        }
    }
    panic!("timed out after {budget} frames waiting for {what}");
}

const SLEEP_OPTIONS: [&str; 3] = ["恶作剧", "轻轻把被子盖好", "今天还是睡吧"];
const CONFIRM_OPTIONS: [&str; 2] = ["是的", "再想想"];

fn instances_events(game: &Game) -> Vec<u32> {
    game.vm.instances().iter().map(|i| i.event).collect()
}

fn has_battle(game: &Game) -> bool {
    game.vm
        .instances()
        .iter()
        .any(|instance| instance.event == 253 || instance.event == 239)
}

/// Real CE329 sleep: three-way night prompt -> confirm -> next-day 08:30 bedroom.
fn sleep_into_next_morning(game: &mut Game, day: i32) {
    let expected_day = day + 1;
    game.auto_choice = false;
    game.vm.vars.set(63, 1);
    let options = wait_for_choice(game, 400, "the three-way sleep prompt");
    assert_eq!(
        options,
        SLEEP_OPTIONS
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        "day {day}: the night prompt is 恶作剧 / 轻轻把被子盖好 / 今天还是睡吧"
    );
    game.set_choice_cursor(2);
    decide(game);
    let confirm = wait_for_choice(game, 200, "the sleep confirmation prompt");
    assert_eq!(
        confirm,
        CONFIRM_OPTIONS
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        "day {day}: the confirmation prompt is 是的 / 再想想"
    );
    game.set_choice_cursor(0);
    decide(game);
    let mut arrived = false;
    for _ in 0..800 {
        game.frame();
        if game.vm.vars.get(160) == expected_day
            && game.vm.vars.get(4) == 8
            && game.vm.vars.get(5) >= 30
        {
            arrived = true;
            break;
        }
    }
    assert!(
        arrived,
        "day {expected_day} 08:30 bedroom was reached (no deadlock in CE329)"
    );
    assert_eq!(
        game.stats.warnings, 0,
        "day {expected_day}: the sleep transition is warning-free"
    );
    assert_eq!(
        game.vm.vars.get(160),
        expected_day,
        "the day counter advanced to day {expected_day}"
    );
    assert_eq!(
        game.vm.vars.get(6),
        expected_day - 1,
        "weekday var6 is {expected_day}-1 for day {expected_day}"
    );
    assert_eq!(game.vm.vars.get(4), 8, "the morning starts at 8");
    assert_eq!(game.vm.vars.get(5), 30, "and 30 minutes");
    assert!(
        game.screen.len() > 20,
        "the day {expected_day} morning scene is drawn: {} pictures",
        game.screen.len()
    );
    assert!(
        !has_battle(game),
        "day {expected_day} 08:30: no resident 253/239 battle coroutine"
    );
}

/// The CE251 day-command rows that route to CE252 (the normal daytime tasks).
/// (button label, `var48` command id, the stat variable the task raises).
///
/// CE251 rolls three distinct increasing values from 1..=10 into self[10..12]; ids
/// 7/8/9 are 地区贡献/探索/远征 (CE253/CE254), a different flow. The roll decides
/// which task row 1 is, so the test picks the first CE252 row like a player would.
const DAY_COMMANDS: [(&str, i32, Option<usize>); 6] = [
    ("给店里帮忙", 1, None),
    ("学习会", 2, Some(171)),
    ("基础锻炼", 3, Some(174)),
    ("攻击练习", 4, Some(170)),
    ("防御练习", 5, Some(173)),
    ("秘密特训", 6, Some(174)),
];

fn day_command(label: &str) -> Option<(i32, Option<usize>)> {
    DAY_COMMANDS
        .iter()
        .find(|(name, _, _)| label.contains(name))
        .map(|(_, id, stat)| (*id, *stat))
}

const DAY_ROW_PICS: [i32; 5] = [1201, 1203, 1205, 1207, 1209];
const DAY_ROW_Y: i32 = 230;

fn day_row_texts(game: &Game) -> Vec<String> {
    DAY_ROW_PICS
        .iter()
        .map(|&number| {
            game.screen
                .get(number)
                .map(|p| p.text.clone())
                .unwrap_or_default()
        })
        .collect()
}

fn evening_work(game: &Game) -> bool {
    game.vm
        .instances()
        .iter()
        .any(|instance| matches!(instance.event, 251 | 300 | 301 | 326 | 406))
}

/// Drive the CE251 day-command menu on a normal 08:30 morning, pick the first row
/// that is a CE252 daytime task (the roll can put 地区贡献 first), let it spend the
/// day to 17:00, wait for the evening to settle, and return the observed internal id
/// (var48). Used only for days 2-3, which do draw the training menu; day 4 does not
/// (see the module docs).
fn spend_day_choosing_row(game: &mut Game, day: i32) -> i32 {
    game.auto_choice = true;
    game.auto_advance = true;

    let mut labels: Vec<String> = Vec::new();
    let mut chosen: Option<(usize, String, i32, Option<usize>)> = None;
    for _ in 0..4000 {
        game.frame();
        labels = day_row_texts(game);
        for (row, text) in labels.iter().enumerate() {
            if let Some((id, stat)) = day_command(text) {
                chosen = Some((row, text.clone(), id, stat));
                break;
            }
        }
        if chosen.is_some() {
            break;
        }
    }
    let (row, label, command_id, stat) = chosen.unwrap_or_else(|| {
        panic!(
            "day {day}: CE251 draws no CE252 day command (no deadlock; clock={}:{}, instances={:?}, labels={labels:?})",
            game.vm.vars.get(4),
            game.vm.vars.get(5),
            instances_events(game)
        )
    });

    let money_before = game.vm.vars.get(7);
    let stat_before = stat.map(|addr| game.vm.vars.get(addr));
    let y = DAY_ROW_Y + 50 * row as i32;
    let energy_before = game.vm.vars.get(14);
    game.mouse = [512, y];
    for _ in 0..10 {
        game.frame();
    }
    game.mouse_buttons = 1;
    for _ in 0..5 {
        game.frame();
    }
    game.mouse_buttons = 0;

    let mut finished = false;
    let mut battle = false;
    for _ in 0..2500 {
        game.frame();
        if has_battle(game) {
            battle = true;
            break;
        }
        if game
            .vm
            .instances()
            .iter()
            .all(|instance| instance.event != 251)
            && game.vm.vars.get(4) == 17
        {
            finished = true;
            break;
        }
    }
    let var48 = game.vm.vars.get(48);
    if battle {
        panic!(
            "day {day}: {label} (y={y}) routed into the CE253/CE239 BATTLE (var48={var48}, instances={:?}, clock={}:{})",
            instances_events(game),
            game.vm.vars.get(4),
            game.vm.vars.get(5)
        );
    }
    assert!(finished, "day {day}: {label} (y={y}) day-command did not finish at 17:00 (instances={:?}, clock={}:{}, var48={var48})", instances_events(game), game.vm.vars.get(4), game.vm.vars.get(5));
    assert_eq!(
        game.stats.warnings, 0,
        "day {day}: the day command is warning-free"
    );
    assert_eq!(
        game.vm.vars.get(4),
        17,
        "day {day}: {label} spends the day until 17:00"
    );
    assert_eq!(game.vm.vars.get(5), 0, "day {day}: on the hour");
    assert_eq!(
        game.vm.vars.get(160),
        day,
        "day {day}: the day command does not roll the day over"
    );
    assert_eq!(
        var48, command_id,
        "day {day}: {label} writes its real command id"
    );
    assert!(
        game.vm.vars.get(14) < energy_before,
        "day {day}: {label} spends energy ({} -> {})",
        energy_before,
        game.vm.vars.get(14)
    );
    if let (Some(addr), Some(before)) = (stat, stat_before) {
        assert!(
            game.vm.vars.get(addr) > before,
            "day {day}: {label} raises stat var {addr} ({before} -> {})",
            game.vm.vars.get(addr)
        );
    } else {
        assert!(
            game.vm.vars.get(7) > money_before,
            "day {day}: {label} pays money (var7 {money_before} -> {})",
            game.vm.vars.get(7)
        );
    }

    // Evening scene (dinner/CE326 and the CE300 daytime pipeline) plays out to the
    // bedroom. The settle time is script-random (18:30 or 19:30), so wait for the
    // machinery to go idle instead of pinning one minute.
    let mut evening = false;
    for _ in 0..6000 {
        game.frame();
        if !evening_work(game) && game.vm.vars.get(4) >= 18 {
            evening = true;
            break;
        }
    }
    assert!(
        evening,
        "day {day}: the evening scene settles into the bedroom (instances={:?}, clock={}:{})",
        instances_events(game),
        game.vm.vars.get(4),
        game.vm.vars.get(5)
    );
    assert_eq!(
        game.stats.warnings, 0,
        "day {day}: the evening scene is warning-free"
    );
    assert_eq!(
        game.vm.vars.get(160),
        day,
        "day {day}: still day {day} after the evening scene"
    );
    assert!(
        game.vm.vars.get(4) >= 18,
        "day {day}: the evening reaches an evening hour ({}:{})",
        game.vm.vars.get(4),
        game.vm.vars.get(5)
    );
    assert_eq!(
        game.vm.vars.get(5),
        30,
        "day {day}: the evening settles on the half hour"
    );
    assert!(
        game.screen.len() > 20,
        "day {day}: the evening bedroom is drawn: {} pictures",
        game.screen.len()
    );
    var48
}

#[test]
fn day_four_morning_is_forced_into_the_subjugation_battle_wall() {
    let Some(mut game) = open_game() else { return };
    game.auto_advance = true;
    game.set_choice_cursor(0);
    game.auto_choice = true;
    game.auto_choice_delay = 2;

    game.start_map_event_by_name("SampleMapA", 0)
        .expect("the new-game entry point exists");
    for _ in 0..1500 {
        game.frame();
    }
    assert_eq!(game.stats.warnings, 0, "the opening story is warning-free");
    assert_eq!(game.vm.vars.get(4), 18, "the bedroom clock is 18:00");
    assert_eq!(game.vm.vars.get(5), 30, "and 18:30");
    assert_eq!(game.vm.vars.get(160), 1, "on day 1");

    // Day 1 -> day 2. Days 2 and 3 draw the real training menu; take the first row (CE252 鍛錬,
    // non-battle) as m6_multiday proves is safe.
    sleep_into_next_morning(&mut game, 1);
    spend_day_choosing_row(&mut game, 2);
    sleep_into_next_morning(&mut game, 2);
    spend_day_choosing_row(&mut game, 3);
    sleep_into_next_morning(&mut game, 3);

    // Day 4 08:30: the furthest the generic day loop guarantees.
    assert_eq!(game.vm.vars.get(160), 4, "the loop reaches day 4");
    assert_eq!(game.vm.vars.get(6), 3, "on weekday index 3");
    assert_eq!(game.vm.vars.get(4), 8, "at 08:30");
    assert_eq!(game.vm.vars.get(5), 30, "at 08:30");
    assert!(
        game.screen.len() > 20,
        "the day-4 morning scene is drawn: {} pictures",
        game.screen.len()
    );
    assert_eq!(game.stats.warnings, 0, "day 4 08:30 is warning-free");
    assert!(
        !has_battle(&game),
        "day 4 08:30: no battle coroutine is resident yet"
    );

    // From 08:30 the day-4 morning plays a forced story into CE253 -> CE239. Answer the
    // single-option cutscene choices (auto_choice stays off so the story is not silently
    // skipped) and check that (a) no five-row day-command menu is ever drawn and (b) the
    // CE253/CE239 battle appears without the clock leaving 08:30.
    game.auto_advance = true;
    game.auto_choice = false;
    let label_pics = [1201i32, 1203, 1205, 1207, 1209];
    let mut battle_seen = false;
    let mut menu_rows = 0usize;
    for _ in 0..600 {
        game.frame();
        if label_pics.iter().any(|&i| {
            game.screen
                .get(i)
                .map(|p| !p.text.is_empty())
                .unwrap_or(false)
        }) {
            menu_rows += 1;
        }
        if let Some(choices) = game.choices.clone() {
            if choices.frames >= 1 {
                game.set_choice_cursor(0);
                decide(&mut game);
            }
        }
        if has_battle(&game) {
            battle_seen = true;
            break;
        }
    }
    assert_eq!(
        menu_rows, 0,
        "day 4 draws NO training menu (no non-empty 1201/1203/1205/1207/1209 label)"
    );
    assert!(battle_seen, "day 4 daytime is forced into the CE253/CE239 subjugation battle (instances={:?}, clock={}:{})", instances_events(&game), game.vm.vars.get(4), game.vm.vars.get(5));
    assert!(
        has_battle(&game),
        "the CE253/CE239 battle coroutine is resident (instances={:?})",
        instances_events(&game)
    );
    assert_eq!(
        game.stats.warnings, 0,
        "the forced day-4 story is warning-free (no deadlock before the battle)"
    );
    assert_eq!(
        game.vm.vars.get(160),
        4,
        "day 4 never rolls over while the rebuilt battle wiring is off: day 5 is unreachable here"
    );
    assert_eq!(
        game.vm.vars.get(4),
        8,
        "the day-4 story consumes no clock time before the battle"
    );
    assert_eq!(
        game.vm.vars.get(5),
        30,
        "clock still 08:30 at the battle wall"
    );

    let frames = game.stats.frames;
    let instructions = game.stats.instructions;
    eprintln!("m6_week_loop: reached day 4 08:30; day-4 daytime forced into CE253/CE239 battle (day 5 unreachable with the wiring off) after {frames} frames, {instructions} instructions");
    assert!(
        instructions > 200_000,
        "several days of real story ran: {instructions} instructions"
    );
    assert!(frames > 4_000, "the loop really ticked frames: {frames}");
}
