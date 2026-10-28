//! The map event runtime: which page of which event runs, and when.
//!
//! WOLF does not run an event because the player walked into it. It picks, per
//! event, the page whose activation conditions hold, and then obeys that page's
//! trigger: 0 action key, 1 autorun, 2 parallel, 3 player touch, 4 event touch.
//! docs/FORMAT_SPEC.md §5.6.3 carries the measurement: the four condition slots
//! are variable comparisons (the trigger_switch bytes are NOT switch ids), and
//! the official manual is explicit that the HIGHEST page whose conditions hold
//! is the one that runs. Page 0 is no fallback - it usually carries conditions
//! of its own - so an event whose conditions all fail is simply not on the map.
//!
//! Two consequences shape this module:
//!
//! * Selection must be re-evaluated every frame, because a page's own commands
//!   write the very variables that select the next page (SampleMapA event 9 is a
//!   ten page state machine driven by global variable 2000062).
//! * A parallel page is a permanent coroutine: when its command stream ends the
//!   interpreter drops the instance and the page is started again next frame,
//!   which is how the click-detection events in SampleMapA stay alive.
//!
//! Known gap (M3): WOLF keeps an event's self variables across page changes,
//! while this runtime starts every page with a fresh self bank. Only one shipped
//! page condition reads a self variable (SampleMapA event 0 page 0, self[0] must
//! equal 0) and the default 0 makes it hold, so the gap is invisible today.

use std::collections::BTreeMap;
use std::sync::Arc;

use wolf_format::mps::{Map, PageCondition, CONDITION_SLOTS};
use wolf_vm::exec::InstanceId;
use wolf_vm::{Magic, MagicKind, Program, SelfVars, Variables, Vm};

/// What a page's trigger byte tells the runtime to do.
///
/// Measured over the 134 shipped pages: 0 appears 14 times, 1 26 times, 2 90
/// times and 3 four times (docs/FORMAT_SPEC.md §5.6). Trigger 4 exists in the
/// engine but not in this game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// Runs when the player presses the action key while facing the event.
    ActionKey,
    /// Runs once as soon as its page becomes the active one.
    Autorun,
    /// Runs as a coroutine that restarts every time its stream ends.
    Parallel,
    /// Runs when the player touches the event's tile.
    PlayerTouch,
    /// A trigger byte no known WOLF build uses; kept so a real file never panics.
    Unknown(u8),
}

impl Trigger {
    #[must_use]
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0 => Trigger::ActionKey,
            1 => Trigger::Autorun,
            2 => Trigger::Parallel,
            3 => Trigger::PlayerTouch,
            other => Trigger::Unknown(other),
        }
    }

    /// The byte this trigger came from, for diagnostics.
    #[must_use]
    pub fn byte(self) -> u8 {
        match self {
            Trigger::ActionKey => 0,
            Trigger::Autorun => 1,
            Trigger::Parallel => 2,
            Trigger::PlayerTouch => 3,
            Trigger::Unknown(other) => other,
        }
    }

    /// True when the runtime starts the page on its own.
    ///
    /// Action key and player touch both need a player, so they stay dormant
    /// until an input or movement layer exists (M2 has neither: the game is a
    /// point-and-click UI whose clicks go through opcode 124, not through map
    /// events).
    #[must_use]
    pub fn is_automatic(self) -> bool {
        matches!(self, Trigger::Autorun | Trigger::Parallel | Trigger::Unknown(_))
    }

    /// True when the page is restarted after its stream ends.
    #[must_use]
    pub fn is_looping(self) -> bool {
        matches!(self, Trigger::Parallel)
    }
}

/// One page of a map event, precompiled and with its conditions decoded.
struct PageState {
    trigger: Trigger,
    conditions: [PageCondition; CONDITION_SLOTS],
    /// None for a page with an empty command stream (blank page).
    program: Option<Arc<Program>>,
}

/// One map event: its pages, and which page is active right now.
struct EventState {
    id: u32,
    name: String,
    pages: Vec<PageState>,
    /// Index of the active page, None when no page's conditions hold.
    active: Option<usize>,
    instance: Option<InstanceId>,
    /// Starts over the whole lifetime, for diagnostics.
    starts: u64,
    /// Starts of the CURRENT page; an autorun page runs once per activation.
    starts_on_page: u64,
}

/// The public view of one running map event.
pub struct PageSlot {
    pub event: u32,
    pub name: String,
    pub page: usize,
    pub trigger: Trigger,
    pub program: Arc<Program>,
    instance: Option<InstanceId>,
    /// How many times this event's page was (re)started; parallel pages grow.
    pub starts: u64,
}

