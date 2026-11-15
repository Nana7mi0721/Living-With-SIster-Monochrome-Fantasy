# UI 与交互 Bug 清单（2026-11-04）

> **历史归档（2026-11-14）**：这些症状仍是 Godot 迁移的风险证据，但“在 Rust 中修复”的任务口径已失效。
> 当前护栏见 [`../../godot/LEGACY_RUST_AUDIT.md`](../../godot/LEGACY_RUST_AUDIT.md)。

> **来源**：用户 2026-11-04 真机反馈（本轮 `release/wolf-cli.exe`）。
> **状态**：2026-11-04 同一轮已修复并独立复核；逐项证据见 §5。本文件保留原始症状与验收口径，便于日后回归。
> **截图总表**：[`work/out/ui_bugs/contact_sheet.png`](../work/out/ui_bugs/contact_sheet.png)
> **原始截图目录**：`work/out/ui_bugs/`（按下列编号命名，均为用户提供原图副本）

## 0. 本轮用户原话要点

用户 m12796 报告：

1. 标题进入存档页面后无法使用鼠标滚轮翻页（动效上是左右平移）。
2. 点击鼠标右键无法退出存档页面回到标题界面（右键应相当于 ESC 的退出与返回）。
3. 按 ESC 退出存档页面后，标题页面的“开始咯”等三个选择项没了，同时也无法进行任何操作。
4. 点击“开始咯”进入游戏出现黄绿背景闪烁的问题依然存在；问是否可以把黄绿背景图删掉。
5. 游戏内的弹窗选项不能用鼠标选择；要求“光标放在哪个选项上面，就预选哪个选项”。
6. 仍有文本显示异常，出现对话框会消失的 bug。
7. 人名框与对话框不应该存在重叠。
8. 选项框仍存在不能完整显示文字的情况；部分情况下人名框消失（确实为角色说话的情况下）。
9. 行动力数值恢复与显示异常。
10. 夜晚与妹妹互动一次后，妹妹不再出现在画面中。
11. 还有其他各种 UI 显示 bug 与图层优先级 bug。

## 1. 截图对照

| 编号 | 文件 | 内容 |
| --- | --- | --- |
| 01 | `01_load_screen_1023x576.png` | 从标题进入的存档/读档画面；四槽 + DLC 说明框，右侧有标题美术背景 |
| 02 | `02_title_after_esc_no_menu_1020x571.png` | ESC 退出存档后回到标题；只有标题美术/Logo，无“开始咯/继续咯/还是算了”三行菜单 |
| 03 | `03_choice_raw_control_codes_515x169.png` | 选项框文字带原始控制码 `你是……\cdb[0:12:0]\cdb[0:12:1]的哥哥。`，未正确解析/截断 |
| 04 | `04_dialog_cafe_nameplate_1020x578.png` | 咖啡店对话；底部消息框 + 左上人名框 |
| 05 | `05_nameplate_overlap_crop_394x203.png` | 人名框与消息框重叠的局部放大 |
| 06 | `06_choice_text_truncated_crop_535x204.png` | 选项框文字被截断：`(确实，考虑到妹妹，我可不能乱来`（缺右括号/尾部） |
| 07 | `07_room_1930_policy_panels_1027x580.png` | 19:30 房间；左上“方针/公会评价”面板与 HUD 的尺寸/图层异常 |

## 2. 待修清单

### P0 — 输入与存读档（阻塞基本操作）

| ID | 现象 | 证据 | 初步定位 | 验收标准 |
| --- | --- | --- | --- | --- |
| UI-01 | 存档页滚轮无效；期望滚轮左右翻页（与动效左右平移一致） | 01，用户原话 1 | `crates/wolf-cli/src/play.rs` 的 `MouseWheel` 只映射 UP/DOWN；存档页的翻页可能走横向光标/op121 或特殊键 | 在存档页滚轮能移动到上/下一槽或上一/下一页，且不误触确认 |
| UI-02 | 存档页鼠标右键不能退出；期望等同 ESC | 01，用户原话 2 | `play.rs` 右键只写 `mouse_buttons & 2`，没有注入 `key::CANCEL` | 右键在存档页触发与 ESC 相同返回路径，回到标题且无残留状态 |
| UI-03 | ESC 退出存档后标题菜单丢失且无法操作 | 02，用户原话 3 | `stop_title_menu()` / 存档返回标题时清了 picture 3/4/5，但未重建 title menu / 未恢复输入状态机 | 退出存档后标题三行菜单重新出现；方向键/滚轮/鼠标点击都能操作 |
| UI-04 | 游戏内弹窗选项不支持鼠标悬停预选 | 用户原话 5 | `handle_choice_input` 的 hover 命中只覆盖标题菜单 special case；普通 102 弹窗可能只靠键盘 | 指针移到任一选项即更新选中项；左键点击确认当前项；移开不改变已选项 |

