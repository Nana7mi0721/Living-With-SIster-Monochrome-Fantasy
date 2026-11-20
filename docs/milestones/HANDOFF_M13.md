# M13 交接：内容一致三批验收（2026-11-01 标准）

> **历史证据（2026-11-14 冻结）**：Godot 阶段入口见 [`godot/README.md`](godot/README.md)；本文件不再追加“下一步”。
>
> 仓库：`E:\Study\github program\imoto life`
> 阅读顺序：本文件（M13 最新）→ `docs/README.md` → `PLAN.md`；`HANDOFF_M12.md` 及更早已标历史归档。
> 数据根 `work/all`；零售存档 `work/retail_backup/Save`（SaveData01..08 + System.sav）。
> 本批提交（master，已 push）：`7c3b728` common-event self 持久修复；`1826edd` 内容矩阵/DLC/主线探针；`ba608f6` 跨天存读/H 阈值/NG+/随机事件 + op111 语义更正；`dcd513a` Text picture 存读修复；`a8d98c8` 九结局其余/条件注入事件/H 端到端/DLC EP26-36；`7044649` 读档 auto-events prime + h_overlay 基线 + P1-E 复测。

## 1. 一句话现状

M13 的自动化内容一致验收三批已完成：首批修 **common-event self 跨调用持久 bug**（SaveData01/08 day90 可无注入自然通关，stage28→29、var492=1、warnings=0）与 Text picture 消息窗尺寸存读丢失；第二批补齐九结局其余、17 项条件注入随机事件、H 端到端、DLC EP26–36 结案；第三批定位并修复**读档伪造 run_condition==2 边沿**（AutoEvents::prime_latched，读档多画 DLC 眼睛层闭环）、重录 h_overlay 视觉基线并完成 P1-E 修复后复测。全量 144 块 / **863 passed / 0 failed / 14 ignored**，`wolf-cli verify` **PASS 70/0**。后续 HUD 调色板/文字基线修复后全量 **146 块 / 874 passed / 0 failed / 14 ignored**，`verify` **PASS 70/0**；H2–H5 人工验收已由用户全部通过（2026-11-02，见 §6 与 `docs/research/M13_MANUAL_ACCEPTANCE.md`）。

## 2. 本批交付（均新增测试 + 研究底稿）

| 主题 | 测试 | 文档 | 结论 |
| --- | --- | --- | --- |
| 主线结局（无注入） | `crates/wolf-game/tests/m13_mainline_endings.rs`（3 默认 + 2 ignore） | `docs/research/M13_MAINLINE_ENDINGS.md` | SaveData01/08 day90→day95：CE251 self51 4→3→2→1→0，stage28 首帧(day95,f164)→stage29，**var492=1、warnings=0**，抓到贺词“恭喜您完成了整个故事剧情…”。SaveData05→day101 逃亡 END（CE305 stage27 期限切れ）。stage30 实为“尖叫胡萝卜”支线，不置结局旗标、门 var366==0 与通关后互斥。 |
| DLC CE452 | `crates/wolf-game/tests/m13_dlc_452.rs`（5 默认 + 1 ignore 全矩阵） | `docs/research/M13_DLC_452.md` | CE251@2498 零参 210 唤 CE452，EP0–EP36 37/37 通过（注入 self10=n 证）；自然推进 var619 0→25（EP0–EP25），EP26 起被 CE452 内 self11 等数据门拦住（非 bug）。 |
| 内容矩阵 | 文档 + 抽查 | `docs/research/M13_CONTENT_MATRIX.md` | 115 行随机事件已定位 114/115（#78 数据不可达结案）；H 6 类门、战斗 CE152/192、物品/服装/胖次、相册/夢精、九结局坐标。 |
| 111 随机事件抽样 | `crates/wolf-game/tests/m13_random_events.rs`（4 默认 + 1 ignore） | `docs/research/M13_RANDOM_EVENTS.md` | 25 事件/23 用例覆盖 12 池，warnings=0 且命中台词；调用目标扫描 19624，dead=0，dormant=8（CE270 8 个 op300 悬空，0 调用者不可达）。 |
| H 阈值矩阵 | `crates/wolf-game/tests/m13_h_thresholds.rs`（3 测） | `docs/research/M13_H_THRESHOLDS.md` | 37 条门命令逐字节锁定；阈值-1/==阈值边界在 VM 上翻转；CE318 信頼=100 spawn CE360、=99 不 spawn。 |
| NG+ 继承 | `crates/wolf-game/tests/m13_ngplus.rs`（3 默认 + 2 ignore） | `docs/research/M13_NGPLUS.md` | CE436 点数表 + SampleMapA 继承菜单均由脚本驱动，生产路径可用（var550 来自 System.sav；置 2 可进 9 选项菜单并结算）。风险：save/system.sav vs Save/ 大小写敏感 FS（仅记录，未改 src）。 |
| 跨天存读/8 槽 | `crates/wolf-game/tests/m13_crossday_save.rs`（2 默认 + 1 ignore） | `docs/research/M13_CROSSDAY_SAVE.md` | 8/8 零售档 load→save→load→save 逐字节幂等；新开档 day4/day11；S1==S2==S3 首次中途存档即不动点（Text picture 修复后）；self banks 跨存读保持。 |

