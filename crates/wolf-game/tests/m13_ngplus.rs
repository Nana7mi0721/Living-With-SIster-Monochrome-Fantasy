//! M13 P1-D: NG+ (new game plus / 二周目) inheritance probe.
//!
//! The retail game has no separate "new game plus" menu entry. Instead:
//!
//!   boot -> TitleMap.mps event 2 (op221 loads a batch of normal vars from
//!            save/system.sav, including var550 = 引き継ぎポイント / inheritance points)
//!        -> CE42 title menu -> はじめから -> Map003 ev1 -> SampleMapA ev0
//!        -> SampleMapA ev0 (autorun) pc347: if var550 >= 2, show the inheritance menu,
//!           allocate points into var552..557, then pc498..509 add them onto the
//!           new run's attack/defence/stamina/skill/money/companion-LV vars.
//!
//! CE436 (引き継ぎPチェック) recomputes the point total from achievements and, once
//! アカツキホモイベ (var531) is far enough along, persists it into Save/System.sav.
//!
//! Everything here is data driven: the engine runs the shipped scripts through the
//! generic interpreter (op111/121/102/401/220/221), so the tests assert the shipped
//! data and the observable engine behaviour rather than hardcoded game logic.
//!
//! The first three tests are fast and only parse data / read a save; they skip (do
//! not fail) when the data root is missing, exactly like the rest of the suite. The
//! `#[ignore]` tests drive the interpreter and are a longer matrix.

use std::path::PathBuf;

use wolf_format::command::{Command, Slot};
use wolf_format::commonevent::CommonEventData;
use wolf_format::mps::Map;
use wolf_game::Game;

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/full".to_owned());
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return path;
    }
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base.pop();
    base.pop();
    base.join(path)
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

/// Decode a command's raw parameter bytes as little-endian i32 words. This mirrors
/// how the VM decodes opcode operands; the tests only need to read them, not run them.
fn words(cmd: &Command) -> Vec<i32> {
    cmd.params
        .chunks_exact(4)
        .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

fn command_refs(slots: &[Slot]) -> Vec<&Command> {
    slots
        .iter()
        .filter_map(|slot| match slot {
            Slot::Command(cmd) => Some(cmd.as_ref()),
            _ => None,
        })
        .collect()
}

fn find_first(root: &PathBuf, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.exists())
}

// ---------------------------------------------------------------------------
// Fast tests: shipped-data invariants. No interpreter needed.
// ---------------------------------------------------------------------------

/// CE436 recomputes the inheritance-point total from the achievement vars and writes
/// it back into System.sav. This pins the shipped script's shape: the achievements it
/// reads, the two save paths it touches, and the single-variable read of var550.
#[test]
fn ce436_recomputes_and_persists_ngplus_points() {
    let root = data_root();
    let Ok(bytes) = std::fs::read(root.join("BasicData").join("CommonEvent.dat")) else {
        eprintln!("skipping: BasicData/CommonEvent.dat missing under {}", root.display());
        return;
    };
    let ce = CommonEventData::parse(&bytes).expect("CommonEvent.dat parses");
    let ev = ce.event(436).expect("CE436 exists");
    assert!(
        ev.title.contains("引き継ぎ"),
        "CE436 should be the inheritance-point check, got {:?}",
        ev.title
    );
    // The dump-json view lists 68 "interesting" commands; the shipped event has 109
    // command slots. Assert a stable lower bound rather than a brittle exact count.
    assert!(
        ev.commands.len() >= 100,
        "CE436 should be a large script, got {} slots",
        ev.commands.len()
    );

    let cmds = command_refs(&ev.commands);

    // The three save paths the event touches (it back-ups System.sav to system2.sav,
    // reads the old total from system.sav, then writes the new total to System.sav).
    let mut strings: Vec<String> = Vec::new();
    for cmd in &cmds {
        strings.extend(cmd.strings.iter().cloned());
    }
    for want in ["save/system2.sav", "save/system.sav", "Save/System.sav"] {
        assert!(
            strings.iter().any(|s| s.eq_ignore_ascii_case(want)),
            "CE436 is missing the save path {want:?}; saw {strings:?}"
        );
    }

    // op221 [2000550, <path>, 2000550, 0]: read the OLD var550 out of System.sav.
    let reads_550 = cmds.iter().any(|cmd| {
        cmd.opcode == 221 && {
            let w = words(cmd);
            w.len() >= 3 && w[0] == 2_000_550 && w[2] == 2_000_550
        }
    });
    assert!(reads_550, "CE436 must read var550 (2_000_550) back via op221");

    // Two op220 saves: the system2.sav back-up and the System.sav write-back.
    let saves = cmds.iter().filter(|cmd| cmd.opcode == 220).count();
    assert!(saves >= 2, "CE436 should op220-save twice, found {saves}");

    // The point total is the sum of achievement checks; pin the vars it tests.
    let tested: Vec<i32> = cmds
        .iter()
        .filter(|cmd| cmd.opcode == 111)
        .filter_map(|cmd| words(cmd).get(1).copied())
        .collect();
    for var in [
        2_000_040, 2_000_042, 2_000_044, 2_000_053, 2_000_534, 2_000_519, 2_000_367,
        2_000_490, 2_000_492, 2_000_503, 2_000_470, 2_000_474, 2_000_476,
    ] {
        assert!(
            tested.contains(&var),
            "CE436 should test achievement var {var}; tested vars = {tested:?}"
        );
    }
}

