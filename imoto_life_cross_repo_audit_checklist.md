# `imouto-life` → `imoto-life` 跨仓库 Bug 查缺补漏与 UI/UX 优化清单

> **用途**：供 **Gemini 4 Neon** 在 `imoto-life` 后续收尾阶段进行代码深挖、静态扫描与本地验证的对照清单。
> **依据**：基于两个仓库当前快照的静态代码与文档核对（`imouto-life` 0.1.0 发布版、`imoto-life` M5 收口版 `AppPaths`/`SetupWizard` R12 修复已合入快照）。云端无 Godot 4.7.2 .NET 运行时与零售 `work/Data`，凡涉及零售脚本实际分布或实机画面表现的项均标注 `需数据核实` 或 `needs-runtime`。

---

## 0. 2026-11-19 落地状态与勘误（优先于下文旧快照）

> 本节记录实际落地结果。下文的“当前代码现状”是审计当时的快照，**已过时**；两者冲突时以本节和 Rust 参考实现为准。

### 已完成

- **AutoEvents**：最终采用**边沿触发** + `PrimeLatched`，仅 `RESUME_ON_LOAD={216,217}` 每次 replay；242/243 仍是 gated call only。原文“电平触发”与 Rust `auto_events.rs` 不符，**不要**再改成每帧重启。
- **Opcode 176**：2026-11-19 R13 复核推翻旧结论：Rust `program.rs:293` 只把 111/112/102/170/179 建成 `BlockKind::Loop` 头，**不含 176**；`exec.rs:3150-3158` 的 case 176 在有循环时 `close_loop`、否则 log 后 `pc+=1`。`Program.cs` 已删除 176 块头，`EventVm.cs` 的 case 176 已改为 `ContinueLoop`。原文“176 是循环头”和“176 只 Pc++”都不准确。
- **150 / 151 / 130 / 140**：150 图片参数按掩码与 Self 段保留 raw，其余 EvalNum；151 frames 已 EvalNum；130 按 raw `target,x,y,map,options`；140 raw volume/frames 等。
- **125 / 126**：125 低 8 位 AutoKeys + 2 帧 TTL + 每实例一次投递（C# 有意偏差，Rust 无 TTL）；126 直接用 host 位布局，不做 WOLF 位转换。
- **211**：已实现预约队列、绝对 target 映射、前台空闲时每帧弹一个、`ResetSessionState` 清空。
- **124**：EngineState 0/1/7/8/9/10/11/12/20/21 与 field 17/18；CharacterState 全字段（-2 为队伍领队）；PictureState field 0/1 = 坐标 + `effect_offset`（shift+shake），field 2/3 = **真实绘制宽高**（文本尺寸/窗口/纹理或分格 cell，Rust `game.rs:6809-6851`），field 8 对 hidden 返回 0。此前“2/3 保持百分比”是旧快照误读。
- **121**：低 8 位编辑器 flag 全部无意义；0x600/0x700 恒等 current，0x800 abs(current)，0xC00 sqrt(max(value,0))*1000；600000+ 调用是绝对映射，不是相对 caller。
- **CommonSelf**：只在实例结束/停止落库，不做实时广播。
- **表现层**：160/161/162 TransitionState + GameRoot 黑色三角波 overlay；MapLayer 改 `MOUSE_FILTER_PASS` 且不再 `accept_event`。
- **PictureStage**：150 fade/slide/erase-fade、hide、blend、独立 `scale_y`、负号固定屏层按原生尺寸（只有 `>=12000` 全屏）；290 per-picture 特效 0..5/7..10；`stage.tick()` 已接入每帧。R13 补齐：tone 默认 `[100,100,100]` 且只读 `TONE_PRESENT`；同槽仅 kind/file/text 且位置相同时继承 shake/zoom/breath/pattern；`_hit_rect` 含 `effect_offset` 与真实尺寸；分格 sprite-sheet 用 AtlasTexture，PATTERN 真正切 cell；blend 材质缓存、MUL 强制 alpha=1；tick 只推进有动画的槽。162 转场等待已补 `return Signal.Blocked`。
- **前端**：F11 全屏、F4 200 条消息回顾、回标题确认、`_modal_release_guard` 释放守卫。
- **RNG**：`WolfRng.FromEnvironment` 在 `WOLF_LIVE_RNG` 非 0 时用 `EventSeed` 时钟扰动；窗口启动默认调用 `vm.SetEventSeed(Time.get_ticks_usec())`，但判断必须用 `DisplayServer.get_name() != "headless"`（`OS.has_feature("headless")` 在 `--headless` 下仍为 false，误用会让 CI 随机化）；headless/CI 保持固定 seed，`WOLF_LIVE_RNG=0` 可关闭窗口扰动（对齐 Rust `M25_LIVE_UX.md`）。

