# M7 W1-a — Real battle round to CE244: reachable depth and the exact blocker

Status: **investigation complete, CE244 settlement proven unreachable in the shipped
event graph.** This document pins the deepest honest milestone (a real CE188 battle line
that boots fully and exposes a selectable real attack plus target-selection windows) and
the precise structural reason the CE244 settlement (the only writer of 2000352 /
2000343) can never run under real play. No pre-existing source or doc was modified; the
accompanying test is crates/wolf-game/tests/m7_battle_round.rs.

Test: `WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_battle_round -- --nocapture`
Result: **1 passed** (~0.33 s). Emitted line:

    M7_BATTLE_ROUND: A(max188pc=55, ce143=true, copyHP=100/100, ce152=true, ce192=true,
      target_win=true, real_attack_selectable=true, ce244=false, warns=0) |
      B(battle_active=true, turn_set=true, v350_max=0, ce242=false, ce243=false,
      ce244=false, v352=0, v343=0, max239pc=857, warns=0)

No legacy CE242/CE243 seeding is used anywhere: scenario B merely runs
`game.start_engine_events()` (the same engine bootstrap a normal play session uses) and
observes that CE242/CE243 never start on their own.

---

## 1. Scenario A — the real CE188 battle line boots all the way to command selection

Driven with `game.start_event(188)` after loading a real retail save, plus documented
data preconditions (party count, party-leader command slots, leader stats, one copy of
item 0). These are the data a real save/flags supply; they are **not** fabricated window
rows.

Frame evidence (data root work/all, save work/retail_backup/Save/SaveData01.sav):

| frame | live instance stack (event@pc) | what it proves |
|------|-------------------------------|----------------|
| f1–f5 | 188@41→42, 189 戦闘初期化 | CE188 begins the real turn loop, CE189 init |
| f6–f40 | 188@42→43, 190 戦闘キャラ配置, 143 主人公DB→戦闘DBコピー @146, 144 敵DB→戦闘DBコピー | CE190 places allies/enemies |
| f7 | type10 row0 = [_, maxHP=100, HP=100, …] | **CE143 copies the leader's HP/maxHP into 戦闘一時ステータス** (cols 1/2) |
| f101–f103 | 191 行動内容のセット @72→83→94 | action-content selection opens |
| f104–f119 | 192 味方コマンド選択 @34→47→56→57→61→86→108→136 | CE192 command menu starts, CE152 runs |
| f121 | 192@138, 65 万能ウィンドウ @78 (mode self1600001=4) | command window drawn |
| f121 | type19 rows: r0=code 26 sel1, r1=25 sel1, r2=30 sel1, r3=29 sel1 | **real attacks are selectable** |
| f134–f136 | 192@209, 157 コマンド選択 @64→82 | confirming a command opens CE157 target selection |
| f139+ | back to 191@94→192@34… | CE157/CE158 returns without committing a target under plain OK input |

Assertions pinned by the test: CE143 ran (copy maxHP=100, HP=100), CE152 ran, CE192
reached, a real attack row (26/25/30/29) became selectable, CE157/CE158 target window
opened, **CE244 never appears**, warnings == 0.

Key CE188 structure (work/_ce_full.json record id 188, 262 cmds):
`pc41` CALL CE189 初期化; `pc42` CALL CE190 配置; `pc54` CALL CE191 行動内容のセット
[0,16777218,1600018,1600001,1600019]; `pc70` 行動順; `pc116` １ターンの処理を実行;
`pc117` ターン終了処理; `pc174` 戦闘不能処理; `pc175` 主人公ＤＢ復元; `pc176` 戦利品;
`pc177` 戦闘終了処理; `pc223` 戦闘メッセージ; `pc254` `210 [1600030,0]` — a **dynamic
post-battle event** id (read from CData type17 row0 col30 at pc250, +500000 when >=0),
NOT CE244. CE188 contains **no** call to CE244.

Why the round never settles from CE188 alone: CE244 終了判定 is the sole writer of both
2000352 (勝ち負け: pc315=1 win / pc371=-1 lose / pc416=-2) and 2000343 (戦闘終了:
pc316/372/417=1). CE188 has no edge to CE244, and there is no caller of CE188 anywhere
else in the graph (see §2), so nothing in this line can write the settlement variables.

---

## 2. Scenario B — the real CE239 battle-caller line pins the 2000350 deadlock

Driven in a full session: `game.start_engine_events()` (auto-starts ENGINE_ALWAYS
[48,261,269,270,271,224,225,213,214,41] and, via auto_events GATED_CALL_ONLY =
[242,243], the 2000350-gated command machine) then `game.start_event(239)`.

Frame evidence: CE239 reaches its battle-active case and sets the turn counter, but the
three-state command machine never leaves idle.

| variable | observed | meaning |
|----------|----------|---------|
| 2000340 (戦闘中) | 0 → **1** | CE239 pc695 sets battle active |
| 2000349 (経過ターン) | 8 → **1** | CE239 pc564 sets the turn counter |
| 2000350 (コマンド処理中) | **stays 0 for all 1600 frames** | machine never entered |
| CE242 / CE243 | **never started** | they fire only when 2000350 == 1 / == 2 |
| CE244 | **never runs** | only caller set would set 2000352/2000343 |
| 2000352 / 2000343 | **0 / 0** | settlement never written |
| CE239 max pc | 857 | stuck in battle-active case1, never enters end-judgment |

