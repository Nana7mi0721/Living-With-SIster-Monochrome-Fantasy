## 6. 数据库 DataBase.dat / CDataBase.dat / SysDatabase.dat

**状态：容器与自描述结构 已验证**（work\fs_db.py / work\fs_db_props.py；
DataBase.dat 94967 B / 40 类型、CDataBase.dat 81832 B / 30 类型、SysDatabase.dat 11607 B / 28 类型；
三者 footer 均为 0xC2、trailing 0）。**字段的中文含义来自同目录的 .project 文件（部分未验证，标 TODO）。**

### 6.1 容器布局（已验证）

| 偏移 | 类型 | 字段 | 实测 |
|---|---|---|---|
| 0 | 6 字节 | magic 00 57 00 00 4F 4C | |
| 6 | u1 | version_header | 0x55 |
| 7 | 3 字节 | magic2 46 4D 00 ("FM\0") | |
| 10 | u1 | version | 0xC2（v3.0 明文）；0xC4 为 v3.3 且正文是打包/压缩的 |
| 11 | u4 | type_count | DataBase 40 / CDataBase 30 / SysDatabase 28 |
| 15 | type_data × type_count | | 见 6.2 |
| 末尾 | u1 | footer | 0xC2 |

> 等价写法：整个头部可看作 9 字节 magic = 00 57 00 00 4F 4C 55 46 4D，随后 u1 version、u4 count。
> 0xC4 分支：WolfTL 在 version == 0xC4 时用 buffer coder 写正文并在末尾 Pack()，然后整个正文被压缩。
> 本游戏三个库都是 0xC2（明文），不需要实现该分支。

### 6.2 自描述结构：type → field 定义 → data 行（已验证）

~~~
type_data:
  u1[4]  sub_header          // 恒 FE FF FF FF
  u4     data_id_method      // 0 手动 / 1 与首字段同 / 2 与前类型同 / 10000 由类型推
  u4     property_count
  u4[ property_count ]  property_position
  u4     data_size2          // 本类型的“数据行数”
  vdata × data_size2
~~~

property_position 的编码（WolfTL Database.hpp:184-185 的权威判据）：

~~~
raw >= 2000 (STRING_START) ⇒ 字符串字段，字段序号 = raw - 2000
raw >= 1000 (INT_START)    ⇒ 数值字段，  字段序号 = raw - 1000
~~~

（Kaitai 写成 block = raw/1000、pos = raw%1000 也成立；但“边界 ≥ 2000”才是 WolfTL 的原始判据。
特殊值：STRING_INDICATOR = 0x0001D4C0 出现在类型的 sub_header 后第二个 u4 位置时，
表示该类型跟着一个 unknown_2 字符串。）

~~~
vdata（每个数据行）:
  s4[ 数值字段个数 ]     // 严格按字段表的出现顺序
  t_str[ 字符串字段个数 ] // 同样按出现顺序
~~~

**关键：先全部数值、再全部字符串**，不是在字段顺序里交错。
所以一行的读取取决于字段表里数值/字符串各自的个数：

~~~
字段表:  2000(串) 2001(串) 1000(数) 1001(数)
⇒ 每行 = s4, s4（数值，按出现顺序） + t_str, t_str（字符串，按出现顺序）
~~~

### 6.3 实测统计（各库前 3 个类型）

| 文件 | 类型 0 | 类型 1 | 类型 2 |
|---|---|---|---|
| DataBase.dat | dm=1，18 字段（14 数值 + 4 字符串），44 行 | dm=0，15 字段（13+2），34 行 | dm=1，21 字段（16+5），40 行 |
| CDataBase.dat | dm=1，51 字段（46+5），17 行 | dm=2，100 字段全数值，16 行 | dm=2，48 字段全数值，22 行 |
| SysDatabase.dat | dm=0，8 字段（4+4），50 行 | 4 字段（3+1），16 行 | 4 字段（3+1），6 行 |

DataBase 类型 0 的 property_position 原值：
2000, 2001, 1000, 1001, 1002, 1003, 1004, 1005, 2002, 1006, 2003, 1007, ..., 1013。
CDataBase 类型 0：2000..2004, 1000..1045（对应角色库：名字、职业、图形、等级/HP/SP 等）。
DataBase 类型 0 第一行数据示例：['什么都不做', '', 0, 10, 0, 0, 100, 0, '', 0, '', 0, 0, 0, 0, 0, 100, 0]。

### 6.4 SysDatabase 的 マップ設定（地图设定）——取地图清单的唯一正确来源

SysDatabase 类型 0（4 数值 + 4 字符串，50 行）的行样本：

| 行 | 值 |
|---|---|
| 0 | 'MapData/TitleMap.mps', -1, -1, 0, -1, '', '', '' |
| 1 | 'MapData/SampleMapA.mps', -2, -1, 0, 2, '', '', '' |
| 2 | 'MapData/SampleMapA_1.mps', -2, -1, 0, 1, '', '', '' |

- 第一列是 .mps 路径（字符串字段），后面是数值字段。
- -1 / -2 是“无 / 交给事件”哨兵，不要当真实索引用。
- 游戏只注册 6 张地图；Data/MapData/ 下有 9 个 .mps，多出的 3 个没有被注册。
- **不要用文件名猜地图顺序**，一律从 SysDatabase マップ設定 读。

