# LWSMF Godot 重制版第四轮独立技术审阅报告（M2 W3 + W4）

> **审阅对象**：`imoto_life_godot_m2w4_2026-11-15.zip`  
> **对照基线**：`DECISIONS.md`（`D-01..D-15`）、`PRIOR_GEMINI_AUDIT_ROUND3.md`、`LEGACY_RUST_AUDIT.md`  
> **证据边界**：本沙箱无 Godot / .NET / Rust 编译器、无 `game_data/`（含 `map_tiles.json`）、无 `work/all` 零售素材与真实 `.sav`。所有构建与运行数据均引自上游 `evidence/` 日志与源码静态核验；无法静态证明的运行时/零售资产行为已标注 `needs-runtime` 或 `needs-retail-data`。

---

## 1. 总评：M2 能否收口进入 M3？

**结论：有条件允许进入 M3 预备阶段，但“M2 完全收口”尚不成立；建议先用 2–3 天完成“发布路径门禁对齐 + 4 项阻断级小修”，再正式开启 M3。**

主场景已切换至 `res://scenes/GameRoot.tscn`（`project/godot/project.godot:11`），`GameSession` 纯核心抽取、三轨音频架构、确定性时钟与 `vars/effects` 对拍均已落地且 26 道上游门禁全绿（`evidence/logs/godot_gate_w4.log`）。但静态审计发现，多处“已关闭”问题仍存在行为边界或门禁盲区：

1. **默认门禁主路径仍不是发布路径**（置信度：**高**）：`godot_check.sh` 第 7–12 项（Title/NewGame/Presentation/SaveLoad/MapMessage）仍运行旧 `TitleScreen.cs`，新发布路径 `GameRoot.gd` 仅靠 `GameRootTest.gd` 与 `SessionParityTest.cs`（仅比对 `SaveState`）覆盖。
2. **2461/2461 对拍升级有效，但存在契约缺陷与窄覆盖风险**（置信度：**高**）：`vars/effects` 对拍覆盖了 428 步变量与 330 步副作用，但仅限 `SampleMapA` 事件 0 的 418 帧；`TraceRecorderHost.cs:64` 将 `SoundChannel.Bgs` 误映射为 `"se"`；`diff_snapshots()` 不比对字符串变量且忽略 `0` 值显式写入；`Wait` 与 `130/160/290` 视觉态未入对拍。
3. **W3/W4 新增代码存在 3 个高价值逻辑/架构缺陷**（置信度：**高**）：
   - `MapLayer.gd:117-184` 在 GDScript 侧重抄了一遍 C# `Terrain.cs` 通行度算法，违反 `DECISIONS.md §2.3`“不允许双实现”；且 `cell_clicked`/`cell_hovered` 信号在 `GameRoot.gd` 中无人连接。
   - `AudioManager.gd:184` 对 `AudioStreamWAV` 将 60Hz 帧数 `loop_point` 直接赋给按采样点计数的 `wav.loop_begin`（单位差 ~735 倍）。
   - `GameSession.cs:235-248` 的 `ApplyTeleport` 未校验 `request.Target`（非主角目标也会强制传送主角），且 `_teleports` 队列无上限。
4. **D-15 常开 `StrictMode` 存在正式版锁档风险**（置信度：**高**）：当前门禁仅实际执行了极少数事件（Title/Map1/CE217/CE329/317），550 事件 / 169,831 指令的剩余覆盖率未知（`needs-retail-data`）；在无安全点回滚或隔离存档槽前直接常开 `StrictMode` 会导致玩家遇到未实现 opcode 时既不能继续也不能存盘。

---

## 2. 第三轮问题复核表

