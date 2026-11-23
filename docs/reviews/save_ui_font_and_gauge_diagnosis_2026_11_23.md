# LWSMF 存档界面异常、自动存档、字体清晰度与格子进度条错位根因报告（Commit `78c4e49a`）

**排查基线**：commit `78c4e49aabc62de47143a295cc92d4c204b0f818` + 5 张实机截图（存档页、标题读档页、姊妹作弹窗参考、警戒度进度条）  
**结论概览**：你反馈的 8 个现象已**全部定位到确切源码行**。其中最核心的“标题页误弹覆盖”和“点第 6 页 `save24` 却弹 `存档槽 04`”同源于**一个 600 偏移量漏加 Bug**（`Magic.SelfArgBase = 600`）；“右键失效 + 按 `Esc` 关掉整个存档页”同源于 `MOUSE_FILTER_STOP` 拦截与 `_modal_release_guard` 漏检键盘释放；“保存无反应”源于伪造单帧点击脉冲打空。

---

## 1. 问题总览与根因速查表

| 编号 | 现象 | 精确根因位置（`file:line`） | 严重度 |
| :--- | :--- | :--- | :---: |
| **S-01** | 标题界面（读档模式）点击存档却弹“覆盖存档？” | `Magic.cs:24`、`GameSessionController.cs:38`、`GameRoot.gd:1908` | **P0** |
| **S-02** | 游戏内点第 6 页空槽 `save24`，弹窗却显示“存档槽 04 已有数据” | `GameRoot.gd:1910, 1913-1914`（同源漏加 `SelfArgBase = 600`） | **P0** |
| **S-03** | 点击存档槽 / 确定覆盖后无反应、界面无变化 | `GameRoot.gd:411, 1926-1934`、`InputRouter.gd:80-83`（伪造 1 帧脉冲打空） | **P0** |
| **S-04** | 二次确认弹窗无法右键回退；按 `Esc` 直接关掉整个存档界面 | `PlayOverlays.gd:280, 722`、`GameRoot.gd:148, 484-487, 1102`、`InputRouter.gd:72` | **P1** |
| **S-05** | 弹窗可读性差、缺游戏内读档与空槽分支 | `PlayOverlays.gd:716-811`、`GameRoot.gd:1899-1934` | **P1** |
| **S-06** | 自动存档功能核查（原版晚饭后自动存槽 1） | `GameSession.cs:674, 816-841`、`docs/engine/OPCODE_220_SAVE.md:192-206` | **P1** |
| **S-07** | 字体又变回系统字体；如何在高分屏下让原版字体保持清晰 | `UiSkin.gd:215-225, 245-286`、`project.godot:24-26` | **P1** |
| **S-08** | “警戒度”格子进度条黑块 `■` 与空框 `□` 逐格左漂未对齐 | `PictureStage.gd:64-78, 453-466`、`UiSkin.gd:215-219` | **P2** |

---

## 2. 存档界面五大 Bug 逐项深挖与修复方案

### S-01 & S-02：标题页误弹“覆盖存档？” + 弹窗槽号与实际选中错位（同源根因：漏加 `Magic.SelfArgBase = 600`）

* **源码铁证**：
  * `godot/core_cs/Magic.cs:24` 定义公共事件自变量 `cself[k]`（`1600000 + k`）在 `SelfVars` 数组中的真实下标为 **`Magic.SelfArgBase + k = 600 + k`**（下标 `0..99` 是地图事件自变量区，在公共事件 `CE131` 中恒为 `0`）。
  * 但 `godot/presentation/GameSessionController.cs:38` 写成了：
    ```csharp
    public int GetSaveSlotMode() => GetEventSelf(131, 0); // 错！应为 Magic.SelfArgBase + 0 (600)
    ```
  * 同时 `godot/presentation_gd/GameRoot.gd:1910, 1913` 写成了：
    ```gdscript
    var row := _save_load_hit_slot(router.hover_position) if is_pointer else session.GetEventSelf(131, 10) # 错！应为 610
    var page := maxi(0, session.GetEventSelf(131, 11))                                                     # 错！应为 611
    ```
