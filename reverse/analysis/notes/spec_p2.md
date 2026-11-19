## 4. 指令流编码（event_command）

**状态：编码骨架已验证**（work\fs_ce_params.py 走完全部 163,828 条指令，文件恰好结束在 5532174）。
**参数语义：部分已验证**（见 4.5 / 4.6）。

### 4.1 编码骨架（core）

~~~
event_command:
  u1  param_count          // 含 command_type 本身；实测恒 >= 1
  if param_count > 0:
      u4  command_type     // 即 opcode（小端）
      u1[ (param_count - 1) * 4 ]  params    // 参数区，字节长度 = (param_count-1)*4
  u1  branch_depth         // 分支嵌套深度（见 4.3，实测最大 91）
  u1  string_count
  t_str[ string_count ]  strings           // 该指令携带的字符串，全部在这里
  if param_count > 0:
      u1  have_route
      route_info  route    // 仅 have_route != 0 时存在
~~~

要点：

1. **参数区长度完全由 param_count 决定**，不需要按 opcode 分派就能正确跳过：
   长度 = (param_count - 1) * 4 字节。这和 Kaitai 的写法一致（它用 param_count*4-4）。
2. **所有字符串都在独立的字符串表里**，参数区只放数值。所以即使某个 opcode 的参数布局没解码，
   也能安全地"数值当黑盒字节、字符串照读"。
3. branch_depth / string_count / have_route 在**参数区之后**，不论 opcode 是什么都在同一位置。
4. 实测：have_route == 1 全文件只有 **5 条**，全部是 opcode 201（Move）——
   即"指令级移动路线"只被移动指令使用。其余 163,823 条 have_route == 0。

#### 4.1.1 指令尾字节（have_route 的实测语义）

参数区之后的那个 u1 是本指令的“结束标记”，实测只有两种值：

~~~
0x00 = 普通结束（163,823 条）
0x01 = 本指令带移动路线块（全文件只有 5 条，全部是 opcode 201 Move）
~~~

带路线时，0x01 之后是：

~~~
u1[5]  未知（route_info 的 animation_frequency / move_speed / move_frequency /
              move_route_mode / behavior_options 这前 5 个字节）
u1     flags（= character_move_route_options）
u4     route_count
RouteCommand[ route_count ]
~~~

RouteCommand 的实测编码（Lead 逐字节验证）：
u1 id → u1 arg_count → arg_count × u4 → 收尾 2 字节 01 00。
**注意**：Kaitai 的 route.ksy 把它写成“u1 type → u1 u4_arg_len → u4_arg_len×u4 → u1 u1_arg_len → u1_arg_len×u1”，
两者对“无参数”情形是一致的（00 01 00 = id 0、arg_count 0、收尾 01 00 里的 01 即 u1_arg_len、00 即那 1 个 u1 参数），
但对带参数的情形需要按 RouteCommand.hpp 的参数表逐 id 核对。
**TODO**：RouteCommand 各 id 的参数个数与含义尚未逐项验证（大部分可从 4.7 的 route_type 表 + arg 个数推）。

### 4.2 param_count == 0 的情况

**实测不存在**：163,828 条指令中 param_count 全 ≥ 1。
NOP（command_type == 0）的 param_count 恒为 1（参数区 0 字节），共 24,555 条。
写作 loader 时：仍建议保留 "param_count == 0 时直接跳过 command_type" 的分支以兼容其他 WOLF 版本，
但在本游戏数据里不会触发。

### 4.3 branch_depth 直方图（实测，必须用动态深度）

| depth | 条数 | depth | 条数 |
|---|---|---|---|
| 0 | 17225 | 11 | 685 |
| 1 | 39687 | 12 | 716 |
| 2 | 33567 | 13 | 594 |
| 3 | 25600 | 14 | 435 |
| 4 | 17842 | 15 | 110 |
| 5 | 10814 | 16 | 66 |
| 6 | 6369 | 17 | 55 |
| 7 | 4139 | 18 | 42 |
| 8 | 2194 | 19/20 | 26 |
| 9 | 1466 | 21/22/23 | 24/22/22 |
| 10 | 1093 | 24 | 20 |

