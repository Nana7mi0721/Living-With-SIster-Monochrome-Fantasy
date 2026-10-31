# M10 Phase 2 前置 · 战斗测试期望重导表（DOCUMENT-ONLY）

Status: 规划/证据文档（task-73）。**本任务只写本文件；未编辑任何 test / src，未应用补丁，未 commit/push。**
仓库 `E:/Study/github program/imoto life`，HEAD `bcbcfba`（工作树含其它队友未提交批次）。
补丁实体：`work/out/opcode111_landed.patch`（PARKED，见 `docs/research/M9_OPCODE_111_REGRESSION.md`）。
相关：`docs/research/M10_OPCODE_111_PLAN.md`、`docs/research/M10_BATTLE_DB_FIXTURE.md`、`docs/research/M7_OPCODE_111.md`。

---

## 0. 这份文档要解决什么

opcode 111 的值操作数一旦按“引用语义”（`eval_num`）解析，`cargo test -q --workspace` 会有 **15 个测试二进制**变红。其中 **11 个是战斗测试**。本文对 11 个战斗测试逐一给出：

1. **现状断言**（精确到 `file:行`，即补丁下失败的那一条）；
2. **打补丁后的实测**（来自 `M9_OPCODE_111_REGRESSION.md` M9 复测表，`--no-fail-fast` 采集）；
3. **正确语义下的期望值**（该断言“应当”是什么 / 应当怎样重写）；
4. **为什么**（引用具体 CE / pc / 变量 / 夹具数据）；
5. **类别与修复方向**（a=陈旧预期；b=真实行为/数据前置；c=下游 magic111 first-divergence 待插桩）。

> 关键校正（以 `M10_BATTLE_DB_FIXTURE.md` task-66 实测为准，覆盖 `M10_OPCODE_111_PLAN.md` 第 0/3 节）：**CE172「敵/味方·勝敗判定」不是这 11 个测试的主开关**。CE190 在每次真实战斗都会把 `CData type18 row69` 由组表阵容重算（实测 1→7）并把落位单位标活（`type10 col3=0`），因此 CE172 pc32/pc53 的 `死数==总数` 恒不成立，永不中断。真正的分歧在 **CE188 下游** `CE147/CE65/CE152/CE161/CE160/CE163/CE64` 的 magic111 站点。本文据此把 `#1/#2/#5` 从 plan 的 “(b)-CE172” 改判为 **下游 (c)**。

---

## 1. 语义与判据速查（供全表引用）

- 补丁改动（`work/out/opcode111_landed.patch`，`crates/wolf-vm/src/exec.rs` `enter_block` opcode 111 分支）：
  - 旧：`let value = arg(cmd, 2 + 3 * i);`（value 操作数当**原始整数常量**比较）
  - 新：`let value = eval_num(arg(cmd, 2 + 3 * i), inst, vars);`（value 操作数**先求值**：魔数/自身变量/系统变量解引用，普通字面量 < 1_000_000 原样返回）
- op111 结构：`ints=[a0, (var,value,op) × n]`，`n = a0 & 0x0F`，`a0 & 0x10` 表示“带 else（后续 c420）”。operator：`1=>= 2=== 3=<=(<=) 5=!=`（`exec.rs` `compare`）。
- “magic 站点”判据：value **≥ 1_000_000**（自身变量地址形如 `1_600_000+slot`、系统变量 `2_000_xxx`、`1_600_xxx` 等）。全库 **约 328 个**这类 op111 站点（`M7_OPCODE_111.md` 更宽口径 356）。旧语义下它们几乎恒假/恒真，引用语义下变成真实比较。
- 两个不同的变量域务必分清：
  - **plain 变量**：op111 变量地址 `2_000_000 + n` 解析为 `vm.vars[n]`（如 `2000350`→`vars[350]`；`2000029`→`vars[29]`？注意：`2_000_029` 的低位是 `29`，见各 pc 原始串）。
  - **self 变量**：`1_600_000 + slot` 解析为该实例 `self_vars[slot]`（如 `1600010`→slot 10 = CE244 阈值；`1600001`→slot 1）。

---

## 2. 总表（11 个战斗测试）

