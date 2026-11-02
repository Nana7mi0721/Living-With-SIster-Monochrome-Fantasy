# M13 · P1-E 可自动化部分 机器审计报告

- 任务：shared task-19（M13 内容验收 · P1-E 可自动化部分）
- 审计主体：teammate `visual-audio-audit`
- 仓库：`E:\Study\github program\imoto life`，HEAD `41f9216fb8b49d9a29c50b4ebbc7a5f4aec6cc9f`
- 日期：2026-11-02
- 产出目录：`work/out/m13_p1e/`（对照图 + `metrics.json`）
- 约束遵守：未 commit；未改 `crates/*/src`；未动用户资料（`work/retail_backup/`、`游戏示例图片/` 只读）；只跑指定测试。
- 复现环境前缀：`export WOLF_GAME_DIR=work/retail_backup; export WOLF_DATA_ROOT=work/all`

> 本文件只覆盖**机器可证**结论；无法由机器判定的部分集中列在 §6「人工剩余清单」，每条都给出精确的查看对象与判定标准。

---

## 0. 结论速览（机器已证）

| 项 | 结论 | 机器证据 |
|---|---|---|
| 关键场景零售像素 diff | **8 张真实零售 1024×576 帧**与我们的同存档回放**逐张对照完成**；画面层（去掉 HUD 文字）SSIM 0.946–0.973、MAE 2.75–5.05，**图形/场景层近乎逐像素一致** | §2，`work/out/m13_p1e/metrics.json` |
| 残差归因 | 残差主要是**HUD 文字抗锯齿**（HUD 区 MAE 13.8–20.0 vs 场景区 2.75–5.05）+ 一处**额外图层**（见下）| §2.2、§2.3 |
| 疑似真差异 | 我们的存档加载会多画一层 `DLC_data/dlc_imouto01_eye.png`（z=-6），8 个存档全部出现，零售 8 张预览**全部没有** | §2.6 |
| 锚点 5/6 | 单元测试（真实素材）5/5 通过，锚点坐标算术正确；**但没有一张零售帧能单独隔离锚点 5/6** → 零售像素级确认转入人工 | §3 |
| 31 首 loop | 机器审计 31 首全部跑通；客观指标分出 A17/B1/C10/D3，28 首需人耳复听 | §5，`work/out/m11_audio_loop_manifest.md` |
| 视觉回归 harness | 5 个场景快照测试 5/5 通过（含 battles/h_overlay 等） | §4 |

---

## 1. 零售参考素材盘点（关键前提）

像素级 diff 的前提是**分辨率、渲染管线一致的零售原始帧**。核查后发现：

1. **真正可用的零售 1024×576 在机帧**：`work/retail_backup/Save/save1.jpg` … `save8.jpg`，均为 **1024×576 RGB**，是零售游戏自己的**存档缩略图（存盘瞬间的实机截图）**，与 `SaveData01.sav` … `SaveData08.sav` 一一对应。这是本次像素对照唯一合法的 1024×576 零售基准。
2. `游戏示例图片/RJ338582-*.jpg`：`RJ338582-1.jpg` 是 1024×576 的**宣传标题图（妹!せいかつ logo 横幅）**，不是游戏画面；其余 12 张均为 **800×450 商店截图（已降采样）**。**按任务要求，800×450 商店图不得当 1024×576 像素基准**，仅可用于“布局/构图”层面的辅助辨认（见 `work/out/m13_p1e/montage_store.png`）。
3. `work/out` 下 529 张 1024×576 PNG 是**我们引擎的截图**，不是零售帧，不能当基准。

8 张零售存档预览的内容一览（缩略拼图：`work/out/m13_p1e/montage_retail8.png`）：

