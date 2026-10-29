# WOLF RPG Editor 3.5 事件字节码语义交叉验证报告

## 研究结论与源码入口

针对你们正在做的 **WOLF RPG Editor 3.5 系 Steam 正式版游戏 Rust 重写**，这次我把证据按可靠性分成三层：**官方 WOLF RPG Editor 手册/更新日志 > 独立解析器 `wolfrpg-map-parser` > `rpg-maker-clone` / WolfTL 的解释器实现**。这个排序很重要，因为 `rpg-maker-clone` 自己的 ADR 明确说明：它没有像 RPG2000/XP 那样拿真正的 `Game.exe` 做 Wine 差分测试；其 WOLF 语义来自 WolfTL、wolftrans、`wolfrpg-map-parser` 三个二手实现，再与官方手册交叉核对。因此它非常适合做你们的“独立交叉验证”，但不应被当成 Game.exe 的黑盒真值。citeturn678780search0

你之前问到但报告里漏掉的仓库地址是：

```text
https://github.com/take-cheeze/rpg-maker-clone
```

事件解释器当前位于：

```text
mruby-wolf/mrblib/interpreter.rb
```

2026 年 9 月那份 ADR 是：

```text
https://github.com/take-cheeze/rpg-maker-clone/blob/master/docs/adr/0065-wolf-rpg-editor-event-interpreter.md
```

ADR 0065 发布于 2026 年 9 月 6 日，并明确记录了 WOLF 事件解释器的来源、限制以及 Common Event 调度模型。citeturn678780search0

WolfTL 仓库：

```text
https://github.com/Sinflower/WolfTL
```

WolfTL 的 opcode 命令表直接位于 README 的 **WolfRPG Commands** 段：

```text
https://github.com/Sinflower/WolfTL/blob/main/README.md#wolfrpg-commands
```

其中明确列出 `101 Message`、`150 Picture`、`210 CommonEvent`、`250 Database`、`290 Effect`、`300 CommonEventByName` 等事件代码；它适合拿来确认“opcode 对应哪类命令”，但它主要服务于翻译文本提取，因此对非文本参数的解析远不如 `wolfrpg-map-parser` 完整。citeturn531988view1

另一个非常有价值的独立实现是：

```text
https://github.com/G1org1owo/wolfrpg-map-parser
```

当前 crates.io/docs.rs 对应版本为 0.6.0，其结构化解析已经覆盖 `PictureCommand`、`EffectCommand`、`DBManagementCommand`、`CommonEventCommand` 等类型。citeturn633367search1turn781250search0

官方 3.50 手册入口为：

```text
https://smokingwolf.github.io/tool_wolf_rpg_editor/help/
```

官方手册更新记录明确写明 **2025-01-10 对应本体 Ver3.50**，所以对于你们的 3.5 系数据，这组文档是目前最直接的权威参考。citeturn678780search3

下面逐项回答你提出的七个问题。由于 GitHub 源码受版权限制，我不会整段复制第三方实现，而是给出**逐字段等价转写、函数名和精确行号**；这样对实现 Rust 解释器反而更有用。

## ピクチャ opcode 150：参数、范围消去、处理时间、字符串变量与 Anchor

### 已确认的 options 位域

`rpg-maker-clone` 的 `Wolf::Interpreter::Run#exec_picture` 对 `arg(0)` 的解析，与 `wolfrpg-map-parser` 的独立枚举基本一致：

```text
bits  0..3   operation
bits  4..6   display type
bits  8..11  blend
bits 12..15  anchor
bits 20..23  zoom mode
bit      24  range
bit      26  free transform
```

其中 operation：

```text
0 = 表示 Show
1 = 移動 Move
2 = 消去 Erase
3 = ディレイリセット Delay Reset
```

display type：

```text
0 = 文件名直接指定
1 = 文件名由字符串变量指定
2 = 文字列ピクチャ
3 = お手軽ウィンドウ，文件名直接指定
4 = お手軽ウィンドウ，文件名由字符串变量指定
```

blend：

```text
0 = Normal
1 = Add
2 = Subtract
3 = Multiply
15 = Same as current
```

`rpg-maker-clone` 直接在源码注释中记录了这些字段，并说明是用 `wolfrpg-map-parser` 与 WolfTL 再对官方 Picture 手册交叉确认得到的。citeturn792948view1

官方 Picture 手册独立确认了四种操作；也确认了“图片编号旁的 checkbox 可以同时操作一段连续编号”，因此 **range 位确实代表连续 picture-number 范围，而不是别的模式**。citeturn114445view4

### 消去确实存在“范围消去”

这一点可以相当有把握地确认。

`wolfrpg-map-parser` 的 Picture erase parser 不是只有一个 `Base` 状态，而是明确有：

