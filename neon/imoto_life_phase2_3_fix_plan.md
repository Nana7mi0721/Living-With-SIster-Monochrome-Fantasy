# `imoto-life` 阶段二 / 阶段三修复实施与补丁指南

> **适用基线**：`imoto-life` commit `a5b356d9b83c7bb15248bb1e0830a1e91e527588`
> **前置条件**：先运行 `audit_programs_static.py` 获取零售 `game_data/programs.json` 的实测命中数据，按命中项依次落地本指南中的最小安全补丁，并通过双模式门禁验证。

## 一、阶段二 A：C# 核心 VM 语义定点修复（`godot/core_cs/` & `godot/presentation/`）

### 1. `Program.cs` & `EventVm.cs` — 修正 `Opcode 176`（ContinueLoop，BUG-36）

- **改动文件**：`godot/core_cs/Program.cs:100`、`godot/core_cs/EventVm.cs:534, 827`
- **根因**：`176` 是 `ループ開始へ戻る`（ContinueLoop），不是循环头、无配对 `498`。
- **实施要点**：
  1. 在 `Program.cs:100` 删除 `case 176:`，使块栈仅由 `111/112/102/170/179` 开启。
  2. 在 `EventVm.cs` 抽取循环步进辅助方法 `StepLoopEnd(Instance inst, Program prog, int blockIdx)`（复用 `ControlKind.LoopEnd` 逻辑），并在 `switch (slot.Opcode)` 中新增：

```csharp
case 176: // ContinueLoop: jump back to innermost loop head (or exit counted loop)
{
    if (inst.Loops.Count > 0)
    {
        int topBlock = inst.Loops[^1].Block;
        StepLoopEnd(inst, prog, topBlock);
        continue;
    }
    inst.Pc++;
    break;
}
```

### 2. `AutoEvents.cs` — 按一阶段扫描结果升级电平触发（BUG-25）

- **改动文件**：`godot/core_cs/AutoEvents.cs:39-40, 95-136`
- **实施要点**：
  - 查看 `static_audit_summary.md` 第 1 节：
    - 若存在除 `216, 217` 之外的 `no_loop_linear_BUG25_candidate` 事件，先确认它们不含「清屏/初始化副作用」，再将其加入 `ResumeOnLoad` 或将 `AutoEvents.Sync` 整体改为电平触发（`if (now) { if (!Live(vm, ev.Id)) Start(...); }`），同时把确认为一次性初始化的 `one_shot_or_long_wait_suspect` 事件列入白名单保持边沿触发（参照 `imouto-life` `HANDOFF_DEEP_REVIEW_FIX.md` §5 教训：切勿让类似 `CE 214` 的清屏事件逐帧重启）。
  - 同步在 `Instance` 上记录 `IsParallel = true`，供 `Opcode 124 field 18` 区分前台与并行事件。

### 3. `EventVm.cs` — `Opcode 150` 参数掩码、`151`/`130`/`140`/`180` 求值与 `125` TTL

- **改动文件**：`godot/core_cs/EventVm.cs`
- **实施要点**：
  1. **`Opcode 150`（`:788-799`）**：若一阶段扫描发现 `likely_db_magic > 0`，取消粗暴的 `((uint)raw & 0x0F00_0000) != 0` 跳过逻辑，改为仅对已知打包颜色/哨兵（如 `-1_000_000`）保持 raw，其余统一走 `EvalNum(raw, inst)`；同时在 `Magic.cs` / `EvalNum` 确认是否需要补齐 `1_000_000_000+` DB magic 读取。
  2. **`Opcode 151`（`:1101-1107`）**：首参 `color` 必须保持 raw，第 2 参 `frames` 改为 `EvalNum`：

