# F3 渲染/图层最终 diff 独立复核（task-13）

- 复核基线：HEAD `8c65153`（2026-11-03 13:15:04 +0800），工作树包含并发修复。
- F3 声明范围：`crates/wolf-game/src/picture.rs`、`atlas.rs`、`window.rs`，以及配套测试。
- 实际共享工作树还包含 `game.rs`、`main.rs`、`play.rs` 的并发改动；本报告按当前工作树行为复核，并单独标注归属风险。
- 约束：未修改任何源码、测试、release、manifest 或存档；截图/日志只写 `work/out/reviews/`。

## 0. 结论摘要

| 验收项 | 判定 | 摘要 |
|---|---|---|
| UI-05 标题进入游戏 913–915 黄绿闪 | **PASS（像素验收）** | 当前构建逐帧黄绿像素 0.00%；旧构建同帧 87.1%。证据 `work/out/reviews/ui05_fix_check/f913.png`～`f930.png`。 |
| UI-05 实现路径 | **部分 PASS / 待定** | 真实生效的是 `game.rs:1317-1327 retire_screen_for_map_change` 的四帧保留，不是 `picture.rs:1177 clear_screen_layers`；后者当前无生产调用，且不是真正的 30 帧 erase fade。 |
| UI-12 19:30 方针/公会评价面板 | **PASS（几何/层级，非回归）** | 530x84 at (40,50) 与零售 `work/retail_backup/Save/save7.jpg` 同构；用户截图 `work/out/ui_bugs/07_room_1930_policy_panels_1027x580.png` 有 6px Windows 窗口 chrome 偏移。文字挤叠是独立 typography 问题，需另立任务。 |
| UI-13 LayerBand 端到端 | **FAIL** | `window.rs` 定义了完整 LayerBand，但 `play.rs`/`main.rs` 完全未接入；两个 render pass 的顺序仍会把 scene text、message text、nameplate 放到错误层。 |
| PANEL_PICTURE 与 TITLE_BACKDROP 冲突 | **PASS** | `PANEL_PICTURE=-1000`，`TITLE_BACKDROP_LAYER=-100`；新增测试防碰撞。 |
| retire 同槽同文件误删 | **FAIL（代码审查，未运行时复现）** | 清理只比较 `(number,file)`；新地图在 4 帧内把同一负槽画成同一文件会被误删。任务要求“不误删新地图同槽”未完全满足。 |
| m11 视觉回归 | **FAIL（当前共享工作树）** | 3 passed / 2 failed；详见第 6 节。不能直接归因 F3，需 Lead 在集成任务隔离。 |

**总判定：UI-05 的行为验收与 UI-12 的几何问题可以接受；UI-13 端到端不能通过，F3 不能以“渲染/图层已全部修复”合并。** `review_render_fix.md` 建议作为 task-13 的 FAIL/部分 PASS 记录，并把 host 合成转给 task-14。

## 1. 图层绘制顺序的真实规则

### 1.1 脚本数据层：Screen/BTreeMap

- `picture.rs:1099-1106`：`Screen { pictures: BTreeMap<i32, Picture> }`；注释明确“WOLF composites pictures in ascending number order”。
- `picture.rs:1445-1485`：`Screen::quads()` 按 `pictures.values()` 升序生成 quad。
- `picture.rs:936-940`：`Picture::is_screen_layer()` 只判断 `number < 0`；负番号无条件当作全屏层。
- `picture.rs:1462-1471`：全屏层的 dst 强制 `[0,0,1024,576]`，忽略 x/y/scale；正番号才走 `place()`。
- `picture.rs:1459`：window kind 且不是 `<...>` 系统图形的图片从 sprite pass 排除，交给 atlas 的 nine-slice panel pass。

所以脚本层的基础顺序是：

```text
负番号全屏层（按 number 升序，-100 先于 -2 先于 -1）
  -> 正番号普通场景图（按 number 升序）
  -> 正番号 text picture（实际不在 sprite pass，见 1.3）
```

### 1.2 atlas 层

`crates/wolf-game/src/atlas.rs:504-615`：

