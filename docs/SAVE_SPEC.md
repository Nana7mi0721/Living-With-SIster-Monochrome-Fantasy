# WOLF 存档（.sav）格式规格 — 已验证实现版

本文是 `crates/wolf-save` 的规格说明。它**取代** `docs/FORMAT_SPEC.md` §9 中标注「TODO 未验证」的存档章节，
并明确纠正其中两处错误（见 §1.3、§1.4）。

所有结论必须同时满足两条来源才写进本文：

1. **参考实现**：`docs/vendor/WolfSave/`（MIT）。这是社区里唯一一份逐字节读过 WOLF 存档的 C++ 代码，
   每个 `Parse()` 都是直白的 `ReadUInt8/16/32` 序列，没有压缩、没有指针。
2. **实测**：本游戏 `Save/` 目录下的 9 个文件（`SaveData01..08.sav` + `System.sav`）。
   解析器跑完后断言「读到的位置 + 1 == 文件长度」且「最后一个字节 == 0x19」，9/9 通过。
   这条断言是 C++ 的 `parseSave()` 结尾照抄的，比任何抽样都强：段边界错一个字节就立刻失败。

标注约定：

- **已验证** — 参考实现和 9 个存档实测一致。
- **已实测** — 9 个存档实测一致，但参考实现没有对应注释解释语义（只有字节布局，没有含义）。
- **未验证** — 参考实现里有代码，但本游戏 `file_version = 0x8C` 走不到该分支，本仓库从未执行过。
  实现照抄了代码，但没有真实数据可以证明它是对的。

---

## 1. 总览

### 1.1 一个存档不是 chunk 容器

解密后，文件是**固定顺序的连续流**：

```text
[0x00 .. 0x14)  明文头（0x14 字节）
[0x14]          u8 起始标记，恒 0x19
[0x15 .. )      MemData<u16> 存档名
                u16 fileVersion
                SavePart1 -> SavePart2 -> SavePart3 -> SavePart4
                -> SavePart5 -> VariableDatabase -> SavePart7
[EOF-1]         u8 结束标记，恒 0x19
```

**没有 chunk id，没有长度前缀，没有任何索引表。** 想跳到第 N 段只能从头顺序解析。
`crates/wolf-save` 因此提供的是「伪 chunk」：`SaveChunk { id: 0..=6, name, offset, len }`，
id 就是段序号，offset/len 是解析时实测出来的真实字节范围。这样上层要按段取原始字节仍然很方便，
但不要误以为文件里存在这样的表。

### 1.2 9 个存档的实测段表

以 `SaveData01.sav`（366146 字节）为例，**已验证**：

| 段 | 起始偏移 | 长度 | 结束偏移 |
|---|---:|---:|---:|
| 明文头 + 起始标记 + 名字 + 版本 | 0x00 | 0x44 | 0x44 |
| SavePart1 | 0x000044 | 34318 | 0x008652 |
| SavePart2 | 0x008652 | 595 | 0x0088A5 |
| SavePart3 | 0x0088A5 | 218610 | 0x03DE97 |
| SavePart4 | 0x03DE97 | 1213 | 0x03E354 |
| SavePart5 | 0x03E354 | 29489 | 0x045685 |
| VariableDatabase | 0x045685 | 81851 | 0x059640 |
| SavePart7 | 0x059640 | 1 | 0x059641 |
| 结束标记 | 0x059641 | 1 | 0x059642 = EOF |

九文件里**只有 SavePart1 恒为 34318 字节、SavePart7 恒为 1 字节**；其余各段随内容变化，
不要把它们当常量（详见 §11 的断言表）。

### 1.3 纠正 FORMAT_SPEC §9.3

§9.3 的表格把正文写成「顺序 1 SavePart1 / 2 SavePart2 / … / 7 SavePart7」并列成一张表，
读起来像 chunk 序列。**顺序是对的，但「chunk」这个词是错的**：文件里没有 id、没有长度、
没有偏移。§9.3 的表格全部标了「TODO 未验证」，本文给出的是实测长度（§11）。

### 1.4 纠正 FORMAT_SPEC §9.5

§9.5 原文说：变量库「是**用 .project 自描述**的」，「纯 Rust 读取器应当**读 .project 文件**，
而不是把字段名硬编码」。

**前半句对，后半句错。** 正确的说法是：

- `Data/BasicData/CDataBase.project` 提供的是**字段名**（显示层）。
- 存档里的 `VariableDatabase` **自带**每个类型的字段配置数组（`m_typeConfig`）和每一行的全部值。
  **字节布局完全不依赖 .project**：不知道字段叫什么，照样能把 30 个类型、每个类型的每一行读出来。
- `crates/wolf-save` **不读 .project**。理由有三：字段名是可选增强；.project 的容器格式
  （u4 计数 + 嵌套 t_str）在第一个类型之后就不是纯 t_str 序列了（实测在 offset 3851979595
  处要求一个 3.8GB 的缓冲，即结构判断错误），稳定解析它需要另一份工作；而存档读取本身
  不需要它。

如果上层想要字段名，正确做法是**另外**写一个 .project 解析器，把「类型序号 → 字段名数组」
交给 `VariableType::config` 的**下标**去对齐（不是按值对齐）。实测第 0 类型的 51 个 cfg 项
（前 5 个 >= 2000 是字符串，后 46 个 < 2000 是数字）与 CDataBase.project 第 0 类型
「主人公ステータス」的 51 个字段名一一对应，**这条对应关系已验证**。

---

## 2. 解密（**已验证**，9/9）

对应 `docs/FORMAT_SPEC.md` §9.1。实现见 `crates/wolf-save/src/lib.rs` 的 `decrypt()`。

- 只有 `[0x14, EOF)` 区间被加密；头 0x14 字节是明文。
- 三个种子直接存在明文头里：`seeds = [b[0], b[3], b[9]]`。这三个字节**从不被异或**。
- 三趟，步长分别是 1、2、5：

```text
for pass, step in [(0,1), (1,2), (2,5)]:
    state = seeds[pass]
    for at in range(0x14, len(buf), step):
        state = (state * 214013 + 2531011) & 0xFFFFFFFF   # MSVC rand()
        r = (state >> 16) & 0x7FFF
        buf[at] ^= (r >> 12) & 0xFF
```

- `rand() >> 12` 在 C++ 里是 `BYTE` 截断，等价于上面取 19 位中的低 3 位。所以**每个字节只有低 3 位被翻**，
  高 5 位永远是明文 —— 这就是为什么 strings 还能从 .sav 里捞出可读文本。
- 加密是同一算法的反向：种子顺序变成 `[b[9], b[3], b[0]]`，步长 5/2/1，再重算校验和写到 offset 2。
  因此「解密 = 用反向种子顺序对同一 buffer 再 XOR 一遍」，可以原地做。

### 2.1 校验和（**已验证**，9/9）

`offset 2` 的一个字节 = **解密后** `b[0x14..EOF]` 逐字节和 `& 0xFF`。
参考实现里这是 `WolfSaveParser::fixCheckSum()`。

`crates/wolf-save` 在解析时顺带算出 `computed_checksum`，与 `stored_checksum` 比对，
`checksum_ok()` 返回结果。**9 个文件全部匹配**（见 §11 表里的 checksum 列）。

---

## 3. 明文头（**已验证**，9/9）

| 偏移 | 宽度 | 含义 |
|---|---:|---|
| 0x00 | u8 | 种子 0 |
| 0x01 | u8 | 未知，实测全 0 |
| 0x02 | u8 | 校验和（§2.1） |
| 0x03 | u8 | 种子 1 |
| 0x04 | u16 | 未知，实测全 0 |
| 0x06 | u8 | 0x55 = 字符串是 UTF-8；否则是 Shift-JIS |
| 0x07 | u16 | 未知，实测全 0 |
| 0x09 | u8 | 种子 2 |
| 0x0A | 10 字节 | 未知，实测全 0 |
| 0x14 | u8 | 起始标记，**恒 0x19**，否则整个文件拒绝 |
| 0x15 | MemData<u16> | 存档名：u16 字节数 + 载荷，末尾带 NUL |
| — | u16 | `fileVersion`，本游戏恒 **0x8C (140)**；上限 0x8E，超过即拒绝 |

