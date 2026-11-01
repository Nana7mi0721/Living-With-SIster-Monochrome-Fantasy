# M13 P0-A — 主线后半与结局探针（mainline endings）

Status: done（2026-11 task-9 复验：修复后自然驱动 SaveData01 与 SaveData08 均可达 stage28→29）。
Deliverable tests in crates/wolf-game/tests/m13_mainline_endings.rs; this document records the
measurement, the two ending paths, and the M13 common-event self-variable persistence fix that
makes the normal-clear path naturally reachable.

## 1. What this proves

1. **逃亡 END 自然可达且有内容**：从 var366=0 的零售存档逐日推进到 day>=100 后，
   早朝调度 CE317 唤 CE305 stage27「期限切れ」（BAD END ～逃亡～），对话正常渲染，零 warning。
2. **正常通关的结局脚本完整存在、可正常渲染**：CE305 stage28「決着」跑完会自动 210 [500305,1,29]
   接 stage29「旅路」，末段给出 “恭喜您完成了整个故事剧情，辛苦了！”，并置 var492=1，零 warning。
   即“体验内容”本身是齐的、与原版一致。
3. **自然驱动下正常通关（修复后已自然可达）**：CE251 工作日「交流战 3 天后」倒计时依赖
   common-event self 变量跨调用持久（self51 4→3→…→0）。旧引擎 spawn_child 每次 210 调用都新建
   全 0 SelfVars，导致 self51 每天复位为 4、永远到不了 0。M13 已把 common-event self 变量做成
   跨调用持久并随存档保存/恢复（见 §6 B2、§10）；修复后从 SaveData01 与 SaveData08（深夜档）
   均自然推进 day91→day95 即递减到 0 并触发 stage28→stage29（stage28 首帧均为 day95）。
4. 附注：CE305 idx3404「エンディング」标签下实测是一段“尖叫胡萝卜”支线小事件，**不是**大结局；
   真正的通关结局在 stage28→29 链。var493 **不是**逃亡旗标（见 §7）。

## 2. Measured environment

- 测量 HEAD（本次复验，task-9）：**1826eddbc178c6fcea8a15cb82e40e257835ea92 (1826edd)**
  “test/docs(m13): content matrix, DLC CE452 and mainline ending probes”，其下含修复提交
  **7c3b728** “fix(vm): persist common-event self variables across invocations (M13)”
  与基线 cbbb14d“docs: close guide content crosswalk”。
- rustc / cargo 1.99.1
- data root: **work/all**（WOLF_DATA_ROOT 可覆盖；缺省回退 work/all）
- **必须 pin 环境**：全量/长测需显式设置 `WOLF_DATA_ROOT=work/all`（另有 `WOLF_GAME_DIR`
  指向 work 数据），否则 wolf-save 会读用户 Steam 安装档而假红。本次运行在 Unix 风格 shell
  以 `WOLF_DATA_ROOT=work/all cargo test ...` 前缀注入，已隔离。
- retail saves: work/retail_backup/Save/SaveData01..08.sav
- CLI: target/debug/wolf-cli.exe
- deliverable: crates/wolf-game/tests/m13_mainline_endings.rs, 本文件（均在 1826edd 已跟踪）

## 3. Retail save inventory (measured via load_save)

| slot | day | var6 | var366 | var367 | var492 | var493 | note |
|---|---|---|---|---|---|---|---|
| 01 | 90 | 5 (周五) | 1 | 1 | 0 | 0 | 最远档；var366=1 已越过“中段” |
| 02 | 2  | 1 (周一) | 0 | 0 | 0 | 0 | |
| 03 | 62 | 5 (周五) | 0 | 0 | 0 | 0 | |
| 04 | 72 | 1 (周一) | 0 | 0 | 0 | 0 | 用于逃亡长跑 |
| 05 | 73 | 2 (周二) | 0 | 0 | 0 | 0 | 用于逃亡长跑 |
| 06 | 53 | 3 (周三) | 0 | 0 | 0 | 0 | |
| 07 | 68 | 4 (周四) | 0 | 0 | 0 | 0 | |
| 08 | 90 | 6 (周六) | 1 | 1 | 0 | 0 | 最远档 |

