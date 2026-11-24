# LWSMF Remake（`8e0a98d1`）三端深度审查与架构级修复闭环报告

- **对应审查台账**：[deep_audit_8e0a98d1_and_arch_fix_plan_2026_11_24.md](deep_audit_8e0a98d1_and_arch_fix_plan_2026_11_24.md)
- **决策归档**：[docs/godot/DECISIONS.md](../godot/DECISIONS.md)（`D-88`、`D-89`、`D-90`）
- **闭环日期**：2026-11-24
- **实施提交链**：
  - 核心 C# 批（P0-4、P0-5、P1-7..P1-11、P2-1/P2-2/P3-1 核心）：`0d183e6`
  - 移动端触控与表现层批（P0-1..P0-3、专项 1、P1-1..P1-6、P2-3、P3-1 UI）：`f4f6053`
  - 原版 `.sav` 双向导出与智能导入批（专项 2）：`c32cbfa`
  - 三端启动画面替换与流式打包批（专项 3、P2-2 工具链）：`25a46c8`

---

## 一、P0 阻断项闭环明细（5 / 5 已解决）

| 编号 | 关键改动文件 | 落地结论 | 回归覆盖 |
| :--- | :--- | :--- | :--- |
| **P0-1** | `GameSession.cs`, `GameSessionController.cs`, `PictureStage.gd`, `GameRoot.gd` | 新增 `GetForegroundStackSignature()`（仅非并行前台 `EventId` 链）与 `foreground_slot_signature()`；移除音效/特效标脏，2 帧实证窗口精准判定空白双击取消 | `GameRootTest.gd`, `InputRouterTest.gd` |
| **P0-2** | `InputRouter.gd`, `VmController.cs`, `GameRoot.gd` | 引入可消耗 3 帧触控脉冲 TTL（`_touch_ok_ttl`/`_touch_cancel_ttl`），配合 `VmController` 右键 2 帧 TTL 与 `ConsumeOkObserved()`/`ConsumeCancelObserved()` 消费清零；桌面鼠标维持 1 帧边沿语义 | `InputRouterTest.gd`, `GameRootTest.gd` |
| **P0-3** | `PlayOverlays.gd`, `InputRouter.gd`, `GameRoot.gd` | `PlayOverlays.consumes_touch_point()` 覆盖全部模态框、抽屉按钮与存档快捷条；`InputRouter` 触控与拖拽入口提前拦截并清手势态 | `GameRootTest.gd`, `InputRouterTest.gd` |
| **P0-4** | `LwsmfJsonContext.cs`, `Lwsmf.csproj`, `GameDatabase.cs`, `SaveCodec.cs`, `LegacySaveImporter.cs`, `GameSessionController.cs` 等 | 全面切换至 `System.Text.Json` 源生成上下文，开启 `JsonSerializerIsReflectionEnabledByDefault=false` 与 `IsAotCompatible=true` | `dotnet build -c Release`（0 警告/0 错误） |
| **P0-5** | `GameDatabase.cs`, `SaveCodec.cs`, `SaveMigrator.cs`, `GameSession.cs`, `VmController.cs` | `SaveState` 升级 `schema_version = 2` 持久化稀疏 `db_overlays`（v1 自动迁空）；读档先清旧覆写再恢复；`ReadSaveAddress` CData 查询目标存档覆写；零售 `.sav` 导入完整保留 `DatabaseCells`/`DatabaseTextCells` | `SaveLoadTest.cs`, `SaveImportTest.cs` |

---

## 二、P1 / P2 / P3 三端隐患闭环明细（12 / 12 已解决）

| 编号 | 关键改动文件 | 落地结论 |
| :--- | :--- | :--- |
| **P1-1** | `InputRouter.gd`, `InputRouterTest.gd` | 快捷抽屉开启手势由左缘右滑改为**双指下滑**（双指上滑/单指左滑关闭），彻底避免与 Android 全面屏返回手势冲突 |
| **P1-2** | `AudioManager.gd`, `GameRoot.gd`, `AudioManagerTest.gd` | `NOTIFICATION_APPLICATION_PAUSED`/`RESUMED` 统一暂停/恢复 BGM/BGS 并静音主总线 |
| **P1-3** | `GameRoot.gd` | 每 `0.5s` 低频轮询及 `RESUMED` 时对比 `DisplayServer.get_display_safe_area()`，180° 翻转即时重排刘海边距 |
| **P1-4** | `ChoiceWindow.gd`, `GameRoot.gd`, `DialogDrawTest.gd` | 触控模式选项行高拓宽至 `36px`；点击命中与空白区统一以 `row_rects()` 为准并在死区/`action_denied` 清 `_click_latch`；`_on_cancel`/`handle_back_request` 先检查 `allow_cancel` |
| **P1-5** | `AssetDB.gd`, `AudioManager.gd`, `AudioManagerTest.gd` | 资源加载默认置 `loop=false`，SE 声部池直接共享缓存流零拷贝播放 |
| **P1-6** | `GameRoot.gd`, `SetupWizard.gd` | 封装 `is_desktop_platform()` 守卫窗口尺寸/全屏逻辑；移动端资源缺失展示轻量错误页并禁止 `OS.create_process`/`OS.shell_open` |
| **P1-7** | `KeyCodes.cs`, `EventVm.cs`, `VmTest.cs` | `ApplyInputPermission` 经 `KeyCodes.FromWolfBits` 映射 WOLF 方向位并补 `0x08 -> Pause` |
| **P1-8** | `Magic.cs`, `EventVm.cs`, `VmTest.cs` | 新增 `Magic.IsStringSlot`/`SysStrOffset`，`op250` 严格按槽位类型写空串，允许清空文本单元格 |
| **P1-9** | `IHost.cs`, `EventVm.cs`, `GameSession.cs`, `SaveLoadTest.cs` | `op221/222` 统一复用 `Magic.Decode`，完整支持字符串槽、`SelfVar` 与 `SysVar > 12` |
| **P1-10** | `AutoEvents.cs`, `AutoEventsTest.cs` | `PrimeLatched` 重绑已恢复并行实例，条件转假时停止孤儿并行实例，严格遵守 `D-33`/`D-62`/`D-82` 红线 |
| **P1-11** | `GameSession.cs` | 提取 `ResolveMapDefinition()` 统一按 `MapDefinition.Id` 同图传送去重，`ApplyPending` 待读档时清空传送队列并短路 |
| **P2-1** | `MediaPakReader.cs`, `MapTileData.cs` | `GodotFileAccessStream` 单次 `GetBuffer` 消除 64 KiB 循环拷贝；`MapTileData.Cache` 加锁并规范化路径键 |
| **P2-2** | `export_presets.cfg`, `ios_inject_data.py`, `package_release.sh`, `VmController.cs` | iOS 开启 `entitlements/increased_memory_limit=true`；`ios_inject_data.py` 改为 1 MiB 流式拷贝与分块 SHA-256；`package_release.sh` 判空 `.pck`；`VmController.FileExists` 支持大小写不敏感与 `MediaPakReader.Has()` 回退 |
| **P2-3** | `PictureStage.gd`, `PictureShapes.gd`, `UiSkin.gd`, `MessageWindow.gd`, `DialogDrawTest.gd` | `_state_animating()` 判别完成态 `zoom`/`pattern`；`<CIRCLE>`/`<TRI-*>` 改为矢量自绘；`UiSkin` 静态单例缓存字体；`MessageWindow` 支撑 `\f[28+]` 气泡真实高度且点击快进停于下一 `\!` 停顿点 |
| **P3-1** | `EventVm.cs`, `PlayOverlays.gd` | `LastUnitStart` 改 `ReadOnlySpan<char>` 零分配、`case 210` 复用 `DecodeCallArguments`；`FaultModal` 改 2×2 `GridContainer` 并按触控态切换提示文案 |

