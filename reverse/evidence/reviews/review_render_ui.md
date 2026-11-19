# 渲染 / UI / 图层独立审计（task-1）

- 审计范围：`crates/wolf-game/src/{picture,atlas,window,textpic,message,choice,game}.rs`、`crates/wolf-cli/src/{main,play}.rs`、`crates/wolf-render/src/renderer.rs`、`crates/wolf-vm/src/{exec,text}.rs`（只读）
- 代码快照：`git rev-parse --short HEAD` = `8c65153`（2026-11-03 13:15:04 +0800）。工作树 dirty（20 个已修改文件，含 `game.rs` 的 title/backdrop/message speaker 改动，+535 行），本报告按**当前工作树源码行号**审计，不是纯 HEAD 快照。
- 审查依据：`AGENT.md`、`docs/README.md`、`docs/UI_BUG_BACKLOG.md`、`docs/ENGINE_STRATEGY.md`、`docs/OPCODE_101.md`、`docs/TEXT_SPEC.md`、`docs/research/UI_LAYOUT.md`
- 产物目录：`work/out/reviews/`；用户截图目录：`work/out/ui_bugs/`

## 0. 直接回答

### 0.1 图层绘制顺序的真实规则

当前实现不是“一个 BTreeMap 排到底”，而是**四段固定 pass**：

| pass | 内容 | 顺序规则 | 代码 |
|---|---|---|---|
| 1 | 地图 tile | `tile_quads` 全量先画 | `crates/wolf-game/src/game.rs:1096-1113` |
| 2 | `Screen` 的 raster picture + window 九宫格 | `BTreeMap<i32,Picture>` 升序；负番号先画并强制全屏；同号 panel 在 content 前；最后 stable sort by `picture.number` | `crates/wolf-game/src/picture.rs:1427-1454`、`crates/wolf-game/src/atlas.rs:504-610` |
| 3 | 宿主 UI 的 sprite：选择面板 → 101 光标 → 名牌面板 | **追加顺序，不按番号排序** | `crates/wolf-cli/src/play.rs:288-348`、`crates/wolf-cli/src/main.rs:2185-2248` |
| 4 | 所有文本：text picture → choice 行 → message 行 → 名牌文字 | **统一 text pass，永远在全部 sprite 之后** | `crates/wolf-render/src/renderer.rs:659-667`、`884-918`；`play.rs:368-419`、`513-515`；`main.rs:2324-2376` |

所以“真实规则”可以写成：

`地图 tile → Screen raster (负番号全屏 → 非负升序，同号 panel 先) → host sprites (选择面板 → 光标 → 名牌面板) → 全部文本 (text picture → choice 行 → message → 名牌文字)`

**WOLF 原生设计里，text picture 确实是独立 band（20/50/70），高于地图和角色**：`renderer.rs:659-667` 的注释明确写了这一点。因此“text picture 不在 sprite 的 BTreeMap 里”本身不是 bug；UI-13 要修的是**宿主 UI（choice/message/nameplate）没有自己的 band，却被塞进了一个无条件的 text pass**。

### 0.2 Screen / BTreeMap 升序被哪些路径破坏

按“是否应严格按 `picture.number` 升序”分类：

1. **`atlas.sprites` 跳过所有 text picture**（`crates/wolf-game/src/atlas.rs:568-607`，`:569` `if quad.kind.is_text() ... continue`）。这些图不在 sprite pass 排序，而在 text pass 最后画。**这是 WOLF 的 text band 设计**，不是回归；但跨类型比较时，它等价于“text picture 永远大于所有 raster number”。
2. **宿主选择面板不参与排序**：`play.rs:288-299` 把 `choice_overlay` 的 panel `append` 到 `sprites` 末尾，注释自己写明“panel appended last so composites over every script layer”。`choice.rs:236/251/264` 的 panel number 固定为 `window::PANEL_PICTURE = -100`，但 append 顺序使它实际永远在最上层，和 `-100` 无关。
3. **名牌面板不参与排序**：`play.rs:334-348` 在 choice 之后 append 九宫格 panel，number 也是 `PANEL_PICTURE=-100`，所以**名牌面板实际压在选择面板之上**，违反 UI-13 的“选择弹窗 > 消息框”。
4. **全部文本 pass 不参与 sprite 排序**：`play.rs:368-419` 的 `build_text_items` 固定顺序为 text_quads → choice rows → message runs → nameplate text；`main.rs:2324-2376` 同序。于是 HUD 文本（1601+）会盖住番号更高但已在 sprite pass 画出的消息框 19500。
5. **`clear_screen_layers` 只按符号过滤，不是排序**：`picture.rs:1160-1162` `retain(|number,_| *number >= 0)`。它把**所有负番号层**一次删掉；UI-05 就是这条路径删掉了 `TITLE_BACKDROP=-100` 和 `-2` 标题图。
6. **`TITLE_BACKDROP` / `PANEL_PICTURE` 的 -100 编号冲突**：`game.rs:78-87` 用 `TITLE_BACKDROP=-100` 作标题黑底；`window.rs:39-44` 用 `PANEL_PICTURE=-100` 作宿主 UI 哨兵，注释还写“scripts use >= -2”。两个语义共用一个 number 是潜在排序炸弹（详见第 7 节 NEW-1）。