* **直接后果**：
  1. `GetSaveSlotMode()` 永远读到 `0`（保存模式），且 `GameRoot.gd:1908` 未检查 `phase == "title"`，于是**标题页读档界面（`cself[0] == 1`）恒被当成保存模式**，点 `save3` 直接弹“覆盖存档？存档槽 03 已有数据”。
  2. `page` 永远读到 `0`！当你翻到第 6 页（`save21..24`，均为 `没有存档`）点击第 4 行 `save24`（`row = 3`）时，代码算出 `slot = 0 * 4 + 3 + 1 = 4`，去查 `save_04.json` 发现有数据，于是对着空栏 `save24` 弹出了“存档槽 04 已有数据”。
* **最小修复**：
  * 在 `VmController.cs` / `GameSessionController.cs` 中明确提供 `GetCommonEventSelf(int eventId, int argIndex)`，内部自动加上 `Magic.SelfArgBase (600)`；
  * `GetSaveSlotMode()` 改为读 `600`（并在 `GameRoot.gd` 侧叠加 `var is_load_mode := (phase == "title") or (session.GetSaveSlotMode() != 0)` 双保险）；
  * `GameRoot.gd:1910, 1913` 分别改读 `610`（`cself[10]`）和 `611`（`cself[11]`），同时建议用 `cself[27]`（每页槽数，默认 `4`）替换硬编码 `4`。

### S-03：点击存档槽 / 确定覆盖后无反应与无变化

* **源码根因**：
  * `GameRoot.gd:1926-1934` 在确认覆盖后采用“瞬移 `hover_position` + 调 `router.trigger_action_pulse("mouse")`”的脆弱方式试图把点击回放给 `CE131`：
    1. 按钮 `pressed` 在**左键松开**时触发，`trigger_action_pulse()` 只设了 `_click_latch = true`（`InputRouter.gd:80-83`）未置 `pointer_pressed = true`，且 `_pending_save_overwrite_confirmed` 在当帧末尾（`GameRoot.gd:411`）立刻被清零；
    2. 只要鼠标在弹窗按钮处微动 1 像素，`hover_position` 就会被真实鼠标位置覆写（不再落在原存档槽上）；
    3. 只要 `CE131` 当帧未恰好处于 `op123`/`op124` 轮询点，这 1 帧脉冲就直接打空，导致**既未落盘也没有重绘刷新**。
* **根治方案（兼顾游戏内直接读档需求）**：
  * 放弃“伪造一帧点击脉冲”的脆弱中转，由引擎层在弹窗确认后直接执行确定性动作：
    * **保存到此槽**：调用 `session.SaveGameToSlot(slot)` 落盘 `save_NN.json` + 同步写入 `System.json` 槽位标记（同 `CE131 pc230..232`）+ 截取干净场景图写入 `saveN.jpg` + 触发 `CE94`（`X[移]セーブ·ロード画面描画`）立即重绘当前页槽位并弹出“已保存至存档槽 NN”Toast。
    * **读取此存档**：直接调用 `session.LoadGameFrom(slot_path)`（自动触发 `_on_load_completed` 清理存档页与弹窗，无缝切入存档画面）。

### S-04：二次确认弹窗无法右键回退，且按 `Esc` 关闭整个存档界面

* **右键失效根因**：
  * `PlayOverlays.gd:722` 将全屏模态容器 `_save_confirm_root.mouse_filter` 设为 `Control.MOUSE_FILTER_STOP`。在 Godot 4 输入链中，GUI 命中 `MOUSE_FILTER_STOP` 会直接截断鼠标事件，右键根本到不了 `InputRouter._unhandled_input()`（`InputRouter.gd:72`），导致 `GameRoot._on_cancel()`（`:484-487`）完全收不到右键信号；同时 `PlayOverlays._input()`（`PlayOverlays.gd:280`）首行仅放行 `_backlog_open` 的键盘事件，忽略了右键。
