# 自动化开发与协作约定（AGENTS.md）

本文件面向在本仓库工作的 AI 智能体（Lead、Subagents 与自动化协作工作流），补充 [docs/README.md](docs/README.md)（文档总索引）与 [docs/godot/README.md](docs/godot/README.md) 下的现行技术规范。

---

## 1. 核心工作模式与并行推进原则

- **多任务并行派发**：存在多个独立子任务时，默认优先派发子代理（Subagent / Teammate）并行推进，避免串行等待阻塞进度。
- **职责划分**：
  - 子代理负责特定模块/文件的只读研究、缺陷修复、单元测试或文档编写；
  - 主代理（Lead）负责全局任务拆解、各分支审查、全量测试回归与最终集成。
- **写作用域隔离**：各并发任务必须在启动前明确界定写入文件范围（Write Scope），杜绝多人同时修改同一文件导致冲突。

---

## 2. 写入与资产隔离护栏

- **禁止提交未授权商业资产**：严禁将原始游戏解包数据、`*.wolf` 文件、商业音乐或 CG 提交至版本库（已在 `.gitignore` 声明排除）。
- **临时生成物隔离**：测试探针、内存转储、临时调试脚本必须放置在 `tools/` 或 `work/out/`，并在任务结束时清理。
- **编码与换行**：遵循统一的 LF 换行风格与 UTF-8 编码，统一由 [.editorconfig](.editorconfig) 约束。

---

## 3. 交付门禁与测试规范

任何核心逻辑或呈现层代码的合并，必须通过以下本地自动化门禁：

```bash
# 1. 编译 Godot C# 核心库
dotnet build godot/Lwsmf.csproj -c Release

# 2. 执行 38 项核心门禁测试
bash tools/godot_check.sh
```

> **注意**：旧 Rust 代码（`crates/`）仅作为开发期 Oracle 比对参考，不再作为 Godot 运行时的直接交付物。

---

## 4. Git 提交与推送约定

- **分阶段提交**：每次完成部分修改任务并通过对应校验后，应及时在本地执行 `git commit`（只暂存当次改动文件，严禁提交 `work/`、解包资产或临时生成物）；
- **回合结束推送**：在一轮对话全部任务完成、通过交付门禁并清理临时文件后，应自主执行 `git push` 将提交推送至远端仓库。

---

## 5. 文档维护约定

- 新增架构决策必须记录于 [docs/godot/DECISIONS.md](docs/godot/DECISIONS.md)；
- 缺陷审查与修复报告必须归档至 [docs/reviews/](docs/reviews/)；
- 必须保持 [docs/README.md](docs/README.md) 的索引链路有效，杜绝产生死链。