```csharp
case 151:
{
    int color = slot.Params.Length > 0 ? slot.Params[0] : 0;
    int frames = slot.Params.Length > 1 ? EvalNum(slot.Params[1], inst) : 0;
    Host.ChangeColor(color, frames);
    inst.Pc++;
    break;
}
```

  3. **`Opcode 130`（`:672-682`）与 `Opcode 140`（`:715-718`）**：若一阶段扫描 `op130_teleport_magic_params` / `op140_sound_magic_params` 命中非零，将 `130` 的 `target/x/y/map` 与 `140` 的 `frames/volume/tempo/loopPoint` 改为走 `EvalNum`。
  4. **`Opcode 180`（`:844-852`）**：若 `frames <= 0`，直接 `inst.Pc++; break;` 不再 `Math.Max(1, frames)` 强制阻塞 1 帧（若需保持与旧 Oracle 逐帧一致，可先跑 `TraceDiffTest` 核对）。
  5. **`Opcode 125`（`:1093-1095`）**：若 `op125_auto_keys` 命中非零，给 `AutoKeys` 增加 `int _autoKeysTtl`（注入时设为 `2`，每帧 `BeginFrameInput` 递减至 `0` 时清零）与 `HashSet<int> _autoKeysDelivered`（每帧清空，同帧内每个 `inst.Id` 最多消费一次）。

### 4. `EventVm.cs` — `Opcode 121` 位域补齐与 `CommonSelf` 实时双写（BUG-27, 29）

- **改动文件**：`godot/core_cs/EventVm.cs:260-310, 333-399`
- **实施要点**（按一阶段 `op121_bitfields` 命中项按需全量补齐）：
  1. **`Compound` 上下限（`:345-346`）**：

```csharp
0x600 => Math.Max(current, value), // 下限（Lower bound）
0x700 => Math.Min(current, value), // 上限（Upper bound）
```

  2. **`ExecuteSetVariable` 位域（`:372-399`）**：
     - `bool clamp = (bits & 0x01) != 0;`（结果钳制到 `[-999_999, 999_999]`）
     - `bool realMode = (bits & 0x02) != 0;`（改用 `double` 进行二元与复合运算后截断回 `int`）
     - `int a = (bits & 0x04) != 0 ? args[1] : EvalNum(args[1], inst);`
     - `int b = (bits & 0x08) != 0 ? args[2] : EvalNum(args[2], inst);`
     - 若 `(bits & 0x20) != 0`，对 `a` 再做一次 `EvalNum(a, inst)`；若 `(bits & 0x40) != 0`，对 `b` 再做一次 `EvalNum(b, inst)`；若 `(bits & 0x10) != 0`，对 `target` 先 `EvalNum(args[0], inst)` 解引用。
  3. **`CommonSelf` 实时双写（`:265-267, 303-305`）**：
     - 当 `inst.Program.SelfBase == Program.CommonSelfBase` 时，`StoreNum`/`StoreStr` 写入 `inst.Self` 后立即同步写入 `CommonSelf[inst.EventId]`，并广播给 `Instances` 中所有同 `EventId` 的其他活跃实例；同时移除退出时整包覆盖的 `PersistCommonSelf`。

### 5. `VmController.cs` — `Opcode 211` 预约队列与 `Opcode 124` 缺失字段

- **改动文件**：`godot/presentation/VmController.cs:550-559, 720`、`godot/core_cs/EventVm.cs:653-671`
- **实施要点**：
  1. **`Opcode 211`**：若 `op211_reserve` 命中非零，在 `VmController` 维护 `Queue<(int EventId, int[] Args, string[] Strings)> _reservedEvents`，在 `Reserve(...)` 入队，于每帧前台无活跃非并行事件时弹出并调用 `StartCommonEvent`。
  2. **`Opcode 124` `EngineState`**：在 `VmController.cs` 补录 `_wheelDelta`（每帧从 `InputRouter` 传入后清零）、`_prevPointerX/Y`（计算帧间位移 `dx/dy`），扩展 `EngineState(int field)`：

