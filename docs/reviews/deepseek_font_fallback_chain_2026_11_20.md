# DxLib 原版字体绑定与缺字回退核查（2026-11-20）

**状态：部分验证。**

已静态核对 `Game.dat`、字体二进制与 `Game.exe` 的字体装载代码，并执行了独立 Windows GDI 隔离探针；未启动完整原版游戏做画面级截图验证，也未读写 Steam `Save/`。因此“字体如何注册、Lolita 缺字时 GDI 返回什么”已实测，“实际游戏窗口最终像素”仍是推导结论。

---

## 1. 结论先行

1. **出货显示的 2702 个 CJK 字符全部被 `Lolita.ttf` 覆盖。** `programs.json` 有 2651 个唯一 CJK，`database.json` 有 722 个；两者并集为 2703，其中唯一的缺字 `㥨 (U+3968)` 来自 3 个已乱码的 `MapChip/..._at㥨*.png` 资源文件名，不是会绘制的文本。排除资源名后，实际文本并集为 **2702，缺字 0**。
2. `Lolita.ttf` 对 **GB2312 6763 个汉字覆盖 0 缺字**。
3. `Game.dat` 的 4 个字体槽均为 `LOLITA`，且这里的值被解释为 **Windows 字体族名**，不是文件路径。`Lolita.ttf` 由 `Game.exe` 在启动时扫描 `*.ttf` / `*.ttc` 后用 **`AddFontResourceExW(..., FR_PRIVATE, ...)`** 私有注册；DxLib 再按族名 `LOLITA` 调 GDI `CreateFontW`。
4. **出货文本不触发缺字回退。** 对当前文本语料而言，Lolita 没有缺字，因此不会走到任何回退路径。
5. 对语料外的任意字符（例如玩家名、存档名或编号外字符），当前机器上的 Windows GDI 探针显示：**Lolita 缺字返回与“未分配码点”完全相同的 `.notdef` 位图，没有自动链接到 07yasashi、GenJyuuGothicXPB、MS Gothic 或系统中文字体**。因此原版的实际表现应是 **tofu/方框**，不是引擎级中文字体替换。
6. DxLib 源码中确实存在可选的逐字形 `AddSubstitutionFontToHandle` 候选表，但本作 `Game.dat` 的四个槽全是同一族名，且未发现 WOLF 配置独立替代字体；**静态证据不能证明本作存在唯一的中文字形回退链**。现有证据更强地指向“没有独立回退”。

---

## 2. 字体清单

文件大小为字节；哈希为 SHA-256；字形数来自 `maxp.numGlyphs`，cmap 数为 Unicode cmap 并集。时间戳是文件系统值，不等同于字体创作时间。

| 文件 | 家族名 | 大小 / 时间戳 | 字形 / cmap | 被 `Game.dat` 引用 | 备注 |
|---|---|---:|---:|---|---|
| `Lolita.ttf` | `Lolita` / `萝莉体` | 11,949,844 B；2025-11-17 | 35,893 / 35,898 | **是，4 槽** | SHA-256 `d91f680b870855d50619f72a1e597d2af5f068535e610a4fc95e5e53bab93449` |
| `Data/Lolita.ttf` | 同上 | 11,949,844 B；2021-07-12 | 同上 | 间接同名 | 与根目录文件**逐字节相同** |
| `07yasashi.ttf` | `07YasashisaGothicBold` / `07やさしさゴシックボールド` | 2,355,728 B；2025-11-17 | 15,563 / 14,963 | 否 | SHA-256 `0f5627b66aedb678b78b3126f8d9640b18731daa671f6786f65a5af8fa3549d82` |
| `Data/07yasashi.ttf`、`Data/07yasashisagoshitsukubo-rudo.ttf` | 同上 | 2,355,728 B | 同上 | 否 | 同一哈希的重复副本 |
| `GenJyuuGothicXPB.ttf` | `Gen Jyuu Gothic XP Bold` / `源柔ゴシックXP Bold` | 11,575,152 B；2025-11-17 | 19,599 / 17,737 | 否 | SHA-256 `7be46e2a309afdcfc233c873621aade3b17fda93f3872ffbf266000df1236a06` |
| `Data/08gothic.ttf`、`Data/08YuanRougoshitsukubo-rudo.ttf` | 同上 | 11,575,152 B | 同上 | 否 | 同一哈希的重复副本 |
| `MSGOTHIC.TTF` | `MS Gothic` / `ＭＳ ゴシック` | 4,170,144 B；2025-11-17 | 13,104 / 12,307 | 否 | SHA-256 `1a09158b3415204b6b5b7b1de90f391706645df053d10d5f96f32d7ea84a9cbe`；系统字体附带的副本，不是本作配置字体 |
| `PixelifySans-VariableFont_wght.ttf` | Pixelify Sans（可变字体） | 57,216 B | 未用于本核查 | 否 | 与本问题无关 |

