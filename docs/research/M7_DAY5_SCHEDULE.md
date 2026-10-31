# M7 — Day-5 schedule audit & reachability

Status: **reached day 5 08:30 in an automated test** (crates/wolf-game/tests/m7_day5_schedule.rs),
with one clearly-marked **TEST-ONLY injection** of the CE244 battle win-settlement
variables. Everything else (sleep, dinner, CE329 就寝, CE317 早朝, the day and
weekday counters) runs the real event code.

Repo: E:/Study/github program/imoto life, HEAD at time of writing 1d36ebe
(+ the task-19 battle_seed.rs). Test data root defaults to work/full (it carries
the bundled Save/System.sav template), honours WOLF_DATA_ROOT, else falls back to
work/all.

Reproduce:

    cargo test -q -p wolf-game --test m7_day5_schedule -- --nocapture

Observed output:

    m7_day5_schedule: day1->day5 08:30 OK; var160=5 var6=4 (Fri) var4=8 var5=30; warnings=0; frames=7640 instructions=392530
    test result: ok. 1 passed

---

## 1. Where the date and clock actually live

There is **no independent calendar engine and no CData/Data schedule table** that
holds the day counter or weekday. (Grep of work/wolftl/dump/db/CDataBase.json /
DataBase.json shows no day/weekday row; the M7 backlog already classifies the
printed Day11/15/18 tables as 攻略级 [C] and forbids encoding them as logic.)
The whole calendar is event-driven through **engine variables** (the normal 0..800
variable block in the save):

| meaning        | engine var (flat index) | var reference in scripts | note |
|----------------|--------------------------|--------------------------|------|
| day counter    | var160 -> vars.get(160)  | 2000160 | starts at 1, +1 per sleep |
| weekday        | var6 -> vars.get(6)      | 2000006 | 0=Mon … 6=Sun |
| hour           | var4 -> vars.get(4)      | 2000004 | |
| minute         | var5 -> vars.get(5)      | 2000005 | |

wolf-vm/src/magic.rs: VAR_BASE = 2_000_000, so script operand 2000160 is
engine var index 160 (2000160 - 2_000_000); game.vm.vars.get(160) is the same cell.

Observed mapping across the loop: day1 var160=1; day2 var6=1; day3 var6=2;
day4 var6=3; **day5 var6=4 (Friday)**.

## 2. Exact write sites (from work/wolftl/dump/common/*.json)

**Day / weekday advance — CE329 ┗就寝処理:**
- idx406 SetVariable [2000160, 1, 0, 256] — **day counter 2000160 += 1**.
- idx377 SetVariable [2000006, 1, 0, 256] — **weekday 2000006 += 1**.
- idx378 VariableCondition [1, 2000006, 7, 1] (weekday >= 7) -> idx380 SetVariable [2000006,0,0,0] — weekday wraps to 0..6.

**Clock (hour/min) writes:**
- Dinner -> 18:00 — CE326 ┗食事処理: idx1614 cond hour <= 17, idx1616 SetVariable [2000004,18,0,0], idx1617 [2000005,0,0,0].
- Night clock (就寝前) — CE328 ┣就寝前パジャマ: branches set 22:00 (idx763/764), 22:30 (idx773/774), 23:00 (idx783/784), 23:30 (idx795/796) etc.
- Sleep -> next morning — CE329: idx385 hour=7, idx386 minute=45; then
- CE317 ■■早朝処理: idx622 hour=8, idx623 minute=0 (weekday morning 08:00); idx123/124 set hour=9 minute=0 on the alternative branch via idx120.
- 08:30 comes from the 時刻表示 helper: CE217 idx16 SetVariable [2000005,1600000,0,256] adds the minutes arg 1600000 to var5; CE317 calls it as CommonEvent [500217,1,15] (idx420/595/613/914/952/998/1053/1060/1084, +15 min each), so the weekday morning 08:00 advances to the observed 08:30.
- The clock only advances through CE217 時刻表示 (+delta, minutes in 1600000); CE329/CE317 rewrite date/time absolutely, which is why the test can sleep from an 18:00 bedroom and still land on 08:30.

**Holiday reset (separate path, do not confuse with the workday chain):** CE310 ■休日
idx2884 sets 2000160=1, idx2883 sets 2000006=1, idx2881/2882 set 09:30 — this is a
holiday/day-1 reset, not part of the workday day loop.

## 3. Day-of-week / day-number gates

These events branch directly on the counters (all confirmed by scanning the dumps):

