# Godot 4.7.2 Mono 主项目 8 项边缘问题根因重构方案报告（无冲突・斩草除根版）

**报告日期**：2026-11-27

**设计原则**：**拒绝表层打补丁，直击架构病根做结构性重构**；同时严格守住 `docs/godot/DECISIONS.md`（`D-01..D-93`）、2026-11-26 移动端四项交互优化（`MOB-01..04`）、Rust Oracle 基准（`crates/wolf-vm/src/exec.rs`，149,655 / 989,439 步对拍）以及已锁定 UI 锚点红线。

**隔离说明**：本报告为纯只读架构与重构方案设计，未修改 Godot 主仓库任何文件，也未纳入 `docs/ui_and_interaction_diagnosis_report.md`。

---

## 一、从“8 个散点补丁”收敛为“5 项根因架构重构”

逐项深挖这 8 个边缘问题在 Godot 源码中的真实成因后，可以发现它们并非 8 个孤立 Bug，而是**5 处底层结构债**的表象。与其打 8 个零散补丁，不如直接按模块一次性根治：

| 重构模块 | 根治的问题编号 | 底层结构病根 | 斩草除根的重构方向 | 必须守住的既有修复/契约（零冲突红线） |
| :--- | :---: | :--- | :--- | :--- |
| **重构 A：统一表现层富文本样式与排版核心** | **G-02、G-05、G-06** | `PictureStage`、`MessageWindow`、`ChoiceWindow` 各自手写一套 `TextTokenizer` 遍历状态机，导致 `<C>/<L>`、`\N` 描边与未展开 token 处理三处漂移 | 在 `TextTokenizer.gd` 之上新增单一权威 `TextStyleEngine.gd`，统一词法样式解释、行对齐与调试告警，三处表现层仅消费规范化排版结果 | `D-58`（选项清洗）、`D-72`（`TEXT_OUTLINE_SIZE=2` 默认描边）、`D-81`（2× 缩放）、`MOB-04`（一般选项框 `center.y=288`）、`DialogDrawTest`（行末 `<L>` 不盖本行 `<C>`）、不碰已锁定 UI 锚点 |
| **重构 B：`InputRouter` 显式 `TouchSession` 状态机** | **G-04** | 15+ 个松散触控/长按/双击字段散落在 5 个函数里手工同步，系统手势丢 `touch_up` 易残留孤儿状态 | 将全部触控跟踪字段收口进内聚 `TouchSession` 状态机（显式 `Phase` 枚举 + 单一 `reset_session()` + 同 `index` 重入自愈） | `MOB-01`（12 帧非对话长按读条）、`MOB-02`（`emulate_mouse_from_touch=false` 与触控守卫）、`D-87`/`D-89`（3 帧消费级 TTL 与经验式双击空白取消） |
| **重构 C：`AssetDB` 平台内存压力与生命周期策略** | **G-01** | `AssetDB.gd` 只管超预算淘汰，缺少平台级生命周期与内存压力治理接口，`GameRoot` 无法语义化联动 | 在 `AssetDB.gd` 内聚 `MemoryPolicy` 与 `on_app_paused()`/`on_low_memory()`/`trim_to()`，依赖 `RefCounted` 保活在屏资源并接入 `F6` 诊断 | `D-81`/`D-83`（桌面 512M / Android 128M / iOS 64M 分级预算、桌面切后台不丢缓存）、`D-89`（后台暂停音频与降帧） |
| **重构 D：`Instance.SelfBankPolicy` 一等策略 + `CoreRuntimeConfig` 集中配置** | **G-03、G-07** | `EventVm` 用旁路 `HashSet<int> _freshInstanceIds` 在 7 处手工同步实例持久化标记；核心 C# 散落 `Environment.GetEnvironmentVariable` 轮询 | 将持久化语义升级为 `Instance.SelfBankPolicy` 一等枚举彻底删掉 `_freshInstanceIds`；新增 `CoreRuntimeConfig` 一次性缓存只读调试开关 | Rust `exec.rs` `store_common_self`（`StopInstance` 必须保持回写以确保 149,655/989,439 步 Oracle 一致）、`D-29`（`CreateFreshInstance`）、`D-82`（`AutoEvents.Sync` 双模式） |
| **重构 E：全链路只读 `SaveAuditReport` 存档诊断管线** | **G-08** | 存档迁移记录、编码告警、零售 `var[302]` 触发、眼层清洗与他端坏档锁检测分散在多处，缺乏统一观测出口 | 建立结构化只读 `SaveAuditReport`，贯通 `SaveCodec` / `SaveMigrator` / `LegacySaveImporter` / `GameSession` / `F6` 诊断快照 | `D-25`/`D-35`（生产 `rejectBadChecksum=true` 且严禁静默篡改原版变量）、`D-74`（`F6` 诊断快照）、`D-88`/`D-90`（v2 `db_overlays` 与双向 `.sav`） |

