# Godot Debug 修复、存档页可点击翻页箭头与全项目审查闭环报告（2026-11-25）

> 日期：2026-11-25（Asia/Shanghai）
> 基准与决策：`D-92`、`D-93`（见 [../godot/DECISIONS.md](../godot/DECISIONS.md)）
> 审查覆盖：Godot 运行时（C# Core + Presentation Bridge + GDScript UI）、发行构建与打包管线（Windows / Android / iOS）、离线转换器与 CI、仓库隐私与合规基线

---

## 1. 本轮解决的用户核心需求

### 1.1 Godot Debug 模式无法运行（灰白 `(DEBUG)` 窗口卡死）

- **现象与根因**：
  1. [godot/presentation_gd/GameRoot.gd](../../godot/presentation_gd/GameRoot.gd) 中 `_window_icons` 原先未声明为 `WindowIcons` 类型（仅 `Variant`），在 Godot 4.7.2 Debug/编辑器脚本解析阶段对 `var ok := _window_icons.Apply(...)` 触发致命 `Parse Error: Cannot infer the type of "ok"`，导致主场景脚本加载失败并停留在灰白 `(DEBUG)` 窗口。
  2. [godot/presentation_gd/PlayOverlays.gd](../../godot/presentation_gd/PlayOverlays.gd) 中存档条目名称校验曾直接检查字面转义 `"\u0000"`，触发 Godot 4.7 `Unicode parsing error: Unexpected NUL character`。
  3. 同步排查出的关联启动链路隐患：[godot/presentation_gd/BootSplashOverlay.gd](../../godot/presentation_gd/BootSplashOverlay.gd) 曾误调用不存在的 `reader.ReadBytes(...)`（实际契约是 [MediaPakReader.cs](../../godot/presentation/MediaPakReader.cs) 暴露的 `Read(name)`，返回 `Variant` / `PackedByteArray`），在移动包从 `media.pak` 读取开机双层揭示图时会抛出调用异常。
- **修复**：
  - `GameRoot.gd` 显式声明 `var _window_icons: WindowIcons` 与 `var ok: bool = _window_icons.Apply(...)`。
  - `PlayOverlays.gd` 改用 `entry_name.to_utf8_buffer().has(0)` 检测 NUL 字节。
  - `BootSplashOverlay.gd` 改用 `reader.Read(...)` 并校验 `is PackedByteArray`，同时在 [GameRootTest.gd](../../godot/tests_gd/GameRootTest.gd) 新增 `MediaPakReader.Read` 与缺失包容错回归断言。
- **验证**：`--check-only` 全量 GDScript 脚本通过；`dotnet build godot/Lwsmf.csproj -c Debug` 0 警告 0 错误；实时 GUI Debug 启动探针 cleanly 加载 `database.json` 与 `新タイトル用イベント`。

### 1.2 存档/读档页面可点击翻页箭头（`12004` / `12005`）

