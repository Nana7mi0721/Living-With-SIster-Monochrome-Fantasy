# 代码审查与缺陷台账索引（docs/reviews/）

本目录收归工程演进过程中各阶段的系统性代码审查（Code Review）、跨仓库审计（Cross-repo Audit）以及缺陷排查与修复报告。

---

## 1. 现行缺陷排查与审查报告

| 文档 | 审查日期 | 范围与主题 | 关键结论与状态 |
| :--- | :---: | :--- | :--- |
| [acceptance_audit_89fc8f5_remediation_2026_11_23.md](acceptance_audit_89fc8f5_remediation_2026_11_23.md) | 2026-11-23 | **`89fc8f5` 验收审查问题收口与回归验证报告** | 完成 6 项新增 P1、14 项 P2、12 项 P3 与残余项修复，记录 P1-6/P2-11 保留原设计依据，双模式门禁与 Python/Rust 单测全绿 |
| [acceptance_audit_89fc8f5_2026_11_23.md](acceptance_audit_89fc8f5_2026_11_23.md) | 2026-11-23 | **Commit `89fc8f5` 整改验收与增量回归审查报告** | 复核 67 项基线（59 全通过 / 7 部分 / 1 未通过）并提出 32 项增量改进清单 |
| [sister_ux_multiplatform_adaptation_2026_11_22.md](sister_ux_multiplatform_adaptation_2026_11_22.md) | 2026-11-22 | **姊妹作交互优化与多平台适配落地报告** | 落地移动端零拷贝与分级缓存、iOS 路径/退出合规、触控手势与 7 级返回、存单确认/跨端 `.zip`/`.sav` 互导与 PE 中文本地化，双模式门禁全绿（见 `D-83`） |
| [full_static_audit_59268ac_remediation_2026_11_22.md](full_static_audit_59268ac_remediation_2026_11_22.md) | 2026-11-22 | **`59268ac` 全量静态审查 67 项缺陷修复闭环报告** | 4 P0 / 24 P1 / 26 P2 / 13 P3 收口，无头 38 项与 OpenGL3 双门禁全绿（见 `D-82`） |
| [full_static_audit_59268ac_2026_11_22.md](full_static_audit_59268ac_2026_11_22.md) | 2026-11-22 | **Commit `59268ac` 全仓库静态代码审查报告** | 识别 4 P0 / 24 P1 / 26 P2 / 13 P3 缺陷与 D-01～D-08 残余缺口 |
| [consolidated_audit_report_2026_11_21.md](consolidated_audit_report_2026_11_21.md) | 2026-11-21 | **`c61d762` 统一核验、深度摸排与 CI 诊断** | 核验 `c61d762` 落地情况，定位并收口 1 项 P1 串态 + 7 项 P2 缺陷以及 Rust `fmt`/CI 问题（详见 `D-71`） |
| [lwsmf_8fa1189_audit_remediation_2026_11_21.md](lwsmf_8fa1189_audit_remediation_2026_11_21.md) | 2026-11-21 | **两份 `8fa1189` 游玩/UI 审计的逐项修复闭环** | P0/P1/P2 均已落到代码、数据语义或测试；`101+102` 同屏、`124` 缩放字段、Tips 点击穿透与 13 张素材核对完成 |
| [release_gate_verification_2026_11_21.md](release_gate_verification_2026_11_21.md) | 2026-11-21 | **基线 `8fa1189` 的修复与无头 38 项门禁验证** | Release 构建 0 警告/0 错误；无头门禁 38/38；418 夹具 2461/2461；主路线 149655/149655；M4 989439/989439；本轮未执行 OpenGL3 渲染门禁 |
| [m4_oracle_refresh_2026_11_21.md](m4_oracle_refresh_2026_11_21.md) | 2026-11-21 | **M4 Oracle 夹具刷新（历史快照）** | 第 362832 步分歧来自早于 op121 语义修复的旧夹具；该次刷新记录为 990102 步 100% 一致。当前 `124` 语义修复后的计数以 [release_gate_verification_2026_11_21.md](release_gate_verification_2026_11_21.md) 为准 |
| [deepseek_retail_mechanism_audit_2026_11_20.md](deepseek_retail_mechanism_audit_2026_11_20.md) | 2026-11-20 | **标题机制、全新封面、字体回退综合核查** | “开始咯”不调用 CE14；全新三段封面分别为 `title01.jpg` / `TitleGraphic.jpg` / `TitleGraphic.png`；Lolita 完整覆盖出货文本，正常文本不触发中文回退 |
| [deepseek_title_newgame_interaction_2026_11_20.md](deepseek_title_newgame_interaction_2026_11_20.md) | 2026-11-20 | **CE42/CE43 点击链路与 Map003 时序** | CE42 新游戏分支无存档钩子；CE43 仅绘制/高亮；Map003 两段 `180[50]` 是数据脚本节奏 |
| [deepseek_title_graphics_fresh_state_2026_11_20.md](deepseek_title_graphics_fresh_state_2026_11_20.md) | 2026-11-20 | **全新 V300/V499/V503/V550 分支与封面** | TitleMap 显示 `title01`，CE42 菜单显示 `TitleGraphic.jpg`，Map003 过渡显示 `TitleGraphic.png` |
| [deepseek_font_fallback_chain_2026_11_20.md](deepseek_font_fallback_chain_2026_11_20.md) | 2026-11-20 | **Lolita/DxLib/GDI 字体回退链** | 四个字体槽均为 `LOLITA`；出货文本与 GB2312 均无缺字；人为缺字为 tofu，无配置中文替代字体 |
| [bug_inspection_report_2026_11_20.md](bug_inspection_report_2026_11_20.md) | 2026-11-20 | **右键点击失效、游戏素材缺失、文本解析内部代号泄露** | 定位 InputRouter 掩码与锁存缺陷、media.pak 错用前作素材导致 97.4% 缺失、ChoiceWindow 裸绘与 TextExpander 补 0 等 8 项文本缺陷 |
| [bug_fix_and_gate_opt_2026_11_20.md](bug_fix_and_gate_opt_2026_11_20.md) | 2026-11-20 | **输入/文本/VM 修复、Oracle 同步与门禁并行加速** | 修复 GameRoot 鼠标拦截、Opcode 122/140 与会话残留状态，重生成 3 份 Oracle 夹具并通过 8 并发门禁优化将全量门禁缩短至 251s（`GODOT_GATE_OK`） |