实测：9 个文件全部 `b[6] == 0x55`（UTF-8）、名字 = `妹相随～黑白世界的缤纷冒险～` + NUL
（namelen = 43，含 NUL），`fileVersion = 0x8C`。因此正文从 **0x44** 开始。
文件最后一个字节**恒 0x19**。

> 注意：参考实现把 `m_header` 定为 `std::array<BYTE, 0x14>`，即 0x14 处那个字节**不属于** header。
> 实现时别把 0x14 那个 0x19 算进头里。

**未验证**：`fileVersion` 为 0x8D、0x8E 时的尾部（本游戏走不到）。代码照抄了参考实现，
但没有任何真实文件验证过。

---

## 4. SavePart1 — 地图快照与对象表（**已验证**）

`docs/vendor/WolfSave/SavePart1.h:478`。

| 字段 | 类型 | 说明 |
|---|---|---|
| `m_var1` / `m_var2` / `m_var3` | u32 ×3 | 实测恒 1 / 0 / 0 |
| `m_var4` / `m_var5` | u32 ×2 | `fileVersion >= 0x69`。地图宽高（**tile 数**），实测 64 × 36 |
| `m_var6` | u32 | **哨兵探测**。见下 |
| `m_vars1` | u32 × 3·w·h | 三个图块层，**层优先**：`[z][y][x]` |
| `m_md1` | MemData<u32> | 图块图文件名，实测 `MapChip/kiri_project.png` |
| `m_vars_140` | MemData<u32> × N | 自动图块名。N = 31（>=0x8A）/ 15（>=0x73）/ 7 |
| `m_var7` | u32 | 按 `(int)` 判 > 0；>=0x64 时跟 `m_vars2New`（u32），否则 `m_vars2Old`（u8） |
| `m_var8` | u32 | 按 `(int)` 判 > 0；跟 `m_var8` 个 `SavePart1_1` |
| `m_var9..m_var19` | u32 × 11 | `fileVersion >= 0x72` |
| `m_var20` | u32 | 按 `(int)` 判 > 0；跟 `m_vars3`（u32） |

### 4.1 哨兵 `m_var6`（**已验证**，关键坑）

参考实现的写法很绕，照抄才不会错：

```text
m_var4 = u32; m_var5 = u32
pos = 当前位置
m_var6 = u32                      // 只为探测，先读进来
if m_var6 != 0xFFFFFFFF:
    Seek(pos)                     // 回退 4 字节，把它当第一个图块重读
    for i in 0..3:                // 层
        for j in 0..m_var4:       // x
            for k in 0..m_var5:   // y
                m_vars1.push(u32)
```

即：`m_var6 == 0xFFFFFFFF` 表示**这个存档没写地图图块**，此时 `m_vars1` 为空；
否则把刚才那个 u32 当成第一个图块，共读 `3 * m_var4 * m_var5` 个。

实测 `SaveData01`：`m_var4 = 64`、`m_var5 = 36`、`m_var6 = 104444`（**不是** 0xFFFFFFFF），
所以走的是「回退 + 读 6912 个 u32」这条分支。这 6912 个值**全部等于 104444**（distinct = 1）——
整张图块层都是同一个默认图块 id。这是真实值，不是解析错位。

### 4.2 `m_vars2New`（**已实测**，9 文件一致）

`m_var7 = 192`，即 192 个 u32。distinct = 11，分布是 `0 ×103, 15 ×50, 768 ×26, 2 ×3, 4 ×3`。
这是「已开启的地图/开关类位掩码数组」。本 crate 原样保留，不解释位含义。

### 4.3 `m_vars_140` 自动图块（**已验证**）

31 个 `MemData<u32>`，实测 **15 个非空**，全部是 `MapChip/[A]..._pipo.png`：

```text
MapChip/[A]Grass1-Dirt1_pipo.png      MapChip/[A]Grass1-Grass2_pipo.png
MapChip/[A]Grass1-Grass3_pipo.png     MapChip/[A]Water2_pipo.png
MapChip/[A]Water5_pipo.png            MapChip/[A]Water7_pipo.png
MapChip/[A]WaterFall2_pipo.png        MapChip/[A]Flower_pipo.png
MapChip/[A]Snow_pipo.png              MapChip/[A]Snow_Grass4_pipo.png
MapChip/[A]Snow_Dirt2_pipo.png        MapChip/[A]Ice2_pipo.png
MapChip/[A]Dirt1-Dirt2_pipo.png       MapChip/[A]Wall-Up_Forest1_pipo.png
MapChip/[A]Wall-Up_Forest2_pipo.png
```

与 `TileSetData` 的 tileset0 的 15 个自动图块名一致 —— 这是 Part1 解析正确的旁证。

### 4.4 SavePart1_1 — 一个对象（地图事件实例）

`docs/vendor/WolfSave/SavePart1.h:172`。C++ 里字段名就是 `m_var1..m_var39`，
没有任何语义命名，所以本 crate 也保持编号（不要凭空起名）。

| 字段 | 类型 | 实测（`SaveData01` 的 objects[0]） |
|---|---|---|
| `m_var1` | u32 | 0 |
| `m_var2` | u32 | 60（对象的地图事件页/模板索引） |
| `m_var3` | u32 | 0 |
| `m_var4` | u32 | 0 |
| `m_var5` | u8 | 0 |
| `m_var6` | u32 | 0xFFFFFFFF（40 个对象全一样） |
| `m_md1` | MemData<u16> | 一个 NUL 字节 |
| `m_var7..m_var10` | u16 ×4 | 255 / 0 / 2 / 0 |
| `m_var11` / `m_var12` | u8 ×2 | 0 / 1 |
| `m_sp1` / `m_sp2` | SavePart1_1_1 ×2 | 见 4.5 |
| `m_var13..m_var18` | u16 ×6 | 全 0 |
| `m_var19..m_var21` | u8 ×3 | 0 / 0 / 0 |
| `m_var22` | u32 | 按 `(int)` 判 > 0；跟 `m_vars1`（u32）。实测恒 0 |
| `m_var23` / `m_var24` | u8 ×2 | >=0x70。实测 1 / 1 |
| `m_var25..m_var28` | u32 ×4 | >=0x73 |
| `m_var29` | u32 | >=0x78。实测 1 |
| `m_var30..m_var32` | u32 ×3 | >=0x85 |
| `m_var33` | u16 | >=0x8A，**按 `(int16_t)` 判 > 0**；跟 `m_var33` 个 `MemData<u16>` |
| `m_var34..m_var37` | u32 / u8 / u32 / u32 | >=0x8A |
| `m_var38` | u32 | >=0x8B |
| `m_var39` | u32 | >=0x8C。实测 131073 |

40 个对象的 `m_var1` 是 0..39 顺序编号（**已实测**），`m_var2` 取 60/126/2/4/6/58/8/12/14/18/20/22/24/10/64 等。

### 4.5 SavePart1_1_1 — 页表

`docs/vendor/WolfSave/SavePart1.h` 里的嵌套类。结构：

```text
6 × u8   (head)
u32      count      // > 0x10000 直接判失败
count × SavePart1_1_1_1 {
    u8  var1
    u8  n1; n1 × u32
    u8  n2; n2 × u8
}
```

实测：`head = [3, 3, 3, 0, 3, 2]`（**9 个文件、40 个对象全一样**），`count = 0`（sp1 与 sp2 都是），
所以 40 个对象的两个页表都只有 6 + 4 = 10 字节。**不要**因为 `count = 0` 就以为这段只有 6 字节。

### 4.6 SavePart1 的收尾

`m_var9..m_var19` 实测 `[0, 0, 0, 0, 100, 100, 0, 0, 0, 0, 100]`（9 文件一致），
`m_var20 = 0` 所以 `m_vars3` 为空。

### 4.7 Rust 视图

`SavePart1` 把 `m_var1/2/3` 保留原名，其余换成可读名：`map_width`、`map_height`、
`map_tiles`、`chip_image`、`auto_tiles`、`vars2_new` / `vars2_old`、
`objects`、`tail_vars`（= `m_var9..m_var19`）、`vars3`。
`SavePart1_1` 因为 C++ 里就没有语义名，Rust 也保持 `var1..var39` 编号
（`md1`、`sp1` / `sp2`、`vars1`、`mds1` 是同名直译）。

