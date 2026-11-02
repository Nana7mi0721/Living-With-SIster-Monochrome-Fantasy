# M13 条件注入型随机事件运行时验证（P2 / task-15）

本文是 task-15 的交付文档：对 docs/research/M13_CONTENT_MATRIX.md §1.2 归入「条件注入」层的 17 条随机事件（16 个 DLC + 特4 海豹任务密封）做**运行时门变量注入 + pc 定点跳转**验证，逐条断言渲染出预期台词且 warnings==0。配套测试：`crates/wolf-game/tests/m13_random_events_injected.rs`（新增，未改生产 src）。

## 1. 测量基线

- 测量日期：2026-11-01
- 仓库 HEAD：19b177b876c8a2dd50d3b504a4f69b5ee1bbc257（短 19b177b）
- 数据根：work/all（测试时 `WOLF_DATA_ROOT=work/all`）
- 工具链：rustc / cargo 1.99.1
- 依据清单：docs/research/M13_CONTENT_MATRIX.md §1.2（条件注入层 17 条）
- 测试文件：crates/wolf-game/tests/m13_random_events_injected.rs（新增）
- 运行命令：`WOLF_DATA_ROOT=work/all cargo test -p wolf-game --test m13_random_events_injected -- --nocapture`

## 2. 方法与关键标定

1. **pc == dump index**：WolfTL dump JSON 每条命令的 `index` 字段即运行时 `inst.pc`。把某事件 `inst.pc` 设为已知分支注释/台词所在的 index，即可定点进入目标块，绕开随机分发。
2. **门变量注入**：从 dump 的 op111 条件块读出 (`var_id`, `threshold`, `op`) 三元组，用 `game.vm.vars.set(index, value)` 注入满足值；局部选择器 self#n 位于扁平槽位 600+n（`selfs` 注入），全局开关用 `globals` 注入。
3. **双重断言**：
   - 运行时：`drive(ce, selfs, globals, pc, max_frames)` 驱动并断言渲染消息包含预期子串、warnings==0（`assert_reached`）。
   - 静态：`injected_gate_triples_present` 复核每个用例点名的门三元组在 dump 中确实存在（防止写了不存在的门坐标）。
4. 结果：**16 DLC + 特4 两条 = 18 个事件点，全部通过；3 个测试全绿**（`injected_dlc_events_reach_expected_block` 16/16、`special4_seal_clear_reachable` 2/2、`injected_gate_triples_present`）。所有用例 warnings==0。

## 3. 逐条结果（18 事件点）

| # | 事件 | CE@pc | 门（op111 三元组 / 注释） | 注入值 | 断言台词片段 | 结果 |
|---|---|---|---|---|---|---|
| #10 | 紫桑的慰问（DLCお昼） | CE460@5 | @1 [1600010,1,2] self#10==1；DLC var619 | self#10=1, var619=1 | 慰劳品 | ✔ warnings=0 |
| #28 | 和优衣一起探病（DLC訪問） | CE465@104 | @95 [2000593,1,2] var593==1；@97 [2000042,10,1] var42>=10 | var593=1, var42=20 | 由衣，你没事吧？ | ✔ warnings=0 |
| #39 | 自己去卖胖次（DLC EP3） | CE452@581 | DLC var619（EP3） | var619=1 | 向熊镇 | ✔ warnings=0 |
| #45 | 撸猫（看板猫） | CE461@1081 | @1079 self#10∈4..7；@1082 self#20∈0..2 | self#10=4, self#20=0 | 小猫 | ✔ warnings=0 |
| #54 | 吃烤曼德拉草（DLC EP20） | CE452@2983 | DLC var619（EP20） | var619=1 | 烤曼陀罗草 | ✔ warnings=0 |
| #58 | 和加奈优衣外面吃饭（買い出し） | CE461@1355 | DLC var619（買い出し） | var619=1 | 东西基本买好了 | ✔ warnings=0 |
| #59 | 被不良顾客缠上 | CE461@1314 | @1320 self#10∈1..3 | self#10=1 | 被坏心眼的客人缠上了 | ✔ warnings=0 |
| #62 | 遇紫桑换衣服（DLC EP29） | CE452@4750 | DLC var619（EP29） | var619=1 | 由香莉姐还在睡 | ✔ warnings=0 |
| #63 | 和小霞出去转换心情（カスミ） | CE461@258 | @256 var565==1；@259 self#23∈0..2 | var565=1, self#23=0 | 和香澄美一起寻找素材了 | ✔ warnings=0 |
| #65 | 抱优衣拿东西（ユイ） | CE461@43 | @41 var573∈1..3；@44 self#22∈0..2 | var573=1, self#22=0 | 与由衣的亲密度上升了 | ✔ warnings=0 |
| #69 | 和优衣去面包店（討伐前段） | CE449@45 | DLC var619 | var619=1 | 源头很可能是 | ✔ warnings=0 |
| #76 | 被说成恐怖组织（低评价分支） | CE461@1409 | @1374 tiers var11>=90/60/30/0（低评价分支） | var11=0（<30） | 用炸弹把委托人都炸飞了 | ✔ warnings=0 |
| #80 | 和加奈一起跑步（カナ） | CE461@460 | @461 self#21∈0..3 | self#21=0 | 和加奈一起锻炼了 | ✔ warnings=0 |
| #84 | 大家一起吸猫（同 #45） | CE461@1081 | @1079 self#10∈4..7；@1082 self#20∈0..2 | self#10=4, self#20=0 | 喵—— | ✔ warnings=0 |
| #101 | 遇小霞狩猎目标（同 #63） | CE461@258 | @256 var565==1；@259 self#23∈0..2 | var565=1, self#23=0 | 和香澄美一起寻找素材了 | ✔ warnings=0 |
| #111 | 忙碌的一天 | CE461@1433 | @1431 self#10∈8..10 | self#10=8 | 今天可真忙啊 | ✔ warnings=0 |
| 特4 | 海豹驱除作战（密封） | CE437@22 | @10 [2000535,0,2] var535==0；@15 self#11==0 | var535=0, self#11=0, var533=1 | 海豹驱除作战 | ✔ warnings=0 |
| 特4 | 海豹跃出水面（密封） | CE438@82 | @32 [2000449,1,1] var449>=1 | var449=1, var533=1 | 海豹跃出水面 | ✔ warnings=0 |

