# Godot 阶段交接：M0 数据与工具链

> 状态：**已完成 / 冻结验收**
> 对应计划：`docs/godot/PLAN.md` §3
> 决策记录：`docs/godot/DECISIONS.md`
> 数据契约：`docs/godot/DATA_CONTRACT.md`
> Trace 契约：`oracle/TRACE_CONTRACT.md`

---

## 1. 状态

- **阶段/日期**：M0（数据与工具链）/ 2026-11-14
- **仓库 HEAD**：`8c65153318e49d77711287d2a0226abc3d748b80`
- **Godot 版本/渲染器**：Godot 4.7.2-stable Mono Win64 (`ed1daf0bf`)，兼容渲染器（Compatibility Renderer）
- **.NET SDK**：.NET SDK 8.0.428 (`E:\Study\tools\dotnet8`)，C# 运行时 8.0.34
- **数据根/夹具**：
  - WOLF 数据源：`work/retail_backup/Data/`（二进制基准，不进 git）
  - 零售存档源：`work/retail_backup/Save/SaveData01..08.sav`（不进 git）
  - Oracle 夹具：`oracle/fixtures/ce42_reference_trace.json`，`oracle/fixtures/ce42_corrupt_trace.json`
- **是否可复现**：**是**（运行 `bash tools/godot_check.sh` 与 `pytest converter/tests` 即可一键全绿复现）

---

## 2. 本批交付

- **Godot 4.7.2 .NET 工程骨架 (`godot/`)**：
  - `godot/project.godot`：Godot 4.7.2 配置，窗口 1024x768，禁用拉伸滤镜，配置 Mono/C# 构建管道。
  - `godot/Lwsmf.csproj`：C# 8.0/net8.0 项目文件，配置 `Nullable` 检查。
  - `godot/core_cs/LegacySaveImporter.cs`：C# 原版 8 槽 `.sav` 解密、校验与字段恢复核心。
  - `godot/tests/Smoke.cs` & `Smoke.tscn`：引擎与 .NET 运行时 headless 冒烟测试场景。
  - `godot/tests/SaveImportTest.cs` & `SaveImportTest.tscn`：C# 原版存档导入与校验 headless 测试场景。
  - `godot/tests/TraceDiffTest.cs` & `TraceDiffTest.tscn`：C# Oracle trace 契约与 diff headless 测试场景。
- **Python 离线转换与验证流水线 (`converter/`)**：
  - `converter/reader.py`：低级二进制流读取扩展（带变长数组、固定数组、对齐字符串支持）。
  - `converter/parse/commonevent.py`：550 个公共事件完整解析器（0..549，0 遗漏，0 未知 opcode）。
  - `converter/parse/database.py`：6 个核心数据库文件解析器（`Game.dat`, `DataBase.dat`, `CDataBase.dat`, `SysDatabase.dat`, `TileSetData.dat`, `MapTree.dat`）。
  - `converter/export/programs.py`：生成中间指令表 `game_data/programs.json`。
  - `converter/export/database.py`：生成规范数据库 `game_data/database.json`。
  - `converter/export/save.py`：生成存档分析与导出 `reports/save_import.json`。
  - `converter/export/manifest.py`：生成全资源哈希追踪 `game_data/manifest.json`。
  - `converter/validate.py`：对账基准 `docs/BASELINE.json`（69/69 项全部通过）。
- **Oracle Trace 对拍工具链 (`oracle/`)**：
  - `oracle/TRACE_CONTRACT.md`：冻结 trace 外层 schema 与 step/effects 契约。
  - `oracle/diff_trace.py`：Python trace 对拍对比脚本（支持容错、差异快照与 JSON 报告）。
  - `oracle/fixtures/ce42_reference_trace.json`：CE42 标题画面标准基准 trace。
  - `oracle/fixtures/ce42_corrupt_trace.json`：故意注入差异的负向测试 trace。
- **规格与规划文档 (`docs/godot/`)**：
  - `docs/godot/DATA_CONTRACT.md`：Manifest/Programs/Database/Save 规范文档。
  - `docs/godot/DECISIONS.md`：D-01 至 D-15 架构决策全纪录。
  - `docs/godot/OPCODE_TEST_PLAN.md`：高频 Top 5 opcode（`401, 210, 121, 111, 180`）测试与执行计划。
  - `tools/godot_check.sh`：自动化门禁脚本（包含版本检测、C#构建、Smoke、SaveImport、TraceDiff 命令行与场景测试 6 大门限）。

