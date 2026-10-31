# M10 阶段一：opcode 111 引用语义「首个分歧点」探针报告

> 任务：task-69（M10 Phase1）。目的：在**不污染共享主工作树**的前提下，用 reference（值操作数走 eval_num）语义重跑 15 个会变红的测试，
> 逐个定位「第一个**分支发生翻转**的 opcode 111 站点」（首个分歧点），给出 CE/pc/变量/左值/字面量 vs 引用值/下游断言失败，
> 并分类为 桩伪造(fixture)/真语义差/过时断言。本文全部为 [实测]，除注明 [推断]。
>
> HEAD bcbcfba 仍绿；补丁仍 park 在 work/out/opcode111_landed.patch；本任务未改任何生产代码/测试，未 commit。

## 0. 方法与可复现命令

- 隔离副本：`work/out/m10probe/` = 仓库源码（Cargo.toml/Cargo.lock/rust-toolchain.toml/crates/ + work/retail_backup），**不含 target**。
- 在该副本 `crates/wolf-vm/src/exec.rs` 的 opcode 111 分支（BlockKind::Condition，原循环 2220–2228，原 `let value = arg(cmd, 2 + 3*i)` 在 2222 行）
  改为 reference 语义 `let raw_value = arg(...); let value = eval_num(raw_value, inst, vars);`，并加日志：
  当 `M10_OP111_LOG` 置位且 `raw_value != value` 时，计算 `d_lit=compare(lhs,raw_value,op)` 与 `d_ref=compare(lhs,value,op)`，
  若二者不同则打印 `M10DIV event=<CE> pc=<pc> var=<目标变量> raw=<原值操作数> lit=<按字面量> ref=<按引用> lhs=<左值> op=<运算符> d_lit/ d_ref`。
  每条日志即「第一个分支翻转点」。
- 构建：`cd work/out/m10probe && export CARGO_TARGET_DIR="/e/Study/github program/imoto life/work/out/m10probe_target" && cargo test -p wolf-game --no-run`（~46s，exit 0）。
- 单测运行（15 个逐个）：
  `WOLF_DATA_ROOT="/e/Study/github program/imoto life/work/all" M10_OP111_LOG=1 cargo test -q -p wolf-game --test <stem> <testfn> -- --exact --nocapture`
  日志落 `/tmp/op111_<stem>.log`。15 个在 reference 语义下**全部 exit 101（红）**，与预期一致。
- 共享主工作树未插入任何探针/测试：探针版 exec.rs 与日志仅存在于 work/out/m10probe，主树保持绿。

## 1. 首分歧总表（每测试按时间顺序的第一个 M10DIV）

