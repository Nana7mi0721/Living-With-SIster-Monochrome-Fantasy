# M12 交接：M11 round 3 收口 + 零售范围审计

> 仓库：`E:\Study\github program\imoto life`
> 阅读顺序：本文件（M12 最新）→ `docs/README.md`（文档总索引）→ `PLAN.md`（当前路线图）；M11 及更早已标历史归档。
> 基线：M11 第一批/round 1/round 2 见 `22de104`、`5ef6b1f`、`fdeed07`；本文件记录 round 3（task-97..100）与本批集成提交。数据根 `work/all`，零售存档 `work/retail_backup/Save`。

## 1. 一句话现状

M0–M11 保持可交付；M11 round 3 已完成四项剩余边界，并全部有测试或可复现命令：

- **H 深链自然帧时序**：不再依赖 `with_budget(1)` 饥饿观测。默认 200000 预算、普通 `Game::frame`、关闭背景 AutoEvents 后，只 `start_event(328)` 即自然到达 CE402（maxpc 509）并驱动 CE410 5 次，`warnings=0`。
- **真实玩家战斗菜单全链路**：harvester 改读已提交的 CData type14，并移到 `drive` 判断之前；真实 2 成员菜单无需测试注入即可注册 2 行并驱动 3 个不同 CE242 body。
- **零售 PRNG 时钟种子**：反汇编确认零售在 `0x35847` 调用 `KERNEL32!GetTickCount` 并直接存 `0x011b17f0`，无 folding；新增 `WolfRng::from_clock_ticks`，并如实记录零状态重读导致单 tick 不可重放。
- **打包与帧预算**：新增独立 `wolf-package` 校验/组装工具（4087 文件、约 1.53 GB VALID）与 `FrameBudgetReport`（debug 600 帧 p50 5.6µs / max 102.6µs）。
- **零售范围审计**：确认本作是纯鼠标点触式；方向键行走/碰撞/contact/ActionKey、地图触发 3/4、相机滚动/281、201 路线播放、远景视差都是引擎通用能力，零售不使用。存档/读档/回标题/退出/截图存档/相册/DLC 则全部有真实调用点。详见 `docs/research/M12_RETAIL_SCOPE.md`。
- **M12 round 1（task-101..103）**：战斗取消/返回/连续确认验收通过（取消不登记，后续确认仍登记两行并启动 2 次 CE157）；SampleMapA 6 个 confirm 页审计收口（5 个空页选择器 + 1 个 `self0==99` 调试页，事件格不是点击命中区）；8 个零售存档经 `Game::load_save → save_to` **整文件逐字节一致**（Part5 未建模 bank/文本 md1/淡入/纵向缩放全部保留）。

## 2. M11 round 3 交付与证据（task-97..100）

