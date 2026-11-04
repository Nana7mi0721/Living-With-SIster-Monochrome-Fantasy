# Review: op121 argc==5 的 arg[4] range 语义与战斗回归审计

- 任务：task-16（只读审计）
- 审计者：review-input-state
- 审计对象：当前工作树 `crates/wolf-vm/src/exec.rs` 的 opcode 121 实现，以及 `crates/wolf-game/tests/m9_battle_round_gap.rs`、`m9_full_route.rs`、`m7_battle_target.rs` 的 rebaseline。
- 只读声明：本次审计没有修改任何源码、测试、release、manifest 或存档；本文件是唯一新增产物。
- 最终判定：**PASS（语义与当前边界正确；战斗 rebaseline 合理）**，附带 5 个 P2 清理项，没有发现 P0/P1 级错误。

---

## 1. 结论摘要

1. **arg[4] 是 inclusive last offset（末槽偏移），不是 count（排他）、不是数组下标，也不是二维索引。** 槽数 = `arg4 + 1`。
2. 当前实现 `for offset in 0..=count` 的**边界是正确的**；把 CE94 pc87 的 `arg4=3` 解释为 4 个连续槽（`1600022..=1600025`）符合原始 Game.exe 的机器码和事件数据。
3. **当前工作树把战斗轨迹从 4 轮改为 2 轮，是合理的语义修正，不是误伤。** CE239/CE244 的 4 槽状态清零、CE241/CE242 对这些槽的读取，以及独立 `m11_battle_command_sequence` 仍保持 4/7 轮多回合，均支持这一结论。
4. 当前实现还存在 5 个不影响现有 236 条数据正确性的 P2 问题：
   - `exec.rs:1838-1839` 注释把 4 个槽写成 3 个槽（`1600022..1600024` 应为 `1600022..=1600025`）。
   - `exec.rs:1843` 用 `argc(cmd) >= 5` 作为 range 开关；原始引擎实际检查 `arg3 & 0x10000`（bit16）。当前 236 条 common + 49 条 map 数据恰好全部等价，但建议改为 bit16 以精确匹配原引擎。
   - 变量名 `count` 语义误导，应该叫 `last_offset`（或者 `last`）。
   - `m9_battle_round_gap.rs:1-25,611-613`、`m7_battle_target.rs:105-112` 的注释仍是旧基线/错误措辞，与新断言不一致（P2 测试卫生）。
   - 另有 20 条 `intArgs.len()==7` 的 map 异常条目（target=1100000000），建议加参数长度防御，避免未来被误当 range 执行。
5. 不建议采用“只对特定 target/arg4 组合打补丁”的窄修法。op121 是通用变量操作，CE175/178/179/185/239/244 等大量战斗/UI 绘制都依赖 range；按 target 白名单会让这些路径继续产生错误槽值。

---

## 2. arg[4] 语义的证据链

### 2.1 文档只能证明“存在 arg[4]”，不能证明语义

- `docs/OPCODE_SPEC.md:607`：argc 统计为 4:17022、5:236。
- `docs/OPCODE_SPEC.md:615-616`：明确 `[3]` 的 bit16 = 存在 `[4]`；`[4]` 含义仍是 **TODO/未确认**（原文档猜测“数组下标/XY 第二维”）。
- `docs/OPCODE_SPEC.md:253-256`：bit16 出现 236 次，与 argc==5 的 236 条完全吻合。
- `docs/vendor/WolfTL-main/WolfTL/WolfRPG/Command.hpp:684-691`：`SetVariable::GetValue()` 只 `return m_args[1]`，没有实现 arg[4] 语义，因此该参考实现不能作为边界依据。
- 结论：必须用零售版 `Game.exe` 的反汇编或事件数据反推，不能把文档的“疑似数组下标”当成事实。

### 2.2 Game.exe 反汇编：明确的 `0..=arg4` 循环

审计对象：`work/runcopy/Game.exe`，PE32，image base `0x400000`，SHA-256 `98949e233e4085d9c745b2e8ba41e17ff49e5dafff16e5f28882d39bcaf804ea`。

