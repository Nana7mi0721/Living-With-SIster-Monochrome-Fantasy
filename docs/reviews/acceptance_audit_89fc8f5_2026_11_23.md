# LWSMF 整改验收与增量回归审查报告（Commit `89fc8f54`）

**验收对象**：`Living-With-SIster-Monochrome-Fantasy` commit `89fc8f54246c8d195e92292f4f25d729d1d708eb`  
**对照基线**：commit `59268ac` 与审计清单 `FULL_STATIC_AUDIT_59268ac_2026_11_22.md`（共 67 项）  
**附加范围**：本轮新增 HD 高清素材包管线（`AssetDB.gd`、`PictureStage.gd`、`HDPackTest.gd`、`tools/upscale_assets.py`、`tools/verify_hd_manifest.py`）、`tools/mobile/windows_pe.py`、移动端 UX 改动及整改声明 `docs/reviews/full_static_audit_59268ac_remediation_2026_11_22.md`  
**验证边界**：静态源码级逐行验收 + 离线 Python/Shell 校验（`lint_decisions.py`、`test_mobile_tools.py` 9/9、`py_compile`、`bash -n`）；需 Godot / .NET / Android 实机复核的条目仍标 `needs-runtime`

---

## 1. 总体结论：有条件通过（主线修复率 88.1%，无新增 P0，余 1 项未通过 + 6 项新增 P1）

| 严重级别 | 基线总数 | 完全通过 | 部分通过 | 未通过 | 增量新增 |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **P0** | 4 | 3 | 1（P0-2 新步复现） | 0 | **0** |
| **P1** | 24 | 21 | 2（P1-4, P1-14） | 1（P1-6） | **6** |
| **P2** | 26 | 24 | 2（P2-3, P2-11） | 0 | **14** |
| **P3** | 13 | 11 | 2（P3-PictureStage, P3-tileset） | 0 | **12** |
| **合计** | **67** | **59（88.1%）** | **7（10.4%）** | **1（1.5%）** | **32** |

* **准入判断**：前序 4 项 P0 阻断（`package_release.sh` 字体解析、`LegacySaveImporter` OOM/死循环、原 `ci.yml` 吞码）主体均已修复，且本轮**未引入任何新的 P0 崩溃或死锁**。但因存在 **1 项未通过的 P1（`AutoEvents.cs:116-117`）**、**1 项 P0-2 同源回归（`ci.yml:44-48`）** 以及 **HD 高清包与零售导入的 5 项新增 P1**，建议按第 4 节清单补齐“收口补丁”后再做正式发布封版。

---

## 2. 基线 67 项整改逐项验收表

### 2.1 P0（4 项：3 通过 / 1 部分通过）

| 编号 | 结论 | 新仓库精确证据（`file:line`） | 说明 |
| :--- | :--- | :--- | :--- |
| **P0-1** | **通过** | `tools/package_release.sh:190-206`、`tools/pack_media.py:95-114` | 已移除错误二进制解析，改用 `to_win_path` + `pack_media.read_pak_index()` 检测 `lolita.ttf` |
| **P0-2** | **部分通过** | `.github/workflows/ci.yml:38-42, 44-48` | 原 `lint_decisions.py` 与 `release_audit.py` 已拆分独立步骤；但新增步骤 `Mobile Packaging Tools Unit Tests`（`:44-48`）在 Windows `pwsh` 下合并了 3 条命令，复现同类吞退出码缺陷（见 **N-P1-5**） |
| **P0-3** | **通过** | `godot/core_cs/LegacySaveImporter.cs:462-498` | 已补齐 `typeCount`/`fc`/`cnt` 剩余字节与上限校验、`cfg.Length==0` 拦截及 `uint` 循环变量 |
| **P0-4** | **通过** | `godot/core_cs/LegacySaveImporter.cs:73-74, 211-220, 253-264` | `ReadBytes` 改为无溢出减法判界；文件大小双重限 `MaxSaveBytes`；地图格数改 `checked(ulong)` |

### 2.2 P1（24 项：21 通过 / 2 部分通过 / 1 未通过）