---

## 二、五大根因重构方案详述

### 重构 A：统一表现层富文本样式与排版核心（根治 G-02、G-05、G-06）

#### 1. 病根剖析
当前表现层存在三套互不共享的 `TextTokenizer` 消费代码：
- `godot/presentation_gd/PictureStage.gd`（L488–595 `_measure_text`）：支持 `FONT_SIZE`/`COLOR`/`OUTLINE_ON/OFF`/`ALIGN_RIGHT`，但**漏掉 `ALIGN_CENTER`/`ALIGN_LEFT`**（G-02）。
- `godot/presentation_gd/MessageWindow.gd`（L355–432 `_layout`、L536–543 `_draw`）：完整支持 `<L>/<C>/<R>` 与 `\E`/`\N`，但 `_layout` 默认 `outline = false` 而 `_draw()` 又**无条件画描边**导致 `\N` 形同虚设（G-05）。
- `godot/presentation_gd/ChoiceWindow.gd`（L198–214 `_label_runs`）：仅处理 `TEXT`/`NEWLINE`/`FONT_SIZE`/`COLOR`，对未展开的 `VARIABLE`/`CDB` **静默丢弃无告警**（G-06）。

只要三处继续各自维护 `match token.kind`，后续每补一个 WOLF 文本标签都会再次出现“改了一处、漏了两处”的漂移。

#### 2. 斩草除根的重构设计
在 `godot/presentation_gd/` 新增纯函数式共享模块 `TextStyleEngine.gd`（`extends RefCounted`，零节点依赖、可单测），承担**所有表现层对 `TextTokenizer` token 流的唯一权威解释**：

1. **统一样式状态机 `parse_lines(source: String, options: Dictionary) -> Dictionary`**：
   - 接收显式契约参数：
     - `default_font_px: int`（`PictureStage` 传 `20`，`MessageWindow` 传 `base_size`，`ChoiceWindow` 传 `FONT_SIZE`）
     - `max_font_px: int`（`PictureStage`/`MessageWindow` 传 `96`，`ChoiceWindow` 传 `FONT_SIZE`）
     - `default_outline: bool`（**`MessageWindow` 显式传 `true`**，100% 守住 `D-72` 默认黑边基线，仅遇 `\N` 关闭；`PictureStage` 传 `false`，严格对齐原版 op150 文本层语义）
     - `flatten_newlines: bool`（`ChoiceWindow` 传 `true` 将 `\n` 折为空格，其余传 `false`）
     - `allow_right_tail_split: bool`（`PictureStage` 传 `true` 保留同行的 `Left<R>Right` 分栏能力）
   - 统一实现 `<L>`/`<C>`/`<R>` 规则（严格对齐 `DialogDrawTest.gd:155-164`）：仅在本行尚无可见字符时更新当前行对齐，行末 `<L>` 只复位后续行；当某行触发 `<R>` 分栏（`tail_from >= 0`）时不叠加 `<C>` 偏移。
   - 统一捕获未展开的 `VARIABLE` / `CDB` token：在结果字典中返回 `unexpanded_tokens: Array[String]`，由上层在非 headless 的 `OS.is_debug_build()` 下按源串去重输出一次 `push_warning`（绝不把未展开码画到屏幕上，100% 遵守 `D-58`）。
2. **三处表现层瘦身接入**：
   - `PictureStage._measure_text()` 直接调用 `TextStyleEngine.parse_lines(..., default_outline=false, allow_right_tail_split=true)`，对 `align == CENTER and tail_from < 0` 的行统一加 `(block_w - line_w) * 0.5` 的 X 偏移；**不改动 `_anchored_position()`、`size`（`block_w`）计算及 `1x1` 命中框**。
   - `MessageWindow._layout()` 复用同一套样式/对齐/描边状态（`default_outline=true`），`_draw()` 改为 `if bool(entry.get("outline", true)): draw_string_outline(...)`；普通对话与 `\E` 对话视觉 100% 零变化，`\N` 真正生效。
   - `ChoiceWindow._label_runs()` 直接复用 `TextStyleEngine.parse_lines(..., flatten_newlines=true)` 产出合并后的 runs，**不改动 `window_rect()` 与 `MOB-04`（`center.y = 288.0`）居中逻辑**。

