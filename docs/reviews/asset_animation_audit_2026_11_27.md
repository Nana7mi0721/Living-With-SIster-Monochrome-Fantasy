# 原版素材出现／消失动画审计与 `150 Picture` 语义修复（2026-11-27）

> **范围**：550 个公共事件与 9 张地图事件中的全部 `150 Picture` 指令；重点核对 CG、立绘、Cutin、战斗特效、消息窗、文本图、系统窗口、DLC、标题与 H 场景素材的出现／消失时序。
> **证据基准**：零售数据导出的 `game_data/programs.json`、WOLF RPG Editor 3.738 官方手册、`wolfrpg-map-parser` 字段解析与仓库内既有 Oracle。截图只用于最终视觉验收，不参与动画语义推断。
> **结论等级**：下文“已验证”表示有字节码字段或官方手册逐条支撑；“残余”表示当前实现仍未覆盖或尚缺直接证据。

## 1. 结论摘要

1. **主根因不是素材缺失，而是 `150` 的三个时间字段被混用**：旧实现把 `Process Time` 当成可回退到 `animate[0]` 的动画时长，又把动作 `3` 当成“隐藏/删除槽位”，并且完全没有执行 Show/Move/Erase 的 `Delay`。这三处会让大量素材提前、延后或永久消失。
2. **动作 `3` 的官方名称是 `Delay Reset`**，只取消尚未开始的延迟动作，不改变当前已显示的图片。旧实现会隐藏并删除槽位，造成打开菜单、切场景时素材被误删。
3. **Show 对已占用槽位等同 Move**，`Process Time >= 1` 时从透明开始；同一个 graphic 的 Show 保留槽位效果，换成另一张 graphic 则是新图，不继承旧图的闪光/抖动状态。
4. 本轮已补齐 Show/Move/Erase 的延迟队列、范围命令逐 picture ID 入队、范围 Delay Reset、Erase 的短格式 Delay、`Delay=0` Erase 的队列清理、占用槽 Show 的 Move 语义，以及 `Angle` 的渲染与 `Process Time` 插值。
5. **仍有一个高优先级结构缺口**：本作实际消息路径是 CE270（`run_condition=3`），不是 CE63 的窗口分支；当前 Godot 仍由自绘 `MessageWindow` 开关面板，CE270 的 640 宽动态气泡、脸图与立绘图片槽尚未接通。其余残余见第 8 节。

## 2. 调查方法

### 2.1 原版字节码

从转换产物 `game_data/programs.json` 读取每个 `op=150` 的原始 `params`、`strings` 与事件 `pc`，不再根据截图逐帧猜测。字段从 `PictureCmd.Args` 的起点开始，即原始 `params[2]`。

### 2.2 官方手册

使用 WOLF RPG Editor 3.738 官方手册 “Picture” 章节（PDF 第 88–92 页）：

- **Show**：显示新图片；若该槽位已有图片，则行为等同 **Move**；`Process Time >= 1` 时从透明状态开始。
- **Move**：移动图片；目标槽为空时不显示。
- **Erase**：擦除图片并释放图片内存。
- **Delay Reset**：取消所有带 `Activation Delay`、尚未发生的未来处理。
- **Process Time**：Show/Move/Erase 的持续帧数。
- **Delay**：动作真正开始前的等待帧数。
- **Angle**：以 `0.1°` 为单位，`3600` 为整圈。

### 2.3 第三方解析器与旧 Oracle 交叉核对

- `wolfrpg-map-parser` 把动作 `3` 解析为 `DelayReset`，并分别提供 Show/Move 的延迟字段与 Erase 的延迟字段。
- 仓库内旧 Rust `crates/wolf-game/src/picture.rs` 只作为历史 Oracle；其“Draw 重建整个槽位”等简化不作为原版行为依据。

### 2.4 原版 `Game.exe` 反汇编补证（2026-11-27）

对本机 WOLF RPG Editor 3.738 的零售 `Game.exe` 使用 Ghidra 定位图片命令处理函数：

