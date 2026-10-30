# M7 待办清单（Post-M6 审计）

> 目标读者：接过 M6 收尾、准备开 M7 的工程师。
> 本文只做**只读审计**：把 M6 结束后仍然挡在前进路线上的子系统和缺口，按「证据 → 下一步 → 风险 → 依赖」列成一张可执行的表，并给出**一个**建议先做的纵切及其可机检验收。
> 全文标记：**[A] 已证**（有实跑命令 / 数据 dump / 现有测试锚点）/ **[B] 数据推断**（从事件流与变量扫描推得，未端到端实跑验证）/ **[C] 攻略级 / 外部资料**（版本相关，需逐版本回归，不可当事实）。
> 测量时点：HEAD `f5aca1c`（"M6: fix night-page teardown and prove the natural sleep loop"）。数据根 `work/all`（全解 1.5 GB，含 `hscene/`、`hword/`、`DLC_data/`）。构建/回归：`cargo test -q -p wolf-game`。本节所有数字都能在 `docs/research/M6_BATTLE_REAL.md`、`docs/research/M6_SCHEDULE.md`、`docs/HSCENE.md`、`docs/TIME_SYSTEM.md`、`docs/HANDOFF_M4.md`、`docs/UNVERIFIED.md`、`docs/MAP_AUDIT.md` 里找到出处。

---

## 0. M6 收尾状态（出发基线）

| 项 | 状态 | 证据 |
|---|---|---|
| M0–M5 | 完成 | `docs/HANDOFF_M4.md` |
| 标题 → 新游戏 → Map003 ev1 → SampleMapA | 跑通、无 warning | `crates/wolf-game/tests/m6_mainline.rs`（7 例） |
| 卧室 HUD / 昼夜循环 | 跑到 **day4 08:30** | `tests/m6_night_sleep.rs`、`tests/m6_ev1_page2.rs` |
| day4 08:30 → day5 | **撞 CE239 战斗墙**（M7 头号） | `tests/m6_week_loop.rs:306` 断言 `day 5 is unreachable here` |
| 夜间整页 teardown | 已修：`map_runtime.rs:328-348`（子协程未返回时 DEFER 停页，不误杀 CE329） | `docs/research/CE328_ABORT.md` |
| 字体宽度 | 已修：`crates/wolf-game/src/fontmetrics.rs` 读 Lolita.ttf 真实 advance，`textpic.rs` `layout_with`/`measure_with` | `tests/text_metrics.rs` |
| 现有测试套件 | 全绿：`m6_mainline`(7) `m6_training_menu`(2) `m6_night_menu`(2) `m6_ev1_page2`(1) `m6_night_sleep`(1) `m6_battle_real`(1) + 匿名存档 day1→day4 综合(1) | `cargo test -q -p wolf-game` |

**一句话**：主线（标题→开场→地图→昼夜→就寝/早朝）已闭合，**唯一硬墙是「战斗」**；其余是旁支的完整度与已知否定项。

---

## 1. 工作流总览表

| # | 工作流 | 目标 | 关键证据（file:line） | 现状 | 下一步（可执行） | 风险 | 依赖 | 证明度 |
|---|---|---|---|---|---|---|---|---|
| W1 | **战斗（M7 头号）** | 走完一次真实战斗回合 → CE244 结算，解锁 day5+ | CE239 1108 slots/104 call sites；CE244 432 slots；`docs/research/M6_BATTLE_REAL.md` | 真实线可达，但**回合走不完**：CE65 空转 | 见 §2.1 三步：补命令映射 / 填武器 DB / 填敌目标列表 | 高（3 处数据+1 处语义耦合） | W3（日程把玩家送进战斗） | [A] |
| W2 | **H 场景运行期** | 从 ce246 入口可交互推进一整场 H | `docs/HSCENE.md` §1.2/§3.5；`atlas.rs:42` 4096 | 静态帧已 **89.4% 非黑 / ~39 quad / 脸三层齐** [A]；**交互推进未做** | 接 `--key-frame` 跨越 14 个 `123 [1600030,17]` 轮询；补 `run_condition==2` 自动调度验证 | 中（Move 语义未定、hword 时长未验证） | 无（独立于战斗） | [A]/[B] |
| W3 | **日程 / 任务 / 日历** | day4 之后主线可继续（非战斗路径） | `docs/TIME_SYSTEM.md` §1；`docs/research/GPT_RESEARCH_TRIAGE.md` §3/§5.3 | **无独立日历引擎**，全靠事件+变量门控 [A] | 若要「日历/任务」数据化：加 GameCalendar/QuestTemplate（当前**无**实现） | 中（多为攻略级，无权威数据） | 依赖 W1（day4 强制进战斗） | [A]（机制）/[C]（日期表） |
| W4 | **地图滚动 / 传送 / 视差** | 多屏地图滚动、传送、视差 | `docs/HANDOFF_M3.md` §5；Sys type6 行104/105；`130` 传送 | 传送已解；滚动是**引擎输出**不是脚本；视差无使用点 | **低优先级**：只在出现 >1 屏滚动地图时再做 | 低 | 无 | [A] |
| W5 | **UI / 文本 / 资产 / 音频 / 存档 完整度** | 消除剩余已知否定项 | `docs/UNVERIFIED.md`；`docs/MAP_AUDIT.md` §9 | 多为一句话可修的**视觉/确定性**项 | 见 §2.4 清单；音频需人耳复检 | 低-中 | 无 | [A]/[B] |

