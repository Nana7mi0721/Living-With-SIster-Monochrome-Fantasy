
## 8. GameHost::database 的实现算法

以下与 crates/wolf-game/src/{game.rs, db.rs} 的现状一致（行号为 HEAD 时的实测位置），可以直接照抄。

### 8.1 容器判定

```rust
// crates/wolf-game/src/db.rs:86
Container::from_flags(flags) = match (flags >> 8) & 3 {
    1 => Container::Sys,
    2 => Container::Data,
    _ => Container::CData,   // 0 和 3 都归 CData
}
```

实测分布：容器 0 有 2,455 站、容器 1 有 7 站、容器 2 有 1,130 站；容器 3 **从未出现**（但默认分支必须落在 CData，因为 250 的 4 参形态 arg3 是 0x050002 之类的值，bits8..9 正好是 0）。

### 8.2 读路径（顺序不能换）

```rust
fn database(&mut self, op: &DbOp) -> Option<DbValue> {
    let container = Container::from(op.container);
    let row       = op.data_id;              // arg1，已 magic 求值
    if op.direction == DbDirection::Write { return self.database_write(op, container, row); }

    // ① 标签优先解析出 (类型, 列)
    let (db_type, column) = resolve_cell(&self.databases, self.stats, op, container);

    // ② row == -1 ⇒ 返回解析后表的行数，不看列
    if row == -1 {
        let count = self.databases.row_count(container, db_type).unwrap_or(0);
        return Some(DbValue::Num(i32::try_from(count).unwrap_or(i32::MAX)));
    }

    // ③ row < -1 ⇒ 复位哨兵在读路径上返回 0 / 空
    if row < -1 { return Some(DbValue::Num(0)); }

    // ④ 正常取值：引擎 cell 层优先，其次 .dat
    if let Some(value) = lookup_cell(&self.databases, container, db_type, row, column) {
        return Some(value);
    }

    // ⑤ 兜底：记录诊断，返回 None（调用方拿到 0）
    self.note(format!(
        "250 DatabaseAccess: {container:?} ({db_type}, {row}, {column}) flags {:#x} labels {:?}/{:?} unanswered",
        op.flags, op.type_label.as_deref().unwrap_or(""), op.field_label.as_deref().unwrap_or("")
    ));
    None
}
```

### 8.3 写路径

```rust
fn database_write(&mut self, op: &DbOp, container, row) -> Option<DbValue> {
    if row < 0 { /* -1 / -2 都出现在写路径上：都不改格子 */ 
        return None;
    }
    let (db_type, column) = resolve_cell(&self.databases, self.stats, op, container);
    let current = lookup_cell(&self.databases, container, db_type, row, column)
                      .map(|v| v.as_num().unwrap_or(0))
                      .unwrap_or(0);
    let next = combine_cell(op.op, current, op.source.as_ref().and_then(|v| v.as_num()));
    if matches!(op.source, Some(DbValue::Str(_))) {
        self.databases.set_text(container, db_type, row, column, text);
    } else {
        self.databases.set_num(container, db_type, row, column, next);
    }
    None   // 250 的写形态不产生返回值
}
```

- `op.op` 来自 flags 的低位（0 = 覆盖，1 = 加，2 = 乘，……见 docs/OPCODE_250.md）。
- 写操作也**必须**先做标签解析，否则 CE104 写"メンバー1"会写到 属性耐性 里。

### 8.4 标签解析的具体实现

```rust
// crates/wolf-game/src/db.rs:376
pub fn resolve_labels(&self, container, db_type: i32, column: i32,
                      type_label: Option<&str>, field_label: Option<&str>) -> (i32, i32) {
    let mut resolved_type = db_type;
    if let Some(label) = type_label {
        let key = normalise_label(label);
        if let Some(table) = self.table(container) {
            if let Some(t) = table.types.iter().find(|t| t.name == key) {
                resolved_type = t.index as i32;
            }
        }
    }
    let mut resolved_column = column;
    if let Some(label) = field_label {
        let key = normalise_label(label);
        if let Some(table) = self.table(container) {
            if let Some(t) = table.types.get(resolved_type as usize) {
                if let Some(i) = t.fields.iter().position(|f| f.name == key) {
                    resolved_column = i as i32;
                }
            }
        }
    }
    (resolved_type, resolved_column)
}
```

