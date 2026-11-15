# LWSMF（《妹相随》）Godot 4.7.2 .NET M1 审阅报告与 M2–M5 执行计划

> **审阅基准**：`imoto_life_godot_m1_2026-11-14`（快照日期：2026-11-14）  
> **最高决策源**：`project/docs/godot/DECISIONS.md`（`D-01..D-12`）  
> **工具链声明**：当前远程沙箱未安装 `dotnet` / `godot` / `cargo`，**所有构建与测试均未在沙箱内实际运行**；包内 `godot_check.sh`（12/12）与 `evidence/` 截图/日志为上游提交方提供的运行证据。凡无法仅靠静态代码证实的行为均已标注 `needs-runtime` 或 `needs-retail-data`。

---

## 1. 结论与置信度

| 评估维度 | 结论 | 置信度 | 关键依据 |
| --- | --- | --- | --- |
| **M1 纵向切片业务闭环** | **基本达成（带条件）** | **高** | `TitleScreen` → `Map003` → `Map001` → `101/102` → `SaveCodec` 原子写 → 读档续跑链路完整，`evidence/logs/` 与 `evidence/save/save_01.json` 相互印证 |
| **C# 核心确定性** | **中高风险（有 4 项 P0 隐患）** | **高** | `EventVm` 缺失公共事件 `CSelf` 持久化、静默跳过未实现 opcode、`210` 无深度限制且未知事件不报错 |
| **存档原子性与兼容性** | **中风险** | **高** | 已有 `.tmp` + `Flush(true)` + `File.Move`，但缺 `.bak`、`SchemaVersion` 校验与损坏回退；`LegacySaveImporter` 仍硬编码 UTF-8 且未拦截坏校验和 |
| **架构分层合规性（D-02）** | **未对齐（过渡态）** | **高** | 当前 `presentation/*.cs`（1,643 行）全为 C# 且 `TitleScreen.cs` 混杂了输入、业务、测试钩子与占位 UI，必须按 M2 计划迁至 GDScript |
| **Oracle 对拍覆盖度** | **低（仅 10 步 fixture）** | **高** | 仅覆盖 `CE42` 固定 10 步；418 帧切片尚未与 Rust Oracle 做逐帧 diff（`needs-runtime`） |

**总评**：M1 证明了“C# 确定性核心 + 离线 `programs.json` + 完整实例状态存档”路线在 Godot 4.7.2 .NET 上完全跑得通，但**绝不能直接带着当前的过渡结构进入 M2 大规模内容扩充**。进入 M2 前必须优先修复 `EventVm` 的 `CSelf` 持久化、未知 opcode/事件 fail-fast、`SaveCodec` `.bak` 与版本校验，并完成向 **GDScript 表现层 + `InputRouter` + 9 级图层** 的切换。

---

## 2. M1 反 Happy-Path 审计问题表（15 项）

