# M6 交接：主线贯通与可玩日常循环（收尾）

> 仓库：`E:\Study\github program\imoto life`
> 基线：本文件随 M6 收尾提交一起入仓；上一提交为 `f5aca1c M6: fix night-page teardown and prove the natural sleep loop`。以 `git log --oneline -3` 为准。
> 阅读顺序：本文件（M6 最新）→ `docs/HANDOFF_M4.md`（M4 音频 + M6 纵切片前史）→ `docs/HANDOFF_M3.md`（M3 收口）。
> 外部调研（`docs/gpt调研/`）不是项目文件，不要提交；与实测冲突时一律以本仓库代码/已验证文档为准。

## 1. 一句话现状

M0–M6 全部达到可交付状态：从标题画面 → 新游戏 → SampleMapA 卧室 HUD → 读书/钻研 → 卧室训练菜单 → 夜间菜单 → 就寝 CE329 → 次日 08:30 的完整可玩日常循环已经打通，并由真实 Game 集成测试逐段固定。多日循环已验证 **day1 → day4 08:30**；day4 早上被剧情强制送入 `CE253 地域貢献 → CE239 战斗`，当前构建的战斗命令菜单（CData type0 命令槽 / CE152 使用可能判定）尚未打通，因此 **day5 经真实 day loop 不可达**。这是已用通过测试锁定的明确边界，不是脚本死锁。

