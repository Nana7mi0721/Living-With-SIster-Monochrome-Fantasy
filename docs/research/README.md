# 研究文档索引（docs/research/README.md）

> 本目录保存**一次性调查、探针结果与验收底稿**。它们不是当前指令；结论以对应的 `docs/HANDOFF_M13.md`、技术规格和现行代码/测试为准。
> 新增研究文档时：文件名用 `Mx_主题.md`（或 `主题.md`），写清测量 HEAD/数据根/复现命令，并登记到本表。

## 推荐入口（想快速了解某个结论）

- 最新综合：[`../HANDOFF_M13.md`](../HANDOFF_M13.md)
- 完整路线：[`M9_FULL_ROUTE.md`](M9_FULL_ROUTE.md)（标题→day5 08:30）
- opcode 111 落地：[`M10_OPCODE_111_LANDING.md`](M10_OPCODE_111_LANDING.md)、[`M10_REBASELINE_REVIEW.md`](M10_REBASELINE_REVIEW.md)
- 战斗/H/地图边界：[`M9_BATTLE_ROUND_GAP.md`](M9_BATTLE_ROUND_GAP.md)、[`M9_BATTLE_LIVE_BRIDGE.md`](M9_BATTLE_LIVE_BRIDGE.md)、[`M9_HSCENE_LOOP.md`](M9_HSCENE_LOOP.md)

## 按里程碑索引

### 基础与方法（4 篇）

跨里程碑的格式逆向、字节码交叉验证和外部调研研判。

| 文档 | 内容 |
| --- | --- |
| [`GPT_RESEARCH_TRIAGE.md`](GPT_RESEARCH_TRIAGE.md) | GPT 外部调研报告 · 综合研判与落地对照（GPT_RESEARCH_TRIAGE） |
| [`GUIDE_CONTENT_CROSSWALK.md`](GUIDE_CONTENT_CROSSWALK.md) | 10 份玩家攻略（151 页）↔ 零售脚本内容对照：变量/译名索引、主线日数门与 CE305/CE452 章节、111 条随机事件定位、H 阈值 vs 脚本门、物品/技能/CG 覆盖清单、冲突与笔误 |
| [`wolf-bytecode-crosscheck.md`](wolf-bytecode-crosscheck.md) | WOLF RPG Editor 3.5 事件字节码语义交叉验证报告 |
| [`wolf-decompile-feasibility.md`](wolf-decompile-feasibility.md) | 完整逆向 WOLF RPG Editor 事件逻辑并生成可读伪代码：可行性与实施报告 |

### M6（7 篇）

主线贯通、日常循环、日程与夜间流程的现场记录（历史）。

| 文档 | 内容 |
| --- | --- |
| [`CE328_ABORT.md`](CE328_ABORT.md) | CE328 map-context abort — root cause (M6 night flow) |
| [`EV1_DAY_CLICK.md`](EV1_DAY_CLICK.md) | EV1 昼之妹クリック判定 — page0 的激活条件、命中框与 headless 可行性 |
| [`EV1_NIGHT_SISTER.md`](EV1_NIGHT_SISTER.md) | EV1 夜之妹クリック判定 — page2 的激活条件与 headless 可达性 |
| [`M6_BATTLE_COMMANDS.md`](M6_BATTLE_COMMANDS.md) | M6 · 战斗命令可选性 — CE152 真实命令封锁的精确断点与修复（task-13） |
| [`M6_BATTLE_REAL.md`](M6_BATTLE_REAL.md) | M6 — REAL Battle Line: reachable boundary & minimal gap |
| [`M6_NIGHT_FLOW.md`](M6_NIGHT_FLOW.md) | M6 Night Flow — natural route from 19:30 day2 bedroom → night menu CE245 → sleep? |
| [`M6_SCHEDULE.md`](M6_SCHEDULE.md) | M6 日程审计：day4 08:30 卡点的精确门、day4→day5 的非战斗路径、以及战斗命令槽初始化缺口 |