| ID | 严重度 | 状态 | 位置（文件:行号） | 触发条件 | 根因 | 影响 | 最小修复建议 | 验证方法 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `M1-P0-01` | **P0** | confirmed | `EventVm.cs:43-72, 980-1014`；`TitleScreen.cs:406-409` | 公共事件（如 `CE251` 倒计时、`CE452` 章节）结束并再次被 `210` 调用，或存读档 | `EventVm` 无 `CommonSelf` 持久化字典；`CreateInstance` 每次新建空白 `SelfVars`，`ReapFinished` 直接丢弃；`SaveGame` 只存地图事件 `SelfVars` | 跨次调用公共事件时 `CSelf0..99` 全归零，M2 日程/章节逻辑必然崩坏 | 在 `EventVm` 增加 `Dictionary<int, SelfVars> CommonSelf`，结束时写回、启动时复制，并纳入 `SaveState.SelfVars` | 新增 `VmTest` 用例：连续两次调用同一 CE 并存读档，校验 `CSelf` 保留 |
| `M1-P0-02` | **P0** | confirmed | `EventVm.cs:878-880` | 执行未在 M1 实现的 36 种 opcode 之一（如 `300 CallByName`、`151`、`125/126`） | `switch (slot.Opcode)` 的 `default` 分支直接执行 `inst.Pc++; break;`，既不报错也不记日志 | M2 扩大事件覆盖面时，未实现指令被静默跳过，脚本状态悄悄漂移 | 在 `default:` 中记录未实现 opcode 诊断并触发 fail-fast（测试/严格模式抛异常或暂停 VM） | 新增负向测试：执行含未实现 opcode 的槽位，断言进入错误态而非跳过 |
| `M1-P0-03` | **P0** | confirmed | `EventVm.cs:58-72, 710` | `210` 调用动态计算出的非法/不存在 `targetEvent`，或形成递归自调用 | `CreateInstance(int eventId)` 在 `GetProgram` 为 `null` 时静默返回 0 行空程序；且无 `MaxInstances` / 栈深上限 | 非法调用静默成功；循环递归 `210` 会无界分配直至 OOM | 未知 `eventId` 报错并置错误态；增加 `MaxInstances = 256` 上限，超限熔断 | `VmTest` 负向测试：未知 CE ID 与无限递归 `210` 触发受控失败 |
| `M1-P0-04` | **P0** | confirmed | `SaveCodec.cs:101-145` | 1) 读到 `schema_version != 1` 的存档；2) `save_01.json` 被截断/损坏；3) 写档后需要回滚 | `FromJson` 未检查 `SchemaVersion`；`Write` 直接 `File.Move(temp, path, true)` 未保留 `.bak`；`Read` 损坏时直接返回 `null` | 版本错配静默误读；主档损坏后无 `.bak` 可恢复（违反 `D-08`） | `Write` 前将现有文件移至 `.bak`；`FromJson` 校验版本并接入 `SaveMigrator`；`Read` 失败时尝试读取 `.bak` | `SaveLoadTest` 增加损坏 JSON 回退 `.bak` 与错误 `schema_version` 拒绝测试 |
| `M1-P1-01` | **P1** | confirmed | `VmController.cs:180-186` | `AutoProcess = true` 且在 120Hz/144Hz 屏幕运行，或对话期间有并行事件运行 | `_Process` 未做 `1/60s` 固定步长累加，且 `_isWaitingChoice`/`_isWaitingMessage` 会冻结整个 `StepFrame()` | 高刷屏游戏倍速运行；对话期间 107 个并行后台事件被意外冻结 | 若启用 `AutoProcess`，改用 `_PhysicsProcess`（60Hz）并移除对话对全局 `StepFrame` 的阻断（仅阻塞对应协程） | `needs-runtime`：在 120Hz 下验证动画/等待帧与 60fps 一致 |
| `M1-P1-02` | **P1** | confirmed | `presentation/*.cs`（全目录 1,643 行） | 进入 M2 开发 9 级图层、101/102 自绘与 `InputRouter` | M1 为赶切片将表现层写在 C#，且 `TitleScreen.cs` 混杂了菜单、地图、存读档与测试逻辑 | 违反 `D-02`（GDScript 表现层），继续堆功能会把过渡代码固化成技术债 | M2 第 1 周将 `TitleScreen`/`ScreenHostView`/`MapView` 拆迁为 GDScript 场景 + `InputRouter.gd`，C# 只留 `core_cs` 与薄桥接 | `godot_check.sh` 12 道门禁在 GDScript 表现层重构后全绿 |
| `M1-P1-03` | **P1** | confirmed | `TitleScreen.cs:379-459`；`SaveCodec.cs:63-89` | 在有 CG/立绘/HUD 图片槽的场景按 F5 存档并读档 | `SaveState` 缺少 `Pictures` 与 `Audio` 字段；`EnterMap` 每次强行 `ClearPictures()` | 读档后所有立绘、CG、窗口图片与 BGM 全部消失 | 在 `SaveState` 增加 `Pictures`（槽位/资产/坐标/锚点/缩放/透明度）与 `Audio` 快照，读档时重建 | M2 截图回归：带立绘状态下 F5→清空→F9 读档对比 |
| `M1-P1-04` | **P1** | confirmed | `LegacySaveImporter.cs:91-98, 182-188` | 导入含 Shift-JIS（CP932）或 GBK 字符的零售 `.sav`，或导入字节损坏的 `.sav` | `ReadMemDataString` 硬编码 `Encoding.UTF8.GetString`；`ImportFile` 算出 `checksumOk == false` 后仍继续返回成功 | 日文/中文字符串变 `` 乱码；损坏的 `.sav` 被当成正常存档导入 | 注册 `CodePagesEncodingProvider`，按 CP932/GBK/UTF-8 严格解码；`!checksumOk` 时标记不可用并拒绝导入 | `SaveImportTest` 增加 CP932 字节样本与篡改校验和的 `.sav` 负向测试 |
| `M1-P1-05` | **P1** | confirmed | `TitleScreen.cs:155-171` | 脚本调用 `123 InputKey` 传入特定位掩码（非全键） | `KeyCode(uint mask)` 完全忽略 `mask` 参数，直接按 `KeyCodes.AllKeys` 返回 | 仅检测特定键位的脚本会误读其他按键 | 结合本地实机抓包明确 `mask` 语义后，按 `mask` 过滤 `held`（`needs-retail-data`） | 本地原版实机录屏/抓包核对 `opcode 123` |
| `M1-P1-06` | **P1** | confirmed | `TitleScreen.cs:125-142, 314-331` | 传送到 `map 1` / `map 3` 之外的地图，或同帧触发两次 `Teleport` | `OnTeleport` 硬编码 `if (map == 3) ... else if (map == 1)`；pending 使用单个布尔值 | 无法进入其他 7 张地图；连续传送会丢目标 | 改用 `PendingTeleport?` 结构体排队，支持任意 `mapId` 加载 | 切换至 `Map002`/`Dungeon` 等地图的集成测试 |
| `M1-P1-07` | **P1** | confirmed | `TitleScreen.cs:219-223`；`ScreenHostView.cs:318-357` | 选项窗按多次 `Down` 超出选项数后再按 `Up`；或静态鼠标悬停 | `_choiceIndex++` 未 clamp 上限；`ShowChoices` 硬编码 `(362, 220)` 且用原生 `Button` | 光标索引越界导致按键卡顿；未按 `Sys3/Sys4` 定位；原生按钮易重演 `RF-01` hover 覆盖 | Clamp `_choiceIndex` 至 `[0, count-1]`；M2 替换为自绘 `ChoiceWindow.gd` + `InputRouter` 仅移动触发 hover | 多帧连续按键边界测试 + `RF-01` 回归 |
| `M1-P1-08` | **P1** | confirmed | `AssetCatalog.cs:19-20, 78-118` | 长时间游玩加载大量 CG/BGM，或播放 `.wav` 音效 | `_textures` 与 `_audio` 为无界 `Dictionary`，无 LRU 淘汰；`.wav` 分支直接返回 `null` | 内存持续膨胀至数 GB；`.wav` 音效完全无声 | M2 迁入 GDScript `AssetDB.gd` 时加 LRU 内存上限（384 MB）并支持 `AudioStreamWAV` 加载 | 30/90 天 soak 内存监控 + `.wav` 播放验证 |
| `M1-P2-01` | **P2** | confirmed | `EventVm.cs:155-156, 206-213` | `121` 指令执行 `int.MinValue / -1` 或参数数组长度 `< 4` | C# `unchecked` 块中 `int.MinValue / -1` 仍抛 `OverflowException`；未校验 `args.Length >= 4` | 极端负数除法或畸形命令导致 VM 崩溃 | `BinaryOp` 与 `Compound` 对 `a == int.MinValue && b == -1` 特判返回 `int.MinValue`；校验 `args.Length` | `VmTest` 边界单测：`int.MinValue / -1` 不抛异常 |
| `M1-P2-02` | **P2** | confirmed | `ProgramLoader.cs:88-160` | 频繁切换地图或重入事件 | 每次 `LoadMap` 都重新打开并 `JsonDocument.Parse` 整个 `programs.json` | 切图产生毫秒级卡顿与大量临时对象 GC | 启动时由 `ProgramDB` 一次性索引缓存所有地图与事件 | `needs-runtime`：切图耗时基准测试 |
| `M1-P2-03` | **P2** | probable | `SaveLoadTest.cs`；`oracle/fixtures/` | 仅跑 `godot_check.sh` 12 道门禁 | Oracle diff 仅测 `CE42` 10 步；`save_01.json` 体积达 747 KB（每事件全量写 1000 槽空 `SelfVars`） | M1 全程 418 帧未与 Rust 对拍，可能掩盖深层变量偏差；存档体积偏大 | 为 418 帧切片生成完整 Rust trace 对拍；`SaveCodec` 对全 0/空 `SelfVars` 做稀疏序列化 | `oracle/diff_trace.py` 全切片 0 diff |

