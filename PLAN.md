# 《妹相随～黑白世界的缤纷冒险～》Rust 重写 — 当前路线图

> 本文件是**当前路线图（living document）**，只记录“现在做到哪、接下来做什么”。
> 原始可行性研究与 M0–M5 的详细规划已归档到 [`docs/archive/PLAN_2026-10-26.md`](docs/archive/PLAN_2026-10-26.md)，仅作历史参考。
> 最新交接：[docs/HANDOFF_M12.md](docs/HANDOFF_M12.md)　｜　文档总索引：[docs/README.md](docs/README.md)　｜　协作约定：[AGENT.md](AGENT.md)

## 1. 当前状态

- 当前源码 HEAD：以 `git log` 为准。M10 交接/基线为 `e52855d`；M11 第一批/round 1/round 2 分别见 `22de104`、`5ef6b1f`、`fdeed07`；M11 round 3 见 M12 交接提交。
- 里程碑：**M0–M11 全部完成并收口；M12 round 1（task-101..103）、day6（task-104）与 30/90 天 soak（task-105）已完成**，可运行、可交付的工程约 **96%**；严格 1:1 原版等价约 **87–91%**。该百分比是引擎/子系统口径——按用户 2026-11-01 的“全内容一致 + 修 bug”口径，主线后半/结局、DLC、随机事件与 H 阈值矩阵仍未验收（见 §4.5）。
- 质量门禁：`cargo test -q --workspace` 全绿（133 test result / 812 passed / 2 ignored）；`./target/debug/wolf-cli.exe verify --root work/all` = **PASS 70 / pending 0**。
- 工作树：3 个永久 scratch（`crates/wolf-format/examples/opscan.rs`、`crates/wolf-format/tests/_scratch_cdata.rs`、`crates/wolf-format/tests/_scratch_event.rs`）与用户资料（`docs/gpt调研/`、`docs/攻略文档/`、`游戏示例图片/`、`docs/research/GUIDE_CONTENT_CROSSWALK.md`）未跟踪；`docs/README.md`、`docs/research/README.md` 有用户自己的未提交改动。**这些都不提交**。

## 2. 里程碑总表（现行命名）

| 里程碑 | 状态 | 主题 | 主要记录 |
| --- | --- | --- | --- |
| M0 | 完成 | 加载器 / 格式解析 / verify 基线 | `FORMAT_SPEC.md`、`BASELINE.json` |
| M1 | 完成 | 标题画面像素级还原 | [`M1_CLOSURE.md`](docs/M1_CLOSURE.md) |
| M2 | 完成 | 开场剧情纵切片（消息/选择/存档） | [`M2_SLICE.md`](docs/M2_SLICE.md) |
| M3 | 完成 | 完整运行时：数据库、时间/日程、地图、战斗/H 调研 | [`HANDOFF_M3.md`](docs/HANDOFF_M3.md) |
| M4 | 完成 | 音频后端与接线 | [`HANDOFF_M4.md`](docs/HANDOFF_M4.md)、[`AUDIO.md`](docs/AUDIO.md) |
| M5 | 完成 | 存档层与官方 .sav 互通 | [`SAVE_SPEC.md`](docs/SAVE_SPEC.md)、[M10 save audit](docs/research/M10_SAVE_COMPLETENESS.md) |
| M6 | 完成 | 主线贯通与可玩日常循环 | [`HANDOFF_M6.md`](docs/HANDOFF_M6.md) |
| M7 | 完成 | 真实战斗 / H 场景切片 / 零售 PRNG / 选择窗口 | [`HANDOFF_M7.md`](docs/HANDOFF_M7.md) |
| M8 | 完成 | day4 战斗结算→day5、H 前向链、地图角色层、模板存档、事件 RNG | [`HANDOFF_M8.md`](docs/HANDOFF_M8.md) |
| M9 | 完成 | 锚点/图片槽位、H 闭环、战斗多回合/多队员、移动与地图触发、音频盘点 | [`HANDOFF_M9.md`](docs/HANDOFF_M9.md) |
| M10 | 完成 | opcode 111 引用语义与 compound 修复、12 个战斗/H 测试重基线、残项收口 | [`HANDOFF_M10.md`](docs/HANDOFF_M10.md) |
| M11 | 完成 | 战斗默认驱动/确定性、菜单桥、H 深链、引擎行走/交互能力、视觉/音频回归 | [`HANDOFF_M11.md`](docs/HANDOFF_M11.md) |
| M12 | 进行中 | 零售范围审计 + round 1（战斗取消/confirm 页/存档逐字节）+ day6 周末日程 + 30/90 天 soak | [`HANDOFF_M12.md`](docs/HANDOFF_M12.md) |

