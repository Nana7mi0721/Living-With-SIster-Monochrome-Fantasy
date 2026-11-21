# 构建、测试与工具链索引（tools/）

本目录包含工程的自动化测试门禁、构建发布流水线、环境配置脚本及逆向分析工具。

---

## 1. 核心测试与质量门禁

| 脚本文件 | 运行环境 | 说明 |
| :--- | :--- | :--- |
| `godot_check.sh` | Bash / Git-Bash | **核心交付门禁脚本**：运行全部 38 项自动化测试（包含无头 Headless 模式与窗口模式），任一项失败即阻塞交付 |
| `release_audit.py` | Python 3 | 发行包资产完整性与结构审计脚本 |
| `refresh_m4_oracle.sh` | Bash + cargo | 重生成 M4 Rust 轨迹夹具；默认使用 `work/all`，可用 `WOLF_DATA_ROOT` 指定数据根；仅当新 Rust 轨迹与 C# 轨迹 100% 一致才覆盖 |
| `copyright_audit.py` | Python 3 | 源码版权与商业素材隔离检查脚本 |
| `make_asset_stub.py` | Python 3 | 自动化生成离线测试用的占位桩（Asset Stub）数据 |

---

## 2. 打包与发布流水线

| 脚本文件 | 目标产物 | 说明 |
| :--- | :--- | :--- |
| `package_release.sh` | Windows Release ZIP | 执行 Release 编译、嵌入 PCK、清洗临时文件并打包独立分发包 |

---

## 3. 环境与辅助脚本

| 脚本文件 | 平台 | 说明 |
| :--- | :--- | :--- |
| `env.sh` | Bash / Linux | 快速载入开发环境变量（Cargo, Godot, .NET 路径配置） |
| `local.env.sh` | Bash | 本机私有（已 `.gitignore`）：在其中 `export GODOT=…`、`DOTNET=…`，`godot_check.sh` 与 `refresh_m4_oracle.sh` 自动载入 |
| `env.ps1` | PowerShell | PowerShell 环境变量初始化脚本 |
| `setpath.ps1` | PowerShell | 将必要工具链永久写入 Windows 用户 PATH 环境变量 |
| `wolfcap.ps1` | PowerShell | 窗口捕获与运行时调试分析辅助脚本 |
| `tmp_dump_ce42.py` | Python 3 | 公共事件 CE42 字节码转储辅助工具 |
| `tmp_dump_map.py` | Python 3 | 地图事件结构分析与转储辅助工具 |