### 已验证

- `PictureStateTest` checks=96、`GameRootTest`、`LayerOrderTest`、`MapLayerTest` checks=18、`Smoke` / `ALL VM TESTS PASSED`、`SessionParityTest` checks=10 frames=418、route diff 149247/149247（100%）均通过。
- 完整 headless 门禁 `reports/gate_headless.log`：`GODOT_GATE_OK`、`EXIT=0`、38/38；M13 slot1 `frames=34437`、slot8 `frames=34631`。
- 渲染门禁 `reports/gate_render.log`（`LWsmf_RENDER=1`）：`GODOT_GATE_OK`、`EXIT=0`、38/38；M13 帧数与 headless 完全一致。
- 注：上一轮门禁 M13 为 35217/35411，本轮少 780 帧；两轮断言均通过、route diff 100%，差异来源未逐帧定位。

### 仍未完成/已知偏差

- 290 颜色公式仍为近似（PATTERN 已通过 AtlasTexture 真正切 sprite-sheet cell）；150 keep-shift(0x400000) 与 relative draw 未做；
- 125 TTL 偏差、211 参数扩展、140 tempo/loopPoint 属长期对齐项；
- 未生成 release 包（本阶段明确不做）。

## 一、两个仓库的定位与架构差异（防误判边界）

两作本质上都是**定屏 2D SLG**，在复用前作经验前需明确两者的玩法增量与工程边界，避免把开发期逆向产生的通用 RPG 机制当成核心玩法去深挖：

| 维度 | `imouto-life`（前作，0.1.0 已发布） | `imoto-life`（姊妹作 / LWSMF，M5 收口中） |
| --- | --- | --- |
| **原作定位与玩法** | 2019 WOLF RPG Editor；定屏 2D SLG，无 RPG 战斗，核心为数值养成、日常对话与深夜交互 | 不同版本 WOLF；同样是**定屏 2D SLG**（无实际玩家/相机滚动与可移动瓦片玩法），但增加了更复杂的数值/数据库系统（550 公共事件、169,831 条指令、9 结局 + DLC）和一个轻量回合制战斗系统 |
| **运行时架构** | Rust 核心（`wolf-vm`/`wolf-game`/`wolf-gdext`）+ Godot 4.7.2 GDExtension 混合前端（`GameView.gd`） | 纯 Godot 4.7.2 .NET（C# `core_cs/` + `presentation/` + GDScript `presentation_gd/`），Python 离线转换（`converter/`），Rust 仅作开发期 Oracle |
| **引擎通用机制冗余** | 前期曾做过瓦片层桥接，T13 已从玩法前端彻底移除，只保留 Picture/Text/HUD | 仓库内包含 `Terrain.cs`、`MapLayer.gd` 三层 `TileMapLayer` 与 `MapRuntime.cs` 碰撞/寻路，属开发期逆向引擎通用机制，实际 SLG 流程基本不用 |
| **不要照搬或浪费算力的项** | `imouto-life/docs/BASELINE.json`、数据库文本期望 | 1) 无需深挖玩家/相机滚动、`Opcode 281`（Scroll）或 `CharacterState` 地图角色移动模型；2) 当前快照已合入 R12 修复（`AppPaths.cs`/`AppPaths.gd` 与 `SetupWizard.gd` 状态文件清理/`convert.bat`），无需重复排查 R12-P1-01/P2-01 |

