# LWSMF 移动端四项交互问题定位与修复方案

> **分析基线**：`Living With Sister: Monochrome Fantasy`（Godot 4.7.2 .NET，提交 `2b68246e`）  
> **涉及文件**：`godot/presentation_gd/InputRouter.gd`、`godot/presentation_gd/GameRoot.gd`、`godot/presentation_gd/ChoiceWindow.gd`、`godot/presentation_gd/PictureStage.gd`、`godot/project.godot`、`godot/presentation/VmController.cs`、`godot/core_cs/EventVm.cs`

## 问题总览

| 编号 | 现象摘要 | 核心根因位置 | 优先级 | 修复难度 |
| :--- | :--- | :--- | :--- | :--- |
| **MOB-01** | 数值界面长按技能图标松手后仅判定为点击一次，无法学习技能 | `InputRouter.gd`（`_handle_screen_touch`、`step_confirm_hold`、`_reconcile_pointer_buttons`） | **P0（功能阻断）** | 中 |
| **MOB-02** | 妹妹互动选睡觉 → 二次确认双击退出 → 偶发点击妹妹无反应（读档恢复） | `GameRoot.gd:1747`（`_release_modal_guard_if_free`）、`project.godot` 触控模拟鼠标、`EventVm.cs:1592` | **P0（偶发软锁）** | 中 |
| **MOB-03** | 取消妹妹互动后，卧室妹妹立绘先在位置 A 闪现几帧，再消失并出现在位置 B | `SampleMapA ev4`（`pc70 → CE245 → CE325` 两段绘制）与 `PictureStage.gd`（`_begin_draw`） | **P2（视觉残影）** | 低–中 |
| **MOB-04** | 一般样式选项框位置偏高，需使其横向中轴与 `1024×576` 画面中轴重合 | `GameRoot.gd:957`（`_choice_position`）与 `ChoiceWindow.gd:65-75`（`window_rect`） | **P1（布局体验）** | 低 |

---

## 1. MOB-01：数值界面长按技能图标学习技能失败

### 1.1 根因定位

当前触控输入链路在“单指按住”期间**不会向 VM 传递持续按压态**，共三处叠加：

1. **按下阶段未置位持续按压标志**（`godot/presentation_gd/InputRouter.gd:311-334`）：`_handle_screen_touch()` 在 `st.pressed == true` 时仅写入 `_touches[st.index]` 并更新 `hover_position`，从未置位 `pointer_pressed`；而 `left_frame_pressed()`（第 629 行）和 `held_mask()`（第 585 行）均只检查 `pointer_pressed or _click_latch or _touch_ok_ttl > 0`，不读取 `_touches`。因此手指按住技能图标期间，VM（`opcode 124 field 7` 与 `opcode 123 MASK_OK`）全程读到 `0`。
2. **现有长按机制仅服务于对话快进**（`InputRouter.gd:215-256`、`GameRoot.gd:469-475`）：`step_confirm_hold(dialogue_eligible, stop_barrier)` 只有在 `dialogue_eligible == true`（`MessageWindow` 打开且无选项框）时才会点亮 `_touch_hold_active`。数值/技能界面没有打开 `MessageWindow`，长按分支全程被跳过。
3. **松手仅发 4 帧短脉冲，且物理鼠标核对会误清触控按压**：
   - 手指松开时（`st.pressed == false`），因移动距离 `< TOUCH_TAP_MAX_DIST (24px)` 且 `_touch_gesture_consumed == false`，`InputRouter.gd:369-371` 触发普通单击，只写入 `TOUCH_PULSE_TTL_FRAMES = 4` 帧脉冲；且 `VmController.cs:849` 一旦被 `EngineState(7) == 1` 读到即标记 `_okConsumedThisFrame = true`，下一帧 `GameRoot.gd:523` 立刻调用 `router.consume_touch_ok()` 清零。技能学习需要连续数十帧持续按住读条，因此永远只被判定为“点击了一次”。
   - 另外 `InputRouter.gd:663` 的 `_reconcile_pointer_buttons()` 每帧用 `Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT)` 核对并清零 `pointer_pressed`，若不排除活跃触控，触控置位的持续按压会被立刻误清。

### 1.2 修复方案

在 `InputRouter.gd` 新增**通用单指长按转持续左键按住**机制（独立于对话快进 `step_confirm_hold`）：

