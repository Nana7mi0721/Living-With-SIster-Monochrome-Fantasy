# WOLF RPG Editor v3 二进制格式规格（实测版）

> 目标读者：准备用 Rust 重写《Living With Sister Monochrome Fantasy》运行时的程序员。
> 本文所有结构均来自**对真实文件的逐字节解析验证**，每节标注 已验证 / 未验证。
> 按本文实现 loader，即可解开 Data/ 下全部 .wolf 归档，并读懂 Game.dat、CommonEvent.dat、
> 9 张 .mps、3 个数据库、TileSetData.dat 与存档 .sav。

---

## 0. 文档范围与验证方法

### 0.1 实测对象

| 项 | 值 |
|---|---|
| 游戏 | Living With Sister Monochrome Fantasy（妹相随～黑白世界的缤纷冒险～），Steam appid 2429860 |
| 引擎 | WOLF RPG Editor v3.x（DxLib + DirectX，Data/ 下 26 个 .wolf 归档；★已实测纠正：实际是 **23 个 .wolf** + 4 个 .ttf 字体，见下方注） |
| 安装目录 | E:\SteamLibrary\steamapps\common\Living With SIster Monochrome Fantasy |
| 归档目录 | <安装目录>\Data\（★已实测：**23 个 .wolf**，不是 26；注意没有任何散落的 .dat） |
| 存档目录 | <安装目录>\Save\（SaveData01..08.sav、System.sav） |
| 解包参考 | work\full\<ArchiveName>\（已用 UberWolfCli.exe 解包，作为验证基准） |
| 屏幕 / 帧率 | 1024×576，tile 16px，60fps，8 方向角色图，3 帧动画，主字体 LOLITA |

> **★已实测纠正（归档数量）**：原文与任务书都写“Data/ 下 26 个 .wolf”，**实测是 23 个**
> （`find Data -name '*.wolf'`，顶层与递归都是 23；Data/ 下总共 27 个文件，另外 4 个是字体：
> 07yasashi.ttf、07yasashisagoshitsukubo-rudo.ttf、08YuanRougoshitsukubo-rudo.ttf、08gothic.ttf）。
> Rust reader 的 `every_archive_reads` 测试就是
> 按“扫目录、见 .wolf 就打开”写的，它扫到的正是这 23 个（4062 个条目）。以 23 为准。

**为什么必须直接读二进制**：WolfTL.exe 把 Data 转成了 JSON，但该 JSON **有损**——
丢掉了每条指令的 run_condition 附加段，指令数 124,204 vs 二进制 139,273（少约 11%）。
因此 Rust loader 必须以二进制为准，JSON 仅作人工交叉参考。

### 0.2 验证标记

- **已验证**：脚本跑过真实文件，解析末位置与文件长度、footer、内部断言全部一致。
- **未验证**：仅从参考源码或其他项目推断，或部分字段未逐字节比对。文末「待解清单」给出下一步。
- **★已实测纠正**（本文第二轮修订新增）：Rust 侧（crates/wolf-format）已经实现了 .mps / TileSetData /
  数据库三个 loader 且测试全绿，实现过程中发现本文有若干处**写错或写漏**。凡带此标记的段落都是
  用真实文件重新统计后改的，并**保留原文说明**（不静默删除），便于回溯。
