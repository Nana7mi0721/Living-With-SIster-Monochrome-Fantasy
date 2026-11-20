# `imoto-life`（`a4319159`）标题异常破案与全游戏素材排布排查报告

- **审阅基线**：`Living-With-SIster-Monochrome-Fantasy` commit `a4319159f14f6eefa1ac42333c77e46742056290`
- **对照证据**：实机标题异常截图（`image_b56d50.jpg`）、DeepSeek 插桩日志与原版参考截图（`游戏示例图片.zip`：`010_标题_三行菜单.png`、`017_对话_开场妹妹叫醒.png`、`005_菜单_初期继承加点.png`）
- **核心结论**：标题画面的“巨型日文「も」+ WOLF 示例背景”已 **100% 坐实为两个独立的代码/资产缺陷叠加**（与 290 特效、文字恢复、字体字形错误无关）；同时顺藤摸瓜在 `PictureStage.gd`、`GameRoot.gd`、`UiSkin.gd` 与 `AssetDB.gd` 中定位到 **7 项影响全游戏素材排布与渲染的结构性问题**。

---

## 一、标题画面异常破案结论

截图里覆盖全屏的巨大黑字白边**不是日文「も」，而是中文「点」字**（上方「卜」、中间「口」、下方四点，左下角为前一个字的半个标点）。整张异常画面由以下两个缺陷叠加产生：

### 1. 巨型「点」字 = `Pic12001`（`SystemFile/Button01.png`）被 `backdrop := number >= 12000` 放大 16.5 倍
- **触发链路**：`TitleMap.mps`（id=1008）Event 2 的 `pc90` 绘制 `Pic12001` = `SystemFile/Button01.png`（原图 **606×35** 的中文提示横条，中心锚点 `anchor=1`，坐标 `(512, 520)`）。
- **代码根因**：`godot/presentation_gd/PictureStage.gd` 在 **`:330`、`:416`、`:482`、`:879`** 四处硬编码 `var backdrop := number >= 12000`，强制将所有 `>= 12000` 的图片设为 `size = 1024×576`、`position = (0, 0)`，并在 **`:437`** 为 `TextureRect` 设置了 `STRETCH_KEEP_ASPECT_COVERED`。
- **几何还原**：`606×35` 横条按 `KEEP_ASPECT_COVERED` 填满 `1024×576` 时，高度放大 `576 / 35 ≈ 16.46` 倍（宽度拉至 `9974 px`）并居中裁切只保留中间 `1024 px`，刚好把提示条最中间的「**点**」字放大 16.5 倍铺满屏幕。这也完全解释了隐藏 `Pic12000` 后背景消失、巨字仍在且场景树中没有任何 `Label` 的现象。
- **连带影响**：WOLF 的 `12000+` 号段广泛用于系统 UI（标题提示条 `12001`、公告/设置窗口 `12500/12501`、菜单按钮等），`number >= 12000` 一刀切拉伸会把所有高号段 UI 全部拉爆成全屏。

### 2. 背景变成 WOLF 示例图 = 缺 `Picture/title01.jpg` + 兜底路径写错目录
- **触发链路**：`TitleMap.mps` Event 2 `pc87` 请求 `Picture/title01.jpg`，但当前 `work/all` 与 `media.pak` 中均缺失该文件。
- **代码根因**：`godot/presentation_gd/PictureStage.gd:413-414` 写了：
  ```gdscript
  if texture == null and (number >= 12000 or file.contains("SystemGraphic/Title")):
      texture = assets.texture("SystemFile/TitleGraphic.png")
  ```
  `SystemFile/TitleGraphic.png` 是 WOLF 编辑器自带的 320×240 示例图（棋盘格 + 蓝衣忍者 + 「サンプルゲーム」），而非正版 1024×576 黑白标题图 `SystemGraphic/TitleGraphic.png`；且条件包含 `number >= 12000`，会导致任何 `12000+` 槽位缺图都变成标题图。

---

## 二、全游戏其他素材排布与渲染隐患清单