### M7（29 篇）

第一批并行工作流的调查与验收底稿（历史）。

| 文档 | 内容 |
| --- | --- |
| [`M7_BACKLOG.md`](M7_BACKLOG.md) | M7 待办清单（Post-M6 审计） |
| [`M7_BATTLE_FULL.md`](M7_BATTLE_FULL.md) | M7 W1 (task-20) — Real CE244 settlement: the reachable command-phase round and the exact menu-entry blocker |
| [`M7_BATTLE_MENU_DB.md`](M7_BATTLE_MENU_DB.md) | M7 battle command-menu database: the 万能ウィンドウ labels (W1-d) |
| [`M7_BATTLE_PLAYABLE.md`](M7_BATTLE_PLAYABLE.md) | M7 · W1 battle playable — rebuilt command-phase wiring (task-34) |
| [`M7_BATTLE_ROUND.md`](M7_BATTLE_ROUND.md) | M7 W1-a — Real battle round to CE244: reachable depth and the exact blocker |
| [`M7_BATTLE_SEED.md`](M7_BATTLE_SEED.md) | M7 — Battle database seed (`Game::seed_battle_databases`) |
| [`M7_BATTLE_TARGET_SCHEDULER.md`](M7_BATTLE_TARGET_SCHEDULER.md) | W1 battle support — CE190 target generation and the 2000350 scheduler |
| [`M7_BATTLE_TARGET_SELECT.md`](M7_BATTLE_TARGET_SELECT.md) | M7 battle — production enemy-count seed and CE162 target selection |
| [`M7_CE244_DIRECT.md`](M7_CE244_DIRECT.md) | M7 W1 — CE244 直接结算 harness（绕过调用图） |
| [`M7_CHOICE_WINDOW.md`](M7_CHOICE_WINDOW.md) | M7 W5 · 运行时选择窗口在 headless capture 里可见（H-MENU-1） |
| [`M7_DAY4_PLAYABLE.md`](M7_DAY4_PLAYABLE.md) | M7 task-38 — Day-4 real battle: exact blocking edge (no-injection audit) |
| [`M7_DAY5_SCHEDULE.md`](M7_DAY5_SCHEDULE.md) | M7 — Day-5 schedule audit & reachability |
| [`M7_HAUTO_EVENTS.md`](M7_HAUTO_EVENTS.md) | M7 W2 — H 自动事件（run_condition==2）调度与可见性验证 |
| [`M7_HMODE_STATE.md`](M7_HMODE_STATE.md) | M7 H-mode / intimacy state (task-36) |
| [`M7_HOST_STATE.md`](M7_HOST_STATE.md) | M7 主机侧 124 查询残缺字段（H-124 后续） |
| [`M7_HSCENE_ADVANCE.md`](M7_HSCENE_ADVANCE.md) | M7 W2-a — Advancing an H scene interactively through its opcode-123 polls |
| [`M7_HSCENE_DEEP.md`](M7_HSCENE_DEEP.md) | W2-d 深 H 分支（CE333 / CE346）真实存档可达性 |
| [`M7_HSCENE_ENTRY.md`](M7_HSCENE_ENTRY.md) | M7 · W2 — H entry path: day loop → CE246 → HUD/header bar, and the CE333/CE346 gate |
| [`M7_HSCENE_EXIT.md`](M7_HSCENE_EXIT.md) | M7 W2 — H scene termination: the real exit from terminal stage 13 |
| [`M7_HSCENE_FULL.md`](M7_HSCENE_FULL.md) | M7 W2 full — resume the retail mid-H save and drive the late-night UI |
| [`M7_HSCENE_PLAYABLE.md`](M7_HSCENE_PLAYABLE.md) | M7 W2 — Playable late-night H wrapper (real pointer input) |
| [`M7_HWORD_VISIBLE.md`](M7_HWORD_VISIBLE.md) | M7 W2-b：H 文字层（hword / word_effect）可见性修复 |
| [`M7_MAP_EDGE.md`](M7_MAP_EDGE.md) | M7 W4 — Map-graph edge cases |
| [`M7_NEW_GAME_ENTRY.md`](M7_NEW_GAME_ENTRY.md) | M7 W4 — Real new-game entry: title → CE42 → Map003 → SampleMapA |
| [`M7_OPCODE_111.md`](M7_OPCODE_111.md) | M7 W5 · opcode 111 条件指令「值操作数」语义（literal vs 变量引用） |
| [`M7_OPCODE_124.md`](M7_OPCODE_124.md) | M7 W5 · opcode 124 SetVariableEx 长格式语义（H-124） |
| [`M7_PRNG.md`](M7_PRNG.md) | M7 W5-H-PRNG-1: port the retail PRNG |
| [`M7_SAVE_WINDOW.md`](M7_SAVE_WINDOW.md) | M7 · 存档 SavePart 覆盖与窗口皮肤（SAVE & WINDOW） |
| [`M7_UNVERIFIED_AUDIT.md`](M7_UNVERIFIED_AUDIT.md) | M7/M8 未验证清单（UNVERIFIED）审计 |

