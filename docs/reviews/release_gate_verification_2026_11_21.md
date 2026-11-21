# 修复验证与无头门禁报告（2026-11-21）

验证对象为基线 HEAD `8fa1189` 的工作树（含本轮修复）。本报告只记录本轮实际执行的构建、
无头门禁与 Oracle 对拍结果；按用户要求，没有启动原版游戏、Godot 编辑器或任何前景窗口。

## 1. 结果摘要

| 检查 | 实测结果 |
| --- | --- |
| C# Release 构建 | `0` 个警告、`0` 个错误 |
| 无头 38 项门禁 | `GODOT_GATE_OK elapsed=67s` |
| 418 帧逐指令夹具 | `TRACE_DIFF_OK: 2461/2461 steps matched 100%` |
| 主路线 Oracle | `ROUTE_DIFF_OK: 149655/149655 steps matched 100%`（1500 帧） |
| M4 全路线 Oracle | `ROUTE_DIFF_OK: 989439/989439 steps matched 100%`（8048 帧） |
| 无头帧循环探针 | `PERFORMANCE_FRAME_OK frames=300 avg_ms=6.955 p95_ms=6.944 fps=143.8` |
| OpenGL3 渲染门禁 | **本轮未执行**（按用户要求不启动前景窗口） |

## 2. 已验证修复

### 2.1 前景事件进入边界与子事件点击抑制

- [EventVm.cs](../../godot/core_cs/EventVm.cs) 对每个开始执行首个可运行槽位的前景实例只发布一次 `EventEntered(eventId, isRoot)`；并行实例不发布，`isRoot` 由是否存在父实例决定。
- [GameRoot.gd](../../godot/presentation_gd/GameRoot.gd) 仅在子事件进入且打开它的动作仍在按下/锁存时启用动作抑制；[InputRouter.gd](../../godot/presentation_gd/InputRouter.gd) 在抑制期间同时屏蔽 `123`/`124` 的 OK 位，并在物理释放后恢复输入。
- 该边界消除了“点击打开道具列表后，同一次点击立刻把子菜单关闭”的穿透路径，同时不影响根事件进入、普通帧输入或释放后的下一次点击。
- [VmTest.cs](../../godot/tests/VmTest.cs)、[InputRouterTest.gd](../../godot/tests_gd/InputRouterTest.gd) 与 [GameRootTest.gd](../../godot/tests_gd/GameRootTest.gd) 分别覆盖一次性事件通知、root/child/parallel 区分、子事件抑制和释放恢复。

### 2.2 输入、选项与对话

- Opcode `102` 的 `bit4` 取消许可已透传至选择窗口；禁止取消的选项会忽略右键、Esc 等取消动作。
- 空格恢复为确认键，不再与取消键混用；对话中的取消与滚轮不会跳过未读文本，模态关闭后会等待物理释放，避免点击穿透到底层地图或图片热区。
- 消息或选择被键盘/鼠标确认关闭后，若决定输入仍按住，[InputRouter.gd](../../godot/presentation_gd/InputRouter.gd) 会把 OK 位屏蔽到物理释放；这覆盖“确认 Tips 选项后同一按键立刻关闭教学图”的路径。
- [MessageWindow.gd](../../godot/presentation_gd/MessageWindow.gd) 已接入逐字速度、`\.` 停顿、`\|`/`\!` 继续等待、语音触发和中文避头尾换行；[ChoiceWindow.gd](../../godot/presentation_gd/ChoiceWindow.gd) 的取消权限、尺寸、行高、显式坐标和 `-1` 居中哨兵均有回归覆盖。

### 2.3 存读档与故障可见性

- [GameSession.cs](../../godot/core_cs/GameSession.cs) 在构建存档时排除 `12000..12599` 存档界面图片以及 `CE14/94/131` 及其子实例；读档后清理临时图片、对话、选项、回顾和转场浮层，再仅在存在活跃消息/选择等待时恢复对话。
- 读档失败回滚时不再重新留下存档界面或灰色文本框；`1000` ProFeature 的目标路径已传入宿主，用于存档缩略图的写入/删除。
- [GameSessionController.cs](../../godot/presentation/GameSessionController.cs) 将未捕获异常写入 `user://crash.log` 并触发可见故障态，避免发布包静默卡死。

### 2.4 音频与画面

- [AudioManager.gd](../../godot/presentation_gd/AudioManager.gd) 改为 BGM/BGS 独立长音轨加 6 路 SE 声部池；SE 延迟按起始延迟处理，WAV 循环补齐 `loop_end`，同曲 BGM 重入不再从头播放。
- [PictureStage.gd](../../godot/presentation_gd/PictureStage.gd) 使用独立整屏染色层，HUD/系统菜单槽位提升到染色层之上；[PictureEffects.gd](../../godot/presentation_gd/PictureEffects.gd) 修正正向提亮上限与正片叠底淡出语义。

### 2.5 提示语、缩放字段与 Tips 素材

- 相邻 `101 → 102` 改为同屏：`101` 的文本作为选择提示保留，`102` 负责阻塞选择；CE323 pc44/45 与本地实机截图一致，不再先要求关闭消息窗口。详见 [OPCODE_102.md](../engine/OPCODE_102.md) §2.2。
- Opcode `124` 的 picture field 2/3 已确定为横向/纵向缩放百分比，直接对应 `150` 的缩放槽；不再返回原图像素，也不把 `scale == 0` 全局改成 `100`。详见 [OPCODE_124.md](../engine/OPCODE_124.md) §5.1/§9.2。
- `work/all/media.pak` 索引核对到 13 张 `cg/tips_00.png`–`cg/tips_12.png`；`AssetDB.resolve()` 对全部条目返回非空。`GameRootTest` 在零售资源根下会逐个断言，缺包时直接失败。

## 3. 门禁证据

| 命令 | 关键输出 |
| --- | --- |
| `dotnet build godot/Lwsmf.csproj -c Release` | 构建成功；`0` 警告、`0` 错误 |
| `bash tools/godot_check.sh` | 38/38 通过；`TRACE_DIFF_OK: 2461/2461`；`ROUTE_DIFF_OK: 149655/149655`；`ROUTE_DIFF_OK: 989439/989439`；`GODOT_GATE_OK elapsed=67s` |
| `LWsmf_RENDER=1 bash tools/godot_check.sh` | 本轮未执行 |

## 4. 边界

- 本轮只执行无头门禁；它验证自动化构建、Oracle 一致性、存档/事件状态机与输入回归，不替代渲染像素校验、完整人工通关或干净目录双击启动验收。
- 本次未生成发布包，也未执行干净目录双击启动验收。
- 文档收口移除了已被当前实现取代的审计草稿；历史审查记录仍保留，但现状以本报告与 [DECISIONS.md](../godot/DECISIONS.md) 为准。