var6 星期映射：周日=0、周一=1 … 周五=5、周六=6（day2=周一验证）。全部 var298/466/491/492/493/619=0。

## 4. Mechanism — CE305 stage machine

- CE305（common/305_メインシナリオ.json）按 self0 分派的大 switch：
  210 [500305,1,N] 的第二实参 N 写到子实例 self 的 SELF_ARG_BASE=600+i，
  即 stage = CE305实例.self_vars.get(600)（magic 1_600_000 = self 组6 slot0 = 扁平索引 600）。
- 顶层 111 [3,1600000,…] 与内层 111 [3,1600000,25/26/27…]、[3,1600000,28/29/30…] 完成分派。
- **stage 调用点全表**（common dump 实扫 code210 intArgs[0]==500305）：
  - CE251 平日：stage28(idx400)、3(426)、6(443)、5(456)、13(489)、18(505)、19(540)、21(742)、22(746)、30(913)
  - CE256 讨伐：stage26(idx103)
  - CE301 ランダム：stage1/2/4/7/8/9/10/11/12/13/14/15/17/20/23
  - CE314 街へ：stage16
  - CE317 早朝：stage24(idx514)、stage27(idx926)
  - CE305 自调：stage25(idx2075/2288/2568/2845)、stage29(idx3294，stage28 尾)
- 结局门（111，op 映射 exec.rs:1493 compare：0 大于 1 大于等于 2 等于 3 小于等于 4 小于 5 不等于）：
  - **stage28「決着」** CE251：var367>=1 → var160>=90 → var366==1 → var6<=2 → self65==0（见 §6 关于 self）
  - **stage27「期限切れ」逃亡** CE317：var160>=100 → var366==0 → stage27
  - **stage30「エンディング」** CE251：var366==0 → 85<=var160<=100（注：内容为胡萝卜支线）
- var366 三态：0=未定（新游戏默认）、1=中段（save01/08）、2=決着后；写点仅 CE251 idx399→2、CE305 idx3140→1。

## 5. 路径 A · 逃亡 END（自然驱动，var366=0）

- 存档：SaveData04（day72 周一）或 SaveData05（day73 周二），var366=0、var367=0。
- 驱动：resume(slot)（load_save → start_engine_events → start_pages(map, skip=0) 跳过 event0 ニューゲーム処理，
  不重置 day）→ 逐日 play_day + sleep_to_next_morning。
- 观测（slot5，M13_SLOT=5）：
  - day73→day100 每天 18:00–19:00 结束，var366 恒 0，CE305 stage 未出现。
  - **day101 07:45 起床 → 08:30 早朝 CE317 触发 CE305 stage27**（pc=3157），
    对话“咦，由香莉姐？/小镇一带已彻底被黑雾笼罩…逃得远远的/时间期限到了。/ **BAD END　～逃亡～**”。
  - **var160=101、var366=0、var492=0、warnings=0**；stage27 首帧 f78（day101 08:30）。
- 结论：逃亡 END 的“日数到 100 + var366=0”门真实成立，结局内容完整、可渲染。

## 6. 路径 B · 正常通关（结局脚本完整，但自然驱动卡在 self 变量）

### B1. 直接注入 CE305 self0=stage 渲染（证明内容齐）
run_injected_stage(stage)：Game::new → load SaveData01 → vm.start_event(305) →
instance_mut(id).self_vars.set(600, stage) → auto 跑完。实测：

| stage | frames | 结局旗标 AFTER [298,466,491,492,493,619] | warnings | 首帧/内容 |
|---|---|---|---|---|
| 28 決着 | ~5148 | [0,0,0,1,0,0]（var492=1） | 0 | f0；progression [(28,0),(29,1826),(10,5097)]；尾段“恭喜您完成了整个故事剧情，辛苦了！” |
| 29 旅路 | 3317 | var492=1 | 0 | 单独注入亦置 var492=1 |
| 27 期限切れ | 142 | [0,0,0,0,0,0] | 0 | f0；BAD END ～逃亡～ |
| 30 エンディング | 699 | [0,0,0,0,0,0] | 0 | 尖叫胡萝卜支线，非大结局 |

⇒ stage28「決着」尾部 210 [500305,1,29] 自动把 stage29「旅路」挂起并跑完，
给出通关贺词——**主线通关的结局内容完整、与原版一致**。