### M8（7 篇）

day4 战斗结算、H 前向链、地图角色层与模板存档的底稿（历史）。

| 文档 | 内容 |
| --- | --- |
| [`M8_BACKLOG.md`](M8_BACKLOG.md) | M8 Backlog — consolidated plan from all M7 research and tests |
| [`M8_BATTLE_SETTLE.md`](M8_BATTLE_SETTLE.md) | M8 W1-c — the day-4 battle now settles: CE242's argument is the acting slot |
| [`M8_CHARACTER_STATE.md`](M8_CHARACTER_STATE.md) | M8 W3-a — real map/actor coordinates behind `character_state` |
| [`M8_ENGINE_STATE.md`](M8_ENGINE_STATE.md) | M8 W3-b — engine-state fields 11 / 12 / 17 pinned from the shipped binary |
| [`M8_HSCENE_FORWARD.md`](M8_HSCENE_FORWARD.md) | M8 W2-d — H forward wrapper chain from the map H button (one continuous playthrough) |
| [`M8_PRNG_RANGE.md`](M8_PRNG_RANGE.md) | M8 W5-a: retail range reduction for 121 / operator 0x6000 `rand(min,max)` |
| [`M8_TEMPLATE_SAVE.md`](M8_TEMPLATE_SAVE.md) | M8 W4-b — a data root without `Save/System.sav` is now saveable |

### M9（21 篇）

锚点/槽位、H 闭环、战斗驱动、移动与地图触发的底稿（历史）。

