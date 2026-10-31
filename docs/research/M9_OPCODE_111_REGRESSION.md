# M9 W5 · opcode 111 值操作数「引用语义」补丁的回归报告（PARKED）

日期：M9 W5 · 归属任务 task-53 · 状态：补丁已准备并**暂存（PARKED）**，未落地 HEAD

补丁包：`work/out/opcode111_landed.patch`（`git diff` 覆盖 `crates/wolf-vm/src/exec.rs` + `crates/wolf-vm/tests/opcode_111_semantics.rs`，175 行）。

## 结论（先看这个）

- opcode 111 值操作数走 `eval_num` 的**语义是正确的**，对应文档 `docs/research/M7_OPCODE_111.md`（356/22695 站点为变量引用，1.57%）。
- 但把该补丁**真正落地**到当前 HEAD（bcbcfba）后，`cargo test -q -p wolf-game` 会有 **3 个战斗测试变红**；`cargo test -q -p wolf-vm` 仍全绿（91 unit + 集成，`opcode_111_semantics` 5 passed / 0 ignored）。
- 为了不阻塞正在并行推进的战斗布线任务（task-49/task-50），Lead 决定**暂存补丁、不落地**。本文件记录：3 个失败测试、正确语义下的根因、CE239/241/242/243/244 的 opcode-111 站点清单，以及战斗布线/测试重推后的落地计划。
- 归因已实测（revert↔restore 双向）：补丁是这 3 个失败的**唯一直接原因**，非并发改动。

## 三个变红的 wolf-game 测试（精确证据）

| # | 测试 | 文件:行 | 断言/日志 | 观测（打补丁后） | 正确语义解释 |
|---|------|---------|-----------|------------------|--------------|
| 1 | `m7_ce244_direct::the_win_lose_display_branch_opens_only_on_the_raw_self_magic` | `crates/wolf-game/tests/m7_ce244_direct.rs:137` | 「2000349=11 expects outcome 1」 | `left=-1 right=1` | 解析后 `11 == self1600010(=11)` 进入显示分支，旧测试把"字面量不命中"当作预期 |
| 2 | `m7_battle_target_scheduler::battle_wiring_advances_to_phase_two_but_ce244_gate_blocks_settlement` | `crates/wolf-game/tests/m7_battle_target_scheduler.rs:363` | 「command phase advances to 2 and then cannot leave」 | `left=0 right=2` | CE242/CE243 的 `var2000029 vs 2000028` 类值操作数解析后改走不同分支，`var350` 未再到 2 |
| 3 | `m7_day4_playable::day4_real_battle_settles_and_reaches_day5_without_injection` | `crates/wolf-game/tests/m7_day4_playable.rs:570` | 「the round must settle on the genuine day-4 replay」 | `v350_seq=[0,1,2] ce241=false ce239_alive=true ce239_pc=Some(13) v352_peak=0 v343_peak=0` | 结算变量 `2000352`/`2000343` 从未置位，CE244 结算未达成 |

> 另有 1 个与本补丁无关的失败：wolf-game lib 测试 `picture::tests::place_only_shifts_the_centre_anchor`（`crates/wolf-game/src/picture.rs:1746`，left `[80,50,20,10]` right `[100,50,20,10]`），由并发队友当时正在编辑 `crates/wolf-game/src/picture.rs` 导致，**不属于本补丁**。

## 根因（为什么"正确语义"反而让现有战斗测试红）

旧解释器把 opcode 111 的 value 操作数当**原始整数**比较；战斗事件 CE239–244 里存在大量 value 是**魔法操作数（变量/自身变量引用）**的条件，旧语义下这些条件按"字面量常量"判定。改成 `eval_num` 后它们变成真实的变量比较，**实质改变战斗控制流**（3 态机 `2000350`、回合/结算 `2000352/2000343`）。现有战斗测试是在旧的（错误）字面量语义下推导出来的，因此必须**在正确语义下重新推导**——这既是测试预期问题，也可能暴露更深的战斗布线缺口（见下"计划"里的 (b) 分支）。

## CE239/241/242/243/244 的 opcode-111 魔法值操作数站点（实测扫描）

扫描源：`work/_ce_full.json`；判据：opcode 111 条件三元组中 `value >= 1_000_000`（即魔法操作数，会被 eval_num 解析）。下列均为会受补丁影响的站点：

- **CE239 ■戦闘呼び出し**（1108 cmds）：
  `pc181 111 var2000341 value1600072 op1`；`pc186 111 var2000341 value1600073 op3`；`pc811/827/848 111 var1600065 value2000188 op3`
- **CE241 ┣ターン開始処理**（635 cmds）：
  `pc125 111 var2000349 value1600030 op1`；`pc357/381/421/518 111 var1600020 value2000188 op3`；`pc428/471 111 var2000029 value2000028 op1`
