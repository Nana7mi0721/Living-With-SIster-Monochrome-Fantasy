# M7 · 存档 SavePart 覆盖与窗口皮肤（SAVE & WINDOW）

日期：M7 · 仓库 HEAD 1d36ebe · 归属任务：存档/窗口切片

## 结论（先看这个）

两个切片都已完成，并且都是「先审计、再用回归钉死」：

1. **存档 SavePart 覆盖**：`crates/wolf-game/src/savegame.rs` 的 `apply` 会把 Part2/Part3/Part5 与 vardb 主动读回内存；Part1（地图快照 + 对象表）与 Part4（对象表续）不由运行时维护，而是由「写档模板」原样带入。审计中发现**一个真正的缺口**：`load_into` 读完档后**没有更新 `game.base_save`**，于是「从某个存档 load → 立刻 save」会用上一个模板（新游戏时是 `Save/system.sav`）的 Part1/Part4 覆盖刚读入的存档，**丢掉该档的对象表**。已在 `load_into` 中补一行 `game.base_save = Some(save);`，并在 `crates/wolf-game/tests/m7_save_parts.rs` 用 2 个用例钉死（修复前第二个用例必红，修复后转绿）。
2. **窗口皮肤**：`work/full/SystemFile/WindowBase.png` 实测确为 18×18 的灰色面板皮肤，`crates/wolf-game/src/window.rs` 的 `nine_slice` 在真实消息框（960×168）上完整铺满、3px 边框、中间拉伸。**H-WIN-1 的假说基本正确，并非过时**；`SystemGraphic/WindowBase.png` 确实是无人引用的深蓝样例。`window.rs` 无需改动，本次只新增回归 `crates/wolf-game/tests/m7_window_skin.rs`（3 个用例）与本文档。

最终整套测试：`cargo test -q -p wolf-game` → **291 passed; 0 failed; 0 ignored**。

## 代码路径

- 读档：`crates/wolf-game/src/savegame.rs` 的 `load_into(game, path)` → `SaveFile::parse` → `apply(game, &save)`；`game.rs` 的 `load_save` 与 220 的 `SaveRequest::Load` 都走它。
- 写档：`crates/wolf-game/src/game.rs` 的 `Game::save_to(&mut self, path)`：克隆 `base_save` 作模板 → 改写 Part2（playtime/Sys3/Sys4）、Part3（800 普通变量 + 自伤变量/字符串）、Part5（`part5_from_screen`）、vardb（`databases.write_into`）→ `to_bytes_rebuilt` 落盘，并把 `base_save` 设为刚写的档。
- `apply`（savegame.rs:77）：读取 `part3.row_words[..800]` → `vm.vars.set`；`databases.apply_saved_database`；`vars3_21[9]/[10]` → `set_sys(3/4)`；`vars3_21[0]` → `set_playtime_frames`；`screen.clear()` 后遍历 `part5.events` 用 `picture_from_record` 重建图片层。

## 覆盖到的 SavePart（1..5 全审计）

| Part | 内容 | 谁写 | 谁读 | 回归断言 |
|---|---|---|---|---|
| Part1 | 地图快照 64×36 + 对象表 | `save_to` 不动，来自 `base_save` 模板 | `apply` 不读（运行时不需要） | `written.part1 == template.part1`；修复后零售档 `written.part1 == source.part1` |
| Part2 | `vars3_21`：[0]=游玩帧，[9]=Sys3，[10]=Sys4 | `save_to` 写 | `apply` 读 | `part2[0]=987654`、`[9]=700`、`[10]=355` |
| Part3 | row0 的 800 个普通变量 + 自伤变量/字符串 | `save_to`（`set_normal_var` 等） | `apply`（`vm.vars.set`） | `part3.normal_var(i) == vars_before[i]`（含负值） |
| Part4 | 对象表续（`lead`/`objects`/`vars2`/`vars3`） | **只能来自 `base_save`**，`save_to` 不重建 | `apply` 不读 | 修复后零售档 `written.part4 == source.part4` |
| Part5 | 150 图片表（`var1` 计数 + `events`） | `save_to`（`part5_from_screen`） | `apply`（`picture_from_record`） | `part5.events.len()==3`，读回后图片快照逐字段相等 |
| vardb | DB（CData）单元格 | `save_to`（`write_into`） | `apply`（`apply_saved_database`） | `num(CData,0,0,5)==4242`、`text(CData,0,0,0)==テスト名前` |

