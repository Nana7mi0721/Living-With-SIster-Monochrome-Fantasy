# M13 P1-D — NG+（二周目 / New Game Plus）继承语义探针

数据来源：零售（中文本地化）数据根 work/all，dump 于 work/wolftl/dump；预构建引擎 target/debug/wolf-cli.exe（HEAD cbbb14d）。测试：crates/wolf-game/tests/m13_ngplus.rs。

## 0. 结论摘要

- **NG+ 没有独立的菜单入口**。它挂在普通「はじめから」流程上：标题画面 → CE42 → Map003 ev1 → SampleMapA ev0（autorun）。执行到 SampleMapA ev0 的 pc347 时，若 var550（引き継ぎポイント，继承点数）>= 2，才弹出继承菜单。
- **进入条件**：上一周目（或任一存档）把「继承点数」写进 Save/System.sav 的 var550；标题启动时 TitleMap ev2 用 op221 把 var550 读进 VM。点数为 0 或 1 时不触发继承。
- **继承点数怎么算**：common event CE436（引き継ぎPチェック，脚本号 500436）按成就逐项累加（见 §2），总分封顶 30；var531（アカツキホモイベ）>= 3 时把结果回写 Save/System.sav。
- **继承什么**：6 类「遗物」按点数分配：强大(var552)、聪慧(var553)、可以信任的家人(var554)、色情的妹妹(var555)、拥有金钱(var556)、同伴的LV(var557)；外加隐藏第 7 项「强韧的肉体」（点数池 >= 2 才出现）。分配后折算出新的攻击/防御/根性/技术/贤さ/技能点/信赖/性兴趣/所持金/三名同伴LV（见 §3 结算表）。
- **重构版现状**：wolf-game 是泛型解释器，不硬编码游戏逻辑；NG+ 不是“专门实现”，而是“能否正确跑通零售脚本”。逐 op 核对后，入口链、op221 单变量读、op220 保存、op130 传送 + autorun 自动启动 SampleMapA ev0、继承菜单 op111/121/102/401 均由现有引擎覆盖。结论：语义上已实现，但此前没有专门测试覆盖；本次新增 m13_ngplus.rs 作为回归护栏。
- **真实样本**：零售 retail_backup/Save/System.sav 的 var550 = 15 >= 2（可继承；var551 = floor(15/2) = 7 点），说明零售存档真的有 NG+ 字段值，无需合成状态即可验证。

## 1. 入口链与触发条件

    boot
     └─ TitleMap.mps event 2 （唯一 event, id=2, 新タイトル用イベント）
          idx30-33  op221  ← save/system2.sav : var503, var626/var531
          idx35-36  op221  ← save/system.sav  : var357
          idx46-62  op221  ← save/system.sav  : var300, 80..87, 324..328, 546
          idx62     op221  ← save/system.sav  : var550 (2_000_550 ← normal_var 550)   ★ NG+ 点载入
          idx103    op210 CommonEvent 500042 (CE42)
     └─ CE42 オリジナルタイトル画面Ａ（标题菜单 [1,2,3] = はじめから/つづきから/おわる）
          idx13-25  若 var550 >= 10 → 画 TitleGraphic6.jpg（NG+ 专属标题图），否则 TitleGraphic.jpg
          选 1（はじめから）→ pc118 op130 [-1, 60, 34, 3, 1] → MapData/Map003.mps
     └─ Map003.mps event 1 ニューゲーム処理
          pc28 op130 [-1, 62, 38, 1, 1] → MapData/SampleMapA.mps
     └─ SampleMapA.mps event 0 ニューゲーム処理（page0 trigger = 1 = Autorun）
          pc347 op111 [1, 2000550, 2, 1]  == (var550 >= 2)
               成立 → 继承菜单；不成立 → ElseCase，直接结算（var552..557 通常为 0）

要点：