#### 3. 冲突与回归验证
- 新增 `TextStyleEngine` 单元测试 + `DialogDrawTest.gd` / `PictureStateTest.gd` 回归断言（覆盖 `<C>`、`<C>Mid<L>\nLeftNext`、`Left<R>Right`、`\N` 关描边 + `\E` 复开、未展开 token 告警）。
- OpenGL3 渲染门禁 `DIALOG_DRAW_TEST_OK (windowed)` 与 `PICTURE_ROUNDTRIP_TEST_OK (windowed)` 全绿。

---

### 重构 B：`InputRouter.gd` 显式 `TouchSession` 状态机（根治 G-04）

#### 1. 病根剖析
`godot/presentation_gd/InputRouter.gd` 经过 `D-83`（纯触控手势）、`D-86`/`D-87`（经验式双击空白取消）、`D-89`（3 帧消费级 TTL 与双指下滑抽屉）以及 `MOB-01`/`MOB-02`（12 帧非对话长按读条与关闭触控模拟鼠标）多轮迭代后，触控状态已膨胀为 15+ 个散落字段（`_touches`、`_touch_max_fingers`、`_touch_peak_dist`、`_touch_gesture_consumed`、`_touch_swipe_locked`、`_touch_scroll_locked`、`_touch_scroll_accum`、`_touch_hold_active`、`_touch_pointer_held`、`_touch_pointer_hold_frames` 等）。
在安卓系统边缘手势截断丢 `InputEventScreenTouch(pressed=false)` 且未触发 `FOCUS_OUT` 时，散落字段容易残留孤儿触点，导致后续单指识别成双指或长按状态异常。而**如果简单打补丁加一个“2 秒无更新超时清理”，则会直接把 `MOB-01`（手指静止按住技能图标读条）和对话长按快进全打断**。

#### 2. 斩草除根的重构设计
在 `InputRouter.gd` 内部将所有触控会话状态收口为内聚的内部类 `TouchSession`（外部公共 API 签名 100% 不变，不影响 `GameRoot.gd` 与现有 30 项 `InputRouterTest`）：

1. **显式手势阶段枚举 `Phase`**：
   - `IDLE`：无活跃触点。
   - `TRACKING`：1–2 指按下，尚未触发任何手势或长按。
   - `DRAGGING`：已锁定滑动/滚轮/抽屉手势（取代 `_touch_swipe_locked` + `_touch_scroll_locked`）。
   - `POINTER_HOLD`：`MOB-01` 非对话长按持续确认持键（一等状态，**架构级禁止任何空闲超时驱逐**）。
   - `CONFIRM_HOLD`：对话长按快进（一等状态，**架构级禁止任何空闲超时驱逐**）。
   - `CONSUMED`：本次触控序列已被消费，等待全部手指抬起后归 `IDLE`。
2. **结构化触点记录 `TouchPoint`**：
   - 每个 `index` 携带 `start: Vector2`、`pos: Vector2`、`prev_pos: Vector2`、`max_dist: float`、`started_ms: int`、`updated_ms: int`。
3. **三条状态机不变式（根治孤儿触点且零冲突 `MOB-01/02`）**：
   - **不变式 1（同 `index` 重入原子自愈）**：收到 `st.pressed == true` 且 `points.has(st.index)` 时，100% 说明前一次 `touch_up` 已被系统吞掉；直接移除旧条目，若因此变空则调用 `reset_session()` 再登记新触点。
   - **不变式 2（仅在非持键态新按压时清扫陈旧孤儿）**：收到 `st.pressed == true` 时，仅当 `phase != POINTER_HOLD and phase != CONFIRM_HOLD`，扫描并清扫其他 `now_ms - pt.updated_ms > 15000`（15 秒无任何事件）的孤儿触点。正常单指长按技能或长按快进期间永远不会误杀。
   - **不变式 3（单一原子重置出口）**：`clear_touch_state()`、`FOCUS_OUT`、`NOTIFICATION_APPLICATION_PAUSED` 统一调用 `session.reset()`，原子归零 `phase`、`points` 与全部手势计数器，彻底杜绝漏清半截状态。