关键事实：`docs/SAVE_SPEC.md` §10 记录 **Part1 在全部 9 个存档里是常量（34318 字节），Part4 会变（1194 / 1213）**。所以「用 System.sav 当模板」对 Part1 无影响，但会**静默替换掉被读存档的 Part4**——这正是下面缺口的要害。

## 缺口：`load_into` 不更新写档模板

- `Game.base_save` 的文档（game.rs:304-309）写明它是「本局最近一次读入的存档，作为 220 mode 0 的模板」；`save_to` 的文档也说模板是「本局读入的那个存档，新游戏则是 `System.sav`」。
- 但修复前 `load_into` 只做 `Ok(apply(game, &save))`，从不回写 `game.base_save`。后果：`load(A) → save(B)` 实际用 A 之前的模板（通常是 `System.sav`）拼 Part1/Part4，B 的 Part4 变成 `System` 的 Part4，**A 的对象表续段丢失**。
- 修复（savegame.rs:70-82）：

```rust
let save = SaveFile::parse(&bytes).map_err(|e| e.to_string())?;
let loaded = apply(game, &save);
// 读入的存档从此成为写档模板：Part1 地图快照与 Part4 对象表续
// 运行时并不维护，必须由刚读入的这一份提供，否则会被旧模板覆盖。
game.base_save = Some(save);
Ok(loaded)
```

- 安全性：`apply` 的唯一调用点就是 `load_into`，因此在此处设定 `base_save` 不会影响其它路径；`game.rs` 既有的 4 个存档测试都是手动设 `base_save`，不受影响（实测仍全绿）。
- 回归证明：`m7_save_parts.rs` 第二个用例遍历零售存档 `SaveData01..08`，修复前对整个仓储断言 `written.part4 == source.part4` 会红；修复后转绿。实测打印：`round-tripped 8 retail save(s); 6 had a Part4 distinct from System.sav` —— 8 个档里 6 个的 Part4 与 System.sav 不同，说明这是真实且高频的丢失，不是理论问题。

## 窗口皮肤验证

### 资产实测（work/full/SystemFile/WindowBase.png，18×18，RGBA）

逐像素网格（Pillow / `image` crate 一致）：

- 四角 `(0,0)`、`(17,17)` 为透明 `(255,255,255,0)`；
- 外框暗线 `0x424242 = (66,66,66,255)` 落在第 0 行/列（如 `(0,1)`）；
- 1px 高光 `0x888888 = (136,136,136)` 在 `(1,1)`，`0xf2f3f3 = (242,243,243)` 在其内侧 `(1,2)`/`(2,1)`；
- 过渡线 `0xd9dcdd = (217,220,221)` 在 `(2,2)`，`0x576062 = (87,96,98)` 在 `(2,3)`/`(3,2)`；
- 3..14 行为灰底 `0x1f2425 = (31,36,37)`、alpha `0xdf`。

这正是「3px 边框 + 拉伸中间体」的灰色面板，`docs/UNVERIFIED.md` H-WIN-1 里列出的 `0x424242 / 0xf2f3f3 / 0x576062` 三个常量在资产上都真实存在（只是落在不同环坐标上），**H-WIN-1 的皮肤判定正确**。

### 真实消息框几何

- `db.rs` 记录 CE63 把消息框放在 `(64,408)`、`960×168`；`Picture 19500`（WindowFile，`window=(960,168)`，锚点居中 `x=512,y=480`）的 `destination` 实测为 `[32.0, 396.0, 960.0, 168.0]`。
- `window::nine_slice([32,396,960,168], 18, 18)` 产出 9 片：左上 `[32,396,3,3]`、中心 `[35,399,954,162]`、右下 `[989,561,3,3]`；目的面积合计 `960*168`，源 18×18 的 324 texel 全部恰好使用一次。

### H-WIN-1 状态判定

- **确认**：`SystemFile/WindowBase.png` 是默认皮肤（18×18、3px 框、中段拉伸），`SystemGraphic/WindowBase.png` 是无人引用的深蓝样例（body 如 `(5,5)=(3,6,75,204)`、`(17,28,115,204)`）。
- **仍未验证**：原版实机截图（本机抓屏只抓到开场过场，游戏不接受合成按键）——与 H-WIN-1 记录一致。
- **已知矛盾仍在**：type3 的 `#12005` 框 0×0（无框尺寸无法九宫格，按设计只画内容）；CN 汉化补丁 `#12501` 文字 `(715,115)` 与窗口 `#12500` `(855,200)` 300×200 不嵌套、右缘 1155>1024 被裁。这些是脚本/补丁排版问题，**本次不改坐标、不自动嵌套、不改视觉**，只记录证据。
- 说明：`docs/UNVERIFIED.md` 不在本任务允许改动的文件清单内，故此处仅记录证据，未修改该文档。