---

## 3. 旧 Rust 风险在 C# 中的对照（`LEGACY_RUST_AUDIT` 映射）

| 旧 Rust ID | 风险摘要 | M1 现状核查 | 覆盖情况 |
| --- | --- | --- | --- |
| `RUST-001` | 存档漏 VM 实例/RNG/音频 | **部分解决**：`SaveCodec` 已存 VM 实例 `pc`/`wait`/`loops` 与 `Rng`，但**仍漏公共事件 `CommonSelf`、图片槽与音频**（见 `M1-P0-01`、`M1-P1-03`） | 需 M2 补齐 |
| `RUST-002` | 非原子写档毁档 | **部分解决**：已实现 `.tmp` + `Flush(true)` + `File.Move`，但**缺 `.bak` 备份与损坏自动恢复**（见 `M1-P0-04`） | 需 M2 补齐 |
| `RUST-003` | 32 位补码与移位语义 | **已覆盖（缺一边界）**：C# `int` + `unchecked` + `& 31` 覆盖了加减乘与移位，但 **`int.MinValue / -1` 在 C# 仍会抛异常**（见 `M1-P2-01`） | 需补单测 |
| `RUST-004` | 超限静默丢弃子调用 | **未解决（变体）**：C# 无实例上限，且未知事件静默生成空程序（见 `M1-P0-03`） | **M2 前必须修** |
| `RUST-005` | `pc += 1` 静默跳过未知 opcode | **重现！**：`EventVm.cs:878-880` `default: inst.Pc++; break;` 原样重现了静默跳过（见 `M1-P0-02`） | **M2 前必须修** |
| `RUST-006` | `SaveTarget::Path` 路径穿越 | **已规避（暂）**：M1 硬编码 `user://save_01.json`，但 M2 实现 `220/221/222` 指令时必须加路径白名单 | M2/M3 护栏 |
| `RUST-007` | 未校验长度 `with_capacity` OOM | **已转移至离线**：Python 转换器接管了二进制解析，但 `LegacySaveImporter` 仍需对 `ReadMemData` 长度做上限保护 | M3 护栏 |
| `RUST-008` | `from_utf8_lossy` 污染文本 | **重现！**：`LegacySaveImporter.cs:97` 使用 `Encoding.UTF8.GetString` 无损外编码会静默变乱码（见 `M1-P1-04`） | 需 M2/M3 修复 |
| `RUST-009` | 负层延迟/快照补丁 | **未触发（未实现）**：M1 尚未实现 9 级图层与负层清理，切图直接 `ClearPictures()`；M2 必须用显式图层生命周期 | M2 架构重点 |
| `RUST-010` | 无界图片解码缓存 | **重现！**：`AssetCatalog.cs:19-20` `_textures`/`_audio` 仍是无界 `Dictionary`（见 `M1-P1-08`） | 需 M2 修复 |
| `RUST-011` / `013` | `textpic` 近似度量与锚点 5/6 | **半解决**：`ScreenHostView.AnchorPosition` 已支持锚点 0–6，但消息文本仍在用 `Label` + `CleanMessage` 剥离控制码 | M2 自绘解决 |
| `RUST-015` / `016` | `123` 掩码与 `CE159/162` | **未解决（`needs-retail-data`）**：`KeyCode(mask)` 仍忽略 `mask`；战斗未进入 M1 | 需本地实机抓包 |

