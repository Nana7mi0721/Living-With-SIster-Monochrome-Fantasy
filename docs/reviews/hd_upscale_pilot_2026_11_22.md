# 2× 活体素材超分与 HD 资产层落地评审（2026-11-22）

## 1. 结论

- 统一 2× 口径成立：1024×576 基准图放大到 2048×1152（短边 1152 ≥ 1080p），2× 恰为 Real-CUGAN 的原生倍率，不需要二次重采样。
- 收益前提是渲染管线改造：`stretch/mode` 从 `viewport` 改为 `canvas_items`，否则 2048×1152 贴图会先被压进 1024×576 帧缓冲再拉伸，超分等于白做。
- 模型矩阵定为 **Real-CUGAN `models-pro`**，按目录切换 denoise：UI=0、立绘/差分=-1、JPG 背景=1、其余 PNG=0。样板对比中它对线稿与文字边缘最稳；`models-se` 更平滑但细节略糊；`waifu2x cunet` 锐度高但会改变文字笔画粗细。
- 运行时按可选 `media_hd.pak` 加载，1× 包不受影响；移动端默认关闭并回退 1×。
- 该引擎构建存在两类必须绕开的缺陷：`-f webp` 直出正常尺寸但像素错乱；纯白 255 输入会触发 NaN/彩虹噪声。管线改为 **PNG 输入输出 + 近白像素钳制 249 + 输出 MAE 校验**，坏图一律回退 Lanczos 并计入 fallback。

## 2. 口径与范围

| 项 | 取值 |
|---|---|
| 目标倍率 | 固定 2×，不归一化、不做后处理缩放 |
| 输出基准 | 2048×1152（1024×576 源） |
| 活体清单 | `work/media_manifest.txt`，2,689 行 |
| 图像条目 | 2,506 张（PNG 1,717 + JPG 789） |
| 音频/字体 | 183 条，全部跳过 |
| 素材源 | `work/retail_dat`（本机解包树，2,847 文件 / 615,764,547 B） |
| 跳过目录 | `MapChip/`（正片未使用，`godot/README.md:66`） |
| 护栏 | 2× 后长边 >8192，或源像素 >4,194,304 时保持 1×，不写入 manifest |

1× 的 `work/all/media.pak` 不替换、不重打；HD 层是独立包，缺失即回退。

## 3. 渲染与布局改造

| 文件 | 改动 |
|---|---|
| `godot/project.godot` | `window/stretch/mode="canvas_items"`；`window_width_override=2048`、`window_height_override=1152` |
| `godot/presentation_gd/GameRoot.gd` | Integer 缩放档要求窗口 ≥2× 基准（`integer_scale_fits`），否则回落 fractional+linear；`size_changed` 重算；读取 `display/hd_textures` 注入 `AssetDB.set_hd_enabled` |
| `godot/presentation_gd/AssetDB.gd` | 打开 `<root>/media_hd.pak` 或 `LWsmf_HD_PAK`，解析 `hd_manifest.json`，按目录优先级建 basename 索引；`hd_scale()` / `hd_path()` / `_image_from_hd()`；缓存预算 384→512 MB，移动端 192 MB 且默认关闭 |
| `godot/presentation_gd/PictureStage.gd` | `_base_size_for` / `_texture_cell_size` / `_build_picture` 按资产倍率除回逻辑尺寸；`AtlasTexture.region` 仍用纹理像素 |
| `godot/presentation_gd/UiSkin.gd` | `nine_patch` 分离 src/dst 转角：贴图 ≥27px 时按 `3 * width / 18` 取样，布局恒 3px |
| `godot/presentation_gd/PictureShapes.gd` | 2× 窗体贴图改用 `Control` + `draw` 自绘（避免 `NinePatchRect` 边框翻倍），1× 仍走原节点 |

## 4. 超分管线

`tools/upscale_assets.py`：

1. 读白名单 → 按目录选 denoise → 生成 `(path, key)` 任务。
2. 小图/细长图（min 边 <64）不走引擎，直接 Lanczos 2×；这类图共 253 张。
3. 其余重编码为干净 PNG 并钳制近白像素（RGB ≤249），按 denoise 组批量送 Real-CUGAN（`-s 2 -m models-pro -f png -j 1:1:1`）。
4. 输出先做尺寸/alpha 校验，再做内容 MAE 校验（下采样回源尺寸，与源图比较，阈值 25）；不合格回退 Lanczos 并记 `fallback=true`。
5. 合格图用 Pillow 无损 WebP（method=4）与优化 PNG 竞价取小；索引写入 `work/hd/hd_manifest.json`，逐图日志写入 `work/out/hd_build_png.jsonl`。

`tools/verify_hd_manifest.py` 逐条校验文件存在、尺寸 = 源×2、alpha 未丢（源 alpha 恒 255 时允许丢通道），并统计未覆盖条目；有缺失/尺寸不符/未覆盖返回非 0。

## 5. 三模型样板对比

对比图：`work/out/p0/compare_sheet.png`（6 个场景 × Lanczos / CUGAN-pro n=-1 / CUGAN-se n=0 / waifu2x cunet n=0），指标：`work/out/p0/metrics.csv`。

