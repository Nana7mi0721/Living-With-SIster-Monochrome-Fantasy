# Godot 阶段交接：M1 纵向切片

> 状态：**已完成（业务闭环，2026-11-14）**
> 对应计划：`docs/godot/PLAN.md` §4
> 决策记录：`docs/godot/DECISIONS.md`
> 上一阶段：`docs/godot/M0_HANDOFF.md`

---

## 1. 状态

- **阶段/日期**：M1 纵向切片 / 2026-11-14
- **仓库 HEAD**：`8c65153318e49d77711287d2a0226abc3d748b80`（工作区包含 M1 未提交实现，HEAD 只代表基线）
- **Godot 版本/渲染器**：Godot 4.7.2-stable Mono Win64（`ed1daf0bf`），兼容渲染器
- **.NET SDK**：.NET SDK 8.0.428（`E:\Study\tools\dotnet8`）
- **数据根/夹具**：`work/all/`（BasicData、MapData、SystemFile）；`game_data/programs.json`；`work/all/MapData/SampleMapA_1.mps`
- **是否可复现**：**是**。`bash tools/godot_check.sh` 全 headless，末尾输出 `GODOT_GATE_OK`。

---

## 2. 本批交付

### C# 确定性核心（`godot/core_cs/`）

- `EventVm.cs`：事件实例调度、等待/唤醒、调用栈、循环块、Self 变量；CommonSelf 跨调用持久化、实例上限 4096、未知事件/未实现 opcode 诊断（StrictMode 可抛错）、`int.MinValue/-1` 安全除法；`RestoreInstance()` 支持按存档重建实例而不重放指令。
- `Variables.cs`：普通/字符串/系统变量与 Self 变量；快照、恢复、克隆。
- `WolfRng.cs`：MINSTD PRNG 与事件随机种子；`RestoreState()`。
- `ProgramLoader.cs`：`programs.json` 公共事件与地图事件加载；地图事件条件、trigger、features、collision_width/height。
- `MapRuntime.cs`：地图页选择（6 种比较运算、4 条件）、autorun/parallel 每帧启动、ActionKey、PlayerTouch 接触框、事件占用/0x08 穿透、地图状态快照/恢复。
- `SaveCodec.cs`：重制版 JSON 存档 `lwsmf.save` v1；变量、地图事件、VM 实例（pc/return/wait/loops/self）、CommonSelf、RNG、对话状态、立绘与音频轨；Self 槽位用稀疏 `num_map`/`str_map` 保存，schema 版本校验、`.bak` 轮换与坏档回退、临时文件 + fsync + 原子替换写入。地图 1 的实测评存档约 14.7 KB。
- `GameClock.cs`：镜像 `217 時刻表示` 的确定性时钟（`4` 时 / `5` 分 / `6` 星期 / `8` 周末标志 / `160` 日数），保留零售「单次 if 进位」缺陷（+130 分 → `13:70`）与 6200 文本格式。
- `LegacySaveImporter.cs`：零售 `.sav` 只读导入（M0 能力保留）。

### 表现层（`godot/presentation/` + `godot/presentation_gd/`）

- `presentation/` 只剩三座 C# 桥：`VmController.cs`（VM 的 Godot host、101/102 信号、Scripted 键码）、`GameSessionController.cs`（把 core `GameSession` 包给 GDScript）、`TerrainBridge.cs`（包装唯一 C# `Terrain`，供 MapLayer 查询）。
- `presentation_gd/`：`GameRoot.gd` 是生产入口（标题 → 新游戏 → 地图 → 消息/选择 → F5/F9），配合 `InputRouter.gd`、`PictureStage.gd`（9 级图层带）、`AssetDB.gd`（384 MB LRU）、`TextTokenizer.gd`、`MessageWindow.gd`、`ChoiceWindow.gd`、`Nameplate.gd`、`UiSkin.gd`、`AudioManager.gd`（BGM/BGS/SE 三轨、循环点、淡入淡出、存档快照）、`MapLayer.gd`（tile 色块 + 地形通行 + 点击/悬停）。
- 旧 `TitleScreen.cs`、`ScreenHostView.cs`、`ScreenHostSessionView.cs`、`MapView.cs`、`AssetCatalog.cs` 与 `ScreenshotTest` 已退役（§4.13）；主场景是 `res://scenes/GameRoot.tscn`。
- 八个 `tests_gd` 场景覆盖输入路由、图层顺序、资产预算、101 排版、立绘重放、GameRoot 冒烟、地图层与音频混音器。

### 测试与门槛

- `godot/tests/TitleTest.cs`：标题事件 CE42/549 调度。
- `godot/tests/NewGameTest.cs`：新游戏事件、teleport map=3 → map=1。
- `godot/tests/PresentationTest.cs`：418 VM 帧到地图 1，事件/实例/HUD 状态。
- `godot/tests/MapRuntimeTest.cs`：合成地图单测，51/51。
- `godot/tests/SaveLoadTest.cs`：98/98；消息中存档、选择中存档、分歧后读档、标题菜单读档、CommonSelf 往返、稀疏 Self、schema 拒绝、`.bak` 回退与 <50 KB 体积、立绘快照往返（含 slot 唯一性、非法 action、`..`/盘符路径拒绝）、Teleport 帧边界队列、`programs.json` 解析缓存复用、音频轨（通道唯一/音量/tempo/循环点/路径穿越/位置）校验。
- `godot/tests/MapMessageTest.cs`：14/14；事件 4 消息 → [訓練] 选择链。
- `godot/tests/OracleStepTraceTest.cs`：418 帧逐指令 trace，2461 steps（与 Rust 全等）。
- `godot/tests/TerrainTest.cs`：`map_tiles.json` 解码、`Terrain.MaskFor` 四向/quarter/autotile 合成；真实地图 blocked/solid = Dungeon 348/114、SampleMapA 0/0、SampleMapB 160/158，与 Rust `BASELINE.json` 一致。
- `godot/tests/ClockTest.cs`：11/11；`GameClock` 与真实 CE217 的 +130 分单 if 进位、跨午夜、周末标志、`7:05` 格式全部一致。
- `godot/tests/SleepSoakTest.cs`：61/61；30 次真实 `CE329 → CE317` 就寝链到第 31 天，逐日校验日数 +1、星期 +1、08:30/09:30 起床与周末标志。
- `godot/tests/SessionParityTest.cs`：core `GameSession` 跑到卧室后与冻结夹具 `oracle/fixtures/session_core_save_state.json` 逐字段相等（418 帧、play_frames/map/leader 全等），`SESSION_PARITY_TEST_OK checks=9`。
- `godot/tests_gd/`：InputRouter / LayerOrder / AssetDB / DialogDraw / PictureRoundtrip / GameRoot / MapLayer / AudioManager / PictureState / DayMenu / WeekendMenu 十一条 GDScript 门槛。
- `tools/godot_check.sh`：30 道门槛（默认 headless 30 步；`LWsmf_RENDER=1` 时第 17/18 步改用真实渲染器出图）。

---

## 3. 证据

| 证据 | 命令/路径 | 结果 |
| --- | --- | --- |
| 全门槛 | `bash tools/godot_check.sh` | 30/30，输出 `GODOT_GATE_OK` |
| 整路线 Oracle | `res://tests/RouteTraceTest.tscn` + `python oracle/diff_route.py` | `73,814/73,814`，`ROUTE_DIFF_OK`，save/load 状态回路通过（§4.12/§4.23） |
| C# 构建 | `DOTNET_ROOT=E:/Study/tools/dotnet8 /e/Study/tools/dotnet8/dotnet build godot/Lwsmf.csproj -v:minimal` | 0 warning / 0 error |
| 标题事件 | `res://tests/TitleTest.tscn` | `TITLE_TEST_OK`；frames=4000 titleSteps=127 keyPolls=362 |
| 新游戏链 | `res://tests/NewGameTest.tscn` | `NEW_GAME_TEST_OK`；program ニューゲーム処理 len=43 blocks=2；teleport map=1 |
| 纵向呈现 | `res://tests/PresentationTest.tscn` | `PRESENTATION_TEST_OK phase=bedroom frames=418`；地图 1 12 事件、17 页、AUTO 0、PAR 3、3 个活实例 |
| 地图运行时 | `res://tests/MapRuntimeTest.tscn` | `MAP_RUNTIME_TEST_OK 51/51` |
| 存读档续跑 | `res://tests/SaveLoadTest.tscn` | `SAVE_LOAD_TEST_OK 98/98`；消息/选择中存档、立绘快照往返并分歧后读档，同实例 id/pc/wait 恢复，CommonSelf 往返，schema/备份/体积/立绘路径穿越/`Pc` 越界检查，temp 文件不残留 |
| 地图消息/选择 | `res://tests/MapMessageTest.tscn` | `MAP_MESSAGE_TEST_OK 14/14 frames=418`；message=`嗯，接下来怎么办呢……`；choices=`[训练]` |
| Oracle trace（夹具） | `oracle/fixtures/ce42_reference_trace.json` + `TraceDiffTest` | 10/10 steps；负向 fixture 在 step 6 检出差异 |
| Oracle 逐指令对拍 | `oracle/fixtures/samplemapa_event0_418_rust_trace.json` + `OracleStepTraceTest` | `TRACE_DIFF_OK: 2461/2461 steps matched 100% (event_id=0)`；418 帧，Rust 与 C# 同格式 trace |
| 地形通行 | `res://tests/TerrainTest.tscn` | `TERRAIN_TEST_OK checks=32 blocked=348,0,160 solid=114,0,158` |
| 地图图层/点触 | `res://tests_gd/MapLayerTest.tscn` | `MAP_LAYER_TEST_OK checks=18`；三层 64×36、点击 (2,3)、Dungeon/SampleMapB 计数一致 |
| 音频三轨 | `res://tests_gd/AudioManagerTest.tscn` | `AUDIO_MANAGER_TEST_OK checks=32`；BGM/BGS/SE 独立、循环点、快照恢复、WAV 循环点帧→采样换算 |
| 时钟 | `res://tests/ClockTest.tscn` | `CLOCK_TEST_OK checks=11`；CE217 +130→13:70、跨午夜、周末标志 |
| 图片状态/指针 | `res://tests_gd/PictureStateTest.tscn` | `PICTURE_STATE_TEST_OK checks=37`；124 的 hover/existence/x/y/scale/text、屏幕层命中、左/右/中三态、真实 CE217 HUD 显示 `8:30` |
| 白天工作菜单 | `res://tests_gd/DayMenuTest.tscn` | `DAY_MENU_TEST_OK checks=13 command=1 hour=18 minute=30 energy=0`；CE251 从 pc1022 走真实图片 hover/点击、自动确认与工作命令 |
| 周末行动菜单 | `res://tests_gd/WeekendMenuTest.tscn` | `WEEKEND_MENU_TEST_OK checks=9 hour=15 energy=1`；CE310 点击“悠然自得”后休息分支结束事件、时钟推进、体力下降 |
| 30 天日程 | `res://tests/SleepSoakTest.tscn` | `SLEEP_SOAK_TEST_OK days=30 checks=61`；真实 CE329→CE317 就寝链 |
| core 存档夹具全等 | `res://tests/SessionParityTest.tscn` | `SESSION_PARITY_TEST_OK checks=10 frames=418 core=frozen`；与 `oracle/fixtures/session_core_save_state.json` 逐字段相等 |
| 数据库 250 | `res://tests/Smoke.tscn` | `DATABASE_TEST_OK name/labels/sentinels/overlay/vm-250`；`.project` 标签漂移、哨兵、覆盖层与 VM 读写均过 |
| 战斗命令切片 | `res://tests/Smoke.tscn` | `BATTLE_COMMAND_TEST_OK control_real=0 seeded_real=1`；CE188 空库存无真实攻击行、有道具可选，均到 CE192 且不生成 CE244 |
| 自动事件 98 | `res://tests/Smoke.tscn` | `AUTO_EVENTS_TEST_OK count=4 start=3 resume=2 gated=1`；CE38/216/217/242 的边沿/resume/停止，`SessionParityTest` 断言 `AutoEventCount==98` |
| 战斗 wiring | `res://tests/Smoke.tscn` | `BATTLE_WIRING_TEST_OK rounds=1 result=-1 ce242_pc=415`；CE239 真实回合经 CE242 命令、350/343/352 结算并返回 |
| 战斗目标选择 | `res://tests/Smoke.tscn` | `BATTLE_TARGET_TEST_OK selected=1 control_pc=49 enemy_hp=100`；CE190/CE175 坐标倍率、CE162 选 0 并退出，无敌人时停 pc49/50 |
| 技能目标入口 | `res://tests/Smoke.tscn` | `BATTLE_SKILL_TARGET_TEST_OK selected=1 no_target=1 no_enemy_pc=50`；CE159 尾段场景/效果目标门、CE162 由 pc247 生成并选中退出，目标 0 不生成、敌数 0 停 pc49/50 |
| H 阈值数据面 | `res://tests/Smoke.tscn` | `H_EVENT_GATE_TEST_OK gates=37 boundaries=59 data_only=1`；37 个 op111 真实载荷与 59 个阈值/多条件边界点，只验证数据面 |
| 第六轮 P2 收口 | `res://tests_gd/AudioManagerTest.tscn` / `PictureRoundtripTest.tscn` / `Smoke.tscn` | `AUDIO_MANAGER_TEST_OK checks=32`、`PICTURE_ROUNDTRIP_TEST_OK`、`H_EVENT_GATE_TEST_OK ... data_only=1`；滚轮 126 拦截、同文件流隔离、未知 soundType 告警、容器镜像与严格 effects 对拍 |
| 第七轮 P2 收口 | `res://tests_gd/GameRootTest.tscn` / `AudioManagerTest.tscn` / `Smoke.tscn` / `converter validate` | `GAME_ROOT_TEST_OK`（Generation+1、重开新游戏后 event 38 重启）、`AUDIO_MANAGER_TEST_OK checks=32`、`unknown sound type` 循环两次仅 1 条日志、`DATABASE_SCHEMA_AUDIT_OK fields=18 rows=44 findings=3 needs_retail=0`；零售 BasicData 四文件哈希与 `work/all` 全等（D-37） |
| 渲染核对 | `LWsmf_RENDER=1 bash tools/godot_check.sh` | `reports/dialog_draw.png`、`picture_roundtrip.png` 可查看；`GODOT_GATE_OK` |