> **附带排查项（对应 `imouto-life` T13 教训）**：既然 `imoto-life` 实际也是定屏 Picture 驱动的 2D SLG，建议确认 `MapLayer.gd` 的地图格子渲染与点击/悬停事件（`cell_hovered`）在常规 SLG 界面下是否会浪费渲染开销，或与 `PictureStage.gd` 的 Picture 按钮热区抢占输入。

---

## 二、VM 字节码、数值系统与事件调度语义（高危 Bug 查缺补漏）

`imoto-life` 的核心复杂度集中在更庞大的数值、数据库读写与事件调度上。以下条目均来自 `imouto-life` 在 `HANDOFF_DEEP_REVIEW_FIX.md`（BUG-25..39）中通过零售 `Game.exe` 反编译证实的引擎语义，并在 `imoto-life` 当前 C# 核心代码中发现了直接对应的问题或未实现桩：

| 优先级 | 检查项 | `imouto-life` 已证实结论 | `imoto-life` 当前代码现状与证据 | 给 Gemini 4 Neon 的深挖方向 |
| --- | --- | --- | --- | --- |
| **P0** | **`AutoEvents` 电平触发 vs 边沿触发**（BUG-25） | 零售 `run_condition == 2` 为**电平触发**（条件满足且无活跃实例时每帧重启），否则无内部循环的并行事件只会执行一次就永久失效 | `godot/core_cs/AutoEvents.cs:25,95-135` 采用**边沿触发**（`now && !before`），仅白名单 `ResumeOnLoad = { 216, 217 }` 为电平重启 | 扫描 `game_data/programs.json` 中所有 `RunCondition == 2` 的公共事件，统计哪些事件内部没有 `170` 循环；评估将 `AutoEvents.Sync` 全面改为电平触发（或扩充白名单）是否会导致部分事件死循环，并核查 `CE 43` 等初始化事件 |
| **P0** | **`Opcode 176`（ContinueLoop）被误作循环头**（BUG-36） | `176` 是 `ループ開始へ戻る`（ContinueLoop），**不是循环头**、无配对 `498`，语义是跳回最内层 `170`/`179` 开头 | `godot/core_cs/Program.cs:100` 与 `EventVm.cs:534` 将 `case 176:` 当作与 `170` 相同的无条件循环头（`170/176`） | 统计 `programs.json` 中全部 `176` 出现位置，检查 `Program.cs` 的块结构解析（block stack / `EndTarget`）是否因把 `176` 当循环头而产生隐式闭合或跳过后续指令 |
| **P1** | **`Opcode 150` 参数求值掩码误伤数据库 Magic** | 仅打包颜色或特定哨兵保持 raw，所有引用变量/数据库的数值参数必须 `EvalNum` | `godot/core_cs/EventVm.cs:783-790` 使用 `((uint)raw & 0x0F00_0000) != 0` 跳过 `EvalNum`；但 `CData`/`DB`/`SysDB` magic（如 `1_100_000_000 = 0x4190_AFB7`）在该掩码下非零 | 核对 `Magic.cs` 的全部地址段编码与 `programs.json` 中 `150` 参数值分布，确认是否有图片坐标/缩放/透明度引用了 `1_000_000_000+` 数据库 magic 并被跳过求值 |
| **P1** | **`Opcode 125`（AutoKeys）无 TTL 与同帧投递锁**（BUG-30） | 注入键需保留 2 帧 TTL 且每协程每帧最多消费一次，防止先于注入者执行的协程漏读或状态粘死 | `godot/core_cs/EventVm.cs:1093-1095` 直接覆盖 `AutoKeys = ...`，无过期机制与消费标记 | 检索 `programs.json` 中 `125` 的全部调用点，检查在多协程同帧调度下是否存在按键注入丢失或卡死 |
| **P1** | **`Opcode 151`（ChangeColor）`frames` 未求值**（BUG-39） | 首参打包 RGB 必须保持 raw，第 2 参 `frames` 必须走 `EvalNum` | `godot/core_cs/EventVm.cs:1101-1107` 对 `slot.Params[1]` 直接取原始整数，未调用 `EvalNum` | 扫描 `programs.json` 中 `151` 调用点，确认第 2 参是否使用变量 magic；若是，补上 `EvalNum` 并确认表现层已接入 |
| **P1** | **`Opcode 211`（Reserve）为空桩**（BUG-32） | 预约公共事件需入队 `reserved_events`（含参数 `reserve_with_args`），在前台空闲时弹出执行 | `godot/presentation/VmController.cs:720` 为空实现 `public void Reserve(int target) { }` | 检索 `programs.json` 与地图事件中全部 `211` 调用点，确认是否有战斗/剧情/菜单依赖预约机制 |
| **P1** | **`Opcode 124` 状态查询接口缺口**（BUG-28, 44, 47） | 需完整支持 `EngineState(10/11/12/17/18/20/21)` 与 `PictureState(2/3)` 真实尺寸（注：`CharacterState` 仅在用到时补，SLG 通常不用） | 1) `VmController.cs:550-559` `EngineState` 仅实现 `7/8/9`，`10`（滚轮增量）、`11/12`（鼠标 dx/dy）、`17/18`（当前/前台事件 ID）、`20/21`（屏幕鼠标坐标）均返回 `0`；2) `PictureStage.gd:200-201` `field 2/3` 仍返回 `scale` 百分比而非像素宽高/文本尺寸 | 统计 `imoto-life` 全库 `124` 调用的 `(sub, field)` 分布，按实际使用频率补齐 `EngineState` 与 `PictureState(2/3)`（`需数据核实`） |
| **P2** | **`Opcode 121` 位域与 `CommonSelf` 实时双写**（BUG-27, 29） | 需完整支持 `0x01` 钳制、`0x02` f64 实数、`0x10/0x20/0x40` 二阶指针、`0x10000` 批量，以及 `SelfVar` 600..700 与 `common_self` 实时双向同步 | 需逐条核对 `EventVm.cs:629`（`121`）及 `StoreNum`/`StoreStr` 是否已完整覆盖上述全部位域与并发协程双写 | 对照 `imouto-life/docs/HANDOFF_DEEP_REVIEW_FIX.md` §6.1 对 `EventVm.cs` 做全位域审计并补单测 |

