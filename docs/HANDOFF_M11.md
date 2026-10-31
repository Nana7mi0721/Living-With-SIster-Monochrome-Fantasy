# M11 交接：战斗默认驱动、真实菜单/行走、H 深链实证

> 仓库：`E:\Study\github program\imoto life`
> 阅读顺序：本文件（M11 最新）→ `docs/README.md`（文档总索引）→ `PLAN.md`（当前路线图）；M10 及更早已标历史归档。
> 基线：M11 第一批/round 1 的集成交付在 `22de104` 与 `5ef6b1f`；本文件记录 round 2（task-89..96）与本批集成提交。数据根 `work/all`，零售存档 `work/retail_backup/Save`。
> 本批结束后按用户要求暂停推进 M11。

## 1. 一句话现状

M0–M10 保持可交付；M11 已完成三批：

- **M11 第一批（task-79..84，见 HANDOFF_M10 §8）**：战斗确定性、H 真实 gate 矩阵、8 向移动/NPC contact、锚点 5/6 几何、多成员命令序列、音频 loop 档案。
- **M11 round 1（task-85..88，见 HANDOFF_M10 §9）**：CE157 确认采集/命令队列桥、var304 真实可达与 var326 书店 gate、窗口 CN patch 右缘 data-only、音频 loop 回归。
- **M11 round 2（task-89..96，本文件 §2）**：
  - **默认武装的真实战斗驱动**：自然 day4 的 CE253→CE239→CE242 无需测试开关即可跑通并结算；显式 opt-out 保留原版 stall。
  - **真实战斗菜单**：CE157 cursor 前提被推翻——没有 VM bug，物理 cursor 是 CData type18 row73；两次确认注册两行、2 成员 fixture 注册 15 行，引擎队列可驱动 4 个不同 CE242 body。
  - **真实 CE328 → CE402/CE410 深链已实证**：只 `start_event(328)` 即经 pc986 `op210[500402]` 进 CE402（pc0→511），CE402 的 10 个 `op210[500410]` 拉起 CE410（pc0→247），warnings=0。之前“真实存档无路可达”的结论被 calendar-consistent fixture 的真实控制流推翻。
  - **NPC 交互与持续行走**：ActionKey 触发页面 + 按住方向每 8 帧连续走一格。
  - **视觉回归基线**：5 个固定场景的像素/quad/text 计数固定；战斗场景因默认驱动改为 600 帧活跃 HUD。

本批唯一生产代码改动：`crates/wolf-game/src/battle_wiring.rs`（默认 auto-arm）、`crates/wolf-game/src/game.rs`（输入/行走接入 + `set_battle_auto_arm`）、`crates/wolf-game/src/map_runtime.rs`（`start_trigger_page`）。其余为测试与文档。

## 2. M11 round 2 交付与证据（task-89..96）

