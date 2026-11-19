# `imoto-life`（`c553d04`）最新代码审阅与下一步行动指导

- **审阅基线**：`imoto-life` commit `c553d04a1dee30b99786205835291183da001086`（对比上一轮 `5dd6956`）
- **门禁状态**：`dotnet build` 0 errors；headless 与 `LWsmf_RENDER=1` 双模式 `GODOT_GATE_OK` 38/38；`PERFORMANCE_FRAME_STRICT_OK fps=3825.1`
- **总体结论**：**当前代码已无 P0/P1 发布阻断缺陷**。上一轮（R13）指出的 7 项问题已全部高质量闭环，并额外补齐了 `150` 头部标志与 `290` 精确色调公式。仅发现 **3 处轻量 P2/P3 边界项**；项目已具备**正式打包、实机验收并推进最终发布（含可选 `media.pak` 混淆）**的条件。

---

## 一、R13 缺陷闭环核验结果

| 原编号 | 审阅项 | `c553d04` 核验结果 | 代码证据 |
| --- | --- | --- | --- |
| `R13-P1-01` | `WolfRng.FromEnvironment()` 传错构造 + 窗口播种 | **已闭环** | `WolfRng.cs:48` 改为 `FromEventSeed(...)`；`GameRoot.gd:158-159` 在非 headless 且 `WOLF_LIVE_RNG != "0"` 时播种 |
| `R13-P1-02` | `_transition_rect` 缺少 `z_index` 被遮挡 | **已闭环** | `GameRoot.gd:84` 已设 `_transition_rect.z_index = 60`（位于 `LAYER_CURSOR=50` 之上、前端 UI `90+` 之下） |
| `R13-P1-03` | `PictureStage.gd` `tone` 默认值 `255` 吞掉变暗特效 | **已闭环** | `PictureStage.gd:565-574, 929-953` 按 `TONE_PRESENT (0x02000000)` 门控，默认 `tone=[100,100,100]`，实现完整 `FLASH/BLINK` 舍入公式 |
| `R13-P1-04` | `EventVm.cs` `case 162` 漏 `return Signal.Blocked` | **已闭环** | `EventVm.cs:908-911` 已改为 `return Signal.Blocked;` |
| `R13-P1-05` | 同槽换图误继承特效 + `_hit_rect` 缺 `shift`/`hidden` | **已闭环** | `PictureStage.gd:121-141, 605-664` 已按 `kind/file/text` 严格区分换图与同图重绘，并透传 `prev_state` 支持 `relative`/`keep_shift`；`:214-215` 排除 `hidden`，`:347-359` 叠加 `zoom/breath/shift/shake` |
| `R13-P2-01` | 每帧重绘 + 材质泄漏 + MUL 忽略透明度 | **已闭环** | `PictureStage.gd:771-790` 新增 `_state_animating` 跳过静态槽重绘；`:956-972` 复用 `_blend_materials` 缓存；`:907-908` `blend==3` 强制 `tint.a=1.0`；渲染帧率跃升至 `3825 fps` |
| `R13-P2-02` | `290 kind=8/9/10` pattern 切帧未接入画面 | **已闭环** | `PictureStage.gd:420-430, 914-926` 已接入 `AtlasTexture` 分格裁剪；`:1048-1052` 同步采纳了 `imouto-life` `duration <= 0` 停止 pattern 动画的修复，提前规避强制演出闪烁 |

---

## 二、本轮新发现的轻量问题（无发布阻断）

| 编号 | 严重度 | 位置 | 问题描述 | 最小修复 |
| --- | --- | --- | --- | --- |
| **R14-P2-01** | **P2** | `godot/presentation_gd/PictureStage.gd:395-397` | **`track_picture` 先 `_store_cmd` 再 `_begin_draw`，导致队列追踪模式下 `prev_cmd` 永远等于新命令自身**。对比 `draw_picture`（`:139-140` 先 `_begin_draw` 再 `_store_cmd`），`track_picture` 顺序颠倒后，`_begin_draw` 中的 `redraws` 判定在同槽换图时会误判为“同图重绘”，跳过新图淡入并误保旧动画相位。 | 将 `track_picture` 第 395 行的 `_store_cmd(...)` 移到第 397 行 `_begin_draw(...)` 之后，同时像 `draw_picture` 一样在 `_begin_draw` 前快照 `prev_state` 传入。 |
| **R14-P3-01** | **P3** | `godot/presentation_gd/PictureStage.gd:901` | 纵向独立缩放（`scale_mode == 3`）已支持 `args[16]`，但 `node.scale` 仅对横向负值取 `-1.0`，未对 `scale_y < 0` 做纵向镜像。 | 改为 `Vector2(-1.0 if int(pose.get("scale", 100)) < 0 else 1.0, -1.0 if int(pose.get("scale_y", 100)) < 0 else 1.0)`。 |
| **R14-P3-02** | **P3** | `godot/presentation_gd/GameRoot.gd:158-159, 826-857` | 实机 RNG 播种只用 `Time.get_ticks_usec()`（进程启动后耗时较接近，熵偏低），且 `confirm_return_to_title()` 后未重新扰动种子。 | 抽一个 `_seed_live_rng_if_windowed()`，使用 `int(Time.get_unix_time_from_system() * 1000.0) ^ Time.get_ticks_usec()`，并在 `_ready()` 与 `confirm_return_to_title()` 成功后调用。 |