- **★已实测纠正（第三轮修订）**：Rust 侧的 .wolf / DXArchive v8 reader 也已全绿
  （`crates\wolf-format\src\dxarchive\`，23 个归档 / 4062 个文件 / 771,585,007 字节全通），
  据此在第 8 章补了 4 处纠正（CRC 输入不含 NUL、head 区必须读到 EOF、parity 只算大写名、
  第二级压缩是自带 LZ 而非 LZ4）并新增 8.7「Rust 实现索引与回归基线」。标记规范同上。

### 0.3 验证脚本索引（都在 work\ 下，可直接重跑）

| 脚本 | 验证对象 | 结果 |
|---|---|---|
| work\fs_gd2.py | Game.dat 全字段偏移轨迹 | 已验证（29273 B，offset 与 footer 完全对齐） |
| work\fs_ce.py / work\fs_ce_params.py | CommonEvent.dat 550 事件 + 全指令流 | 已验证（end 5532174、footer 0x90、trailing 0） |
| work\parse_ce.py | CommonEvent.dat 事件尾部布局参考实现 | 已验证（本次又用它交叉校验一遍） |
| work\fs_db.py / work\fs_db_props.py | DataBase / CDataBase / SysDatabase | 已验证（footer 0xC2、trailing 0） |
| work\fs_ts.py | TileSetData.dat 6 个 tileset | 已验证（end 60785、footer 0xCF、trailing 0） |
| work\fs_mps.py | 9 张 .mps | 已验证（9/9 trailing 0、footer 0x66、事件数匹配） |
| work\fs_wolf_verify.py + work\fs_wolf_verify_all.py | .wolf 解包（DXArchive v8） | 已验证（5 归档 636/636 文件与基准逐字节相等） |
| crates\wolf-format\src\dxarchive\ + tests | Rust 侧 .wolf / DXArchive v8 reader | 23 归档 / 4062 文件 / 771,585,007 B 全部解出；636/636 逐字节相等（★本文据此纠正了 8.3 / 8.4 步骤 2、4 / 8.5） |
| work\fs_sav.py + work\fs_sav_p1.py | .sav 解密、头部、校验和、SavePart1 前段 | 已验证（9/9 存档）；正文其余部分未验证 |
| work\uberwolf\WolfDec.cpp | UberWolf 参考源码（.wolf 内建 key 表，MIT） | 参考来源 |
| crates\wolf-format\src\mps.rs + tests | Rust 侧 .mps loader | 9/9 地图走到 EOF（★本文据此纠正了 5.3 / 5.4 / 5.6） |
| crates\wolf-format\src\tileset.rs + tests | Rust 侧 TileSetData loader | 6/6 tileset、60,786 B（★本文据此纠正了 7.3 / 7.4） |
| crates\wolf-format\src\database.rs + tests | Rust 侧数据库 loader | 三个库 98 个类型（★本文据此纠正了 6.2 / 6.3） |

### 0.4 解包后文件的 sha256（基准值；不一致说明解包通道有误）

| 文件 | 字节数 | sha256 |
|---|---|---|
| Game.dat | 29273 | 6c8cfdc797f479a380973daa0ced7373efc211bb4e7955ee408d78645234c1c5 |
| CommonEvent.dat | 5532175 | 4d1f85cd93f66ce8b1d958097eb67e535a10d8df1f81cf06b0526ec6bedb96cf |
| DataBase.dat | 94967 | 294d75f57e9c66ebdf77e0579467808fd44e5de8c9a025ba8f638ff0df13a5d1 |
| CDataBase.dat | 81832 | 132f1e10b7b6c99620acfe1514ac243dbdc06feab84080ccd4e4823a20ce11ce |
| SysDatabase.dat | 11607 | 1f1b6b2ed31a945c9590c6c2eb7dc6aec3a147aa8d533edba71f9ff4be4c38d2 |
| TileSetData.dat | 60786 | fd06c830f7e4dd1052522149df981d803f5b9295f4e3b904d099dfdd600fa340 |
| TitleMap.mps | 24145 | 6a53d445062dcc3c93680733a06357c546acd94e813e5cac50dd8e1ac8b8d8a3 |
| SampleMapA.mps | 797933 | 869065cc8cd85c58ce9951fd5008d260ecdbbc4537e832ca7d7d44ffe2dda630 |
| SampleMapA_1.mps | 176577 | 264c49fba976ec04dffa02954f15c02c248a4758849157382349b8fbbb957cfe |
| SampleMapB.mps | 13476 | ac1212393c0bde7086f9479f4f6071e620c65d5f340955198d0155f4236f239d |
| Map002.mps | 48683 | 158bb21680e2e6f7914c3a03c8bc22d62cd2fa8eea12178c4ae54c83ad38adf7 |
| Map003.mps | 144891 | 7137c46bcc088c196a5d51d2b8d1ee34a5cb7cd4b2347f40aaaa2abd68584be0 |
| Map024.mps | 237 | 33c4ff16f506e58888ca288442480b1a685dfd53df6ed1c2d206deff1d9b3afe |
| Map031.mps | 1545 | 1b1c52f57a340694e23dfefce83fbc5aaed157b8af35a0f0852b96b351d09c86 |
| Dungeon.mps | 17719 | 02891d5243b2138697005feecfadacd8eda84b949c35c71d0e54f675711274f5 |

---

## 1. 通用约定

### 1.1 基本类型

全部小端（LE），无对齐填充（除非特别说明）。

| 记法 | 含义 |
|---|---|
| u1 / u2 / u4 / s4 | 1/2/4 字节无符号；s4 = 4 字节有符号 |
| t_str | u4 len + len 字节 UTF-8 内容，**len 含结尾 NUL**，末字节必为 0x00 |
| t_str_sjis | 同 t_str 但内容为 Shift-JIS（v2 老文件用；本游戏不用） |
| t_str_array | u4 count + count × t_str |
| t_u4_array | u4 count + count × u4 |
| t_s4_array | s4 count + count × s4 |
| magic_number | 见 1.2 |

t_str 实例（Game.dat 实测）：offset 53 处 u4 len = 43，后接 42 字节 UTF-8（"妹相随～黑白世界的缤纷冒险～"）+ 1 字节 0x00。

### 1.2 magic 前缀表（已验证）

本游戏容器文件都以同一 6 字节前缀开头，再接 version_header，然后按类型分叉：

~~~
byte 0..5 : 00 57 00 00 4F 4C        ("\0W\0\0OL")
byte 6    : version_header = 0x55    (v3 标记)
byte 7..  : 类型后缀
~~~

| 文件 | byte 7..9 | 之后 |
|---|---|---|
| Game.dat | magic 整体是 9 字节 00 57 00 00 4F 4C 00 46 4D（**不走上面 6+1 的分叉**） | byte 9 = version_header 0x55，byte 10 = u4 u8_settings_len |
| CommonEvent.dat | 46 43 00 ("FC\0") | version u1 = 0xC9，u4 count |
| DataBase.dat / CDataBase.dat / SysDatabase.dat / TileSetData.dat | 46 4D 00 ("FM\0") | version u1 = 0xC2 / 0xD2，u4 个数或长度 |

> WolfTL 的等价写法：它把 "57 00 00 4F 4C ?? 46 4D 00" 当 9 字节 magic，?? 位置放 version。
> 即 WolfTL 的 magic 起点比本文偏移多 1 字节（本文的 byte0 = 0x00 是它跳过的那个字节）。以本文表为准（逐字节实测）。

### 1.3 version_header / footer 枚举（实测用到的那几个已验证）

| 枚举 | 值 |
|---|---|
| version_header | 0x00 = v2；**0x55 = v3**（本游戏全部 0x55） |
| version_footer | 0xC2 = v2；0xC3 = Game.dat v3.0；0xC4 = v3.3 |
| CommonEvent.dat version | byte10 = 0xC9 |
| DataBase 家族 version | byte10 = 0xC2；TileSetData 为 0xD2(=210) |
| CommonEvent 容器 footer | 0x8F / 0x90 / 0x91 任一（本游戏实测 0x90） |
| .mps header/footer | byte16 = 0x55；末字节 = 0x66 |

### 1.4 字符串编码（重要，实测纠正）

- 所有 .dat/.mps 内的 t_str 都是 **UTF-8**，包括 TileSetData v210 的 title / base / autotile 文件名
  （Kaitai 的 tilesetdata_dat.ksy 把 v210 写成 t_str、v209 写成 t_str_sjis，实测 v210 就是 UTF-8）。
- **例外（实测，重要）**：TileSetData tileset 5（海岸ダンジョン）的 9 个 autotile 文件名在**原文件里就是坏字节**：
  既不是合法 UTF-8，也不是合法 cp932/GBK（个别项任何常见编码都解不开，例如
  4d6170436869702f625f6174f5f69d2e706e67 = "MapChip/b_at??.png"）。
  这是本地化时被双重编码写坏的残留，游戏自己引用的也是这串坏名字。
  → **loader 规则**：字符串一律保留**原始字节**用于文件查找与比较，只在显示时做 lossy UTF-8 解码；
  千万不要 decode 再 re-encode 回写，否则这些名字会失配、贴图丢失。
- 判断文件是否 UTF-8 通道的旁证：**byte 6 == 0x55**（WolfSave 也用它 m_isUTF8 = At(6)==0x55）。
- 早期看到的"乱码"只是 Windows GBK 控制台的显示问题；Python 里务必把 stdout 包成 UTF-8：
  ~~~python
  if (getattr(sys.stdout,'encoding','') or '').lower().replace('-','') != 'utf8':
      sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
  ~~~

### 1.5 变量 / 事件 ID 的编码习惯（实测样本）

- 变量 id 常见形如 0x00186Axx（1600000 起）；直接当 u4 用即可。
- 公共事件 id 常见 500xxx（500014…500264…），也出现 1600048 之类 >599999 的值。
- -1 / -2 表示"无 / 交给事件"这类哨兵（见第 6 章 SysDatabase マップ設定）。
- 0xFFFFFFFF 在 .mps 的图块层里是"无图块层"哨兵（见第 5 章）。

---

## 2. Game.dat

**状态：已验证**（work\fs_gd2.py；文件 29273 字节，逐字段偏移与 footer 全部对齐）

### 2.1 偏移表

| 偏移 | 类型 | 字段 | 实测值 |
|---|---|---|---|
| 0 | 9 字节 | magic 00 57 00 00 4F 4C 00 46 4D | 固定 |
| 9 | u1 | version_header | 0x55 |
| 10 | u4 | u8_settings_len | 35（**字节数**） |
| 14 | u1 × 35 | u8_settings | 见 2.2 |
| 49 | u4 | string_count | 13 |
| 53 | t_str × 13 | string_settings | 见 2.3，结束于 271 |
| 272 | u4 | filesize | 29272（必须 == 文件长度 - 1） |
| 276 | u4 | unknown3（WolfTL 的 m_unknownSize） | 0 |
| 280 | u4 | u16_settings_len | 22（**这是 u2 的个数，不是字节数**，区域 = 44 字节） |
| 284 | u2 × 22 | u16_settings | 见 2.4，结束于 327 |
| 328 | u4 × 2 + u1 × N | static_randoms | r1 = 22227、r2 = 25684，之后 28936 个 0..127 随机字节 |
| 29272 | u1 | footer | 0xC3 |

> 陷阱：u8_settings_len 是**字节数**，u16_settings_len 是**u2 个数**。把后者当字节数会整体错位 22 字节，
> 使 static_randoms 的 r1/r2 读成 2097216 / 4194432 这种假值。

### 2.2 u8_settings（35 字节，顺序即语义，实测值一并给出）

| # | 字段 | 实测 | 含义 |
|---|---|---|---|
| 0 | tile_size | 16 | 图块像素（枚举 16/32/40/48） |
| 1 | character_directions_image | 8 | 角色图方向数 |
| 2 | character_directions_move | 8 | 移动方向数 |
| 3 | guruguru_enabled | 1 | ぐるぐる（旋转移动）开关 |
| 4 | fps | 60 | 帧率 |
| 5 | character_shadow | 0 | 是否画影子 |
| 6 | midi_source | 1 | MIDI 源 |
| 7 | character_animation_patterns | 3 | 角色动画帧数 |
| 8 | character_movement_width | 0 | |
| 9 | character_movement_hitbox | 0 | |
| 10 | text_horizontal_padding | 0 | |
| 11 | text_line_spacing | 6 | |
| 12 | choice_line_spacing | 14 | |
| 13 | unknown | 0 | |
| 14 | anti_aliasing | 0 | 0 启用 / 1 禁用 / 2 禁用+双倍宽 |
| 15 | move_speed_event | 4 | 事件移动速度（枚举 4..9） |
| 16 | move_speed_hero_allies | 9 | 主角/同伴移动速度 |
| 17 | in_game_language | 4 | 4 = 简体中文 |
| 18 | image_scaling_method | 0 | 0 rough / 1 smooth |
| 19 | inactive_window_behavior | 1 | 0 暂停 / 1 继续运行 |
| 20 | system_language | 1 | 0 日 / 1 英 |
| 21..32 | unknown_2..unknown_5 + pro 开关组 | 全为 1 | 见下方说明 |
| 33 | pro_screen_scale | 1 | |
| 34 | pro_loading_fadeout | 0 | |

说明：Kaitai 在索引 21..34 列出 unknown_2、unknown_3、unknown_4、unknown_5、unknown_6、
pro_enable_f4/f5/f8/f11/f12/prtscr、pro_screen_scale、pro_loading_fadein、pro_loading_fadeout。
实测这 14 个字节 = 1,1,1,1,1,1,1,1,1,1,1,1,1,0；写 loader 时按上表顺序填结构体即可（哪个 1 属哪个开关不影响运行）。

### 2.3 string_settings（13 个 t_str，实测值）

| idx | 字段 | 实测（长度含 NUL） |
|---|---|---|
| 0 | game_title | 妹相随～黑白世界的缤纷冒险～ (len 43) |
| 1 | serial | 0000-0000 (len 10) |
| 2 | encryption_key | WLFRPrO!p(;s5((8P@((UFWlu$#5(= (len 31) |
| 3 | font_base | LOLITA |
| 4 | font_sub1 | LOLITA |
| 5 | font_sub2 | LOLITA |
| 6 | font_sub3 | LOLITA |
| 7 | starting_hero_graphic | ""（len 1） |
| 8 | game_subtitle | +DLC v2.03 |
| 9 | pro_game_loadingpic | SystemFile/logo.jpg |
| 10 | pro_game_loading_gauge | SystemFile/logo2.jpg |
| 11 | pro_title_during_loading | "" |
| 12 | pro_title_during_gameplay | "" |

字符串个数不固定：WolfTL 用 string_count >= 9 / > 9 / > 13 决定是否继续读
subtitle、loadingpic、gauge、title_during_loading/gameplay 以及 unknownString14。
**按 string_count 循环读 t_str 才是稳妥实现**（多余的直接忽略）。

> 关于 encryption_key：本游戏 Game.dat 是**明文**（magic 直接可见），不需要 Pro 的 AES/CTR 解密路径
> （WolfDxArcKey.hpp 那套）。encryption_key 只在"Pro 加密 Game.dat"时才是鸡生蛋的主角，
> 而 .wolf 的解包 key 与它无关（见第 8 章）。

### 2.4 u16_settings（22 个 u2，区域 44 字节，实测值）

| idx | 字段 | 实测 |
|---|---|---|
| 0 | unknown | 38218 |
| 1..14 | custom_move_speed_hero_allies_N / custom_move_speed_events_N，N=0..6 | (2,1) (4,2) (8,4) (16,8) (32,16) (64,32) (128,64) |
| 15 | unknown_2 | 325 |
| 16 | screen_dimensions_w | 1024 |
| 17 | screen_dimensions_h | 576 |
| 18 | wolf_rpg_version | **0**（编辑器没写；不要用它做特性判定） |
| 19 | pro_loading_gauge_x | 0 |
| 20 | pro_loading_gauge_y | 0 |
| 21 | unknown_3 | 21569 |

Kaitai 的字段表按 len 分档：len > 19 才有 idx19，> 20 才有 idx20，> 21 才有 idx21，> 22 才有 pro_default_screen_scale。
本游戏 len = 22，故只到 unknown_3（idx 21）。

### 2.5 static_randoms

- 起点 328，长度 = filesize - 328 = 28944 字节。
- 前 8 字节：u4 r1 = **22227**、u4 r2 = **25684**。
- 其后 28936 字节：全为 0..127 的随机字节（实测 min 0、max 127、128 个值都出现过）。
- 每次在编辑器保存 Game.dat 都会重新生成；可由前几字节推测后续（见 _wolfref\game_dat_random.c）。
- 运行时用途：静态随机数表。重写时可用同样 PRNG 复刻，或改用自家随机源
  （除非要复刻"读档后随机序列一致"的行为，否则无影响）。

---

## 3. CommonEvent.dat

**状态：已验证**（work\fs_ce.py / work\fs_ce_params.py / work\parse_ce.py；550 事件全部解析，
结束位置 5532174、容器 footer 0x90、trailing 0。文件 5532175 字节）

### 3.1 容器布局

| 偏移 | 类型 | 字段 | 实测 |
|---|---|---|---|
| 0 | 6 字节 | magic 00 57 00 00 4F 4C | |
| 6 | u1 | version_header | 0x55 |
| 7 | 3 字节 | magic2 46 43 00 ("FC\0") | |
| 10 | u1 | version | 0xC9 |
| 11 | u4 | count | 550 |
| 15 | common_event × 550 | | 见 3.2 |
| 5532174 | u1 | footer | 0x90（合法值 0x8F/0x90/0x91） |

### 3.2 单个 common_event 的字段顺序（已验证）

| 顺序 | 类型 | 字段 | 说明 |
|---|---|---|---|
| 1 | u1 | header | 恒为 **0x8E**（550/550 实测） |
| 2 | u4 | common_event_id | 事件 id |
| 3 | u1 | condition_operator / run_condition 打包字节 | **位序实测纠正见 3.2.1** |
| 4 | u4 | condition_variable | 触发条件变量 id |
| 5 | u4 | condition_value | 触发条件比较值 |
| 6 | u1 | argument_number_count | 数值参数个数 |
| 7 | u1 | argument_string_count | 字符串参数个数 |
| 8 | t_str | title | 事件名 |
| 9 | u4 | lines_count | 指令条数 |
| 10 | event_command × lines_count | | 见第 4 章 |
| 11 | 5 字节 | unknown4 | 恒为 01 00 00 00 00（550/550） |
| 12 | t_str | memo | 备注 |
| 13 | u1 | separator1 | 恒为 **0x8F** |
| 14 | t_str_array | argument_names | u4 count + count × t_str |
| 15 | u4 | argument_special_specification_mode_pagecount | |
| 16 | u1 × pagecount | argument_special_specification | 每页 1 字节：0 无 / 1 数据库引用 / 2 手动选项 |
| 17 | u4 | manual_option_string_pagecount | |
| 18 | 每页：u4 + u4 × t_str | 手动选项的显示字符串表 | **这是"表数组"，不是单表** |
| 19 | u4 | manual_option_value_pagecount | |
| 20 | 每页：u4 + u4 × u4 | 手动选项的数值表 | 同上 |
| 21 | t_s4_array | argument_number_default_value | 数值参数默认值 |
| 22 | u1 | separator2 | 恒为 **0x90** |
| 23 | u4 | color | 0 黑 1 红 2 蓝 3 绿 4 洋红 5 黄 6 灰 |
| 24 | t_str × 100 | cself_var_names | 本地变量名（每个各自带 u4 长度） |
| 25 | u1 | separator3 | 恒为 **0x91** |
| 26 | 5 字节 | unknown5 | 恒为 01 00 00 00 00 |
| 27 | u1 | separator4 | 恒为 **0x92** |
| 28 | t_str | return_name | 返回值名 |
| 29 | u4 | return_value_id | |
| 30 | u1 | separator5 | 恒为 **0x92** |

#### 3.2.1 位序纠正（run_condition）

Kaitai 的 commonevent_dat.ksy 把 condition_operator 写在 run_condition 前面，**实测反了**：

~~~
raw = byte
run_condition      = raw & 0x0F     // 低 4 位
condition_operator = (raw >> 4) & 0x0F
~~~

依据：全 550 事件的低 4 位直方图 = {0:442, 1:1, 2:97, 3:10}，正好复现"442 普通调用 / 1 auto_start /
97 并行处理(2) / 10 并行常驻(3)"；若取高 4 位会得到 {1:83, 2:465, 3:1, 6:1}（作为 run_condition 无意义）。

→ **108 个事件是每帧并行跑的后台协程（97+10+1）**，因此解释器必须是多协程/多任务，
不能是单一调用栈。这是整个重写中最关键的架构约束。

#### 3.2.2 argument_special_specification 的语义

- mode = 0：无特殊指定。
- mode = 1（数据库引用）：字符串表长度恒为 3，内容是 "-1" "-2" "-3"；数值表长度恒为 3，
  含义 = (数据库类别：0 系统库 / 1 用户库 / 2 变量库，数据库 id，not 标志：0 或 1)。
- mode = 2（手动选项）：字符串表 = 选项显示文本，数值表 = 对应的选项值。
- 实测 550 事件中 36 个事件带 manual 字符串表、195 个事件带数值表（见 work\ce3.txt）。

#### 3.2.3 事件统计（实测）

| 指标 | 值 |
|---|---|
| 事件数 | 550 |
| 指令槽总数 | 163,828 |
| 空指令（command_type == 0，NOP） | 24,555 |
| 真实指令 | 139,273 |
| 不同 opcode | 48 |

> 反直觉点：**param_count（指令首字节）永远 ≥ 1**——163,828 条指令里 param_count == 0 一次都没出现。
> 所谓"空指令"指 command_type == 0（NOP），它们的 param_count == 1（即参数区 0 字节）。

### 3.3 WolfTL JSON vs 二进制（为什么必须读二进制）

WolfTL 的 CommonEvents.hpp 只 dump 了指令的 (cid, args, strings, indent)，**没有 run_condition**，
且它按自己的方式解释 commandCount，导致条数变少（124,204 vs 139,273）。
若拿 WolfTL 的 JSON 当唯一数据源，会丢掉 108 个并行协程标记并漏掉约 1.5 万条指令。
→ Rust loader 必须直接读 CommonEvent.dat；JSON 仅用于人工核对。
## 4. 指令流编码（event_command）

**状态：编码骨架已验证**（work\fs_ce_params.py 走完全部 163,828 条指令，文件恰好结束在 5532174）。
**参数语义：部分已验证**（见 4.5 / 4.6）。

### 4.1 编码骨架（core）

~~~
event_command:
  u1  param_count          // 含 command_type 本身；实测恒 >= 1
  if param_count > 0:
      u4  command_type     // 即 opcode（小端）
      u1[ (param_count - 1) * 4 ]  params    // 参数区，字节长度 = (param_count-1)*4
  u1  branch_depth         // 分支嵌套深度（见 4.3，实测最大 91）
  u1  string_count
  t_str[ string_count ]  strings           // 该指令携带的字符串，全部在这里
  if param_count > 0:
      u1  have_route
      route_info  route    // 仅 have_route != 0 时存在
~~~

要点：

1. **参数区长度完全由 param_count 决定**，不需要按 opcode 分派就能正确跳过：
   长度 = (param_count - 1) * 4 字节。这和 Kaitai 的写法一致（它用 param_count*4-4）。
2. **所有字符串都在独立的字符串表里**，参数区只放数值。所以即使某个 opcode 的参数布局没解码，
   也能安全地"数值当黑盒字节、字符串照读"。
3. branch_depth / string_count / have_route 在**参数区之后**，不论 opcode 是什么都在同一位置。
4. 实测：have_route == 1 全文件只有 **5 条**，全部是 opcode 201（Move）——
   即"指令级移动路线"只被移动指令使用。其余 163,823 条 have_route == 0。

#### 4.1.1 指令尾字节（have_route 的实测语义）

参数区之后的那个 u1 是本指令的“结束标记”，实测只有两种值：

~~~
0x00 = 普通结束（163,823 条）
0x01 = 本指令带移动路线块（全文件只有 5 条，全部是 opcode 201 Move）
~~~

带路线时，0x01 之后是：

~~~
u1[5]  未知（route_info 的 animation_frequency / move_speed / move_frequency /
              move_route_mode / behavior_options 这前 5 个字节）
u1     flags（= character_move_route_options）
u4     route_count
RouteCommand[ route_count ]
~~~

RouteCommand 的实测编码（Lead 逐字节验证）：
u1 id → u1 arg_count → arg_count × u4 → 收尾 2 字节 01 00。
**注意**：Kaitai 的 route.ksy 把它写成“u1 type → u1 u4_arg_len → u4_arg_len×u4 → u1 u1_arg_len → u1_arg_len×u1”，
两者对“无参数”情形是一致的（00 01 00 = id 0、arg_count 0、收尾 01 00 里的 01 即 u1_arg_len、00 即那 1 个 u1 参数），
但对带参数的情形需要按 RouteCommand.hpp 的参数表逐 id 核对。
**TODO**：RouteCommand 各 id 的参数个数与含义尚未逐项验证（大部分可从 4.7 的 route_type 表 + arg 个数推）。

### 4.2 param_count == 0 的情况

**实测不存在**：163,828 条指令中 param_count 全 ≥ 1。
NOP（command_type == 0）的 param_count 恒为 1（参数区 0 字节），共 24,555 条。
写作 loader 时：仍建议保留 "param_count == 0 时直接跳过 command_type" 的分支以兼容其他 WOLF 版本，
但在本游戏数据里不会触发。

### 4.3 branch_depth 直方图（实测，必须用动态深度）

| depth | 条数 | depth | 条数 |
|---|---|---|---|
| 0 | 17225 | 11 | 685 |
| 1 | 39687 | 12 | 716 |
| 2 | 33567 | 13 | 594 |
| 3 | 25600 | 14 | 435 |
| 4 | 17842 | 15 | 110 |
| 5 | 10814 | 16 | 66 |
| 6 | 6369 | 17 | 55 |
| 7 | 4139 | 18 | 42 |
| 8 | 2194 | 19/20 | 26 |
| 9 | 1466 | 21/22/23 | 24/22/22 |
| 10 | 1093 | 24 | 20 |

depth 25..89 各 15 条，90 有 10 条，91 有 4 条。
→ **最大嵌套深度 91**。Rust 解释器不要用固定小数组做分支栈（用 Vec，或按深度动态分配）。

### 4.4 opcode 表（实测：公共事件 47 种非空 + 地图独有 3 种 = 并集 50 种；+ WolfTL 命名）

名称取自 docs\vendor\WolfTL-main\WolfTL\WolfRPG\Command.hpp 的 enum class CommandType。

| opcode | 名称 | 条数（含 NOP 统计口径，共 163,828） |
|---|---|---|
| 0 | Blank（NOP） | 24555 |
| 99 | Checkpoint | 152 |
| 101 | Message（显示文字） | 12027 |
| 102 | Choices（选项） | 495 |
| 103 | Comment | 9707 |
| 104 | （WolfTL 无此名；仅 1 次） | 1 |
| 105 | ForceStopMessage | 5 |
| 106 | DebugMessage | 76 |
| 107 | ClearDebugText | 1 |
| 111 | VariableCondition | 10743 |
| 112 | StringCondition | 123 |
| 121 | SetVariable | 17258 |
| 122 | SetString | 1030 |
| 123 | InputKey | 218 |
| 124 | SetVariableEx | 579 |
| 125 | AutoInput | 98 |
| 126 | BanInput | 10 |
| 130 | Teleport | 1 |
| 140 | Sound | 1368 |
| 150 | Picture | 16716 |
| 160 | SetTransition | 1 |
| 170 | StartLoop | 182 |
| 171 | BreakLoop | 114 |
| 172 | BreakEvent | 1318 |
| 174 | ReturnToTitle | 13 |
| 175 | EndGame | 2 |
| 176 | StartLoop2 | 54 |
| 179 | LoopTimes | 1836 |
| 180 | Wait | 4696 |
| 201 | Move | 5 |
| 210 | CommonEvent（调用公共事件） | 18468 |
| 211 | CommonEventReserve | 4 |
| 212 | SetLabel | 606 |
| 213 | JumpLabel | 1543 |
| 220 | SaveLoad | 8 |
| 221 | LoadGame | 16 |
| 222 | SaveGame | 5 |
| 250 | Database（数据库操作） | 2592 |
| 270 | Party | 5 |
| 281 | ScrollScreen | 3 |
| 290 | Effect | 640 |
| 300 | CommonEventByName（按名调用） | 1164 |
| 401 | ChoiceCase（选项分支） | 18555 |
| 420 | ElseCase | 3279 |
| 421 | CancelCase | 153 |
| 498 | LoopEnd | 2018 |
| 499 | BranchEnd | 11361 |
| 1000 | ProFeature | 24 |

覆盖率最高的 14 个（0/401/210/121/150/101/499/111/103/180/420/250/498/179）合计约 93.9%。

### 4.5 参数区大小表（per opcode，实测；loader 自检用）

下表是每个 opcode 实际出现过的参数区字节长度（= (param_count-1)*4）：

| opcode | 参数区大小 : 条数 |
|---|---|
| 0 | 0:24555 |
| 99 | 4:152 |
| 101 / 103 / 105 / 107 / 170 / 171 / 172 / 174 / 175 / 176 / 212 / 498 / 499 | 0（无参数，文本在字符串表） |
| 102 | 4:495 |
| 106 | 0:76 |
| 111 | 16:6736, 28:1564, 40:2254, 52:189 |
| 112 | 8:93, 12:18, 16:8, 20:4 |
| 121 | 16:17022, 20:236 |
| 122 | 8:562, 12:464, 16:4 |
| 123 | 8:216, 12:2 |
| 124 | 12:174, 16:405 |
| 125 | 4:98 |
| 126 | 4:10 |
| 130 | 20:1 |
| 140 | 16:147, 24:662, 28:559 |
| 150 | 8:203, 12:1839, 16:85, 24:952, 44:11677, 48:84, 52:55, 56:71, 60:43, 72:434, 76:1273 |
| 160 | 8:1 |
| 173 | 0（本次未出现，仅列在 opcode 表中供参考） |
| 179 | 4:1836 |
| 180 | 4:4696 |
| 201 | 4:5 |
| 210 | 8:292, 12:2647, 16:1418, 20:579, 24:4714, 28:8666, 36:152 |
| 211 | 8:4 |
| 213 | 0:1543 |
| 220 | 8:8 |
| 221 | 16:16 |
| 222 | 16:5 |
| 250 | 16:16, 20:2576 |
| 270 | 4:1, 8:3, 12:1 |
| 281 | 12:3 |
| 290 | 28:640 |
| 300 | 8:30, 12:304, 16:341, 20:321, 24:156, 28:8, 32:4 |
| 401 | 4:18555 |
| 420 | 4:3279 |
| 421 | 4:153 |
| 1000 | 4:24 |

### 4.6 已解码的参数布局（逐字节抽查过样本）

#### 4.6.1 通用 param_status 位域

opcode 210 / 211 / 300（以及任何"带可变个参数"的指令）使用同一个 u4 位域：

~~~
u4 param_status:
  bits 0..3   number_count          = raw & 0x0F
  bits 4..7   string_count          = (raw >> 4) & 0x0F
  bits 12..16 string_1_enabled .. string_5_enabled（WolfTL 的 string_1..5_enabled 位）
  bit  24     return_value_enabled
  count = number_count + string_count
~~~

#### 4.6.2 opcode 210 CommonEvent / 211 CommonEventReserve

~~~
u4 event_id
u4 param_status
s4[ param_status.count ]  params
if param_status.return_value_enabled: u4 return_variable_id
~~~

**实测纠正**：Kaitai 的 event_command.ksy 写着"若 500000 <= event_id <= 599999 才跟着 param_status"，
本文件里 **不成立**——event_id 实测范围 500014..1600048（>599999 的也照样跟 param_status），
一律都有 param_status。此规则复现了 18,468/18,468 条 opcode-210 与 4/4 条 opcode-211 的参数区大小。
高频 event_id：500264（7056 次）、500219（1265）、500218（1136）、500443（1106）、500228（958）、500217（568）。

样本：a4a10700 00000000（event_id 500132，status 0 ⇒ 无参数）；
2ea10700 01000000 ffffffff（event_id 500014，status 1 ⇒ count=1，一个参数 -1）。

#### 4.6.3 opcode 300 CommonEventByName

~~~
u4 na（实测恒 0）
u4 param_status（同 4.6.1）
s4[ count ]
if return_value_enabled: u4 return_variable_id
~~~

样本：00000000 02000001 006a1800 016a1800 016a1800（count = 2+0 = 2？注意 raw = 0x01000002 ⇒
低4位 = 2、bit24 = 1 ⇒ count=2 个 s4 + 1 个返回变量，与 32 字节的样本长度一致）。

#### 4.6.4 opcode 111 VariableCondition

~~~
u1 packed: 低 4 位 = condition_count；高 4 位 = use_else_branch
u1[3] 固定 00 00 00
condition_count × { u4 variable_id, s4 value, u4 operator }
~~~

operator 枚举：0 > / 1 >= / 2 == / 3 <= / 4 < / 5 != / 6 位与。
→ 参数区大小 = 4 + 12 × count，实测 count ∈ {1,2,3,4}（大小 16/28/40/52）。
实测最大 condition_count = 4。
样本：01000000 026a1800 01000000 01000000（1 个条件：变量 0x00186A02 与值 1，operator 1）；
三条件样本 03 000000 | (026a1800,1,2) | (026a1800,2,2) | (026a1800,3,2)。

#### 4.6.5 简单指令（参数区 0 字节）

101 Message、103 Comment、105 ForceStopMessage、106 DebugMessage、107 ClearDebugText、
170 StartLoop、171 BreakLoop、172 BreakEvent、174 ReturnToTitle、175 EndGame、176 StartLoop2、
212 SetLabel、213 JumpLabel、498 LoopEnd、499 BranchEnd。
文本 / 标签名全在字符串表（string_count ≥ 1）。实测 213 JumpLabel 与其对应 212 SetLabel 的字符串相同
（例如 移動時回復 / 戦闘時回復）。

#### 4.6.6 其他实测样本（布局未完全解码，给出原始字节）

| opcode | 样本参数区（hex） | 备注 |
|---|---|---|
| 121 SetVariable | 016a1800 016a1800 00000000 00080000 | 第 1 个 u4 = 目标变量 id（未完全验证） |
| 122 SetString | 096a1800 00000000 + 1 字符串 | |
| 140 Sound | 00000002 0a000000 00000000 00000000 46000000 64000000 00000000 + 1 字符串 | 字符串如 BGM/nekomimi.mp3 |
| 150 Picture | 03000001 156a1800 166a1800 ... | 主形 44 字节；参数含义未解 |
| 179 LoopTimes | 136a1800 | |
| 180 Wait | 0a000000 / 01000000 | 等待帧数或变量 |
| 201 Move | feffffff / ffffffff + route | 全文件仅 5 条，且这 5 条都带 route |
| 250 Database | 01000000 006a1800 0f6a1800 00000000 016a1800 | 大小 16/20 |
| 270 Party | 00000000 02000000 | |
| 290 Effect | 28 字节 | |
| 401 ChoiceCase | 01000000 | 分支 id |
| 420 ElseCase | 00000000 | |
| 421 CancelCase | 00000000 | |
| 999 / 1000 ProFeature | 03000000 + 1 字符串（如 Save/save\cself[12].） | |

**未解参数语义**（大小已知、字段含义未验证）：112、121、123、124、140、150、160、221、222、
250、290、1000。→ loader 策略：这些指令的参数区按原始字节保留，由解释器在实现该指令时再逐个解码；
不要为了"解析格式"而硬编码猜测。

### 4.7 路线块（route_info，用于 201 Move 与 .mps 事件页）

**状态：结构已验证（对 .mps 事件页逐字节走通）、字段语义已验证。**

~~~
route_info:
  u1 animation_frequency        // 枚举 频率 0..5（0 每帧 / 1 极短 / 2 短 / 3 中 / 4 长 / 5 极长）
  u1 move_speed                 // 枚举 0..6
  u1 move_frequency             // 同上频率枚举
  u1 move_route_mode            // 0 固定不动 / 1 自定义 / 2 随机 / 3 靠近主角
  u1 behavior_options           // 位域，LSB 起：半格左/半格上/方形碰撞盒/在主角之上/
                                //   可穿过/固定朝向/移动动画/待机动画
  u1 character_move_route_options  // LSB 起：na(低5位)、wait_until_done、skip_impossible_moves、repeat_actions
  u4 route_length
  route_data[ route_length ]
~~~

**route_data 的通用解析（重要简化）**——所有路线类型都可用同一段代码：

~~~
route_data:
  u1 type
  u1 u4_arg_len ;  skip u4_arg_len * 4 bytes
  u1 u1_arg_len ;  skip u1_arg_len bytes
~~~

因为"无参数"的类型其字节就是 00 01 00（u4_arg_len=0、u1_arg_len=1、1 个 u1 参数），
而带参数的命名类型（jump / assign_to_variable / add_to_variable / set_move_speed /
set_move_frequency / set_animation_frequency / wait_x_frames / move_approach_event /
move_approach_position / set_graphic / set_opacity / set_sound / set_height）都符合
"先 u4_arg_len、后 u1_arg_len"的同一形状。
→ Rust 只需一条 route_data 解析路径，再按 type 给参数取名。

route_data.type 枚举（_wolfref\route.ksy，0x00..0x3A）：

| 值 | 含义 | 值 | 含义 |
|---|---|---|---|
| 0x00 | 向下移动 | 0x1D | 设置移动速度 |
| 0x01 | 向左移动 | 0x1E | 设置移动频率 |
| 0x02 | 向右移动 | 0x1F | 设置动画频率 |
| 0x03 | 向上移动 | 0x20 | 待机动画开 |
| 0x04 | 左下移动 | 0x21 | 待机动画关 |
| 0x05 | 右下移动 | 0x22 | 移动动画开 |
| 0x06 | 左上移动 | 0x23 | 移动动画关 |
| 0x07 | 右上移动 | 0x24 | 固定朝向开 |
| 0x08 | 面向下 | 0x25 | 固定朝向关 |
| 0x09 | 面向左 | 0x26 | 可穿过开 |
| 0x0A | 面向右 | 0x27 | 可穿过关 |
| 0x0B | 面向上 | 0x28 | 总在最上层开 |
| 0x0C | 面向左下 | 0x29 | 总在最上层关 |
| 0x0D | 面向右下 | 0x2C | 设置图形 |
| 0x0E | 面向左上 | 0x2D | 设置不透明度 |
| 0x0F | 面向右上 | 0x2E | 设置声音 |
| 0x10 | 随机移动 | 0x2F | 等待 N 帧 |
| 0x11 | 朝主角移动 | 0x30 | 半格移动 |
| 0x12 | 远离主角移动 | 0x31 | 整格移动 |
| 0x13 | 向前一步 | 0x32 | 图案 1 |
| 0x14 | 退后一步 | 0x33 | 图案 2 |
| 0x15 | 跳跃 | 0x34 | 图案 3 |
| 0x16 | 右转 | 0x35 | 靠近事件 |
| 0x17 | 左转 | 0x36 | 靠近坐标 |
| 0x18 | 左右随机转 | 0x37 | 加到变量 |
| 0x19 | 随机朝向 | 0x38 | 图案 4 |
| 0x1A | 朝向主角 | 0x39 | 图案 5 |
| 0x1B | 背向主角 | 0x3A | 设置高度 |
| 0x1C | 赋值到变量 | | |

（0x2A / 0x2B 在 ksy 表中缺失，实现时遇到就报错。）

---

## 5. .mps 地图文件

**状态：已验证**（work\fs_mps.py；9/9 文件 trailing 0、末字节 0x66、事件数与 event_count 匹配）。

### 5.1 地图清单（实测）

| 文件 | 字节 | 宽 × 高 | 事件数 | 有图块层 |
|---|---|---|---|---|
| TitleMap.mps | 24145 | 53 × 30 | 1 | 是 |
| SampleMapA.mps | 797933 | 64 × 36 | 40 | 是 |
| SampleMapA_1.mps | 176577 | 64 × 36 | 12 | 是 |
| SampleMapB.mps | 13476 | 30 × 30 | 6 | 是 |
| Map002.mps | 48683 | 100 × 30 | 5 | 是 |
| Map003.mps | 144891 | 70 × 170 | 2 | 是 |
| Map024.mps | 237 | 53 × 30 | 1 | **否**（无图块层） |
| Map031.mps | 1545 | 53 × 30 | 1 | **否**（无图块层） |
| Dungeon.mps | 17719 | 40 × 30 | 5 | 是 |

游戏实际注册 6 张地图（SysDatabase 的 マップ設定，见 6.4）。
**9 张地图合计 73 个事件 / 134 页 / 37,610 条指令槽（30,558 条真实指令）/ 40 个 opcode**（详见 5.6.2）。
**地图事件总数 73 个**（早期侦察曾记成 67，是漏算了 Dungeon 的 5 个与 Map002 的 5 个中的一部分；
上表 9 行相加 = 73，与 5.6.2 的合计一致）。
⇒ 游戏几乎全靠 550 个公共事件驱动，地图侧只有 73 个入口。

### 5.2 头部布局（已验证）

| 偏移 | 类型 | 字段 | 实测 |
|---|---|---|---|
| 0 | 10 字节 | 全 0x00 | |
| 10 | 6 字节 | "WOLFM\0" | |
| 16 | u1 | version_header | 0x55 |
| 17 | 3 字节 | 全 0x00 | |
| 20 | u4 | unknown1 | 恒为 **0x64**（=100） |
| 24 | u1 | version | **0x66** |
| 25 | t_str | title | 地图名 |
| 36 | u4 | tileset | TileSetData 里的 tileset 序号（0 起） |
| 40 | u4 | width | 宽（图块数） |
| 44 | u4 | height | 高（图块数） |
| 48 | u4 | event_count | 事件数 |
| 52 | 图块数据块 | | 见 5.3 |
| 52 + 图块块大小 | event × event_count | | 见 5.4 |
| 末尾 | u1 | footer | 恒为 **0x66** |

（v3.5 及以上会在 title 之后多出 unknown4 u4 与 layer_count u4，本游戏 version == 0x66 恒为 3 层。）

**★已实测（9/9 张地图，Rust loader 已全绿）**：每张地图都能**精确走到 EOF**（trailing 0），
且头部四个值在 9 个文件里完全一致：version_header = 0x55、unknown1 = 100（0x64）、version = 0x66、
title = "なし"（本游戏所有地图名都是这个占位名，不要用它做地图标识）。

### 5.3 图块数据块与 -1 哨兵（**最容易踩的坑**）

~~~
offset 52:  u4 first_pixel
if first_pixel == 0xFFFFFFFF:
    // 该地图没有图块层，块就是这 4 个字节，事件紧随其后
else:
    // 共 width * height * 3 * 4 字节（3 层 × w × h × u4），已含刚读的 4 字节
~~~

实测证据：Map024.mps 与 Map031.mps 在 offset 52 是 ff ff ff ff，紧接着 offset 56 就是
事件头 6f 39 30 ...（0x6F + u4 0x3039）；其余 7 张地图读满 w*h*3*4 字节后事件头正好落在预期位置。

图块层顺序：layer0（地表）→ layer1（装饰）→ layer2（顶层），每层 w*h 个 u4，行优先。

**★已实测纠正（Rust 实现者踩到）：这块是“层优先（layer-major）”。**
内存排布是 先整层 layer0 的 w*h 个 u4（行优先），再整层 layer1，再整层 layer2；
而 _wolfref/mps.ksy 里的 mapdata_block 把它写成“像素优先”（一个像素连读 3 层，w*h*3 交错）。
两种理解的总字节跨度完全相同（w*h*3*4），所以顺序遍历仍然能正确走到事件区，
但**按像素取 tile 时会读错每一个图块**（layer1/layer2 会串到邻居像素上）。
Rust 侧正确写法：tile(layer, x, y) = u32@(块起点 + ((layer * height + y) * width + x) * 4)。

### 5.4 单个 u4 图块值的解码

~~~
raw = u4
autotile_id = raw / 100000
if autotile_id > 0:            // 自动图块（auto tile）
    4 位小数分别代表四个角的模式：
      top_left     = (raw % 10000) / 1000
      top_right    = (raw %  1000) /  100
      bottom_left  = (raw %   100) /   10
      bottom_right = raw % 10
else:                          // 普通图块
    base_tile = raw            // 直接是图块编号
~~~

实测 SampleMapA 的一层前 6 个值全为 104444（autotile_id=1，四角模式 0/4/4/4）。
0xFFFFFFFF 作为填充值表示"无图块"。

**★已实测纠正**：在本游戏**所有 7 张有图块层的地图里，图块区内部从来没有出现过 0xFFFFFFFF**——
空图块不是用填充值表示，而是干脆不存在（该格不被写入/被上层覆盖）。
0xFFFFFFFF 只在**块首**作为“整块省略”的哨兵出现（Map024 / Map031，见 5.3）。
⇒ loader 里“Tile::Empty”分支（raw == 0xFFFFFFFF）实现了也**不会被真实数据触发**，
属于防御性代码；不要反过来假设地图内部一定有空图块。

### 5.5 事件结构（已验证）

~~~
event:
  u1  header            // 恒 0x6F
  u4  header2           // 恒 0x3039 (=12345)
  u4  event_id
  t_str title
  u4  map_x
  u4  map_y
  u4  page_count
  u4  separator         // 恒 0
  page[ page_count ]
  u1  footer            // 恒 0x70
~~~

实测：SampleMapA 的 event0 名为 "ニューゲーム処理"（30,0），1 页，footer 0x70；
event1 名为 "妹クリック判定（夜"（63,0），3 页。
**注意事件名是"被截断保存"的**（这个 event1 的名字缺右括号），不是解析错误——
WOLF 编辑器按固定字节容量存事件名。做 loader 时不要试图"修复"。

### 5.6 事件页结构（已验证）

~~~
page:
  u1  header                     // 恒 0x79
  u4  graphic_chipid             // 实测 0xFFFFFFFF = 无（用下面的文件名）
  t_str graphic_filename
  u1  row_in_tileset
  u1  col_in_tileset
  u1  opacity
  u1  display_type               // 0 正常 / 1 乘算 / 2 加算 / 3 减算
  u1  trigger_condition          // 0 确认键 / 1 自动(autorun) / 2 并行 / 3 主角接触 / 4 事件接触（★本游戏实测 4 从未出现，见下）
  u1[4] trigger_switch           // 每字节：高 4 位 = operator，低 4 位 = enabled
                                 // ★名字有误导：这不是"开关条件"，槽位 j 的字节属于
                                 //   trigger_variable[j] 这一个变量条件，见 5.6.3
  u4[4] trigger_variable
  s4[4] trigger_value
  route_info route
  u4  command_count
  event_command[ command_count ]
  u4  unknown3                   // features；★134 页实测全为 3（>3 时才会多一个 pageTransfer 字节）
  u1  shadow_graphic_id
  u1  range_extension_x          // 上限 250
  u1  range_extension_y
  u1  footer                     // 恒 0x7A
~~~

**实测纠正（trigger_switch）**：Kaitai 的 mps.ksy 把 trigger_switch 的 enabled 位限制为 0/1，
但真实文件里该字节常见 0x20（operator 2、enabled 0）、0x21、0x11、0x31。
→ 应当**保留原始 u1**，取低 4 位当 enabled、高 4 位当 operator，不要断言取值范围。

> **★已实测纠正（字段含义）**：这 4 个字节**不是"开关条件"**，也不存在任何 switch id——
> 它们是 4 组**变量条件**的 operator<<4|enabled 标志，变量号在同一个槽位的
> trigger_variable[j] 里。完整证据、直方图与"哪一页生效"的判定规则见 **§5.6.3**。
> Rust 侧对应 MapEventPage::conditions()（保留原始 4 字节，按需解码）。

**★已实测纠正（9 张图全部 134 页，共 536 个 trigger_switch 字节的完整直方图）**：

| 原始字节 | 0x10 | 0x11 | 0x20 | 0x21 | 0x31 | 0x50 | 0x51 |
|---|---|---|---|---|---|---|---|
| 出现次数 | 2 | 25 | 324 | 173 | 3 | 2 | 7 |

- 合计 536 = 134 页 × 4 槽位，**只有这 7 种取值**，其余一概没有。
- 低 4 位（enabled）实测只有 **0 或 1**；高 4 位（operator）实测有 1/2/3/5。
  ⇒ 低 4 位可以断言 ≤ 1，但**高 4 位不要断言 0..6 的闭合范围**（operator 5 = not_equal 出现 7 次）。
- 0x50（operator 5、enabled 0）与 0x51（operator 5、enabled 1）都存在——Kaitai 的 any-of [0,1] 写法在这里彻底不成立。

实测 SampleMapA event1 的 page0：

- graphic_chipid = 0xFFFFFFFF、graphic_filename = ""、row 2、col 1、opacity 255、display_type 0
- trigger_condition = 2（并行处理）
- trigger_switch = [0x21, 0x21, 0x21, 0x21]
  **★已实测纠正**：本文早先把这里写成 0x33，那是把 work\fs_mps.py 输出的**十进制 33** 误当成了十六进制。
  真实字节是 **0x21**（operator 2、enabled 1），在 536 字节直方图里出现 173 次；
  0x33 在全库从未出现，且其低 4 位为 3，也与“低 4 位只有 0/1”的实测结论矛盾。
- trigger_variable = [3, 3, 3, 0]、trigger_value = [3, 2, 0, 0]
- route = animation_frequency 3 / move_speed 3 / move_frequency 3 / move_route_mode 0 /
  behavior_options 3 / route_options 2 / route_length 0
- command_count 113、unknown3(features) 3、shadow 0、range 0/0、footer 0x7A

### 5.6.1 页布局的完整字节图（Lead 逐字节验证，9/9 张地图全部解析成功）

~~~
u1   0x79                     页开始
u4   chipid                   // 常 0xFFFFFFFF = 使用下面的图形文件名
t_str graphicName
u1[4] direction, frame, opacity, renderMode
u1[37] conditions             // 1 trigger + 4×(operator<<4|enabled) + 4 triggerVar(u4) + 4 triggerVal(s4)
                              // ★37 字节是定长、没有 switch id 的余地，见 5.6.3
u1[4] movement                // animation_frequency, move_speed, move_frequency, move_route_mode
u1   flags                    // behavior_options
u1   routeFlags               // character_move_route_options
u4   routeCount ; RouteCommand[ routeCount ]
u4   commandCount ; event_command[ commandCount ]
u4   features                 // Kaitai 叫 unknown3，常态 = 3
u1[3] shadowGraphicNum, collisionWidth, collisionHeight
if features > 3: u1 pageTransfer
u1   0x7A                     页结束（容错见下）
~~~

- 这与 Kaitai 的字段表等价：Kaitai 的 row_in_tileset / col_in_tileset 就是这里的 direction / frame；
  37 字节 conditions 完全覆盖 Kaitai 的 trigger_condition + trigger_switch[4] + trigger_variable[4] + trigger_value[4]。
  **★已实测纠正（direction / frame 的真实含义，134 页）**：这两个字节**不是**图块表内的行列，
  而是角色的**方向与动画帧**——direction 是数字小键盘编码（2=下 4=左 6=右 8=上 9=右上），
  实测直方图 {2: 129, 4: 1, 6: 2, 9: 2}（8=上在本游戏中从未使用）；frame 实测 {0: 6, 1: 128}。
  ⇒ 只对 8 个带 CharaChip 图的页有意义，"图块表行列"的读法是错的。
- **★已实测（trigger_condition 直方图，134 页）**：{0: 14, 1: 26, 2: 90, 3: 4}——
  **取值 4（事件接触）在本游戏中从未出现**。⇒ 不要按“0..4 全都会出现”写分支覆盖测试；
  枚举照规格保留 5 个值，但测试与容错只需覆盖 0/1/2/3（其中 2 并行占 67%）。
- **容错（必须实现，★已实测确认：全库恰好 2 处）**：页结束字节有 2 处是 **0x00** 而非 0x7A——
  Dungeon 偏移 17716 与 Map024 偏移 234，**两处后面都紧跟 0x70**（事件结束）。
  Rust 侧规则：读到的结束字节**只有在下一位是 0x70 / 0x79 或已到 EOF 时**才接受 0x00，
  否则按解析错误处理；并且**保留原始 footer 字节**（0x00 或 0x7A）供上层诊断，不要强行归一化。
- commandCount 之后读 features；features > 3 时多一个 pageTransfer 字节。
  **★已实测：134 页的 features 全部 == 3**，所以 pageTransfer 这个字节在本游戏中**一次都没有出现过**，
  该分支属于“按规格实现但未被真实数据触发”。

### 5.6.2 9 张地图的最终统计（Lead 逐字节）

| 地图 | 事件 | 页 | 指令槽 |
|---|---|---|---|
| Dungeon.mps | 5 | 6 | 79 |
| Map002.mps | 5 | 7 | 416 |
| Map003.mps | 2 | 2 | 45 |
| Map024.mps | 1 | 1 | 3 |
| Map031.mps | 1 | 1 | 63 |
| SampleMapA.mps | 40 | 93 | 31083 |
| SampleMapA_1.mps | 12 | 17 | 5752 |
| SampleMapB.mps | 6 | 6 | 36 |
| TitleMap.mps | 1 | 1 | 133 |
| **合计** | **73** | **134** | **37,610**（其中 7,052 空 ⇒ 30,558 真实指令） |

地图侧用到 40 个 opcode，与公共事件（47 种非空）重合 37 种。**不是严格子集：`151 ChangeColor` / `161 PrepareTransition` / `162 ExecuteTransition` 这 3 种只在地图事件里出现**（公共事件侧 0 次）——转场/调色语义恰好全在地图侧。公共事件侧另有 10 种地图侧 0 次（106/107/160/171/175/176/211/222/281/1000）。**并集 50 种 = 必须实现的全集**（`work/probe97.py` 实测）。
（注意：本文 5.1 表里的“事件数”列与上表一致；WolfTL 本身解析失败的 Dungeon 与 Map024 也全部解析成功。）

### 5.6.3 页条件 = 4 组「变量比较」，**没有开关条件、也没有 switch id**（★已实测纠正）

**结论**（M2「哪一页事件生效」判定所需的全部依据）：

- `trigger_switch[4]` 这 4 个字节**不是开关条件**，里面没有任何 switch id。
  槽位 j 的字节 = 第 j 组**变量条件**的 `operator << 4 | enabled`，变量号与比较值在**同一个槽位**的
  `trigger_variable[j]` / `trigger_value[j]` 里。
- WOLF 的事件页条件**只有变量比较一种**：WOLF 官方编辑器文档（`work/_yado_0600.txt`）里「起動条件」的
  例子全是 `セルフ変数[0] が 1 と同じ` / `… が 0 と同じ`，「スイッチ」一词在 docs/ 与官方文档里 0 命中。
- 一页可以有 0 组启用条件，此时它无条件成立；**同一个事件最多 10 页**（官方原文：最大１０ページ；
  本游戏 SampleMapA 事件 9 正好 10 页）。

#### 证据 1：37 字节定长，没有装 switch id 的空间

WolfTL `Map.hpp:55`：`m_conditions = coder.Read(1 + 4 + 4 * 4 + 4 * 4);` = 1（trigger）+ 4（标志字节）
+ 4×u4（变量号）+ 4×s4（比较值）= **37 字节定长**；紧接其后就是 movement(4) / flags / routeFlags /
routeCount+RouteCommand[] / commandCount+Command[] / features / 3 个 u1 / 0x7A。
134 页按此宽度逐字节走完 9 张地图，恰好落在 0x70 / 0x79 / 0x66 上（含 §5.6.1 的两处 0x00 容错），
**一个字节都没剩**；要装 4 个开关 ID 至少还需 8~16 字节，文件里不存在这段。

#### 证据 2：标志字节与变量行是同一个下标（536 槽位交叉制表）

把每个槽位按「启用？」×「(变量号, 值) 是否等于编辑器默认值 `(1000000, 0)`」分类：

| | 默认值 `(1000000, 0)` | 非默认 |
|---|---|---|
| 未启用 | 304 | 24（编辑器残留） |
| 启用 | **1** | 207 |

唯一的「启用 + 默认值」是 SampleMapA.mps 事件 0 页 0 槽位 0：`0x21` + 变量 `1000000`(Self[0]) 值 `0`，
即真实条件 **「セルフ変数[0] == 0」**，不是异常。⇒「启用」位描述的就是同一行那条变量条件，
而不是另外 4 个开关的开关位（否则启用的行不会总带着有意义的变量号）。

#### 证据 3：统计（9 图 / 134 页 / 536 槽位）

- 启用 208 组、未启用 328 组；每页启用数直方图 `{0:19, 1:68, 2:15, 3:18, 4:14}`。
- 启用条件的运算符：`>=` 25 / `==` 173 / `<=` 3 / `!=` 7（即 `_wolfref/common.ksy::operator` 的
  1/2/3/5）。0=`>` 与 4=`<` 在全部 536 个槽位里**一次都没出现**（运算符只有 1/2/3/5 四种）。
- **全是数值比较**：若真是「开关条件」只需要 `==` 一种，而 35 组用的是 `>=`/`<=`/`!=`。
- 变量号落在 `docs/OPCODE_SPEC.md` §2.1 已实测的两个区段：`2000000+n`（普通变量/可変DB[0]）与
  `1000000+n`（事件局部变量）——例如 `2000062` = 普通变量 62、`1100000` = 事件局部变量 100000。

#### 证据 4：语义链（一页写变量 → 下一页的条件读它）

编辑器里「切页」的标准写法：第 N 页把某变量写成 k，第 N+1 页的条件是「该变量 == k」。

- Map002.mps 事件 3：页 0 条件 `2000009 == 1`、页 1 条件 `2000009 == 2`；
  页 0 的指令流（opcode 121）把 `2000009` 写成 2 ⇒ 正好切到页 1。
- Dungeon.mps 事件 3：页 0 无启用条件（无条件），其指令流把 `1100000` 写成 1；页 1 的唯一条件 `1100000 == 1`。
- SampleMapA.mps 事件 9（10 页，官方上限）：页 i 条件 `2000062 == i+1`，即一个变量把一个事件分成 10 种形态；
  事件 10 接着 `2000062 == 11..16`。
- SampleMapA.mps 事件 8（9 页）：页 0 是并行页，条件 `2000340 == 1` 且 `2000350 == 0` 且 `2000035 <= 0`；
  页 1..8 是自动页，条件 `1100009 == 1..8`。
  ⇒ **同一事件的 trigger 会随生效页变化**：本游戏 25 个多页事件里有 8 个页间 trigger 不一致
  （SampleMapA 的 4 / 8 / 11 / 14 / 18 / 21 / 22 / 24），所以 trigger、图形、指令全都属于**页**而不是事件。

#### 页选择的正确规则（照官方文档）

> 「起動条件を満たした**最も大きいページ番号**が画面上に表示されます。」

即在所有启用条件都成立的页里**取页号最大的那一页**（从末页往前找第一个满足的），
该页的 trigger / 图形 / 指令就是这个事件此刻的全部行为；一页都不满足 ⇒ 事件此刻不出现在地图上
（**不是"用页 0 兜底"**：页 0 也常有条件，19 页没有任何启用条件但它们是各自的最后一层兜底）。

Rust 侧已实现（`crates/wolf-format/src/mps.rs`；**不新增结构体字段**，因为
`crates/wolf-game/src/map_runtime.rs` 的测试夹具用完整的结构体字面量构造 `MapEventPage`）：

- `MapEventPage::conditions() -> [PageCondition; 4]`：把 4 个原始字节 + 变量 + 值解码为
  `{enabled, operator, variable, value}`，原始 `trigger_switches` 字段原样保留。
- `PageCondition::compare(actual) -> Option<bool>`（未验证的运算符返回 `None`，不静默当成 false）。
- `MapEventPage::conditions_met(get_variable)`、`MapEvent::active_page_index(get_variable)`、
  `MapEvent::active_page(…)`。

⇒ `wolf-game/src/map_runtime.rs::collect` 的正确做法：**每个事件只挑一页**（`active_page`），
再用**那一页**的 trigger 判断是否为 1/2/3；不能像现在这样让 trigger=1/2/3 的页各自独立启动，
否则 SampleMapA 事件 9 的 10 个并行页会同时跑起来。

### 5.7 地图事件的指令流

事件页的 event_command 与第 4 章**完全同一编码**（同一套 param_count/param_status/route 规则），
因此 Rust 侧只需要一个 event_command 解析器，公共事件与地图事件共用。
实测差异：地图事件指令里会用到 opcode 240 Chip / 241 ChipSet / 242（芯片操作），
它们在本游戏的公共事件里没出现过。Kaitai 的 event_command.ksy 给出如下布局（**未在本游戏数据中实测**）：

~~~
240 Chip:     s4 chipset_id ; u4 bitfield ; u2 na
              bitfield 位（LSB 起）：counter, square, na, star, not_up, not_right, not_left, not_down,
                                      x(6 位), downarrow, triangle
              chipset_id == -1000000 表示 reset
241 ChipSet:  u4 chipset_id
242:          u4 map_layer, map_x, map_y, width, height, chip_id
~~~
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
  u4     data_id_method      // 0 手动 / 1 与首字段同 / 2 与前类型同 / 10000 由类型推（★不要当闭区间校验，见下）
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
表示该类型跟着一个 unknown_2 字符串。
**★已实测：这个分支在本游戏的三个 .dat（DataBase / CDataBase / SysDatabase）里一次都没有出现**
⇒ 按规格实现即可，但属于“实现但未被真实数据触发”，不要为它写测试断言。）

**★已实测纠正（data_id_method 不是 0/1/2 的闭区间）**：Kaitai 的枚举只列了 0 / 1 / 2 / 10000，
但 CDataBase.dat 里实际还出现了 **20002、20003、20004** 和 **30000**。
⇒ loader **不得**把它当封闭枚举校验（否则 CDataBase 直接解析失败）；
按 u4 原样读出、只把已知的 0/1/2/10000 映射成名字，其余当 unknown 保留。

~~~
vdata（每个数据行）:
  s4[ 数值字段个数 ]     // 严格按字段表的出现顺序
  t_str[ 字符串字段个数 ] // 同样按出现顺序
~~~

**★已实测补充（零字段类型，必须支持）**：SysDatabase.dat 里有大量 **property_count == 0** 的类型——
类型 **4、5、6、14、15..23** 的字段数为 0，而它们的**数据行数**分别是
30 / 80 / 150 / 800 / 4 / 7 / 4 / 1 / 1 / 1 / 6 / 1 / 10（其中类型 14 是 800 行全零字节）。
这种类型的每一行是“0 个 s4 + 0 个 t_str”，即**每个数据行占 0 字节**。

⇒ **这是 Rust 实现最容易踩的坑**：如果用 “row_count <= 剩余字节数 / 行最小长度” 之类的边界来校验，
剩余字节数除以 0 会直接 panic 或把整个文件判为损坏。正确做法是：
用 checked_div（或先判字段数为 0 则跳过该判断）+ 一个“行数上限”的 sanity cap（例如 ≤ 100000），
而不是用剩余字节数去卡行数。零字段类型的 data_size2 只能**信任**（无法用字节数交叉验证）。

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

**★已实测（Rust loader 全绿，与本文数值一致）**：DataBase.dat 94,967 B / 40 类型 / 类型 0 是 18 字段
（14 数值 + 4 字符串）/ 44 行；CDataBase.dat 81,832 B / 30 类型 / 类型 0 是 51 字段（46 + 5）/ 17 行；
SysDatabase.dat 11,607 B / 28 类型 / 类型 0 是 8 字段（4 + 4）/ 50 行。三者 footer 均 0xC2、trailing 0。

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
- **★已实测：50 行里只有 6 行路径非空，按游戏顺序是**
  MapData/TitleMap.mps、MapData/SampleMapA.mps、MapData/SampleMapA_1.mps、
  MapData/Map003.mps、MapData/Map024.mps、MapData/Map031.mps
  （对应 5.1 表里的 6 张已注册地图；SampleMapB / Map002 / Dungeon 三个 .mps 存在但未注册）。
- 行 0 的数值部分是 [-1, -1, 0, -1]，行 1 是 [-2, -1, 0, 2]，行 2 是 [-2, -1, 0, 1]。
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
    data × data_count:    t_str 行名（是名字，不是 .dat 编码的行数据；见 6.8）
    t_str  description
    u4     field_type_list_size
    u1[ field_count ]   每字段类型码
    skip( field_type_list_size - field_count )
    u4 index ; index × t_str                                   (unknown1，每字段说明)
    u4 index ; index × ( u4 args_count + args_count × t_str )   (string_args)
    u4 index ; index × ( u4 args_count + args_count × u4 )      (args)
    u4 index ; index × u4                                       (默认值)
~~~

逐字节验证过的完整记录布局（含字段数/行数的实测表与勘误）见 **6.8**——按那里的顺序把四个块读过去，
三个文件的走查都精确落在文件末尾。

**TODO（未验证）**：类型码的具体含义、unknown1 / string_args / args / 默认值的语义尚未逐个解码。
重写游戏时这些只是编辑器元数据，运行时用不到——运行时只需要字段名与“数值/字符串”分类。

### 6.6 次要点（不是 loader 必需）

- SysDataBaseBasic.dat（438 B）：容器不同（10 字节 0x00 开头 + 0xC0 @10 + u4 0x0C @11），footer 0xC0。TODO 未解码。
- MapTree.dat（416 B）：10 字节 0x00 开头 + 0x90 @10 + u4 50 @11，footer 0xBC；记录是 u4 id 与 0xFFFFFFFF 交替，8 字节/条。TODO 未解码。
- MapTreeOpenStatus.dat（75 B）：10 字节 0x00 开头，u4 0x32、footer 0x26。TODO 未解码。
- **以上三个 loader 都不需要**：地图清单来自 SysDatabase マップ設定（见 6.4）。

### 6.7 SysDatabase.dat 逐类型实测 + 勘误（★已逐字节验证）

容器与 6.1 完全同构（同 9 字节 magic、u1 version 0xC2、u4 type_count、末字节 footer 0xC2、尾部 0 字节），
解析器直接复用 DataBase 的容器读取。work/full/BasicData/SysDatabase.dat：**11607 B、28 个类型、footer 0xC2 @11606、trailing 0 字节**。

28 个类型的「字段数 / 行数」实测表（顺序即类型序号，取自 type 记录本身，不是命名约定）：

| # | 字段数 | 行数 | # | 字段数 | 行数 | # | 字段数 | 行数 | # | 字段数 | 行数 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 8 | 50 | 7 | 3 | 10 | 14 | 0 | 800 | 21 | 0 | 6 |
| 1 | 4 | 16 | 8 | 1 | 300 | 15 | 0 | 4 | 22 | 0 | 1 |
| 2 | 4 | 6 | 9 | 1 | 6 | 16 | 0 | 7 | 23 | 0 | 10 |
| 3 | 4 | 20 | 10 | 2 | 3 | 17 | 0 | 4 | 24 | 1 | 12 |
| 4 | 0 | 30 | 11 | 1 | 1 | 18 | 0 | 1 | 25 | 1 | 1 |
| 5 | 0 | 80 | 12 | 3 | 30 | 19 | 0 | 1 | 26 | 1 | 1 |
| 6 | 0 | 150 | 13 | 4 | 80 | 20 | 0 | 1 | 27 | 1 | 1 |

- **13 个类型是 0 字段**（4、5、6、14..23）：它们只有行数，每行 0 字节，行内容完全是空的。
- 类型 12（文字色）是 3 个数值字段（赤/緑/青，0..255），30 行；实测 row0..7 =
  (255,255,255) (140,175,255) (210,130,130) (180,255,180) (90,40,0) (160,80,80) (255,140,0) (255,220,150)。
- 类型 24（顔グラフィック名）是 1 个字符串字段、12 行；row2/3/4 = Picture/face01.png、Picture/face02.png、DLC_data/member_20_s.png。
- 类型 0（マップ設定）见 6.4。注意**逻辑列号 ≠ 物理槽位**：字段序是 [字符串路径, 数值0=BGM, 数值1=BGS, 数值2=ループ, 数值3=遠景, 字符串…]，
  而行内的值永远是「先全部数值、再全部字符串」，所以取某个逻辑列必须按字段表走（本例列 3 = 0，列 4 = -1）。

**勘误（对 6.2 / 6.5）**

1. 6.2 说 data_id_method 只有 0/1/2——对本文件不成立也不算错：SysDatabase.dat 的 28 个类型**全部是 0**；
   CDataBase.dat 里才出现 20002/20003/20004/30000 这类值。所以 loader 不能假设取值集合，
   只能「等于 0x0001D4C0 时多读一个 t_str」。
2. 6.5 的 .project 记录里，字段名之后那一块实测是 **data_count 个 t_str 的行名**（不是 .dat 编码的行数据）；
   其后才是 description、u4 field_type_list_size、u1[field_count] 类型码，以及 6.8 里的四个编辑器元数据块。
   **修正此前的误判**：这四个块并不是「未解码的夹缝」——只要按 6.8 的顺序把四个块都读过去，
   三个文件的走查都**精确落在文件末尾**（SysDatabase 35972/35972、DataBase 174721/174721、CDataBase 170977/170977，
   剩余 0 字节），因此不需要「从上一个记录末尾向后扫描找对齐点」那种启发式做法。实测数据见 6.8。
3. 因此**类型名与行名都只存在于 .project**：0 字段类型（4/5/6/14..23）如类型 6（システム変数名，150 行）
   在 .dat 里取不到任何名字，只能由 .project 补齐（row1 = ﾒｯｾｰｼﾞｳｨﾝﾄﾞｳ X座標）。
   loader 把补名做成可选步骤：缺 .project 时名字为空、不报错。

### 6.8 .project 记录布局（★三个文件都精确走到 EOF，实测）

.dat 是纯位置式的：它给出「第 i 个字段是数值还是字符串」和每行的值，**一个名字都没有**。
名字在与之同名的 .project 里（DataBase.project / CDataBase.project / SysDatabase.project）。
记录与 .dat 的类型表**按下标一一对应**：.project 的第 i 条记录就是 .dat 的第 i 个类型。

~~~
u4  type_count                      ; 必须等于 .dat 的 u4 type_count
type × type_count:
    t_str  type_name                ; 编辑器里显示的类型名，可以为空串
    u4     field_count
    t_str × field_count   field_name
    u4     data_count
    t_str × data_count    row_name  ; 行名（不是 .dat 编码的行数据）
    t_str  description              ; 编辑器帮助文本，运行时用不到
    u4     field_type_bytes
    u1[field_type_bytes]            ; 每字段一个类型码（语义未解码）
    ; 以下四个块是编辑器元数据，长度必须读准，否则下一条记录会失步：
    u4 n1 ; t_str × n1                              ; 每字段的说明（unknown1）
    u4 n2 ; n2 × ( u4 k ; t_str × k )               ; string_args
    u4 n3 ; n3 × ( u4 k ; u4 × k )                  ; args
    u4 n4 ; u4 × n4                                 ; 默认值
~~~

t_str 与 .dat 相同：u4 字节数（**含**结尾 NUL）+ 负载。这里负载是 UTF-8（不像 .dat 的 SJIS 混排），
所以「结尾不是 NUL」或「不是合法 UTF-8」就可以判定这个偏移不是名字。

实测（work/full/BasicData/）：

| 文件 | 字节数 | 类型数 | 有名字的类型 | 字段名总数 | 行名总数 | 走查结束位置 |
|---|---|---|---|---|---|---|
| SysDatabase.project | 35972 | 28 | 25 | 39 | 1632 | 35972（剩余 0） |
| DataBase.project | 174721 | 40 | 23 | 592 | 723 | 174721（剩余 0） |
| CDataBase.project | 170977 | 30 | 24 | 973 | 723 | 170977（剩余 0） |

- **空名字是常态**：DataBase 有 17 个类型名是空串（18、19、21、22、23、24、29、30..39），
  CDataBase 有 6 个（22、23、24、26、28、29），SysDatabase 有 3 个（25、26、27）。
  这些槽位在编辑器里就是分隔行，loader 应把它们当作「无名」而不是错误。
- **字段数会与 .dat 不一致**：编辑器在记录里保留了 .dat 导出时丢掉的那个字段——
  DataBase 的 18、19、21、22、23、24、30、31（project 1 个字段 / .dat 0 个）与 CDataBase 的 22、26。
  这类类型**不能**把字段名贴到列上（列根本不存在），但类型名照常可用；行数则全部一致。
- 类型名举例（DataBase）：0 技能、2 アイテム、3 武器、17 システム設定、26 メッセージウィンドウ種類；
  type0 字段 0/1 = 技能の名前 / スキル説明，行 0/1 = 什么都不做 / 观察状况。
  CDataBase：0 主人公ステータス（字段 0 = 头衔1，行 0 = 勇者先生）、1 ┣ 技能習得Lv（字段 0 = <能力>覚えるレベル，16 行全无名）。
- Rust API（crates/wolf-format）：`Database::apply_project_names(&[u8]) -> usize` 返回**对上的记录数**
  （不等于 .dat 类型数时返回 0 且一个名字都不贴，避免把别的库/别的版本的名字错贴过来），
  之后用 `type_name(ty)` / `field_name(ty, column)` / `row_name(ty, row)`；
  `ProjectNames` / `ProjectTypeNames` 暴露原始记录与 `named_types()/named_fields()/named_rows()`。
  `SysDatabase` 共用同一套走查（`SysDatabase::apply_project_names` + `SysType::field_name`，
  见 6.7），空名字与「字段数不一致就不贴字段名」的规则完全相同。

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

**编码注记**：tileset 0..4 的 title / base / autotile 名字都是合法 UTF-8；
tileset 5 的 9 个 autotile 名字是坏字节（原文件即为双重编码残留，详见 1.4 的例外说明）。
loader 用原始字节比较文件名即可，不要做编码转换。

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
bits 8..9   na（★已实测：**不是 0**，携带真实数据，含义未知）
bits 10..13 na（实测恒 0）
bit 14 downarrow
bit 15 triangle
u2 后两字节 na（★已实测：6 个 tileset 里**全部为 0**，可以断言）
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

**★已实测（Rust loader 全绿）**：文件 60,786 字节；每个 tileset 固定 **31 个 autotile 槽位**；
6 个 tileset 的非空槽位数依次为 **15 / 15 / 15 / 15 / 15 / 12**（空槽是长度 1 的单个 NUL 串）。
tileset 5（海岸ダンジョン）有**恰好 9 个不是合法 UTF-8 的 autotile 名**，必须按原始字节保存（见 1.4 与 7.2 的编码注记）。

**★已实测纠正（原 TODO 作废）**：本文早先写的 “bits 8..13 与末两字节必为 0” 是**错的**。
Rust loader 对 6 个 tileset 的全部 passable 项做了直方图，结论是：

| tileset | bits 8..9 取值直方图 | 说明 |
|---|---|---|
| tileset 0（街） | {0: 1644, 1: 79, 2: 432, 3: 221} | bits 8..9 明显是两位真实数据 |
| tileset 3（ワールドマップ） | {0: 120, 2: 90, 3: 6} | 同上，取值 1 未出现 |

- **bits 8..9 携带真实数据，含义仍未知** ⇒ 规格里就叫 na，loader 原样保留、不要断言为 0。
- **bits 10..13 实测恒 0**；**整个高 16 位（u16 后两字节）在 6 个 tileset 里全部为 0**，这两条可以断言。

---

## 8. .wolf / DXArchive v8

**状态：已验证**（work\fs_wolf_verify.py + work\fs_wolf_verify_all.py；
BasicData/MapData/SystemFile/scene/word 5 个归档 **636/636 文件与 UberWolfCli 的基准逐字节相等**，
覆盖“无压缩 / 仅 LZ / Huffman+LZ / 仅 Huffman”四个分支以及“首尾 Huffman”特例。）
**也可以直接先用 UberWolfCli.exe 解包**（本仓库已有基准），但自己实现只多约 2000-3000 行。

**★已实测（Rust 实现全绿，第三轮修订）**：`crates\wolf-format\src\dxarchive\` 已把本章完整实现，
并把验证面从 5 个归档扩到 **Data\ 下全部 23 个 .wolf**：**4062 个文件条目、771,585,007 字节全部解出**，
五个已解包归档 **636/636 逐字节相等**。实现细节与回归基线见新增的 **8.7**；
本轮据此纠正了 4 处（8.3 的 NUL、8.4 步骤 2 的 EOF、8.4 步骤 4 的 parity 范围、8.5 的“不是 LZ4”），
每处都用 ★ 标记保留原文。

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

> **★已实测纠正（Rust 实现 `crates\wolf-format\src\dxarchive\key.rs` 全绿）——CRC 输入不含结尾 NUL。**
> 原文说“key 末尾要补 0x00 作为字符串终止符”，这句话本身没错（C 侧 `KeyCreate` 收到的是
> NUL 结尾的 `KeyString`），但**极易被误读成“CRC 也要把那颗 NUL 算进去”**，一旦算进去，
> 派生出的密钥就是错的，后面一个文件名都读不出来。准确规则是：
>
> | 量 | 值 | 是否进 CRC |
> |---|---|---|
> | key 主体（v3.14） | **45 字节**（31 F9 01 … D0 7C 82，即上表该行**去掉末尾 00**） | **是** |
> | 字符串终止符 | 1 字节 0x00 | **否** |
>
> 理由在 C 源码里：`DecodeArchive` 用 `KeyStringBytes = CL_strlen(ASCII, KeyString_)` 求长度
> （strlen 天然不含 NUL），再把 `KeyStringBytes` 传给 `KeyCreate`；`KeyCreate` 的
> `if (SourceBytes == 0) SourceBytes = CL_strlen(...)` 与偶数/奇数半段 CRC32 都只看这
> `SourceBytes` 个字节。所以：**缓冲区里补 0x00，但 CRC 只吃 45 字节。**
> 上表 v3.00 / v3.14 / v3.31 / v3.50 四行末尾的 `00` 属于字节转储，**不参与 CRC 输入**。
>
> Rust 侧断言（回归护栏，别删）：`key_create(&V3_14) == 3F 05 F9 EE F7 1E CC`，
> 这正是本作 23 个 .wolf 归档实测出来的全局密钥（`key.rs` 的
> `v3_14_derives_the_measured_archive_key`）。少算或多算那颗 NUL，这个断言立刻失败。
> 另注：`KeyCreate` 里“源短于 4 字节就补 `DXLIBARC`”的分支同样**不含**补进去的 NUL。

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
   本游戏 = v3.14 的 **45 字节** + 0x00（缓冲区共 46 字节；★已实测纠正：早先写的“44 字节”少数了一个）。

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

> **★已实测纠正——“读到 EOF”是硬要求，不是偷懒。** 这一条最容易被“优化”掉：既然
> `HeadSize` 就是 head 区的解压后大小，很多人会顺手写成“读 `HeadSize` 字节”（甚至
> `NameTab + HeadSize`），结果在**小归档**上静默截断 Huffman 流。实测两例：
>
> | 归档 | HeadSize（解压后） | 盘上 head 段实际字节数（= 文件长度 - NameTab） |
> |---|---|---|
> | BasicData.wolf | 15240 | **4159**（压缩后比明文小得多，读 15240 会被 Python/Rust 切片悄悄夹到 EOF，反而“碰巧对”） |
> | Kagura.wolf | 500 | **516**（压缩后**比明文大**；Huffman 头 316 B + 压数据 200 B = 516，只读 500 会短 16 B） |
>
> 所以两个方向的夹取都会出错：读 `HeadSize` 在 Kagura 上截断，读 `NameTab+HeadSize` 在
> BasicData 上截断（Rust 的切片越界会 panic，Python 的切片会静默夹取——**Python 的夹取正是
> 早期脚本能跑通的原因**，换成 Rust 后必须显式写成 `&data[name_tab..]` 才能复现）。
> 正确写法只有一句：**从 `FileNameTableStartAddress` 一直取到文件末尾**，Huffman 解码器
> 自己知道该吃多少（`Huffman_Decode` 先解头再按 `OriginalSize` 输出，多余的尾巴它不碰）。
> 截断的后果不是报错而是**乱码**：Huffman 读到半截会用 0 补位，LZ 层再把错误的 head 区解出来，
> 名字表全花。Rust 侧 `read_head_region` 就是按这条写的，且解完立刻断言
> `len(head) == HeadSize`。

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

> **★已实测纠正——parity 只覆盖“大写化后的名字字节”，不含 NUL 填充。**
> 原文“名字大写形式的全部字节之和 & 0xFFFF”容易被读成“把 `pack*4` 那一整块都加起来”
> （那样会把 1~3 个补齐用的 0x00 也加进去，但因为加的是 0，结果**恰好**相同——所以这个坑不会
> 立刻暴露，一旦有人改成“补齐字节非 0”就炸）。精确定义：
>
> ```
> parity = sum( 大写副本的第 0..len 字节 ) & 0xFFFF      # len = 名字长度（不含 NUL）；NUL 补齐段不计
> ```
>
> 也就是说：遍历原始名字 `FileName[0..len]`，逐字符取它**写进大写副本的那个字节**
> （小写 ASCII 折成大写；Shift-JIS 双字节字符两字节原样照抄并**两字节都累加**），累加后取低 16 位。
> 依据是 C 侧 `AddFileNameData`（`DXArchive.cpp:256`）：`Parity` 只在那个
> `for (i = 0; FileName[i] != 0;)` 循环里累加，循环条件天然排除 NUL 与补齐段。
>
> 实测规模（Rust 侧对 **23 个 .wolf 归档的全部 4062 个文件条目**逐个重算并比对）：
> **4062/4062 全部吻合，0 个不符**。所以 `parity` 是一个**可以硬校验**的字段（Rust 的
> `Tables::name` 就是不符即报 `Error::field`），不要只当装饰跳过。

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

> **★已实测纠正——第二级压缩不是 LZ4。** 任务书和若干二手资料把它写成 “LZ4 block decode”，
> **错的**。本格式用的两级压缩是：第一级 **DXArchive 自带的 Huffman**（§8.5），第二级
> **DXArchive 自带的 LZ**（`Decode`，`docs\vendor\WolfDec-master\src\WolfDec\3rdParty\DXArchive.cpp:1785`，见 §8.6）。
> 那个 LZ 是 WOLF 自己的字节级方案（literal / doubled-escape / back-reference，距离 1..3 字节，
> 最短匹配 4 字节，见 §8.6 的 opcode 表），**和 LZ4 的帧格式、token 编码、hash 表全都不同**，
> 用任何 LZ4 实现去解都只会得到垃圾。
>
> **实现要求：自己按 §8.6 写解码器，不要引 `lz4` / `lz4_flex` / `lz-fear` 之类的 crate。**
> 本仓库 `crates\wolf-format` 是 `#![forbid(unsafe_code)]` + 零外部依赖的，LZ 解码只有
> 约 60 行（`crates\wolf-format\src\dxarchive\lz.rs`），引 crate 既没必要也跑不通。

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

