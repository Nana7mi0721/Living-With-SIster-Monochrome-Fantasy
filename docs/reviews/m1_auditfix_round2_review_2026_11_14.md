# LWSMF（《妹相随》）Godot 4.7.2 .NET M1 + M2 W1 第二轮独立技术审阅报告

> **审阅对象**：`imoto_life_godot_m1_auditfix_2026-11-14.zip`（M1 完成 + M2 W1 审计加固快照）  
> **最高决策源**：`project/docs/godot/DECISIONS.md`（`D-01..D-12`）  
> **证据边界声明**：远程沙箱无 `dotnet` / `godot` / `cargo`，**所有代码构建与门禁均未在沙箱内重跑**；`evidence/logs/godot_gate_w1.log` 为上游提供的本地门禁输出，`evidence/screenshots/` 为 W1 之前的旧截图，`evidence/save/save_gate_test_fixture_01.json` 为测试夹具。包内无 `game_data/`、原始 `.wolf/.dat/.mps` 及零售素材，凡需零售数据或实机运行才能证实的项均标注 `needs-retail-data` 或 `needs-runtime`。

---

## 1. 总评：能否进入 M2 W2？

**结论：有条件可以进入 M2 W2（Conditional Go）。**

W1 确实补上了 M1 最致命的 3 个核心语义缺口（`CommonSelf` 跨调用持久化、稀疏 `SelfVars` 体积从 747 KB 降至 14.7 KB、单 60fps 时钟），但**绝不能把当前状态误判为“核心已稳固”**。进入 M2 W2 前或 W2 第 1 天必须立即止血以下 **3 项阻塞性前置条件**：

1. **修复 `SaveCodec` 稀疏键崩溃与语义校验缺口（P0）**：`EventSelfVars.ExpandNums/ExpandStrings`（`SaveCodec.cs:41-60`）对负数键、`int.MaxValue` 溢出或超大键无任何上界校验，且延迟到 `TitleScreen.LoadGame` 才展开，导致畸形 JSON **绕过 `.bak` 回退直接崩溃**。
2. **修复测试污染真实 `user://`（P1）**：`SaveLoadTest.cs:49` 直接删除并覆写 `user://save_01.json` 与 `.bak`，必须在 W2 第 1 天改为可注入临时目录。
3. **显式处理 `NewGame` 路径上已出现的 `270 Party` / `201 Move`（P1）**：`godot_gate_w1.log:111-113, 127` 实锤记录了 `unhandled opcode 270/201` 仍在 `StrictMode=false` 下被静默 `pc++` 跳过。

| 关键判断 | 置信度 | 依据 |
| --- | --- | --- |
| **W1 解决了 `CommonSelf` 缺失与双时钟倍速，但引入了稀疏键 OOM/越界新风险** | **高** | `EventVm.cs:24,66,994,1040`、`VmController.cs` 已删 `AutoProcess`、`SaveCodec.cs:41-60` 无键范围校验 |
| **当前门禁全绿存在“测试假阳性”：未阻断 `270/201` 且未测整段 Oracle** | **高** | `godot_gate_w1.log:111-113` 含未处理 opcode 却输出 `GODOT_GATE_OK`；Oracle 仅测 CE42 10 步 |
| **表现层仍是 C# 过渡态（1,643 行），W2 必须按 `D-02` 切换为 GDScript** | **高** | `project/godot/presentation/*.cs` 尚未迁移，`InputRouter.gd`/9 级图层/自绘 101-102 均未存在 |
| **`LegacySaveImporter`、`AssetCatalog` 与 `ProgramLoader` 仍处于 M1 原始状态** | **高** | 与上一轮相比零代码改动（`diff -rq` 证实） |

---

## 2. 上一轮 15 项问题逐项复核表

