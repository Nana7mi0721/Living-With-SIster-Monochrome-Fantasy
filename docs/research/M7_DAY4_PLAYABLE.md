# M7 task-38 — Day-4 real battle: exact blocking edge (no-injection audit)

> **SUPERSEDED by M8 W1-c (`docs/research/M8_BATTLE_SETTLE.md`).** The deadlock traced
> below came from the rebuilt wiring forwarding `2000354` (the battle context/enemy type)
> as CE242's arg0. CE242's arg0 is the **acting character slot** (pc143 splits on
> `self1600000 IN {1,2,3}`), so with slot 1 the same from-scratch replay settles
> (`2000352=1`, `2000343=1`) and reaches day 5. `crates/wolf-game/tests/m7_day4_playable.rs`
> now asserts the settlement and the day4→day5 run; this file remains the exact blocker
> record for the old wiring version.

Status: **COMPLETE (honest blocker report)**. Deliverable: this doc + the green test
`crates/wolf-game/tests/m7_day4_playable.rs`.
HEAD: `1d36ebe` (+ task-34 wiring). No production source edits, no commit/push.

## Verdict in one paragraph

The task-38 goal was to prove day4: **real battle settlement → evening → day5 08:30 with
NO injection**, driven through the task-34 rebuilt `BattleWiring`. The genuine schedule
replay does reach the real day-4 battle (CE253 地域貢献 → CE239) and the rebuilt wiring does
fire CE242 on the `2000340` battle-active edge — **but the round cannot settle** because the
from-scratch replay's party battle power `2000028` is only **132**. CE253's own branch at
`pc183` treats 100..149 as a **dead zone** that passes command **0**, so `2000354` is never a
valid 1..3. The wiring faithfully forwards `vars[354]=0`, CE242 fails its `pc143` action gate
(`self1600000 IN {1,2,3}`), takes the `pc210` DLC/追加コマンド ELSE branch, and **never reaches
pc201 CALL 500244 (CE244) / pc207 CALL 500243 / pc208 SET 2000350=2**. `2000350` then sticks at
1 and CE239 parks forever at **pc13**. This is a game-state (self-consistent data) precondition,
not an engine defect and not the `2000349` gate the Lead suspected. The test asserts this
exact chain and passes; a properly-powered party (or the SaveData01 harness with power 1815)
settles the identical CE244 for real (`2000352=1`, `2000343=1`).

Run:
```
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_day4_playable -- --nocapture
```
(reaches day4 08:30 in ~3.5 s; ~7100 frames)

## What the green test proves (observed values)

```
DAY4 BATTLE RESULT: ce253=true ce239=true ce242=true ce243=false ce241=false rounds=1
battle_active_f=Some(726) v350_seq=[0, 1] v354_max=0 v352_peak=0 v343_peak=0
max239pc=845 max242pc=6 ce239_alive=true ce239_pc=Some(13) warnings=0 unanswered250=1
```

* `pre_power = 2000028 = 132` at battle start (the CE253 low branch is taken).
* CE253 → CE239 dispatch is real; CE239 reaches its battle-start staging (`max239pc=845`,
  well past `pc700`).
* `2000340` rises at frame **726** (set by CE239 `pc695 SET 2000340=1`).
* `BattleWiring` fires exactly once (`rounds=1`) on that edge and starts CE242.
* CE242 **dies at max pc 6** — before its `pc143` action gate; `ce243=false`.
* `2000350` sequence is `[0, 1]` — it sticks at **1**, never reaches the action phase **2**.
* `2000352`, `2000343` are never written (`v352_peak=v343_peak=0`) → **no settlement**.
* CE239 stays alive, parked at **pc13** (its end-judgment loop waiting for `2000343==1`).
* `warnings=0`; the single "unanswered 250" is only the cosmetic command-menu section-header
  label `CData (5,16,0) labels "┗ 状態リスト"/""` — not on the settlement path.

## The exact day-4 blocking chain, step by step

### 1. CE253 地域貢献 dispatches CE239 with a branch-chosen command
CE253 (`event 253`, run=0):
```
pc171 c111 [19, 2000028,300,1, 2000028,250,1, 2000028,200,1]   ; IF power>=300 / >=250 / >=200
pc172 c401[1] -> pc173 SET 1600020=1 ; pc174 c210 [500239, 16777220, 6,1,7,1600020,1600030]  ; arg3=value(1600020)=1
pc176 c401[2] -> pc177 c210 [500239, 16777220, 4,1,7,2,     1600030]  ; arg3=2
pc179 c401[3] -> pc180 c210 [500239, 16777220, 3,1,7,3,     1600030]  ; arg3=3
pc182 c420 (else) ->
pc183 c111 [3, 2000028,150,1, 2000028,100,1, 2000028,0,1]      ; >=150 / >=100 / >=0
pc184 c401[1] -> pc185 c210 [500239, 16777220, 2,1,7,0,     1600030]  ; arg3=0
pc187 c401[2] -> pc188 c210 [500239, 16777220, 1,1,7,0,     1600030]  ; arg3=0   <-- 132 lands HERE
pc190 c401[3] -> pc191 c210 [500239, 16777220, 1,1,7,1,     1600030]  ; arg3=1
```
210-call args are `[target, header(16777220=0x01000004 -> 4 args + return var), arg0,arg1,arg2,arg3, return_var]`.
The child's arg3 is stored at CE239 `self1600003` (= SELF_ARG_BASE(600)+3 = flat 603). With
**2000028=132**: `pc171` three conditions (>=300/>=250/>=200) all false → `pc182` else →
`pc183`: `>=150` false, `>=100` **true** → case2 → **`pc188` with arg3=0**. (100..149 is the
dead zone; a valid 1..3 command needs `2000028 >= 200`.)
CE253 then blocks at its PC after the call (`pc189`, observed as `c253=Some(189)`).

