# M7 W2-a — Advancing an H scene interactively through its opcode-123 polls

Status: **done / green** (`cargo test -q -p wolf-game --test m7_hscene_advance` -> 1 passed, 14/14 polls, warnings 0).
Scope: test-only. New files: `crates/wolf-game/tests/m7_hscene_advance.rs` and this doc. No prod-code edit, no commit.

## 1. What is proven

The H common events advance through **14 opcode-123 poll gates**, and each gate is
walked purely from `game.key_bits` (the OK / decide key), exactly the way a player
taps the screen — no variable is prod-poked to skip the gate. The test drives the
real `Game` engine (loads a retail save, starts the engine events, starts the H event)
and asserts, for every one of the 14 polls:

* `key_bits = 0` -> the event instance parks on the poll **retry** pc (its 50-frame
  wait loop) and never reaches the **continue** pc;
* `key_bits = wolf_game::key::OK` (OK bit) -> the instance takes the **continue**
  branch and reaches the continue pc.

So the branch taken is a function of real key input. Counts: **14/14 polls advanced**,
**warnings = 0**.

## 2. The poll shape (why key_bits decides it)

Every poll is the same instruction shape inside the H common events:

```
121 [target, 1, 0, 256]               ; write key code to a scratch self var
123 [target, 17]                      ; target <- host.key_code_auto(mask)  (reads game.key_bits)
111 [2, target, 0, 2, target, 10, 2]  ; key==0 -> retry block ; key==10 -> continue block
... retry    : 180 [50]  (wait 50 frames) then re-poll
... continue : 180 [10]  (wait 10 frames) then scene advances
```

`wolf_game::key::OK = 1` (bit 1); `key::code(OK) == 10` (CODE_OK). The opcode-123 poll
is level-triggered, so holding the OK bit in `game.key_bits` every frame passes the poll
every frame (`key_code_auto` at `crates/wolf-game/src/game.rs` reads `crate::key::code(self.key_bits)`).

## 3. Event / pc map — all 14 polls

`blocks --event <id>` decodes the 111/401 jump tables. The pc pair for each poll is
**(retry_pc, continue_pc)**; the test asserts membership of these exact pcs in the
instance's visited-pc set.

### CE346 — two polls, selected by arg[0]
| poll | route / seed | retry pc | continue pc |
|------|--------------|----------|-------------|
| pc48  | `self[600..]=[2,1,1]` (arg0=2 -> block2 case2) | 52 | 55 |
| pc119 | `self[600..]=[4,0,1]` (arg0=4 -> block10 case1) | 123 | 126 |

(arg0 = 24/25, the CE246 default, routes to the *wait* branches at block13/14/18/19 —
no poll; that is why the earlier "seed [24,1,1]" probes seemed to ignore the key.)

### CE371 — seven polls, one per global `var218 = 2000218` stage
arg[0] is irrelevant for reaching the poll (any 1..12); the global picks the stage/poll.

| global var218 | poll pc | retry pc | continue pc |
|---------------|---------|----------|-------------|
| 1 | 74  | 79  | 83  |
| 2 | 157 | 162 | 166 |
| 3 | 239 | 244 | 248 |
| 4 | 326 | 331 | 335 |
| 5 | 412 | 417 | 421 |
| 6 | 502 | 507 | 511 |
| 7 | 586 | 591 | 595 |

(var218 = 0 or 8 do not reach a poll.)

### CE410 — five polls, one per arg[0] 1..5
| arg[0] | poll pc | retry pc | continue pc |
|--------|---------|----------|-------------|
| 1 | 40  | 44  | 47  |
| 2 | 87  | 91  | 94  |
| 3 | 134 | 138 | 141 |
| 4 | 177 | 181 | 184 |
| 5 | 221 | 225 | 228 |

(arg[0] = 6 -> pc245, no poll. Extra globals `var143 = var144 = 0` pin the sub-branch
so the test matches the CLI-proven route deterministically.)

Total = 2 + 7 + 5 = **14**.

## 4. Key schedule (what the test injects)

Per case, two fresh runs over `frames = 80` frames:

* **no-key run**: `game.key_bits = 0` every frame.
* **OK run**: `game.key_bits = wolf_game::key::OK` every frame.

Setup for each run (identical to the working `m6_*` harness):

```rust
let mut game = Game::new(data_root())?;          // data root work/all
game.auto_advance = true;
game.auto_choice = true;
game.load_save(&repo_root().join("work/retail_backup/Save/SaveData08.sav"))?;
game.start_engine_events();
let id = game.vm.start_event(EVENT).unwrap();
// route args into SelfVars group-6 slots (magic 1600000+i == index 600+i):
game.vm.instance_mut(id).unwrap().self_vars.set(600 + k, arg[k]);
// method-2 global seeds (magic 2000000+i == Vars index i), e.g. vars.set(218, g):
game.vm.vars.set(addr, val);
loop { game.key_bits = if inject { wolf_game::key::OK } else { 0 }; game.frame(); /* record inst.pc */ }
```