```text
erase::base
erase::delay
erase::range
erase::delay_reset
```

而 `Erase::parse_range()` 是一个单独的解析路径。citeturn177420view2

官方 Picture 手册又明确说，勾选编号旁 checkbox 后可以对**连续多个 Picture 编号**同时操作，而这个说明位于“Picture 编号”公共部分，并不限于 Show。citeturn114445view4

所以你看到的：

```text
[head, pic, fade, fade, 0, pic2]
```

结合 `bit24 = range`，**你怀疑它是在消除 `pic..pic2` 是正确方向**。不过有一个细节值得你们重新验证：parser 内部通常把这个字段叫 `range_count` 或第二边界，不同 Picture 子型的实际编码未必都用“绝对终点 ID”。Show range parser 中公开的是 `range_count()`，而不是 `last_picture()`。citeturn197591view0turn197591view1

因此我建议 Rust IR 不要一开始硬编码：

```rust
pic_start..=pic2
```

而先保留原始字段：

```rust
PictureRange {
    first: ValueRef,
    raw_range_arg: ValueRef,
}
```

然后用你们全库样本验证 `pic2` 到底是“终点编号”还是“数量”。**“存在范围模式”已经可以确认；“第 6 个参数一定是绝对闭区间终点”目前开源证据还没有达到同样的确定度。**

另外要注意，当前 `rpg-maker-clone` 的 `exec_picture` 并没有真正实现 range Picture；它在注释中明确说只实现普通 Base layout。也就是说，不能拿它当前的 erase 行为来否定你看到的 range erase。citeturn792948view1

### Move 尾部帧数是插值处理时间，不是“执行后等待”

官方定义非常清楚：

> 「処理時間」是花多少帧完成显示、移动、消去；数值越大，移动过程越慢。

而 `発動ディレイ` 才是“等多少帧以后开始执行”。citeturn114445view4

因此你们解释器应该区分：

```text
process_time = 动画插值持续帧数
delay        = 命令真正开始之前的延迟
```

不能把 `process_time` 当作 command-level wait。

这也解释了 `rpg-maker-clone` 当前实现为何标记为不完整：它目前 Show/Move 会直接 snap 到最终状态，源码注释明确写着没有实现 WOLF 的 gradual `process_time` fade/slide animation。citeturn792948view1

事件线程是否等待动画完成是**另一层问题**。Picture 的处理时间本身不等于事件解释器 `Wait`；图片状态在渲染层按帧推进。

### type 1 / type 4 的字符串变量位置

对它目前已确认的 Base layout，`rpg-maker-clone` 记录：

```text
普通 literal 形态：11 个数字参数
字符串变量文件名形态：12 个数字参数
```

type 1 与 type 4 多出来的那个**尾部参数就是字符串变量编号**；当前实现使用 `cmd.arg(11)` 取该字符串变量。citeturn792948view0turn792948view1

所以普通 Base 形态可以写成：

```text
arg0   options
arg1   picture id
...
arg10  angle
arg11  filename string-variable id   // 仅 type 1 / 4
```

不过要强调：这是 **Base layout**。range/free-transform/Colors/separate zoom/same-current 等模式会改变参数数目；`rpg-maker-clone` 自己明确因此拒绝猜测这些布局。citeturn792948view0turn792948view1

### Anchor：你确认的 0 和 1 正确，但字段其实是 bit12..15

`wolfrpg-map-parser` 的独立枚举给出：

```text
packed value
0 = TopLeft
1 = Center
2 = BottomLeft
3 = TopRight
4 = BottomRight
5+ = Unknown
```

citeturn694803view2

所以你说的：

```text
0 = 左上
1 = 居中
```

正确。

但官方编辑器从 3.330 起实际上可以选择 **七种** anchor：

```text
左上
左下
中心
中央上
中央下
右上
右下
```

官方 Picture 手册明确列了七种。citeturn114445view4

官方“变量操作+”在读取 Picture anchor 时，又把它转成另一套对用户公开的十进制表示：

```text
横：0 左 / 10 中央 / 20 右
纵：0 上 / 1 中 / 2 下

例如：
0  = 左上
11 = 中心
20 = 右上
22 = 右下
```

citeturn678780search2

因此这里需要区分两层：

```text
字节码 packed anchor enum ≠ “变量操作+”返回的公开 anchor 数值
```

目前 `wolfrpg-map-parser` 只建模了五种 packed enum；`rpg-maker-clone` 自己也明确承认官方还有 TopCenter / BottomCenter，但它没有足够独立证据给这两种 packed 值编号，所以没有猜。citeturn792948view1

