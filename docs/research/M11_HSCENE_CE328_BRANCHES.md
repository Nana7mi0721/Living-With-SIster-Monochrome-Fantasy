# M11 P1 — CE328 topic-branch enumeration and the real path to CE402

Data root: work/all (full 23-file .wolf unpack; H files only here).
Saves: work/retail_backup/Save/SaveData01..08.sav.  Event: BasicData/CommonEvent.dat.
Test: crates/wolf-game/tests/m11_hscene_ce328_branches.rs (4 tests, green, warnings 0).
Built on: M11_HSCENE_BOOKSTORE.md (task-90), M11_HSCENE_REAL_GATE.md (task-80),
M9_HSCENE_LOOP.md (task-54).  No production edits, no commit.

## Verdict (short)

**No real-input path from any shipped retail save reaches pc986 op210[500402] =
CE402, hence CE402/CE410 (the H CG cluster) stay unreachable end-to-end.**
CE328's topic menu is real and drivable, but all four topic branches (and the
var304>=1 night entry) terminate long before the H CG call.  pc986 lives in the
悪戯へ (mischief) body that is only entered through the 通常就寝 (pc745) body,
which itself is only reachable via a date-driven 就寝 jump (pc122) plus a
var18 >= 50 gate — a calendar window no shipped save presents when CE328 runs.
CE402 is reproducible only by driving it directly on a synthetic save (as pinned
in m11_hscene_real_gate.rs / m11_hscene_bookstore.rs).

## CE328 entry routing (empirical, start_event(328) on a loaded save)

- var304 >= 1 (real output of the CE245 pc779 / CE290 pc403 intimacy menu):
  pc15 op111 [2000304,1] true -> pc17 op213 "Hしたい" -> pc90 body ->
  pc727/pc731 -> pc740 op210[500332] = CE332 -> pc741 op172 END.
  CE332 pc125 op121 [2000304,0] resets var304 = 0.  **Never reaches pc986.**
- var304 == 0, day handler leaves var6==5 (Fri) & var4<=3: pc102 var6==5 ->
  pc104 var4<=3 -> pc106 op213 "家族会議" -> body pc126 -> topic menu pc173.
  After the topic flow: pc303..668 -> pc749 -> pc755 -> ... -> pc843 ->
  pc1023 op103 "お風呂を0にすること" END.  **Never reaches pc986.**
- var304 == 0, other day state (all shipped saves): pc100..101 -> pc749 ->
  pc755 (var4 branch) -> pc791/792 wait loop (or pc759/769/779) -> pc843 ->
  pc1023.  Low-var4 saves (SD02/04/05/06) instead take pc972 -> pc974 ->
  pc977 -> pc987 -> pc994 -> pc1023.  **Never reaches pc986.**

Default-drive milestones on all 8 saves (var304 = 0 and 1), warnings 0:
CE328 max pc is 1023 (var304=0) or 741 (var304=1) in every case; pc863/957/966/
985/986 are never visited and CE402/CE410 never spawn.

## Topic menu branch table (pc173 op102, 4 options)

Reached only inside the 家族会議 body (needs var6==5 & var4<=3).  Menu pc173
op102 [20] options: 梦想的话题 / 无关紧要的话题 / 色情的话题 / 小鸟飞飞！.
Answering index 0/1/2/3 (observed parked pc 174) routes to:

| index | option | first body pcs | converges to |
|-------|--------|----------------|--------------|
| 0 | 梦想的话题 | pc177,178,179,180,182,183,187,192,193,196,197,198 | pc303/304/305/306 -> 330..333 -> 359 -> 387 -> 577..592 -> 646/660/661 -> 662 -> 665/666/668 -> 716/717/718 -> 749 -> 755 -> 843 -> 1023 |
| 1 | 无关紧要的话题 | pc207..212,221,222,225,226,227 | same tail as index 0 |
| 2 | 色情的话题 | pc236..242,251,252,255,256,257 | same tail as index 0 |
| 3 | 小鸟飞飞！ | pc262..266,285,286,291,292,293 | same tail as index 0 |

Every index: CG-select menu pc579 op102 [19] (3 options) and yes/no pc661 op102
[2] (是/算了) are reached, then the run ends at pc1023.  No index spawns CE402
or CE410; warnings 0.  (If the pc53 op102 [2] 是/算了 confirmation is answered
算了 = index 1, the whole branch aborts early — the 是 path is required.)

## Static gate graph (why pc986 is isolated)

- pc986 op210 [500402]: CE402's ONLY caller in the whole event graph is
  (CE328, pc986) — asserted by scanning every op210 in CommonEvent.dat.