### 0.3 Window / Screen 的负番号、全屏层、消息层、HUD 层如何叠加

- **负番号层**：`picture.rs:1427-1454` 的 `quads()` 按 `BTreeMap::values()` 升序遍历；`dst` 被强制为 `[0,0,1024,576]`，即全屏层。负得越小越先画；`-100`（黑底）在 `-2`（标题画）下面，这是正确的。
- **Window 九宫格**：`atlas.rs:510-552` 先扫描一遍所有 `kind.is_window` 且非 sysgraphic、`window > MIN_BOX=2`、`on_screen` 的 picture，生成 nine-slice panels；`:553-567` push 到 out；`:608` 最后按 number 稳定排序。同号时 panel 在普通 content 之前（panel 是底，content 是面）。
- **消息层**：`message::FRAME_PICTURE=19500`（`message.rs:60`），作为普通 window picture 走 sprite pass；`message_nameplate()`（`game.rs:1446-1457`）生成的名牌 panel 在 `play.rs:334-348` 被 append 到 sprites **最后**；消息文字在 `build_text_items` 里最后画。消息层本身没有独立 band，靠“最后 append + 最后 text”实现，所以任何更晚 append 的宿主元素都能压它。
- **HUD 层**：夜晚房间 HUD 由 `clear_night_room_hud()`（`game.rs:689-692`）清 `1600..1621` 与 `108..120`，再由事件重画 `1600` 底图和 `1601+` 文本。HUD 底图是普通 raster sprite（number 1600+），HUD 文字走 text pass。**HUD 文字因此天然高于消息框 sprite（19500）**，这是 UI-13 的另一个具体偏差。

---

## 1. UI-05 标题点“开始咯”进入游戏时黄绿背景闪烁（已复现，P1）

### 证据

逐帧 capture（cwd 仓库根）：

```bash
./target/debug/wolf-cli.exe --root release/data capture --boot --frames 913 \
  --key-frame 850:10 --press-frame 900 --pointer 80,425 \
  --out work/out/reviews/ui05_913.png
# 912..917 逐帧同理，只改 --frames
```

帧日志（`work/out/reviews/ui05_912.png`..`ui05_917.png` 同名 `.log`）：

| 帧 | `pictures` | 画面 |
|---|---|---|
| 912 | 5：`-100 Kagura/bg.png`、`-2 SystemGraphic/TitleGraphic.jpg`、`3/4/5 Picture/menu1_a/menu2_b/menu3_b` | 彩色标题菜单 |
| 913 | **0 live -> 27075 textured quads** | 整屏黄绿 |
| 914 | 0 live -> 27075 textured quads | 整屏黄绿 |
| 915 | 1：`4000 cg/blackscreen.jpg` alpha 0.03 | 仍黄绿 |
| 916 | 2：`-1 SystemGraphic/TitleGraphic.png` + 4000 alpha 0.07 | 黑白新游戏 loading 画出现 |
| 917 | 同上 alpha 0.10 | 黑白 loading |

PIL 像素统计（`ui05_contact_sheet.png` 目检一致）：912 黄绿像素 0（0.0%）；913 = 513792（87.1%），主色 `(150,188,32)` 156672、`(144,180,31)` 142848、`(164,206,35)` 105984；914 完全相同；915 仍 87.1%，颜色略暗；916/917 归零。**黄绿闪为 913-915 连续 3 帧**。

### 根因（file:line）

