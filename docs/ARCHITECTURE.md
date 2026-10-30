# Living With Sister Monochrome Fantasy —— Rust 重写架构设计

> 目标：用 Rust 从零实现一个能直接读取本作 `Data/` 目录（WOLF RPG Editor v3 / ウディタ 归档）并**逐帧等价**复现游戏的运行时。
> 本文档是开工依据（可执行设计），不是概览。每个关键决策都写明「为什么选 A 不选 B」。
> 文中格式结论来自对真实游戏文件的实测 + 已 vendor 的参考实现；未能确证的项一律标注 **TODO(verify)**。

---

## 0. 事实基线与不变量

### 0.1 引擎与数据（已实测）

| 项 | 实测值 | 来源 |
|---|---|---|
| 引擎 | WOLF RPG Editor v3.x（DxLib + DirectX，`Game.exe` / `GamePro.exe` 双 exe） | 目录侦察 |
| 归档 | `Data/` 下 **26 个 `.wolf`**（DXArchive **版本 8**：`Head='DX'=0x5844`, `Version=0x0008`） | 实测 5 个 .wolf 头全部是明文 `44 58 08 00` |
| 数据文件 | `BasicData/` 下 9 个 `.dat`（明文 v3 结构，无 Pro 加密）+ 4 个 `.project` | 已解包 |
| 地图 | `MapData/` 9 个 `.mps`，游戏实际注册 6 张；Map003 最大 70×170；内容枢纽 SampleMapA 64×36 | Lead 实测 |
| 屏幕 | 逻辑分辨率 **1024×576**，tile 16px，60fps | Game.dat |
| 主字体 | `Lolita.ttf`（游戏根目录），备选 `07yasashi.ttf`（Data/，2.2MB） | 目录侦察 |
| 公共事件 | **550 个**，163,828 个 command slot，其中 **139,273 条真实指令**，**47 种非空 opcode** | `work/parse_ce.py` |
| opcode 并集 | **50 种 = 公共事件 47（非空）+ 地图独有 3（`151` / `161` / `162`）**；交集 **37**、地图侧共 **40** ⇒ **并集即全集** | Lead `work/probe97.py` 差集实测 |
| run_condition | 442 `call_only` / 97 `parallel_process(2)` / 10 `parallel_process_always(3)` / 1 `auto_start(1)` | 同上 |
| 地图事件 | 9 张图共 **73 事件 / 134 页 / 37,610 指令槽（7,052 空 ⇒ 30,558 真实指令）** | Lead 实测 |
| 媒体总量 | png 2777 / jpg 903 / ttf 4；音频只有 **mp3 / ogg / wav 三种扩展名，0 个 MIDI**（条数两次统计不一致：全量扫描得 mp3 172 / ogg 163 / wav 23，早期统计得 mp3 902 / ogg 285 / wav 19 ⇒ **TODO(verify) 复核条数**，不影响任何设计决策）；媒体 ≈ 729 MB（不含 .wolf 归档本身 722MB） | 全量扫描 + 早期统计 |
| 存档 | `Save/SaveData01..08.sav` + `System.sav`（各 ~360-386KB）+ `save1..8.jpg` 缩略图（独立文件，不是存档格式的一部分） | 目录侦察 |

**关键结论 1（解释器必须是多协程）**：`parallel_process(2)` + `parallel_process_always(3)` 共 **107 个事件是每帧并行跑的后台协程**，再加 1 个 `auto_start`。它们必须与主流程同时推进，单调用栈解释器无法表达。

**关键结论 2（`call_only` 事件会阻塞）**：442 个 `call_only` 事件里约 39% 内部含 `Wait(180)`（0 个 wait 的 268 个、1 个 55 个、2 个 15 个、3 个 14 个、4 个 7 个、5 个 7 个、16 个 5 个、21 个 5 个）。⇒ 被 `CommonEvent(210)` 调用时**可能阻塞**，调用者必须挂起等待被调者结束。

**关键结论 3（分支块可静态配对）**：指令记录里**存有 1 字节 `branch_depth(bd)`**（读取顺序：`pc=u1` → `cid=u4 + (pc-1) 个 u4 参数` → `bd=u1` → `str_count=u1` → 字符串 → 尾字节）。用栈按「栈顶 bd == 闭合指令 bd」配对，opener = {102 Choices, 111 VariableCondition, 112 StringCondition, 170 StartLoop, 179 LoopTimes}，closer = {499 BranchEnd, 498 LoopEnd}，对全部 550 事件 163,828 slot 的结果是 **13379 对配对 / 0 个 stray closer / 0 个 unmatched opener**。⇒ 加载期一遍扫描即可得到无歧义跳转表，**运行时不做线性搜索**。

配对种类分布：(111,499)=10643、(179,498)=1810、(102,499)=495、(170,498)=173、(112,499)=118。
全局 bd 分布 {0:17225, 1:39687, 2:33567, 3:25600, 4:17842, 5:10814, 6:6369, 7:4139, 8:2194, 9:1466, 10:1093, 11:685, …, 91:4} ⇒ 最深 91 层，**跳转表用扁平 `Vec<u32>` 即可**，但必须设 128 层上限防恶意数据。

### 0.2 范围

- **做**：运行时从用户自己的 `Data/` 读全部资产（`.wolf` / `.dat` / `.mps` / 字体 / 音视频），不落盘、不重打包。
- **不做**：不内置任何游戏素材；仓库里**不允许存在需要素材才能编译的代码**（所有资产路径均为运行时输入，缺失时优雅报错并提示 `--game-dir`）。
- **不做**：不实现 WOLF 编辑器（`.project` 只在读变量名映射时用到）。
- **交付形态**：`imoto-life.exe`（游戏本体）+ `wolf-cli.exe`（离线工具，差分测试用）。

---

## 1. Cargo workspace 布局

### 1.1 目录树

```
imoto-life/
├─ Cargo.toml                     # [workspace] —— 只放 workspace 级配置
├─ rust-toolchain.toml            # 固定 stable + x86_64-pc-windows-msvc
├─ .cargo/config.toml             # CN 镜像源（见 §7）
├─ docs/
│  ├─ ARCHITECTURE.md             # ← 本文
│  ├─ FORMAT.md                   # 二进制格式规范（format-spec 产出物）
│  └─ vendor/                     # WolfTL / WolfDec / WolfSave / *.ksy（只读参考，不参与编译）
├─ crates/
│  ├─ wolf-format/                # 数据层
│  ├─ wolf-save/                  # 存档层
│  ├─ wolf-vm/                    # 解释器
│  ├─ wolf-render/                # 渲染
│  ├─ wolf-audio/                 # 音频
│  ├─ wolf-game/                  # 组装（应用状态机）
│  └─ wolf-cli/                   # 命令行工具
└─ src/main.rs                    # 主程序（薄壳：参数解析 + 起 wolf-game）
```

### 1.2 各 crate 职责边界

| crate | 负责 | 明确不负责 |
|---|---|---|
| `wolf-format` | DXArchive v8 (`.wolf`) 读取/解压；`.dat` 反序列化（Game / DataBase / CDataBase / SysDatabase / SysDataBaseBasic / CommonEvent / MapTree / TileSetData）；`.mps` 反序列化；`.project`（变量名映射）；PNG/JPEG 解码；虚拟资源索引 | 不知道「怎么画」「怎么放音」；依赖树里**没有** wgpu/kira |
| `wolf-vm` | 指令解码 + 加载期预编译；事件实例 / 调用帧 / 等待状态；变量存储；调度器；把 opcode 语义翻译成 `SideEffect`（消息、图片、音频、地图、数据库…） | 不持任何句柄（无窗口/无 GPU）；不打印到 stdout（走 tracing 事件） |
| `wolf-save` | `.sav` 解密/加密；结构解析与序列化；与官方存档双向兼容；JSON dump（schema 对齐 WolfSave 的 JsonDumper） | 不负责存档 UI；不猜变量语义 |
| `wolf-render` | wgpu 设备/管线/纹理/图集；离屏 1024×576 + 整数倍最近邻 blit；图层栈与批次；glyphon 文本；`\E` 描边 | 不认识 `Picture(150)`，只接受 `DrawCmd` |
| `wolf-audio` | kira 封装；BGM/BGS/SE 三条子轨道；循环点；从归档流式供给音频 | 不认识 `Sound(140)`，只接受 `AudioCmd` |
| `wolf-game` | winit 事件循环、输入映射、场景栈、把 `SideEffect` 翻译成 `DrawCmd`/`AudioCmd`、存档读写、帧计时 | 不含二进制解析细节 |
| `wolf-cli` | `dump-dat` / `dump-mps` / `dump-ce` / `verify-blocks` / `index` / `save2json` / `json2save` / `sim`（headless 推进 N 帧） | 不启动窗口 |

### 1.3 依赖方向（严格单向 DAG）

```
wolf-cli ──► wolf-format          (leaf, 无内部依赖)
   │            ▲
   ├──► wolf-vm ┘（只用编译后的指令 + 变量命名空间）
   └──► wolf-save ──► wolf-format（读 CDataBase.project 做变量名映射）

wolf-game ──► wolf-vm ──► wolf-format
    ├──► wolf-render  (leaf)
    ├──► wolf-audio   (leaf)
    └──► wolf-save

src/main.rs ──► wolf-game
```

### 1.4 为什么这样切（A 不选 B）

- **不选「单 crate + 多 module」**：本工程横跨 4 条完全不同的技术栈（二进制解析 / 解释器 / GPU / 音频）。单 crate 下改一行 UI 颜色要重编全部格式解析代码；拆 crate 后 `wolf-format` 稳定、`wolf-render` 高频改动，增量编译收益显著。
- **不选「把 render/audio 合进 game」**：`wolf-vm` 必须能在**无 GPU 的 CI 上跑**（§8 的差分测试要跑上万次 tick）。因此 render/audio 必须是 `wolf-vm` 的**兄弟**而不是它的依赖者。CI 里用 `cargo tree -p wolf-vm` 断言依赖树中不含 `wgpu`/`kira`/`winit`。
- **`wolf-save` 独立成 crate 而非并入 `wolf-format`**：二者只有「读 `CDataBase.project` 拿变量名」这一点交集；而存档格式的复杂度（WolfSave 全套 5,895 行 C++）与生命周期（首个里程碑就要双向兼容）足以独立。独立后 §8 的 Save-Diff Oracle 可单独 `cargo test -p wolf-save` 且不拉 GPU。
- **`wolf-cli` 与主程序分离**：`wolf-game` 编译成 lib，主程序只是薄壳。这样 `wolf-cli sim --frames 600 --dump-vars out.json` 能在无窗口环境推进游戏逻辑——这是 §8 测试策略的支柱。
- **`wolf-format` 返回懒句柄而非解码后的像素**（`ImageSource` 枚举：`RawRgba` / `Lazy{entry}`）：因为媒体总量 729MB 不可能常驻（§4 内存预算）。

### 1.5 对外 API 轮廓

```rust
// wolf-format
pub struct AssetIndex { /* 虚拟路径 -> AssetEntry{archive, offset, sizes, flags} */ }
impl AssetIndex {
    pub fn scan(game_dir: &Path) -> Result<Self, FormatError>;
    pub fn open(&self, virtual_path: &str) -> Result<AssetReader<'_>, FormatError>;
}

pub struct GameDat { pub tile_size: u32, pub fps: u32, pub screen_w: u32, pub screen_h: u32,
                     pub encryption_key: String, pub font_base: String /* … */ }
pub struct CommonEventTable { pub events: Vec<RawEvent> }
pub struct MapData { pub id: u32, pub width: u32, pub height: u32,
                     pub layers: TileLayers, pub events: Vec<MapEvent> }
pub struct TileSetTable { pub sets: Vec<TileSet> }   // 6 个 tileset，每个 31 组 autotile
```

```rust
// wolf-vm
pub struct Program { /* 550 公共事件 + 73 地图事件，已预编译 */ }
impl Program { pub fn compile(ce: &CommonEventTable, maps: &[MapData]) -> Result<Self, VmError>; }

pub struct Vm { /* 调度器 + 变量 + 实例池 */ }
impl Vm {
    pub fn new(program: Program) -> Self;
    /// 推进 1 帧：每个活跃实例执行到阻塞或指令预算耗尽；产出本帧副作用。
    pub fn tick(&mut self, input: &InputState, out: &mut SideEffectSink);
    pub fn start_event(&mut self, id: EventRef) -> InstanceId;
    pub fn snapshot_vars(&self) -> VarSnapshot;   // §8 差分测试用
}
```

```rust
// wolf-render
pub enum Blend { Normal, Multiply, Add, Subtract }   // 对应 Game.dat graphic_render_mode 0..3
pub enum DrawCmd {
    Tile   { layer: u8, map_pos: (i32, i32), chip: u32, flags: TileFlags },
    Sprite { tex: TexId, dst: Rect, src: Rect, blend: Blend, opacity: f32 },
    Text   { text: Rc<str>, pos: (f32, f32), size: f32, color: Color, outline: Option<Color> },
}
pub struct Renderer { /* … */ }
impl Renderer {
    pub fn new(window: Arc<Window>) -> impl Future<Output = Result<Self, RenderError>>;
    pub fn begin_frame(&mut self, cmds: &[DrawCmd]) -> Result<(), RenderError>;
    pub fn present(&mut self) -> PresentOutcome;   // 见 §3.3 wgpu 30 变更
}
```

---

## 2. 解释器与协程模型（本设计最关键的一节）

### 2.1 指令的数据模型

二进制里一条指令的记录结构（CommonEvent.dat 与 .mps 页内指令**同一套**，已逐字节验证）：

```
u8  pc           // = 参数个数 + 1；pc == 0 表示空指令（Blank），后面直接跟 1 字节 bd
    if pc > 0:
u32 cid          // opcode，见 CommandType 表
u32 args[pc-1]   // 整数参数
u8  bd           // branch_depth（缩进层）
u8  sc           // 字符串参数个数
t_str strings[sc]// t_str = u16 长度 + 字节（UTF-8 / SJK 由 Game.dat 决定）
u8  term         // 尾字节：0x00 = 普通结束；0x01 = 后面跟 Move route（见 §2.1.2）
```

参考实现一致：WolfTL `Command::DumpData` 写的是 `WriteByte(args.size()+1); WriteInt(cid); args...; WriteByte(m_indent); WriteByte(stringArgs.size()); strings...`。
**⇒ 加载器必须直接读这个 bd 字节，不要自己推。**

#### 2.1.1 opcode 覆盖率（决定实现优先级）

占 139,273 条指令的比例：401 ChoiceCase 13.3% · 210 CommonEvent 13.3% · 121 SetVariable 12.4% · 150 Picture 12.0% · 101 Message 8.6% · 499 BranchEnd 8.2% · 111 VariableCondition 7.7% · 103 Comment 7.0% · 180 Wait 3.4% · 420 ElseCase 2.4% · 250 Database 1.9% · 498 LoopEnd 1.4% · 179 LoopTimes 1.3% · 213 JumpLabel 1.1%。
**前 14 个 = 93.9%。**

> **⚠ 修正（推翻早期「地图侧是公共事件子集」的说法）**：Lead 用 `work/probe97.py` 实测差集 —— 公共事件非空 **47 种**、地图侧 **40 种**、交集 **37 种**、**并集 50 种 = 全集**。
>
> - **只在地图侧出现（3 种，公共事件里一个都没有）**：`151 ChangeColor` / `161 PrepareTransition` / `162 ExecuteTransition`。⇒ **转场语义完全住在地图事件里**：`162 ExecuteTransition` 是真正执行转场（淡入/淡出/颜色过渡）的那条指令。
> - **只在公共事件侧出现（10 种）**：`106 DebugMessage` / `107 ClearDebugText` / `160 SetTransition` / `171 BreakLoop` / `175 EndGame` / `176 StartLoop2` / `211 CommonEventReserve` / `222 SaveGame` / `281 ScrollScreen` / `1000 ProFeature`。

⇒ 第一版只需实现 ~30 个 opcode 就能跑通主线（前 14 个占 93.9%），其余按 opcode 表补。但 **`151` / `161` / `162` 必须在 M2/M3 的地图切换场景里补齐**：缺 `162` 的地图切换会「瞬间跳图、没有淡入淡出」，而且**公共事件里没有任何替代路径**（那里只有 `160 SetTransition`，它只登记参数、不执行转场）。详见 §10 里程碑的对应备注。

#### 2.1.2 尾字节不是 have_route，而是「带 route 的 Move」

Lead 逐字节验证：尾字节 `0x01` 后面是 `5 个未知字节 + 1 个 flags 字节 + u4 routeCount + RouteCommand[]`，`RouteCommand = u1 id, u1 argCount, argCount*u4, 尾部 2 字节 01 00`。