> **★已实测补充（同一条坑的 Rust 写法）。** Rust 里的正确写法是
> `for _ in 0..length { let byte = out[out.len() - index]; out.push(byte); }`，
> **不要**写成 `out.extend_from_slice(&out[len - index..len - index + length])` 之类的
> 切片拷贝——那正是在“取窗口一段”的思路，遇到 `length > index`（自重叠游程，真实数据里很常见）
> 就会借到还没写出来的字节，或者干脆 panic。逐字节 push 天然处理自重叠：
> 每写一个字节，窗口就跟着长一格，这正是 LZ 想要的语义。
> 实测确认：`crates\wolf-format\src\dxarchive\lz.rs` 用逐字节写法，
> 单测里专门构造了 `conbo=32 > index=2` 的自重叠游程，输出与 Python 基准逐字节相同。
>
> **另注（`indexsize == 3`）**：C 源码的 `switch` 只写了 case 0/1/2，case 3 没有分支，
> `index` 保持未初始化值就往下走——那是 C 的未定义行为，不是“格式允许 4 字节距离”。
> Rust 侧必须显式报错（`lz.rs` 抛 `Error::field(..., "LZ token uses a 3-byte distance, which the format never emits")`）。
> 本作 4062 个文件全部读完也没触发过该分支，可以放心当“不可能事件”处理。

