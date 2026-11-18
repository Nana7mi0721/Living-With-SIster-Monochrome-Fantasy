# Godot 重制版 Windows x64 发布说明（M5-A）

> 状态：**已完成**
>
> 完成日期：2026-11-18（Asia/Shanghai）
>
> 适用：Godot 4.7.2 .NET，Windows x64
>
> 本文记录 M5-A 的导出预设、打包脚本、版权审计与实测结果。仓库规则见 `AGENT.md`，版权边界见 `docs/godot/DECISIONS.md` 的 D-11。

## 1. 结论

- 导出预设：`godot/export_presets.cfg`，预设名 **Windows Desktop**，导出路径 `build/release/Lwsmf.exe`。
- 打包脚本：`tools/package_release.sh`，从仓库根执行，完成构建、导出、staging 与版权审计。
- 版权审计：`tools/copyright_audit.py`，最终审计通过。
- 最终包：`dist/Lwsmf-win64`，206 个文件，合计 189,666,077 字节，不含任何原版零售素材。

实际标记输出：

```text
EXPORT_PRESET_OK preset=Windows Desktop export_path=/e/Study/github program/imoto life/godot/build/release/Lwsmf.exe
COPYRIGHT_AUDIT_OK files=206 bytes=189666077 forbidden=0
RELEASE_PACKAGE_OK dist=/e/Study/github program/imoto life/dist/Lwsmf-win64
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
- `dotnet/embed_build_outputs=false` 是最终采用值。最初使用 `true` 时，导出能完成但运行 exe 会报 `Failed to open .dotnet-publish-manifest` / `Failed to load hostfxr`，随后崩溃（exit -1073741819）。改为 `false` 后导出产物会带一个必需的 `data_Lwsmf_windows_x86_64` 运行时目录，exe 可正常启动。
- `exclude_filter` 顶层排除 tests、tests_gd、game_data、work、build。pck 最终为 84,588 B，字节扫描确认其中没有 test/game_data/work/build 路径；`.NET` 程序集不在 pck 内，而在运行时目录中。

## 4. 打包命令与实测

在仓库根运行：

```bash
cd "E:/Study/github program/imoto life"
bash tools/package_release.sh
```

脚本默认使用：

- `GODOT_BIN=E:/Study/tools/godot-4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe`
- `DOTNET_BIN=E:/Study/tools/dotnet8/dotnet.exe`
- `DOTNET_ROOT=E:/Study/tools/dotnet8`

脚本实际流程：

1. 若 `godot/Lwsmf.sln` 不存在，创建临时 solution 并在退出时删除；不会留在仓库。
2. `dotnet build godot/Lwsmf.csproj -v:minimal`。
3. `godot ... --headless --path godot --export-release "Windows Desktop"`。
4. 将 `godot/build/release` 下全部产物复制到 `dist/Lwsmf-win64/game/`（包括 `data_Lwsmf_windows_x86_64/`）。
5. 复制 `converter/` 到 `dist/Lwsmf-win64/converter/`，删除 `__pycache__`、`.pyc` 和 `tests/`。
6. 生成 `dist/Lwsmf-win64/README.txt`。
7. 若已有 `dist/Lwsmf-win64`，先移到 `work/out/dist-backup-<时间戳>-<pid>`，不覆盖用户文件。
8. 运行 `tools/copyright_audit.py --dist dist/Lwsmf-win64`。

构建日志结果：

```text
Lwsmf -> E:\Study\github program\imoto life\godot\.godot\mono\temp\bin\Debug\Lwsmf.dll
已成功生成。
    0 个警告
    0 个错误
