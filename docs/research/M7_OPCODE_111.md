# M7 W5 · opcode 111 条件指令「值操作数」语义（literal vs 变量引用）

日期：M7 W5 · 仓库 HEAD 1d36ebe · 归属任务 task-37

## 结论（先看这个）

本作数据里 opcode 111 的**值操作数**（每个条件三元组 [var, value, operator] 的 value）有 356 处是变量/自身变量引用，占全部 22695 个条件三元组的 1.57%。这些站点只有在 value 走 eval_num 解析时才成立；当前解释器（crates/wolf-vm/src/exec.rs:2207）把它当**原始整数**比较，因此这 356 处实际都按字面量处理，语义错误。

证据链完整（CE26 把 300 调用返回值与 CSelf 参数比较、CE109 用两个变量的位掩码、CE64 比较滚动标志、CE244 的显示/胜负门），结论明确：**值操作数必须走 eval_num**。

但按 Lead 的 task-37 协调（m00816）与 m6-multiday 的 task-39 声明，本次**不得修改 crates/wolf-vm/src/exec.rs**（另一任务正在改 opcode 250 的写分支）。因此本任务**不改生产代码**，只做两件事：

1. 给出**精确的一行补丁**（下方「建议补丁」），供 exec.rs 的持有者/Lead 落地；
2. 新增回归文件 crates/wolf-vm/tests/opcode_111_semantics.rs（4 个当前行为用例 + 1 个 #[ignore] 补丁后用例），把"当前按字面量"和"补丁后按引用"两种行为都钉住。

## 代码路径

1. 分派在 crates/wolf-vm/src/exec.rs:2194-2225 的 BlockKind::Condition（opcode 111）。head=arg0，低 4 位 count（高半字节 bit4 是 has-else 提示），逐条 [var, value, operator]。
2. 关键行 crates/wolf-vm/src/exec.rs:2207：
   let value = arg(cmd, 2 + 3 * i);
   左侧已经走解析：let lhs = eval_num(var, inst, vars);（exec.rs:2209）；右侧直接拿原始 i32。
3. 比较函数 compare（exec.rs:1489）：0=>lhs>rhs，1=>lhs>=rhs，2=>lhs==rhs，3=>lhs<=rhs，4=>lhs<rhs，5=>lhs!=rhs，6=>(lhs&rhs)!=0，_=>lhs==rhs。
4. eval_num（exec.rs:1293）：Literal=>原始整数；SelfVar=>inst.self_vars.get(index)；Var=>vars.get(index)；SysVar=>vars.get_sys(index)；StrVar=>0；Opaque=>raw（负数哨兵 -1..-7 原样）。它已能正确处理字面量与负数，所以改右侧只影响魔法值区间。
5. 魔法地址（crates/wolf-vm/src/magic.rs）：1_000_000..1_999_999 自身变量、2_000_000..2_999_999 普通变量、3_000_000..3_999_999 字符串、9_000_000..9_999_999 系统变量；自身变量按 1_000_000 + group*100_000 + slot 扁平化为 group*100+slot（例如 1_600_010 是 group6/slot610，不是"自身变量 10"）。
6. 条件块结构：111 之后 401 标记 then 体起点、499 关闭块；用例采用这一已验证形状（与 exec.rs:2917-2937 的 break 用例一致）。

## 证据扫描（22695 个条件三元组）

数据：work/_ce_full.json（550 个公共事件，命令形状 {c,a:[ints],s:[strs],bd}）+ work/maps_full.json（9 张地图，事件命令 {code,ints,strs,bd}）。分类脚本：work/op111_classify.js，输出 work/_op111_classify.json（均在 work/ 下，属临时分析产物，不在仓库写入范围）。