| 存档 | 游戏内日期/时间 | 场景 |
|---|---|---|
| save1 (SaveData01) | 第90天 21:40 周六 | 卧室，妹妹立绘（睡衣） |
| save2 (SaveData02) | 第2天 18:30 周二 | 卧室（序盘，属性低） |
| save3 (SaveData03) | 第62天 22:00 周六 | 卧室，HUD 显示“加入中出部” |
| save4 (SaveData04) | 第72天 21:30 周一 | 卧室，妹妹在书桌 |
| save5 (SaveData05) | 第79天 18:30 周三 | 卧室，妹妹站立 |
| save6 (SaveData06) | 第5天 18:30 周四 | 卧室，妹妹站立 |
| save7 (SaveData07) | 第68天 19:30 周五 | 卧室，妹妹蹲姿 |
| save8 (SaveData08) | 第90天 0:42 周六 | **H 场景**（HUD 插入/水着/兴奋） |

覆盖范围：7 个不同日期/状态的“日常卧室 + 完整体力条 HUD”场景 + 1 个 **H 端到端 CG 场景**。

---

## 2. 关键场景零售像素 diff —— 方法与原始数据

### 2.1 方法

- 用存档回放拿到对应帧：
  ```
  ./target/debug/wolf-cli.exe --root work/all capture --load-save work/retail_backup/Save/SaveData0N.sav --frames 60 --out work/out/m13_p1e/ours_sd0N.png
  ```
  8 个存档全部 exit 0，输出均为 **1024×576**（与零售缩略图同尺寸，无需缩放）。
- 对齐：两者同为 1024×576，直接像素对齐（无需配准）。
- 指标：灰度 SSIM（7×7 盒窗）、PSNR、MAE，以及全分辨率/半分辨率（BILINEAR 到 512×288）两种口径；另按区域拆分（HUD 区 y∈[0,90)，场景区 y∈[90,576)）。
- 脚本可复跑：见 §8。

### 2.2 全帧指标（`work/out/m13_p1e/metrics.json`）

| 存档 | SSIM(全) | SSIM(半) | PSNR(dB) | MAE | >16 像素% | >48 像素% |
|---|---|---|---|---|---|---|
| 1 | 0.9427 | 0.9571 | 19.57 | 4.90 | 2.82 | 2.32 |
| 2 | 0.9487 | 0.9622 | 20.03 | 4.49 | 2.46 | 2.02 |
| 3 | 0.9286 | 0.9453 | 18.40 | 6.38 | 4.09 | 3.28 |
| 4 | 0.9277 | 0.9444 | 18.30 | 6.46 | 4.12 | 3.29 |
| 5 | 0.9338 | 0.9475 | 18.71 | 5.84 | 3.46 | 2.84 |
| 6 | 0.9309 | 0.9480 | 18.58 | 6.11 | 3.85 | 3.09 |
| 7 | 0.9310 | 0.9452 | 18.47 | 6.12 | 3.60 | 2.98 |
| 8 | 0.9245 | 0.9408 | 19.26 | 6.49 | 5.02 | 3.96 |

### 2.3 区域拆分（把“画面”与“HUD 文字”分开）

| 存档 | 场景区 MAE | HUD 区 MAE | 场景区 SSIM | 场景区 >48% |
|---|---|---|---|---|
| 1 | 2.93 | 15.49 | 0.9704 | 1.08 |
| 2 | 2.75 | 13.84 | 0.9733 | 0.94 |
| 3 | 4.20 | 18.18 | 0.9584 | 1.93 |
| 4 | 4.25 | 18.39 | 0.9574 | 1.93 |
| 5 | 3.54 | 18.31 | 0.9650 | 1.40 |
| 6 | 3.72 | 19.02 | 0.9632 | 1.62 |
| 7 | 3.55 | 19.96 | 0.9649 | 1.39 |
| 8 | 5.05 | 14.26 | 0.9461 | 3.05 |

**读法**：HUD 区（文字密集）MAE 常是场景区的 3–6 倍；把 HUD 文字切掉后，背景/立绘/UI 图形层 SSIM 稳定在 0.946–0.973、MAE 仅 2.75–5.05 灰阶，属于“几乎逐像素一致”的水平。

### 2.4 JPEG 噪声地板（解释 SSIM 未到 1 的程度）

