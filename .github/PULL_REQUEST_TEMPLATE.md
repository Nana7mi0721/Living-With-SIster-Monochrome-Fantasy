## 变更概述
简要描述本 PR 的核心改动内容、动机及所解决的问题。

## 涉及模块
- [ ] `godot/core_cs/` (C# 确定性事件虚拟机与状态机)
- [ ] `godot/presentation_gd/` (GDScript 渲染表现层与视口管理)
- [ ] `converter/` (WOLF 字节码离线转换流水线)
- [ ] `crates/` (Rust 原型解析器与 Oracle 验证基线)
- [ ] `docs/` / `tools/` (文档、规范与构建工具链)

## 验证与测试
- [ ] `dotnet build godot/Lwsmf.csproj` 编译通过（无 Error/Warning）
- [ ] `bash tools/godot_check.sh` 自动化门禁测试通过（38/38）
- [ ] （可选）`cargo test -q --workspace` Rust 基础单元测试通过

## 检查项
- [ ] 代码风格符合 `.editorconfig` 规范
- [ ] 未提交任何原始受限商业游戏资产（`work/Data/` 或未授权 `.wolf` 文件）
- [ ] 说明文档与代码注释使用中性客观的技术语言