**为什么把它建模成「指令自带 route」而不是「独立指令」**：WolfTL `Command.hpp` 里 `Move(201)` 也走同一个 route 编解码器。统一成一个 `Route` 结构可以复用解析器与执行器；而角色移动是**独立于指令流的异步状态**（一个角色可以在事件继续跑的同时沿路径走），所以执行器必须把它挂到一个 `Mover` 组件上，而不是在 VM 里阻塞着走。

### 2.2 加载期预编译（把分支变跳转表）

#### 2.2.1 跳转表构造算法

对每个事件做**一遍**栈式扫描（bd 已在数据里，无需推导）：

```rust
// wolf-vm/src/compile.rs
const OPENERS: &[u16] = &[102, 111, 112, 170, 179]; // Choices, VarCond, StrCond, StartLoop, LoopTimes
const CLOSERS: &[u16] = &[499, 498];                 // BranchEnd, LoopEnd

pub fn build_jump_tables(code: &[Command]) -> Result<JumpTables, CompileError> {
    let mut stack: Vec<(u8 /*bd*/, u32 /*idx*/, u16 /*opcode*/)> = Vec::new();
    let mut branch_end   = vec![u32::MAX; code.len()]; // opener -> 匹配的 499/498
    let mut loop_back    = vec![u32::MAX; code.len()]; // 498 -> 匹配的 170/179
    let mut case_skip    = vec![u32::MAX; code.len()]; // 401/420/421/402 -> 下一个同类标签或终结符

    for (i, c) in code.iter().enumerate() {
        if c.bd as usize > MAX_BRANCH_DEPTH {              // 128 层上限，防恶意数据
            return Err(CompileError::TooDeepBranch(i));
        }
        if OPENERS.contains(&c.cid) {
            stack.push((c.bd, i as u32, c.cid));
        } else if CLOSERS.contains(&c.cid) {
            let Some(&(bd, oi, op)) = stack.last() else {
                return Err(CompileError::StrayCloser(i));   // 实测为 0，出现即数据坏了
            };
            if bd != c.bd {
                return Err(CompileError::UnmatchedCloser { at: i, opener: oi });
            }
            stack.pop();
            branch_end[oi as usize] = i as u32;
            if c.cid == 498 { loop_back[i] = oi; }
        }
    }
    if let Some(&(_, oi, _)) = stack.last() {
        return Err(CompileError::UnmatchedOpener(oi));
    }
    // case 标签链：401/402/420/421 -> 同一个 opener 块内、同 bd 的下一个标签或终结符
    // （401 与它的 opener 同 bd；body 在 bd+1 —— 这是实测结论）
    /* … 见 FORMAT.md §4.3 的精确规则 … */
    Ok(JumpTables { branch_end, loop_back, case_skip, label_pc: Default::default() })
}
```

**为什么用栈 + bd 而不是「按缩进解析成树」**：已有实测证明该规则在 163,828 个 slot 上 **0 例外**；栈式一遍扫描是 O(n)、零分配（除表本身）、且能对坏数据给出精确错误位置。对照实验：把 `176 StartLoop2` 也算 opener 会得到 13239 对 / 140 stray / 194 unmatched ⇒ 176 不满足该规则（见 §2.9 TODO）。

#### 2.2.2 编译产物

```rust
// wolf-vm/src/program.rs
pub struct CompiledEvent {
    pub id: u16,
    pub run_condition: RunCondition,
    pub name: Box<str>,
    pub code: Vec<Command>,
    /// 已把魔法数字解码成 VarRef，避免运行时再判区间
    pub ops: Vec<CompiledOp>,
    pub tables: JumpTables,
    /// 213 JumpLabel 已被解析成绝对 pc（同事件内）
    pub jump_targets: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunCondition { CallOnly = 0, AutoStart = 1, ParallelProcess = 2, ParallelAlways = 3 }

#[derive(Clone, Debug)]
pub struct JumpTables {
    pub branch_end: Vec<u32>,   // opener idx -> 匹配 closer idx
    pub loop_back:  Vec<u32>,   // 498 idx -> 对应 170/179 idx
    pub case_skip:  Vec<u32>,   // 401/420/421/402 idx -> 下一个 case 标签或终结符
    pub label_pc:   std::collections::HashMap<Box<str>, u32>, // 212 SetLabel
}
```

**212/213 的处理**：`212 SetLabel` 685 次、`213 JumpLabel` 1600 次。加载期按 `(event_id, label_name)` 建 HashMap，`213` 直接解析成绝对 pc 存进 `jump_targets`；运行时零字符串查找。

**为什么预编译而不是每帧线性搜索**：1024×576 60fps ⇒ 每帧预算 16.6ms；SampleMapA 单事件闭包就有 52,910 条指令，若 `401/499` 靠线性扫描找配对，最坏每帧几十万次访存。预编译后每帧只有 `Vec` 索引。

### 2.3 运行期对象

```rust
// wolf-vm/src/instance.rs
pub type InstanceId = u32;

/// 一个正在运行的事件实例（协程）。108 个并行事件 = 108 个 EventInstance。
pub struct EventInstance {
    pub id: InstanceId,
    pub event: EventKey,          // PublicEvent(u16) | MapPage { map: u32, event: u32, page: u32 }
    pub pc: u32,                  // 当前指令指针
    pub state: InstanceState,
    /// 210 CommonEvent 的嵌套调用帧（LIFO）。被调者与调用者共享本实例（同一协程）。
    pub frames: Vec<CallFrame>,
    /// cself 自身变量：每层调用帧各一份（100 槽）
    pub self_vars: [i64; SELF_VAR_COUNT],
    pub has_result: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstanceState {
    Running,
    Waiting(WaitKind),
    /// 已跑完（parallel_process 每帧重跑；call_only 结束即销毁）
    Finished,
    /// 指令预算耗尽，本帧让出（防死循环）
    Yielding,
}

/// 210 CommonEvent 推入的调用帧：必须保存返回点与「被调者的 cself」
pub struct CallFrame {
    pub event_id: u16,
    pub ret_pc: u32,
    pub self_vars: [i64; SELF_VAR_COUNT],
    /// 210 的参数：arg0.. 传值；若 arg0 是变量引用则回填返回值
    pub args: Vec<MagicNum>,
    pub ret_slot: Option<MagicNum>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitKind {
    /// 180 Wait(n)：单位是**帧**，不是毫秒（实测取值 1/15/10/50/30/5/100/20/80/40/2/180 帧）
    Frames(u32),
    /// 101 Message：等到消息窗口被关闭（玩家按键/自动播放计时）
    Message { window: u8 },
    /// 202 WaitForMove：等角色路线走完
    Move { target: MoverId },
    /// 161/162 转场
    Transition,
    /// 201 Move 内部派生出的等待
    Input { action: u16 },
}
```

**为什么 `frames: Vec<CallFrame>` 而不是给每个调用建新 `EventInstance`**：实测 442 个 `call_only` 事件里 39% 含 `Wait` ⇒ 调用是**阻塞**的。如果给被调者建独立实例，「阻塞」就得靠某种 join 原语；而放进同一个 `frames` 栈后，`Wait` 天然挂起整条链，`210` 的语义就是「压栈 + 跳转」，`ReturnToCaller` 就是「弹栈 + 恢复 pc」。这与 WOLF 的实际行为一致，且 `frames` 是普通 `Vec`，**天然可序列化**（§2.7 说明为什么这一点是硬需求）。

**`self_vars` 每层一份而不是全局一份**：`cself[0..99]` 的语义是「本事件自身的局部变量」。实测 `160xxxx` 出现在 210 调用的参数位上（例如 `[16, 1600021, 0, 1600080, …]`）⇒ 调用者把自己的 cself 当**实参**传进去。这与「每层一份 + 参数按值拷贝」的模型完全吻合。

### 2.4 调度模型

三类事件的推进策略（实测分布见 §0.1）：

| 类别 | 数量 | 何时创建 | 何时销毁 | 每帧推进 |
|---|---|---|---|---|
| `auto_start(1)` | 1 | 游戏启动 | 事件自行结束 | 是 |
| `parallel_process(2)` | 97 | 满足启动条件时 | 触发条件失效时 | 是 |
| `parallel_process_always(3)` | 10 | 无条件常驻 | 从不（除非 EndGame） | 是 |
| `call_only(0)` | 442 | 被 `210` 调用（压帧不建实例）/ 被 `211/300` 预约（建新实例） | 执行到 pc == end | 仅在有栈帧时 |
| 地图页事件 | 134 页 | 页触发条件满足 | 页切换/事件擦除 | 是 |

```rust
// wolf-vm/src/scheduler.rs
pub struct Scheduler {
    instances: Vec<EventInstance>,          // 已回收的槽位复用
    free: Vec<InstanceId>,
    /// 每帧按固定顺序推进：先 auto/parallel，再 map page，再 call
    pub order: InstanceOrder,
    pub per_instance_budget: u32,           // 默认 10_000 条指令/帧/实例
}

impl Scheduler {
    pub fn tick(&mut self, prog: &Program, vars: &mut VarStore, ctx: &mut Ctx) {
        for slot in self.instances.iter_mut() {
            if slot.state == InstanceState::Finished { continue; }
            if let InstanceState::Waiting(w) = &mut slot.state {
                if !ctx.poll_wait(w, slot) { continue; }   // 仍在等，本帧不推进
                slot.state = InstanceState::Running;
            }
            let mut steps = 0u32;
            while slot.state == InstanceState::Running {
                if steps >= self.per_instance_budget {
                    slot.state = InstanceState::Yielding;   // 下帧继续，防死循环
                    break;
                }
                steps += 1;
                step_one(slot, prog, vars, ctx);
            }
        }
        self.gc();
    }
}
```

**为什么 `per_instance_budget` 是必需的**：地图页指令 37,610 槽、公共事件 139,273 条里存在 `170 StartLoop` 这种无次数上限的循环（∞ 循环靠 `171 BreakLoop`/`172 BreakEvent` 退出）。如果某个事件逻辑上不退出，没有预算就会卡死整个进程。10,000 条/帧≈0.1ms 量级，对正常事件绰绰有余。

**为什么 `FrameArena` / object pool 而不是每帧 `Box::new`**：108 个并行实例 × 60fps 每秒创建 6,480 个对象；而事件实例是长期存活的（parallel_process 跨数千帧），所以用 `Vec + free list` 复用槽位，`InstanceId` 用「代数 + 索引」避免悬垂引用。

### 2.5 阻塞语义（`Wait` / `Message`）

**180 Wait 的单位是帧**（实测取值 1/2/5/10/15/20/30/40/50/80/100/180）。并行事件的 Wait 分布显示：20 个并行事件完全不 Wait（纯每帧轮询型，例如输入检测/画面效果），其余靠 `Wait(10)`/`Wait(6)`/`Wait(5)` 分频 ⇒ **Wait 就是「让出 N 帧"的协程 yield**。

```rust
/// 一次「指令步」：执行一条指令，可能改变 pc / state / frames
pub fn step_one(inst: &mut EventInstance, prog: &Program, vars: &mut VarStore, ctx: &mut Ctx) {
    let ev = &prog.events[inst.event];
    let op = &ev.ops[inst.pc as usize];
    match op {
        Op::Noop | Op::Comment(_) => inst.pc += 1,

        Op::Wait { frames } => {
            inst.state = if *frames == 0 { InstanceState::Running }
                         else { InstanceState::Waiting(WaitKind::Frames(*frames)) };
            inst.pc += 1;   // 下一帧从 Wait 之后继续
        }

        Op::Message { window } => {
            ctx.emit(SideEffect::ShowMessage { window: *window, text: op.text().clone() });
            inst.state = InstanceState::Waiting(WaitKind::Message { window: *window });
            inst.pc += 1;
        }

        // ---- 分支：全部走预编译表 ----
        Op::VarCondition { .. } => {
            let taken = eval_conditions(op, vars, inst);
            let target = if taken { inst.pc + 1 }              // 落到紧随其后的 401 case
                         else { ev.tables.case_skip[inst.pc as usize] };
            inst.pc = target;
        }
        Op::ChoiceCase { .. } => {
            // 401 与 opener 同 bd；branch_taken 时直接跳到终结符，否则比较 case 号
            match ctx.pending_case.take() {
                Some(sel) if sel == op.case_no() => inst.pc += 1,
                _ => inst.pc = ev.tables.case_skip[inst.pc as usize],
            }
        }
        Op::BranchEnd => {
            inst.pc = if inst.pc + 1 <= ev.ops.len() as u32 { inst.pc + 1 } else { u32::MAX };
        }

        Op::CommonEvent { event_id, args, ret_slot } => {
            // 210：压栈而不是新建实例 ⇒ 被调者内部的 Wait 会挂起整条链（实测语义）
            let callee = prog.events[event_id];
            inst.frames.push(CallFrame {
                event_id: *event_id, ret_pc: inst.pc + 1,
                self_vars: inst.self_vars,
                args: args.clone(), ret_slot: *ret_slot,
            });
            inst.self_vars = [0; SELF_VAR_COUNT];   // 被调者有独立的 cself
            ctx.enter_event(callee, inst);
            inst.pc = 0;
        }

        Op::End => { inst.state = InstanceState::Finished; }

        _ => { ctx.dispatch(op, inst, vars); inst.pc += 1; }
    }
}
```

**为什么「Message 阻塞」用 `WaitKind::Message` 而不是回调**：1024×576 的对话框有打字机效果 + 玩家按键跳过 + 自动播放计时 + `105 ForceStopMessage`，状态机在 `ctx£ 里；VM 侧只需要一个「等 window N 关闭」的挂起点。这样 `wolf-vm` 完全不需要知道任何 UI 细节。

**`Wait` 的计时精度**：用整数帧计数而非 `Instant`。理由：原游戏是固定 60fps 逻辑（`Wait(1)`=1/60s），用 `Instant` 会引入浮点漂移；固定步长累加器（§3.6）保证 `Wait(n)` 精确等于 n 帧。

### 2.6 变量存储

魔法数字统一解码（**先判大区间再取模**，这个顺序是实测结论）：

```rust
// wolf-format/src/magic.rs （wolf-vm 直接复用）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MagicNum {
    Literal(i32),
    SelfVar(u32),   // 1_600_000..1_700_000 —— 基址是 1600000，不是 WolfTL 注释写的 1000000
    Var(u32),       // 2_000_000..3_000_000
    StrVar(u32),    // 3_000_000..4_000_000（SetString 用 arg0 % 1_000_000）
    SysVar(u32),    // 9_000_000..9_100_000
}