---

## 4. 边界与未完成

### 4.1 已验证

- 标题 CE42 → 新游戏 ニューゲーム処理 → map 3 → map 1 的确定性链条；新游戏与读档都从同一入口进入地图。
- 地图事件页条件选择、autorun/parallel 自动启动、ActionKey 触发、PlayerTouch 接触框（含 `collision_width/height` 扩展）、active 页阻挡与 0x08 穿透。
- 101 消息与 102 选择在 map 1 事件 4 上真实联通；消息窗/选择窗在地图之上可见。
- JSON 存档包含变量四组、地图事件 Active/InstanceId/计数/Self、VM 实例 pc/return/wait/loops/self、RNG 四字段、当前消息或选项；读档不重放 101/102，不重启已恢复的页。
- 存档写入使用同目录临时文件 + `Flush(true)` + `File.Move(..., true)`；失败会清理 temp，旧档不被半写覆盖。

### 4.2 已知差异/未验证

- **Oracle 逐指令对拍**：SampleMapA 事件 0 的 418 帧 autorun 段 2461 steps 全等（428 步带 `vars`、330 步带 `effects`、10 步带 `str_vars`）；整路线标题→新游戏→map 1 的 1500 帧已冻结为 73,812 steps 全等（§4.12）。白天 101/102 消息/选择尚无整路线覆盖。
- 地图 tile 图层已导出并由 `MapLayer.gd` 渲染；MapChip 是 WOLF 通用示例素材，零售正片未使用，三层保留 16 色调色板占位；地形通行数据已接 `Terrain`（§4.9/§4.13/D-21）。
- 音频三轨已接 `AudioManager.gd`（BGM/BGS/SE、循环点、淡入淡出、存档快照）；图片只映射存在的 png/jpg。
- 选择窗位置是 1024x576 居中回退（`_choiceBox Position=(362,220)`），未按 Sys3/Sys4 锚点布局；原版离屏时还会返回无窗口。
- 消息控制码目前是“清洗显示”（跳过 E/c/f/sp 等），不是完整解释排版；字体仍用 Godot 默认。
- 只验证 map 1 / TitleMap 与事件 4；其余地图事件、550 CE 的完整内容、日程/日期/行动值、战斗/H/结局未进入 M1 范围。
- 存档只实现 slot 01（F5/F9 与标题第 2 行）；没有多槽 UI、没有云同步/备份轮换。
- 零售 `.sav` 仍不能恢复事件实例/pc/call_stack/wait（D-07），导入后走安全锚点；重制版存档不写零售格式。

### 4.3 M2 W1 审计修复（已完成 2026-11-14）

- **P0-01 CommonSelf**：`EventVm` 维护跨调用的 Self bank；页结束/停止/StopAll 写回，启动与 210 调用先用 bank seed 再覆盖参数；存档新增 `common_self`，新游戏清空。CE251/CE452 的跨调用计数不再归零。
- **P0-02/P0-03 诊断与熔断**：未实现 opcode 计入 `UnhandledOpcodeCount` 并记录（`StrictMode` 抛错）；未知事件与超过 `MaxInstances=4096` 的启动被拒绝并计数；210 对空 callee 只前进 pc，不阻塞调用者。
- **P0-04 存档可靠性**：`SaveCodec` 校验 schema 1..current；写前保留有效 `.bak`，主档损坏时回退 `.bak`；不再把坏档当成功。
- **P2-01 除法边界**：`BinaryOp` 0x3 与 `Compound` 0x400 的 `int.MinValue/-1` 返回 `int.MinValue`，不再抛 `OverflowException`。
- **P1-01/P1-07**：删除 `VmController.AutoProcess` 第二套时钟；`VmController` 作为冻结的 `VmBridge`（信号/查询清单见类注释），由 `GameRoot/GameSession` 的唯一 1/60s 时钟驱动；选择索引在末项 clamp。
- **稀疏 Self**：`EventSelfVars` 用 `num_map`/`str_map` 只存非空槽；地图 1 实测存档 14.7 KB（目标 <50 KB）。
- 证据：`bash tools/godot_check.sh` 12/12 `GODOT_GATE_OK`；Smoke 内 VmTest 新增 CommonSelf/未知事件/未实现 opcode/未知 210/实例上限/MinValue 6 项全过；SaveLoadTest 47/47。

---

### 4.4 M2 W2 Day-1 第二轮审计止血（已完成 2026-11-14）

- **W1-P0-01 存档语义**：`SaveCodec.FromJson` 内置校验（self-var 键仅 [0,1000)、vars/str_vars ≤65536、实例 ≤4096 且 id 唯一/父链无环且深度 ≤4096、Pc ≤10M、WaitKind 0..4、MapEvents ≤4096、dialogue/MapFile/SavedAt 上限）；非法档返回 null 并回退 `.bak`。`ReadOne` 增加 8 MiB 上限、严格 UTF-8、权限/编码异常捕获。
- **原子备份**：写入走 `path.tmp` + `Stream.Flush(true)` + `File.Move(overwrite)`；只有有效旧主档才轮换 `.bak`，坏主档不会覆盖最后的好备份；`SaveCodec.Exists` / `TitleScreen.HasSave` 认 `.bak`。
- **W1-P0-02/P0-03 opcode 覆盖**：新增 104/107/125/126/151/174/175/201/213/270/281/300/421/498/1000（201 解析 `move_route`，300 按 title 调用并解码参数）；`210` 目标只接受 500000..599999/600000..699999；default 记录 `FaultReason`，`StrictMode` 抛错；`MaxCallDepth=64`，超深 `210/300` 拒绝并计数，不再静默 pc++。
- **测试隔离与门禁**：`TitleScreen.SavePathOverride` 让 SaveLoadTest 只写临时目录并在结束时删除；Title/NewGame/Presentation/MapMessage/SaveLoad 五条链统一 `StrictMode=true` 并断言 `UnhandledOpcodeCount==0`、`!Faulted`；TitleScreen 在 VM `Faulted` 时拒绝存盘。
- 证据：`bash tools/godot_check.sh` 12/12 `GODOT_GATE_OK`；SaveLoadTest 65/65（负键/键 1000/重复 id/坏父链/未知 WaitKind/坏主档不覆盖 `.bak`）；VmTest 新增 201/270/300/125+126/1000/递归深度 6 组；构建 0 warning 0 error。

---

### 4.5 M2 W2 Day 2–5 GDScript 表现层与逐指令 Oracle 对拍（已完成 2026-11-14）

- **表现层迁移（GDScript，`godot/presentation_gd/`）**：`GameRoot.gd`（固定 1/60s 时钟，`MAX_STEPS_PER_FRAME=8`）接管 VmBridge 驱动；`InputRouter.gd` 统一输入（Z/Enter/KP_Enter/Space=决定，X/ESC/Backspace 与右键=取消，滚轮 ±1，仅 `InputEventMouseMotion` 更新 hover；掩码→按键码交给 C# `KeyCodes`，GDScript 侧不再保留镜像表，见 §4.7）。GDScript 无法直接赋值 C# `Func<uint,int>` 或读取自定义类型属性，`VmController` 增 `SetKeyCodeProvider(Callable)`、`GetSys(int)`、`SetSys(int,int)` 三个桥接方法。
- **9 级图层带（`PictureStage.gd`）**：`Map0 < ScenePicture10 < SceneText11 < MessageFrame20 < MessageText25 < Nameplate30 < ChoicePanel40 < ChoiceText41 < Cursor50`，每带独立 Control 且 `z_as_relative=false`；`draw_picture()` 支持 action 0 重建 / 1 移动 / 2 擦除 / 3 隐藏；backdrop（number<0 或 ≥12000）恒铺满 1024×576。
- **资产（`AssetDB.gd`）**：`work/all` 递归索引 + 精确 → 换扩展 → basename 三级回退；LRU 预算 384 MB（纹理按 `w*h*4` 计费，超额淘汰并计数）；音频支持 `.ogg/.mp3/.wav`。
- **101 自绘（`MessageWindow.gd` + `TextTokenizer.gd`）**：窗口 960×168 @ (64,408)、文字原点 (64,410)、字号 20、行距 26（行高 = 本行最大字号 + 6）、500 字/秒、暂停 0.25s、等待光标 (576,554)；`\f[n]` 是**字号**不是脸号；脸图来源 Sys0 或正文首行 `@n`，307×230 贴右下 (717,346)（下半被窗框覆盖）；`\c[n]`/`\cself[n]`/`\E`/`\>`/`\.`/`\^` 与 `<L>`/`<C>`/`<R>` 全部 token 化（`\E`=描边开启、`<L>/<C>/<R>`=行对齐），`to_source()` 字节级 lossless。
- **102 自绘（`ChoiceWindow.gd`）**：Sys3/Sys4 定位、行高 `ceil(font*1.35)`、悬停高亮、取消返回 -1；`Nameplate.gd` 九宫格底板 + 说话人文字，悬挂在 30 带。
- **测试**：`tests_gd/{InputRouterTest,LayerOrderTest,AssetDBTest,DialogDrawTest,PictureRoundtripTest,GameRootTest}.tscn`；DialogDrawTest 在 `LWsmf_RENDER=1` 时用真实渲染器出图 `reports/dialog_draw.png`（1024×576，两行不截断、无重叠、`\c[1]` 蓝色生效、右下脸图占位）；GameRootTest 覆盖 9 级图层带、8 个 VmBridge 信号连线和消息/选择窗开关。
- **逐指令 Oracle 对拍（Day 5 核心）**：Rust `Vm` 新增 `step_trace: Option<Vec<StepTraceEntry>>`（`frame/event/pc/op`，在 `execute()` 每访问一个 slot 记录，非 Command 记 op=0），`wolf-cli run` 新增 `--step-trace FILE` 输出 `wolf.trace` JSON；C# `OracleStepTraceTest` 注册 550 个公共事件后跑 `SampleMapA` 事件 0 共 418 帧，导出同格式 trace；`python oracle/diff_trace.py` 对拍 **`TRACE_DIFF_OK: 2461/2461 steps matched 100% (event_id=0)`**。参考夹具 `oracle/fixtures/samplemapa_event0_418_rust_trace.json`（196,256 B）由 Rust 实跑生成，不是手工样本。
- **存档立绘往返**：`SaveState.Pictures`（`SavedPicture`：number/kind/action/file/text/args/anchor；≤256 槽、action 0..4、anchor 0..255、slot 唯一），`ScreenHostView.PictureSnapshots()/RestorePictures()`；F5 存盘快照当前立绘，F9 读档在 `EnterMap` 之后重放（否则被 `ShowMap → ClearPictures` 清掉）。
- **立绘重建像素一致**：`tests_gd/PictureRoundtripTest` 把同一份绘制命令序列执行 → 清空 → 重放，用真实渲染器抓两帧并逐字节比较（1024×576 RGBA 完全相同），另断言清空后画面确实不同；`reports/picture_roundtrip.png` / `picture_roundtrip_replay.png` 留档。
- **门槛**：`bash tools/godot_check.sh` **20/20** `GODOT_GATE_OK`；`SAVE_LOAD_TEST_OK 87/87`、`MAP_MESSAGE_TEST_OK 14/14 frames=418`、`PICTURE_ROUNDTRIP_TEST_OK`、`ORACLE_STEP_TRACE_OK slots=846 steps=2461`、`TRACE_DIFF_OK 2461/2461`、6 个 GDScript 测试全绿。
- **边界**：参考 trace 只覆盖 SampleMapA 事件 0 的 418 帧 autorun 段（不含标题 CE42 → map 3 → map 1 的完整输入序列）；`vars/effects` 当时尚未入 trace，W3 Day-5 补齐（见 §4.9）。全路线冻结输入序列留到 W3。

---

### 4.6 M2 W3 Day-1 第三轮审阅 P1 修复（已完成 2026-11-14）

依据 `m2w2_round3_review_2026_11_14.md` §3 审计表与 §8 W3 Day 1 计划，修完 `GameRoot.gd` 的 4 项 P1 与 `\cdb`/存档防御缺口；全部为加法式改动，冻结的 `VmBridge` 签名未动。