## 3. 当前完成度（按子系统）

| 子系统 | 完成度 | 说明 |
| --- | ---: | --- |
| 数据格式 / VM / opcode | 100% | 50 种 opcode 与格式解析已收口；opcode 111 引用语义已落地 |
| 存档 | 99% | 官方 .sav 往返、SavePart 完整性、模板存档、row counts；M12 起 `Game` 层 load→save 与 8 个零售存档整文件逐字节一致；未建模音调/效果 bank 只保留不模拟 |
| 主线 / 日常循环 | 97% | 标题→新游戏→多日日程→睡眠→次日已打通；通用驱动连续 30 天（默认）与 90 天（ignored）到 day97，覆盖周末、剧情战斗与跳天 |
| 战斗 | 96% | 真实回合、结算、BOSS 多回合、真实 2 人队伍、默认 auto-arm（CE253 + 任意 parented CE239）、真实 CData type14 菜单→3 个 CE242 body、取消/返回/连续确认验收已完成；30 天 soak 覆盖 11 个战斗日；剩余仅目标选择/左右切换的逐项 UI 验收（可选） |
| H 场景 | 90–94% | 真实入口→推进→退出→次日已闭环；真实 CE328→CE402/CE410 自然帧（默认预算）已证；act4/5 无零售自然证据、不计必做，剩余为零售像素对照 |
| 地图移动 / NPC | —（非内容项） | 零售注册地图是点触式（6 张地图 115 页 `graphic` 全空，150 图片 + 124 悬停）；confirm 页审计已收口，事件格不是命中区；8 向行走/ActionKey 仅为引擎通用能力与测试夹具 |
| 视觉 | 90% | 标题/地图/窗口/锚点大部分还原；锚点 5/6 与 CN 窗口右缘待像素对照 |
| 音频 | 90% | 358 个音频盘点、混音回归、异常 WAV 容错；剩余人工听感验收 |
| 测试 / 文档 | 100% | 133 test suites / 812 passed / 2 ignored、verify 70/0、HANDOFF_M12/BASELINE/research 索引齐全 |

> 上表按引擎/子系统口径；`主线 / 日常循环 97%` 指循环本身，不等于全内容通关。按 2026-11-01“内容一致 + 修 bug”标准的内容缺口见 §4.5。

## 4. M12 候选与优先级

> 原则：先消除“会让玩家流程不确定”的缺口，再做视觉/音频打磨；每项都必须有测试或可复现证据。
>
> 进度：M11 第一批（task-79..84）与 round 1（task-85..88）见 `docs/HANDOFF_M10.md` §8/§9；round 2（task-89..96）见 `docs/HANDOFF_M11.md`；round 3（task-97..100）与零售范围审计见 `docs/HANDOFF_M12.md` §2/§5；M12 round 1（task-101..103）见 `docs/HANDOFF_M12.md` §8。下列条目保留仍未完成的部分。
>
> **范围约束（M12 审计）**：只实现零售能走到的流程；引擎通用但本作不用的行走/碰撞/contact/ActionKey、触发 3/4、相机/281、201 路线、远景视差不再新增，也不作为内容缺口。证据见 `docs/research/M12_RETAIL_SCOPE.md`。

### P0 — 确定性与真实流程

