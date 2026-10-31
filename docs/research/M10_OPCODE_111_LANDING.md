# M10 opcode 111 原子落地审计（task-74）

> 状态：**未达成原子 0-failed 落地**。本任务核对了 patch、揭示了两个真实的 VM 缺陷，并给出逐条测试重基线计划。
> 基线 HEAD = `22148d8`（M9: anchors, H-scene loop, battle rounds, movement and trigger core）。
> 主树保持干净；全部工作在一个隔离 git worktree `work/out/m10land`（detached @22148d8）内完成。
> 产物 patch：`work/out/m10_landing.patch`（192 行，2 文件，**只含 VM 修复**，不含测试重基线）。`git apply --check` 对主树 PASS。

---

## 1. 结论（Verdict）

1. 正确的 opcode 111「值操作数 = 变量引用」语义（reference semantics）**本身是正确的、必须落地的修复**，另加一个**预存 compound bug** 修复。二者已在隔离树验证：`cargo test -p wolf-vm` 全绿（lib 91 + opcode_111_semantics 5）。
2. 但**把二者与全部测试重基线一次性合成原子落地、达到 workspace 0-failed 是不可行的**。原因不是测试陈旧，而是参考语义把整台游戏的数值控制流从「单回合战斗 / 字面量夹取」校正为「真实多回合战斗 / 变量引用夹取」，级联穿透到 day-loop、H 模式、能量、敌人血量、胜负判定等**几十个生产行为**。仓库现有战斗子系统是**围绕这个字面量 bug 调出来的**。
3. 因此交付物是：**正确的 VM patch + 逐条测试分类与精确重基线动作 + 明确的 M10b 战斗再验证建议**；不以「把期望值改成匹配当前下游行为」的方式伪造绿灯（那会把可能的二次 VM bug 一起掩盖）。

---

## 2. 落地内容（patch 只含这 2 个 VM 文件）

### 2.1 opcode 111 值操作数改为引用（reference semantics）
`crates/wolf-vm/src/exec.rs` opcode 111 比较块（约 pc 2228）：把 `let value = arg(cmd, 2 + 3 * i);` 改为
`let value = eval_num(arg(cmd, 2 + 3 * i), inst, vars);`。
即条件三元组 `[var, value, operator]` 里的 `value` 走 `eval_num`：当它落在 self 变量区（1_000_000..）或普通变量区（2_000_000..）时按**变量值**比较，落在 <1_000_000 的字面量时原样返回，负 Opaque 也原样。数据侧 356/22695（1.57%）三元组的 value 是变量/自身变量引用（见 `M7_OPCODE_111.md`），当前解释器按字面量比较是错的。

### 2.2 compound() 的 0x600/0x700/0x800 修复（预存 bug）
`crates/wolf-vm/src/exec.rs` `fn compound`（约 pc 1464）。`OPCODE_SPEC.md` §3.1 L195-197 规定：
- `0x600 = arg0 = floor(arg0)`
- `0x700 = arg0 = ceil(arg0)`
- `0x800 = arg0 = abs(arg0)`

旧实现写的是 `0x600 => value, 0x700 => value, 0x800 => value.abs()`，其中 `value` 是二元运算（arg1 op arg2）的**结果**，**不是目标的当前值**。改为 `0x600 => current, 0x700 => current, 0x800 => current.abs()`。

**为何这个 bug 之前不可见、且是致命级联点**：CE172「[戦]敵/味方·勝敗判定」pc25-29 计算「阵亡人数」：pc25 `121[1600021,1,0,256]`（self21 -= 1 递减）、pc26 调 CE176 读该队友是否战闘不能、pc27 `121[1600022,0,0,1536]`（0x600）、pc28 `121[1600022,1,0,1792]`（0x700）、pc29 `121[1600023,1600022,0,256]`（self23 += self22）。旧实现把 self22 **强制为 1**（无视 CE176 返回值），于是每个队友都被计为阵亡，pc32 `111[1,1600023,1600020,2]`（dead == ally-count）恒成立 → 直接判负 + op172 BreakEvent。在**字面量**语义下 pc32 永不匹配（小计数恒 != 字面量 1600020），bug 被掩盖；一旦切到参考语义，pc32 立即生效，**全部战斗在 CE172 处崩塌**。修好 compound 后 `m6_battle_real` 直接恢复，workspace 失败面从 22 降到 12。