- **机制核对（`CE131`）**：原版事件在存档页通过图片槽 `12004`（`<TRI-LEFT>`）和 `12005`（`<TRI-RIGHT>`）绘制分页三角形，并在每轮循环 9–10 帧等待后进入 `[58] op=123 mask=771` 轮询鼠标滚轮增量（正值上翻页、负值下翻页）。
- **实现架构**：
  1. **脉冲驱动（12 帧 TTL + 消费即清）**：[InputRouter.gd](../../godot/presentation_gd/InputRouter.gd) 新增 `trigger_wheel_pulse(page_direction, ttl_frames = WHEEL_PULSE_TTL_FRAMES)`（`WHEEL_PULSE_TTL_FRAMES = 12`），只向 `wheel_notch()` 注入对应方向（上一页 `+1` / 下一页 `-1`）而不污染 `_wheel_bits`；[VmController.cs](../../godot/presentation/VmController.cs) 记录 `_wheelConsumedThisFrame`（在 `KeyCodeForPoll` 命中 `mask == 771 && wheelNotch != 0` 时置位）并提供 `ConsumeWheelObserved()`，一旦被 `CE131` 消费立即清空剩余 TTL 防止一次点击连翻两页。
  2. **视觉保真与触控命中**：[PictureStage.gd](../../godot/presentation_gd/PictureStage.gd) 在加载 `12004`/`12005` `<TRI-*>` 时挂载 `save_page_arrow = true` 元数据；`_render_picture()` 为其强制 `blend = 0`（普通 Alpha 混合），消除原版 `blend=1 (ADD)` 在浅色标题底图上严重泛白问题；悬停时应用 `1.15x` 放大与 `1.22x` 亮度高亮，[GameRoot.gd](../../godot/presentation_gd/GameRoot.gd) 在桌面端同步 `CURSOR_POINTING_HAND` / `CURSOR_ARROW`。触控输入使用最小 `44×64` 逻辑命中热区。
  3. **输入边界隔离**：在 `CE131` 活跃态下点击槽位外箭头热区时拦截点击与 `click_latched` / `cancel_pressed`，防止点击箭头误关闭存档页；鼠标悬停更新同步覆盖标题页与地图态进入的存档页。
- **验证**：[InputRouterTest.gd](../../godot/tests_gd/InputRouterTest.gd) 与 [GameRootTest.gd](../../godot/tests_gd/GameRootTest.gd) 覆盖鼠标点击、触控放大命中框、`0..7` 页边界不越界、`material == null` 与悬停缩放。

---

## 2. 全项目审计发现与收口清单

