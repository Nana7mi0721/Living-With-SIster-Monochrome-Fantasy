# M9 W2 — the full H loop: night wrapper -> terminal stage 13 -> end button -> sleep -> next morning

Companion to crates/wolf-game/tests/m8_hscene_loop.rs (3 tests, green in ~4-5 s, deterministic
across repeated runs). Builds on task-46 (crates/wolf-game/tests/m7_hscene_exit.rs,
docs/research/M7_HSCENE_EXIT.md), which proved the H scene has a real exit. This file closes the
loop end-to-end and answers the remaining question: after the exit, where does control go?

## Result in one line

The H scene is a complete, playable **loop**, not an isolated stage. Starting from the real retail
save that resumes SampleMapA's late-night wrapper, a player drives the stage UI to the terminal
stage 13 with real pointer input, presses the on-screen end button, clears the message-window gate,
answers CE345's confirmation, and the teardown then calls CE329 就寝処理 — the ordinary sleep
routine — which advances the day counter (day 90 -> 91), rewinds the clock to the morning (settling
at 09:30) and returns to the normal morning bedroom / day-command scene with **0 warnings**. The H
overlay (hscene/ + cutin/ layers) is gone. Day/clock are **not preserved**: ending the midnight H
scene IS going to sleep, so the loop rolls over to the next morning.

## Setup (real save, one documented normalisation)

- Data root: work/all (H files only live in the full unpack). Save:
  work/retail_backup/Save/SaveData08.sav (the real retail save captured inside the midnight H wrapper).
- Entry: Game::new(data_root) + load_save(save) + start_engine_events() + start_pages(&map, Some(0))
  (skip the opening autorun event 0), exactly like m7_hscene_playable.rs / m7_hscene_exit.rs.
- Only injected value: var364 = 0 — the documented late-night dispatcher gate the save was captured
  with closed (CE335 深夜差分描画 pc3729 = 1 / pc3762 = 0). Everything else (var61/var62/var137,
  the clock, the day, the morning scene) is produced by the real event scripts and real pointer input.
- Input: click_at(x,y) = mouse set, frame, mouse_buttons=1, frame, mouse_buttons=0, 15 frames (the
  exact opcode-124 shape: field 8 hover + fresh left press).

## Stage-UI navigation (real pointer)

nav_to_terminal_stage clicks whichever on-screen h_button / hbutton picture advances var62 and stops
at var62 == 13. The deterministic SaveData08 forward path is **1 -> 2 -> 6 -> 1 -> 4 -> 13**. At stage
13 var62 = 13, var64 = 1, sys var 12 = 0, Picture/endbutton.png (picture 294) is drawn, and the H pose
layer hscene/h_s10_pose is on screen.

## Exit chain (SampleMapA + CommonEvent.dat)

1. event 11 dispatcher: pressing picture 294 writes var61 = 49 (op121 [2000061,49,0,0]).
2. map event 25 深夜終了処理 page 0 (conds var140==0, var60==0, var364==0, var123==0): pc9 op111
   [1,9000012,0,2] (system var 12 == 0, the CE63 message-window flag), pc11 var61==49, pc15 var60<2,
   then pc21 var62=1, pc23 op210 CE345. So the end button only dispatches while no text window is open.
3. CE345 深夜終了コマンド raises two confirmations on the yes path: a first **["是的","再想想"]** and
   then its own **["是","算了"]** decision (pc39 op102 [2]). Either answering option 0 proceeds.
4. CE345 teardown: pc339 var62=0, pc340 var63=0, pc341 var64=0, pc342 var106=0, pc343 var241=0,
   pc344 blackscreen, pc346 picture-290 fade, pc347 op210 CE336, pc348 op210 CE222[2,13,0] (save),
   pc353 op210 CE217 (clock), and **pc361 / pc364 op210 CE329**.
