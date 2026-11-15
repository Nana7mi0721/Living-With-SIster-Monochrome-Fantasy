# Godot 重制版当前路线图

> 状态：**现行**
>
> 本文件只保留当前 Godot 阶段的摘要。完整决策、子系统和阶段交接分别见：
> [`docs/godot/DECISIONS.md`](docs/godot/DECISIONS.md)、
> [`docs/godot/PLAN.md`](docs/godot/PLAN.md)、
> [`docs/godot/M1_HANDOFF.md`](docs/godot/M1_HANDOFF.md)、
> [`docs/godot/M0_HANDOFF.md`](docs/godot/M0_HANDOFF.md)、
> [`docs/godot/LEGACY_RUST_AUDIT.md`](docs/godot/LEGACY_RUST_AUDIT.md)。
>
> 旧 Rust 路线图已归档：`docs/archive/PLAN_RUST_2026-11-04.md`。

## 当前状态

- 目标：Godot 4.7.2 .NET，Windows x64；C# 核心 + GDScript 表现层。
- 阶段：**M1 已完成 (2026-11-14) / 进入 M2 主循环与 UI**。
- 最终运行时不使用 Rust；Rust 只作为开发期 Oracle 保留到 M4。
- 内容目标：550 公共事件、73 地图事件、169,831 条真实指令、9 结局与 DLC。
- 存档目标：原版 `.sav` 尽力导入；新版本保存 VM/地图事件/对话/RNG，临时文件+fsync+原子写入。

## M0 达成成果

1. 锁定 Godot 4.7.2 .NET 开发/CI 门限（当前 `tools/godot_check.sh` 12 门限全绿）。
2. 建立 Python 离线数据管道并对账 550/73/169,831 条指令（`reports/reconcile.json` 69/69 全部通过）。
3. 生成 172 项核心资源 SHA256 清单（`game_data/manifest.json`），0 未知 opcode。
4. C# `LegacySaveImporter` 成功解析校验 8 槽原版 `.sav`（`reports/save_import.json`）。
5. 冻结 Oracle trace 契约与 Python/C# 双向对拍机制（`oracle/TRACE_CONTRACT.md`）。
6. 制定 Top 5 高频 opcode 测试计划（`docs/godot/OPCODE_TEST_PLAN.md`）。
7. 输出 M0 阶段交接文档（`docs/godot/M0_HANDOFF.md`）。

## M1 达成成果