### 2. CE239 stores that 0 as the selected command
CE239 (`event 239`, run=0, cond 2000000==0):
```
pc98  SET 2000350=0
pc102 c111 [1, 1600000, 0, 2]          ; IF self1600000 (arg0) == 0
pc103 c401[1] -> pc104 SET 2000354 = self1600003   ; the only early writer of 2000354
...
pc695 SET 2000340=1     ; battle-active rises here (frame 726)
...
pc797 c420 else -> pc798 SET 2000354 = self1600003  ; arg3 = 0 for the low branch
```
Because the low-power 210-call passes **arg0 = 1** (self1600000=1 ≠ 0), `pc104` is **skipped**;
the later `pc798` writes `2000354 = self1600003 = 0`. Net: **`2000354` is 0 for the whole battle**
(observed `v354_max=0`).

### 3. The wiring forwards command 0; CE242 skips its action block
`BattleWiring::tick` (crates/wolf-game/src/battle_wiring.rs) reads `vm.vars.get(354)` on the
0→1 edge of 2000340 and writes it to the fresh CE242's `self600` (SELF_ARG_BASE). It passes **0**.
CE242 (`event 242`, run=0, cond 2000350==1):
```
pc14  SET 2000350=1
pc143 c111 [3, 1600000,1,2, 1600000,2,2, 1600000,3,2]   ; IF self1600000 IN {1,2,3}
pc144 c401[1] (action path)
   ... pc199 key poll ...
   pc201 c210 [500244, 16777216, 1600001]   ; CALL CE244  (settlement)
   pc202 c111 [1, 1600001, -1, 19]          ; if CE244 returned -1
   pc204 c172 BreakEvent
   pc207 c210 [500243, 0]                    ; CALL CE243 (enemy turn)
   pc208 SET 2000350=2
pc210 c420 (ELSE) -> pc211 "DLC追加コマンド" / pc212 "追加の聖域"  <-- command 0 lands HERE
```
With `self1600000=0`, `pc143` is **false** → `pc210` ELSE → the DLC branch. CE242 ends immediately
(observed max pc 6, because the first frames already branch out); **CE244 and CE243 are never
called** and `2000350` is never set to 2.

### 4. Deadlock at CE239 pc13
CE239's case1 (2000340==0) finishes at `pc874`; `pc875 c401[2]` → case2 (2000340>=1) and at
`pc948+` (because 2000343 != 1) it clears `2000343/352/350/340` at `pc1043-1046` and loops back
to `pc11 → pc13`. Since nothing ever sets 2000343=1 (CE244 never ran), CE239 spins forever at
**pc13**. This matches the observed `ce239_alive=true, ce239_pc=Some(13)`.

## Root cause (authoritative)

> The day-4 地域貢献 battle is gated on party battle power `2000028`. The evaluated power in a
> from-scratch replay is **132**, which CE253 maps (via `pc171` / `pc183`) to command **0**. CE242
> requires a valid command (1..3) in `self1600000` to take its action block; with 0 it exits via
> the DLC/追加コマンド path and never calls CE244, so `2000350` sticks at 1 and CE239 deadlocks at
> pc13. This is a data/game-state precondition (weak party), not an engine bug.

## Refutation of the Lead's task-31 hypothesis (`2000349` gate)

The suspicion was: *CE244 is entered but never settles because `2000349=1` while CE244 `pc36`
expects `self1600010` (11 default / 5 for cmd20)*. This is **false**:

