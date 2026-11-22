# LWSMF 全量静态审查（`59268ac`）缺陷修复与门禁验证报告

**对应审查**：[full_static_audit_59268ac_2026_11_22.md](full_static_audit_59268ac_2026_11_22.md)  
**关联决策**：[D-82](../godot/DECISIONS.md#1-决策台账)  
**完成日期**：2026-11-22

---

## 1. 修复范围总览

本次收口覆盖审查报告列出的全部 **4 项 P0、24 项 P1、26 项 P2、13 项 P3**：

1. **C# 核心引擎（`godot/core_cs/`）**：
   - `LegacySaveImporter.cs`：补齐 `SaveReader.ReadBytes` 整数溢出防线、`ImportFile` `MaxSaveBytes` 与地图格数 `ulong` 乘积边界、`VariableDatabase` 字段数与零列防死循环、`picName` 路径安全检查、非法 UTF-8 步长修复、`ImportAll` 单槽异常隔离与 `.tmp` 原子写入。
   - `EventVm.cs`、`GameDatabase.cs`、`WolfRng.cs`：修正 `FilterKeyCode` 位掩码转换、阻塞式 `op123` + `op125` 自动键交付、`int.MinValue / -1` 与 `% -1` 溢出、除零保留原值、轮转步数游标、`Params`/`Blocks` 越界校验、`strCount` 钳位与空串兜底、`op213` 循环栈清理、`StopAll`/`174`/`175` 过渡态与集合清理、`WolfRng.Range` `long/ulong` 差值防溢出。
   - `GameSession.cs`、`AutoEvents.cs`、`ProgramLoader.cs`、`MapTileData.cs`、`MapRuntime.cs`、`BattleWiring.cs`：持久化 `SavedInstance.IsParallel` 并于 `RestoreInstances` 回填地图实例 `RetainAfterFinish = true`；补齐 `StartTitle`/`StartNewGameNow`/`ApplyLoadedState`/`EnterMap` 状态与队列清理、`222` 写盘容错与路径分隔符归一；保持 `AutoEvents` 实机 `!alive` 守卫与脚本分支 Rust 语义对齐；修复 `JsonDocument` 释放、`BuildTerrain` 按 `TilesetId` 匹配、`ContactBox` 负坐标向下取整并清理死字段 `_roundLoop`。

2. **存档序列化与 C# 表现桥接（`SaveCodec` / `SaveMigrator` / `godot/presentation/`）**：
   - `SaveCodec.cs` / `SaveMigrator.cs`：将 `SaveMigrator.Normalize` 移入 `SaveCodec.FromJson`（`Normalize` 后、`Validate` 前），补齐集合元素判空、$O(N)$ 协程环检测、`WaitKeyTarget`/`WaitChild`/`MapEvent.InstanceId` 与负 `tempo` 校验、`.bak` `UnauthorizedAccessException` 捕获及 `FromBytes` 体积上限。
   - `VmController.cs`、`TerrainBridge.cs`、`MediaPakReader.cs`、`GameSessionController.cs`、`AppPaths.cs`、`Lwsmf.csproj`：`StopAll` 同步清对话与图片、无等待协程时清残余应答与状态、`KeyCodeForPoll` 按 `AcceptsWolfBits` 过滤、`SetKeyStateProvider(GodotObject, StringName)` + `(long)` 传参、`FileExists` 目录限域、区间删图防 `int.MaxValue` 溢出、左键单次边沿通知与 `_wheelDelta` 复位、`TerrainBridge` 空地形语义统一、`MediaPakReader` `Dispose(bool)` 与全路径加锁、`GameSessionController` `QuickSavePath` 与 `CallableView` 存活/安全字典访问、`AppPaths` Android OS 路径分离及 `Lwsmf.csproj` 对 `Release` 同步排除 `tests/**/*.cs`。

3. **GDScript 表现层（`godot/presentation_gd/`）**：
   - `PlayOverlays.gd`、`GameRoot.gd`、`MobileDataBootstrap.gd`、`SetupWizard.gd`：支持 `FaultModal` Esc/F12 重开防止软锁、`MobileDataBootstrap` `.part` 原子写盘/字节字段存在性校验/缺包提示与系统 CJK 回退字体、`PlayOverlays`/`SetupWizard` 绑定 `UiSkin.font()`、`SetupWizard` 无双引号注入参数列表、`op161` 冻结层清理与截图复用、`save_jpg` 前建目录及队列类型防御。
   - `PictureStage.gd`、`PictureEffects.gd`、`PictureShapes.gd`、`InputRouter.gd`、`AudioManager.gd`、`AssetDB.gd`、`UiSkin.gd`、`MessageWindow.gd`、`ChoiceWindow.gd`、`MapLayer.gd`：修复 `scale_mode == 3`、快照持久化 `tone`/`effects_290` 且清零回放淡入、`dump_pictures` 读取 `current`、`FLASH` 到期恢复隐藏、负幅钳位、`InputRouter` 物理鼠标同步、`AudioManager` 分声部音量与缺文件局部返回、`AssetDB` 键前缀与 `Close()`/`miss_paths` 清理、`UiSkin` 字体句柄关闭与九宫格非负夹紧、`MessageWindow` 移除 `_draw()` 冗余同步、`ChoiceWindow`/`MapLayer`/`PictureShapes` 资源清理与缓存上限。

4. **CI、工程工具链与 Rust 测试（`.github/workflows/`、`tools/`、`crates/`）**：
   - `.github/workflows/ci.yml` & `ios-unsigned.yml`：Windows `pwsh` 下多行 `run` 只回报最后一条命令的退出码，因此每个 `run` 仅含单条命令（`lint_decisions.py`、`release_audit.py --repo-scan`、`py_compile`、`windows_pe.py --help`、`test_mobile_tools.py`、`test_hd_tools.py` 各自独立步骤，`bash -n` 显式 `shell: bash`），补全 `Debug`/`Release`/`ExportRelease` 三配置构建，接入 `test_mobile_tools.py`/`test_hd_tools.py` 与 `cargo test -p wolf-format --test tileset`；`ios-unsigned.yml` 升至 `@v5` 并通过 `env` 传 `inputs.version`。
   - `tools/` & `docs/godot/DECISIONS.md`：`package_release.sh` 复用 `pack_media.read_pak_index` 并补全 `to_win_path`；`package_android.sh` 传入 `-p:GodotTargetPlatform=android` 与自动探测 `Lolita.ttf`；`godot_check.sh` 改用 `IFS=$'\x1f'` 隔离含空格路径、预检 `DOTNET`/`PYTHON`、分离槽位 1/8 判据与保留失败日志；`release_audit.py` 仅跳合法控制码、POSIX 匹配并严格拦截 `program_asset_missing`；`visual_regression.py` 跑前清图并校验 `phase`/`save_png`/`misses`；`lint_decisions.py` 校验单调性与跳过围栏代码块并修正 `D-58`/`D-59` 顺序；`env_check.py`/`android_data.py`/`export_preset_template.py` 补全阻塞项、SDK 子目录与 `.blob`/`Lwsmf.dll` 校验。
   - `crates/wolf-format/tests/tileset.rs`：新增 2 项离线合成测试并将 4 项依赖零售数据的集成测试显式标记 `#[ignore]`。

---

## 2. 验证结果

- **C# 构建（0 errors, 0 warnings）**：`dotnet build godot/Lwsmf.csproj -c Release` / `-c Debug` / `-c ExportRelease` 全部通过。
- **Godot 无头全量门禁**：`bash tools/godot_check.sh` → `GODOT_GATE_OK elapsed=81s`（含 `ROUTE_DIFF_OK: 149655/149655`、`ROUTE_DIFF_OK: 989439/989439`、`SESSION_PARITY_TEST_OK`、`SAVE_LOAD_TEST_OK`、`GAME_ROOT_TEST_OK`、`PICTURE_STATE_TEST_OK`、`SETUP_WIZARD_TEST_OK` 等全部 38 项）。
- **Godot OpenGL3 渲染门禁**：`LWsmf_RENDER=1 bash tools/godot_check.sh` → `GODOT_GATE_OK elapsed=83s`（`PERFORMANCE_FRAME_STRICT_OK avg_ms=4.277 p95_ms=4.167 fps=233.8`）。
- **Python 工程门禁**：
  - `python tools/lint_decisions.py` → `DECISIONS_LINT_OK`
  - `python tools/release_audit.py --repo-scan` → `REPO_PRIVACY_AUDIT_OK leaks=0`
  - `python tools/mobile/test_mobile_tools.py` → `Ran 12 tests` `OK`
  - `python tools/test_hd_tools.py` → `Ran 10 tests` `OK`
- **Rust 测试**：
  - `cargo fmt --all -- --check` 通过
  - `cargo test -p wolf-format --test tileset` → `2 passed; 0 failed; 4 ignored`
  - `cargo test -p wolf-format --lib`（30 passed）、`cargo test -p wolf-vm --lib`（97 passed）、`cargo test -p wolf-save --lib`（3 passed）、`cargo test -p wolf-audio --lib`（0 passed）通过（注：本机 Windows 环境下 `wolf-game` 与 `wolf-render` 测试二进制因缺少动态链接库退出 `STATUS_DLL_INIT_FAILED (0xc0000142)`）。
