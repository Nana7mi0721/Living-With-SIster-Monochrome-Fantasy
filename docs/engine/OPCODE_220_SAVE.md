# OPCODE 220 / 221 / 222 —— 存档三件套

> 数据源：`work/_ce_full.json`（550 个公共事件的全部指令）+ `work/maps_full.json`（9 张地图的全部事件）。
> 统计/取证脚本：`work/_p24h.py`（事件清单）、`work/_p24i.py`（站点普查）、`work/_p24j.py`（变量写者）、`work/_p24l.py`（221 站点全表）。
> 实现：`crates/wolf-save`（格式权威文档 `docs/SAVE_SPEC.md`）。
> 标记约定【已验证】= 有字节级或站点级实证；【未验证】= 推断、数据不足或本作数据无法区分。

---

## §0 一句话结论

| opcode | 一句话语义 |
| --- | --- |
| 220 SaveLoad | 把**整局状态**写进 / 读出一个存档文件（`mode` 0=保存 1=读取） |
| 221 LoadGame | 从某个存档文件里**读出一个变量**，写进运行中的变量 |
| 222 SaveGame | 把**一个变量**写进某个存档文件 |

三条指令都只是「文件读写原语」。**存档界面完全不在它们里面**：界面是 commonevent 94（`X[移]セーブ·ロード画面描画`，698 条指令、77 条 150、0 条 101、0 条 102）用图片拼出来的，单槽动作在 commonevent 131（`X┣[移]セーブ欄実行`），缩略图是 ProFeature 1000 的 JPG 截图（§5）。所以「做一个 Rust 存档界面」= 重写 94 的绘制 + 131 的流程，而不是实现 220。

---

## §1 站点普查【已验证】

| opcode | 公共事件（CE） | 地图事件 | 合计 | argc | strc |
| --- | --- | --- | --- | --- | --- |
| 220 | 8 | 2 | **10** | 恒 2 | 恒 0 |
| 221 | 16 | 21 | **37** | 恒 4 | 恒 0 |
| 222 | 5 | 0 | **5** | 恒 4 | 恒 0 |

52 个站点全部参数长度一致，没有例外——这比 101/150 那种「可选字段」式读法干净得多，实现可以按固定下标取值。

### 1.1 220 全部 10 个站点

| 位置 | 参数 | 备注 |
| --- | --- | --- |
| CE ev38 `--------------------` pc7 | `[0, 1600001]` | pc4 self1=0 → pc5 LoopTimes 32 → pc6 self1+=1 → pc7 保存 → pc10 EndGame(175)。32 次槽循环后直接退出程序【未验证】：磁盘只有 8 个 `SaveDataNN.sav`，超出槽数的写盘多半被引擎忽略 |
| CE ev131 `X┣[移]セーブ欄実行` pc233 | `[0, 1600012]` | **保存**分支：self12 = 目标槽号，pc230..232 先把三个变量 222 进 System.sav，再 220 写整档（§4） |
| CE ev131 pc268 | `[1, 1600012]` | **读取**分支：pc251 先 221 探存在性，pc267 等待 9 帧，再 220 读档 |
| CE ev223 `BGM補正` pc59 | `[0, 3000009]` | 字符串变量 3000009 = `"save/system.sav"` → 显式路径写盘 |
| CE ev426 `温泉イベント　討伐` pc326 | `[0, 3000010]` | 字符串变量 3000010 = `"save/system2.sav"` → 第二份系统档 |
| CE ev435 `オートセーブ` pc6 | `[0, 1600012]` | 自动存档（self12=1，见 §5.3） |
| CE ev436 `引き継ぎPチェック` pc96 | `[0, 3000023]` | 3000023 = `"save/system2.sav"` |
| CE ev436 pc105 | `[0, 3000015]` | 3000015 = `"Save/System.sav"` |
| 地图 SampleMapA ev35 `音量調整` pc37 | `[0, 3000010]` | 同上，`"save/system2.sav"` |
| 地图 SampleMapA ev35 pc42 | `[0, 3000009]` | 同上，`"save/system.sav"` |

### 1.2 221 全部 37 个站点

**公共事件 16 个**：ev48 pc209；ev94 pc254 / 289 / 335 / 345 / 348 / 349 / 352 / 353 / 381 / 415 / 427 / 453 / 503（共 13 个）；ev131 pc251；ev436 pc100。

**地图 21 个**：

| 位置 | 参数 |
| --- | --- |
| TitleMap ev2 `新タイトル用イベント` pc33 | `[2000503, 3000010, 2000503, 0]` |
| TitleMap ev2 pc35 | `[2000626, 3000023, 2000531, 0]` |
| TitleMap ev2 pc38 | `[2000357, 3000009, 2000357, 0]` |
| TitleMap ev2 pc52 | `[2000300, 3000008, 2000300, 0]` |
| TitleMap ev2 pc53..60 | `[2000080 .. 2000087, 3000008, 同址, 0]`（8 站） |
| TitleMap ev2 pc61..65 | `[2000324 .. 2000328, 3000008, 同址, 0]`（5 站） |
| TitleMap ev2 pc66 | `[2000546, 3000008, 2000546, 0]` |
| TitleMap ev2 pc67 | `[2000550, 3000008, 2000550, 0]` |
| SampleMapA ev3 `ロード後の処理` pc28 | `[2000550, 3000015, 2000550, 0]` |
| SampleMapA ev35 `音量調整` pc32 | `[2000503, 3000010, 2000503, 0]` |

