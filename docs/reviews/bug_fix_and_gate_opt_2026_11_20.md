# 缺陷排查修复、跨作对齐与门禁加速报告（2026-11-20）

本报告记录针对 [bug_inspection_report_2026_11_20.md](bug_inspection_report_2026_11_20.md) 的核查修复、对照姊妹作（`Imouto Life Monochrome`）非操作类修复的跨仓库审计落地，以及 `tools/godot_check.sh` 耗时瓶颈分析与并行化改造。

---

## 1. 鼠标完全无法操作根因与输入修复

| 缺陷项 | 根因定位 | 修复方案 | 验证回归 |
| --- | --- | --- | --- |
| 鼠标点击/悬停完全无响应 | `GameRoot` 继承 `Control`，默认 `mouse_filter = MOUSE_FILTER_STOP`，在最外层拦截/吞掉了到达子控件与穿透窗口的鼠标事件，且单帧点击未独立锁存进 `EngineState(7/8/9)` | [GameRoot.tscn](../../godot/scenes/GameRoot.tscn) 与 [GameRoot.gd](../../godot/presentation_gd/GameRoot.gd) 显式设置 `MOUSE_FILTER_IGNORE`；[InputRouter.gd](../../godot/presentation_gd/InputRouter.gd) 新增 `left_frame_pressed()` / `right_frame_pressed()` / `middle_frame_pressed()` 与 `_middle_click_latch` 并由 `advance_frame()` 传入 VM | [GameRootTest.gd](../../godot/tests_gd/GameRootTest.gd)（`Viewport.push_input` 实发移动、左/右键点击、滚轮与 `EngineStateBridge(7/8)`） |
| 空格键误映射为确定 | `InputRouter.gd` 将 `KEY_SPACE` 列在 `ACTION_KEYS` | 将 `KEY_SPACE` 移至 `CANCEL_KEYS`（与 `KEY_X`/`KEY_ESCAPE`/`KEY_BACKSPACE` 同组） | [InputRouterTest.gd](../../godot/tests_gd/InputRouterTest.gd) |
| 悬停微抖抢占键盘光标 | `InputEventMouseMotion` 亚像素抖动与键盘/滚轮切换选择框后立即触发 `hover_at` | `InputRouter.gd` 对已激活指针过滤 `distance_squared_to < 1.0` 的亚像素移动；`GameRoot.gd` 在 `_on_move` / `_on_wheel` 后锁定 4px 死区 `_choice_hover_lock_pos`（`CHOICE_HOVER_DEADZONE_SQ = 16.0`） | [InputRouterTest.gd](../../godot/tests_gd/InputRouterTest.gd) |

---

## 2. 文本、表现层与 VM 语义修复（含姊妹作对照）

| 模块 | 修复内容 | 文件 |
| --- | --- | --- |
| 空 `\cself[5..9]` 与 `\sdb` 映射 | `\cself[n]` / `\self[n]` 当 `(index % 100)` 位于 `5..9` 且字符串槽为空时保持 `""`（不再回退数值 `0`）；`\sdb` 映射 `kind = 1` 到 `DbContainer.Sys` | [TextExpander.cs](../../godot/core_cs/TextExpander.cs)、[VmController.cs](../../godot/presentation/VmController.cs) |
| 空选择项渲染与跳过 | `ChoiceWindow` 通过 `_visible_indices()` 在非 `override_rects` 模式下跳过 `display_text` 为空的条目，同时在 `chosen` 信号中保留原始选项索引 | [ChoiceWindow.gd](../../godot/presentation_gd/ChoiceWindow.gd)、[DialogDrawTest.gd](../../godot/tests_gd/DialogDrawTest.gd) |
| SE 音调缩放 | `AudioManager.play` 允许 `CHANNEL_SE` 在 `tempo > 0` 时按 `clampf(float(tempo) / 100.0, 0.25, 4.0)` 应用 `pitch_scale` | [AudioManager.gd](../../godot/presentation_gd/AudioManager.gd)、[AudioManagerTest.gd](../../godot/tests_gd/AudioManagerTest.gd) |
| Opcode 122 字符串全模式 | 支持目标间接寻址（`0x0010`）、`sourceKind` `0/1/2`、追加（`0x0100`）、首行剪切/复制（`0x0200/0x0300`）、首尾字符/转义单元切割（`0x0400/0x0500`，含 `<<GET_FILE_EXIST>>` 返回 `"0"`）、全删（`0x0800`）、替换（`0x0900`）、前缀/后缀截断（`0x0A00/0x0B00`） | [EventVm.cs](../../godot/core_cs/EventVm.cs)、[VmTest.cs](../../godot/tests/VmTest.cs) |
| Opcode 140 多行音效格与动态参数 | 对 `Params[1,4,5,6]` 使用 `EvalNum` 求值；通过 `SplitSoundCell` 拆解 `file\r\nvolume\r\ntempo` 多行格，同时保留单行曲目显式 `volume = 0` | [EventVm.cs](../../godot/core_cs/EventVm.cs)、[VmTest.cs](../../godot/tests/VmTest.cs) |
| 会话残留对话状态清理 | `GameSession` 在 `StartNewGameNow`、`ApplyLoadedState`、`EnterMap` 的 `StopAll()` 后及 `AdvanceFrame` 协程等待失效时同步清理残留 `_messageOpen` / `_hasChoices` | [GameSession.cs](../../godot/core_cs/GameSession.cs) |
| 窗口模式时钟 Sys82 同步 | `GameRoot._sync_live_clock_if_windowed()` 在非 `headless` 且非 `WOLF_LIVE_RNG=0` 时同步系统秒数到 `Sys82` | [GameRoot.gd](../../godot/presentation_gd/GameRoot.gd) |