impl MagicNum {
    pub fn decode(raw: i32) -> Self {
        let a = raw as u32;                     // 负数（如 -240）保持为巨大的 u32 ⇒ 落到 Literal
        if a >= 9_000_000 && a < 9_100_000 { MagicNum::SysVar(a - 9_000_000) }
        else if a >= 3_000_000 && a < 4_000_000 { MagicNum::StrVar(a % 1_000_000) }
        else if a >= 2_000_000 && a < 3_000_000 { MagicNum::Var(a - 2_000_000) }
        else if a >= 1_600_000 && a < 1_700_000 { MagicNum::SelfVar(a - 1_600_000) }
        else { MagicNum::Literal(raw) }
    }
}
```

实测命名空间分布（扫全部 550 事件）：

| 命名空间 | 常量 | 实测 distinct | 实测索引范围 | 建议容量 |
|---|---|---|---|---|
| 用户变量 V | `2_000_000 + i` | 578 | 0 .. 700_005（组 0 用 0..631；组 7 用 0..5） | `Vec<i64>` 按需增长到 `(max_group+1)*100_000`（实测 800_006 ⇒ 6.4MB） |
| 字符串变量 S | `3_000_000 + i`（取模 1_000_000） | 30 | 0 .. 219_201（组 {0:10, 1:16, 2:4}，槽最大 70304） | `Vec<Box<str>>` 按需增长 |
| 系统变量 | `9_000_000 + i` | 53 | 0 .. 118 | 固定 256 槽 |
| cself | `1_600_000 + i` | 95 | 0 .. 99 | 固定 100 槽（每个调用帧一份） |
| 立即数 | 其它 | — | 含负数（`-240/-256` 出现在 Sound 停止操作） | — |

```rust
// wolf-vm/src/vars.rs
pub struct VarStore {
    user: Vec<i64>,          // 扁平：index = raw - 2_000_000；group = index / 100_000, slot = index % 100_000
    string: Vec<Box<str>>,   // 扁平：index = raw % 1_000_000
    sys: [i64; SYS_VAR_COUNT],  // 256
}
impl VarStore {
    #[inline] pub fn get(&self, v: MagicNum, self_vars: &[i64; SELF_VAR_COUNT]) -> i64 {
        match v {
            MagicNum::Literal(n) => n as i64,
            MagicNum::Var(i) => self.user.get(i as usize).copied().unwrap_or(0),
            MagicNum::SelfVar(i) => self_vars[i as usize],
            MagicNum::SysVar(i) => self.sys[i as usize],
            MagicNum::StrVar(i) => self.string.get(i as usize).map_or(0, |_| 0),
        }
    }
    #[inline] pub fn set_user(&mut self, i: u32, val: i64) {
        let i = i as usize;
        if i >= self.user.len() { self.user.resize(i + 1, 0); }   // 按需增长
        self.user[i] = val;
    }
}
```

**为什么用户变量用「扁平 Vec 按需增长」而不是二维数组 `[[i64; 1000]; 100]`**：实测最大索引 700_005（组 7 只有槽 0..5），二维化要预分配 100×1000 且组 0 之外的组几乎全空。扁平增长的实际峰值是 800_006 × 8B = 6.4MB——比二维更省也更简单；同时保留 `group = index / 100_000` 的解释只用于调试打印。

**为什么 `i64` 而不是 `i32`**：WOLF 的运算含乘法、位运算与科学计算（`sin[×1000]`/`√[×1000]`，见 WolfTL `Command.hpp` 的运算符表），中间结果可能溢出 32 位；原引擎内部是 64 位整数变量。用 `i64` 并**在 `/` 与 `%` 时按原引擎语义处理 0 除**（结果为 0，不 panic）。

### 2.7 为什么不用 corosensei / OS 线程（关键取舍）

候选方案对比：

| 方案 | 能否表达 108 并行 + 阻塞调用 | 能否快照/恢复（存档与差分测试） | unsafe / 依赖成本 | 结论 |
|---|---|---|---|---|
| **A. 显式状态机 + 调用帧栈（本文选）** | 能（pc + frames + WaitKind 就是完整续体） | **能**：整个 `EventInstance` 是 Plain Old Data，直接 serde | 零 unsafe | **选它** |
| B. `corosensei` 真栈协程 | 能，且代码像同步函数 | **不能**：续体在原生机器栈上，无法序列化/恢复 | 每个协程一份栈（默认几十 KB × 108 ≈ 数 MB）+ 大量 unsafe | 不选 |
| C. OS 线程 / async runtime | 能 | 不能（同上，且调度不可复现） | 线程栈 1MB×108 = 108MB，且不可确定性调度 | 不选 |
| D. 单调用栈 + 轮询所有事件 | 不能：210 的阻塞调用无法表达 | 能 | 低 | 不选 |

**决定性理由**：WOLF 的存档要保存「每个并行事件运行到哪、等什么、等了多久」。用真栈协程（B/C）可以跑，但**存档与逐帧差分测试都做不了**——而 §8 的测试策略（Save-Diff Oracle + 逐帧变量快照）正是本项目在没有官方运行时对照的情况下唯一的正确性保证。方案 A 用一个 `Vec` + `u32` 就换来完整的可快照性，代价只是每个事件多写 ~200 行 match 分支。

**corosensei 0.3.4 的处理**：从依赖表移除（见 §7 备注）。若将来做「剧情脚本专用 DSL」可再引入，但**不允许进入 `wolf-vm` 的核心路径**。

### 2.8 `210` / `211` / `300` 的分派语义

- `210 CommonEvent`：arg0 编码 = **`500_000 + event_id`**（18427 个字面调用全部落在合法 id 区间；实测 41 个动态调用，取值 `600100/600098/1600024/600097/1600021` 等）。⇒ 解码规则：`if (500_000..600_000).contains(&a) { LiteralEvent(a - 500_000) } else if a 是变量引用 { Indirect }`。`600xxx` 是另一段命名空间（疑似「按索引表调用」），v1 记为 **TODO(verify)** 并落 warning。
- `211 CommonEventReserve`：预约式——**新建并行实例**而不是压栈（§2.9 TODO(verify) 待用官方存档对照确认）。
- `300 CommonEventByName`：字符串名分发（实测字符串形如 `X[xx]システムSE再生`）。加载期建 `HashMap<name, event_id>`；参数里的 `16777217 = 0x01000001` 是状态位（bit24 + bit0），需在实现期用对照实验确认含义。

### 2.9 已知未决项（给 vm-spec 的实现期任务）

1. **`176 StartLoop2`（44 次出现，bd 2..7）**：不满足 §2.2 的配对规则（放进 opener 集合会产生 140 stray + 194 unmatched）。样例中它总出现在某个 111 块的 body 层，之后是同层 `0` 与 `499@(bd-1)`。v1 按 **no-op + 计数器告警**处理，并单独写一个 probe 确认语义。
2. **`600xxx` 动态调用**：语义未知，按「先落 warning 再当惰性调用」处理。
3. **`401` 的 `branch_taken` 复用**：本设计用 `ctx.pending_case: Option<u32>` 表达「刚算出的 case 值」，`420 ElseCase`/`421 CancelCase` 需要区分「未命中任何 case」与「Choices 被取消」两种状态；具体状态位见 FORMAT.md。
4. **`111` 条件的 `count` 高 4 位是 flags**：必须 `n = count & 0x0F`（实测 17/18/19/20 分别对应 1/2/3/4 个条件；`102 Choices` 同样）。**不按 `& 0x0F` 解码会直接越界读。**
5. `107/105/106` 调试类指令、`1000 ProFeature`：v1 按 no-op + 日志（本作是 Pro 版，`1000` 会出现；见 WolfTL `Command.hpp` 的 Type 映射表）。

---

## 3. 渲染管线

### 3.1 技术栈与版本锁定（必须锁死）

```toml
# 已用 crates.io API 逐个核实 max_stable，下列版本号全部正确
wgpu          = "30.0.1"   # MSRV 1.87；底层 wgpu-core/wgpu-types = 30.0.0
winit         = "0.30.13"
glyphon       = "0.12.0"   # Cargo.toml 里 wgpu = "30.0.0" ⇒ 与上面同一个 wgpu，不会引入第二份
cosmic-text   = "0.19.0"
bytemuck      = "1.25.2"
glam          = "0.34.1"
```

**为什么必须锁死 glyphon 与 wgpu 的精确版本**：glyphon 0.12 直接把 `wgpu::Device`/`RenderPass` 作为 API 参数暴露，wgpu 的小版本就会改这些类型的形状（本版已实证：`Surface::get_current_texture` 返回值变了）。若 Cargo 解析出两个不同 wgpu 版本，glyphon 的 `PrepareError` 与我们的 `wgpu` 类型不匹配，编译期会报「同名不同类型」。对策：`Cargo.lock` 入库 + `[workspace.dependencies]` 统一版本 + CI 跑 `cargo tree -d` 断言无重复 wgpu。

### 3.2 wgpu 30 的破坏性变更（逐条，全部从源码实测）

| 变更 | 旧写法 | wgpu 30 写法 | 影响 |
|---|---|---|---|
| 取后备缓冲 | `get_current_texture() -> Result<SurfaceTexture, SurfaceError>` | `-> CurrentSurfaceTexture` 枚举：`Success` / `Suboptimal` / `Timeout` / `Occluded` / `Outdated` / `Lost` / `Validation` | 渲染循环的 match 必须重写；`Suboptimal` **也带 tex，可照常画** |
| present | `surface_texture.present()` | `queue.present(surface_texture)` | 需要持有 Queue；present 与 submit 解耦 |
| Instance 构造 | `InstanceDescriptor::default()` | 同签名，但 **`InstanceDescriptor` 新增 `display: Option<Box<dyn WgpuHasDisplayHandle>>`** | 传 EventLoop 的 OwnedDisplayHandle；或 `new_without_display_handle()` |
| 请求适配器 | `-> Option<Adapter>` | `-> Result<Adapter, RequestAdapterError>` | `.ok_or(...)?` 改成 `?` |
| 请求设备 | `request_device(&desc, None)` | `request_device(&desc)`（trace 并入 `DeviceDescriptor.trace`） | 少一个参数 |
| 设备描述 | 无 experimental_features / memory_hints | `{ label, required_features, required_limits, experimental_features, memory_hints, trace }` | `default()` 够用，但显式给 `Features::empty()` |
| 缓冲/纹理拷贝 | `ImageCopyTexture` / `ImageDataLayout` | `TexelCopyTextureInfo` / `TexelCopyBufferLayout` | 仅改名 |
| 顶点/片元入口 | `entry_point: &str` | `entry_point: Option<&str>` | 显式传 `Some("vs_main")` |
| 顶点缓冲 | `buffers: &[VertexBufferLayout]` | `buffers: &[Option<VertexBufferLayout>]` | 传 `&[Some(layout)]` |
| 颜色目标 | `targets: &[ColorTargetState]` | `targets: &[Option<ColorTargetState>]` | 同上 |
| 渲染通道 | 无 multiview | `RenderPassDescriptor.multiview_mask`、`RenderPassColorAttachment.depth_slice` | 单视图忽略 |
| Instance::new 传参 | `Instance::new(&desc)` | **`Instance::new(desc)`（按值，不是引用）** | 编译器会直接提示，但抄教程必错 |
| 枚举适配器 | `instance.enumerate_adapters(b) -> Vec<Adapter>` | **返回 `impl Future<Output = Vec<Adapter>>`**，必须 `block_on`/`await` | 启动时打印适配器会编译不过 |
| 请求适配器 | `RequestAdapterOptions { power_preference, compatible_surface, force_fallback_adapter }` | **新增 `apply_limit_buckets: bool`** | 结构体字面量必须补字段 |
| 管线 multiview | `RenderPipelineDescriptor.multiview` | **改名 `multiview_mask: Option<NonZeroU32>`** | 编译期报 no field |
| 设备轮询 | `device.poll(Maintain::Wait)` | **`PollType::Wait { submission_index: Option<T>, timeout: Option<Duration> }`（struct variant）** | 不能当值用，必须构造 |
| 表面配置 | 无 color_space / frame latency | `{ color_space: SurfaceColorSpace::Auto, desired_maximum_frame_latency: 2, .. }` | 延迟**显式设 2**（输入延迟敏感） |

> ✅ **本表已实测编译通过**（本次工具安装阶段）：`tools/wgpusmoke/` 用 `wgpu 30.0.1 + winit 0.30.13 + glyphon 0.12.0 + kira 0.12.5` 编出可执行文件，
> 并在真机上**离屏渲染到 1024×576 成功**。实测适配器：RTX 5060 Laptop（Vulkan，driver NVIDIA 610.88）/ 同卡 Dx12（32.0.16.1088）/ GL 4.6 / Microsoft Basic Render Driver(CPU)，共 5 个；
> `max_texture_dimension_2d = 32768`、`max_buffer_size = 4095 MiB`。可复现命令见 [TOOLING.md](TOOLING.md) §5。
> 上表最后 5 行是本次真正编译后才暴露出来的、原稿未记录的错误。

**`CurrentSurfaceTexture` 的标准处理循环**（wgpu 30 与 0.19/0.20 差异最大的地方）：

```rust
// wolf-render/src/surface.rs
pub enum FrameOutcome { Rendered, Skip }

pub fn draw_frame(&mut self) -> Result<FrameOutcome, RenderError> {
    use wgpu::CurrentSurfaceTexture as Cst;
    let frame = match self.surface.get_current_texture() {
        Cst::Success(f) | Cst::Suboptimal(f) => f,   // Suboptimal 仍可画，最多尺寸略旧
        Cst::Timeout => return Ok(FrameOutcome::Skip),   // 逻辑与渲染解耦，不阻塞 VM
        Cst::Occluded => return Ok(FrameOutcome::Skip),  // 最小化：本帧不画
        Cst::Outdated | Cst::Lost => {
            let cfg = self.surface.get_configuration().ok_or(RenderError::NoConfig)?;
            self.surface.configure(&self.device, &cfg);
            return Ok(FrameOutcome::Skip);
        }
        Cst::Validation => return Err(RenderError::SurfaceValidation),
    };
    // … 录制 pass …
    self.queue.submit(Some(encoder.finish()));
    self.queue.present(frame);      // ← present 在 Queue 上
    Ok(FrameOutcome::Rendered)
}
```

### 3.3 winit 0.30 的结构性变化（必须先立骨架）

winit 0.30 把事件循环改成「控制反转」：没有 `EventLoop::run` 闭包，而是 `ApplicationHandler` trait + `run_app`。

```rust
// wolf-game/src/app.rs
use winit::application::ApplicationHandler;
use winit::event::{WindowEvent, KeyEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId, WindowAttributes};

pub struct App {
    window: Option<Window>,     // ← 只能在 resumed() 里创建
    renderer: Option<wolf_render::Renderer>,
    audio: wolf_audio::AudioEngine,
    vm: wolf_vm::Vm,
    acc: f64,                   // 固定步长累加器：逻辑 60Hz，渲染尽量快
    last: std::time::Instant,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        // ★ wgpu 的 Instance / Surface / Adapter / Device 全在这里初始化：
        //   create_surface 需要窗口句柄（SurfaceTarget::Window）。
        let attrs = WindowAttributes::default()
            .with_title("Living With Sister Monochrome Fantasy (Rust)")
            .with_inner_size(winit::dpi::LogicalSize::new(1024, 576))
            .with_resizable(true);
        let window = el.create_window(attrs).expect("create_window");
        // 用 pollster 把 wgpu 的 async 初始化在同步上下文里跑完
        let renderer = pollster::block_on(wolf_render::Renderer::new(&window)).expect("renderer");
        self.window = Some(window);
        self.renderer = Some(renderer);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(_) => { /* surface.configure 延迟到 draw 时做 */ }
            WindowEvent::RedrawRequested => { /* 交给 about_to_wait 的统一 tick */ }
            WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state, .. }, .. } => {
                self.vm.input.push_key(logical_key, state);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        // 固定步长：acc 累加真实时间，while acc >= 1/60 { vm.step(); acc -= 1/60 }
        // 渲染一帧；然后 window.request_redraw() 保持连续重绘
    }
}

pub fn run() -> Result<(), winit::error::EventLoopError> {
    let el = EventLoop::new()?;      // ★ 必须在主线程
    let mut app = App::default();
    el.run_app(&mut app)
}
```

**为什么用「`about_to_wait` 里统一 tick」而不是 `RedrawRequested` 驱动**：`RedrawRequested` 在部分平台（Windows 最小化 `Occluded` 时）会停止投递，而 WOLF 的 108 个并行事件依赖**逻辑每帧推进**——存档计时、剧情计时、SE 淡出都在并行事件里。逻辑与渲染解耦后，窗口最小化也照样推进逻辑（这正是原版 `parallel_process_always` 的语义）。渲染 `Skip` 不影响 VM。

### 3.4 逻辑分辨率与整数倍最近邻放大

**为什么用「1024×576 离屏渲染 + 整数倍放大」而不是「按窗口大小直接渲染」**：
1. 原游戏是像素艺术（tile 16px、角色 16×16/16×32 网格），非整数倍缩放会出现**像素撕裂与接缝抖动**（tile 之间出现 1px 空隙），这是像素游戏最刺眼的瑕疵；
2. WOLF 的 `150 Picture` 坐标与事件里的分支常量按「画面宽 1024」写死，逻辑分辨率必须固定；
3. 整数倍放大让 `Nearest` 采样数学上等价于「复制像素」，文字与 sprite 边缘完全锐利。

```rust
pub const LOGICAL_W: u32 = 1024;
pub const LOGICAL_H: u32 = 576;

pub struct Scaler { pub scale: u32, pub offset: [f32; 2] }

impl Scaler {
    pub fn new(win_w: u32, win_h: u32) -> Self {
        let s = (win_w / LOGICAL_W).min(win_h / LOGICAL_H).max(1);   // 整数倍，至少 1×
        let used_w = LOGICAL_W * s;
        let used_h = LOGICAL_H * s;
        Scaler {
            scale: s,
            // 居中且取整到像素网格，避免半像素导致的模糊
            offset: [((win_w - used_w) / 2) as f32, ((win_h - used_h) / 2) as f32],
        }
    }
}
```

放大方式：**不用 blit shader，直接把离屏纹理当作全屏三角形上的纹理采样**，采样器 `FilterMode::Nearest`、`AddressMode::ClampToEdge`，顶点着色器把 `scale + offset` 变成 NDC。窗口任意大小都只画 1 个 quad。

**为什么不用 `FilterMode::Linear` 做「平滑缩放」**：与原版观感不符。原版 DxLib 用 `SetGraphMode` 直接切分辨率（1024×576 或全屏整数倍）。保留 `Nearest` + 整数倍是忠实复刻；留一个 `Config { scaler: Nearest | Linear }` 开关（`Linear` 时 offset 取小数）。

### 3.5 图层栈（WOLF 的绘制模型）

WOLF 的绘制顺序是多层复合的：`150 Picture` 有 `layer` 参数，与消息窗口、地图图层交错。设计成**统一的 Z 排序绘制列表**：

```rust
// wolf-render/src/layer.rs
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct ZKey { pub band: u8, pub sub: i32 }   // 先比 band，再比 sub

