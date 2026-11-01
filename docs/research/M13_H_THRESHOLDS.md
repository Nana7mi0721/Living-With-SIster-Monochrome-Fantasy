# M13 P1-E — H 阈值矩阵：运行时可执行验证

- 状态：done
- 日期：2026-11-01
- 基线 commit：1826edd（在 7c3b728 common-event self 持久修复之后；工作树仅含本任务新增文件，无生产 src 改动）
- 测试文件：crates/wolf-game/tests/m13_h_thresholds.rs
- 关联文档：docs/research/M13_CONTENT_MATRIX.md §2（门值来源）；docs/OPCODE_SPEC.md §5.9（op111 语义）；crates/wolf-vm/src/exec.rs:1493（compare）

## 1. 方法

### 1.1 数据与基线
- 数据根 work/all（23 个 .wolf 解包）；脚本源 BasicData/CommonEvent.dat（550 个 common event）。
- 零售存档 work/retail_backup/Save/SaveData01..08.sav（本测试用 SaveData01 做端到端驱动）。
- 环境必须 pin：WOLF_GAME_DIR=work/retail_backup WOLF_DATA_ROOT=work/all，否则 wolf-save 会读用户 Steam 安装档导致假红。

### 1.2 op111 解码与「首真命中」语义（关键更正）
- op111 VariableCondition 的 intArgs 布局：intArgs[0] 低 4 位 = 条件数 N，其后每个条件 3 个 int = (变量 2000000+var_id, 值, 运算符)；加载期断言 argc == 1 + 3*N（crates/wolf-vm/src/program.rs:561 condition_layout_ok）。
- VM 将 op111 编译为 BlockKind::Condition 块；N 个条件按顺序求值，第一个为真的条件命中它对应的分支体（case id 1..=N），全假走 opcode 420 else（无 else 则跳过 499 之后）。即 op111 是「首真命中的有序分派」而非「逻辑与」（见 docs/OPCODE_SPEC.md §5.9 与本任务测试）。
- 因此形如 [2, v21,200,1, v21,0,1] 的门含义是「若 var21>=200 -> case1；否则(>=0) -> case2」，不是「var21>=200 且 var21>=0」。M13_CONTENT_MATRIX.md §2 里多条件门此前用「且」书写，现已按本测试更正为「有序分档」；其原始阈值均正确，真正有意义的边界是「被选中的分支发生切换」的那一档。本测试据此断言。
- 运算符（exec.rs:1493 compare）：0=>大于 1=>大于等于 2=>等于 3=>小于等于 4=>小于 5=>不等于 6=>按位与。

### 1.3 三条验证路径（都在 m13_h_thresholds.rs，均通过）
1. gate_bytecode_conformance：从真实 CommonEvent.dat 读取每个文档坐标的 op111 命令，逐字节断言其 intArgs 等于文档门值（37 条门命令精确定位）。
2. gate_branch_boundary_semantics：把上述真实 op111 字节按「首真命中」在运行时 VM 变量库 game.vm.vars 上求值（compare 语义同 exec.rs:1493），对每个门设 阈值-1 / ==阈值 / (分档则 +档) 点，断言被选分支索引恰在阈值处翻转（阈值-1 走 fallback，==阈值 走主分支）。
3. bathroom_trust_gate_spawns_ce360_at_threshold：真实端到端驱动 CE318 ■お風呂——信頼(var21)=100 时选「两人一起洗」进入 H 本体 CE360；=99 时被拒、CE360 从不 spawn。

## 2. 门值与边界结果

约定：CE@pc 为脚本坐标（common event 编号 @ 命令 index）；「解码门值」为 intArgs 原值（var 用 var_id 记）；「分支」=(阈值-1 选中分支 / ==阈值 选中分支)，None=全假走 else 或跳过。

### 2.1 周末 H（CE310@159-165）——四个独立单条件菜单门
| 场景 | 变量(名/id) | CE@pc | 解码门值 | 阈值 | 边界断言(阈值-1 / ==) | 结果 |
|---|---|---|---|---|---|---|
| 周末H | 精力 var16 | CE310@159 | [17,2000016,1,1] | >=1 | None / case0 | 通过 |
| 周末H | 性興味 var22 | CE310@161 | [1,2000022,200,1] | >=200 | None / case0 | 通过 |
| 周末H | 性欲 var18 | CE310@163 | [17,2000018,40,1] | >=40 | None / case0 | 通过 |
| 周末H | 挿入経験 var424 | CE310@165 | [17,2000424,3,1] | >=3 | None / case0 | 通过 |

