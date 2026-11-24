# LWSMF Remake（`8e0a98d1`）三端全面静态代码审查与架构级修复方案

- **审阅基线**：`8e0a98d1dc7f8060ca74fabdba609dfb574e8a41`（对比 `b7ce308f`）
- **审查范围**：`godot/presentation_gd/`、`godot/core_cs/`、`godot/presentation/`、`godot/project.godot`、`godot/export_presets.cfg`、`tools/`、`crates/wolf-save/` 及历史 `docs/godot/DECISIONS.md`（`D-01`–`D-87`）、`docs/reviews/`、`docs/milestones/`
- **日期**：2026-11-24
- **适用对象**：本地开发 Agent 执行架构级修复、三端体验升级与防回归验证

---

## 一、对 `8e0a98d1` 增量修改的审阅结论

相较 `b7ce308f`，`8e0a98d1` 正确撤回了“空闲帧 `op124(field=8)` 截断单指轻点”的误杀逻辑，将确定性空白区收窄到非屏外停放的 `ChoiceWindow` 外部与存档页空白区，并为图片按钮菜单引入了经验式双击空白取消（`InputRouter.gd` + `GameRoot.gd` + `PictureStage.gd`）。**上一轮“可回退选项全部无法点击”的直接阻塞已解除。**

但经过 4 路静态深度审查，发现新引入的经验式双击与现有触控/核心层仍存在 **5 项 P0 级阻断或隐蔽失效点**（其中 3 项位于移动端输入/触控层，2 项位于 C# 核心层），以及 **12 项 P1 级三端实机游玩隐患**。本报告同时完整给出你追加的三项专项方案（**菜单按钮精确定位**、**原版 `.sav` 导入增强与默认 `.sav` 导出**、**三端 Boot Splash 替换与过渡架构**），并在文末附带基于历史 `DECISIONS`/`reviews`/`handoff` 提炼的**防回归红线矩阵**，确保大范围重构绝不导致旧问题复发。

---

## 二、P0 阻断与核心漏洞（5 项，建议优先根治）

