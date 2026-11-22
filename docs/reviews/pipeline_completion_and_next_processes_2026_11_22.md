# 7 大实机问题与 8 条管线疏通验收及新增可规范流程报告（commit `2c57740`）

> 验收对象：`Living-With-SIster-Monochrome-Fantasy`（commit `2c577404f125e03a9749ae95ee53c80d2ef23f4f`）。  
> 验收基准：`LIVE_PLAY_ROOT_CAUSE_DIAGNOSIS_2026_11_22.md` 与 `PIPELINE_OVERHAUL_WORK_ORDER_2026_11_22.md`。  
> 证据边界：本沙箱无 Godot / .NET / Rust 编译器与零售资产，源码级核验已完成；真实窗口视觉/DPI/音频实测项标 `needs-runtime`。

---

## 1. 总体结论

**`LIVE_PLAY_ROOT_CAUSE_DIAGNOSIS_2026_11_22.md` 的全部 7 项实机问题与 8 条底层管线的代码改造已基本全部完成并落地对应回归测试（`D-69` / `D-70`），其中还顺带挖出了 2 个极高价值的真实根因（`CE94` 除零放大 115 倍与 `<<GET_FILE_EXIST>>` 未接通导致缩略图不显示）。**

但源码审计仍捕获到 **3 处残留小缺陷/文档冲突**（均可在 10 分钟内收口），同时发掘出 **5 条尚未体系化、值得在正式发版前疏通规范的工程与测试流程**。

---

## 2. 7 大实机问题与 8 条管线逐项验收表

### 2.1 `LIVE_PLAY_ROOT_CAUSE_DIAGNOSIS_2026_11_22.md`（7 项）

| 问题编号 | 原症状 | 验收状态 | 源码核验依据（文件:行） | 备注 |
| --- | --- | --- | --- | --- |
| **问题 1** | 字体始终回退微软雅黑 | **已完成** | `UiSkin.gd:99-158,208-267`, `PlayOverlays.gd:598`, `tools/package_release.sh:163-201` | 大小写无关探测 + `user://cache/Lolita.ttf` 落盘 `load_dynamic_font` + `我/妹/存` 抽样 + F3 `Font=` + 打包硬检查 |
| **问题 2** | 存档界面黑影、黑字、无缩略图 | **已完成** | `PictureStage.gd:543-548,818-820`, `EventVm.cs:791-799`, `VmController.cs:1021-1042`, `AssetDB.gd:84-87` | 修正 `picture_state(2/3)` 像素宽高根治 115 倍放大遮挡；接通 `<<GET_FILE_EXIST>>` 与 `../save/` 解析；缺失缩略图零尺寸透明 |
| **问题 3** | 存档栏光标闪烁而非稳定高亮 | **已完成** | `PictureStage.gd:1006-1007`, `PictureEffects.gd:44` | 同位置重绘保留 `color_corr`/`effects`；单次闪光 `mini(age, duration)` 钳位 |
| **问题 4** | 读档后卧室中央残留灰黑框 | **已完成** | `GameSession.cs:589-594`, `PictureStage.gd:1386-1388` | 定位为 `CE101` 的 `16000..16002`，扩展过滤 `15000..15999`/`16000..16999` 并跳过淡出中槽位 |
| **问题 5** | 道具列表闪现消失 + 位置偏移 | **已完成** | `GameRoot.gd:334-341` | 移除 `is_root` 与单帧 `_action_input_pending` 限制，入场即抑制并 `SetPointerHeld(false)`；真字体恢复后排版归位（`needs-runtime` 复核） |
| **问题 6** | 存读档点击吞键需连点多次 | **已完成** | `InputRouter.gd:165`, `GameRoot.gd:155,330` | `held_mask` 恢复 `pointer_pressed or _click_latch`，移除 `PointerPressObserved` 提前清空 |
| **问题 7** | 气泡/描边/人名牌偏离原版 | **已完成** | `MessageWindow.gd:42-63,487-504`, `Nameplate.gd:7-78`, `UiSkin.gd:13`, `GameRoot.gd:517` | 2D Rounded-Box SDF 羽化 Shader、`TEXT_OUTLINE_SIZE=2`、28px 双环胶囊无粗黑边人名牌 |

### 2.2 八条底层管线施工单核验

| 阶段 | 管线 | 源码完成度 | 剩余边界 |
| --- | --- | --- | --- |
| **Phase 1** | 键鼠输入与模态路由 | **100% 完成** | 仅 `GameRoot.gd:339` 标题阶段条件可再简化（见 §3） |
| **Phase 2** | 素材与字体加载 | **100% 完成** | `needs-runtime` 实机核对中文字形覆盖 |
| **Phase 3** | 图片绘制与特效状态 | **95% 完成** | `dump_pictures` 的 `tone` 读取类型错误（见 §3 `RES-01`） |
| **Phase 4** | 存读档与瞬态清理 | **100% 完成** | `_reset_presentation_transient_state` 统一清理含 `clear_suppression()` |
| **Phase 5** | 文本排版与对话/人名牌 UI | **100% 完成** | 实机视觉对照（`needs-runtime`） |
| **Phase 6** | 音频时钟与通道语义 | **100% 完成** | 已核实全量 1,611 条 `140` 无 `soundType 3..7`；真驱动听感 `needs-runtime` |
| **Phase 7** | 窗口、缩放、DPI 与帧时钟 | **源码已完成**（模态暂停已接） | Windows 125%/150% DPI + `F8`/`F11` 热区手测（`needs-runtime`） |
| **Phase 8** | 发布打包、首次运行向导与 CI | **100% 完成** | `ci.yml` 已升级 `@v5` + `--lib`，待干净目录双击发包冒烟（`needs-runtime`） |