### P1 — 标题/黄绿闪/消息框

| ID | 现象 | 证据 | 初步定位 | 验收标准 |
| --- | --- | --- | --- | --- |
| UI-05 | 点“开始咯”进入游戏仍出现黄绿背景闪烁 | 用户原话 4 | 不是单一“黄绿图”可删：更像是标题/地图切换时旧层擦除、底色或地图 Chip 图层短暂露出。需要逐帧复现“标题→开始咯→地图/场景”链路 | 逐帧统计黄/绿像素为 0；不能靠删除全局素材解决，避免影响其他场景 |
| UI-06 | 对话框会消失 / 文本显示异常 | 用户原话 6 | 消息帧的 `frames`/`auto_advance` 或消息队列在当前 step 被过早清除；也可能与刚才的消息绘制/名牌改动有关 | 一段对话在玩家确认前持续显示；换行/内联控制码完整；不闪退、不提前消失 |
| UI-07 | 人名框与对话框重叠 | 05/04，用户原话 7 | `message_nameplate()` 的 y 计算（`vars.y - box_h - 2`）贴着消息框上沿，且名牌底/文字层顺序可能压进正文区 | 名牌整体位于消息框上沿外侧，留出间距；不与正文/边框重叠 |
| UI-08 | 选项框文字不完整、原始控制码未解析 | 03/06，用户原话 8 | `choice.rs` 的可见字形测量只处理部分 `\f[n]`；缺少 `\cdb[...]` 等颜色/控制码解析；宽度仍可能按整字符估算 | 选项文字按真实字形宽度排版；`\cdb[...]` 等标记被解析或安全剥离；无截断 |

### P1 — 对话身份与游戏状态

| ID | 现象 | 证据 | 初步定位 | 验收标准 |
| --- | --- | --- | --- | --- |
| UI-09 | 角色明确说话时人名框有时消失 | 用户原话 8 | 名牌依赖 CE264 第二字符串；某些消息走普通 101 或不同窗口包装，没有 `speaker`；不能用 face 文件名冒充名字 | 角色说话必有正确名字；旁白无名牌；名字来自包装参数或正确 DB 行名 |
| UI-10 | 行动力数值恢复/显示异常 | 用户原话 9；04/07 顶部 HUD | 行动力/精力/性欲等状态在睡眠或事件后未正确回填；HUD 绘制可能读旧值 | 睡觉/跨天后数值按原版恢复；HUD 与内部变量一致 |
| UI-11 | 夜晚与妹妹互动一次后妹妹从画面消失 | 用户原话 10 | 夜晚人物/立绘 picture 槽在事件结束后被擦除或未重建；可能与 CE332/CE333 的清理范围有关 | 互动一次后妹妹仍在画面中；场景切换/重绘后角色层保持 |

### P2 — 其他 UI 与图层

| ID | 现象 | 证据 | 初步定位 | 验收标准 |
| --- | --- | --- | --- | --- |
| UI-12 | 19:30 房间“方针/公会评价”面板尺寸/图层异常 | 07，用户原话 11 | 窗口/九宫格尺寸或图片槽编号/绘制顺序；可能与房间 HUD 清屏逻辑交叉 | 面板尺寸与原版一致，不遮挡关键 HUD；层级正确 |
| UI-13 | 其他 UI 显示与图层优先级 bug（持续补充） | 用户原话 11 | 需要建立“选择弹窗 > 消息框 > 场景 UI > 地图/背景”的统一层级表 | 每个新增 UI bug 有截图、复现步骤、修复证据 |

## 3. 修复顺序建议

1. **先做 UI-01/02/03/04**：输入路由和返回状态机，风险低、收益最高。
2. **再做 UI-05/06/07/08**：黄绿闪、消息消失、名牌几何、选项排版，需要逐帧截图回归。
3. **然后 UI-09/10/11**：身份与游戏状态，需要存档/事件夹具复现。
4. **最后 UI-12/13**：全量 UI/图层审计，建立统一层优先级与截图基线。

## 4. 记录约束

- 本清单只记录**用户真机观察到的症状**，未复现前不要声称已修。
- 所有修复必须附：复现命令/存档、修复前后截图、相关单测/回归测试。
- 黄绿背景**不要直接删素材**；先用逐帧截图定位是哪个 picture/地图图层/底色透出，否则会引入其他场景素材缺失。

## 5. 修复与验收记录（2026-11-04）

