# LWSMF 多平台打包工具链与环境准备清单

> **适用项目**：`Living With Sister: Monochrome Fantasy`（Godot 4.7.2-stable .NET + .NET 8.0 SDK + GDScript，`gl_compatibility` 渲染后端）
> **架构说明**：游戏本体仅依赖 Godot .NET 运行时与脚本，`crates/` 下的 Rust 工作区仅用于参考与测试验证，**无需**跨平台编译进各端包体。

## 平台现状总览

| 目标平台 | 目标产物 | 仓库现有配置 | 构建宿主 | 当前主要缺项 |
| :--- | :--- | :--- | :--- | :--- |
| **Windows (x64)** | `.exe` + `.pck` + `.NET 8` 运行时目录 / `.zip` | 预设 `[preset.0]`、`tools/package_release.sh` 已跑通 | Windows | 可选：图标 `.ico`、`rcedit`、代码签名证书 |
| **Android (arm64-v8a)** | `.apk`（含 `mobile_data/`） | 预设 `[preset.1]`、`tools/package_android.sh`、`android_data.py` 已就绪 | Windows / macOS / Linux | `.NET 8 android` workload、JDK 17+、Android SDK、签名 Keystore、运行时 4 项适配 |
| **iOS (arm64)** | 无签名 `.ipa` + 本地数据注入 | 预设 `[preset.2]`、`tools/package_ios.sh`、`inject_ios_data.sh`、`.github/workflows/ios-unsigned.yml` 已就绪 | macOS（或 GitHub Actions `macos-latest`）+ 任意系统本地注入 | 签名/侧载方案、iOS 15.0+ 真机、运行时适配；上架另需 Apple Developer 账号 |
| **Linux (x86_64，可选)** | `.x86_64` + `.pck` + `.NET 8` 运行时目录 / `.tar.gz` | 尚未配置导出预设与打包脚本 | Windows / Linux / macOS（支持交叉导出） | 新增 `[preset.3] Linux`、`tools/package_linux.sh`、大小写敏感排查、Linux 测试环境 |

建议先在仓库根目录执行一次环境体检，确认本机缺项：

```bash
bash tools/check_mobile_env.sh --target all
```

## 1. 通用基础工具链（所有平台共用）

- [ ] **Godot 4.7.2-stable Mono（.NET 版）编辑器与控制台程序**
  - 配置 `GODOT_BIN` 环境变量或加入系统 `PATH`。
