# M11 音频循环验收卷宗（M10 residual · task-84）

> 目标：把「音频循环是否可接受」中**机器能查的部分全部查清并锁定**，同时**明确列出仍然只能靠人耳试听的项**，绝不谎称已完成试听。
> 数据根：`work/all`（可用 `WOLF_AUDIO_ROOT` 覆盖）。测试：`crates/wolf-audio/tests/m11_audio_loop_audit.rs`。
> 本卷宗不含任何播放代码改动；仅新增测试 + 本文档。

## 1. 范围与结论速览

- 全树共 **31** 个带 Vorbis 循环标签（`LOOPSTART`/`LOOPLENGTH`）的曲目，全部为 `.ogg`。
- **31/31 解码成功**；**31/31** 标签能解释出一个「区间起点 < 终点、且终点被 clamp 到解码帧数以内」的可用循环区间；无 START_OOB、无不可用区间。
- 按客观不连续度分级（见 §3）：**A 级 17**（强不连续嫌疑，其中 **14** 个绝对跳变已达 0.20+，**3** 个只有相对比值偏高而绝对跳变很小）、**B 级 1**（元数据异常：`LOOPLENGTH`=整轨帧数、循环到文件尾）、**C 级 10**（seam 平滑但 seam 处响度/直流有明显台阶）、**D 级 3**（无任何客观旗标）。
- **最终验收仍需人耳**：机器指标只能排除「巨大波形跳变 / 明显响度台阶」这类硬缺陷，无法替代「点击声是否可闻、循环点是否落在乐句边界、真机重采样后是否仍有爆音」的判断。§5 给出必听清单与逐曲检查项。

## 2. 方法与指标定义

对每个带循环标签的文件：`decode_file` 解码为 f32 交织 PCM；从原始字节里按 ASCII 大小写不敏感扫出未 clamp 的 `LOOPSTART`/`LOOPLENGTH`（帧）；再取 `decode.rs` 实际保留的区间 `[ls, le)`。在该区间接缝处计算：

| 指标 | 定义 | 含义 |
|---|---|---|
| `seam_abs` | `max_ch abs(sample(le-1,ch) - sample(ls,ch))` | 播放绕回「末样本→首样本」时的瞬时波形跳变（绝对幅度，0..2） |
| `step` | 接缝两侧各 1024 帧的平均相邻样本步长 | 该曲局部「正常一步」有多大，用于归一 |
| `ratio` | `seam_abs / step` | 无量纲：≈1 表示绕回看着像普通一步；越大越像「断口/点击」 |
| `rms_jump` | `abs(RMS(接缝后≈100ms) - RMS(接缝前≈100ms))` | 接缝处响度台阶 |
| `dc_delta` | `abs(mean(接缝后≈100ms) - mean(接缝前≈100ms))` | 接缝处直流偏置台阶 |

客观旗标阈值（仅供分诊，非断言）：`SHORT_LOOP`(loop<1s)、`SEAM_ABS`(seam_abs≥0.20)、`SEAM_RATIO`(ratio≥8)、`RMS_JUMP`(rms_jump≥0.10)、`DC_DELTA`(dc_delta≥0.05)、`OVERRUN`(LOOPSTART+LOOPLENGTH>总帧数)、`LEN_EQUALS_TRACK`(LOOPLENGTH==总帧数)、`START_OOB`、`NO_USABLE_LOOP`。

> 提醒：`ratio` 会惩罚「极其安静平滑」的段落（step 很小），所以 `ratio` 高但 `seam_abs` 极小的情况（A2）大概率不可闻；`seam_abs` 大（A1）才是真正需要人耳确认「有没有咔哒」的对象。

## 3. 31 曲目逐条清单（全部实测）

