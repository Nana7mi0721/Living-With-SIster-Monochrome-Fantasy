# Review: P0 输入修复（play.rs）独立复核

- 任务：task-9（revision 2），只读复核；不修改源码、测试、release、manifest 或存档。
- 审核快照：HEAD 8c65153；crates/wolf-cli/src/play.rs sha256 012f0497d3598fcb61d1b8a8fca4c8fa9aaa7ea1f8ef17f247b57f0571cf4b9a；crates/wolf-game/src/game.rs sha256 9e9c6047c88693e52572c9c9ceb3d06b0e935ccf2b25a22c08eec66aa741edc8（注意：工作树在复核期间被其他 agent 并行修改，game.rs 行号会随编辑漂移；本报告按此快照记录）。
- 说明：工作树是多个 agent 共享的脏树；play.rs 的 diff 里混有非输入改动（见第 6 节）。本报告只对 key_bits/wheel_notch/step/MouseWheel/MouseInput 这些 input hunks 下结论。

## 0. 结论

方向正确：UI-01 的滚轮 ±1 通道和 UI-02 的右键 CANCEL 注入都已接通，主路径可以通过；但 wheel_notch 把滚轮幅度压成符号，CE41 一类读取 engine_state(10) 的脚本会丢失幅度，另有失焦时 CANCEL 粘住风险；非 parked 空白点击误确认已在当前 game.rs 工作树中修复，需回归验证。建议先补第 1、2 条，再做人工滚轮/右键验收。

| 审核点 | 结论 | 级别 |
|---|---|---|
| 1. wheel_notch 只按符号给 ±1，像素/行模式覆盖 | 方向覆盖 LineDelta/PixelDelta；幅度被丢弃；水平滚动不会清掉 pending notch | P1（幅度）；P2（时序） |
| 2. step 写 wheel_dir 后同帧清空，与 CE131 轮询节奏 | 正常 60fps 循环能命中；Wait9/长重绘期间没有 123 轮询则丢失，无队列 | P2 |
| 3. 右键按住注入 CANCEL，长按事件 510 | 对 CE131/事件 510 是正确语义；不需要单帧脉冲；失焦时可能粘住 CANCEL | P2（失焦） |
| 4. 左键 OK 注入 + handle_choice_input 点击路径 | 点在选项行上不会双重确认；当前工作树已修复非 parked 空白点击落入 pressed&OK 的问题（game.rs:1844-1846 unconditional return），需回归验证 | 已修复，待测试 |
| 5. 与 CE41/普通方向菜单兼容 | 普通菜单 UP/DOWN 保留；CE41 的 engine_state(10) 幅度丢失（同上）；右键 CANCEL 可能压过 AutoInput | P1/P2 |

测试：TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-cli --test m12_ui_input -> 7 passed, 0 failed。该测试只覆盖 wheel_notch/key_bits 纯函数，没有覆盖 App::step、GameHost::key_code(771)、CE41 或真实窗口事件。

## 1. wheel_notch：方向覆盖，但幅度丢失

### 证据

- crates/wolf-cli/src/play.rs:246-258：wheel_notch 对 LineDelta(_, y) 和 PixelDelta(pos.y) 都只做 y > 0 / y < 0 判断，返回 (key::UP, 1) 或 (key::DOWN, -1)，否则 (0, 0)。
- crates/wolf-cli/src/play.rs:555-565：MouseWheel 把 bits 并入 self.wheel，且只有 notch != 0 时才写 self.wheel_dir；水平滚动返回 (0,0)，不会覆盖已有 self.wheel_dir。
- crates/wolf-game/src/game.rs:2877-2895：key_code / key_code_auto 在 mask == 771 且 wheel_dir != 0 时直接返回 self.wheel_dir。对 CE131 的 pc58 轮询，这个 ±1 是正确的。
- work/out/ce131.txt:60-66：pc58 op123 ints=[1600021,771]；pc59 明确比较 1600021 == 1 和 == -1；pc61/pc64 是 ホイール上/ホイール下 标签；pc299/pc314 是左右翻页分支。
- CE41：work/wolftl2/dump/common/41_マウス感知コモン.json，index 15 写 1600013 = engine_state(10)，index 33 对负数乘 -1，index 34/40 用 LoopTimes 1600013 控制 AutoInput 16/128。docs/OPCODE_124.md:100 明确写 field 10 是“滚轮转动量，有符号，通常 ±1..±3”。