（TitleMap 共 19 条：pc33 / pc35 / pc38 各一条，pc52..67 共 16 条；其中 3000008 组 14 条在表里按组归并，逐条清单见 `work/_p24i.py` 输出。）

### 1.3 222 全部 5 个站点

| 位置 | 参数 | 写入内容 |
| --- | --- | --- |
| ev131 pc230 | `[1600012, 1600005, 2000000, 0]` | 通常変数 0 ← self12（槽号） |
| ev131 pc231 | `[2000531, 1600005, 2000626, 0]` | 通常変数 626 ← 通常変数 531 |
| ev131 pc232 | `[1600012, 1600005, 2000546, 0]` | 通常変数 546 ← self12（槽号） |
| ev131 pc261 | `[2, 1600012, 2000302, 0]` | 通常変数 302 ← **字面量 2**（证明 arg0 走 magic 求值，不是「必须传变量号」） |
| ev435 pc5 | `[1600012, 1600005, 2000000, 0]` | 通常変数 0 ← self12（自动存档槽） |

---

## §2 220 SaveLoad 语义

### 2.1 参数布局【已验证】

| 下标 | 含义 |
| --- | --- |
| intArgs[0] | `mode`：0 = 保存，1 = 读取。§1.1 的 10 个站点里 0 出现 7 次、1 出现 3 次 |
| intArgs[1] | 目标文件，magic 引用（求值后看**值类型**）：<br>• 字符串变量 → **显式路径**（`"Save/System.sav"` / `"save/system.sav"` / `"save/system2.sav"`）<br>• 数值变量 → **槽号**（1 起） |

### 2.2 证据

1. **0=保存 / 1=读取**：ev131 同一段里 pc233 处于「写完后重绘菜单」的保存分支，pc268 处于「先 probe 存在性、等 9 帧、读档、再做全场特效」的读取分支（`work/_p24j.py` 的上下文 dump）。
2. **字符串 = 路径**：ev436 pc99 `122 [3000015] = "save/system.sav"` → pc100 `221 [2000550, 3000015, 2000550, 0]` → pc104 `122 [3000015] = "Save/System.sav"` → pc105 `220 [0, 3000015]`。同一个变量先当读源再当写目标，路径大小写都出现过（Windows 上等价）。
3. **数值 = 槽号**：ev131 pc206..208 把 self12 算成 1..8（`121 [1600012, 1600027, 1600011, 8192]`、`+= 1600010`、`+= 1`），随后 220 用 self12 落盘；磁盘上正好有 `SaveData01..08.sav` 八个文件。

### 2.3 槽号 → 文件名的映射【未验证】

推断为 `Save/SaveData%02d.sav`（8 个文件、8 个槽、自动存档槽 = 1）。引擎侧没有指令显式拼这个串，游戏代码从头到尾只用槽号，所以文件名规则属于引擎内部约定。

### 2.4 阻塞与耗时【未验证】

ev131 在 220 前有 `180 [16]`（等 16 帧）、后紧跟 `210 [500436]` 与 `300 'メニュー描画'`，读档分支前有 `180 [9]`。看起来是「画面淡出 → 写盘 → 重新布局」，但 220 本身是否阻塞解释器无法从字节码判定。实现时按**同步且立即完成**处理（本项目 Host 是命令式），并在接口上保留「异步完成」的位置。

---

## §3 221 LoadGame 语义

### 3.1 参数布局【已验证】

| 下标 | 含义 |
| --- | --- |
| intArgs[0] | 目标变量（运行中的变量），magic 求值 |
| intArgs[1] | 文件，magic 求值（字符串=路径，数值=槽号，同 §2.1） |
| intArgs[2] | 存档文件内的变量地址，magic 原样编码（2000000 段 / 9000000 段 / 1600000 段都可） |
| intArgs[3] | `flag`，本作只出现 0 和 1 |

### 3.2 三条铁证

1. **ev48 pc206 注释原文**：`最新セーブデータ番号をSystem.savから取得　（System.savの通常変数0番を最終セーブデータ番号として使用）`，紧跟 pc208 `122 [1600005] = 'Save/System.sav'`、pc209 `221 [1600020, 1600005, 2000000, 0]`、pc210 `121 [9000025] = self20`（Sys25 = 現在セーブデータ番号）。语义与参数顺序完全对上。
2. **自反复制**：TitleMap ev2 的 15 个站点形如 `[2000080, 3000008, 2000080, 0]`——目标与档内地址**是同一个变量号**，含义就是「把存档里的 2000080 读回运行时的 2000080」。这是判断「arg0 与 arg2 分别是两侧地址」最干净的证据。
3. **跨 bank**：ev94 pc289 `[1600060, 1600050, 9000029, 0]` 读的是档内 System 变量 29（プレイ時間 秒），落到 self60；pc349/352 读 2000160；pc353 读 2000006。⇒ arg2 的 magic 编码与运行时同一套，不做「只允许某段」的限制。

### 3.3 存档菜单读档流程（ev94 + ev131）

`text
ev131 pc251  221 [1600013, <slot>, 9000024, 0]   读档内 Sys24（存在判定）
ev131 pc252  111 判断 0 / 1
ev131 pc259  注释「セーブデータが存在しているときだけ処理」
ev131 pc260  150 清窗口
ev131 pc261  222 [2, 'Save/System.sav', 2000302, 0]   档内打标记
ev131 pc262..266  重绘菜单 / SE / 特效
ev131 pc267  180 [9]                             等 9 帧
ev131 pc268  220 [1, <slot>]                     真正读档
`

### 3.4 flag 的 4 个 flag=1 站点【未验证】

