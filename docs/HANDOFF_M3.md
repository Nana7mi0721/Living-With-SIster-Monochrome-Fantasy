# M3 交接说明（关机前快照）

> 仓库：`E:\Study\github program\imoto life`
> 当前源码 HEAD：`be4ed7f M3: implement negative database column semantics`（本文档是紧随其后的文档提交）
> 状态：所有已完成的源码/文档均已提交，工作区只剩三个其他 agent 的未跟踪 scratch 文件；可以安全关机。

## 1. 一句话现状

M0（加载器/校验）与 M1（标题画面像素级还原）已完成，M2（开场可玩纵切片）基本完成，M3（完整运行时）已完成数据库、时间日程、地图审计、战斗/H 场景调研、图集扩容、地图 tile 层、按键注入、**151/160/162 画面渲染**、**战斗切片 A 通过 + 切片 B 的 CDataBase 越界列标签修复 + 切片 C 状态机可观测（带种子）**、**H 场景 P1/P2（capture --map 瓦片+图片合成、160/162 淡入淡出、word_effect 别名）**，并刷新 BASELINE。**M3 主体完成**；剩 CE242/CE243 战斗切片 C 的**菜单驱动入口**（CE65 指令菜单导航；状态机本身靠 `--var 2000495=2 --self 600=1` 直驱 CE242 已证明 `2000350` 0→1→2→0 且 `2000352` 写成 1）、清空临时表运行时语义，与 HSCENE §4.5 250 数据库项，属后续迭代。

## 2. 怎么跑

所有命令先进入工程目录并加载工具链：

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
cargo test -q --workspace          # 应全绿
cargo build -q -p wolf-cli
```

常用 CLI（`./target/debug/wolf-cli.exe`）：

```bash
# 格式/加载器回归（63 项）
./target/debug/wolf-cli.exe verify

# 开场纵切片（真实地图事件 0，2000 帧）
./target/debug/wolf-cli.exe capture --map-run SampleMapA --event 0 --frames 2000 \
  --answer 0 --answer-delay 2 --out work/out/m3_open_2000.png

# H 场景 P0（读零售存档 SaveData08）
./target/debug/wolf-cli.exe capture --root work/all \
  --load-save work/retail_backup/Save/SaveData08.sav --frames 60 --out work/out/m3_hscene.png

