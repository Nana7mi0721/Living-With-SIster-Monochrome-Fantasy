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
//! ## Toggle and the natural-battle auto-arm (M11, task-95)
//!
//! There are three modes:
//!
//! * **Auto-arm (default).** With no explicit `Game::set_battle_wiring` call, the wiring arms
//!   itself as soon as a real battle is running, detected in either of two ways:
//!   (a) the shipped battle *dispatch* event CE253 (地域貢献) is resident - it parks on its
//!   `210 500239` call for the whole fight; or (b) a CE239 (■戦闘呼び出し) instance has a
//!   parent, i.e. a script called it (CE305 キャンセナル, CE256 討伐依頼, ...), which covers
//!   every other real battle dispatcher. Direct `start_event(239)` in unit/probe tests
//!   creates a parentless instance, so those tests stay untouched. Auto-armed battles also
//!   run the multi-round loop, because the day-4 battle genuinely needs two command rounds
//!   to settle. This is what makes the natural CE253 -> CE239 line and the day-11 CE305 ->
//!   CE239 story battle drive CE242 without any test-only flag. Turn it off with
//!   `Game::set_battle_auto_arm(false)`.
//! * **Explicit opt-in.** `Game::set_battle_wiring(true)` forces the drive regardless of
//!   CE253; `Game::set_battle_round_loop(true/false)` then selects round-loop vs single-shot.
//!   This is the old behaviour every battle test relies on, unchanged.
//! * **Explicit opt-out.** `Game::set_battle_wiring(false)` is a hard opt-out that overrides
//!   auto-arm (acceptance c): the shipped stall (CE239 parks, 2000350 stays 0) is preserved.
//!
//! Guard against double-driving: if a test already drives CE242 by hand it starts CE239/CE242
//! *directly* (no CE253), so auto-arm stays inert; and any explicit `set_battle_wiring(false)`
//! wins over auto-arm. The wiring never starts a second CE242 while a CE242/CE241/CE243 round
//! event is already alive.
//!
//! ## Menu -> CE242 bridge (M11 task-85, corrected by task-89 / task-98)
//!
//! The driver already consumes a *successive sequence of command indexes*: legacy single-shot
//! takes the next scheduled index, and the round loop takes the next index every round. The
//! producer that turns a player's per-member menu picks into that sequence is the shipped
//! CE188 -> CE191 -> CE192 -> CE152 -> CE65 -> CE157 (コマンド登録) line. Each confirmed command
//! is written into the command-processing list CData type14 at command row = コマンド入力位置
//! (CData 18,73), and CE157 advances that cursor (pc124-125).
//!
//! NOTE: an earlier M9 task-67 note claimed CE157 can only ever register row 1 because it never
//! advances the cursor. M11 round-2 task-89 retracted that folklore: with real consecutive OK
//! confirms the cursor advances 1 -> 8 and distinct rows 1..7 are written. m11_battle_menu_live.rs
//! drives the real menu and reads back two distinct rows, proving it.
//!
//! `harvest_menu_registrations` reads those *committed* CData type14 rows (activator = col 0,
//! ability code = col 3) once at least one CE157 is live, dedupes by row index, and pushes the
//! row's command index (row.clamp(1,8)) into the driver queue. It deliberately reads the table,
//! NOT the CE157 self-var self1600017: that self-var is only assigned the row at pc80-84 and is
//! incremented at pc124, so a step-boundary read catches the instance parked at pc64 with a stale
//! (0) value -- the exact bug task-98 fixed (it had produced a [1,1] queue). `set_menu_command_queue`
//! remains a bounded app-layer hook that installs a sequence read from the same table; the driver
//! drains the harvested queue (or an injected one) front-first before the rotating schedule.

use std::collections::{BTreeSet, VecDeque};

