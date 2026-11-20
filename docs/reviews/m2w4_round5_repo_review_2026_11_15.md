# LWSMF Godot 重制版第五轮独立技术审阅报告（`646a93d6` 仓库快照）

> **审阅对象**：`nana7mi0721/imoto-life`（commit `646a93d6ccb6ef9d21de27a176da90605f33b3ae`）  
> **对照基线**：第四轮报告（`M2W4_ROUND4_REVIEW_2026-11-15.md`）、`docs/godot/DECISIONS.md`（`D-01..D-22`）  
> **证据边界**：本沙箱无 Godot / .NET / Rust 编译器、无 `game_data/`、无 `work/all` 零售素材、且当前仓库快照不含 `reports/` 运行日志（`30/30` 门禁结果仅见于 `docs/godot/M1_HANDOFF.md` 声明）。凡静态无法证明的运行时/零售资产行为均已标注 `needs-runtime` 或 `needs-retail-data`。

---

## 1. 总评：M2 收口状态与 M3 准入研判

**结论：M2 核心与架构收敛已基本达标，可以正式开启 M3（战斗/H 与 `.sav` 导入），但须在 M3 第 1 周顺手关闭本轮查出的 2 项 P1 运行时接线遗漏。**

相对上一轮（`m2w4`），本次仓库快照（`646a93d6`）完成了大量实质性工程收敛，上一轮指出的 5 项阻断/核心缺陷已悉数在源码中落地修复：
1. **第四轮缺陷已闭环（置信度：高）**：`EventVm` `300` 补齐 `Host.CommonEventCall`、`TraceRecorderHost` 修正 `Bgs` 通道映射、`AudioManager` 修正 WAV `loop_begin` 采样点换算、`MapLayer.gd` 删除双实现并改走 `TerrainBridge.cs`、`SaveCodec` 补齐 `MapFile` 安全路径与 `LoopPoint` 上限、`GameSession` 约束传送目标 `-1/0` 与每帧 64 条上限。
2. **旧表现层已彻底退役（置信度：高）**：`TitleScreen.cs`、`ScreenHostView.cs`、`ScreenHostSessionView.cs`、`AssetCatalog.cs`、`MapView.cs` 与 `TitleScreen.tscn` 已从代码树删除；`SessionParityTest.cs` 改用 `SessionHarness` 对比冻结夹具 `oracle/fixtures/session_core_save_state.json`。
3. **整路线对拍与白天菜单已打通（置信度：高）**：新增 `RouteTraceTest.cs` + `oracle/diff_route.py`（1500 帧 / 73,812 步，含 `frame` 与 `str_vars` 比对）；新增 `PictureStateTest.gd`、`DayMenuTest.gd`（`CE251`）与 `WeekendMenuTest.gd`（`CE310`），并将门禁扩展至 30 道。
4. **仍需注意的残余缺口（置信度：高）**：
   - **`126` 权限模型未接入 `GameSession.MoveLeader` 与 `GameRoot` 交互**：`EventVm.BasicMoveDeny`/`BasicAllDeny` 目前仅过滤 `123` 轮询结果，**并未拦截卧室行走与对话/选择按键**。
   - **快照不含 `reports/` 实跑日志**：`30/30` 与 `73812/73812` 在本仓库静态快照中属 `needs-runtime` 待本机复验项；且整路线 1500 帧不含 `101` 消息与白天剧情冷启动。

---

## 2. 第四轮问题复核表