| ID | 修复 | 证据/测试 | 结论 |
| --- | --- | --- | --- |
| UI-01 | `play.rs` 增加 `wheel_dir`/`wheel_notch()`，滚轮有符号 notch 经 `Game.wheel_dir` 交给 `GameHost::key_code(mask 771)`；`engine_state(10)` 同步 | `crates/wolf-cli/tests/m12_ui_input.rs` 8 passed；`work/out/reviews/review_input_fix.md` | PASS |
| UI-02 | 右键按下注入 `key::CANCEL`（保留 raw mouse bit 给 engine_state(8)） | `m12_ui_input` 8 passed；`review_input_fix.md` | PASS |
| UI-03 | `title_menu_choice_active` + 三项 parked Choice 重建（Sys3=512/Sys4=1000/Sys20），选择直接执行新游戏/读档/退出，不重播 CE42 | `review_ui03_ui11_final_ui03_esc.png`；game.rs 单测 + `review_ui03_ui11_final.md` | PASS |
| UI-04 | 非 parked `handle_choice_input` 用 `choice_row_at` 更新 `choice_cursor`，空白点击不确认 | `work/out/m12_ui04_hover2.png`；`m12_ui_dialog`；`review_dialog_fix.md` | PASS |
| UI-05 | 切图时旧标题负层保留 4 帧（generation 校验），不立即清 `TITLE_BACKDROP`；黄色/绿色像素统计为 0 | 913–917 帧 capture；`work/out/m12_render_ui05_contact.png`；`review_render_fix.md` | PASS |
| UI-06 | 消息确认改为 fresh-press 边沿，长按不连清；消息窗口保持 | 600 帧 hold 截图；`m12_ui_dialog`；`review_dialog_fix.md` | PASS |
| UI-07 | 名牌改锚到消息框 `FRAME_PICTURE` 上沿减高度+8px，不再用 Sys2 文本原点 | `work/out/m12_ui07_bottom_2x.png`；`m12_ui07_ce281.png`；`review_dialog_fix.md` | PASS |
| UI-08 | `styled_items` 解析 `\cdb[...]` 等变量文本，选项按可见字形宽度排版/换行 | `m12_ui_dialog`；`work/out/reviews/m12_ui_layers_choice_over_message.png` | PASS |
| UI-09 | 名牌只认 CE264 显式名字，去掉 Sys0 DB 兜底；旁白保持无名牌 | `m12_ui07_ce281.png`；`review_ui03_ui11_final.md` | PASS |
| UI-10 | 载入存档时白名单恢复 HUD 事件 216/217（不误启 506） | `work/out/reviews/ui10_sleep_save01_fixed2.png`；`m12_ui_state` 2 passed；`review_ui10_fix.md` | PASS |
| UI-11 | 房间基础立绘快照改为房间会话级；CE218 -1 临时擦除不消耗恢复；保留 picture 120 | `review_ui03_ui11_final_ui11_chat.png` / `before_after`；`review_ui03_ui11_final.md` | PASS |
| UI-12 | 核对零售 Save7 同构，`Picture/policy.png` 530x84 在 x=40,y=50；属原版排版非 Rust 回归 | `work/out/m12_render_ui12_compare.png`；`m12_render_ui12_policy` 2 passed | PASS（非回归） |
| UI-13 | `window::LayerBand` 贯通 host 合成，`Renderer::draw_layers` 按层独立 submit；Map0<ScenePicture10<SceneText11<MessageFrame20<MessageText21<Nameplate22<ChoicePanel30<ChoiceText31<Cursor40 | `m12_ui_layers` 13 passed、`m12_render_layer_priority` 6 passed、`offscreen::multiple_text_layers_keep_their_own_glyphs`；`review_render_layers_final.md` | PASS |

- **RF-01（P2，已修）**：指针停在标题菜单某行时，hover 每帧覆盖 UP/DOWN；改为仅指针移动/提示新开时应用 hover，并加 `a_resting_pointer_does_not_override_title_keyboard_navigation` 单测。
- **非阻塞残余**：RF-02 仅注释措辞；RF-03 缺“离开房间清 snapshot”的直接回归测试；UI-12 的文字挤叠属于原版模板排版，另立 typography 任务。
- 修复文件：`crates/wolf-cli/src/play.rs`、`crates/wolf-game/src/{game,message,choice,textpic,db,picture,atlas,window,auto_events,savegame}.rs`、`crates/wolf-render/src/{renderer,text,lib}.rs`；新增回归 `m12_ui_input/m12_ui_layers/m12_ui_dialog/m12_ui_state/m12_render_layer_priority/m12_render_ui12_policy`。
- 黄绿闪未删除任何素材；问题定位为标题负层清理时序，用保留旧层 + generation 防误删解决。
- **最终集成（2026-11-04）**：`WOLF_GAME_DIR=$PWD/work/retail_backup WOLF_DATA_ROOT=$PWD/work/all cargo test -q --workspace --no-fail-fast` = 153 test-result blocks / 957 passed / 0 failed / 14 ignored；`release/wolf-cli.exe` 12,256,768 bytes，`wolf-package check --package release` = `PACKAGE OK: 4067 files verified, 813892643 bytes`，`extra package files: none`。