| ID | 缺陷 | 修复 | 验证 |
| --- | --- | --- | --- |
| `W2-P1-01` | 鼠标悬停选项后点击仍选中第 0 项 | `GameRoot` 连接 `router.hover_changed` → `choice.hover_at()`；`ChoiceWindow.hover_at()` 命中行时同步 `selected` 并重绘；左键（MASK_OK）经 `InputRouter` 的 `_click_latch` 进入 `held_mask()`，确认时提交当前 `selected` | `GameRootTest`：悬停第 1 行 → `hovered_index()==1`/`selected==1` → 左键关窗 |
| `W2-P1-02` | 消息框按 ESC/右键只 `message.close()`，VM 卡在 `WaitState.Message` | `_on_cancel()` 关窗后统一走 `_on_message_confirm()` → `vm.DismissMessage()` | `GameRootTest`：扫描公共事件找到 `IsWaitingForMessage()` 的实例，`_on_cancel()` 后断言不再等待 |
| `W2-P1-03` | `123` 掩码被丢弃；`140` 音量/淡入无效 | `VmController` 增 `SetKeyStateProvider(Callable)`、`KeyCodeAuto(mask, autoKeys)`、`KeyCode(mask)`；`InputRouter` 记录滚轮 notch（`mask==771` 直接返回 ±1）与当帧按键锁存（`clear_frame_latches()`）；`_on_sound()` 接 `volume_db`（`linear_to_db(volume/100)`，-1=满）与 `frames/60` 秒 Tween 淡入/淡出 | `InputRouterTest` 8–10：notch、771 映射、latch 清空、左键 MASK_OK；`GameRootTest` 信号连线 |
| `W2-P1-04` | `action=1` 空槽新建幽灵图；负缩放失效 | `draw_picture()` 对不存在槽位的 Move 直接 `return null`；`_build_picture()` 记 `base_size` meta，Move 只改位置/缩放不二次相乘；`scale_percent<0` 走 `TextureRect.flip_h` 水平镜像（**取舍**：审阅建议 `scale.x=-abs(s)`，但 Control 负缩放以左上角为轴向左溢出，`flip_h` 像素等价且保住布局盒） | `PictureRoundtripTest`：空槽 Move 返回 null 且计数不变；±50 尺寸相同、负值 `flip_h==true` |
| `W2-P1-05` | `\cdb[21:3:4]` 被丢弃、`\c[-1]` 越界 | `TextTokenizer` 的 `cdb` 产出 `Kind.CDB` 且 `token.text` 保留整串 `a:b:c`；`_first_int` 支持 `+/-`；`MessageWindow` 对 `COLOR/COLOR_SELF/CDB` 一律 `posmod` 取色（SysDB 调色板不在解包树中，`cdb` 暂回退 8 色表） | `DialogDrawTest` 3c：`\cdb[21:78:0]x` → CDB/21/`"21:78:0"`；`\c[-1]x` → COLOR/-1；两窗口各 7 字形不丢 |
| `W2-P2-01` | `AssetDB` 用 `get_file_as_bytes` 取音频长度导致双倍读盘 | 改 `FileAccess.open(...).get_length()` | `AssetDBTest` 仍全绿 |
| `W1-P0-01` 残余 | `pic.File` 可路径穿越；读档 `Pc` 越界 | `SaveCodec.IsSafeAssetPath()`：空串合法，拒绝首字符 `/`/`\`、含 `:`、任一段 `..`；`TitleScreen.RestoreInstances()` 校验 `0 <= Pc <= program.Len` 并跳过坏实例 | `SaveLoadTest` 新增 4 条（`../escape.png`、`Picture/..\\evil.png`、`C:/windows/evil.png` 拒绝，`Picture/face01.png` 通过） |

- **桥接补丁**：`VmController` 另增 `StopAll()`、`StartEvent(int)`（`StartCommonEvent` 返回项目类型 `Instance?`，GDScript 直接调用会报 `Nonexistent function`，此前 `GameRoot.start_event` 一直是死代码）。
- **证据**：`reports/godot_gate.log` 与 `reports/godot_gate_windowed.log` 均 `GATE_EXIT=0`；`SAVE_LOAD_TEST_OK 87/87`、`GAME_ROOT_TEST_OK`、`PICTURE_ROUNDTRIP_TEST_OK`、`DIALOG_DRAW_TEST_OK`、`INPUT_ROUTER_TEST_OK`、`ORACLE_STEP_TRACE_OK slots=846 steps=2461` + `TRACE_DIFF_OK 2461/2461`。
- **仍开放**：BGS 第三轨（核心 `IHost.SoundChannel` 只有 `{ Bgm, Se }`，W4）、`130/160/290` 转场/特效仍只写日志、`10` 的 teleport target 非 leader 分支未建模（按 leader 处理）。

---

### 4.7 M2 W3 Day-2 核心收敛与传送队列（已完成 2026-11-14）

- **D-13 发布期故障交互**：`VmController.Faulted/FaultReason`；`GameRoot.gd` 在 `StepFrame()` 后检测并 `report_fault()`：冻结 `_process`/`advance_frame` 与五个输入回调、关闭窗口，`z_index=100` 模态显示原因与「存档槽已锁定」，`can_save()` 为假；「导出诊断包」写 `user://fault_diagnostics_<unix>.txt`（帧数/阶段/原因/等待态/队长坐标朝向）。GameRootTest 断言冻结、不可存、可导出；2026-11-15 补上「返回标题」恢复（§4.17）。
- **D-14 CI 占位素材**：`tools/make_asset_stub.py` 生成 `godot/tests/fixtures/asset_stub/`（18×18 纯色 PNG、静音 WAV、README）；`AssetDB.using_fallback` 只在**未显式指定根且索引为空**时为真，显式根永不替换；AssetDBTest 新增 stub 解析与 fallback 断言。
- **W2-P2-02 单一 KeyCodes**：`VmController.KeyCodeForPoll(mask, held, autoKeys, wheelNotch)` 直接调用 `KeyCodes.Code`（`mask==771` 先返回带符号 notch）；删除 `InputRouter` 的 `code_for_poll/code_for_bits/code_for_mask` 三个 GDScript 镜像；`GameRoot` 的 123 provider 改走该桥。InputRouterTest 7/8/10 改为对桥断言，并覆盖 auto_keys 合并。
- **M1-P2-02 programs.json 解析缓存**：`ProgramLoader` 新增按绝对路径 + 文件长度 + 最后写入时间的 `JsonDocument` 缓存（`OpenDocument`），四个 loader 共用；`CachedArtifactCount`/`ClearCache()` 供测试；SaveLoadTest 断言重复加载不新增解析、ClearCache 归零。此前每次调用都重解析整份 artifact（550 CE + 73 地图事件）。
- **M1-P1-06 Teleport 队列**：`TitleScreen.OnTeleport` 只入队 `PendingTeleport(target,map,x,y,options)`，`ApplyPending()` 在 `StepFrame()` 之后逐条应用；map 3 保持新游戏链（Map003 事件 1），其余 id 直接 `EnterMap(map)` 并把队长放到 (x,y)，同帧多次传送按序执行。SaveLoadTest 断言：VM 帧内不切图、帧边界切到 map 1008（53×30）且队长 (5,7)、下一帧不重复。
- **证据**：`bash tools/godot_check.sh` 20/20 `GODOT_GATE_OK`，`LWsmf_RENDER=1` 窗口化同样 20/20；`SAVE_LOAD_TEST_OK 87/87`；C# 0 warning/0 error。

---

### 4.8 M2 W3 Day 3–4 地图呈现与 GameSession 接管（已完成 2026-11-14）

- **图块导出（W3 Day 3）**：`converter/parse/mps.py` 的 `load_map` 漏把 `parsed["layers"]` 放进返回值（`parse_map` 本身已按整层行优先读出 3×w×h 个 u32），修复后 `python -m converter map-tiles --root work/all --out game_data --reports reports` 产出 `game_data/map_tiles.json` **564,948 B**（sha256 `821c896b59877b7353a8a838fa0ea252316d76a0edd470872221b2839f42da01`），6 tilesets / 9 maps；Dungeon/Map002/Map003/SampleMapA/SampleMapA_1/SampleMapB/TitleMap 三层满，Map024/Map031 无图块层。注意 CLI 入口是 `python -m converter`（`converter/cli.py` 没有 `__main__`，`python -m converter.cli` 会静默 exit 0）。
- **C# 地形（`godot/core_cs/Terrain.cs`）**：镜像 `crates/wolf-game/src/map_runtime.rs` 的 `Terrain`——三层逐格 OR，`0xFFFFFFFF` 跳过，`autotile=raw/100000` 取低四位四角模式；非 quarter 的 bit4..7 是四向不可通行，quarter 的 bit4..7 是四角**可**通行、某侧 blocked 当且仅当该侧两角都不可通行。`MapRuntime` 增 `Terrain`/`SetTerrain`/`CanEnter(x,y,step)`；`TitleScreen.EnterMap` 用 `MapTileData.LoadAlongside` 装载，`MoveLeader` 按方向步进。`MapTileData.cs` 读 JSON、base64→u32，按绝对路径+长度+mtime 缓存。
- **GDScript 地图层（`godot/presentation_gd/MapLayer.gd`）**：`TileMapLayer` ×3（z 0/1/2）+ 16×16 调色板 `TileSet` + 黄色队长方块（z 10）；`load_from_file` 解 base64 并按 C# 同算法算 `_bits`，`is_blocked/is_solid_cell/blocked_count/solid_count` 与 C# 对齐；`_gui_input` 左键发 `cell_clicked(cell)`、鼠标移动发 `cell_hovered`；地图居中、点击坐标换算到格子。**MapChip 判定**：`work/all` 现可补齐 102 张零售 PNG，但正片卧室/标题为全屏 `Picture`，tile 层不外显；MapChip 是引擎通用示例素材，因此仍按通行度/图块号画占位色块（solid→15、部分 blocked→14），不接入生产（D-21）。
- **GameSession 抽取（W3 Day 4）**：`godot/core_cs/GameSession.cs` 纯核心（无 Godot 依赖），接口 `IVmSession`（VM 步进/选择/存档恢复）与 `IGameSessionView`（消息/选择/立绘/HUD 回调）；Phase、标题菜单、传送队列、`BuildSaveState`、`SaveGame/LoadGame`、`EnterMap`、`RestoreInstances` 全部从 `TitleScreen` 原样搬入。`TitleScreen.cs` 变薄适配层，`ScreenHostSessionView.cs` 把 `ScreenHostView` 包成 `IGameSessionView`。`SessionParityTest`：旧路径与 core 路径同输入 418 帧后 `SaveState` 归一化逐字节相等（夹具 `reports/session_old_path_state.json`，13,552 B）。
- **GameRoot 接管（W3 Day 4）**：`GameSessionController.cs`（`[GlobalClass]`）把 `GameSession` 桥给 GDScript；`GameRoot.gd` 的标题→新游戏→卧室→F5/F9 全链已走 session，立绘/HUD/地图层由 `CallableView` 回调驱动，F5/F9 在 `_unhandled_input` 接 `SaveGame/LoadGame`。踩到的四个真实缺陷并已修：`DefaultSavePath()` 的 `user://` 必须 `ProjectSettings.GlobalizePath`（core 只认普通路径）；`LoadGame` 漏恢复 `Frames = (int)state.PlayFrames`；脚本回放时键码必须答 `KeyCodes.CodeOk`（`VmController.ScriptedKeyCode`），否则 title 的 `123` 轮询永远拿 0；`F9` 成功后把 `frames` 与 `session.Frames` 同步。
- **证据**：`bash tools/godot_check.sh` **23/23** `GODOT_GATE_OK`（headless 与 `LWsmf_RENDER=1`），新增第 21 `TERRAIN_TEST_OK checks=32`、第 22 `MAP_LAYER_TEST_OK checks=18`、第 23 `SESSION_PARITY_TEST_OK checks=11 frames=418`；`GAME_ROOT_TEST_OK` 已覆盖 scripted 418 帧、存档/读档帧数与位置恢复、D-13 故障模态。
- **边界**：地图层保留占位色块（MapChip 为引擎通用素材，正片未使用）；主场景仍是旧 `TitleScreen.tscn`，切到 `GameRoot.tscn` 与整路线 `vars/effects` trace 留到 W3 Day 5（已完成，见 §4.9）。

---

### 4.9 M2 W3 Day-5 主场景切换与 vars/effects 逐指令对拍（已完成 2026-11-14）