work\sys.txt 保存了 SysDatabase 各段的可读字段名/类型（マップ設定、BGMリスト、BGSリスト、SEリスト、
文字列変数名、システム文字列、システム変数名、位置設定リスト、キャラクター画像、キャラ影グラフィック名、
ウィンドウ画像、トランジションタイプ、文字色、遠景画像、通常変数名、予備変数1..9、顔グラフィック名）。

### 6.5 .dat 里没有字段名——字段名在 .project 文件里

DataBase.dat 只描述“第 i 个字段是数值还是字符串”，**没有字段名、没有类型细节**。
字段名与类型在同级文件：DataBase.project、CDataBase.project、SysDatabase.project、
SysDataBaseBasic.project（已随 BasicData.wolf 解出，在 work\full\BasicData\）。

.project 的解析（WolfTL Database.hpp:323-371）：

~~~
u4  type_count
type × type_count:
    t_str  type_name
    u4     field_count
    field × field_count:  t_str  field_name
    u4     data_count
    data × data_count:    （行数据，与 .dat 的行同样编码）
    t_str  description
    u4     field_type_list_size
    u1[ field_count ]   每字段类型码
    skip( field_type_list_size - field_count )
    u4 index ; index × t_str                                   (unknown1，每字段说明)
    u4 index ; index × ( u4 args_count + args_count × t_str )   (string_args)
    u4 index ; index × ( u4 args_count + args_count × u4 )      (args)
    u4 index ; index × u4                                       (默认值)
~~~

**TODO（未验证）**：类型码的具体含义、unknown1 / string_args / args / 默认值的语义尚未逐个解码。
重写游戏时这些只是编辑器元数据，运行时用不到——运行时只需要字段名与“数值/字符串”分类。

### 6.6 次要点（不是 loader 必需）

- SysDataBaseBasic.dat（438 B）：容器不同（10 字节 0x00 开头 + 0xC0 @10 + u4 0x0C @11），footer 0xC0。TODO 未解码。
- MapTree.dat（416 B）：10 字节 0x00 开头 + 0x90 @10 + u4 50 @11，footer 0xBC；记录是 u4 id 与 0xFFFFFFFF 交替，8 字节/条。TODO 未解码。
- MapTreeOpenStatus.dat（75 B）：10 字节 0x00 开头，u4 0x32、footer 0x26。TODO 未解码。
- **以上三个 loader 都不需要**：地图清单来自 SysDatabase マップ設定（见 6.4）。

---

## 7. TileSetData.dat

**状态：已验证**（work\fs_ts.py；文件 60786 B，6 个 tileset 全部解析，结束位置 60785、footer 0xCF、trailing 0）

> 任务书原本把这一项列为“还没解开”，实际已经解开并逐字节验证过。
> 任务书里“u4 count=4 at offset 15”是误读——offset 15 是第一个 tileset 的 title 的 t_str 长度
> （值 4，因为 “街” 含 NUL 是 4 字节）。

### 7.1 容器布局

**容器与第 6 章的 database_dat 完全同构**，只是 version 不同：

| 偏移 | 类型 | 字段 | 实测 |
|---|---|---|---|
| 0 | 9 字节 | magic 00 57 00 00 4F 4C 55 46 4D | 固定 |
| 9 | u1 | version | **0xD2（=210）** |
| 10 | u4 | tileset_count | **6** |
| 14 | tileset × 6 | | 见 7.2 |
| 末尾 | u1 | footer | 0xCF |

（等价六字节写法：00 57 00 00 4F 4C + 0x55 + 46 4D 00，再 u1 version、u4 count。）

### 7.2 单个 tileset（version 210）

~~~
tileset:
  t_str  title                 // 实测 UTF-8（不是 Shift-JIS）
  t_str  base_tileset_file     // 基础图块图文件名
  t_str[ 31 ]  auto_tile_files // 自动图块文件名，固定 31 个槽位
  u1     separator1            // 恒 0xFF
  u4     tag_number_count
  u1[ tag_number_count ]  tag_numbers
  u1     separator2            // 恒 0xFF
  u4     tile_passable_count
  tile_passable[ tile_passable_count ]    // 每项 4 字节
~~~

version 209 的分支（本游戏不用）：字符串改成 t_str_sjis，且 auto_tile_files 是 15 个而不是 31 个。

### 7.3 tile_passable 的 4 字节位域（LE，按位从 LSB 读）

~~~
bit 0  counter_enabled
bit 1  square
bit 2  quarter_tile
bit 3  star
if quarter_tile:
    bit 4  top_left_passable
    bit 5  top_right_passable
    bit 6  bottom_left_passable
    bit 7  bottom_right_passable
else:
    bit 4  upwards_not_passable
    bit 5  rightwards_not_passable
    bit 6  leftwards_not_passable
    bit 7  downwards_not_passable
bits 8..13  na（应为 0）
bit 14 downarrow
bit 15 triangle
u2 后两字节 na（应为 0）
~~~

### 7.4 实测 6 个 tileset