| 曲目 | 格式 | 采样率/声道 | 时长(s) | 帧数 | LOOPSTART | LOOPLENGTH | 解码 loop 区间(帧) | loop 长度(s) | seam_abs | ratio | rms_jump | dc_delta | 客观旗标 | 级 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| BGM/Acoustic_High_Collar.ogg | ogg | 44100/2 | 58.72 | 2589511 | 1360911 | 2589511 | 1360911..2589511 | 27.86 | 0.0195 | 20.02 | 0.0830 | 0.0005 | SEAM_RATIO,OVERRUN,LEN_EQUALS_TRACK | A |
| BGM/autumncome.ogg | ogg | 44100/2 | 57.05 | 2515863 | 822589 | 1693274 | 822589..2515863 | 38.40 | 0.1946 | 9.66 | 0.1212 | 0.0004 | SEAM_RATIO,RMS_JUMP | A |
| BGM/centennial_loop.ogg | ogg | 44100/2 | 134.18 | 5917423 | 240565 | 2886521 | 240565..3127086 | 65.45 | 0.3019 | 10.27 | 0.0867 | 0.0059 | SEAM_ABS,SEAM_RATIO | A |
| BGM/destiny.ogg | ogg | 44100/2 | 158.72 | 6999342 | 56201 | 6943141 | 56201..6999342 | 157.44 | 0.4619 | 11.35 | 0.0832 | 0.0016 | SEAM_ABS,SEAM_RATIO | A |
| BGM/Die_Letzte_Revolution.ogg | ogg | 44100/2 | 88.53 | 3904128 | 811014 | 2613192 | 811014..3424206 | 59.26 | 0.3626 | 7.23 | 0.1021 | 0.0002 | SEAM_ABS,RMS_JUMP | A |
| BGM/InstaVista.ogg | ogg | 44100/2 | 90.72 | 4000896 | 845923 | 2771822 | 845923..3617745 | 62.85 | 0.3884 | 4.49 | 0.0337 | 0.0200 | SEAM_ABS | A |
| BGM/nemesis_loop.ogg | ogg | 44100/2 | 183.91 | 8110491 | 4061036 | 3681423 | 4061036..7742459 | 83.48 | 0.8079 | 19.66 | 0.0278 | 0.0017 | SEAM_ABS,SEAM_RATIO | A |
| BGM/Our_Construction.ogg | ogg | 44100/2 | 101.85 | 4491648 | 2806 | 4491648 | 2806..4491648 | 101.79 | 0.5524 | 19.66 | 0.0365 | 0.0095 | SEAM_ABS,SEAM_RATIO,OVERRUN,LEN_EQUALS_TRACK | A |
| BGM/RAIN_&_Co.ogg | ogg | 44100/2 | 201.74 | 8896785 | 3737433 | 5058894 | 3737433..8796327 | 114.71 | 0.3204 | 2.87 | 0.1809 | 0.0096 | SEAM_ABS,RMS_JUMP | A |
| BGM/soul_engage_loop.ogg | ogg | 44100/2 | 175.99 | 7761171 | 425697 | 3877316 | 425697..4303013 | 87.92 | 0.6304 | 11.02 | 0.1910 | 0.0058 | SEAM_ABS,SEAM_RATIO,RMS_JUMP | A |
| BGM/straightcontrast.ogg | ogg | 48000/2 | 165.96 | 7966080 | 2148554 | 3686522 | 2148554..5835076 | 76.80 | 0.4450 | 13.97 | 0.0833 | 0.0131 | SEAM_ABS,SEAM_RATIO | A |
| BGM/Sunshine!.ogg | ogg | 44100/2 | 177.87 | 7843968 | 358273 | 6417034 | 358273..6775307 | 145.51 | 0.2290 | 3.44 | 0.1182 | 0.0017 | SEAM_ABS,RMS_JUMP | A |
| BGM/Tenseness.ogg | ogg | 48000/2 | 113.16 | 5431680 | 142664 | 4961751 | 142664..5104415 | 103.37 | 0.4660 | 9.34 | 0.0566 | 0.0005 | SEAM_ABS,SEAM_RATIO | A |
| BGM/The_End_of_Destruction.ogg | ogg | 44100/2 | 156.76 | 6913152 | 577941 | 6335211 | 577941..6913152 | 143.66 | 0.2800 | 6.37 | 0.1448 | 0.0144 | SEAM_ABS,RMS_JUMP | A |
| BGM/waste_loop.ogg | ogg | 44100/2 | 178.94 | 7891154 | 330750 | 3628812 | 330750..3959562 | 82.29 | 0.2520 | 5.54 | 0.0645 | 0.0027 | SEAM_ABS | A |
| BGM/windletter.ogg | ogg | 44100/2 | 149.45 | 6590592 | 781072 | 5438387 | 781072..6219459 | 123.32 | 0.1610 | 16.16 | 0.1331 | 0.0011 | SEAM_RATIO,RMS_JUMP | A |
| DLC_data/bgm01.ogg | ogg | 48000/2 | 182.91 | 8779778 | 4569648 | 4147911 | 4569648..8717559 | 86.41 | 0.8068 | 12.81 | 0.1574 | 0.0140 | SEAM_ABS,SEAM_RATIO,RMS_JUMP | A |
| BGM/We_are_Going.ogg | ogg | 44100/2 | 178.13 | 7855485 | 799430 | 7855485 | 799430..7855485 | 160.00 | 0.1707 | 6.59 | 0.0769 | 0.0136 | OVERRUN,LEN_EQUALS_TRACK | B |
| BGM/Across_-Battle_Results-.ogg | ogg | 44100/2 | 67.47 | 2975616 | 2357360 | 617983 | 2357360..2975343 | 14.01 | 0.0174 | 0.72 | 0.2047 | 0.0129 | RMS_JUMP | C |
| BGM/Be_yourself.ogg | ogg | 44100/2 | 104.77 | 4620348 | 1477927 | 2189792 | 1477927..3667719 | 49.66 | 0.0272 | 1.16 | 0.2885 | 0.0001 | RMS_JUMP | C |
| BGM/Killer_Roll.ogg | ogg | 44100/2 | 180.09 | 7941823 | 1246266 | 6135677 | 1246266..7381943 | 139.13 | 0.1988 | 2.57 | 0.1397 | 0.0078 | RMS_JUMP | C |
| BGM/line.ogg | ogg | 44100/2 | 160.99 | 7099776 | 2058 | 6974244 | 2058..6976302 | 158.15 | 0.0064 | 0.66 | 0.2517 | 0.0062 | RMS_JUMP | C |
| BGM/madoromu2.ogg | ogg | 44100/2 | 172.25 | 7596288 | 709878 | 3441961 | 709878..4151839 | 78.05 | 0.0516 | 3.02 | 0.1145 | 0.0158 | RMS_JUMP | C |
| BGM/morning.ogg | ogg | 44100/2 | 81.03 | 3573504 | 558229 | 2731328 | 558229..3289557 | 61.93 | 0.0115 | 4.57 | 0.1274 | 0.0001 | RMS_JUMP | C |
| BGM/okiraku.ogg | ogg | 44100/2 | 220.66 | 9731059 | 759270 | 8835347 | 759270..9594617 | 200.35 | 0.0423 | 1.33 | 0.1136 | 0.0037 | RMS_JUMP | C |
| BGM/p_egg_loop.ogg | ogg | 44100/2 | 105.72 | 4662341 | 179398 | 2332052 | 179398..2511450 | 52.88 | 0.0943 | 2.21 | 0.3772 | 0.0228 | RMS_JUMP | C |
| DLC_data/03 2NEONS.ogg | ogg | 44100/2 | 128.66 | 5674070 | 1604110 | 3953345 | 1604110..5557455 | 89.65 | 0.1949 | 1.93 | 0.1491 | 0.0048 | RMS_JUMP | C |
| DLC_data/bgm02.ogg | ogg | 48000/2 | 191.59 | 9196465 | 3950132 | 5202580 | 3950132..9152712 | 108.39 | 0.0774 | 3.02 | 0.3704 | 0.0110 | RMS_JUMP | C |
| BGM/ango.ogg | ogg | 44100/2 | 391.44 | 17262720 | 1485817 | 8304383 | 1485817..9790200 | 188.31 | 0.1159 | 1.00 | 0.0443 | 0.0009 | — | D |
| BGM/Migratory_Birds.ogg | ogg | 44100/2 | 106.50 | 4696704 | 349294 | 2753501 | 349294..3102795 | 62.44 | 0.1026 | 3.77 | 0.0339 | 0.0030 | — | D |
| DLC_data/bgm03.ogg | ogg | 48000/2 | 118.89 | 5706642 | 681867 | 5021630 | 681867..5703497 | 104.62 | 0.0247 | 2.77 | 0.0671 | 0.0000 | — | D |

