# `imoto-life`（`5dd6956`）代码审阅、Bug 修复与 2D SLG 体验改进报告

- **审阅基线**：`imoto-life` commit `5dd6956d68691666dfc555773692bc3b60d1c0e2`（对比上一快照 `d0dcbae`），并结合前作 `imouto-life` 最新快照（`530de8a`）的实机经验
- **项目实际定位**：定屏 2D SLG（复杂数值/数据库养成 + 轻量回合制战斗；无真实玩家行走/相机滚动玩法）
- **环境边界**：云端沙箱无 Godot 4.7.2 .NET 运行时与零售 `work/Data` / `game_data/`，凡依赖实机画面或零售脚本分布的验证项均标注 `needs-runtime` 或 `需数据核实`

---

## 一、本轮改动（`123cd0d..5dd6956`）总评

本轮在 VM 核心语义、输入去抢占和表现层动画上推进非常扎实：
- **已闭环的高危项**：`MapLayer.gd` 改为 `MOUSE_FILTER_PASS` 并移除 `accept_event()`，解除了对 Picture 按钮点击的拦截；`EventVm.cs` 补齐了 `124` `field 17/18`、`151` `frames` 求值、`150` 参数求值掩码收窄、`211` 预约入队与空闲触发、`125` 2 帧 TTL 与每协程单次投递；`VmController.cs` 补齐了 `EngineState(10/11/12/20/21)` 与 `animate` 透传；`GameRoot.gd` 落地了 `F11` 全屏、`F4` 消息回顾、弹窗释放守卫 `_modal_release_guard` 与返回标题确认框。
- **仍存在 7 项明确的代码 Bug（其中 4 项 P1 会直接导致功能失效或画面异常）**，以及 **5 项结合 2D SLG 实际玩法的体验与减负改进建议**。

---

## 二、新发现的 Bug 清单（精确到文件与行号）