pub enum Band {
    MapChipFar     = 0,   // 地图 3 层里的最底
    MapChipMiddle  = 10,
    PictureBelow   = 20,  // 150 Picture 中 layer < 0 的
    MapChipNear    = 30,
    Chara          = 40,  // 角色 / 事件图形
    PictureAbove   = 50,  // 150 Picture 中 layer >= 0 的
    Weather        = 60,  // 200 Weather / Fog
    MessageWindow  = 70,  // 101 Message 的窗口与文字
    ChoiceWindow   = 75,  // 102 Choices
    Balloon        = 78,  // 气球 / 表情
    DebugOverlay   = 90,  // 调试 HUD（F3）
}

/// 一个可绘制项：来自地图、角色，或 150 Picture
pub struct DrawItem {
    pub z: ZKey,
    pub kind: DrawKind,
    pub tex: TextureId,
    pub src: [f32; 4],        // UV 矩形（归一化）
    pub dst: [f32; 4],        // 逻辑像素矩形
    pub tint: [f32; 4],       // 颜色 / 不透明度合成
    pub blend: Blend,         // Picture 的 renderMode
    pub xform: Option<glam::Affine2>,   // 150 Picture 的旋转/缩放
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Blend { Normal, Add, Sub, Mul, Screen }   // 实测 renderMode 有这几种
```

**为什么用「排序的绘制列表」而不是「按 pass 固定顺序」**：`150 Picture` 每帧会产生几十个 item（实测 150 占 12.0% 的指令），若每个 Picture 一个 render pass 就是几十次 `set_pipeline`；排序后按 `(texture, blend, xform-presence)` 分桶，同桶合并成**一次 instanced draw**。同时 ZKey 让「角色被前景瓦片遮挡」这类效果天然成立，不需要另写规则。

### 3.6 固定步长循环

```rust
pub const TICK_HZ: u32 = 60;
pub const TICK: f64 = 1.0 / 60.0;

pub fn advance(vm: &mut wolf_vm::Vm, acc: &mut f64,
               now: std::time::Instant, last: &mut std::time::Instant) {
    let dt = (now - *last).as_secs_f64();
    *last = now;
    // 防「调试断点/窗口拖动」后的时间爆炸：单帧最多补 5 tick
    *acc += dt.min(TICK * 5.0);
    let mut n = 0;
    while *acc >= TICK && n < 5 {
        vm.step();          // 一次逻辑帧：推进 108 个并行事件 + 有栈帧的 call 事件
        *acc -= TICK;
        n += 1;
    }
}
```

**为什么不用 `ControlFlow::WaitUntil` + 精确睡眠**：Windows 上 `WaitUntil` 的分辨率受 `timeBeginPeriod` 影响，会漂移到 ~15.6ms；且 `WaitKind::Frames(n)` 的语义要求「n 次逻辑步」。逻辑步长固定 + 累加器是唯一能保证 `Wait(30)` 恰好 0.5s 的做法。渲染帧率随 vsync 走（`PresentMode::AutoVsync`），与逻辑解耦。

### 3.7 瓦片绘制（instanced quad + 图集）

WOLF 地图是 `w×h×3 层`：`SampleMapA` 64×36、`Map003` 70×170。单帧全部 tile 也才 70×170×3 = 35,700 个 quad —— 用 instanced quad 一次 draw 完成。

```wgsl
// tile.wgsl（要点，非全文）
struct TileInst {
  @location(0) dst_xy: vec2<f32>,     // 逻辑像素
  @location(1) src_uv:  vec4<f32>,
  @location(2) flags:   u32,          // bit0: 水平翻转, bit1: 垂直翻转, bit2: 加法混合
};
struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) @interpolate(flat) flags: u32,   // ★ 整数必须 flat，否则 naga 编译失败
};
@vertex fn vs_main(@builtin(vertex_index) vi: u32, inst: TileInst) -> VsOut {
  // 用 vertex_index 生成单位 quad 的 4 个角，避免每实例塞 6 个顶点
  let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
  let p = inst.dst_xy + corner * TILE_PX;
  let ndc = to_ndc(p);   // 逻辑像素 → NDC，含 global_uniform 的 scale/offset
  var uv = inst.src_uv.xy + corner * (inst.src_uv.zw - inst.src_uv.xy);
  if (inst.flags & 1u) != 0u { uv.x = inst.src_uv.z - (uv.x - inst.src_uv.x); }
  return VsOut(ndc, uv, inst.flags);
}
```

**关键点**：
1. **整数 shader I/O 必须 `@interpolate(flat)`**（WGSL 规定），否则 naga 直接编译失败。这是 wgpu 30 下最常见的坑。
2. 顶点数据用 `@builtin(vertex_index)` 生成 quad，实例缓冲只放 `dst_xy / src_uv / flags`（20B/实例），35,700 实例 ≈ 714KB/帧 —— 60fps 下这是浪费。**真正做法是「静态几何缓存」**：
   - 地图瓦片只在**层内容变化时**重建实例缓冲（`TilemapCache` 持 `wgpu::Buffer` + `dirty` 位）；
   - 角色与 Picture 每帧写进一块 `frame_instances` staging buffer（`queue.write_buffer`）；
   - 每个「图集页」一个 bind group，一次 `draw(0..4, 0..n)` 画完所有实例。

```rust
// wolf-render/src/tilemap.rs
pub struct TilemapCache {
    pub instance_buf: wgpu::Buffer,
    pub bind_group:  wgpu::BindGroup,
    pub count: u32,
    pub dirty: bool,          // 任一 tile 变化（地图改写指令）时置位
}
/// 只在 dirty 时重建；64×36 的图 rebuild 约 35,700×20B = 714KB 上传，<0.5ms
pub fn rebuild(&mut self, queue: &wgpu::Queue, map: &wolf_format::Map, atlas: &Atlas) -> u32 {
    if !self.dirty { return self.count; }
    let mut insts: Vec<TileInst> = Vec::with_capacity((map.w * map.h * 3) as usize);
    for _layer in 0..3 { /* 压入每个可见 tile 的实例 */ }
    queue.write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(&insts));
    self.count = insts.len() as u32;
    self.dirty = false;
    self.count
}
```

**为什么不做「每帧重建整个 tile 实例缓冲」**：70×170 的地图每帧 upload 714KB → 60fps = 43MB/s 的 PCIe 流量，在集显/低端机上会挤占带宽。缓存 + dirty 只在「地图改写指令」与地图切换时重建。

### 3.8 文字：glyphon 0.12 + 描边

WOLF 的消息文本支持内联命令（颜色、描边/阴影变体、等待、停顿、一次性显示、字号、换行等）。**实现策略：VM 只负责分段 + 计时，渲染层只拿到「若干 run」**。

```rust
// wolf-render/src/text.rs
pub struct TextRun {
    pub text: String,           // 已剥离内联命令的纯文本段
    pub color: [u8; 4],
    pub font_size: f32,
    pub outline: OutlineSpec,
    pub reveal: usize,          // 打字机：本帧显示前 N 个字符（按字节边界安全切）
}
#[derive(Clone, Copy)]
pub struct OutlineSpec { pub radius: f32, pub color: [u8; 4] }   // 默认 radius=2.0、黑色

pub struct TextLayer {
    pub font_system: cosmic_text::FontSystem,
    pub swash_cache: cosmic_text::SwashCache,
    pub atlas: glyphon::TextAtlas,
    pub viewport: glyphon::Viewport,
    pub renderer: glyphon::TextRenderer,
}
```

**描边实现**：glyphon 0.12 **没有内置描边**，只有 `TextArea { buffer, left, top, scale, bounds, default_color, custom_glyphs }`。做法是**同一段文字画 8 次**，每次偏移一个方向（`(±r,0),(0,±r),(±r,±r)`）用描边色，最后原色画正中；glyphon 允许 `TextArea` 多次引用同一个 `Buffer`，正好零复制：

```rust
/// 生成描边 + 正文的 TextArea 列表；全部引用同一个 Buffer，零复制
pub fn areas_for<'a>(run: &TextRun, buf: &'a cosmic_text::Buffer) -> Vec<glyphon::TextArea<'a>> {
    let mut v = Vec::with_capacity(9);
    if run.outline.radius > 0.0 {
        let r = run.outline.radius;
        for (dx, dy) in [(-1.,0.),(1.,0.),(0.,-1.),(0.,1.),(-1.,-1.),(1.,-1.),(-1.,1.),(1.,1.)] {
            v.push(glyphon::TextArea {
                buffer: buf,
                left: run.left() + dx * r,
                top: run.top() + dy * r,
                scale: 1.0,
                bounds: glyphon::TextBounds::default(),
                default_color: glyphon::Color::rgba(run.outline.color[0], run.outline.color[1],
                                                    run.outline.color[2], run.outline.color[3]),
                custom_glyphs: &[],
            });
        }
    }
    v.push(glyphon::TextArea {
        buffer: buf, left: run.left(), top: run.top(), scale: 1.0,
        bounds: glyphon::TextBounds::default(),
        default_color: glyphon::Color::rgba(run.color[0], run.color[1], run.color[2], run.color[3]),
        custom_glyphs: &[],
    });
    v
}
```

**为什么用「8 次偏移」而不是 SDF / 自定义 shader**：glyphon 的 `custom_glyphs` 与 `prepare_with_custom`（`RasterizeCustomGlyphRequest -> Option<RasterizedCustomGlyph>`）允许接管光栅化，但代价是自己写字形光栅器。8 次偏移在 `Nearest` 整数倍放大下**就是像素级正确的描边**（原版描边正是 8 邻域膨胀），且完全复用 glyphon 的 atlas 缓存。**代价**是文本区数量 ×9；对话框每帧 ~300 字 ⇒ 2400 个 glyph，`Cache` 命中时只是 Vec 写 vertex，成本可忽略。

**为什么每帧调用 `Viewport::update(queue, Resolution)`**：它内部只在尺寸变化时写 uniform（viewport.rs:46），每帧调用无副作用。

**对话框深度与图层**：glyphon 0.12 有 `prepare_with_depth`（多一个 `metadata_to_depth: impl FnMut(usize) -> f32`）。可以把它用于「窗口底图 vs 文字」的前后关系。**但主路径仍是 2D 排序 + `depth_stencil: None`**：`Cache::new` 会按 `Option<DepthStencilState>` 缓存 pipeline，若启用深度，`TextRenderer::new` 的 `depth_stencil` 必须与主 pipeline 完全一致（同格式、同 `depth_write_enabled`、同 `depth_compare`），否则 `Cache` 会创建两套 pipeline 并在 `render` 阶段校验失败。

### 3.9 `150 Picture` 的动态图层与混合

实测 `150 Picture` 的参数含 picture_id、x、y、layer、opacity、render_mode、scale/rotate 子模式；12.0% 的指令是它，`Picture` 目录有 **539 个文件**（含 CG / 立绘）。

```rust
/// 一张正在显示的 Picture（VM 侧登记，渲染侧只读）
pub struct PictureSlot {
    pub slot: u8,               // 画面上的 Picture 槽位号（指令里可选指定）
    pub tex: TextureId,
    pub x: i32, pub y: i32,
    pub layer: i32,
    pub opacity: u8,            // 0..255
    pub render_mode: Blend,
    pub xform: Option<glam::Affine2>,
    pub visible: bool,
}
```

**混合模式映射**到 `wgpu::BlendState`：
- `Normal`：`src_alpha / one_minus_src_alpha`；
- `Add`：`src_alpha / one`；
- `Sub`：`BlendOperation::ReverseSubtract`（**alpha 分量要单独设**）；
- `Mul`：`dst / zero`；
- `Screen`：`one / one_minus_src`。

**为什么每个 Blend 一个 pipeline**：wgpu 的 `BlendState` 是 pipeline 创建期固定量，不能运行时改。5 种 blend ⇒ 5 个 pipeline（配 `PipelineCache` 复用 shader 编译）。**避免组合爆炸**：只做 2 套 shader（无变换 / 有 `Affine2`），blend 用 pipeline 参数化 ⇒ 10 个 pipeline，启动期全建好。

**不用 `PushConstant`**（DX12 后端支持有限），把每 item 的变换写进实例缓冲（20→52 字节），保持单一 bind group。

### 3.10 角色动画

- `CharaChip`：每角色一个文件，含 **8 方向 × 3 帧** 的 16×16（或 16×32 高）网格 —— 图集索引按 `(dir, frame)` 定位。
- 方向顺序（下/左/右/上 + 4 斜向）按样本图确一版，写进常量表 `DIR_ORDER`（实现期任务）。
- 动画步进由 VM 的 `201 Move`/`202 WaitForMove` 驱动（角色移动速度 = 每帧步长），**不由渲染层定时器驱动**。原因：地图事件里有「等待移动结束(202)」的同步点，动画相位必须与 VM 逻辑帧一致。

### 3.11 内存预算（1080p / 4× 放大）

| 项 | 预算 | 说明 |
|---|---|---|
| 离屏 1024×576 RGBA8 | 2.25MB | 主渲染目标 |
| 交换链（1280×720 ×2 帧） | 7.4MB | `desired_maximum_frame_latency = 2` |
| 图集 MapChip（688K 源） | ≤32MB | 打包后 RGBA8，含 autotile 4 象限展开 |
| 图集 CharaChip（300K） | ≤24MB | 8×3 网格全展开 |
| Picture 图集（24M 源，539 文件） | **LRU 8 页 × 8MB = 64MB** | 见 §4.5 流式策略 |
| glyphon 双 atlas（color + mask） | 16MB | 按需增长 |
| VM + 事件实例（108 × ~2KB） | <1MB | 见 §2.3 |
| 变量（800,006 × i64 + 字符串） | 7MB | 见 §2.6 |
| **合计常驻** | **~155MB** | 16GB 内存下极宽松 |

**为什么给 Picture 图集设 LRU 上限而不是全量常驻**：`Picture.wolf` 24MB、`cg.wolf` 118MB、`hscene.wolf` 20MB —— 但 CG/H 场景是**事件触发才显示的大图**，同一时刻最多显示 1-2 张全屏图。LRU 上限 64MB 能装下 8 张 1024×576 全屏 CG，且能扛住 CG 切换抖动。

---

## 4. 资源管线

### 4.1 全链路

```
用户自己的 Data/（只读，运行时绝不写入）
   │  .wolf（DXArchive v8：64B 明文头 + 头部区域哈夫曼压缩 + 载荷 LZ+Huffman+可选 KeyConv）
   ▼
[1] wolf-format::wolf  归档读取器：解析 64B DARC_HEAD → 解哈夫曼头部 → 文件名字表 → FILEHEAD 表 → DIRECTORY 表
   ▼
[2] ArchiveIndex：内存索引（路径 → FileEntry { size, addr, press_size, huff_size }），按需读，不整包 mmap 解压
   ▼
[3] 按需解压单文件载荷（LZ + Huffman + KeyConv）→ Vec<u8>
   ▼
[4] 类型分派：.dat/.mps → 数据层解析器；.png/.jpg → 图像解码；.mp3/.ogg/.wav → 交给 kira 的 symphonia
   ▼
[5] PNG palette 解码 → RGBA8 → 图集打包 / 或直喂 kira 的 Cursor
```

**为什么索引与载荷分离**：4099 个文件、1.25GB 媒体。启动时只解压**头部区域**（每个 .wolf 一个 64B 头 + 压缩的头部块），拿到全部文件名与偏移（实测 `BasicData.wolf` 头部 15,240 字节；`word.wolf` 头部 0x3658 = 13,912 字节）。载荷只在**真正被引用时**才解。启动开销 = 23 个归档的头部解压，量级 <10ms。

### 4.2 归档读取器（v8）核对清单

关键结构以 Lead 侧的实现为准（`.recon/UberWolfDX/DXArchive.h` 是权威）。实现必做的自检：

```rust
// wolf-format/src/wolf/header.rs（v8，#[repr(C, packed)] 照抄 DXArchive.h）
pub const DXA_VER: u16 = 0x0008;
pub const DXA_HEAD_SIZE: usize = 0x40;
pub const DXA_KEY_BYTES: usize = 7;
pub const DXA_FLAG_NO_KEY:        u32 = 0x0000_0001;
pub const DXA_FLAG_NO_HEAD_PRESS: u32 = 0x0000_0002;