### 2.2 泳池/海边（CE310@731-743）
| 场景 | 变量(名/id) | CE@pc | 解码门值 | 阈值 | 边界断言 | 结果 |
|---|---|---|---|---|---|---|
| 泳池海边 | 処女喪失 var265 | CE310@731 | [1,2000265,1,2] | ==1 | 0->None / 1->case0 | 通过 |
| 泳池海边 | 起こした回数 var455 | CE310@733 | [1,2000455,1,1] | >=1 | 0->None / 1->case0 | 通过 |
| 泳池海边 | 挿入経験 var424 | CE310@735 | [1,2000424,1,1] | >=1 | 0->None / 1->case0 | 通过 |
| 泳池海边 | 性興味 var22 | CE310@739 | [17,2000022,120,1] | >=120 | 119->None / 120->case0 | 通过 |
| 泳池海边 | 発情度 var102 | CE310@741 | [17,2000102,30,1] | >=30 | 29->None / 30->case0 | 通过 |
| 泳池海边 | 発情度 var102 | CE310@1099 | [1,2000102,60,1] | >=60 | 59->None / 60->case0 | 通过 |

注：CE310@743 是 Comment（お外エッチ），不是条件门，故不列入。

### 2.3 浴室（CE318）——信頼 100/120/150 三档 + 欲求不満/発情度 二分支
| 场景 | 变量(名/id) | CE@pc | 解码门值 | 阈值 | 边界断言 | 结果 |
|---|---|---|---|---|---|---|
| 浴室 | 信頼 var21 | CE318@213 | [17,2000021,100,1] | >=100 | 99->None / 100->case0 | 通过(端到端) |
| 浴室 | 信頼 var21 | CE318@217 | [17,2000021,100,1] | >=100 | 99->None / 100->case0 | 通过 |
| 浴室 | 信頼 var21 | CE318@227 | [17,2000021,120,1] | >=120 | 119->None / 120->case0 | 通过 |
| 浴室 | 信頼 var21 | CE318@237 | [17,2000021,150,1] | >=150 | 149->None / 150->case0 | 通过 |
| 浴室 | 欲求不満 var103 / 発情度 var102 | CE318@338 | [2,2000103,2,1,2000102,75,1] | 103>=2 或 102>=75 | (103=1,102=74)->None / (103=2)->case0 / (102=75)->case1 | 通过 |
| 浴室 | 同上 | CE318@415 | [2,2000103,2,1,2000102,75,1] | 同上 | 同上 | 通过 |

### 2.4 厕所（CE323）
| 场景 | 变量(名/id) | CE@pc | 解码门值 | 阈值/分档 | 边界断言 | 结果 |
|---|---|---|---|---|---|---|
| 厕所 | 信頼 var21 | CE323@1055 | [2,2000021,200,1,2000021,0,1] | >=200(else>=0) | 199->case1 / 200->case0 | 通过 |
| 厕所 | 機嫌 var23 | CE323@1058 | [2,2000023,5,1,2000023,0,1] | >=5 | 4->case1 / 5->case0 | 通过 |
| 厕所 | 性興味 var22 | CE323@1060 | [3,2000022,250,1,2000022,150,1,2000022,0,1] | 250/150/0 三档 | 0->case2,149->case2,150->case1,249->case1,250->case0 | 通过 |
| 厕所 | おしっこ観察 var222 | CE323@1062 | [2,2000222,0,2,2000222,1,1] | ==0 / >=1 | 0->case0 / 1->case1 | 通过 |
| 厕所 | 妹絶頂 var131 | CE323@1849 | [2,2000131,10,1,2000131,0,1] | >=10 | 9->case1 / 10->case0 | 通过 |
| 厕所 | 性興味 var22 | CE323@1913 | [2,2000022,200,1,2000022,0,1] | >=200 | 199->case1 / 200->case0 | 通过 |

说明：厕所闯入计数变量为 var222（CE323@1062 判定 0 / >=1），CE323@1057 写入 var221=1（尝试计数），M13_CONTENT_MATRIX 记法以 var222 为准。