- `FUN_00a1e150` 的动作 `3` 分支（`00a202af` 附近）会清空图片槽的延迟命令队列。
- 同一函数的动作 `2` 分支只执行擦除或把延迟擦除入队，未发现“延迟 Erase 激活时清空后续队列”的代码。
- `FUN_009946d0` 在 Delay `< 1` 时立即执行，否则把命令压入队列；`FUN_00994e40` 每帧把剩余值 `< 1` 的项出队执行，否则减去帧步长。

因此，“动作 3 清空队列”和“Erase 删除当前图片”是两个独立语义；WOLF Tips 对 `Delay=0` Erase 的重置说明作为上层行为证据保留。

## 3. `150` 字段与动作语义

### 3.1 字段布局

`PictureCmd.Args` 从原始 `params[2]` 开始：

| 下标 | Show / Move | Erase |
|---:|---|---|
| 0 | Process Time | Process Time |
| 1 | 分割宽／窗口宽 | Delay |
| 2 | 分割高／窗口高 | 未使用 |
| 3 | Pattern | 范围终点 |
| 4 | Opacity | |
| 5 | X | |
| 6 | Y | |
| 7 | ScaleX | |
| 8 | Angle（0.1°） | |
| 9 | Padding | |
| 10 | Flags / Tone 标志 | |
| 11 | **Delay** | |
| 12 | 范围终点 | |
| 13–15 | Tone RGB | |
| 16 | ScaleY | |

### 3.2 动作

| 线值 | 官方语义 | 本实现 |
|---:|---|---|
| 0 | Show；已占用槽位按 Move 处理，`Process Time >= 1` 从透明开始 | 已有图片槽生成姿态过渡；空槽 alpha 从 0 淡入 |
| 1 | Move；空槽 no-op | 只更新已有槽位，不凭空创建 |
| 2 | Erase；支持单槽与闭区间 | `Process Time` 淡出，`Delay` 到期后开始；完成后释放槽位 |
| 3 | Delay Reset；取消尚未开始的延迟命令 | 只清理待触发队列，不隐藏、不删除当前图层 |

### 3.3 Delay Reset 的线格式

- 单槽：`[head, picture]`
- 范围：`[head | 0x01000000, picture, endpoint]`

`HasSpan` 只在 head bit `0x01000000` 存在时成立。当前实现支持取消单个槽位和闭区间内的待触发 Show/Move/Erase；已经开始播放的 `Process Time` 过渡不会被倒回。

队列归属按 picture ID 拆分：一条范围 Move/Show/Erase 会为范围内每个 ID 各保存一条 pending，因此单槽 Delay Reset 只取消该 ID，不会误伤邻居。`Delay=0` 的 Erase 会清除该 ID 先前排队的 Show/Move/Erase；延迟 Erase 本身按队列顺序执行，不回溯取消排在它之后的命令。

**帧边界说明**：`PictureStage` 使用 `remaining = delay + 1`，因为命令阶段先于同帧的 `stage.tick()`；这保证 `Delay=N` 不会在命令帧立刻触发。原版队列使用“剩余值 `<1` 才执行、否则减去帧步长”的循环，但尚未通过逐帧运行时采样锁定 `N` 与 `N+1` 的精确边界，因此该 `+1` 目前是依据帧序的实现约定，不冒充手册原文。

## 4. 全量统计

全部公共事件与地图事件合计 **20,425** 条 `150`：

| 动作 | 条数 |
|---|---:|
| Show | 12,547 |
| Move | 3,684 |
| Erase | 3,950 |
| Delay Reset | 244 |

其中具有字面量非零 Delay 的站点：

| 动作 | 非零 Delay |
|---|---:|
| Show | 37 |
| Move | 328 |
| Erase | 148 |

非零角度只有 **4** 条，均为 `BattleEffect/[Attack]Slash01_panop.png`、`Angle=1250`：

| 事件 | pc |
|---|---:|
| CE239 | 763 |
| CE242 | 510 |
| CE242 | 881 |
| CE243 | 1012 |

Free Transform 标志在全部 `150` 中为 **0** 条，本作零售数据未使用自由四角变形。

### 4.1 素材分类

