# Godot 重制版执行计划

> 状态：**现行**
>
> 更新：2026-11-14
>
> 当前阶段：**M1 已完成 (2026-11-14) / 进入 M2 主循环与 UI**
>
> 决策前置：[`DECISIONS.md`](DECISIONS.md)

## 1. 目标与非目标

### 目标

- 用 Godot 4.7.2 .NET（Windows x64）重制原版。
- C# 负责确定性核心，GDScript 负责表现层。
- 保留 550 个公共事件、73 个地图事件、完整剧情、9 个结局与 DLC 链。
- 允许重新设计资源和存档格式；原版 `.sav` 只做尽力导入。
- 用 Rust 版本做开发期 Oracle，直到 M4。

### 非目标

- 不把 Rust/GDExtension 作为最终运行时。
- 不在运行时解析 `.wolf/.dat/.mps/.sav`。
- 不手工重写 550 个事件为 GDScript/Dialogic。
- 不用一个 `await` 协程代表一个事件，也不把节点树写进存档。
- 不在第一版追逐逐像素/逐帧完全一致；体验、行为和数据一致优先。

## 2. 里程碑

| 阶段 | 主要交付物 | 退出条件 | 参考工期（单人 / 2–3 人） |
| --- | --- | --- | --- |
| **M0 数据与工具链（已完成）** | Python 转换器、manifest/programs 契约、原版 `.sav` 导入证明、Rust Oracle trace、Godot 空工程与 headless 测试入口 | 550 CE / 73 地图事件 / 169,831 指令对账通过；未知 opcode=0；8 槽 `.sav` 最小导入成功；第一条 Oracle trace diff 可运行 | **已达成 (2026-11-14)** |
| **M1 纵向切片（已完成）** | C# EventVM 骨架、32 位算术/PRNG、前 14 个高频 opcode、原子存档、标题→开场→第一天→读档 | 切片可玩；trace 与 Oracle 一致；存读档不重放、不丢 PC | **已达成 (2026-11-14)** |
| **M2 主循环与 UI** | 9 级图层、101/102 自绘、字体度量、InputRouter、地图点触、BGM/BGS/SE | 历史 UI 问题不再出现；30 天日程可玩；关键截图回归 | 4–6 / 2.5–3.5 周 |
| **M3 战斗与 H** | 剩余 opcode、战斗公式、CE159/162、H 场景与阈值、原版 `.sav` 导入完善 | 战斗/H 与原版数据、顺序和结果一致；未知项有证据 | 5–7 / 3–4 周 |
| **M4 全内容对齐** | 550 事件、111 随机事件、9 结局、DLC、跨天/NG+、30/90 天 soak | 内容覆盖矩阵达标；无 P0/P1 阻塞；Oracle 对拍收口 | 5–7 / 2.5–4 周 |
| **M5 收口与发布** | 性能、导出、转换引导、版权边界、最终回归 | 60fps 稳定；Windows 包可运行；不含原版素材 | 2–3 / 1–1.5 周 |

合计参考：单人 22–32 周；2–3 人 13–19 周。工期是估算，不是承诺。

## 3. M0：数据与工具链

### 3.1 建议目录

```text
converter/                 # Python，离线，不进入游戏发布包
  parse/                   # wolf/dat/mps/save 解析
  compile/                 # branch_depth、跳转表、programs 预编译
  validate/                # 对账、哈希、未映射资源
  export/                  # manifest / programs / database / maps

godot/                     # Godot 4.7.2 .NET 工程
  project.godot
  core_cs/                 # EventVM、WolfRng、SaveCodec、LegacySaveImporter
  presentation/            # GDScript + scenes：UI、InputRouter、AudioManager
  tests/                   # headless、trace、save round-trip

game_data/                 # 本地转换产物；git 忽略；不随发布分发
oracle/                    # Rust trace/夹具/对拍脚本；开发期
```

名称可在 M0 定稿，但 C# 核心、GDScript 表现、转换器和 Oracle 的边界不要混。

### 3.2 M0 任务清单