- **主场景切换**：新增 `godot/scenes/GameRoot.tscn`（uid `uid://cgameroot0lwsmf`，root Control + `res://presentation_gd/GameRoot.gd`），`godot/project.godot` 的 `run/main_scene` 改为该场景；旧 `scenes/TitleScreen.tscn` 当时保留作对照与回归夹具，后于 2026-11-15 退役（§4.13）。`godot --headless --path godot --quit-after 180` 退出 0，日志只有 `[VmController] Loaded 550 common event programs.` 与 `[GameRoot] title: 新タイトル用イベント (133 slots)`，无 Parse Error。
- **Rust trace 扩到 vars/effects**：`StepTraceEntry` 增 `vars: Vec<(i32,i32)>` 与 `effects: Vec<String>`（effect 是预渲染 JSON，避免 wolf-cli 依赖 serde_json）；`Host` 增默认 `take_trace_effects()`；`Vm` 用 `trace_before` 快照在**下一条指令开始前**补齐上一条的变量差与副作用（帧末由 `finalize_trace()` 收尾），`diff_snapshots()` 只记真实变化（新出现且为 0 的值不记）。trace 地址空间：普通变量 = index，系统变量 = `9_000_000 + index`，Self = `1_000_000 + group*100_000 + slot`。
- **effect 契约（Rust/C# 字面一致）**：`call {target_event_id,args}`、`picture {action,slot,file,args}`、`sound {action,type,file,vol,frames}`、`choice {options,selected}`（取消为 -1）、`teleport {map_id,x,y,direction}`、`database {type,data,item,value}`、`effect {type,duration,args}`。**等待（Wait）不算副作用，两侧都不记录**。
- **C# 对拍扩展**：`TraceRecorderHost` 的 `TraceStep.Vars` 改 `Dictionary<int,int>`，choice 取消记 -1，database 去掉 `flags`，`Wait()` 空实现；`OracleStepTraceTest` 增 `Snapshot()`（三类地址）与 `Finish()`（diff 上一条 + 收割 `PendingEffects`），StepHook 在压入新步前完成上一条，帧循环后再收尾；JSON 的 `vars` 按地址升序、`effects` 用 `JsonObject.ToJsonString()` 原样输出。
- **结果**：两侧同为 **2461 steps、428 步带 vars、330 步带 effects**；`python oracle/diff_trace.py reports/rust_step_trace.json reports/csharp_step_trace.json` → `TRACE_DIFF_OK: 2461/2461 steps matched 100% (event_id=0)`（diff 逐字段比较 event_id/pc/op/vars/effects，不是只比步数）。夹具 `oracle/fixtures/samplemapa_event0_418_rust_trace.json` 已换成本次 Rust 实跑输出（sha256 `9dab9e81787526391681f4a0ef49fecaaf66084e74203105ea6040ad2bb34057`）。
- **门槛**：`bash tools/godot_check.sh` 仍 **23/23**（第 19 步 Oracle 对拍现在同时覆盖 vars/effects）；headless 与 `LWsmf_RENDER=1` 双模式均 `GODOT_GATE_OK`。
- **边界**：trace 仍只覆盖事件 0 的 418 帧；`wait` 与未建模的 130/160/290 转场不入 effects；整路线（标题→新游戏→map 1→消息/选择→存读档）的冻结输入序列尚未纳入 Oracle。

---

### 4.10 M2 W4 Day1-4 音频三轨、存档音频与确定性时钟（已完成 2026-11-15）

- **音频三轨（Day 1–2）**：`140` 的种类位是 `(intArgs[0] >> 4) & 0x07`——0=BGM、1=BGS、2=SE；旧实现只测 `bits & 0x20`，把 43 条 BGS 全部当 BGM。`IHost.SoundChannel` 增 `Bgs`（`Se` 保持 1），`SoundCmd` 增 `LoopPoint`/`Tempo`（`argc=7` 时 `Params[6]` 为循环点、`Params[5]` 为 tempo；停止类 `Params[1]` 为淡出帧）。`VmController` 暂存 `SoundLoopPoint/SoundTempo` 后仍发冻结的 5 参 `SoundRequested`。
- **AudioManager（`presentation_gd/AudioManager.gd`）**：三轨各一个 `AudioStreamPlayer`，`play(channel,file,volume,loop_point,tempo,frames)` 支持 loop/loop_offset（frames→秒）、tempo→pitch_scale（0.25–4）、`frames/60` 淡入；`stop`/`reset`/`snapshot`/`restore`（含 position_ms）。GameRoot 的 `_on_sound` 改走该管理器，删掉旧的单 BGM/SE 对。**取舍**：循环属性写在共享 stream 上，同一文件跨通道时会互相覆盖（零售几乎不复用同文件，未 clone）。
- **音频入档**：`SaveState.audio`（`SavedAudio`：channel/file/volume/loop/loop_point/tempo/position_ms），`MaxAudio=3`；校验通道唯一 0..2、安全路径、音量 0..200、tempo 25..400、位置 0..86,400,000。`GameSession.BuildSaveState/LoadGame` 经 `IGameSessionView.SnapshotAudio/RestoreAudio` 往返，读档在地图装载之后恢复（存档优先）。旧 `ScreenHostSessionView` 返回空列表以保持两条路径同构。
- **确定性时钟（Day 3–4）**：`core_cs/GameClock.cs` 镜像 `217 時刻表示`：读 Var4 时 / Var5 分 / Var6 星期 / Var8 周末标志 / Var160 日数；`ApplyMinutes` 保留零售「单次 if 进位」缺陷（+130 分 → `13:70`），跨午夜才翻星期并把周末标志置 25（从不清除）；`FormatTime` 复刻 6200 的 `7:05` 补零规则。ClockTest 用真实 CE217（Self600=130）验证同一行为。
- **30 天日程（Day 3–4）**：`TraceRecorderHost` 增 `ChoiceSelector`（仅 soak 使用，selector 为空时 oracle 行为不变）。`SleepSoakTest` 连续 30 次真实 `CE329 → CE317` 就寝链（每天 seed var63、23:30、星期、行动值），到第 31 天；逐日校验日数 +1、星期 +1、08:30/09:30 起床（周末 9 点）与未 fault，共 61 断言。
- **严格模式收口（W4 Day5）**：`VmController.StrictMode` 桥到 core，`GameRoot._ready` 在创建 VM 后立即置 true（发布与 CI 同一条路径，D-15）；未实现 opcode/未知事件进入 D-13 的 Faulted 模态并锁档。GameRootTest 新增「生产 GameRoot 必须 StrictMode」断言；core 的单测（VmTest）保留未实现 opcode 抛错用例。
- **证据**：`bash tools/godot_check.sh` **26/26** `GODOT_GATE_OK`（`reports/gate_final_w4.log`；`LWsmf_RENDER=1` 同样 26/26）；`AUDIO_MANAGER_TEST_OK checks=30`、`CLOCK_TEST_OK checks=11`、`SLEEP_SOAK_TEST_OK days=30 checks=61`、`SAVE_LOAD_TEST_OK 98/98`（第四轮审阅加固后计数）。
- **边界**：30 天 soak 只走就寝/日历链，当时未驱动白天 CE251/CE310 的图片菜单；2026-11-15 已由 §4.16 的 CE251/CE310 端到端回归补上（C# 侧 `124`/指针/`EngineState` 三态）；旧 `godot/presentation/*.cs` 与 `AssetCatalog.cs` 当时仍保留，因为 `SessionParityTest` 以 `TitleScreen` 作新旧对拍夹具；后于 2026-11-15 改为 core-only 并退役（§4.13）。StrictMode 已在 GameRoot 强制 true（D-15），发布期故障保护沿用 D-13。

---

### 4.11 M2 第四轮审阅加固（已完成 2026-11-15）

- **opcode 300 的 host 副作用**：`EventVm` case 300 在创建子实例后补 `Host.CommonEventCall(callee.Id, callArgs, callStrings)`，与 210 一致；此前 Rust 300 走 `Signal::CallByName` 会记录 `call`，C# 会漏掉。VmTest 的 300 用例改为挂 `TraceRecorderHost` 并断言至少一条 `kind=call`。
- **BGS 归因**：`TraceRecorderHost.Sound` 的 `type` 改为按 `SoundChannel` 取 `bgm/bgs/se`；旧实现把 `Bgs` 写成 `se`，会与 Rust 未实现 BGS 的情况假阳性全等。VmTest 新增 Bgs 通道映射用例。
- **trace 覆盖面**：`diff_trace.py` 的 step 身份比较加入 `frame`（此前只比 `event_id/pc/op`，`Wait` 偏差可整段漏掉）；`str_vars` 进入两侧 trace（Rust `Variables/SelfVars::trace_str_snapshot` + `diff_str_snapshots`，地址 `3_000_000+n` 普通串、`4_000_000+n` 系统串、`2_000_000+group*100_000+slot` Self 串；C# `TraceStep.StrVars` 与 `SnapshotStrings/Finish` 同构）。事件 0 实测 **10 步带字符串变更**（均 `2_600_005 = "\n"`），两侧逐字段相等。
- **存档防御面**：`SaveCodec.Validate` 校验 `MapFile` 安全路径；`SavedAudio.LoopPoint` 上限 `86_400*60`；`GameSession.ApplyTeleport` 只接受 target `-1/0`（非主角目标忽略并写 status），`QueueTeleport` 单帧上限 64 条。
- **音频循环单位**：`AudioManager._apply_loop` 的 WAV 分支由 60 Hz 帧数改为采样点 `roundi(loop_point * mix_rate / 60)`；AudioManagerTest 断言 `loop_begin` 为换算值。
- **MapLayer 去双实现**：新增 `presentation/TerrainBridge.cs`（`[GlobalClass]`，包装唯一 C# `Terrain`），`MapLayer.gd` 删除 `mask_for_word/_bits_for_raw`，逐格 `Mask(x,y)` 取自 C#；`load_from_file` 自动按同目录 `programs.json` 装载。点击/悬停信号已接到 `GameRoot`（点击相邻格走一步）。
- **证据**：`SAVE_LOAD_TEST_OK 98/98`（新增 MapFile 穿越、音频 LoopPoint 上限、非主角 teleport 三项拒绝）、`AUDIO_MANAGER_TEST_OK checks=30`、`VmTest ALL PASSED`（新增 300 host 副作用与 Bgs 映射）、`TRACE_DIFF_OK 2461/2461`、`MAP_LAYER_TEST_OK checks=18`；`bash tools/godot_check.sh` **26/26** exit 0（`reports/gate_round4.log`），`LWsmf_RENDER=1` 同步复跑（`reports/gate_round4_windowed.log`）。夹具刷新为含 `str_vars` 的 Rust 输出（sha256 `3c6f29e4…`）。
- **仍开放（当时）**：`KeyCodeForPoll` 不按 `mask` 过滤、`126 BanKeys` 从不清除；后于 §4.15 按官方三态实现，§4.16 补上白天 CE251/CE310 端到端。`AudioManager` 共享 stream 的循环属性跨通道覆盖仍未改；整路线见 §4.12，旧表现层已于 §4.13 退役，MapChip 判定见 D-21。

---

### 4.12 M2 整路线输入序列与 Oracle diff（已完成 2026-11-15）

- **route 契约**：两侧都从生产入口 `TitleMap.mps` event 2 起步，脚本输入固定（每帧自动关消息、自动选第 0 项、`123` 恒答 OK），共 **1500 帧**。Rust 侧新增 `crates/wolf-cli/src/route.rs`（`RouteHost` + `Route`，复刻 `GameSession` 的 StepFrame→start_pending→teleport 顺序），C# 侧新增 `godot/tests/RouteTraceTest.cs`（真实 `VmController` + `GameSession` + 包装 `RouteHost`）。两侧逐指令导出 `{step,frame,map_id,event_id,pc,op,vars,str_vars,effects}`，schema `wolf.route`。
- **map id 根因**：首测在 step 237（frame 419）分叉——Rust 起 event 0、C# 起 event 1。原因是 Rust 用 `databases.map_path(1)`（SysDB type 0 row 1）得到 `SampleMapA.mps`，而 programs.json 的 map id 来自 `converter/export/programs.py:12 _map_id`（stem 数字优先，否则 `1000+index`），id 1 是 `SampleMapA_1.mps`。`route.rs` 新增 `map_file_for(root,map_id)` 复刻该规则，`enter_map` 不再看 SysDB 行序。
- **另两处对齐**：`RouteHost::database` 补 `{kind:"database",type,data,item,value}` 副作用（value 仅取 `DbValue::Num`，否则 0）；`enter_map` 在 `MapRuntime::collect` 前按地图宽高夹取领队（new-game 的 130 请求 `(62,38)` 在 64×36 图上越界，两侧实际都落 `(62,35)`）。
- **结果**：1500 帧两侧均 **73,812 步**，`ROUTE_DIFF_OK: 73812/73812 steps matched 100% (frames 1500/1500)`；其中 **12,469 步带 effects**，effect 分布 `database 11361, picture 1100, call 2, sound 2, teleport 2, effect 1, choice 1`，事件分布 `2:44483, 1:25452, 3:3787, 549:47, 42:43`。route 内 **没有 op101 消息**，只有标题菜单 1 次选择；save/load 不计入逐指令 diff。
- **存档/读档回路**：`RouteTraceTest` 在 1500 帧 trace 收尾后先摘掉 `StepHook`，再 `SaveGame()` → 前进 2 帧 → `LoadGame()`，断言帧数、领队坐标、地图 id 全部回到存档点；输出 `ROUTE_TRACE_TEST_OK ... save_load=ok`。这是状态断言，不是逐指令对拍。
- **门禁**：`oracle/diff_route.py` 支持 `.gz` 夹具，逐字段比较 `frame/map_id/event_id/pc/op/vars/str_vars/effects`；Rust 夹具 `oracle/fixtures/main_route_rust_trace.json.gz`（429 KB，原始 8.2 MB）。`tools/godot_check.sh` 扩为 **27/27**，第 27 步跑 `RouteTraceTest` 并 diff 夹具；headless 与 `LWsmf_RENDER=1` 双模式 `GODOT_GATE_OK`（`reports/gate_route.log`、`reports/gate_route_windowed.log`）。
- **边界**：route 只覆盖标题→新游戏→map 1 的 1500 帧，没有白天 `CE251/CE310` 图片菜单与 101/102 消息；`wait` 与未建模的 130/160/290 转场仍不入 effects；更长路线需另立 route 名与夹具。

