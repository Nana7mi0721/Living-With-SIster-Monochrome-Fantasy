# Godot 重制版定稿决策

> 状态：**现行 / 最高优先级**
>
> 定稿日期：2026-11-14（Asia/Shanghai）
>
> 本文件覆盖此前所有“是否迁 Godot”“纯 GDScript”“继续 Rust 内核”或“暂不迁移”的讨论。
> 与原始审计报告、旧路线图、旧交接或历史研究冲突时，以本文件为准。

## 1. 已拍板决策

| ID | 决策 | 具体口径 | 影响 |
| --- | --- | --- | --- |
| `D-01` | **目标引擎与平台** | `Godot_v4.7.2-stable` 的 .NET 版本；仅面向 Windows x64 | 不为移动端/Web 牺牲架构；允许使用桌面文件系统与 .NET |
| `D-02` | **最终运行时语言** | **C# 核心 + GDScript 表现层**；去掉 Rust 运行时，不保留 GDExtension/原生 DLL 作为最终内核 | 确定性核心放 C#，场景、UI、输入、自绘和资源呈现放 GDScript |
| `D-03` | **迁移边界** | Rust 只作为开发期行为 Oracle，保留到 M4；不随游戏发布 | 可以继续运行旧测试/导出 trace，但不能把 Rust 当最终运行时 |
| `D-04` | **离线数据管线** | Python 离线转换器；只在开发期/用户本地转换时运行 | 运行时零 Python、零 Rust；转换产物必须可校验、可复现、可增量 |
| `D-05` | **内容范围** | 550 个公共事件、73 个地图事件、约 169,831 条真实指令；一次性覆盖全部 9 个结局与 DLC 链 | 不把“主线先做、DLC 以后再说”作为最终交付口径 |
| `D-06` | **文本范围** | 第一版仅官方中文；转换器保留原文与中文终稿的对应关系 | 暂不做运行时中日/多语言热切换；文本 token 和控制码必须保真 |
| `D-07` | **原版存档** | **尽量兼容导入**：C# `LegacySaveImporter` 读取原版 8 个槽位；新存档使用完整状态快照 | 原版 `.sav` 不要求逐字节回写；不能承诺恢复原版保存时不存在的 VM 协程状态 |
| `D-08` | **新存档策略** | 版本化 JSON（或可验证的二进制镜像）+ 完整 `event_instances`、`pc`、调用栈、`Wait`、RNG、图片槽、音频与 UI 状态 | 读写必须原子化；必须有 `.bak`、迁移器和损坏恢复 |
| `D-09` | **UI 与输入** | 1024×576 逻辑分辨率、60fps、9 级图层带、自绘 101/102、统一 `InputRouter` | 右键=取消、滚轮翻页、只有鼠标移动才触发 hover；不得把旧 Rust 补丁照搬 |
| `D-10` | **原版缺陷与差异** | 内容/崩溃/坏档/不可达问题修复并记录；影响体验的 UI 怪癖直接改善；不改写剧情 | `UI-12` 改为 `Enhance-in-Godot`；`CE333` 等死分支在 M3/M4 评估并记录 |
| `D-11` | **资产与版权** | `game_data/`、原版 `.wolf/.dat/.mps`、图片/音频/字体不进入 Git，不随发布包分发 | 用户提供正版零售目录；发布物仅含代码、转换器和必要的非原版占位证据 |
| `D-12` | **渲染器** | 默认按 2D/Windows x64 选择 **Compatibility**；M0 用实际场景验证后定稿 | 尚未成为不可变决策；若 Compatibility 出现字体/渲染阻塞，M0 记录后切换 |
| `D-13` | **发布期故障交互** | 未捕获异常或非法 opcode 进入 `Faulted` 模态：冻结输入与帧步进、禁止写档（`can_save()` 为假）、显示原因、允许导出诊断包，并允许用户显式「返回标题」重建标题安全点（放弃当前进度、帧归零、不再写档）；不得静默继续或覆盖存档 | 坏状态可诊断、可恢复，不产生坏档 |
| `D-15` | **音频三轨与严格模式** | `140` 按 `(intArgs[0] >> 4) & 0x07` 分离 BGM(0)/BGS(1)/SE(2)，三轨各自独立播放并携带循环点/tempo；BGM/BGS/SE 的当前文件、音量、循环点与相位写入新存档；发布与 CI 一律 `StrictMode = true`，未实现 opcode / 未知事件立即 `Faulted` 并按 D-13 锁档 | 修复旧实现把 43 条 BGS 当 BGM；坏状态不再静默跳过。Rust Oracle 的 `SoundChannel` 仍只有 Bgm/Se，BGS 语义差异在事件 0 trace 外单列记录 |
| `D-16` | **第四轮审阅加固** | 300 与 210 一样向 host 报 `call`；trace 的 `frame`、`vars`、`str_vars`、`effects` 四类字段两侧逐字段比较；`MapLayer` 不再重抄通行度，统一走 `TerrainBridge→C# Terrain`；存档校验 `MapFile`/音频 `LoopPoint`；`130` 只接受 target `-1/0` 且单帧队列上限 64 | 消除 Rust/C# 假阳性全等（BGS/300/frame/字符串）与双实现漂移；`KeyCodeForPoll` 不过滤 `mask`、`126` 不清除的旧口径已由 D-20 取代 |
| `D-17` | **整路线 Oracle 的边界** | Rust `route.rs` 复刻 `GameSession.AdvanceFrame`（`TitleMap` event 2、1500 帧、自动关消息/选 0/`123` 恒 OK）；除自 `D-28` 起在 VM step 前同步 `AutoEvents` 外不照抄 `Game::frame` 的其余引擎接线；传送 map id 按 programs.json 的 `_map_id` 规则解析，不用 SysDB 行序；save/load 只做状态回路断言，不逐指令比较；消息/选择只覆盖标题菜单 1 次 | 两侧生产入口不同（`start_map_event` vs `start_title` CE43），照抄 `Game::frame` 会从第 0 帧分叉；SysDB 行 1 是 `SampleMapA` 而 programs.json id 1 是 `SampleMapA_1`；1500 帧内无 op101，白天 101/102 仍靠单独用例；自动事件已使整路线增至 73,814 步 |
| `D-18` | **旧表现层退役与测试底座** | `SessionParityTest` 只比 core `GameSession` 与提交的冻结夹具 `oracle/fixtures/session_core_save_state.json`，不再实例化旧 `TitleScreen`；headless 测试统一用 `TestSessionView` + `SessionHarness`，消息清洗与立绘槽语义由 `TestSessionView` 自持；退役 `TitleScreen`/`ScreenHostView`/`ScreenHostSessionView`/`AssetCatalog`/`MapView`/`TitleScreen.tscn`/`ScreenshotTest`，只保留 `VmController`/`GameSessionController`/`TerrainBridge` 三座桥 | 去掉旧表现层对 parity 的牵制；夹具成为不可变历史证据，测试不再依赖生产路径；退役不改变行为；白天菜单仍是剩余项（MapChip 见 D-21） |
| `D-19` | **图片状态与指针单一来源** | `124` 的 hover/存在/坐标/缩放/文字判定只在 `PictureStage.picture_state` 实现一份（C# 不再重算贴图尺寸），`VmController` 只缓存 `PictureCmd` 槽并转调；`EngineState(7)` 按官方三态返回（0 未按 / 1 按住 / 2 刚按下，见 D-22），由 `InputRouter.pointer_pressed` + `GameRoot.advance_frame` 逐帧推进；首个鼠标事件前指针为 `(-1,-1)`（`InputRouter.mouse_seen`），对齐 Rust `Game.mouse` 初值 | 避免 Rust `game.rs`、C# 与 GDScript 三份命中几何漂移；`124` 的屏幕层/text 1×1 窗口/贴图实际尺寸与 Rust 契约一致；CE217 HUD 与 CE251 hover 都走同一条路径 |
| `D-20` | **`123`/`126` 输入权限状态模型** | `126` 不再当“位掩码清 held”：低字节 `bits0..7` 是目标键/设备选择、`bits8..9` 是三态（0=常允许、1=仅移动时禁止、2=移动与按键都禁止），`bit28` 是设备分支（键盘/鼠标/手柄，仅允许/禁止二态；本作把非 0 目标折叠为单一 `device_deny`，未按设备分槽）；未选中项保持不变、无自动清除、只在下一次同目标 `126` 时改写。`123` 的 `arg[1]` 是复合 UI 位字段（读取大类、方向/决定/取消/副键复选、5 种取得时机），**不是允许键掩码**，不能拿来过滤按键；`123` 返回标量键码并按当前 `126` 状态过滤（基本全禁→0，设备禁只作用于物理输入，不作用于 `125` 键盘自动键） | 修正旧 `BanKeys = raw` 把 240（恢复方向）误当禁止的模型；Rust `InputState` 与 C# `EventVm` 同构（`apply_permission`/`ApplyInputPermission` + `filter_code`/`FilterKeyCode`），移动与选择分别看 `basic_all_deny`/`movement_deny`；`123` mask 的逐位表与 `126` target bit↔具体键仍待编辑器二进制差分，不阻塞本作（现有比较均为标量键码） |
| `D-21` | **MapChip 不是内容缺口** | 零售正片卧室/标题/剧情画面使用全屏 `Picture` 背景；`.mps` tile 层与 `game_data/map_tiles.json` 的 `MapChip/...` 引用是 WOLF 引擎通用示例能力，只有 Rust `capture --map-run Dungeon` 的 tint 调试页可见通用 pipo 瓦片；`MapLayer` 保留调色板占位仅用于兼容/测试，不接入 `work/all/MapChip` 的 102 张 PNG | 避免把引擎示例素材误当内容缺口；不新增 tile 渲染/素材管线，遵守“只实现游戏内确实有的内容” |
| `D-22` | **白天菜单端到端与 EngineState 三态** | `124` 的 `0x3000` field 7 按官方定义为三态（0 未按 / 1 按住 / 2 刚按下），不再只返回边沿 1；CE251 从正常处理入口 pc1022 起步（跳过 pc1061/1088/1239 的随机命令写入会让全局 48 永远 0），CE310 从菜单绘制 pc128 起步；测试只覆盖图片 hover/点击、`102` 自动选“是”、日程与体力结算，不声称覆盖冷启动到白天 | 修正 CE251 在按住帧 `111 ==1` 失败的问题；`DAY_MENU_TEST_OK checks=13`、`WEEKEND_MENU_TEST_OK checks=9`，门禁 30/30 双模式；从冷启动进入白天仍需剧情夹具 |
| `D-23` | **126 权限接线与故障复位（Round-5 P1）** | `126` 的 `BasicAllDeny`/`MovementDeny` 必须在表现层同一条路径生效：`GameSession.MoveLeader` 按方向位阻塞、`AdvanceFrame` 的 OK 请求过滤、`GameRoot` 的裸 action/选项/取消尊重；故障「返回标题」必须同时清 `InputRouter.pointer_pressed` 与帧 latch；F9 读档后 `vm.Faulted` 必须进入 D-13 模态而不是静默成功 | 关闭 Round-5 P1 的“核心有状态、表现层没接线”漏口；读档故障不再被当成成功；逐位映射仍按 D-20 待差分 |
| `D-24` | **鼠标右键/中键三态** | `124` 的 `0x3000` field 8/9 按官方定义与 field 7 同为三态（0 未按 / 1 按住 / 2 刚按下）；`VmController` 为左/右/中分开维护 held 与 false→true 边沿，`BeginFrameInput()` 同帧搬运，`ResetPointer()` 清坐标与三组 latch；`InputRouter` 分别记录 `right_pressed`/`middle_pressed`，`GameRoot` 逐帧传入，故障「返回标题」一并复位 | 关闭 Round-5 P2 的 field 8/9 恒 0；右键/中键不再泄漏到左键；`PICTURE_STATE_TEST_OK checks=37`，30 步门禁双模式仍绿 |
| `D-25` | **原版 `.sav` 严格校验与编码告警** | `LegacySaveImporter` 对每个槽校验解密后的起始字节 `0x14=0x19` 与末字节 `0x19`；`ImportFile(..., rejectBadChecksum:true)` 在 checksum 不符时抛 `InvalidDataException`，宽松模式仍保留 `checksum_ok=false` 供诊断。`ReadMemDataString(size, field)` 先严格 UTF-8，失败时按 replacement 解码并记录字段/偏移；`SlotResult.encoding_warnings` 暴露告警，`ImportAll` 在非空时写 `reports/encoding_warnings.json`。不引入 CP932/GBK 重写 | 收口 M3 的导入边界；零售 8 槽仍 `SAVE_IMPORT_OK 8/8`；新校验覆盖坏 checksum、坏起始字节与非法 UTF-8，且不静默改写名字/图片名（DATA_CONTRACT §6/§7） |