| idx | 区间（起..止） | title | base 文件 | tag 数 | passable 数 |
|---|---|---|---|---|---|
| 0 | 14..12532 | 街 | MapChip/[Base]BaseChip_pipo.png | 2376 | 2376 |
| 1 | 12532..25102 | ダンジョン | BaseChip_pipo.png | 2376 | 2376 |
| 2 | 25102..37633 | 森 | BaseChip_pipo.png | 2376 | 2376 |
| 3 | 37633..39477 | ワールドマップ | MapChip/[Base]WorldMap_pipo.png | 216 | 216 |
| 4 | 39477..52002 | kiri | MapChip/kiri_project.png | 2376 | 2376 |
| 5 | 52002..60785 | 海岸ダンジョン | MapChip/map_parts01.png | 1648 | 1648 |

使用关系（Lead 补充）：SampleMapA / Map003 / Map024 / TitleMap 用 tileset 4（kiri_project.png）。
tileset 0 的 31 个自动图块槽位里 15 个非空（其余是单个 NUL 的空串）：
[A]Grass1-Dirt1_pipo.png、Grass1-Grass2、Grass1-Grass3、Water2、Water5、Water7、WaterFall2、
Flower、Snow、Snow_Grass4、Snow_Dirt2、Ice2、Dirt1-Dirt2、Wall-Up1、Wall-Up2。
**这 15 个名字与存档 SavePart1 里的 memdata 列表完全一致**（见 9.4）——互相印证。

**TODO（未验证）**：tile_passable 的 “bits 8..13 与末两字节必为 0” 只做了抽样，未做全文件断言。

---

## 8. .wolf / DXArchive v8

**状态：已验证**（work\fs_wolf_verify.py + work\fs_wolf_verify_all.py；
BasicData/MapData/SystemFile/scene/word 5 个归档 **636/636 文件与 UberWolfCli 的基准逐字节相等**，
覆盖“无压缩 / 仅 LZ / Huffman+LZ / 仅 Huffman”四个分支以及“首尾 Huffman”特例。）
**也可以直接先用 UberWolfCli.exe 解包**（本仓库已有基准），但自己实现只多约 2000-3000 行。

### 8.1 文件地图（磁盘顺序）

~~~
DARC_HEAD (64 B，明文)
→ 各文件的原始数据（未压缩的按原样；压缩的按各自的压缩块）
→ 文件名表 (Name Table)
→ DARC_FILEHEAD 表 (File Table)
→ DARC_DIRECTORY 表 (Directory Table，根目录在最后 32 字节)
~~~

后三段（从 FileNameTableStartAddress 到文件末尾）在磁盘上**整体压缩**：
磁盘布局 = Huffman(LZ(解压后的 head 区))，解出的 head 区长度必须等于 HeadSize。
不变量（7 个归档实测）：DirectoryTableStartAddress + 32 == HeadSize。

### 8.2 DARC_HEAD（64 字节，**明文**，已验证）

~~~
u2  Head                     // 恒 0x5844，即字节 "DX"（小端）
u2  Version                  // 恒 0x0008
u4  HeadSize                 // head 区的解压后大小（不含 DARC_HEAD 自身）
u8  DataStartAddress         // 第一个文件数据的地址；实测恒 64
u8  FileNameTableStartAddress
u8  FileTableStartAddress    // 相对 FileNameTableStartAddress
u8  DirectoryTableStartAddress // 相对 FileNameTableStartAddress
u4  CharCodeFormat           // 本游戏恒 932
u4  Flags                    // 低 16 位 = 标志；高 16 位 = cryptVersion
u1  HuffmanEncodeKB          // 本游戏恒 12
u1[15] Reserve               // 全 0
~~~

7 个归档实测：

| 归档 | 字节数 | HeadSize | NameTab | FileTab | DirTab |
|---|---|---|---|---|---|
| BasicData.wolf | 1278855 | 15240 | 1274696 | 4408 | 15208 |
| MapData.wolf | 146824 | 1048 | 146104 | 296 | 1016 |
| SystemFile.wolf | 666343 | 3716 | 664588 | 1308 | 3684 |
| word.wolf | 40681 | 13912 | 38052 | 5240 | 13880 |
| FIcon.wolf | 19699 | 1132 | 19024 | 308 | 1100 |
| CharaChip.wolf | 303464 | 11656 | 299684 | 5000 | 11624 |
| 1.4.0.wolf | 33551305 | 22288 | 33543780 | 7856 | 22256 |

全部：Head=0x5844、Version=0x0008、CharCodeFormat=932、Flags=0x013A0000、HuffmanEncodeKB=12、
DataStartAddress=64、Reserve 全 0。

Flags 解析：低 16 位 = 0x0000 ⇒ 位 0（DXA_FLAG_NO_KEY=0x1）为 0（**归档被加密**），
位 1（DXA_FLAG_NO_HEAD_PRESS=0x2）为 0（**head 区被压缩**）。
高 16 位 = 0x013A = 314 = **cryptVersion v3.14**。
本游戏 cryptVersion 314 < 331，所以走 **7 字节循环 XOR**，不是 ChaCha20 路径。

### 8.3 先讲清“鸡生蛋”：key 从哪来

**关键结论：.wolf 的 key 不来自 Game.dat 的 encryption_key，而是引擎内建的版本表。**
UberWolf 的 work\uberwolf\WolfDec.cpp 第 57-69 行的 DEFAULT_CRYPT_MODES 里，
每个 WOLF 版本对应一组 key 字节：

