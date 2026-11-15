# Godot 重制版（现行入口）

> 状态：**M1 已完成 (2026-11-14) / 进入 M2 主循环与 UI**
>
> 最后整理：2026-11-15
>
> 引擎：Godot 4.7.2 stable Mono (.NET 8.0)，Windows x64。
> 本目录是 Godot 重制阶段的唯一技术规范与文档入口。

## 1. 先读什么

| 顺序 | 文档 | 用途 |
| --- | --- | --- |
| 1 | [`DECISIONS.md`](DECISIONS.md) | 已拍板决策（D-01..D-18）、`.sav` 边界、技术栈口径 |
| 2 | [`PLAN.md`](PLAN.md) | M0–M5 总体路线、当前阶段任务与退出标准 |
| 3 | [`M1_HANDOFF.md`](M1_HANDOFF.md) | **M1 验收交接**：切片运行链、存读档续跑、证据与边界 |
| 4 | [`M0_HANDOFF.md`](M0_HANDOFF.md) | **M0 验收交接文档**：环境门禁、对账证据、`.sav` 导入、Oracle 对拍 |
| 5 | [`DATA_CONTRACT.md`](DATA_CONTRACT.md) | 离线转换数据契约（manifest / programs / database / save） |
| 6 | [`OPCODE_TEST_PLAN.md`](OPCODE_TEST_PLAN.md) | Top 5 高频 opcode（`401, 210, 121, 111, 180`）测试与执行规约 |
| 7 | [`LEGACY_RUST_AUDIT.md`](LEGACY_RUST_AUDIT.md) | 22 项旧 Rust 缺陷与 Godot 护栏 |
| 8 | [`../../oracle/TRACE_CONTRACT.md`](../../oracle/TRACE_CONTRACT.md) | Rust Oracle 与 Godot EventVM 对拍格式规范 |
| 9 | [`../README.md`](../README.md) | 全仓文档总索引 |

## 2. 当前技术结论

- 引擎：Godot 4.7.2 stable（.NET），Windows x64。
- 运行时：C# 确定性核心 + GDScript 表现层；最终版本不含 Rust/GDExtension。
- 数据：Python 离线转换器；游戏运行时只读 `game_data/`，不解析 `.wolf/.dat/.mps`。
- 内容：550 个公共事件、73 个地图事件、169,831 条真实指令、9 个结局与 DLC。
- 存档：原版 `.sav` 尽力导入；新版本保存 VM/地图事件/对话/RNG/立绘/音频三轨，原子写入。
- 验证：Rust Oracle 保留到 M4；Godot 侧做 trace、截图和全内容覆盖回归。

完整口径见 [`DECISIONS.md`](DECISIONS.md)。

## 3. 当前状态

**已完成（M0 + M1）**

