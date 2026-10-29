# 新会话启动提示（复制即可）

> 用途：本文件给“下一次新开会话”的 coding agent 用。新会话没有旧聊天记录，读完本文件 + `docs/HANDOFF_M3.md` 即可接续。
> 当前仓库：`E:\Study\github program\imoto life`
> 当前源码 HEAD：`2abab9d`（`be4ed7f` 为最近一次代码提交）

## 直接复制给新会话的提示词

```text
你在继续一个“用 Rust 重写 WOLF RPG Editor 游戏”的项目，仓库在 E:\Study\github program\imoto life。

先做三件事，不要凭记忆猜：
1. 读 docs/HANDOFF_M3.md（关机交接快照，包含现状、代码地图、下一步和踩坑）和 docs/NEW_SESSION_START.md（本文件）。
2. 在仓库根目录执行：
   cd '/e/Study/github program/imoto life'
   source tools/env.sh >/dev/null 2>&1
   git status --short
   git log --oneline -5
   cargo test -q --workspace
3. 确认当前 HEAD 与测试全绿后，按 docs/HANDOFF_M3.md 的“M3 下一步”继续，优先级从“地图滚动/传送/视差 + 151/160/162 画面渲染”开始。

项目状态：M0（加载器/verify）与 M1（标题画面像素级还原）完成，M2（开场纵切片）基本完成，M3（完整运行时）进行中。当前不要重做 M0/M1，也不要推倒现有架构。
工程约束：不要提交 crates/wolf-format/examples/opscan.rs、crates/wolf-format/tests/_scratch_cdata.rs、_scratch_event.rs（其他 agent 的 scratch）；提交信息用 git commit -F work/_commit_*.txt；crates/wolf-vm/src/exec.rs 是 CRLF，打补丁要保留 CRLF；截图/临时产物写 work/out/。

先汇报你读到的状态和准备做的第一个具体改动，再动手。
```

## 新会话最需要知道的 6 个事实

1. **权威文档**：`docs/HANDOFF_M3.md` 是交接总览；格式看 `docs/FORMAT_SPEC.md`、`docs/SAVE_SPEC.md`；opcode 看 `docs/OPCODE_*.md`；地图/战斗/H 场景看 `docs/MAP_AUDIT.md`、`docs/BATTLE.md`、`docs/HSCENE.md`。
2. **数据位置**：`work/full` 是测试数据根；`work/all` 是全部 23 个 `.wolf` 的解包结果；游戏安装在 `E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`（文件夹名确实拼错）。
3. **构建命令**：`cargo test -q --workspace`；`cargo build -q -p wolf-cli`；CLI 在 `target/debug/wolf-cli.exe`。
4. **当前未完成重点**：地图滚动/传送/视差；`151/160/162` 的 host 渲染（`GameHost::change_color` 仍是 no-op）；CE242/CE243 战斗调度器；H 场景主纵切片；BASELINE 刷新。
5. **不要做的事**：不要用 shell 启 Godot；不要杀用户进程；不要把旧聊天当作权威（旧会话可能已压缩/摘要）；以仓库文档和 git 状态为准。
6. **完成任务后**：更新 `docs/HANDOFF_M3.md` 或在它旁边新增交接记录，保证下一次仍能从新会话接续。

## 恢复后的第一条命令

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
git status --short
git log --oneline -5
cargo test -q --workspace
```

工作区若只有三个未跟踪 scratch 文件，直接忽略；它们不是交付物。