---

## 2. 里程碑阶段性代码审查报告

| 文档 | 对应里程碑/轮次 | 主题与核心审查内容 |
| :--- | :---: | :--- |
| [m5_round12_review_2026_11_19.md](m5_round12_review_2026_11_19.md) | M5 · Round 12 | 发布前夕全景审查、转换引导与运行时收口验证 |
| [m4_round11_review_2026_11_18.md](m4_round11_review_2026_11_18.md) | M4 · Round 11 | 对齐 Oracle、长程 Soak 稳定性与主循环回归 |
| [m2w4_round5_repo_review_2026_11_15.md](m2w4_round5_repo_review_2026_11_15.md) | M2W4 · Round 5 | 仓库级代码整洁度、跨模块依赖与资产清单审计 |
| [m2w4_round4_review_2026_11_15.md](m2w4_round4_review_2026_11_15.md) | M2W4 · Round 4 | 状态机流转、事件调度与变量库无损往返深度比对 |
| [m2w2_round3_review_2026_11_14.md](m2w2_round3_review_2026_11_14.md) | M2W2 · Round 3 | 日常日程闭环、NPC 移动交互与地图切换 |
| [m1_auditfix_round2_review_2026_11_14.md](m1_auditfix_round2_review_2026_11_14.md) | M1 · Round 2 | M1 启动切片缺陷修复专项回归 |
| [m1_audit_and_m2_plan_2026_11_14.md](m1_audit_and_m2_plan_2026_11_14.md) | M1 / M2 | M1 阶段收口审计与 M2 推进规划 |

---

## 3. 跨仓库对照与体验审计

| 文档 | 对照标的 | 核心内容 |
| :--- | :---: | :--- |
| [imoto_life_title_and_layout_audit_report.md](imoto_life_title_and_layout_audit_report.md) | 姊妹作《妹！生活》 | 标题界面、窗口布局、字体排版像素对齐审计 |
| [imoto_life_cross_repo_audit_checklist.md](imoto_life_cross_repo_audit_checklist.md) | 跨工程协同 | 核心架构、字节码解析器、输入层设计清单对比 |
| [imoto_life_c553d04_review_and_roadmap.md](imoto_life_c553d04_review_and_roadmap.md) | 提交 c553d04 对齐 | 跨项目路线图对齐与设计借鉴 |

---

## 4. 历史保留

| 文档 | 状态 | 说明 |
| :--- | :---: | :--- |
| [lwsmf_orchestrator_pre_release_tasklist.md](lwsmf_orchestrator_pre_release_tasklist.md) | 已取代 | 仅保留为 [PRE_RELEASE_ORCHESTRATOR_AUDIT.md](../godot/PRE_RELEASE_ORCHESTRATOR_AUDIT.md) 的任务清单出处；不再作为执行依据 |