| 编号 | 结论 | 新仓库精确证据（`file:line`） | 未闭环/残余说明 |
| :--- | :--- | :--- | :--- |
| **P1-1** | **通过** | `EventVm.cs:116` | 已改用 `KeyCodes.FromWolfBits(AutoKeys)` 对齐位图 |
| **P1-2** | **通过** | `EventVm.cs:905-914, 1642-1651` | 阻塞式 `op123` 遇到已消费的 125 自动键直接交付 |
| **P1-3** | **通过** | `GameSession.cs:605, 1416-1458, 1486-1508` | 已持久化 `SavedInstance.IsParallel` 并为读档地图实例置 `RetainAfterFinish=true` |
| **P1-4** | **部分通过** | `GameSession.cs:225-248, 1026-1030, 1366-1401, 1525-1529` | `StartTitle`/`ApplyLoadedState`/`EnterMap` 已闭环；但 `StartNewGameNow`（`:1366-1401`）仍未清 `op250` DB 覆盖层与 `Frames`，`LoadRetailSave`（`:1316-1320`）漏清挂起读档与弹窗（见 **N-P2-1**） |
| **P1-5** | **通过** | `EventVm.cs:423, 445`、`GameDatabase.cs:334-335` | `int.MinValue / -1` 与 `% -1` 及除零均已防护 |
| **P1-6** | **未通过** | `AutoEvents.cs:39, 116-117` | `LoopVisualPainters == false` 时非 `ResumeOnLoad` 分支仍漏 `!alive` 守卫，条件上升沿仍可与 `BattleWiring` 双起 CE242/243 |
| **P1-7** | **通过** | `SaveCodec.cs:98-105, 319-491`、`SaveMigrator.cs:48-49` | `Strings == null` 及数组 `null` 元素不再抛 NRE 击穿 `.bak` 回滚 |
| **P1-8** | **通过** | `SaveCodec.cs:288-293, 549-557, 609-620` | `SaveMigrator.Normalize` 已移入 `FromJson` 内部并在 `Validate` 前执行 |
| **P1-9** | **通过** | `VmController.cs:370-399, 1034-1041` | `StopAll()` 已调 `ResetDialogueState()` + 清 `_pictures`；无等待协程时无条件清 `_pendingChoiceReply` 与消息标志 |
| **P1-10** | **通过** | `VmController.cs:997-1010` | 设备轮询（如 `mask==771`）不再混入 `autoKeys` |
| **P1-11** | **通过** | `TerrainBridge.cs:20-32` | `map == null` 时不再解引用 `map.Id` |
| **P1-12** | **通过** | `MediaPakReader.cs:140-177, 180-375` | 已重写 `Dispose(bool)` 并将全部读写纳入 `lock (_gate)` |
| **P1-13** | **通过** | `GameSessionController.cs:208-363`、`VmController.cs:1015-1142`、`AppPaths.cs:16-37` | `QuickSavePath`、`CallableView` 存活校验与 `TryGetValue`、`SetKeyStateProvider(GodotObject, StringName)`、`FileExists` 限域、`AppPaths` 分离 `ResourceRoot`/`GodotResourceRoot` 均已闭环 |
| **P1-14** | **部分通过** | `PlayOverlays.gd:529-535`、`GameRoot.gd:232-234, 1058-1062, 1895-1916` | 桌面端已支持 `Esc`/`F12` 重开 FaultModal；但移动端无 Esc/F12，且 `handle_back_request()` 在隐藏后按返回键仍被 `faulted` 拦截导致触屏端软锁死（见 **N-P2-8**） |
| **P1-15** | **通过** | `MobileDataBootstrap.gd:456-461, 567-587` | 清单与版本标记已改用 `_write_text_atomic_abs()` 原子写入并验错误码 |
| **P1-16** | **通过** | `MobileDataBootstrap.gd:75-116, 285-306, 448` | `_entry_expected_bytes()` 强制校验 `bytes` 字段存在且非负 |
| **P1-17** | **通过** | `MobileDataBootstrap.gd:147-187`、`SetupWizard.gd:119-215`、`PlayOverlays.gd:108-110`、`UiSkin.gd:26-34, 156-201` | 移动端首启与全套前端弹窗均已绑定 CJK 字体主题 |
| **P1-18** | **通过** | `PictureStage.gd:999-1006` | `scale_mode == 3` 独立缩放不再把 `args[7]==UNCHANGED` 误覆盖到 `scale_y` |
| **P1-19** | **通过** | `PictureStage.gd:1423-1502` | 快照已持久化 `tone` 与 `effects_290`，恢复时清零 `args[0]` 避免误重播淡入 |
| **P1-20** | **通过** | `GameRoot.gd:683-684, 721-728, 945` | `op161` 冻结层已复用 `_grab_viewport_image()` 并在空闲/切图时兜底清理 |
| **P1-21** | **通过** | `tools/godot_check.sh:278-283, 350-351` | 并行任务池已改用 `IFS=$'\x1f'` 兼容含空格路径 |
| **P1-22** | **通过** | `tools/release_audit.py:59-63, 209-243` | 已用 `WOLF_CTRL_RE`、`LEADING_REL_RE` 与 `PurePosixPath` 修复漏报/误报 |
| **P1-23** | **通过** | `tools/package_android.sh:198-204`、`tools/package_release.sh:147-292` | Android 预构建已传 `-p:GodotTargetPlatform=android`；`package_release.sh` 全路径已包 `to_win_path` |
| **P1-24** | **通过** | `tools/mobile/env_check.py:340-350`、`tools/visual_regression.py:67-70, 162-178, 284-285` | `env_check` 两项已升 `MISSING` + `blocker=True`；`visual_regression` 已清旧图并校验 `phase`/`save_png`/`misses` |

