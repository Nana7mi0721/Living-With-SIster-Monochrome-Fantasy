# LWSMF（`9f36abb`）发布前 Orchestrator 综合审计与演进报告

> **状态**：现行 · 权威（Phase A 三路并发只读审计汇总，基线 `9f36abb`）
> **任务清单来源**：[lwsmf_orchestrator_pre_release_tasklist.md](../reviews/lwsmf_orchestrator_pre_release_tasklist.md)
> **对比基线（只读参考）**：姊妹作 `Imouto Life Monochrome`（0.1.1 公开发布版）

---

## 1. Workstream 1：`9f36abb` 遗留与未知 Bug 深度复核

### 1.1 上轮 11 项已知问题核查结论

| 检查项 | 现状结论 | 证据与根因分析 |
| :--- | :--- | :--- |
| **`zero_tone_hidden`（读书剪影 / 钻研黑框 / 随机事件双黑块 / 道具闪消）** | **核心黑化已修，但底层机制揭示 P0 根因** | 全库 1,272 次 `0x02000000` Draw 与 418 次 Move 中，读书（`CE222` pc=49）、钻研（`CE222`）与随机事件双黑块（`CE445` 3010/3011）均为 `kind=0` 图片以 `tone=[0,0,0]` 起步并在同帧通过 7 条 `150 Move` 渐变至 `[100,100,100]`。`54687c0` 已补齐 `Move` 跨度与 `tone_transition` 插值；道具列表 `CE235`（slot 2300 `kind=0`）从未触发 `zero_tone_hidden`。但 `zero_tone_hidden` 仅因**混合模式双处失效**而存在（见下节 `W1-P0-01`）。 |
| **零缩放进度条（0% 防满格）** | **纹理条通过；文本与九宫格窗口仍漏像素（`W1-P1-03`）** | 体力条 `CE216`（6233）、战斗血条 `CE240`（3501/3511）、H 场景 `CE339`（1993/1997）为 `TextureRect`，`scale==0 -> 0.0` 正常归零。但 `NinePatchRect` 因 `patch_margin=3` 强制 `6x6` 最小尺寸，且 `PictureTextLabel` 未设 `clip_contents`，导致 `scale==0` 窗口（`CE64/101/184/186/270`）与文本（`CE101/184/186`）仍泄漏可见像素。 |
| **`scale < 0` 镜像定位（战斗 & H 槽）** | **检出 P0 实锤 Bug（`W1-P0-02`）** | `CE240` 3500/3502、`CE339` 1996（H 场景 BAD 槽底框）与 `CE241` 4020（回合横幅）采用 `anchor=0, scale=-100`（按左锚边向左镜像）；当前渲染直接以 `pivot=size*0.5` 原地翻转并保持 `(x, y)` 左上角，导致左槽错盖右槽。 |
| **`150 Erase / Hide / Move` 跨度与 `290` 同帧组合** | **指令解码 0 漏判；存在 `Hide` 桥接与 Move 缓存缺陷** | 扫描全库 550 个公共事件 + 9 张地图：`Erase`（952 span 均 `args[3]`）、`Hide`（41 span 均 `args[0]`）、`Move`（158 span 均 `args[12]`）掩码零遗漏；`290` 共 640 处（378 紧接 `150`），`GameRoot._on_effect` 先刷新队列故不会错序。但 `VmController.cs:510` 的 `Hide` span 误读 `Args[3]`，`PictureStage._cmds` 在 Move 后被 `args[1..2]=0` 覆盖并使读档重放静默丢图（`W1-P1-02`）。 |
| **UI 像素对齐（更新履历 / 气泡 / 名牌 / 选项 / 存档）** | **主布局达标；存在点击穿透、分配与调色板 3 项缺陷** | `2005` 更新履历 600px 居中左对齐、`160..864` 气泡与 `26px` 胶囊名牌（`bubble.y - 26` 零遮挡）、`ChoiceWindow` 34px 行高均已对齐；但 `MessageWindow` 缺 `_arm_modal_release_guard()` 导致关闭 Tips/对话穿透（`W1-P1-01`），`_draw()` 每帧新建 `StyleBoxFlat`（`W1-P1-04`），以及调色板仍用 8 色硬编码而非 `UiSkin.TEXT_COLORS` 30 槽（`W1-P1-05`）。 |
| **`media.pak` 全量资产覆盖** | **通过（0 `[ASSET_MISS]`）** | `work/all/media.pak` 含 3,904 个资产，`work/retail_dat` 中全部非 `BasicData/` 媒体 100% 纳入；M13 主线通关 1/8 槽日志 0 漏图、0 漏音效。 |
| **`M13` 帧数变化定性** | **已确认系正当语义修复引起，非跳过演出** | `54687c0` 修复 `121`/`112`/`124` 与子事件保护后，第 91–95 天白天随机/事件分支变化（1 槽 `34437 -> 33962` `-475` 帧，8 槽 `34631 -> 33644` `-987` 帧），而结局固定过场 `stage29 -> stage10 -> end` 严格逐帧一致（`2536 + 51` 帧）。 |