**依赖顺序（拓扑）**：`W1` 与 `W2` 互相独立；`W3` 的主线推进**被 W1 阻塞**（day4 白天被强制进 CE253→CE239）。`W4`、`W5` 可随时插入。推荐并行：`W1`（主干）+ `W2`（独立高价值旁支）+ `W5`（顺手清账）。

---

## 2. 逐条深挖

### 2.1 W1 战斗（M7 头号）

**真实战斗线**（`docs/research/M6_BATTLE_REAL.md`，[A]）：
`CE239 戦闘呼び出し`（1108 slot / 104 调用点，全作**唯一**真战斗入口）→ `CE240/241/242/243`（引擎 condvar 门控，`2000350` 三态机）→ … → `CE244 ┗終了判定`（432 slot，结算）。

另有独立的 `CE188 ◆戦闘処理` 线（只被 CE28/CE37 调），`CE189 初期化 → CE190 配置 → CE143 主人公DB→戦闘DB → CE144 敵DB→戦闘DB → CE191 → CE192 味方コマンド選択 → CE152 戦闘コマンド一覧算出 → CE65 万能ウィンドウ → CE157 コマンド選択 → … → CE244`。

**已验证能力**（[A]）：从 Game+零售存档可达很深；`CE143/CE144` 会在源行种子正确时把 HP/maxHP/action 抄进 CData type10（type0 列 c20 maxHP、c21 HP、c24 melee、c25 ranged、c26 def、c27 maxAct、c28 act、c47..54 コマンド1..8）。

**卡点**（[A]）：确认一次命令后（只有哨兵 `-2 何もしない` / `-1` 可选；真实攻击行 26/25 被标不可用），控制进入 `CE20 → CE157 → CE159 → CE154 → CE65`，而 `CE65` 的目标列表行数「type18 row17 == 0」⇒ `CE65` 在 `pc78 ↔ pc140` 之间**永久自旋**；`2000350/352/343` 恒 0，warning 0，`CE244` 永不到达。

**三步缺口（按可独立验收的顺序）**：

| 步 | 缺口 | 证据 | 修法 | 验收 |
|---|---|---|---|---|
| 1 | **命令可用性（已精确到 pc89..pc95）**：Data type0 只有 18 字段、没有 col20（発動アイテム要する?），CE152 pc89 读 0 后 `>=0` 恒真 → pc91 读 CData type7 所持アイテム個数 row0=0 → pc93 `<=0` → pc95 写 CData type21 選択状態=0，因此真实攻击 26/25/30/29 全不可用。**注意不是 pc66**（运行时 c6/使用可能シーン=100，pc66 不触发） | `docs/research/M6_BATTLE_COMMANDS.md`；`m6_battle_commands.rs`（control vs seeded 两 Game 因果对照） | 数据前置：CData type7 row0 col0=1 即可让 26/25/30/29 全部 `選択可能=1`；这是数据前置，不是伪造菜单行 | `CE152` 对真实攻击返回「可用」；m6_battle_commands.rs 已 PASS |
| 2 | **物品数量 DB 空**：CData type7「所持アイテム個数」row0=0（250 读返回 0） | `docs/research/M6_BATTLE_COMMANDS.md` | 从 `System.sav` / 存档模板填 type7 row0 col0=1 | 真实攻击行 `選択可能=1`；250 未应答 → 0 |
| 3 | **敌目标列表空**：type18 row17 == 0 | 同上；`CE65 pc78↔pc140` | 填 type18 敌行（CE190 配置阶段应写入） | `CE65` 不再自旋，`2000350` 能走 0→1→2 |

