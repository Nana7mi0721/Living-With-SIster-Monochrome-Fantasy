# M10 交接：opcode 111 引用语义落地、战斗重基线、残项收口

> 仓库：`E:\Study\github program\imoto life`
> 阅读顺序：本文件（M10 最新）→ `docs/README.md`（文档总索引）→ `PLAN.md`（当前路线图）；历史 M9/M8/M7/M6/M4/M3 交接已标为历史归档，需要追溯时从索引进入。
> 基线：M9 集成 HEAD `22148d8`；M10 由 `ce680b3`（残项）与 `3bbbb12`（opcode 111 落地）两个提交组成。M10 剩余边界项（M11 第一批 task-79..84）已收口，见 §8；M11 round 1（task-85..88）已收口，见 §9。`docs/gpt调研/` 与 `.claude/` 不是项目文件，不提交。

## 1. 一句话现状

M0–M9 保持可交付；M10 已完成：

- **opcode 111 引用语义已落地**：值操作数走 `eval_num`（变量 / self 引用按引用求值），不再当字面量。
- **compound() 预存 bug 修复**：0x600 / 0x700 / 0x800 现在作用于目标当前值（而非二元运算结果）。这修掉了 CE172 pc27/28/29 → pc32 `dead == total` 的假 abort（旧 literal 语义把它掩盖了；修复后 CE172 才按真实敌我存活数判定）。
- **12 个战斗/H 测试已重基线**到真实多回合战斗经济与夹取后的能量/H 模式值；自然 day4 路线改为 RNG 容错的结构断言。
- **M10 残项**：存档 CData runtime row counts 恢复、地图 contact box/player start、CE246→CE346 cutin 可达、H 深链合成存档（CE402/CE410）、音频残留审计。

验证门禁（本文件随提交入仓时）：

```bash
cargo test -q --workspace --no-fail-fast   # 全部二进制绿，0 failed
./target/debug/wolf-cli.exe verify --root work/all
# PASS — 70 check(s) matched baseline, 0 pending
```

## 2. 本批交付与证据

| 任务 | 交付 / 证据 | 结论 |
| --- | --- | --- |
| task-74 opcode111 审计 | `work/out/m10_landing.patch`；`docs/research/M10_OPCODE_111_LANDING.md`；`M10_OPCODE_111_FIRST_DIVERGENCE.md` | 隔离树证明两项 VM 修复正确；裸补丁令 15 测试红，先审计不裸落 |
| task-77 重基线 | `work/out/m10_landing_full.patch`（842 行 / 15 文件）；隔离树 99 binaries 701 passed / 0 failed | VM + 12 个 wolf-game 测试全部重基线 |
| task-78 独立复核 | `docs/research/M10_REBASELINE_REVIEW.md` | bytecode 派生复核 PASS 12/12；确认无“匹配观测”掩盖 |
| M10 opcode 落地 | commit `3bbbb12`；`crates/wolf-vm/src/exec.rs`；`crates/wolf-vm/tests/opcode_111_semantics.rs` | `cargo test --workspace` 绿、verify PASS70 |
| task-70 H 深链合成档 | `tests/m8_hscene_cg_deep.rs` 6 测；`docs/research/M10_HSCENE_DEEP_SAVE.md` | 合成 SaveData07 满足 gate；CE402 max pc312/512、CE410 騎乗位 cutin、14 个 h_s08 |
| task-76 CE246→CE346 | `tests/m8_hscene_cutin.rs` 4 测；`docs/research/M10_HSCENE_CE346.md` | op210 [24,1,1]×6 + [25,1,1]；真实存档可达 CE346 max pc180 |
| task-71 地图 contact | `crates/wolf-game/src/map_runtime.rs`、`game.rs`；`tests/m8_map_contacts.rs` 6 测；`docs/research/M9_MAP_CONTACTS.md` | 134 页仅 4 页非零 contact；EXTENSION_UNIT_PX=8；player_start 来自 hero-target 130 |
| task-75 存档完整度 | `crates/wolf-game/src/savegame.rs` +20 行；`tests/m8_save_parts.rs` 6 测；`docs/research/M10_SAVE_COMPLETENESS.md` | 恢复保存的 CData runtime row counts（-2 扩表后 row_count 仍旧 .dat 高度） |
| task-72 音频残留 | `tests/m10_audio_residual.rs` 2 测；`docs/research/M10_AUDIO_RESIDUAL.md` | kana.mp3 死注册、零引用、缺失安全；31 loop-tagged 全 ogg |

## 3. opcode 111 具体改动

`crates/wolf-vm/src/exec.rs`：