| ID | 当前状态 | 严重度 | 证据（文件:行） | 触发条件 | 根因 | 影响 | 最小修复 | 验证方法 | 置信度 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **切换阻塞** | **部分解除（可切换但未收口）** | P1 | `project.godot:11`; `GameSession.cs:61-588`; `MapLayer.gd:270-281`; `GameRoot.gd:316-322` | 玩家在地图点击格子或使用非占位 MapChip 游玩 | 主场景与 `GameSession` 已切通，但 `MapLayer` 为色块占位且 `cell_clicked` 未接线 | 键盘行走与 F5/F9 可用，但鼠标点地移动/点触 NPC 仍无响应；缺贴图 | 在 `GameRoot.gd` 连接 `map_view.cell_clicked` 至寻路/交互；保留 `needs-retail-data` 贴图替换口 | 在 `GameRootTest.gd` 注入 `cell_clicked` 信号验证响应 | 高 |
| **W2-P1-01** | **已修** | P1 | `GameRoot.gd:60,214-218`; `ChoiceWindow.gd:101-113`; `GameRootTest.gd:54-72` | 鼠标移动悬停选项后左键确认 | `router.hover_changed` 驱动 `choice.hover_at()` 同步更新 `selected` | 鼠标点选现已命中悬停行；注：仅鼠标移动触发，未破坏键盘选择 | 无需改动（后续可补真实渲染下的点击端到端测试） | `GAME_ROOT_TEST_OK`（已含 `hover_at` 断言） | 高 |
| **W2-P1-02** | **已修（行为待实机核对）** | P1 | `GameRoot.gd:178-182,259-263`; `MessageWindow.gd:95-125` | `message.is_open()` 时按取消键（X/Esc/右键） | `_on_cancel` 调 `message.close()` 后立即调 `_on_message_confirm()` 解除 `WaitState.Message` | 不会再永久卡死；但若原版 101 未读完时按取消是“瞬间全显”而非“直接关窗”，则会跳过 `\.`/`\>` 节拍（`needs-retail-data`） | 实机确认原版行为；若不允许跳过未读完文本，改 `if not message.finished: message.confirm()` | 实机对照原版 101 取消键行为（`needs-runtime`） | 中 |
| **W2-P1-03** | **部分修复** | P1 | `VmController.cs:300-325`; `GameRoot.gd:71,139-145`; `AudioManager.gd:33-92`; `crates/wolf-game/src/game.rs:3273` | 123 轮询传入特定按键掩码（非 771） | `771` 源自原版 `event 131 pc58` 存档列表滚轮掩码（`game.rs:3273`）；`KeyCodeForPoll` 处理了 `771` 与 `autoKeys`，但**未用 `mask` 过滤 `(held \| autoKeys)`** | 音频三轨与音量/淡入已修；但脚本若用窄掩码轮询 `123`（如只查方向键），未屏蔽的其他按键仍会漏过 | 在 `VmController.cs:303` 按 `mask` 过滤（需确认 `771` 与各掩码位定义，`needs-retail-data`） | 补单测验证窄 `mask` 过滤被屏蔽的按键 | 高 |
| **W2-P1-04** | **已修（像素等价性待实机）** | P1 | `PictureStage.gd:87-94,179-187`; `PictureRoundtripTest.gd:57-68` | `action=1` 空槽 Move 或 `scale_percent < 0` | 空槽 Move 返回 `null`；负缩放改为 `TextureRect.flip_h = true` 且保留 `holder.size`/`anchor_position` | 无幽灵图；`flip_h` 在布局盒内原地水平翻转，不向左溢出；但非 `TextureRect` 占位盒（文本图/缺失框）不会翻转 | 对缺失图/文本图容器也支持翻转或在文本图跳过镜像；实机对照原版负缩放锚点（`needs-retail-data`） | `PICTURE_ROUNDTRIP_TEST_OK` + 实机负缩放截图对比（`needs-runtime`） | 中 |
| **W2-P1-05** | **部分修复（降级保底）** | P1 | `TextTokenizer.gd:182-185,193-206`; `MessageWindow.gd:192-200` | 文本含 `\cdb[21:3:4]` 或 `\c[-1]` | Token 保留完整 `body`；`posmod` 避免负数越界；但 `MessageWindow.gd:199` 无变量库，仍按首个数字 `21` 查硬编码 8 色表 | 不再崩溃或丢字，但 `\cdb` 颜色与原版不一致（`needs-retail-data`） | M3/M4 接入 CData 变量数据库后解析 `21:a:b` 对应 RGB | 接入零售数据库后对比 `\cdb` 渲染色 | 高 |
| **W2-P2-01** | **已修** | P2 | `AssetDB.gd:147-151` | 加载音频计算缓存大小 | 改用 `FileAccess.open(path, FileAccess.READ).get_length()` | 消除 `get_file_as_bytes()` 双倍读盘 | 无需改动 | `ASSETDB_TEST_OK` / 代码审查确认 | 高 |
| **W2-P2-02** | **已修** | P2 | `InputRouter.gd:155-157`; `VmController.cs:300-304` | 查询 123 键码 | 删除 GDScript `code_for_*`，统一经 `VmController.KeyCodeForPoll` 调 `KeyCodes.Code` | 消除键码表双实现 | 无需改动 | `INPUT_ROUTER_TEST_OK` | 高 |
| **W2-P2-03** | **仍开放** | P2 | `UiSkin.gd:16-24` | 渲染中/日文文本 | 包内无零售字体（`Lolita.ttf`），仍用系统字体回退链 | 自动换行点与行高可能存在 3–5% 偏差（`needs-retail-data`） | 接入零售字体目录后优先加载指定字体文件 | 零售环境下对比 101 换行截图 | 高 |
| **W2-P2-04** | **仍开放** | P2 | `EventVm.cs:38-39,564,1008-1011`; `crates/wolf-vm/src/exec.rs:798,2024,2101` | 事件执行 `126 BanKeys` 后继续执行 `123` | C# 与 Rust 均未在 `123` 后清零 `BanKeys`，但注释写“every later 123 poll clears it” | 若零售脚本调用 `126` 禁键后未显式调用 `126(0)`，后续所有 `123` 将永久被屏蔽；反之若原版确实持久化，则注释误导 | 原版实机抓包/反汇编确认 `126` 是否在 `123` 后清除或每帧清除（`needs-runtime`），统一代码与注释 | 实机测试含 `126` 的事件（如菜单/战斗禁键） | 高 |
| **W1-P0-01 残余** | **部分修复（仍有遗漏）** | P1 | `SaveCodec.cs:307,347,357,377-387`; `GameSession.cs:497-501` | 加载被篡改的 `SaveState` JSON | `IsSafeAssetPath` 已覆盖 `pic.File` 与 `audio.File`，`RestoreInstances` 已校验 `0 <= Pc <= program.Len`；但 **`SaveState.MapFile`（`SaveCodec.cs:307`）未调 `IsSafeAssetPath`** | 当前 `EnterMap` 按 `MapId` 查表不用 `MapFile`，暂不触发实际穿越，但防御面不完整 | 在 `SaveCodec.Validate` 第 307 行增加 `if (!IsSafeAssetPath(state.MapFile)) return false;` | 在 `SaveLoadTest` 补充含 `../` 的 `map_file` 负向用例 | 高 |
| **W1-P1-04 残余** | **仍开放** | P1 | `EventVm.cs:1043-1081` vs `:831` | 执行 `300 CallByName` | `case 300:` 分支缺少 `Host.CommonEventCall(callee.Id, callArgs, callStrings)` | 550 事件全量下，任何 `300` 调用在 Oracle trace 中都会漏掉 `call` 副作用，导致宿主侧状态或对拍失真 | 在 `EventVm.cs:1069` 后补齐 `Host.CommonEventCall(callee.Id, callArgs, callStrings);` | 构造 `300` 调用测试并验证 `TraceRecorderHost` 收到 `call` effect | 高 |