### The exact blocker: a circular bootstrap on 2000350

An all-event writer scan (op121 SET, destination == target) gives:

- **2000350** is written to `1` by **only CE242 pc14**. It is set to `2` by CE242
  (many pcs) and CE243 pc1314, cleared to `0` by CE239 pc98/pc1043, CE241 pc628, CE244.
- **CE242** has run condition `run==0, condvar 2000350, condval 1, op ==`. It is
  auto-started by the engine's GATED_CALL_ONLY scheduler only when 2000350 == 1.
- **CE243** has run condition `run==0, condvar 2000350, condval 2, op ==`.
- **CE244** (the ONLY writer of 2000352 and 2000343) is called from **op210 500244** by
  only CE239, CE241, CE242, CE243.

Therefore: CE244 requires the command machine (242/243) to run; 242/243 require
2000350 ∈ {1,2}; the only event that ever produces 2000350 = 1 is CE242 itself. CE239's
battle setup only *clears* 2000350 to 0 (pc98). The bootstrap is circular and cannot
self-start. This is not an input problem — 1600 frames of OK pulses, party/item seeds and
a live enemy id change nothing because the state machine is never entered.

### CE244 is also structurally cut off from CE188

CE188 (X◆戦闘処理) is called by op300 (by name) from **CE28 pc0** and **CE37 pc53** only.
Neither CE28 nor CE37 has any caller anywhere in the 550 common events (no op210/op300
references them). So CE188 is effectively dead code in the shipped event graph, and in
every case it cannot reach CE244. CE239, by contrast, is widely called from map/story
events (CE253 pc174-191 and CE256/257/258/285-289/297-301/305/424-427/…) — it is the real
battle entry the shipped game uses — and it is precisely the line that deadlocks on 2000350.

### CE239 case structure (record id 239, 1108 cmds)

- `pc11` op170 main loop; `pc12` op180[1]; `pc13` `111 [2,2000340,0,2, 2000340,1,1]`
  → case1 (2000340 == 0) body pc15-874; `pc875` 401 → case2 (2000340 >= 1) body
  pc876-1103; `pc1104` 499; `pc1106` 498.
- case1 (setup/presentation): `pc98` SET 2000350=0; `pc564` SET 2000349=1; `pc695` SET
  2000340=1; `pc703` CALL 500264; `pc734` 戦闘開始演出; `pc749` IF var2000495==2 →
  the ONLY pc795 CALL 500244 sits inside the hot-spring (温泉) branch; the normal path
  pc797+ skips CE244.
- case2 (pc876 戦闘終了判定): `pc877` IF 2000343==1 → victory/end branch; else pc948+ →
  pc1034 戦闘不能 → pc1043-1046 clear 2000350/352/343/340. CE239 blocks here until
  2000343 == 1, which only CE244 produces.

---

## 3. Verdict and minimal options

**CE244 settlement is unreachable through the genuine event graph as shipped.** The
real battle line reaches the command menu (CE188 line) or the battle-active presentation
state (CE239 line), but the settlement state machine is never entered because the only
writer of its entry value (2000350 = 1, CE242 pc14) can only run once that value is
already set. Acceptance in the strict sense (CE244 reached, 2000352 ∈ {1,-1,-2},
2000343 == 1, warnings 0) therefore cannot be satisfied by driving inputs or seeding
battle data.

Minimal, non-fabricating options — the choice is the Lead's call, none applied here:

1. **Data-precondition (M6-consistent)**: seed 2000350 = 1 once at battle start (as an
   explicit documented precondition, exactly like the party/item seeds), then let the
   engine's GATED_CALL_ONLY scheduler run CE242 → set 2 → CE243/CE241 → CE244. This
   exercises the real settlement events; it bypasses only the missing bootstrap write.
   This is the smallest step that yields 2000352/2000343 from real CE244.
2. **Engine scheduler fix (real bug candidate)**: when 2000340 transitions 0 → 1 (battle
   becomes active, CE239 pc695), have the runtime seed 2000350 = 1 so the command machine
   starts. This is a behavioural change in game.rs and should be regression-checked
   against the M6 night/battle suites before adoption — it is *not* speculative
   event-specific hardcoding, but it does alter switch timing.
3. Keep the milestone at the deepest honest assertion (this test): real line boots, real
   attack selectable, target windows open, and CE244 pinned unreachable with exact vars.

Option 1 is the most faithful to the project's "seed data preconditions, don't fake
menus" principle and is what the next milestone (drive CE242→CE244 settlement) should
build on.

---

## 4. Reproduce

    cd "E:/Study/github program/imoto life"
    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_battle_round -- --nocapture

Static references: work/_ce_full.json records 188, 239, 242, 243, 244, 28, 37, 240,
241; docs/research/M6_BATTLE_REAL.md (type10 field map + seeds); docs/research/
M6_BATTLE_COMMANDS.md (CE152/item-availability path). Data: data root work/all,
save work/retail_backup/Save/SaveData01.sav.
