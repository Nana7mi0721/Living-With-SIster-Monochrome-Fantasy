# 《妹相随》Godot 重制版统一核验、深度 Bug 摸排与 CI 故障诊断报告（commit `c61d762`）

> 审阅对象：`Living-With-SIster-Monochrome-Fantasy`（commit `c61d76248ea633c211e761a3d83e7c24c39cff11`）。  
> 合并范围：① `release_gate_verification_2026_11_21.md` 与 `lwsmf_8fa1189_audit_remediation_2026_11_21.md` 逐项核验；② 全仓 17,177 行源码深度 Bug 与游玩隐患摸排；③ GitHub Actions `ci.yml` 失败截图根因诊断。  
> 证据边界：本沙箱无 Godot / .NET / Rust 编译器、无 `game_data/` 与零售素材，所有需实机或编译运行验证的项均标 `needs-runtime` / `needs-retail-data`。

---

## 1. 总评与核心结论

1. **既有文档修复基本落地**：两份 `2026-11-21` 报告声称的核心修复（含生产 `GameRoot` 切换至 `TakePicturesJson()` 图片队列排空、子事件点击抑制、`102 bit4` 取消权限、`101+102` 同屏、6 路 SE 池、`12000..12599` 存档过滤、`crash.log`）均已在 `c61d762` 源码落实。
2. **新定位 1 项 P1 实机串态 Bug + 7 项 P2 游玩/边界缺陷**：最影响实际游玩的是**读档与故障恢复漏重置 `screen_tint` 和 `message.position_mode`**（夜晚读白天档画面持续发暗、居中框读档后普通对话错画居中），其余涵盖卧室点击残留移动、切窗失焦吞确认、存档误存瞬态 SE、F4/F12 弹窗叠盖、多位数 `@10` 头像截断、安装向导括号路径失败以及门禁文档误写“语音触发”。
3. **GitHub Actions CI 失败根因已定位（24 秒 `exit code 1`）**：失败步骤是 `.github/workflows/ci.yml:58` 的 `cargo fmt --all -- --check`。仓库无 `rustfmt.toml`，而 `crates/` 中存在 378 行超过 `rustfmt` 默认 100 列宽度的未格式化代码（如 `m11_audio_loop_regression.rs:205-210` 的 245 列结构体字面量），在环境就绪后立即报错退出；截图中的 2 条黄色 Warning 为旧版 v4 Action 的 Node.js 20 弃用提醒。

---

## 2. 既有两份报告声称项逐项核验表

