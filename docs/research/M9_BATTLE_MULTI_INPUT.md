# M9 W1 — Multi-input command menu: why the shipped CE188 line cannot register >1 command

Task: M9 residual (task-67). Question: can the shipped battle command menu
(CE188 → CE191 → CE192 → CE157) register more than one command per turn, so that a
real multi-member party acts? If yes, wire successive indexes through the rebuilt
driver (Game::set_battle_command_schedule). If structurally impossible, pin the exact
missing writer / loop condition.

**Verdict: structurally NOT reachable in the shipped event graph.** The *consumer* of a
multi-command turn already exists (CE194 行動順の計算 loops exactly コマンド入力位置
times), but the *producer* (CE157 コマンド登録) never advances that cursor. Every
selection overwrites command row 1, コマンド入力位置 stays 0 forever, CE194 would loop
zero times, and the CE188 real engine would execute zero commands. The player menu
therefore registers exactly **one** command (row 1 = command index 1). The multi-member
mechanics themselves are already proven separately at the CE242 layer (tasks 49/57: the
command index 1..8 selects one of eight distinct action bodies, each draining a different
party action-point pool 2000345/346/347/348).

Files:
- test: crates/wolf-game/tests/m8_battle_multi_input.rs
- driver (unchanged functionally): crates/wolf-game/src/battle_wiring.rs (doc note only)
- this doc.

Repro:
```
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_battle_multi_input -- --nocapture
```

Runtime evidence (this build, HEAD bcbcfba):
```
menu: rows_with_selectable=[1] registered_row=Some(1) code=30 max_input_pos=0 final_input_pos=0 \
      ce188=true ce191=true ce192=true ce194=false ce242=false warnings=0
ce157: max_pc=82 row1_code=26 row2_code=-1 input_pos=0 warnings=0
```

## 1. The live command-menu line (what actually runs)

CE188 X◆戦闘処理 (run0, 262 cmds) is the only real battle engine; its only static
callers are CE28@0 and CE37@53. Per invocation:

- pc41-45: init calls (戦闘初期化 / 戦闘キャラ配置 / 戦闘開始時処理 / 勝敗判定).
- pc52 c170 loop … pc69 c499: body pc54 c300 CALL CE191 (X┣◆行動内容のセット)
  return→self1600019; pc55 self1600018=0; pc56 c111[2, self1600019==1 → case1 pc57
  逃走, self1600019==10 → case2 pc64]. Otherwise it falls through and **exits the loop**
  to pc70 → the engine advances. So CE188 calls CE191 **once** per invocation; the
  per-member loop lives *inside* CE191.
- pc70 CE194 (行動順の計算) → pc116 CE195 (1ターンの処理を実行) → pc117 CE198 →
  pc175 CE199 → pc176 CE200 → pc177 CE201.

CE191 X戦闘メンバー選択処理 (run0, 150 cmds) has an internal loop pc85 c170 … pc134 c498.
Each iteration pc99 CALL CE192 (X味方コマンド選択画面) return→self1600011, then pc100
c111[3, self1600011 in {0,1,2}] dispatches (continue / cancel / confirm). CE192 (run0,
256 cmds) drives CE152/CE65 for the cursor and, on confirm, CE192 pc208 c300
[0,16777220,self1600000,self1600020,0,0,self1600010] calls CE157 (self1600002 = 0 =
*real registration* branch). CE192 pc36/pc49 are *preview* calls (self1600002 = 1).

So a single confirmed key selection produces CE157(self1600002=0). That is what the
runtime probe exercises.

## 2. CE157 X[共]コマンド登録 (run0, 175 cmds) — the producer

Writes the chosen command into the command list (c250 db_type 15 = physical type 14) and
owns the cursor CData(20,73,0) = コマンド入力位置:

| pc | op | effect |
|----|----|--------|
| pc117-120 | c250 t15 row=self1600017 cols0..3 = [self1600000, self1600018, self1600019, self1600001] | write command row (col3 = 発動技能 code) |
| pc121 | c111 [1, self1600023, 0, 2] (==0) | gate before the cursor advance |
| pc124 | c121 self1600017 += 1 | advance row index |
| pc125 | c250 [20,73,0,196608,self1600017] | **コマンド入力位置 = self1600017** |
| pc151,pc163-166 | c300 / c250 t15 row=self1600017 | same write, in the self1600002==1 branch |
| pc168 | c121 self1600017 += 1 | advance row index |
| pc169 | c250 [20,73,0,…,self1600017] | **コマンド入力位置 = self1600017** |

Only CE157 ever writes CData(20,73,0) (pc18 sub1, pc19 max1, pc27 assign1, pc38 max1,
pc125 assign, pc169 assign). Readers: CE157 pc13/pc84/pc154, CE5 pc88, **CE194 pc7**.

