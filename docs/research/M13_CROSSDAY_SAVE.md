# M13 P0-E — 跨天存读状态连续性 + 全 8 存档槽 round-trip

- 日期：2026-11-01
- 测量 HEAD：`1826edd`（`1826edd test/docs(m13): content matrix, DLC CE452 and mainline ending probes`；含前一提交 `7c3b728` common-event self 持久修复）
- 数据根：`work/all`（23 文件 .wolf 完整解包）
- 零售存档：`work/retail_backup/Save/SaveData01..08.sav` + `System.sav`
- 工具链：rustc / cargo 1.99.1
- 被测件：新增测试 `crates/wolf-game/tests/m13_crossday_save.rs`

## 目标

2026-11-01 完成标准 = 重构版“游戏体验内容与原版一致 + 修复可能存在的 bug”。
本任务验证新修复的 common-event self 持久（7c3b728）在**跨天 + 存档/读档**下不丢状态：

1. 八个零售存档各自 load → save → 用新 Game load → 再 save，断言第二次写出字节 == 第一次写出字节（幂等），且关键状态（var160 天数、钟点、星期、金钱、全部 800 普通变量、图片表、游玩时长、choice anchor、part3 self banks）在重载前后连续。
2. 新启动游戏自然推进数天后 save → reload → save，断言状态连续并收敛到不动点（从第二次写出起 load→save 与上一次逐字节一致）。
3. 校验 common-event self banks（dirty ∪ 仍在运行的 common 协程）在跨存读后完整恢复。
4. 八槽逐一记录通过/失败与数值；无法推进的槽如实说明，不伪造。

## 复现命令

```bash
# 默认（快，并行约 2.6s，串行约 4.4s）：
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m13_crossday_save

# 带数值证据（串行、打印每槽与跨天汇总）：
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m13_crossday_save -- --nocapture --test-threads=1

# 长跑（#[ignore]，约 4.3s；推进到第 11 天）：
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m13_crossday_save -- --ignored --nocapture
```

数据根解析：`WOLF_DATA_ROOT`（相对路径按仓库根 `CARGO_MANIFEST_DIR/../..` 解析），缺省为 `work/all`。
零售存档目录：`WOLF_RETAIL_SAVE_DIR`，缺省 `work/retail_backup/Save`。目录不存在时该测试跳过（不误报）。

## 测试结果

### 默认测试（3 个：2 pass / 1 ignored）

```
running 3 tests
short: arrived day 4 at 07:45 wd=3 money=2929 pics=25 banks_preserved=32 (nonzero 27) S1->S2 picture delta=[16001] S2==S3=yes bytes=347633 elapsed=1.59s
.SaveData01.sav: PASS day=90 clock=21:40 wd=5 money=7586 pics=66 playtime=16642 bytes=366146
SaveData02.sav: PASS day=2  clock=18:30 wd=1 money=2254 pics=58 playtime=1252  bytes=361637
SaveData03.sav: PASS day=62 clock=22:00 wd=5 money=8231 pics=62 playtime=13877 bytes=364263
SaveData04.sav: PASS day=72 clock=21:30 wd=1 money=1293 pics=64 playtime=16536 bytes=365071
SaveData05.sav: PASS day=73 clock=18:30 wd=2 money=1193 pics=63 playtime=16705 bytes=385978
SaveData06.sav: PASS day=53 clock=18:30 wd=3 money=1503 pics=62 playtime=7882  bytes=364257
SaveData07.sav: PASS day=68 clock=19:30 wd=4 money=15453 pics=62 playtime=11143 bytes=364399
SaveData08.sav: PASS day=90 clock=00:42 wd=6 money=7586 pics=84 playtime=16887 bytes=395566
test result: ok. 2 passed; 0 failed; 1 ignored
```

### 长跑测试（#[ignore]）

```
long: arrived day 11 at 07:45 wd=3 money=2729 pics=33 banks_preserved=30 (nonzero 25) S1->S2 picture delta=[16001] S2==S3=yes bytes=372409 elapsed=4.34s
test result: ok. 1 passed
```

## 关键结论与证据

### 1) 八槽 round-trip 严格幂等（默认测试 `eight_retail_slots_round_trip_idempotent_and_continuous`）

对 SaveData01..08 每槽：
- `Game::load_save(orig)` → `save_to(A)`，断言 `bytes(A) == 原始文件字节`（首写即幂等）。
- 新建 Game `load_save(A)`，逐项断言连续：800 普通变量、day(`var160`)、钟点(`var4`,`var5`)、星期(`var6`)、金钱(`var7`)、图片表全字段、游玩时长、sys3/sys4 choice anchor、以及 `all_common_banks()`（事件 0..=550 全部 self bank）。
- 再 `save_to(B)`，断言 `bytes(B) == bytes(A)`，并校验 SaveFile 7 个段名齐全（SavePart1..7 + VariableDatabase）。

实测 8/8 PASS（见上表；字节数与原始存档完全一致，例如 SaveData01 = 366146B、SaveData05 = 385978B）。

