# M10 W2 residual - CE246 -> CE346 H cut-in executor

**Result.** The H cut-in executor CE346 (Hカットイン実行処理昼) has a **legitimate caller and a
complete, terminating path**: CE246's own menu walk 性骚扰 -> 让你看我撸管 opens its 積極的
sub-branch, which issues `210` (call common event) -> CE346 with self args **[24,1,1]**; CE346
runs its pc108 cut-in branch, draws `cutin/cutin_161.png`, and is reaped cleanly with **0
warnings**. This closes the "reach CE346" residual left open by
`docs/research/M10_HSCENE_DEEP_SAVE.md` (ev33 -> CE333 is a CG leaf; the deep bodies were
un-entered).

Pinned by `crates/wolf-game/tests/m8_hscene_cutin.rs` (4 tests, green ~1.2 s).

## 1. Static: the CE246 -> CE346 call shape

`op210` words are `[target, param_status, arg0, arg1, ...]`; `number_count = param_status & 0xF`.
Children receive their args in self group 6, i.e. flat self index `600 + slot`
(`SELF_ARG_BASE = 6*100 = 600`, magic `1_600_000+i`; `exec.rs` `spawn_child`).

CE246 calls CE346 (`target = 500346`) at exactly **7** sites, each with `param_status=3`
(3 args):

| pc | self args (600,601,602) |
|----|--------------------------|
| 695  | 24, 1, 1 |
| 906  | 24, 1, 1 |
| 1477 | 24, 1, 1 |
| **1543** | **25, 1, 1** |
| 1598 | 24, 1, 1 |
| 1692 | 24, 1, 1 |
| 1799 | 24, 1, 1 |

All seven sit inside CE246's 積極的 sub-branch (label ~pc1457, dialogue
"就让我来欣赏哥哥撸管吧"), downstream of the menu walk described in section 3. The parent then
waits on the child handshake via `op212/op213` ("ウェイトA..F", pcs 696/907/1478/1544/1599/1693/
1800) polling `var2000197`, which the child clears at each branch tail.

## 2. Static: the CE346 self-arg gates

CE346 gates its body on the three self args it was handed:

| pc | gate | meaning |
|----|------|---------|
| 2   | `op111 [2,1600001,0,2,1600001,1,2]` | SV1600001 in {0,1} |
| 12  | `op111 [3,1600002,1,2,...,2,...,3,...]` | SV1600002 in {1,2,3} |
| 22  | `op111 [19,1600000,1,2,...,2,...,3,...]` | SV1600000 in {1,2,3} |
| 108 | `op111 [3,1600000,4,2,1600000,24,2,1600000,25,2]` | SV1600000 in {4,24,25} -> cut-in branch |

The real arg **24** is exactly a pc108 selector, so the default call takes the pc108
"H cut-in" branch. CE346 also calls CE217 (ウェイト/演出 helper) at pc81 and pc103
(`op210 500217`), plays `pic350` background at pc1, `op123 [1600030,17]` key polls at pc48/pc119,
and draws `cutin/cutin_162.png` (pc113) / `cutin/cutin_161.png` (pc140/187), writing
`var2000111=5`, `var2000018=1` on the climax tail (pc94/171/218).

## 3. Legitimate end-to-end entry (realism)

The state used to open CE246's 積極的 branch is a **normal high-affection state**, not an
impossible one: it is built in memory from the real `SaveData07.sav` (already day-19:30 bedroom
with the H UI reachable) by raising ordinary affection / relationship counters. No retail save is
mutated; the only persistence is gitignored `work/out/m10_cutin_synth.sav`.

Constructed vars (`WIN_VARS`): 18=100, 20=100, 21=100, 22=300, 23=0, 24=100, 102=100, 103=1,
16=1, 70=0, 15=1, 157=13, 104=0, 105=0, 4=21, 26=100, 27=100, 100=100.

Menu walk (labels drive `game.reply(ChoiceReply::Index)`):

1. Top menu (`op102 [6]`, pc1) `["摸摸头","揉肩","拥抱","亲吻","挠痒痒","性骚扰"]` -> **性骚扰** (idx 5).
2. セクハラ sub-menu (`op102 [5]`, pc572) `["让我看内衣","让我看裸体","想看你自慰","让你看我撸管","小鸟飞飞"]` -> **让你看我撸管** (idx 3).
3. Any later prompt -> first option (idx 0).

Result: CE246 max pc **1940**, CE346 spawned and reaches max pc **180**, `cutin/cutin_161.png`
drawn, CE346 child **completed/reaped**, **0 warnings**, deterministic across two runs.

Menu wiring (from CE246 bytecode): the 性骚扰 label is pc571 -> pc572; the 積極的 branch is reached
only with the affection gates in section 4 open. Alternative 2nd-menu routes reach CE346 too
(e.g. 想看你自慰 via the オナニー/pc683 branch when var23=10), but the 让你看我撸管 route is the
shortest and was pinned.

## 4. Gate necessity (parent chain)

Zeroing one var at a time from the winning preset (`run_ce246(Some(var), 6000)`):

| zeroed var | effect | gate it breaks |
|-----------|--------|-----------------|
| 20  | CE346 never spawned (CE246 stuck ~pc561 / menu loop) | pc557 `var2000020 > 60` gates 性骚扰 |
| 21  | CE346 never spawned | pc1439 `var2000021 >= 100` |
| 22  | CE346 never spawned | pc1441 `var2000022` tiers >250/>150/>50 |
| 102 | CE346 never spawned | pc1445 `var2000022>=...` / `var2000102 >= 40` |
| 16  | CE346 never spawned (CE246 ~pc1431) | pc1425/pc644 `var2000016 >= 1` |
| 23 (control) | CE346 still reached | not required on this route |

`pc1443 var2000023>=6` and `pc1447 var2000103>=1` exist but are **not** required on the
性骚扰->让你看我撸管 route (they gate a different sub-branch).

## 5. Executor in isolation

Started directly with the real args (`self_vars 600=24,601=1,602=1`), CE346 advances to
**max pc >= 180**, draws a `cutin/*.png`, and its instance is **reaped** (completed) in ~420
frames, 0 warnings - deterministic. The `[25,1,1]` variant takes the pc108 alternate selector and
also completes. (A wrong args triple such as `[3,24,1]` stays shallow.)

## 6. Relation to prior work

- `docs/research/M10_HSCENE_DEEP_SAVE.md`: ev33 -> CE333 renders the H CG cluster but CE333 is a
  **leaf** (its only `op210` targets are empty); it never instantiates CE246/CE328/CE402. So the
  CG cluster and the CE346 cut-in executor are reached by **different** callers: CE333 for the CG,
  CE246's 積極的 branch for CE346.
- CE246 stateless (no affection state) stalls at its pc117 `var151>=1` early gate and never
  reaches the pc1943/1457 region; the constructed high-affection state is what lets it descend to
  the CE346 call sites.

## 7. Commands / files

```
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_cutin   # 4 passed
# regressions (all green):
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_cg_deep    # 6 passed
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_cg_run     # 4 passed
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_cg_unlock  # 5 passed
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_loop       # 3 passed
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_hscene_exit       # 3 passed
```

Files: `crates/wolf-game/tests/m8_hscene_cutin.rs` (new), this doc. Only persistence written:
`work/out/m10_cutin_synth.sav` (gitignored). No production edits, no commit.