37 个站点里只有 `ev94 pc335 [1600011, 1600050, 1600062, 1]`、`pc345 [1600006, 1600050, 1600064, 1]`、`pc348 [1600007, 1600050, 1600064, 1]` 三条把 flag 置 1（其余 34 条全是 0），且地址固定落在 1600062 / 1600064（第 6 群），dest 落在 1600006 / 1600007 / 1600011。合理猜测是「按记录基址 + 子字段偏移读取」或「读整块」，但**没有任何一条站点能区分这两种解释**，实现上先当 0 处理并在文档里留钩子（见 §8 U2）。

---

## §4 222 SaveGame 语义

### 4.1 参数布局【已验证】

| 下标 | 含义 |
| --- | --- |
| intArgs[0] | 值来源，magic 求值（变量或字面量；pc261 传字面量 2） |
| intArgs[1] | 文件，magic 求值（字符串=路径，数值=槽号） |
| intArgs[2] | 存档文件内的目标变量地址（magic） |
| intArgs[3] | `flag`，本作 5 个站点全是 0 |

### 4.2 ev131 的完整保存序列（`work/_p24j.py`）

`text
pc223  1000 [3] 'Save/save\cself[12].jpg'     ProFeature type3 = 删旧缩略图
pc225  1000 [0] 'Save/save\cself[12].jpg'     ProFeature type0 = 截屏存 JPG（新缩略图）
pc227  210 [500436]                           转调公共事件 436（写 system2.sav）
pc228  122 [1600005] = 'Save/System.sav'      文件名
pc229  121 [2000546] = self12                 通常変数 546 ← 槽号
pc230  222 [self12, 1600005, 2000000, 0]      通常変数 0   ← 槽号
pc231  222 [2000531, 1600005, 2000626, 0]     通常変数 626 ← 通常変数 531
pc232  222 [self12, 1600005, 2000546, 0]      通常変数 546 ← 槽号（与 229 同值，双保险）
pc233  220 [0, 1600012]                       写整档
pc234  111 [1, 1600001, 0, 2]                 保存是否成功的判定分支
`

⇒ 存档 = **三份文件**：`SaveDataNN.sav`（整局状态）+ `Save/saveN.jpg`（缩略图）+ `System.sav`（记录「最后保存的是哪一槽 / 有存档的槽位标记」）。

---

## §5 存档界面是谁画的

### 5.1 ev94 `X[移]セーブ·ロード画面描画`【已验证】

| 指标 | 值 |
| --- | --- |
| 指令数 | 698 |
| 150 Picture | **77** |
| 101 Message | **0** |
| 102 Choices | **0** |
| 221 | 13 |
| run_condition | call_only（0） |

⇒ 存档界面**没有一个字是用 101 打的**：窗口、标题、槽位、说明文字全部是 150 的 type=2（窗口内文本）与 type=0（图片文件）拼的。典型的槽位文字是 `150 [4128, 1600021, 0, 1, 1, 1, 255, 1600030, 1600031, 100, 0] strs=['\f[\cself[19]]\cself[6]']`（a0=0x1020 → type 0/2 家族、锚点 1 = 中心），文字内容靠 `\cself[18]/[19]/[6]` 从调用方セルフ取。自动存档那一栏在 pc237：字符串直接写死 `"\f[\cself[18]]\cself[6]自动存档"`。

DLC 徽标共 10 张：`DLC_data/save_SD_clear.png / save_SD_dlc.png / save_SD_boss.png / save_SD_taikai.png / save_SD_gassyuku.png / save_SD_start2.png / save_SD_tensai.png / save_SD_cook.png`（pc418..456 各一条 150，图片号 12501 等）。

### 5.2 ev131 `X┣[移]セーブ欄実行`【已验证】

338 条指令：150 × 10、1000 × 2、222 × 4、220 × 2、221 × 1。职责 = 对**一个槽**执行「保存 / 读取 / 删除 / 存在性判定」。

### 5.3 ev435 `オートセーブ`【已验证】

全部 8 条：

`text
pc0  180 [1]                             等 1 帧
pc1  1000 [0] 'save/save1.jpg'           截屏存缩略图（槽 1）
pc2  180 [1]
pc3  121 [1600012] = 1                   槽号 = 1
pc4  122 [1600005] = 'Save/System.sav'
pc5  222 [1600012, 1600005, 2000000, 0]  通常変数 0 ← 1
pc6  220 [0, 1600012]                    写整档
`

⇒ 自动存档**覆盖槽 1**，这与 ev94 pc237 把第一栏标成「自动存档」一致。

### 5.4 缩略图【已验证】

- 写：`1000 [0] 'Save/save\cself[12].jpg'`（ProFeature type 0 = Screenshot JPG，见 `docs/OPCODE_SPEC.md` ProFeature 节）
- 删：`1000 [3] 'Save/save\cself[12].jpg'`（type 3 = FileDelete），保存前先删旧图
- 磁盘实证：`Save/save1..8.jpg` 全部是 **1024×576 baseline JPEG（SOF0）**，108,167 / 108,541 / 116,511 / 115,716 / 116,489 / 114,518 / 118,079 / 123,108 字节

### 5.5 标题画面与其它入口

| 事件 | 作用 |
| --- | --- |
| ev42 `オリジナルタイトル画面Ａ`（135 指令、150×14、102×1） | 标题菜单 |
| TitleMap ev2 `新タイトル用イベント`（19 个 221） | 标题画面开「继续游戏」时把 System.sav / system2.sav 里的槽位数据读回运行变量 |
| SampleMapA ev3 `ロード後の処理` / ev35 `音量調整` | 读档后处理、音量存盘 |

