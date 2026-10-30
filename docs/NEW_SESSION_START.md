# 新会话启动提示（复制即可）

> 用途：本文件给“下一次新开会话”的 coding agent 用。新会话没有旧聊天记录，读完本文件 + `docs/HANDOFF_M4.md`（最新交接）+ `docs/HANDOFF_M3.md`（M3 收口）即可接续。
> 当前仓库：`E:\Study\github program\imoto life`
> 当前源码 HEAD：以 `git log` 为准（M4 音频后端+接线 `270ab37`；M3 收口 `12ecfb6`；M6 纵切片见 `docs/HANDOFF_M4.md` §4.6）

## 直接复制给新会话的提示词

```text
你在继续一个“用 Rust 重写 WOLF RPG Editor 游戏”的项目，仓库在 E:\Study\github program\imoto life。

先做三件事，不要凭记忆猜：
1. 读 docs/HANDOFF_M4.md（最新交接：M4 音频后端+接线，下一步）、docs/HANDOFF_M3.md（M3 收口快照）和 docs/NEW_SESSION_START.md（本文件）。
2. 在仓库根目录执行：
   cd '/e/Study/github program/imoto life'
   source tools/env.sh >/dev/null 2>&1
   git status --short
   git log --oneline -5
   cargo test -q --workspace
3. 确认当前 HEAD 与测试全绿后，按 docs/HANDOFF_M4.md 的“下一步”继续（M4 只差有设备时的人耳验收；M5 存档层已覆盖；M6 纵切片已达成——Map003 ev1 → SampleMapA 卧室 HUD + 开场声明、108 后台事件无死锁，见 §4.6；标题菜单→新游戏入口也已跑通（`capture --boot` 92.1%，地图切换清负号画面层，见 §4.5）；前两段可玩行动也已打通：点击卧室「读书」图标 → 「对各种事的理解加深了。」，点「钻研」→「自主训练提升各项能力。」，可连续触发（work/out/m6_first_action.png、m6_second_action.png），见 HANDOFF_M4 §4.6 末段；M6 剩余可做更长日常循环与存档整合）。M0–M3 已完成并收口（151/160/162 渲染、战斗切片 A/B/C、H 场景 P0/P1/P2、行越界读语义、BASELINE 刷新均已提交）。地图滚动仍是低优先级：Sys 104/105 是引擎输出的相机滚动值，不是 opcode，不要建 player/camera 居中模型。

项目状态：M0（加载器/verify）、M1（标题画面像素级还原）、M2（开场纵切片）、M3（完整运行时）均已完成并收口；M4（音频）后端+接线完成，只差有设备时的人耳验收。剩余大项是 M6 主线贯通。当前不要重做 M0–M3，也不要推倒现有架构。
工程约束：不要提交 crates/wolf-format/examples/opscan.rs、crates/wolf-format/tests/_scratch_cdata.rs、_scratch_event.rs（其他 agent 的 scratch）；提交信息用 git commit -F work/_commit_*.txt；crates/wolf-vm/src/exec.rs 是 CRLF，打补丁要保留 CRLF；截图/临时产物写 work/out/。

先汇报你读到的状态和准备做的第一个具体改动，再动手。
```

## 新会话最需要知道的 6 个事实

1. **权威文档**：`docs/HANDOFF_M3.md` 是交接总览；格式看 `docs/FORMAT_SPEC.md`、`docs/SAVE_SPEC.md`；opcode 看 `docs/OPCODE_*.md`；地图/战斗/H 场景看 `docs/MAP_AUDIT.md`、`docs/BATTLE.md`、`docs/HSCENE.md`。
2. **数据位置**：`work/full` 是测试数据根；`work/all` 是全部 23 个 `.wolf` 的解包结果；游戏安装在 `E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`（文件夹名确实拼错）。
3. **构建命令**：`cargo test -q --workspace`；`cargo build -q -p wolf-cli`；CLI 在 `target/debug/wolf-cli.exe`。
4. **当前未完成重点**：M4 人耳验收（headless 只能证明引擎装上、解码/混音单测过）；M6 主线贯通（纵切片已达成：`capture --map-run Map003 --event 1` 到卧室 HUD，108 并行事件无死锁；标题菜单入口 `capture --boot` 也已 92.1% 干净到卧室；前两段可玩行动已打通（读书→理解+、钻研→自主训练，可连续触发；work/out/m6_first_action.png、m6_second_action.png），剩余可做更长日常循环）；地图传送状态（低优先级，且不要做相机跟随——见 HANDOFF_M3 §5 开头的 104/105 说明）。
5. **不要做的事**：不要用 shell 启 Godot；不要杀用户进程；不要把旧聊天当作权威（旧会话可能已压缩/摘要）；以仓库文档和 git 状态为准；不要把 Sys 104/105（引擎滚动输出）和 opcode 104/105 混为一谈，也不要为它们新增 player/camera 模型。
6. **完成任务后**：更新 `docs/HANDOFF_M4.md`（或新增下一里程碑交接），保证下一次仍能从新会话接续。

## 恢复后的第一条命令

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
git status --short
git log --oneline -5
cargo test -q --workspace
```

工作区若只有三个未跟踪 scratch 文件，直接忽略；它们不是交付物。