## 3. 本批修复的 bug

1. **common-event self 变量跨调用持久**（`7c3b728`，`crates/wolf-vm/src/exec.rs`、`crates/wolf-game/src/game.rs`、`savegame.rs`、`battle_wiring.rs`）：VM 原来每次 common event 调用 self 全 0，Part3 只写不读，导致 CE251 倒计时/CE452 章节计数每日复位、主线与 DLC 不可达。新增 `Vm.common_self` + dirty 集，start 时播种、reap/stop 写回，savegame 读档播种（不标脏保 8/8 逐字节），save_to 写 dirty∪live。合成 battle 驱动改用 `start_event_fresh`。
2. **Text picture 消息窗尺寸存读丢失**（`dcd513a`，`savegame.rs`）：`picture_to_record` 对 Text kind 把 vars7_18[2]/[3] 写成 split=(1,1)，活动消息窗 16001（7680×1200）第一次中途存档后丢尺寸。修复为 Text 用 window/基线回写；新增单测；首次中途存档即 byte fixed point。

## 4. 关键更正（推翻旧假设）

- **op111 单条多三元组是“首真命中的有序分档”，不是逻辑与**（`docs/OPCODE_SPEC.md` §5.9、`program.rs` BlockKind::Condition）。因此 CE318 @338/@415 = “欲求不満≥2 走 case1，否则 発情度≥75 走 case2”；CE323 @1060 = 性興味 250/150/0 三档；@1055 = 信頼≥200/≥0 两档。只有嵌套单条件 111（如 CE310@159→161→163→165）才等价 AND。已同步更正 M13_CONTENT_MATRIX / M13_CROSSWALK_UNRESOLVED / M13_RANDOM_EVENTS。
- 周末 H @163 为 **性欲(var18)≥40**（非欲求不満）；厕所计数为 **var222**（var221 是オナニー進行）；参军 END 位于 CE326@35–71（信頼≤5 ∧ 機嫌≤1）。
- var493 不是逃亡判据；逃亡唯一判据 = CE305 stage27 被调度 + BAD END 文本。

## 5. 验证门禁（本批集成后）

```bash
WOLF_GAME_DIR=work/retail_backup WOLF_DATA_ROOT=work/all cargo test -q --workspace --no-fail-fast
# 144 test-result 块 / 863 passed / 0 failed / 14 ignored / EXIT 0
./target/debug/wolf-cli.exe verify --root work/all   # PASS 70 check(s) / pending 0 / EXIT 0
```

> 环境注意：不 pin `WOLF_GAME_DIR=work/retail_backup` 时，`wolf-save` 的 `every_save_parses_with_the_measured_shape` 与 `typed.rs` 两条会因用户 Steam 安装档已被游玩更新（386830B vs 366146B）而红，与本项目代码无关。
> verify 注意：不带 `WOLF_GAME_DIR` 时检查 `dxarchive.*`，结果为 PASS 70 / pending 0；带 `WOLF_GAME_DIR=work/retail_backup` 时 dxarchive 不可见，报 64 matched + 1 pending（dxarchive.header），两者均 EXIT 0。门禁以不带 env 的 **70/0** 为准。

## 6. 首批遗留（第二批闭环见 §6.1；仅人工项仍开放）