## 4. 门三元组静态复核

`injected_gate_triples_present` 逐条断言下列位置存在精确的 (var_id, threshold, op) 门序列（op 映射：0=`>` 1=`>=` 2=`==` 3=`<=` 4=`<` 5=`!=`；self 选择器偏移 1600000，全局 var 偏移 2000000）。测试通过（`assert_eq!` 全相等）：

| CE@pc | 门三元组序列 | 语义 |
|---|---|---|
| CE460@1 | (1600010,1,2) | self#10==1 |
| CE465@97 | (2000042,10,1) | var42>=10（由衣好感） |
| CE465@95 | (2000593,1,2) | var593==1 |
| CE461@1079 | (1600010,4,2),(…,5,2),(…,6,2),(…,7,2) | self#10∈4..7 |
| CE461@1082 | (1600020,0,2),(…,1,2),(…,2,2) | self#20∈0..2 |
| CE461@1320 | (1600010,1,2),(…,2,2),(…,3,2) | self#10∈1..3 |
| CE461@256 | (2000565,1,2) | var565==1 |
| CE461@259 | (1600023,0,2),(…,1,2),(…,2,2) | self#23∈0..2 |
| CE461@41 | (2000573,1,2),(…,2,2),(…,3,2) | var573∈1..3 |
| CE461@44 | (1600022,0,2),(…,1,2),(…,2,2) | self#22∈0..2 |
| CE461@1374 | (2000011,90,1),(…,60,1),(…,30,1),(…,0,1) | var11>=90/60/30/0（评价分档） |
| CE461@461 | (1600021,0,2),(…,1,2),(…,2,2),(…,3,1) | self#21∈0..3 |
| CE461@1431 | (1600010,8,2),(…,9,2),(…,10,2) | self#10∈8..10 |
| CE437@10 | (2000535,0,2) | var535==0 |
| CE438@32 | (2000449,1,1) | var449>=1 |

## 5. 风险与限制

1. **pc 定点跳转绕过前置控制流**：只证明「门满足 + 跳到该块 → 台词可达」。门的真实存在由 §4 静态复核补证，但运行时不重放整条分发的完整控制流；分发级可达性另见 docs/research/M13_RANDOM_EVENTS.md（池选择器抽样）。
2. **#28 CE465 定点坐标**：pc=108（看病块注释）实测会串到无关分支（渲染「与加奈聊天了。」「…呼。」，因 @108 后控制流跳转），改用 pc=104（同台词「由衣，你没事吧？」的入口消息）后正确命中。这属于**测试夹具坐标问题**，非事件缺失。
3. **#54 CE452@2983 ended=false**：该 DLC EP20 分支很长（含能量饮料/厕所等后续），在 max_frames=1200 内未走到 ended，但预期台词「烤曼陀罗草」已渲染且 warnings==0；`assert_reached` 不要求 ended，判定通过。
4. **DLC 均需 var619=1**：DLC 事件在无 DLC 旗标时数据不可达；本测试统一注入 var619=1 模拟 DLC 已激活，与 8 个零售存档 var619=0 的实际状态区分（见 docs/research/M12_RETAIL_SCOPE.md）。
5. **未覆盖 特1–特3**：这三条已由 docs/research/M13_RANDOM_EVENTS.md 的池分发抽样覆盖（CE278/CE279/CE280 等），本文只补「条件注入」层 17 条（16 DLC + 特4 两事件点），二者合并即覆盖 115+ 行清单中全部可运行时验证项。

## 6. 复现

```bash
cd 'E:/Study/github program/imoto life'
export WOLF_DATA_ROOT=work/all
cargo test -p wolf-game --test m13_random_events_injected -- --nocapture
# 预期：test result: ok. 3 passed; 0 failed
```