depth 25..89 各 15 条，90 有 10 条，91 有 4 条。
→ **最大嵌套深度 91**。Rust 解释器不要用固定小数组做分支栈（用 Vec，或按深度动态分配）。

### 4.4 opcode 表（实测 48 种 + WolfTL 命名）

名称取自 docs\vendor\WolfTL-main\WolfTL\WolfRPG\Command.hpp 的 enum class CommandType。

| opcode | 名称 | 条数（含 NOP 统计口径，共 163,828） |
|---|---|---|
| 0 | Blank（NOP） | 24555 |
| 99 | Checkpoint | 152 |
| 101 | Message（显示文字） | 12027 |
| 102 | Choices（选项） | 495 |
| 103 | Comment | 9707 |
| 104 | （WolfTL 无此名；仅 1 次） | 1 |
| 105 | ForceStopMessage | 5 |
| 106 | DebugMessage | 76 |
| 107 | ClearDebugText | 1 |
| 111 | VariableCondition | 10743 |
| 112 | StringCondition | 123 |
| 121 | SetVariable | 17258 |
| 122 | SetString | 1030 |
| 123 | InputKey | 218 |
| 124 | SetVariableEx | 579 |
| 125 | AutoInput | 98 |
| 126 | BanInput | 10 |
| 130 | Teleport | 1 |
| 140 | Sound | 1368 |
| 150 | Picture | 16716 |
| 160 | SetTransition | 1 |
| 170 | StartLoop | 182 |
| 171 | BreakLoop | 114 |
| 172 | BreakEvent | 1318 |
| 174 | ReturnToTitle | 13 |
| 175 | EndGame | 2 |
| 176 | StartLoop2 | 54 |
| 179 | LoopTimes | 1836 |
| 180 | Wait | 4696 |
| 201 | Move | 5 |
| 210 | CommonEvent（调用公共事件） | 18468 |
| 211 | CommonEventReserve | 4 |
| 212 | SetLabel | 606 |
| 213 | JumpLabel | 1543 |
| 220 | SaveLoad | 8 |
| 221 | LoadGame | 16 |
| 222 | SaveGame | 5 |
| 250 | Database（数据库操作） | 2592 |
| 270 | Party | 5 |
| 281 | ScrollScreen | 3 |
| 290 | Effect | 640 |
| 300 | CommonEventByName（按名调用） | 1164 |
| 401 | ChoiceCase（选项分支） | 18555 |
| 420 | ElseCase | 3279 |
| 421 | CancelCase | 153 |
| 498 | LoopEnd | 2018 |
| 499 | BranchEnd | 11361 |
| 1000 | ProFeature | 24 |

覆盖率最高的 14 个（0/401/210/121/150/101/499/111/103/180/420/250/498/179）合计约 93.9%。

### 4.5 参数区大小表（per opcode，实测；loader 自检用）

下表是每个 opcode 实际出现过的参数区字节长度（= (param_count-1)*4）：

| opcode | 参数区大小 : 条数 |
|---|---|
| 0 | 0:24555 |
| 99 | 4:152 |
| 101 / 103 / 105 / 107 / 170 / 171 / 172 / 174 / 175 / 176 / 212 / 498 / 499 | 0（无参数，文本在字符串表） |
| 102 | 4:495 |
| 106 | 0:76 |
| 111 | 16:6736, 28:1564, 40:2254, 52:189 |
| 112 | 8:93, 12:18, 16:8, 20:4 |
| 121 | 16:17022, 20:236 |
| 122 | 8:562, 12:464, 16:4 |
| 123 | 8:216, 12:2 |
| 124 | 12:174, 16:405 |
| 125 | 4:98 |
| 126 | 4:10 |
| 130 | 20:1 |
| 140 | 16:147, 24:662, 28:559 |
| 150 | 8:203, 12:1839, 16:85, 24:952, 44:11677, 48:84, 52:55, 56:71, 60:43, 72:434, 76:1273 |
| 160 | 8:1 |
| 173 | 0（本次未出现，仅列在 opcode 表中供参考） |
| 179 | 4:1836 |
| 180 | 4:4696 |
| 201 | 4:5 |
| 210 | 8:292, 12:2647, 16:1418, 20:579, 24:4714, 28:8666, 36:152 |
| 211 | 8:4 |
| 213 | 0:1543 |
| 220 | 8:8 |
| 221 | 16:16 |
| 222 | 16:5 |
| 250 | 16:16, 20:2576 |
| 270 | 4:1, 8:3, 12:1 |
| 281 | 12:3 |
| 290 | 28:640 |
| 300 | 8:30, 12:304, 16:341, 20:321, 24:156, 28:8, 32:4 |
| 401 | 4:18555 |
| 420 | 4:3279 |
| 421 | 4:153 |
| 1000 | 4:24 |