三个约束：

1. **类型名匹配不到就退回整数**（实测 15.2% 的站点没有类型名可用）。
2. **列名匹配是"在解析后的类型里找"**，不是全局找——同一个字段名可能出现在多个类型。
3. `table(Sys)` 返回 None（SysDatabase 没有 .project 的字段名装载）⇒ 容器 1 的列名解析目前是 no-op。实测容器 1 只有 7 个站点、列名标签也只出现 0/1 这种小整数，影响为零。**【已实测】**

标签归一化（db.rs:434）：

```rust
fn normalise_label(label: &str) -> String {
    label.trim_end_matches(char::from(0))
         .replace('—', "-")     // U+2014
         .replace('―', "-")     // U+2015
         .replace('　', " ")    // U+3000
         .trim()
         .to_owned()
}
```

### 8.5 引擎 cell 层

```rust
// crates/wolf-game/src/db.rs:119
pub struct Databases {
    data:  Option<Database>,
    cdata: Option<Database>,
    sys:   Option<SysDatabase>,
    cells: BTreeMap<(Container, i32, i32, i32), i32>,     // 引擎写过的数值格
    texts: BTreeMap<(Container, i32, i32, i32), String>,  // 引擎写过的字符串格
}
```

读时 **cells/texts 优先于 .dat**（db.rs:237/250）。理由：.dat 是只读模板，脚本的写操作必须能被后续读看到；直接改 .dat 的内存镜像也行，但那会把"模板"和"运行时"混在一起，存档回写时不好区分。

引擎默认值（apply_engine_defaults，db.rs:168）：

| 地址 | 值 | 用途 |
|---|---|---|
| CData (18, 97, 0) | screen.x / 32 | 座標倍率X |
| CData (18, 98, 0) | screen.y / 24 | 座標倍率Y |
| Data (18, 0, 24) | "SystemFile/WindowBase.png" | 消息窗口皮肤 |

### 8.6 存档同步

```rust
// 读档：把存档 vardb 的每格覆盖到 cell 层
pub fn apply_saved_database(&mut self, database: &wolf_save::VariableDatabase) {
    for ty in &database.types {
        for row in &ty.rows {
            for (column, field) in row.fields.iter().enumerate() {
                if field.definition >= 2000 { self.set_text(Container::CData, ty.index, row.index, column, &field.text); }
                else                        { self.set_num (Container::CData, ty.index, row.index, column, field.value); }
            }
        }
    }
}

// 存档：只把 CData 的 cell 层写回 vardb
pub fn write_into(&self, database: &mut wolf_save::VariableDatabase) -> (usize, usize);
```

- 只同步 **CDataBase**：DataBase（技能/物品定义）与 SysDatabase（地图/字符图）是只读资源，不进存档。
- 存档 vardb 的类型号与 .dat 完全同序（第 2.6 节），不需要映射。
- 若某个格子既没被写过也没在 .dat 里（列越界的运行列），存档时**保持 vardb 原值**，不要凭空造格。

### 8.7 完整解析公式（把本节浓缩成一行）

```
file_cell(container, arg0, arg1, arg2, type_label, field_label):
    ti = resolve_type(container, arg0, type_label)          // 标签优先，整数 fallback
    if arg1 == -1: return RowCount(container, ti)           // 元查询
    if arg1 <  -1: return Reset/0                           // 复位哨兵
    col = resolve_field(container, ti, arg2, field_label)    // 标签优先，整数 fallback
    if col < 0: col = 0                                      // 负列夹 0
    return cell_layer(container, ti, arg1, col)              // 引擎层
        ?? dat_value(container, ti, arg1, col)               // .dat 值（row 越界则夹最后一行）
        ?? 0 / ""
```

---

## 9. CDataBase 的 30 个类型

### 9.1 总表 **【已验证】**（行数由字节解析，类型名/字段名来自 .project）

