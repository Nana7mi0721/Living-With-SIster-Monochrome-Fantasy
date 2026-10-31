# M7/M8 未验证清单（UNVERIFIED）审计

**任务**：task-45（W5）。**HEAD**：bcbcfba（`cargo test -q --workspace` 全绿，`wolf-cli verify` PASS 70/0）。
**范围**：把 `docs/UNVERIFIED.md`（9 条）与 M7/M8 全部已合入证据逐条对齐，给出每条最终状态、依据（file:line / 测试名 / 文档）、精确更正。
**不做**：不改任何源码、不改测试；只写本审计并同步 `docs/UNVERIFIED.md` 的状态描述。
**证据等级图例**：**[实测]** = 有测试/像素对照/反编译佐证；**[推断]** = 依数据分布或逻辑外推，未见原版运行验证；**[文档]** = 仅记录于某报告、尚未被独立测试锁定。

---

## 一、`docs/UNVERIFIED.md` 原始 9 条逐条结论

| # | 条目 | 最终状态 | 严重度 | 关键依据 | 精确更正 |
|---|------|----------|--------|----------|----------|
| 1 | **H-PIC-1** 负番号图片=全屏图层 | 仍未验证（部分解除） | 低 | 实现 `crates/wolf-game/src/picture.rs` `Picture::is_screen_layer`/`Screen::quads`（负号→全图 1024×576）。`-2` 已在标题画面命中并像素对照 **[实测]** | 「`-2`=全屏」已证；**负号真实含义、`-1` 是否另一哨兵、负号能否动画/淡入**仍无证据 [推断]。30 次负号绘制全在标题/读档整屏换图，无法区分解释。保留。 |
| 2 | **H-PIC-2** 图片字段布局长格式槽位 | 仍未验证（低，可忽略） | 低 | 布局 `[6]`不透明/\`[7]`X/[8]Y/[9]缩放/[10]混合/[3][4]窗口宽高 已由标题提示条 `SystemFile/Button01.png` 像素对照（mean abs 0.35/255，98.6% 像素差≤2）**强证** **[实测]**；13,637 长格式样本分布自洽 **[实测]** | `[2]`（0/15/5/30/100…像角度/效果）与 `[5]`（1/2/3/-1000000…）含义仍未知；当前**忽略**这两槽，像素已证不影响标题。保留。 |
| 3 | **H-ANC-1** 锚点位 0x2000..0x6000 | 仍未验证（低） | 低 | 实现 `picture.rs` `place()` 仅 `ANCHOR_CENTRE(0x1000)` 减半宽高。`0x1000` 组 4393 条 X≈512/Y≈288 且像素对照 **[实测]** | 其余 6 nibble 共 **333 条(2.4%)** 一律按左上角；没有任何一张图能证明其指哪个角，且都不在标题/读档路径。保留。 |
| 4 | **H-WIN-1** 窗口皮肤九宫格 | 仍未验证（视觉，低） | 低 | `crates/wolf-game/src/window.rs` `nine_slice`+`SKIN`/`CORNER`。`SystemFile/WindowBase.png` 18×18 逐像素=3px 框+拉伸体 **[实测]**；`tests/m7_window_skin.rs` 3 测 **[实测]**；`docs/research/M7_SAVE_WINDOW.md` | 仍无**原版实机截图**；type3 `#12005` 框 0×0 继承尺寸不画面板；CN 补丁 `#12500/#12501` 不嵌套、右缘 1155>1024 疑补丁排版失误，按脚本原样渲染。保留（纯视觉）。 |
| 5 | **H-KEY-2** 键码模型与 123 掩码 | 仍部分未解 | 中 | 已实测 123 写**键码**（10 決定/11 キャンセル/12 サブ/2·4·6·8 方向），非位掩码 **[实测]**；H-KEY-1 作废；实现 `exec.rs` `Host::key_code(mask)`、`key.rs` `code()` | **未解两条**：(1) `arg[1]` 掩码语义（事件 108 掩码 177 一次返多码）现忽略掩码；(2) 未命名键码 13/14/20/100/300 按「永不返回」处理。一旦有脚本依赖则走 else。保留。 |
| 6 | **H-PRNG-1** 随机数流 | 仍未验证（仅回放位对齐） | 中 | 算式已由反编译落地：`crates/wolf-vm/src/rng.rs` Park-Miller MINSTD（`state=state*16807%0x7FFFFFFF`，>2e9 再乘，0 重播种）+ 模归约 `low+draw%(abs(b-a)+1)`；`tests/rng_retail.rs` 15 测 **[实测]**；`docs/research/M8_PRNG_RANGE.md` | 算法/归约律**已证**；唯一残余=**零售以时钟自播种、移植用固定 `EVENT_SEED=1`**→「同存档+同输入逐位对齐」未证 **[推断]**。保留（仅确定性）。 |
| 7 | **H-MENU-1** 选择肢(102)可视化 | **已解除** ✅ | — | `crates/wolf-game/src/choice.rs` `overlay_at_sized` 画九宫格/行/光标；`crates/wolf-cli/src/main.rs` `build_capture_frame` 接线（commit 7293ee8）；`tests/m7_choice_window.rs` **2 测通过** **[实测]**；截图 `work/out/m7_choice_window.png`（6 选项+▶ 光标） | 原「尚未绘制」记录**过时**。仅默认位置/边框/字体由事件锚点（如 (512,192)）反推，属猜测但已非功能缺口。**从清单移除**。 |
| 8 | **H-124** 变量操作(扩展)124 | **已解除（指令层）** ✅ | — | `crates/wolf-vm/src/exec.rs:1721-1778` 长/短格式完整分派；`tests/opcode_124_long.rs` **7 测通过** **[实测]**；1045 站点（公共事件 579：0x4000×359/0x3000×174/0x1000×37/0x4400×8/0x1200×1）；`docs/research/M7_OPCODE_124.md` | 指令本身无缺陷。残余只在 host 侧且 M8 已大部分实现（engine 11/12 鼠标位移、17=当前 CE 号、picture field5=0、character_state 自 W3-a 起真实）。仅 `field 13`（光标下事件，本作 **0 站点**）仍返回 None，非缺陷。**指令条目从清单移除**，host 残余并入「其它真残余」。 |
| 9 | **H-ASSET-1** `Picture/word_effect*` 笔误 fallback | 仍未验证（数据问题，低） | 低 | 实现 `crates/wolf-game/src/assets.rs` 收窄回退 `Picture/word_effect*→hword/word_effect*` 带诊断；事件 354 的 7 个引用（30/36/55/56/57/58/59）**[实测文件缺失实况]** | fallback 已兜住渲染。**未验证=原作运行时是否也认此笔误**（DxLib 是否另有搜索路径）、作者本意 `hword/` 还是 `word/`；应数据侧修而非永久依赖 fallback。保留。 |