#[derive(BinRead, Debug)]
#[br(little, magic = b"DX")]
pub struct DarcHead {
    pub version: u16,                  // 必须 == 8
    pub head_size: u32,
    pub data_start: u64,
    pub name_table_start: u64,
    pub file_table_start: u64,
    pub dir_table_start: u64,
    pub char_code: u32,                // 932 = SJIS，65001 = UTF-8
    pub flags: u32,
    pub huffman_kb: u8,
    #[br(count = 15)] pub reserve: Vec<u8>,
}
```

**必须实现的 4 条断言**（它们把「解析错了」与「数据合法」区分开）：
1. `version == 8`；
2. `data_start == 0x40`（实测两个样本都成立）；
3. `dir_table_start + 32 == head_size`（两个样本成立）：目录表最后一项结束即头部结束；
4. 每个 `DARC_FILEHEAD` 满足 `data_addr + data_size ≤ 归档总长`。

**头部区域压缩**：`flags & NO_HEAD_PRESS == 0` 时，头部块（`0x40 .. head_size`）是**哈夫曼压缩**的。v8 读取路径（`DXArchive.cpp:1158-1189`）的顺序是「先 `KeyConv` 再 `Huffman_Decode` 再 `Decode`(LZ)」；头部块走同一套但有 `NO_HEAD_PRESS` 短路。**实现顺序：先做 `NO_HEAD_PRESS` 分支（不压缩则直接读表），再由 Probe 逐个归档打印 `flags` 确认本作哪些归档头部被压。** 已实测本作归档头 48 字节明文（`44 58 08 00`），但 NameTab 附近高熵 ⇒ **头部块确实压缩**，所以 `Huffman_Decode` 必须实现（不能省）。
**另有一条待确认项**：头部读取端**不得对偏移 0 应用 `KeyConv`** —— 用 `keycreate(NULL)` 或 Game.dat 的 `encryption_key` 对头做 XOR 会把它破坏成 `Head=0x11f2 / 0xec9c`。等价于 `DXA_FLAG_NO_KEY` 生效或密钥为全零。`KeyConv` 只作用于文件载荷（且 v8 源码把 `File->DataSize` 当 position 传入）。

### 4.3 PNG palette 解码

实测图像是 `.png` 与 `.jpg` 混合（`Picture` 539 文件、`MapChip` 用 `[Base]BaseChip_pipo.png` 等）。**必须显式处理 4 个真实存在的坑**：
1. **低位深（1/2/4bpp palette）**：autotile / 小图标可能是 4bpp indexed ⇒ 用 `Transformations::normalize_to_color8()`；
2. **`tRNS` 透明索引**：旧 PNG 用 `tRNS` chunk 表示透明调色板项，`EXPAND` 会转成 alpha 通道；
3. **无 alpha 的 JPEG**：`.jpg` 没有透明通道 ⇒ 合成时当不透明（原版对 `150 Picture` 的 jpg 也是不透明的）；
4. **颜色管理**：**禁用 `gAMA`/`iCCP` 校正**（像素游戏的颜色是作者逐像素定的，任何 gamma 校正都会偏色）。`png` crate 不做 gamma 处理，但 `image` crate 会 —— **所以只依赖 `png`，不要 `image`**（`image` 还会把 `jpeg`/`webp` 等一堆解码器拖进来，编译时间翻倍）。`.jpg` 用 `zune-jpeg`（纯 Rust、快、无 C 依赖）。

- 输出统一 `Rgba8UnormSrgb`（**不是 `Unorm`**）：纹理内容就是 sRGB 编码的像素，用 `Srgb` 让硬件在采样时转到线性空间，混合才正确。

```rust
pub fn decode_png(bytes: &[u8]) -> Result<DecodedImage, PngError> {
    let mut dec = png::Decoder::new(bytes);
    // EXPAND 处理 tRNS / palette -> RGBA8；STRIP_16 把 16bpp 降 8bpp
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf)?;
    buf.truncate(info.buffer_size());
    let (w, h, ct) = (info.width, info.height, reader.info().color_type);
    // 统一成 RGBA8：只有 RGB 时补 alpha=255
    if ct == png::ColorType::Rgb {
        let mut out = Vec::with_capacity(w as usize * h as usize * 4);
        for px in buf.chunks(3) { out.extend_from_slice(&[px[0], px[1], px[2], 255]); }
        return Ok(DecodedImage { w, h, rgba: out });
    }
    Ok(DecodedImage { w, h, rgba: buf })
}
```

### 4.4 图集打包

```rust
// wolf-render/src/atlas.rs
pub struct Atlas {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub packer: guillotine::Guillotine,     // shelf / guillotine 都是 O(n log n)
    pub entries: HashMap<AssetKey, UvRect>,
    pub padding: u32,                       // 2px：防 Nearest 采样在浮点边界渗色
}
```

**为什么用 guillotine 而不是 skyline**：贴图尺寸高度同构（16×16 tile、16×32 角色、32×32 图标，加少量 1024×576 全屏 CG）。**同构尺寸下 shelf 打包填充率已 ≥92%**，guillotine 在混合尺寸下更好且实现简单（几十行）。skyline 的优势在大量随机尺寸 —— 本项目不适用。

**为什么每张图集留 2px padding 且不做 mipmap**：`Nearest` 采样本身不跨 texel，但 `floor` 边界在浮点误差下可能采到相邻 texel（尤其 `150 Picture` 的 `Affine2` 旋转）。2px padding 消除该问题，代价 <4% 面积。**禁用 mipmap**：像素游戏要锐利。

**图集划分**：
- `atlas_tile`（2048×2048）：`MapChip.wolf` 全部 + autotile 4 象限展开；
- `atlas_chara`（2048×2048）：`CharaChip.wolf` + `FIcon.wolf`；
- `atlas_pic_0..7`（2048×2048 ×8 = 64MB）：`Picture` / `cg` / `hscene` / `cutin` 的 LRU 池。

### 4.5 懒加载与内存预算（1.25GB 媒体的流式策略）

```rust
// wolf-game/src/assets.rs
pub struct AssetServer {
    archives: Vec<wolf_format::wolf::Archive>,   // 文件句柄 + 头部索引，不整包读入
    index: HashMap<Box<str>, ArchiveSlot>,       // 路径 -> 归档 + 偏移（启动时建）
    cache: lru::LruCache<AssetKey, DecodedImage>,// 解压后的 CPU 侧
    atlases: [Atlas; 8],                         // GPU 侧 Picture 池
    pinned: HashSet<AssetKey>,                   // 正在显示的 Picture、当前地图 chip、正在播的 BGM
}
```

| 资源 | 加载时机 | 驻留策略 |
|---|---|---|
| `BasicData.wolf` / `MapData.wolf` 全部数据层 | 启动，**全部解析常驻** | 小（1.3MB + 144KB） |
| `MapChip` / `CharaChip` / `FIcon` | 启动，全部进图集 | 常驻 ~56MB |
| `SystemGraphic` / `SystemFile` | 启动，全部进图集 | 常驻 ~8MB |
| `Picture` 539 文件 | **被 `150` 引用时** | LRU 池 64MB |
| `cg.wolf` 118MB / `hscene.wolf` 20MB | 被引用时 | LRU 池（与 Picture 共用） |
| `BGM.wolf` 237MB | `181 BGM 再生` 时 | **不缓存**：symphonia 通过自定义 `MediaSource` 直接从归档流式解码（见 §5.3） |
| `SE.wolf` 21MB | 首次播放 | 按 SE 单位 LRU 缓存 32 条 |
| `BattleEffect` 109MB / `DLC_data` 148MB / `1.4.0` 32MB | 按需 | LRU / 流式 |

**为什么 BGM 不整轨解码**：`BGM.wolf` 237MB，若全解码成 PCM 是 **~2GB**（mp3/ogg 约 10:1）。必须流式。kira 0.12 的 `StreamingSoundData::from_media_source(impl MediaSource)` 正好允许我们喂一个「从 `.wolf` 偏移读字节」的自定义 source。v1 做简化版：**先按需解压整首曲子到内存 `Vec<u8>`，再用 `Cursor<Vec<u8>>` 适配 `MediaSource`**（单曲 mp3 通常 3-8MB，内存完全够；且 `MediaSource` 是 `Read + Seek`，`Cursor` 天然满足）。v2 再把 `.wolf` 的 LZ/Huffman 解压做成随机访问块。

**为什么 SE 走 `StaticSoundData`**：`179 効果音` 有并发且要求低延迟（<20ms）；`StaticSoundData::from_cursor(...)` 直接吃内存字节，无需落盘（`from_file` 会写临时文件或要求真实路径）。

**内存预算总结**：归档索引（4099 条路径 × ~80B）≈ 0.4MB；常驻图集 56MB + 系统 8MB + Picture 池 64MB = **128MB GPU**；CPU 侧 LRU 上限 **256MB**（可配）；BGM 流式缓冲 3 × 512KB ⇒ 峰值常驻 <400MB。

### 4.6 为什么「全量预热到磁盘」不可取

一个诱人的替代方案是「首次启动把 1.25GB 全部解包到 cache 目录，之后直接读文件」（这正是 `work/alldata` 已经做过的事）。**否决理由**：
1. 用户磁盘写入 1.25GB（且必须放用户可写目录），违背「资产一律运行时从用户自己的 Data/ 读」的约束；
2. `.wolf` 载荷是 LZ+Huffman 压缩的，预热只省 LZ 时间，**PNG 解码仍要花**（而 PNG 解码才是大头）；
3. 缓存与游戏版本会不同步（游戏更新后缓存陈旧），需要 hash 校验 —— 复杂度超过收益。

**保留的加速手段**：把「头部索引解析结果」缓存成一个小文件（~0.4MB bincode）到 `%APPDATA%`，用归档 `(path, size, mtime)` 做失效判定。这只把启动从 10ms 降到 2ms，属可选优化，v1 不做。

### 4.7 素材流式化与图集淘汰

> 本节回答一个具体问题：**当前「一张 2048² 图集 + 只增不减」的实现，会不会在长会话里失效？**
> 结论：**会**。而且这不是理论风险 —— 在游戏自己的脚本数据里，就有 **43 / 557 个事件**在峰值时需要的图片驻留量超过整个图集容量。

#### 4.7.1 外部旁证：另一个引擎的同类故障（独立来源）

社区里有一个由 HBOX 官方授权、用 RPG Maker MV 重写的 Android 版项目，它的 README 这样描述自家引擎的性能问题（下为中文转述）：

> 「游戏卡顿是 RPG Maker 引擎的通病……体现为玩一段时间就变卡，图片素材越多卡得越快，推测为该引擎无法及时清理图片缓存。运行内存越多，卡顿越轻。」
> 「彩绘图片体积过大是导致卡顿的一个主要原因。」

该团队同时把帧率锁在 20 帧、且完全没有启用显卡渲染，社区改版把这两点改掉后明显改善。

**为什么把这当作证据而不是类比**：它来自**另一个引擎、另一份代码、另一组开发者**，说的却是同一组因果：*图片缓存不释放 → 素材越多越慢 → 内存越大越轻*。它与我们下面（4.7.3）在**我们自己的游戏数据上**测出的数字互相印证，属于独立来源的旁证；只看别人的 README 当然不足以定方案，所以 4.7.2–4.7.3 全部是本地实测。

#### 4.7.2 我们当前实现的真实行为（读代码得出，不是设想）

| # | 事实 | 位置 |
|---|---|---|
| 1 | 图集是**单张** 2048×2048 RGBA8 = **16 MiB** 上限 | `crates/wolf-game/src/atlas.rs:25` |
| 2 | shelf packer **只增不减**：`add` 只追加 shelf，没有任何删除/回收路径 | `atlas.rs:195-228` |
| 3 | 同名去重（第二次返回首次的矩形），但这也意味着**一旦打包永不释放** | `atlas.rs:188-190` |
| 4 | 任一维 >2048、或装不下 ⇒ `return None`；调用方 `let Some(rect) = … else { continue; }` **静默跳过**，无日志无计数 | `atlas.rs:192-194`、`atlas.rs:304-306` |
| 5 | `Assets` 缓存是 `HashMap<String, Arc<ImageData>>`，**无上限、无 LRU**；`clear()` 有实现但运行时无人调用 | `crates/wolf-game/src/assets.rs:164-201` |
| 6 | 每打包一张新图，就把**整个 16 MiB** 全量 `write_texture` 一次（判据是 `atlas.len()` 变化），上传成本与图片大小无关 | `crates/wolf-cli/src/play.rs:179-189` |
| 7 | 解码发生在**每帧的渲染重建路径**里（`Atlas::sprites` → `add_asset` → `Assets::load`），单帧内可能同步读盘 + 解码一张 70 MiB 的图 | `atlas.rs:231-237` |

⇒ 与 4.7.1 描述的故障模式**逐条同构**：图片缓存不清理（3、5）、大图多（4）、单帧解码（7）。第 6 条还额外放大了它 —— 新素材出现的瞬间付出的不是「这张图的成本」，而是「整张图集的成本」。

#### 4.7.3 量化：全量 3,680 张图与单场景并发（实测）

统计脚本（只读）：`work/_atlas_stats.py`（尺寸/字节）、`work/_picrefs.py`（`150` 引用的素材集）、`work/_piclive.py`（单事件内 picture slot 的存活模拟）。数据源 `work/alldata/**` 全量 3,680 张 png/jpg。

**（a）按归档的尺寸与解码量**

| 归档 | 文件数 | 磁盘 | 解码 RGBA8 | 中位尺寸 | 最大尺寸 | >2048 的图 |
|---|---|---|---|---|---|---|
| `cg` | 596 | 118.5 MB | **1,467.9 MiB** | 1024×576 | 4608×6500 | 14 |
| `DLC_data` | 623 | 95.4 MB | **1,358.6 MiB** | 1024×576 | 9216×3500 | 55 |
| `BattleEffect` | 92 | 108.4 MB | 751.5 MiB | 1080×480 | 4800×11520 | 30 |
| `hscene` | 327 | 21.8 MB | 621.0 MiB | 540×540 | 4860×576 | 17 |
| `scene`（单色版，同尺寸） | 327 | 0.6 MB | 621.0 MiB | 540×540 | 4860×576 | 17 |
| `Picture` | 539 | 25.8 MB | 474.2 MiB | 200×240 | 4499×1300 | 20 |
| `1.4.0` | 199 | 32.1 MB | 378.8 MiB | 1024×576 | 3072×576 | 4 |
| `cutin` | 223 | 6.8 MB | 34.4 MiB | 240×160 | 1680×160 | 0 |
| 其余 12 个归档（系统图 / chip / 角色 / UI / 字） | 754 | 8.0 MB | 59.2 MiB | ≤1024×576 | 1920×3984 | 1 |
| **合计** | **3,680** | **417.5 MB** | **5,766.6 MiB** | 1.1 MiB/张 | — | **158** |

关键结论：

1. **磁盘 417 MB → 解码 5.63 GiB，放大 13.8 倍。** 「素材体积过大」在这里不是 118 MB 的问题，是 **5.6 GiB** 的问题 —— 压缩率越好，解码爆炸越狠。
2. **16 MiB 的图集只装得下全库的 0.28%；全库需要 360 张图集。**
3. **158 张图任一维 >2048**（最大 `BattleEffect/pipo-btleffect207_480.png` 4800×3840 = **70.3 MiB**）。按 4.7.2 的第 4 条事实，**这些图今天连显示都做不到**，被静默吞掉。
4. **1,336 张 ≥1024×576，占解码量的 81%（4,653 MiB）** —— 只要照顾这一批，内存问题就解决了 8 成。
5. **1,423 张 ≤256×256，合计只有 79.5 MiB（磁盘 21.8 MB）** —— 这才是图集的**天然客户**：chip、角色、图标、UI。它们全部常驻也只要约 5–6 张 2048² 页（含 shelf 装箱损耗）。

**（b）游戏实际会用到的素材集**（从 550 个公共事件的 163,828 条指令槽里的 `150 Picture` 反查）

- `150` 的字符串参数共出现 **2,842** 个不同名字，其中 **2,198** 个能在磁盘上解析成文件（其余 644 个是**文字图片**，标识符以 `<` 或反斜杠开头，不是文件）；
- 这 2,198 个文件的解码总量 = **4,051 MiB**（「玩家理论上可能看到」的素材就有 4 GB 解码量）；
- 其中 **115 张 >2048** ⇒ 今天**静默不画**。这不是内存策略问题，是**正确性缺口**。

**（c）单场景并发（按事件内 picture slot 的存活模拟，557 个事件/地图事件全跑）**

| 指标 | 值 |
|---|---|
| 单事件内同时存活的 picture slot 数 | 最大 **181**，p99 **24**，p90 **5**，中位 0 |
| 峰值驻留解码量 | 最大 **108.9 MiB**（事件 `443 DLC追加差分表示`，24 槽），p90 **11.2 MiB** |
| **峰值超过 16 MiB 图集的事件** | **43 / 557（7.7%）** |
| 画 ≥20 张同屏的事件 | 10 个 |
| 单事件触达的不同图片文件数 | 最多 **117**（`335 深夜差分描画`，解码 247 MiB）、**112**（`333 立ち絵表示`，解码 276 MiB） |

再叠加前文 §0 的事实：**108 个事件是每帧并行跑的后台协程**。上面每个数字都只是「单个事件独占时」的下界，真实峰值还要叠加同时活跃的多个事件。

#### 4.7.4 三个候选方案与取舍

| | A. LRU 淘汰 + 图集重打包 | B. 多页图集（bindless / 多 bind group） | C. 按需解码 + 直接纹理（每图一张 texture） |
|---|---|---|---|
| 内存上界 | 16 MiB（不变） | 页数 × 16 MiB | 存活图之和（可用 LRU 卡死） |
| 命中失败的代价 | **全量重建**：所有 UV 失效 → 本帧所有 quad 重算 → 16 MiB 重打包 + 16 MiB 重上传 | 单页内重打包（UV 仍失效，只波及该页） | 只多创建一张 texture + 一个 bind group |
| 能否容纳 >2048 的图 | **不能**（158 张永远进不去） | 不能（单页仍是 2048） | **能**（`max_texture_dimension_2d` = 32768） |
| draw call | 1 | = 页数（2–7） | = 当前屏上图片数（实测峰值 24，p99 24，中位 5） |
| 对渲染器的改动 | 无 | 大：多纹理 + 多 bind group + 按页批处理 | 中：每图 bind group，pass 内切换 |
| 与「43/557 事件超 16 MiB」的匹配度 | **差**：7.7% 的事件会持续抖动 | 中：2 页不够，需 7 页 = 112 MiB 且仍要淘汰 | **好**：容量随需求线性给，峰值 109 MiB 可接受 |

**为什么 A 被否**：A 的根本问题不是「淘汰」，而是「回收后 UV 全失效」。atlas 矩形被烘进每帧重建的 `SpriteInstance.uv`，一旦重打包，本帧所有 quad 都要重算，并且必须重传整张 16 MiB。而实测 **7.7% 的事件峰值就超过 16 MiB**，在这些场景里 A 会退化成「每帧全量重打包 + 全量上传」—— 比不淘汰更慢。A 只在「工作集 ≪ 图集容量」时才是对的，而我们的工作集是 4,051 MiB。

**为什么 B 不算错但排第二**：分页是标准解，能容纳 24 槽 × 单图几 MiB 的场景（需 7 页 = 112 MiB），代价是渲染器要多 bind group、批处理按页切分，并且 **>2048 的 158 张图仍然无解**（单页仍是 2048）。它解决的是「容量」，没解决「大图」。

**为什么推荐 C**：把 4.7.3 的数字按尺寸切开，问题就消失了 ——

- **≤256×256 的 1,423 张图，合计 79.5 MiB**：全部静态常驻图集（约 5–6 页，永不淘汰，零抖动）；
- **其余 2,257 张（5,687 MiB）**：一律直接纹理 + LRU。它们本来就是大图，一次只显示几张，`max_texture_dimension_2d = 32768` 完全够（我们最大的 9216×3500 也在限内）；
- **同屏 draw call**：实测峰值 24 槽、p99 24、中位 5。RTX 5060 上 24 次 draw call 的 CPU 成本远低于 16 MiB 上传 —— 这是「用几十个 draw call 换掉整张图集重传」的经典划算交易。

#### 4.7.5 推荐方案与触发阈值

**混合方案（M3 落地）**：

1. **静态小图图集**：`w ≤ 256 && h ≤ 256` ⇒ 进图集，启动后只增不减（实测总共 79.5 MiB / 约 5–6 页）。这些是 chip / 角色 / 图标 / UI，**永不淘汰，无抖动**。
2. **直接纹理旁路**：其余任何图 ⇒ 一张 `wgpu::Texture` + 一个 bind group，进 GPU 侧 LRU。
3. **阈值（可直接写成断言）**：
   - 走旁路的判据：`w > 256 || h > 256`（`>2048` 的情形自动被包含）；
   - 图集兜底：`atlas.fill_ratio() > 0.60` 或新图放不下 ⇒ **转旁路**，**绝不重打包**（重打包会让已烘好的 UV 全部失效）；
   - GPU 侧直接纹理 LRU 上限 **256 MiB**（≈250 张典型 1024×576，或 3 张最大 BattleEffect）；CPU 侧 `Assets` LRU 上限 **256 MiB**；
   - 单帧解码预算：**每帧最多解码 1 张大图**（>4 MiB），其余顺延到下一帧 —— 避免 4.7.2 第 7 条那种「一帧解 70 MiB」。

**为什么阈值取 256 而不是 512**：≤256 的集合是 79.5 MiB（约 5–6 页，可全常驻）；放宽到 512 会吃掉 2,257 张里的一大半，图集立刻需要淘汰，等于把方案 A 的抖动请回来。256 正好是「能全装下」与「装不下」的分界。

#### 4.7.6 还需要哪些指标才能把阈值定死

| 缺的指标 | 怎么取 | 用来定什么 |
|---|---|---|
| 单帧 live slot 数 / 单帧新图数 / 单帧解码耗时 | `play` 埋点（现有 `stats.frames` / `instructions`，缺 picture 侧），每 60 帧打一行 | 验证静态模拟的 24 槽峰值；定单帧解码预算 |
| 长会话翻页速率 | 跑一次 cg 画廊（596 张）与 H 场景，记录 `Assets::len` 与 `atlas.fill_ratio()` 的曲线 | 定 LRU 上限与预取窗口 |
| H 场景真实并发 | **未测**（标题终态只有 2 个 quad）。定点跑事件 `443` / `421` / `480` 并用 `capture` 数 quad | 已知静态峰值 108.9 MiB / 24 槽，需动态确认 |
| 直接纹理 vs 图集的真实绘制成本 | 同一画面两种路径各跑 1000 帧，比 CPU 帧时间与 `capture` 耗时 | 若 24 draw call 成本意外高，才回头考虑方案 B |

#### 4.7.7 结论：M2/M3 必须做 / 可以先欠着

**M2 必做（半天量级，零渲染器改动，只动 `wolf-game` + `wolf-cli`）**

1. **大图不再静默丢弃**：`Atlas` 增加 `skipped` 计数（名字 + 尺寸），`play` / `capture` 结束时打印。这是**正确性**问题 —— 今天有 115 张被 `150` 引用的图根本不显示。
2. **`Assets` 加 LRU 上界（256 MiB），并把 `clear()` 接进场景切换**（目前它是个死方法）。
3. **`play` 每帧输出 `atlas fill%` / live picture slots / 本帧解码数**，作为 4.7.6 三个指标的取数口。

**M3 必做**：直接纹理旁路 + 渲染器多 bind group（4.7.5 的混合方案）。它是唯一同时解决「>2048 的 158 张」和「109 MiB 峰值」的路径。

**可以先欠着**

- **方案 A（LRU + 图集重打包）**：与 M3 的 C 路径重复投资，且 7.7% 的事件会抖动；**不做**。
- **方案 B（多页图集 / bindless）**：只有在 M3 之后实测 draw call 成为瓶颈时才回头做；现在做等于先优化一个还没被测到的成本。
- **纹理压缩（BCn / ASTC）**：能压 4–6 倍，但像素艺术 + `Nearest` + 调色板透明度会掉画质，且与「逐帧对比」的验收标准冲突；不做。

---

## 5. 音频

### 5.1 为什么选 kira 0.12.5

| 候选 | 结论 |
|---|---|
| **kira 0.12.5**（选它） | 内建 `Track`/`Mixer`/`Tween`/`Modulator`/`Clock`；`loop_region` 直接支持循环点；`MediaSource` 可自定义；解码走 symphonia，mp3/ogg/wav 全覆盖 |
| rodio | 更简单，但循环点支持弱（只有 `repeat_infinite` 无区间）、无 `MediaSource` 级自定义、BGM 与 SE 的分组/淡出要自己写 |
| symphonia 直接用 | 只到「解码出 PCM」，混音/声道/淡出/循环全部自写 —— 等于把 kira 重写一遍 |
| fyrox-sound / oddio | 面向 3D 空间音频，2D 游戏用不上，生态小 |

**本作的实际音频构成**：只有 **mp3 / ogg / wav 三种扩展名，0 个 MIDI**（条数待复核，见 §0.1）。
⇒ 依赖 feature 只需 `features = ["mp3", "ogg", "wav"]`（kira 的 Cargo.toml 里 `mp3 = ["symphonia", "symphonia/mp3"]`，依此类推）。

### 5.2 为什么不需要 rustysynth

**决定性理由：游戏里根本没有 MIDI 文件。** 上千个音频文件全是 mp3/ogg/wav 采样格式，`181 BGM 再生` / `179 効果音` 拿到的就是这些文件。rustysynth 的作用是「用 SoundFont 合成 MIDI」，没有 MIDI 就没有输入源。
**保留条件**：若未来发现某些 `.mid` 被间接引用（例如 `SystemFile` 下的隐藏文件），再引入 rustysynth + 一个 SF2。当前只需在 `ArchiveIndex` 里统计扩展名分布来断言这一点（**已统计：0 个 MIDI**）。

### 5.3 引擎结构

```rust
// wolf-audio/src/lib.rs
use kira::{
    AudioManager, AudioManagerSettings, DefaultBackend,
    sound::static_sound::{StaticSoundData, StaticSoundSettings},
    sound::streaming::StreamingSoundData,
    track::TrackBuilder,
    tween::Tween,
    Decibels,
};

pub struct AudioEngine {
    manager: AudioManager<DefaultBackend>,
    pub bgm:   kira::track::TrackHandle,   // BGM 轨
    pub bgs:   kira::track::TrackHandle,   // 环境音轨
    pub se:    kira::track::TrackHandle,   // SE 轨（多条并发）
    pub voice: kira::track::TrackHandle,   // 语音轨
}

impl AudioEngine {
    pub fn new() -> Result<Self, kira::StartError> {
        let mut manager = AudioManager::new(AudioManagerSettings::default())?;
        // 每条轨各一个 TrackBuilder：SE 需要并发容量
        Ok(AudioEngine {
            bgm:   manager.add_sub_track(TrackBuilder::new())?,
            bgs:   manager.add_sub_track(TrackBuilder::new())?,
            se:    manager.add_sub_track(TrackBuilder::new())?,
            voice: manager.add_sub_track(TrackBuilder::new())?,
            manager,
        })
    }
}
```

**为什么 BGM 与 BGS 必须是两条 Track**：WOLF 的指令集把「BGM 停止」与「BGS 停止」分开（环境音如雨声/风声与主题曲互不干扰），音量也分别可调。放进同一条轨会导致停 BGM 时把雨声一起停掉。

### 5.4 BGM 循环点

kira 的循环用 `StaticSoundSettings::loop_region` / `StreamingSoundData` 的 `settings.loop_region`：

```rust
use kira::sound::{Region, PlaybackPosition, EndPosition};

/// WOLF 的 BGM 循环有两种来源：
/// (a) 指令显式给的循环点（181 的参数里可选 loop 起止）
/// (b) 文件本身的内嵌循环点（ogg 的 LOOPSTART/LOOPEND 注释，mp3 通常没有）
pub fn bgm_settings(loop_from: Option<(f64, f64)>) -> StaticSoundSettings {
    let mut s = StaticSoundSettings::default();
    if let Some((start, end)) = loop_from {
        // Region 实现了 From<Range<T>>，也可写区间字面量
        s = s.loop_region(Region {
            start: PlaybackPosition::Seconds(start),
            end:   EndPosition::Custom(PlaybackPosition::Seconds(end)),
        });
    }
    s
}
```

**为什么 ogg 的循环点用 `PlaybackPosition::Samples` 而不是 `Seconds`**：ogg 的 `LOOPSTART` 本来就是**采样数**，直接透传无浮点误差；而指令里给的循环点通常是秒（或毫秒）⇒ 用 `Seconds` 保真原值。两条路径各自用自己的单位，只在需要统一时换算（`samples = seconds * sample_rate`）。

**已知待确认**：WOLF 的 BGM 循环点到底来自指令参数还是文件内嵌元数据。实现期任务：解析 `181` 的参数结构 + 检查 `BGM.wolf` 里 ogg 的 Vorbis comment。**在此之前 v1 采用「指令参数优先，缺失则整曲循环」**。

### 5.5 SE 并发与句柄管理

```rust
pub struct SeHandle { pub id: kira::sound::static_sound::StaticSoundHandle, pub slot: u8 }

impl AudioEngine {
    /// 179 効果音：同一 SE 重复触发默认重头播放（新句柄），不打断旧句柄
    pub fn play_se(&mut self, data: StaticSoundData, vol: f32, pan: f32) -> Option<SeHandle> {
        let settings = StaticSoundSettings::default()
            .volume(pct_to_db(vol))
            .panning(pan)
            .start_time(kira::StartTime::Immediate);
        match self.se.play(data.with_settings(settings)) {
            Ok(h) => Some(SeHandle { id: h, slot: 0 }),
            Err(_) => None,     // 轨道容量满：静默丢弃，不 panic（原版也会丢弃）
        }
    }
    pub fn se_finished(&self, h: &SeHandle) -> bool {
        h.id.state() == kira::sound::PlaybackState::Stopped
    }
}
```

**为什么 SE 容量满时静默丢弃而不是扩容**：`AudioManagerSettings.capacities` 决定每种资源的并发上限；无限扩容会让「每帧触发 N 个 SE 的恶性循环」把内存吃光。原版 DxLib 的 SE 缓冲槽也是有限的（超了就不播）。**保留一个 `overflow_count` 计数器写进调试 HUD**，方便发现逻辑 bug。

### 5.6 与 VM 的接口（指令 → 音频）

| 指令 | 行为 |
|---|---|
| `181 BGM 再生` | `bgm` 轨停旧 + 播新；参数含音量、循环点、淡入时长（用 `Tween`） |
| `182 BGM 停止` | `bgm` 轨 `stop(Tween { .. })` 淡出 |
| `183 BGS 再生 / `184 BGS 停止` | 同 `bgs` 轨 |
| `179 効果音` | `se` 轨新句柄；参数含音量、声像（左右）。**实测存在负数参数（`-240 / -256`）出现在停止/操作位** ⇒ 需按操作码区分「播放」与「停止」 |
| `185 音频淡出 / `186 音量设置` | 用 `Tween` 改 `TrackHandle` 音量，不重建声音 |

**为什么音量换算要单独包一个函数**：WOLF 的音量参数是 0..100 百分比，原版 DxLib 内部把它当线性增益；而 kira 用 `Decibels`。把这个换算隔离成 `fn pct_to_db(pct: f32) -> Decibels` 并加单元测试锁数值，避免「音量偏小/偏大」这种难查的手感回归。

### 5.7 音频与逻辑帧解耦

音频**不**走 `advance` 的固定步长：kira 有自己的内部时钟（`internal_buffer_size` 默认 128 采样）与后端线程。VM 只在指令到达时发命令（播放/停止/改音量），不每帧 spin。**理由**：音频抖动必须靠后端环形缓冲吸收，而不是靠逻辑帧率；且 `180 Wait` 的等待是逻辑帧，与音频进度无关（原版亦然）。

---

## 6. 存档层

### 6.1 目标：与官方存档双向兼容

验收标准（硬指标）：
1. **读**：Rust 能解开官方 `SaveData/SaveDataN.sav` 并 dump 出与 WolfSave 的 `JsonDumper` 一致的字段；
2. **写**：Rust 重新加密后与官方存档**逐字节相同**（§8 的 Save-Diff Oracle）；
3. **互操作**：Rust 写出的存档能被原版游戏加载并继续（最终验收，需用户手动跑一次原版）。

WolfSave 的 C++ 源码在 `docs/vendor/WolfSave/`（MIT，5,895 行），是本层唯一的权威对照。

### 6.2 解密/加密（已破解并独立验证）

```rust
// wolf-save/src/crypt.rs
/// WolfSave 的算法：seeds = [b[0], b[3], b[9]]，MSVC rand，从 0x14 起按步长 1/2/5 异或
pub struct SaveCrypt {
    seeds: [u32; 3],
    state: [u32; 3],
}

impl SaveCrypt {
    /// 头部 0x14 字节是明文（含 seed 字节与校验），无密钥
    pub fn from_header(buf: &[u8]) -> Self {
        let seeds = [buf[0] as u32, buf[3] as u32, buf[9] as u32];
        SaveCrypt { seeds, state: seeds }
    }

    /// 每个 byte 用 (rand() >> 12) & 0xFF 异或；三条流分别步长 1/2/5
    pub fn xor_stream(&mut self, buf: &mut [u8], reverse: bool) {
        let mut advance = [1usize, 2, 5];
        if reverse { /* 反向时 seq 需按 WolfSave 的逆序规则重放 */ }
        for (i, b) in buf.iter_mut().enumerate() {
            let k = i % 3;
            for _ in 0..advance[k] { self.next_msvc_rand(k); }
            let v = (self.state[k] >> 12) & 0xFF;
            *b ^= v as u8;
        }
    }

    /// MSVC rand: state = state * 214013 + 2531011
    #[inline] fn next_msvc_rand(&mut self, k: usize) {
        self.state[k] = self.state[k].wrapping_mul(214_013).wrapping_add(2_531_011);
    }
}
```

**已验证的不变量（实现期必须在 `decrypt()` 末尾断言）**：
- 解密后 `buf[0x14] == 0x19`；
- 解密后**最后一个字节 == 0x19**；
- `buf[2] == sum(decrypted[0x14..]) & 0xFF`（校验和）。

这三条把「解密成功」与「解出错位的垃圾」区分开 —— 没有它们，一条错位的流会产生看似合法的变量值，污染后续全部测试。

### 6.3 布局

```
偏移 0x00  明文头（0x14 字节，含 seeds 与 checksum）
0x14       u8  0x19（魔数，解密后可见）
0x15       u16 name_len + name_len 字节（UTF-8 标题 / 玩家名）
   …       u16 file_version = 0x8C (140)
