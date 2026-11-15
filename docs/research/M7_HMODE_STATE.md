# M7 H-mode / intimacy state (task-36)

Derive the variable state a real mid-H save carries, and pin down exactly why the H CG
common event **CE333** cannot be opened from CE246's 性骚扰 (sexual harassment) deep branch
by a pure global-variable setter.

Deliverable code: `crates/wolf-game/tests/m7_hmode_state.rs` (2 tests, green).
Run: `cargo test -q -p wolf-game --test m7_hmode_state` (~2.2s, deterministic, warnings 0).

Context: task-29 (`docs/research/M7_HSCENE_ENTRY.md`) proved a non-synthetic H entry
(day-2 19:30 bedroom -> night command menu -> 身体接触 -> CE246 six-way skinship menu) but
CE333 (CG) / CE346 (cut-in) stayed gated. task-21 (`docs/research/M7_HSCENE_FULL.md`, PNG
89.4% non-black) reproduces the full H layer stack only from a mid-H save. This task seeks
the H-mode variable state that would open the deep branch.

---

## 1. Variable diff: fresh day-2 evening vs the mid-H retail save

Fresh baseline: new game driven through the real day loop to day-2 19:30
(`day2_evening_game()` in the test, identical to m6_night_menu.rs). It is bit-identical to
the state at the CE246 skinship menu — opening the night command menu does not touch these
variables (verified: fresh@19:30 == fresh@CE246menu for every id below).

Mid-H snapshot: `work/retail_backup/Save/SaveData08.sav`, loaded by `Game::load_save`.

| var | fresh day-2 | retail SaveData08 | meaning hint |
|----:|------------:|------------------:|--------------|
| 7   | 2936 | 7586  | money / progress |
| 8   | 0    | 25    | |
| 13  | 60   | 120   | |
| 14  | 115  | 75    | |
| 16  | 4    | 2     | deep gate (needs >= 1) |
| 18  | 22   | 60    | affection tier |
| 20  | 75   | 90    | deep gate (needs > 60) |
| 21  | 20   | 688   | intimacy counter (deep gate needs >= 100) |
| 22  | 0    | 375   | intimacy tier (deep gates need >= 50 / 100 / 200); CE38 pc2 zeroes 2000001..=2000601 |
| 23  | 5    | 9     | relationship level (deep gate needs >= 8 at pc1951) |
| 28  | 103  | 1815  | |
| 62  | 0    | 1     | **H-scene stage index** (深夜UI処理) |
| 64  | 0    | 1     | H-mode armed |
| 70  | 0    | 1     | |
| 102 | 0    | 120   | deep gate (needs >= 30 / 40) |
| 137 | 3    | 8     | H scene pose selector |
| 138 | 0    | 50    | |
| 150 | 0    | 44    | |
| **157** | **1** | **12** | **CE333 deep gate (pc1943 needs == 13)** |
| 160 | 2    | 90    | |
| 200 | 0    | 506   | |
| 452 | 0    | 10    | |
| 365 | 0    | 1737  | |

The five "blocking defaults" on a fresh file are `var22 == 0`, and
`var70 = var102 = var150 = var200 = var452 = 0`; the deep branch returns early long before
it can reach the CE333 tail. The elevated retail column is exactly the H-mode / intimacy
state. Note the mid-H save still has **var157 == 12, not 13** — the CE333 gate is not even
satisfied by a real mid-H playthrough.

Both columns are asserted exactly in `h_mode_var_table_is_derived_from_the_retail_save`.

---

## 2. CE246 deep branch gate chain (to CE333)

All pcs below are CE246 (`BasicData/CommonEvent.dat`, event 246). Decoding from
`crates/wolf-vm/src/exec.rs`: opcode 111 is a **switch** (first matching triplet wins ->
case_target(i+1), else else_pc) with operators `0 >, 1 >=, 2 ==, 3 <=, 4 <, 5 !=`; opcode
121 is `[target, a1, a2, bits]` (Magic::decode(target) stored, a1/a2 through eval_num).

Entry: 性骚扰 (choice index 5) opens the deep five-way submenu
`[让我看内衣, 让我看裸体, 想看你自慰, 让你看我撸管, 小鸟飞飞]`.

Guards before the deep option body:
- pc557 `IF var20 <= 60 -> RETURN` (need var20 > 60; fresh 75, retail 90 both pass)
- pc564 `IF var151 >= 1 -> RETURN`
- pc572 CHOICE[5] (the deep menu above)
- pc575 var21 >= 100; pc577 var22 >= 50; pc581 var23 >= 6; pc593/pc610 var22 >= 100/50;
  pc629 var22 >= 200 -> pc631 SET var439 = 1; pc642 var18 >= 50; pc644 var16 >= 1;
  pc647 CHOICE[2] `[让我看更多些, 就先到这里]`; pc651 var102 >= 30; pc674 var102 >= 40;
  pc681 var18 >= 80; pc687 CHOICE[1] `看破！`; pc695 CALL ev346 `[3,24,1,1]`;
  pc705 CHOICE[1] `发射`.

CE333 call site: **pc1983 `CALL ev333 [4,2,1,0,3]`**, inside the deep option **4 (小鸟飞飞)**
tail, which ends at pc2058 `CALL ev245` (return to the night command menu). Immediately
before it: pc1982 `SET var166 = 1`. The tail is gated by:
- **pc1943 `IF var157 == 13`**  <-- the decisive external gate
- pc1951 `IF var23 >= 8` (3-case switch)
- pc1954 `IF self scratch 1_600_010 == 0 / == 1` (2-case switch; one case is a block that
  returns at pc1966)