- opcode dispatch 位于 VA `0x4abeb0` 附近：`cmp ecx,0xfd; movzx eax, byte ptr [ecx + 0x4ad28c]; jmp dword ptr [eax*4 + 0x4ad1bc]`。
- 查表：opcode 121 -> index 122 -> byte 10 -> target `0x4abef8` -> `call 0x4aea70`；opcode 124 -> `0x4abf04` -> `call 0x4b25b0`。因此 **`0x4aea70` 是 opcode 121 的正式 handler**。
- `0x4aea70` 是正常函数序言：`push ebp; mov ebp,esp; sub esp,0x20c`。它先通过 `[ebx+0x88c]` 取当前 command，然后：
  - `lea ecx,[edx+0x34]; call 0x4c9070`：取参数 vector 的元素个数；
  - `push 0; call 0x4c8e40; mov eax,[eax]`：读第 0 个元素（opcode/类型）并跳转。
- 关键 range 逻辑从 VA `0x4af158` 开始：

```asm
xor edi,edi
mov ecx,edx
push 4
call 0x4c8e40          ; vector[4] = arg3（运算符位域）
test dword ptr [eax],0x10000
; 若 bit16 未置位 -> 不读 arg4，edi 保持 0
; 若置位：
push 5
call 0x4c8e40          ; vector[5] = arg4
mov edi,[eax]
cmp [eax],edi
jge ...
mov [ebp-0x20c],edi    ; last_offset = arg4（非负）
```

- 循环体在 VA `0x4af1b0`：

```asm
mov edx,[ebx+0x88c]
push 1
call 0x4c8e40
mov edi,[eax]          ; arg0（目标 magic）
add edi,esi            ; target = arg0 + esi，esi 从 0 开始
...
; 写入 target 后：
0x4afd58: mov esi,[ebp-0x214]
          inc esi
          cmp esi,[ebp-0x20c]   ; 与 arg4 比较
          jle 0x4af1b0          ; esi <= arg4 继续
```

这直接证明原始引擎执行的是：

```text
if (arg3 & 0x10000) != 0:
    last = arg4
    for offset in 0 ..= last:
        target = arg0 + offset
        write(target, compound(...))
else:
    write(arg0, compound(...))   ; 只写一个槽
```

因此：

- **arg4 不是 count**：如果是 count，循环应为 `offset < arg4`，会少写最后一槽；机器码是 `inc esi; cmp esi, arg4; jle`，即 `<= arg4`。
- **arg4 不是数组下标**：它参与的是 `target = arg0 + offset` 的连续槽地址计算，不是另一维基址。
- **arg4 不是绝对值/结束地址**：结束地址是 `arg0 + arg4`，arg4 本身是相对偏移。

### 2.3 事件数据与反汇编一致

#### CE94：存档窗口坐标缩放的四个槽

`work/out/ce94.txt:85-109`（同一绘制函数）：

- `:85-88` pc83-86 分别设置 `1600022 / 1600023 / 1600024 / 1600025` 四个坐标值（x、y、宽、高）。
- `:89` pc87：`[1600022, 10, 0, 66560, 3]`，即对 `1600022` 起、末偏移 3 的 **4 个槽**同时除以 10。
- 随后 `:90-93` 设置 `1600030..1600033` 四个值；`:94` pc92 同样 `[1600030, 10, 0, 66560, 3]` -> 4 个槽。
- `:95-98` 设置 `1600056..1600058` 三个值；`:98` pc96 `[1600056,...,2]` -> 3 个槽。
- `:99-101` 设置 `1600070..1600071` 两个值；`:101` pc99 `[1600070,...,1]` -> 2 个槽。
- `:102-109` 设置 `1600073..1600079` 七个值；`:109` pc107 `[1600073,...,6]` -> 7 个槽。

四/四/三/二/七槽分别对应 arg4 = 3/3/2/1/6，完全符合 `槽数 = arg4 + 1`。

#### CE178 / CE179 / CE185：不同长度的连续槽块

- `work/wolftl2/dump/common/178_X[戦]味方欄_単体描画.json`：
  - idx99-100 只设置 `1600024/1600025` 两个值；idx101 `[1600024,10,0,66560,1]` -> 2 槽。
  - idx102-124 连续设置 `1600060..1600082` 共 23 个值；idx125 `[1600060,10,0,66560,22]` -> 23 槽。
- `179_X[戦]敵キャラ_単体描画.json`：idx84-88 共 19 个值，idx89 `arg4=18` -> 19 槽。
- `185_X[戦]上部ステータス描画.json`：idx79-82 共 4 个值，idx83 `arg4=3` -> 4 槽；idx84-95 共 12 个值，idx96 `arg4=11` -> 12 槽；idx97-98 共 2 个值，idx99 `arg4=1` -> 2 槽。

这些不同长度的块无法用“arg4 是数组下标”或“arg4 是排他 count”解释，只能用“末槽相对偏移”解释。

