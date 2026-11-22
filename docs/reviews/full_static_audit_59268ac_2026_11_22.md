# LWSMF 全量静态代码审查报告（Commit `59268ac`）

**审查基线**：`Living-With-SIster-Monochrome-Fantasy` commit `59268ac97735d7f4d7e7a471c8b53106b5845951`（对比前序基线 `2c57740`）  
**审查规模**：全仓库 `godot/`（32,083 行 C# / GDScript / 测试）、`.github/workflows/`、`tools/`（含新增 `lint_decisions.py`、`visual_regression.py` 及移动端工具）、`crates/wolf-format/tests/tileset.rs`  
**审查性质**：100% 源码级静态审计；需在 Godot 4.7.2 / .NET 8/9 / Android 实机复核的验证项均标注 `needs-runtime`

---

## 1. 执行摘要与前序死锁项（D-01～D-08）闭环核验

本轮代码相对 `2c57740` 在核心死锁治理、存档迁移（`SaveMigrator.cs`）、Android 首次引导（`MobileDataBootstrap.gd`）和工程门禁（`lint_decisions.py`、`visual_regression.py`）上推进显著。但全量静态扫描仍发现 **4 项 P0、24 项 P1、26 项 P2、13 项 P3** 缺陷，主要集中在：发布/CI 脚本假绿或误报、`.sav` 二进制解析边界、读档与切图的状态复位残余缺口、移动端落盘原子性与中文字体绑定、以及部分 C#/GDScript 跨语言与并发边界。

### 前序死锁排查项（`DEADLOCK_AND_STALL_AUDIT_2026_11_22.md`）闭环表

| 编号 | 闭环状态 | `59268ac` 核验位置 | 残余缺口（已归入下方缺陷清单） |
| :--- | :--- | :--- | :--- |
| **D-01** | **主体闭环** | `MapRuntime.cs:343, 369` 已补 `inst.IsParallel = page.Trigger == MapTrigger.Parallel;` | 读档恢复公共事件时 `GameSession.cs:1201-1238` 恒置 `isParallel = false`，且地图实例漏设 `RetainAfterFinish = true`（见 **P1-3**） |
| **D-02** | **主体闭环** | `InputRouter.gd:246-252, 262`、`GameRoot.gd:302-303` 已在帧首/帧末同步释放 | 若鼠标松开被带 `MOUSE_FILTER_STOP` 的 GUI 吞掉，`pointer_pressed` 缺物理按键兜底（见 **P2-14**） |
| **D-03** | **主体闭环** | `GameSession.cs:993-994`、`VmController.cs:993-998` 已清输入权限与 `_reservedEvents` | `ApplyLoadedState` 未清 `_teleports`/`_pending*`；`EnterMap`（`:1280`）仍直调 `_vm.Vm.StopAll()`；`VmController.StopAll` 未清对话状态与 `_pictures`（见 **P1-4 / P1-9**） |
| **D-04** | **主体闭环** | `VmController.cs:344-369` 已新增 `HasWaitingInstance` 实况校验 | “先 `SelectChoice`/`CancelChoice` 后协程被杀”残留 `_pendingChoiceReply`；`IsWaitingForMessage` 清理漏复位 `_messageDismissed` 与 `MessageKeptForChoice`（见 **P1-9**） |
| **D-05** | **完全闭环** | `BattleWiring.cs:62-74, 140-146, 215-227`、`GameSession.cs:226, 996, 1085, 1164` | 仅残留未读死字段 `_roundLoop`（见 **P3-2**） |
| **D-06** | **部分闭环** | `EventVm.cs:1728, 1742-1750` 已设单实例 50,000 步限额并输出告警 | 仍按固定下标 `0..Count-1` 遍历，前 4 个异常协程即可耗尽 200,000 总预算（见 **P2-1**） |
| **D-07** | **主体闭环** | `EventVm.cs:1657, 1674, 1738, 1779` 已修正短路顺序并清理回收 | `case 174`/`175` 直调 `Instances.Clear()` 未同步清 `_enteredInstanceIds`/`_freshInstanceIds`（见 **P2-7**） |
| **D-08** | **完全闭环** | `PlayOverlays.gd:586-603`、`GameRoot.gd:230-237` 已在 F3 面板输出 `Deny/Guard/v295` | 无缺口 |

---

## 2. P0 级阻断缺陷（必须立即修复，共 4 项）

