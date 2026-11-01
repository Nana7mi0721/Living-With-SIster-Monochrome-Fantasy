# 新会话启动提示（复制即可）

> 用途：本文件给“下一次新开会话”的 coding agent 用。新会话没有旧聊天记录，读完本文件 + `docs/README.md`（文档总索引）+ `docs/HANDOFF_M12.md`（最新交接，覆盖 M11 round 3、M12 零售范围审计、M12 round 1 与 day6 周末日程）即可接续；`HANDOFF_M3`–`HANDOFF_M11` 已标为历史归档，需要追溯时从索引进入。
> 当前仓库：`E:\Study\github program\imoto life`
> 当前源码 HEAD：以 `git log` 为准（M11 三批见 `22de104`、`5ef6b1f`、`fdeed07`；M11 round 3 见 M12 交接提交）。

## 直接复制给新会话的提示词

```text
你在继续一个“用 Rust 重写 WOLF RPG Editor 游戏”的项目，仓库在 E:\Study\github program\imoto life。

先做三件事，不要凭记忆猜：
1. 读 docs/README.md（文档总索引，先看状态图例与推荐阅读路径）、docs/HANDOFF_M12.md（最新交接：M11 round 3 的成果、M12 零售范围审计、边界、下一步）和 docs/NEW_SESSION_START.md（本文件）；历史交接只在需要追溯时从索引进入。
2. 在仓库根目录执行：
   cd '/e/Study/github program/imoto life'
   source tools/env.sh >/dev/null 2>&1
   git status --short
   git log --oneline -5
   cargo test -q --workspace
3. 确认当前 HEAD 与测试全绿后，按 docs/HANDOFF_M12.md 的“零售范围审计结论 / 下一步”继续。M0–M11 均已完成收口；M11 round 3（task-97..100）已收口：H 深链改为默认 200000 预算的自然帧时序（start_event(328) 到 CE402 maxpc509 / CE410 act≤3，warnings=0）；真实 2 成员菜单读 committed CData type14 → CE157 → 3 个 CE242 body 全通；PRNG 零售 GetTickCount 播种无 folding 已反汇编证实并新增 WolfRng::from_clock_ticks；新增 wolf-package 校验/组装与 FrameBudgetReport。M12 已做零售范围审计：本作是纯鼠标点触式（注册 6 张地图 115 页 graphic 全空，0 个 player/touch 页），方向键行走/碰撞/contact/ActionKey、触发 3/4、相机/281/201 路线/远景视差都只是引擎通用能力，不再新增也不再当缺口；H act4/5 无零售自然证据、3–4 人队伍数据不支持，也不计必做。M12 round 1（task-101..103）已完成：战斗取消/返回/连续确认验收（取消不登记、后续确认仍登记两行）、SampleMapA 6 个 confirm 页审计（5 个空页选择器 + 1 个 self0==99 调试页，事件格不是命中区）、8 个零售存档经 Game load→save 整文件逐字节一致。day6 周末日程也已打通：day5 工作日 → 周六 09:30 CE310 休日五按钮 → 12:00/15:00/17:00 三个活动时段 → 周日 09:30 同一菜单（`m12_day6_weekend.rs`）。剩余只有还原度（锚点 5/6 像素 diff、31 首 loop 人工听感）与 day8+ 日程；不要重做已完成部分。

项目状态：M0（加载器/verify）、M1（标题画面像素级还原）、M2（开场纵切片）、M3（完整运行时）、M4（音频）、M5（存档层）、M6（主线贯通与日常循环）、M7（真实战斗/H 场景切片）、M8（day4 战斗结算→day5、H 前向链、地图角色层、模板存档、engine_state、Park-Miller 事件 RNG）、M9（锚点/图片槽位、负缩放、窗口皮肤、H 闭环、战斗多回合/多队员、移动与地图触发、事件 RNG seed 钩子、音频盘点）、M10（opcode111 引用语义落地与战斗重基线）、M11（战斗默认驱动、真实菜单/行走、H 深链实证、视觉/音频回归、自然帧 H/真实菜单/PRNG 时钟/打包）、M12（零售范围审计 + round 1 战斗取消/confirm 页/存档逐字节 + day6 周末日程）均已完成并收口。当前不要重做已完成部分，也不要推倒现有架构。下一步优先项见 docs/HANDOFF_M12.md §5.4/§7。零售地图铺满 1024×576 viewport，不需要相机/滚动/视差；不要把 Sys 104/105、281、201 当作零售缺口。
工程约束：不要提交 crates/wolf-format/examples/opscan.rs、crates/wolf-format/tests/_scratch_cdata.rs、_scratch_event.rs（永久 scratch）；不提交 docs/gpt调研/ 与 .claude/；提交信息用 git commit -F work/_commit_*.txt；crates/wolf-vm/src/exec.rs 实测为 LF，改前核对；截图/临时产物写 work/out/。

先汇报你读到的状态和准备做的第一个具体改动，再动手。
```