---

## 4. 架构审查：C#/GDScript 分层、60fps 时钟、资源与存档

1. **C#/GDScript 分层审查（对照 `D-02`）**：
   - `core_cs/` 的纯度**总体很好**：`EventVm`、`MapRuntime`、`Variables`、`WolfRng`、`SaveCodec`、`LegacySaveImporter` 均零 `Godot` 命名空间依赖（仅 `GDScriptHost.cs` 为桥接），完全可脱离引擎做独立单元测试。
   - 问题在于 `presentation/*.cs` 与 `TitleScreen.cs`：它们直接用 C# 操纵 Godot UI 节点，且 `TitleScreen.cs` 兼任了状态机、输入路由、测试驱动（`ScriptedInput`）与存档编排。**建议在 M2 第 1 周立即将表现层迁至 `presentation_gd/`（GDScript），C# 仅通过 `VmBridge`（原 `VmController`）暴露信号与状态查询接口。**
2. **Godot 60fps 固定时钟审查**：
   - `TitleScreen._Process`（91-102 行）实现了 `1/60s` 累加器且有 `guard < 8` 防螺旋死锁，这是正确的；但 `VmController._Process`（180-186 行）存在另一套无累加器且会在对话时冻结全局的步进逻辑，**必须删除 `VmController.AutoProcess` 分支**，由唯一的主循环时钟驱动 `StepFrame()`。