- 111 指令总数 14444；展开后的条件三元组总数 22695；argc 不匹配 0。
- 按 value 所在 bank：literal=22339（98.43%），self=304，normal=52 → **变量引用共 356 个（1.57%）**。
- 按左侧 var 所在 bank：self=9556，normal=13035，sys=90，literal=14（左操作数本来就绝大多数是变量，符合条件比较的用法）。
- 356 个引用站点：公共事件 351 + 地图事件 5，分布在 96 个不同位置。
- 引用站点的 operator 分布：{0:36, 1:97, 2:86, 3:49, 4:55, 5:25, 6:6, 17:2} → 关系运算（>,>=,<=,<）237、相等/不等 111、其它（位与 op6 共 6，未知 op17 共 2）8。注意 op17 在 compare 里落入 _=>lhs==rhs 兜底，语义仍未定义。
- 引用 value 的 distinct 魔法值 67 个。最高频：1600010:25、1600001:23、2000442:22、1600016:21、1600011:20、1600013:20、1600024:16、1600002:15、1600012:15、1600000:14。普通变量值包括 2000442:22、2000028:7、2000188:7、2000193:1。另有 1100007:4（自身变量 group1 slot7）。
- 引用站点最多的公共事件：CE270 ×┣[メッセ—ジ]表示処理 32；CE224 ウィンドウ処理 19；CE160 X[戦]AI実行 18；CE256 ■討伐依頼 17；CE242 ┣コマンド処理 13；CE225 フィールド上処理 12；CE363 お風呂タイマー1 11；CE64 X[共]万能ウィンドウ描画処理 10；CE132 X┗[移]システム欄実行 10；CE243 ┣敵の行動 10；CE26 ▲戦闘コマンド有無の判定 9；CE109 X[移]装備欄_選択実行 7；CE241 ┣ターン開始処理 7；CE244 ┗終了判定 3。

## 具体证据（可复核的原始命令）

- CE26（▲戦闘コマンド有無の判定）：idx4 code=300 ints=[0,16777218,1600000,0,1600020] strs=["X[移]戦闘コマンド取得"] → 把 300 的调用结果写入自身槽 1600020；idx5 code=111 ints=[1,1600020,1600002,2] → 用该结果与 CSelf 参数 1600002 比较相等。若 value 按字面量，则是"结果 == 1_600_002"，荒谬；按 eval_num 才是"返回值 == 传入参数"。
- CE109（X[移]装備欄_選択実行）：idx274 code=111 ints=[1,1600023,1600024,6] → 对两个变量的位掩码做按位与判非零（operator 6），是典型的 flag & mask。全事件共 5 处同形（idx 102/129/150/239/274）。
- CE64（X[共]万能ウィンドウ描画処理）：idx416 code=111 ints=[1,1600013,1600011,0] → 自身槽13 > 自身槽11 的滚动位置比较。
- CE244（┗終了判定）：pc36 code=111 ints=[1,2000349,1600010,2] → 意图是"变量349 == 自身槽 1600010"（pc3 把该槽设为 11）。pc44 code=111 ints=[17,2000192,2000193,0] → "变量192 > 变量193" 的胜负判定。task-31 的 day4 接线实测 2000349=1，而自身槽为默认 11 或 cmd20 置 5：字面量比 1==1_600_010、解析比 1==11/5 都为假，因此 pc36 在两种语义下都不成立，它不是 CE244 未结算的原因；真正的决定性门是 pc79 的 2000354==11/16。pc36 仍是"操作数在意图上是变量引用"的有效证据（比较变量349与该自身槽），在变量349=11 的假设数据下两种语义会分叉，用例 4 同时覆盖这个分叉和 task-31 的真实 1/11 组合。pc44 的胜负比较在当前字面量语义下恒假（如 5 > 2_000_193 不成立），这也解释了实测中决定胜负的只有 pc79。

## 建议补丁（本任务不落地）

唯一改动，crates/wolf-vm/src/exec.rs:2207：

```
-                let value = arg(cmd, 2 + 3 * i);
+                let value = eval_num(arg(cmd, 2 + 3 * i), inst, vars);
```

