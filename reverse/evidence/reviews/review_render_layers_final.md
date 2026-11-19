# Review: F4 最终渲染层集成（task-17）

- 复核日期：2026-11-04
- 源码快照：当前工作树（HEAD 8c65153，2026-11-03 13:15:04 +0800）
- 复核对象：task-14 的 renderer.rs 多 SceneLayer/每层 submit、play.rs/main.rs build_scene_layers 与 LayerBand、window.rs LayerBand、wolf-render/tests/offscreen.rs 多 text layer 测试
- 复核方式：只读源码 + 实际 cargo test + 真实 wolf-cli capture 像素检查（未修改源码、测试、release、manifest、存档）
- 报告路径：work/out/reviews/review_render_layers_final.md

## 0. 结论摘要

**核心实现 PASS**：每层独立 queue.submit 确实修复了 glyphon 共享 vertex_buffer 串位；LayerBand 已从 engine 分类贯通到 host 合成，层间顺序为 Map(0) < ScenePicture(10) < SceneText(11) < MessageFrame(20) < MessageText(21) < Nameplate(22) < ChoicePanel(30) < ChoiceText(31) < Cursor(40)。层内始终先 sprite 后 text。

**不完全 PASS 的边界**：
1. m11_visual_regression 当前仍 3 passed / 2 failed；失败数值与 task-13（task-14 之前）完全相同，属于既有集成/文本问题，不是 task-14 引入；但 Lead 做最终集成前必须处理或重新定基线。
2. m12_ui_layers 13 passed，但其 5 个顶层用例只断言排序/band/真实存档 display list，**没有 host 合成像素测试**；offscreen 测试覆盖了 renderer 的多层语义，但没有经过 play::build_scene_layers 的真实 UI。
3. 发现 3 个 P3 级潜在陷阱（text_error 不清理、picture_band(PANEL_PICTURE) 归 Map、LayeredText::script 恒 SceneText）。

## 1. 检查范围与调用路径

### 1.1 改动范围（工作树）

task-14 直接相关文件：

- crates/wolf-render/src/renderer.rs
- crates/wolf-render/src/lib.rs
- crates/wolf-render/src/text.rs
- crates/wolf-render/tests/offscreen.rs
- crates/wolf-cli/src/play.rs
- crates/wolf-cli/src/main.rs
- crates/wolf-cli/tests/m12_ui_layers.rs（新增）
- crates/wolf-cli/tests/m11_visual_regression.rs

git diff --stat（仅上述文件）：7 files changed, 1169 insertions(+), 360 deletions(-)。工作树同时有 task-5 的 wolf-game 改动与 build.rs/wolf-layout 改动；本报告只对 task-14 的渲染层路径下结论，不把其他文件的行为归因给 task-14。

### 1.2 所有实际绘制入口都走新路径

- 交互窗口启动首帧：crates/wolf-cli/src/play.rs:728-733 调 draw_scene_layers。
- 交互窗口 RedrawRequested：crates/wolf-cli/src/play.rs:829-835 调 draw_scene_layers。
- App::rebuild()：crates/wolf-cli/src/play.rs:618-625 调 build_scene_layers。
- 离屏 capture：crates/wolf-cli/src/main.rs:2307-2312 排序副本后调 play::draw_scene_layers；CaptureFrame 在 main.rs:2181-2240 由 build_scene_layers 生成。
- 旧的 renderer.draw() + renderer.draw_text() 仍保留在 play.rs:472-474 与 553-560（示例/兼容路径），但 App 的渲染入口已不再调用它。

结论：没有发现“某条 live/capture 路径绕过 LayerBand”的遗漏。

## 2. glyphon 共享 vertex_buffer 串位是否被修复

### 2.1 问题事实

glyphon 0.12.0 的 TextRenderer::prepare 把当前这次 prepare 的所有 glyph 写进 self.vertex_buffer（同一个 buffer），render 使用该 buffer 绘制。也就是说：

- 若在同一 encoder 里连续 prepare layer0、layer1，然后只 submit 一次，layer0 的 pass 会读到 layer1 最后一次写入的顶点，layer0 的 A 变成 layer1 的 B 位置/字形。
- 这是“共享 buffer + 延迟消费”的经典串位，不是合成顺序问题。