/// SampleMapA event 0 is the autorun new-game event, and it carries the whole
/// inheritance block: the var550>=2 gate, the 9-option allocation menu, the per-pick
/// increments, and the settlement table that folds var552..557 onto the new run.
#[test]
fn sample_map_a_event0_is_autorun_and_has_the_inheritance_table() {
    let root = data_root();
    let Ok(bytes) = std::fs::read(root.join("MapData").join("SampleMapA.mps")) else {
        eprintln!("skipping: MapData/SampleMapA.mps missing under {}", root.display());
        return;
    };
    let map = Map::parse(&bytes).expect("SampleMapA parses");
    let ev0 = map
        .events
        .iter()
        .find(|event| event.id == 0)
        .expect("SampleMapA has event 0");
    let page = ev0.pages.first().expect("event 0 has page 0");
    assert_eq!(
        page.trigger, 1,
        "SampleMapA ev0 must be autorun (trigger 1) so a bare 130 teleport starts it"
    );

    let cmds = command_refs(&page.commands);

    // Gate: op111 [1, 2000550, 2, 1] == (var550 >= 2).
    let gate = cmds.iter().any(|cmd| {
        cmd.opcode == 111 && {
            let w = words(cmd);
            w.len() >= 4 && w[0] == 1 && w[1] == 2_000_550 && w[2] == 2 && w[3] == 1
        }
    });
    assert!(gate, "the var550 >= 2 inheritance gate is missing");

    // The allocation menu has nine options (six keepsakes + 強韧的肉体 + 重置 + 不继承).
    let menu = cmds
        .iter()
        .any(|cmd| cmd.opcode == 102 && cmd.strings.len() == 9);
    assert!(menu, "the 9-option inheritance menu is missing");

    // Per-pick allocation: op121 [dest, +amount, _, 256].
    for (var, amount) in [
        (2_000_552, 30i32),
        (2_000_553, 40),
        (2_000_554, 50),
        (2_000_555, 50),
        (2_000_556, 10_000),
        (2_000_557, 8),
    ] {
        let ok = cmds.iter().any(|cmd| {
            cmd.opcode == 121 && {
                let w = words(cmd);
                w.len() >= 4 && w[0] == var && w[1] == amount && w[3] == 256
            }
        });
        assert!(ok, "missing allocation var{var} += {amount}");
    }

    // Settlement: op121 [dst, src, _, 256] folds the keepsakes onto the new run.
    for (dst, src) in [
        (2_000_170, 2_000_552), // attack
        (2_000_173, 2_000_552), // defence
        (2_000_174, 2_000_552), // guts
        (2_000_172, 2_000_552), // technique
        (2_000_171, 2_000_553), // wisdom
        (2_000_010, 2_000_553), // skill points
        (2_000_021, 2_000_554), // trust
        (2_000_022, 2_000_555), // interest
        (2_000_007, 2_000_556), // money
        (2_000_041, 2_000_557), // kana LV
        (2_000_043, 2_000_557), // yui LV
        (2_000_045, 2_000_557), // kasumi LV
    ] {
        let ok = cmds.iter().any(|cmd| {
            cmd.opcode == 121 && {
                let w = words(cmd);
                w.len() >= 4 && w[0] == dst && w[1] == src && w[3] == 256
            }
        });
        assert!(ok, "missing settlement var{dst} += var{src}");
    }
}