**结论：不要自行假定 5=中央上、6=中央下。** 你们已有全库字节码普查的话，最稳妥的方法是做两个最小 WOLF 3.50 工程，分别只生成中央上/中央下 Picture，然后 diff `arg0 >> 12 & 0xF`。这一项官方只定义 UI 语义，没有公开二进制编号。

## 文章の表示 opcode 101：Game.exe 负责什么，Basic System 负责什么

这一部分很容易把两层东西混在一起。

### 101 本身是引擎文字系统，不是完整 RPG 对话框组件

`rpg-maker-clone` 的 ADR 特别强调：WOLF RPG Editor **没有类似 RPG Maker 那种内建菜单/标题/存档/战斗系统**；看起来像“引擎 UI”的大量内容其实来自随编辑器提供的 RPG Basic System Common Events，其中就包括 message window。citeturn678780search0

因此建议你们把显示层分为：

```text
Game.exe / opcode 101:
    文字排版
    字符逐字显示
    特殊控制符处理
    等待输入
    pause cursor
    message-active 状态

RPG Basic System Common Events:
    对话框背景图
    面板 Picture
    头像
    窗口开关动画
    可能的 SE
    高层消息流程
```

`rpg-maker-clone` 也正因为这一点，把 Picture(150) 称为 RPG Basic System 绘制 message window、choice、menu 等可视组件的核心命令。citeturn792948view0turn557206search2

### Sys1 / Sys2 是文字原点，不是窗口图片左上角

官方系统变量说明中，Sys1 / Sys2 虽然名字带“メッセージウィンドウ”，其实际语义说明是 **“文章の表示”中文字开始绘制的位置**，坐标基准是文字的左上角。citeturn459314search0

因此：

```text
Sys1 = message text origin X
Sys2 = message text origin Y
```

而不是：

```text
Sys1 = Basic System window picture left X
Sys2 = Basic System window picture top Y
```

后者通常由 Basic System Common Event 自己计算。

### Sys8 / Sys9 / Sys10

官方系统变量定义确认：

```text
Sys8  = 文章字号
Sys9  = 每秒显示多少字符
Sys10 = 开始显示第一字符之前等待的帧数
```

citeturn459314search0

因此你提出的解释基本正确，但 Sys10 应理解成 **文字开始前的前置等待**，不是窗口关闭等待。

另外官方 Game Settings 还定义了：

```text
文章表示（文字列Picture）微调
- 横方向字詰め
- 改行の間隔
```

也就是说行距是项目级文字渲染设置的一部分。citeturn525476search1

### Pause cursor 的坐标

官方定义 Sys5 / Sys6 是**相对于 Sys1 / Sys2 的 pause cursor 偏移**。因此逻辑位置确实是：

```text
cursor_reference_x = Sys1 + Sys5
cursor_reference_y = Sys2 + Sys6
```

但这个坐标的 anchor 不是图片左上角，而是 pause cursor 的规定基准点；官方说明使用的是 cursor 上边缘的中央基准。citeturn459314search0

因此你们 Rust renderer 若直接写：

```rust
cursor.top_left = (sys1 + sys5, sys2 + sys6);
```

会产生半个 cursor 宽度的水平偏差。

### `Text_Pause.png` 不是 3 帧，而是官方默认 6 分割

官方素材说明对等待输入 cursor 的默认设置明确写了**纵向六分割**；默认系统素材就是 `SystemFile/Text_Pause.png`。citeturn459314search1

所以如果你们游戏实际文件真是：

```text
17 × 90
```

那么按照官方默认规则：

```text
90 / 6 = 15 px
```

应理解为六个 `17×15` frame，而不是三个 `17×30` frame。

除非该 Steam 游戏自己修改了 System DB 中的分割数；所以最终还应以该项目的系统数据库为准。

### “关闭动画 6 帧”不是 opcode 101 的固定引擎语义

没有找到官方证据说 **文章の表示 opcode 101 固定有 6 帧关闭动画**。相反，现有证据更支持“窗口图形和窗口动画属于 Basic System Common Event”。

因此如果你们全库观察到“消息结束后窗口 6 帧收起”，应该追它调用的 `X[共]メッセージウィンドウ` 等 Common Event，而不是写死在：

```rust
exec_opcode_101()
```

中。ADR 0065 甚至提到 sample Basic System 的 `X[共]メッセージウィンドウ` 自己包含循环逻辑，并因为 `GotoLoopStart` 未实现曾导致解释器卡死；这正说明它属于事件层。citeturn678780search0

### 换行“宽度”问题

官方 Game Settings 公开的是：

```text
横方向字詰め
改行の間隔
```

而不是一个“message box width 自动折行阈值”。citeturn525476search1