| # | 测试（file:行） | 现状断言（补丁下失败那条） | 打补丁实测 | 正确语义下期望 | 类别 | 根因锚点 |
|---|---|---|---|---|---|---|
| 1 | `crates/wolf-game/tests/m6_battle_commands.rs:144` | `assert!(control.reached_ce192)`（空背包 control 也要到 CE192） | `reached_ce192=false` | **仍为 true**（CE192 在 CE152 之前，与背包无关，必须到达） | c | CE188 线早段 `CE147 pc37`（及 `CE65 pc160`/`CE152 pc82`）magic111 分歧 |
| 2 | `crates/wolf-game/tests/m6_battle_real.rs:158` | `assert!(reached_ce192)` | `ce143=true ce192=false` | **仍为 true**（同上；`seed_party` 只补 row4，但 CE190 会重算 row69） | c | 同上（CE147/CE65/CE152） |
| 3 | `crates/wolf-game/tests/m7_battle_full.rs:368` | `assert!(c.reached_ce152)` | `ce143=true copyHP=100/100 ce152=false` | **仍为 true**（CE143 已成功复制 HP，须继续到 CE152） | c | CE147 pc37 / CE65 pc160（CE152 尚未 start） |
| 4 | `crates/wolf-game/tests/m7_battle_menu_text.rs:183` | `assert!(control.reached_ce192)` | `false` | **仍为 true** | c | 同上 |
| 5 | `crates/wolf-game/tests/m7_battle_round.rs:255` | `assert!(a.reached_ce152)` | `false` | **仍为 true** | c | CE147/CE65/CE152 |
| 6 | `crates/wolf-game/tests/m7_battle_seed.rs:144` | `assert!(control.reached_ce192)` | `false` | **仍为 true** | c | 同上 |
| 7 | `crates/wolf-game/tests/m7_battle_target_scheduler.rs:363` | `assert_eq!(var(&g,350), 2)`（“进到 2 就出不来”） | `left=0 right=2` | 需重导：正确语义下 `2000350` **回到 0**（旧“卡在 2”是字面量 bug 的产物）；`352/343` 断言一并复核 | c | `CE242 pc184`（`2000029>=2000028`）+ `CE243 pc952` / `CE160` 一线 |
| 8 | `crates/wolf-game/tests/m7_ce244_direct.rs:137` | `assert_eq!(OUTCOME, want)`，cases `(349=11→1),(349=1600010→-1)` | 首例 `left=-1 right=1` | **两例翻转**：`349=11→-1`、`349=1600010→1` | **a 陈旧预期** | `CE244 pc3`(self1600010=11)、`pc36 111[1,2000349,1600010,2]`、`pc44 111[17,2000192,2000193,0]` |
| 9 | `crates/wolf-game/tests/m7_day4_playable.rs:570` | `assert!(settled)` | `v350_seq=[0,1,2] ce241=false v352_peak=0 v343_peak=0 ce239_alive=true` | **仍应为 settled=true**；但需在正确语义下重建结算（CE244 现在 fall-through，不写 352/343） | c | `CE244 pc36/pc44/pc58/pc85/pc132` + `CE242 pc184` 一线 |
| 10 | `crates/wolf-game/tests/m8_battle_command_input.rs:163` | `assert!(saw_ce65)` | `menu_seen=false ce65=false ce192=false` | **仍为 true**（真实键盘选 防御 后 CE65 必须运行） | c | CE147 pc37 → CE65/CE152 早段分歧 |
| 11 | `crates/wolf-game/tests/m8_battle_party.rs:183` | `assert_eq!(r.v343, 1)`（单条命令须结算） | `left=0`（`v343=0`） | **仍应为 v343=1**；正确语义下 CE244 需真正结算 | c | `CE244 pc36/pc44/pc58/pc132`、`CE242 pc184` |

分类小计（11 个战斗测试）：**a=1（#8）｜b=0（CE172 已被 task-66 排除）｜c=10（#1–7、#9–11）**。
（`M10_OPCODE_111_PLAN.md` 原把 #1/#2/#5 归 (b)-CE172 / #13/#14/#15 归 (b)，本文按 task-66 实测改为：11 个战斗测试中只有 #8 是纯 a，其余 10 个都是下游 (c) 或结算状态机分歧，需补丁插桩收敛 first-divergence。）

---

## 3. 逐条重导