4. **严格保留的既有机制**：`project.godot` 的 `emulate_mouse_from_touch=false`、`GameRoot._release_modal_guard_if_free()` 的 `router.touch_mode` 守卫（`MOB-02`）、3 帧 `_touch_ok_ttl`/`_touch_cancel_ttl`（`D-89`）、经验式双击空白取消（`D-87`）均保持原语义不变。

#### 3. 冲突与回归验证
- 完整跑通 `InputRouterTest.gd` 全部 30 项（含第 30 项 `MOB-01` 长按 `EngineState(7)` `2 -> 1 -> 1 -> 0`）与 `GameRootTest.gd` `_test_mobile_4issues_regression`（`MOB-01..04`）。
- 新增 2 条状态机单测：① 同一 `index` 丢失 `touch_up` 后再次按下正常恢复单击与长按；② `POINTER_HOLD` 静止持续 20 秒以上不被任何清理逻辑误杀。

---

### 重构 C：`AssetDB.gd` 平台内存压力与生命周期治理策略（根治 G-01）

#### 1. 病根剖析
`AssetDB.gd` 目前仅按固定 `budget_bytes` 在写入新资源时被动 `_evict()`；`GameRoot.gd` 在 `NOTIFICATION_APPLICATION_PAUSED` 只管了音频与帧率，没有把“应用生命周期”与“资产缓存水位”打通，导致移动端切后台时 128 MiB / 64 MiB 贴图全部滞留，容易被 Android LMK / iOS Jetsam 杀进程。

#### 2. 斩草除根的重构设计
将内存压力治理完整内聚到 `AssetDB.gd`，避免在 `GameRoot.gd` 里散落平台判断和魔数除法：

1. **在 `AssetDB.gd` 确立平台 `MemoryPolicy`**：
   - 桌面端：`budget_bytes = 512 MiB`，`background_trim_ratio = 1.0`（切后台不裁剪，保住 PC 端 Alt-Tab 零重新解码体验）。
   - Android：`budget_bytes = 128 MiB`，`background_trim_ratio = 0.25`（切后台自动裁剪至 32 MiB 热点水位）。
   - iOS：`budget_bytes = 64 MiB`，`background_trim_ratio = 0.25`（切后台自动裁剪至 16 MiB，大幅降低 Jetsam 风险）。
2. **统一生命周期入口与安全裁剪**：
   - `trim_to(target_bytes: int) -> int`：按真 LRU 顺序从 `_cache`/`_sizes`/`_lru` 移除字典引用直至 `used_bytes <= target_bytes`（保留 `_lru.size() > 1` 仅用于常规预算，显式 `target_bytes == 0` 的紧急低内存允许清至 0）。因为 Godot 采用 `RefCounted`，**所有在屏 `Sprite2D`/`TextureRect` 与正在播放的 `AudioStream` 仍持有强引用，切回前台绝对不会出现白图、粉图或崩溃**。
   - `on_app_paused() -> int`：按 `background_trim_ratio` 自动决定是否裁剪并累加 `background_trims` 计数。
   - `on_low_memory() -> int`：立即执行 `trim_to(0)` 释放全部非在屏缓存。
3. **接入 `GameRoot.gd` 与 `F6` 诊断快照（`D-74`）**：
   - `GameRoot._notification` 在 `NOTIFICATION_APPLICATION_PAUSED` 仅调用一行 `if assets != null: assets.on_app_paused()`；若收到系统低内存通知则调用 `assets.on_low_memory()`。
   - `F6` 诊断快照新增 `AssetDB` 的 `used_bytes / budget_bytes / evictions / background_trims` 统计行，便于真机压测观测。

#### 3. 冲突与回归验证
- 新增 `AssetDB` 单元测试验证桌面策略 `on_app_paused()` 零淘汰、移动策略 `on_app_paused()` 降至 25% 水位、`on_low_memory()` 释放全部未挂载资源且在屏强引用纹理完整存活。

---

### 重构 D：`Instance.SelfBankPolicy` 一等持久化策略 + `CoreRuntimeConfig` 集中配置（根治 G-03、G-07）

