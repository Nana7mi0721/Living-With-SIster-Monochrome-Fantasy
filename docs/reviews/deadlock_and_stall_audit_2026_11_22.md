# LWSMF 全链路死锁、假死与状态滞留排查报告（Commit `2c57740`）

**审阅基线**：`Living-With-SIster-Monochrome-Fantasy` commit `2c57740`（`godot/core_cs/` + `godot/presentation/` + `godot/presentation_gd/`）  
**排查范围**：键鼠输入管线、地图并行/自动事件调度、VM 协程等待状态、预约事件队列（`opcode 211`）、读档与切图重置、战斗状态机、长期运行集合增长  
**验证边界**：静态源码级实锤与实机 `F3` 截图互证；以下涉及实机重测的验收项标注为 `needs-runtime`

## 1. 总览：发现 7 项引擎层死锁/滞留缺陷与 1 项原版脚本门锁

| 编号 | 级别 | 所属管线 | 核心位置（`file:line`） | 典型表现 |
| :--- | :--- | :--- | :--- | :--- |
| **D-01** | **P0** | 地图并行事件 × 输入抑制 × 预约队列 | `godot/core_cs/MapRuntime.cs:341-345` | **夜晚卧室键鼠全灭**；左键/回车被并行事件每帧自杀式清零；`opcode 211` 预约事件永久饿死 |
| **D-02** | **P0** | 输入抑制状态机 | `godot/presentation_gd/InputRouter.gd:238-261` | 快点/短按回车触发新事件后，`_action_entry_suppressed` 漏释放，吞掉下一次左键/回车 |
| **D-03** | **P1** | 读档 × `opcode 126` 禁键 × 预约队列 | `godot/core_cs/GameSession.cs:956-981`、`VmController.cs:967-971` | 禁键过场中按 `F9` 或界面读档后，`BasicAllDeny` / `DeviceDeny` 残留锁死新档输入；旧档预约事件串台 |
| **D-04** | **P1** | 协程强杀 × 对话/选项等待同步 | `godot/core_cs/EventVm.cs:1594-1633`、`VmController.cs:344-345` | 换页或自动事件停机强杀正在等消息/选项的协程后，`_isWaitingMessage`/`_isWaitingChoice` 永不归零，界面残留卡死 |
| **D-05** | **P1** | 战斗调度状态机 | `godot/core_cs/BattleWiring.cs:35-48, 120-149, 171-228` | 读档、二战或返回标题后 `_menuCursor` / `_harvestedRows` / `_prevBattleActive` 未重置，导致新战斗指令拒收或首回合不启动 |
| **D-06** | **P1** | VM 单帧预算调度 | `godot/core_cs/EventVm.cs:1517, 1694-1709` | 单帧固定 200,000 步且按固定下标遍历，头部异常循环会永久饿死后续并行/输入监听协程 |
| **D-07** | **P2** | VM 实例追踪集合 | `godot/core_cs/EventVm.cs:120, 1625, 1702, 1734` | `_enteredInstanceIds` 短路顺序写反且回收未删除，41,868 帧已泄漏 33.8 万个死实例 ID |
| **D-08** | **P2** | 原版脚本门锁（非引擎 Bug） | `docs/research/M6_NIGHT_FLOW.md:10, 35-43` | 首日晚餐 `CE326` 置 `var295 = 1`，未开关一次状态/技能栏前，夜晚点妹妹会被 `ev4 page0` 直接打断 |

---

## 2. P0 级致命死锁详情与修复

### D-01：地图并行事件漏设 `inst.IsParallel`，引发“每帧自杀式吞键 + `211` 预约饿死”（本次夜晚卧室真凶）

* **现场实证（用户 `F3` 截图）**：
  `Frame=41868 Phase=bedroom` 下可见 7 个 `SampleMapA` 并行地图事件每帧重建（实例 ID 已涨至 `338653..338659`）：
  `1:338653@0 | 13:338654@0 | 15:338655@0 | 25:338656@0 | 29:338657@0 | 34:338658@0 | 35:338659@0`。
* **源码根因**：
  `godot/core_cs/MapRuntime.cs:341-345` 创建自动/并行地图事件实例时，**未给并行页设置 `inst.IsParallel = true`**（对比 `AutoEvents.cs:115` 和 `GameSession.cs:229` 均显式设置了 `inst.IsParallel = true`）：
  ```csharp
  var inst = vm.CreateInstance(page.Program);
  if (inst == null) continue;
  inst.RetainAfterFinish = true; // 缺：inst.IsParallel = page.Trigger == MapTrigger.Parallel;
  ```