---

### 4.13 旧表现层退役与 core-only 测试（已完成 2026-11-15）

- **SessionParityTest 改为 core-only**：不再实例化 `TitleScreen`/`ScreenHostView`，改用 `SessionHarness` 驱动真实 `VmController` + `GameSession`；418 帧后与冻结夹具 `oracle/fixtures/session_core_save_state.json`（12,350 chars，`play_frames=418`）逐字段比较。输出 `SESSION_PARITY_TEST_OK checks=9 frames=418 core=frozen`。
- **新增测试底座**：`godot/tests/TestSessionView.cs`（`IGameSessionView` 的内存实现，含 `CleanMessage` 控制码清洗与立绘槽语义）与 `godot/tests/SessionHarness.cs`（建 `TestVm` + `GameSession`、接四信号、`RunToBedroom`）。`SaveLoadTest`、`PresentationTest`、`MapMessageTest` 全部迁到该底座，分别 `98/98`、`PRESENTATION_TEST_OK phase=bedroom frames=418`、`14/14`。
- **删除的旧层**：`presentation/TitleScreen.cs`、`ScreenHostView.cs`、`ScreenHostSessionView.cs`、`AssetCatalog.cs`、`MapView.cs` 及其 `.uid`；`scenes/TitleScreen.tscn`；`tests/ScreenshotTest.cs`、`.tscn`、`.cs.uid`。保留 `VmController.cs`、`GameSessionController.cs`、`TerrainBridge.cs` 三座生产桥。
- **文档/注释**：`godot/README.md` 重写为 M2 现状（28 道门槛、主场景、目录契约、单测命令与当前缺口）；`GameSession`/`VmController`/`AssetDB`/`GameRootTest` 中指向旧类的注释已改。
- **证据**：`bash tools/godot_check.sh` **27/27** exit 0，`GODOT_GATE_OK`（`reports/gate_retire.log`）；`LWsmf_RENDER=1` 同样 exit 0（`reports/gate_retire_windowed.log`）；`ROUTE_DIFF_OK 73812/73812`、`SAVE_LOAD_TEST_OK 98/98`、`MAP_MESSAGE_TEST_OK 14/14`、`SESSION_PARITY_TEST_OK checks=9`。
- **边界**：退役只清代码路径，不含行为变化；Message/Choice 仍由 `GameRoot.gd` 自绘。白天 `CE251/CE310` 的图片状态与 HUD 由 §4.14 补上，菜单点击与日程/体力推进已由 §4.16 端到端覆盖；仍未覆盖从冷启动进入白天。

---

### 4.14 图片状态/指针桥与 CE217 HUD（2026-11-15）

- **opcode 124 接真**：`VmController` 维护 `Dictionary<int,PictureCmd>`（draw/move/set 更新，erase/hide 删除），新增 `LastPictureAnchor`/`LastPictureKind`；`PictureState` 通过 `SetPictureStateProvider(target, method)` 调用 `PictureStage.picture_state`，按 Rust `game.rs picture_state` 回答 field 8 hover（缺失 -1）、9 existence（0/1）、0/1 x/y、2/3 scale、4 extras[0]、5→0、10 text?1:-1。因 Godot 的 C# 编组拒绝 `Nullable<int>`，另暴露 `PictureStateBridge`/`EngineStateBridge`（无答案 → `int.MinValue`）。
- **命中几何**：屏幕层（`number < 0 || >= 12000`）整屏；text kind 用窗口宽高（CE251/CE217 的 `args[1]=args[2]=1` → 1×1）；window kind 且宽高 > 2 用窗口；其余用贴图实际尺寸。锚点与缩放复用 `UiSkin.anchor_position`/`UiSkin.unit`。
- **指针链**：`InputRouter` 记录 `pointer_pressed` 与鼠标位置；`GameRoot.advance_frame` 每帧先 `vm.SetPointer/SetPointerPressed/BeginFrameInput`；`VmController.EngineState(7)` 按官方三态返回：本帧新按下 2、按住 1、未按 0。`EngineState` 其它字段仍返回 0（对齐 Rust `GameHost`）。`InputRouter.mouse_seen` 在首个鼠标事件前置 false，此时 `GameRoot` 传 `(-1,-1)`，对齐 Rust `Game.mouse` 初值，避免鼠标未动时左上角/整屏图片误 hover（D-09）；`GameRootTest` 断言该初值，并经生产 provider 断言 124 能达到 `PictureStage`。
- **渲染修正**：`GameRoot._on_picture` 现在把 `anchor`/`kind` 传给 `PictureStage.draw_picture`。此前 anchor 被丢弃，所有图片按左上锚点摆放；`kind` 也由 stage 保留，text 槽不再被误判为贴图。
- **CE251/CE310 取证**：CE251 菜单按钮/文字对 `1200/1201 (512,230)`、`1202/1203 (512,280)`、`1204/1205 (512,330)`，条件行 `1206/1207 (512,380)`（self49）、`1208/1209 (512,430)`（self558）；hover 块在 pc1532-1540（`121` 清 self97 → `124 [..,1200,8]` → `111` 判 self99 → `290` 弹动 → `121` 置 self97=1）。CE310 五按钮在 `(512,200/250/300/350/400)`。CE217 的 6200 是右上锚点（anchor 3）`\f[24]\v[4]:\v[5]`，现在由 text 分支渲染成真实 `8:30`，不再走占位块。
- **证据**：`res://tests_gd/PictureStateTest.tscn` → `PICTURE_STATE_TEST_OK checks=30`（含真实 `CE217`：`HUD label='8:30'`、Var4=8/Var5=30）；`bash tools/godot_check.sh` **28/28** `GODOT_GATE_OK`（`reports/gate_day_menu2.log`），`LWsmf_RENDER=1` 同跑（`reports/gate_day_menu2_windowed.log`）。
- **边界**：`work/all` 未解包 `1.4.0/base_bottan.png`，测试用 `SystemFile/WindowBase.png` 代替按钮尺寸；CE251/CE310 的菜单点击、`102` 自动选择与日程/体力结算已由 §4.16 覆盖；仍不是从冷启动走到白天的完整回归，CE310 的动态标签行也未单独构造。

### 4.15 123/126 输入权限状态模型（已完成 2026-11-15）

- **官方语义（据 WOLF 编辑器帮助「キー入力」页）**：`126` 不是“位掩码清 held”，而是按**目标 + 权限状态**修改：低字节 `bits0..7` 选目标键/设备，`bits8..9` 为三态（`0` 常允许、`1` 仅移动时禁止、`2` 移动与按键都禁止）；`bit28` 是设备分支（键盘/鼠标/手柄/全部设备，只有允许/禁止二态；本作把非 0 目标折叠为单一 `device_deny`，未按设备分槽）。未选中项保持不变，**无自动清除**；恢复必须对同一目标再发 `state 0`。状态跨事件/跨地图持久，现代版本入档。
- **`123` 的 mask**：官方只公开“读取大类 + 方向/决定/取消/副键复选 + 5 种取得时机”，是复合 UI 位字段，**不是允许键掩码**；`123` 返回标量键码。旧实现把 `mask & ~BanKeys` 后仍返回整键盘键码，导致 `BanKeys` 实际无效；现改为 `host.key_code_auto` 拿键码后按权限过滤。`125` 的键盘自动键可穿透设备禁，但不能穿透基本输入全禁。
- **Rust**：`crates/wolf-vm/src/exec.rs` 的 `InputState` 改为 `auto_keys/basic_move_deny/basic_all_deny/device_deny`，新增 `apply_permission` 与 `filter_code`（键码→位，再按权限归零）；`123` 原样传 mask、结果过滤，`126` 走 `apply_permission`。`crates/wolf-game/src/game.rs` 的移动读 `movement_deny()`、`advance_player_input`/`handle_choice_input` 的 OK 读 `basic_all_deny`；标题菜单暂不额外过滤。`cargo test -p wolf-vm` **95 passed**，新增用例覆盖 `0x1F0/0x2F0/0x0F0`、`125` 穿透、设备分支。
- **C#**：`EventVm` 删除 `BanKeys`，改为 `BasicMoveDeny/BasicAllDeny/DeviceDeny` + `ApplyInputPermission`/`FilterKeyCode`；`GameSession.StartTitle`/`StartNewGameNow` 调 `ResetInputPermissions`。`VmTest` 用 `KeyHost` 记录 `LastKeyMask` 并覆盖同一组三态/设备/`125` 穿透；`SMOKE_OK`、`ALL VM TESTS PASSED`。
- **回归**：Rust `route` 与 C# `RouteTraceTest` 均 **73,812 步**，`diff_route.py` **73812/73812 全等**；28 步门禁双模式全绿：headless `GODOT_GATE_OK`（`reports/gate_input126.log`）、`LWsmf_RENDER=1` 同绿（`reports/gate_input126_windowed.log`，`ROUTE_DIFF_OK 73812/73812`、`PICTURE_STATE_TEST_OK checks=30`）。
- **边界**：`123` mask 的逐位表、`126` 的 target bit 与具体键/设备的对应仍未由编辑器二进制差分证实；本作脚本只比较标量键码（10/11/2/8 等），当前模型足以覆盖。`126[0]` 是否清空全部、F12 回标题是否复位，官方无明文，保持“只改目标、不清全部”。

### 4.16 白天菜单端到端与 30 步门禁（已完成 2026-11-15）

- **`EngineState(7)` 改为官方三态**：按 `docs/OPCODE_124.md:97`，`0x3000` 的 field 7 不是单帧布尔，而是 **0=未按 / 1=按住 / 2=刚按下**。`VmController` 改为 `_pointerPressFrame ? 2 : (_pointerPressed ? 1 : 0)`，field 8/9 当时仍返回 0（已在 §4.19 补齐）；`BeginFrameInput()` 继续把 false→true 边沿搬进本帧。旧实现只在边沿返回 1，导致 CE251 的 `111 [1,1600098,1,2]`（`== 1`）在按住帧失败。
- **CE251 真实菜单**：正常处理入口是 pc1022，不是图片绘制块 pc1486；从 pc1486 起步会跳过 pc1061/1088/1239 对 self10/11/12 的随机命令写入（`121 [..,1,7/9/10,24576]`，`0x6000` = rand_range），点击后 self13=0、全局 48 永远 0。`DayMenuTest` 改为 `StartEventAt(251,1022)` + 150 帧预热，hover (512,230) 12 帧、按住 5 帧、松开 20 帧，再循环到 var48 变化并跑到事件结束。
- **CE251 点击链**：图片点击块在 pc1647/1772/1897/1930/1986，分别 `124 field7`→`111 ==1`→erase→`213 ["1".."5"]`；标签定义在 pc2025/2029/2033/2037/2041 的 `102 [5]` 分支里。第 1 行把随机命令 self10 复制到 self13（pc2026），确认在 pc2434 `102 ["是","算了"]`，pc2438 写全局 48，随后按 6/7/8 分派执行 500252/500256 等工作命令。
- **CE310 周末菜单**：菜单绘制在 pc128-168，行按钮/文字为 1200/1201 (512,200)、1202/1203 (250)、1204/1205 (300)、1206/1207 (350)、1208/1209 (400)、1210/1211 (450)。点击块 pc201/236/271/305/339/373 都先 `121 [2000613,0]` 再 `213 ["処理1".."処理6"]`；第 5 行“悠然自得”落在 pc2199 的休息分支，调用 500217/500222，实测把时钟推到 15:30、体力降到 1 并结束事件。
- **新增回归**：`godot/tests_gd/DayMenuTest.gd`（13 项，`DAY_MENU_TEST_OK checks=13 command=1 hour=18 minute=30 energy=0`）与 `godot/tests_gd/WeekendMenuTest.gd`（9 项，`WEEKEND_MENU_TEST_OK checks=9 hour=15 energy=1`）。两者都断言图片存在、hover、三态鼠标、日程推进与体力下降；CE251 另断言命令号与全局 48，CE310 另断言事件结束。`PictureStateTest` 的旧“边沿=1”断言同步改为 2/1/0。
- **门禁**：`tools/godot_check.sh` 扩为 **30/30**，新增第 29 步 CE251、第 30 步 CE310。headless `reports/gate_weekend.log` 与 `LWsmf_RENDER=1` `reports/gate_weekend_windowed.log` 均 `GATE_EXIT=0` / `GODOT_GATE_OK`，30 步全绿；其中 `ROUTE_DIFF_OK 73812/73812`、`PICTURE_STATE_TEST_OK checks=30`（右键/中键补测后为 37，见 §4.19）。
- **边界**：两条测试都从菜单绘制前的中段 pc 起步，不是从冷启动/前一晚走完整剧情；CE251 的 `102` 确认由脚本自动选“是”，没有点击确认窗；CE310 的动态标签行（self5/6/7）未构造，只覆盖无条件的“悠然自得”。从冷启动进入白天仍需剧情夹具。

### 4.17 Faulted 标题安全点恢复（已完成 2026-11-15）