- **CE242 ┣コマンド処理**（1438 cmds）：
  `pc184/1099/1338 111 var2000029 value2000028 op1`；`pc263/375/528/705/826/1009 111 var1600030 value1600013 op3`；`pc451/875/953 111 var1600030 value1600040 op3`；`pc916 111 var1600030 value1600013 op3`
- **CE243 ┣敵の行動**（1316 cmds）：
  `pc46 111 var1600010 value1600031 op1`；`pc490/813/995 111 var1600030 value1600012 op3`；`pc952 111 var2000029 value2000028 op1`；`pc1005/1054 111 var1600030 value2000194 op3`；`pc1024/1071 111 var1600030 value1600013 op3`；`pc1178 111 var1600013 value1600060 op1`
- **CE244 ┗終了判定**（432 cmds）：
  `pc36 111 var2000349 value1600010 op2`（=胜负显示门）；`pc44 111 var2000192 value2000193 op0`；`pc132 111 var2000029 value2000028 op1`

要点：`2000029 vs 2000028`（普通变量相互比较）、`16000xx`（自身变量/参数，如 1600010=11、1600013/1600030/1600031/1600040/1600060/1600072/1600073）以及 `2000188/2000193/2000194` 这些站点在正确语义下不再当常量，而是取运行时值参与比较——这正是 3 态机与结算分支翻转的来源。

## 落地计划（在 task-49 / task-50 完成后）

前置：等 task-49/task-50（战斗布线/命令流程重推）合并，避免与并行改动互相踩踏。**在此之前不编辑任何战斗测试或 `battle_wiring.rs`。**

1. 在任务分支上应用补丁：`git apply work/out/opcode111_landed.patch`（或等价地把 `exec.rs` 那一行改成 `eval_num(...)` 并合入测试改动）。
2. 带补丁运行战斗相关测试，抓取"正确语义下"的真实输出：
   `cargo test -q -p wolf-game --test m7_ce244_direct --test m7_battle_target_scheduler --test m7_day4_playable`，并加临时 trace（不改生产语义）打印 `var2000350/2000352/2000343/2000349/2000192/2000193/2000028/2000029` 的演化。
3. 对每个失败测试做二选一判定：
   - (a) **纯预期过期**（控制流本身正确，只是旧断言锁死了字面量语义）→ 更新测试为正确语义下的期望值（例如 CE244 pc36 门应判"2000349==self1600010"是否成立），并保留原始意图注释。
   - (b) **真实功能缺口**（正确语义下 3 态机卡住 / 结算不可达）→ 修 `battle_wiring.rs` 与 CE241/242/243 相关驱动逻辑，使 `2000350` 周期与 `2000352/2000343` 结算在正确语义下成立；这才是"让战斗真正按原版语义跑通"。
4. 目标门：带补丁时 `cargo test -q -p wolf-game` 与 `cargo test -q -p wolf-vm` **同时全绿**，且 `wolf-cli verify` PASS 70/0。
5. 两者都绿后，由 Lead 正式取消 PARKED、把补丁落入 HEAD 并更新 `docs/research/M7_OPCODE_111.md` 状态为"已落地"。

## 相关

- 语义与证据链：`docs/research/M7_OPCODE_111.md`（结论/356 站点扫描不变，仅本任务状态标记改为 PARKED）。
- 补丁内容：`work/out/opcode111_landed.patch`。
- 未受本补丁影响的既有残余：op17（2 个站点）在 `compare` 里仍落 `_` 兜底当 `==`，语义未定义，另行处理。

## M9 复测（task-49 落地 battle_wiring 后 · --no-fail-fast）：失败面为 15 个测试二进制 / 15 个用例

复测方法（关键：必须 `--no-fail-fast`，否则 cargo 在第 1 个失败目标即停止，会漏报）：
- 打补丁：`git apply work/out/opcode111_landed.patch` → `cargo test -q --workspace --no-fail-fast` → **15 个测试二进制 FAILED**（日志 `work/out/t53_ws_patch.log`）。
- 回退补丁（`git checkout -- crates/wolf-vm/src/exec.rs crates/wolf-vm/tests/opcode_111_semantics.rs`）→ 同一命令 → **93 个测试二进制 ok / 0 failed**（日志 `work/out/t53_ws_base.log`）。
- 归因结论：这个补丁是这 15 个失败的**唯一直接原因**（同一工作树、仅差这两个文件）；不是并发改动、也不是 task-49 的 battle_wiring。

