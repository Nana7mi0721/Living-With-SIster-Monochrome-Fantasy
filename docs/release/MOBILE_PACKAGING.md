# Android / iOS 发包工具链与环境准备说明

> **适用范围**：`Living With Sister: Monochrome Fantasy`（Godot 4.7.2 .NET C# + GDScript）
> **当前定位**：Android arm64 可安装测试包已完成构建与签名校验；触控手势、移动端内存上限与数据路径已实现，仍待真机安装/启动验收。iOS 仍采用 macOS CI 构建引擎壳、Windows/macOS 本地注入数据的方式。

---

## 1. 已配套产物清单

| 组件 | 路径 | 职责说明 |
| :--- | :--- | :--- |
| **导出预设** | [godot/export_presets.cfg](../../godot/export_presets.cfg) | `[preset.1] Android` 使用 arm64-v8a、`use_gradle_build=false`、`mobile_data/*`；桌面预设自动排除 `mobile_data/*` |
| **忽略规则** | [.gitignore](../../.gitignore) | 排除 `/godot/mobile_data/`、`/dist/`、`/work/`、`*.apk`、`*.ipa`、`*.keystore`、`*.jks` |
| **环境体检** | [tools/check_mobile_env.sh](../../tools/check_mobile_env.sh)<br>[tools/mobile/env_check.py](../../tools/mobile/env_check.py) | 探测 Godot 4.7.2 Mono 模板、.NET SDK、JDK、Android SDK、本地媒体与调试签名 |
| **Android Staging & 校验** | [tools/mobile/android_data.py](../../tools/mobile/android_data.py) | `plan` / `stage` / `verify-apk`：紧凑 JSON、项目 schema 与 `media.pak` 转专用 blob；媒体条目在 APK 内强制 `ZIP_STORED` |
| **Android 打包** | [tools/package_android.sh](../../tools/package_android.sh) | 支持 `--dry-run`、`--slim`、`--media-pak`、`--template`、`--debug`/`--release`、`--keystore`、`--out-dir`、`--force` |
| **APK 后处理** | [tools/mobile/postprocess_android_apk.py](../../tools/mobile/postprocess_android_apk.py) | 导出后剔除编译期静态库、执行 16 KB 页对齐、以 v2/v3 重签并做布局校验；由打包脚本默认调用 |
| **静态库剔除** | [tools/mobile/strip_android_static_libs.py](../../tools/mobile/strip_android_static_libs.py) | 删除 `lib/**/*.a`，保留运行时 `.so` 与媒体条目的存储方式 |
| **iOS IPA 与数据注入** | [tools/package_ios.sh](../../tools/package_ios.sh)<br>[tools/inject_ios_data.sh](../../tools/inject_ios_data.sh)<br>[tools/mobile/ios_inject_data.py](../../tools/mobile/ios_inject_data.py) | macOS + Xcode 构建无签名引擎 IPA；Windows/macOS 通用 Python 注入 `game_data/` 与 `work/all/media.pak` |
| **CI** | [.github/workflows/ci.yml](../../.github/workflows/ci.yml) | 编译检查、决策表/文档链接检查、隐私审计、移动工具单测与打包脚本语法检查 |
| **单元测试** | [tools/mobile/test_mobile_tools.py](../../tools/mobile/test_mobile_tools.py) | 覆盖数据 staging、APK 校验、静态库剔除、后处理命令编排、iOS 注入、预设改写与 PE 资源处理 |

---

## 2. 当前 Android 0.1.0 交付包

| 项目 | 当前值 |
| :--- | :--- |
| APK | `dist/android/0.1.0/Lwsmf-0.1.0-android-arm64.apk` |
| 体积 | `656,090,638 B`（约 `625.7 MiB`） |
| SHA-256 | `39D48C76FF8DA22631E0F90267D086B12F8E23A7E13F3DCF19CDAAD615FF29AB` |
| 包名 / 版本 | `com.lwsmf.remake` / `0.1.0` |
| SDK / ABI | minSdk `24`、targetSdk `36`、arm64-v8a |
| 签名 | Debug Keystore，APK Signature Scheme v2 + v3 |
| 校验 | `ANDROID_APK_VERIFY_OK entries=359`；`zipalign -c -P 16 4` 通过；媒体条目为 `ZIP_STORED` |

该包是**可安装的 Debug 签名测试包**，不是商店发布包，也不包含正式发布签名材料。

### 2.1 体积来源与裁剪口径

- 从 `work/retail_dat` 剔除 `1,215` 个未被活体清单引用的素材，共 `155,820,460 B`；剩余 `2,847` 个文件、`615,764,547 B`。
- 按 `2,689` 条活体清单以 v1 `WOLFMPK1` 重打 `media.pak`，最终 `608,297,917 B`；未使用 `--slim`、有损重编码、FLAC 替换或 `media_hd.pak`。
- 使用 Godot 4.7.2 源码构建 `module_mono_enabled=yes` 的 arm64、2D/GLES3 定制模板，模板 APK 为 `21,881,762 B`；关闭 Vulkan/D3D12/Metal/3D/3D 物理/3D 导航等未使用模块。
- Android 构建启用 `InvariantGlobalization=true` 与 `SatelliteResourceLanguages=zh-Hans;en`。
- 导出后移除 `13` 个仅用于编译的 Mono 静态归档，共 `59,922,152 B`，再执行 16 KB 页对齐与 v2/v3 重签。
- 图像继续使用裁剪后的原始 1× 零售素材，没有使用 `work/hd` 中的 2× 超分图片。