| ID / 问题 | 当前状态 | 严重度 | 证据（文件:行） | 残余风险 / 核验说明 | 置信度 |
| --- | --- | --- | --- | --- | --- |
| **`300` 漏报 `CommonEventCall`** | **已修** | — | `godot/core_cs/EventVm.cs:1138` | 已在 `CreateInstance` 后、写参数前补齐 `Host.CommonEventCall(callee.Id, callArgs, callStrings)` | 高 |
| **`TraceRecorderHost` BGS 误作 `se`** | **已修** | — | `godot/core_cs/TraceRecorderHost.cs:65-70` | `SoundChannel.Bgs => "bgs"` 已修正，与 `RouteHost.Sound` 一致 | 高 |
| **trace 漏 `frame` 与 `str_vars`** | **已修** | — | `oracle/diff_trace.py:124,167-186`; `oracle/diff_route.py:32,60-70` | 两侧 diff 均已比对 `frame` 与 `str_vars`；注：`diff_trace.py:44` 仍剥离空串/`None`，与 `diff_route.py:25` 口径略异 | 高 |
| **`SaveCodec` `MapFile` 与 `LoopPoint` 校验** | **已修** | — | `godot/core_cs/SaveCodec.cs:308,360` | 已校验 `IsSafeAssetPath(state.MapFile)` 与 `audio.LoopPoint <= 86_400 * 60` | 高 |
| **`GameSession` 传送目标与队列上限** | **已修** | — | `godot/core_cs/GameSession.cs:235-257` | `MaxTeleportsPerFrame = 64`；`ApplyTeleport` 忽略 `Target != -1 && Target != 0` | 高 |
| **`MapLayer.gd` 双实现 & 未接线** | **已修** | — | `godot/presentation_gd/MapLayer.gd:98-131`; `godot/presentation/TerrainBridge.cs:18-41`; `godot/presentation_gd/GameRoot.gd` | 通行度统一由 `TerrainBridge` 调 C# `Terrain`；`cell_clicked` 已连 `_on_map_cell_clicked` | 高 |
| **`AudioManager` WAV `loop_begin` 单位** | **已修** | — | `godot/presentation_gd/AudioManager.gd:185`; `godot/tests_gd/AudioManagerTest.gd:26-30` | 已按 `roundi(float(loop_point) * float(wav.mix_rate) / 60.0)` 换算采样点 | 高 |
| **旧表现层退役 & 整路线 Oracle** | **已修** | — | `godot/tests/SessionParityTest.cs:38-84`; `godot/tests/RouteTraceTest.cs:21-192` | 旧 `TitleScreen.cs` 等已删；`main_route_rust_trace.json.gz` 实测 1500 帧 / 73,812 步 | 高 |
| **MapChip 贴图缺失** | **口径变更（D-21）** | — | `docs/godot/DECISIONS.md` (`D-21`); `godot/presentation_gd/MapLayer.gd:5-8` | 确认为 WOLF 引擎示例素材，正片卧室/剧情全用 `Picture` 全屏背景，不接入 102 张 PNG | 高 |
| **白天工作菜单 `CE251/CE310`** | **已修（中段入口）** | — | `godot/tests_gd/DayMenuTest.gd:69`; `godot/tests_gd/WeekendMenuTest.gd:56` | 已打通 `124` 命中与 `EngineState(7)` 三态，但从 `pc 1022`/`pc 128` 起步，未测冷启动进白天 | 高 |
| **`126` 权限与 `123` 掩码（D-20）** | **部分修复** | **P1** | `godot/core_cs/EventVm.cs:38-103,628-633,1075`; `godot/core_cs/GameSession.cs:260-285` | `EventVm` 已实现三态与设备分支，但**`GameSession.MoveLeader` 与 `GameRoot` 未检查 `MovementDeny`/`BasicAllDeny`** | 高 |
| **`Faulted` 恢复路径** | **部分修复** | **P1** | `godot/presentation_gd/GameRoot.gd` (`restart_after_fault`) | 已支持「返回标题」重建安全点且不写坏档；但 **`router.pointer_pressed` 与 `LoadGame()` 后 `Faulted` 检查仍有漏口** | 高 |
| **音频共享流污染 & `soundType 3..7`** | **仍开放** | P2 | `godot/presentation_gd/AudioManager.gd:175-185`; `godot/core_cs/EventVm.cs:607-612` | 同文件跨 BGM/BGS 仍直接改写共享 `AudioStream`；未知音频类型位 `3..7` 仍静默落入 BGM | 高 |
| **`W2-P1-02` / `W2-P2-03` / `W2-P1-04`** | **仍开放** | P2 | `GameRoot.gd:178-182`; `UiSkin.gd:16-24`; `PictureStage.gd:289-290` | 101 未读完按取消直接关窗、系统回退字体、非 `TextureRect` 占位框不翻转均待 `needs-retail-data` | 中 |

