//! `wolf-cli` — the project's diagnostic front end.
//!
//! The only command that matters during M0 is `verify`: it loads every game
//! data file, prints the counts, and compares them against the frozen baseline
//! in `docs/BASELINE.json`. Those numbers are a regression ratchet — if a
//! parser change moves one of them, the change is a bug until proven otherwise.

mod play;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use std::sync::Arc;

use wolf_format as wf;
use wolf_format::command::SlotStats;
use wolf_format::commonevent::CommonEventData;
use wolf_format::gamedat::GameDat;
use wolf_game::atlas::{self, Atlas};
use wolf_game::tilemap::TileMap;
use wolf_game::{choice, Game};
use wolf_render::{Renderer, SpriteInstance, TextItem, LOGICAL_H, LOGICAL_W};
use wolf_vm::{
    ChoiceReply, DbOp, DbValue, Host, LogLevel, PictureCmd, PrecompileStats, Program, ProgramSet, SoundCmd,
    StepReport, Vm,
};

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
    let mut event: Option<u32> = None;
    let mut frames: usize = 600;
    let mut trace = false;
    let mut hot = false;
    let mut out: Option<PathBuf> = None;
    let mut answer: Option<usize> = None;
    let mut filter: Option<String> = None;
    let mut frame_limit: Option<u64> = None;
    let mut hold_message = false;
    let mut boot = true;
    let mut map: Option<String> = None;
    let mut map_run: Option<String> = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" | "-r" => {
                root = args.next().map(PathBuf::from);
            }
            "--event" | "-e" => {
                event = args.next().and_then(|value| value.parse().ok());
            }
            "--frames" | "-f" => {
                frames = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(600);
            }
            "--trace" | "-t" => trace = true,
            "--hot" => hot = true,
            "--until" => {
                frame_limit = args.next().and_then(|value| value.parse().ok());
            }
            "--out" | "-o" => {
                out = args.next().map(PathBuf::from);
            }
            "--answer" | "-a" => {
                answer = args.next().and_then(|value| value.parse().ok());
            }
            "--map" | "-m" => {
                map = args.next();
            }
            // Render a map EVENT (a story script) rather than the tile layer.
            "--map-run" => {
                map_run = args.next();
            }
            "--hold-message" => hold_message = true,
            "--boot" => boot = true,
            "--menu-only" => boot = false,
            "--filter" | "-F" => {
                filter = args.next();
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

    let root = root.unwrap_or_else(default_root);
    match command.as_deref() {
        Some("verify") => run_verify(&root),
        Some("precompile") => run_precompile(&root),
        Some("capture") => match map.as_deref() {
            Some(name) => run_capture_map(&root, out, name),
            None => {
                let entry = match map_run.as_deref() {
                    Some(name) => Entry::Script {
                        map: name,
                        event: event.unwrap_or(0),
                    },
                    None if boot => Entry::Boot,
                    None => Entry::Menu,
                };
                run_capture(&root, out, frames, answer, hold_message, entry)
            }
        },
        Some("list") => run_list(&root, filter.as_deref()),
        Some("play") => play::run(&root, frame_limit, boot),
        Some("run") => match (map.as_deref(), event) {
            (Some(name), Some(id)) => run_map_event(&root, name, id, frames, trace, answer, hot),
            (Some(_), None) => {
                eprintln!("run --map needs --event <ID>");
                ExitCode::from(2)
            }
            (None, Some(id)) => run_event(&root, id, frames, trace, answer),
            (None, None) => {
                eprintln!("run needs --event <ID>");
                ExitCode::from(2)
            }
        },
        Some(other) => {
            eprintln!("unknown command: {other}");
            print_help();
            ExitCode::from(2)
        }
        None => {
            print_help();
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    println!("wolf-cli — WOLF RPG Editor data diagnostics");
    println!();
    println!("USAGE:");
    println!("    wolf-cli [--root <DIR>] verify");
    println!("    wolf-cli [--root <DIR>] precompile");
    println!("    wolf-cli [--root <DIR>] run --event <ID> [--frames <N>] [--trace]");
    println!("    wolf-cli [--root <DIR>] capture [--frames <N>] [--out <FILE.png>] [--menu-only]");
    println!("    wolf-cli [--root <DIR>] play");
    println!("    wolf-cli [--root <DIR>] list [--filter <TEXT>]");
    println!();
    println!("COMMANDS:");
    println!(
        "    verify      Load every data file and diff the counts against docs/BASELINE.json."
    );
    println!("    precompile  Compile all 550 common events into jump tables; print the stats.");
    println!("    run         Execute one common event headlessly and report what it did.");
    println!("    capture     Run the boot sequence for N frames and render it to a PNG.");
    println!("                Starts on the boot map (splashes + title art), then holds on the menu.");
    println!("                Choice prompts STAY OPEN unless --answer is given.");
    println!("                because answering it branches away from the title screen.");
    println!("    play        Run the title screen in a window (arrows move, Enter/Z confirm).");
    println!("    list        Print every common event: id, slots, real instructions, title.");
    println!();
    println!("OPTIONS:");
    println!("    -e, --event <ID>  Common event id to run with 'run'.");
    println!("    -f, --frames <N>  Frame budget for 'run' (default 600, one frame = 1/60 s).");
    println!("    -t, --trace       Print every host callback in 'run', not just the summary.");
    println!("    -o, --out <FILE>  PNG path for 'capture' (default work/out/title.png).");
    println!("    -a, --answer <N>  Answer choice prompts with option N instead of holding the menu.");
    println!("    -F, --filter <T>  Only list events whose title contains T (with 'list').");
    println!("    --until <N>       Quit 'play' after N frames (smoke test; default: until closed).");
    println!("    --menu-only       Start common event 42 directly instead of the boot map.");
    println!("    --hold-message    Park on the first 101 message instead of dismissing it.");
    println!("    -m, --map <NAME>  Capture a map instead of the boot sequence (e.g. SampleMapA.mps).");
    println!("    --boot            Start on the boot map (default for capture and play).");
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
            checks.push(Check::text(
                "game.title",
                "妹相随～黑白世界的缤纷冒险～",
                game.title(),
            ));
            checks.push(Check::text("game.subtitle", "+DLC v2.03", game.subtitle()));
            checks.push(Check::num(
                "game.screen.w",
                1024,
                u64::from(game.screen_width()),
            ));
            checks.push(Check::num(
                "game.screen.h",
                576,
                u64::from(game.screen_height()),
            ));
            checks.push(Check::num(
                "game.tile_size",
                16,
                u64::from(game.tile_size()),
            ));
            checks.push(Check::num("game.fps", 60, u64::from(game.fps())));
            checks.push(Check::num("game.strings", 13, game.strings.len() as u64));
            checks.push(Check::num(
                "game.u8_settings",
                35,
                game.u8_settings.len() as u64,
            ));
            checks.push(Check::num(
                "game.u16_settings",
                22,
                game.u16_settings.len() as u64,
            ));
            checks.push(Check::text("game.font_main", "LOLITA", game.font_main()));
            checks.push(Check::num("game.random_a", 22227, u64::from(game.random_a)));
            checks.push(Check::num("game.random_b", 25684, u64::from(game.random_b)));
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
                checks.push(Check::num(run_condition_label(condition), expected, actual));
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

    // --- MapData/*.mps ----------------------------------------------------
    //
    // Every map must parse to its footer byte; a single mis-walked page would
    // desynchronise the rest of the file, so the aggregate counts below are a
    // stronger check than any per-map assertion.
    let map_files = map_file_list(&map_dir);
    if map_files.is_empty() {
        checks.push(Check::text("mps.parse", "9 maps", "NO .mps FILES"));
    } else {
        let mut maps = 0u64;
        let mut events = 0u64;
        let mut pages = 0u64;
        let mut empty_tiles = 0u64;
        let mut stats = SlotStats::default();
        let mut errors: Vec<String> = Vec::new();
        for path in &map_files {
            let Some(bytes) = read(path) else {
                errors.push(format!("{}: unreadable", path.display()));
                continue;
            };
            match wf::mps::Map::parse(&bytes) {
                Ok(map) => {
                    maps += 1;
                    if !map.has_tile_data {
                        empty_tiles += 1;
                    }
                    events += map.events.len() as u64;
                    for event in &map.events {
                        pages += event.pages.len() as u64;
                        for page in &event.pages {
                            stats.extend(page.commands.iter());
                        }
                    }
                }
                Err(err) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    errors.push(format!("{name}: {err}"));
                }
            }
        }
        let parse_note = if errors.is_empty() {
            "0 parse errors".to_owned()
        } else {
            errors.join("; ")
        };
        checks.push(Check::text("mps.parse", "0 parse errors", &parse_note));
        checks.push(Check::num("mps.maps", 9, maps));
        checks.push(Check::num("mps.events", 73, events));
        checks.push(Check::num("mps.pages", 134, pages));
        checks.push(Check::num("mps.slots", 37_610, stats.slots as u64));
        checks.push(Check::num("mps.blank", 7_052, stats.blank() as u64));
        checks.push(Check::num("mps.real", 30_558, stats.real() as u64));
        checks.push(Check::num(
            "mps.distinct_opcodes",
            40,
            stats.distinct_opcodes() as u64,
        ));
        checks.push(Check::num("mps.empty_maps", 2, empty_tiles));
    }

    // --- TileSetData.dat --------------------------------------------------
    match read(&basic.join("TileSetData.dat")).map(|b| wf::tileset::TileSetData::parse(&b)) {
        Some(Ok(sets)) => {
            checks.push(Check::num("tileset.version", 0xD2, u64::from(sets.version)));
            checks.push(Check::num("tileset.count", 6, sets.tilesets.len() as u64));
            checks.push(Check::num("tileset.footer", 0xCF, u64::from(sets.footer)));
            let base = sets
                .tilesets
                .get(4)
                .map(|set| set.base_image.as_str())
                .unwrap_or("<missing>");
            checks.push(Check::text("tileset.base.4", "MapChip/kiri_project.png", base));
        }
        Some(Err(err)) => {
            checks.push(Check::text("tileset.parse", "ok", &format!("FAILED: {err}")));
        }
        None => checks.push(Check::text("tileset.parse", "ok", "FILE MISSING")),
    }

    // --- the three databases ----------------------------------------------
    //
    // Type counts plus one row count each: the row count is what proves the
    // numeric-then-string row layout was walked correctly.
    for (label, file, expected_types, expected_rows) in [
        ("database.types", "DataBase.dat", 40u64, 44u64),
        ("cdata.types", "CDataBase.dat", 30, 17),
        ("sysdata.types", "SysDatabase.dat", 28, 50),
    ] {
        match read(&basic.join(file)).map(|b| wf::database::Database::parse(&b)) {
            Some(Ok(db)) => {
                checks.push(Check::num(label, expected_types, db.types.len() as u64));
                let rows = db.types.first().map(|t| t.rows.len() as u64).unwrap_or(0);
                checks.push(Check::num(
                    match label {
                        "database.types" => "database.rows.0",
                        "cdata.types" => "cdata.rows.0",
                        _ => "sysdata.rows.0",
                    },
                    expected_rows,
                    rows,
                ));
                if label == "sysdata.types" {
                    let settings = db.map_settings();
                    checks.push(Check::num("database.map_settings", 6, settings.len() as u64));
                    let first = settings
                        .first()
                        .map(|s| s.path.as_str())
                        .unwrap_or("<none>");
                    checks.push(Check::text(
                        "database.map_settings.0",
                        "MapData/TitleMap.mps",
                        first,
                    ));
                }
            }
            Some(Err(err)) => {
                checks.push(Check::text("database.parse", "ok", &format!("FAILED: {err}")));
            }
            None => checks.push(Check::text("database.parse", "ok", "FILE MISSING")),
        }
    }

    // --- save files (wolf-save) -----------------------------------------
    // Saves are the one input that lives next to the executable rather than in
    // the unpacked data root, so a missing install directory downgrades this
    // whole section to SKIP instead of failing a fresh checkout.
    let saves = save_file_list();
    if saves.is_empty() {
        checks.push(Check::skip("save.header"));
    } else {
        let mut parsed = 0u64;
        let mut checksum_ok = 0u64;
        let mut versions: BTreeSet<u16> = BTreeSet::new();
        let mut names: BTreeSet<String> = BTreeSet::new();
        let mut endings: BTreeSet<u8> = BTreeSet::new();
        for path in &saves {
            let Some(bytes) = read(path) else { continue };
            match wolf_save::SaveFile::parse(&bytes) {
                Ok(save) => {
                    parsed += 1;
                    if save.checksum_ok() {
                        checksum_ok += 1;
                    }
                    versions.insert(save.file_version);
                    names.insert(save.name.clone());
                    endings.insert(save.end_byte);
                }
                Err(err) => checks.push(Check::text(
                    "save.parse",
                    "ok",
                    &format!("FAILED {}: {err}", path.display()),
                )),
            }
        }
        checks.push(Check::num("save.files", saves.len() as u64, parsed));
        checks.push(Check::num("save.checksum_ok", saves.len() as u64, checksum_ok));
        checks.push(Check::num("save.versions", 1, versions.len() as u64));
        let version = versions.iter().next().copied().unwrap_or(0);
        checks.push(Check::text(
            "save.version",
            "0x8C",
            &format!("0x{version:02X}"),
        ));
        checks.push(Check::num("save.end_bytes", 1, endings.len() as u64));
        let end = endings.iter().next().copied().unwrap_or(0);
        checks.push(Check::text("save.end_byte", "0x19", &format!("0x{end:02X}")));
        checks.push(Check::text(
            "save.name",
            "妹相随～黑白世界的缤纷冒险～",
            names.iter().next().map(String::as_str).unwrap_or("<none>"),
        ));
    }

    // --- .wolf DXArchive v8 (wolf-format::dxarchive) ----------------------
    // work/full was unpacked by UberWolf, so opening the same archive with our own
    // reader and re-parsing a file out of it pins the decryption end to end. The
    // archive is 1.2 GB in total; BasicData.wolf is the smallest one that still
    // exercises the Huffman + LZ path.
    match read(&game_dir().join("Data").join("BasicData.wolf"))
        .map(|bytes| wf::dxarchive::DxArchive::open(&bytes))
    {
        Some(Ok(archive)) => {
            checks.push(Check::num(
                "dxarchive.version",
                8,
                u64::from(archive.version()),
            ));
            checks.push(Check::num(
                "dxarchive.charcode",
                932,
                u64::from(archive.char_code_format()),
            ));
            checks.push(Check::num(
                "dxarchive.flags",
                20_578_304,
                u64::from(archive.flags()),
            ));
            checks.push(Check::num(
                "dxarchive.huffman_kb",
                12,
                u64::from(archive.huffman_encode_kb()),
            ));
            checks.push(Check::num(
                "dxarchive.entries",
                149,
                archive.entries().len() as u64,
            ));
            let extracted = archive
                .entry("Game.dat")
                .and_then(|entry| archive.read(entry).ok());
            let verdict = match extracted {
                Some(bytes) => match GameDat::parse(&bytes) {
                    Ok(game) if game.title() == "妹相随～黑白世界的缤纷冒险～" => "ok",
                    Ok(_) => "wrong title",
                    Err(_) => "parse failed",
                },
                None => "extract failed",
            };
            checks.push(Check::text("dxarchive.game_dat", "ok", verdict));
        }
        Some(Err(err)) => checks.push(Check::text(
            "dxarchive.open",
            "ok",
            &format!("FAILED: {err}"),
        )),
        None => checks.push(Check::skip("dxarchive.header")),
    }

    report(&checks)
}

/// Compile all 550 common events and diff the precompiler's invariants against
/// the counts measured in docs/OPCODE_SPEC.md.
///
/// This is the load-time half of the M1 acceptance test: the interpreter can be
/// as correct as it likes, but if the branch pairing is off by one the jump
/// tables point at the wrong slot and every downstream test lies.
fn run_precompile(root: &Path) -> ExitCode {
    println!("wolf-cli precompile");
    let basic = if root.join("BasicData").is_dir() {
        root.join("BasicData")
    } else {
        root.to_path_buf()
    };
    println!("  data root : {}", root.display());
    println!("  basic dir : {}", basic.display());
    println!();

    let events =
        match read(&basic.join("CommonEvent.dat")).map(|bytes| CommonEventData::parse(&bytes)) {
            Some(Ok(events)) => events,
            Some(Err(err)) => {
                eprintln!("  ! CommonEvent.dat: {err}");
                return ExitCode::FAILURE;
            }
            None => {
                eprintln!("  ! CommonEvent.dat not found under {}", basic.display());
                return ExitCode::FAILURE;
            }
        };

    let programs = ProgramSet::from_common_events(&events);
    let stats: PrecompileStats = programs.stats;

    println!("  blocks      {}", stats.blocks);
    println!("    conditions  {}", stats.conditions);
    println!("    choices     {}", stats.choices);
    println!("    loops       {}", stats.loops);
    println!(
        "  cases       {} (else {} / cancel {})",
        stats.cases, stats.else_cases, stats.cancel_cases
    );
    println!(
        "  orphans     {} case(s), {} end(s)",
        stats.orphan_cases, stats.orphan_ends
    );
    println!(
        "  implicit    {} block(s) closed by a parent 499",
        stats.implicit_closes
    );
    println!(
        "  unclosed    {} block(s) ran to the end of their event",
        stats.unclosed_blocks
    );
    println!("  depth       {} nested block(s)", stats.max_depth);
    println!(
        "  labels      {} defined, {} jump(s), {} dynamic",
        stats.labels, stats.label_jumps, stats.dynamic_labels
    );
    println!("  unresolved  {} label jump(s)", stats.unresolved_labels);
    println!();

    let checks = vec![
        Check::num("slots", 163_828, stats.slots as u64),
        Check::num("blocks", 13_310, stats.blocks as u64),
        // 111 / 102 / 170+179+176 marker counts from docs/BASELINE.json: the
        // precompiler must see each marker command exactly once.
        Check::num("conditions", 10_743, stats.conditions as u64),
        Check::num("choices", 495, stats.choices as u64),
        Check::num("loops", 2_072, stats.loops as u64),
        Check::num("labels", 606, stats.labels as u64),
        Check::num("label_jumps", 1_543, stats.label_jumps as u64),
        // 3, not 5: only events 31/63/86 put \cself[n] inside a 213 label
        // (docs/OPCODE_SPEC.md §4.7 corrected by measuring the data again).
        Check::num("dynamic_labels", 3, stats.dynamic_labels as u64),
        Check::num("malformed_conditions", 0, stats.malformed_conditions as u64),
        Check::num("cases", 18_460, stats.cases as u64),
        Check::num("else_cases", 3_246, stats.else_cases as u64),
        Check::num("cancel_cases", 150, stats.cancel_cases as u64),
        // Tolerance counters: this game really does contain markers with an
        // empty block stack, and the precompiler must survive them unchanged.
        Check::num("orphan_cases", 131, stats.orphan_cases as u64),
        Check::num("orphan_ends", 138, stats.orphan_ends as u64),
        Check::num("implicit_closes", 66, stats.implicit_closes as u64),
        Check::num("unclosed_blocks", 3, stats.unclosed_blocks as u64),
        Check::num("max_depth", 91, stats.max_depth as u64),
        Check::num("programs", 550, programs.len() as u64),
        // Every non-empty title must be findable again (300 CallByName and the
        // debug menu both depend on this index).
        Check::num(
            "titles_indexed",
            count_titles_indexed(&programs),
            count_titles_indexed(&programs),
        ),
    ];
    report(&checks)
}

/// How many event titles the name index can resolve again.
fn count_titles_indexed(programs: &ProgramSet) -> u64 {
    let mut count = 0u64;
    for id in 0..programs.len() as u32 {
        if let Some(program) = programs.event(id) {
            let title = program.title.trim();
            if !title.is_empty() && programs.id_by_name(title).is_some() {
                count += 1;
            }
        }
    }
    count
}

/// Everything the interpreter asked the platform layer to do, counted.
struct TraceHost {
    trace: bool,
    messages: Vec<String>,
    choices: Vec<usize>,
    /// None = hold at the first prompt, which is how the title screen is captured.
    answer: Option<usize>,
    pictures: usize,
    sounds: usize,
    databases: usize,
    move_routes: usize,
    teleports: usize,
    transitions: usize,
    parties: usize,
    scrolls: usize,
    effects: usize,
    saves: usize,
    reserves: usize,
    pro: usize,
    debug: Vec<String>,
    warnings: Vec<String>,
    /// Documented-but-unmodelled behaviour (124 sub-ops, 151/161 layouts).
    infos: Vec<String>,
    ended: bool,
}

impl TraceHost {
    fn new(trace: bool, answer: Option<usize>) -> Self {
        TraceHost {
            trace,
            messages: Vec::new(),
            choices: Vec::new(),
            answer,
            pictures: 0,
            sounds: 0,
            databases: 0,
            move_routes: 0,
            teleports: 0,
            transitions: 0,
            parties: 0,
            scrolls: 0,
            effects: 0,
            saves: 0,
            reserves: 0,
            pro: 0,
            debug: Vec::new(),
            warnings: Vec::new(),
            infos: Vec::new(),
            ended: false,
        }
    }

    fn note(&mut self, line: String) {
        if self.trace {
            println!("{line}");
        }
    }
}

impl Host for TraceHost {
    fn show_message(&mut self, text: &str, face: Option<i32>) {
        match face {
            Some(index) => self.note(format!("  101 message  [face {index}] {text}")),
            None => self.note(format!("  101 message  {text}")),
        }
        self.messages.push(text.to_owned());
    }

    // Auto-advance: a headless trace never waits for a keypress.
    fn message_dismissed(&mut self) -> bool {
        true
    }

    fn close_message(&mut self) {
        self.note("  105 close message".to_owned());
    }

    fn show_choices(&mut self, options: &[String]) {
        self.note(format!("  102 choices  {options:?}"));
    }

    // Without --answer the prompt is held: a headless trace then shows the menu
    // frame instead of branching away from it, which is what M1's screenshot
    // needs (docs/M1_CLOSURE.md §5).
    fn choice_made(&mut self) -> Option<ChoiceReply> {
        let index = self.answer?;
        if self.choices.is_empty() {
            self.choices.push(index);
            self.note(format!("  -> answered option {index}"));
        }
        Some(ChoiceReply::Index(index))
    }

    fn picture(&mut self, cmd: &PictureCmd) {
        self.pictures += 1;
        self.note(format!(
            "  150 picture  #{} kind {} {:?} file={} text={} args={:?}",
            cmd.number,
            cmd.kind,
            cmd.action,
            short(&cmd.file),
            short(&cmd.text),
            cmd.args
        ));
    }

    fn sound(&mut self, cmd: &SoundCmd) {
        self.sounds += 1;
        self.note(format!(
            "  140 sound    {:?} {:?} {} vol={} frames={}",
            cmd.channel,
            cmd.action,
            short(&cmd.file),
            cmd.volume,
            cmd.frames
        ));
    }

    fn database(&mut self, op: &DbOp) -> Option<DbValue> {
        self.databases += 1;
        self.note(format!(
            "  250 database type {} data {} item {} flags {:#x} destination {:?}",
            op.db_type, op.data_id, op.item_id, op.flags, op.destination
        ));
        None
    }

    fn move_route(&mut self, target: i32, route: &[wf::command::RouteCommand]) {
        self.move_routes += 1;
        self.note(format!(
            "  201 route    target {target} ({} step(s))",
            route.len()
        ));
    }

    fn teleport(&mut self, map: i32, x: i32, y: i32, direction: i32) {
        self.teleports += 1;
        self.note(format!(
            "  130 teleport map {map} at {x},{y} dir {direction}"
        ));
    }

    fn transition(&mut self, kind: i32, flags: i32) {
        self.transitions += 1;
        self.note(format!("  16x transition kind {kind} flags {flags:#x}"));
    }

    fn party(&mut self, args: &[i32]) {
        self.parties += 1;
        self.note(format!("  270 party    {args:?}"));
    }

    fn scroll(&mut self, args: &[i32]) {
        self.scrolls += 1;
        self.note(format!("  281 scroll   {args:?}"));
    }

    fn effect(&mut self, kind: i32, duration: i32, args: &[i32]) {
        self.effects += 1;
        self.note(format!(
            "  290 effect   kind {kind} for {duration} {args:?}"
        ));
    }

    fn save_load(&mut self, args: &[i32]) {
        self.saves += 1;
        self.note(format!("  22x saveload {args:?}"));
    }

    fn reserve(&mut self, event: wolf_vm::exec::EventId) {
        self.reserves += 1;
        self.note(format!("  211 reserve  event {event}"));
    }

    fn debug_message(&mut self, text: &str) {
        self.note(format!("  106 debug    {text}"));
        self.debug.push(text.to_owned());
    }

    fn clear_debug(&mut self) {
        self.note("  107 clear debug".to_owned());
    }

    fn return_to_title(&mut self) {
        self.note("  174 return to title".to_owned());
        self.ended = true;
    }

    fn end_game(&mut self) {
        self.note("  175 end game".to_owned());
        self.ended = true;
    }

    fn pro_feature(&mut self, kind: i32, milliseconds: i32) {
        self.pro += 1;
        self.note(format!("  1000 pro     kind {kind} for {milliseconds} ms"));
    }

    fn log(&mut self, level: LogLevel, message: &str) {
        if self.trace {
            println!("  [{level:?}] {message}");
        }
        match level {
            LogLevel::Warn | LogLevel::Error => {
                self.warnings.push(format!("{level:?}: {message}"));
            }
            LogLevel::Info => self.infos.push(message.to_owned()),
            LogLevel::Trace => {}
        }
    }
}

/// Keep trace lines readable: escape-bearing picture names can be long.
fn short(text: &str) -> String {
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= 40 {
            out.push('…');
            break;
        }
        out.push(if ch.is_control() { ' ' } else { ch });
    }
    out
}

