# BGM 音源事实核查与升级可行性审查报告

> **审查对象**：《妹!せいかつ～ファンタジー～》（Living With Sister: Monochrome Fantasy）重制版已解包媒体库的**全部音乐资产**
> **审查日期**：2026-11-23
> **触发来源**：`docs/调研/《妹!せいかつ～ファンタジー～》高音质音源检索与替换指南.md`（第三方检索产出）
> **结论摘要**：原检索指南的**检索词体系可用**，但其**技术前提与本工程不符**，且按其建议**直接替换会让绝大多数曲目的音质变差**。本报告给出逐曲实测对照、勘误清单与可安全执行的替换流程。

---

## 0. 核心结论（先读）

| # | 结论 | 依据 |
| :--- | :--- | :--- |
| 1 | **本游戏不是 RPG Maker MV 作品**，原指南的 `.rpgmvo` 解密、`www/audio/` 目录结构假设在本工程不成立 | 运行时为 WOLF 引擎（`AssetDB.gd` / `AudioManager.gd` / opcode 140），媒体以 `work/retail_dat/{BGM,SE,DLC_data,SystemFile}` 解包形态存放 |
| 2 | **DOVA-SYNDROME 已于 2026-09-15 更名为 OpenTracks**，`dova-s.jp` 现 301 跳转至 `opentracks.com` | `curl -I https://dova-s.jp/` → `Location: https://opentracks.com/` |
| 3 | **源站免费下载上限为 320 kbps MP3，而零售版音乐多数已是 380–465 kbps Vorbis**——"换成高质量音源"的前提对多数曲目**不成立** | 逐曲实测：见 §4 对照表 |
| 4 | 真正的升级候选**只有 1 个经互相关确认**：`BGM/yuuyake.mp3`（128 kbps → 321 kbps 母版）。另有 2 个仅码率占优但**同一性未确认**，不建议直接替换 | 见 §4.6 |
| 5 | 零售版音乐文件是**母版的前缀截取**（互相关峰值 0.987），替换必须**保持时长与采样率一致**，否则循环周期与 loop 标签同时失效 | 见 §3.3 |
| 6 | 原指南的检索词表中**存在 12 处与媒体库对不上的条目**（含幻觉曲目与错配的 DLC 曲目），已在 §5 逐条勘误 | 零售文件内嵌 `TITLE`/`ARTIST` 标签 vs 原指南检索词 |
| 7 | OtoLogic（`otologic.jp`）**当前对本机返回 403**，其管辖的 2 个 BGM 文件无法取得母版 | `requests.get("https://otologic.jp/")` → 403，二进制拦截页 |

---

## 1. 审查方法

1. **媒体库画像**：对 `work/retail_dat/` 下全部 183 个音频文件跑 `ffprobe`，建立 `码率 / 采样率 / 时长 / 容器 / 循环标签 / 内嵌元数据` 台账。
2. **源站寻址**：按零售文件**自带的 `TITLE`/`ARTIST` 标签**（而非人工猜测）反查源站曲目，再以**标题精确匹配 + 时长接近**双条件锁定唯一条目。
3. **码率换算**：源站下载页公布每个下载变体的**文件大小**；以 `大小 × 8 ÷ 时长` 反推源码率，与零售版实测码率直接比较——这决定了替换是升级还是降级。
4. **内容同一性验证**：对候选母版与零售文件做 8 kHz 单声道**归一化互相关**（能量包络域），确认二者是同一段录音并求出对齐偏移；峰值 < 0.80 一律判为"非同一音源"并拒绝替换。

> 本报告全部数据由 `work/out/audiosrc/` 下的脚本产出，产物可复现（见 §8）。

---

## 2. 现有媒体库画像

### 2.1 音乐资产分布

| 目录 | 文件数 | 合计体积 | 说明 |
| :--- | ---: | ---: | :--- |
| `BGM/` | 40 | 212.9 MiB | 正篇 BGM，全部被 `opcode 140` 引用 |
| `DLC_data/` | 10 | 136.9 MiB | 8 个 DLC BGM + `03 2NEONS.ogg` + 1 个 SE |
| `SE/` | 120 | 16.0 MiB | 音效，含 20 个无损 WAV |
| `SystemFile/` | 13 | 0.8 MiB | 旧版 `[SE]*.ogg`，**游戏数据中零引用**（死资产） |
| **合计** | **183** | **366.6 MiB** | |

### 2.2 码率分布（`BGM/` 40 个）

| 码率区间 | 数量 | 典型容器 | 评价 |
| :--- | ---: | :--- | :--- |
| 440–465 kbps | 11 | Vorbis | 已达"透明"水平，**不应替换** |
| 365–435 kbps | 10 | Vorbis | 同上 |
| 320 kbps | 3 | MP3 @48 kHz | 良好 |
| 191–226 kbps | 4 | MP3 / Vorbis | 中等 |
| **128–162 kbps** | **12** | MP3 / Vorbis | **唯一有升级空间的一档** |