**9 条统计**：已解除 **2**（H-MENU-1、H-124）；仍开放 **7**（H-PIC-1、H-PIC-2、H-ANC-1、H-WIN-1、H-KEY-2、H-PRNG-1、H-ASSET-1，均属低风险视觉/数据/确定性，无功能阻断）。

---

## 二、`docs/UNVERIFIED.md` 之外的真残余（本次补录）

| 残余 | 状态 | 依据 | 说明 |
|------|------|------|------|
| **opcode 111 值操作数引用语义** | **真残余（功能性）** | `crates/wolf-vm/src/exec.rs:2222` 仍 `let value = arg(cmd, 2 + 3 * i);`，随后 `compare(lhs, value, operator)`（lhs 在 :2224 走 eval_num）；`tests/opcode_111_semantics.rs` 5 测中最后一条 `#[ignore]`（:204-206）→ **4 过 1 忽略** **[实测代码态]** | 数据里 **356 处（22695 三元组的 1.57%）** `value` 是变量/自身变量引用，当前按字面量比较 → 这些站点的条件会错。`docs/research/M7_OPCODE_111.md` 给出 1 行补丁（rhs 走 eval_num）**但未落地**（当时 task-39 在改 exec.rs 的 250）。影响 CE244 胜负展示分支等。 |
| **CE159/CE162 目标选择执行侧分发** | **真残余（开放）** | `CE188` 指令线从未生成 `CE159`（技能選択実行）；`CE162` 唯一真实调用者是 CE159 `pc247`；`tests/m7_battle_target_flow.rs` 仅为 **zz 调试探针**（zz_debug_calls/zz_trace_attack_route，println 密集），非绿色验收 **[实测代码态]**；`docs/research/M7_BATTLE_TARGET_SCHEDULER.md`、`M7_BATTLE_TARGET_SELECT.md` | 目标**生成**已修（ENEMY_COUNT 单元 CData type18 row69 col0=1；CE190 pc36/CE162 pc12 读取；`tests/m7_battle_target_scheduler.rs` 6 测、`m7_battle_target.rs` 3 测）。缺的是**执行侧**从指令窗口到 CE159/CE162 的调用链。 |
| **CE333 H-scene 坏结局不可达** | **真残余（不可达分支）** | 入口深门 `CE246 pc1943` 需 `var157==13`，全库无字面量 13（只有 {1,2,16}）；`var157` 仅由 self `1_600_013` 拷贝（CE282 pc342 / CE329 pc663）；中段 H 存档 SaveData08 `var157==12`。坏结局还需 `v23==0`，而零售 `v23={9,9,10,6,6,9,8,9}` 全 ≥6；强设 v23=0 触发 `var166=1`+爆発。`tests/m7_hmode_state.rs` 2 测、`m7_hscene_deep.rs` 2 测 **[实测]** | CE346 cutin **可达**（SaveData08 裸体路径，`cutin_211.png`+`cutin_161.png` opacity 255）。无法从任何零售存档进入 CE333 坏结局=确证不可达，属数据/状态缺失而非引擎缺陷。 |
| **character_state 移动/高度/屏幕偏移** | **残余限制（非缺陷）** | `docs/research/M8_CHARACTER_STATE.md`；`tests/m8_character_state.rs` 3 测 **[实测]** | W3-a 已让 `character_state` 对地图事件与队长(-2) 回答字段 0..10 真实坐标；限制：无移动模型、身高 field4 恒 0、`-1`(运行中事件)/`-3..-7`(队伍) 返回 None、屏幕偏移靠推断无截图比对、朝向为原始值。 |
| **PRNG 时钟种子回放位对齐** | **残余（确定性）** | 同 H-PRNG-1；`tests/rng_retail.rs` 15 测 | 见上表第 6 条。 |
| **音频** | **未验证（需人耳）** | `docs/research/M8_BACKLOG.md` G10 | 无自动化音频比对；需人耳核对。 |

