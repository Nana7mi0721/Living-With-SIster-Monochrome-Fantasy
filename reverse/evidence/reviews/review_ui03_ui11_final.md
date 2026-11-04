# task-19 独立复核：task-18 UI-03 / UI-11 final fix

- 日期：2026-11-04
- Reviewer：review-game-events（独立复核）
- 审查范围：`crates/wolf-game/src/game.rs` 及同文件测试（task-18 fix-dialog-core 的 UI-03/UI-11 修改）；产品代码只读，未修改。
- 方法：源码逐行审查 + 指定 cargo 测试 + wolf-cli 真实 capture（SaveData01/SaveData02）；截图全部在 `work/out/reviews/`。
- 总体结论：**PASS（可合入）**。任务指定的 UI-03 ESC 重建、UI-11 真实 CE245→CE247 chat 恢复、picture120 保留、message_speaker 显式名均成立；发现 1 条 P2 级残余交互风险（指针悬停覆盖键盘选择），不作为本次合入阻塞项。

## 1. 结论矩阵

| # | 验收项 | 结论 | 关键证据 |
|---|---|---|---|
| 1 | UI-03：ESC 退出存档/读档窗口后重建 3 项 parked Choice | **PASS** | `review_ui03_ui11_final_ui03_esc.png`；5 pictures、choice HELD 3 options ["1","2","3"]、cursor 1、anchor (512,1000)、Sys20=1 |
| 2 | UI-03：不重播 CE42 | **PASS（代码级）** | `resume_title_menu_if_ready()` 只调 `start_title_menu()`（event43）和 `open_title_menu_choice()`；无 `start_event(42)` |
| 3 | UI-03：title choice 不泄漏进 gameplay | **PASS** | `switch_map` game.rs:1391-1395、`start_map_event_by_name` 1625-1629、load 分支 2290-2295 清 flags；测试 `a_game_map_does_not_resume_the_title_menu` game.rs:4142-4157 PASS |
| 4 | UI-03：ESC/CANCEL 语义 | **PASS** | `handle_choice_input` game.rs:2205-2212：title active 时 CANCEL 只返回、不关闭/不回复；ESC 由存档/读档窗口自身处理 |
| 5 | UI-03：鼠标 hover/click | **PASS** | 测试 `clicking_a_rebuilt_title_row_runs_that_row` game.rs:4073-4109、`the_rebuilt_title_rows_execute_new_game_load_and_quit` 4112-4139 PASS；行 0/1/2 分别 MapRequest(-1,3,60,34)/CE14/quit |
| 6 | UI-03：DOWN+OK 路径 | **PASS（无指针场景）；有 P2 残余** | 测试 `the_title_menu_resumes_after_the_save_window_even_if_event_42_ended` game.rs:4034-4070 验证 DOWN 更新 cursor/Sys20；无指针 capture `review_ui03_ui11_final_ui03_keyboard_clean.png` 显示 menu1_a（行0）被选中、choice 被消费。若鼠标停在行1，UP/DOWN 会被 hover 每帧覆盖，见 §5 RF-01 |
| 7 | UI-11：room-base 快照为 room-session 生命周期，CE218 -1 不消费 pending | **PASS** | `restore_room_base_if_ready` game.rs:1548-1603；只有 `all_missing` 恢复成功后才在 1602 清 pending；临时 pose 时 1590-1594 保留 pending |
| 8 | UI-11：真实 CE245→CE247 chat 后保留 90/91/92/93/98 | **PASS** | capture 终态 67 live；90/91/92/93/98 均在 (600,80)；截图 `review_ui03_ui11_final_ui11_chat.png` |
| 9 | UI-11：保留 picture120 妹妹 | **PASS（独立路径）** | SaveData01 基座无 120；SaveData02 + CE332 capture：58 loaded → 57 live，`pic 120 Picture/imouto04.png (388,368)` 保留；截图 `review_ui03_ui11_final_night_clear.png`；测试 game.rs:4281-4303 PASS |
| 10 | UI-11：CE245 父菜单 / CE247 top menu 恢复路径 | **PASS** | 测试 `erased_room_base_pictures_are_restored_when_the_chat_menu_returns` game.rs:4160-4188、`room_base_pictures_are_restored_when_the_parent_menu_returns` 4192-4226、`a_temporary_room_pose_is_never_overwritten_by_the_base_restore` 4229-4278 PASS；真实 chat 路径亦 PASS |
| 11 | UI-11：离开房间/新场景清 snapshot | **PASS（代码级；无独立 capture）** | `switch_map` game.rs:1391-1395、`start_map_event_by_name` 1625-1629、load 分支 2290-2295 均调 `clear_room_base_snapshot()`；建议补一条回归测试（见 §6） |
| 12 | P2：clear_night_room_hud 只删 108..=111 与 118..=119，保留 120 | **PASS（按实现范围）** | game.rs:770-775；`start_event(332)` 778-782；`GameHost::common_event_call` 2811-2821；测试 4281-4303；直接 capture SaveData02+CE332 保留 pic120。注：任务描述里“preserving 120/118/119”与“removes 118..=119”自相矛盾，见 §5 RF-02 |
| 13 | P2：message_speaker 只用 CE264 显式名，旁白无名牌 | **PASS** | game.rs:1828-1842 只读 `Message.speaker`，无 Sys0/DB fallback；测试 3441-3466、3468-3491 PASS；CE281 capture 看到名牌“妹妹”（`review_ui03_ui11_final_message_name.png`） |

