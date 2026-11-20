# 开发者贡献指南（Contributing Guide）

感谢对本项目（《妹相随～黑白世界的缤纷冒险～》现代游戏重构工程）的关注与支持！为了保持仓库的代码质量、工程结构规范与协作效率，请遵循以下流程与约定。

---

## 1. 架构总览

本仓库采用“离线转换 + 确定性 C# 内核 + Godot 呈现层”的现代化混合分工体系：

- `converter/`：Python 离线转换器（解析 WOLF 二进制数据，生成 `game_data/*.json`）
- `godot/core_cs/`：C# 8.0 确定性虚拟机调度器与游戏状态机（无帧率依赖，纯逻辑流转）
- `godot/presentation_gd/`：Godot 4.7.2 GDScript 呈现层（视口管理、复合立绘层级、文本排版、音效混音）
- `crates/`：Rust 验证原型与 Oracle 对拍基线（用于开发期行为精度对账）
- `docs/`：全量技术契约、Opcode 规格、审查台账与研发记录
- `tools/`：自动化测试、发布打包与逆向辅助脚本

---

## 2. 环境准备

1. **.NET SDK**：.NET 8.0 SDK（推荐 x64）。
2. **Godot 引擎**：Godot 4.7.2 stable Mono（.NET 支持版）。
3. **Python 环境**：Python 3.10+（用于运行数据转换流水线）。
4. **Rust 工具链**（可选，仅用于维护 Oracle）：Rust Stable（参考 `rust-toolchain.toml`）。

---

## 3. 开发与测试流程

### 3.1 本地编译
```bash
# 编译 Godot C# 核心库
dotnet build godot/Lwsmf.csproj -c Release
```

### 3.2 运行自动化门禁测试
```bash
# 执行 38 项核心门禁测试（包含 headless 与窗口模式）
bash tools/godot_check.sh
```

### 3.3 数据转换流水线（若更新了原始数据）
```bash
python converter/main.py --data-root <path_to_unpacked_wolf_data> --output game_data/
```

### 3.4 Rust Oracle 验证（开发期对账）
```bash
source tools/env.sh
cargo test -q --workspace
```

---

## 4. 协作约定与代码准则

1. **写作用域隔离**：在并行开发或多模块修复时，各任务涉及的文件范围需明确界定，避免修改冲突。
2. **资产安全与隔离原则**：
   - 绝对不要将未授权的商业游戏资产（`work/Data/`、`*.wolf` 文件）提交至版本库（已在 `.gitignore` 声明排除）。
   - 临时生成的分析探针、测试日志及截图请勿提交。
3. **提交规范**：
   - 提交信息应遵循工程化规范（`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`），准确描述修改动机与涉及模块。
   - 保持中性、客观的技术描述。
4. **深入阅读**：
   - 协作指南请参阅 [AGENTS.md](AGENTS.md)。
   - 全局文档索引请参阅 [docs/README.md](docs/README.md)。
   - 最新核心缺陷审查报告请参阅 [docs/reviews/bug_inspection_report_2026_11_20.md](docs/reviews/bug_inspection_report_2026_11_20.md)。
