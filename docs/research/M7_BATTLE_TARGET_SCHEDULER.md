# W1 battle support — CE190 target generation and the 2000350 scheduler

Status: test-only investigation, HEAD 1d36ebe. Deliverable test:
crates/wolf-game/tests/m7_battle_target_scheduler.rs (6 tests). No production
code changed.

Verification command (green, 6 passed, 0 failed, 2.09 s):

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_battle_target_scheduler -- --nocapture

## 1. Target generation: CE190 / type18

### 1.1 What CE190 actually reads

CE190 X[戦]戦闘キャラ配置 has 83 commands. The database reads that drive the
target rows are (raw words, magic encoding):

| pc | opcode 250 words | meaning |
|----|------------------|---------|
| 12 | [20, 4, 0, 69632, 1600020] | self1600020 = CData type18 row4 col0 = 味方人数 (party count) |
| 36 | [20, 69, 0, 69632, 1600020] | self1600020 = CData type18 row69 col0 = 敵人数 (enemy count) |
| 37 | [20, 72, 0, 69632, 1600021] | self1600021 = CData type18 row72 col0 = 敵グループID (enemy group id) |

arg0=20 is the label 基本システム用変数 and resolves to CData type18
(docs/PARTY_DB.md:260,262; crates/wolf-game/src/db.rs:650 resolve_labels).
CE190's enemy branch then calls CE144 敵DB→戦闘DBコピー (pc68) and CE175
敵キャラ_座標算出 (pc70), and draws with CE179 (pc73/pc76).

### 1.2 Minimal seed matrix

Game::seed_battle_databases() (crates/wolf-game/src/game.rs:521-526,
src/battle_seed.rs) writes 25 cells. Relevant ones for this task:

- CData type18 row4 col0 = 1 (party count)
- CData type0 row12 cols47..54 = [3,4,6,7,0,0,0,0] (command slots)
- Data type6-derived command codes 26/25/30/29 become selectable together with
  CData type7 row0 col0 = 1 (item stock, CE152 pc89-95 gate)

It deliberately omits:

| database | type | row | col | required value | effect |
|----------|------|-----|-----|----------------|--------|
| CData | 18 | 69 | 0 | 1 (or >=1) | 敵人数 — CE190's enemy loop runs; CE162 has a target |

This is the only missing cell needed to make enemy-target selection usable.
CData type18 row72 col0 = 0 (group id 0) is already the default and is enough;
type13 row0 / type9 row0 already contain a usable enemy, so no other seed change
was required in the measured run.

After the shipped seed plus set_num(CData, 18, 69, 0, 1), running CE190 for 600
frames produced:

- type18 r69 = 1
- type18 r35 = 512, r36 = -2880, r37 = 5120, r38 = 3120 (enemy screen coords)
- type18 r51 = 6, r17 = 15
- type10 row0 maxhp = 100, hp = 1, character number = 12

Without the cell (row69 = 0) CE190's enemy loop is zero iterations and type18
row69 stays 0.

### 1.3 CE162 敵対象選択実行 takes the seed

CE162 internals: pc12 reads type18 row69 into self1600011 (enemy count); pc15-21
build the enemy order self1600040..46 = [5,3,1,0,2,4,6]; pc48/49 draw with
CE179; pc50 op123 waits key 177; pc51 branches on left/right/confirm; confirm
pc57 sets self1600010 = self1600012 (selected enemy number); pc58 op171 exits.

Measured CE162 behaviour when started directly after CE190:

| seed | result |
|------|--------|
| shipped seed (row69 = 0) | CE162 loops pc49<->pc50 for 240+ frames, never reaches pc57, instance still alive (pc49/50) |
| + CData type18 row69 = 1 | CE162 path f0 pc12 -> f1 pc34 -> f2 pc49 -> f3 pc50 -> f4 pc55 -> f5 pc57, then exits; enemy index 0 selected |

So "target window open with a valid enemy selectable" is true iff type18 row69 >
0. The test asserts both the control spin and the seeded success.

### 1.4 Correction to the task hint / earlier docs

The hint (and docs/research/M6_BATTLE_COMMANDS.md §4 line 77) calls CE157/CE158
the target windows. That is wrong. Authoritative CommonEventData titles:

- CE157 X[戦]コマンド登録 (175 cmds) — registers command rows, not a window.
- CE158 X[戦]アイテム選択実行 (140 cmds) — item selection; calls CE161 (ally).
- CE161 X[戦]味方対象選択実行 (101 cmds) — ally target selection.
- CE162 X[戦]敵対象選択実行 (104 cmds) — enemy target selection.
- CE163 X[戦]敵·味方行動対象算出 (416 cmds) — multi-target resolution.

Callers of the real target selectors:

- CE158 pc67 -> CE161
- CE159 pc143 / pc215 -> CE161; CE159 pc247 -> CE162
- CE164 pc58 -> CE163; CE167 pc67 -> CE163

### 1.5 Dead end: CE188 command line never reaches CE162

Driving CE188 with the default seeded command slots and selecting code 30
(强斩) by pressing DOWN twice + OK produced:

- events seen: {143,147,152,156,157,172,188,189,190,191,192,64,65}
- never seen: 159, 161, 162
- type18 row17 history: [(0,15),(33,2),(60,8)]
- final stack: 188@55, 191@100, 192@144, 64@105
- warnings 0

