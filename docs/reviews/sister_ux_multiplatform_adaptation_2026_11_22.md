# 姊妹作交互优化与多平台适配落地报告（2026-11-22）

> **依据决策**：[`DECISIONS.md` D-83](../godot/DECISIONS.md)  
> **参考调研**：姊妹作《妹！生活》UX 与多平台适配研究笔记（`docs/调研/`，本地只读未入库）  
> **核心原则**：在不改动 C# 确定性核心调度/Oracle 契约与不引入 GDExtension（D-02）的前提下，吸收姊妹作已验证的三端运行、纯触控交互、存读档防误触与桌面本地化封装设计。

---

## 1. 落地清单与实现映射

### 一、移动端运行时关键路径与内存收紧（P0）

1. **iOS Bundle 数据根路径修复**
   - [AppPaths.cs](../../godot/presentation/AppPaths.cs) / [AppPaths.gd](../../godot/presentation_gd/AppPaths.gd)：新增 `IsIos` / `is_ios()` 判定；在导出模板包下（`OS.HasFeature("template")`），`PackageRoot()` 返回 `.app` 内部目录 `ResourceRoot()`，与 `ios_inject_data.py` 的注入目录结构一致，修复原路径上跳一层导致的启动缺数据问题。
2. **APK `.blob.cfb` 零拷贝直读与后台明文分块释放**
   - [MediaPakReader.cs](../../godot/presentation/MediaPakReader.cs)：抽取 `IPakReaderStream` 适配层，对 `res://` / `user://` 虚拟路径启用 `Godot.FileAccess` 随机寻址，OS 文件系统路径继续走 `FileStream`，保持线程锁与 CRC-32/XOR 校验语义。
   - [android_data.py](../../tools/mobile/android_data.py)：为 `work/all/media.pak` 清单项写入 `"direct_res": true`。
   - [MobileDataBootstrap.gd](../../godot/presentation_gd/MobileDataBootstrap.gd)：`CHUNK_BYTES` 降至 `1 MiB` 并迁至后台 `Thread` 释放明文小文件；对 `direct_res: true` 项跳过整包复制；升级时执行 `_prune_stale_files` 清理废弃旧文件且严格保留 `Save/` 目录。
3. **Android / iOS 分级纹理缓存上限与字体回退栈**
   - [AssetDB.gd](../../godot/presentation_gd/AssetDB.gd)：定义 `ANDROID_BUDGET_BYTES = 128 MiB` 与 `IOS_BUDGET_BYTES = 64 MiB`（桌面保持 `512 MiB`），移动端关闭 HD；Android 自动探测 `res://mobile_data/work/all/media.pak.blob.cfb`。
   - [UiSkin.gd](../../godot/presentation_gd/UiSkin.gd)：Android 下采用中文字体优先栈（`Noto Sans CJK SC` / `Noto Sans SC` / `Source Han Sans SC`），桌面端末尾补齐 `Segoe UI Symbol` 以改善符号字形回退，并支持直接从 `res://mobile_data/.../media.pak.blob.cfb` 解出 `Lolita.ttf`。

### 二、纯触控手势状态机、系统返回路由与 iOS 合规（P0/P1）

1. **纯触控手势与滚轮累加**
   - [InputRouter.gd](../../godot/presentation_gd/InputRouter.gd)：实现基于 `1024×576` 逻辑坐标的 `InputEventScreenTouch` / `InputEventScreenDrag` 状态机：单指轻点 `< 24px` 映射左键确定、双指轻点 `< 36px` 映射右键取消、左缘 `x <= 60px` 右滑 `>= 56px` 拉出快捷侧栏（向左滑收起）、上下滑动每 `48px` 触发滚轮事件；同帧桌面滚轮事件改为累加（`[-12, 12]`）。
2. **Android 返回键 7 级路由、后台暂停与聚焦防穿透**
   - [GameRoot.gd](../../godot/presentation_gd/GameRoot.gd)：处理 `NOTIFICATION_WM_GO_BACK_REQUEST`（`handle_back_request()` 依次处理故障退出、启动保护期忽略、关闭顶层弹窗/侧栏、子菜单注入取消、标题页退出确认、局内返回标题确认）、`NOTIFICATION_APPLICATION_PAUSED`（落盘显示设置并清空按住与触控态）与 `FOCUS_IN`（重对齐按键并置位 `_modal_release_guard`）；接入 `DisplayServer.get_display_safe_area()` 边距适配与 `can_open_quick_ui()` 过场防误触。
3. **iOS 标题页退出项移除**
   - [GameRoot.gd](../../godot/presentation_gd/GameRoot.gd)：iOS 平台隐藏 `CE43` 槽位 `5` 图片、标题选项框裁为前两项（开始/读取）且屏蔽 `_on_end_game()` 主动退出进程。

### 三、交互打磨、存单覆盖确认与跨端存档互导（P1）

1. **选项加宽、箭头扩热区与 `CE131` 空白点击退出**
   - [ChoiceWindow.gd](../../godot/presentation_gd/ChoiceWindow.gd)：`MIN_WIDTH` 由 `120.0` 提升至 `240.0`（改善「是/否」等短选项点击面积）。
   - [PictureStage.gd](../../godot/presentation_gd/PictureStage.gd)：存读档翻页箭头 `12004/12005` 命中矩形以中心对称扩展至不小于 `44×64`。
   - [GameRoot.gd](../../godot/presentation_gd/GameRoot.gd) + [PlayOverlays.gd](../../godot/presentation_gd/PlayOverlays.gd)：`CE131` 存读档界面点击空白区域自动注入取消退出；保存模式命中已有 `save_NN.json` 槽位时弹出 `SaveOverwriteConfirm` 二级确认（取消/关闭均触发 `_modal_release_guard` 防穿透）；界面右下角提供快捷条（局内「回到标题」/ 标题「导入/导出存档」）；触摸模式下支持左缘「快捷菜单」侧拉抽屉。
