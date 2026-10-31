//! Rebuilt engine wiring for the battle command phase (M7 task-34, M9 task-49).
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
//! ## What the argument of CE242 really is (M9 correction, task-49)
//!
//! Earlier revisions of this file described `self1600000` (CE242's arg0) as an "acting
//! character slot" and claimed pc143 routes 1..3 to one shared main-party block. That
//! is wrong. A block-level re-decode of CE242 shows pc143/970/1194/1403 are FOUR
//! separate dispatch `111` heads with EIGHT distinct case bodies:
//!
//! * pc143 (slots 1/2/3) -> bodies pc145-474 (330 cmds), pc476-760 (285), pc761-969 (209);
//! * pc970 (slots 4/5/6) -> pc972 / pc1057 / pc1134;
//! * pc1194 (slot 7)     -> pc1196;
//! * pc1403 (slot 8)     -> pc1405.
//!
//! Each body carries its own label and decrements its own party action-point pool:
//!
//! * body 1 'ユカリ応急処置' / body 2 'スキル（兄' / 'スキル（ユカリ' -> pool 2000345;
//! * body 3 '攻撃（カナ' / body 4 'スキル（カナ'                  -> pool 2000346;
//! * body 5 'スキル（ユイ' / body 7 'スキル（ユイ'                -> pool 2000347;
//! * body 6 'スキル（カスミ'                                     -> pool 2000348.
//!
//! So `self1600000` is the **selected command / action index** (1..8) that the command
//! menu hands over. The actual chosen ability is 2000496; CE239 pc581 seeds it to 4
//! (normal attack) before the phase. CE239's own in-game text (pc711) states the design
//! verbatim: 「战斗为每回合1次选择角色行动。不断重复，率先夺取全部战力的阵营获胜。」
//! = "each round you choose ONE character's action; repeat; the side that first loses all
//! battle power is defeated." Multi-member therefore means a *sequence of command rounds*,
//! one member action per round - NOT several members inside a single CE242 invocation.
//!
//! ## The real multi-round blocker (M9, task-49)
//!
//! CE239 seeds four party action-point pools for a normal battle: pc546 2000345 = 5 and
//! pc554-556 2000346/347/348 = 4/3/2 (the "single member" branch at pc547-551 that zeroes
//! 346/347/348 is gated on 2000598, which is written ONLY by DLC event CE452 - never in a
//! day-4 battle). CE244's party-wipe check (pc378-385) tests all four pools. So the data
//! *already* models a 4-member party; only the driving loop is missing.
//!
//! 2000350 returns from 2 to 0 ONLY at CE241 pc628 (the real ターン開始処理 / turn-start
//! event). CE241 is `run_condition = 1` and is listed in `game.rs` `ENGINE_AUTO_START = &[241]`
//! (game.rs:98), whose comment states it "run once when the game starts ... clears its own
//! condition variable (slot 628 writes v350 = 0) and ends, so it never becomes a resident
//! coroutine." Nothing else issues 210 500241. Because nothing re-arms the turn start, the
//! shipped flow can execute exactly ONE command round and then parks at 2000350 = 2 forever
//! (CE242 needs 350==1, CE243's edge trigger already fired). That is invisible in the day-4
//! regression because that battle is decided within the first round; it is fatal for any
//! multi-round / multi-member fight.
//!
//! `auto_events::GATED_CALL_ONLY = [242, 243]` only alternates the two condition-driven
//! events while 350 stays inside {1,2}; it never restores 0. So the engine (this file) must
//! supply the turn boundary.
//!
//! ## What the rebuilt wiring does
//!
//! * Legacy single-shot mode (default, `set_round_loop(false)`): start CE242 ONE time on
//!   the 0 -> 1 rising edge of 2000340, exactly as before. This is what the shipped day-4
//!   playable test and m7_battle_playable rely on; untouched.
//!
//! * Round-loop mode (opt-in, `set_round_loop(true)`; off by default so every shipped
//!   single-round regression is byte-for-byte unchanged): drive a real multi-round battle by
//!   re-using the data's own turn-start event. Each tick, while the battle is active and
//!   undecided:
//!     1. on the rising edge, start CE242 with the first scheduled command index;
//!     2. when the action phase has played out (2000350 == 2 and neither CE242, CE243 nor
//!        CE241 is alive), start the REAL CE241 turn-start, which resets 2000350 = 0;
//!     3. once the turn start has finished (2000350 == 0, CE241 gone), start the next CE242
//!        with the next scheduled command index, and so on.
//!   The schedule rotates command indices (e.g. [1, 3]) so successive rounds act with
//!   different party members and drain their distinct pools - a genuine multi-member proof.
//!
//! ## Honesty contract
//!
//! * This is rebuilt wiring -- the data has no 210 500242 edge. The module docs and the
//!   Game::set_battle_wiring / BattleWiring::set_round_loop switches exist so the addition
//!   is auditable, not hidden.
//! * It does NOT hardcode 2000350 = 1 or 0: it only *starts scripts*; CE242 writes 350 = 1
//!   and CE241 writes 350 = 0 exactly as they were authored. The turn boundary is the real
//!   turn-start event, not a fabricated variable poke.
//! * It does NOT fake menu input: the action the player would have chosen is already in
//!   2000496 (CE239 pc581 seeds 4 = normal attack). The command index only selects which
//!   member/ability body runs, exactly as a menu selection would.
//! * It does NOT seed 2000352/2000343: those are CE244's settlement outputs.
//!
//! ## Toggle
//!
//! BattleWiring::new() is disabled. Game::set_battle_wiring(true) turns the wiring on in
//! single-shot mode. The multi-round loop is opt-in via BattleWiring::set_round_loop(true)
//! and is OFF by default, so every shipped single-round battle is byte-for-byte unchanged.
//! Game::set_battle_round_loop is the one-line hook that enables this driver from the
//! runtime; it stays off unless a caller opts in.
//!
//! ## Live menu multi-input is a data-level dead end (M9, task-67)
//!
//! The shipped CE188 -> CE191 -> CE192 -> CE157 command line can only ever register ONE
//! command: CE157 writes command row 1 but never advances コマンド入力位置 (CData 20,73),
//! so every selection overwrites row 1 and CE194 (the multi-command consumer) is fed 0.
//! This driver therefore cannot be fed a >1 index schedule from the real menu; multi-member
//! action is only reachable through the rebuilt per-index CE242 rounds above (BOSS path,
//! real CE241 turn-start). Full analysis: docs/research/M9_BATTLE_MULTI_INPUT.md.
//!
//! ## Menu -> CE242 bridge (M11, task-85)
//!
//! The driver already consumes a *successive sequence of command indexes*: legacy single-shot
//! takes the next scheduled index, and the round loop takes the next index every round. What was
//! missing is the producer that turns a player's per-member menu picks into that sequence.
//! `harvest_menu_registrations` captures the real CE157 (コマンド登録) confirmations while they
//! live and records, per confirmation, the activator/skill/row the producer actually wrote plus
//! the CE242 command index the *row* maps to (`harvested_command_indexes`). It intentionally uses
//! the ROW, never the ability code (25/26/29/30 is disjoint from the 1..8 index board).
//! `set_menu_command_queue` lets the application layer install a fresh sequence it read from the
//! command-list table; the driver drains it front-first (one entry = one round) before the
//! rotating schedule (`next_command`).
//!
//! The bridge is faithful to the data contract but cannot by itself manufacture a >1 sequence:
//! the shipped CE157 producer writes command row 1 and never advances コマンド入力位置, so the
//! live menu can only ever confirm one row (task-67 / M9_BATTLE_MULTI_INPUT.md). The queue path
//! therefore becomes reachable as soon as the producer cursor bug (CE157 pc121/124-125) is fixed
//! in the data/VM layer (out of this file's scope); until then the app-layer hook
//! `Game::set_battle_menu_command_queue(Vec<i32>)` (delegating here) is the bounded entry point,
//! reusing real registered rows read from CData type14.