| 版本名 | cryptVersion | key |
|---|---|---|
| Wolf RPG v2.01 | 0x0 | 0f 53 e1 3e 04 37 12 17 60 0f 53 e1（DXArchive_VER5） |
| v2.10 | 0x0 | 4c d9 2a b7 28 9b ac 07 3e 77 ec 4c（VER5） |
| v2.20 | 0x0 | 38 50 40 28 72 4f 21 70 3b 73 35 38（VER6） |
| Wolf RPG v2.225 | 0x0 | 字符串 "WLFRPrO!p(;s5((8P@((UFWlu$#5(="（即 Game.dat 的 encryption_key） |
| v3.00 | 0x12C | 0F 53 E1 3E 8E B5 41 91 52 16 55 AE 34 C9 8F 79 59 2F 59 6B 95 19 9B 1B 35 9A 2F DE C9 7C 12 96 C3 14 B5 0F 53 E1 3E 8E 00 |
| **v3.14** | **0x13A** | **31 F9 01 36 A3 E3 8D 3C 7B C3 7D 25 AD 63 28 19 1B F7 8E 6C C4 E5 E2 76 82 EA 4F ED 61 DA E0 44 5B B6 46 3B 06 D5 CE B6 78 58 D0 7C 82 00** |
| v3.31 | 0x14B | CA 08 4C 5D 17 0D DA A1 D7 27 C8 41 54 38 82 32 54 B7 F9 46 8E 13 6B CA D0 5C 95 95 E2 DC 03 53 60 9B 4A 38 17 F3 69 59 A4 C7 9A 43 63 E6 54 AF DB BB 43 58 00 |
| v3.50 | 0x15E | D2 84 CE 28 CE 88 82 E4 2A 18 2E 4C 06 B4 EA 84 06 B8 C6 88 5A A0 9E 7C 56 40 BA 34 52 CC C6 7C 2E 14 12 68 FE 5C 76 94 86 78 8E 4C BE 88 66 9C 1E E0 8E 6C 00 |
| Wolf RPG ChaCha2 v1 | 0x64 | C9 82 F8 B4 ... 85 00（前 32 字节 key + 后 12 字节 nonce，0 结尾） |
| One Way Heroics | 0x0 | 字符串 "nGui9('&1=@3#a" |
| One Way Heroics Plus | 0x0 | 字符串 "Ph=X3^]o2A(,1=@3#a" |

选择规则：**cryptVersion = Flags >> 16**（本游戏 = 0x13A ⇒ v3.14）。
WolfDec.h 里注明：key 末尾要补一个 0x00 作为字符串终止符。完整表见 work\uberwolf\WolfDec.cpp（MIT）。

> 关于“必须先拿到 Game.dat 的 encryption_key”：
> encryption_key 只用于 **Pro 加密的 Game.dat**（WolfDxArcKey.hpp 那套 AES/CTR）。
> 本游戏的 Game.dat 是明文；被加密的是 .wolf 归档，用的是引擎内建 key。
> 破解 .wolf 的真正“鸡生蛋”是：**你需要知道 cryptVersion（从 Flags 高 16 位读）才能选对 key**——
> 而 Flags 就在明文的 DARC_HEAD 里，所以没有循环依赖。
> 反过来，**文件名/文件表/目录表那一段是被全局 key XOR 的**：没有正确 key，一个文件名都读不到。
> WOLF 自己是从 Game.dat 的 encryption_key 或内建表拿 key 传给 DXArchive 的；
> 对本游戏而言两条路都通向 v3.14 的那 44 字节。

### 8.4 手工解包步骤清单（照此实现即可复现 636/636）

**第 0 步：确定 key。**
1. 打开 .wolf，**原样**读前 64 字节（不解密）。
2. 校验 Head == 0x5844、Version == 0x0008。
3. cryptVersion = Flags >> 16；从 8.3 取 key 字节，末尾补 0x00 得到 KeyString。
   本游戏 = v3.14 的 44 字节 + 0x00。

**第 1 步：算全局 key（7 字节）。**

~~~
key_create(source_bytes):
    if len(source_bytes) < 4: source_bytes += b"DXLIBARC"      # "DXLIBARC\0"
    crc0 = crc32( source_bytes[0::2] )     # 偶数下标字节 —— 标准 CRC-32（与 zlib.crc32 等价）
    crc1 = crc32( source_bytes[1::2] )     # 奇数下标字节
    key = crc0.to_le_bytes()[0:4] + crc1.to_le_bytes()[0:3]    # 4 + 3 = 7 字节
~~~

CRC32 参数：多项式 0xEDB88320（反射）、初值 0xFFFFFFFF、末尾异或 0xFFFFFFFF。
（KeyString 的长度取 C 字符串长度，即**不含**末尾 0x00。）

**第 2 步：读 head 区。**
1. 从文件偏移 FileNameTableStartAddress 读到 **EOF**（即 文件长度 - NameTab 字节）。
2. 用全局 key 从 **position 0** 做 7 字节循环 XOR：data[i] ^= key[(0 + i) % 7]。
3. 解压（本游戏 Flags bit1 = 0 ⇒ 压缩）：先 Huffman 解码，再对结果做 LZ 解码；长度必须 == HeadSize。
   （若 Flags bit1 = 1，则 head 未压缩，跳过。）

