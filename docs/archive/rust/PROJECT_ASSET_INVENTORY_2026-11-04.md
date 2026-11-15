# 项目资产清单（2026-11-04）

> **历史归档（2026-11-14）**：本清单记录的是 Rust 阶段终止时的资产状态，不代表当前工作树。
> 当前文档入口见 [`../../godot/README.md`](../../godot/README.md)。

> 状态：**项目已停止，不再继续重构**。本文件记录清理后的实际资产、已删除内容与恢复方式。
> 清点时间：2026-11-04　｜　工作目录：`E:/Study/github program/imoto life`
> 清理前约 **14.4 GB** → 清理后 **298.3 MB**（1826 个文件，磁盘占用约 303 MB，约释放 14.1 GB）。
> 另：本地 `.git/lfs/objects` 约 **766 MB** 为 `reverse-assets` 分支的 Git LFS 缓存（已推送到远端）；如不需要本地离线副本，删除本地 `reverse-assets` 分支后执行 `git lfs prune` 即可回收。

## 1. 保留资产总表

| 资产 | 路径 | 文件数 | 大小 | 说明 |
| --- | --- | ---: | ---: | --- |
| Rust 源码 | `crates/` | 227 | 3.7 MB | 7 个 crate，216 个 `.rs`，约 92,961 行；含 136 个测试文件 |
| 文档 | `docs/` | 286 | 36.4 MB | 177 个 Markdown；格式/opcode/里程碑/研究/攻略资料 |
| 工具与脚本 | `tools/` | 16 | 0.1 MB | 环境脚本、截图工具、冒烟测试、翻译管线 |
| 分析底稿 | `work/analysis/` | 466 | 41.6 MB | 结构化数据、调试输出、调查笔记、脚本归档 |
| 精选证据 | `work/out/` | 407 | 130.9 MB | UI/视觉/回归截图、对照图、评审报告 |
| 翻译工程 | `work/translation/` | 59 | 24.6 MB | 汉化文本、wiki 底稿、图片资源 |
| 原版存档备份 | `work/retail_backup/` | 19 | 4.1 MB | `Game.ini` + SaveData01..08 + System.sav + save1..8.jpg |
| 参考截图 | `游戏示例图片/` | 175 | 50.6 MB | 162 张原版实机截图（`游戏示例图片/截图/`，已按场景/日期语义命名）+ 13 张商店/宣传图 |
| 解码参考源码 | `.recon/` | 51 | 1.0 MB | UberWolf/WolfDec/WolfTL 等 C++ 参考 |
| 格式参考 | `_wolfref/` | 9 | 56 KB | WOLF 格式反编译/结构参考 |
| Cheat Engine | `cheat_engine/`、`妹相随 Ver3.CT` | 4 | 0.1 MB | 变量地址表、全能修改表 v4 |
| Git 历史 | `.git/` | — | 4.8 MB | HEAD `8c65153`（2026-11-03） |

## 2. 分目录明细

### 2.1 `crates/` — 可保留的完整工程

| crate | 角色 |
| --- | --- |
| `wolf-format` | Game.dat / CommonEvent / .mps / 数据库 / TileSetData / .wolf / .sav 格式解析 |
| `wolf-save` | 存档加解密与逐字节往返 |
| `wolf-vm` | 事件 VM、opcode、变量/表达式 |
| `wolf-render` | 软件渲染、字体、图层与截图后端 |
| `wolf-audio` | 音频解码/播放 |
| `wolf-game` | 游戏状态机、地图、战斗、H 场景、UI |
| `wolf-cli` | `play` / `capture` / `verify` / `package` / `bench` 入口 |

构建入口仍是根目录 `Cargo.toml`；`cargo build` / `cargo test` 会重新生成 `target/`（本次已删除）。

### 2.2 `docs/` — 文档

- 现行入口：`docs/README.md`、`docs/HANDOFF_M13.md`、`docs/UI_BUG_BACKLOG.md`、`docs/ENGINE_STRATEGY.md`
- 技术规格：`FORMAT_SPEC.md`、`SAVE_SPEC.md`、`OPCODE_SPEC.md`、`OPCODE_*.md`、`BATTLE.md`、`HSCENE.md`、`MAP_AUDIT.md`
- 研究底稿：`docs/research/`
- 用户资料：`docs/攻略/`（Markdown）、`docs/攻略文档/`（PDF/TXT，24.9 MB）、`docs/gpt调研/`
- 第三方工具：`docs/vendor/`（UberWolfCli.exe、WolfTL）

### 2.3 `work/` — 工作目录

| 路径 | 内容 |
| --- | --- |
| `work/analysis/data/` | `_ce_full.json`、`_battle_ce.json`、`maps_full.json`、`ce_codes.json`、`msgs101.bin` 等 55 个结构化文件 |
| `work/analysis/dumps/` | 334 个调试/审计文本 |
| `work/analysis/notes/` | 59 个 opcode/格式/任务调查笔记 |
| `work/analysis/reference/` | 17 个旧 HTML 参考页 |
| `work/analysis/scratch_scripts.zip` | 1488 个一次性探针脚本（.py/.ps1/.rs/.js），1.1 MB |
| `work/out/` | 407 个精选截图/对照图/评审证据；其中 `reviews/` 25.5 MB、`m13_p1e/` 14.7 MB、`h25_review/` 12.3 MB、`hudfix/` 11.7 MB、`refshots/` 8.3 MB、`ui_bugs/` 3.1 MB |
| `work/translation/` | `mv_text.json`、`glossary*.tsv`、`tree.json`、wiki 底稿、`final/` 等 |
| `work/retail_backup/` | 原版存档/设置备份，保留用于重新验证或恢复 |
| `work/_doc/`、`work/_ct/`、`work/uberwolf/` | 零散调查文档、CE 脚本、UberWolf 参考源码 |