---

## 3. Oracle 对齐与 Trace 夹具重生成

- 同步更新 Rust Oracle（[exec.rs](../../crates/wolf-vm/src/exec.rs)、[text.rs](../../crates/wolf-vm/src/text.rs)），`cargo test -p wolf-vm` 全部 `145` 项测试通过（`95 + 5 + 4 + 7 + 4 + 5 + 10 + 15`）。
- 为 [RouteTraceTest.cs](../../godot/tests/RouteTraceTest.cs) 的 `RouteHost` 补齐图片槽跟踪与 `PictureStateInternal`（与 `route.rs` / [M4FullRouteTraceTest.cs](../../godot/tests/M4FullRouteTraceTest.cs) 一致）。
- 重新生成三份 Oracle 参考夹具并与 C# Trace 对拍 100% 一致：
  - `oracle/fixtures/samplemapa_event0_418_rust_trace.json`：`2461/2461` 步一致；
  - `oracle/fixtures/main_route_rust_trace.json.gz`：`149245/149245` 步一致（`1500` 帧）；
  - `oracle/fixtures/m4_full_route_rust_trace.json.gz`：`939630/939630` 步一致（`7663` 帧）。

---

## 4. 门禁耗时排查与性能优化

- **耗时热点定位**：
  1. `M13MainlineEndingsTest.gd` 每帧调用 5 次 `picture_snapshots()` 深拷贝全量图片字典，且 `GameRoot` 逐帧打印 `[GameRoot] MAP ...` 产生海量同步日志；单槽实测耗时约 **735 秒**（12m15s）。
  2. `M4FullRouteTraceTest.cs`（`35/38`）每步指令分配全量 `Dictionary<int, int>` / `Dictionary<int, string>` 快照并组装 JSON DOM 对象；单步构建耗时约 **88 秒**。
  3. `tools/godot_check.sh` 将 38 个门禁步骤与两个 `M13` 槽位严格串行执行，合计超过 **30 分钟**。
- **优化措施与实测效果**：
  - [PictureStage.gd](../../godot/presentation_gd/PictureStage.gd) 新增 `O(1)` 查询接口 `picture_text(number)`，并在 [M13MainlineEndingsTest.gd](../../godot/tests_gd/M13MainlineEndingsTest.gd) 守卫 `HasEvent(305)` 后才调用 `GetEventSelfAll`；[GameRoot.gd](../../godot/presentation_gd/GameRoot.gd) 支持 `LWsmf_QUIET_HUD=1` 抑制每帧 HUD 控制台刷屏（单跑 `M13 slot=1` 从 ~735s 降至 **46.64s**）。
  - [Variables.cs](../../godot/core_cs/Variables.cs) 暴露只读 `ReadOnlySpan` 视图，[M4FullRouteTraceTest.cs](../../godot/tests/M4FullRouteTraceTest.cs) 改为复用平铺缓冲区并直接流式格式化 JSON 效应，C# 侧耗时从 ~88s 降至 **16.27s**（单跑含 Python 对拍合计 **26.43s**）。
  - [godot_check.sh](../../tools/godot_check.sh) 拆分为“前置编译与缓存生成 → 8 并发隔离池（按预估耗时降序派发，共享 `user://` 的测试单车道串行）→ 性能探针独享串行”，全量 38 项门禁 + 双槽 `M13` 主线通关门禁总耗时降至 **`251s`（约 4 分 11 秒）**，`GODOT_GATE_OK`。