**第 3 步：切三段。**
~~~
NameP = head 区起点
FileP = NameP + FileTableStartAddress
DirP  = NameP + DirectoryTableStartAddress
~~~

**第 4 步：读文件名表。** NameP 前 4 字节恒为 00 00 00 00，第一个条目从 NameP+4 开始。
每个条目：

~~~
u2 pack        // 名字字节长度（含 NUL）向上取整到 4 的倍数，再 /4
u2 parity      // 名字大写形式的全部字节之和 & 0xFFFF
u1[ pack*4 ]   // 名字的“全大写、NUL 补齐到 4 字节对齐”副本（小写 ASCII 折成大写）
u1[ align4(len+1) ]  // 原始大小写的名字，NUL 补齐
~~~

实测例：apple.png ⇒ pack=3、parity=0x0285、upper "APPLE.PNG\0\0\0"、orig "apple.png\0\0\0"（12 字节）。
**逐条扫描并校验 parity，扫到 FileTab 为止**即可建立名字表（BasicData.wolf 149 个名字全部校验通过）。

**第 5 步：读目录树。** 根目录 = DirP + 0：

~~~
DARC_DIRECTORY（32 字节）:
  u8 DirectoryAddress
  u8 ParentDirectoryAddress     // 根为 0xFFFFFFFFFFFFFFFF
  u8 FileHeadNum
  u8 FileHeadAddress

DARC_FILEHEAD（72 字节 = 9 × u8）:
  u8 NameAddress      // 相对 NameP
  u8 Attributes       // 位 0x10 = 目录
  u8 Time.Create
  u8 Time.LastAccess
  u8 Time.LastWrite
  u8 DataAddress
  u8 DataSize
  u8 PressDataSize     // 0xFFFFFFFFFFFFFFFF = 未压缩
  u8 HuffPressDataSize // 同上
~~~

- 目录自身的 DARC_FILEHEAD 位于 FileP + DirectoryAddress。
- 子项 i（0 起）位于 FileP + FileHeadAddress + i*72。
- 子目录的 DataAddress（在它的 FILEHEAD 里）相对 DirP，指向子目录的 DARC_DIRECTORY。
- 实测 BasicData 根目录：DirectoryAddress=0、Parent=0xFFFF...、FileHeadNum=149、FileHeadAddress=72。

**第 6 步：派生每个文件的 key。**
~~~
per_file_key = key_create( KeyString + 文件的大写名字 + 各祖先目录的大写名字 )   # 不含根
~~~
- 每个名字都从 NameP + FILEHEAD.NameAddress + 4 处读（**跳过 4 字节 pack/parity 头**），读到块内 NUL 为止。
- 祖先目录名通过 Directory->DirectoryAddress → FileP 处的 FILEHEAD → NameAddress 取。
- CharCodeFormat=932 时字符单位长度 = 1（UTF-16 才要乘 2）。

**第 7 步：解压文件数据。** 数据起点 = 文件偏移 DataStartAddress + DataAddress。
按 PressDataSize / HuffPressDataSize 是否为 0xFFFFFFFFFFFFFFFF 分四支：

| 分支 | 条件 | 步骤 |
|---|---|---|
| a 无压缩 | 两者都是 NOC | 分块（0x1000000）读 DataSize 字节；**XOR position = DataSize + 已写字节数** |
| b 仅 LZ | Huff = NOC，Press ≠ NOC | 读 PressDataSize 字节（**XOR position = DataSize**），LZ 解码 → DataSize |
| c Huffman + LZ | 两者都 ≠ NOC | 读 HuffPressDataSize 字节（XOR position = DataSize），Huffman 解码；若 HuffmanEncodeKB != 0xFF 且 PressDataSize > HuffmanEncodeKB*2048 做首尾拼接；再 LZ 解码 |
| d 仅 Huffman | Press = NOC，Huff ≠ NOC | 读 HuffPressDataSize 字节（XOR position = DataSize），Huffman 解码；若 HuffmanEncodeKB != 0xFF 且 DataSize > HuffmanEncodeKB*2048 做首尾拼接 |

**首尾（head/tail）拼接规则**（H = HuffmanEncodeKB * 1024 = 12288）：

~~~
LZ 缓冲区 = huffout[0..H]                                   # 头部 H 字节（已解 Huffman）
          + 原位直读的中间段（从数据起点 + HuffPressDataSize 读 PressDataSize - 2H 字节，
                            XOR position = DataSize + HuffPressDataSize）
          + huffout[H..2H] 搬到 LZ 缓冲区的**末尾**
然后对 LZ 缓冲区整体做 LZ 解码，得 DataSize 字节
~~~

**关键怪癖**：压缩文件的 XOR 相位**从该文件的“明文大小”开始**，即 (DataSize % 7)，不是从 0。
XOR 本身是 7 字节循环，position 先 % 7。无压缩分支的相位随写入进度累加（DataSize + writeSize）。
压缩块在磁盘上按 4 字节对齐（WriteSize = (DestSize+3)/4*4），所以磁盘上的压缩长度可能大于 PressDataSize；
以 PressDataSize / HuffPressDataSize 为准。

### 8.5 Huffman 解码（Header + 树 + 位序）

位流从每个字节的 **MSB 优先**读；但**解码输出时从每个字节的 LSB 优先**取位。这是最容易写错的地方。

