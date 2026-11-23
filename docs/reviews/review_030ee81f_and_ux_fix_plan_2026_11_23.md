# LWSMF Remake（`030ee81f`）审阅缺陷与全端 / 移动端 UI·UX 修复方案

- **审阅基线**：`030ee81fb9c9a0c517c652ec1a9913d05cf66c3e`（对比 `614c6cb2`）
- **日期**：2026-11-23
- **适用对象**：本地开发 Agent 逐项落地、排查与回归验证

---

## 一、总览与优先级清单

本报告汇总了三部分待办：**A. 全端 / 移动端 UI 与操作适配项（UX-1 ~ UX-3）**、**B. 移动端存档页妹妹 SD 小人变形与跨端素材显示一致性排查（UX-4 ~ UX-5）**，以及 **C. `030ee81f` 审阅发现的细微缺陷（REV-1 ~ REV-6）**。

| 编号 | 优先级 | 适用平台 | 问题摘要 | 核心落点文件 |
| :--- | :---: | :---: | :--- | :--- |
| **UX-1** | **P1** | 全端（Win / Android / iOS） | 选项框将显式 `Sys3/Sys4` 误当左上角，导致整体偏向右半屏而非几何居中 | `ChoiceWindow.gd`, `DialogDrawTest.gd` |
| **UX-2** | **P1** | 移动端（Android / iOS） | 左上角“菜单”按钮在游戏正文残留遮挡 UI，且在存档页骑跨顶栏下沿 | `PlayOverlays.gd`, `GameRoot.gd` |
| **UX-3** | **P1** | 移动端（仅触控生效） | 可取消/回退界面增加“双击空白处返回上一级”（选项框、H 界面多级按钮等） | `InputRouter.gd`, `GameRoot.gd`, `PictureStage.gd` |
| **UX-4** | **P1** | 移动端（Android / iOS） | 存档页妹妹 SD 小人横向压扁、纵向拉长并溢出槽位（Windows 正常） | `AssetDB.gd`, `PictureStage.gd`, `tools/pack_media.py` |
| **UX-5** | **P1** | 跨端一致性（Win vs 移动端） | 全库跨端素材解析、HD/1× 九宫格路径与字体回退链不一致排查与收敛 | `AssetDB.gd`, `PictureShapes.gd`, `UiSkin.gd`, `PictureStage.gd` |
| **REV-1** | **P2** | 全端 | `MessageWindow._process()` 在 `finished` 态每帧 `queue_redraw()` 抵消低功耗模式 | `MessageWindow.gd` |
| **REV-2** | **P2** | 开发态 / 全端 | `JsonArtifactIo.ResolvePath()` 无条件优先 `.json.br`，覆盖更新后的明文 `.json` | `JsonArtifactIo.cs` |
| **REV-3** | **P2** | Windows 打包 | Inno Setup 安装器模板替换时版本号硬编码为 `"0.1.0"` | `tools/package_release.sh` |
| **REV-4** | **P2** | 全端 | 手动存档成功时 `CE94` 刷新被重复触发两次 | `GameSession.cs`, `GameRoot.gd` |
| **REV-5** | **P3** | iOS 打包 | `package_ios.sh` 的 `dotnet new sln` 缺少 `--format sln` | `tools/package_ios.sh` |
| **REV-6** | **P3** | Windows 打包 | Inno Setup 安装器 `[Languages]` 仅声明 `english` | `tools/installer/lwsmf.iss.tmpl` |

---

## 二、新增全端 / 移动端 UI·UX 优化项（UX-1 ~ UX-3）

### UX-1：选项框几何居中修复（全端共有）