本仓库的 crates/wolf-render/src/text.rs:626-806 正是这个结构：每次 prepare 都 self.buffers.clear()/self.placed.clear()，随后 self.glyphs.prepare(...) 写共享 glyph buffer；render（text.rs:808-819）只消费当前 prepared 结果。

### 2.2 修复实现

crates/wolf-render/src/renderer.rs:969-973 的注释明确写出修复策略：

> One submit per logical layer. TextRenderer::prepare() rewrites one shared glyph vertex buffer through Queue::write_buffer, so preparing several layers into a single encoder would make every layer pass read the last layer's glyph vertices. Submitting each layer immediately keeps each prepare ordered before the pass that consumes it.

对应代码：

- renderer.rs:974-989：每层先 text.prepare(...)，失败只记录 text_error 并继续画 sprite。
- renderer.rs:992-998：第一层 LoadOp::Clear(BLACK)，后续 LoadOp::Load。
- renderer.rs:1004-1037：在同一个 render pass 内先 pass.draw(0..4, layer.sprite_start..start+len)，再 text.render(&mut pass)。
- renderer.rs:1038：每层 self.queue.submit(Some(encoder.finish()))，然后才进入下一层 prepare。
- renderer.rs:1041-1071：最后 blit pass 单独 submit + present。

因为 queue.submit 与 queue.write_buffer 在 wgpu 队列中按调用顺序执行，layer N 的 consume pass 一定排在 layer N+1 的 prepare 写 buffer 之前，串位根因被切断。

### 2.3 针对性回归测试

crates/wolf-render/tests/offscreen.rs:435-495：

- 测试名：multiple_text_layers_keep_their_own_glyphs
- 构造 layer0 红色 A 在 (40,40)，layer1 蓝色 B 在 (700,380)
- 断言：A 区域有红像素、B 区域有蓝像素；A 区域无蓝像素、B 区域无红像素（blue_in_a == 0、red_in_b == 0）
- 若两个 prepare 被合并到一个 submit，A 区域会出现蓝色 B，测试会失败；这是对本次修复的直接回归测试。

实测：

- TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-render --test offscreen → 10 passed, 0 failed（task17_offscreen_full.log）
- ... --test offscreen layer -- --nocapture → 3 passed；日志包含上述测试名（task17_offscreen.log）

另两条互补测试（offscreen.rs:330-414）：

- later_sprite_layer_covers_earlier_text_layer：后层 sprite 能盖住前层 text。
- later_text_layer_covers_earlier_sprite_layer：后层 text 能盖住前层 sprite，且前层红色背景仍在。

结论：**glyphon 串位修复是真实有效的，PASS**。

## 3. LayerBand 与绘制顺序

### 3.1 顺序表

crates/wolf-game/src/window.rs:49-90：

| rank | LayerBand | 用途 | 典型对象 |
|---:|---|---|---|
| 0 | Map | 地图/负番号全屏层 | tile、negative picture |
| 10 | ScenePicture | 普通场景图片 | 正番号 picture |
| 11 | SceneText | 普通文本图 | 负番号 text 也会落这里（见 P3） |
| 20 | MessageFrame | 19500 消息窗口/九宫格 | FRAME_PICTURE |
| 21 | MessageText | 消息正文 | message layout runs |
| 22 | Nameplate | 人名框 | 名牌 panel + name text |
| 30 | ChoicePanel | 选项面板 | PANEL_PICTURE |
| 31 | ChoiceText | 选项文字 | choice rows |
| 40 | Cursor | 101 光标 | CURSOR_SLOT |

rank() 在 window.rs:85-90。picture_band 在 window.rs:93-110：

- number < 0 → Map
- 19500..=19503 → MessageFrame
- 其他 → ScenePicture

### 3.2 层内与层间顺序

SceneLayer 定义在 renderer.rs:87-100：文档明确 “Within a layer the sprites are painted before the text… a later layer's sprites cover an earlier layer's text and a later layer's text covers an earlier layer's sprites.”

draw_layers（renderer.rs:867-895）：