### 4.6 已解码的参数布局（逐字节抽查过样本）

#### 4.6.1 通用 param_status 位域

opcode 210 / 211 / 300（以及任何"带可变个参数"的指令）使用同一个 u4 位域：

~~~
u4 param_status:
  bits 0..3   number_count          = raw & 0x0F
  bits 4..7   string_count          = (raw >> 4) & 0x0F
  bits 12..16 string_1_enabled .. string_5_enabled（WolfTL 的 string_1..5_enabled 位）
  bit  24     return_value_enabled
  count = number_count + string_count
~~~

#### 4.6.2 opcode 210 CommonEvent / 211 CommonEventReserve

~~~
u4 event_id
u4 param_status
s4[ param_status.count ]  params
if param_status.return_value_enabled: u4 return_variable_id
~~~

**实测纠正**：Kaitai 的 event_command.ksy 写着"若 500000 <= event_id <= 599999 才跟着 param_status"，
本文件里 **不成立**——event_id 实测范围 500014..1600048（>599999 的也照样跟 param_status），
一律都有 param_status。此规则复现了 18,468/18,468 条 opcode-210 与 4/4 条 opcode-211 的参数区大小。
高频 event_id：500264（7056 次）、500219（1265）、500218（1136）、500443（1106）、500228（958）、500217（568）。

样本：a4a10700 00000000（event_id 500132，status 0 ⇒ 无参数）；
2ea10700 01000000 ffffffff（event_id 500014，status 1 ⇒ count=1，一个参数 -1）。

#### 4.6.3 opcode 300 CommonEventByName

~~~
u4 na（实测恒 0）
u4 param_status（同 4.6.1）
s4[ count ]
if return_value_enabled: u4 return_variable_id
~~~

样本：00000000 02000001 006a1800 016a1800 016a1800（count = 2+0 = 2？注意 raw = 0x01000002 ⇒
低4位 = 2、bit24 = 1 ⇒ count=2 个 s4 + 1 个返回变量，与 32 字节的样本长度一致）。

#### 4.6.4 opcode 111 VariableCondition

~~~
u1 packed: 低 4 位 = condition_count；高 4 位 = use_else_branch
u1[3] 固定 00 00 00
condition_count × { u4 variable_id, s4 value, u4 operator }
~~~

operator 枚举：0 > / 1 >= / 2 == / 3 <= / 4 < / 5 != / 6 位与。
→ 参数区大小 = 4 + 12 × count，实测 count ∈ {1,2,3,4}（大小 16/28/40/52）。
实测最大 condition_count = 4。
样本：01000000 026a1800 01000000 01000000（1 个条件：变量 0x00186A02 与值 1，operator 1）；
三条件样本 03 000000 | (026a1800,1,2) | (026a1800,2,2) | (026a1800,3,2)。

#### 4.6.5 简单指令（参数区 0 字节）

101 Message、103 Comment、105 ForceStopMessage、106 DebugMessage、107 ClearDebugText、
170 StartLoop、171 BreakLoop、172 BreakEvent、174 ReturnToTitle、175 EndGame、176 StartLoop2、
212 SetLabel、213 JumpLabel、498 LoopEnd、499 BranchEnd。
文本 / 标签名全在字符串表（string_count ≥ 1）。实测 213 JumpLabel 与其对应 212 SetLabel 的字符串相同
（例如 移動時回復 / 戦闘時回復）。