---

## 3. W3/W4 新增代码审计表

| 模块分组 | 状态 / 严重度 | 证据（文件:行） | 触发条件 | 根因 | 影响 | 最小修复 | 验证方法 | 置信度 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **GameSession** | 正常（无 Godot 依赖） | `GameSession.cs:1-5` | 核心层编译 | 仅引用 `System.*` 与 `Lwsmf.Core`，已剥离 `Godot` 类型 | 架构边界干净，可独立 headless 单测 | 无需改动 | 静态审查确认 | 高 |
| **GameSession** | P2（职责轻微越界） | `GameSession.cs:168-178` | 每帧更新 HUD | `UpdateMapHud` 在核心层硬编码拼接英语 HUD 调试字符串并传给 `IGameSessionView.SetMapHud` | 表现层文案格式与核心耦合 | 改为传结构化状态或由 `GameRoot` 格式化 | 代码审查 | 高 |
| **GameSession (传送)** | **P1（逻辑缺口）** | `GameSession.cs:67,160,229-248` | 事件对非主角目标执行 `130` 或同帧高频入队 `130` | `ApplyTeleport` 忽略 `request.Target`（只传送主角/切图）；`_teleports` 队列无上限保护 | 非主角事件传送会错传主角；异常死循环入队可致内存/同帧多次切图卡死 | 仅 `Target == 0/-1` 触发切图（其他目标转发事件移动）；为 `_teleports` 加每帧上限（如 64） | 补单测验证非主角 `Teleport` 不切图及队列上限保护 | 高 |
| **GameClock** | 设计确认（含已知怪癖） | `GameClock.cs:45-65` | `ApplyMinutes(+130)` 或跨周五 | 刻意镜像 `CE217` 单次 `if` 进位（`13:70`）及永不清除 `VarWeekendFlag=25` | 保持零售剧情分支数值一致（`D-10`），但若直接把 `VarHour:VarMinute` 用于新 UI 显示会出现 `13:70` | 保持核心 `CE217` 行为不变；若重制版 HUD 需正常时间显示，应在 UI 层或确认不影响剧情分支后再做规范化（`needs-retail-data`） | `CLOCK_TEST_OK` | 高 |
| **Terrain** | 正常（逐位等价） | `Terrain.cs:34-87` vs `map_runtime.rs:249-305` | 计算地图通行度 | `0xFFFFFFFF` 跳过、`raw/100000` 取 autotile、`quarter` 四角可通行逻辑与 Rust 逐位一致 | 三张已知地图阻塞数与 Rust 完全一致 | 注意 `MapRuntime.CanEnter(x,y,step)` 无体型概念（原版单格通行） | `TERRAIN_TEST_OK` | 高 |
| **MapTileData** | 正常（可缓存） | `MapTileData.cs:57-125,140-149` | 读取 `map_tiles.json` | 按绝对路径+长度+UTC mtime 缓存；`DecodeU32` 小端序解码 | 避免重复解析 564 KB JSON；注意缓存静态字典在多测试间需注意 `ClearCache` | 建议增加 `tileset_id` 越界日志而非静默返回 `null` | `TERRAIN_TEST_OK` | 高 |
| **MapLayer** | **P1（双实现 + 未接线）** | `MapLayer.gd:117-184,270-281`; `GameRoot.gd:316-322` | 修改通行规则或玩家鼠标点击地图 | GDScript 重写了一套 `mask_for_word`/`_bits`（违反 `DECISIONS §2.3`）；`cell_clicked`/`cell_hovered` 未连到 `GameRoot` | 双实现易漂移；鼠标点地图无响应；硬编码 `../game_data/map_tiles.json` 路径 | 删除 `MapLayer.gd` 通行度计算，改由 `GameSessionController` 暴露核心 `Terrain` 查询；连接 `cell_clicked` | `MAP_LAYER_TEST_OK` + `GameRootTest` 补点击测试 | 高 |
| **AudioManager** | **P1（WAV 单位错误）** | `AudioManager.gd:175-185` | 播放带 `loop_point` 的 `.wav` 音频 | 第 179 行 OGG/MP3 将 `loop_point`（60Hz 帧）除以 `60.0` 转秒，但第 184 行对 `AudioStreamWAV` 直接将 `loop_point` 赋给按**采样点**计数的 `wav.loop_begin` | WAV 循环点错位 ~735 倍（如 60 帧=1 秒，在 44.1kHz 下仅设了 60 samples ≈ 1.36ms）；`needs-retail-data` 确认零售是否使用 WAV 循环 | 对 `AudioStreamWAV` 乘以 `wav.mix_rate / 60.0`（即 `roundi(loop_point * wav.mix_rate / 60.0)`） | `AudioManagerTest.gd` 补 WAV `loop_begin` 采样点断言 | 高 |
| **AudioManager** | P2（共享流污染 + 未知通道） | `AudioManager.gd:47,175-177`; `EventVm.cs:606-612` | 同一文件跨 BGM/BGS 播放，或 `140` 类型位为 3..7 | `_apply_loop` 直接修改 `AssetDB` 缓存的共享 `AudioStream`；`EventVm` 对 `3..7` 静默落入 `SoundChannel.Bgm` | 同文件跨轨会互相覆盖循环属性；未知音频类型静默误播为 BGM | 跨轨复用或修改循环前 `stream.duplicate()`；`EventVm` 对 `soundType > 2` 打日志或 `StrictMode` 报错 | 补跨轨同文件播放单测 | 高 |
| **SaveCodec.audio** | P2（缺上限 + 旧路径无音频） | `SaveCodec.cs:352-361`; `ScreenHostSessionView.cs:59-61` | 读档校验 `audio` | `LoopPoint >= 0` 无上限；`ScreenHostSessionView` 返回空列表以保持快照同构 | 极端篡改 `LoopPoint`（如 `int.MaxValue`）能通过校验；旧路径不具备音频存档能力 | 校验 `LoopPoint <= 86_400 * 60`（或合理上限）；确认旧路径随 `presentation/*.cs` 退役 | `SAVE_LOAD_TEST_OK` | 高 |

