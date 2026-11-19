# 素材审计 —— 哪些字节是游戏真正引用的

> 目的：回答一个具体问题 —— 本作 1.5 GB 的解包素材里，有多少是 WOLF 引擎自带示例 / 从未被任何事件引用 / 只被死代码引用？把「社区 Android 版删掉 200 MB 无关模板素材 + 贴图有损压缩再省 300 MB」这类说法，换算成我们自己可复现的字节数。

> 硬约束：本文档只描述**数据**，不涉及 `crates/**` 的任何改动。所有数字均由 `work/_audit_*.py` 产出，命令见 §0。

---

## 0. 测量时点与命令索引

### 0.1 时点

- 工作目录 `E:\Study\github program\imoto life`，HEAD `ac0138d`（M0 closed）。
- 数据源 `work/all/`：23 个 `.wolf` 全部解包后的产物（4,085 个文件 / 1,528,662,617 B）。
- 对照组：真实安装目录 `E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`（117 个文件 / 2,174,964,407 B）。
- 字节用 `os.path.getsize` 实测（不是 du 的簇大小）；像素用 PIL 打开后 `im.size` 相乘。
- 任何 crate 改动后请重跑 §0.2 的命令；本文档数字的保质期等同于 `work/all/` 的内容。

### 0.2 命令索引（全部在仓库根执行）

| 产出 | 命令 | 内容 |
|---|---|---|
| `work/_audit_out1.txt` | `python work/_audit_scan.py` | 引用扫描：3,037 条 distinct 路径、2,695 resolved、342 missing、逐目录原始表 |
| `work/_audit_out14.txt` | `python work/_audit_final.py` | **三桶对账**（live / deadref / never + 数据文件），§1 的唯一数据源 |
| `work/_audit_out19.txt` | `python work/_audit_table2.py` | **最终分类表**（files/bytes/live/liveB/dref/drefB/never/neverB）+ 可省字节 |
| `work/_audit_out20.txt` | `python work/_audit_pixels.py` | live 素材的图像/音频拆分 + 图集张数换算 |
| `work/_audit_out21.txt` | `python work/_audit_px2.py` | 逐目录像素表 + 单边 >2048 的图像清单 |
| `work/_audit_out12.txt` | `python work/_audit_closure2.py` | 可达性闭包（含自启事件种子）：414 节点 / 193 个死事件 |
| `work/_audit_out17.txt` | `python work/_audit_missing.py` | 342 条 missing 的 live/dead 归属 |
| `work/_audit_out24.txt` | `python work/_audit_miss3.py` | missing 按 (目录, 来源) 分组 + sysdb-only 统计 |
| `work/_audit_out25.txt` | `python work/_audit_miss4.py` | missing 的 Picture 按地图分组 + SampleMapA_1 名单 |
| `work/_audit_out26.txt` | `python work/_audit_sysg.py` | SystemGraphic 的 8 个 live 文件及其引用点 |
| `work/_audit_out27.txt` | `python work/_audit_dupes.py` | scene↔hscene、word↔hword 的同名/同内容比对 |
| `work/_audit_out28.txt` | `python work/_audit_top.py` | 未引用文件按字节排名（已排除数据文件） |
| `work/_audit_out32.txt` | `python work/_audit_boot3.py` | **M1 启动闭包**（TitleMap.mps ev2）的素材集 |
| `work/_audit_out36.txt` | `python work/_audit_bootpx.py` | M1 素材集的像素预算 + 逐文件清单 |
| `work/_audit_out34.txt` | `python work/_audit_install.py` | 真实安装目录 117 文件 / 2.17 GB 的构成 + 字体 md5 |
| `work/_audit_out35.txt` | `python work/_audit_arch.py` | 23 个 `.wolf` 逐个字节 + 安装目录 `Data/` 清单 |
| `work/_audit_out3.txt` | `python work/_audit_dirs.py` | 9 张地图 → tileset 映射 + 死目录样本 |
| `work/_audit_out5.txt` | `python work/_audit_scene.py` | scene/ 与 word/ 的正则引用确认（= 0 次）+ DataBase type9 指向 |
| `work/_audit_out16.txt` | `python work/_audit_dup.py` | live 素材内的重复内容比对（= 0）+ live 体积构成 |
| `work/_audit_out33.txt` | `python work/_audit_ev42.py` | SysDatabase type 0 的 6 张注册地图 |

