# M10 opcode 111 参考语义落地方案

Status: 规划文档（DOCUMENT-ONLY，未改任何生产代码/测试）。HEAD `bcbcfba`（全绿）。补丁实体：
`work/out/opcode111_landed.patch`（`crates/wolf-vm/src/exec.rs:2228` 的 eval_num 改动 + `opcode_111_semantics.rs` 重写）。
相关：`docs/research/M7_OPCODE_111.md`、`docs/research/M9_OPCODE_111_REGRESSION.md`（含 15 行失败表与 CE172 机制）。

## 0. 结论摘要

- 补丁**语义正确**，但把 op111 三元组的“值操作数”从**字面量**改为 `eval_num` 后，全库**约 328 个 op111 magic 站点**（value >= 1,000,000，即变量/自身变量引用）由“几乎恒假/恒真”变成真实比较，导致 `cargo test -q --workspace` **15 个 wolf-game 集成测试变红**（无补丁 93 ok / 0 failed，有补丁 76 ok / 15 failed）。已按 Lead 指示 PARK，树保持绿。
- 15 测分类（[实测]=直接证据，[推断]=机制成立但未逐测跑全轨迹）：
  - **stale-expectation（a）：2 个** —— 8 `m7_ce244_direct:137`、12 `m6_mainline:427`。
  - **real-defect / 行为真实变化（b）：6 个** —— 1、2、5（`seed_party` 不写 row69 ⇒ CE172 立刻判“胜”中断）、13、14（行动力 clamp 站点复活）、15（CE323 按真实 var102 选阶段）。
  - **unclear（c）：7 个** —— 3、4、6、7、9、10、11（均走 `seed_battle_databases` 深路径；候选 magic111 站点已定位，first-divergence 需实测收敛）。
- 落地策略：**不能只落代码**。必须“战斗 DB 数据前置 + 期望值重导 + 补丁”在同一次 PR 内**原子落地**，否则树必红；CE159/CE162 执行侧分发、CE244 结算深链是与 111 语义**正交**的结构缺口，不随本补丁变绿/变红，单列。

## 1. 补丁改了什么 + 站点规模

- 改动：`crates/wolf-vm/src/exec.rs` opcode 111 分支中
  `let value = arg(cmd, 2 + 3 * i);`（字面量）→ `let value = eval_num(arg(cmd, 2 + 3 * i), inst, vars);`（走 eval_num，magic 操作数解析为真实变量值）。lhs 本就走 eval_num。
- 影响面 [实测]：扫描 `work/_ce_full.json` 全部 550 个公共事件，op111 三元组中 value 绝对值 >= 1,000,000 的站点共 **328** 个（`M7_OPCODE_111.md` 记 356 为更宽口径，含非 111 指令，二者不矛盾）。这 328 处是比较语义真正改变的地方。
- 语义判据：op111 `ints=[condCount(n=a[0]&0xF), 然后 n 组 [var, value, operator]]`；operator 1=>=, 2===, 3=<=(<=), 5=!=(见 docs/OPCODE_SPEC.md)。当 value 是 magic（>=1e6）时，字面量比较几乎不可能命中，参考语义才还原作者意图。

## 2. 状态源反推（数据实测，全部 [实测] 除非标注）

### 2.1 战斗胜负判定 CE172 —— 计数/胜负来源（15 测的核心开关）

- op250 的 arg0 是**标签索引**，`arg0=20/21`（名“基本システム用変数”）**解析到 CData type18**（`docs/research/M7_BATTLE_TARGET_SCHEDULER.md:24-25`；`crates/wolf-game/src/db.rs:650 resolve_labels`）。因此 `battle_seed.rs` 播 `CData type18` 是对的——**不是**表号笔误。
- CE172 `X[戦]敵/味方·勝敗判定`（id=172）关键指令：
  - pc12 `250 [21,72,0,69632,1600030]` → 敌组 ID（type18 r72）→ 用于 pc13-16 读 `type13` 敵グループ 勝利/敗北条件（列 7/8/9/10）。
  - pc17 `250 [21,75,...]` → 现在回合数（type18 r75，CE198 写 1 / CE189 pc14 清 0）。
  - pc21 `250 [21,4,...1600020]` → **我方人数**；pc22-29 逐槽调 `X[戦]基本状態取得`（CE176），累计“戦闘不能”数到 1600023。
  - pc32 `111 [1,1600023,1600020,2]`：若“战闘不能数 == 我方人数” → pc35 `1600099=-1`（败）+ pc36 `BreakEvent`。
  - pc42 `250 [21,69,...1600020]` → **敌方人数**；pc43-50 同上逐槽统计；pc53 `111 [1,1600023,1600020,2]`：若“战闘不能数 == 敌人数” → pc57 `1600099=1`（胜）+ pc58 `BreakEvent`。
  - pc65-123 还有 4 处 magic111（pc92/107 等）继续设 1600099 胜负。