### 2.3 关键结构事实

* **循环点全部为 0**：`game_data/programs.json` 中 1611 条 `opcode 140` 指令的 `intArgs[6]`（60 Hz 帧）无一例外为 0。
* **循环信息实际藏在音频文件里**：`BGM/` 下 27 个 `.ogg` 中有 **25 个**带 `LOOPSTART`/`LOOPLENGTH` Vorbis 注释（单位：采样点）；`docs/engine/AUDIO.md` 已有同样结论（"49 BGM files, 26 tagged"）。
* **`.mp3` 永不带循环标签**，故 `BGM/` 的 13 个 `.mp3` 与全部 DLC `.mp3` 在原引擎中只能整曲循环。
* **零售文件保留了母版的元数据指纹**，这是反查原曲最可靠的线索：

  | 零售文件 | 内嵌 `TITLE` | 内嵌 `ARTIST` |
  | :--- | :--- | :--- |
  | `BGM/Acoustic_High_Collar.ogg` | `dova_High Collar ACG ver` | — |
  | `BGM/Migratory_Birds.ogg` | `dova_Migratory Birds mix` | — |
  | `BGM/The_End_of_Destruction.ogg` | `dova_The End of Destruction Loop_master` | — |
  | `BGM/Die_Letzte_Revolution.ogg` | `dova_Die Letzte Revolution_master` | — |
  | `BGM/madoromu2.ogg` | `まどろむ時間２` | まんぼう二等兵 |
  | `BGM/waste_loop.ogg` | `荒野を旅する風` | shimoya |
  | `BGM/centennial_loop.ogg` | `百年王国` | shimoya |
  | `BGM/soul_engage_loop.ogg` | `SOUL ENGAGE` | shimoya |
  | `BGM/Killer_Roll.ogg` | `Killer Roll` | まんぼう二等兵 |
  | `BGM/Clock-Second_Hand03-1(Loop-1).mp3` | `Clock-Second_Hand03-1(Loop-1)` | **OtoLogic** |
  | `BGM/oyasumi no kiss wo1.mp3` | `oyasumi no kiss wo1` | **OtoLogic** |
  | `BGM/sparrows.mp3` | — | **(c)Taira Komori**（効果音ラボ） |

---

## 3. 源站事实核查

### 3.1 OpenTracks（原 DOVA-SYNDROME）

| 项 | 实测值 |
| :--- | :--- |
| 站点 | `https://opentracks.com`（`dova-s.jp` 301 跳转至此） |
| 曲目页 | `/bgm/detail/<id>` |
| 检索 | `POST /bgm/search`（Django 表单，需 CSRF token + 会话 Cookie），**多词为 AND**，结果分页（`?page=N`，每页 5 条） |
| 下载 | `POST /bgm/detail/<id>/download`，表单字段 `csrfmiddlewaretoken` + `track`；**直接返回音频字节**，非 zip |
| 限制 | 「サーバ負荷軽減のため、同じファイルの連続ダウンロードは出来ません」——同一文件不可连续下载 |
| **格式上限** | **MP3，实测 128 / 160 / 192 / 242 / 320 kbps**；未见 WAV/FLAC 选项 |

### 3.2 其他源站可达性

| 素材站 | 状态 | 实测 |
| :--- | :--- | :--- |
| 効果音ラボ `soundeffect-lab.info` | ✅ 可达 | 直链需带 `Referer`（否则 403）；提供 MP3 |
| OtoLogic `otologic.jp` | ❌ **403** | 返回二进制拦截页，母版不可得 |
| 魔王魂 `maou.audio` | 未在本轮涉及 | 零售 `SE/` 中未见其指纹 |

### 3.3 内容同一性验证（关键）

对 `BGM/Killer_Roll.ogg`（零售 180.09 s / Vorbis 453 kbps）与 OpenTracks `#15317 Killer Roll`（母版 320.46 s / MP3 192 kbps）做 8 kHz 能量包络互相关：

```
best offset = 0.016 s     peak_corr = 0.9874
```

**零售文件就是母版从 0 开始的前 180.09 秒**。推论：

1. 母版更长（母版含完整前奏 + 完整段落，零售只保留循环体），二者**不可直接互换**——直接换会让循环周期从 ~180 s 变成 ~320 s。
2. 正确做法是**按零售时长裁切母版**，再重编码。
3. 裁切后新旧文件时间轴完全对齐，Vorbis `LOOPSTART`/`LOOPLENGTH` 的**采样点数值可以原样搬运**。