| 文档 | 内容 |
| --- | --- |
| [`M9_ANCHOR_SLOTS.md`](M9_ANCHOR_SLOTS.md) | M9 W5-b - picture anchor nibbles and long-form slots [2]/[5] |
| [`M9_AUDIO_EDGE.md`](M9_AUDIO_EDGE.md) | M9 — Audio edge cases: malformed RIFF fmt chunk + missing assets |
| [`M9_AUDIO_SMOKE.md`](M9_AUDIO_SMOKE.md) | M9 — Audio smoke: inventory + mixer regression (shared task-51) |
| [`M9_BATTLE_BOSS.md`](M9_BATTLE_BOSS.md) | M9 W1 · Production multi-round battle driver on the BOSS path |
| [`M9_BATTLE_COMMAND_INPUT.md`](M9_BATTLE_COMMAND_INPUT.md) | M9 Battle Command Input (task-50) |
| [`M9_BATTLE_LIVE_BRIDGE.md`](M9_BATTLE_LIVE_BRIDGE.md) | M9 task-63 — Bridging the live CE65 battle menu into the rebuilt battle driver |
| [`M9_BATTLE_MULTI_INPUT.md`](M9_BATTLE_MULTI_INPUT.md) | M9 W1 — Multi-input command menu: why the shipped CE188 line cannot register >1 command |
| [`M9_BATTLE_PARTY.md`](M9_BATTLE_PARTY.md) | M9 多人队伍战斗（multi-member party battle）调研与结论 |
| [`M9_BATTLE_ROUND_GAP.md`](M9_BATTLE_ROUND_GAP.md) | M9 W1 - Why CE243/CE241 never fire on the natural day-4 battle |
| [`M9_FULL_ROUTE.md`](M9_FULL_ROUTE.md) | M9 full route: title boot to day 5 08:30 |
| [`M9_HOVER_ANCHOR.md`](M9_HOVER_ANCHOR.md) | M9 hover hit-testing: anchor-aware opcode 124 field 8 |
| [`M9_HSCENE_CG_RUN.md`](M9_HSCENE_CG_RUN.md) | M9 W2 — driving the real H CG cluster from the proven `var460` / `ev33` path |
| [`M9_HSCENE_CG_UNLOCK.md`](M9_HSCENE_CG_UNLOCK.md) | M9 W2 residual — unlocking the H-scene CG cluster (var460 / var111 / var157) |
| [`M9_HSCENE_LOOP.md`](M9_HSCENE_LOOP.md) | M9 W2 — the full H loop: night wrapper -> terminal stage 13 -> end button -> sleep -> next morning |
| [`M9_MAP_CONTACTS.md`](M9_MAP_CONTACTS.md) | M9 W3-c follow-up: event contact boxes, player start, contact triggers |
| [`M9_MAP_TRIGGERS.md`](M9_MAP_TRIGGERS.md) | M9 W3-c: Map Event Trigger Dispatch Core |
| [`M9_MOVEMENT.md`](M9_MOVEMENT.md) | M9 W3-b - headless player movement and passability core |
| [`M9_NEGATIVE_SCALE.md`](M9_NEGATIVE_SCALE.md) | M9 W5-b - negative picture scale is a horizontal mirror |
| [`M9_OPCODE_111_REGRESSION.md`](M9_OPCODE_111_REGRESSION.md) | M9 W5 · opcode 111 值操作数「引用语义」补丁的回归报告（PARKED） |
| [`M9_PRNG_SEED.md`](M9_PRNG_SEED.md) | M9 W5-a - Production event-RNG seed hook and deterministic replay |
| [`M9_WINDOW_SKIN.md`](M9_WINDOW_SKIN.md) | M9 W5-b window skin completeness and nine-slice regression |

### M10（10 篇）

opcode 111 引用语义、战斗重基线与残项收口的底稿（当前里程碑，已收口）。

| 文档 | 内容 |
| --- | --- |
| [`M10_AUDIO_RESIDUAL.md`](M10_AUDIO_RESIDUAL.md) | M10 audio residual: missing BGM/kana.mp3 and loop-point audit |
| [`M10_BATTLE_DB_FIXTURE.md`](M10_BATTLE_DB_FIXTURE.md) | M10 Phase 0 — 完整战斗数据库夹具与 CE172 计数证明 |
| [`M10_BATTLE_EXPECTATIONS.md`](M10_BATTLE_EXPECTATIONS.md) | M10 Phase 2 前置 · 战斗测试期望重导表（DOCUMENT-ONLY） |
| [`M10_HSCENE_CE346.md`](M10_HSCENE_CE346.md) | M10 W2 residual - CE246 -> CE346 H cut-in executor |
| [`M10_HSCENE_DEEP_SAVE.md`](M10_HSCENE_DEEP_SAVE.md) | M10 W2 - a constructed deep H save, and how far it advances (task-70) |
| [`M10_OPCODE_111_FIRST_DIVERGENCE.md`](M10_OPCODE_111_FIRST_DIVERGENCE.md) | M10 阶段一：opcode 111 引用语义「首个分歧点」探针报告 |
| [`M10_OPCODE_111_LANDING.md`](M10_OPCODE_111_LANDING.md) | M10 opcode 111 原子落地审计（task-74） |
| [`M10_OPCODE_111_PLAN.md`](M10_OPCODE_111_PLAN.md) | M10 opcode 111 参考语义落地方案 |
| [`M10_REBASELINE_REVIEW.md`](M10_REBASELINE_REVIEW.md) | M10 — Independent review of the opcode-111 re-baselined tests |
| [`M10_SAVE_COMPLETENESS.md`](M10_SAVE_COMPLETENESS.md) | M10 — SavePart completeness audit |