| 编号 | 影响平台 | 核心位置 | 问题根因 | 架构级根治方案 |
| :--- | :--- | :--- | :--- | :--- |
| **P0-1** | Android / iOS | `GameRoot.gd:2026, 2083, 2100`<br>`GameSession.cs:168-176`<br>`PictureStage.gd:667-676` | **经验式双击空白取消在真实画面中几乎必然误判脏状态失效**：`GameSession.DescribeInstances()` 拼接了全部并行事件及每帧跳变的 `@{inst.Pc}` / `w={inst.Wait.Kind}`；`visible_slot_signature()` 纳入闪烁光标/HUD 槽；背景音效/290 特效也会标脏，导致 `is_empirical_blank_double_tap()` 在 1–19 帧窗口内几乎恒为 `false`。 | 在 `GameSession` 新增 `GetForegroundStackSignature()`，**仅序列化非并行前台实例链的 `EventId` 列表**（不含 `Pc` / `Wait.Kind`）；判定改为“Tap 1 后 2 帧内未发生前台事件栈/Phase/MapId 变化、未触发 `had_hit_match()`、未开关消息/选项/模态框、未增删前景交互图片槽”。 |
| **P0-2** | Android / iOS | `InputRouter.gd:103, 324, 329, 526-527`<br>`VmController.cs:966, 976` | **触控单帧脉冲与多帧轮询错拍（随机吞点击/吞取消）**：触控轻点与双击/双指取消仅在抬手瞬间置 1 帧 latch，未置 `pointer_pressed`/`right_pressed`；`VmController.cs:966` 的 2 帧 TTL 要求 `_pointerPressed` 仍为真，第 2 帧即失效，右键更无 TTL。含 `Wait(1)`/`Wait(2)` 的菜单轮询循环会随机丢掉 50%–66% 的触控操作。 | 在 `InputRouter` 引入可消耗的**多帧触控脉冲保持**（如 `_touch_ok_ttl = 3`、`_touch_cancel_ttl = 3`），并在 `VmController` 一旦被 `op123`/`op124` 消费后回调提前清零；同时为右键取消补齐同等消费级 TTL。 |
| **P0-3** | Android / iOS | `InputRouter.gd:82-86, 256-394`<br>`PlayOverlays.gd:157-1255` | **原生触控事件穿透所有 `Control` 模态框与按钮**：Godot 4 的 `Button`/`ScrollContainer`/`MOUSE_FILTER_STOP` 只消费仿真鼠标事件，不消费原始 `InputEventScreenTouch`/`ScreenDrag`。点按抽屉按钮、存档快捷条或模态框按钮时，原始触控先穿透到 `InputRouter._unhandled_input()`，触发底层 `action_pressed` 或在 `MessageBacklog` 内引发双重滚动。 | 在 `PlayOverlays` 统一提供 `consumes_touch_point(pos: Vector2) -> bool`（当 `is_modal_open()` 为真，或命中 `_drawer_tab` / `SavePageBar` 时返回 `true`）；在 `InputRouter._handle_screen_touch()` 与 `_handle_screen_drag()` 入口直接拦截并 `clear_touch_state()`。 |
| **P0-4** | iOS（AOT / Trim） | `GameDatabase.cs:126`<br>`SaveCodec.cs:279, 287, 300-303`<br>`GameSessionController.cs:302, 339`<br>`Lwsmf.csproj:1-14` | **全链路反射式 `System.Text.Json` 在 iOS Full AOT / IL Trimming 下静默瘫痪**：全工程无 `JsonSerializerContext`；`GameDatabase.Load` 吞异常返回 `null`（全库失效），`SaveCodec.FromJson` 捕获 `NotSupportedException` 返回 `null`（所有存档与 `.bak` 全部误判损坏）。 | 新增源生成上下文 `LwsmfJsonContext : JsonSerializerContext`，覆盖 `SaveState`、`IReadOnlyList<SavedPicture/SavedAudio>`、`GameDatabase` DTO（改为 `internal`）与 `LegacySaveImporter` 报告类；全部切到源生成重载，并在 `Lwsmf.csproj` 开启 `<JsonSerializerIsReflectionEnabledByDefault>false</JsonSerializerIsReflectionEnabledByDefault>` 与 `<IsAotCompatible>true</IsAotCompatible>`。 |
| **P0-5** | 全端（Win / Android / iOS） | `GameSession.cs:548-640, 1017-1041, 1093-1134, 1319-1323`<br>`SaveCodec.cs:199-228` | **`op250` 数据库覆写未持久化且读档未清空旧覆写**：① 重启后读档丢失所有 `op250` 写入的战斗/技能/剧情状态；② 同进程读档不清空 `DatabaseStore` 覆写层导致跨档污染；③ `ReadSaveAddress` 读任意存档槽 CData 摘要时错读活内存；④ 导入原版 `.sav` 被迫丢弃 `DatabaseCells`。 | 在 `GameDatabase` 新增 `SnapshotOverlays()`、`RestoreOverlays()` 与 `ReadWithOverlay()`；`SaveState` 升级至 `schema_version = 2`（`SaveMigrator` 对 v1 自动补空）持久化稀疏 `db_overlays`；`ApplyLoadedState` 固定先 `ClearOverlays()` 再恢复；`ReadSaveAddress` CData 分支改查目标存档的 `db_overlays`。此项同时让后续 `.sav` 导出可无损写回 `VariableDatabase`。 |

---

## 三、P1 / P2 / P3 三端实机游玩隐患与重构方案

### 1. 输入、交互与移动端适配（P1）