零售 `saveN.jpg` 是 JPEG（约 0.18–0.21 bpp，约 q80–85）。把我们的 PNG 自身按不同质量重编码后再比自身，得到纯 JPEG 造成的 SSIM 损失上界：

| 重编码质量 | SSIM(原 PNG vs 自身 JPEG) |
|---|---|
| q92 | 0.9950 |
| q85 | 0.9887 |
| q75 | 0.9800 |

即：JPEG 最多只解释约 0.011（q85）的 SSIM 缺口；剩余缺口来自 **HUD 文字抗锯齿/字体栅格化差异** 与 **额外图层**（§2.6），而非场景图形本身。

### 2.5 逐存档对照图

- `work/out/m13_p1e/cmp_sd01.png` … `cmp_sd08.png`：三栏（上=零售 / 中=我们 / 下=×3 绝对差热图）。
- `work/out/m13_p1e/overview_8pairs.png`：8 组“左零售 / 右我们”总览（100% 同尺寸缩小）。
- 视觉核查结论：8 组均为**同一房间/同一立绘/同一 HUD 数值**；差热图热点集中在 HUD 文字边缘与“两只眼睛”额外图层。

### 2.6 一处疑似真差异 —— 额外图层 `dlc_imouto01_eye.png`

在 diff 热图上，1–7 号存档画面上部中间恒定出现“两只眼睛”，零售预览没有。定位到我们加载后的图层栈：

```
pic -6    File at (0, 0) alpha 1.00 scale 100 ... file "DLC_data/dlc_imouto01_eye.png"
```

- 素材：`work/all/DLC_data/dlc_imouto01_eye.png`（1536×576 RGBA，alpha bbox = (173,130,1331,187)，是一条横向“眼睛条”）。
- 出现范围：**8 个存档加载全部带这一层**（z=-6，alpha=1.0），零售 8 张预览**全部看不到**。
- 确定性：save1 在 frame=1/3/10/30/60 输出的“眼睛”位置与强度完全一致（眼睛区 MAE 恒定 26.71，身体/房间区 MAE 仅 1.01），说明它是**加载时即绘制**，不是动画或瞬态。
- 定性：这与“房间/立绘几乎逐像素一致（MAE≈1）”形成鲜明对比，是本轮唯一成规模的**画面内容差异**。它究竟是“DLC 眼睛特效”本应出现的正确表现，还是我们加载路径错误地恢复了该层（零售存盘瞬间没有它），机器无法判定 → 见 §6 人工项 H1。

---

## 3. 锚点 5 / 6 —— 机器可证部分

测试：`cargo test -p wolf-game --test m11_anchor_pixels` → **5 passed / 0 failed**（文件 `crates/wolf-game/tests/m11_anchor_pixels.rs`）。

机器已证（真实素材的实际矩形结果）：

| 项 | 锚点码 | 语义 | 素材 | 输入位置/缩放 | 计算矩形 [x,y,w,h] |
|---|---|---|---|---|---|
| 正例 | code5 | 顶部居中（减去 w/2, 0） | `Picture/stand_sub12.png` 718×700 | (512,100) | [153,100,718,700] |
| 正例 | code6 | 底部居中（w/2, h） | `Picture/…` 482×225 | (800,576) | [559,351,482,225] |
| 反例 | code6 + 负缩放 | 镜像/负 scale | CE245 `pic1220` 482×225 | (240,576) scale=-100 | [-1,351,482,225]（镜像） |

- 锚点算术（code5=顶居中、code6=底居中）在真实素材上验证正确；负缩放镜像也有真实帧佐证：`work/out/m9_negative_scale_hover.png`。
- **局限（转入人工项 H2/H3）**：现有 8 张零售帧里**没有一张能单独隔离锚点 5 或锚点 6 的定位场景**（例如 CE522 的锚点 5 场景，我们的 capture 落到的是相册列表，不是该立绘）。因此“锚点 5/6 的零售像素级正确性”无法用现有零售素材机器闭环。

---

## 4. 视觉回归 harness（M11 快照）