0x44       正文开始（SavePart1..SavePartN 顺序排布）
末尾       0x19 结束标记
```

### 6.4 保真模型（决定性设计）

**绝不做「只解析我们认识的字段 + 重写」。** 官方存档字段数以千计（WolfSave 用 5,895 行 C++ 覆盖 `SavePart1..7`），任何选择性重写都会丢失未知字段 ⇒ 破坏兼容。因此采用**保真三明治**：

```rust
// wolf-save/src/document.rs
/// 保真模型：只有被显式修改的区间才重编码，其余字节原样保留
pub struct SaveDocument {
    /// 0x00..0x14 原样保留（加密时校验和会重算）
    header_raw: [u8; 0x14],
    /// 0x14..0x44 的元数据段：已解析（魔数、名字、版本）
    meta: SaveMeta,
    /// 0x44..末尾-1 的正文：分段为 (Tag, RawRange)，Tag 已知的才解析
    parts: Vec<SavePart>,
    trailer: u8,                     // 0x19
}

pub struct SavePart {
    pub tag: PartTag,
    pub raw: Vec<u8>,                    // 未修改时原样保留
    pub decoded: Option<Box<dyn Part>>,  // 需要读写时才解析
}

pub enum PartTag { Part1, Part2, Part3, Part4, Part5, Part6, Part7, Database, VarDb, Unknown(u32) }
```

**为什么正文要「分段 + 原样保留」**：v1 只需实现「变量区（`VariableDatabase`，329 行 C++）+ 玩家名/标题 + 校验和重算」就能写出**官方可加载**的存档（游戏最关心的是变量），其余部分在尚未实现的地图状态/事件实例状态上保持原样。随着 `Part2`（地图与事件状态，942 行）等被逐步实现，把 `decoded` 填上，`raw` 退化为 fallback。

**分段的定位方式**：每个 `SavePartN` 的长度必须能从自身读出（长度前缀或固定结构），否则就无法在 v1 跳过它。**实现期任务**：为每个 Part 找到「自描述长度」或「可安全跳过的标记」；若某个 Part 没有自描述长度，就必须先完整实现它才能跳过 —— 这是本层最大的工作量风险，**先做 Probe 把 7 个 Part 的边界在真实存档上标定出来再动手**。

**变量区是最先要做的**：§8.2 的差分测试需要「官方存档里的变量值」作为 oracle。`VariableDatabase` 存 `0..631` 的普通变量、字符串变量、系统变量。

### 6.5 我们自己的存档格式（native，仅 Rust 用）

额外支持一个 native 存档（`.rsav`，serde + bincode）用于开发调试：它序列化 §2.3 的 `EventInstance`/`CallFrame`/`VarStore` 全集（能保存「所有 108 个并行协程的精确位置」），这是官方格式**做不到**的（官方保存的是逻辑状态，不是协程续体）。

| 格式 | 读写方 | 用途 |
|---|---|---|
| `SaveData*.sav` | 双方（双向兼容） | 玩家真实存档、跨版本 |
| `*.rsav` | 仅 Rust | 开发调试、逐帧差分、崩溃恢复 |

**为什么这是自研格式而不是扩展官方格式**：`EventInstance` 含 `pc` + `frames: Vec<CallFrame>`，而官方格式里对应的概念只是「事件是否正在运行 + 局部变量」，**没有「指令指针」**（原版每帧重新解释 + 用「事件是否活跃」表达状态）。把 `pc` 塞进官方格式会被原版忽略 ⇒ 原版加载后会从事件开头重跑，行为不同。所以两套并存、互不干扰。

### 6.6 存档时机

WOLF 的存档由公共事件驱动。Rust 侧把它实现为一条**指令的副作用**（`SideEffect::RequestSave { slot }`），由 `wolf-game` 的 `SaveManager` 在**帧边界**执行。**为什么不能在 VM 执行中途写盘**：变量与实例图可能处于不一致的中间态（例如一次 `121 SetVariable` 序列写到一半）。原版同样在指令之间写盘。

---

## 7. 工程化

### 7.1 装工具链（CN 镜像，Windows MSVC）

**目标平台：`x86_64-pc-windows-msvc`**（不是 `-gnu`）。**为什么必须 MSVC**：wgpu 的 DX12 后端与 `windows` crate 都依赖 MSVC 运行时；`-gnu` 下链接 `vcruntime` 与 `d3d12` 反复出问题，且 `wgpu` 官方 CI 只测 MSVC。

PowerShell 里执行（`$env:` 语法，不要用 `export`）：

```powershell
# 1) 先设镜像环境变量，再下 rustup-init.exe
$env:RUSTUP_DIST_SERVER  = "https://mirrors.ustc.edu.cn/rust-static"
$env:RUSTUP_UPDATE_ROOT  = "https://mirrors.ustc.edu.cn/rust-static/rustup"
Invoke-WebRequest -Uri https://mirrors.ustc.edu.cn/rust-static/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe -OutFile "$env:USERPROFILE\rustup-init.exe"
& "$env:USERPROFILE\rustup-init.exe" -y --default-toolchain stable-x86_64-pc-windows-msvc --profile default