| 分类 | 总数 | Show | Move | Erase | Delay Reset | 原版主要表现 |
|---|---:|---:|---:|---:|---:|---|
| CG | 3,176 | 2,829 | 30 | 317 | 0 | Show 常用 15/30 帧淡入，Erase 常见 15 帧淡出 |
| 立绘／差分 | 652 | 470 | 54 | 128 | 0 | 大多瞬间 Show；少量 15/20 帧淡入；眨眼靠同槽 Pattern/Wait |
| Cutin | 1,041 | 861 | 109 | 71 | 0 | 20 帧淡入；Move 均为 Process Time；Erase 可带 Delay |
| 战斗特效 | 111 | 49 | 11 | 51 | 0 | 淡入、短淡出、延迟 200 帧 Erase、4 条 125° 旋转 |
| 消息窗／动态图 | 4,033 | 36 | 2,223 | 1,687 | 87 | 窗口滑入/避让、头像与光标 Move，Delay Reset 频繁 |
| 图片／UI 素材 | 1,874 | 1,595 | 60 | 219 | 0 | Picture/ 下的按钮、卡牌、背景与对话框部件；以 5 帧淡入/淡出为主 |
| 文本图 | 3,533 | 2,466 | 466 | 486 | 115 | 文字 Move 常带 3/6/9 帧 Delay；淡入淡出 |
| 程序化窗口 | 624 | 409 | 75 | 99 | 41 | 背景板与缩放展开；Delay 0/3/5 |
| DLC | 1,981 | 1,659 | 163 | 158 | 1 | 多为 2/5/15/30 帧淡入淡出 |
| 文字特效 | 962 | 480 | 480 | 2 | 0 | 瞬间 Show，再用 40–100 帧 Move 将 opacity 移到 0 |
| H 场景 | 2,348 | 1,634 | 2 | 712 | 0 | Show 全部瞬间；Erase 大多瞬间，少量 5/10 帧淡出 |
| 标题／启动 Logo | 25 | 20 | 0 | 5 | 0 | 20/30 帧淡入淡出 |
| 系统 UI | 2 | 1 | 0 | 1 | 0 | 瞬间显示／擦除 |
| 其他 | 63 | 38 | 11 | 14 | 0 | 混合 |

## 5. 代表性事件证据

### 5.1 CE36：范围 Delay Reset 与瞬时／淡出 Erase

- `pc80`：`action=3`，范围 `1600021..1600022`，取消尚未开始的延迟动作。
- `pc84`：范围 Erase，`Process Time=0`，立即消失。
- `pc89`：范围 Erase，`Process Time=self86`，按 self86 淡出。
- `pc106`：单槽 Erase，`Process Time=self86`。
- `pc228`：瞬间 Show。
- `pc233`：`Show Process Time=self86`，淡入。

### 5.2 CE63 / CE270：实际消息路径

- CE63 `pc70` 检查 `Sys12`，`pc74` 把 `Sys0` 写入 self49，`pc78` 再以 `self49 >= 1` 决定是否显示窗口。本作 CE264 的调用点首个字符串实参为空，CE267 回退使 `Sys0=0`，因此 CE63 的 `pc90` 窗口 Show、`pc93/94` 错峰 Move 与 `pc215` 擦除分支在零售流程中不可达。
- 实际对话由 CE270（`run_condition=3`）绘制：`pc86` 检查 `Sys12`，`pc92/95/96` 读取系统字符串 25，`pc419/422` 分配 640 宽文本槽，`pc521` 画脸，`pc559/574` 画立绘，`pc1422/1427` 显示窗口，`pc1730/1734` 画正文，`pc1885/1889` 画名字。
- 关闭时 `pc2597` 发出 Delay Reset，`pc2598` Erase，`pc2605` 再 Delay Reset，随后逐个移动/擦除子件。

因此不应照 CE63 的 960×168 窗口去猜滑入动画；当前缺口是 CE270 的图片槽时间线没有接入自绘消息框。

### 5.3 CE64 / CE85：错峰展开与滑动

- CE64 `pc315–318`：通过纵向 ScaleY 0→100、`Process Time=5`、`Delay=0/3/5` 做窗口展开。
- CE85 `pc200–202`：同一目标连续 Move，`Process Time=5`、`Delay=0/3/5`，形成分段追赶。

