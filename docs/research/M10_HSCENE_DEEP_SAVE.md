# M10 W2 - a constructed deep H save, and how far it advances (task-70)

**One-line result.** A TEST-ONLY constructed save (from SaveData07, mutated in memory, written
to the gitignored `work/out/m10_deep_synth.sav`) opens the full deep H CG gate
`{var157==13 & var104==0 & var105==0 & var4>=21 & var102>=80}` simultaneously; driving
CE402 from it runs the deepest CG body - max pc 312/512, the 騎乗位系カットイン child CE410
entered, 14 `h_s08` pose/effect files + `h_bg_001`/`h_bg_011`/`h_frame` drawn, 0 warnings,
deterministic. The gate set is **necessary** (plain SaveData07 aborts CE402 at pc0) and, for the
CE402 body itself, **sufficient**. The retail-save blocker from M9 was only that no shipped save
carries all five values at once.

No retail `.sav` is mutated. Deliverables: `crates/wolf-game/tests/m8_hscene_cg_deep.rs`
(6 tests) and this note.

## 1. The gate and where it is read (evidence)

All reads are op111 comparisons in `BasicData/CommonEvent.dat` (var = 2_000_000 + index):

| Common event | pc | condition (op value/operator) | role |
|---|---|---|---|
| CE402 `素股騎乗位` | pc0 | var16 >= 1 (op1) | body gate |
| CE402 | pc2 | var22 >= 180 (op1) | body gate |
| CE402 | pc4 | var4 >= 21 (op1) | body gate |
| CE402 | pc6 | var326 >= 2 (op1) | body gate |
| CE402 | pc8 | var102 >= 80 (op1) | body gate |
| CE328 `┣就寝前パジャマ` | pc4 | var105 == 0 (op2) | entry gate |
| CE328 | pc6 | var104 == 0 (op2) | entry gate |
| CE246 `┣スキンシップ` | pc117 | var151 >= 1 (op1) | early-out gate |
| CE246 | pc1943 | var157 == 13 (op2) | deep-body gate |

CE402 calls its cutin child **CE410 (騎乗位系カットイン)** at pcs
165/173/177/195/198/288/308/311/360/447/450/462/474 (13 sites). CE328 calls **CE402 at pc986**
(the only entry), immediately after the `pc985 op103` label `騎乗位/夢精判定`. CE246 reaches
CE333 only at **pc1983** and CE346 at pcs 695/906/1477/1543/1598/1692/1799, all inside the
pc1943 `var157==13` block. **ev33's only callee is CE333** (map `SampleMapA`), and CE333 is a
leaf (0 op210, 401 op150 draws) - so the map `var460/ev33` path renders the CG cluster but never
enters CE246/CE328/CE402.

## 2. Constructed save

Built in-test from `work/retail_backup/Save/SaveData07.sav` (the only retail save with
var157==13), by setting these normal vars and writing to `work/out/m10_deep_synth.sav`
(`Game::save_to` round-trips normal vars 0..800):

```
var157=13  var104=0  var105=0  var4=21  var102=80
var16=1    var22=180 var326=2   var364=0
```

Reload sanity (pinned by test 1): all nine values read back exactly; plain SaveData07 still has
var4=19 (<21) and var102=59 (<80), i.e. the synth is a genuine delta on top of the retail save.

## 3. Measured advance (live VM, auto_advance + auto_choice, 0 warnings everywhere)

Deep common events are driven on the **loaded save alone** (no engine/map pages started):
background coroutines overwrite the gate vars within the first frames, which is exactly why the
M9 runs, which started the engine, could not keep the gate open. With the gate held:

| run | start | frames | result |
|---|---|---|---|
| CE402 on synth | `start_event(402)` | 1500 | **DEEP**: max pc 312/512; CE410 entered (max pc >=1); 14 `h_s08` files + `h_bg_001`/`h_frame`; warnings 0; identical across 2 repeats |
| CE402 on SaveData07 | `start_event(402)` | 400 | **aborts** at pc0 (max pc < 20); no CE410; 0 `h_s08`; warnings 0 -> gates necessary |
| ev33 cluster on synth | ev11 hover pic444 -> var460=1 | 400 | ev33 runs, var460 peak 1, CE333 draws >=12 H CG files; **CE246/328/402 never instantiated**; no `h_s08` |
| CE246 on synth | `start_event(246)` | 1500 | stalls at **pc116** (pc117 var151>=1 fails, var151=0); pc1943 deep block unreached; CE333/CE346 not called |
| CE328 on synth | `start_event(328)` | 3000 | runs its OWN body (max pc >1000 of 1025) but **skips pc986**: CE402 never instantiated, 0 `h_s08` |

**How far the deep gate actually gets you:** to CE402's own body and its CE410 cutin (pc312).
It does **not** reach CE346 (`Hカットイン実行処理昼`) - that executor is a separate menu-entry
common event called from CE246, not from CE402, and a stateless start of CE346 self-var-blocks
(needs self1600000..3 in range). ev33 -> CE333 is a rendering leaf. CE246's deep block needs
var151>=1 set *before* start; a bare constructed save stalls at pc116.

## 4. Tests (`crates/wolf-game/tests/m8_hscene_cg_deep.rs`)

1. `synthetic_save_opens_the_full_deep_gate_and_round_trips` - build + reload exact; SaveData07
   fails var4/var102; static gate/call-site graph (CE402 gates + CE410 pcs, CE328 entry + pc986,
   ev33 only callee 500333).
2. `ce402_runs_its_deep_cutin_body_on_the_synthetic_save` - deep run, CE410, 14 h_s08, 0 warnings,
   deterministic over 2 runs.
3. `ce402_gates_are_necessary_plain_savedata07_aborts` - SaveData07 aborts, no CE410/h_s08.
4. `ev33_and_ce333_never_reach_the_deep_branch_even_when_gates_are_open`.
5. `ce246_stateless_start_cannot_reach_its_deep_block` - stall pc116.
6. `ce328_runs_its_body_but_skips_the_ce402_sub_branch` - max pc >1000, CE402 absent.

Run:

```
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_cg_deep
```

Result: **6 passed, 0 failed** (~2.1 s). Regressions re-run green:
`m8_hscene_cg_run` 4/4, `m8_hscene_cg_unlock` 5/5, `m8_hscene_loop` 3/3, `m7_hscene_exit` 3/3.

## 5. Constraints / caveats

- Only `work/out/m10_deep_synth.sav` is produced on disk (gitignored); no retail save touched.
- The isolated (no engine) start is deliberate: a fully-live frame clobbers the gate vars, which
  is a data/flow property, not a test artifact - documented here so it is not rediscovered.
- CE402 body is the ceiling of the deep gate; CE346 and CE246's deep dialogue need their own
  preconditions (var151 / self-vars) and stay out of scope, as previously recorded in M9.
