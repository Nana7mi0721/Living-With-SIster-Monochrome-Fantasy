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

## 安全规则（为什么只写“纯文本”行）

gen_override.py 只采纳**彼梦胜出且正文不含任何反斜杠转义码**的行（先剥掉 MV 的
\N<名> / \N[名] 说话人标签）。带 \c[]、\v[] 等变量的行保留官方中文，
因为把 MV 的转义语法猜着映射到 WOLF 的语法会静默改变显示内容。官方行开头的
\E（描边）会补回到译文前，保持字重一致。

覆盖表当前规模：7496 行 / 61 个事件（约占全部 12027 条台词的 62%）；剩余 4350 行
为彼梦移植没有的 DLC/相册事件，回落官方中文。
