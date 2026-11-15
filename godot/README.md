# godot/ — LWSMF Remake（M3 进行中）

标题画面 → 新游戏 → 地图行走 → 消息/选择 → 存档/读档 的 Godot 4.7 C# 切片。所有游戏逻辑走
`core_cs/` 的确定性 VM；`presentation_gd/` 负责显示、输入与真实存档路径，`presentation/` 只剩 C# 桥。

> 阶段：**M1 已完成 (2026-11-14) / M2 已收口，M3 战斗与 H 进行中 (2026-11-15)**。
> 验收证据、存读档续跑边界与下一步见 [`../docs/godot/M1_HANDOFF.md`](../docs/godot/M1_HANDOFF.md)。

## 目录契约

| 目录 | 内容 |
|---|---|
| `core_cs/` | C# 确定性核心：EventVM、WolfRng、ProgramLoader、MapRuntime、Terrain/MapTileData、SaveCodec、GameSession、GameClock、LegacySaveImporter、GameDatabase、AutoEvents、BattleWiring |
| `presentation/` | C# 桥：VmController（IHost/IVmSession）、GameSessionController、TerrainBridge |
| `presentation_gd/` | 生产表现层：GameRoot、PictureStage、MessageWindow、ChoiceWindow、InputRouter、AudioManager、MapLayer、UiSkin |
| `tests/` | headless 测试（烟雾/自动事件/数据库 250/战斗命令切片/战斗目标选择/技能目标入口/H 阈值/会话重建/存档导入/追踪差分/呈现/地图运行/存档往返/地图消息/路线 trace/地形/时钟/30 天 soak 等） |
| `tests_gd/` | GDScript 测试（GameRoot、InputRouter、PictureState、MapLayer、AudioManager、Day/Weekend Menu） |

主场景是 `res://scenes/GameRoot.tscn`；旧的 `TitleScreen`、`ScreenHostView`、`MapView`、`AssetCatalog` 已退役。原版 `.sav` 导入在 `core_cs/LegacySaveImporter.cs` 内校验起始/结束字节，可选严格拒绝 checksum；严格 UTF-8 失败时 replacement 回退并记录 `reports/encoding_warnings.json`。`core_cs/GameDatabase.cs` 承载 `250` 的 CData/Data/Sys 读写与 `.project` 标签解析，`VmController.LoadPrograms` 时加载并暴露 `DatabaseStore`；`tests/BattleCommandTest.cs` 用覆盖层播种跑 `CE188/CE152` 的命令可用性切片。`core_cs/BattleWiring.cs` 接 `CE239/CE242` 真实回合：`340` rising、`CE157` 菜单命令、`CE242`/合成 `CE241` 与 `350/343/352` 结算，`tests/BattleWiringTest.cs` 启动 `CE239` 并等待结算返回；`tests/BattleTargetTest.cs` 用同一批 battle seed 验证 `CE190/CE175` 的敌人生成/坐标倍率与 `CE162` 的索引 0 选择；`tests/BattleSkillTargetTest.cs` 在覆盖层写 `Data 0/2/2=10`、`Data 0/2/6=1` 后从 pc94 驱动 `CE159`，经 pc110/115/244/247 启动 `CE162`，并检查目标 0 不生成、敌数 0 停 pc49/50。`tests/HEventGateTest.cs` 钉住 Rust `m13_h_thresholds.rs` 的 37 个真实 op111 载荷，单条件门测 `value-1`/`value`、多条件门 31 点按 compare 0..6 取 first-true（`H_EVENT_GATE_TEST_OK gates=37 boundaries=59 data_only=1`），只覆盖数据面。`core_cs/AutoEvents.cs` 跟踪 98 个并行事件（跳过标题菜单 43，allow-list 242/243），`GameSession` 在 VM step 前同步、读档后 `PrimeLatched`；`tests/AutoEventsTest.cs` 覆盖边沿/resume/停止。第六轮 P2 收口：`GameRoot._on_wheel` 与 `_on_move` 同查 `BasicAllDeny` 的 126 方向位；`AudioManager.play` 播放前 `duplicate()` 共享 `AudioStream`，BGM/BGS 同文件不再互改 loop；`EventVm` 的 140 对 soundType 3..7 写 `Host.Log` 警告但仍落 BGM；`PictureStage` 用容器 `pivot_offset/scale.x=-1` 镜像文本与占位槽；`oracle/diff_trace.py` 与 `oracle/diff_route.py` 的 effects 统一严格比对。`AutoEvents` 新游戏不复位 latched（D-33），`StopAll` 后再次 Sync 只重放 216/217。第七轮：`presentation/GameSessionController.cs` 的 `Rebuild()` 在故障返回标题时重建 `GameSession`/`AutoEvents` 并递增 `Generation`，`GameRootTest.gd` 断言重开新游戏后 event 38 重启；`AudioManager` 只对 BGM/BGS `duplicate()`，SE 复用缓存流；`EventVm` 140 未知 soundType 按 `(EventId,Pc,SoundType)` 每站点告警一次，`VmTest` 用 `179[2]` 循环两次验证 2 个音效仅 1 条日志。`converter/validate.py` 同入口写 `reports/database_schema_audit.json`（Data type0 18 列/44 行/行宽/raw 顺序，两个 250 回退列记录为 `retail-confirmed-label-absent`，零售哈希见 D-37）；`tools/godot_check.sh` 的门槛 preflight 会先跑 `converter validate` 并断言 `DATABASE_SCHEMA_AUDIT_OK=True`。自然战斗桥：`core_cs/BattleWiring.cs` 的 `HarvestMenuRegistrations` 按 `CData 18/73/0` 游标收取已提交的 CE157 行（不再要求 live CE157），`tests/BattleMenuBridgeTest.cs` 用真实两成员 CE188 菜单收 8 行（前两行技能 26/25），再以 `CE253` 消费队列前两项生成 CE242 pc338/645 并结算 `-1`；headless 无零售 `SaveData01.sav`，输出 `save_parity=0`，四轮 `+1` 仍待 CE143 物化（D-38）。

