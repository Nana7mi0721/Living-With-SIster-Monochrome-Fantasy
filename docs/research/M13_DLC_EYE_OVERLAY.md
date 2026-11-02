# M13 bug 线索 — 读档多出 dlc_imouto01_eye.png 图层（task-20 诊断）

- 任务：共享 task-20；诊断「用 wolf-cli --load-save 回放零售存档时，1024x576 帧上多出 DLC_data/dlc_imouto01_eye.png（两只眼睛）浮层」。
- 实测 HEAD：`41f9216fb8b49d9a29c50b4ebbc7a5f4aec6cc9f`（写文档时；如有变动以文档 git 纪元重测为准）。
- 数据根：`work/all`（完整 .wolf 解包，含 DLC_data/、BasicData/、Save/）。
- 零售档：`work/retail_backup/Save/SaveData01..08.sav` + `save1..8.jpg` 缩略图。
- 结论速览：**眼睛层不在存档里，与读档/恢复、z 序都无关。它是 run_condition==2 常时事件 #506（妹トイレ前まばたき）在加载后由调度器启动、画出 pic -6 造成的；而零售存档自身的活图片表(part5)里并没有这一层。** 因此「加载即多画该层」是我们引擎相对零售存档数据的一个真实偏移，但根因边界（为何零售在该状态没在跑 506）尚未闭合，**本次不改生产源码**，交 Lead 决策。

---

## 1. 现象与复现

复现（默认 auto-events=on）：
```bash
./target/debug/wolf-cli.exe --root work/all capture \
  --load-save work/retail_backup/Save/SaveData01.sav --frames 60 \
  --out work/out/m13_eye/repro_sd01.png
```
日志：`loaded 66 picture(s)` / `pictures 67 live -> 19 textured quad(s), 34 text picture(s)`；活图片表含
`pic -6  File at (0,0) alpha 1.00 scale 100 window None file "DLC_data/dlc_imouto01_eye.png"`；
常驻协程 `coro 4  event 506  pc 6  wait Frames(1)`。

对照（关调度器，A/B）：
```bash
./target/debug/wolf-cli.exe --root work/all capture \
  --load-save work/retail_backup/Save/SaveData01.sav --frames 60 --auto-events off \
  --out work/out/m13_eye/sd01_auto_off.png
```
→ `66 live`，无 pic -6，无 506 协程。

8 档矩阵（`--frames 5`，on vs off）：

| 存档 | auto-on 有 pic -6 | auto-off 有 pic -6 | live(on) | live(off) |
|---|---|---|---|---|
| SaveData01 | 是 | 否 | 67 | 66 |
| SaveData02 | 是 | 否 | 59 | 58 |
| SaveData03 | 是 | 否 | 63 | 62 |
| SaveData04 | 是 | 否 | 65 | 64 |
| SaveData05 | 是 | 否 | 64 | 63 |
| SaveData06 | 是 | 否 | 63 | 62 |
| SaveData07 | 是 | 否 | 63 | 62 |
| SaveData08 | 是 | 否 | 86 | 84 |

8/8 一致：**眼睛层只在 auto-events=on 时出现，与存档加载/恢复本身无关。**

像素对比（1024x576，PIL）：retail save1 vs auto-on mean_abs=4.94 / frac>24=2.67%；
retail vs auto-off mean_abs=3.86 / 2.06%；auto-off vs auto-on 1.29 / 0.74%。
即 **auto-off 明显更贴近零售缩略图**（零售缩略图无眼睛）。三图拼图见 `work/out/m13_eye/compare_3way.png`（左=零售 save1.jpg 无眼；中=auto-on 角色上方偏左两只黑眼；右=auto-off 无眼≈零售）。

> 说明：零售 `saveN.jpg` 是「存档时点的缩略图」，只能证明存档那一刻屏幕长什么样，不能证明「读档之后实时帧」该长什么样。本诊断不把它当唯一真理。

---

## 2. 眼睛层不在存档里（SavePart5 无 pic -6）

解析 8 档 `part5`（活图片表）的负号图片号（临时探针 `crates/wolf-save/tests/tmp_dump_eye.rs`，已删）：

- SD1..SD7：`[-10, -7, -1]`（-10=cg/bg_14_sunset_a、-7=cg/bg_01_home_d/e/f、-1=blackscreen opacity0）
- SD8：`[-10, -7, -5, -4, -1]`（-5/-4 是 hscene 背景）
- **8/8 都不含 pic -6 / dlc_imouto01_eye。**

即：加载路径 `savegame.rs:166-177`（clear screen 后仅按 `save.part5.events` 逐条 `picture_from_record` 插入）**不会**产生 -6；-6 是随后由常时事件补画的。

眼睛 PNG：`work/all/DLC_data/dlc_imouto01_eye.png` = 1536x576 RGBA，**99.37% 像素 alpha=0**，只有约 0.6% 是眼睛（分 3 个横排精灵格，512x576/格；三格 alpha bbox 都在 x≈173-310,y≈130-186，彼此略有差异=眨眼三态）。三格上都画着眼睛，所以「眨眼」只是眼形微动，**只要 506 在跑，眼睛就基本一直可见**。