* **三重连锁死锁**：
  1. **吞杀所有左键点击与回车**：因 `IsParallel == false`，`EventVm.RunInstances()`（`EventVm.cs:1702`）每帧在 `StepInstance()` 执行 `pc 0` 之前抢先发出 `Host.EventEntered()` → `GameRoot._on_event_entered()`（`GameRoot.gd:334-341`）检测到 `router.action_input_held() == true` 后，立刻执行 `router.suppress_action_until_release()`（屏蔽 `MASK_OK`）和 `vm.SetPointerHeld(false)`（清零左键按下/边沿标志）。1 微秒后 `event 1`（`妹クリック判定（夜`）及图标监听事件真正执行时，读到的左键和回车恒为 `0`。
  2. **`opcode 211` 预约事件永久饿死**：`VmController.StepFrame()`（`VmController.cs:244`）在帧末调用 `Map?.StartPending(Vm)` 刚建出这 7 个 `Pc=0, IsParallel=false` 的实例；紧接着 `GameSession.AdvanceFrame()`（`GameSession.cs:280`）调用 `_vm.StartReservedEvent()`（`VmController.cs:1061-1064`），其 `if (Vm.ForegroundBusy()) return;` 永远返回 `true`，导致卧室期间所有预约公共事件被永久堵死。
  3. **前景事件 ID 污染**：`EventVm.ForegroundEventId()`（`EventVm.cs:249-259`）会把它们误报给 `op124 0x3000 field 18`。
* **最小修复**：
  在 `godot/core_cs/MapRuntime.cs:343` 前增加：
  ```csharp
  inst.IsParallel = page.Trigger == MapTrigger.Parallel;
  ```

### D-02：`InputRouter._action_entry_suppressed` 帧末漏释放，快点/短按回车永久吞掉下一次确认

* **源码根因**：
  * `godot/presentation_gd/InputRouter.gd:238-243` 中，`action_input_held()` 把单帧锁存 `_click_latch` 也判定为“按键仍按住”。
  * 当玩家在单帧内完成按下+松开（或松开事件早于 `advance_frame()`），`pointer_pressed` 已为 `false` 但 `_click_latch` 仍为 `true`。此时进入任意前台事件触发 `GameRoot._on_event_entered()`（`GameRoot.gd:334-341`），会调用 `router.suppress_action_until_release()` 置 `_action_entry_suppressed = true`。
  * 帧尾 `InputRouter.clear_frame_latches()`（`InputRouter.gd:256-261`）只清了 `_click_latch`，**未调用 `_release_action_suppression_if_free()`**；`GameRoot.advance_frame()`（`GameRoot.gd:276`）也只调用了 `_release_modal_guard_if_free()`。因为松开事件已经过去，`_action_entry_suppressed` 停留在 `true`，持续拦截 `held_mask()` 的 `MASK_OK`（`:169-170`）与 `left_frame_pressed()`（`:207`），直到玩家再完整按下并松开一次左键或回车。
* **最小修复**：
  1. 在 `godot/presentation_gd/InputRouter.gd:261` 的 `clear_frame_latches()` 末尾补一行 `_release_action_suppression_if_free()`。
  2. 在 `godot/presentation_gd/GameRoot.gd:276` 的 `_release_modal_guard_if_free()` 后同步调用 `router.release_action_suppression_if_free()`（将该方法公开，去掉下划线前缀）。

---

## 3. P1 级高危状态滞留与跨场景死锁

### D-03：普通读档与 `F9` 快速读档未清除 `opcode 126` 禁键策略和 `opcode 211` 预约队列

* **源码根因**：
  * `GameSession.StartTitle()`（`GameSession.cs:207`）调用了 `_vm.Vm.ResetInputPermissions()`；`LoadRetailSave()`（`GameSession.cs:1047`）调用了 `_vm.ResetSessionState()`（内部清除了输入权限与 `_reservedEvents`）。
  * 但**普通槽位读档与 `F9` 快速读档走的 `ApplyLoadedState()`（`GameSession.cs:956-1002`）完全漏掉了 `_vm.Vm.ResetInputPermissions()`**，且只调用了 `_vm.Vm.StopAll()` 而非 `_vm.StopAll()`；同时 `VmController.StopAll()`（`VmController.cs:967-971`）本身也未清空 `_reservedEvents`。