WOLF 的普通消息文本主要依靠事件文本中的换行和控制字符排版；我没有找到 3.50 官方手册给出一个类似 `SysXX = automatic wrapping width` 的系统变量。因此在没有你们实际黑盒测试之前，不建议实现“碰到窗口右边缘自动 wrap”。

也就是说，目前证据支持：

```text
明确的 \n / 编辑器生成换行 → 换行
line spacing → 项目设置/控制符
```

但不支持“Sys1 + 某个固定 width 自动决定折行”。

## DB操作 opcode 250：flags 位域应如何修正

这一项你们的反推已经非常接近，但 **operator 6/7 顺序与你写的正好相反，而且还有 opcode 8**。

官方 DB 操作手册从语义层确认了：

```text
可変DB
システムDB
ユーザDB

DB → 变量
变量 → DB
```

以及 DB 值支持赋值/演算等操作；但官方没有公开它在二进制中的 bit layout。换句话说，**位域本身必须依赖开源解析器和样本交叉验证，而不能称为官方 bitfield 定义**。citeturn927030search2

`rpg-maker-clone` 当前对 3.x 实际数据的等价解析为：

```rust
packed = arg3;

assignment_byte = packed & 0xff;
options_byte    = (packed >> 8) & 0xff;

db_type   = options_byte & 0x0f;
direction = (options_byte >> 4) & 0x0f;

use_var_reference = assignment_byte & 1;
assign_op          = (assignment_byte >> 4) & 0x0f;
```

其源码位于 `interpreter.rb` 的 `exec_database`。citeturn298566view0turn298566view1

所以对于你当前只关心常见模式，可以简化成：

```text
container = (flags >> 8) & 0xF

0 = 可変DB
1 = システムDB
2 = ユーザDB
```

这一点与你的：

```text
(flags >> 8) & 3
```

在 0..2 的正常数据上结果一样，但建议仍然 mask `0xF`，因为实际字段定义是 nibble，不是两 bit。citeturn298566view1

direction：

```text
((flags >> 12) & 0xF)

0 = Write：变量/值 → DB
1 = Read ：DB → 变量
```

所以你写的：

```text
flags & 0x1000
```

对判断 read/write 确实有效。citeturn298566view1

### operator 的正确表

`assign_op = (flags >> 4) & 0xF`

当前已交叉确认：

```text
0 = =
1 = +=
2 = -=
3 = *=
4 = /=
5 = %=
6 = min
7 = max
8 = abs
```

citeturn266769view0

因此你原来的：

```text
6 = max
7 = min
```

需要反过来：

```text
6 = min
7 = max
```

而且不能只 mask `&7`，因为 `8 = abs` 是有效 operator。`rpg-maker-clone` 明确说这套 0–8 编号同时与 SetVariable/SetVariableEx 的独立 enum 对上了。citeturn266769view0

### arg0 / arg1 / arg2

当前实现与真实样本一致：

```text
arg0 = DB type id
arg1 = data/row id
arg2 = field/column id
arg3 = flags/options
arg4 = value or destination variable（numeric/base 形态）
```

citeturn298566view0turn298566view1

所以你说：

```text
arg0 = 运行时 type id
arg2 = 列
```

是对的；但别漏掉：

```text
arg1 = data id / row id
```

而且这三者都是 ValueRef-capable，不一定是 literal。`rpg-maker-clone` 的真实样本里就出现了用变量来指定 field 的情况。citeturn298566view0

另一个很重要的 3.x 细节是：string DB 写入存在 **4 参数 + command string** 的形态，不总是 `arg4`；数值或变量型一般是五参数。citeturn298566view1

建议你们 Rust：

```rust
enum DbPayload {
    ValueRef(u32),
    LiteralString(String),
}
```

而不要把 `arg4` 设成硬必需。

## 効果 opcode 290：11 种 Picture effect 与参数布局

这部分目前的交叉证据很漂亮。

`wolfrpg-map-parser` 明确给出了 0..10 的完整 PictureEffectType：

```text
0  Flash
1  ColorCorrect
2  DrawPositionShift
3  Shake
4  Zoom
5  SwitchFlicker
6  SwitchAutoFlash
7  AutoEnlarge
8  AutoPatternSwitchOne
9  AutoPatternSwitchLoop
10 AutoPatternSwitchRoundTrip
```

citeturn694803view0turn350757view2

这正好回答了你最关心的：

```text
3 = シェイク
7 = 自動拡大縮小
8 = 自動パターン切替 1回
9 = 自動パターン切替 ループ
10 = 自動パターン切替 往復
```

### 通用七参数 layout

`rpg-maker-clone` 对 sample game 的 279 个 opcode 290 调用都观察到七参数形态：

