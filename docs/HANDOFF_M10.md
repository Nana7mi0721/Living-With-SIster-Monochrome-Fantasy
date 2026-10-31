# M10 交接：opcode 111 引用语义落地、战斗重基线、残项收口

> 仓库：`E:\Study\github program\imoto life`
> 阅读顺序：本文件（M10 最新）→ `docs/README.md`（文档总索引）→ `PLAN.md`（当前路线图）；历史 M9/M8/M7/M6/M4/M3 交接已标为历史归档，需要追溯时从索引进入。
> 基线：M9 集成 HEAD `22148d8`；M10 由 `ce680b3`（残项）与 `3bbbb12`（opcode 111 落地）两个提交组成。`docs/gpt调研/` 与 `.claude/` 不是项目文件，不提交。

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
| 11 | `m9_battle_round_gap.rs` | 单回合 v350=[0,1,0] | `+round_loop` 结构断言：首尾 0、phase2 已入、≥2 回合、CE241/CE243 出现、v352∈{1,-1} |
| 12 | `m9_full_route.rs` | pre_power<200 未结算 | `pre_power>=200`（201 高能量分支）+round_loop 到 day5 08:30 |

## 5. 已确认边界 / 未完成

1. **自然路线 RNG 抖动**：`started_events`（HashSet，进程级 RandomState）的遍历顺序影响战斗前 RNG 消费，自然 day4 战斗在 3~4 回合、胜负 ±1 之间摆动。故 #11 用结构断言、#12 断言 v352∈{1,-1}；直接加载 SaveData01 的测试是确定的。PRNG 零售时钟种子重放等价性仍未证（H-PRNG-1）。
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

1. **战斗确定性**：定位 `started_events` HashSet 遍历顺序对预战斗 RNG 消费的量化影响；让自然路线可逐位复现（或显式排序），再把 #11/#12 改为定值断言。
2. **H 完整 CG 分支**：把合成存档的 gate 组合拆成多个测试或做成数据补丁，不要硬改真实存档。
3. **音频人工听感**：按 `M10_AUDIO_RESIDUAL.md` 的 18 个 heuristic 候选听一遍循环点；调查 kana.mp3 来源。
4. **像素**：锚点 5/6 零售画面对照；窗口皮肤 CN patch 右缘数据放置。
