# LWSMF Remake（`030ee81f`）审阅缺陷与全端 / 移动端 UI·UX 修复闭环报告

- **对应审查台账**：[review_030ee81f_and_ux_fix_plan_2026_11_23.md](review_030ee81f_and_ux_fix_plan_2026_11_23.md)
- **架构决策条目**：[../godot/DECISIONS.md](../godot/DECISIONS.md)（`D-86`）
- **日期**：2026-11-23

---

## 1. 修复总表（UX-1 ~ UX-5 / REV-1 ~ REV-6）

| 编号 | 状态 | 关键改动与实际验证 |
| :--- | :---: | :--- |
| **UX-1** | **已闭环** | `ChoiceWindow.window_rect()` 对非屏外停放的显式 `Sys3/Sys4` 与 `-1` 哨兵统一按几何中心换算左上角并裁剪到安全区；同步修正 `GameRoot.gd` 注释与 `DialogDrawTest.gd` 居中/裁剪/停放断言（`DIALOG_DRAW_TEST_OK`） |
| **UX-2** | **已闭环** | `PlayOverlays.gd` 新增 `_drawer_tab_allowed` 与 `update_drawer_state(..., allow_tab)`，`_drawer_tab` 仅在标题与存档界面显示且上移至 `(12 + left_inset, 16 + top_inset)`（纵向 `y=16..50` 居中于顶框内）；`GameRootTest.gd` 新增标题/存档显示与局内隐藏断言（`GAME_ROOT_TEST_OK`） |
| **UX-3** | **已闭环** | `PictureStage.gd` 维护 `op124(field=8)` 轮询槽位 `_polled_hit_slots`/`_last_polled_hit_slots` 并提供 `rotate_hit_polls()` 与 `has_interactive_hit(point)`；`GameRoot.gd` 提供 `is_touch_blank_space(point)` 并绑定 `router.blank_space_predicate`；`InputRouter.gd` 仅在移动端默认开启单指双击空白区（`<=320ms` & `<=48px`）触发取消脉冲，同步新增 `InputRouterTest.gd` #26 与 `GameRootTest.gd` #8（`INPUT_ROUTER_TEST_OK` / `GAME_ROOT_TEST_OK`） |
| **UX-4** | **已闭环** | **锁定实机直接根因**：`CE94`（pc=405..418）在绘制存档槽 `DLC_data/save_SD_*.png` 前先以 `scale=0` 绘制角色库 Row 12 的 `SystemFile/charicon.png`（`200×200`）并用 `op124(2/3)` 反算缩放比例。本机 `D-85` 剪枝 `work/all/media.pak`（2,689 项）遗漏了 `SystemFile/charicon.png`（Windows 有散文件故正常），导致移动端回退 `420×140` 占位框（3:1）并算出 `22% × 68%` 比例将 SD 小人压扁拉长至 `44×136`。已：① 补齐 `PictureStage._apply_picture_geometry` 的 `scale_mode == 3` 与 `UNCHANGED` 守卫，② 扩展 `PictureStage.dump_pictures()` 追加 `resolved \| tex_size \| hd_scale \| split \| base_size \| scale`（`PICTURE_STATE_TEST_OK`），③ 用 `tools/audit_cross_platform_assets.py` 检出并补齐缺失的 21 个运行时引用素材（总计 `687 KB`，含 `SystemFile/charicon.png`）回本地 `work/all/media.pak`（`2,710` 项，`existing_refs_missing_in_pak=0`） |
| **UX-5** | **已闭环** | ① `AssetDB.gd` 补齐 15 类目录 `DIR_PRIORITY` 并增加等秩 `lower_rel` 字典序确定性裁决（`ASSETDB_TEST_OK` / `PICTURE_STATE_TEST_OK`）；② `PictureShapes.gd` 对 1× `NinePatchRect` 显式统一 `draw_center` 与 `AXIS_STRETCH_MODE_STRETCH`（`HD_PACK_TEST_OK` / `LAYER_ORDER_TEST_OK`）；③ `UiSkin.gd` 补齐 `IOS_FONT_NAMES`（`PingFang SC` 等）与 pak 路径探测，`android_data.py` 与 `ios_inject_data.py` 强制校验 `Lolita.ttf`（`--font` 或 `media.pak` 内含，`test_mobile_tools.py` 20/20 通过）；④ 新增跨端资产一致性审计脚本 `tools/audit_cross_platform_assets.py` |
| **REV-1** | **已闭环** | `MessageWindow.gd` 新增 `_last_blink_phase`，`finished` / `waiting_for_continue` 状态仅在 `int(floor(_cursor_time * 4.0))` 跳变时触发 `queue_redraw()`，消除对 `OS.low_processor_usage_mode` 的抵消 |
| **REV-2** | **已闭环** | `JsonArtifactIo.ResolvePath()` 在 `.json` 与 `.json.br` 并存时比较 `GetLastWriteTimeUtc`，明文更新则优先读取明文；`SaveLoadTest.cs` 新增时间戳回归用例（`SAVE_LOAD_TEST_OK`） |
| **REV-3** | **已闭环** | `tools/package_release.sh` 新增 `VERSION="0.1.0"` 与 `--version` 参数透传 `@APP_VERSION@`（`test_mobile_tools.py` 覆盖） |
| **REV-4** | **已闭环** | `GameSession.SaveGameToSlot(slot, refreshUi = true)` 与 `GameSessionController` 暴露可选开关，`GameRoot._on_save_slot_save_requested` 传入 `false` 后落盘缩略图再单次刷新 `CE94`；`SaveLoadTest.cs` 新增单测（`SAVE_LOAD_TEST_OK`） |
| **REV-5** | **已闭环** | `tools/package_ios.sh` 的 `dotnet new sln` 同步补充 `--format sln` |
| **REV-6** | **已闭环** | `tools/installer/lwsmf.iss.tmpl` 增加 `@EXTRA_LANGUAGES@` 占位符，`tools/package_release.sh` 在检测到 Inno Setup 安装目录存在 `ChineseSimplified.isl` 时自动注入简体中文语言条目 |