结合原版参考截图与最新源码，按严重度整理出以下 **7 项结构性问题**：

| 编号 | 严重度 | 位置 | 问题描述与实机后果 | 最小修复方案 |
| --- | --- | --- | --- | --- |
| **L1** | **P0** | `godot/presentation_gd/PictureStage.gd:127-134` | **同层图片缺少按槽位号 `number` 的 Z-Order 排序**。所有图片都在 `Band10` 按绘制先后 `add_child()`，未按 WOLF 槽位号升序排列。只要背景或低号底图重绘（`_free_picture` + `add_child`），就会被插到节点树最末尾，**直接盖死立绘、服装、按钮与 HUD**。 | 在 `band.add_child(node)` 后按 `number` 升序调用 `band.move_child(node, insert_idx)`（保持与 WOLF 槽位遮挡规则一致，不受 `z_index` 4096 上限影响）。 |
| **L2** | **P0** | `godot/presentation_gd/PictureStage.gd:430-437, 491, 898` | **缩放图片时子节点 `TextureRect` 尺寸纹丝不动**。`_build_picture` 创建 `TextureRect` 时只设了一次初始 `rect.size`，未设 `PRESET_FULL_RECT` 锚点；后续 `_apply_picture_geometry` 与 `_render_picture` 只改外层 `holder.size`，导致 `scale != 100%`、`290 zoom`、`290 breath` 在画面上**大小完全不变**；且 `STRETCH_KEEP_ASPECT` 会锁死 `scale_mode == 3` 的纵向独立缩放。 | 给 `TextureRect` 设 `rect.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)`，并将 `stretch_mode` 改为 `TextureRect.STRETCH_SCALE`。 |
| **L3** | **P1** | `godot/presentation_gd/PictureStage.gd:416-436` | **`kind == 3 / 4` 窗口图片未画九宫格，且宽高被误当精灵表行列数**。全作有 667 个 `kind 3/4` 窗口站点（公告框 `#12500`、菜单/状态底框等）；当前把 `args[1]/args[2]`（如 `300×200`）当 `columns/rows` 去切 18×18 的 `WindowBase.png`，切不出有效格就退回 **18×18 小灰点**，窗口面板全部塌缩。 | 对 `kind == 3 / 4`（当窗口宽高 `> 2` 时）使用 `NinePatchRect`（`patch_margin_left/top/right/bottom = 3`，贴图取 `file` 或默认 `SystemFile/WindowBase.png`），禁止进入精灵表分格分支。 |
| **L4** | **P1** | `godot/presentation_gd/GameRoot.gd:448-455` | **`Sys3/Sys4 < 0`（故意藏在屏外的原生 `102` 选项窗）被强行拉回画面正中央**。对比 `010_标题_三行菜单.png`，图片按钮菜单常把 `Sys3/Sys4` 设为 `-1000` 隐藏原生选择窗，但 `_choice_position()` 将所有 `<= 0` 回退到 `(260, 220)`，会把原生 `["1","2","3"]` 灰框砸在标题图中间。 | 仅在 `Sys3 == -1` 或 `Sys4 == -1`（WOLF 默认位置哨兵）时回退默认坐标；其余负值或屏外坐标保持屏外不绘制，仅保留键盘/鼠标选择逻辑。 |
| **L5** | **P1** | `godot/presentation_gd/GameRoot.gd:627-628` | **常驻「返回标题」按钮直接遮挡顶栏右上角「所持金」**。对比 `017_对话_开场妹妹叫醒.png` 与 `005_菜单_初期继承加点.png`，顶栏 `y = 0..36` 最右侧为所持金（如 `3611 G`），而 `_title_button` 固定画在 `(892, 8)` 大小 `120×30`，刚好将其完全挡住。 | 移除常驻按钮，改为 `F12` 快捷键呼出确认框（或仅在鼠标悬停顶部边缘时浮现）。 |
| **L6** | **P1/P2** | `godot/presentation_gd/PictureStage.gd:185-201, 442-466`；`UiSkin.gd:16-24` | **文本图片丢失颜色/描边 + 字体日文优先 + 缺图占位块过大**：1) `kind == 2` 文本图片仅用纯白无描边 `Label`，丢弃 `\c[n]` 颜色与局部字号，在黑白浅底漫画图上看不清；2) `UiSkin.gd:16-24` 将日文字体（`Yu Gothic UI`/`MS Gothic`）排在简中字体前，易出现异体字/混排；3) 缺图时画 `420×140` 不透明大色块会遮死主角立绘。 | 1) 为 `kind == 2` 补上黑色描边（`outline_size`）并按 `TextTokenizer` 支持分段颜色；2) `UiSkin.FONT_NAMES` 针对汉化版将 `Microsoft YaHei` / `Noto Sans CJK SC` 提前；3) 发布态缺图仅记 `push_warning`、不画 `420×140` 实体方块。 |
| **L7** | **P2** | `godot/presentation_gd/AssetDB.gd:89-91, 102-123` | **同名文件目录碰撞与 `Picture/word_effect*` 笔误兜底缺失**。`_basenames` 采用遍历先到先得，存在 `SystemFile/TitleGraphic.png` vs `SystemGraphic/TitleGraphic.png`、`word/word_effect30.png` vs `hword/word_effect30.png` 碰撞风险；事件 354 的 7 处 `Picture/word_effect*.png` 笔误未做 `-> hword/word_effect*` 定向回退，可能错拿 `word/` 下的 `30×100` 占位图。 | 在 `AssetDB.resolve()` 中增加 `Picture/word_effect* -> hword/word_effect*` 窄回退，并让 `_index()` / `_open_media_pack()` 按确定性目录优先级写入 `_basenames`（`SystemGraphic/` 与 `hword/` 优先于 `SystemFile/` 与 `word/`）。 |