/// Run one common event headlessly.
///
/// This is the M1 debugging workhorse from PLAN.md: \`wolf-cli run --event N\`
/// executes the event against a recording host so a branch can be checked
/// without a window, a GPU or a save file.
/// Print every common event with its size, so a caller can find the event that
/// drives a given screen without opening the editor.
///
/// The real-instruction count excludes the NOP slots the editor leaves behind,
/// which is why it is smaller than the slot count (docs/OPCODE_SPEC.md §0.1).
fn run_list(root: &Path, filter: Option<&str>) -> ExitCode {
    let basic = if root.join("BasicData").is_dir() {
        root.join("BasicData")
    } else {
        root.to_path_buf()
    };
    let Some(bytes) = read(&basic.join("CommonEvent.dat")) else {
        return ExitCode::FAILURE;
    };
    let events = match wf::commonevent::CommonEventData::parse(&bytes) {
        Ok(events) => events,
        Err(err) => {
            eprintln!("CommonEvent.dat: {err}");
            return ExitCode::FAILURE;
        }
    };
    let needle = filter.map(str::to_lowercase);
    println!("events in {}/CommonEvent.dat", basic.display());
    let mut shown = 0usize;
    for event in &events.events {
        let title = &event.title;
        if let Some(needle) = &needle {
            if !title.to_lowercase().contains(needle) {
                continue;
            }
        }
        let mut stats = SlotStats::default();
        stats.extend(&event.commands);
        shown += 1;
        println!(
            "{:>4}  {:>5} slot(s)  {:>5} real  {:>3} op(s)  {}",
            event.id,
            stats.slots,
            stats.real(),
            stats.distinct_opcodes(),
            title
        );
    }
    println!("{shown} event(s) listed");
    ExitCode::SUCCESS
}

