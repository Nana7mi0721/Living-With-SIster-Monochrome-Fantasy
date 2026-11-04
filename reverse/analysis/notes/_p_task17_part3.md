
---

## 7. 判定表

**谁启动**一列的含义：

- **引擎/常时** = 通道 (b) 的 `run=3»，引擎无条件常驻（本文 §2.2）；
- **引擎/自动** = 通道 (b) 的 `run=1»，游戏启动时跑一次；
- **引擎/隐式** = 通道 (a)，yado 记载的功能性调用（本作只有 063）；
- **引擎/条件** = 通道 (b) 的 `run=2»，引擎每帧按条件变量启停；
- **脚本** = 只能被 `210/211/300» 显式调用；
- **无人** = 本作既不满足条件、也无人调用。

| 事件ID | 名称 | 触发条件 | 谁启动 | 证据 | 建议 |
|---|---|---|---|---|---|
| **41** | マウス感知コモン | run=3（条件字节 `v1==0» 不参与） | 引擎/常时 | §5.1；0 条 150，只写自己 self 参数槽 `1600010..13» | **启动**（零可见输出） |
| **48** | X[共]基本システム自動初期化 | run=3 | 引擎/常时 | 已在 `ENGINE_EVENTS»；6 帧 115 指令 12 次 DB | **保持**（现状） |
| **63** | X[共]メッセージウィンドウ | run=0 | 引擎/隐式 | yado:61-63、:68-70；全库 0 调用点 | **保持**（现状） |
| **213** | クリックエフェクト | run=3 | 引擎/常时 | §5.2；读 Sys71/72（唯一写者是 225） | **与 225 同时启动** |
| **214** | 健康監視 | run=3 | 引擎/常时 | §5.3；写 v20/v41/v43/v45，26 处外部读者 | **启动** |
| **224** | ウィンドウ処理 | run=3 | 引擎/常时 | §5.4；0 条 150，依赖 225 与其它事件的图片号 | **启动**（当前基本空转） |
| **225** | フィールド上処理 | run=3 | 引擎/常时 | §5.5；15 条 150，自算光标图片号 | **启动**，但需 §8.4 人工裁决 |
| **261** | ×◆座標変数初期化 | run=3 | 引擎/常时 | §5.6；作者注释「自動で処理される」；唯一读者 270 | **启动** |
| **269** | ×┣[メッセ—ジ]初期化·設定 | run=3 | 引擎/常时 | §5.7；置 self `1600099» | **启动**（270 的前置） |
| **270** | ×┣[メッセ—ジ]表示処理 | run=3 | 引擎/常时 | §5.7；condvar `2700020» 全库 0 次出现；98 条 150 全在 self 图号 | **启动** |
| **271** | ×┗[メッセ—ジ]オートスキップ | run=3 | 引擎/常时 | §5.7；写 sysstr 26/27；0 条 150 | **启动**（当前像素不可见） |
| **241** | ┣ターン開始処理 | run=1 | 引擎/自动 | §5.9；尾槽 628 自清 `v350»，跑一次即止 | **启动** |
| **216** | ステータスバー表示 | run=2 `v3 == 1» | 引擎/条件 + 脚本 ×1 | §5.10；`v3» 唯一写者 `SampleMapA#ev0p0-215»；0 条 Wait ⇒ 按 id 调用不阻塞 | 不单独启动，**交给 run=2 驱动** |
| **217** | 時刻表示 | run=2 `v3 == 1» | 引擎/条件 + 脚本 ×625 | §5.10；0 条 Wait | 同上 |
| **38** | --------------------------------- | run=2 `v160 <= 2» | 引擎/条件（开局为真） | §5.10；自我关闭的状态机触发器 | 交给 run=2 驱动 |
| **43** | ┗オリジナルタイトル画面Ｂ | run=2 `v0 == 0» | **无人** | §1 结论 5：0 调用点；图片号 3/4/5 被 `pic 12000» 完全遮挡；6 张 menu 图全库只被它自己引用 | **不启动** |
| **343** | ■特殊カットイン | run=2 `v158 >= 1» | **脚本 ×44** | 条件变量 `2000158» 全库 0 写者 ⇒ 并行实例永不启动；但被 44 处按 id 调用 | **不进并行表**（按 id 调用照旧） |
| **236 / 237** | 立ち絵瞬き1 / 2 | run=2 `v2 == 1 / 2» | 引擎/条件 | 各 347 条；条件变量 103 个写者 | 交给 run=2 驱动 |
| **240** | ┣戦力処理 | run=2 `v340 >= 1» | 引擎/条件 + 脚本 ×1 | 写 `v192/v193»（读者 ev244）；0 条 Wait | 交给 run=2 驱动 |
| **335** | 深夜差分描画 | run=2 `v62 >= 1» | 引擎/条件 | 2858 槽，全表最重（53 帧 / 1321 指令 / 90 次 DB，§6） | 交给 run=2 驱动 |
| **514–521** | サムネ処理1..7 + アルバム個別UI処理 | run=2 `v616 == 1..7 / >= 1» | 引擎/条件 | 条件变量 11 个写者（ce247/511/512） | 交给 run=2 驱动 |
| 其余 **85** 个 run=2 | （见 §3 表 B/C 类） | 各自条件变量 | 引擎/条件 | **90/97 个 0 调用点**，纯靠变量启停 | 交给 run=2 驱动 |

