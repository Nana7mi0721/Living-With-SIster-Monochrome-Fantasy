# M9 W1 - Why CE243/CE241 never fire on the natural day-4 battle

Task-68 (teammate m6-multiday, shared task rev2). Test-only investigation; no
production edits, no commit/push.

Deliverables:

- `crates/wolf-game/tests/m9_battle_round_gap.rs` - two tests, natural route plus
  BOSS control.
- this document.

## 1. Question and answer in one paragraph

On the natural day-4 battle the whole fight settles in one CE242 command round:
`2000350` goes `0 -> 1 -> 0` and never reaches phase `2`, therefore CE243 (enemy
action, condition `2000350 == 2`) is never instantiated during the round and CE241
(turn start, boot-once `ENGINE_AUTO_START`) is already consumed by `Game::boot()`.
CE244 writes the victory result on the decisive frame. This is a legitimate
one-round settlement, not a scheduling gap. The only CE243 ticks are a two-frame
post-settlement drain tail after frame 1600, while CE239 is already returning and
the result is already written. The BOSS control, which forces the multi-round
driver `set_battle_round_loop(true)`, does reach phase 2 and observes both CE243
and CE241, proving the scheduler itself is alive. There is no pre-battle state
difference: both paths settle with `v350 = [0, 1, 0]`, `2000352 = 1`, `2000343 = 1`
and the same enemy-gauge zero arm.

## 2. Commands and exact results

Environment: repo `E:/Study/github program/imoto life`, data root `work/all`
(with `work/retail_backup/Save/SaveData01.sav` for the BOSS control).

Natural + BOSS regression:

    cargo test -q -p wolf-game --test m9_battle_round_gap -- --nocapture

Result: `2 passed; 0 failed`. Natural output (one very long line, wrapped here):

    NATURAL:
      v350=[0, 1, 0, 2, 0]
      phases=[(0,-2147483648,0),(709,0,1),(774,1,0),(1602,0,2),(1603,2,0)]
      active_f=Some(707)
      ce253=true ce239=true ce242=true
      ce241_frames=0 ce243_frames=2 ce244_frames=0
      first244_f=None first244_pc=0 first244_live=[]
      max239pc=1099 max242pc=471 v342_min=0
      gauge_zero_f=Some(774) result_write_f=Some(774) decided_f=Some(774)
      settle_f=Some(774) v349=1 pools=[4,4,3,2]
      newly_started=[240, 242, 243]
      started_before_241=true started_after_241=true
      started_before_243=false started_after_243=true
      started_before_244=false started_after_244=false
      ce243_first=Some(1600) ce243_last=Some(1603) ce243_frames=2
      warnings=0

BOSS output:

    BOSS:
      v350=[0,1,2,0,1,2,0,1,0] rounds=3 turn_starts=2
      ce241_frames=413 ce241_first_f=Some(0)
      ce243_first_f=Some(644) ce243_last_f=Some(1526) ce243_frames=616
      v342_min=0 v352_settle=1 v343_settle=1 v352_now=0 v343_now=0
      settled=true warnings=0

Current-tree `m7_day4_playable` cross-check:

    cargo test -q -p wolf-game --test m7_day4_playable -- --nocapture

prints identically:

    DAY4 BATTLE RESULT: ce253=true ce239=true ce242=true ce243=false ce241=false
    rounds=1 battle_active_f=Some(726) v350_seq=[0,1,0] v354_max=0
    v352_peak=1 v343_peak=1 max239pc=745 max242pc=471 ... warnings=0

So the premise that `m7_day4_playable` sees CE243/CE241 true is not true on the
current tree; the true/true observation belongs to the task-57 BOSS multi-round
driver, reproduced by the BOSS control below.

## 3. Natural day-4 timeline (crates/wolf-game/tests/m9_battle_round_gap.rs:461)

| frame | event | evidence |
|---|---|---|
| 0 | battle start, `2000350 = 0` | `v350` initial |
| 707 | `2000340` (battle active) rises | `active_f=Some(707)` |
| 709 | `2000350 = 1` | CE242 command phase starts |
| 774 | enemy gauge `2000342 = 0`; CE244 result writes | `gauge_zero_f`, `result_write_f`, `decided_f`, `settle_f` all 774 |
| 774 | `2000352 = 1`, `2000343 = 1`, `2000350 = 0` | `v352_at_settle = 1`, `v343_at_settle = 1` |
| 1600-1610 | stale CE242/CE243/CE239 drain tail | `tail_live`, phase briefly 2 at 1602 then 0 at 1603 |

At settlement `v349 = 1` (one elapsed turn) and `v1600010 = 11`, so CE244 did
**not** take its turn-limit arm `pc36` (`2000349 >= 1600010`, docs/BATTLE.md:346).
It took the enemy-gauge-zero victory arm and wrote the four victory cells at CE244
`pc312-317` (`2000350=0`, `2000342=0`, `1600000=1`, `2000352=1`, `2000343=1`,
`2000386=0`; docs/BATTLE.md:350). `v342_min = 0` and the same-frame result writes
are the runtime proof of that arm.

