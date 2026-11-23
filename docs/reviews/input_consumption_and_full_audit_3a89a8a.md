# Imouto-Life-Monochrome (`3a89a8a`) 输入消费与“实机操作无反应”全量静态排查报告

- **审查对象**：`Imouto-Life-Monochrome`（提交 `3a89a8a9d175d0dc00cdb4d18f8a39883b5aa30c`）
- **参照数据**：零售版解包字节码（`SampleMapA.mps` / `CommonEvent`）与上一轮审查基线 `bedd7a1`
- **审查范围**：全量排查 Rust（`wolf-game`、`wolf-vm`、`wolf-gdext`、`wolf-audio`）与 Godot（`GameView.gd`、`project.godot`）中所有可能导致**合法实机操作表现为无反应**的输入消费、事件吞噬、状态门控问题，并同步上报排查中发现的其他隐患与上一轮问题收口状态。

---

## 一、核心结论与背景复核

### 1.1 0.1.1 夜间恶作剧二级按钮（`390/415`）无反应的根因与现状
- **原问题**：在夜间恶作剧（`v100 = 2..12`）界面中，`SampleMapA` 的并行事件 `Ev 15 (コマンドリスト表示)` 先于 `Ev 16 (コマンド選択判定)` 执行 `opcode 123 a=[1100000, 33]`（`0x21` = 方向键 + 取消键，**不接受决定键 `OK`**）。旧版 `mouse_key_bits()` 只要被任意 `123` 调用就把全局 `mouse_consumed` 置为 `true`，导致同帧后执行的 `Ev 16`（轮询 `123 a=[1100001, 17]`）拿不到左键点击，二级按钮 `390/415` 无反应。
- **`3a89a8a` 的修复进展**：已在 `crates/wolf-game/src/game.rs:6029-6045` 增加 `mouse_click_for_poll(bits, query, banned)`，当且仅当 `code_for(bits, query, banned) != 0` 时才标记 `*self.mouse_consumed = true`，堵住了“不接受该按键的 `123` 仍吞点击”的最直接路径。
- **仍存在的隐患**：全量静态排查发现，代码库中仍有 **9 处同类/关联的输入消费与门控漏洞**（`1` 处 P0、`4` 处 P1、`4` 处 P2），以及 **3 处其他系统级隐患**，在多键位并发、多并行事件同帧轮询、过渡帧抢点、热区条件不满足等实机场景下依然会导致操作无反应。

### 1.2 问题清单速览