## 2. 测试结果（全部 exit 0）

执行命令：

```bash
export TEMP="$PWD/work/tmp" TMP="$PWD/work/tmp"
cargo test -q -p wolf-game --lib
cargo test -q -p wolf-game --test m12_ui_dialog
cargo test -q -p wolf-game --test m12_ui_state
cargo test -q -p wolf-game --test m13_all_endings
cargo test -q -p wolf-game --test m13_random_events
```

| Suite | 结果 |
|---|---|
| `--lib` | 274 passed, 0 failed, 0 ignored |
| `--test m12_ui_dialog` | 1 passed, 0 failed |
| `--test m12_ui_state` | 2 passed, 0 failed |
| `--test m13_all_endings` | 14 passed, 0 failed, 2 ignored |
| `--test m13_random_events` | 4 passed, 0 failed, 1 ignored |

日志：`work/tmp/task19_tests.log`。同文件测试重点：title 5 passed（game.rs:4034/4073/4112/4142 及辅助），room_base 2 passed（game.rs:4160/4192/4229/4281 所属测试组）。

## 3. UI-03 详细证据

### 3.1 ESC 后重建 parked Choice（PASS）

复现命令：

```bash
./target/debug/wolf-cli.exe --root release/data capture --menu-only --frames 260 \
  --pointer 10,460 --press-frame 60 --press-hold 3 \
  --key-frame 200:11 \
  --out work/out/reviews/review_ui03_ui11_final_ui03_esc.png
```

终态（`work/tmp/ui03_esc.log`）：

- `pictures: 5 live`：-100 `Kagura/bg.png`、-2 `SystemGraphic/TitleGraphic.jpg`、3 `Picture/menu1_b.png`、4 `Picture/menu2_a.png`、5 `Picture/menu3_b.png`（行1高亮）。
- `choice: HELD on prompt with 3 option(s) ["1", "2", "3"], cursor 1 at anchor (512, 1000)`；`anchor is off screen, engine draws nothing`。
- `sys pads: Sys20 1`；stats 11403 instruction / 118 picture / 1 sound / 558 db / 0 message / 1 choice。
- 截图：`work/out/reviews/review_ui03_ui11_final_ui03_esc.png`；修复前/后对比：`work/out/reviews/review_ui03_ui11_final_ui03_before_after.png`（BEFORE 为 F2 的 `review_f2_title_after_esc3.png`，当时 choice none）。

代码链路：