1. **战斗自然路线确定性（已完成，task-79）**：`started_events` 已是 `BTreeSet`（有序，非 HashSet），实例为 `Vec`，HashMap 仅键查表；自然 day4 路线已逐字节复现（5 次重跑一致：4 回合、v352=1），#11/#12 已改为定值断言（见 `docs/research/M11_BATTLE_DETERMINISM.md`）。PRNG 零售时钟种子来源与无 folding 已由 task-99 反汇编证实（`M11_PRNG_CLOCK.md`）；剩余仅是零状态重读导致“单 tick 完全重放”不可证。
2. **真实多命令菜单桥接（真实菜单已通，task-85/89/93/95/98/101）**：`battle_wiring.rs` 实现 CE157 确认采集、`set_menu_command_queue`，`Game::set_battle_menu_command_queue` 为应用层入口。task-89 推翻“CE157 cursor VM bug”：物理 cursor 是 CData type18 row73；task-93 用 2 成员 fixture 注册 15 行并驱动 4 个不同 CE242 body；task-95 默认 auto-arm 让自然 day4 战斗自行结算；task-98 harvester 改读 committed CData type14 并移到 `drive` 判断之前，真实 2 成员菜单无需注入即注册 2 行、驱动 3 个 CE242 body；task-101 完成取消/返回/连续确认验收（`M12_BATTLE_CANCEL.md`）。剩余只有目标选择/左右切换的逐项 UI 验收（可选）；3–4 人队伍不是零售内容。

### M12 round 1（task-101..103，已完成）

- **战斗取消/返回/连续确认**：`crates/wolf-game/tests/m12_battle_cancel.rs` + `docs/research/M12_BATTLE_CANCEL.md`。CE191 窗口取消重开且不登记；CE192 窗口取消退出且不写行/不启动 CE157；后续确认仍得到 `[(1,0,26),(2,1,25)]`、`ce157_starts=2`。无生产改动。
- **confirm 页命中审计**：`crates/wolf-game/tests/m12_confirm_pages.rs` + `docs/research/M12_CONFIRM_PAGES.md`。5 个空页选择器 + ev24 p6 `self0==99` 调试页；全库无写 `1_000_000`；事件格不是点击命中区，124 图片矩形才是。`MAP_AUDIT.md` §9 对应项已标为解决。
- **存档逐段全量对照**：`crates/wolf-game/src/picture.rs` / `savegame.rs` + `crates/wolf-game/tests/m12_save_full_compare.rs` + `docs/research/M12_SAVE_FULL_COMPARE.md`。`Picture::saved_record` 写入基线；8/8 零售存档整文件逐字节一致；未建模 bank/文本 md1/淡入/纵向缩放保留。

### M12 day6（task-104，已完成）

- **day5→day7 周末日程**：`crates/wolf-game/tests/m12_day6_weekend.rs` + `docs/research/M12_DAY6_WEEKEND.md`。day5 工作日 → 周六 09:30 CE310 休日五按钮（外出/自主锻炼/处理积攒的家务/努力打工/悠然自得）→ 三个活动时段（12:00/15:00/17:00）→ 18:30 晚餐/就寝 → 周日 09:30 同一菜单；`frames=11247 instructions=580912`、warnings=0，3 次运行一致；无时间/日程注入。
- **多日通用 soak**：`crates/wolf-game/tests/m12_multiday_soak.rs` + `docs/research/M12_MULTIDAY_SOAK.md`。一个驱动连续 30 天到 day31（11 个战斗日、4 个周末、7 个工作日菜单日，warnings=0）；`#[ignore]` 90 天到 day97（含剧情跳天）；修复 auto-arm 只认 CE253 的缺口，parented CE239 也可武装。

### 4.5 下一阶段：内容一致（M13，2026-11-01 用户定标）

> 完成标准：**重构版体验内容与原版一致，且修复可能存在的 bug**。验收对象是每条零售可达内容路径，不是引擎百分比。