### M11 第一批（6 篇）

M10 剩余边界项（task-79..84）的验收底稿；见 `../HANDOFF_M10.md` §8。

| 文档 | 内容 |
| --- | --- |
| [`M11_ANCHOR_PIXELS.md`](M11_ANCHOR_PIXELS.md) | M11 task-82 — 锚点 5/6 像素几何、负缩放镜像与残余 |
| [`M11_AUDIO_ACCEPTANCE.md`](M11_AUDIO_ACCEPTANCE.md) | M11 task-84 — 31 首 loop-tagged BGM 验收档案与人工听感分层 |
| [`M11_BATTLE_COMMAND_SEQUENCE.md`](M11_BATTLE_COMMAND_SEQUENCE.md) | M11 task-83 — 真实多成员战斗命令序列与回合/池消耗 |
| [`M11_BATTLE_DETERMINISM.md`](M11_BATTLE_DETERMINISM.md) | M11 task-79 — 战斗自然路线确定性证明（BTreeSet、逐字节轨迹） |
| [`M11_HSCENE_REAL_GATE.md`](M11_HSCENE_REAL_GATE.md) | M11 task-80 — 8 个零售存档的 H 深链 gate 矩阵（var304/var326） |
| [`M11_MOVEMENT_NPC.md`](M11_MOVEMENT_NPC.md) | M11 task-81 — 8 向对角移动、NPC 占位与 contact |

### M11 round 1（4 篇）

M11 第二批（task-85..88）的验收底稿；见 `../HANDOFF_M10.md` §9。

| 文档 | 内容 |
| --- | --- |
| [`M11_AUDIO_LOOP_REGRESSION.md`](M11_AUDIO_LOOP_REGRESSION.md) | M11 task-88 — 31 首 loop-tagged 的逐曲阈值固定与二次 decode 位相同回归 |
| [`M11_BATTLE_MENU_BRIDGE.md`](M11_BATTLE_MENU_BRIDGE.md) | M11 task-85 — CE157 菜单确认采集、命令队列桥与 CE157 cursor 残余 |
| [`M11_HSCENE_PROGRESSION.md`](M11_HSCENE_PROGRESSION.md) | M11 task-86 — var304/var326 真实进度与 CE328 早退边界 |
| [`M11_WINDOW_EDGE.md`](M11_WINDOW_EDGE.md) | M11 task-87 — CE94 窗口/文本几何、CN patch 右缘 data-only 结论 |

### M11 round 2（8 篇）

M11 第三批（task-89..96）的验收底稿；见 `../HANDOFF_M11.md`。

| 文档 | 内容 |
| --- | --- |
| [`M11_BATTLE_AUTOSTART.md`](M11_BATTLE_AUTOSTART.md) | M11 task-95 — 默认武装战斗驱动与自然 day4 结算 |
| [`M11_CE157_CURSOR.md`](M11_CE157_CURSOR.md) | M11 task-89 — CE157 物理 cursor = CData type18 row73，推翻 VM bug 前提 |
| [`M11_HSCENE_BOOKSTORE.md`](M11_HSCENE_BOOKSTORE.md) | M11 task-90 — H 书店线 var276/277/326/304 与 CE328 入口 |
| [`M11_HSCENE_CE328_BRANCHES.md`](M11_HSCENE_CE328_BRANCHES.md) | M11 task-94 — CE328 分支图与 CE402 唯一 caller |
| [`M11_HSCENE_CE328_REAL_CG.md`](M11_HSCENE_CE328_REAL_CG.md) | M11 task-96 — 真实 CE328→CE402/CE410 深链与门表 |
| [`M11_NPC_INTERACTION.md`](M11_NPC_INTERACTION.md) | M11 task-91 — NPC ActionKey 与每 8 帧持续行走 |
| [`M11_PARTY_LIVE_BRIDGE.md`](M11_PARTY_LIVE_BRIDGE.md) | M11 task-93 — 2 成员真实菜单 15 行→CE242 四个 body |
| [`M11_VISUAL_REGRESSION.md`](M11_VISUAL_REGRESSION.md) | M11 task-92 — 5 场景像素/quad 回归基线 |