### P0-1：`tools/package_release.sh:181-198` — `media.pak` 内置字体探测解析了错误的二进制格式
* **根因链路**：`tools/pack_media.py:5-10, 95-115` 定义的 `WOLFMPK1` 线格式为 32 字节头（`<8sHHIQQ`）+ 位于 `index_offset` 的 splitmix64 XOR 加密索引；但 `package_release.sh:188-196` 直接从偏移 `8` 读 4 字节作为 `count`（实际是 `ver=1, flags=0`），跳过 4 字节后按明文解包 `nlen`，必然读出乱码或抛出 `struct.error`，导致仅依赖 `media.pak` 内置 `Lolita.ttf` 时打包误报缺字体（`exit 3`）或崩溃。
* **最小修复**：删除自写二进制解析，直接复用 `tools/pack_media.py` 的 `read_pak_index(Path(sys.argv[1]))` 检查 `any(n.endswith("lolita.ttf") for n in names)`，并对传参统一包一层 `to_win_path`。

### P0-2：`.github/workflows/ci.yml:32-36` — Windows `pwsh` 多行 `run:` 会吞掉 `lint_decisions.py` 的失败退出码
* **根因链路**：`dotnet-check` 运行于 `windows-latest`（默认 shell 为 PowerShell），`$PSNativeCommandUseErrorActionPreference` 默认为 `false`，步骤成功与否仅由最后一条外部命令的 `$LASTEXITCODE` 决定。若 `python tools/lint_decisions.py` 返回非 0 而后续 `python tools/release_audit.py --repo-scan` 返回 0，CI 仍报绿。
* **最小修复**：拆分为两个独立 `run:` 步骤，或显式设置 `shell: bash` 并开启 `set -euo pipefail`。

### P0-3：`godot/core_cs/LegacySaveImporter.cs:451, 459-470` — 畸形 `.sav` 可触发 ~4 GB OOM 与 100% CPU 永久死循环
* **根因链路**：`VariableDatabase` 解析直接执行 `uint[] cfg = new uint[cnt > 0 ? cnt : 0]`（`:451`），未校验 `cnt * 4 <= r.Remaining`，单字节篡改即可触发高达 ~4 GB 分配；当 `fc >= 0x8000_0000` 时 `cnt = (int)fc` 溢出为负数导致 `cfg.Length == 0`；随后 `for (int row = 0; row < typeRowCount; row++)`（`:459`）在 `cfg.Length == 0` 且 `typeRowCount > int.MaxValue` 时每轮读 0 字节且 `int row` 在 `int.MaxValue` 后回绕为 `int.MinValue`，形成永久死循环。
* **最小修复**：对 `cnt` 与 `typeRowCount` 增加上限及剩余字节校验（`if ((long)cnt * 4 > r.Remaining) throw new InvalidDataException(...)`），禁止 `fc > int.MaxValue`，循环变量改为 `uint` 并在 `cfg.Length == 0 && typeRowCount > 0` 时拒绝或跳过。

### P0-4：`godot/core_cs/LegacySaveImporter.cs:71-91, 210, 243-244` — `ReadBytes` 32 位整数溢出绕过 EOF 检查、地图格数乘法溢出及无上限文件读取
* **根因链路**：`SaveReader.ReadBytes(int count)`（`:73`）用 `Position + count > Buffer.Length` 判界，在 `unchecked` `int` 下当 `Position > 0` 且 `count` 接近 `int.MaxValue`（来自 `:87` `(int)ReadU32()`）时加法溢出为负数，绕过 EOF 检查并触发 2 GB 分配；`:243` `long cells = (long)mapWidth * mapHeight * 3` 可溢出有符号 `long` 静默跳过地图数据；`:210` `File.ReadAllBytes(path)` 无文件体积上限。
* **最小修复**：`:73` 改为 `if (count < 0 || count > Buffer.Length - Position)`；`:210` 先用 `FileInfo.Length` 校验不超过 `SaveCodec.MaxSaveBytes`；`:243` 改用 `checked` / `ulong` 校验 `cells * 4 <= (ulong)r.Remaining`。

---

## 3. P1 级高危缺陷（共 24 项，按子系统分组）

### 3.1 C# 核心引擎层（`godot/core_cs/`，6 项）

