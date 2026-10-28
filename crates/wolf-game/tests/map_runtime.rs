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
fn samplemapa_selects_one_page_per_event() {
    let Some(map) = read_map("SampleMapA.mps") else {
        eprintln!("skipping: no unpacked data");
        return;
    };
    let mut runtime = MapRuntime::collect(&map);
    let mut vm = Vm::new(ProgramSet::default());
    let started = runtime.start_pending(&mut vm);
    println!(
        "map {}x{} events {} tracked {}",
        map.width,
        map.height,
        map.events.len(),
        runtime.event_count()
    );
    println!(
        "active {} of {} tracked, triggers {:?}, started {started}",
        runtime.len(),
        runtime.event_count(),
        runtime.trigger_counts()
    );
    // Page selection is one page per event, so the count can never exceed the
    // number of events the map has.
    assert!(runtime.len() <= map.events.len());
    for slot in runtime.slots() {
        println!("  event {:2} page {} trigger {:?} slots {}", slot.event, slot.page, slot.trigger, slot.program.len());
    }
    assert!(!runtime.is_empty(), "no active event at all");
    assert_eq!(started, runtime.len(), "every active page started once");
    for slot in runtime.slots() {
        assert!(slot.trigger.is_automatic(), "{:?} is not automatic", slot.trigger);
        assert_ne!(slot.trigger, Trigger::ActionKey);
        assert!(!slot.program.is_empty());
    }
}

#[test]
fn a_variable_write_switches_the_active_page() {
    let Some(map) = read_map("SampleMapA.mps") else {
        eprintln!("skipping: no unpacked data");
        return;
    };
    // Event 9 is the measured ten page state machine: page i is selected by
    // global variable 2000062 holding i + 1, event 10 continues at 11.
    let Some(event) = map.events.iter().find(|event| event.id == 9) else {
        eprintln!("skipping: SampleMapA has no event 9");
        return;
    };
    assert_eq!(event.pages.len(), 10, "event 9 lost pages");

    let mut runtime = MapRuntime::collect(&map);
    let mut vm = Vm::new(ProgramSet::default());
    let mut host = Quiet;
    // Global variable 62 = the variable half of magic 2000062.
    for (value, expected) in [(1i32, 0usize), (5, 4), (10, 9)] {
        vm.vars.set(62, value);
        let report = vm.step_frame(&mut host);
        let _ = report;
        runtime.start_pending(&mut vm);
        let page = runtime
            .active_pages(&vm)
            .into_iter()
            .find(|(id, _)| *id == 9)
            .and_then(|(_, page)| page);
        println!("global 62 = {value} -> event 9 page {page:?}");
        assert_eq!(page, Some(expected), "global 62 = {value}");
    }
    // Out of range still matches the highest page whose condition holds, which
    // is page 0 only if page 0 has no condition; event 9 page 0 wants 1.
    vm.vars.set(62, 99);
    runtime.start_pending(&mut vm);
    let page = runtime
        .active_pages(&vm)
        .into_iter()
        .find(|(id, _)| *id == 9)
        .and_then(|(_, page)| page);
    println!("global 62 = 99 -> event 9 page {page:?} switches {}", runtime.switches);
    assert_eq!(page, None, "no page of event 9 asks for 99");
}

#[test]
fn parallel_pages_restart_when_their_stream_ends() {
    let Some(map) = read_map("SampleMapA.mps") else {
        eprintln!("skipping: no unpacked data");
        return;
    };
    let mut runtime = MapRuntime::collect_except(&map, Some(0));
    let mut vm = Vm::new(ProgramSet::default());
    let mut host = Quiet;
    runtime.start_pending(&mut vm);
    assert_eq!(runtime.trigger_counts().get(&1), None, "autorun page still collected");
    let mut instructions = 0usize;
    for _ in 0..120 {
        runtime.start_pending(&mut vm);
        let report = vm.step_frame(&mut host);
        instructions += report.steps;
    }
    println!(
        "frames 120 instructions {instructions} starts {} switches {} active {}",
        runtime.restarts,
        runtime.switches,
        runtime.len()
    );
    assert!(
        runtime.restarts > runtime.len() as u64,
        "parallel pages never restarted"
    );
    assert!(instructions > 0);
}

struct Quiet;

impl Host for Quiet {}
