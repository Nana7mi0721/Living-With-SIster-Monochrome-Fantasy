# UI / 素材「布局↔渲染」语义一致性 + 素材存在性审计

> 状态：**审计完成（只读，未改 crates）**　日期：2026-11-02
> 数据源：当前 Steam 安装 Data/*.wolf（补丁后） + 项目数据根 work/all（补丁前）
> 产物：work/out/hudfix/

## 0. 结论速览

| 维度 | 结论 |
| --- | --- |
| 布局↔渲染语义 | **PASS**。op150 用到的 5 种图片类型、6 种锚点、9 类 sysgraphic 原语本移植全部支持；唯一语义缺陷（窗口纵向缩放）已修。 |
| 素材存在性 | 140 个 distinct 的 Picture 引用在补丁版与旧版都不存在，属原版未随包/死引用（含 sc1.jpg 实为 ProFeature 截图输出路径，非素材）。非本移植缺陷。 |
| 数据版本 | **⚠ 关键：work/all 与当前安装 Data/*.wolf 不是同一版**（CE216/CE506/MapData/Picture 均不同，见 §3）；经 §6 复核，当前安装是 Steam 校验后的基础版（DLC 素材大量透明占位、缺地图），**不能作基准**，完整版 work/all 才是正确数据根。 |
| 原版 bug | 零售 Game_ErrorLog.txt 实测 CE506 'Pattern number is too large'；补丁把 CE506 清成 1 条指令修掉。按原版 bug 除外不追。 |

一句话：**渲染语义没有新缺陷；项目数据根 work/all（完整版）是正确的，当前 Steam 安装的 Data/*.wolf 只是校验后的基础版（DLC 被透明占位、缺 Dungeon 地图），不能作为基准——保持完整版、并提示用户重跑 CN Patch v5 exe 恢复游戏资源（§6）。**

## 1. 布局↔渲染 语义一致性（对补丁版数据全量统计）

在补丁安装 Data/*.wolf（23 归档 / 4065 文件）上解析 BasicData.wolf 的 CommonEvent.dat（3,945,490 B，550 事件）与 MapData.wolf 8 张地图，统计所有 op150 绘制：

- 图片类型分布：file 6356 / fileString 55 / text 2009 / windowFile 292 / windowString 55 —— 五种全覆盖。
- 锚点分布：0:6259 / 1:2166 / 2:82 / 3:118 / 4:70 / 6:72（无 5/7）—— place() 七码全覆盖。
- sysgraphic 原语（补丁版实际出现）：<SQUARE> 213、<SQUARE><FRAME> 10、<SQUARE>FRAME 16（缺内层括号的变体）、<GRADX-*> 30、<GRADY-*> 9、<LINE> 7、<CIRCLE> 4、<TRI-LEFT/RIGHT/DOWN> 各 1。
  → crates/wolf-game/src/sysgraphic.rs::parse 对其逐一支持（含 <SQUARE>FRAME 变体与 <GRAD*-a-b> 端点），box_size 对 <LINE> 的零高度也特判。**无未知原语**。
- 缩放语义（本批修复点）：窗口类图片只横向缩放、高度取窗口框；file/text 等比。用 8 个零售存档 part5 全量验证——所有 scaleX≠scaleY 记录都是窗口类且纵向恒 100（6233=62×100 等），file 记录全部 scaleX==scaleY（15/75/150 等比）。

结论：除已修的窗口纵向缩放外，**渲染语义与数据布局一一对应，无新增不一致**。

## 2. 8 存档 HUD 像素对照（补丁前数据根 work/all 渲染）

对 SaveData01..08 --load-save … --frames 60 截帧，与零售 save1..8.jpg 比 HUD 顶栏(y<40) MAE：修复后 8/8 全部下降（如 sd01 22.70→21.32、sd02 20.36→18.77 …），行动条填充在 8 个不同 AP 比例(62/57/21/15/76/18/25/62)下均与零售逐列一致（白 15 行、宽边界一致）。图：work/out/hudfix/montage_hud_retail_vs_ours_8.png。

## 3. ⚠ 数据版本漂移（最重要发现）

用户昨晚推进到 day100 遇到素材 bug 弹窗 → 在 Steam 校验完整性并下载补丁。结果**当前安装的 Data/*.wolf（多数 2026-11-02 16:26 重写）与项目数据根 work/all 已不是同一版**：

| 项 | 当前安装 Data（补丁后） | work/all（项目现用，补丁前） |
| --- | --- | --- |
| BasicData/CommonEvent.dat | 3,945,490 B | 5,532,175 B |
| CE216 ステータスバー | 189 cmds / 10 pics，header=scene/header_bar.jpg(1,395 B) | 192 cmds / 13 pics，header=hscene/header_bar.jpg(10,437 B) |
| CE506 妹トイレ前まばたき | 1 cmd / 0 pics（清空） | 49 cmds / 5 pics（旧 bug 源） |
| MapData | 8 张，SampleMapA 211,847 B（无 Dungeon.mps） | 9 张，SampleMapA 797,933 B（含 Dungeon.mps） |
| Picture 归档 | 553 项 | 539 文件 |

- **CE506 差异正是用户遇到的原版 bug**：零售 Game_ErrorLog.txt 反复记录 'Error [Picture] Pattern number is too large! > ID-6 Pattern 3(Max2) … Process mark:[CommonEv 506 Live 16]'。旧版 CE506 用图片 -6（眼睛层）以越界 pattern 绘制 → 弹窗；**补丁版把 CE506 清成 1 条指令，bug 消失**。
- CE216 差异：补丁把 HUD 表头从 hscene/header_bar.jpg 换成 scene/header_bar.jpg 并重排（10 vs 13 pics；精力/性欲等由其它事件补画，最终画面等价——补丁版截图 patch_sd01.png 与零售/旧版 HUD 三者肉眼一致，见 work/out/hudfix/cmp_retail_workall_patch_hud.png）。
- 我把补丁版 .wolf 全量解包到 **work/all_patch**（4065 文件 / 550,621,646 B，0 失败），可直接作为新数据根做渲染/验收。

**含义**：项目自 M0 起所有内容一致验收、视觉基线、CE506 眼睛层修复，都是针对 work/all（补丁前）做的；用户当前实际在玩的是补丁后版本。两者 HUD 渲染等价，但 CE506/地图/若干素材不同。

## 4. 素材存在性（对补丁版数据）

对补丁版 CommonEvent.dat + MapData 的 op150 静态资源引用（1218 distinct）比对 23 归档的 4065 个条目：

- **MISSING 140 distinct**：139 个 picture/… + 1 个 sc1.jpg。
  - 典型：picture/skill01..08、skill*_report.png、chart01..18(_report).png、paja_body2.png、explosion.png、attack_effect01.png、quest_result(_2).png、bg_map3_effect.png、map_village01_top.png、battle_icon_end.png。
  - **这些在补丁版归档与旧 work/all 里都不存在**（Picture 归档实际是 skill_a_01.png/paja_body.png/quest_sd_00.png 等新命名）→ 属原版脚本指向未随包/已改名素材的引用（死引用或原版缺素材），不是本移植渲染缺陷。与既有 docs/ASSET_AUDIT.md（342 条 missing）同源。
  - sc1.jpg = 1000 ProFeature 的截图存档输出路径（ev435 save/save1.jpg、ev452 Data/SC1.jpg），不是要读取的素材 → 假阳性。
- 余下 1078 个引用全部命中归档 → 素材齐全。

## 5. 建议（已于 §6 修正：不要迁移数据根）

1. ~~换数据根到补丁版~~ → **撤销**。经 §6 复核，当前安装 Data/*.wolf 只是 Steam 校验后的「基础版且 DLC 素材大量为透明占位」，并非用户完整游戏；项目数据根 work/all（完整版）才是正确基准，保持不动。
2. **保留原版 bug 例外**：CE506 pattern 越界弹窗是原版 bug；若用户希望移植版复刻此 bug 就保留，若要修掉则单独把 CE506 眼睛层改成不越界即可，无需换整份数据。
3. 死引用素材（140）不当作移植缺陷。

## 6. 复核修正（2026-11-02 晚，重要）

在真正把数据根切到补丁版后做了逐文件复核，结论**反转**：当前安装 Data/*.wolf 不是用户完整游戏，不能作为基准。

证据（均用 crates/wolf-format 的 DxArchive v8 读取器直接读归档、readlen==size 校验通过，非解包 bug）：

| 项 | 当前安装 Data/*.wolf（Steam 校验后） | 完整版 work/all（原 work/all_prepatch） |
| --- | --- | --- |
| DLC_data.wolf | 72 MB / 635 项；**331/455 张 PNG 解码后 bbox=None（整张全透明占位）**，仅 124 张真图 | 155 MB / 633 项；0 张占位，全部真图 |
| cg.wolf | 32.7 MB；448/596 个文件与完整版逐字节不同 | 123 MB |
| MapData | 8 张，**缺 Dungeon.mps** | 9 张含 Dungeon.mps |
| BasicData/CommonEvent.dat | 3,945,490 B（比完整版少 ~7000 条 op，max_branch_depth 91→24） | 5,532,175 B |

- 安装根目录另有：`Cosplay DLC/`（仅 gallery 21 张 jpg + thumbs，24 MB，不含 DLC_data 立绘）、`Living With Sister Monochrome Fantasy CN Patch v5 with Cosplay DLC.exe`（1.31 GB，2024-12-21）、`dlc.dat`(64 B)。Data 下多数 .wolf 的 mtime 为 2026-11-02 16:26 = 今日 Steam 重新下载。
- 判断：**Steam「校验完整性并下载补丁」把 CN Patch v5 + Cosplay DLC 覆盖进 Data 的内容回滚成了基础版**（DLC 立绘被写成透明占位、少一张地图、事件被裁）。完整版内容仍在 `work/all_prepatch`（项目从 M0 起一直用它验收，863 tests 全绿）。
- 措施：已把 work/all 还原为完整版（`work/all`=完整 1.5 G，`work/all_base_steamverify`=当前安装的裸基础版 534 M 留档）；wolf-cli verify --root work/all 恢复 **PASS 70 / 0**。
- 给用户的行动建议：需要**重新运行安装目录里的 `CN Patch v5 with Cosplay DLC.exe`** 恢复游戏内 DLC/中文补丁资源，才能恢复你昨天在玩时的完整内容；否则在 Steam 里会看到 DLC 图空白。移植项目基准保持完整版不变。
### 6.1 结论（用户重装最新神乐补丁后，2026-11-02 晚）

用户按提示重跑了 Kagura 官网最新「妹相随」补丁+DLC 后，当前安装 `Data/*.wolf` 已与项目完整版 **逐项一致**，不再有透明占位/缺图/缺图：

| 项 | 重装补丁后的当前安装 | 项目 work/all（完整版） |
| --- | --- | --- |
| BasicData.wolf | 1,278,855 B；CommonEvent.dat 5,532,175 B | 同 |
| DLC_data.wolf | 154,450,763 B；633 项；0 张 <200B 占位（453 PNG 全真图） | 同 |
| cg.wolf | 123,375,407 B；596 项 | 同 |
| MapData | 9 张含 Dungeon.mps | 同 |
| Picture.wolf | 25,041,363 B；539 项 | 同 |
| .wolf mtime | 2024-12-21（补丁文件日期） | — |

→ **§3/§6 的「当前安装是基础版」只是 Steam 校验后的短暂中间态；重装最新补丁已恢复完整版，与 work/all 完全相同。数据根保持 work/all 不动即可（verify PASS 70/0）。** 不再需要任何数据根迁移。