- **恢复动作**：故障模态新增「返回标题（放弃当前进度）」；`GameRoot.restart_after_fault()` 先 `StopAll`/`ResetDiagnostics`，清指针按压、消息/选择窗、图片层、音频轨与地图层，再 `session.StartTitle()` 重建标题事件；成功后才释放 `faulted`，`frames` 归零。失败则保持 `phase="failed"`，不假装恢复。
- **不写坏档**：恢复路径不调用 `SaveGame`，标题阶段本身也不能存盘；原存档槽保持原样。`GameSession.StartTitle()` 现在同时清 `_messageOpen/_hasChoices/_actionPressed/_pendingLoad` 与传送队列，避免故障时打开的对话框/排队传送污染下一次标题。
- **证据**：`godot/tests_gd/GameRootTest.gd` 在报告故障、导出诊断后调用 `restart_after_fault()`，断言 latch 清除、`phase=="title"`、模态隐藏、`frames==0`、`session.Phase=="title"`；`GAME_ROOT_TEST_OK`。整段回归仍由 30 步门禁覆盖。
- **边界**：这是“回到标题”的放弃式恢复，不是回到故障前最后一步；带 `VmController` 状态的安全点快照/回滚仍留 M5（D-13/D-21 之外的发布策略）。

### 4.18 Round-5 审阅 P1：126 权限接线与故障复位（已完成 2026-11-15）

- **`126` 权限进入移动与交互**：`VmController` 暴露 `MovementDeny`/`BasicAllDeny`，`GameSessionController` 同步；`GameSession.MoveLeader` 在设置朝向前按 `KeyCodes.Up/Down/Left/Right` 拒绝被禁方向，`AdvanceFrame` 的 `ActionKeyRequested` 追加 `BasicAllDeny & KeyCodes.Ok` 检查；`GameRoot` 的裸 action/选项与取消尊重 `BasicAllDeny`，消息确认仍按 Rust `message_dismissed` 放行。
- **故障/读档补口**：`restart_after_fault()` 在清指针按压之外同时清 `InputRouter.pointer_pressed` 与 `clear_frame_latches()`；F9 先 `LoadGame()`，若 `vm.Faulted` 则进入 D-13 模态并显示 `load faulted`，不再把故障读档当成功。
- **证据**：`GameRootTest` 新增 126 全禁/方向禁与故障复位断言，headless `GAME_ROOT_TEST_OK`（`reports/game_root_test_p1.log`）；30 步门禁 headless `reports/gate_round5_p1.log` 与 `LWsmf_RENDER=1` `reports/gate_round5_p1_windowed.log` 均为 `GATE_EXIT=0` / `GODOT_GATE_OK`，`GAME_ROOT_TEST_OK`、`ROUTE_DIFF_OK 73812/73812`、`DAY_MENU_TEST_OK checks=13`、`WEEKEND_MENU_TEST_OK checks=9`。
- **边界**：只把已有 D-20 状态接到表现层，不改逐位映射；`EngineState(8/9)` 已在 §4.19 补齐，effects 归一化、共享 `AudioStream` 循环属性仍是 P2。

### 4.19 鼠标右键/中键三态（已完成 2026-11-15）

- **官方契约**：`docs/OPCODE_124.md:97` 与 `:293` 规定 `0x3000` 的 field 7/8/9 都是鼠标三态——**0=未按 / 1=按住 / 2=刚按下**。此前只有 field 7 实现三态，field 8/9 恒 0，CE ev41 的「右/左/中键状态」读取会缺右/中两项。
- **单一状态源**：`VmController` 为左/右/中分别维护 `_pointerPressed`、`_rightPressed`、`_middlePressed` 与各自的 `Pending/Frame` 边沿；`EngineState(7/8/9)` 统一走 `TriState(held, pressedThisFrame)`。`SetPointerRightPressed`/`SetPointerMiddlePressed` 在 false→true 时置 pending，`BeginFrameInput()` 在同一帧把三组 pending 搬进 frame；`ResetPointer()` 清坐标 `(-1,-1)` 与三组 held/pending/frame。
- **输入层**：`InputRouter` 分别记录 `pointer_pressed`/`right_pressed`/`middle_pressed`，鼠标按钮按 `LEFT/RIGHT/MIDDLE` 统一更新；`GameRoot.advance_frame` 每帧传三个 held 状态，故障「返回标题」调用 `ResetPointer()` 并清三个 router 标志。
- **测试**：`PictureStateTest` 新增 field 8/9 的边沿 2→按住 1→释放 0 断言，并断言右键不泄漏到 field 7、中键同理，`PICTURE_STATE_TEST_OK checks=37`；`InputRouterTest` 新增右键/中键 held 与释放清零，`INPUT_ROUTER_TEST_OK`。
- **门禁**：30 步 headless `reports/gate_m3_pointer.log` 与 `LWsmf_RENDER=1` `reports/gate_m3_pointer_windowed.log` 均 `GATE_EXIT=0` / `GODOT_GATE_OK`；含 `PICTURE_STATE_TEST_OK checks=37`、`DAY_MENU_TEST_OK checks=13`、`WEEKEND_MENU_TEST_OK checks=9`、`ROUTE_DIFF_OK 73812/73812`。
- **边界**：field 10 仍是滚轮量（不属三态）；`125` 注入的虚拟鼠标与真实右键/中键的优先级仍沿用现有链路，未新增语义。

### 4.20 原版 `.sav` 导入严格校验（已完成 2026-11-15）

- **契约**：`docs/godot/DATA_CONTRACT.md` §6/§7 要求严格 UTF-8、失败后 replacement 解码并记录告警；`docs/FORMAT_SPEC.md:1861-1868` 明确零售 8 槽的 `raw[6]=0x55`（UTF-8）、`0x14=0x19`、末字节 `0x19`。这批存档不是 CP932/GBK，审查建议的代码页重写不适用。
- **实现**：`LegacySaveImporter` 增 `StrictUtf8`；`SaveReader.ReadMemDataString(size, field)` 先严格解码，失败时用 `Encoding.UTF8` replacement 回退并记录 `"<field> at 0xXXXX: invalid UTF-8, replacement applied"`；`SlotResult.encoding_warnings` 暴露每条告警，`ImportAll` 在非空时写 `reports/encoding_warnings.json`。
- **严格校验**：`ImportFile` 始终校验解密后的起始字节 `0x19` 与末字节 `0x19`；新增 `rejectBadChecksum`，为 true 时 checksum 不符抛 `InvalidDataException`，宽松模式仍保留 `checksum_ok=false` 供诊断。`ImportAll` 透传该选项。
- **测试**：`SaveImportTest` 仍验证零售 8/8；新增篡改 `raw[2]`（宽松 `ChecksumOk=false`、strict 抛）、篡改 `raw[0x14]`（抛）、合法 UTF-8 “あ” 不告警、非法 `FF FE` 恰好 1 条告警且不抛。输出 `SAVE_IMPORT_VALIDATION_OK checksum=strict start=strict utf8=warn-once` 与 `SAVE_IMPORT_OK 8/8`。门禁第 4 步同时断言两条标记。
- **门禁**：headless `reports/gate_save_import.log` 与 `LWsmf_RENDER=1` `reports/gate_save_import_windowed.log` 均 `GATE_EXIT=0` / `GODOT_GATE_OK`，30/30 全绿。
- **边界**：不把 `raw[6]` 当作可切换编码；不重写原版 `.sav`；`encoding_warnings` 是附加字段，`schema_version` 仍为 1；未建模的 VM 协程字段仍按 `not_recoverable` 排除。

### 4.21 `250` 数据库宿主（已完成 2026-11-15）

- **契约**：`250` 读写三个数据库容器——`CData`（角色/队伍）、`Data`（系统/物品）、`Sys`（系统变量表）。逻辑列顺序来自转换后的 `game_data/database.json`：根键是 `cdata`/`database`/`system`，每个 type 的 `fields` 顺序即列号；`field.kind=="str"` 取 `strings[]`，否则取 `numbers[]`。编辑器标签来自 `work/all/BasicData/{CDataBase,DataBase,SysDatabase}.project`。
- **实现**：新增 `godot/core_cs/GameDatabase.cs`。基础层读 `.dat` 导出的行；脚本写只进覆盖层 `_cells`/`_texts`，行数写进 `_rowCounts`。读 `data_id=-1` 返回行数、`item_id=-1` 返回首个全空行；写 `data_id=-2` 改行数并丢弃越界覆盖，写 `item_id=-2` 清整行；`>=100` 的行按扩展行返回空。写算子 `(flags>>4)&7` 覆盖 assign/add/sub/mul/div/rem/max/min，除/模 0 保持原值；数字写不覆盖非空文本列，文本写数字列按 InvariantCulture 解析整数。
- **标签解析**：`.project` 解析 type 名与字段名，`ResolveLabels` 先按 type 标签找 type，再按字段标签在字段数一致时找列；标签做 NUL 截断、U+2014/U+2015→'-'、U+3000→' ' 与 Trim。`Sys` 容器按 Rust 规则不使用 `.project` 标签。实测 `Data(18,22)` 的“システム設定/装備可能欄数”漂移到 `(17,22)`，`CData(3,1)` 的“パーティー情報/メンバー1”解析为 `(6,1)`。
- **接线**：`VmController` 新增 `DatabaseStore` 与 `Database(DbOp)`；`LoadPrograms` 从 `programs.json` 同目录加载 `database.json`，并从 `work/all/BasicData` 读三份 `.project`。`EventVm` 的 `250` 已把 `DbOp` 交给 host；数据库缺失只报错，不阻断程序加载。
- **测试**：`DatabaseTest` 先直接断言导出名 `勇者先生`、CData 0/0/5=1、标签漂移、哨兵行/列、行数覆盖、清行、扩展行、文本保护与覆盖层；再用真实 `EventVm` 跑一条 `250` 程序，覆盖数值读、字符串读、带标签读、字符串写、数值写。输出 `DATABASE_TEST_OK name/labels/sentinels/overlay/vm-250`；`VmTest` 在 `TestMaxCallDepth` 后调用它，Smoke 门禁第 3 步同时断言 `SMOKE_OK` 与 `DATABASE_TEST_OK`。
- **门禁**：headless `reports/gate_database.log` 与 `LWsmf_RENDER=1` `reports/gate_database_windowed.log` 均 `GATE_EXIT=0` / `GODOT_GATE_OK`，30/30 全绿；含 `DATABASE_TEST_OK .../vm-250`、`ROUTE_DIFF_OK 73812/73812`、`PICTURE_STATE_TEST_OK checks=37`、`DAY_MENU_TEST_OK checks=13`、`WEEKEND_MENU_TEST_OK checks=9`。
- **边界**：只解析 `.project` 的 type/field 标签，不消费行名、说明、默认值等编辑器元数据；覆盖层不写回 `database.json`；战斗侧 `CE152/CE188` 的命令可用性切片已在该宿主上验证（§4.22），完整战斗回合仍未接。

### 4.22 战斗命令可用性切片 CE188/CE152（已完成 2026-11-15）

- **来源**：Rust `crates/wolf-game/tests/m6_battle_commands.rs`；C# 侧此前没有战斗实现，本切片先补 `CE188` 的命令菜单链。
- **数据前置**：CE188 在空 VM 上因敌全灭直接结束；测试显式写 `CData 18/4/0=1`（队伍数）、`CData 18/69/0=1`（敌数）、`CData 0 row12 cols47..54=[3,4,6,7,0,0,0,0]`（命令槽）与 `CData 0 row12 cols20..28,9,10,4=[100,100,50,50,20,15,10,30,30,20,10,10]`（领队属性），seeded 线另写 `CData 7/0/0=1`（道具 0）。这些值在 Rust 测试里来自 `SaveData01`，C# 用数据库覆盖层直接播种。
- **执行**：`godot/tests/BattleCommandTest.cs` 每条线新建 `VmController`、`LoadPrograms`、`Vm.Host=vm`、`StrictMode=false`，偶数帧 `CodeOk`/奇数帧 `CodeNone` 脉冲，`StartEvent(188)` 跑 200 帧；第 0..59 帧持续播种。逐帧记录是否出现 CE152/CE192/CE244 与是否有真实攻击行（CData 19 的 col1∈{26,25,30,29} 且 col2==1）。不能只看末态：CE192 会重写这些行。
- **结果**：control 无可选真实攻击行，seeded 有；两条线都到 CE192，seeded 到 CE152，均不生成 CE244。输出 `BATTLE_COMMAND_TEST_OK control_real=0 seeded_real=1`。
- **门禁**：`VmTest` 在 `DatabaseTest` 后调用；`tools/godot_check.sh` 第 3 步追加 `BATTLE_COMMAND_TEST_OK`。headless `reports/gate_battle_command.log` 与 `LWsmf_RENDER=1` `reports/gate_battle_command_windowed.log` 均 `GATE_EXIT=0` / `GODOT_GATE_OK`，30/30 全绿。
- **边界**：只覆盖命令构建/可用性，不覆盖真实回合推进、目标选择、伤害/胜负、CE239/CE242/CE243；`CE188` 不调用 `CE244` 是结构边界；敌数/队伍数仍靠播种而非完整存档导入。

### 4.23 AutoEvents 调度器与整路线同步（已完成 2026-11-15）

