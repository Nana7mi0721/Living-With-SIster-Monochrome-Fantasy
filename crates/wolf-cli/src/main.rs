//! `wolf-cli` — the project's diagnostic front end.
//!
//! The only command that matters during M0 is `verify`: it loads every game
//! data file, prints the counts, and compares them against the frozen baseline
//! in `docs/BASELINE.json`. Those numbers are a regression ratchet — if a
//! parser change moves one of them, the change is a bug until proven otherwise.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use wolf_format::commonevent::CommonEventData;
use wolf_format::gamedat::GameDat;

/// One asserted number.
struct Check {
    label: &'static str,
    expected: String,
    actual: String,
    ok: bool,
    /// True when the loader for this section does not exist yet.
    skipped: bool,
}

impl Check {
    fn text(label: &'static str, expected: &str, actual: &str) -> Self {
        Self {
            label,
            expected: expected.to_owned(),
            actual: actual.to_owned(),
            ok: expected == actual,
            skipped: false,
        }
    }

    fn num(label: &'static str, expected: u64, actual: u64) -> Self {
        Self {
            label,
            expected: expected.to_string(),
            actual: actual.to_string(),
            ok: expected == actual,
            skipped: false,
        }
    }

    fn skip(label: &'static str) -> Self {
        Self {
            label,
            expected: "-".to_owned(),
            actual: "not implemented".to_owned(),
            ok: true,
            skipped: true,
        }
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut root: Option<PathBuf> = None;
    let mut command: Option<String> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" | "-r" => {
                root = args.next().map(PathBuf::from);
            }
            "--help" | "-h" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            other if other.starts_with('-') => {
                eprintln!("unknown flag: {other}");
                print_help();
                return ExitCode::from(2);
            }
            other => command = Some(other.to_owned()),
        }
    }

    match command.as_deref() {
        Some("verify") => {}
        Some(other) => {
            eprintln!("unknown command: {other}");
            print_help();
            return ExitCode::from(2);
        }
        None => {
            print_help();
            return ExitCode::from(2);
        }
    }

    let root = root.unwrap_or_else(default_root);
    run_verify(&root)
}

fn print_help() {
    println!("wolf-cli — WOLF RPG Editor data diagnostics");
    println!();
    println!("USAGE:");
    println!("    wolf-cli [--root <DIR>] verify");
    println!();
    println!("OPTIONS:");
    println!("    -r, --root <DIR>  Unpacked data root containing BasicData/ and MapData/.");
    println!("                      Defaults to ./work/full, then $WOLF_DATA_ROOT.");
    println!("    -h, --help        Print this help.");
}