- **机制**：字面量下 pc32/pc53 的 `==` 恒假（小计数 == 字面 1600020）→ **永不结算** → 调用方 CE188 继续 → 这正**是现绿路径依赖的 bug**。参考语义下这两处 `==` 复活：若某侧人数=0，则 `0==0` → 立刻判该侧全灭。CE188 调用点：pc45/66/73/120/164（op300 名 `X[戦]敵/味方·勝敗判定`, param 1600020）；pc46 `111[1,1600020,0,5]` 见非 0 返回即分支走向战斗结束（pc171 主人公DB復元…），**到不了 CE192**。
- 人数/计数的写入锚点 [实测]：`CData type18 row4`=我方人数（CE174 pc23 读，列名`現在パーティー人数`）、`row69`=敌最大数（CE179 pc58 读，列名`[戦闘]敵の最大数(システム)`；CE48 pc102 写 7）、`row72`=敌组 ID、`row75`=现在回合数。CE190 戦闘キャラ配置 pc12/36/37 读 r4/r69/r72；pc17 调 CE143 主人公DB→戦闘DB（行号=1600001=队长角色ID=12，与 `battle_seed` 播 `type0 row12` 一致），pc68 调 CE144 敌DB→戦闘DB。
- **测试 1/2/5 机制确证 [实测链]**：`m6_battle_commands.rs:58 seed_party` 只写 `type18 row4=1`，**不写 row69**；`m6_battle_real.rs:37 seed_party`、`m7_battle_round.rs:53 seed_party` 同。→ 敌人数 0 → CE172 pc53 `0==0` → 判“胜”中断 → 断言 `reached_ce192 / reached_ce152` 失败。修 **数据**即可（补 row69），非改语义。

### 2.2 行动力 var14 / 上限 var13（测试 13、14 的机制）

- var14=行动力（`2000014`），var13=上限（`2000013`）。写者仅 CE216 `ステータスバー表示` 与 CE226 `■行動力変動`。
- CE216 pc13 `111 [2,2000014,2000013,1, 2000014,0,3]` = `if var14>=var13 clamp; elif var14<=0 →0`。字面量下 value=2000013 常量 → `var14>=2000013` **恒假**（不 clamp）；参考语义下 true 分支活 → var14 封顶到 var13。
- CE226 pc15 `111 [18,1600012,0,3, 1600012,1600010,1]` = `if delta<=0 or delta>=var13`。字面量下 `1600010` 常量恒假；参考语义活 → 影响 var14 归零/回满。
- var13 的初值/来源：CE252 pc383 `121[2000013,1600010,0,256]` 写 var13=self1600010；日初 refill 经 CE226(+arg) / CE216。→ 测试 13 期望 `before_energy==100` 实得 60、测试 14 期望 `115` 实得 35，是这两个 magic111 站点由“死”变“活”后 clamp/refill 行为改变所致（**b 行为真实变化**，期望值须重导）。

### 2.3 H カットイン var218（测试 15 的机制）

- CE372 `カットイン呼び出しトイレ` 自身**只按 arg0/arg1 写 var218 字面**（pc14=1,pc22=2,pc28=6,pc42=3,…,pc149=7），**无 magic111**。真正的阶段选择在上游 **CE323 `■トイレ`**（43 处 op210 调 CE372，pc120/232/240/337/522/…/2388，传 arg0∈{2,3}, arg1∈{0..14,-1}）。
- CE323 magic111 两处：pc107、pc138 `111 [17,1600010,2000102,3]` = `if self1600010 <= var102`。字面量下 `<= 2000102` **恒真** → 固定走某阶段;参考语义下按真实 var102 → 阶段/arg 变 → var218 变（测试 15 期望 `var218==2` 实得 0）。
- CE371 `トイレタイマー1` 另有 7 处 `111 [1,1600020,1600000,2]`（==self1600000）同样由死变活，轮询 var218（pc137/221/306/391/477/568/659）。

## 3. 15 个失败测试分类表

