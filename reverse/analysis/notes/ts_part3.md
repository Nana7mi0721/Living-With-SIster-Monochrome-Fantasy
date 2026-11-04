
### 3.3 验收基线（本模块的测试断言）

`cargo test -p wolf-format --test messagetext` 共 15 个测试，其中 3 个读真实数据：

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

另有 **60 条病态输入** 的单元测试（`malformed_input_degrades_to_literal` 等）全部 0 失败：
`\`、`\c[`、`\c[]`、`\v[1`、`\cself[\v[`、`\img[`、`<`、`<L`、
`<X>`、`\A`、`\r[a]`、`\v9[1]`、`\v0[1]`、`\img[ピクチャ\Test1.png]`、
`\cdb[0:\cdb[6:0:1]:0]`、NUL/BOM 字符等。

### 3.4 语义 vs 原文：哪些码我们「懂」，哪些只「留」

| 级别 | 码 | 运行时行为 |
| --- | --- | --- |
| **实现语义** | `\E` `\N` `\c` `\f` `\f+` `\m` `\font` `\cS` `\cE` `\wE` `\A+` `\A-` `\-` `\space` `\sp` `\>` `\<` `\.` `\!` `\^` `\bx` `\mx` `\my` `\ax` `\ay` `<L>` `<C>` `<R>` | 词法器给出**有类型的** `TextToken` 变体，渲染器/VM 直接消费，不需要再解析字符串 |
| **保留原文** | `\img` `\imgS` `\se` `\r` | 括号体是**路径/地址/带指令的载荷**，不递归、不解释转义（`\img[ピクチャ\Test1.png]` 的路径里合法含反斜杠）。`\r` 按**第一个逗号**切成 `Ruby{text,ruby}` |
| **结构完整、语义待接** | `\v` `\vK` `\s` `\sys` `\sysS` `\self` `\cself` `\udb` `\cdb` `\sdb` `\xy` `\xyN` | 词法器只给出 `VarRef{bank, args}`，**不查值**（本 crate 没有 VM 状态）。求值见 §5 |
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