---

## §6 引擎存档时必须提供哪些数据

按 `docs/SAVE_SPEC.md` 的段布局逐项对照。长度取 `SaveData01.sav` / `System.sav` 实测值：

| 段 | 长度（01 / System） | 内容 | 谁在游戏侧驱动 | 写出时必须重算 |
| --- | --- | --- | --- | --- |
| 明文头 0..0x14 | 20 | 3 个 PRNG 种子（off 0/3/9）、校验和（off 2）、UTF-8 标记（off 6） | 引擎 | **全部**：种子每次随机，校验和 = 正文逐字节和 |
| 0x14 | 1 | `0x19` 起始标记 | 引擎 | 固定值 |
| 名字 | 2+43 | 长度（含 NUL）+ UTF-8 标题 `妹相随～黑白世界的缤纷冒险～` | 引擎（Game.dat） | 长度字段 |
| 版本 | 2 | `0x8C` | 引擎 | 保持 |
| SavePart1 | 34318 / 34318 | 地图快照：tiles 6912×u32、chip 图名、31 个自动图块名、vars2_new 192×u32、40 个地图事件对象 | 引擎（地图状态） | 整段 |
| SavePart2 | 595 / 617 | 全局设置：BGM 路径与音量、远景、字体、时间戳 var71=982000 | 引擎 | 整段 |
| SavePart3 | 218610 / 218650 | **最大段**：500（或 550）组 ×100 个 u32 = 50,000 个セルフ数値 + 500 组 ×5 个セルフ文字列 + 9 组 row_words 835 + 26 条 mds1 字符串 | 各公共事件的セルフ変数 | 整段 |
| SavePart4 | 1213 / 1213 | 对象表续（lead 为 SavePart1_1，var1=0xFFFFFFFE、var2=62） | 引擎 | 整段 |
| SavePart5 | 29489 / 37497 | **ピクチャ状态表**：84 条，图片号（4100/4200/…/6232）、路径（`Picture/scene0102_bless01.png`、`cutin/cutin_211.png`…）、x/y/层/透明度/颜色 | 150 Picture 的运行时状态 | 整段 |
| VariableDatabase | 81851 / 81851 | 可変DB 30 个类型的字段配置 + 全部行（81851 或 81842 字节两种） | 250 Database 的写操作 | 整段 |
| SavePart7 | 1 / 1 | 空表（gate=0） | 引擎 | 固定 1 字节 `0x00` |
| 末字节 | 1 | `0x19` | 引擎 | 固定值 |

**结论**：一条 Rust 侧「存档」要做的事 = ①按当前 VM 状态重新生成上面 7 段字节；②按顺序拼接；③重算头部（名字 / 版本 / 种子 / 校验和）；④XOR 混淆写盘。①里只有 SavePart3/5 与 VariableDatabase 有明确的语义映射（セルフ変数 / ピクチャ / DB），Part1/2/4 目前只知道长度和字段切分。

### 6.1 SavePart3 的语义定位（**已验证**：组号 = 公共事件 id、槽 = セルフ変数号）

`list_words` 的读法是「var5 个组，每组 1 字节计数 + 计数个 u32」，实测 `list_counts` 恒为 100 ⇒ 500×100 = 50,000 个槽；`slot_counts` 恒为 5 ⇒ 2,500 个字符串。把它读成「组号 = 公共事件 id、槽 = セルフ変数号」的五条证据：

1. g131（事件 131 = 存档执行器）s0..s4 的字符串是 `"Save/System.sav"`，正是 ev131 pc228 `122 [1600005] = 'Save/System.sav'` 写的那个文件名。
2. g131 的 s12 = **8**（三个文件都一样），与 ev131 里 self12「保存先スロット番号」同址。
3. g48（ev48）s16 = **6**，对应 ev48 pc122 `250 [18,51,0,69632,1600016]` 读进来的「敵味方最大数 = 6」。
4. g48 s25 = **12**、s26 = **24**：ev48 pc30..31 令 s26 = Sys117/24（= rateY = 24），pc47..48 令 s25 = s26×5/10（= 12）。两个值都是 ev48 自己算出来又存进セルフ的，落盘一致。
5. g94 s60 = 14661 / 16536 / 16182 … 跨 8 个档各不相同，正是 ev94 pc289 `221 [1600060, …, 9000029, 0]` 读进来的「プレイ時間（秒）」。

同时 `var5` 有两种值：`SaveData01..07 / System = 500`，`SaveData08 = 550`（`slot_counts` 同步 500/550）。550 恰好等于本作公共事件总数，是「组数 = 事件数」的旁证；500 的更老档则说明这份表是按「写档时的公共事件数」增长的【未验证】。

### 6.2 早先的误判：通常変数「找不到落盘区」【已推翻 —— 见 §6.3】

222/221 的地址明确写着 2000000 段（通常変数）与 9000000 段（システム変数），但**逐段排查后，文件里没有第二块能装下 5000 个通常変数的区域**：

| 候选段 | 容量 | 结论 |
| --- | --- | --- |
| SavePart3.list_words | 50,000 u32 | 太大且已被「セルフ変数」占用（§6.1） |
| SavePart3.group_words | 50 u32 | 太少 |
| SavePart3.row_words | 835 u32 | 太少 |
| SavePart1.vars2_new | 192 u32 | 太少 |
| SavePart2 | 无数组 | 全是设置项 |
| 段间空隙 | **0 字节**（9 个档的段首尾全部相接） | 不存在隐藏区 |