1. `510-552`：先收集所有 `kind.is_window()`、非 sysgraphic、`window > MIN_BOX`、`on_screen` 的 panel；新增 `516-522` 跳过 hidden/opacity<=0。
2. `553-567`：把 nine-slice panel piece push 到 `out`。
3. `568-607`：遍历 `screen.quads` push 普通 sprite；`569` 跳过 `kind.is_text()` 与 src 0 尺寸。
4. `608-614`：`out.sort_by_key(|quad| (window::picture_band(quad.number).rank(), quad.number))`。

`window.rs:49-110` 的 LayerBand：

```text
Map=0, ScenePicture=10, SceneText=11,
MessageFrame=20, MessageText=21, Nameplate=22,
ChoicePanel=30, ChoiceText=31, Cursor=40
```

`picture_band()` 只分三类：`number<0 -> Map`；`19500..=19503 -> MessageFrame`；其余正数 `ScenePicture`。它不处理 text picture（text 走 glyph pass），也不处理 host panel；host panel 必须显式给 band。

### 1.3 text 层与两个 render pass

- `atlas.rs:569` 明确跳过所有 text picture；`atlas::texts()`（`619-636`）把它们单独变成 `TextQuad`。
- `game.rs:1757-1777 text_quads()` 按 `self.screen.iter()` 升序遍历 `kind.is_text()` 且非 hidden/非空的图片，生成 markup-aware text runs。
- `play.rs:531-533` 与 `play.rs:629-631`：先 `renderer.draw(&self.sprites)` 画完所有 sprite，再 `renderer.draw_text(&items)` 画所有 text。
- `main.rs:2321` 与 `main.rs:2396` 的 capture 路径完全同序。

**因此真实帧顺序不是 LayerBand 顺序，而是：**

```text
sprite pass: 所有 tile / picture quad / panel / choice panel / cursor / nameplate panel
text pass:   所有 text_quads / message runs / choice rows / nameplate text
```

这就是 UI-13 的核心：只要一个场景 text picture 的编号是 1600，它的字形仍在所有 sprite 之后绘制，能覆盖 `MessageFrame=20` 的消息框；只要 choice row 是 text，它就会在 message text 之后（当前代码），从而反向覆盖消息文本。

### 1.4 Window/Screen 负番号、全屏层、消息层、HUD 层如何叠加

- **负番号/全屏层**：`Screen::quads` 先按 number 升序，且 `number<0` 一律覆盖整屏；`apply_screen_tint` 只对负番号层（`quad.number < 0`）和地图 tile 生效（`game.rs:1155-1175`）。
- **消息框**：CE63 把 frame 画在 `message::FRAME_PICTURE=19500`（`message.rs:60`）及 19501–19503；`picture_band` 把它们归 `MessageFrame=20`。frame 是 script window picture，由 atlas panel pass 生成 nine-slice。
- **消息文字**：不是 picture，而是 `game.message_layout()` 的 runs，由 `build_text_items` 追加到 text pass；没有自己的 sprite band。
- **HUD 文本**：CE232 的 1601+ 文本图，编号正数，`text_quads` 生成；理论上是 `SceneText=11`，实际在 text pass 中最后绘制，能覆盖消息框。
- **选项框**：`choice_overlay` 的 panel 由 host append 到 sprites，rows 由 host append 到 texts，number 固定 `window::PANEL_PICTURE=-1000`；host 从未按 band 排序。
- **名牌**：`message_nameplate` 的 nine-slice panel 是 host 最后 append 的 sprite，文字是 text pass 最后 append 的 item；理论 band 是 `Nameplate=22`，实际高于 choice panel/choice text。

### 1.5 被破坏的路径

1. **text pass 全局后置**：`play.rs:531-533` / `main.rs:2321,2396`。这使所有 `SceneText`（HUD/1601+）高于 `MessageFrame`/`ChoicePanel`，是 UI-13 未完成的根因。
2. **host append 顺序**：`play.rs:315-318` choice panel 后，`321-350` cursor，`354-367` nameplate panel；`386-439` text 顺序为 scene text → message runs → nameplate。全部没有 band 参数。
3. **atlas 的 band 排序只覆盖 script sprite**：`atlas.rs:608-614` 的注释说“future caller”，说明 host 合并尚未发生；`LayerBand` 全仓 grep 只出现在 `window.rs`、`atlas.rs`、`m12_render_layer_priority.rs`，`play.rs`/`main.rs` 零引用。
4. **PANEL_PICTURE 的潜在陷阱**：`window.rs:99` 注释直接写 host UI 不得从 `PANEL_PICTURE` 推断 band；但 `picture_band(-1000)` 会返回 `Map`。当前 host 没把 panel 送进 atlas，所以未触发；未来合并显示列表若忘记显式 band，会把 host UI 排到地图层下面。
5. **消息 frame 的 19500 范围**：`picture_band` 只认 19500..=19503；如果 CE63 的 frame 片段超出该范围，会退回 `ScenePicture`，需要数据扫描确认（当前代码注释只列这四个槽）。