- `resume_title_menu_if_ready` game.rs:888-899：`title_menu_suspended && title_screen_active && !WindowBase` → `start_title_menu()`，若 `choices.is_none()` 再 `open_title_menu_choice()`。
- `start_title_menu` game.rs:801-814：只确保 event43 存在；**不重启 event42**。
- `open_title_menu_choice` game.rs:908-927：写 Sys3=512、Sys4=1000，Sys20 取 0..2，构造 `Choices { options: ["1","2","3"] }`，置 `title_menu_choice_active=true`，同步三行高亮。
- `choose_title_menu_row` game.rs:935-958：行0 `start_engine_events()` + `MapRequest{target:-1,map:3,x:60,y:34,options:1}`；行1 `start_title_load_screen()`（CE14 self args 1,-1）；行2 `quit_requested=true`。
- `Game::frame` game.rs:2232-2239：仅 `title_screen_active && has_file_containing("SystemFile/WindowBase")` 时 `stop_title_menu()`；`resume_title_menu_if_ready()` 在 2361 行、save_requests 之后执行。
- `switch_map` game.rs:1391-1395、`start_map_event_by_name` 1625-1629、load 分支 2290-2295：均清 `title_menu_choice_active`/suspended/room snapshot，避免 title choice 泄漏到 gameplay。

### 3.2 鼠标与键盘

- 鼠标：测试 `clicking_a_rebuilt_title_row_runs_that_row` game.rs:4073-4109 覆盖行1（`mouse=[10,460]`）→ event14 启动；行2（`[10,510]`）→ quit_requested。PASS。
- 键盘：测试 `the_title_menu_resumes_after_the_save_window_even_if_event_42_ended` game.rs:4034-4070 覆盖 DOWN：cursor 1、Sys20=1；`the_rebuilt_title_rows_execute_new_game_load_and_quit` 4112-4139 覆盖 0/1/2 动作。PASS。
- 无指针 capture：`review_ui03_ui11_final_ui03_keyboard_clean.png`：5 pictures（menu1_a/menu2_b/menu3_b），Sys20=0，choice none（说明重建 Choice 已被消费），stats 9614/118/1/468。支持“无指针时键盘可选行0”。
- 指针悬停时的键盘问题见 §5 RF-01。

### 3.3 ESC/CANCEL

- `handle_choice_input` game.rs:2205-2212：title active 时 CANCEL 直接 return，不产生 `ChoiceReply::Cancel`（没有 coroutine 可接），保持三行菜单；符合“重建菜单不被 CANCEL 撕掉”的语义。
- ESC 关闭存档/读档窗口由 CE14/事件自身处理；窗口消失后下一帧 `resume_title_menu_if_ready` 重建，见 3.1 capture。PASS。

## 4. UI-11 详细证据

### 4.1 真实 CE245→CE247 chat 路径（PASS）

复现命令：

```bash
./target/debug/wolf-cli.exe --root release/data capture \
  --load-save work/retail_backup/Save/SaveData01.sav \
  --event 245 --frames 600 \
  --pointer 510,250 --press-frame 60 --press-hold 5 \
  --answer 0 --answer-delay 40 --press-every 25:10 \
  --out work/out/reviews/review_ui03_ui11_final_ui11_chat.png
```

终态（`work/tmp/ui03_ui11_chat.log`）：

- `loaded: 66 picture(s)`；`pictures: 67 live`。
- `pic 90 Picture/stand_imouto_paja2.png (600,80)`、`pic 91 stand_blow_a`、`pic 92 stand_eye_a`、`pic 93 stand_mouth_a`、`pic 98 paja03` 全部保留。
- `choice: none open`；`auto: 98 tracked ... 2 running [216, 217]`（HUD/时钟事件正常）。
- stats 65714 instruction / 9226 picture / 3 sound / 4 message / 2 choice。
- 截图：`work/out/reviews/review_ui03_ui11_final_ui11_chat.png`；BEFORE/AFTER 对比：`work/out/reviews/review_ui03_ui11_final_ui11_before_after.png`（BEFORE `review_f2_ui11_ce245_comm.png` 显示菜单+妹妹；AFTER 同上房间 22:20，妹妹仍在画面）。

代码链路：

