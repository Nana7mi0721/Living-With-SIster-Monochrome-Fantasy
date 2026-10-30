# GPT 外部调研报告 · 综合研判与落地对照（GPT_RESEARCH_TRIAGE）

> 性质：**研判索引 / 落地对照**，不是新规格。凡与本仓库已实测结论冲突处，一律以仓库代码注释与 docs/ 下已验证文档为准（见 §2.4）。
> 输入目录：E:/Study/github program/docx/gpt调研
> 输入清单：6 份 .md、1 份 PDF（12 页）、2 张 jpg。
> 目标仓库：E:/Study/github program/imoto life —— WOLF RPG Editor 重实现，目标作 Living With Sister: Monochrome Fantasy（Steam appid 2429860，日文《妹!せいかつ～ファンタジー～》）。
> 证据分级（沿用 PDF）：A = 开发者/发行商直接说明；B = 可定位画面/攻略实测；C = 第三方推断；U = 未找到。另加一层 **【实测-本仓库】** = 仓库已用二进制解析 / 逐字节断言 / 像素对照证实。
> 综合时点：本次会话；仓库状态以每条引用的文件:行号为准。cargo test/build 由 Lead 执行，本 agent 未运行。

---

## 0. 速读结论（三句话）

1. 六份报告质量极不均衡：**(4)（515 行字节码逆向）与 (5)（334 行，偏战斗/手册）信息密度最高且大体可信**；**(2) 的 opcode 速查最不可靠**（130 布局、290 编号、9000071/72 均带错误推测）；**(0)(1)(3) 属剧情/攻略级**，只能当"待验证的玩法线索"，不能当规格。
2. **报告不解决本仓库当前的主要问题**：本仓库的引擎层（130/150/151/160-162/250/290/run_condition/210 返回值/存档）已相当完整，多份报告建议的 P0 在本仓库多已实现；报告真正的增量价值在**数据层**（日程/任务/数值表）。
3. **报告 (4)(5) 有三处"必须立即改"与本仓库实测相反**，切勿照做：9000071/72（报告说鼠标、实测是主人公画面坐标）、250 位域（报告说容器 &0x0f、实测 &3 零例外）、CE242/243 判定（报告说查 call graph、实测是 condvar 门控）。详见 §2.4。

---

## 1. 各报告可追溯摘要（文件 → 主要结论）

### 1.0 输入总览

| 文件 | 类型 / 证据级别 | 规模 | 一句话定位 |
|---|---|---|---|
| deep-research-report.md | 剧情/攻略速览（早期） | 95 行 | 剧情日程 + 妹妹阈值 + 体位，大量自评"推测/未找到"，混用旧作名 |
| deep-research-report (1).md | 攻略/术语/职业特性 | 117 行 | 职业初始、特性、道具、妹妹四值，全标"玩家攻略" |
| deep-research-report (2).md | 引擎 opcode 速查 | 46 行 | 混合：run_condition 引官方手册；130/151/250/290 标推测 |
| deep-research-report (3).md | 战斗/日程攻略 | 60 行 | 回合制战斗描述 + 锦标赛关键日 + 职业表 |
| deep-research-report (4).md | 字节码逆向级（最权威） | 515 行 | 带证据等级，纠正 130/250/290/9000071，给 P0/P1/P2 |
| deep-research-report (5).md | 偏战斗/官方手册，与(4)重叠 | 334 行 | 战斗 UI 七列、Energy 机制、130/290/9000071 细节 |
| 世界地图…深度研究报告.pdf | dossier（A/B/C/U 分级） | 12 页 | 世界结构/探索/公会/经济/日程，工程实体建议 |
| 2 张 jpg | 实机 HUD 截图 | 800² / 1529×860 | 可直接核对 HUD 字段（§5） |

### 1.1 deep-research-report.md（早期，攻略级）
- 主角=哥哥，妹妹患病；状态值 欲情(Lust: BLust 哥 / SLust 妹)、信頼 Trust、興味 Interest、Mood、Energy、Condition、Rep 声望。
- 按天触发表：Day11 Exhibition Match；Day15 Guild Bounties；Day18 Regional Qualifier vs Akatsuki（败入农夫结局）；Day24 会 Yukari/救公会学厨艺；Day25 模拟战；Day29 Seal Hunt(3天)；Day33 Guild Decision；Day46 第二次；Day52 占卜师；Day53 八人赛资格赛；Day54 Yukari 提议训练营；Day57 训练营周；Day64 Slay Monsters；Day74 大赛开始；Day78 半决赛；Day84 决赛；Day85 遗迹狩猎(40步15天)；Day95 重赛 Akatsuki；Day100 锁定日常。
- 阈值：信頼≥50 拍肩/拥抱、≥100 接吻；興味≥50 看内裤、≥150 展示身体；料理需 BLust≥100 & SLust≥40 & 興味≥100。
- 体位解锁：Cowgirl 需 Naughty/Thin Book；Piledriver 生病连洗两次澡；69 夜晚侧身。
- 新游戏+点数。
- **警示**：全文自评多处"推测/未找到"；且把日文名混用为旧作"モノクローム"（见 §2.3）。属 C/攻略，不能当数据。

### 1.2 deep-research-report (1).md（攻略/术语/职业）
- 地图/探索/公会/日常/掉落全部标"推测/未找到"。
- 术语表：公会/任务/战斗/お兄ちゃん/妹/行動力 AP/精力/信頼/興味/感度/キゲン/就寝時間。
- 职业初始：学生 AP60 钱 3000-6000 就寝 24 点；フリーター AP40 钱 8000-12000 23 点；社会人 AP30 钱 30000-50000 23 点；ダメ人間 AP25 钱 1000 24 点。
- 特性表：体力自慢 AP+10、むっつり 精力+2/AP-5、ドスケベ 精力+1/信赖+15、クマヘッド AP+10/信赖+80/钱-10000、イケメン 信赖+40、光合成 AP+5、絶倫 精力+3、ストロング AP+5/精力+1、爆死 AP-10、脱糞 AP-10、不感症 精力-1、変態 信赖+10。
- 妹妹四值：信頼/興味/感度/キゲン（キゲン 5 档：爆发临界/不快/普通/愉快/兄Love）。
- 道具：茶 +10AP、玉露 +20AP、红茶 +5AP+1h、咖啡 +2h；精力剂/媚药/安眠茶/事后药（3 天内否则怀孕 End，需 50000 金币否则 GameOver）；哑铃 AP+5/训练器 +15/安眠枕。
- AP 上限 99，精力上限 9。全部标"玩家攻略"。

