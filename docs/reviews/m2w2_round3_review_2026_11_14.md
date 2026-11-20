# LWSMF（《妹相随》）Godot 4.7.2 .NET M2 W2 第三轮独立技术审阅报告

> **审阅对象**：`imoto_life_godot_m2w2_2026-11-14.zip`（M1 + M2 W1 + M2 W2 Day 1–5 快照）  
> **最高决策源**：`project/docs/godot/DECISIONS.md`（`D-01..D-12`）  
> **证据边界声明**：远程沙箱无 `dotnet` / `godot` / `cargo`，**所有门禁与构建均未在沙箱重跑**；`evidence/logs/godot_gate_w2.log` 与 `godot_gate_w2_windowed.log` 为上游本地结果。包内无 `game_data/`、原始 `.wolf/.dat/.mps`、零售图音字体与真实用户 `.sav`，无法静态证实的项均已标 `needs-runtime` 或 `needs-retail-data`。

---

## 1. 总评：能否把主场景切到 `GameRoot.gd`？

**结论：当前“不可以”直接将主场景切换至 `GameRoot.gd`（No-Go for immediate switch，需先完成 2 天对齐修复）。**

W2 Day 1–5 做得非常扎实：第二轮的 3 项阻塞前置条件均已实质关闭，`presentation_gd/` 9 个文件也搭起了干净的 9 级图层、101/102 自绘与 LRU 缓存骨架。但 **`GameRoot.gd` 目前只是“能接通信号的展示壳”，尚未具备 `TitleScreen.cs` 的完整游戏编排能力**（缺少地图视图/行走/接触框、存读档热键与界面、传送切换），且 `GameRoot.gd` 与 `PictureStage.gd` 自身存在 **4 项 P1 交互与渲染缺陷**（鼠标点击未确认 hover 项、取消消息不唤醒 VM、忽略 `_mask`/音量/淡入、Move 缺槽新建）。

| 关键判断 | 置信度 | 依据 |
| --- | --- | --- |
| **第二轮 3 项阻塞（`W1-P0-01`、`W1-P1-03`、`W1-P1-04`）均已实质关闭** | **高** | `SaveCodec.cs:94-116,283-330` 语义校验完整；`SaveLoadTest.cs:33-37` 隔离临时目录；14 个新 opcode 落地且 5 条门禁强制 `StrictMode=true` |
| **“2461/2461 全等”是真实的顺序逐条匹配，但仅证明控制流一致，零数据证明** | **高** | `diff_trace.py:124-180` 逐条比对 `event_id/pc/op/vars/effects`，但 `rust_step_trace.json` 与 `csharp_step_trace.json` 全量 `"vars":{},"effects":[]` |
| **默认 headless 门禁 17/18 不检查任何像素** | **高** | `godot_gate_w2.log` 显示 `DIALOG_DRAW_RENDER_SKIPPED` 与 `PICTURE_ROUNDTRIP_RENDER_SKIPPED` |
| **`GameRoot.gd` 存在 P1 交互缺陷且缺地图/存档编排，暂不可替换 `TitleScreen.cs`** | **高** | `GameRoot.gd:52-58,120-125,157-206` 与 `ChoiceWindow.gd:99-105` 核实 |

---

## 2. 第二轮 3 项阻塞前置条件复核表