- [ ] **Godot `4.7.2.stable.mono` 官方导出模板**
  - 确认模板目录包含 Windows、`android_debug.apk`、`android_release.apk`、`android_source.zip`、`ios.zip` 以及 Linux 模板。
  - Windows 默认路径：`%APPDATA%\Godot\export_templates\4.7.2.stable.mono\`。
- [ ] **.NET 8.0 SDK**
  - 对应 `godot/Lwsmf.csproj` 的 `<TargetFramework>net8.0</TargetFramework>`。
  - 未加入全局 `PATH` 时需配置 `DOTNET_BIN` 与 `DOTNET_ROOT`。
- [ ] **Python 3.10+ 与 `Pillow` 库**
  - 用于紧凑化 `game_data/*.json`、运行 `tools/release_audit.py` 审计、iOS 数据注入，以及 `--slim` 模式下的 WebP 图片瘦身。
- [ ] **完整游戏载荷数据**
  - `game_data/`：`manifest.json`、`programs.json`、`database.json`、`map_tiles.json`。
  - `work/all/media.pak`（加密媒体归档）。
  - `work/all/BasicData/`：`CDataBase.project`、`DataBase.project`、`SysDatabase.project`。
  - 字体文件：`Lolita.ttf`（位于 `work/all/` 或已打入 `media.pak`）。

## 2. Windows（x86_64）

当前标准版与 `--slim` 瘦身包均已可通过 `tools/package_release.sh` 导出并通过审计，仅剩面向公开分发的可选项：

- [ ] **（可选）应用图标 `.ico` 与 `rcedit` 工具**
  - 当前 `godot/export_presets.cfg` 中 `application/icon`、`file_version`、`product_version` 为空；若需为 `Lwsmf.exe` 写入图标和版本元数据，需在 Godot 编辑器中配置 `rcedit` 路径。
- [ ] **（可选）Windows 代码签名证书**
  - 用于正式公开分发时减少 Windows SmartScreen 拦截；自用或小范围分发可跳过。

## 3. Android（arm64-v8a APK）

- [ ] **安装 .NET 8 Android 工作负载**
  - Godot .NET 导出 Android 必需：
    ```bash
    dotnet workload install android
    ```
- [ ] **安装 JDK 17+（提供 `java` 与 `keytool`）**
  - 配置 `JAVA_HOME` 或将 `bin/` 加入 `PATH`，用于密钥生成与 APK 签名。
- [ ] **安装 Android SDK**
  - 通过 Android Studio 或 `commandline-tools` 安装 `platform-tools`、`build-tools` 与 `platforms;android-34`（或更高版本）。
  - 设置 `ANDROID_HOME`（或 `ANDROID_SDK_ROOT`）环境变量。
- [ ] **准备签名密钥（Keystore）**
  - **测试包（Debug）**：运行以下命令自动生成 `work/out/android/debug.keystore`：
    ```bash
    bash tools/check_mobile_env.sh --target android --setup-debug-keystore
    ```
  - **正式包（Release）**：使用 `keytool` 生成正式 `.jks` / `.keystore` 并妥善备份，打包时通过 `--release --keystore <路径>` 传入。
- [ ] **ADB 与开启 USB 调试的 Android arm64 真机**
  - `adb` 随 `platform-tools` 附带，用于装包测试与 `adb logcat` 排查启动崩溃。

## 4. iOS（arm64 IPA）

- [ ] **准备 macOS + Xcode + `.NET` iOS 工作负载（二选一）**
  - **方案 A（无 Mac，推荐）**：在 GitHub Actions 手动触发 `.github/workflows/ios-unsigned.yml`（`workflow_dispatch`），由 `macos-latest` 自动安装 `.NET 8` + `ios` workload、Godot 4.7.2 Mono 与 Xcode，下载不含商业资产的 `Lwsmf-<ver>-ios-arm64-unsigned.ipa`。
  - **方案 B（本地 Mac）**：安装 Xcode（含 iOS SDK 与 `xcodebuild`）、.NET 8 SDK、`dotnet workload install ios` 以及 Godot 4.7.2 Mono macOS 版与导出模板。
- [ ] **本地数据注入（Windows / macOS / Linux 通用）**
  - 使用仓库自带的纯 Python 脚本注入 `game_data/` 与 `work/all/media.pak`：
    ```bash
    bash tools/inject_ios_data.sh \
      "Lwsmf-0.1.0-ios-arm64-unsigned.ipa" \
      --game-data game_data \
      --work-all work/all \
      --out "work/out/Lwsmf-0.1.0-ios-arm64-unsigned-full.ipa" \
      --force
    ```
- [ ] **准备签名与装机方案**
  - **个人自测 / 侧载**：免费 Apple ID + Sideloadly / AltStore / SideStore（7 天续签）；或使用巨魔（TrollStore）直接安装无签名 IPA。
  - **TestFlight / 正式分发**：Apple Developer Program 付费账号（$99/年）+ Distribution 证书 + Provisioning Profile，并替换 `godot/export_presets.cfg` 中 `[preset.2] iOS` 的占位配置（`app_store_team_id="XXXXXXXXXX"` 等）。
- [ ] **iOS 15.0+ arm64 真机（开启开发者模式）**
  - 匹配预设中的 `application/min_ios_version="15.0"`。

## 5. Linux（x86_64，可选）

Linux 接入成本最低：Godot 4.7.2 Mono 官方导出模板已自带 Linux x86_64 模板，且 .NET 8 SDK 支持在 Windows 本机直接交叉发布 `linux-x64`，无需额外安装重型 SDK。

- [ ] **在 `godot/export_presets.cfg` 新增 `[preset.3] Linux` 导出预设**
  - 架构选 `x86_64`，`exclude_filter` 沿用 Windows 预设（`tests/*,tests_gd/*,game_data/*,work/*,build/*,addons/*,mobile_data/*`）。
- [ ] **编写 `tools/package_linux.sh` 打包脚本**
  - 参考 `tools/package_release.sh` 导出 `Lwsmf.x86_64`、`Lwsmf.pck` 与 `data_Lwsmf_linux_x86_64/`，打成 `.tar.gz` 并确保保留可执行权限（`chmod +x`）。
  - 若需单文件便携分发，可额外准备 `appimagetool` 打包为 `.AppImage`。
- [ ] **排查文件路径大小写敏感问题**
  - Linux 文件系统区分大小写，需确认 `game_data/`、`BasicData/*.project`、`Lolita.ttf` 以及 `media.pak` 索引查找在代码中已统一规范化大小写。
- [ ] **准备 Linux 测试环境**
  - 实体机、虚拟机或 Windows WSL2（启用 WSLg），用于验证 OpenGL（`gl_compatibility`）渲染、PulseAudio/PipeWire 音频输出与文件权限；也可在 GitHub Actions `ubuntu-latest` 中做自动导出与冒烟测试。

## 打包之外的关键前置项

1. **移动端 4 项运行时代码改造（Android / iOS 真机可玩前提）**
   - 详见 `docs/release/MOBILE_PACKAGING.md` 第 3 节：
     - `MediaPakReader.cs`：支持从 Android APK 内 `ZIP_STORED` 的 `media.pak.blob.cfb` 直读，或首启异步解包至 `user://`。
     - `AppPaths.cs` / `AppPaths.gd`：按 `OS.HasFeature("android")` / `OS.HasFeature("ios")` 区分只读载荷路径与可写存档根 `user://`（当前依赖 `OS.GetExecutablePath()/..` 仅适用于桌面端）。
     - 触控交互适配：补齐无鼠标悬停下的点击确认、长按/双指取消、小热区容错与屏幕安全区避让。
     - 内存与首启优化：移动端将 `AssetDB.gd` 纹理缓存上限由 384 MB 下调至 128–192 MB，并为大型 `programs.json` 反序列化增加加载提示。
2. **常用打包命令速查**

| 平台 | 命令 |
| :--- | :--- |
| Windows 标准包 | `bash tools/package_release.sh` |
| Windows 瘦身包 | `bash tools/package_release.sh --slim` |
| Android 预演（不构建） | `bash tools/package_android.sh --slim --dry-run` |
| Android 调试 APK | `bash tools/package_android.sh --slim --debug` |
| Android 正式 APK | `bash tools/package_android.sh --slim --release --keystore <path> --ks-user <alias> --ks-pass <pass>` |
| iOS 本地数据注入 | `bash tools/inject_ios_data.sh <unsigned.ipa> --game-data game_data --work-all work/all --out <full.ipa> --force` |