测试：`cargo test -p wolf-cli --test m11_visual_regression -- --nocapture` → **5 passed / 0 failed**。

观察到的各场景机读指标（用于回归基线）：

| 场景 | frames | peak | pics | quads | textpics | atlas | nonblack |
|---|---|---|---|---|---|---|---|
| title | 636 | 6 | 2 | 19082 | 0 | 18 | 538282 |
| map_idle | 1200 | 12 | 49 | 27661 | 33 | 29 | 544405 |
| day_command | 120 | 6 | 9 | 4 | 3 | 2 | 552254 |
| battle | 600 | 7 | 52 | 961 | 29 | 20 | 524412 |
| h_overlay (SaveData08 f60) | — | 7 | 86 | 257 | 17 | 42 | 527568 |

> 注意：脚本里 `battle` 的基线帧数是 **600**（M11 task-95 起），而旧文档 `docs/research/M11_VISUAL_REGRESSION.md` 里写的 1200 已过时——以测试文件 `crates/wolf-cli/tests/m11_visual_regression.rs` 为准。

---

## 5. 31 首 loop 音频机审（不替代人耳）

测试：`cargo test -p wolf-audio --test m11_audio_loop_audit --test m11_audio_loop_regression -- --nocapture` → 两个测试均 **1 passed / 0 failed**。

结果（客观指标仅能筛“可疑”，不能判“好听/自然”）：

- 共 **31** 首 loop 轨道；**28** 首被标记需人耳复听（D 档 3 首无客观告警可不听）。
- 分档：**A=17 / B=1 / C=10 / D=3**（与 `docs/research/M11_AUDIO_LOOP_REGRESSION.md` 钉住的分档表一致）。
- 分档定义：
  - A = 客观接缝缺陷（`seam_abs≥0.20` 或 `ratio≥8`）；
  - B = 元数据异常（OVERRUN / LEN_EQUALS_TRACK）；
  - C = 响度/直流跳变（`rms_jump≥0.10` 或 `dc_delta≥0.05`）；
  - D = 无客观告警。
- 机器清单（含每首 seam_abs/step/ratio/rms_jump/dc_delta/flags）：`work/out/m11_audio_loop_manifest.md`。
- A 档 17 首（必须重点复听）目录：`Acoustic_High_Collar`、`Die_Letzte_Revolution`、`InstaVista`、`Our_Construction`、`RAIN_&_Co.`、`Sunshine!`、`Tenseness`、`The_End_of_Destruction`、`autumncome`、`centennial_loop`、`destiny`、`nemesis_loop`、`soul_engage_loop`、`straightcontrast`、`waste_loop`、`windletter`、`DLC_data/bgm01`。
- 说明：客观指标只能定位“接缝能量跳变/响度跳变”，无法判断 loop 点是否**可听出咔哒/乐感断裂**，也无法判断循环长度是否符合原作听感。这些仍需人耳。

---

## 6. 无法机器闭环 → 人工剩余清单（精确、可执行）

> 以下每条都给出“看/听什么、在哪、合格标准”。这是 P1-E 真正剩下的工作。
> 人工操作包：`work/out/h25_review/index.html`（H4 八张 2× 对照、H2/H3 预期几何、H5 28 段接缝试听；说明见同目录 `README.md`）。验收结果记录到 [`M13_MANUAL_ACCEPTANCE.md`](M13_MANUAL_ACCEPTANCE.md)。

### H1. 额外图层 `dlc_imouto01_eye.png`（画面内容差异，最高优先）
- 现象：我们加载任意存档都会在 z=-6 多画一层“眼睛条”，零售 8 张预览都没有。
- 需要人判定：在**零售原版**里，这 8 个存档对应时刻画面上**是否应当出现这两只眼睛**（是否 DLC 眼睛特效的既有表现）。
- 具体查看对象：`work/out/m13_p1e/overview_8pairs.png`（左零售 / 右我们），或 `cmp_sd01.png` 下栏热图。零售对照 = `work/retail_backup/Save/save1.jpg`。
- 合格标准：若零售**应有** → 我们正确，无需改；若零售**不应有** → 我们的加载路径错误恢复了 `pic -6`，需修（属 P1-E 唯一已定位到的画面内容 bug 线索）。

