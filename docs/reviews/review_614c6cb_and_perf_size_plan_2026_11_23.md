# LWSMF 最新代码库审阅（Commit `614c6cb2`）与性能降耗/三端瘦身本地 Agent 施工单

**审阅基线**：commit `614c6cb2383ef8730d076fb0ed3828671228e266`（对比 `78c4e49a`）  
**配套输入**：`lwsmf_perf_and_multiplatform_size_guide.md`  
**报告用途**：上半部分给出 `614c6cb2` 增量审阅与遗留收口项；下半部分将性能降耗与三端包体瘦身调研逐条映射到本项目真实代码位置，剔除已做项与项目专属坑点，形成可直接交由本地 Agent 按批次执行的施工单。

---

## 第一部分：最新代码库（`614c6cb2`）审阅结论

### 1. 前序 8 项问题（S-01～S-08）验收表

| 编号 | 结论 | `614c6cb2` 精确证据（`file:line`） | 说明与残余 |
| :--- | :--- | :--- | :--- |
| **S-01 / S-02** | **通过** | `VmController.cs:345-349`、`GameSessionController.cs:37-39`、`GameRoot.gd:1912-1921` | 新增 `GetCommonEventSelf(eventId, argIndex)` 自动加 `Magic.SelfArgBase (600)`；`GetSaveSlotMode()` 改读 `600` 并叠加 `phase == "title"`；`row`/`page`/`per_page` 改读 `610`/`611`/`627`，彻底根治标题页误弹覆盖与跨页槽号错乱 |
| **S-03** | **部分通过** | `GameSession.cs:724-776`、`GameRoot.gd:1935-1958` | 已废弃单帧点击脉冲，改为确定性直调 `SaveGameToSlot(slot)`（写槽 + `System.json` + `CE94` 重绘）与 `LoadGameFrom`；但存在 1 处缩略图写盘时序与缓存刷新隐患（见下方 **R-P1-1**） |
| **S-04** | **通过** | `PlayOverlays.gd:287-299`、`GameRoot.gd:1452-1454`、`InputRouter.gd:337-338` | `PlayOverlays._input` 已统一拦截右键与 `Esc`；`_release_modal_guard_if_free()` 已补齐 7 个键盘键释放检查；`InputRouter` 保护期间同步掩掉 `MASK_OK \| MASK_CANCEL` |
| **S-05** | **通过** | `PlayOverlays.gd:753-900` | 已落地姊妹作风格深底白框大弹窗，完整实现标题/游戏内 × 空槽/有档 4 象限交互矩阵 |
| **S-06** | **通过** | `GameSession.cs:543-545, 913-916` | `CanSaveMapState()` 已放行地图事件期（含晚饭后 `CE435` 自动存档）落盘，并补充 `[AutoSave]` 状态日志 |
| **S-07** | **通过** | `project.godot:29`、`UiSkin.gd:109-165, 197-235` | 已开 `window/dpi/allow_hidpi=true`、补全 `Lolita.ttf`/`media.pak` 搜索路径，并设置 `oversampling=0.0`、`HINTING_NORMAL`、`FONT_ANTIALIASING_GRAY`、`generate_mipmaps=false` |
| **S-08** | **部分通过** | `PictureStage.gd:47, 55-151, 536` | 仍依赖字体 `get_string_size("■"/"□")` 取 `maxf` 后调 `draw_string()`，**未改为纯几何 `draw_rect()`**，因此面对底层 PNG 底框或不同字体留白时第 4/5 格仍偏移（见下方 **R-P1-2**） |
| **Android 瘦身新增** | **通过** | `Lwsmf.csproj:5-6`、`tools/mobile/strip_android_static_libs.py`、`postprocess_android_apk.py`、`package_android.sh:232-243` | 已新增 Android `InvariantGlobalization`/`SatelliteResourceLanguages`、剔除 `lib/*.a`、16 KB `zipalign` 与 `apksigner` 重签；`test_mobile_tools.py` 离线单测全部通过 |

### 2. 本轮增量新发现的 2 项待修问题（归入第 0 批立即收口）