---

## 4. Oracle 对拍可信度评估

### 4.1 升级后证明了什么 vs 仍没证明什么

- **已证明**：在 `SampleMapA` 事件 0 的 418 帧 autorun 段中，C# `EventVm` 与 Rust `wolf-vm` 的**指令步进序列（2461 步）**、**428 步整数变量变更（含普通/系统/Self 变量）**、以及 **330 步已建模副作用（`call`/`picture`/`sound`/`choice`/`teleport`/`database`/`effect`）** 完全一致（`evidence/oracle/step_trace_diff.json` 抽样核对结构一致）。
- **仍没证明**：
  1. **字符串状态完全未对拍**：`diff_snapshots()` 与 `OracleStepTraceTest.Snapshot()` 只收集整数变量，`StrVars`、`SysStrVars`、`Self.Str` 的任何错乱（如 `\cdb`、文本拼接、`122` 赋值）**完全不会触发差异**。
  2. **非事件 0 的整条主线与战斗/菜单逻辑**：不包含标题 `CE42` → 新游戏 `Map003` → `Map001` → 101/102 交互 → 存读档。
  3. **BGS 通道与时间/等待/转场**：`130/160/290` 视觉表现与 `Wait` 时长/种类均被排除在外。

### 4.2 能骗过 2461/2461 的偏差举例