---

## 5. SavePart2 — 全局设定快照（**已验证**）

`docs/vendor/WolfSave/SavePart2.h:39`。这一段版本闸门最多，共 20 个；本游戏 `0x8C` 全部打开。
实现是逐行的直译，字段名保持 C++ 的编号。**唯一容易翻车的地方**：`m_md1..m_md9` 全是
`MemData<WORD>`（u16 长度），**只有 `m_md10` 是 `MemData<DWORD>`（u32 长度）**。

实测（`SaveData01`）：

| 字段 | 值 |
|---|---|
| `m_var3..m_var21` | `[16642, 0, 0xFFFFFFFD, 0xFFFFFFFD, 0x3F800000, 0, 1, 8, 2000, 512, 240, 0, 1, 2, 0, 0, 0, 0, 0]` |
| `m_var22..24` | `[0, 65535, 65535]` |
| `m_var29..37` | `[20, 2, 0, 0, 0xFFFFFFFF, 0, 0, 0, 255]` |
| `m_var39` | 2 |
| `m_var40/41` | 14 / 8 |
| `m_var42..45` | `[160, 68, 75, 200]`（像 RGBA 颜色） |
| `m_var46..48` | `[120, 1000000, 6]` |
| `m_var54..56` | 0xFFFFFFFF / 10 / 1 |
| `m_var58` + `m_vars2` | 5 个 `0xFFFFFFFF` |
| `m_var64..69` | `[144, 16, 145, 32, 146, 64]` |
| `m_var70` | 2（u16） |
| `m_md1` / `m_md2` | 各一个 NUL 字节 |
| `m_var71` | **每文件不同**，存档时间戳（`SaveData01 = 967479`） |
| `m_var72..74` | 100 / 100 / 100 |
| `m_md3` + `m_var75..77` | `BGM/yuuyake.mp3` + 音量/声像/循环 = 50 / 90 / 0 |
| `m_md4` + `m_var78..80` | `BGM/se_akimusi2.ogg` + 75 / 100 / 0 |
| `m_md5` | 当前背景 CG：`cg/bg_01_home_d.jpg` |
| `m_md6` | 一个 NUL 字节 |
| `m_var81` / `m_var82` | 1 / 1 |
| `m_md7..m_md9` | 各一个 NUL 字节 |
| `m_var86` | 12 |
| `m_var88/89` | 0 / 5 |
| `m_var92..95` | `[100, 100, 0, 0]` |
| `m_var96` / `m_var97` | 1 / 1 |
| `m_var98..101` + `m_vars2` | `[1, 0, 0, 0]` + 24 个 0 |
| `m_var102..105` | `[0, 14, 14, 0xFFFFFFFF]` |
| `m_md10` + `m_var106` | 一个 NUL 字节 + 5 |
| `m_mds2` | 5 个 NUL 字节 |

`System.sav` 与 `SaveData01` 的差别（**已实测**）：`m_var3..m_var21` 的第 10 个是 280 而不是 240；
`m_md3` = `BGM/se_akimusi2.ogg`、`m_md4` = `BGM/Clock-Second_Hand03-…`；
`m_var75..77` = 60/100/0、`m_var78..80` = 45/100/0。说明这一段确实是「当前播放状态」。
`m_var71` 在 `System.sav` 是 982000。

Rust 视图：版本闸门下的可选块各自成结构体 —— `block100`（`Option<SavePart2Block100>`）、
`bgm`（`Option<SavePart2Bgm>`，含 `path1/vars75_77/path2/vars78_80`）、
`block109`、`block137`、`block8a`；单字段直接平铺（`vars3_21`、`vars42_45`、
`vars46_48`、`background`、`var86` …）。未触发的闸门对应 `None` 或零值。

---

## 6. SavePart3 — 地图/事件运行时大表（**已验证**）

`docs/vendor/WolfSave/SavePart3.h:38`。这是**最大的一段**（218610 字节，占 `SaveData01` 的 60%）。

四层嵌套全部挂在有符号判定下面，**任何一层负数都会静默截断整段尾部**（参考实现就是这样，
本 crate 对「负数计数」报错而不是静默截断，因为静默截断会让后续段全部错位）：

```text
m_var1 = u32; m_var2 = u32
if (int)m_var2 >= 0:
    repeat m_var2:
        v1 = u32;  if (int)v1 < 0 -> 失败
        repeat v1:
            v2 = u8
            repeat v2: u32
    m_var3 = u32
    if m_var3 <= 0x270F:
        if (int)m_var3 > 0:
            repeat m_var3:
                v = u32; if (int)v < 0 -> 失败
                repeat v: u32
        m_var4 = u32
        if (int)m_var4 >= 0:
            repeat m_var4: MemData<u16>(<0x6F) / MemData<u32>(>=0x6F)
            m_var5 = u32
            if (int)m_var5 < 0 || m_var5 > 10000 -> 失败
            repeat m_var5: { v = u8; repeat v: u32 }
            m_var6 = u32
            if m_var6 <= 10000:
                if (int)m_var6 <= 0 -> 失败
                repeat m_var6: { v = u8; if v: repeat v: MemData<u16|u32> }
```

实测（`SaveData01`）：`m_var1 = 1179839859`（每文件不同，像时间戳/随机种子）、
`m_var2 = 2`（组数）、`m_var3 = 10`、`m_var4 = 26`、`m_var5 = 500`、`m_var6 = 500`
（`SaveData05` / `SaveData08` 的 `m_var5` / `m_var6` 是 550）。

`m_var4 = 26` 那 26 个 `MemData<u32>` 的实测内容是**星期 + 存档缩略图路径 + 章节名**：

```text
[0] "周一"  [4] "大冒险！"  [5] "../save/save1.jpg"
```

其余槽位是空串。`SaveData01` 的第 5 个是 `../save/save8.jpg`，`SaveData03` 是 `../save/save3.jpg` ——
**这就是 `Save/saveN.jpg` 是每个存档槽缩略图的证据**（缩略图文件真实存在于 `Save/` 目录，
大小 108167..123108 字节）。

`m_var5` 的 500 项每项是 `u8 count(=100)` + 100 个 u32，即一张 500 × 100 的二维表。
`m_var6` 的 500 项每项是 `u8 count(=5)` + 5 个 `MemData<u32>`（实测全是 NUL 字节）。

Rust 视图：为了不让 500×100 的嵌套变成 `Vec<Vec<u32>>` 的开销，实现把每层拆成
「计数数组 + 扁平数据数组」两件：`group_counts/group_bytes/group_words`（`m_var2` 组）、
`row_counts/row_words`（`m_var3` 行）、`list_counts/list_words`（`m_var5` 表）、
`slot_counts/slot_strings`（`m_var6` 槽）、`mds1`（`m_var4`）。
上层按计数切片即可还原嵌套结构。

---

### 6.1 三张变量表的语义（**已验证**：通常変数 = row 0，9/9）

三层「计数 + 扁平数据」里有两层是**真的变量表**，不是运行时脏数据：

| 表 | 维度 | 语义 | 已验证程度 |
|---|---|---|---|
| row_words（按 row_counts 切） | 10 行 / 835 词 | **row 0 = 通常変数 2000000+n**，800 词 | **已验证**，9/9 |
| list_words（按 list_counts 切） | 500 或 550 组 × 100 词 | 组号 = 公共事件 id，词号 = **セルフ数値 CSelf0..99** | **已验证**（§6.1 五条证据） |
| slot_strings（按 slot_counts 切） | 500 或 550 组 × 5 串 | 组号 = 公共事件 id，**串号 0 = CSelf5**、串号 4 = CSelf9 | **已验证** |

row_counts 在 9 个存档里**恒为** [800, 4, 7, 4, 1, 1, 1, 6, 1, 10]（合计 835 词），
所以 row 0 的 800 个词与通常変数 2000000..2000799 一一对应，row_words[7] 就是 2000007。
行 1..9 的含义**未确认**（实测 row 7 = [512, 288, 512, 288, 24, 19]，形似「画面中心 1024/2、576/2
+ 座標倍率 24 + 基本字号 19」的 UI 缓存，但没有任何写者能对上）。