| ID | 当前状态 | 严重度 | 证据（文件:行） | 触发条件 | 根因 | 影响 | 最小修复 | 验证方法 | 置信度 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `M1-P0-01` | **已修（有边界缺口）** | P1 | `EventVm.cs:24,66-69,982,994,1040`；`SaveCodec.cs:128`；`TitleScreen.cs:411-414,495-501,522` | 同一公共事件并发多实例、或带 `210` 参数覆盖/`StopAll` 后重入 | 已实现 `CommonSelf` 持久化与新游戏清空，但同 CE 并发多实例会后写覆写前写；仅用合成事件 60 测试 | 真实 550 CE 并发/嵌套下仍可能出现 `CSelf` 覆盖顺序偏差（`needs-retail-data`） | 禁止同 CE 并发写回或按原版实机对比并发策略；补 `210` 参数+`StopAll`+嵌套测试 | `VmTest.cs:450-472` + 新增真实 CE 对拍 | **高** |
| `M1-P0-02` | **部分修复** | P0 | `EventVm.cs:26,28,893-907`；`godot_gate_w1.log:111-113,127` | 生产默认 `StrictMode=false` 下执行 `270`/`201` 等 36 种未实现 opcode | 仅计数 `UnhandledOpcodeCount++` 并 `Host.Log` 后仍 `pc++`；门禁未断言 `UnhandledOpcodeCount == 0` | M1 新游戏链已实际跳过 `270 Party` 和 `201 Move`，M2 将掩盖更多缺失指令 | 门禁强制开启 `StrictMode=true` 或断言 `UnhandledOpcodeCount == 0`；生产遇未实现 opcode 置错 | `VmTest.cs:486-505`；门禁增加计数断言 | **高** |
| `M1-P0-03` | **部分修复** | P1 | `EventVm.cs:25,51-84,720-727` | `210` 传入未知 CE ID、或形成深层递归（<4096 次） | `CreateInstance` 返回 `null` 并 `RejectedSpawnCount++`，但 `210` 直接 `inst.Pc++` 不阻塞不报错；无调用栈深度限制 | 非法调用静默继续并可能写入“看似正常”的存档；递归至 4096 才停 | 增加 `MaxCallDepth = 64` 限制；拒绝时设置 VM 错误态并禁止存盘 | `VmTest.cs:474-541` + 新增递归深度测试 | **高** |
| `M1-P0-04` | **部分修复** | P0 | `SaveCodec.cs:41-60,151-222`；`TitleScreen.cs:376,497-507` | 1) 稀疏 `num_map` 含负键/超大键；2) `File.Copy` 写 `.bak` 断电；3) 仅 `.bak` 存在时看 `HasSave` | `FromJson` 只查 `Schema`/`SchemaVersion`；`ExpandNums` 无界且延迟到 `LoadGame` 崩溃；`HasSave` 不认 `.bak` | 畸形存档绕过 `.bak` 直接崩溃；断电可同时毁 `.bak`；`HasSave` 与 `Read` 矛盾 | `FromJson` 内置语义校验与键范围 `[0, 999]` 检查；`.bak` 改用 `.bak.tmp` + `Flush` + `Move`；`HasSave` 兼容 `.bak` | `SaveLoadTest` 注入负键/超大键/双坏档测试 | **高** |
| `M1-P1-01` | **已修（需实机）** | P1 | `VmController.cs:18,180+`（已删 `AutoProcess`）；`TitleScreen.cs:91-102` | 120/144Hz 显示器、窗口失焦/暂停、对话中并行事件 | 唯一时钟收口到 `TitleScreen._Process` 的 `1/60s` 累加器（`guard < 8`） | 单机 60fps 逻辑对齐，但失焦/掉帧超 8 帧步长及高刷屏抖动仍 `needs-runtime` | 后续迁至 `_PhysicsProcess(60Hz)` 并实机测试高刷屏与暂停恢复 | `needs-runtime`：120Hz 实机帧率测试 | **中** |
| `M1-P1-02` | **未修** | P1 | `project/godot/presentation/*.cs`（全目录无改动） | 进入 M2 表现层开发 | M1 过渡 C# 表现层尚未迁至 GDScript；`TitleScreen.cs` 仍混合业务、输入与测试钩子 | 违反 `DECISIONS.md D-02`，继续在 C# 写 UI 会固化错误分层 | M2 W2 按计划拆分为 GDScript 表现层 + `VmBridge` | `godot_check.sh` 在 GDScript 重构后全绿 | **高** |
| `M1-P1-03` | **未修** | P1 | `SaveCodec.cs:109-136`；`TitleScreen.cs:524` | 带立绘/CG/HUD 图片或 BGM 时存读档 | `SaveState` 仍无 `Pictures` 与 `Audio` 字段；`EnterMap` 仍直接 `ClearPictures()` | 读档后画面立绘与背景音乐全部丢失 | 在 `SaveState` 增加 `Pictures` 与 `Audio` 快照，在 M2 W2/W4 接入重建 | M2 截图回归对比 | **高** |
| `M1-P1-04` | **未修** | P1 | `LegacySaveImporter.cs:91-98,182-188` | 导入含 CP932/GBK 的零售 `.sav` 或校验和损坏的 `.sav` | `diff -rq` 证实零改动：仍硬编码 `Encoding.UTF8`，且 `checksumOk=false` 不拒绝导入 | 零售存档文本变乱码，坏档被静默导入（`needs-retail-data` 确认编码） | 注册 `CodePagesEncodingProvider` 并严格解码；`!checksumOk` 拒绝导入 | `SaveImportTest` 负向与多编码测试 | **高** |
| `M1-P1-05` | **未修** | P1 | `TitleScreen.cs:155-170` | `opcode 123` 传入特定键位掩码 | `KeyCode(uint mask)` 仍完全忽略 `mask` 参数 | 特定掩码轮询会误返回其他按键（`needs-retail-data`） | 本地实机抓包确认 `123` 掩码后在 `InputRouter` 按掩码过滤 | 本地实机录屏/抓包 | **高** |
| `M1-P1-06` | **未修** | P1 | `TitleScreen.cs:315-331` | 传送到 `map 1/3` 以外的地图，或同帧两次 `Teleport` | 仍硬编码 `map == 3` / `map == 1` 且用单个 bool 标志 | 无法进入其他 7 张地图，连续传送丢目标 | 改用 `PendingTeleport` 队列并支持全地图加载 | 多地图传送集成测试 | **高** |
| `M1-P1-07` | **部分修复** | P1 | `TitleScreen.cs:218-224`；`ScreenHostView.cs:318-357` | 鼠标静止悬停、使用 `Sys3/Sys4` 锚点、需要控制码排版 | 上下键索引已 clamp 到 `[0, count-1]`，但仍是原生 `Button` + 居中坐标 `(362, 220)` | 未完成自绘 101/102、字体度量与仅移动触发 hover | M2 W2–W3 替换为 `ChoiceWindow.gd` + `InputRouter.gd` | 多帧按键+鼠标移动回归测试 | **高** |
| `M1-P1-08` | **未修** | P1 | `AssetCatalog.cs:19-20,78-118` | 长流程加载大量 CG 或播放 `.wav` 音效 | 仍为无界 `Dictionary`，`.wav` 仍返回 `null` | 内存无限增长；`.wav` 音效无声 | M2 W2 替换为 `AssetDB.gd`（LRU 384MB + `AudioStreamWAV`） | 内存上限压测 + `.wav` 播放测试 | **高** |
| `M1-P2-01` | **已修** | P2 | `EventVm.cs:167,189`；`VmTest.cs:543-554` | `121` 指令执行 `int.MinValue / -1` | `BinaryOp` 与 `Compound` 已特判返回 `int.MinValue` | 消除了该特定除法 `OverflowException`，但其他 opcode 参数长度仍需系统校验 | 保持当前修复，并在转换器/VM 补齐参数长度边界检查 | `VmTest.cs:543-554` 已覆盖 | **高** |
| `M1-P2-02` | **未修** | P2 | `ProgramLoader.cs:88-160` | 频繁切图或重入事件 | 每次 `LoadMap` 仍重开并解析整个 `programs.json` | 切图产生重复 IO 与 JSON 解析 GC 卡顿 | 由 `ProgramDB` 启动时一次性缓存索引 | `needs-runtime` 切图耗时基准 | **高** |
| `M1-P2-03` | **部分修复** | P2 | `SaveCodec.cs:21-60`；`SaveLoadTest.cs:155-168`；`oracle/fixtures/` | 运行完整 418 帧切片对拍 | 稀疏化将存档从 747 KB 降至 14.7 KB，但 **418 帧整段 Oracle diff 仍未完成** | 仅靠 CE42 10 步无法保证 M1 418 帧与 Rust 完全一致 | 在 M2 W2 生成 418 帧冻结输入与 Rust Oracle 全量 diff | `oracle/diff_trace.py` 全切片 0 diff | **高** |