| 编号 | 平台 | 位置 | 问题描述 | 架构级修复方案 |
| :--- | :--- | :--- | :--- | :--- |
| **P1-1** | Android | `InputRouter.gd:22, 364`<br>`GameRoot.gd:1866` | 局内隐藏「菜单」按钮后仅剩 `x <= 60` 左缘右滑拉出抽屉，与 Android 10+ 全面屏系统左缘返回手势直接冲突，玩家难以在局内稳定打开快捷菜单。 | 将抽屉手势改为**双指下滑**（或在左缘非冲突区增加可隐藏/可拖动的小把手，默认仅在标题/存档显示常驻按钮，局内由双指下滑呼出）。 |
| **P1-2** | Android / iOS | `GameRoot.gd:922-929`<br>`AudioManager.gd:185-193` | 切后台（`NOTIFICATION_APPLICATION_PAUSED`）未暂停音频总线，锁屏/切应用后 BGM 仍继续播放，且 iOS 恢复前台后易出现音频静默。 | `PAUSED` 时对主总线静音并暂停 BGM/BGS（记录播放位置），`RESUMED` 时恢复播放并取消静音。 |
| **P1-3** | Android / iOS | `GameRoot.gd:1820-1856, 930-939` | 横屏锁定 `sensor_landscape` 下旋转 180°（刘海左↔右）时窗口宽高不变，`size_changed` 不触发，导致 `_safe_insets` 左右反置、UI 被刘海遮挡。 | 在 `_process` 低频（0.5s）或 `RESUMED` 时对比 `DisplayServer.get_display_safe_area()`，变化即刷新 `_apply_safe_area()`。 |
| **P1-4** | 全端 | `ChoiceWindow.gd:11, 135-148`<br>`GameRoot.gd:493-502, 537-541, 1998, 2194` | ① `LINE_HEIGHT=27px` 在手机上仅 ~3.2mm 易误触；② `window_rect()` 与 `row_rects()` 间存在 6–10px 内边距死区且点击死区时未清 `_click_latch`；③ `allow_cancel=false` 时取消仍误锁 `_modal_release_guard` 且 Android 返回键空转。 | 触控模式适当放大选项行高（如 `36px`）；空白区与点击命中统一以 `row_rects()` 为准并在死区/`action_denied` 显式清 `_click_latch`；`_on_cancel` 与 `handle_back_request` 先检查 `choice.allow_cancel`。 |
| **P1-5** | Android / iOS | `AudioManager.gd:91, 133` | 每次播放 BGM/BGS/SE 均调 `cached.duplicate()` 深拷贝音频缓冲，高频 SE 造成移动端内存抖动并绕过 `AssetDB` 64MB/128MB 预算。 | 加载时统一设默认 `loop=false`，SE 直接复用共享只读流（零拷贝）；仅 BGM/BGS 需要改 `loop_point` 且跨通道共享时才按需拷贝。 |
| **P1-6** | Android / iOS | `GameRoot.gd:1288-1301, 1345-1417`<br>`SetupWizard.gd:179-211, 297-317` | ① `_load_display_settings()` 在移动端仍尝试恢复桌面窗口宽高与全屏模式；② iOS 资源缺失时会落入桌面 `SetupWizard`，展示无法在手机执行的 `python`/`bash` 引导。 | 封装 `is_desktop_platform()` 守卫窗口尺寸/全屏逻辑；移动端资源校验失败时展示专用轻量错误页，禁止调用 `OS.create_process` 与 `OS.shell_open`。 |

### 2. C# 核心 VM、事件与打包稳定性（P1 / P2 / P3）