## 2. UI-05：标题“开始咯”进入游戏黄绿闪

### 2.1 复现命令

```bash
cd "E:/Study/github program/imoto life"
./target/debug/wolf-cli.exe --root release/data capture --boot \
  --frames 913 --key-frame 850:10 --press-frame 900 --pointer 80,425 \
  --out work/out/reviews/ui05_fix_check/f913.png
```

逐帧替换 `--frames 910..930` 可得到 `f910.log`～`f930.log`。

### 2.2 当前构建逐帧数据

| 帧 | live pictures | textured quads | 黄绿像素（PIL） | 画面 |
|---:|---:|---:|---:|---|
| 910 | 5 | 19085 | 0 / 0.00% | 彩色标题菜单，-100 Kagura/bg、-2 TitleGraphic.jpg、3/4/5 menu |
| 912 | 5 | 19085 | 0 / 0.00% | 同上 |
| 913 | 2 | 27077 | 0 / 0.00% | 只剩 -100 + -2 全屏层（旧标题画） |
| 914 | 2 | 27077 | 0 / 0.00% | 同上 |
| 915 | 3 | 27078 | 0 / 0.00% | 增加 4000 cg/blackscreen.jpg alpha 0.03 |
| 916 | 4 | 27079 | 0 / 0.00% | 增加 -1 SystemGraphic/TitleGraphic.png，alpha 0.07 |
| 917 | 2 | 27077 | 0 / 0.00% | 旧 -100/-2 已清，剩 -1 + 4000 |
| 930 | 2 | 27077 | 0 / 0.00% | 稳定 loading 画 |

旧构建同命令（`work/out/reviews/ui05_912.png`～`ui05_917.png`）：

| 帧 | live pictures | textured quads | 黄绿像素 | 备注 |
|---:|---:|---:|---:|---|
| 913 | 0 | 27075 | 513792 / 87.1% | top (150,188,32)/(144,180,31)/(164,206,35) |
| 914 | 0 | 27075 | 513792 / 87.1% | 与 913 相同 |
| 915 | 1 | 27076 | 87.1% | 4000 alpha 0.03 之后仍黄绿 |
| 916 | 2 | 27077 | 0% | -1 标题画出现 |

证据：`work/out/reviews/ui05_contact_sheet.png`（旧构建 912/913/914/915/916/917 的 3x2 对比），当前构建 `work/out/reviews/ui05_fix_check/f910.png`～`f930.png`。

### 2.3 根因链（当前代码）

1. `game.rs:107`：`TITLE_BACKDROP=-100`；`boot()` / `start_title()` 通过 `ensure_title_backdrop()` 画纯黑 `Kagura/bg.png` 到 -100。
2. `game.rs:1275-1303 switch_map()` 调 `retire_screen_for_map_change()`；旧逻辑（以及 `clear_screen_layers` 若被调用）会删除负番号层。
3. `picture.rs:1177-1179` 的 `clear_screen_layers` 只保留 `number>=0 || number==TITLE_BACKDROP_LAYER`；它本身当前没有生产调用（全仓只在 `picture.rs:2813` 测试出现）。
4. 实际修复在 `game.rs:1317-1327`：把所有 `number<0` 的 `(number,file)` 放入 `stale_screen_layers`，设 `stale_clear_countdown=4`；`game.rs:2092-2105` 每帧递减，4 帧后只删除仍匹配同一 `(number,file)` 的旧图。
5. 因此 913–915 旧的 -2 TitleGraphic 仍覆盖全屏；916 新地图 event 画 -1 TitleGraphic，917 旧层清掉。这里没有 30 帧 erase fade，而是四帧“保留旧层后硬切”。