- M0：Godot 4.7.2 .NET 骨架、Python 数据管道 550/73/169,831 对账、8 槽 `.sav` 导入、Oracle 契约与 CE42 对拍、172 项 SHA256。详见 [`M0_HANDOFF.md`](M0_HANDOFF.md)。
- M1：C# EventVM、MINSTD PRNG、事件调度/等待/调用栈、地图事件页运行、101/102 消息选择、地图行走/碰撞/PlayerTouch。
- M1 切片：标题 CE42 → 新游戏 → map 3 → map 1，418 帧进入可操作地图；12 事件、3 个自动并行页。
- M1 存档：JSON `lwsmf.save` v1，保存变量/地图事件/VM 实例 pc/wait/loops/self/CommonSelf/RNG/对话；Self 槽位稀疏编码，schema 校验与 `.bak` 回退，临时文件+fsync+原子替换；消息与选择中读档不重放；地图 1 实测约 14.7 KB。
- M2 W1 审计加固：CommonSelf 持久化、未知事件/实例上限 4096/未实现 opcode 诊断、`int.MinValue/-1` 安全除法、移除 `AutoProcess` 并冻结 `VmBridge`、选择索引 clamp。
- M2 W2 Day 2–5：GDScript 表现层（InputRouter、9 级图层、101/102 自绘、字体度量、384 MB 资产 LRU）、立绘入档与读档重建、Rust↔C# 418 帧逐指令对拍 2461/2461。
- M2 W3 Day-1：第三轮审阅 P1 修复（鼠标悬停点选、Cancel 唤醒 VM、123 掩码/滚轮、音量淡入、空槽 Move/负缩放镜像、`\cdb` 与负色索引、立绘路径穿越与读档 `Pc` 越界），`SaveLoadTest` 87/87。
- M2 W3 Day-2：D-13 Faulted 模态/写档锁/诊断包、D-14 CI 占位夹具、KeyCodes 单一实现（`KeyCodeForPoll`）、`programs.json` 解析缓存、Teleport 帧边界队列。
- M2 W3 Day-3：`game_data/map_tiles.json`（564,948 B，6 tilesets / 9 maps）、`Terrain.cs` 四向/quarter 通行度、`MapLayer.gd` 三层 TileMapLayer + 点击/悬停；`TERRAIN_TEST_OK checks=32`、`MAP_LAYER_TEST_OK checks=18`（MapChip 是引擎通用素材，正片未使用；调色板占位仅兼容/测试）。
- M2 W3 Day-4：`core_cs/GameSession.cs` 抽取 + `GameSessionController` 桥接，`GameRoot.gd` 接管标题→新游戏→卧室→F5/F9；`SESSION_PARITY_TEST_OK checks=9 frames=418 core=frozen`（core `SaveState` 与冻结夹具逐字段相等）。
- M2 W3 Day-5：`project.godot` 主场景切到 `scenes/GameRoot.tscn`（旧 `TitleScreen.tscn` 已于 2026-11-15 退役）；Rust/C# 逐指令 trace 增 `vars`/`effects`，事件 0 的 418 帧 **2461/2461 全等（428 步 vars、330 步 effects、10 步 str_vars）**；夹具与门禁同步更新。
- M2 W4 Day1-4：`140` 按 `(bits>>4)&7` 分离 BGM/BGS/SE，`AudioManager.gd` 三轨 + 循环点/淡入淡出 + 存档快照；`GameClock.cs` 镜像 `217` 单次 if 进位；真实 `CE329→CE317` 连续 30 天就寝 soak 通过（`days=30 checks=61`）。
- M2 W4 Day5（部分）：GameRoot 强制 `StrictMode = true`（D-15），未实现 opcode 立即 Faulted 并按 D-13 锁档；旧 `presentation/*.cs` 已于 2026-11-15 退役（§4.13）。
- M2 第四轮审阅加固：300 补 host `call` 副作用、BGS 归因 `bgs`、trace 比较 `frame` 并增 `str_vars`；存档校验 `MapFile`/`LoopPoint`，非主角 teleport 忽略且队列上限 64；WAV 循环点帧→采样；`TerrainBridge` 让 `MapLayer` 复用 C# 通行度。
- M2 整路线 Oracle：Rust `route.rs` 与 C# `RouteTraceTest` 同跑 `TitleMap` event 2 → 新游戏 → map 1 的 1500 帧，两侧 **73,812/73,812 步全等**（12,469 步带 effects），route 末尾 `SaveGame→LoadGame` 状态回路通过；`diff_route.py` 支持 `.gz` 夹具，逐字段比较 `frame/map_id/event_id/pc/op/vars/str_vars/effects`。
- 旧表现层退役（2026-11-15）：`SessionParityTest` 改为 core-only（夹具 `oracle/fixtures/session_core_save_state.json`，`checks=9`）；`TestSessionView`/`SessionHarness` 接管 `SaveLoadTest 98/98`、`PresentationTest`、`MapMessageTest 14/14`；删除 `TitleScreen`/`ScreenHostView`/`ScreenHostSessionView`/`AssetCatalog`/`MapView`/`TitleScreen.tscn`/`ScreenshotTest`，保留三座 C# 桥。
- 图片状态/指针（2026-11-15）：`VmController` 缓存 `PictureCmd` 槽并把 `124` 的 hover/existence/x/y/scale/text 委派给 `PictureStage`，`EngineState(7)` 按官方三态（0/1/2）提供左键；`GameRoot` 现在把 `anchor`/`kind` 传给舞台，`CE217` 的 6200 渲染成真实 `8:30`；右键/中键 field 8/9 三态见 §4.19（D-24）。`PICTURE_STATE_TEST_OK checks=37`。
- 输入权限（2026-11-15）：`126` 按官方帮助改为「目标位 + 三态（常允许/移动禁/全禁）+ bit28 设备分支」，无自动清除；`123` mask 是复合 UI 位字段而非允许键掩码，改为标量键码后按权限过滤。Rust `InputState` 与 C# `EventVm` 同构，`cargo test -p wolf-vm` 95 passed、`VmTest` 覆盖三态/设备/`125` 穿透。
- Round-5 审阅 P1（2026-11-15）：`126` 的 `BasicAllDeny`/`MovementDeny` 已接到 `GameSession.MoveLeader`、`AdvanceFrame` 与 `GameRoot` 交互；故障返回标题清 `InputRouter` 指针/帧 latch，F9 读档后检查 `vm.Faulted`（§4.18，D-23）。
- 白天菜单端到端（2026-11-15）：`EngineState(7)` 按官方三态返回（0 未按/1 按住/2 刚按下）；CE251 从 pc1022 真实菜单点击到全局 48 与工作命令，CE310 从 pc128 点击“悠然自得”到 15:30/体力 1。`DAY_MENU_TEST_OK checks=13`、`WEEKEND_MENU_TEST_OK checks=9`。
- 鼠标右键/中键三态（2026-11-15）：`0x3000` field 8/9 补齐官方三态（0/1/2），与左键分开维护；`InputRouter` 记录右/中按压，故障返回标题一并复位。`PICTURE_STATE_TEST_OK checks=37`、`INPUT_ROUTER_TEST_OK`（§4.19，D-24）。
- 原版 `.sav` 导入严格校验（2026-11-15）：起始/结束字节必检，`rejectBadChecksum` 可选严格拒绝；严格 UTF-8 失败时 replacement 并记录 `encoding_warnings`，非空时写 `reports/encoding_warnings.json`。零售 8 槽 `SAVE_IMPORT_OK 8/8`、`SAVE_IMPORT_VALIDATION_OK`（§4.20，D-25）。
- `250` 数据库宿主（2026-11-15）：`GameDatabase` 读 `database.json` 的 CData/Data/Sys 与 `work/all/BasicData/*.project` 标签，脚本写走覆盖层；`VmController` 加载并暴露 `DatabaseStore`，`EventVm` 的 `250` 委派 host。`DATABASE_TEST_OK .../vm-250`（§4.21，D-26）。
- AutoEvents 调度器（2026-11-15）：`Program` 解析条件四元组，`AutoEvents` 跟踪 98 个并行事件（跳过 43，allow-list 242/243），`GameSession` 在 VM step 前同步、读档后 `PrimeLatched`；Rust `route.rs` 同序同步后整路线由 73,812 增至 **73,814** 步。`AUTO_EVENTS_TEST_OK count=4 start=3 resume=2 gated=1`（§4.23，D-28）。
- 战斗命令切片（2026-11-15）：`CE188` 经 `CE152` 构建攻击命令；覆盖层播种队伍/敌数/命令槽/领队属性后，空库存的真实攻击行不可选，`CData 7/0/0=1` 后可选；均到 CE192 且不生成 CE244。`BATTLE_COMMAND_TEST_OK control_real=0 seeded_real=1`（§4.22，D-27）。
- 战斗 wiring（2026-11-15）：`BattleWiring` 把 `340` rising、`CE157` 菜单命令、`CE242`/合成 `CE241` 与 `350/343/352` 结算接进 `GameSession` 帧序；`BattleWiringTest` 启动 `CE239` 并结算返回。`BATTLE_WIRING_TEST_OK rounds=1 result=-1 ce242_pc=415`（§4.24，D-29）。
- 战斗目标选择（2026-11-15）：`GameDatabase` 补引擎默认坐标倍率 32/24 与窗口皮肤，读序为覆盖层 → 引擎层 → 文件层；`CE190/CE175` 生成敌人并写 512/-288/512/312，`CE162` 选中索引 0 后退出，无敌人时停 pc49/50。`BATTLE_TARGET_TEST_OK selected=1 control_pc=49 enemy_hp=100`（§4.25，D-30）。
- 技能目标入口（2026-11-15）：CE159 的 pc110/115 读场景与效果目标，pc244 放行 10/11/12 后 pc247 启动 CE162；覆盖层播种目标/场景后从 pc94 验证到 CE162 pc57 退出，目标 0 不生成 CE162、敌数 0 停 pc49/50。`BATTLE_SKILL_TARGET_TEST_OK selected=1 no_target=1 no_enemy_pc=50`（§4.26，D-31）。

