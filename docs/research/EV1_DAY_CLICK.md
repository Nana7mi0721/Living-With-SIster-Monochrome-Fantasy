# EV1 昼之妹クリック判定 — page0 的激活条件、命中框与 headless 可行性

**作者**: m6-ev1b（task-9）
**仓库**: `E:/Study/github program/imoto life`，数据根 `work/all`
**结论一句话**: ev1（`SampleMapA.mps` 事件 1 `妹クリック判定（夜`）**page0 可以在普通昼/傍晚状态下自然进入**，它的命中框就是**真实的妹妹精灵**（`Picture/imouto02.png`，左上锚点，位于 `(240,340)`，贴图 `149x212`），**不需要任何重绘、也不需要注入输出变量**。对 `(240,340)` hover 约 8 帧再按一次 Decide，page0 自己写出 `var68=1`（全流程 warnings=0）；`var68` 随后被 ev4 page0 消费，调用 CE245（夜コマンドメニュー）。

与 page2 的关键区别：page2 在深夜 `var64==1` 时由 **ev2 page1 把 picture 120 重绘成 120x80 `<SQUARE>`、中心 `(140,340)`**；page0 没有这次重绘，因为 ev2 的两页条件（`var65==1` / `var64==1 && var167==0`）在 page0 状态下都不成立，picture 120 保持为**真实精灵**。

---

## 1. 事件与页面事实（`work/_map_audit.json` / `work/maps_full.json`）

`SampleMapA.mps` 事件 1，x63 y0，3 页，**全部 trigger=2 = parallel**。页选择规则：**取条件全部成立的最高页号**。

| page | 触发 | 条件（全部需为真） | 指令数 | 页内写输出 |
|---|---|---|---|---|
| 0 | parallel | `v69==1, v25==0, v26==0, v302==0` | 113 | idx11 `op121 [2000068,1,0,0]` → **var68=1** |
| 1 | parallel | `v65==1, v25==0, v26==0, v340==0` | 17 | idx124 → var67=1 |
| 2 | parallel | `v64==1, v25==0, v26==0, v302==0` | 17 | idx141 → var63=1 |

在 day2 19:30 卧室实测：`v69=1, v64=0, v65=0, v340=0, v25=v26=v302=0, v167=0, v62=0, v68=0, v63=0, v295=1`，warnings=0。于是 page0 条件成立、page1/page2 条件不成立 → **活动页 = 0**。

> `var69` 的唯一写入者：`ニューゲーム処理`（ev0）`idx841 op121 [2000069,1,0,0]`，设为 1；全表未发现清除点，故整局保持 1。它是「新游戏已开始」的环境标志，不是某条分支的输出。

---

## 2. page0 的命中框：真实精灵，而非重绘

page0 的点击检测是 `op124 [1100000,0x4000,120,8]`（picture 120 的 field 8 = hover），它测的是 **picture 120 当前的画面框**。在 day2 19:30（`var64=0, var65=0`）实测 picture 120：

| 字段 | 值 |
|---|---|
| file | `Picture/imouto02.png`（妹妹精灵） |
| kind | sprite（`kind as u8 == 0`） |
| anchor | `0`（左上锚点，**不是** ANCHOR_CENTRE） |
| scale | `100` |
| opacity | `255` |
| 原点 `(x,y)` | `(240,340)` |
| 贴图尺寸 | `149 x 212`（PNG IHDR 实测） |
| **hover AABB** | **`[240, 389) x [340, 552)`** |

为什么不重绘：ev2 `妹クリック判定` 只有两页，`page0` 条件 `var65==1`（动作：把 picture 120 画成 `800x500 <SQUARE>` @`(512,300)` 或擦除），`page1` 条件 `var64==1 && var167==0`（动作：画成 `120x80 <SQUARE>` @`(140,340)`）。page0 状态下 `var65==0、var64==0`，两者都不触发，所以 picture 120 就是那张真实精灵，`op124` 直接测精灵边界。**这就是 page0 与 page2 在「命中框」上的本质差异。**

---

## 3. 输入链（`work/maps_full.json` SampleMapA.mps ev1 page0）