3. **资源加载与存档原子性审查**：
   - `SaveCodec.Write` 使用同目录 `.tmp` + `Flush(true)` + `File.Move(..., overwrite: true)` 是正确的原子写入起点，但缺少 `.bak` 轮换与 `SchemaVersion` 校验；`evidence/save/save_01.json` 达 747 KB 是因为 `SelfVars` 固定输出 1000 个元素，建议改为仅序列化非零/非空槽位（预计可压缩至 < 40 KB）。

---

## 5. M2 逐项冲刺计划与 M3–M5 路线

### 5.1 M2 冲刺计划（主循环、GDScript 表现层与 UI，参考工期 4–6 周 / 2.5–3.5 人周）

| Sprint / 周次 | 任务项 | 负责层 | 退出验收标准 |
| --- | --- | --- | --- |
| **W1：核心加固与分层剥离** | 1. 修复 `M1-P0-01..04`（`CommonSelf` 持久化、未知 opcode/事件 fail-fast、`SaveCodec` `.bak`+版本校验、稀疏 `SelfVars`）<br>2. 冻结 `VmBridge` 接口，移除 `VmController.AutoProcess` | C# (`core_cs`) | `VmTest`、`SaveLoadTest` 含负向用例全绿；存档体积降至 < 50 KB |
| **W2：GDScript UI 骨架与输入** | 1. 建立 `InputRouter.gd`（右键=ESC、滚轮、仅移动触发 hover、防静止光标抢焦点）<br>2. 建立 9 级图层带 `PictureStage.gd`（`Map0..Cursor50`）与 `AssetDB.gd`（LRU 384 MB + `.wav` 支持） | GDScript (`presentation_gd`) | 9 级图层顺序测试通过；`RF-01` hover 与键盘导航互不干扰 |
| **W3：101/102 自绘与字体度量** | 1. 实现 `TextTokenizer.gd` + `MessageWindow.gd` / `ChoiceWindow.gd` / `Nameplate.gd`（`_draw()` + `Font.get_string_size()`）<br>2. 支持 `\cdb[...]`、`\c[n]`、`\f[n]`、`\E`、`<R>` 与 `Sys3/Sys4` 锚点 | GDScript (`presentation_gd`) | `UI-04/06/07/08/09/12` 截图回归通过；文字无截断/重叠 |
| **W4：地图 Tile、音频与日程** | 1. 转换器导出 `MapChip` 与通行度，接入 `TileMapLayer` 与点触<br>2. 接入 `AudioManager.gd`（BGM/BGS/SE + 循环点）<br>3. 补齐 418 帧切片完整 Rust Oracle diff，跑通 30 天日程 | Python + C# + GDScript | 30 天日程可玩无崩溃；M1 418 帧切片与 Rust Oracle 0 diff |