---

## 3. W1 新引入/新暴露的回归与异常问题表

| ID | 严重度 | 状态 | 证据（文件:行） | 触发条件 | 根因 | 影响 | 最小修复 | 验证方法 | 置信度 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `W1-P0-01` | **P0** | confirmed | `SaveCodec.cs:41-60`；`TitleScreen.cs:499,507,576` | 存档 `num_map`/`str_map` 含负键、`key >= 1000` 或 `int.MaxValue` | `ExpandNums`/`ExpandStrings` 未检查 `key < 0` 或 `key >= 1000`，`key + 1` 可溢出，且延迟到 `LoadGame` 才展开 | **直接触发 `IndexOutOfRangeException` 或数 GB OOM，且绕过 `.bak` 自动恢复** | 在 `FromJson` 校验所有 `NumMap`/`StrMap` 键 ∈ `[0, 999]`；`Expand` 钳制长度 `<= 1000` | `SaveLoadTest` 注入 `-1`、`int.MaxValue`、坏 UTF-8 测试 | **高** |
| `W1-P1-01` | **P1** | confirmed | `SaveCodec.cs:148-161`；`EventVm.cs:917` | 读入含重复 `SavedInstance.Id`、越界 `Pc`/`Page`/`WaitBlock` 或坏 `ParentId` 的存档 | `FromJson` 只检查 `Schema` 和版本 `1`，完全无语义校验 | `PollWaits` 访问 `Blocks[inst.Wait.Block]` 直接崩溃；重复 ID 导致状态错乱 | 在 `SaveCodec.ValidateSemantic` 中校验 ID 唯一性、`WaitKind` 枚举与非负索引，并在 `RestoreInstances` 校验 `Pc`/`Block` | 构造畸形 `SaveState` 注入 `Read` 断言返回 `null` 并回退 `.bak` | **高** |
| `W1-P1-02` | **P1** | confirmed | `SaveCodec.cs:170-181,204-222`；`TitleScreen.cs:376` | 1) `File.Copy` 写 `.bak` 时断电；2) 主档丢失但 `.bak` 存在；3) 非 `IOException` | `File.Copy` 无 fsync 与并发锁；`HasSave` 只查 `File.Exists(_savePath)`；`ReadOne` 漏捕权限/编码异常 | `.bak` 可能半写损坏；仅 `.bak` 存在时 UI 显示无存档；权限异常直接崩溃 | `.bak` 写入走 `.bak.tmp`+`Flush`+`Move`；`HasSave` 改为 `File.Exists(p) \|\| File.Exists(p+".bak")`；`ReadOne` 捕获 `UnauthorizedAccessException` | 删主档留 `.bak` 测 `HasSave` 与标题读档 | **高** |
| `W1-P1-03` | **P1** | confirmed | `SaveLoadTest.cs:49,180-189`；`TitleScreen.cs:67` | 运行 `bash tools/godot_check.sh` | 测试硬编码写入真实 `user://save_01.json` 并轮换 `.bak` | 每次跑门禁都会**销毁开发者真实存档**，并把测试夹具当成环境状态 | `TitleScreen` 支持外部注入 `SavePathOverride` 或测试使用 `user://test_saves/` | 跑完门禁后检查真实 `user://save_01.json` 未被修改 | **高** |
| `W1-P1-04` | **P1** | confirmed | `EventVm.cs:66-69,992-996,1040` | 同一公共事件并发运行 2 个实例，或 `StopAll()` 中断执行中的 CE | 并发实例共享同一 `CommonSelf[id]` 键，后结束者覆写前者；`StopAll` 无条件写回半成品 `Self` | 并发 CE 状态静默丢失（`needs-retail-data` 验证原版是否允许同 CE 并发） | 并发检测告警 + 核对 Rust/原版并发 CE 写回策略；补 `210` 参数+`StopAll`+嵌套测试 | 构造双实例并发 CE 单测对比 Rust `exec.rs` | **中** |
| `W1-P1-05` | **P1** | confirmed | `TitleScreen.cs:601-605`；`MapRuntime.cs:280-292` | 读档时若某实例恢复失败但 `SavedMapEvent.InstanceId` 仍指向它 | `EnterMap` 在 `RestoreInstances` 后无条件调 `StartPending`，静默清空悬空 `InstanceId` 并重启自动页 | 掩盖损坏存档或漏恢复的协程，导致事件从 `pc=0` 重跑 | 读档后校验 `SavedMapEvent.InstanceId` 必能在 `Vm.Instances` 找到，否则报错而非静默重启 | 注入悬空 `InstanceId` 读档测试 | **高** |
| `W1-P2-01` | **P2** | confirmed | `EventVm.cs:893-897`；`docs/OPCODE_SPEC.md:1402` | `0`/`99`/`401`/`420`/`499` 被当 NOP 跳过 | `99 Checkpoint` 在 `OPCODE_SPEC.md` 确认为 no-op，`0` 为空槽，但 `401/420/499` 依赖离线预编译块结构正确 | 若块配对有误会静默穿透分支；目前与 `OPCODE_SPEC.md` 一致（`needs-retail-data` 抽样复核） | 转换器静态校验所有 `401/420/499` 均已配对闭合 | 转换器 `validate.py` 块闭合检查 | **高** |