* **R-P1-1（`GameRoot.gd:1935-1944`、`AssetDB.gd`）— 存档缩略图写盘早于 `SaveGameToSlot` 且未刷新 `AssetDB` 缓存**：
  * `_on_save_slot_save_requested()` 先执行 `_on_pro_feature(0, 0, "Save/save%d.jpg" % slot)` 覆盖旧图，再调 `session.SaveGameToSlot(slot)`；若保存失败会污染旧缩略图；保存成功后若 `AssetDB` 已缓存旧 `Save/saveN.jpg`，`CE94` 重绘预览仍会显示旧图。
  * **修复**：先成功执行 `SaveGameToSlot(slot)` 后再写 `Save/save%d.jpg`，并在 `_on_pro_feature(kind=0/3)` 写删 JPG 后调用 `assets.invalidate_path("Save/save%d.jpg" % slot)` 清除纹理缓存。
* **R-P1-2（`PictureStage.gd:64-151`）— 格子进度条 `■`/`□` 改纯几何 `draw_rect()` 彻底根治偏移**：
  * 放弃用字体绘制 `■`/`□`，改为在 `_measure_run_width()` 与 `PictureTextLabel._draw()` 中对 `■`/`□` 采用固定整数全角步进 `step = float(px)`（若经 `F6` 确认底框为 PNG 贴图则对齐底图精确格距），用 `draw_rect()` 直接绘制实心/空心方框，彻底消除第 4/5 格后的累积偏差。

---

## 第二部分：`lwsmf_perf_and_multiplatform_size_guide.md` 现状对照体检

在交给本地 Agent 施工前，已逐条核对调研指南与 `614c6cb2` 源码的真实差距，避免重复开发或踩坑：

