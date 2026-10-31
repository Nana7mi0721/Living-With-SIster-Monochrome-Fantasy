# M7 W1 (task-20) — Real CE244 settlement: the reachable command-phase round and the exact menu-entry blocker

Status: **investigation complete.** This document proves a real, engine-driven CE244
settlement round (the sole writer of 2000352 / 2000343) and pins the one edge that stops a
*menu-driven* turn from starting in the shipped event graph: the circular bootstrap of the
three-state battle flag 2000350. It also documents the closest real-entry path (CE239's own
`210 500244` edge) that executes CE244 yet writes no settlement, and proves that CE239's
return/unwind (to pc1106) works as soon as a settlement exists. No pre-existing source or
doc was modified; the deliverable is the new test
`crates/wolf-game/tests/m7_battle_full.rs` plus this file.

Test (HEAD 1d36ebe, data root work/all):

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_battle_full -- --nocapture

Result: **1 passed** (~1-2 s). Emitted:

    SC0 CE188 line: ce143=true copyHP=100/100 ce152=true ce192=true target_win=true selectable=true max65pc=132 left_spin=false unanswered250=0 warns=0
    SC1 CE242 round: v350_seq=[0, 1, 0, 2, 0] saw350=1:true saw350=2:true v352=1 v343=1 v349=8 final350=0 frames=200 warns=0
    SC2 CE239 entry: battle_active=true turn_set=true v350_max=0 v352=0 v343=0 max_pc=796 hotspring_region=true else_region=false end_judgment=false warns=0
    SC3 CE239 unwind: entered_victory_case=true max_pc=1099 alive_at_end=false warns=0

The task-19 helper `Game::seed_battle_databases(&mut self) -> usize` (crates/wolf-game/src/game.rs:514,
returns 25) is called explicitly at the start of every scenario; it is deliberately not
auto-invoked by `Game::new`. Game entry points used: `start_event` (game.rs:503),
`start_engine_events` (game.rs:523), `frame` (game.rs:1030); interpreter `compare`
(crates/wolf-vm/src/exec.rs:1489, operator 0=`>`,1=`>=`,2=`==`,3=`<=`,4=`<`,5=`!=`).

---

## 0. Four scenarios at a glance

| # | driver | what it proves | settlement? |
|---|--------|----------------|-------------|
| SC0 | `start_event(188)` + task-19 seed, 400 f | real CE188 line boots to the command window; a real attack (codes 26/25/30/29) is selectable; CE65 opens | **No** — CE188 has no edge to CE244 |
| SC1 | `start_event(242)` + `2000495=2` + self1600000=1, 200 f | genuine CE242→CE244 command/execute cycle, 2000350 0→1→2→0 | **Yes** — 2000352=1, 2000343=1 |
| SC2 | `start_engine_events`+`start_event(239)` + `2000495=2`, 900 f | real battle caller executes its own pc795 `210 500244` edge | **No** — CE244 called but writes nothing |
| SC3 | `start_event(239)` with 2000340=1,343=1,352=1 pre-seeded, 1200 f | CE239 end-judgment victory branch unwinds and returns (pc1104/1106) | n/a (return half only) |

---

## 1. Scenario 0 — the real CE188 battle line reaches a selectable real attack

Drive: `Game::new` → `load_save(work/retail_backup/Save/SaveData01.sav)` →
`seed_battle_databases()` (re-applied frames 1..=60 so table clears during CE143/CE144 do not
strand the copy) → `start_event(188)`, `auto_advance/auto_choice` on, `key_bits = OK` on even
frames.

Frame/state evidence:

| observation | value | meaning |
|---|---|---|
| CE143 ran | `reached_ce143=true` | 主人公DB→戦闘DBコピー executed |
| type10 row0 col1/col2 | **maxHP=100 / HP=100** | CE143 copies leader stats into 戦闘一時ステータス |
| CE152 ran | `reached_ce152=true` | 戦闘コマンド一覧算出 executed |
| CE192 reached | `reached_ce192=true` | 味方コマンド選択 menu opened |
| CE157/CE158 | `reached_target_window=true` | target-selection windows open |
| type19 rows r0..r3 | codes **26/25/30/29**, 選択可能 col2=1 | a **real attack is selectable** |
| CE65 max pc | `max65pc=132` (`left_spin=false`) | 万能ウィンドウ opened; it does not hard-lock |
| warnings / unanswered-250 | **0 / 0** | clean run |

