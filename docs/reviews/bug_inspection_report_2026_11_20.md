# 《妹相随～黑白世界的缤纷冒险～》重制版核心缺陷审查报告
**报告编号**：AUDIT-20261120-LWSMF  
**审查日期**：2026-11-20  
**审查范围**：键鼠输入逻辑与右键失效、游戏素材大量缺失、文本解析内部代号泄露与显示异常  
**执行原则**：代码与资产纯只读审查，未修改任何工程源码与资产文件  

> **历史状态**：本报告保留为 2026-11-20 的审查快照。后续按 WOLF 蓝本复核推翻了“空格=取消”的
> 判断；当前空格与 Enter/Z 一样是确认键。行号与部分修复建议可能已被后续实现取代，现状以
> [DECISIONS.md](../godot/DECISIONS.md) D-58/D-70 与[最新验证报告](release_gate_verification_2026_11_21.md)为准。

---

## 目录
- [一、 审查概述](#一-审查概述)
- [二、 问题一：鼠标右键点击失效与键鼠交互逻辑审计](#二-问题一鼠标右键点击失效与键鼠交互逻辑审计)
  - [2.1 期望操作逻辑与现状偏差比对](#21-期望操作逻辑与现状偏差比对)
  - [2.2 鼠标右键无法触发 Cancel 的深层代码缺陷](#22-鼠标右键无法触发-cancel-的深层代码缺陷)
  - [2.3 空格键映射错位缺陷](#23-空格键映射错位缺陷)
  - [2.4 滚轮预选与鼠标悬停（Hover）的光标所有权冲突](#24-滚轮预选与鼠标悬停hover的光标所有权冲突)
  - [2.5 WOLF 原生存档系统（CE131）按键轮询兼容性分析](#25-wolf-原生存档系统ce131按键轮询兼容性分析)
- [三、 问题二：游戏素材大量缺失与画面色块异常审计](#三-问题二游戏素材大量缺失与画面色块异常审计)
  - [3.1 致命根因：资源打包供应链错置为前作《妹！生活》](#31-致命根因资源打包供应链错置为前作妹生活)
  - [3.2 资产引用与实际打包量化统计表](#32-资产引用与实际打包量化统计表)
  - [3.3 Steam 原版 23 个 `.wolf` 数据包审计](#33-steam-原版-23-个-wolf-数据包审计)
  - [3.4 画面大面积方块与占位文字渲染机理](#34-画面大面积方块与占位文字渲染机理)
- [四、 问题三：文本解析异常与内部代号泄露审计（对比姊妹作）](#四-问题三文本解析异常与内部代号泄露审计对比姊妹作)
  - [4.1 8 处核心文本缺陷总览表](#41-8-处核心文本缺陷总览表)
  - [4.2 缺陷深度剖析与姊妹作实现对比](#42-缺陷深度剖析与姊妹作实现对比)
- [五、 整改与修复建议路线图](#五-整改与修复建议路线图)

---

## 一、 审查概述

本项目致力于在 Godot 4 引擎上重制 WOLF RPG 经典作品《妹相随～黑白世界的缤纷冒险～》（Living With Sister Monochrome Fantasy）。当前版本在运行过程中暴露了以下三大阻塞性体验问题：
1. **交互层**：鼠标右键无法进行取消或返回，且键鼠操作（空格、滚轮、方向键）与原版及玩家习惯不一致；
2. **资产层**：画面大面积呈现彩色方块和 `(missing asset)`，场景背景及人物差分大量缺失；
3. **表现层**：文本与选项频繁出现 `\f[12]`、`\c[1]`、`\E` 等原始转义字符，动态选项尾部多出无效 `"0"`，角色名与数据库内容出现 `:0]` 乱码。

本次审查调动了底层输入映射追踪、WOLF 字节码解析、Steam 原版资源交叉比对以及姊妹作（《妹！生活》`Imouto Life Monochrome`）架构对照，查清了上述所有问题的根本成因。

---

## 二、 问题一：鼠标右键点击失效与键鼠交互逻辑审计

### 2.1 期望操作逻辑与现状偏差比对

根据用户期望的键鼠交互规范，与当前 Godot 实现进行逐项比对：

| 输入动作 | 期望行为 | 当前重制版实际表现 | 状态 | 缺陷判定 |
| :--- | :--- | :--- | :---: | :--- |
| **回车 / 鼠标左键** | 确认 | 确认（ChoiceWindow、MessageWindow、Opcode 123 响应） | 正常 | 符合预期。 |
| **Esc / 鼠标右键** | 取消 / 返回 | **Esc**：部分界面生效<br>**右键**：**完全无反应** | 严重异常 | `InputRouter.gd` 未映射右键至 Cancel，且缺乏单帧锁存。 |
| **空格键** | 确认（后续蓝本复核更正） | 被当作**“确认 (Action)”**触发 | 正常 | 原报告“空格=取消”的判断已推翻；`KEY_SPACE` 应保留在 `ACTION_KEYS`。 |
| **上下方向键** | 改变选项预选 / 改变预选存档栏 | 改变选项预选 / 改变存档栏槽位 | 正常 | WOLF 原生 Up/Down（键值 4/5）正常派发。 |
| **左右方向键** | 存档页面翻页 | 正常翻页 | 正常 | WOLF 原生 Left/Right（键值 2/3）正常响应。 |
| **鼠标滚轮** | 1. 改变选项预选<br>2. 存档页面翻页 | 1. 选项框滚动后瞬间被鼠标悬停覆盖<br>2. 存档界面滚轮完全失效 | 严重异常 | 1. `ChoiceWindow.gd` 每帧 Hover 强制抢夺选中态；<br>2. 未将滚轮事件转译为 WOLF 滚轮按键（键值 20/21）。 |

---

### 2.2 鼠标右键无法触发 Cancel 的深层代码缺陷

#### 缺陷 A：`InputRouter.gd` 的 `held_mask()` 遗漏右键 Cancel 映射（致命根因）
- **代码文件**：[InputRouter.gd](../../godot/presentation_gd/InputRouter.gd#L133-L157)
- **代码片段**：
  ```gdscript
  func held_mask() -> int:
      var mask := _physical_mask()
      if _click_latch or Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT):
          mask |= MASK_OK
      if _any_pressed(CANCEL_KEYS):
          mask |= MASK_CANCEL
      return mask
  ```
- **机理分析**：
  WOLF RPG 引擎的所有内置系统（例如公共事件 CE131 存档界面、道具栏、装备栏、状态栏）都是通过 **Opcode 123 (KeyInput)** 轮询 `held_mask()` 来监听取消操作（键值 11 / `MASK_CANCEL`）的。
  在当前实现中：
  1. 左键检查了 `MOUSE_BUTTON_LEFT` 并追加了 `MASK_OK`；
  2. 键盘检查了 `CANCEL_KEYS` 并追加了 `MASK_CANCEL`；
  3. **右键（`right_pressed` 或 `MOUSE_BUTTON_RIGHT`）完全没有参与 `held_mask()` 计算！也没有任何 `_right_click_latch`！**
- **历史铁证**：
  在项目早期的 Rust CLI 实现 [play.rs](../../crates/wolf-cli/src/play.rs#L538-L543) 中，曾专门对该问题进行了修复：
  ```rust
  // Wofl RPG uses RMB as cancel in menus and message advance
  if mouse_buttons & 2 != 0 {
      bits |= key::CANCEL;
  }
  ```
  在从 Rust 迁移到 Godot 的 GDScript 时，该映射被完全遗漏。

#### 缺陷 B：鼠标右键缺少单帧锁存（Frame Latch）
- **代码文件**：[InputRouter.gd:70-80](../../godot/presentation_gd/InputRouter.gd#L70-L80) 与 [GameRoot.gd:235](../../godot/presentation_gd/GameRoot.gd#L235)
- **机理分析**：
  左键点击通过 `_click_latch = true` 实现了帧锁存，即使在单帧内完成“按下+抬起”，该帧依然能向 VM 报告点击。
  而右键在 [InputRouter.gd:75](../../godot/presentation_gd/InputRouter.gd#L75) 中采用瞬时判定：
  ```gdscript
  elif event.button_index == MOUSE_BUTTON_RIGHT:
      right_pressed = event.pressed
  ```
  当玩家进行一次快速右击时，若按下与抬起均在同一物理帧发生，[GameRoot.gd:235](../../godot/presentation_gd/GameRoot.gd#L235) 调用 `vm.SetPointerRightPressed()` 时已变为 `false`。这导致底层用于查询鼠标按键状态的 Opcode 124 脚本无法感知右键按下。

#### 缺陷 C：`GameRoot.gd` 未向上分发全局 Cancel 信号
- **代码文件**：[GameRoot.gd:330-336](../../godot/presentation_gd/GameRoot.gd#L330-L336)
- **机理分析**：
  ```gdscript
  func _on_action() -> void:
      if choice.is_open: choice.confirm()
      elif message.is_open(): message.advance()
      elif session != null: session.PressActionKey()

  func _on_cancel() -> void:
      if choice.is_open: choice.cancel()
      elif message.is_open(): message.cancel()
      # 这里没有任何对 session 或 VM Cancel 键的分发！
  ```
  当处于地图行走、自定义贴图界面或非原生 Choice/Message 弹窗时，触发 `_on_cancel()` 会被直接吞掉。

---

### 2.3 已推翻：空格键映射错位

- **代码文件**：[InputRouter.gd:27-31](../../godot/presentation_gd/InputRouter.gd#L27-L31)
- **代码片段**：
  ```gdscript
  const ACTION_KEYS: Array[Key] = [
      KEY_Z, KEY_ENTER, KEY_KP_ENTER, KEY_SPACE,
  ]
  const CANCEL_KEYS: Array[Key] = [
      KEY_X, KEY_ESCAPE, KEY_BACKSPACE,
  ]
  ```
- **后续复核**：
  原报告误把空格当作取消键。WOLF 蓝本及本作实际输入中，空格与 Enter/Z 一样是确认键；
  `KEY_SPACE` 应保留在 `ACTION_KEYS`，不应移入 `CANCEL_KEYS`。

---

### 2.4 滚轮预选与鼠标悬停（Hover）的光标所有权冲突

- **代码文件**：[ChoiceWindow.gd:64-70](../../godot/presentation_gd/ChoiceWindow.gd#L64-L70)
- **代码片段**：
  ```gdscript
  func hover_at(pos: Vector2) -> void:
      var idx := _index_at(pos)
      if idx >= 0 and idx < options.size():
          selected = idx # 只要鼠标停留在某个选项上，selected 被强行设为鼠标所在项！
  ```
- **分析**：
  当玩家尝试使用滚轮或键盘上下键改变预选项时，只要鼠标光标停留在选项窗口内部，主循环下一帧调用 `hover_at()` 就会瞬间把 `selected` 强行拉回鼠标悬停的行，造成键盘/滚轮选择被鼠标“抢夺”并产生严重视觉抖动。
  正确的交互逻辑应当区分**输入来源**（鼠标移动激活 Hover，键盘/滚轮激活按键模式），直到鼠标发生实际位移前，不应反复覆盖键盘/滚轮的选中状态。

---

### 2.5 WOLF 原生存档系统（CE131）按键轮询兼容性分析

经解析原版数据中管理存档/读档的公共事件 **CE131 (`X┣[移]セーブ欄実行`)**：
- **Opcode 123 轮询逻辑**：CE131 调用 Opcode 123 监听 `mask = 771`（包含方向键、确认、取消、以及滚轮上/下）；
- **上下键**：`pc=282..295` 判定键值 `4`（Down）与 `5`（Up），用于在 5 个存档栏位之间循环移动光标（完全符合用户期望）；
- **左右键**：`pc=298..324` 判定键值 `2`（Left）与 `3`（Right），用于在存档页面（Page 1 ~ 5）之间翻页（完全符合用户期望）；
- **滚轮翻页**：`pc=58..66` 专门判定了键值 `20`（滚轮上）与 `21`（滚轮下），原生用于快速翻页。
- **结论**：CE131 内部的原生 WOLF 脚本逻辑完全支持用户期望的操作方式。当前无法正常运作的原因在于 Godot 端的 `InputRouter.gd` 没有将右键和滚轮打包进 `held_mask`，导致 VM 根本收不到对应的事件。

---

## 三、 问题二：游戏素材大量缺失与画面色块异常审计

### 3.1 致命根因：资源打包供应链错置为前作《妹！生活》

重制版当前加载的资产包位于 `work/all/media.pak`。经查验其打包清单构建脚本：
- **脚本文件**：`work/_make_media_manifest.py:10`
- **代码片段**：
  ```python
  root = Path(r"<sister-repo>\work\Data")
  ```
- **审计结论**：
  **打包脚本的素材源路径直接指向了姊妹作《妹！生活》的目录！**
  当前打包进 `media.pak` 的 1,214 个资源中，有 **691 个文件（56.9%）根本在本作中不存在**，纯属前作废弃资源；而本作真正需要的美术与音频资源几乎全部被遗漏在包外。

---

### 3.2 资产引用与实际打包量化统计表

| 资产类别 | 本作脚本实际引用数量 | 当前 `media.pak` 命中数量 | 资产缺失率 | Steam 原版 `Data/*.wolf` 命中数量 |
| :--- | :---: | :---: | :---: | :---: |
| **图像 (Picture/CG/UI)** | 2,215 项 | **57 项** | **97.4% 缺失** | **2,200 项 (99.3%)** |
| **音频 (BGM/BGS/SE)** | 316 项 | 114 项 | 63.9% 缺失 | **316 项 (100.0%)** |
| **合计** | 2,531 项 | 171 项 | **93.2% 缺失** | **2,516 项 (99.4%)** |

> 注：Steam 原版缺失的 15 张图片属于原版脚本中未使用的冗余引用或运行时动态拼接路径。Steam 原版完全覆盖了所有游戏所需的美术素材。

---

### 3.3 Steam 原版 23 个 `.wolf` 数据包审计

检查零售安装根下的数据目录（`<retail-root>/Data`），共有 23 个 `.wolf` 文件，总容量约 **757 MB**，包含 **4,062 个文件**，且文件全部完整未损坏：

| 数据包名称 | 文件大小 | 内部文件数 | 核心内容 | 是否已解包合入当前工程 |
| :--- | :---: | :---: | :--- | :---: |
| `DLC_data.wolf` | 154 MB | 633 | DLC 剧情 CG、事件立绘与高精插图 | **未解包** |
| `cg.wolf` | 123 MB | 596 | 核心 CG 差分图集 | **未解包** |
| `Picture.wolf` | 25 MB | 539 | 主要角色常驻立绘、表情差分 | **未解包** |
| `hscene.wolf` | 20 MB | 327 | 事件场景画面 | **未解包** |
| `1.4.0.wolf` | 33 MB | 199 | 1.4.0 版本新增差分与素材 | **未解包** |
| `cutin.wolf` | 6 MB | 223 | 剧情切入特写插画 | **未解包** |
| `BGM.wolf` | 172 MB | 26 | 原版高质量音乐轨 | **未解包** |
| `SE.wolf` | 10 MB | 250 | 系统音效与环境音 | **未解包** |
| `Title.wolf`, `Sys.wolf`, `Window.wolf` | ~5 MB | ~150 | 窗口皮肤、系统光标、标题组件 | **未解包** |
| `MapChip.wolf` | 22 MB | 61 | 地图瓦片图集 | **未解包** |
| 其他补丁包 (`1.1.0` ~ `1.3.1.wolf`) | ~200 MB | ~1,000 | 历史各版本补丁资源 | **未解包** |

**结论**：重制版工程从未对 Steam 原版的这 23 个核心 `.wolf` 包进行正规解包，而是错将前作的旧资源打了包，导致游戏运行时陷入大面积资产缺失。

---

### 3.4 画面大面积方块与占位文字渲染机理

- **代码文件**：[PictureStage.gd:573-584](../../godot/presentation_gd/PictureStage.gd#L573-L584)
- **代码片段**：
  ```gdscript
  func _draw_missing_placeholder(p: PictureInstance) -> void:
      var rect := Rect2(pos, Vector2(420, 140))
      var hue := fposmod(float(p.pic_id) * 0.137, 1.0)
      draw_rect(rect, Color.from_hsv(hue, 0.65, 0.9, 0.45), true)
      draw_rect(rect, Color.WHITE, false, 2.0)
      draw_string(font, pos + Vector2(10, 30), "(missing asset)", ...)
  ```
- **分析**：
  当图片加载失败时，代码会按照图片 ID 分配一个不同色相的彩色半透明矩形，并绘制 `(missing asset)`。WOLF 引擎的复合画面往往由立绘身体、发型、衣服、脸部、表情等数十个层叠加而成。当上层差分全部丢失时，屏幕上便会叠加出密密麻麻的彩色方块。
- 此外，[MapLayer.gd](../../godot/presentation_gd/MapLayer.gd) 未接入 `MapChip` 纹理映射；按 [DECISIONS.md](../godot/DECISIONS.md) 的 D-21，零售正片不依赖该通用示例瓦片层，单色块只用于兼容/测试。

---

## 四、 问题三：文本解析异常与内部代号泄露审计（对比姊妹作）

### 4.1 8 处核心文本缺陷总览表

| 编号 | 缺陷现象 | 影响文本示例 | 出错代码位置 | 姊妹作（Imouto Life）实现比对 |
| :---: | :--- | :--- | :--- | :--- |
| **1** | **选项框裸绘内部控制码** | 选项中出现 `\f[12]出门`、`\c[1]休息` | [ChoiceWindow.gd:152](../../godot/presentation_gd/ChoiceWindow.gd#L152) | 选项框直接调用 `font.draw_string(options[i])`，完全跳过了 `TextTokenizer` 分词与剥离流程。 |
| **2** | **空动态选项被错误填充 `"0"`** | 选项列表末尾多出一行无意义的 `"0"` | [TextExpander.cs:76](../../godot/core_cs/TextExpander.cs#L76) | 自变量为空时，C# 误调用 `.ToString()` 补上了数值 `0`。姊妹作在 `wolf-vm/src/text.rs:252` 明确保护 5..=9 字符串槽位为空时不补 0。 |
| **3** | **`\E` 后接数字误判为普通文字** | `\E3000G`、`\E10年` 泄露字符 `\E` | [TextTokenizer.gd:126-130](../../godot/presentation_gd/TextTokenizer.gd#L126-L130) | 特判逻辑将 `name == "E" and source[end].is_valid_int()` 视作普通文本；`\E` 实为 WOLF 黑边描边样式指令。 |
| **4** | **数据库引用全线失效并泄露代号** | 角色名/道具名丢失，屏幕显示 `:0]`、`:1]` | [TextExpander.cs:63](../../godot/core_cs/TextExpander.cs#L63) 与 [TextTokenizer.gd:27](../../godot/presentation_gd/TextTokenizer.gd#L27) | 未实现 `\cdb[...]`、`\udb[...]`、`\sdb[...]` 数据库文本展开，遇冒号解析失败；分词器又误把 `cdb` 当成了颜色标签。 |
| **5** | **中括号嵌套提前截断** | `\udb[28:\v[160]:0]` 截断残损 | [TextExpander.cs:59](../../godot/core_cs/TextExpander.cs#L59) | 使用简单 `text.IndexOf(']', ...)`，遇到内部嵌套有 `\v[...]` 的多层中括号时在最内层提前断开。 |
| **6** | **历史回顾（Backlog）暴露原始控制码** | 历史记录里充斥着 `\c[0]\f[24]` | [GameRoot.gd:422](../../godot/presentation_gd/GameRoot.gd#L422) | 将未清理控制码的原始 `full_text` 塞入 `_backlog`，而后由普通 `Label` 渲染。 |
| **7** | **未展开环境下的转义 `\n`** | 文字出现换行异常或打印字符 `\n` | [TextExpander.cs:17](../../godot/core_cs/TextExpander.cs#L17) | WOLF 中文本内部字面量 `\n` 为换行符，但在没有预展开的普通字段未统一处理换行符转义。 |
| **8** | **行首特殊符号未清空** | 消息窗口开头残留内部符号 | [MessageWindow.gd:164](../../godot/presentation_gd/MessageWindow.gd#L164) | 缺乏针对 WOLF 消息行前置标记字符（如 `_`、`@` 等控制窗口行为符号）的剥离逻辑。 |

---

### 4.2 缺陷深度剖析与姊妹作实现对比

#### 1. 选项框裸绘内部控制码
- **代码文件**：[ChoiceWindow.gd:152](../../godot/presentation_gd/ChoiceWindow.gd#L152)
- **代码**：
  ```gdscript
  draw_string(font, text_pos, options[i], ...)
  ```
- **分析**：
  `MessageWindow.gd` 正确使用了 `TextTokenizer.gd` 来剔除 `\f[...]`、`\c[...]` 并转换字体颜色和大小。但在 `ChoiceWindow.gd` 中，开发者直接使用 `draw_string` 将选项字符串完整画在画布上。导致诸如 `\f[12]`（调整字号）、`\c[1]`（改变文本颜色）的指令直接作为文字显示在玩家屏幕上。

#### 2. 空动态选项被错误填充 `"0"`
- **代码文件**：[TextExpander.cs:76](../../godot/core_cs/TextExpander.cs#L76)
- **分析**：
  在 WOLF 中，动态选项常常通过自变量槽位 `\cs[5]` ~ `\cs[9]` 进行传参，如果未使用的选项槽位未初始化，其自变量应保持为空字符串 `""`。
  但 C# 展开器在取不到字符串值时，回落（Fallback）调用了：
  ```csharp
  inst.Self.Get(id).ToString()
  ```
  由于数值型默认值为 0，直接将未填的选项字符串替换成了 `"0"`！
- **姊妹作参考**：姊妹作在 Rust 端 `wolf-vm/src/text.rs:252` 中明确规定：自变量 5~9 号为纯字符串自变量，当不存在或为空时必须保留为空串，绝不可调用数值获取填入 0。

#### 3. 描边指令 `\E` 后接数字误判为普通文字
- **代码文件**：[TextTokenizer.gd:126-130](../../godot/presentation_gd/TextTokenizer.gd#L126-L130)
- **分析**：
  ```gdscript
  if name == "E" and end < length and source[end].is_valid_int():
      # 误判：以为 \E 后跟数字是普通文本
      tokens.append(Token.text(source.substr(cursor, end - cursor)))
  ```
  在 WOLF 引擎规范中，`\E` 表示开启粗黑描边文本（Edge/Outline 效果），后接数字表示字号或数值。当前分词器错误地认为 `\E` 后面接数字就不是控制码，直接导致 `\E3000G`、`\E10年` 句子开头的 `\E` 裸露打印。

#### 4. 数据库引用标签（`\cdb`、`\udb`、`\sdb`）全线缺失
- **代码文件**：[TextExpander.cs:63](../../godot/core_cs/TextExpander.cs#L63) 与 [TextTokenizer.gd:27](../../godot/presentation_gd/TextTokenizer.gd#L27)
- **分析**：
  WOLF 广泛使用 `\cdb[type:data:field]`、`\udb[...]`、`\sdb[...]` 来动态读取角色名、技能名和物品名。
  1. 在 `TextExpander.cs` 中，代码仅使用 `int.TryParse` 去尝试解析括号内容，遇到包含冒号 `:` 的数据库查询表达式直接失败返回；
  2. 随后该标签流转到 GDScript 的 `TextTokenizer.gd`，分词器根据前缀 `cdb` 错误地将其当作了颜色指令，仅截取了开头的 `\cdb[`，导致后半段 `:0]`、`:1]` 被当成普通文本残留在对话框中。

---

## 五、 整改与修复建议路线图

> **注**：本次阶段严格遵守“仅审查、不修改”原则。以下建议供后续修复阶段参考：

### 阶段一：重构资源解包与打包供应链（优先度：极高）
1. 修改 `work/_make_media_manifest.py`，将资源源路径修正为零售安装根下的 `<retail-root>/Data`；
2. 编写解包脚本批量提取原版 23 个 `.wolf` 数据包（特别是 `DLC_data.wolf`、`cg.wolf`、`Picture.wolf`、`hscene.wolf` 等核心包）；
3. 重新生成完整的 `media.pak`，将 2,215 个立绘与 316 个音频全部纳入。

### 阶段二：修复键鼠映射与输入路由（优先度：高）
1. 在 [InputRouter.gd](../../godot/presentation_gd/InputRouter.gd#L140) 的 `held_mask()` 中加入右键判定：
   `if _right_click_latch or Input.is_mouse_button_pressed(MOUSE_BUTTON_RIGHT): mask |= MASK_CANCEL`；
2. 为鼠标右键增加类似左键的 `_right_click_latch`，并在帧末由 `consume_frame()` 统一重置；
3. 按 WOLF 蓝本将 [InputRouter.gd](../../godot/presentation_gd/InputRouter.gd#L27) 中的 `KEY_SPACE` 保留在 `ACTION_KEYS`（确认），不要移入取消键；
4. 在 [ChoiceWindow.gd](../../godot/presentation_gd/ChoiceWindow.gd) 中对鼠标 Hover 增加位移门限检测，防止其覆盖键盘和滚轮的选中状态；
5. 将鼠标滚轮事件转化为 WOLF 键值 `20` / `21` 打包入掩码，实现存档界面的滚轮翻页。

### 阶段三：对齐文本解析器与姊妹作标准（优先度：中）
1. 改造 [ChoiceWindow.gd](../../godot/presentation_gd/ChoiceWindow.gd)，接入 `TextTokenizer` 进行富文本与控制码清洗后再绘制；
2. 修复 [TextExpander.cs](../../godot/core_cs/TextExpander.cs#L76)，禁止对 5~9 号空字符串槽位填充 `"0"`；
3. 修正 [TextTokenizer.gd](../../godot/presentation_gd/TextTokenizer.gd)，支持 `\E` 描边指令并剥离 `\cdb` / `\udb` / `\sdb` 数据库转义残余；
4. 增强 [TextExpander.cs](../../godot/core_cs/TextExpander.cs) 支持嵌套中括号栈匹配算法；
5. 在写入 `GameRoot.gd` 的 `_backlog` 前先通过 `TextTokenizer` 剥离转义码。
