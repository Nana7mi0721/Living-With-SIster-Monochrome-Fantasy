//! The map event runtime: which pages run, and when.
//!
//! WOLF starts a page because its TRIGGER says so, not because the player walked
//! into it. The triggers observed in this game (73 events / 134 pages) are
//! 0 = action key (14 pages), 1 = autorun (26), 2 = parallel (90), 3 = parallel
//! while an event is running (4). A parallel page is a permanent coroutine: when
//! its command stream ends the interpreter drops the instance and WOLF starts the
//! page again on the next frame, which is exactly how the click-detection events
//! in SampleMapA stay alive for the whole scene.
//!
//! Page SELECTION (the last page whose conditions hold) is not modelled yet: the
//! condition words live in `MapEventPage::trigger_switches`, `trigger_variables`
//! and `trigger_values`, and evaluating them needs the live switch/variable
//! banks. Until then this module picks the last page with a non-empty stream,
//! which is the state the game is in when a scene starts.

use std::sync::Arc;

use wolf_format::mps::Map;
use wolf_vm::exec::InstanceId;
use wolf_vm::{Program, Vm};

/// What a page's trigger tells the runtime to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// Runs when the player presses the action key while facing the event.
    ActionKey,
    /// Runs once as soon as the map is entered.
    Autorun,
    /// Runs as a coroutine that restarts every time it ends.
    Parallel,
    /// Parallel, but only while some other event is running.
    ParallelWhileRunning,
    /// A trigger byte no known WOLF build uses; kept so a real file never panics.
    Unknown(u8),
}

impl Trigger {
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0 => Trigger::ActionKey,
            1 => Trigger::Autorun,
            2 => Trigger::Parallel,
            3 => Trigger::ParallelWhileRunning,
            other => Trigger::Unknown(other),
        }
    }

    /// True when the runtime starts the page by itself.
    pub fn is_automatic(self) -> bool {
        matches!(
            self,
            Trigger::Autorun
                | Trigger::Parallel
                | Trigger::ParallelWhileRunning
                | Trigger::Unknown(_)
        )
    }

    /// True when the page is restarted after its stream ends.
    pub fn is_looping(self) -> bool {
        matches!(self, Trigger::Parallel | Trigger::ParallelWhileRunning)
    }
}

/// One page of one map event, plus the interpreter instance running it.
pub struct PageSlot {
    pub event: u32,
    pub name: String,
    pub page: usize,
    pub trigger: Trigger,
    pub program: Arc<Program>,
    instance: Option<InstanceId>,
    /// How many times the page was (re)started; parallel pages grow forever.
    pub starts: u64,
}

impl PageSlot {
    pub fn instance(&self) -> Option<InstanceId> {
        self.instance
    }
}

/// Every automatic page of one map, and the rules that keep them alive.
#[derive(Default)]
pub struct MapRuntime {
    slots: Vec<PageSlot>,
    /// Total starts across all slots, i.e. how much the map is doing.
    pub restarts: u64,
}

impl MapRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    /// Collect the automatic pages of `map`.
    ///
    /// Pages with an empty command stream are skipped: WOLF treats them as
    /// blank pages, and starting them would burn an instance slot for nothing.
    pub fn collect(map: &Map) -> Self {
        let mut slots = Vec::new();
        for event in &map.events {
            for (index, page) in event.pages.iter().enumerate() {
                let trigger = Trigger::from_byte(page.trigger);
                if !trigger.is_automatic() || page.commands.is_empty() {
                    continue;
                }
                let program = Program::from_slots(event.id, &event.name, page.commands.clone());
                slots.push(PageSlot {
                    event: event.id,
                    name: event.name.clone(),
                    page: index,
                    trigger,
                    program: Arc::new(program),
                    instance: None,
                    starts: 0,
                });
            }
        }
        Self { slots, restarts: 0 }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn slots(&self) -> &[PageSlot] {
        &self.slots
    }

    /// Which triggers are present, for diagnostics.
    pub fn trigger_counts(&self) -> std::collections::BTreeMap<u8, usize> {
        let mut counts = std::collections::BTreeMap::new();
        for slot in &self.slots {
            let key = match slot.trigger {
                Trigger::ActionKey => 0,
                Trigger::Autorun => 1,
                Trigger::Parallel => 2,
                Trigger::ParallelWhileRunning => 3,
                Trigger::Unknown(other) => other,
            };
            *counts.entry(key).or_insert(0) += 1;
        }
        counts
    }

    /// Start every automatic page that is not running yet.
    ///
    /// Returns how many were started. An autorun page starts once; a parallel
    /// page is started again whenever its instance is gone, which is what makes
    /// it a loop.
    pub fn start_pending(&mut self, vm: &mut Vm) -> usize {
        let mut started = 0;
        for slot in &mut self.slots {
            let running = slot.instance.map(|id| vm.instance(id).is_some()).unwrap_or(false);
            if running {
                slot.instance = slot.instance.filter(|id| vm.instance(*id).is_some());
                continue;
            }
            slot.instance = None;
            if slot.starts > 0 && !slot.trigger.is_looping() {
                continue;
            }
            if let Some(id) = vm.start_program(Arc::clone(&slot.program)) {
                slot.instance = Some(id);
                slot.starts += 1;
                self.restarts += 1;
                started += 1;
            }
        }
        started
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wolf_format::command::Slot;
    use wolf_format::mps::{MapEvent, MapEventPage};

    fn page(trigger: u8, commands: Vec<Slot>) -> MapEventPage {
        MapEventPage {
            graphic_chip_id: 0,
            graphic: String::new(),
            direction: 2,
            frame: 0,
            opacity: 255,
            display_type: 0,
            trigger,
            trigger_switches: [0; 4],
            trigger_variables: [0; 4],
            trigger_values: [0; 4],
            route: Default::default(),
            commands,
            features: 3,
            shadow_graphic: 0,
            collision_width: 0,
            collision_height: 0,
            footer: 0x7A,
        }
    }

    #[test]
    fn triggers_map_onto_the_four_modes() {
        assert_eq!(Trigger::from_byte(0), Trigger::ActionKey);
        assert_eq!(Trigger::from_byte(1), Trigger::Autorun);
        assert_eq!(Trigger::from_byte(2), Trigger::Parallel);
        assert_eq!(Trigger::from_byte(3), Trigger::ParallelWhileRunning);
        assert_eq!(Trigger::from_byte(9), Trigger::Unknown(9));
        assert!(!Trigger::from_byte(0).is_automatic());
        assert!(Trigger::from_byte(1).is_automatic());
        assert!(Trigger::from_byte(2).is_looping());
        assert!(!Trigger::from_byte(1).is_looping());
    }

    #[test]
    fn only_automatic_pages_are_collected() {
        let map = Map {
            version_header: 0,
            version: 0,
            name: String::new(),
            tileset_index: 0,
            width: 4,
            height: 4,
            has_tile_data: false,
            layers: Vec::new(),
            events: vec![MapEvent {
                id: 7,
                name: "door".to_owned(),
                x: 1,
                y: 1,
                pages: vec![page(0, Vec::new()), page(2, vec![Slot::Blank]), page(1, Vec::new())],
            }],
            footer: 0x66,
        };
        let runtime = MapRuntime::collect(&map);
        // page 0 is the action key (not automatic), page 1 is parallel but
        // empty, page 2 is parallel with a command.
        assert_eq!(runtime.len(), 1);
        assert_eq!(runtime.trigger_counts().get(&2), Some(&1));
        assert_eq!(runtime.slots()[0].event, 7);
        assert_eq!(runtime.slots()[0].page, 1);
    }
}