| 类别 | 文档声称已解决项 | 源码核验状态 | 关键源码证据（文件:行） | 备注 / 边界 |
| --- | --- | --- | --- | --- |
| **前景事件与穿透** | `EventEntered(eventId, isRoot)` 一次性通知 + 子事件点击抑制至物理释放 | **已落实（有失焦边界）** | `EventVm.cs:1694`, `GameRoot.gd:338-343`, `InputRouter.gd:51,169-170,206-207,229-253` | 切窗失焦未清 `_action_entry_suppressed`（见 §3 `BUG-02`） |
| **输入与选项** | `102 bit4` 禁止取消透传、空格为确认、取消不跳过未读、向上滚轮开回顾 | **已落实** | `EventVm.cs:688`, `ChoiceWindow.gd:25,69,127`, `InputRouter.gd:26-28`, `GameRoot.gd:390-427` | 逻辑完整 |
| **对话与文本** | `101 -> 102` 同屏、逐字速度/`\.`/`\|`/`\!`/`\-[n]`/避头尾换行/语音触发 | **部分落实（无语音触发）** | `EventVm.cs:669-675`, `MessageWindow.gd:43,153-168`, `TextTokenizer.gd:17,24-25,97` | 源码无任何语音触发实现（见 §3 `BUG-08`）；排版需 `LWsmf_RENDER=1`（`needs-runtime`） |
| **存读档与故障** | 过滤 `12000..12599` 与 `CE14/94/131`、读档清浮层与回滚、`1000` 缩略图、`crash.log` | **已落实（漏清染色/框模式）** | `GameSession.cs:85-86,535-585,686-706,793`, `GameRoot.gd:222-223,654-696,847-859`, `GameSessionController.cs:102-159` | 读档/故障恢复漏重置 `screen_tint` 与 `message.position_mode`（见 §3 `BUG-01`） |
| **音频系统** | 6 路 SE 声部池、SE 延迟独立、WAV `loop_end`、同曲 BGM 不重播 | **已落实（SE 误入档）** | `AudioManager.gd:14,36-45,65-76,117-146,276-288,347-356` | `snapshot()` 误保存瞬态 SE 且 `loop_begin` 未钳制（见 §3 `BUG-04`） |
| **画面与染色** | `151` 独立染色层、`12000+` 置顶、`blend == 3` 半透明淡出 | **已落实** | `PictureStage.gd:32,114-125`, `PictureEffects.gd:6-14,61-68` | `MULTIPLY_SHADER` 数学正确 |
| **数据与语义** | `124` field 2/3 为缩放百分比、`150 scale==0` 保留、`150 kind=1/4` 动态展开、13 张 Tips 断言 | **已落实** | `EventVm.cs:1019`, `tools/godot_check.sh:57` | `needs-retail-data` / `needs-runtime` |
| **长跑信号崩溃（R8）** | 生产 `GameRoot` 改用 `TakePicturesJson()` 图片队列排空 | **已落实** | `GameRoot.gd:215-237,297-311` | 彻底替代跨语言 `EmitSignal(PictureRequested)` |
| **卧室移动关闭** | 卧室阶段关闭隐形移动主角坐标入口 | **部分落实（有遗漏）** | `GameRoot.gd:429-438`（已删）；`GameRoot.gd:794-811,834-835`（**未删**） | `_on_map_cell_clicked` 仍连接并调 `MoveLeader`（见 §3 `BUG-03`） |
| **构建与门禁** | Release 0 警告、38 步无头门禁、Oracle 对拍（`2461` / `149655` / `989439`） | **文档有记录（沙箱未重跑）** | `tools/godot_check.sh:46-342`, `release_gate_verification_2026_11_21.md:8-16` | `needs-runtime`；`LWsmf_RENDER=1` 本轮未执行 |

---

## 3. 全量代码摸排发现的未报 Bug 与实机游玩隐患（统一去重版）

### 3.1 P1 / P2 明确缺陷