---

## 三、推荐修复顺序与验收标准

### 第一批（立即执行，解决所有显著排版与遮挡灾难）
1. **拆除 `number >= 12000` 全屏拉伸**（`PictureStage.gd:330, 416-437, 482-495, 879-883`），并同步更新 `godot/tests_gd/LayerOrderTest.gd:60-66` 的旧断言。
2. **修正标题兜底与补齐资产**：`PictureStage.gd:413-414` 收窄为仅匹配标题图且优先兜底 `SystemGraphic/TitleGraphic.png`；从原版 `Data/Picture.wolf` 补回 `Picture/title01.jpg` 与 `SystemGraphic/TitleGraphic.png` 重打 `media.pak`，清理 `work/all/SystemFile/` 下残留的 320×240 示例图。
3. **补齐 L1（`Band10` 按槽位号升序 `move_child`）+ L2（`TextureRect` `PRESET_FULL_RECT` + `STRETCH_SCALE`）+ L3（`kind 3/4` `NinePatchRect` 九宫格）**。

### 第二批（UI 细节对齐原版截图）
4. **修复 L4（`Sys3/Sys4` 仅 `-1` 回退默认，`-1000` 保持屏外）+ L5（移除遮挡所持金的常驻「返回标题」按钮，改 `F12`）**。
5. **修复 L6（文本图片描边/颜色、中文字体优先、发布态隐藏缺图色块）+ L7（`AssetDB.gd` `hword/word_effect*` 回退与确定性目录优先级）**。

### 验收方法
1. 跑通 `bash tools/godot_check.sh` 与 `LWsmf_RENDER=1 bash tools/godot_check.sh` 全量门禁。
2. 使用 `--write-movie --fixed-fps 30 --quit-after 380` 抓帧，对照 `010_标题_三行菜单.png`、`005_菜单_初期继承加点.png`、`017_对话_开场妹妹叫醒.png` 确认：
   - 标题页显示 1024×576 黑白妹妹标题图，底部 `(512, 520)` 正常显示 606×35 中文提示条，无巨字、无示例图、无居中灰框；
   - 进入卧室与加点/对话场景后，顶栏「所持金」无遮挡、立绘缩放与叠加层级正确、九宫格窗口框完整不塌缩。