| 调研建议项 | 当前代码现状（`614c6cb2`） | 判定 | 本项目专属注意/坑点 |
| :--- | :--- | :--- | :--- |
| **锁定 60 FPS + 档位切换** | `GameRoot.gd:16, 1668` 已有 `[60, 30, 15]` 档位；`project.godot` 尚未设 `application/run/max_fps=60` | 🟡 部分落地 | 补上 `project.godot` 默认 `60` 即可防首帧/引导期高刷空转；**切勿影响 `PerformanceFrameProbe.gd` 的 `Engine.max_fps = 0`** |
| **失焦 / 最小化降频** | `GameRoot.gd:876-879` 失焦仅清输入，未改 `Engine.max_fps` | ❌ 未落地 | Headless 测试与无头回归必须跳过降频；恢复焦点时按 `_perf_tier` 还原，不要硬编码 60 |
| **静止态 `low_processor_usage_mode`** | 全仓未使用 | ❌ 未落地 | 必须仅在**非 headless、无过渡动画、无打字机推进、无特效/淡入淡出、等待玩家点击**时才开启，输入瞬间立即关闭，否则会让 60Hz `PictureStage.tick()` 卡顿 |
| **关闭 2D 物理引擎** | 全仓未调 `PhysicsServer2D.set_active(false)` | ❌ 未落地 | 本作碰撞全在 `MapRuntime.cs`，可直接于 `GameRoot._ready()` 开头关闭，零副作用 |
| **空图片队列跳过 JSON 解析** | `GameRoot.gd:240-241` 已实现 `if queued.is_empty(): return true` | ✅ **已落地** | 无需重做 |
| **消除 `PictureStage.gd` 深拷贝** | `PictureStage.gd` 仍有 **15 处 `.duplicate(true)`**（`:314, 394, 445, 823, 1045, 1086, 1106-1125, 1167-1174, 1205-1211`） | ❌ 未落地 | `pose`/`flags` 全是扁平基础类型，直接改浅拷贝 `.duplicate()` 即可立降 GC；`effects` 数组按需复制 |
| **释放 `ProgramLoader.cs` 50 MB `JsonDocument`** | `ProgramLoader.cs:46-52` 仍长期缓存完整 `JsonDocument` | ❌ 未落地 | 改为一次性解析公共事件/地图索引到轻量结构后立即 `Dispose()`，内存立降 50–100 MB |
| **`MediaPakReader.cs` 用 `ArrayPool<byte>` 降 LOH** | `MediaPakReader.cs:78` 每次 `new byte[length]` | ❌ 未落地 | 返回给 GDScript 仍需 `byte[]`，建议内部解密与 `GodotFileAccessStream` 复用池化缓冲，并新增直接返回 `Godot.Image` 的 C# 方法跳过 GDScript 中间 `PackedByteArray` 拷贝 |
| **Workstation GC + 全平台 `.NET` 精简** | `Lwsmf.csproj:5-6` 仅给 Android 开 `InvariantGlobalization` 与 `SatelliteResourceLanguages`；缺 GC 配置 | 🟡 部分落地 | 去掉 Android 条件限制推广至三端（先跑全量 C# 门禁验证无中文日期/区域依赖），并加 `ServerGarbageCollection=false`、`ConcurrentGarbageCollection=true` |
| **三端分级 LRU 显存上限** | `AssetDB.gd:17-20` 已设 512 / 128 / 64 MiB | ✅ **已落地** | 可选将桌面端从 512 下调至 384 MiB |
| **单色图 `FORMAT_L8` 降 75% 显存** | `AssetDB.gd` 尚未启用 | ⚠️ **高风险慎做** | Godot 4 `gl_compatibility`（GLES3）下 `FORMAT_L8` 底层走 `GL_R8`，若不验证纹理 swizzle 可能导致单色图发红；须先做单图实机验证 |
| **Android `.cfb` 零拷贝直读** | `MediaPakReader.cs` 已有 `GodotFileAccessStream`，但 `MobileDataBootstrap.gd` 首启仍将 `media.pak` 复制到 `user://` | 🟡 部分落地 | 让 `AssetDB.gd` 在 Android 直接打开 `res://mobile_data/work/all/media.pak.blob.cfb`，`MobileDataBootstrap` 跳过复制 `media.pak`，手机实占空间直接砍半 |
| **iOS `xcodebuild` 符号剥离 + IPA 分流压缩** | `ios_inject_data.py:23, 90-94` **已落地**分流压缩；`package_ios.sh:127` **缺** `STRIP_*` 参数 | 🟡 部分落地 | 仅需给 `package_ios.sh:127` 补 4 个 `xcodebuild` 剥离参数，零风险省 20%–40% 主程序体积 |
| **Windows Inno Setup LZMA2 安装器** | `package_release.sh:299` 仅产出普通 `zip`（`compresslevel=6`） | ❌ 未落地 | 新增可选 `--installer` 参数生成 `.iss` 并调 `ISCC.exe`（`lzma2/ultra64` + `SolidCompression=yes` + `media.pak` `Flags: nocompression`） |
| **`game_data/*.json` → `.json.br` + 字体子集化** | 当前为明文 `.json`（20.55 MB）与完整 `Lolita.ttf` | ❌ 未落地 | C# `.NET 8` 原生支持 `BrotliStream` 零第三方依赖；`pyftsubset` 需从 `programs.json` + `database.json` + UI 脚本提取全量字符集并保留常用标点/数字防缺字 |
| **媒体无损转码（WebP Lossless / OxiPNG / FLAC）** | `pack_media.py` 仅有有损 `--slim` | ⚠️ **重要纠偏** | **切勿把原版 `.ogg`/`.mp3` 音频转 FLAC**（原版已有损压缩，转 FLAC 体积会暴涨 3–5 倍且零音质提升！仅对原版 `.wav` 考虑 FLAC）；图片可安全做 WebP Lossless vs OxiPNG 竞价 |
| **定制 2D/GLES3 + Mono 导出模板** | 使用官方全功能模板 | ❌ 未落地（Phase 4） | 收益大（−20～50 MB/端）但需维护三端 SCons 编译流水线，放在最后独立推进 |

---

## 第三部分：本地 Agent 分阶段落地施工单（按 ROI 排序）

### Phase 0 — 立即收口 `614c6cb2` 残余（预计 0.5 小时，零风险）

1. **`GameRoot.gd:1935-1944` + `AssetDB.gd`（R-P1-1）**：
   * 调整 `_on_save_slot_save_requested()` 顺序：先 `var ok = session.SaveGameToSlot(slot)`，仅在 `ok` 为真时调用 `_on_pro_feature(0, 0, "Save/save%d.jpg" % slot)` 并在写盘成功后调用 `assets.invalidate_path("Save/save%d.jpg" % slot)`（给 `AssetDB.gd` 新增 `invalidate_path(logical)`，同时清除 `tex:` 缓存键并扣减 `used_bytes`），再补一次 `session.RefreshSaveUiForSlot(slot)` 刷新缩略图。
2. **`PictureStage.gd:64-151, 536`（R-P1-2）**：
   * 将 `■`/`□` 进度条字符从字体 `draw_string()` 改为纯几何 `draw_rect()` 绘制，统一固定步进 `step = float(px)`（实机按 `F6` 核对警戒度底框是否为 PNG；若是 PNG 则按底图每格精确像素周期对齐）。

