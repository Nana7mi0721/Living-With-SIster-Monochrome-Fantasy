# 《妹相随～黑白世界的缤纷冒险～》（imoto-life）Godot 4 重制版总体迁移规划与遗留 Rust 缺陷审计报告

> **原始审计归档 / 不是当前路线图**
>
> 本文件保留 Gemini 原始报告，便于追溯事实审计、Rust 缺陷证据和架构讨论。
> 其中的 **§0 与 §9** 反映 2026-11-14 的 C# 核心 + GDScript 表现层决策；
> **§4.2、§5.2 ADR-01/ADR-05、§7.3 多语言和任何“纯 GDScript / Godot 4.3+ / 不读 .sav”**
> 都是较早草稿，已被 [`../DECISIONS.md`](../DECISIONS.md) 覆盖。
> 当前规划见 [`../PLAN.md`](../PLAN.md)，风险护栏见 [`../LEGACY_RUST_AUDIT.md`](../LEGACY_RUST_AUDIT.md)。

> **生成时间**：2026-11-14（Asia/Tokyo）
>
> **审计对象**：`imoto_life_context/`（20 个压缩包，经 `SHA256SUMS.txt` 校验全部通过）
>
> **运行时约束**：最终游戏运行时**纯 GDScript**（不保留 Rust/GDExtension），保留原版 **550 个公共事件**与完整剧情，允许重新设计资源与存档格式，离线转换器可使用 Python。

## 0. 已拍板项目约束（2026-11-14 用户确认定稿）

| 决策维度 | 最终确认方案 | 对本规划的直接影响 |
| --- | --- | --- |
| **引擎与目标平台** | **Godot_v4.7.2-stable**，仅面向 **Windows x64** | 无需兼顾移动端/Web 导出限制，可用 .NET/C# 与桌面原生特性 |
| **运行时语言** | **C# 核心 + GDScript 表现层**（去 Rust，不强制纯 GDScript） | 彻底摆脱 Rust `target/` 数百 MB 编译缓存，且原生 `int` 即为 32 位有符号整数，天然契合 WOLF 算术、PRNG 与 `.sav` 二进制兼容 |
| **原版资产与存档** | 拥有完整零售数据；**尽量兼容原版 `.sav` 存档** | 采用“**原版 `.sav` 兼容导入 + 完整状态快照**”双轨策略：C# 直接移植 `wolf-save` 解密/解析以读取原版 8 槽存档，同时保存完整 VM 协程/RNG/音频快照 |
| **内容与缺陷处理** | **一次性包含全部 9 结局 + DLC 链**；原版怪癖**立即美化/补全** | `UI-12`（19:30 方针面板）升级为 `Enhance-in-Godot` 重排；`CE333` 坏结局分支纳入 M3/M4 评估与补全 |
| **文本与语言** | **第一版仅保留官方中文** | 离线转换器直接输出中文终稿，省去运行时多语言覆盖切换复杂度 |
| **验证与 Oracle** | **保留 Rust Oracle 至 M4**；允许本地实机录屏/抓包验证 | 优先补齐 `opcode 123` 按键掩码（`H-KEY-2`）与 `CE159/162` 战斗调度实机证据 |

---

## 1. 执行摘要

**核心结论**：完全可行，且**推荐采用“Python 离线转换器 + 纯 GDScript 显式状态机 VM + 自绘 101/102 窗口 + 版本化 JSON 存档 + 开发期 Rust Oracle 对拍”**的架构路线。当前 Rust 实现（`crates/` 共 216 个 `.rs` 文件、92,977 行）在格式逆向、opcode 语义、UI 图层与 PRNG 方面积累了极高价值的规格资产，但也存在**非原子写档、存档缺失 VM 协程与 RNG/音频状态、32 位整数溢出依赖、无界资源缓存、路径校验缺失、以及大量为绕过旧架构而打的补丁（如 `room_base_pictures` 快照和负层延迟 4 帧清理）**。

通过将 `.wolf/.dat/.mps` 解析与 `branch_depth` 预编译彻底前移至离线 Python 转换器，Godot 运行时只需专注于**确定性事件执行、状态管理与表现层渲染**。预计单人全职开发 **22–32 周（约 5.5–8 个月）** 可完成从数据转换到全内容回归发布；**2–3 人小队约 12–18 周（3–4.5 个月）**。首个可玩纵向切片（标题 → 开场 → 第一天 → 存读档）可在 **3–5 周** 内跑通。

## 2. 事实、假设与未知项审计

### 2.1 已确认事实（引用包内路径）

1. **仓库现状**：截至 2026-11-13，仓库中**不存在任何 Godot 工程文件**（无 `project.godot`、`.gd`、`.tscn`），当前可运行实现为 7 个 crate 组成的 Rust workspace（`Cargo.toml`：`wolf-format`、`wolf-save`、`wolf-vm`、`wolf-render`、`wolf-audio`、`wolf-game`、`wolf-cli`）。

2. **代码规模（实测）**：`crates/` 下共 **216 个 `.rs` 文件，总计 92,977 行**（`wolf-game` 53,469 行、`wolf-format` 10,521 行、`wolf-vm` 8,055 行、`wolf-cli` 7,159 行、`wolf-save` 5,970 行、`wolf-render` 4,602 行、`wolf-audio` 3,201 行）。*注：任务书 §2 提及的 86,823 行为早期快照数字，当前代码库已增长约 6,154 行。*

3. **数据与指令基线（`docs/BASELINE.json`）**：

   * 公共事件 **550 个**，共 163,828 个 command slot，其中 **139,273 条真实指令**（24,555 空槽），涉及 **47 种**非空公共事件 opcode；运行条件分布为 `0 (call_only)=442`、`1=1`、`2 (parallel)=97`、`3 (parallel_always)=10`（合计 107 个并行事件）。

   * 地图事件 **73 个**、134 页、37,610 槽（**30,558 条真实指令**），涉及 40 种 opcode；全库合计 **169,831 条真实指令**。

   * 逻辑分辨率 **1024×576**，tile 16px，60fps；前 14 个高频 opcode 覆盖约 93.9% 指令。

4. **UI 修复与历史决策（`docs/archive/rust/UI_BUG_BACKLOG_2026-11-04.md`、旧 `PLAN.md`、`docs/archive/rust/ENGINE_STRATEGY_2026-11-04.md`）**：

   * `UI-01..UI-13` 已于 2026-11-04 标记为 PASS，集成测试记录为 `153 test-result blocks / 957 passed / 0 failed / 14 ignored`。

   * 旧 `PLAN.md` 中的“项目终止”与 `docs/archive/rust/ENGINE_STRATEGY_2026-11-04.md` 中的“不迁 Godot”属于旧决策，已被用户最新“Godot 4 原生重制版”决策覆盖。

5. **待验证与真残余（`docs/UNVERIFIED.md`）**：仍存在 `H-PIC-1`（负番号全屏）、`H-KEY-2`（`123` 掩码与键码 `13/14/20/100/300`）、`H-ASSET-1`（事件 354 的 7 处 `Picture/word_effect*` 笔误）、独立 Y 轴缩放缺失（`H-PIC-2`）、以及 `CE159/CE162` 战斗目标调度和 `CE333` 坏结局不可达等问题。

