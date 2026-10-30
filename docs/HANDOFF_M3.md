# M3 交接说明（关机前快照）

> 仓库：`E:\Study\github program\imoto life`
> 当前源码 HEAD：`be4ed7f M3: implement negative database column semantics`（本文档是紧随其后的文档提交）
> 状态：所有已完成的源码/文档均已提交，工作区只剩三个其他 agent 的未跟踪 scratch 文件；可以安全关机。

## 1. 一句话现状

M0（加载器/校验）与 M1（标题画面像素级还原）已完成，M2（开场可玩纵切片）基本完成，M3（完整运行时）已完成数据库、时间日程、地图审计、战斗/H 场景调研、图集扩容、地图 tile 层与按键注入；**下一步从「地图滚动/传送/视差 + 151/160/162 的画面渲染」接续**。

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
2. **151/160/162 的画面渲染**（`crates/wolf-game/src/game.rs` 的 `GameHost::change_color` 仍是 no-op，transition 没画）：151 是整屏颜色乘法/闪烁（Dungeon ev1/ev3、SampleMapB ev5 有真实数据）；160/162 的过渡遮罩是 `SystemFile/[Transition]Fade.png`（640x480 8-bit 灰度），160 默认 CE48 `[-1, 65546]` 为无遮罩直切 10 帧。
3. **CE242/CE243 战斗调度器**：由变量 `2000350` 三态驱动，静态无调用者；唯一入口 CE239 戦闘呼び出し（1108 槽，104 个外部调用点）。先做 docs/BATTLE.md 的三阶段切片 A（只读 CE189 数据视图）→ B（CE188 单回合）→ C（可玩）。
4. **H 场景切片**：docs/HSCENE.md 的 P0 已达成（39 atlas images / 89.4% 非黑）；还需要 `--key-frame N:code` 注入 14 个 `123 [1600030,17]` 按键轮询，然后做主 H 纵切片。
5. **BASELINE 刷新 + M3 提交**：M3 收口时更新 `docs/BASELINE.json` 的 map_runtime/opcode 统计，重跑 `verify` 与 `cargo test --workspace`。

## 6. 已知 M2 残留（不阻塞 M3）

- 官方存档回读（`SaveData01..09`）尚未逐字节校验。
- 101 消息窗口几何（多行/换行）尚未完全按零售对齐。
- CData row 100（战斗临时角色槽）仍是运行时策略，未定论。
- 290 闪烁数学已实现，但 type 3/7/8/9/10 仍未确认。
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