1. **新增状态字段与常量**（`InputRouter.gd`）：
   ```gdscript
   const TOUCH_POINTER_HOLD_DELAY_FRAMES := 10  # 约 167ms，避开普通单击，迅速进入技能长按读条
   var _touch_pointer_hold_frames := 0
   var _touch_pointer_held := false
   ```
2. **在 `advance_frame()` 的 60Hz 步进中推进单指按住计时**（在 `step_confirm_hold` 之后、`vm.SetPointerPressed(...)` 之前调用 `router.step_touch_pointer_hold(not _modal_window_open() and not choice.is_open and not message.is_open())`）：
   ```gdscript
   func step_touch_pointer_hold(eligible: bool) -> void:
       if not eligible or not _is_touch_hold_steady() or _touch_hold_active:
           _touch_pointer_hold_frames = 0
           _touch_pointer_held = false
           return
       if _touch_pointer_held:
           return
       _touch_pointer_hold_frames += 1
       if _touch_pointer_hold_frames >= TOUCH_POINTER_HOLD_DELAY_FRAMES:
           _touch_pointer_held = true
           _touch_gesture_consumed = true   # 关键：松手时不再补发一次单击脉冲
           last_input_source = "touch"
           reset_double_tap_state()
   ```
3. **接入持续按压查询并保护不被 `_reconcile_pointer_buttons()` 误清**：
   - `left_frame_pressed()` 与 `held_mask()`：`pointer_pressed or _touch_pointer_held or _click_latch or _touch_ok_ttl > 0`。
   - `action_input_held()`：同样纳入 `_touch_pointer_held`。
   - `clear_touch_state()` 与 `_handle_screen_drag()`（位移 `>= TOUCH_HOLD_CANCEL_DIST` 时）：归零 `_touch_pointer_hold_frames = 0` 与 `_touch_pointer_held = false`。
   - 在 `VmController.cs:849` / `GameRoot.gd:523`：`ConsumeOkObserved()` 只消费一次性脉冲 `_touch_ok_ttl`（`consume_touch_ok()`），**不清除 `_touch_pointer_held`**，这样只要手指不松开，`EngineState(7)` 就能持续稳定返回 `1`（长按首帧 `2` → 后续帧持续 `1` → 松手变 `0`）。

### 1.3 验收标准
- 移动端在数值/技能界面长按任意可学技能图标，读条连续推进直至学会技能；松手后不会多触发一次单击。
- 普通短按点击按钮、对话长按快进、双指取消与拖动翻页均保持原样不受影响。

---

## 2. MOB-02：睡觉二次确认双击退出后偶发点击妹妹无反应

### 2.1 根因定位

结合源码定位出两条最直接导致“偶发无反应 + 读档即恢复”的链路：

1. **链路 A：`_modal_release_guard` 与 `emulate_mouse_from_touch` 竞态卡死（最高概率）**
   - 在睡觉二次确认选项框（`["是", "算了"]`）外双击空白退出时，`InputRouter.gd:351` 调用 `trigger_cancel_pulse("touch")` → `GameRoot.gd:638` 的 `_on_cancel()` 调用 `_arm_modal_release_guard()` 将 `_modal_release_guard` 设为 `true`。
   - 守卫释放函数 `_release_modal_guard_if_free()`（`GameRoot.gd:1747-1763`）在释放前会检查全局 `Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT)`。
   - 检查 `godot/project.godot` 发现未关闭 `input_devices/pointing/emulate_mouse_from_touch`（Godot 移动端默认开启）。虽然 `InputRouter.gd:268,284` 过滤了 `DEVICE_ID_EMULATION` 事件，但 `Input` 单例层仍受模拟鼠标影响。若快速双击第二下抬手与系统模拟鼠标释放出现跨帧错位，`_modal_release_guard` 会卡在 `true`，导致 `GameRoot.gd:482` 每帧向 VM 传入 `SetPointerPressed(false)`、`_on_action()` 首行直接 `return`，点击妹妹完全无响应。