1. **九结局**：已测 逃亡、正常通关；其余 7 条已由 §6.1 的 `m13_all_endings.rs` 闭环。
2. **条件注入型随机事件 17 项**：已由 §6.1 的 `m13_random_events_injected.rs` 闭环。
3. **P1-E**：机器部分已闭环（8 张零售帧 SSIM/PSNR/MAE、锚点 5/6、31 首 loop 分档、h_overlay 基线重录）；仅剩人工 H2–H5（锚点零售像素核对、HUD 抗锯齿验收、28 首 loop 人耳）——待用户。
4. **DLC EP26–EP36**：已结案（task-18）。不是移植缺口：var395 是永久「献身」技能标志（不可重置）；EP26 卡住是隔离探针缺“已升级队伍”战斗属性所致，预置真实战斗属性后同一局自然链 EP0→36 全通、var619=36。
5. 文档收口：`PLAN.md`/README 的 M13 段、`docs/research/README.md` 登记、`docs/BASELINE.json` 证据键均已更新。

## 6.1 第二批验收（本轮完成）

- 九结局其余 7 条：`m13_all_endings.rs`（14 默认 + 2 ignored 全绿），覆盖妊娠/兵役/返家/病亡症状链/暗影/魔界/农民/爆炸；`M13_ALL_ENDINGS.md`。
- 17 项条件注入随机事件：`m13_random_events_injected.rs`（3 测试全绿：16 DLC 事件点 + 特4 海豹 + 15 门三元组）；`M13_RANDOM_EVENTS_INJECTED.md`。
- H 端到端：`m13_h_endtoend.rs`（5 测试全绿：周末/厕所/浴室/CE335/餐厅）；餐厅门更正为 CE326@801–805 `var424≥3 ∧ var18≥100 ∧ var16≥1`；`M13_H_ENDTOEND.md`。
- DLC EP26–36（task-18 结案）：`m13_dlc_452_ep26_36.rs`（2 默认+2 ignored）；**不是移植缺口，未改生产 src**。var395 是永久「由衣献身」技能解锁标志，绝不能在战斗结束重置；EP26 卡住是隔离探针未预置“已升级队伍”战斗属性的假象（默认战力~15 vs 敌槽~4500）。一次性预置 `var2000040..45/2000170..175=300`、var395 全程不清洗后，同一局自然链 EP0→EP36 全通、var619=36（diagnostic 未升级队伍卡 EP26 maxpc=4224）。`M13_DLC_452_EP26_36.md`。
- 全量：`860 passed / 0 failed / 14 ignored`；`wolf-cli verify` PASS（64 项匹配 baseline + 1 项 dxarchive pending，退出 0）。

## 6.2 第三批验收（本轮完成：读档伪启动修复 + 视觉基线 + P1-E 复测）

- **根因**：读档 `savegame::apply` 恢复变量/DB/picture 时未同步 `auto_events.latched`；首帧 sync 把当时为真的 `run_condition==2` 事件伪造成 false→true 边沿启动。8/8 零售档 part3 row_words[610]==1，事件 506（妹トイレ前まばたき）随即画 `DLC_data/dlc_imouto01_eye.png` @ z=-6，而零售同帧无此层。
- **修复**：`crates/wolf-game/src/auto_events.rs` 新增 `pub fn prime_latched(&mut self, vm: &Vm)`：按当前真值预置 `latched`，不 start/stop、不改 enabled；`crates/wolf-game/src/savegame.rs::apply` 在 part5 picture 重建后调用。读档运行态由 SavePart5 快照呈现，不再伪造边沿。
- **测试**：新增 `crates/wolf-game/tests/m13_auto_events_load.rs`（load SaveData01/08 后无 506 实例、无 pic -6；HUD `pic 6000` 保留；fresh boot event38 仍启动；2 passed）；`m7_hauto_events` 按裁决 (A) 改为“读档不伪启动 + gate 0→1 才调度”语义（2 passed）；15 个 RNG 敏感 fixture 仅在测试内 `set_event_seed(N)` 固定种子、断言零改动（18 target 0 failed）；`crates/wolf-vm/src/rng.rs` 零改动。
- **视觉基线**：`crates/wolf-cli/tests/m11_visual_regression.rs` 的 h_overlay 基线按修复后重录（peak 7→0、live 86→84、quads 257→254、atlas 42→39、glyphs 59→60、nonblack 527568→527714；两次 capture PNG sha256 一致）；仅改 h_overlay，其余 4 基线未动；5 passed。
- **P1-E 修复后复测**：8 零售档 1–7 全指标改善（全帧 SSIM +0.010~0.011、PSNR +1.1~1.5 dB、场景 SSIM +0.012~0.013）；第 8 档全帧小幅波动 0.9245→0.9190，但场景区改善 0.9461→0.9479，变差仅在 HUD（+1 glyph），无场景回归；眼睛层已消失（sd01 眼弧暗像素 23.30%→16.10%）。见 `M13_P1E_AUTOMATED_AUDIT.md` §11 与 `work/out/m13_p1e_after`。
- **全量**：`144 test-result 块 / 863 passed / 0 failed / 14 ignored / EXIT 0`；`wolf-cli verify --root work/all`（不带 env）= **PASS 70 / pending 0 / EXIT 0**。