### B2. 自然驱动（修复后已可达）
修复前：从 SaveData01（day90）逐日推进，CE251 每个工作日结束即被回收、次日新起，旧 spawn_child
用全 0 SelfVars，self51 每天复位 4、self65 复位 1，永远到不了 0，stage28 不可达。
修复（crates/wolf-vm/src/exec.rs 的 common-event self 持久库 + crates/wolf-game 的存档保存/恢复）后，
自然驱动 SaveData01（M13_SLOT=1）：

    day91(周六 v6=6): CE251.self51 max=0（var6<=2 gate 未过，未递减）
    day92(周日 v6=0): CE251.self51 max=4  self65=1
    day93(周一 v6=1): CE251.self51 max=3  self65=1
    day94(周二 v6=2): CE251.self51 max=2  self65=1
    day95(周三 v6=3): CE251.self51 max=1  → 当天递减到 0 → CE251 idx400 唤 stage28

- 结果（SaveData01，M13_SLOT=1，2.41s）：start_day=91，days_driven=5，frames=8966；
  **stage28 首帧 = (day95, play_day 内 f164)**；逐帧 stage 采样确证 28 与 29 都出现
  （stage 集合 {10,28,29}；stage28 父实例阻塞等 stage29 子实例，多帧共存）；var366 1→2；
  **var492=1、warnings=0**；结尾实测抓到贺词原文
  “恭喜您完成了整个故事剧情，辛苦了！／请继续享受妹相随生活！”。倒计时逐日 4→3→2→1→0，不再复位。
- 关键不变量：load→save 且未游玩时 common_self 无 dirty，part3 逐字节不变（m12 8/8 保持）。
- 根因与修法：见 §10；exec.rs 的 common_self 持久库在 reap_finished/stop_instance 回写并标 dirty，
  game.rs save_to 写 dirty∪live 到 part3，savegame.rs apply 从 part3 逐 event 播种。

### B3. SaveData08 深夜档（day90 00:42）也能自然推进
SaveData08 存在 **day90 00:42**，此刻停在一个交互场景（H 场景：屏幕 pic 292“0%”、
pic 293“兴奋”、pic 294 `Picture/endbutton.png`），v62=1；纯喂 OK 不推进时钟。处理方式
（新 helper `reach_next_morning`）：
1. 若 hour<5（深夜）且屏幕存在 pic 294（结束按钮），先 `click_picture(294)` 关闭该场景（v62→0，界面清空）；
2. 再连续喂 OK，引擎自然过渡到 **day91 07:45**（未改任何存档变量，纯输入驱动）。

之后与 SaveData01 完全同一套逐日 play_day/sleep 逻辑，结果（M13_SLOT=8，2.77s）：
start_day=91、days_driven=5、frames=9264、**stage28 首帧 (day95,f164)**、stage 集合 **{11,28,29}**、
var366 1→2、**var492=1、warnings=0**、同样抓到“恭喜您完成了整个故事剧情”贺词。
⇒ **两张最远零售档（SaveData01 与 SaveData08）在修复后都能无需注入自然通关。**

## 7. 变量陷阱（避免误判）

- **var493 不是逃亡旗标**。写点仅两处：CE305 idx3301（stage29 旅路区内，置 1）、CE405「妊娠END」idx28（置 1），
  以及 CE305 idx3502 清 0。stage27 注入后 var493/492 全 0（见 §6 表），故逃亡 END 不写任何结局旗标。
  长跑中曾观察到 var493 在到达逃亡前已为 1（背景事件写入），**不能**用它判定逃亡。
  逃亡 END 的唯一可靠判据 = CE305 stage27 被调度（+ BAD END 文本）。
- var492 由 CE305 idx3503 置 1（stage30/種を植えよう 区），在 stage28→29 链的注入跑中亦被置 1。
- var466 由 344/347 射精命令周期性置 0/1，与结局无关。

## 8. Self 变量索引

SelfVars magic 1_000_000 + group*100_000 + slot → 扁平 group*100 + slot。
故：self0 = get(600)（stage）、self10 = get(610)、self51 = get(651)、self52 = get(652)、self65 = get(665)；
210 实参基址 SELF_ARG_BASE = COMMON_SELF_BASE = 600。

