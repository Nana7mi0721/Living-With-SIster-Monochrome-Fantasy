# `LWSMF`（`4e46498`）标题、键鼠交互与深度缺陷对照审计报告

- **审阅对象**：`Living-With-SIster-Monochrome-Fantasy`（commit `4e46498de030eb77f5a5784724a649a3b6571cee`）
- **参考蓝本**：姊妹作 `Imouto-Life-Monochrome`（commit `4f77b1283effcbea8b32452f99d2dd9e01c7b9f1`）
- **核验进展**：上一轮的 `L1`（槽位 Z-Order `move_child`）、`L2`（`TextureRect` `PRESET_FULL_RECT`）、`L3`（`NinePatchRect` 九宫格）与 `L7`（`word_effect` 回退）均已在 `4e46498` 落地；当前核心瓶颈转移至**标题菜单闭环、键鼠操作体系**以及若干**尚未接线的底层 Host 空桩**。

---

## 一、标题背景错用姊妹作 & 标题三选项缺失

### 1. 标题背景误用为姊妹作《妹！せいかつ ～モノクローム～》（P0 资产错误）
- **现象**：实机截图显示双马尾水手服妹妹 + 左上角「妹！生活 ~ Monochrome ~」，这是前作 `Imouto-Life-Monochrome` 的 `SystemGraphic/TitleGraphic.png`，而非本作《妹相随 黑白世界的缤纷冒险》的黑白抱膝标题图（`010_标题_三行菜单.png`）。
- **根因**：`TitleMap.mps` Event 2 `pc87` 请求本作的 `Picture/title01.jpg`，因 `work/all` 与 `media.pak` 中缺失该图，触发了 `PictureStage.gd:435-436` 兜底 `SystemGraphic/TitleGraphic.png`；而目录中的 `TitleGraphic.png` 被误拷成了姊妹作的文件。
- **修复**：从**本作（LWSMF）**的原版 `Data/Picture.wolf` / `SystemGraphic.wolf` 提取正确的 `Picture/title01.jpg` 与 `SystemGraphic/TitleGraphic.png`，替换误拷文件并重打 `media.pak`。

### 2. 标题左下角三选项（「开始咯!!!!! / 继续咯!!!!! / 还是算了......」）完全不显示（P0）
- **根因**：
  1. `godot/core_cs/AutoEvents.cs:36` 将 `CE43` 写进了 `Skipped = { 43 }`。在 WOLF 原版中，`CE42` 负责画背景并将 `Sys3/Sys4` 设为 `-1000` 隐藏原生 `102` 框，而真正负责在左下角画出三张选项图片按钮的并行事件正是 **`CE43`**。
  2. 缺少当前选项游标到 **`Sys20`** 的实时镜像，以及游标变化时重播 `CE43` 的机制。
- **借鉴姊妹作实现（`Imouto-Life-Monochrome` `auto_events.rs:165` + `game.rs:5068-5108`）**：
  1. 将 `43` 从 `AutoEvents.Skipped` 移除，在 `AutoEvents.cs` 新增 `SetTitleMenuActive(bool active)` 门控：仅当标题 `102` 选择框开启时允许 `CE43` 运行（防止其他 `v0 == 0` 场景误画标题按钮）。
  2. 标题选项开启期间，每帧将当前选中下标写入 `vm.Vars.SetSys(20, cursor)`；每当光标因键盘、滚轮或鼠标悬停改变时，立即调用 `autoEvents.Restart(43, vm)` 重绘三按钮亮/暗态。
  3. 标题选项关闭后，`SetTitleMenuActive(false)` 自动停止 `CE43`。
  4. **切忌硬抄前作常量**：先查 `game_data/programs.json` 确认本作 `CE42` 的 `102` 选项文本（是 `["1","2","3"]` 还是 `["menu1","menu2","menu3"]`）以及 `CE43` 所用图片槽位号，热区直接读 `PictureStage.picture_rect(slot)`。

---

## 二、键鼠操作体系 5 处核心缺陷（对照姊妹作修复）