| `D-26` | **250 数据库宿主** | `250` 的 CData/Data/Sys 三容器由 `GameDatabase` 唯一实现：基础层读 `game_data/database.json`，脚本写进覆盖层，行数/清行/首空行/扩展行按 Rust 哨兵语义；`work/all/BasicData/*.project` 只解析 type/field 标签，按 type 名与字段数匹配列，标签做 NUL/破折号/全角空格规范化。`VmController` 在 `LoadPrograms` 时加载并暴露 `DatabaseStore`，`EventVm` 的 `250` 走 host | 先补战斗命令 `CE152/CE188` 的数据底座；`.project` 行名/说明/默认值不消费，覆盖层不回写；`DATABASE_TEST_OK .../vm-250`，30 步门禁双模式仍绿 |
| `D-27` | **CE188/CE152 战斗命令切片** | 用数据库覆盖层显式播种队伍数/敌数/命令槽/领队属性（Rust 测试里来自 `SaveData01`），`BattleCommandTest` 以 200 帧、奇偶帧 OK 脉冲跑 control（空库存）与 seeded（CData 7/0/0=1）两条 CE188 线；逐帧判断 CData 19 是否有真实攻击行可选，而不看被 CE192 重写的末态；断言两条线到 CE192、seeded 到 CE152、均不生成 CE244 | 先关闭“命令菜单可用性”这一最小战斗切片；敌数/队伍数靠播种而非完整存档导入；真实回合 CE239/CE242、目标选择与伤害结算仍未覆盖 |
| `D-28` | **AutoEvents 调度器与 route 同步** | `Program` 解析 `run_condition_op_var_val` 四元组；新增 `AutoEvents` 跟踪 `run_condition==2` 与 allow-list `242/243`，跳过标题菜单 `43`；条件算子在 C# 与 Rust 一致为 `1 >=`、`2 ==`、`3 <=`，变量地址只接受 `VAR_BASE+0..800`。`GameSession.EnsureAutoEvents()` 惰性建表，`AdvanceFrame` 在 `_vm.StepFrame()` 前 `Sync`，读档在 `EnterMap` 后 `PrimeLatched`；`StartNewGameNow` 不清调度器（否则同一事件会双重启动）。Rust `route.rs` 也在 `vm.step_frame` 前 `auto_events.sync`，并重生成 73,814 步夹具 | 关闭 `CE38/216/217` 等并行事件在 C# 完全缺失的缺口；同步顺序对齐 `Game::frame`，避免“条件持续为真时重启一次性事件”与读档重放 HUD 的错误；route 从 73,812 增至 73,814 步，`ROUTE_DIFF_OK 73814/73814` |
| `D-29` | **CE239/CE242 战斗 wiring** | 新增 `BattleWiring`：`340` rising 驱动，`CE157` 的 CData type14 rows1..8 收割为菜单命令，单发要求 `350==0 && 343==0`，多轮在 `343!=0` 停并在条件满足时启动 `CE242`、补 `CE241`；命令号先取队列，否则循环 `[1,3]`；`RealBattleRunning` 以 `CE253` live 或 `CE239` 有父实例判定。`EventVm.CreateFreshInstance` 创建空白 `Self` 的一次性实例并在 `PersistCommonSelf` 跳过回写。`GameSession.AdvanceFrame` 顺序为 AutoEvents → BattleWiring → VM step | 关闭“命令切片有了但真实回合没有调度”的缺口；`BATTLE_WIRING_TEST_OK rounds=1 result=-1 ce242_pc=415`，30 步门禁双模式仍绿；只覆盖 CE239 直接入口单轮，CE253 自然线/目标选择/伤害矩阵/H 留后续 |
| `D-30` | **战斗目标选择与坐标倍率** | `GameDatabase` 增加与 Rust `Databases::with_engine_defaults` 一致的引擎默认层（CData 18/97/0=32、18/98/0=24、Data 18/0/24=`SystemFile/WindowBase.png`），读序为 250 覆盖层 → 引擎层 → `database.json` 文件层，脚本写仍落覆盖层。`BattleTargetTest` 播种 26 个 battle seed，启动 `CE190` 验证敌数/HP 与 `CE175` 坐标 512/-288/512/312，再启动 `CE162` 验证索引 0 选中、pc57 退出；控制线只清 18/69 并断言停在 pc49/pc50。`Drive` 的 OK/NONE 脉冲按帧号而非 123 调用次数，因为 Rust `300` 会阻塞调用方一帧 | 关闭 CE175 坐标被文件 0 覆盖的缺口；目标选择目前是 CE162 直启，`CE159` 执行入口与伤害/胜负矩阵仍未接；引擎默认层只补三项，不等同完整 Rust 默认表 |
| `D-31` | **CE159 技能尾段与 CE162 目标入口** | CE159 的 pc110 读 Data 0/row/6（`使用可能シーン`），pc115 读 Data 0/row/2（`効果対象`），pc244 只放行 10/11/12，pc247 以 `300` 启动 CE162。测试在覆盖层写 `Data 0/2/2=10`、`Data 0/2/6=1`，从 pc94 注入 CE159 并设 `Self 1600000=1、1600001=2、1600002=0、1600022=2`；`BattleSkillTargetTest` 断言 PC110/115/247 与 CE162 pc12/pc57 后退出，目标 0 不生成 CE162，敌数 0 时 CE162 停 pc49/pc50；脉冲按帧号奇数帧 OK | 关闭 CE162 直启与技能尾段之间的空白；`work/all` 的 type0 是 18 列精简 schema、`.project` 标签错位，`col2`（効果対象）零售全 0、`col3` 才是 10/11/12，因此必须显式播种，不能靠标签解析修复；入口仍是测试注入，CE188 命令 dispatch 与 CE253 自然线未接 |
| `D-32` | **H 阈值数据面** | 把 Rust `m13_h_thresholds.rs` 的 37 个真实 op111 载荷（CE310 10、CE318 6、CE323 6、CE335 15）钉进 `HEventGateTest`：逐条断言 event/pc/params 与 Rust 一致；单条件门按 `(Ints[0]&0x0F)==1` 测 `value-1` 不匹配与 `value` 匹配 case0；多条件门显式 31 点，`FirstTrueCase` 按 compare `0>`/`1>=`/`2==`/`3<=`/`4<`/`5!=`/`6&` 取第一个真分支；不执行 H 场景 | 关闭 H 门只有 Rust 测试、C# 无数据面回归的缺口；`H_EVENT_GATE_TEST_OK gates=37 boundaries=59 data_only=1`，30 步门禁双绿；CE318 信頼 100/99→CE360、CE310/323/335 剧情调用仍未接，不能视为 H 完成 |
| `D-33` | **AutoEvents 新游戏不复位** | 第六轮建议在 `StartNewGameNow` 的 `StopAll` 后调用 `ResetForNewGame()` 清 `_latched/_running`；实现后 `SESSION_PARITY_TEST` 从 10/10 退化为 1/10，actual 多出 event 38 的 `common_self`，实例 id 由 6/7/8 顺移为 7/8/9。Rust `AutoEvents` 公开 API 没有 reset，`Game` 只在构造时建表，新游戏不清 latched；latched 与 `_running` 是 session-scoped 边沿状态。因此删除 `ResetForNewGame()` 与调用，保留 `StopAll` 不清边沿；`AutoEventsTest` 断言条件保持 1 时 StopAll 后再次 Sync 只重放 216/217（`Started==2`）且不重启 38 | 修复“新游戏把一次性条件事件当新边沿重放”的潜在重复实例；若未来要支持同会话重开新游戏，必须同时重建整个 `GameSession`/`AutoEvents`，不能只清字典 |
| `D-34` | **第六轮 P2 表现层与 Oracle 收口** | `GameRoot._on_wheel` 与 `_on_move` 同查 `BasicAllDeny` 的 16/32 位；`AudioManager.play` 在 `_apply_loop` 前 `duplicate()` 共享 `AudioStream`，避免同文件 BGM/BGS 互改 loop；`EventVm` 的 140 对 soundType 3..7 写 `Host.Log` 警告但保留 BGM 落点；`PictureStage` 用容器 `pivot_offset/scale.x=-1` 镜像文本与占位槽；`diff_trace.py`/`diff_route.py` 的 effects 统一严格比对；H 输出加 `data_only=1` | 关闭审阅可执行的交互/资源/对拍口径问题；`AUDIO_MANAGER_TEST_OK checks=32`、`PICTURE_ROUNDTRIP_TEST_OK`、`GAME_ROOT_TEST_OK`、双模式 30 步门禁与 73814 路线全等；Rust 的 soundType 3..7 实际落 SE，C# 暂不改变通道，留待音频通道语义确认 |
| `D-35` | **Data type0 列契约与 .sav 生产严格入口** | `work/all` 的 Data type0 为 18 列精简 schema，`.project` 无 `使用可能シーン`/`効果対象` 字段，CE159 的 250 字段标签回退到 `ItemId` 列：`col2`=`スキルタイプ`（44 行全 0）、`col3`=`消費行動力`（10/11/12）。转换器按字段顺序写 `numbers`，未发现列偏移证据；测试必须写 250 覆盖层，不得把 `┏ 効果対象` 别名到 col3。`LegacySaveImporter` 的 `rejectBadChecksum` 默认 false 只服务诊断，生产 UI/发行入口必须显式 true | 把“脚本按更宽零售 schema 编写”与“转换器错列”分开，避免用别名掩蔽数据缺口；Data 0 列错位保持 `needs-retail-data`，取得零售 `DataBase.dat` 前不修；生产导入不得静默接受坏 checksum |
| `D-14` | **CI 占位素材** | `godot/tests/fixtures/asset_stub/` 只放程序生成的纯色 18×18 PNG 与静音 WAV（`tools/make_asset_stub.py` 可复现），供无零售资产的环境跑表现层测试；显式指定 AssetDB 根时永不回退到占位 | 不违反 D-11；缺零售资产时 local/CI 行为确定、可断言 |