## 6.3 第四批（bug 修复）：行动力条白色填充没有铺满纵向

- **症状**：加载 SaveData01（75/120 AP）后 HUD 行动力条 6233 的白色填充只有 11/15 行高，底部露出一条灰底（用户截图 `work/out/m13_p1e_after/cmp_hud_retail_old_new.png` 中栏）。
- **根因**：零售 8 个存档 part5 中，图片 6233 是窗口类 `<SQUARE>` 150×14，横向缩放=AP 比例（62）、**纵向恒 100**；`Picture` 只有一个 `scale`，`destination` 把 62% 同时用于两轴，于是高度也被压到 62%。
- **修复**：`crates/wolf-game/src/picture.rs::Picture::destination` —— 窗口类图片（`kind.is_window()`，含 `<SQUARE>/<GRADX>/<CIRCLE>` 等 sysgraphic）的 `height_scale=1.0`，高度取窗口框；文件/文本图片仍等比。
- **验证**：新增单测 `a_window_gauge_scales_x_only_so_the_fill_spans_the_full_bar_height`（6233 62×100→93×14）；逐列像素：零售 W=15/G=0、修复前 W=11/G=4、修复后 W=15/G=0；8 存档 `load→save` 仍逐字节一致（`m12_save_full_compare` 2 passed）；`wolf-game --lib` 239 passed、`m11_visual_regression` 5 passed（无基线漂移）。
- **顺带产出**：新增 `wolf-cli` 子命令 `wolf-layout`（`crates/wolf-cli/src/bin/wolf-layout.rs`）导出任意公共/地图事件的 `150/101` 绘制表，把 HUD 布局从「肉眼核对」改成「读数据」；底稿 `docs/research/UI_LAYOUT.md`（含 CE216/217/226 布局表与「窗口类仅横向缩放」规则）。

## 7. 约束与未跟踪

- 不提交：3 个永久 scratch（`crates/wolf-format/examples/opscan.rs`、`tests/_scratch_cdata.rs`、`tests/_scratch_event.rs`）；用户资料 `docs/攻略文档/`、`妹相隨 Ver3.CT`、`游戏示例图片/`。
- 提交信息用 `git commit -F work/_commit_*.txt`；临时文件写 `work/out`。

## 8. P2 打包与性能（2026-11-03，第一轮实现 + 收口）

- **交付**：`crates/wolf-game/src/data_root.rs` 启动数据根校验（`Game::new` 立即 `check_layout`，`GameError::DataRoot` 明确错误）；`assets.rs` 新增 `cache_bytes()` / `cache_stats()`，`play` 退出打印 `assets N image(s), X MiB`；`wolf-package assemble` 新增 `--font`（`WOLF_GAME_DIR` 自动取 Lolita/07yasashi/GenJyuuGothicXPB/MSGOTHIC），并跳过 `.wolf` 源归档；`play.rs` 输出 data/window/renderer/assets/font/window ready 六段启动拆解与 step p50/p95/max/mean。
- **发布包**：真实 `work/all` 组装 **4070 files / 814,703,960 bytes**（data 4065 files / 772,716,836 bytes + 4 fonts），`wolf-package check` **PACKAGE OK / 4070 files verified**；包内 `./wolf-cli.exe play --root data --boot --until 300` 冒烟通过（0 warnings，exit 0，字体自包含）。
- **修 bug**：`crates/wolf-cli/src/bin/wolf-package.rs:979` 手写 JSON 字符串解析器 `byte as char` 把 UTF-8 多字节按 Latin-1 解码，导致含日文/中文文件名的 manifest 路径 mojibake；真实包曾报 `MISSING (30)`/`EXTRA (30)`。改为 `Vec<u8>` 累积 + `std::str::from_utf8` 解码，转义前 flush；新增 `manifest_json_round_trips_non_ascii_paths` 与 m11 合成根 `data/SystemFile/くしゃくしゃ.png` 回归。
- **性能实测**：release bench（work/all, SaveData01, CE188, 600f）`p50=1.5µs p95=2.8µs max=86.4µs warnings=0`；打包版 warm 启动 `data 97–114ms / window 147–181ms / window ready 872–877ms`。EnumWindows 外测：零售首窗口 165–480ms（标题恒 `Starting up, initializing system`），我方游戏窗口 ~251ms 可见；零售真正首帧不可观测，因此只声称窗口可见同量级，首帧 warm ~0.87–0.9s 如实记录（瓶颈 wgpu adapter ~276ms + swapchain configure ~192ms，临时 profiling 已移除）。
- **测试**：`cargo test -p wolf-cli --bin wolf-package --test m11_package` = 3 unit + 4 m11 passed；全量 **146 块 / 878 passed / 0 failed / 14 ignored / EXIT 0**（日志 `work/out/p2_final_test.log`），`wolf-cli verify --root work/all` = **PASS 70 / pending 0 / EXIT 0**。
- **文档**：`docs/research/M11_PACKAGING.md` 已按 release 数据重写；`PLAN.md` P2 #8 已更新。