### 2.4 哪个 quad / 裁剪步骤造成黄绿闪

不是某个 quad 的裁剪，而是 **没有任何全屏 quad**：旧代码在 `switch_map` 时清掉 -100（黑底）和 -2（标题画），而新地图的 -1（TitleGraphic.png）要等到 916 才出现；913–915 的可见层只剩 tilemap 的 45000+ 个绿色地块 quad（27075 textured quads），所以整屏为黄绿。修复不需要裁剪，需要保证切换期间至少有一张不透明全屏层（保留 -100，或提前画黑幕/新背景）。

### 2.5 最小修改建议

- **保留 UI-05 行为修复**：`retire_screen_for_map_change` 保留 -100 是正确方向；建议把“四帧固定”改为“直到新地图第一张全屏背景（-1/-2）出现，或最多 N 帧，超时用黑色 150 覆盖”，避免新地图 event 延迟超过 4 帧时再次露 tilemap。
- **不要删除素材或 tilemap**；不要用改变地图 tile 颜色的方式掩盖。
- **同槽误删**：给 `Picture` 增加 `insert_generation`（或保存 `age` 快照），清理时比较 generation；不能只比较 `(number,file)`。
- **若要求真正 30 帧 fade**：让 `switch_map` 对 -2 使用 `150 [2,12000,30,5]` 的 `erase_fade` 路径，或由新地图 event 先画黑幕再淡入；当前实现只是硬切，不能宣称“fade 已实现”。
- **测试**：逐帧捕获 913–917，断言不存在 `(r>110,g>120,b<80)` 且 g-r>20 的黄绿像素；再构造“新图 5 帧后才画 -1”的 fake map，断言过渡期间全屏层非空。

## 3. UI-12：19:30 房间方针/公会评价面板

### 3.1 证据

- 用户截图：`work/out/ui_bugs/07_room_1930_policy_panels_1027x580.png`（Windows 窗口有 6px chrome，面板视觉上比游戏内低 6px）。
- 正确对照：`work/retail_backup/Save/save7.jpg`（1024x576） vs 当前 `work/out/reviews/task13_save7_current.png`。
- 500KB 截图 `work/out/reviews/save7.png` 是旧 Rust 版本；当前与旧版全屏 diff mean 0.1135/255、pct(max>8)=0.09%，基本同图。
- 当前 vs retail：全屏 diff mean 4.4526/255、max 255、pct(max>8)=4.9179%（JPEG/抗锯齿差异）。
- 对比图：`work/out/reviews/task13_ui12_compare.png`（上当前 Rust、下 retail）与 `work/out/reviews/ui12_compare.png`（旧 Rust/retail）。

### 3.2 代码/几何

- `game.rs` 当前 SaveData07 的 slot 1600 是 `Picture/policy.png` 或 `policy2/3.png`；`destination(530,84)` = `[40,50,530,84]`。
- 动态文本 1601/1602/1610 等按脚本坐标绘制；`Picture/policy.png` 是 530x84 已含静态标签的底图，所以标签与动态数字的挤叠是素材/脚本原始布局，不是 Sprite pass 缩放造成。
- `crates/wolf-game/tests/m12_render_ui12_policy.rs:57-126` 只断言：slot 1600 文件名为 policy.png/policy2.png/policy3.png、destination 530x84 at (40,50)、1601/1602/1610 文本 y 在 strip 内。**没有像素对比、没有与 retail 同帧 diff 的断言。**

### 3.3 结论

- **UI-12 的“尺寸/图层异常”在 Current HEAD 不是 Rust 重写回归**；与零售 save7 的面板几何/层级同构。
- 用户截图的“面板低 6px”来自窗口 chrome，不是游戏坐标；应以 1024x576 的 save7 对比。
- **文字挤叠（尤其“方针 烦恼全开”“公会评价 中坚公会 状态精神无比”）是独立 typography/字体度量问题**，不是 task-6 的 panel 排序问题，也不应算 UI-12 的 LayerBand 修复。建议另立任务；若用户坚持 UI-12 包含可读性，需提供另一张无窗口 chrome 的游戏内截图作为验收基准。

## 4. UI-13：LayerBand 端到端 FAIL