impl PageSlot {
    #[must_use]
    pub fn instance(&self) -> Option<InstanceId> {
        self.instance
    }
}

/// Every event of one map, plus the rules that keep their pages alive.
#[derive(Default)]
pub struct MapRuntime {
    events: Vec<EventState>,
    /// Total starts across all events, i.e. how much the map is doing.
    pub restarts: u64,
    /// How many times an event switched to a different page.
    pub switches: u64,
}

impl MapRuntime {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Collect the pages of every event of a map.
    #[must_use]
    pub fn collect(map: &Map) -> Self {
        Self::collect_except(map, None)
    }

    /// Same, but never collects a skipped event (the boot event is started by
    /// the caller, and collecting it too would run the title script twice).
    ///
    /// Events whose every page has an empty command stream are dropped: WOLF
    /// treats them as blank, and keeping them would burn an instance slot per
    /// frame for nothing.
    #[must_use]
    pub fn collect_except(map: &Map, skip: Option<u32>) -> Self {
        let mut events = Vec::new();
        for event in &map.events {
            if Some(event.id) == skip {
                continue;
            }
            if event.pages.iter().all(|page| page.commands.is_empty()) {
                continue;
            }
            let pages = event
                .pages
                .iter()
                .map(|page| {
                    let program = if page.commands.is_empty() {
                        None
                    } else {
                        let program =
                            Program::from_slots(event.id, &event.name, page.commands.clone());
                        Some(Arc::new(program))
                    };
                    PageState {
                        trigger: Trigger::from_byte(page.trigger),
                        conditions: page.conditions(),
                        program,
                    }
                })
                .collect();
            events.push(EventState {
                id: event.id,
                name: event.name.clone(),
                pages,
                active: None,
                instance: None,
                starts: 0,
                starts_on_page: 0,
            });
        }
        Self {
            events,
            restarts: 0,
            switches: 0,
        }
    }

    /// How many events are tracked (not all of them are active at once).
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// How many events have an active page with a command stream right now.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events
            .iter()
            .filter(|state| {
                state.active.is_some_and(|index| {
                    state
                        .pages
                        .get(index)
                        .is_some_and(|page| page.program.is_some())
                })
            })
            .count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// One slot per active event, in map file order.
    #[must_use]
    pub fn slots(&self) -> Vec<PageSlot> {
        self.events
            .iter()
            .filter_map(|state| {
                let index = state.active?;
                let page = state.pages.get(index)?;
                let program = page.program.as_ref()?;
                Some(PageSlot {
                    event: state.id,
                    name: state.name.clone(),
                    page: index,
                    trigger: page.trigger,
                    program: Arc::clone(program),
                    instance: state.instance,
                    starts: state.starts,
                })
            })
            .collect()
    }

    /// Which triggers the active pages carry, for diagnostics.
    #[must_use]
    pub fn trigger_counts(&self) -> BTreeMap<u8, usize> {
        let mut counts = BTreeMap::new();
        for slot in self.slots() {
            *counts.entry(slot.trigger.byte()).or_insert(0) += 1;
        }
        counts
    }

    /// The page each tracked event would run with the current variables.
    ///
    /// Read-only, so a test or a trace can ask the question without starting
    /// anything. Self variables are taken from the running instance, which is
    /// the closest thing to WOLF's persistent per-event self bank we have.
    #[must_use]
    pub fn active_pages(&self, vm: &Vm) -> Vec<(u32, Option<usize>)> {
        self.events
            .iter()
            .map(|state| (state.id, select_page(state, vm)))
            .collect()
    }


    /// Re-select every page, swap the coroutines of the pages that changed, and
    /// start whatever should be running. Returns how many were started.
    ///
    /// This is the whole per-frame contract: call it once before stepping the
    /// VM, every frame, because a page can switch itself by writing a variable
    /// that its own page selection then reads.
    pub fn start_pending(&mut self, vm: &mut Vm) -> usize {
        // Phase 1 reads the VM (variables and self banks); phase 2 mutates it.
        // Splitting them keeps the borrow checker happy without cloning the VM.
        let mut wanted: Vec<Option<usize>> = Vec::with_capacity(self.events.len());
        for state in &self.events {
            wanted.push(select_page(state, vm));
        }

        let mut started = 0;
        for (state, want) in self.events.iter_mut().zip(wanted) {
            if state.active != want {
                if let Some(id) = state.instance.take() {
                    vm.stop_instance(id);
                }
                if state.active.is_some() {
                    self.switches += 1;
                }
                state.active = want;
                state.starts_on_page = 0;
            }
            let Some(index) = state.active else {
                continue;
            };
            let Some(page) = state.pages.get(index) else {
                state.active = None;
                continue;
            };
            let trigger = page.trigger;
            let program = page.program.as_ref().map(Arc::clone);
            // Still running? Then there is nothing to do for this event.
            if let Some(id) = state.instance {
                if vm.instance(id).is_some() {
                    continue;
                }
                state.instance = None;
            }
            if !trigger.is_automatic() {
                continue;
            }
            let Some(program) = program else {
                continue;
            };
            if state.starts_on_page > 0 && !trigger.is_looping() {
                continue;
            }
            if let Some(id) = vm.start_program(program) {
                state.instance = Some(id);
                state.starts += 1;
                state.starts_on_page += 1;
                self.restarts += 1;
                started += 1;
            }
        }
        started
    }
}