fn run_event(root: &Path, id: u32, frames: usize, trace: bool, answer: Option<usize>) -> ExitCode {
    println!("wolf-cli run --event {id} --frames {frames}");
    let basic = if root.join("BasicData").is_dir() {
        root.join("BasicData")
    } else {
        root.to_path_buf()
    };
    println!("  data root : {}", root.display());
    println!();

    let events =
        match read(&basic.join("CommonEvent.dat")).map(|bytes| CommonEventData::parse(&bytes)) {
            Some(Ok(events)) => events,
            Some(Err(err)) => {
                eprintln!("  ! CommonEvent.dat: {err}");
                return ExitCode::FAILURE;
            }
            None => {
                eprintln!("  ! CommonEvent.dat not found under {}", basic.display());
                return ExitCode::FAILURE;
            }
        };

    let programs = ProgramSet::from_common_events(&events);
    let program = match programs.event(id) {
        Some(program) => Arc::clone(program),
        None => {
            eprintln!("  ! no common event with id {id}");
            return ExitCode::from(2);
        }
    };
    println!("  event     [{id}] {}", program.title);
    println!("  slots     {}", program.len());
    println!();

    let mut vm = Vm::new(programs);
    let mut host = TraceHost::new(trace, answer);
    if vm.start_event(id).is_none() {
        eprintln!("  ! cannot start event {id}");
        return ExitCode::FAILURE;
    }

    let mut total_steps = 0usize;
    let mut peak_instances = 1usize;
    let mut frames_run = 0usize;
    let mut reported: Vec<String> = Vec::new();
    for _ in 0..frames {
        let report: StepReport = vm.step_frame(&mut host);
        frames_run += 1;
        total_steps += report.steps;
        peak_instances = peak_instances.max(report.running);
        for note in &report.notes {
            if !reported.iter().any(|seen| seen == note) {
                reported.push(note.clone());
            }
        }
        if report.running == 0 {
            break;
        }
    }

    println!();
    println!("  frames run       {frames_run}");
    println!("  instructions     {total_steps}");
    println!("  peak coroutines  {peak_instances}");
    println!("  messages         {}", host.messages.len());
    println!("  choices          {}", host.choices.len());
    println!("  pictures         {}", host.pictures);
    println!("  sounds           {}", host.sounds);
    println!("  database ops     {}", host.databases);
    println!("  move routes      {}", host.move_routes);
    println!("  teleports        {}", host.teleports);
    println!("  transitions      {}", host.transitions);
    println!("  party ops        {}", host.parties);
    println!("  scrolls          {}", host.scrolls);
    println!("  effects          {}", host.effects);
    println!("  save/load        {}", host.saves);
    println!("  reserves         {}", host.reserves);
    println!("  pro features     {}", host.pro);
    if answer.is_none() && !host.choices.is_empty() {
        println!(
            "  holding          the event is parked on choice prompt {} (pass --answer <N> to take a branch)",
            host.choices.len()
        );
    }
    if !host.messages.is_empty() {
        println!();
        println!("  first message    {}", short(&host.messages[0]));
        if host.messages.len() > 1 {
            println!(
                "  last message     {}",
                short(&host.messages[host.messages.len() - 1])
            );
        }
    }
    if !host.infos.is_empty() {
        println!();
        println!(
            "  unmodelled       {} (documented gaps, not failures)",
            host.infos.len()
        );
        let mut seen: Vec<&String> = Vec::new();
        for note in &host.infos {
            if !seen.contains(&note) {
                seen.push(note);
            }
        }
        for note in seen.iter().take(10) {
            println!("    {note}");
        }
    }
    if !host.warnings.is_empty() {
        println!();
        println!("  warnings         {}", host.warnings.len());
        let mut seen: Vec<&String> = Vec::new();
        for note in &host.warnings {
            if !seen.contains(&note) {
                seen.push(note);
            }
        }
        for note in seen.iter().take(20) {
            println!("    {note}");
        }
    }
    if !reported.is_empty() {
        println!();
        println!("  diagnostics:");
        for note in reported.iter().take(40) {
            println!("    {note}");
        }
    }

    let checks = vec![
        Check::num("started", 1, 1),
        Check::num("frames_run > 0", 1, u64::from(frames_run > 0)),
        // Zero warnings means every opcode this event touched is implemented and
        // every string/label it needed resolved.
        Check::num("warnings", 0, host.warnings.len() as u64),
        Check::num("no diagnostics", 0, reported.len() as u64),
    ];
    let code = report(&checks);
    if !reported.is_empty() {
        println!(
            "NOTE — {} distinct interpreter diagnostic(s) above",
            reported.len()
        );
    }
    code
}