---

## 三、表现层、图片渲染与特效缺口

作为纯 Picture + UI 驱动的 2D SLG，图片图层、渐变动画与特效几乎承载了全部视觉表现。对照 `imouto-life/docs/LIVE_PLAY_FIX_REPORT.md` 与 `T13_UI_REGRESSION_AUDIT.md`，`imoto-life` 当前表现层（`godot/presentation_gd/`）存在多处空缺：

| 优先级 | 检查项 | `imouto-life` 经验与修复方案 | `imoto-life` 当前代码现状与潜在问题 |
| --- | --- | --- | --- |
| **P1** | **`Opcode 160/161/162` 转场与 `Opcode 290` 特效未接入渲染** | `150` 淡入淡出、`160` 画面冻结/转场、`290` 位移 `(dx, dy)` 与呼吸/震动均参与实际排版与动画；`M4_SETTINGS_UI.md` 证实忽略 `290` 位移会导致设置界面文字重叠 | `GameRoot.gd:357-362` 中 `_on_transition` 与 `_on_effect` **仅输出日志**，无任何屏幕转场、闪屏、震动、呼吸或坐标偏移生效（`needs-runtime`） |
| **P1** | **`PictureStage.gd` 缺少淡入淡出、同图重绘补间与动画保留**（#5b, #8, #9, BUG-48） | 1) `Draw/Move/Erase` 使用 `fade_from`→`opacity` 帧级斜坡（修复黑屏转场瞬变）；2) 同图重定位保留旧 alpha 并做位置补间（修复侧拉栏瞬移）；3) 同图同位重绘保留 `breath/zoom/pattern` 状态（修复图标不动） | `PictureStage.gd` 目前在 `Draw`/`Move` 时直接瞬时赋值 `modulate` 与 `position`，`Erase` 直接释放节点，缺少帧级插值与动画状态继承 |
| **P1** | **`PictureStage.gd` 缺少混合模式（Blend）与纵向缩放/镜像** | `0=Normal, 1=ADD, 2=SUB, 3=MUL`（且 `MUL` 强制忽略不透明度）；同材质合批以维持高帧率 | `PictureStage.gd:354-364` 仅处理横向负缩放镜像（`scale.x = -1`），未解析混合模式字段，也未处理纵向独立缩放/镜像 |
| **P1** | **负号与 `>=12000` 图片的固定屏层 vs 全屏拉伸语义**（`M4_HUD.md` §1） | `imouto-life` 曾误将 `number < 0` 一律拉伸为 `1024x576`，导致左下/右下 HUD 图标与侧拉栏黑屏遮罩遮满全屏；实证负号仅表示固定屏层，不改变原图尺寸 | `PictureStage.gd:12,357` 将 `number < 0 or >= 12000` 一律设为 `final_size := SCREEN_SIZE`（`1024x576`）。需核查 `imoto-life` 中所有负号与 `>=12000` 图片的原始尺寸与用途，确认非全屏 UI 图标是否被错误放大 |
| **P2** | **气泡/对话框漏出原生头像与高度溢出**（#1, #2, #7） | 气泡模式启用期间全程屏蔽原生头像槽位（防淡出瞬间闪现大头像）；按实测高度收敛气泡框；顶栏遮挡改用几何交集判定（`y < 36`）而非按槽号一刀切 | 检查 `imoto-life` `MessageWindow.gd` 与 `PictureStage.gd` 在带立绘对话、黑幕转场时的图层遮挡与打字中按取消键的行为 |