CE188 (record 188, 262 cmds) calls CE189/190 at pc41/42, CE191 at pc54, CE192, CE152, CE65 —
but it has **no** call to CE244, so this line cannot produce a settlement. It is the honest
"battle boots and accepts a real attack" milestone; the settlement lives in the command-phase
line below.

---

## 2. Scenario 1 — the reachable CE244 settlement (command-phase round)

Drive: `Game::new` → `load_save` → `seed_battle_databases()` → `vm.vars.set(495, 2)`
(2000495=2, battle-active / hot-spring flag; CE242 pc15 uses it to *skip* the 万能ウィンドウ
command menu, which needs unresolved CData(19,·) strings, and run command 1's heal branch
headlessly) → `start_event(242)` → `self_vars.set(600, 1)` on the live CE242 instance
(self1600000=1, the player's chosen command). 200 frames, `key_bits=OK` even frames.

Observed 2000350 (flat slot 350) transition sequence, excluding repeats:

    v350_seq = [0, 1, 0, 2, 0]

with the per-frame detail (matches docs/BATTLE.md §5.3 on the same head):

| frame | 2000350 | writer | phase |
|---|---|---|---|
| f0 | 0 | — | idle |
| f1 | 1 | CE242 pc14 | command phase (コマンド処理中) |
| f91 | 0 | CE244 pc312 | **CE244 settles** (2000352=1 @pc315, 2000343=1 @pc316) |
| f94 | 2 | CE242 pc208 | execute phase hand-off |
| f95 | 0 | CE241 pc628 | turn start clears back to idle |

Settlement variables observed at end: **2000352 = 1**, **2000343 = 1**, 2000349 = 8,
final 2000350 = 0, warnings = 0. 2000352 = 1 is written **only** at the win site pc315 (§3),
so this run reached the victory settlement; the traced 戦力 values were 2000192 = 41 and
2000193 = 0. This is a real,
engine-driven settlement: CE244 (432 cmds, run=0, cond 2000000==0) actually executes and
writes 2000352/343, which are written **only** by CE244 (§6).

---

## 3. CE244 終了判定 — opcode branch table (record 244, 432 cmds)

Setup and gate instructions (index = pc; from work/_ce_full.json). Note the string labels
(212 SetLabel / 213 JumpLabel) are mojibake in the dump, so branches are identified below by
their **SET sites**, which are unambiguous:

    pc0  121 [1600000,0,0,0]        self1600000 = 0
    pc2  121 [2000353,0,0,0]        2000353 = 0
    pc3  121 [1600010,11,0,0]       self1600010 = 11
    pc4  111 [1,2000354,20,2]       IF 2000354 == 20  -> pc6 self1600010 = 5
    pc9  111 [17,2000495,2,2]       IF 2000495 == 2   -> pc11 self1600010 = 5
    pc36 111 [1,2000349,1600010,2]  IF 2000349 == self1600010
    pc44 111 [17,2000192,2000193,0] IF 2000192 > 2000193  (戦力 comparison)

The three settlement sites (each is a distinct label target, reached through the pc36/pc44
gate and jump network above):

    win  site : pc311 label, pc312 2000350=0, pc313 2000342=0,
                pc314 self1600000=1, pc315 2000352=1, pc316 2000343=1
    lose site : pc370 self1600000=-1, pc371 2000352=-1, pc372 2000343=1,
                pc373 2000342=0, pc374 2000350=0
    -2   site : pc415 self1600000=-2, pc416 2000352=-2, pc417 2000343=1,
                pc418 2000350=0, pc419 2000342=0

2000352 uniquely identifies the site (1 at pc315 / -1 at pc371 / -2 at pc416); 2000343 = 1
and 2000350 = 0 at every site. -2 is the excluded "第三结局" per docs/BATTLE.md §5.3.

CE242 calls CE244 at pc201 as `210 [500244, 16777216, 1600001]` (flag 16777216 = capture the
return value into self1600001); pc202 `111 [1,1600001,-1,19]` then `172 BreakEvent` ends the
command script if CE244 returned -1. CE242's own exit path is pc208 `121 2000350=2`.

---

## 4. Scenario 2 — the real battle caller CE239 executes its CE244 edge but cannot settle

Drive: `start_engine_events()` (boots the run==2 auto events + GATED_CALL_ONLY=[242,243]) →
`start_event(239)` with `2000495=2`, 900 frames.

Observed: `2000340 = 1` (battle active, CE239 pc695), `2000349 = 1` (CE239 pc564), but
**2000350 stays 0 the entire run**, and 2000352 / 2000343 stay 0. CE239 reaches **max pc 796**
and takes the hot-spring case1 body pc751..796 (not the else body pc798..874), i.e. it runs
*through* the real CE244 call edge. Opcode proof around that edge:

    pc749 111 [17,2000495,2,2]   IF 2000495 == 2  -> case1
    pc750 401 [1]                case1 begin
    pc768 179 [3]                LoopTimes 3 (loop body pc769..785)
    pc786 498 []                 LoopEnd
    pc791 123 [1600030,144]      key poll
    pc792 121 [2000477,0,0,0]    2000477 = 0
    pc795 210 [500244, 0]        <- REAL call to CE244 (return value discarded)
    pc796 0 []
    pc797 420 [0]                else
    pc798 121 [2000354,1600003,0,0]   else body sets 2000354 = self1600003, NO CE244

Why CE244 does not settle here even though it is invoked: the command machine never starts
(2000350 = 0 → CE242/CE243 never run → §7), so CE244 is entered in an unprepared state and
its settlement gates are not satisfied — 2000354 = self1600003 (unset ⇒ 0 ≠ 20); 2000349 = 1
≠ self1600010 (= 5, since 2000495==2); and CE240 (戦力処理, run==2) has set
2000192 = 2000193 = 100, so pc44's `2000192 > 2000193` is false. None of the win/lose/draw
write sites is reached. Result: 2000352 = 0, 2000343 = 0 across 900 frames.
CE239 then loops pc13 and, because 2000343 never becomes 1, its end-judgment case2 waits
(case2 victory branch is gated on 2000343==1; see SC3). This is the "closest real path": the
shipped caller *does* reach the CE244 call site, but with the flags unset it is a no-op.

Note on `--watch-start 244`: CE244 is called as an inline synchronous `210` (not a spawned
child), so it never appears as its own instance. The `2000352`/`2000343` writes (sole writers,
§6) are the reliable settlement signal, not instance watching.

---

## 5. Scenario 3 — CE239 return/unwind works once a settlement exists

Drive: `start_event(239)` with a completed settlement pre-seeded
(2000340=1 battle active, 2000343=1 戦闘終了, 2000352=1 勝ち負け), 1200 frames, OK held.

Observed: CE239 enters its **case2 victory branch** (pc ≥ 876), runs to **max pc 1099**
(the last sampled pc is a `Child(131)` wait; the instance then ends cleanly), and
`alive_at_end = false` — i.e. it unwinds through pc1104/1106 and returns. warnings = 0.
This isolates the return half: **CE239 can return to pc1106 as soon as 2000343 = 1 exists.**
The only missing link for a full real turn is the engine wiring that starts the command
phase (CE242) on a real 2000340 rising edge (§8).

---

## 6. Event headers and authoritative writer scan

Headers (work/_ce_full.json):

| event | run | op | condvar | condval | argn | role |
|---|---|---|---|---|---|---|
| CE239 | 0 | 2 | 2000000 | 0 | 4 | ■戦闘呼び出し (needs a caller; none in graph) |
| CE240 | 2 | 1 | 2000340 | 1 | — | 戦力処理 (writes 2000192/2000193) |
| CE241 | 1 | 2 | 2000350 | 2 | — | ターン開始処理 (ENGINE_AUTO_START) |
| CE242 | 0 | 2 | **2000350** | **1** | 1 | コマンド入力 (command phase) |
| CE243 | 0 | 2 | 2000350 | 2 | — | 敵の行動 (execute phase) |
| CE244 | 0 | 2 | 2000000 | 0 | — | 終了判定 (settlement) |

Authoritative `121` (SET) destinations:

    2000350 = [ CE239 pc98/1043 ->0 | CE241 pc628 ->0 | CE242 pc14 ->1 |
                CE242 pc208/309/471/556/616/754/944/967/1054/1131/1191/1400/1433 ->2 |
                CE243 pc1314 ->2 | CE244 pc312/374/418 ->0 ]
    2000352 = [ CE239 pc1044 ->0 | CE244 pc315 ->1, pc371 ->-1, pc416 ->-2 ]
    2000343 = [ CE239 pc1045 ->0 | CE244 pc316/372/417 ->1 ]

=> **CE244 is the sole writer of 2000352 and 2000343; only CE242 pc14 sets 2000350 = 1.**
There is no other setter of the command-phase flag anywhere in the 550-event graph.

---

## 7. Root cause — the 2000350 circular bootstrap (the precise missing trigger)

The three-state command machine is driven **only** by run_condition on the same flag it
represents:

* CE242 (コマンド入力) has `run=0, condvar=2000350, condval=1`: it starts only when
  **2000350 == 1**.
* The only writer of 2000350 = 1 is **CE242 pc14 itself** (§6).
* There is **no external starter**: no event issues `210 500242`; CE188 has no CE244 edge;
  CE243/CE241 are likewise flag-gated (2000350==2) and only CE242 pc208/· sets 2000350=2;
  CE239 is a passive `run=0 cond 2000000==0` script with **no map caller** and is listed in
  no engine auto-start set.

So a real turn deadlocks before the command phase: nothing turns the crank to set
2000350 = 1, so CE242 never runs, so CE243/CE241 never run, so CE244 never settles. This is
a **structural data gap**, not a bug in the interpreter or scheduler (auto_events.rs maps the
run==2 events correctly; the state machine simply was never given an external entry in the
shipped common-event graph). Equivalently: CE188 is a fully separate turn engine with no
caller of its own, and CE239 is a fully separate battle caller with no caller — the two never
wire into the flag-gated CE242/243/244 machine.

---

## 8. Minimal fix proposal (do **not** hardcode; wire an entry, labelled "rebuilt")

Preferred (BATTLE.md §5.3.1 option B — *rebuilt wiring, data has no such call*): add a small
engine hook that, on a **rising edge of 2000340 (battle-active 0→1)** while the command flag
2000350 is idle, starts the command-phase script CE242 (i.e. `start_event(242)` /
auto-start it once). That supplies the one missing external starter while leaving every real
script, condition and settlement write intact — CE242→CE244 then runs its genuine
0→1→2→0 cycle exactly as Scenario 1 shows. This must be documented as *rebuilt wiring* (the
shipped graph has no such `210 500242`), not as a recovered data path.

Honest alternative (option A): keep the observable seeded round of Scenario 1 (2000350
0→1→2→0, real CE244 settlement) as the demonstrated depth and state plainly that the
menu-driven entry is unreachable in data. Do **not** fake the 万能ウィンドウ menu, and do
**not** hardcode 2000350=1 or pre-write 2000352/2000343 as if the engine produced them.

Related honest boundary: the day4 08:30 → day5 08:30 continuation that consumes a battle
settlement (CE253 → CE239 → CE326 → CE329) is a separate schedule task; the settlement half
proven here (`2000343 = 1`) is exactly the precondition that continuation needs.

---

## 9. Reproduce / seeds / scope

Command: `WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_battle_full -- --nocapture`
(repo root, HEAD 1d36ebe). Seed cells are produced by `Game::seed_battle_databases()`
(task-19; game.rs:514, returns 25: type18 row4 col0=1; type0 row12 col47..54=[3,4,6,7,0,0,0,0]
= attack codes 26/25/30/29; type0 row12 stats c20=100,c21=100,c22=50,c23=50,c24=20,c25=15,
c26=10,c27=30,c28=30,c9=20,c10=10,c4=10; type7 row0 col0=1; type8 rows0..2 col0=1).

Scenario seeds on top of that:

* SC1 (settlement): `vm.vars.set(495, 2)` (2000495=2) + `self_vars.set(600, 1)` on the live
  CE242 instance (self1600000=1). These two are the *only* extra seeds; they stand for
  "battle is active" and "the player picked command 1", not for battle stats.
* SC2 (real entry): `vm.vars.set(495, 2)` only, plus `start_engine_events()`.
* SC3 (unwind): `vm.vars.set(340,1)`, `vm.vars.set(343,1)`, `vm.vars.set(352,1)`.

No commit / push. Only `crates/wolf-game/tests/m7_battle_full.rs` (new) and this file (new)
were authored; no existing source or doc was edited, and the three permanent scratch files
(crates/wolf-format/examples/opscan.rs, crates/wolf-format/tests/_scratch_cdata.rs,
crates/wolf-format/tests/_scratch_event.rs) were never touched.