- pc986 sits inside the 悪戯へ body: label defined at pc862 op212 悪戯へ -> 863.
  That body runs pc863..952 -> pc957.. -> pc964 var64=1 -> pc966 op210[500331]
  = CE331 -> pc973 op210[500222] = CE222 (save) -> pc985 op103 騎乗位/夢精判定
  -> pc986 op210[500402].
- The 悪戯へ body is entered only from the dispatch pc827..pc861
  (pc839/pc847/pc854/pc857 op213 悪戯へ), which lives inside the 通常就寝
  (pc745) body (label pc744 op212 就寝 -> 745).
- The 通常就寝 body is entered only via a 就寝 jump to label pc744:
  - pc122 op213 就寝  (live, but only on the var6==4/var4>20 day branch)
  - pc721 op213 就寝  (**dead**: pc719 op499 closes the 合宿(694) block and
    jumps straight to pc749, skipping pc720/pc721)
- The 745 body is additionally gated at pc746 op111 [17,2000018,50,1] =
  var18 >= 50 before pc748/pc749 run.
- pc122 is reached only from the var6==4 (Thursday) branch:
  pc102 op111 var6==5 (Fri) -> family meeting; else pc111 op111 var6==4 ->
  pc113 op111 var4>=6 -> pc115 op111 var4<=20; when var4>20 the else path
  pc121 -> pc122 就寝.  i.e. pc122 needs var6==4 AND var4>20, disjoint from the
  Fri/early-month window that opens the topic menu.

Variable writers / readers (raw scan of CommonEvent.dat; line = event pc):

- var304: writers CE245 pc779 =1, CE290 pc403 =1, CE332 pc125 =0.
  readers CE328 pc15 op111 (var304>=1 -> Hしたい), pc694 op111 (合宿 branch).
- var4 (day-of-month, from the clock): writers CE217 pc19=1/pc23=24,
  CE257 pc425=9, CE277 pc70=9, CE305 many=7/10/16, CE310 pc2881=9,
  CE317 pc123=9/pc622=8, CE326 pc1616=18, CE327 pc11=18, CE405 pc26=18/pc60=15,
  and CE328 itself pc117=20, pc763/773=22, pc783/795=23, pc901/911=22,
  pc921/933=23.  readers CE328 pc20/31/57/69/104/113/115/755/787/850/852/893/925.
- var6 (weekday, from the clock): writers CE217 pc24=1/pc27=0,
  CE305 pc2072/2277/2559/2836=1, CE310 pc2883=1, CE329 pc377=1/pc380=0.
  readers CE328 pc102 (var6==5), pc111 (var6==4).
- var18 (used by the 745 body sleep gate): writers include CE216 pc126=120,
  CE246 many +/- (affection deltas), CE247/CE248/CE277/CE278, CE278 pc557=0.
  reader CE328 pc746 (var18>=50).
- CE332 (深夜開始): called at CE328 pc711 and pc740 (op210 500332); pc125 resets
  var304=0.

Because the clock (CE217) and the nightly day handler recompute var4/var6 every
night from the calendar, simply seeding var4/var6 on a save does not survive to
the pc102/pc111 branch evaluation — the running event re-derives them.  That is
why every shipped save lands on the Friday/family or the bath-reset branch and
none reaches the Thursday/late-month 就寝(pc122) route.

## What the test file pins

crates/wolf-game/tests/m11_hscene_ce328_branches.rs:
1. topic_menu_four_branches_diverge_to_bath_reset_not_h_cg — drives SaveData01
   with var6=5/var4=3 for topic index 0..3; asserts pc174 topic menu (4 options),
   pc579 CG-select, pc661 yes/no, terminal pc1023, and NO pc986/CE402/CE410.
2. var304_entry_short_circuits_to_ce332_and_never_h_cg — var304 in {1,2};
   asserts CE332 seen, max pc 741, pc740 op210[500332], pc741 op172, no pc986.
3. no_shipped_save_reaches_the_ce402_call — all SaveData01..08 x var304 in {0,1};
   asserts pc986/CE402/CE410 never reached, warnings 0.
4. gate_graph_pins_the_only_route_to_the_ce402_call — static op210/op212/op111/
   op121/op213 assertions pinning the gate graph above.

All four green via:
  WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_hscene_ce328_branches

## Closest real path achieved

SaveData01 (var304=0): pc100/101 -> pc749 -> pc755 -> pc791/792 loop -> pc843 ->
pc1023.  This is one 就寝-jump and one var18>=50 gate short of the 悪戯 body
that contains pc986.  With var4 forced <= 3 the run reaches the real topic menu
(pc173) and the CG-select pc579, but still ends at pc1023 without pc986.
