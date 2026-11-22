# LWSMF `89fc8f5` 验收审查收口与回归验证报告（2026-11-23）

**关联审查**：[acceptance_audit_89fc8f5_2026_11_23.md](acceptance_audit_89fc8f5_2026_11_23.md)  
**对照基线**：commit `89fc8f5`  
**范围**：基线审计残余项（P1-4、P2-3）、6 项新增 P1（`N-P1-1..6`）、14 项新增 P2（`N-P2-1..14`）、12 项新增 P3，以及 2 项经实测确认应保留原设计的“假阳性”条目说明（P1-6、P2-11）。

---

## 1. 修复清单

### 1.1 C# 核心与存读档（`godot/core_cs/`、`godot/presentation/`）

- **P1-4 & N-P2-1（会话重置收口）**：`GameSession.cs` 抽取共享复位助手 `ResetTransientSessionState()`，统一清理 `_pendingGameLoad` / `_pendingSystemLoads` / `ChoiceAllowsCancel` 与对话弹窗；`StartNewGameNow()` 在保留 `TitleMap` 初始化变量的同时复位 `op250` 数据库覆盖层与 `Frames`；`LoadRetailSave()` 同步复用该助手。
- **N-P1-1（`ImportRetailSaveToSlot` 数据完整性）**：保留合法负编号画面槽位（与 `SaveCodec.Validate` 的屏幕层级负数槽策略一致）；若原版 `.sav` 含 `DatabaseCells`，通过 `GameSession.LastImportNotice` 明确记录丢弃细胞数而非静默丢弃（`SaveImportTest.cs`）。
- **N-P2-2（`InferLegacyParallel`）**：纳入 `AutoEvents.GatedCallOnly`（CE242/243）及以其为父事件的调用链（`VmTest.cs`）。
- **N-P2-3 & N-P2-4（孤立 UTF-16 代理对与包大小早退）**：`SaveCodec.FromJson()` 将 `StrictUtf8.GetByteCount(json)` 移入 `try` 并捕获 `EncoderFallbackException`；`GameSession.ValidateSaveJson()` 在 `JsonDocument.Parse` 前用严格 UTF-8 统计字节数并与 `MaxSaveBytes` 比对，在 `try` 内统一捕获 `JsonException` / `ArgumentException` / `EncoderFallbackException`（`SaveLoadTest.cs`）。
- **P2-3 / P3**：
  - `Variables.cs`：`Variables.SetStr`、`Variables.SetSysStr`、`SelfVars.SetStr` 对 `null` 兜底为空串 `""`。
  - `EventVm.cs`：`op174` / `op175` 清理过渡状态（复用 `ResetTransitionState()`）；`StopInstance` 同步清理 `_autoKeysDelivered`（`VmTest.cs`）。
  - `LegacySaveImporter.cs`：多字节 UTF-8 截断判定由仅判 `0`/`1` 改为检查 UTF-8 续字节 `(b & 0xC0) != 0x80`（`SaveImportTest.cs`）。
  - `MediaPakReader.cs`：`IPakReaderStream` 新增 `ReadBytes(int)`，`GodotFileAccessStream` 直接返回 `GetBuffer`，消除 `ReadExactly` 中的二次整块堆分配。

### 1.2 GDScript 表现层与 HD 素材管线（`godot/presentation_gd/`）

