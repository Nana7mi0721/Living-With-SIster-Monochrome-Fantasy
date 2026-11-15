# 文档总索引

> **当前阶段（2026-11-14）：Godot 重制版 M1 已完成 / 进入 M2 主循环与 UI。**
>
> 最高优先级入口：[`godot/README.md`](godot/README.md) →
> [`godot/DECISIONS.md`](godot/DECISIONS.md) →
> [`godot/PLAN.md`](godot/PLAN.md) →
> [`godot/M1_HANDOFF.md`](godot/M1_HANDOFF.md) →
> [`godot/M0_HANDOFF.md`](godot/M0_HANDOFF.md)。
>
> 根目录 [`../PLAN.md`](../PLAN.md) 是当前路线摘要；
> 旧 Rust 路线图已归档为 [`archive/PLAN_RUST_2026-11-04.md`](archive/PLAN_RUST_2026-11-04.md)。

## 0. 状态图例

| 状态 | 含义 |
| --- | --- |
| **现行** | 当前执行的决策、路线或协作规则 |
| **迁移输入** | Rust 阶段验证出的格式/行为规格，Godot 重制仍应复用 |
| **历史证据** | 旧实现、里程碑、测试和研究底稿；只用于追溯与 Oracle |
| **归档** | 已被新决策替代的入口、策略或路线图 |
| **用户资料** | 攻略、外部调研、第三方工具；不作为项目权威 |

## 1. 先读什么

| 目标 | 阅读顺序 |
| --- | --- |
| 开始 Godot 工作 | [`godot/README.md`](godot/README.md) → [`godot/DECISIONS.md`](godot/DECISIONS.md) → [`godot/PLAN.md`](godot/PLAN.md) |
| 查看 M1 验收证据 | [`godot/M1_HANDOFF.md`](godot/M1_HANDOFF.md) → [`godot/PLAN.md`](godot/PLAN.md) §4 |
| 查看 M0 验收证据 | [`godot/M0_HANDOFF.md`](godot/M0_HANDOFF.md) → [`godot/DATA_CONTRACT.md`](godot/DATA_CONTRACT.md) → [`godot/OPCODE_TEST_PLAN.md`](godot/OPCODE_TEST_PLAN.md) |
| 查旧 Rust 风险 | [`godot/LEGACY_RUST_AUDIT.md`](godot/LEGACY_RUST_AUDIT.md) → [`godot/audit/GEMINI_AUDIT_AND_PLAN_2026-11-14.md`](godot/audit/GEMINI_AUDIT_AND_PLAN_2026-11-14.md) |
| 查二进制格式 | [`FORMAT_SPEC.md`](FORMAT_SPEC.md) → [`SAVE_SPEC.md`](SAVE_SPEC.md) → [`PARTY_DB.md`](PARTY_DB.md) |
| 查 opcode | [`OPCODE_SPEC.md`](OPCODE_SPEC.md) → 对应 `OPCODE_*.md` 专题 |
| 查战斗/H/地图/文本 | [`BATTLE.md`](BATTLE.md) / [`HSCENE.md`](HSCENE.md) / [`MAP_AUDIT.md`](MAP_AUDIT.md) / [`TEXT_SPEC.md`](TEXT_SPEC.md) |
| 查旧里程碑做了什么 | [`archive/README.md`](archive/README.md) → 下方 §3 的交接表 |
| 查研究底稿 | [`research/README.md`](research/README.md) |

## 2. Godot 现行文档

| 文档 | 状态 | 内容 |
| --- | --- | --- |
| [`godot/README.md`](godot/README.md) | 现行 | Godot 阶段唯一入口、当前状态、维护规则 |
| [`godot/DECISIONS.md`](godot/DECISIONS.md) | 现行 | D-01..D-15 定稿决策、`.sav` 边界、废止旧方案 |
| [`godot/PLAN.md`](godot/PLAN.md) | 现行 | M0–M5 路线、M0/M1 退出（已完成）、M2 入口 |
| [`godot/M1_HANDOFF.md`](godot/M1_HANDOFF.md) | 现行 | M1 验收交接：切片运行链、存档续跑、证据与边界 |
| [`godot/M0_HANDOFF.md`](godot/M0_HANDOFF.md) | 现行 | M0 验收交接：环境门限、对账、.sav 导入、Oracle 对拍 |
| [`godot/DATA_CONTRACT.md`](godot/DATA_CONTRACT.md) | 现行 | 离线转换数据契约（manifest / programs / database / save） |
| [`godot/OPCODE_TEST_PLAN.md`](godot/OPCODE_TEST_PLAN.md) | 现行 | Top 5 高频 opcode（`401, 210, 121, 111, 180`）测试计划 |
| [`godot/LEGACY_RUST_AUDIT.md`](godot/LEGACY_RUST_AUDIT.md) | 现行 | 22 项 Rust 风险、行为兼容矩阵、禁止带入 Godot 的习惯 |
| [`godot/HANDOFF_TEMPLATE.md`](godot/HANDOFF_TEMPLATE.md) | 模板 | 后续 Godot 里程碑交接格式 |
| [`../oracle/TRACE_CONTRACT.md`](../oracle/TRACE_CONTRACT.md) | 现行 | Rust Oracle 与 Godot EventVM 对拍格式规范 |
| [`../PLAN.md`](../PLAN.md) | 现行 | 当前路线摘要与下一步 |
| [`../AGENT.md`](../AGENT.md) | 现行 | 协作规则；Godot 阶段以新入口为准 |