### 1.2 Workstream 1 确认缺陷清单

- **`W1-P0-01` 混合模式双处失效（`EventVm.cs` + `PictureStage.gd`）**：WOLF 真实混合模式位于 `head` 的 `(head >> 8) & 0xF`（`0=Normal, 1=ADD, 2=SUB, 3=MUL, 15=UNCHANGED`），而 `args[8]` 是 `0.1°` 旋转角（如 `CE239` `1250 = 125.0°`）；且 `PictureStage._apply_blend` 只给空的容器 `holder` 设置 `material`，子节点 `TextureRect` / `NinePatchRect` / `PictureTextLabel` 均未开启 `use_parent_material = true`。所有 8 个触发 `zero_tone_hidden` 的 `CursorBase.png` 窗口实际上都是 `blend = 1 (ADD)` + `tone = [0,0,0]`。
- **`W1-P0-02` 负水平缩放与非居中锚点定位错误（`PictureStage.gd`）**：`scale < 0` 且 `anchor in [0, 2]`（左锚）未向左平移 `size.x`（`anchor in [3, 4]` 未右移 `size.x`），破坏战斗左侧血条与 H 场景 BAD 槽。
- **`W1-P1-01` `MessageWindow` 关闭点击穿透（`GameRoot.gd`）**：左/右键关闭 Tips/对话未调用 `_arm_modal_release_guard()`，且 `_key_state()` 未在 guard 期间屏蔽鼠标按鍵位。
- **`W1-P1-02` `Move` 后 `_cmds` 损坏与读档丢图（`PictureStage.gd`, `GameRoot.gd`）**：`Move` 覆盖了原始 `action` 与 `args[1..2]`，`restore_pictures()` 回放 `action=1` 变空操作，且快照前未调用 `_flush_pending_pictures()`。
- **`W1-P1-03` `scale == 0` 泄漏像素（`PictureStage.gd`）**：`scale_x == 0 || scale_y == 0` 时未隐藏节点（`node.visible = false`），导致 `NinePatchRect` 残留 `6x6` 方点、文本未缩放。
- **`W1-P1-04` `_draw()` 每帧堆分配（`MessageWindow.gd`, `Nameplate.gd`）**：60Hz 重绘每次新建 2–3 个 `StyleBoxFlat.new()`。
- **`W1-P1-05` 文本调色板偏差（`MessageWindow.gd`, `UiSkin.gd`）**：`MessageWindow` 使用 8 色 `COLOR_TABLE` 模 8 索引，未统一调用 `UiSkin.color_for_index()`；`UiSkin.TEXT_COLORS` 缺后 10 槽（20..29）。
- **`W1-P2-01..04`**：`VmController` `Hide` span 参数偏移、`PictureStage` 大跨度优化（`290..8300`）、`GameRoot` `no target` 1.3 万条日志刷屏、`290 kind=5` 三角波、`<C>...<L>` 行对齐覆写、`AssetDB` 缓存键规范化与覆盖计数泄漏、`ChoiceWindow` 首个空选项高亮及离屏停绘。

---

## 2. Workstream 2：对照姊妹作（`Imouto Life Monochrome`）跨仓查漏矩阵