- **N-P1-2 & N-P2-6（`PictureStage.gd`）**：`track_picture()` 向 `_texture_cell_size()` 传入第 4 参数 `assets.hd_scale(file)`；标题封面回退至 `SystemGraphic/TitleGraphic.png` 后改用 `resolved_file` 查询 `hd_scale()`。
- **N-P1-3（`GameRoot.gd` & `AssetDB.gd`）**：抽取 `AssetDB.default_hd_enabled()`（移动端默认 `false`、桌面端 `true`）；`_save_display_settings()` 持久化 `hd_textures`，`_load_display_settings()` 仅在 `has_section_key("display", "hd_textures")` 时覆盖。
- **N-P1-4 & N-P2-9（`AssetDB.gd`、`ChoiceWindow.gd`、`PictureShapes.gd`、`UiSkin.gd`）**：
  - HD 解码失败记录到 `_hd_failed` 并使 `hd_scale()` 回落为 `1.0`；`_has_exact_1x()` 存在精确 1x 资产时禁止 HD 裸名跨目录串图；`_image_from_hd()` 首行空路径短路。
  - 校验 HD manifest `"entries"` 必须为 `Dictionary`（拦截 `null`）并在失败分支关闭 `reader`。
  - 窗口皮肤九宫格移除 `width >= 27` 启发式，改由 `PictureShapes` / `ChoiceWindow` 显式透传 `assets.hd_scale()` 至 `UiSkin.nine_patch()`。
- **N-P2-5（`MessageWindow.gd`）**：抽取 `cursor_layout()`，按实际命中光标路径的 `assets.hd_scale()` 缩放目标矩形并保持纹理分帧区域。
- **N-P2-7 & N-P2-8 & P3（`GameRoot.gd`、`PlayOverlays.gd`、`MobileDataBootstrap.gd`）**：
  - 无配置文件或未保存宽高且非全屏时调用 `_fit_window_to_usable_area()` 夹紧窗口；`safe_area_insets()` 在 `aspect="keep"` 下扣除左右黑边。
  - `faulted` 且 `FaultModal` 隐藏时，`handle_back_request()` 重新打开 `FaultModal` 防止触屏软锁死；同步更新隐藏按钮文案。
  - 移除 `MobileDataBootstrap.gd` 3 个死字段；`PlayOverlays.gd` 新增 `drawer_closed` 信号替代误发 `backlog_closed`，为原生 `FileDialog` 继承主题。
- **离线单元测试扩展（`godot/tests_gd/HDPackTest.gd`、`GameRootTest.gd`）**：新增离线 2x `track_picture`、解码失败回退 `scale=1.0`、裸名优先级、标题回退、2x 光标分帧、九宫格缩放、`default_hd_enabled`、`fit_window_size`、`safe_area_insets` 与 `faulted` 返回键重开测试。

### 1.3 工具链、CI、门禁与文档（`.github/workflows/ci.yml`、`tools/`、`docs/`）

- **N-P1-5 & N-P2-10（`.github/workflows/ci.yml`）**：拆分原 Windows `pwsh` 多命令步骤为独立 `run:`，并将 `verify_hd_manifest.py` / `upscale_assets.py` / `release_audit.py` 纳入 `py_compile`；接入 `test_hd_tools.py` 与 `cargo test -p wolf-format --test tileset`。
- **N-P1-6 & N-P2-14（`tools/verify_hd_manifest.py`、`tools/release_audit.py`）**：`degraded` 计入失败并返回退出码 `1`，失败时输出 `HD_MANIFEST_VERIFY_FAIL`；建立小写路径映射表适配大小写敏感文件系统；跳过 `#` 注释与畸形清单；清理 `release_audit.py` 重复局部 `import`。
- **N-P2-11..13 & P3（`tools/upscale_assets.py`）**：移除本机硬编码路径，改为 `--engine-dir` / `LWsmf_REALCUGAN` / `PATH` 探测；使用 `write_text_atomic()` 原子写与 `mkdir` 提前建父目录；输出切换 `.png`/`.webp` 时清理同基名旧格式产物；移除死常量 `ART_DIRS` 并修正白电平注释。
- **P3（`tools/mobile/windows_pe.py`、`tools/package_release.sh`、`tools/godot_check.sh`）**：
  - `windows_pe.py`：移除未用 `shutil`；`parse_lang_id` 仅认 `0x` 为十六进制（裸数字按十进制）；校验重复语言项、0x0804 碰撞及 RVA 越过文件尾。新增 3 项 PE 单测（共 12 项）与 10 项 HD 工具单测（`tools/test_hd_tools.py`）。
  - `package_release.sh`：`--hd` 时 `SHA256SUMS.txt` 同步写入 `media_hd.pak`。
  - `godot_check.sh`：`[13/38]` 导入类缓存时将 `APPDATA`/`LOCALAPPDATA`/`XDG_*` 隔离至 `$ROOT_TEMP`，防止宿主 `EditorSettings` 网络更新检查或并发编辑器进程阻塞 `--import --quit`。