---

## 3. 根因：run_condition==2 常时事件 #506（妹トイレ前まばたき）

事件 506（`work/wolftl/dump/common/506_妹トイレ前まばたき.json`，29 条命令）头条件：
```
@0 111 VariableCondition [1, 2000610, 1, 2]   # var610 == 1
@1 401 ChoiceCase 1                            # 成立 -> 眨眼体
@3 150 Picture ... number=-6 file=dlc_imouto01_eye.png  opacity=255 (立即满强度, fade=0)
@4 179 LoopTimes self0 ; @5 Wait1 ; @6 if var610==0 -> 退出 ; @13 SetLabel A ;
@14/16/18/20 150 Picture 同一 pic -6 依次 pattern 2/3/2/1（眨眼）；...
```
它由 `crates/wolf-game/src/auto_events.rs` 的 run==2 调度器跟踪。**加载存档后调度器一同步（Game::frame 首步）就因 var610>=1 启动 506，@3 立刻以 opacity=255 画出 -6** —— 这就是多出来的眼睛层。

var610 = 通常変数 magic **2000610**。它的写者只有事件 505（■妹トイレH前）：idx2 置 0、idx82 置 1（上目瞬き）、idx85 置 0；读者只有 505/506。

### 关键原始数据：8 档 part3 的 var610 全 = 1

临时探针读 `part3.row_words[610]`：**8/8 都等于 1**（var611=0、var612=0、var619=0，邻域 600..609、612..620 全 0，孤立为 1）。
映射可信度：`docs/SAVE_SPEC.md` §6.1（line 392/396/406）已验证 `row_counts=[800,4,7,4,1,1,1,6,1,10]`，row 0 的 800 词与通常変数 2000000..2000799 一一对应（9/9、跨 SaveData01/08/System.sav 用所持金 2000007 交叉验证）。故 `row_words[610]` 就是通常変数 2000610，值确为 1。

---

## 4. 决定性发现：零售存档「在跑 216/217、却没跑 506」

零售存档 part5 里**同时存在 HUD 文本图片**（34 张 text picture，来自 run==2 事件 216/217 的 HUD/时钟重绘），**却唯独没有 -6**。
216/217 一般由 run==2 调度器驱动（`ENGINE_ALWAYS` 列表 `game.rs:84-95` 只有 48/261/269/270/271/224/225/213/214/41，**不含 216/217**），说明「产生这批存档的那个运行里，216/217 这类常时事件是跑过的，其输出也进了 part5」。

**但 506 的 -6 没进 part5。** 由于 506 只要启动就立刻以 opacity=255 画 -6 且循环期间不清除，如果它当时在跑，-6 必然出现在 part5。既然没有，只能推断：**产生这些存档的运行里，506 并未在画 -6（尽管 part3 记录 var610==1）。**

而我们引擎加载这些存档时会无条件启动 506。二者不符。

### 复现性证据：load → 1 帧 → save，part5 被多写入 -6、且不再字节等同零售

临时探针 `crates/wolf-game/tests/tmp_resave_eye.rs`（已删）：
```
Game::new(work/all) ; load_save(SaveData01) ; g.frame() 一帧 ; save_to(work/out/m13_eye/resaved_sd01.sav)
→ resaved part5 negatives = [-10, -7, -6, -1] ; any -6 = true
→ 字节数 366602（零售原始 366146），byte identical to retail = false
```
即：只要读档后跑一帧，我们就把零售存档原本没有的 -6 层写进来；再存档即偏离零售 ground truth（part3 大小也随之 +N）。

---

## 5. 与 §11.4 记录的矛盾（帧 1 可见性变了）

`docs/ENGINE_EVENTS.md` §11.4（line 962-973，M3 引入 commit `02d2416`）当时实测：
「506 会让活图片 66→67（新增 -6 层），**但第 1 帧画面无可见差异**，说明要么被场景遮住、要么与存档已有眼睛状态相同；后续多帧留给 M3 时间系统验证。」

当前 HEAD 复测（SaveData01，`--frames` 取 1/2/3/4/5/6/8/10/20/60）：
- 每个帧数都含 pic -6，且 **f1..f60 的 PNG 全部 md5 相同 = `16b79c4ea7e4e24cd73301362350a452`**，即眼睛**从第 1 帧就可见且不随帧变化（不眨眼）**。
- `--frames 1` 下 on vs off 差异已包含眼睛区：热点格 (x,y,mean)≈(544,128,135)、(384,128,91)、(384,160,84)…，bbox diff>32 覆盖 x53..916 / y12..184；而 §11.4 当时说「世界/立绘/面板区域全部 0.000」。

→ 结论：**§11.4 的「第 1 帧无可见差异」在现行 HEAD 不再成立**——介于 M3(`02d2416`) 与 HEAD 之间的渲染提交（相关：`22148d8` M9、`0e1288b` M12 改过 atlas.rs/picture.rs）让 -6 层在帧 1 就合成出来，而且我们的 blink 循环没有表现出可见动画（三格眼睛差异过小 + 可能 pattern 更新/循环推进未生效）。这两点是「把原本良性的一层变成显眼浮层」的次生因素。