## 4. 分级与「必须人工试听」清单

### A1 级：高绝对跳变（`seam_abs≥0.20`）——最可能听到接缝，优先试听（14 首）
  - BGM/Die_Letzte_Revolution.ogg（seam_abs=0.3626，ratio=7.23）
  - BGM/InstaVista.ogg（seam_abs=0.3884，ratio=4.49）
  - BGM/Our_Construction.ogg（seam_abs=0.5524，ratio=19.66）
  - BGM/RAIN_&_Co.ogg（seam_abs=0.3204，ratio=2.87）
  - BGM/Sunshine!.ogg（seam_abs=0.2290，ratio=3.44）
  - BGM/Tenseness.ogg（seam_abs=0.4660，ratio=9.34）
  - BGM/The_End_of_Destruction.ogg（seam_abs=0.2800，ratio=6.37）
  - BGM/centennial_loop.ogg（seam_abs=0.3019，ratio=10.27）
  - BGM/destiny.ogg（seam_abs=0.4619，ratio=11.35）
  - BGM/nemesis_loop.ogg（seam_abs=0.8079，ratio=19.66）
  - BGM/soul_engage_loop.ogg（seam_abs=0.6304，ratio=11.02）
  - BGM/straightcontrast.ogg（seam_abs=0.4450，ratio=13.97）
  - BGM/waste_loop.ogg（seam_abs=0.2520，ratio=5.54）
  - DLC_data/bgm01.ogg（seam_abs=0.8068，ratio=12.81）