#### 1. 现象与根因
- **实机现象**：与妹妹对话弹出选项框（如「无关紧要的话题 / 色色的话题 / 有点可怕的话题」）时，选项框左边缘落在画面水平中线附近，整张框偏向右半屏并遮挡立绘。
- **代码根因**：
  - 原版 WOLF RPG Editor（1024×576）中，`Sys3`（`SYS_CHOICE_ANCHOR_X`，默认值 `512`，见 `godot/core_cs/LegacySaveImporter.cs:146`）与 `Sys4`（`SYS_CHOICE_ANCHOR_Y`，默认值 `240`）表示选项框的**几何中心坐标（center anchor）**，而非左上角坐标。
  - `godot/presentation_gd/ChoiceWindow.gd:51-58` 仅对 `-1` 哨兵做了 `512.0 - win_size.x * 0.5` 居中，对显式传入的 `Sys3/Sys4` 直接当作 `Rect2.position`（左上角）绘制。
  - 单元测试 `godot/tests_gd/DialogDrawTest.gd:346-349` 也将该错误假设固化为了断言（`"an explicit Sys3/Sys4 pair must be treated as the window's top-left"`）。

#### 2. 修复方案
1. 修改 `godot/presentation_gd/ChoiceWindow.gd:47-58` 的 `window_rect()`：除屏外停放 `_is_offscreen_parked()` 外，无论是 `-1` 哨兵还是显式 `Sys3/Sys4`，均按**几何中心**换算左上角并做安全区裁剪：
   ```gdscript
   func window_rect() -> Rect2:
       var win_size := window_size()
       if _is_offscreen_parked():
           return Rect2(position_hint, win_size)
       var center := position_hint
       if is_equal_approx(center.x, DEFAULT_POSITION_SENTINEL):
           center.x = 512.0
       if is_equal_approx(center.y, DEFAULT_POSITION_SENTINEL):
           center.y = 288.0
       var top_left := center - win_size * 0.5
       var max_x := maxf(24.0, 1000.0 - win_size.x)
       var max_y := maxf(48.0, 536.0 - win_size.y)
       return Rect2(
           Vector2(clampf(top_left.x, 24.0, max_x), clampf(top_left.y, 48.0, max_y)),
           win_size
       )
   ```
2. 同步更新 `godot/presentation_gd/GameRoot.gd:835` 注释（`Sys3/Sys4 hold the choice window center`）。
3. 同步更新 `godot/tests_gd/DialogDrawTest.gd:346-349` 断言，验证 `Vector2(512, 220)` 时 `explicit_choice.get_center()` 等于 `(512, 220)`。

---

### UX-2：移动端“菜单”按钮可见性收口与位置上移

#### 1. 现象与根因
- **游玩界面残留遮挡**：`godot/presentation_gd/PlayOverlays.gd:1192` 的可见性条件未限定界面场景，导致进入游戏正文后“菜单”按钮持续停留在左上角，直接压住音量图标与「方针」状态栏。
- **存档界面位置偏低**：`PlayOverlays.gd:1106, 1191` 将按钮尺寸设为 `64×34`，位置固定在 `Vector2(8.0 + left_inset, 52.0 + top_inset)`（纵向占 `y = 52..86`），而存档页顶部提示框底部边界约在 `y = 68`，导致按钮骑跨顶框下沿并压住 `save1自动存档` 标题。

#### 2. 修复方案
1. **严格限定按钮显示场景（仅标题界面 & 存档界面）**：
   - 在 `godot/presentation_gd/PlayOverlays.gd` 新增 `var _drawer_tab_allowed := false`。
   - 调整 `update_drawer_state()`（`PlayOverlays.gd:1211`）增加 `allow_tab: bool = false` 参数并写入 `_drawer_tab_allowed`。
   - 在 `godot/presentation_gd/GameRoot.gd:1839` 传入 `phase == "title" or save_ui`。
   - 在 `PlayOverlays.gd:1192` 的 `_layout_drawer()` 中改为：
     ```gdscript
     _drawer_tab.visible = _touch_ui_enabled and _drawer_tab_allowed and not _drawer_open and not is_modal_open()
     ```
2. **调整按钮位置至存档页顶部框内**：
   - 将 `PlayOverlays.gd:1191` 的按钮坐标上移至 `Vector2(12.0 + left_inset, 16.0 + top_inset)`（纵向占 `y = 16..50`，完整居中于顶部提示框内，不压上下边框与 `save1`）。

---