Why CE244 itself is invisible to `instance.event == 244`:

CE239 calls it with opcode `210` (`500244`) at CE239 `pc795`; see
`pc312-317` evidence and `docs/BATTLE.md:327` for the 22 CE244 call sites.
`Vm::spawn_child` (`crates/wolf-vm/src/exec.rs:1163-1209`) creates the child but
does not insert it into `Vm::started_events` (unlike `start_event` at
`crates/wolf-vm/src/exec.rs:866`), and the 432-slot CE244 can run to completion
inside one frame, so no later frame sees a live instance 244. The settlement is
still observable through its variable writes, which is what the test asserts.

## 4. BOSS control (crates/wolf-game/tests/m9_battle_round_gap.rs:682)

Setup (`open_boss_battle`, :655): load `work/retail_backup/Save/SaveData01.sav`,
`seed_battle_databases()`, `start_engine_events()`, `set_battle_wiring(true)`,
`set_battle_round_loop(true)`, seeds `2000445 = 1`, `2000446 = 4000`, then
`start_event(253)`.

Observed: the three-round driver enters phase 2 three times
(`v350 = [0,1,2,0,1,2,0,1,0]`), CE243 lives for 616 sampled frames starting at
644 and CE241 for 413 sampled frames starting at 0. `rounds = 3` and
`turn_starts = 2`. This is the control that separates "the scheduler is broken"
from "the natural fight ended before phase 2".

## 5. Source reasons

- Phase definitions: `crates/wolf-game/src/battle_wiring.rs:6-10` - `2000350`
  value 1 runs CE242 command processing, value 2 runs CE243 enemy action and CE241
  turn start.
- CE242 is gated on `2000350 == 1` (`crates/wolf-game/src/battle_wiring.rs:12-17`);
  CE241 is the boot-once `run_condition == 1` event (`:58-64`).
- Engine wiring: `crates/wolf-game/src/game.rs:98` `ENGINE_AUTO_START = &[241]`;
  `game.rs:653-677` `start_engine_events` is idempotent through
  `engine_events_started` and returns 0 after the first call; `game.rs:691`
  `boot()` performs that first call. Hence on the natural route CE241 was already
  started and consumed before the battle, and `start_engine_events()` after boot
  returns 0 (asserted at `m9_battle_round_gap.rs:466-469`).
- Synthetic scheduler: `crates/wolf-game/src/auto_events.rs:54-68`
  `GATED_CALL_ONLY = [242, 243]`; `sync` starts an event only on a false-to-true
  condition edge (`auto_events.rs:181-191`). CE243 therefore cannot start unless
  `2000350` actually becomes 2.
- Both CE242 and CE243 have zero static callers (docs/BATTLE.md:158-160, :865), so
  only this synthetic gate can drive them.

## 6. Classification

**(a) Legitimate one-round settlement, not a scheduling gap.** The shipped battle
script ended the fight through CE244 before the command phase handed over to the
enemy-action phase. CE243's condition was never true during the fight, and CE241
is structurally boot-once rather than battle-triggered. The BOSS control proves
that when a fight does reach phase 2 both events instantiate and run normally.

**(b) Pre-battle state difference?** No. The natural route and the current
`m7_day4_playable` agree on `v350 = [0,1,0]`, `v352 = 1`, `v343 = 1`, `v342_min = 0`
and enemy-gauge-zero at the decisive frame. CE253/CE239/CE242 are all live in both.

**(c) Residual artifact, not a missing round.** After the decisive frame 774, a
stale CE242 remains live until frame 1601 and a phase-2 value appears at frame
1602 while CE239 is still tearing down; CE243 is sampled live only at frames 1600
and 1603. This is a drain-tail scheduling artifact of the synthetic
`GATED_CALL_ONLY` gate reacting after the result is already written. It does not
change `2000352` or `2000343` and disappears when CE239 returns. The test scopes
the no-phase-2 assertion to frames before/at settlement
(`m9_battle_round_gap.rs:592-620`).

## 7. Smallest fix location (only if one wants to remove the tail)

No production fix is needed for the day-4 route: it settles correctly and
warnings stay 0. If the harmless post-settlement phase-2 tail should be
suppressed, the narrowest location is the synthetic gate in
`crates/wolf-game/src/auto_events.rs:181-191` (`sync`): skip starting
`GATED_CALL_ONLY` events once `2000343` (battle finished) is 1, or stop those
instances on that edge. That is a production scheduler change and was not made in
this task.

## 8. Worktree hygiene

- New untracked files only: `crates/wolf-game/tests/m9_battle_round_gap.rs` and
  this document.
- No production or existing-test edits; no commit or push.
- Permanent scratch files left untouched.