| 编号 | 位置（`file:line`） | 根因与影响 | 最小修复 |
| :--- | :--- | :--- | :--- |
| **P1-1** | `EventVm.cs:111-118` | `FilterKeyCode` 对 `DeviceDeny` 直接用 Host 位 `bits & AutoKeys`（WOLF 位图）相与，导致 125 方向键自动输入被误杀，且物理 `Up` 可反绕 `DeviceDeny` | `:116` 改为 `(bits & KeyCodes.FromWolfBits(AutoKeys)) == 0` |
| **P1-2** | `EventVm.cs:874-884, 1593-1609` | 实时模式（`BlockOnKeyWait=true`）下阻塞式 `op123` 遇到 125 自动键仍设 `requireRelease=true` 并等待归零，2 帧 TTL 到期后自动键被白吞、协程卡在 `KeyInput` | `:879` 改为 `if (blocking && !(autoConsumed && key != 0))` 才阻塞，否则直接交付 |
| **P1-3** | `GameSession.cs:1201-1238` | `RestoreInstances` 对非地图事件恒置 `isParallel=false`（读档后重现 D-01），对地图事件未置 `RetainAfterFinish=true`（结束后被立即回收，丢失 `SelfVars` 写回） | `SavedInstance` 持久化 `IsParallel`（兼容旧档按 `RunCondition==2`/父实例推导）；地图恢复实例设 `RetainAfterFinish=true` |
| **P1-4** | `GameSession.cs:213-239, 991-1039, 1159-1187, 1277-1287` | `StartTitle`/`StartNewGameNow` 未调 `ResetSessionState()`，前一周目 `op250` 数据库覆盖层与变量泄漏到新游戏；`ApplyLoadedState` 未清 `_teleports`/`_pending*`/`_actionPressed`；`EnterMap:1280` 仍直调 `_vm.Vm.StopAll()` 绕过预约队列与对话复位 | 统一清理挂起队列；新游戏/回标题按边界重置 DB 覆盖层；`EnterMap:1280` 改调 `_vm.StopAll()` + `_vm.ResetDialogueState()` |
| **P1-5** | `EventVm.cs:420, 442`、`GameDatabase.cs:334-335` | `int.MinValue % -1`（EventVm）与 `int.MinValue / -1`、`int.MinValue % -1`（GameDatabase）即使在 `unchecked` 中仍抛 `OverflowException`；`GameDatabase` 除零落入 `_ => operand` 覆写为 `0` | 补充 `int.MinValue` 与 `-1` 边界判断；`GameDatabase` 除零保持原值 `current` |
| **P1-6** | `AutoEvents.cs:108, 118` | `LoopVisualPainters == false` 时非 `ResumeOnLoad` 事件缺少 `!alive` 守卫，`BattleWiring` 已启动 `CE242`/`CE243` 后条件翻转会重复起第二个实例 | `:118` 改为 `shouldStart = !alive && (ev.ResumeOnLoad ? now : (now && !before));` |

### 3.2 存档序列化与 C# 表现桥接层（`SaveCodec` / `SaveMigrator` / `godot/presentation/`，7 项）

