# M10 — Independent review of the opcode-111 re-baselined tests

Task: task-78. Reviewer: m6-night2. Date: 2025-10-31.
Repo: `E:/Study/github program/imoto life`, main-tree HEAD `ce680b3`.
Input reviewed: `work/out/m10_landing_full.patch` (842 lines / 54,533 bytes / 15 files, produced by task-77 in the isolated worktree `work/out/m10land` @ `22148d8`) plus `work/out/m10_landing_full_values.txt` and `docs/research/M10_OPCODE_111_LANDING.md` §10.1.

This is a docs-only review: no source or test file was edited. Every PASS/FAIL below is an independent judgement of the *expected value* against (a) the corrected VM semantics encoded by the two VM hunks and (b) the shipped event bytecode (CE/pc/var) in `work/_battle_ce.json`.

---

## 0. The two VM fixes are the ground truth (both correct)

The re-baseline is only legitimate if the VM change is correct. I decoded both hunks against `docs/OPCODE_SPEC.md`:

1. `crates/wolf-vm/src/exec.rs:1464-1470` — `compound()`: `0x600/0x700/0x800` now return a unary transform of **`current`** (target's current value), not of the binary `value` result:
   `0x600 => current, 0x700 => current, 0x800 => current.abs()`.
   Correct per §3.1 (`cur.floor()/cur.ceil()/cur.abs()`). The old code returned `value`, which let CE172 pc27/pc28 force the ally dead-count accumulator to 1 regardless of the CE176 status read → CE172 pc32 `dead == total` abort.
2. `crates/wolf-vm/src/exec.rs:2226-2231` — opcode 111 value operand `arg(cmd, 2+3*i)` → `eval_num(arg(cmd, 2+3*i), inst, vars)`.
   Correct: the value operand is a full magic operand; 356/22,695 (1.57%) of condition triples are variable/self references that must be dereferenced before comparison (CE226, CE109, CE64, CE26 …). Plain literals are returned unchanged by `eval_num`, so the 98.4% literal sites are behaviour-preserving.

`crates/wolf-vm/tests/opcode_111_semantics.rs` was rewritten to pin these. Reported `wolf-vm` = 130 tests green in the worktree.

**Independent recomputation of the two mechanisms that drive most numeric changes** (from bytecode):

- **CE226 `■行動力変動` (46 cmds)** — the energy clamp: `pc0 1600011=2000014` (cur=var14); `pc1 1600010=2000013` (max=var13); `pc2 1600012=1600011+1600000` (proposed); `pc15 c111[18, 1600012,0,3, 1600012,1600010,1]` = *if proposed<=0 →* `var14=0` (`pc17`); *else if proposed>=var13 →* `var14=var13` (`pc20`). With the **literal** bug the second operand `1600010` was compared as the raw number 1,600,010, so `proposed>=1600010` was **never** true and the clamp never fired — `var14` then ran free through the pc23-39 ±1 animation. With `eval_num` it dereferences `var13`, so `var14` is clamped to the real max-energy budget. `var13` is a decaying budget (CE216 pc10 = 120 at init; 14 further `-=1` sites in CE252/301/308/310/312/313/326): 120 → 60 by day-1 18:30 → 35 by a fresh day-2 evening. This single mechanism explains #2 (60) and #8 (35), and it is a *correct clamp*, not a downstream mask.
- **CE239 case2 cleanup** — `pc877 c111[1,2000343,1,2]`; when `2000343 != 1` it falls to `pc1043 2000350=0 / pc1044 2000352=0 / pc1045 2000343=0 / pc1046 2000340=0` — the *undecided-battle abort cleanup*. This is the legitimate source of #5's new end state.

---

## 1. Verdict table (12 battle tests + 2 VM files)

| # | File:line | Change | Verdict |
|---|-----------|--------|---------|
| VM | exec.rs:1464 / :2226 | compound `current`; op111 `eval_num` | **PASS** |
| VM | opcode_111_semantics.rs | rewrite | **PASS** |
| 1 | m6_mainline.rs:427 | `>10_000` → `>9_000` (obs 9792) | **PASS** (stale proxy) |
| 2 | m6_training_menu.rs:162 | `==100` → `==60` | **PASS** |
| 3 | m7_battle_menu_text.rs | control seeds party+1 enemy (fixture) | **PASS** |
| 4 | m7_battle_seed.rs | same control fixture | **PASS** |
| 5 | m7_battle_target_scheduler.rs:363 | `var350==2` → `==0` | **PASS** (minor comment nit, §4) |
| 6 | m7_ce244_direct.rs:128 | `[(11,1),(1600010,-1)]` → `[(11,-1),(1600010,1)]` | **PASS** (pure stale) |
| 7 | m7_day4_playable.rs:428 | +`set_battle_round_loop(true)` | **PASS** (pre_power<200 held) |
| 8 | m7_hmode_state.rs:189-191 | var14 115→35, var16 4→3 | **PASS** (clamp explained) |
| 9 | m8_battle_party.rs:148-190 | +`settles` flag (1-4 settle, 5-8 don't) | **PASS** (behaviour re-scoped) |
| 10 | m8_hscene_forward.rs:890 | end `var218==2` → peak `var218_max==2` | **PASS** (non-masking) |
| 11 | m9_battle_round_gap.rs:458+ | single-round → structural multi-round | **PASS** (structure, RNG-tolerant) |
| 12 | m9_full_route.rs:495-634 | `pre_power<200` → `>=200` (+round_loop, 40k) | **PASS** (loss→day5 green) |

**Overall: PASS.** 12/12 battle re-baselines are semantically justified; no test is a mere "make it match observed" mask. Two non-blocking documentation nits (§4). Witnessed worktree verification: `cargo test -q --workspace --no-fail-fast` = 99 binaries / 701 passed / 0 failed; `wolf-cli verify --root work/all` = PASS 70 / 0.

---

## 2. Cluster A — stale assertions on the *new* semantics (#1, #6)

**#1 `m6_mainline.rs:427`** — `assert!(game.stats.instructions > 10_000)` → `> 9_000`, observed 9,792. The sleep script's instruction total legitimately drops because a few now-taken/now-untaken branches in the magic-111 conditions change the path length. The load-bearing assertions in the same test (clock reaches day 4 08:30, screen non-empty) still gate "the script really ran", so lowering the proxy threshold does **not** hide a short-circuit. The value 9,792 is still > 3× a trivial path. **PASS.**

**#6 `m7_ce244_direct.rs:128`** — cases `[(11,1),(1600010,-1)]` → `[(11,-1),(1600010,1)]`. This is the cleanest case. CE244 `pc36 c111[1,2000349,1600010,2]` = *if `2000349 == self1600010`*.
- probe `2000349 = 11`: `self1600010` was set `pc3 = 11`, so `11 == 11` is **true** → display branch opens → `pc44 c111[17,2000192,2000193,0]` (`> `) reads the two gauges (equal here) → else → `pc49 敗北` → defeat arm `pc370 … 2000352 = -1 / 2000343 = 1`. Expected **-1**. Old test asserted **+1** because the literal bug compared `2000349` against the raw 1,600,010 and never entered the branch → fall-through win.
- probe `2000349 = 1_600_010`: `self1600010` is still the *value* 11, so `1600010 == 11` is **false** → fall-through → win `2000352 = 1`. Expected **+1**.
Both flips are exactly what the corrected operand semantics demand. **PASS** (pure stale).

---

## 3. Cluster B — energy clamp (#2, #8); Cluster C — battle dynamics (#5, #7, #9, #11, #12)

### #2 `m6_training_menu.rs:162` — `before_energy 100 → 60`
Day-1 18:30 bedroom start. CE226 pc15 clamps `var14` to `var13`; at that point `var13 = 60`. Pre-fix the clamp never fired and `var14` was left at the animated **100** (a literal-comparison artifact). 60 is the genuine max-energy budget at 18:30 day 1. This is the *intended* demonstration of the op111 fix, not a mask. **PASS.**

### #8 `m7_hmode_state.rs:189-191` — `var14 115→35, var16 4→3`
Same CE226 clamp; fresh **day-2 evening** has `var13 = 35` (budget decayed 120→60→35 over two days), so the clamped `var14=35`; `var16` is the day counter (CE329 pc222 `var16+=1`), 4→3 reflects the corrected sleep count reached. The 35 vs #2's 60 is **time-of-day**, not a contradiction — both equal their respective `var13` budgets. `M10_OPCODE_111_LANDING.md` §6 explicitly warns that rewriting a value to match a downstream number can mask a second bug; I checked that this does **not** apply here: the number is the direct output of the one changed comparison (`1600012 >= var13`), and `var13`'s decay is independent of the compound fix. **PASS** (documented caveat: provenance is by mechanism, not by an itemised energy ledger — acceptable, and consistent with #2).

### #5 `m7_battle_target_scheduler.rs:363` — `final var350 2 → 0`
Sequence `[0,1,2,0]`, with `2000352=0, 2000343=0, 2000349=1`, `rounds=1`. This is the **CE239 pc1043 abort cleanup**: after one command round the battle is undecided (`2000349=1` < CE244's default threshold `self1600010=11`), CE244 falls through without writing a result, CE239 loops to case2 (`pc877 2000343!=1`) and executes `pc1043-1046` (`350=0, 352=0, 343=0, 340=0`). So the state is coherent: no decision, phase returned idle, battle torn down. Old "stuck at 2" was the literal bug leaving `2000350` at 2 after the aborted round. **PASS.** *Nit:* the new code comment attributes the `2→0` to "CE241/CE242"; the actual writer is CE239 pc1043 (CE241 is boot-once and not running here). The assert is right; the comment should cite CE239 pc1043.

### #7 `m7_day4_playable.rs:428` — +`set_battle_round_loop(true)`
Only addition; `pre_power < 200` at :556 and the frame budgets are untouched. Witnessed: this route's `pre_power` stays **<200** (CE253 low branch), `rounds=2`, `v350=[0,1,2,0,1,0]`, `v352_peak=1`, `v343_peak=1`, `ce239_alive=false`, warnings 0. Enabling the round loop is *required* now that the enemy gauge spans multiple rounds; without it the fight would not settle in one command. Settlement still occurs inside the existing 1600-trace + 1600-drain window. **PASS.**

### #9 `m8_battle_party.rs:148-190` — `EXPECT` gains `settles:bool`
Pool mapping (the actual multi-member proof) is unchanged: cmds 1-4 → member-1/2 pools (`2000345`), 5-6 → Kana/Yui, etc., each draining its pool by 1. New: cmds 1-4 settle in one command (`v343=1, v352=1`), cmds 5-8 leave the enemy alive (`v343=0, v352=0`). Under reference semantics the enemy gauge and per-command damage are no longer the degenerate party/8 that one-shot everything, so weaker member commands genuinely need a second round; the test now asserts that honestly instead of pretending every command settles. Test 2 keeps the shipped CE253 single-command-settle behaviour green with the round loop **off**. This is a behaviour re-scope, not a mask: the assertions were tightened (settling commands still must write a real `{1,-1}`, non-settling must write nothing). **PASS.**

### #11 `m9_battle_round_gap.rs:458+` — single-round → structural multi-round
Renamed to `..._runs_genuine_multi_round_and_settles`; enables `set_battle_round_loop(true)`; loop 1600→4000. Assertions become structural: `main_phases.first()==0 & last()==0, command_rounds>=2, action_phases>=1`, phase-2-before-settle, `ce241_frames>0` (was `==0`), `ce243` first frame ≤ settle and `frames>0`, `v349_at_settle>=2` (was `==1`), `v352 in {1,-1}` (was `==1`). This is the right way to re-baseline an RNG-sensitive multi-round fight: assert the *shape* (`0→1→2→0` repeated, CE241/CE243 really running, a real result) rather than a brittle exact round count. The bytecode supports all of it (CE241 pc628 is the only normal `2→0` reset; CE243 is gated on `350==2` so it must appear once phase 2 is entered). **PASS** (conditional only on actual green, which the worktree run confirms; process-to-process round-count varies 3-4 — the structural asserts absorb that).

### #12 `m9_full_route.rs:495-634` — `pre_power<200 → >=200`, 40k frames
`var28 = 201` now crosses CE253's `>=200` gate → the HIGH (harder) branch; the fresh party fights a genuine 3-4-round battle with `v352 in {1,-1}`. The old low-branch instakill was the literal-operand artifact. My concern was whether a **loss** (`v352=-1`) still lets the route reach day 5 08:30; the witnessed worktree run is green, i.e. the day-4 battle outcome does not gate the day-5 transition on this route. Later asserts are unchanged and RNG-tolerant (`v352_peak in {1,-1}`, `v343_peak==1`, day5 08:30). **PASS.**

---

## 4. Cluster D/E — fixtures and transient state (#3, #4, #10)

**#3 `m7_battle_menu_text.rs` / #4 `m7_battle_seed.rs` (control runs).** Both add a *party-only* seed for `with_seed == false`: `CData(18,4,0)=1` (party), `CData(18,69,0)=1` (one enemy), and the leader stat block (`type0 row12` cols 20/21/22/23/24/25/26/27/28/9/10/4). Item/weapon/**command-slot** cells are deliberately left empty, so CE152's item gate still makes real attacks unselectable — the control's purpose (a real key press cannot select an attack) is preserved. Rationale is exactly the CE172 abort: with the corrected op111, CE172 pc32 `dead == total` aborts on `0 == 0` when there is no party or no enemy. Seeding the minimum live roster keeps the line alive to CE192 without turning on real attacks. **PASS** (fixture correction, not a behavioural whitewash).

**#10 `m8_hscene_forward.rs:890` — end `var218 == 2` → peak `var218_max == 2`.** CE372 pc22 sets `2000218 = 2` on the toilet stage, but CE371's timer (pc142/226/311/396/482/573/664) resets `var218 = 0` on expiry, so the value is 0 at run end. Tracking the **peak** (still must equal 2) proves the stage ran, whereas asserting the end value equal 2 was a literal-bug artifact. This is strictly stronger than a masked assert because it still requires the intermediate `2`. **PASS** (non-masking).

---

## 5. Risks / things a landing must still watch

1. **Determinism (#11, #12).** Natural continuous-route battles vary 3-4 rounds and win/loss across process runs because `started_events` uses a `HashSet` whose iteration order shifts RNG consumption before the battle (`EVENT_SEED` is fixed). #11 is structural and #12 accepts `{1,-1}`, so both are safe, but the landing must not later "pin" an exact round count. SaveData01-direct tests (#3/#4/#5/#8/#9/#10) are stable.
2. **#8 provenance** is by mechanism (CE226 clamp to decaying `var13`), not an itemised energy ledger. If the team wants belt-and-braces, add a short note in the landing doc tracing `var13` 120→60→35. Non-blocking.
3. **Comment accuracy nits** (do not affect asserts): #5's comment says CE241/CE242 produce the `2→0`; the real writer is CE239 pc1043 (undecided-battle cleanup). #7's `pre_power<200` is route-local (132) whereas #12 is 201 — worth stating explicitly so the two don't look contradictory.
4. **Patch fidelity (independently checked).** `git apply --check --exclude=docs/research/M10_OPCODE_111_LANDING.md work/out/m10_landing_full.patch` returns exit 0 against main-tree HEAD `ce680b3`; `git diff --stat 22148d8 HEAD` across all 13 code/test files is empty, i.e. the worktree base (`22148d8`) has no drift from HEAD for those files. The only apply conflict is the pre-existing untracked `docs/research/M10_OPCODE_111_LANDING.md` (task-74 artifact already in the tree), which is expected and not a code issue. The diff I inspected in `work/out/m10land` matches the patch. No stray edits to `game.rs`, `battle_wiring.rs`, or the 3 permanent scratch files.

---

## 6. Conclusion

The re-baseline is **methodologically sound and semantically correct**. Each of the 8 real-behaviour changes (#2,#5,#7,#8,#9,#10,#11,#12) is the direct consequence of the two verified VM fixes (op111 value dereference; compound `current`), or of correctly re-scoping a test to the genuine multi-round battle economy that the old literal bug had collapsed into a one-shot. None is a "rewrite the expectation to match observed" mask: the surviving assertions were tightened where the behaviour changed (settle vs non-settle, peak vs end, structure vs exact) rather than loosened. The 4 stale/fixture tests (#1,#3,#4,#6) are straightforward.

**Recommendation: PASS — land the patch** after (a) the two comment nits in §5.3 are corrected, and (b) the determinism caveat is preserved verbatim in `docs/research/M10_OPCODE_111_LANDING.md`. No blocking defects found.
