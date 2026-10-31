# 《妹相随～黑白世界的缤纷冒险～》Rust 重写 — 当前路线图

> 本文件是**当前路线图（living document）**，只记录“现在做到哪、接下来做什么”。
> 原始可行性研究与 M0–M5 的详细规划已归档到 [`docs/archive/PLAN_2026-10-26.md`](docs/archive/PLAN_2026-10-26.md)，仅作历史参考。
> 最新交接：[docs/HANDOFF_M10.md](docs/HANDOFF_M10.md)　｜　文档总索引：[docs/README.md](docs/README.md)　｜　协作约定：[AGENT.md](AGENT.md)

## 1. 当前状态

- 当前源码 HEAD：`e52855d`（M10 交接/基线文档提交；M10 代码提交为 `ce680b3` + `3bbbb12`）。以 `git log` 为准。
- 里程碑：**M0–M10 全部完成并收口**，可运行、可交付的工程约 **90%**；严格 1:1 原版等价约 **80–85%**。
- 质量门禁：`cargo test -q --workspace` 全绿；`./target/debug/wolf-cli.exe verify --root work/all` = **PASS 70 / pending 0**。
- 工作树：仅允许 3 个永久 scratch（`crates/wolf-format/examples/opscan.rs`、`crates/wolf-format/tests/_scratch_cdata.rs`、`crates/wolf-format/tests/_scratch_event.rs`）与用户资料 `docs/gpt调研/` 未跟踪。

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

## 3. 当前完成度（按子系统）

| 子系统 | 完成度 | 说明 |
| --- | ---: | --- |
| 数据格式 / VM / opcode | 100% | 50 种 opcode 与格式解析已收口；opcode 111 引用语义已落地 |
| 存档 | 95% | 官方 .sav 往返、SavePart 完整性、模板存档、row counts 已覆盖；剩余逐段全量对照 |
| 主线 / 日常循环 | 95% | 标题→新游戏→多日 08:30 日程→睡眠→次日已打通 |
| 战斗 | 85–90% | 真实回合、结算、BOSS 多回合、多队员已证；自然路线 RNG 与多命令菜单仍是硬缺口 |
| H 场景 | 85–90% | 真实入口→推进→退出→次日已闭环；深层 CG 仅合成存档可达 |
| 地图移动 / NPC | 70–75% | 地形/碰撞/接触触发/玩家起点已实现；对角移动、NPC、动画仍缺 |
| 视觉 | 90% | 标题/地图/窗口/锚点大部分还原；锚点 5/6 与 CN 窗口右缘待像素对照 |
| 音频 | 90% | 358 个音频盘点、混音回归、异常 WAV 容错；剩余人工听感验收 |
| 测试 / 文档 | 100% | 99 binaries / 701 tests、verify 70、HANDOFF/BASELINE 齐全 |

## 4. M11 候选与优先级

> 原则：先消除“会让玩家流程不确定”的缺口，再做视觉/音频打磨；每项都必须有测试或可复现证据。

### P0 — 确定性与真实流程

1. **战斗自然路线确定性**：`started_events` 的顺序影响预战斗 RNG 消费，导致 3–4 回合/胜负摆动；需要把事件 RNG 的零售时钟种子与 replay 固定下来（见 `docs/research/M9_PRNG_SEED.md`、`HANDOFF_M10.md` §5）。
2. **真实多命令菜单桥接**：shipped CE188/CE157 数据层只注册一条命令；需要在不改数据的引擎侧补上“菜单选择 → CE242 命令索引”的桥（见 `docs/research/M9_BATTLE_LIVE_BRIDGE.md`、`M9_BATTLE_MULTI_INPUT.md`）。

### P1 — 内容完整性

3. **H 完整 CG 分支**：CE402/CE410 深层链目前只靠合成存档可达；目标是在真实存档/真实操作下达到。
4. **移动 / NPC**：对角移动、连续行走动画、NPC 占位与交互、地图间传送后的起点一致性。
5. **日程边界**：更多天数/随机事件的自然推进与跨天存档一致性。

### P2 — 还原度与交付

6. 视觉像素对照：锚点 5/6 几何、CN 窗口皮肤右缘、关键场景截图 diff。
7. 音频人工听感验收（BGM 循环点、SE 音量/延迟）。
8. 打包/性能：Windows 发布包、启动时用户数据目录校验、每帧预算与资源缓存。
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
- 最新交接：[docs/HANDOFF_M10.md](docs/HANDOFF_M10.md)
- 技术规格：[docs/FORMAT_SPEC.md](docs/FORMAT_SPEC.md)、[docs/OPCODE_SPEC.md](docs/OPCODE_SPEC.md)、[docs/SAVE_SPEC.md](docs/SAVE_SPEC.md)
- 子系统全景：[docs/BATTLE.md](docs/BATTLE.md)、[docs/HSCENE.md](docs/HSCENE.md)、[docs/MAP_AUDIT.md](docs/MAP_AUDIT.md)
- 研究底稿：[docs/research/README.md](docs/research/README.md)
- 历史原稿：[docs/archive/PLAN_2026-10-26.md](docs/archive/PLAN_2026-10-26.md)
