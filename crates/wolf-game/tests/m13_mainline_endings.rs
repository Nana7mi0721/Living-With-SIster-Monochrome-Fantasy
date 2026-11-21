//! M13 P0-F — 主线后半自然驱动与结局复验 (mainline endings, post common-self fix).
//!
//! 目的（2026-11-01 口径：游戏体验内容与原版一致 + 修复 bug）：
//!   1. 证明「正常通关」与「逃亡」两条结局脚本内容存在且可正常渲染。
//!   2. 证明 common-event self 持久修复（7c3b728）后，最远的零售存档
//!      (SaveData01 / SaveData08，均为 day90) 可以**无需注入**逐日自然推进到
//!      stage28 決着 → stage29 旅路 → 通关贺词，并置 var492=1。
//!   3. 如实记录 stage30 的真实身份（尖叫胡萝卜支线，非大结局）。
//!
//! 数据根：work/all（可用 WOLF_DATA_ROOT 覆盖）。
//! 零售存档：work/retail_backup/Save/SaveDataNN.sav（WOLF_RETAIL_SAVE_DIR 不改时用默认）。
//!
//! 复现命令（在 repo 根，务必 pin 环境变量，否则 wolf-save 会读用户 Steam 安装档）：
//!   # 默认测试（快，约 1s）
//!   WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m13_mainline_endings
//!   # 长跑：自然正常通关（SaveData01 day90 -> stage28/29），约 3s
//!   M13_SLOT=1 cargo test -q -p wolf-game --test m13_mainline_endings drive_normal_path_diagnostic -- --ignored --nocapture
//!   # 长跑：自然正常通关（SaveData08 day90 00:42 深夜，先结束 H 场景再推进），约 3s
//!   M13_SLOT=8 cargo test -q -p wolf-game --test m13_mainline_endings drive_normal_path_diagnostic -- --ignored --nocapture
//!   # 长跑：逃亡 END（SaveData05 var366=0 -> day100+ stage27），约数秒
//!   M13_SLOT=5 cargo test -q -p wolf-game --test m13_mainline_endings drive_escape_path_from_retail_save -- --ignored --nocapture
//!
//! 结论与逐项证据详见 docs/research/M13_MAINLINE_ENDINGS.md。
use std::path::PathBuf;
use wolf_game::Game;

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}
fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/all".into());
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        p
    } else {
        repo_root().join(p)
    }
}
fn retail_dir() -> Option<PathBuf> {
    let d = repo_root().join("work/retail_backup/Save");
    d.is_dir().then_some(d)
}
fn sample_map() -> wolf_format::mps::Map {
    let bytes = std::fs::read(data_root().join("MapData/SampleMapA.mps")).expect("map");
    wolf_format::mps::Map::parse(&bytes).expect("parse")
}
#[allow(dead_code)]
fn live(game: &Game) -> Vec<String> {
    game.vm
        .instances()
        .iter()
        .map(|i| format!("{}@{}", i.event, i.pc))
        .collect()
}
fn stage_of(game: &Game) -> i32 {
    for i in game.vm.instances() {
        if i.event == 305 {
            return i.self_vars.get(600);
        }
    }
    -1
}
fn decide(game: &mut Game) {
    game.key_bits = wolf_game::key::OK;
    game.frame();
    game.key_bits = 0;
}
#[allow(dead_code)]
fn has(game: &Game, event: u32) -> bool {
    game.vm.instances().iter().any(|i| i.event == event)
}

const SLEEP_OPTIONS: [&str; 3] = ["恶作剧", "轻轻把被子盖好", "今天还是睡吧"];
const DAY_LABELS: [&str; 6] = [
    "给店里帮忙",
    "学习会",
    "基础锻炼",
    "攻击练习",
    "防御练习",
    "秘密特训",
];
const ROW_PICS: [i32; 5] = [1201, 1203, 1205, 1207, 1209];