### #1 `crates/wolf-game/tests/m6_battle_commands.rs:144`
- 夹具：`run(with_inventory,frames)`（`m6_battle_commands.rs:83-135`）→ `load_save(SaveData01)`，`start_event(188)`，`f in 1..=60` 每帧 `seed_party`（仅写 `type18 row4=1` + `type0 row12` 命令槽 47..54=[3,4,6,7,0,0,0,0] + 数值列，**不写 row69**），`with_inventory` 时写 `type7 row0 col0=1`。
- 现状断言：`m6_battle_commands.rs:144-147`
  `assert!(control.reached_ce192, "control run must reach CE192 味方コマンド選択 ...")`
- 打补丁实测：`reached_ce192=false`（M9 表 #1）。
- 正确语义期望：**`reached_ce192 == true`（不变）**。CE192「味方コマンド選択」在 CE152「戦闘コマンド一覧算出」**之前**发生；背包只通过 CE152 pc89-95 的发動アイテム判定影响“攻击行是否可选”，与“是否到达 CE192”无关。所以这条 reach 断言本身是对的，不是陈旧预期。
- 为什么失败：`seed_party` 不写 `row69` **不**是原因——CE190「戦闘キャラ配置」在 188→189→190 时把 `type18 row69` 重算为组内真实阵容（实测 1→7）并把单位标活（`type10 col3=0`），CE172 pc32/pc53 的 `死数==总数` 不成立、不中断（task-66 §2/§4 实测）。真正的分歧在**下游** magic111 站点，最早可达为 `CE147 pc37 = 111[2,1600001,-1,2, 1600001,1600026,5]`（`self1600001==-1` else `self1600001!=self1600026`），其后 `CE65 pc160=111[1,1600011,1600024,1]`、`CE152 pc82=111[1,1600017,1600015,0]`、`CE161 pc31`、`CE160`、`CE163`、`CE64`。
- 类别/修复：**(c)**。修复方向：先用插桩（不改生产语义的临时 trace）在这些站点抓 first-divergence，再二选一——(i) 属数据前置缺失→补全战斗 DB 夹具（task-66 `seed_full_battle_db` 模板）并保持 `reached_ce192==true`；(ii) 属真实正确行为改变→按实测更新断言（但要说明 CE192 不再到达的正当理由）。

### #2 `crates/wolf-game/tests/m6_battle_real.rs:158`
- 夹具：`m6_battle_real.rs:37-59 seed_party`（`type18 row4=1`，命令槽 `[1,2,3,4,0,0,0,0]`，数值列），`start_event(188)`，每帧 `f in 1..=45` 重播 seed。
- 现状断言：`m6_battle_real.rs:158` `assert!(reached_ce192, "CE192 味方コマンド選択 never reached")`。
- 打补丁实测：`reached_ce143=true reached_ce192=false reached_ce157=false`（M9 表 #2）。
- 正确语义期望：**仍为 `reached_ce192 == true`**。CE143 已把真实 HP/maxHP 复制进 `type10 row0`（同测断言 `t10_maxhp=100,t10_hp=100` 仍成立），说明 188→143 正常；CE192 是后续应达里程碑。
- 为什么失败：与 #1 同源——非 CE172（CE190 重算 row69），而是 CE147/CE65/CE152 的 magic111 分歧。注意本测 `seed_party` 命令槽是 `[1,2,3,4,...]`（与 #1 的 `[3,4,6,7,...]` 不同），但两支都在 CE147 之后；first-divergence 需插桩确认是否同为 CE147 pc37。
- 类别/修复：**(c)**，同 #1。

### #3 `crates/wolf-game/tests/m7_battle_full.rs:368`
- 夹具：`m7_battle_full.rs:run_ce188_line`（同 188 线，`seed_battle_databases`/party 前置；日志行 360 打印 `SC0 ...`）。
- 现状断言：`m7_battle_full.rs:368` `assert!(c.reached_ce152, "CE188 line must reach CE152 戦闘コマンド一覧算出")`（同段 365-367 的 CE143/copyHP 断言仍成立）。
- 打补丁实测：`ce143=true copyHP=100/100 ce152=false ce192=false`（M9 表 #3；task-66 §3 `seeded-battle` 路径在**无补丁**下到达 `{…143,144,147,172,188,189,190,191,241,271}`，即 CE152 未列）。
- 正确语义期望：**仍为 `reached_ce152 == true`**（CE143 已完成，须继续到 CE152）。这是“到 CE152”的里程碑断言，不是锁死字面量 bug。
- 为什么失败：`ce152=false` 表示 CE152 事件尚未 start，分歧在 CE65（`pc160=111[1,1600011,1600024,1]`）或更早的 CE147 pc37。CE152 自身 `pc82=111[1,1600017,1600015,0]` 是“进入 CE152 之后”的站点，非本例首因。
- 类别/修复：**(c)**，插桩收敛 first-divergence 后按数据/行为二选一，期望值保持 reach=true。

