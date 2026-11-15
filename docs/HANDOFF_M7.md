# M7 交接：并行工作流第一批（战斗回合 / H 场景交互 / 零售 PRNG / 选择窗口）

> **历史证据（2026-11-14 冻结）**：Godot 阶段入口见 [`godot/README.md`](godot/README.md)；本文件不再追加“下一步”。
>
> 仓库：`E:\Study\github program\imoto life`
> 基线：本文件随 M7 集成提交一起入仓；上一提交为 `b5a2d56 M6: close the playable daily loop and pin the battle boundary`。以 `git log --oneline -3` 为准。
> 阅读顺序：本文件（M7 最新）→ `docs/HANDOFF_M6.md`（M0–M6 主线与日常循环）→ `docs/HANDOFF_M4.md` → `docs/HANDOFF_M3.md`。
> 外部调研（`docs/gpt调研/`）不是项目文件，不要提交；与实测冲突时一律以本仓库代码/已验证文档为准。

## 1. 一句话现状

M0–M6 保持可交付；M7 第一批并行工作流已落地并机检：

- **W5 零售 PRNG**：`crates/wolf-vm/src/rng.rs` 已从占位 xorshift32 换成零售 MSVC C LCG（`state = state*0x343FD + 0x269EC3 mod 2^32`，输出 `(state>>16) & 0x7FFF`，默认种子 22227）。同一随机序列现在可复现。
- **W1-a 真实战斗回合**：`m7_battle_round.rs` 证明 CE188 真实战斗线能完整启动、CE143/CE144 会复制 HP/maxHP、真实攻击行可选、CE157 目标窗可开；但 **CE244 结算在出厂事件图中不可达**（结构性边界，非测试缺口）。
- **W2-a H 场景交互推进**：`m7_hscene_advance.rs` 逐一用真实 `game.key_bits=OK` 跨越 H 事件全部 **14 个 opcode-123 轮询**（key=0 只到 retry pc，key=OK 走到 continue pc），warnings=0。
- **W5 选择窗口**：运行时选择窗口在 `b5a2d56` 已由 `choice::overlay_at_sized` + `wolf-cli` capture 路径绘制；本批补 `m7_choice_window.rs` 回归（2 测）与文档，`docs/UNVERIFIED.md` 的 H-MENU-1 旧说已纠正。

PRNG 更换使若干 M6 测试里“按旧 xorshift 偶然固定”的精确数字失效，本批已把它们改成按实际标签/语义断言（见 §4）。全量 `cargo test -q --workspace` 绿，`wolf-cli verify` PASS 70/0。

## 2. 怎么跑 / 怎么验

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
cargo test -q --workspace                 # 全绿
cargo build -q -p wolf-cli
./target/debug/wolf-cli.exe verify        # 预期 PASS 70, pending 0