1. `crates/wolf-game/src/game.rs:78-87`：`TITLE_BACKDROP: i32 = -100`，注释直说标题脚本画 `Kagura/bg.png`（纯黑 1024x576）后丢弃它；地图 tile 是绿色；保留内部黑层可以让 fade 黑到黑。注释同时承认：“It lives in a negative slot, so switch_map removes it with the other screen layers before the first gameplay map is drawn.”
2. `crates/wolf-game/src/game.rs:1192-1227`：`switch_map()` 在 `:1205` 调 `self.screen.clear_screen_layers()`。
3. `crates/wolf-game/src/picture.rs:1160-1162`：`clear_screen_layers()` 实现是 `self.pictures.retain(|number, _| *number >= 0)`，于是 `-100`（黑底）和 `-2`（标题画）一起被删。
4. 新地图 event1 直到帧 916 才画 `-1 SystemGraphic/TitleGraphic.png`；913-915 没有任何全屏图，`tile_quads` 的 27075 个黄色地块直接暴露。

**具体造成黄绿闪的步骤：`switch_map` 的 `clear_screen_layers` 删除了 `-100` 黑底 → 913/914 只剩 tilemap（无全屏遮罩）→ 915 的 `4000 cg/blackscreen.jpg` 才 alpha 0.03，来不及盖住。** 不是 tile 数据错，也不是 30 帧 fade 没做；是标题黑底被清掉。

### 最小修改建议

- 首选：`switch_map()` 在 `clear_screen_layers()` 之后立刻 `ensure_title_backdrop()`（`game.rs:702-715`）重新加 `-100` 黑底；或把 `clear_screen_layers()` 改成 `remove_range(-99, -1)`，只清脚本负层，保留 `TITLE_BACKDROP=-100`。**不要**改成保留所有负层（会漏旧场景图）。
- 可选双保险：新地图 event1 第一次绘制前先画一帧黑屏 `150 [0,...]` 覆盖；但这会改变事件语义，首选还是保留黑底。
- 不要删地图素材；tile 的绿色是原版素材。

### 建议测试

- `crates/wolf-cli/tests/m11_visual_regression.rs` 新增 `title_start_transition_has_no_green_flash`：复用上面的命令行，抓 912..917，断言每帧黄色/黄绿像素（`(150,188,32)` 附近容差）为 0；同时断言 913/914 至少有一张全屏黑底 picture number=-100。
- 单元测试：`clear_screen_layers_preserves_title_backdrop`，构造 `-100` + `-2` + `3`，断言只删 `-2`、保留 `-100/3`。

---

## 2. UI-06 文本显示异常 / 对话框消失（部分复现，P1 待确认）

### 2a. 原始控制码当普通文字显示（已由截图证明，与 UI-08 同根）

- 用户截图：`work/out/ui_bugs/03_choice_raw_control_codes_515x169.png` 显示选项文字 `你是……\cdb[0:12:0]\cdb[0:12:1]的哥哥。` 原样显示。
- `crates/wolf-game/src/message.rs:439-475` `styled_items()`：`:447-450` 对 `TextToken::Var(_)` 调 `wolf_format::messagetext::to_source(...)` 后 `push_plain`，即把变量转义当普通 glyph。
- `crates/wolf-vm/src/exec.rs:1747-1765`（101）与 `:1770-1774`（102）只调 `expand()`；`crates/wolf-vm/src/text.rs:134-141` 的 `expand()` 只识别 `cself|self`、`v|corgv`、`s|corgs`、`sys|csys`，`\cdb`/`\udb`/`\sdb` 不在其中，`:67-69` 注释“Unrecognised escapes are copied through verbatim”，所以原样到达 `styled_items`。
- 好消息：`crates/wolf-game/src/textpic.rs:524-550` `var_text_inner()` 已有正确实现：`VarBank::VarDb | VarBank::SysDb => { let [ty,row,column]=self.db_coords(var); self.ctx.cells.cdb(ty,row,column).unwrap_or_default() }`，`UserDb=>udb`，`Normal=>vars.num`，`Str|SelfVar|CommonSelf=>vars.text`。修复应复用这套语义，而不是 `to_source`。

**建议修法**：把 `textpic` 的变量解析抽成共享 `resolve_var_text(bank, index, ctx) -> String`；`styled_items` 的 `Var` 分支调用它，未知 bank 返回空串/安全占位，绝不 `to_source`。`message_layout()` 已走 `layout_styled()`（`message.rs:599-700`），不用改 plain `layout()`。`docs/OPCODE_101.md:314` 明确 `\cdb[...]` 应展开为可变 DB 的值（全库 28 处），`docs/TEXT_SPEC.md:445` 也有 `pic 6220 \cdb[0:12:0]…哥哥` 的例子。