### H2. 锚点 5 零售像素级确认
- 机器只证了坐标算术（[153,100,718,700] 等）与单元测试；**无隔离零售帧**。
- 需人：在零售原版里走到能触发锚点 5（顶部居中）的场景（如 CE522 相册/列表类立绘），肉眼核对立绘顶边贴合位置与我们 `work/out/m13_p1e/` 对应 capture 是否一致。
- 合格标准：立绘上边缘与画面坐标 (x 居中,y=100) 语义一致，无偏移/无裁切。

### H3. 锚点 6（含负缩放镜像）零售像素级确认
- 机器证据：负缩放镜像真实帧 `work/out/m9_negative_scale_hover.png` + 单元测试 5/5。
- 需人：零售原版确认底部居中锚点（w/2,h）与负缩放镜像方向正确。

### H4. HUD 文字字形差异（是否算可接受）
- 机器已量化：HUD 区 MAE 13.8–20，热点全在文字边缘（字体抗锯齿/JPEG）。
- 需人判定：这个抗锯齿差异是否影响“体验与原版一致”的验收（若要求逐字像素级，则需统一字体栅格策略；若只要求可读+布局一致，则通过）。
- 2026-11-02 复验修复：周六蓝色 / 周日红色此前因 savegame 调色板索引（0x0f 被误当 `\c[0]`）与 glyphon sRGB 线性化而丢失，现已修复；重出 8 张对照图后 HUD 区 MAE 12.8–20.2，8/8 行动力条白色纵向 15/15 行铺满。
- 对照：任意 `cmp_sd0N.png` 下栏热图。

### H5. 31 首 loop 人耳复听（28 首）
- 依据：`work/out/m11_audio_loop_manifest.md`（A 档 17 首优先）。
- 需人：逐首试听 loop 接缝是否有咔哒/爆音/乐感断裂；确认循环长度听感符合原作。
- 合格标准：接缝无“咔哒/爆音/明显乐句断裂”；循环衔接自然。

---

## 7. 机器可证 vs 只能人工（总表）

| 维度 | 机器可证 | 只能人工 |
|---|---|---|
| 关键场景画面一致性 | ✅ 8 张零售 1024×576 帧逐张 SSIM/PSNR/MAE；场景层 SSIM 0.946–0.973 | ✏ 该抗锯齿残差是否达验收 / 眼睛层是否应存在（H1/H4） |
| 锚点 5/6 | ✅ 单元测试 5/5 + 真实素材矩形 + 负缩放真实帧 | ✏ 零售像素级隔离场景核对（H2/H3，缺零售帧） |
| 31 首 loop | ✅ 客观分档 A17/B1/C10/D3，清单已出 | ✏ 人耳接缝/循环听感（H5） |
| 视觉回归 | ✅ 5 场景快照 5/5 通过 + 机读指标 | —（harness 已闭环） |

---

## 8. 复现命令

```bash
# 0) 环境（Windows bash）
export WOLF_GAME_DIR=work/retail_backup; export WOLF_DATA_ROOT=work/all
cd "E:/Study/github program/imoto life"

# 1) 锚点测试
cargo test -p wolf-game --test m11_anchor_pixels

# 2) 音频 loop 审计 + 回归（会写 work/out/m11_audio_loop_manifest.md）
cargo test -p wolf-audio --test m11_audio_loop_audit --test m11_audio_loop_regression -- --nocapture

# 3) 视觉回归快照
cargo test -p wolf-cli --test m11_visual_regression -- --nocapture

# 4) 关键场景对照：抓 8 个存档回放帧
for n in 1 2 3 4 5 6 7 8; do
  ./target/debug/wolf-cli.exe --root work/all capture \
    --load-save "work/retail_backup/Save/SaveData0$n.sav" --frames 60 \
    --out "work/out/m13_p1e/ours_sd0$n.png"
done

# 5) 计算 SSIM/PSNR/MAE 并出对照图（见 §9 脚本说明）
#    原始数值全部落盘在 work/out/m13_p1e/metrics.json
```

