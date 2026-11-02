# M13 全部结局逐条可执行验证（七结局）

> 交付：`crates/wolf-game/tests/m13_all_endings.rs` + 本文件
> 角色：teammate mainline-endings（task-13）
> 口径：2026-11-01 完成标准 = 重构版“游戏体验内容与原版一致 + 修复可能存在的 bug”。本文用**可执行测试**逐条证明各非主线结局脚本都能被驱动到其收尾文本，且运行期 `stats.warnings == 0`。

## 1. 测量环境与复现

| 项 | 值 |
|---|---|
| git HEAD | `19b177b876c8a2dd50d3b504a4f69b5ee1bbc257`（19b177b，含 7c3b728 common-self 修复、dcd513a Text picture 存读修复）|
| 数据根 | `work/all`（`WOLF_DATA_ROOT`）|
| 脚本来源 | `work/wolftl/dump/common/*.json`（WolfTL dump，命令 `.index` == 运行期 `pc`，已由 farmer dispatch `idx1193 == pc1193` 实测证实）|
| 默认快测 | `WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m13_all_endings` → **14 passed; 0 failed; 2 ignored**（~0.8s）|
| 长链（ignored） | `WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m13_all_endings -- --ignored` → **2 passed; 0 failed**（~0.55s）|

所有驱动断言均在 `warnings == 0` 下通过（`assert_reached` 同时校验渲染文本与 warnings）。

## 2. 结局清单与测试映射

| # | 结局 | 触发事件 / 坐标 | 门变量与阈值 | 测试函数 |
|---|---|---|---|---|
| 1 | 妹妹怀孕 好/坏 END | CE405，gate@pc55 | `var21(信頼) >= 300`=好；否则坏 | `pregnancy_good_end` / `pregnancy_bad_end` |
| 2 | 参军 兵役 END | CE326，trust@pc36 / mood@pc38、army gate@pc54 | `var21<=5` 且 `var23<=1`，再 `var151>=1`=兵役 | `military_service_end` |
| 3 | 返家 END | CE326 同上，`var151<1` 分支 | 同上但 `var151=0` | `return_home_end` |
| 4 | 妹妹病亡（症状恶化链） | CE277，dispatch@pc79 `self0∈{4,5,6}` | 轻症：`self0=4`；重症臂副作用见 §4 | `sister_sickness_mild_arm` / `sister_sickness_severe_arm_side_effects` |
| 5 | 暗影 END | CE305 stage25 战斗结果@pc2915 | `self30(战斗结果)==1`（胜） | `shadow_end` |
| 6 | 魔界 BAD END | CE305 同上@pc2915 | `self30<=-1`（败） | `makai_end` |
| 7 | 农民 END | CE305 stage13/14/15 dispatch@pc1193 | `self0∈{13,14,15}` 且 `var534==2` | `farmer_end` |
| 8 | 妹妹爆炸 BADEND | CE322@1442 / CE323@2548 / CE359@2022（同文本） | 直接跳到 headline | `sister_explosion_badend_ce322/323/359` |
| — | 爆炸预告（占卜师） | CE297 大冒険 | 前置剧情，非结局 | `ce297_oracle_foreshadows_explosion` |
| — | 门存在性静态核对 | 全部上述 gate | — | `ending_gates_present` |

## 3. 各结局细节

### 3.1 妹妹怀孕（CE405）
- 前置：CE326@79 的“妊娠END判定”，`var105 >= 3`（中出/妊娠计数）→ 调用 CE405。
- CE405 gate@pc55：`111 count=1 (var21, 300, op1>=)` → ChoiceCase[1]=好结局块；ElseCase=坏结局块。
- 好结局：`var21=350` 注入 → 渲染 `<C>\E怀孕END　～幸福的家庭～` 与 `恭喜您达成good end。`（测试断言 `怀孕END` + `good end`）。
- 坏结局：`var21=100` → `<C>\EBADEND　～不被期待的怀孕～`。
- 长链 `pregnancy_chain_via_ce326`（ignored）：从 CE326 的 `var105>=3` gate 起跑，直接冒泡调用到 CE405，good/bad 两支均通过。

### 3.2 参军 / 返家（CE326）
- 门：`var21(信頼)<=5`（pc36）∧ `var23(機嫌)<=1`（pc38）进入“结局判定”块；块内再 `var151>=1`（pc54）→ 兵役，否则返家。
- 兵役：`var21=3, var23=0, var151=1` → `父亲安排下去服兵役了` / `<C>\EBADEND　～兵役～`。
- 返家：同注入但 `var151=0` → `<C>\EBADEND　～返家～`。
- 注：`ARMY_GATE` 三元组在事件内 pc17 亦出现（另一处 `var151>=1`），仅做存在性核对，不影响分支驱动。

### 3.3 妹妹病亡 / 症状恶化链（CE277）
- CE277 是 “悪いイベント系” 多路选择：dispatch@pc79 `111 count=3 (self0==4|self0==5|self0==6)` → case1 进入 `症状悪化` 块（pc81-98）。
- 轻症臂（`self0=4`，gate 注入）：渲染 `……咳咳。` / `妹妹的身体状况好像不太好……`（`var20 -= 20`）。
- 重症臂副作用块（pc100-128，headline pc107 `……哥哥。今天特别的色呢……噗！`）：连续 `121` 复合赋值——`var151 += 1`、`var23 += -1`、`var18 += -5`、`var102 += -10`。
  - 关键：`var151 += 1` 是“病亡链”喂给 CE326 兵役分支的钩子（`var151>=1`）。测试断言：`var151` `+1`、`var18` `-5`、`var102` `-10` 持久生效。
  - `var23(機嫌)` 在 pc123 置 -1，但同一事件后段会把它重算回 0（帧跟踪：pc125 时 v23=-1，事件结束时 0），故最终值不稳定，不作断言（仅记录）。