#### CE38：初始化 600 个普通变量

`38_---------------------------------.json` idx2：`[2000001, 0, 0, 65536, 600]`。

- `2000001` 是普通变量 1（`2000000 + id`）。
- 若 `arg4=600` 是 inclusive last offset，则清 `2000001..=2000601`，即 601 个普通变量，符合“初始化全部普通变量”的用途。
- 若 `arg4=600` 只是数组下标，则只清 1 个变量，显然不符合上下文。

### 2.4 236 条 argc==5 的分布

扫描 `work/wolftl2/dump/common/*.json`：

- code 121 且 intArgs 长度 == 5 的指令共 **236 条**。
- arg4 分布：`1:111, 3:45, 2:35, 4:15, 9:9, 5:4, 7:3, 11:3, 6:3, 10:2, 22:2, 18:1, 600:1, 20:1, 17:1`。
- 所有 236 条的 `arg3` 都带 `0x10000` bit16。
- 另有 `work/wolftl2/dump/mps/*.json` 中 49 条 argc==5（arg4：1:34, 4:8, 3:3, 5:2, 7:1, 2:1），合计 285 条；只是任务统计的 236 条仅限 common。

### 2.5 反例/异常数据

扫描中还发现 **20 条 code121 的 intArgs 长度为 7** 的异常项（例如 `SampleMapA_1 events/6/pages/0/list` 索引 1541 附近），其中 `target=1100000000 (0x4190AB00)`、`arg4=0/10`。

- 这类 target 不在普通变量/自变量的正常 magic 范围内，且参数长度不符合 121 的 4/5 参数格式，疑似 map dump 的解析伪影或特殊编码。
- 当前 `argc(cmd) >= 5` 的逻辑会对这些条目也启用 range；如果它们被实际执行，可能写入异常 target。
- 现有路径未观察到它们执行；建议在 parser 或 op121 入口加防御（见 §5 P2-4），但不要因此否定 236 条正常数据的语义。

---

## 3. 当前边界判定：`0..=arg4` 正确

`crates/wolf-vm/src/exec.rs:1843-1853` 当前代码：

```rust
let count = if argc(cmd) >= 5 {
    eval_num(arg(cmd, 4), inst, vars).clamp(0, 1024)
} else {
    0
};
for offset in 0..=count {
    let slot = target.wrapping_add(offset);
    let current = eval_num(slot, inst, vars);
    let value = compound((bits & 0x0FFF) as i32, current, result);
    store_num(inst, vars, slot, value);
}
```

- 边界 `0..=count` 与反汇编的 `esi <= arg4` 一致；**PASS**。
- 变量名 `count` 和注释是错的，但实现边界不是错的。`exec.rs:1838-1839` 写“`1600022..1600024` 三个槽”，实际是 `1600022..=1600025` 四个槽；这是一处会误导后续维护的 P2 注释错误。
- `clamp(0, 1024)`：
  - 负数：原引擎在 VA `0x4af158` 对 arg4 做非负检查，负数直接不进入 range；当前 clamp 到 0 会执行一次 `offset=0` 写入。对“range 不生效”的语义而言，两者写入结果可能相同（单槽写入仍发生），但不是逐条等价；当前 236 条数据没有负数。
  - 大于 1024：原引擎没有上限；当前会截断。当前数据最大 600，未触发。建议保留上限作为防御，但在注释中说明这是实现保护而非原始语义。

### 3.1 为什么不能改成 `0..count`

如果按“count 排他”实现：

- CE94 pc87 `arg4=3` 只写 `1600022..1600024` 三个槽，`1600025`（高度）仍为 10 倍；存档窗口高度错误，和已知 UI 症状不符。
- CE178 idx125 `arg4=22` 只写 22 个槽，会漏掉 `1600082` 最后一个值；与 idx102-124 的 23 个赋值不匹配。
- CE38 `arg4=600` 只写 `2000001..2000600` 600 个，仍漏一个变量。

因此 `0..=arg4` 是唯一自洽边界。

### 3.2 为什么不能只按 target 白名单窄修

如果只为 CE94 的 `1600022` 开 range：

- CE175 pc72 `[1600020,10,0,66560,3]` 仍只缩放 x，漏掉 y、x-scale、y-scale；`m7_battle_target` 的 -288/512/312 无法得到。
- CE178/179/185 的存档/战斗状态栏绘制仍会少写槽。
- CE239/CE244 的 `2000386` 四行动标志仍只清一个，战斗继续出现旧 4 轮轨迹。
- CE38 的 600 变量初始化仍错误。