### 0.3 引用扫描的方法（为什么可信）

六个来源合并成 `sites[path] = [(source, locator, opcode), ...]`：

1. **公共事件**（550 个）：`work/wolftl/dump/common/<id>_<title>.json` 的 `stringArgs`（WolfTL 导出的 UTF-8 JSON）。
2. **地图事件**（9 张 `.mps`）：`work/maps_full.json` 的 `strs` + `work/wolftl/dump/mps/*.json`（7 张）的 `stringArgs`。
3. **SysDatabase 的资产型 type**：1 BGMリスト / 3 SEリスト / 8 キャラクター画像 / 9 キャラ影 / 10 ウィンドウ画像 / 13 遠景画像 / 24 顔グラフィック名。
4. **DataBase 全部字符串字段**：按 `\r\n` 分行（字段里塞着「文件名\r\n音量\r\n周波数」这种多行值）。
5. **TileSetData**：6 个 tileset 的 `base_image` + 31 个 `autotiles`（由 Rust loader 导出为 `work/fs_ts.json`）。
6. **Game.dat**：字体名 / 加载图。

判定 `is_asset`：以 `.png/.jpg/.jpeg/.bmp/.webp/.gif/.mp3/.ogg/.wav/.m4a/.mid/.wma/.mp4/.webm/.ttf/.otf` 结尾，且**不是** `<...>` 尖括号名（尖括号是 WOLF 内置图元，不是文件，见 §3.4）。解析顺序：精确路径 → 大小写不敏感回退。

**字符串可信度**：`work/_ce_full.json`（550 个公共事件）与 `work/maps_full.json`（9 张地图）里的字符串经实测是正确 UTF-8 —— 550 个事件 71,474 个 stringArgs 中 U+FFFD 计数为 0；`maps_full.json` 的 8,267 个 strs 同样为 0。因此可以直接用来做路径匹配，不需要绕道 Rust 侧导出。

**可达性种子（关键）**：`docs/FORMAT_SPEC.md:333-337` 定义 `run_condition = raw & 0x0F`，全 550 事件的低 4 位直方图是 `{0:442, 1:1, 2:97, 3:10}` —— 也就是说 **108 个事件是并行/常驻协程，天然自启**。所以可达性种子 = 108 个 `run != 0` 的公共事件 + 6 张注册地图的 57 个地图事件 = 165 个种子。

> ⚠️ 只用「6 张注册地图」当种子会漏掉这 108 个自启事件，得到 306 节点 / 249 个可达公共事件的**错误**结论（`work/_audit_out7.txt`、`work/_audit_out23.txt` 里那批数字就是错的，**不要引用**）。正确结论在 `work/_audit_out12.txt`。

---

## 1. 总量表

### 1.1 磁盘全貌（`work/_audit_out14.txt` / `out35.txt`）

| 项 | 文件数 | 字节 | 占比 |
|---|---:|---:|---:|
| `work/all/` 总计 | **4,085** | **1,528,662,617** | 100% |
| ├ 23 个 `.wolf` 原始归档 | 23 | 757,077,610 | 49.5% |
| └ 解包产物 | 4,062 | 771,585,007 | 50.5% |
|   ├ 素材（22 个素材目录） | 3,904 | 763,976,724 | 49.98% |
|   └ 数据文件（BasicData + MapData） | 158 | 7,608,283 | 0.50% |

23 个 `.wolf` 逐个字节（`out35.txt`，降序）：`BGM` 248,438,212 ｜ `DLC_data` 154,450,763 ｜ `cg` 123,375,407 ｜ `BattleEffect` 113,360,799 ｜ `1.4.0` 33,551,305 ｜ `Picture` 25,041,363 ｜ `SE` 21,855,423 ｜ `hscene` 20,948,679 ｜ `cutin` 6,170,548 ｜ `SystemGraphic` 2,860,488 ｜ `EnemyGraphic` 2,659,705 ｜ `BasicData` 1,278,855 ｜ `MapChip` 702,136 ｜ `SystemFile` 666,343 ｜ `hword` 525,022 ｜ `CharaChip` 303,464 ｜ `Kagura` 282,740 ｜ `scene` 191,824 ｜ `Fog_BackGround` 149,609 ｜ `MapData` 146,824 ｜ `BalloonBase_CafeLatte` 57,721 ｜ `word` 40,681 ｜ `FIcon` 19,699。