| 编号 | 严重度 | `LWSMF` 现状与证据 | 姊妹作已验证解法（参考位置） | 修复要点 |
| --- | --- | --- | --- | --- |
| **K1** | **P0** | **点选项外空白处直接确认选项**：`InputRouter.gd:85-86` 左键无条件发 `action_pressed` → `GameRoot.gd:259-263` 看到 `choice.is_open` 直接 `choice.confirm()` | `Imouto-Life-Monochrome`（BUG-22）：鼠标点击必须命中选项行热区才确认，点空白处忽略 | 区分键盘确认与鼠标点击；鼠标左键仅在命中 `choice.hover_at(pos) >= 0` 时调用 `choice.confirm()`，未命中时不做任何确认 |
| **K2** | **P0** | **屏外标题 `102` 选项无鼠标热区**：`Sys3/Sys4 = -1000` 把 `ChoiceWindow` 移出屏幕后，`ChoiceWindow.row_rects()` 全在屏外，`hover_at()` 恒返回 `-1` | `Imouto-Life-Monochrome` `choice.rs:282-298`：标题图片菜单使用三个按钮图片矩形做鼠标悬停与点击命中 | `ChoiceWindow.gd` 支持注入外部 `override_rects`（来自 `PictureStage.picture_rect(slot)`）；标题页悬停命中时更新 `selected` → 同步 `Sys20` → 重播 `CE43` |
| **K3** | **P1** | **缩放/全屏黑边下鼠标坐标偏移**：`InputRouter.gd:56, 65` 直接用 `event.position` 当作 `1024×576` 逻辑坐标 | `Imouto-Life-Monochrome` `GameView.gd:1007`：`_screen_to_logical` 做 canvas 逆变换 | 统一通过 `GameRoot.get_global_transform_with_canvas().affine_inverse() * event.position` 换算逻辑坐标后再写入 `hover_position` 与 `vm.SetPointer` |
| **K4** | **P1** | **缺方向键连发 & 按压瞬间坐标锁存**：`InputRouter.gd:103` 过滤 `not event.echo`，长菜单无法长按滚动；左键按下未锁存坐标，帧内手微移易脱靶 | `Imouto-Life-Monochrome` `GameView.gd:748`（`mouse_latch_pos`）+ `game.rs:5059`（`update_key_repeat`） | 允许方向键在 `choice.is_open` 时按固定间隔连发；左键按下瞬间锁存 `click_latch_pos`，本物理帧按该坐标做命中 |
| **K5** | **P1** | **鼠标左键在非对话界面误触发 `PressActionKey()`**：`InputRouter.gd:85-86` 点左键既写 `pointer_pressed`/`_click_latch` 又发 `action_pressed` 触发 `GameRoot.gd:270` | `Imouto-Life-Monochrome`：Picture 按钮走 `124 EngineState(7)` / `123`，不混触地图 `ActionKey` | `session.PressActionKey()` 仅响应键盘确认键；无对话/选择框时鼠标左键只通过 `124 EngineState(7)` 和 `123` 投递给脚本 |

---

## 三、深挖 `LWSMF` 当前其余隐藏缺陷（及姊妹作现成方案）

除标题与键鼠外，对 `4e46498` 核心与表现层做全量扫描后，还发现 **7 项在实机流程中会直接卡死或失效的隐藏缺陷**——其中 **6 项在姊妹作 `Imouto-Life-Monochrome` 中均已解决过**：