---

## 4. 逐曲对照表

> 判定口径：把源站下载页公布的**文件大小 ÷ 该曲时长**换算成源码率，与零售版文件的实测码率比较。
> `≥1.15×` 判为升级，`0.92–1.15×` 判为持平，`<0.92×` 判为降级。

### 4.1 ✅ 升级（3 个）

| 媒体库文件 | 零售版 | 源站曲目 | 源站 ID | 源码率 | 比值 | 源时长 | 判定依据 |
| :--- | :---: | :--- | :---: | :---: | :---: | :---: | :--- |
| `BGM/p_egg_loop.ogg` | 133 kbps vorbis | 夢の中で呼ばれた（`559`） | 559 | 161 kbps | 1.206 | 109s | duration-match |
| `BGM/train.mp3` | 128 kbps mp3 | Train（`11485`） | 11485 | 321 kbps | 2.508 | 182s | exact-title |
| `BGM/yuuyake.mp3` | 128 kbps mp3 | 夕焼けレモンティー（`2305`） | 2305 | 321 kbps | 2.509 | 199s | exact-title |

### 4.2 ➖ 持平（3 个）

| 媒体库文件 | 零售版 | 源站曲目 | 源站 ID | 源码率 | 比值 | 源时长 | 判定依据 |
| :--- | :---: | :--- | :---: | :---: | :---: | :---: | :--- |
| `BGM/Veil_Nebula.mp3` | 320 kbps mp3 | Veil Nebula（`13694`） | 13694 | 320 kbps | 1.001 | 209s | exact-title |
| `BGM/White_Base.mp3` | 160 kbps mp3 | White Base（`10803`） | 10803 | 161 kbps | 1.007 | 140s | exact-title |
| `BGM/nonbiri.mp3` | 160 kbps mp3 | のんびりした日々（`6476`） | 6476 | 160 kbps | 1.002 | 79s | 互相关确认 |

### 4.3 ⛔ 降级（16 个）

| 媒体库文件 | 零售版 | 源站曲目 | 源站 ID | 源码率 | 比值 | 源时长 | 判定依据 |
| :--- | :---: | :--- | :---: | :---: | :---: | :---: | :--- |
| `BGM/Killer_Roll.ogg` | 453 kbps vorbis | Killer Roll（まんぼう二等兵）（`15317`） | 15317 | 192 kbps | 0.425 | 320s | exact-title |
| `BGM/Tenseness.ogg` | 417 kbps vorbis | Tenseness（`2064`） | 2064 | 320 kbps | 0.768 | 113s | exact-title |
| `BGM/ango.ogg` | 454 kbps vorbis | アンゴラたちの踊り（まんぼう二等兵）（`12152`） | 12152 | 192 kbps | 0.424 | 390s | exact-title |
| `BGM/destiny.ogg` | 435 kbps vorbis | Destiny（`7692`） | 7692 | 160 kbps | 0.369 | 187s | exact-title |
| `BGM/line.ogg` | 415 kbps vorbis | Loneliness of the sea（`5359`） | 5359 | 322 kbps | 0.777 | 160s | duration-match |
| `BGM/morning.ogg` | 383 kbps vorbis | Morning（`2445`） | 2445 | 320 kbps | 0.835 | 73s | exact-title |
| `BGM/nagare.mp3` | 226 kbps mp3 | NAGARE（`23584`） | 23584 | 156 kbps | 0.689 | 182s | exact-title |
| `BGM/okiraku.ogg` | 417 kbps vorbis | おきらくシュガー（まんぼう二等兵）（`15572`） | 15572 | 192 kbps | 0.46 | 423s | exact-title |
| `BGM/windletter.ogg` | 365 kbps vorbis | South Wind（`9297`） | 9297 | 192 kbps | 0.526 | 150s | duration-match |
| `DLC_data/bgm01.ogg` | 425 kbps vorbis | 奥羽山脈を越えて（`17758`） | 17758 | 320 kbps | 0.754 | 357s | exact-title |
| `DLC_data/bgm02.ogg` | 387 kbps vorbis | お狐さまと夢うつつ（`16754`） | 16754 | 320 kbps | 0.826 | 350s | exact-title |
| `DLC_data/bgm03.ogg` | 403 kbps vorbis | ハツユキ・ウーリッツァー（`15657`） | 15657 | 320 kbps | 0.793 | 320s | exact-title |
| `DLC_data/bgm04.ogg` | 348 kbps vorbis | 休み時間のおしゃべり（`13647`） | 13647 | 320 kbps | 0.919 | 170s | exact-title |
| `DLC_data/bgm06.ogg` | 308 kbps vorbis | 緑色の木漏れ日（MT）（`9978`） | 9978 | 96 kbps | 0.312 | 102s | exact-title |
| `DLC_data/bgm07.ogg` | 358 kbps vorbis | 畦道（MT）（`9548`） | 9548 | 192 kbps | 0.537 | 109s | exact-title |
| `DLC_data/bgm08.ogg` | 425 kbps vorbis | 緑色の木漏れ日（Yoshinori Tanaka）（`9978`） | 9978 | 96 kbps | 0.226 | 102s | exact-title |