| 编号 | 严重度 | 位置 | 根因与实际影响 | 最小修复方案 |
| --- | --- | --- | --- | --- |
| **R13-P1-01** | **P1（功能失效）** | `godot/core_cs/WolfRng.cs:30, 41-47`；`godot/presentation_gd/GameRoot.gd` | **实机 RNG 种子传错构造函数，且窗口启动未开启**。`WolfRng.FromEnvironment()` 调用 `new WolfRng(uint seed)`，该重载只设置静态 LCG `_state`（`NextRaw`），而事件随机 `NextEventDraw()`/`Range()` 用的是 `_eventState`（仍固定为 `EventSeedDefault = 1`）。此外 `GameRoot.gd` 实机窗口启动也未开启时钟播种，玩家双击 `.exe` 每次开局随机序列仍完全相同。 | 1) `WolfRng.FromEnvironment()` 改为 `WolfRng.FromEventSeed(unchecked((uint)Environment.TickCount64))`；2) 在 `GameRoot.gd` 非 headless 窗口模式启动新会话时（除非 `WOLF_LIVE_RNG=0`）显式调用 `vm.Vm.Rng.SetEventSeed(...)`，保持 headless 与测试确定性。 |
| **R13-P1-02** | **P1（渲染失效）** | `godot/presentation_gd/GameRoot.gd:78-83` | **160/161/162 全屏淡入淡出黑幕被 PictureStage 完全遮挡**。`PictureStage` 内部 9 个 Band 均设置了 `z_as_relative = false` 且 `z_index = 10..50`；但 `_transition_rect` 直接挂在 `GameRoot` 下未设 `z_index`（默认 `0`），导致全屏转场黑幕画在所有立绘和对话框下方，实机完全看不见淡入淡出（`needs-runtime`）。 | 为 `_transition_rect` 设置 `_transition_rect.z_index = 60`（位于 `LAYER_CURSOR=50` 之上、前端 UI `90+` 之下）。 |
| **R13-P1-03** | **P1（色调失效）** | `godot/presentation_gd/PictureStage.gd:454-458, 726-745` | **`tone` 默认值写成了 `255` 而非 WOLF 百分比基数 `100`**。`_tone_color` 按 WOLF `0..200` 百分比公式 `(base * 255 + 50) / 100` 计算；默认 `tone = [255, 255, 255]` 在无特效时凑巧被 `clampi(..., 0, 200)` 截成 `200` 再截成 `255`（看似正常），但一旦脚本触发 `290 kind=1` 变暗（如 `color_corr = -80`），`255 - 80 = 175` 仍远超 `100`，导致**变暗色调特效完全失效**。 | `_new_state` 中 `tone` 默认值改为 `100` 且范围钳制为 `0..200`（`clampi(_arg(args, 13, 100), 0, 200)`），并核对 `args[13..15]` 在零售脚本中的实测值域。 |
| **R13-P1-04** | **P1（VM 时序缺陷）** | `godot/core_cs/EventVm.cs:903-909` | **`Opcode 162` 设完等待帧数后漏写 `return Signal.Blocked;`**。`162` 调用了 `inst.Wait.WaitForFrames((uint)frames)` 后只执行 `break;`，而不是像 `case 130`（`:740`）和 `case 180`（`:934`）那样立即返回 `Signal.Blocked`。结果 `StepInstance` 会在同一帧内继续往下执行转场后的指令，导致下一幕画面在转场等待前抢跑。 | 将 `EventVm.cs:908` 的 `break;` 改为 `return Signal.Blocked;`，并重新核对 route trace 与 M13 帧数。 |
| **R13-P1-05** | **P1（热区/特效污染）** | `godot/presentation_gd/PictureStage.gd:200-203, 239-268, 466-496` | **同槽换图误继承特效 + `_hit_rect` 与渲染几何脱节**：1) `_begin_draw` 忽略了 `_file`/`_text` 对比，同槽位换成**不同图片**时仍继承旧图的 `shake/breath/zoom/shift/color_corr`，且 `animate[0] > 0` 时会把新图从旧图位置滑过来；2) `_hit_rect` 未叠加 `state["shift"]`（`290 kind=2` 位移），导致平移后的按钮渲染位置与点击热区错位；3) `picture_state(field=8)` 未排除 `state["hidden"] == true`，被 `150 Hide` 隐藏的图片仍可被 hover 命中。 | 1) `_begin_draw` 仅在「同 `file` 且同 `text`」时继承动画相位与触发同图滑动，换图一律清空旧特效；2) `_hit_rect` 加上 `shift` 偏移；3) `field == 8` 在 `state.get("hidden", false)` 时返回 `0`。 |
| **R13-P2-01** | **P2（性能与混合保真）** | `godot/presentation_gd/PictureStage.gd:605-608, 720-723, 748-762` | **每帧无条件重绘 + 材质对象泄漏 + MUL 混合未忽略透明度**。`tick()` 每帧对 `_states` 中所有静态图都调用 `_render_picture()`，且 `_apply_blend` 对每张 `blend != 0` 的图片**每帧 `CanvasItemMaterial.new()`**；另外 WOLF 原版 `blend == 3`（MUL）强制忽略 `opacity`（alpha=1.0）。 | 1) 将 3 种混合模式（ADD/SUB/MUL）的 `CanvasItemMaterial` 抽成静态/成员单例复用，仅在模式变化时赋值；2) 仅在槽位存在活跃动画/特效或脏标记时才在 `tick()` 中调用 `_render_picture`；3) `blend & 0xF == 3` 时 `tint.a = 1.0`。 |
| **R13-P2-02** | **P2（空转逻辑）** | `godot/presentation_gd/PictureStage.gd:660-666, 841-877` | **`290 kind=8/9/10`（pattern 切帧）只算状态不改画面**。`_step_pattern` 每帧更新 `pattern["cell"]`，但 `_render_picture()` 从未读取 `pattern["cell"]` 去裁剪 `TextureRect` 的精灵表区域。 | 先通过 `neon/audit_programs_static.py` 确认 `imoto-life` 零售脚本是否实际使用了 `290 kind=8/9/10`；若站点为 0 则标注保留/精简，若有实际站点则为 `TextureRect` 接入 `AtlasTexture` 分格裁剪（`需数据核实`）。 |