| # | 测试 (crates/wolf-game/tests) | 首分歧 CE/pc | 触发 opcode 111 | 左值 lhs | 字面量→引用值 | lit/ref 翻转 | 后果（下游断言） | 分类 |
|---|---|---|---|---|---|---|---|---|
| 1 | m6_battle_commands | CE172 pc32 | `111[1,self23,self20,2]`(死亡数==己方数) | 1(=死亡) | 1600020→1 | false→**true** | 走「味方全灭/敌方胜利」:1600099=-1+op172中止 → 到不了 CE192 (:144) | 桩伪造 |
| 2 | m6_battle_real | CE172 pc32 | 同上 | 1 | 1600020→1 | false→true | 同上，「CE192 never reached」(:158) | 桩伪造 |
| 3 | m7_battle_full | CE172 pc32 | 同上 | 1 | 1600020→1 | false→true | 中止 → 到不了 CE152 (:368) | 桩伪造 |
| 4 | m7_battle_menu_text | CE172 pc32 | 同上 | 1 | 1600020→1 | false→true | 中止 → 到不了 CE192 (:183) | 桩伪造 |
| 5 | m7_battle_round | CE172 pc32 | 同上 | 1 | 1600020→1 | false→true | 中止 → 到不了 CE152 (:255) | 桩伪造 |
| 6 | m7_battle_seed | CE172 pc32 | 同上 | 1 | 1600020→1 | false→true | 中止 → 到不了 CE192 (:144) | 桩伪造 |
| 7 | m7_battle_target_scheduler | CE242 pc184 | `111[1,2000029,2000028,1]`(战力>=阈值) | 1796 | 2000028→1510 | false→**true** | var29 被夹到 1510；var350 收 0(期望 2)(:363) | 真语义差 |
| 8 | m7_ce244_direct | CE244 pc36 | `111[1,2000349,self10,2]`(回合==结局码) | 11 | 1600010→11 | false→**true** | 进「战斗结束」展示→PC44 判 敗北 -1（原走胜利 1），期望 outcome1 got -1(:137) | 过时断言(钉死字面量bug) |
| 9 | m7_day4_playable | CE239 pc181 | `111[1,2000341,self72,1]`(战力>=下限) | 127 | 1600072→40 | false→true | 之后 186/242p375/243 多处夹取 → 「round must settle」(:570) | 真语义差 |
| 10 | m8_battle_command_input | CE172 pc32 | 同 #1 | 1 | 1600020→1 | false→true | 中止 → 「CE65 must run」(:163) | 桩伪造 |
| 11 | m8_battle_party | CE239 pc181 | 同 #9 | 1894 | 1600072→450 | false→true | 后续 186/242p1099/243 夹取 → 战斗未决(left0/right1)(:183) | 真语义差 |
| 12 | m6_mainline | CE48 pc133 | `111[1,self23,self19,2]`(选中位ID==主角ID) | 1? 实测 l/r 均… | 1600019→? | (见 §2) | 指令数 9792(断言 >10000)(:427) | 过时断言/敏感性 |
| 13 | m6_training_menu | CE226 pc15 | `111[18,self12,0,3, self12,self10,1]`(行动力<=0 / >=上限) | 100 | 1600010→60 | 第2条件 false→true | var14 被夹到 var13=60(期望100)(:162) | 真语义差 |
| 14 | m7_hmode_state | CE226 pc15 | 同 #13 | 100 | 1600010→60 | false→true | 后接 CE329 pc233 再夹 → var14 35(期望115)(:241) | 真语义差 |
| 15 | m8_hscene_forward | CE339 pc310 | `111[1,self21,self22,5]`(H槽!=H槽上限) | 0 | 1600022→0 | **true**→false | 不再置 var2000198=0；后 CE371 timer== 分支 → var218 收 0(期望2)(:890) | 真语义差 |

> 注：#9/#11 的 `d_lit/d_ref` 实测：CE239 pc181 `lhs127 raw1600072 ref40 op1 d_lit=false d_ref=true`（>=下限）；pc186 `lhs45 raw1600073 ref30 op3 d_lit=true d_ref=false`（<=上限，补丁**移除**了原来恒真的夹取）。

## 2. 关键站点逐条语义（Dump 自 work/_ce_full.json；a=[ints]，op 1>= 2== 3<= 5!= 0→'原样相加'，op111 头部低4=条件数）

- **CE172 `X[戦]敵/味方·勝敗判定`**（判胜负）：pc21 `250[21,4]` 读 CData type18 敌方`ALLY/己方计数`→self20；pc22 self21=-1；pc24-31 循环对每槽调 CE176（op300 `[0,16777218,self21,0,self22]`，arg1=0）；
  **pc32 `111[1,1600023,1600020,2]`：死亡累计(self23) == 己方总数(self20) → pc34-36 打印「味方が全滅」、1600099=-1、op172 BreakEvent**。
  pc42 再读敌方数(type18 r69)，pc53 同样 dead==total → 胜利。**字面量下 pc32/pc53 拿小计数比 1600020/1600069 恒假 → 从不中止**（这正是旧测试能「走到 CE192」的原因，也是 bug）。