~~~
头:
  OriginalSize = Read(Read(6) + 1)      # 先读 6 位得 n，再读 n+1 位
  PressSize    = Read(Read(6) + 1)
  权重表 256 项:
    BitNum = (Read(3) + 1) * 2 ; Minus = Read(1) ; SaveData = Read(BitNum)
    Weight[0] = SaveData
    Weight[i] = Minus ? Weight[i-1] - SaveData : Weight[i-1] + SaveData   # u16 回绕
  HeadSize = 已消耗字节数（向上取整）
建树（与编码器完全一致，必须逐位复刻）:
  节点 0..255 = 各符号，权重用上面重建出的 Weight[]
  反复从未被认领的节点里挑两个权重最小的（NodeIndex 递增扫描，比较用严格 >，平局按扫描序）:
       ChildNode[0] = 较小的, ChildNode[1] = 第二个, ParentNode = NodeNum, NodeNum++
  根 = 节点 510
解码 OriginalSize 个字节:
  从根 510 开始: 取一位（当前字节的 LSB 起，取完一位后 >>= 1，8 位用完再取下一字节）
  位为 0 → ChildNode[0]，位为 1 → ChildNode[1]；节点 <= 255 时输出该符号并回到根
~~~

C 源码里的 9 位查表（NodeIndexTable[512]）与“最后 17 字节特殊处理”都只是优化；
朴素版“每次都从根 510 走”结果完全一致。

### 8.6 LZ 解码（逐字节，含被修掉的坑）

~~~
destsize = u32@src[0]
srcsize  = u32@src[4] - 9
keycode  = src[8]
sp = 9
while srcsize > 0:
    if src[sp] != keycode:
        输出 src[sp]; sp += 1; srcsize -= 1
    elif src[sp+1] == keycode:
        输出 keycode; sp += 2; srcsize -= 2
    else:
        code = src[sp+1]; if code > keycode: code -= 1
        sp += 2; srcsize -= 2
        conbo = code >> 3
        if code & 4: conbo |= src[sp] << 5; sp += 1; srcsize -= 1
        conbo += 4                      # MIN_COMPRESS
        indexsize = code & 3
        indexsize == 0: index = src[sp];                     sp += 1; srcsize -= 1
        indexsize == 1: index = u16@src[sp];                 sp += 2; srcsize -= 2
        indexsize == 2: index = u16@src[sp] | src[sp+2]<<16; sp += 3; srcsize -= 3
        indexsize == 3: C 源码未处理（会越界）——遇到就报错
        index += 1
        重复 conbo 次: 输出 out[-index]      # ← 逐字节重叠拷贝
返回 destsize 字节
~~~

> **最大的坑**：复制必须是“逐字节、允许重叠”（out.append(out[-index]) 循环）。
> 若写成 out += out[-index:][-conbo:]（取窗口的**最后** conbo 字节），
> 在 conbo < index 时会产出错误数据——head 区约 96 字节后就烂掉，文件名表全花。

---

## 9. 存档 .sav

**状态：解密 + 头部 + 校验和 已验证**（work\fs_sav.py：9/9 存档全部通过）；
**SavePart1 前段 已验证**（work\fs_sav_p1.py）；**正文其余部分 未验证**（仅从 WolfSave 源码推断，标 TODO）。

参考实现：docs\vendor\WolfSave\（MIT，C++）。

### 9.1 解密（已验证，9/9 存档）

~~~
def decrypt(path):
    b = bytearray(open(path, "rb").read())
    seeds = [b[0], b[3], b[9]]          # 这三个字节从不被异或，所以原样保存种子
    for i, inc in enumerate((1, 2, 5)): # 三遍，步长 1 / 2 / 5
        state = seeds[i]
        for j in range(0x14, len(b), inc):
            state = (state * 214013 + 2531011) & 0xFFFFFFFF     # MSVC CRT rand 状态推进
            r = (state >> 16) & 0x7FFF                          # rand() 返回 15 位
            b[j] ^= (r >> 12) & 0xFF                            # rand() >> 12 ⇒ 只会是 0..7
    return b
~~~

- 起始偏移 **0x14**，三遍步长 **1 / 2 / 5**，异或掩码来自 **MSVC CRT 的 rand()**。
- 种子就是明文的 b[0]、b[3]、b[9]；**加密时**种子顺序相反（seed[0]=b[9]、seed[1]=b[3]、seed[2]=b[0]），
  自己写加密器必须还原这三个字节并重算校验和。
- 掩码只有 3 位 ⇒ 每字节只翻低 3 位，高 5 位是明文。

### 9.2 解密后的头部布局（已验证）