**「通常変数 = row 0」的三条独立证据**：

1. **原作注释**：ev48（X[共]基本システム自動初期化）pc206 的注释原文是
   「System.savの通常変数0番を最終セーブデータ番号」，而 System.sav 的 row_words[0] 实测
   **= 8**，正好是最后使用的存档槽号（SaveData08.sav 存在、ev131 的 CSelf12 也是 8）。
2. **所持金**：row_words[7]（= 通常変数 2000007）在 SaveData01 / SaveData08 / System.sav
   三个文件里都是 **7586**，与 HUD 所持金的量级和「同一存档线继承」完全吻合；
   其他槽是 2254 / 8231 / 1293 / 1193 / 1503 / 15453。
3. **写入者闭环**：ev131 的 222 站点 pc230 [1600012, 1600005, 2000000, 0] 把
   CSelf12（槽号）写进 Save/System.sav 的通常変数 0 —— 落盘值 8 = CSelf12 = 8，链闭合。

**三张表的逐档实测值**（crates/wolf-save/tests/typed.rs 全部断言）：

| 文件 | 2000007 所持金 | playtime 帧 | playtime | 组数 | ev131 CSelf12 | part3 字节 |
|---|---:|---:|---:|---:|---:|---:|
| SaveData01.sav | 7586 | 16642 | 4:37 | 500 | 8 | 218610 |
| SaveData02.sav | 2254 | 1252 | 0:20 | 500 | 2 | 217708 |
| SaveData03.sav | 8231 | 13877 | 3:51 | 500 | 3 | 218472 |
| SaveData04.sav | 1293 | 16536 | 4:35 | 500 | 4 | 218466 |
| SaveData05.sav | 1193 | 16705 | 4:38 | 550 | 5 | 239821 |
| SaveData06.sav | 1503 | 7882 | 2:11 | 500 | 6 | 218462 |
| SaveData07.sav | 15453 | 11143 | 3:05 | 500 | 7 | 218574 |
| SaveData08.sav | 7586 | 16887 | 4:41 | 550 | 8 | 240000 |
| System.sav | 7586 | 16887 | 4:41 | 500 | 8 | 218650 |

恒定值（9/9 一致，可直接断言）：self_var(48, 16) == 6、self_var(48, 25) == 12、
self_var(48, 26) == 24、self_string(131, 0) == 「Save/System.sav」。
self_var(131, 12) 逐档为 8/2/3/4/5/6/7/8/8 —— SaveData01 是 8 而不是 1，所以它更像「最后一次在存读档界面里选中的槽」，而不是「本档是第几号槽」【未验证】。

**list_counts 恒为 100、slot_counts 恒为 5**（9/9 全部条目），所以切片就是
list_words[100*g .. 100*g+100] 与 slot_strings[5*g .. 5*g+5]；实现仍按累加偏移算，
以免将来遇到别的 WOLF 游戏（组大小可变）就崩。

**システム変数（9000000 段）仍然找不到落盘区**：222 写过的 2000000 / 2000546 / 2000626
里，前两个在磁盘上都有值（8 和 8），但 626 在 System.sav 里是 0，而 222 的 pc231 写的是
[2000531, 1600005, 2000626, 0]（把 531 抄进 626），磁盘上 531 = 2。这只能解释成「222 的
源操作数在 RAM 里求值，落盘的 531 是旧值」，属【未验证】推断；システム変数则完全没有候选区
（§13 保留）。

### 6.2 写回：SavePart3::to_bytes 逐字节可逆（**已验证**，9/9）

parse 的每一层都是「长度 + 数据」，所以编码器就是它的镜像，逐字段写回即可：

- SavePart3::to_bytes() —— 按 file_version >= 0x6F 的 4 字节 MemData 长度编码（本作就是 0x8C）；
  to_bytes_with_version(fv) 供更老的存档用（< 0x6F 时 MemData 长度是 u16）。
- 三条早退分支（m_var2 < 0、m_var3 > 0x270F、m_var4 < 0）与 parse 一一对应：参考实现
  在这里截断整段，编码器也停在同一位置，所以坏档也能原样吐回前缀而不是报错。
- 正常路径会校验「扁平列必须被计数完全消费」（group_bytes / group_words / row_words /
  mds1 / list_words / slot_strings 一个都不能剩），不一致时报 Error::field 而不是写出错位数据。

**实测：9 个存档的 part3 全部与 chunk_bytes(2) 逐字节相同**（上表最后一列的长度即原始长度）。
这是「改一个变量不会碰坏另外 21 万字节」的前提。

写回 API（crates/wolf-save/src/lib.rs）：

| API | 作用 |
|---|---|
| SaveFile::set_part3(&SavePart3) | 重新编码 part3 并 replace_part(PART3_ID = 2)，同时刷新 typed 视图 |
| SaveFile::playtime_frames() -> Option<u32> | 读 SavePart2::vars3_21[0]（段内偏移 **2**，因为 var1/var2 各 1 字节） |
| SaveFile::set_playtime_frames(u32) | 直接 patch part2 的 4 个字节；实测 16705 -> 4321 只改偏移 2、3 两个字节 |
| SavePart3::normal_var(n) / set_normal_var(n, v) | 通常変数 2000000 + n（越界返回 None / false） |
| SavePart3::self_var(ev, slot) / set_self_var | セルフ数値；组号 = 公共事件 id（0 基），slot = 0..99 |
| SavePart3::self_string(ev, slot) / set_self_string | セルフ文字列；**slot 0 = CSelf5**，slot 4 = CSelf9（越界/非 UTF-8 返回 None） |
| SaveFile::normal_var / set_normal_var / self_var / self_string | 上面三个的 SaveFile 门面，setter 会连带重新编码 part3 |

测试（crates/wolf-save/tests/typed.rs，6 个，全部用真实存档、目录不存在时跳过）：
every_part3_re_encodes_byte_for_byte、the_tables_hold_the_numbers_the_game_ships、
editing_a_normal_variable_survives_a_write、editing_self_state_survives_a_write、
playtime_round_trips_through_the_header、out_of_range_indexes_are_refused。

---
## 7. SavePart4 — 对象表续（**已验证**）

`docs/vendor/WolfSave/SavePart4.h:39`：

```text
SavePart1_1 (lead)
u32 n; n × SavePart1_1
u8 m_var2; u8 m_var3
u32 n; n × u32          // m_vars2
if fileVersion >= 0x8A:
    u32 n; n × u64      // m_vars3
```

实测（`SaveData01`）：`lead.m_var1 = 0xFFFFFFFE`、`lead.m_var2 = 62`、`lead.m_var39 = 131073`；
后面 5 个对象；`m_var2 = m_var3 = 0`；`m_vars2` 有 100 个 u32（首元素 8）；`m_vars3` 为空。
整段 1213 字节（部分存档 1194）。

> 注意 `lead` 是一个**独立的** `SavePart1_1`，不是数组的第一个元素。`m_var1 = 0xFFFFFFFE`
> 在这里是正常值（对比 SavePart1 的对象里 `m_var1 = 0..39`）。

---

## 8. SavePart5 — 当前画面图片表（**已验证**）

> **修正（已验证）**：本节旧标题「公共事件运行时状态」是错的。SavePart5 记录的是**存档瞬间仍然活着的 `150` 图片**，
> 不是事件协程状态；公共事件的 self 变量/字符串在 SavePart3 里。实测 SaveData01 的 66 条记录对应 66 张活图片。
> 字段语义见下表与 §8.1。

`docs/vendor/WolfSave/SavePart5.h:661`（外层）与 `:44`（`SavePart5_1`）。

外层只有两个字段：

```text
m_var1 = u16
if (m_var1 & 0x8000) == 0:
    m_var1 × SavePart5_1
```

实测 `m_var1` = 58..84（各文件不同），bit 15 从未置位，所以总能读到 `m_var1` 个条目。
**这就是「存档瞬间哪些图片还活着」的列表** —— 与公共事件总数 550 无关。

`SavePart5_1` 的字段（20 个版本闸门，本游戏全部打开）：