---

## 3. 本次新增/变更代码专项审计表

| 模块 | 严重度 | 证据（文件:行） | 触发条件 | 根因 | 影响 | 最小修复 | 验证方法 | 置信度 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **`EventVm` / `GameSession` / `GameRoot`（`126` 接线）** | **P1** | `EventVm.cs:45` (`MovementDeny`); `GameSession.cs:260-285`; `GameRoot.gd:159-210` | 脚本执行 `126` 禁止移动（state 1）或全禁（state 2）后，玩家在卧室按方向键/点击地图或按确认键 | `EventVm` 虽计算了 `BasicMoveDeny`/`BasicAllDeny`，但 `GameSession.MoveLeader` 与 `GameRoot._on_action`/`_on_move` 未读取这两个属性 | 与 Rust `game.rs` 行为不一致：神谕侧 `126` 锁步/锁键生效，Godot 实机仍可自由走动或确认 | 在 `VmController` 暴露 `MovementDeny`/`BasicAllDeny`；`GameSession.MoveLeader` 遇 `MovementDeny` 阻塞移动，`GameRoot._on_action`/`_on_cancel` 在非系统模态下尊重 `BasicAllDeny` | 在 `GameRootTest.gd` 注入 `126` 禁移动并验证 `MoveLeader` 不位移 | 高 |
| **`GameRoot.gd`（故障复位 & 检出点）** | **P1** | `GameRoot.gd` (`advance_frame`, `restart_after_fault`, `_unhandled_input`) | 鼠标按住时触发 `Faulted` 后点「返回标题」，或 F9 `LoadGame()` 期间触发 VM 故障 | `restart_after_fault()` 调用 `vm.SetPointerPressed(false)` 但**未清 `router.pointer_pressed` 与 `router.clear_frame_latches()`**；`LoadGame()` 后未检 `vm.Faulted` | 回到标题后下一帧 `advance_frame` 会立刻把残留的 `router.pointer_pressed` 重新喂给 `vm` 造成误点击 | 在 `restart_after_fault()` 增加 `router.pointer_pressed = false; router.clear_frame_latches()`；在 `F9 LoadGame()` 后检查 `if vm.Faulted: report_fault(...)` | 在 `GameRootTest.gd` 模拟按住左键触发 fault 再重启标题，断言 `EngineStateBridge(7) == 0` | 高 |
| **`PictureStage.gd`（`124` 命中与锚点）** | P2 | `PictureStage.gd:152-207,234-249` | 绘制 `kind == 2`（text）或 `kind == 3`（window）图片并查询 `124` | `kind == 2/4` 用 `args[1], args[2]` 作命中盒（对齐 Rust 1×1 规则），而真实 `Label` 按 `_measure_text` 渲染 | 符合 Rust 契约；但注意若 `PictureStage.picture_state` 被 `LoadGame()` 后的 `restore_pictures` 调用，`_cmds` 已恢复，可正常命中 | 无需改动（若未来发现 `text` 槽需要可点击，需双边同步修改 Rust 与 GDScript） | `PICTURE_STATE_TEST_OK` | 高 |
| **`VmController.cs`（`EngineState` 键位）** | P2 | `VmController.cs:362-370` | 脚本查询 `124 0x3000` field 8（右键）或 9（中键） | `EngineState` 仅实现了 field 7（左键三态 `0/1/2`），field 8/9 硬编码返回 `0` | 若 M3/M4 战斗或隐藏菜单读取右键状态（field 8），将永远读到未按下（`needs-retail-data`） | 在 `InputRouter` 补充右键/中键 `pressed` 状态并透传至 `VmController` | 补 `PictureStateTest.gd` 右键三态用例 | 高 |
| **`oracle/diff_trace.py` vs `diff_route.py`** | P2 | `oracle/diff_trace.py:38-48` vs `oracle/diff_route.py:24-25` | 比较单事件 trace 与整路线 trace 的 `effects` | `diff_trace.py` 会过滤 `v == "" or v is None`，而 `diff_route.py` 保留空串键 | 两套对拍脚本对 `effects` 空字段的严格度不一致 | 统一 `normalize_effects` 逻辑，避免静默忽略字段差异 | 单元测试 `diff_trace.py` 与 `diff_route.py` | 高 |
| **`SessionHarness` / `TestSessionView`** | 说明 | `godot/tests/SessionHarness.cs`; `godot/tests/TestSessionView.cs` | 运行门禁 9/11/12/23 | 替代已退役的 `TitleScreen.cs` 驱动 `GameSession` | 解耦了旧表现层，但门禁 9/11/12 仍只测 `TestSessionView` 而非 `GameRoot.gd` | 后续可将 `SaveLoadTest` 的核心断言镜像一份到 `GameRootTest.gd` | `needs-runtime` 复跑 30/30 | 高 |