---

## 2. 本轮回归验证摘要

- **C# 编译**：`dotnet build godot/Lwsmf.csproj -c Release` 与 `-c Debug` 均 `0 warnings, 0 errors`。
- **无头模式 38 项核心门禁**：`bash tools/godot_check.sh` → `GODOT_GATE_OK elapsed=102s`（含 `DIALOG_DRAW_TEST_OK`、`INPUT_ROUTER_TEST_OK`、`PICTURE_STATE_TEST_OK`、`ASSETDB_TEST_OK`、`SAVE_LOAD_TEST_OK`、`GAME_ROOT_TEST_OK`、`HD_PACK_TEST_OK`、`PERFORMANCE_CORE_OK`、`PERFORMANCE_FRAME_OK` 及 `149,655` / `989,439` 步 Oracle 100% 一致）。
- **OpenGL3 渲染回归门禁**：`LWsmf_RENDER=1 bash tools/godot_check.sh` → `GODOT_GATE_OK elapsed=100s`（含窗口化 `DIALOG_DRAW_TEST_OK`、`PICTURE_ROUNDTRIP_TEST_OK` 与 `PERFORMANCE_STRICT_OK`）。
- **工具链与隐私/规范检查**：`python -m unittest tools/mobile/test_mobile_tools.py` → `Ran 20 tests ... OK`；`bash -n tools/package_release.sh tools/package_ios.sh` 退出码 `0`；`python tools/release_audit.py --repo-scan` → `REPO_PRIVACY_AUDIT_OK leaks=0`；补包后的 `work/all/media.pak`（`2,710` 项）经 `tools/audit_cross_platform_assets.py --allow-missing-references` 复检 `existing_refs_missing_in_pak=0`。
- **仍待真机端覆验**：Android / iOS 真机实装目视确认重打包后 `media.pak` 下存档槽 SD 小人比例、双击空白返回手感、菜单按钮位置与待机低功耗帧率回落（当前环境无真机连接，仅完成桌面无头/OpenGL3 及工具链闭环）。