| 编号 | 严重度 | 类型 | 位置（文件:行） | 实际游玩触发场景与现象 | 根因 | 最小修复建议 |
| --- | --- | --- | --- | --- | --- | --- |
| `BUG-01` | **P1** | 实机串态 | `GameRoot.gd:847-859`（`_on_load_completed`）、`1322-1357`（`restart_after_fault`）、`1361-1403`（`recover_to_last_save`） | ① 在夜晚/傍晚/红色染色场景（`151`）按 `F9` 或读档菜单载入白天存档，白天画面残留暗色滤镜直到下次 `151`；② 在居中旁白框读档，下一句普通对话仍画在屏幕中央；③ 故障返回标题后残留旧名牌、屏幕染色，且窗口模式未重新播种 RNG | `_on_load_completed` 漏调 `message.set_position_mode(MSG_POS_BOTTOM)` 与 `stage.reset_screen_tint()`；`restart_after_fault` 对比 `restart_title`（`1179-1200`）漏清名牌、位置模式、屏幕染色与 `_seed_live_rng_if_windowed()` | 抽取 `_reset_presentation_transient_state()`，在 `_on_load_completed`、`restart_after_fault`、`recover_to_last_save` 统一重置上述状态；`restart_after_fault` 补调 `_seed_live_rng_if_windowed()` |
| `BUG-02` | **P2** | 新增边界 Bug | `GameRoot.gd:747-759`, `InputRouter.gd:51,229-253` | 触发子事件抑制或确认抑制（`_action_entry_suppressed = true`）期间切出窗口，并在窗外松开左键/确认键；切回窗口后首次点击或按确认键被吞掉 | `FOCUS_OUT` 仅清理了 `_modal_release_guard` 与指针 latch，未重置 `router._action_entry_suppressed` | 在 `InputRouter` 增加 `clear_suppression()` 并在 `GameRoot._notification(FOCUS_OUT)` 调用 |
| `BUG-03` | **P2** | 遗漏修复 | `GameRoot.gd:794-811,834-835` | 卧室阶段点击地图相邻格仍会隐形移动主角坐标 | `map_view.cell_clicked` 仍连接到 `_on_map_cell_clicked` 并调用 `session.MoveLeader(...)`，与文档“卧室地图移动入口关闭”矛盾 | 移除 `_on_map_cell_clicked` 中的卧室移动调用（或直接 `return`） |
| `BUG-04` | **P2** | 音频与存档 | `AudioManager.gd:182-222,285-287` | ① 在瞬态 SE 播放瞬间按 `F5` 存档，每次 `F9` 读档都会凭空重播半截 SE；② WAV `loop_point` 超界时 `loop_begin >= loop_end` 导致循环异常 | `snapshot()` 遍历全部 `CHANNEL_COUNT`（含 `CHANNEL_SE`），`restore()` 读档一并恢复；`_apply_loop` 未对 `loop_begin` 上限钳制 | `snapshot()` 仅保存 BGM/BGS（跳过 `CHANNEL_SE`）；`wav.loop_begin = clampi(begin, 0, total_samples - 1)` |
| `BUG-05` | **P2** | UI 与交互 | `GameRoot.gd:928-938`, `PlayOverlays.gd:218-237` | ① 打开 `F12` 返回标题确认框时按 `F4`，回顾窗叠盖在确认框之上（或反之）；② `F4` 回顾中长按 `↑/↓/W/S` 无法连续滚动，浏览 200 条记录只能单点 | `KEY_F4` 与 `KEY_F12` 未做模态互斥检查；`PlayOverlays._input:222` 过滤了 `key.echo` | 打开 `F4` 前检查 `not title_confirm_open()`，打开 `F12` 前检查 `not backlog_open()`；对回顾方向键放行 `key.echo` |
| `BUG-06` | **P2** | 文本解析不一致 | `EventVm.cs:664-668` vs `MessageWindow.gd:97-102` | 若任意 `101` 台词含两位数头像前缀（如 `@10你好`），头像号被误拆为 `1` 且正文开头多出孤立字符 `"0"` | `EventVm.cs:664-668` 仅读取单个字符 `expanded[1] - '0'`，而 `MessageWindow.gd` 支持连续多位数 | 将 `EventVm.cs` 的 `@<n>` 扫描改为连续数字解析，与 `MessageWindow.gd` 保持一致 |
| `BUG-07` | **P2** | 向导兼容性 | `SetupWizard.gd:246-249` | 首次运行向导选择含右括号 `)` 的路径（如 `D:/Games/LWSMF (v1.4)/Data`）点击“开始转换”直接报 `cmd.exe` 语法错误 | Windows 命令模板 `"cd /d \"%s\" && (%s) > ..."` 中的 `)` 会提前闭合 `(...)` 分组 | 改为写入临时 `.bat` 脚本执行或去掉外层括号分组 |
| `BUG-08` | **P2** | 文档与代码不符 | `release_gate_verification_2026_11_21.md:32`, `MessageWindow.gd`, `TextTokenizer.gd` | 门禁报告声称 `MessageWindow.gd` 已接入“语音触发”，但源码中无任何语音触发回调或控制码 | 文档描述超前或把 `AudioManager` 的 SE `voice` 声部池误写进 `MessageWindow` | 若原版语音全走独立 `140` 指令，更正门禁文档；若 `101` 含内嵌语音码则补齐实现 |

### 3.2 P3 轻度体验与语义边界

1. **`12000..12599` 硬编码过滤边界（`GameSession.cs:85-86,567,691,702`）**：若后续 DLC 在该槽位段放置非存档 UI 的持久系统图片，存读档时会被静默剔除；建议保持边界记录或在扩展时仅于 `IsSaveUiEvent` 活跃期过滤。
2. **模态打开时打字机未暂停（`MessageWindow.gd:170-201`）**：打开 `F4` 回顾或 `F12` 确认框暂停了 `GameRoot.advance_frame()`，但 `MessageWindow._process` 仍在后台跑完当前台词。
3. **存档未持久化 `screen_tint`（`151`）**：`SaveState` 目前不保存环境染色，在夜晚/染色场景存档后重开读档需等下一次 `151` 恢复。

---

## 4. GitHub Actions CI 失败根因分析（对应截图）

### 4.1 失败现象（commit `c61d762`，总耗时 41s）
- `.NET C# Core Build Check`：**通过**（37s），伴 1 条 Node.js 20 弃用警告。
- `Rust Workspace Check & Oracle Test`：**失败**（24s），`Process completed with exit code 1.`，伴 1 条 Node.js 20 弃用警告。