- 所有层的 sprite 连续写入同一个 instance buffer，每层记录 (sprite_start, sprite_len)；
- 每层的 texts 被 clone 进 RenderLayer（renderer.rs:102-111），保证超出调用生命周期；
- present 按 layers 顺序逐层 prepare + pass + submit。

scene_bands（play.rs:130-152）按 rank 去重排序；空列表返回 [Map]；draw_scene_layers（play.rs:154-191）按同一 bands 生成 Vec<SceneLayer>，因此 **band 顺序 = pass 顺序 = 像素覆盖顺序**。

### 3.3 host 合成顺序（build_scene_layers）

crates/wolf-cli/src/play.rs:201-340：

1. tile_quads → LayeredSprite::script（Map）
2. picture_quads → LayeredSprite::script（按 picture_band）
3. text_quads → LayeredText::script（恒 SceneText）
4. choice panel/rows → LayeredSprite::host(ChoicePanel) / LayeredText::host(ChoiceText)
5. 101 cursor → LayeredSprite::host(Cursor)
6. nameplate 九宫格 → LayeredSprite::host(Nameplate)
7. message layout runs → LayeredText::host(MessageText)
8. nameplate text → LayeredText::host(Nameplate)
9. sort_layered_sprites / sort_layered_texts（play.rs:100-108，键 (band.rank(), number, seq)）

关键点：choice 面板和 nameplate 都使用 PANEL_PICTURE=-1000 作为 trace number，但因为 host 显式指定 band，排序键中的 band 先决定顺序；同一 band 内才用 number/seq 决胜。play.rs:194-200 的文档也明确说明这一点。

### 3.4 实际 UI 顺序（代码推断 + 测试）

- choice panel(30) > nameplate(22)：m12_ui_layers.rs:75-107 choice_panel_sorts_above_nameplate_and_every_scene_layer 断言通过。
- choice text(31) > message text(21) > scene text(11)：m12_ui_layers.rs:109-136 通过。
- cursor(40) 在最后：m12_ui_layers.rs:75-107 的 bands 序列最后是 Cursor。
- message frame(20) 可盖住 scene text(11)：offscreen later_sprite_layer_covers_earlier_text_layer 的合成语义 + build_scene_layers 的 band 分类共同保证。
- nameplate text(22) 低于 choice text(31)：m12_ui_layers.rs:109-136 通过。

### 3.5 空层/全空帧/legacy draw 兼容性

- draw_layers(&[]) 全空帧：renderer.rs:867-895 只清空 layers、置 layers_active=true；present 在 renderer.rs:948-955 发现 layers 为空时 push 一个 RenderLayer::default()，第一层仍执行 Clear(BLACK)，不会保留上一帧。**代码审查 PASS，但当前 offscreen 没有专门的 draw_layers(&[]) 用例**（现有 offscreen.rs:191-211 只测 draw(&[])）。
- draw(&[]) legacy 路径：renderer.rs:840-852 清 layers、layers_active=false、instance_count=0；offscreen.rs:191-211 empty_batch_does_not_replay_the_previous_frame 已覆盖“空 batch 必须清黑、不得重放上一帧”。
- legacy draw + draw_text 兼容：renderer.rs:840-852 与 draw_text:702-707 仍保留；present 在 layers_active=false 时用单层 {sprite_start:0, sprite_len:instance_count, texts:text_items.clone()}（renderer.rs:956-962），语义与旧版一致。当前 play.rs 的 App 路径不调用它，只有示例/兼容用途；没有发现 App 误用。
- 风险：draw_layers 传空 slice 与传“只含空 SceneLayer 的 slice”都走同一个空 layers fallback；建议补一条 offscreen 像素测试，断言先画红图再 draw_layers(&[]) 后为全黑（当前无直接测试）。

### 3.6 性能/资源创建