1. **环境门槛**：安装/验证 Godot 4.7.2 stable .NET 与 C# 构建；建立空工程、headless 启动、CI 命令。
2. **转换器骨架**：定义 `manifest`、`programs`、`database`、`save` 四类 schema。
3. **数据导出**：先支持 `CommonEvent.dat`、`Game.dat`、地图 `.mps` 和图片/字体清单。
4. **指令对账**：550 公共事件、73 地图事件、169,831 条真实指令；未知 opcode、缺失资源、重复 ID 全部出报告。
5. **`.sav` 导入证明**：C# 解析 8 个零售槽位，建立“可恢复字段 / 不可恢复字段”清单。
6. **Oracle**：从 Rust 导出固定种子的事件 trace；定义 frame/pc/op/vars/effects 的稳定格式。
7. **Godot 测试入口**：C# 单元测试 + Godot headless 冒烟 + 一条 trace diff。
8. **实机证据**：本地录制 `opcode 123` 掩码/键码和 `CE159/162` 战斗目标流程；未取得前标记 `needs-runtime`。

### 3.3 M0 退出标准

- [x] `godot/` 空工程可打开、可 headless 启动、可导出一个最小场景。（见 `tools/godot_check.sh` 冒烟场景 `res://tests/Smoke.tscn`）
- [x] 转换器对 550/73/169,831 数量对账通过。（见 `reports/reconcile.json` 69/69 全部通过）
- [x] 未知 opcode = 0；缺失资源有白名单和报告；manifest 含 SHA256。（见 `game_data/manifest.json`，172 资源追踪）
- [x] 8 个原版 `.sav` 至少能读取基础变量/图片/时间；不支持的字段有明确报告。（见 `godot/tests/SaveImportTest.cs` 与 `reports/save_import.json`）
- [x] Rust trace 与 Godot 空壳 trace 的字段契约冻结。（见 `oracle/TRACE_CONTRACT.md` 与 `oracle/diff_trace.py`）
- [x] `D-07` 的 `.sav` 限制写进导入报告和 UI 文案。（见 `LegacySaveImporter.cs` 与 `reports/save_import.json`）
- [x] 第一批高频 opcode（至少 `401,210,121,111,180`）有可执行测试计划。（见 `docs/godot/OPCODE_TEST_PLAN.md`）
- [x] `docs/godot/M0_HANDOFF.md` 记录证据、边界、遗留项和下一阶段入口。（见 `docs/godot/M0_HANDOFF.md`，状态：已完成）

## 4. M1 首个纵向切片（已完成）

> 交接证据与边界：[`M1_HANDOFF.md`](M1_HANDOFF.md)

切片范围固定为：

`标题 → 新游戏 → 开场事件 → 第一天卧室 HUD → 消息/选择 → 存档 → 读档继续`

验收不是“画面看起来像”，而是：

- 输入序列相同；
- 事件实例的 `program_id/pc/call_stack/wait` 相同；
- 变量 delta 相同；
- RNG 序列相同；
- 读档后不重放已消费事件；
- 选择分支不丢事件。

**达成情况（2026-11-14）**