# 2) 验证（必须看到 msvc，不是 gnu）
rustup show
rustc -vV          # host: x86_64-pc-windows-msvc
```

**注意 `--default-host`**：若机器上已装过 gnu 工具链，`rustup-init` 可能沿用旧默认。用 `--default-host x86_64-pc-windows-msvc` 显式覆盖更稳。

### 7.2 `~/.cargo/config.toml`（CN 源替换）

Windows 路径是 `%USERPROFILE%\.cargo\config.toml`：

```toml
# 替换 crates.io 为 rsproxy（字节跳动，国内最快且稳定；USTC 也可）
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy]
registry = "sparse+https://rsproxy.cn/index/"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[registries.rsproxy]
index = "sparse+https://rsproxy.cn/index/"

[net]
git-fetch-with-cli = true
retry = 3

[build]
# 让 16 核机器吃满；增量编译保留（见 7.4）
jobs = 16
```

```toml
# 若 rsproxy 不可用，换 USTC：
[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
[source.crates-io]
replace-with = "ustc"
```

**为什么用 `sparse+` 而不是 git index**：git index 要 clone 整个 crates 索引（数百 MB），sparse 只按需拉单个 crate 的元数据（KB 级）。首次 `cargo build` 的依赖拉取从 10+ 分钟降到 1-2 分钟。

### 7.3 Cargo workspace 完整依赖清单（精确版本，全部已核实）

根 `Cargo.toml`：

```toml
[workspace]
resolver = "2"
members = ["crates/wolf-format", "crates/wolf-vm", "crates/wolf-save",
           "crates/wolf-render", "crates/wolf-audio", "crates/wolf-game", "crates/wolf-cli"]

[workspace.package]
edition = "2021"          # 不用 2024：ecosystem 生态尚未全面适配
rust-version = "1.87"     # = wgpu 30 的 MSRV（源码 lib.rs 明确写了 1.87）
license = "MIT OR Apache-2.0"

[workspace.dependencies]
# ---- 数据层 ----
binrw        = { version = "0.15.2", features = ["std"] }
encoding_rs  = "0.8"                     # SJIS(932) <-> UTF-8
serde        = { version = "1", features = ["derive"] }
serde_json   = "1"
bincode      = "1.3"

# ---- 解释器 ----
glam         = { version = "0.34.1", features = ["bytemuck"] }
thiserror    = "2"
log          = "0.4"
env_logger   = "0.11"
indexmap     = "2"

# ---- 渲染 ----
wgpu         = "30.0.1"
winit        = "0.30.13"
glyphon      = "0.12.0"
cosmic-text  = "0.19.0"
bytemuck     = { version = "1.25.2", features = ["derive"] }
pollster     = "1.0.1"
unicode-linebreak = "0.1.5"              # cosmic-text 换行所需（即便不直接调用也要锁版本）

# ---- 资源 ----
png          = "0.18"
zune-jpeg    = "0.4"
crc32fast    = "1.5"

# ---- 音频 ----
kira         = { version = "0.12.5", features = ["mp3", "ogg", "wav"] }
# 注：kira 的 symphonia 依赖由上面的 feature 自动带入，不要手写 symphonia 版本

# ---- CLI ----
clap         = { version = "4", features = ["derive"] }

[profile.release]
opt-level = 3
lto = "thin"            # thin 而不是 fat：编译时间 1/3，性能损失 <3%
codegen-units = 1       # 单一 CGU 才能让 LTO 充分发挥；release 构建慢一点无所谓
panic = "abort"         # 无 unwinding 开销；VM 不应靠 panic 恢复
strip = "debuginfo"

[profile.dev]
opt-level = 1           # 让 debug 构建也跑得动（VM 是热点）
incremental = true

[profile.dev.package."*"]
opt-level = 3           # 依赖（wgpu/naga/cosmic-text）用 O3，自己的代码 O1
```

**为什么 `edition = "2021"` 而不是 `2024`**：`2024` 要求 rustc ≥ 1.85（满足），但部分依赖的宏在 `2024` 下有 lint 变化；本项目无 `gen` 关键字冲突等 `2024` 收益，保持 2021 降低变量。

**为什么 `[profile.dev.package."*"] opt-level = 3`**：这是**必做项**而不是优化 —— wgpu 的 naga（shader 编译）与 cosmic-text 的排版在 `opt-level = 0` 下慢到无法调试（首帧可能要几秒）。自己的 crate 保持 `opt-level = 1` 保证断点单步仍然可用（O0 才逐行精确，但 VM 循环用 O0 会卡；折中在 §7.4 说明）。

### 7.4 Windows MSVC 构建注意事项

| 项 | 建议 | 为什么 |
|---|---|---|
| Linker | 用 `lld-link`：`[target.x86_64-pc-windows-msvc] linker = "lld-link"`（rustup 组件 `cargo install -Z` 不需要，`rustup component add llvm-tools` 自带） | 默认 `link.exe` 在 wgpu 这种大 crate 上链接要 20-40s；`lld-link` 降到 3-5s。**但注意**：`lld-link` 对 `/DEBUG`/PDB 的兼容偶有问题，若调试 PDB 异常就换回 `link.exe` |
| 增量编译 | `incremental = true`（dev），`false` 用于 release | wgpu 全量重建约 3-5 分钟；增量后改一行 VM 代码编译 ~10s |
| `codegen-units = 1` | 仅 release | dev 下 `codegen-units = 256` 才能有增量。**dev 若设 1，增量编译完全失效** |
| LTO | release `thin` | `fat` 会把构建时间拉到 5-10 分钟，而我们没有跨 crate 内联的极致需求（VM 的热点在单 crate 内） |
| 长路径 | 开启 `LongPathsEnabled`（注册表）或把 `CARGO_TARGET_DIR` 设到 `E:\cargo-target` | `E:\Study\github program\imoto life\` 路径已 30+ 字符，加上 `target\release\deps\...` 容易破 260 |
| 杀软 | 把 `target/` 与 `CARGO_TARGET_DIR` 加进 Defender 排除 | Defender 实时扫描会把链接阶段的 I/O 拖慢数倍 |
| `VSCode` + `rust-analyzer` | `rust-analyzer.cargo.features` 打开 `--all-features`，`checkOnSave` 用 `check` 而非 `clippy` | `clippy` 在 wgpu 依赖上会跑很久 |
| 内存 | 16GB 足够，但 `codegen-units = 1 + lto = fat` 会峰值 8GB+ | 想并行跑 rust-analyzer 时保持 thin |

```toml
# .cargo/config.toml 追加（linker 加速）
[target.x86_64-pc-windows-msvc]
linker = "rust-lld.exe"
# 若 PDB 异常，删掉上面这行（回退 MSVC link.exe）
```

### 7.5 调试策略（VM 是热点，O1 不够精确时怎么办）

```rust
// 在需要精确单步的少数函数上加属性，而不是全局降 O0
#[cfg_attr(debug_assertions, inline(never))]
fn step_one(inst: &mut EventInstance, prog: &Program, vars: &mut VarStore, ctx: &mut Ctx) { /* … */ }
```

配套一个 `F3` 调试 HUD（渲染在 `Band::DebugOverlay`）：显示 FPS、当前 map/event/page、活跃实例数、`pc` 与 `frames.len()`、`overflow_count`（SE 丢弃数）、`per_instance_budget` 触发次数。**这些计数器是 §8 里定位「VM 卡住」的第一手证据**。

### 7.6 CI（GitHub Actions 或本地脚本）

```yaml
# 最小可用流水线（Windows runner）
- cargo fmt --check
- cargo clippy --workspace --all-targets -- -D warnings
- cargo test --workspace
- cargo tree -d          # ★ 断言无重复 wgpu / winit / kira（§3.1 的类型冲突防线）
- cargo run -p wolf-cli -- wolf-verify --data-dir "%GAME_DIR%/Data"   # 数据层自检
```

**`cargo tree -d` 是必需的**：它是唯一能在编译前抓住「两个 wgpu 版本」的自动检查。

---

## 8. 测试策略（没有官方运行时可对照时怎么保证正确性）

**核心困境**：没有开源 WOLF 运行时可以逐行为对照，唯一能跑的是闭源收费的浏览器版。因此不能靠「跑起来一样」验收，必须**建立多个独立的 oracle**，每一个覆盖不同的失败模式。

### 8.1 五层测试

| 层 | Oracle | 覆盖的失败模式 | 成本 |
|---|---|---|---|
| L1 单元 | 手写期望值 | 解码边界（`MagicNum`、bd 配对、tRNG） | 低 |
| L2 差分（数据层） | **WolfTL 的 JSON dump** | 结构解析错位（偏移错 1 字节就会全线崩） | 低（已有产物） |
| L3 交叉（数据层） | **WolfTL + 独立 Python 解析器（已有 `work/parse_ce.py`）** | 两个独立实现的共识 | 低 |
| L4 存档往返 | **WolfSave 的 `JsonDumper` + 官方存档** | 变量/状态语义错 | 中 |
| L5 视觉 | **原版录屏逐帧对比** | 渲染/坐标/图层错 | 高（人工） |

L2/L3 的产物已经存在（`work/wolftl/dump/` 与 `work/ce3.txt`），**它们能在写完 `wolf-format` 的当天就跑起来**，这是最快的早期信心来源。

### 8.2 L2 差分：数据层交叉验证（最先要做）

```rust
// crates/wolf-format/tests/diff_wolftl.rs
/// 断言：Rust 解析出的指令条数与 opcode 直方图与 WolfTL JSON 完全一致
/// 已知 WolfTL JSON 会丢 run_condition ⇒ 这一项**只对指令体**，不对 run_condition
#[test]
fn common_event_matches_wolftl_dump() {
    let ce = wolf_format::commonevent::parse_file(&data_path("BasicData/CommonEvent.dat")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&read("work/wolftl/dump/CommonEvent.json")).unwrap();

    assert_eq!(ce.events.len(), 550);
    // 已实测的硬数字，任何回归都会立刻炸
    let total_slots: usize = ce.events.iter().map(|e| e.commands.len()).sum();
    assert_eq!(total_slots, 163_828);
    let real: usize = ce.events.iter().map(|e| e.commands.iter().filter(|c| c.opcode != 0).count()).sum();
    assert_eq!(real, 139_273);

    // opcode 直方图逐项对比（前 14 个已实测；这里对全部 48 个）
    let mut hist = std::collections::BTreeMap::<u16, usize>::new();
    for e in &ce.events { for c in &e.commands { if c.opcode != 0 { *hist.entry(c.opcode).or_default() += 1; } } }
    assert_eq!(hist[&401], 18_522);   // 13.3% of 139,273（实现期用真实数字替换）
    assert_eq!(hist.len(), 48);
}
```

**为什么这比「跟原版比」更早更可靠**：数据层的错误（少读一个字节）会让后续一切努力白费，而它恰好有一个**免费的独立实现**（WolfTL）可以对拍。L2 全绿之前不要碰渲染。

**同时必须断言 WolfTL 的已知损失**：`run_condition` 不在 JSON 里 ⇒ 我们的 `442/97/10/1` 分布**无法用 L2 验证**，只能用 `work/parse_ce.py`（L3）对拍。这正是「JSON 有损所以要读二进制」在测试上的映射。

### 8.3 L4 Save-Diff Oracle（存档往返，最有力的语义验证）

```rust
// crates/wolf-save/tests/roundtrip.rs
/// 三个子测试，从弱到强
#[test] fn decrypt_then_encrypt_is_byte_identical() {
    // (a) 纯往返：读官方存档 -> 立即写出 -> 与原文逐字节相同
    for f in official_saves() {
        let orig = std::fs::read(&f).unwrap();
        let doc = SaveDocument::open(&orig).unwrap();
        assert_eq!(doc.write().unwrap(), orig, "byte-identical roundtrip failed for {:?}", f);
    }
}

