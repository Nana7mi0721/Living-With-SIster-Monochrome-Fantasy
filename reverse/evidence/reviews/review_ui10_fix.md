# Review: UI-10 存档 HUD 恢复修复（task-11 独立复核）

审查对象：task-8（fix-game-state）的 UI-10 修复
审查方式：只读源码 + 运行定向测试 + 真实存档截图/日志复核
结论：**验证通过（无阻断性缺陷）**；有 1 个非阻断测试缺口、2 个流程/附带变更项待 Lead 确认。
对应 UI ID：**UI-10（行动力数值恢复/显示异常）**

---

## 0. 结论摘要

| 检查项 | 结论 | 证据 |
|---|---|---|
| 216/217 白名单是否只含 HUD 事件 | 已验证 | `auto_events.rs:93` 常量为 `[216, 217]`；`auto_events.rs:122-140` 逐事件用 `RESUME_ON_LOAD.contains(&event.id)` 赋值 |
| 载入后是否启动 216/217 | 已验证 | `savegame.rs:185` 在变量/图片恢复后调用 `prime_latched`；m12 测试通过；CLI 载入日志 `2 running [216, 217]` |
| 是否每帧重入 | 已验证 | `game.rs:1995` 每帧先 `auto_events.sync`；`auto_events.rs:261-275` 对白名单事件按“当前无 live 实例则启动”处理；5/10 帧启动计数分别为 10/20 次 |
| 是否重复启动 | 已验证（运行证据） | `prime_latched` 有 `already_running` 防护（`auto_events.rs:204-211`）；5 帧 10 次、10 帧 20 次启动，等价每事件每帧 1 次；没有 4 次/帧的倍增 |
| 506 眼罩是否被误启动 | 已验证 | m12 测试断言 506 不在 `running_ids` 且连续 3 帧无 506 实例/无 `dlc_imouto01_eye` 图片；m13 载入回归 8 帧也通过；CLI 载入日志只有 2 个 distinct 事件（216/217） |
| 睡眠后 HUD 是否与内部变量一致 | 已验证 | `ui10_sleep_save01_fixed2.png` 与独立复跑均显示 91日 9:30、120/120、6/6、60%、7586；m12 的 `hud_values` 断言全部通过 |
| 测试是否覆盖“重复载入/实际实例数” | **待定（测试缺口）** | m12 主要断言 `running_ids`；该值来自 scheduler 的 `running` map，不能证明 VM 中实际只有 1 个实例；未测试连续两次 load 后的实例计数 |

总评：修复方向正确，核心行为已由源码、m12/m13/auto_events 测试和真实存档截图共同验证。未发现 P0/P1 问题。建议补 1 个实例计数的回归断言后再合并。

---

## 1. 变更审查

### 1.1 白名单定义

- `crates/wolf-game/src/auto_events.rs:49-54`：`AutoEvent` 新增 `resume_on_load: bool` 字段，注释说明只用于 HUD 绘制页。
- `crates/wolf-game/src/auto_events.rs:79-93`：`RESUME_ON_LOAD` 常量为 `[216, 217]`，注释明确 216/217 是顶部 HUD/时钟，SavePart5 存的是渲染后的字面文本，因此读档后必须重放。
- `crates/wolf-game/src/auto_events.rs:122-140`：`from_common_events` 对每个事件执行 `resume_on_load: RESUME_ON_LOAD.contains(&event.id)`。这是白名单而不是按 `run_condition` 全量恢复，结构上不会把 506 或 337 纳入。

**已核对：** 常量只有 216/217；没有其他事件能在此路径得到 `resume_on_load == true`。  
**待定：** m12 只对 506 做点检，没有遍历 98 个 tracked 事件逐项断言 `!resume_on_load`；建议在 `auto_events.rs` 的单元测试中补全表断言（见 §4）。

### 1.2 载入时启动

- `crates/wolf-game/src/savegame.rs:176-185`：恢复普通变量、事件 self bank、屏幕图片后，在 `savegame.rs:185` 调用 `game.auto_events.prime_latched(&mut game.vm)`。
- `crates/wolf-game/src/auto_events.rs:193-216`：`prime_latched` 先为每个事件写入 `latched = now`；仅当 `now && event.resume_on_load` 才尝试启动。
- `crates/wolf-game/src/auto_events.rs:204-211`：`already_running` 同时检查 scheduler 记录的 instance id 和 `vm.instances()` 中同 event 的实例；若已存在则跳过，避免重复载入时叠加 HUD。
- `crates/wolf-game/src/auto_events.rs:212-214`：`vm.start_event(event.id)` 仅启动一次并记录 `running[event.id]`。

**已验证：** m12 `loading_save01_starts_the_hud_events_and_never_the_dlc_overlay` 通过；测试断言 216/217 出现在 `running_ids`、506 不在。  
**注意：** 此处“启动”不执行事件；第一帧 `sync` 看到 216/217 实例仍 live 时不会再启动，随后 `step_frame` 执行并 `reap_finished` 回收，下一帧再启动。时序正常。

### 1.3 每帧重入