| 类别 | 模块 / 文件 | 发现问题 | 处理结果 |
| :--- | :--- | :--- | :--- |
| **C# 运行时与桥接** | [WindowIcons.cs](../../godot/presentation/WindowIcons.cs)<br>[GameRoot.gd](../../godot/presentation_gd/GameRoot.gd) | 缺少 `IsWindow(HWND)`、空/零长 ICO 载荷拦截、`uint` ICO 目录项 `checked` 溢出风险，以及退出时 `HICON` 句柄释放 | 增加参数与 `IsWindow` 校验、无符号安全边界比较、`Release()` + `Dispose(bool)` 释放句柄，并在 `GameRoot._exit_tree()` 调用释放 |
| **C# 运行时与桥接** | [MapRuntime.cs](../../godot/core_cs/MapRuntime.cs) | `EventOccupies()` 与 `CountBy()` 未拦截负数 `state.Active` 索引，异常存档可触发越界访问 | 增加 `index >= 0` 前置校验并在 [MapRuntimeTest.cs](../../godot/tests/MapRuntimeTest.cs) 补充回归测试 |
| **C# 运行时与桥接** | [SaveCodec.cs](../../godot/core_cs/SaveCodec.cs)<br>[LegacySaveExporter.cs](../../godot/core_cs/LegacySaveExporter.cs)<br>[GameDatabase.cs](../../godot/core_cs/GameDatabase.cs) | `PlayFrames > int.MaxValue` 强转、`IsSafeAssetPath` 未拒 `\0`、畸形 `DbOverlays` 行/列/类型号无上限（可导致 `.sav` 导出分配超大数组）、`FirstEmptyRow` 与 `FirstEmptyRowWithOverlay` 逻辑重复且 `fieldCount == 0` 仍索引 `_tables` | 在 `SaveCodec.Validate` 增加 `PlayFrames` 上限、`\0` 检查与 `DbOverlays` 维度上限（类型/列 `≤ 4096`，行 `≤ 100,000`）；`WriteVarDb` 同步过滤；提取 `GameDatabase.FirstEmptyRowCore` 并以 `fieldCount <= 0` 安全早退（补充测试于 [SaveLoadTest.cs](../../godot/tests/SaveLoadTest.cs)） |
| **C# 运行时与桥接** | [GameSession.cs](../../godot/core_cs/GameSession.cs)<br>[VmController.cs](../../godot/presentation/VmController.cs)<br>[VmTest.cs](../../godot/tests/VmTest.cs) | `LoadRetailSave()` 仅恢复数字格 `DatabaseCells`，遗漏文本格 `DatabaseTextCells`；`FileExists("save/sub/x")` 错误回退匹配根目录同名文件 | 在 `IVmSession` / `GameDatabase` / `VmController` 增加 `ApplySavedTextCells` 并接入 `LoadRetailSave`；收窄 `ExistsInSaveDirectoryCaseInsensitive` 仅用于两段路径 `save/<file>` |
| **GDScript 与 UI** | [PlayOverlays.gd](../../godot/presentation_gd/PlayOverlays.gd) | Android `content://` 导入经 `stage_uri_to_temp_file` 改名后丢失原始文件名，导致 `System.sav`/`System.json` 分流失效；`close_topmost_modal()` 将 `_backlog_open` 排在更高层 `_drawer_open` 之前 | 保留 URL 解码后的 `_pending_orig_leaf`；调整 `close_topmost_modal()` 优先关闭抽屉 |
| **GDScript 与 UI** | [GameRoot.gd](../../godot/presentation_gd/GameRoot.gd)<br>[TextTokenizer.gd](../../godot/presentation_gd/TextTokenizer.gd) | 模态窗口打开时 `F9`/`F12` 仍可穿透开启回顾/回标题；最小化时保存误窗口尺寸；`_center_window(size)` 参数遮蔽与 `TextTokenizer` 未用局部变量 | 统一用 `_modal_window_open()` 拦截穿透（允许仅回顾打开时按 `F9` 关闭）；仅在 `WINDOW_MODE_WINDOWED` 下持久化窗口尺寸；消除变量遮蔽与冗余局部变量 |
| **打包、导出与 CI** | [godot/export_presets.cfg](../../godot/export_presets.cfg)<br>[sync_release_icons.py](../../tools/sync_release_icons.py) | Windows/Android/iOS 三端未互相排除他端生成图标；Android 432×432 自适应前景满帧裁切未留安全区 | 三端预设按需精准过滤（Windows `.pck` 仅携 `.bin`）；`sync_release_icons.py` 改为 62% 安全区居中透明画布 |
| **打包、导出与 CI** | [package_release.sh](../../tools/package_release.sh)<br>[package_android.sh](../../tools/package_android.sh)<br>[package_ios.sh](../../tools/package_ios.sh)<br>[custom-templates.yml](../../.github/workflows/custom-templates.yml)<br>[ci.yml](../../.github/workflows/ci.yml) | 无 `_publish` 目录时缺少可控 `--allow-placeholder`；`cleanup()` 可能误删外部预生成的 `generated/`；`SLN_CREATED` 标志滞后；Windows 发布包缺少 `docs/BASELINE.json`；Android/Windows 导出前缺少 `--import`；iOS 部分注入参数缺校验与 `Lolita.ttf` 自动探测；`custom-templates.yml` 缺占位图标与有效退出校验 | 逐项补齐标志、`trap` 与 `cleanup()` 护栏、`--import --quit`、`$STAGE/docs/BASELINE.json` 与严格退出码校验，并补全 `ci.yml` 的 Python/Bash 检查清单 |
| **数据转换与移动校验** | [converter/cli.py](../../converter/cli.py)<br>[converter/tests/test_cli_all.py](../../converter/tests/test_cli_all.py)<br>[android_data.py](../../tools/mobile/android_data.py)<br>[test_mobile_tools.py](../../tools/mobile/test_mobile_tools.py) | `ALL_STAGES` 先执行 `manifest` 再产出 `programs/database/map-tiles`，且未哈希 `map_tiles.json`；`FORBIDDEN_APK_PREFIXES` 错用 `res://tests/` 而未命中 APK 内实际前缀 `assets/tests/` | 将 `manifest` 调整至产物生成之后、补充 `map_tiles` 校验摘要并同步单测；修正 APK 禁运前缀为 `assets/tests/` / `assets/tests_gd/` 并新增单测断言；对齐 `release_audit.py` 必检 `docs/BASELINE.json` 与 PCK `mobile_data/` 拦截 |
| **仓库合规与隐私** | [Cargo.toml](../../Cargo.toml)<br>`crates/*/Cargo.toml`<br>`crates/wolf-{cli,format,render,save}`<br>[release_audit.py](../../tools/release_audit.py)<br>`docs/` / `README*.md` | Rust 清单遗留违背 `AGENTS.md` §2 的 `MIT OR Apache-2.0` 声明与旧仓库别名；多处测试/文档硬编码本机用户名与绝对路径；`release_audit.py --repo-scan` 漏扫 Rust/Converter/Tests/Docs；代码注释含少许过时轮次标签 | 移除开源许可证声明，所有 Rust/文档路径改为相对路径或环境变量；扩展 `--repo-scan` 覆盖范围并清零泄露；按“三不写、两保留”规范整理注释并同步 `README*.md`、`DECISIONS.md` 与发行文档 |