所以修法应是“按 op121 的 bit16 语义通用支持 range”，不是“按 target 特判”。

---

## 4. 战斗回归审计：4 轮 -> 2 轮是合理 rebaseline

### 4.1 测试基线变化

当前工作树 `crates/wolf-game/tests/m9_battle_round_gap.rs` 的断言：

- `m9_battle_round_gap.rs:606-609`：`main_phases == [0,1,2,0,1,0]`（2 个 command rounds），原为 4 轮。
- `:614`：`settle == 884`（原 1382，旧的模块注释写 1516）。
- `:617-619`：`command_rounds == 2`、`action_phases == 1`、`v349_at_settle == 2`。
- `:620-624`：`pools_at_settle == [4,3,3,2]`（旧 [3,2,3,2]）。
- `:625-626`：CE241 86 帧、CE243 98 帧（旧 258/294）。
- `:627-635`：CE243 first frame 641、result write frame == settle。

同样，`m9_full_route.rs:627-649` 断言 `wiring_rounds == 2`、`v350_seq == [0,1,2,0,1,0]`、`max_ce242_pc == 866`；`m7_battle_target.rs:105-112` 的坐标从 -2880/5120/3120 改为 -288/512/312。

运行结果（本机当前工作树，`WOLF_DATA_ROOT=work/all`）：

- `cargo test -q -p wolf-game --test m9_battle_round_gap` -> 2 passed, 0 failed。
- `cargo test -q -p wolf-game --test m9_full_route` -> 3 passed, 0 failed。
- `cargo test -q -p wolf-game --test m7_battle_target` -> 1 passed, 0 failed。
- `cargo test -q -p wolf-game --test m11_battle_command_sequence` -> 2 passed, 0 failed；输出：
  - schedule [1,2,3,4]：settle=1941，rounds=4，pools [3,2,3,2]；
  - schedule [1,3,5,6]：settle=3269，rounds=7，pools [3,2,1,1]（文档旧值写 9 rounds，已过时）。

`cargo test -q -p wolf-vm` 也全部通过（94+5+7+4+5+10+15 个测试，0 failed）。

### 4.2 为什么不是“误伤战斗”

关键不是“轮数变少”本身，而是旧实现少清了状态槽：

- `239_■戦闘呼び出し.json` idx6：`[2000386, 0, 0, 65536, 3]`，按 range 应清 `2000386..=2000389` 四个成员行动/回合标志。旧单槽实现只清 `2000386`，留下 `2000387/388/389` 的上一场残留。
- `241_┣ターン開始処理.json` pc346/371/502 分别检查 `2000386/2000387/2000389 == 1`；`242_┣コマンド処理.json` pc33 检查 `2000387 == 1`。这些正是 CE239 清掉的四槽块。
- `244_┗終了判定.json` idx317/375/420 同样清 `2000386..=2000389`，进一步证明这四个槽是同组状态。
- CE241 pc0 `[2000500,1,0,66048,2]` 对 `2000500..=2000502` 三槽减 1；CE239 idx1 清 `2000500..=2000502`，也是同一 range 模式的独立证据。

旧 4 轮轨迹是“单槽实现留下旧成员标志 -> 部分成员回合被跳过/错位 -> 战斗被拖长”的产物；正确清四个标志后，2 轮轨迹仍有完整阶段（`0->1->2->0->1->0`）、真实的多回合、CE241/CE243 均实际运行，最终仍在 CE244 正常结算。这不是把战斗短路成假胜利，而是修正了旧语义错误。

另外，`m11_battle_command_sequence` 的独立 schedule 仍能产生 4 轮和 7 轮，说明 range 修复没有把所有战斗都压成 2 轮。`m7_battle_target` 的 -288/512/312 也直接对应 CE175 pc72 的四个连续坐标。

因此结论：**合理 rebaseline，PASS；不是误伤。**

### 4.3 测试注释/命名的 P2 问题

`m9_battle_round_gap.rs:1-25` 模块注释仍写：

- `0 -> 1 -> 2 -> 0 FOUR times`
- `v350=[0,1,2,0,1,2,0,1,2,0,1,0]`
- `settle frame 1516`
- `2000349=4`
- `pools [3,2,3,2]`
- `CE241 258 / CE243 294`

