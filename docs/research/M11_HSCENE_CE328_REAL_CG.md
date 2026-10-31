# M11 P1（task-96）— 真实 CE328 → CE402/CE410 深度路径实证

**结论：可以。** 从零售存档 + 一个历法一致的合成 fixture 出发，**只** `start_event(328)`（深夜例程），
让 CE328 自己执行控制流，即可经 `pc986 op210[500402]` 进入 CE402；CE402 从其 512 槽程序的
**pc0 跑到 pc511（末槽）**，其间 10 处 `op210[500410]` 调用把 CE410（騎乗位系カットイン）
拉起，CE410 从 **pc0 跑到 pc247（末槽）**。全程 **warnings=0**。**没有直接 `start_event(402)`。**

- 新增测试：`crates/wolf-game/tests/m11_hscene_ce328_real_cg.rs`（2 个测试，均绿，~0.60s）
  - `real_ce328_reaches_ce402_and_drives_ce410_to_completion`
  - `ce402_var23_gate_aborts_at_pc54_when_below_six`
- 复现命令：
  ```bash
  cd "E:/Study/github program/imoto life"
  WOLF_DATA_ROOT="E:/Study/github program/imoto life/work/all" \
    cargo test -q -p wolf-game --test m11_hscene_ce328_real_cg -- --nocapture
  ```
- 数据源：`work/all/BasicData/CommonEvent.dat`（经 `work/_ce_full.json` 索引；pc 即该事件 cmds 下标）。
- 存档：`work/retail_backup/Save/SaveData01.sav`。HEAD 5ef6b1f。

---

## 一、运行姿态（为什么用 budget=1）

测试装载 SaveData01 → 写入 fixture → 把 VM 换成 `Vm::with_budget(programs, 1)`（保留同一
`programs` 与 `vars`）→ `g.start_event(328)`，然后逐帧 `g.frame()`，每次 choice 立即
`ChoiceReply::Index(0)`。

- budget=1 只改变**帧边界落点**（每帧退休 1 条指令），不改变**执行哪条指令**；因此轨迹是
  CE328 真实分支的可重复渲染。
- 不这么做的原因：正常帧束下 CE402（512 条、含大量即时绘图/无长等待）经常在**单帧内跑完并被回收**，
  从 `instances()` 里根本观察不到（实测正常驱动时 hot-trace 只能瞥到 CE402 的 pc8，CE410 完全看不到）。
- 需要强调：CE410 由 `op210` 的 `spawn_child` 启动；`spawn_child` **不会**写入
  `Vm::started_events`（只有公开 `start_event` 会，见 `crates/wolf-vm/src/exec.rs:1163`），
  所以不能用 `started_events` 判 CE410 是否被调用——本测试改为逐帧跟踪 `instances()` 中
  `event==410` 的 pc。

## 二、合成 fixture（历法一致，var 值 = 实测种子值）

| var | 值 | 作用 / 约束 |
|---|---|---|
| var304 | 0 | CE328 pc694 `var304>=1` 走 else（不进 Hしたい 分支） |
| var6 | 4 | 星期四（深夜日期分支） |
| var4 | 21 | 21:00；到 CE402 时已被 CE217 推进为 **22**，仍满足 pc4 `var4>=21` |
| var5 | 0 | 分 |
| var18 | 20 | CE328 pc746 `var18>=50` 为**假** → else（走向 CE402 所在支） |
| var494 | 0 | pc845 `var494==2` 为假，避免跳进 悪戯へ 后经 420 跳过 CE402 的 else 体 |
| var160 | 90 | pc850 条件仅 `var160==83 || var160==84`；90 使之为假 → else pc969 |
| 其余 | var16=1, var22=180, var326=2, var102=80, var103=2, var23=6, var151=0, var20=61, var265=0, var545=1 | CE402 各门（见第三节） |

## 三、CE328 真实分支轨迹（实测 pc 序列，关键检查点）

