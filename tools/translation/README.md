# 中文翻译流水线（策略 A：彼梦精翻为默认 + 术语规范化）

这套脚本把安卓彼梦精翻语料合并进 WOLF 本体，产出可以运行时加载的**文本覆盖表**，
不改动任何 .wolf / .dat 原始数据。设计见
[docs/research/TRANSLATION_DUAL_TRACK.md](../../docs/research/TRANSLATION_DUAL_TRACK.md)。

## 输入（都在 work/ 下，gitignored）

| 路径 | 内容 |
| --- | --- |
| work/wolftl/dump/common/*.json | WOLF 550 个公共事件的反编译 dump；code==101 的 stringArgs[0] 是官方中文台词，index 是该指令的 slot（= 运行时 pc）。 |
| work/translation/mv_text.json | 从彼梦 MV 仓库抽取的逐行台词（CommonEvents.json code 401）。 |
| work/translation/glossary_canonical.tsv | 术语规范表；on_image=true 且 canonical != bimeng 的行会把彼梦文本里的词替换成官方标准词。 |

## 两步流程

    # 从仓库根目录运行
    python tools/translation/pipeline.py      # align -> select -> normalize -> emit
    python tools/translation/gen_override.py  # 生成运行时覆盖表

pipeline.py 产出 work/translation/final/：

- wolf_text_AB.jsonl：逐行 official / bimeng / final / source / sim / method。
- final_text.jsonl：逐行最终文本（已清洗展示用标签）。
- summary_by_event.tsv、review_lowconf.tsv、REPORT_A_PIPELINE.md。

gen_override.py 产出 work/translation/final/wolf_override.tsv（event<TAB>pc<TAB>text），
供运行时直接加载。

## 运行时用法

    # 只做 A/B 文本检查，不开窗口
    WOLF_TEXT_OVERLAY=work/translation/final/wolf_override.tsv \
      target/debug/wolf-cli.exe run --root work/all --event 313 --trace

    # 截图（flag 会设好环境变量，capture/play/run 都生效）
    target/debug/wolf-cli.exe capture --root work/all --event 418 --frames 40 --hold-message \
      --text-overlay work/translation/final/wolf_override.tsv --out shot.png

Rust 侧入口是 wolf_game::translation::TextOverlay：Game::new 在编译程序集之前
按 (event id, slot pc) 替换 101 Message 的字符串，其余指令、变量引用与转义码
原样保留。未设置 WOLF_TEXT_OVERLAY 时行为与原始数据逐字节一致。

## 转义映射规则（MV -> WOLF）

gen_override.py 先剥掉 MV 的 \N<名> / \N[名] 说话人标签，再把 MV 转义翻译到 WOLF 语法：

- \C[n] / \c[n] -> \c[n]；若官方同一行在这段用不同色号，以官方色号为准（用官方行的
  同类 token 队列按顺序回填，避免 MV 的调色板与 WOLF 不一致）。
- \V[n] / \v[n] -> 官方的同类变量转义（如 \cself[8]）；官方没有时写 \v[n]。
- \{ / \} -> 官方行的字号转义（如 \f[32]）；\_ -> 半角空格，\_C[n] -> \C[n]。
- \. / \^ / \! 原样保留（WOLF 支持）。
- 官方行首的样式前缀（\E 描边、\f[n] 字号）整体补回到译文前。已知转义码表来自
  crates/wolf-format/src/messagetext.rs 的 tokeniser；扫描时按最长已知码匹配，否则
  \E 会把后面的 ASCII 正文吞成未知码。

无法安全映射的行回落官方。含 <L>/<C>/<R> 对齐标签的 20 行也回落官方：这些槽位通常
是“标题 + 正文”的多行帮助文本，MV 译文会合并成一段，替换会丢标题/对齐。

覆盖表当前规模：7657 行 / 64 个事件；其中 180 行是带转义的行，0 行残留未知转义
（tests/translation_overlay.rs 逐行 tokenise 校验）。其余 4350 行官方中文的行见
official_fallback_audit.tsv（按事件分类）与 official_only.tsv（逐行）。

## 官方兜底审计

    python tools/translation/audit_fallback.py

产出 work/translation/final/official_fallback_audit.tsv（每个仍有官方行的事件：标题、
method、category、official_lines、kana_lines、avg_sim、样例行）和 official_only.tsv
（全部官方回落行的 event/pc/official_zh）。category 区分 mv_no_text（彼梦无同号事件，
含 DLC/相册）、mv_id_collision（同号不同事件）、aligned_unmatched_lines（已对齐事件里
MV 缺的少数行）、dlc_absent、album_absent。45 行含假名的是引擎内部错误串，不是正文。