---

## 9. 证据文件索引（`work/out/m13_p1e/`）

| 文件 | 说明 |
|---|---|
| `metrics.json` | 8 存档全帧 + 分区 SSIM/PSNR/MAE/阈值像素占比、JPEG 噪声地板、retail bpp |
| `ours_sd01.png` … `ours_sd08.png` | 我们对应 8 个零售存档的回放帧（1024×576） |
| `cmp_sd01.png` … `cmp_sd08.png` | 三栏对照：零售 / 我们 / ×3 绝对差热图 |
| `overview_8pairs.png` | 8 组“左零售右我们”总览 |
| `montage_retail8.png` | 8 张零售存档预览缩略拼图（场景清单来源） |
| `montage_store.png` | 13 张商店图缩略拼图（仅辅助辨认，不作像素基准） |
| `ours_sd01_f60.log` 等 | 每次 capture 的图层/文本/统计日志（“pic -6 … dlc_imouto01_eye.png” 出处） |
| `sd01_f1/f3/f10/f30.png` | 验证“眼睛层是加载即绘制、非动画”的多帧证据 |
| `../m11_audio_loop_manifest.md` | 31 首 loop 机器审计清单（A/B/C/D 分档 + 每首指标） |

---

## 10. 一句话给 lead

**画面层已经做到“几乎逐像素复刻”**（8 张真实零售帧场景区 SSIM 0.946–0.973、MAE≈2.8–5.1），残差几乎全在 HUD 文字抗锯齿；**唯一成规模的画面差异是加载时多画了一层 `dlc_imouto01_eye.png`（H1，需人判是否应出现）**；锚点 5/6 的坐标算术与负缩放已单元级验证，但**缺隔离零售帧、像素级确认转人工（H2/H3）**；31 首 loop 机器已分档出清单，**28 首人耳复听仍是硬需求（H5）**。

---

## 11. task-22 修复后复测（AutoEvents::prime_latched）

> 复测日期 2026-11-02。task-22 已落地 `AutoEvents::prime_latched`：**读档不再伪造 `run_condition==2` 边沿**，此前读档时被误启动的 7 个 coroutine（含 506 DLC 眼睛层 `dlc_imouto01_eye.png` @ z=-6）不再出现。本节记录该修复对视觉基线与 P1E 8 存档对照的影响。

### 11.1 h_overlay 基线复核（两次 capture 一致性）

命令（与测试同路径：`--load-save work/retail_backup/Save/SaveData08.sav --frames 60`；`unset WOLF_GAME_DIR`、`WOLF_DATA_ROOT=work/all`）：

- 两次 capture 的 PNG **sha256 完全相同**：`a071f83964b561b9bdfac9d92c07941d6f4d2489a880d674960d353a0989b3d8`；两份日志除 `wrote:` 输出路径外逐字节一致 → 指标完全确定（可重复）。
- 复核后实测指标：`frames 60 / peak_coroutines 0 / pictures_live 84 / textured_quads 254 / text_pictures 17 / atlas 39 / text 17 item, 60 glyph, 17 line / nonblack 527714 / messages 0 / choices 0 / warnings 0`。

### 11.2 基线更新（仅 h_overlay）

只更新 `crates/wolf-cli/tests/m11_visual_regression.rs` 中 `name:"h_overlay"` 的期望字段，并加注释说明 prime_latched 修复与历史值；**其余 4 个基线、测试逻辑/断言结构未动**。

| h_overlay 字段 | 修复前（历史） | 修复后（新基线） |
|---|---|---|
| peak_coroutines | 7 | **0** |
| pictures_live | 86 | **84** |
| textured_quads | 257 | **254** |
| atlas_images | 42 | **39** |
| text_glyphs | 59 | **60** |
| nonblack | 527568 | **527714** |
| text_pictures / text_items / text_lines | 17 / 17 / 17 | 17 / 17 / 17（不变） |

