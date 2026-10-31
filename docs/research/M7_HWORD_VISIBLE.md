# M7 W2-b：H 文字层（hword / word_effect）可见性修复

> 结论：H 文字事件把字形条画出来之后，会立刻用一条「move」把同一图层 park 到 opacity 0。
> 这条 move 的淡出时长写在 **raw slot 2（WOLF 的“图片淡入/淡出帧数”）**里，且**没有**
> 显式动画尾巴（`argc == 11`，`cmd.animate` 为空）。旧 `Transition::from_command` 只读
> 显式尾巴，拿不到时长就返回 `None`，于是 move 在**画出的同一帧内瞬移到 opacity 0**，字形
> 一帧都没亮过（实测 max alpha = **0.000**）。修复后同一帧 alpha 到 **0.988**，并在 ~100 帧
> 内插值到 0，字形真正可见（渲染出 2581 个非黑字形像素）。

## 1. 现象与根因（line-level）

真实指令 —— CE350「文字エフェクトみぎ」（work/_ce_full.json）：

| pc | 指令 | 语义 |
|---|---|---|
| pc26 | `op150 [4096,4000,0,1,1,1,255,1600010,1600011,100,0]` + `"hword/word_effect38.png"` | 画 slot 4000，opacity 255，位置 self[10],self[11]，scale 100 |
| pc27 | `op150 [65537,4000,100,0,0,1,0,-10,10,150,0]` | move：raw[2]=100 = 100 帧淡出，目标 opacity 0、scale 150 |
| pc28 | `op180 [180]` | 等 180 帧（100 帧淡出必须在等待里跑完，否则这条等待没有意义） |

- `150` 长格式槽位（docs/OPCODE_SPEC.md §5.19）：raw[0]=type+cmd，raw[1]=图片编号，
  **raw[2]=未知/淡变时间（值域 0、15、5、30、100、10、20、3…… 正是帧数）**，
  raw[6]=不透明度，raw[7]/raw[8]=X/Y，raw[9]=缩放百分比，raw[13..]=动画参数（显式尾巴）。
  `PictureCmd.args` 从 raw arg2 起算，所以 `cmd.args[0] == raw[2]`，`cmd.animate == raw[13..]`。
- 旧代码：`crates/wolf-game/src/picture.rs` `Transition::from_command()` 仅
  `let frames = u32::try_from(*cmd.animate.first()?).ok()?;` —— 尾巴为空直接 `?` 返回 `None`。
- `Screen::apply` 的 `PictureAction::Move`（旧 ~1113-1154）：`None` 分支
  `picture.transition = None`，即把 `merge_args` 之后的最终字段（opacity 0）**直接落地**。
  又因 `merge_args` 把 raw[2](=100) 写进 `picture.fade`，而 `alpha() = opacity/255 * min(age/fade,1)`
  里 opacity 已经=0，alpha 恒 0。
- 结果：CE350 在 frame0 内依次执行 pc26→pc27→pc28，渲染时 slot 4000 已是 opacity 0。
  **旧行为实测：slot 4000 的 `hword/word_effect38.png` 在整个 200 帧运行中 max alpha = 0.000。**

## 2. 修复（最小改动，仅 picture.rs）

1. `Transition::from_command`（picture.rs:675）：显式尾巴优先（保持“作者写 0 / 未解析
   self 变量 = 故意瞬移”的旧语义，不回落）；**仅当尾巴为空、且本次 move 真的改变了 opacity**
   （`from.opacity != to.opacity`）时回落到 raw slot 2：
   `Some(frames) => u32::try_from(*frames).ok()?,` / `None => u32::try_from(*cmd.args.first()?).ok()?`。
   仍然 `frames == 0 || frames > MAX_TRANSITION_FRAMES(600) => None`。
2. `Screen::apply` 的 Move 分支（picture.rs:1164）：当 transition 成立时把
   `picture.fade = 0`。否则 `merge_args` 写入的 `fade = raw[2]` 会和 transition 的 opacity
   插值相乘（`alpha = opacity * min(age/fade,1)`），把每一帧都压暗。

保持不变的部分：
- Draw 仍用 raw[2] 作为淡入（`merge_args` 写 `picture.fade`），未改动。
- 带显式动画尾巴的 597 条 move 仍以 `animate[0]` 为时长，行为不变。
- raw[2]==0 或显式尾巴为 0/未解析 的 move 仍是一次瞬移（既有 3 条 teleport 单测继续绿）。