fn default_root() -> PathBuf {
    if let Ok(value) = std::env::var("WOLF_DATA_ROOT") {
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    PathBuf::from("work/full")
}

fn run_verify(root: &Path) -> ExitCode {
    println!("wolf-cli verify");
    println!("  data root : {}", root.display());

    let basic = if root.join("BasicData").is_dir() {
        root.join("BasicData")
    } else {
        root.to_path_buf()
    };
    let map_dir = root.join("MapData");
    println!("  basic dir : {}", basic.display());
    println!("  map dir   : {}", map_dir.display());
    println!();

    let mut checks: Vec<Check> = Vec::new();

    // --- Game.dat ---------------------------------------------------------
    match read(&basic.join("Game.dat")).map(|bytes| GameDat::parse(&bytes)) {
        Some(Ok(game)) => {
            checks.push(Check::text("game.title", "妹相随～黑白世界的缤纷冒险～", game.title()));
            checks.push(Check::text("game.subtitle", "+DLC v2.03", game.subtitle()));
            checks.push(Check::num("game.screen.w", 1024, u64::from(game.screen_width())));
            checks.push(Check::num("game.screen.h", 576, u64::from(game.screen_height())));
            checks.push(Check::num("game.tile_size", 16, u64::from(game.tile_size())));
            checks.push(Check::num("game.fps", 60, u64::from(game.fps())));
            checks.push(Check::num("game.strings", 13, game.strings.len() as u64));
            checks.push(Check::num("game.u8_settings", 35, game.u8_settings.len() as u64));
            checks.push(Check::num("game.u16_settings", 22, game.u16_settings.len() as u64));
            checks.push(Check::text("game.font_main", "LOLITA", game.font_main()));
            checks.push(Check::num(
                "game.random_a",
                22227,
                u64::from(game.random_a),
            ));
            checks.push(Check::num(
                "game.random_b",
                25684,
                u64::from(game.random_b),
            ));
            checks.push(Check::num("game.footer", 0xC3, u64::from(game.footer)));
        }
        Some(Err(err)) => {
            checks.push(Check::text("game.parse", "ok", &format!("FAILED: {err}")));
        }
        None => checks.push(Check::text("game.parse", "ok", "FILE MISSING")),
    }

    // --- CommonEvent.dat --------------------------------------------------
    match read(&basic.join("CommonEvent.dat")).map(|bytes| CommonEventData::parse(&bytes)) {
        Some(Ok(events)) => {
            let stats = events.stats();
            checks.push(Check::num("ce.events", 550, events.events.len() as u64));
            checks.push(Check::num("ce.version", 0xC9, u64::from(events.version)));
            checks.push(Check::num("ce.footer", 0x90, u64::from(events.footer)));
            checks.push(Check::num("ce.slots", 163_828, stats.slots as u64));
            checks.push(Check::num("ce.blank", 24_555, stats.blank() as u64));
            checks.push(Check::num("ce.real", 139_273, stats.real() as u64));
            checks.push(Check::num(
                "ce.distinct_opcodes",
                47,
                stats.distinct_opcodes() as u64,
            ));
            checks.push(Check::num(
                "ce.max_branch_depth",
                91,
                u64::from(stats.max_branch_depth),
            ));
            checks.push(Check::num(
                "ce.move_route_tails",
                5,
                stats.move_route_tails as u64,
            ));
            checks.push(Check::num(
                "ce.param_layout_mismatches",
                0,
                stats.param_layout_mismatches as u64,
            ));
            let hist = events.run_condition_histogram();
            for (condition, expected) in [(0u8, 442u64), (1, 1), (2, 97), (3, 10)] {
                let actual = hist.get(&condition).copied().unwrap_or(0) as u64;
                checks.push(Check::num(
                    run_condition_label(condition),
                    expected,
                    actual,
                ));
            }
            // The two heaviest opcodes are the cheapest sanity signal that the
            // stream stayed in sync all the way through.
            checks.push(Check::num(
                "ce.opcode.210",
                18_468,
                stats.opcodes.get(&210).copied().unwrap_or(0) as u64,
            ));
            checks.push(Check::num(
                "ce.opcode.401",
                18_555,
                stats.opcodes.get(&401).copied().unwrap_or(0) as u64,
            ));
        }
        Some(Err(err)) => {
            checks.push(Check::text("ce.parse", "ok", &format!("FAILED: {err}")));
        }
        None => checks.push(Check::text("ce.parse", "ok", "FILE MISSING")),
    }

    // --- sections whose loaders land next --------------------------------
    checks.push(Check::skip("mps.events"));
    checks.push(Check::skip("tileset.count"));
    checks.push(Check::skip("database.types"));
    checks.push(Check::skip("dxarchive.header"));
    checks.push(Check::skip("save.header"));

    report(&checks)
}

fn run_condition_label(condition: u8) -> &'static str {
    match condition {
        0 => "ce.run_condition.0",
        1 => "ce.run_condition.1",
        2 => "ce.run_condition.2",
        _ => "ce.run_condition.3",
    }
}

fn read(path: &Path) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(err) => {
            eprintln!("  ! cannot read {}: {err}", path.display());
            None
        }
    }
}

fn report(checks: &[Check]) -> ExitCode {
    let width = checks.iter().map(|c| c.label.len()).max().unwrap_or(0);
    let mut failures = 0usize;
    let mut skipped = 0usize;
    for check in checks {
        let mark = if check.skipped {
            skipped += 1;
            "SKIP"
        } else if check.ok {
            "  ok"
        } else {
            failures += 1;
            "FAIL"
        };
        if check.skipped {
            println!("[{mark}] {:<width$}  (loader pending)", check.label, width = width);
        } else if check.ok {
            println!(
                "[{mark}] {:<width$}  {}",
                check.label,
                check.actual,
                width = width
            );
        } else {
            println!(
                "[{mark}] {:<width$}  expected {}, got {}",
                check.label,
                check.expected,
                check.actual,
                width = width
            );
        }
    }
    println!();
    if failures == 0 {
        println!(
            "PASS — {} check(s) matched baseline, {} pending",
            checks.len() - skipped,
            skipped
        );
        ExitCode::SUCCESS
    } else {
        println!("FAIL — {failures} check(s) disagree with the baseline");
        ExitCode::FAILURE
    }
}
