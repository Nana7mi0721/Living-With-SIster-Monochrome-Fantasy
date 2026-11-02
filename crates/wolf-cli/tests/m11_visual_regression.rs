//! Deterministic visual regression suite for the offscreen capture path.
//!
//! These tests render five fixed scenes through the real wolf-cli capture path and
//! pin the resulting frame statistics: size, non-black pixels, live picture count,
//! textured quad count, text picture count, atlas image count, message/choice
//! counters, and warnings.  They are deliberately black-box: the capture pipeline
//! (Game -> CaptureFrame -> Renderer -> PNG) is private to the wolf-cli binary, so
//! each test spawns the freshly built binary and parses its human-readable report.
//!
//! The fixtures live outside the repository (work/all and work/retail_backup); if
//! they are absent the affected test prints a skip notice instead of failing.  PNGs
//! are written to work/out, which is gitignored; only the measured baselines are
//! committed, in docs/research/M11_VISUAL_REGRESSION.md.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use image::GenericImageView;

const SCREEN_W: u32 = 1024;
const SCREEN_H: u32 = 576;
const TOTAL_PIXELS: u64 = (SCREEN_W as u64) * (SCREEN_H as u64);

/// Offscreen wgpu work is not safe to overlap with itself in this harness; the
/// five scenes run serially through one process-wide lock.
static CAPTURE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug)]
struct Baseline {
    name: &'static str,
    args: &'static [&'static str],
    expect_header: &'static str,
    frames: u32,
    peak_coroutines: u32,
    pictures_live: u32,
    textured_quads: u32,
    text_pictures: u32,
    choice_open: bool,
    choice_options: u32,
    atlas_images: u32,
    text_items: u32,
    text_glyphs: u32,
    text_lines: u32,
    nonblack: u64,
    messages: u32,
    choices: u32,
}

#[derive(Debug)]
struct Metrics {
    frames: u32,
    peak_coroutines: u32,
    pictures_live: u32,
    textured_quads: u32,
    text_pictures: u32,
    choice_open: bool,
    choice_options: u32,
    atlas_images: u32,
    text_items: u32,
    text_glyphs: u32,
    text_lines: u32,
    width: u32,
    height: u32,
    nonblack: u64,
    png_nonblack: u64,
    messages: u32,
    choices: u32,
    warnings: u32,
}

const BASELINES: [Baseline; 5] = [
    Baseline {
        name: "title",
        args: &["capture", "--frames", "636"],
        expect_header: "  entry     : boot map (TitleMap.mps event 2)",
        frames: 636,
        peak_coroutines: 6,
        pictures_live: 2,
        textured_quads: 19082,
        text_pictures: 0,
        choice_open: false,
        choice_options: 0,
        atlas_images: 18,
        text_items: 0,
        text_glyphs: 0,
        text_lines: 0,
        nonblack: 538282,
        messages: 0,
        choices: 0,
    },
    Baseline {
        name: "map_idle",
        args: &[
            "capture",
            "--map",
            "SampleMapA",
            "--frames",
            "1200",
            "--answer",
            "0",
            "--answer-delay",
            "1",
        ],
        expect_header: "  map       : MapData/SampleMapA.mps 64x36 tile(s)",
        frames: 1200,
        peak_coroutines: 12,
        pictures_live: 49,
        textured_quads: 27661,
        text_pictures: 33,
        choice_open: false,
        choice_options: 0,
        atlas_images: 29,
        text_items: 33,
        text_glyphs: 76,
        text_lines: 33,
        nonblack: 544394,
        messages: 39,
        choices: 5,
    },
    Baseline {
        name: "day_command",
        args: &["capture", "--event", "251", "--frames", "120"],
        expect_header: "  entry     : common event 251",
        frames: 120,
        peak_coroutines: 6,
        pictures_live: 9,
        textured_quads: 4,
        text_pictures: 3,
        choice_open: false,
        choice_options: 0,
        atlas_images: 2,
        text_items: 3,
        text_glyphs: 13,
        text_lines: 3,
        nonblack: 552254,
        messages: 4,
        choices: 0,
    },
    Baseline {
        name: "battle",
        args: &[
            "capture",
            "--event",
            "253",
            "--frames",
            "600",
            "--answer",
            "0",
            "--answer-delay",
            "1",
        ],
        expect_header: "  entry     : common event 253",
        // M11 task-95 made the rebuilt battle drive default-on, so the 1200-frame frame
        // had advanced past the fight. 600 frames lands on the active battle HUD after
        // the settled multi-round drive settles the first command exchange.
        frames: 600,
        peak_coroutines: 7,
        pictures_live: 52,
        textured_quads: 961,
        text_pictures: 29,
        choice_open: false,
        choice_options: 0,
        atlas_images: 20,
        text_items: 29,
        text_glyphs: 51,
        text_lines: 29,
        nonblack: 524409,
        messages: 26,
        choices: 1,
    },
    Baseline {
        name: "h_overlay",
        args: &[
            "capture",
            "--load-save",
            "work/retail_backup/Save/SaveData08.sav",
            "--frames",
            "60",
        ],
        expect_header: "  entry     : save file work/retail_backup/Save/SaveData08.sav",
        // task-22 (AutoEvents::prime_latched): loading a save no longer fabricates a
        // run_condition==2 edge, so the 7 spurious auto-start coroutines (including the
        // 506 DLC eye overlay at z=-6) are gone. Baseline re-recorded AFTER the fix.
        // Verified deterministic: two consecutive captures produce byte-identical PNGs.
        // Old (pre-fix) values for reference: peak 7 / live 86 / quads 257 / atlas 42 /
        // glyphs 59 / nonblack 527568.
        frames: 60,
        peak_coroutines: 0,
        pictures_live: 84,
        textured_quads: 254,
        text_pictures: 17,
        choice_open: false,
        choice_options: 0,
        atlas_images: 39,
        text_items: 17,
        text_glyphs: 60,
        text_lines: 17,
        nonblack: 527713,
        messages: 0,
        choices: 0,
    },
];

