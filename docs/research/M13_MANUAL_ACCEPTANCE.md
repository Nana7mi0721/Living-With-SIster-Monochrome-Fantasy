# M13 人工验收记录（H2–H5）

> 机器验收见 docs/research/M13_P1E_AUTOMATED_AUDIT.md；本文件只记录必须由人判定的结果。
> 对照包操作说明：work/out/h25_review/README.md（构建产物，gitignored）。
> 当前状态：**待用户验收**。

## 汇总

| 项 | 内容 | 状态 | 结果 / 备注 |
|---|---|---|---|
| H2 | 锚点 5（顶部居中）零售像素确认：CE522、stand_sub12.png 718×700 @(512,100) → 预期 [153,100,718,700] | 待验 |  |
| H3 | 锚点 6（底部居中）+ 负缩放镜像：CE245、1.4.0/SD_menu_010.png 482×225 @(240,576) scale=-100 → 预期 [-1,351,482,225] | 待验 |  |
| H4 | HUD 文字抗锯齿是否可接受（8 张零售帧 HUD 区 MAE 13.8–20，热点在字形边缘） | 待验 |  |
| H5 | 28 首 BGM loop 人耳复听（A17/B1/C10；D3 免） | 待验 | 逐首结果填 M11_AUDIO_LOOP_REGRESSION.md §8 |

## 人工操作包

- 总览页：work/out/h25_review/index.html
- H4 对照：work/out/h25_review/h4_hud_sd01.png … h4_hud_sd08.png
- H2 预期几何：work/out/h25_review/h2_anchor5_expected_geometry.png
- H3 预期几何：work/out/h25_review/h3_anchor6_expected_geometry.png（另见 work/out/m9_negative_scale_hover.png）
- H5 试听页：work/out/h25_review/h5_loop_review.html；音频 work/out/h25_review/h5_audio/

## 验收结论（用户回复后填写）

- H2：
- H3：
- H4：
- H5：
- 结论人 / 日期：