| 编号 | 位置（`file:line`） | 根因与影响 | 最小修复 |
| :--- | :--- | :--- | :--- |
| **P1-7** | `SaveCodec.cs:99, 274-293, 307-328` | 损坏 JSON 含 `"strings": null` 或数组含 `null` 元素（`audio`/`pictures`/`instances`/`map_events`）时，`Normalize`/`IsValid` 抛未捕获 `NullReferenceException`，击穿 `.bak` 自动回滚并阻断 `Write` 保存 | `EventSelfVars.IsValid` 判空 `Strings`；`Normalize`/`Validate` 对集合元素判空或直接拒绝 |
| **P1-8** | `SaveMigrator.cs:28-51`、`SaveCodec.cs:281, 374, 513-521, 573-583` | `SaveMigrator.Normalize` 在 `FromJson`（含 `Validate`）**之后**执行，导致 `SchemaVersion < 1` 迁移分支成死代码、含瞬态 UI 图致 `Pictures.Count > 256` 的旧档被提前拒收，且 `FromJson`/`FromBytes` 绕过迁移 | 将 `SaveMigrator.Normalize` 移入 `FromJson` 内部（`SaveCodec.Normalize` 之后、`Validate` 之前），删除 `ReadWithSource` 重复调用 |
| **P1-9** | `VmController.cs:354-369, 993-998` | D-03/D-04 残余：`StopAll()` 未清对话状态与 `_pictures`；`SelectChoice`/`CancelChoice` 后协程被杀时因 `_isWaitingChoice` 已为 `false`，`_pendingChoiceReply` 永久残留被下一选项误消费；`IsWaitingForMessage` 清理未复位 `_messageDismissed` 与 `MessageKeptForChoice` | `StopAll()` 调用 `ResetDialogueState()` + `_pictures.Clear()`；无对应等待协程时无条件清空 `_pendingChoiceReply`、`_currentChoices`、`_messageDismissed=true`、`MessageKeptForChoice=false` |
| **P1-10** | `VmController.cs:957-963` | `KeyCodeForPoll` 在设备轮询（`((mask >> 8) & 0x0F) != 0`，如 `mask==771`）时未过滤 `autoKeys`，与 `KeyCodes.AcceptsWolfBits` 契约冲突 | 仅在 `KeyCodes.AcceptsWolfBits(mask, autoKeys)` 为真时混入 `FromWolfBits(autoKeys)` |
| **P1-11** | `TerrainBridge.cs:20-25` | `LoadForMap` 允许 `map == null`，但 `:22` 执行 `tiles?.BuildTerrain(map!)`，而 `BuildTerrain` 首行访问 `map.Id` 必抛 `NullReferenceException` | `:22` 改为 `(map != null && tiles != null) ? tiles.BuildTerrain(map) : null` |
| **P1-12** | `MediaPakReader.cs:21, 50, 65-171, 187-242` | 继承 `RefCounted` 持有 `FileStream` 但未重写 `Dispose(bool)`，释放对象时泄漏 FD/锁文件；`_entries`/`LastError` 多处读写在 `lock (_gate)` 之外存在并发竞态 | 重写 `Dispose(bool)` 调用 `Close()`；将 `_entries`/`PackPath`/`LastError`/`_file` 全部读写纳入 `lock (_gate)` |
| **P1-13** | `GameSessionController.cs:124-125, 181-182, 185-268`、`VmController.cs:968-990, 1050-1069`、`AppPaths.cs:15-31` | 桥接层 4 处边界缺陷：(a) `QuickSavePath()` 忽略自定义 `_savePath`；(b) `CallableView` 14 处 `_host.Call` 未验 `IsInstanceValid` 且字典下标缺键直崩存档；(c) `SetKeyStateProvider` 裸存 `Callable` 且 `(int)mask` 高位截断为负；(d) `VmController.FileExists` 剥离 `../` 并扁平化子目录破坏路径语义；`AppPaths.ResourceRoot()` 在 Android 返回 `"res://"` 与 `System.IO` 契约冲突 | `QuickSavePath` 优先用 `_savePath`；`CallableView` 加存活判空与 `TryGetValue`；键状态桥接改 `(GodotObject, StringName)` + `(long)`；`FileExists` 拒绝 `..` 并用 `GetFullPath` 限域；Android 下区分 `res://` 与 OS 路径 |

### 3.3 GDScript 表现层（`godot/presentation_gd/`，7 项）

| 编号 | 位置（`file:line`） | 根因与影响 | 最小修复 |
| :--- | :--- | :--- | :--- |
| **P1-14** | `PlayOverlays.gd:422-428`、`GameRoot.gd:943, 1347-1359` | FaultModal 点击“关闭”仅隐藏 `_fault_root`，而 `faulted == true` 已停摆 `_process()` 并屏蔽除 F8/F11 外所有热键，画面永久软锁死 | 移除关闭按钮，或在 `GameRoot.gd:943` 允许 `ESC`/`F12` 重新唤出 FaultModal |
| **P1-15** | `MobileDataBootstrap.gd:316-328` | `mobile_data_manifest.json` 与 `.staging_version` 非原子写入且未检查 `store_string()` 返回值，空间不足写出 0 字节后仍触发重载，形成首启死循环 | 改为 `.part` 写入 + 校验 `store_string() == true` + `close()` + `rename_absolute` |
| **P1-16** | `MobileDataBootstrap.gd:86, 313` | `actual != int(entry.get("bytes", actual))` 在清单缺 `"bytes"` 字段时恒为 `false`，损坏/截断文件静默通过校验 | 先验 `entry.has("bytes") and int(entry["bytes"]) >= 0`，再比对 `actual` |
| **P1-17** | `MobileDataBootstrap.gd:149-177`、`SetupWizard.gd:150-209`、`PlayOverlays.gd:57-70, 275-299, 389-428, 493-496, 558-562` | `project.godot` 未设全局 CJK 字体，移动端首启引导页、F12 确认框、FaultModal、Toast、F2/F3 浮层均用默认无 CJK 字体，Android 实机易全屏豆腐块 | 为 `PlayOverlays`/`SetupWizard` 根节点绑定 `UiSkin.font()` 主题；`MobileDataBootstrap` 绑定系统 CJK 兜底字体 |
| **P1-18** | `PictureStage.gd:988-996` | R12 回归：`_begin_draw` 在 `scale_mode == 3`（独立缩放）下仅保持 `scale_x == UNCHANGED` 时，`:991` 误把显式传入的新 `scale_y` 覆盖回旧值 | 仅在 `scale_mode != 3` 时将 `args[7] == UNCHANGED` 联动应用到 `args[16]` |
| **P1-19** | `PictureStage.gd:1399-1423` | `picture_snapshots()` 丢失 `_begin_move` 改写的 `tone` 与全部 `op290` 特效；`restore_pictures()` 回放原 `args[0]` 导致读档后全屏图片误重播淡入 | 快照持久化 `tone` 与持久型 290 状态；回放前清零 `args[0]` |
| **P1-20** | `GameRoot.gd:617-630, 657-671` | `op161` 冻结转场置 `_transition_freeze_rect`（`z_index=60`）且 `_transition_frames=0`，若脚本提前返回未执行 `op162`，画面永久定格；且未复用 `_grab_viewport_image()` 的 Headless 防护 | 复用 `_grab_viewport_image()`；切图或前台事件全部结束时清理悬挂冻结层 |