#[test]
fn title_opening_frame_matches_its_baseline() {
    run_baseline(baseline("title"));
}

#[test]
fn story_map_idle_frame_matches_its_baseline() {
    run_baseline(baseline("map_idle"));
}

#[test]
fn day_command_menu_frame_matches_its_baseline() {
    run_baseline(baseline("day_command"));
}

#[test]
fn battle_frame_matches_its_baseline() {
    run_baseline(baseline("battle"));
}

#[test]
fn h_overlay_frame_matches_its_baseline() {
    run_baseline(baseline("h_overlay"));
}

fn baseline(name: &str) -> &'static Baseline {
    BASELINES
        .iter()
        .find(|baseline| baseline.name == name)
        .expect("baseline name must exist")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/wolf-cli must live two levels below the repository root")
        .to_path_buf()
}

fn data_root() -> PathBuf {
    match std::env::var_os("WOLF_DATA_ROOT") {
        Some(value) => {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                repo_root().join(path)
            }
        }
        None => repo_root().join("work/all"),
    }
}

fn save_fixture() -> PathBuf {
    repo_root().join("work/retail_backup/Save/SaveData08.sav")
}

fn run_baseline(baseline: &'static Baseline) {
    let Some(metrics) = capture_scene(baseline) else {
        return;
    };
    assert_scene(baseline, &metrics);
    println!(
        "scene {:12} frames {:4} peak {:2} pictures {:3} quads {:5} textpics {:2} atlas {:2} text {}/{}/{} nonblack {:6}/{} messages {:2} choices {:2} warnings {}",
        baseline.name,
        metrics.frames,
        metrics.peak_coroutines,
        metrics.pictures_live,
        metrics.textured_quads,
        metrics.text_pictures,
        metrics.atlas_images,
        metrics.text_items,
        metrics.text_glyphs,
        metrics.text_lines,
        metrics.nonblack,
        TOTAL_PIXELS,
        metrics.messages,
        metrics.choices,
        metrics.warnings,
    );
}