| 页内 idx | opcode | 参数 | 作用 |
|---|---|---|---|
| 5 | 124 | `[self100(1100000),0x4000,120,8]` | picture 120 field8 = hover → self var0(slot100) |
| 6 | 111 | `[1, self100, 1, 2]` | if self100 == 1 |
| 8 | 123 | `[self101(1100001),16]` | 轮询按键（mask 16）→ self var1 |
| 9 | 111 | `[1, self101, 10, 2]` | if 轮询 == 10（CODE_OK / Decide） |
| 11 | 121 | `[2000068,1,0,0]` | **写 var68 = 1** |

`op111` 判定语义：`compare(lhs,rhs,op)`，`op==2` 为 `==`；`op123` 经 `host.key_code_auto` → `key::code(key_bits)`，Decide 位映射到 `CODE_OK=10`（`crates/wolf-game/src/key.rs`）。故 `game.key_bits = wolf_game::key::OK` 时轮询帧恰好返回 10。

---

## 4. headless 复现（真实 Game、确定性、约 1s）

进 day2 19:30 的路径与 `m6_night_menu.rs` / `m6_mainline.rs` 相同：新游戏 → day1 夜（今天还是睡吧）→ day2 上午 → 一次 学习会 → 晚餐 → 19:30 卧室。**唯一 seed 是 day1 把 `var63=1`（仅用于驱动标准 day-loop 的睡眠分支进入 day2，是 page2 的标志、不是 page0 的输出，且 19:30 时已回到 0）。`var68` 从不注入。**

步骤与实测结果：

1. 断言基线：`v160=2, v4=19, v5=30, warnings=0`；`v69=1, v64=0, v65=0, v25=v26=v302=0`；**`v68==0`（输出未注入）**；picture120 = imouto02 精灵 `(240,340)` anchor0。
2. 文档化归一化：`v295=1`（首日晚餐 TIPS 未关，ev4 page0 pc1 会硬 break）→ 清 `v295=0`，并 `v68=0`。
3. 自然手势：`mouse=[240,340]` 悬停 **8 帧**，再一帧 `key_bits=OK`。
4. **实测：`v68==1`（page0 idx11 真写入），`v63==0` 未被动，`warnings==0`。**
5. 下游：ev4 page0（autorun，条件 `var68==1 && var69==1 && var62==0`）到 pc335 `op210 [500245]` → **CE245 夜コマンドメニュー**；跑 40 帧后 event 245 协程存活、菜单贴图 `[1200..1207, 1212, 1213]` 全部画出，warnings=0。

---

## 5. 与 page2 对照

| | page0（本文件） | page2（`EV1_NIGHT_SISTER.md` / `m6_ev1_page2.rs`） |
|---|---|---|
| 进入条件 | `var69==1`（+v25=v26=v302=0） | `var64==1`（+v25=v26=v302=0） |
| 命中框 | 真实精灵 `imouto02.png`，左上 `(240,340)`，`149x212`，AABB `[240,389)x[340,552)` | ev2 page1 重绘 `120x80 <SQUARE>`，ANCHOR_CENTRE `(140,340)` |
| 输出 | `var68=1`（page0 idx11） | `var63=1`（page2 idx141） |
| headless 手势 | hover `(240,340)` 8f + Decide | hover `(140,340)` 8f + Decide |
| 下游 | ev4 page0 → CE245 夜コマンドメニュー | ev4 page4 → 深夜睡眠三选一 |

---

## 6. 测试与命令

- 文件：`crates/wolf-game/tests/m6_ev1_page0.rs`
- 测试：`daytime_sister_sprite_click_writes_var68_and_opens_the_night_command_menu`
- 命令：`cargo test -q -p wolf-game --test m6_ev1_page0`
- 结果：**1 passed**，约 1.03–1.06s（连续 3 次确定性通过）；warnings 全程 0。

## 7. 边界与未解

- 只 seed 最小前置（仅 day1 `var63=1` 用于 day-loop）；page0 输出 `var68` 从不注入。
- 未修改任何既有源文件/测试/`docs/BASELINE.json`/`docs/HANDOFF_M4.md`；未提交。
- page0 大页体还测 picture 121/122 等昼间图标，不在本任务范围，仅交叉验证不影响结论。