- 重要解码纠正：dump 里 `[2000023, 4294967295, 0, 256]` **不是** “把 var23 赋 -1”，而是 `var23 += -1`（`bits & 0x0FFF` = `0x100` → add；`0x200` → sub）。因此凡 `bits` 非 0 的 `121` 都必须按复合赋值理解。

### 3.4/3.5 暗影 END / 魔界 BAD END（CE305 stage25 战斗结果）
- 坐标：CE305 dump `idx2915`：`111 count=2 (self30==1 | self30<=-1)`。
  - arm1 `self30==1`（战斗胜）→ pc2921-2924 黑雾散去/阴影出现 → `idx2927 \EEND　～阴影～`。
  - arm2 `self30<=-1`（战斗败）→ pc2935-2941 `涌出的黑雾覆盖了整个国家` … `魔界。` → `idx2941 \EBAD END　～魔界～`。
- 坑：**同一三元组在事件早期 pc433 也出现**（另一处战斗结果分支）。`gate_pc` 顺序扫描会误命中 pc433（结果显示农民/拂晓剧情）。故测试用显式 `SHADOW_PC=2915`（dump index==pc）并在该 pc 断言 `cond_triples == SHADOW_GATE`，再注入 `self30=1`/`-1` 分别命中。
- 长链 `ce305_stage25_scene_to_makai`（ignored）：从 stage25 dispatch 起跑完整场景（由香莉警告黑雾 → 战斗）。headless 永不赢战斗，`self30` 落到 `<=-1`，天然进入 `魔界` BAD END——证明 stage25 整段与魔界结局连通。暗影胜分支由 `shadow_end`（注入战后结果）确定性覆盖。

### 3.6 农民 END（CE305 stage13/14/15）
- CE305 dump `idx1193`：`111 count=3 (self0==13|14|15)`（期限切れ/逃亡相关 stage 组）→ 块内 `var534==2` 分支。
- 注入 `self0=13, var534=2` → `结果公会还是和其他公会合并了` / `……挥着锄头耕地……` / `\E农民END`。

### 3.7 妹妹爆炸 BADEND（CE322 / CE323 / CE359）
- 三处同文本 `<C>BADEND\n～妹妹爆炸～`：CE322@1442、CE323@2548、CE359@2022。
- 前置剧情 CE297 `■大冒険` 占卜师：预言“魔力积蓄会因突发情况爆炸”；`@171` 二选一（保留爆炸可能+新魔石 / 用未来交换救妹妹）。
- 测试对三个事件分别 `msg_pc("妹妹爆炸")` 跳 headline 驱动，断言 `妹妹爆炸` 渲染且 `warnings==0`；`ce297_oracle` 断言占卜师文本含 `爆炸`。

## 4. 驱动方法（helper 契约）

- `cond_triples(program, pc)`：`Slot::Command`，`opcode==111`，`count = carg(0) & 0x0F`，三元组从 `carg(1+3i)` 起，`(var, value, op)`，`op 0=> 1>= 2== 3<= 4< 5!=`。
- `msg_pc(program, needle)`：扫描 `opcode==101` 且 `strings.join("").contains(needle)` 的运行期 pc。
- `gate_pc(program, want)`：顺序扫描首个匹配的三元组（**有重复三元组时会误命中**，重要 case 用显式 pc）。
- `drive(game, ce, selfs, globals, pc, max_frames)`：`start_event` → `instance_mut` 设 self（`slot = 600 + N`，对应 magic `1_600_000+N`）/ 覆盖 pc → `vars.set` globals → `auto_advance=false, auto_choice=true, delay=0` → 循环 `key_bits=OK → frame() → 0`，收集去重后的 `clean(game.messages)`，实例消失即 `ended`。
- `assert_reached(label, run, expected)`：任一消息含 `expected` 且 `warnings==0`。
- `self(N)` 对应 flat slot `600+N`（`COMMON_SELF_BASE=600`）。

## 5. 已知边界 / 未决点

1. **暗影 END 无法在 headless 下由真实战斗自然到达**：战斗胜 `self30==1` 由 `CallEvent 500239` 依战斗结果写入，headless 必败。故 `shadow_end` 采用“注入战后结果 `self30=1` 跳到 pc2915”的方式确定性验证；完整 stage25 场景到魔界由 ignored 长链覆盖。原版该分支需玩家赢下战斗，属战斗系统范畴（见战斗测试）。
2. **`var23(機嫌)` 复合赋值后会被同事件后段重算**（pc123 置 -1，事件结束回 0），最终值不稳定，测试只断言持久的 `var151/var18/var102` 增量；`docs` 记录此行为非 bug。
3. **未发现独立的“妹妹病亡/死亡”BADEND 文本**：游戏把病亡实现为“症状恶化 → 体力/健康链 + `var151` 兵役钩子”，CE277 中无“死亡END”字面量。若后续发现别处死亡分支另记。
4. 以上 gate 坐标为 `.index`（==运行期 pc，已抽样证实）；若日后脚本被重新 dump/编辑需复跑 `ending_gates_present` 作护栏。

## 6. 结论

- 七类结局（怀孕好/坏、兵役、返家、症状恶化链、暗影、魔界、农民、妹妹爆炸×3）的收尾文本均可被脚本驱动渲染，`warnings == 0`，内容与原版 dump 一致。
- 未发现新的引擎 bug；上述“重复三元组”“复合赋值语义”“var23 重算”“headless 战斗必败”均为已理解的行为，不是缺陷。
- 文件：`crates/wolf-game/tests/m13_all_endings.rs`（默认 14 passed / 2 ignored；`--ignored` 2 passed）；本说明 `docs/research/M13_ALL_ENDINGS.md`。