---

## 三、需从旧文档纠正的「过时否定」（已被 M7/M8 证据推翻）

| 旧结论（出处） | 现状 | 依据 |
|----------------|------|------|
| opcode 250 写分支无法写字符串/名字（M7_BATTLE_MENU_DB 早期） | **已修（task-39）**：`exec.rs:2050-2059` argc≥5 时按字符串 kind 写入；`tests/opcode_250_string.rs` 4 测 + `tests/m7_battle_menu_text.rs` **[实测]** | 不是残余。 |
| CE244 战斗结算结构性不可达（M6_BATTLE_REAL / M7_CE244_DIRECT 旧结论） | **已修（M8 W1-c）**：`crates/wolf-game/src/battle_wiring.rs` `CE242` arg0=行动槽 1（非 2000354）；`tests/m7_day4_playable.rs` 证 day1→day4→真实 CE253→CE239→CE244(2000352=1/2000343=1)→晚餐→夜→day5 08:30 无注入；`tests/m7_battle_playable.rs` **[实测]** | 仅余 111 字面量使「胜负展示分支」(pc36 `2000349==self1600010`) 恒死，属显示层。 |
| H 文字 word_effect 层恒 alpha 0（M7_HAUTO_EVENTS G4） | **已修（W2-c）**：`crates/wolf-game/src/picture.rs` `Transition::from_command` 无动画尾时回退 raw slot 2（WOLF 淡入时长）；`tests/m7_hword_visible.rs` 证 EV350(var118 门) 槽 4000 `hword/word_effect38.png` alpha 升到 ~0.99 再 100 帧插值到 0 **[实测]** | 可见性已解决。 |
| H 选择窗口(102)未绘制（H-MENU-1 旧） | **已解除（M7/task-18）**，见上表第 7 条 | — |
| H-124 唯一功能级否定（M7_BACKLOG） | **已解除**，见上表第 8 条 | — |
| day4→day5 必须走战斗/无自然路径（M6 旧） | **有自然非战斗路径且已证**：`tests/m7_day4_playable.rs` day4 经真实 CE253/CE239 结算后自然入夜到 day5 08:30 | — |

---

## 四、结论（供 Lead 汇总）

- **UNVERIFIED.md 9 条**：解除 2（H-MENU-1、H-124 指令层），保留 7（全部低风险：视觉/数据/确定性，无功能阻断）。
- **补录真残余 6 类**：opcode111 值操作数引用（功能性，356 站点，补丁未落地）、CE159/CE162 目标执行侧分发（开放）、CE333 坏结局不可达（状态缺失）、character_state 移动/高度/偏移（限制）、PRNG 时钟种子（确定性）、音频（人耳）。
- 旧文档的 6 条「过时否定」应在各报告/M8_BACKLOG §7 中按上表纠正；本审计以 `docs/UNVERIFIED.md` 与 M7/M8 已合入测试为准，未新增任何测试或源码改动。
