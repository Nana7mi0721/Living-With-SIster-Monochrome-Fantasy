# LWSMF Godot 重制版第十二轮技术审阅报告（`e954ecc`）

- **审阅对象**：`imoto-life`（commit `e954eccf07e69d7a19104b39febc1da4843e4ee7`）
- **主审区间**：`bce18a1..e954ecc`（M5 收口与发布打包）
- **审阅日期**：2026-11-19

## 一、总评

1. **M5 打包、版权审计、存档回退与性能验收框架整体扎实（置信度：高）**
   - `godot/Lwsmf.csproj` 在 `ExportRelease` 配置下剔除 `tests/**/*.cs`，`godot/export_presets.cfg` 排除 `tests/*,tests_gd/*,game_data/*,work/*,build/*`，配合 `tools/copyright_audit.py` 对禁用扩展名与 `manifest.json` SHA-256 指纹做全量扫描，版权隔离严密。
   - `SaveCodec.cs` / `GameSession.cs` / `GameRoot.gd` 补齐主档损坏回退 `.bak`、读档失败保留当前卧室状态及故障弹窗「回到最近存档（`recover_to_last_save`）」。
   - `PerformanceProbeTest`、`PerformanceFrameProbe`（strict OpenGL3 `4.2 ms/frame`、`~238 fps`）与 90 天 `SleepSoakTest` 证明性能余量与长线稳定性（`needs-runtime` 复核）。
2. **发现 1 项发布包 P1 阻断缺陷：导出构建运行时仍用 `res://` 解析外部 `game_data/`（置信度：高）**
   `SetupWizard.gd:32-47` 正确区分了 `OS.has_feature("template")` 并以 `OS.get_executable_path().get_base_dir()` 定位包体根目录；但正式游戏数据加载链路中的 **`VmController.cs:112,130`**、**`GameSessionController.cs:113`**、**`GameRoot.gd:421`** 与 **`AssetDB.gd:48`** 仍然全部硬编码 `ProjectSettings.GlobalizePath("res://")` / `ProjectSettings.globalize_path("res://")`。在 Windows 导出包 `dist/Lwsmf-win64/game/Lwsmf.exe` 运行时，`GlobalizePath("res://")` 返回虚拟前缀 `"res://"` 而非 `.exe` 物理目录，导致 `SetupWizard` 判定 `game_data` 就绪后，主程序加载 `programs.json` / `database.json` / `map_tiles.json` 必定失败并进入 `phase="failed"`（`needs-runtime` 实机双击验证）。
3. **发现 1 项 P2 安装引导竞态与命令兼容性隐患（置信度：高）**
   `SetupWizard.gd:227-241` 启动转换前未删除旧的 `user://setup_conversion_status.txt`，二次重试时 `_process()` 会立即读到上次遗留退出码而提前误判；且 `build_convert_command` 在 Windows 写死 `py -3`，与发布 `README.txt` 及 `convert.bat` 要求的 `python` 不一致。
4. **第十一轮 P3 遗留（`bce18a1`）已闭环，38 步门禁脚本标签有轻微滞后（置信度：高）**
   `R11-P3-01`（`14/35..16/35`）与 `R11-P3-02`（`M13Harness.NewVm()` 显式 `StrictMode = true`）已在 `bce18a1` 闭环；`e954ecc` 扩至 38 步后，`14..16` 标签仍停留 `/35`（或 `/38` 需统一）。

## 二、快照核对结论

- **快照来源**：基于最新上传的 `imoto-life` 归档包（解压目录 `imoto-life-e954eccf07e69d7a19104b39febc1da4843e4ee7`），对应 commit `e954ecc`。
- **增量范围**：涵盖 `Lwsmf.csproj`、`export_presets.cfg`、`SetupWizard.gd`、`GameRoot.gd`、`GameSession.cs`、`SaveCodec.cs`、`TraceRecorderHost.cs`、`SleepSoakTest.cs`、`PerformanceProbeTest.cs`、`PerformanceFrameProbe.gd`、`converter/cli.py`、`converter/convert.bat`、`tools/package_release.sh`、`tools/copyright_audit.py`、`tools/godot_check.sh` 及 `M5_HANDOFF.md`/`RELEASE.md`。
- **边界声明**：沙箱无 Godot 4.7.2 .NET 运行时，未实机启动 `dist/Lwsmf-win64/game/Lwsmf.exe`，运行时结果标 `needs-runtime`。

## 三、本轮新增问题清单

| ID | 严重度 | 证据(文件:行) | 触发条件 | 根因 | 影响 | 最小修复 | 验证方法 | 置信度 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `R12-P1-01` | **P1（发布阻断）** | `godot/presentation/VmController.cs:112,130`; `godot/presentation/GameSessionController.cs:113`; `godot/presentation_gd/GameRoot.gd:421`; `godot/presentation_gd/AssetDB.gd:48`; 对比 `SetupWizard.gd:32-47` | 运行打包后的 `dist/Lwsmf-win64/game/Lwsmf.exe`（`OS.has_feature("template") == true`） | 仅 `SetupWizard.gd` 使用 `OS.get_executable_path().get_base_dir()`；其余 C#/GDScript 加载器仍调用 `ProjectSettings.GlobalizePath("res://")` | 导出包中 `GlobalizePath("res://")` 返回 `"res://"`，导致 `programs.json`、`database.json`、`map_tiles.json` 与资产目录解析失败，游戏无法进入标题画面 | 在 C#（如 `AppPaths.PackageRoot()`）与 GDScript（`SetupWizard.package_root()`）抽取统一根目录解析：编辑器/测试用 `ProjectSettings.GlobalizePath("res://")/..`，模板导出用 `OS.GetExecutablePath().GetBaseDir()/..` | 执行 `package_release.sh` 后，在独立目录双击启动 `dist/Lwsmf-win64/game/Lwsmf.exe` 完成首次转换并进入卧室存档（`needs-runtime`） | 高 |
| `R12-P2-01` | P2 | `godot/presentation_gd/SetupWizard.gd:102-103,227-241` | 玩家首次转换失败后在向导内再次点击「开始转换」，或机器只装了 `python` 未装 `py` 启动器 | 1) `start_conversion()` 未预先删除 `user://setup_conversion_status.txt`；2) `build_convert_command` 硬编码 `py -3` 而非统一走 `converter/convert.bat` 或探测 `python` | 重试时首帧误读旧状态文件直接报错退出；缺少 `py.exe` 的 Windows 环境无法通过向导转换 | 1) `start_conversion()` 开头 `DirAccess.remove_absolute(_status_file)`；2) Windows 优先调用 `converter/convert.bat`（或尝试 `py -3` / `python` 回退） | `SetupWizardTest.gd` 增加状态文件清理与重试回归（`needs-runtime`） | 高 |
| `R12-P3-01` | P3 | `tools/godot_check.sh`; `docs/godot/M5_HANDOFF.md:8` | 运行 `tools/godot_check.sh` 或查阅 M5 交接文档 | 38 步门禁扩步后部分旧标签未同步；`M5_HANDOFF.md:8` 仍写“提交号见 git log” | 仅影响日志/文档一致性 | 统一门禁步数标签并在 `M5_HANDOFF.md` 填入最终发布提交 SHA | 静态检查 | 高 |

## 四、发布准入与下一步指示

**当前结论**：`e954ecc` 距离最终 Windows 发布包交付**仅差修复 `R12-P1-01` 与 `R12-P2-01` 并完成一次独立目录导出包实机冒烟**。修复并实测通过后，即可正式冻结 M5 发布分支。