### 1.3 deep-research-report (2).md（引擎 opcode 速查；最不可靠）
- run_condition=0 =「呼び出しのみ」仅被调用时触发（**称官方手册事实**）。
- WOLF 无独立战斗引擎，战斗由基本系统通用事件脚本实现（190+ 系列）。
- 130 Teleport 参数写作「目标角色/目标地图(0=当前)/X/Y/朝向(1-9,0不变)」——**此处标"推测"，且顺序错误**（见 §2.3/§2.4）。
- 151 ChangeColor R/G/B/Alpha 0-255（推测）。
- 160/161/162 过渡（指定/准备/执行）。
- 250 DatabaseAccess 官方未明（未找到）。
- 290 Effect 0=闪光 1=震动 2=闪烁A 3=闪烁B 4=全屏闪白（社区惯例推测，**编号/语义均错**）。
- 系统变量：9000104/105=屏幕滚动 X/Y；9000071/072 官方未列（社区推测窗口尺寸/输入）；9000000+ 系统状态（鼠标/菜单）；仅全局变量随存档保存。存档格式未找到。
- 素材：图 PNG/JPG/BMP(≤2048²)、音频 MIDI/WAV/MP3/OGG、字体 TTF/OTF、图层支持负层级（3.5 起 Z 排序）、Alpha/乘算混合。

### 1.4 deep-research-report (3).md（战斗/日程攻略）
- 生活模拟 + 回合制战斗 RPG；战斗在公会交流战/锦标赛/讨伐触发（第 11/18/46/53/74/78/84 日），第 100 日后周末讨伐（5 Boss：イカ大王/サイコマッチョ 等）；日常探索不战斗，无随机战。
- 回合制：每回合每角色行动一次，动作=攻击/技能/防御/道具，消耗 AP，**回合开始全队 AP+2**，必杀技时间条点击。
- 战力=HP，敌战力 0 胜、己 0 败、10 回合未分比战力。
- 术语：戦力/行動ポイント/防御/戦闘スキル/一般スキル/えっちスキル/Energy/Lust(20%)/Trust/Interest/感度/Mood/消費★(推测)。
- 表：职业初始（同 (1)）；称号 Dauntless 攻+1、Always Ready 闪避+1、Naughty 精力+1/欲情+20；AP 每回合+2；10 回合。
- 讨伐触发：第 101 天起周末，イカ大王进度 10、サイコマッチョ 12。
- 锦标赛关键日：74 vs スカウトユニオン、78 vs ナナホシ、84 vs 冒険者教会；第 33 天公会解散需料理事件否则農業エンド；周末选项（村里散策/逛街 100G/游泳 500G/冒险）。

### 1.5 deep-research-report (4).md（最权威，字节码逆向，带证据等级）
- 正式日文名《妹!せいかつ～ファンタジー～》（App 2429860，2024-03-22，Inusuku/Kagura）；《～モノクローム～》是另一部前作。
- 战斗存在（Steam"Unique Combat System"，零售截图含三头像/Actions/Battle Score/ATK/CRT/DEF/BLK/CNT）；"テスト"敌人未找到外部证据（不能证可达也不能证废弃）。
- run_condition 四态：0 呼び出しのみ（不扫条件）/1 自動実行（条件成立运行，独占阻塞）/2 並列実行（条件真并行，失效可中断）/3 並列実行常時（无条件常驻）。不要加 if id==63/241 特例。
- Common Event 参数：数值 1-4→CSelf0-3，字符串 1-4→CSelf5-8，"1600000+i"是变量调用值编码区不是第六组；返回 return_value_id→caller return_dst。
- opcode 101：Sys1-6 属 message 系统变量但逐项未证；Sys8 字号/Sys9 文字速度/Sys10 前置等待/▼位置/Text_Pause.png 17×90/关闭 6 帧 均未找到，勿写进 engine，应由 Basic System 事件驱动。
- **9000071/072 = 鼠标 X/Y（不是主人公屏幕坐标）**；9000104/105 = 地图滚动 X/Y。（★此条与本仓库实测相反，见 §2.4）
- opcode 150 Picture：表示/移动/消去/延迟重置，range 消去，处理时间≠延迟；anchor 0 左上/1 中心已证，bit12-14 需修正。
- opcode 250 DB：官方 可変DB/システムDB/ユーザDB/XY 配列；建议容器=(flags>>8)&0x0f（不要 &3），assignment=flags&0xff，options=(flags>>8)&0xff，db_kind=options&0x0f，db_mode=options>>4；op 0..7=代入/加/减/乘/除/余/max/min；arg0=type id，arg1=data，arg2=column/field。（★位域建议与本仓库实测相反，见 §2.4）
- opcode 290 Effect：picture_effect_type 0-10 共 11 种——0 Flash、1 ColorCorrect、2 DrawPositionShift、3 Shake(X偏移,Y偏移,次数,处理时间;≥10万次=无限)、4 Zoom(中心X/Y,缩放率)、5 SwitchFlicker、6 SwitchAutoFlash、7 AutoEnlarge、8/9/10 AutoPatternSwitch(One/Loop/RoundTrip)；目标"□～□"=连续 Picture ID 闭区间。
- opcode 151 ChangeColor：RGB 0-200，100 中性；flash 勾选限时；raw bit 未证。
- 160 SetTransition/161 PrepareTransition/162 ExecuteTransition 三阶段模型，raw flags 未证。
- opcode 130：WolfTL 确认=Teleport；-1/-2 本次未映射；direction 27/16/32 不像 4/8 方向，疑 options bitfield，勿直接 direction=raw_arg，P0 differential fixture。
- 消息/图片生命周期：事件结束/CE 返回/MapEvent page end/BreakEvent **不自动清 Picture**，勿全局 pictures.clear()；标题/HUD/菜单/战斗/存档都是项目 Common Events 而非 Rust 内建系统。
- 推荐 cross-reference：take-cheeze/rpg-maker-clone、Sinflower/WolfTL、djytw/wolf-rpg-formats、wolfrpg-map-parser。证据优先级：WOLF 3.50 官方手册 > 原版 Game.exe 最小 differential fixture > 真实运行 trace > 两个以上独立开源解析器 > 全库统计推断。
- P0：修 scheduler / 210-300 参数返回 / CE242-243 调用图 / Picture150 生命周期+range / DB250；P1：Effect290 / 正式战斗路径 / 101+message CE / title+HUD；P2：Teleport / transition / ChangeColor raw bit / 死代码清理。