- **CE176 `X[戦]基本状態取得`**：pc13 `111[3,self1,0,2, self1,1,2, self1,2,2]` 按 self1(0/1/2) 选模式；pc16 读 CData type10【戦闘不能？】列→self10。
- **CE48 `X[共]基本システム自動初期化`(run=3)**：pc122 `250[18,51]` 取队伍数→self16；pc133 `111[1,self23,self19,2]`（选中位主角ID == 主角ID）相等则 pc135 把「选中位主角ID」设为 -1（去重/清除）。新游戏引导时二者皆为 -1，reference 下 -1==-1 成立 → 进入该清除分支（字面量恒假不进）。
- **CE244 `┗終了判定`**：pc31 `111[1,2000354,13,2]`（战斗上下文==13→self10=10）；**pc36 `111[1,2000349,self10,2]`（回合==结局码=11）成立则进入「战斗结束」展示，pc44 `111[17,2000192,2000193,0]` 比较两结果变量决定 勝利/敗北**。字面量下 pc36 恒假 → 直接落到胜利 (#8 断言把该 bug 钉死)。
- **CE329 `┗就寝処理`**：pc225 `121[2000102,-30]`（体力-30）；**pc233 `111[1,2000016,2000015,1]` var16>=var15 → pc235 var16=var15 夹取**（lhs1/ref0 实测）。
- **CE226 `■行動力変動`**：**pc15 `111[18,self12,0,3, self12,self10,1]`**：条件1 self12<=0→pc17 var14=0；条件2 self12>=self10→pc20 var14=var13（夹到当前上限）。实测 lhs self12=100、raw self10→ref=60，reference 100>=60 成立 → var14=var13(=60)，而字面量下这第二条件恒假、var14 保留起始值(100/115)。
- **CE63 `X[共]メッセージウィンドウ`**：pc74 self49=9000000；**pc75 `111[1,self48,self49,5]`（self48 != self49）→ pc77 self48=self49**（人脸号自动显示）。实测 l/r 均 1，reference 不成立、字面量成立。
- **CE242 `┣コマンド処理`**：pc183 var29(战力)+=self15；**pc184 `111[1,2000029,2000028,1]` var29>=var28 → pc186 var29=var28（夹取）**。另 **pc374 self30=…, pc375 `111[17,self30,self13,3]`（self30<=self13）实测 lhs22 ref5 → 字面量真/引用假**（补丁移除一恒真分支）。
- **CE243 `┣敵の行動`**：pc40 `111[1,2000354,11,2]`；pc42 self10=2000341(伤害)；pc45 self31=2000028；**pc46 `111[1,self10,self31,1]` 伤害>=敌HP → pc48 夹取**（实测 lhs1171 ref377）。**pc813/pc995 `111[17,self30,self12,3]`（self30<=self12）实测 lhs45 ref15 → 字面量真/引用假**（原恒真分支被移除）。pc1054/pc1071 同类。
- **CE239 `■戦闘呼び出し`**：pc180 2000341=self70+self71；**pc181 `111[1,2000341,self72,1]`（>=下限→pc183 夹为下限）；pc186 `111[1,2000341,self73,3]`（<=上限→pc188 夹为上限）**。实测 day4: 127>=40 真、45<=30 假(字面量真);m8_party: 1894>=450 真、529<=350 假。
- **CE436 `引き継ぎPチェック`**：pc100 `221` 读 System.sav→var2000550；**pc101 `111[1,self20,2000550,0]`（op0）实测 lhs2, raw2000550, ref0 → 字面量假/引用真**。
- **CE339 `Hゲージ関連`(run=2)**：**pc310 `111[1,self21,self22,5]`（self21!=self22 → pc312 var2000198=0）实测 lhs0 ref0 → 字面量真/引用假**（补丁移除一恒真分支）。
- **CE371 `トイレタイマー1`**：**pc137/221 `111[1,self20,self0,2]`（self20==self0 → pc139 wait50, pc140 self0=0, pc141 self20=0）实测 lhs10 ref10 → 引用真/字面量假**（补丁**新增**一真分支）。

## 3. 分类结论

- **(a) 过时断言（测试把字面量 bug 当成期望值，需改期望/放宽）**：
  - **#8 m7_ce244_direct:137** —— 明确锁定「只有 raw self magic 才开胜负展示分支」的 bug；reference 下 PC36 11==11 成立→展示→判敗北(-1)。期望应改 -1 或改测「raw 哨兵」之外的真回合值。
  - **#12 m6_mainline:427** —— 断言「睡眠脚本真跑：>10000 指令」实测 9792；指令数对分支翻转敏感（CE48/CE329/CE226/CE63 分支变化），应放宽到 ~9500 或改为断言关键变量（var63/时间）。
- **(b) 真语义差（补丁修正了此前被字面量掩盖的夹取/门控，测试旧期望基于错误行为，需按新语义定期望或补种真实状态）**：
  - #7 CE242 pc184（var29 夹到 var28）、#9 CE239 pc181/186 + CE242 p375 + CE243、#11 CE239 pc181/186 + CE242 p1099、#13 CE226 pc15（行动力夹到上限 var13）、#14 CE226 pc15 + CE329 pc233（var14 35 vs 115）、#15 CE339 pc310 + CE371（var218 0 vs 2）。
  - 这些站点 operand 是**自身变量引用**（self10/self12/self13/self15/self20/self22/self23/self31/self0/self48/self49/self72/self73，以及 var2000028 等），reference 语义是**正确引擎行为**（夹取到上限、判胜负、计时器相等复位）。补丁暴露了旧测试依赖「恒真/恒假」的错误分支。
- **(c) 桩伪造（fixture 不真实，导致 reference 在 CE172 提前判负；修 fixture 而非改期望）**：
  - **#1 #2 #3 #4 #5 #6 #10** 全部首分歧 = **CE172 pc32**：seed_party/裸 CE188 只写 CData type18 r4=1（己方计数 1），CE176 却把这唯一己方单位判为「戦闘不能」→ 死亡累计 1 == 总数 1 → reference 立即 1600099=-1 中止，根本到不了 CE152/CE192/CE65。**这是桩缺少「存活状态(type10 col3)」种子所致，不是真实战斗中止**。
  - 反证 [实测]：**真实 CE253/CE190 队伍路径（#9 day4_playable、#11 m8_battle_party）日志中完全没有 CE172 分歧** —— CE190 布阵会按 roster 重写 type18 r69（实测 1→7）并把单位标为存活(type10 col3=0)，故 dead(0)!=total(1或7)，CE172 不中止。这**再次印证 task-66 的更正**：0==0 瞬胜/瞬负只是空 roster 合成产物。
  - **Phase2 动作**：seed_party 系列 fixture 须补种 type10 存活单元（col3=0，含己方槽）或改走真实 CE190 布阵，之后它们的首分歧才会落到与 #7/#11 同类的夹取站点。

## 4. 对 Phase2「落地 opcode111 reference 语义且不弄红主树」的直接含义

1. **先修真 fixture（#1/#2/#3/#4/#5/#6/#10）**：给 seed_party/裸 CE188 增加存活状态种子（type10 col3=0 己方槽），消除 CE172 pc32 假中止；否则 reference 补丁在这 7 个测试上根本进不了战斗主体。
2. **夹取类站点（#7/#9/#11/#13/#14）**：这是 patch 的**正确**语义，旧测试期望值基于「夹取分支恒不被走」的错误状态。需按 reference 后真实数值改期望（如能量 var14 应从 100/115 改为 60 或补种 var13=100 使夹取后回到 100；var29 夹到 var28 等），或补种 retail System.sav 中真实的 var13/var28/self72/self73。
3. **过时断言（#8/#12）**：直接改期望值（#8 outcome→-1；#12 指令数放宽或换断言目标）。
4. **#15 H 线**：CE339 pc310 / CE371 计时器在 reference 下翻转，须在 H 自动事件推进链上按真实 H 槽上限重定 var218 期望。
5. 建议 Phase2 顺序：fixture 存活种子 → 逐工作流（battle 夹取→ #13/#14 能量→ #15 H）定新期望，每个 PR 保持 `cargo test --workspace` 全绿。

## 5. 附：每测试完整首分歧序列（时间顺序；均来自 /tmp/op111_<stem>.log）

- #7 target_scheduler: CE48 pc133 → CE63 pc75 → CE242 pc184 → CE243 pc46 → CE243 pc813 → CE243 pc995（var350 末值 0，期望 2）
- #9 day4: CE48 pc133 → CE63 pc75 → CE226 pc15 → CE329 pc233 → CE436 pc101 → CE239 pc181 → CE239 pc186 → CE242 pc375 → CE243 pc813 → CE243 pc995 → CE243 pc1054（**无 CE172**）
- #11 m8_party: CE48 pc133 → CE63 pc75 → CE239 pc181 → CE239 pc186 → CE242 pc1099 → CE243 pc1054 → CE243 pc1071（**无 CE172**）
- #12 m6_mainline: CE48 pc133 → CE329 pc233 → CE226 pc15 → CE63 pc75
- #13 m6_training_menu: CE48 pc133 → CE63 pc75 → CE226 pc15
- #14 m7_hmode_state: CE48 pc133 → CE63 pc75 → CE226 pc15 → CE329 pc233
- #15 m8_hscene_forward: CE339 pc310 → CE48 pc133 → CE63 pc75 → CE371 pc221 → CE371 pc137
- #1/#2/#3/#4/#5/#6/#10: 仅 1–2 行，均为 CE172 pc32
- #8 m7_ce244_direct: 单行 CE244 pc36

> 复现：在 work/out/m10probe 内按 §0 命令逐个跑；主树 HEAD bcbcfba 未受影响。