#### 4.6.6 其他实测样本（布局未完全解码，给出原始字节）

| opcode | 样本参数区（hex） | 备注 |
|---|---|---|
| 121 SetVariable | 016a1800 016a1800 00000000 00080000 | 第 1 个 u4 = 目标变量 id（未完全验证） |
| 122 SetString | 096a1800 00000000 + 1 字符串 | |
| 140 Sound | 00000002 0a000000 00000000 00000000 46000000 64000000 00000000 + 1 字符串 | 字符串如 BGM/nekomimi.mp3 |
| 150 Picture | 03000001 156a1800 166a1800 ... | 主形 44 字节；参数含义未解 |
| 179 LoopTimes | 136a1800 | |
| 180 Wait | 0a000000 / 01000000 | 等待帧数或变量 |
| 201 Move | feffffff / ffffffff + route | 全文件仅 5 条，且这 5 条都带 route |
| 250 Database | 01000000 006a1800 0f6a1800 00000000 016a1800 | 大小 16/20 |
| 270 Party | 00000000 02000000 | |
| 290 Effect | 28 字节 | |
| 401 ChoiceCase | 01000000 | 分支 id |
| 420 ElseCase | 00000000 | |
| 421 CancelCase | 00000000 | |
| 999 / 1000 ProFeature | 03000000 + 1 字符串（如 Save/save\cself[12].） | |

**未解参数语义**（大小已知、字段含义未验证）：112、121、123、124、140、150、160、221、222、
250、290、1000。→ loader 策略：这些指令的参数区按原始字节保留，由解释器在实现该指令时再逐个解码；
不要为了"解析格式"而硬编码猜测。

### 4.7 路线块（route_info，用于 201 Move 与 .mps 事件页）

**状态：结构已验证（对 .mps 事件页逐字节走通）、字段语义已验证。**

~~~
route_info:
  u1 animation_frequency        // 枚举 频率 0..5（0 每帧 / 1 极短 / 2 短 / 3 中 / 4 长 / 5 极长）
  u1 move_speed                 // 枚举 0..6
  u1 move_frequency             // 同上频率枚举
  u1 move_route_mode            // 0 固定不动 / 1 自定义 / 2 随机 / 3 靠近主角
  u1 behavior_options           // 位域，LSB 起：半格左/半格上/方形碰撞盒/在主角之上/
                                //   可穿过/固定朝向/移动动画/待机动画
  u1 character_move_route_options  // LSB 起：na(低5位)、wait_until_done、skip_impossible_moves、repeat_actions
  u4 route_length
  route_data[ route_length ]
~~~

**route_data 的通用解析（重要简化）**——所有路线类型都可用同一段代码：

~~~
route_data:
  u1 type
  u1 u4_arg_len ;  skip u4_arg_len * 4 bytes
  u1 u1_arg_len ;  skip u1_arg_len bytes
~~~

因为"无参数"的类型其字节就是 00 01 00（u4_arg_len=0、u1_arg_len=1、1 个 u1 参数），
而带参数的命名类型（jump / assign_to_variable / add_to_variable / set_move_speed /
set_move_frequency / set_animation_frequency / wait_x_frames / move_approach_event /
move_approach_position / set_graphic / set_opacity / set_sound / set_height）都符合
"先 u4_arg_len、后 u1_arg_len"的同一形状。
→ Rust 只需一条 route_data 解析路径，再按 type 给参数取名。

route_data.type 枚举（_wolfref\route.ksy，0x00..0x3A）：