## 2. 怎么跑 / 怎么验

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
cargo test -q --workspace                 # 全绿
cargo build -q -p wolf-cli
./target/debug/wolf-cli.exe verify        # 预期 PASS 70, pending 0
# 单跑 M6 测试：
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m6_mainline
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m6_training_menu
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m6_night_menu
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m6_night_sleep
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m6_ev1_page0
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m6_ev1_page2
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m6_week_loop
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m6_battle_real
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test 'm6_multiday'
```

数据根：`work/all`（23 个 .wolf 全解包）；`work/full` 是瘦身测试根。游戏安装：`E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`（目录名确实拼错）。

## 3. 已交付能力与证据

| 能力 | 证据 / 测试 |
| --- | --- |
| 标题菜单 → 新游戏 → 卧室 HUD | `capture --boot` 92.1% 非黑、干净卧室；`m6_mainline.rs::the_title_menu_new_game_path_no_longer_leaves_title_art_over_the_room`；Map003 ev1 → `130 [-1,62,38,1,1]` → SampleMapA(62,38) |
| 开场声明 + 108 后台事件无死锁 | `m6_mainline.rs::new_game_story_runs_on_the_background_scheduler_without_deadlock`（900 帧、>50k 指令、peak≥9、warnings 0） |
| 读书/钻研可连续行动 | `m6_mainline.rs::clicking_the_reading_icon_in_the_bedroom_runs_the_first_playable_action`、`::the_bedroom_actions_can_be_played_one_after_another`；截图 work/out/m6_first_action.png、m6_second_action.png |
| 行动后存档往返 | `m6_mainline.rs::a_live_played_scene_survives_a_save_and_reload`（画面层/800 变量/playtime 一致） |
| 卧室自然训练菜单（无注入） | `m6_training_menu.rs` 2 测：点图片 122（comicon_10.png @195,475）→ 事件 245 三选一「力量锻炼/空挥/冥想」；18:30→20:00、体力 -20/-20/-10、对应属性 +2 |
| CE251 日间命令真正执行 | `m6_mainline.rs::the_bedroom_sleep_menu_choice_rolls_over_to_a_new_day`：hover+按住「学习会」→ 08:30→17:00、体力 -20、学习 +8、var48=2；随后晚餐→次日可重复 |
| 夜间菜单自然入口 | `m6_night_menu.rs` 2 测：hover 图片 120（imouto02.png @240,340）+ Decide → ev1 page0 写 var68=1 → ev4 page0 调 CE245；点「今天先休息吧」→ 是/算了 |
| 纯自然夜间就寝 → day3 08:30 | `m6_night_sleep.rs::the_natural_night_menu_sleep_rolls_over_to_day_three`；仅归一化 var295=0（首次晚餐 TIPS 门） |
| ev1 page0 白天/傍晚点击 | `m6_ev1_page0.rs::daytime_sister_sprite_click_writes_var68_and_opens_the_night_command_menu`；真实 sprite 命中框 x[240,389)×y[340,552)，var68 先断言 0 后观察 1 |
| ev1 page2 深夜点击 | `m6_ev1_page2.rs::hovering_sister_at_deep_night_writes_var63_and_opens_the_sleep_choice`；页门 var64==1，注入最小前置，点击框中心 (140,340) |
| CE328/CE329 页切换中止修复 | `crates/wolf-game/src/map_runtime.rs:328-348`：仅对 `WaitState::Child` 的整页失活做延迟；`docs/research/CE328_ABORT.md`；回归 m6_night_sleep |
| 多日循环 day1→day4 08:30（含中期存读） | `crates/wolf-game/tests/m6_multiday.rs::the_day_loop_repeats_through_day_four_with_a_midpoint_save_reload` |
| day4 强制战斗墙 | `crates/wolf-game/tests/m6_week_loop.rs::day_four_morning_is_forced_into_the_subjugation_battle_wall`：day4 08:30 **不画训练菜单**，唯一剧情「早上好」→ CE253 → CE239，时钟停 8:30、day 不滚、warnings 0 |
| 真实战斗线可达深度 | `crates/wolf-game/tests/m6_battle_real.rs::m6_battle_real_reaches_command_selection`：CE188→CE143/CE144→CE192→CE157→CE65；type10 row0=`[0,100,100,0,0,0,0,0,20,10,0,0,0,100,50,0]`（HP/maxHP 复制成功）；CE244 未到 |

### 3.1 关键源码位置

| 位置 | 内容 |
| --- | --- |
| `crates/wolf-game/src/game.rs` | Game/GameHost/frame、map_requests、teleport/transition/scroll/effect、tile_quads、text_quads |
| `crates/wolf-game/src/map_runtime.rs:328-348` | 页失活延迟修复（Child 调用中不拆页） |
| `crates/wolf-game/src/picture.rs` | Screen/画面层、Transition、tone/blend |
| `crates/wolf-vm/src/exec.rs` | opcode 分发；130/151/160-162/281 等 |
| `crates/wolf-vm/src/text.rs` | `\cself[n]` 先读字符串槽再回退数值（CE251 按钮文本修复） |
| `crates/wolf-game/src/fontmetrics.rs` | 真实 Lolita.ttf 字宽，文本图片右对齐 |
| `crates/wolf-cli/src/main.rs` | capture/dump/play、--key-frame/--pointer/--answer 等输入注入 |

## 4. 已确认边界（M6 剩余、非 bug）

1. **day5 经真实 day loop 不可达**：day4 08:30 没有可选非战斗行；CE251 直接进入剧情 → CE253 → CE239。`m6_week_loop.rs` 已锁定该墙。真正缺失的是 CE239 战斗线 `2000350=1`（等玩家命令）状态下，CE242/CE192 能基于 CData type0 的 8 个命令槽渲染出可交互命令菜单。
2. **战斗命令槽来源缺口**：CData type0 主人公ステータス 运行列 47..54 是 8 个命令槽；CE26（戦闘コマンド初期設定）会写 49..56，但全库静态调用者 0；CE77←CE13 也是 0。新游戏依赖随包 `Save/System.sav` 模板值；静态 DataBase 无每角色命令表。**真实命令不可选的精确断点已由 `m6_battle_commands.rs` 钉在 CE152 pc89..pc95，不是 pc66**：Data type0 只有 18 字段、没有 col20（発動アイテム要する?[消費]）→ 250 读到 0 → pc89 `>=0` 恒真 → pc91 读 CData type7 所持アイテム個数 row0=0 → pc93 `<=0` → pc95 把 CData type21 選択状態写 0（不可用）。数据前置修复=给 CData type7 row0 col0 播种 1，则 26/25/30/29 全部 `選択可能=1`（同一测试用两个 Game 做了因果对照）；这是数据前置，不是伪造菜单行。CE244 从 start_event(188) 结构上不可达（CE188 无 `210 CALL 500244`，只有 CE239/241/242/243 调），直接 start_event(239) 会在 pc13 停住，需真实地图上下文。详见 `docs/research/M6_BATTLE_REAL.md`、`M6_BATTLE_COMMANDS.md`、`M6_SCHEDULE.md` §3-4。
3. **明确不做的事**：不要为地图滚动新增 player/camera 跟随模型——Sys 104/105 是引擎输出的相机滚动值（HANDOFF_M3 §5 开头）；不要把 `130` 的 arg0 当地图 id（真实布局 target,x,y,map,options）。
4. 201 SetMoveRoute 无地图层、290 kind 10、CData (5,16,0) 越界读等旧诊断仍按既有透传，不影响本切片；但记录在 `docs/UNVERIFIED.md`。

## 5. 下一步（M6 之后）

M6 主线与日常循环已收口；下一里程碑工作流清单见 `docs/research/M7_BACKLOG.md`（本次新增，按依赖排序）。优先级建议：

1. **战斗命令菜单落地**：让 CE239 的 `2000350=1` 态真正渲染/接受命令；优先验证 CE26（或等价 CData/Data 初始化）能否产出合法命令，再补引擎侧 CE242/CE192 驱动。
2. **H 场景渲染纵切**：`docs/HSCENE.md` 已有 P0/P1/P2 研究（39 atlas / 89.4% 非黑）；补运行时渲染与输入。
3. **日程/任务系统**：把 TIME_SYSTEM 的日/星期/周末变量与剧情门控整理成数据化日程/任务视图。
4. **地图滚动/视差**：按 Sys 104/105 的真实语义接相机参数；不要造玩家居中模型。

## 6. 工程约束 / 协作陷阱

- 不要提交 3 个永久 scratch 文件：`crates/wolf-format/examples/opscan.rs`、`crates/wolf-format/tests/_scratch_cdata.rs`、`crates/wolf-format/tests/_scratch_event.rs`。临时 example/battle dbg 文件提交前删除。
- `docs/gpt调研/` 是用户资料，不是项目文件，不提交。
- 提交信息用 `git commit -F work/_commit_*.txt`；推送用 `git -c http.sslBackend=openssl push origin master`。
- `crates/wolf-vm/src/exec.rs` 实测为 LF；修改前用 python 统计 crlf/lf 并按实际换行写回（旧文档 CRLF 说明已过时）。
- 截图/临时日志写 `work/out/`；不跑 git clean；不杀用户/编辑器进程。
- 多 agent 共用工作树：每人只写自己声明的文件范围；集成前 Lead 检查 `git status` / `git diff`，并跑全量测试 + verify。