### UX-3：移动端双击空白处返回上一级（仅移动端生效）

#### 1. 现状与设计目标
- **现状**：`godot/presentation_gd/InputRouter.gd:222-267` 当前仅将**双指轻点**映射为取消脉冲（`_right_click_latch = true` + `cancel_pressed.emit()`），单手操作不便。
- **目标**：仅在移动端触控模式下支持**单指双击空白处**返回上一级（覆盖选项框、与妹妹互动界面、H 界面两侧多级交互按钮、存档界面等），Windows 桌面端维持右键取消不变。

#### 2. 精准空白区判定与防误触设计
1. **基于 `op124 field == 8` 记录活跃交互图片按钮（`PictureStage.gd:611-617`）**：
   - WOLF RPG 所有可点击图片按钮（含 H 界面两侧多级交互按钮、房间快捷按钮）都会通过 `op124 field == 8` 轮询命中，而背景与静态立绘不会。
   - 在 `PictureStage.gd` 维护当前步/上一步被 `field == 8` 查询过的未隐藏图片号集合 `_polled_hit_slots` / `_last_polled_hit_slots`，提供 `has_interactive_hit(point: Vector2) -> bool`。
2. **在 `GameRoot.gd` 提供 `is_touch_blank_space(point: Vector2) -> bool`**：
   - `_modal_window_open()` 或 `faulted` → `false`。
   - `choice.is_open` → 仅当 `choice.allow_cancel` 且 `point` 不在 `choice.window_rect()`（或 `override_rects`）内时为 `true`。
   - `message.is_open()` 且 `not choice.is_open` → `false`（防止对话快速连点翻页误触发取消）。
   - `_save_load_ui_active()` → `not _save_load_hit_interactive(point)`。
   - 其他交互态 → 若 `(int(session.BasicAllDeny) & 2) != 0` 返回 `false`；若 `stage.has_interactive_hit(point)` 返回 `false`；否则 `true`。
3. **在 `InputRouter.gd` 中实现移动端双击空白取消**：
   - 仅在 `OS.has_feature("mobile") or OS.has_feature("android") or OS.has_feature("ios")` 启用 `mobile_double_tap_cancel_enabled = true`。
   - 新增常量 `TOUCH_DOUBLE_TAP_MAX_MS := 320`、`TOUCH_DOUBLE_TAP_MAX_DIST := 48.0`。
   - 单指轻点释放且命中空白区时：若落在双击时间窗与距离窗内，触发 `trigger_cancel_pulse("touch")`（清 `_click_latch`、置 `_right_click_latch = true`、emit `cancel_pressed`）；首次点空白区仅记录时间戳与位置、不触发确认脉冲；点非空白区则清空双击状态并正常发 `action_pressed`。保留原有双指轻点作为备用。

---

## 三、移动端存档页妹妹 SD 小人变形与跨端素材一致性排查（UX-4 ~ UX-5）

### UX-4：移动端存档栏妹妹 SD 小人被压扁拉长（Windows 正常）

#### 1. 现象核对
- **实机对比**：在移动端存档页截图（`save2` 槽位）中，同一画面里的背景、存档卡片、文字、按钮均为正常 1:1 比例（已排除全局视口 stretch 拉伸），**唯独存档槽上的妹妹 SD 小人被横向压扁约一半、纵向拉长至顶穿 `save2` 框侵入 `save1`**；同一场景在 Windows 端比例完全正常。

#### 2. 代码级根因候选与精确定位步骤
由于离线审阅环境不含原版素材包（`media.pak` / `media_hd.pak`），结合 `CE94` 绘制链路（`docs/engine/OPCODE_124.md:194, 558`、`docs/godot/DECISIONS.md:72, 78`）与全库平台分支审计，**仅存在以下三类会导致“仅该槽位在移动端变形而 Windows 正常”的直接根因**：