- **触发旗标 = var550（引き継ぎポイント）**，阈值 >= 2。另有装饰性阈值 >= 10 仅用于切换标题美术（CE42 idx13-25），不影响是否可继承。
- SampleMapA ev0 是 **autorun 页**（trigger=1；实测二进制 ver 0x66 / tileset 4 / 64x36 / events 40，ev0 id0 x30 y0 pages1，page0 trig=1 sw=[33,32,32,32] var=[1000000×4] val=[0×4]，slot0 条件 self[0]==0 成立）。所以裸 op130 传送后 start_pages 会经 autorun 自动启动它，无需额外 start_map_event。
- **var550 从哪来**：TitleMap ev2 启动即从 save/system.sav 读入 VM（idx62）。只有玩家已有正点数时才会进入继承分支。
- 注：脚本里同时出现小写 save/system.sav（CE436 idx61、TitleMap）与大写 Save/System.sav（CE436 idx66）两个路径。Windows 大小写不敏感两者等价；在 Linux 等大小写敏感文件系统上会解析成不同路径（目录是 Save、文件是 system.sav 小写），小写形态可能读不到，属移植风险（见 §7）。

## 2. CE436「引き継ぎPチェック」：继承点数计算

脚本：work/wolftl/dump/common/436_引き継ぎPチェック.json（dump 视图 68 条）→ work/full/BasicData/CommonEvent.dat 实际 **109 个命令槽**，名 引き継ぎPチェック。累加器 = 临时变量 1600020（acc）。逻辑逐 op 如下（op111 比较，op401 分支，op121 赋值；flag 256(0x100) 表示 +=）。下表「槽位」为实际事件槽号（0 基，dump json 的序号更小，不是同一计数）：

| 槽位 | 条件（op111 lhs >= rhs，op=1） | 加分 |
|---|---|---|
| 1 | var40 カナ好感度 >= 5 | +2 |
| 6 | var42 ユイ好感度 >= 5 | +2 |
| 11 | var44 カスミ好感度 >= 5 | +2 |
| 16 | var53 新メニュー開発 >= 3 | +2 |
| 21 | var534 アザラシ討伐 >= 4 | +2 |
| 26 | var519 ユカリ好感イベント >= 1 | +2 |
| 31 | var367 妹キャライベクリア >= 1 | +2 |
| 36 | var490 パンツコンプ >= 1 | +2 |
| 41 | var492 ストーリーエンド >= 1 | +2 |
| 46 | var470 イカ >= 1 | +1 |
| 51 | var471 殺人鬼 >= 1 | +1 |
| 56 | var472 メンヘラ >= 1 | +1 |
| 61 | var473 サイレントキラー >= 1 | +1 |
| 66 | var474 アザラヒーローズ >= 1 | +1 |
| 71 | var476 超大冒険クリア >= 2 | +1 |
| 76 | var503 温泉イベントクリア >= 1 | +2 |

其后：

- 槽 81 temp17(1600017) = var160(経過日数) / 10（op121 flag 12288=0x3000 除法，第 3 参除数 10）。
- 槽 82-84 若 temp17 >= 10 → temp17 = 10（单段封顶 10）。
- 槽 87 acc += temp17（天数贡献，封顶 10）。
- 槽 88-90 若 acc >= 30 → acc = 30（**总分封顶 30**）。
- 槽 93 若 var531（アカツキホモイベ）>= 3 → 进入持久化块：槽95 SetString 3000023 = 'save/system2.sav'；槽96 op220 [0, 3000023]（整存档快照备份到 system2.sav）；槽99 SetString 3000015 = 'save/system.sav'；槽100 op221 [2000550, 3000015, 2000550, 0]（从 system.sav 读旧 var550 到临时）；槽101 op111 [1, 1600020(acc), 2000550(旧), 0]（op=0，语义推断为「>」）；槽103 2000550 = acc（写回新值）；槽104 SetString 3000015 = 'Save/System.sav'；槽105 op220 [0, 3000015]（把更新后的 var550 保存进 System.sav）。
- **调用点**：CE131（X┣[移]セーブ欄実行，存档界面）idx180（每次存档前）与 CE317（■■早朝処理）idx1007（每回合早朝）。即「存档时 / 早朝」重算继承点数。
- 说明：var550 本身的持久化不止这一条路径——常规保存会写整块 normal var。System.sav 里出现的正 var550 值（零售为 15）说明它确实被持久化，可作为 NG+ 入口的真实依据。

## 3. SampleMapA ev0：继承应用（菜单 → 分配 → 结算）

事件：work/wolftl/dump/mps/SampleMapA.json，events[0].pages[0].list 共 **673 条**；继承块精确索引如下（按 2000550..2000557 扫描）：