| 编号 | 级别 | 位置 | 问题描述 | 架构级修复方案（含防回归约束） |
| :--- | :---: | :--- | :--- | :--- |
| **P1-7** | **P1** | `EventVm.cs:68-94`<br>`KeyCodes.cs:16-19, 84-99` | **Opcode 126 方向位未转换**：WOLF 编辑器方向位（`Down=0x10, Left=0x20, Right=0x40, Up=0x80`）与宿主位域不同，单向/双向禁走或禁键会锁错方向。 | 在 `ApplyInputPermission` 对非设备形式调用 `KeyCodes.FromWolfBits(raw & InputTargetMask)`（并补 `0x08 -> Pause`），保留 `D-61` 三态与 `DeviceDeny` 模型；改完必跑 `VmTest` 与全路线 Oracle 验证。 |
| **P1-8** | **P1** | `EventVm.cs:1365-1376`<br>`GameDatabase.cs:380-392` | **Opcode 250 无法清空文本单元格**：用 `!string.IsNullOrEmpty(EvalStr(...))` 判定字符串源，空串 `CSelf` 槽退化为数值 `0` 后被 `SetNum` 拒绝。 | 在 `Magic` 新增 `IsStringSlot`（`StrVar` + `SelfVar` 槽 `5..9` + `SysStr`），严格按槽位类型而非内容非空构造 `DbValue`。 |
| **P1-9** | **P1** | `GameSession.cs:994-1083`<br>`EventVm.cs:1277-1295` | **Opcode 221/222 缺字符串变量与 `SysVar > 12`**：硬编码截断在 `index <= 12` 且 222 仅 `EvalNum`，不支持持久化字符串设置。 | 统一复用 `Magic.Decode(op.Address)` 分发全部四类变量及字符串槽，并在 `SaveOp` 补 `StringValue`。 |
| **P1-10** | **P1** | `AutoEvents.cs:50, 89, 132`<br>`GameSession.cs:1096` | **读档后并行事件永不停止**：`ApplyLoadedState` 恢复了并行实例，但 `PrimeLatched` 未重建 `_running[ev.Id] = inst.Id`，后续条件变假时 `_running.Remove` 找不到旧实例导致永久驻留。 | **【遵守 `D-33` 与 `D-62` 红线】** 不改动 `StartNewGameNow` 的 `_latched` 行为，也不把 `CE240` 加入 `ResumeOnLoad`；改为在 `!now` 时直接遍历 `vm.Instances` 停止所有 `inst.EventId == ev.Id && inst.IsParallel` 实例（或读档后调用 `RebindRunning(vm)` 重建映射）。 |
| **P1-11** | **P1** | `GameSession.cs:362-377, 484-493` | **同图传送去重混用 SysDB 行号与 artifact MapId**，且同帧传送与读档未互斥短路。 | 提取 `ResolveMapDefinition(int mapRowId)` 统一按 `MapDefinition.Id` 去重；`ApplyPending` 若存在待执行读档则直接清空 `_teleports` 并短路执行读档。 |
| **P2-1** | **P2** | `MediaPakReader.cs:126-144, 446`<br>`MapTileData.cs:46, 64`<br>`ProgramLoader.cs:31, 58` | ① Android `GodotFileAccessStream` 64KB 循环跨边界分配；② `MapTileData.Cache` 无锁；③ `JsonDocument.Parse` 启动内存峰值高。 | Android `ZIP_STORED` `.cfb` 改为基于 APK 物理偏移的 `SafeFileHandle + RandomAccess.Read`（或单次 `GetBuffer`）；为 `MapTileData.Cache` 加锁并归一化键；将 `ProgramLoader` / `GameDatabase` 改为 `Utf8JsonReader` 流式解析压低移动端峰值内存。 |
| **P2-2** | **P2** | `export_presets.cfg:143`<br>`ios_inject_data.py:162, 262`<br>`package_release.sh:352`<br>`VmController.cs:1100-1149` | ① iOS 未开 `entitlements/increased_memory_limit=true`；② `ios_inject_data.py` 全量 `read_bytes()` 载入 500MB+ IPA；③ `package_release.sh:352` 未判空 `.pck`；④ `VmController.FileExists` 在移动端无法查 `media.pak` 且区分大小写。 | 开启 iOS 高内存权限；`ios_inject_data.py` 改流式拷贝与分块哈希；`package_release.sh` 补 `if pck.is_file()`；`FileExists` 共享 `ResolveSaveTarget` 大小写不敏感逻辑并回退查 `MediaPakReader.Has()`。 |
| **P2-3** | **P2** | `PictureStage.gd:1360-1370`<br>`PictureShapes.gd:95-133`<br>`UiSkin.gd:279-335`<br>`MessageWindow.gd:202-217, 305-322` | ① 结束的 `zoom`/`pattern` 动画仍占 `_state_animating()` 阻止省电模式；② `<CIRCLE>`/`<TRI-*>` 逐像素循环阻塞主线程；③ `UiSkin` 重复读两次 `Lolita.ttf`；④ 大字号 `\f[28+]` 溢出气泡底部、点击快进跳过 `\!` 停顿。 | `_state_animating()` 检查 `elapsed < duration` 与 `not done`；矢量图改 `draw_circle`/`draw_polygon`；静态缓存单例 `_base_retail_font`；`_layout()` 输出真实高度且快进停在下一个 `\!` 边界。 |
| **P3-1** | **P3** | `EventVm.cs`（1966 行）<br>`GameSession.cs`（1691 行）<br>`PlayOverlays.gd:551-574` | 巨型类职责耦合、`210`/`300` 解码重复、`LastUnitStart` $O(N^2)$ 分配、`FaultModal` 4 按钮横向挤出 560px 面板、移动端弹窗残留 `F4/Esc/W/S` 提示。 | 拆分 `EventVm` 辅助模块与 `GameSession`（`SaveManager` + `MapNavigator`）；`LastUnitStart` 改 `ReadOnlySpan<char>`；`FaultModal` 改 2×2 `GridContainer` 并按 `_touch_ui_enabled` 切换触控提示。 |