```text
arg0 = options
arg1 = time / interval / effect-specific frame parameter
arg2 = first picture
arg3 = last picture
arg4 = value1
arg5 = value2
arg6 = value3
```

并指出这与 `wolfrpg-map-parser::effect_command::base::Base` 的七字段模型一致。citeturn298566view1

options：

```text
target      = arg0 & 0x0F
effect_type = (arg0 >> 4) & 0x0F
```

target：

```text
0 = Picture
1 = Character
2 = Map
```

citeturn792948view2turn298566view1

### Picture target 一定是闭区间，不是 A→B 渐变

这一项可以给你确定答案：

**`arg2..arg3` 是 Picture ID 的连续闭区间。**

官方手册写的是：

> ピクチャに対するエフェクト（□～□ の欄にピクチャ番号を指定）

也就是对编号区间施加 effect。citeturn792948view4

`rpg-maker-clone` 当前实现更直接：

```ruby
(first..last).each
```

对每张 Picture 分别调用 effect。citeturn792948view2

其源码注释还记录了真实游戏样本中确有一次 ColorCorrect 同时覆盖六个连续 Picture ID。citeturn298566view1

所以绝不是：

```text
picA 的参数逐渐插值到 picB
```

而是：

```text
for picture_id in picA..=picB
```

### 11 种参数表

结合官方 UI 文字、`wolfrpg-map-parser` enum 和旧官方更新日志，可以得到下面这张对你们解释器最有用的表：

| type | 官方效果 | arg1 | arg4 | arg5 | arg6 |
|---|---|---|---|---|---|
| 0 | Flash | frames | R | G | B |
| 1 | ColorCorrect | process/delay frames | R | G | B |
| 2 | DrawPositionShift | process/delay frames | X shift | Y shift | unused/0 |
| 3 | Shake | per-process frames | X shake | Y shake | count |
| 4 | Zoom | frames | center X | center Y | zoom % |
| 5 | SwitchFlicker | interval frames | R | G | B |
| 6 | SwitchAutoFlash | interval frames | R | G | B |
| 7 | AutoEnlarge | enlarge/shrink frames | horizontal Δ% | vertical Δ% | rest frames |
| 8 | AutoPattern One | interval frames | start pattern | end pattern | normally unused |
| 9 | AutoPattern Loop | interval frames | start pattern | end pattern | normally unused |
| 10 | AutoPattern RoundTrip | interval frames | start pattern | end pattern | normally unused |

对于 0、3、5，当前 `rpg-maker-clone` 已经直接按上述槽位运行：Flash 读取 arg1+arg4..6；Shake 读取 arg1、X、Y、count；Flicker 读取 arg1、RGB。citeturn792948view2

type 4 的官方/编辑器字符串明确显示格式相当于：

```text
中心X / 中心Y / zoom% / frames
```

因此对应：

```text
arg1 = frames
arg4 = center X
arg5 = center Y
arg6 = zoom %
```

citeturn126620search3turn126620search11

type 7 的官方更新日志更直接说明可设置：

```text
横拡大率加算
縦拡大率加算
休フレーム
処理時間
```

所以对应 Base 七槽最自然且与 UI 顺序吻合的映射就是：

```text
arg1 = process time
arg4 = Δzoom X
arg5 = Δzoom Y
arg6 = rest frames
```

citeturn126620search2

type 8/9/10 的官方描述确认三者共同使用：

```text
开始 pattern
结束 pattern
切换 interval frames
```

一回结束后回原 pattern；Loop 到尾后回 start；RoundTrip 到尾后反向逐帧回 start。citeturn792948view4turn126620search1

因此：

```text
arg1 = interval
arg4 = start pattern
arg5 = end pattern
```

是高度可信的；`arg6` 在这三个旧式 Base effect 中没有官方公开的第四语义字段，建议保持 raw/reserved，不要赋予意义。

### 一个 3.50 特别注意事项

上面 0..10 是 `wolfrpg-map-parser` 这套旧 PictureEffectType 的核心表；**3.50 官方手册已经新增了更多 picture effects**，例如第二种坐标 shift、倍率补正、角度、不透明度、转换、Picture copy/move/Z order、视频操作、picture-link 等。citeturn792948view4turn678780search1

所以：

```rust
match effect_type {
    0..=10 => legacy_known,
    n => UnknownRaw(n),
}
```

比：

```rust
unreachable!()
```

合理得多。

你们的 Steam 游戏是 3.5 系，如果全库普查真的只出现 0..10，那实现这 11 个即可；但不要据此定义“opcode 290 永远只有 11 种”。

## コモンイベント 210 / 300：实参、CSelf 与返回值

这里你们的实测有一半非常准确，另一半需要把“地址编码”与“参数槽编号”分开。