- [x] 标题 CE42 → 新游戏 → map 3 → SampleMapA（artifact 1005）的确定性链条，418 帧进入可操作地图。
- [x] 地图事件页条件、autorun/parallel、ActionKey、PlayerTouch 接触框、active 页碰撞与 0x08 穿透。
- [x] 101 消息 / 102 选择真实联通（SampleMapA 事件 4），窗口位于地图层之上。
- [x] 存档保存变量、地图事件状态、VM 实例 pc/return/wait/loops/self、RNG 与对话；临时文件+fsync+原子替换。
- [x] 消息中、选择中存档后分歧再读档，实例 id/pc/wait 一致，不重放 101/102；标题菜单可读档继续。
- [x] 门槛 23/23：`PRESENTATION_TEST_OK`、`MAP_RUNTIME_TEST_OK 51/51`、`SAVE_LOAD_TEST_OK 87/87`、`MAP_MESSAGE_TEST_OK 14/14`、`PICTURE_ROUNDTRIP_TEST_OK`、`ORACLE_STEP_TRACE_OK` + `TRACE_DIFF_OK 2461/2461`、`GODOT_GATE_OK`。
- [x] M2 W1 审计加固（2026-11-14）：CommonSelf 跨调用持久化、未知事件/实例上限 4096/未实现 opcode 诊断（StrictMode）、schema 校验 + `.bak` 回退 + 稀疏 Self（实测 14.7 KB）、`int.MinValue/-1` 安全除法、移除 `AutoProcess` 第二时钟并冻结 `VmBridge` 接口、选择索引 clamp；Smoke 新增 6 项、SaveLoadTest 47/47。
- [x] M2 W2 Day 2–5（2026-11-14）：GDScript 表现层 `godot/presentation_gd/`（GameRoot/InputRouter/PictureStage/AssetDB/TextTokenizer/MessageWindow/ChoiceWindow/Nameplate/UiSkin）+ 6 个 `tests_gd` 门槛；101 窗口 960×168 自绘、`\f[n]` 字号、`<L>/<C>/<R>` 行对齐与 `\E` 描边、102 选择窗 Sys3/Sys4 定位、GameRootTest 覆盖 GameRoot 信号连线；立绘 `SaveState.Pictures` 快照与读档重建（SaveLoadTest 76/76）；Rust `--step-trace` 与 C# `OracleStepTraceTest` 418 帧 2461/2461 逐指令全等。
- [x] M2 W3 Day-1 第三轮审阅 P1 修复（2026-11-14）：鼠标悬停点选、Cancel 唤醒 VM、123 掩码/滚轮 notch、140 音量与淡入、空槽 Move 与负缩放镜像、`\cdb`/负色索引、立绘路径穿越与读档 `Pc` 越界；`SaveLoadTest` 87/87，`GAME_ROOT_TEST_OK`（详见 M1_HANDOFF §4.6）。
- [x] M2 W3 Day-2（2026-11-14）：D-13 Faulted 模态/存档写锁/诊断包、D-14 CI 占位夹具（显式根不回退）、KeyCodes 单一实现（`VmController.KeyCodeForPoll`，删除 GDScript 镜像）、`programs.json` 解析缓存（路径+长度+mtime）、Teleport 帧边界队列（map 3 新游戏链、其余地图直切）；`SaveLoadTest` 87/87（详见 M1_HANDOFF §4.7）。
- [x] M2 W3 Day-3 地图呈现（2026-11-14）：`converter/parse/mps.py` 修 `layers` 丢失后导出 `game_data/map_tiles.json`（564,948 B，6 tilesets / 9 maps）；`Terrain.cs` 镜像 Rust 四向/quarter 通行度，`MapRuntime.CanEnter(x,y,step)`；`MapTileData.cs` 缓存解码；`MapLayer.gd` 三层 `TileMapLayer` + 调色板占位 + 队长层 + 点击/悬停信号；`TERRAIN_TEST_OK checks=32`（Dungeon 348/SampleMapB 160）、`MAP_LAYER_TEST_OK checks=18`。MapChip 是 WOLF 通用示例素材，零售正片未使用；调色板占位仅兼容/测试。
- [x] M2 W3 Day-4 GameSession 接管（2026-11-14）：`core_cs/GameSession.cs`（`IVmSession`/`IGameSessionView`，标题菜单、传送队列、存档构建、`EnterMap`、实例恢复全部迁入）；`TitleScreen` 变薄适配层 + `ScreenHostSessionView`；`GameSessionController` 桥到 `GameRoot.gd`，标题→新游戏→卧室→F5/F9 全链已由 GDScript 驱动；修复 `user://` 存档路径本地化、读档恢复 `PlayFrames`、脚本回放键码答 OK、F9 帧数同步；`SESSION_PARITY_TEST_OK checks=11 frames=418`（新旧 `SaveState` 逐字节相等）。
- [x] M2 W3 Day-5 主场景与 vars/effects trace（2026-11-14）：`project.godot` 的 `run/main_scene` 切到 `res://scenes/GameRoot.tscn`（旧 `TitleScreen.tscn` 已于 §4.13 退役），headless 启动无 Parse Error；Rust `StepTraceEntry` 增 `vars/effects`（`Vm::trace_before` + `finalize_trace`/`diff_snapshots`，系统变量地址 `9_000_000+n`、Self 地址 `1_000_000+group*100_000+slot`），C# `OracleStepTraceTest` 同步导出；两侧 **2461/2461 步全等，428 步带 vars、330 步带 effects、10 步带 str_vars**，`diff_trace.py` 对 `frame/event_id/pc/op/vars/str_vars/effects` 逐字段比较通过，夹具为本次 Rust 输出（第四轮审阅后刷新，sha256 `3c6f29e4…`）。
- [x] M2 W4 Day1-4 音频与时钟（2026-11-15）：`140` 按 `(bits>>4)&7` 分离 BGM/BGS/SE（旧实现把 43 条 BGS 当 BGM），`SoundCmd` 带 `LoopPoint/Tempo`；`AudioManager.gd` 三轨独立、循环点/淡入淡出、`snapshot/restore`；`SaveState.audio`（最多 3 轨）入档并经 `IGameSessionView` 往返；`GameClock.cs` 镜像 `217` 的单次 if 进位（+130 分 → 13:70）与跨午夜/周末标志；真实 `CE329→CE317` 连续 30 天就寝 soak 到第 31 天全部通过（`days=30 checks=61`）。门槛 26/26，`SAVE_LOAD_TEST_OK 94/94`。
- [x] M2 第四轮审阅加固（2026-11-15）：300 补 `Host.CommonEventCall`；BGS trace 不再写成 `se`；`diff_trace` 比较 `frame` 且两侧 trace 增 `str_vars`（普通/系统/Self 字符串地址空间）；`SaveCodec` 校验 `MapFile` 与 `LoopPoint`；非主角 teleport 忽略、队列上限 64；WAV `loop_begin` 帧→采样换算；`TerrainBridge.cs` 让 `MapLayer` 复用唯一 C# `Terrain` 实现并接通点击/悬停。`SAVE_LOAD_TEST_OK 98/98`、`AUDIO_MANAGER_TEST_OK checks=30`、26/26 双模式全绿（§4.11）。
- [x] M2 整路线 Oracle（2026-11-15）：Rust `route.rs` 与 C# `RouteTraceTest` 都从 `TitleMap` event 2 起步，固定脚本输入跑 1500 帧；初版修掉 database 副作用与领队夹取后为 **73,812/73,812**。D-39/D-40 进一步把 map id 改为 SysDB 行号语义（`SampleMapA` artifact 1005）并补 Autorun 守卫，两侧收敛为 **149,247/149,247**。`diff_route.py` 支持 `.gz` 夹具并逐字段比较 `frame/map_id/event_id/pc/op/vars/str_vars/effects`；route 末尾另做 `SaveGame→前进→LoadGame` 状态回路断言（§4.12/§4.31）。
- [x] M2 图片状态/指针桥（2026-11-15）：`VmController` 缓存 `PictureCmd` 槽并委派 `124` 给 `PictureStage`（hover/existence/x/y/scale/text/屏幕层），`EngineState(7)` 为单帧左键，`GameRoot` 传 `anchor`/`kind`；真实 `CE217` HUD 渲染 `8:30`。`PICTURE_STATE_TEST_OK checks=30`，门禁 28/28（§4.14）。
- [x] M2 123/126 输入权限状态模型（2026-11-15）：据 WOLF 编辑器帮助，`126` 改为“目标位 + 三态（常允许/移动禁/全禁）+ bit28 设备分支”，无自动清除；`123` mask 是复合 UI 位字段而非允许键掩码，改为拿标量键码后按权限过滤。Rust `InputState`（`apply_permission`/`filter_code`）与 C# `EventVm`（`ApplyInputPermission`/`FilterKeyCode`）同构，移动/选择分别读 `movement_deny`/`basic_all_deny`；`cargo test -p wolf-vm` 95 passed，`VmTest` 覆盖三态/设备/`125` 穿透；route 仍 73,812/73,812，门禁 28/28（§4.15，D-20）。
- [x] M2 旧表现层退役（2026-11-15）：`SessionParityTest` 改为 core-only（`GameSession` vs `oracle/fixtures/session_core_save_state.json`，`checks=9`）；新增 `TestSessionView`/`SessionHarness`，`SaveLoadTest 98/98`、`PresentationTest`、`MapMessageTest 14/14` 全部迁到 core；删除 `TitleScreen.cs`、`ScreenHostView.cs`、`ScreenHostSessionView.cs`、`AssetCatalog.cs`、`MapView.cs`、`TitleScreen.tscn` 与 `ScreenshotTest`。保留 `VmController`/`GameSessionController`/`TerrainBridge`；`godot/README.md` 同步为 M2 现状。门禁 28/28（§4.13）。
- [x] M2 白天菜单端到端（2026-11-15）：`EngineState(7)` 改为官方三态（0/1/2），CE251 从 pc1022 真实菜单点击到全局 48/工作命令，CE310 从 pc128 点击“悠然自得”到 15:30/体力 1；`DAY_MENU_TEST_OK checks=13`、`WEEKEND_MENU_TEST_OK checks=9`，门禁当时 30 步双模式（§4.16）。
- [x] M2 Faulted 标题安全点恢复（2026-11-15）：故障模态新增「返回标题（放弃当前进度）」；`restart_after_fault()` 停 VM、清诊断/指针/窗口/图片/音频/地图层后重建标题事件，`frames` 归零，不写档；`StartTitle` 同时清对话/选择/传送队列。GameRootTest 断言 latch 清除、phase title、模态隐藏、帧归零（§4.17，D-13）。
- [x] M2 Round-5 审阅 P1（2026-11-15）：`126` 的 `BasicAllDeny`/`MovementDeny` 接到 `GameSession.MoveLeader`、`AdvanceFrame` 与 `GameRoot` 交互；故障「返回标题」同清 `InputRouter.pointer_pressed`/帧 latch，F9 读档后检查 `vm.Faulted`。`GAME_ROOT_TEST_OK`，30 步门禁 headless/windowed 均绿（§4.18，D-23）。
- [x] M3 鼠标右键/中键三态（2026-11-15）：`0x3000` 的 field 8/9 补齐官方三态（0/1/2），`VmController` 三组 held/边沿同帧推进，`InputRouter` 分别记录右/中按压，故障返回标题一并复位；`PICTURE_STATE_TEST_OK checks=37`、`INPUT_ROUTER_TEST_OK`，30 步门禁 headless/windowed 双绿（§4.19，D-24）。
- [x] M3 原版 `.sav` 导入严格校验（2026-11-15）：`LegacySaveImporter` 校验起始/结束字节，`rejectBadChecksum` 可选严格拒绝；`ReadMemDataString` 先严格 UTF-8，失败按 replacement 记 `encoding_warnings`，`ImportAll` 在非空时写 `reports/encoding_warnings.json`；`SAVE_IMPORT_OK 8/8`、`SAVE_IMPORT_VALIDATION_OK`，30 步门禁双绿（§4.20，D-25）。
- [x] M3 `250` 数据库宿主（2026-11-15）：新增 `GameDatabase`，读 `database.json` 的 `cdata/database/system` 三容器与 `work/all/BasicData/*.project` 标签；脚本写走覆盖层，支持行数/清行/首空行/扩展行与 8 种写算子，数字写不覆盖文本。`VmController` 在 `LoadPrograms` 时加载并暴露 `DatabaseStore`，`EventVm` 的 `250` 委派 host。`DATABASE_TEST_OK .../vm-250`，30 步门禁双模式全绿（§4.21，D-26）。
- [x] M3 AutoEvents 调度器与 route 同步（2026-11-15）：`Program` 解析 `run_condition_op_var_val`，新增 `AutoEvents` 跟踪 98 个并行事件（跳过标题菜单 43，allow-list 242/243），边沿触发、resume 重放与 `PrimeLatched`；`GameSession` 在 `StepFrame` 前同步、读档后 prime。Rust `route.rs` 同序同步后整路线由 73,812 增至 73,814 步，当时两侧 `ROUTE_DIFF_OK 73814/73814`（D-39/D-40 后为 149,247/149,247）；`AUTO_EVENTS_TEST_OK`，当时 30 步门禁双绿（§4.23，D-28）。
- [x] M3 战斗命令可用性切片（2026-11-15）：`CE188` 经 `CE152` 构建真实攻击命令列表；`BattleCommandTest` 用数据库覆盖层播种队伍/敌数/命令槽/领队属性，空库存时四条真实攻击行不可选，`CData 7/0/0=1` 后可选，两条线都到 CE192 且不生成 CE244。`BATTLE_COMMAND_TEST_OK control_real=0 seeded_real=1`，30 步门禁双模式全绿（§4.22，D-27）。
- [x] M3 CE239/CE242 战斗 wiring（2026-11-15）：新增 `BattleWiring` 把 `340` rising、`CE157` 菜单命令、`CE242`/合成 `CE241` 与 `350/343/352` 结算接进 `GameSession` 帧序（AutoEvents → BattleWiring → VM step）；`BattleWiringTest` 以 26 个 battle seed 启动 `CE239`，`BATTLE_WIRING_TEST_OK rounds=1 result=-1 ce242_pc=415`，30 步门禁双绿（§4.24，D-29）。
- [x] M3 CE190/CE175 敌人生成与 CE162 目标选择（2026-11-15）：`GameDatabase` 补引擎默认坐标倍率 32/24 与窗口皮肤，读序为覆盖层 → 引擎层 → 文件层；`CE190` 生成敌人后 `CE175` 写 512/-288/512/312，`CE162` 选中索引 0 并在 pc57 退出，无敌人时停 pc49/50。`BattleTargetTest` 的脉冲改为按帧号，输出 `BATTLE_TARGET_TEST_OK selected=1 control_pc=49 enemy_hp=100`，30 步门禁双绿（§4.25，D-30）。
- [x] M3 CE159 技能尾段与 CE162 目标入口（2026-11-15）：CE159 的 pc110/115 读场景与效果目标，pc244 放行 10/11/12，pc247 启动 CE162；覆盖层写 `Data 0/2/2=10`、`Data 0/2/6=1` 后 `BattleSkillTargetTest` 从 pc94 验证到 CE162 pc57 退出，目标 0 不生成、敌数 0 停 pc49/50。`BATTLE_SKILL_TARGET_TEST_OK selected=1 no_target=1 no_enemy_pc=50`，30 步门禁双绿（§4.26，D-31）。
- [x] M3 H 阈值数据面（2026-11-15）：把 Rust `m13_h_thresholds.rs` 的 37 个真实 op111 载荷钉进 `HEventGateTest`，逐条断言 event/pc/params；单条件门测 `value-1`/`value`，多条件门 31 点按 compare 0..6 取 first-true。`H_EVENT_GATE_TEST_OK gates=37 boundaries=59 data_only=1`，30 步门禁双绿（§4.27，D-32）。
- [x] M3 第六轮审阅 P1/P2 收口（2026-11-15）：`GameRoot._on_wheel` 补 126 方向位过滤，`AudioManager` 播放前 duplicate 共享流，`EventVm` 140 对未知 soundType 3..7 告警，`PictureStage` 以容器 pivot/scale 镜像，`diff_trace/diff_route` 统一严格 effects；AutoEvents 新游戏复位按 D-33 拒绝。`AUDIO_MANAGER_TEST_OK checks=32`、`PICTURE_ROUNDTRIP_TEST_OK`、`SESSION_PARITY_TEST_OK checks=10 frames=418`，30 步门禁双模式双绿（§4.28，D-33/D-34/D-35）。
- [x] M3 CE188→CE157→CE253 自然战斗菜单桥（2026-11-15）：`BattleWiring` 改为按 `CData 18/73/0` 游标 harvest 已提交的 CE157 行；`BattleMenuBridgeTest` 用真实 CE188 两成员菜单收 8 行（前两行技能 26/25），再以 `CE253` 消费队列前两项，生成 CE242 pc338/645 并结算 `-1`。`BATTLE_MENU_BRIDGE_TEST_OK rows=8 real=2 rounds=2 turns=1 result=-1 pcs=338,645 save_parity=0`，30 步门禁双绿（§4.30，D-38）。
- [x] M3 第七轮审阅 P1/P2 与 D-35 审计（2026-11-15）：故障返回标题经 `GameSessionController.Rebuild()` 重建会话与 AutoEvents（`Generation` 递增，重开新游戏后 event 38 重启）；`AudioManager` 仅 BGM/BGS 复制流、SE 复用缓存；`EventVm` 未知 soundType 每站点告警一次。`converter/validate.py` 增写 `reports/database_schema_audit.json`（18 列/44 行/行宽/raw 顺序，两个 250 回退列记录为 `retail-confirmed-label-absent`）。`GAME_ROOT_TEST_OK`、`AUDIO_MANAGER_TEST_OK checks=32`、`DATABASE_SCHEMA_AUDIT_OK fields=18 rows=44 findings=3 needs_retail=0`，30 步门禁双绿（§4.29，D-36）。