use crate::db::{Container, Databases};
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
/// Common event 地域貢献 (CE253) -- the natural battle *dispatch* event. It calls the real
/// battle caller CE239 and stays resident (parked on the call) until CE239 returns. Its
/// presence distinguishes a genuine in-game battle from the unit/probe tests that start
/// CE239 or CE242 directly, so it is the auto-arm signal for the shipped route (task-95).
const EVENT_BATTLE_DISPATCH: u32 = 253;
/// Common event ■戦闘呼び出し (CE239) -- the real battle caller. A CE239 instance that has a
/// parent was started by a script (CE305 キャンセナル, CE256 討伐依頼, ...), while the
/// unit/probe tests start it directly and get `parent == None`. This is the second auto-arm
/// signal: it covers every real battle dispatcher, not just CE253 (M12 day11 soak).
const EVENT_BATTLE_CALLER: u32 = 239;
/// Common event X[共]コマンド登録 -- the menu producer that writes a command row (task-85).
/// Each time the real menu confirms a command, CE157 runs once. Harvesting those instances is
/// how the bridge turns real menu confirmations into CE242 command indexes.
const EVENT_COMMAND_REGISTER: u32 = 157;
/// Physical CData table the CE157 producer writes each confirmed command row into
/// (論理 コマンド処理リスト -> physical type 14). Row index r is the CE242 command index.
const CMD_LIST_TYPE: i32 = 14;

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
    /// Command-processing-list row (論理 コマンド処理リスト, physical CData type14) this
    /// confirmation filled, 1-based. Harvested from the committed table, not the transient CE157
    /// self-var; the row index equals the CE242 command index by contract.
    pub row: i32,
    /// The CE242 command index this confirmation is assigned, in confirmation order (1-based).
    /// This reconstructs the row-append the shipped producer is missing: the k-th real menu
    /// confirmation feeds command index k, exactly the row-index == command-index contract.
    pub command_index: i32,
}

