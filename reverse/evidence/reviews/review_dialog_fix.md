# F2 最终 diff 独立复核报告（task-12）

- 复核对象：F2 对话/标题/选项/夜晚妹妹恢复修复，工作树当前状态（含 task-4/6/8 的并行改动）。
- 复核人：review-game-events（独立复核，未修改任何产品源码/测试/存档）。
- 复核日期：2026-11-04。
- 结论：**不是全量 PASS。** UI-03 与 UI-11 在真实 CLI 路径仍失败；UI-04/06/07/08/09、UI-10 通过（UI-09 的 DB face 兜底仅代码级检查）；另有 3 项残留/待定风险。

## 0. 复核范围、方法与限制

### 0.1 复核文件

F2 的主要改动集中在：

- `crates/wolf-game/src/game.rs`：title menu 状态机、room base picture 快照恢复、message speaker/nameplate、choice hover。
- `crates/wolf-game/src/message.rs`：`styled_items`/`inline_runs_with`/`layout_styled_with` 的变量解析。
- `crates/wolf-game/src/choice.rs`：`styled_width_with`/`layout_at_sized_with`/多 run 绘制。
- `crates/wolf-vm/src/exec.rs`：101 的 face 兜底、121 的多槽复合赋值、210 的 host 调用。
- `crates/wolf-game/src/savegame.rs`：字体 size=0 往返、读档后的 `prime_latched`。
- `crates/wolf-game/src/db.rs`：`name_of`。

### 0.2 限制

1. 当前工作树混合了多个并行任务的改动，无法只从 git base 拆出 F2；本报告以当前工作树 + F2 相关 hunk 为准。
2. `--load-save` + `--event` 的 CLI 入口不启动地图运行时；UI-11 不能直接用 `--event 247` 作为用户路径，必须从真实 `CE245 -> 210 CE247` 菜单进入。本报告已按该路径重做。
3. 无法在 CLI 中完整模拟“鼠标点击 CE245 的自绘按钮 + 后续地图层输入”的全部组合；但 `--pointer + --press-hold` 已能稳定打开 CE245 子菜单和 CE247 顶层菜单，足以验证本次状态机。
4. 所有截图均在 `work/out/reviews/`；`release/data/save/system.sav` 在测试中可能被事件改写，已按既有备份恢复（详见 §7）。

## 1. 结论矩阵

| UI | 验收要点 | 结论 | 关键证据 |
| --- | --- | --- | --- |
| UI-03 | ESC 退出存档后标题三行菜单重新出现，方向键/滚轮/鼠标均可操作 | **FAIL** | `review_f2_title_after_esc3.png`：菜单图在，但 `choice none`，按键无效 |
| UI-04 | 普通 102 指针悬停更新选中，左键确认，移开不改变 | **PASS** | 指针 600,220/250/280/310 -> cursor 0/1/2/3；点击 600,280 -> 选项生效 |
| UI-06 | 玩家确认前持续显示，控制码/换行完整，不提前消失 | **PASS**（边沿语义） | `review_f2_ui06_held_edge.png`，600 帧仍显示 2 行 27 字 |
| UI-07 | 名牌在消息框上沿外侧，留间距，不与正文/边框重叠 | **PASS**（视觉） | 名牌“妹妹”约 y354-392，消息框顶约 y395，间隔约 3px |
| UI-08 | 选项真实字形宽度，`\cdb` 等控制码解析，无截断 | **PASS**（单测级） | `m12_ui_dialog` 1 passed；仅覆盖 `\cdb`，未覆盖所有控制码 |
| UI-09 | 角色说话有正确名字；旁白无名牌；来自包装参数/DB 行名 | **PASS（显式名）/ 待定（DB face 兜底）** | `ui09_ce281_named.png` 名牌“妹妹” |
| UI-10 | 睡眠/跨天后数值恢复，HUD 与变量一致 | **PASS**（task-11 已复核） | `ui10_sleep_save01_end.png` vs `ui10_sleep_save01_fixed2.png` |
| UI-11 | 互动一次后妹妹仍在画面；场景切换/重绘后角色层保持 | **FAIL** | `review_f2_ui11_after_chat.png`，真实 CE245->CE247 聊天分支结束后无妹妹 |

## 2. 关键发现

### F2-01 [P1] UI-03：ESC 关闭存档页后，标题菜单视觉恢复但输入状态机没有恢复

