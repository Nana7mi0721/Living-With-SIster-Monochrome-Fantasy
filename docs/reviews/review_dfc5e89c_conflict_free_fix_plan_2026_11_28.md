# LWSMF Remake（`dfc5e89c`）增量 Review 与无冲突修复方案

- **审阅基线**：`dfc5e89c4bb157225c1f66a3fa1a99a1ded8dff0`（对比 `cae87b52`）
- **决策对齐**：已逐条交叉核对 `docs/godot/DECISIONS.md`（`D-01`–`D-104`）与 `docs/reviews/lwsmf_mobile_4issues_remediation_2026_11_26.md`（`MOB-01`–`MOB-04`），剔除全部与历史决策冲突的改动
- **日期**：2026-11-28
- **适用对象**：本地开发 Agent 逐项落地与回归验证

---

## 一、总体审阅结论与历史决策对齐

1. **总体结论**：`dfc5e89c` 无 **P0** 阻断，无 iOS AOT/Trim JSON 反射回归，无版权素材入库，完整守住了 `D-33`（新游戏不复位 `AutoEvents`）、`D-62`（`CE240` 不入 `ResumeOnLoad`）与 `D-82`（脚本对拍分支不加 `!alive`）红线；离线 `py_compile`、`bash -n` 与 `tools/mobile` 单元测试全部通过。
2. **本轮检出待修项**：共 **7 项 P1**、**10 项 P2** 与 **5 项 P3**。
3. **与历史决策对齐后的明确边界（严禁误改）**：
   - **保留 `MOB-04`（`ChoiceWindow.gd:76-80`）**：继续保持标准 `ChoiceWindow` 垂直居中 `center.y = 288.0`，**不**改回显式 `Sys4`（`DialogDrawTest.gd` 与 `GameRootTest.gd` 已固化断言）；仅在底部 `MessageWindow` 同屏开启（`D-70`）且触控 36px 多选项下沿撞框时动态收紧 `max_y`。
   - **保留 `D-92` 第 ③ 条**：保持 Windows 发行元数据中的 `Inusuku` 署名与版权声明不变，不主动改动。

---

## 二、P1 重点缺陷与修复方案（7 项）