影响面：`150` cmd=1 move 全库 3684 条，其中带显式尾巴 597 条、无尾巴 3087 条。新逻辑只让
**无尾巴且改变 opacity** 的 move（约 1500 条，涵盖全部 word_effect park 淡出）在 raw[2]>0 时
按帧淡变；**纯位移/缩放（opacity 不变）的 move 保持原有瞬移，行为零改动**，故“普通图片 move”
语义完全保留。既有 picture 单测 55/55 全绿（见 §4）。

## 3. 前后对比（真实数据，SaveData08 + work/all）

| 指标 | 修复前 | 修复后 |
|---|---|---|
| EV350 是否被调度（var118>=1 后） | 是 | 是 |
| slot 4000 `hword/word_effect38.png` max alpha（200 帧） | **0.000** | **0.988 @frame0** |
| 亮起帧数（alpha>0.01） | 0 | **99** |
| 是否产生 sprite quad | — | 1 |
| 渲染 PNG 非黑字形像素 | 0 | **2581** |
| warnings | 0 | 0 |

基线对照：不设门（var118 保持存档值）时 EV350 不被调度，slot 4000 max alpha=0.000；
设 `vars.set(118,1)` 后自动事件同步调度 EV350，slot 4000 立刻可见。
（SaveData08 启动时只有 EV357 常驻，其余 6 个 H 文字事件需各自门变量≥1，详见
`docs/research/M7_HAUTO_EVENTS.md`。）

## 4. 回归测试与验证

测试：`crates/wolf-game/tests/m7_hword_visible.rs` —— `h_word_effect_move_fade_is_visible`
- 基线断言：不设门时 slot 4000 alpha 恒 0（旧行为即便设门也只是 0，故本测试在旧代码必红）。
- 设门后断言：EV350 被调度；slot 4000 alpha > 0.9；文件是 `word_effect`；亮帧 ≥30；
  至少 1 个 quad；并对 word 图层做 CPU 合成渲染写出
  `work/out/m7_hword_visible.png`，非黑字形像素 > 200（实测 2581，PNG 可见“ああ…”竖排字形）。
- 运行：`WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_hword_visible -- --nocapture`
  → `1 passed`，打印
  `scheduled=true max_alpha=0.988 @frame0 file="hword/word_effect38.png" frames_lit=99 word_quads=1 png_non_black=2581`。

单测：`cargo test -q -p wolf-game --lib picture` → **55 passed; 0 failed**。

全量：见下方“全量结果”（各测试用自带默认数据根，不设 WOLF_DATA_ROOT）。

> 全量结果（**不设 WOLF_DATA_ROOT，各测试用自带默认数据根**：H 测试默认 work/all，存档测试默认 work/full）：
> `CARGO_TARGET_DIR=work/out/target_ev1 cargo test -q -p wolf-game --no-fail-fast` →
> **46 个测试二进制，312 passed / 0 failed**（日志 work/out/ev1_fullsuite_default.log）。
> 其中包含本任务新增的 `m7_hword_visible`、`m7_hauto_events` 及 picture 模块 55 个单测，全绿。
>
> 配置注意（实测教训）：若全局强设 `WOLF_DATA_ROOT=work/all`，`m7_save_parts`（默认 work/full，需
> work/full 的 System.sav 作 base_save 模板）会因 base_save 为 None 而 panic —— 这是**数据根配置错配**，
> 不是 picture 改动导致；各测试必须用默认数据根分别运行。另外此前一轮曾见到 `m7_battle_playable`
> 的 battle 2000350 断言失败，属战斗子系统 WIP 的未跟踪测试，与 Picture/Screen 无关，本轮已随对端修复转绿。

## 5. 残余与边界

- 本修复让字形**可见且按帧淡出**，但字形内容是 hword/word_effect*.png 位图，最终成像质量
  取决于这些资源与锚点，未在本任务内逐一像素校验。
- CE367（切图眨眼，层 911，`4259841` 特殊效果）不是文字层，未纳入本测试；其可见性依赖
  独立 effect 路径。
- 其余 H 文字事件（350/351/353/354/357/358）各用自己的层号（4000/4100/9300/2200/4400/4300）
  与 raw[2] 淡出，均走同一修复路径；测试以 EV350 作为代表性证据。