但 `:606-626` 当前断言已是 `[0,1,2,0,1,0]`、884、2、[4,3,3,2]、86/98。注释 611-613 的“shift 134 frames earlier than the pre-M13 baseline”也与当前 884 不符（1516-134=1382，当前比旧值早 632 帧）。建议更新模块说明和该注释，否则后续维护者会按旧基线判断回归。

`m7_battle_target.rs:105-112` 注释写“count=3”，应改为“last offset=3，覆盖 4 个连续坐标”。数值本身正确。

---

## 5. P2 问题清单与最小修改建议

### P2-1 `exec.rs:1838-1839` 注释错误

- 现状：注释说 `1600022..1600024` 3 个槽。
- 实际：CE94 pc83-86 设置 `1600022/23/24/25` 四个值；pc87 的 arg4=3 按 inclusive 覆盖四个槽。
- 最小修改：改为 `1600022..=1600025`（或“四个槽：x/y/宽/高”）。

### P2-2 `exec.rs:1843` 的 range 开关不够精确

- 现状：`if argc(cmd) >= 5`。
- 原始引擎：VA `0x4af158` 检查 `arg3 & 0x10000`（bit16）。
- 当前数据：236 条 common + 49 条 map 的 argc==5 全部 bit16=true，所以没有当前行为差异。
- 最小修改：

```rust
let last_offset = if (bits & 0x10000) != 0 {
    eval_num(arg(cmd, 4), inst, vars).clamp(0, 1024)
} else {
    0
};
for offset in 0..=last_offset {
    ...
}
```

（`bits` 已经是 `arg(cmd,3) as u32`；`arg(cmd,4)` 只在 bit16 设置时读取。）

### P2-3 变量命名与注释

- `count` -> `last_offset` 或 `last`，避免未来有人把 `0..=last_offset` 又改回 `0..count`。
- 在函数内注释说明：arg4 是 inclusive last offset，槽数 = arg4+1；不是 count。

### P2-4 异常 argc==7 条目的防御（可选）

- 20 条 `intArgs.len()==7` 的 map 条目（例如 SampleMapA_1 索引 1541 附近，target=1100000000）当前会因 `argc>=5` 被当成 range。
- 建议在 op121 入口增加参数长度校验（只接受 4 或 5），或在 parser 记录并跳过这类条目；至少不要在未验证 target 上执行 range。若采用 P2-2 的 `bits & 0x10000` 检查，仍需确认这些条目 bit16 状态；不要假设全是解析伪影。

### P2-5 测试/文档 stale（不阻塞功能）

- `m9_battle_round_gap.rs:1-25` 更新为当前 2 轮数值。
- `m9_battle_round_gap.rs:611-613` 的“134 frames earlier”注释按当前基线改写；建议写明“相对于 M13 基线 1382 提前到 884”。
- `m7_battle_target.rs:105-112` “count=3”改为“last offset=3，四个槽”。
- `docs/OPCODE_SPEC.md:615-616` 的 TODO 建议补上审计结论：arg[4] = inclusive last offset，槽数 = arg4+1，bit16 是 range 标志；附 Game.exe 0x4aea70 / 0x4af158 / 0x4afd58 证据。

---

## 6. 建议补的单测

### 6.1 op121 合成单测（wolf-vm）

建议放在 `crates/wolf-vm/tests/` 或 `exec.rs` 的 `#[cfg(test)]` 模块，直接构造 Command：

1. **inclusive range 写 4 槽**：
   - `target = 2000001`（普通变量 1）、`a = 1`、`b = 0`、`arg3 = 0x10000`（低 12 位=0 表示 assign；bit12 以上=0 表示 a+b）、`arg4 = 3`。
   - 执行后断言变量 1/2/3/4 都等于 1；变量 5 保持原值不变。
2. **argc==4 只写 1 槽**：
   - `target = 2000001`、`a = 1`、`b = 0`、`arg3 = 0`；断言变量 1 被写、变量 2 不被写。
3. **arg4=0 只写 1 槽**：
   - `arg3 = 0x10000`、`arg4 = 0`；断言变量 1 被写、变量 2 不被写。
4. **负 arg4 不扩展**（可选）：
   - `arg4 = -1`；断言只写 target 自身/或不写，与当前 clamp 语义一致并记录为保护行为。
5. **复合运算 + range**（可选）：
   - `arg3 = 0x10400`（0x400=div，低 12 位）、`a = 10`、`b = 0`、`arg4 = 3`；先把 4 个槽设为 100，执行后断言 10。

### 6.2 集成/回归测试