| # | 失败用例 | 文件:行 | 期望 | 实测 |
|---|----------|---------|------|------|
| 1 | m6_battle_commands_real_attack_selectable_with_inventory | crates/wolf-game/tests/m6_battle_commands.rs:144 | control.reached_ce192==true | false（CE192 未到达） |
| 2 | m6_battle_real_reaches_command_selection | crates/wolf-game/tests/m6_battle_real.rs:158 | 到达 CE192 | ce143=true ce192=false ce157=false |
| 3 | m7_battle_full_settles_via_ce244_and_pins_real_entry_gap | crates/wolf-game/tests/m7_battle_full.rs:368 | CE188 线到 CE152 | ce143=true copyHP=100/100 ce152=false ce192=false |
| 4 | m7_battle_menu_text_is_seeded_and_the_line_runs_clean | crates/wolf-game/tests/m7_battle_menu_text.rs:183 | control 到 CE192 | false |
| 5 | m7_battle_round_real_line_and_ce244_blocker | crates/wolf-game/tests/m7_battle_round.rs:255 | CE188 到 CE152 | false |
| 6 | m7_battle_seed_makes_a_real_attack_selectable | crates/wolf-game/tests/m7_battle_seed.rs:144 | control 到 CE192 | false |
| 7 | battle_wiring_advances_to_phase_two_but_ce244_gate_blocks_settlement | crates/wolf-game/tests/m7_battle_target_scheduler.rs:363 | 阶段==2 | left=0 right=2 |
| 8 | the_win_lose_display_branch_opens_only_on_the_raw_self_magic | crates/wolf-game/tests/m7_ce244_direct.rs:137 | outcome==1 | left=-1 right=1 |
| 9 | day4_real_battle_settles_and_reaches_day5_without_injection | crates/wolf-game/tests/m7_day4_playable.rs:570 | 结算达成 | v350_seq=[0,1,2] ce241=false v352_peak=0 v343_peak=0 ce239_alive=true |
| 10 | real_ce188_command_window_registers_a_chosen_non_default_command | crates/wolf-game/tests/m8_battle_command_input.rs:163 | CE65 运行 | menu_seen=false ce65=false ce192=false |
| 11 | multi_member_commands_map_to_distinct_party_pools | crates/wolf-game/tests/m8_battle_party.rs:183 | cmd5 battle decided==1 | left=0 |
| 12 | the_sleep_event_rolls_over_to_the_next_morning | crates/wolf-game/tests/m6_mainline.rs:427 | instructions>10_000 | 9792 |
| 13 | every_training_option_advances_the_clock_and_applies_its_real_effect | crates/wolf-game/tests/m6_training_menu.rs:162 | before_energy(var14)==100 | 60 |
| 14 | h_mode_var_table_is_derived_from_the_retail_save | crates/wolf-game/tests/m7_hmode_state.rs:241 | fresh day-2 var14==115 | 35 |
| 15 | sibling_wrappers_reach_ce246_ce217_ce346_and_pin_ce323_ce372_ce328_ce402 | crates/wolf-game/tests/m8_hscene_forward.rs:890 | CE372 选中 var218==2 | left=0 |

### 具体语义冲突机制（以战斗链路为例，已用临时探针实测事件链）

- CE188→…→CE144→CE147→**CE172 敵/味方·勝敗判定**。CE172 含 4 个魔法值操作数条件：
  `pc32 111 [1,1600023,1600020,2]`、`pc53 111 [1,1600023,1600020,2]`（self1600023 == self1600020）、`pc92 111 [1,1600024,1600035,1]`、`pc107 111 [1,1600024,1600036,1]`。
- 旧"字面量"语义下 `1600023 == 1600020` 恒不成立（RHS 被当常量 1,600,020），于是 CE172 一路落到 pc39 之后正常返回 → 调用方继续 → CE191→CE65→CE192 命令选择。**这正是当前全绿工作树的行为。**
- 正确引用语义下这些条件解析为真实自身变量比较，CE172 在 pc32/pc53 取到 else/命中分支后 `pc36 172 BreakEvent` 提前结束并写 `self1600099` 结果标志 → 调用方据此走战斗结束清理（事件链实测变为 172→CE199 主人公DB复原→CE149 战闘DB→主人公DB→CE201 战闘終了処理），**CE192 永远到不了**。战斗流程此前是靠"字面量 bug"跳过胜负判定的——这是真实语义冲突，不是单纯陈旧断言。
- 非战斗失败同理：var14 能量初值（training 100→60、hmode 115→35）、睡眠脚本指令数（>10000→9792）、CE372 阶段选择等，都是被通用 111 魔法值操作数改变的初始化/分支。

### 更新后的结论与建议

- "3 个失败" 更正为 **15 个测试二进制**（11 战斗 + 4 非战斗）。这不是"少数 stale expectation"：多数是真实行为/初始化差异（例如 CE172 胜负判定被激活、var14 初值变化），所以**不能把本补丁当作一行改动直接落地**。
- 建议：保持 **PARKED**；新开一个里程碑级 task，逐条判定 15 条为"陈旧预期（更新断言）"还是"真实缺陷/初始化缺失（修战斗与初始化逻辑，例如给 CE172 提供正确的存活/总数计数、var14 能量初值来源）"，重推后再由 Lead 落地补丁并取消 PARKED。