### #4 `crates/wolf-game/tests/m7_battle_menu_text.rs:183`
- 夹具：`run(with_seed,frames)`（`m7_battle_menu_text.rs:62-110`）→ `Game::new`（不 load 存档！），`with_seed` 时 `seed_battle_databases()`（断言写 26 个数值单元），`start_event(188)`；断言命令名来自 `Data type6` 列0、由 CE152 写入 `CData type19` 列0（字符串写路径 task-39）。
- 现状断言：`m7_battle_menu_text.rs:183` `assert!(control.reached_ce192, "control must still reach CE192 味方コマンド選択")`。
- 打补丁实测：`control.reached_ce192=false`（M9 表 #4）。
- 正确语义期望：**仍为 `reached_ce192 == true`**。本测的“主证明”（`Data type6 row3..row7 コマンド名=蓄力/防御/强斩/反击`、`type19` 文本 seed 正确）都在 CE152 之前就与 111 语义无关；control 只是“空背包也到 CE192”的前置对照。
- 为什么失败：与 #1/#2 同源（CE188 线早段 magic111）。本测用 `Game::new` 无存档，party/enemy DB 为零，风险是 CE190 无法重算 row69（无组表阵容）→ 若阵容 0 则 CE172 可能 `0==0` 判胜——**但 task-66 已示：空阵容 0==0 只在“合成退化夹具（强制 r69=0 且不经 CE190 重算）”下发生**；`seed_battle_databases` 播 `row69=1`，且 CE190 若运行会重算。first-divergence 须插桩确认是 CE147 还是退化 CE172。
- 类别/修复：**(c)**，但需优先区分：若确属“`Game::new` 无存档→CE190 无阵容→row69 退化”则属**夹具数据前置 (c→数据)**，补全战斗 DB 后断言保持 true。

### #5 `crates/wolf-game/tests/m7_battle_round.rs:255`
- 夹具：`run_ce188_line`（`m7_battle_round.rs:84-120`），`seed_party`（`:53-66`，含 `type7 row0 col0=1` 使真实攻击可选），`start_event(188)`，每帧 `f in 1..=60` 重播 seed。
- 现状断言：`m7_battle_round.rs:255` `assert!(a.reached_ce152, "CE188 line must reach CE152 戦闘コマンド一覧算出")`。
- 打补丁实测：`reached_ce152=false`（M9 表 #5）。
- 正确语义期望：**仍为 `reached_ce152 == true`**。
- 为什么失败：同源早段 magic111（CE147 pc37 / CE65 pc160）。本测 Scenario A 后半段（CE157/CE158 target window 打开、`saw_ce244==false`）的断言是结构事实，不受本分歧影响，但都被 CE152 未到达而短路。
- 类别/修复：**(c)**，同 #3。

### #6 `crates/wolf-game/tests/m7_battle_seed.rs:144`
- 夹具：`run(with_seed,frames)`（`m7_battle_seed.rs:49-95`）→ `Game::new`，`with_seed` 时 `seed_battle_databases()`，`start_event(188)`；`seed_battle_databases` 已先断言播 `type18 row4=1,row69=1` 等（本测 108-136 行逐格断言）。
- 现状断言：`m7_battle_seed.rs:144` `assert!(control.reached_ce192, "control must still reach CE192 味方コマンド選択 (party/command precondition)")`。
- 打补丁实测：`control.reached_ce192=false`（M9 表 #6）。
- 正确语义期望：**仍为 `reached_ce192 == true`**；control（无 seed）到 CE192 是“seed 只影响可选性”的对照前提。
- 为什么失败：与 #4 同为 `Game::new` 无存档路径（CE190 阵容/row69 可能退化）。row69 已由 seed 播 1（控制组 control 不 seed，但 `Game::new` 默认 row69=0 → 有退化风险）。first-divergence 须插桩区分“CE172 退化 0==0” vs “CE147/CE65 下游”。
- 类别/修复：**(c)**，优先按“夹具数据前置”排查（control 组 row69=0 的退化分支）。注意：若确认是 control 组退化 CE172，则属**测试夹具需要补 row69/阵容**（数据），断言仍 true。