1. **值操作数引用语义**：opcode 111 的 value 操作数由 `eval_num(...)` 求值。全库 22695 个三元组中约 356 个操作数是变量/self 引用（1.57%）。
2. **compound() 0x600/0x700/0x800**：旧实现写 `value`（arg1 op arg2 的结果），现改为 `current`（目标当前值，0x800 再取 abs）。这是 OPCODE_SPEC §3.1 的语义。

**为何 compound bug 是致命级联点**：CE172「[戦]敵/味方·勝敗判定」pc25-29 计算阵亡人数：
- pc25 `121[1600021,1,0,256]`（self21 递减）
- pc26 调 CE176 读该队友是否战闘不能
- pc27 `121[1600022,0,0,1536]`（0x600）
- pc28 `121[1600022,1,0,1792]`（0x700）
- pc29 `121[1600023,1600022,0,256]`（self23 += self22）
- pc32 `111[1,1600023,1600020,2]`（dead == ally-count → 判负）

旧 compound 把 self22 强制为 1（无视 CE176 返回值），每个队友都被算阵亡 → pc32 恒成立。literal 语义下 pc32 永不匹配，bug 被掩盖；切到引用语义后立刻全线崩塌。修复 compound 后 workspace 失败面 22 → 12。

## 4. 12 个重基线测试（old → new）

| # | 文件:行 | 旧断言 / 旧值 | 新断言 / 新值 |
| --- | --- | --- | --- |
| 1 | `m6_mainline.rs:428` | `instructions > 10_000` | `> 9_000`（实测 9792） |
| 2 | `m6_training_menu.rs:162` | `before_energy == 100` | `== 60`（CE226 pc15 夹取） |
| 3 | `m7_battle_seed.rs` | 控制组 reached_ce192=false | 补仅队伍种子 → true |
| 4 | `m7_battle_menu_text.rs` | 控制组 reached_ce192=false | 补仅队伍种子 → true |
| 5 | `m7_battle_target_scheduler.rs:363` | `var350==2` | `var350==0`（transitions [0,1,2,0]；CE239 pc1043 cleanup） |
| 6 | `m7_ce244_direct.rs:128` | `[(11,1),(1600010,-1)]` | `[(11,-1),(1600010,1)]`（stale） |
| 7 | `m7_day4_playable.rs` | 单回合无法结算（路线本地 pre_power=132<200） | `+set_battle_round_loop(true)`；rounds=2, v350=[0,1,2,0,1,0], v352_peak=1, v343_peak=1 |
| 8 | `m7_hmode_state.rs` | (var14,115), (var16,4) | (var14,35), (var16,3)（CE226 pc15 clamp） |
| 9 | `m8_battle_party.rs` | 8 命令均单回合结算 | cmd1-4 结算(v343=1)，cmd5-8 留敌(v343=0,v352=0) |
| 10 | `m8_hscene_forward.rs:890` | `vars.get(218)==2` | `var218_max==2`（瞬态） |
| 11 | `m9_battle_round_gap.rs` | 单回合 v350=[0,1,0] | `+round_loop` 定值（task-79 逐字节复现）：v350=[0,1,2,0,1,2,0,1,2,0,1,0]、settle=1516、v349=4、pools=[3,2,3,2]、v352=1、CE241/CE243 出现 |
| 12 | `m9_full_route.rs` | pre_power<200 未结算 | `pre_power>=200`（201 高能量分支）+round_loop 到 day5 08:30；战斗定值 rounds=4、v350=[0,1,2,0,1,2,0,1,2,0,1,0]、v352_peak=1、max239pc=845 |

## 5. 已确认边界 / 未完成

1. **战斗确定性（已证，task-79）**：`started_events` 现为 `BTreeSet<EventId>`（`exec.rs:806`），`instances` 为 `Vec`（按启动顺序），其余 `HashMap`（`program.rs` 的 labels/by_name）只用于键查表、从不迭代；RNG 只依赖固定帧循环 + Vec 实例顺序（EVENT_SEED 固定）。`m9_battle_round_gap` / `m9_full_route` 连续 5 次进程重跑轨迹逐字节一致（md5 相同）：v350=[0,1,2,0,1,2,0,1,2,0,1,0]、settle=1516、v349=4、pools=[3,2,3,2]、v352=1。故 #11/#12 已改为定值断言（见 `docs/research/M11_BATTLE_DETERMINISM.md`）；旧『HashSet 抖动』结论作废。PRNG 零售时钟种子重放等价性（H-PRNG-1）仍待单独证明。
2. **H 深链**：CE402/CE410 只在 **合成存档**（基于 SaveData07，`work/out/m10_deep_synth.sav`）下可达；真实 8 个零售存档无单一同时满足所有 gate。CE346 走 CE246 真实存档可达。
3. **opcode 111 已落地，M10_OPCODE_111_PLAN.md 的 Phase0..3 可视为完成**；旧 PARKED patch 作废，以 HEAD `exec.rs` 为准。
4. 其余边界沿用 HANDOFF_M9：多输入菜单只注册一条命令、CE242 arg0=命令索引、H 深链 gate、窗口右缘、移动无 diagonal/NPC、音频人工听感、锚点 5/6 几何推断。