* **按 `Esc` 连带关闭整个存档界面的根因**：
  * Godot 4 `_unhandled_input` 自底向上派发，子节点 `InputRouter`（`GameRoot.gd:148`）先于 `GameRoot` 收到 `KEY_ESCAPE`，触发 `GameRoot._on_cancel()` 关闭弹窗并调用 `_arm_modal_release_guard()`。
  * **但 `_release_modal_guard_if_free()` 只检测鼠标左/中/右键释放，完全未检测键盘 `KEY_ESCAPE`/`KEY_X`/`KEY_BACKSPACE` 是否松开**！人手按一次 `Esc` 持续 3～9 帧，下一帧 `advance_frame()` 见鼠标未按住便立即解除保护，`router.held_mask()` 立刻把仍按着的 `KEY_ESCAPE` 以 `MASK_CANCEL (11)` 喂给 `CE131`，导致 `CE131` 直接退出整个存档界面。
* **最小修复**：
  1. 在 `PlayOverlays._input(event)` 中增加统一模态取消处理：当 `is_modal_open()` 为真且收到 `MOUSE_BUTTON_RIGHT`（`pressed == true`）或 `KEY_ESCAPE`（`pressed == true and not echo`）时，调用 `close_topmost_modal()`、`get_viewport().set_input_as_handled()` 并发出信号通知 `GameRoot` 开启释放保护。
  2. 在 `GameRoot._release_modal_guard_if_free()` 中补上取消键物理释放检查（`Input.is_key_pressed(KEY_ESCAPE) or Input.is_key_pressed(KEY_X) or Input.is_key_pressed(KEY_BACKSPACE)`），在按键未彻底松开前持续屏蔽 `MASK_CANCEL`（`router.held_mask` 同步掩掉取消位），彻底杜绝 `Esc` 穿透关闭底层存档界面。

### S-05：参考姊妹作重构存档操作弹窗（样式 + 4 象限交互矩阵）

对照你提供的姊妹作截图（`{AEED3B1E...}.png`、`{CB663AA3...}.png`），建议将 `PlayOverlays.gd:716-811` 统一升级为**存档槽操作弹窗**：

* **视觉样式规范**：
  * 居中宽卡片 `560 × 240`（逻辑坐标），深灰蓝实底 `Color(0.106, 0.114, 0.145, 0.96)` + 1px 清晰纯白边框 `Color(0.92, 0.94, 0.98, 0.90)`，外层半透黑遮罩 `alpha = 0.72`；
  * 顶部居中粗体大标题 `存档槽 N`（26px 白字），下方居中说明文案（18px 浅灰字，如 `该槽位已有存档。` / `该槽位为空。`）；
  * 底部居中 `HBoxContainer`（间距 `18px`）放置等宽大按钮（`min_size = Vector2(148, 44)`），默认深底细灰框、聚焦/悬停高亮白框与浅底色，支持方向键/鼠标/右键/`Esc`。
* **4 种场景交互矩阵**：

| 打开入口 | 点击槽位状态 | 弹窗行为与按钮列表 |
| :--- | :--- | :--- |
| **标题界面（读档模式）** | **空槽** | **不弹窗，无任何反应**（直接拦截返回） |
| **标题界面（读档模式）** | **有数据槽** | 弹窗标题 `存档槽 N` / `该槽位已有存档。`，按钮：**`[读取此存档]`**、**`[取消]`** |
| **游戏内（存档模式）** | **空槽** | 弹窗标题 `存档槽 N` / `该槽位为空。`，按钮：**`[保存到此槽]`**、**`[取消]`**（不显示 `读取此存档`） |
| **游戏内（存档模式）** | **有数据槽** | 弹窗标题 `存档槽 N` / `该槽位已有存档。`，按钮：**`[保存到此槽]`**、**`[读取此存档]`**、**`[取消]`** |