> 一句话：**「引擎自己启动」的集合 = 11 个（10 个 run=3 + 1 个 run=1）**，加上已在名单里的 63（run=0，通道 (a)），
> 共 **12 个**；另有 97 个 run=2 由条件驱动、442 个 run=0 由脚本驱动。

---

## 8. 给 `Game::start_engine_events» 的推荐

### 8.1 分三期，不要一次全上

| 期 | 内容 | 风险 | 验收 |
|---|---|---|---|
| **P1（本文）** | 把 `ENGINE_EVENTS» 扩成三组常量，按依赖顺序启动 11 个引擎常驻 + 保留 63 | 只有一个事件（225）可能改像素 | §8.3 的 A/B 协议 |
| **P2（下一次）** | 实现 `run=2» 的 run_condition 驱动器 | 会**一次性**唤醒最多 97 个协程，必须限流与观察 | 变量快照 + 像素 + `StepReport» 统计 |
| **P3（后续）** | 在消息渲染里消费 sysstr 26/27（`文章表示追加 先頭/最後尾»） | 会改变所有消息的排版 | `docs/TEXT_SPEC.md» 的排版用例全跑 |

**P1 的收益**：让「引擎常驻服务」这一层与原版对齐；代价是每帧约 +580 条指令（§6），可忽略。
**P1 的风险**：225 会把鼠标光标画成图片（§5.5）。这是唯一可能立刻改变屏幕像素的事件。

### 8.2 代码骨架（可直接落地；文件属于 Lead 的写权限）

```rust
// crates/wolf-game/src/game.rs

/// Events the engine starts **unconditionally** because their header says
/// `parallel_process_always(3)». The condition byte they also carry is an editor
/// artefact: e.g. `270» names variable `2700020», which never appears anywhere in
/// the 550 common events or the 7 map resources (docs/ENGINE_EVENTS.md §2.2).
/// Order matters: 261 must precede 270 (270 reads `2700000..2700005»), and 269
/// must precede 270 (270 tests the flag `1600099» that 269 sets).
pub const ENGINE_ALWAYS: &[u32] = &[
    48,  // X[共]基本システム自動初期化 — system variables first
    261, // ×◆座標変数初期化      — writes 2700000..2700005 for 270
    269, // ×┣[メッセ—ジ]初期化·設定 — sets self 1600099 for 270
    270, // ×┣[メッセ—ジ]表示処理
    271, // ×┗[メッセ—ジ]オートスキップ — writes sysstr 26/27
    224, // ウィンドウ処理         — needs 225's Sys71/72 + other events' picture numbers
    225, // フィールド上処理         — publishes Sys71/72, draws the picture cursor
    213, // クリックエフェクト        — needs 225's Sys71/72
    214, // 健康監視
    41,  // マウス感知コモン
];

/// `auto_start(1)»: run once at game start. It clears its own condition variable
/// (slot 628 writes `v350 = 0») and ends, so it never becomes a resident coroutine.
pub const ENGINE_AUTO_START: &[u32] = &[241];

/// Started by an engine *feature* rather than by `run_condition»: YADOT-WOLF
/// documents that every 【文章の表示】 executes 063 (work/yado.txt:61-63 / :68-70),
/// and no script anywhere targets 500063.
pub const ENGINE_IMPLICIT: &[u32] = &[63];