1. **候选根因 A（最高概率）：移动端 `media.pak` 与 Windows 散文件 / `media_hd.pak` 解析到不同尺寸或不同目录的同名素材**
   - `AssetDB.gd:27-32, 185, 400, 472` 中，`DIR_PRIORITY` 仅包含 4 个目录（`systemgraphic`, `hword`, `systemfile`, `picture`），其余目录（`charachip`, `ficon`, `dlc_data`, `cg`, `kagura`, `1.4.0` 等）全部并列 `rank = 4`。
   - 当路径走 basename 回退时，Windows 散文件按 NTFS 目录枚举序先到先得，移动端 `media.pak` 按 `pack_media.py:220` 字母序先到先得，`media_hd.pak` 按 `hd_manifest.json` 键序先到先得；并且 `tools/pack_media.py:157` 跳过了 `BasicData/`。
   - 一旦移动端命中的贴图尺寸或网格结构（如 3×4 行走图 vs 单帧小人图）与 Windows 不同，`CE94` 按目标框计算或按固定 `columns/rows` 切分时就会直接出现压扁拉长。
2. **候选根因 B（高概率）：`CE94` 预载测量（`scale=0` + `op124 field 2/3`）在移动端首次读取失败或落入 `420×140` 占位基准尺寸**
   - `PictureStage.gd:632-638` 的 `field 2/3` 返回 `base_size`；若首次预载 `draw_picture` 时因路径差异或缓存异常导致 `assets.texture(file)` 为 `null`，`PictureStage.gd:855-856, 929-930` 会把 `base_size` 设为 `Vector2(420, 140)`（宽高比 **3:1**）。若脚本据此反算 `scale_x` / `scale_y`，横向缩放会变成纵向缩放的约 1/3，呈现典型的“压扁拉长”。
3. **候选根因 C（需核验）：`media.pak` 与 Windows 散文件 / `media_hd.pak` 中同一 `CharaChip` / `charicon` 的实际像素尺寸或 `hd_scale` 不一致**
   - Windows 默认 `AssetDB.default_hd_enabled() == true`（`AssetDB.gd:88-93`），移动端默认 `false`。若某张图在 `media_hd.pak` 的 `hd_manifest.json` 中记录的 `scale` 与真实倍率不符，或 1× `media.pak` 打包时来源文件与桌面端不同，都会在 `PictureStage.gd:701-702` 的 `_texture_cell_size()` 中放大为两端比例差异。

#### 3. 本地 Agent 立即定位与修复动作
1. **扩展 `PictureStage.dump_pictures()`（`PictureStage.gd:1461-1499`）**：在诊断列中追加 `resolved_path | tex_size | hd_scale | split(c,r) | base_size | scale(sx,sy)`，在 Windows 与移动端分别打开存档页导出 `user://picture_dump.txt`（移动端可在快捷菜单触发或启动日志打印 `12050..12130` 槽位），**1 分钟内对比两端的 `resolved_path`、`tex_size`、`split`、`scale_x/scale_y`** 即可锁定确切根因并直接修复。
2. **修复 `PictureStage.gd:954-971` `_apply_picture_geometry()` 的 `scale_mode` 缺口**：第 959 行当前直接取 `args[16]` 而不检查 `scale_mode == 3`，虽通常会被紧随其后的 `_render_picture()` 覆盖，但应统一改为调用 `_pose_from_args()` / `_apply_scale_mode()`，消除单轴缩放误触发隐患。

---

### UX-5：全库跨端素材显示一致性排查清单（防隐藏回归）

对 `godot/presentation_gd/`、`godot/core_cs/` 与 `tools/` 全量扫描所有平台分支（`OS.has_feature`、`OS.get_name`、`hd_enabled`、`media.pak`）后，除 UX-4 外，共排查出 **4 处会导致桌面端与移动端素材显示不一致的隐患点**，建议本轮一并收敛：