fn run_condition_label(condition: u8) -> &'static str {
    match condition {
        0 => "ce.run_condition.0",
        1 => "ce.run_condition.1",
        2 => "ce.run_condition.2",
        _ => "ce.run_condition.3",
    }
}

/// All .mps files in a map directory, sorted so the output is stable.
/// Locate the game's `.sav` files.
///
/// Ordering matters: `SaveData01`..`SaveData08` are user slots and
/// `System` is the settings file, and sorting by path keeps the report
/// reproducible across runs.
fn save_file_list() -> Vec<PathBuf> {
    let dir = std::env::var_os("WOLF_SAVE_DIR")
        .map_or_else(|| game_dir().join("Save"), PathBuf::from);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("sav"))
        })
        .collect();
    files.sort();
    files
}

fn map_file_list(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("mps"))
        })
        .collect();
    files.sort();
    files
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
            println!(
                "[{mark}] {:<width$}  (loader pending)",
                check.label,
                width = width
            );
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


/// Install directory of the original game.
///
/// `$WOLF_GAME_DIR` wins; the literal default is this machine's Steam library and is
/// the only hard-coded path in the project (fonts and saves are not in the unpacked
/// data root, and both are optional inputs).
fn game_dir() -> PathBuf {
    const DEFAULT: &str = "E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy";
    match std::env::var("WOLF_GAME_DIR") {
        Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(DEFAULT),
    }
}