/// The page WOLF would display for this event right now.
fn select_page(state: &EventState, vm: &Vm) -> Option<usize> {
    let self_vars = state
        .instance
        .and_then(|id| vm.instance(id))
        .map(|instance| instance.self_vars.clone());
    (0..state.pages.len())
        .rev()
        .find(|index| conditions_met(&state.pages[*index].conditions, self_vars.as_ref(), &vm.vars))
}

/// True when every enabled condition holds.
fn conditions_met(
    conditions: &[PageCondition; CONDITION_SLOTS],
    self_vars: Option<&SelfVars>,
    vars: &Variables,
) -> bool {
    conditions.iter().all(|condition| {
        !condition.enabled
            || condition
                .compare(condition_value(condition.variable, self_vars, vars))
                .unwrap_or(false)
    })
}

/// Resolve a WOLF magic variable number against the live banks.
///
/// A page condition may address any bank. The shipped pages use globals
/// (2000062 in the SampleMapA state machines) and exactly one self variable.
fn condition_value(magic: u32, self_vars: Option<&SelfVars>, vars: &Variables) -> i32 {
    let magic = Magic::decode(magic as i32);
    match magic.kind() {
        MagicKind::Var => vars.get(magic.index() as usize),
        MagicKind::SysVar => vars.get_sys(magic.index() as usize),
        MagicKind::SelfVar => self_vars
            .map(|self_vars| self_vars.get(magic.index() as usize))
            .unwrap_or(0),
        MagicKind::StrVar | MagicKind::Literal | MagicKind::Opaque => magic.raw(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use wolf_format::command::Slot;
    use wolf_format::mps::{MapEvent, MapEventPage};
    use wolf_vm::{Host, ProgramSet};

    fn page(trigger: u8, commands: Vec<Slot>) -> MapEventPage {
        page_with(trigger, [0; 4], [0; 4], [0; 4], commands)
    }

    fn page_with(
        trigger: u8,
        switches: [u8; 4],
        variables: [u32; 4],
        values: [i32; 4],
        commands: Vec<Slot>,
    ) -> MapEventPage {
        MapEventPage {
            graphic_chip_id: 0,
            graphic: String::new(),
            direction: 2,
            frame: 0,
            opacity: 255,
            display_type: 0,
            trigger,
            trigger_switches: switches,
            trigger_variables: variables,
            trigger_values: values,
            route: Default::default(),
            commands,
            features: 3,
            shadow_graphic: 0,
            collision_width: 0,
            collision_height: 0,
            footer: 0x7A,
        }
    }

    fn map_with(events: Vec<MapEvent>) -> Map {
        Map {
            version_header: 0,
            version: 0,
            name: String::new(),
            tileset_index: 0,
            width: 4,
            height: 4,
            has_tile_data: false,
            layers: Vec::new(),
            events,
            footer: 0x66,
        }
    }

    struct Quiet;

    impl Host for Quiet {}

    #[test]
    fn triggers_map_onto_the_four_modes() {
        assert_eq!(Trigger::from_byte(0), Trigger::ActionKey);
        assert_eq!(Trigger::from_byte(1), Trigger::Autorun);
        assert_eq!(Trigger::from_byte(2), Trigger::Parallel);
        assert_eq!(Trigger::from_byte(3), Trigger::PlayerTouch);
        assert_eq!(Trigger::from_byte(9), Trigger::Unknown(9));
        assert!(!Trigger::from_byte(0).is_automatic());
        assert!(!Trigger::from_byte(3).is_automatic());
        assert!(Trigger::from_byte(1).is_automatic());
        assert!(Trigger::from_byte(2).is_looping());
        assert!(!Trigger::from_byte(1).is_looping());
        assert_eq!(Trigger::from_byte(2).byte(), 2);
    }

    #[test]
    fn the_highest_page_whose_conditions_hold_wins() {
        // Global variable 62 selects the page, exactly like SampleMapA event 9.
        let map = map_with(vec![MapEvent {
            id: 7,
            name: "door".to_owned(),
            x: 1,
            y: 1,
            pages: vec![
                page(2, vec![Slot::Blank]),
                page_with(
                    2,
                    [0x21, 0, 0, 0],
                    [2_000_062, 0, 0, 0],
                    [1, 0, 0, 0],
                    vec![Slot::Blank],
                ),
                page_with(
                    2,
                    [0x21, 0, 0, 0],
                    [2_000_062, 0, 0, 0],
                    [2, 0, 0, 0],
                    vec![Slot::Blank],
                ),
            ],
        }]);
        let mut runtime = MapRuntime::collect(&map);
        let mut vm = Vm::new(ProgramSet::default());
        let mut host = Quiet;

        runtime.start_pending(&mut vm);
        assert_eq!(runtime.slots()[0].page, 0, "no variable set: page 0");

        vm.vars.set(62, 1);
        runtime.start_pending(&mut vm);
        assert_eq!(runtime.slots()[0].page, 1);
        vm.vars.set(62, 2);
        runtime.start_pending(&mut vm);
        assert_eq!(runtime.slots()[0].page, 2, "highest matching page wins");

        vm.vars.set(62, 3);
        runtime.start_pending(&mut vm);
        assert_eq!(
            runtime.slots()[0].page,
            0,
            "page 0 is a page of its own, not a fallback"
        );
        assert!(runtime.slots()[0].instance().is_some());
        let _ = vm.step_frame(&mut host);
    }

    #[test]
    fn an_event_whose_conditions_all_fail_is_absent() {
        let map = map_with(vec![MapEvent {
            id: 3,
            name: "chest".to_owned(),
            x: 0,
            y: 0,
            pages: vec![page_with(
                2,
                [0x21, 0, 0, 0],
                [1_000_000, 0, 0, 0],
                [5, 0, 0, 0],
                vec![Slot::Blank],
            )],
        }]);
        let mut runtime = MapRuntime::collect(&map);
        let mut vm = Vm::new(ProgramSet::default());
        assert_eq!(runtime.start_pending(&mut vm), 0);
        assert_eq!(runtime.len(), 0);
        assert!(runtime.is_empty());
        assert_eq!(runtime.active_pages(&vm), vec![(3, None)]);
    }

    #[test]
    fn a_parallel_page_restarts_and_an_autorun_page_does_not() {
        let map = map_with(vec![
            MapEvent {
                id: 1,
                name: "loop".to_owned(),
                x: 0,
                y: 0,
                pages: vec![page(2, vec![Slot::Blank])],
            },
            MapEvent {
                id: 2,
                name: "once".to_owned(),
                x: 0,
                y: 0,
                pages: vec![page(1, vec![Slot::Blank])],
            },
        ]);
        let mut runtime = MapRuntime::collect(&map);
        let mut vm = Vm::new(ProgramSet::default());
        let mut host = Quiet;
        assert_eq!(runtime.start_pending(&mut vm), 2);
        assert_eq!(runtime.trigger_counts().get(&2), Some(&1));
        assert_eq!(runtime.trigger_counts().get(&1), Some(&1));
        for _ in 0..4 {
            let _ = vm.step_frame(&mut host);
            runtime.start_pending(&mut vm);
        }
        let starts: Vec<u64> = runtime.slots().iter().map(|slot| slot.starts).collect();
        assert!(starts[0] > 1, "parallel page never restarted: {starts:?}");
        assert_eq!(starts[1], 1, "autorun page restarted: {starts:?}");
    }

    #[test]
    fn a_page_change_stops_the_old_coroutine() {
        let map = map_with(vec![MapEvent {
            id: 5,
            name: "switch".to_owned(),
            x: 0,
            y: 0,
            pages: vec![
                page_with(
                    2,
                    [0x21, 0, 0, 0],
                    [2_000_010, 0, 0, 0],
                    [0, 0, 0, 0],
                    vec![Slot::Blank],
                ),
                page_with(
                    2,
                    [0x21, 0, 0, 0],
                    [2_000_010, 0, 0, 0],
                    [1, 0, 0, 0],
                    vec![Slot::Blank],
                ),
            ],
        }]);
        let mut runtime = MapRuntime::collect(&map);
        let mut vm = Vm::new(ProgramSet::default());
        runtime.start_pending(&mut vm);
        let first = runtime.slots()[0].instance().expect("page 0 started");
        assert!(vm.instance(first).is_some());
        vm.vars.set(10, 1);
        runtime.start_pending(&mut vm);
        assert!(
            vm.instance(first).is_none(),
            "the coroutine of the retired page is still alive"
        );
        let second = runtime.slots()[0].instance().expect("page 1 started");
        assert_ne!(first, second);
    }
}