#[test] fn decrypted_body_matches_wolfsave_json() {
    // (b) 与 WolfSave 的 JsonDumper 输出对比：变量区逐项相同
    //     这是「语义正确」而非「字节正确」的验证
}

#[test] fn self_written_save_has_valid_invariants() {
    // (c) 我们构造的存档：魔数/校验和/末尾 0x19 全部成立
}
```

**为什么 (a) 是最高价值的测试**：(a) 通过意味着「解密 + 分段 + 重新加密」全链路无误，**任何一处偏移错、rand 流步长错、校验和错都会立刻暴露**，而且它是**完全的自动化**（不需要原版游戏）。这是本项目里唯一能对官方二进制产物做**逐字节断言**的地方。
**要求**：用户需提供 2-3 个官方存档样本（不同进度/不同角色名长度/是否含中日文字符名）放进 `tests/fixtures/saves/`。**这些是用户自己的存档文件，不是游戏资产**，可以入库（或加 `.gitignore` 由用户本地放置）。

### 8.4 L5 视觉：逐帧对比

```rust
// crates/wolf-cli/src/cmd/visual_diff.rs
/// 输入：原版录屏片段 (mp4/png 序列) + 相同输入脚本
/// 输出：逐帧差异报告（差异像素比例 + 首个超阈值帧号 + 差异可视化图）
pub struct DiffReport { pub frames: usize, pub worst_frame: usize, pub worst_ratio: f32 }
```

**做法**（这是唯一能验证「渲染正确」的手段）：
1. 用 OBS 以**固定 1024×576 窗口、无缩放**录制原版 30-60 秒（例如：标题画面 → 进入 SampleMapA → 走几步 → 触发一次对话）；
2. Rust 侧用 `godot` 式的「脚本化输入」（记录 `(frame_no, key, state)` 序列）重放同一段；
3. 逐帧对比：**允许阈值**（差异像素 <0.5%）——因为字体 hinting、亚像素、`\E` 描边的边缘抗锯齿可能与原版差 1px；
4. 差异图人工看一眼：若差异是「整体偏移 1px」或「颜色通道错」，那是坐标/颜色空间 bug；若是「文字不同」那是字体/编码 bug。

**注意字体**：主字体是 **LOLITA**（在 `SystemFile` 或 `SystemGraphic.wolf` 里），必须解出来喂给 cosmic-text。**LOLITA 的许可**需要确认（若不允许随衍生作品分发，则**运行时从用户自己的 Data/ 读**，我们的仓库里不放字体文件、只放字体名的配置）。这正好符合「资产一律运行时从用户自己的 Data/ 读」的约束。

### 8.5 L6 模糊测试（防 panic）

输入是**不可信数据**（用户自己的游戏文件可能被改过/版本不同）。所有解析器（`wolf-format` 的 4 个 .dat 解析器 + `.mps` + `.wolf` + `wolf-save`）必须**永不 panic**，只返回 `Result`。

```rust
// fuzz/fuzz_targets/parse_commonevent.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    let _ = wolf_format::commonevent::parse_bytes(data);   // 只要求不 panic
});
```

**理由**：`binrw` 的 `#[br(count = n)]` 若 `n` 是攻击者可控的巨大值会 OOM；`#[br(seek_before)]` 负偏移会 panic。语料（corpus）直接用真实游戏文件的前若干字节 + 手工构造的越界样本。**这条在 v1 就要做**，因为「用户的游戏版本和我们的样本不同」几乎必然发生。

### 8.6 `wolf-cli verify`：一键自检子命令

```
wolf-cli verify --data-dir <游戏 Data/>
  # 全部期望值从 docs/BASELINE.json 读入（机器可校验；由 work/_ce_full.json +
  # work/maps_full.json + 解码后的 Game.dat 生成）。命令里不硬编码任何数字。
  1. 打开全部 .wolf，逐个打印 Head/Version/HeadSize/Flags 与文件数，断言 4 条不变量
  2. CommonEvent.dat：550 事件 / 163,828 槽 / 24,555 空 / 139,273 真实 / 47 非空 opcode
     / run_condition {0:442, 1:1, 2:97, 3:10}
  3. 9 个 .mps：73 事件 / 134 页 / 37,610 槽 / 7,052 空 / 30,558 真实；
     逐图对齐 BASELINE.json 的 w/h/tileset/empty_tiles/events/pages/commands
  4. 断言 opcode 并集 = 50（公共 47 + 地图独有 151/161/162），交集 = 37，地图侧 = 40
  5. TileSetData.dat：tileset_count=6、6 个 base_file 与 BASELINE.json 一致
  6. Game.dat：全部标量字段与 BASELINE.json 一致（含 encryption_key / pro_loadingpic / 分辨率）
  7. 扫全部音频扩展名，断言 0 个 MIDI
  8. 对每个归档，抽样 5 个文件（含 PNG/MP3）解压 + 解码成功
  9. 若有存档样本：跑 Save-Diff Oracle (a)(c)（用 BASELINE.json 的存档参数）
  退出码 = 失败项数
```

**期望值的唯一来源是 `docs/BASELINE.json`**（机器可校验，字段覆盖：公共事件 `550/163828/24555/139273` + `distinct_opcodes=47` + `run_condition`；地图事件 `73/134/37610/7052/30558` + `distinct_opcodes=40` + `per_map` 逐图 w/h/tileset/empty_tiles；`opcode_union=50`；`Game.dat` 全部标量；存档参数；`TileSetData` 的 6 个 tileset）。**`verify` 不硬编码任何数字**：期望值全部从该 JSON 读，实测值与之逐字段比对 —— 这样将来游戏的更新版本（或用户的 DLC 数据）只要重跑一次 `wolf-cli baseline` 刷新 JSON，`verify` 就自动跟着更新。

**这个子命令是整个工程的「健康检查」**：每个里程碑结束跑一次，任何一步数据层回归都会立刻定位到具体归档/文件。**它的输出也是给用户的第一份可信交付**（证明 Rust 侧真能读懂他的游戏）。

---

## 9. 风险清单（按严重度）

| # | 风险 | 影响 | 对策 |
|---|---|---|---|
| R1 | **wgpu 30 API 变动**：`VertexState.buffers: &[Option<VertexBufferLayout>]`、`entry_point: Option<&str>`、`ImageCopyTexture` 改名、`get_current_texture` 改返回枚举、`present` 移到 Queue | 编译期大量报错，网上教程（多为 wgpu 0.19/22）全部失效 | **本文件 §3.2 的对照表就是对策**；所有结论来自 `work/_crates/wgpu-30.0.1/` 源码实测，遇到疑问 grep 源码而不是搜网 |
| R2 | **整数 shader I/O 未加 `@interpolate(flat)`** | naga 编译失败，报错信息晦涩 | §3.7 已标注；CI 里加一个 `wolf-cli shader-check` 提前编译全部 WGSL |
| R3 | glyphon 与 wgpu 版本不匹配（两份 wgpu） | 类型同名不同源，编译错误无法自解 | `CI: cargo tree -d` + `Cargo.lock` 入库 + glyphon 0.12 的 Cargo.toml 已确认 `wgpu = "30.0.0"` |
| R4 | **winit 0.30 起必须用 ApplicationHandler，窗口只能在 `resumed()` 里建**，wgpu Surface 也依赖窗口 | 骨架写错后要重构 | §3.3 已给出完整骨架；`EventLoop::new()` 必须主线程 |
| R5 | winit 多显示器 DPI / `ScaleFactorChanged` 在 0.30 有已知 bug（窗口拖到不同 DPI 屏幕时 `inner_size` 报告不更新） | 缩放计算错误 → 画面模糊或拉伸 | 不依赖 `ScaleFactorChanged`，直接用 `Window::inner_size()` 每次 `Resized` 重算 `Scaler`（§3.4 的 `Scaler::new(w,h)` 是纯函数） |
| R6 | **`.wolf` 头部区域的哈夫曼实现**（39KB 参考代码） | 索引读不出来，全盘阻塞 | 先实现 `NO_HEAD_PRESS` 分支并**实测每个归档的 `flags`**；若全部归档头部未被压缩，则可推迟 Huffman（但 NameTab 高熵提示已压缩，故按「必须实现」排期） |
| R7 | `SavePartN` 若无自描述长度，就无法在 v1 跳过未实现的段 | 存档写入做不出来 | §6.4 的保真三明治 + **先跑 Probe 标定 7 个 Part 的边界**（这是 §6 的第一个任务，而不是最后一个） |
| R8 | `176 StartLoop2` / `600xxx` 动态调用语义未知 | 少量事件行为错（不影响主线 93.9%） | §2.9 已记为 TODO；v1 落 warning + 计数器，靠 §8.4 视觉对比时逐一暴露 |
| R9 | 字体 LOLITA 的许可 | 若不可分发，仓库不能带字体 | 运行时从用户 `SystemFile` 读；仓库只存「字体名 + 回退字体名」配置 |
| R11 | **`151 ChangeColor` / `161 PrepareTransition` / `162 ExecuteTransition` 只出现在地图事件里**（公共事件侧没有任何替代路径，那里只有「只登记参数」的 `160 SetTransition`） | 漏实现 ⇒ 地图切换是「瞬间跳图、无淡入淡出」，极易被误判成渲染 bug 而非 VM 缺口 | 三者列入 M2/M3 的必做清单（见 §10 备注）；`wolf-vm` 的 opcode 覆盖率测试按**并集 50** 统计，而不是只统计公共事件的 47 | §2.5 已按帧实现（取值 1/2/5/10/…/180 明显是帧数）；**用一个 30 秒的剧情片段做 L5 对比即可证伪** |

## 10. 里程碑（每步都有可验收产物）

| 里程碑 | 内容 | 验收 |
|---|---|---|
| M0 | workspace 骨架 + `wolf-cli verify` 只做归档层 | 打开 23 个 .wolf，打印文件清单，与 `work/alldata` 的 4089 个文件一一对应 |
| M1 | 数据层：CommonEvent / DataBase / SysDatabase / TileSetData / .mps | §8.2 L2 差分全绿；`verify` 输出 550 / 163,828 / 24,555 / 139,273 / **47 非空 opcode** 与 73 / 134 / 37,610 / 30,558，并断言 opcode 并集 = 50 |
| M2 | `wolf-vm`：`MagicNum` + 预编译跳转表 + `step_one` + `Scheduler` | 单元测试 +「跑 TitleMap event2 闭包（813 指令 / 27 opcode）1000 帧无 panic、变量演化确定」（同一 .rsav 重放两次结果一致） |
| M3 | 渲染：winit + wgpu 30 清屏 → tile → 角色 → 文字 → **地图切换转场（`161`/`162`/`151`）** | 能显示 SampleMapA 并走动；**地图切换有淡入淡出**（不是瞬间跳图）；§8.4 视觉对比差异 <0.5% |
| M4 | 音频接入 | BGM 播放 + 循环点 + `140 Sound` 的 SE 并发（`argc` 4/6/7 三套语义，负 `arg[0]` = 停止/淡出，`stringArgs[0]` 可为转义串需先求值；注意 `179` 是 LoopTimes 循环次数，与 SE 无关）。**状态：已达成**（`crates/wolf-audio` symphonia 解码 + cpal 输出 + 软件混音，循环点读 ogg `LOOPSTART/LOOPLENGTH`；`Game::enable_audio` + CLI `--audio` 接线，见 `docs/AUDIO.md` §6；`cargo test -p wolf-audio` 9 项 + `crates/wolf-game/tests/audio_wiring.rs`；commit `270ab37`） |
| M5 | 存档层 | §8.3 三个子测试全绿（尤其 (a) 逐字节往返） |
| M6 | 主线贯通 | 从标题画面走到 SampleMapA 的一小段剧情（对照 108 个并行事件全部活跃无死锁）。**状态：纵切片已达成**（Map003 ev1 新游戏钩子 → 130 到 SampleMapA(62,38) → ev0 卧室 HUD + 开场声明；92.1% 非黑、6000 帧无死锁、108 后台注册表活跃；`m6_mainline.rs` 固定，见 HANDOFF_M4 §4.6）。标题入口也已可验收：`capture --boot` 从标题菜单确认进新游戏后，地图切换清掉旧标题的负号画面层，输出与 Map003 路径同为 92.1% 非黑、干净卧室+HUD（`work/out/title_clean.png`，测试 `the_title_menu_new_game_path_no_longer_leaves_title_art_over_the_room`）。前两段可玩行动也已打通（卧室有两个鼠标图标：读书 → 「对各种事的理解加深了。」、钻研 → 「自主训练提升各项能力。」，可连续触发；work/out/m6_first_action.png、m6_second_action.png，见 HANDOFF_M4 §4.6 末段）。行动后的存档往返也已验证（真实 Game 跑 1500 帧 + 读书 + 钻研 → save_to → load_into，可见画面层/800 变量/playtime 一致；测试 `a_live_played_scene_survives_a_save_and_reload`）。跨天引擎侧已跑通：CE329 就寝処理 从 18:30 第 3 天推进到 08:30 第 4 天（work/out/m6_sleep.png、m6_mainline.rs::the_sleep_event_rolls_over_to_the_next_morning）；玩家菜单事件链也已补通：SampleMapA ev1 妹クリック判定（夜 page3 写 var63=1 → ev4 メニュー処理 page4 三选一 → 今天还是睡吧 → 是的 → CE329 → 08:30 第 2 天；测试 `the_bedroom_sleep_menu_choice_rolls_over_to_a_new_day`。仅 ev1 的指针命中判定未在 headless 复刻。 |

**为什么 M2 排在 M3 之前**：VM 的正确性可以**完全脱离渲染**验证（这是选择「显式状态机 + 可快照」的直接收益，见 §2.7）；而渲染没有 VM 就只能画静态图，无法验证图层与坐标的语义。先 VM 后渲染让两个阶段各自有独立、可自动化的验收。

**⚠ M2/M3 的追加必做项：地图切换转场（`151` / `161` / `162`）**

这 3 个 opcode **只出现在地图事件里**（§2.1.1 修正），**公共事件中没有任何等价替代**——公共事件侧只有 `160 SetTransition`（登记转场参数），真正执行转场的 `162 ExecuteTransition` 一条都不在公共事件里。因此：

- **M2（VM）**：opcode 覆盖率必须按 **并集 50** 统计。只统计公共事件会得到 47，会把这 3 条静默漏掉；建议单元测试里加一条「地图事件闭包（73 事件 / 134 页）全 opcode 均可编译成跳转表且无 `UnknownOpcode` 警告」。
- **M3（渲染）**：验收标准显式包含「**地图切换有淡入淡出**」。缺 `162` 的表现是**瞬间跳图**，肉眼看起来像渲染没画好（黑一帧 / 直接换图），实际是 VM 缺指令 —— 这类「症状在渲染、根因在 VM」的误判是本项目最容易踩的坑，所以在里程碑表里把转场直接写进 M3 的验收。
- **`151 ChangeColor`** 与 `160/161` 的关系：`160` 登记、`161` 准备、`162` 执行、`151` 改全屏染色。四者构成一套完整状态，实现时必须成套（只做 `162` 而不做 `151`，颜色过渡的中间帧仍然是错的）。