- CE300 日中イベント判定 — idx23 day-threshold gate 2000160 <= {7,20,44,9999}; idx14/675 2000160 story gate.
- CE251 ■昼コマンド　平日 — many day gates (idx245 2000160>=106, idx414 ==4, idx424 in {11,15,18}, idx486 in {33,43,46}, idx509 in {50,53}, idx714 in {71,74,78}, idx750 in {79,85}, idx898/900/976, idx1275..1359, idx2330/2332); weekday gate idx2565 2000006 == 4 (Friday special).
- CE317 ■■早朝処理 — weekday gate idx120 2000006 in {5,0} (weekend 09:00 branch); day gates idx2 (<=15), idx504 (<=84), idx915 (>=100), idx931 (>=57), idx933 (<=61), idx1107 (>=33).
- CE326 ┗食事処理 — idx160/278/733 2000006 in {5,0}, idx469 2000006 >= 5, plus several 2000160 story gates; idx1614/1616 dinner clock.
- CE328 ┣就寝前パジャマ — idx102 weekday, idx111 2000006 >= 4, day gates idx306/308/336/359/387/827/850.
- CE245 ■夜コマンドメニュー, CE290 ■夜コマンドメニューB — 2000160 gates (e.g. 2000160 >= 83, != 101).

## 4. Why day 5 is blocked without settling the day-4 battle

On day 4 the workday command menu does **not** draw a selectable non-battle row
(pictures 1201/1203/1205/1207/1209 stay blank, no 5-option Choice). The day-4
workday row resolves to internal id 7 and dispatches the forced story:

    CE251 昼コマンド平日 (row1 id=7) -> CE253 地域貢献 -> CE239 battle
      CE251 idx419 JumpLabel 地域貢献 -> idx2465 SetLabel -> idx2466 CommonEvent[500253,0]
      -> CE253 -> CE239 at CE253 pc174/177/180/185/188/191 (six 210[500239])
      -> CE239 pc695 sets 2000340=1 and waits for a command the unfinished command UI
         never produces -> clock frozen at 15:00.

In the test, after day4 08:30 the driver reaches this wall and asserts:
saw_253 == true, saw_239 == true, **no 5-row training menu**, and CE239 parked at
pc13 with 2000340==1 and var4==15 — 0 warnings, no engine deadlock.

## 5. TEST-ONLY injection (the only faked state)

CE244 (the battle **turn settlement**) is unreachable in the shipped event graph:
2000350 has a circular bootstrap — only CE242 pc14 writes 2000350=1, and CE242
only starts when 2000350==1 (see docs/HANDOFF_M7.md). So the engine cannot settle a
battle turn on its own in this build. To let the *real* CE239 return normally, the
test writes exactly the four variables that CE244's **win branch** (CE244 pc312-317)
would write:

| var | script ref | value | meaning |
|-----|-----------|-------|---------|
| var350 | 2000350 | 0 | battle-engine idle |
| var342 | 2000342 | 0 | turn flag |
| var352 | 2000352 | 1 | win |
| var343 | 2000343 | 1 | settlement done |

These are ordinary engine variables (vars.set(350,0) etc.), no CData faking. Once
set, CE239 proceeds pc880 -> 941 -> 946, its cleanup at pc1043-1046 clears
2000350/352/343/340, and it returns normally.

After settlement the **real** chain runs: clock 15:00 -> 16:00 -> 17:30 -> 18:00
(CE300/CE326, dinner CE326 idx1616 = 18:00), then the test seeds var63=1
(the sister-click flag, exactly as the m6_mainline / m6_multiday sleep helper does)
to open the night sleep prompt, and the real CE329 就寝 advances var160 += 1,
var6 += 1 and CE317 早朝 sets 08:30. Result: **day 5, var160=5, var6=4
(Friday), 08:30, warnings 0.**

## 6. Honest boundary (real vs injected, pending task-20)

- **Real:** every night's sleep (CE329/CE317), weekday/day increments, the
  dinner/evening chain (CE300/CE326), the 18:00 bedroom, the night sleep prompt,
  and the day-5 08:30 arrival.
- **Test-only injection:** the four CE244 win-settlement variables at the CE239
  stall (section 5). This stands in for the missing real battle settlement
  (CE244 / 2000350 circular bootstrap) tracked as task-20.
- **Faithful constraint:** day 4 daytime is the retail-forced CE253->CE239 battle;
  there is no selectable non-battle day-4 row to route around it (see
  crates/wolf-game/tests/m6_week_loop.rs and docs/research/M6_SCHEDULE.md §8).
- **Minor timing note:** the post-battle evening ends at the 18:00 bedroom (dinner)
  rather than the usual 19:30, because the post-battle CE300 day page returns to the
  bedroom earlier. This does not matter: CE329/CE317 set date/time absolutely.
