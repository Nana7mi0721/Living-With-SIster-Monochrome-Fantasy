# M13 交接：内容一致首批验收（2026-11-01 标准）

> 仓库：`E:\Study\github program\imoto life`
> 阅读顺序：本文件（M13 最新）→ `docs/README.md` → `PLAN.md`；`HANDOFF_M12.md` 及更早已标历史归档。
> 数据根 `work/all`；零售存档 `work/retail_backup/Save`（SaveData01..08 + System.sav）。
> 本批提交（master，已 push）：`7c3b728` common-event self 持久修复；`1826edd` 内容矩阵/DLC/主线探针；`ba608f6` 跨天存读/H 阈值/NG+/随机事件 + op111 语义更正；`dcd513a` Text picture 存读修复。

## 1. 一句话现状

M13 的首批内容一致验收已完成：**common-event self 跨调用持久 bug 修复后，两张最远零售档（SaveData01/08 day90）可无注入自然通关**（stage28 決着→stage29 旅路，var492=1、warnings=0）；DLC CE452、111 随机事件抽样、H 阈值矩阵、NG+ 继承、跨天存读/8 槽 round-trip 均有可执行测试与证据；另修 Text picture 消息窗尺寸存读丢失。仍有“9 个结局中的其余结局、条件注入 17 项逐条、像素/音频人工验收”等未做（见 §6）。

## 2. 本批交付（均新增测试 + 研究底稿）

| 主题 | 测试 | 文档 | 结论 |
| --- | --- | --- | --- |
| 主线结局（无注入） | `crates/wolf-game/tests/m13_mainline_endings.rs`（3 默认 + 2 ignore） | `docs/research/M13_MAINLINE_ENDINGS.md` | SaveData01/08 day90→day95：CE251 self51 4→3→2→1→0，stage28 首帧(day95,f164)→stage29，**var492=1、warnings=0**，抓到贺词“恭喜您完成了整个故事剧情…”。SaveData05→day101 逃亡 END（CE305 stage27 期限切れ）。stage30 实为“尖叫胡萝卜”支线，不置结局旗标、门 var366==0 与通关后互斥。 |
| DLC CE452 | `crates/wolf-game/tests/m13_dlc_452.rs`（5 默认 + 1 ignore 全矩阵） | `docs/research/M13_DLC_452.md` | CE251@2498 零参 210 唤 CE452，EP0–EP36 37/37 通过（注入 self10=n 证）；自然推进 var619 0→25（EP0–EP25），EP26 起被 CE452 内 self11 等数据门拦住（非 bug）。 |
| 内容矩阵 | 文档 + 抽查 | `docs/research/M13_CONTENT_MATRIX.md` | 115 行随机事件已定位 114/115（#78 数据不可达结案）；H 6 类门、战斗 CE152/192、物品/服装/胖次、相册/夢精、九结局坐标。 |
| 111 随机事件抽样 | `crates/wolf-game/tests/m13_random_events.rs`（4 默认 + 1 ignore） | `docs/research/M13_RANDOM_EVENTS.md` | 25 事件/23 用例覆盖 12 池，warnings=0 且命中台词；调用目标扫描 19624，dead=0，dormant=8（CE270 8 个 op300 悬空，0 调用者不可达）。 |
| H 阈值矩阵 | `crates/wolf-game/tests/m13_h_thresholds.rs`（3 测） | `docs/research/M13_H_THRESHOLDS.md` | 37 条门命令逐字节锁定；阈值-1/==阈值边界在 VM 上翻转；CE318 信頼=100 spawn CE360、=99 不 spawn。 |
| NG+ 继承 | `crates/wolf-game/tests/m13_ngplus.rs`（3 默认 + 2 ignore） | `docs/research/M13_NGPLUS.md` | CE436 点数表 + SampleMapA 继承菜单均由脚本驱动，生产路径可用（var550 来自 System.sav；置 2 可进 9 选项菜单并结算）。风险：save/system.sav vs Save/ 大小写敏感 FS（仅记录，未改 src）。 |
| 跨天存读/8 槽 | `crates/wolf-game/tests/m13_crossday_save.rs`（2 默认 + 1 ignore） | `docs/research/M13_CROSSDAY_SAVE.md` | 8/8 零售档 load→save→load→save 逐字节幂等；新开档 day4/day11；S1==S2==S3 首次中途存档即不动点（Text picture 修复后）；self banks 跨存读保持。 |

## 3. 本批修复的 bug

1. **common-event self 变量跨调用持久**（`7c3b728`，`crates/wolf-vm/src/exec.rs`、`crates/wolf-game/src/game.rs`、`savegame.rs`、`battle_wiring.rs`）：VM 原来每次 common event 调用 self 全 0，Part3 只写不读，导致 CE251 倒计时/CE452 章节计数每日复位、主线与 DLC 不可达。新增 `Vm.common_self` + dirty 集，start 时播种、reap/stop 写回，savegame 读档播种（不标脏保 8/8 逐字节），save_to 写 dirty∪live。合成 battle 驱动改用 `start_event_fresh`。
2. **Text picture 消息窗尺寸存读丢失**（`dcd513a`，`savegame.rs`）：`picture_to_record` 对 Text kind 把 vars7_18[2]/[3] 写成 split=(1,1)，活动消息窗 16001（7680×1200）第一次中途存档后丢尺寸。修复为 Text 用 window/基线回写；新增单测；首次中途存档即 byte fixed point。

