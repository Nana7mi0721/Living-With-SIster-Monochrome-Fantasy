# 新会话启动提示（复制即可）

> 用途：本文件给“下一次新开会话”的 coding agent 用。新会话没有旧聊天记录，读完本文件 + `docs/README.md`（文档总索引）+ `docs/HANDOFF_M11.md`（最新交接，覆盖 M11 三批）即可接续；`HANDOFF_M3`–`HANDOFF_M10` 已标为历史归档，需要追溯时从索引进入。
> 当前仓库：`E:\Study\github program\imoto life`
> 当前源码 HEAD：以 `git log` 为准（M10 交接见 `docs/HANDOFF_M10.md`；M11 三批分别见 `22de104`、`5ef6b1f` 与本批提交）。

## 直接复制给新会话的提示词

```text
你在继续一个“用 Rust 重写 WOLF RPG Editor 游戏”的项目，仓库在 E:\Study\github program\imoto life。

先做三件事，不要凭记忆猜：
1. 读 docs/README.md（文档总索引，先看状态图例与推荐阅读路径）、docs/HANDOFF_M11.md（最新交接：M11 三批的成果、边界、下一步）和 docs/NEW_SESSION_START.md（本文件）；历史交接只在需要追溯时从索引进入。
2. 在仓库根目录执行：
   cd '/e/Study/github program/imoto life'
   source tools/env.sh >/dev/null 2>&1
   git status --short
   git log --oneline -5
   cargo test -q --workspace
3. 确认当前 HEAD 与测试全绿后，按 docs/HANDOFF_M11.md 的“边界/下一步”继续。M0–M10 均已完成收口；M11 三批已收口：第一批=战斗确定性（BTreeSet、逐字节轨迹）、8 向移动/NPC contact、锚点 5/6 几何、多成员命令序列、H 真实 gate 矩阵、音频 loop 档案；round 1=CE157 菜单确认采集/命令队列桥、var304 真实可达与 var326 书店 gate、窗口 CN patch 右缘 data-only、音频 loop 回归；round 2=默认武装的真实战斗驱动（自然 day4 无需测试开关即结算）、CE157 物理 cursor 订正（CData type18 row73，无 VM bug）、2 成员 fixture 注册 15 行并驱动 4 个 CE242 body、真实 CE328→CE402/CE410 深链实证、NPC ActionKey+每 8 帧持续行走、5 场景视觉回归。下一优先是完整玩家 UI 多命令战斗验收、H 深链的自然日历/真实帧时序与像素入口、行走动画/NPC 全对话、零售像素与人工听感验收；不要重做已完成部分。

项目状态：M0（加载器/verify）、M1（标题画面像素级还原）、M2（开场纵切片）、M3（完整运行时）、M4（音频）、M5（存档层）、M6（主线贯通与日常循环）、M7（真实战斗/H 场景切片）、M8（day4 战斗结算→day5、H 前向链、地图角色层、模板存档、engine_state、Park-Miller 事件 RNG）、M9（锚点/图片槽位、负缩放、窗口皮肤、H 闭环、战斗多回合/多队员、移动与地图触发、事件 RNG seed 钩子、音频盘点）、M10（opcode111 引用语义落地与战斗重基线）、M11（战斗默认驱动、真实菜单/行走、H 深链实证、视觉/音频回归）均已完成并收口。当前不要重做已完成部分，也不要推倒现有架构。下一步优先项见 docs/HANDOFF_M11.md §6/§8。地图滚动按 Sys 104/105 的真实语义接相机参数，不要建 player/camera 居中模型。
工程约束：不要提交 crates/wolf-format/examples/opscan.rs、crates/wolf-format/tests/_scratch_cdata.rs、_scratch_event.rs（永久 scratch）；不提交 docs/gpt调研/ 与 .claude/；提交信息用 git commit -F work/_commit_*.txt；crates/wolf-vm/src/exec.rs 实测为 LF，改前核对；截图/临时产物写 work/out/。

先汇报你读到的状态和准备做的第一个具体改动，再动手。
```

## 新会话最需要知道的 6 个事实

1. **权威文档**：`docs/README.md` 是文档总索引；`docs/HANDOFF_M11.md` 是最新交接，`PLAN.md` 是当前路线图；格式看 `docs/FORMAT_SPEC.md`、`docs/SAVE_SPEC.md`；opcode 看 `docs/OPCODE_*.md`；地图/战斗/H 场景看 `docs/MAP_AUDIT.md`、`docs/BATTLE.md`、`docs/HSCENE.md`；研究底稿索引在 `docs/research/README.md`。旧里程碑交接与 `M1_CLOSURE.md`/`M2_SLICE.md` 仅作历史参考。
2. **数据位置**：`work/full` 是测试数据根；`work/all` 是全部 23 个 `.wolf` 的解包结果；游戏安装在 `E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`（文件夹名确实拼错）。
3. **构建命令**：`cargo test -q --workspace`；`cargo build -q -p wolf-cli`；CLI 在 `target/debug/wolf-cli.exe`；静态验收 `./target/debug/wolf-cli.exe verify`（预期 PASS 70，pending 0）。
4. **当前未完成重点**：战斗自然路线确定性已证（`started_events` 是 BTreeSet，5 次进程轨迹 md5 一致）；默认 auto-arm 已让自然 day4 战斗自行结算；真实 CE157 cursor 无 VM bug，物理 cursor 是 CData type18 row73，真实连续确认逐行推进；2 成员 fixture 注册 15 行并驱动 4 个不同 CE242 body。仍缺完整玩家 UI→CE157 多输入验收。H 深链已实证真实 CE328 控制流可经 pc986 进 CE402（pc0→511）并驱动 CE410（pc0→247），但仍是 calendar-consistent fixture + budget 观测，自然日历/真实帧时序与像素截图入口待补。NPC ActionKey + 每 8 帧持续行走已做；行走动画帧、NPC 全对话、传送起点一致性待做。零售像素对照、锚点 5/6 几何、音频人工听感、PRNG 零售时钟种子、打包/性能仍未完成。
5. **不要做的事**：不要用 shell 启 Godot；不要杀用户进程；不要把旧聊天当作权威（旧会话可能已压缩/摘要）；以仓库文档和 git 状态为准；不要把 Sys 104/105（引擎滚动输出）和 opcode 104/105 混为一谈，也不要为它们新增 player/camera 模型；不要为了 `cargo fmt --all --check` 重排未涉及的文件（全仓历史不是绿门禁）。
6. **完成任务后**：更新 `docs/HANDOFF_M11.md`（或新增下一里程碑交接），同步 `docs/BASELINE.json`（记录本批证据）、`PLAN.md` 与 `docs/README.md`，保证下一次仍能从新会话接续；里程碑完成后按 `AGENT.md` §6 清理临时文件/构建缓存/worktree。

## 恢复后的第一条命令

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
git status --short
git log --oneline -5
cargo test -q --workspace
```

工作区若只有三个未跟踪 scratch 文件，直接忽略；它们不是交付物。
