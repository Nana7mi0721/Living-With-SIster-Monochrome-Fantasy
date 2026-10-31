# M10 Phase 0 — 完整战斗数据库夹具与 CE172 计数证明

Status: 实测文档（task-66）。唯一新增/改动：本文件 + `crates/wolf-game/tests/m10_battle_db_fixture.rs`（新测，未提交）。**无生产代码/无其它测试改动/无 commit**。HEAD `bcbcfba`（工作树含其它队友未提交批次，见文末）。
数据根 `work/all`，脚本 `work/_ce_full.json`，二进制 `./target/debug/wolf-cli.exe`。

## 0. 结论（一句话）

**"op111 参考语义补丁会让 CE172 提前判胜负中断"这一假设被实测证伪。** CE190「戦闘キャラ配置」在每次真实战斗都会把敌方人数 `type18 row69` 重算为组内真实阵容（实测 1→7）、并把所有落位单位标为存活（`type10 col3=0`）。因此真实运行中 CE172 的 `死数(0) != 总数(1/7)`，**永不进入 pc32/pc53 的中断分支**。7 个 unclear 失败**没有一个**是 CE172 中断，全部是更下游的 magic111 站点分歧（CE64/CE65/CE152/CE161/CE160/CE163 一线）。空阵容 `0==0` 立即判胜只是**合成夹具**的退化产物。

## 1. CE172 指令级结构（实测，来自 work/_ce_full.json）

`CE172 id=172 "X[戦]敵/味方·勝敗判定" run=0, 124 slots`：

| pc | 指令 | 作用 |
|---|---|---|
| 8 | `121[1600099,0]` | 判定值 = 0 |
| 12 | `250[21,72,0,69632,1600030]` | 敌方组 ID ← CData **type18 row72** |
| 13-16 | `250[13,<组>,col7/8/9/10,...,1600033/35/34/36]` | 组条件：胜利条件/胜利A/败北条件/败北B ← CData **type13** 对应组 |
| 17 | `250[21,75,...,1600024]` | 回合 ← CData **type18 row75** |
| 21 | `250[21,4,...,1600020]` | **我方人数** ← CData **type18 row4** |
| 22-31 | `179` 循环 + `300[..CE176..]` | 循环 slot 0..我方人数：调用 CE176 取 `type10[slot].col3`，累加进 1600023(死数) |
| **32** | `111[1,1600023,1600020,2]` | `死我方 == 我方人数` → pc35 判定=-1(败) → pc36 BreakEvent |
| 42 | `250[21,69,...,1600020]` | **敌方人数** ← CData **type18 row69** |
| 43 | `1600021=9` | 敌方 slot 从索引 10 开始 |
| 44-52 | 循环 | slot 10..10+敌方人数：CE176 取 `type10[slot].col3` 累加死数 |
| **53** | `111[1,1600023,1600020,2]` | `死敌方 == 敌方人数` → pc57 判定=1(胜) → pc58 BreakEvent |
| 65-107 | 111/121 | 组表特殊胜败条件（本夹具置 0，不触发） |
| 92 | `111[1,1600024,1600035,1]` | 回合 >= 胜利A 等特殊条件 |

`CE176 id=176 "X[戦]基本状態取得"`：pc16 `250[10,<slot>,3,...,1600010]` 返回 `type10[slot].col3 【戦闘不能?】`（0 存活 / 1 阵亡），pc29 累加进 CE172 的 1600023。⇒ **死数 = Σ type10[slot].col3**。[col3 语义=推断；cell 取值与累加路径=实测]

## 2. 关键实测：CE190 会重算敌方人数并标活

| 夹具 | 起始 type18 r4/r69 | 跑 CE188 后 r4/r69 | type10 col3 (我/敌) | CE172 参考判定 |
|---|---|---|---|---|
| `seed_battle_databases` | 1 / 1 | 1 / **7** | 0 / 0 | **0 不中断** |
| `seed_party`(SaveData01 + 逐帧) | 1 / 未设(0) | 1 / **7** | 0 / 0 | **0 不中断** |

两种夹具 CE190 后 `r69` 都变成 **7**（由组表阵容决定，与种子无关），且双存活。⇒ **CE172 永不中断**。
命令：
```
cargo test -q -p wolf-game --test m10_battle_db_fixture -- --nocapture
```
输出：`seeded-battle: enemy_count=7 reached={48,63,65,70,98,143,144,147,172,188,189,190,191,241,271}`（无 CE199/CE149/CE201 中断链）。

## 3. 新增验收测试（crates/wolf-game/tests/m10_battle_db_fixture.rs，3 测全绿）

`seed_full_battle_db(g)` 显式填全套（满足任务要求）：
- **type18**：r4=1(我方) / r69=1(敌方) / r72=0(组) / r75=3(回合)
- **type13**：组 0 的 col7/8/9/10 胜败条件 = 0（不触发特殊胜负）
- **type10**：slot 0(我)/10(敌) 的 col2 存在=1、col3 阵亡=0（存活）、HP 列
- **type0 row12**：主角指挥槽 47..54=[3,4,6,7,0,0,0,0] + 数值列 (20,21,22,23,24,25,26,27,28,9,10,4)

`reference_ce172_verdict(g)`：用**与 CE172 相同的 DB cell**（type18 r4/r69 + type10 col3）复算 pc32/pc53：返回 -1 败 / +1 胜 / 0 未决。