2. **跨端 `.zip` / `.sav` 存档导入导出**
   - [GameSession.cs](../../godot/core_cs/GameSession.cs) / [GameSessionController.cs](../../godot/presentation/GameSessionController.cs)：新增 `ValidateSaveJson` 与 `ImportRetailSaveToSlot`（不改动活跃会话，写入 `save_NN.json`）。
   - [PlayOverlays.gd](../../godot/presentation_gd/PlayOverlays.gd)：新增 `SaveTransferModal` 与静态 ZIP/URI 工具方法，强校验 ZIP 签名、`256 MiB` 包体积 / `256` 文件项 / `32 MiB` 单文件上限、路径穿越、`SaveCodec` 结构校验及导入前自动备份，兼容 Android `content://` `1 MiB` 分块暂存。

### 四、性能档位、外部热键、移动端锁横屏与 Windows 本地化交付（P2）

1. **性能档位与热键扩展**
   - [GameRoot.gd](../../godot/presentation_gd/GameRoot.gd)：新增 `F10`（或调试态按 `W` / 触控侧栏）切换 `60 / 30 / 15 fps` 性能档位（持久化至 `user://display_settings.cfg`）、`Shift+F8` 开关窗口标题栏实时帧率信息、`F7` 打开存档管理及 `--hotkeys` / `user://hotkeys.cfg` 热键覆盖解析（坏行仅回退该行默认值）。
2. **构建预设与 Windows PE 中文本地化**
   - [project.godot](../../godot/project.godot)：设置 `window/handheld/orientation=4`（`sensor_landscape` 横屏锁定）。
   - [export_presets.cfg](../../godot/export_presets.cfg)：补齐 Windows 导出预设版本号与产品描述元数据。
   - [windows_pe.py](../../tools/mobile/windows_pe.py) / [package_release.sh](../../tools/package_release.sh) / [ci.yml](../../.github/workflows/ci.yml)：新增导出后 Windows PE `RT_VERSION` 语言修订（`0x0409` → `0x0804` 简体中文并重算 PE checksum，不损坏尾部 PCK）、配套单测与 CI 语法检查。

---

## 2. 本作差异化边界与有意未迁项

- **相册空白退出 / `CE303` 与 `hand3.png` 双指针**：经扫描 `game_data/programs.json`，本作回忆相册为 `CE510..527`（含左侧多分类页签），媒体包仅含 `charachip/hand2.png`（非姊妹作 `CE303` 与 `hand3.png` 双系统指针）；为避免在分类页签误关相册或引入与原版事件冲突的系统箭头，本轮未强行注入相册空白点击或替换系统指针。
- **独立 `F4` 窗口倍增热键与桌面 NSIS 安装器**：本作已由 `D-72..D-78` 固化 `F4` 为消息回顾、`F8` 为 `Integer / Fit-Nearest / Fit-Linear` 缩放模式切换、`F5/F9` 为快存快读，为保持既有快捷键与门禁一致，保留现有热键分工并通过 `F10`、`Shift+F8` 与 `--hotkeys` 扩展；Windows 交付继续保持免安装便携 ZIP 路线，暂未增加 NSIS 安装脚本。
- **iOS 原生 UIKit `UIDocumentPicker` 扩展桥接**：受 `D-02` 不引入原生 C++/Rust GDExtension 约束，iOS 暂通过 Godot 原生 `FileDialog`（`use_native_dialog = true`）及 `export_presets.cfg` 已开启的「文件 App / iTunes 文件共享」承担互导入口，未另编静态原生 `.a` 插件。

---

## 3. 回归测试与门禁结果

- **C# 编译**：`dotnet build godot/Lwsmf.csproj -c Release` 与 `-c ExportRelease` 均通过（0 警告、0 错误）。
- **新增与扩展自动化用例**：
  - [SaveImportTest.cs](../../godot/tests/SaveImportTest.cs)：覆盖 `ValidateSaveJson` 与 `ImportRetailSaveToSlot` 写槽测试。
  - [AssetDBTest.gd](../../godot/tests_gd/AssetDBTest.gd)、[SetupWizardTest.gd](../../godot/tests_gd/SetupWizardTest.gd)：覆盖 `AppPaths.resolve_package_root`（含 iOS `.app/`）、`MediaPakReader` 读取 `res://` 虚拟路径、`128 MiB / 64 MiB` 平台上限、`direct_res: true` 跳过 `media.pak` 复制及升级保留 `Save/` 清理。
  - [InputRouterTest.gd](../../godot/tests_gd/InputRouterTest.gd)、[DialogDrawTest.gd](../../godot/tests_gd/DialogDrawTest.gd)、[GameRootTest.gd](../../godot/tests_gd/GameRootTest.gd)：覆盖触控手势（单指、双指、左缘侧滑、垂直翻页）、滚轮累加、`ChoiceWindow` 240px 下限、`12004/12005` `44×64` 热区、iOS 隐藏退出项、7 级返回路由、过场保护、覆盖确认弹窗及 ZIP 安全导入导出校验。
  - [test_mobile_tools.py](../../tools/mobile/test_mobile_tools.py)：9 项单测通过（含合成 PE32+ 的 `0x0409 -> 0x0804` 本地化与 checksum 校验）。
- **全量门禁**：`tools/godot_check.sh` 38 项无头门禁与 `LWsmf_RENDER=1` OpenGL3 渲染门禁、`tools/lint_decisions.py`、`tools/release_audit.py --repo-scan` 均在最终修改后通过（实机 Android/iOS 真机手势及跨端导出打包待对应目标设备实测）。