/// The rebuilt command-phase starter. See the module docs.
#[derive(Debug, Clone)]
pub struct BattleWiring {
    /// Whether Game::set_battle_wiring was ever called. `None` = never set, so the wiring
    /// auto-arms on the natural CE253 battle; `Some(true/false)` = explicit opt-in/opt-out
    /// (an explicit `false` is a hard opt-out that overrides auto-arm, task-95).
    manual_enable: Option<bool>,
    /// Explicit manual round-loop state set by Game::set_battle_round_loop. `None` = never
    /// set; in that case an auto-armed natural battle runs the multi-round loop (day-4 needs
    /// it to settle) while an explicit single-shot opt-in stays single-shot.
    manual_round_loop: Option<bool>,
    /// Auto-arm the rebuilt drive on the shipped natural battle (CE253 -> CE239). Default on;
    /// disable with Game::set_battle_auto_arm(false) to reproduce the raw shipped stall.
    auto_arm: bool,
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
    /// Command rows already harvested from the committed command list (dedup by row index).
    harvested_rows: BTreeSet<i32>,
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
            manual_enable: None,
            manual_round_loop: None,
            auto_arm: true,
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
            harvested_rows: BTreeSet::new(),
            menu_queue: VecDeque::new(),
        }
    }

    /// Enable or disable the rebuilt wiring. This is an EXPLICIT opt-in/opt-out: it records
    /// that the switch was set by hand, so `set_enabled(false)` is a hard opt-out that beats
    /// the natural-battle auto-arm (task-95 acceptance c).
    pub fn set_enabled(&mut self, enabled: bool) {
        self.manual_enable = Some(enabled);
    }

    /// Turn the natural-battle auto-arm on or off. Default on; `false` restores the raw
    /// shipped stall (CE253 -> CE239 with no command-phase driver) unless the wiring is also
    /// explicitly enabled. This does not change an explicit `set_enabled` opt-in/opt-out.
    pub fn set_auto_arm(&mut self, enabled: bool) {
        self.auto_arm = enabled;
    }

    /// Whether the natural-battle auto-arm is enabled (default true).
    #[must_use]
    pub fn auto_armed(&self) -> bool {
        self.auto_arm
    }

    /// Enable or disable the multi-round turn loop. When set explicitly it also records the
    /// choice (`manual_round_loop`). Left unset, an auto-armed natural battle runs the loop
    /// (day-4 needs it to settle) while an explicit single-shot opt-in stays single-shot.
    pub fn set_round_loop(&mut self, enabled: bool) {
        self.round_loop = enabled;
        self.manual_round_loop = Some(enabled);
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

    /// Harvest real menu confirmations from live CE157 (コマンド登録) instances. While a CE157
    /// is live (the menu is open in ordinary map state) every committed command row in the
    /// command-processing list (CData type14) not yet seen is recorded once (deduplicated by row
    /// index). Returns the number of newly harvested rows. This is the engine-side capture half
    /// of the menu -> CE242 bridge; reading the committed table (not the transient CE157
    /// self-var) is what makes multiple distinct confirmations harvestable -- task-98.
    pub fn harvest_menu_registrations(&mut self, vm: &Vm, db: &Databases) -> usize {
        // Only harvest while a real command-registration coroutine is live (the CE188 -> CE192
        // -> CE157 menu is open in ordinary map state, before the rebuilt drive arms). Gating on
        // a live CE157 keeps stale command rows from a previous fight out of a later battle.
        if !vm.instances().iter().any(|i| i.event == EVENT_COMMAND_REGISTER) {
            return 0;
        }
        let mut added = 0usize;
        // The row index IS the CE242 command index (row-index == command-index contract).
        for row in 1..=8i32 {
            if self.harvested_rows.contains(&row) {
                continue;
            }
            // col 0 = 発動者番号 (member number, >= 0 once the producer committed this row).
            let activator = match db.num(Container::CData, CMD_LIST_TYPE, row, 0) {
                Some(a) if a >= 0 => a,
                _ => continue,
            };
            // col 3 = chosen ability code; recorded only, never used to pick a command body.
            let skill = db.num(Container::CData, CMD_LIST_TYPE, row, 3).unwrap_or(0);
            self.harvested_rows.insert(row);
            let command_index = Self::command_index_for_row(row);
            self.menu_selections.push(MenuSelection {
                activator,
                skill,
                row,
                command_index,
            });
            self.menu_queue.push_back(command_index);
            added += 1;
        }
        added
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

    /// True when a genuine in-game battle is on screen: either CE253 is parked on its
    /// `210 500239` dispatch call, or a CE239 instance was started by a parent script
    /// (CE305 キャンセナル, CE256 討伐依頼, ...). A directly started CE239/CE242 has no
    /// parent, which is how unit/probe tests keep the raw shipped behaviour.
    #[must_use]
    pub fn real_battle_running(vm: &Vm) -> bool {
        vm.instances()
            .iter()
            .any(|instance| instance.event == EVENT_BATTLE_DISPATCH)
            || vm.instances().iter().any(|instance| {
                instance.event == EVENT_BATTLE_CALLER && instance.parent.is_some()
            })
    }

    /// One engine tick, called at the top of Game::frame after the automatic events
    /// are synced and before the VM steps.
    pub fn tick(&mut self, vm: &mut Vm, db: &Databases) -> Option<u32> {
        let battle_active = vm.vars.get(VAR_BATTLE_ACTIVE) != 0;
        let rising = battle_active && !self.prev_battle_active;
        self.prev_battle_active = battle_active;

        // Resolve the effective drive mode (M11 task-95).
        //
        // * An explicit Game::set_battle_wiring(v) wins outright: `Some(false)` is a hard
        //   opt-out (acceptance c) and `Some(true)` is the old opt-in.
        // * Otherwise the wiring auto-arms on the shipped natural battle: either the CE253
        //   dispatch event is resident (it parked on its CE239 call), or CE239 was started by
        //   a parent script (any real battle dispatcher, e.g. CE305 / CE256). Unit/probe tests
        //   that start CE239 or CE242 directly create a parentless instance and have no CE253,
        //   so they stay byte-for-byte untouched.
        let auto = self.auto_arm && BattleWiring::real_battle_running(vm);
        let drive = match self.manual_enable {
            Some(v) => v,
            None => auto,
        };
        // Bridge half 1: capture any real menu confirmations (CE157 instances) while they live,
        // so a subsequent battle can be driven with the command indexes the player selected.
        // This MUST run even when the drive is off (M11 task-98): the player's battle command
        // menu (CE188 -> CE192 -> CE157) is confirmed in ordinary map state, before the rebuilt
        // drive arms on the CE253 battle dispatch. Harvesting only inside the drive would miss
        // every real menu confirmation. Deduplication by row index makes the call idempotent
        // and a no-op when no CE157 (menu) instance is live, so non-menu ticks are unaffected.
        self.harvest_menu_registrations(vm, db);

        if !drive {
            return None;
        }
        // Round loop: an explicit setting wins; an auto-armed natural battle runs the loop
        // (day-4 needs more than one command round to settle under correct opcode-111
        // semantics); an explicit single-shot opt-in stays single-shot.
        let round_loop = match self.manual_round_loop {
            Some(v) => v,
            None => auto,
        };

        // Legacy single-shot path: byte-for-byte the original behaviour.
        if !round_loop {
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
