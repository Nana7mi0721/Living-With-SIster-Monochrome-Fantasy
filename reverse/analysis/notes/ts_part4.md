
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

现有 `wolf-vm::text::expand(text: &str, lookup: &L) -> String` 是临时实现：它直接对原字符串
做 64 轮「最内层替换」，只能处理 `\cself`/`\self`/`\v`/`\s`/`\sys` 与 `\\`。
换成 token 化之后，替换变成一次自底向上遍历，**不需要轮数上限**，也不会误伤 `\E3000G`。

### 5.2 需要的 trait

`VarRef` 的每个字段本身也是 token 列表，所以求值要先递归求值字段、再把结果拼成数字/名字。
因此查询接口要能同时回答「数值槽」「字符串槽」「数据库坐标」三种问法：

`
use wolf_format::messagetext::{TextToken, VarBank, VarRef};

/// The VM state a message needs in order to be rendered.
///
/// Every method takes &self: expansion is a read-only pass over the VM, and
/// keeping it immutable means a message can be expanded while the interpreter
/// is mid-instruction without borrowing the mutable half of the world.
pub trait VarSource {
    /// \v[n], \vK[n], \sys[n], \self[n], \cself[n] — numeric slots.
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
`

### 5.3 展开函数

`
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
`

`resolve` 的骨架：先用 `expand_vars` 把 `var.args` 里每一层字段递归求值成纯文本，再
`strip` 掉残留的绘制码得到裸字符串，按 `VarBank` 分派到 `number` / `string` / `db`，
最后把结果 `to_string()` 成文字。

### 5.4 调用点

`
// In the interpreter, where opcode 101 is executed:
let raw = &command.strings[0];
let tokens = messagetext::tokenize(raw);
let shown = wolf_vm::text::expand_vars(&tokens, &self.vars);
// shown goes to the message window, which walks it frame by frame for the
// typewriter and applies \c / \E / \f as it goes.
`

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

`cargo test -p wolf-format --test messagetext` —— 15 个测试：

- `roundtrip_is_exact_for_every_message`：12,027 条真实字符串，0 失败。
- `token_histogram_matches_spec` / `observed_codes_match_spec`：§3.1 的每个数字。
- `malformed_input_degrades_to_literal` / `nesting_is_bounded`：病态输入不 panic、不失真。
- `outline_before_digits_is_not_a_code`：`\E3000G` 那条。
- `nested_variable_escape_is_preserved`：`\cself[\v[12]]` 与 `\cdb[0:\cdb[6:0:1]:0]`。
- `reserve_bank_digit_is_part_of_the_name`：`\v1[30]` 与 `\v0[1]`。
- `image_path_is_not_tokenised`：`\img[ピクチャ\Test1.png]`。
- `ruby_splits_on_first_comma` / `non_canonical_argument_stays_verbatim`。
- `strip_keeps_prose_only` / `plain_text_is_one_token` / `angle_tags_are_exact` /
  `width_hint_counts_glyphs`。

数据缺席时测试自动跳过（`WOLF_DATA_ROOT`，默认 `work/full`），crate 在没有游戏文件时仍可测。

---

## 8. 参考

- <https://pipoya.net/rpg/special-character/>（官方，全部控制码）
- <http://yado.tk/wolf/02_ibekoma/0101_tokushumoji/>（社区，码表 + 嵌套优先级）
- `docs/FORMAT_SPEC.md` §4.4：opcode 101 = Message，12,027 次；文本在字符串表里，0 参数字节。
- `docs/ARCHITECTURE.md`:77：wolf-render 的 `\E` 描边实现。