* **死锁路径**：
  1. 玩家在任意执行了 `opcode 126` 禁键（`BasicAllDeny != 0` 或 `DeviceDeny == true`）的事件中按 `F9` 读档或从界面读档 → 旧档的 `BasicAllDeny`/`DeviceDeny` 残留到新档 → 新档进入后回车、右键、移动与 `op123` 全部失效。
  2. 旧档尚未出队的 `opcode 211` 预约事件残留在 `_reservedEvents` 中，新档读入后第一帧前台空闲时突然弹出旧档事件，破坏状态或卡死流程。
* **最小修复**：
  * 在 `GameSession.ApplyLoadedState()`（`GameSession.cs:958-960`）中改调 `_vm.StopAll()` 并显式调用 `_vm.Vm.ResetInputPermissions()`。
  * 在 `VmController.StopAll()`（`VmController.cs:967-971`）中补一行 `_reservedEvents.Clear();`。

### D-04：`StopInstance()` 强杀正在等待消息/选项的协程时，宿主等待标志未同步清除

* **源码根因**：
  * `EventVm.StopInstance()`（`EventVm.cs:1594-1633`）在地图事件换页（`MapRuntime.cs:321`）或 `AutoEvents` 条件失效（`AutoEvents.cs:126`）时会连带杀掉该协程及其子协程，但**不会通知 `VmController` 复位 `_isWaitingMessage` / `_isWaitingChoice`**。
  * `VmController.IsWaitingForChoice()` 与 `IsWaitingForMessage()`（`VmController.cs:344-345`）直接返回缓存布尔字段 `_isWaitingChoice` / `_isWaitingMessage`，并未校验 `Vm.Instances` 中是否真的还有协程处于 `WaitStateKind.Choice` / `WaitStateKind.Message`。
  * 因此 `GameSession.AdvanceFrame()`（`GameSession.cs:292-301`）的自动关闭兜底逻辑（`if (_hasChoices && !_vm.IsWaitingForChoice())` / `if (_messageOpen && !_vm.IsWaitingForMessage())`）永远不会触发，导致旧对话框或选项框残留在屏幕上并继续拦截键鼠输入（`GameRoot.gd:359-382`）。
* **最小修复**：
  将 `VmController.cs:344-345` 改为直接以 VM 实时协程状态为准（或与布尔值做交集）：
  ```csharp
  public bool IsWaitingForChoice() => _isWaitingChoice && Vm.Instances.Any(i => i.Wait.Kind == WaitStateKind.Choice);
  public bool IsWaitingForMessage() => _isWaitingMessage && Vm.Instances.Any(i => i.Wait.Kind == WaitStateKind.Message);
  ```
  并在检测到无对应等待协程时顺手复位 `_isWaitingChoice = false` / `_isWaitingMessage = false` 及 `_pendingChoiceReply = null`。

### D-05：`BattleWiring` 单例在读档、二战或返回标题后永不重置

* **源码根因**：
  * `GameSession.cs:94` 持有会话级单例 `_battleWiring = new BattleWiring()`，但 `BattleWiring`（`BattleWiring.cs:35-48`）**没有 `Reset()` 方法**，且 `StartTitle()`、`ApplyLoadedState()`、`LoadRetailSave()`、`ApplyTeleport()` 均未重置它。
  * `BattleWiring.HarvestMenuRegistrations()`（`BattleWiring.cs:120-149`）用 `_menuCursor` 和 `_harvestedRows` 跟踪 `CData 18/73/0` 游标；一旦读档回到战斗前或开启第二场战斗（游标回到 `1`），`if (cursor <= _menuCursor) return 0;`（`:125`）与 `_harvestedRows.Contains(row)`（`:132`）会**直接跳过新战斗的指令采集**。
  * 若在战斗中读档，`_prevBattleActive`（`:181-183`）仍为 `true`，导致新档战斗首帧 `rising == false`，非 `roundLoop` 模式下首回合指令永不启动。
* **最小修复**：
  为 `BattleWiring` 增加 `Reset()`（清空 `_prevBattleActive`、`_cursor`、`_menuCursor = -1`、`_harvestedRows`、`_menuQueue`、`_menuSelections`、`StartedRounds`、`TurnStarts`、`_sawDispatch`、`_sawCaller`），并在 `StartTitle()`、`ApplyLoadedState()`、`LoadRetailSave()` 以及 `battleActive` 从 `true` 变为 `false`（战斗结束）时调用。