### 1.6 deep-research-report (5).md（与 (4) 高度重叠，偏战斗/手册）
- 正式日名同 (4)。
- 战斗确为玩家指令式（Steam 攻略：普通 Onii-chan 攻击→Crazy Strat→EX Onii-chan Attack≈3 倍；西语攻略 Kana 输出/Yui 治疗/1★防御；胜利得基础能力/S.Point/公会声望）。
- 战斗来源三类：公会日常 hunt、剧情强制战（第 4 天 community service 会输）、Tournament of the Eights（Day46 exhibition/Day53 qualifying/Day74 Scout Union/Day78 semifinals/Day83-84 决赛）。无传统随机遇敌。
- UI 字段：ATK/SKI(暴击)/DEF/DET(格挡闪避)/INT(特技发动)/SP(技能点)/Reputation/★/Battle Score（战力，勿等同 HP，需 xref 2000350）。七列能力=Reputation/ATK/SKI/DEF/DET/INT/SP。
- 日常核心资源是 Energy（非固定 N 次行动；阅读约 -10、训练约 -20、公会约 -35；睡眠/做饭/洗澡恢复）。
- 公会工作日 3 选 1：Attack Training→ATK,SKI；Defense→DEF,DET；Special→DET；Study Group→INT,SP；Community Service→Rep,SP；Hunt→Rep+多属性/SP；Tavern→钱/SP；Basic→Max Energy。数值版本不一（ATK+7 vs +9），以 Steam 数据版回归。
- 周末：周五晚对话→选下周 Policy；周末兼职/外出/买冒险书/独立训练。
- 标题：新游戏回答问题得 Title/称号；RTA 计时从「はじめるぞ！」开始（表面已项目级自定义，不能证是否 fork 自 オリジナルタイトル画面Ａ）。存档：Steam Cloud + 手动；SaveData01 是否自动未知，勿硬编码槽语义。
- 130：-1=当前事件(CE 中=调用源)、-2=主人公/队列第一人、-3..-7=队友 1..5、>=0=地图事件 ID；destination map -1/-2 无特殊义；direction 0 不变/1-9 小键盘(5 无方向)/可变量；27/16/32 非法 direction→须拆 packed。
- 9000071/072=マウス X/Y（"必须立即改"）；9000104/105=scroll。（★9000071/72 与本仓库实测相反，见 §2.4）
- 290：对象 picA..picB 闭区间；官方 3.5 列 11+ 种；Shake 处理时间越短越快，次数≥100000 无限；rpg-maker-clone 尚未实现 Zoom/auto pattern（勿当 WOLF 没有）。
- CE239/188/242/243 判定：若可从正式 Hunt/赛事入口 call graph 达→当真实战斗线；判据四条（公会 Hunt 入边/Eights 首战入边/Kana-Yui-Onii 技能串 xref/BattleScore+★+胜负变量切片）。（★与本仓库的 condvar 门控实现不同，见 §2.4）

### 1.7 世界地图/探索/公会/经济 PDF（12 页，A/B/C/U 分级）
- 身份：Steam Living With Sister: Monochrome Fantasy = 日文《妹!せいかつ～ファンタジー～》，モノクローム 是另一作品 [S01-03]。平台：2022 日文原作、Steam 2024-03-22、本体补丁 v1.02、DLC v2.02、SteamDB 检出 WolfRPGEditor/.wolf [S05,S08,S10,S11]。
- 世界结构：**日程驱动的场景选择 + "进度条式远征"，非自由行走大地图**（地图总数/MapData ID = U）。场景：自宅/ギルド/村を散策/街へ繰り出す/泳ぎ/海/大冒険だ!/超大冒険だ!/温泉/討伐にいくぞ!。
- 探索三类入口：普通工作日讨伐（第 15 日解锁，随机 3 选 1，进度式，按钮 進む/休む/帰還，进度 1/3 [S14]，归零遇痩せたうりぼう）；周末大冒险（0-100，妹体调 10/10 [S15]，進む +1/-5/-1、背負う +1/-8、休む +15/+3，每 20 进度固定战斗 20農夫/40小悪党/60盗賊団/80アザラ団/100大樹の守護者，60 后有禁休段）；随机宝箱采集=U。
- 公会工作日随机 3 选 1：攻撃練習 Energy-35 ATK+9 SKI+3；防御練習 Energy-35 DEF+9 DET+3；基礎鍛錬 Energy-35 最大 Energy+2 DET 增（v1.02 版写 Max AP+3）；秘密特訓 DET 增（值 U）；お勉強会 Energy-20 INT+12 SP+7；地域貢献 Energy-40 声望+SP 增/随机战斗；お店の手伝い Energy-25 Gold+250/+350 SP 增；討伐依頼 Energy-35 声望增 ATK/SKI/DEF+2 SP+10。（Defense Practice v1.02 写 DEF+11）
- 四套任务入口：普通随机讨伐、第 29 日海豹讨伐支线（约 3 日期限，10/10 或 40/40）、第 64 日联合讨伐（3 日，进度 50，100 野猪战可选）、第 85 日遗迹调查（15 日，进度 40，连战影+???）、通关后周末讨伐（5 头目）。
- 可实证对象（本报告拟定主键，非 WOLF DB ID）：Q-GUILD-EARLY、Q-SEAL、Q-JOINT-64、Q-RUINS-85、Q-MONKEY-POST、Q-POST-01..05（イカ大王 10、サイコマッチョ 12、メンヘラ 12、サイレントキラー 15、アザラ勇者 15）。
- 章节/解锁天数表（Day11/15/18/24-29/33/39-43/46/53/55-61/64/74/78/84/85-99/95/100/106），详见 §5.3。
- 敌人名单：痩せたうりぼう/農夫/小悪党/盗賊団/アザラ団/大樹の守護者/うりぼう×100/何者かの影/???/サル軍団/サル山の大将；赛事对手美食倶楽部/ランニング大根愛好会ノーソン支部/マッシブブラザーズ/スカウトユニオン/ナナホシ事務所/冒険者教会タウン支部；通关后 5 Boss。随机表/HP/ATK/DEF/掉落 ID/掉率 = U。
- 经济：公会店铺 +250/+350G Energy-25；周末打工 +800G（农业技能 +1000G）；进城镇 -100G；哑铃 1000G；铁下驮 1000G；羽毛寝具 4000G（睡眠 +20）；冒险之书 300G/折 200G（读增五项战斗属性）；技能卷轴 4000G/折 3000G。周薪公式 = U。
- 工程建议实体：GameCalendar(day,weekday,phase)、Scene、Action、QuestTemplate(quest_family,progress_mode,goal,deadline_days)、QuestInstance、Encounter、StoryFlag、ShopOffer、ProgressionRule。**三进度模式不可混用**：REMAINING_TO_ZERO（讨伐 3→0）/ ACCUMULATE_TO_TARGET（大冒险 0→100）/ SCRIPTED_STAGES（赛事多阶段）。建议旗标（本报告建议，非原变量）：guild_hunt_unlocked、first_exhibition_resolved、rival_result、kasumi_recruited、adventure_100_cleared、ruin_mission_resolved、ending_seen、hot_spring_unlocked、stat_cap_500_unlocked、dlc_enabled。测试 T-01..T-12。
- 冲突总表：日名（以 App2429860 ファンタジー 为主键）；经验涨幅（带版本回归）；Day55 vs 57（分别记启动/执行）；Day95（按前置旗标）；1000 日 vs 100 日（疑笔误保留）；版本范围；地图/掉落清单 = U；Ci-en 原文受限。