| 编号 | 严重度 | 所属模块与位置 | 问题类型 | 实机表现 |
| --- | --- | --- | --- | --- |
| **INP-01** | **P0** | `crates/wolf-game/src/game.rs:6014-6035, 5089-5100` | 单一全局布尔 `mouse_consumed` 跨键位连坐清空 + 多并行轮询器饿死 | 3 帧内右键接左键时左键被抹除；同帧多并行事件均轮询 `OK` 时后置按钮无反应 |
| **INP-02** | **P1** | `crates/wolf-game/src/game.rs:6868-6874, 6888-6896` | 键盘/`125` 自动键已命中时仍无条件调用 `mouse_key_bits()` | 按住方向键或脚本自动按键同帧点鼠标，点击被白白吞掉 |
| **INP-03** | **P1** | `crates/wolf-game/src/game.rs:4190-4215` (`4202`) | `picture_action_event_at` 命中热区后未确认事件启动即消费点击 | 快捷图标/映射热区当前页条件不满足时点击被吞，无法穿透到下层 |
| **INP-04** | **P1** | `crates/wolf-game/src/game.rs:4128-4132, 4583-4625` | `handle_action_key_input` 与 `handle_choice_input` 忽略 `pending_mouse` | 对话/过场刚结束 1~2 帧内抢点房间图标、妹妹立绘或选项框无反应 |
| **INP-05** | **P1** | `crates/wolf-game/src/game.rs:4612-4625` (`4619`) | `handle_choice_input` 在禁止取消的选项下仍无条件消费右键 | 禁止右键取消的选项框内按右键，事件被静默吞噬 |
| **INP-06** | **P2** | `crates/wolf-game/src/game.rs:4686-4731` | `apply_wheel()` 滚轮向 `key_bits` 注入 `UP/DOWN` 抢占鼠标点击 | 滚动滚轮同时点击鼠标时，方向键优先级压过决定键导致点击无效 |
| **INP-07** | **P2** | `crates/wolf-game/src/game.rs:4950-5006` | `sync_save_grid_cursor()` 槽位外左键直接转 `CANCEL` | 存档页叠加控件未拦截或点击微偏时误触关闭/无反应 |
| **INP-08** | **P2** | `godot/scripts/GameView.gd:950-965, 2918-2928` | `_modal_release_guard` 失焦清理与电平滞留边界 | 切出窗口或系统文件选择器异常返回后，回到游戏前几帧或持续点击无反应 |
| **INP-09** | **P2** | `crates/wolf-game/src/game.rs:170-173, 4153-4155, 4800-4810` | `story_input_locked()` 静态事件白名单一刀切屏蔽 | 列表内任一公共事件异常驻留或等待输入时，所有快捷图标与立绘点击失灵 |
| **OTH-01** | **P1** | `crates/wolf-game/src/game.rs:5301-5335` | `220 SaveLoad` 读档时 `resume_loaded_map()` 失败无保底恢复 | 异常存档读档后画面出现但无地图事件运行，全屏点击无反应（软锁） |
| **OTH-02** | **P2** | `crates/wolf-game/src/game.rs:6132-6186` | `frame_mouse_blocked` 同帧内只置 `true` 不随 `close_message` 复位 | 同帧瞬时开关消息框后，后续并行轮询仍被误挡 |
| **OTH-03** | **P2** | `godot/scripts/GameView.gd:4659+` & `game.rs` | `--log-input` 缺少 Rust 侧消费/拦截归因日志 | 实机复现“无反应”时难以快速判断是被哪一层门控吞掉 |

---

## 二、输入消费与“无反应”问题详细排查及修复方案

### [INP-01 / P0] `mouse_key_bits()` 单一全局布尔 `mouse_consumed` 导致跨键位连坐清空与多并行轮询器饿死

- **代码位置**：`crates/wolf-game/src/game.rs:6014-6035`、`crates/wolf-game/src/game.rs:5089-5100`
- **字节码证据（`SampleMapA.mps`）**：
  - 夜间恶作剧状态下，以下 4 个并行地图事件（`trigger = 2`）同时存活并在每帧执行 `opcode 123`：
    - `Ev 15 (コマンドリスト表示)`：`var100 == 2..12` 时每帧调用 `123 a=[1100000, 33]`（接受 `CANCEL | DIR`）。
    - `Ev 16 (コマンド選択判定)`：`var100 >= 1` 时包含 33 处 `124 hover -> 123 a=[1100001, 17]`（接受 `OK | DIR`）。
    - `Ev 18 (カーソル切り替え)`：`var100 >= 1` 时包含 12 处 `123 a=[1100001, 17]`（含 `pc=46/80/91` 等脱衣/身体热区分支，接受 `OK | DIR`）。
    - `Ev 26 (アイテムクリック監視)`：`var100 >= 1` 时包含 4 处 `124 hover(1112..1114) -> 123 a=[1100001, 17]`（接受 `OK | DIR`）。