| ID | 当前状态 | 严重度 | 证据（文件:行） | 触发条件 | 根因 | 影响 | 最小修复 | 验证方法 | 置信度 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `W1-P0-01` | **已修（有边界残余）** | P2 | `SaveCodec.cs:47-116,238,283-344,363-378`；`TitleScreen.cs:534-575` | 1) 混合 `num_map`+`nums`；2) `RestoreInstances` 时 `Pc > prog.Len`；3) `pic.File` 含 `..` | `FromJson` 已校验 `[0,1000)`、ID 唯一、环检测；但 `RestoreInstances` 未校 `Pc <= Program.Len`，`pic.File` 未禁 `..` | 负键/超大键 OOM 已彻底消除；仅剩加载后 `Pc` 越界与路径穿越防御缺口 | `SaveCodec.Validate` 禁 `pic.File` 含 `..`/盘符；`RestoreInstances` 校验 `Pc`/`WaitBlock` 范围 | `SaveLoadTest` 76/76 + 补充 `Pc` 越界测试 | **高** |
| `W1-P1-03` | **已修** | — | `TitleScreen.cs:67-69,380-387`；`SaveLoadTest.cs:33-37` | 运行 `bash tools/godot_check.sh` | `SaveLoadTest` 注入独立临时目录并在结束清理；`save_gate_residue_w1` mtime 证明 W2 未再污染 `user://` | 不再覆盖开发者真实存档；唯一注意点是清理临时目录后失败现场需看日志 | 失败时不删临时目录以便排障 | 跑门禁前后对比 `user://save_01.json` 哈希 | **高** |
| `W1-P1-04` | **已修（门禁关闭，生产未强停）** | P2 | `EventVm.cs:32-39,982-1094`；`godot_gate_w2.log:111-113`（已无 270/201 告警） | 1) 门禁跑 5 条主链；2) `300` 调用与 `210` 行为对比；3) `126` 禁用键后调 `123` | 14 个 opcode 已实现；门禁 `StrictMode=true`；但 `300` 未调 `Host.CommonEventCall`，且 `126` `BanKeys` 在 `123` 后未按注释清零 | `StrictMode=false` 时生产遇未实现 opcode 置 `Faulted` 并拒绝存盘（可能丢玩家进度）；`126` 注释与代码矛盾 | `300` 补调 `Host.CommonEventCall`；核实 `126` 原版是否单次清除（`needs-retail-data`）；发布期 `Faulted` 弹窗提示 | `VmTest` + `godot_gate_w2.log` 零 `unhandled` | **高** |

---

## 3. W2 新增代码审计表（`presentation_gd/` + 核心新逻辑）

| ID | 当前状态 | 严重度 | 证据（文件:行） | 触发条件 | 根因 | 影响 | 最小修复 | 验证方法 | 置信度 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `W2-P1-01` | **未修** | **P1** | `GameRoot.gd:50-56,112-118`；`ChoiceWindow.gd:99-105,122-124` | 玩家用鼠标悬停选项 2 并点击左键确认 | `GameRoot` 未连接 `router.hover_changed` 到 `choice.hover_at`，且左键确认走 `choice.confirm()` 提交 `selected`（仍为 0）而非 `_hover` | **鼠标点选选项永远选中第 0 项** | 连接 `hover_changed` 更新 `choice.hover_at`；左键点击若命中行则选中该行并确认 | `InputRouterTest` / `GameRootTest` 补充鼠标悬停+点击用例 | **高** |
| `W2-P1-02` | **未修** | **P1** | `GameRoot.gd:120-125,185-187` | 消息框打开时按 `ESC` / `X` / 右键 | `_on_cancel()` 调用 `message.close()` 但**未调用 `vm.DismissMessage()`** | 窗口关闭但 VM 永远卡死在 `WaitState.Message` | 消息框确认/关闭时统一判断或禁止 Cancel 跳过未确认消息（按 `OPCODE_101.md` 决定） | `GameRootTest` 测消息框按 Cancel 后 VM 不卡死 | **高** |
| `W2-P1-03` | **未修** | **P1** | `GameRoot.gd:62,157-172,197-206` | `123` 传特定 `mask`；`140` 传音量/淡入或 BGS；`130/160/290` | `SetKeyCodeProvider` 丢弃 `_mask`；`_on_sound` 忽略 `_volume`/`_frames` 且无 BGS 轨；传送/转场/特效仅写日志 | 特定键位过滤失效（`needs-retail-data`）；BGM 音量/淡入无效；无 BGS/转场/特效 | `_on_sound` 接 `volume_db` 与 Tween 淡入、补 BGS 轨；`KeyCodeProvider` 按 `mask` 过滤 | `GameRoot` 音频/转场集成测试 | **高** |
| `W2-P1-04` | **未修** | **P1** | `PictureStage.gd:80-93,156` | `150` 对未创建槽位执行 `action=1`（Move），或传负 `scale_percent`（水平镜像） | `action == 1` 槽位不存在时落入 `_build_picture` 新建节点；`scale_percent <= 0` 被强制当 `1.0` | Move 误建幽灵图片；负缩放水平镜像失效（`H-PIC-2`） | `action == 1` 无槽位时直接 `return null`；`scale_percent < 0` 时对节点做水平翻转 `scale.x = -abs(s)` | `PictureRoundtripTest` 补 Move 空槽与负缩放用例 | **高** |
| `W2-P1-05` | **未修** | **P1** | `TextTokenizer.gd:182`；`MessageWindow.gd:166-218` | 消息含 `\cdb[21:3:4]`（数据库颜色/文本）或 `\c[-1]` | `TextTokenizer._first_int` 只取首数字 `21` 丢弃 `:3:4`；`MessageWindow._layout` 未处理 `Kind.CDB`；`token.number % size` 未防负数 | `\cdb[...]` 在自绘窗口中被静默丢弃；负颜色索引会越界崩溃 | `TextTokenizer` 解析 `cdb` 三元组并查库；`COLOR_TABLE` 索引先 `posmod` 防负 | `DialogDrawTest` 增加 `\cdb[...]` 与 `\c[-1]` 用例 | **高** |
| `W2-P2-01` | **未修** | P2 | `AssetDB.gd:129` | 加载较大 `.ogg`/`.wav` 音频 | `FileAccess.get_file_as_bytes(path).size()` 为获取文件大小而将整份音频读入临时字节数组 | 音频加载瞬间双倍内存峰值与 GC 抖动 | 改用 `FileAccess.open(path, FileAccess.READ).get_length()` | `AssetDBTest` 验证内存分配 | **高** |
| `W2-P2-02` | **未修** | P2 | `InputRouter.gd:125-152` vs `KeyCodes.cs:57-78` | 未来新增按键或修改优先级 | 两份独立的掩码→键码映射表 | 长期维护必然漂移 | 由 C# `KeyCodes` 暴露统一静态方法给 GDScript 调用 | 删除 `InputRouter.code_for_mask` 重复实现 | **高** |
| `W2-P2-03` | **未修** | P2 | `MessageWindow.gd:13-24,62-63`；`UiSkin.gd:10-24` | 真实零售字体 `Lolita.ttf` 与全量立绘上线 | 当前常量来自 `OPCODE_101.md` 测量，但字体使用系统回退链 `Yu Gothic UI`，非零售字体（`needs-retail-data`） | 行宽与换行点可能与原版有 3–5% 偏差 | 本地接入 `Fonts/Lolita.ttf` 后跑视觉对比 | `needs-retail-data`：本地窗口化对比 | **中** |
| `W2-P2-04` | **未修** | P2 | `EventVm.cs:38,564` | `126` 禁用按键后执行 `123` | `EventVm.cs:38` 注释称“每次 `123` 轮询后清除 `BanKeys`”，但 `:564` 未清零 | 注释与代码行为相反（`needs-retail-data` 核对原版） | 核对 Rust/原版后决定保留或清零并同步注释 | `VmTest` 补 `126`+`123` 连续两次轮询测试 | **高** |