### #7 `crates/wolf-game/tests/m7_battle_target_scheduler.rs:363`
- 夹具：`seeded_game()` + `set_battle_wiring(true)` + `start_engine_events()` + `vars.set(495,2)`（2000495=2 温泉支线）+ `start_event(239)` + `self1600003=1`（`m7_battle_target_scheduler.rs:331-345`）。
- 现状断言：`m7_battle_target_scheduler.rs:363` `assert_eq!(var(&g,350), 2, "command phase advances to 2 and then cannot leave")`（配套 364/365/366：`352==0`、`343==0`、`349==1`）。
- 打补丁实测：`left=0 right=2`（即 `var350` 结束为 **0**，不是 2；M9 表 #7）。
- 正确语义期望：**需重导**。该测原意是钉死“2000350 状态机在深路径前被打断、卡在 2、CE244 结算门不触发”。旧语义下这是**字面量 bug 的产物**（`2000029 >= 2000028` 等 magic 比较恒假，命令相位停在 2 出不来）。正确语义下 `CE242 pc184=111[1,2000029,2000028,1]` 与 `CE243 pc952=111[1,2000029,2000028,1]`、`CE160` AI 一线变为真实比较，**相位会推进并回到 0**。故期望应从“`==2` 卡死”改为“相位经历 1、2 后**回到 0**”，并一并复核 `352/343/349` 在正确语义下的真实终值。
- 为什么失败：分歧站点 `CE242 pc184 / pc1099 / pc1338`（均 `2000029>=2000028`）、`CE243 pc952`、`CE160` 多 pc（`M10_BATTLE_DB_FIXTURE.md §4` 列出）。
- 类别/修复：**(c)**。这是“**陈旧的控制流模型**”，不是单纯改数字：必须先插桩取正确语义下 `2000350` 的真实序列（很可能 `[0,1,2,0]` 或含多轮），再重写整段断言。

### #8 `crates/wolf-game/tests/m7_ce244_direct.rs:137`  —— 唯一纯 **(a) 陈旧预期**
- 夹具：`the_win_lose_display_branch_opens_only_on_the_raw_self_magic`（`m7_ce244_direct.rs:121-145`）→ `open_game()`（real data root），`vars.set(349, probe)`，`start_event(244)`，`frame()` 一次。`OUTCOME=352`（`:45`），`SETTLED=343`（`:46`）。
- 现状断言：`m7_ce244_direct.rs:137-141`
  `assert_eq!(game.vm.vars.get(OUTCOME), want, "2000349={probe} expects outcome {want}")`
  cases（`:128`）：`(349=11 → 1)`、`(349=1600010 → -1)`。
- 打补丁实测：首例 `left=-1 right=1`（`349=11` 期望 1，实得 **-1**）；M9 表 #8。
- 正确语义期望：**两例翻转**
  - `349=11` → 期望 **-1**
  - `349=1600010` → 期望 **1**
- 为什么（逐 pc）：`CE244 pc3` 把 `self1600010 = 11`（默认阈值；`pc4` 若 `2000354==20`→5、`pc9` 若 `2000495==2`→5，本例都不触发，故阈值=11）。`CE244 pc36 = 111[1,2000349,1600010,2]` 即 `if (var349 == self1600010)`。旧字面量语义把右值当常量 `1_600_010`，`var349=11` 永不相等 → **fall-through** → 走到默认胜利写 `2000352=1`（这就是旧期望 1 的来源，本质是**钉死了字面量 bug**）。引用语义下：
  - `var349=11 == self1600010(=11)` → **TRUE** → 进入 pc36 块 → `pc44 = 111[17,2000192,2000193,0]` 即 `if (var192>var193)`；裸 Vm 两者皆 0 → false → 取 else（**敗北**）→ `2000352=-1`。
  - `var349=1_600_010` vs `self1600010=11` → **FALSE** → fall-through → 默认胜利 → `2000352=1`。
- 类别/修复：**(a) 纯陈旧预期**。修复：把 `cases` 改为 `[(11,-1),(1_600_010,1)]`，并**删改文件顶部模块注释 `m7_ce244_direct.rs:15-19`**（那段明确写“111 值操作数按字面量比较、magic 永不匹配、强制 2000349=1600010 才进分支”的旧 bug 说明）。`SETTLED==1`、`warnings==0` 两断言在两例下都仍成立（CE244 两臂都写 `2000343=1`）。