### M11 round 3（4 篇）

M11 第四批（task-97..100）的验收底稿；见 `../HANDOFF_M12.md`。

| 文档 | 内容 |
| --- | --- |
| [`M11_BATTLE_MENU_LIVE.md`](M11_BATTLE_MENU_LIVE.md) | M11 task-98 — 真实 2 成员菜单 CData type14 → CE157 → CE242 三 body |
| [`M11_HSCENE_NATURAL.md`](M11_HSCENE_NATURAL.md) | M11 task-97 — 默认预算自然帧 CE328→CE402/CE410 与 save+event 截图 |
| [`M11_PACKAGING.md`](M11_PACKAGING.md) | M11 task-100 — wolf-package 校验/组装与 FrameBudgetReport |
| [`M11_PRNG_CLOCK.md`](M11_PRNG_CLOCK.md) | M11 task-99 — 零售 GetTickCount 播种与零状态重读边界 |

### M12（6 篇）

零售范围审计 + round 1/day6/multi-day soak 的玩家流程验收：区分游戏流程、引擎通用能力、示例资产与保真工具；补齐战斗取消/返回/连续确认、confirm 页命中审计与存档逐段全量对照。

| 文档 | 内容 |
| --- | --- |
| [`M12_RETAIL_SCOPE.md`](M12_RETAIL_SCOPE.md) | M12 — 零售点触式输入实证；行走/触发 3/4/相机/281/201/远景为非零售；存档/回标题/截图/DLC 为真实流程；H act4/5 与 3–4 人队伍不计缺口 |
| [`M12_BATTLE_CANCEL.md`](M12_BATTLE_CANCEL.md) | M12 task-101 — 真实菜单取消/返回/连续确认：CE191 取消重开不登记；CE192 取消退出；后续确认仍登记 (1,0,26)/(2,1,25) 并启动 2 次 CE157 |
| [`M12_CONFIRM_PAGES.md`](M12_CONFIRM_PAGES.md) | M12 task-102 — SampleMapA 6 个 confirm 页：5 个空页选择器 + ev24 p6 调试专用；事件格不是点击命中区，124 图片矩形才是 |
| [`M12_SAVE_FULL_COMPARE.md`](M12_SAVE_FULL_COMPARE.md) | M12 task-103 — 8 个零售存档经 Game load→save 整文件逐字节一致；Part5 未建模 bank/文本 md1/淡入/纵向缩放保留 |
| [`M12_DAY6_WEEKEND.md`](M12_DAY6_WEEKEND.md) | M12 task-104 — day5 工作日 → 周六 09:30 CE310 休日五按钮 → 12:00/15:00/17:00 三个活动时段 → 周日 09:30 同一菜单；无时间/日程注入 |
| [`M12_MULTIDAY_SOAK.md`](M12_MULTIDAY_SOAK.md) | M12 task-105 — 通用驱动连续 30 天到 day31、90 天到 day97（含剧情跳天）；11/27 个战斗日；修复 parented CE239 auto-arm |

### M13（12 篇）

内容一致首批验收：主线结局/DLC/随机事件/H 阈值/NG+/跨天存读/op111 更正，以及第二批九结局、条件注入事件、H 端到端与 DLC EP26–36 结案。