- `capture_room_base_pictures` game.rs:1474-1485：取 screen 90..=98 clone，置 captured=true、pending=false。
- `refresh_room_base_pictures` game.rs:1503-1539：CE247 live 且 top menu（options.len()==4）时补抓；CE247 不 live 时，只有 parent_menu_live && pending 才保留快照，否则仅在 `!room_scene_live` 时清空；快照属于 room session，不随 CE247 一次分支结束而丢弃。
- `restore_room_base_if_ready` game.rs:1548-1603：`menu_waiting` 覆盖 CE247 top4、CE245 parent choice、CE245 图片按钮 1200..=1213、room map 且 choices none；`all_missing` 才 insert；只有成功 restore 才在 1602 清 pending，临时 pose 在 1590-1594 保留 pending。
- `GameHost::common_event_call` game.rs:2784-2807：target247 首次调用抓 90..=98；target218 `args.first()==-1` 且有 base 时置 pending。
- `Game::frame` game.rs:2242 `refresh_room_base_pictures()`、2281 `restore_room_base_if_ready()`（VM step 后、渲染前）。

### 4.2 picture120 保留（PASS，独立路径）

- `clear_night_room_hud` game.rs:770-775：`remove_range(1600,1621)`、`remove_range(108,111)`、`remove_range(118,119)`、`clear_room_base_snapshot()`；`start_event(332)` game.rs:778-782 调用。
- `GameHost::common_event_call` game.rs:2811-2821 对 target332 做同样精确清理，并清 room snapshot。
- 直接 capture：`./target/debug/wolf-cli.exe --root release/data capture --load-save work/retail_backup/Save/SaveData02.sav --event 332 --frames 2 --out work/out/reviews/review_ui03_ui11_final_night_clear.png`；输出 `loaded: 58 picture(s)` → `pictures: 57 live`，`pic 120 Picture/imouto04.png (388,368)` 保留，108/109/110/111/118/119 不再出现（`work/tmp/ui03_night_clear.log`）。
- 测试 `the_night_chain_drops_the_room_hud_but_keeps_the_sister_picture` game.rs:4281-4303 PASS。
- 说明：SaveData01 基座没有 picture120（只有 90/91/92/93/98 睡衣立绘），所以真实 chat capture 不出现 120 是数据事实；task 要求“90..98+120”时，120 由 CE332/SaveData02 独立路径验证，建议补一条 SaveData 基座同时含 90+120 的回归夹具。

### 4.3 离开房间/新场景清 snapshot（PASS，代码级）

- `switch_map` game.rs:1391-1395：`stop_title_menu`、title flags、`clear_room_base_snapshot()`。
- `start_map_event_by_name` game.rs:1625-1629：新地图/新游戏同样清 snapshot。
- load 分支 game.rs:2290-2295：读档清 snapshot，避免把上一局房间立绘带到新档。
- 现有测试没有直接覆盖“离开房间后 snapshot 为空”；建议增加（见 §6）。

## 5. 新发现/残余风险

### RF-01（P2）指针悬停每帧覆盖键盘选择

- 位置：`crates/wolf-game/src/game.rs:2118-2128`（`handle_choice_input` parked 分支）。
- 现象：只要鼠标停在某一行上，每帧都会执行 `cursor = row; set_sys(20,row)`；随后即使按下 UP/DOWN 改了 cursor，下一帧又被 hover 拉回。表现为“鼠标停在行1时，按 UP 再 OK 仍然打开行1”，方向键不可靠。
- 复现：`./target/debug/wolf-cli.exe --root release/data capture --menu-only --frames 340 --pointer 10,460 --press-frame 60 --press-hold 3 --key-frame 200:11 --key-frame 220:8 --key-frame 240:10 --out work/out/reviews/review_ui03_ui11_final_ui03_keyboard.png`。终态 6 pictures 含 `SystemFile/WindowBase_amania.png`、coro event14/128/65、choice none、stats 35995/1245/1/2471；说明 UP+OK 仍进入 Load（行1），不是行0。
- 对照：无指针路径 `review_ui03_ui11_final_ui03_keyboard_clean.png` 可选中行0，证明不是按键本身失效，而是 hover 覆盖。
- 最小修改建议：parked 分支只在 `self.mouse != self.previous_mouse`（指针实际移动）时应用 hover；或同帧有 UP/DOWN/OK/CANCEL pressed 时跳过 hover。`handle_title_menu_input` game.rs:2074-2092 的初始标题路径也有同样结构，建议一起修。
- 建议测试：在 game.rs title 测试中加“mouse 固定在行1 + key UP + OK”断言打开行0/MapRequest，而不是 CE14；或捕获 `review_ui03_ui11_final_ui03_keyboard.png` 作为回归夹具。

