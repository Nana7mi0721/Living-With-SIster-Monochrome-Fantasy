# 原版素材出现／消失动画审计与 `150 Picture` 语义修复（2026-11-27）

> **范围**：550 个公共事件与 9 张地图事件中的全部 `150 Picture` 指令；重点核对 CG、立绘、Cutin、战斗特效、消息窗、文本图、系统窗口、DLC、标题与 H 场景素材的出现／消失时序。
> **证据基准**：零售数据导出的 `game_data/programs.json`、WOLF RPG Editor 3.738 官方手册、`wolfrpg-map-parser` 字段解析与仓库内既有 Oracle。截图只用于最终视觉验收，不参与动画语义推断。
> **结论等级**：下文“已验证”表示有字节码字段或官方手册逐条支撑；“残余”表示当前实现仍未覆盖或尚缺直接证据。

## 1. 结论摘要

1. **主根因不是素材缺失，而是 `150` 的三个时间字段被混用**：旧实现把 `Process Time` 当成可回退到 `animate[0]` 的动画时长，又把动作 `3` 当成“隐藏/删除槽位”，并且完全没有执行 Show/Move/Erase 的 `Delay`。这三处会让大量素材提前、延后或永久消失。
2. **动作 `3` 的官方名称是 `Delay Reset`**，只取消尚未开始的延迟动作，不改变当前已显示的图片。旧实现会隐藏并删除槽位，造成打开菜单、切场景时素材被误删。
3. **Show 对已占用槽位等同 Move**，`Process Time >= 1` 时从透明开始；同一个 graphic 的 Show 保留槽位效果，换成另一张 graphic 则是新图，不继承旧图的闪光/抖动状态。
4. 本轮已补齐 Show/Move/Erase 的延迟队列、范围 Delay Reset、Erase 的短格式 Delay、占用槽 Show 的 Move 语义，以及 `Angle` 的渲染与 `Process Time` 插值。
5. **仍有一个高优先级结构缺口**：CE63 的消息窗／头像时间线由 `150` 图片槽驱动，但当前 Godot 使用原生 `MessageWindow` 自行开关和布局；原版的窗口滑入、避让和头像移动尚未接到图片槽。其余残余见第 8 节。

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

### 5.2 CE63：消息窗时间线

- `pc90`：Show 窗口皮肤，`Process Time=5`。
- `pc93`：Move，`Process Time=5`、Delay 0。
- `pc94`：同一目标 Move，`Process Time=5`、Delay 3。
- `pc101`：Delay Reset，不删除窗口。
- `pc102/103`：Move，`Process Time=4`、Delay 0/2。
- `pc160–162`：Move，`Process Time=5`、Delay 0/3/5。

这些指令的设计意图是同一帧发出多条错峰动作；旧实现忽略 Delay，三条 Move 会同时抢到终态。

### 5.3 CE64 / CE85：错峰展开与滑动

- CE64 `pc315–318`：通过纵向 ScaleY 0→100、`Process Time=5`、`Delay=0/3/5` 做窗口展开。
- CE85 `pc200–202`：同一目标连续 Move，`Process Time=5`、`Delay=0/3/5`，形成分段追赶。

### 5.4 CE94 / CE426 / CE301：文本、CG 与战斗特效

- CE94 `pc214–216`：文本图同目标 Move，`Process Time=5`，Delay 为 self96/97/98。
- CE426 `pc384`：`cg/whitescreen.jpg` Show，`Process Time=200`；`pc488` 立绘 Show 20 帧；`pc491` Erase 20 帧。
- CE301 `pc535`：战斗特效 Show 10 帧；`pc544` Erase `Process Time=100`、`Delay=200`。

## 6. 本轮代码修复

### 6.1 `godot/presentation_gd/PictureStage.gd`

- 新增全局待触发队列 `_pending_pictures`：Show/Move/Erase 的 Delay 到期后才按原始命令重放。
- 动作 `3` 改为 `_reset_delayed_pictures()`：按单槽或闭区间取消 pending，不隐藏当前图层。
- Show/Move 读取 `args[11]` 的 Delay，Erase 读取 `args[1]` 的 Delay。
- `_transition_duration()` 只读取 `args[0]`，移除把 `animate[0]`（实际是 Delay）当后备时长的错误。
- 占用槽 Show 使用 Move 语义；`Process Time >= 1` 时 alpha 从 0 开始，同一 graphic 保留 290 效果，换成另一张 graphic 时不继承旧图的闪光/抖动。
- `Angle=args[8]` 进入姿态、插值与 `Control.rotation`；旋转中心为图片中心。
- `has_active_animations()`、`clear_pictures()`、`restore_pictures()` 同步处理待触发队列。
- `track_picture()` 与 `draw_picture()` 使用相同的 Delay/Delay Reset 语义，避免路线追踪模式与实机模式分叉。

### 6.2 `godot/presentation/VmController.cs`

- 只有 `PictureAction.Erase` 从 `_pictures` 移除槽位。
- `PictureAction.Hide`（线值 3）不再删除 slot，只交给 `PictureStage` 执行 Delay Reset。
- Erase 范围终点固定读取 `Args[3]`；Delay Reset 不再误读 Erase 字段。

### 6.3 `godot/core_cs/EventVm.cs` / `IHost.cs`

- 保留 `PictureAction.Hide` 枚举名以兼容轨迹夹具，但注释明确其 wire 语义为 Delay Reset。
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
| `python tools/run_godot_test.py res://tests_gd/PictureStateTest.tscn --timeout 90` | `PICTURE_STATE_TEST_OK checks=225` |
| `bash tools/godot_check.sh` | 36/38 通过。仅 `Smoke`（`headless fixture must run four command rounds, got 3`）与 `M13 slot 1`（180s 超时，`stage=10 has305=false v492=1`）失败；两者均指向本机 `work/retail_backup/Save/SaveData01.sav` 被真实进度覆盖（387,265 B，测试基线 366,146 B），非本轮改动引入 |
| `LWsmf_RENDER=1 bash tools/godot_check.sh` | 36/38 通过，失败项同上；`DialogDrawTest`、`PictureRoundtripTest`、`SaveImportTest`、`GameRootTest` 全部通过。此前 `SaveImport`/`GameRoot` 的渲染失败已消除 |
| Rust Oracle 路线 | `149,655/149,655` 与 `989,439/989,439` 步 100% 一致 |

## 8. 残余缺口

### P1：CE63 消息窗与头像未由图片槽驱动

当前 `MessageWindow.open()/close()` 直接切换自绘面板可见性；CE63 的 `150` 窗口 Show/Move/Delay 时间线仍在 `PictureStage` 中执行，但没有驱动原生消息窗。结果是：

- 窗口从屏下 96 px 滑入、向上避让等动作缺失；
- 头像与窗口之间的相对位移缺失；
- 原生消息框与图片槽可能重复绘制或层级不一致。

这是后续最值得优先补的结构性动画缺口。

### P2：负数图片号的层级

官方手册规定：

- `-1..-99,999`：在地图之上、事件之下；
- `<= -100,000`：在地图之下、背景之上。

当前 `PictureStage` 将所有场景图片放在同一 `LAYER_SCENE_PICTURE`。零售剧情大量使用全屏正数图片，短期不一定暴露，但 H 场景与地图背景层仍可能出现层序偏差。

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

如果邮箱仍收到失败通知，优先检查是否为旧 run 的延迟通知；新提交后的状态应以新 run 为准。