- 无明显泄漏：instance buffer 只在 reserve_instances（renderer.rs:897-915）按需扩容，不重复创建；文本 font atlas 有周期性 trim（text.rs:827 起）；RenderLayer.texts 的 clone 在 present 结束后随 layers 一起 drop。
- 每帧有可优化的分配：present 在 renderer.rs:949 用 std::mem::take(&mut self.layers)，把 Vec 容量留成 0；下一帧 draw_layers 重新分配 Vec 并 push。层数最多 9，开销很小，但可改为 swap/clear 保留容量。
- draw_layers 每层 texts.to_vec()（renderer.rs:890）会 clone 每层 TextItem 字符串；这是为了跨越 draw_layers 调用生命周期，正确但每帧有分配；当前文本量下实测无卡顿。
- 每层一个 queue.submit（最多 9 个）+ blit 一个；每层 text.prepare 重新 shape 该层文本。当前 1024x576 下可接受，属于 P2 观察项。
- 结论：没有资源泄漏；有每帧小分配，不是 task-14 的正确性问题。

## 4. 真实 capture 与像素证据

所有命令均在仓库根目录执行；截图/日志在 work/out/reviews/。

### 4.1 消息框 + 名牌（UI-07 相关）

命令：

    ./target/debug/wolf-cli.exe --root release/data capture --map SampleMapA --frames 600 --hold-message --out work/out/reviews/task17_msg_600.png

日志关键值：

- quad pic19500 band MessageFrame
- quad pic -100000 band Cursor
- text pic19500 band MessageText 六行
- message: 7 rows / 73 chars / font20 at [64,410] / wrap928 / cursor [576,554]
- text: 6 items / 67 glyphs / 6 lines
- non-black: 585690 / 589824 = 99.3%

目检 task17_msg_600.png：完整消息框、正文、名牌，无串层、无黄绿。

当前 message_nameplate 几何（crates/wolf-game/src/game.rs:1710-1733）已改为有 frame 时 (rect[0]+4, rect[1]-box_h-8)，不再使用正文原点 vars.y - box_h - 2；此前 task-1 报告的 12px 压边框在当前快照已不成立（该修复属于并行任务，不是 task-14 的 renderer 改动，但集成后没有回归）。

### 4.2 选项面板（UI-13 / host 合成）

命令：

    ./target/debug/wolf-cli.exe --root release/data capture --event 247 --frames 300 --out work/out/reviews/task17_choice247.png

日志关键值：

- choice HELD 4 options: ["聊天","0","小鸟飞飞！","讨好"], cursor0 at (512,192)
- 9 个 ChoicePanel quad，band ChoicePanel
- 5 个 ChoiceText quad，band ChoiceText

目检 task17_choice247.png：四行选项、三角光标、面板边框都正常；选项中的 0 是数据内容，不是排版/图层问题。

### 4.3 UI-05 黄绿闪没有回归

命令（逐帧 912..917）：

    ./target/debug/wolf-cli.exe --root release/data capture --boot --frames N --key-frame 850:10 --press-frame 900 --pointer 80,425 --out work/out/reviews/task17_ui05_N.png

日志：

- 912：pictures 5 / 19085 quads（-100 Kagura/bg.png、-2 TitleGraphic.jpg、3/4/5 menu）
- 913-914：pictures 2 / 27077 quads（-100,-2）
- 915：pictures 3 / 27078（+4000 blackscreen alpha 0.03）
- 916：pictures 4 / 27079（+-1 TitleGraphic.png、4000 alpha 0.07）
- 917：pictures 2 / 27077（-1、4000）

PIL 黄绿像素判据 g>120 && r>110 && b<80：912-917 全部 0.00%。task-14 未回归 UI-05。

## 5. 测试覆盖评估

### 5.1 实际执行

- cargo test -q -p wolf-render --test offscreen：10 passed, 0 failed（task17_offscreen_full.log）
- cargo test -q -p wolf-render --test offscreen layer -- --nocapture：3 passed，含 multiple_text_layers_keep_their_own_glyphs、later_sprite_layer_covers_earlier_text_layer、later_text_layer_covers_earlier_sprite_layer（task17_offscreen.log）
- cargo test -q -p wolf-cli --test m12_ui_layers：13 passed, 0 failed（task17_m12_ui_layers.log）
- cargo test -q -p wolf-game --test m12_render_layer_priority：6 passed（task17_m12_priority.log）
- cargo test -q -p wolf-game --test m12_render_ui12_policy：2 passed（task17_m12_policy.log）
- cargo test -q -p wolf-cli --test m11_visual_regression：3 passed, 2 failed（task17_m11.log）

### 5.2 m12_ui_layers 的真实覆盖