**反证**：ev131 用 222 往 System.sav 写了 `2000000 / 2000546 / 2000626` 三个地址，可是 9 个样本存档里 `list_words[0] / [531] / [546] / [626]` 全是 0（`work/_p24l.py`、`crates/wolf-save/tests/_tmp_probe.rs` 实测）。两种可能：(i) 地址映射不是「group=n/100、slot=n%100」；(ii) 这份表根本不是通常変数。本作数据无法区分——写出一度因此只做整段替换（§7）。**该结论已于 §6.3 推翻**：漏查了 `row_words`，通常変数就在那里。

---

### 6.3 已解决：通常変数就是 SavePart3 的 row 0（**已验证**，9/9）

§6.2 的排查漏了一层表：`m_vars4`/`m_vars5`（`row_counts` / `row_words`）才是通常変数，
`m_vars5`/`m_vars6`（`list_counts` / `list_words`）是セルフ変数。`row_counts` 在 9 个档里恒为
`[800, 4, 7, 4, 1, 1, 1, 6, 1, 10]`，**row 0 的 800 个词 = 通常変数 2000000..2000799**。

三条独立证据（逐档数字见 `docs/SAVE_SPEC.md` §6.1）：

1. **原作注释对上落盘值**：ev48（`X[共]基本システム自動初期化`）pc206 注释原文
   「System.savの通常変数0番を最終セーブデータ番号」，而 `System.sav` 的 `row_words[0]` 实测 = **8**，
   正好是最后使用的存档槽（`SaveData08.sav` 存在、ev131 的 CSelf12 也是 8）。
2. **所持金对上 HUD**：`row_words[7]`（= 通常変数 2000007）逐档为
   7586 / 2254 / 8231 / 1293 / 1193 / 1503 / 15453 / 7586 / 7586，与各档 HUD 所持金量级一致，
   且 `SaveData01` / `SaveData08` / `System.sav` 三者同为 7586（同一存档线继承）。
3. **写入者闭环**：ev131 的 222 站点 pc230 `[1600012, 1600005, 2000000, 0]` 把 CSelf12（槽号）
   写进 `Save/System.sav` 的通常変数 0 —— 落盘 8 = CSelf12 = 8。

**同时解释了 §6.2 的反证**：ev131 pc231 是 `222 [2000531, 1600005, 2000626, 0]`（把 531 抄进 626），
磁盘上 531 = 2 而 626 = 0。若 222 的源操作数在文件里求值，626 就该是 2；所以在 RAM 里求值、磁盘值是
旧值 —— 这条推断仍是【未验证】（要实机存档对比才能证实）。

**システム変数（9000000 段）仍然没有落盘区**：row 0 只有 800 词，其余 9 行合计 35 词，9 个档里没有
任何一块的规模或取值像「150 个系统变量」。合理猜测是**根本不必存**——载入时 ev48 会按
`Sys115/116/117` 把 Sys1..Sys10、Sys23 等全部重算一遍（ev48 pc26..50 就是这段）【未验证】。

### 6.4 由此得到的写出范围

| 能改 | 落点 | API |
| --- | --- | --- |
| 通常変数 2000000+n（n < 800） | SavePart3 row 0 | `SavePart3::set_normal_var` / `SaveFile::set_normal_var` |
| セルフ数値 CSelf0..99 | SavePart3 list_words（组 = 公共事件 id） | `set_self_var` |
| セルフ文字列 CSelf5..9 | SavePart3 slot_strings（串号 0 = CSelf5） | `set_self_string` |
| 游戏时间 | SavePart2 `vars3_21[0]`（段内偏移 2） | `SaveFile::set_playtime_frames` |
| システム変数 | **无落盘区** | 不支持（载入时会重算） |
---

## §7 wolf-save 的写出路径

### 7.1 API【已验证】

```rust
/// 混淆 / 解混淆是同一个三步 XOR（入射对合），头 0x14 字节不参与。
pub fn encrypt(data: &[u8]) -> Result<Vec<u8>>;

impl SaveFile {
    /// 用「头部 + 当前 7 段」重建**明文**文件；名字/版本/校验和重算。
    pub fn serialize(&self) -> Result<Vec<u8>>;
    /// serialize 之后再混淆：可以原样写回磁盘的字节。
    pub fn to_bytes(&self) -> Result<Vec<u8>>;
    /// 整段替换（id 0..=6），后续段偏移自动平移，校验和重算。
    pub fn replace_part(&mut self, id: u32, bytes: &[u8]) -> Result<()>;    /// 用 SavePart3 的 typed 值重新编码整段并替换（视图同步刷新）。
    pub fn set_part3(&mut self, part: &SavePart3) -> Result<()>;
    /// 用 typed 值重新编码**全档**（part1..part5；vardb/part7 仍复制原始字节）。
    pub fn rebuild(&self) -> Result<Vec<u8>>;
    /// rebuild 之后再混淆：opcode 220 mode 0 可直接写盘的字节。
    pub fn to_bytes_rebuilt(&self) -> Result<Vec<u8>>;
    /// 其余四个段的 typed 重编码 + 替换（与 set_part3 同构）。
    pub fn set_part1(&mut self, part: &SavePart1) -> Result<()>;
    pub fn set_part2(&mut self, part: &SavePart2) -> Result<()>;
    pub fn set_part4(&mut self, part: &SavePart4) -> Result<()>;
    pub fn set_part5(&mut self, part: &SavePart5) -> Result<()>;
    /// part2 的 vars3_21[0]，段内偏移 2；单位是帧，60fps。
    pub fn playtime_frames(&self) -> Option<u32>;
    pub fn set_playtime_frames(&mut self, frames: u32) -> Result<()>;
    /// 通常変数 2000000+n（n < 800）与セルフ数値 / セルフ文字列（slot 0 = CSelf5）。
    pub fn normal_var(&self, index: usize) -> Option<i32>;
    pub fn set_normal_var(&mut self, index: usize, value: i32) -> Result<bool>;
    pub fn self_var(&self, event: u32, slot: usize) -> Option<i32>;
    pub fn self_string(&self, event: u32, slot: usize) -> Option<&str>;
}
```