| 隐患点 | 位置 | 桌面端（Windows）行为 | 移动端（Android / iOS）行为 | 潜在显示异常与修复建议 |
| :--- | :--- | :--- | :--- | :--- |
| **1. Basename 回退排序不确定** | `AssetDB.gd:27-32, 185, 400, 472` | 散文件按 NTFS 枚举序 + `hd_manifest` 键序 | `media.pak` 按路径字母序，且排除 `BasicData/` | 同名不同目录素材在两端加载成不同图片。<br>**修复**：补全 `DIR_PRIORITY` 覆盖全部素材目录（加入 `charachip`, `ficon`, `dlc_data`, `cg`, `hscene`, `cutin`, `battleeffect`, `enemygraphic`, `mapchip`, `kagura`, `1.4.0`），并在同 rank 冲突时统一按 `lower_rel` 字典序取最小者，保证散文件、`media.pak`、`media_hd.pak` 三套索引 100% 确定性一致。 |
| **2. 九宫格窗口双渲染路径分歧** | `PictureShapes.gd:170-193` | `hd_enabled=true`（`art_scale=2.0`）走自定义 `Control + UiSkin.nine_patch()` | `hd_enabled=false`（`art_scale=1.0`）走原生 `NinePatchRect` | 两条路径在小数缩放、边缘像素采样与父节点 Shader 混合（`use_parent_material` + `blend=1/3`）下表现可能不一致。<br>**修复**：核验移动端带 `ADD`/`MUL` 混合的窗口框（如光标框、渐变遮罩）与桌面端是否一致；必要时统一走 `UiSkin.nine_patch()` 绘制路径，彻底消除双轨差异。 |
| **3. 三端字体回退链分裂** | `UiSkin.gd:15-34, 150-215` | 优先 `Lolita.ttf`，回退 `Microsoft YaHei UI` | Android 回退 `ANDROID_FONT_NAMES`；iOS 未区分直接落入 `FONT_NAMES`（仅 `PingFang SC` 命中） | 若移动包漏打 `Lolita.ttf` 或包内加载失败，三端中文字宽/基线不同会导致 HUD 数值偏移、对话气泡换行错位。<br>**修复**：在 `android_data.py` 与 `ios_inject_data.py` 打包审计中强制校验 `media.pak`（或指定字体路径）已包含 `Lolita.ttf`，并在 `UiSkin.gd` 为 iOS 显式补齐 `IOS_FONT_NAMES`（`PingFang SC`, `Hiragino Sans GB`, `sans-serif`）。 |
| **4. 贴图分帧与 `hd_scale` 整除精度** | `PictureStage.gd:701-702, 716-722` | `floor(2W / cols) / 2.0` | `floor(W / cols) / 1.0` | 当 1× 原图宽高不能被 `columns`/`rows` 整除时，2× 图与 1× 图的 cell 尺寸可能差 `0.5~1px`，叠加 `AtlasTexture` 采样可能出现边缘半像素渗色。<br>**修复**：新增跨端资产审计脚本（Python），离线扫描 `work/all`、`media.pak` 与 `media_hd.pak`，校验：① 所有同名冲突文件清单；② `media.pak` 与 `media_hd.pak` 中每张图的宽高比误差 `|w_hd/h_hd - w_1x/h_1x| < 1e-3` 及 `w_hd / w_1x == scale`；③ `media.pak` 相对 `work/all` 是否遗漏除 `.dat`/`.project`/存档外的任何图片/字体资源。 |

---

## 四、`030ee81f` 审阅发现的既有缺陷（REV-1 ~ REV-6）

### REV-1（P2）：`MessageWindow` 光标每帧重绘抵消 `OS.low_processor_usage_mode`
- **位置**：`godot/presentation_gd/MessageWindow.gd:217-226` 对比 `godot/presentation_gd/GameRoot.gd:254-269`
- **问题**：`GameRoot._can_use_low_processor_mode()` 仅在 `message.is_open() and message.finished` 时开启低功耗模式，但 `MessageWindow._process()` 在 `finished == true`（及 `waiting_for_continue == true`）时每帧无条件调用 `queue_redraw()`，迫使 Godot 满帧重绘，低功耗模式实际失效。
- **修复**：在 `MessageWindow.gd` 记录 `_last_blink_phase := -1`，在 `finished` / `waiting_for_continue` 分支中按离散步进（如 `int(floor(_cursor_time * 4.0))`）仅在相位变化时调用 `queue_redraw()`。

