//! Scheduler for common events whose header says `run_condition == 2`
//! (parallel process with a condition).
//!
//! WOLF's four run conditions are measured on this game's 550 events as
//! `{0: 442, 1: 1, 2: 97, 3: 10}` (docs/ENGINE_EVENTS.md section 2.2). The ten
//! `run_condition == 3` events are engine-owned always-on coroutines and live in
//! [`crate::game::ENGINE_ALWAYS`]; the single `run_condition == 1` event is
//! [`crate::game::ENGINE_AUTO_START`]. This module owns the remaining 97: each one
//! is created when its condition becomes true and destroyed when it becomes false.
//!
//! The scheduler is edge-triggered. A condition that stays true does not restart a
//! coroutine that ended on its own, and a condition that stays false does nothing.
//! That matches WOLF's page lifecycle ("created while the condition holds, discarded
//! when it fails") without turning a one-shot event into a per-frame loop.
//!
//! The one exception is RESUME_ON_LOAD: the HUD drawing events 216/217 are
//! linear pages that must be replayed while their condition holds, including every
//! frame after a save load. See the comment on AutoEvents::sync.
//!
//! ## A deliberately skipped event
//!
//! Event 43 (`┗オリジナルタイトル画面Ｂ`) is the visible three-item title menu. It has
//! no 210/211/300 call site (the WOLF title screen is engine driven), and its run
//! condition is true at boot, so a literal scheduler would start it on *every* map.
//! That left its menu pictures 3/4/5 drawn over the first gameplay scene. It is
//! listed in [`SKIPPED`] here and started explicitly by [`crate::game::Game::boot`]
//! and [`crate::game::Game::start_title`] instead; map changes stop it.

use std::collections::BTreeMap;

use wolf_format::commonevent::CommonEventData;
use wolf_vm::magic::VAR_BASE;
use wolf_vm::exec::InstanceId;
use wolf_vm::Vm;

/// Event id -> condition operator / variable / value, taken from the event header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutoEvent {
    /// Common event id.
    pub id: u32,
    /// Editor title, for diagnostics.
    pub title: String,
    /// Comparison operator (1 `>=`, 2 `==`, 3 `<=`; measured on all 97 events).
    pub operator: u8,
    /// Raw ordinary-variable address (`2_000_000 + n`).
    pub variable: u32,
    /// Right-hand side of the comparison.
    pub value: u32,
    /// Whether this event is replayed while its condition holds, including
    /// immediately after a save load. Only the HUD drawing events use this: they
    /// are pure presentation coroutines that end immediately, so they have to be
    /// re-created every frame to replace the literal SavePart5 text with live
    /// \v[...] markup.
    pub resume_on_load: bool,
}

/// Events this scheduler deliberately does not start.
///
/// Event 43 is the title menu. It must only run on the title screen, so the title
/// entry points start it explicitly; see the module documentation.
pub const SKIPPED: &[u32] = &[43];

/// `run_condition == 0` events that the engine never starts on its own but that
/// this runtime must start from a condition variable, because the battle flow
/// relies on them.
///
/// WOLF's `run_condition == 0` ("called only") events normally run only when a
/// 210/211/300 instruction calls them. The battle state machine breaks that
/// rule: `CE242` (condvar 2000350 == 1) and `CE243` (condvar 2000350 == 2) have
/// zero static call sites and are started purely by the engine watching their
/// condition (docs/BATTLE.md section 1.3). This is the same edge-triggered
/// condition scheduler as the `run_condition == 2` events; the list is an
/// explicit allow-list so the other thirteen dormant `run==0` events seen on
/// this data set are not accidentally woken (they are not validated battle
/// entry points). Both conditions are false at boot (2000350 == 0), so they
/// cost nothing until the battle sets 2000350.
pub const GATED_CALL_ONLY: &[u32] = &[242, 243];