---

### 8.7 Rust 实现索引与回归基线（★已实测）

本章已在 Rust 侧完整实现并通过全部字节级比对，代码在 `crates\wolf-format\src\dxarchive\`。
crate 是 `#![forbid(unsafe_code)]` + **零外部依赖**，所以 CRC32 / Huffman / LZ / Shift-JIS 全部手写。

| 文件 | 行数量级 | 负责什么 | 对应本章 |
|---|---|---|---|
| `mod.rs` | ~760 | `DARC_HEAD` 解析与不变量校验、名字表 / 目录表 / 文件头表遍历、`DxArchive` / `ArchiveEntry` 公开 API、四个存储分支的 `read()` | 8.1 / 8.2 / 8.4 |
| `key.rs` | ~155 | 内建 key 表（v3.00 / v3.14 / v3.31 / v3.50）、`key_create`（偶/奇半段 CRC32 → 4+3 字节）、7 字节循环 `key_conv` | 8.3 / 8.4 步骤 1、6 |
| `crc32.rs` | ~41 | 反射 CRC-32（0xEDB88320，初值/末异或 0xFFFFFFFF），逐位实现（输入只有几十字节，不需要查表） | 8.4 步骤 1 |
| `huffman.rs` | ~227 | 头部位流（MSB 优先）、权重表重建、与 C 完全一致的建树（最小两项扫描 + u64 权重求和）、解码（**LSB 优先**、朴素地从根 510 走） | 8.5 |
| `lz.rs` | ~181 | literal / doubled-escape / back-reference 三种 token、1~3 字节距离、**逐字节重叠拷贝** | 8.6 |
| `sjis.rs` | ~110 | CP932（Shift-JIS）→ UTF-8 名字解码；未定义双字节对按 Python `errors='replace'` 语义回退（输出 U+FFFD 后把尾字节当新字节重解） | 8.2 `CharCodeFormat=932` |
| `cp932.rs` | ~1037 | **生成文件**：60 lead × 188 trail 的 `u16` 双字节表（0 = 未定义），由 `work\gen_cp932.py` 从 Python 的 cp932 codec 生成，**不要手改** | 同上 |