## 新会话最需要知道的 6 个事实

1. **权威文档**：`docs/README.md` 是文档总索引；`docs/HANDOFF_M12.md` 是最新交接，`PLAN.md` 是当前路线图；格式看 `docs/FORMAT_SPEC.md`、`docs/SAVE_SPEC.md`；opcode 看 `docs/OPCODE_*.md`；地图/战斗/H 场景看 `docs/MAP_AUDIT.md`、`docs/BATTLE.md`、`docs/HSCENE.md`；研究底稿索引在 `docs/research/README.md`。旧里程碑交接与 `M1_CLOSURE.md`/`M2_SLICE.md` 仅作历史参考。
2. **数据位置**：`work/full` 是测试数据根；`work/all` 是全部 23 个 `.wolf` 的解包结果；游戏安装在 `E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`（文件夹名确实拼错）。
3. **构建命令**：`cargo test -q --workspace`；`cargo build -q -p wolf-cli`；CLI 在 `target/debug/wolf-cli.exe`；静态验收 `./target/debug/wolf-cli.exe verify --root work/all`（预期 PASS 70，pending 0）；发布校验可用 `cargo run -q -p wolf-cli --bin wolf-package -- validate work/all`。
4. **当前未完成重点**：只做还原度与日程边界。战斗 2 成员真实菜单已通、取消/返回/连续确认已验收（`m12_battle_cancel.rs`）；confirm 页已审计为「5 个空页选择器 + 1 个 self0==99 调试页，事件格不是命中区」（`m12_confirm_pages.rs`）；存档 8/8 经 Game load→save 整文件逐字节一致（`m12_save_full_compare.rs`）；day5→day7 周末日程已通关（`m12_day6_weekend.rs`：周六 09:30、周日 09:00→09:30，CE310 五按钮与三个活动时段）。剩余：锚点 5/6 零售像素 diff、关键场景截图 diff、31 首 loop 人工听感、day8+ 自然日程；详见 HANDOFF_M12 §5.4/§7。**已确认非零售、不再作为缺口**：方向键行走/碰撞/8 向/contact/ActionKey、地图触发 3/4、相机滚动/281/201 路线播放/远景视差（引擎通用但零售不用）；H act4/5（无零售自然证据）；3–4 人队伍 UI（CData 只有 2 人）；PRNG 单 tick 重放、wolf-package 发布流水线、release 帧预算（工具/保真）。
5. **不要做的事**：不要用 shell 启 Godot；不要杀用户进程；不要把旧聊天当作权威（旧会话可能已压缩/摘要）；以仓库文档和 git 状态为准；不要把 Sys 104/105（引擎滚动输出）和 opcode 104/105 混为一谈；不要新增相机/滚动/视差/行走/路线播放等零售不用的引擎通用能力；不要为了 `cargo fmt --all --check` 重排未涉及的文件（全仓历史不是绿门禁）。
6. **完成任务后**：更新 `docs/HANDOFF_M12.md`（或新增下一里程碑交接），同步 `docs/BASELINE.json`（记录本批证据，已有 `m12_round1`）、`PLAN.md` 与 `docs/README.md`，保证下一次仍能从新会话接续；里程碑完成后按 `AGENT.md` §6 清理临时文件/构建缓存/worktree。

## 恢复后的第一条命令

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
git status --short
git log --oneline -5
cargo test -q --workspace
```

工作区若只有三个未跟踪 scratch 文件，直接忽略；它们不是交付物。