/// Conditional events that must be replayed after a save load even when their
/// condition already held before the save.
///
/// `CE216` (ステータスバー表示) and `CE217` (時刻表示) are the top-bar HUD. They are
/// linear, one-shot drawing events: each runs once and ends. Their condition is
/// `2000003 == 1`, which stays true for the whole game, so the normal edge
/// scheduler would start them once at the beginning of a session and never
/// again. The save stores the *rendered* text of every picture in SavePart5
/// (e.g. picture 6200 is literally "21:40", not a \v[4]:\v[5] template), so
/// without this replay the loaded HUD keeps showing the values from the moment
/// the player saved, while the underlying variables continue to change.
///
/// This is deliberately an allow-list: replaying every `run_condition == 2`
/// event would also restart event 506 (the DLC eye overlay) and event 337.
pub const RESUME_ON_LOAD: &[u32] = &[216, 217];

/// Result of one scheduler pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Coroutines created this pass.
    pub started: usize,
    /// Coroutines stopped this pass.
    pub stopped: usize,
    /// Coroutines still alive after the pass.
    pub running: usize,
}

/// The run-condition-2 scheduler.
#[derive(Debug, Default)]
pub struct AutoEvents {
    events: Vec<AutoEvent>,
    /// Last observed truth value, so a transition can be detected.
    latched: BTreeMap<u32, bool>,
    /// Instance ids of coroutines this scheduler owns. An entry stays here even
    /// after the coroutine ends naturally: the condition must go false before a
    /// second start is allowed (edge trigger).
    running: BTreeMap<u32, InstanceId>,
    enabled: bool,
}

impl AutoEvents {
    /// Collects every `run_condition == 2` event except [`SKIPPED`].
    #[must_use]
    pub fn from_common_events(data: &CommonEventData) -> Self {
        let events = data
            .events
            .iter()
            .filter(|event| {
                !SKIPPED.contains(&event.id)
                    && (event.run_condition() == 2
                        || (event.run_condition() == 0
                            && GATED_CALL_ONLY.contains(&event.id)))
            })
            .map(|event| AutoEvent {
                id: event.id,
                title: event.title.clone(),
                operator: event.condition_operator(),
                variable: event.condition_variable,
                value: event.condition_value,
                resume_on_load: RESUME_ON_LOAD.contains(&event.id),
            })
            .collect();
        Self {
            events,
            latched: BTreeMap::new(),
            running: BTreeMap::new(),
            enabled: true,
        }
    }

    /// Number of tracked events (98 = 96 `run==2` + 2 gated `run==0` calls).
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether no event is tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Enables or disables the scheduler, used by tests and A/B captures.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether the scheduler is enabled.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Seeds the latched state from the current variable values after a save
    /// load, and resumes the HUD drawing events listed in [`RESUME_ON_LOAD`].
    ///
    /// A load restores the ordinary variables to their saved values, but leaves
    /// this scheduler's `latched` map empty. On the first `frame()`, `sync` would
    /// then read every condition that already holds as a fresh false -> true edge
    /// and start its event — replaying edges that only "appeared" because the
    /// variables were restored. Event 506 (`妹トイレ前まばたき`, the DLC eye
    /// overlay) is the visible symptom. Priming records the loaded truth values so
    /// those events stay dormant until a real 0 -> 1 transition, which still
    /// starts them normally.
    ///
    /// Two events are the exception: 216/217 draw the top-bar HUD from `\v[...]`
    /// markup, while SavePart5 stores their text already rendered. They are
    /// therefore started here (once, if their condition holds) so the HUD becomes
    /// live again on the load frame; [`Self::sync`] then replays them every frame
    /// while the condition stays true. Starting is skipped when an instance of the
    /// same event is already alive, which keeps a second load from duplicating the
    /// HUD.
    ///
    /// See docs/research/M13_AUTO_EVENTS_SCOPE.md.
    pub fn prime_latched(&mut self, vm: &mut Vm) {
        if !self.enabled {
            return;
        }
        for event in &self.events {
            let now = condition_value(vm, event.variable)
                .map_or(false, |left| condition_met(event.operator, left, event.value));
            self.latched.insert(event.id, now);
            if !now || !event.resume_on_load {
                continue;
            }
            let already_running = self
                .running
                .get(&event.id)
                .is_some_and(|instance| vm.instance(*instance).is_some())
                || vm.instances().iter().any(|instance| instance.event == event.id);
            if already_running {
                continue;
            }
            if let Some(instance) = vm.start_event(event.id) {
                self.running.insert(event.id, instance);
            }
        }
    }

