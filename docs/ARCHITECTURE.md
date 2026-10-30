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