`start_event(328)` 后的实际 pc（省略前半段睡眠问答）：

```
... 679,680,693(合宿処理),694(IF var304>=1 FALSE->else),
715,716,717,718,719,720,721(213 就寝),745(通常就寝),
746(IF var18>=50 FALSE->else),845(IF var494==2 FALSE),850(IF var160 83/84 FALSE->else),
969,970,971,972,973(CE222),974,...,985,986(op210[500402]),
987,988,993..997,998..1024
```

- 轨迹中 `ce328_seq.last()==1024`（CE328 最后一条槽）。
- 到 CE402 时实测变量：`var4=22, var5=0, var6=4, var16=1, var18=20, var20=61, var22=180,
  var23=6, var102=80, var103=2, var160=90, var265=0, var304=0, var326=2, var494=0, var545=1`。
  与 fixture 一致，仅 var4 被 CE217 从 21 推进到 22。

### 关键结构修正（推翻 task-94/task-96 早前的假设）

早前文档写「var6==4 且 var4>20 → pc122 就寝(pc745)」。**编译后不是这样**：

- `pc111 111[17,2000006,4,2]` = `var6==4`；`pc113 111[17,2000004,6,1]` = `var4>=6`；
  `pc115 111[1,2000004,20,3]` = `var4<=20`。
- `pc119 499` 闭合内层 block115；`pc121 420` 接在外层 block113 上；`pc122 213 "就寝"`（→ label 就寝=pc745）
  只有 `pc113` 为假（即 **var6==4 且 var4<6**，凌晨）才会走到；`var6==4 且 var4>20` 实际在
  `pc115` 为假后经 `pc120/121/125` 落进 **pc126 家族会議/topic 体**，不是就寝。
- 本轨迹 var6=4、var4=21，先进入 topic/家族体，回答 topic 菜单（见第五节）后，经其尾部
  `679/680/693/694 → 715..721 → 745` 才到「通常就寝」。这是**真实可玩的菜单选择路径**，不是异常。

补充：`pc850 111[18,2000160,83,2,2000160,84,2]`，低 4 位=2 → 只有 `var160==83 || var160==84`；
`pc852 111[2,2000004,18,1,2000004,3,3]` 是嵌套的另一条 IF（var4>=18 || var4<=3），两者不可混读。
若 `pc850` 为真（var160∈{83,84}）会跳 label 悪戯へ(pc862)，执行 862..966 后在 `pc968 420`
跳回 block81 末尾 998，**跳过** 969..997 的 else 体——而 **CE402 调用就在 else 体的 pc986**。
因此本路径必须让 pc850 为假（var160=90）。

### 与 task-96 静态图的分歧（重要，需 Lead 知悉）

task-96 描述里写「pc122 就寝 → pc745 → pc827..861 → pc862 悪戯へ → pc863..952 → pc957.. →
pc964 var64=1 → pc966 CE331 → pc973 CE222 → pc985 op103 → pc986 op210[500402]」。实测/编译后这条链
**不是一条连续可执行路径**：

- `pc964 var64=1`、`pc966 CE331`、`pc968 420` 都在 **悪戯へ 分支（pc862 起，block81 的 case2）**里。
  执行到 `pc968 420` 时，420 是「Case 结束」标记，会把 **block81 的 case2 体**收束并跳到
  **block81 末尾 pc998**，从而**跳过 969..997 的 else 体**——而 CE402 调用 `pc986` 正在 else 体里。
  所以 `pc862→…→pc964→…→pc986` 在结构上不可能；走 862 分支只会跳过 986。
- 真正能到 `pc986` 的是 **pc850 为假 → else pc969 → 969..985 → 986** 这条。本测试正是走这条；
  因此 **该路径不经过 pc964，也不写 var64**（测试额外断言 `!ce328_seq.contains(964)` 且
  `var64_end==0`）。这也解释了为什么 var64 的写入点和 CE402 可达性不能混谈。