1. **字符串变量静默写错**：`122 StringVar` 把目标字符串写为空或乱码，只要不触发后续整数分支，2461/2461 照样 100% 通过。
2. **BGS 通道伪装成 SE**：`TraceRecorderHost.cs:64` 现行代码为 `cmd.Channel == SoundChannel.Bgm ? "bgm" : "se"`。若事件中播放了 `SoundChannel.Bgs`，C# trace 会输出 `"type": "se"`，恰好与尚未实现 BGS 的 Rust 侧一致——**两边错到一起产生假阳性全等**。
3. **`diff_snapshots()` 忽略“新出现且为 0”与步内往返**：若某变量此前从未被赋值（未进入稀疏表），某指令显式写入 `0`，`diff_snapshots()` 不记录变化；若未来扩展到多协程同帧切换时序错误，只要未改变最终整数值也可能被掩盖。
4. **`Wait` 时长/种类偏差**：如 `180 Wait` 少等 1 帧或多等 10 帧，只要在下一个非阻塞分支前未影响后续帧号对齐（`diff_trace.py` 第 124 行**根本没有比较 `frame` 字段**！只比较 `["event_id", "pc", "op"]`），2461/2461 会完全漏掉时序偏差！

### 4.3 契约维护方案与整路线冻结输入夹具设计

- **契约防漂移方案**：将 `oracle/TRACE_CONTRACT.md` 升级为单一真源 JSON Schema（如 `oracle/trace_schema.json`），在 `oracle/diff_trace.py` 中首先做 schema 校验（禁止 `diff_trace.py:44` 无条件剥离空字段，显式区分缺省与空串），并增加 `frame` 字段比对选项；新增 `Bgs` 类型到契约（对 Rust 侧标记已知差异或同步补齐 `SoundChannel::Bgs` 仅用于 trace）。
- **整路线冻结输入夹具（`oracle/fixtures/full_route_input.json`）设计**：
  - **输入表达**：按帧索引的稀疏事件列表 `[{ "frame": 12, "key_mask": 1, "choice": 0, "move": [0, -1], "action": "save"|"load" }]`。
  - **时钟与随机数冻结**：固定 `WolfRng` 种子 `0`，确定性 `1/60s` 步进（禁止依赖 `delta` 实时时钟）；`GameClock` 仅由脚本 `CE217` 驱动。
  - **打破循环论证**：期望输出**必须由独立运行的 Rust Oracle CLI（`wolf-cli` 读取同一份输入序列）离线生成并冻结 SHA256**，绝不能由 C# `GameSession` 自产自销（如当前 `SessionParityTest` 的 `session_old_path_state.json`）。工作量约 **2–3 人日**（优先于 M3 展开前或 M3 第 1 周完成）。

---

## 5. `StrictMode` / `Faulted` 恢复路径评估

