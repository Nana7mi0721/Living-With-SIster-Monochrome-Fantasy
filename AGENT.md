# AGENT.md — 本项目协作约定

本文件面向在本仓库工作的 AI coding agent（Lead 与子代理），补充 `docs/README.md`（文档总索引）、`docs/NEW_SESSION_START.md` 与最新的 `docs/HANDOFF_M10.md`。

## 1. 默认并行推进

- **有多个独立任务时，默认先派发子代理并行执行，不要串行等一个做完再开下一个。** 把能独立定义、独立验收的工作拆成共享任务（team_task_create），指定清晰 write scope，然后启动或复用空闲 teammate。
- 主代理（Lead）在等待子代理期间应继续做**不冲突的**工作：读代码、写文档、准备集成脚本、跑与子代理文件范围不重叠的测试；不要空等。
- 只需要读/研究、或只写互不重叠新文件的任务，优先并行；需要改同一文件的写入必须串行或拆清范围。
- 子代理只写任务声明的文件；Lead 负责最终集成、全量测试、verify、提交与推送。

## 2. 需要并行的信号

- 用户明确说“并行推进/多用子代理/加快效率”。
- 任务板出现了互不依赖的当前里程碑工作流（战斗、H 场景、日程、地图、UI 等）。
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
- 每次交付必须保持工作树可解释：只允许 3 个永久 scratch、`docs/gpt调研/`（用户资料）和明确声明的临时文件。

## 5. 事实优先级

1. 仓库当前源码与实测命令输出；
2. `docs/README.md` 中标记为**现行/稳定参考**的文档：`docs/HANDOFF_M10.md`、`PLAN.md`、`docs/BASELINE.json`、技术规格与经测试验证的 `docs/research/*`；
3. 历史归档文档（`HANDOFF_M3`–`HANDOFF_M9`、`M1_CLOSURE.md`、`M2_SLICE.md`、`docs/archive/`）只作追溯，不作为当前指令；
4. 旧聊天摘要仅作线索，不作为权威；
5. 外部调研（GPT/攻略）与实测冲突时，以实测为准。

## 6. 文档维护与里程碑后清理（强制）

### 6.1 文档维护

- **单一入口**：`docs/README.md` 是唯一文档总索引；任何新增/移动/删除文档都必须同步更新它。研究底稿还要登记到 `docs/research/README.md`。
- **现状与历史分开**：最新交接只保留 `docs/HANDOFF_M10.md`（下一个里程碑则 `HANDOFF_M11.md`）；旧交接、旧验收基准统一标为“历史归档”，保留不删。
- **每次里程碑收口必须同步**：`docs/HANDOFF_Mx.md`（现状/证据/边界/下一步）→ `PLAN.md`（里程碑表与 M11+ 优先级）→ `docs/BASELINE.json`（本批证据）→ `docs/README.md` 与 `docs/research/README.md`。
- **文档必须可复现**：技术/研究文档开头写清状态（现行/已验证/未验证/历史）、测量 HEAD、数据根（`work/all` / `work/full`）和复现命令；不要只写结论。
- **路线图不双轨**：`PLAN.md` 只放当前路线；旧规划稿放 `docs/archive/`，不在根目录保留过期版本。

### 6.2 里程碑完成后清理项目文件（避免文件夹膨胀）

每个里程碑完成后，Lead 必须做一次磁盘体检并清理，报告清理前后大小：

```bash
du -sh target work .git docs 2>/dev/null
git worktree list
```

清理清单：

1. **构建缓存**：`target/` 通常 10 GB 以上。若近期还要继续开发，优先把 `CARGO_TARGET_DIR` 指到仓库外（例如 `E:/cargo-target/imoto`），再删除仓库内旧 `target/`；若暂时不构建，可执行 `cargo clean`。清理前确认没有正在跑的构建/测试。
2. **旧 worktree**：`git worktree list` 中已合并、无未提交改动的旧 agent worktree（如 `.claude/worktrees/*`）可以整体删除；删除前逐个确认状态，**有未合并提交的不得删除**。
3. **重复解包/临时构建目录**：删除 `work/out/*_target`、旧的 `work/alldata` / `work/unpack*` / `audio_probe` 等重复副本（先确认与 `work/all` 文件集一致），以及临时探针可执行文件。
4. **work/out 产物**：删除不再被文档/测试引用的中间截图、日志、临时 patch；保留关键验收截图及其对应记录。不要为了省空间删除 `work/all`、`work/full`、`work/imouto`、`work/retail_backup`。
5. **临时文件**：删除 `tmp_*`、`_scratch_*`（永久 scratch 除外）、`*.bak`、失败日志等；确认 `git status` 只剩 3 个永久 scratch、`docs/gpt调研/` 和明确声明的临时文件。
6. **报告**：在 HANDOFF/提交说明里记录清理前后 `du -sh .` 与删除类别；不要静默清理用户文件。

### 6.3 删除安全

- 删除/移动前先确认解析后的绝对路径是你想操作的目标；不要对未确认的变量路径执行递归删除。
- 不删除任何已跟踪文档：过期文档移到 `docs/archive/`；不删除 `docs/gpt调研/`（用户资料）。
- 不跑 `git clean`；不清理用户自己的游戏安装、截图和未跟踪工作。