- [x] M3 SysDatabase 地图语义、Autorun 守卫与冷启动日线（2026-11-15）：`op130` 的 map 参数确认为 `SysDatabase` type0 行号（`SampleMapA` row1、`SampleMapA_1` row2、`Map003` row3），C# `GameSession.EnterMap` 经 `VmController.MapPath → GameDatabase.ReadText(Sys,0,mapId,0)` 取路径，Rust `route.rs` 同步 `Databases::map_path`/`artifact_id_for_file`。`MapRuntime.StartPending` 补 `StartsOnPage>0 && !IsLooping` 守卫，清除 C# 比 Rust 多出的 2,176 步 Autorun 重启，整路线 **149,247/149,247**，oracle 夹具与 `SessionParity` 夹具按 map1005 重生成。新增 `ColdStartDayTest`（生产 `GameRoot`）：标题→新游戏→卧室（断言 map1005）→day1 18:30→点姐姐→睡眠选择→day2 08:30，`COLD_START_DAY_TEST_OK day=2 hour=8 minute=30 weekday=1 pictures=60`。门禁扩到 **31 步**，headless/windowed 双绿（§4.31，D-39/D-40）。

- [x] M3 队列图片状态与 CE253 自然日线（2026-11-16）：`VmController.QueuePictureSignals` + `TakePicturesJson()` 把 `Picture` 命令改为 JSON 队列，`PictureStage.track_picture` 只维护 `_cmds`/`_base_sizes` 与命中框几何，不创建 Control，消除 headless 长跑 `AccessViolationException` 并保留 opcode124 的 hover/点击。`Ce253DayRouteTest` 走生产 `GameRoot`：冷启动→day2/day3 真实 CE251→day4 自然 CE253/CE239/CE242→每帧观测 CE244 瞬态→day5 08:30，输出 `CE253_DAY_ROUTE_TEST_OK day=5 hour=8 minute=30 rounds=2 turns=1 result=1 seeded=34`（`peak342=45`、`peak352=1`、`saw343=true`）。门禁扩到 **32 步**（§4.33，D-42）。