> 归档与解包体积几乎相同（757.1 MB vs 771.6 MB），说明 `.wolf` 内部**没有做有意义的压缩**（dxarchive 的 LZ 主要针对重复模式）。这条对打包策略有直接影响，见 §5.1。

### 1.2 三桶对账（`work/_audit_out14.txt`）

| 桶 | 定义 | 文件数 | 字节 | 占解包体积 |
|---|---|---:|---:|---:|
| **live** | 被「可达代码」引用 | **2,689** | **608,156,264** | 78.8% |
| **deadref** | 只被「不可达代码」引用 | **6** | **336,706** | 0.04% |
| **never** | 从未被任何字符串引用 | **1,209** | **155,483,754** | 20.1% |
| **数据文件** | BasicData + MapData（不是素材） | 158 | 7,608,283 | 0.99% |
| 合计 | | 4,062 | 771,585,007 | 100% |

对账：608,156,264 + 336,706 + 155,483,754 + 7,608,283 = **771,585,007** ✓

**结论一句话：解包素材里 20.1%（155.5 MB）从未被任何事件 / 数据库 / 地图 / tileset 字符串提到过一次。**

6 个 deadref（被死代码引用、本身是活文件）：

| 字节 | 路径 |
|---:|---|
| 258,277 | `DLC_data/dlc_bg_019.jpg` |
| 28,333 | `Picture/paja_eye2.png` |
| 22,940 | `Picture/paja_mayu2.png` |
| 21,735 | `Picture/paja_kuchi2.png` |
| 4,468 | `SE/[System]Get3_wolf.ogg` |
| 953 | `SystemGraphic/Text_Pause.png` |

live 素材内部**没有重复内容**（全量 md5 比对，`work/_audit_out16.txt`：0 个冗余文件 / 0 B）—— 所以「去重」这条路在本作上省不到任何字节，可省字节只能来自 never 桶。

### 1.3 引用解析（`work/_audit_out1.txt`）

| 项 | 数量 | 字节 |
|---|---:|---:|
| distinct 引用路径 | 3,037 | — |
| resolved（磁盘上找到） | 2,695 | 608,492,970 |
| missing（被引用但文件不存在） | **342** | — |

resolved 2,695 里有 6 个只被死代码引用，所以 live = 2,689 / 608,156,264 B。missing 的构成见 §3.4。

### 1.4 安装体积能降到多少（`work/_audit_out19.txt` 尾部）

| 口径 | 字节 | 说明 |
|---|---:|---|
| 只保留 live 素材 + 数据文件 | **615,764,547**（587.2 MB） | 608,156,264 + 7,608,283 |
| 解包全量 | 771,585,007（735.8 MB） | 省 **155,820,460 B（20.2%）** |
| 若连 23 个 `.wolf` 一起丢掉 | 615.8 MB | 相对 1,528,662,617 B 全量省 **59.7%** |
| 真实安装目录 | 2,174,964,407（2.02 GB） | 615.8 MB 占其 **28.3%** |

---

## 2. 分类表

### 2.1 逐目录（`work/_audit_out19.txt`，字节降序）