**关键根因**（[A]）：CData type0 コマンド槽 `47..56`（`docs/PARTY_DB.md:355`）只由 `CE26/CE77/CE78` 写，而 **`CE26 戦闘コマンド初期設定` 的调用者数 = 0**（`CE77` 唯一调用者 `CE13` 也是 0 调用者）⇒ **新游戏路径从不初始化这 8 槽**；现有值来自捆绑的 `Save/System.sav` 模板（`Game::new` 加载 `root/Save/System.sav`）。直接 `start_event(188) 4000` 帧 → choices=0、messages=0。
⇒ **两条修法二选一**：(i) 新游戏时用 `System.sav` 的 CData 填 type0 47..56；(ii) 补一条调用 `CE26` 的路径。建议 (i)（数据驱动、可回归、不改事件）。

**三态机（[A]，供调试锚点）**：`2000350`：0 idle → `CE242 pc14` 写 1（等玩家命令）→ `CE242` 13 处写 2（执行回合）→ `CE241 回合开始 pc628` 写 0。`CE240 pc695=1 / pc1046=0`（戦闘中 `2000340`）。`CE239` 清理 `pc1043-1046` 清 `2000350/352/343/340`；`pc1106 code 498` 返回。`CE244` 写 `2000352`（win=1 / -1 lose / -2，三态）+ `2000343=1`（唯一写者）。

**日程把玩家送进战斗的精确门**（`docs/research/M6_SCHEDULE.md`，[A]）：`SampleMapA ev4 page2`（workday，trig=1）跑 `CE251 昼コマンド 平日` → `CE300 日中イベント判定`；`CE251` 菜单第 1 行内部 id（`self10` 经定种 RNG）在 day4 变成 7 → `2000048=7` → 派发到 `CE253 地域貢献`（不是 `CE252 鍛錬`）→ `CE253` 无条件调 `CE239`（`pc174/177/180/185/188/191` 六条 `210[500239]`）→ `2000350=1` 等命令，菜单产不出命令 → 时钟冻在 15:00，永不返回。**day4 菜单甚至不画**（pic `1201/1203/1205/1207/1209` 空，无 5 选项 Choice）⇒ `tests/m6_week_loop.rs` 断言 day4 被强推 `CE253/CE239`。复现：`WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m6_week_loop`。

**非战斗 day4→day5 路径在数据层存在**（[A]）：晚间/就寝/次日链（`CE300 → CE326 食事 → CE328 就寝前 → CE329 就寝 → CE317 早朝`）**读不到**任何战斗变量（`2000340/343/350/352`）。**只要白天任务正常返回**就能过。所以 W1 不必「打赢」，**只要让白天事件正常返回**即可解锁主线；但 day4 行 1 固定 id=7→CE253，绕不开 ⇒ 至少要让 `CE239` 快速走完（或修 day4 菜单）。

### 2.2 W2 H 场景运行期

**闭包**（`docs/HSCENE.md` §1.1，[A]）：22 事件 / 15,931 slot / 12,609 条真实指令。入口：`SampleMapA ev33`（H 按钮，336 cmd）→ `CE333 □基本表示`（1565 instr，**唯一**同时碰全部 10 个 hscene 场景组的事件）+ `CE346/340/341`。**最短可玩 H**：`CE246 ┣スキンシップ → 333(立绘) + 346(cutin)`。

**已经不在瓶颈上的**（[A]）：
- H 无消息窗口（0 条 101/102/140）、无音频、无存读档 opcode；VM 侧所有 opcode 已在 `crates/wolf-vm/src/exec.rs` 实现。
- 渲染已解：`crates/wolf-game/src/atlas.rs:42` `ATLAS_SIZE=4096`，`add` 失败经 `atlas.rs:427` `miss` 记诊断、`atlas.rs:459` `take_warnings` 折叠。验收命令 `capture --root work/all --load-save work/retail_backup/Save/SaveData08.sav --frames 60` → **89.4% 非黑 / 86 live pic / ~39 quad / 脸三层齐**。`docs/HSCENE.md` §5.1 目标「≥39 quad、脸可辨识」**已达**。
- 关键：`hscene/`、`hword/`、`DLC_data/` **只在 `work/all`，不在 `work/full`** ⇒ 所有 H 验收必须显式 `--root work/all`。