### Phase 1 — 运行时零风险降耗与内存瘦身（预计 2–3 小时，立竿见影）

> **验收门禁**：跑通全量 `tools/godot_check.sh`、`PerformanceFrameProbe.gd`、149,245 步 Oracle 确定性对比（`needs-runtime`）。

1. **主循环与物理降耗**：
   * `godot/project.godot`：`[application]` 增加 `run/max_fps=60`。
   * `godot/presentation_gd/GameRoot.gd`：
     * `_ready()` 首行增加 `PhysicsServer2D.set_active(false)`。
     * `_notification()` 监听 `NOTIFICATION_APPLICATION_FOCUS_OUT` / `NOTIFICATION_WM_WINDOW_FOCUS_OUT`（非 headless 时 `Engine.max_fps = 10`）、`NOTIFICATION_WM_WINDOW_MINIMIZED`（`Engine.max_fps = 5`）、`NOTIFICATION_APPLICATION_FOCUS_IN` / `NOTIFICATION_WM_WINDOW_FOCUS_IN`（调用 `_apply_performance_tier()` 恢复）。
     * 在 `_process()` 中对纯静态等待状态（`not _is_headless()` 且 `message.is_open() and message.finished` 且无活跃转场/图片补间/290 动画）动态切换 `OS.low_processor_usage_mode = true`，有任何输入事件或状态变化立即设回 `false`。
2. **消除 `PictureStage.gd` 高频字典深拷贝**：
   * 将 `godot/presentation_gd/PictureStage.gd:314, 394, 445, 823, 1045, 1086, 1117-1125, 1167-1174, 1205-1211` 中所有扁平字典（`pose`/`target`/`current`/`flags`）的 `.duplicate(true)` 改为浅拷贝 `.duplicate()`；对 `effects` 数组仅在非空时复制元素。
3. **释放 `ProgramLoader.cs` 50 MB 常驻 `JsonDocument`**：
   * 重构 `godot/core_cs/ProgramLoader.cs:20-52`：加载 `programs.json` 时一次性解析为紧凑的 `Dictionary<int, Program>`（公共事件）与 `Dictionary<string, MapDefinition>`（地图事件），解析完毕后在 `using (var doc = JsonDocument.Parse(stream))` 块尾立即释放 `JsonDocument`，不再常驻 `Documents` 静态缓存。
4. **`Lwsmf.csproj` GC 与全平台 `.NET` 精简**：
   * `godot/Lwsmf.csproj` 的 `<PropertyGroup>` 中：
     * 增加 `<ServerGarbageCollection>false</ServerGarbageCollection>` 与 `<ConcurrentGarbageCollection>true</ConcurrentGarbageCollection>`；
     * 将 `<InvariantGlobalization>true</InvariantGlobalization>` 与 `<SatelliteResourceLanguages>zh-Hans;en</SatelliteResourceLanguages>` 移除 Android 条件推广到全平台，随后跑完整 C# 测试套件确认无区域格式化回归。
5. **`MediaPakReader.cs` 降低 LOH 分配**：
   * 在 `godot/presentation/MediaPakReader.cs:76-81, 350-375` 使用 `ArrayPool<byte>.Shared.Rent()` 进行分块读取与 XOR 解密（并确保 `finally` 归还）；为 `AssetDB.gd` 纹理加载热路径新增 `ReadImage(string path)` 或直接复用缓冲，避免大图 2× 超分读取反复触发 Gen2 LOH GC。

### Phase 2 — 三端包体高 ROI 无损瘦身（预计 3–4 小时，严格无损）

1. **Android 零拷贝直读 `media.pak.blob.cfb`（手机实占空间直接砍半）**：
   * `godot/presentation_gd/MobileDataBootstrap.gd`：跳过把 `work/all/media.pak.blob.cfb` 复制到 `user://`（仅解包小体积的 `game_data/*.json` 与 `BasicData/*.project`）。
   * `godot/presentation_gd/AssetDB.gd`：Android 下直接以 `MediaPakReader.Open("res://mobile_data/work/all/media.pak.blob.cfb")` 随机 seek 读取（复用已有的 `GodotFileAccessStream`），前提是继续保持 `android_data.py` 对 `.cfb` 的 `ZIP_STORED` 校验。