---

## 4. 存档威胁模型评估

| 威胁场景 | 当前 W1 防护 | 实际漏洞与后果 | 必须补齐的护栏 |
| --- | --- | --- | --- |
| **写主档时断电/崩溃** | `.tmp` + `Flush(true)` + `File.Move` | 主档安全，但此前 `File.Copy(..., ".bak")` 未 `Flush`，**可能同时损坏 `.bak`** | `.bak` 也走 `.bak.tmp` + `Flush(true)` + `File.Move` |
| **磁盘满 / 杀软锁定 / 并发保存** | `catch { Delete(temp); throw; }` | 无文件锁，连按两次 F5 并发写同一 `.tmp` 会抛 `IOException` | `SaveService` 加串行队列/互斥锁，禁止重入保存 |
| **主档损坏 + `.bak` 完好** | `Read()` 回退 `.bak` | 若损坏在 `NumMap` 键（`W1-P0-01`），**回退不会触发且直接崩溃**；若主档被删，`HasSave` 返回 `false` | `FromJson` 内执行完整语义校验；`HasSave` 检查主档或 `.bak` |
| **双坏档 / 截断 / 非法 UTF-8** | `Read()` 返回 `null` | `ReadAllText` 对非法 UTF-8 做有损替换而非抛异常；`ReadOne` 未捕获权限异常 | 使用 `UTF8Encoding(false, true)` 严格读取并在 `ReadOne` 捕获所有文件/解码异常 |
| **恶意/畸形超大 JSON 或深层嵌套** | `JsonSerializer` 默认限制（MaxDepth=64） | 未限制单文件体积上限；`NumMap` 单键可触发 4 GB 分配 | 读取前检查 `FileInfo.Length <= 4 MB`，限制 `NumMap`/`StrMap` 键范围 |
| **版本迁移** | 仅允许 `schema_version == 1` | 无 `SaveMigrator` 骨架，未来升 v2 需硬编码迁移链 | 在 `SaveCodec` 预留 `Migrate(JsonDocument, int fromVer)` 入口 |
| **测试污染真实 `user://`** | **无隔离** | `SaveLoadTest` 每次覆盖真实 `user://save_01.json` 与 `.bak` | 测试强制注入独立临时目录 |