- 未完成边界：逐指令 Oracle 现有两条线——SampleMapA 事件 0 的 418 帧（2461 步）与整路线 1500 帧（149,247 步，map1005）；整路线只含标题菜单 1 次选择，没有 op101 消息，save/load 只做状态回路而不逐指令对拍；`wait` 与未建模的 130/160/290 转场不入 effects；MapChip 为引擎通用素材，正片未使用（D-21）；30 天 soak 只走就寝/日历链，白天 `CE251/CE310` 图片菜单已由 §4.16 端到端覆盖，冷启动→day1 18:30→day2 08:30 已由 `ColdStartDayTest` 覆盖（§4.31），但未跑完整 30 天；旧表现层已退役（§4.13）；`123/126` 已按官方三态/设备分支实现（§4.15，D-20），但 `123` mask 的逐位表与 `126` target 的具体键/设备对应仍待编辑器二进制差分；战斗侧命令可用性由 CE188/CE152 覆盖，真实回合由 CE239/CE242 wiring 覆盖（§4.24），但 `CE253` 自然日线、真实 `CE159` 命令 dispatch、伤害/胜负矩阵仍缺（`CE162` 直启由 §4.25 覆盖，`CE159` 尾段由 §4.26 覆盖）；98 个自动事件已接入 `GameSession`（§4.23，D-28），标题菜单合成仍未接；H 只到阈值数据面（§4.27，D-32），CE318→CE360 与 CE310/323/335 剧情调用仍未接；第六轮 P2 的滚轮 126、音频流隔离、未知 soundType、图片镜像与严格 effects 已收口（§4.28，D-34），`101` 未读完取消与零售字体仍待运行时/零售证据；Data type0 的 `col2`/`col3` 已由零售 BasicData 哈希确认无列错位（D-37）；`database_schema_audit.json` 记录 18 列/44 行结构与 `効果対象`/`使用可能シーン` 回退列，不据此改列（D-36）；文本槽负缩放（R7-NEW-03）暂延后；CE188→CE157 菜单桥已用游标 harvest 接通；零售存档导入（800 vars + 16,800 CData cells）与 `PrimeLatched` 后 CE253 已跑满四轮 `+1`，弱队场景 10 轮 `-1`（§4.34/D-43）；地图语义（D-39）与 Autorun 守卫（D-40）已收口，`SaveLoadTest`/`MapMessageTest` 的事件 4 page4 条件为 63=1/62=0/464=0/60=0（§4.31）。页切换时 `StartsOnPage` 已随新页复位（D-41，`11d071d`）；生产 `Ce253DayRouteTest` 已自然走到 day5 08:30，CE253→CE239→CE242 自然触发并跑 2 轮，CE244 以 `v342=0` 直接判胜（瞬态 `352=1/343=1`，战后清零），真实伤害矩阵/四轮 `+1` 已接（§4.34/D-43），第三态 `-2` 需四个 action pool 全空、双人夹具不可达，记为边界。