| 偏移 | 类型 | 字段 | 实测 |
|---|---|---|---|
| 0 | 1 | seed0（明文，未加密） | SaveData01 = 136 |
| 1 | 1 | ? | |
| 2 | 1 | **checksum** = 解密后 b[0x14..EOF] 全部字节之和 & 0xFF | 9/9 一致 |
| 3 | 1 | seed1（明文） | 182 |
| 4..5 | 2 | ? | |
| 6 | 1 | **0x55 表示 UTF-8**（WolfSave 用它判 m_isUTF8） | 0x55 |
| 7..8 | 2 | ? | |
| 9 | 1 | seed2（明文） | 95 |
| 0x0A..0x13 | 10 | ? | 全 0（实测） |
| 0x14 | 1 | **恒 0x19**（否则不是合法存档） | 0x19 |
| 0x15 | u2 | 名字长度（含 NUL） | 43 |
| 0x17 | N | 名字（UTF-8） | "妹相随～黑白世界的缤纷冒险～\0" |
| 0x17+N | u2 | **fileVersion** | 0x8C（140，< 上限 0x8E） |
| 0x44（本例） | - | 正文开始 | 见 9.3 |
| 最后 1 字节 | 1 | **恒 0x19** | 0x19 |

| 存档 | 字节数 | seeds [b0,b3,b9] | checksum |
|---|---|---|---|
| SaveData01.sav | 366146 | 136, 182, 95 | 0xB2 |
| SaveData02.sav | 361637 | 241, 177, 136 | 0xFA |
| SaveData03.sav | 364263 | 11, 134, 74 | 0x71 |
| SaveData04.sav | 365071 | 142, 176, 12 | 0xEC |
| SaveData05.sav | 385978 | 184, 10, 18 | 0x3A |
| SaveData06.sav | 364257 | 123, 1, 39 | 0xE0 |
| SaveData07.sav | 364399 | 134, 201, 14 | 0xB9 |
| SaveData08.sav | 395566 | 141, 100, 148 | 0xD8 |
| System.sav | 374216 | 186, 159, 232 | 0xA4 |

### 9.3 正文结构（**部分未验证**）

| 顺序 | 段 | 状态 |
|---|---|---|
| 1 | SavePart1 | 前段已验证（见 9.4） |
| 2 | SavePart2 | TODO 未验证（docs\vendor\WolfSave\SavePart2.h:39 起） |
| 3 | SavePart3 | TODO 未验证（SavePart3.h:38 起） |
| 4 | SavePart4 | TODO 未验证（SavePart4.h:39 起） |
| 5 | SavePart5 | TODO 未验证（SavePart5_1 @ SavePart5.h:44、SavePart5 @ :661） |
| 6 | VariableDatabase | TODO 未验证（VariableDatabase.h:281 起） |
| 7 | SavePart7 | TODO 未验证（SavePart7.h:40 起） |
| 末尾 | u1 = 0x19 | 已验证 |

版本门：SavePart5 等大量字段按 fileVersion 与 0x69 / 0x6B / 0x72 / 0x73 / 0x74 / 0x75 / 0x76 /
0x81 / 0x87 / 0x89 比较决定读不读。本游戏 fileVersion = 0x8C，因此所有" >= 0x89"及以下的门成立、
所有" < 0x81"的分支不走。
**实现提示**：WolfSave 用 MemData<T>（u4 len + 元素）表示变长数组，读顺序必须与源码逐行对齐；
字段名是通用的 m_var1..m_var123，不要凭名字猜。

### 9.4 SavePart1（前段已验证）

~~~
u4 var1 ; u4 var2 ; u4 var3            // 地图/坐标一类
if fileVersion >= 0x69:
    u4 var4 ; u4 var5 ; u4 var6        // 宽 / 高 / 图块层第一个 u4（哨兵探测）
    if var6 != 0xFFFFFFFF:             // 不是哨兵 ⇒ 回退 4 字节，读 3 层 w*h 个 u4
        Seek(-4) ; 读 var4*var5*3 个 u4
MemData var_md1                        // u4 len + 字节（实测 = 基础图块图文件名）
n = 7 / 15 / 31  （fileVersion >= 0x8A 时 31，>= 0x73 时 15，否则 7）
n × MemData<u4>                        // 实测 = 自动图块文件名列表（31 项）
s4 var7 ; if var7 > 0: var7 × (u4 或 u1，按 fileVersion >= 0x64 分)
s4 var8 ; if var8 > 0: var8 × SavePart1_1
if fileVersion >= 0x72: var9..var19（11 个 u4）
s4 var20 ; if var20 > 0: var20 × u4
~~~

SaveData01.sav 实测走通的路径：var1=1、var2=0、var3=0、var4=64、var5=36（= SampleMapA 的 64×36）、
var6=104444（≠ 0xFFFFFFFF ⇒ 走“回退 4 字节 + 读全部图块”分支）、随后 6912 = 3×64×36 个 u4 图块
（全部非 0，前 6 个都是 104444）；md1 = "MapChip/kiri_project.png"；
31 个 memdata 里 15 个非空，**与 TileSetData tileset 0 的 15 个自动图块名完全一致**（见 7.4）。
→ 存档本质是“运行时内存快照”：地图、图块层、图块集缓存、变量、数据库都在里面。
这个互证同时确认了 TileSetData 与 SavePart1 两边的解读都正确。

### 9.5 变量数据库（VariableDatabase）

WolfSaveParser::init 会去读 <存档相对路径>\../Data\BasicData\CDataBase.project 来确定字段名，
所以变量库是“用 .project 自描述”的（见 6.5）。
纯 Rust 读取器应当**读 .project 文件**，而不是把字段名硬编码。

---

## 10. 待解清单与下一步

### 10.1 已验证（可放心实现）