---

## 四、额外专项优化方案（3 项）

### 专项 1：移动端「菜单」按钮精确定位（彻底避开标题顶部黑条）

#### 1. 现状与边界实测
- **标题界面**：顶部纯黑标题栏（含白色「妹相随 黑白世界的缤纷冒险 +DLC」字样）占据 **`y = 0..28`**。当前 `_drawer_tab`（`64×34`）放在 `y = 16..50`，上半截直接压住黑条与游戏名左侧。
- **存档界面**：顶部提示栏（「请选择想要读取的存档。」）占据 **`y = 0..68`**。
- **关键边界**：从标题点击「继续咯」进入存档页时，**`phase == "title"` 与 `_save_load_ui_active() == true` 同时成立**，不能简单按 `in_title` 区分。

#### 2. 推荐方案（按界面上下文分档 Y 偏移）
1. 在 `PlayOverlays.gd` 将 `_drawer_tab_allowed: bool` 升级为 `var _drawer_tab_context := "hidden"`（取值 `"hidden"` / `"title"` / `"save"`），并在 `GameRoot.gd:1866-1867` 中按**存档优先于标题**传参：
   ```gdscript
   var drawer_tab_ctx := "hidden"
   if save_ui:
       drawer_tab_ctx = "save"
   elif phase == "title" and choice != null and choice.is_open:
       drawer_tab_ctx = "title"
   overlays.update_drawer_state(phase == "title", can_open_quick_ui(), PERF_FPS_LABELS[_perf_tier], scaling_mode_name(), drawer_tab_ctx)
   ```
2. 在 `PlayOverlays.gd:1192` 的 `_layout_drawer()` 中分档定位并防止安全区双叠加：
   - **`"title"`**：`base_y = 36.0`（占据 **`y = 36..70`**，完整位于 `0..28` 黑条下方并留 8px 呼吸间隙，落在白色蕾丝装饰下沿，完全不遮游戏名与标题菜单项 3/4/5）。
   - **`"save"`**：`base_y = 17.0`（占据 **`y = 17..51`**，在 `0..68` 存档顶栏内上下各留 17px 严格垂直居中，左起 `x = 12 + left_inset` 避开居中提示文字）。
   - 纵坐标统一取 `maxf(base_y, 8.0 + top_inset)`，同时为 `_drawer_tab` 套用 `_style_modal_button` 的黑白极简边框风格，并将其矩形纳入 **P0-3** 的 `consumes_touch_point()` 防穿透。
3. 同步更新 `GameRootTest.gd:1107, 1120` 断言，分别校验 `title` 态 `y >= 36` 与 `save` 态 `y in 16..20`。

---

### 专项 2：原生 `.sav` 存档导入增强 & 默认原版 `.sav` 导出

#### 1. 现状缺口
- **导入侧**：`LegacySaveImporter.cs` 已实现 3 遍 LCG XOR 解密、魔数/校验和验证与 Part1~Part7 解析，但：① 未检查 `raw[6] == 0x55` UTF-8 标记（Shift-JIS 原版档走 UTF-8 替换）；② 缺少 `System.sav` 导入通道；③ `ImportRetailSaveToSlot` 写完 `save_NN.json` 后未同步 `System.json` 槽位标记（`Var[0]/Var[546]/Var[626]`）；④ `PlayOverlays.gd:1260-1280` 的 ZIP 白名单拒收 `.sav` 文件。
- **导出侧**：当前仅支持导出 JSON ZIP（`PlayOverlays.gd:1363-1395`），无原版 `.sav` 二进制写出能力；且移动端导出仅落盘私有目录 `user://exports/`，玩家难以直接取出。

