# 遗留 Rust 缺陷与 Godot 迁移护栏

> 状态：**现行风险登记册**
>
> 来源：Gemini 对 Rust workspace 的静态审计（2026-11-14）；原始证据和 22 项完整表在
> [`audit/GEMINI_AUDIT_AND_PLAN_2026-11-14.md`](audit/GEMINI_AUDIT_AND_PLAN_2026-11-14.md)。
>
> 注意：原始审计的“纯 GDScript”技术栈段落已过时；本文件按
> [`DECISIONS.md`](DECISIONS.md) 的 C# 核心 + GDScript 表现层重述迁移动作。

## 1. P0/P1 阻塞项

| ID | 风险 | Rust 证据 | Godot 必须做什么 |
| --- | --- | --- | --- |
| `RUST-001` | 旧存档只存变量/图片，漏 VM 实例、RNG、音频 | `game.rs`、`savegame.rs`；读档靠 `prime_latched` 强拉 HUD | 新 `SaveSchema` 保存完整 `event_instances/pc/call_stack/wait/rng/audio/ui_mode`；读档不得重放已消费事件 |
| `RUST-002` | 非原子写档，崩溃/断电会毁档 | `std::fs::write` 直接覆盖 `.sav` | 写 `.tmp` → flush → 备份 `.bak` → 原子 rename；失败恢复必须测试 |
| `RUST-004` | 超过 `MAX_INSTANCES` 时子调用被静默丢弃，父事件不阻塞 | `exec.rs` 直接 `return` | 明确上限 + 熔断 + 可诊断错误；绝不静默继续父事件 |
| `RUST-005` | 执行错误时 `pc += 1` 静默跳过指令 | `exec.rs` `Signal::Error` 分支 | 转换期拒绝未知 opcode；运行时 fail-fast + trace，不跳过 |
| `RUST-008` | 多处 `from_utf8_lossy` 静默污染文本 | `wolf-format`/`wolf-save`/`game` | 转换器按 CP932/GBK/UTF-8 严格解码；失败即终止并报告位置 |
| `RUST-009` | 负层延迟 4 帧、`room_base_pictures` 快照、parked choice 等启发式补丁 | `game.rs` | 用显式场景/图层生命周期、`PictureStage` 和 `InputRouter` 重建，不照搬补丁 |
| `RUST-011` | 文本图片宽度按半角/全角估算，部分 token 被丢 | `textpic.rs` | GDScript 自绘用 `Font.get_string_size()` 真实度量，补齐 token 渲染 |
| `RUST-015` | `123` 按键掩码/特殊键码未完全逆向 | `exec.rs` `key_code(mask)` | M0 实机抓包；`InputRouter` 集中映射并记录未知查询 |
| `RUST-016` | `CE159/162` 战斗目标调度未闭环 | `battle_wiring.rs` | M3 用零售存档/实机录屏补 Oracle trace，不凭推断实现 |
| `RUST-022` | 测试偏结构/单帧，用户实机仍见 UI 异常 | 旧测试与 UI bug 记录 | Godot 必须有多帧输入、场景切换、截图 diff 和全流程 soak |

## 2. 完整风险索引