| 值 | 含义 | 值 | 含义 |
|---|---|---|---|
| 0x00 | 向下移动 | 0x1D | 设置移动速度 |
| 0x01 | 向左移动 | 0x1E | 设置移动频率 |
| 0x02 | 向右移动 | 0x1F | 设置动画频率 |
| 0x03 | 向上移动 | 0x20 | 待机动画开 |
| 0x04 | 左下移动 | 0x21 | 待机动画关 |
| 0x05 | 右下移动 | 0x22 | 移动动画开 |
| 0x06 | 左上移动 | 0x23 | 移动动画关 |
| 0x07 | 右上移动 | 0x24 | 固定朝向开 |
| 0x08 | 面向下 | 0x25 | 固定朝向关 |
| 0x09 | 面向左 | 0x26 | 可穿过开 |
| 0x0A | 面向右 | 0x27 | 可穿过关 |
| 0x0B | 面向上 | 0x28 | 总在最上层开 |
| 0x0C | 面向左下 | 0x29 | 总在最上层关 |
| 0x0D | 面向右下 | 0x2C | 设置图形 |
| 0x0E | 面向左上 | 0x2D | 设置不透明度 |
| 0x0F | 面向右上 | 0x2E | 设置声音 |
| 0x10 | 随机移动 | 0x2F | 等待 N 帧 |
| 0x11 | 朝主角移动 | 0x30 | 半格移动 |
| 0x12 | 远离主角移动 | 0x31 | 整格移动 |
| 0x13 | 向前一步 | 0x32 | 图案 1 |
| 0x14 | 退后一步 | 0x33 | 图案 2 |
| 0x15 | 跳跃 | 0x34 | 图案 3 |
| 0x16 | 右转 | 0x35 | 靠近事件 |
| 0x17 | 左转 | 0x36 | 靠近坐标 |
| 0x18 | 左右随机转 | 0x37 | 加到变量 |
| 0x19 | 随机朝向 | 0x38 | 图案 4 |
| 0x1A | 朝向主角 | 0x39 | 图案 5 |
| 0x1B | 背向主角 | 0x3A | 设置高度 |
| 0x1C | 赋值到变量 | | |

（0x2A / 0x2B 在 ksy 表中缺失，实现时遇到就报错。）

---

## 5. .mps 地图文件

**状态：已验证**（work\fs_mps.py；9/9 文件 trailing 0、末字节 0x66、事件数与 event_count 匹配）。

### 5.1 地图清单（实测）

| 文件 | 字节 | 宽 × 高 | 事件数 | 有图块层 |
|---|---|---|---|---|
| TitleMap.mps | 24145 | 53 × 30 | 1 | 是 |
| SampleMapA.mps | 797933 | 64 × 36 | 40 | 是 |
| SampleMapA_1.mps | 176577 | 64 × 36 | 12 | 是 |
| SampleMapB.mps | 13476 | 30 × 30 | 6 | 是 |
| Map002.mps | 48683 | 100 × 30 | 5 | 是 |
| Map003.mps | 144891 | 70 × 170 | 2 | 是 |
| Map024.mps | 237 | 53 × 30 | 1 | **否**（无图块层） |
| Map031.mps | 1545 | 53 × 30 | 1 | **否**（无图块层） |
| Dungeon.mps | 17719 | 40 × 30 | 5 | 是 |

游戏实际注册 6 张地图（SysDatabase 的 マップ設定，见 6.4）。
**9 张地图合计 73 个事件 / 134 页 / 37,610 条指令槽（30,558 条真实指令）/ 40 个 opcode**（详见 5.6.2）。
**地图事件总数仅 67 个——游戏几乎全靠 550 个公共事件驱动。**

### 5.2 头部布局（已验证）

| 偏移 | 类型 | 字段 | 实测 |
|---|---|---|---|
| 0 | 10 字节 | 全 0x00 | |
| 10 | 6 字节 | "WOLFM\0" | |
| 16 | u1 | version_header | 0x55 |
| 17 | 3 字节 | 全 0x00 | |
| 20 | u4 | unknown1 | 恒为 **0x64**（=100） |
| 24 | u1 | version | **0x66** |
| 25 | t_str | title | 地图名 |
| 36 | u4 | tileset | TileSetData 里的 tileset 序号（0 起） |
| 40 | u4 | width | 宽（图块数） |
| 44 | u4 | height | 高（图块数） |
| 48 | u4 | event_count | 事件数 |
| 52 | 图块数据块 | | 见 5.3 |
| 52 + 图块块大小 | event × event_count | | 见 5.4 |
| 末尾 | u1 | footer | 恒为 **0x66** |

