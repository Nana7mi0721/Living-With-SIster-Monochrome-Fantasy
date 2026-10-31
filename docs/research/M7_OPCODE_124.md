# M7 W5 · opcode 124 SetVariableEx 长格式语义（H-124）

日期：M7 W5 · 仓库 HEAD 1d36ebe · 归属任务 task-23

## 结论（先看这个）

`124 SetVariableEx`（变量操作扩展）在 HEAD 1d36ebe 上**已完整实现**，且对本作实际出现的所有子类型都有证据支撑，本任务**不需要改引擎**。`docs/UNVERIFIED.md` 的 H-124 仍写着"语义未知，当前实现保持旧值 + 记 Info 诊断"——这是**过时**的描述（那段描述的是更早的占位实现）。本次交付补的是：

1. 回归矩阵 `crates/wolf-vm/tests/opcode_124_long.rs`（7 个用例，全绿）；
2. 本文档：代码路径、按子类型语义、1045 站点证据扫描、残余未知；
3. 更新 `docs/UNVERIFIED.md` 的 H-124，标记为已实现并注明残余的 host 侧空档。

注意 H-124 里的 579 是**公共事件（CommonEvent）**内的站点数；把地图事件也算上，全库共 1045 条（见下）。

## 代码路径

1. 分派在 `crates/wolf-vm/src/exec.rs:1721-1778`。它是**纯查询**：选一个引擎状态值写入目标变量，永不阻塞、不碰 block 栈（branch_depth）。
2. 参数形状：`argc == 4` → `[dst, sub, object, field]`；`argc == 3` → `[dst, sub, field]`。短格式只被 `0x3000` 使用（本作 278/278 条 0x3000 都是 argc 3）。field 取 `argc>=4 ? arg3 : arg2`（exec.rs:1732-1736）。
3. 目标 `dst` 通过 `store_num`（exec.rs:1352）写入，支持 magic 前缀：`1600000+n` 自身变量（本作 1045/1045 全部如此）、`2000000+n` 普通变量、`9000000+n` 系统变量、`1100000+n` 地图事件自身变量。
4. 对象 `object`（0x1000/0x1200/0x4000/0x4400）必须走 `eval_num`（exec.rs:1293）：它既可能是变量引用/魔法值，也可能是 5 位图片番号（如 12004、3149840），并会保留 -1..-7 这类负哨兵（eval_num 的 Opaque 分支原样返回）。
5. 具体属性值来自 Host trait（exec.rs:505 起）：
   - `picture_state(number, field)`（0x4000/0x4400）
   - `character_state(target, field)`（0x1000/0x1200）
   - `engine_state(field)`（0x3000）
   三者默认都返回 `None`；`None` 时解释器**不写目标**（保持旧值），这正是原引擎在"查询对象不存在"时的行为（docs/OPCODE_124.md §3.1）。
6. `0x3000 field 18`（"正在处理的事件号"）由解释器直接回答，公共事件为 `事件号 + 500000`（exec.rs:1741-1745），从不交给 host。

## 按子类型语义

sub = (type<<12)|modifier：

- `0x3000`（type3 "其它"，仅 argc 3）：field 0=当前地图 id，1=当前 BGM id（-1 无），7/8/9=鼠标左/右/中三态（0 抬起、1 按住、2 刚按下；脚本里与 2 比较），10=滚轮增量，11=鼠标下地图事件 id（-1），12=接触扩展 id（-1），17=未定义（返回 0），18=正在运行的自动/接触/决定事件号（公共事件 +500000，无则 -1），20/21=鼠标 X/Y 像素。
- `0x1000 / 0x1200`（type1 角色/地图事件；0x1200 = type1 的 modifier 0x200 缩放变体）：field 0/1=tile X/Y，2/3=精确 X/Y（tile*2），4=高度像素，5=朝向 1..9，6/7=屏幕 X（中心）/Y（脚），8/9=缩放后版本，10=事件 id（-1 无）。obj>=0 为地图事件（不存在→None→不写）；-1=正在运行的事件；-2=队长；-3..-7=队伍 1..5。本作 zoom=100%，故 0x1200 与 0x1000 等价。
- `0x4000 / 0x4400`（type4 图片；0x4400 = modifier 0x400 "无特效修正"）：field 0=X 像素，1=Y 像素，2/3=X/Y 缩放%，4=pattern，5=未定义（返回 0），8=光标悬停（1 是/0 否/-1 不存在；AABB 判定、忽略旋转、透明像素也算、按屏幕裁剪），9=是否使用（0/1），10=文字是否显示完成（1 完成/0 打字中/-1 非文字）。本作图片无 Effects，故 0x4400 == 0x4000。
- 其它 sub（如 0x2000 type2）：本作 0 站点；解释器记一条 Info 诊断并保持旧值。

## 证据扫描（1045 站点）

数据：`work/_ce_full.json`（公共事件命令）+ `work/maps_full.json`（地图事件）。命令形状 `{c,a,s,bd}`，地图事件 `{code,ints,strs,bd}`。