- **文档核对**：修正 [full_static_audit_59268ac_remediation_2026_11_22.md](full_static_audit_59268ac_remediation_2026_11_22.md) 第 27/41 行及 [DECISIONS.md](../godot/DECISIONS.md) `D-81` 中 JPG/UI 降噪参数描述。

---

## 2. 两条经验证应保留原设计的审计项说明

| 编号 | 审计建议 | 保留原设计的依据与验证 |
| :--- | :--- | :--- |
| **P1-6** | `AutoEvents.cs:116-117` 在 `LoopVisualPainters == false` 时补充 `!alive` 守卫 | 实机模式（`!ScriptedInput`）由 `GameSession.cs:310` 设 `LoopVisualPainters = true`，走 `AutoEvents.cs:109-113` 已含 `!alive` 守卫；`LoopVisualPainters == false` 专用于脚本对拍模式，必须与 [auto_events.rs](../../crates/wolf-game/src/auto_events.rs#L278) 保持上升沿一致。实测若在此分支加入 `!alive`，`35/38 M4FullRouteTrace` 在 frame 2307（`CE242`）即与 Oracle 产生分歧；保留原样后 989,439 步 100% 通过。 |
| **P2-11** | `SaveCodec.cs:378` 在 `WaitKind == Child` 时拦截 `!byId.ContainsKey(inst.WaitChild)` | 存档 UI 子协程（`CE14 -> CE131`）及旧版存档可能省略已过滤子实例，`GameSession.ClearMissingChildWaits()` 负责在读档后显式释放调用者（见 `GameSession.cs:1515-1525` 与 `SaveLoadTest.cs:519-540`）。`SaveCodec.ValidInstance` 已严格拒绝 `WaitChild <= 0 || WaitChild == Id`；保留原放行+恢复契约后 `SaveLoadTest` 173/173 全通过。 |

---

## 3. 全量门禁与单测实测记录（最终代码状态下）

- **C# 构建**：`dotnet build godot/Lwsmf.csproj -c Release` 与 `-c Debug` 均 0 警告、0 错误。
- **无头全量门禁**：`bash tools/godot_check.sh` → `GODOT_GATE_OK elapsed=176s`（含 38 步、`HD_PACK_TEST_OK`、`GAME_ROOT_TEST_OK`、`SAVE_LOAD_TEST_OK`、`SAVE_IMPORT_OK`、`ROUTE_DIFF_OK: 149655/149655`、`ROUTE_DIFF_OK: 989439/989439`、`M13 slot 1/8`、`PERFORMANCE_FRAME_OK`）。
- **OpenGL3 渲染回归门禁**：`LWsmf_RENDER=1 bash tools/godot_check.sh` → `GODOT_GATE_OK elapsed=180s`（`DIALOG_DRAW_TEST_OK (windowed)`、`PICTURE_ROUNDTRIP_TEST_OK (windowed)`、`PERFORMANCE_FRAME_STRICT_OK avg_ms=17.396`）。
- **Python 与工具链单测**：
  - `python tools/mobile/test_mobile_tools.py` → `Ran 12 tests` `OK`
  - `python tools/test_hd_tools.py` → `Ran 10 tests` `OK`
  - `python tools/lint_decisions.py` → `DECISIONS_LINT_OK`
  - `python tools/release_audit.py --repo-scan` → `REPO_PRIVACY_AUDIT_OK leaks=0`
- **Rust 离线测试**：`cargo test -p wolf-format --test tileset` → `2 passed; 0 failed; 4 ignored`。