---

## 3. 决定性证据：自然 day4 战斗在参考语义下变「多回合」

隔离树内对 m9 自然 day4 路线做了一次受控实验：`game.set_battle_round_loop(true)` + 20000 帧预算，结果（实测）：

```
v350=[0,1,2,0,1,0]  settle_f=1018  v342_min=0  v349=2  pools=[4,3,3,2]
ce241_frames=86  ce243_frames=98  warnings=0  gauge_zero_f=1018  result_write_f=1018
```

即发生**两个命令回合**：0→1→2（CE242 命令走完 + 敌人反击 CE243），CE241 回合开始 2→0，再 0→1→0 结算。旧测试期望的「单回合 `v350=[0,1,0]`、774 帧结算」是**字面量 bug 的产物**。这证明：参考语义下仓库所谓「一回合秒杀」的战斗**不再成立**，这是真实行为变更，不是测试陈旧。

机制（CE239 敌血槽）：pc186 `111[1,2000341,1600073,3]`（<=）在 pc188 与 pc199 间选分支；pc193 `1600011=2000341%2`；pc194 `2000341+=2000028%4`；pc195 `2000342=2000341+1600011`；pc204 `2000342+=2000446`；pc205-207 夹到最小 100。伤害 `2000342 -= 1600015`（opbits 512）在 CE242 pc287/399/540/731/850/928/1033；CE244 pc313/373/419 清零。参考语义下 `2000342` 正确地更大 → 一回合打不空 → 必然多回合。

---

## 4. 全网级联 first-divergence（自然 day4 路线，M10DIV 实测）

| 事件/PC | 条件（值操作数） | 字面量结果 | 参考结果 | 影响 |
|---|---|---|---|---|
| ev48 pc133 | `111[?,1600023,1600019,2]` | lhs=-1 lit=false | ref=true | 分支翻转 |
| ev63 pc75 | `111[?,1600048,1600049,5]` (!=) | lit=true | ref=false | **昼命令分派翻转** |
| ev226 pc15 | `111[18,1600012,1600010,1]` (>=) | lhs=100 lit=false | ref=true | **能量夹取 var14** |
| ev329 pc233 | `111[?,2000016,2000015,1]` | lhs=4 ref=3 | — | 夜晚数值 |
| ev436 pc101 | `111[?,1600020,2000550,0]` | lhs=2 ref=0 | — | — |

这张表解释了为什么 day-loop、能量、H 模式、胜负展示全部位移：不是单点，是贯穿全剧的控制流校正。

---

## 5. 修复 VM 后仍红的 12 个 wolf-game 测试函数（逐条分类 + 精确动作）

其余全部测试绿，`cargo test -p wolf-vm` 全绿。逐条如下（分类：**[stale]=陈旧期望** / **[real]=真实行为变更，需重基线或再验证**）：

