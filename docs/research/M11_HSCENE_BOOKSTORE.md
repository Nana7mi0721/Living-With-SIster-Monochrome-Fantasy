# M11 W2 - H CG bookstore / skill line and the CE328 deep branch (task-90)

One-line result: **The bookstore -> skill line is real and reachable from a shipped save
(var276, var277, var326 and var304 are ALL obtainable from SaveData01 through real event
inputs), but CE402 / CE410 are still unreachable end-to-end because CE328's real H/合宿
branch early-returns at pc741 op172 BEFORE the CE402 call, which lives only at CE328
pc986.  The remaining blocker is CE328 control flow, not the bookstore / var326 gate.**
This corrects the earlier task-86 conclusion that var277 / var326 are unreachable.

Data: work/all (23 .wolf maps incl SampleMapA, BasicData/CommonEvent.dat);
saves work/retail_backup/Save/SaveData01..08.sav.
Test: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_hscene_bookstore (5 passed).
Every test asserts stats.warnings == 0.

---------------------------------------------------------------------------
## 1. Reachability matrix (all from SaveData01, real inputs only)
---------------------------------------------------------------------------

  variable / gate        value needed      writer (event pc)                 reachable? evidence
  ---------------------  ----------------  --------------------------------  ---------- ---------------------------------------
  var276 (色色的书)       >= 1              CE314 pc1188 op121[2000276,1]     YES        buy via bookstore menu (test 1)
  var277 (小薄本)         >= 1              CE314 pc1233 op121[2000277,1]     YES        buy via bookstore menu (test 1)  <-- was thought unreachable
  var326 (learned 騎乗位)  == 2             CE301 pc6320 / pc6365 op121       YES        CE301 after owning books (test 2)
  var304 (intimate route) == 1             CE290 pc403 op121[2000304,1]      YES        night menu option 5 (test 3)
  CE328 entry             var105==0,var104==0,var304>=1  CE328 pc4/pc6/pc15   YES        after the above vars, CE328 entered (test 3)
  CE402 body              var326>=2,...      CE402 pc0..pc8                    state met  but CE402 is never CALLED on the real path
  CE402 called            (only ce328 pc986) op210[500402]                   NO         CE328 early-returns at pc741 first (test 3/4)
  CE410 (騎乗位カットイン)  (only from CE402)  op210[500410] x10 in CE402        NO (real)  only synthetic direct CE402 (test 5)

So all four "gate variables" are reachable; the wall that remains is purely that CE328's
reachable branch does not fall through to its pc986 CE402 call.

---------------------------------------------------------------------------
## 2. The bookstore menu is a real, drivable event (CE314)
---------------------------------------------------------------------------

CE314 is entered from CE308 pc115 and CE310 pc670 (op210[500314]); those are the day
"go to town with sister" handlers.  CE308 itself is drivable: its top menu
(pc0 op102[8]) has a "go to town" branch at pc87 op401[3]; the money gate is
pc88 op111[1,2000007,500,3] (needs var7 >= 500).  Choosing that option on SaveData01
(var7 = 7586) reaches pc115 and instantiates CE314 (observed CE314 max pc 1533).

Inside CE314 the real menus are:
  * pc80  op102[23] town menu: 食品店街 / 日用百货店 / 服装店 / 书店 / ... / 差不多该回去了.
  * pc1134 op102[8] bookstore menu: 冒险之书 / 色色的书 / 小薄本 / ...
  * buying 色色的书: pc1186..pc1188 op121[2000276,1]  (300G, guarded by var7 >= 300,
    var1600011 != 1, var276 < 10)
  * buying 小薄本:   pc1227..pc1233 op121[2000277,1]  (500G, guarded by var7 >= 500,
    var1600011 != 1, var277 < 10)

Test 1 starts CE314 on SaveData01 (var276==var277==0) and answers the real menus
(书店 -> 色色的书 -> 是, 小薄本 -> 是).  Result: var276>=1, var277>=1, max pc >= 1233,
warnings 0.  This is the concrete purchase path that task-86 said did not exist.

---------------------------------------------------------------------------
## 3. The real skill event CE301 sets var326 = 2 once the books are owned
---------------------------------------------------------------------------

var326 has exactly two writers in the whole corpus: CE301 pc6320 and pc6365
(op121[2000326,2]).  CE301's skill block is gated (pc6281) on var276 >= 1 && var277 >= 1
(and later var102 >= 60 && var103 >= 1); on success it plays "\E妹妹习得了技能 骑乘位！"
and sets var326 = 2.

Test 2 buys the books (test 1's path), persists the save (Game::save_to -> work/out/m11_hscene_books_t{2,3}.sav,
a scratch copy - no retail save is touched), reloads it, and runs
the real CE301: var326 becomes 2, CE301 max pc >= 6365, warnings 0.  So var326 IS
obtainable from a shipped save through real gameplay events.

(Note: buying must be persisted and the skill event run on the reloaded save.  If CE308's
"go to town" trip and CE301 are run back-to-back in one live session, the trip bumps var4
and leaves the town-trip coroutine running, and CE301's skill block is then skipped - the
real pattern is "buy in town, later day learn the skill", which the persist/reload models.)