### 1.8 附带图片（已人工查看，仍建议 Lead 确认来源）
- **BxrYxVoG6GbITkXCMTaPbA8bwSHm2R_UDXcsOOTVbU5OU1fseWnsSwLOQYBs979LE_vP5G9ANy5YRcxx5T7mNrdEw47JKVIo9yDwIF2yvJC-kWOzvK5gfxBA7fdGVFflvj8UqzvrRX70q6sut6RCE9A-8UqYydHWxrwTyPzfYPycxBG-5wg9oImGoLi5uA5Q.jpg**（800×800，128776 B）：中文界面复合截图（上/下两帧）。可见 HUD：上帧 Day1 22:30 星期一、行动力 0/60、精力 3/4、所持金 2932G、信赖 20/心情 正常、公会评价"无名"/状态"普通"、女性关系 0/性慾 0、战力列 ATK40 SKI6 DEF28 DET12 INT9；下帧 信赖20/糖度 上機嫌、战力 ATK17 SKI6 DEF20 DET4 INT4，含角色对话（ユカリ/エルフベース）。**建议人工查看一次**（多为攻略文章拼图，非原始单帧）。
- **t2dRIy0r9r3WCX-npIwv76mW_Gc7vNIZkYsoksgyrRTIMGMYJc72GKJXoWiFznmxD_YctrE659k36mvJcwkM9uHq9Td8dT1fsZTKZ-xxn-Ql3Es5mzerYfbxzmWFKtqA4FMYME2PlnVCZ9QvFXk1abvpl1q28xAcSPzTwkxWA8w.jpg**（1529×860，143216 B）：英文界面实机截图，HUD 精确可读：Day1 23:00 Mon、Energy 30/60、Stamina 3/3、Money 2846 G、Guild Rep. "Totally Obscure / Normal"、ATK40 SKI6 DEF9 DET8 INT7 SP0、Policy "Starting OFF"、Trust20 Mood Normal，对话框 "Are you going to bed, Onii-chan?"。**建议人工查看**（可直接核对 §5 的 UI 字段与数值；该帧**不含**报告 (4) 提到的 CRT/BLK/CNT 三列——差异需留意）。

---

## 2. 事实 / 推测 / 冲突三分

### 2.1 事实层（官方手册 / 开源解析器 / 本仓库实测）

| 结论 | 证据来源 | 级别 |
|---|---|---|
| 正式日文名《妹!せいかつ～ファンタジー～》，App 2429860，2024-03-22；モノクローム 是前作 | Steam 商店/发行商 [S03][S05] | A |
| 引擎 = WOLF RPG Editor 3.x（存在 3.5 手册） | SteamDB/WolfRPGEditor [S10][S11] | A/B |
| run_condition 四态 0/1/2/3；本作分布 {0:442, 1:1, 2:97, 3:10}（550 事件） | (4)(5) + 本仓库 auto_events.rs:4-9 | 已验证 |
| 130 布局 = target, x, y, map, options | wolfrpg-map-parser 0.4.1 + 本仓库 OPCODE_SPEC.md:760-761 / exec.rs:1790-1798 | 开源 + 实测 |
| 250 容器=(flags>>8)&3、方向 bit12、方法 (flags>>4)&7 | 本仓库 OPCODE_250.md:37-39（2,195/2,195 零例外） | 实测 |
| 290 type 0..10 官方名来自 Editor.exe 3.735 字符串表 | 本仓库 OPCODE_290.md:30-40 | 实测 |
| 9000104/105 = 地图滚动 X/Y（SysDatabase type6 第 104/105 行，引擎输出，无脚本读写） | docs/HANDOFF_M3.md:84 | 实测 |
| 9000071/072 = 主人公画面坐标（非鼠标）；鼠标 = 1600012/1600013 | docs/ENGINE_EVENTS.md:412-417 | 实测（★纠正） |
| 150 长格式槽 [6]不透明 [7]X [8]Y [9]缩放 [10]混合；负番号=全屏层 | docs/UNVERIFIED.md H-PIC-1/2 | 实测 |
| 210/300 返回值：return_value_id（1_600_000+id）→ caller dest | docs/OPCODE_210_RETURN.md | 实测 |
| 存档非 chunk 表，SavePart1-7 固定顺序，9/9 实测 | docs/SAVE_SPEC.md / crates/wolf-save | 实测 |
| 音源 mp3/ogg/wav，Symphonia 解码 | docs/AUDIO.md | 实测 |

### 2.2 推测 / 攻略 / 社区层（不可当规格）