**公开 API**（`crates\wolf-format\src\dxarchive\mod.rs`）：

~~~
DxArchive::open(data: &[u8]) -> Result<DxArchive>
DxArchive::entries(&self) -> &[ArchiveEntry]
DxArchive::entry(&self, name: &str) -> Option<&ArchiveEntry>   // 先按根级名字，再按完整路径
DxArchive::read(&self, entry: &ArchiveEntry) -> Result<Vec<u8>> // 返回明文
ArchiveEntry { name, directory, data_offset, size, press_size, huff_press_size, is_compressed }
ArchiveEntry::path(&self) -> String                            // directory + "/" + name
// 另有只读 getter：version / char_code_format / flags / crypt_version /
// huffman_encode_kb / is_encrypted / data_start_address
~~~

**四个存储分支都已被真实数据覆盖**（`read()` 按 `PressDataSize` / `HuffPressDataSize` 是否为 NOC 分派）：

| 分支 | 本作命中情况 | 备注 |
|---|---|---|
| a 无压缩（两者皆 NOC） | 有（小 .dat / 小 .png 常见） | 分块 0x1000000 读，XOR 相位随写入进度累加 |
| b 仅 LZ | 有 | 无首尾拼接 |
| c Huffman + LZ | 有（大文件主力） | **会走首尾拼接**（`PressDataSize > HuffmanEncodeKB*2048` 时） |
| d 仅 Huffman | 有 | 首尾拼接的判据换成 `DataSize` |