#### 1. 病根剖析
- **G-03 病根**：`EventVm.cs` 通过外部旁路集合 `HashSet<int> _freshInstanceIds`（L129）标记某实例是否跳过 `PersistCommonSelf`，被迫在 `CreateFreshInstance`（L238）、`PersistCommonSelf`（L306）、`Reset`/`Load`（L1433/1443）、`StopInstance`（L1684）、`StopAll`（L1704）、`ReapFinished`（L1865）**7 处同步维护**。同时必须明确：Rust Oracle（`crates/wolf-vm/src/exec.rs:1075-1082, 1251-1260`）明确要求普通公共事件在 `StopInstance` 强杀时也执行 `store_common_self`——直接删掉强杀回写会破坏 149,655 / 989,439 步 Oracle 对拍！
- **G-07 病根**：`AutoEvents.cs:245`（`LWsmf_TRACE_AUTO`）、`WolfRng.cs:43`（`WOLF_LIVE_RNG`）各自调用 `Environment.GetEnvironmentVariable(...)`，前者在每次自动事件启停都会触发系统调用。

#### 2. 斩草除根的重构设计
1. **以 `Instance.SelfBankPolicy` 彻底消灭 `_freshInstanceIds` 旁路表**：
   - 在 `godot/core_cs/Instance.cs` 新增一等枚举与属性：
     ```csharp
     public enum SelfBankPolicy
     {
         /// <summary>WOLF / Rust Oracle default: fold CommonSelf back on both normal finish and StopInstance.</summary>
         PersistentAll = 0,
         /// <summary>Fold CommonSelf back only on normal completion; discard partial writes if killed by StopInstance.</summary>
         PersistentOnFinishOnly = 1,
         /// <summary>Ephemeral one-shot call (replaces _freshInstanceIds): start clean and never write back.</summary>
         Ephemeral = 2,
     }
     ```
   - `Instance` 持有 `public SelfBankPolicy SelfPolicy { get; init; } = SelfBankPolicy.PersistentAll;`。
   - **彻底删除 `EventVm._freshInstanceIds` 及其全部 7 处同步代码**。
   - `CreateFreshInstance` 直接构造 `SelfPolicy = SelfBankPolicy.Ephemeral`；所有标准 WOLF 事件默认 `SelfPolicy = SelfBankPolicy.PersistentAll`，**100% 保持与 Rust `exec.rs` `store_common_self` 逐帧一致**；宿主若未来需要“仅正常完成才回写”的受控调用，可直接选用 `PersistentOnFinishOnly`。
   - `PersistCommonSelf(Instance inst, bool isForcedStop)` 单点裁决：
     ```csharp
     if (inst.Program.SelfBase != Program.CommonSelfBase) return;
     if (inst.SelfPolicy == SelfBankPolicy.Ephemeral) return;
     if (isForcedStop && inst.SelfPolicy == SelfBankPolicy.PersistentOnFinishOnly) return;
     CommonSelf[inst.EventId] = inst.Self.Clone();
     ```
   - 并在 `CoreRuntimeConfig.TraceAutoEvents` 开启时，对同一 `EventId` 同时存在两个非 `Ephemeral` 活跃实例输出只读并发冲突提示。
2. **新增 `godot/core_cs/CoreRuntimeConfig.cs` 集中管理只读调试环境变量**：
   - 一次性静态缓存 `TraceAutoEvents`（`LWsmf_TRACE_AUTO == "1"`）与 `LiveRng`（`WOLF_LIVE_RNG`）；
   - `GameSessionController.cs:265` 的 `LWsmf_SAVE_DIR` 属于路径覆盖配置（测试可能按用例切换），保持显式方法读取不纳入静态只读缓存，避免测试隔离冲突。

#### 3. 冲突与回归验证
- `dotnet build godot/Lwsmf.csproj -c Release` 0 警告 0 错误。
- 38 项无头门禁全绿，特别是 `BATTLE_WIRING_TEST_OK`（`D-29` `CreateFreshInstance` 验证）与 149,655 / 989,439 步 M4/M13 Oracle 对拍零差异。

---

### 重构 E：全链路只读 `SaveAuditReport` 存档诊断管线（根治 G-08）

#### 1. 病根剖析
目前存档的诊断信息处于碎片化状态：`LegacySaveImporter` 有 `EncodingWarnings`，`SaveMigrator` 默默清洗了瞬态 UI 图层（`12000..12599`/`15000..16999`）、泄露的厕所眼层（`#-6`）和原版 `var[302] = 2` 触发，却没有结构化返回“本次读档/导入到底修复了什么、发现了什么潜在异常锁”，也没有接入 `D-74` 的 `F6` 现场诊断快照。同时，`D-25`/`D-35` 明确禁止像 Unity 1.0.10 那样在 `EnterRoom` 中静默篡改 `2000439`/`2000464`/`2000545`/`2000064`（会破坏原版“夜里在床上”状态与数据契约）。