**剩余缺口**（[A]/[B]）：
1. **交互推进（[A] 缺口，机制清楚）**：H 场景唯一的输入依赖是 **14 条 `123 [1600030,17]`**（`ev346 pc48/119`、`ev371 pc74/157/239/326/412/502/586`、`ev410 pc40/87/134/177/221`），headless 下 `key_bits=0` ⇒ 恒走「没按」分支空转（`game.rs:1527 key_code`）。**注**：`--key-frame N:code` 注入机制**已存在**（`crates/wolf-cli/src/main.rs:85/:159` 解析、`:429 parse_key_frame`、`:1878-1882` 每帧写 `game.key_bits = wolf_game::key::bits_for_code(code)`；`crates/wolf-game/src/key.rs::bits_for_code`）。所以 W2 的推进**不是造轮子，是补测试 + 串一条可复现脚本**。
2. **`run_condition==2` 自动事件调度（[A]）**：文字特效 `350/351/353/354/357/358/367` 不被任何事件调用，靠引擎按变量挂起。HUD/进度的 `auto_events.sync` 已在 `00b7967` 落地，但**H 文字层是否真的被 `sync` 拉起未端到端验收**。
3. **未验证语义（[B]）**：`word_effect` 的 `Move` 是「停放到屏外」还是「上飘」（`docs/HSCENE.md` §3.3）；`hword` 实机可见时长无录像；`290` 计数与文档不一致（640 vs 810）。
4. **DLC（[B]，~M4 体量）**：`CE445` 涉及 70 张 `DLC_data` 图，是独立大块，建议单独排期。

### 2.3 W3 日程 / 任务 / 日历

**机制（[A]）**：**没有独立的日历/任务调度引擎**——主线由 CommonEvent + 变量门控驱动（`var160` 天数、`var6` 星期）。时钟不被帧驱动：只有 `CE217 時刻表示` 按「加分钟」推进（`+delta`，读 self600，**单次 if 进位不是 while**，是刻意保留的引擎怪癖），`CE329/317` 用绝对值改写。验收锚：`tests/time_system.rs`（9 断言，wd=5 23:30 → 9:30 wd=6 flag8=0，`instructions==8088 / frames==1396`）。

**数据化的价值与代价（[C]/[B]）**：`docs/research/GPT_RESEARCH_TRIAGE.md` §3 结论——引擎层基本完备，外部资料的**真实增量在数据层**（日程/任务/数值表）。当前**无** `GameCalendar/StoryFlag/QuestTemplate` 实现；建议按 PDF §10 数据化。
**日期表**（`GPT_RESEARCH_TRIAGE.md` §5.3，**全部 [C]**，攻略级、版本相关、未数据验证）：Day11 展出、Day15 狩猎解锁、Day18 预选、Day24-29、Day33 公会、Day39-43 霞、Day46、Day53、Day55/57-61 训练营、Day64 联合狩猎、Day74/78/84 tournament、Day85-99 遗迹、Day100 锁定、Day106+ 后日谈。
**风险**：这些日期是攻略站口径，**不能当事实写进逻辑**；要做也必须先有权威数据源或逐版本回归。
**下一步**：W3 **不应先做**。先让 W1 把 day4 打通；日历/任务数据化等有可信数据源再启动。

### 2.4 W4 地图滚动 / 传送 / 视差

**核心结论（[A]）**：Sys var **104/105 = SysDatabase type6 行 104/105 的 X/Y スクロール値（magic `9000104/9000105`）= 引擎每帧输出的相机滚动量，不是 opcode**；550 CE + 9 地图里**脚本 0 读 0 写**。⇒ **不要建「玩家/相机跟随模型」**（先前有报告建议做，方向错误）。
- 注册的 6 张地图都无页面图形；`SampleMapA 64x36 = 1024x576` 恰好一屏，`tilemap.quads(camera=[0,0])` 正确。
- **`130` 传送**（[A]）：真实布局 `target,x,y,map,options`，`target=-1` 玩家、地图 id 在 arg3；`[-1,62,38,1,1]`=地图1 SampleMapA、`[-1,60,34,3,1]`=地图3 Map003。**已解，可用**。
- **`281`（滚动锁）** 只有 3 处（`CE225`），是样式锁不是位移；**视差**（SysDatabase type13 + 地图行 col4）**无真实使用点**。
**下一步**：W4 **最低优先级**，只在出现 >1 屏滚动地图时再回头。**风险**：低。