## 4. 关键更正（推翻旧假设）

- **op111 单条多三元组是“首真命中的有序分档”，不是逻辑与**（`docs/OPCODE_SPEC.md` §5.9、`program.rs` BlockKind::Condition）。因此 CE318 @338/@415 = “欲求不満≥2 走 case1，否则 発情度≥75 走 case2”；CE323 @1060 = 性興味 250/150/0 三档；@1055 = 信頼≥200/≥0 两档。只有嵌套单条件 111（如 CE310@159→161→163→165）才等价 AND。已同步更正 M13_CONTENT_MATRIX / M13_CROSSWALK_UNRESOLVED / M13_RANDOM_EVENTS。
- 周末 H @163 为 **性欲(var18)≥40**（非欲求不満）；厕所计数为 **var222**（var221 是オナニー進行）；参军 END 位于 CE326@35–71（信頼≤5 ∧ 機嫌≤1）。
- var493 不是逃亡判据；逃亡唯一判据 = CE305 stage27 被调度 + BAD END 文本。

## 5. 验证门禁（本批集成后）

```bash
WOLF_GAME_DIR=work/retail_backup WOLF_DATA_ROOT=work/all cargo test -q --workspace --no-fail-fast
# 139 test-result 块 / 836 passed / 0 failed / 10 ignored / EXIT 0
./target/debug/wolf-cli.exe verify --root work/all   # PASS 70 / pending 0
```

> 环境注意：不 pin `WOLF_GAME_DIR=work/retail_backup` 时，`wolf-save` 的 `every_save_parses_with_the_measured_shape` 与 `typed.rs` 两条会因用户 Steam 安装档已被游玩更新（386830B vs 366146B）而红，与本项目代码无关。

## 6. 首批遗留（第二批闭环见 §6.1；仅人工项仍开放）

1. **九结局**：已测 逃亡、正常通关；其余 7 条已由 §6.1 的 `m13_all_endings.rs` 闭环。
2. **条件注入型随机事件 17 项**：已由 §6.1 的 `m13_random_events_injected.rs` 闭环。
3. **P1-E**：锚点 5/6 与关键场景零售像素 diff、31 首 loop 人耳验收（需人工/截图）——仍开放。
4. **DLC EP26–EP36**：已结案（task-18）。不是移植缺口：var395 是永久「献身」技能标志（不可重置）；EP26 卡住是隔离探针缺“已升级队伍”战斗属性所致，预置真实战斗属性后同一局自然链 EP0→36 全通、var619=36。
5. 文档收口：`PLAN.md`/README 的 M13 段、`docs/research/README.md` 登记、`docs/BASELINE.json` 证据键均已更新。

## 6.1 第二批验收（本轮完成）

- 九结局其余 7 条：`m13_all_endings.rs`（14 默认 + 2 ignored 全绿），覆盖妊娠/兵役/返家/病亡症状链/暗影/魔界/农民/爆炸；`M13_ALL_ENDINGS.md`。
- 17 项条件注入随机事件：`m13_random_events_injected.rs`（3 测试全绿：16 DLC 事件点 + 特4 海豹 + 15 门三元组）；`M13_RANDOM_EVENTS_INJECTED.md`。
- H 端到端：`m13_h_endtoend.rs`（5 测试全绿：周末/厕所/浴室/CE335/餐厅）；餐厅门更正为 CE326@801–805 `var424≥3 ∧ var18≥100 ∧ var16≥1`；`M13_H_ENDTOEND.md`。
- DLC EP26–36（task-18 结案）：`m13_dlc_452_ep26_36.rs`（2 默认+2 ignored）；**不是移植缺口，未改生产 src**。var395 是永久「由衣献身」技能解锁标志，绝不能在战斗结束重置；EP26 卡住是隔离探针未预置“已升级队伍”战斗属性的假象（默认战力~15 vs 敌槽~4500）。一次性预置 `var2000040..45/2000170..175=300`、var395 全程不清洗后，同一局自然链 EP0→EP36 全通、var619=36（diagnostic 未升级队伍卡 EP26 maxpc=4224）。`M13_DLC_452_EP26_36.md`。
- 全量：`860 passed / 0 failed / 14 ignored`；`wolf-cli verify` PASS（64 项匹配 baseline + 1 项 dxarchive pending，退出 0）。

## 7. 约束与未跟踪

- 不提交：3 个永久 scratch（`crates/wolf-format/examples/opscan.rs`、`tests/_scratch_cdata.rs`、`tests/_scratch_event.rs`）；用户资料 `docs/攻略文档/`、`妹相隨 Ver3.CT`、`游戏示例图片/`。
- 提交信息用 `git commit -F work/_commit_*.txt`；临时文件写 `work/out`。