---

## 四、实机随机性、存读档安全与音频/时钟健壮性

1. **实机游玩 RNG 固定种子问题（P1，对应 `imouto-life` `M25_LIVE_UX.md` §1）**
   * **现状**：`imoto-life/godot/core_cs/WolfRng.cs:41` 提供了 `SetEventSeed(uint)`，但全仓仅在测试中可见相关逻辑，`GameSession.NewGame()` 与 `GameRoot.gd` 实机启动时未注入时钟种子，导致默认恒用 `EventSeedDefault = 1`。
   * **建议**：保持 `WolfRng` 默认构造为固定种子（确保无头测试与 Oracle 对拍确定性），仅在 `GameRoot.gd` 实机窗口新开局时按系统时钟扰动 `SetEventSeed`（可留环境变量开关关闭）。

2. **存档防误触覆盖、自动存档与返回标题安全链路（P1/P2，对应 `imouto-life` `M25_LIVE_UX.md` §4）**
   * **现状**：`imoto-life` 已扎实落地 `.bak` 损坏回退、`recover_to_last_save()` 与 `SaveState.audio` 恢复；但原版脚本存档格通常“单击即覆盖”无确认，且缺少每日定时自动存档槽与独立前端「返回标题」入口。
   * **建议**：评估在 `GameRoot.gd` 增加前端「返回标题」确认弹窗（复用已有的 `Rebuild()` 逻辑）以及防误触覆写保护。

3. **跨日/战斗/H 场景中断后的状态残留清理与音频恢复（P2，对应 `imouto-life` `#3` & `BUG-42`）**
   * 检查 `imoto-life` 战斗或 H 场景被中途打断/跳过时，是否存在并行计时事件或临时 Picture 槽位（类似 `imouto-life` 夜袭进度条 `500..503`）未清零导致画面残留的问题；同时核对读档后 `AudioManager.gd` 对 BGM/BGS 循环点与淡入淡出的恢复表现（`needs-runtime`）。

---

## 五、操作交互、UI/UX 与前端工程增强

以下能力在 `imouto-life` 中已作为标准 SLG 前端体验落地（主要位于 `godot/scripts/GameView.gd`），可按需平滑移植到 `imoto-life/godot/presentation_gd/`：