5. CE329 就寝処理 (the ordinary sleep routine, the same one the player's "今天还是睡吧" path uses):
   pc406 op121 [2000160,1,0,256] -> day counter var160 += 1; pc385 op121 [2000004,7,0,0] -> clock hour
   var4 = 7; pc386 op121 [2000005,45,0,0] -> clock minute var5 = 45. The following morning scene plays
   the clock forward to 09:30.

Static proof pinned by test 3:
- call_pcs(CE345, 500_329) contains 361 and 364; call_pcs(CE345, 500_222) contains 348;
  call_pcs(CE345, 500_217) contains 353.
- write_pcs(CE329, 2_000_160) == vec![406]; ce_ints(CE329, 385) == [2_000_004,7,0,0];
  ce_ints(CE329, 386) == [2_000_005,45,0,0].
- ce_ints(CE345, 339) == [2_000_062,0,0,0] (var62=0); ce_ints(CE345, 341) == [2_000_064,0,0,0] (var64=0).

## Observed post-exit state (deterministic, three+ runs)

Before the end press: day 90, clock (hour,minute) = (0,42), var62 = 13, var64 = 1.
After the loop: day **91**, clock settles **(9,30)**, and
- var62 = 0, var63 = 0, var64 = 0, var106 = 0, var241 = 0 (all CE345 resets), var137 = 2;
- warnings = 0 (exit, sleep and morning are all warning-free);
- the H overlay is gone: no picture file under hscene/ or cutin/ remains;
- the normal morning scene is drawn: bg_01_home background, stand_imouto portrait, sub_button
  day-command buttons present, picture count > 40;
- the map runtime is alive: the always-on メニュー処理 rollover event (event 4) stays live; the event
  set observed at morning arrival was {1, 2, 4, 13, 25, 63, 317, 329, 345}.

Residuals (documented, not a leak): two h-word overlays persist on the picture list —
hword/word_effect26.png (picture 9000) and hword/word_effect115.png (picture 9300) — but both have
opacity 0 (invisible). The test asserts no *visible* hword/ overlay (not hidden and opacity > 0)
survives the exit, so the residual is inert.

Contrast (test 2): without pressing the end button, the wrapper holds stage 13 for 700 idle frames and
the day stays 90 / var64 stays 1. The day rollover is caused by the exit specifically, not by idle time.

## Reachable wrapper vs gated CGI callees

The task names the SampleMapA H wrapper cluster as ev33 -> CE333 -> CE246/CE372/CE402. Those CGI /
common-event callees are gated on this save and are NOT synthesized:
- map event 33 activates only on global var460 == 1 (its single enabled page condition; variable
  2_000_460, value 1), and SaveData08 carries var460 = 0. var460 is set to 1 only by CE344/CE345/CE347
  (the ejaculation chain), which needs v111 >= 100 (retail 0 here).
- CE246's deep CG needs the intimacy self-var chain self1600013 / var157; CE402 needs var104/var105 != 0
  and var4 >= 21.

The wrapper a player actually sees and drives here is the var62 stage UI (events 9/10/11/14/25 + CE345),
which is what the loop tests exercise. The gated CGI branches are pinned in test 3 and left documented
rather than faked (see also docs/research/M7_HSCENE_PLAYABLE.md, M8_HSCENE_FORWARD.md).

## What the three tests pin

1. full_loop_enters_stage13_exits_and_rolls_to_next_morning — real pointer to stage 13; assert the end
   button + H pose layer are present; press end; assert CE345 (345), CE222 save (222) and CE329 sleep
   (329) ran; assert var62/63/64/106/241 all 0; let the sleep routine run; assert day -> day+1, clock
   hour >= 7, warnings 0; assert the H overlay is gone and the morning scene (bg_01_home + stand_imouto +
   sub_button) is drawn; assert event 4 stays live; assert no visible hword overlay survives.
2. no_end_press_holds_stage_13_without_rollover — reach stage 13, idle 700 frames without the end
   button; assert var62 stays 13, var64 stays 1, day stays put; this proves the rollover is the exit.
3. named_cg_chain_is_gated_and_rollover_is_wired_through_ce329 — save var460 == 0; ev33 page 0's single
   condition is var460 == 1; CE345 -> CE329 (pc361/364), CE222 (pc348), CE217 (pc353); CE329 pc406
   var160 += 1 and pc385/pc386 morning clock; CE345 pc339/pc341 resets.

## Verdict

- The H scene is a **complete playable loop**: enter (real save) -> stage UI to terminal stage 13 (real
  pointer) -> end button (sys var 12 gate + CE345 confirmations) -> CE345 teardown -> CE329 sleep ->
  next-morning day scene, warning-free.
- **Day/clock roll over**, not preserved: ending the midnight H scene is the sleep action, so the loop
  continues at the next day morning (09:30), on the normal day-command scene.
- **Residuals:** two h-word overlays remain but are fully transparent (opacity 0) and inert.
- **No missing data gate.** The named ev33 -> CE333 -> CE246/CE372/CE402 CGI cluster is separately gated
  (var460 / self1600013 / var104-105 / var4) and is not the reachable wrapper; it is documented, not
  synthesized. The only injected engine value across the whole loop is the documented var364 = 0.
- No screenshot is produced (not required by task-54); a full picture layer (>40 pictures) is asserted
  instead.

Run: WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_loop
