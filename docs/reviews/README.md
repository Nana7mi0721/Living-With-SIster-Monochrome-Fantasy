# 代码审查与缺陷台账索引（docs/reviews/）

本目录收归工程演进过程中各阶段的系统性代码审查（Code Review）、跨仓库审计（Cross-repo Audit）以及缺陷排查与修复报告。

---

## 1. 现行缺陷排查、性能与审查报告

| 文档 | 审查日期 | 范围与主题 | 关键结论与状态 |
| :--- | :---: | :--- | :--- |
| [review_614c6cb_and_perf_size_plan_2026_11_23.md](review_614c6cb_and_perf_size_plan_2026_11_23.md) | 2026-11-23 | **Commit `614c6cb` 审阅与发包体积/性能优化规划** | 制定 Android/iOS 裁剪、Brotli 压缩与 Mono 静态库剥离方案，推进移动端包体精简（见 D-85） |
| [lwsmf_perf_and_multiplatform_size_guide.md](lwsmf_perf_and_multiplatform_size_guide.md) | 2026-11-23 | **全平台运行时性能与素材包体压缩指南** | 统一桌面与移动端内存驻留预算、纹理压缩格式与无损音频存储规范 |
| [lwsmf_bgm_source_audit_2026_11_23.md](lwsmf_bgm_source_audit_2026_11_23.md) | 2026-11-23 | **已解包媒体库 183 个音频文件的音源事实核查与替换可行性审查** | 源站上限 320 kbps MP3 低于零售版多数曲目的 380–465 kbps Vorbis，**盲目替换反而降级**；仅 `BGM/yuuyake.mp3` 一例确认可升级；原检索指南勘误 |
| [review_030ee81f_remediation_2026_11_23.md](review_030ee81f_remediation_2026_11_23.md) | 2026-11-23 | **`030ee81f` 审阅缺陷与全端 UX/资产一致性修复闭环报告** | 完成 UX-1~UX-5 与 REV-1~REV-6 闭环，锁定并修复 `SystemFile/charicon.png` 剪枝导致的存档 SD 变形，双模式门禁与单测全绿（见 D-86） |
| [review_030ee81f_and_ux_fix_plan_2026_11_23.md](review_030ee81f_and_ux_fix_plan_2026_11_23.md) | 2026-11-23 | **LWSMF Remake（`030ee81f`）审阅缺陷与全端 / 移动端 UI·UX 修复方案** | 汇总选项框居中、菜单按钮、双击空白取消、移动端 SD 变形排查、跨端资产一致性及 REV-1~REV-6 修复方案 |
| [save_ui_font_and_gauge_remediation_2026_11_23.md](save_ui_font_and_gauge_remediation_2026_11_23.md) | 2026-11-23 | **存档界面 `CSelf600`、弹窗重构、自动存档、字体与进度条对齐修复报告** | 闭环修复 S-01..S-08 全部 8 项问题，新增 C#/GDScript 回归单测并通过双模式门禁（见 D-84） |
| [save_ui_font_and_gauge_diagnosis_2026_11_23.md](save_ui_font_and_gauge_diagnosis_2026_11_23.md) | 2026-11-23 | **Commit `78c4e49a` 存档界面异常、自动存档、字体清晰度与格子进度条错位根因报告** | 定位 `CSelf600` 偏移、1 帧脉冲打空、`Esc` 穿透、`Phase` 守卫、HiDPI 字体与 `■`/`□` 左漂 8 项根因 |
| [acceptance_audit_89fc8f5_remediation_2026_11_23.md](acceptance_audit_89fc8f5_remediation_2026_11_23.md) | 2026-11-23 | **`89fc8f5` 验收审查问题收口与回归验证报告** | 完成 6 项新增 P1、14 项 P2、12 项 P3 与残余项修复，记录 P1-6/P2-11 保留原设计依据，双模式门禁全绿 |
| [acceptance_audit_89fc8f5_2026_11_23.md](acceptance_audit_89fc8f5_2026_11_23.md) | 2026-11-23 | **Commit `89fc8f5` 整改验收与增量回归审查报告** | 复核 67 项基线（59 全通过 / 7 部分 / 1 未通过）并提出 32 项增量改进清单 |
| [input_consumption_and_full_audit_3a89a8a.md](input_consumption_and_full_audit_3a89a8a.md) | 2026-11-22 | **Commit `3a89a8a` 输入判定、触控取消与全量审查** | 修复非选项区域触控双击取消判定，消除误消费与无操作吞噬 |
| [deadlock_and_stall_audit_2026_11_22.md](deadlock_and_stall_audit_2026_11_22.md) | 2026-11-22 | **死锁与执行挂起深度排查报告** | 审查事件并行队列死锁与信号锁死风险，完善看门狗与超时自愈机制 |
| [hd_upscale_pilot_2026_11_22.md](hd_upscale_pilot_2026_11_22.md) | 2026-11-22 | **HD 高清素材放大重采样实验报告** | 评估 2x/4x 神经超分与双三次滤波对像素立绘的呈现保真度 |
| [sister_ux_multiplatform_adaptation_2026_11_22.md](sister_ux_multiplatform_adaptation_2026_11_22.md) | 2026-11-22 | **姊妹作交互优化与多平台适配落地报告** | 落地移动端零拷贝与分级缓存、iOS 路径/退出合规、触控手势与 7 级返回、跨端互导与 PE 本地化（见 D-83） |
| [full_static_audit_59268ac_remediation_2026_11_22.md](full_static_audit_59268ac_remediation_2026_11_22.md) | 2026-11-22 | **`59268ac` 全量静态审查 67 项缺陷修复闭环报告** | 4 P0 / 24 P1 / 26 P2 / 13 P3 收口，无头 38 项与 OpenGL3 双门禁全绿（见 D-82） |
| [full_static_audit_59268ac_2026_11_22.md](full_static_audit_59268ac_2026_11_22.md) | 2026-11-22 | **Commit `59268ac` 全仓库静态代码审查报告** | 识别 4 P0 / 24 P1 / 26 P2 / 13 P3 缺陷与 D-01～D-08 残余缺口 |
| [pipeline_completion_and_next_processes_2026_11_22.md](pipeline_completion_and_next_processes_2026_11_22.md) | 2026-11-22 | **8 条底层管线验收总评与防回归工程流程落地** | 闭环验收 RES-01..03，建立跨平台测试与持续集成防退化规范（见 D-74） |
| [pipeline_overhaul_work_order_2026_11_22.md](pipeline_overhaul_work_order_2026_11_22.md) | 2026-11-22 | **8 条底层管线分阶段排雷施工单** | 规划音频、输入、图层与存档等 8 条核心管道的系统性重构步骤（见 D-73） |
| [live_play_root_cause_diagnosis_2026_11_22.md](live_play_root_cause_diagnosis_2026_11_22.md) | 2026-11-22 | **实机体验 7 项核心缺陷根因定位与修复** | 修复转场穿透、文本延迟、图层撕裂与按键冲突，通过 OpenGL3 实测（见 D-72 / D-73） |
| [consolidated_audit_report_2026_11_21.md](consolidated_audit_report_2026_11_21.md) | 2026-11-21 | **`c61d762` 统一核验、深度摸排与 CI 诊断** | 核验 `c61d762` 落地情况，定位并收口 1 项 P1 串态 + 7 项 P2 缺陷以及 Rust `fmt`/CI 问题（详见 D-71） |
| [lwsmf_8fa1189_audit_remediation_2026_11_21.md](lwsmf_8fa1189_audit_remediation_2026_11_21.md) | 2026-11-21 | **两份 `8fa1189` 游玩/UI 审计的逐项修复闭环** | P0/P1/P2 均已落到代码、数据语义或测试；`101+102` 同屏、`124` 缩放字段、Tips 点击穿透核对完成 |
| [release_gate_verification_2026_11_21.md](release_gate_verification_2026_11_21.md) | 2026-11-21 | **基线 `8fa1189` 的修复与无头 38 项门禁验证** | Release 构建 0 警告/0 错误；无头门禁 38/38；418 夹具 2461/2461；主路线 149655/149655 |
| [m4_oracle_refresh_2026_11_21.md](m4_oracle_refresh_2026_11_21.md) | 2026-11-21 | **M4 Oracle 夹具刷新（历史快照）** | 记录 990,102 步 Oracle trace 刷新全量匹配 |
| [deepseek_retail_mechanism_audit_2026_11_20.md](deepseek_retail_mechanism_audit_2026_11_20.md) | 2026-11-20 | **标题机制、全新封面、字体回退综合核查** | “开始咯”不调用 CE14；全新三段封面分别为 `title01` / `TitleGraphic`；Lolita 覆盖出货文本 |
| [deepseek_title_newgame_interaction_2026_11_20.md](deepseek_title_newgame_interaction_2026_11_20.md) | 2026-11-20 | **CE42/CE43 点击链路与 Map003 时序** | CE42 新游戏分支无存档钩子；CE43 仅绘制/高亮；Map003 两段 `180[50]` 是数据脚本节奏 |
| [deepseek_title_graphics_fresh_state_2026_11_20.md](deepseek_title_graphics_fresh_state_2026_11_20.md) | 2026-11-20 | **全新 V300/V499/V503/V550 分支与封面** | TitleMap 显示 `title01`，CE42 菜单显示 `TitleGraphic.jpg`，Map003 过渡显示 `TitleGraphic.png` |
| [deepseek_font_fallback_chain_2026_11_20.md](deepseek_font_fallback_chain_2026_11_20.md) | 2026-11-20 | **Lolita/DxLib/GDI 字体回退链** | 四个字体槽均为 `LOLITA`；出货文本与 GB2312 均无缺字；人为缺字为 tofu，无配置中文替代字体 |
| [bug_inspection_report_2026_11_20.md](bug_inspection_report_2026_11_20.md) | 2026-11-20 | **右键点击失效、游戏素材缺失、文本解析内部代号泄露** | 定位 InputRouter 掩码与锁存缺陷、media.pak 前作素材引用问题、ChoiceWindow 裸绘等 8 项缺陷 |
| [bug_fix_and_gate_opt_2026_11_20.md](bug_fix_and_gate_opt_2026_11_20.md) | 2026-11-20 | **输入/文本/VM 修复、Oracle 同步与门禁并行加速** | 修复 GameRoot 鼠标拦截、Opcode 122/140 与会话残留状态，重生成 3 份 Oracle 夹具并加速门禁 |
| [lwsmf_title_input_and_deep_audit_report.md](lwsmf_title_input_and_deep_audit_report.md) | 2026-11-20 | **标题交互、输入路由与全量字节码审计** | 梳理标题界面多模态输入（鼠标、触控、按键）与底层字节码的映射逻辑 |
| [imoto_life_5dd6956_review_and_ux_report.md](imoto_life_5dd6956_review_and_ux_report.md) | 2026-11-19 | **Commit `5dd6956` 表现层与交互审查** | 审查立绘图层层级（z=60 转场叠加）、色调/Alpha 与负缩放镜像表现 |
| [imoto_life_deep_audit_report_a5b356d9.md](imoto_life_deep_audit_report_a5b356d9.md) | 2026-11-19 | **Commit `a5b356d9` 核心字节码深层审计** | 静态排查 RunCondition=2 循环等待、Opcode 176/211/125/151 高危调用 |
| [imoto_life_phase2_3_fix_plan.md](imoto_life_phase2_3_fix_plan.md) | 2026-11-19 | **Phase 2 & Phase 3 缺陷整改实施方案** | 确定 AutoEvents 边沿触发、176 循环头与 211 预约队列的规范实现方案 |

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