| 文档 | 内容 |
| --- | --- |
| [`M13_MAINLINE_ENDINGS.md`](M13_MAINLINE_ENDINGS.md) | 无注入通关 SaveData01/08 day90→95（stage28→29,var492=1）与逃亡 END |
| [`M13_ALL_ENDINGS.md`](M13_ALL_ENDINGS.md) | 其余 7 结局逐条：妊娠/兵役/返家/病亡症状链/暗影/魔界/农民/爆炸；14 默认+2 ignored |
| [`M13_DLC_452.md`](M13_DLC_452.md) | CE452 EP0–36 注入全通、自然 var619 0→25 |
| [`M13_DLC_452_EP26_36.md`](M13_DLC_452_EP26_36.md) | task-18 结案：var395 为永久献身技能标志（不可重置）；EP26 卡点=探针缺已升级队伍属性；预置属性后自然链 EP0→36、var619=36 |
| [`M13_CONTENT_MATRIX.md`](M13_CONTENT_MATRIX.md) | 115 随机事件、H 门、战斗、物品、结局坐标矩阵 |
| [`M13_CROSSWALK_UNRESOLVED.md`](M13_CROSSWALK_UNRESOLVED.md) | 攻略 ↔ 脚本未决项 #78 结案等 |
| [`M13_RANDOM_EVENTS.md`](M13_RANDOM_EVENTS.md) | 111 事件自然 97/条件注入 17/不可达 1；25 事件抽样 |
| [`M13_RANDOM_EVENTS_INJECTED.md`](M13_RANDOM_EVENTS_INJECTED.md) | 17 条件注入逐条：18 事件点+15 门三元组全绿 |
| [`M13_H_THRESHOLDS.md`](M13_H_THRESHOLDS.md) | 37 条 H 门字节锁定、op111 有序分档更正 |
| [`M13_H_ENDTOEND.md`](M13_H_ENDTOEND.md) | 周末/厕所/浴室/CE335/餐厅 H 端到端；餐厅门更正 CE326@801–805 |
| [`M13_NGPLUS.md`](M13_NGPLUS.md) | CE436 点数表与 SampleMapA 继承菜单 |
| [`M13_CROSSDAY_SAVE.md`](M13_CROSSDAY_SAVE.md) | 8/8 存档 load→save 字节幂等、跨天存读 |

## 状态约定

- **M6–M9 文档**：历史记录，可能描述当时的 blocker；如果与现行实现冲突，以现行代码/测试和 HANDOFF 为准。
- **M10 文档**：对应 M10 已收口的实现与重基线；其中 `M10_OPCODE_111_PLAN.md`、`M10_BATTLE_EXPECTATIONS.md` 是计划/期望推导，最终以 `M10_OPCODE_111_LANDING.md` 与 HEAD 测试为准。
- **M11 文档**：M11 第一批/round 1/round 2/round 3 的验收底稿；结论以 `docs/HANDOFF_M12.md`（round 3）与 `docs/HANDOFF_M11.md`（round 2）及 HEAD 测试为准（第一批/round 1 亦见 HANDOFF_M10 §8/§9）。
- **M12 文档**：零售范围审计 + round 1/day6/multi-day soak 底稿；结论以 `docs/HANDOFF_M12.md` §5/§8 与 HEAD 代码/数据/测试为准。
- **M13 文档**：内容一致首批验收与第二批底稿；结论以 `docs/HANDOFF_M13.md` 与 HEAD 代码/测试为准。
- **GUIDE_CONTENT_CROSSWALK**：外部攻略索引，不是规格；攻略为玩家观测，数值/天数/译名均需回 `work/full` 脚本核对。攻略 PDF 留在 `docs/攻略文档/`（用户资料，未跟踪）。
- **M7_BACKLOG / M8_BACKLOG**：历史计划，已被后续 M9/M10 交接取代；只用于了解当时的缺口清单。