1. C# EventVM、MINSTD PRNG、事件调度/等待/调用栈与地图事件页运行时。
2. 纵向切片走通：`标题 CE42 → 新游戏 → map 3 → map 1`，418 帧进入可操作地图。
3. 101 消息 / 102 选择与地图事件 4 联通；地图行走、碰撞、0x08 穿透、PlayerTouch 接触框。
4. JSON `lwsmf.save` v1：变量/地图事件/VM 实例 pc/wait/loops/self/RNG/对话，临时文件+fsync+原子替换；消息与选择中读档不重放。
5. 12 道 headless 门槛全绿：`SAVE_LOAD_TEST_OK 47/47`、`MAP_RUNTIME_TEST_OK 51/51`、`MAP_MESSAGE_TEST_OK 12/12`。详见 [`docs/godot/M1_HANDOFF.md`](docs/godot/M1_HANDOFF.md)。
6. M2 W1 审计加固：CommonSelf、实例上限/未实现 opcode 诊断、schema + `.bak` + 稀疏存档（14.7 KB）、`int.MinValue/-1` 安全除法、移除第二时钟。
7. M2 W2 Day 2–5：GDScript 表现层（InputRouter、9 级图层、101/102 自绘 + 字体度量）、立绘入档与读档重建、Rust↔C# 418 帧逐指令对拍 2461/2461、立绘重建像素一致；门槛 20/20。
8. M2 W3 Day-1：第三轮审阅 4 项 P1 交互缺陷（悬停点选、Cancel 唤醒 VM、123 掩码/滚轮 notch、音量淡入）+ 空槽 Move/负缩放镜像 + `\cdb`/负色 + 立绘路径与 `Pc` 越界校验；`SaveLoadTest` 87/87，门槛 20/20 双模式全绿。
9. M2 W3 Day-2：D-13 Faulted 模态/写档锁/诊断包、D-14 CI 占位夹具、KeyCodes 单一实现（`VmController.KeyCodeForPoll`）、`programs.json` 解析缓存、Teleport 帧边界队列（任意地图）。
10. M2 W3 Day-3：`game_data/map_tiles.json` 导出（564,948 B，6 tilesets / 9 maps）、`Terrain.cs` 四向/quarter 通行度、`MapLayer.gd` 三层 TileMapLayer + 点击/悬停；`TERRAIN_TEST_OK checks=32`、`MAP_LAYER_TEST_OK checks=18`；MapChip 为引擎通用素材，正片未使用；调色板占位仅兼容/测试。
11. M2 W3 Day-4：`core_cs/GameSession.cs` 抽取、`GameSessionController` 桥接、`GameRoot.gd` 接管标题→新游戏→卧室→F5/F9；`SESSION_PARITY_TEST_OK frames=418`（当时新旧 `SaveState` 逐字节相等；夹具已于 2026-11-15 改为 core-only，`checks=9`）；门槛 23/23 双模式全绿。
12. M2 W3 Day-5：`project.godot` 主场景切到 `scenes/GameRoot.tscn`；Rust/C# 逐指令 trace 增 `vars`/`effects`，事件 0 的 418 帧 **2461/2461 全等（428 步 vars、330 步 effects、10 步 str_vars）**，Oracle 夹具与门禁同步更新。
13. M2 W4 Day1-4：`140` 分离 BGM/BGS/SE（修复 43 条 BGS 误判），`AudioManager.gd` 三轨 + 循环点/淡入淡出，`SaveState.audio` 入档；`GameClock.cs` 镜像 `217` 的 13:70 单次进位与跨午夜/周末标志；真实 `CE329→CE317` 连续 30 天就寝 soak 通过；门槛 26/26，`SAVE_LOAD_TEST_OK 94/94`。
14. M2 第四轮审阅加固：300 补 host `call` 副作用、BGS trace 归因 `bgs`、`diff_trace` 比较 `frame` 并增 `str_vars`；存档校验 `MapFile`/`LoopPoint`，非主角 teleport 忽略、队列上限 64；WAV 循环点帧→采样；`TerrainBridge` 让 `MapLayer` 复用唯一 C# 通行度实现。`SAVE_LOAD_TEST_OK 98/98`、`AUDIO_MANAGER_TEST_OK checks=30`、26/26 双模式全绿。
15. M2 整路线 Oracle：Rust `route.rs` 与 C# `RouteTraceTest` 同跑 `TitleMap` event 2 → 新游戏 → map 1 的 1500 帧，**73,812/73,812 步全等**（12,469 步带 effects）；route 末尾 `SaveGame→LoadGame` 状态回路通过；`diff_route.py` 逐字段比较 `frame/map_id/event_id/pc/op/vars/str_vars/effects`。
16. M2 旧表现层退役：`SessionParityTest` 改为 core-only（`oracle/fixtures/session_core_save_state.json`，`checks=9`），新增 `TestSessionView`/`SessionHarness` 并迁移 `SaveLoadTest 98/98`、`PresentationTest`、`MapMessageTest 14/14`；删除 `TitleScreen`/`ScreenHostView`/`ScreenHostSessionView`/`AssetCatalog`/`MapView`/`TitleScreen.tscn`/`ScreenshotTest`，保留 `VmController`/`GameSessionController`/`TerrainBridge`；门禁全绿。
17. M2 图片状态/指针桥：`VmController` 缓存 `PictureCmd` 槽、委派 `124` 给 `PictureStage`（hover/existence/x/y/scale/text/屏幕层），`EngineState(7/8/9)` 左/右/中三态，`GameRoot` 传 `anchor`/`kind`；真实 `CE217` HUD 显示 `8:30`。`PICTURE_STATE_TEST_OK checks=37`，门禁扩到 28/28 双模式全绿（右键/中键见第 21 条）。
18. M2 123/126 输入权限状态模型：`126` 改为「目标位 + 三态（常允许/移动禁/全禁）+ bit28 设备分支」，无自动清除；`123` mask 是复合 UI 位字段而非允许键掩码，改为标量键码后按权限过滤。Rust `InputState` 与 C# `EventVm` 同构，移动/选择分别读 `movement_deny`/`basic_all_deny`；`cargo test -p wolf-vm` 95 passed，route 仍 73,812/73,812，门禁 28/28。
19. M2 白天菜单端到端：`EngineState(7)` 改为官方三态（0/1/2），CE251 从 pc1022 真实菜单点击到全局 48/工作命令，CE310 从 pc128 点击“悠然自得”到 15:30/体力 1；`DAY_MENU_TEST_OK checks=13`、`WEEKEND_MENU_TEST_OK checks=9`，门禁 30/30 双模式全绿（§4.16）。
20. M2 Round-5 审阅 P1：`126` 的 `BasicAllDeny`/`MovementDeny` 接入 `GameSession.MoveLeader`、`AdvanceFrame` 与 `GameRoot` 交互；故障返回标题清 `InputRouter` 指针/帧 latch，F9 读档后检查 `vm.Faulted`；`GAME_ROOT_TEST_OK`，30 步门禁 headless/windowed 均绿（§4.18，D-23）。
21. M3 鼠标右键/中键三态：`0x3000` 的 field 8/9 补齐官方三态（0/1/2），`VmController` 三组 held/边沿同帧推进，`InputRouter` 分别记录右/中按压，故障返回标题一并复位；`PICTURE_STATE_TEST_OK checks=37`、`INPUT_ROUTER_TEST_OK`，30 步门禁 headless/windowed 双绿（§4.19，D-24）。
22. M3 原版 `.sav` 导入严格校验：`LegacySaveImporter` 校验起始/结束字节，`rejectBadChecksum` 可选严格拒绝；`ReadMemDataString` 先严格 UTF-8，失败按 replacement 记 `encoding_warnings`，`ImportAll` 非空时写 `reports/encoding_warnings.json`；`SAVE_IMPORT_OK 8/8`、`SAVE_IMPORT_VALIDATION_OK`，30 步门禁双绿（§4.20，D-25）。