### D-06：`EventVm` 单帧预算耗尽无轮转调度，头部异常循环会永久饿死后续协程

* **源码根因**：
  `EventVm.StepFrame()`（`EventVm.cs:1517`）设置固定单帧预算 `budget = 200_000`，`RunInstances()`（`EventVm.cs:1694-1709`）每帧固定从索引 `0` 开始顺序执行。若前部某个协程出现缺少 `Wait 1 frame` 的条件循环或异常死循环，耗尽 200,000 步后会直接 `break`，排在它后面的并行事件、UI 刷新事件和输入轮询协程每一帧都分不到 1 步执行机会，且控制台无任何告警。
* **最小修复**：
  在 `RunInstances()` 中对每个实例设置单帧步数上限（如 `perInstanceBudget = 50_000`）并在触发上限时通过 `Host.Log` 输出带 `EventId`/`Pc` 的告警，确保单个异常协程无法饿死全图并行与输入协程。

---

## 4. P2 级长期退化与原版脚本门锁

### D-07：`EventVm._enteredInstanceIds` 短路顺序写反且回收漏删，造成持续内存泄漏

* **源码根因**：
  1. `EventVm.cs:1702` 写作 `if (_enteredInstanceIds.Add(inst.Id) && !inst.IsParallel)`——`Add` 在短路逻辑左侧，即使修复 D-01，每帧 7 个并行地图事件依然会继续把新实例 ID 写入 `_enteredInstanceIds`。
  2. `StopInstance()`（`EventVm.cs:1625`）与 `ReapFinished()`（`EventVm.cs:1734`）只清理了 `_freshInstanceIds.Remove(id)`，从未清理 `_enteredInstanceIds`（只在 `StopAll()` `:1641` 清空）。实机 41,868 帧已堆积 338,659 个无效 ID。
* **最小修复**：
  * 将 `EventVm.cs:1702` 改为 `if (!inst.IsParallel && _enteredInstanceIds.Add(inst.Id))`。
  * 在 `StopInstance()`（`:1625`）与 `ReapFinished()`（`:1734`）同步调用 `_enteredInstanceIds.Remove(...)`。

### D-08（原版脚本门锁，非引擎死锁）：首日晚餐 `var295 == 1` 技能 TIPS 锁死夜晚菜单

* **机制说明**（`docs/research/M6_NIGHT_FLOW.md:10, 35-43`）：
  首日晚餐 `CE326 pc1648` 会置 `var295 = 1`；在玩家点开并关闭一次状态/技能界面（`var295: 1 → 2 → 0`）之前，`SampleMapA ev4 page0 pc1` 会直接执行 `var68 = 0; BreakEvent`，表现为点妹妹无反应。
* **可观测性加固**：
  建议在 `PlayOverlays.gd:593-600` 的 `F3` 调试面板增加一行状态输出：
  `Deny=basic:%d/dev:%s  Guard=modal:%s/act:%s  v295=%d`，以后遇到任何“点不动/按不动”，按一下 `F3` 即可 1 秒区分是引擎锁死还是原版脚本门锁。

---

## 5. 推荐修复顺序与回归验收清单

| 优先级 | 修复项 | 验收方式 |
| :--- | :--- | :--- |
| **1（立即）** | **D-01**（`MapRuntime.cs:343`）+ **D-02**（`InputRouter.gd:261`、`GameRoot.gd:276`）+ **D-07**（`EventVm.cs:1625, 1702, 1734`） | 实机进入夜晚卧室，`F3` 确认点击妹妹（`pic120`）、图标及回车/右键立即响应；挂机 1000 帧 `_enteredInstanceIds.Count` 不再增长（`needs-runtime`） |
| **2（紧随）** | **D-03**（`GameSession.cs:958`、`VmController.cs:970`）+ **D-04**（`VmController.cs:344-345`） | 在过场禁键期间按 `F9` 读档，验证读档后键鼠立即可用；在对话/选项打开时触发地图换页或读档，验证无残留文本框（`needs-runtime`） |
| **3（战斗与长跑）** | **D-05**（`BattleWiring.Reset()`）+ **D-06**（单实例预算上限与告警）+ **D-08**（`F3` 面板补 `Deny/Guard/v295`） | 连续打完两场战斗及战斗中 `F9` 读档重打，验证指令登记与回合推进不卡死（`needs-runtime`） |