### 4.4 ❔ 未找到（28 个）

| 媒体库文件 | 零售版 | 源站曲目 | 源站 ID | 源码率 | 比值 | 源时长 | 判定依据 |
| :--- | :---: | :--- | :---: | :---: | :---: | :---: | :--- |
| `BGM/Acoustic_High_Collar.ogg` | 381 kbps vorbis | — | — | — | — | —s | no candidate |
| `BGM/Across_-Battle_Results-.ogg` | 450 kbps vorbis | — | — | — | — | —s | no candidate |
| `BGM/Be_yourself.ogg` | 414 kbps vorbis | — | — | — | — | —s | unresolved |
| `BGM/Clock-Second_Hand03-1(Loop-1).mp3` | 61 kbps mp3 | source is OtoLogic, not OpenTracks（OtoLogic） | — | — | — | —s | no candidate |
| `BGM/Die_Letzte_Revolution.ogg` | 381 kbps vorbis | — | — | — | — | —s | no candidate |
| `BGM/Migratory_Birds.ogg` | 449 kbps vorbis | — | — | — | — | —s | unresolved |
| `BGM/RAIN_&_Co.ogg` | 460 kbps vorbis | —（ISAo.） | — | — | — | —s | no candidate |
| `BGM/Sunshine!.ogg` | 465 kbps vorbis | — | — | — | — | —s | no candidate |
| `BGM/The_End_of_Destruction.ogg` | 431 kbps vorbis | — | — | — | — | —s | no candidate |
| `BGM/We_are_Going.ogg` | 448 kbps vorbis | — | — | — | — | —s | unresolved |
| `BGM/autumncome.ogg` | 427 kbps vorbis | — | — | — | — | —s | no candidate |
| `BGM/centennial_loop.ogg` | 153 kbps vorbis | —（shimoya） | — | — | — | —s | no candidate |
| `BGM/madoromu2.ogg` | 409 kbps vorbis | —（まんぼう二等兵） | — | — | — | —s | no candidate |
| `BGM/nc51105.ogg` | 194 kbps vorbis | — | — | — | — | —s | no candidate |
| `BGM/nekomimi.mp3` | 320 kbps mp3 | — | — | — | — | —s | no candidate |
| `BGM/nemesis_loop.ogg` | 162 kbps vorbis | — | — | — | — | —s | no candidate |
| `BGM/oyasumi no kiss wo1.mp3` | 128 kbps mp3 | source is OtoLogic, not OpenTracks（OtoLogic） | — | — | — | —s | no candidate |
| `BGM/se_akimusi2.ogg` | 151 kbps vorbis | — | — | — | — | —s | unresolved |
| `BGM/soul_engage_loop.ogg` | 161 kbps vorbis | —（shimoya） | — | — | — | —s | no candidate |
| `BGM/sparrows.mp3` | 162 kbps mp3 | source is Taira Komori (効果音ラボ), not OpenTracks（(c)Taira Komori） | — | — | — | —s | no candidate |
| `BGM/straightcontrast.ogg` | 428 kbps vorbis | — | — | — | — | —s | no candidate |
| `BGM/syuuenn.mp3` | 320 kbps mp3 | — | — | — | — | —s | no candidate |
| `BGM/takibi.mp3` | 192 kbps mp3 | —（My Recording） | — | — | — | —s | no candidate |
| `BGM/waste_loop.ogg` | 162 kbps vorbis | —（shimoya） | — | — | — | —s | no candidate |
| `BGM/wave1.mp3` | 192 kbps mp3 | — | — | — | — | —s | unresolved |
| `DLC_data/03 2NEONS.ogg` | 507 kbps vorbis | —（isao） | — | — | — | —s | no candidate |
| `DLC_data/bgm09.mp3` | 320 kbps mp3 | —（Ryo Lion） | — | — | — | —s | no candidate |
| `DLC_data/gulp_down_water1.mp3` | 167 kbps mp3 | —（(c)Taira Komori） | — | — | — | —s | no candidate |

### 4.5 统计

| 判定 | 数量 |
| :--- | ---: |
| ✅ 升级 | 3 |
| ➖ 持平 | 3 |
| ⛔ 降级 | 16 |
| ❔ 未找到 | 28 |
| **合计** | **50** |

### 4.6 可替换的曲目（码率占优者）

