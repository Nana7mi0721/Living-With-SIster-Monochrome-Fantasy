# Android / iOS 发包工具链与环境准备说明

> **适用范围**：`Living With Sister: Monochrome Fantasy`（Godot 4.7.2 .NET C# + GDScript）
> **当前定位**：工具脚本、导出预设、CI 流程与本地环境体检已就绪；Android 首启数据落盘与路径切换已完成，触控与内存专项仍待实机收口（详见 [PRE_RELEASE_ORCHESTRATOR_AUDIT.md](../godot/PRE_RELEASE_ORCHESTRATOR_AUDIT.md) 与 [RELEASE.md](../godot/RELEASE.md)）。

---

## 1. 已配套产物清单

| 组件 | 路径 | 职责说明 |
| :--- | :--- | :--- |
| **导出预设** | [godot/export_presets.cfg](../../godot/export_presets.cfg) | 新增 `[preset.1] Android`（arm64-v8a、`use_gradle_build=false`、`mobile_data/*`）与 `[preset.2] iOS`（arm64、`export_project_only=true`、`com.lwsmf.remake`）；桌面 `[preset.0]` 自动排除 `mobile_data/*` |
| **忽略规则** | [.gitignore](../../.gitignore) | 排除 `/godot/mobile_data/`、`*.apk`、`*.ipa`、`*.keystore`、`*.jks` |
| **环境体检** | [tools/check_mobile_env.sh](../../tools/check_mobile_env.sh)<br>[tools/mobile/env_check.py](../../tools/mobile/env_check.py) | 探测 Godot 4.7.2 Mono 模板（`android_*.apk`/`ios.zip`）、`.NET 9`（Android）/`.NET 8`（桌面）、JDK `java`/`keytool`、Android SDK、本地 `media.pak`/`game_data` 与 `debug.keystore`；Godot 编辑器已配置的 JDK/SDK 路径可自动回退 |
| **Android Staging & 校验** | [tools/mobile/android_data.py](../../tools/mobile/android_data.py) | `plan` / `stage` / `verify-apk`：紧凑 JSON 转 `.json.blob`、`BasicData/*.project` 转 `.blob`、`media.pak` 转 `.blob.cfb`（强制 APK `ZIP_STORED` 不压缩），防路径穿越与玩家存档混入 |
| **Android 打包脚本** | [tools/package_android.sh](../../tools/package_android.sh) | 支持 `--dry-run`、`--slim`（调用 `pack_media.py --slim`）、`--debug`/`--release`、`--keystore`，退出自动清理 `godot/mobile_data/` 与恢复预设 |
| **iOS IPA 打包与数据注入** | [tools/package_ios.sh](../../tools/package_ios.sh)<br>[tools/inject_ios_data.sh](../../tools/inject_ios_data.sh)<br>[tools/mobile/ios_inject_data.py](../../tools/mobile/ios_inject_data.py) | macOS + Xcode 构建无签名引擎 IPA；Windows/macOS 通用纯 Python `zipfile` 注入 `game_data/` 与 `work/all/media.pak`，保留 Unix 权限位与符号链接并校验 CRC |
| **预设临时改写器** | [tools/mobile/export_preset_template.py](../../tools/mobile/export_preset_template.py) | 原子修改 `export_presets.cfg` 单项配置（如 keystore/自定义模板），配合脚本 `EXIT` trap 自动还原 |
| **macOS CI 工作流** | [.github/workflows/ios-unsigned.yml](../../.github/workflows/ios-unsigned.yml) | 在 `macos-latest` 安装 `.NET 8` + `ios` workload 与 Godot 4.7.2 Mono，导出不含商业资产的 engine-only unsigned IPA |
| **单元测试** | [tools/mobile/test_mobile_tools.py](../../tools/mobile/test_mobile_tools.py) | 覆盖 `android_data`、`ios_inject_data`、`export_preset_template` 与 `env_check` 预设解析 |

---

## 2. 本机环境体检与前置补齐步骤

运行体检命令查看当前机器缺失项：

```bash
bash tools/check_mobile_env.sh --target all
```

