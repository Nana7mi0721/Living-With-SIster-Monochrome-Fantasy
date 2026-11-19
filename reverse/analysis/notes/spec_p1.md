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
| 引擎 | WOLF RPG Editor v3.x（DxLib + DirectX，Data/ 下 26 个 .wolf 归档） |
| 安装目录 | E:\SteamLibrary\steamapps\common\Living With SIster Monochrome Fantasy |
| 归档目录 | <安装目录>\Data\（26 个 .wolf；注意没有任何散落的 .dat） |
| 存档目录 | <安装目录>\Save\（SaveData01..08.sav、System.sav） |
| 解包参考 | work\full\<ArchiveName>\（已用 UberWolfCli.exe 解包，作为验证基准） |
| 屏幕 / 帧率 | 1024×576，tile 16px，60fps，8 方向角色图，3 帧动画，主字体 LOLITA |

**为什么必须直接读二进制**：WolfTL.exe 把 Data 转成了 JSON，但该 JSON **有损**——
丢掉了每条指令的 run_condition 附加段，指令数 124,204 vs 二进制 139,273（少约 11%）。
因此 Rust loader 必须以二进制为准，JSON 仅作人工交叉参考。

### 0.2 验证标记

- **已验证**：脚本跑过真实文件，解析末位置与文件长度、footer、内部断言全部一致。
- **未验证**：仅从参考源码或其他项目推断，或部分字段未逐字节比对。文末「待解清单」给出下一步。

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
| work\fs_sav.py + work\fs_sav_p1.py | .sav 解密、头部、校验和、SavePart1 前段 | 已验证（9/9 存档）；正文其余部分未验证 |
| work\uberwolf\WolfDec.cpp | UberWolf 参考源码（.wolf 内建 key 表，MIT） | 参考来源 |

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