### 2.2 关键假设（标注为“假设”）

* **假设 A1**：目标 Godot 版本为 **Godot 4.3+（推荐 4.3 Stable 或 4.4）**，渲染器默认使用 **Forward+ / Compatibility**（2D 项目优先 Compatibility 以获得最佳老旧核显兼容性与快速启动）。

* **假设 A2**：用户拥有原版完整零售游戏数据（约 2.6 GB，含 729 MB 媒体），可在本地运行 Python 离线转换器生成 `game_data/`。

* **假设 A3**：Rust 工具链仅用于开发期本地跑通 oracle 对拍，不随最终游戏发布。

### 2.3 未知与冲突项

* **沙箱未执行项**：当前远程沙箱未安装 `cargo` / `rustc`，**`cargo test` 与 `wolf-cli verify` 未执行**，所有运行时行为均基于源码静态审计与文档/截图证据交叉核对。

* **文档与源码小幅漂移**：任务书基线代码行数为 86,823 行，实际快照为 92,977 行；`UI_BUG_BACKLOG_2026-11-04.md` 标记 UI-01..13 全部 PASS，但源码中多个修复依赖特定状态机补丁（如 `room_base_pictures`、`generation` 延迟清理），在复杂边界下仍有脆弱性。

* **缺失数据依赖**：完整 2.6 GB 零售资源、`docs/攻略文档/` PDF 与 Windows 实机 `GetTickCount` 抓包未在上传包中，部分视觉/音频细节需在本地零售数据上复核。

### 2.4 对迁移影响最大的 5 个技术事实

1. **WOLF 整数是 32 位有符号补码（`i32`），而 GDScript `int` 是 64 位有符号整数**：`crates/wolf-vm/src/exec.rs:1550-1575` 使用 `wrapping_add/sub/mul` 与 `(b & 31)` 位移，且 Park-Miller MINSTD PRNG（`exec.rs` + `rng.rs`）依赖 32 位溢出行规。GDScript 若不显式做 32 位截断（`wrap_i32`），数学运算与随机数将发生静默漂移。

2. **当前 Rust 存档未保存 VM 协程与 RNG/音频状态**：`crates/wolf-game/src/game.rs:676-758` 与 `savegame.rs:86-188` 只存变量、CData 和图片表，靠 `auto_events.prime_latched` 强行重拉 HUD 事件 216/217。Godot 新存档必须序列化完整 `event_instances`（`program_id`, `pc`, `call_stack`, `wait`, `loops`, `self_vars`）及 RNG/音频状态。

3. **107 个并行事件 + 442 个 `call_only`（含 `Wait`）不能用 GDScript `await` 协程**：`await` 无法序列化进存档，且同帧执行顺序不可控。必须采用数据驱动的**显式状态机 VM**。

4. **`textpic.rs` 的半角/全角估算导致像素漂移，而 Godot `Font.get_string_size()` 可直接根治**：`textpic.rs:60-75` 记录了估算宽度（60px）与实际 `Lolita.ttf` 宽度（56.31px）的偏差；迁入 Godot 后自绘 `_draw()` 可直接使用真实字体度量。

5. **WOLF 渲染依赖严格的 9 级图层带与自定义 9 宫格皮肤**：`Map0 < ScenePicture10 < SceneText11 < MessageFrame20 < MessageText25 < Nameplate30 < ChoicePanel40 < ChoiceText41 < Cursor50`，且不能为每个图片槽常驻 `Sprite2D`。

## 3. 遗留 Rust 缺陷与迁移风险审计

> **审计声明**：沙箱无 Rust 工具链，`cargo test` / `cargo clippy` **未执行**；以下结论均基于对 `crates/` 源码的静态逐行审计与 `docs/`/`work/out/` 证据交叉验证。