fn rows(game: &Game) -> Vec<String> {
    ROW_PICS
        .iter()
        .map(|&n| {
            game.screen
                .get(n)
                .map(|p| p.text.clone())
                .unwrap_or_default()
        })
        .collect()
}
fn evening_work(game: &Game) -> bool {
    game.vm
        .instances()
        .iter()
        .any(|i| matches!(i.event, 251 | 300 | 301 | 326 | 406))
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

/// Resume a retail save into a normally-running day loop.
/// start_pages skips event 0 (ニューゲーム処理) so the loaded day is NOT reset to day 1.
fn resume(slot: u32) -> Game {
    let map = sample_map();
    let mut game = Game::new(data_root()).expect("game");
    game.load_save(&retail_dir().unwrap().join(format!("SaveData{slot:02}.sav")))
        .expect("load");
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    game.start_engine_events();
    game.start_pages(&map, Some(0));
    game
}

/// Plays one day, stops at evening. Mirrors m12_multiday_soak.rs.
#[allow(dead_code)]
fn play_day(game: &mut Game, day: i32, log: &mut impl FnMut(&Game, usize)) {
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let mut last_action: Option<(i32, i32, i32)> = None;
    let mut last_action_frame = 0usize;
    for frame in 0..200_000usize {
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
        if last_action == Some(clock_key) && frame.saturating_sub(last_action_frame) > 400 {
            last_action = None;
        }
        let can_click = last_action != Some(clock_key);
        log(game, frame);
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
                last_action = Some(clock_key);
                last_action_frame = frame;
                continue;
            }
        }
        game.key_bits = if frame % 2 == 0 {
            wolf_game::key::OK
        } else {
            0
        };
        game.frame();
        game.key_bits = 0;
        if game.vm.vars.get(160) != day {
            eprintln!(
                "day {day}: scripted jump to {} at frame {frame}",
                game.vm.vars.get(160)
            );
            return;
        }
        if !evening_work(game) && game.vm.vars.get(4) >= 18 {
            eprintln!(
                "day {day}: evening {}:{:02} v366={} stage={}",
                game.vm.vars.get(4),
                game.vm.vars.get(5),
                game.vm.vars.get(366),
                stage_of(game)
            );
            return;
        }
    }
    panic!(
        "day {day}: stuck {}:{} live={:?} rows={:?}",
        game.vm.vars.get(4),
        game.vm.vars.get(5),
        live(game),
        rows(game)
    );
}

/// Opens the night menu by setting var63=1, chooses 今天还是睡吧 (index 2) then 是的 (index 0).
#[allow(dead_code)]
fn sleep_to_next_morning(game: &mut Game, day: i32) -> bool {
    let expected = day + 1;
    game.auto_choice = false;
    game.vm.vars.set(63, 1);
    let mut got = false;
    for frame in 0..2400 {
        game.key_bits = if frame % 2 == 0 {
            wolf_game::key::OK
        } else {
            0
        };
        game.frame();
        if let Some(choice) = &game.choices {
            let opts = choice.options.clone();
            if opts
                == SLEEP_OPTIONS
                    .iter()
                    .map(|s| (*s).to_owned())
                    .collect::<Vec<_>>()
            {
                got = true;
            } else {
                eprintln!(
                    "day {day}: sleep menu was {:?} @{}:{}",
                    opts,
                    game.vm.vars.get(4),
                    game.vm.vars.get(5)
                );
            }
            break;
        }
    }
    game.key_bits = 0;
    if !got {
        eprintln!(
            "day {day}: no sleep menu (v160={} {}:{})",
            game.vm.vars.get(160),
            game.vm.vars.get(4),
            game.vm.vars.get(5)
        );
        return false;
    }
    game.set_choice_cursor(2);
    decide(game);
    for _ in 0..300 {
        game.frame();
        if game.choices.is_some() {
            break;
        }
    }
    game.set_choice_cursor(0);
    decide(game);
    for _ in 0..2400 {
        game.frame();
        if game.vm.vars.get(160) == expected {
            eprintln!(
                "reached day {expected} at {}:{:02}",
                game.vm.vars.get(4),
                game.vm.vars.get(5)
            );
            return true;
        }
    }
    eprintln!(
        "day {expected}: morning not reached v160={} {}:{}",
        game.vm.vars.get(160),
        game.vm.vars.get(4),
        game.vm.vars.get(5)
    );
    false
}