**现象（已复现）**

命令：

```bash
./target/debug/wolf-cli.exe --root release/data capture --menu-only --frames 260 \
  --pointer 10,460 --press-frame 60 --press-hold 3 \
  --key-frame 200:11 --out work/out/reviews/review_f2_title_after_esc3.png
```

输出关键行：

```
pictures  : 5 live
pic -100  File ... Kagura/bg.png
pic -2    File ... SystemGraphic/TitleGraphic.jpg
pic 3     File ... Picture/menu1_b.png at (10,400)
pic 4     File ... Picture/menu2_a.png at (10,450)
pic 5     File ... Picture/menu3_b.png at (10,500)
choice    : none open
sys pads  : Sys20 1
stats     : 11403 instruction(s), 118 picture op(s), 1 sound(s), 558 db access(es), 0 message, 1 choice
```

截图对比：

- 基线：`work/out/reviews/review_f2_title_menu.png`（`choice HELD 3 options ["1","2","3"]`，5 pictures，60 instruction）
- 存档页打开：`work/out/reviews/review_f2_title_save_page.png`（24 pictures，16 text pictures，`SystemFile/WindowBase_amania.png` 存在）
- ESC 后：`work/out/reviews/review_f2_title_after_esc3.png`（三行菜单图仍在，但 `choice none`；追加方向键/点击 stats 不再增长）

**根因（源码级）**

1. `crates/wolf-game/src/game.rs:858-866` 的 `resume_title_menu_if_ready()` 只调用 `start_title_menu()`。
2. `crates/wolf-game/src/game.rs:773-786` 的 `start_title_menu()` 只启动 `TITLE_MENU_EVENT=43`；event43 只负责画三行菜单图和 hover 贴图，不创建 `102` 选择窗口。
3. 标题的 `102 Choices`（dump `intArgs=[19]`、`stringArgs` 3 项）在 event42 的 pc79；event42 的选择分支（pc80/85/92）执行后会结束，存档页关闭时 event42 已经不存在，因此不会重建这个 parked choice。
4. `crates/wolf-game/src/game.rs:1935-1954` 的 `handle_title_menu_input()` 只更新 `Sys20` 和 menu 贴图，不 reply；`crates/wolf-game/src/game.rs:1956-1963` 的 `handle_choice_input()` 在 `self.choices` 为 `None` 时直接 `return`。所以画面有菜单，输入全失效。
5. 触发存档页时 `crates/wolf-game/src/game.rs:2060-2068` 的条件是 `title_menu_live() && screen.has_file_containing("SystemFile/WindowBase")`。若 event43 已先结束（只剩图片），`title_menu_live()` 为 false，`stop_title_menu()` 不会运行，`title_menu_suspended` 也不会置位；之后既不会正确 suspend，也不会恢复 event42。

**最小修改建议**

- 方案 A（推荐）：把标题控制状态机从 event42 的实例生命中解耦。`resume_title_menu_if_ready()` 在重新启动 event43 的同时，显式重入 event42 的菜单段（或调用一个只负责重开 `102` 的 title menu 函数），并重新建立 `choices` 的 3 个选项。
- 方案 B：event42 选择分支不要直接结束，进入一个等待循环；存档页关闭后回到该循环的 `102`。
- 无论哪种方案，`stop_title_menu()` 的触发条件应改为“存档页打开且标题屏幕活跃”，不要依赖 event43 仍 live；并且 `start_title_menu` 不能只启动视觉 event43。
- 不要把“重新启动 event42 整个开场”作为实现，否则会重播标题 BGM/logo/开场变量。

**建议测试**

- 集成测试：`--menu-only` -> 点击第 2 行 -> 等存档页 -> ESC -> 断言 `choices.is_some()` 且 3 个选项、`title_menu_suspended == false`、方向键 DOWN 后 `choice_cursor==1`、鼠标点击第 3 行回复第 3 项。
- 回归：`cargo test -q -p wolf-game --lib title_menu`（当前 5 passed，但未覆盖 ESC 存档路径）。

### F2-02 [P1] UI-11：真实 CE245 -> CE247 聊天分支结束后，妹妹仍从房间消失

**现象（已复现，真实入口）**

命令：