## 3. 迁移输入（稳定参考）

这些文档来自 Rust 阶段，但对方格式、事件语义和内容验收仍有复用价值。
它们不是 Godot 现有实现的说明；若与 `godot/DECISIONS.md` 冲突，以决策文件为准。

### 3.1 格式与运行时基础

| 文档 | 状态 | 内容 |
| --- | --- | --- |
| [`FORMAT_SPEC.md`](FORMAT_SPEC.md) | 迁移输入 | Game.dat / CommonEvent / .mps / 数据库 / TileSetData / .wolf / .sav 规格 |
| [`SAVE_SPEC.md`](SAVE_SPEC.md) | 迁移输入 | WOLF `.sav` 格式与加解密；用于 C# 导入器 |
| [`PARTY_DB.md`](PARTY_DB.md) | 迁移输入 | CDataBase / SysDatabase / DataBase 运行时规格 |
| [`ENGINE_EVENTS.md`](ENGINE_EVENTS.md) | 迁移输入 | 引擎常驻公共事件调查 |
| [`TIME_SYSTEM.md`](TIME_SYSTEM.md) | 迁移输入 | 游戏内时间/星期/就寝/早朝 |
| [`TEXT_SPEC.md`](TEXT_SPEC.md) | 迁移输入 | 消息文本内联标记与词法契约 |
| [`ASSET_AUDIT.md`](ASSET_AUDIT.md) | 迁移输入 | 素材引用与可达性审计 |
| [`UNVERIFIED.md`](UNVERIFIED.md) | 迁移输入 | 尚未证实的假设；迁移前需逐项处理 |

### 3.2 子系统与 opcode

| 文档 | 状态 | 内容 |
| --- | --- | --- |
| [`OPCODE_SPEC.md`](OPCODE_SPEC.md) | 迁移输入 | 50 种 opcode 总规格 |
| [`OPCODE_101.md`](OPCODE_101.md) | 迁移输入 | 101 消息窗口 |
| [`OPCODE_102.md`](OPCODE_102.md) | 迁移输入 | 102 选择肢窗口 |
| [`OPCODE_112.md`](OPCODE_112.md) | 迁移输入 | 112 字符串条件 |
| [`OPCODE_124.md`](OPCODE_124.md) | 迁移输入 | 124 SetVariableEx |
| [`OPCODE_210_RETURN.md`](OPCODE_210_RETURN.md) | 迁移输入 | 210/300 调用与返回值 |
| [`OPCODE_220_SAVE.md`](OPCODE_220_SAVE.md) | 迁移输入 | 220/221/222 存档三件套 |
| [`OPCODE_250.md`](OPCODE_250.md) | 迁移输入 | 250 数据库读写 |
| [`OPCODE_290.md`](OPCODE_290.md) | 迁移输入 | 290 图片效果 |
| [`BATTLE.md`](BATTLE.md) | 迁移输入 | 战斗子系统全景、事件图、DB 依赖 |
| [`HSCENE.md`](HSCENE.md) | 迁移输入 | H 场景子系统地图 |
| [`MAP_AUDIT.md`](MAP_AUDIT.md) | 迁移输入 | 六张地图、页面激活与点触语义 |

### 3.3 Rust 实现参考（不作为目标架构）

| 文档 | 状态 | 内容 |
| --- | --- | --- |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | 历史实现参考 | Rust 7-crate 架构、wgpu/kira 管线、旧里程碑表 |
| [`AUDIO.md`](AUDIO.md) | 历史实现参考 | Rust 音频后端与接线；Godot 用 `AudioManager` 重建 |
| [`TOOLING.md`](TOOLING.md) | 历史实现参考 | Rust/wgpu/kira 工具链与踩坑 |
| [`BASELINE.json`](BASELINE.json) | 历史基线 | Rust 版本的数据/测试/UI 证据快照 |

## 4. Rust 历史证据（冻结）

