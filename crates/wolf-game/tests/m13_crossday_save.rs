//! M13 P0-E: cross-day save/load state continuity + all eight retail slot round-trips.
//!
//! What this proves (completion bar 2026-11-01: content parity + bug fixing, not engine %):
//!  1. Eight retail saves (SaveData01..08) survive load -> save -> load -> save byte for
//!     byte: the second written file equals the first and the engine state (day, clock,
//!     weekday, money, all 800 normal variables, the picture table, play time, choice
//!     anchor and every persistent common-event self bank) is continuous across the reload.
//!  2. A freshly booted game played forward across several days, then saved, reloaded and
//!     saved again, keeps that same state and converges to a fixed-point save: from the
//!     second written file onward a load -> save reproduces the exact bytes.
//!  3. The common-event self banks the M13 fix (commit 7c3b728) persists - the dirty set
//!     plus every still-live common coroutine - are restored intact after the reload.
//!
//! Method note (measured, not assumed): a retail save carries variables and the picture
//! table but NOT the suspended day-loop coroutine. Loading one and feeding frames leaves
//! the clock frozen (probed: SaveData02 stays at day 2 18:30 over 4000 frames even with
//! the engine events started), and re-entering the SampleMapA entry point restarts the
//! new-game intro at day 1. So the eight slots are validated as save/load round trips,
//! and genuine multi-day advancement is proven on the fresh-game path (section 2), which
//! is the only entry point that reconstructs the live daily loop. This is recorded as a
//! finding, not silently papered over.
//!
//! Fixed (Text picture record): the live message-window text picture 16001 stores its box
//! size in vars7_18[2]/[3]. picture_from_record (crates/wolf-game/src/savegame.rs) now
//! carries those words in Picture.window and picture_to_record writes them back instead of
//! substituting the Text split (1,1), so the FIRST mid-scene save is a fixed point. Retail
//! saves never contain picture 16001 (a menu save has no live message box), so section 1
//! was already byte equal; section 2 now asserts the stronger S1 == S2 == S3 fixed point.
//!
//! Run (default, fast):  WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m13_crossday_save
//! Long soak (#[ignore]): WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m13_crossday_save -- --ignored

use std::path::PathBuf;
use std::time::Instant;

use wolf_game::db::Container;
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

fn retail_save_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("WOLF_RETAIL_SAVE_DIR") {
        let dir = PathBuf::from(dir);
        let dir = if dir.is_absolute() {
            dir
        } else {
            repo_root().join(dir)
        };
        return dir.is_dir().then_some(dir);
    }
    let dir = repo_root().join("work/retail_backup/Save");
    dir.is_dir().then_some(dir)
}