```bash
./target/debug/wolf-cli.exe --root release/data capture \
  --load-save work/retail_backup/Save/SaveData01.sav \
  --event 245 --frames 600 --pointer 510,250 --press-frame 60 --press-hold 5 \
  --answer 0 --answer-delay 40 --press-every 25:10 \
  --out work/out/reviews/tmp_f2_ui11_ok_600.png
```

输出/截图：

- 最终 `pictures: 62 live`；live list 中没有 `pic 90/91/92/93/98`，也没有 `pic 120`；`choice none open`。
- `work/out/reviews/review_f2_ui11_after_chat.png`：22:20 自宅房间，HUD 行动力 65/120，房间无妹妹。
- 基线 `work/out/reviews/review_f2_ui11_ce245_comm.png`：同一存档 21:40，CE245 夜菜单，妹妹在右侧（`pic 90 Picture/stand_imouto_paja2.png` 等，HUD 75/120）。
- 对比图：`work/out/reviews/review_f2_ui11_before_after.png`（1024x1232，BEFORE/AFTER）。

控制实验：

- `--answer 0 --answer-delay 600 --frames 900`（`review_f2_ui11_ce245_delay600.png`）：最终 69 pictures，`pic 90/91/92/93/98` 仍在，`choice HELD 3 options ["无关紧要的话题","色色的话题","有点可怕的话题"]`。这只是尚未走到 CE218 -1，不能算 UI-11 PASS。
- `--answer-delay 40`：100 帧时 `choice` 4 项、`pic90` 在；150 帧后 `choice none`、90 消失；500/600/700 帧仍是房间无妹妹，说明分支结束/返回后没有恢复。

**根因（源码级，两层）**

1. **快照在 CE247 结束后被清掉**：`crates/wolf-game/src/game.rs:1380-1410` `refresh_room_base_pictures()`。当 `menu_live == false`（CE247 不再 live）时，只有 `parent_menu_live && room_restore_pending` 才保留快照；否则 `room_base_pictures.clear(); room_base_captured=false; room_restore_pending=false;`。真实路径在聊天分支结束后直接回到地图/父级，CE245 也已不 live，快照被清空，`restore_room_base_if_ready` 再无快照可恢复。
2. **pending 在“全部槽位都缺失”判断之前被消耗**：`crates/wolf-game/src/game.rs:1455-1461`：

```rust
let all_missing = self
    .room_base_pictures
    .iter()
    .all(|picture| self.screen.get(picture.number).is_none());
self.room_restore_pending = false;   // <-- 先消费
if !all_missing {
    return;                            // <-- 只要有一个临时槽位还在就永远不再重试
}
```

只要 CE218 的临时 pose/发型/表情还占着 90..98 中任意一个槽位，pending 就被用掉；之后这些槽位再被后续 CE218 删除，也不会触发第二次恢复。

3. CE247 的快照来源：`crates/wolf-game/src/game.rs:2617-2630` 在 `target==ROOM_MENU_EVENT` 时把 90..=98 复制到 `room_base_pictures`；`crates/wolf-game/src/game.rs:2635-2640` 在 `CE218 args[0]==-1` 且快照非空时置 `room_restore_pending=true`。快照本身是对的（基线检查有 90/91/92/93/98），问题在生命周期/消费条件。

**最小修改建议**

- 快照生命周期改为“房间会话级”：`room_base_pictures` 只在切换到非房间地图/明确加载新场景时清空，CE247 结束不应清空；用地图 id 或 `CE325` 房间重绘作为边界。
- `restore_room_base_if_ready` 只在真正恢复成功后清 `room_restore_pending`；`!all_missing` 时保持 pending，下一帧继续尝试。
- 恢复触发条件从“只认 CE247/CE245 的菜单 wait”放宽为“回到房间地图且当前没有临时立绘/菜单选择”，避免真实地图返回路径没有 4 项 choice 时永远不恢复。
- 如果采用“快照能保留但恢复可能压住临时 pose”的保守策略，应记录每个槽位的占用来源，而不是仅用 `all_missing` 全局布尔。

**建议测试**

- 集成测试：载入 SaveData01 -> 真实 `CE245` 菜单 -> 点“沟通” -> 走完一个聊天分支返回 -> 断言 `screen.get(90)` 或 `screen.get(120)` 存在，且不是临时 CE218 pose。
- 单测补充：快照跨 `CE247 停止 -> CE245 停止 -> 地图帧` 仍存在；`all_missing=false` 时 pending 不被清空，下一帧变为全缺失时可恢复。
- 回归截图：`review_f2_ui11_before_after.png` 作为视觉基线。