### 3.4 工程工具链与打包脚本（`tools/`，4 项）

| 编号 | 位置（`file:line`） | 根因与影响 | 最小修复 |
| :--- | :--- | :--- | :--- |
| **P1-21** | `tools/godot_check.sh:246-250, 313-314` | 并行任务池把命令压成空格串再 `read -r -a` 拆词，仓库路径 `$IMO` 含空格时全部并行 Godot 任务写日志失败 | 改用 `IFS=$'\x1f'` 拼接与还原参数数组 |
| **P1-22** | `tools/release_audit.py:193-196, 210-217` | `:195` `or "\\" in raw` 跳过全部反斜杠静态资源（漏报）；`:215` Windows 下 `Path.with_suffix("")` 输出反斜杠与 POSIX `pak_paths` 永不匹配（误报）；`:210` `lstrip("./")` 使 `:211` `../` 分支成死代码 | 用正则仅跳 WOLF 控制码；`:215` 用 `.as_posix()`；`:210` 用 `re.sub(r"^(?:\.\./|\./)+", "", ...)` 后统一过滤 `save/` |
| **P1-23** | `tools/package_android.sh:183`、`tools/package_release.sh:181, 207, 256` | Android 预构建未传 `-p:GodotTargetPlatform=android`（错编桌面 `net8.0` 且未用 `ExportRelease`）；`package_release.sh` 仍有 3 处 Windows Python 路径调用漏包 `to_win_path` | `package_android.sh` 传 `-c ExportRelease/ExportDebug -p:GodotTargetPlatform=android`；补齐 3 处 `to_win_path` |
| **P1-24** | `tools/mobile/env_check.py:315, 320`、`tools/visual_regression.py:51-61, 132-136, 140-142, 271` | `env_check.py --strict` 对缺失 Godot 及非 Mono 模板仅报 `WARN` 放行；`visual_regression.py` 不清旧图、无视 `assets.misses > 0`、不验 `save_png()` 返回值且缺数据时把向导页截成 `01_title_menu.png` 报绿 | `env_check.py` 将两项升级为 `MISSING` + `blocker=True`；`visual_regression.py` 跑前清旧图、断言 `phase` 非 `setup`/`failed`、`save_png()==OK`、`misses==0` |

---

## 4. P2 级中危缺陷汇总（共 26 项）