### REV-2（P2）：开发态 `.json.br` 无条件优先于更新的明文 `.json`
- **位置**：`godot/core_cs/JsonArtifactIo.cs:14-24`
- **问题**：`ResolvePath()` 只要检测到 `.json.br` 存在就直接返回；本地重新生成明文 `programs.json` / `database.json` 时若残留旧 `.json.br`，会静默读到过期数据。
- **修复**：明文 `full` 与 `brPath` 同时存在时比较 `File.GetLastWriteTimeUtc(full) > File.GetLastWriteTimeUtc(brPath)`，明文更新则优先返回 `full`。

### REV-3（P2）：Inno Setup 安装器版本号硬编码为 `"0.1.0"`
- **位置**：`tools/package_release.sh:382`
- **问题**：替换 `@APP_VERSION@` 时写死 `"0.1.0"`，未与 `package_android.sh` / `package_ios.sh` 的 `--version` 对齐。
- **修复**：为 `tools/package_release.sh` 增加 `VERSION="0.1.0"` 与 `--version <ver>` 选项并透传至模板替换。

### REV-4（P2）：手动存档成功触发两次 `CE94` 界面刷新
- **位置**：`godot/core_cs/GameSession.cs:742-783` 与 `godot/presentation_gd/GameRoot.gd:1994-2003`
- **问题**：`SaveGameToSlot(slot)` 内部调用了一次 `RefreshSaveUiForSlot(slot)`，随后 `GameRoot.gd:2003` 写完缩略图后又调用了一次。
- **修复**：为 `SaveGameToSlot(int slot, bool refreshUi = true)` 增加可选参数，`GameRoot.gd:1994` 传 `false`，待缩略图落盘并 `invalidate_path()` 后统一刷新一次。

### REV-5（P3）：iOS 打包脚本 `dotnet new sln` 缺少 `--format sln`
- **位置**：`tools/package_ios.sh:129`
- **问题**：未与 `package_android.sh:214` 同步 `--format sln`，在 .NET 9+ 环境可能默认生成 `.slnx`。
- **修复**：补充 `--format sln`。

### REV-6（P3）：Inno Setup 安装器语言仅声明 `english`
- **位置**：`tools/installer/lwsmf.iss.tmpl:23`
- **问题**：`[Languages]` 仅包含英文。
- **修复**：在确认构建机 Inno Setup 安装目录存在 `ChineseSimplified.isl` 后，按需补充中文语言条目（或做成可选参数）。

---

## 五、回归测试与实机验收清单（`needs-runtime`）

1. **自动化单测与资产审计**：
   - 更新 `DialogDrawTest.gd:346-349` 后跑通 Godot 渲染/UI 门禁测试，验证选项框几何居中与安全区裁剪。
   - 新增 `PlayOverlays` 与 `InputRouter` 单元测试，覆盖菜单按钮显隐/坐标、移动端双击空白返回与桌面端不触发。
   - 运行跨端资产一致性审计脚本，确认 `work/all`、`media.pak`、`media_hd.pak` 无 basename 冲突、无宽高比漂移、无漏打 `Lolita.ttf`。
   - 运行 `dotnet test` 验证 `InvariantGlobalization=true` 与 `JsonArtifactIo` 时间戳优先逻辑。
2. **移动端（Android / iOS）与 Windows 双端实机对比验收**：
   - 对比双端存档页 `PictureDump`（`12050..12130`），确认妹妹 SD 小人比例与位置两端完全一致、无压扁拉长或溢出。
   - 标题界面与存档界面左上角“菜单”按钮完整落在顶栏框内；进入房间、对话、H 场景后按钮完全隐藏。
   - 全端对话/事件选项框均严格几何居中。
   - 移动端在选项框外及 H 界面两侧多级交互菜单空白处双击可顺畅返回上一级，快速连点交互按钮与对话翻页不误触发取消。
   - 修复 **REV-1** 后，在对话等待态与失焦/最小化状态下确认 FPS 与 CPU/GPU 占用按预期回落。
