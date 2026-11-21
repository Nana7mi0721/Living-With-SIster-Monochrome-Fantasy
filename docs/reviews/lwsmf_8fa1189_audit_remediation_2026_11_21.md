# `8fa1189` 两份体验审计的修复闭环报告（2026-11-21）

## 1. 范围与结论

本报告汇总并关闭基线 `8fa1189` 的两份审计：

- `lwsmf_8fa1189_gameplay_ux_and_hidden_bugs_report.md`
- `lwsmf_8fa1189_ui_parity_and_bug_audit_report.md`

原报告从会话归档恢复后用于逐项核对，不在本仓库重复分发。修复以源码、数据取证和自动化回归为准；
没有启动原版游戏、Godot 编辑器或任何前景窗口。结论是两份报告列出的 P0/P1/P2 均已有实现或明确
的语义结论，仍保留的人工验收边界见 §5。

## 2. 逐项闭环

### 2.1 P0：剧情、输入与存档

| 原问题 | 处理结果 | 回归证据 |
| --- | --- | --- |
| `102 bit4` 禁止取消未透传，强制选项可被 Esc/右键取消 | VM 把 `allow_cancel` 传给宿主；选择窗口拒绝取消；存档保存并从旧档恢复该标志 | `VmTest.TestOpcode102CancelFlag`、`SaveLoadTest` 不可取消选择读档断言、`GameRootTest` |
| 空格/取消/滚轮跳过未读台词 | 空格改为确认；取消保持未读行；滚轮不再确认消息，向上滚轮打开回顾 | `InputRouterTest`、`GameRootTest` |
| 确认选择后同一次按键穿透到下一个 `123` 等待轮询（Tips 教学图秒关） | 消息/选择确认关闭后，若决定输入仍按住，屏蔽 `123`/`124` 的 OK 位直到物理释放 | `GameRootTest` 键盘选择守卫、`InputRouterTest` 释放恢复 |
| 发布包父亲序章后静默卡死 | 失败态进入可见故障弹窗；未捕获异常写 `user://crash.log`；转场/模态故障时清理 | `GameRootTest`、`GameSessionController` 回归 |
| 读档后存档 UI 与灰黑文本框残留 | 保存时过滤 `12000..12599` 及 `CE14/94/131` 调用链；读档只恢复仍有活跃等待的对话，并清空回顾/选项/转场浮层 | `SaveLoadTest` 场景图/存档 UI 过滤与读档恢复断言 |
| 存档栏点击无反应或需连点 | 保留物理按住态，单帧锁存只消费一次；模态/子事件不再吞掉下一次独立点击 | `GameRootTest`、`InputRouterTest` |
| 道具面板、通知 Tips 点击穿透 | 子事件进入时若打开它的动作仍按住，抑制到释放；通知 Tips 同路径覆盖 | `VmTest` 事件进入通知、`GameRootTest` CE128 与选择守卫 |

### 2.2 P1：画面、文本与音频

| 原问题 | 处理结果 | 回归证据 |
| --- | --- | --- |
| `151` 场景变色/闪光只影响地图或负号图片 | 改为整屏乘法染色层；地图保持中性避免双染；HUD/系统菜单提升到染色层之上 | `LayerOrderTest`、`PictureStateTest` |
| `blend == 3` 正片叠底无法半透明淡出 | 保留 `modulate.a`，不再强制 alpha=1；正向提亮不再钳死在 1.0 | `PictureStateTest`、`PictureEffects` 回归 |
| 对话框/居中框/选项框边框、行高、位置偏差 | 使用九宫格与 `UiSkin` 字体；选项行高 27px；显式坐标与 `-1` 居中哨兵分离；`101+102` 同屏 | `DialogDrawTest`、`GameRootTest`、`VmTest.TestOpcode101PairedChoice` |
| 字体发灰、人名牌不可读、`\-[n]` 字距码泄露 | 统一粗体/描边回退；名牌提高对比；文本 tokenizer 解析字距码且只画纯文本 | `DialogDrawTest`、`TextTokenizer` 回归 |
| 文本速度过快、`\.`/`\|`/`\sp[n]` 未接入 | 接入逐字速度、停顿、继续等待、速度覆盖与中文避头尾换行 | `DialogDrawTest`、`MessageWindow` 回归 |
| 单通道 SE 互斥、延迟误当淡入、WAV 无 `loop_end`、同曲 BGM 重播 | SE 六声部池；延迟独立；WAV `loop_end` 补齐；同曲 BGM 保持播放位置 | `AudioManagerTest` 45 checks |
| 卧室阶段隐形移动主角坐标 | 卧室地图移动入口关闭，只保留事件坐标系与朝向逻辑 | `GameRootTest` 方向键/移动断言 |
| 标题 F12 误弹确认、F4 回顾缺方向键 | 标题阶段不打开放弃确认；F4 回顾统一字体并支持方向键翻页 | `GameRootTest`、`DialogDrawTest` |