| 姊妹作踩坑项 | LWSMF 现状 | 严重度 | 证据落点与修复结论 |
| :--- | :--- | :---: | :--- |
| **`PUB-02` 存档非原子写盘** | **已达标** | 通过 | `SaveCodec.Write` / `WriteRaw` 全走 `.tmp -> Flush(true) -> File.Move` + `.bak` 校验轮转；仅加固 `catch` 内删 `.tmp` 的异常捕获。 |
| **`PUB-03` 并发输入与热区穿透** | **4 处缺口** | **P1** | `MessageWindow` 左/右键关闭缺 `_arm_modal_release_guard()`；`_key_state()` 缺鼠标位抑制；`_on_map_cell_clicked` 缺 guard 检查。 |
| **`PUB-05` 隐私与硬编码绝对路径** | **实锤泄露（39 处/13 文件）** | **P0** | `UiSkin.gd:82` 硬编码 Steam 路径；`AGENT.md` 含个人邮箱与旧路径；`tools/{env,setpath,wolfcap,godot_check,package_release}` 含 `REISEN` 与 `E:/Study/...` 本机路径。 |
| **`PUB-01` 构建与 CI 自洽性** | **3 处工程缺口** | **P1** | `godot/export_presets.cfg:11` 漏排 `addons/*`；`godot_check.sh` / `package_release.sh` 硬编码 `/e/Study/tools/...` 而非探测 `PATH`。 |
| **`220/221/222` 与 `System.sav`** | **P1 严重串档 Bug + CData 缺口** | **P1** | `EventVm.cs:1207` 仅认 `StrVar`，把 `CE48/CE131/CE435` 传入的 `CSelf5 (1600005="Save/System.sav")` 误当数值 `0` 写进 `save_01.json`！需对齐 `SelfVar` 字符串槽 `5..9` 判定（并同步 `crates/wolf-vm`）。 |
| **取消读档回标题防死锁** | **已达标** | 通过 | `GameSession.cs:263-272` 自动重建 `TitleMap` ev2 主循环。 |
| **窗口失焦（`Alt+Tab`）防卡键** | **2 处细缺口** | **P2** | `GameRoot.gd:679-688` 失焦未清 `_modal_release_guard`、未调 `vm.ResetPointer()`、未调 `Input.flush_buffered_events()`。 |
| **`174 ReturnToTitle` / `175 EndGame`** | **P1 帧中同步重入隐患** | **P1** | `EventVm.cs:1385` 在 `RunInstances` 循环内同步触发 `restart_title()` 重建 `Session`；应改帧尾延迟执行，`ResetSessionState()` 需补清等待态、`_pictures` 与 `_backlog`。 |
| **存档格防单击误覆盖** | **与原作一致裸覆盖（槽 1 只读）** | **P2** | `CE131` 原生锁定槽 1 仅给 `CE435` 自动存档，槽 2..N 单击直接覆盖（底层有 `.bak`）。 |
| **战斗 / H 场景并行与中断泄漏** | **`CE240`/`CE339` 仅边沿触发** | **P1** | `CE240`（战斗战力条）与 `CE339`（H 场景进度条）为无循环线性事件，未列入 `AutoEvents.ResumeOnLoad` 会导致数值变动时不重绘；需纳入 `ResumeOnLoad` 并做全量对拍回归验证。`CE362` 浴室计时器中断残留（`2000210` 仅自清）标为 P2 记录。 |

---

## 3. Workstream 3：《超越原作优化项移植评估表》