```csharp
public int? EngineState(int field)
{
    return field switch
    {
        7 => TriState(_pointerPressed, _pointerPressFrame),
        8 => TriState(_rightPressed, _rightPressFrame),
        9 => TriState(_middlePressed, _middlePressFrame),
        10 => _wheelDelta,
        11 => (int)(PointerX - _prevPointerX),
        12 => (int)(PointerY - _prevPointerY),
        20 => (int)PointerX,
        21 => (int)PointerY,
        _ => 0,
    };
}
```

### 6. `WolfRng.cs` / `GameRoot.gd` — 实机开局时钟播种

- **改动文件**：`godot/presentation/VmController.cs`、`godot/presentation_gd/GameRoot.gd`
- **实施要点**：保持 `WolfRng` 无参构造仍为确定性默认种子（保护全部无头测试与 Oracle 对拍）；在 `VmController.cs` 暴露 `public void SeedEventRng(uint seed) => Vm.Rng.SetEventSeed(seed);`，在 `GameRoot.gd` 实机启动（非 `ScriptedInput` 且环境变量 `LWsmf_FIXED_RNG != "1"`）时调用 `vm.SeedEventRng(int(Time.get_ticks_usec()) & 0x7FFFFFFF)`。

## 二、阶段二 B：表现层、输入隔离与渲染管线补齐（`godot/presentation_gd/`）

### 1. `MapLayer.gd` & `GameRoot.gd` — 修复左键输入抢占（高优）

- **改动文件**：`godot/presentation_gd/MapLayer.gd:53, 248-259`
- **最小修复**：
  - 将 `MapLayer.gd:53` 的 `mouse_filter` 改为 `Control.MOUSE_FILTER_PASS`（或在常规 SLG 模式下默认 `MOUSE_FILTER_IGNORE`）。
  - 在 `MapLayer.gd:254-259` 移除无条件的 `accept_event()`（仅当确实消费了相邻格移动且无上层 Picture 热区命中时才处理），避免吞掉送往 `InputRouter._unhandled_input` 的左键事件。

### 2. `PictureStage.gd` — 补齐 `field 2/3` 像素尺寸、混合模式、渐变斜坡与负号图片语义

- **改动文件**：`godot/presentation_gd/PictureStage.gd`
- **实施要点**：
  1. **`picture_state` `field 2/3`（`:200-201`）**：返回真实缩放后像素宽高，而非百分比：

```gdscript
2:
    return int(round(_hit_rect(number).size.x))
3:
    return int(round(_hit_rect(number).size.y))
```

  2. **混合模式（`0=Normal, 1=ADD, 2=SUB, 3=MUL`）**：在 `PictureStage` 预创建 3 个共享 `CanvasItemMaterial`（`_mat_add`、`_mat_sub`、`_mat_mul`），从 `Opcode 150` `head` 的 `(head >> 8) & 0x0F`（需随 `PictureCmd` 透传 `blend` 字段）赋值到 `holder.material`；当 `blend == 3`（乘算）时按 `imouto-life` 实证强制 `alpha = 1.0`。
  3. **帧级淡入淡出与移动补间**：在 `_pictures` 旁维护 `_tweens: Dictionary`（记录 `target_pos`、`target_alpha`、`frames_left`），由 `GameRoot.advance_frame()` 每帧驱动 `stage.tick_frame()`（保证 60Hz 确定性，不依赖 wall-clock `Tween`）；`Erase` 带 `frames > 0` 时先斜坡淡出至 0 再 `queue_free()`。
  4. **负号 / `>=12000` 拉伸修正**：根据 `audit_programs_static.py --assets` 的 `non_fullscreen_assets` 结果，若存在非 `1024x576` 的负号 HUD 图标，移除 `number < 0` 的强制 `SCREEN_SIZE` 覆盖，仅按原图尺寸与坐标锚点排版（仅在明确为背景槽位且图片接近全屏时铺满）。

### 3. `GameRoot.gd` — 接入 `160/161/162` 转场与 `290` 特效