/// Advances from the loaded save state to the next morning.
///
/// - 傍晚/正常时刻（hour >= 5）：走 sleep_to_next_morning（var63=1 → 今天还是睡吧）。
/// - 深夜（hour < 5，如 SaveData08 的 day90 00:42）：此时可能停在一个交互场景（例如 H 场景，
///   屏幕上有 pic 294 = Picture/endbutton.png）。先点「结束」按钮关闭该场景，再连续喂 OK
///   让引擎自然过渡到次日 07:45。整个过程不修改任何存档变量，纯输入驱动。
fn reach_next_morning(game: &mut Game) -> bool {
    let day0 = game.vm.vars.get(160);
    if game.vm.vars.get(4) < 5 {
        // Dismiss a deep-night interactive scene via its end button, if present.
        for _ in 0..3 {
            if game.screen.get(294).is_some() {
                click_picture(game, 294);
                for _ in 0..90 {
                    game.frame();
                }
            } else {
                break;
            }
        }
        for f in 0..8000 {
            game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
            game.frame();
            game.key_bits = 0;
            if game.vm.vars.get(160) != day0 {
                eprintln!(
                    "deep-night -> morning: day {} {}:{:02} (was day {day0})",
                    game.vm.vars.get(160),
                    game.vm.vars.get(4),
                    game.vm.vars.get(5)
                );
                return true;
            }
        }
        eprintln!(
            "deep-night: still day {day0} {}:{:02} after 8000 frames",
            game.vm.vars.get(4),
            game.vm.vars.get(5)
        );
        return false;
    }
    sleep_to_next_morning(game, day0)
}

/// Directly starts common event 305 with self0 (=stage) pre-seeded and runs it to completion.
/// Returns (stage progression, final ending-flag tuple, warnings).
/// stage = CE305 self0 = magic 1_600_000 (flat self slot 600); each stage is its own story block.
fn run_injected_stage(stage: i32, verbose: bool) -> (Vec<(i32, usize)>, [i32; 6], usize) {
    let mut game = Game::new(data_root()).expect("game");
    if let Some(dir) = retail_dir() {
        let _ = game.load_save(&dir.join("SaveData01.sav"));
    }
    let flags = |g: &Game| {
        [
            g.vm.vars.get(298),
            g.vm.vars.get(466),
            g.vm.vars.get(491),
            g.vm.vars.get(492),
            g.vm.vars.get(493),
            g.vm.vars.get(619),
        ]
    };
    let before = flags(&game);
    let id = game.vm.start_event(305).expect("start305");
    game.vm.instance_mut(id).unwrap().self_vars.set(600, stage);
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let mut stages: Vec<(i32, usize)> = Vec::new();
    let mut msgs_seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for f in 0..20000usize {
        for inst in game.vm.instances() {
            if inst.event == 305 {
                let s = inst.self_vars.get(600);
                if !stages.iter().any(|(x, _)| *x == s) {
                    stages.push((s, f));
                }
            }
        }
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        game.key_bits = 0;
        if verbose {
            for m in game.messages.iter() {
                let t = m.text.trim();
                if !t.is_empty() && msgs_seen.insert(t.to_string()) {
                    eprintln!("  MSG @f{} <<{}>>", f, t);
                }
            }
        }
        if game.vm.instances().iter().all(|i| i.event != 305) {
            if verbose {
                eprintln!("  CE305 finished @f{}", f);
            }
            break;
        }
    }
    let after = flags(&game);
    if verbose {
        eprintln!("STAGE {stage}: progression={:?}", stages);
        eprintln!(
            "  BEFORE={:?} AFTER={:?} warn={}",
            before, after, game.stats.warnings
        );
    }
    (stages, after, game.stats.warnings as usize)
}

// ---------------------------------------------------------------------------
// 默认测试（快）— 证明两条结局脚本存在、可渲染；并明确 stage30 是支线
// ---------------------------------------------------------------------------

#[test]
fn true_ending_script_renders_from_stage28() {
    if retail_dir().is_none() {
        eprintln!("retail saves absent, skip");
        return;
    }
    // stage28 決着 尾部会自动 210 [500305,1,29] 调起 stage29 旅路 并跑完，末段给出通关贺词。
    let (prog, after, warn) = run_injected_stage(28, false);
    assert!(
        prog.iter().any(|(s, _)| *s == 28),
        "stage28 not entered: {:?}",
        prog
    );
    assert!(
        prog.iter().any(|(s, _)| *s == 29),
        "stage28 did not chain into stage29: {:?}",
        prog
    );
    assert_eq!(
        after[3], 1,
        "normal clear should set var492=1, flags={:?}",
        after
    );
    assert_eq!(warn, 0, "warnings should be 0");
}

