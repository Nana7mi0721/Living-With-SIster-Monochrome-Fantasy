# M7 W5 · 运行时选择窗口在 headless capture 里可见（H-MENU-1）

日期：M7 W5 · 仓库 HEAD b5a2d56 · 归属任务 task-18

## 结论（先看这个）

运行时选择窗口（opcode 102 的灰色面板 + 逐项文字 + 光标）**在 HEAD b5a2d56 上已经实现并已合入**，
渲染路径是 `wolf-game/src/choice.rs` 的 `overlay_at_sized`，由 `wolf-cli/src/main.rs` 的
`build_capture_frame` 调用。实现来自提交 `7293ee8`（"M2: paint the choice window with the grey SystemFile skin"），
是 b5a2d56 的祖先。因此 `docs/UNVERIFIED.md` 里 H-MENU-1 写的"尚未绘制运行时选择窗口"是**过时的**描述。

本任务**不需要改引擎**。本次交付补的是：
1. 回归测试 `crates/wolf-game/tests/m7_choice_window.rs`（2 个用例）；
2. 一张真实 capture 证据 `work/out/m7_choice_window.png`（0 warnings）；
3. 本文档记录代码路径、capture 命令、截图路径与先前缺失之处。

## 代码路径

1. opcode 102 到达时，`wolf-game/src/game.rs` 把选项放进 `Game.choices: Option<Choices>`；
   `Choices { options: Vec<String>, frames: u32 }` 在 `game.rs` 定义。
2. 窗口锚点由系统变量决定：`Game::choice_anchor()` = `(vars.get_sys(3), vars.get_sys(4))`；
   字号由 `Game::font_px()`（Sys8，范围 8..=64，否则回退 `message::DEFAULT_FONT_PX`）。
3. capture 合成帧 `build_capture_frame`（`crates/wolf-cli/src/main.rs`，约 2058-2128 行）在 `game.choices` 为 Some 时：
   - 取 `options`、`choice_cursor()`、`choice_anchor()`、`font_px()`；
   - 调 `choice::overlay_at_sized(atlas, assets, options, cursor, x, y, font_px)`；
   - 把返回的窗口 nine-slice 精灵并入 sprite pass，逐项文字并入 text pass（与 101 文本/图片同一渲染路径）。
4. `choice::layout_at_sized`（`crates/wolf-game/src/choice.rs:177`）：options 为空返回 `None`；
   面板矩形只要与 1024x576 视口有任何相交就返回 `Some(layout)`，完全在视口外则 `None`（引擎此时本来就不画，
   标题事件 sys4=1000 正是靠这个"离屏裁剪"自然不画，无需特判）。
5. `choice::draw`（`choice.rs:210`）：先 `window::nine_slice` 出 9 片 `SystemFile/WindowBase.png` 皮肤，
   再为每个选项输出一个 `TextQuad`（选中行前缀 `▶ `）。

## 关键几何（本次实测）

- 事件 246 顶层 6 项菜单锚点 `(512, 192)`，`engine rect = [512.0, 192.0, 122.0, 208.0]`，6 行，全部落在 1024x576 内。
- 标题事件 42 锚点 `(512, 1000)`（屏幕高 576），`layout_at_sized` 返回 `None`：离屏，不画。

## 精确 capture 命令

```
cargo run -q -p wolf-cli -- --root work/all capture --event 246 --frames 120 --out work/out/m7_choice_window.png
```

实测输出（关键行）：

```
  entry     : common event 246
  choice    : HELD on prompt with 6 option(s) ["摸摸头", "揉肩", "拥抱", "亲吻", "挠痒痒", "性骚扰"], cursor 0 at anchor (512, 192)
            : engine rect [512.0, 192.0, 122.0, 208.0], 6 row(s)
  rendered  : 1024x576, 25372/589824 non-black pixel(s) = 4.3%
  wrote     : work/out/m7_choice_window.png
  stats     : ... 1 choice(s), 0 warning(s)
```

## 截图

- 路径：`work/out/m7_choice_window.png`（1024x576）
- 内容：中央偏上 `(512,192)` 一块灰色 nine-slice 面板，逐行显示 `▶ 摸摸头 / 揉肩 / 拥抱 / 亲吻 / 挠痒痒 / 性骚扰`，
  光标 `▶` 在第一行 —— 这就是 opcode 102 的运行时选择窗口。

## 为什么不用任务里示例的 `--event 4`、也不用标题 `--event 42`

- `--event 4`：事件 4 根本不打开选择肢，实测 `choice: none open`，渲染 0% 非黑（空白）。
- `--event 42`（标题）：它的 102 把窗口停在 `(512, 1000)`，y=1000 > 576，**按设计就不画**；
  所以标题截图里看不到 1/2/3 是对的，不是 bug。要拍"看得见的选择窗口"必须用锚点在屏内的事件。
- 事件 246 顶层菜单锚点 `(512,192)` 在屏内，且孤立在黑色背景上，最适合做验收截图。
  其它可用样本：事件 247（4 项，锚点 512,192）、事件 278（2 项，锚点 512,192）。

## 回归测试

文件：`crates/wolf-game/tests/m7_choice_window.rs`

用例 1 `a_live_prompt_paints_one_row_per_option_inside_the_screen`：
- 真 `Game`（work/all）→ `start_engine_events()` → `start_event(246)`，跑到 choices 打开；
- 断言 6 个选项文本、cursor 0、锚点 (512,192)；
- `layout_at_sized` 返回 Some，`lines.len() == options.len()`（一行一项），面板/每一行/光标矩形都在 1024x576 内；
- `overlay_at_sized` 返回非空 panel（nine-slice 皮肤）且 `rows.len() == options.len()`（一选项一文字 quad），选中行含 `▶`；
- 新 atlas 的 `take_warnings()` 为空，`game.stats.warnings == 0`。

用例 2 `no_choice_draws_nothing_and_the_title_anchor_is_clipped`：
- 新 `Game` 无选择肢 → `choices.is_none()`，`layout_at_sized(&[], ...) == None`；
- 标题事件 42 → 选项 `["1","2","3"]`、锚点 (512,1000)，`layout_at_sized` 返回 `None`（离屏不画），warnings 0。

运行命令与结果：

```
cargo test -q -p wolf-game --test m7_choice_window
# running 2 tests .. test result: ok. 2 passed; 0 failed
```

## 先前缺失 / 本次补了什么

- 库层渲染其实早已存在；缺的是"可复现的验收证据 + 回归"：没有测试钉住"一项一行、几何在屏内、离屏不画"，
  也缺一张能真正看到灰色面板的 capture（旧文档只说"看不到 1/2/3"，没给出能看到窗口的正确事件）。
- 本次补上测试、截图与本文档，并澄清 H-MENU-1 描述已过时。**无需任何引擎/源码改动。**

## 与其它交付不冲突

- 只新增 `crates/wolf-game/tests/m7_choice_window.rs` 与本文档，生成 `work/out/m7_choice_window.png`；
- 未触碰既有源码、battle 测试、`rng.rs`、`docs/BASELINE.json`、`docs/HANDOFF_M4.md` 及 3 个永久 scratch 文件；未 commit/push。