### 问题

1. **幅度丢失（P1）**：LineDelta(0,3) 现在返回 (UP,1)，3 格滚轮只算 1 格；PixelDelta(0,360) 同样只算 1 格。CE41 的 LoopTimes 1600013 依赖原始幅度，快速滚轮/高分辨率触控板会明显变慢；engine_state(10) 也只返回符号，所有 124 field10 脚本都受影响。
2. **像素/行模式只覆盖符号，不覆盖单位（P2）**：LineDelta 的 y 是行数，PixelDelta 的 y 是物理像素；当前两种都直接当 1 个 notch。若一个 PixelDelta 事件是 120px，语义上应归一成 1 格；若驱动程序一次给 240px，应按 2 格或至少保留原始幅度给 CE41。
3. **水平滚动不会丢 pending（这一项是 PASS）**：horizontal-only 返回 (0,0)，MouseWheel 不会清 self.wheel_dir；同一帧内先竖后横，竖向 notch 仍在。真正会丢的是下一节的跨帧时序。

### 最小修法

把“方向码”和“原始幅度”拆成两个值，不要改 key::code()：

~~~rust
// play.rs
// wheel_dir 继续只给 mask 771 的 CE131 用：+1/-1。
// 新增 wheel_delta: i32，保留 LineDelta 的原始 y；PixelDelta 先累计，按 120px 归一。
fn wheel_notch(delta: &MouseScrollDelta) -> (u32, i32, i32) { ... } // (bits, sign, amount)
self.game.wheel_dir = self.wheel_dir;
self.game.wheel_delta = self.wheel_delta;
~~~

GameHost::key_code(771) 继续返回 sign；engine_state(10) 返回 wheel_delta（不是 sign）；Game::key_code_auto(771) 同样返回 sign。若不想新增字段，至少让 wheel_notch 接受 f64 幅度并在 Game 侧保存 raw，否则 CE41 无法恢复。

### 建议测试

- m12_ui_input：LineDelta(0,3) 的 sign 必须是 +1，但 raw 必须是 3；LineDelta(0,-2) raw 必须是 -2；PixelDelta(0,360) 归一为 3。
- Game 级：wheel_dir=1、wheel_delta=3，断言 key_code(771)==1 且 engine_state(10)==3。
- CE41 集成：给 wheel_delta=3，断言 LoopTimes 走 3 次 AutoInput 16；给 -3 时走 3 次 AutoInput 128。

## 2. step 同帧清空与 CE131 轮询节奏

### 证据

- crates/wolf-cli/src/play.rs:266-280：step 先写 game.wheel_dir = self.wheel_dir，再 game.frame()，然后 self.wheel_dir = 0。
- work/out/ce131.txt:60-61：CE131 的 pc58 InputKey 只在完成 Wait1 后轮询；脚本 hover 重绘分支（pc67/pc68）以及 pc55 的 Wait9 期间不在 pc58。
- crates/wolf-game/src/game.rs:1777-1876：handle_choice_input 在 vm.step_frame 之前运行，不读取 wheel_dir；真正消费它的是 GameHost::key_code 对应的 opcode 123。

### 问题（P2）

正常标题/存档循环下，鼠标事件到达后下一 step 会从 pc58 开始，能命中；但如果玩家在 Wait9 或长重绘期间滚轮，事件只存活到该帧结束，脚本还没轮到 pc58，notch 就丢了。没有 pending 队列，也不会自动重发。原版输入更新是否同样只保留一帧未能从现有资料确认，因此列为风险而非确定回归。

### 最小修法

二选一：

1. 保持“一帧事件”语义，只在轮询命中时消费：Game 增加 wheel_consumed: bool，GameHost::key_code(771) 读到 wheel_dir 时置 true；App::step 仅在 consumed 后清 wheel_dir，避免 Wait9 丢事件（但要防止陈旧事件进入下一菜单）。
2. 更简单：App 增加 wheel_pending: i32，step 传 pending，Game::frame 结束时若该帧没有收到 123 轮询则保留 1-2 帧再清；CE131 的 Wait9 最多 9 帧，需要确认是否值得覆盖。

### 建议测试

- 启动 CE131，在 Wait9 期间注入滚轮上，断言下一轮 pc58 能读到 1，而不是丢失。
- 连续两帧无 123 轮询后再轮询，断言不会把两次滚轮叠加成 2 页，也不会把陈旧事件带入新菜单。