码率占优**不等于**可以替换：候选母版必须与零售文件是同一段录音。下表的「同一性」列给出 8 kHz 能量包络互相关峰值——**≥0.95 才是同一录音**，0.80–0.95 需人工试听，<0.80 判为非同一音源。

| 媒体库文件 | 现状 | 源码率 | 源站曲目 | OpenTracks 链接 | 同一性 | 结论 |
| :--- | :---: | :---: | :--- | :--- | :---: | :--- |
| `BGM/p_egg_loop.ogg` | 133 kbps | 161 kbps | 夢の中で呼ばれた | https://opentracks.com/bgm/detail/559 | 0.8613 | ⚠️ 需人工试听 |
| `BGM/train.mp3` | 128 kbps | 321 kbps | Train | https://opentracks.com/bgm/detail/11485 | 0.8581 | ⚠️ 需人工试听 |
| `BGM/yuuyake.mp3` | 128 kbps | 321 kbps | 夕焼けレモンティー | https://opentracks.com/bgm/detail/2305 | 0.9999 | ✅ 可替换 |


### 4.7 低码率曲目：母版确认结果

零售版中码率低于 200 kbps 的曲目共 13 个，是唯一可能从源站获益的一档。逐条回源并做互相关后：

| 零售文件 | 零售码率 | 母版检索结果 | 互相关峰值 | 结论 |
| :--- | :---: | :--- | :---: | :--- |
| `BGM/yuuyake.mp3` | 128 kbps | `#2305 夕焼けレモンティー`（321 kbps） | **0.9999** | ✅ **可替换**，已产出验证件 |
| `BGM/train.mp3` | 128 kbps | `#11485 Train`（321 kbps）；另 4 个同名候选 | 0.8581 | ⚠️ 偏弱，需人工试听 |
| `BGM/p_egg_loop.ogg` | 133 kbps | `#559 夢の中で呼ばれた`（161 kbps） | 0.8613 | ⚠️ 偏弱，需人工试听 |
| `BGM/se_akimusi2.ogg` | 151 kbps | `#2382 うきうきピアノジングル01`（352 kbps） | 0.8093 | ⚠️ 偏弱，需人工试听 |
| `BGM/centennial_loop.ogg` | 153 kbps | 2 个候选长度均短于零售文件 | 0.0000 | ⛔ 非同一音源 |
| `BGM/nonbiri.mp3` | 160 kbps | `#6476 のんびりした日々`（160 kbps） | **1.0000** | ➖ 同一录音确认，但**码率持平、无增益** |
| `BGM/soul_engage_loop.ogg` | 161 kbps | 源站无 `SOUL ENGAGE` 条目（`Soul` 全 28 条、`engage` 全 0 条） | — | ⛔ 母版不存在于该站 |
| `BGM/sparrows.mp3` | 162 kbps | 効果音ラボ `sparrow-morning1.mp3` | 0.6136 | ⛔ 非同一音源 |
| `BGM/waste_loop.ogg` | 162 kbps | 源站无 `荒野を旅する風` 条目（`荒野` 全 30 条无匹配） | — | ⛔ 母版不存在于该站 |
| `BGM/nemesis_loop.ogg` | 162 kbps | 源站无 `Nemesis` 条目 | — | ⛔ 母版不存在于该站 |
| `BGM/wave1.mp3` | 192 kbps | 4 个时长相近候选 | ≤0.784 | ⛔ 非同一音源 |
| `BGM/takibi.mp3` | 192 kbps | 标签 `TITLE=171210_011` / `ARTIST=My Recording`（个人录音） | — | ⛔ 无公开母版 |
| `BGM/nc51105.ogg` | 194 kbps | 源站无该编号 | — | ⛔ 无公开母版 |

> **结论：13 个低码率曲目中，只有 `BGM/yuuyake.mp3` 一例可以确认替换。**
> 其余曲目要么母版不存在于该站，要么经互相关判定并非同一段录音，**建议一律保留零售版**。

---

## 5. 原检索指南勘误

原指南（`docs/调研/《妹!せいかつ～ファンタジー～》高音质音源检索与替换指南.md`）的检索词表与媒体库存在以下出入。**勘误依据是零售文件自带的 `TITLE`/`ARTIST` 标签 + 源站实测**。

### 5.1 技术前提勘误