> 旧文档中 `work/all/...`、`work/full/...`、`work/wolftl/...` 的路径已失效。目录说明见 `work/README.md`。

## 3. 已删除内容

### 3.1 发布包与构建产物

| 路径 | 清理前 | 说明 |
| --- | ---: | --- |
| `target/` | 约 9.2 GB | cargo 构建目录 |
| `release/` | 约 793 MB | 完整发布包（4067 files / 813,892,643 bytes）及 `wolf-cli.exe` |

### 3.2 重复/可再生成的游戏数据

| 路径 | 清理前 | 说明 |
| --- | ---: | --- |
| `work/all/` | 约 1.5 GB | 游戏解包数据 + .wolf 原包 |
| `work/all_base_steamverify/` | 约 534 MB | Steam 校验副本 |
| `work/full/` | 约 468 MB | 另一份解包数据 |
| `work/imouto/` | 约 343 MB | RPG Maker 版 Data/dump |
| `work/wolftl/`、`work/wolftl2/` | 各约 52 MB | 旧翻译/解包工作副本 |
| `work/runcopy/` | 约 41 MB | 含指向 Steam 的 Data junction（已安全移除 junction 后删除） |
| `work/tmp/` | 约 95 MB | 构建/测试临时目录 |
| `work/game/`、`work/_crates/`、`work/uw/`、`work/probes/` | 约 34 MB | 旧副本/探测目录 |
| `work/out` 未保留文件 | 约 512 MB | 历史逐帧截图与日志（保留精选 130.9 MB） |

### 3.3 下载的工具链

| 路径 | 清理前 |
| --- | ---: |
| `tools/bin/` | 56 MB |
| `tools/cmake/` | 152 MB |
| `tools/ffmpeg/` | 291 MB |
| `tools/rustup-init.exe` | 13 MB |

以上工具链可在需要时重新下载；`tools/` 中已跟踪的脚本/源码未删。

### 3.4 临时文件

- `_tmp_crates.py`、`_tmp_deps.py`、`ce.txt`
- 根目录旧的 1488 个一次性脚本已归档到 `work/analysis/scratch_scripts.zip`
- 旧的 10 个测试/调试日志已删除

## 4. 重新获取/重建方式

1. **原始游戏数据**：仍在 `E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`（约 2.6 GB）；需要时重新解包到 `work/all`。
2. **重新构建**：`cargo build --release -p wolf-cli`；发布包由 `wolf-package` 重新生成。
3. **重新运行验证**：`cargo test --workspace`；旧基线记录见 `docs/research/M11_VISUAL_REGRESSION.md`。
4. **查看历史证据**：`work/out/reviews/*.md`、`work/out/ui_bugs/contact_sheet.png`、`work/out/refshots/sheet1..7.png`。

## 5. 当前源码状态

- Git HEAD：`8c65153`（2026-11-03 `docs: record charicon icon and double-click release launch`）
- 跟踪文件：492 个；工作树有 50 项未提交改动/未跟踪资产（含本文件、最终 UI 修复、`docs/UI_BUG_BACKLOG.md`、`docs/ENGINE_STRATEGY.md`、新增测试与 `cheat_engine/`、`游戏示例图片/`）
- 本次清理仅删除被 `.gitignore` 忽略的构建/工作数据，未改动源码与 Git 历史。

## 6. 远程逆向资产分支（2026-11-04）

- 仓库：`https://github.com/Nana7mi0721/imoto-life.git`（**私有**）
- 分支：`reverse-assets`，commit `1f8c3f9`
- 浏览：https://github.com/Nana7mi0721/imoto-life/tree/reverse-assets
- 内容：`reverse/unpacked/` 4062 files / 735.8 MB（23 个 `.wolf` 全量解包）、`reverse/analysis/` 466 files / 41.6 MB、`reverse/evidence/` 91 files / 37.0 MB、`reverse/manifest.json`（4619 entries）/ `ARCHIVE_INDEX.md`；总 814.4 MB，提交 4623 files changed。
- Git LFS：794 MB / 3666 objects；`reverse/unpacked/**` 与证据图片由 LFS 跟踪。
- 本地分支：`reverse-assets`（已保留）；上传用的临时 worktree `E:/Study/github program/imoto life-reverse` 已移除，可用 `git worktree add <路径> reverse-assets` 重新展开。
- 版权说明：仅私有仓库个人研究、汉化与兼容性分析，不公开分发。

## 7. 历史质量记录（清理前，供追溯）

- 最后一次全量测试：153 个 test-result block / 957 passed / 0 failed / 14 ignored
- 最后一次 `verify`：PASS 70 / pending 0
- 最后一次发布包：4067 files / 813,892,643 bytes，`wolf-cli.exe` 12,256,768 bytes
- 已知未完成项：`docs/UI_BUG_BACKLOG.md` 的 UI-01..UI-13 曾在最终轮修复并由独立评审复核；用户最终仍决定停止重构。

---

*本清单由 2026-11-04 清理会话生成；目录体积为实际文件统计，可能因文件系统块大小略有差异。*