#### 2. 斩草除根的重构设计
在 `godot/core_cs/` 建立统一的只读存档审计管线，**完全不修改原版游戏变量语义，只将全部清洗动作与异常锁状态透明化**：

1. **新增结构化只读模型 `SaveAuditReport`**（可放于 `SaveMigrator.cs` 或独立 `SaveAuditReport.cs`，并按 `D-88` 保持零反射 AOT 兼容）：
   - `SourceKind`（`NativeMain` / `NativeBackupFallback` / `RetailImport`）
   - `MigratedFromSchema`（例如 `1 -> 2`）
   - `StrippedTransientPictures: int`、`StrippedLeakedToiletEye: bool`、`StrippedTransientSe: int`
   - `RetailPostLoadTriggerApplied: bool`（是否命中 `var[302] = 2` 规范化）
   - `EncodingWarnings: List<string>`（透传 `D-25` 严格 UTF-8/CP932 告警）
   - `SuspiciousLocks: List<string>`（纯只读检测他端坏档残留锁：`var[545]!=0` 他操作中锁、`var[439]!=0` 图鉴锁、`var[464]!=0` 界面锁、`var[60]!=0 && var[127]!=0` 夜袭异常锁）
2. **统一接入点**：
   - `SaveMigrator.Normalize(SaveState state, SaveAuditReport? report = null)` 与 `ApplyRetailPostLoadTriggerIfNeeded(SaveState state, SaveAuditReport? report = null)` 保持原签名默认参数兼容，同步填充审计报告。
   - `GameSession` 保存 `public SaveAuditReport? LastSaveAudit { get; private set; }`，每次读档或 `ImportRetailSaveToSlot` / `LoadRetailSave` 自动更新。
   - `GameRoot` 的 `F6` 诊断快照（`user://diagnostic_snapshot.txt`）增加 `[SaveAudit]` 段落，一键打印最近一次读档/导入的迁移记录与异常锁清单。

#### 3. 冲突与回归验证
- 完整保留 `D-25`/`D-35` 生产严格 checksum 校验与零静默篡改原则、`D-88` `IsAotCompatible=true` 零反射编译与 `D-90` 双向 `.sav` 往返测试。
- 新增 `SaveMigrator` 单测验证 `SaveAuditReport` 准确记录清洗项与只读 `SuspiciousLocks`，且绝不修改 `vars[545]`/`vars[439]`/`vars[464]`/`vars[60]`/`vars[127]` 的值。

---

## 三、建议重构落地节奏与全量门禁清单

按耦合度由低到高分 **3 个独立可验证 PR/批次** 推进，每批均严守 `AGENTS.md` 双模式门禁（严禁裸跑 `godot --headless`）：

1. **批次 1（C# 确定性核心收敛）**：落地 **重构 D**（`Instance.SelfBankPolicy` + 删除 `_freshInstanceIds` + `CoreRuntimeConfig`）与 **重构 E**（`SaveAuditReport` 接入 `SaveMigrator` / `GameSession` / `F6`）。
2. **批次 2（表现层富文本管线收敛）**：落地 **重构 A**（`TextStyleEngine.gd` 统一 `PictureStage` / `MessageWindow` / `ChoiceWindow`）与 **重构 C**（`AssetDB.gd` `MemoryPolicy` + `on_app_paused`/`on_low_memory`）。
3. **批次 3（移动端触控状态机收敛）**：落地 **重构 B**（`InputRouter.gd` 显式 `TouchSession` 状态机），重点回归 `MOB-01..04` 与 `D-87`/`D-89`。
4. **每批必跑门禁**：
   - `dotnet build godot/Lwsmf.csproj -c Release`（0 warnings, 0 errors，验证 AOT 零反射）
   - `bash tools/godot_check.sh`（38 项无头门禁，含 `InputRouterTest` 30 项、`GameRootTest` `_test_mobile_4issues_regression`、`SaveImportTest`、M4/M13 Oracle 对拍）
   - `LWsmf_RENDER=1 bash tools/godot_check.sh`（OpenGL3 渲染回归门禁，含 `DIALOG_DRAW_TEST_OK (windowed)`、`PICTURE_ROUNDTRIP_TEST_OK (windowed)` 与 `PERFORMANCE_FRAME_STRICT_OK`）