## 9. P2 收口门禁

```bash
WOLF_GAME_DIR=work/retail_backup WOLF_DATA_ROOT=work/all cargo test -q --workspace --no-fail-fast
./target/debug/wolf-cli.exe verify --root work/all   # 目标 PASS 70 / pending 0
```

- 全量结果：**146 test-result 块 / 878 passed / 0 failed / 14 ignored / EXIT 0**（`work/out/p2_final_test.log`）；`wolf-cli verify --root work/all` = **PASS 70 / pending 0 / EXIT 0**（`work/out/p2_verify.log`）；`wolf-package check --package work/out/package_p2_release` = **PACKAGE OK / 4070 files verified**。
- 未完成/需人工：零售真正首帧的仪器化对比；H2–H5 像素/音频人耳（既有清单）。

## 10. H-PRNG-1 生产时钟播种（2026-11-03）

- **结论**：H-PRNG-1 从「仍开放」解除。零售事件 RNG 在 RVA 0x35847 调 GetTickCount、0x35852 直接写 state VA 0x011b17f0（无 folding，零状态重读时钟），已由 M11_PRNG_CLOCK 证实；此前只缺生产接线。交互 `wolf-cli play` 现在在 `Game::new` 成功后、boot/load 前用本地时钟毫秒低 32 位播种。
- **实现**：`crates/wolf-vm/src/rng.rs` 新增纯函数 `resolve_session_event_seed(raw: Option<&str>, clock_ticks: u32) -> u32`：`Some` 走 `resolve_event_seed`（`WOLF_EVENT_SEED` 回放），`None` 走 `seed_from_clock_ticks` 并把 0 归一化为 `EVENT_RESEED`。`crates/wolf-cli/src/play.rs` 调用 `game.vm.rng.set_event_seed(event_seed)` 并打印 `startup: event RNG seed <seed> (clock|WOLF_EVENT_SEED)`。库内 `WolfRng::new` / `Game::new` 与诊断命令仍固定 `EVENT_SEED=1`，既有测试向量不变。
- **测试**：`crates/wolf-vm/tests/prng_seed.rs` 10 测（原 8 + 生产策略 2）：无覆盖用时钟、同 tick 同流/不同 tick 不同流、零 tick 归一化、显式 seed 优先、非法值回退。实跑 `wolf-cli play --root work/all --boot --until 2` 两次 seed 2675025206 / 2675026572（clock），`WOLF_EVENT_SEED=24680` 得 24680（WOLF_EVENT_SEED），均 0 warnings。全量 `cargo test --workspace`：**146 test-result 块 / 880 passed / 0 failed / 14 ignored / EXIT 0**（`work/out/h_prng1_workspace_test.log`）。
- **残留**：与某一次具体零售运行逐位对齐仍需该次 GetTickCount 原值（用 `WOLF_EVENT_SEED=<tick>` 注入），是可选验证，不是功能缺口；零 tick 的零售行为仍为假设。
- **文档**：`docs/UNVERIFIED.md` 统计变为 4 解除 / 2 证据缺口 / 3 开放；`docs/research/M9_PRNG_SEED.md` 增 Production wiring；`docs/research/M11_PRNG_CLOCK.md`、`docs/BASELINE.json` 已同步。