#### 2. 端到端架构设计（字节级对齐 `crates/wolf-save`）
1. **新增 `godot/core_cs/LegacySaveExporter.cs`**（严格对照 `crates/wolf-save/src/lib.rs:430-651, 1009-1075` 与 `crates/wolf-game/src/savegame.rs:272-425`）：
   - **头部**：20B 明文头写入 3 字节种子（`raw[0/3/9]`）、`raw[6] = 0x55`（UTF-8）、`raw[2]` 预留字节和，接 `0x19`、`MemData<u16>` 标题与版本号 `0x008C`。
   - **七段定序流**：
     - `SavePart1`：`map_width=64, height=36`、`0xFFFFFFFF` 无快照哨兵、192 字开关表、固定 11 字尾；
     - `SavePart2`：`PlayFrames`、`ChoiceAnchor`（`SysVars[3..4]`）、BGM/BGS 路径（`Audio` 通道 0/1）；
     - `SavePart3`：Row 0 固定 800 个 `i32`（`Vars`）、550 组 × 100 `CSelf` 数值、550 组 × 5 `CSelf` 字符串（**`slotCount = 550` 严禁为 0**）；
     - `SavePart4`：默认空 `lead` + `0,0,0,0`；
     - `SavePart5`：经 `SaveMigrator.Normalize` 剔除 12000–12599 / 15000–16999 瞬态 UI 图层后，逆映射 `SavedPicture` 与 `EncodePictureText`（还原 `\E`/`\f`/`\c` 字节码）；
     - `VariableDatabase`：首字节 `0x6F`，结合 **P0-5** 的 `db_overlays` 完整编码变长数据库覆盖（无覆写时合法输出 `type_count = 0`）；
     - `SavePart7` + `0x00` + 尾字节 `0x19`。
   - **校验与加密**：计算 `plain[2] = sum(plain[0x14..]) & 0xFF`，直接复用 `LegacySaveImporter.Decrypt(plain)`（3 遍 XOR 为对合运算，`encrypt == decrypt`）。
2. **`GameSession.cs` & `GameSessionController.cs` 补齐接口**：
   - 导出：`ExportSlotToRetailSav(int slot, string dest)`、`ExportSystemToRetailSav(string dest)`、`ExportAllToRetailSavDir(string destDir, bool includeThumbnails)`（全部走 `.tmp` 原子替换）。
   - 导入：补齐 `ImportRetailSystemSave(string savPath)`、`ValidateRetailSave(string savPath)`；修复 `ImportRetailSaveToSlot` 同步写入 `System.json` 的 `Var[0]/546/626` 并透传 `LastImportNotice`；为 `LegacySaveImporter` 补齐 `raw[6] != 0x55` 时的 Shift-JIS（`cp932`）解码支持（使用 `CodePagesEncodingProvider.Instance`）。
3. **`PlayOverlays.gd` 跨端交互改造**：
   - **默认导出改为原版 `.sav`**：主按钮 1「导出原版存档包 (.zip)」（默认含 `SaveDataNN.sav` + `System.sav` + `saveN.jpg`）；主按钮 2「导出当前槽位 (.sav)」（默认文件名 `SaveData%02d.sav`）；次级按钮保留「导出重制版完整备份 (JSON .zip)」。
   - **统一智能导入**：`is_valid_save_archive_entry` 放行 `savedata\d{2}\.sav` 与 `system.sav`；单文件与 ZIP 均按文件头（`PK` = ZIP、`{` = JSON、其余 = `.sav`）与文件名（`System.sav` 自动归入系统档）自动分流。
   - **移动端文件读写**：对接 Android SAF（`ACTION_CREATE_DOCUMENT` / `ACTION_OPEN_DOCUMENT`）与 iOS `UIDocumentPickerViewController` / `UIActivityViewController`，解决沙盒不可见问题。
4. **交叉验证**：在 `SaveImportTest.cs` 新增 `SaveState -> Export -> Import -> SaveState` 往返测试，并用仓库内 `cargo test -p wolf-save` 对 C# 导出的 `.sav` 做字节级校验。

---

### 专项 3：替换 Godot 默认启动过场图（三端统一 + 原版解包即插即用）