---

## 三、关于截图中「M13 少 780 帧」的定向排查建议

截图中提到本轮 M13 `slot1`（34437）与 `slot8`（34631）较上一轮（35217/35411）**精确各少 780 帧**，但两轮断言全过且 `route diff` 100% 一致。结合本轮 diff，最可能的三个根因按概率排序如下：

1. **`VmController.EngineState` 对未知字段从返回 `0` 改为返回 `null`**（`VmController.cs:631`）：如果 M13 某段长剧情/日程循环中有 `124 0x3000` 查询了未覆盖字段，旧版写入 `0`、新版保持变量旧值，可能提前跳出某个等待或循环。
2. **`EventVm.cs` `case 125` 新增 2 帧 TTL 与 `AcceptsWolfBits`**（`EventVm.cs:685-689, 1307-1320`）：旧版 `125` 按键在清空前持续生效，新版最多活 2 帧且每协程只消费一次，可能在连续推进时改变了某段自动菜单的等待帧数。
3. **`EventVm.cs` `case 162` 的转场等待**（`EventVm.cs:904-908`）与 **`AutoEvents.cs` `IsParallel = true`**（`AutoEvents.cs:109`）：并行标记影响了 `124 field 18`（前台事件 ID）的返回值，可能导致某并行事件提前退出或走不同分支。

**建议操作**：不要靠猜，在本地用 `M13` 对 `123cd0d` 和 `5dd6956` 分别打一段每 1000 帧的 `(day, hour, minute, map_id, active_events)` 摘要日志，二分找到**第一次出现帧数偏移的那一天/事件**即可 5 分钟定性——确认是“修掉了旧版空转/卡顿”还是“误跳过了某段演出”。

---

## 四、结合 2D SLG 实际的体验改进与架构减负建议（不生搬硬套）

结合本作「定屏 2D SLG + 复杂数值 + 轻量战斗」的实际形态，以下建议刻意**避开**对本作无意义的移植（如不追着改 `CharacterState` 行走模型、不照搬前作 `imouto-life` 的 GBK 启发式与安卓触控方案），只做玩家实机能直接感知的提升：

### 1. 前端「返回标题」按钮位置与显隐优化（高收益）
- **现状问题**：`GameRoot.gd:596-604` 在 `(892, 8)` 放了一个常驻的 Godot 默认样式 `Button("返回标题")`。
  1) 在 `phase == "title"`（已经在标题画面）时它依然显示，点击会弹出“放弃本次游戏并返回标题吗？”的迷惑提示；
  2) 右上角 `(892..1012, 8..38)` 极易与 SLG 游戏原生的右上角 HUD（金钱、时间、系统菜单图标）重叠遮挡。
- **建议**：在 `phase == "title"` 时隐藏该按钮；游玩中改为**低存在感半透明小图标/悬停顶部边缘浮现**，或直接绑定快捷键（如 `F12` 呼出确认框），把 1024×576 画面完整还给游戏本身。

### 2. 对 `MapLayer` 与 `MoveLeader` 做「SLG 模式静默化」（减负防误触）
- **现状问题**：虽然 `MapLayer.gd` 已去掉 `accept_event()` 不再吞点击，但 `GameRoot.gd:323-331`（方向键）和 `:468-485`（点击相邻格）在 `phase == "bedroom"` 下仍然会调用 `session.MoveLeader(...)`，且 `map_view.visible = true` 会把底层的 TileMap 和金黄色 `_leader` 方块画在最底层（背景图淡出或透明间隙时会穿帮）。
- **建议**：既然实际游戏没有瓦片行走玩法，建议在 `GameRoot.gd` 中**默认不显示 `map_view` 也不响应 `_on_map_cell_clicked` / 方向键 `MoveLeader`**（可通过环境变量如 `LWsmf_SHOW_MAP_DEBUG=1` 保留给 `MapLayerTest` 和调试用），彻底消除背景穿帮与误触移动。