2. **链路 B：`CancelChoice()` 分支与夜间事件清理门控竞态**
   - 双击退出走的是 `session.CancelChoice()`（`reply.IsCancel == true`），`EventVm.cs:1592-1594` 跳转到 `block.CancelPc ?? block.EndTarget` 而非显式选项 `"算了"`。
   - 若双击第二下的 `trigger_cancel_pulse` 跨帧与 `SampleMapA ev4` 末尾的 `var68 = 0` / `CE325` 清理产生时序冲突，可能导致 `var68` 仍为 `1` 或 `SampleMapA ev1 page0` 的前置门控（`var25 / var26 / var302 / var295`）未完全复位，使 `pic120` 点击判定失效。
   - 读档会同时重置 VM 实例栈、全部全局变量以及 `_modal_release_guard` / `_action_entry_suppressed`，因此读档后不易复现。

### 2.2 修复方案

1. **在 `godot/project.godot` 关闭移动端触控模拟鼠标（推荐根本消除冲突）**：
   ```ini
   [input_devices]

   pointing/emulate_mouse_from_touch=false
   ```
   （本项目 `InputRouter.gd` 已独立完整处理 `InputEventScreenTouch` / `InputEventScreenDrag` 并主动丢弃 `DEVICE_ID_EMULATION`，关闭系统模拟鼠标可彻底消除 `Input` 单例残留假左键态。）
2. **加固 `_release_modal_guard_if_free()`（`GameRoot.gd:1747`）**：
   - 在 `router != null and router.touch_mode` 时，仅检查 `router.has_active_touches()` 与 `router.is_touch_hold_active()`，不再被 `Input.is_mouse_button_pressed(...)` 卡死。
   - 当 `_on_cancel()` 是由**触控双击抬手**触发（此时 `not router.has_active_touches()`）时，守卫只需拦截当前这一帧（或下一帧自动释放），避免无限期挂起。
3. **核查并加固 `CancelChoice` 路径**：
   - 复现（或写回归测试驱动“卧室点击妹妹 → 选睡觉 → `CancelChoice()`”）时，检查 `F3` 调试行 `Guard=modal:.../act:... Deny=... v295=...` 以及 `var68 / var25 / var26 / var302`；若确认睡觉确认 `opcode 102` 未定义 `421 CancelCase` 且跳过 `"算了"` 分支的清理逻辑，应在 `EventVm.cs` 或该选项取消回调中确认 `ev4` 正常回归 `var68 = 0` 并恢复 `ev1 page0` 可触发态。

### 2.3 验收标准
- 在移动端连续 10 次执行“点击妹妹 → 选睡觉 → 二次确认页双击空白取消 → 立刻再点妹妹”，每次都能正常重新打开妹妹互动菜单，`_modal_release_guard` 在抬手后 1 帧内恢复 `false`。

---

## 3. MOB-03：取消妹妹互动后，妹妹立绘先在位置 A 闪现几帧再跳到位置 B

### 3.1 根因定位

1. **原版脚本的两段式绘制序列**（见 `docs/research/M6_NIGHT_FLOW.md`）：
   - 进入夜间菜单前，`SampleMapA ev4 page0`（`メニュー処理`）在 `pc70` 先绘制了互动/默认位的妹妹立绘 `pic120`（位置 A），随后在 `pc335` 调用 `CE245`（夜间互动菜单）。
   - 取消 `CE245` 返回 `ev4` 后，`ev4` 先执行 `pc338-339`（清 `var9 = 0, var68 = 0`），再在 `pc340` 调用 `CE325`（`■自宅`）。
   - `CE325` 会重新刷新卧室场景并**随机重新抽取妹妹在卧室中的站位（位置 B）**，再重新绘制 `pic120`。
2. **帧间露出旧立绘**：在 `CE245` 退出释放菜单图层到 `CE325` 完成位置抽取与重绘之间存在数帧间隔（脚本内部 `Wait` 或 `PictureStage.gd:1230-1248` `_begin_draw` 的淡入/过渡帧），导致 `pic120` 先以旧坐标/旧立绘在位置 A 渲染数帧，随后被 `CE325` 清除并画到位置 B。

### 3.2 定位验证与修复方案

1. **先用 5 行临时日志确认精确帧序列**（在 `GameRoot.gd` 的 `_consume_picture_queue` 中对 `number == 120` 打印）：
   ```gdscript
   if number == 120:
       print("[PIC120] f=%d act=%d file=%s xy=(%s,%s) fade=%s stack=%s" % [
           frames, action, file,
           str(args[5] if args.size() > 5 else -1),
           str(args[6] if args.size() > 6 else -1),
           str(args[0] if args.size() > 0 else 0),
           session.GetForegroundStackSignature()
       ])
   ```