CE346 cut-in is also called from CE246 at pc695/906/1477/1543/1598/1692/1799 (`[3,24|25,1,1]`).

---

## 3. The precise blocker: global var157 == 13 has no literal writer

Scanning the whole `BasicData/CommonEvent.dat` (+ `MapData/SampleMapA.mps`) for every
opcode-121 store into the **global** var157 (magic `2_000_157`) yields exactly:

| site | writes var157 = |
|------|-----------------|
| CE258 pc1  | literal 16 |
| CE282 pc342 | **SELF variable copy** `1_600_013` |
| CE318 pc437 | literal 2 |
| CE318 pc983 | literal 2 |
| CE329 pc311 | literal 16 |
| CE329 pc663 | **SELF variable copy** `1_600_013` |
| CE453 pc10  | literal 1 |
| MAP ev0 page0 pc42 | literal 1 |

So the only literal values ever assigned to global var157 are **{1, 2, 16}**. There is **no
literal `var157 = 13` anywhere in the data.** The value 13 is reachable only when CE282
pc342 or CE329 pc663 executes `var157 = <self var 1_600_013>` and that event-local self
variable happens to be 13 at that moment.

`1_600_013` is in the **self-variable** magic range (`crates/wolf-vm/src/magic.rs:30-36`:
SELF_BASE 1_000_000, VAR_BASE 2_000_000; self range 1_000_000..=1_999_999), i.e. it is an
event-local slot (`inst.self_vars`), **not** a global. A global variable-state loader
(`game.vm.vars.set(...)`) cannot set it, and even setting the global var157 = 13 does not
reproduce the control flow the branch expects.

The exact event/pc that reads the gate: **CE246 pc1943 reads global var157 and compares
`== 13`**.

---

## 4. Empirical evidence (warning-free, no deadlock)

After loading the mid-H snapshot, `start_engine_events()` + `start_event(246)`, choose
性骚扰 (cursor 5), then 小鸟飞飞 (deep cursor 4), then auto-advance:
- the deep branch runs to its tail: max event-246 pc ≈ **2049** (>= 2000);
- event 246 ends cleanly (returns to the night menu via pc2058 `CALL ev245` => the event
  246 instance disappears; no deadlock, 3000-frame budget not exhausted);
- `stats.warnings == 0`;
- **CE333 and CE346 never start** (tracked per-frame over the whole run);
- forcing global `var157 = 13` every frame changes nothing: warnings stay 0, var157 reads
  13, and CE333/CE346 are still not started.

Both the "don't start" and the "forcing the global gate doesn't help" facts are asserted in
`ce333_deep_branch_needs_an_event_local_self_variable_gate`, together with the static
writer-set proof above.

Conclusion: the H-mode variable **state is necessary but not sufficient**. The last gate is
**event-local self-variable control flow** (self`[1_600_013] == 13` inside CE282/CE329,
feeding global var157 == 13 at CE246 pc1943), driven by real H-scene progression. It cannot
be produced from a global variable snapshot, and it is not reachable from the mid-H save
(which carries var157 == 12).

---

## 5. Recommendation for a production H-state loader (do not implement here)

Rather than a fragile "set every global to a mid-H snapshot" API, expose one of:

1. **H-scene entry API** that starts the real H wrapper by map event, already proven
   headless in task-21: `Game::start_map_event_by_name("SampleMapA", 33)` after loading a
   mid-H save and skipping the opening (see `m7_hscene_full.rs`), instead of trying to
   satisfy CE246's internal self-variable gate. This reproduces the full H layer stack
   (bg + poses + face + ef/action + frame, ~86 pictures, warnings 0).
2. **Self-variable-aware state loader** that, in addition to globals, can seed each common
   event's `SelfVars` (the `inst.self_vars` bank addressed by magic 1_000_000..) so the
   CE282/CE329 `var157 = self[1_600_013]` copy can be driven to 13. This mirrors
   `crates/wolf-vm/src/vars.rs:161` `SelfVars` and `exec.rs:1355` `store_num` for the self
   bank, and would let a test reproduce the CE246 -> CE333 tail directly.

Neither is required to reproduce player-visible H content: the map-event-33 wrapper (option
1) already provides it. Option 2 is only needed if a caller must drive CE246's deep branch
deterministically.

---

## 6. Key file / line references

- `crates/wolf-game/tests/m7_hmode_state.rs` — this task's deliverable (2 tests).
- `crates/wolf-vm/src/magic.rs:30-36` — SELF_BASE/VAR_BASE; self range 1_000_000..2_000_000.
- `crates/wolf-vm/src/exec.rs:1672-1691` — opcode 121 (target, a1, a2, bits); a1 in self
  range is a self copy.
- `crates/wolf-vm/src/exec.rs:1489-1503` — opcode 111 compare operators (2 ==).
- `crates/wolf-vm/src/vars.rs:72-77` — Variables::get/set take usize index;
  `crates/wolf-vm/src/vars.rs:161` — SelfVars; `exec.rs:1355` — store_num self bank.
- `work/retail_backup/Save/SaveData08.sav` — the mid-H reference snapshot.
