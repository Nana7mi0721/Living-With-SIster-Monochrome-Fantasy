# M11 P0 — Arm the engine battle drive by default (natural day-4 route)

Task: task-95. Repo HEAD at start: 5ef6b1f. No commit/push.

## 1. Problem

Before this task the rebuilt battle wiring (BattleWiring) was built **disabled** and only tests
called `Game::set_battle_wiring(true)` / `Game::set_battle_round_loop(true)`. On the shipped
natural route the day-4 story forces CE253 (地域貢献) -> CE239 (戦闘呼び出し), but nothing started the
command phase CE242. CE239 raises the battle-active flag 2000340 and parks at pc13, while 2000350
(command phase) stays 0 — CE242 is the only writer of 2000350=1, so the shipped default stalls the
battle forever and the clock never leaves day 4 08:30.

Empirical default (temp copy of m7_day4_playable with the wiring flags removed):

    ce253=true ce239=true ce242=false rounds=0 battle_active_f=Some(726)
    v350_seq=[0] v352_peak=0 v343_peak=0 max239pc=845 max242pc=0
    ce239_alive=true ce239_pc=Some(13) warnings=0   (day stays 4)

Single-round (wiring on, round_loop OFF): CE242 starts, v350_seq=[0,1,2], but v352=0 / v343=0
(not settled) and the fight ends with 2000350 stuck at 2. So the day-4 fight **requires** the
multi-round loop under the corrected opcode-111 semantics. With round_loop ON it settles.

## 2. Exact change

### crates/wolf-game/src/battle_wiring.rs
- New const `EVENT_BATTLE_DISPATCH: u32 = 253` (the shipped battle dispatch instance).
- Struct fields: replaced `enabled: bool` with:
  `manual_enable: Option<bool>`, `manual_round_loop: Option<bool>`, `auto_arm: bool`
  (constructor: `manual_enable: None`, `manual_round_loop: None`, `auto_arm: true`).
- `set_enabled(v)` -> `manual_enable = Some(v)` (so `set_battle_wiring(false)` is a hard opt-out,
  `set_battle_wiring(true)` a hard opt-in).
- `set_round_loop(v)` -> sets `round_loop = v`, `manual_round_loop = Some(v)`.
- New `set_auto_arm(bool)` and `auto_armed() -> bool`.
- `tick()` rewritten:
  - `battle_active = vars[340] != 0`; `rising = battle_active && !prev`; `prev = battle_active`.
  - `dispatch_alive = any instance.event == 253` — CE253 stays resident on its `210 500239` call for
    the whole fight, so this is the unique "natural battle" discriminator.
  - `auto = auto_arm && dispatch_alive`; `drive = manual_enable.unwrap_or(auto)`.
  - if `!drive` return None (no drive).
  - `round_loop = manual_round_loop.unwrap_or(auto)`.
  - `harvest_menu_registrations(vm)` then the existing single-shot / round-loop logic.
- Module doc: replaced the old "Toggle" section with "Toggle and the natural-battle auto-arm
  (M11, task-95)": Auto-arm(default) / Explicit opt-in / Explicit opt-out.

### crates/wolf-game/src/game.rs (additive, after set_battle_round_loop)
```rust
pub fn set_battle_auto_arm(&mut self, enabled: bool) {
    self.battle_wiring.set_auto_arm(enabled);
}
pub fn battle_wiring_auto_armed(&self) -> bool {
    self.battle_wiring.auto_armed()
}
```

### Why the CE253 discriminator (compat)
All direct-CE239 / injected-CE242 probe tests start CE239/CE242 themselves with **no CE253 instance**,
so `dispatch_alive == false` there and the auto-arm never engages — those tests stay byte-for-byte:
- m7_battle_target_scheduler (ce239_slots_never_raise_2000350, injected_ce242_exposes_the_three_state_cycle)
- m7_battle_full scenarios 2/3, m7_battle_round scenario B (all start_event(239) directly, assert v350 stays 0)
- m6_week_loop::day_four_morning_is_forced_into_the_subjugation_battle_wall — reaches day 4 but its
  `has_battle()` loop breaks on the first frame a 253/239 instance appears (before 2000340 rises), so its
  day==4 / 08:30 asserts still hold even with the auto-arm on.
- m8_battle_party::open_battle(false) calls `set_battle_wiring(false)` -> hard opt-out preserved.

## 3. Natural-route trace (test a, NO set_battle_wiring call)

`crates/wolf-game/tests/m11_battle_autostart.rs::natural_day4_battle_settles_without_wiring_flag`
loads a fresh Game, runs the genuine new-game schedule (start_map_event_by_name "SampleMapA" -> 1600f
-> day1 18:30 -> sleep d2 -> spend d2 -> sleep d3 -> spend d3 -> sleep d4 08:30), seeds the battle
databases, starts engine events, then traces 2400 frames of the day-4 fight **without ever calling
`set_battle_wiring`**:

    AUTOSTART (a): ceil253=true ce239=true ce242=true ce243=true ce241=true rounds=2
    battle_active_f=Some(726) v350_seq=[0, 1, 2, 0, 1, 0]
    v352_peak=1 v343_peak=1 max239pc=845 max242pc=866
    ce239_alive=false ce239_pc=None warnings=0 unanswered250=1
    -> day 5 08:30, warnings=0, frames=8237