| 原指南表述 | 实际情况 | 修正 |
| :--- | :--- | :--- |
| 「RPG Maker MV 官方素材库 / 引擎安装目录 `NewData/audio/`」 | 本作是 WOLF 引擎工程，无此目录 | 删除该来源 |
| 「`.rpgmvo` 加密文件需用 RPGMakerMVDecrypter 解密」 | 媒体已解包为明文 `.ogg`/`.mp3`/`.wav` | 无需解密，直接替换解包目录中的文件 |
| 「音频位于 `www/audio/` 下分 `bgm/bgs/me/se`」 | 实际为 `work/retail_dat/{BGM,SE,DLC_data,SystemFile}` | 按实际路径替换 |
| 「`.ogg` 头部写入 `LOOPSTART`/`LOOPLENGTH` 用于循环」 | 该结论**成立**，但需注意 **Godot 运行时目前不读这两个标签**（见 §7.3） | 替换时必须原样保留标签 |
| 「`ffmpeg -c:a libvorbis -q:a 10`」 | 可用；本仓库 `tools/pack_media.py` 另有 `-q:a 5` 的 WAV→Ogg 分支 | 保留质量预算需与体积目标一并权衡 |

### 5.2 曲目检索词勘误

| 原指南条目 | 原检索词 | 修正后的正确检索词 | 依据 |
| :--- | :--- | :--- | :--- |
| 02 Acoustic High Collar | `Acoustic High Collar` | **`High Collar ACG ver`** | 零售 `TITLE=dova_High Collar ACG ver` |
| 05 Migratory Birds | `Migratory Birds` | **`Migratory Birds mix`** | 零售 `TITLE=dova_Migratory Birds mix` |
| 11 Madoromu2 | `Madoromu2` | **`まどろむ時間２`** | 零售 `TITLE=まどろむ時間２`；时长 172 s 亦吻合 |
| 23 Waste | `Waste` | **`荒野を旅する風`** | 零售 `TITLE=荒野を旅する風`（ARTIST=shimoya） |
| 25 The End of Destruction | `The End of Destruction` | **`The End of Destruction Loop`** | 零售 `TITLE=dova_The End of Destruction Loop_master` |
| 28 centennial | `百年王国` | `百年王国`（ARTIST=shimoya） ✅ | 原条目正确 |
| 30 Soul Engage | `Soul Engage` | `SOUL ENGAGE`（ARTIST=shimoya） ✅ | 原条目基本正确 |
| 09 Nonbiri | `Nonbiri` | **`のんびりした日々`**（`#6476`） | 零售文件名 `nonbiri` 非原曲名；互相关 **1.0000** 确认为同一录音，但源码率同为 160 kbps，**无增益** |
| DLC bgm08 | `星空の下で #2` | **`緑色の木漏れ日`**（ARTIST=Yoshinori Tanaka） | 零售 `DLC_data/bgm08.ogg` 的 `TITLE` 实为 `緑色の木漏れ日`；`星空の下で #2` 在源站无此条目 |

### 5.3 幻觉曲目（媒体库中不存在对应文件）

以下条目在原指南中列出，但**媒体库 183 个音频文件中没有任何对应资产**，且源站亦无对应条目：

`Summer`、`Yuuyake -Night Version-`、`InstaVista`、`You Wind`、`Our Construction`、`Sirorenga`、`The Grand Adventure's Theme`、`DLC bgm1`（→ 实为 `DLC_data/bgm01.ogg`，**存在**）、`DLC bgm09`（→ 实为 `DLC_data/bgm09.mp3`，**存在**）、`DLC bgm05`（媒体库无 `bgm05`）、`星空の下で #2`、`サマー・ストロベリー・ポップ`（**实为 `DLC_data/bgm09.mp3` 的 `TITLE`，源站条目名不同**）。

### 5.4 检索词有效性实测

原指南 46 条检索词逐条回源实测（`work/out/audiosrc/doc_claims.json`）：**命中 34 / 未命中 12**。未命中项为：

`Acoustic High Collar`、`Wind Letter`、`Madoromu2`、`Sunshine!`（尾部 `!` 干扰检索）、`Syuuenn`、`The End of Destruction`、`Across`、`Our Construction`、`百年王国`（该词在源站已下架，改用 `百年` 可命中 `百年の謀略`）、`Soul Engage`、`Nemesis`、`Sirorenga`、`InstaVista`、`大冒険のテーマ`、`星空の下で #2`、`サマー・ストロベリー・ポップ`。

> 检索坑位记录：源站检索**是 AND 语义且对词尾标点敏感**，且 `data-next` 属性恒指回第 1 页（真实翻页链接在 `paging-forward` 的 `?page=N`）。

---

## 6. 替换作业流程

### 6.1 必须满足的三条硬约束

1. **时长一致**：新文件必须覆盖与零售文件完全相同的音乐跨度（裁切母版），否则 BGM 循环周期改变。
2. **采样率一致**：新文件必须重采样到零售文件的采样率，否则 `LOOPSTART`/`LOOPLENGTH` 的采样点数值失效。
3. **循环标签搬运**：新 `.ogg` 必须原样写入零售文件的 `LOOPSTART`/`LOOPLENGTH`。