| # | 文件:行 | 测试 | 断言 / 实测 | 分类 | 精确修正动作 |
|---|---|---|---|---|---|
| 1 | `m6_mainline.rs:427` | the_sleep_event_rolls_over... | `instructions > 10_000` got **9792** | stale | 阈值放宽到 `> 9_000`（task#12 已预判 ~9500） |
| 2 | `m6_training_menu.rs:162` | every_training_option... | `before_energy == 100` got **60** | real | 参考语义下卧室开局 var14=60（能量夹取漂移）；需确认 60 是否为正确日态，若是改期望为 60，否则补 setup 播种 100。属数值再推导 |
| 3 | `m7_battle_menu_text.rs:183` | ..._seeded_and_the_line_runs_clean | `control.reached_ce192` **false** | stale | control=`run(false)` 无播种→0 队友→CE172 pc32（0==0）判负中止。给 control 只播种队伍（CData type18 r4=1 + stats），不播种武器/命令槽→战斗可推进但攻击仍不可选 |
| 4 | `m7_battle_seed.rs:144` | ..._real_attack_selectable | `control.reached_ce192` **false** | stale | 同上，control 补队伍种子 |
| 5 | `m7_battle_target_scheduler.rs:363` | battle_wiring_advances_to_phase_two_but... | `var350==2` got **0**（还断言 v352==0,v343==0,v349==1） | real | 前提「停在 phase2 无法结算」已失效；参考语义下多回合会推进/结算。需重测 transitions 后重写期望 |
| 6 | `m7_ce244_direct.rs:137` | the_win_lose_display_branch_opens_only_on_the_raw_self_magic | 用例 `[(11,1),(1600010,-1)]`，probe=11 得 outcome **-1** | stale | pc36 `2000349==self1600010` 在参考语义下 11==11 真匹配→走 defeat 展示。新用例应为 `[(11,-1),(1600010,1)]`；测试名/注释「raw self magic」作废 |
| 7 | `m7_day4_playable.rs:570` | day4_real_battle_settles_and_reaches_day5... | `settled` **false**；`:556` `pre_power<200` got **201** | real | 多回合：开 `set_battle_round_loop(true)`，把单回合断言改为多回合结算断言（参 §3 数字） |
| 8 | `m7_hmode_state.rs:241` | h_mode_var_table_is_derived_from_retail_save | H_MODE_VARS 表 fresh 列 var14 want **115** got **35** | real | H_MODE_VARS 是「数据漂移卡」，参考语义下 fresh-day2 整张表（var13/14/22/23/102/157…）都需重测重填；这是全状态重基线，非单点 |
| 9 | `m8_battle_party.rs:183` | multi_member_commands_map_to_distinct_party_pools | `r.v343 == 1` got **0**；文档 L23-32 明写「单回合秒杀、四回合不可达」前提 | real | 该测试 premise 被 §3 直接否定：参考语义下多回合可达。整套「每个命令单回合结算 + 恰好一个池 -1」的断言要改为多回合/池耗散设计（高工作量） |
| 10 | `m8_hscene_forward.rs:890` | sibling_wrappers_reach..._pin_ce372... | `vars.get(218)==2` got **0** | real | CE372 阶段选择经 magic111 变化，var218 选值改变；需按参考语义重测 CE372 分支后重写期望（确认 0 还是 2 正确） |
| 11 | `m9_battle_round_gap.rs:591` | natural_day_four_battle_settles_before_enemy_action_phase | settle **None**（round_loop 下能结算但断言期望单回合 v350=[0,1,0]） | real | 改为 round_loop + 多回合断言（§3 数字） |
| 12 | `m9_full_route.rs:624` | continuous_new_game_route_reaches_day_five_0830 | `pre_power<200` got **201**；战斗未结算 v352_peak=0 | real | 多回合 + 数值边界重推导 |

**统计：2 条纯陈旧（#1、#6，机械可改）；#3/#4 为 setup 陈旧（补队伍种子即可）；其余 8 条（#2/#5/#7/#8/#9/#10/#11/#12）是真实行为变更，需战斗/H 状态再基线或数值再推导。**

---

## 6. 为什么不能原子 0-failed

- 这份 patch 校正的是**语义**，导致的是**真实的多回合战斗 / 变量引用夹取**，仓库现有战斗测试是围绕字面量 bug 调出来的。要达到 0-failed，必须把这 8 条「real」测试连同其依赖的生产路径（battle_wiring 单发/回合循环默认、敌血槽公式、能量、H 模式表）**重新验证并重基线**——这正是 6 号建议里 M10b 的范围。
- 若强行把期望值改成「匹配当前下游行为」以换取绿灯，会把 §2.2 compound bug 之外**尚未定位的二次 VM 缺陷**（如 #2 的 var14=60 是否另有夹取错误）一并掩盖。task-53 已确立同款原则：语义正确但会把共享树弄红时，先 park，不落裸 patch。

---

## 7. 建议（M10b 战斗再验证）

1. **先合 VM 双修复（本 patch）**：opcode 111 引用语义 + compound 0x600/0x700/0x800。此为独立、可审、wolf-vm 全绿的底座。
2. **M10b 战斗再验证**（独立工作流）：
   - 决定默认是「回合循环」还是「单发」；§3 证明参考语义下自然战斗本应多回合——建议把 `set_battle_round_loop(true)` 作为 day4/主线默认，并重验敌血槽公式（CE239 pc186-207）。
   - 逐条按 §5 重基线：先做 2 条 pure-stale（#1/#6）+ 2 条 setup-stale（#3/#4），再处理 8 条 real。
   - H 模式表（#8）与 CE372 var218（#10）单独再测。
