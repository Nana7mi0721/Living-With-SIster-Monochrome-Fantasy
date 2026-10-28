//! The map runtime against the real game data.
//!
//! Skipped when the unpacked data is absent, like every other integration test.

use std::path::PathBuf;

use wolf_format::mps::Map;
use wolf_game::map_runtime::{MapRuntime, Trigger};
use wolf_vm::{Host, ProgramSet, Vm};

fn data_root() -> PathBuf {
    let raw = std::env::var("WOLF_DATA_ROOT").unwrap_or_else(|_| "work/full".to_owned());
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        base.pop();
        base.pop();
        base.join(path)
    }
}

fn read_map(name: &str) -> Option<Map> {
    let bytes = std::fs::read(data_root().join("MapData").join(name)).ok()?;
    Map::parse(&bytes).ok()
}

#[test]
fn samplemapa_starts_ninety_of_its_pages() {
    let Some(map) = read_map("SampleMapA.mps") else {
        eprintln!("skipping: no unpacked data");
        return;
    };
    let runtime = MapRuntime::collect(&map);
    let counts = runtime.trigger_counts();
    println!("map {}x{} events {}", map.width, map.height, map.events.len());
    println!("automatic pages {} triggers {:?}", runtime.len(), counts);
    assert!(runtime.len() > 20, "only {} automatic pages", runtime.len());
    for slot in runtime.slots() {
        assert!(slot.trigger.is_automatic());
        assert_ne!(slot.trigger, Trigger::ActionKey);
        assert!(!slot.program.is_empty());
    }
}

#[test]
fn parallel_pages_restart_when_their_stream_ends() {
    let Some(map) = read_map("SampleMapA.mps") else {
        eprintln!("skipping: no unpacked data");
        return;
    };
    let mut runtime = MapRuntime::collect(&map);
    let mut vm = Vm::new(ProgramSet::default());
    let mut host = Quiet;
    let started = runtime.start_pending(&mut vm);
    assert_eq!(started, runtime.len());
    let mut instructions = 0usize;
    for _ in 0..120 {
        runtime.start_pending(&mut vm);
        let report = vm.step_frame(&mut host);
        instructions += report.steps;
    }
    println!(
        "frames 120 instructions {instructions} starts {} slots {}",
        runtime.restarts,
        runtime.len()
    );
    assert!(runtime.restarts > runtime.len() as u64, "parallel pages never restarted");
    assert!(instructions > 0);
}

struct Quiet;

impl Host for Quiet {}