### 7.2 不变式

1. `encrypt` 与 `decrypt` 是同一个函数（XOR 密钥流只依赖地址，不依赖数据），所以 `encrypt(decrypt(x)) == x` 对任意长度成立。
2. `serialize()` 的输出对**任何**合法存档都应逐字节等于 `decrypted()`；不等就是头部重建有误。
3. `to_bytes()` 对 9 个真实存档逐字节等于原始文件（下面的测试就是这条）。
4. `replace_part` 只改内存里的字节与 chunk 偏移，不碰磁盘；被替换段的类型化视图（`part1..part7`）**不会**自动刷新，要用 `SaveFile::parse` 重新解析。
5. `replace_part` 拒绝不存在的 id，且被拒绝时不改任何字节。

### 7.3 测试（`crates/wolf-save/tests/write.rs`，6 个）

| 测试名 | 断言的关键数字 |
| --- | --- |
| `the_cipher_wraps_any_buffer` | 200 字节合成缓冲：头 0x14 不变、正文变化、`decrypt(encrypt(x)) == x` |
| `every_save_round_trips_byte_for_byte` | 9 个存档：`serialize() == decrypted()`、`to_bytes() == 原文件`（长度与字节全等）、重解析后 `checksum_ok()` 且末字节 0x19 |
| `renaming_rebuilds_the_header_and_shifts_the_body` | 名字改成 `"test"`：文件 366146 → **366108**（-38），正文整体前移 38，part3 字节不变，part5 事件数不变 |
| `growing_a_middle_segment_shifts_the_later_offsets` | part3 加 10 字节：part4/part7 偏移各 +10，文件 +10，编辑点之前字节完全相同、之后整体平移 10，头部校验和跟新正文一致 |
| `a_replaced_trailing_segment_parses_back` | part7 换成 `[1,1,0,0,0,0x05,0x2A]`：文件 +6、重解析得 1 条 entry（tag 5、payload 0x2A）、校验和 ok |
| `an_unknown_segment_id_is_rejected` | id 7 与 `u32::MAX` 都报错，且 `serialize()` 仍等于 `decrypted()` |

### 7.4 与游戏侧接线的建议顺序

1. **先只做 round-trip**：读档 → 存回去（等价于「原样另存」），验证引擎能加载我们写出的文件（等价于永远不会弄坏用户的档）。
2. 再接 §6.1 的 SavePart3 セルフ変数表（组=事件 id、槽=セルフ号），这是唯一语义已定位的大表。
3. SavePart5（ピクチャ）语义也基本清楚（图片号 / 路径 / x / y / 透明度 / 颜色），可做第二个。
4. SavePart1/2/4 与 VariableDatabase 先原样搬运（我们目前不产生这些状态）。

### 7.5 变量表 API 的测试（`crates/wolf-save/tests/typed.rs`，6 个）

| 测试名 | 断言的关键数字 |
| --- | --- |
| `every_part3_re_encodes_byte_for_byte` | 9 档 `part3.to_bytes()` 与 `chunk_bytes(2)` **逐字节相同**（218610 / 217708 / 218472 / 218466 / 239821 / 218462 / 218574 / 240000 / 218650） |
| `the_tables_hold_the_numbers_the_game_ships` | 逐档断言 row_counts、2000007 所持金、playtime、组数 500/550、ev131 CSelf12、part3 长度；恒定值 ev48 CSelf16/25/26 = 6/12/24、ev131 CSelf5 = `Save/System.sav` |
| `editing_a_normal_variable_survives_a_write` | 2000007 7586 → 123456：写出再解析读回、段长不变、playtime 与セルフ值不变、`checksum_ok()` 为真、part3 仍逐字节可逆 |
| `editing_self_state_survives_a_write` | ev48 CSelf25 12 → 99、ev131 CSelf5 → `Save/Other.sav`，写出再解析读回，相邻槽（CSelf26 = 24、串 1 = 空、CSelf12 = 5）不动 |
| `playtime_round_trips_through_the_header` | 16705 → 4321 只改 part2 的偏移 **2、3** 两个字节（4321 = 0x10E1），part3 一字节不动，文件总长不变 |
| `out_of_range_indexes_are_refused` | 2000799 有值、2000800 为 `None`；CSelf99 有值、CSelf100 为 `None`；组号越界为 `None`；串槽 5 为 `None`；setter 返回 `false` 且不改字节 |
### 7.6 全文件 rebuild 的测试（`crates/wolf-save/tests/rebuild.rs`，4 个，task-28）

`serialize()` / `to_bytes()` 复制原始段字节，只重建头部；`rebuild()` / `to_bytes_rebuilt()`
用逐段编码器（SavePart1/2/3/4/5）重新编码整档，vardb 与 part7 仍复制。
两者的详细分工与编码器清单见 `docs/SAVE_SPEC.md` §11.2 与 §14。