- **文字层（`cg/tips_00.png`）**：CUGAN 两档笔画边缘明显比 Lanczos 干净；waifu2x 笔画偏细，细笔画有断感。
- **立绘（`Picture/stand_sub08.png`）**：CUGAN-pro 的衣褶与描线最接近原画；se 略糊；waifu2x 锐但出现轻微边缘振铃。
- **H 背景（`hscene/h_bg_002.png`）**：CUGAN 字效边缘完整；waifu2x 字缘偏薄。
- **切入特效 / DLC SD**：四档差异较小，CUGAN-pro 在纯色块边缘更稳。
- **JPG 背景**：CUGAN 对压缩噪点的抑制优于 waifu2x，未见涂抹。

结论：统一 `models-pro`，denoise 按 §4 的目录表分配；不引入 se/cunet 作为全局替代。

## 6. 全量产出

- 白名单图像 2,506 张，实际入选 2,421 张（跳过 85 张护栏项，缺失 0）：引擎 2,168 张，CPU Lanczos 253 张。
- `work/hd`：2,421 个文件、**1,397.4 MiB**；`hd_manifest.json` 2,421 条，`verify_hd_manifest.py` 复检 `missing=0 mismatch=0 degraded=0 uncovered=0`（`opaque_alpha_dropped=4` 属已知恒 alpha 源）。
- `media_hd.pak`：**1,464,087,236 B（约 1.36 GiB）**，2,421 entries，SHA-256 `e7093c71df9838c522af0b16256bc15c18e66bb6bcc9635b4e4a53508e1d3a62`。
- 引擎分布：`realcugan/models-pro/n0` 1,816 张、`n-1` 352 张；最终回退 Lanczos 259 张（253 张 min 边 <64 的 CPU 项 + 6 张引擎不稳项）。
- 全量耗时 3,010 s（约 50 分钟），编码速率约 38 张/分钟。

## 7. 踩坑与修复（均已复现）

1. **GUI 不可作管线**：`settings.ini` 默认有损 Q96、压平目录、同名覆盖，且无 CLI。改用目录内 ncnn CLI。
2. **min 边 <64 崩溃**：Real-CUGAN 对 18×18、606×35 输入报 `vkWaitForFences failed -4` / `0xC0000005`。管线对该类改走 Lanczos。
3. **`-f webp` 坏图**：输出尺寸与 alpha 正常，但像素呈彩虹噪声。60 张抽样 25 张 MAE>40；同图改用 PNG 输出后 MAE 0.39。管线禁用引擎 WebP 直出。
4. **纯白 NaN**：`tips_00.png` 等含大片 255 白的图层，即使 PNG 输出仍整图噪声；把 RGB 上限钳到 249 后连续 5 次输出 MAE 5.2，稳定。254/253/252 仍会复现。后续全量又发现 249 并非普适阈值：部分 RGBA CG（如 `DLC_data/dlc_imouto10_01.png`）在 249 下仍输出彩虹噪声，降到 245 即恢复（MAE 2.7），最终对 13 张复检项改用 244/238 局部重跑，避免全库压暗高光。
5. **批处理线程竞争**：`-j 2:2:2` 批量运行时部分图会偶发同类噪声；改 `-j 1:1:1` 并保留 MAE 兜底。
6. **长批次静默劣化**：同一引擎进程连续处理超过约 126 张后，输出开始整图彩虹噪声（尺寸/alpha 均正常，MAE 100+）；管线改为 **每 48 张重启引擎**，并提供 `--retry-fallback` 对已落盘坏项逐张重跑。
7. **Alpha 丢弃误判**：4 张源图 alpha 恒 255，引擎输出无 alpha 属正常；校验器按源 alpha 极值区分，不记为降级。

## 8. 验收

- `dotnet build godot/Lwsmf.csproj -c Release`：0 warning / 0 error。
- 38 项 Godot headless 门禁（带最终 `media_hd.pak`）：`GODOT_GATE_OK elapsed=92s`，含 `HD_PACK_TEST_OK`、`PICTURE_STATE_TEST_OK checks=123`。
- 渲染门禁：OpenGL3、2048×1152 窗口、120 帧无 ERROR；`PERFORMANCE_FRAME_OK fps=143.7`。
- 视觉回归：1×（1024×576）与 2×（2048×1152 + HD 包）各 6 个场景，`VISUAL_CONTACT_SHEET_OK count=6`，`unresolved=0`；2× 截图内容铺满 2048×1152，文字与九宫格边框为原生 2× 渲染，无拉伸糊边。对比图：`work/out/visual_1x|visual_2x/visual_contact_sheet.png`、`work/out/compare_03_retail_save01_bedroom.png`。

## 9. 边界与未做

- 253 张小图（含 18×18 九宫格、30×120 字效）走 CPU Lanczos，未做 SR；另有 6 张引擎不稳项（`CharaChip/cursor.png` 与 5 张 1240×310 的 command_cutin/eyecatch）也回退 Lanczos，合计 259 张，占 10.7%。
- 85 张被护栏跳过（含 `MapChip/` 54 张、超大图层），运行时按 1× 回退。
- 未做 PlayOverlays 图形菜单开关，当前开关为 `user://display_settings.cfg` 的 `display/hd_textures` 与 `LWsmf_HD_PAK`。
- HD 素材与对比图不入库；仓库只保留脚本、清单与文档。