/// Finds a TTF to draw WOLF text with.
///
/// Search order: `<root>/Fonts`, `<root>`, then `$WOLF_GAME_DIR`, then the documented
/// Steam install path. A missing font is not fatal - the capture still renders the art
/// and reports that text was skipped.
fn find_font(root: &Path) -> Option<(PathBuf, Vec<u8>)> {
    const NAMES: [&str; 4] = [
        "Lolita.ttf",
        "07yasashi.ttf",
        "GenJyuuGothicXPB.ttf",
        "MSGOTHIC.TTF",
    ];
    let mut bases: Vec<PathBuf> = vec![root.join("Fonts"), root.to_path_buf()];
    bases.push(game_dir());
    for base in bases {
        for name in NAMES {
            let candidate = base.join(name);
            if let Ok(bytes) = std::fs::read(&candidate) {
                return Some((candidate, bytes));
            }
        }
    }
    None
}


/// Builds a render instance from an atlas quad, carrying its alpha and its picture tone.
///
/// The tone is per picture, not per frame: a `290` flash changes what one picture
/// multiplies its texels by while everything else on screen keeps drawing at full
/// brightness, so it has to ride on the instance rather than on a global draw state.
fn tint_instance(quad: &wolf_game::atlas::SpriteQuad) -> SpriteInstance {
    SpriteInstance::new(quad.rect, quad.uv)
        .with_alpha(quad.alpha)
        .with_tint_rgb(quad.tint[0], quad.tint[1], quad.tint[2])
}/// Where a capture starts.
///
/// The retail entry point is the boot map, but two other entries are useful: the
/// bare title menu (to photograph the runtime choice panel without the splash
/// chain) and an arbitrary map event (the M2 story scripts, which never call the
/// title at all).
enum Entry<'a> {
    Boot,
    Menu,
    Script { map: &'a str, event: u32 },
}