| 测试名 | 断言 |
| --- | --- |
| `every_retail_save_rebuilds_to_the_same_bytes` | 9 个零售存档 `parse -> rebuild -> encrypt` 与原始文件**逐字节相同** |
| `rebuild_matches_serialize_and_to_bytes` | `serialize() == rebuild()`，且 `to_bytes()` 等于原文件 |
| `each_segment_encoder_reproduces_its_parsed_bytes` | 9 档 × 5 段，每个 `to_bytes()` 等于对应 `chunk_bytes(id)` |
| `mutations_in_part2_and_part5_survive_a_rebuild` | 改 part2 playtime 与 part5 的 `vars7_18[0]` 后写出再解析可读回，改回原值则输出与原文件逐字节相同 |

---

## §8 未验证清单

| 编号 | 未验证项 | 现状 | 怎么证 |
| --- | --- | --- | --- |
| U1 | 通常変数（2000000 段）的落盘位置 | **已解决（§6.3）**：= SavePart3 row 0 的 800 个词，`System.sav` row0[0] = 8 = 最后使用槽；システム変数（9000000 段）仍未找到，疑为载入时重算 | —（システム変数部分仍需实机存一次档对比） |
| U2 | 221 的 `flag=1` 语义（3 个站点，地址固定 1600062/1600064，dest 固定 1600006/1600007/1600011） | 只知「与记录基址/子字段有关」 | 找到 flag=1 站点的上下文阅读（ev94 pc330..350 的注释已被读空） |
| U3 | 220 是否阻塞、是否自己写缩略图 | 220 前后有 180 等待，但 1000 已单独截屏 | 引擎行为观察 |
| U4 | 槽号 → `Save/SaveData%02d.sav` 的映射 | 8 槽 8 文件，推断一致 | 引擎行为观察 |
| U5 | セルフ文字列：`122 [1600005]` 写到的是槽 5，落盘却在组内索引 0 | §6.1 证据 1 就是这个现象 | 再找一个「写 self 字符串 0..4」的事件核对索引 |
| U6 | part3 `var5` 为何 500/550 两种 | 550 = 公共事件总数（旁证） | 老版本 CommonEvent.dat 事件数 |
| U7 | `serialize` / `rebuild` 对「非本作」存档是否也逐字节可逆 | 只验证了本作 9 个档（`rebuild` 的逐段编码器现已 9/9 逐字节相同，见 SAVE_SPEC §14） | 其它 WOLF 游戏的存档 |
| U8 | 存档界面 77 条 150 的完整槽位布局（像素坐标） | 只解到「a0=0x1020 家族 + \cself 文本」 | 逐条解 150 参数（属 OPCODE 150 文档范围） |

---

## §9 复现脚本与事件索引

| 脚本 | 用途 |
| --- | --- |
| `work/_p24h.py` | 打印 220/221/222 相关事件的标题、指令数、各 opcode 计数，以及 ev94 的全部 150/221 站点 |
| `work/_p24i.py` | 220/221/222 的 CE/地图分栏普查、argc/strc 直方图、全部站点列出 |
| `work/_p24j.py` | 指定变量的写者、ev131 的 220/222/1000 上下文、ev38/ev223/ev426/ev435/ev436 的 220 上下文 |
| `work/_p24l.py` | ev48/ev94/ev131/ev436 的 221 站点全表 |
| `work/_p24k.py` | `Save/save1..8.jpg` 的 JPEG 尺寸 |

| 事件 | id | 角色 |
| --- | --- | --- |
| `X[移]セーブ·ロード画面描画` | 94 | 存档界面绘制（77×150） |
| `X┣[移]セーブ欄実行` | 131 | 单槽保存 / 读取 / 判定 |
| `オートセーブ` | 435 | 自动存档（槽 1） |
| `引き継ぎPチェック` | 436 | 二周目继承数据 → system.sav / system2.sav |
| `X[共]基本システム自動初期化` | 48 | 开机从 System.sav 读「最终存档号」等 |
| `オリジナルタイトル画面Ａ` | 42 | 标题菜单 |
| `--------------------` | 38 | 32 次槽循环存盘后 EndGame（用途未验证） |
| `BGM補正` / `温泉イベント　討伐` | 223 / 426 | 用显式路径存 system.sav / system2.sav |
| TitleMap ev2 `新タイトル用イベント` | 地图事件 | 19 个 221，标题画面读槽数据 |

---

## §10 与 `docs/SAVE_SPEC.md` 的出入（§6.1 / §6.2 / §11.1 / §13 已同步写入 SAVE_SPEC）

1. SAVE_SPEC §5 把 SavePart3 描述成「地图与事件运行时表」，现在可以补上 §6.1 的定位：`list_words` = 500（或 550）组 ×100 个セルフ数値、`slot_strings` = 同一批组 ×5 个セルフ文字列。
2. ~~SAVE_SPEC 未记录「通常変数 / システム変数 找不到落盘区」~~ —— **已写并已改正**：SAVE_SPEC §6.1 记录「通常変数 = row 0」的三条证据与逐档表值，§13 只保留「システム変数没有落盘区」这一条真缺口。
3. `docs/FORMAT_SPEC.md` §9.3 的「chunk 序列」措辞仍与实现不符（SAVE_SPEC §1.3 已纠正），建议 Lead 统一。

---

## §11 引擎侧落地记录（Lead 追加，2026-xx）

### 11.1 解释器：`SaveOp` 通道（已实现）

`crates/wolf-vm/src/exec.rs` 新增：

