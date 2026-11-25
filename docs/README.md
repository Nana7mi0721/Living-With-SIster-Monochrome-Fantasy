# 文档总索引导航中心（docs/README.md）

本文件是《妹相随～黑白世界的缤纷冒险～》重制版工程的**唯一全局文档导航中心**。所有设计规范、技术决策、Opcode 契约与审查报告按模块结构化归档：

```mermaid
graph LR
    Root["docs/"] --> Core["核心指引与基线"]
    Root --> Plans["plans/ (顶层架构与路线)"]
    Root --> Godot["godot/ (Godot 客户端与呈现层)"]
    Root --> Engine["engine/ (核心引擎与Opcode)"]
    Root --> Reviews["reviews/ (缺陷审查与台账)"]
    Root --> Milestones["milestones/ (M1-M13交付记录)"]
    Root --> Release["release/ (资产审计与打包)"]
    Root --> Guide["guide/ (游戏攻略与百科)"]
    Root --> Research["research/ (研究底稿与实验)"]
    Root --> Archive["archive/ (历史归档区)"]
```

---

## 0. 状态约定

| 状态标签 | 含义说明 |
| :--- | :--- |
| **现行 · 先读** | 现行且必须优先阅读的核心规范或入口文档 |
| **现行 · 权威** | 核心架构与逻辑事实来源，结论与数据可直接引用 |
| **现行 · 记录** | 现行的任务进度台账、缺陷排查或执行日志 |
| **历史证据** | 研发攻坚阶段的历史备忘与推导过程（以现行规范为准） |
| **已归档** | 已被新设计或总报告取代，保留作为历史证据链 |

---

## 1. 核心交接与现行规范（先读）

| 文档 | 状态 | 内容与定位 |
| :--- | :---: | :--- |
| [godot/README.md](godot/README.md) | **现行 · 先读** | **Godot 重制阶段唯一规范入口**：环境基线、当前完成状态与维护约定 |
| [godot/DECISIONS.md](godot/DECISIONS.md) | **现行 · 权威** | **定稿架构决策（D-01..D-90）**：确定采用纯 C# 确定性事件内核 + GDScript 呈现层 |
| [godot/PLAN.md](godot/PLAN.md) | **现行 · 权威** | Godot 阶段总体演进路线与 M0–M5 各阶段退出标准 |
| [BASELINE.json](BASELINE.json) | **现行 · 权威** | 自动化测试与一致性校验基线（包含 Game.dat 字段、公共事件、地图事件统计） |
| [ARCHITECTURE.md](ARCHITECTURE.md) | **现行 · 权威** | 全局架构设计方案（数据转换管线、运行时状态机、渲染层级） |
| [../AGENTS.md](../AGENTS.md) | **现行 · 必读** | 自动化开发与协作约定（写作用域隔离、测试门限要求、内容安全） |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | 现行 · 指南 | 开发者贡献指南（环境准备、源码构建、测试流程与规范约定） |
| [../CHANGELOG.md](../CHANGELOG.md) | 现行 · 历史 | 版本发布与变更日志（语义化版本控制） |
| [../DISCLAIMER.md](../DISCLAIMER.md) | 现行 · 法律 | 原作知识产权与非商业研究免责声明、纯代码与商业资产隔离原则 |

---

## 2. 顶层规划与演进方案（[plans/](plans/README.md)）

存放顶层演进路线图与重大架构重构方案：

- [plans/README.md](plans/README.md) — 方案目录索引
- [plans/PLAN.md](plans/PLAN.md) — **工程总体路线图**：涵盖 Rust Oracle 验证期至纯 Godot Mono 全流程里程碑划分与验收标准

---

## 3. Godot 运行时与呈现层架构（[godot/](godot/README.md)）

Godot 4.7.2 Mono 架构、视口呈现与运行时交互规范：