- **当前覆盖率不足以支撑正式版无条件常开 `StrictMode`**：虽然转换器在 M0 报告未知 opcode = 0，但 `EventVm.cs:500-1112` 仅覆盖已用到的高频指令，550 个公共事件与 73 个地图事件中的冷门分支、H 场景、战斗调度（`CE159/162`）在 M3/M4 才开始实测。若玩家在正式版（`GameRoot.gd:67`）触发未实现 opcode，直接锁死输入并禁止写档会导致玩家丢失数小时未保存进度，与 `D-10`“改善体验、避免死档”存在张力。
- **检出时序缺口**：`GameRoot.gd:129` 仅在 `advance_frame()` 的 `session.AdvanceFrame()` 返回后检查 `vm.Faulted`。若故障发生在 `LoadGame()` 期间（如恢复协程时触发异常）或信号回调中（`MessageRequested` 等），无法立即拦截。建议在 `_on_*` 回调入口及 `LoadGame()` 后也同步检查 `vm.Faulted`。
- **诊断包字段补强**：`export_diagnostics()`（`GameRoot.gd:421-443`）目前缺少**当前活动实例栈（`EventId`/`Pc`/`ParentId`）**、**最后 10 条执行指令历史**及 **关键系统/时钟变量（Var4/5/6/160）**，建议由 `VmController` 暴露环形指令缓冲写入诊断文件。
- **推荐恢复路径**：
  1. **M2–M4 开发与 CI 期**：保持 `StrictMode = true` + 立即 `Faulted` 暴露全部未实现分支。
  2. **M5 发布期保护**：保留 `Faulted` 模态与主存档槽写锁（绝不污染 `save_01.json`），但增加**“导出诊断包并写入独立隔离故障档（`fault_autosave_01.json`）”**或**“回退至上次进入地图前的内存/磁盘安全快照”**选项，避免玩家进度归零。

---

## 6. 26 道门禁可信度评估

### 6.1 逐项“证明了什么 / 没证明什么”

| 门禁编号 | 名称 | 证明了什么 | 没证明什么 |
| --- | --- | --- | --- |
| 1–2 | `versions` / `build solutions` | Godot 4.7.2 .NET 与 C# 工程可编译（上游日志） | 未在无缓存干净环境验证，依赖本地 `/e/Study` 路径 |
| 3–4 | `Smoke` / `SaveImportTest` | 基础核心类型与 8 槽 `.sav` 最小解析不崩溃 | 未覆盖损坏/GBK 混杂的真实玩家 `.sav` 边界 |
| 5–6 | `diff_trace.py` / `TraceDiffTest` | `diff_trace.py` 自检通过；`CE42` 10 步占位比对通过 | 仅 10 步极短路径，非全流程 |
| 7–12 | `Title` / `NewGame` / `Presentation` / `MapRuntime` / `SaveLoad` / `MapMessage` | **旧 `TitleScreen.cs` 路径**可走通标题→卧室→存读档（94/94）与地图触发 | **不是发布路径 `GameRoot.gd`**；无真实音频/渲染/指针输入校验 |
| 13–16 | `class cache` / `InputRouter` / `LayerOrder` / `AssetDB` | 9 级图层 z-order、InputRouter 掩码与悬停规则、AssetDB LRU 与占位回退 | 仅单元逻辑，未测真实鼠标拖拽与 384MB 极限驱逐 |
| 17–18 | `DialogDrawTest` / `PictureRoundtripTest` | 默认 headless 下仅验证布局矩形与指令簿记（`RENDER_SKIPPED`）；`LWsmf_RENDER=1` 下验证像素一致 | 默认 CI 无 GPU 时不跑像素对比；且使用系统回退字体而非零售字体（`needs-retail-data`） |
| 19 | `OracleStepTraceTest` | `SampleMapA` 事件 0（418 帧 / 2461 步）控制流、整数变量与 7 类 effects 与 Rust 一致 | 漏字符串变量、`Bgs` 映射误为 `se`、`frame`/`Wait`/转场未比对 |
| 20 | `GameRootTest` | 新主场景 `GameRoot.gd` 信号连线、标题→卧室 418 帧、F5/F9 音频立绘往返、`D-13` 锁档 | 未测 `MapLayer.cell_clicked`、未测 101/102 在 `GameRoot` 下的真实鼠标点选流程 |
| 21–22 | `TerrainTest` / `MapLayerTest` | 3 张地图阻塞数与 Rust 一致；`MapLayer` 能画占位色块并响应模拟 `_gui_input` | **3 层 × 16 种角模式组合爆炸未穷举**；`MapLayer` 用了重复的 GDScript 算法 |
| 23 | `SessionParityTest` | 新旧路径同脚本输入 418 帧后的 `SaveState` JSON 一致 | 不比对画面、音频、HUD、输入响应；**不能单独作为退役旧层的充分条件** |
| 24 | `AudioManagerTest` | 三轨独立、snapshot/restore（含 `loop_point`）、音量映射与坏输入拒绝（用静音 stub WAV） | **未测淡入淡出实际音量随时间变化、未测 `tempo` 变调、未测 WAV `loop_begin` 单位（导致漏掉 Bug）、未测同文件跨轨** |
| 25–26 | `ClockTest` / `SleepSoakTest` | `CE217` 单次进位与 `CE329->CE317` 连续 30 天就寝链（61 断言）不崩溃 | **未测白天分支 `CE251/310`、行动值、随机事件、12 个月/跨年、以及 `GameClock` 与 UI 显示同步** |