| 编号 | 领域 | 位置 | 根因与影响 | 无冲突修复方案（含护栏） |
| :--- | :--- | :--- | :--- | :--- |
| **P1-1** | 启动遮罩 / 输入 | `godot/presentation_gd/GameRoot.gd:120-126, 355-360`<br>`godot/presentation_gd/BootSplashOverlay.gd:26, 95-96` | 启动擦除 + 淡出期间（约 `0.53s`）`GameRoot` 已提前运行 `advance_frame()`，且 `BootSplashOverlay` 仅设置 `MOUSE_FILTER_STOP`（只拦鼠标 GUI，不拦键盘与 `InputEventScreenTouch`），玩家在 Logo 期间按键或点屏会穿透触发标题菜单或退出确认。 | 当 `_boot_splash != null` 且尚未完成揭示时，在 `GameRoot` 与 `InputRouter` 入口拦截全部键盘与触控输入，并暂缓 `advance_frame()`（仅允许首帧绘制底图）。<br>**护栏（`D-90`）**：严格限定仅在 `_boot_splash != null`（非无头窗口态）生效，确保无头 38 项门禁与 `ColdStartDayTest` 帧序/耗时零变化。 |
| **P1-2** | 移动端存档导入 | `godot/presentation_gd/PlayOverlays.gd:1695` | `apply_save_zip` 写非 `.sav` 文件时直接使用压缩包原始 `leaf`（`save_dir.path_join(leaf)`），未调用 `_retail_system_target_name()`；若 ZIP 内是小写 `system.json` 或 `System2.json`，在 Android/iOS/Linux 大小写敏感文件系统上落盘错名，无法覆盖正确系统档。 | 在 `PlayOverlays.gd:1695` 落盘前先查 `var sys_target := _retail_system_target_name(leaf_lower)`，非空时写入 `save_dir.path_join(sys_target)`。 |
| **P1-3** | 旧档安全 / 防闪退 | `godot/core_cs/LegacySaveImporter.cs:427-428` | 未校验 `mds1Count` 与剩余字节即执行 `new List<string>(mds1Count)`；畸形 `.sav` 写入极大正整数会直接触发 `OutOfMemoryException` 闪退（`ValidateFile` / `ImportRetailSaveToSlot` 均不捕获 OOM）。 | 分配前校验 `(ulong)mds1Count * 4UL <= (ulong)r.Remaining && mds1Count <= Variables.MaxSlots`，越界抛 `InvalidDataException`；初始容量改用 `Math.Min(mds1Count, 1024)`。 |
| **P1-4** | 存档损坏回退 | `godot/core_cs/GameSession.cs:1056-1060` | `op 221` 对 `Sys24` 的存在性快路径改用 `SaveCodec.Exists(path)` 仅查磁盘文件，不校验内容；若主档与 `.bak` 均为空或损坏 JSON，探针误返 `1`，存档页显示“第 0 天 / 00:00”幽灵槽位且点击读档失败。 | 空槽先用 `if (!SaveCodec.Exists(path)) return 0;` 快速早退；文件存在时走带元数据缓存的 `ReadCachedSave(path).Exists ? 1 : 0`，兼顾性能与损坏档回退语义。 |
| **P1-5** | Windows 句柄安全 | `godot/presentation/WindowIcons.cs:140-160` | `Release()` / `Dispose()` 直接 `DestroyIcon` 销毁仍绑定到存活 `HWND`/`WNDCLASS` 的 3 个 `HICON`；且句柄字段未使用 `Interlocked.Exchange` 原子清零，存在主线程与终结器双释放竞态。 | 保存目标 `HWND` 及 `SendMessage`/`SetClassLongPtr` 返回的原始句柄；在 `Release()` 中先于主线程（`IsWindow(hwnd)` 为真）恢复旧句柄，再用 `Interlocked.Exchange` 原子取出并销毁，显式释放后调用 `GC.SuppressFinalize(this)`。<br>**护栏（`D-97`/`D-101`）**：恢复旧句柄仅发生在 `_exit_tree() -> Release()` 或再次 `Apply()` 替换旧句柄时，运行期维持双路径自定义图标不变。 |
| **P1-6** | CI 必挂 | `.github/workflows/custom-templates.yml:38, 105-109, 268` | `build-windows` 第 38 行仅安装 `scons` 未安装 `pillow`，执行 `sync_release_icons.py` 会直接 `SystemExit(2)`；`build-ios` 第 268 行在 `macos-latest` 直接 `pip install pillow` 未使用 venv，在 PEP 668 环境下必然失败。 | Windows 第 38 行改为安装 `scons pillow`；iOS 第 268 行复用 `ios-unsigned.yml:38-42` 的独立 venv 安装模式并导出 `PYTHON_BIN`。 |
| **P1-7** | 密钥安全 | `tools/release_tui.py:115-120, 537-539, 801-802, 1081-1082, 1580`<br>`tools/mobile/github_ios_ci.py:189-193` | `save_config()` 将 Android Keystore 密码明文持久化到 `work/out/release_tui_config.json`，并在 CLI 计划与终端命令回显中完整打印；`github_token.txt` 写入未收紧文件权限。 | 移除 JSON 对签名密码的持久化（改用环境变量或运行时 `getpass.getpass` 传入）；在 `format_cmd()` 中对 `--ks-pass`/`--key-pass`/`--token` 后一参数统一脱敏为 `***`；Unix 下对 `github_token.txt` 设置 `0o600`。 |

---

## 三、P2 缺陷与健壮性修复清单（10 项）

### 1. 表现层与启动画面

- **P2-1：标准选项框与底部对话框碰撞兜底（兼容 `MOB-04` 与 `D-70`）**
  - **位置**：`godot/presentation_gd/ChoiceWindow.gd:76-86`、`godot/presentation_gd/GameRoot.gd`
  - **问题**：保持 `MOB-04` 的 `center.y = 288.0` 居中规则不变，但在 `D-70`（`101→102` 提示语与选项同屏）且触控 `36px` 行高遇到 7–8 项长选项时，选项框下沿会向下侵入底部 `MessageWindow`（`y = 408..576`）。
  - **修复**：仅在检测到 `MessageWindow` 处于底部打开态时，将 `ChoiceWindow` 的 `max_y` 收紧至对话框上缘之上（`message.bubble_rect().position.y - 8.0 - win_size.y`），无底部对话框时 100% 维持 `center.y = 288.0`。
- **P2-2：大跨度延迟范围命令漏掉 `_pending_pictures` 中的槽位（强化 `D-102`/`D-104`）**
  - **位置**：`godot/presentation_gd/PictureStage.gd:811-825, 1382`
  - **问题**：`_slots_in_span(lo, hi)` 在跨度 `> 256` 时只扫描 `_cmds`/`_pictures`/`_states`。若槽位 `500` 先排入延迟 `Show(delay=5)`、随后排入延迟范围 `Erase 1..1000(delay=10)`，入队时 `500` 尚未进入 `_cmds`，会被遗漏导致图片残留。
  - **修复**：在 `_slots_in_span` 的 `> 256` 分支补充扫描 `_pending_pictures` 中 `lo <= entry["number"] <= hi` 的槽位。