## 5. 测试与验收

| 层 | 测试 | 通过标准 |
| --- | --- | --- |
| L1 数据 | 转换器对账、哈希、schema、资源引用 | 数量和 ID 100% 可解释 |
| L2 行为 | C# headless VM trace vs Rust Oracle | 核心路线/随机事件/战斗/H 变量与副作用一致 |
| L3 表现 | Godot 场景截图、输入注入、多帧状态 | 无截断/重叠/闪烁/立绘丢失；关键图有 diff |
| L4 内容 | 550 事件、111 随机事件、9 结局、DLC、NG+、30/90 天 soak | 覆盖矩阵全部有状态，无静默未测 |

第一版 DoD：数据一致、行为一致、表现一致；像素级一致作为后续优化目标。

## 6. 下一步

M1 已完成，M2 W1 审计修复、W2 Day-1 第二轮审计止血、W2 Day 2–5 表现层/逐指令对拍、W3 Day-1 第三轮审阅 P1 修复、W3 Day-2 核心收敛、W3 Day-3 地图呈现、W3 Day-4 GameSession 接管、W3 Day-5 主场景与 vars/effects trace、W4 Day1-4 音频三轨与确定性时钟、第四轮审阅加固、整路线 Oracle、图片状态/指针桥、旧表现层退役、`123/126` 输入权限状态模型、白天菜单端到端、AutoEvents 调度器、SysDatabase 地图语义/Autorun 守卫/冷启动日线已完成（2026-11-15）：