## 9. 复现命令（repo 根）

    # 默认测试（快，~0.6s，3 passed / 2 ignored；零售存档缺失时 skip，不失败）
    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m13_mainline_endings

    # 逃亡 END 自然长跑（~16s；断言 day101 触发 CE305 stage27、warnings==0）
    WOLF_DATA_ROOT=work/all M13_SLOT=5 cargo test -q -p wolf-game --test m13_mainline_endings drive_escape_path_from_retail_save -- --ignored --nocapture

    # 正常通关自然驱动 · SaveData01（~2.4s；断言 self51 递减→0、stage28→29、var492=1、贺词、warnings=0）
    WOLF_DATA_ROOT=work/all M13_SLOT=1 cargo test -q -p wolf-game --test m13_mainline_endings drive_normal_path_diagnostic -- --ignored --nocapture

    # 正常通关自然驱动 · SaveData08（深夜档，~2.8s；先结束 H 场景再推进，断言同上）
    WOLF_DATA_ROOT=work/all M13_SLOT=8 cargo test -q -p wolf-game --test m13_mainline_endings drive_normal_path_diagnostic -- --ignored --nocapture

默认测试清单（3 个，均快）：
- true_ending_script_renders_from_stage28：注入 stage28，断言进 28、自动接 29、var492=1、warnings=0。
- escape_ending_script_renders_from_stage27：注入 stage27，跨帧累积文本断言出现 “BAD END”+“逃亡”、warnings=0。
- stage30_is_side_event_not_grand_ending：注入 stage30，断言文本含“胡萝卜”、不含“恭喜”、var492 仍为 0
  —— 明确 stage30 是支线、非大结局，避免夸大。
（drive_escape_path_from_retail_save / drive_normal_path_diagnostic 为 #[ignore] 长跑；后者用
 M13_SLOT 选档、M13_DAYS 选天数，默认 slot=1、days=30。）

## 10. 结论（对 2026-11-01 口径）

- **逃亡结局**：体验内容与原版一致、自然可达（day>=100 且 var366=0）。OK
- **主线通关结局内容**：stage28 決着 → stage29 旅路 → 恭喜完成 脚本完整、可渲染、置 var492=1。OK（内容层面一致）
- **主线通关自然可达性**：修复后 **自然可达（两张最远档均已实测通过）**。M13 将 common-event
  self 变量做成跨调用持久并随存档保存/恢复（crates/wolf-vm/src/exec.rs 的 common_self 持久库 +
  dirty 回写；crates/wolf-game/src/game.rs save_to 与 savegame.rs apply 的 part3 保存/播种）。
  - **SaveData01**（day90 21:40）：自然驱动 day91→day95（days_driven=5、frames≈8966），
    self51 4→3→2→1→0，stage28 首帧 (day95,f164)，触发 stage28→29，var492=1、warnings=0。
  - **SaveData08**（day90 00:42 深夜 H 场景）：先点 pic 294 结束场景再推进到 day91 晨，
    同样 day91→day95（frames≈9264），stage28 首帧 (day95,f164)，stage 集合 {11,28,29}，
    var492=1、warnings=0。两档均实测抓到“恭喜您完成了整个故事剧情”贺词。
  - 配合 load→save 未游玩时 part3 逐字节不变（m12 8/8），存档一致性未破坏。
- **API 分层（实现注记）**：持久路径 = 脚本 opcode 210 调用（exec.rs spawn_child 从 common_self 播种，
  结束后 reap/stop 回写并标 dirty）、save load 的 seed_common_self、以及需要显式续用的 Vm::start_event_resuming。
  引擎自有合成协程（重建战斗驱动的 CE241/CE242，数据中没有任何 210 500241/500242 引用）用
  Vm::start_event_fresh 启动：读空白 SelfVars、persistent_self=false、结束不入库不标 dirty，逐轮行为与
  修复前逐位一致。普通 Vm::start_event 默认是持久语义（从 common_self 播种、结束回写），只有合成驱动显式调用 start_event_fresh。
- 附注（非本次范围，供 Lead）：CE305 idx3404「エンディング」标签实为胡萝卜支线；长跑中 var493 背景置 1 待 triage。