impl Game {
    /// Start the events the engine owns. Idempotent.
    pub fn start_engine_events(&mut self) -> usize {
        if self.engine_events_started {
            return 0;
        }
        self.engine_events_started = true;
        ENGINE_ALWAYS
            .iter()
            .chain(ENGINE_AUTO_START)
            .chain(ENGINE_IMPLICIT)
            .filter(|id| self.start_event(**id))
            .count()
    }
}
```

**为什么保留 `engine_events_started» 幂等标志**：`start_engine_events» 同时被 `boot()»（`game.rs:273»）与
`start_map_event_by_name»（`game.rs:315»）调用，后者在切图时也会走 ⇒ 没有标志会重复启动。

**为什么分成三个常量而不是一个大数组**：三组的**生命周期语义不同**（常驻 / 一次性 / 功能性），
将来实现 `EndGame» 或读档重开时，只有 `ENGINE_ALWAYS» 需要重建，另两组不需要。这也让文档与代码一一对应。

### 8.3 验收协议（每加一个事件都要跑）

我们现在**没有**「额外启动某公共事件」的 CLI flag（`wolf-cli capture --help» 只有
`--frames / --out / --answer / --filter / --until / --menu-only / --hold-message / -m / --boot / --root»），
所以 A/B 有两种做法：

1. **推荐**：给 `wolf-cli» 加一个 `--engine-events <id,id,…>»（对 `capture» 与 `run» 生效），
   覆盖 `ENGINE_ALWAYS»。这样不必反复改 `game.rs» 就能做逐事件 A/B；
2. **退路**：改 `ENGINE_ALWAYS» 后重编译，逐次对比。

每次对比的命令：

```bash
./target/debug/wolf-cli.exe capture --frames 716 --out work/out/ab_a.png
# 改启动集
./target/debug/wolf-cli.exe capture --frames 716 --out work/out/ab_b.png
cargo run -q -p wolf-render --example imgdiff -- work/out/ab_a.png work/out/ab_b.png
```

判定：

- **`identical: true»** ⇒ 该事件在当前实现下零可见影响，可以放心留用；
- **有像素变化** ⇒ 记下 `mean abs / max / psnr / 变化像素数»，并回答：这些像素是「零售也有的」还是「我们多画的」？
  前者说明我们之前漏画了（要补），后者必须回退。

基准数值（C2，当前状态，未启动那 10 个）：`14319 instruction(s) / 10 picture op(s) / 36 db access(es) / 0 warning(s)»，
`rendered 1024x576, 538282/589824 non-black»，与零售截图 mean abs 0.012 / max 1 / psnr 67.20 dB。

### 8.4 两处必须人工裁决的地方

1. **225 的图片化鼠标光标**：实机把鼠标移到标题画面中央停住，截一张带光标的图，
   看屏幕上是否出现**图片风格**（而不是系统箭头）的光标。
   - 有 ⇒ 零售确实启动 225，我们也要启动（并且 213 的坐标通道才有意义）；
   - 没有 ⇒ 说明原版在**标题地图**上下文里没启动 225（或它把光标图片设成了不可见），
     我们要加一层「只在字段地图上启用」的抑制，而不是无条件常驻。
2. **216 / 217 的「并行实例 + 按 id 调用」双跑**：两者都实测 **0 条 `180»/`101»**
   （§5.10），所以按 id 调用**不会阻塞调用者**，双跑在原版是自洽设计。
   但我们的 `210» 语义是「spawn 子协程并阻塞到它结束」（`crates/wolf-vm/src/exec.rs:85/304/880»），
   一旦将来给 216/217 加上 Wait，625 处按 id 调用就会把整条调用链卡住 ⇒ **P2 之前必须复查这一点**。

### 8.5 P2：`run=2» 驱动器（真正的缺口）

现在 `Program» 里没有 run_condition（§2.5），所以要分两处改：

```rust
// crates/wolf-vm/src/program.rs —— 让程序自带自己的启动条件
pub struct Program {
    // …既有字段…
    /// `run_condition» byte (low nibble) from the event header.
    pub run_condition: u8,
    /// `condition_operator» (high nibble): 0 > / 1 >= / 2 == / 3 <= / 4 < / 5 != / 6 bitand.
    pub condition_operator: u8,
    /// `2000000 + N» magic for the variable slot.
    pub condition_variable: u32,
    pub condition_value: u32,
}

impl Program {
    /// Only `parallel_process(2)» is gated; `parallel_process_always(3)» and
    /// `auto_start(1)» are started by the engine regardless of this byte.
    pub fn is_gated(&self) -> bool {
        self.run_condition == 2
    }