- **P2-3：旧版 `user://hotkeys.cfg` 与 `D-91` 重排热键冲突**
  - **位置**：`godot/presentation_gd/GameRoot.gd:25-37, 2030-2065`
  - **问题**：`F4`–`F10` 重排后，老用户已有 `hotkeys.cfg`（如 `backlog=F4`、`quick_save=F5`）会与默认 `window_size=F4`/`frame_rate=F5` 键位重叠，且新键判断在前导致旧配置被静默吞掉。
  - **修复**：为 `hotkeys.cfg` 增加 `version=2` 标记或键位去重检查（加载自定义键时清除与其冲突的默认绑定，或自动迁移 v1 键位）。
- **P2-4：`BootSplashOverlay` 单缺 `logo.jpg` 错配与后缀/大小写限制（强化 `D-90`）**
  - **位置**：`godot/presentation_gd/BootSplashOverlay.gd:145-162, 177-178`
  - **问题**：若 `logo.jpg` 缺失而 `logo2.jpg` 存在，`base_tex` 回退 `splash_static.png` 但未清空 `progress_tex`，导致 `logo2.jpg` 擦除叠在静态图上；且松散目录与 pak 直读仅查 `.jpg`（未兼容 `.png`/`.webp` 或大小写变体）。
  - **修复**：回退 `STATIC_PATH` 时同步置 `progress_tex = null`；候选路径按 `logo.jpg`/`.png`/`.webp` 顺序探测。

### 2. C# 存档与系统变量

- **P2-5：`SyncPersistentSystemVarsFromDisk` 漏合 `System.json` 中的 `var[503]/[626]` 且未隔离脚本模式**
  - **位置**：`godot/core_cs/GameSession.cs:2010-2035, 2121`
  - **问题**：`SaveGameToSlot`（`:856`）与 `ImportRetailSaveToSlot`（`:1515-1517`）均把 `var[503]`/`var[626]` 写入 `System.json`，但 `sys1MergeVars` 遗漏了这两项，导致无 `system2.json` 时新游戏丢失周目解锁；且 `StartNewGameNow()` 未加 `!ScriptedInput` 守卫，本地残留 `System.json` 可能污染脚本对拍。
  - **修复**：将 `503`、`626` 加入 `sys1MergeVars`，并在 `StartNewGameNow()` 改为 `if (!ScriptedInput) SyncPersistentSystemVarsFromDisk();`。
- **P2-6：`ImportRetailSystemSaveTo` 落盘前缺少归一化与校验**
  - **位置**：`godot/core_cs/GameSession.cs:1569-1596`
  - **问题**：构造 `SaveState` 后直接 `SaveCodec.Write`，未走 `SaveMigrator.Normalize` 与 `ValidateSaveJson`，异常数据会写出后续无法读回的损坏 JSON。
  - **修复**：落盘前先调用 `SaveMigrator.Normalize(state)` 并通过 `ValidateSaveJson(...)` 校验，失败返回 `false`。
- **P2-7：`ImportRetailSaveToSlot` 清洗瞬态图前先截断 `MaxPictures` & `SavedPicture.Flags` 缺校验**
  - **位置**：`godot/core_cs/GameSession.cs:1477-1492`、`godot/core_cs/SaveCodec.cs:462-472`
  - **问题**：导入原版存档时先 `.Take(SaveCodec.MaxPictures)` 再清 `15000..16999` 瞬态窗口可能挤占正常立绘名额；`SaveCodec.Validate()` 未对新增 `SavedPicture.Flags` 校验键数量与键名长度。
  - **修复**：先按 `SaveMigrator.IsTransientUiPicture` / `SanitizeRestoredPictures` 清洗再 `.Take(MaxPictures)`；在 `SaveCodec.Validate()` 中限定 `Flags.Count <= 16`、键名非空且长度 `<= 32`（或白名单限定 `"blend"`/`"scale_mode"`）。

### 3. 工具链与 CI

- **P2-8：`run_godot_test.py` 队列尾行未跑致命日志检测**
  - **位置**：`tools/run_godot_test.py:215-245`
  - **问题**：进程退出后 drain 残留输出行未匹配 `FATAL_LINE_PATTERNS`，可能漏报以退出码 `0` 结束的末尾 GDScript 报错。
  - **修复**：将 `FATAL_LINE_PATTERNS` 检测抽为公共辅助函数，在 drain 阶段同样执行。