## 下一步（M2 主循环与 UI）

1. ~~InputRouter、9 级图层、101/102 自绘与字体度量~~（W2 Day 2–4 已完成）；~~第三轮审阅 P1 交互/存档缺陷~~（W3 Day-1 已完成）；~~`TitleScreen` 业务切到 `GameRoot.gd`~~（W3 Day-4 已完成）；~~主场景切到 `GameRoot.tscn`~~（W3 Day-5 已完成）。
2. ~~tile 呈现、地形通行、地图点触、角色层~~（W3 Day-3 已完成）；MapChip 为引擎通用素材，正片未使用，调色板占位仅兼容/测试。
3. ~~把“标题→新游戏→map 1”冻结成输入序列~~（整路线 1500 帧 73,812/73,812，§4.12）；消息/选择只覆盖标题菜单 1 次，白天 101/102 与 save/load 的逐指令对拍仍待补。
4. ~~日程/时间接入确定性时钟~~（W4 Day1-4 已完成）；HUD 时间已随图片状态桥显示（§4.14）；~~白天 `CE251/CE310` 工作菜单~~（2026-11-15 已完成端到端点击/日程/体力回归，§4.16；仍缺从冷启动进入白天的剧情夹具）。
5. ~~StrictMode 收口~~（W4 Day5 已完成：GameRoot 强制 true，D-15）；~~旧 `godot/presentation/*.cs` 退役~~（2026-11-15 已完成，`SessionParityTest` 已换 core 夹具）；~~发布期 Faulted 恢复策略~~（2026-11-15 已完成标题安全点恢复，§4.17；存档槽回退留 M5）；`123/126` 已按官方三态/设备分支实现并接到表现层（§4.15/D-20、§4.18/D-23），逐位表待编辑器差分；鼠标右键/中键三态已补齐（§4.19/D-24）；原版 `.sav` 导入严格校验已完成（§4.20/D-25）。

## 约束

- 决策冲突：以 `docs/godot/DECISIONS.md` 为准。
- 路线冲突：以 `docs/godot/PLAN.md` 为准。
- 旧交接、旧研究、旧策略只作历史证据，不作为当前指令。
- 原版素材、`game_data/`、攻略 PDF、外部调研和 vendor 工具不入库、不随发布分发。
