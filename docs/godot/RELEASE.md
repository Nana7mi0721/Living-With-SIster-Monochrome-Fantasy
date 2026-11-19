# Godot 重制版 Windows x64 发布说明（M5-A）

> 状态：**发布脚本已改为自带素材；尚未重新打包**
>
> 更新日期：2026-11-19（Asia/Shanghai）
>
> 适用：Godot 4.7.2 .NET，Windows x64
>
> 本文记录发布预设、打包脚本、素材审计与验证记录。仓库规则见 `AGENT.md`，发布策略见 `docs/godot/DECISIONS.md` 的 D-53（修订 D-11）。

## 1. 结论

- 2026-11-19 起取消“发布包不带原版素材”的限制：发布包自带 `game_data/`（已转换数据）与 `work/all/`（运行所需素材），正常游玩解压后直接运行，无需安装 Python，也无需先转换。
- 打包脚本：`tools/package_release.sh`，从仓库根执行，完成构建、导出、staging、README 生成与发布审计。
- 发布审计：`tools/release_audit.py`，校验程序、运行时目录、`game_data/`、`work/all/` 与 manifest 全部 assets。
- **本次只改脚本与文档，没有运行打包；`dist/Lwsmf-win64` 仍是 2026-11-18 的旧无素材包，不得当作当前发布包。** 重新打包后以脚本输出的 `RELEASE_AUDIT_OK` 数字为准。

预期标记输出：

```text
EXPORT_PRESET_OK preset=Windows Desktop export_path=.../godot/build/release/Lwsmf.exe
RELEASE_AUDIT_OK files=<实际> bytes=<实际> assets=<实际> asset_bytes=<实际> mps=<实际>
RELEASE_PACKAGE_OK dist=.../dist/Lwsmf-win64
```

## 2. 导出模板与版本

Godot 可执行文件：

```text
E:\Study\tools\godot-4.7.2-mono\Godot_v4.7.2-stable_mono_win64\Godot_v4.7.2-stable_mono_win64_console.exe
```

`--version` 输出：

```text
4.7.2.stable.mono.official.ed1daf0bf
```

导出模板实际目录为 `%APPDATA%\Godot\export_templates\4.7.2.stable.mono`（注意：是 **4.7.2.stable.mono**，不是 `4.7.2.stable`）：

```text
C:\Users\REISEN\AppData\Roaming\Godot\export_templates\
├─ 4.7.2.stable\                 # 非 mono 模板，保留未动
└─ 4.7.2.stable.mono\
   ├─ version.txt                  # 4.7.2.stable.mono
   ├─ windows_release_x86_64.exe   # 109,513,728 B
   └─ windows_release_x86_64_console.exe  # 188,928 B
```

模板来源为 Godot 官方 `Godot_v4.7.2-stable_mono_export_templates.tpz`，解压后 `templates/` 内容展开到 `4.7.2.stable.mono`。本包只使用 release 模板。

## 3. 导出预设

文件：`godot/export_presets.cfg`

关键字段：

```ini
name="Windows Desktop"
platform="Windows Desktop"
export_filter="all_resources"
exclude_filter="tests/*,tests_gd/*,game_data/*,work/*,build/*"
export_path="build/release/Lwsmf.exe"
custom_template/release=""
binary_format/embed_pck=false
binary_format/architecture="x86_64"
application/product_name="LWSMF Remake"
dotnet/include_scripts_content=false
dotnet/include_debug_symbols=false
dotnet/embed_build_outputs=false
```

说明：

- `custom_template/release` 留空，让 Godot 自动使用上面已安装的 mono 模板；不要指向非 mono 的 `4.7.2.stable`。
- `dotnet/embed_build_outputs=false` 是最终采用值。最初使用 `true` 时，导出能完成但运行 exe 会报 `Failed to open .dotnet-publish-manifest` / `Failed to load hostfxr`，随后崩溃（exit -1073741819）。改为 `false` 后导出产物会带一个必需的 `data_Lwsmf_windows_x86_64` 运行时目录。
- `exclude_filter` 顶层排除 tests、tests_gd、game_data、work、build。`game_data/` 与 `work/all/` 作为发布包外部目录随包 staging，不进入 pck；pck 保持程序资源体积。

## 4. 打包命令与流程

在仓库根运行：

```bash
cd "E:/Study/github program/imoto life"
bash tools/package_release.sh
```

脚本默认使用：

- `GODOT_BIN=E:/Study/tools/godot-4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe`
- `DOTNET_BIN=E:/Study/tools/dotnet8/dotnet.exe`
- `DOTNET_ROOT=E:/Study/tools/dotnet8`

脚本流程：

1. 若 `godot/Lwsmf.sln` 不存在，创建临时 solution 并在退出时删除。
2. `dotnet build godot/Lwsmf.csproj -v:minimal`。
3. `godot ... --headless --path godot --export-release "Windows Desktop"`。
4. 将 `godot/build/release` 下全部产物复制到 staging 的 `game/`（包括 `data_Lwsmf_windows_x86_64/`）。
5. 复制 `game_data/` 到 staging 根目录；缺失时打包失败。
6. 复制 `work/all/` 到 staging 根目录，并删除 `work/all/save/`（玩家存档不随包）；缺失时打包失败。
7. 复制 `converter/`，删除 `__pycache__`、`.pyc` 和 `tests/`。
8. 生成 `README.txt`。
9. 若已有 `dist/Lwsmf-win64`，先移到 `work/out/dist-backup-<时间戳>-<pid>`，不覆盖用户文件。
10. 运行 `tools/release_audit.py --dist dist/Lwsmf-win64`。