- **根因分析**：
  1. **跨键位连坐清空**：`pending_mouse` 是 `u32` 位掩码（同时承载 `key::OK` 与 `key::CANCEL`），但 `mouse_consumed` 仅是 `bool`。当玩家在 `PENDING_MOUSE_TTL`（3 帧）内先右键取消、紧接着左键点击目标按钮时，`pending_mouse` 同时包含 `OK | CANCEL`。`Ev 15` 轮询 `0x21` 命中 `CANCEL` 后置 `*self.mouse_consumed = true`，随后同帧的 `Ev 16` 被 `if *self.mouse_consumed { return 0; }` 直接挡死，且帧末 `settle_pending_mouse()`（`5090-5092`）将整个 `self.pending_mouse = 0` 清空，连带抹除未被消费的 `OK`。
  2. **多并行轮询器单消费者饿死**：原版 WOLF 引擎中，非等待型 `123`（`!query.wait`）属于状态轮询（同帧内多个并行事件共享输入状态）。当前实现中，一旦两个并行事件（如 `Ev 16` 按钮热区与 `Ev 18` 身体/兜底分支、或 `Ev 26` 道具热区）在同帧都命中 `124` 并调用接受 `OK` 的 `123`，先调度的事件会直接把 `*self.mouse_consumed = true` 锁死，后调度的事件读到 `0` 而无反应。
- **修复措施**：
  1. 将 `mouse_consumed: bool` 改为按位掩码 `mouse_consumed_bits: u32`，并在 `settle_pending_mouse()` 中仅清除已消费位：
     ```rust
     // game.rs: settle_pending_mouse
     if self.mouse_consumed_bits != 0 {
         self.pending_mouse &= !self.mouse_consumed_bits;
         if self.pending_mouse == 0 {
             self.pending_mouse_age = 0;
         }
     } else if self.pending_mouse != 0 {
         self.pending_mouse_age += 1;
         if self.pending_mouse_age > PENDING_MOUSE_TTL {
             self.pending_mouse = 0;
             self.pending_mouse_age = 0;
         }
     }
     self.mouse_consumed_bits = 0;
     ```
  2. 在 `mouse_click_for_poll()` 中仅返回与当前 `query` 匹配且实际会生效的键位子集（如 `OK` 或 `CANCEL`），不要把未消费的另一位混入返回值；且仅在 `mouse_consumed_bits & matched_bit != 0` 时拦截同一键位。
  3. 对同帧**非等待型** `123`（`!query.wait`），将消费粒度从“全局一次性”改为“按 `InstanceId` 帧内去重 + 帧末清 `pending_mouse` 对应位”（与 `wolf-vm/src/exec.rs:2833` 的 `auto_keys_delivered` 机制保持一致）：
     - 同一并行事件在本帧内只拿到一次点击边缘；
     - 同帧内多个并行事件（如 `Ev 16` / `Ev 18` / `Ev 26`）互不饿死；
     - 只有真正消费掉交互的强独占入口（对话框 `message_dismissed`、选项框 `handle_choice_input`、地图动作 `handle_action_key_input`、等待型 `123` `query.wait == true`）才立即将对应位写入 `mouse_consumed_bits` 阻断后续事件。

---

### [INP-02 / P1] `key_code()` / `key_code_auto()` 在键盘或 `125` 自动键已满足轮询时仍吞掉鼠标点击

- **代码位置**：`crates/wolf-game/src/game.rs:6868-6874`、`crates/wolf-game/src/game.rs:6888-6896`
- **当前代码**：
  ```rust
  let held = base | self.mouse_key_bits(query, crate::key::from_wolf_bits(banned));
  crate::key::code_for(held, query, crate::key::from_wolf_bits(banned))
  ```
- **根因分析**：无论 `base`（键盘按键）或 `auto`（`opcode 125` 注入按键）是否已经能满足当前 `query`，Rust 都会先求值右侧的 `self.mouse_key_bits(...)`。一旦当帧同时存在有效鼠标边缘，即使最终 `code_for` 返回的是方向键或自动键，鼠标点击依然被标记为已消费并丢弃。
- **修复措施**：改为短路求值，先判定 `base | auto`，仅当其无法产生有效键码时才查询并消费鼠标：
  ```rust
  let banned_bits = crate::key::from_wolf_bits(banned);
  let base_bits = base | crate::key::from_wolf_bits(auto);
  let base_code = crate::key::code_for(base_bits, query, banned_bits);
  if base_code != 0 {
      return base_code;
  }
  let mouse_bits = self.mouse_key_bits(query, banned_bits);
  crate::key::code_for(mouse_bits, query, banned_bits)
  ```