---

## 三、对“仍标注的偏差”的分级定性（哪些要修，哪些可以放过）

不要把所有已知偏差都当成同等优先级去硬啃，按投入产出比分为三档：

1. **必须先查清（高价值，低成本）**：
   - **M13 少 780 帧的定位**：虽然两轮断言全过且 `route diff` 100%，但 780 帧（13 秒）不是偶然误差。花 10 分钟定位第一帧分叉点，能确保不是某段过场演出或战斗等待被意外跳过。
   - **150 的 `span` 标志（`head & 0x01000000`）**：目前 `Erase`/`Hide` 无条件把 `args[3]`/`args[0]` 当范围终点，万一某条单槽擦除的残留参数非零，可能误擦相邻槽位。按位门控只需 5 行代码，建议顺手补上。
2. **实机体验加分项（按需补齐，半天内可完）**：
   - **`F8` 缩放过滤切换（Nearest / Linear）+ 初始窗口尺寸记忆**：高分屏（2K/4K）玩家感受明显，建议做。
   - **白名单剔除无用素材 + C# `media.pak` 密钥混淆**：如果你计划对外分发独立包体且不希望散装 PNG/OGG 直接裸露在目录里，这是最值得投入的一步（详见下文第四节阶段三）。
3. **明确止损、不要再投入精力的项**：
   - **`F4` backlog 的 `Nameplate.speaker`**：既然原版事件本身就不写 `Nameplate.speaker`（人名多半写在对话正文首行或图片里），不必为了它去反推写死人名解析器。
   - **`290` 逐像素截图对拍 & `125 TTL / 211 参数扩展 / 140 tempo` 的零星理论偏差**：在没有实机肉眼 Bug 反馈前，保持现状即可，避免过度工程化。

---

## 四、下一步详细行动指导（分 4 个可执行阶段）

### 阶段 1：轻量代码收尾 + M13 780 帧定性（约 0.5 小时）
1. **修掉 3 个轻量收尾项**：
   - 修复 `R14-P2-01`（`PictureStage.gd` `track_picture` 的 `_store_cmd` 顺序）；
   - 给 `EventVm.cs` `PictureCmd` 补上 `HasSpan = (head & 0x0100_0000) != 0` 并在 `PictureStage.gd` `_start_erase` / `_hide_picture` 按位门控；
   - 补上 `R14-P3-01`（`scale_y < 0`）与 `R14-P3-02`（`_seed_live_rng_if_windowed()`）。
2. **10 分钟定位 M13 少 780 帧**：
   - 临时在 `M13Harness` 中每隔 500 帧打印 `(frame, day, hour, minute, active_event_ids)`，对比 `123cd0d` 与 `c553d04` 的第一处分叉：
     - 若是修掉了旧版空转/卡死（例如 `125` 粘键或未知 `124` 字段返回 0 导致的多余循环），直接记入 `M5_HANDOFF.md` 关闭；
     - 若是某段 `162` 或事件被提前截断，当场定点修复。
3. **重跑门禁**：确认 `bash tools/godot_check.sh` 与 `LWsmf_RENDER=1 bash tools/godot_check.sh` 保持 `38/38` 全绿。