# 交互式跑一个地图事件或公共事件；注意是 --until，不是 --frames
./target/debug/wolf-cli.exe play --map-run SampleMapA --event 0 --until 600
./target/debug/wolf-cli.exe play --event 329 --until 3000
```

数据根：`work/full` 是测试用的精简数据根（`WOLF_DATA_ROOT`）；`work/all` 是 23 个 `.wolf` 全解包结果（4,085 个文件）。测试用 `target/test-scratch`，不要用系统临时目录。

## 3. 这轮已提交的内容（最近 14 个提交）

```
be4ed7f M3: implement negative database column semantics
e8e5dd8 M3: draw the map tile layer and inject one-frame key presses
9bffbad docs: correct the negative database row and column semantics
6425b5f M3: widen the atlas page and report packing failures
aa90f0b M3: time and schedule vertical slice
f65ab2c M3: treat opcode 104 as a no-op
e0ce754 M3: database row sentinels and runtime row counts
38a7e9e M3: play a map event or common event interactively
a386025 docs: H-scene subsystem audit and vertical-slice plan
5d07ace M3: transition state and screen colour opcodes
cd045dc docs: variable database runtime specification
b3e7d5c M3: capture --event starts a common event
e4de9d5 docs: battle subsystem audit and vertical-slice plan
027dad2 docs: map audit of all registered map events and pages
```

刚完成的负面数据库列语义（`be4ed7f`，`crates/wolf-game/src/db.rs` + `crates/wolf-game/src/game.rs`）：

- 读列 `-1` = 该类型第一个空行（`Databases::first_empty_row`，CE58 学技能 / CE143 放防具）。
- 写列 `-2` = 清空整行（`Databases::clear_row`，CE75 强制全回复 / 战斗复制事件）。
- 其他负列不再被夹到字段 0：返回空值并输出 `250 DatabaseAccess: ... negative column not modelled`。
- `Databases::num` 现在也支持 `Container::Sys` 的数值读取。
- 新增两个测试，`cargo test -q -p wolf-game --lib db::` 17 passed；全 workspace 绿色。

## 4. 代码地图（从这里找）

| 路径 | 职责 |
| --- | --- |
| `crates/wolf-format/` | `.wolf` DXArchive v8、Game.dat、CommonEvent.dat、.mps、TileSetData.dat、数据库全部纯 Rust 加载器；零第三方依赖 |
| `crates/wolf-vm/` | WOLF 字节码解释器；`exec.rs` 的 opcode match 是核心；`program.rs` 预编译为跳转表 |
| `crates/wolf-game/` | 游戏层：`game.rs`（Game/GameHost/frame）、`map_runtime.rs`（页面条件/并行事件）、`db.rs`（数据库/存档）、`picture.rs`、`tilemap.rs`、`choice.rs`、`window.rs`、`sysgraphic.rs`、`atlas.rs` |
| `crates/wolf-render/` | wgpu 30 + glyphon 文本、离屏截图、imgdiff 工具 |
| `crates/wolf-cli/` | 命令行：verify / precompile / run / list / capture / play |
| `docs/` | `FORMAT_SPEC.md`、`SAVE_SPEC.md`、`OPCODE_250.md`、`OPCODE_102.md`、`OPCODE_124.md`、`OPCODE_290.md`、`TIME_SYSTEM.md`、`MAP_AUDIT.md`、`BATTLE.md`、`HSCENE.md`、`M1_CLOSURE.md`、`ASSET_AUDIT.md`、`ARCHITECTURE.md` |
| `docs/BASELINE.json` | 反回归基线，改动解释器/加载器后必须刷新并重跑 `verify` |

## 5. M3 下一步（按优先级）

> **先读这条，避免走错方向：** Sys 变量 104/105 是 **SysDatabase type 6 第 104/105 行**的 `Ｘスクロール値` / `Ｙスクロール値`（magic `9000104` / `9000105`），是**引擎每帧输出的相机滚动值，不是 opcode**。全库没有任何脚本读写它们（扫描 550 个公共事件 + 9 张地图，`9000104`/`9000105` 命中 0 次），这正说明它们是引擎输出，**不能反推“相机必须由主人公位置推导”**。六张已注册地图都没有 page 图形，`SampleMapA` 为 64x36 格 = 1024x576，恰好一屏，因此 `tilemap.quads(camera=[0,0])` 目前是对的。`130` 是 Teleport（map,x,y,direction），不是移动；`281` 全库只有 3 处，都在 CE225，参数 `[3,0,0]` / `[2,0,0]` / `[3,0,0]`，是滚屏锁定/解除风格，没有真实位移。**不要**为了 104/105 新增 player/camera 居中模型；只有将来出现“大于屏幕且需要滚动”的地图时才重新评估。下面的“地图滚动”因此降为低优先级，建议先做第 2 项（151/160/162）或第 3 项（战斗切片）。

1. **地图传送状态（低优先级；不做相机跟随）**（`crates/wolf-game/src/game.rs` / `tilemap.rs`）：130 传送共 9 处（地图页 8 + CE42 pc118），全部用负地图 id。可以只记录 `(map, x, y, direction)` 供后续事件/传送使用，**不要**据此重算 camera；视差（SysDatabase type 13 + 地图行第 4 列）当前也没有真实使用点。
2. **151/160/162 的画面渲染（已完成）**（`crates/wolf-game/src/tint.rs` + `crates/wolf-game/src/game.rs`）：新增 `ScreenTint` 表示 `151 ChangeColor` 的整屏乘法（R/G/B 在位 16/8/0，各 /100；bit24 = 闪烁三角形；0 帧 = 立即生效）。`GameHost::change_color` 写入 `Game.screen_tint`，`Game::frame` 每帧 `tick()`；`tile_quads` 与新增的 `picture_quads`（CLI 的 `capture`/`play` 改用它，替代直接调 `atlas.sprites`）把倍率乘进每个 quad 的 `tint`，只作用于 number<0 的屏层图，HUD 不受影响。160/162 唯一实测调用 CE48 `[-1, 65546]` 是 kind -1 直切，故不画遮罩；`SystemFile/[Transition]Fade.png` 在 work/full 存在但当前不可达。验证：`tint.rs` 5 个单测 + `crates/wolf-game/tests/screen_tint.rs`（SampleMapA 地砖中性/染色/还原）+ 实机 `capture --map-run MapData/Dungeon.mps --event 1 --answer 0` 第 13 帧 tint=`[1.5, 2.0, 1.5]`（截图 work/out/dungeon_peak.png vs dungeon_neutral.png）。
3. **CE242/CE243 战斗调度器（进行中：切片 A 已通过 / B 标签修复已落地但仍卡 CE65 输入 / C 画面已达成）**：由变量 `2000350` 三态驱动，静态无调用者；唯一入口 CE239 戦闘呼び出し（1108 槽，104 个外部调用点）。按 docs/BATTLE.md 三阶段推进：
   - **切片 A（只读 CE189 数据视图）= 通过。** 新增 `wolf-cli dump` 子命令（驱动真实 `Game`/`Databases`，可选 `--var/--self/--db/--print-var/--print-self`）。`dump --event 189 --frames 20 --self 628=77 --db C:9..C:22` 证明 CE189 的 6 条 `250 row=-2`（行数写）**经标签解析**落到真实类型 10/11/12/13/14/15，六者 `row_count` 全部等于 SV28=77；`unanswered` 诊断 0。
   - **切片 B（CE188 单回合）= 标签修复已落地，仍卡输入。** `dump --event 188 --frames 2000 --var 2000350=2` 达 517917 指令、峰值 6 协程。诊断从 21 条/17 条 `unanswered` 降到 **5 条/1 条 `unanswered`**。**根因分析**：战斗脚本读的是一套比出货 `.dat` 更丰富的“战斗模板”表结构——`CData` type10「戦闘時一時ステータス」实为 40 行 16 列（无「装備中の武器/防具1..4」），`Data` type9「敵キャラ個体デ—タ」100 行 9 列（无「基本命中率/回避率/取得経験値/入手…」），`CData` type22「万能ウィンドウ」甚至 0 列；且类型标签 `×┏ 戦闘一時ステータス[基]`（`×`=禁用标记）与真实 type10 名称不符，`resolve_labels` 只能退回数字 10。**修复**：`crates/wolf-game/src/game.rs` 的 `Game::database` 读路径在“越界列”上返回引擎默认空值（数字 0 / 字符串 ""）并计入新增 `GameStats.database_out_of_schema`，不再记为 `unanswered`；`fields>0` 是硬条件（否则 0 列的 type22 会把所有第 0 列读空，破坏行数测试）；行号越界仍按原设计判 `unanswered`（保留 `a_cleared_row_reads_as_unanswered`/`a_runtime_row_past_the_last_row...` 两测试语义）。单测 `a_column_past_the_field_count_reads_as_the_empty_cell` 通过，`wolf-game` 213 测试全绿。**剩余 1 条** `CData (5,16,0)` 是“行 16 越过 16 行”的行越界，属有意保留。**BATTLE.md §5 line 792“CE188 不需 242/243 即能走完一回合”经验证不成立**——它先卡在 CE65 玩家输入。
   - **切片 C（可玩）= 画面已达成、状态机未达成。** 现有代码 `capture --event 239 --frames 600` 已渲染 61.4% 非黑（战斗背景 + 3 名 battle_member + WindowBase 皮肤状态窗 + 战力条，见 work/out/battle.png）；但 `2000350` 的 0→1→2→0 循环未发生（CE239 自身也停在开场输入）。
   - **已落地**：`crates/wolf-game/src/auto_events.rs` 新增 `GATED_CALL_ONLY = [242, 243]`，把这两条 `run_condition==0` 但由变量 `2000350` 门控的战斗事件并入边缘触发的条件调度器（开机 2000350=0 恒休眠）；跟踪数 96→98，含单测。
   - **按键注入已打通**：`--key-frame N:CODE` 原本只在 `capture` 里有，现补进 `dump`（并新增 `dump --watch EVENT` 打印某事件每次 PC 变化）。实测 `dump --event 188 --var 2000350=2 --key-frame 80:10 --watch 65` 把 CE65 从 `pc 78`（万能ウィンドウ选择轮询）推进到 `pc 132`，证明按键注入确实能驱动 CE65。
   - **下一步（B/C 收口）**：`CDataBase.json` 缺失列标签问题已按“越界列→引擎默认空值”修好（见上）。仍有两块：① CE65 推进后需连续多次导航/确认走完指令菜单（曾进入 CE188↔CE188/CE194/CE122 高频循环，260 帧 3300 万指令仍未判胜负）；② 战斗进行到深处会读“被清空/重建的临时表”（如 `CData (10,0,3) 戦闘不能？`、`(10,0,6) SP`、`(11,0,17) キャラクタ番号`），这些是行号越界/表被 resize 到 0 的运行时语义（`exec.rs:2039` 读到 `None` 会保留旧值），需在完整战斗状态机里统一处理，非标签问题。做完这两块，`2000352` 与 `2000350` 循环才可观测。**切片 C 探查补充**：在真实 `Game` 里预置 `2000350=1` 能经 `auto_events` 边缘触发拉起 CE242（峰值协程 6→7、出现 CE242 独有的 `290 Effect`）；但 CE242（「コマンド入力」1438 指令）顶部 `201 [8]` SetMoveRoute 在无地图层时无法完成、且内含 13 处 `123 [1600030,144]` 按键轮询，无头推进需「带地图/战斗角色的可交互驱动」或输入脚本，详见 docs/BATTLE.md §5.3。`dump --var/--print-var` 现已同时接受扁平索引与 `2000350` 这类魔数地址。
   - **切片 C 状态机已打通（带种子；2026 迭代）**：CE242 整库**无静态调用者**（无 `210/211`→`500242`，也无脚本写 `242`），只能由 `GATED_CALL_ONLY` 门控拉起；其 self 组 6 入参 `1600000`/`1600030`（扁平 self 600/630）在重写运行时从未被填。用 `--var 2000495=2`（battle-active 标志，CE425 写 2）＋`--self 600=1`（模拟玩家已选第 1 条命令＝ユカリ応急処置）跑通 CE242 治疗分支；新增的 `--watch-var` 打出三态轨迹：`2000350` 0(f0)→1(f1,pc14)→0(f91,CE244 结算)→2(f94,pc208)→0(f95,CE241 pc628)，`2000352` 在 f91 由 CE244 pc315 写成 **1（胜）**。命令：`dump --event 242 --frames 400 --var 2000495=2 --self 600=1 --watch-var 2000350 --watch-var 2000352`（10082 指令、峰值 6、instance 正常结束）。**诚实边界**：不是菜单真选出来的回合（`2000495==2` 时菜单被跳过，`self600` 是种子）；真正从 CE239/CE188 主流程进 CE242 仍需接通战斗调用方或补菜单写 `1600000`。详见 docs/BATTLE.md §5.3。新增工具：`--watch-var ADDR`（变量变化打印）、`--press-every N:CODE`（dump+capture，可重复，满足 `123` 轮询）、`--auto-input`（每帧确认键）、capture 的 `--var/--self` 种子；`capture --event 242 --frames 95 --var 2000495=2 --self 600=1` 可在 `2000350==2` 那帧截 work/out/battle_m3.png（3.1% 非黑，两名 battle_member 角色）。`exec.rs` 的 `201` 本已是非阻塞软降级，无需改。
4. **H 场景切片（P0 已全部达成；P1/P2 已落地，提交 684d711）**：docs/HSCENE.md 的两个 P0 现在都已在代码里闭环。① 图集容量：`crates/wolf-game/src/atlas.rs:42` `ATLAS_SIZE=4096` + `miss/take_warnings` 把放不下的图记成 `atlas full:` 诊断（不再静默丢）；同一 `SaveData08.sav` 在 `--root work/all` 下现在 **89.4% 非黑**，脸部三层（眼/嘴/眉）与动作层全部进 quad——旧 `work/out/_m3_sd08_all.png` 是没眼没嘴的空白脸，新 `work/out/_m3_sd08_all2.png` 表情完整。② 按键注入：`capture`/`dump` 都支持 `--key-frame N:CODE`（`key.rs::bits_for_code`），能把 `123 [1600030,17]` 轮询推过去。**P1/P2 已实现**：③ 瓦片+图片层合成——`capture --map` 改为驱动真实 `Game`（`start_pages` 装 tile 层并启动自动页，`--pointer/--press-frame/--key-frame/--load-save/--answer`），与 `capture` 共用 `drive_capture_frames/build_capture_frame/render_capture_frame`；`SampleMapA` ev0 是开场演出，`--frames 10` 可见米色 tile 层压在 eyecatch 图片下（98.3% 非黑，work/out/hscene_room_f10.png）。④ 160/162 遮罩——新增 `crates/wolf-game/src/transition.rs` `ScreenTransition`（kind<0 直切；kind>=0 全屏淡入淡出，折叠进 `apply_screen_tint`，两条渲染路径都生效），4 单测；出货只有 CE48 直切，用合成 capture 验证亮度 1.0→0.4→0.0。⑤ word_effect 前缀——`assets.rs` 窄别名 `Picture/word_effect* -> hword/word_effect*`（30 张含 7 张原本缺的都解析到真图），记 `asset fallback:` 诊断，2 单测，docs/UNVERIFIED.md H-ASSET-1。**唯一待复核假设**：160/162 淡入淡出按对称三角实现，若 162 是单向应改 `brightness()=1-t`（代码注释已标）。剩下 250 数据库项（HSCENE §4.5）仍属 M3 之后。
5. **BASELINE 刷新 + M3 提交（已完成本项刷新）**：`docs/BASELINE.json` 新增 `screen_tint_151` / `screen_transition_160_162` / `battle_slice` / `h_scene_p1_p2` 四段，记录 151 染色、160/162 淡入淡出、战斗切片 A/B/C 与 H 场景 P1/P2 的实现状态与证据；map_runtime/opcode 统计因本轮无加载器/opcode 变更而保持原值。`verify` **PASS 63 项 0 pending**、`cargo test --workspace` 全绿（复核于战斗切片 C 观测落地之后）。战斗切片 C 的可观测证据（`dump --event 242 --frames 400 --var 2000495=2 --self 600=1 --watch-var 2000350 --watch-var 2000352` → `2000350` 0→1→2→0、`2000352=1`）与新工具 `--watch-var` / `--press-every` / `--auto-input` 已写入 `battle_slice.slice_c`。

## 6. 已知 M2 残留（不阻塞 M3）

- ✅ 官方存档回读（`SaveData01..08`）已逐字节 + 引擎级双向校验：`crates/wolf-save/tests/write.rs::every_save_round_trips_byte_for_byte` 证明 9 个文件 `parse -> to_bytes` 逐字节还原（含 `System.sav`）；新增 `crates/wolf-game/tests/save_roundtrip.rs::every_retail_save_survives_a_game_level_save_reload` 证明真实 `Game` 的 `load_into -> save_to -> 重新 load_into` 后图片表 / 800 普通变量 / 读档时间 / 选择锚点完全一致（8/8 槽通过）。过程中修掉一个真实缺陷：`savegame::picture_to_record` 曾把横向 scale `-100`（零售 SaveData08 的窗口图 1996/1998）`max(0)` 钳成 0，导致回读后缩放丢失 —— 现已保留符号（渲染侧仍 `max(0)` 不受影响），并加单测 `a_negative_horizontal_scale_survives_a_save_round_trip`。两份测试在无零售存档数据时自动跳过（数据受版权保护）。
- 101 消息窗口几何：换行/原点/行距已按零售实现并有测试；`docs/OPCODE_101.md` §10 曾误列的 C5/C6（把 `window.rs` 的 SKIN 改 SystemFile、CORNER 改 6）已撤回——逐像素实测两张 18×18 皮肤均为 3 px 环 + 12×12 底（18=3+12+3），`window.rs` 保持 `SKIN=SystemGraphic`、`CORNER=3`（102 选择窗用它，101 消息窗自带 SystemFile 皮肤经 150 图片传入）。见 docs/OPCODE_101.md §3.5/§10/§11 与 docs/OPCODE_102.md §6.1。
- CData row 100（战斗临时角色槽）仍是运行时策略，未定论。
- 290 闪烁数学已实现，type 3/7/8/9/10（shake/breath/pattern once/loop/pingpong）在 `crates/wolf-game/src/picture.rs` 均有实现分支；剩余只是缺零售逐帧视觉比对，不是缺代码。
- 125 AutoInput 只存储、不真正注入按键。
- 221 LoadGame 的 flag 站点未全部覆盖。

## 7. 环境/协作陷阱（血泪）

- `crates/wolf-vm/src/exec.rs` 是 **CRLF**；用 Python 打补丁时读文件要 `newline=''`，把所有 anchor 的 `\n` 转成 `\r\n`，否则 assert 静默失败。
- 任何 JSON/多行提交信息用 `git commit -F work/_commit_*.txt`；`-m` 里带转义会留下字面 `\n`。
- `cargo fmt -- <files>` 可能重排整个 workspace；只对精确文件跑 rustfmt，之后用 `git diff --stat` 复核。
- 交互/截图产物一律写 `work/out/`，不要用 `/tmp`（Windows 下 `PermissionDenied`）。
- 不要用 shell 启动 Godot（本任务不涉及）；不要杀用户进程。
- 团队 8 人已全部 inactive，任务板所有任务 completed；恢复工作时新建任务再派发。

## 8. 恢复后的第一条命令

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
git status --short
git log --oneline -3
cargo test -q --workspace
```

若工作区只有 `crates/wolf-format/examples/opscan.rs`、`crates/wolf-format/tests/_scratch_cdata.rs`、`crates/wolf-format/tests/_scratch_event.rs` 三个未跟踪文件，直接忽略（可删）；它们不是交付物。