| 字段 | 类型 | 说明 |
|---|---|---|
| `m_var1` | u32 | 图片编号（按 i32 解释；负数是整屏层/引擎层） |
| `m_var2` / `m_var3` | u8 ×2 | 未用（实测全 0） |
| `m_var4` | u16 | anchor：0 左上 / 1 中心 / 2 左下 / 3 右上 / 4 右下 |
| `m_var5` / `m_var6` | u8 ×2 | 类型：m_var5=1 → Text；m_var6=1 → WindowFile；两者 0 → 普通文件图（无 var5=2） |
| `m_md1` | MemData<u16> | 文件名（含结尾 NUL）或文本图片的转义流 |
| `m_vars7_18` | u32 ×12 | `[x, y, 分割列/窗口宽, 分割行/窗口高, pattern(1 基), opacity, blend, -1, fade, scaleX%, scaleY%, 0]` |
| `m_vals1` | u32 ×6 | 源码里是 `m_vals1[0..3]`，实际 6 个 |
| `m_vars19_36` | u32 ×18 | >=0x69 |
| `m_var37` | u8 | >=0x6B |
| `m_var38` / `m_var39` | u8 ×2 | >=0x72 |
| `m_vals2` / `m_vals3` | u32 ×4 each | >=0x72 |
| `m_vars40_43` | u32 ×4 | >=0x73 |
| `m_vars44_48` | u32 ×5 | >=0x74 |
| `m_vars49_54` | u32 ×6 | >=0x75 |
| `m_vars55_60` | u32 ×6 | >=0x73（**注意闸门是 0x73，不是 0x75**） |
| `m_vars61_63` | u32 ×3 | >=0x76 |
| `m_vars64_84` | u32 ×21 | >=0x81 |
| `m_vars85_89` | u32 ×5 | >=0x87 |
| `m_var90` | u32 | >=0x89，子表个数 |
| 子表 | `{u32 n; n × u32}` × m_var90 | >=0x89 |
| `m_vars91_97` | u32 ×7 | >=0x89 |

`m_var90` 的子表在参考实现里是「先读一个 u32，再读那么多个 u32」，与 §6 的 `m_var5` 同构。

### 8.1 记录语义（实测，220 mode 0 / `--load-save` 共用）

- **普通文件图**（m_var5=0、m_var6=0）：m_md1 是文件名，`m_vars7_18[2]/[3]` 是**精灵表分割列/行**（1x1 = 整图），
  `[4]` 是 **1 基** pattern 单元号；picture 90 `Picture/stand_imouto_paja2.png` = `[600,80,6,2,2,255,0,-1,1,100,100,0]` 就是 6x2 表第 2 格。
- **窗口图**（m_var6=1）：`[2]/[3]` 是窗口宽高；picture 19500 = 960x168。
- **文本图**（m_var5=1）：m_md1 是 `01 <总长> <payload>` 的转义流，实测只出现 02→`\E`、04→带字距字体、
  05→字体（payload[2] = 字号+14）、06→颜色（payload[3] = 颜色号+14，0x0f 特判 `\c[0]`）；
  例如 pic 6200 = `01 05 03 0e 26 01 "21:40" 00` → `[24]21:40`。
- **fade 字段 [8] 在载入时必须置 0**：它是历史上一次淡入的时长，不是「还要再淡入」；零售 save1 的面板记录 fade=10 却已经 100% 显示。
- 写回侧：`crate::savegame::part5_from_screen` / `picture_to_record` 是读入的逆过程；`encode_picture_text` 把 `\E`/`[n]`/`\c[n]`
  重新编码成 02/05/06 字节流。唯一语义等价但非字节等价的是 retail 的 04（带字距字体）记录：读成 `[n]`，写回成 05。
- 测试：`a_saved_game_reloads_its_live_picture_layer`、`every_retail_text_picture_survives_re_encoding`、
  `a_text_picture_round_trips_through_its_save_record`、`a_file_picture_keeps_its_sprite_grid_in_the_record`。

---

## 9. VariableDatabase — 变量库（**已验证**）

`docs/vendor/WolfSave/VariableDatabase.h:281`（外层）、`:162`（`VariableType`）、
`:56`（`TypeData`）。

```text
u8  m_unknown        // 源码注释原文：「Is here 1 byte just skipped?」实测 0x6F
u32 m_typeCount      // 实测 30
m_typeCount × VariableType
```

`VariableType`：

```text
i32 m_unknown
int32 fieldCount = m_unknown
if m_unknown <= -1:
    if m_unknown <= -2: i32 m_dis
    u32 m_fieldCount; fieldCount = m_fieldCount
if fieldCount > 0: fieldCount × u32   // m_typeConfig
u32 m_typeDataCount                   // 行数
m_typeDataCount × TypeData
```

`TypeData`（**这一段最容易写错**）：

```text
第一趟：按 m_typeConfig 顺序，读全部 < 2000 的项，每项一个 i32
第二趟：按 m_typeConfig 顺序，读全部 >= 2000 的项，每项一个 MemData<u32>
最后：按字段 id 排序（std::sort，比较 ID）
```

**两趟，不是交织。** 存档里先存该行所有数字字段，再存该行所有字符串字段；参考实现读完才按 id
排回字段顺序。按交织顺序读会在第一个字符串字段处立刻错位。

实测 30 个类型的形状（`SaveData01`，dis / fieldCount / 行数 / 数字字段数 / 字符串字段数）：

```text
 0: dis=1     fc=51  rows=17   46+5
 1: dis=2     fc=100 rows=16  100+0
 2: dis=2     fc=48  rows=22   48+0
 3: dis=2     fc=100 rows=40  100+0
 4: dis=2     fc=100 rows=16  100+0
 5: dis=2     fc=100 rows=16  100+0
 6: dis=0     fc=7   rows=1    7+0
 7: dis=20002 fc=1   rows=40   1+0
 8: dis=20003 fc=1   rows=10   1+0
 9: dis=20004 fc=1   rows=10   1+0
10: dis=1     fc=16  rows=40   15+1
11: dis=2     fc=100 rows=11  100+0
12: dis=2     fc=100 rows=11  100+0
13: dis=2     fc=100 rows=11  100+0
14: dis=0     fc=4   rows=100  4+0
15: dis=0     fc=60  rows=1   60+0
16: dis=0     fc=14  rows=15  11+3
17: dis=30000 fc=21  rows=20  20+1
18: dis=0     fc=2   rows=100  1+1
19: dis=0     fc=11  rows=200  5+6
20: dis=0     fc=6   rows=1    6+0
21: dis=1     fc=9   rows=17   3+6
22: dis=0     fc=0   rows=1    0+0
23: dis=0     fc=1   rows=1    0+1
24: dis=0     fc=1   rows=1    0+1
25: dis=0     fc=9   rows=1    9+0
26: dis=0     fc=0   rows=1    0+0
27: dis=0     fc=6   rows=1    4+2
28: dis=0     fc=1   rows=1    0+1
29: dis=0     fc=1   rows=1    0+1
```

注意 `m_unknown` 实测**恒为 -2**（`0xFFFFFFFE`），所以每个类型都走 `<= -2` 分支、都读 `m_dis`。
类型 22 和 26 的 `fc = 0`（配置数组为空），这是合法的 —— 此时行数据只有字符串或什么都没有。

第 0 类型实测：`m_typeConfig[0..5] = [2000, 2001, 2002, 2003, 2004]`（字符串字段），
后面 46 项是 1000..1045（数字字段）。行 0 的实测内容：

```text
数字字段: 99, 99, 50, 50, 20, 20, 2, 1, 1, 1, 2, 90, …
字符串字段: "勇者先生", "勇者", "", "CharaChip/[Chara]Hero3_USM.png", ""
```

对照 `Data/BasicData/CDataBase.project` 第 0 类型「主人公ステータス」的 51 个字段名
（头衔1/2/3、歩行グラフィック画像、顔画像、勇气、机巧、运动、心、技、体、精神、賢さ、器用…），
前 5 个正好是 3 个头衔 + 行走图 + 脸图 —— **这条对应关系已验证**，但它需要 .project 解析器才能用上，
本 crate 不提供（§1.4）。

### 9.1 `m_dis` 的作用（**已实测**，语义未确认）

参考实现的 `TypeData::name()`：