CE244 (`event 244`, 432 cmds) head:
```
pc0  SET self1600000=0
pc3  SET self1600010=11
pc4  c111 [1, 2000354, 20, 2]  -> pc6 SET self1600010=5
pc9  c111 [17, 2000495, 2, 2]  -> pc11 SET self1600010=5
pc36 c111 [1, 2000349, 1600010, 2]   ; IF 2000349 == self1600010
pc44 c111 [17, 2000192, 2000193, 0]; IF 2000192 > 2000193
pc46 c213 jmp "勝" / pc49 c213 jmp "負"
...
pc310 c150 ... ; pc311 c212 label "勝"
pc312 SET 2000350=0 ; pc313 SET 2000342=0 ; pc314 SET 1600000=1
pc315 SET 2000352=1 ; pc316 SET 2000343=1 ; pc317 SET 2000386
```
The win block `pc310-317` is reached **linearly** (the `pc36`/`pc44` quick-jump path only fast-forwards
to the same label). CLI proof — running the real command phase directly:
```
wolf-cli dump --event 242 --frames 300 --self 600=1 --var 2000495=0 --var 2000349=0  -> f66 settles 2000352=1 2000343=1
wolf-cli dump --event 242 --frames 300 --self 600=1 --var 2000495=0 --var 2000349=1  -> f66 settles 2000352=1 2000343=1
wolf-cli dump --event 242 --frames 300 --self 600=1 --var 2000495=0 --var 2000349=11 -> f66 settles 2000352=1 2000343=1
wolf-cli dump --event 242 --frames 300 --self 600=1 --var 2000495=2 (cmd1)            -> f91 settles 2000352=1 2000343=1
```
`2000349` makes **no difference**. It is written only by CE239 `pc564 SET 2000349=1` and CE241
`pc78 SET 2000349=1` (full 550-event scan; no increment anywhere). It is a red herring — the real
gate is upstream at `2000354` / CE242 `pc143`.

## Contrast: why the task-34 direct test passes

`crates/wolf-game/tests/m7_battle_playable.rs` loads **SaveData01**
(`work/retail_backup/Save/SaveData01.sav`): `v160=90, v6=5, v4=21, v5=40, v28=1815`.
Its party battle power is **1815 (>= 300)** and its persisted vars already carry a valid
`2000354`; CE242 therefore takes `pc144` (action) → `pc201` CALL CE244 → real settlement
(`2000350` seq `[0,1,0,2,0]`, `2000352=1`, `2000343=1`). SaveData01 is **not a day-4 state**
(`v160=90`), so it cannot stand in for the genuine day-4 replay — it only demonstrates that the
CE244 settlement itself is sound when a valid command is supplied.

## Acceptance-criteria mapping (task-38)

| Criterion | Result |
|---|---|
| Replay genuine day1→day4 08:30 with no settlement injection | ✅ proven (`v160=4 v6=3 v4=8 v5=30`) |
| Real day-4 CE253 → CE239 dispatch | ✅ proven |
| Rebuilt wiring fires CE242 on the 2000340 edge | ✅ proven (`rounds=1`, frame 726) |
| CE244 settlement `2000352 in {1,-1}`, `2000343=1` | ❌ blocked upstream (party power 132 → cmd 0) |
| `2000350` reaches 2 / settles / CE239 returns | ❌ `2000350` sticks at 1; CE239 parks pc13 |
| warnings 0 | ✅ proven |
| 250 unanswered == 0 in battle section | ⚠ only one cosmetic label read (`CData 5,16,0`), not settlement |
| day4 → day5 08:30 | ❌ depends on settlement |

The test **passes** by asserting the proven chain + the exact blocker (per task-38's "leave the
test asserting the proven part"). It does **not** fake a settlement.

## Minimal remedies (report only — NOT implemented; production edits were forbidden)

1. **Intended gameplay route — build the party.** Raise the party battle power `2000028` to
   **>= 200** before day 4 (the high branches `pc174/177/180` then pass a valid command 1/2/3 and
   the identical wiring settles CE244 for real). This is normal progression, not a code change.
   NOTE the data quirk: powers 100..149 (our 132) map to command 0; <=99 maps to command 1 via
   `pc191`, and >=200 maps to 1/2/3 — only 100..199 is the dead zone.
2. **Engine/wiring-level (would be a real change, out of task-38 scope):** have the wiring supply
   a data-valid default attack command when `2000354 == 0` (i.e. derive a command from the
   battle databases instead of forwarding 0). This effectively invents menu input, which the
   task forbids faking; it must be labelled rebuilt wiring, not shipped data.

Do **not** hardcode `2000350=1`, seed `2000352/2000343`, or fake the command menu.

## Residuals / honest boundaries

* The single "unanswered 250" is the cosmetic `CData (5,16,0)` section-header label drawn by the
  DLC/追加コマンド branch — a consequence of the wrong branch, not a battle mechanic.
* Non-warning diagnostics observed in this section: 201 SetMoveRoute no-map-layer, debug/160/162
  transition, 270 Party — pre-existing soft-degrades, none affect the chain.
* The test is a **blocker audit**, not a settlement proof. A green full day4→day5 requires the
  party to be strong enough (remedy 1), which is progression state, not engine work.

## Files

* `crates/wolf-game/tests/m7_day4_playable.rs` — new test `day4_real_battle_reaches_ce239_and_exposes_command_gate` (green).
* `docs/research/M7_DAY4_PLAYABLE.md` — this document.
* Reference (unchanged): `crates/wolf-game/src/battle_wiring.rs`, `crates/wolf-game/src/game.rs`,
  `crates/wolf-game/tests/m7_battle_playable.rs`, `crates/wolf-game/tests/m7_day5_schedule.rs`.