已核实边界（HEAD `b2da473`，8 个零售存档）：最远 day90（SaveData01/08），`var619 DLCチャプター` 全 0，`var466/492/493` 结局旗标全 0；后半与 DLC 没有零售存档可对照。`m12_multiday_soak` 到 day97 只证循环，不证 stage/结局。

- **P0-A 主线后半与结局**：SaveData01/08 day90 → stage 25→27→28/30；另跑约 day100 期限的逃亡 END；再补其余 7 个结局。
- **P0-B DLC CE452**：CE251@2498 → EP0–EP34 → CE446/447；第二主线 CE424–CE428 温泉讨伐链。
- **P0-C 内容矩阵**：111 条随机事件、H 分支、战斗敌人/胜负、道具/技能/CG，逐条标 `已测 / 需夹具 / 数据不可达`。
- **P1-D**：跨天存读 + NG+ 继承（CE436）+ 全存档槽。
- **P1-E**：锚点 5/6 与关键场景零售像素 diff；31 首 loop 人耳。
- **Bug 政策**：移植版 bug 必修；原版明确缺陷（死锁/崩溃/内容不可达/文本错位）修并记录差异；无害怪癖默认保留，单列决策。

第一步与可执行细节见 `docs/HANDOFF_M12.md` §9。

### 4.6 M13 进展（2026-11-01 首批验收）

- 已修复 common-event self 跨调用持久 bug（`7c3b728`）与 Text picture 存读尺寸丢失（`dcd513a`）。
- 已验收：主线 SaveData01/08 无注入通关（stage28→29，var492=1）、逃亡 END、DLC CE452（自然 var619 0→25；注入 EP0–36 全通）、111 随机事件抽样（25/115，dead=0）、H 阈值矩阵（37 门边界）、NG+ 继承（CE436/SampleMapA）、跨天存读与 8/8 槽 round-trip。
- 全量：836 passed / 0 failed / 10 ignored；verify PASS 70/0。
- 剩余：像素/音频人工验收（P1-E）。详见 `docs/HANDOFF_M13.md` §6。
- **第二批（task-13..18）**：九结局其余 7 条、17 条件注入随机事件、H 端到端、DLC EP26–36 结案均已完成（全量 860 passed / 0 failed / 14 ignored；verify PASS）。DLC 结案结论：var395 是永久献身技能标志不可重置，EP26 卡点为探针缺已升级队伍属性，预置后自然链 EP0→36/var619=36；像素/音频仍需人工。
- **第三批（task-19..23）**：P1-E 可自动化审计、DLC 眼睛层根因定位、**读档伪启动修复**（`AutoEvents::prime_latched`，读档不再伪造 run==2 边沿、不再多画 506 眼睛层 -6）、h_overlay 视觉基线重录、P1-E 修复后复测；全量 **863 passed / 0 failed / 14 ignored**，`wolf-cli verify` **PASS 70/0**。P1-E 机器部分闭环（1–7 档全指标改善，第 8 档仅 HUD +1 glyph 波动、场景区改善），仅剩 H2–H5 人工（锚点零售像素核对、HUD 抗锯齿、28 首 loop 人耳）。详见 `docs/HANDOFF_M13.md` §6.2。

### P1 — 内容完整性