- [godot/README.md](godot/README.md) — Godot 重制阶段权威入口与环境基线
- [godot/DECISIONS.md](godot/DECISIONS.md) — 拍板决策集合（D-01..D-90：确定采用纯 C# 确定性事件内核 + GDScript 呈现层）
- [godot/PLAN.md](godot/PLAN.md) — Godot 阶段总体演进路线与 M0–M5 各阶段退出标准
- [godot/DATA_CONTRACT.md](godot/DATA_CONTRACT.md) — 离线转换数据契约（`programs.json`、`database.json`、`manifest.json`）
- [godot/M0_HANDOFF.md](godot/M0_HANDOFF.md) — M0 验收交接：环境门限、数据对账与 Oracle 对拍机制
- [godot/M1_HANDOFF.md](godot/M1_HANDOFF.md) — M1 验收交接：切片运行链、存读档续跑与视口管理
- [godot/M5_HANDOFF.md](godot/M5_HANDOFF.md) — M5 收口交接：发布包结构、运行时性能与长程 Soak 验证
- [godot/RELEASE.md](godot/RELEASE.md) — 发布与导出说明：构建流程、导出预设与实测记录
- [godot/HOTKEYS.md](godot/HOTKEYS.md) — Windows 桌面版快捷键、窗口倍增、全屏与帧率切换说明
- [godot/LEGACY_RUST_AUDIT.md](godot/LEGACY_RUST_AUDIT.md) — 22 项旧 Rust 实现缺陷防范与 Godot 交互护栏
- [godot/UI_UX_GROUND_TRUTH_SPEC.md](godot/UI_UX_GROUND_TRUTH_SPEC.md) — UI/UX/动效真相规格书：系统字体/窗口/光标/图层/转场硬证据与表现层审计
- [godot/PRE_RELEASE_ORCHESTRATOR_AUDIT.md](godot/PRE_RELEASE_ORCHESTRATOR_AUDIT.md) — **历史证据**：基线 `9f36abb` 的发版前审计；部分问题已被后续决策与门禁关闭，现状以最新验证报告与 `DECISIONS.md` 为准

---

## 4. 核心引擎规范与行为契约（[engine/](engine/README.md)）

收拢底层字节码指令集（Opcode）、二进制格式与游戏子系统权威契约：

- [engine/README.md](engine/README.md) — 核心引擎规范导航索引
- [engine/OPCODE_SPEC.md](engine/OPCODE_SPEC.md) — **WOLF RPG 完整指令集架构总览**
- [engine/FORMAT_SPEC.md](engine/FORMAT_SPEC.md) — `Game.dat`、数据表与 DXArchive 二进制打包格式全景
- [engine/SAVE_SPEC.md](engine/SAVE_SPEC.md) — `.sav` 文件字节结构、变量库哈希、双向无损往返契约
- [engine/TEXT_SPEC.md](engine/TEXT_SPEC.md) — 文本转义标记与展开器行为规范
- [engine/ENGINE_EVENTS.md](engine/ENGINE_EVENTS.md) — 事件调度状态机与并发队列规范
- [engine/TIME_SYSTEM.md](engine/TIME_SYSTEM.md) — 游戏内时间流转与日程作息系统
- [engine/BATTLE.md](engine/BATTLE.md) — 回合制战斗状态机与数值结算规范
- [engine/PARTY_DB.md](engine/PARTY_DB.md) — 角色属性、成长与队伍数据库规范
- [engine/AUDIO.md](engine/AUDIO.md) — BGM/BGS/SE 三轨独立混音与生命周期管理
- [engine/HSCENE.md](engine/HSCENE.md) — 特殊场景多层立绘差分叠加与状态流转契约
- [engine/KEY_INPUT_SPEC.md](engine/KEY_INPUT_SPEC.md) — 按键掩码与硬件输入映射核查

---

## 5. 代码审查与缺陷台账（[reviews/](reviews/README.md)）

系统性代码审查（Code Review）、跨仓库审计与缺陷追踪记录：