### #9 `crates/wolf-game/tests/m7_day4_playable.rs:570`
- 夹具：真实 day1→day4 流程（`sleep_into_next_morning`×3 → `spend_day_choosing_row`×2 → day4 08:30），`seed_battle_databases()`、`set_text(CData,5,16,0,"┗ 状態リスト")`、`start_engine_events()`、`set_battle_wiring(true)`；实跑 CE253→CE239→CE242 真实链（`:540-598` 断言）。
- 现状断言：`m7_day4_playable.rs:570` `assert!(settled, "the round must settle on the genuine day-4 replay")`，其中 `settled` = `v343==1 && v350==0`（循环内判定）。
- 打补丁实测：`v350_seq=[0,1,2] ce241=false v352_peak=0 v343_peak=0 ce239_alive=true ce239_pc=Some(13)`（M9 表 #9）。
- 正确语义期望：**仍应为 `settled=true`**（这是 day4 可玩性的核心验收）。当前未结算，属正确语义下**暴露的结算缺口**，不是“应该不结算”。
- 为什么失败：`2000350` 走到 2 后再没回到 0；CE244（唯一写 `2000352/2000343` 者）在正确语义下 `pc36=111[1,2000349,1600010,2]`、`pc44=111[17,2000192,2000193,0]`、`pc58=111[1,2000345,0,3]`、`pc85`、`pc132=111[1,2000029,2000028,1]` 这些门不再按旧字面量“恒假→fall-through 默认胜利”，而是走真实分支；同时 `CE242 pc184=111[1,2000029,2000028,1]` 改变行动/战力流。结果 CE244 **fall-through 不写结算**，CE239 停在 pc13（等 2000350 相位）→ `ce239_alive=true`。注意：`ce241=false`——CE241 本次甚至未启动（boot-once `ENGINE_AUTO_START=[241]`，`game.rs:98`），无法把 `2000350` 从 2 复位到 0，这是与 op111 **正交**的结构缺口（`M9_OPCODE_111_PLAN.md §3 Phase 3`）。
- 类别/修复：**(c)**（结算状态机 + 结构缺口叠加）。修复：插桩取 CE244 在正确语义下各 pc 门的实值，判定是 (i) 数据/战力经济需重建（boss 路径 `CE239 pc202 2000445==1`），还是 (ii) 需 `battle_wiring` round-loop 驱动真实 CE241 复位相位（task-57 机制），再重导 `v350_seq` 与 `settled` 断言（`settled` 目标值不变）。

### #10 `crates/wolf-game/tests/m8_battle_command_input.rs:163`
- 夹具：`real_ce188_command_window_registers_a_chosen_non_default_command`（`m8_battle_command_input.rs:55-...`）→ `Game::new` + `load_save(SaveData01)` + `seed_battle_databases()` + `start_event(188)`，用 DOWN/OK 真实键盘操作 CE65 万能ウィンドウ选 `防御`（code 25）；断言 CE157 把选择登记到 `CData type14 row1 col3`（`コマンド入力位置`=1）。
- 现状断言：`m8_battle_command_input.rs:163` `assert!(saw_ce65, "CE65 万能ウィンドウ選択実行 must run")`（后续 164-183：`saw_ce192`、`saw_command_menu`、`selectable_codes`、`saw_ce157`、`registered_row==Some(1)`）。
- 打补丁实测：`menu_seen=false ce65=false ce192=false`（M9 表 #10；日志行 155-161 会打印这些）。
- 正确语义期望：**仍为 `saw_ce65 == true`**（真实键盘操盘必须打开 CE65；否则整条“菜单→CE157 登记”证据链失效）。
- 为什么失败：`ce65=false ce192=false` 说明 CE188 线在 CE65 之前就断了。CE65 之前路径为 188→189→190→143/144→147→172→191→192→…→65；`CE172` 已被排除（CE190 重算 row69、双存活）。最早可达 magic111 为 `CE147 pc37=111[2,1600001,-1,2, 1600001,1600026,5]`（装備補正，`self1600001==-1` / `self1600001!=self1600026`），随后 `CE152 pc82`、`CE161 pc31`、`CE160`。本案 `Load` 的 SaveData01 有真实存档，故 lineage 数据更完整，first-divergence 大概率是 CE147 pc37 而非退化 CE172。
- 类别/修复：**(c)**，插桩确认 CE147 pc37 后按数据/行为二选一，期望 `saw_ce65==true` 保持。