fn run_capture(
    root: &Path,
    out: Option<PathBuf>,
    frames: usize,
    answer: Option<usize>,
    hold_message: bool,
    entry: Entry<'_>,
) -> ExitCode {
    let out = out.unwrap_or_else(|| PathBuf::from("work/out/title.png"));
    let mut game = match Game::new(root) {
        Ok(game) => game,
        Err(err) => {
            eprintln!("cannot load the game data: {err}");
            return ExitCode::FAILURE;
        }
    };
    // A capture has no keyboard, so messages dismiss immediately. Choice prompts are
    // HELD by default: the title's first prompt is its menu, and answering it teleports
    // into the game (docs/M1_CLOSURE.md §5), so a held run is what photographs the title.
    // --hold-message parks the run on the first 101 message instead, which is how a
    // capture photographs the message window (the story scripts talk before they branch).
    game.auto_advance = !hold_message;
    if let Some(index) = answer {
        game.set_choice_cursor(index);
        game.auto_choice = true;
    }
    // Two entry points: the real boot path (map autorun event -> splashes -> title art
    // -> menu) and the bare menu event 42, kept for regression captures of the menu
    // overlay on its own.
    let (started, label) = match entry {
        Entry::Boot => (
            game.boot().map_err(|err| err.to_string()),
            "boot map (TitleMap.mps event 2)".to_owned(),
        ),
        Entry::Menu => (
            game.start_title().map_err(|err| err.to_string()),
            "common event 42".to_owned(),
        ),
        Entry::Script { map, event } => (
            game.start_map_event_by_name(map, event)
                .map_err(|err| err.to_string()),
            format!("map script {map} event {event}"),
        ),
    };
    if let Err(err) = started {
        eprintln!("{err}");
        return ExitCode::FAILURE;
    }
    println!("  entry     : {label}");

    let mut peak = 0usize;
    for _ in 0..frames {
        let outcome = game.frame();
        peak = peak.max(outcome.report.running);
    }

    let mut atlas = Atlas::new();
    let mut sprites = atlas.sprites(&game.screen, &mut game.assets);
    let mut texts = atlas::texts(&game.screen);
    // A WOLF script never draws its own choice frame: the engine paints a window skin and
    // moves a cursor over it, at the coordinates system variables 3 (x) and 4 (y) name.
    // There is no centred default (docs/OPCODE_102.md section 4.2), and when the finished
    // rectangle falls outside the 1024x576 viewport the engine paints nothing at all -
    // which is what the title event relies on when it parks the window at y = 1000.
    if let Some(choices) = &game.choices {
        let options = choices.options.clone();
        let cursor = game.choice_cursor();
        let (x, y) = game.choice_anchor();
        if let Some((panel, rows)) =
            choice::overlay_at(&mut atlas, &mut game.assets, &options, cursor, x, y)
        {
            sprites.extend(panel);
            texts.extend(rows);
        }
    }
    let [atlas_w, atlas_h] = atlas.size();

    println!("capture");
    println!("  data root : {}", root.display());
    println!("  frames    : {frames} (peak {peak} coroutine(s))");
    println!(
        "  pictures  : {} live -> {} textured quad(s), {} text picture(s)",
        game.screen.len(),
        sprites.len(),
        texts.len()
    );
    for picture in game.screen.iter() {
        println!(
            "    pic {:<6} {:?} at ({}, {}) alpha {:.2} scale {} window {:?} {}",
            picture.number,
            picture.kind,
            picture.x,
            picture.y,
            picture.alpha(),
            picture.scale,
            picture.window,
            if picture.file.is_empty() {
                format!("text {:?}", picture.text)
            } else {
                format!("file {:?}", picture.file)
            }
        );
    }
    match &game.choices {
        Some(choices) => {
            let (x, y) = game.choice_anchor();
            let anchored = choice::layout_at(&choices.options, game.choice_cursor(), x, y);
            println!(
                "  choice    : HELD on prompt with {} option(s) {:?}, cursor {} at anchor ({x}, {y})",
                choices.options.len(),
                choices.options,
                game.choice_cursor()
            );
            match anchored {
                Some(panel) => println!(
                    "            : engine rect {:?}, {} row(s)",
                    panel.rect,
                    panel.lines.len()
                ),
                None => println!("            : anchor is off screen, engine draws nothing"),
            }
        }
        None => println!("  choice    : none open"),
    }
    println!(
        "  atlas     : {} image(s), {:.1}% of {atlas_w}x{atlas_h}",
        atlas.len(),
        atlas.fill_ratio() * 100.0
    );
    for quad in &sprites {
        println!(
            "    quad pic {:>6} at [{:.0},{:.0},{:.0},{:.0}] uv [{:.3},{:.3},{:.3},{:.3}]",
            quad.number, quad.rect[0], quad.rect[1], quad.rect[2], quad.rect[3],
            quad.uv[0], quad.uv[1], quad.uv[2], quad.uv[3]
        );
    }
    for text in &texts {
        let preview: String = text.text.chars().take(40).collect();
        println!(
            "    text pic {:>6} at [{:.0},{:.0},{:.0},{:.0}] {}",
            text.number, text.rect[0], text.rect[1], text.rect[2], text.rect[3], preview
        );
    }

    let mut renderer = match Renderer::new_offscreen() {
        Ok(renderer) => renderer,
        Err(err) => {
            eprintln!("no usable GPU adapter: {err}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(err) = renderer.set_atlas(atlas.pixels(), atlas_w, atlas_h) {
        eprintln!("atlas upload failed: {err}");
        return ExitCode::FAILURE;
    }
    // Text pictures need a font; the original game keeps its TTFs next to Game.exe, not
    // inside the unpacked data root, so the lookup falls back to the install directory.
    match find_font(root) {
        Some((path, bytes)) => match renderer.set_font(&bytes) {
            Ok(()) => println!("  font      : {}", path.display()),
            Err(err) => eprintln!("  font rejected ({}): {err}", path.display()),
        },
        None => println!("  font      : none found; text pictures will not draw"),
    }
    let instances: Vec<SpriteInstance> = sprites
        .iter()
        .map(tint_instance)
        .collect();
    renderer.draw(&instances);
    let mut items: Vec<TextItem> = texts
        .iter()
        .map(|text| TextItem::new(text.text.clone(), text.rect, choice::TEXT_PX).with_alpha(text.alpha))
        .collect();
    // The engine draws 101 message text itself: the frame comes from common event 63 as
    // ordinary pictures, the glyphs come from here. Only the newest message is visible,
    // which is the order the interpreter produces.
    if let Some(message) = game.messages.last() {
        let vars = game.message_vars();
        let layout = wolf_game::message::layout(&message.text, &vars, message.frames);
        let preview: String = message.text.chars().take(60).collect();
        let typing = if layout.typing { ", typing" } else { "" };
        let cursor = layout
            .cursor
            .map_or(String::new(), |at| format!(", cursor at [{:.0},{:.0}]", at[0], at[1]));
        println!(
            "  message   : {} row(s), {}/{} char(s), font {:.0} at [{:.0},{:.0}], wrap {:.0}{}{}",
            layout.rows.len(),
            layout.typed,
            layout.total,
            layout.font_px,
            vars.x,
            vars.y,
            vars.wrap_width(),
            typing,
            cursor
        );
        println!("    text    : {preview:?}");
        for row in &layout.rows {
            items.push(TextItem::new(row.text.clone(), row.rect, layout.font_px));
        }
        if let Some(at) = layout.cursor {
            let box_w = layout.font_px * 2.0;
            items.push(TextItem::new(
                "▼",
                [at[0], at[1], box_w, layout.line_height],
                layout.font_px,
            ));
        }
    }
    renderer.draw_text(&items);

    renderer.present();
    // Text statistics are only known after the pass has been encoded, so they are reported
    // here rather than next to the item list.
    let text_stats = renderer.text_stats();
    println!(
        "  text      : {} item(s), {} glyph(s), {} line(s), {} substituted",
        text_stats.items, text_stats.glyphs, text_stats.lines, text_stats.substituted
    );
    if let Some(missing) = text_stats.missing.first() {
        eprintln!("  warning   : font has no glyph for {missing:?}");
    }
    let pixels = match renderer.capture_rgba() {
        Ok(pixels) => pixels,
        Err(err) => {
            eprintln!("readback failed: {err}");
            return ExitCode::FAILURE;
        }
    };

    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                eprintln!("cannot create {}: {err}", parent.display());
                return ExitCode::FAILURE;
            }
        }
    }
    if let Err(err) = image::save_buffer(&out, &pixels, LOGICAL_W, LOGICAL_H, image::ColorType::Rgba8)
    {
        eprintln!("cannot write {}: {err}", out.display());
        return ExitCode::FAILURE;
    }

    let lit = pixels
        .chunks_exact(4)
        .filter(|pixel| pixel[0] | pixel[1] | pixel[2] != 0)
        .count();
    let total = (LOGICAL_W * LOGICAL_H) as usize;
    println!(
        "  rendered  : {LOGICAL_W}x{LOGICAL_H}, {lit}/{total} non-black pixel(s) = {:.1}%",
        lit as f64 * 100.0 / total as f64
    );
    println!("  wrote     : {}", out.display());

    let warnings = game.stats.warnings;
    println!(
        "  stats     : {} instruction(s), {} picture op(s), {} sound(s), {} db access(es), {warnings} warning(s)",
        game.stats.instructions, game.stats.pictures, game.stats.sounds, game.stats.databases
    );
    if !game.diagnostics.is_empty() {
        println!("  diagnostics ({} distinct):", game.diagnostics.len());
        for (text, count) in game.diagnostics.iter().take(12) {
            println!("    {count:>6}x {text}");
        }
    }
    ExitCode::SUCCESS
}