### 3.1 Top 10 迁移阻塞项

 1. **`RUST-001`（P0）存档缺失 VM 协程、RNG 与音频状态（仅存变量与图片快照）**

    `Game::save_to`（`crates/wolf-game/src/game.rs:676-758`）与 `savegame::apply`（`savegame.rs:86-188`）不保存 `vm.instances`（`pc`、`WaitState`、`call_stack`、`loops`）、`vm.rng` 和 BGM/BGS 状态，读档后靠 `auto_events.prime_latched` 硬拉 216/217 HUD 事件。若照搬到 Godot，任何在 `Wait`/嵌套调用/剧情中途的存档都会丢进度或重放事件。**必须在 M0/M1 存档 Schema 设计阶段彻底重构，纳入完整实例与状态快照。**

 2. **`RUST-002`（P0）非原子存档写入导致掉电/崩溃直接毁档**

    `Game::save_to`（`game.rs:752`）与 `savegame::write_save`（`savegame.rs:54`）直接调用 `std::fs::write(path, &bytes)` 覆写目标 `.sav`。若写入中途崩溃、磁盘满或断电，原存档直接截断损坏。**必须在 M1 `SaveService.gd` 阶段强制采用“写 `.tmp` → `flush` → 原子 `rename` + 保留 `.bak`”机制。**

 3. **`RUST-003`（P1）GDScript 64 位 `int` 与 WOLF 32 位 `i32` 溢出语义冲突**

    `wolf-vm/src/exec.rs:1550-1575` 明确使用 `i32::wrapping_add/sub/mul/div/rem` 和 `(b & 31)` 位移，Park-Miller PRNG（`rng.rs`）亦基于 32 位运算。GDScript 的 `int` 为 64 位有符号整数，直接翻译会导致溢出结果、负数位移与随机序列完全不一致。**必须在 M1 `EventVM` 核心运算层引入 `wrap_i32()` 辅助函数。**

 4. **`RUST-004`（P1）`MAX_INSTANCES` 静默丢弃子调用且不阻塞父协程**

    `exec.rs:1310-1313` 当活跃实例数达到 `MAX_INSTANCES` 时仅记录日志并 `return`，既不执行子事件，也不阻塞调用方（`wait` 未设），导致父事件在未获得返回值的情况下继续执行，引发剧情/数值静默错乱。**必须在 M1 VM 调度阶段改为明确报错/熔断机制。**

 5. **`RUST-005`（P1）`Signal::Error` 静默跳过指令导致 PC 漂移**

    `exec.rs:1282` 遇到执行错误时仅执行 `self.instances[i].pc += 1;` 强行跳过。在 Godot 中若遇到未知 opcode 或非法参数，绝不能静默跳过，**必须在 M0 离线转换器阶段拦截未知 opcode，并在 M1 运行时严格 fail-fast + 输出 trace。**

 6. **`RUST-006`（P2）`SaveTarget::Path` 存在路径穿越与任意文件覆写风险**

    `game.rs:2673-2687` 的 `save_path()` 直接拼接用户/脚本传入的 `PathBuf`，若为绝对路径或含 `..` 则可读写游戏根目录之外的文件。**必须在 M0/M1 将所有存档路径限制在 `user://saves/` 白名单槽位内，禁止任意路径拼接。**

 7. **`RUST-007`（P2）`wolf-format` 多处未校验长度即执行 `Vec::with_capacity`（OOM DoS）**

    `gamedat.rs:81-82`（`u16_len`）和 `dxarchive/mod.rs:421`（`dest_size`）在未校验剩余字节或上限的情况下直接 `Vec::with_capacity`，畸形输入可触发数 GB 内存分配崩溃。**必须在 M0 Python 转换器中对所有长度字段施加文件大小上限校验。**

 8. **`RUST-008`（P1）多处使用 `String::from_utf8_lossy` 造成文本静默损毁**

    `bytes.rs:192`、`tileset.rs:146`、`db.rs:525`、`savegame.rs:194,445`、`wolf-save/src/lib.rs:241,1275` 对非 UTF-8 字节做有损替换（生成 \`\`）。**必须在 M0 转换器中显式按 CP932（Shift-JIS）/GBK/UTF-8 规则严格解码并在失败时报错，绝不将坏字符带入 Godot JSON。**

 9. **`RUST-009`（P1）立绘/负层/标题菜单依赖脆弱的运行时补丁而非状态机建模**

    `game.rs:1419-1467` 靠延迟 4 帧清理负层修复黄绿闪（UI-05）；`game.rs:1474-1600` 靠 `room_base_pictures` 快照硬恢复夜晚妹妹立绘（UI-11）；`UI-03` 靠特殊 `parked Choice` 重建标题菜单。这些启发式补丁极易在新场景失效。**必须在 M2 表现层与 `InputRouter` 中用显式场景/图层生命周期替代。**

10. **`RUST-010`（P3）`assets.rs` 图片解码缓存完全无界（内存悬崖）**

    `crates/wolf-game/src/assets.rs:189,227` 明确注释缓存为“deliberately unbounded”，729 MB 媒体全解码 RGBA 后在长流程中会持续膨胀至数 GB。**必须在 M2 `AssetDB.gd` 中实现按场景卸载 + LRU 内存预算淘汰（如上限 256–512 MB）。**

### 3.2 遗留 Rust 缺陷与风险完整审计表（22 项）

| ID | 状态 | 严重度 | 迁移处置 | 位置 | 触发/复现 | 现象与影响 | 根因 | 原版还是 Rust | Godot 护栏 | 建议修复 | 
| ----- | ----- | ----- | ----- | ----- | ----- | ----- | ----- | ----- | ----- | ----- | 
| `RUST-001` | confirmed | P0 | `do-not-carry-over` | `wolf-game/src/game.rs:676-758`, `savegame.rs:86-188` | 在 `Wait`/子事件调用中存读档 | 协程 `pc`/调用栈/RNG/音频丢失，依赖 `prime_latched` 强拉 HUD | `.sav` 格式与实现未序列化 VM 实例与 RNG | 迁移设计风险 / Rust 限制 | 新存档 Schema 必须包含 `event_instances`, `rng`, `audio` | Godot 中重构为完整状态快照 | 
| `RUST-002` | confirmed | P0 | `fix-before-port` | `wolf-game/src/game.rs:752`, `savegame.rs:54` | 写档瞬间强杀进程/磁盘满 | 原有 `.sav` 被截断清零，玩家进度永久丢失 | 直接 `std::fs::write` 覆盖原文件 | Rust 实现缺陷 | `SaveService` 强制 `.tmp` + `flush` + `rename` + `.bak` | Rust 侧可改写为临时文件后 `rename` | 
| `RUST-003` | confirmed | P1 | `encode-invariant` | `wolf-vm/src/exec.rs:1550-1575` | `121` 指令发生 32 位整数溢出或移位 | GDScript 64 位 `int` 与 Rust `i32` 算术结果不一致，破坏分支与 PRNG | GDScript `int` 为 64 位，WOLF/Rust 为 32 位补码 | WOLF 原版行为 | VM 算术封装 `wrap_i32(x)`，位移掩码 `& 31` | 保持 `i32::wrapping_*` 并写入对拍测试 | 
| `RUST-004` | confirmed | P1 | `encode-invariant` | `wolf-vm/src/exec.rs:1310-1313` | 深层递归或并发事件超 `MAX_INSTANCES` | `210`/`300` 子调用被丢弃且父协程不阻塞，剧情状态错乱 | 超限后直接 `return`，未设 `WaitState::Child` 也未中止 | Rust 实现缺陷 | 设置合理栈/实例上限，超限立即暂停并弹窗/写崩溃日志 | 超限时应显式返回错误信号而非静默返回 | 
| `RUST-005` | confirmed | P1 | `do-not-carry-over` | `wolf-vm/src/exec.rs:1278-1283` | 遇到未知 opcode 或非法命令 | 静默 `pc += 1` 跳过指令，可能导致后续分支错位或剧情缺失 | `Signal::Error` 处理分支直接步进 PC | Rust 实现缺陷 | 转换器静态拒绝未知 opcode；运行时报错停止 | 在测试模式/strict 模式下直接 fail | 
| `RUST-006` | confirmed | P2 | `guard-only` | `wolf-game/src/game.rs:2673-2687` | `220`/`221`/`222` 传入 `../` 或绝对路径 | 可读写游戏目录外任意路径文件 | `save_path` 未做路径规范化与沙箱目录校验 | Rust 实现缺陷 | 仅允许 `user://saves/slot_NN.json`，拒绝路径字符 | 校验 `canonicalize()` 必须位于 `Save/` 内 | 
| `RUST-007` | confirmed | P2 | `guard-only` | `wolf-format/src/gamedat.rs:81-82`, `dxarchive/mod.rs:421` | 损坏的 `Game.dat` 或 `.wolf` 伪造超大长度 | 触发数十 GB 内存预分配导致立即 OOM 崩溃 | `Vec::with_capacity` 前未对比 `r.remaining()` 或合理上限 | Rust 实现缺陷 | Python 转换器校验所有计数/解压大小不超过物理上限 | 解码前增加 `remaining()` 与上限检查 | 
| `RUST-008` | confirmed | P1 | `fix-before-port` | `wolf-format/src/bytes.rs:192`, `wolf-save/src/lib.rs:241,1275`, `savegame.rs:194,445` | 非 UTF-8 编码文本进入解码路径 | 文本中出现 \`\` 乱码且无任何报错日志 | 调用 `String::from_utf8_lossy` 静默替换非法字节 | Rust 实现缺陷 | 转换器严格按 CP932/GBK/UTF-8 解码，失败即终止并报告位置 | 替换为显式编码解码器并返回 `Result` | 
| `RUST-009` | confirmed | P1 | `do-not-carry-over` | `wolf-game/src/game.rs:1419-1600` | 快速切图、夜晚互动后返回、ESC 退回标题 | 依赖 4 帧延迟清负层、`room_base_pictures` 快照等启发式逻辑，易漏边界 | 缺少显式场景状态机与图层所有权模型 | Rust 实现缺陷 | Godot 中按场景树/图层生命周期显式管理立绘与背景 | 补充 `RF-03` 离开房间清理快照回归测试 | 
| `RUST-010` | confirmed | P3 | `fix-before-port` | `wolf-game/src/assets.rs:189,227` | 长时间游玩加载大量 CG/立绘 | 内存持续增长无上限（数百 MB 至数 GB） | `HashMap` 缓存无容量限制与淘汰策略 | Rust 实现缺陷 | `AssetDB.gd` 使用 LRU 缓存 + 场景切换资源引用计数清理 | 引入 LRU 上限淘汰解码纹理 | 
| `RUST-011` | confirmed | P1 | `fix-before-port` | `wolf-game/src/textpic.rs:44-46,60-75` | 带注音/图标/字距的文本图片或右对齐数字 | 宽度按全角/半角估算导致 3–5px 偏移；部分转义符被静默丢弃 | 未加载真字体时使用近似度量；未实现 `\ax`/`\ay` 等 | Rust 实现缺陷 | Godot `_draw()` 使用 `Font.get_string_size()` 真度量并支持完整 token | 在 Godot 中补齐 token 渲染器 | 
| `RUST-012` | confirmed | P2 | `preserve-for-now` | `wolf-game/src/picture.rs`, `docs/UNVERIFIED.md` (`H-PIC-2`) | 脚本独立修改 Y 轴缩放或纵向镜像 | 仅支持统一缩放与横向镜像，独立 Y 缩放被忽略 | `Picture` 结构体缺少 `scale_y` 字段 | 待验证 | 在 Godot `PictureLayer` 预留 `scale_vec: Vector2`，转换器检查是否用到独立 Y 缩放 | 实机验证后决定是否启用独立 Y 缩放 | 
| `RUST-013` | confirmed | P2 | `fix-before-port` | `wolf-game/src/textpic.rs:48-58`, `docs/UNVERIFIED.md` (`H-ANC-1`) | 文本图片使用锚点 5（上中）或 6（下中） | `textpic::anchor_origin` 仅支持 0–4，5/6 退化为左上角 | `textpic.rs` 与 `picture.rs` 锚点逻辑未统一 | Rust 实现缺陷 | Godot 统一 `AnchorUtil.compute_origin(rect, anchor_code)` 覆盖 0–6 | 复用 `picture.rs` 的 0–6 锚点表 | 
| `RUST-014` | confirmed | P2 | `fix-before-port` | `wolf-game/src/assets.rs:309-355`, `docs/UNVERIFIED.md` (`H-ASSET-1`) | 执行事件 354 引用 `Picture/word_effect30..59.png` | 依赖运行时窄 fallback 重定向至 `hword/` | 原作者路径笔误 | WOLF 原版行为 | 离线转换器在构建 `manifest.json` 时显式修正并记录映射日志 | 保持转换期修正，运行时不再保留隐式路径猜谜 | 
| `RUST-015` | needs-runtime | P1 | `encode-invariant` | `wolf-vm/src/exec.rs:658`, `docs/UNVERIFIED.md` (`H-KEY-2`) | `123` 指令传入特定位掩码或查询键码 `13/14/20/100/300` | `Host::key_code(mask)` 忽略 `mask`，且永不返回未命名键码 | 尚未完全逆向 `Game.exe` DirectInput 键码表 | 待验证 | `InputRouter.gd` 集中映射并记录未识别键码查询日志 | 本地对照原版实机验证 `123` 掩码 | 
| `RUST-016` | needs-runtime | P1 | `unknown` | `wolf-game/src/battle_wiring.rs`, `docs/UNVERIFIED.md` §四 | 战斗中触发 `CE159` 技能选择执行 | `CE188` 未触发 `CE159`，`CE162` 目标选择流程未完整闭环 | 战斗接线尚有未覆盖路径 | 待验证 | M3 阶段对照零售存档/实机录屏完成战斗全链路 oracle 对拍 | 补充战斗实机 trace 对拍 | 
| `RUST-017` | confirmed | P1 | `preserve` | `docs/UNVERIFIED.md` §四 (`CE333`) | 检查 H 场景坏结局分支 | `var157 == 13` 全库无赋值且 `v23 == 0` 在零售存档中不可达 | 原版游戏自身脚本死分支/未启用逻辑 | WOLF 原版行为 | 忠实保留原版数据与条件，不擅自修改剧情触发条件 | 无需修复，记录为原版脚本特征 | 
| `RUST-018` | confirmed | P3 | `enhance-in-godot` | `docs/archive/rust/UI_BUG_BACKLOG_2026-11-04.md` (`UI-12`) | 19:30 房间打开“方针/公会评价”面板 | `530x84` 面板内文字略显拥挤 | 原版模板布局如此（零售 Save7 同构） | WOLF 原版行为 | 按 `D-10` 允许重排并记录差异 | 单独列为 M2 UI 润色项 | 
| `RUST-019` | confirmed | P2 | `guard-only` | `wolf-audio/src/decode.rs:200-214` | 音频文件损坏或采样率为 0 / 循环点越界 | `decode.rs` 已做 `clamp/min` 保护，但无自动化音频回归 | 缺少音频管线自动化验证（`M8_BACKLOG G10`） | Rust 实现缺陷（测试缺口） | Godot `AudioManager` 校验 `loop_start < loop_end <= length`，转换器导出元数据 | 增加音频循环点与播放状态机测试 | 
| `RUST-020` | confirmed | P3 | `do-not-carry-over` | `wolf-game/src/game.rs:2116-2175` | 标题菜单鼠标悬停与键盘上下键混用 | `play.rs` 曾因静态指针每帧覆盖键盘导航（RF-01 已补） | 输入焦点与状态路由分散在各处 | Rust 实现缺陷 | `InputRouter.gd` 统一按“鼠标移动事件”而非“每帧轮询坐标”更新 hover | 保留仅移动时触发 hover 的规则 | 
| `RUST-021` | confirmed | P2 | `do-not-carry-over` | `wolf-cli/src/bin/wolf-package.rs:495-505` | 打包/解包时资源路径含异常分隔符或重名 | 需依赖 `destinations.insert` 防重，无统一虚拟路径校验 | CLI 打包器直接操作 OS 路径 | Rust 实现缺陷 | Python 转换器统一规范化为 POSIX 相对路径并检查大小写冲突 | 转换器强制小写/规范化键名校验 | 
| `RUST-022` | probable | P2 | `guard-only` | `crates/wolf-game/tests/` | 测试通过但用户实机仍报 UI bug | 结构/单元测试只测数据或单帧，未测连续交互边沿与跨场景状态残留 | 测试假阳性（缺乏多帧状态机与输入边沿回归） | Rust 实现缺陷（测试盲区） | Godot CI 引入多帧 headless 场景切换 + golden screenshot diff | 迁移时优先重写流程级端到端测试 | 

### 3.3 8 类必查缺陷覆盖说明

1. **正常游玩崩溃/卡死/无界内存**：已发现 `RUST-010`（`assets.rs` 无界 RGBA 纹理缓存）；未发现正常数据下的必然死循环（VM 有 `budget=200,000` 保护）。

2. **畸形输入解析越界/DoS/路径穿越**：已发现 `RUST-006`（`save_path` 路径穿越）、`RUST-007`（`gamedat.rs:81` / `dxarchive/mod.rs:421` 未校验容量的 `Vec::with_capacity`）。

3. **存档丢失/半写入/状态漏存**：已发现 `RUST-001`（缺 VM 协程/RNG/音频状态）、`RUST-002`（非原子 `fs::write`）。

4. **VM 语义错误/调度丢失/跳过**：已发现 `RUST-003`（32 位溢出语义）、`RUST-004`（`MAX_INSTANCES` 静默丢子调用）、`RUST-005`（`Signal::Error` 跳过指令）。

5. **静默数据损坏**：已发现 `RUST-008`（7 处 `from_utf8_lossy`）、`RUST-014`（事件 354 资源笔误）。

6. **UI/渲染状态泄漏**：已发现 `RUST-009`（负层延迟 4 帧与 `room_base_pictures` 补丁）、`RUST-011`（`textpic` 宽度估算与未实现 token）、`RUST-013`（锚点 5/6 缺失）、`RUST-020`（hover 覆盖键盘）。

7. **性能与资源悬崖**：已发现 `RUST-010`（无界缓存）；`wolf-render/src/text.rs:734` 提到字形缓存修剪会遍历全表，但在 Godot 中由引擎字体图集接管。

8. **测试假阳性**：已发现 `RUST-022`（旧 `UI_BUG_BACKLOG_2026-11-04.md` 此前多处测单帧结构而漏掉跨界面交互边沿）。

### 3.4 最低复现与测试矩阵（沙箱未执行声明）

> **执行状态**：因远程沙箱无 `cargo` / `rustc`，以下测试**未执行**。在本地 Windows/Linux 环境可执行以下验证命令：

```
# 1. 全量 Rust 单元测试与回归测试（Oracle 基线）
WOLF_GAME_DIR="$PWD/work/retail_backup" WOLF_DATA_ROOT="$PWD/work/all" cargo test -q --workspace --no-fail-fast

