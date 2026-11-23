# LWSMF（Godot 4.7.2 Mono / .NET 8）运行时降耗与三端包体极致瘦身指南

> **适用项目**：`Living With Sister: Monochrome Fantasy`（Godot 4.7.2-stable .NET + .NET 8.0 SDK + GDScript，`gl_compatibility` 渲染后端）  
> **优化红线**：① 画面与玩法表现**完全不变**；② 游戏媒体素材保持**严格无损**；③ 兼顾 **Windows 安装程序（`.exe`）**、**Android（`.apk`）** 与 **iOS（`.ipa`）** 三端交付。

## 核心结论总览

| 优化方向 | 核心抓手 | 预期收益 | 对表现/画质影响 |
| :--- | :--- | :--- | :--- |
| **CPU / GPU / 发热功耗** | 锁定 60 FPS + 失焦降频 + 静止态低功耗模式 + 关闭无用 2D 物理 | 桌面端/移动端功耗直降 **50%–80%**，消除高刷屏空转发热 | **零变化**（逻辑本就是 60 Hz） |
| **内存与 GC 卡顿** | 释放常驻 50 MB `JsonDocument` + `ArrayPool` 避开 LOH + Workstation GC | 托管内存立降 **50–100 MB**，消除大图读取与字典深拷贝引发的 GC 掉帧 | **零变化**（且帧时间更稳） |
| **显存占用（兼容超分）** | 三端分级 LRU 上限（Win 256–384 / Android 128 / iOS 64 MB）+ 单色图保 L8 通道 | 防止 2× 超分后移动端 OOM，单色图显存直降 **75%** | **零变化** |
| **三端包体体积** | 通用层（去 149 MB 死资产 + 2D Mono 模板 + Brotli JSON + 无损图音）+ 三端封装优化（Inno LZMA2 / APK 剥离与直读 / IPA 剥离符号与分流压缩） | 基准包无损先省 **~272 MB（−29%）**；三端安装器/安装包再各自压缩 **15%–35%** | **严格无损** |

---

## 一、表现不变的前提下：降低游玩时性能占用（CPU / GPU / 内存 / 显存）

### 1. 渲染与主循环降耗：消除高刷空转与无效重绘

本项目 `GameRoot.gd` 采用固定步进（`STEP = 1/60s`）驱动虚拟机，但 `project.godot` 尚未限制帧率或开启低功耗策略，导致在 144Hz/240Hz 显示器或关闭 VSync 的设备上 GPU 仍会白跑数百 FPS。

