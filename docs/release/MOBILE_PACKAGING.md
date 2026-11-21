# Android / iOS 发包工具链与环境准备说明

> **适用范围**：`Living With Sister: Monochrome Fantasy`（Godot 4.7.2 .NET C# + GDScript）
> **当前定位**：工具脚本、导出预设、CI 流程与本地环境体检已就绪；实际移动端运行时适配仍属后续专项（详见 [PRE_RELEASE_ORCHESTRATOR_AUDIT.md](../godot/PRE_RELEASE_ORCHESTRATOR_AUDIT.md) 与 [RELEASE.md](../godot/RELEASE.md)）。

---

## 1. 已配套产物清单

| 组件 | 路径 | 职责说明 |
| :--- | :--- | :--- |
| **导出预设** | [godot/export_presets.cfg](../../godot/export_presets.cfg) | 新增 `[preset.1] Android`（arm64-v8a、`use_gradle_build=false`、`mobile_data/*`）与 `[preset.2] iOS`（arm64、`export_project_only=true`、`com.lwsmf.remake`）；桌面 `[preset.0]` 自动排除 `mobile_data/*` |
| **忽略规则** | [.gitignore](../../.gitignore) | 排除 `/godot/mobile_data/`、`*.apk`、`*.ipa`、`*.keystore`、`*.jks` |
| **环境体检** | [tools/check_mobile_env.sh](../../tools/check_mobile_env.sh)<br>[tools/mobile/env_check.py](../../tools/mobile/env_check.py) | 探测 Godot 4.7.2 Mono 模板（`android_*.apk`/`ios.zip`）、`.NET 8` 与 `android`/`ios` workload、JDK `java`/`keytool`、Android SDK、本地 `media.pak`/`game_data` 与 `debug.keystore` |
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
   - **待安装（真正打 APK 时补齐）**：
     ```bash
     # 1. 安装 .NET 8 Android 工作负载（Godot .NET 导出必需）
     dotnet workload install android

     # 2. 安装 Android SDK（Android Studio / commandline-tools：platform-tools + build-tools + android-34+）
     # 并设置 ANDROID_HOME（或 ANDROID_SDK_ROOT）

     # 3. 首次生成调试签名密钥（work/out/android/debug.keystore）
     bash tools/check_mobile_env.sh --target android --setup-debug-keystore
     ```
   - 预演打包计划（不改写工作区）：
     ```bash
     bash tools/package_android.sh --dry-run
     bash tools/package_android.sh --slim --dry-run
     ```
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

## 3. 后续真正开启移动端运行时适配前的代码改造清单

当前发包工具链已就位；在首次实机运行移动端 APK/IPA 前，C#/GDScript 运行时仍需补齐以下 4 项移动端专项改造（对比姊妹作纯 Rust 原生层的差异）：

1. **`MediaPakReader.cs` APK 直读支持**：当前使用 `System.IO.FileStream` 读取文件系统路径；Android 打包把 `media.pak.blob.cfb` 以 `ZIP_STORED` 放在 APK 内，或由首启异步搬运至 `user://`（双份空间），或改为按 APK ZIP 偏移/流读取。
2. **`AppPaths.cs` / `AppPaths.gd` 移动端路径收敛**：Android 的 `OS.GetExecutablePath()` 在 APK 内不可直接拼 `../game_data`，需按 `OS.HasFeature("android")` / `OS.HasFeature("ios")` 区分读写根（`user://` vs `<bundle>/game_data`、`res://mobile_data/` 解包校验 `staging_version`）。
3. **触控手势与菜单适配**：补齐无鼠标悬停下的双击确认、长按/双指右键取消、Tips/战斗/H 场景小热区容错与屏幕安全区避让。
4. **内存与首启性能优化**：将 `AssetDB.gd` 纹理缓存上限在移动端由 384 MB 下调为 128–192 MB，并为 50 MB `programs.json` 启动反序列化增加异步加载提示。