```rust
pub enum SaveTarget { Slot(i32), Path(String) }
pub struct SaveOp {
    pub opcode: u8,        // 220 / 221 / 222
    pub mode: i32,         // 220: 0 = save, 1 = load; unused by 221/222
    pub file: SaveTarget,  // numeric operand = slot, string variable = path
    pub destination: i32,  // 221: raw arg0, receives the answer
    pub address: i32,      // 221/222: raw arg2, address inside the save
    pub value: i32,        // 222: evaluated arg0
    pub flag: i32,         // evaluated arg3 (0 when argc < 4)
    pub owner: u32,        // the calling event id, for self-variable addressing
}
```

`Host::save_load(&SaveOp) -> Option<i32>` is the only host hook; `221` stores the returned `i32` into `destination`, `220`/`222` ignore it. File operands are resolved by magic kind: `MagicKind::StrVar` evaluates to `SaveTarget::Path`, every other kind evaluates to `SaveTarget::Slot`. `address` and `destination` stay raw opcodes on purpose - they are addresses, not values.

### 11.2 游戏层：`GameHost::save_load`（已实现）

- Slot → `root/Save/SaveData{slot:02}.sav`; path → relative to the game root unless absolute.
- 220 `mode=1` queues `SaveRequest::Load(path)`; `Game::frame` applies it after `vm.step_frame` with `savegame::load_into`, which clears the picture screen and rebuilds it from part5, restores 800 normal variables, playtime and Sys3/Sys4.
- 220 `mode=0` queues `SaveRequest::Save(path)`; `Game::frame` calls `Game::save_to`, which clones the save the game was loaded from (or `Save/System.sav` for a new game) as the map template, copies play time, Sys3/Sys4, the 800 normal variables and every live common-event self variable / self string into `SavePart2`/`SavePart3`, then writes `to_bytes_rebuilt()`.
- **变量库已经可以写了（task-30 + 已接线）**：`SaveFile::set_vardb(&VariableDatabase)` 会调用 `VariableDatabase::to_bytes()` 重编码第六段，
  按格写有 `set_vardb_num` / `set_vardb_string` / `set_vardb_text`（越界或字段类型不符时返回 `Ok(false)` 且不动任何字节）。
  `SaveFile::rebuild()` 现在编码 vardb 而不是复制模板字节；`Game::save_to` 在 `set_part5` 之后调用
  `let (written, missed) = self.databases.write_into(&mut save.vardb);`，`missed > 0` 时写一条 `220 SaveLoad` 诊断（不静默丢格）。
  因此 opcode 250 对 CDataBase 的运行时写入会随整档保存落盘。详见 SAVE_SPEC §14.4 / §14.5。
- **画面图片表也会重建（已接线）**：`savegame::part5_from_screen(&self.screen)` 把当前活着的每张 `150` 图片
  反向编码成 `SavePart5_1`（`picture_to_record`），文本图用 `encode_picture_text` 把 `\E`/`[n]`/`\c[n]` 写回转义流。
  读入侧 `picture_from_record` 是它的逆过程；`a_saved_game_reloads_its_live_picture_layer` 逐字段比较 66 张图。
  唯一语义等价但非字节等价的是 retail 的 04（带字距字体）记录：读成 `[n]`，写回成 05。
- 221 reads the file, maps `address` through `save_address` (`2_000_000+n` normal, `1_600_000+n` self, `9_000_000+n` system) and returns `Some(0)` when the file does not exist (the script uses that as "slot empty").
- 222 reads, applies `write_save_address` (`set_normal_var`, `set_self_var`, `set_playtime_frames`, `vars3_21[n+6]`) and writes the file back.
- `flag=1` remains treated as 0 (U2 unchanged).

### 11.3 测试

| 层 | 测试 | 断言 |
| --- | --- | --- |
| wolf-vm | `save_ops_decode_slot_paths_and_store_the_221_answer` | 221 的字符串变量 → Path、数值 → Slot(0)、destination/address/owner、222 value 已求值、220 mode=1、221 返回值写回目标变量 |
| wolf-game | `a_save_address_finds_normal_self_and_system_variables` | 零售 SaveData01：2000007=7586、2000008=25、ev131 self12=8、9000003=512、90000029=playtime/60 |
| wolf-game | `a_save_write_reaches_the_next_read` | 222 写 2000007=1234 后 221 读回 1234；写 playtime=277 后 `playtime_frames()==16620`；越界地址返回 false |
| wolf-game | `a_save_file_round_trips_through_the_writer` | `write_save` 后 `read_save` 的 `to_bytes()` 与原件逐字节相等 |
| wolf-game | `a_save_written_from_a_retail_template_round_trips` | 用 SaveData01 当模板写盘，normal var 7 / playtime / Sys3 / Sys4 都能读回 |
| wolf-game | `a_saved_game_reloads_its_live_picture_layer` | 载入 SaveData01 → `Game::save_to` → 重新载入，66 张图的 number/kind/file/x/y/anchor/scale/split/pattern 全部相同 |
| wolf-game | `a_saved_game_keeps_the_database_cells` | CData (0,12,5)=4242 → 保存 → 重新载入仍读回 4242（`write_into` 接线） |
| wolf-save | `a_mutated_vardb_number_survives_a_full_rebuild` / `a_mutated_vardb_text_survives_a_full_rebuild` | 改变量库任一格后 `to_bytes_rebuilt()` 再解析读回新值、`checksum_ok()`；改回原值后与原文件逐字节相同（task-30） |
| wolf-save | `each_segment_encoder_reproduces_its_parsed_bytes` | 9 档 × 6 段（含 VariableDatabase 81842 / 81851 字节）逐段与 `chunk_bytes(id)` 相同 |

复现：`cargo test -q -p wolf-vm`（81）、`cargo test -q -p wolf-game`（187 + 3 + 1）、`cargo test -q -p wolf-save`（21）。