## 5. 预期目录与体积

```text
dist/Lwsmf-win64/
├─ game/
│  ├─ Lwsmf.exe                         约 109 MB
│  ├─ Lwsmf.pck                         程序资源包
│  └─ data_Lwsmf_windows_x86_64/        .NET 运行时与程序集（必须随 exe 一起发布）
├─ game_data/                           已转换数据（manifest/programs/database/map_tiles，约 53 MB）
├─ work/all/                            运行素材（BasicData/MapChip/MapData/SystemFile，约 11 MB；不含 save）
├─ converter/                           转换器 Python 源码（含 convert.bat）
└─ README.txt                           发布说明
```

`data_Lwsmf_windows_x86_64` 不是可选的调试文件：`dotnet/embed_build_outputs=false` 时，exe 需要它来加载 .NET 运行时。发布或复制时不能只拿 `Lwsmf.exe` 和 `Lwsmf.pck`。

包体预计比旧无素材包增加约 64 MB（`game_data` 53 MB + `work/all` 11 MB）；最终文件数、字节数由 `release_audit.py` 实测输出。

## 6. 发布审计

命令：

```bash
python tools/release_audit.py --dist dist/Lwsmf-win64
```

规则：

- 必需程序：`game/Lwsmf.exe`、`game/Lwsmf.pck`、`game/data_Lwsmf_windows_x86_64/Lwsmf.dll`。
- 必需数据：`game_data/manifest.json`、`programs.json`、`database.json`、`map_tiles.json`，以及 manifest `artifacts` 中声明的文件与大小。
- 必需素材：`work/all/BasicData`、`MapChip`、`MapData`、`SystemFile` 非空；`MapData` 至少一个 `.mps`；manifest `assets` 中每一项的 `logical` 路径都必须存在于 `work/all/`。
- 禁止：`work/all/save` 不得随包（避免携带玩家存档）。
- `tools/copyright_audit.py` 保留为兼容包装，委托 `release_audit.py`；不再按扩展名或零售哈希拒绝素材。D-53 已取消“发布包不含原版素材”的旧限制。

成功输出：

```text
RELEASE_AUDIT_OK files=<实际> bytes=<实际> assets=<实际> asset_bytes=<实际> mps=<实际>
```

## 7. 玩家使用流程

发布包已自带 `game_data/` 与 `work/all/`，正常游玩：

```bat
game\Lwsmf.exe
```

如果 `game_data` 缺失或不完整，程序会打开首次运行向导；也可以手动重新转换：

1. 安装 Python 3.10+，确保命令行可运行 `python`。
2. 准备自己合法持有的原版 Data 目录（该目录应直接包含 `BasicData/` 与 `MapData/`）。
3. 在发布包根目录（与 `game`、`converter` 同级）运行：

```bat
converter\convert.bat --root "D:\你的原版\Data" --out game_data --reports reports
```

4. 看到 `CONVERT_ALL_OK` 后重新运行 `game\Lwsmf.exe`。

转换器只读取本机指定的原版目录，并在本地写入 `game_data\` 和 `reports\`；不会上传或分发原版文件。若提示缺少 `BasicData\Game.dat`，说明 `--root` 指向的不是解包后的 Data 目录。

## 8. 验证记录（历史旧包）

以下为 2026-11-18 旧无素材包的实际记录，保留作回归基线；**不代表 2026-11-19 策略调整后的新包**：

- 旧包：`dist/Lwsmf-win64` 共 206 files / 189,666,077 bytes。
- 旧审计：`COPYRIGHT_AUDIT_OK files=206 bytes=189666077 forbidden=0`。
- 旧 `Lwsmf.pck` 字节扫描：

```text
res://tests/ 0
res://tests_gd/ 0
res://game_data/ 0
res://work/ 0
res://build/ 0
Lwsmf.dll 0
PCK_SIZE=84588
```

- 旧 exe 启动烟测：`game/Lwsmf.exe --headless --quit` 返回 `QUIT_EXIT=0`；仅有无窗口 `user://` 日志和根证书警告。
- 旧发布程序集已剔除 C# 测试类型：`data_Lwsmf_windows_x86_64/Lwsmf.dll`=237,056 B，测试符号扫描 0。

新包的 `RELEASE_AUDIT_OK`、pck 扫描与启动烟测应在实际打包后补记。

## 9. 已知边界与后续

1. **发布程序集**：`godot/Lwsmf.csproj` 的 `ExportRelease` 条件排除 `tests/**/*.cs`；Debug 构建保留测试类型供 38 道门禁使用。
2. **运行时目录必须随包**：`data_Lwsmf_windows_x86_64/` 是 .NET 运行时与程序集，缺少时 exe 会在启动阶段失败。
3. **模板依赖**：本机必须安装 `4.7.2.stable.mono` 导出模板；仅有非 mono 的 `4.7.2.stable` 会找不到模板或报 .NET SDK 相关错误。
4. **素材权利**：发布包自带 `game_data/` 与 `work/all/` 后体积增加，分发者需确认拥有相应素材的分发权利；Git 仍不提交这些素材。
5. **存档不随包**：`work/all/save` 在 staging 时删除，发布包不包含玩家存档。
6. **签名**：当前 exe 未做 Authenticode 代码签名；如需正式对外分发，按发布流程补充签名与 SmartScreen 说明。