### 3. 补齐缩放模式与窗口体验（贴合桌面 SLG 游玩）
- **现状问题**：已有 `F11` 无边框全屏，但缺少类似 `imouto-life` 的显示模式与平滑度选择；另外 1024×576 在高分屏（2K/4K）窗口模式下偏小。
- **建议**：
  - 增加窗口初始尺寸记忆或一键缩放切换（1x / 1.5x / 2x 窗口，或 `F8` 切换最近邻/双线性过滤）；
  - 实机核对全屏有黑边（16:10 / 带鱼屏）时 `InputRouter.gd` 的 `event.position` 是否已由 Godot `canvas_items` stretch 自动映射到 `1024x576` 逻辑坐标，确保黑边不会导致 Picture 按钮点击偏移（`needs-runtime`）。

### 4. 消息回顾（`F4`）与存读档防误触打磨
- **消息回顾**：目前 `GameRoot.gd:378` 仅记录 `message.full_text`，建议顺手把说话人名（`Nameplate` 或 `@n` 解析结果）一并拼入记录，如 `【妹妹】...`，SLG 长对话回看体验会好很多；另外打开回顾时用鼠标滚轮也能自然滚动（目前 `_on_wheel` 在 `_modal_window_open()` 时直接 return，需确认 `ScrollContainer` 自身能正常收到滚轮事件，`needs-runtime`）。
- **存档防误触**：`F5` 快捷存档目前直接覆盖 `save_01.json` 且只在控制台打一行 `[GameRoot] saved ...`，玩家在游戏画面上看不到任何反馈。建议在屏幕角落加一个 **1.5 秒自动淡出的轻量 Toast 提示**（如「已快速存档」/「已读取存档」/「当前场景不可存档」），让 `F5`/`F9` 有明确的操作回馈。

### 5. 收口前不必再投入精力的项（明确止损）
- **不要再扩展 `CharacterState` / `Terrain.cs` / `Opcode 281`（Scroll）**：对定屏 2D SLG 无实际收益，现有测试绿着即可冻结。
- **不要盲目改动 `AutoEvents` 的全局边沿触发策略**：除非 `audit_programs_static.py` 在真实 `game_data/programs.json` 中扫出某个 `RunCondition == 2` 且无 `170` 循环的实际业务事件确实卡死，否则保持现有白名单（`216/217`）+ `route diff 100%` 最稳妥。
- **不要照搬 `imouto-life` 的 GBK 优先解码与 Android 触控改造**：`imoto-life` 是 WOLF v3 原生 UTF-8，且当前交付目标为 Windows x64，保持范围聚焦才能尽快稳定封板。

---

## 五、建议的下一步执行顺序

1. **优先修复 4 个 P1 代码 Bug**：`WolfRng.FromEnvironment()` 构造函数（`R13-P1-01`）→ `_transition_rect.z_index = 60`（`R13-P1-02`）→ `_tone_color` 默认值 `100`（`R13-P1-03`）→ `EventVm.cs` `case 162` 补 `return Signal.Blocked`（`R13-P1-04`），顺手修掉 `PictureStage.gd` 同图判断与 `_hit_rect` 的 `shift`/`hidden` 判定（`R13-P1-05`）。
2. **定位 M13 少 780 帧的分叉点**：修完 `R13-P1-04` 后重跑 `tools/godot_check.sh` 与 `LWsmf_RENDER=1` 门禁，观察帧数是否回正或定位具体差异日。
3. **完成 2D SLG 轻量 UX 收口并重新打包**：隐藏标题页的“返回标题”按钮、静默 `MapLayer` 调试方块、给 `F5/F9` 加上画面 Toast 提示；随后重新跑 `bash tools/package_release.sh` 与 `python tools/release_audit.py`，在独立目录双击实机验证一次即可封板。