### 4.1 规格

`window.rs:49-83` 定义的目标顺序：

```text
Map(0) < ScenePicture(10) < SceneText(11)
< MessageFrame(20) < MessageText(21) < Nameplate(22)
< ChoicePanel(30) < ChoiceText(31) < Cursor(40)
```

### 4.2 代码缺口

- **sprite pass**：`play.rs:310-369` 顺序为 tile/picture → choice panel → cursor → nameplate panel。nameplate panel 最后 append，超过 choice panel；cursor 不是最后，nameplate 可覆盖 cursor。
- **text pass**：`play.rs:386-439` 顺序为 `text_quads` → `message_layout` runs → nameplate text。因此：
  - `SceneText`（例如 1601+ HUD）在所有 sprite（含 MessageFrame 20）之后；
  - `MessageText`（21）在 `ChoiceText`（31）之后；
  - `Nameplate`（22）文本最后，盖在 `ChoiceText` 上。
- **capture**：`main.rs:2205-2284` 同序：`2207-2209` tile/picture/text；`2215-2218` choice；`2223-2250` cursor；`2255-2267` nameplate；`2321` 先 draw sprites，`2344` styled message text，`2396` draw_text。
- **`LayerBand` 全仓使用范围**：`window.rs`（定义）、`atlas.rs`（脚本 sprite 排序）、`m12_render_layer_priority.rs`（测试）；`play.rs`/`main.rs` 无引用。

### 4.3 具体必然错误

1. 任何 scene text（HUD/1601+）会盖住 101 message frame（`MessageFrame=20`）和 choice panel（`ChoicePanel=30`），因为它在 text pass 最后统一画。
2. 选项文本（choice rows）先 append，message runs 后 append，所以有消息框时 message text 会盖选项文字，尽管 choice 应高于 message。
3. 名牌 panel 是最后 sprite，名牌文字是最后 text；当 choice 覆盖 message 区域时，名牌可能盖住 choice 的边框/文字。
4. cursor 是 sprite，所有 text 都在它之后，所以文字可盖 cursor；cursor 应为 Topmost 40。
5. 现有用户截图 `work/out/ui_bugs/04_dialog_cafe_nameplate_1020x578.png`、`05_nameplate_overlap_crop_394x203.png` 证明了老版名牌压 message frame；新版 `message_nameplate` 已由 task-5 改为 frame top - 8（`game.rs:1679-1701`），旧截图不应再当 current 缺陷。

### 4.4 最小修改建议

- 建立统一的 display list：`struct Item { band: LayerBand, picture_number: i32, sequence: u32, kind: Sprite|Text }`；`Game` 产出带 band 的 item，`play.rs`/`main.rs` 只按 `(band, picture_number, sequence)` 排序后分别提交 sprite/text。
- 具体 band 分配：message frame 20；message text 21；nameplate panel+text 22；choice panel 30；choice text 31；cursor 40；scene text 11。
- 若暂不做大重构：至少在 `play.rs`/`main.rs` 中按固定 band 分组绘制：先 draw Map/ScenePicture sprites，再 draw SceneText，再 draw MessageFrame sprites，再 draw MessageText，再 Nameplate，再 ChoicePanel/ChoiceText，最后 Cursor；但 text 与 sprite 交错需要 renderer 支持按 item 顺序调用两个 API。
- 测试：新增 `m12_ui_layers` 测试，构造 scene text + message frame + choice panel + nameplate + cursor 的合成帧，断言输出序列 band 单调递增；对 `play.rs`/`main.rs` 两条路径都测（或在 `Game` 层输出 display list 后只测一份）。

## 5. 其他 F3 发现

### 5.1 `clear_screen_layers` 是死路径（P2）

`picture.rs:1177-1179` 新增保留 `TITLE_BACKDROP_LAYER` 的逻辑，但全仓只有定义与 `picture.rs:2813` 测试引用，生产路径没有调用；真实 UI-05 修的是 `game.rs:1317-1327` 的 retire。风险：

- 读者会误以为 `clear_screen_layers` 在保护 UI-05；
- 测试 `clear_screen_layers_preserves_title_backdrop` 通过不代表真实切图路径通过；
- 若未来某处再调用它，它会删除 -2 标题画但保留 -100，仍可能短暂露出旧 title art 或黑底。