| 编号 | 严重度 | `LWSMF` 位置与现状 | 实机后果 | 姊妹作是否已解决 & 借鉴方向 |
| --- | --- | --- | --- | --- |
| **D1** | **P0（功能空桩）** | `godot/presentation/VmController.cs:554`：`public int? SaveLoad(SaveOp op) => null;` | 原版 **`Opcode 220/221/222`** 全部失效：标题点「继续咯!!!!!」进读档菜单无法读档、游戏内原生存档菜单无法存盘、`TitleMap pc52` 无法读 `save/system.sav`（导致周目标志、CG 回想、系统设置全丢失） | **已解决**（`crates/wolf-game/src/savegame.rs`、`docs/OPCODE_220_SAVE.md`、BUG-34/42）。需在 `VmController.cs` / `GameSession.cs` 接入 `SaveLoad(SaveOp)`，打通 `system.sav` 读写与原生槽位存读档（保留 `.bak` 与音频恢复） |
| **D2** | **P0（功能空桩）** | `godot/presentation/VmController.cs` 未重写 `IHost.ReturnToTitle()` 与 `IHost.EndGame()`（`EventVm.cs:1209-1216` 已触发） | 标题点第三项「还是算了......」（`175 EndGame`）**窗口不关闭且卡死**；结局或脚本执行 `174 ReturnToTitle` **不会返回标题** | **已解决**（BUG-32：`pending_return_to_title` → `reset_to_title()`、`GameView.gd:_quit_app()`）。在 `VmController.cs` 补发 `ReturnToTitleRequested` / `EndGameRequested` 信号，由 `GameRoot.gd` 分别接 `confirm_return_to_title` 重建逻辑与 `get_tree().quit()` |
| **D3** | **P1（操作卡键）** | `godot/presentation_gd/GameRoot.gd` 全文无 `_notification(what)` | `Alt+Tab` 切屏或弹窗失焦时，按住的鼠标键/方向键收不到抬起事件，切回游戏后永久卡住连点或光标失控 | **已解决**（`GameView.gd:1572-1589`）。在 `GameRoot.gd` 监听 `NOTIFICATION_WM_WINDOW_FOCUS_OUT` / `NOTIFICATION_APPLICATION_FOCUS_OUT`，失焦立即清空 `router` 所有按下态、帧锁存与 `vm.ResetPointer()` |
| **D4** | **P1（画面缺失）** | `godot/presentation/VmController.cs` 未实现 `IHost.ChangeColor(int color, int frames)`（`EventVm.cs:1188` 已调用 `151`） | 全屏色调变化（黄昏/夜晚滤镜、闪红、渐暗）全部静默丢失 | **已解决**（BUG-39：`wolf-game` screen overlay 渐变）。在 `VmController.cs` 发射 `ScreenColorRequested(color, frames)`，由 `GameRoot.gd` 驱动一个 `z_index = 55` 的全屏 `ColorRect` 渐变叠加层 |
| **D5** | **P1（UI 遮挡）** | `godot/presentation_gd/GameRoot.gd:632-637` 仍保留常驻 `(892, 8)` `120×30` 的「返回标题」按钮 | 实机直接盖住原版顶栏右上角的「所持金（如 `3611 G`）」 | **已解决**（`GameView.gd` 不在游戏画面常驻按钮，改用 `Esc` / 快捷键呼出确认弹窗）。移除常驻按钮，改为 `F12` 呼出返回标题确认框 |
| **D6** | **P1（潜在死锁）** | 接入 `CE43` 与读档菜单后，从标题选「继续」→ 进读档页 → 按取消返回标题时，`TitleMap event 2 → CE42` 调用栈可能已跑完 | 玩家取消读档后停死在只有标题背景、没有任何菜单可点的僵死画面 | **已解决**（`docs/M3_TITLE_UX.md`）：检测到标题阶段栈空无活动实例/无选项时，自动重新启动标题事件，确保取消读档一定回到可操作的三选项标题页 |
| **D7** | **P2（音频体验）** | `godot/presentation_gd/AudioManager.gd` 仅用 `140` 指令自带 `volume`，未联动系统设置的 BGM/SE 主音量 | 玩家在设置界面调低音量后，当前正在播放的 BGM/BGS 不会实时变小 | **已解决**（`GameView.gd:2242-2249`）。在 `GameRoot.gd` 读取系统音量变量并同步给 `AudioManager` 主增益系数 |

---

## 四、推荐给本地代理的落地顺序

1. **第一批（修标题与退出闭环，立刻可玩）**：
   - 替换误拷的姊妹作 `TitleGraphic.png`，从本作原版包补回 `Picture/title01.jpg` 与 `SystemGraphic/TitleGraphic.png` 重打 `media.pak`；
   - 接入 **CE43 门控 + `Sys20` 镜像与重播**（先查 `programs.json` 确认本作 `CE42/CE43` 选项键与槽号）；
   - 补齐 **D2（`174 ReturnToTitle` / `175 EndGame` 信号接线）** 与 **D5（移除遮挡所持金的右上角常驻按钮，改 `F12`）**。
2. **第二批（键鼠操作彻底对齐姊妹作）**：
   - 一次性修复 **K1 ~ K5**（选项外不误确认、标题图片按钮热区、逻辑坐标逆变换、按压坐标锁存 + 方向键连发、取消鼠标误触 `PressActionKey`）及 **D3（失焦清空输入 `_notification`）**。
3. **第三批（原生存读档、全屏变色与设置闭环）**：
   - 落地 **D1（`Opcode 220/221/222` `SaveLoad` 接线与 `system.sav` 读写）** 与 **D6（取消读档防标题死锁）**；
   - 补齐 **D4（`151 ChangeColor` 全屏色调层）** 与 **D7（系统主音量实时同步）**；
   - 双模式跑通 `bash tools/godot_check.sh` 与 `LWsmf_RENDER=1 bash tools/godot_check.sh`，并在干净目录实机走完「标题三选项（开始 / 继续→取消 / 退出）→ 卧室 → 对话/菜单 → 存读档 → 切屏恢复」全链路。
