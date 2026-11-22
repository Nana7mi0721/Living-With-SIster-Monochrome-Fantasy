# 《妹相随》Godot 重制版底层 8 条管线排雷与疏通施工单（基线 `c91dc88`）

> 适用对象：开发负责人 / 编码 Agent。  
> 核心原则：**不动已通过百万步 Oracle 对拍的 C# 核心（`EventVm` 算术/控制流、`MapRuntime`、`BattleWiring`、`AutoEvents`）**；按 8 条外围与桥接管线分阶段疏通。  
> 纪律：**每完成一个阶段，必须同时通过“无头回归 + 真窗口手点验收”才能进入下一阶段**，禁止多条管线混改导致回归定位困难。

---

## 阶段总览与执行顺序

| 阶段 | 对应管线 | 优先级 | 核心目标 | 涉及主要文件 |
| --- | --- | --- | --- | --- |
| **Phase 1** | 键鼠输入与模态路由 | **P0** | 根治存读档连点无反应、道具列表闪现消失、失焦吞键、F4/F12 叠盖 | `InputRouter.gd`, `VmController.cs`, `GameRoot.gd`, `PlayOverlays.gd` |
| **Phase 2** | 素材与字体加载 | **P0** | 根治 `Lolita.ttf` 静默回退微软雅黑、打包漏字体、缺图不可见 | `UiSkin.gd`, `AssetDB.gd`, `tools/package_release.sh` |
| **Phase 3** | 图片绘制与特效状态 | **P0** | 根治存档栏光标闪烁、存档 `save2/3` 黑影、补齐活跃槽位诊断 Dump | `PictureStage.gd`, `PictureEffects.gd`, `GameRoot.gd` |
| **Phase 4** | 存读档与瞬态清理 | **P0** | 根治读档灰黑残框、夜晚读白天档染色串态、居中框串态、SE 误入档 | `GameSession.cs`, `GameRoot.gd`, `AudioManager.gd` |
| **Phase 5** | 文本排版与对话/人名牌 UI | **P1** | 还原原版平滑羽化对话框、双层胶囊人名牌、2–3px 描边与 `@<n>` 多位头像 | `MessageWindow.gd`, `Nameplate.gd`, `UiSkin.gd`, `EventVm.cs` |
| **Phase 6** | 音频时钟与通道语义 | **P2** | 补齐 WAV `loop_begin` 钳制、核实 `soundType 3..7`、真驱动压测 | `AudioManager.gd`, `EventVm.cs` |
| **Phase 7** | 窗口、缩放、DPI 与时钟 | **P2** | 核查 125%/150% DPI 与三种缩放下鼠标热区精准度、模态暂停打字机 | `GameRoot.gd`, `InputRouter.gd`, `MessageWindow.gd` |
| **Phase 8** | 发布打包、向导与 CI | **P2** | 消除 `SetupWizard` 括号路径炸裂、CI `cargo fmt` 变绿、干净目录发包冒烟 | `SetupWizard.gd`, `tools/package_release.sh`, `.github/workflows/ci.yml` |

---

## Phase 1：键鼠输入与模态路由管线（P0）

### 施工任务
1. **恢复 `held_mask` 对持续按住左键的识别（修存读档难点击）**
   - 文件：`godot/presentation_gd/InputRouter.gd:164-171`
   - 改动：将 `if _click_latch:` 改为 `if pointer_pressed or _click_latch:`（保留下方 `if _action_entry_suppressed: mask &= ~MASK_OK` 兜底）。
2. **移除先手 `124` 误杀 `123` 的反向清空**
   - 文件：`godot/presentation_gd/GameRoot.gd:332-334`、`godot/presentation/VmController.cs:789`
   - 改动：取消 `_on_pointer_press_observed()` 中调用 `router.clear_click_latch()` 的逻辑，统一交由抑制守卫防穿透。
3. **修复 `_on_event_entered` 单帧失效（修道具列表闪现消失）**
   - 文件：`godot/presentation_gd/GameRoot.gd:321,338-343`
   - 改动：移除 `if is_root or not _action_input_pending` 的单帧与根事件限制；只要任意前景事件首次进入可执行槽位且 `router.action_input_held()` 为真，立即调用 `router.suppress_action_until_release()`，并同步清除 `vm` 当帧 `_pointerPressFrame`。