```

导出日志尾部结果：

```text
[ DONE ] savepack
EXPORT_PRESET_OK preset=Windows Desktop export_path=/e/Study/github program/imoto life/godot/build/release/Lwsmf.exe
```

## 5. 最终目录与体积

```text
dist/Lwsmf-win64/
├─ game/
│  ├─ Lwsmf.exe                         109,372,928 B
│  ├─ Lwsmf.pck                              84,588 B
│  └─ data_Lwsmf_windows_x86_64/        186 个文件（.NET 运行时与程序集，必须随 exe 一起发布）
├─ converter/                           转换器 Python 源码（含 convert.bat）
└─ README.txt                            1,526 B
```

总计：206 个文件，189,666,077 字节。

`data_Lwsmf_windows_x86_64` 不是可选的调试文件：`dotnet/embed_build_outputs=false` 时，exe 需要它来加载 .NET 运行时。发布或复制时不能只拿 `Lwsmf.exe` 和 `Lwsmf.pck`。

## 6. 版权审计

命令：

```bash
python tools/copyright_audit.py --dist dist/Lwsmf-win64
```

规则：

- 递归扫描发布目录（默认 `dist/Lwsmf-win64`）。
- 拒绝扩展名：`.dat`、`.mps`、`.wolf`、`.sav`、`.png`、`.jpg`、`.jpeg`、`.webp`、`.bmp`、`.ogg`、`.wav`、`.mp3`、`.ttf`、`.otf`、`.ttc`。
- 从 `game_data/manifest.json` 的 `source.files` 和 `assets` 构建 `sha256/size` 指纹；发布目录中即使扩展名不同，只要字节哈希命中零售文件，也判违规。
- `dist` 不存在或 manifest 缺失时明确报 `COPYRIGHT_AUDIT_ERROR` 并以非零码退出。

最终输出：

```text
COPYRIGHT_AUDIT_OK files=206 bytes=189666077 forbidden=0
```

审计覆盖了 `game/Lwsmf.exe`、`game/Lwsmf.pck`、`game/data_Lwsmf_windows_x86_64/`、`converter/` 和 `README.txt`；没有发现禁止扩展名或零售哈希命中。

## 7. 玩家使用流程

发布包只含重制版程序、转换器与说明，不含原版素材。玩家需要：

1. 安装 Python 3.10+，确保命令行可运行 `python`。
2. 准备自己合法持有的原版 Data 目录（该目录应直接包含 `BasicData\` 与 `MapData\`）。
3. 在发布包根目录（与 `game`、`converter` 同级）运行：

```bat
converter\convert.bat --root "D:\你的原版\Data" --out game_data --reports reports
```

4. 看到 `CONVERT_ALL_OK` 后运行：

```bat
game\Lwsmf.exe
```

转换器只读取本机指定的原版目录，并在本地写入 `game_data\` 和 `reports\`；不会上传或分发原版文件。若提示缺少 `BasicData\Game.dat`，说明 `--root` 指向的不是解包后的 Data 目录。

## 8. 验证记录

### 8.1 pck 排除验证

对 `dist/Lwsmf-win64/game/Lwsmf.pck` 做字节扫描，结果：

```text
res://tests/ 0
res://tests_gd/ 0
res://game_data/ 0
res://work/ 0
res://build/ 0
Lwsmf.dll 0
PCK_SIZE=84588
```

说明测试资源路径、游戏数据、工作目录和构建目录均未进入 pck；`.NET` 程序集位于 `data_Lwsmf_windows_x86_64`，不在 pck 内。

### 8.2 exe 启动烟测

```bash
"dist/Lwsmf-win64/game/Lwsmf.exe" --headless --quit
echo QUIT_EXIT=$?
```

结果：

```text
QUIT_EXIT=0
Godot Engine v4.7.2.stable.mono.official.ed1daf0bf
```

headless 模式会输出 `Failed to open 'user://logs/...'` 与 `Failed to read the root certificate store`，这是无窗口/无用户目录环境的日志与证书库警告，不影响退出码和发布包使用。

## 9. 已知边界与后续

1. **发布程序集已剔除 C# 测试源码**：`godot/Lwsmf.csproj` 增加 `ExportRelease` 条件的 `<Compile Remove="tests/**/*.cs" />`。最终 `data_Lwsmf_windows_x86_64/Lwsmf.dll` 为 237,056 B，`SaveLoadTest` / `SleepSoakTest` / `PerformanceProbeTest` 符号扫描均为 0；pck 的 `res://tests/` 字节扫描仍为 0。Debug 构建保留测试类型供 38 道门禁使用。
2. **运行时目录必须随包**：`data_Lwsmf_windows_x86_64/` 是 `.NET` 运行时与程序集，缺少时 exe 会在启动阶段失败。`tools/package_release.sh` 已自动复制；手工复制发布包时不要遗漏。
3. **模板依赖**：本机必须安装 `4.7.2.stable.mono` 导出模板；仅有非 mono 的 `4.7.2.stable` 会找不到模板或报 .NET SDK 相关错误。导出前可用本文第 2 节路径检查。
4. **`.gitignore`**：`/dist/`、`/godot/build/`、`/work/` 已在现有 `.gitignore` 中忽略，本任务不需要修改；未改 `.gitignore`。
5. **临时文件**：`tools/__pycache__` 为本地语法检查生成，非源码，已清理；`godot/Lwsmf.sln` 由打包脚本临时生成并在退出时删除，仓库不保留。
6. **签名**：当前 exe 未做 Authenticode 代码签名；如需正式对外分发，按发布流程补充签名与 SmartScreen 说明。