### F2-03 [P2] `clear_night_room_hud` 仍把 108..120 全删，包含妹妹 picture120 与 speaker 118/119

**代码**

`crates/wolf-game/src/game.rs:744-747`：

```rust
fn clear_night_room_hud(&mut self) {
    self.screen.remove_range(1600, 1621);
    self.screen.remove_range(108, 120);
}
```

`remove_range` 是闭区间（`crates/wolf-game/src/picture.rs:1148-1152`）。数据语义：

- 108 = `sub_button02.png`，109-111 = `sub_button03/04/05.png`（菜单子按钮）
- 118 = `speaker.png`
- 119 = 三行 ■/□ 文本
- 120 = `CE325 ■自宅` 的 `Picture/imouto01..04.png` 妹妹立绘

直接实验：`capture --load-save SaveData02 --event 332 --frames 2` 从 58 pictures 降到 56，`pic120` 消失；说明 `start_event(332)` 的 clear 会直接删 120。正常就寝路径 `CE328` 本身在 idx 704/732 已有 `Picture[16777218,108,0,0,0,120]` 的 108..=120 擦除，因此引擎这刀可能是重复实现；但它对任何直接进入 332 的路径仍会误删 120/118/119。

**最小修改建议**

- 若保留引擎清理：改为精确槽位 `remove_range(108,111)` + `remove_range(118,119)`，不要碰 120。
- 或删除这段重复清理，完全交给 CE328 的显式 erase；先核对 CE332 自身是否有依赖该清理的路径。
- 增加回归：`start_event(332)` 前后断言 120 保留/按事件脚本决定。

**建议测试**

- `cargo test -q -p wolf-game --lib room_base` 增加 332 直接启动用例，断言 picture120 不被引擎误删。
- 截图：`work/out/reviews/ui11_save02_ce332.png`（旧证据）与修复后的同命令对比。

### F2-04 [P2/待定] `message_speaker` 的 DB face 兜底可能给旁白/普通 101 错误名牌

**代码**

- `crates/wolf-game/src/game.rs:1689-1703`：显式 `message.speaker` 优先；否则 `message.face?` -> `databases.name_of(Container::Sys, 24, face)`。
- `crates/wolf-vm/src/exec.rs:1760-1764`：每个普通 101 只要 `Sys0>=2`，就把当前 Sys0 快照进 `Message.face`。
- `crates/wolf-game/src/game.rs:2672-2677` `show_message` 保存 face。

风险：非对话的普通 101（旁白、系统提示）在 Sys0 仍为上一个角色脸号时，会得到上一个角色的 DB 行名，出现“旁白被标上妹妹”的错误名牌。验收要求“旁白无名牌”；当前没有真实事件复现（零售角色台词大多有 CE264 显式名），因此标**待定**，但需要在合入前加旁白/系统提示用例。

**最小修改建议**

- 只对显式 `@n` 消息或 CE264 明确要求显示脸的消息使用 DB 行名兜底；不要对所有 `Sys0>=2` 的普通 101 自动启用。
- 或者增加 `Message.face_from_prefix/face_from_sys0` 标志，仅在 `@n` 或包装事件明确指定时查 DB 名称。

**建议测试**

- 构造“先角色对话（Sys0=妹），再旁白 101”的序列，断言旁白 `message_speaker()==None`。
- CE281/CE264 显式名回归仍应显示“妹妹”。

### F2-05 [P2/待定] op121 的多槽 `arg4` 在 event38 会把 601 个槽当成范围

**代码**

`crates/wolf-vm/src/exec.rs:1843-1853` 对 argc>=5：

```rust
let count = eval_num(arg(cmd, 4), inst, vars).clamp(0, 1024);
for offset in 0..=count { ... }
```

CE94 的多个站点（pc87/92/99/107）证明 `arg4` 是“inclusive last offset”，例如 `[1600022,10,0,66560,3]` 写 1600022..1600024。但这个语义在 `event 38 pc2 [2000001,0,0,65536,600]` 会写 2000001..2000601 共 601 槽；是否原版有意初始化整个系统区仍未确认。

**最小修改建议**