### RF-02（P2 文档/验收措辞不一致，非行为阻塞）

- `GameHost::common_event_call` game.rs:2812-2813 注释写“keep picture 120 (the sister) and the two separate speaker slots 118/119”，但 2816 行实际 `remove_range(118,119)`；`clear_night_room_hud` 的注释 766-769 也把 108-111 描述成 room message window（实际数据是 sub_button02-05 菜单按钮）。
- 任务描述 P2 同时写“removes only 108..=111 and 118..=119”与“preserving 120/118/119”，两者矛盾。本次按“删除 108..=111、118..=119，保留 120”的实现验收 PASS；若产品要求保留 118/119，则需要改代码并调整测试 4281-4303。建议 Lead 明确口径并修正注释，避免后续 review 误判。

### RF-03（P3）缺少“离开房间清 snapshot”的直接回归测试

- 代码路径明确（§4.3），但现有测试只覆盖 restore/capture，不覆盖 `switch_map`/load 后 `room_base_pictures` 为空。建议加一条 game.rs 单测：capture 后调用 `switch_map` 或 `start_map_event_by_name`，断言 `room_base_pictures.is_empty() && !room_base_captured && !room_restore_pending`。

## 6. 回归与风险结论

- task-18 的 UI-03 核心修复成立：ESC 后三行 parked Choice 确实重建，鼠标/键盘在无指针遮挡时可用，CE42 未重播，title flag 在 map/load 路径清理。
- task-18 的 UI-11 核心修复成立：真实 CE245→CE247 chat 路径保留 90/91/92/93/98；pending 不再被临时 pose 提前消费；CE332 保留 120；parent/top menu 与临时 pose 单测通过。
- 唯一需要在合入后跟进的是 **RF-01（P2）指针 hover 覆盖键盘**；建议作为后续 UI-03 小修，不影响本次 P1 结论。
- 没有发现 product regression；所有指定测试绿。工作树产品代码未由本 review 修改。

## 7. 复现资产清单

- UI-03 ESC 后：`work/out/reviews/review_ui03_ui11_final_ui03_esc.png`、`review_ui03_ui11_final_ui03_before_after.png`
- UI-03 键盘：`review_ui03_ui11_final_ui03_keyboard.png`（指针覆盖问题）、`review_ui03_ui11_final_ui03_keyboard_clean.png`（无指针正常）
- UI-11 真实路径：`review_ui03_ui11_final_ui11_chat.png`、`review_ui03_ui11_final_ui11_before_after.png`
- UI-11 picture120：`review_ui03_ui11_final_night_clear.png`
- message_speaker：`review_ui03_ui11_final_message_name.png`
- 测试日志：`work/tmp/task19_tests.log`；各 capture 日志在 `work/tmp/ui03_*.log`、`work/tmp/ui03_ui11_chat.log`、`work/tmp/ui03_night_clear.log`。

## 8. 方法与存档完整性

- 本 review 只读 `crates/wolf-game/src/game.rs`，未修改产品代码、测试、release、manifest、存档。
- 所有 capture 使用零售存档副本 `work/retail_backup/Save/SaveData01.sav` / `SaveData02.sav`；未写回零售存档。
- 复核结束时 `release/data/save/system.sav` md5 = `564f3570677e10dfef2eae2f10aa69e8`，与 `work/out/release_save_backup/system.sav` 一致（363523 bytes）。