#### 1. 单一资产槽规范（`godot/assets/boot/`）
建立唯一 Drop-in 目录（纳入 PCK）：
- `godot/assets/boot/splash_static.png`：纯黑 `#000000` 底 + 居中圆形 `imotowork` logo（`1024×576` 或 `1920×1080`）。三端原生启动屏与引擎启动图共用此单源；在原版 `Game.exe` 尚未解包完成前，先放一张纯黑占位图即可立刻消灭 Godot 默认灰屏 Logo，解包完成后直接覆盖 PNG 重新打包即可三端同步更新。
- `godot/assets/boot/splash_frame_0.png`、`splash_frame_1.png`（可选）：原版 2 帧 chibi 启动动画，存在时在引擎内过渡层自动以 4–5 Hz 轮播，不存在时静默降级为静态图。

#### 2. 四层三端无缝衔接架构
1. **第一层（Godot `boot_splash` — Windows 全程 + 移动端引擎接管期）**：
   在 `godot/project.godot` `[application]` 增加：
   ```ini
   boot_splash/bg_color=Color(0, 0, 0, 1)
   boot_splash/image="res://assets/boot/splash_static.png"
   boot_splash/fullsize=false
   boot_splash/stretch_mode=1
   boot_splash/use_filter=true
   boot_splash/show_image=true
   boot_splash/minimum_display_time=0
   ```
2. **第二层（Android 12+ 原生冷启动防闪蓝机器人）**：
   保持当前轻量化无 Gradle 导出，在 `export_presets.cfg` `[preset.1.options]` 配置 `launcher_icons/main_192x192`、`adaptive_foreground_432x432`（安全区 62% 居中 logo）与 `adaptive_background_432x432`（纯黑 `#000000`），由前置脚本 `tools/sync_boot_assets.py` 自动从 `splash_static.png` 派生，保证 Android 12+ 系统冷启动图标与 Godot `boot_splash` 纯黑无缝衔接。
3. **第三层（iOS 原生 `LaunchScreen.storyboard`）**：
   Godot 4 iOS 导出不会自动继承 `boot_splash/image`，必须在 `export_presets.cfg` `[preset.2.options]` 显式配置：
   ```ini
   storyboard/use_launch_screen_storyboard=true
   storyboard/image_scale_mode=1
   storyboard/custom_image@2x="res://assets/boot/splash_static.png"
   storyboard/custom_image@3x="res://assets/boot/splash_static.png"
   storyboard/use_custom_bg_color=true
   storyboard/custom_bg_color=Color(0, 0, 0, 1)
   ```
   并在 `tools/package_ios.sh` 导出 `.xcodeproj` 后校验 `SplashImage.imageset` 已非 Godot 默认图。
4. **第四层（引擎内过渡层 `BootSplashOverlay.gd` + `MobileDataBootstrap.gd`）**：
   - 将 `MobileDataBootstrap.gd:189` 背景色从灰蓝改为纯黑 `Color(0, 0, 0, 1)`，进度条上方展示 `splash_static.png`（或 2 帧 chibi 轮播）。
   - 新增 `godot/presentation_gd/BootSplashOverlay.gd`（`z_index = 190`）：`GameRoot._ready()` 先挂纯黑居中过场层并通过 `call_deferred("_start_game")` 让首帧立即上屏再解析 `programs.json`；进入 TitleMap 首帧（下层同为纯黑 `Kagura/bg.png`）后 0.25s 线性淡出并释放；`_is_headless()` 时直接跳过，保证自动化测试帧序与耗时零变化。

---

## 五、历史防回归红线矩阵（梳理自 `DECISIONS.md` / `reviews` / `handoff`）

执行上述中大型重构时，以下 **6 条历史验证红线绝对不可触碰**，否则会导致既有 Oracle 或核心玩法回归：