## 6. 关键源码位置

| 位置 | 内容 |
| --- | --- |
| `crates/wolf-vm/src/exec.rs` | opcode111 `eval_num`、compound 0x600/0x700/0x800 current |
| `crates/wolf-game/src/savegame.rs` | 恢复 CData runtime row counts |
| `crates/wolf-game/src/map_runtime.rs` | contact box、player start、terrain/trigger |
| `crates/wolf-game/tests/m8_hscene_cg_deep.rs` | 合成存档 CE402/CE410 深链 |
| `crates/wolf-game/tests/m8_hscene_cutin.rs` | CE246→CE346 cutin |
| `docs/research/M10_OPCODE_111_LANDING.md` | 完整 old→new 表与 §10.2 RNG caveat |
| `docs/research/M10_REBASELINE_REVIEW.md` | 独立 bytecode 复核 |

## 7. 下一步建议

1. **战斗确定性（已完成，task-79）**：`started_events` 已是 `BTreeSet`、无 HashSet 顺序问题；自然 day4 路线已逐位复现，并把 #11/#12 改为定值断言（`docs/research/M11_BATTLE_DETERMINISM.md`）。剩余仅 H-PRNG-1 零售时钟种子重放等价性待证。
2. **H 完整 CG 分支**：把合成存档的 gate 组合拆成多个测试或做成数据补丁，不要硬改真实存档。
3. **音频人工听感**：按 `M10_AUDIO_RESIDUAL.md` 的 18 个 heuristic 候选听一遍循环点；调查 kana.mp3 来源。
4. **像素**：锚点 5/6 零售画面对照；窗口皮肤 CN patch 右缘数据放置。

## 8. M10 剩余边界项收口（M11 第一批，task-79..84）

本批六项全部完成，均未改动生产逻辑（唯一生产代码改动是 task-81 在 `crates/wolf-game/src/map_runtime.rs` 增加 8 向移动/NPC contact，随后由 Lead 统一验证）。

| 任务 | 交付 / 证据 | 结论 |
| --- | --- | --- |
| task-79 战斗确定性 | `crates/wolf-game/tests/m9_battle_round_gap.rs`、`m9_full_route.rs` 收紧为定值；`docs/research/M11_BATTLE_DETERMINISM.md` | 5 次进程重跑轨迹 md5 一致：v350=[0,1,2,0,1,2,0,1,2,0,1,0]、settle=1516、v349=4、pools=[3,2,3,2]、v352=1；旧 HashSet 抖动结论作废 |
| task-80 H 真实 gate 矩阵 | `crates/wolf-game/tests/m11_hscene_real_gate.rs` 4 测；`docs/research/M11_HSCENE_REAL_GATE.md` | 8 个零售存档均缺 var304（夜间亲密置 1）与 var326（CE301 学会骑乗位）；CE402/CE410 仍只能合成档到达，属内容进度而非数据 bug |
| task-81 对角移动 / NPC | `crates/wolf-game/src/map_runtime.rs` +214 行；`tests/m11_movement_npc.rs` 7 测；`docs/research/M11_MOVEMENT_NPC.md` | WOLF 8 向、对角不切角、阻挡仍转向、NPC 占位；game.rs 未改 |
| task-82 锚点 5/6 像素几何 | `tests/m11_anchor_pixels.rs` 5 测；`docs/research/M11_ANCHOR_PIXELS.md` | 七个锚点几何与负缩放镜像已固定；仍缺零售像素 diff，垂直镜像需独立 Y scale 字段 |
| task-83 多成员命令序列 | `tests/m11_battle_command_sequence.rs` 418 行 2 测；`docs/research/M11_BATTLE_COMMAND_SEQUENCE.md` | 真实 CE253→CE239 BOSS gauge 下 schedule [1,2,3,4] 4 回合结算、[1,3,5,6] 9 回合；命令索引桥仍需引擎侧数据补丁 |
| task-84 音频 loop 验收 | `crates/wolf-audio/tests/m11_audio_loop_audit.rs`；`docs/research/M11_AUDIO_ACCEPTANCE.md` | 31 首 loop-tagged 全 ogg、31/31 decode 且有界；14 首 A1 高绝对接缝优先人工听；机器不能替代人工听感 |