| # | 文件:行 | 断言 | 观测 | 机制 | 类别 | 修复 |
|---|---------|------|------|------|------|------|
| 1 | `m6_battle_commands.rs:144` | `reached_ce192==true` | false | `seed_party` 不写 row69 ⇒ CE172 pc53 `0==0` 判“胜”中断（§2.1）[实测链] | **b** | 补 `type18 row69>=1`；断言保持 reach CE192 |
| 2 | `m6_battle_real.rs:158` | `reached_ce192` | ce143=true,ce192=false | 同 1（`seed_party:37` 仅 row4）[实测链] | **b** | 同 1 |
| 3 | `m7_battle_full.rs:368` | `reached_ce152` | ce143=true,copyHP=100/100,ce152=false | 走 `seed_battle_databases`(row69=1)，深路径另有 magic111 分歧：候选 `CE152 pc82`、`CE154 pc28/70`、`CE65 pc160` | **c** | 需实测 first-divergence 后重导 |
| 4 | `m7_battle_menu_text.rs:183` | `reached_ce192` | false | 同上深路径；候选 `CE152 pc82`/`CE65 pc160` | **c** | 需实测 |
| 5 | `m7_battle_round.rs:255` | `reached_ce152` | false | `seed_party:53` 仅 row4 ⇒ CE172 中断；另有 start_event(239) 场景 | **b** | 补 row69 后重测 |
| 6 | `m7_battle_seed.rs:144` | `reached_ce192` | false | `seed_battle_databases` 深路径；候选同一批 | **c** | 需实测 |
| 7 | `m7_battle_target_scheduler.rs:363` | `var(350)==2` | left=0 | 2000350 状态机在深路径前被打断；候选 `CE160 AI pc87/93/…`、`CE65 pc160` | **c** | 需实测 |
| 8 | `m7_ce244_direct.rs:137` | `var349=11→outcome1; var349=1600010→-1` | 第一例 outcome=-1 | 该测**显式 pin 字面 bug**（`2000349==self1600010` 字面永不匹配→走 win=1）；参考语义下匹配→分支翻转 | **a** | 重写期望为 CE244 真实 2000349 分支结果 |
| 9 | `m7_day4_playable.rs:570` | `settled` | v350_seq=[0,1,2],ce241=false,v352_peak=0,ce239_alive=true | 真实 day4→CE253→CE239→CE242 复放；参考语义下某 magic111 改变回合推进 | **c** | 需实测（与结构缺口 CE159/CE162 区分） |
| 10 | `m8_battle_command_input.rs:163` | `saw_ce65` | menu_seen=false,ce65=false | 深路径；候选 `CE65 pc160`/`CE152`/`CE154` | **c** | 需实测 |
| 11 | `m8_battle_party.rs:183` | `r.v343==1` | left=0 | 深路径（战斗未决）；候选同批 | **c** | 需实测 |
| 12 | `m6_mainline.rs:427` | `instructions>10_000` | 9792（-2%） | 功能断言(416-426: day4 08:30/var4=8/var5=30/var160=4/var6=2/画面>20图/warnings=0)**全过**，仅阈值脆弱 | **a** | 阈值放宽到 ~9_500 |
| 13 | `m6_training_menu.rs:162` | `before_energy(var14)==100` | 60 | §2.2 CE216 pc13 / CE226 pc15 clamp 复活 | **b** | 重导 var14 真实值 |
| 14 | `m7_hmode_state.rs:241` | `fresh day-2 var14==115` | 35 | 同 13 | **b** | 重导 |
| 15 | `m8_hscene_forward.rs:890` | `CE372 选 var218==2` | 0 | §2.3 CE323 pc107/138 按真实 var102 选阶段 | **b** | 重导 CE323 阶段期望 |

计数：**a=2（8,12）｜b=6（1,2,5,13,14,15）｜c=7（3,4,6,7,9,10,11）**。

## 4. 安全落地实现顺序（保持树绿）

**Phase 0 —— 数据/夹具先行（零风险，可先于补丁做）**
- 新增 test-only 夹具（如 `crates/wolf-game/tests/m10_battle_db_fixture.rs` 内的辅助函数，不改生产）：完整填战斗 DB 前置——`CData type18 row4=1,row69=1,row72=<组ID>`，`type13` 敌组勝利/敗北条件，`type10` 参战者 HP/alive，`type0 row12` 队长与命令槽、`type7` 物品。
- 断言 CE172 在**开战**返回中性 `1600020==0`（不误判）；并对照“仅 `seed_party`(row69=0)”变体断言会立刻判“胜”。此步即可把 **b(1,2,5)** 与 **c(深路径)** 分流，且不改生产代码、不落补丁。

**Phase 1 —— 低风险期望修正（可与补丁同 PR，但不能单独改变红绿）**
- 测试 12 阈值 `10_000 → ~9_500`。

**Phase 2 —— 补丁 + 语义重导，原子落地（同一 PR）**
1. 落 `exec.rs:2228` eval_num 补丁；
2. 按 Phase 0 结果更新战斗簇期望：1/2/5 = “补全 row69 后仍达 CE192/CE152”；3/4/6/10/11/9 = 按实测 first-divergence 更新深里程碑；
3. 重写测试 8 期望（从 CE244 真实 2000349 分支重导，不再 pin 字面 bug）；
4. 重导 13/14（var14 clamp 后真实值）、15（CE323 按 var102 的真实阶段）；
5. `cargo test -q -p wolf-vm` 与 `cargo test -q --workspace --no-fail-fast` 均须 **0 failed**（以 `M9_OPCODE_111_REGRESSION.md` 15 行表为回归基线）。

