# M9 交接：锚点/图片槽位、H 场景闭环、战斗驱动、移动与地图触发

> 仓库：`E:\Study\github program\imoto life`
> 基线：本文件随 M9 集成提交入仓。M8 集成 HEAD / origin/master 均为 `bcbcfba`；M9 从该提交之后开始。
> 阅读顺序：本文件（M9 最新）→ `docs/HANDOFF_M8.md` → `docs/HANDOFF_M7.md` → `docs/HANDOFF_M6.md` → `docs/HANDOFF_M4.md` → `docs/HANDOFF_M3.md`。
> `docs/gpt调研/` 不是项目文件，不提交；与实测冲突时以本仓库代码/已验证文档为准。

## 1. 一句话现状

M0–M8 保持可交付；M9 已落地并机检：

- **W5-b 图片锚点/槽位**：`picture.rs::place` 实现 0..6 七种锚点（0 左上、1 中心、
  2 左下、3 右上、4 右下、5 上中、6 下中，7 回退左上）；`hover` 改为复用
  `picture_hit_box`/`picture_texture_extent`，命中框与真实绘制矩形一致（含锚点、scale、
  sprite-sheet cell）。原始长格式 slot[2] = 淡入帧数、slot[5] = 1-based 图集格号。
- **W5-b 负缩放镜像**：负 scale 表示水平镜像，不再被 clamp 成不可见；`Screen::quads` 用
  `mirrored_columns` 逐源列反向绘制，命中矩形不变。CE245 夜间图标 `scale=-100` 现在可见。
- **W5-b 窗口皮肤**：`SystemFile/WindowBase.png` 的 3px 九宫格在 `window.rs`/`atlas.rs`
  已按实测 18x18 皮肤切分；`m8_window_skin` 覆盖 667 个 op150 窗口站点、300x200 与多窗口。
- **W2 H 场景闭环**：`m8_hscene_loop` 从真实 SaveData08 导航到 stage13，按两次 end
  按钮确认 → CE345 teardown → CE222 存档 → CE217 时钟 → CE329 就寝 → day90→91 09:30；
  `m7_hscene_exit` 钉死真实退出链（KEY GATE = `vars.get_sys(12)==0`）；`m8_hscene_cg_unlock`
  用插入 hover code35 写 `var460=1` 打开 ev33 的真实 H CG 图集；`m8_hscene_cg_run`
  实测 ev33→CE333 渲染 20-21 个 H-CG 文件（89.4% 非黑）。深层 CE246/CE328/CE402 的
  `var157/var104/var105/var4/var102` gate 在现有 8 个零售存档中无单一可同时满足者。
- **W1 战斗驱动**：订正 CE242 arg0 = **命令/行动索引 1..8**（不是队员槽位）；四个行动点池
  2000345/346/347/348 由 CE239 播种为 5/4/3/2；`BattleWiring` 增加 opt-in
  `round_loop`（真实 CE241 turn-start 把 2000350 2→0）和 `set_command_schedule`；
  BOSS 路径 `m8_battle_boss_rounds` 实测 rounds=3、pools [5,4,3,2]→[3,3,3,2]、
  v352=1/v343=1。默认单发模式字节不变。真实 CE65 菜单只能注册 row1/index1
  （CE157 不推进 `CData(20,73)`），多输入菜单在shipped数据层不可达。
- **W3-c 移动/地图触发**：`MapRuntime` 增加 `Terrain`（从 TileSetData 三层 OR 出四向
  可通行位图）、`try_move_leader`/`move_leader`（先转向、越界/瓦片/事件占位阻挡、
  precise=tile*2）、`try_trigger`（ActionKey/PlayerTouch，EventTouch 已解码但不触发）。
  真实 `m9_full_route` 从标题一路自然跑到 day5 08:30：8855 帧、456004 指令、warnings 0。
- **W5-a 事件 RNG seed 钩子**：`rng.rs` 增加 `event_seed` 记录、`WOLF_EVENT_SEED`
  环境钩子与 `resolve_event_seed`；默认 `new/from_seed` 不读环境，序列不变。
- **W5-c 音频**：`audio_inventory` 7 测盘点 358 个音频（357 decode OK，1 个 26-byte
  malformed fmt 的 wav 由 `decode.rs` 严格窄范围 PCM fallback 修复）；`audio_edge_cases`
  5 测覆盖 truncated/garbage/missing。