---

## 3. 验证与门禁实测结果

以下检查均在最终代码与文档修改完成后实测通过：

| 检查项 | 命令 | 实测结果 |
| :--- | :--- | :--- |
| **.NET 8 三配置编译** | `dotnet build godot/Lwsmf.csproj -c Release / Debug / ExportRelease -v:minimal` | 均 `0 warnings, 0 errors` |
| **GDScript 全量解析** | `godot --headless --path godot --check-only --script ...`（覆盖 `presentation_gd/*.gd` 与 `tests_gd/*.gd`） | `ALL_GDSCRIPT_CHECK_ONLY_OK` |
| **实时 GUI Debug 启动** | `timeout 7s "$GODOT" --path godot --verbose` | `LIVE_GUI_DEBUG_PROBE_OK`（干净加载 `database.json` 与 `新タイトル用イベント`） |
| **无头 38 项交付门禁** | `bash tools/godot_check.sh` | **38/38 全通过**（含 `SAVE_LOAD_TEST_OK 203/203`、`MAP_RUNTIME_TEST_OK`、`INPUT_ROUTER_TEST_OK`、`GAME_ROOT_TEST_OK`、`RouteTrace` `149655/149655`、`M4FullRouteTrace` `989439/989439`、`M13 slot 1 & 8`、`GODOT_GATE_OK`） |
| **OpenGL3 渲染回归门禁** | `LWsmf_RENDER=1 bash tools/godot_check.sh` | **全通过**（含窗口绘制校验及 `PERFORMANCE_FRAME_STRICT_OK avg_ms=16.818 p95_ms=16.667 fps=59.5`，`GODOT_GATE_OK elapsed=107s`） |
| **决策与隐私审计** | `python tools/lint_decisions.py && python tools/release_audit.py --repo-scan` | `DECISIONS_LINT_OK`，`REPO_PRIVACY_AUDIT_OK leaks=0` |
| **转换器与移动工具单测** | `python -m unittest discover -s converter/tests`；`python tools/mobile/test_mobile_tools.py` | 转换器 35/35 `OK`；移动/发布工具 26/26 `OK` |
| **脚本语法与字节码检查** | `python -m py_compile ...` 与 `bash -n ...` | 全通过 |
| **Rust 格式与核心测试** | `cargo fmt --all -- --check`；`cargo test -p wolf-format -p wolf-vm -p wolf-save --lib`；`cargo test -p wolf-save`（25 passed）；`cargo test -p wolf-format --test tileset --test dxarchive` | 全部通过（注：`cargo test` 在未设环境变量时可直接识别 `work/retail_backup` 存档；全 workspace `clippy` 因本机 `zerocopy`/`libc`/`num-traits` 构建脚本调用 `rustc` 触发 `0xc0000142` 未运行全量，核心 `wolf-format`/`wolf-vm`/`wolf-save` 已通过） |