建议：要么删除/标注 `#[cfg(test)]`，要么让 `switch_map` 使用统一的 retire 路径并把 `clear_screen_layers` 作为测试辅助；补齐“切图帧”测试，而不只是 Screen 单元测试。

### 5.2 retire 同槽同文件误删（P1，代码审查）

`game.rs:1317-1327` 保存 `(number, file)`；`game.rs:2092-2105` 只在 `screen.get(number)` 的 `file == 旧 file` 时删除。若新地图在 4 帧内：

1. 旧地图 -1 = `A.png`；
2. 切图，stale 记录 `(-1, "A.png")`；
3. 新地图 event 在第 2 帧画 -1 = `A.png`（同一路径/素材，甚至同一张图但新的 Picture 实例）；
4. 第 4 帧清理看到 `(-1, "A.png")` 匹配，删除新图；新地图失去背景，再次露出 tilemap。

**未运行时复现**（需要构造 fake map 或事件脚本）；但从代码上无法区分旧/新实例。建议给 `Picture` 加 `insert_seq: u64`（每次 `Screen::insert/apply Draw` 递增），stale 保存 `insert_seq`，清理只删旧 seq；或保存 `age_at_retire` 并在清理时核对。新增单测：旧 -1 A -> 2 帧后新 -1 A -> 清理后仍存在。

### 5.3 连续 map change 覆盖 stale 列表（P2）

`stale_screen_layers` 是单个 `Vec`；`retire_screen_for_map_change` 每次直接覆盖。若第一次切图 4 帧内发生第二次切图，第一批 stale 层永远不会被清理，叠在新地图上。建议用代际队列或合并去重；测试连续两次 `switch_map`。

### 5.4 `merge_args` raster window 全黑隐藏范围过宽（P2，未全数据验证）

`picture.rs:1861-1895`：对 `kind.is_window() && !file.starts_with('<')` 且 `tone=(0,0,0)` 的图片统一设 `hidden=true`，注释只说明用于 `CursorBase.png`。如果还有别的 raster window 正常使用零 tone（不是光标黑块），也会被永久隐藏。需要对全数据统计所有 window picture 的 tone；至少加白名单/按文件名限定，并补测试。

### 5.5 PANEL_PICTURE=-1000 的 latent trap（P2）

`window.rs:47` 的注释已经说 host panel 不能用 number 推断 band，但 `picture_band(-1000)=Map`（`window.rs:101-110` 只看 `number<0`）。当前 host 不把它送进 atlas，故无实际 bug；如果 task-14 合并列表时直接调用 `atlas.sprites` 或复用 `picture_band`，host UI 会掉到地图层。建议 `picture_band` 对 `PANEL_PICTURE` 单独返回一个 `HostPanel` 或 panic，避免误用。

### 5.6 m11 视觉回归失败（集成阻塞，不归因 F3）

命令：

```bash
TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-cli --test m11_visual_regression
```

结果：`3 passed; 2 failed`。日志 `work/out/reviews/task13_m11.log`：

- `h_overlay`: `left: 527554`, `right: 527713`（-159），`crates/wolf-cli/tests/m11_visual_regression.rs:382`。
- `story_map_idle`: `left: 544431`, `right: 544394`（+37）。
- 同日志的 capture 显示 `pictures 49 live -> 27662 quads`（baseline 27661，+1）；h_overlay `text 17 items/59 glyphs`（baseline 60）。

这两个失败可能来自并发 task-5 的文本/名字牌改动，未做隔离实验，**不能直接归因 task-6**；但当前共享工作树不能通过 m11，Lead 在 task-7 集成前必须处理或重建基线。

## 6. 测试结果

| 命令 | 结果 |
|---|---|
| `TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-game --test m12_render_layer_priority` | 6 passed |
| `TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-game --test m12_render_ui12_policy` | 2 passed |
| `TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-cli --test m11_visual_regression` | 3 passed, 2 failed（见 5.6） |

`m12_render_layer_priority.rs:1-16` 文档标题声称“host and script UI”，但实际 `:38-173` 只断言 LayerBand 常量、PANEL_PICTURE 不撞、`picture_band` 边界、atlas 顺序 `[-100,-2,10,500]`；**没有 host 合成断言**。这是 UI-13 仍 FAIL 的直接测试缺口。