The real-registration branch (self1600002 = 0 → body pc36..) is: pc117-120 write row
self1600017, then **pc121 gate self1600023 == 0**. In the live path self1600023 is **not**
0 — it gets 100 written at pc93/pc100 when the resolved member id (self1600019) ≥ 100 — so
pc121 is false and execution skips pc122..pc127 (the pc124-125 cursor advance). The event
then finishes with the row written but the cursor untouched. The self1600002 = 1 branch's
advance (pc168-169) is also unreachable in practice: the selected instance terminates
around pc152 (observed max pc = 152), short of pc163.

Runtime proof of this exact gate: test
ce157_registers_row_one_but_never_advances_input_position starts CE157 with self1600000=1,
self1600001=26, self1600002=0; it writes CData(14,1,3) = 26, leaves CData(14,2,3) = -1,
and CData(20,73,0) = 0 (max pc 82, warnings 0). No invocation with self1600002 ∈
{−2,−1,0,1} advances the cursor.

## 3. CE194 X┣◆行動順の計算 (run0, 64 cmds) — the consumer that is never reached

pc7 c250[20,73,0,200704,self1600030] **reads コマンド入力位置** into self1600030.
pc12 self1600011=0. pc13 c179[self1600030] **loops self1600030 times**; each iteration
pc14 self1600011+=1, pc15 self1600012=self1600011+1, pc16/pc17 read command rows
self1600011 / self1600012 col1 (敏捷性) and sort by it. So **number of registered
commands == コマンド入力位置**, and CE194 iterates that many times. CE195 then executes
the sorted actions (calls CE197 1行動ループ).

Because the producer leaves コマンド入力位置 = 0, CE194's loop body never runs → the CE188
engine performs zero commands, i.e. the shipped real battle engine is dead. The player
menu line stops after registering exactly one row-1 command.

## 4. Why the day-4 / playable path still works (it bypasses this dead menu)

The shipped playable battle is driven by the *rebuilt* engine (battle_wiring.rs): it
starts CE242 directly with a command index in self-var 600 and does **not** rely on the
CE188 menu or on コマンド入力位置. Task-63 proved a single real menu selection is bridged
to the driver via the registered **row number** (row 1 = index 1) — code domain
{25,26,29,30} is disjoint from the CE242 index domain 1..8, so the bridge uses the row,
not the code. Tasks 49/57 proved the driver honours arbitrary indexes 1..8 (each →
distinct action body → distinct pool) and that a real multi-round fight is possible on the
BOSS path (CE239 pc202 2000445==1 → gauge 2000446) using the real CE241 turn-start. None
of that needs the CE188 menu.

## 5. Precise missing writer / loop condition (the blocker to pin)

- **Missing writer:** CE157's cursor advance at **pc124 (self1600017 += 1) and pc125
  (CData(20,73,0) = self1600017)**, gated by **pc121 self1600023 == 0**; and equivalently
  **pc168/pc169** in the self1600002==1 branch. In the live path self1600023 is set to 100
  (pc93/pc100) before pc121, so the advance is skipped, and the self1600002==1 branch
  terminates before pc163.
- **Missing loop condition:** nothing re-enters CE191/CE192 with an already-advanced
  コマンド入力位置, so each member selection overwrites row 1 instead of appending to row
  N+1. CE194 (which would execute the accumulated rows) is consequently always fed 0.

## 6. Minimal fix proposal (NOT implemented here — no speculative hardcoding)

The architecture is intact; only one cursor update is missing. Two honest options:

1. **Producer fix (data/engine semantics):** make command registration append instead of
   overwrite — after writing command row self1600017, always advance コマンド入力位置
   (= number of commands) so CE194's real loop runs them. Concretely the pc121 gate must
   not be defeated by self1600023, i.e. the registration must reach pc124-125 (or an
   equivalent "pos = count" write). This is a one-writer change and keeps CE194/CE195 as
   the executing engine.
2. **Engine-driven turns (already available):** drive turns with the rebuilt
   battle_wiring.rs round loop, starting CE242 with a schedule of command indexes (one
   member per round), using the real CE241 turn-start — already proven on the BOSS path
   (task-57). This does not need the CE188 menu at all.

We implement neither (both are product/engine decisions beyond a research pin), and we do
not seed コマンド入力位置 or fake rows in shipped code, because that would fabricate a
turn the data never produces.

## 7. Files & scope

- crates/wolf-game/tests/m8_battle_multi_input.rs — 2 tests, both green
  (live_menu_registers_one_command_and_never_advances_input_position,
  ce157_registers_row_one_but_never_advances_input_position).
- crates/wolf-game/src/battle_wiring.rs — module-doc note only (no behaviour change; the
  round-loop driver already covers option 2).
- No commit / no push. Existing regressions untouched.