/// The shipped save fixtures really do carry 引き継ぎポイント (var550 >= 2), so NG+ is
/// genuinely reachable from retail data and not only from a synthesised state.
#[test]
fn retail_system_sav_carries_ngplus_points() {
    let root = data_root();
    let Some(path) = find_first(
        &root,
        &["Save/System.sav", "Save/system.sav", "System.sav", "system.sav"],
    ) else {
        eprintln!("skipping: no System.sav under {}", root.display());
        return;
    };
    let Ok(data) = std::fs::read(&path) else {
        eprintln!("skipping: cannot read {}", path.display());
        return;
    };
    let Ok(save) = wolf_save::SaveFile::parse(&data) else {
        eprintln!("skipping: {} is not a parseable save", path.display());
        return;
    };
    let points = save.normal_var(550).unwrap_or(0);
    assert!(
        points >= 2,
        "{} should carry NG+ points (var550) but has {points}",
        path.display()
    );
    assert_eq!(
        save.normal_var(551).unwrap_or(0),
        0,
        "var551 (使用ポイント) is a transient counter, not stored in System.sav"
    );
}

// ---------------------------------------------------------------------------
// Longer matrix: drive the interpreter.
// ---------------------------------------------------------------------------

/// Boot the retail title chain and confirm the engine loads var550 out of
/// Save/System.sav into the VM before the title menu opens. This is the entry half of
/// NG+: without it the inheritance block can never trigger.
#[test]
#[ignore = "boots the full retail title chain (slower); run with --ignored"]
fn boot_loads_ngplus_points_from_system_sav() {
    let root = data_root();
    let Some(path) = find_first(
        &root,
        &["Save/System.sav", "Save/system.sav", "System.sav", "system.sav"],
    ) else {
        eprintln!("skipping: no System.sav under {}", root.display());
        return;
    };
    let Ok(data) = std::fs::read(&path) else {
        eprintln!("skipping: cannot read {}", path.display());
        return;
    };
    let Ok(save) = wolf_save::SaveFile::parse(&data) else {
        eprintln!("skipping: {} is not a parseable save", path.display());
        return;
    };
    let expected = save.normal_var(550).unwrap_or(0);
    let Some(mut game) = open_game() else {
        return;
    };
    game.boot().expect("the boot map has its autorun event");
    // TitleMap ev2 issues the op221 reads early; a few hundred frames is plenty and
    // also lets the splash art settle.
    for _ in 0..400 {
        game.frame();
    }
    let loaded = game.vm.vars.get(550);
    assert_eq!(
        loaded, expected,
        "TitleMap ev2 should load var550 from System.sav into the VM"
    );
    assert!(
        loaded >= 2,
        "the fixture must actually have NG+ points, got var550 = {loaded}"
    );
}

/// Drive SampleMapA event 0 from a seeded point pool and watch the inheritance menu
/// allocate 強大 (cursor 0 -> var552 += 30) and the settlement fold it onto var170.
/// This is the apply half of NG+.
#[test]
#[ignore = "runs the whole SampleMapA new-game opening to the inheritance menu"]
fn new_game_inheritance_menu_applies_points() {
    let Some(mut game) = open_game() else {
        return;
    };
    // Seed the point pool as if a previous play-through had persisted var550 = 2,
    // which is exactly the shipped work/full fixture value (var551 = 2 / 2 = 1 pick).
    game.vm.vars.set(550, 2);
    let before_attack = game.vm.vars.get(170);
    game.start_map_event_by_name("SampleMapA", 0)
        .expect("SampleMapA event 0 starts");

    game.auto_advance = true;
    game.auto_choice = true;
    game.auto_choice_delay = 3;

    let mut seen_menu: Option<Vec<String>> = None;
    let mut settled = false;
    for _ in 0..12_000 {
        game.frame();
        if let Some(choices) = game.choices.as_ref() {
            if choices.options.len() == 9 && seen_menu.is_none() {
                seen_menu = Some(choices.options.clone());
            }
        }
        // Auto-choice answers cursor 0 = 強大, so var552 becomes 30 and the settlement
        // adds it onto var170; once both have happened the allocation is proven.
        if seen_menu.is_some()
            && game.vm.vars.get(552) >= 30
            && game.vm.vars.get(170) >= before_attack + 30
        {
            settled = true;
            break;
        }
    }

    let menu = seen_menu.expect("never reached the 9-option inheritance menu");
    assert_eq!(menu.len(), 9, "the inheritance menu must have nine options");
    assert!(
        game.vm.vars.get(552) >= 30,
        "強大 allocation should add 30 to var552, got {}",
        game.vm.vars.get(552)
    );
    assert!(
        settled,
        "settlement did not fold var552 into var170 (var170 = {}, before = {before_attack})",
        game.vm.vars.get(170)
    );
}