- **锁定逻辑匹配帧率**：在 `project.godot` 设置 `application/run/max_fps=60`（桌面端开启 VSync）；移动端默认 `60`，并可提供 30/60 fps 省电切换（类似姊妹作 `F5` 档位）。对于 60Hz 逻辑的 2D 游戏，超过 60 FPS 没有任何视觉增量。
- **窗口失焦 / 最小化自动降频**：参考 [Godot Proposal #2001](https://github.com/godotengine/godot-proposals/issues/2001)，在 `GameRoot.gd` 的 `_notification()` 中监听失焦与最小化——切出窗口查攻略或挂机时将 `Engine.max_fps` 降至 `10`（最小化降至 `2–5`），焦点恢复瞬间切回 `60`，后台 CPU/GPU 占用几乎归零。
- **静态对话期动态启用低功耗模式**：对话等待点击、菜单静止且无补间动画/打字机推进时，临时开启 `OS.low_processor_usage_mode = true`，仅在有输入或画面变化时重绘，笔记本风扇噪音和手机发热会大幅下降。
- **关闭闲置的 Godot 2D 物理引擎**：本作地图碰撞由 `MapRuntime.cs` 自主计算，不依赖 Godot 2D 物理节点。在启动时调用 `PhysicsServer2D.set_active(false)`，省去每帧物理世界步进开销。

### 2. 减少 C# ↔ GDScript 跨语言开销与每帧堆分配

当前代码有两个隐蔽的 CPU/GC 热点：`GameRoot.gd` 每帧从 `VmController.cs` 拉取 `TakePicturesJson` 并调用 `JSON.parse_string()`；`PictureStage.gd` 在 `draw_picture` 与 `_merged_move` 中高频调用 `.duplicate(true)` 深拷贝状态字典。

- **空队列零分配快路径**：`VmController.TakePicturesJson()` 在本帧无图片变更时直接返回 `""`，GDScript 侧 `if queued.is_empty(): return` 直接跳过 `JSON.parse_string()`。
- **消除 `PictureStage.gd` 字典深拷贝**：将 `_states.get(number, {}).duplicate(true)` 与 `flags.duplicate(true)` 改为按字段增量比对和浅拷贝，大幅减少 GDScript 短期字典分配。
- **高频位移动画用平坦数组替代 JSON**：若战斗或 H 场景出现高频连续图片移动/透明度插值，可将批量坐标/透明度改用 `PackedInt32Array` / `PackedFloat32Array` 传给 GDScript，彻底免去每帧 JSON 编解码与字符串垃圾。

### 3. Godot-Mono (.NET 8) 专属内存与 GC 调优

- **立即释放 `ProgramLoader.cs` 常驻的 50 MB `JsonDocument`**：当前 `ProgramLoader.cs` 把 `programs.json` 的完整 `JsonDocument` 长期缓存在静态字典 `Documents` 中，导致常驻内存虚高 50–100 MB。改为启动时一次性解析为紧凑的 `Slot[]` 数组（或按事件 ID 懒加载）后立刻 `Dispose()` 原始 `JsonDocument`，运行时零性能回退、内存立降。
- **用 `ArrayPool<byte>` 消除 `MediaPakReader.cs` 的大对象堆（LOH）抖动**：当前 `MediaPakReader.Read()` 每次读图都执行 `new byte[(int)entry.Size]`。在 `.NET` 中，大于等于 **85 KB** 的数组会直接进入 **LOH（大对象堆）**，只能靠昂贵的 Gen2 STW GC 回收；未来做 2× 超分后每张图都是几 MB，翻几页 CG 就会触发卡顿。改用 `ArrayPool<byte>.Shared.Rent()` 借用缓冲做 XOR 解密，传给 Godot 后立即 `Return()`，可将 LOH 分配压至接近于零。
- **配置 Workstation GC 与内存保守模式**：在 `godot/Lwsmf.csproj` 显式锁定客户端友好的 Workstation GC（参考 [Microsoft .NET GC 配置文档](https://learn.microsoft.com/en-us/dotnet/core/runtime-config/garbage-collector)）：
  ```xml
  <PropertyGroup>
    <ServerGarbageCollection>false</ServerGarbageCollection>
    <ConcurrentGarbageCollection>true</ConcurrentGarbageCollection>
  </PropertyGroup>
  ```
  移动端可额外配合 `System.GC.ConserveMemory` 控制堆膨胀，减少被系统后台杀进程的概率。

### 4. 显存（VRAM）与解码优化（兼顾未来 2× 超分与无损音频）

- **单色/灰度图保持单通道格式（显存直降 75%）**：`ImageTexture.create_from_image()` 在 GPU 侧按像素格式占显存。本作含大量单色漫画风立绘与 CG，若以 8-bit 灰度 PNG 存储且不强转 `RGBA8`，Godot 可按 `FORMAT_L8`（1 字节/像素）上传显存，仅为 `RGBA8`（4 字节/像素）的 **1/4**。
- **三端分级纹理缓存上限**：在 `AssetDB.gd` 中按平台设置 LRU 预算——桌面端 `256–384 MiB`、Android `128 MiB`、iOS `64 MiB`（参考姊妹作在 iPhone XR 3 GB 内存机型上的实测上限）；跨场景或换日时主动清理非活跃 CG 缓存。
- **切曲即释放 + 后台线程预解码**：无损/高质量 BGM 只在内存保留当前播放的一首，切换即释放旧流；大图或 FLAC BGM 解码放入后台线程，避免阻塞主线程导致 16.6 ms 帧预算超时。

---

## 二、减少三端包体体积（公共层 + Windows / Android / iOS 专项）

### 0. 三端共享的通用无损瘦身基线（先做这 4 项，全端同步减重）

1. **白名单剔除死资产（−148.60 MB）**：按 `docs/release/ASSET_AUDIT.md` 仅打包 2,689 个可达活体素材（保留 `1.4.0/` 与 `Kagura/`），先剔除 1,215 个死资产，避免给废图做超分。
2. **定制 2D/GLES3 + Mono 导出模板（每端 −20 至 −50 MB）**：参考 [Godot 4.7 体积优化文档](https://docs.godotengine.org/en/4.7/engine_details/development/compiling/optimizing_for_size.html) 与 [.NET 编译指南](https://docs.godotengine.org/en/4.7/engine_details/development/compiling/compiling_with_dotnet.html)，编译关闭 `vulkan`、`d3d12`、`metal`、`disable_3d=yes`、`disable_physics_3d=yes`、`disable_navigation_3d=yes`、`accesskit=no`、`sdl=no` 且开启 `production=yes module_mono_enabled=yes` 的自定义模板（避开 `deprecated=no`）。
3. **`.NET 8` 运行时无损精简（每端 −15 至 −25 MB）**：在 `godot/Lwsmf.csproj` 启用 `<InvariantGlobalization>true</InvariantGlobalization>`（去掉庞大的 ICU 全球化数据）与 `<SatelliteResourceLanguages>zh-Hans;en</SatelliteResourceLanguages>`（剔除十余种无关语言卫星程序集），并确保 `dotnet/include_debug_symbols=false`、`dotnet/include_scripts_content=false`。（注：激进 `PublishTrimmed` 在 Godot 中存在反射崩溃风险，见 [Godot Issue #92268](https://github.com/godotengine/godot/issues/92268)，除非逐一白名单验证否则不建议硬开。）
4. **结构化数据与媒体无损压缩**：`game_data/*.json`（20.55 MB）离线压为 `.json.br`（.NET 8 内置 `BrotliStream` 读取，降至 **~3 MB**）；`Lolita.ttf` 用 `pyftsubset` 按实际字符集无损子集化（省 60%–80%）；图片走 **WebP Lossless / OxiPNG** 竞价、音频走 **FLAC**（C# 解码喂给 `AudioStreamWAV`），封装进支持逐条 Zstd 的 `media.pak v2`。

---

### 1. Windows 端 `.exe` 安装程序专项瘦身

| 优化项 | 推荐配置 / 做法 | 效果与原因 |
| :--- | :--- | :--- |
| **优先选用 Inno Setup（LZMA2 固实压缩）** | `Compression=lzma2/ultra64`、`SolidCompression=yes`、`LZMADictionarySize=65536`（参考 [Inno Setup Compression 文档](https://jrsoftware.org/ishelp/topic_setup_compression.htm)） | 比普通 ZIP 再小 **15%–25%**；**核心优势**：LZMA2 对高熵已压缩数据几乎零膨胀（~0.005%），且**突破 NSIS 默认 2 GB 单包上限**（未来 2× 超分 + FLAC 极可能接近或超过 2 GB）。 |
| **若包体小于 2 GB 也可用 NSIS 固实 LZMA** | 在所有 `File` 指令前加 `SetCompressor /SOLID lzma`（参考 [NSIS SetCompressor 文档](https://nsis.sourceforge.io/Reference/SetCompressor)） | 安装器自身壳体仅约 34 KB；跨文件固实压缩对 `Lwsmf.exe` + 186 个高度相似的 `.NET 8` DLL 压缩率极高。 |
| **对 `media.pak` 关闭二次压缩** | Inno Setup 中给 `media.pak` 单独加 `Flags: nocompression` | `media.pak` 内部已完成 Zstd 压缩并经过 XOR 混淆（呈随机噪声态），再压几乎省不了字节却极度拖慢打包与安装解压速度。把固实压缩全留给 `Lwsmf.exe`、`.NET 8` DLL 与 `game_data`。 |
| **清理非必要附带文件** | 确认 0 `.pdb`、排除 `tests/*` / `addons/*` / `mobile_data/*`；如面向普通玩家可不随包附带 `converter/` | 保持分发目录干净，减少安装器文件表体积。 |

---

### 2. Android 端 `.apk` 包专项瘦身

| 优化项 | 推荐配置 / 做法 | 效果与原因 |
| :--- | :--- | :--- |
| **严格单 ABI（仅 `arm64-v8a`）** | 保持 `architectures/arm64-v8a=true`，其余全部 `false` | 每多勾一个架构就会多打包一整份 `libgodot_android.so` + `.NET` 原生运行时（+40–60 MB）。 |
| **定制 2D Mono Android 模板 + Strip** | 用 `lto=thin`（避开 NDK r30 full LTO 链接崩溃）+ `module_raycast_enabled=no` + `disable_3d=yes` 编译模板，并用 `llvm-strip --strip-unneeded` 剥离 `.so` 调试符号 | `libgodot_android.so` 体积可降约 20–30%，APK 直接省 **~15–25 MB**。 |
| **APK 内 `.cfb` 零拷贝直读** | 保持 `media.pak.blob.cfb` 在 APK 中为 `ZIP_STORED`（不压缩），将 `MediaPakReader.cs` 的 `FileStream` 改为 Godot `FileAccess` 随机 seek | 虽然不减少 APK 下载体积，但**避免首启把整包复制到 `user://`**，手机实际占用空间直接砍半（对 1 GB+ 高清包尤为致命）。 |
| **原生库与字体精简** | 关闭 `.pdb`（`DebugType=none`）、启用 `InvariantGlobalization`；移动端用系统 `Noto Sans CJK SC` 或子集化后的 `Lolita.ttf` | 减少 `.NET` 依赖与完整中文字体带来的 10–20 MB 冗余。注：若未来计划上架 Google Play，需留意 [Godot Issue #110262](https://github.com/godotengine/godot/issues/110262)（Android 16 KB 页对齐需升级至 `.NET 9`）。 |
| **高清包体分离（超分版备选）** | 若未来 2× 超分 + FLAC 超过 1.5–2 GB，改为“精简引擎 APK（~60–90 MB）+ 外置高清 `media.pak` 导入/下载” | 规避 Android 低配机解析超大单 APK 安装失败、签名校验极慢与存储翻倍问题。 |

---

### 3. iOS 端 `.ipa` 包专项瘦身

iOS 强制 AOT 静态编译（不允许 JIT），且 IPA 本质是一个 ZIP 归档，瘦身重点在 **AOT 二进制剥离** 与 **IPA 分流压缩**：

| 优化项 | 推荐配置 / 做法 | 效果与原因 |
| :--- | :--- | :--- |
| **强制剥离 Xcode 调试符号与 dSYM** | 在 `tools/package_ios.sh` 的 `xcodebuild` 命令追加：`STRIP_INSTALLED_PRODUCT=YES`、`DEPLOYMENT_POSTPROCESSING=YES`、`COPY_PHASE_STRIP=YES`、`DEBUG_INFORMATION_FORMAT=dwarf`，并确保 `dotnet/include_debug_symbols=false` | 避开 [Godot Issue #85612](https://github.com/godotengine/godot/issues/85612) 中 AOT 附带调试符号导致的二进制虚胖，通常可直接削减 **20%–40%** 的主程序体积。 |
| **定制 2D Mono iOS 导出模板** | 在 `macos-latest` CI 编译并缓存关闭 3D/XR/Navigation/Physics3D 的 `ios.zip` 模板 | 显著缩小 `libgodot.ios.template_release.arm64.a` 静态库体积，同时缩短 CI 链接耗时。 |
| **`.NET` iOS AOT 精简** | 在 `Lwsmf.csproj` 针对 iOS 启用 `InvariantGlobalization=true`、`SatelliteResourceLanguages=zh-Hans;en`，并仅保留 `arm64` 真机切片（不含模拟器切片） | 减少 AOT 编译进主二进制的托管框架体积。 |
| **IPA 分流压缩（`inject_ios_data.py`）** | 注入数据时：对已压缩/高熵的 `media.pak` 与 `.json.br` 用 `ZIP_STORED`；对 `.app` 主二进制、`.pck`、`.plist` 用 `ZIP_DEFLATED`（`compresslevel=9`） | 既把主程序与元数据的 ZIP 体积压到极限，又避免对高熵媒体包做无效 DEFLATE 压缩，大幅加快 Sideloadly 重签名与传输装机速度。 |
| **高清包外置导入（超分版备选）** | 利用预设已开启的 `accessible_from_files_app=true`，支持先侧载轻量化引擎 IPA（~50–80 MB），再通过 iOS「文件」App 或 iTunes 文件共享直接拷入高清 `media.pak` | 彻底解决免费 Apple ID 每 7 天用 Sideloadly 重签一次 1.5 GB+ 大包耗时过长甚至超时失败的痛点。 |

## 参考来源

1. 本项目源码与文档：`godot/presentation_gd/GameRoot.gd`、`godot/presentation_gd/PictureStage.gd`、`godot/presentation_gd/AssetDB.gd`、`godot/presentation/VmController.cs`、`godot/presentation/MediaPakReader.cs`、`godot/core_cs/ProgramLoader.cs`、`docs/godot/RELEASE.md`、`docs/release/ASSET_AUDIT.md`。
2. [Optimizing a build for size — Godot Engine 4.7 Documentation](https://docs.godotengine.org/en/4.7/engine_details/development/compiling/optimizing_for_size.html)
3. [Compiling with .NET — Godot Engine 4.7 Documentation](https://docs.godotengine.org/en/4.7/engine_details/development/compiling/compiling_with_dotnet.html)
4. [Add a FPS limit for unfocused and minimized windows (#2001) — godot-proposals](https://github.com/godotengine/godot-proposals/issues/2001)
5. [Garbage collector config settings — .NET / Microsoft Learn](https://learn.microsoft.com/en-us/dotnet/core/runtime-config/garbage-collector)
6. [Godot builds normally with PublishTrimmed but crashes when run (#92268) — godotengine/godot](https://github.com/godotengine/godot/issues/92268)
7. [Godot 4.2 C# generate and attach Debug Symbols Always in AOT (#85612) — godotengine/godot](https://github.com/godotengine/godot/issues/85612)
8. [Gradle generated Android dotnet exports are not 16kb page aligned (#110262) — godotengine/godot](https://github.com/godotengine/godot/issues/110262)
9. [Reference/SetCompressor — NSIS Documentation](https://nsis.sourceforge.io/Reference/SetCompressor)
10. [[Setup]: Compression — Inno Setup Help](https://jrsoftware.org/ishelp/topic_setup_compression.htm)