**Phase 3 —— 保持 blocked / 不承诺（与 111 正交）**
- CE159/CE162 执行侧分发、CE244 结算深链：结构缺口，不因本补丁变绿也不应变红；只在补丁 PR 之外单列跟踪。`m7_battle_target_flow.rs` 仍为 zz 调试探针，非验收。

**备选（更小爆炸半径）**：先实现“按站点白名单”的 eval_num（仅 CE172/CE216/CE226 等已证实的 magic 站点），验证后再全量；但终态仍应是全局语义正确。

## 5. 推荐第一步（单一、可度量）

写 **`crates/wolf-game/tests/m10_battle_db_fixture.rs`（test-only，不改生产代码）**：
- 填完整战斗 DB 前置后 `start_event(188)`，断言 (a) 开战首帧 CE172 令 `1600020==0`（不误判）；(b) 对照仅 `seed_party`(row69=0) 时 `1600020!=0`（误判“胜”）；(c) 对测试 3/4/6/10/11/9 场景逐一记录首个 magic111 分歧 pc。
- **验收**：该测试在“补丁 + 完整前置”下 PASS，并能逐场景列出 first-divergence，从而把 7 个 unclear 收敛为 a/b；此步不落地生产补丁、不改变现有红绿。

## 6. 证据文件清单

- `docs/research/M7_OPCODE_111.md` —— 111 三元组值操作数问题与 356 站点统计。
- `docs/research/M9_OPCODE_111_REGRESSION.md` —— 15 行失败表 + CE172 机制。
- `docs/research/M7_BATTLE_TARGET_SCHEDULER.md:24-25` —— op250 arg0=20/21 → CData type18 映射。
- `crates/wolf-game/src/battle_seed.rs` —— 播种单元（type18 row4/69、type0 row12、type7/8）。
- `crates/wolf-game/src/db.rs:650` —— resolve_labels。
- 事件数据：`work/_ce_full.json`（CE172/176/188/190/143/144/216/226/252/323/371/372），扫描脚本 `work/_m10h..p.py`。
- 失败测试：见 §3 表内 file:line。

## 7. Phase 0 实测结果（task-66，修正第 0 节与第 5 节）

详见 `docs/research/M10_BATTLE_DB_FIXTURE.md`；新增 `crates/wolf-game/tests/m10_battle_db_fixture.rs`（3 测绿）。

**实测修正（重要）**：第 0 节把 1/2/5 归为 "b：seed_party 不写 row69 ⇒ CE172 立刻判胜中断" 是**错误**的。实测 CE190「戦闘キャラ配置」在每次真实战斗都会把 `type18 row69` 由组表阵容重算（实测 1→7）并把落位单位标活（`type10 col3=0`）；因此真实/常规夹具下 CE172 的 `死数(0) != 总数(1/7)`，**永不中断**，#1/#2/#5 也不是 CE172 中断。空阵容 `0==0` 立即判胜只发生在**合成退化夹具**（r69 被强制 0 且不经 CE190 重算）。

- 第 5 节 (b) 的 "对照仅 seed_party(row69=0) 时 1600020!=0" **不可复现**：CE190 会把 row69 从 0 重算为真实阵容（7）。正确对照是**直接对 DB cell 建模**：`r69=0 ⇒ 死敌0==敌0 ⇒ PC53 胜中断`（fixture 强制 r69=0、不经 CE190 即可示出）。
- 第 5 节 (c)：**7 个 unclear 无一迁移到 (b)-CE172**；CE172 被排除。分歧在 CE188 下游 **CE147/CE65/CE152/CE161/CE160/CE163/CE64** 的 magic111 站点（已在 fixture 文档第 4 节逐 pc 列出，需落补丁逐点插桩确认 first divergence）。
- 完整战斗 DB 夹具模板 = `seed_full_battle_db`（type18 r4/r69/r72/r75 + type13 组条件 + type10 HP/alive + type0 row12）；完整夹具下 CE188 线到达 CE152/CE192、无 CE199/CE149/CE201 中断链、warnings 0。
- 回归：`m10_battle_db_fixture`(3) + `m7_battle_target_scheduler`(6) + `m8_battle_party`(2) 全绿；树保持绿；补丁仍 PARK。

**Phase 1 落地顺序据此调整为**：先在 CE147/CE65/CE152/CE161/CE160/CE163 上逐点确认 first divergence → 数据前置（补齐 row69/type10 alive，本夹具为模板）→ 再原子落 op111 补丁 + 重导期望。CE172 不再是首要目标。