### 5.2 M3–M5 路线与退出标准

- **M3（战斗与 H 系统，5–7 周 / 3–4 人周）**：补齐剩余 36 种 opcode；结合本地实机录屏解开 `123` 掩码（`H-KEY-2`）与 `CE159/162` 战斗调度；实现 `LegacySaveImporter` CP932/GBK 严格解码与校验和拦截；完成 `CE333` 评估。**退出标准**：战斗数值、H 阈值与事件顺序与 Rust Oracle 一致，8 槽零售 `.sav` 全部无乱码导入。
- **M4（全内容对齐，5–7 周 / 2.5–4 人周）**：覆盖 550 公共事件、111 随机事件、9 结局与 DLC 链；跑通 30/90 天 soak 与 NG+ 跨周目。**退出标准**：内容覆盖矩阵无未解释缺口，P0/P1 清零。
- **M5（收口与发布，2–3 周 / 1–1.5 人周）**：Windows x64 导出打包、本地零售资源转换引导向导、60fps 性能与内存压测（< 512 MB）。**退出标准**：发布包零原版版权素材，全回归测试绿灯。

---

## 6. L1–L4 测试矩阵

| 层级 | 测试对象 | 执行方式 | 通过标准 | 当前 M1 覆盖状态 |
| --- | --- | --- | --- | --- |
| **L1 数据** | `converter/` 转换产物、`programs.json`、`manifest.json` | `python -m converter validate` + `pytest` | 550 CE / 73 地图事件 / 169,831 指令 100% 对账，未知 opcode = 0 | **已达成**（69/69） |
| **L2 行为** | `EventVm`、`MapRuntime`、`WolfRng`、`SaveCodec` | `godot --headless` + `oracle/diff_trace.py` | 与 Rust Oracle 同输入下 `pc`/变量/`effects`/RNG 0 偏差；含坏档/越界负向测试 | **部分达成**（仅 CE42 10 步，缺 418 帧全切片与负向 VM 测试） |
| **L3 表现** | `PictureStage`、101/102 自绘、`InputRouter`、`AudioManager` | GUI 截图比对 + 多帧输入注入 | 无闪烁/重叠/截断/立绘消失，9 级图层正确 | **占位状态**（M2 正式建立） |
| **L4 内容** | 550 事件、111 随机事件、9 结局、DLC、30/90 天 soak | 自动化路线脚本 + 覆盖率统计 | 全结局与 DLC 链可达且无崩溃，长流程内存稳定 < 512 MB | **未开始**（M3–M4） |

---

## 7. 未决问题与需本地取证项

1. **`opcode 123` 掩码与未命名键码 `13/14/20/100/300`（`needs-retail-data` / `needs-runtime`）**：需在本地通过原版实机录屏/抓包确认 `arg[1]` 掩码如何过滤组合键。
2. **`CE159/162` 战斗目标调度闭环（`needs-retail-data`）**：需在 M3 前抓取原版战斗实机 trace 作为 Oracle。
3. **Compatibility 渲染器下的 `Lolita.ttf` 字体度量与描边（`needs-runtime`）**：M2 自绘上线时需在 Windows x64 `Godot 4.7.2` 实机验证抗锯齿与描边清晰度。
4. **原版 `.sav` 文本编码分布（`needs-retail-data`）**：需抽样确认 8 个零售存档中 `md1` 与 `saveName` 究竟是纯 CP932 还是混杂 GBK（官方中文补丁），以决定 `LegacySaveImporter` 的解码顺序。