---

## 4. 逐指令对拍可信度评估（`2461/2461`）

1. **它证明了什么**：`diff_trace.py:124-180` 严格按顺序逐条比较 `steps[i]` 的 `event_id`、`pc`、`op`、`vars`、`effects`（并非集合相等），`ce42_corrupt_trace.json` 也证明第 6 步差异可被检出。因此 `2461/2461` **100% 证明了 `SampleMapA` 事件 0 在 418 帧 autorun 期间的控制流、分支跳转、循环计数与 `180 Wait` 帧同步完全一致**。
2. **它没有证明什么**：抽样核查 `evidence/oracle/rust_step_trace.json` 与 `csharp_step_trace.json` 证实，**全部 2,461 步的 `"vars": {}, "effects": []` 均为空**！这意味着如果某条 `121` 计算错了数值但未触发分支变化，或 `150` 画错了坐标/锚点，对拍依然会报 `2461/2461 100% matched`。此外，它**不含**标题 `CE42` → `Map003` → `Map001` 的完整玩家输入链。
3. **最小补强方案（约 1–2 天工作量）**：
   - 在 Rust `exec.rs` 与 C# `EventVm.StepHook` 中，记录每条指令执行前后的 **`vars` 变更字典（`vars_delta`）** 与 **宿主副作用摘要（`PIC_DRAW:slot:x:y`, `MSG:text_hash`, `CHOICE:n`, `SOUND:file`）**；
   - 新增一段覆盖 `标题 CE42 → 新游戏 → Map001 → 对话选择 → 存读档` 的**冻结输入序列夹具**，同步跑双端 `diff_trace.py`。

---

## 5. 双实现切换方案（`presentation/` → `presentation_gd/`）

> **策略建议**：**并行至 W3 末期再删除 `presentation/*.cs`**，绝不在当前直接删除旧路径（否则 1–12 门禁失去回归基线）。

