# AGENT.md — 本项目协作约定

本文件面向在本仓库工作的 AI coding agent（Lead 与子代理），补充 `docs/README.md`（文档总索引）与 `docs/godot/` 下的当前入口。旧 `docs/archive/rust/NEW_SESSION_START_RUST_2026-11-04.md`、旧 HANDOFF 和旧研究只作历史证据。

> **当前阶段（2026-11-15）：Godot 重制版 M2 已收口，进入 M3（战斗与 H）。**
> 当前入口是 [`docs/godot/README.md`](docs/godot/README.md)、
> [`docs/godot/M1_HANDOFF.md`](docs/godot/M1_HANDOFF.md)、
> [`docs/godot/DECISIONS.md`](docs/godot/DECISIONS.md) 与
> [`docs/godot/PLAN.md`](docs/godot/PLAN.md)。
> 本文件 §4 的 Rust 命令不再作为 Godot 阶段的交付门禁，只用于维护开发期 Oracle；
> 旧 `HANDOFF`、旧 `PLAN`、旧 `ENGINE_STRATEGY` 和 `docs/research/` 只作历史证据。

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

> Godot 交付门禁 30 道，headless 与窗口模式双跑；任一道失败即不是可交付状态。
> Rust 命令只用于维护开发期 Oracle，不能代替 Godot 数据/行为/画面验收。

```bash
cd '/e/Study/github program/imoto life'
DOTNET_ROOT=E:/Study/tools/dotnet8 /e/Study/tools/dotnet8/dotnet build godot/Lwsmf.csproj -v:minimal
bash tools/godot_check.sh   # 预期 GODOT_GATE_OK（12/12）
```

Rust Oracle（仅开发期）：

```bash
source tools/env.sh >/dev/null 2>&1
cargo test -q --workspace
cargo build -q -p wolf-cli
./target/debug/wolf-cli.exe verify   # 预期 PASS 70, pending 0
```

- 提交信息用 `git commit -F work/_commit_*.txt`；**每次提交后必须立即推送**：`git -c http.sslBackend=openssl push origin master`（网络/凭据失败时保留本地提交并报告，不重写历史）。
- 不跑 `git clean`；不杀用户/编辑器启动的进程；不通过 shell 启动 Godot。
- 集成前 Lead 必须检查 `git status` / `git diff`，确认没有临时探针文件被误提交。
- 每次交付必须保持工作树可解释：只允许 3 个永久 scratch、`docs/gpt调研/`（用户资料）和明确声明的临时文件。

## 5. 事实优先级

1. `docs/godot/DECISIONS.md`（技术栈、存档、内容范围与差异政策）；
2. `docs/godot/PLAN.md` 与根 `PLAN.md`（当前阶段和下一步）；
3. 当前 Godot 工程/测试的实际输出（`tools/godot_check.sh` 12/12、`GODOT_GATE_OK`）；
4. `docs/README.md` 标记为“迁移输入”的格式、opcode、文本、战斗、H、地图规格；
5. Rust 源码/测试与 `docs/godot/LEGACY_RUST_AUDIT.md`（开发期 Oracle/风险证据）；
6. 历史 HANDOFF、`BASELINE.json`、`M1_CLOSURE`、`M2_SLICE`、`docs/research/*` 只作追溯；
7. 旧聊天摘要、攻略和外部调研仅作线索，不作为权威。

禁止用旧 `PLAN.md` 的 Rust 里程碑、旧 `ENGINE_STRATEGY` 的“不迁移”结论或原始审计中的纯 GDScript 草稿覆盖当前决策。

## 6. 文档维护与里程碑后清理（强制）

### 6.1 文档维护

- **单一入口**：`docs/README.md` 是唯一文档总索引；任何新增/移动/删除文档都必须同步更新它。
- **单一决策源**：Godot 技术栈、存档边界、内容范围先改 `docs/godot/DECISIONS.md`，再改实现和路线。
- **单一路线图**：完整 Godot 路线只在 `docs/godot/PLAN.md`；根 `PLAN.md` 只做摘要/跳转。
- **现状与历史分开**：Godot 新文档放 `docs/godot/`；旧 `HANDOFF_M3..M13`、`M1_CLOSURE`、`M2_SLICE`、`docs/research/` 冻结为历史证据，不再追加“下一步”。
- **里程碑收口**：M0 起新增 `docs/godot/M0_HANDOFF.md`（或对应阶段）→ 更新 `docs/godot/PLAN.md` → 更新 `docs/README.md`；不要再写新的 Rust `HANDOFF_Mx`。
- **文档必须可复现**：技术文档写清状态、日期/HEAD、数据根和复现命令；无法验证的写“未验证/需要补充材料”。
- **归档不删除**：旧路线和旧策略移动到 `docs/archive/`，加归档横幅；不要用当前口吻重写历史。

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

## 7. 协作授权与外部核实

- **提交/推送无需逐次征求用户同意，且每次提交后直接推送**：Lead 在完成当前批次、通过验收门禁并检查 `git status` / `git diff` 后，可自行 commit，并在同一批次内立即 push；提交信息与分支沿用 §4。推送失败时保留本地提交，报告凭据或网络原因，不重写历史。
- **需要联网核实的问题默认交给用户用 Gemini 调研**：Lead 不自行展开大规模网络检索；把问题、已核实事实、需要的证据和期望输出写成清单交给用户。仅当问题能在本仓库、Rust Oracle 或已抓取资料内确认时才自行查证。
