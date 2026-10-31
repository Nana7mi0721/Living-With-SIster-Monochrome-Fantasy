# 新会话启动提示（复制即可）

> 用途：本文件给“下一次新开会话”的 coding agent 用。新会话没有旧聊天记录，读完本文件 + `docs/README.md`（文档总索引）+ `docs/HANDOFF_M10.md`（最新交接，含 §8 M11 第一批收口）即可接续；`HANDOFF_M3`–`HANDOFF_M9` 已标为历史归档，需要追溯时从索引进入。
> 当前仓库：`E:\Study\github program\imoto life`
> 当前源码 HEAD：以 `git log` 为准（M10 交接见 `docs/HANDOFF_M10.md`）。M10 的两个提交为 `ce680b3`（残项）与 `3bbbb12`（opcode 111 落地）。

## 直接复制给新会话的提示词

```text
你在继续一个“用 Rust 重写 WOLF RPG Editor 游戏”的项目，仓库在 E:\Study\github program\imoto life。

先做三件事，不要凭记忆猜：
1. 读 docs/README.md（文档总索引，先看状态图例与推荐阅读路径）、docs/HANDOFF_M10.md（最新交接：M10 已完成 opcode 111 引用语义 + compound 修复、12 个战斗/H 测试重基线、存档 row counts、地图 contact、CE246→CE346、H 深链合成档、音频残留）和 docs/NEW_SESSION_START.md（本文件）；历史交接只在需要追溯时从索引进入。
2. 在仓库根目录执行：
   cd '/e/Study/github program/imoto life'
   source tools/env.sh >/dev/null 2>&1
   git status --short
   git log --oneline -5
   cargo test -q --workspace
3. 确认当前 HEAD 与测试全绿后，按 docs/HANDOFF_M10.md 的“边界/下一步”继续。M0–M9 均已完成收口；M10 已完成：opcode111 引用语义、compound 0x600/0x700/0x800 修复、12 个测试重基线、存档 row counts、地图 contact/player start、CE246→CE346、H 深链合成档、音频残留。M10 剩余边界项（M11 第一批）已收口：战斗确定性已证（BTreeSet、轨迹逐字节一致）、8 向移动/NPC contact、锚点 5/6 几何、多成员命令序列、H 真实 gate 矩阵、音频 loop 验收档案。下一优先是真实多命令菜单桥接、H 完整 CG 的真实进度条件、NPC 交互/行走动画与零售像素/人工听感验收；不要重做 M0–M10 已完成部分。

项目状态：M0（加载器/verify）、M1（标题画面像素级还原）、M2（开场纵切片）、M3（完整运行时）、M4（音频）、M5（存档层）、M6（主线贯通与日常循环）、M7（真实战斗/H 场景切片）、M8（day4 战斗结算→day5、H 前向链、地图角色层、模板存档、engine_state、Park-Miller 事件 RNG）、M9（锚点/图片槽位、负缩放、窗口皮肤、H 闭环、战斗多回合/多队员、移动与地图触发、事件 RNG seed 钩子、音频盘点）、M10（opcode111 引用语义落地与战斗重基线）均已完成并收口。当前不要重做已完成部分，也不要推倒现有架构。下一步优先项见 docs/HANDOFF_M10.md §7。地图滚动按 Sys 104/105 的真实语义接相机参数，不要建 player/camera 居中模型。
工程约束：不要提交 crates/wolf-format/examples/opscan.rs、crates/wolf-format/tests/_scratch_cdata.rs、_scratch_event.rs（永久 scratch）；不提交 docs/gpt调研/ 与 .claude/；提交信息用 git commit -F work/_commit_*.txt；crates/wolf-vm/src/exec.rs 实测为 LF，改前核对；截图/临时产物写 work/out/。

先汇报你读到的状态和准备做的第一个具体改动，再动手。
```

## 新会话最需要知道的 6 个事实

1. **权威文档**：`docs/README.md` 是文档总索引；`docs/HANDOFF_M10.md` 是最新交接，`PLAN.md` 是当前路线图；格式看 `docs/FORMAT_SPEC.md`、`docs/SAVE_SPEC.md`；opcode 看 `docs/OPCODE_*.md`；地图/战斗/H 场景看 `docs/MAP_AUDIT.md`、`docs/BATTLE.md`、`docs/HSCENE.md`；研究底稿索引在 `docs/research/README.md`。旧里程碑交接与 `M1_CLOSURE.md`/`M2_SLICE.md` 仅作历史参考。
2. **数据位置**：`work/full` 是测试数据根；`work/all` 是全部 23 个 `.wolf` 的解包结果；游戏安装在 `E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`（文件夹名确实拼错）。
3. **构建命令**：`cargo test -q --workspace`；`cargo build -q -p wolf-cli`；CLI 在 `target/debug/wolf-cli.exe`；静态验收 `./target/debug/wolf-cli.exe verify`（预期 PASS 70，pending 0）。
4. **当前未完成重点**：战斗自然路线已确定性复现（`started_events` 是 BTreeSet，非 HashSet），但真实多命令菜单仍只注册一条命令（多回合靠 rebuilt CE241 loop）；H 深链 CE402/CE410 仍仅合成存档可达，真实存档缺 var304/var326 内容进度；NPC 交互与行走动画未做（8 向移动/占位碰撞已做）；音频人工听感；锚点 5/6 零售像素对照；窗口皮肤 CN patch 右缘数据放置。PRNG 零售时钟种子重放等价性仍待证。
5. **不要做的事**：不要用 shell 启 Godot；不要杀用户进程；不要把旧聊天当作权威（旧会话可能已压缩/摘要）；以仓库文档和 git 状态为准；不要把 Sys 104/105（引擎滚动输出）和 opcode 104/105 混为一谈，也不要为它们新增 player/camera 模型；不要为了 `cargo fmt --all --check` 重排未涉及的文件（全仓历史不是绿门禁）。
6. **完成任务后**：更新 `docs/HANDOFF_M10.md`（或新增下一里程碑交接），同步 `docs/BASELINE.json`（记录本批证据）、`PLAN.md` 与 `docs/README.md`，保证下一次仍能从新会话接续；里程碑完成后按 `AGENT.md` §6 清理临时文件/构建缓存/worktree。

## 恢复后的第一条命令

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
git status --short
git log --oneline -5
cargo test -q --workspace
```

工作区若只有三个未跟踪 scratch 文件，直接忽略；它们不是交付物。