---

## 三、三项专项优化落地结果（3 / 3 已解决）

1. **专项 1（移动端「菜单」按钮精确定位）**：
   - `PlayOverlays.gd` 将按钮可见性升级为 `_drawer_tab_context`（`"hidden"` / `"title"` / `"save"`）；
   - `GameRoot.gd` 按**存档优先于标题**传参（解决标题点「继续」进入存档页时双真冲突）；
   - `"title"` 位于 `y = 36..70`（完整避开 `y = 0..28` 黑色标题栏），`"save"` 位于 `y = 17..51`（严格居中于 `y = 0..68` 存档顶栏），套用统一黑白极简边框并纳入触控防穿透。
2. **专项 2（原生 `.sav` 增强导入与默认 `.sav` 导出）**：
   - 新增 [LegacySaveExporter.cs](../../godot/core_cs/LegacySaveExporter.cs)，字节级对齐 `crates/wolf-save`（`0x008C`、UTF-8 `0x55`、Part1–7、`var6=550`、`EncodePictureText`、`VariableDatabase` 覆写编码与 LCG 加密）；
   - `LegacySaveImporter.cs` 补齐 `raw[6] != 0x55` Shift-JIS (`cp932`) 解码与 `ValidateFile`；`GameSession.cs` / `GameSessionController.cs` 补齐单槽/系统/批量导出、`ImportRetailSystemSave` 与 `ImportRetailSaveToSlot` 同步写入 `System.json` 槽位标记（`Var[0]/546/626`）；
   - `PlayOverlays.gd` 将默认导出切换为原版 `.sav`（单槽 + 打包 ZIP，同时保留重制版 JSON ZIP 备份），支持单文件/ZIP 文件头自动分流及原生系统文件对话框；
   - `SaveImportTest.cs` 新增往返单测，并在 `crates/wolf-save/tests/rebuild.rs` 通过 Rust `wolf-save` 字节级解析与重构交叉校验。
3. **专项 3（三端启动画面四层无缝替换）**：
   - 建立 `godot/assets/boot/` 纯黑占位单源与 [sync_boot_assets.py](../../tools/sync_boot_assets.py)（自动派生 Android `192×192` / `432×432` 图标，含无 Pillow 纯标准库 PNG 回退）；
   - 配置 `godot/project.godot` `boot_splash/*`、Android 图标及 iOS `LaunchScreen.storyboard`（`package_ios.sh` 新增导出后 `SplashImage.imageset` 校验）；
   - 更新 `MobileDataBootstrap.gd` 纯黑背景与启动图展示，新增 [BootSplashOverlay.gd](../../godot/presentation_gd/BootSplashOverlay.gd)（`z_index = 190`，非无头主场景延迟启动并在首帧 0.25s 淡出，无头/测试场景同步跳过）。

---

## 四、全量门禁与交叉验证记录

- `dotnet build godot/Lwsmf.csproj -c Release`：0 warnings, 0 errors（反射序列化已关闭）
- `bash tools/godot_check.sh`：38 / 38 项全通过（含 `149655/149655` 与 `989439/989439` Oracle 轨迹一致、`SaveImportTest`、`SaveLoadTest`、9 项 GDScript 表现层测试与 `lint_decisions.py`）
- `LWsmf_RENDER=1 bash tools/godot_check.sh`：OpenGL3 渲染回归门禁全通过
- `python -m unittest tools/mobile/test_mobile_tools.py`：23 / 23 项全通过（含 `release_audit.repo_scan` 零违规）
- `cargo test -p wolf-save`（含 C# 导出 `.sav` 字节级解析与重构交叉验证）：全通过