## 操作

| 键 | 作用 |
|---|---|
| 方向键 / WASD | 地图上移动（地形通行 + active 事件页阻挡；点击相邻格也会走一步） |
| Enter / Z / Space | 决定（标题菜单、消息关闭、动作键） |
| Esc / X | 取消 |
| F5 | 保存到 slot 01（`user://save_01.json`） |
| F9 | 读取 slot 01 |

标题菜单第 2 行（"2"）同样会读取 slot 01。

## 门槛命令（30 道，Windows / CI）

```bash
GODOT=/e/Study/tools/godot-4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe
bash tools/godot_check.sh    # 末尾打印 GODOT_GATE_OK 即全绿
LWsmf_RENDER=1 bash tools/godot_check.sh   # 带窗口的截图/渲染分支
```

单测也可以单独跑：

```bash
"$GODOT" --headless --path godot res://tests/PresentationTest.tscn   # PRESENTATION_TEST_OK phase=bedroom frames=418
"$GODOT" --headless --path godot res://tests/SaveLoadTest.tscn       # SAVE_LOAD_TEST_OK 98/98
"$GODOT" --headless --path godot res://tests/MapMessageTest.tscn     # MAP_MESSAGE_TEST_OK 14/14
"$GODOT" --headless --path godot res://tests/RouteTraceTest.tscn     # ROUTE_TRACE_TEST_OK steps=73814
"$GODOT" --headless --path godot res://tests/Smoke.tscn             # SMOKE_OK + AUTO_EVENTS_TEST_OK + DATABASE_TEST_OK + BATTLE_COMMAND_TEST_OK + BATTLE_WIRING_TEST_OK + BATTLE_TARGET_TEST_OK + BATTLE_SKILL_TARGET_TEST_OK + BATTLE_MENU_BRIDGE_TEST_OK + H_EVENT_GATE_TEST_OK(data_only=1) + unknown-soundType dedupe
"$GODOT" --headless --path godot res://tests_gd/GameRootTest.tscn    # GAME_ROOT_TEST_OK
"$GODOT" --headless --path godot res://tests_gd/PictureStateTest.tscn # PICTURE_STATE_TEST_OK checks=37
"$GODOT" --headless --path godot res://tests_gd/DayMenuTest.tscn      # DAY_MENU_TEST_OK checks=13
"$GODOT" --headless --path godot res://tests_gd/WeekendMenuTest.tscn  # WEEKEND_MENU_TEST_OK checks=9
```

可视化核对可跑 `res://tests_gd/GameRootTest.tscn`（带 `LWsmf_RENDER=1` 时输出截图到 `reports/`）。

## 当前缺口

- MapChip 是 WOLF 通用示例素材，零售正片未使用；`MapLayer` 保留 16 色占位色块，仅作引擎兼容/测试，不接入 `work/all/MapChip`；`Terrain` 通行数据已接。
- 白天 `CE251/CE310` 的图片点击、`102` 自动确认与日程/体力推进已由 `DayMenuTest`/`WeekendMenuTest` 覆盖；仍缺从冷启动进入白天的剧情夹具。
- 整路线 Oracle 已冻结 1500 帧（标题→新游戏→map 1，73,814 步，含 AutoEvents），但还没有消息/选择段与存档/读档的逐指令 trace。
- `123/126` 已按官方三态/设备分支实现（D-20/§4.15）；`123` mask 逐位表与 `126` target 具体键仍待编辑器差分，不阻塞标量键码路径。
- 商店/BGS 之外的 W4 音频三轨、BGS 循环点、音频存档已接；Rust oracle 侧尚未实现 BGS 混音轨。
- 选择窗是居中回退（未用 Sys3/Sys4 锚点），消息控制码只清洗不排版。
- 战斗只覆盖 `CE188/CE152` 命令可用性、`CE239/CE242` 直接入口单轮结算、`CE162` 直启目标选择与 `CE159` 尾段目标入口；`CE253` 自然日线、真实 `CE159` 命令 dispatch、伤害/胜负矩阵仍未接；H 只到阈值数据面（§4.27，输出 `data_only=1`），H 场景与 CE318→CE360 仍待。自动事件已接 98 个并行事件，标题菜单合成仍未接。第六轮 P2 的滚轮 126、音频流隔离、未知 soundType、图片镜像与严格 effects 已收口；`101` 未读完取消与零售字体仍待运行时/零售证据，Data type0 列错位经零售 BasicData 哈希确认不存在（D-37）；第七轮已重建故障后会话、收敛 SE 复制与 soundType 去重，并补 `database_schema_audit.json`（§4.29，D-36），文本槽负缩放（R7-NEW-03）暂延后；`CE188→CE157→CE253` 菜单桥已接通，headless 夹具只跑 2 个 CE242 body 并结算 `-1`（`save_parity=0`，D-38），四轮 `+1`、伤害/胜负矩阵与逃跑仍未接。
