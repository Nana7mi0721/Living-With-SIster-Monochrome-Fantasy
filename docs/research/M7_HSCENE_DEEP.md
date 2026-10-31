# W2-d 深 H 分支（CE333 / CE346）真实存档可达性

> 任务：task-40。在不伪造任何状态的前提下，用真实存档（SaveData01..08）把 CE246「スキンシップ」推进到深 H 分支，记录确切门槛变量，尝试到达 CE333 / CE346，或精确证明阻断点。
> 结论：**CE346（H カットイン 実行処理）可从真实 SaveData08 端到端到达并真实绘制可见 cutin；CE333（深夜パート失敗ルート / 妹爆発 BADEND）在任何真实存档下都不可达，确切阻断变量是 v23==0（且 v157!=13），而 8 个零售存档的 v23 全部 >= 6。**
> 交付物：`crates/wolf-game/tests/m7_hscene_deep.rs`（回归测试，2 passed）、本文档。未改任何生产代码（无需 hook），未提交。

## 0. 结论速览

| 目标 | 真实存档可达？ | 证据 | 确切门槛 |
|---|---|---|---|
| CE346 カットイン（`□Hカットイン実行処理昼`） | **是**（SaveData08，裸体路径） | CE346 实例存活 420 帧、maxpc=180；屏幕出现可见 `cutin/cutin_211.png`、`cutin/cutin_161.png`（opacity=255）；warnings=0 | 见 §2 门槛链（v21≥200, v22≥100, v102≥40, 场景自身把 v18 推到 80…） |
| CE333 BADEND（`深夜パート失敗ルート` / `妹爆発`） | **否**（所有真实存档） | 真实 Save08 走成功分支（var166=0、无 explosion）；只有把 v23 强制为 0 才进入坏结局（var166=1、出现 `cg/explosion*`） | **v23==0 且 v157!=13**；零售 v23 全部 ≥6 |

## 1. CE246 深分支结构（`work/all` BasicData/CommonEvent.dat，CE id=246）

CE246 共 2062 个指令槽。入口选择与进入深层的门槛（槽号=扁平 pc）：

- `pc1 op102[6]` 光选择：`摸摸头/揉肩/拥抱/亲吻/挠痒痒/性骚扰`。`ChoiceReply::Index(i)` → `case_target(i+2)`（`crates/wolf-vm/src/exec.rs:1046`）。性骚扰=index5→`CASE7 pc556`。
- `pc557 op111[1, 2000020, 60, 3]` 若 `v20<=60` → 礼貌拒绝并结束（pc560）；因此必须 **v20>60**。
- `pc564 op111[1, 2000151, 1, 1]` 若 `v151>=1` → 结束；真实存档 v151 全 0，通过（只有 SaveData06 v20=60 卡在 pc560）。
- `pc572 op102[5]` 深选择：`让我看内衣 / 让我看裸体 / 想看你自慰 / 让你看我撸管 / 小鸟飞飞`；index i → `CASE(i+2)`：内衣=CASE2 pc573，裸体=CASE3 pc804，自慰=CASE4 pc1015，撸管=CASE5 pc1423，小鸟飞飞=CASE6 pc1941。

## 2. CE346 可达（真实 SaveData08，裸体路径）— 门槛链与证据

深选择「让我看裸体」= CASE3（pc804 起）。到 CE346（pc906）的完整门槛链，逐槽如下（`work/_ce_full.json` CE246）：

| pc | 指令 | 说明 |
|---|---|---|
| 804 | `op401 CASE[3]` | 裸体分支入口 |
| 806 | `op111[17,2000021,200,1]` | **v21>=200** |
| 808 | `op111[17,2000022,100,1]` | **v22>=100** |
| 812 | `op111[2,2000023,7,1,2000023,0,1]` | v23>=7 或 v23>=0（恒真） |
| 822 | `op111[2,2000070,0,2,2000070,1,2]` | v70==0 或 ==1（恒真） |
| 826/844 | `op111[2,2000022,200,1,2000022,100,1]` | v22>=200 或 >=100 |
| 866 | `op111[1,2000022,300,1]` | **v22>=300** 进入加码块（SaveData08 v22=375 命中） |
| 879 | `op111[1,2000018,50,1]` | v18>=50 |
| 881 | `op111[1,2000016,1,1]` | v16>=1 |
| 883 | `op102[18] {让我看更多些,就先到这里}` | 选 index0「让我看更多些」→ CASE2 pc884 |
| 885 | `op111[17,2000102,40,1]` | **v102>=40** |
| 892 | `op111[17,2000018,80,1]` | v18>=80；场景内 `op210 CALLEV 500496` 等会把 v18 推到 80（实测 v18:65→80） |
| 894 | `op103 オナニー` | 内部分支注释 |
| 898 | `op102[17] {看破！}` | 选 index0「看破！」→ CASE2 pc899 |
| 906 | **`op210[500346,3,24,1,1]` = 调用 CE346** | H カットイン 実行 |

SaveData08 的真实初值（本路径关键值）：`v21=688, v22=375, v102=120, v18=60→场景推到80, v16=2, v20=90, v70=1, v151=0`，全部满足上述门槛，**无需任何伪造**。

实测（`drive(8, 深选项=1裸体, 不强制任何变量)`）：
- 深选择菜单到达：true。
- CE346 实例出现：true，存活 **420 帧**，maxpc=**180**（`□Hカットイン実行処理昼` 自身程序跑完一圈）。
- 屏幕可见 cutin：`(5000, "cutin/cutin_211.png", opacity=255)`、`(7100, "cutin/cutin_161.png", opacity=255)`。
- warnings = 0。

