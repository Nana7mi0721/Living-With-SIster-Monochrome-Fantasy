# 新会话启动提示（复制即可）

> 用途：本文件给“下一次新开会话”的 coding agent 用。新会话没有旧聊天记录，读完本文件 + `docs/HANDOFF_M6.md`（最新交接）+ `docs/HANDOFF_M4.md` + `docs/HANDOFF_M3.md` 即可接续。
> 当前仓库：`E:\Study\github program\imoto life`
> 当前源码 HEAD：以 `git log` 为准（M6 收口见 `docs/HANDOFF_M6.md`；M4 音频后端+接线 `270ab37`；M3 收口 `12ecfb6`）。

## 直接复制给新会话的提示词

```text
你在继续一个“用 Rust 重写 WOLF RPG Editor 游戏”的项目，仓库在 E:\Study\github program\imoto life。

先做三件事，不要凭记忆猜：
1. 读 docs/HANDOFF_M6.md（最新交接：M6 主线贯通与可玩日常循环收口）、docs/HANDOFF_M4.md、docs/HANDOFF_M3.md 和 docs/NEW_SESSION_START.md（本文件）。
2. 在仓库根目录执行：
   cd '/e/Study/github program/imoto life'
   source tools/env.sh >/dev/null 2>&1
   git status --short
   git log --oneline -5
   cargo test -q --workspace
3. 确认当前 HEAD 与测试全绿后，按 docs/HANDOFF_M6.md 的“下一步”继续（M0–M6 已完成：标题→新游戏→卧室 HUD→读书/钻研→训练菜单→夜间菜单→CE329 就寝→次日 08:30 完整可玩日常循环；多日 day1→day4 08:30 已验证；day4 早上被剧情强制送入 CE253→CE239 战斗，因 CData type0 命令槽缺失导致 day5 经真实 day loop 不可达，已由 m6_week_loop.rs 锁定；M4 音频只差有设备时的人耳验收；M5 存档层已覆盖）。

项目状态：M0（加载器/verify）、M1（标题画面像素级还原）、M2（开场纵切片）、M3（完整运行时）、M4（音频）、M5（存档层）、M6（主线贯通与日常循环）均已完成并收口。当前不要重做已完成部分，也不要推倒现有架构。下一步优先项见 docs/research/M7_BACKLOG.md：战斗命令菜单、H 场景渲染、日程/任务系统、地图滚动/视差。地图滚动按 Sys 104/105 的真实语义接相机参数，不要建 player/camera 居中模型。
工程约束：不要提交 crates/wolf-format/examples/opscan.rs、crates/wolf-format/tests/_scratch_cdata.rs、_scratch_event.rs（永久 scratch）；不提交 docs/gpt调研/；提交信息用 git commit -F work/_commit_*.txt；crates/wolf-vm/src/exec.rs 实测为 LF，改前核对；截图/临时产物写 work/out/。

先汇报你读到的状态和准备做的第一个具体改动，再动手。
```

## 新会话最需要知道的 6 个事实

1. **权威文档**：`docs/HANDOFF_M6.md` 是最新交接；格式看 `docs/FORMAT_SPEC.md`、`docs/SAVE_SPEC.md`；opcode 看 `docs/OPCODE_*.md`；地图/战斗/H 场景看 `docs/MAP_AUDIT.md`、`docs/BATTLE.md`、`docs/HSCENE.md`；M7 待办看 `docs/research/M7_BACKLOG.md`。
2. **数据位置**：`work/full` 是测试数据根；`work/all` 是全部 23 个 `.wolf` 的解包结果；游戏安装在 `E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`（文件夹名确实拼错）。
3. **构建命令**：`cargo test -q --workspace`；`cargo build -q -p wolf-cli`；CLI 在 `target/debug/wolf-cli.exe`；静态验收 `./target/debug/wolf-cli.exe verify`（预期 PASS 70，pending 0）。
4. **当前未完成重点**：M7 工作流（战斗命令菜单、H 场景渲染、日程/任务、地图滚动）。M6 边界见 HANDOFF_M6 §4：day4 08:30 无训练菜单，直接 CE253→CE239 战斗；CE239 命令槽依赖 CData type0 行 26/25/47..54，而静态数据只有 17 行，CE26 静态调用者 0。
5. **不要做的事**：不要用 shell 启 Godot；不要杀用户进程；不要把旧聊天当作权威（旧会话可能已压缩/摘要）；以仓库文档和 git 状态为准；不要把 Sys 104/105（引擎滚动输出）和 opcode 104/105 混为一谈，也不要为它们新增 player/camera 模型。
6. **完成任务后**：更新 `docs/HANDOFF_M6.md`（或新增下一里程碑交接），同步 `docs/BASELINE.json`，保证下一次仍能从新会话接续。

## 恢复后的第一条命令

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
git status --short
git log --oneline -5
cargo test -q --workspace
```

工作区若只有三个未跟踪 scratch 文件，直接忽略；它们不是交付物。