### 2b. 对话框提前消失（**未能用真实输入复现**，代码线索）

- 代码：`crates/wolf-game/src/game.rs:2297-2308 show_message()` 先 `messages.clear()` 再 push `Message{frames:0}`；`:2310-2314 message_dismissed()` 返回 `self.auto_advance || self.key_bits & crate::key::OK != 0`，用的是**held 位**而非 fresh press。
- `crates/wolf-vm/src/exec.rs:1153-1161` 的 `WaitState::Message` 每帧 poll `message_dismissed()`，true 就 `close_message()`；因此玩家按住 OK/Enter 穿过一次消息边界时，下一条消息可能在出现的下一帧就被清掉（尤其是长按/连点）。`auto_advance` 默认 `false`（`game.rs:520`），但 capture 默认 `auto_advance = !hold_message`（`main.rs` 附近），所以截图/回归必须加 `--hold-message`，否则消息会瞬失，这也是之前截图中“对话框消失”的一个测试侧来源。
- **未能复现**说明：没有用真人输入/长按回放单独抓到“第一条消息显示、第二条瞬失”的帧序列；以上是已核对代码线索，未当作已证实回归。建议按 P1 待确认处理。

**建议修法**：`message_dismissed()` 要求“本帧 fresh press”而不是 held；在 `show_message()` 里重置 `message_input_latch`（或增加 `key_pressed` 边沿位）；`auto_advance` 只用于测试/演示，不与玩家输入混用。
**建议测试**：在 `wolf-game` 输入测试里模拟 `OK` 持续 held 跨两条 101 消息，断言第一条至少存活 N 帧、第二条不会在同帧被清；再跑 `wolf-cli capture` 不加 `--hold-message` 时记录消息帧数。

---

## 3. UI-07 人名框与消息框重叠（已复现，P1）

### 证据

```bash
./target/debug/wolf-cli.exe --root release/data capture --map SampleMapA \
  --frames 600 --hold-message --out work/out/reviews/ui07_repro.png
```

日志（`ui07_repro.png` 同目录 log）：消息框 slot `19500` 九宫格 top `[32,396,3,3]` / `[35,396,954,3]` / `[989,396,3,3]`，left `[32,399,3,162]`，右下 `[989,561,3,3]`；message layout font20 at `[64,410]`，cursor `[576,554]`。

`work/out/reviews/ui07_nameplate_annotated.png`（822x252，crop `36,348..310,432` 放大 3x）：橙框为 `(68,378)-(133,408)` 的名牌，红线 `frame top y=396`，蓝线 `nameplate bottom y=408`；**名牌 65x30，底部 408，压进 frame 上边框 396..399，重叠 12px**。用户截图 `work/out/ui_bugs/04_dialog_cafe_nameplate_1020x578.png`、`05_nameplate_overlap_crop_394x203.png` 同症状；`ui06_msg_600f.png` 也能直接看到“妹妹”压住左上边框。

### 根因（file:line）

`crates/wolf-game/src/game.rs:1446-1457 message_nameplate()`：

```rust
let font = vars.font_px;
let text_w = name.chars().count() * font;
let box_w = (text_w + font * 1.25).max(font * 2.5);
let box_h = (font * 1.5).max(24.0);
let x = vars.x + 4.0;
let y = vars.y - box_h - 2.0;   // <-- 用正文原点 y，不是 frame top
```

`vars.y = 410`、`box_h = 30`，所以 `y = 378`、`bottom = 408`；消息框 `frame top = 396`，重叠 12px。`message_vars()`（`game.rs:1372-1403`）只从 `FRAME_PICTURE` 的 window 取 `rect[0]+rect[2]-sys(1)` 作为 wrap width，**没有 frame_top 字段**，所以 `message_nameplate` 根本不知道 frame 上沿。

### 最小修改建议

1. `message_vars()` 增加 `frame_top: Option<f32>`（从 `FRAME_PICTURE=19500` 的 window rect `[1]` 取；无 frame 时为 None）。
2. `message_nameplate()` 增加 `frame_top` 参数：`y = frame_top.unwrap_or(vars.y) - box_h - gap`，`gap` 建议 2~4px；`box_h`、`box_w` 公式保持。
3. 当前名牌 panel 在 `play.rs:334-348` append 到最后 sprite，层级上是对的（在消息框上面）；只需要修 y 几何，不要把 panel 移到 frame 下面。