- [reviews/README.md](reviews/README.md) — 代码审查台账总览索引
- [reviews/full_project_audit_2026_11_25.md](reviews/full_project_audit_2026_11_25.md) — **现行·闭环**：Godot Debug 修复、`12004`/`12005` 存档翻页箭头与全项目审计闭环报告（见 D-93）
- [reviews/deep_audit_8e0a98d1_remediation_2026_11_24.md](reviews/deep_audit_8e0a98d1_remediation_2026_11_24.md) — **现行·闭环**：`8e0a98d1` 三端深度审查 5 P0 / 12 P1–P3 与 3 项专项修复闭环报告（见 D-88..D-90）
- [reviews/deep_audit_8e0a98d1_and_arch_fix_plan_2026_11_24.md](reviews/deep_audit_8e0a98d1_and_arch_fix_plan_2026_11_24.md) — **现行·台账**：`8e0a98d1` 三端全面静态代码审查与架构级修复方案（5 P0 / 12 P1–P3 / 专项 1–3）
- [reviews/review_614c6cb_and_perf_size_plan_2026_11_23.md](reviews/review_614c6cb_and_perf_size_plan_2026_11_23.md) — **现行·规划**：Commit `614c6cb` 审阅与发包体积/性能优化规划（见 D-85）
- [reviews/lwsmf_perf_and_multiplatform_size_guide.md](reviews/lwsmf_perf_and_multiplatform_size_guide.md) — **现行·指南**：全平台运行时性能与多端素材包体压缩指南
- [reviews/lwsmf_bgm_source_audit_2026_11_23.md](reviews/lwsmf_bgm_source_audit_2026_11_23.md) — **现行·专题**：已解包媒体库音源事实核查与替换可行性审查（源站上限低于零售版码率、仅 1 例确认可升级）
- [reviews/review_030ee81f_remediation_2026_11_23.md](reviews/review_030ee81f_remediation_2026_11_23.md) — **现行·闭环**：`030ee81f` 审阅 UX-1~UX-5 与 REV-1~REV-6 修复闭环报告（见 D-86）
- [reviews/review_030ee81f_and_ux_fix_plan_2026_11_23.md](reviews/review_030ee81f_and_ux_fix_plan_2026_11_23.md) — **现行·台账**：`030ee81f` 审阅缺陷与全端 / 移动端 UI·UX 修复方案
- [reviews/save_ui_font_and_gauge_remediation_2026_11_23.md](reviews/save_ui_font_and_gauge_remediation_2026_11_23.md) — **现行·闭环**：S-01..S-08 存档 UI、模态输入、自动存档、HiDPI 字体与进度条对齐修复闭环报告（见 D-84）
- [reviews/save_ui_font_and_gauge_diagnosis_2026_11_23.md](reviews/save_ui_font_and_gauge_diagnosis_2026_11_23.md) — **现行·台账**：Commit `78c4e49a` 存档界面异常、自动存档、字体清晰度与格子进度条错位根因排查
- [reviews/acceptance_audit_89fc8f5_remediation_2026_11_23.md](reviews/acceptance_audit_89fc8f5_remediation_2026_11_23.md) — **现行·闭环**：`89fc8f5` 验收审查全部增量项收口、P1-6/P2-11 保留设计依据与双模式门禁验证
- [reviews/acceptance_audit_89fc8f5_2026_11_23.md](reviews/acceptance_audit_89fc8f5_2026_11_23.md) — **现行·台账**：Commit `89fc8f5` 整改验收与增量回归审查（6 P1 / 14 P2 / 12 P3）
- [reviews/input_consumption_and_full_audit_3a89a8a.md](reviews/input_consumption_and_full_audit_3a89a8a.md) — **现行·闭环**：触控双击取消判定与全量审查闭环
- [reviews/sister_ux_multiplatform_adaptation_2026_11_22.md](reviews/sister_ux_multiplatform_adaptation_2026_11_22.md) — **现行·闭环**：姊妹作 UX 优化、移动端零拷贝/触控/返回键与跨端存档互导落地报告（见 D-83）
- [reviews/full_static_audit_59268ac_remediation_2026_11_22.md](reviews/full_static_audit_59268ac_remediation_2026_11_22.md) — **现行·闭环**：`59268ac` 全量静态审查 67 项缺陷修复与双模式门禁验证（见 D-82）
- [reviews/full_static_audit_59268ac_2026_11_22.md](reviews/full_static_audit_59268ac_2026_11_22.md) — **现行·台账**：Commit `59268ac` 全仓库静态代码审查（4 P0 / 24 P1 / 26 P2 / 13 P3）
- [reviews/pipeline_completion_and_next_processes_2026_11_22.md](reviews/pipeline_completion_and_next_processes_2026_11_22.md) — **现行·闭环**：8 条管线验收总评、`RES-01..03` 缺陷收口与 5 大防回归工程流程落地（见 D-74）
- [reviews/pipeline_overhaul_work_order_2026_11_22.md](reviews/pipeline_overhaul_work_order_2026_11_22.md) — **现行·施工单**：8 条底层管线分阶段排雷施工单（见 D-73）
- [reviews/live_play_root_cause_diagnosis_2026_11_22.md](reviews/live_play_root_cause_diagnosis_2026_11_22.md) — **现行·闭环**：实机体验 7 项核心缺陷根因定位、修复与 OpenGL3 实测验证（见 D-72 / D-73）
- [reviews/consolidated_audit_report_2026_11_21.md](reviews/consolidated_audit_report_2026_11_21.md) — **现行·闭环**：`c61d762` 统一核验、P1/P2 深度摸排与 CI 故障根因诊断
- [reviews/lwsmf_8fa1189_audit_remediation_2026_11_21.md](reviews/lwsmf_8fa1189_audit_remediation_2026_11_21.md) — **现行·闭环**：两份 `8fa1189` 游玩/UI 审计的逐项修复、语义结论与回归映射
- [reviews/release_gate_verification_2026_11_21.md](reviews/release_gate_verification_2026_11_21.md) — **现行·验证**：基线 `8fa1189` 的 Release 构建与无头 38 项门禁实测证据
- [reviews/deepseek_retail_mechanism_audit_2026_11_20.md](reviews/deepseek_retail_mechanism_audit_2026_11_20.md) — **现行·专题**：标题“开始/继续”、全新封面分支、Map003 时序与 Lolita 字体回退综合核查
- [reviews/deepseek_title_newgame_interaction_2026_11_20.md](reviews/deepseek_title_newgame_interaction_2026_11_20.md) — 标题菜单/新游戏入口/原版点击行为分项报告
- [reviews/deepseek_title_graphics_fresh_state_2026_11_20.md](reviews/deepseek_title_graphics_fresh_state_2026_11_20.md) — 全新状态封面与 CE42/Map003 选图分项报告
- [reviews/deepseek_font_fallback_chain_2026_11_20.md](reviews/deepseek_font_fallback_chain_2026_11_20.md) — DxLib 字体绑定与缺字回退分项报告
- [reviews/bug_inspection_report_2026_11_20.md](reviews/bug_inspection_report_2026_11_20.md) — **历史·重点**：右键点击失效、素材缺失、文本解析内部代号泄露深度审查报告
- [reviews/bug_fix_and_gate_opt_2026_11_20.md](reviews/bug_fix_and_gate_opt_2026_11_20.md) — **现行·记录**：输入/文本/VM 修复、Oracle 对齐与 38 项门禁并行加速总结
- [reviews/m4_oracle_refresh_2026_11_21.md](reviews/m4_oracle_refresh_2026_11_21.md) — **现行·记录**：M4 Oracle 夹具刷新、刷新脚本修复与双模式门禁恢复
- [reviews/m5_round12_review_2026_11_19.md](reviews/m5_round12_review_2026_11_19.md) — M5 发布前夕全景审查与收口验证
- [reviews/m4_round11_review_2026_11_18.md](reviews/m4_round11_review_2026_11_18.md) — M4 对齐 Oracle 与长程 Soak 稳定性审查
- [reviews/m2w4_round5_repo_review_2026_11_15.md](reviews/m2w4_round5_repo_review_2026_11_15.md) — M2W4 仓库代码整洁度与模块依赖审计
- [reviews/imoto_life_title_and_layout_audit_report.md](reviews/imoto_life_title_and_layout_audit_report.md) — 跨仓库标题与布局像素对齐审计