4. **切窗失焦与卧室残留点击清理**
   - 文件：`godot/presentation_gd/InputRouter.gd:229-253`、`godot/presentation_gd/GameRoot.gd:747-759,794-811`
   - 改动：`InputRouter` 新增 `clear_suppression()` 并在 `GameRoot._notification(FOCUS_OUT)` 中调用；关闭 `_on_map_cell_clicked` 的卧室 `MoveLeader` 入口。
5. **F4 / F12 模态互斥与回顾长按连发**
   - 文件：`godot/presentation_gd/GameRoot.gd:928-938`、`godot/presentation_gd/PlayOverlays.gd:218-237`
   - 改动：F4 与 F12 互检对方未打开；`PlayOverlays._input` 对 `KEY_UP/DOWN/W/S` 放行 `key.echo`。

### 验收标准
- **自动化**：`INPUT_ROUTER_TEST_OK`、`GAME_ROOT_TEST_OK`（新增：按住左键进前景事件抑制、失焦清抑制、F4/F12 互斥断言）。
- **真窗口手点**：① 卧室点“道具”图标，道具列表稳定保持显示，再次点击才关闭；② 存档/读档界面单次点击存档栏 100% 一次响应；③ 切窗松键回窗不吞点击；④ F4 回顾长按方向键顺滑连滚。

---

## Phase 2：素材与字体加载管线（P0）

### 施工任务
1. **根治 `media.pak` 加载 `Lolita.ttf` 静默回退微软雅黑**
   - 文件：`godot/presentation_gd/UiSkin.gd:191-205`
   - 改动：放弃 `ff.data = pak_bytes` + `if ff.get_face_count() > 0`；当从 `media.pak` 读出字体字节时，写入 `user://cache/Lolita.ttf`（仅在缺失或字节数变化时重写），再调用 `ff.load_dynamic_font(cache_path) == OK`。
2. **字体加载可观测性与中文字形验真**
   - 文件：`godot/presentation_gd/UiSkin.gd`、`godot/presentation_gd/PlayOverlays.gd`（F3 Debug HUD）
   - 改动：记录字体实际来源（`loose` / `pak_cache` / `SYSTEM_FALLBACK`），启动时打印一次日志并抽样检测 `has_char("我") / has_char("妹") / has_char("存")`，在 F3 HUD 显示当前生效字体与 `[ASSET_MISS]` 总计数。
3. **打包脚本大小写不敏感查找字体并加硬检查**
   - 文件：`tools/package_release.sh:163-167`
   - 改动：同时查找 `work/all/Lolita.ttf`、`work/all/LOLITA.TTF`、`work/all/Data/Lolita.ttf`；若散文件与 `media.pak` 均不含字体，直接报错退出（非静默放行）。

### 验收标准
- **自动化**：`DIALOG_DRAW_TEST_OK`、`ASSETDB_TEST_OK`。
- **真窗口验证**：启动日志明确打印 `Lolita.ttf` 命中路径且非 `SYSTEM_FALLBACK`；进入标题与卧室后，肉眼核对对话与 HUD 为原版卡通圆体（若 `Lolita.ttf` 缺失部分简体字，立即记录缺失样本用于决定回退策略）。

---

## Phase 3：图片绘制与特效状态管线（P0）

### 施工任务
1. **修复存档栏光标“闪烁”问题**
   - 文件：`godot/presentation_gd/PictureStage.gd:1000-1005`、`godot/presentation_gd/PictureEffects.gd:43-49`
   - 改动：
     - `PictureStage._begin_draw` 在 `redraws && same_position` 分支中保留 `prev["color_corr"]` 与 `prev["effects"]`（除非新 `150` 显式重写色调）；
     - `PictureEffects.tone_color` 对 `kind == 0`（单次 `FLASH`）在 `age >= duration` 后结束并清理，不再按 `age % duration` 无限取模循环。