| 说法 | 出处 | 性质 |
|---|---|---|
| 130 参数 target,map,x,y,direction | (2) | 推测（已推翻） |
| 130 direction 27/16/32 = 朝向 | (2)(4)(5) | 推测；实为 options bitfield/非法 direction |
| 9000071/072 = 鼠标 X/Y | (2)(4)(5) | 推测（与本仓库实测相反，见 §2.4） |
| 290 0闪/1震动/2闪A/3闪B/4全屏白（5 类） | (2) | 社区惯例推测（不完整、编号错） |
| 151 RGB 0-255 | (2) | 推测；本仓库按百分比 100=中性 |
| 战斗引擎内置 / 固定状态机 | (2)(3) | 推测；实为 CommonEvent 脚本 |
| 回合开始全队 AP+2、必杀技时间条 | (3) | 攻略推测 |
| 各种数值（ATK+7 vs +9、DEF+9 vs +11、Max AP+2 vs +3） | PDF/Steam 表 | 玩家观测，版本相关 |
| 日程天数表、阈值（信赖≥50/100 等） | (0)(1)(3) | 攻略 |
| 职业初始值、特性加成 | (1)(3) | 攻略 |
| 战斗 UI 含 CRT/BLK/CNT | (4) 零售截图口述 | 待复核；本仓库目标截图仅 ATK/SKI/DEF/DET/INT/SP |
| 地图总数/掉落/宝箱/掉率/周薪公式 | 各报告 | U/推测 |

### 2.3 报告之间互相矛盾

| 主题 | 说法 A | 说法 B | 研判 |
|---|---|---|---|
| 130 参数顺序 | (2)：target, map, x, y, direction | (4)(5)/PDF：target, x, y, map, options（-1 主角/-2 变量/地图在 arg3） | **B 对**（解析器+仓库证实）；A 作废 |
| 9000071/72 | (4)(5)：鼠标 X/Y（"必须立即改"） | (2)：官方未列/社区猜窗口尺寸或输入；仓库：主人公画面坐标 | 三方不一，以仓库实测为准（§2.4） |
| 290 type 编号 | (2)：5 类，0闪/1震动/2闪A/3闪B/4全屏白 | (4)：11 类 0..10（wolfrpg-map-parser）；Editor.exe 3.735 11 类 | **(4)/仓库对**；(2) 不完整且 1/2/3/4 语义错 |
| 战斗是否引擎内建 | (2)：WOLF 无独立战斗引擎、脚本实现 | (3)：有回合制战斗（描述细） | 不矛盾：界面=脚本事件的回合制；"引擎无固定状态机"成立 |
| 训练营启动日 | Gamerch：55 日 | Steam：57 日训练营周 | 分别记启动日/执行天数（PDF 建议） |
| 结局再战/日数 | (0)：Day95 重赛 Akatsuki | PDF/Steam：需前置旗标 | 条件式，非仅日数 |
| 经验增幅 | Steam 2025 活动表 | 2026 v1.02 攻略 | 版本+技能+状态，需逐版本回归 |
| 日名 | (0)(1) 混用"モノクローム" | (4)(5)/PDF 明确"ファンタジー" | 以 App2429860 ファンタジー 为主键 |
| 1000 日 vs 100 日 | 日文攻略偶见 1000 日 | 其它 100 日 | 疑笔误，保留存疑，勿改 |

### 2.4 报告与本仓库实测的直接冲突（最高优先，勿照报告改）

| 主题 | 报告主张 | 本仓库实测 | 处置 |
|---|---|---|---|
| 9000071/9000072 | (4)(5)：= マウス X/Y（鼠标），(5) 称"必须立即改" | ENGINE_EVENTS.md:412-417：= 主人公画面坐标；鼠标在 1600012/1600013；由 CE225 槽 49/50 读 124(0x1000, obj -2, f6/7) 再发布到 9000071/72 | **不采纳"改回鼠标"**；仓库现状正确 |
| 250 容器/位域 | (4)：容器=(flags>>8)&0x0f、勿用 &3；assignment=flags&0xff；db_mode=options>>4 | OPCODE_250.md:37-39：容器=(flags>>8)&3，2195/2195 零例外；方向 bit12；方法 (flags>>4)&7 | **以仓库为准**；报告位域布局与本作数据不符 |
| CE242/243 判定 | (4)(5)：0 静态调用者 → 疑死代码，判据是"能否从 Hunt/赛事 call graph 达" | BATTLE.md §0.1 / auto_events.rs:54-70：二者是 condvar 门控（2000350==1/==2）的 call_only 事件，按条件变量启动，不靠 call graph | **报告"查 call graph"思路不适用**；仓库已用门控实现 |
| 290 type 5/6 | (2)：type3=闪B 等 | Editor.exe：type5=点滅A[明滅]，type6=点滅B（本作从未用） | 以仓库/Editor.exe 为准 |
| 130 | (2) 旧顺序错；(4) direction=raw 猜测不足 | exec.rs:1790-1798 已按 target,x,y,map,options 落地；标题→Map003→SampleMapA 跑通（HANDOFF_M4.md:53-54、m6_mainline.rs:132-139） | **(4) 的 options bitfield 判断与仓库一致**，可作 P0 differential fixture 依据 |

---

## 3. 与仓库既有实现的对照表

> "本仓库现状"一栏：能核对者给文件:行号；标注"文档记载"的为仓库 docs 自述、本 agent 未运行 cargo 复证。

