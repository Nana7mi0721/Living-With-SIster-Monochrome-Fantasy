# Godot 重制版 Windows x64 发布说明

> 状态：**现行（2026-11-27 正式 Windows 安装包与 Android Debug 包已完成构建、审计和烟测）**
>
> 更新日期：2026-11-27（Asia/Shanghai）
>
> 适用：Godot 4.7.2 .NET，Windows x64
>
> 本文记录发布预设、打包脚本、包体账本、发布审计与验证记录。仓库规范见 [../../AGENTS.md](../../AGENTS.md)，定稿决策见 [DECISIONS.md](DECISIONS.md)（D-53 / D-64 / D-65 / D-92 / D-94 / D-95 / D-96 / D-97 / D-100 / D-101）。

## 1. 发布策略概览

- 自 D-53 起发布包自带 `game_data/`（紧凑 JSON）与 `work/all/media.pak`（加密归档）及 `BasicData/*.project`，玩家解压即可直接运行 `game\Lwsmf.exe`，无需安装 Python，也无需预先手动转换。
- 打包脚本：`tools/package_release.sh [--demo] [--slim|--lossless] [--media-pak FILE] [--hd] [--brotli] [--installer] [--template FILE|--official-template] [--allow-placeholder]`，自动从环境变量、`tools/local.env.sh` 或 `PATH` 探测 `godot`、`dotnet`、Inno Setup 与本地定制 2D Mono 模板（`work/out/templates/windows/windows_release_x86_64.exe`，零本机硬编码路径；传 `--official-template` 可强制回退官方模板），先对 `$STAGE` 执行 `release_audit.py`，通过后再交换进入 `dist/LWSMF-<版本>-windows-x64[-slim][-hd]/` 并生成同名 `.zip` 与独立的 `.zip.sha256`；`--installer` 另行产出同名 `-setup.exe` 与独立的 `-setup.exe.sha256`。若 `work/all/_publish/` 缺省（如 `--demo` 或纯离线构建），可传 `--allow-placeholder` 或 `LWSMF_ALLOW_PLACEHOLDER_ICONS=1` 生成占位图标。
- 交互式发包界面：`python tools/release_tui.py`（或 Windows 直接双击 `tools/release_packager.bat`），与底层打包脚本共用同一套工具链与模板解析规则，默认以**真无损压缩（`--lossless` + `--brotli`）**作为三端标准版本（不包含有损 slim 档），支持交互切换版本号、导出平台、定制/官方模板、Inno Setup、iOS 数据注入与统一产物验真（`--verify` / `--smoke-setup`）。
- 发布审计：`tools/release_audit.py --dist <dir>` 与 `tools/release_audit.py --repo-scan`，校验程序集、紧凑 JSON、`media.pak` CRC/资产覆盖、0 `.pdb`、0 `addons/`/`tests/` 渗透以及 0 隐私/本机绝对路径泄露。

## 2. 工具链与导出模板