## 11. 磁盘清理（2026-11-03）

- **触发**：P2/H-PRNG 收口后按 `AGENT.md` §6.2 做一次全盘体检与瘦身。
- **前后**：`du -sb .` 从 19,780,431,973 B（约 19 GB）降到清理后约 3.7 GB，释放约 15 GB。
- **删除**：`target/`（`cargo clean`，48,648 files / 11.6 GiB）；`tools/smoke/target` 与 `tools/wgpusmoke/target`（约 2.0 GB）；`work/out` 中不再被文档/测试引用的中间截图、日志、临时 patch、旧发布包（共 1.81 GB，保留 282 个被引用的关键截图/日志/夹具，80.1 MB）；`work/mtout` 临时探针可执行文件（6.7 MB）。
- **保留**：`work/all`（约 1.5 GB）、`work/all_base_steamverify`（534 MB，独立裸安装留档；已逐文件核对，与 `work/all` 非重复：2,395 同、164 缺失、1,506 不同）、`work/full`（468 MB）、`work/imouto`（343 MB）、`work/retail_backup`（4.2 MB）。`work/out/package_p2_release` 已删除，可用 `wolf-package assemble` 重建。
- **状态**：清理后 `git status` 仍只有 3 个永久 scratch 与用户资料（`docs/攻略文档/`、`妹相隨 Ver3.CT`、`游戏示例图片/`）；未删除任何已跟踪文件或用户资料。

## 12. 最新发布包（2026-11-03 重建）

- **位置**：仓库根 `release/`（已加入 `.gitignore`，commit `4c5f85a`）；包内为 `wolf-cli.exe`、`data/`、`data/Fonts/`（Lolita / 07yasashi / GenJyuuGothicXPB / MSGOTHIC）与 `manifest.json`。直接双击 `wolf-cli.exe` 会识别同目录 `data/` 并启动 play（无 command 即 play）。
- **规模**：**4067 files / 813,818,915 bytes**（data 4062 files / 771,585,007 bytes + 4 fonts + exe 12,183,040 bytes）；`wolf-package check --package release` = **PACKAGE OK / 4067 files verified**。
- **启动程序图标**：`crates/wolf-cli/build.rs` 用 Windows SDK `rc.exe` 把 `crates/wolf-cli/assets/wolf-cli.ico`（由 `work/game/Data/SystemFile/charicon.png` 生成，含 16/24/32/48/64/128/200/256 八档）编进 `wolf-cli.exe`；`crates/wolf-cli/src/play.rs::window_icon()` 同时把同一 PNG 解码为 winit 窗口图标，标题栏/任务栏一致。`rc.exe` 不在 PATH 时依次找 `RC`、`PATH`、`WindowsSdkDir`、`Windows Kits\10\bin\<ver>\<arch>`，都没有只告警不阻断。已用 `ExtractAssociatedIcon` 从包内 exe 抽图核对（32x32 可见）。
- **与 §8 的差异**：本次从 `work/all` 组装时排除了 `work/all/Save` 下 3 个测试跑出的存档（避免把测试进度打进发布包），比 §8 少 3 个文件；exe 为双击启动+图标版，比上一版 exe（11,944,448 bytes）大 238,592 bytes。建包后 `work/all/Save` 已原样恢复。
- **冒烟**：包内 `./wolf-cli.exe --until 300`（无 command，等同双击）= **300 frames / 9393 instructions / 10 pictures / 0 warnings / EXIT 0**；启动 data 82 ms、window created 138 ms、window ready 891 ms、step p50 0.45 ms / p95 0.72 ms；220 SaveLoad 仅打印“absents; using synthesized new-game template”（预期，不进入 warnings）；烟测只生成空 `data/Save` 目录，已删除，manifest 无 Save。
- **构建**：release 二进制放在仓库外 `E:/cargo-target/imoto/release`，避免重新生成仓库内 `target/`；重建日志 `work/out/icon_release_build.log`、`work/out/icon_release_assemble.log`、`work/out/icon_release_check.log`、`work/out/icon_release_smoke.log`。
- **本轮门禁**：`cargo test --workspace` = **146 test-result blocks / 882 passed / 0 failed / 14 ignored / EXIT 0**（`work/out/icon_workspace_test.log`）；`wolf-cli verify --root work/all` = **PASS 70 / pending 0 / EXIT 0**（`work/out/icon_verify.log`）。