## 2. 技术含义

### 2.1 C# 核心负责什么

- 事件程序解释器与确定性帧步进。
- 32 位算术、溢出/位移/PRNG 语义和 trace 输出。
- 原版 `.sav` 解密、解析、导入；新存档编解码和迁移。
- 需要 headless 测试、与 Rust Oracle 对拍的纯逻辑。

### 2.2 GDScript 负责什么

- Godot 场景、节点生命周期和画面切换。
- 101/102 自绘、字体排版、图片槽、九级图层和 HUD。
- `InputRouter`、`AudioManager`、存档 UI、设置页和调试面板。
- 把 C# 核心输出的 side effect/状态快照呈现出来。

### 2.3 不允许的双实现

不要维护“一份 C# VM + 一份 GDScript VM”。GDScript 只做表现与编排；
任何会改变变量、分支、RNG、事件实例或存档的确定性逻辑，只能有一个实现。

## 3. 原版 `.sav` 的准确边界

“兼容 `.sav`”定义为：

1. 能读取原版 `SaveData01..08.sav` 的基础变量、数据库覆盖、图片表、时间/地图等已建模数据；
2. 能在安全场景边界恢复，并重建 HUD/自动事件状态；
3. 不承诺恢复原版格式中不存在的 VM 协程 `pc`、调用栈、`Wait` 或音频精确相位；
4. 新版本保存自己的完整状态，不依赖原版 `.sav` 作为长期存档格式；
5. 是否回写原版 `.sav` 作为导出功能，留到 M3 之后决定。