### 2.3 P2（26 项：24 通过 / 2 部分通过）与 P3（13 项：11 通过 / 2 部分通过）

* **全部完全通过的 P2（24 项）**：P2-1、P2-2、P2-4、P2-5、P2-6、P2-7、P2-8、P2-9、P2-10、P2-12、P2-13、P2-14、P2-15、P2-16、P2-17、P2-18、P2-19、P2-20、P2-21、P2-22、P2-23、P2-24、P2-25、P2-26。
* **部分通过的 P2（2 项）**：
  * **P2-3**（`EventVm.cs:310-332` 已钳位 `strCount <= 4`，但 `Variables.cs:31, 73, 113` 的 `SetStr` 底层未做 `value ?? ""` 兜底）。
  * **P2-11**（`SaveCodec.cs:321, 401, 482` 已补其余校验，但 `SaveCodec.cs:361-394` 仍漏检 `WaitKind == Child` 时 `byId.ContainsKey(inst.WaitChild)`，悬空 `WaitChild` 仍可放行）。
* **部分通过的 P3（2 项）**：
  * **P3-PictureStage**（死常量已清，`6252..11999` 跳过 `ScreenTint` 仍需 `needs-runtime` 实机确认）。
  * **P3-tileset**（`crates/wolf-format/tests/tileset.rs` 已拆出 2 项离线测试并 `#[ignore]` 4 项零售测试，但 `.github/workflows/ci.yml:86` 仍只跑 `--lib`，未运行 `--test tileset`）。

---

## 3. 本轮增量代码与 HD 素材管线新发现的问题清单

### 3.1 新增 P1（6 项 — 建议发版前收口）

| 编号 | 位置（`file:line`） | 根因与影响 | 最小修复 |
| :--- | :--- | :--- | :--- |
| **N-P1-1** | `godot/core_cs/GameSession.cs:1136-1250` | 新增 `ImportRetailSaveToSlot` 静默丢弃 `data.DatabaseCells`（`op250` 数据库覆盖层），且 `:1220-1227` 未过滤 `pic.Number < 0`（可能导致 `:1229` 校验整槽拒收） | 在 `SaveState` 支持持久化 `db_cells` 或在方法状态中显式提示；`:1222` 补 `pic.Number >= 0`（或对齐负槽策略） |
| **N-P1-2** | `godot/presentation_gd/PictureStage.gd:759-760` | `track_picture()` 调用 `_texture_cell_size()` 漏传第 4 参数 `assets.hd_scale(file)`，2x HD 包下无头/追踪模式的命中框面积膨胀 4 倍且非左上锚点坐标全部偏移 | 改为 `_texture_cell_size(tex, ..., assets.hd_scale(file))` |
| **N-P1-3** | `godot/presentation_gd/GameRoot.gd:1235-1249, 1265-1266`、`AssetDB.gd:93-94` | 移动端首次关闭 `hd_enabled = false`，但 `_save_display_settings()` 不写 `hd_textures`，二次启动 `_load_display_settings()` 用默认值 `true` 强制开启 HD 包，击穿移动端 64/128 MB 纹理预算 | `:1266` 默认值改用 `assets.hd_enabled`（或先验 `has_section_key`），并在保存时持久化 `hd_textures` |
| **N-P1-4** | `godot/presentation_gd/AssetDB.gd:281-299, 486-512` | (a) HD 解码失败回退 1x 后 `hd_scale()` 仍返回 `2.0`，把 1x 纹理误缩为 0.5x；(b) HD 裸名回退先于 1x 精确路径导致跨目录串图；(c) `:507` 未判 `path.is_empty()` 致频繁空读 `hd_pak` | 记录实际命中缩放倍率或拉黑失败 HD 键；仅在 1x 精确路径缺失时允许 HD 裸名回退；`:507` 补 `path.is_empty()` 短路 |
| **N-P1-5** | `.github/workflows/ci.yml:44-48` | 新增 `Mobile Packaging Tools Unit Tests` 在 Windows `pwsh` 下合并 3 条命令，前两条失败会被吞码（P0-2 同源回归） | 拆为独立 `run:` 步骤或设 `shell: bash` + `set -euo pipefail` |
| **N-P1-6** | `tools/verify_hd_manifest.py:75-80`、`tools/release_audit.py:298-307` | 退出码漏判 `degraded`（真实非全不透明 Alpha 通道丢失）且失败时仍打印 `HD_MANIFEST_VERIFY_OK`，导致 `--hd-root` 门禁放行掉透明坏图 | `:80` 纳入 `degraded` 判定，失败时输出 `HD_MANIFEST_VERIFY_FAIL` 并返回 `1` |