该测试用 #[path = "../src/play.rs"] mod play;（m12_ui_layers.rs:32）编译真实 build_scene_layers/排序函数，这一点很好。13 个 test 中：

- 5 个本文件用例：
  - choice_panel_sorts_above_nameplate_and_every_scene_layer（75-107）
  - message_text_sorts_below_choice_text_but_above_scene_text（109-136）
  - scene_bands_merge_each_present_band_once_in_rank_order（138-168）
  - scene_pictures_in_the_same_band_keep_picture_number_order（170-179）
  - real_save_display_list_is_sorted_by_band_then_picture_number（202-237）
- 其余 8 个来自 play.rs 内部的输入测试（由 #[path] 一并编译）。

**没有**任何用例用 Renderer 把 build_scene_layers 的完整结果画到 offscreen framebuffer 并做像素断言；real_save_display_list 只检查 (band.rank(), number) 有序，不检查 band 值是否对应正确语义（例如把所有东西都改成 ScenePicture，只要有序仍可能过一部分断言）。这是 P2 级测试缺口。

### 5.3 m11_visual_regression 失败说明

task17_m11.log：

- h_overlay_frame_matches_its_baseline: left=527554, right=527713，差 -159 non-black pixels
- story_map_idle_frame_matches_its_baseline: left=544431, right=544394，差 +37
- panic 位置：crates/wolf-cli/tests/m11_visual_regression.rs:382

task-13 在 task-14 之前记录过完全相同数值，因此这不是 task-14 的 renderer 层改动引入的；但当前共享工作树同时有 task-5 的文本/字体改动，未做隔离实验，不能归因给 task-5，也不能宣称 m11 通过。建议 Lead 在 task-7 集成时先决定是修字体/文本基线还是更新基线，不要直接跳过。

## 6. 发现与风险

### P3-1 text_error 只在有文本时清空

- 位置：crates/wolf-render/src/renderer.rs:963-966（has_any_text 时才 self.text_error=None）
- 问题：某一帧某层 text.prepare() 失败后 text_error 被设置；下一帧若整个画面没有文本层，has_any_text=false，错误不会被清除，诊断会一直显示旧错误。
- 影响：低（不影响绘制，只影响 Renderer::text_error() 诊断/测试）。
- 最小修改：present 开头无条件 self.text_error=None，或在 for 循环结束后按本次是否发生错误重新赋值。
- 建议测试：在 offscreen 里构造一次 set_font 失败后清空字体/无文本帧，断言 text_error().is_none()。

### P3-2 picture_band(PANEL_PICTURE) 落回 Map

- 位置：crates/wolf-game/src/window.rs:93-110 与 :39-47
- PANEL_PICTURE = -1000，picture_band 对 number < 0 一律返回 LayerBand::Map。
- 当前 host 路径都使用 LayeredSprite::host(LayerBand::ChoicePanel/Nameplate)，不会触发；但任何未来代码若把 host panel 交给 LayeredSprite::script，它会掉到 Map(0)，被所有场景层盖住。
- 影响：潜在陷阱，当前无生产 bug。
- 建议：为 host-only sentinel 增加显式检查/debug_assert，或在 picture_band 注释/测试里固定“PANEL_PICTURE 不允许走 script 分类”。

### P3-3 LayeredText::script 把所有 text picture 归 SceneText

- 位置：crates/wolf-cli/src/play.rs:80-88
- LayeredText::script 不读取 quad.number，恒返回 LayerBand::SceneText；若负番号 text picture 本应属于 Map/全屏层，会被画到 SceneText(11)，盖住 ScenePicture(10)。
- 当前 game.text_quads() 的文本图多为正番号 UI 文本，风险未实际触发；但这是与 picture_band 不对称的 latent trap。
- 建议：与 picture_band 共用分类，或在 LayeredText::script 加 number < 0 的断言/测试。

### P3-4 draw_layers(&[]) 全空帧没有直接像素测试

- 位置：crates/wolf-render/src/renderer.rs:948-955 的 fallback 与 crates/wolf-render/tests/offscreen.rs:191-211 的 legacy 测试
- 现状：draw_layers(&[]) 的“clear 黑帧”逻辑存在且代码审查正确，但现有 empty_batch_does_not_replay_the_previous_frame 只调用 draw(&[])，没有调用 draw_layers(&[])。
- 建议：新增一条 offscreen 测试，先画一个红 sprite，再 draw_layers(&[]) + present，断言读回全黑且 instance_count()==0。