fn capture_scene(baseline: &'static Baseline) -> Option<Metrics> {
    let root = data_root();
    if !root.is_dir() {
        eprintln!(
            "skipping {}: data root {} is missing (set WOLF_DATA_ROOT)",
            baseline.name,
            root.display()
        );
        return None;
    }
    if baseline.args.iter().any(|arg| *arg == "--load-save") && !save_fixture().is_file() {
        eprintln!(
            "skipping {}: save fixture {} is missing",
            baseline.name,
            save_fixture().display()
        );
        return None;
    }

    let out_dir = repo_root().join("work/out");
    std::fs::create_dir_all(&out_dir).expect("create work/out");
    let png_path = out_dir.join(format!("m11_visual_{}.png", baseline.name));
    let log_path = out_dir.join(format!("m11_visual_{}.log", baseline.name));

    let _guard = CAPTURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let mut command = Command::new(env!("CARGO_BIN_EXE_wolf-cli"));
    command.current_dir(repo_root());
    // The visual baselines are captured with the game's own fonts.  An inherited
    // WOLF_GAME_DIR (e.g. a bare save directory used by the wolf-save tests) has no
    // TTF, which would silently drop all text and change every metric; pin the font
    // source so these pixel-stability fixtures stay independent of that env var.
    command.env_remove("WOLF_GAME_DIR");
    command.arg("--root").arg(&root);
    for arg in baseline.args {
        command.arg(arg);
    }
    command.arg("--out").arg(&png_path);

    let output = command
        .output()
        .unwrap_or_else(|err| panic!("{}: cannot spawn wolf-cli: {err}", baseline.name));
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    std::fs::write(&log_path, format!("{stdout}{stderr}")).expect("write capture log");
    assert!(
        output.status.success(),
        "{}: capture failed with status {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        baseline.name,
        output.status
    );
    assert!(
        stdout.contains(baseline.expect_header),
        "{}: capture output did not contain header {:?}\n--- stdout ---\n{stdout}",
        baseline.name,
        baseline.expect_header
    );

    let metrics = parse_metrics(baseline.name, &stdout);
    let png = image::open(&png_path).unwrap_or_else(|err| {
        panic!(
            "{}: cannot decode {}: {err}",
            baseline.name,
            png_path.display()
        )
    });
    let (width, height) = png.dimensions();
    let rgba = png.to_rgba8();
    let png_nonblack = rgba
        .pixels()
        .filter(|pixel| pixel[0] | pixel[1] | pixel[2] != 0)
        .count() as u64;

    Some(Metrics {
        width,
        height,
        png_nonblack,
        ..metrics
    })
}

fn assert_scene(baseline: &Baseline, metrics: &Metrics) {
    assert_eq!(
        (metrics.width, metrics.height),
        (SCREEN_W, SCREEN_H),
        "{}: PNG geometry",
        baseline.name
    );
    assert_eq!(
        metrics.nonblack, baseline.nonblack,
        "{}: stdout non-black pixels",
        baseline.name
    );
    assert_eq!(
        metrics.nonblack, metrics.png_nonblack,
        "{}: decoded PNG non-black pixels must equal the report",
        baseline.name
    );
    assert_eq!(metrics.frames, baseline.frames, "{}: frames", baseline.name);
    assert_eq!(
        metrics.peak_coroutines, baseline.peak_coroutines,
        "{}: peak coroutines",
        baseline.name
    );
    assert_eq!(
        metrics.pictures_live, baseline.pictures_live,
        "{}: live pictures",
        baseline.name
    );
    assert_eq!(
        metrics.textured_quads, baseline.textured_quads,
        "{}: textured quads",
        baseline.name
    );
    assert_eq!(
        metrics.text_pictures, baseline.text_pictures,
        "{}: text pictures",
        baseline.name
    );
    assert_eq!(
        metrics.choice_open, baseline.choice_open,
        "{}: choice open state",
        baseline.name
    );
    assert_eq!(
        metrics.choice_options, baseline.choice_options,
        "{}: choice options",
        baseline.name
    );
    assert_eq!(
        metrics.atlas_images, baseline.atlas_images,
        "{}: atlas images",
        baseline.name
    );
    assert_eq!(
        metrics.text_items, baseline.text_items,
        "{}: text items",
        baseline.name
    );
    assert_eq!(
        metrics.text_glyphs, baseline.text_glyphs,
        "{}: text glyphs",
        baseline.name
    );
    assert_eq!(
        metrics.text_lines, baseline.text_lines,
        "{}: text lines",
        baseline.name
    );
    assert_eq!(
        metrics.messages, baseline.messages,
        "{}: message ops",
        baseline.name
    );
    assert_eq!(
        metrics.choices, baseline.choices,
        "{}: choice ops",
        baseline.name
    );
    assert_eq!(
        metrics.warnings, 0,
        "{}: warnings must be zero",
        baseline.name
    );
}