1. **逐条对齐清单**：
   - **消息/名牌/选项**：`GameRoot.gd` 补齐鼠标 hover 联动点击（`W2-P1-01`）、Cancel 不丢 VM 等待（`W2-P1-02`）、`\cdb` 渲染（`W2-P1-05`）。
   - **立绘与存档**：统一 `SaveState.Pictures` 快照与 `PictureStage.draw_picture`（补齐 `action=1` 空槽与负缩放），删除 `ScreenHostView.PictureSnapshots` 后由 `PictureStage.export_snapshots()` 接管。
   - **地图与摄像机**：将 `MapView.cs` 逻辑拆为 GDScript `MapLayer.gd`（挂在 `PictureStage.LAYER_MAP = 0` 带下），与 `PictureStage`（1024×576 UI 坐标）通过 `CanvasLayer` 分离世界相机与屏幕 UI。
   - **`TitleScreen.cs` 拆分**：保留 `SaveCoordinator` / `MapSession` 确定性状态机于 `core_cs/`；菜单、F5/F9 快捷键、输入与 HUD 全部迁入 `GameRoot.gd`。
2. **切换验收门禁**：同一冻结输入序列分别驱动旧 `TitleScreen.cs` 与新 `GameRoot.gd`，断言两者的 `SaveState`（变量、RNG、实例 `pc`/`wait`、图片槽）**逐字段相等**。
3. **回滚条件**：切换后若任一原有 1–12 门禁或 418 帧状态快照出现偏差，保持 `project.godot` 主场景指向旧 `TitleScreen.tscn`，仅合并核心修复。

---

## 6. 存档威胁模型再审计

- **主档与 `.bak` 原子性**：`SaveCodec.WriteRaw`（`SaveCodec.cs:407-426`）对 `.bak` 使用 `path + ".bak.tmp"`，与主档 `.tmp` 无同名冲突，均经过 `Flush(true)` + `File.Move(..., true)`，断电安全。**残余风险**：无并发互斥锁，连按两次 F5 仍可能竞争同一 `.tmp`。
- **语义校验与路径穿越**：`SaveCodec.Validate` 已封死稀疏键 OOM、重复 ID、环父链与 8 MiB 体积炸弹；但 **`SavedPicture.File`（329 行）未禁止 `..`/绝对路径**，且 **`RestoreInstances` 未校验 `SavedInstance.Pc <= prog.Len`**。
- **音频入档**：`SaveState` 仍无 `Audio` 字段，建议放在 **W4 与三轨音频系统同步入档**。
- **`Faulted` 拒绝存盘**：`StrictMode=false` 下遇到未实现 opcode 置 `Faulted` 并拒绝存盘可防止坏状态污染好存档，但玩家会丢失当前进度；发布期必须弹出明确提示引导玩家“不覆盖当前槽位或导出诊断日志”。

---

## 7. 20 道门禁可信度评估

| 门禁组 | 证明了什么 | 没证明什么（核心盲区） |
| --- | --- | --- |
| **1–12（旧 C# 链）** | 核心 VM、地图条件、76 项存读档、稀疏化、`.bak`、临时目录隔离均通过 | 跑在**即将废弃的 `presentation/*.cs`** 上；未验证新 GDScript 表现层 |
| **13–16（GDScript 基础）** | 类缓存、InputRouter 规则（仅移动 hover/右键取消）、9 级 z-index、384MB LRU 触发 | `InputRouter` 未测触控板惯性抖动；`AssetDB` 默认依赖本地 `work/all`（CI 缺素材） |
| **17–18（`DIALOG_DRAW` / `PICTURE_ROUNDTRIP`）** | 窗口化（`LWsmf_RENDER=1`）下 101 布局无溢出且绘制→清空→重放 RGBA 逐字节一致 | **默认 headless 下输出 `RENDER_SKIPPED`，完全不检画面**；CI 无 GPU 时形同虚设 |
| **19（`ORACLE_STEP_TRACE`）** | `SampleMapA` 事件 0 的 418 帧 autorun 控制流 2461/2461 一致 | **`vars`/`effects` 为空**，未验证变量与副作用，未覆盖标题→卧室完整输入流 |
| **20（`GAME_ROOT_TEST`）** | `GameRoot.gd` 可加载且 8 信号已连接 | 仅测了 2 次手动调用 `_on_message`/`_on_choices`，**未测真实 VM 跑通、鼠标点击选项或 Cancel** |

> **CI 无 GPU 画面回归方案**：在 Windows 自托管 Runner 上以 `LWsmf_RENDER=1`（或软件渲染 `llvmpipe`/Mesa）作为合并前必跑项；headless 门禁必须对 `RENDER_SKIPPED` 显式标黄告警而非静默当成“视觉通过”。

---

## 8. M2 W3 / W4 按天计划与 M3–M5 路线修订