**回归基线（本机实测，改任何一处都要重跑）**：

| 项 | 数字 |
|---|---|
| 全量扫描的归档数 | **23 个 .wolf**（Data\ 下的全部） |
| 全部条目数 | **4062 个文件条目**（名字表 parity 4062/4062 校验通过） |
| 解出的总字节数 | **771,585,007 字节** |
| 与 UberWolfCli 基准逐字节相等 | **636/636**（BasicData 149、MapData 9、SystemFile 32、scene 327、word 119） |
| 头部字段 | 23/23 全部 Version 8、CharCodeFormat 932、Flags 0x013A0000、HuffmanEncodeKB 12、cryptVersion 0x13A |
| 目录深度 | 全部归档 **maxdepth = 0**（没有子目录；祖先目录那半边 key 派生逻辑本作不触发，但仍按 §8.4 步骤 6 正确实现） |

各归档条目数：1.4.0=199、BGM=49、BalloonBase_CafeLatte=13、BasicData=149、BattleEffect=92、
CharaChip=91、DLC_data=633、EnemyGraphic=118、FIcon=10、Fog_BackGround=10、Kagura=4、MapChip=102、
MapData=9、Picture=539、SE=273、SystemFile=32、SystemGraphic=28、cg=596、cutin=223、hscene=327、
hword=119、scene=327、word=119。

