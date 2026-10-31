//! Rebuilt engine wiring for the battle command phase (M7 task-34).
//!
//! ## Why this file exists
//!
//! The shipped event graph has a bootstrap loop that never closes by itself. The
//! battle turn is a three-state machine held in normal variable 2000350 (flat slot 350):
//!
//! * 1 = the command phase runs, i.e. common event CE242 (command processing) is alive;
//! * 2 = the action phase runs, i.e. CE243 (enemy action) + CE241 (turn start);
//! * 0 = idle.
//!
//! CE242's own header has run_condition = (2000350 == 1), and the only writer that
//! puts 2000350 = 1 is CE242 itself (pc14). Nothing in the 550 common events issues
//! 210 500242, so the command phase can never be entered from the data alone: the
//! state needs to already be 1 for the only event that could set it to 1 to start.
//! This is the exact self-boot paradox documented in docs/BATTLE.md section 5.3.1 and
//! proven in docs/research/M7_BATTLE_FULL.md (task-20).
//!
//! CE239 (battle caller) is passive (run_condition = 2000000 == 0, no map event calls
//! it) and raises the battle-active flag 2000340 from 0 to 1 (pc695) once a battle has
//! actually begun. It never touches 2000350 = 1.
//!
//! ## What the real engine must do (BATTLE.md section 5.3.1, option B)
//!
//! When a battle becomes active (the 0 -> 1 edge of 2000340) and the command phase is
//! idle (2000350 == 0), the engine starts the command-phase script CE242. CE242 then
//! sets 2000350 = 1 itself and reads the command the caller already stored in 2000354
//! (CE239 pc104 copies its argument there). This file is that wiring.
//!
//! ## Honesty contract
//!
//! * This is rebuilt wiring -- the data has no 210 500242 edge. The comment and the
//!   Game::set_battle_wiring toggle exist so the addition is auditable, not hidden.
//! * It does NOT hardcode 2000350 = 1: it only starts the script, and the script
//!   writes the variable as it was written to do.
//! * It does NOT fake menu input: the command the player would have chosen is already
//!   in 2000354 because the real caller (CE239, fed by CE253/CE256/...) put it there.
//! * It does NOT seed 2000352/2000343: those are CE244's settlement outputs.
//!
//! ## Toggle
//!
//! BattleWiring::new() is disabled. Game::set_battle_wiring(true) enables it, and that
//! is what the playable test does. Keeping it off by default means the existing
//! capture/regression suite (which asserts 2000350 stays 0 on an unprepared battle) is
//! unaffected; a production boot flips it on with one call.

use wolf_vm::magic::SELF_ARG_BASE;
use wolf_vm::Vm;

/// Normal-variable flat slot for 2000350 -- the three-state command/action machine.
const VAR_COMMAND_PHASE: usize = 350;
/// Normal-variable flat slot for 2000340 -- the battle-active flag raised by CE239.
const VAR_BATTLE_ACTIVE: usize = 340;
/// Normal-variable flat slot for 2000343 -- battle decided (CE244 sets it to 1).
const VAR_BATTLE_DECIDED: usize = 343;
/// Normal-variable flat slot for 2000354 -- the command code CE239 received from its
/// caller (pc104: `2000354 = self1600003`). It is the value the player-menu would have
/// selected, so it is the honest argument to hand CE242 (whose lone argument is
/// `選択コマンド`, self1600000).
const VAR_CHOSEN_COMMAND: usize = 354;
/// Common event that runs the player command phase (command processing).
const EVENT_COMMAND_PHASE: u32 = 242;

/// The rebuilt command-phase starter. See the module docs.
#[derive(Debug, Clone)]
pub struct BattleWiring {
    /// Master switch. Off by default; Game::set_battle_wiring flips it.
    enabled: bool,
    /// Battle-active state remembered from the previous tick, for the 0 -> 1 edge.
    prev_battle_active: bool,
    /// How many times this wiring has started the command phase (for auditing/tests).
    started_rounds: u64,
}

impl BattleWiring {
    /// Disabled wiring, with no edge history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            enabled: false,
            prev_battle_active: false,
            started_rounds: 0,
        }
    }

    /// Enable or disable the rebuilt wiring.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Number of command phases this wiring has started since boot.
    #[must_use]
    pub fn started_rounds(&self) -> u64 {
        self.started_rounds
    }

    /// One engine tick, called at the top of Game::frame after the automatic events
    /// are synced and before the VM steps.
    ///
    /// Fires on the rising edge of the battle-active flag (0 -> 1) while the command
    /// phase is idle, and starts CE242. Returns the event id it started, if any.
    pub fn tick(&mut self, vm: &mut Vm) -> Option<u32> {
        if !self.enabled {
            // Still track the edge so toggling on mid-battle behaves predictably.
            self.prev_battle_active = vm.vars.get(VAR_BATTLE_ACTIVE) != 0;
            return None;
        }
        let battle_active = vm.vars.get(VAR_BATTLE_ACTIVE) != 0;
        let rising = battle_active && !self.prev_battle_active;
        self.prev_battle_active = battle_active;
        if !rising {
            return None;
        }
        // Do not disturb a command phase already running, and do not start one after the
        // battle has been decided (CE244 keeps CE239 draining its end sequence then).
        if vm.vars.get(VAR_COMMAND_PHASE) != 0 || vm.vars.get(VAR_BATTLE_DECIDED) != 0 {
            return None;
        }
        // Start the command-phase script, then hand it the command the caller already
        // resolved into 2000354, exactly as a `210 500242, ..., <command>` edge would
        // (call arguments land in self group 6, magic::SELF_ARG_BASE + slot). Without
        // this argument CE242 reads self1600000 == 0 and falls through pc143
        // (`self1600000 in {1,2,3}`) without ever reaching pc201/pc207/pc208, so no
        // turn executes and 2000350 never advances to 2 (docs/BATTLE.md:824).
        let command = vm.vars.get(VAR_CHOSEN_COMMAND);
        if let Some(id) = vm.start_event(EVENT_COMMAND_PHASE) {
            if let Some(inst) = vm.instance_mut(id) {
                inst.self_vars.set(SELF_ARG_BASE, command);
            }
            self.started_rounds = self.started_rounds.saturating_add(1);
            return Some(EVENT_COMMAND_PHASE);
        }
        None
    }
}

impl Default for BattleWiring {
    fn default() -> Self {
        Self::new()
    }
}