### W3：GDScript 表现层补齐、MapChip 与主场景切换

| 天数 | 任务内容 | 负责层 | 依赖与写入 | 验收命令 / 退出标准 |
| --- | --- | --- | --- | --- |
| **Day 1** | 修复 `GameRoot.gd` 4 项 P1 缺陷（hover 点击、Cancel 唤醒 VM、音量/淡入、负缩放/空槽 Move）+ `\cdb` 渲染 + `SaveCodec` 路径/PC 校验 | GDScript + C# | 写入 `GameRoot.gd`, `PictureStage.gd`, `MessageWindow.gd`, `SaveCodec.cs` | `tests_gd/` 全部通过 + 补齐鼠标点击选项目测试 |
| **Day 2** | 消除 `code_for_mask` 双实现；`ProgramLoader` 启动缓存索引（`M1-P2-02`）；全地图 `Teleport` 队列（`M1-P1-06`） | C# + GDScript | 写入 `ProgramLoader.cs`, `InputRouter.gd`, `TitleScreen.cs` | 切图耗时显著下降；可传送至 `Map002`/`Dungeon` |
| **Day 3** | Python 转换器导出 `MapChip` 与通行度；GDScript 实现 `MapLayer.gd`（TileMapLayer + 角色层 + 地图点触） | Python + GDScript | 依赖 `TileSetData.dat`；写入 `converter/`, `MapLayer.gd` | 卧室地图真实图块渲染与通行碰撞通过 |
| **Day 4** | 将 `TitleScreen.cs` 业务拆入 `core_cs/GameSession.cs`，`GameRoot.gd` 接管标题→新游戏→卧室→F5/F9 完整链 | C# + GDScript | 依赖 Day 1–3 | 新老路径同输入序列 `SaveState` 逐字段相等 |
| **Day 5** | 418 帧全切片 + `vars_delta`/`effects` 纳入 Rust/C# Trace 对拍；切换 `project.godot` 主场景至 `GameRoot.tscn` | Rust Oracle + C# + Python | 依赖 Day 4 | `diff_trace.py` 含变量/副作用 0 diff；20+ 门禁全绿（**若失败则回滚主场景指针**） |

### W4：音频、日程与旧层退役

- **W4 Day 1–2**：`AudioManager.gd` 实现 BGM/BGS/SE 三轨、循环点（`manifest.json`）与淡入淡出，并将 `Audio` 状态纳入 `SaveState`。
- **W4 Day 3–4**：接入日程/日期/行动值确定性时钟，跑通 30 天日程；确认无误后**删除旧 `godot/presentation/*.cs` 与 `AssetCatalog.cs`**。
- **W4 Day 5**：`StrictMode` 策略收口（门禁强制 `true`，发布期 `Faulted` 弹窗 + 保护原存档）。
- **M3–M5 修订**：M3 优先完成 `LegacySaveImporter` CP932/GBK 严格解码与坏校验和拦截（`M1-P1-04`），并结合本地实机抓包对齐 `123` 掩码与 `CE159/162`；M4 完成 550 CE/9 结局/DLC 全量 `vars+effects` Oracle 对拍。

---

## 9. 未决问题清单

| 分组 | 问题 | 建议处理方式 |
| --- | --- | --- |
| **必须现在决定（W3 Day 1）** | `AssetDB` 在 CI 上缺少 `work/all` 时的素材提供方式 | 建议提交极简非版权占位夹具包（仅含测试用 1×1 PNG/WAV）供 CI 使用，零售目录仅本地挂载 |
| **必须现在决定（W3 Day 1）** | 发布期遇未实现 opcode 置 `Faulted` 后的用户交互 | 弹出模态错误框 + 允许“导出诊断包”，**禁止覆盖已有存档槽** |
| **可延后（W4 / M3）** | 公共事件并发多实例 `CommonSelf` 写回顺序 & `126 BanKeys` 是否单次清除 | 延后至 M3 对照 Rust `exec.rs` 与原版实机行为统一裁决 |
| **需要用户证据（`needs-retail-data` / `needs-runtime`）** | 1. `opcode 123` 掩码与 `126` 清除语义实机抓包<br>2. `CE159/162` 战斗调度实机录屏<br>3. `Fonts/Lolita.ttf` 与零售 `Picture/face*` 在 `GameRoot.gd` 下的实机视觉对照<br>4. 零售 `.sav` 编码分布（CP932 vs GBK） | 需用户在本地 Windows + 完整零售目录运行并反馈 |
