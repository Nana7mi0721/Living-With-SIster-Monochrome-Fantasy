# M5 收口与发布交接

> 按 [`HANDOFF_TEMPLATE.md`](HANDOFF_TEMPLATE.md) 结构填写；记录 M5 实际结果。

## 1. 状态

- 阶段/日期：M5 收口与发布 / 2026-11-18（已完成；R12 修复 2026-11-19）
- 仓库 HEAD：`d0dcbae`（R12 修复；M5 发布包基于 `e954ecc`，2026-11-19 发布策略调整，未重新打包）
- 发布策略：2026-11-19 起发布包自带 `game_data/` 与 `work/all/`（详见 RELEASE.md）；下方 2026-11-18 的旧包数字为历史证据。
- Godot 版本/渲染器：Godot 4.7.2 stable Mono（.NET 8.0），Windows x64；运行使用 Compatibility / OpenGL 3.3
- 数据根/夹具：`work/all`（零售 BasicData 副本）、`game_data/`；`work/retail_backup/Save` 为可选 M13 零售存档回归
- 是否可复现：是；命令见 §3

## 2. 本批交付

- 代码/工程：
  - `godot/Lwsmf.csproj`：`ExportRelease` 配置排除 `tests/**/*.cs`，发布程序集不含测试类型；Debug 仍保留测试供门禁。
  - `godot/export_presets.cfg`：Windows Desktop / x86_64；`custom_template/release` 留空以使用已安装的 `4.7.2.stable.mono` 模板；`dotnet/embed_build_outputs=false`，发布时必须携带 `game/data_Lwsmf_windows_x86_64/` 运行时目录。
  - `tools/package_release.sh` 在导出前临时生成 `godot/Lwsmf.sln` 并在退出时删除，仓库不保留 solution 文件。
  - `godot/presentation_gd/SetupWizard.gd`：首次运行数据检查、零售目录选择、后台转换（Windows 优先 `converter/convert.bat`，重试前清理状态文件）、完成后重载；`GameRoot.gd` 在 `phase="setup"` 时暂停正常游戏流程。
  - `godot/presentation/AppPaths.cs`、`godot/presentation_gd/AppPaths.gd`：统一编辑器/导出包根目录解析；导出构建按 exe 所在目录定位 `game_data`，修复导出后 `programs.json`/`database.json`/`map_tiles.json` 解析失败。
  - `godot/core_cs/SaveCodec.cs`、`GameSession.cs`、`GameSessionController.cs`、`presentation_gd/GameRoot.gd`：主档损坏回退 `.bak`、读档失败保留 live bedroom、`recover_to_last_save()` 重建会话且不写新档。
  - `godot/core_cs/TraceRecorderHost.cs`：新增 `DatabaseProvider` 委托，供 soak 的 database/battle wiring 使用。
  - `godot/tests/SleepSoakTest.cs`：day83 剧情战接入 `BattleWiring` 与 `GameDatabase`，修复 90 天 soak 的假失败。
  - `tools/package_release.sh`：构建、Godot `.NET` release 导出、staging `game_data/` 与 `work/all/`（不含 `save/`）、README 生成、发布审计。
  - `tools/release_audit.py`：自包含发布树完整性审计（程序、game_data、manifest assets）；`tools/copyright_audit.py` 保留为兼容包装。
- 转换器/schema：
  - `converter/cli.py` 增加 `stage all`：manifest → programs → database → map-tiles → validate，严格短路。
  - `converter/convert.bat`：玩家入口，自动定位仓库根并调用 `python -m converter all`。
  - `converter/tests/test_cli_all.py`：8 个 all-stage 测试。
- C# 核心：见上；无新增运行时依赖。
- GDScript/UI：`SetupWizard.gd`、`GameRoot.gd` setup 阶段分支。
- 测试/截图：
  - `godot/tests/PerformanceProbeTest.cs`（core 性能探针）
  - `godot/tests_gd/PerformanceFrameProbe.gd`（帧探针，headless / windowed strict）
  - `godot/tests_gd/SetupWizardTest.gd`、`godot/tests_gd/SetupWizardTest.tscn`
  - `godot/tests_gd/GameRootTest.gd`、`godot/tests/SaveLoadTest.cs` 扩展

## 3. 证据