2. **iOS `xcodebuild` 符号剥离**：
   * `tools/package_ios.sh:127`：在 `xcodebuild` 命令中追加 `STRIP_INSTALLED_PRODUCT=YES`、`DEPLOYMENT_POSTPROCESSING=YES`、`COPY_PHASE_STRIP=YES`、`DEBUG_INFORMATION_FORMAT=dwarf`，并确保 `export_presets.cfg` 中 `dotnet/include_debug_symbols=false`。
3. **`game_data/*.json` 离线 Brotli 压缩（20.55 MB → ~3 MB）**：
   * 在打包脚本（`package_release.sh`、`android_data.py`、`ios_inject_data.py`）中可选将 `programs.json` / `database.json` / `map_tiles.json` 压为 `.json.br`（quality 11）；
   * 在 `ProgramLoader.cs`、`GameDatabase.cs`、`MapTileData.cs` 中优先探测同名 `.json.br`，存在时用 .NET 8 内置 `System.IO.Compression.BrotliStream` 流式解压，不存在时回退读取明文 `.json`（开发态零影响）。
4. **Windows Inno Setup LZMA2 安装器脚本**：
   * 新增 `tools/installer/lwsmf.iss.tmpl` 并在 `tools/package_release.sh` 增加 `--installer` 开关：设置 `Compression=lzma2/ultra64`、`SolidCompression=yes`、`LZMADictionarySize=65536`，并对 `work/all/media.pak` 与 `work/all/media_hd.pak` 单独加 `Flags: nocompression`（只固实压缩 `Lwsmf.exe` + `.NET 8` DLL + `game_data`）。

### Phase 3 — 媒体与字体深度无损压缩（预计 2–3 小时，按需开启）

1. **白名单剔除死资产**：在 `tools/pack_media.py` 增加 `--prune-from docs/release/ASSET_AUDIT.md`（或机器可读白名单），打包时仅收入 2,689 个可达活体素材（直接省 148.60 MB）。
2. **严格无损图片竞价**：在 `tools/pack_media.py` 新增 `--lossless-opt` 模式，对 PNG 做 `WebP Lossless`（`cwebp -lossless -z 9`）与 `oxipng -o 4` 择小保存（保持与现有 `AssetDB.gd` 零代码改动兼容）；**音频方面仅对原版 `.wav` 转码或保留原版 `.ogg`/`.mp3`，严禁把原版有损音频转成 FLAC**。
3. **字体无损子集化**：编写 `tools/subset_font.py`，扫描 `programs.json`、`database.json` 与 `godot/presentation_gd/*.gd` 提取全部出现字符 + ASCII + 常用中文标点，用 `fonttools pyftsubset` 生成子集 `Lolita.ttf` 并自动跑字符覆盖率断言（确保 0 缺字）。

### Phase 4 — 长期基建项（高维护成本，独立里程碑）

1. **定制 2D/GLES3 + Mono 导出模板**：在 GitHub Actions 增加独立 workflow 编译并缓存关闭 3D/Vulkan/D3D12/Metal/Physics3D/Nav3D/AccessKit 的 Windows / Android / iOS 模板（每端再减 20–50 MB）。
2. **`FORMAT_L8` 单色显存优化实验**：仅在 `gl_compatibility` 实机验证 OpenGL ES 3.0 单通道纹理采样不会变红且与 `PictureStage` 色调 Shader 兼容后，再在 `AssetDB.gd` 启用灰度图 `FORMAT_L8` 路径。

---

## 第四部分：给本地 Agent 的验收检查清单

- [ ] **Phase 0 验收**：保存失败时不覆写旧缩略图；保存成功后左下角预览立即更新为新截图；“警戒度”10 格进度条 0 像素错位（`needs-runtime`）。
- [ ] **Phase 1 验收**：`tools/godot_check.sh` 全绿；`PerformanceFrameProbe.gd` 平均帧时间下降且无回归；窗口失焦时 `F3` 帧率降至 10 FPS、切回立刻恢复 60 FPS；任务管理器托管内存下降 ≥ 50 MB（`needs-runtime`）。
- [ ] **Phase 2 验收**：三端启用 `InvariantGlobalization` 后 C# 全量单测与 `M4FullRouteTraceTest` 全绿；Android 首启无需复制 `media.pak` 至 `user://` 即可正常进标题与游玩；`.json.br` 与明文 `.json` 双路径读入一致；Windows Inno Setup 与 iOS `STRIP_*` 产物校验通过（`needs-runtime`）。