    /// The tracked events, for diagnostics and tests.
    #[must_use]
    pub fn events(&self) -> &[AutoEvent] {
        &self.events
    }

    /// Coroutines currently alive.
    #[must_use]
    pub fn running_count(&self) -> usize {
        self.running.len()
    }

    /// The instance id of a tracked event, if it was started and has not been
    /// stopped by a failing condition.
    #[must_use]
    pub fn instance(&self, event: u32) -> Option<InstanceId> {
        self.running.get(&event).copied()
    }

    /// Event ids this scheduler currently owns, for diagnostics.
    #[must_use]
    pub fn running_ids(&self) -> Vec<u32> {
        self.running.keys().copied().collect()
    }

    /// Evaluates every condition and creates or destroys coroutines as needed.
    ///
    /// Events in [`RESUME_ON_LOAD`] are presentation coroutines that finish
    /// immediately and must be re-created on the next pass. WOLF treats a
    /// run_condition == 2 page as a parallel process that stays resident while
    /// its condition holds, so a linear page like CE216 runs once per frame.
    /// This is the one exception to the otherwise edge-triggered rule, and it is
    /// restricted to the allow-list: replaying every run_condition == 2 event
    /// would wake 506 (the DLC eye overlay) and 337.
    pub fn sync(&mut self, vm: &mut Vm) -> SyncReport {
        let mut report = SyncReport::default();
        if !self.enabled {
            return report;
        }
        for event in &self.events {
            let now = condition_value(vm, event.variable)
                .map_or(false, |left| condition_met(event.operator, left, event.value));
            let before = self.latched.get(&event.id).copied().unwrap_or(false);
            if event.resume_on_load && now {
                // A finished coroutine can remain in Vm::instances until the next
                // step reaps it. Treat that as gone so the HUD is redrawn exactly
                // once per frame; a coroutine parked on a child call is still live.
                let live = vm.instances().iter().any(|instance| {
                    instance.event == event.id
                        && (instance.pc < instance.program.len()
                            || !matches!(instance.wait, wolf_vm::exec::WaitState::None))
                });
                if !live {
                    if let Some(instance) = vm.start_event(event.id) {
                        self.running.insert(event.id, instance);
                        report.started += 1;
                    }
                }
            } else if now && !before {
                if let Some(instance) = vm.start_event(event.id) {
                    self.running.insert(event.id, instance);
                    report.started += 1;
                }
            } else if !now && before {
                if let Some(instance) = self.running.remove(&event.id) {
                    if vm.stop_instance(instance) {
                        report.stopped += 1;
                    }
                }
            }
            self.latched.insert(event.id, now);
        }
        report.running = self
            .running
            .values()
            .filter(|instance| vm.instance(**instance).is_some())
            .count();
        report
    }
}

/// Reads a normal-variable operand used by a run condition.
///
/// Every one of the 108 automatic events names an ordinary variable in
/// `2_000_000..2_999_999` (docs/ENGINE_EVENTS.md section 2.3), so the only legal
/// address is `VAR_BASE + 0..800`.
fn condition_value(vm: &Vm, raw: u32) -> Option<i32> {
    let index = raw.checked_sub(VAR_BASE as u32)? as usize;
    if index < 800 {
        Some(vm.vars.get(index))
    } else {
        None
    }
}