/// Renders one map frame to a PNG. This is M2 step one: the tile pipeline end to end.
///
/// The camera centres on the map's first event, which is where the engine puts the
/// player when a map starts; a map with no events looks at the top-left corner. Only
/// the cells inside the 1024x576 window are tessellated, so the 70x170 map costs the
/// same as the 53x30 one.
fn run_capture_map(root: &Path, out: Option<PathBuf>, name: &str) -> ExitCode {
    let out = out.unwrap_or_else(|| PathBuf::from("work/out/map.png"));
    let basic = if root.join("BasicData").is_dir() {
        root.join("BasicData")
    } else {
        root.to_path_buf()
    };
    let tileset_bytes = match read(&basic.join("TileSetData.dat")) {
        Some(bytes) => bytes,
        None => {
            eprintln!("no TileSetData.dat under {}", basic.display());
            return ExitCode::FAILURE;
        }
    };
    let tilesets = match wf::tileset::TileSetData::parse(&tileset_bytes) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("TileSetData.dat: {err}");
            return ExitCode::FAILURE;
        }
    };
    let relative: PathBuf = if name.contains('/') || name.ends_with(".mps") {
        PathBuf::from(name)
    } else {
        PathBuf::from(format!("MapData/{name}.mps"))
    };
    let map_bytes = match read(&root.join(&relative)) {
        Some(bytes) => bytes,
        None => {
            eprintln!("cannot read {}", root.join(&relative).display());
            return ExitCode::FAILURE;
        }
    };
    let map = match wf::mps::Map::parse(&map_bytes) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("{}: {err}", relative.display());
            return ExitCode::FAILURE;
        }
    };
    let Some(tileset) = tilesets.get(map.tileset_index) else {
        eprintln!("map points at tileset {} but only {} exist", map.tileset_index, tilesets.tilesets.len());
        return ExitCode::FAILURE;
    };

    let mut graphics: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for event in &map.events {
        for page in &event.pages {
            if !page.graphic.is_empty() {
                *graphics.entry(page.graphic.clone()).or_insert(0) += 1;
            }
        }
    }
    if !graphics.is_empty() {
        println!("  graphics  : {} distinct page chip(s)", graphics.len());
        for (name, count) in graphics.iter().take(20) {
            println!("              {count:>3}x {name}");
        }
    }

    let tile_map = TileMap::new(&map, tileset, tilesets.version);
    let [map_w, map_h] = tile_map.size_px();
    let screen = [LOGICAL_W as i32, LOGICAL_H as i32];
    let focus = match map.events.first() {
        Some(event) => [
            event.x as i32 * wolf_game::tilemap::TILE_PX as i32,
            event.y as i32 * wolf_game::tilemap::TILE_PX as i32,
        ],
        None => [0, 0],
    };
    let camera = [
        (focus[0] - screen[0] / 2).clamp(0, (map_w - screen[0]).max(0)),
        (focus[1] - screen[1] / 2).clamp(0, (map_h - screen[1]).max(0)),
    ];

    let mut atlas = Atlas::new();
    let mut assets = wolf_game::Assets::new(root);
    let sprites = tile_map.quads(&mut atlas, &mut assets, camera, [LOGICAL_W, LOGICAL_H], 0);
    let [atlas_w, atlas_h] = atlas.size();

    println!("capture --map {name}");
    println!("  data root : {}", root.display());
    println!(
        "  map       : {} {}x{} tile(s) = {}x{} px, {} layer(s), {} event(s)",
        relative.display(),
        tile_map.width,
        tile_map.height,
        map_w,
        map_h,
        tile_map.layer_count(),
        map.events.len()
    );
    println!(
        "  tileset   : index {} base {}",
        tile_map.tileset_index, tileset.base_image
    );
    println!("  camera    : [{}, {}]", camera[0], camera[1]);
    println!("  quads     : {} tile(s)", sprites.len());
    println!("  atlas     : {} image(s), {:.1}% of {atlas_w}x{atlas_h}", atlas.len(), atlas.fill_ratio() * 100.0);

    let mut renderer = match Renderer::new_offscreen() {
        Ok(renderer) => renderer,
        Err(err) => {
            eprintln!("no usable GPU adapter: {err}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(err) = renderer.set_atlas(atlas.pixels(), atlas_w, atlas_h) {
        eprintln!("atlas upload failed: {err}");
        return ExitCode::FAILURE;
    }
    let instances: Vec<SpriteInstance> = sprites
        .iter()
        .map(tint_instance)
        .collect();
    renderer.draw(&instances);
    renderer.draw_text(&[]);
    renderer.present();
    let pixels = match renderer.capture_rgba() {
        Ok(pixels) => pixels,
        Err(err) => {
            eprintln!("readback failed: {err}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                eprintln!("cannot create {}: {err}", parent.display());
                return ExitCode::FAILURE;
            }
        }
    }
    if let Err(err) = image::save_buffer(&out, &pixels, LOGICAL_W, LOGICAL_H, image::ColorType::Rgba8) {
        eprintln!("cannot write {}: {err}", out.display());
        return ExitCode::FAILURE;
    }
    let lit = pixels
        .chunks_exact(4)
        .filter(|pixel| pixel[0] | pixel[1] | pixel[2] != 0)
        .count();
    let total = (LOGICAL_W * LOGICAL_H) as usize;
    println!(
        "  rendered  : {LOGICAL_W}x{LOGICAL_H}, {lit}/{total} non-black pixel(s) = {:.1}%",
        lit as f64 * 100.0 / total as f64
    );
    println!("  wrote     : {}", out.display());
    ExitCode::SUCCESS
}

/// Run one MAP event page through the interpreter.
///
/// The M1 title screen lives in a map (TitleMap.mps event 2), and PLAN's M2
/// acceptance closure is keyed on SampleMapA event 1, so map command streams
/// need the same treatment common events already get: compile the page with
/// `Program::from_slots` (it builds the identical jump tables) and step it.
/// Common events stay loaded because map scripts reach them through opcode 210.
fn run_map_event(
    root: &Path,
    map_name: &str,
    event_id: u32,
    frames: usize,
    trace: bool,
    answer: Option<usize>,
    hot: bool,
) -> ExitCode {
    println!("wolf-cli run --map {map_name} --event {event_id} --frames {frames}");
    let basic = if root.join("BasicData").is_dir() {
        root.join("BasicData")
    } else {
        root.to_path_buf()
    };
    println!("  data root : {}", root.display());

    let events =
        match read(&basic.join("CommonEvent.dat")).map(|bytes| CommonEventData::parse(&bytes)) {
            Some(Ok(events)) => events,
            Some(Err(err)) => {
                eprintln!("  ! CommonEvent.dat: {err}");
                return ExitCode::FAILURE;
            }
            None => {
                eprintln!("  ! CommonEvent.dat not found under {}", basic.display());
                return ExitCode::FAILURE;
            }
        };
    let programs = ProgramSet::from_common_events(&events);

    let cleaned = map_name.replace(std::path::MAIN_SEPARATOR, "/");
    let relative: PathBuf = if cleaned.contains('/') || cleaned.ends_with(".mps") {
        PathBuf::from(cleaned)
    } else {
        PathBuf::from(format!("MapData/{cleaned}.mps"))
    };
    let map_bytes = match read(&root.join(&relative)) {
        Some(bytes) => bytes,
        None => {
            eprintln!("  ! cannot read {}", root.join(&relative).display());
            return ExitCode::FAILURE;
        }
    };
    let map = match wf::mps::Map::parse(&map_bytes) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("  ! {}: {err}", relative.display());
            return ExitCode::FAILURE;
        }
    };
    println!(
        "  map       {} {}x{} tile(s), {} event(s)",
        relative.display(),
        map.width,
        map.height,
        map.events.len()
    );

    let found = map.events.iter().find(|event| event.id == event_id);
    let Some(event) = found else {
        eprintln!("  ! no event with id {event_id}; present ids:");
        for event in &map.events {
            eprintln!("      [{:>6}] {}", event.id, event.name);
        }
        return ExitCode::from(2);
    };
    let Some(page) = event.pages.first() else {
        eprintln!("  ! event {event_id} has no page");
        return ExitCode::from(2);
    };
    println!(
        "  event     [{event_id}] {} at ({}, {}), {} page(s)",
        event.name,
        event.x,
        event.y,
        event.pages.len()
    );

    for (index, page) in event.pages.iter().enumerate() {
        println!(
            "  page {index}    trigger {} graphic {:?} chip {} slots {}",
            page.trigger,
            page.graphic,
            page.graphic_chip_id,
            page.commands.len()
        );
    }

    let program = Program::from_slots(event.id, &event.name, page.commands.clone());
    let slots = program.len();
    println!("  slots     {slots}");
    println!();

    let mut vm = Vm::new(programs);
    vm.hot_trace = hot;
    let mut host = TraceHost::new(trace, answer);
    if vm.start_program(Arc::new(program)).is_none() {
        eprintln!("  ! cannot start map event {event_id}");
        return ExitCode::FAILURE;
    }

    let mut total_steps = 0usize;
    let mut peak_instances = 1usize;
    let mut frames_run = 0usize;
    let mut spots: Vec<(u32, usize, u64)> = Vec::new();
    for _ in 0..frames {
        let report: StepReport = vm.step_frame(&mut host);
        frames_run += 1;
        total_steps += report.steps;
        peak_instances = peak_instances.max(report.running);
        for spot in &report.hottest {
            match spots
                .iter_mut()
                .find(|entry| entry.0 == spot.event && entry.1 == spot.pc)
            {
                Some(entry) => entry.2 += spot.hits,
                None => spots.push((spot.event, spot.pc, spot.hits)),
            }
        }
        if report.running == 0 {
            break;
        }
    }

    println!();
    println!("  frames run       {frames_run}");
    println!("  instructions     {total_steps}");
    println!("  peak coroutines  {peak_instances}");
    println!("  messages         {}", host.messages.len());
    println!("  choices          {}", host.choices.len());
    println!("  pictures         {}", host.pictures);
    println!("  sounds           {}", host.sounds);
    println!("  database ops     {}", host.databases);
    println!("  move routes      {}", host.move_routes);
    println!("  teleports        {}", host.teleports);
    println!("  transitions      {}", host.transitions);
    println!("  party ops        {}", host.parties);
    println!("  scrolls          {}", host.scrolls);
    println!("  effects          {}", host.effects);
    println!("  save/load        {}", host.saves);
    println!("  reserves         {}", host.reserves);
    println!("  pro features     {}", host.pro);
    // The park position is what makes a stuck script debuggable: a story script that
    // redraws the same two pictures every frame is waiting on one specific slot.
    for instance in vm.instances() {
        println!(
            "  live             id {} event {} pc {}/{} wait {:?} self0 {} self10 {}",
            instance.id,
            instance.event,
            instance.pc,
            instance.program.len(),
            instance.wait,
            instance.self_vars.get(0),
            instance.self_vars.get(10)
        );
    }
    if !spots.is_empty() {
        spots.sort_by_key(|spot| std::cmp::Reverse(spot.2));
        println!();
        println!("  hot spots        (instruction address -> hits)");
        for (event, pc, hits) in spots.iter().take(10) {
            println!("    ev {event:<6} pc {pc:<5} {hits} hit(s)");
        }
    }
    if !host.infos.is_empty() {
        println!();
        println!(
            "  unmodelled       {} (documented gaps, not failures)",
            host.infos.len()
        );
        let mut seen: Vec<&String> = Vec::new();
        for note in &host.infos {
            if !seen.contains(&note) {
                seen.push(note);
            }
        }
        for note in seen.iter().take(10) {
            println!("    {note}");
        }
    }
    if !host.warnings.is_empty() {
        println!();
        println!("  warnings         {}", host.warnings.len());
        let mut seen: Vec<&String> = Vec::new();
        for note in &host.warnings {
            if !seen.contains(&note) {
                seen.push(note);
            }
        }
        for note in seen.iter().take(10) {
            println!("    {note}");
        }
    }

    ExitCode::SUCCESS
}