```text
if m_dis > 1:                         return 基类名（不按行名显示）
if m_type.GetDataStr(m_idx).empty():  return 基类名
return m_type.GetDataStr(m_idx)
```

即只有 `dis <= 1` 的类型才用「行名字符串」当显示名。实测 `dis` 取值是
`0 / 1 / 2 / 20002 / 20003 / 20004 / 30000` —— 大于 1 的那些看起来是「特殊数据源标记」
（20002..20004、30000 这种大数是 WOLF 的类型引用常量）。**这些大数的确切含义未确认**，
本 crate 只原样保留 `dis`。

Rust 视图：`VariableDatabase { unknown, type_count, types }`、
`VariableType { unknown, dis, field_count, config, row_count, rows }`、
`VariableRow { index, fields }`、`VariableField { id, value }`、
`VariableValue::{ Number(i32), String(Vec<u8>) }`。
便捷访问器：`number(id)` / `string(id)` 按**字段 id**取值（不是按 cfg 下标）；
`number_field_count()` / `string_field_count()` 返回该类型的数字/字符串字段数。

写回：`VariableDatabase::to_bytes()` 见 §14.1，按格写（`set_num` / `set_text`）与
编码器必须照抄的字段顺序见 §14.5。

---

## 10. SavePart7 — 尾部小表（**已验证**）

`docs/vendor/WolfSave/SavePart7.h:40`：

```text
u8 m_var1
if m_var1 != 1: return          // 段到此结束
u32 m_var2
m_var2 × { u8 tag; tag < 0xFA ? u8 : u32 }
```

实测 9 个文件 `m_var1 = 0`，所以这一段**只有 1 字节**。若哪天读到 1，才需要继续读表。
这也是为什么 `SaveFile::chunks()` 里 SavePart7 的长度恒为 1 —— 不是漏读。

---

## 11. 测试断言表（`crates/wolf-save/tests/save.rs`）

测试用 `WOLF_GAME_DIR` 定位安装目录（默认
`E:\SteamLibrary\steamapps\common\Living With SIster Monochrome Fantasy`，
注意文件夹名里 `SIster` 的大小写），只 glob `Save/*.sav`（**不要**把 `saveN.jpg` 也算进去）。
目录不存在时 `eprintln!` 跳过而不是失败（存档是受版权保护的游戏数据，不能进仓库）。

| 文件 | 大小 | seeds | cksum | p1 | p2 | p3 | p4 | p5 | vardb | p7 | 事件数 |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| SaveData01.sav | 366146 | 136,182,95 | 0xB2 | 34318 | 595 | 218610 | 1213 | 29489 | 81851 | 1 | 66 |
| SaveData02.sav | 361637 | 241,177,136 | 0xFA | 34318 | 595 | 217708 | 1194 | 25910 | 81842 | 1 | 58 |
| SaveData03.sav | 364263 | 11,134,74 | 0x71 | 34318 | 595 | 218472 | 1194 | 27772 | 81842 | 1 | 62 |
| SaveData04.sav | 365071 | 142,176,12 | 0xEC | 34318 | 595 | 218466 | 1194 | 28586 | 81842 | 1 | 64 |
| SaveData05.sav | 385978 | 184,10,18 | 0x3A | 34318 | 595 | 239821 | 1194 | 28138 | 81842 | 1 | 63 |
| SaveData06.sav | 364257 | 123,1,39 | 0xE0 | 34318 | 595 | 218462 | 1213 | 27748 | 81851 | 1 | 62 |
| SaveData07.sav | 364399 | 134,201,14 | 0xB9 | 34318 | 595 | 218574 | 1213 | 27778 | 81851 | 1 | 62 |
| SaveData08.sav | 395566 | 141,100,148 | 0xD8 | 34318 | 617 | 240000 | 1213 | 37497 | 81851 | 1 | 84 |
| System.sav | 374216 | 186,159,232 | 0xA4 | 34318 | 617 | 218650 | 1213 | 37497 | 81851 | 1 | 84 |

九文件**恒定**、可直接断言的内部值：

- `header.start_byte == 0x19`、`end_byte == 0x19`、`file_version == 0x8C`、`is_utf8 == true`、
  `name == "妹相随～黑白世界的缤纷冒险～"`、`checksum_ok()` 为真。
- `part1.var1 == 1`、`var2 == 0`、`var3 == 0`、`map_width == 64`、`map_height == 36`、
  `map_tiles.len() == 6912`、`chip_image == "MapChip/kiri_project.png"`、
  `auto_tiles.len() == 31`（15 个非空）、`vars2_new.len() == 192`、
  `objects.len() == 40`、`vars3` 为空、`tail_vars == [0,0,0,0,100,100,0,0,0,0,100]`。
- `part1.objects[0].sp1.head == [3,3,3,0,3,2]`、`sp1.count == 0`、`sp2.count == 0`。
- `part2.var39 == 2`、`block100.var55 == 10`、`var86 == 12`、`var96 == 1`、`var97 == 1`。
- `part3.var2 == 2`、`var3 == 10`、`var4 == 26`；`var5` / `var6` 是 500 或 550。
- `part4.lead.var1 == 0xFFFFFFFE`、`part4.objects.len() == 5`、`part4.var2 == 0`、`part4.var3 == 0`。
- `vardb.unknown == 0x6F`、`vardb.type_count == 30`、`vardb.types[0].unknown == -2`、
  `vardb.types[0].dis == 1`、`vardb.types[0].config.len() == 51`、
  `vardb.types[0].number_field_count() == 46`、`string_field_count() == 5`。
- `part7.var1 == 0`、`part7.entries` 为空。

**只有 SavePart1 的 34318 字节和 SavePart7 的 1 字节是跨文件常量**；`p2` 有 595 / 617 两种，
`p3/p4/p5/vardb` 每个文件都不同。把它们当常量写死会在第二个存档上失败。