3. **H 完整 CG 分支（自然帧已证，task-80/86/90/94/96/97）**：var304=1、var276/277/326 可从 SaveData01 真实输入获得；task-96 用 calendar-consistent fixture 只 `start_event(328)` 经 pc986 进 CE402（pc0→511）并驱动 CE410（pc0→247），warnings=0；task-97 用默认 200000 预算、普通 `Game::frame`、关闭背景 AutoEvents 复现自然帧 `frames=3667 maxpc402=509 CE410 acts=[1,1,2,1,3] warnings=0`，并新增 `--load-save`+`--event` 截图入口。关键修正：pc122 就寝是 `var6==4 && var4<6`；可达路径是 pc850 假→else pc969→pc986，不经过 pc964/var64。**已确认非缺口**：act4/5 只在 budget≤5 饥饿观测出现，无零售自然证据（`M11_HSCENE_NATURAL.md`、`M12_RETAIL_SCOPE.md`）。
4. **移动 / NPC（引擎通用，不是零售内容，task-81/91/102）**：零售注册地图是点触式（6 张地图 115 页 `graphic` 全空，交互靠 150 图片 + 124 悬停），没有 NPC 行走动画/走位对话；8 向对角移动、contact、每 8 帧连续行走与 ActionKey 只是引擎能力与测试夹具，不再作为内容缺口。M12 task-102 已收口点触命中框与事件格：6 个 confirm 页不是点击目标，命中区是 124 图片矩形（`M12_CONFIRM_PAGES.md`）。
5. **日程边界**：通用 soak 已连续覆盖 30 天（默认测试）与 90 天（ignored，day84→97 剧情跳天）；跨天存档一致性仍可补。

### P2 — 还原度与交付

> 2026-11-01 标准下，P2 排在内容覆盖之后；先做 §4.5 的 P0-A/B/C。

6. 视觉像素对照：锚点 5/6 几何已固定（task-82，`M11_ANCHOR_PIXELS.md`）；CN 窗口皮肤右缘已证为 data-only 且无 1024 宽溢出（task-87，`M11_WINDOW_EDGE.md`）。剩余零售像素 diff、关键场景截图 diff。
7. 音频人工听感验收：已产出 31 首 loop 验收档案与分层（task-84，`M11_AUDIO_ACCEPTANCE.md`），并有逐曲阈值固定、两次 decode 位相同的回归（task-88，`M11_AUDIO_LOOP_REGRESSION.md`）；剩余 BGM 循环点/SE 音量延迟的人工确认。
8. 打包/性能（工具，非游戏流程）：`wolf-package`（validate/assemble/check/bench）与 `FrameBudgetReport` 已落地（task-100，`M11_PACKAGING.md`）；剩余 Windows 发布包接线、启动数据目录自动校验、release 性能与资源缓存，可按需做。
9. 文档：每个新里程碑完成时按 §5 收口。

## 5. 里程碑完成的定义（Definition of Done）

一个里程碑只有同时满足以下条件才算完成：

1. **代码**：实现落地，未破坏既有测试；关键行为有集成测试或可复现命令。
2. **验证**：`cargo test -q --workspace` 全绿；`wolf-cli verify` 保持 PASS 70 / pending 0（或同步更新基线并说明原因）。
3. **交接**：写 `docs/HANDOFF_Mx.md`（现状、证据、边界、下一步），把上一份交接标为历史归档。
4. **文档**：更新本 `PLAN.md`、`docs/README.md`、`docs/BASELINE.json`；研究底稿登记到 `docs/research/README.md`。
5. **清理**：按 [AGENT.md](AGENT.md) §6 清理临时文件/构建缓存/worktree，确认 `git status` 只剩约定的未跟踪文件。

## 6. 文档地图

- 总索引：[docs/README.md](docs/README.md)
- 新会话启动：[docs/NEW_SESSION_START.md](docs/NEW_SESSION_START.md)
- 最新交接：[docs/HANDOFF_M12.md](docs/HANDOFF_M12.md)
- 技术规格：[docs/FORMAT_SPEC.md](docs/FORMAT_SPEC.md)、[docs/OPCODE_SPEC.md](docs/OPCODE_SPEC.md)、[docs/SAVE_SPEC.md](docs/SAVE_SPEC.md)
- 子系统全景：[docs/BATTLE.md](docs/BATTLE.md)、[docs/HSCENE.md](docs/HSCENE.md)、[docs/MAP_AUDIT.md](docs/MAP_AUDIT.md)
- 研究底稿：[docs/research/README.md](docs/research/README.md)
- 历史原稿：[docs/archive/PLAN_2026-10-26.md](docs/archive/PLAN_2026-10-26.md)