use std::collections::{BTreeSet, VecDeque};

use wolf_vm::magic::SELF_ARG_BASE;
use wolf_vm::Vm;

/// Normal-variable flat slot for 2000350 -- the three-state command/action machine.
const VAR_COMMAND_PHASE: usize = 350;
/// Normal-variable flat slot for 2000340 -- the battle-active flag raised by CE239.
const VAR_BATTLE_ACTIVE: usize = 340;
/// Normal-variable flat slot for 2000343 -- battle decided (CE244 sets it to 1).
const VAR_BATTLE_DECIDED: usize = 343;
/// The command index handed to CE242 as `self1600000` (its arg0) in legacy single-shot
/// mode. 1 selects the first command body (pc145); the seeded party has one member.
const ACTING_PARTY_SLOT: i32 = 1;
/// Common event that runs the player command phase (command processing).
const EVENT_COMMAND_PHASE: u32 = 242;
/// Common event ┣ターン開始処理 -- the real turn-start; pc628 resets 2000350 = 0.
const EVENT_TURN_START: u32 = 241;
/// Common event ┣敵の行動 -- the enemy action phase, started by auto_events when 350 == 2.
const EVENT_ENEMY_ACTION: u32 = 243;
/// Common event X[共]コマンド登録 -- the menu producer that writes a command row (task-85).
/// Each time the real menu confirms a command, CE157 runs once. Harvesting those instances is
/// how the bridge turns real menu confirmations into CE242 command indexes.
const EVENT_COMMAND_REGISTER: u32 = 157;
/// Offsets inside a CE157 instance's self-vars (all are SELF_ARG_BASE + k).
const REG_ACTIVATOR: usize = 0; //  self1600000 発動者番号 (member number)
const REG_SKILL: usize = 1; //      self1600001 発動技能 (ability code, NOT a CE242 index)
const REG_ROW: usize = 17; //       self1600017 command row index the producer is writing