运行：

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh
cargo test -p wolf-save -- --nocapture
```

实测输出 `checked 9 save file(s)`，1 passed。

### 11.1 写出与变量表的断言（tests/write.rs / tests/typed.rs）

tests/write.rs（6 个）：9 档 serialize() / to_bytes() 与**原始文件逐字节相同**、改档名后头部重建
（SaveData01 366146 -> 366108）、中间段变长（part3 +10 字节）后各段偏移正确平移、替换尾段后可重新解析、
未知段 id 被拒。

tests/typed.rs（6 个）：9 档 part3 与 chunk_bytes(2) 逐字节相同；逐档断言 §6.1 的表值
（所持金 / playtime / 组数 / ev131 槽号 / part3 长度）；改通常変数 2000007 后写出再解析能读回且
playtime 与セルフ值不变、checksum_ok() 为真；改セルフ数値与セルフ文字列后读回；
改 playtime 只动 part2 的偏移 2、3；越界索引返回 None / false。

---

### 11.2 全文件 rebuild 与逐段编码器（`tests/rebuild.rs`，8 个）

⚠️ 本仓库现在有**两个**写出入口，语义不同，别混用：

| 段 | `serialize()`（旧） | `rebuild()`（新，task-28 / task-30） |
|---|---|---|
| SavePart1 | 复制解析时的原始字节 | `SavePart1::to_bytes(file_version)` |
| SavePart2 | 复制 | `SavePart2::to_bytes(file_version)` |
| SavePart3 | 复制 | `SavePart3::to_bytes_with_version(file_version)` |
| SavePart4 | 复制 | `SavePart4::to_bytes(file_version)` |
| SavePart5 | 复制 | `SavePart5::to_bytes(file_version)` |
| VariableDatabase | 复制 | `VariableDatabase::to_bytes()`（task-30） |
| SavePart7 | 复制 | **复制**（1 字节，尚无编码器） |

也就是说：改 `part1..part5` 或变量库（vardb）里任一 typed 字段，`rebuild()` / `to_bytes_rebuilt()` 会写进文件；
只有 SavePart7 仍然只能走 `replace_part`。

| 测试名 | 断言的关键数字 |
| --- | --- |
| `every_retail_save_rebuilds_to_the_same_bytes` | 9 个零售存档：`parse -> rebuild -> encrypt` 与原始文件**逐字节相同**（长度与内容，首个不同偏移会被打印） |
| `rebuild_matches_serialize_and_to_bytes` | 同一份内存状态的两条写出路径必须一致：`serialize() == rebuild()`，且两者长度都等于原文件、`to_bytes() == 原文件` |
| `each_segment_encoder_reproduces_its_parsed_bytes` | 逐段比对：9 档 × **6 段**，每个 `to_bytes()` 都等于 `chunk_bytes(id)`（part1 34318 / part2 595 或 617 / part3 217708..240000 / part4 1194 或 1213 / part5 25910..37497 / **vardb 81842 或 81851**） |
| `mutations_in_part2_and_part5_survive_a_rebuild` | 改 part2 `vars3_21[0]`（playtime）+1234、改 part5 某个事件的 `vars7_18[0] = 0x5A5A5A5A`：写出再解析都能读回，`checksum_ok()` 为真，事件 id 与名字不变；把同一个字段改回原值后输出与原文件逐字节相同 |
| `a_synthetic_variable_database_round_trips_through_to_bytes` | 手搓一个变量库，四种 `m_unknown` 形状（正数 / -1 / -2 / -3）每种都写一遍：`to_bytes() -> parse_bytes()` 与原值相等，再编码一次字节不变 |
| `a_mutated_vardb_number_survives_a_full_rebuild` | 零售 SaveData01 第一个数值单元格 +1：`set_vardb_num` 后 `to_bytes_rebuilt()` 再解析读回新值、类型数不变、`checksum_ok()`；改回原值后与原始文件逐字节相同 |
| `a_mutated_vardb_text_survives_a_full_rebuild` | 第一个文本单元格写成 `書き込みテスト`（自动补 NUL）：写出再解析 `text()` 读回同一串；再写回原字节，读回原值 |
| `vardb_setters_refuse_what_they_cannot_encode` | 四种非法写（数值写到文本字段、文本写到数值字段、类型/行/字段下标越界）全部返回 false 且**不改变**数据库；合法写能落到 `to_bytes()` 里 |

**为什么「逐字节相同」能证明编码器完整**：`parse()` 对 9 个文件都恰好停在 `EOF-1`（§1.1 的收尾断言），
所以 9 个文件里没有任何「解析器跳过的字节」。于是 `encode(parse(x)) == x` 意味着每个字段的
**顺序、宽度、有无（版本闸门）** 都写对了 —— 少写一个字段会让后面所有字节错位，多写一个字段同理。
本作的 `file_version = 0x8C` 打开了 SavePart1/2/4/5 的**全部**版本闸门，所以这条断言覆盖了所有
`>= 0x8A` 的分支；`<= 0x89` 的老版本分支仍然只有「照抄参考实现」这一条证据（§13）。
变量库的 9/9 逐字节另有单段证据（`each_segment_encoder_reproduces_its_parsed_bytes` 现在覆盖 6 段）。

```bash
cargo test -p wolf-save --test rebuild -- --nocapture   # 8 passed
```

---

## 12. 实现坑点汇总（都是实际踩过的）

1. **`m_md1..m_md9` 是 u16 长度，只有 `m_md10` 是 u32 长度**（§5）。全按 u32 读会在
   `0x8652` 处直接跑飞。
2. **所有计数字段按 `(int)` 判正负**，不是按无符号。`0xFFFFFFFF` 意味着「没有」，不是「四十亿」。
   涉及 SavePart1 的 `m_var7/m_var8/m_var20`、SavePart3 的 `m_var2/m_var3/m_var4/m_var5/m_var6/m_var90`、
   SavePart5 的 `m_var90`、SavePart1_1 的 `m_var33`（这一处是 `int16_t`）。
3. **VariableDatabase 的行数据是「先全部数字，再全部字符串」两趟**（§9）。按字段交织读必错。
4. **SavePart1 的哨兵要回退 4 字节重读**（§4.1），不能「先读进 m_var6 就跳过」。
5. **`MemData` 的长度是字节数**，不是元素数；长度为 0 时一个字节都不读；
   长度 <= 1 时 `toString()` 返回空串（只有 NUL 终止符）。
6. **`Save/` 目录里还有 `save1.jpg..save8.jpg`**，glob 必须只取 `*.sav`，否则会在 jpg 上解析失败。
7. **段边界没有魔数保护**，唯一的收尾校验是「位置 + 1 == 长度」且「末字节 == 0x19」。
   任何一段读错都会在这条断言上暴露 —— 建议照抄这条断言，不要只信任段长。
8. **SavePart3 的四张表都是「计数 + 扁平数据」两件**，取第 i 组/行/槽必须先把前面所有计数加起来；
   不要假设组大小恒定（本作 list_counts 恒 100、slot_counts 恒 5，但编码器与存取器仍按累加算）。
9. **SavePart3 写回时 `MemData` 的长度宽度随 `file_version` 变**（`< 0x6F` 是 u16），所以
   `to_bytes()` 只对 `>= 0x6F` 有效（本作 0x8C）；别的版本走 `to_bytes_with_version(fv)`。
10. **改 playtime 有两条等价路径**：①直接 patch part2 段内偏移 2 的 4 个字节再 `replace_part(PART2_ID)`
    （`set_playtime_frames` 就是这么做的，实测只改 2 个字节）；②改 `part2.vars3_21[0]` 再 `set_part2(&part2)`，
    它会用 §14 的 `SavePart2::to_bytes` 重编码整段 —— 两条路径产出的字节完全相同（`tests/rebuild.rs` 的
    `mutations_in_part2_and_part5_survive_a_rebuild` 就是拿这两条互证）。

---

## 13. 未确认项清单

| 项 | 状态 |
|---|---|
| 明文头 offset 0x01 / 0x04 / 0x07 / 0x0A..0x13 的含义 | **未确认**。实测全 0，可能是保留位 |
| `fileVersion` 0x8D / 0x8E 的尾部结构 | **未验证**。代码照抄参考实现，无真实数据 |
| SavePart1 的 `m_vars2New` 位掩码每一位的含义 | **未确认**。只知道是 192 个 u32 |
| SavePart2 各字段的语义（除 BGM/背景/时间戳） | **部分未确认**。字段值是实测的，含义多为推测 |
| SavePart3 的 `m_var1` 到底是什么 | **未确认**。每文件不同，形似时间戳或随机种子 |
| SavePart4 的 `lead.m_var1 = 0xFFFFFFFE` 的含义 | **未确认**。只知道是稳定值 |
| SavePart5_1 各字段的语义 | **未确认**。只知道事件 id 和 `m_md1` 是图片路径 |
| `m_dis` 取值 20002/20003/20004/30000 的含义 | **未确认**。形似 WOLF 类型引用常量 |
| `Data/BasicData/CDataBase.project` 的完整容器格式 | **未确认**。只验证了「30 个类型 + 字段名」的头部；第一个类型之后结构不同 |
| SavePart3 的 row 1..9（4/7/4/1/1/1/6/1/10 词）分别是什么 | **未确认**。row 7 = [512,288,512,288,24,19] 形似 UI 缓存，但找不到写者 |
| システム変数（9000000 段）的落盘位置 | **未确认**。逐段排查无候选区；222 写过的 2000546 在磁盘上有值（8），2000626 却是 0 |
| 222 的源操作数是在 RAM 还是文件里求值 | **未验证**。磁盘 531 与 626 不符，怀疑 222 用 RAM 值写入 |

---

## 14. 逐段编码器（write side，**已验证** 9/9 逐字节）

### 14.1 现有编码器清单

| 类型 | 入口 | 版本闸门处理 |
|---|---|---|
| `SavePart1` | `to_bytes(file_version)` | 全部（`>=0x69` 地图、`>=0x72` 尾 11 词、`>=0x8A` 31 个自动图块） |
| `SavePart1_1` | `to_bytes(file_version)` | 全部（`>=0x70` / `0x73` / `0x78` / `0x85` / `0x8A` / `0x8B` / `0x8C`） |
| `SavePart1_1_1` | `to_bytes()` | 无闸门 |
| `SavePart1_1_1_1` | `to_bytes()` | 无闸门 |
| `SavePart2` | `to_bytes(file_version)` | 全部（20 处） |
| `SavePart3` | `to_bytes()` / `to_bytes_with_version(fv)` | `MemData` 长度宽度随 `fv < 0x6F` 变 |
| `SavePart4` | `to_bytes(file_version)` | `>=0x8A` 的 `m_vars3`（u64 表） |
| `SavePart5` / `SavePart5_1` | `to_bytes(file_version)` | 全部（20 处） |
| `VariableDatabase` | `to_bytes()` | 无版本闸门（字段顺序见 §14.5） |
| `SavePart7` | **无编码器** | `rebuild()` 直接复制原始字节（1 字节） |

`SaveFile` 层新增：

```rust
impl SaveFile {
    pub fn rebuild(&self) -> Result<Vec<u8>>;            // 明文、typed 重编码
    pub fn to_bytes_rebuilt(&self) -> Result<Vec<u8>>;   // rebuild + encrypt
    pub fn set_part1(&mut self, part: &SavePart1) -> Result<()>;
    pub fn set_part2(&mut self, part: &SavePart2) -> Result<()>;
    pub fn set_part4(&mut self, part: &SavePart4) -> Result<()>;
    pub fn set_part5(&mut self, part: &SavePart5) -> Result<()>;
    pub fn set_vardb(&mut self, vardb: &VariableDatabase) -> Result<()>;          // task-30
    pub fn set_vardb_num(&mut self, t: usize, r: usize, f: usize, v: i32) -> Result<bool>;
    pub fn set_vardb_text(&mut self, t: usize, r: usize, f: usize, s: &str) -> Result<bool>;
}
pub const PART1_ID: u32 = 0;   // 已有 PART2_ID = 1 / PART3_ID = 2
pub const PART4_ID: u32 = 3;
pub const PART5_ID: u32 = 4;
pub const VARDB_ID: u32 = 5;
pub const PART7_ID: u32 = 6;
```

五个 `set_partX` 与 `set_part3` 同构：编码 → `replace_part` → 覆盖 typed 视图。
所以一次编辑只重写一段，其余段只是偏移平移。变量库的单格写入细节见 §14.5。

### 14.2 编码器里三个必须照抄的反向细节

1. **SavePart1 的 `m_var6` 哨兵要写回，不是写 0**（§4.1）。`map_tiles` 为空时输出的是单个
   `0xFFFFFFFF`，后面直接跟 `m_md1`。若写成 `0`，解析器会把 0 当成第一格贴图，整段错位。
2. **SavePart1_1 的 `m_mds1` 计数是 int16 判正**（§12 第 2 条）。编码器把上限卡在 `i16::MAX`，
   因为 `0x8000..=0xFFFF` 在解析侧会被读成「没有」—— 允许写出这种值等于制造一个读不回来的存档。
3. **SavePart5 的 `m_var1` 既是计数又是「没有表」的标志**（bit 15）。`0x8000` 置位时后面必须
   一个事件都不写；未置位时 `var1` 必须等于 `events.len()`。编码器对两个方向都报错而不是静默修正。
   同理 `SavePart5_1::m_var90`（子表个数）本身就是计数，**不是**「先写 var90、再写一遍个数」。

### 14.3 9 档逐字节证明

`tests/rebuild.rs` 对 `work/retail_backup/Save`（或 `WOLF_GAME_DIR/Save`）下的
`SaveData01..08.sav` + `System.sav` 逐个做 `parse -> rebuild -> encrypt`，与磁盘文件比较：

| 文件 | 大小 | rebuild 结果 |
|---|---:|---|
| SaveData01.sav | 366146 | 逐字节相同 |
| SaveData02.sav | 361637 | 逐字节相同 |
| SaveData03.sav | 364263 | 逐字节相同 |
| SaveData04.sav | 365071 | 逐字节相同 |
| SaveData05.sav | 385978 | 逐字节相同 |
| SaveData06.sav | 364257 | 逐字节相同 |
| SaveData07.sav | 364399 | 逐字节相同 |
| SaveData08.sav | 395566 | 逐字节相同 |
| System.sav | 374216 | 逐字节相同 |

### 14.4 与「opcode 220 mode 0（运行中整档保存）」的关系

这条链已经完全打通：VM 侧持有 `SavePart1..5` 与 [VariableDatabase] 的 typed 值 → `set_partX` /
`set_vardb`（或按格写入的 `set_vardb_num` / `set_vardb_text`）→ `to_bytes_rebuilt()` →
写到 `Save/SaveData%02d.sav`。`crates/wolf-game/src/game.rs` 的 `Game::save_to` 已经把运行中的
`Game` 接上这条链：以「本局载入的存档」或新游戏的 `Save/System.sav` 为模板，写入 playtime、
Sys3/Sys4、800 个通常変数、全部活着的公共事件 `self` 数值变量（0..99）与 `CSelf5..9` 字符串。

task-30 + 引擎侧接线之后，这条链已完整：`Game::save_to` 在 `set_part5` 之后调用
`Databases::write_into(&mut save.vardb)`，把运行时的 CData 单元格写进变量库再 `to_bytes_rebuilt()`；
`missed > 0` 时会写一条 `220 SaveLoad` 诊断而不是静默丢弃。测试 `a_saved_game_keeps_the_database_cells`
（CData (0,12,5)=4242 → 保存 → 载入后仍为 4242）覆盖了这条接线。存档库这一侧的接口见 §14.5。

### 14.5 VariableDatabase 编码器与按格写入（task-30，**已验证** 9/9 逐字节）

```rust
impl VariableDatabase {
    /// 整段编码，镜像 vardb::parse 的读取顺序。
    pub fn to_bytes(&self) -> Result<Vec<u8>>;
    /// 单独解析一段 vardb 字节（测试与工具用）。
    pub fn parse_bytes(bytes: &[u8]) -> Result<VariableDatabase>;

