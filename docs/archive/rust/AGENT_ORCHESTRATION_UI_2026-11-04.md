# 子代理任务编排 — UI/交互修复（2026-11-04）

> **历史归档（2026-11-14）**：这是 Rust UI 修复阶段的编排记录，不再是当前协作流程。
> 当前阶段见 [`../../godot/README.md`](../../godot/README.md) 与根 [`../../../AGENT.md`](../../../AGENT.md)。

> 目的：把 `docs/UI_BUG_BACKLOG.md` 的 P0/P1/P2 问题拆成可并行、写范围不重叠的子代理任务，并让独立的 review 子代理先找根因、顺带挖未发现的 bug。
> 原则：**review 先读后写；fix 按文件所有权拆；Lead 只做集成、冲突裁决和最终验收。**

## 1. 角色与职责

| 角色 | 名字建议 | 类型 | 职责 |
| --- | --- | --- | --- |
| Lead | lead | 集成 | 创建任务、派发/等待、冲突裁决、全量测试、打包、最终报告 |
| R1 | `review-render-ui` | 只读 review | 审渲染/图层/窗口/文字度量，覆盖 UI-05/06/07/08/12/13，挖额外视觉 bug |
| R2 | `review-input-state` | 只读 review | 审输入路由/存读档/标题返回/选择 hover，覆盖 UI-01/02/03/04 |
| R3 | `review-game-events` | 只读 review | 审游戏状态/事件链/对话生命周期，覆盖 UI-09/10/11，挖额外状态 bug |
| F1 | `fix-frontend-input` | 实现 | 修 `play.rs` 的滚轮/右键/键位注入；P0 输入 |
| F2 | `fix-dialog-core` | 实现 | 修标题返回状态、选择 hover、消息框/名牌/选项文本、行动力、夜晚妹妹消失 |
| F3 | `fix-render-layer` | 实现 | 修黄绿闪、图层优先级、房间方针/公会面板尺寸/顺序 |

## 2. 写范围（硬边界）

> 同一文件只允许一个实现 agent 写；review agent 绝不改源码。

| 任务 | Owner | 写范围 |
| --- | --- | --- |
| T1 review-render | R1 | `work/out/reviews/review_render_ui.md`（只读源码，不写源码/测试） |
| T2 review-input | R2 | `work/out/reviews/review_input_state.md`（只读源码，不写源码/测试） |
| T3 review-game-events | R3 | `work/out/reviews/review_game_events.md`（只读源码，不写源码/测试） |
| T4 fix-frontend-input | F1 | `crates/wolf-cli/src/play.rs`、`crates/wolf-cli/tests/m12_ui_input.rs` |
| T5 fix-dialog-core | F2 | `crates/wolf-game/src/game.rs`、`message.rs`、`choice.rs`、`textpic.rs`、`db.rs`、`savegame.rs`、`crates/wolf-cli/src/main.rs`、`crates/wolf-game/tests/m12_ui_*.rs` |
| T6 fix-render-layer | F3 | `crates/wolf-game/src/picture.rs`、`atlas.rs`、`window.rs`、`crates/wolf-game/tests/m12_render_*.rs` |
| T7 integration | Lead | `release/`、`release/manifest.json`、`docs/`、最终测试/截图/打包 |

补充：
- 任何任务需要改 `crates/wolf-vm/src/exec.rs`、`auto_events.rs` 或不属于自己 scope 的文件，先在任务/消息里说明，由 Lead 裁决或统一修改。
- 不允许子代理修改 `release/`、manifest、用户真实存档目录；release 重建只由 Lead 做。

## 3. 执行波次

### Wave 1 — 三路独立 review（并行，只读）

三个 review 子代理用全新上下文（fresh），各自从 `docs/UI_BUG_BACKLOG.md` + `work/out/ui_bugs/` 出发，既要**验证已知 bug 的根因**，也要**主动找相邻模块的未发现 bug**。

每个 review 报告必须包含：

1. 结论：根因/可疑点，带 `file:line`；
2. 复现：命令、存档、帧号或截图路径；
3. 影响：对应 UI ID + 是否发现新 bug；
4. 建议：最小修改点、风险、建议测试；
5. 不做源码修改，不运行破坏性命令。

### Wave 2 — 三路实现（并行，按写范围）

- `fix-frontend-input` 先做 T4：UI-01/02（滚轮、右键=ESC），补 `m12_ui_input.rs` 单测。
- `fix-dialog-core` 做 T5：UI-03/04/06/07/08/09/10/11（标题返回、hover、消息框/名牌/选项、状态）。
- `fix-render-layer` 做 T6：UI-05/12/13（黄绿闪、图层/房间面板）。

三者互不写同一文件；F2 拥有 `game.rs`，F1 只改 `play.rs`，F3 只改渲染文件。跨文件接口变更先在任务说明里同步。

### Wave 3 — Lead 集成

1. `git diff` 审阅每个任务的改动，确认没有越界文件；
2. `cargo build -q --workspace`；
3. `WOLF_GAME_DIR=$PWD/work/retail_backup cargo test -q --workspace`；
4. `cargo test -q -p wolf-cli --test m11_visual_regression`；
5. 关键场景截图：标题、存读档、对话、选项、夜晚；
6. `cargo build --release -p wolf-cli` → `python work/tmp/upd_manifest.py` → `wolf-package check --package release`；
7. 把结果写回 `docs/UI_BUG_BACKLOG.md`（逐条标状态/证据）和最终交接。

## 4. 验收标准（按任务）

- T4：存档页滚轮可翻页；右键等同 ESC；`m12_ui_input.rs` 覆盖两种输入路径。
- T5：ESC 退出存档后标题三行菜单恢复且可操作；普通弹窗 hover 预选；消息框不提前消失；名牌不压正文；选项完整显示且解析 `\cdb[...]`；角色说话有名牌；行动力恢复正确；夜晚互动后妹妹仍在画面。
- T6：标题→开始咯→地图切换逐帧无黄/绿像素；19:30 房间方针/公会面板尺寸/层级正确；截图对照原版。
- T7：全量测试全绿；m11 视觉基线通过；package check OK；release 可双击。

## 5. 风险与并行纪律

1. **文件冲突**：`game.rs` 是热点，指定 F2 独占；其他 agent 需要改它时必须请求 Lead。
2. **测试互相干扰**：子代理只跑自己 crate/目标测试；全量测试由 Lead 在 Wave 3 统一跑。
3. **视觉 bug 不可只靠单测**：每个 UI bug 必须有前后截图或帧统计，不接受“代码看起来正确”。
4. **不把“记录”当“修好”**：任务完成需在 `docs/UI_BUG_BACKLOG.md` 更新状态，并附证据。
5. **Review 独立性**：review agent 不读 Lead 的修复合集，先独立给根因；汇总后再让实现 agent 参考。

## 6. 预计产出

- 三份独立 review：`work/out/reviews/review_render_ui.md`、`review_input_state.md`、`review_game_events.md`（含新发现 bug）
- 三组修复 diff + 新增/更新测试
- 更新后的 `docs/UI_BUG_BACKLOG.md`
- 新的 release 包与最终截图总表
