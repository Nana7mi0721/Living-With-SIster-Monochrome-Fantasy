# 转换器数据契约（M0）

> 状态：**现行**。对应 `docs/godot/PLAN.md` §3 任务 2「定义转换器与运行时之间的数据契约」。
> 决策依据：`docs/godot/DECISIONS.md` D-04（Python 离线转换器）、D-05（全量内容一次导出）、D-07（存档导入口径）、D-11（原始资产不入库）。
> 代码实现：`converter/schema.py`（唯一权威定义）。本文与代码不一致时，以代码为准并修本文。

---

## 1. 边界

| 项 | 约定 |
|---|---|
| 运行期 | 转换器只在开发期运行（Python 3.10+，仅标准库）；游戏运行时不得依赖 Python 或 Rust |
| 输入 | `work/all/`（= `Data/` 下 23 个 .wolf 解包后的目录树：`BasicData/`、`MapData/`、`SystemFile/`） |
| 禁止输入 | ① 直接读 `.wolf`（解包是工具链的事）② WolfTL 导出的 JSON（**有损**：124,204 vs 139,273 条指令，见 FORMAT_SPEC §0.1） |
| 输出 | `game_data/`（**gitignored，永不入库、永不分发**，见 D-11）与 `reports/`（对账报告，可入库） |
| 可复现 | 相同输入 → **逐字节相同**的产物；产物内不写墙钟时间、不写绝对路径 |

## 2. 通用外层

每个产物都是一个 UTF-8、LF、无 BOM 的 JSON 文件；键排序、2 空格缩进、末尾换行（`canonical_json`）。

```json
{
  "schema": "wolf.<name>",
  "schema_version": 1,
  "producer": { "name": "converter", "version": "0.1.0" }
}
```

- 整数一律十进制 JSON number（u32 上限 4294967295，JSON 可精确表示）；ID / 时间 / 金钱**禁止**用浮点。
- 字符串原样保留：不 trim、不做全角半角归一、空串保留为空串。解码规则见 §7。
- 任何字段一旦发布即不得改语义；只能新增可选字段或升级 `schema_version`。

## 3. `manifest.json`

| 字段 | 类型 | 说明 |
|---|---|---|
| `game` | object | `{id, title, appid, version}`，来自 Game.dat / Game.ini |
| `source.files[]` | array | `{path, bytes, sha256}`，path 相对 `work/all`；覆盖每个被读取的输入文件 |
| `artifacts` | object | `programs` / `database` 各为 `{path, bytes, sha256}` |
| `counts` | object | `{common_events, map_events, maps, slots, blank_slots, real_instructions, distinct_opcodes}` |
| `assets[]` | array | `{logical, source, kind, bytes, sha256}`，kind ∈ image / audio / font / other |
| `asset_omissions[]` | array | `{logical, reason}`，刻意不导出的资源（必须写理由） |

校验：`source.files` 中 Game.dat / CommonEvent.dat / DataBase.dat 的 `sha256` 必须等于 FORMAT_SPEC §0.4 基准值，否则转换失败（不是警告）。

## 4. `programs.json`

```json
{ "common_events": [Program], "maps": [{ "id": 2, "file": "Map002.mps", "width": 100, "height": 30,
  "tileset_id": 4, "events": [Program] }] }
```

**Program**（CE 与地图事件共用；差异只有 `kind` 与 `self_base`）

| 字段 | 说明 |
|---|---|
| `id` | CE id 或地图事件 id（地图内唯一） |
| `kind` | `"common_event"` 或 `"map_event"` |
| `title` / `memo` | 原文标题 / 备注 |
| `self_base` | 自变量的运行时基址：CE = 600，地图 = 100 |
| `return_value_id` | `4294967295` 表示无返回值（550 个 CE 中 476 个） |
| `run_condition_op_var_val` | 事件运行条件三元组 `[run_condition, operator, variable, value]` |
| `args` | `{num, str, names[], defaults[], special_spec[], string_pages[][], value_pages[][]}` |
| `color` | 事件色 |
| `self_variable_names[]` | 100 个自变量名 |
| `slots[]` | 指令槽（定长，op=0 为空槽） |
| `blocks[]` | 结构化块（见下） |
| `labels` | `{labelName: pc}`，标签是**字符串**不是索引 |
| `stats` | 与 Rust `PrecompileStats` 同名同义 |

**Slot**

```json
{ "pc": 12, "op": 210, "params": [0, 5], "branch_depth": 0, "strings": ["..."], "move_route": null }
```

- `pc` = 槽序号（= 指令下标，含空槽）。
- `op == 0` 即空槽，此时 `params` / `strings` 必须为空、`branch_depth` 为 0。
- `params` 是 `(param_count-1)` 个小端 u32 的**原样无符号值**；有符号语义由解码器按 opcode 重解释（转换器不猜）。
- `move_route` 非 null 时：`{animation_frequency, move_speed, move_frequency, move_route_mode, behavior_options, route_options, commands:[{kind, u32_args[], u8_args[]}]}`。

**Block**

```json
{ "head": 12, "kind": "condition", "cases": [[1, 13]], "else_pc": null, "cancel_pc": null, "end_pc": 20 }
```

- `kind` ∈ `condition` / `choices` / `loop`；`end_pc = 4294967295` 表示块延续到流末尾（Rust 的 MAX 哨兵），此时导出的 `end_pc` 保持哨兵值，由运行时解释为 `len`。
- `cases` 的 id 语义：条件 111 分支从 1 起，选项 102 为 option_index+2。