| 任务 | 交付 / 证据 | 结论 |
| --- | --- | --- |
| task-97 H 自然帧深链 + save+event 截图 | `crates/wolf-cli/src/main.rs` 新增 `CaptureEntry::SaveWithEvent { path, event }`；`crates/wolf-game/tests/m11_hscene_ce328_natural.rs`（2 测）；`docs/research/M11_HSCENE_NATURAL.md`；`work/out/m11_hscene_natural.png` | `--load-save` + `--event` 可组合；自然链 `frames=3667 maxpc328=1023 maxpc402=509 CE410 acts=[1,1,2,1,3] cg_files=22 warnings=0 var64=0`。诚实上限：自然时序 CE402 在 pc208 早退到 tail pc509，act4/5 的 CE410 pc>200 不可达。 |
| task-98 真实玩家战斗菜单→CE157→CE242 | `crates/wolf-game/src/battle_wiring.rs`、`crates/wolf-game/src/game.rs`；`crates/wolf-game/tests/m11_battle_menu_live.rs`；`docs/research/M11_BATTLE_MENU_LIVE.md` | harvester 读 committed CData type14（activator=col0、ability code=col3、row 1..=8、按 row 去重、command_index=row.clamp(1,8)），不再读瞬态 self1600017；tick 在 `drive` 判断前采集。真实 2 成员 `menu_rows=[{row1,act0,code26},{row2,act1,code25}]`、`harvested=[1,2]`、`ce157_starts=2`，battle `rounds=3 first_pcs=[338,645,792] arg0s=[1,2,3] final_pools=[3,3,3,2] settle=(1,1) turn_starts=2 warnings=0`，断言全程不调用测试注入入口。 |
| task-99 PRNG 零售时钟种子 | `crates/wolf-vm/src/rng.rs` 新增 `seed_from_clock_ticks` / `WolfRng::from_clock_ticks`；`crates/wolf-vm/tests/prng_clock_seed.rs`（5 测）；`docs/research/M11_PRNG_CLOCK.md` | 反汇编 `work/runcopy/Game.exe`（SHA256 98949e233e4085d9c745b2e8ba41e17ff49e5dafff16e5f28882d39bcaf804ea，PE32 i386，ImageBase 0x400000）：RVA 0x35810 / 文件偏移 0x34c10；0x35847 `FF 15 44 20 80 00 call [0x00802044]`=GetTickCount，0x35852 `A3 F0 17 1B 01 mov [0x011b17f0],eax` 直接存 tick；QPC/GetSystemTimeAsFileTime/GetTimeZoneInformation 0 次；lazy seed guard VA 0x011b15e8。RCX mode 0=Park-Miller advance（*16807 %0x7fffffff，rejection >2e9），2=set state，else peek；zero-state 0x358d1 重读 GetTickCount，无法从单 tick 重放，故 build 仍用 EVENT_RESEED。`cargo test -q -p wolf-vm` 135 passed。 |
| task-100 打包/帧预算 | `crates/wolf-cli/src/bin/wolf-package.rs`（validate/assemble/check/--check/bench、manifest path/bytes/sha256、streaming SHA-256）；`crates/wolf-cli/tests/m11_package.rs`（4 测）；`crates/wolf-game/src/frame_budget.rs`；`docs/research/M11_PACKAGING.md` | validate real `work/all`：3/3 dirs、8/8 files、4087 files、约 1.53 GB（字节数随 Save 数据变化）、extra none、VALID；assemble/check round-trip deterministic、sha mismatch/missing 均测；bench 600 帧 debug p50=5.6µs p95=7.5µs max=102.6µs mean=6.0µs warnings=0。release 编译超时，文档如实说明。 |

## 3. 关键修正（推翻旧假设）

1. **H 深链的 budget=1 是 starvation artefact**：真实 confound 是 ~9 个 `run_condition==2` 背景 AutoEvents 协程把 CE402 引到早退分支；测试必须 `g.auto_events.set_enabled(false)`，然后用默认 200000 预算观察自然帧。
2. **战斗 harvester 不能读瞬态 self1600017**：pc64 的 stale 0 会污染成 `[1,1]`；真实权威是已提交的 CData type14。采集必须在 `if !drive { return None; }` 之前执行，否则非战斗菜单帧根本不采集。
3. **PRNG 时钟种子无折叠**：GetTickCount 结果直接写全局；Park-Miller 拒绝采样在 advance 分支。零状态会重读时钟，因此“给一个 tick 就完全重放”不成立。
4. **打包不依赖游戏运行**：`wolf-package` 是独立 bin，只校验目录/必需文件并流式计算 SHA-256，不修改 `main.rs` 的现有 capture/run 路径。

## 4. 验证门禁（本批集成后）

```bash
WOLF_DATA_ROOT=work/all cargo test -q --workspace --no-fail-fast -j 2   # 131 test result / 810 passed / 0 failed / 1 ignored
./target/debug/wolf-cli.exe verify --root work/all                      # PASS 70 / pending 0
cargo build -q -p wolf-cli                                             # EXIT 0
```

本批集成实测：`cargo test --workspace` EXIT 0，131 个 test-result 块，810 passed / 0 failed / 1 ignored（唯一 warning 来自永久 scratch `_scratch_cdata.rs` 未用变量）；`verify` = PASS 70 / pending 0；`wolf-cli` 单测与回归全绿。

## 5. 零售范围审计结论（M12）

**原则**：只实现游戏零售能走到的流程；WOLF 引擎有、但本作不用的通用能力不再新增，也不再写成“剩余缺口”。完整证据见 `docs/research/M12_RETAIL_SCOPE.md`。

### 5.1 零售实际使用（必须保留）

| 功能 | 证据 |
| --- | --- |
| 220 SaveLoad / 221 LoadGame / 222 SaveGame | 220 8 条、221 16 条、222 5 条；CE131/CE435/CE436 存档/自动存档，CE94 载入窗口，TitleMap ev2 初始化 system.sav/system2.sav |
| 174 ReturnToTitle / 175 EndGame | 174 13 条（CE246/247/305/322/323/326/359/405 + SampleMapA ev4/5/24）；175 2 条（CE38/CE42） |
| 1000 ProFeature（截图） | CE131 存档截图、CE435 自动存档截图、CE452 DLC `Data/SC1.jpg`、CE525/526 相册 |
| 150 Picture + 124 hover/click | 零售交互主路径；全库 150 共 16716 CE + 3709 地图页 = 20425 条，124 共 579 CE + 466 地图页 = 1045 条 |
| 130 Teleport / 151/160/161/162 | 新游戏 Map003 ev1 → SampleMapA；地图侧唯一转场组 |