- 30 道门槛全绿（`GODOT_GATE_OK`，headless 与 `LWsmf_RENDER=1` 双模式）；`SAVE_LOAD_TEST_OK 98/98`、`SAVE_IMPORT_OK 8/8`、`SAVE_IMPORT_VALIDATION_OK`、`AUDIO_MANAGER_TEST_OK checks=30`、`CLOCK_TEST_OK checks=11`、`SLEEP_SOAK_TEST_OK days=30 checks=61`、`MAP_RUNTIME_TEST_OK 51/51`、`MAP_MESSAGE_TEST_OK 14/14`、`PICTURE_ROUNDTRIP_TEST_OK`、`TERRAIN_TEST_OK`、`MAP_LAYER_TEST_OK`、`SESSION_PARITY_TEST_OK checks=10`、`TRACE_DIFF_OK 2461/2461`、`ROUTE_DIFF_OK 73814/73814`、`PICTURE_STATE_TEST_OK checks=37`、`DAY_MENU_TEST_OK checks=13`、`WEEKEND_MENU_TEST_OK checks=9`、`DATABASE_TEST_OK .../vm-250`、`BATTLE_COMMAND_TEST_OK control_real=0 seeded_real=1`、`AUTO_EVENTS_TEST_OK count=4 start=3 resume=2 gated=1`、`BATTLE_WIRING_TEST_OK rounds=1 result=-1 ce242_pc=415`、`BATTLE_TARGET_TEST_OK selected=1 control_pc=49 enemy_hp=100`、`BATTLE_SKILL_TARGET_TEST_OK selected=1 no_target=1 no_enemy_pc=50`。详见 [`M1_HANDOFF.md`](M1_HANDOFF.md)。