**怎么重跑**：

~~~
WOLF_DATA_ROOT=work/full cargo test -p wolf-format --test dxarchive
#   6 项：BasicData / MapData / SystemFile / scene+word 逐字节比对，外加两个头部拒绝用例
cargo test -p wolf-format --test dxarchive -- --ignored --nocapture
#   every_archive_reads：把 Data\ 下 23 个归档全部打开并逐条解出（约 2.5 秒）
~~~

游戏目录不存在时测试自动 skip（可用 `WOLF_GAME_DIR` 覆盖路径；本机是
`E:\SteamLibrary\steamapps\common\Living With SIster Monochrome Fantasy\Data`，注意目录名拼写是 SIster）。

**Shift-JIS 名字**：本作有 30 个非 ASCII 名字（FIcon 10、MapChip 11、Picture 2、SE 6、cg 1），
例如 `82 AD 82 B5 82 E1 82 AD 82 B5 82 E1 2E 70 6E 67` → `くしゃくしゃ.png`。
名字既用作查表键又用作测试比对键，所以 `sjis.rs` 必须与 Python 的 cp932 codec 逐字符一致（已单测覆盖）。

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
3. event_command 骨架 + opcode 210/211/300/111 的参数布局 + route 块 + 全 50 opcode 的参数字节大小表。
4. .mps 头部、空地图哨兵、事件、事件页（含 0x00 页尾容错）。
   **★已在 Rust 中实现并通过（crates/wolf-format/src/mps.rs + tests）：9/9 张地图全部走到 EOF。**