### 3.1 菜单与分配

| idx | 命令 | 含义 |
|---|---|---|
| 347 | op111 [1,2000550,2,1] | var550 >= 2 才进入继承 |
| 349 | Message | 「有可以继承的力量。」 |
| 350 | SetLabel 重置 | 重置点标签 |
| 351 | var551 = 0 | 使用点数清零 |
| 352-357 | SetString 3000016..3000021 | 遗物选项文案：强大 / 聪慧 / 可以信任的家人 / 色情的妹妹 / 拥有金钱 / 同伴的LV |
| 358 | op111 [1,2000377,2,1] | 若 var377(自動回復)>=2：清 var370/371/377/331=0、var13(最大行動力)-=10、var102(発情度)=0（清上一周目练度增量） |
| 366 | var552 = 0（flag 65536/参5 特殊清空） | 强さ初值 |
| 367 | op121 [2000551,2000550,2,12290] → var551 = var550 / 2 | **可用点数 = floor(引き継ぎポイント / 2)** |
| 368 | op111 [17,2000551,2,1] | var551 >= 2 才出现隐藏第 7 项「强韧的肉体」 |
| 373 | SetLabel 继承 | 菜单循环标签 |
| 375 | op111 [17,2000551,1,1] | var551 >= 1 时弹菜单 |
| 379 | op102 9 选项 | 强大 / 聪慧 / 家人 / 妹妹 / 金钱 / 同伴LV / 强韧的肉体 / 重置 / 不继承 |

选择分支（op401 ChoiceCase 编号 = 选项号 + 1）：

| 选项 | idx | 分配动作 |
|---|---|---|
| 强大 (case2) | 381 | var552 += 30 |
| 聪慧 (case3) | 396 | var553 += 40 |
| 可以信任的家人 (case4) | 411 | var554 += 50，且 var11(ギルド評判) += 2 |
| 色情的妹妹 (case5) | 427 | var555 += 50，且 var102(発情度) += 20 |
| 拥有金钱 (case6) | 443 | var556 += 10000 |
| 同伴的LV (case7) | 458 | var557 += 8 |
| 强韧的肉体 (case8) | 472-479 | var370/371/377/331 = 2（赋值），var13 += 10，var551 -= 1 |
| 重置 (case9) | 480-481 | JumpLabel「重置」，回到 idx350 重来 |
| 不继承 (case10) | 482-492 | var324(バック)=0；若 var377>=2 再清 var370/371/377/331=0、var13-10、var102=0；var552=0；跳「继承处理结束」 |

每次普通选择后：idx494 var551 -= 1，idx495 跳回「继承」菜单循环（var551 用尽后自动退出）。

### 3.2 结算表（继承最后一步，全部 op121 flag=256 即 +=）

| idx | dst（可读名） | += src |
|---|---|---|
| 498 | var170 攻撃技術 | var552 引き継ぎ強さ |
| 499 | var173 防御技術 | var552 |
| 500 | var174 根性 | var552 |
| 501 | var172 技術 | var552 |
| 502 | var171 賢さ | var553 引き継ぎ賢さ |
| 503 | var10 スキルPT | var553 |
| 504 | var21 信頼 | var554 引き継ぎ信頼 |
| 505 | var22 性興味 | var555 引き継ぎ興味 |
| 506 | var7 所持金 | var556 引き継ぎ所持金 |
| 507 | var41 カナLV | var557 引き継ぎ仲間 |
| 508 | var43 ユイLV | var557 |
| 509 | var45 カスミLV | var557 |

结算完成后 idx510 续开场剧情；之后 idx591 起是「難易度選択」（是啊/NORMAL、我觉得可以再拼一点儿/HARD，idx613 var216=1 记 HARD）——与 NG+ 相互独立，首次游玩也会问。

**变量名对照**（work/_sdb_names.json type[14]「通常変数名」，index=变量号）：var7 所持金、var10 スキルPT、var11 ギルド評判、var13 最大行動力、var21 信頼、var22 性興味、var170 攻撃技術、var171 賢さ、var172 技術、var173 防御技術、var174 根性、var550 引き継ぎポイント、var551 使用ポイント、var552 引き継ぎ強さ、var553 引き継ぎ賢さ、var554 引き継ぎ信頼、var555 引き継ぎ興味、var556 引き継ぎ所持金、var557 引き継ぎ仲間。