---

## 5. 测试与门禁可信度评估（`godot_gate_w1.log`）

| 门禁测试项 | 上游日志结果 | 它真正证明了什么 | 它没有证明什么（盲区） |
| --- | --- | --- | --- |
| `SMOKE_OK` + `VmTest` | 通过（含 6 个 W1 新单测） | 合成事件下的 `CommonSelf` 累加、未知 ID 返回 `null`、`StrictMode` 抛异常、`int.MinValue/-1` 不崩溃 | 未测真实 550 CE `CSelf` 并发/嵌套；未测 `StrictMode=false` 下生产漏指令风险 |
| `SAVE_IMPORT_OK 8/8` | 通过 | 8 个零售 `.sav` 可解密且基础字段可读 | 未测 CP932/GBK 中文乱码、未拦截 `checksumOk=false` 坏档 |
| `TRACE_DIFF_OK` | 10/10 steps | `CE42` 前 10 步固定 fixture 与 Rust 一致、step 6 差异可检出 | **未覆盖 418 帧整段切片**、未覆盖地图事件与分支 |
| `TITLE_TEST_OK` / `NEW_GAME_TEST_OK` | 通过（4000 帧） | 标题 CE42 与 `Map003` 新游戏链不卡死 | 日志 **第 111–113 行实锤存在 `unhandled opcode 270/201` 却照样通过** |
| `PRESENTATION_TEST_OK` | 418 帧到达卧室 | 标题→map3→map1 链路连通，`SampleMapA_1` 3 个并行事件可跑 | **使用占位 `ColorRect`/`Label`**，无纹理/图块/音频/真实输入 |
| `MAP_RUNTIME_TEST_OK` | 51/51 | 合成地图上的页条件、接触框、碰撞逻辑 | 未测真实 9 张地图全量事件与复杂地形通行 |
| `SAVE_LOAD_TEST_OK` | 47/47 | 消息/选择中断点续跑、稀疏存档 < 50 KB、简单 `.bak` 回退 | **污染真实 `user://`**；未测负键/超大键/双坏档/并发写/图片音频恢复 |
| `MAP_MESSAGE_TEST_OK` | 12/12 | map 1 事件 4 消息与 `[训练]` 选项触发 | 未测控制码排版、字体度量、多选项 hover 与取消 |