### 2.5 W5 UI / 文本 / 资产 / 音频 / 存档 完整度

**已知否定项清单**（`docs/UNVERIFIED.md`，挑可行动者）：

| 项 | 现象 | 级别 | 状态 / 下一步 |
|---|---|---|---|
| H-124 `124 SetVariableEx`（579 处） | 现实现 = 保留旧值 + Info 诊断 | 功能缺口 | **唯一功能级**已知否定；H 场景 `0x3000/0x4000` field 8/9 已实现，剩余是通用 long-format 语义 |
| H-MENU-1 `102` 运行时选择窗 | 只捕获 `Game::choices`，capture 看不到 1/2/3 | 视觉 | 需在渲染层画 choice 窗；否则验收截图无法显示选择 |
| H-WIN-1 窗口皮肤 / 九宫格 | `type3 0x0` 首帧不画；CN patch 布局冲突、无零售截图 | 视觉 | 低优先，缺对照图 |
| H-PRNG-1 PRNG | 未从 `game_dat_random.c` 移植 → 战利/命中无法逐帧复现 | 确定性 | 战斗回归需要它；建议随 W1 一起做 |
| H-PIC-1/2、H-ANC-1 | 负号图/长格式槽[2][5]/锚点位 | 边缘 | 像素已证安全，低优先 |
| H-KEY-2 | 键盘码模型完成；arg[1] mask 语义未定；无名码 13/14/20/100/300 当「永不返回」 | 输入 | 与 W2 一起复检 |

**MAP_AUDIT §9 未验证项**（`docs/MAP_AUDIT.md:570-578`）：①`MapTree.dat` 已解码，只剩「引擎如何用树启动新游戏」待实测；②`130` 的 `-2` 目的地（仅 `Map002` 4 次，x/y 变量间接）；③`self100006/self100009` 写入时机（引擎写还是事件写）；④`confirm` 事件格 vs 图片矩形两套判定交互规则；⑤`Map024/Map031` 无瓦片时的渲染（黑屏 vs 保留背景）。

**音频**：H 场景本身**无 `140` opcode**（[A]）；全作音频/BGM 的真实点在外部地图事件，**是否与零售一致需人耳复检**（[B]，无自动验收手段）⇒ 作为**人工验收项**排期，不占自动回归。
**存档完整度**：中期存/读已在一个测试（匿名 day1→day4）里证明（[A]）；`SavePart5` 图片 split 已被 C17 实测证明存盘正确（[A]）。剩余「全 `SavePart` 覆盖」尚未逐段核对（[B]），建议随 W1 存档回归补齐。

---

## 3. 依赖顺序（建议执行序）

1. **W1 步骤 1（命令映射，最便宜）** — 只改数据/加载，风险最低，先把 `CE152` 打开。
2. **W1 步骤 2/3（武器 DB / 敌目标）** — 让 `CE65` 不再自旋。
3. **W1 结算** — 让 `CE244` 被到达、`2000352` 落地 ⇒ day5+ 解锁。
4. **W2 交互推进**（与 W1 并行，独立）— 复用既有 `--key-frame`，把 14 个轮询打通，出第一张「能往下走的 H」。
5. **W5 顺手清账**（H-MENU-1 视觉、H-PRNG-1 确定性）— 低风险、提升验收可读性。
6. **W3 / W4** — 数据源可信后 / 出现多屏地图时再动。

---

## 4. 建议的第一个纵切（ONE vertical slice）

> **纵切名：W1-a「一次真实战斗回合打到 CE244 结算」**

**为什么选它**：它是 M7 的头号，且是唯一能解除 day4→day5 主线阻塞的路径；验收指标单一、可机检、失败点已被精确定位到 `CE152 pc89..pc95 / CData type7 item count / type18 row17`（[A]），风险可控。相比之下 W2 也独立可做，但 H 的 `Move`/`hword` 语义还带 [B] 未定项，先做战斗能拿最硬的成果。

**范围**（薄而全，穿数据→VM→game 三层）：
- 数据：新游戏时从 `root/Save/System.sav` 的 CData 填 **type0 列 47..56**（命令 1..8）；填 **type8 所持武器個数**；`CE190 配置` 阶段保证 **type18 敌行**非空。
- VM：无需新 opcode（`CE152/CE65` 指令均已实现，问题在数据）。
- game：从 `Game::new` + 现有存档种子，`start_event(188)` 或经 `CE253→CE239`，注入一次「攻击」命令（`--key-frame` / 直设 self），跑到 `CE244` 结算后 `CE239` 返回。