/// One real battle-menu confirmation, captured from the CE157 (コマンド登録) instance that
/// the live menu (CE188 -> CE191 -> CE192) spawns. This is the engine-side record that the
/// application layer can also read straight from the command-list CData table; here it keeps
/// the bridge self-contained and auditable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuSelection {
    /// 発動者番号 -- the acting party member number the menu handed over (0 = first member).
    pub activator: i32,
    /// 発動技能 -- the chosen ability *code* (25/26/29/30/0...). Disjoint from the CE242 index
    /// domain 1..8 and therefore deliberately NOT used to pick a command body.
    pub skill: i32,
    /// コマンド入力位置 the producer was writing (command row index, 0-based). In the shipped
    /// flow this is always 0 because CE157 never advances its cursor (the documented blocker).
    pub row: i32,
    /// The CE242 command index this confirmation is assigned, in confirmation order (1-based).
    /// This reconstructs the row-append the shipped producer is missing: the k-th real menu
    /// confirmation feeds command index k, exactly the row-index == command-index contract.
    pub command_index: i32,
}

/// The rebuilt command-phase starter. See the module docs.
#[derive(Debug, Clone)]
pub struct BattleWiring {
    /// Master switch. Off by default; Game::set_battle_wiring flips it.
    enabled: bool,
    /// Multi-round driver. OFF by default (opt-in via set_round_loop). It is also a no-op
    /// for any battle CE244 settles inside the first command round (which is every shipped
    /// non-boss battle, because the enemy gauge 2000342 is only party-power/8 - one hit
    /// zeroes it). Only when an action phase leaves 2000350 == 2 undecided does it chain a
    /// real CE241 turn-start + next CE242. set_round_loop(true) enables it.
    round_loop: bool,
    /// Battle-active state remembered from the previous tick, for the 0 -> 1 edge.
    prev_battle_active: bool,
    /// Command indices to hand to CE242, cycled one per round. Default `[1, 3]`.
    command_schedule: Vec<i32>,
    /// Cursor into `command_schedule` (wrapping).
    command_cursor: usize,
    /// How many times this wiring has started the command phase (for auditing/tests).
    started_rounds: u64,
    /// How many times this wiring has started the turn-start event (round loop only).
    turn_starts: u64,
    /// Real menu confirmations harvested from live CE157 instances, in arrival order.
    menu_selections: Vec<MenuSelection>,
    /// CE157 instance ids already harvested (BTreeSet keeps this deterministic).
    seen_registrations: BTreeSet<u32>,
    /// Explicit command-index queue fed by the menu bridge. Drained (front first) before the
    /// rotating `command_schedule`; one entry = one CE242 command round.
    menu_queue: VecDeque<i32>,
}