| 主题 | 报告结论（要点） | 本仓库现状（文件:行） | 建议动作 |
|---|---|---|---|
| 130 场所移动 | (2) map,x,y,direction；(4)(5) target,x,y,map,options + options bitfield；-1 主角/-2 变量；地图在 arg3 | exec.rs:1790-1798 解码 target,x,y,map,options；exec.rs:553-561；game.rs:1728-1736 入队；game.rs:1080-1107 switch_map；db.rs:388-393 地图 id 有符号(-1=行1/-2=行0)；tests/m6_mainline.rs:132-139 标题→Map003→SampleMapA；docs/HANDOFF_M4.md:53-54 | **立即修=已修**（保留 differential fixture 回归） |
| 150 图片 | (4) 表示/移动/消去/延迟重置；range 闭区间；处理时间≠延迟；bit12-14 锚点 | exec.rs:1825-1829；crates/wolf-game/src/picture.rs（range 见 1160-1195）；docs/UNVERIFIED.md H-PIC-1/2、H-ANC-1 | 后续里程碑：补 [2]/[5] 槽与 0x2000.. 锚点语义 |
| 151 变色 | (2) RGB 0-255；(4) RGB 0-200/100 中性 | exec.rs:1830-1834 + :568-576；game.rs:1738-1742；tint.rs:48-101（byte/100，100=中性，未夹 200） | 仅记录；0-200 夹取需实测确认（现按百分比） |
| 160/161/162 转场 | (4) 三阶段 指定/准备/执行；raw flags 未证 | exec.rs:1835-1864 + :563-566；game.rs:1744-1763；transition.rs:1-60（kind<0 硬切，否则按 frames 淡入淡出；真 mask Fade.png 未复刻） | 后续里程碑：接 SystemFile/[Transition]Fade.png 真实遮罩 |
| 250 数据库 | (4) 可変DB/システムDB/ユーザDB/XY 配列；容器 &0x0f、db_mode options>>4 | exec.rs:2003；db.rs:68-101（负行 -1 查行数/-2 设行数/100+n 运行槽；负列 -1/-2）、248-400；docs/OPCODE_250.md | **已实现**负行/列哨兵（BATTLE.md 旧缺口已补）；报告位域建议不采纳 |
| 290 效果 | (2) 5 类；(4) 11 类 0..10；(5) 官方 3.5 列 11+（角度/不透明度等） | exec.rs:2079-2091（kind=head>>4）；picture.rs:237-266 常量 0..10、:1168-1258 处理 10 类（type6 本作未用，故意不实现）；docs/OPCODE_290.md:30-40 | **已实现 10/11**；与 (4) 一致，(2) 作废 |
| run_condition | (4)(5) 四态；不要加 id==63/241 特例 | commonevent.rs:32-92、260-266；auto_events.rs:1-130（0/1/2/3 全覆盖；SKIPPED=[43] 死代码）；game.rs:526-530 | **已实现**；与 (4)(5) 一致 |
| 存档 | (2) 未找到格式；(4)(5) Steam Cloud + 手动，SaveData01 是否自动未证 | crates/wolf-save/src/lib.rs（SavePart1-7）；docs/SAVE_SPEC.md（9/9 实测） | **已实现**；报告无有效信息（Cloud 猜想仅记录） |
| 战斗 | (3) 回合制 AP+2/10 回合/战力=HP；(4) Unique Combat System、UI ATK/CRT/DEF/BLK/CNT；(5) 七列 Rep/ATK/SKI/DEF/DET/INT/SP、Energy 非固定次数 | docs/BATTLE.md（CE239 入口/CE188 主循环/CE244 判定/2000350 三态；run 239 PASS 4/4 文档记载）；auto_events.rs:54-70（242/243 门控） | 后续里程碑：正式 Hunt/Eights 入口接线 + 报告数值逐版本回归 |
| 日程/时间 | PDF/攻略 天数表（Day11..106）、分支、能量消耗 | docs/TIME_SYSTEM.md（时/分/星期/周末标志 = 变量 4/5/6/8，TIME_SYSTEM.md:44-53）；**无独立"日程式章节调度器"**，剧情按 CommonEvent + 变量门控 | 后续里程碑：GameCalendar/StoryFlag/QuestTemplate 数据化（PDF §10 实体） |
| H 场景 | (0)(1) 阈值/体位/怀孕 End 等攻略 | docs/HSCENE.md（事件闭包/图片号/资产台账；运行时渲染待补） | 后续里程碑（渲染纵切） |
| 音频 | (2) MIDI/WAV/MP3/OGG | crates/wolf-audio（decode/mixer/engine）；docs/AUDIO.md；本作实测 mp3/ogg/wav | **已实现**；无 MIDI（仅记录差异） |
| UI | (5) HUD 字段/标题选项 | crates/wolf-game/src/{message,choice,window,textpic,sysgraphic}.rs；截图核对 §5 | **已实现**（M2）；报告仅作字段参考 |
| 标题 | (5) 新游戏问答得 Title；RTA 从「はじめるぞ」；疑非 fork | game.rs:42-49（TITLE_EVENT=42 / BOOT_MAP=TitleMap）、:723-732 start_title；wolf-render/examples/title_blit.rs；game.rs:75 标题像素 parity 0.130 | **已实现**；报告标题观感为仅记录 |

---

## 4. 对重实现最有价值的可执行清单（P0/P1/P2，每条含验收）

> 优先级按"是否影响正确性/当前行为"排序，不按报告原有 P0/P1/P2（报告 P0 多数在本仓库已完成）。每条给验收方式。

### P0（正确性 / 当前行为）
1. **130 布局回归 fixture**：固定 5 个真实站点（Map003 ev1 [-1,62,38,1,1]；标题 CE42 [-1,60,34,3,1]；Map002 4×-2）断言 target/x/y/map/options 解码与 switch_map 目标。**验收**：测试断言 diagnostics 出现 MapData/Map003.mps 与 SampleMapA（对齐 m6_mainline.rs:132-139），且无 "no map registered"。
2. **250 负行/负列哨兵**：断言 -1=查行数、-2=设行数、100+n 运行期槽、负列 -1/-2 清空/首空列。**验收**：db.rs 单测 + CE48 pc153 / CE191 pc30 / CE157 pc28 真实站点跑通不 panic、读写值正确。
3. **9000071/72 语义钉死**：以 ENGINE_EVENTS.md:412 为准，断言 CE225 槽 49/50→9000071/72，鼠标=1600012/13。**验收**：禁止任何"把 9000071/72 当鼠标"的读取；加注释与测试。
4. **210/300 返回值路径**：对 376 个 rv=1 站点抽样断言 caller[dest]←callee.self[1_600_000+rv]。**验收**：OPCODE_210_RETURN 命令重跑一致 + 单测覆盖无返回值(0xFFFFFFFF)不复制。
5. **run_condition 四态调度**：断言 0 不扫条件、1 独占阻塞、2 条件并行、3 常时；不加 id==63/241 特例；43 列 SKIPPED。**验收**：auto_events.rs 现有测试 + 全库 histogram {442,1,97,10}。

### P1（覆盖度 / 中期）
6. **151/160/162 视觉保真**：151 百分比 clamp 策略实测确认；160/162 接 Fade.png 遮罩。**验收**：capture 截图与零售对照（tint/transition 单测 + imgdiff）。
7. **290 补全**：type6（若出现）、[2]/[5] 槽、0x2000.. 锚点补齐后像素对照。**验收**：docs/UNVERIFIED 对应条目转为 FORMAT_SPEC「已验证」。
8. **战斗正式入口接线**：从公會 Hunt / Tournament of the Eights 的 210 入边进 CE239，跑通至少一战。**验收**：wolf-cli run --event <entry> --frames N 无 warning、胜负变量 2000352 正确；BATTLE.md §5 三阶段。
9. **日程/任务数据化**：引入 GameCalendar/QuestTemplate/QuestInstance/StoryFlag（PDF §10），落地 Day11..106 天数表 + 三进度模式。**验收**：单测模拟 Day15 讨伐进度 3→0 触发战斗、Day64 进度 50/3 天超时。
10. **数值回归**：把 PDF/Steam/攻略的职业初始、公会活动增减、能量消耗按"版本+技能"分列做参数化回归。**验收**：表驱动测试，标注来源与版本，冲突值各自保留。