fn out_dir(tag: &str) -> PathBuf {
    let dir = repo_root()
        .join("work/out")
        .join(format!("m13_crossday_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

// ---------------------------------------------------------------------------
// Natural daily driver: a compact copy of the proven m12_multiday_soak loop.
// ---------------------------------------------------------------------------

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

fn click_picture(game: &mut Game, number: i32) {
    let p = game.screen.get(number).expect("menu picture").clone();
    game.mouse = [p.x, p.y];
    for _ in 0..10 {
        game.frame();
    }
    game.mouse_buttons = 1;
    for _ in 0..5 {
        game.frame();
    }
    game.mouse_buttons = 0;
    game.mouse = [-1, -1];
}

fn play_day(game: &mut Game, day: i32) -> i32 {
    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 1;
    let mut last: Option<(i32, i32, i32)> = None;
    let mut last_frame = 0usize;
    for frame in 0..200_000usize {
        let mr = rows(game);
        let weekend = mr.iter().any(|t| t.contains("悠然自得")) && has(game, 310);
        let weekday = mr
            .iter()
            .any(|t| DAY_LABELS.iter().any(|l| t.contains(l)) || t.contains("地区贡献"))
            && has(game, 251);
        let key = (
            game.vm.vars.get(4),
            game.vm.vars.get(5),
            if weekend { 2 } else { 1 },
        );
        if last == Some(key) && frame.saturating_sub(last_frame) > 400 {
            last = None;
        }
        let can = last != Some(key);
        if can && weekend {
            let rest = game.vm.vars.get(14) < 30;
            let row = if rest {
                mr.iter().position(|t| t.contains("悠然自得"))
            } else {
                mr.iter()
                    .position(|t| t.contains("自主锻炼"))
                    .or_else(|| mr.iter().position(|t| t.contains("悠然自得")))
            };
            if let Some(r) = row {
                click_picture(game, ROW_PICS[r]);
                last = Some(key);
                last_frame = frame;
                continue;
            }
        }
        if can && weekday {
            let row = mr
                .iter()
                .position(|t| DAY_LABELS.iter().any(|l| t.contains(l)))
                .or_else(|| mr.iter().position(|t| !t.is_empty()));
            if let Some(r) = row {
                let y = 230 + 50 * r as i32;
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
                last = Some(key);
                last_frame = frame;
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
        let cur = game.vm.vars.get(160);
        if cur != day {
            return cur;
        }
        if !evening_work(game) && game.vm.vars.get(4) >= 18 {
            return day;
        }
    }
    panic!(
        "play_day stuck at day {day}: {}:{} warnings={}",
        game.vm.vars.get(4),
        game.vm.vars.get(5),
        game.stats.warnings
    );
}

fn sleep_to_next_morning(game: &mut Game, day: i32) {
    let expected = day + 1;
    game.auto_choice = false;
    game.vm.vars.set(63, 1);
    let mut options = Vec::new();
    for frame in 0..2400 {
        game.key_bits = if frame % 2 == 0 {
            wolf_game::key::OK
        } else {
            0
        };
        game.frame();
        if let Some(c) = &game.choices {
            options = c.options.clone();
            break;
        }
    }
    game.key_bits = 0;
    assert_eq!(
        options,
        SLEEP_OPTIONS
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        "day {day}: sleep menu not reached at {}:{}",
        game.vm.vars.get(4),
        game.vm.vars.get(5)
    );
    game.set_choice_cursor(2);
    decide(game);
    let mut confirm = Vec::new();
    for _ in 0..300 {
        game.frame();
        if let Some(c) = &game.choices {
            confirm = c.options.clone();
            break;
        }
    }
    assert_eq!(
        confirm,
        CONFIRM_OPTIONS
            .iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<_>>(),
        "day {day}: sleep confirm not reached"
    );
    game.set_choice_cursor(0);
    decide(game);
    let mut arrived = false;
    for _ in 0..2400 {
        game.frame();
        if game.vm.vars.get(160) == expected {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "day {expected}: morning not reached");
}

// ---------------------------------------------------------------------------
// State snapshots.
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq, Clone)]
struct PicSnap {
    number: i32,
    kind: String,
    file: String,
    text: String,
    x: i32,
    y: i32,
    opacity: i32,
    scale: i32,
    anchor: u8,
    split: (u32, u32),
    pattern_index: u32,
    window: Option<(i32, i32)>,
}

fn picture_snapshot(game: &Game) -> Vec<PicSnap> {
    game.screen
        .iter()
        .map(|p| PicSnap {
            number: p.number,
            kind: format!("{:?}", p.kind),
            file: p.file.clone(),
            text: p.text.clone(),
            x: p.x,
            y: p.y,
            opacity: p.opacity,
            scale: p.scale,
            anchor: p.anchor,
            split: p.split,
            pattern_index: p.pattern_index,
            window: p.window,
        })
        .collect()
}

// Continuity view of the picture table: the set of picture slots plus the real image
// file backing each File picture. Text / window-box pictures are re-encoded by the
// picture layer on load, so their exact fields are not part of this comparison.
fn pic_continuity(game: &Game) -> Vec<(i32, Option<String>)> {
    game.screen
        .iter()
        .map(|p| {
            let is_file = format!("{:?}", p.kind) == "File";
            (p.number, if is_file { Some(p.file.clone()) } else { None })
        })
        .collect()
}

fn vars800(game: &Game) -> Vec<i32> {
    (0..800).map(|i| game.vm.vars.get(i)).collect()
}

fn written_bank(game: &Game, event: u32) -> Option<Vec<i32>> {
    if let Some(i) = game
        .vm
        .instances()
        .iter()
        .find(|i| i.event == event && i.program.self_base == wolf_vm::program::COMMON_SELF_BASE)
    {
        return Some((0..100).map(|s| i.self_vars.get(600 + s)).collect());
    }
    game.vm
        .common_self(event)
        .map(|b| (0..100).map(|s| b.get(600 + s)).collect())
}

fn persisted_banks(game: &Game) -> Vec<(u32, Vec<i32>)> {
    let mut events: Vec<u32> = game.vm.common_self_dirty().iter().copied().collect();
    for i in game.vm.instances() {
        if i.program.self_base == wolf_vm::program::COMMON_SELF_BASE {
            events.push(i.event);
        }
    }
    events.sort();
    events.dedup();
    let mut out = Vec::new();
    for ev in events {
        if let Some(v) = written_bank(game, ev) {
            out.push((ev, v));
        }
    }
    out
}

fn all_common_banks(game: &Game) -> Vec<(u32, Vec<i32>)> {
    let mut out = Vec::new();
    for ev in 0..=550u32 {
        if let Some(b) = game.vm.common_self(ev) {
            out.push((ev, (0..100).map(|s| b.get(600 + s)).collect()));
        }
    }
    out
}

fn assert_banks_preserved(expected: &[(u32, Vec<i32>)], reloaded: &Game) -> (usize, usize) {
    let mut nonzero = 0usize;
    for (ev, want) in expected {
        if want.iter().any(|v| *v != 0) {
            nonzero += 1;
        }
        match reloaded.vm.common_self(*ev) {
            Some(b) => {
                let got: Vec<i32> = (0..100).map(|s| b.get(600 + s)).collect();
                assert_eq!(
                    &got, want,
                    "event {ev}: persistent self bank changed across save/reload"
                );
            }
            None => assert!(
                want.iter().all(|v| *v == 0),
                "event {ev}: non-zero bank lost across reload"
            ),
        }
    }
    (expected.len(), nonzero)
}

fn chunk_names() -> [&'static str; 7] {
    [
        "SavePart1",
        "SavePart2",
        "SavePart3",
        "SavePart4",
        "SavePart5",
        "VariableDatabase",
        "SavePart7",
    ]
}

// ---------------------------------------------------------------------------
// Section 1: the eight retail slots, byte-for-byte idempotent + continuous.
// ---------------------------------------------------------------------------

#[test]
fn eight_retail_slots_round_trip_idempotent_and_continuous() {
    let Some(dir) = retail_save_dir() else {
        eprintln!("skipping: no retail save directory (set WOLF_RETAIL_SAVE_DIR)");
        return;
    };
    let root = data_root();
    let out = out_dir("slots");
    let mut checked = 0usize;
    let mut lines: Vec<String> = Vec::new();
    for slot in 1..=8u32 {
        let name = format!("SaveData{slot:02}.sav");
        let source = dir.join(&name);
        let original = std::fs::read(&source).unwrap_or_else(|e| panic!("read {name}: {e}"));

        let mut g = Game::new(root.clone()).expect("game");
        g.load_save(&source)
            .unwrap_or_else(|e| panic!("load {name}: {e}"));
        let a_day = g.vm.vars.get(160);
        let a_clock = (g.vm.vars.get(4), g.vm.vars.get(5));
        let a_wd = g.vm.vars.get(6);
        let a_money = g.vm.vars.get(7);
        let a_vars = vars800(&g);
        let a_pics = picture_snapshot(&g);
        let a_playtime = g.playtime_frames;
        let a_anchor = (g.vm.vars.get_sys(3), g.vm.vars.get_sys(4));
        let a_banks = all_common_banks(&g);

        let pa = out.join(format!("A_{name}"));
        g.save_to(&pa)
            .unwrap_or_else(|e| panic!("save A {name}: {e}"));
        let bytes_a = std::fs::read(&pa).expect("read A");
        assert_eq!(bytes_a, original, "{name}: load -> save must reproduce the official bytes (idempotent at the first write)");

        let mut g2 = Game::new(root.clone()).expect("game2");
        g2.load_save(&pa)
            .unwrap_or_else(|e| panic!("reload {name}: {e}"));
        assert_eq!(
            vars800(&g2),
            a_vars,
            "{name}: 800 normal variables changed across the reload"
        );
        assert_eq!(
            g2.vm.vars.get(160),
            a_day,
            "{name}: day changed across the reload"
        );
        assert_eq!(
            (g2.vm.vars.get(4), g2.vm.vars.get(5)),
            a_clock,
            "{name}: clock changed across the reload"
        );
        assert_eq!(
            g2.vm.vars.get(6),
            a_wd,
            "{name}: weekday changed across the reload"
        );
        assert_eq!(
            g2.vm.vars.get(7),
            a_money,
            "{name}: money changed across the reload"
        );
        assert_eq!(
            picture_snapshot(&g2),
            a_pics,
            "{name}: picture table changed across the reload"
        );
        assert_eq!(
            g2.playtime_frames, a_playtime,
            "{name}: play time changed across the reload"
        );
        assert_eq!(
            (g2.vm.vars.get_sys(3), g2.vm.vars.get_sys(4)),
            a_anchor,
            "{name}: choice anchor changed across the reload"
        );
        assert_eq!(
            all_common_banks(&g2),
            a_banks,
            "{name}: common-event self banks changed across the reload"
        );

        let pb = out.join(format!("B_{name}"));
        g2.save_to(&pb)
            .unwrap_or_else(|e| panic!("save B {name}: {e}"));
        let bytes_b = std::fs::read(&pb).expect("read B");
        assert_eq!(
            bytes_b, bytes_a,
            "{name}: second load -> save must equal the first written bytes"
        );

        let parsed = wolf_save::SaveFile::parse(&bytes_b).expect("parse B");
        let names: Vec<&str> = parsed.chunks().iter().map(|c| c.name).collect();
        assert_eq!(names, chunk_names(), "{name}: every segment present");

        lines.push(format!(
            "{name}: PASS day={a_day} clock={:02}:{:02} wd={a_wd} money={a_money} pics={} playtime={} bytes={}",
            a_clock.0, a_clock.1, a_pics.len(), a_playtime, bytes_b.len()
        ));
        checked += 1;
    }
    for l in &lines {
        eprintln!("{l}");
    }
    let _ = std::fs::remove_dir_all(&out);
    assert_eq!(checked, 8, "all eight retail slots were validated");
}

// ---------------------------------------------------------------------------
// Section 2: fresh boot, play forward across days, save/reload/save convergence.
// ---------------------------------------------------------------------------

struct CrossDay {
    arrival_day: i32,
    pics: usize,
    nonzero_banks: usize,
}

fn boot_fresh() -> Game {
    let mut g = Game::new(data_root()).expect("game");
    g.auto_advance = true;
    g.auto_choice = true;
    g.auto_choice_delay = 2;
    g.set_choice_cursor(0);
    g.start_map_event_by_name("SampleMapA", 0)
        .expect("new-game entry event");
    for _ in 0..1600 {
        g.frame();
    }
    assert_eq!(g.vm.vars.get(160), 1, "boot must land on day 1");
    assert!(g.seed_battle_databases() > 0, "battle databases seeded");
    g.databases
        .set_text(Container::CData, 5, 16, 0, "┗ 状態リスト");
    let _ = g.start_engine_events();
    g
}

fn run_cross_day(target: i32, tag: &str) -> CrossDay {
    let out = out_dir(tag);
    let t0 = Instant::now();
    let mut g = boot_fresh();
    sleep_to_next_morning(&mut g, 1);
    let mut day = 2;
    while day < target {
        let ended = play_day(&mut g, day);
        if ended != day {
            eprintln!("  scripted day jump {day} -> {ended}");
            day = ended;
            continue;
        }
        sleep_to_next_morning(&mut g, day);
        day += 1;
    }
    assert_eq!(g.stats.warnings, 0, "cross-day play must be warning free");

    let arrival_day = g.vm.vars.get(160);
    let clock = (g.vm.vars.get(4), g.vm.vars.get(5));
    let weekday = g.vm.vars.get(6);
    let money = g.vm.vars.get(7);
    let vars = vars800(&g);
    let pics_cont = pic_continuity(&g);
    let pic_count = g.screen.iter().count();
    let banks_before = persisted_banks(&g);

    let s1 = out.join("S1.sav");
    g.save_to(&s1).expect("save S1");
    let b1 = std::fs::read(&s1).expect("read S1");

    let mut g2 = Game::new(data_root()).expect("game2");
    g2.load_save(&s1).expect("reload S1");
    assert_eq!(
        g2.vm.vars.get(160),
        arrival_day,
        "day changed across reload"
    );
    assert_eq!(
        (g2.vm.vars.get(4), g2.vm.vars.get(5)),
        clock,
        "clock changed across reload"
    );
    assert_eq!(g2.vm.vars.get(6), weekday, "weekday changed across reload");
    assert_eq!(g2.vm.vars.get(7), money, "money changed across reload");
    assert_eq!(
        vars800(&g2),
        vars,
        "800 normal variables changed across reload"
    );
    // Picture continuity: the picture slots and every real image file survive the reload.
    // Text message-box dimensions are now preserved too; the byte fixed point below
    // (S1 == S2) proves the whole picture layer round-trips without a second-cycle repair.
    assert_eq!(
        pic_continuity(&g2),
        pics_cont,
        "picture slots / image files changed across reload"
    );
    assert_eq!(
        g2.screen.iter().count(),
        pic_count,
        "picture count changed across reload"
    );
    let (banks_n, nonzero) = assert_banks_preserved(&banks_before, &g2);

    let s2 = out.join("S2.sav");
    g2.save_to(&s2).expect("save S2");
    let b2 = std::fs::read(&s2).expect("read S2");
    let mut g3 = Game::new(data_root()).expect("game3");
    g3.load_save(&s2).expect("reload S2");
    let s3 = out.join("S3.sav");
    g3.save_to(&s3).expect("save S3");
    let b3 = std::fs::read(&s3).expect("read S3");
    assert_eq!(
        b2, b1,
        "the FIRST mid-scene save must already be a fixed point (S1 == S2)"
    );
    assert_eq!(
        b3, b2,
        "the reload -> save fixed point must be stable (S2 == S3)"
    );

    let p1 = wolf_save::SaveFile::parse(&b1).expect("parse S1");
    let p2 = wolf_save::SaveFile::parse(&b2).expect("parse S2");
    for id in 0..=6u32 {
        assert_eq!(
            p1.chunk_bytes(id),
            p2.chunk_bytes(id),
            "segment {id} changed between the first and second save"
        );
    }
    let mut differ: Vec<i32> = Vec::new();
    for (r1, r2) in p1.part5.events.iter().zip(p2.part5.events.iter()) {
        let same = r1.var1 == r2.var1
            && r1.var4 == r2.var4
            && r1.var5 == r2.var5
            && r1.var6 == r2.var6
            && r1.md1 == r2.md1
            && r1.vars7_18 == r2.vars7_18;
        if !same {
            differ.push(r1.var1 as i32);
        }
    }
    assert!(differ.is_empty(), "the first mid-scene save must be a fixed point; pictures changed on reload -> save: {differ:?}");

    eprintln!(
        "{tag}: arrived day {arrival_day} at {:02}:{:02} wd={weekday} money={money} pics={pic_count} banks_preserved={banks_n} (nonzero {nonzero}) S1->S2 picture delta={differ:?} S2==S3=yes bytes={} elapsed={:?}",
        clock.0, clock.1, b1.len(), t0.elapsed()
    );
    let _ = std::fs::remove_dir_all(&out);
    CrossDay {
        arrival_day,
        pics: pic_count,
        nonzero_banks: nonzero,
    }
}

#[test]
fn cross_day_play_save_reload_keeps_state_and_converges() {
    if Game::new(data_root()).is_err() {
        eprintln!("skipping: data root not available");
        return;
    }
    let s = run_cross_day(4, "short");
    assert!(
        s.arrival_day >= 4,
        "advanced to at least day 4, got {}",
        s.arrival_day
    );
    assert!(
        s.nonzero_banks > 0,
        "at least one non-zero common-event self bank must be exercised and preserved, got {}",
        s.nonzero_banks
    );
    assert!(s.pics > 0, "the picture table round-trips");
}

#[test]
#[ignore = "long soak: ~10 s, run with -- --ignored"]
fn ten_days_then_save_reload_converges() {
    if Game::new(data_root()).is_err() {
        eprintln!("skipping: data root not available");
        return;
    }
    let s = run_cross_day(11, "long");
    assert!(
        s.arrival_day >= 11,
        "advanced to at least day 11, got {}",
        s.arrival_day
    );
    assert!(s.nonzero_banks > 0, "non-zero self banks preserved");
}