| 证据 | 命令/路径 | 结果 |
| --- | --- | --- |
| 转换器单元测试 | `python -m pytest converter/tests -q` | `38 passed, 54 subtests passed` |
| 真实转换 | `python -m converter all --root work/all --out work/out/m5_convert_all --reports reports` | `CONVERT_ALL_OK`；manifest files=18 assets=276；programs bytes=52,256,568 common_events=550 map_events=134 maps=9 slots=201,438 blank_slots=31,607 real_instructions=169,831 distinct_opcodes=50；database bytes=1,725,160；map-tiles bytes=564,948 tilesets=6 maps=9；validate 69/69 |
| 存档回退 | `godot/tests/SaveLoadTest.tscn` | `SAVE_LOAD_TEST_OK 107/107` |
| 安装引导 | `godot/tests_gd/SetupWizardTest.tscn`、`GameRootTest.tscn` | `SETUP_WIZARD_TEST_OK`、`GAME_ROOT_TEST_OK` |
| 性能 core | `godot/tests/PerformanceProbeTest.tscn` | `PERFORMANCE_CORE_OK startup_ms=754.790 frames=600 avg_ms=0.1257 p50=0.0713 p95=0.1213 max=12.6358 gc_mb=51.421 static_mb=24.179` |
| 性能帧（headless） | `godot/tests_gd/PerformanceFrameProbe.tscn` | `PERFORMANCE_FRAME_OK frames=300 avg_ms=6.924 p95_ms=6.906 max_ms=12.418 fps=144.4`（headless 主循环地板） |
| 性能帧（strict 窗口） | `LWsmf_PERF_STRICT=1 godot --path godot --rendering-driver opengl3 res://tests_gd/PerformanceFrameProbe.tscn` | `PERFORMANCE_FRAME_STRICT_OK avg_ms=4.166 p95=4.167 fps=240.0`（RTX 5060 Laptop，OpenGL 3.3） |
| 90 天 soak | `LWsmf_SOAK_DAYS=90 godot --headless --path godot res://tests/SleepSoakTest.tscn` | `SLEEP_SOAK_TEST_OK days=90 checks=181`；day83→100 剧情跳天按合法时钟校验 |
| 发布打包（2026-11-18 旧无素材包证据） | `bash tools/package_release.sh` | `EXPORT_PRESET_OK`、`COPYRIGHT_AUDIT_OK files=206 bytes=189666077 forbidden=0`、`RELEASE_PACKAGE_OK`；`Lwsmf.exe`=109,372,928 B、`Lwsmf.pck`=84,588 B、`data_Lwsmf_windows_x86_64/`=186 files（含 `Lwsmf.dll`=237,056 B）；详见 [`RELEASE.md`](RELEASE.md) |
| 发布审计（2026-11-19 起） | `python tools/release_audit.py --dist dist/Lwsmf-win64` | 校验必需程序、`game_data/`、`work/all/` 与 manifest 全部 assets；旧版权审计夹具随 D-53 退役，新包数字待重新打包后记录 |
| 门禁 | `bash tools/godot_check.sh`；`LWsmf_RENDER=1 bash tools/godot_check.sh` | 双跑均 `GODOT_GATE_OK` 38/38；headless 30 天 `SLEEP_SOAK_TEST_OK days=30 checks=61`，headless 帧 `PERFORMANCE_FRAME_OK avg_ms=6.895 p95_ms=6.944 fps=145.0`；窗口 strict `PERFORMANCE_FRAME_STRICT_OK avg_ms=4.205 p95_ms=4.545 fps=237.8` |

## 4. 边界与未完成

- 已验证：
  - 转换器 `all` 在真实 `work/all` 上完成并生成可校验的 `game_data`。
  - 安装引导、存档回退、性能探针、90 天 soak 均有 headless 可复现证据。
  - 2026-11-19 起发布脚本改为自带 `game_data/` 与 `work/all/`，并使用 `release_audit.py`；旧 clean/bad/missing 版权审计夹具已退役。
- 未验证/需要补充材料：
  - 无；最终 headless 与 `LWsmf_RENDER=1` 双跑均通过 38/38（30 天默认 soak 已包含在门禁中）。`[2/38] build-solutions` 的 .NET SDK 探测提示不影响实际 `dotnet build`。
- 已知差异（原版行为 / Godot 缺陷 / 决策变更）：
  - 性能帧探针的 headless 结果受 Godot 主循环约 6.9 ms 地板限制，不代表渲染吞吐；窗口 strict 结果才代表 60fps 以上的实际渲染余量。
  - `dotnet/embed_build_outputs=true` 的导出曾在运行时报 `Failed to open .dotnet-publish-manifest` / `Failed to load hostfxr`；最终改为 `false` 并交付运行时目录，见 `RELEASE.md` §3/§9。
  - 发布程序集已剔除 C# 测试类型：`Lwsmf.dll`=237,056 B，测试符号扫描为 0；pck 不含 `res://tests/`。
  - 发布包（2026-11-19 起）自带 `game_data/` 与 `work/all/`，正常游玩不需要 Python；仅在数据缺失或重新转换时需要 `converter\convert.bat` 与 Python 3.10+。
  - SetupWizard 的转换启动依赖 `converter/convert.bat` 与 Python；发布 README 已写入此前提。
  - 90 天 soak 使用测试夹具战斗 wiring，不是人工交互；day83→100 剧情跳天是内容数据既有路径，不是测试遗漏。

## 5. 下一步

1. 发布包分发时确认 `dist/Lwsmf-win64/game/data_Lwsmf_windows_x86_64/` 与 exe 同目录；确认 `game_data/` 与 `work/all/media.pak`（及 `BasicData/*.project`）已随包，发布 README 已写明正常游玩无需 Python。
2. 发版收尾打包与验证见 [`RELEASE.md`](RELEASE.md) 与 [`PRE_RELEASE_ORCHESTRATOR_AUDIT.md`](PRE_RELEASE_ORCHESTRATOR_AUDIT.md)，文档统一以 [docs/README.md](../README.md) 为全局索引。
