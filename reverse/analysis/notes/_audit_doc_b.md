## 3. 引擎样例与死资产清单

### 3.1 全死目录（引用 0 次，可整目录排除）

| 目录 | 文件 | 字节 | 证据 |
|---|---:|---:|---|
| **scene/** | 327 | 677,686 | 正则 `(?<![A-Za-z0-9_])scene/` 在所有字符串里命中 **0** 次（`work/_audit_out5.txt`）。此前统计到的 2,131 次全部是 `hscene/` 的子串误匹配。 |
| **word/** | 119 | 11,042 | 同上，`word/` 独立前缀命中 **0** 次（480 次全是 `hword/`）。 |
| **EnemyGraphic/** | 118 | 2,624,945 | 0 次引用。`DataBase` type 9「敵キャラ個体デ—タ」的「画像ファイル」字段指向 `Picture/enemy_00N.png`，**不指向 EnemyGraphic/**（`work/_audit_out5.txt` 的 row0/row1/row2 实证：row1 是 `Picture/enemy_001.png`，row2 是 `Picture/enemy_002.png`）。 |
| **Fog_BackGround/** | 10 | 146,048 | 0 次引用。文件名是 `[BackGround]night1..3_pipo.jpg`、`[BackGround]sky1/2_pipo.jpg`、`space1/2_pipo.jpg`、`sunset1/2_pipo.jpg`、`[Fog]Cloud_pipo.png` —— WOLF 官方示例素材包。 |

**scene/ 与 word/ 不是 hscene/、hword/ 的副本**（`work/_audit_out27.txt`）：

- `scene/` 327 个 vs `hscene/` 327 个，basename 只重叠 **11** 个，md5 相同的对 **0**。scene 侧是 `y_bg_001.jpg`、`pose_01..09.png`、`pai_bg01.jpg`、`header_bar.jpg`（「Y」系背景 + 立绘位）；hscene 侧是 `h_bg_001.jpg` 系。
- `word/` 119 个 vs `hword/` 119 个，basename **全部相同**（`word_effect01..119.png`），但 md5 相同的对 **0** —— 同名不同图，是两套不同的字效素材。

⇒ 这 4 个目录合计 **574 个文件 / 3,459,721 B** 可以直接不进安装包。

### 3.2 半死目录（部分未引用）

| 目录 | 死文件 | 死字节 | 死的都是什么 |
|---|---:|---:|---|
| **BattleEffect/** | 48 / 92 | **92,237,827** | `pipo-btleffect*.png` 系列（WOLF 社区通用战斗特效包）+ `kaenbeam.png` 18,952,170 + `pipo-yaame.png` 5,004,303 + `wing_640.png` 4,498,012 + `water_*`。本作用的是 RPG Maker 风战斗立绘，不走这套特效。**这是最大的一块死资产，占全部 never 字节的 59.3%。** |
| **BGM/** | 9 / 49 | 25,258,009 | `sirorenga.mp3` 6,003,966、`Our_Construction.ogg` 5,457,387、`InstaVista.ogg` 5,069,232、`karuizawa-birds-1.mp3` 4,322,741、`summer.mp3` 2,004,113、`you_wind.mp3` 1,940,960、`wind1_night.mp3`、`decision21.mp3`、`decision27.mp3` —— WOLF 示例曲 |
| **cg/** | 55 / 596 | 11,524,318 | `cg/bg_18_home.png` 414,148、`training_b1..b3.jpg`、`map_02.jpg`、`bg_37_restaurant.jpg`、`event73_01.jpg`、`end1_02.jpg` 等 |
| **DLC_data/** | 57 / 633 | 9,290,284 | `dlc_yui07_04.jpg` 410,130、`dlc_command_bg_01c/02c/03c.jpg`、`dlc_yui04_02`、`dlc_yui05_06`、`dlc_yui08_11`、`dlc_end_01_3` 等 |
| **SE/** | 152 / 273 | 5,732,036 | 最大 `hse_fukumomu.mp3` 947,202 |
| **Picture/** | 68 / 539 | 2,879,991 | `FaceGraphic_Edy/Wolfarl/Yuichi.png`、`arrow.png`、`card0042/45/46.jpg`、`character01_1.png`、`paja_body.png` 627,395、`battle_icon_05 - 副本 - 副本.png` |
| **1.4.0/** | 18 / 199 | 1,838,158 | `comicon_01..09/12.png`、`v144_cg20_ef01/04.png`、`v144_cg3/5/6/7_1/2.jpg`、`v144_cg24_1.jpg` |
| **CharaChip/** | 84 / 91 | 288,990 | `[Animal]/[Chara]/[Monster]/[Special]` 示例图 + `point_*`。**live 只有 7 个**：`hand1.png hand2.png hand3.png look.png talk.png cursor.png mushroom.png`（`work/_audit_out24.txt`） |
| **MapChip/** | 48 / 102 | 250,118 | `[A]Dirt*` / `[A]Grass*` pipo 系列 |
| **SystemGraphic/** | 19 / 28 | 181,506 | `WindowBase.png`、`WindowBase_amania.png`、`CursorBase.png`、`cursol.png`、`CharaShade_8dir.png`、`[Transition]Fade.png`、**12 个 `[SE]*.ogg`**（`SystemGraphic/[SE]*` 被引用 0 次；真正活的是 `SystemFile/[SE]*` 的 13 个，`work/_audit_out24.txt` 实证） |
| **SystemFile/** | 14 / 32 | 551,487 | 14 个未引用 |
| **hword/** | 16 / 119 | 77,623 | 16 个未引用 |
| **BalloonBase_CafeLatte/** | 5 / 13 | 57,422 | 5 个未引用 |

### 3.3 两个反例：1.4.0/ 与 Kagura/ 不是死目录

`1.4.0/` 看起来像版本更新残留，但**181 / 199 个文件是 live 的**：字符串里 `1.4.0/` 前缀出现 **378 次（公共事件）+ 8 次（地图）**，例如 `ev221` 的 `1.4.0/SD_basya.png`、`ev245` 的 `1.4.0/base_bottan.png` 与 `1.4.0/SD_menu_010.png`。这是一套**存档缩略图 / UI 按钮的补丁素材**，删了会缺图。

**Kagura/** 4 个文件 100% live，但只属于启动链（`work/_audit_out26.txt`）：`Kagura/bg.png` 2,650（TitleMap ev2 的纯黑底板）、`Kagura/Kagura.png` 70,940（ev549 splash 1）、`Kagura/Disc.png` 87,298（splash 2）、`Kagura/inusuku.png` 122,843（splash 3）。

**SystemGraphic 的 8 个 live 文件**（`work/_audit_out26.txt`）：`TitleGraphic.jpg` 358,497（ev42#20/#30）、`TitleGraphic.png` 424,764（TitleMap ev2#84、Map003 ev1#19）、`TitleGraphic1..6.jpg` 324,826 / 314,916 / 304,294 / 310,515 / 303,241 / 337,568（ev42 的 #17/27/37/40/43/48/51/67/71/75/81/85）。

> ⚠️ `TitleGraphic1..6.jpg` 严格按可达性算是 **live**（ev42 可达），但真实标题画面走的是 `Picture/title01.jpg`（`docs/M1_CLOSURE.md` v2 已用像素比对确证）。也就是说 ev42 那套「オリジナルタイトル画面Ａ」示例标题图**会被加载但不会显示**——这是我们运行时可以明确不做的一条路径。

### 3.4 342 条 missing 的真实构成（`work/_audit_out17/24/25.txt`）

| 目录 | 条数 | 来源 | 性质 |
|---|---:|---|---|
| Picture | 191 | 地图 138 + sysdb 45 + common 8 | 138 条全部来自 `SampleMapA_1` 的 ev7（167 次）：`Picture/skill01..75.png`、`skill*_report.png`、`chart01..18*.png`、`quest_result*.png`、`battle_icon_end.png` —— **WOLF 示例 RPG 的技能 / 报表图标**，本作未用。`Map002` 3 条（`map_Village01_top.png`、`explosion.png`、`attack_effect01.png`）。 |
| CharaChip | 132 | 全 sysdb type8 キャラクター画像 | `Nereis001/002.png`、`atk1..22_l/r.png` —— 磁盘上 CharaChip 只有 91 个文件且全是示例图，这些名字是**数据库注册表里的空槽位**。 |
| MapChip | 9 | tileset5 | **编码缺陷，不是缺资产**（见下） |
| cg | 2 | `cg/bg01.jpg`（sysdb type13 row0）、`cg/event_14_06.jpg`（ev380#760） | 真缺 |
| BGM | 2 | `BGM/Battle01_Juno.mid`（DataBase type17）、`BGM/kana.mp3`（sysdb type1 row5） | 真缺（未验证是否被触达） |
| 假阳性 | 6 | 运行时拼接 | `../save/save/cself[30].jpg`、`<<GET_FILE_EXIST>>./save/save/cself[30].jpg`、`Save/save/cself[12].jpg`、`save/save1.jpg`、`Data/SC1.jpg`、`SC1.jpg` —— 字符串里带 `cself[...]` 变量码，运行时才求值（来自 ev131/ev435/ev452） |

**关键统计**：sysdb-only missing 179 条，其中**被事件直接引用的 = 0**（`work/_audit_out24.txt`）。也就是说这些名字只存在于数据库注册表里，游戏从不按名加载它们 —— 属于「未使用的注册项」，不是缺资产。

**tileset5 的编码缺陷**：`work/all/BasicData/TileSetData.dat` 里 tileset5（海岸ダンジョン，base `MapChip/map_parts01.png`）的 12 个非空 autotile 名中，**8 个是 cp932 双字节序列而不是 UTF-8**。原始 hex：

`
4d6170436869702f625f3032  f46281ba a5a2a5a2  5f6174  cde3a5a8  30322e706e67
  M a p C h i p / b _ 0 2    (cp932 坏字节)     _ a t   (cp932)   0 2 . p n g
`

按 cp932 解码得到 `MapChip/b_02.._at02.png` 这种「被写坏的日文名」，磁盘上真实存在的是 UTF-8 的 `b_02高山_at水02.png`、`b_04荒地_at水.png`、`b_05砂漠_at01水.png`、`b_06海岸_at01..03.png`、`b_07洞窟_at水.png`、`b_07洞窟_t01.png`、`b_at滝.png`。⇒ **修复方法：把 TileSetData 的名字按 cp932 重新解码后再查盘**（WOLF 编辑器写坏，Rust loader 需要容错）。

**尖括号名不是文件**：`CommonEvent.dat` 里共 24 种 `<...>` 名（`<C>` ×106、`<SQUARE>` ×92、`<R>` ×27、`<L>` ×14、`<GRADX-744-400>` ×11、`<LINE>` ×7、`<TRI-RIGHT>`、`<CIRCLE>`、`<FRAME>` …），mps 侧同样分布（`<SQUARE>` ×351、`<C>` ×123）。这些是 WOLF 运行时**程序化绘制的图元**（矩形 / 圆 / 三角 / 渐变 / 线 / 框），扫描时已排除，不算 missing。

### 3.5 事件层死代码（`work/_audit_out12.txt`）

- 含自启种子（165 个）的全游戏闭包：**414 节点 = 357 个公共事件 + 57 个地图事件**。
- **仍有 193 个公共事件从任何入口都不可达**（`run != 0` 的死事件为 0 个，说明自启事件本身都活着）。
- 启动闭包（TitleMap.mps ev2）：**62 节点 = 61 个公共事件 + 1 个地图事件**，与 `docs/M1_CLOSURE.md` 一致。
- 启动 + 自启闭包：215 节点 / 214 个公共事件。

事件死代码**不等于素材死**：素材可达性按「引用它的那个事件是否可达」算，§1.2 的 live 桶已经是可达性修正后的结果。

动态调用不可静态解析：`210` 有 18,458 条字面量 + **10 条非常量**（ev70 `raw=1600010`、ev127 `1600024` ×4、ev165 `1600011`、ev168 `1600048`、ev183 `1600037`、ev188 `1600030`、ev197 `1600012` —— 全是 self 变量码）；`300` 有 1,164 条，仅 2 个标题解析失败（`▲┣[ズ—ム]イベント位置をズーム座標に`、`○◆マップをズーム`，各 4 次）。⇒ 闭包是**下界**，真实 live 桶可能比 2,689 略大，但上限是 resolved 的 2,695，只差 6 个文件。

### 3.6 未注册地图（死资产）

`SysDatabase` type 0（マップ設定，50 行）里非空且带 `マップファイル名` 的只有 **6 行**（`work/_audit_out33.txt`）：

| 行 | 名称 | 地图 |
|---:|---|---|
| 0 | タイトル | `MapData/TitleMap.mps` |
| 1 | 自宅1 | `MapData/SampleMapA.mps` |
| 2 | クマヘ | `MapData/SampleMapA_1.mps` |
| 3 | ニューゲーム処理 | `MapData/Map003.mps` |
| 24 | ロード処理 | `MapData/Map024.mps` |
| 31 | セーブ | `MapData/Map031.mps` |

磁盘上有 9 个 `.mps`，未注册的 3 个是 **`Dungeon.mps`（40×30, tileset0, 79 cmds）、`Map002.mps`（100×30, tileset4, 416 cmds）、`SampleMapB.mps`（30×30, tileset0, 36 cmds）**（`work/_audit_out3.txt`）。

交叉验证：全部 `130 Teleport` 指令的 arg[0] 只有 `-2`（4 次）和 `-1`（5 次）两种取值，**没有任何传送指向未注册的地图索引**（地图事件侧 9 条：Map002 `-2`×4、Dungeon `-1`、Map003 `-1`、Map031 `-1`、SampleMapB `-1`；公共事件侧 1 条 `-1`）。⇒ Dungeon / Map002 / SampleMapB 是纯死资产（WOLF 编辑器示例图）。

⚠️ 注意 `SampleMapA.mps` 虽然注册，但它有 **31,083 条指令**（`nev=40`，示例 RPG 的大地图），本作实际只在「自宅1」这一张图上活动 —— 这解释了为什么 138 条 Picture missing 全来自 `SampleMapA_1`（示例地图事件引用的示例图标）。

---

## 4. 体积排名

### 4.1 未引用最大的 30（`work/_audit_out28.txt`，已排除数据文件）

| # | 字节 | 路径 |
|---:|---:|---|
| 1 | 18,952,170 | `BattleEffect/kaenbeam.png` |
| 2 | 10,713,886 | `BattleEffect/pipo-btleffect162.png` |
| 3 | 9,856,806 | `BattleEffect/pipo-btleffect207_480.png` |
| 4 | 8,193,996 | `BattleEffect/pipo-btleffect191_640.png` |
| 5 | 6,003,966 | `BGM/sirorenga.mp3` |
| 6 | 5,457,387 | `BGM/Our_Construction.ogg` |
| 7 | 5,188,385 | `BattleEffect/pipo-btleffect133.png` |
| 8 | 5,091,710 | `BattleEffect/pipo-btleffect144.png` |
| 9 | 5,069,232 | `BGM/InstaVista.ogg` |
| 10 | 5,004,303 | `BattleEffect/pipo-yaame.png` |
| 11 | 4,498,012 | `BattleEffect/wing_640.png` |
| 12 | 4,322,741 | `BGM/karuizawa-birds-1.mp3` |
| 13 | 3,868,795 | `BattleEffect/pipo-btleffect023.png` |
| 14 | 3,456,172 | `BattleEffect/pipo-btleffect192_640.png` |
| 15 | 3,072,119 | `BattleEffect/pipo-btleffect145.png` |
| 16 | 2,750,266 | `BattleEffect/pipo-btleffect190_640.png` |
| 17 | 2,438,364 | `BattleEffect/pipo-btleffect201_480.png` |
| 18 | 2,402,351 | `BattleEffect/pipo-btleffect157.png` |
| 19 | 2,004,113 | `BGM/summer.mp3` |
| 20 | 1,940,960 | `BGM/you_wind.mp3` |
| 21 | 976,512 | `BattleEffect/pipo-btleffect132.png` |
| 22 | 947,202 | `SE/hse_fukumomu.mp3` |
| 23 | 935,949 | `BattleEffect/pipo-btleffect152.png` |
| 24 | 627,395 | `Picture/paja_body.png` |
| 25 | 569,192 | `BattleEffect/pipo-btleffect160.png` |
| 26 | 565,336 | `BattleEffect/pipo-btleffect030.png` |
| 27 | 486,293 | `BattleEffect/pipo-btleffect034.png` |
| 28 | 462,659 | `BattleEffect/pipo-btleffect031.png` |
| 29 | 414,148 | `cg/bg_18_home.png` |
| 30 | 410,130 | `DLC_data/dlc_yui07_04.jpg` |

前 30 名合计 **116,680,550 B（111.3 MiB）**，占 never 桶的 **75.0%**；其中 `BattleEffect/` 占 22 项。

### 4.2 被引用最大的 15（`work/_audit_out1.txt`）

| 字节 | 路径 |
|---:|---|
| 22,218,222 | `BGM/ango.ogg` |
| 11,702,400 | `BGM/nekomimi.mp3` |
| 11,607,952 | `BGM/RAIN_&_Co.ogg` |
| 11,516,723 | `BGM/okiraku.ogg` |
| 10,340,339 | `BGM/Sunshine!.ogg` |
| 10,202,646 | `BGM/Killer_Roll.ogg` |
| 9,993,489 | `BGM/We_are_Going.ogg` |
| 9,722,294 | `DLC_data/bgm01.ogg` |
| 9,281,620 | `DLC_data/bgm02.ogg` |
| 9,023,964 | `DLC_data/bgm09.mp3` |
| 8,887,068 | `BGM/straightcontrast.ogg` |
| 8,812,472 | `BGM/madoromu2.ogg` |
| 8,632,962 | `BGM/destiny.ogg` |
| 8,454,473 | `BGM/The_End_of_Destruction.ogg` |
| 8,367,346 | `BGM/line.ogg` |

### 4.3 live 桶的体积构成（`work/_audit_out16.txt`）

| 目录 | 字节 | 占 live |
|---|---:|---:|
| BGM | 223,256,712 | 36.7% |
| DLC_data | 145,655,256 | 24.0% |
| cg | 112,713,046 | 18.5% |
| 1.4.0 | 31,866,936 | 5.2% |
| Picture | 24,047,869 | 4.0% |
| hscene | 21,477,607 | 3.5% |
| BattleEffect | 21,434,351 | 3.5% |
| SE | 16,789,197 | 2.8% |
| 其余 8 个目录 | 10,915,290 | 1.8% |

⇒ **live 桶 90% 是音频 + 大图**（BGM 36.7% + DLC_data 24.0% + cg 18.5%），不是引擎逻辑能省的。真要减体积只能走有损压缩（就是社区 Android 版做的事）。

---

## 5. 对我们的意义

### 5.1 打包策略：三档，M1 不需要全量

| 档 | 内容 | 字节 | 相对 1.5 GB 全量 |
|---|---:|---:|---:|
| **M1 启动档** | TitleMap ev2 闭包（61 个公共事件 + 1 个地图事件）引用到的素材 | **28 文件 / 18,126,094**（17.3 MB） | 1.19% |
| **M2 切片档** | live 素材 + 数据文件 | 615,764,547（587.2 MB） | 40.3% |
| **完整档** | 解包全量（含 never 桶） | 771,585,007（735.8 MB） | 50.5% |

M1 那 28 个文件（`work/_audit_out36.txt` 逐文件清单，图像 25 个 / 8,089,366 px = **1.93 张 2048² 图集**，音频 3 个）：

- `BGM/White_Base.mp3` 2,819,135、`BGM/nekomimi.mp3` 11,702,400、`SE/[Action]DoorOpen_Komori.ogg` 14,338
- `Kagura/bg.png` 2,650、`Kagura/Kagura.png` 70,940、`Kagura/Disc.png` 87,298、`Kagura/inusuku.png` 122,843
- `Picture/title01.jpg` 280,653（真实标题图）
- `SystemGraphic/TitleGraphic.png` 424,764 + `TitleGraphic.jpg` 358,497 + `TitleGraphic1..6.jpg` 2,205,384（8 个示例标题图，合计 2,988,645）
- `SystemFile/Button01.png` 13,042、`CharaChip/hand3.png` 16,337
- `DLC_data/save_SD_*.png` 10 个（akatuki / boss / clear / cook / dlc / gassyuku / start / start2 / taikai / tensai），合计 317,837

**归档不要随包发**：23 个 `.wolf` 是 757,077,610 B，解包后是 771,585,007 B（仅膨胀 1.9%）。既然我们最终要读明文，安装包里只需二选一 —— 保留 `.wolf` 让用户自己解包，或直接发解包目录（省掉用户的解包步骤）。**两者不能同时发**，否则 1.5 GB 里有一半是重复内容。

### 5.2 atlas 只增不减必然爆（最硬的一条约束）

- 当前实现：`crates/wolf-game/src/atlas.rs:25` `pub const ATLAS_SIZE: u32 = 2048;` —— 单张 2048² RGBA，M1 的 capture 只占 **14.1%**（`cargo run -q -p wolf-cli -- capture --frames 200` 输出）。
- live 素材的图像总量：**2,506 个文件 / 312,870,884 B（298.4 MiB）/ 1,106,149,163 px（1.11 Gpx）**（`work/_audit_out20.txt`，0 张不可读）。
- 换算：2048² 需 **263.7 张**；4096² 需 65.9 张；8192² 需 16.5 张；16384² 需 4.1 张。
- **116 张图像单边 > 2048**（合计 284,738,784 px，占 25.7%），最大的 `DLC_data/dlc_kasumi07_kuchi.png` 9216×576、`DLC_data/dlc_kasumi04_eye.png` 9216×576、`BattleEffect/warp_640.png` 1536×7488、`cg/staff.png` 1024×6500 —— 这些**根本贴不进 2048² 图集**，必须走「大图单独纹理」通道。
- 逐目录像素（`work/_audit_out21.txt`，2048² 张数）：cg 84.0 ｜ DLC_data 79.6 ｜ hscene 36.7 ｜ Picture 25.4 ｜ 1.4.0 22.7 ｜ BattleEffect 11.4 ｜ cutin 2.0 ｜ SystemGraphic 1.1 ｜ Kagura 0.6 ｜ MapChip 0.2 ｜ hword 0.1。

**结论**：atlas 必须是「按当前帧需求增量打包 + LRU 淘汰」，不能是「加载过的都留在里面」。M1 只有 6 张图存活所以看不出问题；M2 一进 `cg/`（84 张图集）就会立刻暴露。这条应写进 M2 的设计前提。

### 5.3 字体在 `.wolf` 之外（Rust 版必须从安装根目录找）

真实安装目录 117 个文件 / 2,174,964,407 B（`work/_audit_out34.txt`）：

| 位置 | 文件 | 字节 |
|---|---:|---:|
| 根目录 | 23 | 1,357,874,454 |
| ├ `Living With Sister Monochrome Fantasy CN Patch v5 with Cosplay DLC.exe` | 1 | **1,308,781,184（占安装体积 60.2%）** |
| ├ `Lolita.ttf` | 1 | 11,949,844 |
| ├ `GenJyuuGothicXPB.ttf` | 1 | 11,575,152 |
| ├ `Game.exe` / `GamePro.exe` | 2 | 7,329,792 ×2 |
| ├ `MSGOTHIC.TTF` | 1 | 4,170,144 |
| ├ `07yasashi.ttf` | 1 | 2,355,728 |
| ├ `Config.exe` | 1 | 2,127,872 |
| ├ `GuruguruSMF4.dll` | 1 | 147,456 |
| ├ `PixelifySans-VariableFont_wght.ttf` | 1 | 57,216 |
| └ 其余（7 张截图 / index.html / index.1.html / Game.ini / dlc.dat / Script.vdf） | 12 | ≈ 1.36 MB |
| `Data/` | 27 | 784,939,370 |
| ├ 23 个 `.wolf` | 23 | 757,077,610 |
| └ 4 个 `.ttf` | 4 | 27,861,760 |

字体细节：

- `Data/` 下 4 个 ttf 两两 md5 相同：`07yasashi.ttf` == `07yasashisagoshitsukubo-rudo.ttf`（a297ba75b7d0）、`08gothic.ttf` == `08YuanRougoshitsukubo-rudo.ttf`（be401bb87e76）—— **实际只有 2 套字体，重复占了 13.9 MB**。
- 根目录 `07yasashi.ttf` 与 `Data/` 那份同 md5；`GenJyuuGothicXPB.ttf` 与 `Data/08gothic.ttf` 同 md5。
- `Game.json` 只声明 `MainFont=LOLITA`、`SubFonts=[LOLITA,LOLITA,LOLITA]`，而 **`Lolita.ttf` 不在 `Data/` 里，在游戏根目录**（11,949,844 B，md5 559b2f1c068d）。
- `work/full/Fonts/` 里的 5 个 ttf 是从**根目录**拷来的（含 `MSGOTHIC.TTF`、`PixelifySans`），所以 M1 的 `find_font()` 走 `<root>/Fonts` 能命中 —— 但那只是因为手工拷贝过。**从零安装时字体在 `Data/` 之外，必须同时探测安装根目录。**

⇒ 建议资源根解析顺序：用户指定的游戏根目录 → `<root>/Data/` 解包缓存 → `<root>/Fonts` → 安装根目录。当前 `crates/wolf-cli/src/main.rs:1136` 的 `game_dir()` 是全项目唯一硬编码路径，M2 必须去掉。

### 5.4 与社区 Android 版的对照

| 社区 README 的说法 | 我们实测的数字 | 结论 |
|---|---|---|
| 「删去日文原版里 200 MB 多的与妹相随无关的 RPG Maker 模板素材图片」 | 从未引用素材 **155,483,754 B（148.3 MiB）**；其中 WOLF 示例素材（BattleEffect pipo 系列 + 示例 BGM + Fog / EnemyGraphic / scene / word）≈ **133 MB** | 量级吻合。他们的 200 MB 含 RPG Maker MV 侧自己的模板素材，不是同一批文件，但**同一个现象** |
| 「无用的 .m4a 音频文件与 .jpg 图片」 | BGM 死 25,258,009 B / 9 个；SE 死 5,732,036 B / 152 个 | 吻合 |
| 「为所有贴图做了有损压缩，减少约 300 MB」 | live 图像 312,870,884 B（298.4 MiB） | 量级吻合 —— 说明他们压缩的是**全部贴图**，不只是死资产 |

**关键差异**：他们是「授权重写成 RPG Maker MV」，必须把 WOLF 数据转成 MV 资源格式，顺带删掉 MV 模板素材；我们是「重实现 WOLF 运行时」，**不需要转换格式，可以直接按需加载**，所以省字节的方式应该是「按需解包 / 懒加载」而不是「预先删文件」。

### 5.5 给 lead 的 crates 侧需求（我不改 `crates/**`，只提字段清单）

当前审计靠 Python 读 `work/wolftl/dump/**` + `work/_ce_full.json` + `work/maps_full.json` + `work/fs_ts.json`。要在 Rust 侧一条命令复现，需要 `wolf-cli audit` 子命令导出以下字段（JSON 到 stdout 或文件）：

1. **事件侧**：每个公共事件 `{id, run_condition, slots, instrs, cmds:[{opcode, ints, strs}]}`（`strs` 必须是 UTF-8 原文）—— 现有 `CommonEventData::parse` 已足够。
2. **地图侧**：每张 `.mps` `{file, title, tileset, w, h, events:[{id, x, y, trig, cmds}]}` —— 现有 `Map::parse` 已足够。
3. **SysDatabase**：`{types:[{index, name, rows:[{name, fields:[{name, value}]}]}]}` —— 现有 `Database::parse` 已足够，但需要**保留原始 type index 与 field 顺序**。
4. **DataBase / CDataBase**：同上，且字符串字段要**保留 `\r\n` 分隔**（现在 `Database::parse` 是否保留需确认）。
5. **TileSetData**：`{tilesets:[{index, name, base_image, autotiles, tags, passability}]}` —— 现有 `TileSetData::parse` 已足够，**但 autotile 名需要 cp932 容错解码**（§3.4 的 8 个坏名字）。
6. **Game.dat**：`{title, title_plus, fonts_main, fonts_sub, loading_picture, starting_hero_graphic}` —— 现有 `GameDat::parse` 已足够。
7. **资源根**：`{install_root, data_dir, fonts:[{path, size, md5}]}` —— 需要新写（现在只有 `game_dir()` 硬编码）。

有了这 7 项，审计脚本可以缩成 ~150 行 Rust + 一个 `--json` 开关，且**彻底摆脱 Python 侧对 dump 目录的依赖**。

### 5.6 未验证项

1. **`SampleMapA.mps` 的 31,083 条指令是否真的会执行** —— 它是注册地图（行 1「自宅1」），但本作实际是否只用到它的少数事件未验证。若它的示例事件（如 ev7）从不执行，则 138 条 Picture missing 与部分 live 判定会变。
2. **`cg` / `hscene` 的 live 比例偏高（90.7% / 94.0%）是否因为「数据库注册表引用」而非实际执行** —— sysdb type13 遠景 / type8 キャラクター画像 这类注册表引用被算成 live，但注册项可能从不按名加载（§3.4 已证明 sysdb-only missing 179 条从不被事件引用）。若把「仅 sysdb 引用」降级为 deadref，cg / hscene / CharaChip 的 live 比例会显著下降，可省字节会从 155.5 MB 升到接近 200 MB —— 这条与社区版「200 MB」的说法正好对上，但**当前未量化**，不要引用这个数。
3. **`BattleEffect` 的 44 个 live 文件是否真的会在战斗里加载** —— 它们全部由公共事件的 150 指令引用（common 47 次 + db 31 次 + map 4 次），但本作战斗是 RPG Maker 风立绘制，是否走 pipo 特效未在运行时验证。
4. **`BGM` 的 40 个 live 曲是否都被播放** —— 引用来自 SysDatabase type1 BGMリスト（16 行）与事件里的 `140` 指令，注册≠播放。
5. **342 条 missing 是否会被实际执行路径触达** —— 全部按静态引用统计，未做运行时验证；sysdb-only 的 179 条已知不被事件引用，其余 163 条未验证。
6. **`DLC_data` 的 575 个 live 文件是否都随 DLC 解锁才加载** —— 若用户没买 Cosplay DLC，这 145.7 MB 里可能有相当一部分可以延后下载（未验证 DLC 开关机制）。

---

## 6. 一句话结论

**1.5 GB 里有 155.5 MB（20.1%）是从未被引用过的 WOLF 引擎示例素材，其中 92.2 MB 是 `BattleEffect/` 的 pipo 战斗特效包；把 never 桶整批排除后安装体积降到 587.2 MB（省 20.2%），而真正决定体积的是 live 里的 298 MiB 贴图 —— 这才是 atlas / 懒加载要解决的问题，删文件省不了多少。**
