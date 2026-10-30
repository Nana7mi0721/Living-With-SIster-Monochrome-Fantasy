# M6 — REAL Battle Line: reachable boundary & minimal gap

Status: research note for task-8, corrected under task-12 (teammate m6-night2). HEAD f5aca1c.
Scope: prove the REAL WOLF battle line (CE239 -> CE188 -> CE189/CE190 -> CE191 -> CE192 ->
CE65 -> CE157 command selection -> ... -> CE244 settlement) runs as far as it actually can in a
real `Game`, WITHOUT the legacy CE242/CE243 (var350-gated synthetic hook) path, and document the
exact point / why it stops.

Deliverables: this doc + `crates/wolf-game/tests/m6_battle_real.rs`.
No commit/push. No edits outside these two files (`game.rs` / `exec.rs` / `main.rs` HEAD-clean).

> Correction (task-12): an earlier draft wrongly claimed CE143/CE144 fail to copy HP/level. That
> came from reading the wrong `CData type10` columns. With the correct column map (§2) and a party
> row seeded at the correct `type0` columns, **CE143/CE144 DO copy HP/maxHP/action into `type10`**
> (§4). The honest boundary below (only sentinel commands selectable, CE244 not reached) is unchanged.

---

## 1. Verdict (short)

- The REAL line is reachable and runs DEEP. From a real `Game` + retail save, starting `ce188`
  (◆戦闘処理) directly, the engine executes in order:
  **CE188 -> CE189 戦闘初期化 -> CE190 戦闘キャラ配置 -> CE143 主人公DB→戦闘DBコピー ->
  CE144 敵DB→戦闘DBコピー -> CE191 行動内容のセット -> CE192 味方コマンド選択 ->
  CE152 戦闘コマンド一覧算出 -> CE65 万能ウィンドウ (command list) -> CE157 コマンド選択**.
  A command row IS selectable and is selected.
- CE143/CE144 correctly copy the party/enemy battle stats into `CData type10 戦闘一時ステータス`
  when the source rows are populated at the right columns (HP/maxHP/action all land — §4).
- It does NOT complete a full turn / reach **CE244** settlement. After a command is confirmed
  (only the sentinels `-2 何もしない` / `-1` are selectable; real attack rows 26/25 are marked
  unavailable), control enters CE20 コマンド処理 -> CE157 コマンド選択 -> CE159 -> CE154 ->
  CE159 -> **CE65 万能ウィンドウ**, whose target list has row count `CData type18 row17 == 0`.
  CE65 loops **pc78 <-> pc140** forever. `2000350 / 2000352 / 2000343` stay 0; `warnings == 0`;
  CE244 never reached. The blocker is the **command-availability / target-list gap**, not HP copy.

## 2. CData column maps used here

`CData type10 戦闘一時ステータス` (16 logical fields):
`0 name, 1 max HP, 2 HP, 3 max MP, 4 MP, 5 max courage, 6 courage, 7 melee atk, 8 ranged atk,
9 defense, 10 speed, 11 max action, 12 action, 13 action count, 14 body, 15 skill`.

`CData type0 主人公ステータス`:
`20 max HP, 21 HP, 22 max MP, 23 MP, 24 melee, 25 ranged, 26 defense, 27 max action, 28 action,
47..54 コマンド1..8`.

(An earlier draft read `type10` cols 4/5 as HP/maxHP — those are actually MP/max MP — which is why
it wrongly concluded HP was not copied. HP is `type10` col 2, max HP col 1, from `type0` 21/20.)

## 3. Precondition: the raw loaded party points at an empty template row

`work/retail_backup/Save/SaveData01.sav` -> `game.load_save(...)` -> `savegame::apply` ->
`Databases::apply_saved_database` (`crates/wolf-game/src/db.rs:486`). Probe:

```
type6 row0 (party) = [0 12 -1 -1 -1 -1 -1]     // money=0, member1 -> 主人公ステータス ROW 12
type0 r0 : name='勇者先生'   hp=99/99
type0 r12: name='温柔的'     hp=0/0            // the party's leader row: EMPTY template
type0 r13: name='リコ'       hp=99/99
```

`type6 row0 col1 == 12` and `CData type0 row 12` is the `.dat` template with HP 0; populated
combat chars (hp99) sit at rows 0..5 etc. CE190 pc12 reads 基本システム用変数 row4 = party count
(0 in the raw save), so the ally loop (`179 [self20]`) runs 0 times and CE143 never runs — hence the
harness supplies a battle-ready party row.

## 4. Seed and proof that CE143/CE144 DO copy battle stats

Seed in `m6_battle_real.rs` (correct `type0` columns):
- `CData type18 row4 col0 = 1` (party count).
- `CData type0 row12`: c20=100 maxHP, c21=100 HP, c22=50 maxMP, c23=50 MP, c24=20 melee,
  c25=15 ranged, c26=10 defense, c27=30 max action, c28=30 action, c9=20, c10=10, c4=10,
  c47..54 = [1,2,3,4,0,0,0,0] コマンド1..8.

Measured after CE188 (~7 frames), reproduced by the test:
```
type10 row0 = [0, 100, 100, 0, 0, 0, 0, 0, 20, 10, 0, 0, 0, 100, 50, 0]
             name mHP  HP  mMP MP mCou Cou mAtk rAtk Def Spd mAct Act ActCnt Body Skill
```
**max HP (col1)=100 and HP (col2)=100 are copied** from `type0` c20/c21 — CE143 works. Command
slots (cols 27..34) also copy = [1,2,3,4,0,0,0,0]. CE144 lands positive HP in an enemy `type10` row
(~frame 30). Note: seeded `type0` c27/c28 land at type10 action-count(13)=100 / body(14)=50 in this
build; the semantic action slot (col12) reads 0 — secondary field placement, not the blocker.