### 阶段 2：人工实机游玩冒烟（核心验收，约 1 小时）
自动化门禁已经跑满 38 步，**接下来最有价值的发现一定来自真实眼睛和鼠标**：
1. 先按当前流程打一次完整自包含包：`bash tools/package_release.sh`，并跑 `python tools/release_audit.py --dist dist/Lwsmf-win64`。
2. 将 `dist/Lwsmf-win64/` 复制到一个**不含源码仓库的干净目录**，双击 `game/Lwsmf.exe`，重点手测以下 7 件事：
   - **标题与开局**：确认标题页没有右上角“返回标题”按钮；连续新开局两次，验证随机事件/开场抽取不再固定。
   - **黑屏转场与特效**：进卧室、切换日期、进出事件时，观察 `160/161/162` 黑幕淡入淡出是否可见、是否平滑、`290` 变暗/闪烁/呼吸是否正常。
   - **热区与输入**：在卧室和菜单界面测试鼠标点击 Picture 按钮是否灵敏；测试 `F11` 全屏（含黑边）下点击是否偏移；确认底部不再露出任何黄色 leader 方块或调试色块。
   - **存读档与 Toast**：在卧室按 `F5` / `F9`，确认 1.5 秒 Toast 显示清晰，读档后 BGM/BGS 音量与循环点正常衔接；在非卧室场景按 `F5` 确认弹出「仅卧室可保存」。
   - **消息回顾与返回标题**：按 `F4` 滚动查看历史对话、按 `Esc` 关闭后确认鼠标不穿透；点击右上角“返回标题”→ 取消 / 确认，验证重回标题后一切干净。
   - **战斗与 H 场景**：手打 1 场完整战斗（攻击、技能选目标、胜利/战败结算）并触发 1 次 H 场景，检查是否有图片残留、图层遮挡或音效缺失。
   - **高分屏观感**：在当前显示器下评估是否需要补 `F8`（Nearest/Linear 过滤切换）和窗口默认缩放倍数（如 1.5x）。

### 阶段 3（推荐）：素材白名单裁剪 + C# `media.pak` 密钥混淆
如果阶段 2 实机验收无视觉/音频异常，就可以把分发包升级为「**无散装素材暴露的精简混淆包**」，步骤如下：
1. **生成显式白名单 `tools/release-assets.manifest`**：
   - 基于 `game_data/manifest.json` + 扫描 `programs.json`/`database.json` 中的全部静态及前缀拼接路径 + 表现层硬编码路径（`SystemFile/WindowBase.png`、`Text_Pause.png`、`TitleGraphic.png` 等）；
   - 排除 `MapChip/` 等未使用的瓦片图（如确认无引用）及所有未引用的原版冗余图片/音效，按字节序排序入库。
2. **新增 C# `godot/presentation/MediaPakReader.cs` 与 Python 打包脚本 `tools/pack_media.py`**：
   - 单文件格式：`Header`（魔数 + 版本 + 索引偏移/长度） + 流式混淆后的媒体字节 + 混淆后的索引表（`rel_path_lower -> (offset, length, salt)`）；
   - 混淆算法放在 `MediaPakReader.cs`（编译进 `Lwsmf.dll`），按 `Span<byte>` / 64-bit 步进做基于固定密钥 + 条目盐值的流式 XOR，读取耗时 < 1 ms 且全程不落明文临时文件。
3. **接入 `AssetDB.gd`（开发期零侵入）**：
   - `_index()` 检测到 `work/media.pak` 时加载 C# 索引；开发期无 `.pak` 时仍走散文件 `work/all/`；
   - `texture()` / `audio()` 命中 `.pak` 时调用 `pak.ReadBytes(rel)`，直接走 `Image.load_*_from_buffer` 与 `AudioStreamOggVorbis.load_from_buffer` / `AudioStreamMP3.data` / `AudioStreamWAV.load_from_buffer` 内存解码。
4. **升级打包与审计脚本**：
   - `tools/package_release.sh` 调用 `pack_media.py` 生成 `dist/Lwsmf-win64/work/media.pak`，不再复制散装图片与音频目录（同时评估是否移除运行期不需要的 `work/all/BasicData/*.dat` 与 `MapData/*.mps`）；
   - `tools/release_audit.py` 改为校验 `media.pak` 覆盖全部白名单条目，并强制断言发布树中 **0 个散装 `.png/.jpg/.jpeg/.bmp/.webp/.ogg/.mp3/.wav`**。

### 阶段 4：最终封板与交付
1. 同步更新 `docs/godot/M5_HANDOFF.md` 与 `docs/godot/RELEASE.md` 的最终提交 SHA、包体大小与 SHA-256 校验和。
2. 确认分发形态（独立 `.zip`/`.7z` 压缩包，或通过 itch.io / 安装器分发以降低 Windows SmartScreen 误报），完成一次干净目录的双击冷启动最终复测后打 Tag 封板。
