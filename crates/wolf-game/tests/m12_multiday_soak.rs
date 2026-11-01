//! M12 multi-day soak: one generic driver, many consecutive days.
//!
//! The day-by-day tests (`m7_day5_schedule`, `m12_day6_weekend`) prove specific
//! boundaries; this file answers the broader question "does the loop keep going?".
//! A single driver reacts to whatever retail menu is on screen:
//!
//! * weekday CE251 picture menu -> click a CE252 workday row (never 地区貢献 when a
//!   non-battle row is available);
//! * weekend CE310 ■休日 menu -> 自主锻炼 while energy is healthy, otherwise 悠然自得;
//! * story days, morning greetings and battles -> feed OK and let the real scripts run;
//! * evening -> real dinner, then the real CE329 sleep prompt (only `var63`, the
//!   shared sister-click flag, is seeded, exactly as every other sleep test does).
//!
//! The 30-day test reaches day 31 08:30 with `warnings == 0`. Along the way it
//! exercises real battles from dispatchers other than CE253 (day 11+): CE305
//! キャンセナル calls CE239 directly, which is why `BattleWiring` now also auto-arms on
//! a parented CE239 instance (see `crates/wolf-game/src/battle_wiring.rs`).
//!
//! A `#[ignore]`d 90-day test runs the same driver much further and shows a second
//! retail behaviour: a scripted story jump moves the day counter by itself (day 84 ->
//! 97 in the measured run). The driver treats that as a completed day instead of an
//! error, which is why it is not a `for day in ..` loop.
//!
//! Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m12_multiday_soak -- --nocapture
//! Long run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m12_multiday_soak -- --ignored

use std::path::PathBuf;

use wolf_game::db::Container;
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

fn data_root() -> PathBuf {
    if let Ok(raw) = std::env::var("WOLF_DATA_ROOT") {
        return resolve_root(&raw);
    }
    let full = resolve_root("work/full");
    if full.join("Save").join("System.sav").exists()
        || full.join("save").join("system.sav").exists()
    {
        full
    } else {
        resolve_root("work/all")
    }
}

fn decide(game: &mut Game) {
    game.key_bits = wolf_game::key::OK;
    game.frame();
    game.key_bits = 0;
}

fn has(game: &Game, event: u32) -> bool {
    game.vm.instances().iter().any(|i| i.event == event)
}

const SLEEP_OPTIONS: [&str; 3] = ["恶作剧", "轻轻把被子盖好", "今天还是睡吧"];
const CONFIRM_OPTIONS: [&str; 2] = ["是的", "再想想"];
const DAY_LABELS: [&str; 6] = [
    "给店里帮忙", "学习会", "基础锻炼", "攻击练习", "防御练习", "秘密特训",
];
const ROW_PICS: [i32; 5] = [1201, 1203, 1205, 1207, 1209];

fn rows(game: &Game) -> Vec<String> {
    ROW_PICS
        .iter()
        .map(|&number| game.screen.get(number).map(|p| p.text.clone()).unwrap_or_default())
        .collect()
}

fn evening_work(game: &Game) -> bool {
    game.vm
        .instances()
        .iter()
        .any(|instance| matches!(instance.event, 251 | 300 | 301 | 326 | 406))
}

fn click_picture(game: &mut Game, number: i32) -> (i32, i32) {
    let picture = game.screen.get(number).expect("menu picture").clone();
    game.mouse = [picture.x, picture.y];
    for _ in 0..10 {
        game.frame();
    }
    game.mouse_buttons = 1;
    for _ in 0..5 {
        game.frame();
    }
    game.mouse_buttons = 0;
    game.mouse = [-1, -1];
    (picture.x, picture.y)
}

/// What one played day produced, for the soak assertions.
#[derive(Default)]
struct DayReport {
    battle: bool,
    weekday_actions: usize,
    weekend_actions: usize,
}

