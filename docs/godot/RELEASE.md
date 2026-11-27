# Godot 重制版 Windows x64 发布说明

> 状态：**现行（标准版、真无损安装包与 `--slim` 版均已完成打包与发布审计）**
>
> 更新日期：2026-11-25（Asia/Shanghai）
>
> 适用：Godot 4.7.2 .NET，Windows x64
>
> 本文记录发布预设、打包脚本、包体账本、发布审计与验证记录。仓库规范见 [../../AGENTS.md](../../AGENTS.md)，定稿决策见 [DECISIONS.md](DECISIONS.md)（D-53 / D-64 / D-65 / D-92 / D-94 / D-95 / D-96 / D-97）。

## 1. 发布策略概览

- 自 D-53 起发布包自带 `game_data/`（紧凑 JSON）与 `work/all/media.pak`（加密归档）及 `BasicData/*.project`，玩家解压即可直接运行 `game\Lwsmf.exe`，无需安装 Python，也无需预先手动转换。
- 打包脚本：`tools/package_release.sh [--demo] [--slim|--lossless] [--media-pak FILE] [--hd] [--brotli] [--installer] [--allow-placeholder]`，自动通过环境变量或 `PATH` 探测 `godot`、`dotnet` 与 Inno Setup（零本机硬编码路径），先对 `$STAGE` 执行 `release_audit.py`，通过后再交换进入 `dist/LWSMF-<版本>-windows-x64[-slim][-hd]/` 并生成同名 `.zip` 与 `dist/SHA256SUMS.txt`；`--installer` 另行产出同名 `-setup.exe` 并把 SHA-256 追加进清单。若 `work/retail_dat/_publish/` 缺省（如 `--demo` 或纯离线构建），可传 `--allow-placeholder` 或 `LWSMF_ALLOW_PLACEHOLDER_ICONS=1` 生成占位图标。
- 交互式发包界面：`python tools/release_tui.py`（或 Windows 直接双击 `tools/release_packager.bat`），默认以**真无损压缩（`--lossless` + `--brotli`）**作为三端标准版本（不包含有损 slim 档），支持交互切换版本号、导出平台、定制模板、Inno Setup 与 iOS 数据注入。
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
exclude_filter="tests/*,tests_gd/*,game_data/*,work/*,build/*,addons/*,mobile_data/*,assets/boot/android_*.png,assets/boot/generated/*.png,assets/boot/generated/*.ico"
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

发布源图位于 ignored 的 `work/retail_dat/_publish/`；打包期派生到 `work/out/release_icons/`，并把 Godot 所需文件临时复制到 `godot/assets/boot/generated/`，导出结束后自动清理：

| 素材 | 用途 |
| :--- | :--- |
| `icon_pc.png` | Windows 主程序、快捷方式与安装程序 `.ico` |
| `icon_pc_dockbar.png` | Windows 任务栏大图标 |
| `icon_pc_corner.png` | Windows 窗口左上角小图标 |
| `icon_mobile.jpg` | Android 192/432 图标与 iOS 1024 应用图标 |
| `setup_poster.png` | 海报母版原字节副本；`copy2` 直通，不裁切、不覆盖 |
| `setup_promo.png` | 完整 `1536×864` 横版海报；由 `sisfan_promo.png` 原字节直通，用于欢迎页顶部横幅 |
| `setup_small.png` | 内页右上角 `55×55` 白底应用图标 |

Windows 运行时通过 `WindowIcons.cs` 调用 `WM_SETICON`：任务栏使用 `icon_pc_dockbar.bin`，窗口左上角使用 `icon_pc_corner.bin`。`.bin` 仅是 `.ico` 的逐字节副本，用于绕过 Godot 图片导入管线；两者由打包测试断言完全一致。

Windows 版本资源固定为简体中文语言 `0x0804`，字段如下：

| 字段 | 值 |
| :--- | :--- |
| 文件说明 / 产品名称 | `妹相随～黑白世界的缤纷冒险～` |
| 公司 / 发布者 | `Inusuku` |
| 版权 | `Copyright (C) 2026 Inusuku. All rights reserved.` |
| 文件版本 / 产品版本 | 打包脚本的 `--version`，规范化为四段数字 |

移动端应用显示名统一为 `妹相随`。安装器仅声明简体中文语言并设置 `ShowLanguageDialog=no`，不再显示语言选择框。现行紧凑向导使用 `WizardStyle=modern`、`DisableWelcomePage=no`、`DisableReadyPage=yes`，流程为欢迎页→目标位置→进度→完成；欢迎页由独立 `TBitmapImage` 在顶部按 `16:9` 完整显示 `setup_promo.png`（`sisfan_promo.png` 原字节），完成页由原生 `WizardBitmapImage` 显示 `TitleGraphic6.jpg` 同尺寸转存的 PNG 并保持 `1:2`，内页右上角使用 `setup_small.png`，不启用 `WizardBackImageFile` 整窗背景效果。`DisableDirPage=no` 始终显示自定义安装路径页。默认安装目录使用完整中文产品名，固定 `AppId` 保持升级/卸载识别稳定。

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
9. 通过后替换进 `dist/LWSMF-<版本>-windows-x64[-slim][-hd]`，旧目录备份到 `work/out/dist-backup-*`，随后生成同名 `.zip` 与 `dist/SHA256SUMS.txt` 并打印体积账本。
10. `--installer` 时以 `tools/installer/lwsmf.iss.tmpl` 生成仅简体中文、欢迎页完整横版海报、目录与快捷方式合并的 Inno Setup LZMA2 安装包；自动探测 `ChineseSimplified.isl`（缺失即失败）并输出 `LWSMF-<版本>-windows-x64[-slim][-hd]-setup.exe`，随后把 Setup SHA-256 追加进 `dist/SHA256SUMS.txt`。
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