附带：**SaveData07 的「让你看我撸管」路径（深选项=3）亦可到达 CE346**（撸管分支 CE346 位于 pc1799，实测 ev346 实例出现）。即深分支 CE346 的可达性不唯一，但 SaveData08 已足够。

## 3. CE333 精确阻断 — 需要 v23==0，而所有真实存档 v23>=6

小鸟飞飞 = CASE6（pc1941 起）。到 CE333 的判定结构：

| pc | 指令 | 说明 |
|---|---|---|
| 1941 | `op401 CASE[6]` | 小鸟飞飞分支入口 |
| 1943 | `op111[17,2000157,13,2]` | **v157==13** → CASE1 pc1944（成功支线，无 CE333） |
| 1950 | `op420 ELSE` | v157!=13 时进入 |
| 1951 | `op111[3, 2000023,8,1, 2000023,2,1, 2000023,1,1]` | 三个成功 case：v23>=8 / v23>=2 / v23>=1；**只要 v23>=1 就匹配某个 case**（成功支线，无 CE333） |
| 1968/1974 | `op401 CASE[2]/CASE[3]` | 其余成功子支线（均含 `op121 2000023,-1` 递减 v23） |
| 1980 | `op420 ELSE` | **三个 case 全不匹配（即 v23==0）才到这里** |
| 1981 | `op150[...,6000,...,8300] cg/blackscreen.jpg` | 黑屏 |
| 1982 | `op121[2000166,1,0,0]` | **var166=1（CE333 分支唯一可检测写入）** |
| 1983 | **`op210[500333,4,2,1,0,3]` = 调用 CE333** | |
| 1984 | `op103 深夜パート失敗ルートへ` | 坏结局 |
| 1985-2029 | `op150 cg/explosion01..13` + `BADEND ～妹爆発～` + `op174` 游戏结束 | 爆炸 CG |

即 CE333 的进入条件是：**v157 != 13 且 v23 == 0**。

真实存档 v23（`SaveFile::parse(...).normal_var(23)`，测试内逐个读取并断言 ≥6）：
SaveData01..08 = `9, 9, 10, 6, 6, 9, 8, 9`，**最小值 6，无任何存档为 0**。v157 仅 Save07==13，其余 !=13。故：
- 真实存档下 v23>=1 恒成立 → pc1951 必中某个成功 case → **永不进入 pc1980 ELSE → CE333 不可达**。

实测（SaveData08 + 小鸟飞飞）：
- 真实变量：`deep=true, var166=0, explosion=false`（成功支线）。
- 仅把 v23 强制为 0（零售无此状态，纯为定位 blocker）：`var166=1, explosion=true`，程序尾迹出现 1984-2029 坏结局区段。

=> **CE333 的确切阻断点 = v23==0 这一缺失状态**；在"不伪造"的约束下无法到达，到此为止。

## 4. 关键方法学陷阱（务必记录）

op210 `CALLEV` 子事件（CE346/CE333 等）**不会写入 `vm.started_events`**（该集合只记录 host/Vm::start_event 主动启动的事件，不记录脚本内 op210 子调用）。因此不能用 `started_events.contains(&346)` 判断子事件是否跑过——这正是最初探针误判"CE346/CE333 皆不可达"的原因。正确判定：
- 运行期用 `game.vm.instances()` 观察 `instance.event == 346/333`；
- 或以屏幕副作用判定：可见 `cutin/cutin_*.png`（CE346）、`cg/explosion*`（CE333）、`var166==1`（CE333 分支标记）。

## 5. 复现

```
# 2 个回归测试
CARGO_TARGET_DIR=work/out/target_ev1 cargo test -q -p wolf-game --test m7_hscene_deep -- --nocapture
# 预期：CE346 deep: deep=true ce346=true maxpc=180 frames=420 cutin=[(5000,"cutin/cutin_211.png",255),(7100,"cutin/cutin_161.png",255)] warn=0
#       CE333 real Save08: deep=true var166=0 explosion=false
#       CE333 v23:=0: deep=true var166=1 explosion=true
#       test result: ok. 2 passed; 0 failed
```

测试实现要点：
- 驱动：`Game::new(data_root)` → `load_save(SaveDataNN)` → `auto_advance=true, auto_choice=false` → `start_event(246)`（`crates/wolf-game/src/game.rs:514`）→ 循环 `frame()`；遇 `game.choices` 用 `game.reply(ChoiceReply::Index(i))`（`crates/wolf-game/src/game.rs:783`，`choice_made` 见 `game.rs:1536`）。
- 数据根：默认 `work/all`（H 资源仅在此），可用 `WOLF_DATA_ROOT` 覆盖。

## 6. 文件清单 / 边界

- 新增：`crates/wolf-game/tests/m7_hscene_deep.rs`（仅测试，未改生产代码）。
- 新增：`docs/research/M7_HSCENE_DEEP.md`（本文）。
- 未提交、未 push。未触碰 `game.rs`/`exec.rs`，未与 task-36（H-mode 状态）冲突；本任务不需要生产 hook。
- 只读数据：`work/all/BasicData/CommonEvent.dat`（经 `work/_ce_full.json` 反汇编）、`work/retail_backup/Save/SaveData01..08.sav`。