补充：

- `Lolita.ttf` 的 Unicode 覆盖为 CJK 统一表意文字基本区 **20,908 / 20,992**，缺 84 个；CJK 扩展 A、扩展 B 完全未覆盖。
- 84 个缺字主要是 `U+9FA6..U+9FBB` 一带及少数其他基本区码点；GB2312 的 6763 个汉字和本作实际文本均不在缺口内。
- 07yasashi 与 GenJyuuGothicXPB 都覆盖了部分 Lolita 缺口（例如 `U+9FA6`），但这只说明“若有人显式把它们加入替代链，它们可补字”，不等于运行时真的会回退。

---

## 3. `Game.dat` 的四个槽

`work/all/BasicData/Game.dat` 为 29,273 B，SHA-256 `6C8CFDC797F479A380973DAA0CED7373EFC211BB4E7955EE408D78645234C1C5`。按公开 `GameDat.hpp` 的字符串表布局直接读取，字符串区（UTF-8、长度含 NUL）为：

| 索引 | 字段 | 值 | 文件内偏移 |
|---|---|---|---|
| 0 | `title` | 妹相随～黑白世界的缤纷冒险～ | 53 |
| 3 | `font_base` | `LOLITA` | 149 |
| 4 | `font_sub1` | `LOLITA` | 160 |
| 5 | `font_sub2` | `LOLITA` | 171 |
| 6 | `font_sub3` | `LOLITA` | 182 |
| 8 | `subtitle` | `+DLC v2.03` | 198 |

四个槽不是四个候选字体文件，而是 **0 号主字体和 3 个可由 `\font[n]` 选择的可替换族名槽**。本作四个槽相同，所以即使脚本切换 `\font[1..3]`，也只是切回同族字体，不产生缺字替换。

---

## 4. `Game.exe` 证据

### 4.1 身份与导入

- SHA-256：`98949e233e4085d9c745b2e8ba41e17ff49e5dafff16e5f28882d39bcaf804ea`。
- PE32 / i386，ImageBase `0x400000`，TimeDateStamp `0x64745ea5`（2023-05-29 UTC）。
- `GamePro.exe` 与 `Game.exe` 同一 SHA-256。
- GDI32 导入只有 `AddFontResourceExW`、`RemoveFontResourceExA`、`GetDeviceCaps`。字体创建/取字形由静态链接的 DxLib 通过 `LoadLibrary`/`GetProcAddress` 后调用，不在 PE 导入表中直接出现；二进制内可确认存在 DxLib 的 `DxFont.cpp` / `DxFontWin.cpp`、`CreateFontToHandle`、`GetGlyphOutline` 与字体缓存日志字符串。

### 4.2 字体文件怎样进入进程

在 `Game.exe` 的 RVA `0x4c3c00..0x4c3e9f` 可见两轮相同的字体目录枚举：

1. `FindFirstFileW(..., L"*.ttf", ...)` / `FindNextFileW` 循环；
2. 对每个命中项调用 `AddFontResourceExW(lpFileName, 0x10, 0)`。`0x10` 即 `FR_PRIVATE`，只对本进程可见，不写入系统全局字体。
3. 紧接着以 `L"*.ttc"` 重复同一过程。
4. 代码在扫描前后调用 `SetCurrentDirectoryW` 切换目录；静态上可确认存在根目录/`Data` 两个扫描上下文，但这两个全局路径最终对应哪个目录，因字符串对象在运行时构造，**静态证据强度为中**。
5. `AddFontResourceExW` 的实参是目录枚举结果中的短文件名，而非硬编码 `Lolita.ttf`；PE 中没有 `Lolita.ttf` 字面量，也没有“从 `Game.dat` 拼字体文件路径”的证据。