### A2 级：比值高但绝对跳变很小（`ratio≥8`、`seam_abs<0.20`）——大概率不可闻，仍需一次确认（3 首）
  - BGM/Acoustic_High_Collar.ogg（seam_abs=0.0195 很小，但 ratio=20.02 偏高，因为局部 step=0.0010 很平）
  - BGM/autumncome.ogg（seam_abs=0.1946 很小，但 ratio=9.66 偏高，因为局部 step=0.0202 很平）
  - BGM/windletter.ogg（seam_abs=0.1610 很小，但 ratio=16.16 偏高，因为局部 step=0.0100 很平）

### B 级：元数据异常——`LOOPLENGTH` 等于整轨帧数，解码循环区间一直跑到文件尾（1 首）
- BGM/We_are_Going.ogg
  - 这 3 首的 `LOOPLENGTH` 恰好等于总帧数（Acoustic_High_Collar 2589511、Our_Construction 4491648、We_are_Going 7855485），按 `decode.rs` 的 `start+len` 语义会被 clamp 到 EOF，即「从 LOOPSTART 循环到文件尾」。这是资产标记约定问题，不是解码 bug；但「从 LOOPSTART 到 EOF 是否就是作者意图的循环区间」必须由人耳/原始素材确认。其中 Acoustic、Our_Construction 同时属 A1。

### C 级：接缝波形平滑但响度/直流有明显台阶（10 首）
  - BGM/Across_-Battle_Results-.ogg（rms_jump=0.2047，dc_delta=0.0129）
  - BGM/Be_yourself.ogg（rms_jump=0.2885，dc_delta=0.0001）
  - BGM/Killer_Roll.ogg（rms_jump=0.1397，dc_delta=0.0078）
  - BGM/line.ogg（rms_jump=0.2517，dc_delta=0.0062）
  - BGM/madoromu2.ogg（rms_jump=0.1145，dc_delta=0.0158）
  - BGM/morning.ogg（rms_jump=0.1274，dc_delta=0.0001）
  - BGM/okiraku.ogg（rms_jump=0.1136，dc_delta=0.0037）
  - BGM/p_egg_loop.ogg（rms_jump=0.3772，dc_delta=0.0228）
  - DLC_data/03 2NEONS.ogg（rms_jump=0.1491，dc_delta=0.0048）
  - DLC_data/bgm02.ogg（rms_jump=0.3704，dc_delta=0.0110）