/// Applies one run-condition operator to the two operands.
///
/// Measured operators on the 97 `run_condition == 2` events are only `>=` (77),
/// `==` (19) and `<=` (1). An unknown operator is treated as false and must never
/// start a coroutine.
#[must_use]
pub fn condition_met(operator: u8, left: i32, right: u32) -> bool {
    let right = right as i32;
    match operator {
        1 => left >= right,
        2 => left == right,
        3 => left <= right,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use wolf_vm::{ProgramSet, Vm};

    fn data_root() -> PathBuf {
        if let Ok(value) = std::env::var("WOLF_DATA_ROOT") {
            if !value.is_empty() {
                return PathBuf::from(value);
            }
        }
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.pop();
        path.push("work");
        path.push("full");
        path
    }

    fn have_data() -> Option<(CommonEventData, PathBuf)> {
        let root = data_root();
        let file = root.join("BasicData").join("CommonEvent.dat");
        let Ok(bytes) = std::fs::read(&file) else {
            eprintln!("skipping: no game data at {}", file.display());
            return None;
        };
        Some((CommonEventData::parse(&bytes).expect("CommonEvent.dat"), root))
    }

    #[test]
    fn the_scheduler_tracks_conditional_and_gated_events_and_skips_the_dead_one() {
        let Some((data, _)) = have_data() else { return };
        let auto = AutoEvents::from_common_events(&data);
        assert_eq!(auto.len(), 98, "96 run=2 events plus gated 242/243");
        assert!(auto.events().iter().all(|event| matches!(event.operator, 1..=3)));
        assert!(auto.events().iter().any(|event| event.id == 38));
        assert!(!auto.events().iter().any(|event| event.id == 43));
        assert!(auto.events().iter().any(|event| event.id == 242));
        assert!(auto.events().iter().any(|event| event.id == 243));
    }

    #[test]
    fn gated_run_zero_events_start_on_their_variable_and_stay_dormant_at_boot() {
        let Some((data, _)) = have_data() else { return };
        let programs = ProgramSet::from_common_events(&data);
        let mut vm = Vm::new(programs);
        let mut auto = AutoEvents::from_common_events(&data);
        // Dormant at boot: 2000350 == 0, so neither 242 (==1) nor 243 (==2) fires.
        let report = auto.sync(&mut vm);
        assert_eq!(report.started, 1, "only event 38 is true at boot");
        assert!(!vm.instances().iter().any(|instance| instance.event == 242));
        assert!(!vm.instances().iter().any(|instance| instance.event == 243));
        // 2000350 == 1 starts 242 only.
        vm.vars.set(350, 1);
        auto.sync(&mut vm);
        assert!(vm.instances().iter().any(|instance| instance.event == 242));
        assert!(!vm.instances().iter().any(|instance| instance.event == 243));
        // 2000350 == 2 stops 242 and starts 243.
        vm.vars.set(350, 2);
        auto.sync(&mut vm);
        assert!(!vm.instances().iter().any(|instance| instance.event == 242));
        assert!(vm.instances().iter().any(|instance| instance.event == 243));
        // Back to 0 stops 243.
        vm.vars.set(350, 0);
        auto.sync(&mut vm);
        assert!(!vm.instances().iter().any(|instance| instance.event == 243));
    }

    #[test]
    fn the_three_measured_operators_compare_like_wolf() {
        assert!(condition_met(1, 2, 2));
        assert!(!condition_met(1, 1, 2));
        assert!(condition_met(2, 2, 2));
        assert!(!condition_met(2, 3, 2));
        assert!(condition_met(3, 2, 2));
        assert!(!condition_met(3, 3, 2));
        assert!(!condition_met(99, 2, 2), "unknown operators never fire");
    }

    #[test]
    fn a_true_condition_starts_event_38_and_false_stops_it() {
        let Some((data, _)) = have_data() else { return };
        let programs = ProgramSet::from_common_events(&data);
        let mut vm = Vm::new(programs);
        let mut auto = AutoEvents::from_common_events(&data);
        let report = auto.sync(&mut vm);
        assert_eq!(report.started, 1, "only event 38 is true at boot (43 is skipped)");
        assert!(vm.instances().iter().any(|instance| instance.event == 38));
        // Normal variable 160 is event 38's condition variable; 3 fails its <= 2 test.
        vm.vars.set(160, 3);
        let report = auto.sync(&mut vm);
        assert_eq!(report.stopped, 1);
        assert!(!vm.instances().iter().any(|instance| instance.event == 38));
        // Setting it back to 0 re-arms the edge and starts a fresh coroutine.
        vm.vars.set(160, 0);
        let report = auto.sync(&mut vm);
        assert_eq!(report.started, 1);
    }

    #[test]
    fn prime_latched_seeds_loaded_truth_without_starting_and_keeps_the_edge() {
        let Some((data, _)) = have_data() else { return };
        let programs = ProgramSet::from_common_events(&data);
        let mut vm = Vm::new(programs);
        let mut auto = AutoEvents::from_common_events(&data);
        // A fresh game: event 38's condition (v160 <= 2, v160 = 0) already holds.
        // Priming must record that without starting anything.
        auto.prime_latched(&mut vm);
        let report = auto.sync(&mut vm);
        assert_eq!(report.started, 0, "priming must not fabricate an edge");
        assert_eq!(report.stopped, 0);
        assert!(!vm.instances().iter().any(|instance| instance.event == 38));
        assert!(auto.enabled(), "priming must not touch enabled");
        // With the condition now false, priming-then-sync still does nothing.
        vm.vars.set(160, 3);
        auto.prime_latched(&mut vm);
        let report = auto.sync(&mut vm);
        assert_eq!(report.started, 0);
        assert!(!vm.instances().iter().any(|instance| instance.event == 38));
        // A genuine false -> true edge must still start it after priming.
        vm.vars.set(160, 0);
        let report = auto.sync(&mut vm);
        assert_eq!(report.started, 1, "a real edge still starts after priming");
        assert!(vm.instances().iter().any(|instance| instance.event == 38));
    }

    #[test]
    fn prime_latched_resumes_the_hud_events_once_and_never_the_dlc_overlay() {
        let Some((data, _)) = have_data() else { return };
        let programs = ProgramSet::from_common_events(&data);
        let mut vm = Vm::new(programs);
        let mut auto = AutoEvents::from_common_events(&data);
        // Both HUD events are gated on 2000003 == 1 (ordinary variable index 3).
        vm.vars.set(3, 1);
        auto.prime_latched(&mut vm);
        assert!(auto.running_ids().contains(&216));
        assert!(auto.running_ids().contains(&217));
        assert_eq!(
            vm.instances()
                .iter()
                .filter(|instance| instance.event == 216)
                .count(),
            1,
            "priming must start exactly one HUD status-bar instance"
        );
        assert_eq!(
            vm.instances()
                .iter()
                .filter(|instance| instance.event == 217)
                .count(),
            1,
            "priming must start exactly one HUD clock instance"
        );
        assert!(
            !auto
                .events()
                .iter()
                .find(|event| event.id == 506)
                .expect("event 506 is tracked")
                .resume_on_load,
            "the DLC eye overlay must never be a resume-on-load event"
        );
        // A second load (or the normal first-frame sync) must not duplicate them.
        auto.prime_latched(&mut vm);
        auto.sync(&mut vm);
        assert_eq!(
            vm.instances()
                .iter()
                .filter(|instance| instance.event == 216)
                .count(),
            1
        );
        assert_eq!(
            vm.instances()
                .iter()
                .filter(|instance| instance.event == 217)
                .count(),
            1
        );
        assert!(!vm.instances().iter().any(|instance| instance.event == 506));
    }
}
