# LWSMF 移动端四项交互问题核查与修复闭环报告（2026-11-26）

> **关联诊断**：[lwsmf_mobile_4issues_diagnosis_and_fix_plan.md](lwsmf_mobile_4issues_diagnosis_and_fix_plan.md)  
> **修复范围**：`godot/project.godot`、`godot/presentation_gd/InputRouter.gd`、`godot/presentation_gd/GameRoot.gd`、`godot/presentation_gd/ChoiceWindow.gd`、`godot/core_cs/EventVm.cs`、`godot/core_cs/GameSession.cs`、`godot/tests_gd/InputRouterTest.gd`、`godot/tests_gd/DialogDrawTest.gd`、`godot/tests_gd/GameRootTest.gd`

---

## 1. 根因核查结论

对 `lwsmf_mobile_4issues_diagnosis_and_fix_plan.md` 中的四项定位逐一对照源码与 `game_data/programs.json` 字节码实测核验：

| 编号 | 核查结论 | 实证与修正补充 |
| :--- | :--- | :--- |
| **MOB-01** | **属实** | 数值/技能界面 `SampleMapA ev6 p0` 在悬停技能图标（`op 124 field 8`）后通过 `op 123 [self5, 17]`（`pc=881`）持续轮询 `MASK_OK` 推进读条。触控按住期间 `step_confirm_hold` 仅在 `MessageWindow` 打开时激活，`held_mask()` 与 `left_frame_pressed()` 均未纳入触控长按状态，导致松手仅触发一次 `TOUCH_PULSE_TTL_FRAMES=4` 的单击脉冲。另在实现时发现：`_is_touch_hold_steady()` 必须同时放行 `_touch_pointer_held`，否则置位 `_touch_gesture_consumed = true` 后下一帧会立刻自清。 |
| **MOB-02** | **属实** | `godot/project.godot` 未关闭 `input_devices/pointing/emulate_mouse_from_touch`（默认 `true`），双击退出睡觉二次确认框 `CE328 ["是", "算了"]` 时 `_on_cancel()` 挂起 `_modal_release_guard = true`，而 `_release_modal_guard_if_free()` 直接轮询 `Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT)`，受系统触控模拟鼠标抬手错位/残留影响可能卡死守卫，进而屏蔽后续对妹妹 `pic 120` 的点击。 |
| **MOB-03** | **现象属实，根因精确修正** | 核查 `SampleMapA ev4 p0` 与 `CE325` 发现：`ev4 pc72` 实际为 `op 150 [65538, 120, 0]`（立即擦除 `pic 120`），并非保留旧图；真正造成位置 A 闪现十余帧再消失并跳至位置 B 的原因是：`CE328 pc1010/1019`（睡觉确认选“算了”或取消）及 `CE245 pc640/755/764` 在退出 `CE245` 前**先调用了一次 `CE325`**（绘制 `pic 120` 于位置 A），随后经 `CE328 Wait(1)` 与 `CE245 BreakEvent` 返回 `ev4 pc336..340`，而 `ev4 pc340` **无条件再次调用 `CE325`**——先执行 `CE325 pc12 Wait(15)`（使位置 A 滞留 15 帧），再于 `pc15` 擦除 `99..199`（立绘消失），最后在 `pc36..48` 重新随机抽位绘制到位置 B。 |
| **MOB-04** | **属实** | `GameRoot._choice_position()` 将系统变量 `Sys3/Sys4` 直接传入 `ChoiceWindow.open()`，`ChoiceWindow.window_rect()` 仅在 `position_hint.y == -1.0` 时居中，其余正值（如 `Sys4 = 220`）直接采用，导致一般样式选项框整体偏高。 |

---

## 2. 已落地的修复措施

