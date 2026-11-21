//! `wolf-layout` — dump a screen/event's script-driven UI layout.
//!
//! In WOLF RPG Editor the UI is data, not engine code: every status bar, clock and
//! message box is a sequence of `150 Picture`, `101 Message` and `102 Choices`
//! instructions whose arguments already carry the exact x/y/scale/opacity/anchor.
//! This tool prints those arguments as a table so a screen can be laid out from the
//! data instead of eyeballed against a screenshot. See docs/research/UI_LAYOUT.md.
//!
//! Usage:
//!   wolf-layout --root <DATA_ROOT> --event <COMMON_EVENT_ID>
//!   wolf-layout --root <DATA_ROOT> --map <MapName> --event <ID> [--page N]

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use wolf_format::command::Slot;
use wolf_format::commonevent::CommonEventData;
use wolf_format::mps::Map;

struct Args {
    root: String,
    map: Option<String>,
    event: u32,
    page: Option<usize>,
}

fn parse_args() -> Args {
    let mut a = Args {
        root: "work/all".into(),
        map: None,
        event: 216,
        page: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--root" | "-r" => a.root = it.next().unwrap_or_default(),
            "--map" | "-m" => a.map = it.next(),
            "--event" | "-e" => a.event = it.next().and_then(|s| s.parse().ok()).unwrap_or(216),
            "--page" | "-p" => a.page = it.next().and_then(|s| s.parse().ok()),
            other => eprintln!("ignoring unknown argument {other:?}"),
        }
    }
    a
}

fn head_decode(head: i32) -> (u8, u8, u8) {
    let ty = ((head >> 4) & 0x7) as u8;
    let cmd = (head & 0xF) as u8;
    let anchor = ((head & 0xF000) >> 12) as u8;
    (ty, cmd, anchor)
}

fn tag(kind: u8) -> &'static str {
    match kind {
        0 => "file",
        1 => "fileStr",
        2 => "text",
        3 => "winFile",
        4 => "winStr",
        _ => "?",
    }
}

fn dump_pictures(commands: &[Slot]) {
    println!(
        "{:<5} {:<8} {:<4} {:>7} {:>5} {:>5} {:>8} {:>5} {:>5} {:>7} {:>5} {:>6}  text",
        "pc", "kind", "cmd", "#pic", "boxW", "boxH", "opacity", "x", "y", "scale", "anch", "blend"
    );
    for (pc, slot) in commands.iter().enumerate() {
        let Slot::Command(cmd) = slot else { continue };
        match cmd.opcode {
            150 => {
                let ints: Vec<i32> = cmd
                    .params
                    .chunks_exact(4)
                    .map(|w| i32::from_le_bytes([w[0], w[1], w[2], w[3]]))
                    .collect();
                if ints.len() < 10 {
                    continue;
                }
                let (ty, cn, anchor) = head_decode(ints[0]);
                let text = cmd.strings.first().cloned().unwrap_or_default();
                println!(
                    "{:<5} {:<8} {:<4} {:>7} {:>5} {:>5} {:>8} {:>5} {:>5} {:>7} {:>5} {:>6}  {:?}",
                    pc,
                    tag(ty),
                    cn,
                    ints[1],
                    ints[3],
                    ints[4],
                    ints[6],
                    ints[7],
                    ints[8],
                    ints[9],
                    anchor,
                    ints[10],
                    text
                );
            }
            101 => {
                let text = cmd.strings.first().cloned().unwrap_or_default();
                println!("pc{pc:<4} message  {:?}", text);
            }
            _ => {}
        }
    }
}

fn load_common(root: &Path, id: u32) -> Option<Vec<Slot>> {
    let path = root.join("BasicData").join("CommonEvent.dat");
    let bytes = fs::read(&path).ok()?;
    let data = CommonEventData::parse(&bytes).ok()?;
    let ev = data.events.into_iter().find(|e| e.id == id)?;
    Some(ev.commands)
}

fn load_map(root: &Path, name: &str, id: u32, page: Option<usize>) -> Option<Vec<Slot>> {
    let cleaned = name.replace(std::path::MAIN_SEPARATOR, "/");
    let relative = if cleaned.contains('/') || cleaned.ends_with(".mps") {
        cleaned
    } else {
        format!("MapData/{cleaned}.mps")
    };
    let bytes = fs::read(root.join(&relative)).ok()?;
    let map = Map::parse(&bytes).ok()?;
    let ev = map.events.iter().find(|e| e.id == id)?;
    match page {
        Some(p) => ev.pages.get(p).map(|pg| pg.commands.clone()),
        None => ev.pages.first().map(|pg| pg.commands.clone()),
    }
}

fn main() -> ExitCode {
    let args = parse_args();
    let root = Path::new(&args.root);
    let commands = match &args.map {
        Some(m) => load_map(root, m, args.event, args.page),
        None => load_common(root, args.event),
    };
    let Some(commands) = commands else {
        eprintln!("cannot find the requested event under {}", root.display());
        return ExitCode::FAILURE;
    };
    let title = match &args.map {
        Some(m) => format!("map {m} event {} page {:?}", args.event, args.page),
        None => format!("common event {}", args.event),
    };
    println!("{title}");
    dump_pictures(&commands);
    ExitCode::SUCCESS
}