- 先 dump event38 的执行结果与原始 WOLF 行为对比；如果只是初始化到 2000600 的边界，应改写为明确的批量初始化或把语义限定在已知范围。
- 在 op121 单测中增加 `arg4=0/1/3/600` 的边界用例。

## 3. UI 逐项复核

### 3.1 UI-03：标题菜单恢复（FAIL）

见 F2-01。补充：

- 当前 `cargo test -q -p wolf-game --lib title_menu` 是 **5 passed, 0 failed**。此前曾出现 3 passed/2 failed（`game::tests::the_title_menu_starts_once_and_tracks_the_pointer`，game.rs:3573；`clicking_a_parked_title_menu_row_answers_that_row`，game.rs:3596），并行改动后当前已绿；但这两个测试只覆盖标题菜单自身 hover/点击，不覆盖“存档页 -> ESC”的状态机，因此不能作为 UI-03 通过依据。
- 真实截图基线 `review_f2_title_menu.png` 有 3 个 choice；ESC 后 `review_f2_title_after_esc3.png` 只有 menu 图片，无 choice。两者视觉相似，但输入状态不同，正是用户“菜单丢了且无法操作”的复现。

### 3.2 UI-04：普通 102 hover 预选（PASS）

命令：

```bash
./target/debug/wolf-cli.exe --root release/data capture \
  --load-save work/retail_backup/Save/SaveData01.sav --event 247 --frames 300 \
  --pointer 600,220 --out work/out/reviews/review_f2_ui04_hover_row0.png
# 同理 600,250 / 600,280 / 600,310
```

输出分别 `choice HELD on prompt with 4 option(s) ["聊天","0","小鸟飞飞！","讨好"]`，`cursor 0/1/2/3`，`anchor (512,192)`。点击 `--pointer 600,280 --press-frame 100 --press-hold 5` 后 `choice none`、`1 message`；截图 `review_f2_ui04_hover_row2.png`、`review_f2_ui04_click.png`。

代码：`crates/wolf-game/src/game.rs:1960-2002` 普通分支只在 `pointer_moved || prompt_fresh` 时预选；移开不改变已选项，符合验收。注意：`--pointer 700,220` 会落到行宽外，cursor 仍 0，不是缺陷。

### 3.3 UI-06：消息不提前消失（PASS，边沿语义）

命令：

```bash
./target/debug/wolf-cli.exe --root release/data capture --event 281 --frames 600 \
  --hold-message --press-every 1:10 --out work/out/reviews/review_f2_ui06_held_edge.png
```

输出：`message: 2 row(s), 27/27 char(s), font 20 at [64,410], wrap 928, cursor [576,554]`；`stats: 1 message(s)`。600 帧仍显示，说明第一帧的 OK 边沿只关闭第一条，后续持续按住 OK 不会关闭下一条。`game.rs:2681-2686` `message_dismissed = auto_advance || pressed_bits & OK` 使用帧首按下沿，而不是 held。

限制：capture 默认 `auto_advance = !hold_message`；不传 `--hold-message` 时消息会被 headless 自动关闭，不能用于 UI-06 测试。

### 3.4 UI-07：名牌几何（PASS，视觉；frame 缺失路径待定）

`review_f2_ui06_held_edge.png` 中“妹妹”名牌矩形约 y354-392，消息框顶约 y395，间隔约 3px；三行文本在框内。代码 `crates/wolf-game/src/game.rs:1710-1732`：

- 有 `FRAME_PICTURE=19500` 的 window frame：`rect[0]+4, rect[1]-box_h-8`
- 无 frame：回退 `vars.x+4, vars.y-box_h-2`

当前真实 CE63 窗口路径有 frame，视觉通过；无 frame 的测试/自定义消息仍可能贴上沿，建议补单测固定回退几何。

### 3.5 UI-08：选项控制码/宽度（PASS，单测级）

- `crates/wolf-game/tests/m12_ui_dialog.rs` 的 `an_option_resolves_database_variables_and_is_not_truncated`：option `你是……\cdb[0:12:0]\cdb[0:12:1]的哥哥。` 解析为 `你是……妹妹的哥哥。`；断言 message layout 无 `cdb`、choice 行宽 >= painted_width。`cargo test -q -p wolf-game --test m12_ui_dialog` -> 1 passed。
- 代码：`choice.rs:114-123` `styled_width_with` 用 `message::inline_runs_with` + `choice::advance_at` 求和；`choice.rs:231-252` `layout_at_sized_with` 返回真实行宽。
- 观察项：CE247 顶层第二项显示 `"0"`（dump 中选项为 `\cself[5]`）。当前测试未覆盖 `\cself`；可能是 CLI 直启事件缺少 map 层 self 字符串，真实地图路径可能不同，标**未定论**，不作为 FAIL。