1. **MOB-01（非对话长按转持续确认持键）**：
   - 在 `InputRouter.gd` 新增 `TOUCH_POINTER_HOLD_DELAY_FRAMES := 12`（约 `200ms`）、`_touch_pointer_held` 与 `step_touch_pointer_hold(eligible)`，并在 `GameRoot.advance_frame()` 中仅对非标题/非弹窗/非选项/非对话/非存档界面推进计时。
   - 将 `_touch_pointer_held` 接入 `held_mask()`、`left_frame_pressed()`、`action_input_held()` 与 `reconcile_pointer_buttons()`，并在 `_is_touch_hold_steady()` 放行 `_touch_pointer_held`、在 `_handle_screen_drag()` 超出 `TOUCH_HOLD_CANCEL_DIST`、松手、`clear_touch_state()`、`trigger_cancel_pulse()` 与切回非触控模式时清零。
   - `consume_touch_ok()` 仅清除一次性 `_click_latch / _touch_ok_ttl`，保持长按期间持续返回 `MASK_OK` 与 `EngineState(7) = 2 -> 1 -> ... -> 0`，松手不再补发单击。

2. **MOB-02（关闭触控模拟鼠标 + 触控模式守卫加固）**：
   - 在 `godot/project.godot` 添加 `[input_devices]` 与 `pointing/emulate_mouse_from_touch=false`，消除 `Input` 单例的触控模拟鼠标残留状态。
   - 在 `GameRoot._release_modal_guard_if_free()` 中，当 `router != null and router.touch_mode` 时跳过 `Input.is_mouse_button_pressed(...)` 检查，改由 `router` 真实触控与按键状态决定释放，杜绝双击取消后守卫假卡死。

3. **MOB-03（消除 `ev4 pc340` 对 `CE325` 的重复刷新）**：
   - 在 `EventVm.cs` 增加轻量的 `ShouldSkipCall` 与 `OnInstanceFinished` 回调；在 `GameSession.cs` 的非脚本交互模式（`!ScriptedInput && _map?.Id == 1005`）下，当 `CE325` 已于 `SampleMapA ev4 pc336` 等待 `CE245` 返回前完成本次卧室刷新（且之后未再打开消息/选项或其他非收尾公共事件）时，跳过 `ev4 pc340` 的重复 `CE325` 调用，使卧室立绘单次落位不再先现位置 A 再闪至位置 B。

4. **MOB-04（一般样式选项框垂直居中）**：
   - 修改 `ChoiceWindow.window_rect()`：在非屏外停泊（`not is_offscreen_parked()`）且非自定义图片按钮（`override_rects.is_empty()`）时，统一将 `center.y` 设为 `288.0`，使其横向中轴与 `1024×576` 画面中轴严格重合，同时保留 `Sys3` 横向锚点、`override_rects` 标题图片菜单及屏外停泊行为。

---

## 3. 回归测试与双模式门禁验证

- **新增/更新回归测试**：
  - `godot/tests_gd/InputRouterTest.gd`（第 30 项）：验证 `step_touch_pointer_hold` 阈值、`EngineState(7)` `2 -> 1 -> 1 -> 0`、松手不补发点击以及拖动超距取消。
  - `godot/tests_gd/DialogDrawTest.gd`：更新并校验标准 `ChoiceWindow` 保持 `Sys3 X` 且 `center.y == 288.0`。
  - `godot/tests_gd/GameRootTest.gd`（`_test_mobile_4issues_regression`）：端到端覆盖 `MOB-01`（局内长按与松手）、`MOB-02`（夜间卧室点击妹妹 → 选睡觉 → 触控双击空白取消 → 重复两次后仍可重新打开 `CE245` 且 `_modal_release_guard == false`）、`MOB-03`（`pic 120` 仅单次出现，无 A→隐藏→B 闪烁）与 `MOB-04`（`ChoiceWindow` 居中于 `y = 288`）。
- **构建与门禁结果**：
  - `dotnet build godot/Lwsmf.csproj -c Release`：0 warnings, 0 errors。
  - `bash tools/godot_check.sh`（无头模式 38 项全量门禁，含 M13 slot 1/8 与 Oracle 对拍）：`GODOT_GATE_OK`。
  - `LWsmf_RENDER=1 bash tools/godot_check.sh`（OpenGL3 渲染回归门禁，含 `DIALOG_DRAW_TEST_OK (windowed)`、`PICTURE_ROUNDTRIP_TEST_OK (windowed)` 与 `PERFORMANCE_FRAME_STRICT_OK`）：`GODOT_GATE_OK`。