CE190/CE143/CE144 and CE157 コマンド登録 run, but CE192 pc208 only registers and
redraws, then loops. CE162 is reachable only from CE159 pc247 (技能選択実行) or
through CE163; neither is invoked from the CE188 command-selection loop in this
line. The missing trigger is therefore an execution-side caller of CE159/CE162,
not another database cell. This is why a synthetic start_event(162) is used to
prove the generation half independently.

Also measured and ruled out: selecting ordinary attack code 0, 26, or 30 and
pressing OK simply returns to the command list (max CE192 pc 212); type18 row17 is
the command-row count (8 or 15), not a target-row count and not a condition for
CE162.

## 2. CE239 / CE242 / CE241 and 2000350

Magic 2000350 decodes to plain variable index 350. The complete writer census
across all 550+ common events:

| value | writers (event pc) |
|-------|--------------------|
| 1 | CE242 pc14 only |
| 2 | CE242 pc208,309,471,556,616,754,944,967,1054,1131,1191,1400,1433; CE243 pc1314 |
| 0 | CE239 pc98,1043; CE241 pc628; CE244 pc312,374,418 |

There is no op210 call with target 500242 anywhere in the data: nothing naturally
starts CE242.

### 2.1 Natural CE239 route

start_engine_events() + var 2000495 = 2 + start_event(239), 900 frames:

- 2000350 transitions: [(0,0,0,0,0)] (initial 350=0, 352=0, 343=0, 349=0)
- final: 350 = 0, 352 = 0, 343 = 0, 349 = 1
- max CE239 pc = 796; events seen = {231,232,239,241,264,271,48,63,70,98}
- CE242 and CE244 never run.

Slot matrix self1600002 = 0..=8, 250 frames each, same hotspring setup: every
slot gave maxpc239 = 796, max 350 = 0, CE242 = false, CE244 = false, final
352 = 0, 343 = 0, 349 = 1. The hotspring branch at pc749 ([2000495]==2) preempts
the pc16-97 dispatch; pc795 does call CE244 directly, but CE244 cannot settle
because its precondition 2000349 == self1600010 fails (349 = 1), so 352/343 stay
0.

### 2.2 Injected CE242 shows the full cycle

start_event(242) + self600 = 1 + var 2000495 = 2, 200 frames, transitions of
2000350:

    (frame, 350) = (0,0) -> (2,1) -> (92,0) -> (95,2) -> (96,0)
    after frame 92: 352 = 1, 343 = 1
    final: 350 = 0, 352 = 1, 343 = 1, 349 = 0, max CE242 pc = 208, warnings 0

The state sequence 0 -> 1 -> 0 -> 2 -> 0 is observable, and 352=1 / 343=1 proves
the CE244 win branch (CE244 pc312-316) ran. It is observable only because CE242
is injected.

### 2.3 Blocking edge

- 2000350 = 1 is written only by CE242 pc14, and CE242 has no data caller.
- CE239 writes only 0 (pc98/pc1043); CE241 writes 0 at pc628; CE244 writes 0 at
  its settle points.
- The natural CE239 route therefore keeps 2000350 = 0 forever. A rebuilt engine
  hook now exists in crates/wolf-game/src/battle_wiring.rs (task-34): when enabled
  with Game::set_battle_wiring(true), it starts CE242 on the rising edge of
  2000340 0 -> 1. It is OFF by default, so a plain boot and the CE239-direct probe
  above still see 2000350 = 0.
- With that wiring enabled the data graph can reach phase 1 and phase 2, but the
  settlement still does not fire; see 2.4 for the exact remaining gate.

## 2.4 Rebuilt wiring path and the remaining CE244 gate

Measured with Game::set_battle_wiring(true) + start_engine_events + var 2000495 = 2
+ start_event(239) + CE239 self1600003 = 1 (the command a real caller passes),
1500 frames:

- battle_wiring_rounds = 1, max 2000340 = 1
- 2000350 transitions: 0 -> 1 (frame 66) -> 2 (frame 326)
- final: 350 = 2, 352 = 0, 343 = 0, 349 = 1

The command phase now advances past the old bootstrap, but CE244 never settles.
The exact gate is CE244 pc36: 111 [1, 2000349, 1600010, 2] (2000349 ==
self1600010). CE244 pc4 maps 2000354 to self1600010: command 20 -> 5, command
7/13 -> 10, everything else -> 11. In the wired CE239 run 2000349 is left at 1
while self1600010 = 11, so the gate fails, CE244 skips its win/lose/draw arms
(pc312/374/418), and 2000352/2000343 stay 0. Precise remaining edge: the battle
setup/caller must leave 2000349 equal to CE244's expected self1600010 for the
chosen command (11 for a default command, 5 for command 20), or the command must
be routed so those values agree. Nothing in the seed sets 2000349.

## 3. Test inventory

crates/wolf-game/tests/m7_battle_target_scheduler.rs:

1. ce190_reads_type18_rows_4_69_72_and_seed_omits_the_enemy_count
2. enemy_count_seed_turns_the_ce162_spin_into_a_selectable_enemy
3. only_ce242_writes_2000350_one_and_nothing_calls_ce242
4. ce239_slots_never_raise_2000350
5. injected_ce242_exposes_the_three_state_cycle
6. battle_wiring_advances_to_phase_two_but_ce244_gate_blocks_settlement

All six passed in 2.09 s under WOLF_DATA_ROOT=work/all.