（v3.5 及以上会在 title 之后多出 unknown4 u4 与 layer_count u4，本游戏 version == 0x66 恒为 3 层。）

### 5.3 图块数据块与 -1 哨兵（**最容易踩的坑**）

~~~
offset 52:  u4 first_pixel
if first_pixel == 0xFFFFFFFF:
    // 该地图没有图块层，块就是这 4 个字节，事件紧随其后
else:
    // 共 width * height * 3 * 4 字节（3 层 × w × h × u4），已含刚读的 4 字节
~~~

实测证据：Map024.mps 与 Map031.mps 在 offset 52 是 ff ff ff ff，紧接着 offset 56 就是
事件头 6f 39 30 ...（0x6F + u4 0x3039）；其余 7 张地图读满 w*h*3*4 字节后事件头正好落在预期位置。

图块层顺序：layer0（地表）→ layer1（装饰）→ layer2（顶层），每层 w*h 个 u4，行优先。

### 5.4 单个 u4 图块值的解码

~~~
raw = u4
autotile_id = raw / 100000
if autotile_id > 0:            // 自动图块（auto tile）
    4 位小数分别代表四个角的模式：
      top_left     = (raw % 10000) / 1000
      top_right    = (raw %  1000) /  100
      bottom_left  = (raw %   100) /   10
      bottom_right = raw % 10
else:                          // 普通图块
    base_tile = raw            // 直接是图块编号
~~~

实测 SampleMapA 的一层前 6 个值全为 104444（autotile_id=1，四角模式 0/4/4/4）。
0xFFFFFFFF 作为填充值表示"无图块"（仅高层/空位出现）。

### 5.5 事件结构（已验证）

~~~
event:
  u1  header            // 恒 0x6F
  u4  header2           // 恒 0x3039 (=12345)
  u4  event_id
  t_str title
  u4  map_x
  u4  map_y
  u4  page_count
  u4  separator         // 恒 0
  page[ page_count ]
  u1  footer            // 恒 0x70
~~~

实测：SampleMapA 的 event0 名为 "ニューゲーム処理"（30,0），1 页，footer 0x70；
event1 名为 "妹クリック判定（夜"（63,0），3 页。
**注意事件名是"被截断保存"的**（这个 event1 的名字缺右括号），不是解析错误——
WOLF 编辑器按固定字节容量存事件名。做 loader 时不要试图"修复"。

### 5.6 事件页结构（已验证）

~~~
page:
  u1  header                     // 恒 0x79
  u4  graphic_chipid             // 实测 0xFFFFFFFF = 无（用下面的文件名）
  t_str graphic_filename
  u1  row_in_tileset
  u1  col_in_tileset
  u1  opacity
  u1  display_type               // 0 正常 / 1 乘算 / 2 加算 / 3 减算
  u1  trigger_condition          // 0 确认键 / 1 自动(autorun) / 2 并行 / 3 主角接触 / 4 事件接触
  u1[4] trigger_switch           // 每字节：高 4 位 = operator，低 4 位 = enabled
  u4[4] trigger_variable
  s4[4] trigger_value
  route_info route
  u4  command_count
  event_command[ command_count ]
  u4  unknown3                   // 恒 3
  u1  shadow_graphic_id
  u1  range_extension_x          // 上限 250
  u1  range_extension_y
  u1  footer                     // 恒 0x7A
~~~

**实测纠正（trigger_switch）**：Kaitai 的 mps.ksy 把 trigger_switch 的 enabled 位限制为 0/1，
但真实文件里该字节常见 0x20（operator 2、enabled 0）、0x21、0x11、0x31。
→ 应当**保留原始 u1**，取低 4 位当 enabled、高 4 位当 operator，不要断言取值范围。