`m12_render_ui12_policy.rs:57-126` 只测 geometry/file/y；没有 retail 像素比较，也没有验证文字重叠。测试通过不能宣称 UI-12 像素级修复。

## 7. 归属与建议

- **素材/数据完整性**：`git diff --name-status -- release/data work/all work/retail_backup Save` 为空；`git status --short` 无删除项。F3 没有靠删素材/改存档规避问题。
- **F3 可接受部分**：`atlas.rs` 的 hidden/opacity skip、LayerBand 常量、`PANEL_PICTURE` 防撞、UI-05 的像素行为（由 game.rs retire 实际实现）。
- **F3 不能接受部分**：UI-13 端到端未接 host；retire 同槽同文件误删；`clear_screen_layers` 死路径/测试假覆盖。
- **task-14 必须做**：按 4.4 接入 host LayerBand；建议同时修 `message_nameplate` 与 choice 的合成顺序测试；补 `m12_ui_layers`。
- **task-5 的 UI-07**：`message_nameplate` 已改为 frame top - 8（`game.rs:1679-1701`），旧截图 `work/out/reviews/ui07_nameplate_annotated.png` 是修复前证据，不应再作为 current 缺陷；请用当前真实说话人场景复拍确认。
- **UI-12**：不建议再改 panel 几何；若要修文字挤叠，单开 typography 任务，不要混入 F3。

## 8. 新发现/未登记问题候选

| 编号 | 严重度 | 问题 | 证据 | 建议 |
|---|---|---|---|---|
| NEW-F3-1 | P1 | host `LayerBand` 未接入，text pass 全局后置，UI-13 端到端仍失败 | `play.rs:310-439`, `main.rs:2205-2396`；`LayerBand` 无 host 引用 | 统一 display list；task-14 |
| NEW-F3-2 | P1 | retire 清理只比 `(number,file)`，同槽同文件新图会被误删 | `game.rs:1317-1327,2092-2105`；代码审查，未运行时复现 | 增加 `insert_seq`；补 fake map 测试 |
| NEW-F3-3 | P2 | 连续 map change 覆盖 `stale_screen_layers` | `game.rs:1317-1327` | 队列/合并 |
| NEW-F3-4 | P2 | `clear_screen_layers` 是死路径，UI-05 测试假覆盖 | `picture.rs:1177-1179`；全仓仅测试引用 | 删除/标注，补切图帧测试 |
| NEW-F3-5 | P2 | `merge_args` 对 raster window tone0 统一 hidden，未验证全数据 | `picture.rs:1861-1895` | 限定 CursorBase 或统计 tone |
| NEW-F3-6 | P2 | `picture_band(PANEL_PICTURE)` 返回 Map，未来 host 合并会踩坑 | `window.rs:47,101-110` | 对 PANEL 单独处理/panic |

## 9. 复现与证据索引

- UI-05 当前：`work/out/reviews/ui05_fix_check/f910.png`～`f930.png`，同名 `.log`。
- UI-05 旧：`work/out/reviews/ui05_912.png`～`ui05_917.png`，`ui05_contact_sheet.png`。
- UI-12：`work/out/reviews/task13_save7_current.png`、`task13_ui12_compare.png`、`save7.png`、`work/retail_backup/Save/save7.jpg`；用户截图 `work/out/ui_bugs/07_room_1930_policy_panels_1027x580.png`。
- 消息/名牌：`work/out/reviews/ui06_msg_600f.png`、`task13_ui07_current.png`；旧 `ui07_nameplate_annotated.png`（修复前，勿作 current 证据）。
- m11：`work/out/reviews/task13_m11.log`。
- 代码：`crates/wolf-game/src/picture.rs:1099-1106,1177-1179,1445-1485`；`atlas.rs:504-615`；`window.rs:39-110`；`game.rs:107,1275-1327,1679-1701,1757-1777,2092-2105`；`message.rs:60`；`play.rs:310-439,531-533`；`main.rs:2205-2396`；测试 `m12_render_layer_priority.rs:1-173`、`m12_render_ui12_policy.rs:57-126`、`m11_visual_regression.rs:382`。