So the auto-armed default reproduces the settling baseline exactly: two command rounds
(0->1->2->0->1->0), 2000352=1, 2000343=1, CE239 returns, day 5 08:30.

## 4. Two-member queue under the auto drive (test b)

`two_member_queue_dispatches_distinct_bodies_under_auto_drive` builds a 2-member party fixture
(party count type18 r4 c0 = 2; second member status row 13; command slots [4,3,6,7,...]); queues
command indexes [1,2,3,4] via `set_battle_menu_command_queue`; seeds the genuine BOSS gauge cells
(2000445=1, 2000446=4000 — CE239 pc202 boosts the enemy gauge from 2000446), and never enables wiring:

    AUTOSTART (b): rounds=4 first_pcs=[338, 645, 792, 996] arg0s=[1, 2, 3, 4]
    pre_pools=[[5,4,3,2], [4,4,3,2], [3,4,3,2], [3,3,3,2]] final_pools=[3,2,3,2]
    settle=(1, 1) warnings=0

Each queued index reaches a DISTINCT CE242 command body (body-entry pcs 338/645/792/996) and drains
the correct member pool: idx1/idx2 -> 2000345, idx3/idx4 -> 2000346. This proves the multi-member
command stream works under the default auto drive with no test flag.

## 5. Explicit opt-out (test c)

`explicit_disable_overrides_auto_arm` calls `set_battle_wiring(false)` before `start_event(253)`:

    AUTOSTART (c): battle_active=true max350=0 rounds=0 v352=0 v343=0 warnings=0

The CE253 battle still reaches the battle-active flag, but the drive stays off (the shipped stall is
preserved) — the hard opt-out is honoured.

## 6. Test / regression counts

- m11_battle_autostart: 3/3 pass (natural day-4 auto-armed settle; two-member queue; opt-out).
- Regressions run before the full sweep: m6_week_loop 1/1, m7_battle_target_scheduler 6/6,
  m7_battle_full 1/1, m7_battle_round 1/1 — all green after the change.
- Full workspace `WOLF_DATA_ROOT=work/all cargo test -q --workspace --no-fail-fast -j 2`:
  every target passes EXCEPT two, both green again the moment `auto_arm` is set back to false,
  i.e. both are caused solely by the new default-on drive:

  1. `wolf-game --test m7_day5_schedule::day_five_morning_is_reachable_after_settling_the_forced_day4_battle`
     — a COMMITTED legacy test whose harness (file header lines 9-16, body lines 240-273) waits for
     the OLD stall (`340==1 && hour==15 && event_pc(239)==Some(13)`), then **injects** the four CE244
     win vars (2000350=0,2000342=0,2000352=1,2000343=1) to unstick it, and asserts the evening
     settles at minute 30 (line 290). With the real drive on, the battle genuinely settles and CE251
     resumes at 16:00 so the evening dinner ends at 18:00 (minute 0) — the assertion
     `left: 0, right: 30` fails. This test's premise (battle unreachable) is exactly what task-95
     removes; it must be re-baselined to the no-injection, real-settlement flow already used by
     `m7_day4_playable` (drop the inject block; expect the 18:00 evening). NOT edited here — it is
     outside this task's write scope.
  2. `wolf-cli --test m11_visual_regression::battle_frame_matches_its_baseline`
     — an UNTRACKED test from a concurrent task (crates/wolf-cli/tests/, no CE253 autostart awareness);
     its battle-frame pixel baseline (522838) was captured against the stalled battle. With the drive
     on, the captured battle frame has 512087 non-black pixels. The baseline owner must regenerate it
     after this feature lands. NOT edited here — it is another task's in-flight file.

- `wolf-cli verify`: PASS 70 / 0 pending.

### Recommendation for the Lead (re-baseline, outside this task's write scope)
- `m7_day5_schedule.rs`: replace the stall-wait + 4-var inject block with a real-settlement wait
  (mirror `m7_day4_playable.rs::finish_day_after_battle` + `sleep_naturally_into_next_morning`) and
  relax/retarget the evening assertion to the post-battle 18:00 anchor (hour >= 18).
- `m11_visual_regression.rs`: regenerate the battle-frame baseline under the auto-armed drive.
- If instead the desired policy is "opt-in, not default-on", the whole feature is one bool away:
  `BattleWiring::new` `auto_arm: true` -> `false` (or `Game::set_battle_auto_arm(false)`), and every
  existing test returns byte-for-byte to HEAD behaviour (both failures above disappear).

## 7. Residual / boundaries

The multi-round loop still relies on the genuine BOSS path or a long enough enemy gauge; the shipped
non-boss enemy gauge (CE239 pc195: 2000342 = party_power/8) is zeroed by one hit, so most non-boss
fights still settle in round 1 — that is retail behaviour, not a defect. This task only arms the
existing rebuilt drive by default on the natural route; it does not change the battle economy.