2. **增加一键活跃槽位表 Dump 诊断（秒定位黑影/残框/位置偏移）**
   - 文件：`godot/presentation_gd/PictureStage.gd`、`godot/presentation_gd/GameRoot.gd`
   - 改动：新增调试热键（如 `F6` 或 F3 附带命令）输出当前所有活跃槽位的 `number | file | kind | anchor | blend | tone | opacity | rect` 到控制台与 `user://picture_dump.txt`。
3. **排查并修复存档界面 `save2/3` 黑影与缩略图异常**
   - 文件：`godot/presentation_gd/PictureStage.gd`、`godot/presentation_gd/GameRoot.gd:654-696`
   - 改动：
     - 用上述 Dump 查明覆盖在 `save2/3` 左侧的槽位号与 `blend == 3` 材质归属；
     - 当 `file` 为 `save/*.jpg` 且文件尚未存在时，渲染为**完全透明**而非不透明深色 `placeholder_color` 占位框（避免在 `blend=3` 下把卡片与文字乘黑）；
     - 核对 `save_dir` 中缩略图写入文件名与 `CE94` 读取路径完全一致。

### 验收标准
- **自动化**：`PICTURE_STATE_TEST_OK`、`PICTURE_ROUNDTRIP_TEST_OK`（`LWsmf_RENDER=1` 窗口化）。
- **真窗口手点**：打开存档/读档界面，4 个存档栏无左侧黑影、`save2/3` 文字清晰纯白、光标悬停时稳定高亮不闪烁、已存槽位左下角正确显示干净场景缩略图。

---

## Phase 4：存读档与瞬态清理管线（P0）

### 施工任务
1. **抽离唯一的瞬态表现重置函数（修染色与居中框串态）**
   - 文件：`godot/presentation_gd/GameRoot.gd:847-859,1169-1204,1322-1403`
   - 改动：抽公共 `_reset_presentation_transient_state()`，统一执行：
     - `stage.reset_screen_tint()`
     - `_active_speaker = ""` + `nameplate.hide_speaker()`
     - `_active_msg_pos_mode = MSG_POS_BOTTOM` + `message.set_position_mode(MSG_POS_BOTTOM)`
     - `_clear_transition_overlays()` + `router.clear_suppression()`
   - 在 `_on_load_completed`、`restart_title`、`restart_after_fault`、`recover_to_last_save` 全部调用；`restart_after_fault` 补调 `_seed_live_rng_if_windowed()`。
2. **彻底清除读档后中央灰黑残框**
   - 文件：`godot/core_cs/GameSession.cs:85-86,567,691,702`
   - 改动：结合 Phase 3 的 Dump 或直接查看当前 `save_02.json` 的 `pictures` 数组，拿到中央 `kind=4` 灰黑框的精确槽位号；将其纳入 `IsSaveUiPicture` 过滤（存、读两侧同步过滤，确保旧存档读入也能立即清除）。
3. **停止将瞬态 SE 写入存档**
   - 文件：`godot/presentation_gd/AudioManager.gd:182-201`
   - 改动：`snapshot()` 仅遍历 `[CHANNEL_BGM, CHANNEL_BGS]`，跳过 `CHANNEL_SE`。

### 验收标准
- **自动化**：`SAVE_LOAD_TEST_OK`、`GAME_ROOT_TEST_OK`（新增：染色 + 居中框下读档复位断言）、`AUDIO_MANAGER_TEST_OK`（新增：SE 不入 snapshot 断言）。
- **真窗口手点**：① 夜晚/染色场景按 F9 读白天档，画面立刻恢复正常白底；② 居中旁白框读档后普通对话回到底部；③ 读档后卧室中央零残框、零凭空 SE 声。

---

## Phase 5：文本排版与对话/人名牌 UI 管线（P1）

### 施工任务
1. **对话气泡升级为平滑羽化 Shader**
   - 文件：`godot/presentation_gd/MessageWindow.gd:40-42,445-447`
   - 改动：用 2D Rounded-Box SDF 羽化 Shader 替代 4 层离散 `StyleBoxFlat`（中心 `rgba(0.10,0.10,0.11,0.80)`，边缘平滑羽化 `12–14px`，圆角 `14px`）。