## 4. 重构版（Rust）实现现状

结论：**NG+ 由泛型解释器实现，语义上可达**；wolf-game 不硬编码游戏逻辑，因此“是否实现 NG+”= “能否正确执行零售脚本 + 正确做存档单变量读写”。

逐点核对：

- **入口/载入**：Game::boot()（crates/wolf-game/src/game.rs:750）跑 TitleMap.mps（BOOT_MAP game.rs:58，BOOT_EVENT=2 game.rs:61）的 event 2；脚本内 op221 逐个从 save/system.sav 读 normal var，包括 var550。op221 解码在 crates/wolf-vm/src/exec.rs:220..=222（SaveOp），写回 destination 在 exec.rs:2145；Game::save_load 的 op221 走 crate::savegame::read_save + save_address，其中 2_000_000..=2_999_999 => save.normal_var(addr-2_000_000)，故 address 2000550 → normal_var(550) **成立**。
- **标题/新游戏链**：op130 传送入队 → frame 处理（game.rs:1464-1500）→ switch_map → start_pages；SampleMapA ev0 为 autorun（trigger=1），MapRuntime::start_pending（crates/wolf-game/src/map_runtime.rs:1224）对 trigger.is_automatic() 自动启动。见既有测试 crates/wolf-game/tests/m7_new_game_entry.rs（boot → title → 选 1 → 两跳 → SampleMapA，证明零售链路可跑）。
- **继承菜单/分配/结算**：op111（VariableCondition）、op401（ChoiceCase）、op102（Choices）、op121（SetVariable，含 +=/除法/清空 flag）均在 VM 实现，SampleMapA ev0 原样执行。
- **回写 System.sav**：op220 Save（mode != 1）→ SaveRequest::Save(path) → Game::save_to（game.rs:533-617）：以 base_save 为模板，写 part2/part3（**0..800 normal vars = vm.vars.get(index)**）/common self/part5/vardb，to_bytes_rebuilt 落盘。故 CE436 的 op220 会把更新后的 var550 持久化进 Save/System.sav（save_path game.rs:1786：Path 目标 = root.join(path)）。
- **Game::new(root)**（game.rs:356）读 root/Save/System.sav 作 base_save 模板（不 seed VM）；缺失时用 new_game_template_with_vardb 合成。

**此前缺失**：没有专门测试覆盖 NG+（crates/wolf-game/tests/ 下原先无 m13_*.rs）。本次新增 crates/wolf-game/tests/m13_ngplus.rs：3 个快速数据/存档断言 + 2 个 #[ignore] 驱动式长测（--ignored 运行）。

## 5. 验证方式

crates/wolf-game/tests/m13_ngplus.rs：

- 默认快速（不跑解释器，仅在数据缺失时 skip）：
    1. ce436_recomputes_and_persists_ngplus_points — 解析 BasicData/CommonEvent.dat，断言 CE436 标题含「引き継ぎ」、68 条命令、三个存档路径、单变量读 2000550、两次 op220、以及读取的成就变量集合。
    2. sample_map_a_event0_is_autorun_and_has_the_inheritance_table — 解析 MapData/SampleMapA.mps，断言 ev0 trigger==1、var550>=2 门槛、9 选项菜单、6 条分配增量、12 条结算 dst += src。
    3. retail_system_sav_carries_ngplus_points — 读 Save/System.sav，断言 var550 >= 2（真实 NG+ 数据）、var551 == 0（瞬态不落盘）。
- #[ignore] 长矩阵：
    4. boot_loads_ngplus_points_from_system_sav — boot 零售标题链，断言 VM 的 var550 等于 System.sav 里的值（证明 TitleMap ev2 的 op221 载入）。
    5. new_game_inheritance_menu_applies_points — 置 var550=2，直接启动 SampleMapA ev0，auto 应答至 9 选项继承菜单，断言 var552>=30 且结算把 var552 折进 var170。

运行：cargo test -p wolf-game --test m13_ngplus（快测）；cargo test -p wolf-game --test m13_ngplus -- --ignored（长矩阵）。

**本机实测（数据根 work/full，HEAD cbbb14d 工作树）：** 3 个快测全部通过；2 个 #[ignore] 长测全部通过——

