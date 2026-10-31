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
//! Self variables belong to the EVENT, not to the coroutine running one of its
//! pages: they survive a page switch, and a parallel page that ends and starts
//! again resumes with the bank it left behind. Each [EventState] therefore owns
//! one [SelfVars], copied into the fresh instance at every start and read back
//! when an instance finishes or a page changes.

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
        matches!(
            self,
            Trigger::Autorun | Trigger::Parallel | Trigger::Unknown(_)
        )
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
    /// Facing direction of this page (WOLF numpad code; the shipped maps use 2 and 9).
    ///
    /// Opcode 124 `0x1000` field 5 reports this for the event's active page, so it
    /// must survive page switches instead of being read from page 0 forever.
    direction: i32,
}

/// One map event: its placement, pages, and which page is active right now.
struct EventState {
    id: u32,
    name: String,
    /// Event placement on the map, in tiles (the .mps event record's map_x/map_y).
    x: i32,
    /// Event placement on the map, in tiles.
    y: i32,
    pages: Vec<PageState>,
    /// False for the boot event started by the caller and for blank events: the
    /// placement is kept (opcode 124 can still query it) but no coroutine is owned.
    runs: bool,
    /// Index of the active page, None when no page's conditions hold.
    active: Option<usize>,
    instance: Option<InstanceId>,
    /// Starts over the whole lifetime, for diagnostics.
    starts: u64,
    /// Starts of the CURRENT page; an autorun page runs once per activation.
    starts_on_page: u64,
    /// The event-owned self-variable bank, WOLF style. None until a page runs.
    self_vars: Option<SelfVars>,
}

/// A placed map event as opcode 124 `0x1000` / `0x1200` sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapActor {
    /// Map event id (the object operand of `124`).
    pub id: u32,
    /// Tile x from the event's map placement.
    pub x: i32,
    /// Tile y from the event's map placement.
    pub y: i32,
    /// Facing of the active page, None while no page's conditions hold.
    pub direction: Option<i32>,
    /// Index of the active page, None while no page's conditions hold.
    pub active_page: Option<usize>,
}