/// Play one day from morning to the evening. The `i32` result is the day counter at
/// the end: normally the same day, but a scripted story jump can move it further.
fn play_day(game: &mut Game, day: i32) -> (i32, DayReport) {
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let mut report = DayReport::default();
    let mut last_action: Option<(i32, i32, i32)> = None;
    let mut last_action_frame = 0usize;
    let start_frames = game.stats.frames;

    for frame in 0..200_000usize {
        if has(game, 239) || has(game, 253) {
            report.battle = true;
        }
        let menu_rows = rows(game);
        let weekend = menu_rows.iter().any(|t| t.contains("悠然自得")) && has(game, 310);
        let weekday = menu_rows
            .iter()
            .any(|t| DAY_LABELS.iter().any(|l| t.contains(l)) || t.contains("地区贡献"))
            && has(game, 251);
        let clock_key = (
            game.vm.vars.get(4),
            game.vm.vars.get(5),
            if weekend { 2 } else { 1 },
        );
        // A refused option leaves the menu on the same clock; allow a retry after a pause
        // so the driver can pick a different row (usually 悠然自得 with low energy).
        if last_action == Some(clock_key) && frame.saturating_sub(last_action_frame) > 400 {
            last_action = None;
        }
        let can_click = last_action != Some(clock_key);

        if can_click && weekend {
            let want_rest = game.vm.vars.get(14) < 30;
            let row = if want_rest {
                menu_rows.iter().position(|t| t.contains("悠然自得"))
            } else {
                menu_rows
                    .iter()
                    .position(|t| t.contains("自主锻炼"))
                    .or_else(|| menu_rows.iter().position(|t| t.contains("悠然自得")))
            };
            if let Some(row) = row {
                click_picture(game, ROW_PICS[row]);
                report.weekend_actions += 1;
                last_action = Some(clock_key);
                last_action_frame = frame;
                continue;
            }
        }

        if can_click && weekday {
            let row = menu_rows
                .iter()
                .position(|t| DAY_LABELS.iter().any(|l| t.contains(l)))
                .or_else(|| menu_rows.iter().position(|t| !t.is_empty()));
            if let Some(row) = row {
                let y = 230 + 50 * row as i32;
                game.mouse = [512, y];
                for _ in 0..10 {
                    game.frame();
                }
                game.mouse_buttons = 1;
                for _ in 0..5 {
                    game.frame();
                }
                game.mouse_buttons = 0;
                game.mouse = [-1, -1];
                report.weekday_actions += 1;
                last_action = Some(clock_key);
                last_action_frame = frame;
                continue;
            }
        }

        game.key_bits = if frame % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        game.key_bits = 0;
        let current_day = game.vm.vars.get(160);
        if current_day != day {
            // A scripted story jump (measured: day 84 -> day 97 during CE317). It is a
            // completed stretch of calendar, not a driver error.
            eprintln!("day {day}: scripted jump to {current_day} at frame {frame}");
            return (current_day, report);
        }
        if !evening_work(game) && game.vm.vars.get(4) >= 18 {
            eprintln!(
                "day {day}: end {}:{:02} energy={} battle={} weekday_actions={} weekend_actions={} frames={} warnings={}",
                game.vm.vars.get(4),
                game.vm.vars.get(5),
                game.vm.vars.get(14),
                report.battle,
                report.weekday_actions,
                report.weekend_actions,
                game.stats.frames - start_frames,
                game.stats.warnings
            );
            return (day, report);
        }
    }
    panic!(
        "day {day}: stuck at {}:{} energy={} warnings={} live={:?} rows={:?}",
        game.vm.vars.get(4),
        game.vm.vars.get(5),
        game.vm.vars.get(14),
        game.stats.warnings,
        game.vm
            .instances()
            .iter()
            .map(|i| format!("{:?}@{}", i.event, i.pc))
            .collect::<Vec<_>>(),
        rows(game)
    );
}

/// The real sleep prompt: feed OK until it appears, then 今天还是睡吧 -> 是的.
fn sleep_to_next_morning(game: &mut Game, day: i32) {
    let expected = day + 1;
    game.auto_choice = false;
    game.vm.vars.set(63, 1);
    let mut options = Vec::new();
    for frame in 0..2400 {
        game.key_bits = if frame % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        if let Some(choice) = &game.choices {
            options = choice.options.clone();
            break;
        }
    }
    game.key_bits = 0;
    assert_eq!(
        options,
        SLEEP_OPTIONS.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
        "day {day}: unexpected sleep menu at {}:{} live={:?}",
        game.vm.vars.get(4),
        game.vm.vars.get(5),
        game.vm
            .instances()
            .iter()
            .map(|i| format!("{:?}@{}", i.event, i.pc))
            .collect::<Vec<_>>()
    );
    game.set_choice_cursor(2); // 今天还是睡吧
    decide(game);
    let mut confirm = Vec::new();
    for _ in 0..300 {
        game.frame();
        if let Some(choice) = &game.choices {
            confirm = choice.options.clone();
            break;
        }
    }
    assert_eq!(
        confirm,
        CONFIRM_OPTIONS.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
        "day {day}: unexpected sleep confirm"
    );
    game.set_choice_cursor(0); // 是的
    decide(game);
    let mut arrived = false;
    for _ in 0..2400 {
        game.frame();
        if game.vm.vars.get(160) == expected
            && ((game.vm.vars.get(4) == 8 && game.vm.vars.get(5) >= 30)
                || (game.vm.vars.get(4) >= 9 && game.vm.vars.get(5) >= 30)
                || game.vm.vars.get(4) >= 6)
        {
            arrived = true;
            break;
        }
    }
    assert!(
        arrived,
        "day {expected}: morning not reached (v160={} v4={} v5={})",
        game.vm.vars.get(160),
        game.vm.vars.get(4),
        game.vm.vars.get(5)
    );
}

#[derive(Default)]
struct SoakSummary {
    reached: i32,
    battle_days: Vec<i32>,
    weekend_days: Vec<i32>,
    weekday_days: Vec<i32>,
    jumped_days: Vec<(i32, i32)>,
}