- **M10 预备（未落地）**：opcode 111 的“值操作数按变量引用求值”补丁已存
  `work/out/opcode111_landed.patch`，但会让 15 个旧测试按正确语义变红，已 PARKED；
  `docs/research/M10_OPCODE_111_PLAN.md` 给出 Phase0/1/2/3 重推导计划。
  `exec.rs` 当前保持旧 literal 语义，未改。

## 2. 怎么跑 / 怎么验

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
cargo test -q --workspace            # 全绿（0 failed；有 1 ignored）
cargo build -q -p wolf-cli
./target/debug/wolf-cli.exe verify   # PASS — 70 check(s) matched baseline, 0 pending
```

M9 重点单跑：

```bash
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_anchor_slots
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hover_anchor
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_negative_scale
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_window_skin
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_loop
WOLF_DATA_ROOT=work/all WOLF_RETAIL_SAVE_DIR=work/retail_backup/Save cargo test -q -p wolf-game --test m7_hscene_exit
WOLF_DATA_ROOT=work/all WOLF_RETAIL_SAVE_DIR=work/retail_backup/Save cargo test -q -p wolf-game --test m8_hscene_cg_unlock
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_cg_run
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_battle_party
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_battle_boss_rounds
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_battle_live_bridge
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_battle_multi_input
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m9_battle_round_gap
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_movement
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_map_triggers
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m9_full_route -- --nocapture
cargo test -q -p wolf-vm --test prng_seed
cargo test -q -p wolf-audio
```

数据根：`work/all`（23 个 .wolf 全解包，H 场景/DLC 只在这里）、`work/full`（带
`Save/System.sav`，存档/读档测试用）。游戏安装：`E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`（目录名确实拼错）。

## 3. 本批交付与证据

| 任务 | 交付 / 证据 | 结论 |
| --- | --- | --- |
| task-47 锚点/槽位 | `src/picture.rs`；`tests/m8_anchor_slots.rs` 4 测；`docs/research/M9_ANCHOR_SLOTS.md` | 0..6 锚点；slot2=fade、slot5=cell；非角锚点 census 2=84/3=115/4=70/5=8/6=72/7=0 |
| task-55 hover 锚点 | `src/game.rs` `hover_picture`/`picture_hit_box`；`tests/m8_hover_anchor.rs` 5 测；`M9_HOVER_ANCHOR.md` | 命中框与绘制矩形一致；event251 图标仍可点 |
| task-60 负缩放 | `src/picture.rs` `is_mirrored`/`mirrored_columns`；`tests/m8_negative_scale.rs` 5 测；`M9_NEGATIVE_SCALE.md` | CE245 pic1220 -100 可见；逐列 quad |
| task-59 窗口皮肤 | `tests/m8_window_skin.rs` 5 测 + `m7_window_skin` 3 测；`M9_WINDOW_SKIN.md` | 18x18 皮肤 3px 框、300x200 九宫格、667 个窗口站点 |
| task-54 H 循环 | `tests/m8_hscene_loop.rs` 3 测；`M9_HSCENE_LOOP.md` | stage13→退出→CE329→day90→91 09:30，warnings0 |
| task-46 H 退出 | `tests/m7_hscene_exit.rs` 3 测；`M7_HSCENE_EXIT.md` | 真实 exit；KEY GATE sys12==0 |
| task-56 H CG 解锁 | `tests/m8_hscene_cg_unlock.rs` 5 测；`M9_HSCENE_CG_UNLOCK.md` | hover code35→var460=1→ev33 开图集 |
| task-61 H CG 运行 | `tests/m8_hscene_cg_run.rs` 4 测；`M9_HSCENE_CG_RUN.md`；`work/out/m9_hscene_cg_run.png` | ev33→CE333 渲染 20-21 个 H-CG，89.4% 非黑 |
| task-49 战斗多队员 | `src/battle_wiring.rs`；`tests/m8_battle_party.rs` 2 测；`M9_BATTLE_PARTY.md` | CE242 arg0=命令索引 1..8；cmd3 池->[5,3,3,2]、cmd6->[5,4,3,1] |
| task-57 BOSS 多回合 | `tests/m8_battle_boss_rounds.rs` 1 测；`M9_BATTLE_BOSS.md` | rounds=3 turn_starts=2，pools 5/4/3/2→3/3/3/2，v352=1/v343=1 |
| task-63 实时菜单桥 | `tests/m8_battle_live_bridge.rs` 2 测；`M9_BATTLE_LIVE_BRIDGE.md` | 真选強斬 code30→schedule[1]→结算；>1 index 菜单供不了 |
| task-67 多输入菜单 | `tests/m8_battle_multi_input.rs` 2 测；`M9_BATTLE_MULTI_INPUT.md` | CE157 不推进 CData(20,73)，shipped 只注册 row1 |
| task-68 回合缺口 | `tests/m9_battle_round_gap.rs` 2 测；`M9_BATTLE_ROUND_GAP.md` | 自然单回合结算合法；ce243/ce241=false 非缺陷 |
| task-50 战斗指令输入 | `tests/m8_battle_command_input.rs` 2 测；`M9_BATTLE_COMMAND_INPUT.md` | CE65 行全可选；选择写 CData type14 row1 col3 |
| task-52 移动 | `src/map_runtime.rs` Terra/移动；`tests/m8_movement.rs` 6 测；`M9_MOVEMENT.md` | 单格移动/转向/边界/瓦片/事件阻挡 |
| task-58 地图触发 | `src/map_runtime.rs` Trigger/Terrain；`tests/m8_map_triggers.rs` 5 测；`M9_MAP_TRIGGERS.md` | action14/autorun26/parallel90/player-touch4；event-touch 不触发 |
| task-64 完整路线 | `tests/m9_full_route.rs` 679 行 1 测；`M9_FULL_ROUTE.md` | 标题→day5 08:30，8855 帧、456004 指令、warnings0 |
| task-48 事件 seed | `crates/wolf-vm/src/rng.rs`；`tests/prng_seed.rs` 8 测；`M9_PRNG_SEED.md` | WOLF_EVENT_SEED 钩子；默认序列不变 |
| task-51 音频盘点 | `crates/wolf-audio/tests/audio_inventory.rs` 7 测；`M9_AUDIO_SMOKE.md` | 358 文件，357 decode；185 SoundCmd 184 resolve |
| task-65 音频边界 | `src/decode.rs` fallback；`tests/audio_edge_cases.rs` 5 测；`M9_AUDIO_EDGE.md` | 26-byte fmt wav 可解；坏数据不 panic |
| task-66 战斗夹具 | `tests/m10_battle_db_fixture.rs` 3 测；`M10_BATTLE_DB_FIXTURE.md` | CE190 重写 type18 row69；CE172 真实运行不中断 |
| task-45 UNVERIFIED 审计 | `docs/UNVERIFIED.md`；`M7_UNVERIFIED_AUDIT.md` | 9 项：解决 2、仍开 7；另列 6 真残余 |
| task-62 M10 计划 | `M10_OPCODE_111_PLAN.md` | 15 失败分类 + Phase0..3 重推导 |
| task-53 opcode 111 | `work/out/opcode111_landed.patch`；`M9_OPCODE_111_REGRESSION.md`；`M7_OPCODE_111.md` | PARKED；exec.rs 未改 |

## 4. 关键源码位置

| 位置 | 内容 |
| --- | --- |
| `crates/wolf-game/src/picture.rs` | 七锚点 `place`、`destination`、`is_mirrored`、`mirrored_columns` |
| `crates/wolf-game/src/game.rs` | `hover_picture`/`picture_hit_box`/`picture_texture_extent`、`try_move_leader`、`set_battle_round_loop`、`set_battle_command_schedule` |
| `crates/wolf-game/src/map_runtime.rs` | `Terrain`、`TriggerRequest`、`try_trigger`、`try_move_leader`/`move_leader` |
| `crates/wolf-game/src/battle_wiring.rs` | CE242 arg0=命令索引；`round_loop`；CE241 turn-start |
| `crates/wolf-vm/src/rng.rs` | `WOLF_EVENT_SEED`、`resolve_event_seed`、`event_seed()` |
| `crates/wolf-audio/src/decode.rs` | 严格窄范围 RIFF/WAVE PCM fallback |
| `crates/wolf-game/tests/m9_full_route.rs` | 标题→day5 真实路线 |
| `docs/research/M9_OPCODE_111_REGRESSION.md` | 15 个 opcode111 失败与重推导计划 |

## 5. 已确认边界 / 未完成

1. **opcode 111（PARKED）**：正确语义 = 值操作数按变量引用求值；裸补丁会让 15 个测试
   按真实战斗控制流变红（11 战斗 + 4 非战斗），因此未落地。补丁在
   `work/out/opcode111_landed.patch`；按 `M10_OPCODE_111_PLAN.md` 的 Phase0..3 逐项
   重推导后再落地。
2. **CE242 arg0 订正**：M8 的 `m8_workstreams.battle_settle` 与 HANDOFF_M8 中“arg0 =
   行动队员槽位”的说法已被 M9 task-49 订正为“命令/行动索引 1..8”；以本文件为准。
3. **多输入菜单**：shipped CE188→CE191→CE192→CE157 只注册一条命令（CE157 不推进
   `CData(20,73)`，CE194 循环 0 次）。多队员只能走 rebuilt round loop（真实 CE241
   turn-start），不能声称由真实菜单喂入 >1 条命令。
4. **H 深链 gate**：ev33→CE333 可真实开图集；CE246/CE328/CE402 需要
   `var157==13 & var104==0 & var105==0 & var4>=21 & var102>=80`，8 个零售存档无单一
   同时满足。`SaveData07 var4=19/var102=59`，`SaveData04 var104=1/var105=2`。
5. **窗口皮肤**：视觉只有右侧数据放置风险：CN patch 文本 #12501(715,115) 与
   #12500(855,200) 不嵌套，右缘 1155>1024；生产 window.rs/atlas.rs 未改。
6. **移动限制**：无 diagonal/连续走/动画/NPC；event collision box 单位未定，只阻
   当前 active-page 自身 tile；story map 全可走；无 player start 坐标。
7. **音频**：仍待人工听感；`BGM/kana.mp3` missing；1 个 malformed fmt 已由 fallback 解。
8. **PRNG**：生产 seed 钩子已有，但零售时钟种子捕捉/逐位对拍仍未证；默认 EVENT_SEED=1。
9. **锚点 5/6**：几何推断，无零售像素对照；镜像逐列 quad 比单 quad 贵。

## 6. 工程约束 / 协作陷阱

- 不提交 3 个永久 scratch：`crates/wolf-format/examples/opscan.rs`、
  `crates/wolf-format/tests/_scratch_cdata.rs`、`_scratch_event.rs`；`_m6_actor_probe.rs`
  同样是临时探测文件。`docs/gpt调研/` 与 `.claude/` 是用户资料/工具目录，不提交。
  根目录的 `nul` 是 shell 误建的空文件，提交前删除。
- 提交信息用 `git commit -F work/_commit_*.txt`；推送用
  `git -c http.sslBackend=openssl push origin master`。
- `crates/wolf-vm/src/exec.rs` 实测为 LF（工作树在 Windows 上显示 CRLF，提交时按仓库配置处理）。
- 截图/临时日志写 `work/out/`；不跑 `git clean`；不杀用户/编辑器进程。
- 多 agent 共用工作树：每人只写自己声明的文件范围；集成前 Lead 检查 `git status` /
  `git diff`，并跑全量测试 + verify。`cargo fmt --all --check` 在全仓历史上不是绿门禁，
  不要为了它重排未涉及的文件。

## 7. 下一步建议

1. **M10 opcode 111**：按 `M10_OPCODE_111_PLAN.md` 先做 Phase0 fixture、Phase1 阈值
   放宽，再把 eval_num 与战斗原子补丁一起重推导；每次用 `--no-fail-fast` 要求 0 failed。
2. **H 深链**：如果要做完整 CG 分支，构造一个满足 var157/104/105/4/102 gate 的合成
   SaveData 或按分支拆成多个测试，不要硬改真实存档。
3. **战斗 polish**：把 live menu 的 index 线程接到 `set_battle_command_schedule`（当前
   真实菜单只能给 1）；BOSS 多回合已有真实 CE241 驱动。
4. **音频**：人工听一遍 BGM/SE/DLC 循环点；补 `BGM/kana.mp3` 的来源调查。
5. **像素**：锚点 5/6 做零售画面对照；窗口皮肤做 CN patch 右缘数据放置修正。