因此，原版存档导入是**尽力兼容**，不是逐字节双向兼容。

## 4. 已废止的旧决策

以下是历史内容，不得再作为实现依据：

- 原始审计报告中“纯 GDScript 推荐”“Godot 4.3+”“运行时零 C#”的全部段落；
- 原始审计报告 ADR-01（纯 GDScript）和 ADR-05（完全不读旧 `.sav`）；
- `docs/archive/rust/ENGINE_STRATEGY_2026-11-04.md` 的“暂不迁移、Rust 内核 + GDExtension”结论；
- 旧 `PLAN.md` 的 Rust 里程碑、项目终止说明和“继续修 UI”路线；
- 旧 `UI_BUG_BACKLOG.md` 的“在当前 Rust 中继续修”任务口径。

原始审计报告仍保留事实审计、Rust 风险表、Oracle 方法和架构思路；
但凡是与 `D-01..D-12` 冲突的段落，均视为草稿。

## 5. 仍待拍板

| 问题 | 当前默认 | 最晚决策点 |
| --- | --- | --- |
| Compatibility vs Forward+ | Compatibility 先做 spike | M0 结束 |
| 原版 `.sav` 是否导出回写 | 不实现回写 | M3 |
| `CE333` 不可达分支是否补全 | 先记录，不擅自改剧情 | M3 |
| `UI-12` 面板的最终布局 | 允许增强，保持信息层级 | M2 |
| Godot .NET 的具体 SDK/CI 版本 | 跟随 4.7.2 stable 官方要求 | M0 第 1 周 |
| 用户实机抓包格式与保存位置 | 本地录制，不入库 | M0 第 2 周 |
| `123` 的 `mask` 逐位表、`126` target bit 与具体键/设备的对应 | 已按官方三态与设备分支实现（D-20）；逐位映射不阻塞标量键码路径 | 编辑器二进制差分可测时 |
| StrictMode 常开后的故障恢复路径 | D-13 锁档 + 诊断包 + 显式返回标题（标题安全点）；存档槽回退留 M5 | M5 |
| MapChip 零售贴图 | 引擎通用示例素材，正片未使用；不接入生产，调色板占位仅兼容/测试 | 不适用（D-21） |
| 整路线内 101/102 逐指令与更长路线 | 1500 帧只含标题菜单 1 次选择；消息/选择另测 | M3 内容对齐前 |
| 白天 CE251/CE310 端到端回归 | 菜单点击/确认/日程/体力已由 `DayMenuTest`/`WeekendMenuTest` 覆盖（§4.16）；从冷启动进入白天仍需剧情夹具 | M3 内容对齐前 |

## 6. 变更规则

修改 `D-01..D-12` 必须在 `docs/godot/DECISIONS.md` 增加一条变更记录，
写明旧决策、原因、影响和迁移动作；不得只在聊天或代码注释里修改。