---------------------------------------------------------------------------
## 4. CE328 returns early at pc741 before CE402
---------------------------------------------------------------------------

The real night intimacy menu (CE290 option index 5) sets var304 = 1 at pc403 and calls
CE328 at pc406.  Test 3 runs the full real chain (bookstore -> persist -> reload -> CE301
=> var326=2 -> CE290 idx5 => var304=1 -> CE328) and observes:

  * CE328 is entered (seen event 328).
  * var304 reaches 1.
  * CE328 max pc == 741 (deterministic for this path).
  * CE402 and CE410 are NEVER instantiated.
  * warnings 0.

Why pc741: CE328 has a 合宿処理 block (label pc693) reached from the topic menu.  It tests
var304 twice: pc694 op111 and pc724 op111.  When var304 >= 1 the flow takes the 合宿/H
branch which ends in CE332 (pc711 or pc740 op210[500332]) followed by op172 END
(pc712 or pc741).  The CE402 call is at pc985/pc986 (op210[500402]) deep in the 就寝
(sleep) branch, which the var304 >= 1 branch never reaches.  CE402's ONLY caller in the
entire corpus is CE328 pc986.

Extra empirical data (all warnings 0, CE402 never seen):
  * SaveData01, var304=1, var326=2 -> CE328 max pc 741, no CE402.
  * SaveData07, var304=1, var326=2 -> CE328 max pc 712 (the other 合宿 sub-branch), no CE402.
  * SaveData01/07, var304=0, var326=2 (even var102=80) -> CE328 runs a longer body
    (max pc 1023) but STILL never instantiates CE402 on the reachable sub-branch.

So on every drive we can construct the CE402 call site is not executed; the exact pc of the
early return is 712 or 741 depending on entry, both before pc986.

---------------------------------------------------------------------------
## 5. The synthetic var326=2 path (state is the only remaining ingredient)
---------------------------------------------------------------------------

Driving CE402 DIRECTLY (start_event(402)) on a bare-loaded SaveData01 whose DEEP_GATE is
seeded - {var157==13, var104==0, var105==0, var4>=21, var102>=80, var16>=1, var22>=180,
var326>=2, var364==0} - runs CE402 deep (max pc >= 300) and enters CE410
(騎乗位系カットイン, max pc 192), warnings 0.  This proves the CG events themselves are
fine; the only reason they do not fire from the real H route is that CE328 never calls
CE402 (section 4).  (Gate vars are clobbered by engine/map background coroutines within
the first frames, so the deep events must be driven on the loaded save alone - no
start_engine_events / start_pages.)

---------------------------------------------------------------------------
## 6. What tests/m11_hscene_bookstore.rs pins (5 tests, all green)
---------------------------------------------------------------------------

1. bookstore_menu_buys_both_skill_books - real CE314 menus buy var276>=1 and var277>=1
   (reaches pc1233), warnings 0.  Corrects the "var277 unreachable" claim.
2. persisted_book_purchase_lets_ce301_learn_the_skill - purchase -> save_to -> reload ->
   CE301 sets var326=2 (max pc >= 6365), warnings 0.
3. real_chain_reaches_ce328_but_early_returns_at_pc741 - full real chain sets var326=2 and
   var304=1, enters CE328, CE328 max pc == 741, CE402/CE410 never seen, warnings 0.
4. gate_graph_pins_bookstore_and_ce328_deep_branch - static: var326 writers are exactly
   {(301,6320),(301,6365)}; var277=1 only CE314 pc1233/pc1530; bookstore callers
   {(308,115),(310,670)}; CE402 unique caller {(328,986)}; CE314 purchase pcs 1188/1233;
   CE308 pc115 calls bookstore; CE328 pc740 op210[500332], pc741 op172, pc986 op210[500402],
   pc694/pc724 op111.
5. synthetic_var326_two_path_drives_ce402_to_ce410 - DEEP_GATE direct CE402 -> CE410,
   warnings 0.

---------------------------------------------------------------------------
## 7. Verdict
---------------------------------------------------------------------------

1. var276, var277, var326 and var304 are all reachable from SaveData01 via real event
   inputs (bookstore purchase -> skill event -> night menu).  The "bookstore/var326 gate"
   is therefore NOT the blocker to the H CG cluster.
2. The real blocker to CE402 / CE410 is CE328 control flow: its reachable H/合宿 branch
   early-returns at op172 (pc712 or pc741) before the only CE402 call at pc986.  To reach
   CE402 from CE328 a different (sleep / 就寝) sub-branch would have to be taken, and in
   every drive we built it is not.
3. Consequently CE402 / CE410 remain reproducible only by driving CE402 directly on a
   synthetic DEEP_GATE save.  This is consistent with task-80 / m11_hscene_real_gate, and
   narrows the earlier task-86 "var277/var326 unreachable" claim to "reachable, but CE328
   still does not fall through to CE402".