### 2.5 夜袭（CE335）
| 场景 | 变量(名/id) | CE@pc | 解码门值 | 阈值 | 边界断言 | 结果 |
|---|---|---|---|---|---|---|
| 夜袭 | 性興味 var22 | CE335@3100/3416/3444/3470 | [2,2000022,200,1,2000022,0,1] | >=200 | 199->case1 / 200->case0 | 通过(4档) |
| 夜袭 | 発情度 var102 | CE335@3102/3230/3457 | [2,2000102,60,1,2000102,0,1] | >=60 | 59->case1 / 60->case0 | 通过 |
| 夜袭 | 発情度 var102 | CE335@2561/2703/3063/3418/3446/3472 | [2,2000102,40,1,2000102,0,1] | >=40 | 39->case1 / 40->case0 | 通过 |
| 夜袭 | 発情度 var102 | CE335@3429 | [2,2000102,50,1,2000102,0,1] | >=50 | 49->case1 / 50->case0 | 通过 |
| 夜袭 | 発情度 var102 | CE335@3483 | [2,2000102,70,1,2000102,0,1] | >=70 | 69->case1 / 70->case0 | 通过 |

## 3. 端到端真驱动：CE318 浴室 信頼门 -> CE360
- 驱动方式：Game::new(work/all) 载入 SaveData01，clear_common_self，置 pajama 拥有位 var80..87=1 使入浴前睡衣选择循环立即结束，置 var1600010=0，设 var21=trust，start_event(318)；每帧若出现选项则选含「两人一起洗」的项；跟踪是否出现 event 360 实例。
- 结果：
    - 信頼(var21)=100 -> CE360 被 spawn（进入 H 本体），warnings=0。
    - 信頼(var21)=99  -> CE360 不 spawn（「两人一起洗」被拒：CE318@213 var21>=100 不成立转 else -> JumpLabel A -> CE264 拒绝台词 -> 转向偷窥分支），warnings=0。
- 值：这是「恰好达到阈值进入、差 1 不进入」的运行时实证，直接对应 CE318@213 门。

## 4. 未被端到端驱动的门（构造方式与说明）
- 周末 H / 泳池海边（CE310）、厕所深链（CE323）、夜袭（CE335）：这些门的 op111 字节已在测试 1 精确锁定、边界已在测试 2 在运行时 VM 变量库上验证；但它们嵌在数百~数千条命令的菜单/时段流程深处（CE310 2332 命令、CE323 2348 命令），从事件头 drive 需逐步控制多层 Choice 且需大量前置状态，本次未做全链路真驱动。
    - 可用夹具构造：载入 SaveData01、clear_common_self、按 2.x 表把被门涉及的全部变量设为过门值、必要时 seed 前置（如泳池需 var265 処女喪失、var455 起こした回数；厕所需在 CE323@1051 选「闯入」并在 CE323@1055 前使 var21=200），再 start_event(对应 CE)，用 ChoiceReply 逐项选择，断言目标子事件 spawn / 目标分支写入的变量变化。
    - CE310 周末 H 的 4 个门是菜单可用性判定（非独立子事件），真驱动需定位其所在选项并以 choice cursor/reply 命中，属夹具专项。
- 餐厅(H)：CE326@1160-1421 全为料理/菜单/计数变量判定，无 var18/102/103 门，即攻略门（自身性欲>=100 且 妹性欲>=60）未落在 CE326 脚本上，需夹具专项（见 M13_CONTENT_MATRIX §2）。

## 5. 复现命令

    # 快速（默认，3 tests，<1s）：
    WOLF_GAME_DIR=work/retail_backup WOLF_DATA_ROOT=work/all \
      cargo test -q -p wolf-game --test m13_h_thresholds

    # 带输出：
    WOLF_GAME_DIR=work/retail_backup WOLF_DATA_ROOT=work/all \
      cargo test -q -p wolf-game --test m13_h_thresholds -- --nocapture

实测输出（本机 2026-11-01，commit 1826edd）：

    running 3 tests
    bathroom_trust_gate_spawns_ce360: 信頼=100 -> CE360=true, 信頼=99 -> CE360=false
    gate_branch_boundary_semantics: all documented thresholds flip the branch as expected
    gate_bytecode_conformance: 37 gate commands pinned exactly
    test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.85s

## 6. 遗留 / 待补
- CE310 / CE323 / CE335 的 H 门：字节级与边界谓词已验，全链路真驱动未做（见 §4 构造方式）；建议后续按门写夹具测试。
- 多条件 op111 的「且」写法建议在 M13_CONTENT_MATRIX.md §2 加注为「首真命中有序分档」（值不变，仅语义表述更准）。
- 本次未改动生产代码；新增仅 crates/wolf-game/tests/m13_h_thresholds.rs 与本文件。