`cargo test -q -p wolf-cli --test m11_visual_regression` → **5 passed / 0 failed**。

### 11.3 8 存档 P1E before/after（同一套 SSIM/PSNR/MAE 方法）

产物：`work/out/m13_p1e_after/`（`ours_sd01..08.png`、`metrics_after.json`、`overview_8pairs_after.png`、`eye_check_sd01.png`）。基准仍为 `work/retail_backup/Save/save1..8.jpg`。

| 档 | ssim_full B→A | PSNR B→A | MAE B→A | scene_mae B→A | scene_ssim B→A | >48% B→A |
|---|---|---|---|---|---|---|
| 1 | 0.9427→**0.9533** | 19.57→**21.11** | 4.90→**3.83** | 2.93→**1.63** | 0.9704→**0.9830** | 2.32→**1.77** |
| 2 | 0.9487→**0.9593** | 20.03→**21.86** | 4.49→**3.39** | 2.75→**1.45** | 0.9733→**0.9859** | 2.02→**1.47** |
| 3 | 0.9286→**0.9392** | 18.40→**19.52** | 6.38→**5.32** | 4.20→**2.89** | 0.9584→**0.9710** | 3.28→**2.73** |
| 4 | 0.9277→**0.9383** | 18.30→**19.44** | 6.46→**5.36** | 4.25→**2.94** | 0.9574→**0.9701** | 3.29→**2.74** |
| 5 | 0.9338→**0.9444** | 18.71→**19.98** | 5.84→**4.75** | 3.54→**2.23** | 0.9650→**0.9776** | 2.84→**2.28** |
| 6 | 0.9309→**0.9415** | 18.58→**19.81** | 6.11→**5.01** | 3.72→**2.41** | 0.9632→**0.9759** | 3.09→**2.53** |
| 7 | 0.9310→**0.9416** | 18.47→**19.66** | 6.12→**5.02** | 3.55→**2.25** | 0.9649→**0.9775** | 2.98→**2.42** |
| 8 | 0.9245→0.9190 | 19.26→18.78 | 6.49→6.91 | 5.05→**4.96** | 0.9461→**0.9479** | 3.96→4.19 |

（B=修复前 `work/out/m13_p1e/metrics.json`；A=修复后 `work/out/m13_p1e_after/metrics_after.json`）

### 11.4 眼睛层消失检查

sd01 上部眼弧区（y∈[85,165)、x∈[340,640)）灰度：修复前 mean=176.7、暗像素(<64)占 23.30%；修复后 mean=197.1、暗像素占 16.10% → 眼弧亮化消失。裁剪对照见 `work/out/m13_p1e_after/eye_check_sd01.png`（上=修复前眼弧可见，下=修复后干净）。各档 live pictures 由 86 降至 84、nonblack 与 text 计数随之变化，均印证伪启动图层已去除。

### 11.5 结论

- **1–7 档全部改善**：ssim_full +0.010~0.011、PSNR +1.1~1.5 dB、MAE −1.0~1.4、场景区 SSIM +0.012~0.013、场景 MAE −1.2~1.4、>48 像素占比 −0.5~0.6pp。
- **8 档（H 场景）**：全帧 ssim_full 0.9245→0.9190、MAE 6.49→6.91 **轻微变差**，但**场景区反而改善**（scene_mae 5.05→4.96、scene_ssim 0.9461→0.9479）；变差全部落在 **HUD 区**（hud_mae 14.26→17.47），根因是修复后 HUD 多渲染 1 个 glyph（text_glyphs 59→60、nonblack +146），属 HUD 文字层面，**非场景图形回归**。
- 综合：task-22 正面消除了读档伪造边沿引入的伪启动图层（眼睛层），画面层一致性整体提升；除 8 档因 ±1 HUD glyph 引起的全帧小幅波动外，**无任何场景指标变差**。原 §6 的 H1（眼睛层是否应出现）至此由 task-22 修复闭环。