### 5.2 引擎通用、零售不使用（不新增、不列缺口）

- **方向键行走/碰撞/contact/ActionKey**：注册 6 张地图 115 页 `graphic` 全空，0 个 player-touch/event-touch 页；`game.rs:1384 advance_player_input()` 是唯一接进帧循环的行走钩子，现有 8 向/接触 API 仅供 synthetic 测试。
- **触发类型 3/4、相机滚动/culling/281 ScrollScreen、201 SetMoveRoute 路线播放、远景视差**：281 全库 3 条且只在其无调用者的 CE225；201 在注册地图只剩 `[-2]`/`[-4]` 无形定位；SampleMapA 64×36 正好 1024×576，地图铺满 viewport；远景速度全 0 且被 3 层 tile 完全遮住。
- **处理**：现有 API 保留，避免大范围回归；以后若清理，优先摘 `game.rs:1384` 的行走钩子并同步改 `crates/wolf-game/tests/m11_npc_interaction.rs`，这属于代码瘦身，不是内容缺口。

### 5.3 示例资产 / 无零售证据（不计必做）

- Dungeon / SampleMapB / Map002 是 WOLF 编辑器示例（目的地 map 27/32 未注册，Map002 未注册）；Map024/Map031 无 tile 层且无自然 130 入口。
- **TitleMap 不是死资产**：`game.rs:723-742 boot()` 实测 TitleMap ev2 → CE549 三 splash → 按键等待 → CE42；文档旧说法已修正。
- H act4/5：自然帧 CE402 pc208 早退到 tail pc509，CE410 acts=[1,1,2,1,3]；act4/5 只在 budget≤5 饥饿观测出现，无零售存档证据。
- 3–4 人队伍：CData type18 row4 col0=2，真实队伍只有 2 人。

### 5.4 真正的剩余

1. ~~**战斗完整玩家输入**~~：M12 task-101 已完成取消/返回/连续确认验收（`M12_BATTLE_CANCEL.md`）；真实 2 人菜单取消不登记、后续确认仍登记两行并启动 2 次 CE157。剩余只有目标选择窗口/左右切换的逐项 UI 验收（可选）。
2. ~~**点触命中框与事件格坐标一致性**~~：M12 task-102 已收口（`M12_CONFIRM_PAGES.md`）。5 个 confirm 页是空页选择器，第 6 个是 `self0==99` 调试页且全库无写入；点击只走 124 图片矩形，事件格不是命中区。
3. **视觉像素**：锚点 5/6 的零售像素 diff、关键场景截图 diff（仍需零售对照截图）。
4. **音频人工听感**：31 首 loop 的 seam/音量/延迟（需人工）。
5. ~~**存档逐段全量对照**~~：M12 task-103 已达成 `Game` 层 load→save 整文件逐字节一致（8/8 存档，`M12_SAVE_FULL_COMPARE.md`）；未建模的音调/效果 bank 只保留、不模拟。
6. **日程/多日自然推进**（若还有未覆盖边界；当前自然推进到 day5 08:30）。
7. **工具/保真（非游戏流程）**：PRNG 单 tick 完全重放、`wolf-package` 发布流水线、release 帧预算——按需做，不算内容缺口。

## 6. 关键源码位置

| 位置 | 内容 |
| --- | --- |
| `crates/wolf-cli/src/main.rs` | `CaptureEntry::SaveWithEvent`、`--load-save`+`--event`、`run_capture` 的 `Entry::Save` 组合 |
| `crates/wolf-game/src/battle_wiring.rs` | `harvest_menu_registrations` 改读 CData type14、tick 前置采集 |
| `crates/wolf-game/src/game.rs` | `battle_wiring.tick(&mut self.vm, &self.databases)` 接入 |
| `crates/wolf-vm/src/rng.rs` | `seed_from_clock_ticks` / `WolfRng::from_clock_ticks` |
| `crates/wolf-cli/src/bin/wolf-package.rs` | 独立打包校验/组装工具 |
| `crates/wolf-game/src/frame_budget.rs` | `FrameBudgetReport` / `measure_game` |
| `crates/wolf-game/tests/m11_hscene_ce328_natural.rs` | 自然帧 H 深链 2 测 |
| `crates/wolf-game/tests/m11_battle_menu_live.rs` | 真实菜单→CE242 全链路 |
| `crates/wolf-game/tests/m12_battle_cancel.rs` | 取消/返回/连续确认 2 测 |
| `crates/wolf-game/tests/m12_confirm_pages.rs` | confirm 页/命中审计 3 测 |
| `crates/wolf-game/tests/m12_save_full_compare.rs` | 零售存档整文件逐字节 2 测 |
| `crates/wolf-game/src/picture.rs` | `Picture::saved_record` 写入基线 |
| `crates/wolf-game/src/savegame.rs` | Part5 活字段覆盖 + 未建模 bank 保留 |
| `crates/wolf-vm/tests/prng_clock_seed.rs` | 时钟种子 5 测 |
| `crates/wolf-cli/tests/m11_package.rs` | 打包 4 测 |