    pub fn condition_holds(&self, vars: &Variables) -> bool {
        let index = (self.condition_variable - 2_000_000) as usize;
        let lhs = vars.get(index).max(0) as u32;
        let rhs = self.condition_value;
        match self.condition_operator {
            0 => lhs > rhs,
            1 => lhs >= rhs,
            2 => lhs == rhs,
            3 => lhs <= rhs,
            4 => lhs < rhs,
            5 => lhs != rhs,
            6 => lhs & rhs != 0,
            _ => false,
        }
    }
}
```

```rust
// crates/wolf-vm/src/exec.rs —— 每帧同步一次「该跑的并行事件」
impl Vm {
    /// Create gated background events whose condition just became true, and stop
    /// those whose condition went false. Returns (started, stopped).
    ///
    /// Runs before `run_instances» so a freshly satisfied gate gets a slice this
    /// same frame (the original engine behaves this way for the opening scene,
    /// where 「SampleMapA」 turns on `v3» and `216/217» start refreshing immediately).
    pub fn sync_gated_events(&mut self) -> (usize, usize) {
        let mut start = Vec::new();
        let mut stop = Vec::new();
        for program in self.programs.iter_gated() {
            let want = program.condition_holds(&self.vars);
            let live = self.instances().iter().any(|i| i.event == program.id && i.parent.is_none());
            match (want, live) {
                (true, false) => start.push(program.id),
                (false, true) => stop.push(program.id),
                _ => {}
            }
        }
        let started = start
            .into_iter()
            .filter(|id| self.start_event(*id).is_some())
            .count();
        let stopped = stop
            .into_iter()
            .filter(|id| self.stop_event_instances(*id) > 0)
            .count();
        (started, stopped)
    }
}
```

需要同步补的三处：

1. `ProgramSet» 要暴露一个**迭代器**（现在只有 `program.rs:442» 的按 id 取值 `event(id)»）；
2. `Program::compile»（`program.rs:240»）要接收 `&CommonEvent» 的 run_condition 四元组；
3. `Vm::stop_event_instances(event)» —— 现有 `stop_instance(InstanceId)»（`exec.rs:680»）是按实例停，
   按事件停要自己扫；注意它得连带唤醒被 `210» 阻塞的父协程（现有实现已经处理了这一点，见其注释）。

**限流建议**：P2 第一次上线时把每帧「新启动的 gated 事件数」限制在个位数，并在 `StepReport» 里加
`gated_started / gated_stopped» 两个计数，避免开局一瞬间 97 个协程同时抢预算。

---

## 9. 未验证项与后续

| # | 未验证的点 | 怎么验证 |
|---|---|---|
| 1 | 零售是否真的把 **225** 的图片化光标画出来 | 实机截图（§8.4-1） |
| 2 | 零售在标题画面是否运行 **213 / 214 / 224 / 261 / 269 / 270 / 271** | §8.3 的 A/B（我们这边像素应无变化；若零售有可见差异则说明我们漏画） |
| 3 | `063» 的真实时机是「每次【文章の表示】」还是「启动一次」 | yado 明写「每次」（§4.1），但我们的实现里只在 `boot()» 启动一次；等 `Message» 接入窗口位置系统变量（sysvar 1/2）时逐条比对 |
| 4 | `224» 在「它要检测的图片号都不存在」时的行为 | 启动后看 `diagnostics» 是否新增 `124» 相关条目（我们 `engine_state» 对 field 11/12/18 目前返回 `None»，见 `game.rs:827»） |
| 5 | `run=2» 的 97 个事件里哪些在**真实存档**下会被唤醒 | 用 `WolfSave» 的 JSON dumper 读一个真实存档，把变量表喂给 `Program::condition_holds» 离线求值一次，得到「读档后立刻该启动的事件集」 |
| 6 | `343» 那 44 处按 id 调用是否说明作者**本意**是 run=0 | 它 run=2 但条件变量无人写，属作者改过启动条件后忘记清残留；按 id 调用照旧工作，无需处理 |

---

## 附录 A：本文与其它文档的关系

| 文档 | 关系 |
|---|---|
| `docs/ARCHITECTURE.md» §2.4 | `run_condition» 的调度语义表（本文 §2.2 引用） |
| `docs/FORMAT_SPEC.md» §3.2.1 | 条件字节的位序实测纠正（本文 §2.1 引用） |
| `docs/M1_CLOSURE.md» | 标题画面基线数字（本文 C2 引用），格式与标记规范来源 |
| `docs/OPCODE_124.md» | `124» 的 field 编号（本文 §5.4/§5.5 引用） |
| `docs/OPCODE_101.md» | `101 文章の表示» 与 063 的配合 |
| `work/yado.txt» | 官方帮助文档，本文 §4 原文取证的唯一来源 |