### 4.2 根因定位
1. **直接元凶：`cargo fmt --all -- --check` 失败（`.github/workflows/ci.yml:58`）**
   - `Rust Workspace Check & Oracle Test` 仅运行 **24 秒**即退出——在 `windows-latest` 上，Checkout + `dtolnay/rust-toolchain@stable` + 缓存恢复约占 20–22 秒，紧随其后的第一步就是 `cargo fmt --all -- --check`。
   - 经全仓扫描，仓库根目录**没有 `rustfmt.toml`**（使用默认 100 列规则），而 `crates/` 下存在 **378 行超过 100 字符的代码行**，典型如 `crates/wolf-audio/tests/m11_audio_loop_regression.rs:205-210`（单行结构体字面量长达 203–247 字符且无 `#[rustfmt::skip]`）。因此 `cargo fmt --all -- --check` 会在 1–2 秒内直接返回 `exit code 1`，后续的 `clippy` 与 `cargo test` 尚未执行即中止。
2. **潜在二次风险（修复 `fmt` 后需同步关注）**
   - `.github/workflows/ci.yml:61` 执行 `cargo clippy --workspace --all-targets -- -D warnings`，若 `crates/` 在最新 `stable` 工具链下存在任意 clippy 新警告也会报错（`needs-runtime`）。
   - `.github/workflows/ci.yml:64` 步骤名为 `Offline Unit Tests`，但命令为 `cargo test --workspace --no-fail-fast`（未加 `--lib`），会编译并运行 `crates/*/tests/` 下的 90 余个集成测试；虽然 `audio.rs`、`database.rs` 等多数测试在缺 `work/all` 时会跳过，但仍建议显式验证全部集成测试在无 `work/all` 环境下不会 panic，或对仅跑单元测试的步骤加上 `--lib`（`needs-runtime`）。
3. **两条黄色 Warning 原因**
   - 截图提示 `Node.js 20 is deprecated... actions/checkout@v4, actions/setup-dotnet@v4, actions/cache@v4`。这是 GitHub Actions 平台逐步淘汰 Node 20 运行时的提示，不导致失败，升级到新版 Action（如 `@v5` 或官方 Node 24 兼容版本）即可消除。

### 4.3 CI 最小修复方案
1. 在本机执行 `cargo fmt --all`（对 `m11_audio_loop_regression.rs` 等大表可加 `#[rustfmt::skip]` 保持表格可读性）并提交格式化变更。
2. 在本机无 `work/all`（或重命名 `work/all` 临时测试）下运行：
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace --no-fail-fast
   ```
   确认全部通过后再推主干。
3. 同步更新 `.github/workflows/ci.yml` 中的 `actions/checkout`、`actions/setup-dotnet`、`actions/cache` 版本以消除 Node.js 20 警告。

---

## 5. 统一修复与本机验收顺序（`needs-runtime`）

1. **第一批（立即修复并推 CI 变绿）**：
   - 修复 `BUG-01`（读档/故障恢复重置 `screen_tint`、`message.position_mode`、名牌与 RNG 播种）；
   - 修复 `BUG-02`（`FOCUS_OUT` 清理 `_action_entry_suppressed`）、`BUG-03`（关闭 `_on_map_cell_clicked` 卧室移动）、`BUG-04`（SE 不入档 + WAV `loop_begin` 钳制）、`BUG-05`（F4/F12 互斥 + 回顾方向键连发）；
   - 执行 `cargo fmt --all` 并升级 `ci.yml` Action 版本，恢复 GitHub Actions 全绿。
2. **第二批（细节收口与文档一致）**：
   - 修复 `BUG-06`（`EventVm.cs` 多位数 `@<n>` 解析）与 `BUG-07`（`SetupWizard.gd` 括号路径）；
   - 核实并更正 `BUG-08`（`release_gate_verification_2026_11_21.md` 中的“语音触发”表述）。
3. **本机实机验收（`needs-runtime`）**：
   - 跑通 `bash tools/godot_check.sh`（38/38 无头门禁）与 **`LWsmf_RENDER=1 bash tools/godot_check.sh`**（补齐本轮缺失的 OpenGL3 渲染像素门禁）；
   - 在干净目录双击发布包，人工复测：夜晚读白天档（染色复位）、居中框读档、切窗失焦回窗点击、长按方向键滚回顾、`101+102` 同屏与 Tips 13 图。
