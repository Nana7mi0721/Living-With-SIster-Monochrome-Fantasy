# S-01..S-08 存档界面、模态输入、自动存档、字体清晰度与格子进度条对齐修复报告

- **对照诊断基线**：[save_ui_font_and_gauge_diagnosis_2026_11_23.md](save_ui_font_and_gauge_diagnosis_2026_11_23.md)（Commit `78c4e49a`）
- **决策记录**：[docs/godot/DECISIONS.md](../godot/DECISIONS.md) `D-84`
- **日期**：2026-11-23

---

## 1. 修复总览（S-01 .. S-08 全部闭环）

| 编号 | 问题现象 | 修复方案与关键改动文件 | 回归验证 |
| :--- | :--- | :--- | :--- |
| **S-01** | 标题读档模式点击存档槽误弹“覆盖存档？” | `VmController.cs` / `GameSessionController.cs` 新增 `GetCommonEventSelf(eventId, argIndex)` 自动偏移 `Magic.SelfArgBase (600)`；`GetSaveSlotMode()` 改读 `cself[0]`（600）；`GameRoot.gd` 叠加 `phase == "title"` 双保险 | `GameRootTest.gd` / `SaveLoadTest.cs` |
| **S-02** | 游戏内跨页点击槽位 `save24` 弹窗误显示槽 04 | `GameRoot.gd` 改读 `GetCommonEventSelf(131, 10)`（`cself[10]`）、`11`（`cself[11]`）与 `27`（`cself[27]`，默认 `4`）计算跨页槽位号 | `GameRootTest.gd` / `SaveLoadTest.cs` |
| **S-03** | 点击存档槽或确认后无反应、未落盘且页面未刷新 | 移除 `_pending_save_overwrite_confirmed` 单帧脉冲黑客逻辑；`GameSession.cs` 新增 `SaveGameToSlot`（写 `save_NN.json`、同步 `System.json` 变量 `0/626/546`、调 `RefreshSaveUiForSlot` 更新 `CE131` 并并行触发 `CE94` 重绘）与 `LoadGameFromSlot`（经 `GameSessionController.cs` 暴露）；`GameRoot.gd` 直接写缩略图并调用 `SaveGameToSlot` / `LoadGameFrom` 并同步 `frames`/`phase` 与 Toast | `SaveLoadTest.cs` / `GameRootTest.gd` |
| **S-04** | 二次确认弹窗右键失效，按 `Esc` 连带退出整个存档页 | `PlayOverlays._input` 在 `is_modal_open()` 为真时优先拦截右键按下与非重复 `KEY_ESCAPE`，调用 `close_topmost_modal()` 并截断事件；`GameRoot._release_modal_guard_if_free()` 增加取消与确认键物理释放检测；`InputRouter.held_mask(false)` 在守卫期间掩掉 `MASK_OK \| MASK_CANCEL` | `InputRouterTest.gd` / `GameRootTest.gd` |
| **S-05** | 弹窗可读性差，缺少游戏内读档与空槽四象限分支；导入导出与回标题风格/位置不统一 | 抽取 `_make_modal_card_style` / `_style_modal_button` 统一「存档槽确认（560×240 四象限）」「返回标题确认（560×240）」「跨端导入导出（680×440）」三弹窗为暗灰蓝 1px 白框视觉风格；将 `SavePageBar`（「导入 / 导出存档」与「回到标题」）移至 `CE94` 自动保存提醒框（`y=100..300`）与预览缩略图（`y=358..502`）之间（`x=732..948, y=307..351`，避开右翻页箭头 `12005`） | `GameRootTest.gd` |
| **S-06** | 自动存档（`CE435`）在非 `"bedroom"` 阶段静默拒存风险 | `GameSession.cs` 抽取 `CanSaveMapState()`，在 `BuildSaveState()`、`SaveGameTo()` 与 `LoadGameFrom()` 回滚检查中仅拦截 `"title" / "boot" / "failed" / "newgame"` 或无地图态；`HandleSaveLoad` case 220 对 `Owner == 435` 输出 `[AutoSave] slot=N ok/fail` 状态日志 | `SaveLoadTest.cs` |
| **S-07** | HiDPI / 窗口缩放字体清晰度与 `Lolita.ttf` 路径探测 | `UiSkin.gd` 在 `_configure_font_file()` 设置 `oversampling = 0.0`、`FONT_ANTIALIASING_GRAY`、`HINTING_NORMAL`、`generate_mipmaps = false`、`multichannel_signed_distance_field = false`，扩展可执行目录 / `res_root` / `user://` 字体候选路径；`project.godot` 开启 `window/dpi/allow_hidpi=true` | `GameRootTest.gd` / `DialogDrawTest.tscn` |
| **S-08** | “警戒度”格子进度条 `■` 与 `□` 逐格左漂未对齐 | `PictureStage.gd` 为 `■□◆◇●○` 定义配对等宽步进 `_gauge_cell_step()`（取同组实心/空心字符最大 advance），统一用于 `_measure_text` 与 `PictureTextLabel._draw()` 并逐格居中绘制，混合非几何字符分段保持原样绘制 | `PictureStateTest.gd` |

---

## 2. 自动化门禁与回归验证结果

| 验证项 | 命令 | 结果 |
| :--- | :--- | :--- |
| Release & Debug 核心编译 | `dotnet build godot/Lwsmf.csproj -c Release && dotnet build godot/Lwsmf.csproj -c Debug` | 0 warnings, 0 errors |
| 无头 38 项核心门禁 + M13 + HD | `bash tools/godot_check.sh` | `GODOT_GATE_OK elapsed=111s`（149,655/149,655 与 989,439/989,439 Oracle 100% 一致；`Font=file:Lolita.ttf`） |
| OpenGL3 渲染回归门禁 | `LWsmf_RENDER=1 bash tools/godot_check.sh` | `GODOT_GATE_OK elapsed=106s`（`DIALOG_DRAW_TEST_OK (windowed)`、`PICTURE_ROUNDTRIP_TEST_OK (windowed)`、`PERFORMANCE_FRAME_STRICT_OK avg_ms=16.777 fps=59.6`） |
| 决策与文档链接检查 | `python tools/lint_decisions.py` | `DECISIONS_LINT_OK` |
| 隐私与资产泄露扫描 | `python tools/release_audit.py --repo-scan` | `REPO_PRIVACY_AUDIT_OK leaks=0` |