| ID | 严重度 | 状态 | 迁移处置 | 摘要 |
| --- | --- | --- | --- | --- |
| `RUST-001` | P0 | confirmed | `fix-before-port` | 存档缺 VM/RNG/音频完整状态 |
| `RUST-002` | P0 | confirmed | `fix-before-port` | 非原子写档会毁档 |
| `RUST-003` | P1 | confirmed | `encode-invariant` | WOLF 32 位补码/移位语义；C# 用 `int` 且显式测 unchecked 行为，不靠默认 |
| `RUST-004` | P1 | confirmed | `encode-invariant` | 实例上限超限时静默丢弃子调用 |
| `RUST-005` | P1 | confirmed | `do-not-carry-over` | opcode 错误被静默跳过，PC 漂移 |
| `RUST-006` | P2 | confirmed | `guard-only` | `SaveTarget::Path` 可路径穿越/任意覆盖 |
| `RUST-007` | P2 | confirmed | `guard-only` | 解析长度未校验，可能 OOM |
| `RUST-008` | P1 | confirmed | `fix-before-port` | 有损 UTF-8 解码污染文本 |
| `RUST-009` | P1 | confirmed | `do-not-carry-over` | UI/立绘靠延迟清理和快照补丁 |
| `RUST-010` | P3 | confirmed | `fix-before-port` | 图片解码缓存无界，内存持续增长 |
| `RUST-011` | P1 | confirmed | `fix-before-port` | 文本图片近似度量和 token 缺失 |
| `RUST-012` | P2 | confirmed | `guard-only` | 独立 Y 缩放未建模；`PictureLayer` 预留 `Vector2`，实机确认 |
| `RUST-013` | P2 | confirmed | `fix-before-port` | 文本图片锚点 5/6 退化到左上 |
| `RUST-014` | P2 | confirmed | `guard-only` | 事件 354 资源名笔误，旧实现靠 fallback |
| `RUST-015` | P1 | needs-runtime | `guard-only` | `123` 键码/掩码未完全反向 |
| `RUST-016` | P1 | needs-runtime | `unknown` | 战斗目标 `CE159/162` 未闭环 |
| `RUST-017` | P1 | confirmed | `preserve-or-decide` | `CE333` 坏结局分支在原版数据中不可达；不得擅自改剧情 |
| `RUST-018` | P3 | confirmed | `enhance-in-godot` | 19:30 方针面板拥挤；按 `D-10` 允许重排，记录差异 |
| `RUST-019` | P2 | confirmed | `guard-only` | 音频循环点/损坏文件缺自动化回归 |
| `RUST-020` | P3 | confirmed | `do-not-carry-over` | hover 与键盘导航曾互相覆盖 |
| `RUST-021` | P2 | confirmed | `guard-only` | 打包/转换路径需统一 POSIX 规范化与重名检查 |
| `RUST-022` | P2 | probable | `guard-only` | 测试假阳性/单帧覆盖不足 |

## 3. 行为兼容矩阵

| 分类 | 内容 | 处理 |
| --- | --- | --- |
| **Preserve** | 50 种 opcode、32 位算术、Park-Miller MINSTD、107 个并行事件顺序、9 级图层、1024×576、101/102 文本控制码 | Godot 必须对齐；用 Oracle trace 和截图证明 |
| **Fix-in-Godot** | 完整存档、原子写档、严格编码、LRU 资源缓存、真实字体度量、显式 UI 生命周期、统一 InputRouter | 不复制旧补丁；用测试证明 |
| **Guard-only** | 解析长度/容量上限、路径白名单、未知 opcode 拒绝、调用栈深度熔断、音频循环点钳制 | 不改变玩法，只防崩溃/坏档/越界 |
| **Enhance-in-Godot** | `UI-12` 方针面板排版、可安全改善的字体/窗口怪癖 | 记录前后截图，不改变信息层级 |
| **Preserve-or-decide** | `CE333` 原版死分支 | 先保留/记录；补全必须单独拍板 |
| **Unknown** | `opcode 123` 键码、`CE159/162`、`H-PIC-1` 非 1024×576 负图缩放 | 实机/零售数据验证前不要猜 |

## 4. 绝对不要带进 Godot 的实现习惯

1. 未校验的长度直接分配数组。
2. 未知 opcode 或非法参数静默跳过。
3. 把节点树、`Node`、`Control`、`Sprite2D` 直接序列化进存档。
4. 用 `await` 协程充当事件 VM 调度器。
5. 只存变量/图片，不存事件实例 `pc`、调用栈和 `Wait`。
6. 非原子覆盖写档。
7. 用“多等几帧”“再打一个状态快照”掩盖图层生命周期错误。
8. 用 C# 和 GDScript 各实现一套确定性逻辑。

## 5. 迁移解释

- `RUST-003` 在 C# 中不是“64 位 vs 32 位”问题，因为 C# `int` 本身就是 32 位；但仍需固定 unchecked/wrapping 语义、移位掩码和边界测试，不能依赖编译器/运行时默认行为。
- 原版 `.sav` 缺少 VM 协程状态，是格式事实上限；导入器不得伪造精确恢复。见 [`DECISIONS.md`](DECISIONS.md) §3。
- Rust 版本继续作为 Oracle，但审计里标为 `needs-runtime` 的项必须本地取证，不能把旧实现的猜测写成规格。