---

### [INP-03 / P1] `handle_action_key_input()` 命中图片热区后未确认事件启动成功即消费点击

- **代码位置**：`crates/wolf-game/src/game.rs:4190-4215`（第 `4202` 行）
- **根因分析**：当 `picture_action_event_at(self.mouse)` 返回一个映射事件 ID 后，第 `4202` 行直接执行 `self.mouse_consumed = true;`，然后才调用 `self.runtime.start_event_by_id(...)`。如果目标地图事件当前页条件不满足（返回 `None`），点击已被提前吞噬，后续的 `123` 轮询或其它地图热区无法再接收到这次点击。
- **修复措施**：将 `self.mouse_consumed = true`（改造后为 `self.mouse_consumed_bits |= crate::key::OK`）移入 `if let Some(started) = ...` 成功启动分支内部；若 `start_event_by_id` 返回 `None`，不消费本次点击，允许继续按默认链路穿透。

---

### [INP-04 / P1] `handle_action_key_input()` 与 `handle_choice_input()` 忽略 `pending_mouse`，过渡帧抢点丢失

- **代码位置**：
  - `crates/wolf-game/src/game.rs:4128-4132`（`handle_action_key_input`）
  - `crates/wolf-game/src/game.rs:4583-4625`（`handle_choice_input`）
- **根因分析**：
  - `Game::frame()`（`5112` 行）将未消费的点击缓存进 `pending_mouse`（存活 3 帧），就是为了吸收玩家在对话结束、界面切换、事件过渡时的 1~2 帧提前点击。
  - 但目前仅 `mouse_key_bits()` 与 `message_dismissed()` 读取了 `pending_mouse`；`handle_action_key_input()` 和 `handle_choice_input()` 只检测当帧 `fresh_click`。因此在对话/过渡刚结束时点相册、保存、数据、妹妹立绘或刚弹出的选项，点击虽在 `pending_mouse` 里却不会被这两个入口响应，实机表现为“点早了一点就没反应”。
- **修复措施**：
  - 在 `handle_action_key_input()` 中，将点击条件改为：
    ```rust
    let left_edge = (self.mouse_buttons & 1 != 0 && self.previous_mouse_buttons & 1 == 0)
        || (self.pending_mouse & crate::key::OK != 0);
    let fresh_click = left_edge && !self.mouse_consumed && !self.frame_mouse_blocked;
    ```
  - 在 `handle_choice_input()` 中同理接纳 `pending_mouse` 的 `OK`（和允许取消时的 `CANCEL`），并在命中选项或取消后清零对应位。
  - **注意**：需确保对话框关闭的那一帧仍通过 `frame_mouse_blocked` 阻止同一点击穿透到刚出现的界面，仅让“对话已关闭后的过渡帧点击”被 `pending_mouse` 接住。

---

### [INP-05 / P1] `handle_choice_input()` 在禁止取消的选项框内仍无条件消费右键

- **代码位置**：`crates/wolf-game/src/game.rs:4612-4625`
- **根因分析**：第 `4619` 行在 `if self.choice_cancel_allowed()` 之外执行 `self.mouse_consumed = true;`，使得在不可取消的选项状态下右键点击被强制吞掉（且由于 `mouse_consumed` 是全局布尔，若与 `INP-01` 叠加还会连带阻断同帧左键）。
- **修复措施**：将消费语句移入 `if self.choice_cancel_allowed()` 内部，仅当右键实际触发了选项取消时才消费 `CANCEL` 位。

---

### [INP-06 / P2] `apply_wheel()` 滚轮向 `key_bits` 注入 `UP/DOWN` 可能抢占同帧鼠标点击