- **改动文件**：`godot/presentation_gd/GameRoot.gd:357-362`
- **实施要点**：
  - 在 `PictureStage` 顶层增加全屏 `ColorRect` 遮罩（用于 `151 ChangeColor` 色调与 `160/161/162` 画面冻结/淡出过渡）。
  - 在 `_on_effect(kind, duration, args)` 中先按 `audit_programs_static.py` 统计到的高频 `kind` 接入：画面震动（`stage.position` 偏移）、闪屏（遮罩 alpha 衰减）与 Picture 偏移 `(dx, dy)`。

## 三、阶段三：UI/UX 体验移植与本地验证门禁

### 1. 弹窗释放守卫（借鉴 `imouto-life/godot/scripts/GameView.gd:410, 700-712`）

- 在 `GameRoot.gd` 新增 `var _modal_release_guard := false`：
  - 关闭故障弹窗（及后续新增的返回标题/覆写存档确认弹窗）时设 `_modal_release_guard = true`。
  - 在 `advance_frame()` 开头检测：当 `_modal_release_guard` 为真时，若 `Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT)` 或 `router.pointer_pressed` 仍为真，则向 VM 传递 `false` 并屏蔽 `action_pressed`；直到全部鼠标按键释放后再解除守卫。

### 2. 便利热键与消息回顾（`GameRoot.gd` & `MessageWindow.gd`）

- **`F11` 全屏切换**：在 `_unhandled_input` 记录切换前的 `DisplayServer.window_get_position()` 与 `window_get_size()`，在 `WINDOW_MODE_WINDOWED` 与 `WINDOW_MODE_FULLSCREEN` 间切换。
- **`F4` 消息回顾（Backlog）**：在 `MessageWindow.open()` 中将纯文本推入 `GameRoot` 维护的 200 条内存环形数组（不写存档），按 `F4` 弹出只读 `ScrollContainer` 浮层并开启 `_modal_release_guard`。
- **前端「返回标题」确认弹窗**：复用 `restart_after_fault()` 相同的 `session.Rebuild()` + `session.StartTitle()` 链路，加确认弹窗防误触。

### 3. 编码温和回退（可选防御，保持 UTF-8 优先）

- 在 `converter/reader.py:84-89` 与 `LegacySaveImporter.cs:104-112` 保持严格 UTF-8 优先；仅在捕获 `UnicodeDecodeError` 时依次尝试 `cp932`、`gbk`，仍失败再回退 `utf-8 replace` 并保留告警记录。

### 4. 本地单测与双模式门禁清单

1. **新增 C# 单测**（`godot/tests/VmTest.cs`）：
   - `Opcode 176` 不开启新循环块、在计数循环 `179` 内正确递减并跳回循环头；
   - `Opcode 121` `0x01` 钳制、`0x02` f64、`0x10/0x20/0x40` 二阶指针、`0x600/0x700` 上下限与 `CommonSelf` 并发双写；
   - `Opcode 151` 首参 RGB 保持 raw + 第 2 参 `frames` 变量求值；
   - `Opcode 125` 2 帧 TTL 与同帧每协程单次消费。
2. **新增 GDScript 测试**（`godot/tests_gd/`）：
   - `MapLayer` 可见时点击地图内坐标，`InputRouter` 仍能正常收到左键点击（验证无输入抢占）；
   - `PictureStage` `field 2/3` 返回像素宽高、`blend 0..3` 材质绑定与帧级淡入淡出。
3. **全量门禁与导出包冒烟**：

```bash
bash tools/godot_check.sh
LWsmf_RENDER=1 bash tools/godot_check.sh
bash tools/package_release.sh
```

随后将 `dist/Lwsmf-win64` 拷贝至无源码的独立目录，双击 `Lwsmf.exe` 完成首次向导转换、进图点击 Picture 按钮、存档/读档与 `F11`/`F4` 实机冒烟。