- 总站点 1045（公共事件 579 + 地图 466）。`argc` 分布：3=278，4=767。
- `dst` 高 4 位非零：0（全部纯覆写，无复合赋值、无 ±999999 夹取）。sub 低 8 位非零：0。未处理 sub 站点：0。
- 按 sub 的 field 分布：
  - 0x4000 计数 719：field 0:4 1:1 2:19 3:12 4:7 5:1 8:588 9:85 10:2
  - 0x1000 计数 39：field 2:2 3:2 5:2 6:16 7:16 10:1
  - 0x3000 计数 278（argc3）：field 0:3 1:1 7:210 8:36 9:1 10:10 11:1 12:1 17:5 18:3 20:4 21:3
  - 0x4400 计数 8：field 2:4 3:4
  - 0x1200 计数 1：field 7
- 与 H-124 的公共事件计数一致：0x4000 359 + 0x3000 174 + 0x1000 37 + 0x4400 8 + 0x1200 1 = 579。
- 0x1000/0x1200 的 obj<0 共 34 处，distinct object 集合 {-7,-6,-5,-4,-3,-2,-1, 1600000, 1600011, 1600013} → 确实需要 eval_num 解析魔法值。
- field18 站点：CE224（dst 1600010）、CE270（dst 1600061，2 条），全在公共事件内，故 +500000 正确；无地图事件 field18 站点。
- 其余少见 field 站点：field17 在 CE265/266/267/268/270（dst 1600099，共 5）；field11/12 在 CE225（dst 1600023/1600024）；0x4000 field5 在 CE233（obj 120）；0x4400 全部在 CE63/90/94/178（obj 1600021，field 2/3）。

## 回归测试

文件：`crates/wolf-vm/tests/opcode_124_long.rs`

用 Mock Host `Probe` 记录每次查询并按 (编号, 字段) 投喂预设答案；未预设 → None → 目标必须保持旧值。7 个用例：

1. `picture_subops_read_the_requested_field_and_leave_unknowns_untouched`：5 位番号 12004 原样透传；0x4400 用自身变量 1600021→300 作对象；未答字段 803/77 保留旧值 7。
2. `character_subops_evaluate_the_object_and_share_one_field_space`：变量引用 2000000=42 被解析为 42；obj -2 原样透传；0x1200 与 0x1000 同答案。
3. `engine_short_form_reads_host_state_and_field_18_from_the_interpreter`：argc3 [dst,0x3000,20/21]；field18 → 事件 55+500000=500055，且 host.engines 里不含 18；未答 99 保留旧值。
4. `an_unknown_sub_keeps_the_destination_and_logs_one_info_line`：0x2000 → 目标不动 + 恰好 1 条 Info 含 "0x2000"。
5. `the_destination_magic_routes_to_normal_system_and_self_banks`：2000000 / 9000005 / 1600002 均写入，自身变量经 121 复制出来核对。
6. `an_unanswered_query_leaves_a_self_destination_untouched`：自身变量 1600005 保持 9。
7. `a_run_of_124_queries_finishes_in_a_single_frame`：连跑 6 条查询 + 172 → report.running 0、finished 1（证明 124 不阻塞）。

运行命令与结果：

```
cargo test -q -p wolf-vm --test opcode_124_long
# running 7 tests .. test result: ok. 7 passed; 0 failed
```

## 残余未知（host 侧，超出本任务写入范围）

解释器分派完全覆盖；下列空缺在 `crates/wolf-game/src/game.rs` 的 host 实现里，属于其它任务/写入范围，这里只如实记录、不改：

- `engine_state`（game.rs:1921）未实现 field 11、12、17，均返回 None。数据里有 field 11:1、12:1、17:5 站点；docs/OPCODE_124.md 建议 field17 返回 0。当前行为：这些站点保持旧值。
- `picture_state`（game.rs:1890）未实现 field 5，返回 None。数据里 0x4000 field5 仅 1 处（CE233 obj120）；docs/OPCODE_124.md 说该字段返回 0。
- field18 的 "+500000" 只对公共事件有站点，无地图事件样本，地图事件的偏移无法用本作数据证实（按原引擎推测同样是 +500000）。
- `character_state`（game.rs:1914）恒返回 None（地图层尚未接入），因此 0x1000/0x1200 目前的 40 个站点全部保持旧值——这是 M2 地图层的待办，不是本指令的缺陷。

## 先前缺失 / 本次补了什么

- 库层实现其实早就存在；缺的是"可复现回归 + 一份按子类型的语义/证据说明"。没有测试钉住"短格式只对 0x3000 成立、对象必须走 eval_num、None 不写目标、field18 由解释器回答、未知 sub 只记一条诊断、124 不阻塞"。
- 本次补上 `opcode_124_long.rs`（7 测）与本文档，并澄清 H-124 的"语义未知"已过时。**无需任何引擎/源码改动。**

## 与其它交付不冲突

- 只新增 `crates/wolf-vm/tests/opcode_124_long.rs`、本文档，并更新 `docs/UNVERIFIED.md` 的 H-124 段；
- 未触碰其它源码、battle 测试、`rng.rs`、`docs/BASELINE.json`、`docs/HANDOFF_M4.md` 及 3 个永久 scratch 文件；未 commit/push。