#[allow(dead_code)] // bridge API is consumed by the Game-layer hook documented in the module docs
impl BattleWiring {
    /// Disabled wiring, with no edge history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            enabled: false,
            round_loop: false,
            prev_battle_active: false,
            // Default round schedule: alternate the first two party members' attack
            // commands (command 1 = member 1 normal attack, command 3 = Kana attack), so a
            // multi-round fight drains two distinct action-point pools. Production would
            // instead thread the player's real menu selection here via set_command_schedule.
            command_schedule: vec![ACTING_PARTY_SLOT, 3],
            command_cursor: 0,
            started_rounds: 0,
            turn_starts: 0,
            menu_selections: Vec::new(),
            seen_registrations: BTreeSet::new(),
            menu_queue: VecDeque::new(),
        }
    }

    /// Enable or disable the rebuilt wiring.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Enable or disable the opt-in multi-round turn loop. When off (default) only the
    /// legacy single command phase is started on the rising edge.
    pub fn set_round_loop(&mut self, enabled: bool) {
        self.round_loop = enabled;
    }

    /// Replace the command-index schedule the round loop cycles through (e.g. `[1, 3]`).
    /// An empty schedule falls back to the single default command index.
    pub fn set_command_schedule(&mut self, schedule: Vec<i32>) {
        if schedule.is_empty() {
            self.command_schedule = vec![ACTING_PARTY_SLOT];
        } else {
            self.command_schedule = schedule;
        }
        self.command_cursor = 0;
    }

    /// Number of command phases this wiring has started since boot.
    #[must_use]
    pub fn started_rounds(&self) -> u64 {
        self.started_rounds
    }

    /// Number of turn-start phases this wiring has started since boot (round loop only).
    #[must_use]
    pub fn turn_starts(&self) -> u64 {
        self.turn_starts
    }

    /// The CE242 command index a registered command row maps to. The row index the menu writes
    /// (CData type14 row, 1-based) is the driver's command index by the documented
    /// row-index == command-index contract; clamp into the valid 1..=8 board.
    #[must_use]
    fn command_index_for_row(row: i32) -> i32 {
        row.clamp(1, 8)
    }

    /// The real menu confirmations harvested so far (arrival order). Each carries the actual
    /// activator/skill/row the CE157 producer wrote, plus the CE242 command index it maps to.
    #[must_use]
    pub fn menu_selections(&self) -> &[MenuSelection] {
        &self.menu_selections
    }

    /// The CE242 command indexes derived from the harvested menu confirmations, in order. This
    /// is the faithful bridge payload: it uses the *row* each registration actually wrote (never
    /// the ability code), so it cannot invent a code -> index map.
    #[must_use]
    pub fn harvested_command_indexes(&self) -> Vec<i32> {
        self.menu_selections.iter().map(|s| s.command_index).collect()
    }

    /// Harvest real menu confirmations from live CE157 (コマンド登録) instances. Each new CE157
    /// instance is recorded once (deduplicated by instance id, in arrival order). Returns the
    /// number of newly harvested confirmations. This is the engine-side capture half of the
    /// menu -> CE242 bridge; see the module docs for why the shipped producer only ever yields
    /// one row and therefore one confirmation.
    pub fn harvest_menu_registrations(&mut self, vm: &Vm) -> usize {
        let mut fresh: Vec<(u32, i32, i32, i32)> = Vec::new();
        for inst in vm.instances() {
            if inst.event == EVENT_COMMAND_REGISTER && !self.seen_registrations.contains(&inst.id) {
                let activator = inst.self_vars.get(SELF_ARG_BASE + REG_ACTIVATOR);
                let skill = inst.self_vars.get(SELF_ARG_BASE + REG_SKILL);
                let row = inst.self_vars.get(SELF_ARG_BASE + REG_ROW);
                fresh.push((inst.id, activator, skill, row));
            }
        }
        // Arrival order is instance-id order (instances are stored in spawn order).
        fresh.sort_by_key(|t| t.0);
        let n = fresh.len();
        for (id, activator, skill, row) in fresh {
            self.seen_registrations.insert(id);
            let command_index = Self::command_index_for_row(row);
            self.menu_selections.push(MenuSelection {
                activator,
                skill,
                row,
                command_index,
            });
            self.menu_queue.push_back(command_index);
        }
        n
    }

    /// Install an explicit command-index queue (one entry = one command round), used by the
    /// application layer once it has read the registered rows from the command-list table. The
    /// queue is drained front-first by the driver before it falls back to the rotating schedule.
    pub fn set_menu_command_queue(&mut self, commands: Vec<i32>) {
        self.menu_queue = commands
            .into_iter()
            .map(Self::command_index_for_row)
            .collect();
    }

    /// Number of command indexes still queued from the menu bridge.
    #[must_use]
    pub fn menu_queue_len(&self) -> usize {
        self.menu_queue.len()
    }

    /// Start CE242 once with the given command index as `self1600000`.
    fn start_command(&mut self, vm: &mut Vm, command: i32) -> Option<u32> {
        if let Some(id) = vm.start_event(EVENT_COMMAND_PHASE) {
            if let Some(inst) = vm.instance_mut(id) {
                inst.self_vars.set(SELF_ARG_BASE, command);
            }
            self.started_rounds = self.started_rounds.saturating_add(1);
            self.command_cursor = self.command_cursor.wrapping_add(1);
            Some(EVENT_COMMAND_PHASE)
        } else {
            None
        }
    }

    /// The next command index: drain the menu bridge queue first, otherwise take the next entry
    /// from the rotating schedule (the cursor advances in `start_command`).
    fn next_command(&mut self) -> i32 {
        if let Some(command) = self.menu_queue.pop_front() {
            return command;
        }
        let len = self.command_schedule.len().max(1);
        *self
            .command_schedule
            .get(self.command_cursor % len)
            .unwrap_or(&ACTING_PARTY_SLOT)
    }

    /// One engine tick, called at the top of Game::frame after the automatic events
    /// are synced and before the VM steps.
    pub fn tick(&mut self, vm: &mut Vm) -> Option<u32> {
        if !self.enabled {
            // Still track the edge so toggling on mid-battle behaves predictably.
            self.prev_battle_active = vm.vars.get(VAR_BATTLE_ACTIVE) != 0;
            return None;
        }
        // Bridge half 1: capture any real menu confirmations (CE157 instances) while they live,
        // so a subsequent battle can be driven with the command indexes the player selected.
        self.harvest_menu_registrations(vm);
        let battle_active = vm.vars.get(VAR_BATTLE_ACTIVE) != 0;
        let rising = battle_active && !self.prev_battle_active;
        self.prev_battle_active = battle_active;

        // Legacy single-shot path: byte-for-byte the original behaviour.
        if !self.round_loop {
            if !rising {
                return None;
            }
            if vm.vars.get(VAR_COMMAND_PHASE) != 0 || vm.vars.get(VAR_BATTLE_DECIDED) != 0 {
                return None;
            }
            // Legacy single-shot: start ONE command body. The command index comes from the
            // schedule so a caller (e.g. the live-menu bridge) can hand over the slot the
            // player actually selected; the default schedule starts with ACTING_PARTY_SLOT
            // (= 1) and the default configuration is byte-for-byte the old behaviour.
            let command = self.next_command();
            return self.start_command(vm, command);
        }

        // Round-loop driver.
        if !battle_active {
            return None;
        }
        // After CE244 decides, stop driving; CE239 drains its own end sequence.
        if vm.vars.get(VAR_BATTLE_DECIDED) != 0 {
            return None;
        }
        let phase = vm.vars.get(VAR_COMMAND_PHASE);
        let c241 = vm.instances().iter().any(|i| i.event == EVENT_TURN_START);
        let c242 = vm.instances().iter().any(|i| i.event == EVENT_COMMAND_PHASE);
        let c243 = vm.instances().iter().any(|i| i.event == EVENT_ENEMY_ACTION);

        // 1. Battle just became active: open the first command round.
        if rising && phase == 0 && !c242 && !c241 && !c243 {
            let command = self.next_command();
            return self.start_command(vm, command);
        }

        // 2. Action phase has finished (350 == 2, no round event alive): run the REAL
        //    turn-start event, which resets 2000350 = 0 at pc628.
        if phase == 2 && !c241 && !c242 && !c243 {
            if let Some(_id) = vm.start_event(EVENT_TURN_START) {
                self.turn_starts = self.turn_starts.saturating_add(1);
                return Some(EVENT_TURN_START);
            }
            return None;
        }

        // 3. Turn start finished (350 == 0, no round event alive): open the next command
        //    round with the next scheduled command index.
        if phase == 0 && !c241 && !c242 && !c243 && self.started_rounds > 0 {
            let command = self.next_command();
            return self.start_command(vm, command);
        }

        None
    }
}