/// Boot a new game and play through `target` calendar days with one driver.
fn soak(target: i32) -> Option<SoakSummary> {
    let mut game = match Game::new(data_root()) {
        Ok(game) => game,
        Err(err) => {
            eprintln!("skipping: {err}");
            return None;
        }
    };
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 2;
    game.set_choice_cursor(0);
    game.start_map_event_by_name("SampleMapA", 0)
        .expect("new-game entry event");
    for _ in 0..1600 {
        game.frame();
    }
    assert_eq!(game.vm.vars.get(160), 1, "day 1 reached");
    assert_eq!(game.stats.warnings, 0);

    // Real battle database seed (party stats / command slots / item stock) and the one
    // cosmetic command-menu section-header label a fresh save leaves blank. No settlement
    // or schedule value is seeded.
    assert!(game.seed_battle_databases() > 0);
    game.databases
        .set_text(Container::CData, 5, 16, 0, "┗ 状態リスト");
    let _ = game.start_engine_events();

    let mut summary = SoakSummary::default();
    sleep_to_next_morning(&mut game, 1);
    let mut day = 2;
    while day <= target {
        let (end_day, report) = play_day(&mut game, day);
        if report.battle {
            summary.battle_days.push(day);
        }
        if report.weekend_actions > 0 {
            summary.weekend_days.push(day);
        }
        if report.weekday_actions > 0 {
            summary.weekday_days.push(day);
        }
        if end_day != day {
            summary.jumped_days.push((day, end_day));
            day = end_day;
            continue;
        }
        sleep_to_next_morning(&mut game, day);
        day += 1;
        assert_eq!(
            game.vm.vars.get(6),
            (day - 1) % 7,
            "day {day} weekday: expected {}, got {}",
            (day - 1) % 7,
            game.vm.vars.get(6)
        );
        assert_eq!(game.stats.warnings, 0, "day {day} warnings");
    }
    // Let the final morning chain reach its full clock (the sleep helper stops on the
    // first morning frame, e.g. 07:45).
    let reached = game.vm.vars.get(160);
    for _ in 0..1200 {
        game.frame();
        if game.vm.vars.get(160) == reached && game.vm.vars.get(4) >= 8 {
            break;
        }
    }
    summary.reached = reached;
    assert_eq!(game.stats.warnings, 0, "the whole soak is warning-free");
    eprintln!(
        "soak to day {reached}: {}:{} battle_days={:?} weekend_days={:?} weekday_menu_days={} jumps={:?} frames={} instructions={} warnings=0",
        game.vm.vars.get(4),
        game.vm.vars.get(5),
        summary.battle_days,
        summary.weekend_days,
        summary.weekday_days.len(),
        summary.jumped_days,
        game.stats.frames,
        game.stats.instructions
    );
    Some(summary)
}

#[test]
fn thirty_days_advance_without_deadlock_or_warnings() {
    const TARGET: i32 = 30;
    let Some(summary) = soak(TARGET) else {
        return;
    };

    assert_eq!(summary.reached, TARGET + 1, "day 31 reached");
    assert!(summary.battle_days.contains(&4), "day 4 is the forced CE253 battle");
    assert!(
        summary.battle_days.iter().any(|day| *day > 10),
        "later story battles must also settle (day 11+ CE305 -> CE239), got {:?}",
        summary.battle_days
    );
    // A played day `d` starts on weekday `(d - 1) % 7`, so its weekend slots are
    // d % 7 == 6 (Saturday) and d % 7 == 0 (Sunday).
    let expected_weekends: Vec<i32> = (6..=TARGET)
        .filter(|day| matches!(day % 7, 6 | 0))
        .collect();
    for day in &expected_weekends {
        assert!(
            summary.weekend_days.contains(day),
            "weekend day {day} must draw CE310, got {:?}",
            summary.weekend_days
        );
    }
    assert!(
        summary.weekday_days.len() >= 5,
        "weekdays must draw the CE251 workday menu repeatedly, got {:?}",
        summary.weekday_days
    );
    assert!(summary.jumped_days.is_empty(), "no story jump inside 30 days");
}

/// The same driver over the whole retail-style long run. Ignored by default because
/// it costs ~45 s; it documents that the loop survives day-jumping scripts.
#[test]
#[ignore = "long soak: ~45 s, run with --ignored"]
fn ninety_days_survive_story_day_jumps() {
    let Some(summary) = soak(90) else {
        return;
    };
    assert!(summary.reached >= 90, "reached day {}", summary.reached);
    assert!(
        !summary.jumped_days.is_empty(),
        "the long run must exercise a scripted day jump"
    );
    for (from, to) in &summary.jumped_days {
        assert!(to > from, "story jump {from} -> {to} must move forward");
    }
    assert!(summary.battle_days.contains(&4));
    assert!(summary.battle_days.len() >= 10, "many story battles settle");
    assert!(summary.weekend_days.len() >= 20, "weekends keep drawing CE310");
}