## 回归测试

### `crates/wolf-game/tests/m7_save_parts.rs`（2 个用例）

- `every_save_part_a_played_game_writes_round_trips`：真 `Game`（work/full）→ 写 800 个普通变量（含负值）、`playtime=987654`、`sys3=700/sys4=355`、一个 DB 数值/文本单元格、3 张图片（Text/File/WindowFile）→ `save_to` 私有临时目录 → 断言写入档的 Part1/Part4 与模板一致、Part2/Part3/Part5 各字段正确 → 改动内存后重新 `Game::new + load_into` 读回 → 断言 `LoadedSave{pictures:3, normal_vars:800, playtime_frames:987654, choice_anchor:(700,355)}`、变量快照/图片快照/playtime/sys/DB 单元格全部一致。
- `a_loaded_retail_save_keeps_its_object_tables_through_a_save`：遍历 `work/retail_backup/Save/SaveData01..08.sav`，`Game::new → load_into → save_to → read_save`，断言 `written.part1 == source.part1` 且 `written.part4 == source.part4`；统计 Part4 与模板不同的档数（实测 6/8），断言 `checked>0 && part4_varied>0`。**修复前此用例必红**。

### `crates/wolf-game/tests/m7_window_skin.rs`（3 个用例）

- `the_default_skin_is_a_grey_18px_asset`：读真实资产，断言 18×18 与关键像素（透明角、`0x888888`、`0xd9dcdd`、body `0x1f2425` alpha `0xdf`）。
- `the_navy_sample_asset_is_not_the_default_skin`：断言 `window::SKIN == SystemFile/WindowBase.png == db::WINDOW_SKIN_FILE` 且 `!= SystemGraphic/WindowBase.png`；深蓝样例 body 像素满足 blue>red && blue>green && blue>60。
- `the_message_window_box_is_covered_by_the_18px_nine_slice`：`Picture 19500`（window 960×168、锚点居中）→ `destination == [32,396,960,168]` → `nine_slice` 9 片、3px 角、中心 `[35,399,954,162]`、目的面积合计 `960*168`、全部落在框内、源面积 `18*18`。

## 运行命令与结果

```
cargo test -q -p wolf-game --test m7_save_parts       # 2 passed; 0 failed
cargo test -q -p wolf-game --test m7_window_skin      # 3 passed; 0 failed
cargo test -q -p wolf-game                            # 291 passed; 0 failed; 0 ignored
```

（整套里曾出现的红是**与本任务无关的未跟踪探针**在并行/单线程下的偶发：`m7_day5_schedule.rs::probe_day4` 与 `m7_hauto_events.rs::scheduler_tracks_all_seven_h_text_events`，二者单独跑与重跑均通过；另我第一个用例在单线程整套里曾因临时存档文件被清掉报 `os error 2`，已改为私有 per-pid 子目录消除竞争，重跑稳定绿。）

## 先前缺失 / 本次补了什么

- 存档：库层对 Part2/Part3/Part5/vardb 的读写早已存在，但**没有回归钉住「写出的 5 个 Part 与 vardb 都能原样读回」**；更关键的是 `load_into` 不回写 `base_save` 的缺口此前无人覆盖，会让「读档再存档」静默丢 Part4 对象表。本次补测试 + 最小修复。
- 窗口：`window.rs` 的九宫格实现与既有单测早已存在，但缺「真实资产像素 + 真实消息框几何」这一层验收；本次补 `m7_window_skin.rs` 钉死 18×18 资产与 960×168 消息框的铺满关系，并把 H-WIN-1 的确认/未验证点用证据记录。

## 与其它交付不冲突

- 改动文件仅：`crates/wolf-game/src/savegame.rs`（`load_into` 补 `base_save`）、新增 `crates/wolf-game/tests/m7_save_parts.rs`、新增 `crates/wolf-game/tests/m7_window_skin.rs`、本文档。
- `crates/wolf-game/src/window.rs` **未改**（实现本就正确）；未触碰其它源码、battle 测试、`rng.rs`、`docs/UNVERIFIED.md`、`docs/BASELINE.json` 及既有探针/scratch 文件；未 commit / 未 push。