| 任务 | 交付 / 证据 | 结论 |
| --- | --- | --- |
| task-89 CE157 cursor | `crates/wolf-game/tests/m11_ce157_cursor.rs`（2 测）；`docs/research/M11_CE157_CURSOR.md`；exec.rs 无 diff | **前提为假，无 VM bug**。物理 cursor 是 CData type18 row73 col0（旧探针读 type20 row73 恒 0）。真实 CE188→CE190→CE191→CE192→CE157 连续 7 次确认写 7 行，input_pos 1→8。残余：>1 真实 action code 需要 2 成员 DATA fixture。 |
| task-90 H 书店线 | `crates/wolf-game/tests/m11_hscene_bookstore.rs`（5 测）；`docs/research/M11_HSCENE_BOOKSTORE.md` | var276/277/326/304 均可从 SaveData01 真实输入获得（CE308 town option→CE314 pc1188/1233；CE301 pc6320/6365 学 var326=2；CE290 option5→CE328 var304=1）；但 task-96 前仍以 CE328 pc741 `op172` 早退为阻塞。 |
| task-91 NPC/持续行走 | `crates/wolf-game/src/map_runtime.rs` +56、`game.rs` +92；`tests/m11_npc_interaction.rs`（5 测）；`docs/research/M11_NPC_INTERACTION.md` | `WALK_STEP_FRAMES=8`：方向按下立即走一格、每 8 帧重复；换向/释放重置；被墙挡住仍转向。`interact_action_key` 在 fresh OK 边沿用 `try_trigger`+`start_trigger_page`；`advance_player_input` 在 VM step 前运行；`switch_map` 重置 cadence。 |
| task-92 视觉回归 | `crates/wolf-cli/tests/m11_visual_regression.rs`（5 测）；`docs/research/M11_VISUAL_REGRESSION.md` | 5 个固定场景逐字段固定：title 636f/91.3% nonblack/quads19082/pics2；map_idle 1200f/92.3%/27661/49；day_command event251 120f/93.6%/4/9；battle event253 600f/88.6%/quads961/pics52（见 §4 重基线）；h_overlay SaveData08 60f/89.4%/quads258/pics86。每测解码 PNG 校验 1024x576 与 nonblack。 |
| task-93 2 成员真实菜单→CE242 | `crates/wolf-game/tests/m11_party_live_bridge.rs`（1 测）；`docs/research/M11_PARTY_LIVE_BRIDGE.md` | in-memory fixture：CData type18 row4 col0=2、type6 row0 col2=13、type0 row13 stats/command cols47..54=[4,3,6,7,0,0,0,0]；真实 CE188→CE157 注册 15 行（row1 activator0 code26 蓄力、row2 activator1 code25 防御），input_pos=16；queue [1,2,3,4] 在真实 BOSS CE253→CE239 上产生 4 个不同 CE242 body first_pc 338/645/792/996，池 2000345/346 渐减为 [3,2,3,2]，settle v352=1/v343=1/rounds=4/turn_starts=3/warnings=0。 |
| task-94 CE328 分支图 | `crates/wolf-game/tests/m11_hscene_ce328_branches.rs`（4 测）；`docs/research/M11_HSCENE_CE328_BRANCHES.md` | 静态分支：任何 shipped save 都不满足 pc986 路径；var304>=1→pc17 Hしたい→pc740 CE332→pc741 op172 END；var304=0 且 var6==5/var4<=3→家族会議→topic menu pc173（4 选项汇合 pc303..668→749→755→843→1023）；其它→749/755/791/792→843→1023。pc986 在 悪戯へ body，仅从通常就寝 pc745 进入。 |
| task-95 默认武装战斗驱动 | `crates/wolf-game/src/battle_wiring.rs`、`game.rs`；`tests/m11_battle_autostart.rs`（3 测）；`docs/research/M11_BATTLE_AUTOSTART.md` | `EVENT_BATTLE_DISPATCH=253`；`manual_enable: Option<bool>` + `manual_round_loop: Option<bool>` + `auto_arm: bool=true`。tick：`manual_enable.unwrap_or(auto_arm && CE253常驻)`；round_loop 同理。`set_enabled(false)`=hard opt-out；新增 `Game::set_battle_auto_arm(bool)`/`battle_wiring_auto_armed()`。自然 day4 重放：ce253/239/242/243/241 全出现，rounds=2，v350_seq=[0,1,2,0,1,0]，v352_peak=1，v343_peak=1，max239pc=845，max242pc=866，day5 08:30，warnings=0，frames=8237；双人队列 auto 下 rounds=4/first_pcs=[338,645,792,996]。opt-out 保留 battle_active=true/max350=0/rounds=0。 |
| task-96 真实 CE328→CE402/CE410 | `crates/wolf-game/tests/m11_hscene_ce328_real_cg.rs`（2 测）；`docs/research/M11_HSCENE_CE328_REAL_CG.md` | 只 `start_event(328)`（不是 start_event(402)），calendar-consistent fixture：var304=0、var6=4、var4=21、var5=0、var18=20、var494=0、var160=90 加 CE402 门（16=1/22=180/326=2/102=80/103=2/23=6/151=0/20=61/265=0/545=1）。CE328 真实轨迹 679..986，CE402 pc0→511，CE410 pc0→247，warnings=0；var23=5 时第二测在 pc54 op172 中止、无 CE410。4 次菜单均 Index(0)（pc54/174/580/662）。CG 实测 36 项（cutin_138/139/140、h_s08_*、hword/* 等）。 |

## 3. 关键修正（推翻旧假设）

1. **CE157 cursor 不是 VM bug（task-89）**：物理 cursor 是 CData type18 row73 col0；旧结论把 type20 row73 当 cursor，永远读 0。真实连续确认会逐行推进。
2. **CE328 pc122 就寝条件（task-96）**：实测 `pc111 var6==4`、`pc113 var4>=6`、`pc115 var4<=20`；只有 pc113 为假（var6==4 且 **var4<6**，凌晨）才到 pc122 就寝。`var6==4 && var4>20` 会经 pc120/121/125 进入 topic/家族会議体，不是就寝。var4 是**小时**（0..23），不是日期。
3. **task-96 原静态路径不成立**：`pc862 悪戯へ→pc964 var64=1→pc968 420` 会在 pc998 收束并跳过 969..997 else 体，而 CE402 调用在 **pc986**；所以“必经 pc964/var64=1”是错的。可达路径是 **pc850 假→else pc969→pc986**，且不写 var64（测试断言 `var64_end==0`、轨迹不含 964）。
4. **CE402 实测门**：pc0 var16>=1；pc2 var22>=180；pc4 var4>=21；pc6 var326>=2；pc8 var102>=80；pc21 var103>=2；pc52 var23>=6（否则 pc54 op172 中止）；pc57 var151==0；pc62 var20>60；pc78 var265==0。旧建议 gate 集 {157=13,104=0,105=0,364==0} 并非 CE402 必要门。
5. **CE402/CE410 调用点**：CE402 全库唯一 caller = CE328 pc986 `op210[500402]`；CE410 的 10 个 caller 全在 CE402（pc165/173/177/195/198/288/308/311/360/447）。`spawn_child` 不写 `Vm.started_events`，只能用逐帧 `instances()` 观察 CE410。

## 4. 重基线 / 旧回归修复

- `crates/wolf-game/tests/m7_day5_schedule.rs`：去掉旧“stall 后注入 4 个 CE244 变量”的测试专用路径，改为真实默认驱动自然结算；等待 CE251 恢复（hour>=16），再等晚宴结束（!CE300/!CE326/hour>=18），并断言 saw253/saw239/saw242、!saw_training_menu、settled、v352_peak∈{1,-1}、v343_peak=1、CE239 消失、warnings=0；随后自然到 day5 08:30。实测 frames=8006/instructions=425409，warnings=0。
- `crates/wolf-cli/tests/m11_visual_regression.rs` battle baseline：1200 帧在默认 auto drive 下已过战斗（旧基线 522838 是 stalled 战场），改为 `--frames 600` 的活跃战斗 HUD（peak7/pictures52/quads961/textpics29/atlas20/text29/51/29/nonblack524412/589824/messages26/choices1）。其余 4 场景不变。

## 5. 验证门禁（本批集成后）

```bash
WOLF_DATA_ROOT=work/all cargo test -q --workspace --no-fail-fast -j 2   # 123 test result / 786 passed / 0 failed / 1 ignored
./target/debug/wolf-cli.exe verify --root work/all                      # PASS 70 / pending 0
```

本批集成实测：`cargo test -q --workspace --no-fail-fast -j 2` exit 0，123 个 test result、786 passed / 0 failed / 1 ignored；`wolf-cli verify --root work/all` = PASS — 70 check(s) matched baseline, 0 pending。

## 6. 边界 / 剩余（M11 尚未完成）

1. **H 深链**：真实 CE328 控制流已可达 CE402/CE410，但目前用 calendar-consistent 合成 fixture + budget=1 观测；尚未证明从某个 shipped save/自然日期在真实帧时序下必然进入（rand self10 分支也未穷举）。截图：`wolf-cli capture --event 410` 直启无画面（CE410 依赖 CE402 传入 self 参数），像素证据需先暴露 Game 帧 raster 入口。
2. **战斗菜单**：引擎桥和 2 成员 fixture 已通；shipped 自然流程中玩家 UI 输入到 CE157 的真实链路仍只按单行/单命令观测，尚未做完整多输入 UI 验收。
3. **移动/NPC**：ActionKey + 按住方向行走已实现；无行走动画帧、无对角线动画、NPC 对话路由仅覆盖测试样例；事件 contact 碰撞盒仍只对已知 4 个非零页。
4. **像素/音频**：锚点 5/6 仍是几何推断、无零售像素 diff；音频的人工听感（loop seam/音量/延迟）仍待验收。
5. **PRNG**：H-PRNG-1 零售时钟种子重放等价性仍未证。
6. **打包/性能**：Windows 发布包、启动数据目录校验、每帧预算/缓存仍未做。

## 7. 关键源码位置

| 位置 | 内容 |
| --- | --- |
| `crates/wolf-game/src/battle_wiring.rs` | 默认 auto-arm、CE253 dispatch、菜单采集/命令队列、round loop |
| `crates/wolf-game/src/game.rs` | `set_battle_auto_arm`、`advance_player_input`、`interact_action_key`、switch_map cadence reset |
| `crates/wolf-game/src/map_runtime.rs` | 8 向移动、contact box、`start_trigger_page` |
| `crates/wolf-game/tests/m11_hscene_ce328_real_cg.rs` | 真实 CE328→CE402/CE410 深链 |
| `docs/research/M11_HSCENE_CE328_REAL_CG.md` | 逐 pc/var/菜单/门/CG 全记录 |
| `crates/wolf-cli/tests/m11_visual_regression.rs` | 5 场景像素/quad 基线 |

## 8. 下一步建议（恢复后）

1. 把 task-96 的 budget=1 观测升级为真实帧时序 + 自然日历驱动的 H 深链集成测试，并补像素截图入口。
2. 把 2 成员真实菜单桥接入完整玩家循环（菜单 UI → CE157 → CE242），形成可玩多命令战斗。
3. NPC 行走动画与对话路由；锚点 5/6 零售像素对照；音频人工听感。
4. 若继续里程碑：写 `HANDOFF_M12.md` 并把本文件标历史归档。