## 5. What is in the CE192 command window (the real blocker)

`CE152 戦闘コマンド一覧算出` fills `CData type19` (万能ウィンドウ一時DB) with 8 rows. Dump of
`type19 rows 0..8 [項目コード col1, 選択可能 col2]` at the open command window:

```
r0 [c1=-2, c2=1]   // 何もしない — selectable
r1 [c1=-1, c2=1]   // second sentinel — selectable
r2 [c1=26, c2=0]   // a real attack/skill — NOT selectable
r3 [c1=25, c2=0]   // a real attack/skill — NOT selectable
r4..r7 [c1=0, c2=0] // empty
```

Only sentinels `-2`/`-1` are selectable. Auto-OK lands on row0 = `-2` (何もしない). DOWN+OK moves
to row1 = `-1` and it IS dispatched (`CE192 pc208 -> CE20 コマンド処理 -> CE157`), but proceeds into
an empty `CE159/CE154` target window and CE65 spins.

Command availability in `CE152 pc31` is a 3-case `op111`
`[3, self1600010, -3, op2,  self1600010, -1, op3,  self1600010, 0, op1]`
(if/elif: `==-3` / `<=-1` / `>=0` — every integer hits one case) and each case body marks the row
unavailable. That, plus the empty/legacy `Data6` command mapping and the missing item/weapon DB,
leaves the real attacks unavailable.

## 6. The precise gap (named)

To reach CE244 (sole writer of `2000352 = 1/-1/-2` and `2000343 = 1`):
1. **Command availability** — CE152 pc31's test must let a real attack through (item code 26/25
   currently falls into the "unavailable" branch for every integer case); root cause is the
   empty/legacy `Data6` command mapping.
2. **Item/weapon DB `CData type8 所持武器個数`** — attacks discarded as unavailable (many
   `250 ... 所持武器個数 unanswered` reads).
3. A populated **target battle list** so the `CE157/CE158/CE159` target window row count > 0
   (today `type18 row17 == 0`).

None of this is an HP-copy / engine-VM defect: `exec.rs` is HEAD-clean and CE143/CE144 demonstrably
copy real stats.

## 7. Evidence summary (measured)

Run: `WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m6_battle_real -- --nocapture`
(OK pulsed even frames; DOWN then OK when the CE65 command window is open on row0; seed frames 1..=45;
300 frames, 0.14s).

| item | value |
|------|-------|
| reached CE143 / CE192 / CE157 | true / true / true |
| selectable command row seen | true (type18 row17>=1 & type19 row0 col2==1) |
| type10 row0 (f7) | `[0,100,100,0,0,0,0,0,20,10,0,0,0,100,50,0]` (HP/maxHP copied) |
| enemy type10 HP (>0) | found (~f30) |
| boundary stack | `[188@55 506@39 191@100 192@209 157@50 159@79 65@78]` (CE65 spin pc78<->140) |
| v350 / v352 / v343 | 0 / 0 / 0 |
| warnings | 0 |
| CE244 | never reached |

## 8. Reproduce

```
cd 'E:/Study/github program/imoto life'
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m6_battle_real -- --nocapture
```

Test `m6_battle_real_reaches_command_selection` (0.14s) asserts: type10 row0 equals the measured
copied vector (HP/maxHP copied from `type0` c20/c21), a positive enemy type10 HP exists, reaches
CE192 and CE157 command selection, `v352/v343 == 0` (CE244 not reached), `warnings == 0`.

## 9. Pointers (line-level)

- CE188 setup calls: `work/_ce_full.json` id188; op210 blocking = `WaitState::Child` at
  `crates/wolf-vm/src/exec.rs:1209`.
- CE190 party read/loop: id190 pc12 `250 [20,4,...]`, pc14 `179`, pc17 `300 CE143`,
  pc36 `250 [20,69]`, pc37 `250 [20,72]`, pc43 `250 [13,self21,0,332288,1600022]`, pc68 `300 CE144`.
- CE143 stat/command copy: id143 pc29..58 read 主人公ステータス (HP c21, maxHP c20, コマンド c47..54),
  pc62..97 write `CData type10` row0 (HP col2, maxHP col1, コマンド cols27..34).
- CE144 敵DB→戦闘DBコピー: id144, reads `Data type9 敵ステータス`, writes `CData type10` enemy rows.
- CE150 戦闘コマンド取得: id150 pc23..48 read `type10 [10,1600000,27+self1,...]`.
- CE152 戦闘コマンド一覧算出: id152 (called CE192 pc60); pc31 availability 3-case op111;
  fills `CData type19` + rowcount `type18 row17`.
- CE192 味方コマンド選択: id192 pc60 CE152; pc137/pc190 CE65; pc208 `300 CE20`; pc209 dispatch.
- CE65 万能ウィンドウ: id65 pc73 read type19 col1 -> item code; pc120 read type19 col2 (選択可能);
  pc121 if 選択可能==1 break else SE error (the pc78<->pc140 spin).
- DB layering: `crates/wolf-game/src/db.rs` `set_num`(600)/`set_text`(612),
  `apply_saved_database`(486), `resolve_labels`(633); `type6` party row0 col1 = 主人公ステータス row id.