### 建议测试

- `crates/wolf-game/tests/m7_choice_window.rs` 或新 `message_layout` 测试：构造 `FRAME_PICTURE` window top=396、`vars.y=410`、font=20，断言 `nameplate.bottom <= 396 - 2`。
- `wolf-cli` 截图回归：`SampleMapA --frames 600 --hold-message` 固定断言 `ui07_repro.png` 中名牌 bbox 与 frame top 的几何关系（可直接用 capture 日志的 nameplate/frame 数值，不必做像素 OCR）。

---

## 4. UI-08 选项框文字截断 / 部分角色说话时人名框消失（部分复现，P1）

### 4a. 已确认：`\cdb` 既计宽又原样显示

- 用户截图：`work/out/ui_bugs/03_choice_raw_control_codes_515x169.png`（原始控制码）、`06_choice_text_truncated_crop_535x204.png`（长选项右侧截断）。
- `crates/wolf-game/src/choice.rs:113-118 styled_width()` 对 `message::inline_runs` 每个 `run.rect[2]` 求和；`:125-134 size()` 宽 = `widest + font_px*1.5(marker) + PADDING*2 + WIDTH_SLACK`；`:223-278 draw()` 里 `:268` 每个 `TextQuad` 的 `rect[2]=run.rect[2]+run.font_px`，`:274 x += run.rect[2]`。
- `message::inline_runs()`（`message.rs:487-506`）复用 `styled_items`，`:493-496` 只取 `Item::Glyph`，`Item::Break`/`Align` 全部 `continue` 丢弃；`message.rs:486` 注释“A hard break is dropped: an option is one row”。因此含换行的选项被压成一行；`\cdb` 被 `styled_items` 当 glyph，既计入 `run.rect[2]` 又原样画出来。
- 已确认的“长选项截断”高概率机制：`styled_width` 只累计 `run.rect[2]`，但 `draw` 给每个 `TextQuad` 的宽度额外加 `run.font_px`（`choice.rs:268`），面板宽度没有为最后一个 glyph 的 overhang 留量；长 CJK 串会从右侧溢出/被裁。**尚未用原始事件存档逐帧复现**，目前依据用户截图 + 代码路径，标“待定根因”。

### 4b. 建议修法

1. 在 `message::styled_items` 的 `Var` 分支复用 `textpic` 的 `VarBank` 解析（见 UI-06 2a），不要在 choice 里显示 `\cdb`。
2. `inline_runs` 若选项允许换行，必须保留 `Break` 并让 `choice::size` 按行数算高；若产品约定“选项永远一行”，则在 `styled_width`/`draw` 前把 `Break` 替换为空格并明确记录，不能静默丢掉。
3. `choice::size` 的宽度改为 `sum(run.rect[2]) + last_font_overhang` 或直接使用 `message` shaper 返回的实际 advance 宽度；`draw` 的 x 步进与 quad 宽度用同一个值，避免 `size` 与 `draw` 两套宽度。
4. 保留 `WIDTH_SLACK`，但增加一次“左侧 marker + 右侧 2px”的边界检查测试。

### 4c. 建议测试（`crates/wolf-game/tests/m7_choice_window.rs`）

- `styled_choice_does_not_render_raw_cdb`：选项含 `\cdb[0:12:0]`，断言 draw 出的文本不含反斜杠、宽度等于解析后文本宽度。
- `long_cjk_choice_stays_inside_panel`：用截图 06 的原文/等价长句，断言所有 `TextQuad` 右边界 <= panel 右边界 - 1。
- `break_in_choice_is_not_silently_dropped`：含 `\n` 的选项，断言要么两行、要么显式空格，不能少字。
- 现有 `m7_choice_window.rs:78-153` 只测 6 个短选项（`choice::layout_at_sized`/`overlay_at_sized`），未覆盖 styled/长 CJK/Break/`\cdb`；新增用例即可。

### 4d. “部分角色说话时人名框消失”

- **未复现**。当前代码看，`message_nameplate()` 只要 `message_speaker` 非空就会生成；如果 speaker 为空/`None` 就不画（设计如此）。可能的真实原因：`show_message` 的 speaker 参数在某些 101 调用中被 `split_face` 吃掉，或消息提前关闭（UI-06 2b）导致整块消失，而不是单独的人名框消失。建议等 UI-06 输入侧修好后复测；报告标“待定，需具体角色/事件存档”。