- **Godot 版本**：Godot 4.7.2-stable Mono（Win64），对应官方 Mono 导出模板 `4.7.2.stable.mono`（放置于系统 `%APPDATA%\Godot\export_templates\4.7.2.stable.mono\`）。
- **.NET SDK**：.NET SDK 8.0（构建 `-c Release`）。
- 未加入全局 `PATH` 时，可通过环境变量传入：

```bash
export GODOT_BIN="/path/to/Godot_v4.7.2-stable_mono_win64_console.exe"
export DOTNET_BIN="/path/to/dotnet.exe"
export DOTNET_ROOT="/path/to/dotnet8"
```

## 3. 导出预设

文件：[../../godot/export_presets.cfg](../../godot/export_presets.cfg)

```ini
name="Windows Desktop"
platform="Windows Desktop"
export_filter="all_resources"
include_filter="assets/boot/generated/*.bin,assets/boot/generated/icon_pc_taskbar.png"
exclude_filter="tests/*,tests_gd/*,game_data/*,work/*,build/*,addons/*,mobile_data/*,assets/boot/android_*.png,assets/boot/generated/android_*.png,assets/boot/generated/ios_*.png,assets/boot/generated/*.ico"
export_path="build/release/Lwsmf.exe"
debug/export_console_wrapper=0
dotnet/include_scripts_content=false
dotnet/include_debug_symbols=false
dotnet/embed_build_outputs=false
```

说明：
- `addons/*` 排除开发期插件（如 `addons/godot_ai/`），防止打包进 `Lwsmf.pck`。
- `dotnet/embed_build_outputs=false` 时需随包携带 `game/data_Lwsmf_windows_x86_64/` 运行时目录。
- Windows 可执行文件使用 `res://assets/boot/generated/icon_pc.ico`，该文件由 `tools/sync_release_icons.py` 在导出前从本地 `_publish/icon_pc.png` 派生；运行时任务栏/角标读取同目录的 `.bin` 原始 ICO 副本。

### 3.1 发行元数据与视觉资源

发布源图位于 ignored 的 `work/all/_publish/`；打包期派生到 `work/out/release_icons/`，并把 Godot 所需文件临时复制到 `godot/assets/boot/generated/`，导出结束后自动清理：

| 素材 | 用途 |
| :--- | :--- |
| `icon_pc.png` | Windows 主程序、快捷方式与安装程序 `.ico` |
| `icon_pc_dock.png` | Windows 任务栏大图标 |
| `icon_pc_corner.png` | Windows 窗口左上角小图标 |
| `icon_mobile.jpg` | Android 192/432 图标与 iOS 1024 应用图标 |
| `setup_welcome.png` | 完整 `1536×864` 横版海报；`copy2` 原字节直通，用于欢迎页顶部横幅 |
| `setup_finish.png` | 完整 `1807×2348` 竖版海报；`copy2` 原字节直通，用于完成页并按源图比例布局 |
| `setup_small.png` | 内页右上角 `55×55` 白底应用图标；构建期由 `icon_pc.png` 派生 |

Windows 运行时通过 `WindowIcons.cs` 同时调用 `WM_SETICON` 与窗口类图标 API：任务栏使用 `icon_pc_dockbar.bin`，窗口左上角使用 `icon_pc_corner.bin`；窗口类大/小图标也指向任务栏图，覆盖 Explorer 的类图标回退路径。`.bin` 仅是 `.ico` 的逐字节副本，用于绕过 Godot 图片导入管线；两者由打包测试断言完全一致。

Windows 版本资源固定为简体中文语言 `0x0804`，字段如下：

| 字段 | 值 |
| :--- | :--- |
| 文件说明 / 产品名称 | `妹相随～黑白世界的缤纷冒险～` |
| 公司 / 发布者 | `Inusuku` |
| 版权 | `Copyright (C) 2026 Inusuku. All rights reserved.` |
| 文件版本 / 产品版本 | 打包脚本的 `--version`，规范化为四段数字 |

移动端应用显示名统一为 `妹相随`。安装器仅声明简体中文语言并设置 `ShowLanguageDialog=no`，不再显示语言选择框。现行紧凑向导使用 `WizardStyle=modern`、`DisableWelcomePage=no`、`DisableReadyPage=yes`，流程为欢迎页→目标位置→进度→完成；欢迎页由独立 `TBitmapImage` 在顶部按 `16:9` 完整显示 `setup_welcome.png`，完成页由原生 `WizardBitmapImage2` 显示 `setup_finish.png`，构建期把源 PNG 的真实宽高注入 Inno 脚本并等比适配，不裁切、不拉伸，内页右上角使用 `setup_small.png`，不启用 `WizardBackImageFile` 整窗背景效果。`DisableDirPage=no` 始终显示自定义安装路径页。默认安装目录使用完整中文产品名，固定 `AppId` 保持升级/卸载识别稳定。

## 4. 打包命令与流程

在仓库根目录运行：

```bash
# 标准发布包（保留原始媒体字节 + 紧凑 game_data + 最小 BasicData/*.project）
bash tools/package_release.sh

# Slim 瘦身包（media.pak 内 PNG/JPG 按 >=5% 收益转 WebP、剔除未使用的 MapChip/）
bash tools/package_release.sh --slim

# 真无损瘦身安装包（原版 1× 媒体；PNG/BMP 仅做像素级无损 WebP，剔除未使用的 MapChip/；输出名不带 -slim）
bash tools/package_release.sh --lossless --brotli --installer
```

脚本流程：
1. 若 `godot/Lwsmf.sln` 不存在，临时创建 solution 并在退出时自动清理。
2. 执行 `dotnet build godot/Lwsmf.csproj -c Release -v:minimal`。
3. 由 `tools/sync_release_icons.py` 派生窗口/任务栏/安装器图标与海报，并写入 Windows 版本元数据。
4. 执行 Godot `--export-release "Windows Desktop"` 至 `godot/build/release/` 并剔除任何 `.pdb`。
5. 将 `game_data/{programs,database,map_tiles}.json` 压缩为单行紧凑 JSON 放入 `$STAGE/game_data/`。
6. 仅复制 `work/all/BasicData/*.project` 与 `work/all/media.pak`；`--lossless` 调用 `tools/pack_media.py --from-pak ... --lossless --extra-root ...`，`--slim` 调用 `--slim --extra-root ...`，并用 `work/all/SystemFile/` 补齐清单缺失项（只补缺失、不覆盖基包条目），跳过 `work/all/save/` 与冗余 `BasicData/*.dat`/`MapChip/`/`MapData/`/`SystemFile/`。
7. Staging `converter/`（剥离 `__pycache__`、`.pyc` 与 `tests/`）、最新 `README.txt` 与 `按键说明.txt`。
8. 在替换 `dist/` 之前先对 `$STAGE` 执行 `python tools/release_audit.py --dist "$STAGE"`。
9. 通过后替换进 `dist/LWSMF-<版本>-windows-x64[-slim][-hd]`，旧目录备份到 `work/out/dist-backup-*`，随后生成同名 `.zip` 与独立的 `<包名>.zip.sha256` 并打印体积账本。
10. `--installer` 时以 `tools/installer/lwsmf.iss.tmpl` 生成仅简体中文、欢迎页完整横版海报、目录与快捷方式合并的 Inno Setup LZMA2 安装包；自动探测 `ChineseSimplified.isl`（缺失即失败）并输出 `LWSMF-<版本>-windows-x64[-slim][-hd]-setup.exe`，随后生成独立的 `<包名>-setup.exe.sha256`。
11. 最后执行 `normalize_integrity`：若工作区被外部代理设为 `Low Mandatory Level`，对 `dist/` 递归恢复为 `Medium`；普通令牌无 `WRITE_OWNER` 时先补当前用户 FullControl。可单独运行 `tools/fix_integrity_level.ps1` 修复工作区。

## 5. 实测包体账本与校验指纹（2026-11-21）

> 表内 `Lwsmf-win64*` 为历史包名；2026-11-25 起统一使用 `LWSMF-<版本>-windows-x64[-slim]`。

| 组成部分 | 基线估算（未优化） | 标准包 `Lwsmf-win64` | Slim 包 `Lwsmf-win64-slim` | 说明 |
| :--- | ---: | ---: | ---: | :--- |
| `game/Lwsmf.exe` | 104.31 MB | 104.31 MB | 104.31 MB | Godot 4.7.2 Mono Win64 |
| `game/Lwsmf.pck` | 0.08 MB | 0.17 MB | 0.17 MB | 不含 `tests/`、`addons/` |
| `game/data_Lwsmf_windows_x86_64/` | 76.42 MB | 76.46 MB | 76.46 MB | .NET 8 运行时与程序集（0 PDB） |
| `game_data/*.json` | 52.06 MB | **20.55 MB** | **20.55 MB** | 紧凑 JSON（−31.51 MB） |
| `work/all/media.pak` | 728.78 MB | 728.78 MB | **423.78 MB** | 3,904 条 → 3,802 条（3,436 图转 WebP，剔除 `MapChip/`） |
| `work/all/other` | 8.67 MB | **0.37 MB** | **0.37 MB** | 仅保留 `BasicData/*.project` |
| `converter + README` | 0.07 MB | 0.07 MB | 0.07 MB | — |
| **非压缩总计** | **~970.4 MiB** | **930.71 MB** | **625.70 MB** | Slim 较标准包净减 **305.01 MB（−32.8%）** |
| **ZIP 归档** | — | **800.75 MB** | **495.65 MB** | Slim Zip 控制在 500 MB 以内 |

- **标准包审计**：`RELEASE_AUDIT_OK files=215 bytes=975917746 pak_entries=3904 assets=172`
  - `Lwsmf-win64.zip` SHA-256：`275109c67b2ee5e8e9b26c5f11964418b2c4dbf75f749ecf371c6e7f7ecaf05c`
  - `Lwsmf-win64/game/Lwsmf.exe` SHA-256：`a24e7c676a0e057a67cdb4a5b19fad5298ee25f36796cf6daf91597adb398099`
  - `Lwsmf-win64/work/all/media.pak` SHA-256：`d923b3acaee9f630d6b5f56f8ea380727097d0a44da0548712bc49f484ec063d`
- **Slim 包审计**：`RELEASE_AUDIT_OK files=215 bytes=656098759 pak_entries=3802 assets=172`（`PACK_MEDIA_OK entries=3802 slim=True orig_bytes=763198055 packed_bytes=444164477 pak_bytes=444365978 webp=3436 ogg=0 kept=366`）
  - `Lwsmf-win64-slim.zip` SHA-256：`39a090bcd24a49d172cfa9bbef92ac98121643374557b0fc43e7a4bc8d712f24`
  - `Lwsmf-win64-slim/game/Lwsmf.exe` SHA-256：`a24e7c676a0e057a67cdb4a5b19fad5298ee25f36796cf6daf91597adb398099`
  - `Lwsmf-win64-slim/work/all/media.pak` SHA-256：`e3669a9e1e587e822c022c8d63345031eff539d687753f77a4ca578e31f2dbd1`
- **Slim 载荷实测**：通过宿主 Godot 读取 `dist/Lwsmf-win64-slim/work/all/media.pak`（3,802 条）与紧凑 JSON，`BattleEffect/kaenbeam.png`、`BattleEffect/pipo-btleffect162.png`、`Picture/title01.jpg`、`Picture/item_list_base.png`、`SystemGraphic/TitleGraphic.jpg`、`SystemFile/WindowBase.png`、`cg/bg_01_home_a.jpg`、`SystemFile/[SE]Cursor.ogg` 与 `programs.json`（550 公共事件）+ `database.json` 全部零错误解码（`SLIM_PAYLOAD_SMOKE_OK misses=0`）。注：在自动化沙盒终端内直接拉起新建 `.exe` 会触发 Low-IL ACL 限制（`0xC0000142`），独立桌面会话双击解压包正常运行不受此限。

### 5.1 可选 2× HD 包（`media_hd.pak`，2026-11-22）

```bash
# 1) 生成 2× 素材（Real-CUGAN CLI 出 PNG，Pillow 转无损 WebP；输出 work/hd，不入库）
python tools/upscale_assets.py --root work/retail_dat --manifest work/media_manifest.txt --out work/hd

# 2) 校验尺寸/alpha/覆盖率（缺文件、尺寸不符或未覆盖都会返回非 0）
python tools/verify_hd_manifest.py

# 3) 打成独立包（不替换 1× 的 media.pak）
python tools/pack_media.py --hd-root work/hd --out work/all/media_hd.pak

# 4) 带 HD 层打包发布（输出 dist/LWSMF-<版本>-windows-x64-hd）
bash tools/package_release.sh --hd
```

| 指标 | 实测值 | 说明 |
| :--- | ---: | :--- |
| 活体图像 | 2,506 张（实际 SR 2,147 + Lanczos 274 + 85 护栏跳过） | 白名单 `work/media_manifest.txt` |
| 产出条目 | 2,421 | `hd_manifest.json`，缺失 0 / 尺寸不符 0 / 未覆盖 0 |
| 2× 像素总量 | 约 3.93 Gpx | 源 1.11 Gpx，最大 9216×576、1536×7488 等按护栏保持 1× |
| `work/hd` 体积 | **1,273.9 MiB** | 2,421 个文件，灰度去色后无损 WebP 明显缩小 |
| `media_hd.pak` | **本轮未生成** | 按要求先交付 loose `work/hd`，确认素材后再打包 |
| 全量耗时 | 3,010 s（约 50 分钟） | RTX 5060 Laptop，含推理与 WebP 编码；灰度修复另约 8 分钟 |
| 引擎回退 | 274 张（253 CPU + 21 色噪/涂抹/不稳） | 其余 2,153 张为 Real-CUGAN 2× |
| 校验标记 | `HD_MANIFEST_VERIFY_OK entries=2421 missing=0 mismatch=0 degraded=0 uncovered=0 opaque_alpha_dropped=4` | 4 张源 alpha 恒为 255，丢通道无视觉影响 |

运行侧：`project.godot` 默认窗口 1024×576、`stretch/mode=canvas_items`；桌面自动探测 `<root>/media_hd.pak`，移动端（`mobile` feature）默认关闭并回退 1×；缓存预算桌面 512 MB / 移动 192 MB。开关写入 `user://display_settings.cfg` 的 `display/hd_textures`，也可用 `LWsmf_HD_PAK` 指定包路径。

> 上表为 2026-11-21 基线；自 2026-11-24 起，发布包根目录还会包含 `按键说明.txt`。

### 5.2 真无损媒体包与 Windows Setup（2026-11-25）

```bash
bash tools/package_release.sh --lossless --brotli --installer --version 0.1.0
```

| 指标 | 实测值 | 说明 |
| :--- | ---: | :--- |
| 媒体来源 | 原版 1× `work/retail_dat` | 未使用 2× 超分资源 |
| `media.pak` | **517,980,992 B** | 2,666 条；1,688 张无损 WebP + 978 条原字节 + 10 条打包期补齐 |
| 发布目录 | **722,181,164 B** | 218 个文件 |
| ZIP 归档 | **600,957,997 B** | `LWSMF-0.1.0-windows-x64.zip` |
| Setup | **580,595,989 B** | Inno Setup 6.7.4 / LZMA2，仅简体中文、紧凑向导、自定义路径页 |

- **无损约束**：仅 PNG/BMP 生成 `lossless=True, quality=100, method=6, exact=True` 的 WebP 候选，编码后重新解码并逐像素比对；只有体积更小且像素完全一致才替换；JPG/MP3/OGG/WAV 保持原字节。发布名不带 `-slim`，避免与有损瘦身包混淆。
- **打包与审计**：`PACK_MEDIA_OK entries=2666 mode=lossless slim=False lossless=True extra=10`、`RELEASE_AUDIT_OK files=218 bytes=722181164 pak_entries=2666 assets=172`。`program_asset_missing=139` 为零售源本身缺失的静态引用，非本轮打包新增，不阻断发布。
- **安装器交互核验**：用 UI Automation 与 Win32 控件树确认首屏为欢迎页、第二步为“选择目标位置”，无语言选择页、任务页与准备安装页；目录页勾选框、安装说明和磁盘空间提示不重叠。150% DPI 下欢迎页可用区 `828×613`，横幅 `828×466`（16:9），标题与提示分别位于 `y=479`、`y=525`；完成页图片 `306×613`（1:2）。
- **窗口图标核验**：冷启动后任务栏按钮为 `icon_pc_dockbar` 女生头像，标题栏左上角为 `icon_pc_corner` Q 版角标；窗口注册后 0.25 s / 1.0 s 的两次图标消息补发消除了 Explorer 首次缓存 Godot 默认图标的竞态。
- **校验指纹**：
  - `LWSMF-0.1.0-windows-x64.zip` SHA-256：`9cc25901d2e69789b3cdbe35676843b5458894640b5ea30718ec0e2fe5bd3ec5`
  - `LWSMF-0.1.0-windows-x64-setup.exe` SHA-256：`363e4ee6f373829d46141289091b67dacf9361651584bffdbc17a590e50e19fd`
  - `game/Lwsmf.exe` SHA-256：`fe0b8ac6ba78334bb78c31d4f347caf7fcce3905e4232c25710a2b45e04661ee`
  - `work/all/media.pak` SHA-256：`58400bdb22143dddff42f7b1586cfedf54c777555c556d49d21d63de06cedb30`

### 5.3 紧凑向导正式交付（2026-11-25）

| 指标 | 实测值 |
| :--- | ---: |
| 正式 Setup | **580,595,989 B** |
| SHA-256 | `363e4ee6f373829d46141289091b67dacf9361651584bffdbc17a590e50e19fd` |
| 欢迎页海报 | `sisfan_promo.png`（`1536×864`）原字节直通，顶部 16:9 完整展示，不裁切 |
| 完成页海报 | `TitleGraphic6.jpg`（`520×1041`）同尺寸转存为 PNG，页面按 1:2 展示，不裁切 |
| 页面顺序 | 欢迎页 → 选择目标位置 → 安装进度 → 完成页 |

- **JPEG 约束**：Inno Setup 6.7.4 的 `WizardImageFile` 不接受该 JPEG，直接引用会在启动时弹出“Bitmap image is not valid.”；构建期从源 JPG 解码并同尺寸转存 PNG，源文件字节保持不变。
- **烟测**：正式 Setup 静默安装至工作区临时目录，`game/Lwsmf.exe`、`work/all/media.pak` 与 `按键说明.txt` 均落盘，媒体包 SHA-256 与发布目录一致；开始菜单、桌面快捷方式、HKCU 卸载项均创建成功，静默卸载后全部清理。冷启动任务栏与标题栏图标已截图核验，测试包与临时探针已删除。

### 5.4 新发行素材正式重建（2026-11-27）

| 指标 | 实测值 |
| :--- | ---: |
| Windows 发布目录 | **722,231,504 B**（218 个文件） |
| Windows ZIP | **600,993,724 B** |
| Windows Setup | **583,384,556 B** |
| Android Debug APK | **573,075,046 B** |
| 完成页海报 | `setup_finish.png`（`1807×2348`）原字节直通，按真实宽高比适配 |
| 欢迎页海报 | `setup_welcome.png`（`1536×864`）原字节直通，顶部 16:9 展示 |

- **发行指纹**：
  - `LWSMF-0.1.0-windows-x64.zip` SHA-256：`74d00a7fe7e0ef1772a76c1a12f0826d870ad1bfa6c819317fe9f74a8f8d8d97`
  - `LWSMF-0.1.0-windows-x64-setup.exe` SHA-256：`4c0b3c6bdd86a5cf79d5fd668bf8e72bd75133ce3e58169c8cd72aca9e97ae25`
  - `game/Lwsmf.exe` SHA-256：`b63afe67cba00c0ecbfa8b6ad4d3ffa1bc5d4bce65a38919eb7f5578e419455b`
  - `work/all/media.pak` SHA-256：`58400bdb22143dddff42f7b1586cfedf54c777555c556d49d21d63de06cedb30`
  - `LWSMF-0.1.0-android-arm64.apk` SHA-256：`36c917ec102c37213efd7df3f595e0267ee61731d81e816d6cfc545cb42fd15c`
- **安装器布局**：构建期从 `setup_finish.png` 读取 `1807×2348` 并注入 `FinishPosterWidth/FinishPosterHeight`；欢迎页和完成页文件均以 `copy2` 原字节直通。本轮完成页实际仍由未布局的 `WizardBitmapImage2` 显示，比例问题已由 5.5 修正。
- **Windows 烟测**：Setup 退出码 `0`，静默安装后 `game/Lwsmf.exe`、`按键说明.txt` 与 `work/all/media.pak` 均存在，媒体包 SHA-256 与清单一致；静默卸载退出码 `0`，安装目录、桌面快捷方式和开始菜单项均已清理。
- **Android 图标**：`mipmap-xxxhdpi-v4/icon_foreground.webp` 的不透明区为 `60..371`（311/432，72%），背景为 `#E8E8E6`；中央 `288×288` 可见区被完全覆盖，无启动器黑边。APK 校验为 `ANDROID_APK_VERIFY_OK entries=384`。
- **回归边界**：29 项移动/发布工具单测、决策表 lint、隐私审计和 C# Release 编译通过。完整 38 步 Godot 门禁受本机 `work/retail_backup/Save/SaveData01.sav` 被真实游戏进度覆盖影响（`387,265 B`，测试基线为 `366,146 B`），`Smoke` 战斗夹具与 M13 两项无法作为有效回归；这与本轮发行素材和安装器改动无关。
- **资源恢复**：`work/all/SystemGraphic/TitleGraphic.jpg` 已从 `work/all/media.pak` 原字节恢复，`358,497 B`、`1024×576`、SHA-256 `33d1a4299e0fb2544967b34769f8e71871333110918084564eb1dd6c257dc6ba`。

### 5.5 完成页比例与任务栏类图标修复重建（2026-11-27）

| 指标 | 实测值 |
| :--- | ---: |
| Windows 发布目录 | **722,232,528 B**（218 个文件） |
| Windows ZIP | **600,994,281 B** |
| Windows Setup | **583,393,280 B** |
| 完成页槽位 / 海报 | `320×613` / `320×415`（150% DPI） |
| 媒体包 | `517,980,992 B`，SHA-256 未变 |

- **完成页根因**：模板此前误改欢迎页的 `WizardBitmapImage`，完成页实际使用的 `WizardBitmapImage2` 仍保留 `164:314` 默认锚点，被 Inno 的 `AnchorOuterPage` 强制拉伸。现改为在 `InitializeWizard()` 清空两张大图的 `Anchors`，在 `CurPageChanged(wpFinished)` 对 `WizardBitmapImage2` 按 `1807×2348` 源图比例居中布局后再显示。实测槽位 `320×613`、海报 `320×415`，无裁切、无拉伸。
- **任务栏根因**：用户截图中的通用图标与 Godot 窗口类默认图标一致，说明 Explorer 走了 `GCLP_HICON/GCLP_HICONSM` 回退，而旧实现只写 `WM_SETICON`。现同时设置根窗口类图标并刷新，实测类大图标 `256×256`、类小图标 `24×24` 均为 `icon_pc_dockbar`。
- **正式指纹**：
  - `LWSMF-0.1.0-windows-x64.zip` SHA-256：`0ccbcc020b748583f25496eb6462e848f90c39b3ee5d746f4e76d358b8ea59da`
  - `LWSMF-0.1.0-windows-x64-setup.exe` SHA-256：`44b7d76a7bfeaf30cea010bb0966431395795507207ae3bfbd537230acfcd301`
  - `game/Lwsmf.exe` SHA-256：`b63afe67cba00c0ecbfa8b6ad4d3ffa1bc5d4bce65a38919eb7f5578e419455b`
  - `work/all/media.pak` SHA-256：`58400bdb22143dddff42f7b1586cfedf54c777555c556d49d21d63de06cedb30`
- **烟测**：正式 Setup 静默安装退出码 `0`，`game/Lwsmf.exe`、`按键说明.txt` 与 `work/all/media.pak` 均落盘，媒体包哈希一致；静默卸载退出码 `0`，安装目录、开始菜单与桌面快捷方式均清理。29 项发布工具单测、Inno 6.7.4 实际编译与 C# Release 0 警告/0 错误通过。完整 Godot 门禁仍有 2 项与本轮无关的既有失败：`Smoke` 战斗夹具仅跑 3 回合、M13 slot 1 超时。

## 6. 发布审计与移动端配套

```bash
python tools/release_audit.py --dist "dist/LWSMF-<版本>-windows-x64"
python tools/release_audit.py --dist "dist/LWSMF-<版本>-windows-x64-slim"
python tools/release_audit.py --hd-root work/hd
python tools/release_audit.py --repo-scan
bash tools/check_mobile_env.sh --target all
```

- 预期标记：`RELEASE_AUDIT_OK ...`、`REPO_PRIVACY_AUDIT_OK leaks=0` 与 `MOBILE_ENV_CHECK`。
- Android / iOS 发包预设、环境体检、`package_android.sh`、`package_ios.sh`、`inject_ios_data.sh` 与无签名 iOS CI 详情见 [../release/MOBILE_PACKAGING.md](../release/MOBILE_PACKAGING.md)。