## 3. 跨仓库对照、前置架构与专题研究

| 文档 | 对照标的 | 核心内容 |
| :--- | :---: | :--- |
| [imoto_life_title_and_layout_audit_report.md](imoto_life_title_and_layout_audit_report.md) | 姊妹作《妹！生活》 | 标题界面、窗口布局、字体排版像素对齐审计 |
| [imoto_life_cross_repo_audit_checklist.md](imoto_life_cross_repo_audit_checklist.md) | 跨工程协同 | 核心架构、字节码解析器、输入层设计清单对比 |
| [imoto_life_c553d04_review_and_roadmap.md](imoto_life_c553d04_review_and_roadmap.md) | 提交 c553d04 对齐 | 跨项目路线图对齐与设计借鉴 |
| [lwsmf_multiplatform_packaging_checklist.md](lwsmf_multiplatform_packaging_checklist.md) | 多端打包发布 | Windows/Android/iOS 多平台资产导出核验清单 |

---

## 4. 历史保留与闭环对照

| 文档 | 状态 | 说明 |
| :--- | :---: | :--- |
| [lwsmf_orchestrator_pre_release_tasklist.md](lwsmf_orchestrator_pre_release_tasklist.md) | 已取代 | 仅保留为 [PRE_RELEASE_ORCHESTRATOR_AUDIT.md](../godot/PRE_RELEASE_ORCHESTRATOR_AUDIT.md) 的任务清单出处；不再作为执行依据 |