- **代码位置**：`crates/wolf-game/src/game.rs:4686-4731`
- **根因分析**：在非选项、非存档页且 `CE223` 未打开时，`apply_wheel()` 会直接执行 `self.key_bits |= UP/DOWN`。如果玩家边滚动滚轮边点击（触控板或鼠标中键误滚在实机非常常见），面对同时接受方向键和决定键的 `123` 轮询（如 `0x11`），`UP/DOWN` 键码会先于或替代 `OK` 被返回，配合 `INP-02` 会导致该帧左键点击无反应。
- **修复措施**：
  - 方案 A：在 `apply_wheel()` 中当本帧存在 `fresh_mouse` 左键边缘时跳过 `UP/DOWN` 合成；
  - 方案 B（更彻底）：将滚轮合成的 `UP/DOWN` 独立存放（如 `wheel_key_bits`），在 `key_code()` 中优先级低于真实键盘与真实鼠标点击边缘。

---

### [INP-07 / P2] `sync_save_grid_cursor()` 槽位外左键自动注入 `CANCEL` 依赖前端绝对拦截

- **代码位置**：`crates/wolf-game/src/game.rs:4950-5006`
- **根因分析**：存档界面中，凡是落在槽位区及 `[SAVE_NOTICE_RECT, SAVE_RETURN_RECT]` 以外的左键点击，都会在第 `5001` 行被 `inject_key_and_consume_click(CANCEL)` 转成取消退出。如果前端新增或调整存档导入/导出按钮位置、或某叠加控件未消费鼠标事件，点击会穿透到此处变成退出存档页或吞点击。
- **修复措施**：
  - 前端 `GameView.gd` 中所有存档页叠加按钮的点击处理必须调用 `get_viewport().set_input_as_handled()` 并在按下期间不写入 `mouse_down`/`mouse_latch`；
  - Rust 侧尽量收窄“点击空白关闭”的触发区，或仅在显式点击 `SAVE_RETURN_RECT` 及确认非叠加 UI 区时注入 `CANCEL`。

---

### [INP-08 / P2] `GameView.gd` 的 `_modal_release_guard` 在失焦/外部弹窗返回边界存在卡滞风险

- **代码位置**：`godot/scripts/GameView.gd:950-965`、`1091`、`1259`、`1595`、`2918-2928`
- **根因分析**：
  - 打开系统文件选择器或模态框时 `_modal_release_guard` 设为 `true`，在 `_physics_process` 中会屏蔽所有传给 Rust 的鼠标按键，直到 `_mouse_level_buttons() == 0 && mouse_down == 0 && mouse_latch == 0`。
  - `_notification` 在失焦（`FOCUS_OUT` / `PAUSED`）时调用了 `_clear_held_input()`，但如果 `_clear_held_input()` 未复位 `_modal_release_guard`，或 OS 在系统文件选择器关闭后瞬时上报陈旧的按钮电平，玩家回到游戏后的初次点击会被静默屏蔽。
- **修复措施**：
  - 在 `_clear_held_input()` 中同步将 `_modal_release_guard = false`；
  - 给 `_modal_release_guard` 增加一个短超时计数器（如连续 10 个物理帧无新按下事件自动解除），彻底消除永久卡死可能。

---

### [INP-09 / P2] `story_input_locked()` 静态白名单对快捷图标与立绘的一刀切屏蔽

- **代码位置**：`crates/wolf-game/src/game.rs:170-173`、`4153-4155`、`4800-4810`
- **根因分析**：只要 `STORY_COMMON_EVENTS`（`219, 220, 245..259, 283, 284, 290`）中任一非并行事件实例仍存活，整个 `handle_action_key_input()` 就会直接返回。其中 `CE249`、`CE251` 内部包含 `123` 等待轮询（`0x91`），若未来脚本或读档状态导致列表中任一事件异常残留，会造成房间内所有快捷图标与立绘点击完全无反应。
- **修复措施**：保留该保护逻辑的同时，在 `--log-input` 诊断信息中明确输出是哪一个 `STORY_COMMON_EVENTS` 实例触发了 `story_input_locked()`，并在读档 `220 SaveLoad` 清理链路中确认旧剧情协程已被正确回收。

---