以下文档保留原路径，以维持源码注释、旧规格和研究索引的可追溯性。
它们**不是当前任务**，不再继续扩写。

| 文档 | 状态 | 覆盖范围 |
| --- | --- | --- |
| [`HANDOFF_M3.md`](HANDOFF_M3.md) | 历史证据 | M3 完整运行时快照 |
| [`HANDOFF_M4.md`](HANDOFF_M4.md) | 历史证据 | 音频后端 + 接线 |
| [`HANDOFF_M6.md`](HANDOFF_M6.md) | 历史证据 | 主线贯通与日常循环 |
| [`HANDOFF_M7.md`](HANDOFF_M7.md) | 历史证据 | 战斗/H/PRNG/选择窗口 |
| [`HANDOFF_M8.md`](HANDOFF_M8.md) | 历史证据 | 战斗结算、H 前向链、地图角色层 |
| [`HANDOFF_M9.md`](HANDOFF_M9.md) | 历史证据 | 锚点、H 闭环、战斗、地图触发 |
| [`HANDOFF_M10.md`](HANDOFF_M10.md) | 历史证据 | opcode 111、战斗重基线 |
| [`HANDOFF_M11.md`](HANDOFF_M11.md) | 历史证据 | 战斗默认驱动、菜单、H 深链、视觉回归 |
| [`HANDOFF_M12.md`](HANDOFF_M12.md) | 历史证据 | 零售范围、存档/日程/soak |
| [`HANDOFF_M13.md`](HANDOFF_M13.md) | 历史证据 | 内容一致三批、P1-E、打包性能 |
| [`M1_CLOSURE.md`](M1_CLOSURE.md) | 历史证据 | 标题画面像素级闭包 |
| [`M2_SLICE.md`](M2_SLICE.md) | 历史证据 | 开场剧情/消息/选择纵切片 |
| [`research/README.md`](research/README.md) | 历史证据索引 | M6–M13 研究、探针、视觉/音频验收 |

研究文档的状态、测量 HEAD、数据根和复现命令仍按各自文件为准；任何“下一步”均已失效。

## 5. 归档

| 文档 | 内容 |
| --- | --- |
| [`archive/README.md`](archive/README.md) | 归档规则、已移动文件、冻结文档清单 |
| [`archive/PLAN_2026-10-26.md`](archive/PLAN_2026-10-26.md) | 项目开工前可行性研究与 M0–M5 旧规划 |
| [`archive/PLAN_RUST_2026-11-04.md`](archive/PLAN_RUST_2026-11-04.md) | Rust 阶段最终路线图与终止说明 |
| [`archive/rust/`](archive/rust/README.md) | 旧策略、UI backlog、资产清点、旧会话启动与编排记录 |
| [`godot/audit/GEMINI_AUDIT_AND_PLAN_2026-11-14.md`](godot/audit/GEMINI_AUDIT_AND_PLAN_2026-11-14.md) | Gemini 原始审计与规划；含已过时纯 GDScript 草稿，仅保留证据 |

## 6. 用户资料与外部材料（不入库、不作为权威）

| 位置 | 内容 |
| --- | --- |
| [`攻略/`](攻略/) | 玩家攻略、数据定稿、正文；用户资料 |
| [`攻略文档/`](攻略文档/) | 攻略 PDF/TXT；用户资料，禁止随发布分发 |
| [`gpt调研/`](gpt调研/) | 外部 GPT 调研；用户资料 |
| [`vendor/`](vendor/) | WolfDec/WolfTL/WolfSave 等第三方工具；只作参考 |
| [`deepseek 建议.txt`](deepseek%20建议.txt) | 外部实现建议；非权威 |

## 7. 文档维护规则

1. **单一当前入口**：Godot 阶段只看 `docs/godot/README.md`、`DECISIONS.md`、`PLAN.md`。
2. **单一决策源**：任何技术栈、存档、内容范围变化先改 `DECISIONS.md`，再改实现/文档。
3. **单一路线源**：完整路线只在 `godot/PLAN.md`；根 `PLAN.md` 只做摘要。
4. **历史不重写**：旧 HANDOFF、研究、审计保留原貌；发现错误加勘误，不把历史改成当前。
5. **移动必登记**：新增/移动/归档文档必须更新本索引和 `archive/README.md`。
6. **证据优先**：Rust 代码/测试是 Oracle；攻略和外部调研不能覆盖实测。
7. **不入库**：`game_data/`、零售素材、攻略 PDF、`gpt调研/`、`vendor/`、临时截图/日志。
8. **不复活旧路线**：旧 `ENGINE_STRATEGY` 的“不迁移”、旧 Rust `PLAN` 的里程碑和旧 UI backlog 的修复任务均不得作为当前执行依据。