### 5.4 CE94 / CE426 / CE301：文本、CG 与战斗特效

- CE94 `pc214–216`：文本图同目标 Move，`Process Time=5`，Delay 为 self96/97/98。
- CE426 `pc384`：`cg/whitescreen.jpg` Show，`Process Time=200`；`pc488` 立绘 Show 20 帧；`pc491` Erase 20 帧。
- CE301 `pc535`：战斗特效 Show 10 帧；`pc544` Erase `Process Time=100`、`Delay=200`。

## 6. 本轮代码修复

### 6.1 `godot/presentation_gd/PictureStage.gd`

- 新增按 picture ID 归属的待触发队列 `_pending_pictures`：范围 Show/Move/Erase 逐 ID 入队，Delay 到期后才按原始命令重放。
- 动作 `3` 改为 `_reset_delayed_pictures()`：按单槽或闭区间取消 pending，不隐藏当前图层。
- Show/Move 读取 `args[11]` 的 Delay，Erase 读取 `args[1]` 的 Delay。
- `Delay=0` 的 Erase 清除该槽位先前排队的 Show/Move/Erase；延迟 Erase 激活时不清空后续命令。
- `_transition_duration()` 只读取 `args[0]`，移除把 `animate[0]`（实际是 Delay）当后备时长的错误。
- 占用槽 Show 使用 Move 语义；`Process Time >= 1` 时 alpha 从 0 开始，同一 graphic 保留 290 效果，换成另一张 graphic 时不继承旧图的闪光/抖动。
- `Angle=args[8]` 进入姿态、插值与 `Control.rotation`；旋转中心为图片中心。
- `has_active_animations()`、`clear_pictures()`、`restore_pictures()` 同步处理待触发队列。
- `track_picture()` 与 `draw_picture()` 使用相同的 Delay/Delay Reset 语义，避免路线追踪模式与实机模式分叉。

### 6.2 `godot/presentation/VmController.cs`

- 只有 `PictureAction.Erase` 从 `_pictures` 移除槽位。
- `PictureAction.DelayReset`（线值 3）不再删除 slot，只交给 `PictureStage` 执行 Delay Reset。
- Erase 范围终点固定读取 `Args[3]`；Delay Reset 不再误读 Erase 字段。

### 6.3 `godot/core_cs/EventVm.cs` / `IHost.cs`

- 枚举名由 `PictureAction.Hide` 改为 `PictureAction.DelayReset`；轨迹录制仍输出 `"hide"` 以兼容既有 fixture。
- `HasSpan` 说明改为 Erase / Delay Reset 的范围终点。

### 6.4 标题菜单与测试夹具

- iOS 标题菜单隐藏第 5 行时改为显式 `action=2 Erase`，不再借用动作 3。
- `TestSessionView`、`RouteTraceTest`、`M4FullRouteTraceTest` 改为只有 Erase 删除槽位。
- `PictureStateTest` 新增 Delay Reset、延迟 Move/Erase、范围取消、已开始过渡保留、旋转插值、Show 透明起始等回归。
- `GameRootTest` 的 Title→Save/Load 取消断言改为等待 Delay 与淡出真正结束，不再假设 30 帧内瞬时关闭；该场景恢复 `GAME_ROOT_TEST_OK`。

### 6.5 附带素材修正

- `PictureShapes.gd` 将 `<GRADX/GRADY-RGB1-RGB2>` 从“白色 Alpha 梯度”修正为三位十进制 RGB 端点的程序化渐变；HUD/战斗条的渐变色不再被误画成透明。
- 战斗 AP 的 9 连星 `★/☆` 与方块、圆点采用同一套逐格定宽步进，避免多槽叠加后左漂。

## 7. 验证