#[test]
fn escape_ending_script_renders_from_stage27() {
    if retail_dir().is_none() {
        eprintln!("retail saves absent, skip");
        return;
    }
    // stage27 期限切れ = BAD END 逃亡。
    let mut game = Game::new(data_root()).expect("game");
    let _ = game.load_save(&retail_dir().unwrap().join("SaveData01.sav"));
    let id = game.vm.start_event(305).expect("start305");
    game.vm.instance_mut(id).unwrap().self_vars.set(600, 27);
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    // 结局文本在结末帧被清屏，必须跨帧累积所有 message 才能断言。
    let mut log = String::new();
    for f in 0..4000usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        game.key_bits = 0;
        for m in game.messages.iter() {
            let t = m.text.trim();
            if !t.is_empty() && !log.contains(t) {
                log.push_str(t);
                log.push('\n');
            }
        }
        if game.vm.instances().iter().all(|i| i.event != 305) {
            break;
        }
    }
    assert!(
        log.contains("BAD END"),
        "escape ending should render BAD END text; log={log}"
    );
    assert!(
        log.contains("逃亡"),
        "escape ending should render 逃亡; log={log}"
    );
    assert_eq!(game.stats.warnings, 0, "warnings should be 0");
}

/// stage30（CE305 的 @index 3404「エンディング」标签 / CE251 idx913 派发点）不是大结局，
/// 而是「尖叫胡萝卜」支线小事件：找魔物→无害→放走，返回后只「干劲提升」，不置任何结局旗标。
/// 这里明确断言它不设置 var492（大结局旗标），把「支线」与「大结局」区分开，避免夸大。
#[test]
fn stage30_is_side_event_not_grand_ending() {
    if retail_dir().is_none() {
        eprintln!("retail saves absent, skip");
        return;
    }
    let mut game = Game::new(data_root()).expect("game");
    let _ = game.load_save(&retail_dir().unwrap().join("SaveData01.sav"));
    let id = game.vm.start_event(305).expect("start305");
    game.vm.instance_mut(id).unwrap().self_vars.set(600, 30);
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let mut log = String::new();
    for f in 0..4000usize {
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        game.key_bits = 0;
        for m in game.messages.iter() {
            let t = m.text.trim();
            if !t.is_empty() && !log.contains(t) {
                log.push_str(t);
                log.push('\n');
            }
        }
        if game.vm.instances().iter().all(|i| i.event != 305) {
            break;
        }
    }
    assert!(
        log.contains("胡萝卜"),
        "stage30 should be the 尖叫胡萝卜 side event; log={log}"
    );
    assert!(
        !log.contains("恭喜"),
        "stage30 is NOT the grand ending; log={log}"
    );
    assert_eq!(
        game.vm.vars.get(492),
        0,
        "stage30 must not set the grand-ending flag var492"
    );
    assert_eq!(game.stats.warnings, 0, "warnings should be 0");
}

// ---------------------------------------------------------------------------
// 长跑（#[ignore]）— 用零售存档自然驱动（无需注入）
// ---------------------------------------------------------------------------

