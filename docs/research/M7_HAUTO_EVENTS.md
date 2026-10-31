# M7 W2 — H 自动事件（run_condition==2）调度与可见性验证

## 结论

- **调度器已完整，无需修改 `crates/wolf-game/src/auto_events.rs`。** 全部 7 个 H 文字特效事件都被跟踪（operator=1 `>=`，variable=2_000_000+索引，value=1），门控变量一旦 `>=1`，下一帧 `sync()` 即 `vm.start_event(id)`。
- **文本层（`hword/word_effect*.png`）在本引擎中恒为 alpha 0：** 脚本每次绘制后立即以同号 move 把 opacity 置 0，而引擎把该 move 当瞬时传送（无 animate 帧），故不产生可见字形。要真正显示字形需改 `picture.rs`，超出本任务写权限（本任务只允许改写 auto_events.rs / 该测试 / 本文档）。
- **但“至少一个可见 H 文字/画面”已用事件 353（文字エフェクト挿入中）达成：** 调度后新增可见 H CG `hscene/h_s10_sonyuu01.png`（picture 1185，opacity 255），quad 数 40→41；同时重绘 hword 槽 4100/9300，证明文字绘制指令确实执行。

## 门控逻辑（代码级，数据证明 A）

- `auto_events.rs` 构造过滤：`!SKIPPED.contains(id) && (run_condition()==2 || (run_condition()==0 && GATED_CALL_ONLY.contains(id)))`；`SKIPPED=&[43]`，`GATED_CALL_ONLY=&[242,243]`（共 98 个事件 = 96 个 run==2 + 2 个门控）。
- `AutoEvent { operator, variable: raw(2_000_000+n), value }`。`sync()` 每帧在 `game.rs:1019`（`handle_choice_input()` 之前）执行，边沿触发：`now && !before` → `vm.start_event(id)`；`!now && before` → `vm.stop_instance`。
- `condition_value`: `index = raw - 2_000_000`；`if index < 800 { Some(vm.vars.get(index)) }`。
- `condition_met`: `1 => >=`，`2 => ==`，`3 => <=`。
- 7 个事件（真名取自 `wolf-cli --root work/all list`）：

| id | 真实标题 | 门控变量 | 普通索引 | run | op | value |
|----|----------|----------|----------|-----|----|-------|
| 350 | 文字エフェクトみぎ | 2000118 | 118 | 2 | 1 (>=) | 1 |
| 351 | 文字エフェクトひだり | 2000119 | 119 | 2 | 1 (>=) | 1 |
| 353 | 文字エフェクト挿入中 | 2000099 | 99 | 2 | 1 (>=) | 1 |
| 354 | 文字エフェクト射精 | 2000124 | 124 | 2 | 1 (>=) | 1 |
| 357 | 文字エフェクトアイテム | 2000062 | 62 | 2 | 1 (>=) | 1 |
| 358 | 文字エフェクト妹絶頂 | 2000123 | 123 | 2 | 1 (>=) | 1 |
| 367 | カットイン瞬き | 2000207 | 207 | 2 | 1 (>=) | 1 |

注：367 是“cut-in 瞬き”，不是文字事件；但它同为 run==2，一并纳入调度回归。

## 经验验证（work/all + SaveData08，数据证明 A）

- Harness：`Game::new(work/all)` + `auto_advance=true` + `auto_choice=true` + `load_save(work/retail_backup/Save/SaveData08.sav)` + `start_engine_events()`。
- 开机后 7 个 H 文字事件中只有 357（var62>=1）已调度，其余 6 个 `instance()==None`。
- 逐个 `vm.vars.set(index, 1)` + `frame()` 之后：7/7 全部 `instance()` 为 `Some` 且进入 `running_ids()`；此时 `running` 总数 18（含开机即运行的其它 auto 事件，并非只有这 7 个），`stats.warnings == 0`。
- 353 单独调度效果：baseline 56 张可见 picture / 40 quads → 57 / 41；新增 picture `1185 = hscene/h_s10_sonyuu01.png`，位置 (512,306)，scale 100，opacity 255；hword 槽 4100 `word_effect102→07`、9300 `word_effect115→26`。

## 为何文字层始终不可见（机制 + 行级证据，数据证明 A）

- 绘制示例 CE350 `pc26` op150 `ints=[4096,4000,0,1,1,1,255,1600010,1600011,100] "hword/word_effect38.png"`：opacity = `raw[6] = 255`。
- 紧随 `pc27` op150 `ints=[65537,4000,100,0,0,1,0,-10,10,150]`：同号 4000，`raw[6] = 0` → opacity 0。
- `crates/wolf-game/src/picture.rs:675` `Transition::from_command` 需 `cmd.animate.first()`（即 `args[11..]`）非 0 且 `<= MAX_TRANSITION_FRAMES`；此处 argc=10，无 animate → `None` → 无过渡，瞬时传送。
- `crates/wolf-game/src/picture.rs:1545` `merge_args` 用 `field::OPACITY`（=4=raw[6]，见 `picture.rs:146`）覆盖 `picture.opacity` → 0。
- `crates/wolf-game/src/picture.rs:890` `alpha()`：`fade>0` 时 `opacity * progress`，opacity=0 则恒为 0。
- 静态扫描确认：**每一个** word draw 都在任何 `op180` wait 之前被同号 move 停到 opacity 0（系统性，非个例）。
- 修复方向（**超出本任务写权限**，需 owner 决策）：把 cmd=1(move) 且 `raw[2]`(fade) != 0 解释为“时长 fade 的透明度过渡”而非瞬时；否则字形永不显示。

## 测试

`crates/wolf-game/tests/m7_hauto_events.rs`（两个用例）：

1. `scheduler_tracks_all_seven_h_text_events`：断言 7 个事件都被 `AutoEvents` 跟踪、门控变量/运算符/阈值正确；开机仅 357 live；逐个 set gate 后全部出现在 `running_ids` 中。**若 run==2 过滤 / sync 边沿触发 / condition_value 任一被回退，本用例立即失败 —— 即旧行为回归守卫。**
2. `scheduling_ev353_draws_h_text_and_a_visible_h_cg`：基线无 1185；设 `var99=1` 调度 353 后出现可见 CG 1185、quad 增加、hword 槽重绘。

- 运行：`WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_hauto_events`
- 结果：`2 passed; 0 failed`（约 1.08s），warnings=0。

## 数据证明 vs 推断

- **A（数据证明）**：7 事件 run==2 / op>= / value 1；调度 7/7；353 绘制 1185；词槽重绘；opacity0 机制与上述 picture.rs 行号。
- **B（推断）**：若把 move+fade 改为时间过渡，字形将可见；该改动未实装、未做像素级验证。