# 2. 针对性复现/验证已发现的边界问题
cargo test -p wolf-vm --test prng_seed --test rng_retail --test opcode_111_semantics --test opcode_124_long
cargo test -p wolf-game --test m8_anchor_slots --test m11_anchor_pixels --test m11_window_edge
cargo test -p wolf-cli --test m12_ui_input --test m12_ui_dialog --test m12_ui_layers --test m12_ui_state

```

### 3.5 行为兼容矩阵 & 禁止带入 Godot 的实现习惯

| 分类 | 具体条目 | 说明 | 
| ----- | ----- | ----- | 
| **Preserve** | 50 种 opcode 语义、32 位 `i32` 溢出与位移掩码、Park-Miller MINSTD PRNG、107 并行事件顺序、9 级图层次序、1024×576 逻辑分辨率、101/102 窗口与 `\cdb`/`\v`/`\c`/`\f` 语义、`CE333` 原版死分支 | 构成游戏内容与体验一致性的核心基线，Godot 必须严格对齐 | 
| **Preserve-for-now** | `UI-12`（19:30 方针面板拥挤排版）、`H-PIC-2`（暂只用统一缩放+水平镜像，预留 Y 缩放字段） | 第一版保持与原版一致，后续可作为高保真/润色项迭代 | 
| **Fix-in-Godot** | 原子写档（`.tmp`+`rename`）、完整存档（含 VM 实例/RNG/音频）、LRU 资源缓存、`Font.get_string_size()` 真字体度量、统一 `InputRouter`、严格编码解码、显式场景/立绘生命周期 | 属于 Rust 实现缺陷或架构包袱，Godot 中必须一次性根治 | 
| **Guard-only** | 转换器长度/容量上限校验、存档路径白名单沙箱、未知 opcode 转换期拦截、VM 调用栈深度熔断、音频循环点越界钳制 | 不改变玩法，纯增加防崩溃与防坏档护栏 | 
| **Unknown** | `123` 按键掩码 `arg[1]` 与键码 `13/14/20/100/300`（`H-KEY-2`）、`CE159/162` 战斗目标调度完整链路、非 1024×576 负番号图片缩放行为（`H-PIC-1`） | 需在本地结合完整零售数据与实机录屏/反编译补充验证 | 

**绝对不要带入 Godot 的 6 个实现习惯**：

1. **不要**用未校验的二进制长度直接分配数组；

2. **不要**在 VM 中静默跳过未知 opcode 或超限子调用；

3. **不要**将 Godot 节点树（`Node`/`Control`/`Sprite2D`）直接序列化进存档；

4. **不要**用 GDScript `await` 协程充当事件 VM 调度器；

5. **不要**只保存变量和图片表而漏存事件实例 `pc`/调用栈/`Wait`；

6. **不要**使用非原子文件覆盖写档（必须 `.tmp` + `rename`）。

## 4. 目标验收与关键技术决策

### 4.1 分层验收标准（A）

| 验收层级 | 定义 | 可测试标准 | 第一版 DoD | 
| ----- | ----- | ----- | ----- | 
| **1. 数据一致** | 550 公共事件、73 地图事件、Database、111 随机事件、9 结局、DLC 旗标无丢失 | 转换器对账：指令数 169,831 条 100% 匹配，未映射资源为 0，SHA256 校验通过 | **必须达成** | 
| **2. 行为一致** | 同初始变量与输入序列下，事件触发顺序、分支、等待、变量变更、RNG 与 Rust Oracle 一致 | Headless VM 对拍：核心路线与随机事件 `trace` 中 `variables_changed` 与 `side_effects` 零偏差 | **必须达成** | 
| **3. 表现一致** | 文本、选项、人名框、9 级图层、图片槽、音频、输入响应（滚轮/右键=ESC/hover）无错乱 | 关键画面人工验收通过，无截断/重叠/闪烁/立绘丢失 | **必须达成** | 
| **4. 像素级一致** | 与原版截图逐像素对比 | Golden screenshot diff 均值误差受控（允许字体抗锯齿微小差异） | 后续优化目标 | 

### 4.2 技术栈决策表（B）

| 评估维度 | 纯 GDScript（**推荐**） | Godot C# | GDScript + C# 混合 | 
| ----- | ----- | ----- | ----- | 
| 开发效率与场景/UI 迭代 | **极高**，与 Godot 节点/信号/自绘无缝配合 | 中等，需编译等待与跨语言绑定 | 中等，需维护两套类型系统与桥接 | 
| 跨平台导出（Win/Linux/macOS/Android） | **最简单**，无 .NET 运行时体积与 AOT 限制 | 需携带 .NET 运行时，部分移动端受限 | 同 C# | 
| 二进制解析与 VM 性能 | 二进制解析已移至 Python 离线转换器；60fps 下 VM 每帧仅执行数百至数千条指令，纯 GDScript 完全胜任 | 数值计算更快，但本项目无高强度物理/3D 瓶颈 | 性能收益不显著却增加复杂度 | 
| **结论** | **采用纯 GDScript（Godot 4.3+ Compatibility 渲染器）** | 不推荐（无需运行时解析 `.wolf`） | 不推荐 | 

> **为什么 Python 离线转换器不违背“运行时全 Godot 原生”？**
>
> Python 脚本仅在开发期/用户本地初始化时运行一次，将 `.wolf/.dat/.mps` 转为 `PNG/OGG/TTF + JSON/预编译指令表`。游戏本体发布包与运行时零 Python、零 Rust、零 GDExtension，100% 纯 GDScript 执行。

## 5. 目标架构、数据流与核心 ADR

### 5.1 总体架构与数据流（C）

```
flowchart LR
    subgraph Offline["离线工具链 (Python, 不进游戏包)"]
        WOLF[".wolf / .dat / .mps / 媒体"] --> CONV["converter/ (解析+校验+预编译)"]
        CONV --> DATA["game_data/ (manifest.json, programs.json, db.json, maps/, assets/)"]
    end

    subgraph Runtime["Godot 4 纯 GDScript 运行时"]
        DATA --> ADB["AssetDB (LRU 懒加载)"]
        DATA --> PDB["ProgramDB (预编译指令+跳转表)"]
        IN["InputRouter (统一边沿/滚轮/Hover)"] --> VM["EventVM (显式状态机)"]
        PDB --> VM
        VM -- "SideEffect 指令" --> GS["GameState (变量/日程/地图/战斗/H/图片槽)"]
        GS --> PRES["Presentation (9级图层/自绘101-102/TileMap)"]
        GS --> AUD["AudioManager (BGM/BGS/SE 循环点)"]
        GS <--> SAVE["SaveService (原子JSON快照 + Migrator)"]
        VM <--> SAVE
    end