**下一步（M2）**

- 整段切片（标题→新游戏→map 1）已冻结成 1500 帧整路线 Oracle（73,814/73,814，含 AutoEvents）；消息/选择只覆盖标题菜单 1 次，白天 101/102 与 save/load 的逐指令对拍仍待补。
- 白天 `CE251/CE310` 工作菜单的图片点击、确认与日程/体力推进已端到端覆盖（§4.16）；仍缺从冷启动进入白天的剧情夹具。`123/126` 已按官方三态/设备分支实现并接到表现层（逐位表待编辑器差分，见 §4.15/D-20、§4.18/D-23）；Faulted 已支持显式返回标题重建安全点（§4.17），存档槽级回退留 M5；原版 `.sav` 导入已做起始/结束字节、可选 checksum 与严格 UTF-8 告警（§4.20/D-25）；`250` 数据库宿主已落地（§4.21/D-26）；CE188/CE152 命令可用性切片已接（§4.22/D-27），CE239/CE242 真实回合 wiring 也已接（§4.24/D-29）；CE253 自然日线、真实 CE159 命令 dispatch、伤害矩阵与 H 仍待（CE162 直启由 §4.25/D-30 接，CE159 尾段由 §4.26/D-31 接）。

## 4. 当前唯一路线

以 [`PLAN.md`](PLAN.md) 为准。M0 已锁定数据转换契约、旧存档导入边界、C# 确定性基线和 Oracle trace；M1 已把标题→新游戏→地图→消息/选择→存读档串成可运行切片。

M2 的重点从“验证核心”转为“可回归的主循环与 UI”：InputRouter、9 级图层、101/102 自绘、字体度量、tile/地形、音频（MapChip 仅调试对照，不接入生产）。

在 M2 的输入/图层骨架稳定前，不应急着把 550 个事件铺成正式剧情。

## 5. 文档状态

| 状态 | 含义 | 位置 |
| --- | --- | --- |
| 现行 | 可作为当前执行依据 | `docs/godot/`、根 `PLAN.md`、`AGENT.md` |
| 迁移输入 | 旧 Rust 阶段验证出的格式/行为规格，仍可复用 | `docs/FORMAT_SPEC.md`、`docs/OPCODE_SPEC.md` 等 |
| 历史证据 | 旧实现/里程碑/研究底稿，只用于追溯 | `docs/HANDOFF_*`、`docs/M1_CLOSURE.md`、`docs/research/` |
| 归档 | 被新决策替代的入口或策略 | `docs/archive/` |
| 用户资料 | 攻略、外部调研、第三方工具，不作为权威 | `docs/攻略*`、`docs/gpt调研/`、`docs/vendor/` |

## 6. 维护规则

1. `docs/godot/DECISIONS.md` 是唯一决策源；不得在聊天、代码注释或旧 plan 里另立口径。
2. `docs/godot/PLAN.md` 是唯一 Godot 路线图；根 `PLAN.md` 只做摘要和跳转。
3. 新增 Godot 里程碑文档放 `docs/godot/`，并按需建立 `handoffs/`；不要让旧 Rust `HANDOFF_Mx` 继续膨胀。
   每阶段交接从 [`HANDOFF_TEMPLATE.md`](HANDOFF_TEMPLATE.md) 复制。
4. 任何文档移动/归档都要同步 [`../README.md`](../README.md) 和 [`../archive/README.md`](../archive/README.md)。
5. 旧 Rust 文档不做“追认式重写”；发现错误时只加勘误说明，不把历史改成当前。
6. 原版素材、攻略 PDF、`docs/gpt调研/` 和 vendor 工具不提交、不移动。