/// 逃亡 END 自然可达：从 var366=0 的零售存档 (SaveData05, day73) 逐日推进到 day102，
/// CE317 早朝処理在 day>=100 且 var366==0 时唤 CE305 stage27（期限切れ / BAD END 逃亡）。
#[test]
#[ignore]
fn drive_escape_path_from_retail_save() {
    let slot = std::env::var("M13_SLOT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5u32);
    let max_day: i32 = std::env::var("M13_MAXDAY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(102);
    let mut game = resume(slot);
    reach_next_morning(&mut game);
    let mut last_stage = -1;
    let mut saw27 = false;
    let mut escape_day: i32 = -1;
    while game.vm.vars.get(160) <= max_day {
        let day = game.vm.vars.get(160);
        let mut logger = |g: &Game, frame: usize| {
            let s = stage_of(g);
            if s == 27 {
                saw27 = true;
                if last_stage != 27 {
                    eprintln!("  >> ESCAPE END: CE305 stage27 期限切れ @f{} day={} {}:{:02} v366={} pc={:?}",
                        frame, g.vm.vars.get(160), g.vm.vars.get(4), g.vm.vars.get(5), g.vm.vars.get(366),
                        g.vm.instances().iter().find(|i| i.event == 305).map(|i| i.pc));
                }
            }
            if s != last_stage && s != -1 {
                eprintln!(
                    "  >> STAGE {} @f{} day={} {}:{:02}",
                    s,
                    frame,
                    g.vm.vars.get(160),
                    g.vm.vars.get(4),
                    g.vm.vars.get(5)
                );
                last_stage = s;
            }
        };
        play_day(&mut game, day, &mut logger);
        if saw27 {
            escape_day = day;
            break;
        }
        let d2 = game.vm.vars.get(160);
        eprintln!(
            "day{day}: evening {}:{:02} v366={} v492={} v493={} ev405={} (no escape yet)",
            game.vm.vars.get(4),
            game.vm.vars.get(5),
            game.vm.vars.get(366),
            game.vm.vars.get(492),
            game.vm.vars.get(493),
            has(&game, 405)
        );
        if d2 >= max_day {
            break;
        }
        if !reach_next_morning(&mut game) {
            break;
        }
    }
    assert!(
        saw27,
        "escape END (CE305 stage27 期限切れ) not dispatched within day {max_day}"
    );
    assert!(escape_day > 0, "escape day not recorded");
    assert_eq!(game.stats.warnings, 0, "warnings should be 0");
}

/// 正常通关自然驱动（无需注入）：从最远的零售存档 (SaveData01/SaveData08, day90) 逐日推进。
///
/// M13 common-self 修复前（7c3b728 之前）：spawn_child 给每个 common-event 实例新建全 0
/// SelfVars，CE251 的交流战倒计时 self51 每天复位为 4、self65 复位为 1，body 每天重放，
/// self51 永远到不了 0，stage28 決着 不可达。修复后 common_self 银行跨调用持久，
/// self51 递减 4→3→2→1→0，self51==0 时 CE251 idx400 以 210 [500305,1,28] 唤 stage28，
/// stage28 尾部自动接 stage29 旅路，末段给出「恭喜您完成了整个故事剧情，辛苦了！」并置 var492=1。
///
/// 环境变量：M13_SLOT（默认 1；用 8 验证 SaveData08 深夜存档）、M13_DAYS（默认 30）。
#[test]
#[ignore]
fn drive_normal_path_diagnostic() {
    let slot = std::env::var("M13_SLOT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1u32);
    let max_days: i32 = std::env::var("M13_DAYS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    let mut game = resume(slot);
    reach_next_morning(&mut game);
    let start_day = game.vm.vars.get(160);
    let end = start_day + max_days;

    let mut daily: Vec<(i32, i32, i32)> = Vec::new();
    let mut saw28 = false;
    let mut saw29 = false;
    let mut saw_high51 = false;
    let mut zero_after_high = false;
    let mut stage28_at: Option<(i32, usize)> = None;
    let mut frames = 0usize;
    let mut stages_seen: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    let mut all_msgs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut collect_msgs = |g: &Game| {
        for m in g.messages.iter() {
            let t = m.text.trim();
            if !t.is_empty() {
                all_msgs.insert(t.to_string());
            }
        }
    };

    while game.vm.vars.get(160) < end && game.vm.vars.get(366) != 2 && !saw28 {
        let day = game.vm.vars.get(160);
        let mut max51 = i32::MIN;
        let mut max65 = i32::MIN;
        let mut logger = |g: &Game, f: usize| {
            frames += 1;
            collect_msgs(g);
            for i in g.vm.instances() {
                if i.event == 305 {
                    let s = i.self_vars.get(600);
                    stages_seen.insert(s);
                    if s == 28 {
                        saw28 = true;
                        if stage28_at.is_none() {
                            stage28_at = Some((g.vm.vars.get(160), f));
                        }
                    }
                    if s == 29 {
                        saw29 = true;
                    }
                }
            }
            // Prefer the live CE251 instance; fall back to the persisted bank
            // (the fix stores it on reap / stop), so the countdown is visible
            // even on days when CE251 is not currently running.
            let mut live51 = None;
            let mut live65 = None;
            for i in g.vm.instances() {
                if i.event == 251 {
                    live51 = Some(i.self_vars.get(651));
                    live65 = Some(i.self_vars.get(665));
                }
            }
            let s51 = live51.or_else(|| g.vm.common_self(251).map(|b| b.get(651)));
            let s65 = live65.or_else(|| g.vm.common_self(251).map(|b| b.get(665)));
            if let Some(v) = s51 {
                if v > max51 {
                    max51 = v;
                }
                if v >= 2 {
                    saw_high51 = true;
                }
                if v == 0 && saw_high51 {
                    zero_after_high = true;
                }
            }
            if let Some(v) = s65 {
                if v > max65 {
                    max65 = v;
                }
            }
        };
        play_day(&mut game, day, &mut logger);
        let v51 = if max51 == i32::MIN { -1 } else { max51 };
        let v65 = if max65 == i32::MIN { -1 } else { max65 };
        daily.push((day, v51, v65));
        eprintln!(
            "day{day}: v6={} v366={} v367={} CE251.self51={} self65={} stage={} v492={}",
            game.vm.vars.get(6),
            game.vm.vars.get(366),
            game.vm.vars.get(367),
            v51,
            v65,
            stage_of(&game),
            game.vm.vars.get(492)
        );
        if saw28 || game.vm.vars.get(366) == 2 {
            break;
        }
        let d2 = game.vm.vars.get(160);
        if d2 >= end {
            break;
        }
        if !reach_next_morning(&mut game) {
            break;
        }
    }

    // stage28 決着 由 CE251 idx400 唤出；play_day 会在 day 跳变时提前返回，
    // 此时 stage28/29 可能仍在跑；继续空转帧让 28→29 旅路 完整播完。
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    for f in 0..120_000usize {
        frames += 1;
        collect_msgs(&game);
        for i in game.vm.instances() {
            if i.event == 305 {
                let s = i.self_vars.get(600);
                stages_seen.insert(s);
                if s == 28 {
                    saw28 = true;
                    if stage28_at.is_none() {
                        stage28_at = Some((game.vm.vars.get(160), f));
                    }
                }
                if s == 29 {
                    saw29 = true;
                }
            }
        }
        if stages_seen.contains(&29)
            && !game.vm.instances().iter().any(|i| i.event == 305)
            && game.vm.vars.get(492) == 1
        {
            break;
        }
        game.key_bits = if f % 2 == 0 { wolf_game::key::OK } else { 0 };
        game.frame();
        game.key_bits = 0;
    }
    let saw_congrats = all_msgs.iter().any(|m| m.contains("恭喜"));
    eprintln!(
        "normal path slot{slot}: start_day={start_day} days_driven={} frames={frames} stage28_at={:?} saw28={saw28} saw29={saw29} stages={stages_seen:?} v492={} v366={} warnings={} saw_congrats={saw_congrats}",
        daily.len(), stage28_at, game.vm.vars.get(492), game.vm.vars.get(366), game.stats.warnings
    );
    eprintln!("daily self51/self65: {daily:?}");
    if saw_congrats {
        for m in &all_msgs {
            if m.contains("恭喜") {
                eprintln!("  CONGRATS MSG: {m}");
            }
        }
    }

    // self51 必须先高后降到 0（不再每天复位为 4）。
    assert!(
        saw_high51,
        "CE251 self51 should start high (>=2) before counting down"
    );
    assert!(
        zero_after_high,
        "CE251 self51 should reach 0 after having been high (countdown completed): daily={daily:?}"
    );
    assert!(
        saw28,
        "CE305 stage28 決着 should be dispatched along the natural path: daily={daily:?}"
    );
    assert!(
        saw29,
        "stage28 should chain into stage29 旅路 along the natural path: daily={daily:?}"
    );
    assert_eq!(
        game.vm.vars.get(492),
        1,
        "normal clear should set var492=1, v366={}",
        game.vm.vars.get(366)
    );
    assert!(
        saw_congrats,
        "the grand-ending completion text (恭喜...) should render along the natural path"
    );
    assert_eq!(game.stats.warnings, 0, "warnings should be 0");
}