- **来源**：Rust `crates/wolf-game/src/auto_events.rs`。该调度器跟踪 98 个事件：96 个 `run_condition==2` 的并行事件，加 allow-list 的 `CE242/CE243`；标题菜单 `CE43` 跳过（只由标题入口显式启动），`CE216/CE217` 是读档后每帧重放的 HUD 事件。条件算子只有 `1 >=`、`2 ==`、`3 <=`，其他为 false；变量地址只接受 `VAR_BASE+0..800`（raw `2000000+n`，`n<800`）。同步是边沿触发：false→true 启动；true→false 只停止自己持有的实例；条件持续为真时不重启已自然结束的一次性事件；resume 事件在每个 `sync` 发现同事件没有 live 实例时重放。live 判定为 `pc < program.len` 或 `wait != None`。
- **实现**：`Program` 新增 `RunCondition/ConditionOperator/ConditionVariable/ConditionValue`，`ProgramLoader` 读 `run_condition_op_var_val` 四元组；新增 `godot/core_cs/AutoEvents.cs`（`FromPrograms`、`Sync`、`PrimeLatched`、`IsRunning`、`RunningCount`）。`GameSession.EnsureAutoEvents()` 惰性建表，`AdvanceFrame` 在 `_vm.StepFrame()` 之前同步，读档在 `EnterMap` 后 `PrimeLatched`；`StartNewGameNow` 不重建调度器，避免标题阶段已启动的 `CE38` 被重复启动（parity 的 map instance 从 7,8,9 恢复到 6,7,8）。调试可用 `LWsmf_TRACE_AUTO=1`。
- **route Oracle 修正**：`crates/wolf-cli/src/route.rs` 原先只复刻消息/选择应答，从不同步自动事件；C# 接入后第 32 步即分叉（Rust `frame=2/event 2`，C# `frame=1/event 38`）。现在 `route.rs` 在 `vm.step_frame` 前调用 `auto_events.sync(&mut route.vm)`，与 `Game::frame` 的“先同步、后 step”一致，传动/start_pending 仍在 step 之后；Rust 夹具重生成后两侧都是 **73,814 步**，`ROUTE_DIFF_OK 73814/73814`。
- **测试**：`AutoEventsTest` 用合成 38/216/217/43/242/244 程序覆盖 allow-list、边沿启动、resume 重放、条件回落停止与 `PrimeLatched`，输出 `AUTO_EVENTS_TEST_OK count=4 start=3 resume=2 gated=1`；`SessionParityTest` 断言 `AutoEventCount==98`，仍 `SESSION_PARITY_TEST_OK checks=10 frames=418 core=frozen`，冻结夹具已按新增 `CE38` 实例刷新。
- **门禁**：`VmTest` 在 `DatabaseTest` 后调用 `AutoEventsTest`，`tools/godot_check.sh` 第 3 步追加 `AUTO_EVENTS_TEST_OK`；整路线夹具由 73,812 步更新为 73,814 步。30 步 headless `reports/gate_auto_wired2.log` 与 `LWsmf_RENDER=1` `reports/gate_auto_wired2_windowed.log` 均 `GATE_EXIT=0` / `GODOT_GATE_OK`，含 `AUTO_EVENTS_TEST_OK count=4 start=3 resume=2 gated=1`、`SESSION_PARITY_TEST_OK checks=10 frames=418 core=frozen`、`ROUTE_DIFF_OK 73814/73814`。
- **边界**：自动事件只读普通变量条件，不覆盖 `raw_condition` 的其他分支；没有把标题菜单合成或引擎层 `CE271` 常驻事件一起接入；`CE38` 在标题阶段即可启动，与 Rust `Game::frame` 一致；完整战斗 `CE239/CE242` 已由 §4.24 接上 wiring，但 `CE253` 自然线、目标选择与 H 场景仍未覆盖。

### 4.24 CE239/CE242 战斗 wiring（已完成 2026-11-15）

- **来源**：Rust `crates/wolf-game/src/battle_wiring.rs` 与 `crates/wolf-game/tests/m7_battle_playable.rs`。Rust 的真实 day4 战斗由 `CE253` 调 `CE239`，需要 `seed_battle_databases`、`CData(5,16,0)` 标签、引擎协程与 `set_battle_wiring(true)`；C# 侧此前完全没有战斗调度。
- **实现**：新增 `godot/core_cs/BattleWiring.cs`。`Tick` 先检测 `340` 的 rising edge，再收割 `CE157` 写入的 CData type14 rows1..8（col0 激活者、col3 技能，`<0` 跳过、按行去重），随后按 `drive = manualEnable ?? (autoArm && RealBattleRunning)` 决定是否驱动。单发模式要求 rising、`350==0`、`343==0`；多轮模式在 `343!=0` 停止，条件满足时启动 `CE242`，`350==2` 且无 `241/242/243` 时补 `CE241`，下一轮在 `350==0` 且无三者时启动。命令号先取菜单队列，否则循环 `[1,3]`。`RealBattleRunning` 以 `CE253` live 或 `CE239` 实例有 `ParentId` 判定。
- **合成实例**：`EventVm` 新增 `CreateFreshInstance(eventId,parentId,returnTo)`，创建后 `Self.Restore(null,null)` 并登记；`PersistCommonSelf` 跳过这些一次性实例，保证合成 `CE241/CE242` 使用空白 `Self` 且不回写公共 Self。
- **帧序**：`GameSession.AdvanceFrame` 的顺序是 `AutoEvents.Sync` → `BattleWiring.Tick(_vm.Vm, op => _vm.Database(op))` → `_vm.StepFrame()`，与 Rust `Game::frame` 的“自动事件、战斗接线、采样输入、VM step”一致；`IVmSession` 为此新增 `Database(DbOp)`。
- **测试**：`BattleWiringTest` 用 26 个 battle seed（队伍/敌数、命令槽、领队属性、道具与初始武器）建 `VmController`，`SetEnabled(true)` + `SetRoundLoop(false)`，奇偶帧 OK/NONE 脉冲启动 `CE239`。运行中每次 `101` 消息等待自动 `DismissMessage`，直到 `343` 置位且 `CE239` 结束；`352` 会被结算清空，因此记录绝对值峰值而不是末态。结果 `BATTLE_WIRING_TEST_OK rounds=1 result=-1 ce242_pc=415`；`CE242` 的 pc 到 415，`350` 到 1，`352` 峰值 `±1`，`343` 置位。
- **门禁**：`VmTest` 在 `BattleCommandTest` 后调用；`tools/godot_check.sh` 第 3 步追加 `BATTLE_WIRING_TEST_OK`。30 步 headless `reports/gate_battle_wiring.log` 与 `LWsmf_RENDER=1` `reports/gate_battle_wiring_windowed.log` 均 `GATE_EXIT=0` / `GODOT_GATE_OK`，整路线仍 `ROUTE_DIFF_OK 73814/73814`。
- **边界**：只验证 `CE239` 直接入口的单轮结算，不覆盖 `CE253` 自然日线、目标选择窗 `CE159/CE162`、伤害公式矩阵、多人队伍与 H 场景；菜单命令队列/轮转 API 已实现但尚未由真实剧情驱动。

### 4.25 CE190/CE175 敌人生成与 CE162 目标选择（已完成 2026-11-15）

- **来源**：Rust `crates/wolf-game/tests/m7_battle_target.rs`。CE190 按 CData 18/69 的敌数生成敌人；CE190 调 CE144 把敌人 DB 行拷进战斗 DB，CE175 用坐标倍率算屏幕坐标。CE162 读同一个 18/69，敌方索引 0 选中后在 pc57 写出并退出；控制线只清 18/69=0，CE162 停在 pc49/pc50 自旋。
- **引擎默认层修正**：`GameDatabase` 新增与 Rust `Databases::with_engine_defaults` 一致的默认层：CData 18/97/0=32、18/98/0=24、Data 18/0/24=`SystemFile/WindowBase.png`。读顺序是 250 覆盖层 → 引擎默认层 → `database.json` 文件层；脚本写仍落覆盖层，所以 `set_num` 能覆盖引擎值。此前 CE175 把 160 乘上文件里的 0，四个坐标全写成 0；修正后 18/35=512、18/36=-288、18/37=512、18/38=312。
- **测试**：`BattleTargetTest` 播种 26 个 battle seed 后启动 CE190 600 帧，读回敌数/HP/坐标；再启动 CE162 300 帧并逐帧记录 pc，断言到 pc12、pc57 且事件退出。控制线清 18/69=0 后同样启动 CE190/CE162，断言未到 pc57、事件仍活、末 pc 为 49 或 50。输出 `BATTLE_TARGET_TEST_OK selected=1 control_pc=49 enemy_hp=100`。
- **关键修正**：`Drive` 的 OK/NONE 脉冲必须按**帧号**而不是按 `123` 调用次数。Rust 的 `300 CallByName` 会阻塞调用方一帧，CE162 的 pc50 `123` 落在奇数帧；C# 同序阻塞后若仍按调用次数发 OK，就会在奇数帧读到 OK，控制线误到 pc57。改成每次 `StepFrame` 前写 `currentFrame=i`、provider 按 `i % 2` 返回后，两条线才与 Rust 一致。
- **门禁**：`VmTest` 在 `BattleWiringTest` 后调用；`tools/godot_check.sh` 第 3 步追加 `BATTLE_TARGET_TEST_OK`。headless `reports/gate_battle_target.log` 与 `LWsmf_RENDER=1` `reports/gate_battle_target_windowed.log` 均 `GATE_EXIT=0` / `GODOT_GATE_OK`，30/30；整路线仍 `ROUTE_DIFF_OK 73814/73814`。
- **边界**：CE162 是从测试直接启动，不是 CE188 命令确认后的真实 dispatch；`CE159` 技能选择执行入口仍未接，伤害/胜负矩阵与 H 未覆盖。引擎默认层目前只补坐标倍率与窗口皮肤三项，不声称覆盖 Rust 的全部引擎默认值。

### 4.26 CE159 技能尾段与 CE162 目标入口（已完成 2026-11-15）

- **来源**：Rust `crates/wolf-game` 的 CE159 `X[戦]技能選択実行` 与 CE162 `X[戦]敵対象選択実行`；C# 侧此前只有 CE162 直启，没有技能选择到目标选择的完整尾段。CE159 的 pc110 读 Data 0/row/6（`使用可能シーン`），pc115 读 Data 0/row/2（`効果対象`），pc244 只放行目标值 10/11/12，pc247 才以 `300` 启动 CE162。
- **数据事实**：`work/all` 的 Data type0 是 18 列精简 schema，零售 44 行 `col2` 全 0，`col3` 才是 10/11/12；`.project` 的字段标签与数据版本错位，`使用可能シーン`/`効果対象` 无匹配标签，250 只能按回退列号读取。测试因此显式写覆盖层 `Data 0/2/2=10`（效果目标）与 `Data 0/2/6=1`（可用场景），不修改文件层。
- **测试**：新增 `BattleSkillTargetTest`。battle seed 后写入目标/场景，`CreateInstance(159)` 并设 `Self 1600000=1、1600001=2、1600002=0、1600022=2`，从 pc94 起跑（跳过 pc46 的菜单列表分支，直接验证尾段），`123` 脉冲按帧号奇数帧 OK。断言 pc110/115/247 到达、CE162 pc12/pc57 到达并退出、CE159 结束。
- **控制线**：效果目标为 0 时 CE159 走取消支路，CE162 从未启动；敌数为 0 时 CE162 已经由 pc247 生成，但停在 pc49/pc50 自旋，不写选中索引。分别输出 `no_target=1`、`no_enemy_pc=50`。
- **门禁**：`VmTest` 在 `BattleTargetTest` 后调用；`tools/godot_check.sh` 第 3 步追加 `BATTLE_SKILL_TARGET_TEST_OK`。headless `reports/gate_skill_target.log` 与 `LWsmf_RENDER=1` `reports/gate_skill_target_windowed.log` 均 `GATE_EXIT=0` / `GODOT_GATE_OK`，30/30；整路线仍 `ROUTE_DIFF_OK 73814/73814`。
- **边界**：测试从 pc94 注入 CE159 尾段，不是 CE188 命令确认后的生产 dispatch；没有覆盖 CE157 菜单到 CE159 的真实选择、CE253 自然日线、伤害/胜负矩阵与 H 场景。`使用可能シーン` 的 1/2 语义来自本数据集实测（0 立即结束、1/2 放行、100 在 pc113 后结束），不等于全部零售脚本语义。

### 4.27 H 阈值数据面（HEventGateTest）

- **来源**：Rust `crates/wolf-game/tests/m13_h_thresholds.rs` 的 `gate_bytecode_conformance` 与 `gate_branch_boundary_semantics`；真实 H 门共 37 个（CE310 10、CE318 6、CE323 6、CE335 15）。
- **测试**：新增 `HEventGateTest`。加载 `programs.json` 后逐条断言 37 个 `(event, pc, params)` 与 Rust 载荷一致（opcode 111、参数长度与每个 int）；单条件门按 `(Ints[0] & 0x0F) == 1` 识别，取 `value-1`（不匹配，first-true 为 -1）与 `value`（匹配 case0）两个边界；多条件门显式 31 点，`FirstTrueCase` 按 compare `0 >`、`1 >=`、`2 ==`、`3 <=`、`4 <`、`5 !=`、`6 &` 与 live store 求第一个真分支。
- **结果**：`H_EVENT_GATE_TEST_OK gates=37 boundaries=59 data_only=1`；14 个单条件门 ×2 + 31 个多条件点 = 59。
- **门禁**：`VmTest` 在 `BattleSkillTargetTest` 后调用；`tools/godot_check.sh` 第 3 步追加 `H_EVENT_GATE_TEST_OK`。headless `reports/gate_h_threshold.log` 与 `LWsmf_RENDER=1` `reports/gate_h_threshold_windowed.log` 均以 `GODOT_GATE_OK` 结束（shell exit 0），30/30；整路线仍 `ROUTE_DIFF_OK 73814/73814`。
- **边界**：只验证 bytecode/data 面与 compare 分支语义，不执行 H 场景；CE318 信頼 100/99 到 CE360 的剧情生成、CE310/323/335 的实际调用仍未接。它是 H 场景的前置门数据回归，不是 H 完成。

