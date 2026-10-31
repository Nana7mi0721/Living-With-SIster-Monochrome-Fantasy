# M8 W1-c — the day-4 battle now settles: CE242's argument is the acting slot

Status: **DONE** (test green, no settlement injection). Deliverables:
`crates/wolf-game/src/battle_wiring.rs` (one constant + the argument it passes),
`crates/wolf-game/tests/m7_day4_playable.rs` (rewritten from blocker audit to the
day4→day5 acceptance test), `crates/wolf-game/tests/m7_battle_playable.rs`
(assertion updated). HEAD before this change: `b9e0d36` (local M7 batch-2 integration).

Reproduce:

```
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_day4_playable -- --nocapture
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_battle_playable -- --nocapture
```

## 1. The one-line defect

The rebuilt wiring started CE242 (command processing) with `self1600000 = 2000354`.
`2000354` is the **battle context / enemy type** the caller selected (CE239 pc104/pc798
copy its arg3 there; CE242 pc91/pc105/pc115/pc126 dispatch on it). CE242's own arg0 is
the **acting character slot**: pc143 splits on `self1600000 IN {1,2,3}` (main-party
action block) versus the `DLC用追加処理` branch.

On the genuine day-4 replay the party power `2000028` is 132, so CE253's low-power
branch passes context 0. The old wiring forwarded that 0 as the slot, CE242 took the
DLC branch and died before its action block; `2000350` stuck at 1, CE239 parked at
pc13 and the day never advanced.

## 2. Evidence that the slot is the right argument

| probe | result |
|---|---|
| `wolf-cli dump --event 242 --frames 200 --self 600=1 --var 2000495=0` | `2000350` 0→1→0, `2000352=1`, `2000343=1` at frame 66 |
| same with `--self 600=0` (old behaviour) | DLC branch, no settlement |
| CE239 pc581 | seeds `2000496 = 4` (the action code) **before** the command phase — the command is not the phase argument |
| CE239 pc104 / pc798 | write `2000354 = self1600003` — battle context, read by CE242 pc91..pc129 |
| CE242 pc143 | `self1600000 IN {1,2,3}` → main action block; else → `DLC用追加処理` |
| CE242 pc201 / pc207 / pc208 | `CALL 500244` (settlement) → `CALL 500243` → `SET 2000350=2`, all inside the main block |

The seeded party has one member, so the wiring supplies slot **1**. With a multi-member
party the engine would start CE242 once per acting member; that loop is not reconstructed
yet (the current seed and this game's day-4 wrapper only need the leader).

Honesty contract unchanged: the wiring does not set `2000350`, does not fake menu input
(the action lives in `2000496`), and does not seed `2000352`/`2000343`.

## 3. The full day-4 → day-5 acceptance run

`m7_day4_playable::day4_real_battle_settles_and_reaches_day5_without_injection`:

```
DAY4 BATTLE RESULT: ce253=true ce239=true ce242=true ce243=false ce241=false rounds=1
battle_active_f=Some(726) v350_seq=[0, 1, 0] v354_max=0 v352_peak=1 v343_peak=1
max239pc=745 max242pc=471 ce239_alive=false warnings=0 unanswered250=1
m7_day4_playable [SETTLED]: ... day5 08:30; warnings=0, frames=8008
```

* days 1..4 08:30 replay the genuine schedule (real sleep menu, real workday menu);
* day 4 daytime forces CE251 → CE253 地域貢献 → CE239 (no injection);
* the rebuilt wiring fires once on the `2000340` 0→1 edge and CE242 executes the leader
  action: `2000350` 0→1→0, CE244 writes `2000352=1` and `2000343=1`;
* CE239 drains its victory sequence and returns; CE253/CE251 finish at 16:00 (the forced
  route is one hour earlier than the normal 17:00 menu);
* CE300 walks the clock to 17:00, CE326 dinner runs and ends around 18:00;
* the night menu is opened with a real hover-sister + Decide gesture, 今天先休息吧 → 是,
  and CE329 就寝処理 rolls to **day 5 08:30** (`var160=5`, `var6=4` Friday, `var4=8`,
  `var5=30`), warnings 0.

Only input-state normalisation: `var295` (the undismissed first-dinner TIPS flag) if it
is still 1, and `var68` reset before the click — the same recipe as `m6_night_sleep`.
No `vars.set` touches `2000340/343/350/352`.

## 4. Why the evening is one hour early

The forced 地域貢献 branch freezes the clock at 15:00 while CE239 is open. After the
settlement CE251 resumes and its tail adds one hour → 16:00. CE300 then advances to
17:00 and runs the CE326 dinner, which ends at 18:00 instead of the normal 19:30.
`finish_day_after_battle` therefore waits for "CE300 and CE326 both gone, clock ≥ 18:00"
instead of pinning 19:30. The natural night menu works at 18:00.

## 5. Remaining boundaries (unchanged)

* Party power `2000028 = 132` on a from-scratch day-4 replay; the CE253 branch therefore
  still passes context 0. Raising it is normal progression, not a port defect.
* The single `unanswered 250` is the cosmetic command-menu header label
  `CData (5,16,0)`; it is not on the settlement path.
* `ce243`/`ce241` do not run in this battle because the leader's action settles the
  round immediately (CE244 returns, CE242 breaks out). That is the data's win path,
  not a missing phase.
* A multi-member party would need the per-member CE242 loop (not reconstructed).