| 测试 | 断言 | 意义 |
|---|---|---|
| `full_fixture_keeps_ce172_from_aborting_and_the_line_alive` | 参考判定==0；literal 判定==0；CE188 240 帧后到达 CE152/CE192，无 CE199/CE149/CE201，warnings==0 | 完整夹具下战斗线**存活到指令选择**；若未来补丁错误地让 CE172 中断，此测立刻红（中断链出现/CE192 丢失） |
| `empty_enemy_roster_would_abort_under_reference_model` | r69=0 ⇒ 参考判定==1（胜中断/PC53）；完整 r69=1 ⇒ 0；我方全灭 ⇒ -1（败中断/PC32） | **证明 0==0 机制**：旧 `seed_party` 若 r69 保持 0 会被参考语义瞬间判胜中断——这正是问题根源，不是引擎 bug |
| `seeded_battle_db_stalls_downstream_not_at_ce172` | CE190 后 enemy_count>0；到达 CE172 但无中断链；到达 CE65 后停 | **钉死 CE172 不中断**，纠正把 unclear 归因 CE172 |

## 4. 7 个 unclear 的 first-divergence 定位（结论：全部继续为 c，均非 CE172）

CE188 战斗线实测可达事件（seed_battle_databases 800f）：`188→189→190→143/144/147→172→191→{65,241,271,63,70,98,48}`；用 SaveData01+OK 脉冲则 `→152→155→156→157→192→{337,506}`。**均含 CE172 且无 CE199/CE149/CE201。**
⇒ 7 个 unclear 的分歧**不在 CE172**，而在下列**在路径上**的 magic111 站点（`value>=1e6`，即补丁会改变比较处；实测扫描 `work/_ce_full.json`）：

| 事件 | pc | raw 111 | 说明 |
|---|---|---|---|
| CE147 pc37 | 37 | `111[2,1600001,-1,2, 1600001,1600026,5]` | 装備補正 阶段，最早可达；self1600001 != self1600026 |
| CE65 pc160 | 160 | `111[1,1600011,1600024,1]` | 万能ウィンドウ選択実行：记忆 vs 回合 |
| CE152 pc82 | 82 | `111[1,1600017,1600015,0]` | コマンド一覧算出 |
| CE161 pc31 | 31 | `111[1,1600000,1600010,2]` | 味方対象選択 |
| CE160 | 多处 | pc87/93/102/108/115/121/166/172/181/187/194/200/218/277/287/308/327/365 | AI 実行（大量 magic） |
| CE163 | 多处 | pc215/286/333 | 行動対象算出 |
| CE64 | 多处 | pc407/416/521/584/599/655/713/730/795/825 | 万能ウィンドウ描画 |

**归类（哪个 unclear→a/b）**：
- 3 `m7_battle_full:368`、4 `m7_battle_menu_text:183`、6 `m7_battle_seed:144`、10 `m8_battle_command_input:163`、11 `m8_battle_party:183`：走 CE188 + `seed_battle_databases`（r69 被 CE190 重算为 7、双存活）⇒ **CE172 不中断**，分歧在 CE65/CE152/CE64 一线 ⇒ **仍为 (c)，非 (b)-CE172**。
- 7 `m7_battle_target_scheduler:363`（phase==2 left0 right2）、9 `m7_day4_playable:570`（v350_seq / CE241）：不在 CE188 直接链（经 CE253→CE239 状态机），与 CE172 无关 ⇒ **(c)**。
- **无任何 unclear 迁移到 (b)-CE172**。真正的 (b) 仍需在 CE147/CE65/CE152/CE161/CE160/CE163 这些下游 magic111 站点逐点插桩跑补丁确认（本任务未落补丁，故标记"需补丁插桩确认"）。这是对 `M10_OPCODE_111_PLAN.md` 第 0 节 "1/2/5 = CE172 中断" 分类的**实测修正**。

### 对 #1/#2/#5 的修正
`m6_battle_commands:144 / m6_battle_real:158 / m7_battle_round:255` 也**不是** CE172 中断：SaveData01 + 逐帧 seed_party + OK 脉冲实测 `r69=7、r0c3=0、到达 CE152/CE192、无中断链`。它们与上述同属下游 magic111 分歧。

## 5. 回归

```
cargo test -q -p wolf-game --test m10_battle_db_fixture                 # 3 passed
cargo test -q -p wolf-game --test m7_battle_target_scheduler --test m8_battle_party  # 6+2 passed
```
均绿；warnings 0。

## 6. 对 Phase 1 落地的含义

1. **不要**把 Phase 1 首要目标放在 CE172（它已由真实阵容保护）；应在 **CE147/CE65/CE152/CE161/CE160/CE163** 的 magic111 站点上逐点插桩确认 first divergence，再把对应测试期望重导。
2. 任何声称"修 op111 只需处理 CE172"的路线都不成立；必须先在**战斗 DB 数据前置**上把 r69/type10 alive 显式补齐（本夹具即模板），否则退化夹具会造出假中断。
3. 树绿色保持不变；补丁仍 PARK 在 `work/out/opcode111_landed.patch`。

## 7. 工作树备注

`git status` 显示其它队友未提交批次（`battle_wiring.rs / game.rs / map_runtime.rs / picture.rs / rng.rs / decode.rs / docs/UNVERIFIED.md / M7_OPCODE_111.md` 等）——非本任务改动，未触碰。本任务新增仅：本文件 + `crates/wolf-game/tests/m10_battle_db_fixture.rs`。无 commit/push。