| ti | 类型名 | method | 字段数 | 数值/字符串 | 行数 |
|---|---|---|---|---|---|
| 0 | 主人公ステータス | 1 | 51 | 46 / 5 | 17 |
| 1 | ┣ 技能習得Lv | 2 | 100 | 100 / 0 | 16 |
| 2 | ┣ 主人公行動AI | 2 | 48 | 48 / 0 | 22 |
| 3 | ┣ 属性耐性 | 2 | 100 | 100 / 0 | 40 |
| 4 | ┣ 状態耐性 | 2 | 100 | 100 / 0 | 16 |
| 5 | ┗ 状態リスト | 2 | 100 | 100 / 0 | 16 |
| 6 | パーティー情報 | 0 | 7 | 7 / 0 | 1 |
| 7 | ┣所持アイテム個数 | 20002 | 1 | 1 / 0 | 40 |
| 8 | ┣所持武器個数 | 20003 | 1 | 1 / 0 | 10 |
| 9 | ┗所持防具個数 | 20004 | 1 | 1 / 0 | 10 |
| 10 | 戦闘時一時ステータス | 1 | 16 | 15 / 1 | 40 |
| 11 | ×┣ 属性耐性計算結果 | 2 | 100 | 100 / 0 | 11 |
| 12 | ×┣ 状態耐性計算結果 | 2 | 100 | 100 / 0 | 11 |
| 13 | ×┣ 状態リスト | 2 | 100 | 100 / 0 | 11 |
| 14 | ×┣ コマンド処理リスト | 0 | 4 | 4 / 0 | 100 |
| 15 | ×┗ お宝リスト | 0 | 60 | 60 / 0 | 1 |
| 16 | × お店一時処理用 | 0 | 14 | 11 / 3 | 15 |
| 17 | ×主人公一時DB | 30000 | 21 | 20 / 1 | 20 |
| 18 | 基本システム用変数 | 0 | 2 | 1 / 1 | 100 |
| 19 | 万能ウィンドウ一時DB | 0 | 11 | 5 / 6 | 200 |
| 20 | ×ドロップアイテムリスト | 0 | 6 | 6 / 0 | 1 |
| 21 | メンバー情報 | 1 | 9 | 3 / 6 | 17 |
| 22 | （空） | 0 | 1 | 0 / 1 | 1 |
| 23 | （空） | 0 | 1 | 0 / 1 | 1 |
| 24 | （空） | 0 | 1 | 0 / 1 | 1 |
| 25 | 追加キーコンフィグ設定 | 0 | 9 | 9 / 0 | 1 |
| 26 | （空） | 0 | 1 | 0 / 1 | 1 |
| 27 | カスタム選択肢 | 0 | 6 | 4 / 2 | 1 |
| 28 | （空） | 0 | 1 | 0 / 1 | 1 |
| 29 | （空） | 0 | 1 | 0 / 1 | 1 |

DataBase.dat 的 40 个类型行数（0 起）：44, 34, 40, 40, 40, 25, 19, 10, 24, 100, 40, 42, 40, 41, 1, 1, 1, 1, 1, 1, 11, 1, 1, 1, 1, 22, 9, 20, 101, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1。

SysDatabase.dat 的 28 个类型行数（0 起）：50, 16, 6, 20, 30, 80, 150, 10, 300, 6, 3, 1, 30, 80, 800, 4, 7, 4, 1, 1, 1, 6, 1, 10, 12, 1, 1, 1。

**零字段类型陷阱**：SysDatabase 的 4/5/6/14/15..23 号类型字段数 = 0，但行数不为 0（30、80、150、800、4、7、4、1、1、1、6、1、10）。类型 14 是 800 行零字节。若实现里用 `row_count = remaining / min_row_len` 猜行数，会在这些类型上**除零**。正确做法：直接读 u4 row_count，并对它做上限检查（如 ≤ 100000）。

### 9.2 与队伍相关的六个类型的字段清单

**ti 0 主人公ステータス**（property = [2000,2001,2002,2003,2004, 1000..1045]，5 个字符串在前 46 个数值在后）