- `crates/wolf-game/src/game.rs:1991-1995`：`Game::frame` 开始处先调用 `self.auto_events.sync(&mut self.vm)`。
- `crates/wolf-game/src/auto_events.rs:252-296`：`sync` 对每个事件计算 `now` / `before`。
- `crates/wolf-game/src/auto_events.rs:261-275`：白名单事件走独立分支：
  - `live` 定义为同 event 且 `pc < program.len()` 或 `WaitState != None`；
  - 没有 live 实例时 `start_event`，并覆盖 `running[event.id]`；
  - 注释明确“finished coroutine can remain until next step reaps it”，因此以 `pc < len` 判定结束；
  - `report.started` 只在实际启动时增加。
- `crates/wolf-vm/src/exec.rs:1346-1360`：`reap_finished` 每帧结束删除 `pc >= program.len() && wait == None && !retain_after_finish` 的实例，解释了下一帧能再次启动而不会累积。

**已验证：**  
1. `m12_ui_state` 2 个测试全部通过（见 §2）。  
2. 独立 CLI：载入 SaveData01 后 5 帧显示 `auto: 98 tracked ..., 2 running [216, 217]`、`background: ... 2 distinct common event(s) started ... 10 start(s) total`；10 帧为 `20 start(s) total`。说明 216/217 每帧各启动 1 次，而不是只启动一次，也没有每帧 2 次以上的重复。  
3. 独立 1600 帧睡眠复跑：最终文本为 `6200: 9:30`、`6211: 91`、`6235: 120/120`、`6236: 精力 6/6`、`6246: 60％`、`6251: 7586`，与变量一致。

### 1.4 506 及其他 run==2 事件的抑制

- `crates/wolf-game/src/auto_events.rs:91-92`：注释明确全量重放会重启 506（DLC 眼罩）与 337。
- `crates/wolf-game/src/auto_events.rs:201-203`：`if !now || !event.resume_on_load { continue; }`，非白名单事件不会在 `prime_latched` 启动。
- `crates/wolf-game/src/auto_events.rs:276-280`：非白名单事件仍走原来的 edge-triggered 分支，只有真正 `!before -> now` 才启动；因此 506 若在载入后发生真实边沿，仍能按原逻辑启动，不会被永久封死。
- `crates/wolf-game/src/auto_events.rs:281-285`：条件从 true 变 false 时只停止 scheduler 记录的实例。

**已验证：**
- `m12_ui_state.rs:111-151`：载入 SaveData01 后断言 506 实例不存在，连续 3 帧也不存在，且不出现 `dlc_imouto01_eye` 图片。
- `m13_auto_events_load.rs`：现有载入回归连续 8 帧检查 506 和眼罩图片，2 个测试通过。
- CLI 载入 SaveData01：`auto: 98 tracked ..., 2 running [216, 217]`；`2 distinct common event(s) started`，没有 506/337。
- `m7_hauto_events` 2 个测试通过，说明非白名单 event 的 edge 行为未回归。

### 1.5 附带变更（不属于 UI-10 主路径）

- `crates/wolf-game/src/auto_events.rs:17-35`、`:57-61`：同一 diff 还把事件 43 的模块注释从“死代码”改写为“标题菜单”，并在 `SKIPPED` 注释中说明由 `Game::boot`/`start_title` 显式启动；相关 `game.rs` 的 `TITLE_MENU_EVENT`/`start_title_menu`/`stop_title_menu` 改动不属于 UI-10。
- `crates/wolf-game/src/savegame.rs:472-481`：`decode_picture_text` 的 `size` 范围由 `1..=96` 改为 `0..=96`，允许 `\f[0]` 往返。对应新增测试 `a_zero_font_escape_survives_a_save_round_trip`（`savegame.rs:592-601`）。
- `savegame.rs` 的 savegame 单测共 10 个通过，未发现该改动影响 UI-10；但它与 UI-10 修复无直接关系，建议 Lead 在最终 diff 中确认其归属（可能是 task-8 一并带入的文本修复）。

---

## 2. 实际运行的测试

统一环境：`TEMP=$PWD/work/tmp TMP=$PWD/work/tmp`；游戏数据使用 `WOLF_DATA_ROOT=work/all`。

```text
cargo test -q -p wolf-game --test m12_ui_state -- --nocapture
=> 2 passed; 0 failed; 0 ignored; finished in 0.83s

cargo test -q -p wolf-game --lib auto_events
=> 6 passed; 0 failed; 0 ignored; 259 filtered out

cargo test -q -p wolf-game --test m13_auto_events_load
=> 2 passed; 0 failed; 0 ignored

cargo test -q -p wolf-game --test m7_hauto_events
=> 2 passed; 0 failed; 0 ignored

cargo test -q -p wolf-game --lib savegame
=> 10 passed; 0 failed; 0 ignored; 255 filtered out
```

**m12 测试覆盖：**
- `m12_ui_state.rs:111-151`：载入 SaveData01 后 216/217 被恢复、506 不启动；3 帧内检查 506 实例与眼罩图片。
- `m12_ui_state.rs:153-180`：载入 SaveData01，启动 CE329 睡眠，跑 1600 帧；用 `hud_values`（`:80-96`）解析 6235/6236/6246/6251，断言行动力/精力/性欲/金钱与 `vm.vars` 一致，且行动力不再是 75。