```

**推荐目录结构**：

* `autoload/`：`AssetDB.gd`, `ProgramDB.gd`, `EventVM.gd`, `GameState.gd`, `SaveService.gd`, `AudioManager.gd`, `InputRouter.gd`

* `core/`：`event_vm/`（`vm.gd`, `vm_instance.gd`, `opcodes_core.gd`, `opcodes_ui.gd`, `wolf_rng.gd`）、`data/`（`database.gd`, `map_data.gd`）、`save/`（`save_schema.gd`, `save_migrator.gd`）

* `scenes/`：`Boot.tscn`, `Title.tscn`, `Field.tscn`, `Battle.tscn`, `HScene.tscn`

* `ui/`：`picture_stage.gd`, `message_window.gd`, `choice_window.gd`, `nameplate.gd`, `hud.gd`, `save_load_screen.gd`

* `tests/`：`headless/`（VM trace 对拍、存档 round-trip、soak）、`visual/`（截图回归）

* `game_data/`（`.gitignore` 忽略，存放本地转换产物）

> **Headless 可测 vs 场景验证边界**：`EventVM`、`GameState`、`Database`、`WolfRng`、`SaveService`、`TextTokenizer` 全部为纯逻辑类，可在 `godot --headless` 下运行自动化测试；`PictureStage`、`MessageWindow._draw()`、`ChoiceWindow`、`InputRouter` 需在带渲染的测试场景中通过输入注入 + `get_viewport().get_texture().get_image()` 截图回归验证。

### 5.2 架构决策记录（ADR）

| ADR | 背景 | 选项 | 决定 | 后果 | 
| ----- | ----- | ----- | ----- | ----- | 
| **ADR-01** | 需确定最终运行时语言 | 纯 GDScript / Godot C# / 混合 | **纯 GDScript** | 工程极简、导出方便；需注意 32 位整数显式掩码 | 
| **ADR-02** | 原版 `.wolf/.dat/.mps` 解析复杂且含 CP932 编码 | 运行时解析 / 离线 Python 转换 | **离线 Python 转换** | 运行时零二进制解析负担；需维护转换器版本与校验 | 
| **ADR-03** | 107 并行事件与 442 `call_only`（含 `Wait`）调度 | `await` 协程 / 显式状态机 VM | **显式状态机 VM** | 支持确定性帧步进与完整存档序列化；代码略多于 `await` | 
| **ADR-04** | WOLF 101/102 窗口、`\cdb` 控制码、九宫格排版特殊 | `RichTextLabel` / 自定义 `Control._draw()` | **自定义 `Control._draw()`** | 精准控制换行、描边、右对齐 `<R>` 与真字体度量 | 
| **ADR-05** | 存档格式与内容完整性 | 兼容 `.sav` / 新建版本化 JSON | **新建版本化 JSON + 完整状态快照** | 彻底解决漏存 `pc`/RNG/音频问题，可读可迁移；不读旧 `.sav` | 
| **ADR-06** | 历史 UI-01..04 输入/焦点冲突频发 | 各 Control 独立接信号 / 统一 `InputRouter` | **统一 `InputRouter`** | 右键=ESC、滚轮翻页、移动才触发 hover 全局一致 | 
| **ADR-07** | 729 MB 媒体全加载导致内存爆炸 | 全量常驻 / 按需懒加载 + LRU 淘汰 | **按需懒加载 + LRU 缓存** | 控制内存 < 512 MB；首帧切换需预热当前场景资源 | 
| **ADR-08** | 确保 550 事件行为 1:1 不漂移 | 人工肉眼测试 / Rust Oracle Trace 对拍 | **保留 Rust 为开发期 Oracle** | 可自动化发现任何 PC/变量/RNG 漂移 | 

## 6. 核心子系统详细设计

### 6.1 离线转换器（D）

* **输入**：`Data.wolf`、`BasicData/`（`Game.dat`, `CommonEvent.dat`, `*.project`, `*.dat`）、`MapData/*.mps`、媒体文件（`Picture/`, `Fonts/`, `BGM/`, `BGS/`, `SE/`）。

* **输出（`game_data/`）**：

  * `manifest.json`：版本、转换时间戳、源文件哈希、资源映射表（`orig_id`, `new_path`, `type`, `size: [w, h]`, `anchor`, `loop_start`, `loop_end`, `sha256`）及 `alias_fixes`（如记录 `Picture/word_effect* -> hword/word_effect*`）。

  * `programs.json`（调试版）+ `programs.bin`（或 Godot `Resource` 加速版）：预编译后的 550 公共事件与 73 地图事件。每条指令扁平化为 `{ "op": int, "args": [...], "strs": [...], "jump_pc": int, "else_pc": int, "end_pc": int, "src_line": int }`，在离线阶段完成 `branch_depth` 配对与跳转表计算。

  * `database.json`、`maps/*.json`、`texts_raw.json` + `texts_zh.json`（汉化覆盖表）。

* **幂等与对账校验**：转换器每次输出至临时目录，校验 `common_events == 550`、`real_instructions == 169,831`、无未识别 opcode、所有引用资源存在于目录或白名单后，原子替换 `game_data/` 并生成 `conversion_report.json`。**严禁**运行时解析 `.wolf` 或手工将 550 个事件重写为 GDScript/Dialogic。

### 6.2 事件 VM（E）

* **显式状态结构（`VMInstance`）**：`instance_id`, `program_id`, `pc`, `parent_id`, `wait_state`（`NONE / FRAMES(n) / MESSAGE / CHOICE(block) / CHILD(id)`）, `loops`（`[{block_pc, remaining}]`）, `self_vars`（数值+字符串）, `return_to`, `retain_after_finish`。

* **调度顺序**：每帧按固定顺序（1. 递减 `WaitState::Frames` 并唤醒 `Message/Choice`；2. 检查 `auto_events` `run_condition 2/3` 边沿启动；3. 按 `instance_id` 升序步进活跃实例，单帧总指令预算 `200,000`；4. 回收已完成子实例并回写返回值给阻塞的父实例）。

* **关键语义**：

  * `210`/`300` 调用创建子实例并置父实例为 `WaitState::Child(child_id)`，参数写入 `CSelf0..` 与 `CSelf5..8`；

  * `180` 等待置 `WaitState::Frames(n)`；`111`/`112` 条件与 `401`/`420`/`499`/`498` 均通过预编译的 `jump_pc` / `end_pc` O(1) 跳转；`213` 标签跳转使用预编译 `label_map[name] -> pc`；

  * **32 位整数保护**：`func wrap_i32(v: int) -> int: v = v & 0xFFFFFFFF; return v - 0x100000000 if v >= 0x80000000 else v`；

  * **PRNG**：严格实现 Park-Miller MINSTD（`state = (state * 16807) % 0x7FFFFFFF`），种子随存档保存。

* **Trace 对拍格式**：`{"frame": 12, "inst": 3, "ev": 241, "pc": 628, "op": 121, "vars_delta": {"350": 0}, "effects": ["PIC_SET:90"]}`。

* **Opcode 实施优先级**：先实现占 93.9% 指令量的前 14 个高频 opcode（`401, 210, 121, 150, 101, 499, 111, 103, 180, 420, 250, 498, 179, 213`），再按事件覆盖矩阵补齐剩余 36 个。

### 6.3 表现层与 UI（F）

* **分辨率与缩放**：工程设置 `viewport/width=1024`, `viewport/height=576`, `stretch/mode="canvas_items"`, `stretch/aspect="keep"`（支持整数缩放开关）。

* **9 级统一图层带（`z_index`）**：`Map(0) < ScenePicture(10) < SceneText(11) < MessageFrame(20) < MessageText(25) < Nameplate(30) < ChoicePanel(40) < ChoiceText(41) < Cursor(50)`。图片槽使用轻量池化的 `PictureLayer`（`TextureRect`/自绘），按槽位番号排序，**绝不为每个资源建常驻节点**。

* **101/102 自绘与原生 Control 分工**：设置页、存档槽列表外壳可使用 Godot 原生 `Control`；**101 消息框、102 选项框、人名框、文本图片（type 2/4）必须使用自定义 `Control._draw()`**，配合 `TextTokenizer` 解析 `\f[n]`、`\c[n]`、`\E`/`\N`、`\cdb[...]`、`\v[n]`、`<R>`，并用 `Font.get_string_size()` 精确排版。

* **统一 `InputRouter.gd`**：拦截 `_input(event)`，输出标准化 `InputIntent`（`CONFIRM`, `CANCEL`, `UP/DOWN/LEFT/RIGHT`, `WHEEL_DELTA`, `POINTER_MOVED`, `POINTER_CLICK`）。**仅当 `InputEventMouseMotion` 发生时**才触发选项 hover 预选，彻底消除 `RF-01` 鼠标静止覆盖键盘光标的问题；右键统一映射为 `CANCEL`（同时保留 raw bit 给 `engine_state(8)`）。

* **音频**：`AudioManager` 维护 BGM / BGS / SE 三条 `AudioStreamPlayer` 总线，利用 `manifest.json` 导出的 `loop_start`/`loop_end` 实现无缝循环与淡入淡出。

### 6.4 存档系统（G）

**新存档 JSON Schema 示例**：

```
{
  "schema_version": 1,
  "game_version": "0.1.0",
  "saved_at": "2026-11-14T01:10:00+09:00",
  "playtime_frames": 124500,
  "chapter": 2,
  "map": { "id": 3, "x": 12, "y": 8, "dir": 2 },
  "variables": { "normal": {"4": 21, "5": 40}, "sys": {"3": 512, "4": 240}, "self_banks": {} },
  "cdata_overrides": {},
  "event_instances": [
    { "id": 12, "program_id": 216, "pc": 45, "parent_id": null, "wait": "NONE", "loops": [], "self_vars": {} }
  ],
  "auto_events_latched": [216, 217],
  "pictures": [
    { "slot": 90, "kind": "FILE", "asset": "Picture/stand_imouto_paja2.png", "x": 512, "y": 288, "anchor": 1, "scale": [100, 100], "opacity": 255 }
  ],
  "audio": { "bgm": {"asset": "BGM/daily.ogg", "pos_sec": 14.2, "vol": 0.8}, "bgs": null },
  "rng": { "seed": 2675025206, "state": 987654321 },
  "ui_mode": "FIELD"
}

```

* **原子写入**：写入 `user://saves/slot_01.json.tmp` → `flush()` → 备份旧档为 `.bak` → `DirAccess.rename_absolute` 替换正式文件；同时保存 `slot_01.png` 缩略图与轻量 `meta.json`。

* **读档重建顺序**：1. 校验 `schema_version`（必要时经 `SaveMigrator` 升级）；2. 清空当前场景与 `EventVM`；3. 恢复 `GameState`（变量、`CData`、`RNG`、`auto_events_latched`）；4. 加载地图与重建 `pictures` 图层；5. 恢复 `event_instances` 与 `audio`；6. 切入对应 `ui_mode` 场景，**由于 `pc` 与 `latched` 已精确恢复，已消费事件绝不会在读档后重放**。

## 7. 测试、性能、管线与执行路线图

### 7.1 三层测试与 Rust Oracle 对拍（H）

1. **Layer 1（Headless VM Trace 对拍）**：在本地保留 Rust `wolf-cli` 作为开发期 Oracle。针对核心事件/随机事件/战斗/H 阈值，输入相同初始变量与按键序列，导出 Rust trace 与 Godot `--headless` trace 进行逐行 diff。现有 153 个测试块（957 用例）优先迁移**路线/存档/战斗/日程/结局测试**，不盲目逐行翻译底层解析单测。

2. **Layer 2（UI 与 Golden Screenshot）**：针对标题、读档页（滚轮/右键/ESC）、对话框+人名框、选项 hover、19:30 房间、夜晚互动等画面，在 Godot 测试场景中渲染并对比基线截图。

3. **Layer 3（Soak 与内容覆盖矩阵）**：跑通 30/90 天连续日程 soak、8 槽位反复存读、NG+、550 公共事件、111 随机事件、9 个结局与 DLC 链覆盖率统计。CI 使用 `godot --headless --script res://tests/run_all.gd` 自动执行。

### 7.2 性能与资源预算（I）

* **懒加载与缓存**：729 MB 媒体按需从 `game_data/` 读取，解码纹理存入 `AssetDB` LRU 缓存（预算 **384 MB**），切图/切日时释放非活跃场景引用。`programs.bin` 仅约数 MB，可启动时全量加载。

* **性能目标**：启动至标题 < 1.5s，首帧渲染 < 100ms，稳定 60fps（单帧 VM 耗时 < 2ms），常驻内存 < 512 MB。

* **不要过早优化清单**：不要在 profile 前把 GDScript 改写为 C++；不要预生成全部 729 MB 的 `.import` 纹理；不要为极低频 opcode 做复杂 JIT 编译。

### 7.3 内容管线与多语言支持（J）

* 550 事件保持纯数据驱动，禁止在 `.tscn` 节点中硬编码剧情分支。

* **汉化管线**：转换器保留原文 `texts_raw.json` 与 `texts_zh.json` 覆盖表，运行时根据语言设置查表替换，支持热切换与文本 diff 审查。

* **内置调试工具**：开发期按 `F9` 呼出 Debug Overlay（实时查看变量 `v[n]`、活跃协程 `pc`、图片槽与图层），便于内容对齐。

### 7.4 分阶段路线图与工期（K）

| 阶段 | 核心交付物 | 验收标准 | 预计工期（单人 / 2–3人） | 
| ----- | ----- | ----- | ----- | 
| **M0：数据与工具链** | Python 离线转换器、`manifest.json`、预编译 `programs`、Oracle 导出脚本 | 169,831 条指令 100% 转换校验通过 | 3–4 周 / 2–3 周 | 
| **M1：纵向切片** | `EventVM` 核心 + 前 14 opcode、`SaveService` 原子存档、标题→开场→第一天→存读档 | 切片流程跑通，Trace 与 Rust Oracle 一致 | 3–5 周 / 2–3 周 | 
| **M2：主循环与 UI** | 9 级图层、自绘 101/102/名牌/控制码、地图点触、统一 `InputRouter`、三轨音频 | 解决 UI-01..13 所有历史痛点，地图日程可玩 | 4–6 周 / 2.5–3.5 周 | 
| **M3：战斗与 H 系统** | 补齐剩余 36 opcode、战斗公式、`CE159/162` 核查、H 场景与阈值逻辑 | 战斗与 H 场景与原版逻辑/数值一致 | 5–7 周 / 3–4 周 | 
| **M4：全内容对齐** | 550 事件/111 随机事件/9 结局/DLC 覆盖、30/90 天 soak、截图回归 | 覆盖率达标，无阻塞性 P0/P1 缺陷 | 5–7 周 / 2.5–4 周 | 
| **M5：收口与发布** | 性能优化、多语言切换验证、本地转换引导工具、导出打包 | 满足 DoD，60fps 稳定运行 | 2–3 周 / 1–1.5 周 | 
| **合计** | — | — | **22–32 周 / 13–19 周** | 

**第一个 2 周 Sprint 任务清单**：

1. 搭建 Godot 4.3+ 工程骨架与 `autoload/` 目录；

2. 完成 Python 转换器对 `CommonEvent.dat` 与 `Database` 的解析与 JSON 导出（含 `branch_depth` 预编译）；

3. 实现 `EventVM` 状态机骨架、`wrap_i32()`、Park-Miller RNG 与前 6 个高频 opcode（`121, 111, 401, 499, 210, 180`）；

4. 跑通第一条 `godot --headless` 与 Rust Oracle 的 trace 对拍测试。

## 8. 风险登记册（L）与版权边界（M）

### 8.1 风险登记册

| 风险项 | 概率 | 影响 | 触发信号 | 缓解措施 | 应急方案 | 
| ----- | ----- | ----- | ----- | ----- | ----- | 
| **VM 语义漂移** | 高 | 高 | Oracle 对拍变量不一致 | 每实现一批 opcode 立即对拍；强制 `wrap_i32` | 针对差异事件抓取原版/Rust 逐指令 trace 定位 | 
| **文字/窗口布局漂移** | 中 | 中 | 文本换行位置/名牌偏移 | 放弃 `RichTextLabel`，全程用 `_draw()` + 真字体度量 | 针对个别模板微调偏移表 | 
| **存档状态漏存** | 中 | 高 | 读档后剧情重放或立绘消失 | 序列化完整 `VMInstance` + `auto_events_latched` | 读档后对比存档前后的全量状态 dump | 
| **资源 ID/锚点/图层错位** | 中 | 中 | 立绘位置偏移或遮挡 HUD | `manifest.json` 强制校验；统一 9 级 `z_index` | 启用 `F9` 图层调试器定位槽位 | 
| **550 事件/结局覆盖不足** | 高 | 高 | 偏门分支或 DLC 触发崩溃 | 建立事件覆盖率追踪表，自动化 soak 跑遍分支 | 发布前对未覆盖事件做静态可达性审计 | 
| **转换器版本漂移** | 中 | 中 | 更新转换器后旧工程报错 | `manifest.json` 与 `SaveSchema` 均带强版本号 | 保留向后兼容的 `SaveMigrator` | 
| **GDScript 性能/内存峰值** | 低 | 中 | 长对话掉帧或内存超 1GB | LRU 缓存 + 预编译扁平指令数组 | 仅对确认的热点做局部缓存优化 | 
| **版权与素材分发风险** | 高 | 极高 | 仓库或发布包误含原版图音 | `.gitignore` 严格忽略 `game_data/` 与原版资产 | 仅分发纯代码 + 本地转换工具，由用户提供正版游戏目录 | 
| **范围膨胀** | 中 | 中 | 第一版强求重绘 UI 或改写剧情 | 严格锁定“原版内容 1:1，视觉后续优化” | 砍掉非必要重制特性，死守 M1–M4 里程碑 | 

### 8.2 版权与发布边界（M）

* **仓库策略**：`game_data/`、原版 `.wolf`/`.dat`/媒体文件**严禁提交至 Git 仓库**，仅保留转换器代码、Godot 纯脚本工程与极少量占位测试夹具。

* **分发方式**：若公开发布开源项目或可执行程序，**仅分发 Godot 运行时程序与转换器**；玩家需自行拥有原版《妹相随》游戏，首次启动时通过本地转换器指向其正版游戏目录生成 `game_data/` 后游玩，彻底规避版权素材二次分发风险。

## 9. 架构调整摘要（基于已拍板决策）

1. **为什么转为 `Godot 4.7.2 C# 核心 + GDScript 表现层`**：
   - 你既要求**减少 Rust 硬盘占用**、又要求**尽量兼容原版 `.sav`**、且只发布 **Windows x64**——这使得 **C# 核心（VM / 32 位算术 / Park-Miller PRNG / `wolf-save` 原版存档导入）+ GDScript 表现层（场景 / 101-102 自绘 / InputRouter / AudioManager）** 成为最优解。
   - **收益**：相比纯 GDScript，省去了所有 64 位→32 位手动掩码与二进制加解密性能顾虑；相比 Rust，彻底消除 400 MB+ `target/` 编译目录。
2. **原版 `.sav` 兼容策略（双轨方案）**：
   - **读取兼容（必做）**：C# `LegacySaveImporter.cs` 移植 `wolf-save` 的 3 阶段 PRNG 解密与 `Part1..Part7 + VarDB` 解析，可直接载入原版 `SaveData01..08.sav`（通过 `auto_events.prime_latched` 恢复 HUD）。
   - **保存策略（推荐）**：以新版完整 JSON/二进制快照保存（含 `event_instances`、`pc`、`call_stack`、`rng`、`audio`），彻底消除 `RUST-001` 漏存风险；若需回写原版 `.sav`，可在 C# 侧保留模板 `.sav` 壳回写基础变量与图片表作为导出选项。
3. **下一步立即执行（M0 启动）**：
   - **本地实机抓包**：录制原版实机下的 `opcode 123`（掩码 177 与 13/14/20/100/300）与 `CE159/162` 战斗目标选择流程；
   - **搭建 Godot 4.7.2 (.NET) 工程**：建立 `core_cs/`（`EventVM.cs`, `WolfRng.cs`, `LegacySaveImporter.cs`）与 `ui/`（`MessageWindow.gd`, `InputRouter.gd`）；
   - **运行 Python 转换器 M0 原型**：导出官方中文版 `programs.json`、`database.json` 与 `manifest.json`。
