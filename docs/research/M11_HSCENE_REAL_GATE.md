# M11 W2 - H deep CG real-save gate matrix (CE328 -> CE402 -> CE410)

One-line result: **No shipped retail save can reach CE402 / CE410.** Two gate variables are
0 in all eight saves - **var304 == 0** (so CE328, the unique caller of CE402, never enters) and
**var326 == 0** (so CE402's own body gate var326 >= 2 never passes). Every gate variable DOES
have a normal gameplay writer (traced below), so this is not a data bug: the retail snapshots
simply never capture the required state. Only a synthetic, engine-injected save opens the
branch (proved in tests/m8_hscene_cg_deep.rs), so the deep CG cluster is reproducible only
synthetically, never from the day/night loop with these saves.

Data: work/all (23 MapData maps incl SampleMapA.mps); saves work/retail_backup/Save/SaveData01..08.sav.
Test: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_hscene_real_gate  (4 passed).

---------------------------------------------------------------------------
## 1. Gate graph (static, BasicData/CommonEvent.dat)
---------------------------------------------------------------------------

CE402 body gates (op111). Operator code = exec.rs compare(): 0 '>', 1 '>=', 2 '==', 3 '<=', 4 '<', 5 '!=', 6 'bitand'. So op1 is >=, op2 is ==.

  pc0   var16  >= 1      (age/day)
  pc2   var22  >= 180    (affection)
  pc4   var4   >= 21     (day counter)
  pc6   var326 >= 2      (learned 騎乗位 skill)
  pc8   var102 >= 80     (affection)
  pc11  self1600010 ...  (self-gate)
  (later op2 arms for var103/var142/var144/var265/var151/var367 live in sub-branches, see note)

CE402 -> CE410 (騎乗位系カットイン) op210 500410 call sites: pcs 165,173,177,195,198,288,308,311,360,447 (10).

CE402 has EXACTLY one caller in the whole corpus: **CE328 pc986** (op210 500402 scan over all events).

CE328 entry gates (op111):
  pc4   var105 == 0
  pc6   var104 == 0
  pc15  var304 >= 1
  pc20  var4 in {18,19,20}  (three op2 arms)

Note on the deeper static list: CE402 also carries later gates (pc21 var103>=2, pc57 var151>=1,
pc78 var265==0, pc92 var142==0, pc102 var144==1, pc295 var367==0), but the synthetic DEEP_GATE
proof below reaches CE410 with var103=0/var265=1/var142=2/var144=4/var151=0, so those later gates
sit on sub-branches that are NOT on the executed path. The empirically necessary subset is the
nine-value DEEP_GATE set.

---------------------------------------------------------------------------
## 2. Writer trace - every gate variable has a normal gameplay writer
---------------------------------------------------------------------------

var326 (learned 騎乗位 skill): ONLY CE301 pc6320 and pc6365, both op121 [2000326,2,0,0] (set =2).
  Guarded by CE301 pc6316 op111 [1,2000326,0,2] (if var326==0) then dialogue pc6319
  "\E妹妹习得了技能 骑乘位！". Callers of CE301: CE300 (day-action hub, ~140 pcs), CE301 self
  pc5510/5623, CE510 pc11. NO map event writes var326. => a routine day event.

var304 (per-night intimate-route flag): set =1 ONLY by CE245 pc779 and CE290 pc403 (the two
  亲密路 night-menu routes). Cleared =0 by CE332 pc125 (night reset) and MAP EV3 p0 pc16.
  All 8 saves have var304=0 because they are daytime snapshots, never mid-intimate-night.

var157 (chapter flag): CE257 pc1=16, CE281 pc342=self1600013, CE317 pc437/983=2, CE328 pc311=16,
  CE328 pc663=self1600013, CE452 pc10=1, MAP EV0 p0 pc42=1; read by CE246 pc1943==13,
  CE247 pc779==13, CE328 pc268==13, CE329 pc339==13 (values 1..13 seen in saves).

var4  (day counter): 31 CE writers + map writers.   var16: 126 CE + map.   var22: 199 CE + 42 map.
var102 (affection): 582 CE + 218 map.               var104/var105: CE248/CE326/CE347/CE361 + MAP EV7.
var364 (dispatcher): only CE335 pc3729=1 / pc3762=0.

---------------------------------------------------------------------------
## 3. Save matrix - raw values  (v4 v16 v22 v102 v104 v105 v157 v304 v326 v364)
---------------------------------------------------------------------------

  SaveData01: 21  4 347 120  0 0 12  0 0 0   DEEP pass 7/9   entry: no  (var304=0; var4=21 out of {18..20})
  SaveData02: 18  4   0  12  0 0  1  0 0 0   DEEP pass 4/9   entry: var4 ok but var304=0
  SaveData03: 22  4 102  52  0 0 10  0 0 0   DEEP pass 5/9   entry: no
  SaveData04: 21  3 122  90  1 2 12  0 0 0   DEEP pass 4/9   entry: no  (var104=1, var105=2)
  SaveData05: 18  3 121  59  1 3 12  0 0 0   DEEP pass 2/9   entry: no  (var104=1, var105=3)
  SaveData06: 18  2 157  33  0 0  8  0 0 0   DEEP pass 4/9   entry: var4 ok but var304=0
  SaveData07: 19  2 213  59  0 0 13  0 0 0   DEEP pass 6/9   entry: var4 ok but var304=0
  SaveData08:  0  2 375 120  0 0 12  0 0 0   DEEP pass 6/9   entry: no  (var4=0)

DEEP_GATE = {var157==13, var104==0, var105==0, var4>=21, var102>=80, var16>=1, var22>=180,
             var326>=2, var364==0}  (9 predicates; the necessary-and-sufficient set from
             m8_hscene_cg_deep.rs, which drives CE402 -> CE410 on a save holding all nine).

Observations:
  * var304 == 0 in ALL 8  -> CE328 entry fails for every save (universal entry blocker).
  * var326 == 0 in ALL 8  -> CE402 body gate var326>=2 fails for every save (universal body blocker).
  * No save passes all 9. Closest is SaveData01 (7/9), failing exactly var157==13 and var326>=2.
  * The only var4-in-range (18..20) entry candidates are SaveData02/06/07; all are killed by
    var304==0 and each also fails the body independently (02: v22=0,v102=12; 06: v22=157,v102=33;
    07: v102=59; all var326=0).

---------------------------------------------------------------------------
## 4. Empirical live-VM behaviour (bare load, no engine/map coroutines)
---------------------------------------------------------------------------

start_event(402) on each retail save -> CE402 max pc < 20 (aborts at the first failing gate - the
  var326>=2 gate at pc6 fails - within the first frame), CE410 never entered, 0 warnings.
start_event(328) on each retail save -> CE328 runs its own body (max pc 1023 of 1025), but the
  pc986 CE402 sub-branch is never taken, so CE402 is never instantiated, 0 warnings.
Synthetic SaveData07 + DEEP_GATE -> CE402 max pc 509 of 512, CE410 (騎乗位系カットイン) entered,
  0 warnings (this is test 4 = the sufficiency proof; identical to task-70 / m8_hscene_cg_deep.rs).

Why the gates are clobbered by the live loop but not the bare load: the engine + map background
coroutines rewrite the gate vars in the first frames, so the deep events must be driven on the
loaded save alone (Game::new + load_save, no start_engine_events / start_pages). This file follows
that rule for every empirical assertion.

---------------------------------------------------------------------------
## 5. Verdict
---------------------------------------------------------------------------

1. No shipped retail save - and therefore no real player operation that merely reads such a save -
   can reach CE402 / CE410. Entry is blocked by var304==0 in all eight saves; even if you waived
   var304, the body needs var326>=2 which is 0 in all eight; and each entry candidate additionally
   fails var22/var102/var4.
2. This is NOT a data/logic defect. Every gate variable has a genuine gameplay writer:
   var304 is set by the live night intimate route (CE245/CE290); var326 is earned by a normal day
   event (CE301); var4/var16/var22/var102 are ordinary progression/affection counters. The retail
   saves simply never capture the state where they all coincide.
3. The conjunction is structurally narrow: CE328 entry wants var4 in {18,19,20} while CE402 body
   wants var4>=21 (CE328's own body bumps var4 at pc117/763/773/783/795/901/911/921/933, so a live
   run can thread it), and it additionally needs a learned 騎乗位 skill plus high affection. No
   snapshot is in that window.
4. Only a synthetic engine-injected save opens it (m8_hscene_cg_deep.rs). Consequence: the deep CG
   cluster CE402/CE410 is reproducible only synthetically - never from the day/night loop with the
   shipped saves.

---------------------------------------------------------------------------
## 6. What tests/m11_hscene_real_gate.rs pins (4 tests, all green)
---------------------------------------------------------------------------

1. gate_graph_and_writers_are_as_documented - static assertions on CE402 body gates, the CE410 call
   sites, the unique CE328 pc986 caller of CE402, CE328 entry gates, var326's CE301-only writers,
   var304's CE245/CE290 setters + CE332 reset, and ev33->CE333 leaf.
2. gate_matrix_over_the_eight_retail_saves_is_all_fail - pins the raw 8x10 value matrix, asserts
   var304==0 and var326==0 in all eight, entry fails everywhere (var304), DEEP_GATE pass counts
   [7,4,5,4,2,4,6,6], best save = SaveData01 at 7/9 failing exactly {var157, var326}.
3. ce402_aborts_and_ce328_skips_the_deep_branch_on_every_retail_save - live VM: CE402 aborts
   (<20 pc, no CE410) and CE328 runs body but never instantiates CE402, 0 warnings, on all eight.
4. synthetic_deep_gate_drives_ce402_to_ce410_proving_state_is_the_only_blocker - builds the
   DEEP_GATE synthetic save (work/out, gitignored), drives CE402 to pc>=300 and enters CE410,
   0 warnings - proving the sole missing ingredient is state, not a defect.