---

## 5. UI-12 19:30 房间“方针/公会评价”面板与 HUD 尺寸/图层异常（不是当前 Rust 回归，P2/建议降级）

### 证据

- Rust：`work/out/reviews/save7.png`（由 `work/retail_backup/Save/SaveData07.sav` 载入，`save7.log` 记录 `loaded 62 picture(s)`、`frames 30`、`entry save file ...SaveData07.sav`）
- 零售：`work/retail_backup/Save/save7.jpg`
- 对照图：`work/out/reviews/ui12_compare.png`（600x376，上=Rust，下=零售）
- 同尺寸 1024x576，PIL 全屏 diff：mean=4.3988/255，max=255，`max channel > 8` 占 4.91%（主要是 JPEG/抗锯齿），面板区域布局完全同构：方针/烦恼全解、公会评价/中坚公会/状态/精神/无性、信赖/566/心情/很开心、性趣/213/妹妹性欲/59、攻击247/技巧247/防御233/毅力233/智慧193/技能点数62 的文字挤叠位置一致。

### 结论

`Picture/policy.png` 实际是 530x84 的一整张已画好标签与空白数值框的底图；运行时 `pic 1600 File at (40,50)` 原尺寸绘制，动态数字用 `1601+` 文本图覆盖，**很多动态文本正好落在静态标签上**，这是原版素材/脚本自己的排版。零售 `save7.jpg` 与 Rust `save7.png` 像素级同构，故 **UI-12 在当前工作树不是 Rust 重写回归**。除非用户另指一个不同于 save7 的场景/分辨率，否则建议把 UI-12 从“待修 bug”降级为“原版布局/产品改进”，不要在 task-6 里改面板坐标。

### 建议

- 若产品确实要优化，另开“UI 布局改进”任务，统一重排 `Picture/policy.png` 与 1601+ 文本坐标；不要以“回归修复”名义改。
- 回归测试：把 `save7` 场景加入 `m11_visual_regression` 作为**已知零售基线**（允许 JPEG 容差），防止未来误改。

---

## 6. UI-13 其他图层优先级 bug（代码级确认，P2）

### 当前实际层级 vs 目标

目标（`docs/UI_BUG_BACKLOG.md:69`）：**选择弹窗 > 消息框 > 场景 UI > 地图/背景**。

| 目标 | 当前实际 | 代码 | 偏差 |
|---|---|---|---|
| 地图/背景最底 | `tile_quads` 先画 | `game.rs:1096-1113` | 正确 |
| 场景 UI 在 map 上 | `picture_quads` BTreeMap 升序 | `picture.rs:1427-1454` | 基本正确 |
| 消息框在场景 UI 上 | frame 19500 是普通 sprite；HUD 1600+ 也是 sprite，19500 后画 | `atlas.rs:608` + `message.rs:60` | 消息框 sprite 高于 HUD sprite，正确 |
| 消息文字在消息框上 | 文字统一在 text pass | `renderer.rs:659-667` | 正确（但 HUD text 也在这 pass） |
| HUD 文字在场景 UI 上、在消息框下 | HUD 1601+ 文本与 message text 同 pass；`build_text_items` 先 text_quads 后 message | `play.rs:368-419` | **HUD 文字会盖住消息框 19500**（若重叠） |
| 选择弹窗在消息框上 | choice panel append 到 sprites 末尾（`play.rs:296-299`），但 nameplate panel 更晚 append（`:334-348`）；choice 行文字在 message text 之前 | `play.rs:288-348`、`368-419` | **nameplate panel 压 choice panel；message text 压 choice 行文字** |
| 名牌属于消息框 | nameplate panel 最后 append、文字最后画 | `play.rs:334-348` | 相对消息框正确，但相对 choice 过高 |

### 具体可见/潜在症状

1. 选择界面与消息框同屏时：`build_text_items` 顺序是 `text_quads` → `choice rows` → `message runs` → `nameplate text`；消息正文可能盖住选项文字，名牌面板可能盖住选项面板。
2. HUD + 消息同屏：HUD 的 `1601+` 文本（text pass）会压过消息 frame `19500`（sprite pass），因为 text pass 无条件最后。
3. `choice_overlay` 的 panel 与 `nameplate` 都用 `PANEL_PICTURE=-100`，但**实际顺序只由 append 决定**，number 完全无效；任何新增宿主 UI 只要 append 在 nameplate 之后就会压住它。
4. `TITLE_BACKDROP=-100` 与 `PANEL_PICTURE=-100` 冲突（NEW-1），一旦未来把宿主 panel 放回 Screen 排序就会和标题黑底互相覆盖。