### 4.28 第六轮审阅 P1/P2 收口（2026-11-15）

- **来源**：`m3_round6_review_2026_11_15.md`，快照 `84bd3c7`；可执行项为 R5-P1-01、R5-P2-02/03/04/07，以及 D-28 的复位边界。
- **输入权限（R5-P1-01）**：`GameRoot.gd` 的 `_on_wheel` 与 `_on_move` 同口径检查 `BasicAllDeny` 的 16/32 位；`GameRootTest.gd` 在 126 state 2 下断言滚轮不移动选项，普通权限下滚轮仍可移动。
- **音频（R5-P2-03）**：`AudioManager.gd` 播放前取 `_assets.audio(file).duplicate()`，BGM/BGS 同文件不再共享 `AudioStream.loop_*`；`AudioManagerTest.gd` 断言两者不是同一实例且 BGS 播放后 BGM 的 `loop_begin` 仍为 0。
- **未知音效类型（R5-P2-04）**：`EventVm` 的 140 在 soundType 3..7 时写 `Host.Log("[EventVm] unknown sound type ...")`，仍落 BGM；`VmTest.TestUnknownSoundType` 钉住日志与通道。Rust 对这些位落 SE，C# 暂不改落点，避免静音。
- **图片镜像（R5-P2-07）**：`PictureStage` 改为容器 `pivot_offset=中心`、`scale.x=-1`，文本与占位槽随贴图一起镜像；`PictureRoundtripTest` 断言容器 scale/pivot 而非单个 TextureRect 的 flip。
- **Oracle（R5-P2-02）**：`diff_trace.py` 与 `diff_route.py` 的 effects 都改为严格 `[dict(...)]` 比对，不再一边滤空一边只滤假值；整路线仍 73814/73814。
- **AutoEvents 复位（D-33）**：第六轮建议的 `ResetForNewGame()` 曾实现，随后回退。Rust `AutoEvents` 没有 reset API，latched 是 session-scoped；清空会让 CE38 在新游戏首帧重放并顺移实例 id（6/7/8→7/8/9），破坏 `SESSION_PARITY_TEST`。现保留“StopAll 不清 latch”，测试断言 StopAll 后再次 Sync 只重放 216/217。
- **H 标记（D-32）**：输出追加 `data_only=1`，避免把 H 阈值数据面误读成 H 场景完成。
- **测试**：`GAME_ROOT_TEST_OK`、`AUDIO_MANAGER_TEST_OK checks=32`、`PICTURE_ROUNDTRIP_TEST_OK`、`SESSION_PARITY_TEST_OK checks=10 frames=418`、`H_EVENT_GATE_TEST_OK gates=37 boundaries=59 data_only=1`。
- **门禁**：headless `reports/gate_review6_r3.log` 与 `LWsmf_RENDER=1` `reports/gate_review6_r3_windowed.log` 均 `GODOT_GATE_OK` / `GATE_EXIT=0`，30/30；整路线 `ROUTE_DIFF_OK 73814/73814`。
- **边界**：`101` 未读完取消、零售字体、Data type0 列错位仍需运行时/零售证据；玩家胜利与多轮战斗、CE253 自然线、伤害/胜负矩阵、H 场景仍未接。

### 4.29 第七轮审阅 P1/P2 与 D-35 静态审计（2026-11-15）

- **来源**：`m3_round7_review_2026_11_15.md`，快照 `80eab07`；本轮处理 R7-NEW-04（会话重建）、R7-NEW-01（SE 复制策略）、R7-NEW-02（soundType 日志去重）与 D-35 的转换期审计，R7-NEW-03（文本槽负缩放）暂延后。
- **故障恢复重建（R7-NEW-04，D-36）**：`GameSessionController` 保存 `_ui/_savePath` 并新增 `Generation` 与 `Rebuild()`；`Rebuild()` 重新 `Setup` 一个全新 `GameSession`（连同新的 `AutoEvents` 边沿状态），不是复用旧对象清字典。`GameRoot.restart_after_fault()` 在 `StartTitle()` 前调用 `session.Rebuild()`，失败则回到 `phase=failed`。
- **测试**：`GameRootTest` 记录故障前 `generation_before`，重启后断言 `Generation == generation_before+1`；再以 scripted 输入重开新游戏，最多 900 帧回到 bedroom，并断言 `session.AutoEventStarts(38) >= 1`。`AutoEvents` 新增 `_starts`/`StartsFor`，`GameSession`/控制器透传 `AutoEventStarts`。
- **SE 复制（R7-NEW-01）**：`AudioManager.play` 只对 `channel != CHANNEL_SE` 的 BGM/BGS 复制 `AudioStream`；SE 直接使用缓存流，避免高频音效每次分配。BGM/BGS 的同文件 loop 隔离仍由 `AudioManagerTest` 的 `bgm.stream != bgs.stream` 与 BGM `loop_begin==0` 钉住，`AUDIO_MANAGER_TEST_OK checks=32`。
- **soundType 去重（R7-NEW-02）**：`EventVm` 新增 `(EventId, Pc, SoundType)` 的 `HashSet`，140 的 3..7 告警每个站点只写一次；`VmTest.TestUnknownSoundType` 改成 `179[2]` 循环两次 140，断言产生 2 个 `SoundRequested` 但只有 1 条日志（通道仍落 BGM）。
- **音效分布证据**：全量扫描 `game_data/programs.json` 的 140 共 **1,611** 条（CE 1,368 + map），soundType `{0:342, 1:113, 2:1156}`，零售导出**没有 3..7**。因此当前通道分歧只存在于合成测试，C# 保留 BGM 回退并告警，不据此改生产通道。
- **D-35 静态审计**：`converter/validate.py` 新增 `audit_database()`，与 `reconcile` 同入口写 `reports/database_schema_audit.json`；检查 Data type0 的 18 列、44 行、每行 14 个 `numbers` + 4 个 `strings`、字段 raw 顺序。`DATABASE_SCHEMA_AUDIT_OK=True fields=18 rows=44 findings=3 needs_retail=0`，69/69 BASELINE 对账不变。
- **回退列与零售裁决（D-37）**：审计把 `┏ 効果対象` 记录为回退到 field2 `スキルタイプ`（全 0，另有 field3 `消費行動力` 10/11/12），把 `使用可能シーン` 记录为回退到 field6 `筋力影響`（100）。随后从 `Data/BasicData.wolf` 解包零售 `BasicData`，四个核心文件与 `work/all/BasicData` 的 SHA-256 逐一相同，确认 **D-31 不是转换器列错位**，而是当前零售 build 的 `.project` 无这两个标签；CE159 pc115 在该 build 上数据不可达。审计从 `needs-retail-data` 改为 `retail-confirmed-label-absent` / `override-required`，不据此改列，也不把 `効果対象` 别名到 col3。`BattleSkillTargetTest` 的 `<remarks>` 改为标注这是有意的 250 覆盖层夹具，不是零售数据缺口。
- **边界（R7-NEW-03）**：文本槽负缩放仍未拆分——当前 `PictureStage` 的容器镜像会让 `Label` 文字也反向；现阶段没有生产文本槽使用负缩放，故记录为可延后，不在无实机样本时改布局。
- **门禁**：headless `reports/gate_review7.log` 与 `LWsmf_RENDER=1` `reports/gate_review7_windowed.log` 均 `GODOT_GATE_OK` / `GATE_EXIT=0`，30/30；门槛 preflight 先跑 `converter validate` 并断言 `DATABASE_SCHEMA_AUDIT_OK=True`，所以日志同时含 69/69 与审计 marker；整路线仍 `ROUTE_DIFF_OK 73814/73814`。

---

## 5. 下一步（M2 入口）

1. ~~**输入与 UI 骨架**：InputRouter、9 级图层、101/102 自绘与字体度量~~（W2 Day 2–4 已完成，见 §4.5；第三轮审阅 P1 交互/存档缺陷见 §4.6；D-13/D-14 与 KeyCodes/缓存/传送收敛见 §4.7）；GameRoot 已接管标题→新游戏→卧室→F5/F9（§4.8），`project.godot` 主场景已切到 `GameRoot.tscn`（§4.9），旧表现层已退役（§4.13）。
2. **地图呈现**：tile 图层与地形通行已接通（§4.8）；MapChip 为引擎通用素材，正片未使用，调色板占位仅回归辅助；OVERVIEW 保留。
3. **整段 trace**：SampleMapA 事件 0 的 418 帧逐指令对拍（§4.5/§4.9，2461/2461）与整路线 1500 帧（§4.12/§4.23，73814/73814）均已打通；整路线只覆盖标题菜单 1 次选择，白天 101/102 消息/选择仍靠单独用例。
4. **日程/时间**：确定性时钟与 30 天就寝链已打通（§4.10）；HUD 时间已由 `CE217` 的 6200 真实渲染（§4.14）；白天 `CE251/CE310` 的图片菜单点击、确认与日程/体力推进已由 §4.16 端到端覆盖。仍未做“从冷启动进入白天”的剧情夹具。
5. **输入权限**：`123/126` 已按官方三态/设备分支实现并由两侧单测覆盖（§4.15，D-20）；表现层已按 §4.18 接线（D-23）；`123` mask 的逐位表与 `126` target 的具体键对应仍待编辑器差分，不阻塞本作。
6. **故障恢复**：D-13 除锁档/诊断包外已支持显式「返回标题」重建标题安全点（§4.17）；返回标题时会清 `InputRouter` 指针/帧 latch，F9 读档故障进入模态（§4.18）；存档槽级安全点回滚留 M5。
7. **原版 `.sav` 导入**：起始/结束字节与可选 checksum 严格校验、严格 UTF-8 + replacement 告警已落地（§4.20，D-25）；零售 8 槽 `SAVE_IMPORT_OK 8/8`；回写原版 `.sav` 仍不在 M3 范围。
8. **数据库**：`250` 的三容器读写、`.project` 标签漂移、覆盖层与扩展行已落地并由 `DATABASE_TEST_OK .../vm-250` 覆盖（§4.21，D-26）；战斗 `CE152/CE188` 的命令可用性切片已由 §4.22 接到该宿主，完整战斗回合仍需。
9. **战斗命令切片**：`CE188→CE152→CE192` 的空库存/有道具两条线已由 `BattleCommandTest` 覆盖（§4.22，D-27）；真实回合已由 §4.24 接上 CE239/CE242，目标选择与伤害胜负仍未接。
10. **自动事件**：98 个并行事件已由 `AutoEvents` 调度并接入 `GameSession`，整路线同步后两侧均为 73,814 步（§4.23，D-28）；HUD 读档重放之外的引擎接线仍未接。
11. **战斗 wiring**：`CE239` 真实回合经 `CE242` 命令、`350/343/352` 结算并返回已由 `BattleWiringTest` 覆盖（§4.24，D-29）；`CE253` 自然日线、真实 `CE159` 命令 dispatch、伤害矩阵与 H 仍未接。
12. **战斗目标选择**：`CE190/CE175` 的敌人生成/坐标倍率与 `CE162` 的敌方索引 0 选择已由 `BattleTargetTest` 覆盖（§4.25，D-30）；`CE159` 技能选择执行入口、伤害/胜负矩阵仍未接。
13. **技能目标入口**：`CE159` 尾段（场景 1/2、目标 10..12）到 `CE162` 的生成与选中退出已由 `BattleSkillTargetTest` 覆盖（§4.26，D-31）；入口仍从 pc94 注入，生产命令链未接。
14. **H 阈值**：37 个真实 op111 载荷与 59 个阈值/多条件边界已由 `HEventGateTest` 钉住（§4.27，D-32），输出带 `data_only=1`；只到数据面，H 场景与 CE318→CE360 仍未接。
15. **第六轮审阅 P1/P2**：滚轮 126、同文件音频流隔离、未知 soundType 告警、图片容器镜像、严格 effects 对拍与 H 标记已收口（§4.28，D-33/D-34）；AutoEvents 新游戏复位按 D-33 拒绝；Data type0 列错位与 `.sav` 生产严格入口写入契约（D-35）。
16. **第七轮审阅 P1/P2**：故障返回标题重建 `GameSession`/`AutoEvents`，SE 复用缓存流，未知 soundType 按站点去重；D-35 增加 `reports/database_schema_audit.json` 静态审计并把两个 250 回退列标为 `retail-confirmed-label-absent`；零售 BasicData 哈希确认关闭 D-31（§4.29，D-36/D-37）。
17. 决策变化先更新 `docs/godot/DECISIONS.md`，再改实现与本文档。