1. **鼠标光标与热区回落策略（P1，对应 `LIVE_PLAY_FIX_REPORT.md` #4 & `M25_LIVE_UX.md` §2）**
   * 核查 `imoto-life` 零售版是否使用 `Sys74` 系统指针或在互动/H 场景使用图片光标；若脚本依赖热区悬停切换光标却在移出热区后“粘住”，可借鉴 `cursor_fallback.rs` 在宿主层统一做热区外回落。

2. **弹窗输入隔离与鼠标释放守卫（P1，对应 `GameView.gd` `_modal_release_guard`）**
   * 核查 `imoto-life` 的故障弹窗（`GameRoot.gd` fault modal）及未来新增的确认弹窗：关闭弹窗的那一帧若玩家尚未松开鼠标左键，需阻止该次按压穿透触发底层 Picture 按钮或选项点击。

3. **GBK / CP932 编码启发式边界（P1，对应 `M4_HUD.md` §2 & `LIVE_PLAY_FIX_REPORT.md` #6）**
   * 检查 `imoto-life/converter/` 与 `LegacySaveImporter.cs` 的字符串解码逻辑：确认是否会像 `imouto-life` 早期那样将合法 UTF-8 碰撞的 GBK 字节（如「强壮」`C7 BF D7 B3`）误判为 UTF-8 乱码，或将 GBK 省略号/问号误判为 CP932 半角标点 `U+FF61..FF64`。

4. **玩家便利热键与窗口缩放体验（P2，对应 `M5_BEYOND.md` & `M7_HOTKEYS.md`）**
   * `GameRoot.gd:461-485` 目前仅处理 `F5`/`F9`。建议补充：
     * `F11` 无边框全屏切换（记录切换前窗口位置与尺寸）；
     * 缩放模式切换（Integer / Fit-Nearest / Fit-Linear）及全屏/黑边下的鼠标逻辑坐标逆映射校验；
     * `F4` 消息回顾（Backlog，内存环形缓冲最近 200 条 `MessageWindow` 文本，不写存档）；
     * 可选热键配置文件与 `F2`/`F3` 调试/统计浮层。

5. **翻译文本替换层（P2，对应 `M7_I18N.md`）**
   * `imoto-life` 已有 `tools/translation/`，可参考 `imouto-life` 的运行时 TSV 目录替换（`[msg]`/`[choice]` 上下文键）与 100% 静态覆盖率严格检查命令，避免直接修改或污染零售二进制数据。

---

## 六、建议给 Gemini 4 Neon 的执行顺序与本地验证门禁

1. **第一阶段（数据静态摸底，快速定性 P0/P1）**：
   * 编写只读 Python 脚本扫描 `imoto-life` 的 `game_data/programs.json`（及地图事件），输出四份统计：① `RunCondition == 2` 事件是否含内部 `170` 循环；② `Opcode 176`、`211`、`125`、`151` 全部站点；③ `Opcode 124` 的 `(sub, field)` 调用分布；④ `Opcode 150` 中 `number < 0` 或 `>= 12000` 的图片原始尺寸与用途，以及参数是否含 `1_000_000_000+` 数据库 magic。同时确认 `MapLayer.gd` 在常规 SLG 界面下是否需要禁用或防抢占输入。

2. **第二阶段（核心与表现层定点修复）**：
   * 根据第一阶段实证，优先修补命中的 VM 语义（`AutoEvents`、`176`、`150` magic、`124` 缺失字段、`151`、`211`）与 `WolfRng` 实机时钟播种；
   * 随后补齐 `PictureStage.gd` / `GameRoot.gd` 的 `160/290` 特效管线、`fade` 斜坡、`blend` 混合与同图重绘补间。

3. **第三阶段（UI/UX 移植与本地门禁回归）**：
   * 合入 `F11` 全屏、`F4` 消息回顾、弹窗释放守卫等前端优化；
   * 在本地通过 `bash tools/godot_check.sh` 与 `LWsmf_RENDER=1 bash tools/godot_check.sh` 双模式全量门禁，并在 Windows 导出包中完成一次独立目录双击启动与实机冒烟。