---

## 6. M2 W2 按天可执行计划（进入 GDScript 表现层与核心加固）

> **前置门禁**：W2 Day 1 必须先完成 `W1-P0-01`（稀疏键校验）、`W1-P1-03`（测试 `user://` 隔离）及门禁断言 `UnhandledOpcodeCount == 0`（或显式白名单追踪 `270/201`）。

| 天数 | 任务内容 | 负责层 | 依赖与写入范围 | 可自动化验收命令 / 退出标准 |
| --- | --- | --- | --- | --- |
| **Day 1** | **核心止血与测试隔离**：修复 `SaveCodec` 稀疏键范围 `[0,999]` 与语义校验；`.bak` 原子化；`HasSave` 兼容 `.bak`；`SaveLoadTest` 改用临时目录 | C# (`core_cs` + `tests`) | 写入 `SaveCodec.cs`, `TitleScreen.cs`, `SaveLoadTest.cs` | `SaveLoadTest` 新增负键/超大键/仅 `.bak`/双坏档用例全绿，且真实 `user://` 零改动 |
| **Day 2** | **冻结 `VmBridge` 并建立 GDScript 骨架**：将 `TitleScreen`/`ScreenHostView` 逻辑迁出至 `presentation_gd/`（`GameRoot.gd`, `InputRouter.gd`）；实现统一 `InputRouter.gd`（右键=取消、滚轮、仅 `InputEventMouseMotion` 更新 hover） | C# (`VmBridge`) + GDScript (`presentation_gd`) | 依赖 Day 1；保留旧 `TitleScreen.tscn` 对照至切换完成 | 新增 `InputRouterTest.tscn`：验证静态鼠标不抢键盘光标、右键=ESC、滚轮事件 |
| **Day 3** | **9 级图层带与 `AssetDB.gd`**：实现 `PictureStage.gd` 严格 `z_index`（`Map0 < ScenePicture10 < SceneText11 < MessageFrame20 < MessageText25 < Nameplate30 < ChoicePanel40 < ChoiceText41 < Cursor50`）与槽位池；实现 `AssetDB.gd`（LRU 上限 384 MB + `.wav` 支持） | GDScript (`presentation_gd`) | 依赖 Day 2；写入 `PictureStage.gd`, `AssetDB.gd` | `LayerOrderTest.tscn` 验证 9 级绘制顺序；`AssetDBTest.tscn` 验证超 384 MB 触发 LRU 淘汰与 `.wav` 加载 |
| **Day 4** | **101/102 自绘与字体度量**：实现 `TextTokenizer.gd` + 自绘 `MessageWindow.gd`、`ChoiceWindow.gd`、`Nameplate.gd`（`_draw()` + `Font.get_string_size()`）；支持 `\cdb`/`\c`/`\f`/`\E`/`<R>`、锚点 0–6 与 `Sys3/Sys4`（离屏不画） | GDScript (`presentation_gd`) | 依赖 Day 3；对照 `docs/TEXT_SPEC.md` 与 `OPCODE_102.md` | `DialogDrawTest.tscn` 截图验证无截断/重叠；`needs-retail-data` 核对 `Lolita.ttf` |
| **Day 5** | **整段 418 帧 Oracle 对拍与图片入档**：为 M1 切片导出冻结输入序列，跑通 Rust vs C# 418 帧全量 `diff_trace.py`；在 `SaveState` 增加 `Pictures` 快照与读档重建 | Python (`oracle`) + C# + GDScript | 依赖 Day 1–4；写入 `SaveCodec.cs`, `PictureStage.gd` | `diff_trace.py` 418 帧 0 diff；带立绘 F5→F9 截图一致（**若失败则回滚表现层改动保留 Day 1 修复**） |