### 6.2 “默认门禁不是发布路径”的改造建议

1. **新路径专属门禁**：将第 7–12 项的核心场景测试（标题、新游戏、消息中存读档、选择中存读档、接触触发）**迁移或镜像到 `GameRoot.tscn` 驱动**，使发布路径成为默认门禁的主测试载体。
2. **无 GPU CI 渲染方案**：保留 `--headless` 作为快速逻辑门禁，但在 Windows x64 本地/Nightly CI 中固定以 `LWsmf_RENDER=1 --rendering-driver opengl3`（或软件渲染器 Mesa/SwiftShader）运行 `DialogDrawTest` 与 `PictureRoundtripTest`，不允许长期跳过像素回归。
3. **补齐测试矩阵**：`TerrainTest` 增加 3 层叠加 × 16 种四分格角模式的随机/穷举单测；`AudioManagerTest` 增加 WAV `loop_begin` 采样换算、淡入 Tween 完成态及跨轨同文件隔离断言；`SleepSoakTest` 扩充至包含白天行动点消耗与随机事件触发（见 §7.2）。

---

## 7. 四件未完成工作的优先级与方案

| 优先级 | 任务 | 性质归属 | 具体方案与建议 |
| --- | --- | --- | --- |
| **P0（最先）** | **1. 整路线 Oracle 冻结与对拍补全** | 核心验证底座 | 1) 先修 `EventVm` `300` 缺 `CommonEventCall` 及 `TraceRecorderHost` `Bgs` 映射；2) 扩展对拍字段覆盖 `StrVars` 与 `frame`；3) 用 `wolf-cli` 跑通“标题→新游戏→Map001→消息/选择→存档/读档”并冻结输入与期望 trace；不依赖美术素材即可完成。 |
| **P1** | **2. 白天工作菜单驱动（`CE251/CE310`）** | **测试夹具与宿主查询缺口**（非纯核心缺失） | C# `EventVm` 已支持 `124 PictureState` 与 `123` 轮询，但 `GameSession` / `TraceRecorderHost` 缺**图片点击/坐标命中模型**（Rust `m12_multiday_soak.rs` 已有）。最小可行方案：在 `TraceRecorderHost` 维护轻量级 `PictureRect` 表（记录 `150` 绘制的槽位坐标与尺寸），在 `ChoiceSelector` 之外增加 `PictureClickProvider` 响应 `123/124` 鼠标轮询，即可在 headless 中驱动 `CE251/CE310`（约 1–2 人日）。 |
| **P1** | **3. 旧表现层退役（`presentation/*.cs`）** | 架构清理 | 替换 `SessionParityTest` 的“旧路径依赖”：用 Rust Oracle（或冻结的 `GameRoot` 基线 `SaveState` + 关键帧截图哈希）作为黄金夹具，确认新路径通过全部 7–12 项迁移测试后，即可安全删除 `TitleScreen.cs`、`ScreenHostView.cs`、`ScreenHostSessionView.cs`、`AssetCatalog.cs`（仅保留 `VmController.cs` 与 `GameSessionController.cs` 作为 C#/GDScript 桥）。 |
| **P2（受外部素材制约）** | **4. MapChip 贴图缺失** | **需要用户提供零售素材**（`needs-retail-data`） | 通行度与碰撞交互可通过 `Terrain` + `MapLayer` 占位色块**独立验收**（`TERRAIN_TEST_OK` 已证明）；但视觉还原必须由用户从正版零售包提取 MapChip 与 Autotile PNG 放入 `work/all/MapChip/`，并在 `MapLayer.gd` 替换 `_make_tileset()` 与 autotile 拼接逻辑。 |

---

## 8. M3–M5 路线修订