因此可确认：**Game.dat 的 `LOLITA` 不是路径，字体文件也不是靠这个字符串直接打开。** 现实流程是“扫描目录并把所有 ttf/ttc 私有注册 → GDI 字体族表里出现 `Lolita` → DxLib 按族名选择”。

证据强度：

- `AddFontResourceExW` 调用、`FR_PRIVATE`、`*.ttf/*.ttc` 枚举：**高**（指令与操作数直接可见）。
- 两轮目录分别就是安装根和 `Data`：**中**（控制流可见，但目录字符串需运行时确认）。
- `Game.dat` 的 `LOLITA` 最终由 GDI 族名匹配：**高**（未发现文件路径解析；DxLib 字体创建 API 是族名接口）。

### 4.3 DxLib 字体表语义

对照同源 DxLib 的 `DxFont.cpp` / `Windows/DxFontWin.cpp`（本机带 `GetFontUnicodeRanges`、`CreateFontToHandle`、字体缓存日志，推断为 3.24/3.25 代）：

- `CreateFontToHandle_PF` 直接调用 `CreateFontW`，`lfFaceName` 就是 WOLF 传入的族名。若 `CreateFontW` 失败，才依次尝试 `DEFAULT_CHARSET`、`SHIFTJIS_CHARSET`，最后退到 `ＭＳ ゴシック` / 默认字体。**这是“字体创建失败”的回退，不是逐字形回退。**
- DxLib 另有 `SubstitutionInfo[16]`，可由 `AddSubstitutionFontToHandle` 注册；`GetFontCacheChar_Inline` 在某个字符缓存失败时，会遍历这些替代字体句柄。**这是逐字形回退，但属于可选的显式 API，不是 Windows 自动行为。**
- 本作没有任何 `Game.dat` 字段或字符串把 `07yasashi` / `GenJyuuGothicXPB` / `MSGOTHIC` 声明为替代字体；四个槽又全是 `LOLITA`。因此即使 WOLF 内部存在调用 DxLib 替代 API 的代码，本作配置也构造不出一个不同于 Lolita 的中文字体链。
- `\font[0..3]` 是 WOLF 文本标签的字体槽选择；`Game.exe` 中有对应错误文本（“`\font` 的 `]` 找不到”“值超出 0～3”），其语义是切换槽位，而不是缺字回退。

**静态不能确定“唯一中文字形回退”。** 更强的结论是：没有证据表明存在这样的回退；实际运行结果更接近“主字体缺字即 `.notdef`”。

---

## 5. Windows GDI 的实际缺字行为（已执行隔离探针）

为排除“DxLib 之外还有 Windows 字体链接”的可能，本机执行了一个不启动游戏的 GDI 探针。探针不写注册表、不改系统字体，只在进程内：

1. `AddFontResourceExW(<原版>/Lolita.ttf, FR_PRIVATE, 0)`；
2. `CreateFontW(-48, 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, NONANTIALIASED_QUALITY, FIXED_PITCH, L"Lolita")`；
3. 选入兼容位图 DC，调用 `GetGlyphOutlineW(..., GGO_BITMAP, ...)`。

结果（位图 SHA-256 前缀）：

| 字体 / 码点 | GDI 返回 | 解释 |
|---|---|---|
| Lolita / `U+9FA5 龥`（有字） | 43×45 位图，`8ba8ec...` | 正常字形 |
| Lolita / `U+9FA6 龦`（缺字） | 38×38，`2cf67a...` | 与未分配码点相同 |
| Lolita / `U+FA0E`（缺字） | 38×38，`2cf67a...` | `.notdef` |
| Lolita / `U+0378`（未分配） | 38×38，`2cf67a...` | 基准 `.notdef` |
| MS Gothic / `U+9FA6` | 46×45，`b49fcc...` | 与 SimSun 的同一码点一致，说明 GDI `SystemLink` 生效 |
| SimSun / `U+9FA6` | 46×45，`b49fcc...` | 同上 |