2. **根据日志结果采用对应最小修复**：
   - **情况 1（最常见：`CE245` 退出时未隐藏旧 `pic120`，隔几帧 `CE325` 才重绘到新位置）**：在 `CE245` 退出并即将返回 `ev4 → CE325` 的过渡期间（或检测到 `CE245` 取消关闭、且 `ev4` 仍在调用栈中尚未进入 `CE325` 重绘前），先将 `pic120` 临时隐藏，待 `CE325` 发出新位置 `Draw` 指令后再显示，彻底消除位置 A 的几帧残影。
   - **情况 2（`PictureStage.gd` `_begin_draw` 在 `pic120` 换图时沿用了旧坐标 `prev["current"]` 做补间过渡）**：若日志显示 `CE325` 紧接着就发了位置 B 的 `Draw` 但因 `duration > 0` 且 `_pose_geometry_differs` 触发了 `state["current"] = prev["current"]` 从位置 A 漂移/闪烁到位置 B，则在 `PictureStage.gd:1233` 对不同文件/新立绘且坐标跳变的情况直接瞬移到目标位置（仅做 alpha 淡入，不做跨半屏几何插值）。

### 3.3 验收标准
- 取消妹妹互动返回卧室时，立绘直接以淡入或单步方式出现在最终位置 B，不再出现“先在位置 A 闪现几帧再消失跳到位置 B”的残影。

---

## 4. MOB-04：一般样式选项框位置偏高，需调整为垂直居中

### 4.1 根因定位

- `godot/presentation_gd/GameRoot.gd:957-965` 的 `_choice_position()` 直接把原版脚本的系统变量 `Sys3 / Sys4` 传给 `ChoiceWindow.open()`。原版脚本为给底部对话框留出空间，将 `Sys4` 设得偏高（约 `220~250`）。
- `godot/presentation_gd/ChoiceWindow.gd:65-75` 的 `window_rect()` 仅在 `position_hint.y == -1.0`（`DEFAULT_POSITION_SENTINEL`）时才回退到垂直中心 `288.0`（`576 / 2`）；其余非 `-1` 值全部直接使用，导致一般样式选项框紧贴上方状态栏。

### 4.2 修复方案

修改 `godot/presentation_gd/ChoiceWindow.gd` 的 `window_rect()`（第 65–75 行），对**非屏外停泊**且**非自定义图片按钮（`override_rects.is_empty()`）**的一般样式选项框，强制将垂直中心对齐画面横向中轴 `288.0`：

```gdscript
func window_rect() -> Rect2:
	var win_size := window_size()
	if is_offscreen_parked():
		return Rect2(position_hint, win_size)
	var center := position_hint
	if is_equal_approx(center.x, DEFAULT_POSITION_SENTINEL):
		center.x = 512.0
	if override_rects.is_empty():
		# 一般样式选项框统一垂直居中，使横向中轴与 1024x576 画面中轴 (y = 288) 重合
		center.y = 288.0
	elif is_equal_approx(center.y, DEFAULT_POSITION_SENTINEL):
		center.y = 288.0
	var top_left := center - win_size * 0.5
	var max_x := maxf(24.0, 1000.0 - win_size.x)
	var max_y := maxf(48.0, 536.0 - win_size.y)
	return Rect2(Vector2(clampf(top_left.x, 24.0, max_x), clampf(top_left.y, 48.0, max_y)), win_size)
```

> **同步检查测试**：改动后需跑一次 `tools/godot_check.sh`，确认 `godot/tests_gd/` 下（如 `DayMenuTest.gd`、`WeekendMenuTest.gd`、`DialogDrawTest.gd`）是否有硬编码断言旧 `Sys4` 坐标的测试用例；若有，同步更新预期为垂直居中 `center.y == 288.0`。

### 4.3 验收标准
- 泡茶（绿茶/咖啡/…/避孕药）、睡觉二次确认（是/算了）等所有一般样式选项框，其上下中点严格落在 `y = 288`（画面正中横向中轴），不再贴近上方状态栏。
- 标题页自定义图片菜单（`override_rects` 非空）与屏外停泊选项（`is_offscreen_parked() == true`）位置完全不受影响。