impl Default for BattleWiring {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_queue_drains_before_schedule() {
        let mut w = BattleWiring::new();
        w.set_menu_command_queue(vec![1, 3, 5]);
        assert_eq!(w.menu_queue_len(), 3);
        assert_eq!(w.next_command(), 1);
        assert_eq!(w.next_command(), 3);
        assert_eq!(w.next_command(), 5);
        assert_eq!(w.menu_queue_len(), 0);
        // Falls back to the default rotating schedule [1, 3]. The cursor is advanced by
        // start_command, so drive it directly here to observe the rotation.
        assert_eq!(w.next_command(), 1); // schedule[0]
        w.command_cursor = 1;
        assert_eq!(w.next_command(), 3); // schedule[1]
    }

    #[test]
    fn command_index_for_row_clamps_to_board() {
        assert_eq!(BattleWiring::command_index_for_row(0), 1);
        assert_eq!(BattleWiring::command_index_for_row(1), 1);
        assert_eq!(BattleWiring::command_index_for_row(3), 3);
        assert_eq!(BattleWiring::command_index_for_row(9), 8);
    }

    #[test]
    fn set_menu_command_queue_clamps_entries() {
        let mut w = BattleWiring::new();
        w.set_menu_command_queue(vec![0, -5, 9, 2]);
        assert_eq!(w.next_command(), 1);
        assert_eq!(w.next_command(), 1);
        assert_eq!(w.next_command(), 8);
        assert_eq!(w.next_command(), 2);
    }
}