    pub fn type_of(&self, type_index: usize) -> Option<&VariableType>;
    pub fn row(&self, type_index: usize, row_index: usize) -> Option<&VariableRow>;
    pub fn number(&self, type_index: usize, row_index: usize, field_index: usize) -> Option<i32>;
    pub fn string(&self, type_index: usize, row_index: usize, field_index: usize) -> Option<&[u8]>;
    pub fn text(&self, type_index: usize, row_index: usize, field_index: usize) -> Option<String>;
    pub fn is_number_field(&self, type_index: usize, field_index: usize) -> Option<bool>;

    /// 写一个数值/文本单元格；字段类型不符或下标越界时返回 false 且不改任何东西。
    pub fn set_num(&mut self, type_index: usize, row_index: usize, field_index: usize, value: i32) -> bool;
    pub fn set_string(&mut self, type_index: usize, row_index: usize, field_index: usize, body: &[u8]) -> bool;
    /// 同上，但自动补上游戏存储用的尾部 NUL。
    pub fn set_text(&mut self, type_index: usize, row_index: usize, field_index: usize, text: &str) -> bool;
}

impl SaveFile {
    pub fn set_vardb(&mut self, vardb: &VariableDatabase) -> Result<()>;
    pub fn set_vardb_num(&mut self, type: usize, row: usize, field: usize, value: i32) -> Result<bool>;
    pub fn set_vardb_string(&mut self, type: usize, row: usize, field: usize, body: &[u8]) -> Result<bool>;
    pub fn set_vardb_text(&mut self, type: usize, row: usize, field: usize, text: &str) -> Result<bool>;
}
```

写盘时必须照抄的三个反向细节：

1. **一行的写入顺序是「先全部数值字段，再全部文本字段」，都按 config 顺序**（§9）。内存里 `fields`
   是按 id 排序的，所以编码器走 `config` 而不是 `fields`——按 `fields` 顺序写会得到错误的字节序。
2. **`m_unknown` 有三种形状**：正数时它自己就是字段数（不写 `m_dis`、不写 `m_fieldCount`）；
   `-1` 时写 `m_fieldCount`；`<= -2` 时先写 `m_dis` 再写 `m_fieldCount`。本作 30 个类型全是 `-2`，
   另外两种只有合成测试覆盖。
3. **写不进去的字段必须报错而不是丢掉**：`VariableRow::to_bytes` 会检查每个字段 id 都在 config 里，
   否则 `Error::field`——静默丢字段会写出一份「解析回来比原来少几列」的档。