/// The party leader's placement, when the engine has placed it on the current map.
///
/// The map files in this game carry no player start coordinate (MAP_AUDIT §7.5:
/// `MapTree.dat` has no x/y and no start field), and the game is point-and-click,
/// so there is no walkable leader actor in the data. A position therefore exists
/// only after the engine places one: [`MapRuntime::place_leader`] (the new-game /
/// debug hook) or a `130` transfer that targets the hero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeaderPlacement {
    /// Tile x (precise units divided by two, truncating like the engine's integer math).
    pub x: i32,
    /// Tile y.
    pub y: i32,
    /// Precise x: half-tile units (`2 * tile` for a static placement).
    pub precise_x: i32,
    /// Precise y: half-tile units.
    pub precise_y: i32,
    /// Facing, when the placement carried one. A bare `130` transfer does not.
    pub direction: Option<i32>,
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
    /// The party leader's placement on the current map, when the engine has placed it.
    leader: Option<LeaderPlacement>,
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

    /// Collect the pages and placements of every event of a map.
    #[must_use]
    pub fn collect(map: &Map) -> Self {
        Self::collect_except(map, None)
    }

    /// Same, but never runs a skipped event (the boot event is started by the
    /// caller, and starting it too would run the title script twice).
    ///
    /// Every event keeps its map placement (x/y and each page's direction) so
    /// opcode 124 can answer for any event id, but events whose every page has an
    /// empty command stream, and the skipped event, get `runs == false` and never
    /// own a coroutine. That preserves the old behaviour of not burning an instance
    /// slot per frame on a blank event while making the placement layer complete.
    #[must_use]
    pub fn collect_except(map: &Map, skip: Option<u32>) -> Self {
        let mut events = Vec::with_capacity(map.events.len());
        for event in &map.events {
            let runs =
                Some(event.id) != skip && event.pages.iter().any(|page| !page.commands.is_empty());
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
                        direction: i32::from(page.direction),
                    }
                })
                .collect();
            events.push(EventState {
                id: event.id,
                name: event.name.clone(),
                x: event.x as i32,
                y: event.y as i32,
                pages,
                runs,
                active: None,
                instance: None,
                starts: 0,
                starts_on_page: 0,
                self_vars: None,
            });
        }
        Self {
            events,
            leader: None,
            restarts: 0,
            switches: 0,
        }
    }

    /// Every event this runtime knows, runnable or not.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// The placement of map event `id` as `124` sees it.
    ///
    /// Tile x/y always come from the parsed map. `direction` is the facing of the
    /// page selected this frame, so it follows page switches; it is `None` while no
    /// page's conditions hold (the event is not on the map), which is exactly the
    /// "no placement" case callers must not fabricate.
    #[must_use]
    pub fn actor(&self, id: u32) -> Option<MapActor> {
        let state = self.events.iter().find(|state| state.id == id)?;
        let direction = state
            .active
            .and_then(|index| state.pages.get(index))
            .map(|page| page.direction);
        Some(MapActor {
            id: state.id,
            x: state.x,
            y: state.y,
            direction,
            active_page: state.active,
        })
    }

    /// Places the party leader (the `-2` target of `124`), in whole tiles.
    ///
    /// `direction` is the WOLF facing code; pass None when the placement carries no
    /// facing (a bare `130` transfer has none) so field 5 stays unanswered instead
    /// of inventing one. The precise (0.5-tile) coordinates are derived as `tile * 2`.
    pub fn place_leader(&mut self, x: i32, y: i32, direction: Option<i32>) {
        self.place_leader_precise(x.saturating_mul(2), y.saturating_mul(2), direction);
    }

    /// Places the leader from half-tile units, the space a precise `130` transfer uses.
    ///
    /// WOLF's `130` options bit 0 means "precise coordinates": the shipped new-game
    /// transfer targets `(62, 38)` on a 64x36 map, which only fits after halving to
    /// (31, 19), and the title transfer's `(60, 34)` halves to Map003 event 1's own
    /// (30, 17) tile. Tile fields therefore truncate `precise / 2`.
    pub fn place_leader_precise(
        &mut self,
        precise_x: i32,
        precise_y: i32,
        direction: Option<i32>,
    ) {
        self.leader = Some(LeaderPlacement {
            x: precise_x / 2,
            y: precise_y / 2,
            precise_x,
            precise_y,
            direction,
        });
    }

    /// The leader's current placement, None until the engine has placed it.
    #[must_use]
    pub fn leader(&self) -> Option<LeaderPlacement> {
        self.leader
    }

    /// How many runnable events have an active page with a command stream right now.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events
            .iter()
            .filter(|state| {
                state.runs
                    && state.active.is_some_and(|index| {
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

    /// One slot per active event that the runtime runs, in map file order.
    #[must_use]
    pub fn slots(&self) -> Vec<PageSlot> {
        self.events
            .iter()
            .filter_map(|state| {
                if !state.runs {
                    return None;
                }
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

    /// Stops every coroutine this runtime owns, e.g. before switching maps.
    pub fn stop_all(&mut self, vm: &mut Vm) {
        for state in &mut self.events {
            if let Some(id) = state.instance.take() {
                vm.stop_instance(id);
            }
            state.active = None;
        }
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
    /// anything. Self variables come from the running instance when there is
    /// one, otherwise from the event's stored bank.
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
        // A retained instance whose program has ended still owns the bank the
        // event must keep; take it and retire the instance before selection.
        for state in &mut self.events {
            let Some(id) = state.instance else {
                continue;
            };
            let Some(instance) = vm.instance(id) else {
                state.instance = None;
                continue;
            };
            if instance.pc >= instance.program.len() {
                state.self_vars = Some(instance.self_vars.clone());
                vm.stop_instance(id);
                state.instance = None;
            }
        }

        // Phase 1 reads the VM (variables and self banks); phase 2 mutates it.
        // Splitting them keeps the borrow checker happy without cloning the VM.
        let mut wanted: Vec<Option<usize>> = Vec::with_capacity(self.events.len());
        for state in &self.events {
            wanted.push(select_page(state, vm));
        }

        let mut started = 0;
        for (state, want) in self.events.iter_mut().zip(wanted) {
            if state.active != want {
                // Deactivation while the page's coroutine is parked in a blocking
                // 210/300 CALL must wait: the call is synchronous, so the page is
                // still running its own command list and has not reached a point
                // where WOLF would abandon it. (Switching to another live page is
                // left to the normal path below; only losing the page entirely is
                // deferred.) Otherwise a variable written inside the called event
                // that falsifies this page's activation condition tears the page
                // and its whole descendant CALL stack down mid-script. Without this
                // guard CE329's own var69 = 0 (pc41) kills its ancestor page and
                // the chain before the day rollover at pc385.
                if want.is_none() {
                    if let Some(id) = state.instance {
                        if let Some(instance) = vm.instance(id) {
                            if matches!(instance.wait, wolf_vm::WaitState::Child(_)) {
                                continue;
                            }
                        }
                    }
                }
                if let Some(id) = state.instance.take() {
                    if let Some(instance) = vm.instance(id) {
                        state.self_vars = Some(instance.self_vars.clone());
                    }
                    vm.stop_instance(id);
                }
                if state.runs && state.active.is_some() {
                    self.switches += 1;
                }
                state.active = want;
                state.starts_on_page = 0;
            }
            // A skipped boot event or a blank event keeps its placement and page
            // selection (so 124 can answer for it) but never owns a coroutine.
            if !state.runs {
                continue;
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
            let self_vars = state.self_vars.clone().unwrap_or_default();
            if let Some(id) = vm.start_program_with_self(program, self_vars) {
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
    let live = state
        .instance
        .and_then(|id| vm.instance(id))
        .map(|instance| &instance.self_vars);
    let self_vars = live.or(state.self_vars.as_ref());
    (0..state.pages.len())
        .rev()
        .find(|index| conditions_met(&state.pages[*index].conditions, self_vars, &vm.vars))
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
    use wolf_format::command::{Command, Slot, Tail};
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

    fn command(opcode: u32, params: &[i32]) -> Slot {
        let mut bytes = Vec::with_capacity(params.len() * 4);
        for param in params {
            bytes.extend_from_slice(&param.to_le_bytes());
        }
        Slot::Command(Box::new(Command {
            param_count: params.len() as u8 + 1,
            opcode,
            params: bytes,
            branch_depth: 0,
            strings: Vec::new(),
            tail: Tail::None,
        }))
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

    #[test]
    fn self_variables_survive_a_page_switch() {
        // Page 0 writes self[0] = 1; page 1 only exists while that value is 1.
        let map = map_with(vec![MapEvent {
            id: 7,
            name: "bank".to_owned(),
            x: 0,
            y: 0,
            pages: vec![
                page(1, vec![command(121, &[1_100_000, 1, 0, 0])]),
                page_with(
                    2,
                    [0x21, 0, 0, 0],
                    [1_100_000, 0, 0, 0],
                    [1, 0, 0, 0],
                    vec![Slot::Blank],
                ),
            ],
        }]);
        let mut runtime = MapRuntime::collect(&map);
        let mut vm = Vm::new(ProgramSet::default());
        let mut host = Quiet;

        runtime.start_pending(&mut vm);
        assert_eq!(
            runtime.slots()[0].page,
            0,
            "page 0 starts with the bank at 0"
        );
        let first = runtime.slots()[0].instance().expect("page 0 started");
        let _ = vm.step_frame(&mut host);
        assert!(vm.instance(first).is_some(), "a finished page is retained");
        assert_eq!(vm.instance(first).unwrap().self_vars.get(100), 1);

        runtime.start_pending(&mut vm);
        assert_eq!(
            runtime.slots()[0].page,
            1,
            "the page condition must read the bank the finished page left behind"
        );
    }

    #[test]
    fn a_parallel_page_keeps_its_self_bank_across_restarts() {
        let map = map_with(vec![MapEvent {
            id: 8,
            name: "counter".to_owned(),
            x: 0,
            y: 0,
            pages: vec![page(2, vec![command(121, &[1_100_000, 1, 0, 0x100])])],
        }]);
        let mut runtime = MapRuntime::collect(&map);
        let mut vm = Vm::new(ProgramSet::default());
        let mut host = Quiet;

        runtime.start_pending(&mut vm);
        let first = runtime.slots()[0].instance().expect("page started");
        let _ = vm.step_frame(&mut host);
        assert_eq!(vm.instance(first).unwrap().self_vars.get(100), 1);

        runtime.start_pending(&mut vm);
        let second = runtime.slots()[0].instance().expect("page restarted");
        assert_ne!(first, second);
        let _ = vm.step_frame(&mut host);
        assert_eq!(
            vm.instance(second).unwrap().self_vars.get(100),
            2,
            "a restarted parallel page must resume with the event's bank"
        );
    }

    #[test]
    fn placements_are_kept_for_skipped_and_blank_events() {
        let map = map_with(vec![
            MapEvent {
                id: 1,
                name: "skip".to_owned(),
                x: 4,
                y: 5,
                pages: vec![page(2, vec![Slot::Blank])],
            },
            MapEvent {
                id: 2,
                name: "blank".to_owned(),
                x: 6,
                y: 7,
                pages: vec![page(2, Vec::new())],
            },
            MapEvent {
                id: 3,
                name: "run".to_owned(),
                x: 8,
                y: 9,
                pages: vec![page(2, vec![Slot::Blank])],
            },
        ]);
        let mut runtime = MapRuntime::collect_except(&map, Some(1));
        let mut vm = Vm::new(ProgramSet::default());
        let mut host = Quiet;
        runtime.start_pending(&mut vm);

        // The boot event and the blank event own no coroutine ...
        assert_eq!(runtime.len(), 1, "only event 3 may run");
        let slots = runtime.slots();
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].event, 3);
        // ... but their placement is still queryable for opcode 124.
        let skipped = runtime.actor(1).expect("skipped event placement");
        assert_eq!((skipped.x, skipped.y), (4, 5));
        assert_eq!(skipped.direction, Some(2));
        let blank = runtime.actor(2).expect("blank event placement");
        assert_eq!((blank.x, blank.y), (6, 7));
        assert!(runtime.actor(99).is_none(), "no such map event");
        let _ = vm.step_frame(&mut host);
    }

    #[test]
    fn an_actors_direction_follows_the_active_page() {
        // Page 0 faces down (2); page 1 faces up-right (9) and takes over when v10 = 1.
        let mut up = page_with(
            2,
            [0x21, 0, 0, 0],
            [2_000_010, 0, 0, 0],
            [1, 0, 0, 0],
            vec![Slot::Blank],
        );
        up.direction = 9;
        let map = map_with(vec![MapEvent {
            id: 7,
            name: "facing".to_owned(),
            x: 12,
            y: 13,
            pages: vec![page(2, vec![Slot::Blank]), up],
        }]);
        let mut runtime = MapRuntime::collect(&map);
        let mut vm = Vm::new(ProgramSet::default());

        runtime.start_pending(&mut vm);
        let first = runtime.actor(7).expect("placed event");
        assert_eq!((first.x, first.y), (12, 13));
        assert_eq!(first.active_page, Some(0));
        assert_eq!(first.direction, Some(2));

        vm.vars.set(10, 1);
        runtime.start_pending(&mut vm);
        let second = runtime.actor(7).expect("placed event");
        assert_eq!(second.active_page, Some(1));
        assert_eq!(
            second.direction,
            Some(9),
            "field 5 must read the active page, not page 0"
        );
    }

    #[test]
    fn an_actor_without_an_active_page_has_no_direction() {
        let map = map_with(vec![MapEvent {
            id: 4,
            name: "conditional".to_owned(),
            x: 2,
            y: 3,
            pages: vec![page_with(
                2,
                [0x21, 0, 0, 0],
                [2_000_010, 0, 0, 0],
                [5, 0, 0, 0],
                vec![Slot::Blank],
            )],
        }]);
        let mut runtime = MapRuntime::collect(&map);
        let mut vm = Vm::new(ProgramSet::default());
        runtime.start_pending(&mut vm);
        let actor = runtime.actor(4).expect("placement exists");
        assert_eq!((actor.x, actor.y), (2, 3), "the map placement is real");
        assert_eq!(actor.active_page, None);
        assert_eq!(actor.direction, None, "no page means no facing to report");
    }

    #[test]
    fn the_leader_exists_only_after_an_explicit_placement() {
        let mut runtime = MapRuntime::new();
        assert_eq!(runtime.leader(), None);
        runtime.place_leader(11, 12, None);
        assert_eq!(
            runtime.leader(),
            Some(LeaderPlacement {
                x: 11,
                y: 12,
                precise_x: 22,
                precise_y: 24,
                direction: None
            })
        );
        runtime.place_leader(11, 12, Some(8));
        assert_eq!(runtime.leader().unwrap().direction, Some(8));
        // A precise `130` transfer stores half-tiles; the tile fields truncate.
        runtime.place_leader_precise(63, 39, Some(2));
        let precise = runtime.leader().unwrap();
        assert_eq!((precise.x, precise.y), (31, 19));
        assert_eq!((precise.precise_x, precise.precise_y), (63, 39));
        // A fresh collection is a new map; it must not carry the old map's position.
        let map = map_with(Vec::new());
        let collected = MapRuntime::collect(&map);
        assert_eq!(collected.leader(), None);
    }
}
