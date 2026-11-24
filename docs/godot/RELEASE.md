# Godot 重制版 Windows x64 发布说明

> 状态：**现行（标准版与 `--slim` 版均已完成打包与发布审计）**
>
> 更新日期：2026-11-24（Asia/Shanghai）
>
> 适用：Godot 4.7.2 .NET，Windows x64
>
> 本文记录发布预设、打包脚本、包体账本、发布审计与验证记录。仓库规范见 [../../AGENTS.md](../../AGENTS.md)，定稿决策见 [DECISIONS.md](DECISIONS.md)（D-53 / D-64 / D-65）。

## 1. 发布策略概览

- 自 D-53 起发布包自带 `game_data/`（紧凑 JSON）与 `work/all/media.pak`（加密归档）及 `BasicData/*.project`，玩家解压即可直接运行 `game\Lwsmf.exe`，无需安装 Python，也无需预先手动转换。
- 打包脚本：`tools/package_release.sh [--demo] [--slim]`，自动通过环境变量或 `PATH` 探测 `godot` 与 `dotnet`（零本机硬编码路径），先对 `$STAGE` 执行 `release_audit.py`，通过后再交换进入 `dist/` 并生成 `.zip` 与 `dist/SHA256SUMS.txt`。
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
exclude_filter="tests/*,tests_gd/*,game_data/*,work/*,build/*,addons/*"
export_path="build/release/Lwsmf.exe"
debug/export_console_wrapper=0
dotnet/include_scripts_content=false
dotnet/include_debug_symbols=false
dotnet/embed_build_outputs=false
```

说明：
- `addons/*` 排除开发期插件（如 `addons/godot_ai/`），防止打包进 `Lwsmf.pck`。
- `dotnet/embed_build_outputs=false` 时需随包携带 `game/data_Lwsmf_windows_x86_64/` 运行时目录。

## 4. 打包命令与流程

在仓库根目录运行：

```bash
# 标准发布包（保留原始媒体字节 + 紧凑 game_data + 最小 BasicData/*.project）
bash tools/package_release.sh

# Slim 瘦身包（media.pak 内 PNG/JPG 按 >=5% 收益转 WebP、剔除未使用的 MapChip/）
bash tools/package_release.sh --slim
```

脚本流程：
1. 若 `godot/Lwsmf.sln` 不存在，临时创建 solution 并在退出时自动清理。
2. 执行 `dotnet build godot/Lwsmf.csproj -c Release -v:minimal`。
3. 执行 Godot `--export-release "Windows Desktop"` 至 `godot/build/release/` 并剔除任何 `.pdb`。
4. 将 `game_data/{programs,database,map_tiles}.json` 压缩为单行紧凑 JSON 放入 `$STAGE/game_data/`。
5. 仅复制 `work/all/BasicData/*.project` 与 `work/all/media.pak`（`--slim` 时调用 `tools/pack_media.py --from-pak ... --slim`），跳过 `work/all/save/` 与冗余 `BasicData/*.dat`/`MapChip/`/`MapData/`/`SystemFile/`。
6. Staging `converter/`（剥离 `__pycache__`、`.pyc` 与 `tests/`）、最新 `README.txt` 与 `按键说明.txt`。
7. 在替换 `dist/` 之前先对 `$STAGE` 执行 `python tools/release_audit.py --dist "$STAGE"`。
8. 通过后替换进 `dist/Lwsmf-win64`（或 `dist/Lwsmf-win64-slim`），旧目录备份到 `work/out/dist-backup-*`，随后生成 `.zip` 与 `dist/SHA256SUMS.txt` 并打印体积账本。

## 5. 实测包体账本与校验指纹（2026-11-21）

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

# 4) 带 HD 层打包发布（输出 dist/Lwsmf-win64-hd）
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

## 6. 发布审计与移动端配套

```bash
python tools/release_audit.py --dist dist/Lwsmf-win64-slim
python tools/release_audit.py --hd-root work/hd
python tools/release_audit.py --repo-scan
bash tools/check_mobile_env.sh --target all
```

- 预期标记：`RELEASE_AUDIT_OK ...`、`REPO_PRIVACY_AUDIT_OK leaks=0` 与 `MOBILE_ENV_CHECK`。
- Android / iOS 发包预设、环境体检、`package_android.sh`、`package_ios.sh`、`inject_ios_data.sh` 与无签名 iOS CI 详情见 [../release/MOBILE_PACKAGING.md](../release/MOBILE_PACKAGING.md)。