5. 三个数据库的容器 + type/field/data 自描述结构 + 行编码（数值在前、字符串在后）。
   **★已在 Rust 中实现并通过（database.rs）：三个库 98 个类型全部读完，含零字段类型。**
6. TileSetData.dat 容器 + 每个 tileset 的字段 + tile_passable 位域 + 6 个 tileset 的实测值。
   **★已在 Rust 中实现并通过（tileset.rs）：6/6 tileset、60,786 B 全部读完。
   注意 7.3 的位域已被实测纠正（bits 8..9 不是 0）。**
7. .wolf/DXArchive v8 全链路解包（636/636 文件逐字节校验通过）。
   **★已在 Rust 中实现并通过（crates/wolf-format/src/dxarchive/ + tests/dxarchive.rs）：
   23 个归档 / 4062 个文件 / 771,585,007 字节全部解出，636/636 逐字节相等。
   注意本章有 4 处已被实测纠正（见 8.3 / 8.4 步骤 2、4 / 8.5 的 ★ 标记）。**
8. .sav 解密算法、头部布局、checksum、fileVersion、SavePart1 前段。

### 10.2 未验证 / TODO（按优先级）

| # | 事项 | 下一步怎么做 |
|---|---|---|
| 1 | 存档正文 SavePart2..7 + VariableDatabase 的逐字段布局 | 按 docs\vendor\WolfSave\SavePartN.h 逐行移植成 Python，用 9 个存档逐个断言“读完不越界且恰好落在末字节 0x19 前” |
| 2 | 若干 opcode 的参数语义（112 / 121 / 123 / 124 / 140 / 150 / 160 / 221 / 222 / 250 / 290 / 1000） | 同 opcode 样本对齐打印推字段；或改一个参数再存盘做差分（最可靠） |
| 3 | tile_passable 的 **bits 8..9 含义**（★已实测：不是 0，携带真实数据；直方图见 7.4） | 已确认 bits 10..13 与高 16 位恒 0；剩下 bits 8..9 需要在编辑器里改通行性再存盘做差分才能定名。loader 原样保留即可 |
| 4 | .project 的类型码 / unknown1 / string_args / args / 默认值语义 | 只在做数据库编辑器时需要 |
| 5 | SysDataBaseBasic.dat / MapTree.dat / MapTreeOpenStatus.dat | loader 不需要；要做先解各自容器 header |
| 6 | 文本内嵌控制码（\cself[n]、\v[n] 等） | grep word.wolf / 文本，做转义解析器；决定 Message / Choices 显示是否正确 |
| 7 | 若干“规格写了但真实数据不触发”的分支 | 已实测未触发：STRING_INDICATOR(0x0001D4C0) 分支、页 pageTransfer 字节(features>3)、Tile::Empty(0xFFFFFFFF 出现在图块区内部)、trigger_condition == 4。按规格实现，但**不要写会被数据证伪的断言**（详见 5.4 / 5.6.1 / 6.2） |

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

## 附录 A：曾经的失败点与最终解释（5 个主要失败点 + 1 个数据坑）

| # | 失败现象 | 根因 | 最终解释 |
|---|---|---|---|
| 1 | Game.dat 的 r1/r2 读成 2097216 / 4194432，后续全错位 | 把 u16_settings_len 当**字节数**（22）而不是 **u2 个数**（22 个 u2 = 44 字节） | u8_settings_len 是字节数、u16_settings_len 是个数；filesize 在 offset 272 而非 29238 |
| 2 | TileSetData.dat 按 Kaitai 的独立容器布局解析失败；后来又以为 count 在 offset 15 | 它的容器与 database_dat **完全同构**（00 57 00 00 4F 4C + 55 + 46 4D 00 + version 0xD2 + u4 count=6）；offset 15 是第一个 title 的 t_str 长度（"街" = 4 字节含 NUL） | 6 个 tileset 全部解析成功，end 60785、footer 0xCF |
| 3 | 字符串显示成乱码，一度以为 TileSetData v210 是 Shift-JIS | 两件事叠加：(a) Windows 控制台是 GBK，Python 打印 UTF-8 会 mojibake；(b) tileset 5 的 9 个 autotile 名在原文件里本来就是坏字节 | v210 的 title / base / autotile 绝大多数是 **UTF-8**（tileset 5 的 9 项例外，见 1.4）；脚本里把 stdout 包成 UTF-8 后即可正常显示 |
| 4 | .wolf head 区解压后在约 96 字节处开始腐烂，文件名表全花 | LZ 复制写成了“取窗口最后 conbo 字节”（out += out[-index:][-conbo:]），而正确语义是**逐字节重叠拷贝** | 改成 for _ in range(conbo): out.append(out[-index]) 后 636/636 文件全部逐字节匹配 |
| 5 | 一度认为 “.wolf 必须先拿到 Game.dat 的 encryption_key” 是循环依赖，且 head 区 key 找不到 | key 不是从 encryption_key 派生，而是引擎内建版本表；cryptVersion 就写在**明文** DARC_HEAD 的 Flags 高 16 位 | Flags = 0x013A0000 ⇒ cryptVersion 0x13A = v3.14 ⇒ 用 v3.14 的 44 字节 key；只有 Name/File/Dir 三段被 XOR，才需要 key |

| 6 | 以为 TileSetData 所有字符串都是 UTF-8，写 loader 时统一 decode / re-encode | tileset 5 的 9 个 autotile 文件名在原文件里就是坏字节（双重编码残留） | 字符串一律保留原始字节做文件名比较，只在显示时 lossy UTF-8 解码，不要 re-encode（详见 1.4） |
| 7 | Rust 实现按 Kaitai 的 mps.ksy 读图块，**每个图块都取错**，但事件仍能正常解析 | mapdata_block 在 ksy 里是“像素优先”（w*h*3 交错），真实文件是**层优先**（整层 layer0 再 layer1 再 layer2）；两者字节跨度相同，所以顺序遍历照样能走到事件区，错位只在取 tile 时暴露 | 按层优先取址（详见 5.3）；这是“跨度相同掩盖了索引错误”的典型 |
| 8 | 文档里把某页的 trigger_switch 写成 0x33，与实测直方图（只有 0x10/0x11/0x20/0x21/0x31/0x50/0x51）矛盾 | 把调试脚本输出的**十进制 33** 当成了十六进制 | 真实字节是 0x21；凡是从 Python print 抄进文档的数值都要标清进制（详见 5.6） |
| 9 | 用“行数 ≤ 剩余字节 / 行最小长度”校验数据库行数，SysDatabase 直接失败 | SysDatabase 有 13 个 **property_count == 0** 的类型（类型 4/5/6/14/15..23），每行 0 字节，除零即崩 | 零字段类型只能信任 data_size2；用 checked_div + 行数 sanity cap（详见 6.2） |

（文档完。所有“已验证”项均可用 0.3 节列出的脚本重跑复现。）