2. **对话文字描边加粗**
   - 文件：`godot/presentation_gd/UiSkin.gd:13`、`MessageWindow.gd:455`
   - 改动：对话文字描边提升至 `2`（或 `3`）px，贴合原版特写的高对比锐度。
3. **人名牌改双层全圆角胶囊并去黑描边**
   - 文件：`godot/presentation_gd/Nameplate.gd:7-67`、`GameRoot.gd:518`
   - 改动：外层 `2px` 纯黑边框 + 中层 `3px` 深灰环 + 内芯中浅灰（`rgb(0.55,0.55,0.57)`），`corner_radius = height / 2`；去掉文字纯黑 `draw_string_outline`；左缘微调至气泡左上角外伸位置。
4. **修复多位数 `@<n>` 头像解析 & 核实文档“语音触发”**
   - 文件：`godot/core_cs/EventVm.cs:664-668`、`docs/reviews/release_gate_verification_2026_11_21.md:32`
   - 改动：`EventVm` 改为连续数字扫描 `@<n>`，与 `MessageWindow.gd:97-102` 一致；核实 `101` 是否存在内嵌语音码并同步修正文档或补齐实现。
5. **道具列表位置校准**
   - 依赖 Phase 2 真字体完成后，用 Phase 3 的槽位 Dump 对照原版 `032_日常_菜单_道具列表.png`（`x≈108, y≈148, w≈460, h≈390`）校准 `CE128` 窗口/文本锚点。

### 验收标准
- **自动化**：`DIALOG_DRAW_TEST_OK`（`LWsmf_RENDER=1`）、`VmTest` 新增 `@10` 多位数头像用例。
- **真窗口视觉对照**：与 4 张原版特写图及 `032_日常_菜单_道具列表.png` 并排比对，确认气泡平滑无台阶、人名牌双层胶囊白字清晰、道具列表位置贴合原版。

---

## Phase 6：音频时钟与通道语义体检（P2）

### 施工任务
1. `AudioManager.gd:285-287` 将 `wav.loop_begin` 钳制为 `clampi(begin, 0, total_samples - 1)`。
2. 扫描零售 `game_data/programs.json` 中全部 `opcode 140` 的 `soundType` 分布；若存在 `3..7`，决定 `EventVm.cs:932-936` 是否对齐 Rust 落到 `Se`。
3. 实机 Windows 音频驱动下测试 BGM/BGS 淡入淡出、循环无缝性与高频 SE 连播无爆音/泄漏。

---

## Phase 7：窗口、缩放、DPI 与帧时钟体检（P2）

### 施工任务
1. 在 Windows 125% 与 150% DPI 缩放下，分别切换 `F8`（`Integer` / `Fit-Nearest` / `Fit-Linear`）与 `F11` 全屏，手测卧室道具/存档按钮、选项框与黑边区域点击坐标无偏移。
2. 确认模态弹窗（F4/F12）打开时，`MessageWindow._process(delta)` 暂停推进打字机，关闭弹窗后恢复。

---

## Phase 8：发布打包、首次运行向导与 CI 收口（P2）

### 施工任务
1. **修 `SetupWizard.gd:246-249` 括号路径炸裂**：将 Windows 转换命令写入临时 `.bat` 执行，避免路径含 `)` 时被 `cmd.exe` 的 `(...)` 截断。
2. **修 GitHub Actions CI 变绿**：本机运行 `cargo fmt --all`（对大表加 `#[rustfmt::skip]`）+ `cargo clippy --workspace --all-targets -- -D warnings` + 离线 `cargo test --workspace --no-fail-fast`；升级 `.github/workflows/ci.yml` 中的 `actions/checkout`、`actions/setup-dotnet`、`actions/cache` 版本以消除 Node.js 20 警告。
3. **发包前强制门禁**：
   - 执行 `bash tools/godot_check.sh`（38/38 无头）
   - 执行 `LWsmf_RENDER=1 bash tools/godot_check.sh`（OpenGL3 像素渲染）
   - 将发布包解压到**脱离开发仓库的干净目录**，双击启动走通“标题 → 新游戏 → 父亲序章 → 卧室 → 道具列表 → 存档/读档 → F4/F12/F8/F11”全链路。