### 3.6 UI-09：说话者名字（显式名 PASS；DB face 兜底待定）

- 真实 CE281（pc3 `CommonEvent[500264,12323,0,2002,1,0,0] ['','妹妹','']` -> pc4 `Message ...`）：`ui09_ce281_named.png` 显示左上角名牌“妹妹”，2 行文本 font20 at [64,410]；CE264 包装链路正常。
- `Message.face` 现在在 `game.rs:1689-1703` 被 DB 行名使用；`db.rs:492-506` `name_of(Container::Sys,24,row)` 是正确接口。风险见 F2-04：普通 101 自动快照 Sys0 可能污染旁白。
- 全 dump 13169 条 101 中只有 20 条 `@n`，且全部 `@1`（物品/金钱提示），没有真实角色台词依赖 `@n`；因此 UI-09 的主要可见路径靠 CE264 显式名字，DB 兜底是补强而非主路径。

### 3.7 UI-10：HUD 读档恢复（PASS，task-11 复核）

- 修复前 `ui10_sleep_save01_end.png`：91日9:30，HUD 行动力 75/120、精力 4/6、性欲 56%、金钱 7586。
- 修复后 `ui10_sleep_save01_fixed2.png`：91日9:30，HUD 行动力 120/120、精力 6/6、性欲 60%、金钱 7586。
- `auto_events.rs:93 RESUME_ON_LOAD=&[216,217]`、`auto_events.rs:193-215 prime_latched`、`auto_events.rs:252-296 sync` 的白名单每帧重入逻辑已由 task-11 复核；`m12_ui_state` 2 passed，506 不启动、216/217 不重复启动。结论沿用 task-11 PASS。

### 3.8 UI-11：夜晚互动后妹妹保持（FAIL）

见 F2-02。补充：

- 直接 `--event 247` 不经过 `Host::common_event_call`，不会触发 `capture_room_base_pictures` 的 host 路径，因此其“妹妹消失”不能作为 F2 结论；本报告用真实 `CE245` 菜单包装路径复现。
- 基线 `review_f2_ui11_ce245_comm.png` 显示 picture 90 等妹妹立绘；`review_f2_ui11_after_chat.png` 为同一存档聊天分支结束后的 22:20 房间，妹妹完全不在。
- 该路径是用户实际可操作的“身体接触/沟通/洗澡...”中的“沟通 -> 聊天”，满足 UI-11 的“互动一次后”语义。

## 4. 回归测试结果

| 命令 | 结果 | 说明 |
| --- | --- | --- |
| `TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-game --test m12_ui_dialog` | **1 passed** | UI-08 控制码/宽度 |
| `TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-game --test m12_ui_state` | **2 passed** | HUD 216/217、506 抑制、睡眠重绘 |
| `TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-game --lib title_menu` | **5 passed** | 标题菜单 hover/点击；未覆盖 ESC 存档 |
| `TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-game --lib room_base` | **2 passed** | 合成快照恢复；未覆盖真实 CE245/CE247 路径 |
| `TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-game --test m13_all_endings` | **14 passed；2 ignored** | 原 9 项失败已在 `m13_all_endings.rs:94-99` 改为隔帧脉冲 OK；夹具合理，不是产品回归 |

测试结论：F2 新增/修改测试当前全绿，但 **UI-03 的 ESC 存档路径、UI-11 的真实 CE245 路径都没有被现有测试覆盖**；全绿不等于两项验收通过。

## 5. 新发现/残留问题（相对 F2）

| 编号 | 严重度 | 问题 | 关联 UI | 状态 |
| --- | --- | --- | --- | --- |
| NB-F2-01 | P1 | ESC 退出存档后标题菜单只恢复视觉，不重建 event42 的 parked 102，输入失效 | UI-03 | 已复现 |
| NB-F2-02 | P1 | CE245->CE247 聊天分支结束后，room base 快照被清空/ pending 提前消费，妹妹不恢复 | UI-11 | 已复现 |
| NB-F2-03 | P2 | `clear_night_room_hud` 的 108..120 误删 picture120（妹妹）与 118/119（speaker） | UI-11/UI-12 | 代码级 + 332 直接实验 |
| NB-F2-04 | P2/待定 | `message_speaker` 对普通 101 的 Sys0 face 做 DB 行名兜底，可能给旁白错误名牌 | UI-09 | 代码级，待真实事件复现 |
| NB-F2-05 | P2/待定 | op121 argc>=5 的 `arg4` 在 event38 写入 601 槽，范围语义待确认 | 数据/状态 | 待验证 |