| fi | 名字 | field_type |
|---|---|---|
| 0 | 头衔1 | 0 |
| 1 | 头衔2 | 0 |
| 2 | 头衔3 | 0 |
| 3 | 歩行グラフィック画像 | 1（"CharaChip"） |
| 4 | 顔画像 [バトル·ステ—タス用] | 1（"Picture"） |
| 5 | 勇气 | 0 |
| 6 | 机巧 | 0 |
| 7 | 运动 | 0 |
| 8 | 心 | 0 |
| 9..16 | 技 / 体 / 精神 / 賢さ / 器用 / 敏捷 / 筋力 / 屈強 | 0 |
| 17..21 | 最大行動力 / 行動力 / 防御力 / 最大HP / HP | 0 |
| 22..29 | 最大MP / MP / 近攻撃力 / 遠攻撃力 / 防御力 / 戦行動力最大 / 戦行動力 / 素早さボーナス | 0 |
| 30..50 | （无名） | 0 |

**ti 6 パーティー情報**：0 持有金额 / 1..6 メンバー1..6

**ti 17 ×主人公一時DB**（property = [1000..1014, 2000, 1015..1019]，20 值 + 1 字符串）

0 [一時]計算済みMaxHP · 1 MaxSP · 2 攻撃力 · 3 防御力 · 4 精神攻撃 · 5 精神防御 · 6 敏捷性 · 7 命中率 · 8 回避率 · 9 クリティカル · 10 [一時]移動時ステの有無 · 11 [一時]HP自然増減値 · 12 [一時]SP自然増減値 · 13 [一時]次ステ—タス回復まで · 14 [一時]次ステ—タス回復Max · **15 [一時]状態異常名一覧（字符串）** · 16 [一時]封印状態？ · 17 [記憶]移動時技能位置 · 18 [記憶]戦闘時技能位置 · 19 [記憶]戦闘時アイテム位置 · 20 [記憶]戦闘時コマンド位置

**ti 18 基本システム用変数**（property = [1000, 2000]，100 行）

0 数値 / 1 文字列。这是全局变量表的镜像：行 5/6/7/35/36/37/38/51/97/98 被 CE90 当座標/倍率用；行 97/98 被 apply_engine_defaults 写死为屏幕倍率。

**ti 19 万能ウィンドウ一時DB**（property = [2000, 1000, 1001, 1002, 2001, 2002, 2003, 1003, 1004, 2004, 2005]，200 行）

0 項目文（str） · 1 項目コード · 2 選択可能状態[1=可] · 3 残り個数/消費ポイント · 4 付与文字列[前]（str） · 5 付与文字列[後]（str） · 6 説明文（str） · 7 [オプション]予備数値A · 8 予備数値B · 9 予備文字列A（str） · 10 予備文字列B（str）

**ti 21 メンバー情報**（property = [2000..2005, 1000..1002]，17 行）

0 名字（str） · 1 バトルグラフィック基本 · 2 攻撃 · 3 転ぶ · 4 喜ぶ · 5 バトル顔アイコン（str） · 6 仲間になっているか · 7 隊列 · 8 最大行動力

注意 property 顺序是 **6 个字符串在前、3 个数值在后**，但字段名顺序是 名字/グラ/攻撃/転ぶ/喜ぶ/顔/加入/隊列/行動力 —— 解析时要按 property 分块（先 6 个 t_str 再 3 个 s4），**不能**按字段名顺序读写。

---

## 10. 未验证 / 未解释

| # | 项 | 状态 | 下一步 |
|---|---|---|---|
| 1 | `row == -1` 的"行数"是否含被 clamp 的越界行 | 已验证语义，边界未测 | 无站点会踩到；若踩到按"解析后表的 row_count"即可 |
| 2 | `row < -1` 的写操作到底清空哪些格子 | 未验证 | 目前实现是"不改任何格子"。若要更真实，可在 CE5/CE112 路径上抓一次内存快照比对 |
| 3 | 运行列号 30..35、61..64 在类型 0 上到底对应什么 | 未解析 | 没有站点带字段名标签用这些列；可能只有 magic 列 1600012 |
| 4 | 为什么 CData#6 读用 3、写用 6 | 未解释（只记录） | 编辑器历史遗留；不影响实现 |
| 5 | 存档 vardb 段结束后还剩 2 字节 | 未验证 | 可能是分行/尾部标记，见 docs/SAVE_SPEC.md |
| 6 | `DataBase` 容器一条写都没有，但存在 4 条 `(Sys, 10)` 的写 | 未验证 | 容器 1 的 arg0 = 10 无标签，需追事件 |
| 7 | `trigger_condition == 4` 等"规格里有、本作没触发"的分支 | 见 FORMAT_SPEC 10.2 | — |