| 项目 | 结果 |
|---|---|
| `dotnet build godot/Lwsmf.csproj -c Release` | 0 警告 / 0 错误 |
| `python tools/run_godot_test.py res://tests_gd/PictureStateTest.tscn --timeout 90` | `PICTURE_STATE_TEST_OK checks=236` |
| `bash tools/godot_check.sh` | 36/38 通过。仅 `Smoke`（`headless fixture must run four command rounds, got 3`）与 `M13 slot 1`（180s 超时，`stage=10 has305=false v492=1`）失败；两者均指向本机 `work/retail_backup/Save/SaveData01.sav` 被真实进度覆盖（387,265 B，测试基线 366,146 B），非本轮改动引入 |
| `LWsmf_RENDER=1 bash tools/godot_check.sh` | 36/38 通过，失败项同上；`DialogDrawTest`、`PictureRoundtripTest`、`SaveImportTest`、`GameRootTest` 全部通过。此前 `SaveImport`/`GameRoot` 的渲染失败已消除 |
| Rust Oracle 路线 | `149,655/149,655` 与 `989,439/989,439` 步 100% 一致 |

## 8. 残余缺口

### P1：CE270 消息窗与头像未由图片槽驱动

当前 `MessageWindow.open()/close()` 直接切换自绘面板可见性；CE270 的 640 宽动态气泡、脸图与立绘 `150` 图片槽没有被调度。具体缺口：

- `AutoEvents` 只调度 `run_condition==2`，CE270 的 `run_condition==3` 不会启动；
- `101` 只调用 `Host.ShowMessage`，没有写入 CE270 需要的 `Sys12` 与系统字符串 25；
- `9900025` 这类系统字符串目前按普通系统变量解析会越界，CE270 即使启动也读不到正文；
- 当前自绘气泡约为 712 宽，不是 CE270 的 640 宽动态布局。

这是后续最值得优先补的结构性动画缺口。CE63 的窗口分支在本作不可达，不应作为实现依据。

### 已核实：负数图片号的层级

官方手册规定：

- `-1..-99,999`：在地图之上、事件之下；
- `<= -100,000`：在地图之下、背景之上。

全部零售 `150` 中没有 `<= -100,000` 的槽位；`-1..-99,999` 与正数槽位在当前项目共用的 `LAYER_SCENE_PICTURE` 上，且本项目没有真实事件角色层，因此当前内容没有可观测的层级差异。若后续加入地图事件角色层再重新评估。

### P2：Free Transform

全量 `150` 的 Free Transform 标志为 0，零售数据未使用；当前不实现四角自由变形不构成本作内容缺口。

### 未验证：待触发队列的存档边界

`picture_snapshots()` 只保存当前可见槽位，不保存尚未到期的 Delay 命令。原版存档是否保留这些未来动作尚未取得直接证据，因此本轮不把 pending 队列写入存档，避免猜错格式。

### 已核实：`290 Picture Effect` 没有独立 Activation Delay

本作全部 810 条 `290` 均为固定 7 参布局 `[head, duration, lo, hi, v1, v2, v3]`，字段中不存在
`150` Show/Move/Erase 那样的 `Delay`。因此动作 3 的待触发取消范围只覆盖 `150` 的 Delay 队列；
`290` 的 `duration` 是效果自身的处理时间/间隔，不应被当作延迟命令取消。若后续版本引入带
`Activation Delay` 的 290 子型，需要重新按字节码扩展队列。

## 9. CI 报错邮件

本报告生成前的失败链已闭环：

- `45993864592`：Homebrew Python PEP 668；由 `7e4016f` 改用 venv 修复。
- `45995107834`：iOS SplashImage 原字节校验误判；由 `6b88aa1` 改为像素级校验修复。
- `CI` #57（run `45996401545`）与 `iOS Unsigned IPA` #3（run `45996796593`）在 `6b88aa1` 上均为成功。
- `CI` #58（run `46012905723`）在 `959da1f` 上成功；#60（run `46016008972`）在 `3e5df4e` 上成功。
- `CI` #59（run `46014749642`）曾在 `4f403be` 上失败，随后由 `3e5df4e` 修复并由 #60 覆盖；旧失败 run 的邮件可能延迟到达。

如果邮箱仍收到失败通知，优先检查是否为旧 run 的延迟通知；新提交后的状态应以新 run 为准。本轮提交前必须再核对新 push 的 CI，而不是只引用历史成功记录。