---

## 3. 证据

| 证据项 | 命令 / 路径 | 验证结果 |
|---|---|---|
| **CI / 全门限脚本** | `bash tools/godot_check.sh` | **退出码 0**，输出 `GODOT_GATE_OK`（覆盖 6 个步骤） |
| **数据对账** | `python -m converter validate` | **退出码 0**，`69/69 checks passed: 0 errors, 0 warnings` |
| **全指令无未知 Opcode** | `python -m converter programs` | 550 CE + 73 地图事件共 169,831 条真实指令，未知 opcode = 0 |
| **单元测试套件** | `pytest converter/tests` | **30 passed in 2.32s**（commonevent, mps, oracle_trace, save） |
| **8 槽零售存档导入** | `res://tests/SaveImportTest.tscn` | 8/8 存档全部通过 3-pass CRT rand PRNG 解密与 checksum 校验 |
| **Oracle Trace 对拍** | `python oracle/diff_trace.py ...` | `10/10 steps matched 100%`，负向测试精准报告 Step 6 divergence |
| **Godot 内部 Trace 校验** | `res://tests/TraceDiffTest.tscn` | `TRACE_DIFF_TEST_OK 10/10 steps verified and divergence detected correctly` |

---

## 4. 边界与未完成

### 4.1 已验证
- Godot 4.7.2 Mono + .NET 8.0.428 SDK 在 Windows 本机完全可用，headless 执行与场景退出码工作正常。
- 550 个公共事件、9 张地图（73 个地图事件，134 页）的二进制结构与指令总数（169,831）与 Rust 原型基准 100% 对齐。
- 8 槽原版 `.sav` 的变量空间（800 个普通变量、CData 覆盖）、游戏时间、已加载图片表均可稳定解析并提供结构化输出。
- `oracle/diff_trace.py` 与 Godot C# `TraceDiffTest` 双向对拍机制建立并冻结。

### 4.2 暂留项（标记 `needs-runtime`）
- **`opcode 123 InputKey` 掩码与键码**：本作中出现 218 次，因 M0 为无头数据环境，保留待 M1 交互骨架时上真机校验按键掩码。
- **`CE159/CE162` 战斗目标派发**：战斗系统深层逻辑待 M3 阶段结合实机录制样本对齐。

### 4.3 已知不可恢复边界（D-07 确认）
原版 `.sav` 中因 WOLF 闭源引擎内存动态指针与瞬态调用机制，以下字段明确不可恢复，已写入 `reports/save_import.json` 及 UI 提示文案：
- `event_instances`（未完成的并行事件运行栈）
- `pc` 与 `call_stack`（事件正在执行的字节码指针）
- `wait`（等待帧剩余计数）
- `audio_phase`（BGM/BGS 毫秒播放偏移）
- **UI 引导策略**：导入原版存档时，强制将玩家放置在对应存档的地图安全锚点（Safe Spot / 卧室），重新加载主角与环境，避免因中断的事件栈引发坏档。

---

## 5. 下一步（M1 入口）

M0 所有 8 项退出标准均已 100% 达成。进入 **M1 纵向切片（Vertical Slice）**：
1. **实现 C# EventVM 核心骨架**：
   - 寄存器管理：800 个普通变量、系统变量、每个事件独立的 Self 变量表（`Int` 与 `Str` 物理分离）。
   - 确定性 32 位运算：实现 `unchecked` 包装的 `Opcode 121` 与 MINSTD PRNG。
   - 调度器：基于 60fps 逻辑时钟驱动协程池，支持 `Opcode 180 Wait` 让步与唤醒。
2. **实现前 14 个最高频 Opcode**：
   - `401, 210, 121, 150, 101, 499, 111, 103, 180, 420, 250, 498, 179, 213`。
3. **跑通首个纵向业务切片**：
   - `标题画面 (CE42) → 新游戏 → 开场事件 → 第一天卧室 HUD → 消息/选择 → 原子存档 → 读档继续`。
   - 产出首条真实 CE42 对拍 trace 并由 `oracle/diff_trace.py` 自动化比对。