### 2) 跨天存档 → 重载 → 存档 收敛到不动点（默认测试 + 长跑）

从新游戏入口 `SampleMapA` 自然推进（day1 18:30 起步，逐日走 `play_day` 日常循环 + `sleep_to_next_morning` 睡觉过夜），跨天游玩全程 `stats.warnings == 0`：
- 短跑：推进到 **第 4 天 07:45**（wd3，money 2929，图片 25 张；保存 347633B），耗时 1.59s。
- 长跑：推进到 **第 11 天 07:45**（wd3，money 2729，图片 33 张；保存 372409B），耗时 4.34s。

每轮定义 S1 = 游玩后存档，S2 = 重载 S1 后存档，S3 = 重载 S2 后存档：
- 重载 S1 的状态与 S1 前逐项相等：800 变量 / day / 钟点 / 星期 / 金钱 / 游玩时长 / choice anchor / part3 self banks 全连续；图片**槽位集合与每个 File 图片文件路径**不变（见“已知残留”对 Text/窗口图片的说明）。
- **S2 == S3 逐字节一致**（真正的不动点：从第二次写出起 load→save 复现完全相同的字节）。
- S1 与 S2 同长度，仅 `SavePart5`(chunk id 4) 不同，且**唯一差异图片是 16001**（代码断言 `differ 全部 == 16001` 否则失败）。

### 3) common-event self banks 跨存读保持（`7c3b728` 修复契约）

存档时捕获 `persisted_banks() = common_self_dirty() ∪ 仍在运行的 common 协程`，重载后逐 bank 断言相等；某个 bank 读回“缺失”时仅当期望全 0（0 值与缺失等价）才放行。
- 短跑：保留 32 个 bank（其中 27 个非零）；长跑：保留 30 个（25 个非零）。
- 每个场景都断言 `nonzero_banks > 0`，确保确实有非零 self 状态被持久化并恢复（不是空洞通过）。

说明：一次完整扫描发现全事件里约 288 个非零 self 槽（含事件 48/70/90 等），但其中很多既不在 dirty 集也无 live 实例，属于纯内存 bank，按修复契约**不写入存档**（“dirty ∪ live”才写）。测试断言的正是这条契约，而非“全部内存 bank 都存”。

## 已知残留（预先存在，非本次修复引入；未改生产 src）

- **图片 16001（live 消息窗文本图）首存非不动点**：`crates/wolf-game/src/savegame.rs` 的 `picture_from_record` 对 Text 图片（var5==1、var6==0）忽略 `vars7_18[2]/[3]`，故窗口框尺寸（如 7680×1200）在重载后丢失（变为 1×1）。因此首次中途存档（S1→S2）在 chunk4 上差一张图 16001；从 S2 起收敛（S2==S3）。
  - 零售存档**从不含图片 16001**（实测 8 槽 `has(16001)==false`：真实存档是在无 live 消息框的菜单里存的），所以八槽 round-trip 严格逐字节相等，不受影响。
  - 该缺陷属图片层既有局限，生产 src 由 self-persist 修复负责人持有，本任务只记录、上报 Lead，不在此修改。
- **Text / 窗口图片重载后重编码**：加载时 `kind` 名（如 WindowFile↔WindowString）、默认 `(1,1)` 窗口、前导字体码（`\f[..]`）会被图片层规范化，故 live 屏幕与重载屏幕在这些图片上字段不完全相等。测试据此只对“槽位集合 + File 图片文件路径”做连续性断言，逐字节保真由 S2==S3 保证。

## 方法学发现（如实记录，不伪造）

零售存档 `load_save` 只恢复变量与图片表，**不恢复被挂起的日程循环协程**：载入后反复喂帧（含 `start_engine_events`）钟点/天数冻结（实测 SaveData02 在 4000 帧内始终 day2 18:30）；若改走 `start_map_event_by_name("SampleMapA",0)` 会重跑新游戏序章并把天数重置为 1。
故：
- 八槽以“逐字节 save/load round-trip + 状态连续”验证（零售场景能存读但不可由重构引擎续玩）。
- 真正的多天推进只能在**新游戏冷启动路径**上验证（该路径重建了 live 日程循环），见第 2 节。

## 改动文件

- 新增 `crates/wolf-game/tests/m13_crossday_save.rs`（默认 2 测试 + 1 `#[ignore]` 长跑）。
- 新增本文件 `docs/research/M13_CROSSDAY_SAVE.md`。
- 生产 src 未改，无 git commit / git clean。

## 遗留问题

1. 图片 16001 的 Text 窗口框尺寸在 `picture_from_record` 丢失（见上），需图片层负责人修 `vars7_18[2]/[3]` 的保存/还原；本任务因 src 归属限制未动。
2. 零售存档不可续玩（缺少日程协程快照）是引擎结构限制，非本任务范围；若需从任意零售档续玩，需另立引擎任务做协程快照/序列化。