### 最小修改建议（架构，不要求一次全改）

1. 引入显式 `LayerBand`（u32）：`Map=0`、`ScenePicture=10`、`SceneText=11`、`MessageFrame=20`、`MessageText=21`、`Nameplate=22`、`ChoicePanel=30`、`ChoiceText=31`、`Cursor=40`。每个 `SpriteInstance`/`TextItem` 带 `(band, picture_number, seq)`，统一 `sort_by_key`；渲染器仍可 sprite/text 分两次提交，但**排序必须在合并后的显示列表上算**，不能两个 pass 各排各的。
2. 或者最小补丁：在 `play.rs`/`main.rs` 里把 choice panel+rows 移到消息/名牌之后绘制；把 HUD text 归入 `text_quads` 的 scene band；nameplate 面板/文字紧跟 message 之后、choice 之前。
3. 保留 `PANEL_PICTURE` 作为“无番号宿主层”的哨兵，但必须与 `TITLE_BACKDROP` 分开（见 NEW-1）。

### 建议测试

- 合成场景：绿色地图 + HUD 1600/1601 + 消息 frame 19500 + 名牌 + choice 三行，抓一张 1024x576；在重叠点断言 choice 像素 > message 像素 > HUD 文字像素 > 地图。
- 现有 `m11_visual_regression` 增加 `--force-choice`/`--force-message` 的 fixture；或新 `m12_layer_priority.rs` 用 `wolf-game` 的 `picture_quads/text_quads` 直接断 quad 顺序（不依赖 GPU 像素）。

---

## 7. 新发现 bug（尚未登记）

> 说明：本次没有发现超出已登记 UI-05/06/07/08/12/13 的**新用户可见重回归**；以下 2 条是代码级风险，均 P2，建议登记为“架构/工具链”类而非用户 bug。

### NEW-1（P2）`TITLE_BACKDROP` 与 `PANEL_PICTURE` 共用 -100，语义冲突

- 位置：`crates/wolf-game/src/game.rs:78-87`（`TITLE_BACKDROP: i32 = -100`）与 `crates/wolf-game/src/window.rs:39-44`（`PANEL_PICTURE: i32 = -100`，注释“scripts use >= -2, so a large negative sentinel sorts it behind every script layer”）。
- 影响：当前宿主 choice/nameplate 是 append 到 sprite 列表，不经过 `Screen` 排序，所以尚未见可见症状；但任何把宿主 panel 放回 `Screen` 的改动、或任何脚本/引擎图使用 `-100`，都会让标题黑底和 UI 面板同号，排序退化为插入顺序。它也是 UI-05 修复时最容易踩的坑。
- 最小修：`PANEL_PICTURE` 改为不与 `TITLE_BACKDROP` 冲突的哨兵（如 `-1000`，`CURSOR_SLOT` 已是 `-100000`）；或者为 `TITLE_BACKDROP` 单独保留 `const` 并在 `clear_screen_layers` 里按常量保护。
- 测试：`panel_picture_number_does_not_collide_with_title_backdrop` 编译期/单元断言。

### NEW-2（P2）capture 的 101 光标用 plain layout，文字用 styled layout，位置可能不一致

- 位置：`crates/wolf-cli/src/main.rs:2205`（capture cursor 仍走 `message::layout`，plain）与 `main.rs:2324-2376`（文字走 `message_layout`，styled）；`play.rs:305` 附近的 play 光标路径也需对齐。
- 影响：带 `\c`/`\f`/内联 markup 的消息，capture 截图里光标位置与实际文字排版不一致；play 窗口若同样混用也会偏。属于工具/截图可信度问题，不影响默认纯文本。
- 最小修：capture 的 cursor 位置改用 `message_layout()` 的 `cursor`/`cursor_offset`，与文字同一 layout 结果；或把 `message_layout` 的结果缓存给 cursor 复用。
- 测试：含内联颜色的消息，capture 后比较 cursor bbox 与最后 glyph 的右边界。

---

## 8. 未复现 / 待定清单

