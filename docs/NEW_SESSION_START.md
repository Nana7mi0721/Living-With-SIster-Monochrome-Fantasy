# 新会话启动提示（复制即可）

> 用途：本文件给“下一次新开会话”的 coding agent 用。新会话没有旧聊天记录，读完本文件 + `docs/HANDOFF_M9.md`（最新交接）+ `docs/HANDOFF_M8.md` + `docs/HANDOFF_M7.md` + `docs/HANDOFF_M6.md` + `docs/HANDOFF_M4.md` + `docs/HANDOFF_M3.md` 即可接续。
> 当前仓库：`E:\Study\github program\imoto life`
> 当前源码 HEAD：以 `git log` 为准（M9 交接见 `docs/HANDOFF_M9.md`；M8 见 `docs/HANDOFF_M8.md`；M7 见 `docs/HANDOFF_M7.md`；M6 收口见 `docs/HANDOFF_M6.md`）。M9 的改动随 M9 集成提交入本地 master；origin/master 可能停在 M8 的 `bcbcfba`，以实际 git 状态为准。

## 直接复制给新会话的提示词

```text
你在继续一个“用 Rust 重写 WOLF RPG Editor 游戏”的项目，仓库在 E:\Study\github program\imoto life。

先做三件事，不要凭记忆猜：
1. 读 docs/HANDOFF_M9.md（最新交接：M9 已完成锚点/图片槽位、负缩放镜像、窗口皮肤、H 场景闭环、战斗多回合/多队员驱动、移动与地图触发、事件 RNG seed 钩子、音频盘点；opcode111 正确语义补丁 PARKED 等 M10）、docs/HANDOFF_M8.md、docs/HANDOFF_M7.md、docs/HANDOFF_M6.md、docs/HANDOFF_M4.md、docs/HANDOFF_M3.md 和 docs/NEW_SESSION_START.md（本文件）。
2. 在仓库根目录执行：
   cd '/e/Study/github program/imoto life'
   source tools/env.sh >/dev/null 2>&1
   git status --short
   git log --oneline -5
   cargo test -q --workspace
3. 确认当前 HEAD 与测试全绿后，按 docs/HANDOFF_M9.md 的“边界/下一步”继续。M0–M8 均已完成收口；M9 已完成：七锚点/命中框、负缩放镜像、窗口皮肤回归、H 场景 exit/loop/CG 解锁与 CE333 图集、战斗多回合（真实 CE241）、移动/地图触发、WOLF_EVENT_SEED 钩子、音频边界解码。下一优先是 M10 opcode111 的 Phase0..3 重推导；不要直接把 work/out/opcode111_landed.patch 裸打到 HEAD。

项目状态：M0（加载器/verify）、M1（标题画面像素级还原）、M2（开场纵切片）、M3（完整运行时）、M4（音频）、M5（存档层）、M6（主线贯通与日常循环）、M7（真实战斗/H 场景切片）、M8（day4 战斗结算→day5、H 前向链、地图角色层、模板存档、engine_state、Park-Miller 事件 RNG）均已完成并收口。当前不要重做已完成部分，也不要推倒现有架构。下一步优先项见 docs/HANDOFF_M9.md §7 与 docs/research/M10_OPCODE_111_PLAN.md。地图滚动按 Sys 104/105 的真实语义接相机参数，不要建 player/camera 居中模型。
工程约束：不要提交 crates/wolf-format/examples/opscan.rs、crates/wolf-format/tests/_scratch_cdata.rs、_scratch_event.rs（永久 scratch）；不提交 docs/gpt调研/ 与 .claude/；提交信息用 git commit -F work/_commit_*.txt；crates/wolf-vm/src/exec.rs 实测为 LF，改前核对；截图/临时产物写 work/out/。

先汇报你读到的状态和准备做的第一个具体改动，再动手。
```

## 新会话最需要知道的 6 个事实

1. **权威文档**：`docs/HANDOFF_M9.md` 是最新交接（M8/M7/M6 历史见 HANDOFF_M8.md / HANDOFF_M7.md / HANDOFF_M6.md）；格式看 `docs/FORMAT_SPEC.md`、`docs/SAVE_SPEC.md`；opcode 看 `docs/OPCODE_*.md`；地图/战斗/H 场景看 `docs/MAP_AUDIT.md`、`docs/BATTLE.md`、`docs/HSCENE.md`；M9 研究成果在 `docs/research/M9_*.md`，M10 计划见 `docs/research/M10_OPCODE_111_PLAN.md`。
2. **数据位置**：`work/full` 是测试数据根；`work/all` 是全部 23 个 `.wolf` 的解包结果；游戏安装在 `E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`（文件夹名确实拼错）。
3. **构建命令**：`cargo test -q --workspace`；`cargo build -q -p wolf-cli`；CLI 在 `target/debug/wolf-cli.exe`；静态验收 `./target/debug/wolf-cli.exe verify`（预期 PASS 70，pending 0）。
4. **当前未完成重点**：M10 opcode111 值操作数引用语义（PARKED，补丁在 `work/out/opcode111_landed.patch`，需按 Phase0..3 重推导）；H 深链 gate 无单一零售存档可满足；多队员真实菜单只注册一条命令（多回合靠 rebuilt CE241 loop）；音频人工听感；锚点 5/6 像素对照；PRNG 零售时钟种子逐位对拍。
5. **不要做的事**：不要用 shell 启 Godot；不要杀用户进程；不要把旧聊天当作权威（旧会话可能已压缩/摘要）；以仓库文档和 git 状态为准；不要把 Sys 104/105（引擎滚动输出）和 opcode 104/105 混为一谈，也不要为它们新增 player/camera 模型；不要为了 `cargo fmt --all --check` 重排未涉及的文件（全仓历史不是绿门禁）。
6. **完成任务后**：更新 `docs/HANDOFF_M9.md`（或新增下一里程碑交接），同步 `docs/BASELINE.json`（`m9_workstreams` 已记录本批证据），保证下一次仍能从新会话接续。

## 恢复后的第一条命令

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
git status --short
git log --oneline -5
cargo test -q --workspace
```

工作区若只有三个未跟踪 scratch 文件，直接忽略；它们不是交付物。