### 6.2 标准作业步骤

```bash
# 1) 按同一跨度裁切母版并重编码（示例：train.mp3，母版 182 s → 零售 78.47 s）
ffmpeg -y -v error -ss 0.016 -i src/train.mp3 -t 78.471 \
       -ar 44100 -c:a libvorbis -q:a 10 \
       -metadata LOOPSTART=... -metadata LOOPLENGTH=... train.ogg

# 2) 用互相关复核新文件与零售文件确为同一段录音
python work/out/audiosrc/match_audio.py <retail> <new>

# 3) 覆盖解包媒体库并重打媒体包
#    pack_media.py 直接从 --root 遍历，不读清单；清单仅供姊妹仓的 media-pack.exe 使用
python tools/pack_media.py --root work/retail_dat --out work/all/media.pak

# 4) 交付门禁
dotnet build godot/Lwsmf.csproj -c Release
bash tools/godot_check.sh
LWsmf_RENDER=1 bash tools/godot_check.sh
```

### 6.3 换容器是否安全

若希望把 `.mp3` 换成 `.ogg`（Vorbis 在同等听感下更省体积，且便于携带循环标签），**运行时可解析**：`AssetDB.gd:299-304` 在精确路径未命中时，会依次尝试 `basename + .ogg / .mp3 / .wav`；`AUDIO_EXTENSIONS` 顺序为 `[".ogg", ".mp3", ".wav"]`。因此 `BGM/train.mp3` 的引用会自动落到 `BGM/train.ogg`。但**媒体包清单必须同步重打**，否则包内路径与引用不一致。

### 6.4 已验证的替换产物

`BGM/yuuyake.mp3` 是唯一完成"下载 → 同一性验证 → 裁切重编码 → 回验"全链路的曲目。产物位于
`work/out/audiosrc/upgrade/yuuyake.ogg`（`/work/` 已被 `.gitignore` 排除）：

| 项 | 零售版 | 替换件 |
| :--- | :--- | :--- |
| 文件 | `BGM/yuuyake.mp3` | `upgrade/yuuyake.ogg` |
| 容器 / 码率 | MP3 128 kbps | **Vorbis 386 kbps**（`libvorbis -q:a 10`） |
| 采样率 | 44 100 Hz | 44 100 Hz（一致） |
| 时长 | 197.9008 s | **197.9010 s**（一致） |
| 与零售版互相关 | — | **0.9999 @ offset 0.000**（同一录音） |

执行的转码命令：

```bash
ffmpeg -y -v error -ss 0.000 -i 002305_Yuuyake.bin -t 197.901 \
       -ar 44100 -c:a libvorbis -q:a 10 upgrade/yuuyake.ogg
```

> 该曲零售版为 `.mp3`，**不带循环标签**（`.mp3` 从不携带 `LOOPSTART`/`LOOPLENGTH`），因此无需搬运标签。
> 若后续改用 `.ogg` 容器替换 `.mp3`，运行时可正常解析（见 §6.3），但媒体包清单必须同步重打。

---

## 7. 附带发现

### 7.1 `DLC_data/bgm09.mp3` 被当作音效播放

`opcode 140` 指令中，`DLC_data/bgm09.mp3` 的通道位为 `2`（**SE**）而非 `0`（BGM），其余 7 个 DLC BGM 均为 BGM 通道。属原作行为，重制版照搬。

### 7.2 `SystemFile/` 下 13 个 `[SE]*.ogg` 是死资产

游戏数据中 `SystemFile/` 音频引用数为 **0**，且与 `work/all/SystemFile/` 下的同名文件 sha256 完全一致。

### 7.3 Godot 运行时未读取 Vorbis 循环标签

`AudioManager.gd:341-357` 的 `_apply_loop()` 只使用**游戏数据里的 `loop_point`（60 Hz 帧）**，而零售数据中该值**恒为 0**；它**不读取** `.ogg` 的 `LOOPSTART`/`LOOPLENGTH`。Rust Oracle 侧（`crates/wolf-audio`）则按标签循环。因此当前 Godot 端口的 BGM 是**整曲从 0 循环**，与原作"前奏→循环体"的听感存在偏差。

**这是先于本次音源工作存在的保真度缺口**，但它直接影响音源替换方案：替换后的文件必须携带正确的标签，将来 Godot 侧补上标签读取后才能还原原作循环。**本次替换必须原样保留标签。**

### 7.4 换用 OGG 容器的额外收益

`docs/reviews/review_614c6cb_and_perf_size_plan_2026_11_23.md:55` 已明确警告：**不可把原本有损的 `.ogg`/`.mp3` 转 FLAC**（体积暴涨 3–5 倍且零增益），Godot 4 也不原生支持 FLAC 解码。因此升级路径限定在 **`libvorbis -q:a 10`（约 500 kbps）或 `libmp3lame -b:a 320k`** 之内。

