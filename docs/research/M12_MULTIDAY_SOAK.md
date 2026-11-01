# M12 多日 soak：一个通用驱动连续推进 30/90 天

> 状态：**已验证（真实数据、真实菜单输入、无时间/日程注入）**
> 数据根：`work/all`
> 测试：`crates/wolf-game/tests/m12_multiday_soak.rs`（1 个默认测试 + 1 个 `#[ignore]` 长跑测试）
> 复现：
>
> ```bash
> cd '/e/Study/github program/imoto life'
> source tools/env.sh >/dev/null 2>&1
> WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m12_multiday_soak -- --nocapture
> # 90 天长跑（约 45 秒）：
> WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m12_multiday_soak -- --ignored --nocapture
> ```

## 1. 结论

不需要一天一个测试。`m12_multiday_soak.rs` 用一个**通用逐日驱动**处理屏幕上出现的任何零售菜单：

- **工作日**（CE251）：找第一个 CE252 工作行（有非战斗行时不点「地区贡献」），按零售 `230+50*row` 坐标点击；
- **周末**（CE310 ■休日）：体力 ≥30 时点「自主锻炼」，否则点「悠然自得」；按钮按文本图片自己的 `(x,y)` 点击；
- **剧情日/早安/战斗**：只喂 OK + `auto_choice`，让真实脚本、CE253/CE305 战斗和过场自己跑；
- **晚上**：真实晚餐后进入 18:30/19:30 卧室；只注入 `var63`（所有睡眠测试共用的点妹妹旗标），走真实 CE329 三选一「今天还是睡吧 → 是的」。

30 天默认测试的结果：

```
day 1 -> day 31 08:00+ (weekday 2)
battle_days   = [4, 11, 15, 16, 17, 18, 19, 22, 23, 24, 30]   (11 天)
weekend_days  = [6, 7, 13, 14, 20, 21, 27, 28]               (4 个完整周末，每个 3 次 CE310)
weekday_menu_days = [2, 3, 5, 8, 9, 10, 12]                   (7 天)
frames=65649  instructions=3183844  warnings=0
```

默认测试断言：day31 到达、weekday = `(day-1)%7`、day4 强制战斗、day11+ 的 CE305→CE239 战斗、每个周末都出现 CE310、工作日菜单重复出现、30 天内没有剧情跳天、全程 warnings=0。

## 2. 为什么之前会一天一天推进

逐日测试是为了钉住**边界**（day4 强制战斗、day5→day6 的 09:30、周末按钮几何）。这次改用通用驱动后发现两个此前未覆盖的行为：

1. **day11 的 CE305 → CE239 战斗会卡住**：重建驱动的 auto-arm 原来只认 CE253（地域貢献）。day11 的剧情由 CE305 キャンセナル 直接调 CE239，没有 CE253，所以 CE242 永远不启动。修复：`BattleWiring::real_battle_running` 现在也接受**有 parent 的 CE239**（脚本调用），而单元/探针测试直接 `start_event(239)` 得到 parent=None，保持原行为不变。见 `crates/wolf-game/src/battle_wiring.rs`。
2. **剧情会自行跳天**：90 天长跑在 day84 早晨从 CE317 直接跳到 **day97**（不是死锁）。通用驱动把「日计数器变化」视为一天完成并继续，不强行要求 +1。默认 30 天测试不会遇到这个跳转。

## 3. 90 天长跑（`#[ignore]`）

```
day 1 -> day 97 (scripted jump 84 -> 97)
battle_days  = 27 天
weekend_days = 23 天（每个周末 3 次 CE310）
weekday_menu_days = 7 天（后期工作日转为剧情/战斗）
frames=223752  instructions=10437869  warnings=0
```

耗时约 46 秒，因此默认不跑；用 `--ignored` 复现。它证明驱动能跨越剧情跳天、连续数十场战斗与周末循环而不死锁。

## 4. 驱动规则与边界

- 选择的是**安全行**：工作日只点已知 CE252 标签行，周末在体力低时改点休息；不点击「地区贡献/外出」这些可能进入额外战斗/城镇线。
- 体力数值受 PRNG/输入时序影响，soak 只记录不写死；日程/菜单/天数才是断言对象。
- 睡眠仍注入 `var63`（点妹妹旗标），与所有既有睡眠测试一致；没有注入时间、天数、体力、战斗结算或菜单状态。
- 剧情跳天被当作“完成一天”，因此长跑的最终天数可能大于目标（day84→97）；默认 30 天测试断言没有跳天。
- `#[ignore]` 长跑不计入默认门禁；默认门禁是 133 test-result / 812 passed / 2 ignored（含本文件的 ignored 测试）。

## 5. 生产代码变更

`crates/wolf-game/src/battle_wiring.rs`：

```rust
pub fn real_battle_running(vm: &Vm) -> bool {
    vm.instances().iter().any(|i| i.event == EVENT_BATTLE_DISPATCH)   // CE253
        || vm.instances().iter().any(|i| i.event == EVENT_BATTLE_CALLER && i.parent.is_some())
}
```

auto-arm 使用它；显式 `set_battle_wiring(true/false)` 仍然优先。直接 `start_event(239)` 的测试（m7_battle_full、m7_battle_round、m7_battle_target_scheduler 等）没有 parent，因此不会改变既有字节行为；完整 workspace 回归全绿。