# docs/archive — 历史归档

> 状态：**归档**
>
> 本目录保存不再作为当前执行依据的旧路线图、旧策略和旧入口。
> 当前 Godot 入口：[`../godot/README.md`](../godot/README.md)。

## 1. 已归档文件

| 文件 | 原位置 | 归档原因 |
| --- | --- | --- |
| [`PLAN_2026-10-26.md`](PLAN_2026-10-26.md) | `docs/archive/` | 项目开工前可行性研究与 M0–M5 旧规划 |
| [`PLAN_RUST_2026-11-04.md`](PLAN_RUST_2026-11-04.md) | 根 `PLAN.md` | Rust 阶段最终路线图与项目终止说明 |
| [`rust/ENGINE_STRATEGY_2026-11-04.md`](rust/ENGINE_STRATEGY_2026-11-04.md) | `docs/ENGINE_STRATEGY.md` | “暂不迁 Godot”的旧决策，已被 2026-11-14 定稿覆盖 |
| [`rust/UI_BUG_BACKLOG_2026-11-04.md`](rust/UI_BUG_BACKLOG_2026-11-04.md) | `docs/UI_BUG_BACKLOG.md` | 旧 Rust UI 修复清单；现在只作迁移风险证据 |
| [`rust/PROJECT_ASSET_INVENTORY_2026-11-04.md`](rust/PROJECT_ASSET_INVENTORY_2026-11-04.md) | `docs/PROJECT_ASSET_INVENTORY.md` | 旧资产/工作树清点 |
| [`rust/AGENT_ORCHESTRATION_UI_2026-11-04.md`](rust/AGENT_ORCHESTRATION_UI_2026-11-04.md) | `docs/AGENT_ORCHESTRATION_UI_2026-11-04.md` | 旧 Rust UI 子代理编排 |
| [`rust/NEW_SESSION_START_RUST_2026-11-04.md`](rust/NEW_SESSION_START_RUST_2026-11-04.md) | `docs/NEW_SESSION_START.md` | 旧 Rust 会话启动提示 |

## 2. 冻结但未移动的文件

这些文档仍保留原路径，因为源码注释、规格或研究索引直接引用它们：

- `docs/HANDOFF_M3..HANDOFF_M13.md`
- `docs/M1_CLOSURE.md`、`docs/M2_SLICE.md`
- `docs/research/` 全部研究底稿
- `docs/ARCHITECTURE.md`、`docs/AUDIO.md`、`docs/TOOLING.md`
- `docs/BASELINE.json`

它们的状态已在 [`../README.md`](../README.md) 标为“历史证据”或“历史实现参考”。
继续保留路径不等于继续维护；不要在这些文件里追加新路线或新“下一步”。

## 3. 使用规则

1. 归档文档中的“下一步”“当前建议”“路线图”均已失效。
2. 归档文档可作事实证据，但必须先回到当前源码/测试或 [`../godot/LEGACY_RUST_AUDIT.md`](../godot/LEGACY_RUST_AUDIT.md) 复核。
3. 原始审计报告中的旧纯 GDScript 段落也属于草稿；以 [`../godot/DECISIONS.md`](../godot/DECISIONS.md) 为准。
4. 不删除归档文档，不做“追认式重写”；需要修正时加勘误说明。