### P2（打磨 / 记录）
11. **"脚本 vs 引擎"清单**：标题/HUD/菜单/战斗/存档全部证为项目 CommonEvent 而非 Rust 内建（报告 (4) 结论）。**验收**：ARCHITECTURE.md 增一栏"哪些是脚本、哪些是引擎"。
12. **音频补 BGS/3D/流式**（AUDIO.md 未来项）；**H 渲染纵切**（HSCENE）。
13. **死代码清理**（CE43 已 SKIPPED；报告 (4) P2）。
14. **术语/UI 中英日对照固化**到 docs/（本文件 §5 可作起点）。

---

## 5. 关键术语 / UI / 数值表

### 5.1 术语/UI 对照（合并 PDF §07 + 报告 (1)(3)(5)）

| 日文 | 英文 | 中文建议 | 含义 / 来源 |
|---|---|---|---|
| 行動力 | Energy / AP | 行动力 | 探索和训练消耗；可与 Stamina 分离（PDF §07、截图） |
| 精力 | Stamina | 精力 | 独立资源（截图 3/3）；报告(1) 上限 9 |
| 進行度 | Progress | 进度 | 可正向 0→100 或倒数 3→0（PDF 三模式） |
| 休息 | Rest | 休息 | 黑点次数 / 禁休区 |
| 妹体調 | Sis Health | 妹妹体调 | 长程探索生存资源 |
| 進む / 背負う / 帰還 | Proceed / Carry / Return | 前进 / 背上 / 返回 | 大冒险 / 远征 |
| ギルド評判 | Guild Reputation | 公会声望 | 工资与商品可售性 |
| 討伐依頼 / 地域貢献 | Hunt / Community Service | 讨伐 / 地区贡献 | 工作日公会活动 |
| 大冒険だ! / 超大冒険 | Adventure time! / Super | 大冒险 / 超大冒险 | 周末 100 / 通关后 |
| 攻撃 / 技術、防御 / 根性、賢さ、スキルPT、所持金、戦力 | ATK/SKI、DEF/DET、INT、SP、Money、Battle Score | 攻击/技术、防御/根性、智慧、技能点、持有金、战力 | 战斗七列（截图 + 报告(5)） |

### 5.2 战斗数值 / UI（截图实测 + 报告）

| 字段 | 截图观测 | 报告说法 | 说明 |
|---|---|---|---|
| Day / 时刻 | Day1 23:00 Mon（英）/ Day1 22:30 星期一（中） | Day 计数 | 开局 |
| Energy | 30/60 | 非固定次数（PDF/(5)） | 上限随训练成长 |
| Stamina | 3/3 | 精力独立 | 报告(1) 上限 9（攻略） |
| Money | 2846 G / 2932 G | — | — |
| Guild Rep | Totally Obscure / Normal；无名 / 普通 | 等级阈值不显进度（PDF §06） | — |
| Trust / Mood | 20 / Normal | 妹妹 4 值（1） | — |
| ATK/SKI/DEF/DET/INT/SP | 40/6/9/8/7/0（英）；40/6/28/12/9（中上）；17/6/20/4/4（中下） | 七列（5）；ATK+7/+9、DEF+9/+11 版本差 | 随存档/角色/技能变化 |
| CRT / BLK / CNT | 截图未见 | (4) 零售截图口述 | 待复核（可能另一版本/HUD 态） |
| ★ / Battle Score | 截图未读数 | (5) Battle Score=战力，xref 2000350 | 待实测 |

战斗规则（报告 (3)(5)，攻略级，均标 C）：每回合每角色行动一次；动作=攻击/技能/防御/道具；消耗 AP；回合开始全队 AP+2；10 回合未分比战力；敌战力 0 胜、己 0 败。需以 CE188 脚本回归。

### 5.3 日程 / 日期事件表（合并 PDF §05 + 报告 (0)(3)）

| Day | 事件 | 条件 / 分支 | 证据 |
|---|---|---|---|
| Day1 | 新游戏（Title→Map003→SampleMapA） | — | 仓库 HANDOFF_M4.md |
| Day11 | 美食倶楽部交流战 | 败亦可推进 | B |
| Day15 | 日常讨伐开启（随机 3 选 1） | 进度 1/3 截图 | B |
| Day18 | 地区预选 + ライバル决斗 | 胜负影响 24-29 | B |
| Day24-29 | 公会经营/海豹支线/学厨艺 | 18 日胜败 + 做饭变量 | B/C |
| Day33 | 公会经营决定 / 特殊分歧 | 需料理事件，否则農業エンド | B/攻略 |
| Day39-43 | Kasumi 城镇初遇 / 入队 | — | B |
| Day46 | Massive Brothers 交流战 | — | B |
| Day53 | 美食倶楽部地区预选 | — | B |
| Day55 / 57-61 | 训练营 | 启动日 55 vs 57 分歧 | B |
| Day64 | 联合讨伐（3 天，进度 50，乌里坊×100） | 主线可跳过 | B |
| Day74 | 一回战 Scout Union | 特定回合 UI 异常 | B |
| Day78 | 半决赛 ナナホシ事務所 | 特殊敌援 | B |
| Day84 | 决赛 / 乱入（第 5 回合） | — | B |
| Day85-99 | 遗迹讨伐（15 天，进度 40） | 15 日未解→雾事件 | B |
| Day95（条件式） | 与 rival 再战 / 结局 | 需大冒险 100 + 遗迹完成 | B |
| Day100 后 | 昼间日常可能锁定 | — | B |
| Day106+ | 后日谈 温泉 / 猴群战 / 属性上限 500 | 复合条件 | B |

注：报告 (0) 另给 Day25/52/54/57/78/84/95 细目与阈值（攻略级，见 §2.2），不与上表冲突处保留。