### 3.2 新增 P2（14 项）

| 编号 | 位置（`file:line`） | 问题与最小修复 |
| :--- | :--- | :--- |
| **N-P2-1** | `GameSession.cs:1310-1321` | `LoadRetailSave` 漏清 `_pendingGameLoad`/`_pendingSystemLoads`/`ChoiceAllowsCancel` 与视图弹窗；提取共用复位函数 |
| **N-P2-2** | `GameSession.cs:1486-1508` | `InferLegacyParallel` 仅判 `RunCondition == 2`，遗漏 `AutoEvents.GatedCallOnly`（CE242/243）；补入该列表 |
| **N-P2-3** | `GameSession.cs:1089-1130` | `ValidateSaveJson` 在 `JsonDocument.Parse` 前未先验 `MaxSaveBytes`；首行增加 UTF-8 字节上限早退 |
| **N-P2-4** | `SaveCodec.cs:283` | `FromJson` 在 `try` 外调 `StrictUtf8.GetByteCount(json)`，孤立 UTF-16 代理对会抛未捕获 `EncoderFallbackException`；移入 `try` 或改用 `Encoding.UTF8` |
| **N-P2-5** | `MessageWindow.gd:545-573` | `_draw_cursor()` 未除以 `assets.hd_scale()`，2x HD 光标将放大 2 倍并偏移；按实际命中的光标路径取 `art_scale` 缩放目标尺寸与分帧高度 |
| **N-P2-6** | `PictureStage.gd:782-796` | 标题图回退加载 `SystemGraphic/TitleGraphic.png` 后仍用原 `file` 查 `hd_scale`，若标题图为 2x 会画成 2048×1152；改查 `resolved_file` |
| **N-P2-7** | `project.godot:24-25`、`GameRoot.gd:1252-1259` | 默认窗口改为 `2048×1152`，但无 `display_settings.cfg` 时提前 `return` 跳过夹紧，1080p 屏首次启动窗口溢出；无配置分支也调用屏幕可用区夹紧 |
| **N-P2-8** | `GameRoot.gd:1895-1916`、`PlayOverlays.gd:529-535` | 触屏端隐藏 FaultModal 后无 Esc/F12 且返回键被 `faulted` 拦截；在 `handle_back_request()` 中于 `faulted` 且隐藏时重开弹窗（或移动端隐藏关闭按钮） |
| **N-P2-9** | `AssetDB.gd:436`、`PictureShapes.gd:169-170`、`UiSkin.gd:325-326` | 畸形 HD manifest `"entries": null` 强转 `Dictionary` 崩溃且漏关 `reader`；窗口皮肤 `width >= 27` 启发式易误判 1x 自定义皮肤倍率；改 `is Dictionary` 校验并透传 `hd_scale()` |
| **N-P2-10** | `.github/workflows/ci.yml:85-86` | `cargo test --workspace --lib` 跳过集成测试，`tileset.rs` 新增 2 项离线测试未在 CI 运行；追加 `cargo test -p wolf-format --test tileset` |
| **N-P2-11** | `tools/upscale_assets.py:47-49` | 硬编码本机路径 `D:\Program\Waifu2x-Extension-GUI\...`，违反仓库无本机路径红线；默认置空改为环境变量/参数/PATH 探测 |
| **N-P2-12** | `tools/upscale_assets.py:388, 422, 536-539` | 写 `work/out/hd_*` 与 `mkdtemp(dir=out_root.parent)` 前未确保父目录存在，`hd_manifest.json` 非原子覆写；提前 `mkdir` 并改 `.tmp` + `os.replace` |
| **N-P2-13** | `tools/upscale_assets.py:97-116, 143` | 重跑换格式（`.png` → `.webp`）不删旧产物，`pack_media.py --hd-root` 会打入孤儿旧图；写入后清理同基名另一后缀文件 |
| **N-P2-14** | `tools/verify_hd_manifest.py:39-41, 61-64` | Linux/macOS 大小写敏感盘上用全小写 `key` 查源图误报缺失，且未跳 `#` 注释行；建立 `rel.lower() -> rel` 映射并跳过 `#` 行 |