| 阶段 | 现在该做什么与先后顺序 | 退出标准（DoD） |
| --- | --- | --- |
| **M2 收口补丁（2–3 天）** | 1. 修 `EventVm` `300` 调用、`AudioManager` WAV `loop_begin` 单位、`SaveCodec.MapFile` 校验、`GameSession` 传送目标/上限、`MapLayer` 去双实现并接线 `cell_clicked`；<br>2. 将 7–12 项门禁迁至 `GameRoot` 路径，补齐整路线 Oracle 夹具与 `StrVars` 对拍；<br>3. 接入 `CE251/CE310` 最小图片点击夹具并退役旧 `presentation/*.cs`。 | 1. 发布路径覆盖全部存读档/地图/消息门禁；<br>2. 整路线 Oracle 对拍（含整数+字符串+BGS）100% 通过；<br>3. 旧 `presentation/*.cs` 冗余视图已移除。 |
| **M3 战斗、H 与存档完善** | 1. **前置**：用户完成零售实机抓包（`123` 掩码、`CE159/162` 战斗流程、`126 BanKeys`）；<br>2. 实现剩余战斗/H 相关 opcode（含 `124` 完整查询、`250` 数据库读写接入真实 CData）；<br>3. 完善 `LegacySaveImporter`（CP932/GBK 严格解码、坏 checksum 拦截、8 槽导入测试）；<br>4. 接入零售字体与 MapChip 贴图（若素材到位）。 | 1. `CE159/162` 战斗与 H 场景变量/副作用与 Rust Oracle 一致；<br>2. 8 槽真实 `.sav` 导入与坏档拦截全部通过；<br>3. `StrictMode = true` 下已知战斗/H 路径零 `Faulted`。 |
| **M4 全内容对齐** | 1. 550 公共事件、111 随机事件、9 结局、DLC 链全量覆盖矩阵；<br>2. 30/90 天完整日程 soak（含白天工作、行动点、天气、节日）；<br>3. 制定发布期 `Faulted` 隔离存档/安全点恢复策略。 | 1. 内容覆盖矩阵 100% 有状态（无静默未测）；<br>2. 90 天全分支 soak 零 P0/P1 阻断；<br>3. Oracle 对拍收口，Rust 退出日常门禁。 |
| **M5 收口与发布** | 1. 60fps 性能压测与 384MB `AssetDB` LRU 极限验证；<br>2. Windows x64 打包导出与用户本地 `game_data/` 转换引导；<br>3. 版权与隐私边界终检（Git/发布包零零售素材）。 | 1. Windows x64 导出包可独立引导并稳定 60fps；<br>2. 全部门禁在 `LWsmf_RENDER=1` 渲染模式下通过。 |

---

## 9. 未决问题

### 必须现在决定（进入 M3 前）
1. **发布期 `StrictMode` 遇到未实现 opcode 时的恢复策略（`D-13`/`D-15`）**：是否允许在锁定主存档槽的同时，自动保存一份可诊断的隔离故障档或支持“回退到上次进图安全点”？
2. **旧 `presentation/*.cs` 退役时机**：是否同意用“冻结的 Rust Oracle 整路线输出 + `GameRoot` 基线快照”立即替换 `SessionParityTest` 中的 `TitleScreen.cs` 夹具？

### 可延后（M3/M4 期间决定）
1. **`GameClock` `13:70` 单次进位怪癖**：核心逻辑保持原版以保剧情一致，UI 状态栏显示是否需要规范化进位展示？
2. **音频共享流克隆策略**：是按通道隔离 `AssetDB` 缓存键，还是仅在 `AudioManager` 检测到同文件跨轨时 `.duplicate()`？

### 需要用户证据（`needs-retail-data` / `needs-runtime`）
1. **MapChip 与零售字体资源**：`work/all` 缺失的 MapChip/Autotile 图片及 `Lolita.ttf` 字体文件（用于替换色块与消除换行偏差）。
2. **`opcode 123` / `126 BanKeys` 实机行为**：确认原版 `126` 是在下一次 `123` 轮询后清除、每帧清除还是持久化；确认窄 `mask` 对 `123` 按键的过滤语义及 `771` 滚轮之外的特殊键码。
3. **`101` 消息窗未读完时按取消键的行为**：确认原版是“立刻全显文本”、“忽略按键”还是“直接关窗跳过”。
4. **负缩放与 WAV 循环点实机表现**：确认零售图片负缩放在非 `TextureRect`/特定锚点下的真实像素偏移，以及零售音频中是否存在带 `loop_point` 的 `.wav` 文件。
5. **`Map024` / `Map031` 无图块层**：需核对原始 `.mps` 数据是否本身为空图。
