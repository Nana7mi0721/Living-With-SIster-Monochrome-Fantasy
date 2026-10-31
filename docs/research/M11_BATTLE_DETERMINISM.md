# M11 战斗确定性证明与锁定（task-79）

> 结论：**自然 day4/连续路线战斗是逐字节确定的**。全仓库已无运行期迭代顺序不确定性——`started_events`
> 早已是 `BTreeSet`（非 `HashSet`），实例是 `Vec`（生成顺序），所有 `HashMap` 仅作键查表、从不
> 迭代。连续 5 次进程重跑 `m9_battle_round_gap` 与 `m9_full_route` 的轨迹逐字节一致。
> 因此把两个测试从「结构断言 / v352∈{1,-1}」收紧为**定值断言**，并删除 HANDOFF_M10/PLAN 里过时的
> 「HashSet RandomState 抖动」说法。不涉及任何 `src` 改动。

- 仓库：`E:/Study/github program/imoto life`，基线 HEAD `71803f9`。
- 数据根：`work/all`（`WOLF_DATA_ROOT` 缺省解析到 `<repo>/work/all`）。
- 写范围：仅 `crates/wolf-game/tests/m9_battle_round_gap.rs`、`crates/wolf-game/tests/m9_full_route.rs`、
  `docs/HANDOFF_M10.md`、`PLAN.md`、`docs/research/M11_BATTLE_DETERMINISM.md`。未改动 `src`，未 commit/push。

## 1. 迭代顺序审计（为什么运行期是确定的）

战斗/RNG 消费路径上所有「多元素容器」的遍历顺序都是显式确定的：

| 位置 | 结构 | 是否影响顺序 |
| --- | --- | --- |
| `crates/wolf-vm/src/exec.rs:806` | `pub started_events: BTreeSet<EventId>` | 有序集合；仅做「事件是否已启动」成员判定（初始化 :826，insert :885），不产生顺序 |
| `crates/wolf-vm/src/exec.rs:791` | `instances: Vec<Instance>`（注释「running coroutines, in spawn order」） | 按生成顺序；RNG/帧执行顺序完全由它决定，确定 |
| `crates/wolf-vm/src/program.rs:230` | `labels: HashMap<String,usize>` | 仅 `labels.get(...)` 查表（标签寻址），从不迭代 |
| `crates/wolf-vm/src/program.rs:453/456` | `by_name / by_name_plain: HashMap<String,u32>` | 仅 `by_name.get(...)`（opcode 300 CallByName 名字调用），从不迭代 |
| `crates/wolf-vm/src/program.rs:451` | `events: Vec<Option<Arc<Program>>>` | 按事件 id 索引，确定 |
| `crates/wolf-game/src/auto_events.rs:86/90` | `latched / running: BTreeMap` | 有序 map |
| `crates/wolf-game/src/battle_seed.rs`、`db.rs` | `cells / texts / row_counts: BTreeMap` | 有序 map |
| `crates/wolf-game/src/game.rs` diagnostics | `BTreeMap` | 有序 map |
| `crates/wolf-game/src/game.rs:658-666` | 引擎事件 id `Vec`，来自固定切片 `ENGINE_ALWAYS` + `ENGINE_AUTO_START=[241]`（:98）+ `ENGINE_IMPLICIT=[63]`（:103） | 固定顺序拼接 |
| `picture.rs / map_runtime.rs` | `BTreeMap` | 有序 map |

其余 `HashMap/HashSet` 只出现在**渲染缓存**（`assets.rs`、`atlas.rs`、`fontmetrics.rs`），不在事件/RNG 路径上。
=> 运行期不存在 RandomState 影响行为顺序的通道。旧 HANDOFF_M10 §5.1 的「`started_events` 是 HashSet，
RandomState 遍历顺序扰动战斗前 RNG 消费」结论**不成立**（字段一直是 BTreeSet；即使曾是 HashSet，也只做成员判定，
不参与 RNG 消费顺序）。

RNG（`crates/wolf-vm/src/rng.rs`）：`WolfRng` 双发生器——默认种子 `DEFAULT_SEED=22227` 的 MSVC LCG
（`MULTIPLIER=0x343FD`、`INCREMENT=0x269EC3`），事件发生器 `EVENT_SEED=1`（`EVENT_RESEED=1`）叠加
Park-Miller（`16807 / 0x7FFFFFFF`）。事件抽签只取决于固定帧循环 + `Vec` 实例顺序；`WOLF_EVENT_SEED`
环境钩子可覆盖种子，但缺省为固定 `EVENT_SEED`，故默认构建确定。

## 2. 经验复现（5 次进程重跑，逐字节一致）

命令（工作目录 = 仓库根）：