### 210 和 300 的差别

当前 `rpg-maker-clone`：

```text
210 CommonEvent:
    target 由 event id 指定

300 CommonEventByName:
    target 由 command string 中的名称指定
```

其余参数协议基本复用相同结构。citeturn491950view4turn627380view1

210 中，公共事件引用的编码区间使用：

```text
500000 + CommonEvent ID
```

实现中会减去 `500000` 得到实际 CE id。citeturn491950view4

### 参数状态 word

当前实现观察：

```text
param_status low nibble       = number_count
param_status next nibble      = string_count
bit 0x01000000                = return enabled
```

numeric 参数随后依次跟在 command args 中；开启返回值时，caller 的 return target 放在这些实参之后。citeturn491950view4

这与你说的：

> “返回值写回调用者参数表末尾”

一致。

### 实参到底写入哪个 CSelf

官方 Common Event 手册明确规定普通输入参数位置：

```text
数值参数 1 → CSelf0
数值参数 2 → CSelf1
数值参数 3 → CSelf2
数值参数 4 → CSelf3

字符串参数 1 → CSelf5
字符串参数 2 → CSelf6
字符串参数 3 → CSelf7
字符串参数 4 → CSelf8
```

citeturn531988view2

同时，官方 ValueRef 体系规定：

```text
1600000 ～ 1600099
```

是“当前 Common Event 的 CSelf 直接引用范围”。citeturn531988view2

所以你观察到：

```text
1600000 + i
```

这一部分是对的，但称它为“CSelf 第 6 组”容易误导。

它实际上是：

```text
1600000 + 0 = 当前 CommonEvent 的 CSelf0
1600000 + 1 = CSelf1
...
```

也就是说，**1600000 是地址空间基址，不是第六组参数 bank。**

官方还规定 CSelf5～9 是字符串槽，而其他 CSelf 是数值槽。citeturn525476search2

`rpg-maker-clone` 当前内部调用实现会依次找 numeric self slots，跳过字符串区；这属于它为了通用解释器做的实现策略，并不应覆盖官方“事件定义里明确声明的 4 numeric + 4 string 输入槽”规则。citeturn627380view1

对于兼容 WOLF 3.5，建议你们按 CommonEvent.dat 的 `argument_number_count` / `argument_string_count` 和参数定义来映射，而不是“按所有 numeric CSelf 顺序无限塞”。

### return_value_id

官方 Common Event 编辑器有明确的“結果を返す”选项，语义是：

> 将指定的 Common Self Variable 的值返回给调用方。

citeturn531988view2

`CommonEvent.dat` 的公开格式解析中也有一个实际字段：

```text
return_value_id
```

citeturn678780search5

当前 `rpg-maker-clone` 的调用实现就是：

```text
1. 运行 callee
2. 读取 callee 的 return_variable / return_value_id 对应 CSelf
3. 写入 caller command 尾部指定的 return target
```

citeturn627380view1

所以你们这条实测：

> “返回值从被调事件的 return_value_id 槽读回、写回调用者参数表末尾”

**可以评为强确认。**

Rust 里建议直接建模成：

```rust
struct CommonCall {
    target: CommonTarget,
    number_args: Vec<ValueRef>,
    string_args: Vec<StringRef>,
    return_target: Option<ValueRef>,
}

struct CommonEventDefinition {
    return_self_slot: Option<u8>,
    ...
}
```

而不是把 callee return slot 塞进 call instruction 本身。

## 常時作動、run_condition 与“63 / 241 是否是引擎隐式 Common Event”

### run_condition 的官方意义

`CommonEvent.dat` 的公开格式和官方手册一致：

```text
0 = 呼び出しのみ / call_only
1 = 自動実行 / auto_start
2 = 並列実行 / parallel_process
3 = 並列実行（常時） / parallel_process_always
```

citeturn678780search5turn531988view2

其中：

- 自动执行：条件满足时启动，并阻塞通常事件流程，直到结束。
- 条件并行：条件满足期间并行运行。
- 常时并行：不依赖普通条件，持续作为并行事件调度。citeturn531988view2

因此 **“哪些 CE 会自动跑”首先由数据里的 run_condition 决定，不是由固定编号决定。**

### “run_condition=3 的 10 个 + 241”不能当成引擎规则

如果你们这个具体 Steam 游戏里统计出来：

```text
10 个 run_condition=3
以及 CE 241
```

那只说明**该项目的数据如此**。

官方格式没有定义：

```text
CE #241 永远自动执行
```

这样的特殊编号。`wolfrpg-map-parser` 也只是按 `run_condition` 字段读取，而没有 CE id special-case。citeturn678780search5