---

## 附录 A：探针脚本索引（全部在 work/ 下，可重跑）

| 脚本 | 产出 |
|---|---|
| work/_pd_01_parse.py | 三个 .dat 的容器解析 + method 直方图 |
| work/_pd_02_t0.py | CData#0 的 51 字段与 17 行全量 |
| work/_pd_09_types.py | CData 6/14/16/18/19/21 的行内容 |
| work/_pd_14_ce71.py | CE71 参数映射 + opcode 300 普查 |
| work/_pd_24_project.py / _pd_45_prjfull.py | .project 全量 schema（后者写 work/_prj_full.json） |
| work/_pd_46_cdata_table.py | 9.1 的 30 类型表 + field_type 串 |
| work/_pd_47_savemethod.py / _pd_48_savemethod2.py | 存档 vardb 与 .dat 的 method/字段/行数比对（9 存档零差异） |
| work/_pd_50_rowsrc.py | from_type_in_db 行数与源表比对（证明不可推导） |
| work/_pd_57_dump.py | CE105 / CE104 / CE98 指令体 |
| work/_pd_59_table.py / _pd_66_rowsall.py / _pd_67_fullrows.py | 三个 .dat 的行数与 spot cell 校验（EOF 精确） |
| work/_pd_61_map.py | 标签解析率 + 每 (容器, arg0) 映射 |
| work/_pd_62_cell.py | CData#3 r0c1 = 100 与 CData#6 r0c1 = 12 的字节级对照 |
| work/_pd_64_colmap.py | 类型 0 的运行列 → 编辑器标签表 |
| work/_pd_65_rm1.py | 29 处 row == -1 + 18 处 row < -1 |
| work/_pd_69_save.py | 存档解密 + vardb 全量解析 |
| work/_pd_70_ce70.py / _pd_71_ce70b.py / _pd_73_p100.py | CE70 的两处 100 比较与 7 个 param-100 站点 |
| work/_pd_75_saverows.py | 存档 CData#0 全 17 行 + CData#21 前 6 行 |
| work/_pd_77_fullmap2.py | 3.2 的完整 (容器, arg0) 表 |

复现前提：`work/_ce_full.json`（550 个公共事件的完整指令解码）、`work/_prj_full.json`（.project schema）、`work/_cdata_tables.json`（30 类型摘要）、`work/_rows_all.json`。

## 附录 B：交叉引用

| 文档 | 章节 | 内容 |
|---|---|---|
| docs/FORMAT_SPEC.md | 6.1 | .dat 的三文件布局、FE FF FF FF 分隔符、footer 0xC2 |
| docs/FORMAT_SPEC.md | 6.2 | 零字段类型、data_id_method 不封闭、STRING_INDICATOR 分支 |
| docs/FORMAT_SPEC.md | 6.3 | 实测字节数字（尺寸/类型数/字段数/行数） |
| docs/FORMAT_SPEC.md | 6.4 | SysDatabase 类型 0 的 6 个已注册地图路径 |
| docs/OPCODE_250.md | 2.1 | flags 位域、arg 编码、magic 变量求值、负列夹取 |
| docs/OPCODE_250.md | 6 | 类型号漂移与标签优先规则（本文第 3 章的直接来源） |
| docs/SAVE_SPEC.md | — | 存档解密、vardb 段定位、fileVersion 0x8C |
| crates/wolf-game/src/db.rs | 47-64, 73-93, 119, 135, 168, 202, 237, 250, 277, 312, 343, 355, 376, 421, 434, 448, 455, 469 | 本文第 8 章对应的实现 |
| crates/wolf-game/src/game.rs | 1068, 1090, 1335, 1370-1377 | 读/写路径与标签优先的实现与注释 |
| crates/wolf-vm/src/exec.rs | 267, 332, 339, 367, 394 | DbOp / DbContainer / DbDirection / DbValue |
| crates/wolf-save/src/vardb.rs | 44, 78, 111, 150, 201 | 存档 vardb 解析（含 u>=0/-1/-2 头编码） |