---

## 3. 本轮代码审查新发现的 3 处残缺与冲突（建议顺手修复）

| 编号 | 严重度 | 位置（文件:行） | 现象与根因 | 最小修复 |
| --- | --- | --- | --- | --- |
| `RES-01` | **P2** | `godot/presentation_gd/PictureStage.gd:1335-1339` | **`F6` 槽位 Dump 的 `tone` 列永远输出假值 `100,100,100`**。`state["color_corr"]` 是 `Array` `[0,0,0]`（`PictureStage.gd:956`），不是 `Dictionary` `{"r","g","b"}`，且漏读了 `state["tone"]`（`Array` `[100,100,100]`） | 改为读取 `var t: Array = st.get("tone", [100, 100, 100])` 与 `var c: Array = st.get("color_corr", [0, 0, 0])`，同时输出 `tone` 和 `corr` |
| `RES-02` | **P3** | `godot/presentation_gd/GameRoot.gd:339` | `_on_event_entered` 写了 `if phase != "title" or event_id == 128 or event_id == 131: vm.SetPointerHeld(false)`，标题页进入其他子事件（如 `CE42`）时未清 `vm` 同帧 `_pointerPressed` | 去掉多余条件，在 `router.action_input_held()` 为真时统一调用 `vm.SetPointerHeld(false)` |
| `RES-03` | **P2**（文档冲突） | `docs/godot/DECISIONS.md:68` vs `:135-136`、`docs/reviews/live_play_root_cause_diagnosis_2026_11_22.md:148` | ① `DECISIONS.md` 出现**两个 `D-70`**，且第 68 行旧 `D-70` 仍写“`124` field 2/3 禁止替换成原图像素”，与第 135 行 `D-69`（“按 WOLF 语义返回像素宽高”）直接矛盾；② `D-69`/`D-70` 被误插在 `## 5. 仍待拍板` 表底部而非 `## 1. 已定决策`；③ `live_play` 第 148 行仍写“保留单帧 `_click_latch`”，与实际源码 `pointer_pressed or _click_latch` 不一致 | 将新决策重编号为 `D-71`、`D-72` 移入 `## 1. 已定决策`，在旧 `D-70` 标注 `（124 field 2/3 部分已被 D-71 取代）`，并同步更正 `live_play` 第 148 行 |

---

## 4. 发掘：还有哪些方面/流程值得进一步疏通规范？

底层 8 条核心管线打通后，当前项目已经进入**“防回归 + 工业化发版”**阶段。以下 **5 条工程与质量流程**目前仍是薄弱环节，建议按需建设：

1. **关键画面视觉回归流水线（Golden Screenshot Diff）**
   - **现状缺口**：之前 `D-60` 混色改坏存档页、字体回退、气泡台阶感都是靠你人工截图才发现，无头门禁与现有 `LWsmf_RENDER=1` 只覆盖了少量测试场景。
   - **规范建议**：固定 6–8 个核心场景（标题、卧室、对话+人名牌、道具列表、存档页、战斗、H 场景），在 `LWsmf_RENDER=1` 下自动截屏并生成**拼图对比页（contact sheet）**或 SSIM/像素差告警，每次改动渲染/字体/UI 时一键肉眼扫一眼即可防视觉回退。

2. **全资产静态引用交叉校验流程（Pre-Release Asset Integrity Check）**
   - **现状缺口**：`package_release.sh` 刚补上 `Lolita.ttf` 校验，但对 `programs.json` 里静态引用的全部图片/音频（如 Tips、立绘、CG、BGM/SE）尚未在打包阶段做“脚本引用 vs `media.pak` 索引”全量对账。
   - **规范建议**：在 `tools/release_audit.py`（或打包前）增加静态引用扫描，直接列出 `programs.json` 引用但 `media.pak` 缺失的文件清单，杜绝“打完包玩到一半才缺图”的问题。

3. **存档 Schema 迁移与“老存档清洗”统一管道（`SaveMigrator`）**
   - **现状缺口**：目前靠在 `SaveGame` 与 `LoadGame` 多处散落 `IsSaveUiPicture` 过滤来清理旧档残框；未来若新增 `screen_tint`、对话框位置模式等持久化字段，很容易造成版本兼容碎片化。
   - **规范建议**：在 `SaveCodec.ReadWithSource` 后收口一个 `SaveMigrator.Normalize(SaveState)`，集中处理旧版本字段补齐、脏 UI 槽位清洗与 `schema_version` 升级。

4. **一键现场诊断快照流程（Unified Diagnostic Bundle）**
   - **现状缺口**：现在诊断信息分散在 `user://crash.log`、`user://picture_dump.txt`（F6）、F3 Debug HUD 与故障弹窗导出 4 个地方。
   - **规范建议**：把 `F6` 升级为**一键导出完整现场快照**（单文件包含：字体来源与字形命中、AssetDB 缺图清单、活跃 `Instances`（eventId/pc/wait）、活跃 `PictureDump`（含 `tone` + `color_corr`）、`InputRouter` 抑制/按键状态、最近 30 条日志），以后你实机遇到任何怪现象，按一下 `F6` 就能把完整现场发出来，排错效率提升 10 倍。

5. **决策账本与文档一致性自动 Lint（Single Source of Truth）**
   - **现状缺口**：高频迭代下已经出现 `DECISIONS.md` 重复 `D-70`、新旧决策互殴、报告滞后源码的情况，容易误导后续审阅或编码 Agent。
   - **规范建议**：加一个 30 行 Python 检查脚本接入 `godot_check.sh` / CI：自动校验 `DECISIONS.md` 的 `D-XX` 编号唯一且不重复、无跨表错位，确保架构决策账本始终是唯一可信真相源。