## 三、全量摸排顺带发现的其他问题与基线核对

### 3.1 `bedd7a1` 静态审查问题的修复核对（`3a89a8a`）
经对比 `bedd7a1` 与 `3a89a8a` 的全量差异，上一轮高/中危问题已确认收口：
- **`wolf-audio/src/engine.rs`**：`self.mixer.lock().unwrap()` 已统一替换为 `self.lock()`，消除毒锁 panic 风险。
- **`wolf-gdext/src/ios_file_picker.mm`**：`NSURLIsDirectoryKey` 目录检查已移入 `startAccessingSecurityScopedResource` 权限作用域内，修复 iOS 文档提供程序误判问题。
- **`wolf-vm/src/exec.rs` / `wolf-game/src/savegame.rs`**：上一轮 CI 触发的 Clippy 告警（多余类型转换、可省略生命周期、参数数量等）已清理。

### 3.2 新发现 / 仍需关注的其他问题

1. **[OTH-01 / P1] `220 SaveLoad` 读档失败分支缺少地图事件保底恢复**
   - **位置**：`crates/wolf-game/src/game.rs:5324-5335`
   - **现象**：`resume_loaded_map()` 返回 `Err(err)` 时仅写入一条 `diagnostics` 计数，游戏画面仍停留在刚加载的立绘/背景上，但地图运行时没有任何活动事件，玩家此时点击任何区域都毫无反应（表现为“读档后假死”）。
   - **修复措施**：当 `resume_loaded_map()` 失败时，向前端弹出明确错误提示或尝试回退重载默认主房间地图事件，不要让玩家停留在“有画面但零事件响应”的静默状态。

2. **[OTH-02 / P2] `frame_mouse_blocked` 同帧只增不减**
   - **位置**：`crates/wolf-game/src/game.rs:6138`、`6178-6185`
   - **现象**：`show_message()` 和 `show_choices()` 会把 `self.frame_mouse_blocked = true`，但 `close_message()` 并未区分“本帧开启并已关闭”与“仍活跃”，在极端脚本同帧闪现/关闭提示的场景下会多阻断同帧剩余轮询。
   - **修复措施**：`mouse_key_bits()` 已经同时检查 `!self.messages.is_empty() || self.choices.is_some()`，可将 `frame_mouse_blocked` 明确收窄为仅用于“本帧刚关闭侧边栏/刚由点击关闭上一条消息，防止同一次点击穿透”的语义，避免和当前活跃窗口状态混用。

3. **[OTH-03 / P2] 缺少“输入被谁吞掉”的可观测诊断字段**
   - **位置**：`crates/wolf-game/src/game.rs`、`godot/scripts/GameView.gd:4659+`
   - **现象**：目前 `--log-input` 无法区分某次点击究竟是未到达 Rust、被 `_modal_release_guard` 拦截、被 `story_input_locked` 拦下、还是被某个 `Ev` 的 `123` 提前消费。
   - **修复措施**：在 Rust 侧维护最近一次点击的 `last_click_disposition`（如 `consumed_by_ev(id)` / `blocked_by_story(ce_id)` / `blocked_by_message` / `unconsumed`），在开启 `--log-input` 时打印一行结构化日志，可将此类“无反应”问题的定位时间从小时级降到秒级。

---

## 四、推荐修复优先级

1. **立即修复（阻断实机无反应）**：`INP-01`（按位消费 + 非等待 `123` 按实例去重）→ `INP-02`（`key_code` 短路避免键盘吞鼠标）→ `INP-03`（热区未启动事件时不消费点击）。
2. **高优修复（消除手感掉帧/抢点失灵）**：`INP-04`（`handle_action_key_input` 与 `handle_choice_input` 接纳 `pending_mouse`）→ `INP-05`（不可取消选项不吞右键）→ `OTH-01`（读档恢复地图失败保底）。
3. **中优加固（边缘场景与可观测性）**：`INP-06`、`INP-07`、`INP-08`、`INP-09`、`OTH-02`、`OTH-03`。