fn parse_metrics(name: &str, stdout: &str) -> Metrics {
    let frames_line = field_line(stdout, "frames")
        .unwrap_or_else(|| panic!("{name}: no frames line in capture output"));
    let frames = parse_u32_before(frames_line, "(peak")
        .unwrap_or_else(|| panic!("{name}: cannot parse frames from {frames_line:?}"));
    let peak_coroutines = parse_u32_before(frames_line, "coroutine(s))")
        .unwrap_or_else(|| panic!("{name}: cannot parse coroutine peak from {frames_line:?}"));

    let pictures_line = field_line(stdout, "pictures")
        .unwrap_or_else(|| panic!("{name}: no pictures line in capture output"));
    let pictures_live = parse_u32_before(pictures_line, "live")
        .unwrap_or_else(|| panic!("{name}: cannot parse live pictures from {pictures_line:?}"));
    let textured_quads = parse_u32_before(pictures_line, "textured")
        .unwrap_or_else(|| panic!("{name}: cannot parse textured quads from {pictures_line:?}"));
    let text_pictures = parse_u32_before(pictures_line, "text")
        .unwrap_or_else(|| panic!("{name}: cannot parse text pictures from {pictures_line:?}"));

    let choice_line = field_line(stdout, "choice")
        .unwrap_or_else(|| panic!("{name}: no choice line in capture output"));
    let (choice_open, choice_options) = if choice_line.contains("none open") {
        (false, 0)
    } else {
        let options = parse_u32_before(choice_line, "option(s)")
            .unwrap_or_else(|| panic!("{name}: cannot parse choice options from {choice_line:?}"));
        (true, options)
    };

    let atlas_line = field_line(stdout, "atlas")
        .unwrap_or_else(|| panic!("{name}: no atlas line in capture output"));
    let atlas_images = parse_u32_before(atlas_line, "image(s),")
        .unwrap_or_else(|| panic!("{name}: cannot parse atlas images from {atlas_line:?}"));

    let text_line = field_line(stdout, "text")
        .unwrap_or_else(|| panic!("{name}: no text line in capture output"));
    let text_items = parse_u32_before(text_line, "item(s),")
        .unwrap_or_else(|| panic!("{name}: cannot parse text items from {text_line:?}"));
    let text_glyphs = parse_u32_before(text_line, "glyph(s),")
        .unwrap_or_else(|| panic!("{name}: cannot parse text glyphs from {text_line:?}"));
    let text_lines = parse_u32_before(text_line, "line(s),")
        .unwrap_or_else(|| panic!("{name}: cannot parse text lines from {text_line:?}"));

    let rendered_line = field_line(stdout, "rendered")
        .unwrap_or_else(|| panic!("{name}: no rendered line in capture output"));
    let mut rendered_tokens = rendered_line.split_whitespace();
    assert_eq!(
        rendered_tokens.nth(2),
        Some("1024x576,"),
        "{name}: rendered line geometry: {rendered_line:?}"
    );
    let fraction = rendered_tokens
        .find(|token| token.contains('/'))
        .unwrap_or_else(|| panic!("{name}: no non-black fraction in {rendered_line:?}"));
    let (lit, total) = fraction
        .split_once('/')
        .unwrap_or_else(|| panic!("{name}: malformed non-black fraction {fraction:?}"));
    let nonblack = lit
        .parse::<u64>()
        .unwrap_or_else(|_| panic!("{name}: malformed lit pixel count {lit:?}"));
    let reported_total = total
        .parse::<u64>()
        .unwrap_or_else(|_| panic!("{name}: malformed total pixel count {total:?}"));
    assert_eq!(
        reported_total, TOTAL_PIXELS,
        "{name}: capture total pixels must be 1024x576"
    );

    let stats_line = field_line(stdout, "stats")
        .unwrap_or_else(|| panic!("{name}: no stats line in capture output"));
    let messages = parse_u32_before(stats_line, "message(s),")
        .unwrap_or_else(|| panic!("{name}: cannot parse message ops from {stats_line:?}"));
    let choices = parse_u32_before(stats_line, "choice(s),")
        .unwrap_or_else(|| panic!("{name}: cannot parse choice ops from {stats_line:?}"));
    let warnings = parse_u32_before(stats_line, "warning(s)")
        .unwrap_or_else(|| panic!("{name}: cannot parse warnings from {stats_line:?}"));

    Metrics {
        frames,
        peak_coroutines,
        pictures_live,
        textured_quads,
        text_pictures,
        choice_open,
        choice_options,
        atlas_images,
        text_items,
        text_glyphs,
        text_lines,
        width: 0,
        height: 0,
        nonblack,
        png_nonblack: 0,
        messages,
        choices,
        warnings,
    }
}

fn field_line<'a>(stdout: &'a str, field: &str) -> Option<&'a str> {
    stdout.lines().find(|line| {
        let Some((name, _)) = line.split_once(':') else {
            return false;
        };
        name.starts_with("  ") && name.trim() == field
    })
}

fn parse_u32_before(line: &str, marker: &str) -> Option<u32> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let index = tokens.iter().position(|token| *token == marker)?;
    tokens.get(index.checked_sub(1)?)?.parse().ok()
}
