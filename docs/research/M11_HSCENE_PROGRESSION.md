# M11 P1 — H CG real progression (CE402/CE410 reachability)

HEAD 22de104. Data root work/all. Saves work/retail_backup/Save/SaveData01..08.sav.
Test: crates/wolf-game/tests/m11_hscene_progression.rs (4 tests, green).

## One-line result

CE402/CE410 cannot be reached end-to-end from any shipped retail save via real inputs.
The two gate variables behave asymmetrically: var304 = 1 IS reachable from SaveData01
through a real night-menu choice, and the CE301 skill machine really does write
var326 = 2 — but only once the bookstore flags var276>=1 && var277>=1 are set, and
var277 is 0 in every shipped save and is only ever set by the un-reachable bookstore
event CE314. Even after seeding those flags, the CE328 H-shitai script terminates at
pc741 (op172) and never reaches the CE402 call at pc986.

## Fixed gate set (from prior M11 work)

CE328 entry pc4 var105==0, pc6 var104==0, pc15 var304>=1 -> H label pc17; CE402 is
called ONLY from CE328 pc986 op210[500402,0]; CE402 body pc0 var16>=1, pc2 var22>=180,
pc4 var4>=21, pc6 var326>=2, pc8 var102>=80. SaveData01 is the only shipped save that
satisfies all the non-learned constants (v16=4, v22=347, v4=21, v102=120, v104=0,
v105=0); its only gaps are var304=0, var326=0, var276/277. SaveData07 fails the CE402
body (v102=59<80, v4=19<21).

## The two learnable gates

### var304 — REACHABLE (real night menu)
CE290 (night handler, startable) top-menu pc107 op102[7] has 7 options
(renders: 身体接触 / 沟通 / 泡茶 / 洗澡 / 自主锻炼 / “0” / 今天先休息吧). Choosing index 5
shifts to case 7 (pc392 op401[7]) -> intimacy block: pc393 var23>=5, pc395 var22>=100,
pc397 var102>=50, then pc403 op121[2000304,1] (var304=1), pc404 var432=1,
pc406 op210[500328] calls CE328. Empirical (test 1, SaveData01): var304 peaks at 1,
CE328 spawns and runs to pc>=640, warnings 0. (SaveData07 likewise reaches var304=1.)
The var304 write is cleared again by downstream CE328/CE332 logic, so it is a
transient during the scene.

### var326 — reachable only via CE301, gated on the bookstore flags
var326 has exactly two writers in the whole event graph: CE301 pc6320 and CE301
pc6365 (both op121[2000326,2], preceded by pc6316/pc6361 op111[2000326==0] and the
“妹妹习得了技能 骑乗位！” line at pc6319). Entering the skill block needs
pc6281 op111[2000276>=1 && 2000277>=1] (the 小本本 event) AND
pc6298/pc6342 op111[200102>=60 && 200103>=1].

Empirical (test 2, SaveData01, var276=var277=1 seeded to represent a completed
bookstore purchase): driving CE301 reaches pc6365 and var326 becomes 2, warnings 0.
Without the flags (SaveData01 as shipped) CE301 runs to pc6376 but var326 stays 0
(the skill branch is inside the var276>=1 && var277>=1 block).

## The missing gate (why var326 is unreachable from a save)

- var277 is written to 1 ONLY by the bookstore CE314 pc1233 and pc1530 (CE247 pc704 is
  the sole clearer, value -1). var276 is written +1 by many events incl. CE314, but the
  skill gate needs BOTH var276>=1 && var277>=1.
- CE314 (bookstore) is called ONLY from CE308 pc115 and CE310 pc670.
- Empirically: start_event(314) aborts at maxpc ~118 (never reaches the bookstore menu
  at pc1140+); driving CE308 / CE310 from SaveData01 never instantiates CE314
  (maxpc 75 / 188) and leaves var277 == 0.
- Shipped-save scan (all eight): var277 == 0 and var326 == 0 in every save. (var276 is
  already 2 in SaveData04/05, but the gate still fails because var277=0.)

So var326 can only be produced by first completing the bookstore purchase, which
requires reaching CE314 through the day-event chain (CE308/CE310) with money var7
(SaveData01 var7=7586), and no shipped save / direct start does that.

## Residual: even with var326 preset, CE402 is not entered

Test 4 runs the closest real/semi-real chain on SaveData01: seed var276/277 -> real
CE301 (var326=2) -> real night menu index 5 (var304=1, CE328 spawned). The CE328
H-shitai branch (pc17 -> pc89) plays its dialogue and terminates at pc741 (op172) — a
blackscreen 就寝 (sleep) branch — with CE328 max pc < 985 and CE402 never entered,
warnings 0. The CE402 call family lives at pc944 op401[3] -> pc946/966/973 ->
pc985 op103 騎乗位/夢精判定 -> pc986 op210[500402], a sub-branch not reached by this H
flow. Reaching it needs additional H-session state beyond var304/var326.

## What the tests pin

1. real_night_menu_sets_var304_and_calls_ce328 — CE290 opt-index 5 writes var304=1 and
   calls CE328; warnings 0 (real progression achievable).
2. real_ce301_skill_event_sets_var326_given_books — CE301 writes var326=2 when the
   book flags are present (skill machine real); warnings 0.
3. bookstore_gate_pins_var326_unreachable_from_shipped_saves — static: var326 writers
   {301:6320,301:6365}, var277 +1 writers {314:1233,314:1530}, CE314 callers
   {308:115,310:670}, CE402 caller {328:986}; all 8 saves var277=0/var326=0; direct
   CE314 start < 200 pc and CE308/CE310 never set var277.
4. full_chain_still_does_not_reach_ce402 — closest real chain; CE402 not entered,
   CE328 max pc < 985, warnings 0.

## Commands

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_hscene_progression   # 4 passed

No production code, retail saves, or shared files were modified; temp probes deleted.