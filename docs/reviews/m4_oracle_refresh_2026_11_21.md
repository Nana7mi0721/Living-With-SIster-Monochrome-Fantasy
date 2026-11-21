# M4 Oracle 夹具刷新与双模式门禁恢复报告（2026-11-21）

本报告记录 `M4FullRouteTrace` 在 35/38 处失败的定位、夹具刷新脚本修复与完整门禁复核。

---

## 1. 故障现象

- `tools/godot_check.sh` 仅剩 `35/38 M4FullRouteTrace` 失败，C# 测试本身输出 `M4_FULL_ROUTE_TRACE_TEST_OK`。
- `reports/m4_route_diff.json` 报告首次分歧位于第 `362832` 步：地图 `1005`、事件 `406`、`pc=5`、opcode `121`。
- 旧夹具在该步的 `vars` 为空，C# 写入 `vars[399]=2000400`；旧夹具总计 `939630` 步 / `7663` 帧，当前 C# 为 `990102` 步 / `8056` 帧。

## 2. 根因

1. `oracle/fixtures/m4_full_route_rust_trace.json.gz` 与旧 `reports/rust_m4_route_trace.json` 内容哈希相同，生成时间为 2026-11-20 16:25。
2. `54687c0` 在 2026-11-21 02:43 同时修正 Rust 与 C# 的 opcode `121` 语义：bit `0x04` 表示参数 a 是字面量，不再作为变量地址求值。旧夹具生成于该修复之前，因此在 CE406 的该分支缺少写入并提前结束。
3. `tools/refresh_m4_oracle.sh` 原先只做了语法检查，未实际跑通：
   - `route-full` 未传 `--root`，默认读取不存在的 `work/full`，而本机数据根是 `work/all`；
   - 只载入 `GODOT`/`DOTNET` 变量，未设置 Godot 所需的 `DOTNET_ROOT` 与 `PATH`，会误调用无 SDK 的系统 `dotnet`；
   - 压缩夹具时直接写目标路径，对拍或压缩中断可能破坏已有夹具。
4. 先前记录的 `cargo build --release` “启动 rustc 失败 0xc0000142”经本轮复现，实际是 `rc.exe` 嵌入程序图标失败 `0xc0000142`。Cargo 将其作为非致命警告，Release 构建正常完成；该警告不影响轨迹生成。

## 3. 修复

- `tools/refresh_m4_oracle.sh` 显式解析数据根：优先 `WOLF_DATA_ROOT`，否则自动选择 `work/all`，再回退 `work/full`；并校验 `BasicData/CommonEvent.dat`。
- 脚本接入本机 .NET SDK 的 `DOTNET_ROOT` 与 `PATH`，与 `tools/godot_check.sh` 保持一致。
- Rust 侧改为 `wolf-cli --root <DATA_ROOT> route-full`，不再依赖默认数据根。
- 夹具压缩改为同目录临时文件加 `os.replace` 原子替换；只有 Rust/C# 对拍 100% 一致才执行替换。
- 重新生成并提交 `oracle/fixtures/m4_full_route_rust_trace.json.gz`。

## 4. 验证证据

| 检查 | 结果 |
| --- | --- |
| Rust 轨迹 | `M4_ROUTE_OK steps=990102`，8056 帧，终态 day 5 08:30 |
| C# 轨迹 | `M4_FULL_ROUTE_TRACE_TEST_OK steps=990102`，8056 帧 |
| 逐字段对拍 | `ROUTE_DIFF_OK: 990102/990102 steps matched 100%` |
| 轨迹 SHA-256 | Rust 与 C# 均为 `bd50751d0dbf6c405bbf96569b69ef7f6553669195995b9e471aa0f6e498fafb` |
| C# Release 构建 | `0` 个警告、`0` 个错误 |
| 完整无头门禁 | `GODOT_GATE_OK elapsed=94s`；M4 任务 37s；M13 slot 1/8 通过 |
| OpenGL3 渲染门禁 | `GODOT_GATE_OK elapsed=90s`；M4 任务 34s；含渲染截图测试 |
| 夹具刷新脚本 | `M4_ORACLE_REFRESH_OK`，耗时 1m54.5s |

## 5. 已知边界

- 本机 Windows SDK 的 `rc.exe` 仍以 `0xc0000142` 退出，导致 `wolf-cli` Release 构建无法嵌入程序图标并输出一条 Cargo 警告；二进制构建、启动与 Oracle 轨迹均不受影响。
- `crates/wolf-cli/src/route.rs` 的 `Teleport.options` 仍触发既有 dead-code 警告；不属于本次 M4 夹具修复范围。
