# 构建、测试与工具链索引（tools/）

本目录包含工程的自动化测试门禁、构建发布流水线、多平台打包脚本、逆向对账与分析工具。

---

## 1. 核心测试与质量门禁

| 脚本文件 | 运行环境 | 说明 |
| :--- | :--- | :--- |
| `godot_check.sh` | Bash / Git-Bash | **核心交付门禁脚本**：运行全部 38 项自动化测试（包含无头 Headless 模式与窗口模式），任一项失败即阻塞交付 |
| `release_audit.py` | Python 3 | 发行包资产完整性与结构审计脚本 |
| `audit_programs_static.py` | Python 3 | 离线静态扫描 `game_data/programs.json`，检测高危 Opcode、循环条件与事件安全 |
| `audit_cross_platform_assets.py` | Python 3 | 跨平台（Windows / Android / iOS）素材与字体兼容性静态审查 |
| `lint_decisions.py` | Python 3 | 校验 `docs/godot/DECISIONS.md` 编号规范与 `docs/README.md` 全局链接有效性 |
| `refresh_m4_oracle.sh` | Bash + Cargo | 重新生成 M4 Rust 轨迹夹具（仅当与 C# 100% 一致时更新） |
| `copyright_audit.py` | Python 3 | 源码版权与商业素材隔离检查脚本 |
| `make_asset_stub.py` | Python 3 | 自动化生成离线测试用的占位桩（Asset Stub）数据 |
| `visual_regression.py` | Python 3 | 基于 SSIM 算法的图层渲染与立绘像素级视觉回归测试 |

---

## 2. 打包与发布流水线

| 脚本/目录 | 目标平台 | 说明 |
| :--- | :--- | :--- |
| `package_release.sh` | Windows | 执行 Release 编译、嵌入 PCK、清洗临时文件并打包独立分发包 |
| `pack_media.py` | 全平台 | 从现有 `media.pak` 重打媒体归档；`--lossless` 对 PNG/BMP 做像素级无损 WebP 压缩，`--extra-root` 仅补缺失路径 |
| `fix_integrity_level.ps1` | Windows | 将工作区或指定路径的 NTFS 完整性级别从 Low 重置为 Medium；`-GrantCurrentUser` 用于普通权限修复 |
| `package_android.sh` | Android | Android APK 导出构建与素材包嵌入 |
| `package_ios.sh` | iOS | iOS Xcode 工程导出与无签名包归档流水线 |
| `sync_release_icons.py` | 全平台 | 从本地 `_publish` 素材派生三端应用图标、Windows 窗口图标与安装器素材；Android 自适应前景使用 72% 居中布局并提供浅色背景，避免启动器黑边；`sisfan_promo.png` 原字节派生为欢迎页海报，`TitleGraphic6.jpg` 同尺寸转存为完成页 PNG，另生成 `55×55` 白底小图标 |
| `brotli_json.py` | 全平台 | 对 `game_data/*.json` 进行无损 Brotli 高压缩预处理（体积缩减 ~80%） |
| `installer/` | Windows | Inno Setup 安装包脚本模板与构建定义 |
| `mobile/` | Android / iOS | 移动端环境检测、APK 后处理、静态库剥离（`strip_android_static_libs.py`）工具 |
| `templates/` | 全平台 | 自定义 2D / GLES3 + Mono 极简导出模板流水线 |

---

## 3. 逆向分析、音画与本地化工具

| 脚本/目录 | 说明 |
| :--- | :--- |
| `cheat_engine/` | Cheat Engine 内存分析表、618 个具名通常变量十六进制偏移对照表与使用指南 |
| `translation/` | 多语言文本提取、差异对比与翻译覆盖流水线 |
| `upscale_assets.py` | HD 2x 高清素材放大重采样实验工具 |
| `verify_hd_manifest.py` | HD 资产清单完整性与哈希一致性核验工具 |

---

## 4. 环境与辅助脚本

| 脚本文件 | 平台 | 说明 |
| :--- | :--- | :--- |
| `env.sh` | Bash / Linux | 快速载入开发环境变量（Cargo, Godot, .NET 路径配置） |
| `local.env.sh` | Bash | 本机私有（已 `.gitignore`）：供本地覆盖 `GODOT`、`DOTNET` 路径 |
| `env.ps1` | PowerShell | PowerShell 环境变量初始化脚本 |
| `setpath.ps1` | PowerShell | 将必要工具链永久写入 Windows 用户 PATH 环境变量 |
| `wolfcap.ps1` | PowerShell | 窗口捕获与运行时调试分析辅助脚本 |