| 目录 | 文件 | 字节 | live | live B | dref | dref B | never | never B |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| BGM | 49 | 248,514,721 | 40 | 223,256,712 | 0 | 0 | 9 | 25,258,009 |
| DLC_data | 633 | 155,203,817 | 575 | 145,655,256 | 1 | 258,277 | 57 | 9,290,284 |
| cg | 596 | 124,237,364 | 541 | 112,713,046 | 0 | 0 | 55 | 11,524,318 |
| BattleEffect | 92 | 113,672,178 | 44 | 21,434,351 | 0 | 0 | **48** | **92,237,827** |
| 1.4.0 | 199 | 33,705,094 | 181 | 31,866,936 | 0 | 0 | 18 | 1,838,158 |
| Picture | 539 | 27,000,868 | 468 | 24,047,869 | 3 | 73,008 | 68 | 2,879,991 |
| hscene | 327 | 22,847,281 | 300 | 21,477,607 | 0 | 0 | 27 | 1,369,674 |
| SE | 273 | 22,525,701 | 120 | 16,789,197 | 1 | 4,468 | 152 | 5,732,036 |
| cutin | 223 | 7,151,448 | 208 | 6,664,858 | 0 | 0 | 15 | 486,590 |
| BasicData | 149 | 6,383,077 | — | — | — | — | — | — |
| SystemGraphic | 28 | 2,861,080 | 8 | 2,678,621 | 1 | 953 | 19 | 181,506 |
| EnemyGraphic | 118 | 2,624,945 | 0 | 0 | 0 | 0 | **118** | **2,624,945** |
| MapData | 9 | 1,225,206 | — | — | — | — | — | — |
| MapChip | 102 | 778,669 | 54 | 528,551 | 0 | 0 | 48 | 250,118 |
| SystemFile | 32 | 702,549 | 18 | 151,062 | 0 | 0 | 14 | 551,487 |
| scene | 327 | 677,686 | 0 | 0 | 0 | 0 | **327** | **677,686** |
| hword | 119 | 555,617 | 103 | 477,994 | 0 | 0 | 16 | 77,623 |
| CharaChip | 91 | 327,651 | 7 | 38,661 | 0 | 0 | 84 | 288,990 |
| Kagura | 4 | 283,731 | 4 | 283,731 | 0 | 0 | 0 | 0 |
| Fog_BackGround | 10 | 146,048 | 0 | 0 | 0 | 0 | **10** | **146,048** |
| BalloonBase_CafeLatte | 13 | 127,932 | 8 | 70,510 | 0 | 0 | 5 | 57,422 |
| FIcon | 10 | 21,302 | 10 | 21,302 | 0 | 0 | 0 | 0 |
| word | 119 | 11,042 | 0 | 0 | 0 | 0 | **119** | **11,042** |
| **合计** | **4,062** | **771,585,007** | **2,689** | **608,156,264** | **6** | **336,706** | **1,209** | **155,483,754** |

BasicData / MapData 是游戏数据（`.dat` / `.project` / `.mps`），不参与素材分母，见 §2.2。

### 2.2 两套分母：数据文件 ≠ 素材

**BasicData/（149 文件 / 6,383,077 B）**（`work/_audit_out13.txt`）：

| 文件 | 字节 |
|---|---:|
| CommonEvent.dat | 5,532,175 |
| DataBase.project | 174,721 |
| CDataBase.project | 170,977 |
| DataBase.dat | 94,967 |
| CDataBase.dat | 81,832 |
| TileSetData.dat | 60,786 |
| SysDatabase.project | 35,972 |
| Game.dat | 29,273 |
| SysDatabase.dat | 11,607 |
| EncodedIconList.xxxxx | 3,354 |
| SysDataBaseBasic.project / .dat | 2,417 / 438 |
| MapTree.dat / MapTreeOpenStatus.dat | 416 / 75 |
| apple.png | 15,400 |
| icon000..icon133.png（134 个） | 168,667 |

**MapData/（9 文件 / 1,225,206 B）**：Dungeon / Map002 / Map003 / Map024 / Map031 / SampleMapA / SampleMapA_1 / SampleMapB / TitleMap，各 1 个 `.mps`。

> 这两类**不能删**：CommonEvent.dat 是全部剧本，`.mps` 是地图，删了游戏就没了。但它们也不该出现在「未引用素材」排名里 —— `work/_audit_out1.txt` 的「30 biggest unreferenced」里出现的 `BasicData/CommonEvent.dat 5,532,175` 与 `MapData/SampleMapA.mps 797,933` 是**分类错误**；正确口径见 `out28`（排除数据文件后，未引用最大的是 `BattleEffect/kaenbeam.png 18,952,170`）。

**MapData 的引用口径**：9 个 `.mps` 不在任何事件字符串里出现，它们的「引用」来自 `SysDatabase` type 0（マップ設定）的 `マップファイル名` 字段。扫描器按扩展名过滤，所以 MapData 的 live 记为 0 是**口径问题，不是死资产**。

---