> **延后项（明确不进 W2）**：`MapChip` 全量 tile 导出与地形通行（放在 W3）、`AudioManager` 三轨循环点与 30 天日程时钟（放在 W4）、`LegacySaveImporter` CP932 改造与 `CE159/162`（放在 M3）。

---

## 7. M3–M5 路线修订（基于当前证据）

- **M2 W3–W4 修订**：必须把 **`ProgramDB` 启动缓存索引**（`M1-P2-02`）与 **全地图 `Teleport` 队列**（`M1-P1-06`）提前至 W3 地图呈现前完成，否则 `SampleMapA` 与其他地图切换无法测试；同时在 W4 前实现 `201 Move`（73 地图站点）与 `270 Party`（当前门禁已触发）。
- **M3（战斗与 H 系统）修订**：必须将 **`LegacySaveImporter` CP932/GBK 严格解码与坏校验和拦截**（`M1-P1-04`）列为 M3 硬退出门槛，与本地实机抓取的 **`123` 键码掩码**及 **`CE159/162` 战斗 trace** 同步验收。
- **M4（全内容对齐）修订**：Rust Oracle 在 M4 结束前必须完成 **418 帧切片 + 30/90 天日程 + 9 结局 + DLC 链**的全量 trace 对拍；门禁脚本 `godot_check.sh` 必须改为在 `StrictMode=true` 下零未处理 opcode 通过。

---

## 8. 未决问题清单

| 分组 | 问题 | 建议处理方式 |
| --- | --- | --- |
| **必须现在决定（W2 Day 1）** | 生产环境遇到未实现 opcode 时，是立即停机报错还是记录错误态并禁止存盘？ | 建议开发/门禁期 `StrictMode=true` 立即失败；发布期进入错误提示弹窗并禁止覆盖有效存档 |
| **必须现在决定（W2 Day 1）** | 测试隔离目录路径方案 | 建议 `TitleScreen` 暴露 `SavePathOverride`，`godot_check.sh` 统一指向临时目录 |
| **可延后（W3 / M3）** | 同一公共事件并发多实例时的 `CommonSelf` 写回语义 | 延后至 M3 结合 Rust `exec.rs` 与原版实机行为对齐 |
| **可延后（M3）** | `SaveMigrator` v1→v2 迁移策略与是否回写原版 `.sav` | 按 `DECISIONS.md` §5 留到 M3 决定 |
| **需要用户证据（`needs-retail-data` / `needs-runtime`）** | 1. `opcode 123` 掩码 `arg[1]` 与键码 `13/14/20/100/300` 实机抓包<br>2. `CE159/162` 战斗目标调度实机录屏<br>3. 零售 `.sav` 字符串真实编码（CP932 vs GBK）<br>4. 120/144Hz 实机时钟与 `Lolita.ttf` Compatibility 渲染效果 | 需用户在本地 Windows + 零售数据环境录制并提供证据 |