## 7. 下一步建议（恢复后）

1. **不要再扩展引擎通用能力**：行走/碰撞/8 向/contact/ActionKey、触发 3/4、相机/281/201/远景视差均不是零售内容；现有实现保留即可。
2. 还原度优先：锚点 5/6 零售像素 diff、关键场景截图 diff；31 首 loop 人工听感。
3. 日程/多日自然推进：从 day5 08:30 继续到 day6/7，若零售流程支持；补跨天存档一致性。
4. 工具按需：PRNG 单 tick 重放、`wolf-package` 发布流水线、release 帧预算——不作为内容缺口。
5. 每个新里程碑完成时按 `AGENT.md` §5 收口：HANDOFF、BASELINE、PLAN、README、research 索引同步。

## 8. M12 round 1 交付（task-101..103）

| 任务 | 交付 / 证据 | 结论 |
| --- | --- | --- |
| task-101 战斗取消/返回/连续确认 | `crates/wolf-game/tests/m12_battle_cancel.rs`（2 测）；`docs/research/M12_BATTLE_CANCEL.md` | 无生产改动。真实 CE188→CE191→CE192→CE65 链：CE191 窗口取消（-100）重开且不登记；CE192 窗口取消退出（`self1600099=-1`）且不写行/不启动 CE157；之后确认仍得到 `command_rows=[(1,0,26),(2,1,25)]`、`ce157_starts=2`、warnings=0，全程无 wiring/queue 注入。 |
| task-102 confirm 页命中审计 | `crates/wolf-game/tests/m12_confirm_pages.rs`（3 测）；`docs/research/M12_CONFIRM_PAGES.md`；`docs/MAP_AUDIT.md` §9 更新 | ev11/14/18/21/22 的 confirm 页 0 条真实命令，只在 `v302==1` 时作为最高页停掉 parallel 页；ev24 p6 有 251 条命令但门控 `self0==99`，全库 550 CE + 9 地图无任何写 `magic 1_000_000` 的脚本，零售不可达。事件格像素上只有整屏背景层（ev21 另有 sub button 107），点击命中区是 124 图片矩形，不是事件格。 |
| task-103 存档逐段全量对照 | `crates/wolf-game/src/picture.rs`、`crates/wolf-game/src/savegame.rs`；`crates/wolf-game/tests/m12_save_full_compare.rs`（2 测）；`docs/research/M12_SAVE_FULL_COMPARE.md` | `Picture::saved_record` 作为写入基线；`picture_to_record` 只覆盖活字段，保留未建模 bank、文本 md1、上次淡入长度、纵向缩放。8/8 零售存档 `load_save → save_to` **整文件逐字节一致**（SaveData01 366146 … SaveData08 395566）；移动图片后新位置写入且未建模 bank 保留。 |

### 8.1 关键修正

1. **Part5 不是「必须从活屏幕重建」**：重建活字段是对的，但把未建模 bank 清零会破坏零售状态；正确做法是以加载记录为基线、只覆盖活字段。
2. **文本图片的 md1 不能无条件重编码**：解码只近似组合字体/间距转义，重编码会改长度；文本未变时复用原字节流。
3. **画面纵向缩放独立于横向**：图片 6233（行动条）与 SaveData08 的 1996/1998 使用 62×100 / −100×100；统一缩放写回会把 `v[10]` 改坏。
4. **confirm 页不是点击目标**：五个空页只是页优先级开关；第六个是 `self0==99` 调试页，全库无写入。
5. **取消语义分两层**：CE191 的取消重开同一窗口（`1600015 == -100` 继续循环）；CE192 的取消退出（`self1600099 = -1` + BreakEvent）。两者都不登记指令。