| 类别 | 优化项 | LWSMF 现状 | 判定 | 预估工作量 | 冲突与集成说明 |
| :--- | :--- | :--- | :---: | :---: | :--- |
| **显示与窗口** | **`F8` 缩放过滤切换**（Integer / Fit-Nearest / Fit-Linear） | 未实现 | **推荐落地 (P1)** | 0.5 人日 | `GameRoot` + `content_scale_stretch` / `canvas_item_default_texture_filter` + Toast 提示。 |
| **显示与窗口** | **窗口初始尺寸记忆（HiDPI 自适应）** | 固定 1024×576 | **推荐落地 (P1)** | 0.5 人日 | 持久化 `user://display_settings.cfg`，按 `screen_get_usable_rect` clamp；headless 跳过。 |
| **显示与窗口** | **`F11` 全屏 + 黑边小鼠坐标精度** | `F11` 已有，缺黑边钳位 | **推荐落地 (P1)** | 0.2 人日 | `project.godot` 显式 `stretch/aspect="keep"`；`InputRouter` 忽略黑边外点击并钳位悬停。 |
| **游玩便利** | **`F4` 消息回顾（Backlog）抛光** | 已实现 90% | **推荐落地 (P1)** | 0.2 人日 | 补 `restart_title`/`Load` 清空 `_backlog` 与 `PgUp`/`PgDn` 翻页。 |
| **游玩便利** | **`F5/F9` 快捷存读档独立槽** | **冲突！直接写 `save_01.json`** | **推荐落地 (P1 必改)** | 0.5 人日 | `CE435` 每日自动存档占用 `save_01.json`，`F5/F9` 必须迁至独立 `quicksave.json`（`GameSessionController.QuickSavePath()`）。 |
| **游玩便利** | **每日自动存档槽** | **原生 `CE435` 已具备** | **不适用（不重复造轮子）** | 0 | 严禁自建冗余自动存档；只需修好 `222 CSelf5` 让原生 `CE435` 正常更新 `System.sav`。 |
| **游玩便利** | **`--hotkeys` 快捷键重绑定 / `F12` 返回标题确认** | `F12` 已落地；热键硬编码 | **延后 (P2)** | 0.5 人日 | 前期保留 `F2/F3/F4/F5/F8/F9/F11/F12` 默认约定；热键自定义配置留作 P2。 |
| **调试洞察** | **`F2` 游玩倾向 / 里程碑统计浮层** | 未实现 | **推荐落地 (P2)** | 0.5 人日 | 在 `PlayOverlays.gd` 只读读取 LWSMF 本作真实变量（`v4/v5/v6/v160/v40/v42/v44/v503/v550`），绝不写档。 |
| **调试洞察** | **`F3` 运行时调试浮层** | 未实现 | **推荐落地 (P2)** | 0.5 人日 | 在 `PlayOverlays.gd` 显示 FPS、帧号、`phase`、活跃事件、`picture_count`、`AssetDB` 内存与漏图。 |
| **生态跨平台** | **运行时 TSV 翻译替换层** | `tools/translation/` 未接 Godot C# | **延后 (P2)** | 1–1.5 人日 | 可作为 `TextOverlay.cs` 在 `101/102` 处挂载，当前原生中文文本已完整。 |
| **生态跨平台** | **`--mod` 覆盖层** | `AssetDB` 已有散文件优先机制 | **延后 (P2)** | 1 人日 | 在 `AssetDB` 插入 `mod_roots` 层并防路径穿越即可，当前优先保发布收口。 |
| **生态跨平台** | **Android 移植（Godot 4.7.2 .NET）** | 未配置 | **延后（长期专项）** | 3–5 周 | **可行但本轮不开工**：包体是姊妹作 6.4 倍（728.8 MB 媒体 + ~100 MB .NET Android 运行时 ≈ 850 MB 级 APK，slim 后仍 ~500–600 MB）；`MediaPakReader` 用 `FileStream` 无法直读 APK；SLG/战斗/H 多级小热区需完整触控手势重构。 |

---

## 4. Workstream 4：代码规范、去注释污染与结构治理

### 4.1 “三不写、两保留”注释规范与扫污基线
在全仓 55 个源码与工具文件中检出 **81 行**需要清理的叙事/临时编号注释（涉及 20 个文件，Top 20 文件占 66 行，其中 `GameRoot.gd` 29 行、`PictureStage.gd` 10 行、`GameSession.cs` 6 行、`VmController.cs` 6 行、`AssetDB.gd` 4 行），以及测试代码中 10 处 C# 可空编译警告（`M13Harness.cs`、`M4FullRouteTraceTest.cs`、`RouteTraceTest.cs`；生产代码 0 警告）。
同时清理根目录双重且相互冲突的 `AGENT.md`（保留 `AGENTS.md` 作为唯一规范），并在 `AGENTS.md` 写入“三不写（不写过去式重构叙事、不写临时审查/里程碑编号、不写复述代码的废话）、两保留（保留 WOLF/零售数据硬约束 Why、保留简洁公共契约/安全不变量）”。

