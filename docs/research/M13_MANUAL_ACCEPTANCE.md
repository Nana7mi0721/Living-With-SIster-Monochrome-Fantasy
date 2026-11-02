# M13 人工验收记录（H2–H5）

> 机器验收见 docs/research/M13_P1E_AUTOMATED_AUDIT.md；本文件只记录必须由人判定的结果。
> 对照包操作说明：work/out/h25_review/README.md（构建产物，gitignored）。
> 当前状态：**H2–H5 全部通过**（2026-11-02）。

## 汇总

| 项 | 内容 | 状态 | 结果 / 备注 |
|---|---|---|---|
| H2 | 锚点 5（顶部居中）零售像素确认：CE522、stand_sub12.png 718×700 @(512,100) → 预期 [153,100,718,700] | **通过** | 用户 2026-11-02 目视确认 |
| H3 | 锚点 6（底部居中）+ 负缩放镜像：CE245 夜菜单 7 个 482×225 反应图标（现帧为 SD_menu_013.png）@(240,576) scale=-100 → 预期 [-1,351,482,225] | **通过** | 用户 2026-11-02 目视确认 |
| H4 | HUD 文字抗锯齿是否可接受（8 张零售帧 HUD 区 top60 MAE 6.7–14.7，热点在字形边缘） | **通过** | 用户 2026-11-02 复验确认。修复含周六/周日颜色（savegame 调色板索引 + glyphon sRGB）、整行文字基线上移 2px、混排小字号（`日`/`G`）按基线对齐；8/8 白色行动条纵向 15 行铺满、周六蓝/周日红与零售一致 |
| H5 | 28 首 BGM loop 人耳复听（A17/B1/C10；D3 免） | **通过** | 用户 2026-11-02 已逐条勾选 28 首（A17/B1/C10） |

## 人工操作包

- 总览页：work/out/h25_review/index.html
- H4 对照：work/out/h25_review/h4_hud_sd01.png … h4_hud_sd08.png
- H2 预期几何：work/out/h25_review/h2_anchor5_expected_geometry.png
- H3 预期几何：work/out/h25_review/h3_anchor6_expected_geometry.png（另见 work/out/m9_negative_scale_hover.png）
- H5 试听页：work/out/h25_review/h5_loop_review.html；音频 work/out/h25_review/h5_audio/

## 验收结论（用户回复后填写）

- H2：通过（用户目视确认锚点 5 顶部居中几何）
- H3：通过（用户目视确认锚点 6 底部居中与镜像方向）
- H4：通过（用户复验确认白色行动条纵向 15/15 铺满；周六蓝 / 周日红；日期/金钱位置与混合字号基线与零售一致；top60 MAE 6.71–14.72）
- H5：通过（用户逐条勾选 28 首，记录见 M11_AUDIO_LOOP_REGRESSION.md §8）
- 结论人 / 日期：用户 / 2026-11-02（H2、H3、H4、H5 全部通过）