验证门禁（Lead 集成后）：`cargo test -q --workspace --no-fail-fast` 全绿；`wolf-cli verify --root work/all` PASS 70 / pending 0。

## 9. M11 round 1（task-85..88）：菜单桥、H 进度、窗口右缘、音频回归

本批四项全部完成，均未由子代理提交；Lead 集成时只在 `crates/wolf-game/src/game.rs` 补了 `Game::set_battle_menu_command_queue(Vec<i32>)` 这一应用层入口（委托 `BattleWiring::set_menu_command_queue`），使 `battle_wiring.rs` 模块文档的入口承诺与实际代码一致。唯一生产文件改动是 `crates/wolf-game/src/battle_wiring.rs`（+180/-1）。

| 任务 | 交付 / 证据 | 结论 |
| --- | --- | --- |
| task-85 真实多成员菜单桥 | `crates/wolf-game/src/battle_wiring.rs`；`tests/m11_battle_menu_bridge.rs` 2 测；`docs/research/M11_BATTLE_MENU_BRIDGE.md` | 新增 `MenuSelection{activator,skill,row,command_index}`、`harvest_menu_registrations`（扫 live CE157，读 self1600000/1600001/1600017，按 instance id 去重，`command_index=row.clamp(1,8)`）、`set_menu_command_queue`、`menu_queue_len`；`next_command` 先 drain 队列再回退 schedule（空时 byte-identical）；3 个 unit + 2 个集成测。真实 BOSS 路径 schedule [1,3] + round_loop：rounds=5、turn_starts=4、v352=1、v343=1、warnings=0。剩余 producer 缺口：CE157 pc121 门 `self1600023==0` 被 pc93/pc100 置 100 后跳过 pc124-125，`コマンド入力位置` 恒 0，live 菜单只注册 row 1；需数据/VM 修复或应用层显式 feed（`Game::set_battle_menu_command_queue`）。 |
| task-86 H CG 真实进度 | `tests/m11_hscene_progression.rs` 4 测；`docs/research/M11_HSCENE_PROGRESSION.md` | var304=1 真实可达：`start_event(290)` 夜间菜单 case7 → `pc403 op121[2000304,1]` → `op210[500328]` 调 CE328（SaveData01 peak 1、maxpc≥640、warnings0）。var326=2 只由 CE301 pc6320/pc6365 写，需 var276>=1 && var277>=1；var277 只由书店 CE314 增，CE314 只由 CE308/CE310 调用，8 档全 0。即使 seed var326=2 + 真 var304，CE328 H 分支在 pc741 `op172` 就寝黑屏终止，maxpc<985，CE402（唯一调用 pc986）不可进。 |
| task-87 窗口 CN patch 右缘 | `tests/m11_window_edge.rs` 3 测；`docs/research/M11_WINDOW_EDGE.md`；截图 `work/out/task87_event94.png` | DATA-ONLY，无渲染 bug；M9 §8 的 1155 右缘溢出是算术错误，已就地更正。真实 CE94：#12500 WindowString (855,200) anchor1 300x200 → dst [705,100,300,200]（右 1005、下 300 均在 1024x576 内），皮肤 nine-slice [705,100,3,3]/[708,103,294,194]/[1002,297,3,3]；#12501 Text anchor0 (715,115) 七段 x715..955 y115..297。`window.rs`/`atlas.rs` 未改。 |
| task-88 音频 loop 回归 | `crates/wolf-audio/tests/m11_audio_loop_regression.rs` 476 行 1 测；`docs/research/M11_AUDIO_LOOP_REGRESSION.md` | 31 首 loop-tagged 全 ogg、31/31 decode；tag→region 契约固定（`loop_start==LOOPSTART`、`loop_end==min(LOOPSTART+LOOPLENGTH,frames)`）；逐曲 seam_abs/step/ratio/rms_jump/dc_delta 阈值固定；tier A=17/B=1/C=10/D=3；flags SEAM_ABS=14、SEAM_RATIO=11、RMS_JUMP=18、OVERRUN=3、LEN_EQUALS_TRACK=3；最差 seam_abs nemesis_loop .8079、ratio Acoustic_High_Collar 20.02、rms_jump p_egg_loop .3772、dc_delta p_egg_loop .0228；第二次 decode 位相同；输出 `work/out/m11_audio_loop_manifest.md`（gitignored）。 |

验证门禁（Lead 集成后）：`cargo test -q --workspace --no-fail-fast -j 2` 全绿（CARGO_EXIT=0）；`wolf-cli verify --root work/all` PASS 70 / pending 0。