- 保留并更新 `m9_battle_round_gap` 的 2 轮断言，但把模块注释改成当前值；建议同时保留结构断言（至少 2 个 command round、CE241/CE243 都运行、最终 2000352 有结果）而不是只锁死数值。
- `m7_battle_target` 已覆盖 CE175 四坐标；建议加一句注释引用 `ce94.txt:85-89` 的同型证据。
- 若要防止 target 白名单回归，建议新增一个“CE178/CE185 多槽绘制”的最小事件测试，确认 `1600060..1600082` 的 23 个槽都被缩放，而不是只写首槽。

### 6.3 运行命令

```bash
TEMP=$PWD/work/tmp TMP=$PWD/work/tmp WOLF_DATA_ROOT=work/all \
  cargo test -q -p wolf-game --test m9_battle_round_gap
TEMP=$PWD/work/tmp TMP=$PWD/work/tmp WOLF_DATA_ROOT=work/all \
  cargo test -q -p wolf-game --test m9_full_route
TEMP=$PWD/work/tmp TMP=$PWD/work/tmp WOLF_DATA_ROOT=work/all \
  cargo test -q -p wolf-game --test m7_battle_target
TEMP=$PWD/work/tmp TMP=$PWD/work/tmp cargo test -q -p wolf-vm
```

---

## 7. 可复现证据索引

| 证据 | 路径/位置 | 用途 |
|---|---|---|
| CE94 四槽准备 + arg4=3 除 10 | `work/out/ce94.txt:85-89` | 最直接的 inclusive last offset 证据 |
| CE94 后续 3/2/7 槽块 | `work/out/ce94.txt:90-109` | 不同 arg4 对应不同槽数 |
| CE178 2 槽/23 槽 | `work/wolftl2/dump/common/178_X[戦]味方欄_単体描画.json` idx99-125 | 2 槽 arg4=1、23 槽 arg4=22 |
| CE179 19 槽 | `work/wolftl2/dump/common/179_X[戦]敵キャラ_単体描画.json` idx84-89 | arg4=18 对应 19 槽 |
| CE185 4/12/2 槽 | `work/wolftl2/dump/common/185_X[戦]上部ステータス描画.json` idx79-99 | 多组独立块 |
| CE38 601 个普通变量 | `work/wolftl2/dump/common/38_---------------------------------.json` idx2 | arg4=600 的初始化语义 |
| CE239 四行动标志清零 | `work/wolftl2/dump/common/239_■戦闘呼び出し.json` idx6 | 战斗 rebaseline 的直接原因 |
| CE241/CE242 读取行动标志 | `work/wolftl2/dump/common/241_┣ターン開始処理.json` pc346/371/502；`242_┣コマンド処理.json` pc33 | 旧单槽残留影响分支 |
| 当前 op121 实现 | `crates/wolf-vm/src/exec.rs:1822-1856` | 审计对象 |
| Game.exe 反汇编 | `work/runcopy/Game.exe` VA `0x4aea70` / `0x4af158` / `0x4af1b0` / `0x4afd58` | arg4 inclusive 循环证明 |
| 战斗测试结果 | `crates/wolf-game/tests/m9_battle_round_gap.rs:606-626` | 2 轮断言与新基线 |

## 8. 最终判定表

| 检查项 | 判定 | 说明 |
|---|---|---|
| arg[4] 是 inclusive last offset | **PASS** | Game.exe 0x4af158 + 0x4afd58 明确 `esi <= arg4` |
| 槽数 = arg4+1 | **PASS** | CE94/178/179/185 的槽数与 arg4 全部吻合 |
| 当前 `0..=count` 边界 | **PASS** | 实现正确；仅命名/注释需改 |
| range 是否只应作用于特定 target | **FAIL（窄修思路）** | 通用 opcode，CE175/178/179/185/239/244 都需要 range；target 白名单会让这些路径继续错误 |
| 战斗 4 轮 -> 2 轮是否误伤 | **PASS（合理 rebaseline）** | CE239/CE244 四标志清零 + CE241/CE242 读取；m11 独立战斗仍 4/7 轮 |
| 当前工作树测试 | **PASS** | m9_battle_round_gap 2/2、m9_full_route 3/3、m7_battle_target 1/1、m11 2/2、wolf-vm 全通过 |
| 注释/文档一致性 | **P2（待清理）** | `exec.rs:1838-1839`、m9 模块头、m7 注释、OPCODE_SPEC TODO |

**总体：PASS。** 不需要回退当前 range 改动；建议按 §5 做 P2 清理，并按 §6 补合成单测后再交给 Lead 做全量验证。