# M7 新交付单跑：
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_battle_round
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_hscene_advance
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_choice_window
cargo test -q -p wolf-vm --test rng_retail
```

数据根：`work/all`（23 个 .wolf 全解包，H 场景文件只在 `work/all`）；`work/full` 是瘦身测试根。游戏安装：`E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`（目录名确实拼错）。

## 3. 本批交付与证据

| 工作流 | 交付 / 证据 | 结论 |
| --- | --- | --- |
| W5 零售 PRNG | `crates/wolf-vm/src/rng.rs`、`crates/wolf-vm/tests/rng_retail.rs`（11 测）、`docs/research/M7_PRNG.md` | 算法来自 `_wolfref/game_dat_random.c`：`RAND(a)=a*0x343fd+0x269ec3`、`(seed>>16)&0x7fff`；seed=1 复现 MSVC 序列 41,18467,6334,26500…；seed=22227 前 5=7086,1060,5553,18415,25972；range 归约 `(draw*(span+1))>>15` 标为 inferred |
| W1-a 真实战斗回合 | `crates/wolf-game/tests/m7_battle_round.rs`（1 测）、`docs/research/M7_BATTLE_ROUND.md` | A: max188pc=55, ce143=true, copyHP=100/100, ce152=true, ce192=true, target_win=true, real_attack_selectable=true, ce244=false, warns=0；B: battle_active=true, turn_set=true, v350_max=0, ce242=false, ce243=false, ce244=false, v352=0,v343=0, max239pc=857,warns=0 |
| W2-a H 场景交互 | `crates/wolf-game/tests/m7_hscene_advance.rs`（1 测）、`docs/research/M7_HSCENE_ADVANCE.md` | 14/14 opcode-123 polls advanced on key_bits=OK only；warnings=0；picture_quads=39；截图 `work/out/m7_hscene_advance_static.png` 89.4% 非黑 |
| W5 选择窗口 | `crates/wolf-game/tests/m7_choice_window.rs`（2 测）、`docs/research/M7_CHOICE_WINDOW.md`、`work/out/m7_choice_window.png` | `--event 246` 6 选项、anchor (512,192)、engine rect [512,192,122,208]；`--event 4` 无 choice、event42 离屏不画 |
| PRNG 回归修正 | `m6_mainline.rs`、`m6_training_menu.rs`、`time_system.rs`、`m6_ev1_page0.rs`、`m6_ev1_page2.rs`、`m6_night_menu.rs`、`m6_night_sleep.rs`、`m6_multiday.rs` | 见 §4；不再 pin 旧 PRNG 偶然值，改按标签/实际精灵/语义断言 |

### 3.1 关键源码位置

| 位置 | 内容 |
| --- | --- |
| `crates/wolf-vm/src/rng.rs` | 零售 MSVC C LCG；`DEFAULT_SEED=22227`；`from_game(a,_,_)` 用 random_a 播种 |
| `crates/wolf-vm/src/exec.rs:1672-1690` | opcode 121 的 0x6000 随机分支调用 `rng.range(a,b)` |
| `crates/wolf-game/tests/m7_battle_round.rs` | CE188 真实战斗线、CE239 2000350 死锁证据 |
| `crates/wolf-game/tests/m7_hscene_advance.rs` | 14 个 H poll 的 key_bits 因果测试 |
| `crates/wolf-game/src/choice.rs`、`crates/wolf-cli/src/main.rs` | 运行时选择窗口绘制路径（b5a2d56 已存在） |

## 4. PRNG 更换对 M6 测试的影响（重要）

旧的 `xorshift32` 让很多“偶然固定”的值看起来稳定；换成零售 PRNG 后下列断言已按真实语义重写，**不是删除验证**：

| 文件 / 测试 | 旧断言 | 现断言 |
| --- | --- | --- |
| `m6_mainline.rs::the_bedroom_sleep_menu_choice_rolls_over_to_a_new_day` | 首个按钮固定“学习会”var48=2、学习+8、体力-20 | 按 `first_label` 动态选择 防御练习(173,5,7)/秘密特训(174,6,9)/学习会(171,2,8)；体力只断言减少（防御练习实测 140→105） |
| `m6_training_menu.rs::every_training_option...` | 18:30→20:00（90 分钟） | 零售流实测 18:30→19:30；体力 -20/-20/-10 与属性 +2 仍精确 |
| `time_system.rs::sleeping_on_a_saturday_night...` | 固定 8088 指令 / 1396 帧 | 保留钟点/星期/警告语义，帧数/指令数改为合理区间（PRNG 抖动等待长度） |
| `m6_ev1_page0.rs`、`m6_ev1_page2.rs`、`m6_night_menu.rs`、`m6_night_sleep.rs` | 固定 picture 120 = imouto02/(240,340) | hover 实际妹妹精灵中心，断言 anchor=1 / 文件含 imouto |
| `m6_multiday.rs` | day2 首按钮固定学习会 var48=2 | 多日测试只断言日无关后果（17:00、同日、体力减少、真实标签）；逐命令效果归 `m6_mainline.rs` |

## 5. 已确认边界 / 未完成

1. **CE244 战斗结算在真实事件图中不可达**（W1 头号边界）：CE244 是 `2000352/2000343` 唯一写者，但 CE188 **没有** `210 CALL 500244`；只有 CE239/241/242/243 调它。CE239 需要 `2000350==1` 才进入命令态，而 `2000350` 又只由 CE242 pc14 写、CE242 自身需 `2000350==1`，形成 circular bootstrap；CE239 卡 pc857 等 `2000343`。直接 `start_event(239)` 无真实地图上下文会停 pc13。真实命令可用性仍卡 `CE152 pc89..pc95`（CData type7 row0 col0=0 → pc95 写 選択状態=0）；播种 type7 row0 col0=1 只证明“数据前置可使真实攻击可选”，不等于新游戏路径。
2. **H 场景只验证了 stage-selected polls**：`SampleMapA ev33 → CE333 → CE246/CE372/CE402` 的完整 wrapper chain 尚未端到端复现；14 个 poll 是按事件/阶段分别设种子验证的。word_effect / hword 时长语义仍未解。
3. **日程/主线**：day4 08:30 的 `CE253 地域貢献 → CE239` 墙仍在；W1 不解决，day5 经真实 day loop 仍不可达。
4. **地图滚动/视差**：Sys 104/105 是引擎相机输出，不要新增 player/camera 跟随模型；>1 屏地图出现前低优先级。
5. **M7 backlog 剩余**：见 `docs/research/M7_BACKLOG.md`，W1 战斗命令菜单、W2 完整 H wrapper、W3 日程、W4 地图滚动、W5 剩余 UI/文本/音频人耳。

## 6. 工程约束 / 协作陷阱

- 不要提交 3 个永久 scratch：`crates/wolf-format/examples/opscan.rs`、`crates/wolf-format/tests/_scratch_cdata.rs`、`crates/wolf-format/tests/_scratch_event.rs`。临时 example 提交前删除。
- `docs/gpt调研/` 是用户资料，不是项目文件，不提交。
- 提交信息用 `git commit -F work/_commit_*.txt`；推送用 `git -c http.sslBackend=openssl push origin master`。
- `crates/wolf-vm/src/exec.rs` 实测为 LF；修改前核对换行。
- 截图/临时日志写 `work/out/`；不跑 git clean；不杀用户/编辑器进程。
- 多 agent 共用工作树：每人只写自己声明的文件范围；集成前 Lead 检查 `git status` / `git diff`，并跑全量测试 + verify。