本机 `HKLM\...\FontLink\SystemLink` 有 88 个族名条目，**没有 `Lolita`、`LOLITA` 或 `萝莉体`**。这解释了探针差异：MS Gothic 被登记到 SimSun 链接，Lolita 没有登记，因此不会自动链接到任何中文字体。

**重要限制：**

- 这是 64 位 GDI 探针，不是 32 位 `Game.exe` 的完整窗口截图；GDI 字体链接逻辑与位数通常无关，但严格画面级结论仍需实机。
- 由于全部出货显示文本已被 Lolita 覆盖，原版正常流程本来就不会遇到缺字，**出货文本不触发回退**。
- 缺字场景下，DxLib 的 `GetGlyphOutlineW` 得到非零 `.notdef` 位图，会把它缓存并绘制，所以表现是 **方框/tofu**，不是空白（除非字体自身 `.notdef` 为空）。

---

## 6. 回退链结论表

| 场景 | 实际绑定 | 证据强度 |
|---|---|---|
| 出货文本（2702 个实际 CJK） | Lolita 直接命中，不查回退 | **高** |
| 玩家名/输入/任意语料外 BMP 汉字 | GDI 选 Lolita，缺字得到 `.notdef`；无证据进入 07yasashi/GenJyuu/MS Gothic | **中高**（GDI 已实测，未做窗口截图） |
| GB2312 6763 汉字 | Lolita 全部覆盖 | **高** |
| CJK Ext A / Ext B / 84 个 URO 缺口 | Lolita 无字形；不会自动换字体 | **中高** |
| `\font[1..3]` | 切到同一 `LOLITA` 族，不是回退 | **高** |
| 主字体族完全缺失 | DxLib 创建字体失败时才退到 `ＭＳ ゴシック`/默认字体 | **高**（源码语义；本作不触发） |
| DxLib `AddSubstitutionFontToHandle` | 代码能力存在；本作未见显式配置，且四槽同字体 | **低/未知，不能证明有唯一回退** |
| Windows GDI `SystemLink` | Lolita 不在表内；探针显示 `.notdef` | **高（当前机器）** |
| 07yasashi / GenJyuuGothicXPB / MSGOTHIC | 只被通用字体扫描注册，未成为 Game.dat 或 DxLib 替代链成员 | **中高** |

---

## 7. 覆盖统计与边界

### 7.1 文本来源

- `game_data/programs.json`：所有事件/命令字符串，2651 个唯一 CJK。
- `game_data/database.json`：数据库字符串，722 个唯一 CJK；其中 3 个字符串是已经乱码的 `MapChip/*_at㥨*.png` 资源名，属于文件名而非 UI 文本。
- 两者并集：2703；剔除资源名中的 `㥨` 后是 **2702 个实际 CJK，Lolita 缺 0**。
- `Game.dat` 的标题/副标题/加载图名等字符串也均被覆盖。
- `DLC_data.wolf`（633 项）、`KAGURA.wolf`（4 项）、`scene.wolf`（327 项）、`word.wolf` / `hword.wolf`（各 119 项）的目录枚举只发现图像/音频，没有 `.dat`、`.project`、`.mps` 等文本数据；因此没有额外需要字体渲染的 DLC/补丁文本。

### 7.2 不可由静态语料保证的部分

- 玩家名、存档名、控制台/外部文本输入：字符集无界，可落入 84 个 URO 缺口、CJK Ext A/B 或任意非中文字符。
- 程序运行时拼接的数字、日期、按键提示、错误消息：已逐个覆盖的语言字符串都在 Lolita 内；极端的动态格式字符仍可能越界。
- 攻略文本、图片内文字不属于游戏字体渲染；不参与本字体链。

---

## 8. 隔离实验方案（可复现）

若需把上面的 GDI 结论升级为“原版 Game.exe 窗口级”证据，建议在 `work/out/...` 建沙盒：