**对账口径（必须逐项复现 docs/BASELINE.json）**：CE 550 事件 / 163,828 槽 / 24,555 空槽 / 139,273 实指令 / 47 种 opcode；地图 73 事件 / 134 页 / 37,610 槽 / 7,052 空槽 / 30,558 实指令 / 40 种 opcode，且逐图数字一致（Dungeon 40×30 ts0 5 事件 6 页 79 指令；Map002 100×30 ts4 5 事件 7 页 416 指令；Map003 70×170 ts4 2 事件 2 页 45 指令 ……）。

**未知 opcode 处理**：`op` 不在 opcode 表中时**照常导出**，并计入 `reports/unknown_opcodes.json`。M0 的退出条件是 unknown-opcode 计数 = 0；不为 0 时必须停下来查，不得静默放过。

## 5. `database.json`

```json
{ "game": {}, "database": {}, "cdata": {}, "system": {}, "tilesets": [], "map_tree": {} }
```

- `game` ← Game.dat（变量/开关初值、字符串表、CE 名表等）
- `database` ← DataBase.dat（98 个类型，Rust `database.rs` 已全通）
- `cdata` ← CDataBase.dat，`system` ← SysDatabase.dat，`tilesets` ← TileSetData.dat（6 个 tileset）
- `map_tree` ← MapTree.dat + MapTreeOpenStatus.dat

所有类型都保留**原始类型 id 与序号**，不做重编号；文字按 D-06 保留原文。

**Data type 0（技能）列契约（D-35）**：`work/all` 导出的 type 0 是 18 列精简 schema（`技能の名前`、`スキル説明`、`スキルタイプ`、`消費行動力`、`戦況影響値`、`効果基本値`、`筋力影響`、`精神影響`、`使用時文章`、`スキル属性`、`アニメーション`、`付与するステータス1/2`、`回復させるステータス1/2/3`、`相手の防御を`、重复的 `スキルタイプ`）。`.project` 没有 `使用可能シーン` 或 `効果対象` 字段，因此 CE159 pc110/pc115 的 250 字段标签会回退到 `ItemId` 列：`col2` 是 `スキルタイプ`（44 行全 0），`col3` 是 `消費行動力`（出现 10/11/12）。转换器按字段顺序写 `numbers`，没有证据表明存在列偏移；脚本显然按更宽的零售 schema 编写。**在取得零售 `DataBase.dat` 证据前，不得把 `┏ 効果対象` 别名到 `col3`**；战斗测试必须显式写 250 覆盖层（D-31）。

## 6. `save_import.json`（.sav 导入报告）

由 C# `LegacySaveImporter` 生成，schema 在此冻结；M0 只要求 8 个存档槽全部可解析。

```json
{ "slots": [{ "index": 1, "file": "SaveData01.sav", "bytes": 0, "checksum_ok": true, "encoding_warnings": [],
  "recovered": { "variables": {}, "switches": {}, "pictures": [], "play_time": {},
                 "map": { "id": 0, "x": 0, "y": 0 }, "database_overrides": [] },
  "not_recoverable": ["event_instances","pc","call_stack","wait","audio_phase"] }] }
```

`not_recoverable` 清单直接对应 DECISIONS.md §3(3)：不承诺恢复 pc / 调用栈 / Wait / 音频相位；UI 文案必须与此一致。

`encoding_warnings` 为附加字段：每个槽列出该槽内严格 UTF-8 失败并 replacement 的字段与偏移；`ImportAll` 在任一槽非空时汇总写入 `reports/encoding_warnings.json`，空则不改写该文件。`ImportFile(..., rejectBadChecksum:true)` 在 checksum 不符时抛 `InvalidDataException`；所有导入都校验起始字节 `0x14=0x19` 与末字节 `0x19`。`schema_version` 保持 1（附加字段不破坏旧读取器）。**生产 UI/发行入口必须显式传 `rejectBadChecksum:true`**；默认 `false` 只服务 `SaveImportTest` 的诊断路径（宽松导入仍写 `checksum_ok=false`，不得当成合法生产导入）。

## 7. 字符串与编码

- `t_str` = `u32` 字节长度（**含**结尾 NUL）+ 载荷；读取时去掉 NUL。
- 解码：先严格 UTF-8；失败则按 `errors="replace"` 解码（与 Rust `String::from_utf8_lossy` 等价），并把该字段记入 `reports/encoding_warnings.json`，不得静默改写文本。实现见 `godot/core_cs/LegacySaveImporter.cs` 的 `StrictUtf8` / `ReadMemDataString(size, field)`。
- 归档内文件名是 CP932（Shift-JIS），只影响 .wolf 解包，与本契约无关。

## 8. 版本与迁移

`schema_version` 单调递增；破坏性变更必须 +1，并在 `converter/migrations/` 放一个把 N 版产物升到 N+1 的函数。运行时读到不认识的高版本必须**报错退出**，不得降级猜测。

## 9. 验证入口

```
python -m converter manifest --root work/all --out game_data
python -m converter programs --root work/all --out game_data
python -m converter validate  --root work/all --data game_data --reports reports
```

`validate` 输出 `reports/reconcile.json`；任何一项与 BASELINE 不符即 **exit 1**（失败，不是警告）。

同一入口还写 `reports/database_schema_audit.json`（D-35、D-37）：审计 Data type0 的结构（18 列、44 行、每行 14 个 `numbers` + 4 个 `strings`、字段 raw 顺序），并记录 `効果対象` 与 `使用可能シーン` 因 `.project` 缺少标签而回退到的列。零售 `Data/BasicData.wolf` 解包后的 `DataBase.dat`、`DataBase.project`、`CDataBase.dat`、`CDataBase.project` 与 `work/all/BasicData` 四个 SHA-256 完全一致，故 **D-31 不是转换器列错位**；当前零售 build 的 CE159 pc115 因标签缺失而数据不可达。回退列不再标 `needs-retail-data`，不参与 69 项 BASELINE 对账，也不据此改列；不得把 `効果対象` 别名到 `col3`。