### #11 `crates/wolf-game/tests/m8_battle_party.rs:183`
- 夹具：`multi_member_commands_map_to_distinct_party_pools`（`m8_battle_party.rs:159-191`）→ `run_command(cmd)`（`:99-145`）：`open_battle(wiring=false)`（`load_save(SaveData01)`+`seed_battle_databases`+`set_text(CData,5,16,0)`+`start_engine_events`+`start_event(253)`），推进到 `2000340!=0`（battle-active），然后 **手动** `start_event(242)` 并 `self_vars.set(600, cmd)`，跑到 `v343==1 && v350==0` 或 20000 帧。`EXPECT`（`:148-157`）逐 cmd 断言：CE242 到达对应 body `[lo,hi)`、对应 pool（`2000345/346/347/348`，初始 `[5,4,3,2]`，2000598 DLC-only）少 1。
- 现状断言：`m8_battle_party.rs:183` `assert_eq!(r.v343, 1, "cmd {cmd}: battle must be decided")`（184/185：`v352 in {1,-1}`、`warnings==0`）。
- 打补丁实测：`left=0`（`v343=0`，即单条 CE242 命令**未结算**；M9 表 #11）。
- 正确语义期望：**仍应为 `v343 == 1`**（命令→成员/pool 映射的“单命令即结算”是数据事实，见该测模块注释与 task-49 实测：8 条命令全部 `v352 in {1,-1}`、`v343=1`）。pool 断言（`:174-181`）也应仍成立（pool 消耗在 CE242 body 内完成，先于 CE244）。
- 为什么失败：`v343=0` 表示 CE244 在正确语义下**不再默认结算**。核心站点：
  - `CE242 pc184 = 111[1,2000029,2000028,1]`（`if 2000029>=2000028`）——引用语义下真实比较可能改变“行动后战力/胜负预判”；
  - CE244 结算门 `pc36=111[1,2000349,1600010,2]`（`2000349==阈值`）、`pc44=111[17,2000192,2000193,0]`（`192>193`）、`pc58=111[1,2000345,0,3]`（`2000345<=0`）、`pc132=111[1,2000029,2000028,1]`——旧字面量下这些几乎恒假→CE244 fall-through 到默认胜利写 `352/343`；引用语义下改走真实分支，可能既不胜利也不败北而 fall-through 不写结算。
- 类别/修复：**(c)** 结算状态机分歧。修复：插桩取 CE244 各门实值（`2000349/2000192/2000193/2000029/2000028/2000345`），判定是“战力经济需重建（boss 血量路径）”还是“CE244 门语义需正确重建”，再重导断言（`v343=1` 目标不变）。

---

## 4. 跨测试的根因分组（便于 Phase 2 一次修）

**组 A — 纯陈旧预期（1 项，直接改断言/注释即可）**
- `m7_ce244_direct.rs:137`（#8）：`(349=11→1)/(349=1600010→-1)` 翻转为 `(11→-1)/(1600010→1)`；并改模块注释 `:15-19`。

**组 B — CE188 线早段 magic111 分歧，reach 断言应保持 true（#1–6、#10 共 7 项）**
- 候选 first-divergence（按路径先后）：`CE147 pc37 = 111[2,1600001,-1,2, 1600001,1600026,5]`（最早可达，装備補正）→ `CE65 pc160 = 111[1,1600011,1600024,1]`（万能ウィンドウ選択実行）→ `CE152 pc82 = 111[1,1600017,1600015,0]`（コマンド一覧算出）→ `CE161 pc31 = 111[1,1600000,1600010,2]`（味方対象選択）→ `CE160`（AI，多 pc）→ `CE163 pc215/286/333` → `CE64`（万能ウィンドウ描画，多 pc）。
- 重要分歧点：`Game::new` 无存档路径（#4 `m7_battle_menu_text`、#6 `m7_battle_seed` 的 control 组）存在 **CE190 阵容/`type18 row69` 退化**风险，需优先排除是否退化为 CE172 `0==0` 判胜；有存档路径（#1/#2/#3/#5/#10）大概率是 CE147 pc37 起步。
- 处理：先插桩取每例 first-divergence；若属数据前置（阵容/alive/row69）→ 用 task-66 `seed_full_battle_db` 模板补夹具、断言保持 true；若属真实正确行为 → 更新断言并记录依据。