`load_save` restores the picture layer + normal vars but **not** coroutines; the H event
is started explicitly with `start_event`, so the poll logic actually runs.

## 5. Assert values (exact)

For each case the test asserts on the visited-pc `BTreeSet`:

* `no_key.contains(retry_pc) == true`
* `no_key.contains(cont_pc)  == false`
* `ok_key.contains(cont_pc)  == true`

and finally `advanced == 14` and `warnings == 0` (`game.stats.warnings`).

Additionally a **static-layer** sanity check on a plain `SaveData08` load (no H event
started, 30 frames) asserts `picture_quads >= 39` — the same textured-quad floor the
capture-backed render uses — and reports `warnings == 0` and which `run_condition==2`
H text events are live.

Observed stderr on a passing run:

```
m7_hscene_advance: 14/14 opcode-123 polls advanced on key_bits=OK only; warnings=0
test result: ok. 1 passed; 0 failed; ... finished in ~5.2s
```

## 6. Commands

```
# the test (green, ~5.2 s, deterministic across repeated runs)
cargo test -q -p wolf-game --test m7_hscene_advance -- --nocapture

# decode a poll's jump table
cargo run -q -p wolf-cli -- --root work/all blocks --event 346
cargo run -q -p wolf-cli -- --root work/all blocks --event 371
cargo run -q -p wolf-cli -- --root work/all blocks --event 410

# reproduce one gate from the CLI (arg0=2 poll at pc48: retry 52 vs continue 55)
cargo run -q -p wolf-cli -- --root work/all dump --event 346 --self 600=2 --self 601=1 --self 602=1 --frames 40 --watch 346
cargo run -q -p wolf-cli -- --root work/all dump --event 346 --self 600=2 --self 601=1 --self 602=1 --frames 40 --watch 346 --auto-input

# static H render / screenshot (89.4% non-black, 0 warnings)
cargo run -q -p wolf-cli -- --root work/all capture --load-save work/retail_backup/Save/SaveData08.sav --frames 60 --out work/out/m7_hscene_advance_static.png
```

dump notes: the self-seed flag is `--self slot=value` (there is no `--slot`); globals use
`--var addr=value`; `--auto-input` holds code 10 every frame (equivalent to the test's
`key_bits = OK`); `--load-save` is only honoured by capture/play, so the CLI dump runs with
globals at their defaults (which is what the pc tables above were measured against and then
re-confirmed inside the `Game` harness with the save loaded).

## 7. Screenshots / render counts

* Test-harness static load: `picture_quads = 39` (asserted `>= 39`), `warnings = 0`.
* `work/out/m7_hscene_advance_static.png` — 1024x576, **527251/589824 = 89.4%** non-black,
  75 picture ops, 2456 instructions, 0 warnings. This is the H scene the polls sit on top of
  (the static render W2-b already covered), used here as the visual reference the advance happens against.

## 7b. run_condition==2 H text events (task question)

Of the H text auto events `[350, 351, 353, 354, 357, 358, 367]`, under `SaveData08` only
**ev357** is scheduled after `start_engine_events` + 30 frames (`live_h_text_auto=[357]`);
the other six are not running under this save (their run conditions are unmet at this
state). So the text/word layer is not fully auto-scheduled from the save alone — the
remaining events depend on scene state the wrapper chain sets up. Recorded, not asserted.

## 8. Unknowns / caveats

* **polls are stage-selected, not one linear walk.** CE346 picks its poll by arg[0] (2 vs 4),
  CE371 by the global `var218` (1..7), CE410 by arg[0] (1..5). The test exercises each stage's
  gate in isolation with the exact route that reaches it; it does not assert a single
  uninterrupted 14-step playthrough, because the caller (`CE246`/`CE372`/`CE402`) drives the
  stage index and re-enters per stage.
* **who advances `var218` / arg[0] between stages** is the parent event's job (`CE372`/`CE323`
  for 371, `CE402`/`CE328` for 410, `CE246`/`CE315`/`CE322` for 346). Reproducing the whole
  wrapper chain end-to-end (starting from the in-map H button, `SampleMapA ev33` -> `CE333`) is
  not attempted here; the poll gates themselves are proven key-driven.
* **word_effect / hword on-screen timing** (noted in `M7_BACKLOG.md`) is not touched: this task
  is the opcode-123 advance only.
* The continue branch parks on a short `180[10]` wait and then loops back to the stage entry
  (`pc 67`); the assertions are on the poll branch pcs, which are unique to the gate, not on a
  terminal state.