- **P2-9：移动端打包临时目录碰撞、iOS 漏删 `.slnx` 与占位图标开关**
  - **位置**：`tools/package_android.sh:257, 272`、`tools/package_ios.sh:149, 160-163, 178-182`、`tools/release_tui.py:1317-1318`
  - **问题**：① Android/iOS 共用 `work/out/release_icons` 并在 `cleanup()` 中 `rm -rf` 易互踩；② `package_ios.sh` 漏删 `godot/Lwsmf.slnx` 且无条件传 `--allow-placeholder`；③ `release_tui.py --verify` 硬编码目录名，忽略 `-hd` 后缀。
  - **修复**：改用带 `$$` 的独立临时图标目录；`package_ios.sh` 补删 `Lwsmf.slnx` 并改为显式 `--allow-placeholder` 开关（**护栏**：同步在 `.github/workflows/ios-unsigned.yml` 与 `custom-templates.yml` 的 iOS 步传入 `--allow-placeholder`）；`release_tui.py` 按 `cfg.include_hd` 拼接 `-hd`。
- **P2-10：`github_ios_ci.py` 静默回退旧版 IPA & `ci.yml` 语法检查遗漏**
  - **位置**：`tools/mobile/github_ios_ci.py:390-408`、`.github/workflows/ci.yml:45`
  - **问题**：`--fetch-latest` 版本不匹配会静默下载旧版壳且无告警；`ci.yml:45` 的 `py_compile` 漏掉了 `tools/release_tui.py`、`tools/mobile/github_ios_ci.py` 与 `tools/run_godot_test.py`。
  - **修复**：版本不匹配回退时打印明确 `WARN`（或默认要求精确匹配）；将 3 个新 Python 工具补入 `ci.yml:45`。

---

## 四、P3 优化细节清单（5 项，按需顺手收口）

1. **冷启动双重解码启动图（`BootSplashOverlay.gd:55` + `GameRoot.gd:158-159`）**：`load_retail_textures()` 若已成功解码真实 `logo.jpg`/`logo2.jpg`，在 `_start_game()` 中直接跳过二次解码，避免将一次性启动图常驻进 `AssetDB` LRU。
2. **`ChoiceWindow` 接入 `TextStyleEngine` 后丢弃 `outline`（`ChoiceWindow.gd:206, 215-216, 304`）**：在 `_append_run` 保留 `outline` 字段并按条件调用 `draw_string_outline`，使选项中的 `\N` 与 `MessageWindow` 行为一致。
3. **原版 `.sav` 导出补齐 `MapEventSelfVars`（`LegacySaveExporter.cs:276`）**：`WritePart3` 按 `state.MapId` 与 `state.SelfVars` 写出对应 `groupCount` 组，完善 `.sav -> JSON -> .sav` 往返无损度。
4. **`WindowIcons` 缩放宽高与 32 位兼容（`WindowIcons.cs:168, 230-231`）**：将 `preferredWidth`/`preferredHeight` 传入 `CreateIconFromResourceEx`，并按 `IntPtr.Size == 8` 分发 `SetClassLongPtrW` / `SetClassLongW`。
5. **工具链微瑕（`pack_media.py:313-316`、`lwsmf.iss.tmpl:84-90`、`github_ios_ci.py:6-8`）**：`--lossless` 缺 Pillow 时显式报错/告警而非静默原样拷贝；欢迎页同步注入真实海报宽高比；对齐 `github_ios_ci.py` 环境变量与 `github_token.txt` 的优先级注释。

---

## 五、回归验证门禁与真机验收清单

1. **构建与自动化门禁**：
   - `dotnet build godot/Lwsmf.csproj -c Release` 与 `-c Debug`：0 warnings, 0 errors。
   - `bash tools/godot_check.sh` 与 `LWsmf_RENDER=1 bash tools/godot_check.sh`：全量门禁 + 149,655 / 989,439 步 Oracle 100% 通过（确保 `DialogDrawTest`、`GameRootTest` 的 `MOB-04` 与 `ColdStartDayTest` 零回归）。
   - `python -m unittest tools/mobile/test_mobile_tools.py`、`python -m py_compile ...`、`bash -n tools/package_*.sh` 全绿。
   - 补充单测：`LegacySaveImporter` 畸形超大 `mds1Count` 抛 `InvalidDataException` 不 OOM；`op 221` `Sys24` 面对损坏 JSON 返回 `0`；`apply_save_zip` 规范化 `system.json`/`System2.json`；`_slots_in_span` 覆盖 `_pending_pictures`。
2. **三端实机覆验（`needs-runtime`）**：
   - Windows / Android / iOS 冷启动 `logo.jpg` → `logo2.jpg` 水平揭示期间按键/触屏不穿透，揭示完成后标题菜单正常响应；
   - Windows 退出游戏无异常图标闪烁或崩溃；
   - GitHub Actions 触发 `custom-templates.yml` 与 `ios-unsigned.yml` 确认 Pillow/venv 与 `--allow-placeholder` 均正常通过。