**组 C — 战斗状态机 / 结算分歧（#7、#9、#11 共 3 项）**
- 关键站点：`CE242 pc184/1099/1338 = 111[1,2000029,2000028,1]`；`CE243 pc952 = 111[1,2000029,2000028,1]`；`CE244 pc36/44/58/85/132`；`CE160` AI。
- 处理：插桩取 `2000350` 序列与 CE244 各门实值；判定“战力经济重建 / CE244 门语义重建 / 需 battle_wiring round-loop 驱动真实 CE241（`game.rs:98 ENGINE_AUTO_START=[241]` boot-once）”；`settled`(#9)、`v343==1`(#11) 目标值不变，`var350`(#7) 的控制流模型需重写（很可能改为回到 0）。

---

## 5. 战斗 DB 夹具前置（Phase 0 模板，task-66）

在落补丁前，涉及 CE188 线的测试（#1–6、#10）应显式补全战斗 DB，避免“合成退化夹具”制造假中断。`seed_full_battle_db`（`crates/wolf-game/tests/m10_battle_db_fixture.rs`）模板：
- `CData type18`：`row4`=我方人数(=1)、`row69`=敌方人数(=1，CE190 会按组表重算为真实阵容，实测 1→7)、`row72`=敌组 ID(=0)、`row75`=回合数(=3)。
- `CData type13`：组 0 的 col7/8/9/10 勝/敗条件 = 0（不触发特殊胜负）。
- `CData type10`：slot0(我)/slot10(敌) 的 col2=存在=1、col3=陣亡=0（存活）、HP 列。
- `CData type0 row12`：主角指挥槽 `47..54=[3,4,6,7,0,0,0,0]` + 数值列 `(20,21,22,23,24,25,26,27,28,9,10,4)`。
- `CData type7 row0 col0=1`（item 0，供 CE152 pc89-95 判定可选）。

> 备注：`CE172` pc32/pc53 的“死数==总数”判定，只有在**强制 row69=0 且不经 CE190 重算**的退化夹具下才会 `0==0` 判胜中断（task-66 §3 `empty_enemy_roster_would_abort_under_reference_model`）；真实/完整夹具不会。

---

## 6. 执行顺序建议（不改树绿）

1. **本任务（已完成）**：仅出本文档，钉死 11 条现状断言 vs 正确期望 vs 依据。
2. **Phase 0（零生产改动）**：把 `seed_full_battle_db` 应用到 6 个 CE188 线测试的夹具（test-only），先在不落补丁时确认它们仍绿；再把 7 个正测试的 first-divergence 用 `<site>` 插桩（临时 trace，不改语义）逐点定位。
3. **Phase 2（原子落地）**：`git apply work/out/opcode111_landed.patch` + 按本文重导：组 A 改 `m7_ce244_direct`；组 B 依 first-divergence 决定“补数据/改断言”；组 C 重导 `2000350`/CE244 结算（可能需 task-57 round-loop 真 CE241）。
4. **验收**：`cargo test -q -p wolf-vm` 与 `cargo test -q --workspace --no-fail-fast` 均 **0 failed**（以 `M9_OPCODE_111_REGRESSION.md` 15 行表为回归基线），`wolf-cli verify` PASS 70/0。

---

## 7. 明确的边界（本任务未做 / 不能做）

- **未应用补丁**（PARKED），未编辑任何 test / src，未 commit / push。
- 因此 10 个 (c) 测试的 **first-divergence 只给候选站点，不给最终实测 pc**——需 Phase 2 落补丁后插桩确认（`M9_OPCODE_111_REGRESSION.md` 与 `M10_BATTLE_DB_FIXTURE.md §4` 已列出候选）。
- `CE172` 中断假设已被 `M10_BATTLE_DB_FIXTURE.md` 实测证伪，本文据此不把 #1/#2/#5 归 CE172。
- 与 op111 **正交**的结构缺口（CE159/CE162 执行侧分发、CE244 结算深链、CE241 boot-once）不因本补丁变绿/变红，单列跟踪，不混入期望重导。