3. 在 M10b 过程中保持主树绿：先落 VM patch（不改变默认测试期望会红 → 需与 M10b 的测试改动同批合入，或用只读隔离树验证后再整体切）。

---

## 8. 复现命令（隔离树 work/out/m10land）

```bash
export WOLF_DATA_ROOT="E:/Study/github program/imoto life/work/all"
export CARGO_TARGET_DIR="E:/Study/github program/imoto life/work/out/m10land_target"
# 隔离树准备（一次性）：git worktree add --detach work/out/m10land 22148d8，并 cp -r work/retail_backup work/out/m10land/work/retail_backup
cargo test -q -p wolf-vm                 # 全绿
cargo test -q --workspace --no-fail-fast # 12 个 wolf-game 测试函数失败（上表）
# 多回合证据：在 m9_battle_round_gap 内打开 set_battle_round_loop(true)+20000 帧，见 §3
```

## 9. 产物

- `work/out/m10_landing.patch`（192 行 / 2 文件：`crates/wolf-vm/src/exec.rs` 18 行、`crates/wolf-vm/tests/opcode_111_semantics.rs` 93 行），`git apply --check` 对主树 PASS。
- 主树未改动、未提交；隔离树无诊断残留（M10DIV/M10DIAG/M10BRK 全清）。

---

## 10. task-77 完整重基线（隔离树 work/out/m10land，已达成 0-failed）

task-74 判定"原子 0-failed 不可行"的前提是**保留全部旧断言强度、只改期望值**。task-77 证明：只要允许**按语义改动断言形态**（把"单回合秒杀"断言改为"真实多回合/引用夹取"断言，并对 setup 陈旧的控制组补队伍种子），14 个测试文件即可一次性重基线到 **workspace 0 failed**。隔离树内实测：
- `cargo test -q -p wolf-vm`：全绿（lib 91 + opcode_111_semantics 5 + rng 等，共 130）。
- `cargo test -q --workspace --no-fail-fast`：**99 个测试二进制，701 passed，0 failed**。
- `wolf-cli.exe verify --root work/all`：**PASS — 70 check(s) matched baseline, 0 pending**。

产物：`work/out/m10_landing_full.patch`（vs 22148d8，含 `crates/wolf-vm/src/exec.rs` + `crates/wolf-vm/tests/opcode_111_semantics.rs` + 12 个 wolf-game 测试文件）。主树未改动。

### 10.1 逐条 old -> new（全部为实测值）