### D 级：无任何客观旗标（3 首）——结构最干净，仍建议入耳
- BGM/Migratory_Birds.ogg、BGM/ango.ogg、DLC_data/bgm03.ogg

## 5. 为什么这些项仍然只能靠人耳（机器测不出的部分）

1. **乐句/节拍边界**：`seam_abs` 小只说明波形连续，不代表循环点落在乐句重拍上；错拍循环听感是「节奏被打断」，任何波形指标都测不出。
2. **点击声可闻性**：A1 的 `seam_abs` 是单样本跳变，是否「咔哒」取决于内容频谱与播放音量——同一数值在打击乐里可忽略、在长音垫底里很明显。
3. **真机重采样**：多数曲目源为 44.1kHz，输出设备常为 48kHz，mixer 会重采样。相等采样率下的绕回连续性有单测（`engine_bgm_loop_wrap_is_sample_continuous`，见 `crates/wolf-audio/tests/audio_inventory.rs`），但**非整数比重采样后的接缝听感**只能实测。
4. **立体声像/相位**：`seam_abs` 取各声道最大值，测不出一侧相位跳变造成的空间感突兀。
5. **循环意图**：B 级（`LOOPLENGTH`=整轨）等标记是否就是作者想要的区间，无法从数据断言。
6. **循环时长体感**：`SHORT_LOOP`（本次无）与「太短导致重复感强」是主观判断。

**逐曲试听检查表（人耳）**：接缝处是否有「咔/噗」、循环点是否在乐句末/首对齐、响度是否突变、左右是否偏、连听 3~5 个循环是否露馅、真机（目标设备 48kHz）下复听一次。

## 6. 已由机器锁定（`m11_audio_loop_audit` 断言）

- 循环标签曲目**恰为 31 个**（`EXPECTED_LOOP_TAGGED`），全为 `ogg`。若资产集变化，测试与本文档同步更新。
- 每首都**解码成功**、采样率>0、帧数>0。
- `LOOPSTART` 必存在；解码区间必可用；无 `START_OOB` / `NO_USABLE_LOOP`。
- 「原始标签 → 解码区间」契约精确成立：`ls==LOOPSTART`，`le==min(LOOPSTART+LOOPLENGTH, 总帧数)`（缺 `LOOPLENGTH` 时取总帧数）。
- 所有指标有限（无 NaN/Inf）。
- 指标**确定性**：同一次运行内二次比对完全一致；表格按路径排序，输出稳定。

与既有测试的关系：`crates/wolf-audio/tests/audio_inventory.rs` 负责全量解码/inventory 与 mixer 回归；`crates/wolf-audio/tests/m10_audio_residual.rs::loop_tag_audit` 已对同一批 31 首做结构审计。本测试在此之上补了 `rms_jump`/`dc_delta`/`ratio` 三个接缝不连续度指标与明确的人耳分诊分级，作为**验收卷宗**，不重复播放代码。

## 7. 复现

    cargo test -p wolf-audio --test m11_audio_loop_audit -- --nocapture
    # 数据根不在默认位置时：
    WOLF_AUDIO_ROOT="E:/Study/github program/imoto life/work/all" cargo test -p wolf-audio --test m11_audio_loop_audit -- --nocapture

实测：`1 passed; 0 failed`（约 7s，31 首全解码）。

## 8. 残余与结论

- **机器可判定的部分：全部通过**——31/31 解码、区间合法、标签契约成立、无结构性越界；已量化每首接缝不连续度并分诊。
- **不能由机器判定的部分：仍待人工试听**——A1（14 首）最可能可闻，优先；A2（3 首）大概率无碍但需一次确认；B（`We_are_Going`，另含 Acoustic/Our_Construction）需确认「循环到 EOF」是否为意图；C（10 首）确认响度台阶是否可接受；D（3 首）结构最干净仍建议入耳。
- 本卷宗**不代表已完成正式听感验收**；它把「需要耳朵」的范围压缩到上表并按风险排序，供人工作业时逐条勾掉。

_TASK-84 交付；未修改任何播放代码；未提交。_
