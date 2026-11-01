# M12 交接：M11 round 3 收口（自然帧 H、真实战斗菜单、PRNG 时钟、打包/帧预算）

> 仓库：`E:\Study\github program\imoto life`
> 阅读顺序：本文件（M12 最新）→ `docs/README.md`（文档总索引）→ `PLAN.md`（当前路线图）；M11 及更早已标历史归档。
> 基线：M11 第一批/round 1/round 2 见 `22de104`、`5ef6b1f`、`fdeed07`；本文件记录 round 3（task-97..100）与本批集成提交。数据根 `work/all`，零售存档 `work/retail_backup/Save`。

## 1. 一句话现状

M0–M11 保持可交付；M11 round 3 已完成四项剩余边界，并全部有测试或可复现命令：

- **H 深链自然帧时序**：不再依赖 `with_budget(1)` 饥饿观测。默认 200000 预算、普通 `Game::frame`、关闭背景 AutoEvents 后，只 `start_event(328)` 即自然到达 CE402（maxpc 509）并驱动 CE410 5 次，`warnings=0`。
- **真实玩家战斗菜单全链路**：harvester 改读已提交的 CData type14，并移到 `drive` 判断之前；真实 2 成员菜单无需测试注入即可注册 2 行并驱动 3 个不同 CE242 body。
- **零售 PRNG 时钟种子**：反汇编确认零售在 `0x35847` 调用 `KERNEL32!GetTickCount` 并直接存 `0x011b17f0`，无 folding；新增 `WolfRng::from_clock_ticks`，并如实记录零状态重读导致单 tick 不可重放。
- **打包与帧预算**：新增独立 `wolf-package` 校验/组装工具（4087 文件、约 1.53 GB VALID）与 `FrameBudgetReport`（debug 600 帧 p50 5.6µs / max 102.6µs）。

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
WOLF_DATA_ROOT=work/all cargo test -q --workspace --no-fail-fast -j 2   # 128 test result / 803 passed / 0 failed / 1 ignored
./target/debug/wolf-cli.exe verify --root work/all                      # PASS 70 / pending 0
cargo build -q -p wolf-cli                                             # EXIT 0
```

本批集成实测：`cargo test --workspace` EXIT 0，128 个 test-result 块，803 passed / 0 failed / 1 ignored（唯一 warning 来自永久 scratch `_scratch_cdata.rs` 未用变量）；`verify` = PASS 70 / pending 0；`wolf-cli` 单测与回归全绿。

## 5. 边界 / 剩余（M12 可继续）

1. **H 深链**：自然帧已到 CE402 pc509 / CE410 act≤3；act4/5 的 CE410 pc>200（`v144_cg20`、`cutin_189/190`）在自然早退路径不可达。若要从自然日历必然进入更深分支，仍需事件/日历推进而非测试 fixture。
2. **战斗 UI**：真实菜单 harvester 已通；玩家在多命令界面里的完整输入序列（含取消、返回、多回合连续确认）仍只覆盖 2 成员 fixture。可扩展到 shipped 3–4 人队伍。
3. **移动/NPC**：零售注册地图是点触式（6 张地图 115 页 `graphic` 全空，交互靠 150 Picture + 124 悬停），没有 NPC 行走动画/走位对话内容可补；现有 8 向行走与 ActionKey 只是引擎通用能力/测试夹具，不计入内容缺口。剩余仅是点触命中框与事件格坐标一致性（若还有未覆盖的 confirm 页）。
4. **像素/音频**：锚点 5/6 仍是几何推断，无零售像素 diff；31 首 loop 的人工听感（seam/音量/延迟）仍待验收。
5. **打包/性能**：`wolf-package` 已有 validate/assemble/check/bench，但 Windows 发布包（安装器/资源布局）与启动数据目录自动校验仍未接线到主程序；release 性能未测。
6. **PRNG**：零售时钟种子来源与折叠已证，但“同 tick 完全重放”受零状态重读限制；如需逐帧重放，仍以 `EVENT_RESEED` 为准。

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
| `crates/wolf-vm/tests/prng_clock_seed.rs` | 时钟种子 5 测 |
| `crates/wolf-cli/tests/m11_package.rs` | 打包 4 测 |

## 7. 下一步建议（恢复后）

1. 若继续内容：把 2 成员真实菜单扩展到 shipped 队伍，补完整多命令 UI（取消/返回/连续确认）验收。
2. 若继续 H：用自然日历/事件把 act4/5 深分支推进到真实可达，并补 Game 帧 raster 的稳定截图入口。
3. 若继续交付：把 `wolf-package` 接成发布流水线，补 release 帧预算与数据目录启动校验。
4. 每个新里程碑完成时按 `AGENT.md` §5 收口：HANDOFF、BASELINE、PLAN、README、research 索引同步。