| 红线 | 来源决策 | 不可破坏的不变量 | 守护测试 |
| :--- | :--- | :--- | :--- |
| **红线 1** | `D-33` | **`StartNewGameNow()` 不得清空 `AutoEvents` 的 `_latched`/`_running`**（清空会导致多出 `CE38` 实例使对拍退化）；同会话重开新游戏必须走 `GameSessionController.Rebuild()`。 | `SessionParityTest.cs`, `GameRootTest.gd` |
| **红线 2** | `D-82`、`89fc8f5` P1-6 | **`AutoEvents.Sync` 在 `LoopVisualPainters == false`（脚本对拍路径）不得加 `!alive` 守卫**（否则 `M4FullRouteTrace` 在 frame 2307 `CE242` 分歧）；`!alive` 仅用于实机模式 `LoopVisualPainters == true`。 | `M4FullRouteTraceTest.cs`, `AutoEventsTest.cs` |
| **红线 3** | `D-62` | **`CE240`（战力条）不得加入 `AutoEvents.ResumeOnLoad`**（每帧触发 2 次 `0x6100` PRNG，破坏战斗随机序列）。 | `BattleWiringTest.cs`, `BattleMenuBridgeTest.cs` |
| **红线 4** | `89fc8f5` P2-11 | **`SaveCodec.Validate` 在 `WaitKind == Child` 时不得拦截缺失的 `WaitChild` 目标**（只拦 `<= 0` 或自环），由读档后的 `ClearMissingChildWaits()` 释放。 | `SaveLoadTest.cs` |
| **红线 5** | `D-72`, `D-78`, `D-84` | **`PictureStage.picture_state` field 2/3 必须保持返回像素宽高（`scale==0` 返回未缩放基准尺寸）**；`op150` 混合模式来自 `(head >> 8) & 0x0F` 且子节点开 `use_parent_material = true`；`SaveGameToSlot` 与 `F5/F9` `quicksave.json` 彻底隔离。 | `PictureStateTest.gd`, `PictureRoundtripTest.gd`, `SaveLoadTest.cs` |
| **红线 6** | `D-20`, `D-78`, `D-87` | **`op123 arg[1]` 不是键位掩码**不可拿来滤键；`GameRoot._on_event_entered` 不得用 `PointerPressObserved` 抢先清 `_click_latch`；移动端单指双击空白取消**严禁在首次轻点截断未知图片菜单**。 | `InputRouterTest.gd`, `GameRootTest.gd`, `VmTest.cs` |

---

## 六、建议落地顺序与回归验证门禁

1. **第一批（核心正确性与 iOS/全端 P0 根治）**：**P0-4**（`LwsmfJsonContext` 源生成）→ **P0-5**（`GameDatabase` 覆写持久化 `schema_version=2`）→ **P1-7 ~ P1-11**（`op126`、`op250` 空串、`op221/222`、`AutoEvents` 读档停止、传送去重）。
2. **第二批（移动端输入与 UI/UX 彻底重构）**：**P0-1 + P0-2 + P0-3**（前景实例栈签名 + 触控多帧消费级 TTL + `PlayOverlays` 触控防穿透）→ **专项 1**（菜单按钮分档 Y 定位）→ **P1-1 ~ P1-6**（抽屉手势、后台静音、180° 安全区、选项行高、音频零拷贝、移动端显示/向导守卫）。
3. **第三批（存档 `.sav` 导出与三端启动画面）**：**专项 2**（`LegacySaveExporter.cs` + `PlayOverlays` 默认 `.sav` 导出与智能导入 + `cargo test -p wolf-save` 交叉校验）→ **专项 3**（`godot/assets/boot/` + 三端配置 + `BootSplashOverlay.gd` + `tools/sync_boot_assets.py`）。
4. **全量门禁与真机覆验**：
   - `dotnet build godot/Lwsmf.csproj -c Release`（验证关闭反射序列化后零警告/零错误）；
   - `bash tools/godot_check.sh` 与 `LWsmf_RENDER=1 bash tools/godot_check.sh`（守住 38 项门禁与 149,655 / 989,439 步 Oracle 100% 一致）；
   - `python -m unittest tools/mobile/test_mobile_tools.py` 与 `cargo test -p wolf-save`；
   - Android / iOS / Windows 三端真机覆验：冷启动无 Godot Logo、标题页菜单按钮不压黑条、存档页 SD 小人正常、可回退菜单单点不丢包且双击空白稳定返回、默认导出 `.sav` 可被原版 `Game.exe` 正常读取。