- task-96 建议的 gate 集 `{157=13,104=0,105=0,364==0,...}` 并非 CE402 实测门；本测试按
  CE402 raw 实测门（第四节表）给 fixture，未使用 var157/104/105/364，仍能完整跑通 CE402→CE410。

> 即：**「CE328 真实控制流可达 CE402/CE410」成立；但「必经 pc964/var64=1」不成立。**
> 本测试证明的是前者，并在断言里显式锁定后者为否，避免后人误把两条支路当成一条。

## 四、CE402 门（实测，逐条对应 raw pc）

| pc | 条件（raw） | 本测试值 | 结果 |
|---|---|---|---|
| 0 | `var16>=1` | 1 | 过 |
| 2 | `var22>=180` | 180 | 过 |
| 4 | `var4>=21` | 22 | 过 |
| 6 | `var326>=2` | 2 | 过 |
| 8 | `var102>=80` | 80 | 过 |
| 10 | `op121[1600010,0,2,24576]` = rand self10∈0..2 | — | 随机 |
| 11 | `self10<=1` | — | 否则 pc16 中止 |
| 21 | `var103>=2` | 2 | 过 |
| 52 | `111[1,2000023,5,3]` = **var23<=5 → pc54 op172 中止** | 6 | 过（必须 var23>=6） |
| 57 | `var151>=1` 中止 | 0 | 过 |
| 62 | `var20<=60` 中止 | 61 | 过（必须 >60） |
| 67 | `op121[2000545,1,0,0]` var545=1 | 1 | 执行 |
| 78 | `var265==0` | 0 | 过 |
| 80 | `103 "確定で素股"` | — | 进入 |
| 84 | `op121[1600010,1,10,24576]` rand self10∈1..10 | — | 随机 |
| 86 | `self10>=7` → label C（CG 支）/ 否则素股支 | — | 两路都汇入后续，仍会调 CE410 |

**CE402 实测序列**：`0,2,4,6,8,10,11,13,52,57,62,67,68,...,210,486..511` → **max pc 511**（程序末槽）。
第二个测试（var23=5）实测 CE402 序列 `0,2,4,6,8,10,11,13,52,54`，在 **pc54 op172 中止**，
**不产生 CE410** —— 钉死 pc52 var23 门。

## 五、每一次 choice 的回答（实测菜单）

测试对所有菜单回答 **Index(0)**，菜单出现位置（=CE328 实例当时的 pc）：

| 菜单 pc | 选项数 | 回答 | 位置/含义 |
|---|---|---|---|
| 54 | 2 | 0（是） | CE328 pc53 `102[2] "是/算了"`，问「今天要休息了么」 |
| 174 | 4 | 0（梦想的话题） | pc173 `102[20] "梦想的话题/无关紧要的话题/色情的话题/小鸟飞飞！"` |
| 580 | 3 | 0 | pc579 `102[19]` 周主题三选一（`\cself[5..7]`） |
| 662 | 2 | 0（是） | pc661 `102[2] "是/算了"`，「可以么？」 |

## 六、CE402 → CE410 调用点（全库扫描，唯一来源）

- **CE402 只有 1 个调用点**：CE328 `pc986 op210[500402,0]`（全 550 个公共事件扫描唯一命中）。
- **CE410 的 10 个调用点全部在 CE402**：pc165,173,177,195,198,288,308,311,360,447
  （args 形如 `[500410,3,k,0,1]`）。本测试实际走到 pc165/173/177/195/198 等，CE410 多条实例启动。
- CE410 实测 pc 序列首=0、**末=247**（248 槽程序跑完）。

## 七、实测汇总

| 项 | 值 |
|---|---|
| CE328 末槽 | pc1024 |
| CE402 | started=true，pc0→**511** |
| CE410 | started=true，pc0→**247** |
| warnings | **0** |
| budget=1 帧数 | 28130（~0.60s wall） |
| 菜单回答 | 4 次，均 Index(0) |