### 3.3 新增 P3（12 项，精简汇总）

* **C# 核心与存档（4 项）**：`EventVm.cs:1447-1464, 1699-1701`（`174`/`175` 未清过渡态、`StopInstance` 漏清 `_autoKeysDelivered`）；`Variables.cs:31, 73, 113`（`SetStr` 未做 `value ?? ""`）；`LegacySaveImporter.cs:584-591`（非法多字节截断未检 UTF-8 续字节）；`MediaPakReader.cs:117-130, 356-358`（`GodotFileAccessStream.ReadExactly` 双倍堆分配）。
* **GDScript 与测试（3 项）**：`GameRoot.gd:1707-1718`（`_apply_safe_area` 在 `aspect="keep"` 20:9 屏未扣左右黑边）；`HDPackTest.gd:10-44`（仅测 `_base_size_for` 且硬编码 `1024×576`，缺 `track_picture`/切图/失败回退离线用例）；`MobileDataBootstrap.gd:23-24` 死字段与 `PlayOverlays.gd:1104, 1506` 信号/弹窗主题卫生。
* **工具链与整改文档一致项（5 项）**：`tools/upscale_assets.py:58, 450-452`（注释 `244` 与默认 `249` 不一致及死常量 `ART_DIRS`）；`tools/release_audit.py:299-300`（`main()` 内重复 `import sys/subprocess` 局部遮蔽风险）；`tools/mobile/windows_pe.py:14, 45-53, 103, 436-442`（未用 `shutil`、10 进制 LCID 误按 16 进制解析、重复语言项与 RVA 尾界检查）；`tools/package_release.sh:311-318`（`--hd` 时 `SHA256SUMS.txt` 漏记 `media_hd.pak`）；**文档核对**：`docs/reviews/full_static_audit_59268ac_remediation_2026_11_22.md` 第 27 行关于 `ci.yml` 防吞码的声明与 `:44-48` 不符、第 41 行 `Ran 6 tests` 已过期（现为 9 测），`DECISIONS.md:81`（D-81）对 `JPG=1` 描述已落后于 `upscale_assets.py:62-71`。

---

## 4. 建议收口动作（1 个短补丁即可达到完全通过）

1. **修复 1 项未通过 + 2 项关键残余**：`AutoEvents.cs:116-117` 补 `!alive`（P1-6）、`SaveCodec.cs:371` 补 `byId.ContainsKey(inst.WaitChild)`（P2-11）与 `:283` `try` 保护（N-P2-4）、`GameSession.cs:1366` 补 `StartNewGameNow` 会话重置（P1-4）。
2. **修复 6 项新增 P1**：`PictureStage.gd:759` 补 `hd_scale`（N-P1-2）、`GameRoot.gd:1266` 保留移动端默认 `hd_enabled`（N-P1-3）、`AssetDB.gd:281-512` 修复 HD 失败倍率/裸名串图/空路径（N-P1-4）、`ci.yml:44-48` 拆步防吞码（N-P1-5）、`verify_hd_manifest.py:80` 拦截 `degraded`（N-P1-6）、明确或补齐 `ImportRetailSaveToSlot` 的 `DatabaseCells` 迁移（N-P1-1）。
3. **清理硬编码路径与实机回归（`needs-runtime`）**：移除 `tools/upscale_assets.py:47-49` 的本机 `D:\Program\...` 路径，随后在 Windows / Android 实机复跑 `tools/godot_check.sh`（含 `LWsmf_HD_PAK`）、`tools/package_release.sh --hd` 与 `tools/visual_regression.py`。
