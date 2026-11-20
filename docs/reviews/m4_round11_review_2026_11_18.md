# LWSMF Godot 重制版第十一轮技术审阅报告（`25e72ec`）

- **审阅对象**：`imoto-life`（commit `25e72ece9aa67564d541ecf7b34bc672611050cb`）
- **主审区间**：`dcc4360..25e72ec`（M4 三道硬门槛收口 + 第十轮问题修复）
- **审阅日期**：2026-11-18

## 一、总评

1. **第十轮核心缺陷全部闭环，顺带修复一处隐藏协程泄漏（置信度：高）**
   `R10-P2-01`（`BattleNaturalLoopTest.StrictMode = true`）、`R10-P2-02`（`LoadRetailSave` 显式 `rejectBadChecksum: true`、恢复 `ChoiceAnchor` 至 `Sys3/4`、`DecodePictureText` 记录 `encoding_warnings`）、`R10-P3-02`（Rust `m13_random_events_injected.rs` 校验 `external_gates`）及 `R10-P3-03`（`tools/godot_check.sh` 末尾有条件接入 `M13MainlineEndingsTest`）均已在源码中核实落地。同时 `EventVm.StopInstance`（`godot/core_cs/EventVm.cs:1274-1310`）新增子孙协程级联终止，消除了地图页切换时 `210` 调用链残留协程泄漏的隐患。
2. **M4 第一门槛（含交互整路线逐指令 Oracle）已实质闭环（置信度：高）**
   直接解析 `oracle/fixtures/m4_full_route_rust_trace.json.gz` 可证实：该夹具完整覆盖 7,663 帧、939,630 步，包含 `op101=230`、`op102=25`、`op140=114`、`op250=2402`、白天 `CE251`（1,690 步）、战斗 `CE253/CE239/CE242/CE244`（2,943 步）及晚餐/夜间 `CE300/CE326/CE245/CE329`，且 `M4FullRouteTraceTest.cs` 全程开启 `StrictMode = true` 并在末尾完成 F5/F9 存档往返校验。需注意的客观边界：`M4FullRouteTraceTest.cs:185` 第 4 天前仍调用 `SeedBattle(1, 1)`，且 F5/F9 回路在 `StepHook = null` 后做状态断言（与 `D-17` 一致）。
3. **M4 第二、三门槛（全结局/随机池 C# 化、`op150` 负缩放全量扫描）均已达标（置信度：高）**
   `M13AllEndingsTest.cs` 覆盖其余 7 结局（`cases=15 gates=7`），`M13RandomEventsTest.cs` 覆盖 111 随机池全量扫描与 19,624 处调用目标校验（`cases=23 gates=8 sweep=38`）；`Op150NegativeScaleScanTest.cs` 与 `oracle/scan_op150_negative_scale.py` 扫描全部 20,425 条 `op150` 指令，证实 63 处真实负缩放均为 `kind 0/3` 图片，**`text=0`**，正式关闭 `R7-NEW-03` 风险。
4. **仅剩 2 处极轻微 P3 收口项，M4 退出条件已满足，可准入 M5 收口与发布阶段（置信度：高）**
   `tools/godot_check.sh:123-125` 仍残留 `14/34..16/34`（未随第 35 步更新为 `/35`）；`M13Harness.NewVm()` 可显式补上 `vm.StrictMode = true`。两者均不影响功能与发布阻断。

## 二、快照核对结论

- **快照来源**：基于上传的 `imoto-life` 归档包（解压目录 `imoto-life-25e72ece9aa67564d541ecf7b34bc672611050cb`），对应 commit `25e72ec`。
- **边界声明**：沙箱无 Godot 4.7.2 .NET 运行时与 Rust 工具链，截图中的双模式门禁（`gate_m4_op150.log`、`gate_m4_op150_windowed.log`）及 Rust 参考测试结果由提交方本机提供，运行时结论标 `needs-runtime`；`m8_negative_scale` 单条用例因 `work/all` 缺 `1.4.0/SD_menu_010.png` 失败属已知测试资产缺失，与 C# 运行时无关。

## 三、第十轮问题复核表

| ID | 当前状态 | 证据(文件:行) | 复核结论 |
| --- | --- | --- | --- |
| `R10-P2-01` | **已闭环** | `godot/tests/BattleNaturalLoopTest.cs:187` | `vm.Vm.StrictMode = true` 已启用 |
| `R10-P2-02` | **已闭环** | `godot/core_cs/GameSession.cs:548,588-592`; `LegacySaveImporter.cs:424,533,585-597`; `SaveImportTest.cs:127-137` | `rejectBadChecksum: true`、`Sys3/4` 恢复及 `DecodePictureText` UTF-8 告警与单测均已补齐 |
| `R10-P2-03` | **保留为低风险已知边界** | `godot/tests_gd/SignalStressTest.gd:23-36` | `ChoicesRequested`/`EffectRequested` 仍走 `EmitSignal` 传数组；全 7,663 帧 `M4FullRoute` 长跑未复现崩溃（`needs-runtime`），M5 性能/稳定性阶段按需观察即可 |
| `R10-P3-01` | **部分闭环（仍差 1 步）** | `tools/godot_check.sh:123-125` | 从 `/33` 改成了 `/34`，但本轮已扩至 35 步，14..16 行未同步改为 `/35` |
| `R10-P3-02` | **已闭环** | `crates/wolf-game/tests/m13_random_events_injected.rs:152-159` | 已补齐 `external_gates` 5 条存在性与非别名断言 |
| `R10-P3-03` | **已闭环** | `tools/godot_check.sh:258-267` | 已在检测到 `SaveData01.sav` 时自动跑 `M13MainlineEndingsTest` 槽位 1 与 8 |

## 四、本轮新增项与遗留清单

| ID | 严重度 | 证据(文件:行) | 问题描述 | 最小修复 |
| --- | --- | --- | --- | --- |
| `R11-P3-01` | P3 | `tools/godot_check.sh:123-125` | `run_gd "14/34".."16/34"` 标签落后于全脚本 `/35` 计数 | 改为 `14/35..16/35`（或引入自增变量） |
| `R11-P3-02` | P3 | `godot/tests/M13Harness.cs:35-42` | `M13Harness.NewVm()` 未显式设 `vm.StrictMode = true`（虽有 `Unhandled == 0` 断言兜底） | 在 `NewVm()` 增加 `vm.StrictMode = true;` |

## 五、M4 退出与 M5 准入结论

- **M4 退出标准对照**：
  - 550 公共事件 / 73 地图事件 / 169,831 指令对账与 19,624 处调用目标扫描通过；
  - 9 结局（B2/B3 + `M13AllEndingsTest`）、111 随机池（`M13RandomEventsTest`）、DLC 条件注入矩阵（`ContentMatrixTest`）、H 场景全支（`HSceneTest`）及无外部存档自然战斗闭环（`BattleNaturalLoopTest`）全部覆盖；
  - 7,663 帧 / 939,630 步交互整路线（`M4FullRouteTraceTest`）与 Rust Oracle 100% 逐字段对拍通过（`needs-runtime` 复核）；
  - `op150` 负缩放文本槽零命中（`text=0`），`LoadRetailSave` 严格校验已接线。
- **准入结论**：**M4 已达标，准予进入 M5（收口与发布）**。M5 阶段重点关注：Windows x64 正式导出包构建与无零售资产冷启动引导、真机 60fps 帧耗时与 GC 剖析（含 `TakePicturesJson` 与 `ChoicesRequested`/`EffectRequested` 稳定性复核）、以及 30/90 天长跑实机验收。