---

## 4. Oracle 对拍与 30 道门禁可信度评估

1. **整路线 Oracle（`RouteTraceTest.cs` + `main_route_rust_trace.json.gz`）**：
   - **已证明**：从生产入口 `TitleMap.mps` event 2 → 新游戏 → `Map003` → `Map001` 的 **1,500 帧 / 73,812 步**，两侧在 `frame`、`map_id`、`event_id`、`pc`、`op`、`vars`（整数）、`str_vars`（字符串）及 7 类 `effects` 上实现了结构化逐字段比对（并修复了此前 Rust `SysDB` 行序与 `programs.json` `_map_id` 不一致的真实分叉）。
   - **仍没证明**：这 1,500 帧内 **0 条 `101` 消息**、仅有标题菜单 1 次选择、无 `BGS` 播放；且 `RouteTraceTest` 末尾的 `SaveGame/LoadGame` 仅做状态断言而非逐指令对拍。
2. **白天菜单测试（`DayMenuTest.gd` / `WeekendMenuTest.gd`）**：
   - **已证明**：`EngineState(7)` 的官方三态（0 未按 / 1 按住 / 2 刚按下）与 `PictureStage.picture_state`（field 8/9/0/1/10）能真实驱动 `CE251`（从 `pc 1022` 起步）与 `CE310`（从 `pc 128` 起步）完成随机命令抽取、悬停、点击、自动确认、时钟推进与体力消耗。
   - **仍没证明**：未覆盖“从前一晚或冷启动自然进入白天菜单”的完整剧情衔接；未覆盖 `CE310` 动态条件标签行（`self5/6/7`）；且无对应的 Rust Oracle 对拍。
3. **本仓库快照运行时状态（`needs-runtime`）**：
   - 本次上传的 Git 快照不包含 `reports/` 目录及本地门禁日志，因此 `M1_HANDOFF.md` 中声明的 **30/30 双模式全绿** 与 **73,812/73,812 全等** 需以你本地最近一次 `bash tools/godot_check.sh` 与 `LWsmf_RENDER=1` 的实际退出码为准。

---

## 5. 下一步行动建议（进入 M3 前/第 1 周）

1. **立即修补 2 项 P1 遗漏（约 0.5 人日）**：
   - 把 `EventVm.MovementDeny` / `BasicAllDeny` 接入 `GameSession.MoveLeader` 与 `GameRoot` 输入分发；
   - 在 `GameRoot.restart_after_fault()` 同步重置 `router.pointer_pressed` 与 `router.clear_frame_latches()`，并在 `LoadGame()` 后补 `vm.Faulted` 检查。
2. **正式开启 M3**：
   - 按 `PLAN.md` 推进战斗与 H 相关 opcode（补齐 `EngineState(8)` 右键三态）、`CE159/162` 实机抓包对照、以及 `LegacySaveImporter` 的 CP932/GBK 严格解码与坏 checksum 拦截测试。
   - 将 `CE251`/`CE310` 的白天菜单与 101/102 对话纳入下一条独立冻结路线夹具（`oracle/fixtures/daytime_route_rust_trace.json.gz`），补齐主线 1500 帧之外的 Oracle 盲区。
