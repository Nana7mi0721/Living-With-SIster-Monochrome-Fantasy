# TEXT_SPEC — 消息文本（opcode 101）的标记语法与词法契约

本文是 [crates/wolf-format/src/messagetext.rs](../crates/wolf-format/src/messagetext.rs) 的规格书：
它定义「消息字符串里那些反斜杠到底是什么」，记录**本作实际用到哪几个**，并给出运行时
（wolf-render / wolf-vm）消费 token 的推荐方式。

---

## 1. 范围与术语

WOLF RPG Editor 的 Message 指令（opcode 101）带一条字符串，它是**混合体**：既有玩家要读的
字面文字，也有半角反斜杠开头的**控制码**，后者改绘制状态（颜色、描边、字号）、注入变量值
（`\v[12]`、`\cself[13]`、`\cdb[21:78:0]`），或驱动打字机
（`\sp[40]`、`\.`、`\!`）。

- **字面文字**（literal）：玩家能读到的字符，UTF-8，本作含中文与日文标点。
- **控制码**（code）：`\` 开头、半角、**大小写敏感**（`\E` 不是 `\e`）。
- **token**：词法器对一条字符串的切分结果，见 `TextToken`。

三条铁律（对应模块文档的三条不变量，逐条有测试覆盖）：

1. **lossless**：`to_source(tokenize(s)) == s` 对任意输入成立。token 列表不是源码的有损视图，
   而是与它一一对应的另一种写法 —— 这是「能把消息改完写回 CommonEvent.dat」的前提。
2. **永不 panic**：输入来自文件而非我们自己的源码，可能是改坏的存档、汉化补丁或 mod。
   未闭合的 `[`、结尾孤零零的 `\`、孤立 `<` 一律退化成字面文字。
3. **未知码原样透传**：WOLF 的语法比任何一款游戏用到的都大，猜错的代价是**悄悄改变玩家读到的内容**；
   留着原样则表现为可见的乱码 —— 明显、可报告、加一行表就能修。

---

## 2. 完整语法（官方/社区文档）

来源：<https://pipoya.net/rpg/special-character/> 与
<http://yado.tk/wolf/02_ibekoma/0101_tokushumoji/>。
两份文档互相补充，个别码的写法有差异（见 §2.2 的 `\A+` 注）。

> 本节是**兼容性**规格：词法器实现全部条目，但本作只用到其中一小部分（§3）。
> 理由见 §1 铁律 3 —— 只认实测子集的词法器会在别的 WOLF 游戏上静默出错。

### 2.1 字面反斜杠与无括号码

| 写法 | 含义 |
| --- | --- |
| `\\` | 一个字面反斜杠。必须有，否则 `Data\EnemyGraphic` 会被读成 `\E` |
| `\E` | 打开文字描边（edge） |
| `\N` | 关闭描边。**这是默认状态** |
| `\A+` / `\A-` | 开/关抗锯齿 |
| `\>` | 从此处起瞬间显示（取消打字机） |
| `\<` | 取消瞬间显示 |
| `\!` | 停下来等玩家按键，然后继续（结尾仍会再等一次） |
| `\^` | 不等按键，直接关闭消息窗 |
| `\.` | 暂停 0.25 秒（`\.\.\.\.` = 1.00 秒） |

注：`\!` / `\^` / `\.` 在「選択肢」与「ピクチャ」（字符串图片）里**无效**。

### 2.2 绘制状态码

| 写法 | 含义 |
| --- | --- |
| `\c[n]` | 文字颜色，n 是 SystemDB[12:文字色] 的**数据 ID** |
| `\cS[n]` | 阴影颜色，同一张表 |
| `\cE[n]` | 描边颜色；`\cE[-1]` 回到默认黑 |
| `\wE[n]` | 描边粗细 1–5 |
| `\f[n]` | 字号（像素） |
| `\f+[n]` | 相对字号变化，负值缩小 |
| `\m[n]` | 本行的基线位置，换行时重置为 `\m[0]` |
| `\font[n]` | 字体槽 0=基础、1/2/3=备用（在游戏基本设置里配） |
| `\-[n]` | 字间距收紧 n 像素（**负值加宽**） |
| `\space[n]` | 之后换行的行间距（像素） |

`\A+` 的写法：pipoya 页面印成**全角** `￥A+`，yado 页面印成半角 `\A+`。
这是排版渲染的假象，不是语法的一部分；词法器接受两种拼写。

### 2.3 变量与数据库

| 写法 | 含义 |
| --- | --- |
| `\v[n]` | 普通数值变量 |
| `\v1[n]` … `\v9[n]` | 备用变量库 1–9。**数字写在字母和括号之间**，不是 `\v[1:n]` |
| `\s[n]` | 字符串变量 |
| `\sys[n]` | 系统变量 |
| `\sysS[n]` | 系统字符串，**大写 S 必须有** |
| `\self[n]` | 地图事件的自身变量 |
| `\cself[n]` | 公共事件的自身变量；槽 0–4 与 10–99 是数值，5–9 是字符串 |
| `\udb[A:B:C]` | 用户数据库 |
| `\cdb[A:B:C]` | 可变数据库 |
| `\sdb[A:B:C]` | 系统数据库 |
| `\xy[N:X:Y]` | XY 数组按编号取格 |
| `\xyN[NAME:X:Y]` | XY 数组按名字取格 |

DB 类码还接受两段式 `\cdb[T:D]`（D 是数据**名**）与一段式 `\cdb[T]`（T 是类型**名**）；
`\sdb` / `\udb` 同。

**嵌套优先级**（官方）：`\v` < `\v1`..`\v9` < `\self` < `\cself` < `\sys`
< `\udb` < `\cdb` < `\sdb` < `\s` < `\sysS`。
括号里**只能**放优先级更低的码，所以 `\cself[\v[12]]` 合法、`\v[\cself[2]]` 不合法。
这就是 `VarRef::args` 存 token 列表而不是整数、词法器必须递归的原因。

### 2.4 打字机与定位

| 写法 | 含义 |
| --- | --- |
| `\sp[n]` | 之后文字的每秒字符数；`\sp[0]` = 瞬间。默认取系统变量[9]，初值 500 |
| `\bx[n]` | **之后各行**的 X 起点（第一行必须用 `\mx`） |
| `\mx[n]` / `\my[n]` | 下一个字相对当前笔位的偏移（±1000..±5000） |
| `\ax[n]` / `\ay[n]` | 下一个字的绝对位置（相对行首 / 第一行顶部） |

### 2.5 注音、图标、图片、音效

| 写法 | 含义 |
| --- | --- |
| `\r[文字,ルビ]` | 在「文字」上方画注音。注音颜色 = SystemDB[12] 数据 13，字号 = 系统变量[23] |
| `\fR[n]` / `\cR[n]` | 覆盖注音字号 / 颜色 |
| `\i[n]` / `\iS[n]` | 行内图标，取自 `Data/BasicData/iconNNN.png`（0–999 三位，1000–9999 四位）。`\i` 平滑缩放，`\iS` 最近邻 |
| `\isize[n]` | 之后 `\i` / `\iS` / `\img` 的百分比缩放 |
| `\img[路径]` / `\imgS[路径]` | 行内图片，路径相对 Data/；括号体里还可带 `<DIV/X6/Y4/8>`、`<CUT/X80-240/Y60-180>`、`<ANIME/X6/Y4/12>` 指令 |
| `\se[地址]` | 行内播放音效；括号体第二、三段是音量与频率 |

### 2.6 行对齐

`<L>` / `<C>` / `<R>` = 该行左/中/右对齐。对齐基准是**整条消息里最宽的一行**，
所以单行消息永远是左对齐；`<L>` 的作用是撤销前面的 `<C>` / `<R>`。
三个标签都必须**精确三字节**匹配 —— 消息里存在 `<状態A>` 这类正常行文。

---

## 3. 本作实测（验收基线）

数据源：`work/full/BasicData/CommonEvent.dat`，version_header `0xC9`，550 个公共事件，
**12,027** 条 opcode 101 指令、12,027 条字符串（解析器消费 5532174/5532175 字节）。

### 3.1 出现过的码

| 码 | 次数 | 说明 |
| --- | ---: | --- |
| `\E` | 11,964 | 11,915 条消息**以它开头**；从不带括号 |
| `\c[n]` | 386 | 只有 4 个取值：0→192、1→89、2→29、7→76 |
| `\cself[n]` | 113 | 17 个不同槽位，范围 0..97+ |
| `\f[n]` | 76 | 10 个不同字号，最大 72，最常见 14（44 次） |
| `\v[n]` | 35 | 28 个不同槽位，全部数值 |
| `\cdb[21:a:b]` | 12 | 三段式，首段恒为 21 |
| `\sp[n]` | 1 | `\sp[8]` |
| `\>` | 44 | 常与 `\cself[9]` 连用 |
| `\.` | 9 | 只出现在 H 场景文本 |
| `\^` | 6 | 同上 |
| `<C>` | 28 | |
| `<L>` | 11 | |

**未出现**（本作不用，但词法器实现）：`\\` `\N` `\wE` `\cE` `\cS`
`\f+` `\m` `\font` `\A+` `\A-` `\-` `\s` `\sys`
`\sysS` `\self` `\xy` `\xyN` `\udb` `\sdb` `\!` `\<`
`\r` `\i` `\iS` `\isize` `\img` `\imgS` `\se` `\bx`
`\mx` `\my` `\ax` `\ay` `\space` `<R>`。

### 3.2 其它实测事实

- 3 条消息完全不含反斜杠；3,477 条含换行（最多 4 个）；无 TAB / CR。
- 最长消息 226 字符。
- 所有数字参数都是 **canonical 十进制**：无前导零、无 `+12`、无 `-0`。
  所以整数 token 用 `to_string()` 回写即字节精确（§4.3）。
- 括号全部配平；无以反斜杠结尾的消息；`[` 后无空格。
- `<...>` 里除 `<C>`/`<L>` 外都是正常行文（`<状態A>` 等），必须精确匹配。
- `\E` 后面紧跟数字是**普通文字**，不是未知码：`\E3000G诶……要买么？` 是
  `Outline(true)` + `Text("3000G诶……要买么？")`。这类 6 条（3000G / 4000G / 10回合 /
  5天 / 3年 / 1号桌 / 5天）。
- `\` + 字面中日文 3 条、`\` + 换行 1 条，都是作者手误或排版，按字面文字处理。
- 开头的 `1` 前缀（`1\n`）存在于 20 条消息，但它属于**说话者/立绘**前缀，
  不在本模块的词法范围内。

### 3.3 验收基线（本模块的测试断言）

`cargo test -p wolf-format --test messagetext` 共 16 个测试，其中 3 个读真实数据：

| 断言 | 实测值 | 说明 |
| --- | ---: | --- |
| 公共事件数 | 550 | 与 `docs/BASELINE.json` 一致 |
| opcode 101 字符串数 | 12,027 | |
| **round-trip 失败数** | **0 / 12,027** | `to_source(tokenize(s)) == s` |
| token 变体种类 | 11 | 只有 §3.1 那些 |
| `Text` | 15,864 | |
| `Outline` | 11,964 | |
| `Newline` | 3,502 | |
| `Color` | 386 | |
| `Var` | 160 | `v` 35 + `cself` 113 + `cdb` 12 |
| `FontSize` | 76 | |
| `Instant` | 44 | |
| `Align` | 39 | `<C>` 28 + `<L>` 11 |
| `Pause` | 9 | |
| `Close` | 6 | |
| `Speed` | 1 | |
| `Code` 回落 | 0 | 本作没有非规范参数 |
| 以 `\E` 开头 | 11,915 | |
| 完全无反斜杠 | 3 | |
| 含换行 | 3,477 | |

另有 **60 条病态输入** 的单元测试（`pathological_inputs_round_trip`）全部 0 失败：
`\`、`\c[`、`\c[]`、`\v[1`、`\cself[\v[`、`\img[`、`<`、`<L`、
`<X>`、`\A`、`\r[a]`、`\v9[1]`、`\v0[1]`、`\img[ピクチャ\Test1.png]`、
`\cdb[0:\cdb[6:0:1]:0]`、NUL/BOM 字符等。

### 3.4 语义 vs 原文：哪些码我们「懂」，哪些只「留」

| 级别 | 码 | 运行时行为 |
| --- | --- | --- |
| **实现语义** | `\E` `\N` `\c` `\f` `\f+` `\m` `\font` `\cS` `\cE` `\wE` `\A+` `\A-` `\-` `\space` `\sp` `\>` `\<` `\.` `\!` `\^` `\bx` `\mx` `\my` `\ax` `\ay` `<L>` `<C>` `<R>` | 词法器给出**有类型的** `TextToken` 变体，渲染器/VM 直接消费，不需要再解析字符串 |
| **保留原文** | `\img` `\imgS` `\se` `\r` | 括号体是**路径/地址/带指令的载荷**，不递归、不解释转义（`\img[ピクチャ\Test1.png]` 的路径里合法含反斜杠）。`\r` 按**第一个逗号**切成 `Ruby{text,ruby}` |
| **结构完整、语义待接** | `\v` `\v1`..`\v9` `\s` `\sys` `\sysS` `\self` `\cself` `\udb` `\cdb` `\sdb` `\xy` `\xyN` | 词法器只给出 `VarRef{bank, args}`，**不查值**（本 crate 没有 VM 状态）。求值见 §5 |
| **原样透传** | 未在 §2 出现的任何 `\x` | 保持字面文字，`to_source` 逐字节还原 |

---

## 4. 词法契约（crate API）

`use wolf_format::messagetext::{tokenize, strip, to_source, display_width_hint, TextToken, VarRef, VarBank, CodeKind, Align, MAX_DEPTH};`

### 4.1 入口

`pub fn tokenize(s: &str) -> Vec<TextToken>`
—— 切分。永不 panic、永不丢字节。相邻字面字符合并成一个 `Text`（token 列表是字符串最短的保字节描述）。

`pub fn to_source(tokens: &[TextToken]) -> String`
—— `tokenize` 的**精确逆运算**。这是「能安全改写消息」的依据。

`pub fn strip(s: &str) -> String`
—— 只要纯文本：保留 `Text` / `Newline` / `Backslash`（还原成单个 `\`）/ `Ruby.text`，其余全丢。
日志、搜索索引、调试打印用它，**渲染器不要用它**（会丢颜色和描边）。

`pub fn display_width_hint(tokens: &[TextToken]) -> u32`
—— 字形格数的**上界估计**，供换行初判。忽略字间距、字号与 `Var` 展开后的位数，
渲染器排完行仍须实测字形宽度。

`pub const MAX_DEPTH: u8 = 8` —— 括号递归深度上限；超过则反斜杠按字面处理。

### 4.2 `TextToken`（37 个变体）

| 分组 | 变体 |
| --- | --- |
| 字面 | `Text(String)` `Newline` `Backslash` `Align(Align)` |
| 描边 | `Outline(bool)` `OutlineWidth(u8)` `OutlineColor(Option<u32>)`（`None` = `\cE[-1]`） |
| 颜色 | `Color(u32)` `ShadowColor(u32)` `RubyColor(u32)` |
| 字号/排版 | `FontSize(u32)` `FontSizeDelta(i32)` `RubySize(i32)` `Baseline(i32)` `FontSlot(u8)` `LetterSpacing(i32)` `LineGap(i32)` `AntiAlias(bool)` |
| 打字机 | `Speed(u32)` `Wait` `Close` `Pause` `Instant(bool)` |
| 定位 | `LineStartX(i32)` `OffsetX(i32)` `OffsetY(i32)` `AbsoluteX(i32)` `AbsoluteY(i32)` |
| 内联资源 | `Icon(u32)` `IconNearest(u32)` `IconScale(u32)` `Image(String)` `ImageNearest(String)` `Sound(String)` `Ruby{text,ruby}` |
| 求值 | `Var(VarRef)` |
| 兜底 | `Code{kind, args}` |

`impl fmt::Display for TextToken` 走 `to_source`，所以 `format!("{token}")` 得到源码拼写。

### 4.3 三个设计取舍

**(a) `\E` 是 token，不是文字上的装饰。**
原引擎画描边是**同一段字形画四遍偏移副本**（见 `docs/ARCHITECTURE.md`:77）。
如果词法器把 `\E` 摊平成「带描边属性的文字段」，绘制顺序就丢了 —— 一段文字里
`\E\c[1]甲\N乙` 的「甲有描边、乙没有」必须由 token 序列表达，不能由属性合并表达。

**(b) 数字参数做 canonical 校验，非规范就回落 `Code`。**
`parse_u32` 要求 `value.to_string() == text`：`007`、`+12`、`-0` 都**不算**整数，
而是落进 `TextToken::Code{kind, args}` 保留原文。本作 0 例外（§3.2），所以这条规则
在本作是空转；它存在是为了别的 WOLF 游戏和手改过的数据 —— 规范化会把 `\c[007]` 写成
`\c[7]`，那就不是 lossless 了。

**(c) 括号体不递归的三处例外。**
`\img` / `\imgS` / `\se` 的括号体是**路径或地址**，`\img[ピクチャ\Test1.png]` 里那个
反斜杠是路径分隔符不是转义起始；`\r` 的括号体是「文字,注音」两段自然语言。
这四者存原始字符串。其余全部递归（`\cself[\v[12]]` 的内层必须保住）。

---

## 5. 推荐调用方式：tokenize → 展开成纯文本（给 wolf-vm）

### 5.1 分工

| 层 | 职责 |
| --- | --- |
| `wolf-format::messagetext` | 只恢复**结构**。给出 `VarRef`，不查值（没有 VM 状态） |
| `wolf-vm` | 把 `VarRef` 求值成文字，产出**可以直接交给渲染器**的 token 列表 |

> **现状（M1）**：`crates/wolf-vm/src/text.rs::expand` 是**临时实现** —— 它不 token 化，
> 直接对原字符串做最多 64 轮「最内层替换」，码表只有 `\cself`/`\self`/`\v`/`\s`/`\sys`
> 及其别名拼写（`\cself_s`/`\self_s`/`\corgv`/`\corgs`/`\csys`），且只认 `[n]` 形式
> —— 数据库类 `\cdb`/`\sdb`/`\udb`、`\xy`、以及括号里套括号的间接引用它都不处理。
> 迁移到本节这套 tokenize + 一次遍历是 **M2 的重构项**。
>
> **分工写死，后来的人不要重复实现**：词法、结构、lossless 回写归 `wolf-format::messagetext`
> （已实现并有 16 个测试兜底）；**只**有「把 `VarRef` 查成值」这一件事归 `wolf-vm`。
> 所以在 `wolf-vm` 里不要再写第二个转义解析器 —— 需要新语义时改 `messagetext` 的码表
> 或 `VarSource` 的实现，不要在 `text.rs` 里加正则。

换成 token 化之后，替换变成一次自底向上遍历，**不需要轮数上限**，也不会误伤 `\E3000G`。

### 5.2 需要的 trait

`VarRef` 的每个字段本身也是 token 列表，所以求值要先递归求值字段、再把结果拼成数字/名字。
因此查询接口要能同时回答「数值槽」「字符串槽」「数据库坐标」三种问法：

```
use wolf_format::messagetext::{TextToken, VarBank, VarRef};

/// The VM state a message needs in order to be rendered.
///
/// Every method takes &self: expansion is a read-only pass over the VM, and
/// keeping it immutable means a message can be expanded while the interpreter
/// is mid-instruction without borrowing the mutable half of the world.
pub trait VarSource {
    /// \v[n], \v1[n]..\v9[n], \sys[n], \self[n], \cself[n] — numeric slots.
    fn number(&self, bank: VarBank, slot: u32) -> Option<i64>;

    /// \s[n], \sysS[n], \cself[5..9] — string slots.
    fn string(&self, bank: VarBank, slot: u32) -> Option<String>;

    /// \udb[A:B:C], \cdb[A:B:C], \sdb[A:B:C], \xy[N:X:Y], \xyN[NAME:X:Y].
    ///
    /// A field is either a number or a name, because WOLF accepts both the
    /// three-part coordinate form and the named form. Resolving a name to a
    /// number is the VM's job, not the formatter's.
    fn db(&self, bank: VarBank, fields: &[Field]) -> Option<String>;
}

/// One resolved database coordinate component.
pub enum Field {
    /// A slot or index number.
    Number(i64),
    /// A data or type name, as written.
    Name(String),
}
```

### 5.3 展开函数

```
/// Resolve every Var token, leaving all drawing codes untouched.
///
/// Returns a token list the renderer can consume directly: the drawing codes
/// (\c, \E, \f, ...) are still typed tokens, and every Var has been replaced by
/// the literal characters the player will see. A slot the VM cannot resolve
/// expands to nothing rather than to an error string, matching the original
/// engine, which draws a blank where a missing variable would be.
pub fn expand_vars(tokens: &[TextToken], source: &dyn VarSource) -> Vec<TextToken> {
    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        match token {
            TextToken::Var(var) => out.push(TextToken::Text(resolve(var, source))),
            TextToken::Ruby { text, ruby } => out.push(TextToken::Ruby {
                text: text.clone(),
                ruby: ruby.clone(),
            }),
            other => out.push(other.clone()),
        }
    }
    out
}
```

`resolve` 的骨架：先用 `expand_vars` 把 `var.args` 里每一层字段递归求值成纯文本，再
`strip` 掉残留的绘制码得到裸字符串，按 `VarBank` 分派到 `number` / `string` / `db`，
最后把结果 `to_string()` 成文字。

### 5.4 调用点

```
// In the interpreter, where opcode 101 is executed:
let raw = &command.strings[0];
let tokens = messagetext::tokenize(raw);
let shown = wolf_vm::text::expand_vars(&tokens, &self.vars);
// shown goes to the message window, which walks it frame by frame for the
// typewriter and applies \c / \E / \f as it goes.
```

`strip()` 只用在「这条消息讲了什么」的地方（日志、搜索、调试），不要用它喂渲染器。

---

## 6. 渲染器要点（wolf-render）

1. **不要把 `strip` 的结果拿去画** —— 会丢颜色、描边、字号。
2. **描边是四份偏移副本**：遇到 `Outline(true)` 就置标志，之后每个字形在
   `(±w,0)`、`(0,±w)` 各画一份，最后画正面。`OutlineWidth` 给 w（1–5）。
3. **打字机**：`Speed(u32)` 是每秒字符数，`Pause` 是固定 0.25 秒，`Wait` 阻塞到按键，
   `Close` 立刻收窗。`Instant(true)` 之后全部立即显示。
4. **换行与对齐**：`Newline` 断行；对齐基准是**最宽的一行**，所以要先排完整条消息再定
   每行 x（单行消息恒左对齐）。`LineStartX` 只影响之后各行。
5. **`display_width_hint` 只是上界**：CJK 全角、`Var` 展开后的位数、字间距都会改变实际宽度，
   排版必须实测字形。

---

## 7. 测试

`cargo test -p wolf-format --test messagetext` —— 16 个测试：

- `roundtrip_is_exact_for_every_message`：12,027 条真实字符串，0 失败。
- `token_histogram_matches_spec` / `observed_codes_match_spec`：§3.1 的每个数字。
- `malformed_input_degrades_to_literal` / `nesting_is_bounded`：病态输入不 panic、不失真。
- `outline_before_digits_is_not_a_code`：`\E3000G` 那条。
- `nested_variable_escape_is_preserved`：`\cself[\v[12]]` 与 `\cdb[0:\cdb[6:0:1]:0]`。
- `reserve_bank_digit_is_part_of_the_name`：`\v1[30]` 与 `\v0[1]`。
- `image_path_is_not_tokenised`：`\img[ピクチャ\Test1.png]`。
- `ruby_splits_on_first_comma` / `non_canonical_argument_stays_verbatim`。
- `pathological_inputs_round_trip`：60 条病态输入（§3.3 那张表），0 失败。
- `strip_keeps_prose_only` / `plain_text_is_one_token` / `angle_tags_are_exact` /
  `width_hint_counts_glyphs`。

数据缺席时测试自动跳过（`WOLF_DATA_ROOT`，默认 `work/full`），crate 在没有游戏文件时仍可测。

---

## 8. 参考

- <https://pipoya.net/rpg/special-character/>（官方，全部控制码）
- <http://yado.tk/wolf/02_ibekoma/0101_tokushumoji/>（社区，码表 + 嵌套优先级）
- `docs/FORMAT_SPEC.md` §4.4：opcode 101 = Message，12,027 次；文本在字符串表里，0 参数字节。
- `docs/ARCHITECTURE.md`:77：wolf-render 的 `\E` 描边实现。
---

## 9. 图内文字（`150` 的 kind 2/4）与消息文本的差异（本作实测）

**本节的触发点**：M2 剧情态（docs/M2_SLICE.md §2.3）画出的 HUD 里，顶栏三段文字互相压叠、
状态面板的数字漏到屏幕右沿。查下去发现根因不在排版，而在**图内文字的转义**。

### 9.1 `\f[n]` 是字号，图内文字里也用（已验证）

`150` 指令的字符串参数（kind 2 文本图 / kind 4 字符串变量图）里出现的 `\f[n]` 取值分布：

| n | 6 | 8 | 12 | 14 | 16 | 18 | 20 | 24 | 32 | 48 | 64 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 次数 | 18 | 1 | 253 | 55 | 640 | 62 | 58 | 66 | 73 | 39 | 2 |

48 与 64 不可能是脸图编号（本作 SysDB 24 只有 12 行脸图），所以这里的 `\f[n]` **只能是字号**；
消息文本里同样是字号（§3.1 记 76 次，最大 72）。
带 `\f[n]` 的图内字符串共 **1,206** 条（kind 2: 1,147 / kind 4: 59）。

### 9.2 目前的实现与后果（已知缺陷）

`wolf-cli` 的绘制路径对所有文本图都用一个字号（Sys8 = 20 px）出图，**没有按片段解析 `\f[n]`**。
后果在这个 HUD 上是可量化的：`pic 6200`（`\f[24]18:30`，x=140）与 `pic 6210`（`\f[18]周一`，x=150）
在 24 px 下必然压叠；`pic 6251`（`\f[24] 2409 \f[14]G`，x=1015）在 24 px 下必然越过 1024 右沿。
也就是说：**这三处看起来像作者排版问题，实际更可能是我们忽略了字号**（未验证：需要同一时刻的零售截图）。

### 9.3 图内文字独有的转义（消息文本里从不出现）

| 转义 | 含义 | 出处示例 |
|---|---|---|
| `\ax[n]` / `\ay[n]` | 文字锚点（1/1000 百分比） | 第三方气泡库 `pic 10238` `\f[0]0\ax[3200]` |
| `\udb[a:b:c]` / `\cdb[a:b:c]` | 用户/可变数据库取值 | `pic 1621` `\udb[28:1:0]`、`pic 6220` `\cdb[0:12:0]…哥哥` |
| `\space[n]` | 字间距 | `pic 119` `\space[0]` |
| `<R>` | 行右对齐（可出现在**同一图内**的多行之间） | `pic 10238` |
| `\-[n]` / `\A-` | 见 docs/OPCODE_SPEC.md（未实现） | 气泡库 |

这五个码在 §3.1 的「消息文本未出现」表里都记着，但它们在**图内文字**里是常态——
所以词法器必须按调用方区分上下文（`wolf_format::messagetext` 目前只服务 101）。