| 项 | 状态 | 说明 |
|---|---|---|
| UI-06 对话框提前消失 | **未复现（代码线索 P1）** | `message_dismissed` 用 held OK 位；未用真实长按回放抓到帧序列。不要当已证实回归。 |
| UI-06 消息文本原始控制码 | **同根确认，消息路径未单独截图** | `message.rs:439-475` 与 choice 共用 `styled_items`；用户截图 03 是 choice。 |
| UI-08 长 CJK 截断精确根因 | **部分确认（P1）** | 用户截图 06 + `choice.rs` 宽度路径；尚未找到原始事件存档逐帧回放，根因标“高概率：`styled_width` 未计最后 glyph overhang”。 |
| UI-08 “部分角色说话时人名框消失” | **未复现** | 需具体角色/事件存档；先修 UI-06 消息生命周期再复测。 |
| UI-12 面板异常 | **确认非 Rust 回归** | 与零售 `save7.jpg` 同构；建议降级/关闭。 |
| UI-13 实际用户画面 | **代码级确认，未构造重叠场景** | 需要 choice+message+HUD 同屏 fixture 才能出可见截图。 |

---

## 9. 建议测试汇总

| 测试 | 文件 | 断言 |
|---|---|---|
| `title_start_transition_has_no_green_flash` | `crates/wolf-cli/tests/m11_visual_regression.rs` | 912..917 无黄绿像素；913/914 有 `-100` 黑底 |
| `clear_screen_layers_preserves_title_backdrop` | `crates/wolf-game/src/picture.rs` 单测或 `tests/m12_layer.rs` | `-100` 保留、`-2` 删除、正数保留 |
| `nameplate_sits_above_message_frame` | `crates/wolf-game/tests/m7_choice_window.rs` | `nameplate.bottom <= frame_top - 2` |
| `styled_choice_does_not_render_raw_cdb` | `crates/wolf-game/tests/m7_choice_window.rs` | 输出不含 `\cdb`，宽度按解析文本 |
| `long_cjk_choice_stays_inside_panel` | 同上 | 所有 TextQuad 右边界 <= panel 右边界-1 |
| `break_in_choice_is_not_silently_dropped` | 同上 | 换行不被吞字 |
| `choice_above_message_above_hud` | 新 `crates/wolf-game/tests/m12_layer_priority.rs` | quad band 顺序符合 UI-13 |
| `message_held_ok_does_not_dismiss_next` | `crates/wolf-game/tests/m7_choice_window.rs` 或输入测试 | held OK 不跨消息边界清下一条 |

运行方式（不要跑全量 workspace）：

```bash
TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-game <testname>
# 视觉回归
TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-cli --test m11_visual_regression
```

---

## 10. 证据文件索引

- `work/out/reviews/ui05_912.png` .. `ui05_917.png` 及 `.log`：UI-05 逐帧
- `work/out/reviews/ui05_contact_sheet.png`：UI-05 3x2 汇总
- `work/out/reviews/ui07_repro.png`：UI-07 消息框/名牌
- `work/out/reviews/ui07_nameplate_annotated.png`：UI-07 几何标注（822x252）
- `work/out/reviews/ui06_msg_600f.png`：SampleMapA 消息框名牌
- `work/out/reviews/ui12_compare.png`：Rust/零售 save7 对照
- `work/out/reviews/save7.png`、`save7.log`：19:30 房间 Rust capture
- `work/out/ui_bugs/03_choice_raw_control_codes_515x169.png`：UI-06/08 原始控制码
- `work/out/ui_bugs/04_dialog_cafe_nameplate_1020x578.png`、`05_nameplate_overlap_crop_394x203.png`：UI-07
- `work/out/ui_bugs/06_choice_text_truncated_crop_535x204.png`：UI-08
- `work/out/ui_bugs/07_room_1930_policy_panels_1027x580.png`：UI-12 用户原图
- `work/out/ui_bugs/contact_sheet.png`：用户截图汇总

## 11. 审查结论（一句话）

UI-05/07 是**已实测的布局/清屏 bug**；UI-08 的 `\cdb` 与 Break 处理是**已确认的文本管线 bug**；UI-06 的“消息消失”是**待确认的输入边沿 bug**；UI-12 是**原版布局、不是 Rust 回归**；UI-13 是**宿主 UI 没有 layer band** 的架构问题，也是 UI-05/07/08 反复出现的共同土壤。