### 2.2 已验证与未验证

- 已通过：剪枝媒体重建后的 `bash tools/godot_check.sh` 38/38 门禁；`dotnet build godot/Lwsmf.csproj -c Release` 为 0 errors / 0 warnings；最终 APK 的内容、签名与 16 KB 对齐校验。
- 未验证：当前没有连接 Android 设备，因此尚未执行 `adb install`、冷启动、首启数据准备、触控与长时间游玩测试。

---

## 3. 构建与打包步骤

### 3.1 前置条件

1. 安装 .NET 9+ SDK、JDK、Android SDK（platform-tools、build-tools、NDK）与 Godot 4.7.2 Mono 导出模板。
2. 在 Godot 编辑器设置 Android SDK/JDK 路径，或设置 `ANDROID_HOME`、`ANDROID_SDK_ROOT`、`JAVA_HOME`。
3. 首次生成调试签名：

```bash
bash tools/check_mobile_env.sh --target android --setup-debug-keystore
```

4. 体检预览：

```bash
bash tools/check_mobile_env.sh --target android
bash tools/package_android.sh --dry-run
```

### 3.2 构建定制 Mono 模板（可选但用于当前交付包）

在 **Godot 4.7.2 源码树**中执行。`scons`、Android NDK 与 JDK 需在 `PATH` 或环境中可用：

```bash
scons platform=android target=template_release arch=arm64 ndk_platform=android-24 \
  production=yes lto=thin opengl3=yes vulkan=no d3d12=no metal=no \
  accesskit=no sdl=no angle=no swappy=no module_raycast_enabled=no \
  module_mono_enabled=yes disable_3d=yes disable_physics_3d=yes \
  disable_navigation_3d=yes -j8

python modules/mono/build_scripts/build_assemblies.py \
  --godot-output-dir bin --godot-platform android

cd platform/android/java
./gradlew generateGodotMonoTemplates
```

将 `bin/android_monoRelease.apk` 复制到仓库的可选模板位置（例如 `work/out/android-mono-template/`），再通过 `--template` 传给 Android 打包脚本。

### 3.3 构建可安装 APK

当前工作区已将 `work/all/media.pak` 替换为剪枝后的 1× 媒体包。完整构建命令：

```bash
export GODOT_BIN="/path/to/Godot_v4.7.2-stable_mono_win64_console.exe"
export DOTNET_BIN="/path/to/dotnet"

bash tools/package_android.sh --debug --force \
  --media-pak work/all/media.pak \
  --template work/out/android-mono-template/android_monoRelease.apk \
  --out-dir dist/android/0.1.0
```

脚本顺序为：导出 APK → 原始载荷校验 → 剔除 `lib/**/*.a` → 16 KB 对齐 → v2/v3 重签 → 最终载荷校验 → 写入 `SHA256SUMS.txt`。

若使用官方 Mono 模板，省略 `--template`；若需要临时 WebP 瘦身，使用 `--slim`，不要与 `--media-pak` 同时指定。

---

## 4. 移动端运行时适配

1. **首启数据准备**：`MobileDataBootstrap.gd` 只把约 `33.9 MB` 的紧凑 JSON、项目 schema 与字体复制到 `user://mobile_data/`；`media.pak` 标记为 `direct_res`，直接从 `res://` 读取，不再复制约 750 MB 的媒体包。
2. **媒体路径**：`AppPaths` 在 Android 将包根指向 `user://mobile_data`；`MediaPakReader` 通过 Godot 文件接口直接随机读取 APK 内媒体，避免安装后额外占用一整份 `media.pak`。
3. **显存与 HD**：`AssetDB` 使用桌面 512 MiB、Android 128 MiB、iOS 64 MiB 的分级纹理缓存；移动端默认关闭 HD 层。
4. **触控与生命周期**：已实现轻点、双指取消、侧栏滑动、滚动模拟、返回键路由、暂停自动保存与安全区避让；仍需在真机覆盖不同厂商的返回手势与横屏安全区。

---

## 5. iOS 构建简述

在 GitHub Actions 手动触发 `.github/workflows/ios-unsigned.yml`，下载 engine-only unsigned IPA，然后注入数据：

```bash
bash tools/inject_ios_data.sh \
  "Lwsmf-0.1.0-ios-arm64-unsigned.ipa" \
  --game-data game_data \
  --work-all work/all \
  --out "work/out/Lwsmf-0.1.0-ios-arm64-unsigned-full.ipa" \
  --force
```

使用 Sideloadly 或个人 Apple ID 签名安装。iOS 仍未纳入本次 Android 交付验证。