| 编号 | 位置（`file:line`） | 核心问题 | 最小修复 |
| :--- | :--- | :--- | :--- |
| **P2-1** | `EventVm.cs:1529, 1728-1753` | D-06 残余：总预算 `200_000` 固定从 `i=0` 遍历，前 4 个异常协程即可饿死后续协程 | 引入帧级轮转起始游标 |
| **P2-2** | `EventVm.cs:475-480, 586-588, 1134, 1156, 1179, 1571, 1717, 1723` | 多处 `slot.Params` 与 `prog.Blocks[idx]` 缺少长度/范围校验，畸形字节码或损坏存档可抛越界异常 | 统一先校验 `Params.Length` 与 `0 <= idx < Blocks.Count` |
| **P2-3** | `EventVm.cs:318-327, 1184-1193` | `strCount > 4` 时分配 `string[strCount]` 但仅填前 4 项，残留 `null` 写入 `SelfVars` 致后续 `op122` NRE | 将 `strCount` 钳位至 `<= 4` 并在 `SelfVars.SetStr` 使用 `value ?? ""` |
| **P2-4** | `EventVm.cs:649-662` | `op213` 标签跳转跳出循环时未弹出 `inst.Loops` 栈，导致栈增长与后续 `171`/`176` 误跳 | 跳转后弹出所有不再覆盖目标 `Pc` 的循环帧 |
| **P2-5** | `GameSession.cs:785, 841-846` | `case 222` 写盘无 `try/catch` 异常穿透帧循环；`ResolveSaveTarget` 未先将 `\` 转 `/` 导致非 Windows 路径错位 | 包裹 `try/catch`；先 `Replace('\\', '/')` 再 `Path.GetFileName` |
| **P2-6** | `GameSession.cs:893-940` | `ReadSaveAddress` 对 `CData` 地址读实时数据库而非入参 `SaveState`，跨槽位探测读串 | 明确标注或待存档持久化 `CData` 覆盖层后改读 `state` |
| **P2-7** | `EventVm.cs:128-133, 1418-1426, 1669-1676` | `StopAll()` 未复位 `_transition*` 过渡态；`case 174`/`175` 直调 `Instances.Clear()` 未清 `_enteredInstanceIds`/`_freshInstanceIds`/`_autoKeysDelivered` | 在 `StopAll()` 复位过渡态；`174`/`175` 同步清空三集合 |
| **P2-8** | `ProgramLoader.cs:22, 42-47`、`MapTileData.cs:46, 120-123, 143, 147` | 缓存刷新覆盖旧 `JsonDocument` 未 `Dispose`；`MapTileData.Load` 静默吞异常致全图穿墙；`BuildTerrain` 用列表下标而非 `t.Index` 查找图块集 | 覆盖前 `Dispose`；`catch` 输出告警；按 `t.Index == entry.TilesetId` 查找 |
| **P2-9** | `WolfRng.cs:98-111` | `int diff = max - min` 在宽区间下发生 32 位有符号溢出，大幅压缩随机范围 | 先转 `long`/`ulong` 计算 `span` 后取模 |
| **P2-10** | `SaveCodec.cs:351-362` | 协程环检测在 4096 实例链下退化为 $O(N^3)$，恶意存档可冻结主线程 | 先建 `Dictionary<int, SavedInstance>` 并记忆已验节点降为 $O(N)$ |
| **P2-11** | `SaveCodec.cs:310, 369, 378, 445-448` | `Validate` 漏检 `WaitKey` 字段、悬空 `WaitChild`/`MapEvent.InstanceId` 与负 `pic.Number`；`audio.Tempo <= 0` 把负值静默洗白为 `100` | 补齐引用存在性与范围校验；仅将 `Tempo == 0` 归一为 `100` |
| **P2-12** | `SaveCodec.cs:468-477, 573-583` | `.bak` 轮转未捕获 `UnauthorizedAccessException`；`FromBytes` 缺少 `MaxSaveBytes` 上限 | 捕获 `IOException or UnauthorizedAccessException`；`FromBytes` 首行限长 |
| **P2-13** | `VmController.cs:246, 596-599, 793-815, 856-946, 1005-1025`、`MediaPakReader.cs:131-153, 208, 266`、`GameSessionController.cs:73-79, 96-121`、`TerrainBridge.cs:32-38`、`LegacySaveImporter.cs:423, 599-605` | `Picture` 范围删在 `hi==int.MaxValue` 死循环；左键 TTL 松开后仍报 `2` 且同帧重复发 `PointerPressObserved`；`ResetPointer` 未清 `_wheelDelta`；`MediaPakReader.Open` 失败残留半解析索引与 2 GB 单条上限；`GameSessionController.Rebuild` 未清 `LastError` 且每帧重复弹故障框；`TerrainBridge` 空地形时 `Mask` 与 `IsBlocked` 语义相反；`LegacySaveImporter` 缺 `picName` 路径校验且非法 UTF-8 `CharLen` 误吞 4 字节 | 范围删改用 `long`；限制首帧边沿通知与清 `_wheelDelta`；`Open` 失败清 `_entries` 并收紧上限；`Rebuild` 清 `LastError` + 单次故障上报；统一空地形阻挡语义；补路径与 UTF-8 步长校验 |
| **P2-14** | `InputRouter.gd:238-243` | D-02 残余：鼠标松开被 GUI 吞掉时 `pointer_pressed` 滞留 `true`，阻断 `_action_entry_suppressed` 释放 | 非 headless 且非脚本模式时结合 `Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT)` 纠偏 |
| **P2-15** | `PictureStage.gd:1344-1345` | R12 回归：`dump_pictures()` 从 `st` 顶层而非 `st["current"]` 读 `blend`/`anchor`，诊断输出恒为 `0` | 提前提取 `cur` 后改读 `cur.get("blend", 0)` 与 `cur.get("anchor", ...)` |
| **P2-16** | `GameRoot.gd:250-262` | `_consume_picture_queue()` 未做 `p is Dictionary` 与 `.get()` 防御，畸形项直抛运行时错 | 补类型检查与 `.get()` 默认值 |
| **P2-17** | `GameRoot.gd:248-249, 694-706, 721-725` | `_clean_scene_image` 未随系统 UI 关闭释放；`save_jpg` 前未确保 `SaveDirectory` 存在 | 写图前 `make_dir_recursive_absolute`；系统 UI 关闭时置空 `_clean_scene_image` |
| **P2-18** | `AudioManager.gd:125, 281-297` | `set_master_volumes()` 强杀淡入 Tween 致爆音、用末条 SE 音量覆盖全部 SE 声部且漏 `_se_pending`；单条 SE 缺文件误杀全部 6 路并发 SE | 记录各声部独立音量；缺文件仅 `return false`；淡入中平滑更新目标 |
| **P2-19** | `PictureEffects.gd:90-103, 170-178`、`PictureStage.gd:708-711, 1266-1269` | `zero_tone_hidden` 槽位有限 FLASH 结束后永久显影；Breath 负幅未夹紧产生负尺寸 `Rect2` | 特效到期后按 `zero_tone_hidden` 恢复 `hidden=true`；缩放因子 `maxf(0.0, ...)` |
| **P2-20** | `AssetDB.gd:232-265, 311, 450-455`、`UiSkin.gd:245-253, 297-308`、`MessageWindow.gd:506-509` | 纹理/音频缓存无类型前缀冲突、重入 `_open_media_pack` 未先 `Close()`、解码失败未记 miss、`clear()` 未清 `miss_paths`；`UiSkin` 字体读句柄未 `close()`（Windows 覆写锁文件）及九宫格负尺寸；`MessageWindow._draw()` 每帧重复调 `_sync_backing_nodes()` | 缓存键加前缀并显式 `Close()`；字体校验后 `close()`；夹紧九宫格角尺寸；移除 `_draw()` 冗余同步 |
| **P2-21** | `SetupWizard.gd:273-276`、`MobileDataBootstrap.gd:39-45` | 非 Windows `bash -lc` 双引号拼路径存在 Shell 注入；Android 缺清单时错误回退至桌面版 `SetupWizard` | 改用参数数组或单引号转义；Android 缺数据时展示移动端缺包提示 |
| **P2-22** | `tools/mobile/android_data.py:186-205`、`tools/package_android.sh:15, 158` | `verify-apk` 漏检 `manifest.json.blob`、`CDataBase/SysDatabase.project.blob` 及 `Lwsmf.dll`；`package_android.sh` 未自动探测 `Lolita.ttf` | 补齐 3 个 `.blob` 与 `Lwsmf.dll` 校验；`FONT_OPT` 空时自动回退探测 `work/all/Lolita.ttf` |
| **P2-23** | `tools/mobile/env_check.py:111-117, 141-160, 177-193` | 编辑器配置仅查 Windows 路径；`dotnet` 子进程忽略非 0 退出码；Android SDK 未校验 `platform-tools`/`build-tools` | 按平台补 macOS/Linux 路径；透传 stderr；验证 SDK 子目录 |
| **P2-24** | `tools/godot_check.sh:18, 69-70, 115-171, 255-257` | `DOTNET` 缺前置判空；5 个并行 job 失败时在 `set -e` 下直接退出丢日志；`SaveData01.sav` 存在性误代 `SaveData08.sav` | 补 `DOTNET` 预检；用 `if ! out=...` 捕获失败日志；分别判定槽位 1 与槽位 8 |
| **P2-25** | `tools/lint_decisions.py:10-11, 42-50`、`tools/release_audit.py:172-177, 311-315` | `ROW_RE` 仅匹配反引号编号且未检乱序（当前 `DECISIONS.md:58-59` `D-59` 在 `D-58` 前）；`lint_readme_links` 未跳代码块；`release_audit.py` 算了 `program_asset_missing` 却既不打印也不拦截 | 放宽行匹配 + 校验单调性 + 跳过围栏代码；`release_audit` 输出并在 `--strict` 拦截缺失 |
| **P2-26** | `tools/mobile/test_mobile_tools.py`、`godot/tests_gd/SetupWizardTest.gd:52-149`、`godot/tests/SaveLoadTest.cs:537-558, 622` | `test_mobile_tools.py` 为未接入 CI 的孤儿测试；`SetupWizardTest._remove_tree` 漏设 `include_hidden=true` 致删不掉 `.staging_version` 且缺失败路径用例；`SaveLoadTest:622` 失败退出码恒为 `0` 且 `SaveMigrator` 边界覆盖不全 | 接入 `ci.yml`；补 `include_hidden=true` 与失败重试用例；`SaveLoadTest` 按 `_failures` 返回退出码并补齐迁移边界断言 |

---

## 5. P3 级低危改进与代码卫生（共 13 项）

| 位置（`file:line`） | 说明与建议 |
| :--- | :--- |
| `MapRuntime.cs:175-178` | `ContactBox` 负坐标截断向零，建议改用向下取整除法 |
| `BattleWiring.cs:38, 82` | `_roundLoop` 为只写不读死字段，可删除 |
| `GameDatabase.cs:357-373`、`ProgramLoader.cs:308, 356` | `ClearRow` 漏判 `row < 0`、`LoadMap` 漏判 `pageIndex < 0`，建议补边界保护 |
| `Lwsmf.csproj:3-4, 9, 13-15` | `<Compile Remove="tests/**/*.cs" />` 仅限 `ExportRelease`，建议扩展覆盖 `Release` |
| `VmController.cs:146-177, 490` | `LoadPrograms` 缺 JSON/IO 异常兜底；`_picJsonSb` 高水位不缩容 |
| `LegacySaveImporter.cs:233, 645-675` | `ImportAll` 单槽异常中断整批导入且报告非原子写入 |
| `PictureStage.gd:31-32, 1283-1284` | `HUD_TOP_SLOT_MAX` / `SYSTEM_UI_SLOT_MIN` 成为死常量；另需确认 `6252..11999` 全跳过 `ScreenTint` 是否符合剧情预期（`needs-runtime`） |
| `GameRoot.gd:660-671, 977-985, 1290, 1353-1398` | `_tick_transition` 1 帧 off-by-one；`KEY_PAGEUP/PAGEDOWN` 死分支；少量 `!= null` 防御缺口 |
| `MobileDataBootstrap.gd:180-191, 237, 246` | `_open_next` 失败路径重复关闭 `_source`；`_begin` 建议同步复位 `_part_path`/`_target_path` |
| `ChoiceWindow.gd:83-87`、`MapLayer.gd:199`、`PictureShapes.gd:8` | `close()` 未清 `override_rects`；静态 `TileSet` 每次切图重建；`_sysgraphic_cache` 无上限 |
| `.github/workflows/ios-unsigned.yml:25, 28, 60-64`、`ci.yml:30` | `ios-unsigned.yml` 仍用 `@v4` 且直接插值 `inputs.version`；`ci.yml` 建议补 `dotnet build -c ExportRelease` |
| `tools/godot_check.sh:50-180`、`tools/visual_regression.py:19, 203, 224, 255-262`、`tools/mobile/export_preset_template.py:58` | `godot_check.sh` 硬编码 `python`；`visual_regression.py` 缺 `PIL` 提示、`with Image.open` 与 `TimeoutExpired` 捕获；`export_preset_template.py` 未转义 `\` 与 `"` |
| `crates/wolf-format/tests/tileset.rs:37-194` | 无零售数据时 4 个集成测试静默 `return` 且未在 `ci.yml`（`--lib`）运行，建议显式区分离线/零售测试 |

---

## 6. 建议修复批次与验证路线

1. **第一批（立即合入 — 解除发布与崩溃阻断）**：修复 **P0-1～P0-4**（`package_release.sh` 字体检测、`ci.yml` PowerShell 吞码、`LegacySaveImporter` OOM/死循环）以及 **P1-14、P1-15、P1-16、P1-17**（FaultModal 软锁死、Android 引导原子落盘与中文字体）。
2. **第二批（紧随合入 — 彻底封口读档/切图/跨语言与输入残余）**：修复 **P1-1～P1-13、P1-18～P1-20**（重点：`SaveMigrator` 管线顺序、`SaveCodec` null 击穿 `.bak`、`GameSession`/`VmController` 读档与 `StopAll` 残余、`MediaPakReader` 句柄与锁、`PictureStage` `scale_mode==3` 与快照恢复）。
3. **第三批（门禁加固 — `needs-runtime` 实机复核）**：修复 **P1-21～P1-24** 与 **P2-22～P2-26**，把 `test_mobile_tools.py` 接入 CI，在 Windows + Android 实机跑通 `tools/godot_check.sh`、`tools/package_release.sh`、`tools/package_android.sh` 与 `tools/visual_regression.py`。