## 3. 右键按住注入 CANCEL

### 证据

- crates/wolf-cli/src/play.rs:232-238：key_bits 在 mouse_buttons & 2 != 0 时始终 OR key::CANCEL，每帧都注入，不是脉冲。
- crates/wolf-cli/src/play.rs:567-581：MouseInput 只维护 mouse_buttons 的 bit 1/2/4，右键不直接生成 key code。
- crates/wolf-game/src/game.rs:1777-1876：handle_choice_input 用 pressed = key_bits & !previous_keys 判断取消，只有按下第一帧收到一次；因此菜单不会每帧重复取消。
- crates/wolf-game/src/key.rs:19-20：事件 510 pc975 在图片 EXIT “长按右键” 下轮询 mask 32，并要求结果为 11；这里需要按住期间持续读到 11。
- work/wolftl2/dump/common/510_アルバムモード表示.json:8763-8820：Wait1 -> InputKey [1600012,32] -> 检查 11/0，确认长按语义。
- crates/wolf-game/src/key.rs:119-149：key::code 的优先级是 OK > CANCEL > SUB > PAUSE > 方向；右键按住确实会稳定返回 CODE_CANCEL。

### 结论

- CE131 cancel：正确。pc201 InputKey [1600020,33] 用 key_code 读 key_bits，右键得到 11，进入 pc200 的 キャンセル 分支。
- 事件 510 长按右键：正确。这里是 level-triggered 轮询，持续 11 是脚本要求，不需要单帧脉冲。
- 普通 Choice 菜单：正确。handle_choice_input 用上升沿，按住右键不会每帧取消。
- **右+左同时按住（P2 边界）**：key::code 先检查 OK 再检查 CANCEL，所以左键会压过右键返回 10；如果玩家同时按两个键，决定优先。这是否符合原版未验证，建议至少测试并记录。

### 新增风险：失焦时 CANCEL 粘住（P2）

play.rs 的 WindowEvent 处理没有 Focused(false) 分支；只有 CursorLeft（play.rs:551-554）会清 mouse_buttons。若按住右键时 Alt-Tab/窗口失焦，release 事件可能收不到，mouse_buttons bit2 会一直保留，key_bits 每帧注入 CANCEL，游戏可能永久处于取消状态。

最小修法：

~~~rust
WindowEvent::Focused(false) => {
    self.held.clear();
    self.mouse_buttons = 0;
    self.wheel = 0;
    self.wheel_dir = 0;
}
~~~

建议测试：模拟 Focused(false) 后 key_bits 不再含 CANCEL；重新聚焦后不残留。

## 4. 左键 OK 注入 + handle_choice_input（当前工作树已修复）

### 证据

- crates/wolf-cli/src/play.rs:266-280：click = left_click_fresh()；game.key_bits = bits() | if click { key::OK } else { 0 }。
- crates/wolf-game/src/game.rs:1777-1876（当前快照）：点击命中选项行时 1829-1833 reply(Index(row)) 并 return；parked 菜单用 picture_hover 命中时 1834-1843 也 reply 并 return。
- crates/wolf-game/src/game.rs:1844-1846：clicked 但未命中任何行时现在无条件 return，注释 “NEW-02: a blank click must not confirm the highlighted option.”，不会再落到 pressed&OK。
- crates/wolf-game/src/game.rs:1811-1823：非 parked 弹窗已有 pointer_moved/prompt_fresh 触发的 hover 预选，移动鼠标到某行会更新 choice_cursor/Sys20；这同时修复了 UI-04 的非 parked 部分。

### 结论

- 点在选项行上：不会双重确认，主路径正确。
- 点在非 parked 弹窗空白处：当前工作树已消费点击并 return，不会误确认当前 cursor；该问题已由并行的 game.rs 修改修复，不是 play.rs diff 的遗漏。
- UI-04 的 hover 预选也已在同一处修复；因属于 game.rs 并行修改，不能作为 play.rs 本次 diff 的验收证据，需要独立回归。
- 仍需测试：这次 fixed 分支是在复核期间由其他 agent 改入的，报告无法保证它与最终提交一起保留；请 Lead 以最终工作树为准跑下面用例。

### 建议测试