1. Game.dat 全部字段偏移与取值（含 static_randoms 边界）。
2. CommonEvent.dat 容器 + 550 事件全字段 + 事件尾部布局 + run_condition nibble。
3. event_command 骨架 + opcode 210/211/300/111 的参数布局 + route 块 + 全 48 opcode 的参数字节大小表。
4. .mps 头部、空地图哨兵、事件、事件页（含 0x00 页尾容错）。
5. 三个数据库的容器 + type/field/data 自描述结构 + 行编码（数值在前、字符串在后）。
6. TileSetData.dat 容器 + 每个 tileset 的字段 + tile_passable 位域 + 6 个 tileset 的实测值。
7. .wolf/DXArchive v8 全链路解包（636/636 文件逐字节校验通过）。
8. .sav 解密算法、头部布局、checksum、fileVersion、SavePart1 前段。

### 10.2 未验证 / TODO（按优先级）

| # | 事项 | 下一步怎么做 |
|---|---|---|
| 1 | 存档正文 SavePart2..7 + VariableDatabase 的逐字段布局 | 按 docs\vendor\WolfSave\SavePartN.h 逐行移植成 Python，用 9 个存档逐个断言“读完不越界且恰好落在末字节 0x19 前” |
| 2 | 若干 opcode 的参数语义（112 / 121 / 123 / 124 / 140 / 150 / 160 / 221 / 222 / 250 / 290 / 1000） | 同 opcode 样本对齐打印推字段；或改一个参数再存盘做差分（最可靠） |
| 3 | tile_passable 的 “bits 8..13 与末两字节必为 0” 全文件断言 | 在 work\fs_ts.py 里对全部 6 个 tileset 断言 |
| 4 | .project 的类型码 / unknown1 / string_args / args / 默认值语义 | 只在做数据库编辑器时需要 |
| 5 | SysDataBaseBasic.dat / MapTree.dat / MapTreeOpenStatus.dat | loader 不需要；要做先解各自容器 header |
| 6 | 文本内嵌控制码（\cself[n]、\v[n] 等） | grep word.wolf / 文本，做转义解析器；决定 Message / Choices 显示是否正确 |

### 10.3 给 Rust loader 的最小实现顺序

1. **容器层**：magic 校验 + t_str 读法（第 1 章）——所有文件共用。
2. **Game.dat**（第 2 章）：标题/字体/屏幕尺寸/encryption_key。
3. **CommonEvent.dat + event_command**（第 3、4 章）：先读成
   struct Command { cid: u32, params: Vec<u8>, strings: Vec<String>, branch_depth: u8, route: Option<Route> }
   ——**参数先存原始字节**，不要在 loader 层解释语义。
4. **.mps**（第 5 章）：地图、图块层、事件页（同一套 Command）。
5. **数据库**（第 6 章）：字段表 + 行的通用读取，字段名从 .project 补。
6. **TileSetData**（第 7 章）：图块通行性判定。
7. **.wolf**（第 8 章）：loader 最外层（先解包再走 2..6）。
8. **.sav**（第 9 章）：最后做，因为它依赖前面所有数据结构。
9. 解释器：**必须多协程**（108 个并行事件），分支用动态深度（最大 91），文本要处理内嵌控制码。

---

## 附录 A：五个曾经的失败点与最终解释

| # | 失败现象 | 根因 | 最终解释 |
|---|---|---|---|
| 1 | Game.dat 的 r1/r2 读成 2097216 / 4194432，后续全错位 | 把 u16_settings_len 当**字节数**（22）而不是 **u2 个数**（22 个 u2 = 44 字节） | u8_settings_len 是字节数、u16_settings_len 是个数；filesize 在 offset 272 而非 29238 |
| 2 | TileSetData.dat 按 Kaitai 的独立容器布局解析失败；后来又以为 count 在 offset 15 | 它的容器与 database_dat **完全同构**（00 57 00 00 4F 4C + 55 + 46 4D 00 + version 0xD2 + u4 count=6）；offset 15 是第一个 title 的 t_str 长度（"街" = 4 字节含 NUL） | 6 个 tileset 全部解析成功，end 60785、footer 0xCF |
| 3 | 字符串显示成乱码，一度以为 TileSetData v210 是 Shift-JIS | Windows 控制台是 GBK，Python 打印 UTF-8 会 mojibake | v210 的 title / base / autotile 都是 **UTF-8**；脚本里把 stdout 包成 UTF-8 即可 |
| 4 | .wolf head 区解压后在约 96 字节处开始腐烂，文件名表全花 | LZ 复制写成了“取窗口最后 conbo 字节”（out += out[-index:][-conbo:]），而正确语义是**逐字节重叠拷贝** | 改成 for _ in range(conbo): out.append(out[-index]) 后 636/636 文件全部逐字节匹配 |
| 5 | 一度认为 “.wolf 必须先拿到 Game.dat 的 encryption_key” 是循环依赖，且 head 区 key 找不到 | key 不是从 encryption_key 派生，而是引擎内建版本表；cryptVersion 就写在**明文** DARC_HEAD 的 Flags 高 16 位 | Flags = 0x013A0000 ⇒ cryptVersion 0x13A = v3.14 ⇒ 用 v3.14 的 44 字节 key；只有 Name/File/Dir 三段被 XOR，才需要 key |

（文档完。所有“已验证”项均可用 0.3 节列出的脚本重跑复现。）