### 2.3 P0/P1：数据与语义修正

| 原问题 | 处理结果 | 回归证据 |
| --- | --- | --- |
| `101+102` 是否同屏未验证 | 实机截图与 CE323 pc44/45 证明同屏；相邻 `101→102` 不再先等消息关闭 | `VmTest.TestOpcode101PairedChoice`、`GameRootTest` |
| `124` picture field 2/3 被怀疑是原图尺寸 | 确认为横向/纵向缩放百分比；CE63/CE94 直接写入 `150` 缩放槽；不再返回素材宽高 | `PictureStateTest` 114 checks、两份路线 Oracle |
| 存档界面 Q 版徽标异常放大 | 旧实现把 picture field 2/3 的像素宽高写回 CE94 的 `150` 缩放槽，导致徽标按 `~190%` 绘制；改为返回当前缩放百分比后保持 100% | `PictureStateTest` field 2/3 断言 |
| `150 scale == 0` 被误改为 100% | 保留 0%；CE216 pc34 的 AP 条空值必须得到空槽，CE63 注释只适用于头像缩放设置 | `PictureStateTest`、`GameRootTest` |
| Tips 13 张教学图素材缺失 | `media.pak` 实含 `cg/tips_00.png`–`tips_12.png`；`AssetDB.resolve()` 全部命中；不再提交零售素材，只修路径/点击穿透 | `GameRootTest` 零售资源根逐项断言 |
| `150 kind=1/4` 动态字符串路径未展开/缺系统字符串 | 统一走 `EvalStr` + `Expand`，支持自变数、普通字符串和系统字符串 | `VmTest.TestOpcode150DynamicFile` |
| `1000 ProFeature` 存档缩略图未写/删 | 传递目标路径；kind 0 写 JPEG、kind 3 删除；系统 UI 打开前抓干净场景图 | `GameRootTest` 缩略图写入/解析/删除 |

## 3. 本轮自动化结果

| 项目 | 结果 |
| --- | --- |
| Release 构建 | `0` 警告、`0` 错误 |
| 无头完整门禁 | 38/38，`GODOT_GATE_OK elapsed=67s` |
| 418 帧逐指令夹具 | `2461/2461` 步，100% |
| 主路线 Oracle | `149655/149655` 步，1500 帧，100% |
| M4 全路线 Oracle | `989439/989439` 步，8048 帧，100% |
| 定向表现层回归 | `GAME_ROOT_TEST_OK`、`INPUT_ROUTER_TEST_OK`、`PICTURE_STATE_TEST_OK`、`DIALOG_DRAW_TEST_OK`、`SAVE_LOAD_TEST_OK`、`AUDIO_MANAGER_TEST_OK` |

## 4. 取证说明

- `101+102` 同屏截图包含零售画面，只在本轮作为本地审计证据，不纳入 Git 分发；结论文本与
  CE323 pc44/45 的数据形态足以复核该语义。
- `media.pak` 的 13 张 Tips 图通过包索引逐项核对；未将 `work/retail_dat/`、`work/all/` 或任何零售
  素材加入提交。
- `124` 与 `150 scale` 的结论来自 `game_data/programs.json` 的 CE63/CE94/CE216 指令流、YADOT
  对「画像サイズ / 拡大率」的区分，以及 `PictureStateTest` 的字段回归。

## 5. 未执行项与边界

- 本轮没有执行 `LWsmf_RENDER=1 bash tools/godot_check.sh`，因此没有重新生成 OpenGL3 截图或
  像素级渲染证据；前一轮构建/无头门禁的结论已由本报告替代。
- 没有启动原版游戏或前景 Godot 窗口，也没有进行完整人工通关、干净目录双击启动和发布包实机验收。
- 正式存档仍只允许在卧室行走地图阶段执行；非该阶段由 `SaveGameTo` 拒绝并写状态栏，不把标题/特殊
  事件中的临时状态伪装成可安全恢复的存档，因此没有扩大为“任意阶段都可保存”。
- 路线测试宿主的 `124` field 3 仍沿用夹具生成时的简化值；生产 `PictureStage.gd` 已使用独立
  `scale_y`，并由 `PictureStateTest` 覆盖。若未来需要让路线 Oracle 也验证独立纵向缩放，必须先同步
  Rust/C# 路线宿主并重新生成两份夹具，不能只改一侧。