### 4.2 Top 5 大文件低风险拆分方案
1. **`PictureStage.gd`（1,668 → ~1,240 行）**：拆出 **`PictureShapes.gd`**（`<SQUARE>`/`<GRADX>`/`<GRADY>`/`<TRI-*>`/`<CIRCLE>`/`<LINE>` 与九宫格）与 **`PictureEffects.gd`**（`290` 特效/色调/混合/呼吸缩放）。
2. **`GameRoot.gd`（1,490 → ~960 行）**：拆出 **`PlayOverlays.gd`**（Toast、D-13 故障弹窗、F4 Backlog、F12 返回标题确认、F2 统计、F3 调试），删除死控件 `_title_button`，并将 `PROF` 性能采集门控至 `LWsmf_PROFILE=1`。
3. **集中管理裸魔数**：C# 新增 **`WolfIds.cs`**，GDScript 新增 **`WolfConst.gd`**，消除公共事件号、SysVar 索引、消息位置模式 `2002/2005/2008` 与位掩码的跨文件硬编码。

---

## 5. Workstream 5：包体体积精简、性能优化与发布工程化

### 5.1 发版前未优化包体体检账本（实测）

| 组成部分 | 文件数 | 实测体积 | 占比 | 现状说明与瘦身抓手 |
| :--- | ---: | ---: | ---: | :--- |
| `game/Lwsmf.exe` | 1 | 104.31 MB | 10.7% | 官方 Godot 4.7.2 Mono 全特性模板（3D/Vulkan/D3D12） |
| `game/Lwsmf.pck` | 1 | 0.08 MB | <0.1% | 需补充 `exclude_filter` 排除 `addons/*` |
| `game/data_Lwsmf_windows_x86_64/` | 186 | 76.42 MB | 7.9% | 0 PDB；全量 .NET 8 BCL（反射限制暂不硬剪） |
| `game_data/*.json` | 4 | **52.06 MB** | 5.4% | `programs.json` 为带缩进格式（49.84 MB）→ 紧凑 JSON 可直接降至 **~20.54 MB**（**−31.5 MB**，零代码风险） |
| `work/all/media.pak`（`WOLFMPK1`） | 1 | **728.78 MB** | **75.0%** | 3,904 个资产零压缩打包（OGG 227.18 / PNG 226.03 / JPG 191.30 / MP3 80.42 / WAV 3.66 MB）；31/37 BGM OGG 带 `LOOPSTART`，0/23 WAV 带循环点；`MapChip/`（0.74 MB）从不渲染。 |
| `work/all/` 散文件（不含 `save/`） | 292 | **8.67 MB** | 0.9% | 运行时仅读 `BasicData/*.project`（384 KB）与 `media.pak`；`BasicData/*.dat`、`MapData/*.mps`、`MapChip/`、`SystemFile/` 均为冗余拷贝（**−8.3 MB**）。 |
| `converter/` | 17 | 0.07 MB | <0.1% | 保留供离线数据校验 |
| **合计（标准导出基线）** | **502** | **~970.4 MiB** | **100%** | — |

### 5.2 运行时性能与发布工程化结论
- **`MessageWindow.gd` & `Nameplate.gd`**：修复 `_draw()` 每帧 `StyleBoxFlat.new()`（改为 `_init()` 缓存复用）。
- **`VmController.TakePicturesJson()`**：复用成员变量 `StringBuilder _picJsonSb`，消除活动帧 JSON 串构建分配；`AutoEvents` / `BattleWiring` 热点移除 LINQ 闭包。
- **`AssetDB.gd`（384 MB LRU）**：修复逻辑路径反斜杠/前缀未归一化导致的重复解码、修复 `_store()` 覆写时 `used_bytes` 漏减、按图片/音频拆分扩展名解析防同名碰撞，并增加**文件头魔数嗅探**（`RIFF....WEBP` / `\x89PNG` / `\xFF\xD8` / `OggS`），确保 `--slim` 透明兼容。
- **工程工具链**：新增纯 Python 零外部依赖的 `tools/pack_media.py`（字节级兼容 `MediaPakReader.cs` 的 `WOLFMPK1` + `--slim` Pillow WebP/无损重压并保留原始路径或兼容魔数加载）；升级 `tools/release_audit.py`（适配 `media.pak` + 隐私/路径/PDB/`addons` 扫描）；升级 `tools/package_release.sh`（先审计 `$STAGE` 再交换 `dist/`、Release 构建、`--slim` 开关、ZIP 压缩与 `SHA256SUMS.txt` 账本输出）。