因此如果 241 没有 run_condition=3，但你们观察到它自行进入，需要继续查：

```text
是否 run_condition=1/2
是否被某个 always-parallel CE 调用
是否由 map event 调用
是否是 reserved common event call
是否由基本系统初始化链调用
```

不要先归因于 Game.exe 的硬编码 CE ID。

### CE 63 更不像是“引擎隐式调用”

这一点值得特别纠正。

`rpg-maker-clone` 的 ADR 对它用来 soak-test 的 RPG Basic System 样本明确提到：

```text
X[共]メッセージウィンドウ
```

是一个 Common Event，它被其他 Common Events 调用，而且里面有真正的 WOLF 事件循环；正因为缺少 `GotoLoopStart` 实现，这个 CE 曾把整个解释器跑进无限循环。citeturn678780search0

这说明“メッセージウィンドウ”是**Basic System 事件代码**，不是隐藏在 Game.exe 里的固定 CE63 调用。

因此你们如果项目里 `63 = メッセージウィンドウ`，且看到大量消息流程落到 CE63，正确建模应更接近：

```text
其他 Common Events / map events
      ↓
call CommonEvent 63
      ↓
CE63 画窗口 / 管理状态
```

而不是：

```text
opcode 101
      ↓ Game.exe hidden magic
implicit call CE63
```

目前我没有找到官方文档支持 “Game.exe 固定隐式调用 Common Event 63” 这个规则。

若你们动态 trace **确实出现 instruction stream 中没有 210/300、但 CE63 execution frame 凭空出现**，那反而是极有价值的新证据；建议在真正 Game.exe 上对 call stack / 当前 CE sysvar 做一次黑盒测试。但现有官方资料与三个开源实现都不支持把 63 写死成 engine special id。

## 事件结束后的 Picture / 窗口生命周期

### Picture 不会因为事件函数 return 而自动清除

官方 Picture 模型是全局编号对象：

```text
一个 picture number 保存一张当前 picture
同编号再次 Show 会替换/转成 Move 行为
Erase 明确负责消除
```

官方甚至建议为了缓存图片，可以把某个 Picture **长期以 opacity 0 显示着**；如果 Picture 会在事件结束时自动清掉，这种官方建议就不可能成立。citeturn114445view4

官方 Effect 手册也明确说明某些 Picture effect 会持续存在，直到：

```text
把 effect 参数复位
或 picture 被消去
```

citeturn792948view4

因此对你们解释器：

```rust
return_from_event();
```

绝对不要做：

```rust
scene.clear_pictures_owned_by_this_event();
```

WOLF Picture 不是 event-owned temporary sprite。

### 窗口也不能按“事件结束”统一清

需要分两种：

**opcode 101 的文字状态**由 message engine 自己管理，它的结束/强制关闭遵循 message command 生命周期。

但 **RPG Basic System 的对话框背景、头像、装饰、菜单窗体**通常本质上都是 Picture(150)。这些就必须由事件自己发：

```text
Picture Erase
```

或者用后续 Show 覆盖。citeturn678780search0turn557206search2

所以：

```text
事件 return
≠ 自动 erase Picture
≠ 自动 erase Basic-System window pictures
```

这也与你问的“关闭动画是不是 6 帧”联系起来：窗口图片的 6-frame fade/slide 如果存在，就是事件代码中的 Picture erase/move `process_time=6`，不是 Common Event 返回后由 Game.exe 统一清理。

### 3.50 的 Picture 行为模式还需要纳入

Ver3.50 新增了 **Picture behavior Ver2**。官方说明，Ver2 下同 ID 已显示 Picture 再执行 Show 时，会继承“当前瞬间”的坐标和 opacity，并表现得更像 Move；有处理时间的 Erase 也从“执行瞬间坐标”开始消失。Ver1 与它有旧兼容差异。citeturn895398search0

因此你们是在重写一个 3.5 Steam 商业游戏，最好不要只实现：

```rust
if show_existing_id {
    replace_immediately();
}
```

而应该读取 `Game.dat` 中项目的 Picture behavior version，然后分别实现：

```text
Picture behavior Ver1
Picture behavior Ver2 / 3.50+
```

否则一些 UI 过渡很可能出现“逻辑全对但画面错半拍”的情况。

## 可直接用于 Rust 实现的校正版

把目前证据压缩成一套可以直接与你们现有逆向结果 diff 的定义，我会建议采用下面的版本：

```rust
// Picture 150
operation     =  head        & 0x0f;
display_type  = (head >>  4) & 0x07;
blend         = (head >>  8) & 0x0f;
anchor        = (head >> 12) & 0x0f;
zoom_mode     = (head >> 20) & 0x0f;
range         = (head & (1 << 24)) != 0;
free_transform= (head & (1 << 26)) != 0;

// Base type 1 / 4:
string_var_id = arg[11];

// Picture process_time:
// interpolation/fade duration, NOT an event wait.
```