### P2-1 m12_ui_layers 没有 host 合成像素测试（覆盖缺口）

- 位置：crates/wolf-cli/tests/m12_ui_layers.rs:202-237
- 真实存档 display list 只验证排序键；不能捕获“Sort 对了但 band 语义错”“build_scene_layers 把 nameplate 和 choice 颠倒但排序仍单调”这类回归。
- 建议：在 m12_ui_layers 或 m12_ui_layers_pixels 中调用 play::build_scene_layers 后，用 Renderer::new_offscreen + draw_scene_layers：
  1. 同时构造 message 19500 + choice panel（可用 --event 247 或直接构造 Game 状态）；
  2. 断言 choice 文字区域颜色覆盖 message 文字区域；
  3. 断言 nameplate 底部/消息 frame 顶部无交叠（UI-07 几何）。
- 这不是 task-14 的阻断项，但应在 task-7 最终回归前补上。

### P2-2 offscreen text 测试在缺少字体/GPU 时会“静默通过”

- 位置：crates/wolf-render/tests/offscreen.rs:280-313
- test_font() 找不到 Lolita.ttf 时 offscreen_text_or_skip() 返回 None，每个测试 let Some(...) else { return; } → test 视为 passed；无 GPU 的 offscreen_or_skip 同理。
- 当前运行日志没有 skipping GPU text test，1.86s 内完成 10 个测试，说明本机确实执行了 GPU/字体路径；但 CI/headless 机器上会出现“0 个真实断言仍全绿”。
- 建议：用 #[ignore] + 显式 CI 环境变量，或在测试内 panic! 当 WOLF_GAME_DIR 应存在但字体缺失；至少输出 SKIPPED 统计并在 CI 中禁止 skip。

### P2-3 每层独立 submit 的性能上限

- 位置：renderer.rs:974-1039
- 最多 9 个 queue.submit（每个 band 一个）+ 1 个 blit submit；每层 text.prepare 会重新 shape 该层全部文本。当前 1024x576、文本量不大，实测 capture 时间正常；若未来 UI 层/文本量增大，可能成为 CPU 开销。
- 影响：非正确性问题，P2 观察项，不建议现在改；若优化，可用每层独立 glyph buffer（需修改 glyphon wrapper）或合并相邻无文本层。

## 7. 结论与建议

- **task-14 核心目标（多 SceneLayer/每层 submit、LayerBand 贯通、host UI 顺序）PASS**，可以进入 Lead 的集成复核。
- 不建议把当前 m11_visual_regression 的 2 个失败当作 task-14 通过；它们是 pre-existing（与 task-13 数值一致），但最终 release 前必须解决。
- 建议在 task-7 前至少补 P2-1 的 host 合成像素测试；P2-2 的 skip 行为应纳入 CI 可见性；P3-1/P3-2/P3-3 可作为后续清理，不阻断本次集成。
- 证据目录：work/out/reviews/task17_msg_600.png、task17_choice247.png、task17_ui05_912..917.png、task17_*.log；所有文件均在 work/out/reviews/ 下。

## 8. 复现命令清单

    # 1) 多图层 glyph 串位回归
    TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-render --test offscreen layer -- --nocapture

    # 2) 全部 offscreen
    TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-render --test offscreen

    # 3) host 层测试
    TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-cli --test m12_ui_layers
    TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-game --test m12_render_layer_priority

    # 4) UI-05 逐帧
    ./target/debug/wolf-cli.exe --root release/data capture --boot --frames 913 --key-frame 850:10 --press-frame 900 --pointer 80,425 --out work/out/reviews/task17_ui05_913.png

    # 5) 消息框/名牌
    ./target/debug/wolf-cli.exe --root release/data capture --map SampleMapA --frames 600 --hold-message --out work/out/reviews/task17_msg_600.png

    # 6) 选项面板
    ./target/debug/wolf-cli.exe --root release/data capture --event 247 --frames 300 --out work/out/reviews/task17_choice247.png