```
cargo test -q -p wolf-game --test m9_battle_round_gap \
  natural_day_four_battle_runs_genuine_multi_round_and_settles -- --nocapture
cargo test -q -p wolf-game --test m9_full_route \
  the_continuous_new_game_route_reaches_day_five_0830 -- --nocapture
```

各跑 5 次（独立进程），抽取轨迹行做 md5：

- `m9_battle_round_gap` 的 `NATURAL:` 行 5 次完全相同，`md5 = 5f11d63933e9e5127b68ac6b4dae2c92`。
- `m9_full_route` 的 `milestone 5 battle` + `PASS:` 行 5 次完全相同，
  `md5 = 392c3ae2994d7e96f52e4097f98289f1`。

自然 day4 战斗精确轨迹（两次测试一致）：

```
v350=[0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 0]
phases=[(0,-2147483648,0),(709,0,1),(824,1,2),(961,2,0),(963,0,1),(1068,1,2),
        (1205,2,0),(1207,0,1),(1322,1,2),(1459,2,0),(1461,0,1),(1516,1,0)]
active_f=Some(707)  settle_f=Some(1516)  v349=4  pools=[3,2,3,2]  v352=1  v343=1
ce241 live ~258 frames   ce243 live ~294 frames   warnings=0
```

即 4 个命令回合：3 次完整 `0->1->2` 循环，末回合 `0->1->0` 后 CE244 结算（enemy gauge 2000342 归零，
结果 `2000352=1` 胜利，`2000343=1`）。**不是**「3~4 回合、胜负 ±1」的抖动。连续路线最终 day5 08:30，
`frames=9257, instructions=467424`（同样 5 次一致）。

## 3. 收紧后的定值断言

### crates/wolf-game/tests/m9_battle_round_gap.rs
模块文档已改为真实值（`settle=1516`、`v350=[0,1,2,0,1,2,0,1,2,0,1,0]`、CE241 258 帧、CE243 294 帧），
断言由结构式改为定值：

- `main_phases == [0,1,2,0,1,2,0,1,2,0,1,0]`（:608）
- `settle == 1516`（:611）；`command_rounds == 4`（:614）；`action_phases == 3`（:615）
- `v349_at_settle == 4`（:616）；`pools_at_settle == [3,2,3,2]`（:618）
- `ce241_frames == 258`（:622）；`ce243_frames == 294`（:623）；`ce243_first_frame == Some(775)`
- `result_write_frame == Some(settle)`、`enemy_gauge_zero_frame == Some(settle)`、`v342_min == 0`
- `max_ce239_pc == 1099`（:640）；`max_ce242_pc == 967`（:641）
- `v352_at_settle == 1`（胜，:644）；`v343_at_settle == 1`；`!has_event(239)`；`warnings == 0`

### crates/wolf-game/tests/m9_full_route.rs
- `battle_active_frame == Some(707)`（:624）
- `wiring_rounds == 4`（:628）
- `max_ce239_pc == 845`（:617）；`max_ce242_pc == 967`（:641）
- `v350_seq == [0,1,2,0,1,2,0,1,2,0,1,0]`（:647）
- `v352_peak == 1`（:651，定值胜；不再是 `∈{1,-1}`）
- day5 08:30（`160==5, 6==4, 4==8, 5==30`）与 `warnings==0` 保持。

## 4. 残留 / 未覆盖

- **H-PRNG-1（零售时钟种子重放等价性）仍待证**：本证明覆盖「当前构建以固定 `EVENT_SEED=1` 重跑确定」，
  不代表能逐位复刻**零售主机时钟播种**的某次具体运行。这属于 `docs/research/M9_PRNG_SEED.md` 的独立议题，
  与本次「确定性锁定」不冲突（`WOLF_EVENT_SEED` 钩子是后续接入点）。
- 本证明为**单机、单平台、单构建**下的进程间确定性；跨平台浮点/字体度量等渲染差异不影响 RNG/事件顺序。

## 5. 变更文件

- `crates/wolf-game/tests/m9_battle_round_gap.rs`：模块文档改正 + 断言收紧为定值。
- `crates/wolf-game/tests/m9_full_route.rs`：战斗断言收紧为定值。
- `docs/HANDOFF_M10.md`：§5.1 ①、§7 ①、§4 表 #11/#12 删除过时 HashSet 说法 / 更新为定值。
- `PLAN.md`：§4 P0 ① 标记完成并纠正 HashSet 说法。
- `docs/research/M11_BATTLE_DETERMINISM.md`：本文件。
- 未改任何 `src`；未 commit/push。

## 6. 复跑

```
source tools/env.sh
cargo test -q -p wolf-game --test m9_battle_round_gap -- --nocapture
cargo test -q -p wolf-game --test m9_full_route -- --nocapture
cargo test -q --workspace        # 保持全绿
./target/debug/wolf-cli.exe verify   # PASS 70
```