## 6. 待定/未复现项

1. **UI-03 完整鼠标/滚轮路径**：CLI 可以复现 ESC 后 choice 消失，但无法完全模拟用户在存档页的滚轮翻页/存档槽点击；结论以“ESC 后 choice none + 按键 stats 不变”为准，已足以判 FAIL。
2. **UI-11 其他分支**：目前确认“沟通 -> 聊天”分支失败；洗澡/身体接触分支可能走不同 CE218 mode/CE325 重绘，未逐一做完整点击；报告不声称所有分支都相同，但 UI-11 验收已被一个真实分支打破。
3. **UI-09 旁白污染**：没有找到真实零售事件中“Sys0>=2 后紧接旁白 101”的确定组合；需要全 dump 静态搜索 + 真实 capture 才能定论。
4. **`\cself[5]` 显示 0**：CE247 顶层菜单第二项在 CLI 直启时显示 “0”，可能是缺少 map 层 self 字符串，也可能是 `inline_runs_with` 对 `\cself` 的解析缺口；当前 UI-08 单测只覆盖 `\cdb`。
5. **event38 op121 601 槽**：需要对比原版 WOLF 的 save/初始化结果，当前只确认代码会写 601 槽，不确定是否有意。

## 7. 复现资产与清理

### 7.1 截图

- `work/out/reviews/review_f2_title_menu.png`：标题菜单基线（choice 3 项）
- `work/out/reviews/review_f2_title_save_page.png`：存档槽页面
- `work/out/reviews/review_f2_title_after_esc3.png`：ESC 后菜单图在但 choice none（UI-03 FAIL）
- `work/out/reviews/review_f2_ui04_hover_row2.png`、`review_f2_ui04_click.png`：UI-04
- `work/out/reviews/review_f2_ui06_held_edge.png`：UI-06/07
- `work/out/reviews/ui09_ce281_named.png`：UI-09 显式名牌
- `work/out/reviews/ui10_sleep_save01_end.png`、`ui10_sleep_save01_fixed2.png`：UI-10 修复前后
- `work/out/reviews/review_f2_ui11_ce245_comm.png`、`review_f2_ui11_after_chat.png`、`review_f2_ui11_before_after.png`：UI-11 基线/失败/对比

### 7.2 存档/环境

- 主用存档：`work/retail_backup/Save/SaveData01.sav`（21:40，90日，行动力 75/120）
- 对照存档：`work/retail_backup/Save/SaveData02.sav`（18:30，2日，35/61）
- `release/data/save/system.sav` 在跑 CE329/部分事件时会写入；已从 `work/out/release_save_backup/system.sav` 恢复。备份 md5：`564f3570677e10dfef2eae2f10aa69e8`。复核前后请用 `md5sum` 确认。
- CLI：`target/debug/wolf-cli.exe`；工作目录仓库根。

### 7.3 限制说明

本报告只读产品源码、测试和存档；未修改任何产品文件。报告与截图均写在 `work/out/reviews/`。如需我在 F2 修复后复审 UI-03/UI-11，建议直接用同一条命令重跑并对比本报告基线图。

## 8. 给 Lead 的结论摘要

- **PASS**：UI-04、UI-06、UI-07、UI-08（单测级）、UI-09（显式 CE264）、UI-10（沿用 task-11）。
- **FAIL**：UI-03（ESC 后标题菜单视觉在、choice/输入没有恢复）、UI-11（真实 CE245->CE247 聊天分支后妹妹消失）。
- **待定/风险**：UI-09 的 DB face 兜底可能污染旁白；op121 的 601 槽范围；`clear_night_room_hud` 仍误删 picture120。
- 测试当前全绿（m12_ui_dialog 1、m12_ui_state 2、title_menu 5、room_base 2、m13_all_endings 14+2），但都没有覆盖上述两个真实失败路径。