1. 复制 `Game.exe`、`Game.ini`、根目录字体；用硬链接/只读副本构造 `Data/`，排除或替换 `Data/Lolita.ttf`，避免扫描到未修改副本。
2. 用 `fontTools` 打开完整 `Lolita.ttf`，只从一个或多个 Unicode cmap 子表删除目标码点映射（保留其他字形与族名），保存为测试字体。不要用 `subset` 过度裁剪，以免改变复杂度/度量。
3. 目标字符选 `U+9FA6`、`U+FA0E` 或某个可见 UI 汉字；用 `ttx`/`fontTools` 确认测试字体 cmap 中确已不存在该码点，且 `LOLITA` 族名不变。
4. 在沙盒启动原版，使用独立 `Save/`，截图含该字的标题/菜单/存档名。若显示方框且无法从其他字体取到该字，则确认无回退；若字符正常且字形明显不同，则存在系统/引擎回退。
5. 不在 Steam `Save/` 运行，不修改 `Data/*.wolf`，实验后清理沙盒。

本次未执行第 4 步；已执行第 1～3 步的 GDI 等价探针，结果为 `.notdef`。成本主要是启动窗口与输入自动化，风险可通过目录隔离消除。

---

## 9. 对复刻版的建议

1. **优先字体链：`Lolita.ttf` 第一。** 这样才能保持原版的字宽、换行、标点与描边观感；Godot 的字体加载应按文件/`FontFile` 绑定，不依赖 Windows 的 `LOLITA` 族名。
2. **不要把 07yasashi / GenJyuuGothicXPB / MS Gothic 放进主链。** 它们不是 `Game.dat` 配置的字体；07yasashi/GenJyuu 是日文字形倾向，MSGOTHIC 也不是本作的中文回退。把它们列为“已加载但未使用”更符合原版。
3. **保真模式：Lolita 缺字显示 tofu/方框，不自动替换。** 这与原版 GDI 探针一致；适合严格对照截图、逐字排版与回归测试。
4. **可玩模式：仅在语料外文本（玩家名、输入框、未来补字）启用系统 CJK 回退。** 建议链路为 `Lolita` → `Microsoft YaHei UI` / `Noto Sans CJK SC` / `Source Han Sans SC`，并记录每个替换字符，便于区分“原版行为”与“可玩性增强”。
5. **保留 `Lolita.ttf` 根目录/`Data` 两种探测路径。** 该安装两种位置都在，且哈希一致；发布包若不自带字体，应继续从用户目录探测，避免把商业字体提交进仓库。
6. **加两条自动检查：** (a) 2702 个实际文本 CJK + GB2312 6763 汉字覆盖必须为 0 缺字；(b) 人为删除一个 cmap 映射时，按所选策略断言“tofu”或“记录一次系统回退”，避免未来字体链静默改变。

---

## 10. 剩余风险与未解决项

- **未实机验证**：没有启动完整 `Game.exe` 做窗口级缺字截图；当前“tofu”结论来自 GDI 探针 + 静态代码，不是最终像素证据。
- **两轮扫描目录的精确映射**：静态控制流能确认两次 `SetCurrentDirectoryW` 和两次字体枚举，但目录字符串是运行时构造；只能高置信推断为安装根 + `Data`，未能从静态字符串给出绝对路径。
- **WOLF 是否调用 DxLib 的 `AddSubstitutionFontToHandle`**：未在 `Game.exe` 中定位到显式配置证据；即使存在，本作四槽同为 `LOLITA`，没有可区分的替代族。该点保留为“未知但无实际差异”。
- **系统差异**：FontLink 表随 Windows 版本/语言包变化；Lolita 未登记在当前机器，不能保证所有用户的注册表相同。复刻版不应依赖系统隐式回退来保证一致性。
- **字体许可**：`Lolita.ttf`/07yasashi/GenJyuu 的分发许可未在本报告核实；仓库继续按“运行时读用户原版目录”处理，不提交字体二进制。

---

## 11. 复核产物

临时探针与源码仅位于 `work/out/deepseek_font_fallback/`，核查结束后清理；未修改 Steam `Save/`，未提交 git。