### 5.4 状态值（多源合并，标注性质）
- 报告 (1)（攻略）：妹妹 4 值 信頼/興味/感度/キゲン；キゲン 5 档 爆发临界/不快/普通/愉快/兄Love。
- 报告 (0)（攻略）：Lust（BLust/SLust）/Trust/Interest/Mood/Energy/Condition/Rep。
- 报告 (3)（攻略）：Energy/Lust(20%)/Trust/Interest/感度/Mood；消费★（推测）。
- 截图（B）：Trust 20、Mood Normal、糖度 上機嫌、女性関係 0、性慾 0。
- 本仓库：能量/精力/时间/星期/周末标志为引擎变量（TIME_SYSTEM.md）；妹妹四值等为脚本变量（未专用核对）→ 仅记录。

---

## 6. 来源清单

### 6.1 六份 .md 报告内引用

| 文件 | 报告内引用 / 来源 | 备注（推测明示） |
|---|---|---|
| deep-research-report.md | 无外链；早期版本 | 全文属 C/攻略；多处自评"推测/未找到"；混用旧作名 |
| deep-research-report (1).md | 无外链；自标"玩家攻略" | 职业/特性/道具/妹妹值全为 C |
| deep-research-report (2).md | 引"官方手册"（run_condition=0 呼び出しのみ）；130/151/250/290 自标推测 | 混合：仅 run_condition 有手册依据；其余多推测 |
| deep-research-report (3).md | 无外链；攻略型 | 回合制/数值为 C |
| deep-research-report (4).md | github take-cheeze/rpg-maker-clone（ADR 0065；mruby-wolf/mrblib/interpreter.rb、vars.rb、scripts/wolf/interpreter_check.rb、mruby-wolf/test/wolf_test.rb）；Sinflower/WolfTL（opcode 名表）；djytw/wolf-rpg-formats（Kaitai 2.2x/3.0x/3.3x）；wolfrpg-map-parser（Rust） | 证据档位自述；9000071/250 位域两条与本仓库实测冲突（§2.4） |
| deep-research-report (5).md | Steam 攻略 / 西语攻略 / 官方手册 | 战斗/UI 为 B+手册；130/290 细节部分推测 |

### 6.2 PDF [S01]-[S17] 原链接

| 编号 | 来源级别 | 链接 / 说明 |
|---|---|---|
| S01 | 开发者/官方 | https://inusuku.dojin.com/ （作品定位、系列辨析） |
| S02 | 开发者/官方（2022.02） | https://masukuinu.sakura.ne.jp/imouto/mono_02/index.html |
| S03 | 发行商/官方（2024.03.22） | https://store.steampowered.com/app/2429860/ |
| S04 | 发行商/官方 | https://www.kaguragames.com/product/living-with-sister-monochrome-fantasy/ |
| S05 | 发行商/官方 PDF（本体 v1.02 / DLC v2.02） | https://kaguragamer.com/wp-content/uploads/2024/12/…妹相随DLC相关问题.pdf |
| S06 | 玩家攻略（2026.05 更新） | https://gamerch.com/biim/339950 |
| S07 | 玩家攻略+实机图（2025.12 更新） | https://gamerch.com/biim/326067 |
| S08 | 玩家实测（2024.04 发布、2025.01 更新，含 DLC） | https://steamcommunity.com/sharedfiles/filedetails/?id=3213959374 |
| S09 | 玩家攻略（自称 v1.02，2026.03） | https://www.gameguidewebsite.com/post/LivingWithSister.html |
| S10 | 第三方技术索引 | https://steamdb.info/app/2429860/ufs/ |
| S11 | 玩家技术记录（v1.02—v1.04） | https://steamcommunity.com/profiles/76561198137446192/recommended/2429860/ |
| S12 | 开发者 Ci-en（访问受限，仅索引提示） | https://ci-en.dlsite.com/creator/64/article/600362 |
| S13 | 玩家实机截图（探索 26/100） | https://images.steamusercontent.com/ugc/2486623401418048093/… |
| S14 | Gamerch 公会讨伐截图（第 15 日 1/3） | https://cdn.gamerch.com/contents/wiki/1818/entry/qIG9R76a.jpg |
| S15 | Gamerch 大冒险截图（第 62 日 1/100） | https://cdn.gamerch.com/contents/wiki/1818/entry/OdchPM9w.jpg |
| S16 | Gamerch 讨伐战斗截图（第 66 日 うりぼう×100） | https://cdn.gamerch.com/contents/wiki/1818/entry/Fwvjk2ww.jpg |
| S17 | YouTube 实况（2:15:22，无逐帧时间戳） | https://www.youtube.com/watch?v=IHdFa1YP9B4 |

### 6.3 图片来源（本目录）
- BxrYxVoG6GbITkXCMTaPbA8bwSHm2R_UDXcsOOTVbU5OU1fseWnsSwLOQYBs979LE_vP5G9ANy5YRcxx5T7mNrdEw47JKVIo9yDwIF2yvJC-kWOzvK5gfxBA7fdGVFflvj8UqzvrRX70q6sut6RCE9A-8UqYydHWxrwTyPzfYPycxBG-5wg9oImGoLi5uA5Q.jpg（800×800，128776 B）——中文界面复合截图，**建议人工确认来源**。
- t2dRIy0r9r3WCX-npIwv76mW_Gc7vNIZkYsoksgyrRTIMGMYJc72GKJXoWiFznmxD_YctrE659k36mvJcwkM9uHq9Td8dT1fsZTKZ-xxn-Ql3Es5mzerYfbxzmWFKtqA4FMYME2PlnVCZ9QvFXk1abvpl1q28xAcSPzTwkxWA8w.jpg（1529×860，143216 B）——英文实机截图，**建议人工查看**（可核对 §5）。

### 6.4 "未找到（U）"汇总（重实现一律标 U，勿编造）
地图总数 / MapData ID；掉落表 / 宝箱点位 / 采集点 / 掉率 / 刷新；随机遇敌概率与生成池；周薪可复算公式；随机任务权重；版本精确对应关系；CE 内部变量编号 / 优先级 / 互斥判定顺序；逐帧视频时间戳；怪物 HP/ATK/DEF 与掉落 ID。

---

### 附：本文件生成约束（供 Lead 复核）
- 只读输入：未修改任何代码或其他文档；未提交 git。
- 未触碰：crates/wolf-format/examples/opscan.rs、crates/wolf-format/tests/_scratch_cdata.rs、crates/wolf-format/tests/_scratch_event.rs。
- 未运行 cargo test/build（按要求由 Lead 执行）。
- 本文件引用行号基于本次会话读取时的仓库工作树；若 Lead 后续改动可能偏移，按文件/函数名重新定位。