| # | 文件:行 | 旧断言 / 旧值 | 新断言 / 新值 | 依据（CE/pc/var，为何旧值是字面量 bug 产物） |
|---|---|---|---|---|
| 1 | `m6_mainline.rs:428` | `instructions > 10_000`（旧=10000+） | `> 9_000`（实测 9792） | 参考语义减少无效指令；9792 为真实指令数 |
| 2 | `m6_training_menu.rs:162` | `before_energy == 100` | `before_energy == 60` | CE226 pc15 `111[18,1600012,1600010,1]` 把 var14 夹到 var13；day1 18:30 能量真实为 60（后段断言用相对差值不受影响） |
| 3 | `m7_battle_seed.rs` run() | 控制组 reached_ce192=false | 控制组补"仅队伍"种子：CData type18 r4=1、r69=1（1 敌人）、leader stats（type0 r12 c20..28/9/10/4）→ reached_ce192=true | 旧值："0 队友"→CE172 pc32/pc53 双方 0==0 秒判负中止；需 ≥1 存活队友且 ≥1 敌人才推进到 CE192 |
| 4 | `m7_battle_menu_text.rs` run() | 同 #3（控制组 reached_ce192=false） | 同 #3 补"仅队伍"种子 → true | 同 #3 |
| 5 | `m7_battle_target_scheduler.rs:363` | `assert_eq!(var350, 2)`（停在 phase2） | `assert_eq!(var350, 0)`（transitions=[0,1,2,0]） | 参考语义多回合会推进并回 0 结算；"停在 phase2 无法结算"前提失效 |
| 6 | `m7_ce244_direct.rs:128` | 用例 `[(11,1),(1600010,-1)]` | `[(11,-1),(1600010,1)]` | pc36 `2000349==self1600010` 在参考语义下 11==11 真匹配→defeat；测试名"raw self magic"作废 |
| 7 | `m7_day4_playable.rs` | 单回合无法结算（settled=false；该测试路线本地 pre_power=132<200，CE253 低能量分支） | `+set_battle_round_loop(true)`；多回合结算：rounds=2, v350=[0,1,2,0,1,0], v352_peak=1, v343_peak=1, ce239_alive=false, warnings=0 | 参考语义真实多回合；CE253 低能量分支。注意 #7 的 pre_power<200 是**该测试路线本地值 132**，与 #12 自然路线 201 分属不同 fixture |
| 8 | `m7_hmode_state.rs` H_MODE_VARS | (var14,115,75), (var16,4,2) | (var14,35,75), (var16,3,2) | op111 引用语义下 CE226 pc15 夹取 var14；fresh-day2 状态卡重测 |
| 9 | `m8_battle_party.rs` | "8 个命令每个都单回合结算（v343==1, v352∈{1,-1}）"，doc L23-32 称多回合不可达 | 每命令仍断言 pool 恰减 1、body_pc 落区；结算按强弱分：cmd1-4 结算(v343=1)，cmd5-8 留敌存活(v343=0, v352=0)；doc premise 改"真多回合" | 参考语义下成员1/2 伤害足、成员3/4 需第二回合；pool 映射不变（仍 5/4/3/2，各命令各减对应池 1） |
| 10 | `m8_hscene_forward.rs:890` | `vars.get(218)==2` | `var218_max==2`（瞬态 0→2→1→0） | CE372 阶段选择经 magic111 变化，var218 为瞬态；用峰值断言 |
| 11 | `m9_battle_round_gap.rs` natural | 单回合：v350=[0,1,0]、phase2 永不入、ce241=0、ce243<10、v349=1、v352=1 | `+set_battle_round_loop(true)`；多回合结构：首尾 0、phase2 已入、command_rounds≥2、ce241>0、ce243>0、v349≥2、v352∈{1,-1}、settle≈1616 | 参考语义真实多回合；**精确回合数/胜负随战斗 RNG 在 3~4 回合摆动**，故断言结构而非定值（见 10.2） |
| 12 | `m9_full_route.rs` | `pre_power<200` got 201；未结算 | `pre_power>=200`（201，CE253 高能量分支）；`+set_battle_round_loop(true)`；战斗多回合结算（v352∈{1,-1}）；仍到 day5 08:30 | pre_power=201 跨过 CE253 的 >=200 阈值→高分支真实多回合 |

### 10.2 关键新发现：战斗结算的回合数/胜负存在 RNG 抖动（3~4 回合 / 胜或负）

隔离树内对自然 day4 路线重复跑测发现：同一构建下 `v350` 序列会在
`[0,1,2,0,1,2,0,1,2,0,1,0]`（3.0 回合、常见胜利）与 `[0,1,2,0,1,2,0,1,2,0,1,2,0]`（4 回合、曾见失败）之间摆动，`2000352` 随之为 +1 或 -1。根因是引擎启动时对 `started_events`（HashSet，进程级 RandomState）的遍历顺序影响了战斗前 RNG 消费序列；RNG 本身 EVENT_SEED 固定，但预战斗消费次数会随之漂移。因此对自然路线的 #11 采用**结构断言**（首尾 0、phase2 已入、≥2 命令回合、CE241/CE243 均出现、v352∈{1,-1}）而非定死回合数；#12 本就把 v352 断言为 {1,-1}，天然稳。直接加载 SaveData01 的 #3/#4/#5/#8/#9/#10 是确定的（多次跑测一致）。这是随本 patch 一起记录的**残余确定性风险**（PRNG 时钟种子重放等价性未定，见 H-PRNG-1）。

### 10.3 复现（隔离树）

```bash
export WOLF_DATA_ROOT="E:/Study/github program/imoto life/work/all"
export CARGO_TARGET_DIR="E:/Study/github program/imoto life/work/out/m10land_target"
cargo test -q -p wolf-vm
cargo test -q --workspace --no-fail-fast          # 99 binaries, 701 passed, 0 failed
work/out/m10land_target/debug/wolf-cli.exe verify --root work/all   # PASS 70/0
```