- boot_loads_ngplus_points_from_system_sav：boot 后 VM 的 var550 == work/full/Save/system.sav 的 2。
- new_game_inheritance_menu_applies_points：置 var550=2 → 启动 SampleMapA ev0 → 到达 9 选项继承菜单 → auto 选「强大」→ var552=30 → 结算 var170 增加 30。

即 **重构引擎在生产路径上确实能跑通 NG+ 继承**（非仅静态可解析）。

## 6. 真实存档样本（retail 确有 NG+ 数据）

用独立 rustc 程序（work/out/savdump.rs，链接预构建 libwolf_save）解析 wolf_save::SaveFile 后读 normal_var(i)：

| 文件 | playtime(min) | var550 | 备注 |
|---|---|---|---|
| work/full/Save/system.sav | 24 | **2** | 测试数据根；var552=30（曾分配 1 次强大） |
| work/full/Save/SaveData01.sav | 121 | 2 | |
| work/all/Save/system.sav | 829 | **5** | |
| work/all/Save/SaveData01.sav | 281 | 15 | var531=2 |
| work/retail_backup/Save/System.sav | 281 | **15** | 与 SaveData01/08 值一致；var531=2, var554=200 |
| retail_backup/Save/SaveData01.sav | 277 | 15 | |
| retail_backup/Save/SaveData02.sav | 20 | 0（无） | 早期（var160=2），无 var550 |
| retail_backup/Save/SaveData03.sav | 231 | 8 | |
| retail_backup/Save/SaveData04.sav | 275 | 9 | |
| retail_backup/Save/SaveData05.sav | 278 | 9 | |
| retail_backup/Save/SaveData06.sav | 131 | 9 | var554=200 |
| retail_backup/Save/SaveData07.sav | 185 | 9 | var531=1, var554=200 |
| retail_backup/Save/SaveData08.sav | 281 | 15 | = System.sav |

读法：var550 >= 2 → 进入继承；var551 = floor(var550/2) → 可用分配次数。零售 System.sav var550 = 15 ⇒ 7 点可分配的 NG+ 状态真实存在，无需合成即可验证。（var554=200 是历史遗物分配残留，无碍；SaveData02 是首次通关前的早期档。）

## 7. 剩余风险 / 待决

1. **跨平台路径大小写**：CE436 与 TitleMap 用 save/system.sav（目录小写 s、文件小写），而实际数据目录是 Save/（大写 S）、文件 system.sav。Windows 大小写不敏感可通；Linux 大小写敏感时 save/... 会读/写失败（read_save/save_to 的 root.join 直接拼路径）。建议要么规范化脚本路径大小写，要么在 save_path 层做大小写不敏感解析。目前引擎无此归一化。
2. **base_save 缺失时的合成模板**：Game::new 若读不到 Save/System.sav，用 new_game_template_with_vardb 合成，此时 VM 不会 seed 任何存档变量（含 var550），TitleMap ev2 的 op221 因无模板/文件而返回 0 → NG+ 在全新数据目录下无法触发（这是正确行为，但需注意：真实“已有二周目”场景必须有 System.sav）。
3. **CE436 持久化门槛**：var531(アカツキホモイベ)>=3 才走 op221/op220 回写块；var531 < 3 时 CE436 只算不写。零售 System.sav 的 var550=15 与 var531=2 并存，说明点数也可经常规整档保存持久化，不依赖这条特殊路径——重构版同样经 save_to 整档写 part3 而持久化。此点建议 Lead 记入“存档完整性”说明。
4. **长矩阵依赖具体数据根**：#[ignore] 测试 4 依赖 work/full/Save/system.sav 存在且 var550>=2（该 fixture 为 2）；测试 5 用 var550=2 合成状态跑通继承菜单（不依赖存档）。若数据根切到无 var550 的目录，测试 3/4 会如实失败/跳过，需以数据根为准。
5. **继承菜单前有 6 段剧情问答**（SampleMapA ev0 idx61/100/125/153/178/202，各 4 选项）。new_game_inheritance_menu_applies_points 用 auto_choice（cursor 0）一次性穿过，若将来剧情分支改变导致无法到达 9 选项菜单，长测会暴露；这属于可接受的回归护栏。