- 构造非 parked Choice，pointer 落在任一选项外，mouse_buttons 从 0->1，断言 reply 队列仍为空且 choice_cursor 不变。
- pointer 落在第 2 行，断言 reply 是 Index(1)，且没有额外的 Index(choice_cursor) 回复。
- pointer 悬停第 2 行但不点击，断言 choice_cursor==1 且预算/标记随 hover 更新；随后点击空白，断言没有 reply。

## 5. CE41/普通方向菜单兼容性

### 普通方向菜单

wheel_notch 仍返回 key::UP/key::DOWN，key::code 对 UP/DOWN 返回 CODE_UP=8 / CODE_DOWN=2；mask 771 的特殊分支只在 key_code/mask==771 进入，普通菜单不受影响。m12_ui_input 的 ordinary_wheel_still_speaks_the_direction_key_codes 已覆盖这一点。

### CE41 / engine_state(10)

- crates/wolf-game/src/game.rs:2995：engine_state(10) 返回 self.wheel_dir（符号），但 docs/OPCODE_124.md:100 要的是原始有符号滚轮量 ±1..±3；CE41 的 index 33/34 用它控制循环次数。
- 这与第 1 条是同一根因：CLI 只把符号塞进 wheel_dir，GameHost::engine_state(10) 没有独立的 raw delta。
- 兼容性风险：若 CE41 在右键按住时执行 AutoInput，key_code_auto 走 key::code(self.key_bits|auto)，CANCEL 优先级高于 auto 方向，可能让 CE41 的方向输入变成取消；建议在 CE41 场景测试，必要时让 auto 方向在 mask != 771 时优先于右键 CANCEL。

### 结论

普通菜单 PASS；CE41 幅度 FAIL（P1，同第 1 条）；右键+AutoInput 是 P2 待确认。

## 6. 工作树范围污染（必须告知 Lead）

git diff --stat -- crates/wolf-cli/src/play.rs：191 insertions(+), 29 deletions(-)，共 220 行。除输入修复外还包含：

- crates/wolf-cli/src/play.rs:76：game.enable_audio()（音频行为改动）。
- crates/wolf-cli/src/play.rs:297-349：choice_overlay/nameplate 绘制（选择框/名牌 UI）。
- crates/wolf-cli/src/play.rs:390-419：message 文本/nameplate 布局。
- crates/wolf-cli/src/play.rs:461：窗口从 1920x1080 改为 PhysicalSize::new(1024,576)（窗口大小/DPI 相关，不是输入逻辑）。

这些改动看起来是其他任务的并行修改，不能算在 fix-frontend-input 的验收范围；本报告的 PASS 只覆盖 key_bits/wheel_notch/step/MouseWheel/MouseInput，不代表这些 hunk 已复核。建议 Lead 按文件/任务拆分后提交，并在最终验收时对完整 diff 再做一次人工审查。

## 7. 测试与证据清单

- `TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-cli --test m12_ui_input`：7 passed, 0 failed。
- `git diff --check -- crates/wolf-cli/src/play.rs`：exit 0，无空白错误。
- 代码证据：crates/wolf-cli/src/play.rs:232-280、555-581；crates/wolf-game/src/game.rs:2877-2895、2995、1777-1876；crates/wolf-game/src/key.rs:19-20、119-149。
- 数据证据：work/out/ce131.txt:60-66；work/wolftl2/dump/common/41_マウス感知コモン.json index 15/33/34/40；work/wolftl2/dump/common/510_アルバムモード表示.json 8763-8820；docs/OPCODE_124.md:100。
- 测试缺口：m12_ui_input 没有 App::step 级滚轮清除测试、没有 GameHost::key_code(771) 测试、没有 CE41/事件510/失焦测试；建议按第 1/2/3/4 节补。

## 8. 建议处置

1. 先修 P1 幅度丢失（第 1 条）：保留 sign 给 mask 771，新增 raw wheel_delta 给 engine_state(10)/CE41；这不会破坏 UI-01/UI-02。
2. 决定是否接受 P2 一帧丢失（第 2 条）；若要保证 Wait9 期间滚轮可用，加 pending/consumed。
3. 补 Focused(false) 清理，避免右键 CANCEL 粘住（第 3 条）。
4. 回归验证当前工作树已修复的非 parked 空白点击/悬停预选（第 4 条；该修复在 game.rs，不属于 play.rs diff 本身）。
5. 把 play.rs 中非输入改动从本任务验收范围拆出，按第 6 节告知 Lead。

以上为只读复核结论；未修改任何源码/测试/存档。