---

## 3. 自动存档功能核查结论（S-06）

* **代码已有完整底层支持**：
  * 原版脚本 **`CE435`（`オートセーブ`）** 共 8 条指令（见 `docs/engine/OPCODE_220_SAVE.md:192-206`）：等 1 帧 → `op1000 [0] 'save/save1.jpg'` 截屏存槽 1 缩略图 → `self12 = 1` → `op222` 写 `System.sav` 的 `var0 = 1` → `op220 [0, 1600012]` 写槽 1。
  * C# 核心 `GameSession.HandleSaveLoad`（`GameSession.cs:816-841`）已实现 `case 220`（`slot = 1` → `save_01.json`）与 `case 222`；`GameRoot._on_pro_feature` 已实现 `save/save1.jpg` 截图落盘。
* **存在一处隐蔽拦截风险（解释为何槽 1 可能为空，`needs-runtime` 复测）**：
  * `GameSession.SaveGameTo()`（`GameSession.cs:674`）开头硬卡了：
    ```csharp
    if (Phase != "bedroom" || _map == null || _mapRuntime == null) return false;
    ```
  * 如果晚饭剧情调用 `CE435` 时，当前 `Phase` 处于 `"event"` 而非 `"bedroom"`，自动存档会被这行守卫**静默拒绝**（且屏幕上没有任何提示）！
  * **修复建议**：将 `opcode 220` 触发的 `SaveGameTo()` 守卫收窄为仅拦截 `Phase == "title" || Phase == "boot" || Phase == "failed" || _map == null || _mapRuntime == null`，允许在地图事件（含晚饭后 `CE435`）期间正常落盘，并在日志记录 `[AutoSave] slot=1 ok/fail` 便于实机核验。

---

## 4. 字体回退原因与“原版字体 + 全分辨率/高 DPI 清晰渲染”方案（S-07）

* **为什么画面里又变成了微软雅黑而非原版 `Lolita.ttf`**：
  * `UiSkin.gd:245-286` 本身仍优先加载 `Lolita.ttf`（文件目录 → `media.pak` → 系统字体），未改成默认系统字体。
  * 你截图 1/2 中呈现的是 `Microsoft YaHei UI`（`UiSkin.gd:16`），说明你本次运行的程序目录下 `work/all/Lolita.ttf`、`Data/Lolita.ttf` 以及 `media.pak` 内**均未打包/放置 `Lolita.ttf`**，触发了第 284 行的系统字体静默回退（可在游戏内按 `F3` 看 `Font=` 行确认是否为 `system`）。
* **为什么大窗口下显得更清晰，以及如何让原版 `Lolita.ttf` 同样锐利清晰**：
  * 清晰的真正原因是 `project.godot:24-26` 将窗口拉伸模式从 `viewport` 改成了 **`window/stretch/mode="canvas_items"`** + 默认 `2048×1152`——Godot 会按窗口物理像素实时光栅化动态字体，不再把 1024×576 低分画布放大糊屏。
  * 当前 `UiSkin.gd:215-225` 的 `_configure_font_file()` 对 `Lolita.ttf` 仅设置了 `subpixel_positioning = DISABLED` 与 `hinting = HINTING_LIGHT`。要让原版 `Lolita.ttf` 在 1080p / 2K / 4K 与 125%～200% Windows DPI 下都保持高清晰度，建议：
    1. **确保 `Lolita.ttf` 随 `media.pak` 或 `Data/Lolita.ttf` 稳定分发**（按 `F3` 确认 `Font=file:Lolita.ttf` 或 `pak:Lolita.ttf`）；
    2. **在 `UiSkin.gd:215-225` 显式优化动态光栅化参数**（兼顾 CJK 大字库零卡顿与高 DPI 锐度）：
       * `ff.oversampling = 0.0`（强制跟随 `canvas_items` 物理缩放比与 OS HiDPI 自动过采样，切勿锁死 `1.0`）；
       * `ff.antialiasing = TextServer.FONT_ANTIALIASING_GRAY`（灰度抗锯齿，边缘均匀无彩边）；
       * `ff.hinting = TextServer.HINTING_NORMAL`（在 1x/1.5x/2x 屏上对细笔画汉字对齐像素网格，显著提升锐利度）；
       * `ff.generate_mipmaps = false`（UI 文本禁用 mipmap 避免缩小变糊）；
       * 保持 `multichannel_signed_distance_field = false`（MSDF 虽可无限缩放，但对中文字库首次遇到新汉字会造成微卡顿且细笔画描边易发虚；`canvas_items` + 动态 `oversampling = 0.0` 是 Godot 4 中 CJK 原版 TTF 清晰度与性能的最优解）。
    3. **在 `project.godot` 开启 `display/window/dpi/allow_hidpi=true`**，确保高 DPI 笔记本缩放（125%/150%/200%）下字体直接按物理像素渲染而非被 Windows DWM 拉伸变糊。