影响面：eval_num 对 <1_000_000 的字面量与负数 Opaque 原样返回，所以 22339 个字面量条件（98.43%）的语义完全不变；只有 356 个魔法值站点会从"比原始整数"变为"取出变量值再比"。现有 111 字面量单测 exec.rs:2832（cmd(111,[1,2_000_000,5,0])、v0=9）、exec.rs:2856（v0=1）、exec.rs:2926（[1,2_000_000,2,2]）均不受影响。风险点：必须由 exec.rs 当前持有者（m6-multiday，opcode 250 写分支）在合入后跑全量 wolf-vm/wolf-game 回归；本任务未验证补丁后的真实 game 行为（见残余）。

## 回归测试

文件：crates/wolf-vm/tests/opcode_111_semantics.rs（合成 CommonEvent 驱动 Vm，Mock Host 无副作用）

1. a_literal_value_operand_is_compared_as_an_integer：if(v0>5) 在 v0=9 取 then、v0=1 取 else，证明字面量路径正确（补丁后仍须通过）。
2. a_variable_valued_operand_is_currently_read_as_a_literal：v0=v1=5，条件 [1,2_000_000,2_000_001,2]（v0==v1）在解析语义下应为真；当前解释器把右侧当 2_000_001，走 else。断言 v0==5、v1==5、目标仍为 2 —— 钉住当前 bug；对照用例 if(v0==字面量5) 正常取 then。
3. a_magic_shaped_operand_matches_only_when_the_left_side_holds_the_raw_integer：v0 直接塞 1_600_010 时，[1,2_000_000,1_600_010,2] 才为真 —— 说明当前只是拿整数 1_600_010 硬比，没有解引用。
4. the_ce244_display_gate_and_win_lose_compare_fall_through_under_literal_semantics：合成 CE244 pc3/pc36/pc44 —— 子场景 A（变量349=11、自身槽=11，解析应相等）却落 else；变量192=5 > 变量193=3（解析应为真）却落 else；并复制自身槽断言它确实是 11，排除"值本身就不同"的歧义。子场景 B 用 task-31 day4 真实接线数据（变量349=1、自身槽=11）：字面量 1==1_600_010 与解析 1==11 都为假，证明 pc36 不是结算阻塞点，真正的门是 pc79。
5. reference_valued_operands_resolve_before_comparison（#[ignore]，原因写明"requires the proposed opcode-111 value-operand patch"）：补丁落地后应取 then；届时去掉 ignore 即成为回归守卫。

运行命令与结果：

```
cargo test -q -p wolf-vm --test opcode_111_semantics
# running 5 tests .. test result: ok. 4 passed; 0 failed; 1 ignored
```

## 残余未验证 / 为什么本任务不改

- 补丁未落地：真实解释器仍按字面量；第 5 个用例被 #[ignore]，不是通过状态。真实 game 层在补丁后的战斗/窗口/装备行为未运行验证（静态证据充分，但动态验证留给合入者）。
- CE244 的 pc36/pc44 在本任务用合成事件复现；pc36 在 task-31 真实数据（2000349=1 vs 自身槽 11/5）下两种语义都为假，不是结算差异来源，真正的门是 pc79。未改真实 CE244 数据；m7_ce244_direct 的 6 个用例仍按当前字面量语义绿。
- op17 这 2 个引用站点的 operator 语义在 compare 里是兜底"=="，与运算数解析无关，仍未定义；不影响本次结论。
- exec.rs 未改一行：本任务文件仅 新增 crates/wolf-vm/tests/opcode_111_semantics.rs、新增本文档；未触碰 wolf-game、battle 测试、rng.rs、docs/BASELINE.json、docs/HANDOFF_M4.md、3 个永久 scratch 文件；未 commit/push。

## 与其它交付不冲突

- exec.rs 正被 m6-multiday（task-39，opcode 250 写分支 ~exec.rs:2043-2054）占用；本任务明确不写 exec.rs，补丁仅记录在本文档与 Lead 报告中，供其合入后统一跑回归。
