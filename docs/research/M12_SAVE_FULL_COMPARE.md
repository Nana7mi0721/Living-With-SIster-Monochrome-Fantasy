# M12 存档逐段全量对照：零售存档经游戏层 load → save 逐字节一致

> 状态：**已验证（HEAD 工作树；M12 round 1）**
> 数据根：`work/all`（23 个 `.wolf` 解包）；零售存档 `work/retail_backup/Save/SaveData01..08.sav`
> 测试：`crates/wolf-game/tests/m12_save_full_compare.rs`（2 测）
> 复现：
>
> ```bash
> cd '/e/Study/github program/imoto life'
> source tools/env.sh >/dev/null 2>&1
> WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m12_save_full_compare -- --nocapture
> ```

## 1. 结论

M12 之前，`Game::load_save` → `Game::save_to` 只在**活的字段**上一致（`save_roundtrip.rs` 比对 Picture 的 12 个字段、800 普通变量、游戏时间、选择锚点），但整份文件不是零售文件的复制：解密后比较，Part1/2/3/4、VariableDatabase、Part7 已经相同，**只有 Part5（图片表）不同**，首个差异在 Part5 偏移 68，长度差 12–23 字节。

M12 修复后，8 个零售存档**整份加密文件逐字节一致**：

| 存档 | 字节数 | 修复前 Part5 | 修复后整文件 |
| --- | ---: | --- | --- |
| SaveData01.sav | 366146 | 长度 −12，偏移 68 起不同 | **逐字节一致** |
| SaveData02.sav | 361637 | 长度 −12，偏移 68 起不同 | **逐字节一致** |
| SaveData03.sav | 364263 | 长度 −12，偏移 68 起不同 | **逐字节一致** |
| SaveData04.sav | 365071 | 长度 −15，偏移 68 起不同 | **逐字节一致** |
| SaveData05.sav | 385978 | 长度 −15，偏移 68 起不同 | **逐字节一致** |
| SaveData06.sav | 364257 | 长度 −12，偏移 68 起不同 | **逐字节一致** |
| SaveData07.sav | 364399 | 长度 −12，偏移 68 起不同 | **逐字节一致** |
| SaveData08.sav | 395566 | 长度 −13，偏移 68 起不同 | **逐字节一致** |

> 方法说明：先直接比较两份**加密**文件；Part5 变长会让其后的密文字节整体错位，所以最初 VariableDatabase / Part7 的「不同」是**流位置移动造成的假差异**。用 `wolf_save::decrypt` 解出明文后逐段比较，才得到上表：除 Part5 外全部相同。修复后 Part5 长度不再变化，整份加密文件也自然一致（密码流是位置的函数，且 `crates/wolf-save/tests/write.rs` 已证明加解密互为逆运算）。

## 2. Part5 为什么不同

`SavePart5.events` 是本作的图片表：`var1` 是图片号，`vars7_18` 存位置/尺寸/图案/透明度/混合/褪色/缩放，`vars19_36` 等尾部是端口**尚未建模**的音调/效果/动画 bank。修复前 `picture_from_record` 只恢复渲染需要的字段，`picture_to_record` 从默认记录重写：

| 字段 | 修复前行为 | 零售值 | 后果 |
| --- | --- | --- | --- |
| `vars7_18[8]`（上次淡入长度） | 加载时按「不是进行中的淡入」置 `Picture::fade = 0`，保存写 0 | 1 / 30 | 每次 load→save 丢掉历史淡入长度 |
| `vars7_18[10]`（纵向缩放） | 写 `v[9]`（统一缩放） | 例如图片 6233 为 62×100；SaveData08 1996/1998 为负横向缩放 + 100 纵向 | 纵向比例被横向值覆盖 |
| `vars19_36`、`vars55_60`、`vars61_63`、`vars64_84`、`vars85_89`、`vars91_97` | 全部写默认 0/−1 | 音调 `0x00646464`(100,100,100)、效果/动画参数 | 零售的演示状态被清零 |
| `md1`（文本图片） | 用 `decode_picture_text` 的近似结果再 `encode_picture_text` | 组合字体/间距转义 `01 04 12 1d 36` 等 | 转义被改写，长度 ±1 |

典型证据（SaveData01）：

```
part5[0] var1=-10  orig7_18=[0,0,1,1,1,255,0,MAX,1,100,100,0]
                  new 7_18=[0,0,1,1,1,255,0,MAX,0,100,100,0]
       orig19_36=[6579300,6579300,0,...,0,100,100]  new=[0,...,0]
part5[45] var1=6233 orig7_18=[...,1,62,100,0]      new=[...,1,62,62,0]
```

## 3. 修复

`crates/wolf-game/src/picture.rs`：

- `Picture` 新增 `saved_record: Option<Box<wolf_save::part5::SavePart5_1>>`。它只是**写入基线**，不参与渲染；`Picture::new` 为 `None`。

`crates/wolf-game/src/savegame.rs`：

- `picture_from_record` 把整条记录克隆到 `saved_record`。
- `picture_to_record` 从 `saved_record` 起笔（没有则默认记录），只覆盖活图片拥有的字段：`var1/var4/var5/var6/md1`、`vars7_18[0..7]`、`[9]`。
- `vars7_18[8]`：活 `fade != 0` 时写新值，否则保留记录的「上次淡入长度」。
- `vars7_18[10]`：活横向缩放仍等于记录值时保留记录的纵向缩放；脚本改了统一缩放后两项都写新值。
- `md1`：文本图片若仍等于记录解码出的文本，直接复用原 `md1` 字节流；文本被脚本改过才重新编码。
- 非文本图片的文件名字节流重编码本来就与零售一致（记录 0 的 `md1` 未变）。
- `vars19_36` 等未建模 bank 保留记录值。

这样，加载后未改动的图片会原样写回；`150` 新建/替换的图片没有基线，仍按端口默认值写（不冒充零售状态）。

## 4. 测试证明

1. `every_retail_save_round_trips_byte_for_byte_through_the_game`：8 个存档逐个 `Game::new` → `load_save` → `save_to`，读回的文件与零售原文件 `assert_eq!(written, original)`；同时断言段名序列为 Part1/2/3/4/Part5/VariableDatabase/Part7。
2. `a_moved_loaded_picture_writes_the_new_position_and_keeps_the_unmodelled_banks`：加载 SaveData01，把图片 90 的 x +7（等价于 `150 Move` 落到 `Picture::x`），保存后断言新 x 被写入、`vars19_36`/`vars55_60`/`md1` 原样保留；重新加载得到 x+7。基线不会冻结活字段。

## 5. 边界（如实）

- **只保留、不模拟**：端口目前不把 `vars19_36` 等 bank 解释成音调/效果，加载后也不对渲染施加这些音调/效果；M12 的保证是「未建模字节在 load→save 中原样保留」。如果脚本之后施加 `290` 效果并改变这些 bank，端口不会更新它们，写回的仍是加载时的旧值——这是已知限制，不是字节级回归。
- **`150` 新建的图片没有基线**：字段默认值由端口决定；本测试不声称这些默认值与零售引擎逐字节一致。
- **Part1/Part4 仍来自写入模板**：`save_to` 从 `base_save` 复制地图快照/对象表；加载零售存档时 `base_save` 就是该存档，因此 8 个存档一致。全新游戏（模板为 `Save/System.sav`）不在本对照范围内。
- 测试在缺少零售存档时跳过（版权数据）。