---

## 5. “警戒度”格子进度条黑块与方框错位根因与修复（S-08）

* **现象特征（见截图 5 `{043AF524...}.png`）**：
  * 底层 10 个空心方框 `□□□□□□□□□□` 与顶层 7 个实心黑块 `■■■■■■■` 从左到右呈**线性累积左漂**（第 1 格基本对齐，第 2 格微偏，到第 7 格已左移近半格）。
* **源码根因**：
  * 原版 WOLF/DxLib 绘制全角符号时使用固定等宽步进；但 Godot 的 `PictureStage.gd:77-78` 与 `:463` 直接调用 `draw_string()` / `get_string_size()` 按当前字体字形表推进：
    1. 在回退字体 `Microsoft YaHei UI`（或 `Lolita.ttf` 缺失 `■`/`□` 触发系统字体回退）中，**`■` (U+25A0) 与 `□` (U+25A1) 的水平步进宽度（advance）并不相等**（通常相差 1～2px）；
    2. 两层文本图片分别整串绘制，单格 1～2px 的宽度差乘以 7 格后直接累积成肉眼可见的错位。
* **最小修复**：
  * 在 `PictureStage.gd` 的 `_layout_text()`（`:463`）与 `_PictureTextCanvas._draw()`（`:66-78`）中，检测包含进度条几何方块（`■`/`□`/`◆`/`◇`/`●`/`○`）的文本片段：
    * 计算统一的等宽格距 `var cell_step := maxf(draw_font.get_string_size("■", ..., px).x, draw_font.get_string_size("□", ..., px).x)`（或固定 `ceilf(float(px))`）；
    * 逐字按 `pos.x + i * cell_step` 居中对齐绘制每个 `■`/`□`（并用同样的 `cell_step * count` 作为布局宽度），即可保证无论使用 `Lolita.ttf` 还是任意系统字体，黑块与空框在任意分辨率下都 **0 像素偏差严丝合缝**。

---

## 6. 推荐修复顺序

1. **第一优先级（立刻修通存档核心可用性）**：修复 **S-01 / S-02**（`Magic.SelfArgBase = 600` 偏移）+ **S-03 / S-05**（姊妹作样式弹窗 + 确定性保存/读档直调 + 4 象限空槽分支）+ **S-04**（`PlayOverlays._input` 右键/Esc 统一关闭 + `_release_modal_guard_if_free` 补键盘取消键释放检查）。
2. **第二优先级（数据安全与视觉还原）**：放宽 **S-06** `SaveGameTo` 的 `Phase` 守卫验证晚饭后 `CE435` 自动存档落盘（`needs-runtime`）；补齐 **S-07** `Lolita.ttf` 分发检查 + `allow_hidpi=true` + `HINTING_NORMAL`；落地 **S-08** `■`/`□` 等宽步进绘制消除进度条错位。