**可机检验收（写进 `crates/wolf-game/tests/m7_battle_round.rs`）**：

| # | 断言 | 阈值/期望 |
|---|---|---|
| 1 | `CE152` 对真实攻击（行 26 或 25）返回「可用」 | ≥ 1 条可选真实攻击 |
| 2 | 战斗段 `250 DatabaseAccess` 未应答数 | **== 0**（当前 250） |
| 3 | `CE65` 不再 `pc78↔pc140` 自旋 | 帧数有界（≤ N 帧内离开该区间） |
| 4 | `2000350` 三态推进 | 观测到 `0→1→2→0` 的完整回路 |
| 5 | `CE244` 被到达并写结算 | `2000352 ∈ {1, -1}` 且 `2000343 == 1` |
| 6 | `stats.warnings` | **== 0** |
| 7 | 回合后 `CE239` 正常返回（`pc1106`），不挂死 | 回到调用者，时钟可继续推进 |

**回归护栏**：`cargo test -q -p wolf-game` 全绿（含 `m6_battle_real`、`m6_week_loop` 的既有断言不得退化为失败）；`m6_week_loop` 的「day5 unreachable」注释在验收后应升级为「day5 可达」并补一条 day4→day5 实际跨日断言。

**若 W1-a 推进受阻**（如 type18 数据在 `System.sav` 缺失），回退纵切：**W2-a「H 场景交互推进」**——用 `--key-frame` 跨过全部 14 条 `123 [1600030,17]`，验收 = 14 个轮询全部越过 + `capture` 出 `≥39 quad / ≥89.4% 非黑 / 0 warning` + 到达定义好的终止 pc。该纵切**不依赖战斗**，可独立交付。

---

## 5. 明确不做 / 降级

- **不做玩家/相机跟随模型**：Sys 104/105 是引擎相机输出（§2.4，[A]），做了就是错方向。
- **不把攻略日期表当逻辑**：W3 的 Day11/15/… 全是 [C]，无权威数据前不落代码。
- **不追 `290` 的 552 条 `not drawn yet`**：H 里那些是「目标图当时不存在」，不是渲染 bug（`docs/HSCENE.md` §3.7，[A]）。
- **不先做 DLC（CE445，70 图）**：独立 ~M4 体量，单独排期。
- **不在本任务内 commit/push**：仅新增本文档；`crates/wolf-format/examples/opscan.rs`、`crates/wolf-format/tests/_scratch_cdata.rs`、`crates/wolf-format/tests/_scratch_event.rs` 三个永久 scratch 文件**永不入库**，且**不跑 git clean**。

---

## 6. 附：证据锚点速查

| 主题 | 文件 | 关键行/数字 |
|---|---|---|
| M6 收尾判据 | `docs/HANDOFF_M4.md` | day loop → day4 08:30；teardown fix `map_runtime.rs:328-348` |
| 战斗真实线 | `docs/research/M6_BATTLE_REAL.md` / `M6_BATTLE_COMMANDS.md` | CE239 1108 slots/104 sites；CE244 432 slots；CE152 pc89..pc95；CData type7 item count；type18 row17==0 |
| day4 强制战斗 | `docs/research/M6_SCHEDULE.md` / `crates/wolf-game/tests/m6_week_loop.rs:306` | CE251 row1 id=7→CE253→CE239；`day 5 is unreachable here` |
| 战斗命令槽 | `docs/PARTY_DB.md:355` | CData type0 47..56 = コマンド1..8；CE26 caller=0 |
| H 闭包 / 渲染 | `docs/HSCENE.md` §1.1/§5.1；`atlas.rs:42` | 22 events/12,609 instr；89.4%/~39 quad；4096 atlas |
| 时钟 | `docs/TIME_SYSTEM.md` §1；`tests/time_system.rs` | var4/5/6/8；8088 instr/1396 frames |
| 地图滚动 | `docs/HANDOFF_M3.md` §5 | Sys type6 行104/105=相机输出；130 传送；281 仅 3 处 |
| 已知否定 | `docs/UNVERIFIED.md`；`docs/MAP_AUDIT.md:570-578` | H-124/H-MENU-1/H-PRNG-1/H-WIN-1；MAP_AUDIT §9 五项 |
| 外部研究 | `docs/research/GPT_RESEARCH_TRIAGE.md` §3/§5.3 | 增量在数据层；日期表全 [C] |