0. ~~M2 W1：M1 审计 P0/P1/P2 加固~~（已完成，见 §4）。
1. ~~W2 Day-1：存档语义/原子备份、全 opcode 覆盖、StrictMode 门禁~~（已完成，见 M1_HANDOFF §4.4）。
2. ~~InputRouter、9 级图层、101/102 自绘与字体度量~~（W2 Day 2–4 已完成）；~~第三轮审阅 P1 交互/存档缺陷~~（W3 Day-1 已完成）；~~`TitleScreen` 业务切到 `GameRoot.gd`~~（W3 Day-4 已完成，§4.8）；~~主场景切到 `GameRoot.tscn`~~（W3 Day-5 已完成，§4.9）；
3. ~~tile 呈现与地形通行、地图点触、角色层~~（W3 Day-3 已完成，§4.8）；MapChip 为引擎通用素材，正片未使用；调色板占位仅兼容/测试；
4. ~~把“标题→新游戏→SampleMapA（1005）→消息/选择→存档/读档”冻结成输入序列~~（整路线 1500 帧 149,247/149,247，§4.31/D-39）；冷启动→day2 08:30 已有 `ColdStartDayTest`；消息与白天选择仍靠单独用例）；
5. ~~日程/时间接入确定性时钟~~（W4 Day1-4 已完成，§4.10）；~~白天 CE251/CE310 工作菜单与 HUD 时间~~（2026-11-15 已完成，§4.16）；~~从冷启动进入白天~~（`ColdStartDayTest` 覆盖到 day2 08:30，§4.31）；
6. ~~StrictMode 收口与旧表现层退役~~（StrictMode 已强制 true，D-15；旧表现层 2026-11-15 退役，§4.13）；~~`123/126` 输入权限~~（2026-11-15 已按官方三态/设备分支实现，§4.15，D-20）；~~AutoEvents 调度器~~（2026-11-15 已接 98 个并行事件并同步 route，§4.23，D-28）；~~发布期 Faulted 恢复策略~~（2026-11-15 已完成标题安全点恢复，§4.17；存档槽回退留 M5）。
7. 决策变化先更新 `DECISIONS.md`。