## 八、CG/图片实例（CE402/CE410 活动期间 `Screen::quads` 实测可见名，alpha>0.5）

真实出图（节选，完整 36 项）：
```
cg/bg_01_home_a.jpg
cutin/cutin_138.png, cutin/cutin_139.png, cutin/cutin_140.png
hscene/h_bg_001.jpg, hscene/h_bg_011.png, hscene/h_frame.png
hscene/h_s08_paja02.png, hscene/h_s08_pose04.png, hscene/h_s08_chinko.png,
hscene/h_s08_cami01.png, hscene/h_s08_face_mayu01/02.png, face_eye01/02.png, face_kuchi01/02.png,
hscene/h_s08_ef_ecstasy/omorasi/splash/syasei01/wet.png
hword/word_effect20/31/37/40/64/82/101.png
Picture/scene0102_bless02.png, Picture/command_cutin13.png, Picture/speaker.png ...
```
测试断言集合非空且至少含 `cutin/` 或 `cg/`。

> **未生成截图**：`wolf-cli capture --event 410` 直启 CE410 时 pictures=0（CE410 读 CE402 传入的
> self 参数，脱离父调用画不出东西），这种截图会误导。真实路径的可视性已由上面的
> `Screen::quads` 资产清单在测试内实证；若要像素图，需要把本测试的 `Game` 帧做 raster 落盘
> （现有 `wolf-cli capture` 不暴露该入口）。

## 九、数据证明 vs 推断

- **[实测-数据]**：所有 pc/var/菜单/调用点/CE402·CE410 max pc/warnings/CG 文件名，均来自
  `work/_ce_full.json` + 本次实际运行（测试可复跑）。
- **[实测-代码]**：`spawn_child` 不写 `started_events`（exec.rs:1163）；`Vm::with_budget` 行为。
- **[推断]**：`pc10/pc84` 的 rand 分支走向（self10 由零售 PRNG 决定，本测试用固定种子环境恰好走通
  到 511；两个分支都最终经过 CE410 调用点，故不阻塞断言）。真实玩家选择不同可能改变具体 CG 支，
  但「CE328→pc986→CE402→CE410」的调用链不受影响。

## 十、局限

1. budget=1 是**观测手段**，不是游戏运行模式；它保证了指令级可重复轨迹，但不模拟真实帧时序/输入节奏。
2. 本 fixture 是合成存档（非零售某一天的精确状态），只保证**历法/关系门自洽**；用于证明可达性，
   不证明某份原版存档在第几天必然进入。
3. CE402 内部 rand(self10) 分支未穷举；已证明至少一条完整路径（0→511）与 CE410 全跑。

## 十一、验证记录（环境/全量测试）

- 本测试单独运行：`WOLF_DATA_ROOT=E:/Study/github program/imoto life/work/all cargo test -q -p wolf-game --test m11_hscene_ce328_real_cg -- --test-threads=1` → **2 passed, 0 failed**（~0.96s），无 warnings。
- 全量工作区（与任务要求同形）：`cargo test -q --workspace --no-fail-fast -j 2` → **123 个 test result，786 passed, 0 failed, 1 ignored**（exit 0，`work/out/m11_task96_workspace_exact.log`）。
- 另以 `-j 4 -- --test-threads=1` 复核一次：同样 **123 result / 786 passed / 0 failed / 1 ignored**（`work/out/m11_task96_workspace_j4.log`）。
- 备注（环境级瞬时现象，非本测试断言失败）：本机首次并发跑全量时曾出现 `m11_hscene_bookstore` / `m11_hscene_ce328_branches` / 本测试 `memory allocation of 7280000/4665600 bytes failed` 并以 `0xc0000409 STATUS_STACK_BUFFER_OVERRUN` 退出、`wolf-render --lib` 两个 wgpu 测试 `wgpu error: Out of Memory`；这是大批 M11 H/render 二进制同时并发时的显存/内存压力，负载释放后同一 `-j 2` 命令即 0 failed。