---

## 8. 工具与产物

全部脚本与产物位于 `work/out/audiosrc/`（`/work/` 已被 `.gitignore` 排除，**不会进入版本库**）：

| 文件 | 作用 |
| :--- | :--- |
| `ot.py` | OpenTracks 客户端：检索（带 CSRF / 分页）、详情、下载选项、下载 |
| `map_full.py` | 以零售文件内嵌标签为主键反查源站条目 |
| `map_sources.py` | 早期版本（手工检索词），保留作为对照 |
| `resolve_pass2.py` | 第二轮：用**服务端时长分桶**收窄候选，针对零售码率低于 200 kbps 的 12 个曲目 |
| `test_candidates.py` | 下载候选并逐条互相关判定同一性 |
| `final_table.py` | 生成逐曲判定表（升级 / 持平 / 降级 / 未找到） |
| `merge_corr.py` | 把互相关结论合并回判定表，重算判定 |
| `check_doc_claims.py` | 原指南 46 条检索词逐条回源实测 |
| `download_sources.py` | 批量下载母版并逐个校验（可解码 / 长度） |
| `match_audio.py` | 8 kHz 能量包络归一化互相关，判定"是否同一段录音"并求对齐偏移 |
| `prepare_upgrade.py` | 按零售跨度裁切母版、重编码、搬运循环标签 |
| `make_report.py` | 由实测数据生成本报告的表格章节 |
| `downloads/` | 已下载的母版（原始 MP3）与候选件 |
| `upgrade/` | 已验证的替换产物（如 `yuuyake.ogg`） |
| `final_table.json` / `doc_claims.json` / `download_results.json` / `candidate_corr.json` | 实测数据台账 |

---

## 9. 未解决项

| 项 | 状态 | 后续建议 |
| :--- | :--- | :--- |
| `OtoLogic` 管辖的 `BGM/Clock-Second_Hand03-1(Loop-1).mp3`（61 kbps）、`BGM/oyasumi no kiss wo1.mp3`（128 kbps） | 站点 403，母版不可得 | 从日本网络环境访问 `otologic.jp` 取 WAV 母版；该站以未压缩 WAV 为主，是这两首的**唯一**高码率来源 |
| `BGM/sparrows.mp3`（162 kbps，ARTIST=(c)Taira Komori） | 効果音ラボ可达，但尚未锁定确切同名条目（候选 `sparrow-morning1.mp3` 互相关仅 0.61，判为非同一音源） | 按音效库目录逐条比对波形后再替换 |
| `BGM/takibi.mp3`（192 kbps，`TITLE=171210_011`）、`BGM/nc51105.ogg`（194 kbps） | 标签显示为**个人录音 / RPG Maker 素材**，无公开源站 | 视为"无更高质量来源"，保留零售版 |
| `BGM/se_akimusi2.ogg`（151 kbps） | 源站候选 `#2382 うきうきピアノジングル01`（352 kbps）互相关仅 **0.8093**，未达同一录音阈值 | 该文件疑似 RPG Maker 自带素材；可人工试听后再决定 |
| `BGM/train.mp3`（128 kbps） | 源站候选 `#11485 Train`（321 kbps）互相关 **0.8581**；另 4 个同名候选均更低 | 人工试听确认后方可替换 |
| `BGM/p_egg_loop.ogg`（133 kbps） | 源站候选 `#559 夢の中で呼ばれた`（161 kbps）互相关 **0.8613** | 增益有限（133→161 kbps），人工试听后再决定 |
| 源站下架条目（`The End of Destruction Loop`、`The End of Destruction`） | 源站已查无此条目 | 尝试作者主页或存档站；否则保留零售版 |
| 未被判定的曲目 | 见 §4 对照表 `NO-SOURCE` 行 | 多数为自录音频或已下架条目，建议保留零售版 |
| Godot 侧 Vorbis 循环标签读取（§7.3） | 本次未修改 | 属独立保真度课题，需另开条目 |

---

## 10. 授权与合规提示

* OpenTracks（原 DOVA-SYNDROME）素材**免费但需遵守其「音源利用ライセンス」与制作者个别条件**，商用须署名；替换进本工程前应补齐署名列表。
* 効果音ラボ、OtoLogic 同理，各自条款独立。
* 本仓库 `.gitignore` 已排除 `/work/`，**所有母版与新编码文件均留在本地，不会随提交进入版本库**（符合 `AGENTS.md` §2「禁止提交未授权商业资产」）。
* `docs/调研/` 亦未被 git 跟踪，原检索指南为本地文件；本报告为**权威替代版本**。

