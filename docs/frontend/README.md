# 前端与展示层架构导航（docs/frontend/）

本目录收归游戏前端交互、Godot 引擎集成与展示层架构的权威技术文档。

---

## 核心前端技术文档

本工程的现行前端完全基于 **Godot 4.7.2 Mono (.NET 8.0 C# + GDScript)** 实现。所有 Godot 重制阶段的定稿决策与实现细节收归在专项目录 [docs/godot/](../godot/README.md) 中：

| 文档 | 定位与内容 |
| :--- | :--- |
| [docs/godot/README.md](../godot/README.md) | **Godot 重制阶段唯一规范入口**、环境基线与维护约定 |
| [docs/godot/DECISIONS.md](../godot/DECISIONS.md) | **已定稿架构决策（D-01..D-53）**：明确采用纯 C# 确定性事件内核 + GDScript 呈现层 |
| [docs/godot/PLAN.md](../godot/PLAN.md) | Godot 阶段 M0–M5 里程碑推进计划与阶段退出标准 |
| [docs/godot/DATA_CONTRACT.md](../godot/DATA_CONTRACT.md) | 离线转换数据契约（`programs.json`、`database.json`、`manifest.json`） |
| [docs/godot/M0_HANDOFF.md](../godot/M0_HANDOFF.md) | M0 验收交接：环境门禁、数据对账与 Oracle 对拍机制 |
| [docs/godot/M1_HANDOFF.md](../godot/M1_HANDOFF.md) | M1 验收交接：切片运行链、存档续跑与视口管理 |
| [docs/godot/M5_HANDOFF.md](../godot/M5_HANDOFF.md) | M5 收口交接：发布包结构、运行时性能与长程 Soak 验证 |
| [docs/godot/RELEASE.md](../godot/RELEASE.md) | 发布与导出说明：构建流程、导出预设与实测记录 |
| [docs/godot/OPCODE_TEST_PLAN.md](../godot/OPCODE_TEST_PLAN.md) | 高频 Opcode 测试与验证用例集合 |
| [docs/godot/LEGACY_RUST_AUDIT.md](../godot/LEGACY_RUST_AUDIT.md) | 22 项旧 Rust 实现缺陷防范与 Godot 交互护栏 |