> 注：事件 506 的眨眼三格视觉差异很小（cell0/1/2 互相 meanabs 3~6、frac>16 仅 1.3~2.4%，且都在同一小 bbox），所以即使动画正常，眼睛也基本一直可见——「眨眼」不是让它消失的机制。

---

## 6. 分类判定

按任务给出的三类：

- **A（我们渲染顺序错/z 序错）**：否。auto-off 与零售缩略图一致（无眼）；on 才多 -6；且 -6 是独立 drawn，排序不是问题。
- **B（加载把 hidden/stale 当可见）**：**不适用于 -6**——-6 不在 part5（不是加载插进来的）。此条对 -10/-7/-1 亦无证据（三层零售缩略图里也可见/本就该在）。
- **C（零售预览时点不含该层，非 bug）**：方向上接近「缩略图不等于读档后实时帧」，但**不能简单归为非 bug**：因为零售存档 part5 有 HUD 文本却无 -6，说明该存档产生时 506 实际没在画 -6；而我们读档后把它画了出来并会写回存档（§4），这是相对零售数据的一处真实偏移。

**综合判定：不是读档/恢复 bug，不是 z 序 bug；是 run_condition==2 常时事件 #506 在加载后被我们的调度器启动、画出 -6 造成的。** 该层不在零售存档活图片表里；我们「load→跑一帧→save」会把 -6 写回、偏离零售字节。根因边界（零售为何在该状态没跑 506）尚未闭合：
  1. 506 可能本应随地图/场景作用域（妹トイレ）启动，而非全局常时——我们的调度器按「全局条件成立即启动」处理，可能过宽；
  2. 或这批存档来自「尚无 DLC / 506 不存在」的旧运行，而现在我们用含 DLC 的 CommonEvent.dat 加载；
  3. 或 §11.4 的帧 1 可见性回归是独立渲染问题（见 §5）。

---

## 7. 结论与建议（未改生产源码）

- 交付性质：**诊断文档**。按任务约定「只有确认 bug 且边界清楚才改 src + 回归测试，否则只出诊断文档」，本次**未修改任何生产源码、未新增回归测试**，因为修复边界（是否/怎样抑制 506、是否属于旧档 artifact、是否属帧 1 合成回归）不清晰，贸然改全局 run==2 调度器或读档路径风险高。
- 给 Lead 的建议下一步（择一）：
  1. **对照真实零售运行时**：若能在正版引擎里加载 SaveData01 后截一张实时帧，即可判定「零售读档后是否也有眼睛」。这是唯一能给定论的 ground truth。（无零售运行时则退而求其次）
  2. **确认 506 的原始启动语义**：查 WOLF 编辑器里事件 506 的「実行条件/起動条件」是否带地图作用域，以及零售 run==2 常时事件是否在「读档/开机初始同步」时对已成立条件启动（我们 §11.2 的 [216,217,337,506] 其实是**我们引擎**的行为表，不是零售引擎实测）。
  3. 若判定应复刻零售（读档后不应有眼）：最小改动方向应落在「run==2 调度器对 map/场景作用域事件的启动条件」或「存档快照口径」，**不是**特判 506、也不是关掉整个调度器。
- 若判定为「旧档 artifact / 忠实复刻」，则无需改代码，最多在文档记录该现象为已知原版数据状态。

---

## 8. 证据文件（work/out/m13_eye/，诊断用，可留存作证据）

- `repro_sd01.png`（auto-on frames60，含眼睛）
- `sd01_auto_off.png`（auto-off frames60，无眼睛）
- `compare_3way.png`（左 retail / 中 auto-on / 右 auto-off）
- `f1.png … f60.png`（各帧，md5 全同 16b79c4e…）
- `off1.png`（auto-off frames1）、`sd0N_on.png / sd0N_off.png`（8 档矩阵）
- `eye_cells.png`（眼睛 PNG 三格拼图）
- `resaved_sd01.sav`（load→1帧→save，part5 含 -6、366602B，用于 §4 证据）

临时探针（**均已在收尾时删除，不提交**）：`crates/wolf-save/tests/tmp_dump_eye.rs`、`crates/wolf-game/tests/tmp_eye_meta.rs`、`crates/wolf-game/tests/tmp_resave_eye.rs`；`work/out/m13_eye/tmp_*.py` 为一次性分析脚本。

## 9. 复现命令汇总
```bash
# 有眼睛
./target/debug/wolf-cli.exe --root work/all capture --load-save work/retail_backup/Save/SaveData01.sav --frames 60 --out work/out/m13_eye/repro_sd01.png
# 无眼睛（更贴近零售缩略图）
./target/debug/wolf-cli.exe --root work/all capture --load-save work/retail_backup/Save/SaveData01.sav --frames 60 --auto-events off --out work/out/m13_eye/sd01_auto_off.png
# 8 档矩阵：对 1..8 各跑 on/off 两版并 grep 'pic -6'
```