Picture 的 operation/display/blend/range/anchor 字段都有多源支持；但 anchor 5/6 与各高级 Picture layout 仍建议保留 Unknown/raw，直到你们从 3.50 editor 制造最小样本。citeturn792948view1turn694803view2turn114445view4

```rust
// Database 250
db_type    = (flags >> 8)  & 0x0f; // 0 CDB, 1 SysDB, 2 UserDB
direction  = (flags >> 12) & 0x0f; // 0 write, 1 read
assign_op  = (flags >> 4)  & 0x0f;
use_ref    =  flags        & 0x01;

arg0 = type_id;
arg1 = data_id;
arg2 = field_id;
arg3 = flags;
arg4 = source_or_dest; // except string-literal 4-arg form
```

并把 assignment 改成：

```text
0 =
1 +=
2 -=
3 *=
4 /=
5 %=
6 min
7 max
8 abs
```

你当前版本最值得立即修的就是 **6/7 反了，以及 `&7` 会把 8=abs 吃掉**。citeturn298566view1turn266769view0

```rust
// Effect 290, Picture target
target_kind = arg0 & 0x0f;
effect_type = (arg0 >> 4) & 0x0f;

first_pic = value(arg2);
last_pic  = value(arg3);

for pic in first_pic..=last_pic {
    apply_effect(pic, ...);
}
```

0..10 的 effect type 可直接采用 `wolfrpg-map-parser` enum。citeturn694803view0turn792948view2

```text
0 Flash              time,R,G,B
1 ColorCorrect       time,R,G,B
2 DrawShift          time,X,Y,-
3 Shake              interval,X,Y,count
4 Zoom               time,centerX,centerY,zoom%
5 Flicker            interval,R,G,B
6 AutoFlash          interval,R,G,B
7 AutoEnlarge        time,dZoomX,dZoomY,rest
8 AutoPatternOne     interval,start,end,-
9 AutoPatternLoop    interval,start,end,-
10 AutoPatternRound  interval,start,end,-
```

citeturn792948view2turn126620search2turn126620search3turn126620search1

对于 Common Event：

```text
numeric args  → declared numeric CSelf inputs, normally CSelf0..3
string args   → declared string CSelf inputs, normally CSelf5..8

1600000+n     → 当前 CE 的 CSelf[n] ValueRef 地址

callee return → callee.return_value_id
              → caller command 的 return target
```

citeturn531988view2turn627380view1

最后，关于你们目前七个猜测，证据强度可以归纳为：

| 你的猜测 | 结论 | 置信度 |
|---|---|---|
| Picture erase 有 range | **是** | 很高 |
| `[... pic2]` 一定表示绝对 `pic..pic2` | 很可能，但需最小样本确认“终点 vs count” | 中高 |
| Move 尾帧 = 插值帧 | **是** | 官方确认 |
| type 1/4 字符串变量在 Base arg11 | **是** | 高 |
| Anchor 0=左上,1=中心 | **是** | 高 |
| DB `container=(flags>>8)&3` | 基本对，但字段应按 nibble | 高 |
| DB bit12=read | **是** | 高 |
| DB op 6=max, 7=min | **错：6=min,7=max；还有8=abs** | 很高 |
| Effect target 是闭区间 | **是** | 很高 |
| Effect 3/7/8/9/10 编号 | **是** | 很高 |
| 参数进 `1600000+i` | 地址解释对，但不是“第6组” | 很高 |
| return_value_id → caller 尾 target | **是** | 很高 |
| 固定 CE241 自动启动 | **不是通用引擎规则** | 高 |
| Game.exe 隐式调用 CE63 | **目前证据不支持；更像 Basic System 显式调用链** | 高 |
| Event return 自动清 Picture | **否** | 很高 |
| opcode 101 固定 6-frame close | **否，至少不是官方 engine contract** | 高 |
| Text_Pause 17×90 = 3×30 | **默认规则不对；官方默认纵向 6 分割** | 很高 |

其中最值得你们马上改代码的三点是：**DB 的 min/max/abs、不要把 `process_time` 当 Wait、不要在事件 return 时清 Pictures**。而最值得制作 WOLF 3.50 最小测试工程继续验证的三点是：**range erase 最后一参到底是 end-id 还是 count、中央上/中央下 packed anchor 编号、3.50 新增 Effect/Picture 模式的 raw layout**。这些正是目前官方只描述 UI 语义、开源解析器又没有完全覆盖的少数缺口。citeturn114445view4turn792948view1turn678780search1