1. **Android 构建（Windows / macOS / Linux）**：
   - **已就绪**：Godot 4.7.2 Mono 模板中的 `android_debug.apk`、`android_release.apk`、`android_source.zip`，以及 JDK (`java`/`keytool`)。
   - **前置条件**：
     ```bash
     # 1. 安装 .NET 9 SDK；Godot 4.7.2 Mono Android 模板使用 net9.0
     #    桌面构建仍使用 net8.0。打 Android 包时通过 DOTNET_BIN 指向 .NET 9。

     # 2. 安装 Android SDK（Android Studio / commandline-tools：platform-tools + build-tools + android-34+）
     # 并在 Godot 编辑器设置 Android SDK 路径，或设置 ANDROID_HOME / ANDROID_SDK_ROOT。

     # 3. 首次生成调试签名密钥（work/out/android/debug.keystore）
     bash tools/check_mobile_env.sh --target android --setup-debug-keystore
     ```
   - Godot Mono Android 导出通过 NuGet 还原 `Microsoft.NETCore.App.Runtime.Mono.android-arm64`，无需 `dotnet workload install android`。
   - `godot/project.godot` 已开启 `rendering/textures/vram_compression/import_etc2_astc=true`，这是 Android 导出的硬性要求。
   - 预演打包计划（不改写工作区）：
     ```bash
     bash tools/package_android.sh --dry-run
     bash tools/package_android.sh --slim --dry-run
     ```
   - 完整 Debug APK：
     ```bash
     export GODOT_BIN="/path/to/Godot_v4.7.2-stable_mono_win64_console.exe"
     export DOTNET_BIN="/path/to/dotnet9/dotnet"
     bash tools/package_android.sh --debug --force
     ```
   - 2026-11-22 本机验证：Debug APK `870,575,047` B，v2/v3 签名通过，`assets/.godot/mono/publish/arm64/Lwsmf.dll` 与 `assets/mobile_data/` 均已入包。
2. **iOS 构建（macOS CI + Windows 本地注入）**：
   - 在 GitHub Actions 手动触发 `.github/workflows/ios-unsigned.yml`（`workflow_dispatch`），下载 `Lwsmf-<ver>-ios-arm64-unsigned.ipa`。
   - 在 Windows 本机注入 `game_data/` 与 `work/all/media.pak`（含 `--slim` 时传 `--media-pak`）：
     ```bash
     bash tools/inject_ios_data.sh \
       "Lwsmf-0.1.0-ios-arm64-unsigned.ipa" \
       --game-data game_data \
       --work-all work/all \
       --out "work/out/Lwsmf-0.1.0-ios-arm64-unsigned-full.ipa" \
       --force
     ```
   - 使用 Sideloadly 或自签名工具用个人 Apple ID 签名安装至设备。

---

## 3. 移动端运行时适配清单

当前发包工具链与 Android 首启数据链路已就位：

1. **已完成：Android 首启数据落盘**：`MobileDataBootstrap.gd` 在 `OS.HasFeature("android")` 时读取 `res://mobile_data/mobile_data_manifest.json`，按清单把 `.blob`/`.blob.cfb` 以 4 MB 分块复制到 `user://mobile_data/`，去除暂存后缀、校验文件大小并写入 `.staging_version` 标记。首次复制约 750 MB；中途退出会在下次启动重试，已完成的文件不会半写入正式路径。
2. **已完成：移动端路径收敛**：`AppPaths.cs` / `AppPaths.gd` 在 Android 上把包根切到 `user://mobile_data`，`res://` 继续由 Godot 的 Android AssetManager 读取；`MediaPakReader` 因此继续使用 `FileStream` 读取 `user://mobile_data/work/all/media.pak`，不再要求直接读 APK 压缩条目。
3. **待实机：触控手势与菜单适配**：补齐无鼠标悬停下的双击确认、长按/双指右键取消、Tips/战斗/H 场景小热区容错与屏幕安全区避让。
4. **待实机：内存与首启性能优化**：将 `AssetDB.gd` 纹理缓存上限在移动端由 384 MB 下调为 128–192 MB，并为 50 MB `programs.json` 启动反序列化增加异步加载提示。
