# AGENT.md — 本项目协作约定

本文件面向在本仓库工作的 AI coding agent（Lead 与子代理），补充 `docs/NEW_SESSION_START.md` 与各 `docs/HANDOFF_*.md`。

## 1. 默认并行推进

- **有多个独立任务时，默认先派发子代理并行执行，不要串行等一个做完再开下一个。** 把能独立定义、独立验收的工作拆成共享任务（team_task_create），指定清晰 write scope，然后启动或复用空闲 teammate。
- 主代理（Lead）在等待子代理期间应继续做**不冲突的**工作：读代码、写文档、准备集成脚本、跑与子代理文件范围不重叠的测试；不要空等。
- 只需要读/研究、或只写互不重叠新文件的任务，优先并行；需要改同一文件的写入必须串行或拆清范围。
- 子代理只写任务声明的文件；Lead 负责最终集成、全量测试、verify、提交与推送。

## 2. 需要并行的信号

- 用户明确说“并行推进/多用子代理/加快效率”。
- 任务板出现了互不依赖的 M7 工作流（战斗、H 场景、日程、地图、UI 等）。
- 当前路径被一个长测试/长构建阻塞，但其他工作流可以直接开始。
- **Token/额度预算充足或即将到期时，优先多派发有明确验收标准的子任务，避免额度浪费。**

## 3. 写入与文件边界

- 每个任务写 scope（team_task_create 的 write_scopes）要具体到文件/目录；多人同时写同一文件会互相覆盖。
- 永久 scratch，不得提交：`crates/wolf-format/examples/opscan.rs`、`crates/wolf-format/tests/_scratch_cdata.rs`、`crates/wolf-format/tests/_scratch_event.rs`。
- `docs/gpt调研/` 是用户资料，不提交；临时 example/日志/截图写 `work/out/` 或提交前删除。
- 改 `crates/wolf-vm/src/exec.rs` 前先核对实际换行（实测 LF，旧文档 CRLF 说法过时）。

## 4. 集成与验收入口

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
cargo test -q --workspace
cargo build -q -p wolf-cli
./target/debug/wolf-cli.exe verify   # 预期 PASS 70, pending 0
```

- 提交信息用 `git commit -F work/_commit_*.txt`；推送用 `git -c http.sslBackend=openssl push origin master`。
- 不跑 `git clean`；不杀用户/编辑器启动的进程；不通过 shell 启动 Godot。
- 集成前 Lead 必须检查 `git status` / `git diff`，确认没有临时探针文件被误提交。

## 5. 事实优先级

1. 仓库当前源码与实测命令输出；
2. `docs/HANDOFF_M6.md` / `docs/BASELINE.json` / `docs/research/*` 中经测试验证的结论；
3. 旧聊天摘要仅作线索，不作为权威；
4. 外部调研（GPT/攻略）与实测冲突时，以实测为准。