---

## 3. 截图核对

| 文件 | 观测值 |
|---|---|
| `work/out/reviews/ui10_sleep_save01_end.png`（修复前） | 91日 9:30；行动力 **75/120**；精力 **4/6**；性欲 **56%**；金钱 7586 |
| `work/out/reviews/ui10_sleep_save01_fixed2.png`（修复后） | 91日 9:30；行动力 **120/120**；精力 **6/6**；性欲 **60%**；金钱 7586 |
| `work/out/reviews/ui10_review_fix_verify.png`（本次独立复跑） | 同上：6200=9:30、6211=91、6235=120/120、6236=精力 6/6、6246=60％、6251=7586 |

独立复跑命令：

```bash
./target/debug/wolf-cli.exe --root release/data capture \
  --load-save work/retail_backup/Save/SaveData01.sav \
  --event 329 --frames 1600 --auto-input \
  --out work/out/reviews/ui10_review_fix_verify.png
```

该命令最后会写 `release/data/save/system.sav`；本次已从
`work/out/release_save_backup/system.sav` 恢复，md5 为
`564f3570677e10dfef2eae2f10aa69e8`，与备份一致。

---

## 4. 发现与建议

### 已验证，无需修改

1. **白名单确实只含 216/217**：`auto_events.rs:93` + `:138` 的结构性赋值可证明；不存在“按 run==2 全量恢复”的入口。
2. **载入启动一次**：`prime_latched` 的 `already_running` 防护有效；m12 与 CLI 日志均显示 216/217 各只有一条 scheduler 记录。
3. **每帧重入不会重复**：m12 通过；CLI 5 帧/10 帧的启动总数分别为 10/20，符合 2 个事件 × 帧数；1600 帧共 3214 次启动（216/217 约 3200 次 + 其他事件约 14 次），没有 2 倍膨胀。
4. **506 不会因载入误启动**：m12 3 帧、m13 8 帧的现有回归均通过；CLI 只启动 2 个 distinct 事件。

### 待定 / 建议补强（非阻断）

1. **m12 测试的“重复启动”断言偏弱（P2 测试缺口）**  
   `m12_ui_state.rs:146-150` 只断言 `game.auto_events.running_ids()` 包含 216/217；`running_ids` 来自 scheduler 的 `running` BTreeMap，事件自然结束后 entry 仍会保留，因此不能证明 VM 中实际只有 1 个实例。建议在 `:131-145` 的每帧循环里加：
   ```rust
   for id in [216, 217] {
       let count = game.vm.instances().iter().filter(|i| i.event == id).count();
       assert_eq!(count, 1, "event {id} must have one live instance (frame {frame})");
   }
   ```
   并增加“连续 load 两次”用例（加载后先跑 2 帧，再 `game.load_save(...)`，断言实例数仍为 1）。

2. **未逐项遍历所有 98 个 tracked 事件验证 `resume_on_load == false`（P2 测试缺口）**  
   源码结构上安全；建议在 `auto_events.rs` 新增测试：
   ```rust
   let flagged: Vec<u32> = auto.events().iter()
       .filter(|e| e.resume_on_load).map(|e| e.id).collect();
   assert_eq!(flagged, vec![216, 217]);
   ```
   这样未来改动 `RESUME_ON_LOAD` 时不会误把 506 等事件纳入。

3. **`m12_ui_state.rs` 当前为 untracked**  
   `git status --short -- crates/wolf-game/tests/m12_ui_state.rs` 输出 `?? ...`；若最终提交未显式纳入该文件，回归测试不会进入仓库。请 Lead/提交人确认。

4. **附带变更归属**  
   `savegame.rs:472-481` 的 `\f[0]` 解码修复与 UI-10 无直接关系；测试已通过，但建议在最终提交说明中单列，避免被误认为 UI-10 根因修复的一部分。

---

## 5. 最小修改建议（若需进一步加固）

- 仅在测试层补强，不需要改产品代码。
- 若希望把“scheduler 记录”与“实际 VM 实例”区分更清楚，可给 `AutoEvents` 增加只读诊断方法：
  ```rust
  pub fn live_instance_count(&self, vm: &Vm, event: u32) -> usize {
      vm.instances().iter().filter(|i| i.event == event).count()
  }
  ```
  但这是可选可读性改进，不影响当前行为。

---

## 6. 结论

**UI-10 修复通过独立复核。** 216/217 白名单精确；载入后启动一次；之后每帧重入且不重复；506/337 等 run==2 事件不会被误启动；睡眠后 HUD 与内部变量一致（120/120、6/6、60%、7586，91日 9:30）。  
**唯一实质待定项是测试强度：** m12 未直接断言 VM 实例数、未覆盖连续 load，也没有全表断言 `resume_on_load` 仅 216/217。建议补测试后再合并；当前产品行为未发现需要阻断的缺陷。