实测 SampleMapA event1 的 page0：graphic_chipid = 0xFFFFFFFF、graphic_filename = ""、
row 2、col 1、opacity 255、display_type 0、trigger_condition 2（并行处理）、
trigger_switch = [0x33,0x33,0x33,0x33]、trigger_variable = [3,3,3,0]、trigger_value = [3,2]（第 5 个字段为 route）、
route = animation_frequency 3 / move_speed 3 / move_frequency 3 / move_route_mode 0 /
behavior_options 3 / route_options 2 / route_length 0、
command_count 113、unknown3 3、shadow 0、range 0/0、footer 0x7A。

### 5.6.1 页布局的完整字节图（Lead 逐字节验证，9/9 张地图全部解析成功）

~~~
u1   0x79                     页开始
u4   chipid                   // 常 0xFFFFFFFF = 使用下面的图形文件名
t_str graphicName
u1[4] direction, frame, opacity, renderMode
u1[37] conditions             // 1 trigger + 4 (switch+operator) + 4 triggerVar(u4) + 4 triggerVal(s4)
u1[4] movement                // animation_frequency, move_speed, move_frequency, move_route_mode
u1   flags                    // behavior_options
u1   routeFlags               // character_move_route_options
u4   routeCount ; RouteCommand[ routeCount ]
u4   commandCount ; event_command[ commandCount ]
u4   features                 // Kaitai 叫 unknown3，常态 = 3
u1[3] shadowGraphicNum, collisionWidth, collisionHeight
if features > 3: u1 pageTransfer
u1   0x7A                     页结束（容错见下）
~~~

- 这与 Kaitai 的字段表等价：Kaitai 的 row_in_tileset / col_in_tileset 就是这里的 direction / frame；
  37 字节 conditions 完全覆盖 Kaitai 的 trigger_condition + trigger_switch[4] + trigger_variable[4] + trigger_value[4]。
- **容错（必须实现）**：有 2 张地图（Dungeon 偏移 17716、Map024 偏移 234）的页结束字节是 **0x00** 而非 0x7A，
  其后紧跟 0x70（事件结束）。实现时应接受“0x00 之后接 0x70 或 0x79”作为合法页结束。
- commandCount 之后读 features；features > 3 时多一个 pageTransfer 字节（常规地图 features == 3，无此字节）。

### 5.6.2 9 张地图的最终统计（Lead 逐字节）

| 地图 | 事件 | 页 | 指令槽 |
|---|---|---|---|
| Dungeon.mps | 5 | 6 | 79 |
| Map002.mps | 5 | 7 | 416 |
| Map003.mps | 2 | 2 | 45 |
| Map024.mps | 1 | 1 | 3 |
| Map031.mps | 1 | 1 | 63 |
| SampleMapA.mps | 40 | 93 | 31083 |
| SampleMapA_1.mps | 12 | 17 | 5752 |
| SampleMapB.mps | 6 | 6 | 36 |
| TitleMap.mps | 1 | 1 | 133 |
| **合计** | **73** | **134** | **37,610**（其中 7,052 空 ⇒ 30,558 真实指令） |

地图侧用到 40 个 opcode，是公共事件 48 个的**严格子集**，没有新 opcode。
（注意：本文 5.1 表里的“事件数”列与上表一致；WolfTL 本身解析失败的 Dungeon 与 Map024 也全部解析成功。）

### 5.7 地图事件的指令流

事件页的 event_command 与第 4 章**完全同一编码**（同一套 param_count/param_status/route 规则），
因此 Rust 侧只需要一个 event_command 解析器，公共事件与地图事件共用。
实测差异：地图事件指令里会用到 opcode 240 Chip / 241 ChipSet / 242（芯片操作），
它们在本游戏的公共事件里没出现过。Kaitai 的 event_command.ksy 给出如下布局（**未在本游戏数据中实测**）：

~~~
240 Chip:     s4 chipset_id ; u4 bitfield ; u2 na
              bitfield 位（LSB 起）：counter, square, na, star, not_up, not_right, not_left, not_down,
                                      x(6 位), downarrow, triangle
              chipset_id == -1000000 表示 reset
241 ChipSet:  u4 chipset_id
242:          u4 map_layer, map_x, map_y, width, height, chip_id
~~~