---

## 6. 里程碑研发交付记录（[milestones/](milestones/README.md)）

攻坚期各里程碑阶段的交付记录与验收证据：

- [milestones/README.md](milestones/README.md) — 里程碑记录总览
- [milestones/M1_CLOSURE.md](milestones/M1_CLOSURE.md) — M1 启动切片收口
- [milestones/M2_SLICE.md](milestones/M2_SLICE.md) — M2 日常循环切片
- [milestones/HANDOFF_M3.md](milestones/HANDOFF_M3.md) 至 [milestones/HANDOFF_M13.md](milestones/HANDOFF_M13.md) — M3 到 M13 各阶段完整运行时交付记录

---

## 7. 资产审计与发布工具链（[release/](release/README.md)）

- [release/README.md](release/README.md) — 资产与工具链导航
- [release/ASSET_AUDIT.md](release/ASSET_AUDIT.md) — 游戏原始素材全景审计与解包规则
- [release/MAP_AUDIT.md](release/MAP_AUDIT.md) — 地图图块与事件热区审计
- [release/TOOLING.md](release/TOOLING.md) — 编译构建、离线转换与自动化脚本设计
- [release/MOBILE_PACKAGING.md](release/MOBILE_PACKAGING.md) — Android / iOS 发包工具链、导出预设、环境体检与 CI 注入指南

---

## 8. 攻略与逆向研究区（[guide/](guide/README.md) & [research/](research/README.md)）

- [guide/README.md](guide/README.md) — 游戏攻略与百科全书索引
- [research/README.md](research/README.md) — 逆向实验、前置调研与技术底稿（含 50 余份 M6–M13 研究记录）
- [archive/README.md](archive/README.md) — 历史归档区（旧 Rust 规划与早期设计）
- [vendor/](vendor/) — 第三方分析与逆向工具包（WolfDec, WolfSave, WolfTL）
