# Review: 游戏状态 / 事件 / 对话生命周期独立审计

- **任务**：task-3「Review: 游戏状态/事件/对话生命周期独立审计」
- **审查者**：review-game-events
- **日期**：2026-11-04
- **代码基线**：当前工作区（wolf-game / wolf-vm / wolf-cli）
- **约束**：只读源码、测试、release、manifest、存档；本报告及截图只写入 `work/out/reviews/`
- **文件**：本文件 `work/out/reviews/review_game_events.md`

## 0. 结论摘要

| 编号 | 结论 | 严重度 | 主要证据 |
| --- | --- | --- | --- |
| UI-10 | **已确认根因**：存档载入后 `prime_latched` 只填充 latched，不恢复 216/217 等 HUD 绘制事件；内部行动力已变化，HUD 保持 SavePart5 旧快照 | P1 | `ui10_sleep_save01_end.png`；`2000014` 75→105→115 而 HUD 仍 75/120 |
| UI-11 | **已复现事件级路径**：夜晚菜单进入 CE247 聊天分支，`CE218 mode -1` 删除 picture 90-98，返回 `コミュトップ` 时没有重绘妹妹 | P1 | `ui11_ce245_day90.png` → `ui11_ce247_chat.png`；`ui11_before_after.png` |
| UI-09 | **未能复现**：CE281 的 CE264 名牌正常（「妹妹」）；VM 字符串参数索引链路正确，不是 `strings[1]` 索引 bug | P1（原登记） | `ui09_ce281_named.png`；全 dump CE264 调用扫描 |
| UI-06 | **状态侧未能复现**：消息保持 600 帧不消失；单窗口覆盖是结构性风险，但未找到真实并发 101 路径 | P1（原登记） | `ui06_msg_600f.png`；`game.rs:2289-2310` |
| 新 NB-01 | **新发现 P2**：`Message.face` 只存不用，`db.rs::name_of(Container::Sys,24,row)` 未被游戏层调用；普通 101 / `@n` 没有名牌兜底 | P2 | `game.rs:142-151`、`game.rs:2289-2298`、`db.rs:492-506` |
| 新 NB-02 | **新发现 P2（主线影响待定）**：`clear_night_room_hud` 删除 108-120，实际是菜单按钮/说话人图标/妹妹立绘槽，不是消息窗口（消息窗口是 19500-19503） | P2 | `game.rs:684-687`、`picture.rs:1148-1152`；SaveData02 + `--event 332` 复现 |

> 本次没有发现 UI-10/UI-11 之外的 P0；UI-11 的直接事件复现已经能稳定删除妹妹立绘，真实地图点击链路仍受 CLI 限制（见 §7）。

---

## 1. UI-10 行动力数值恢复/显示异常

**UI ID**：UI-10  
**结论**：已确认根因（P1），不是变量恢复失败，而是 HUD 的常驻绘制事件没有在读档后重启。

### 1.1 根因（file:line）

- `crates/wolf-game/src/savegame.rs:176-183`：`apply()` 末尾调用 `game.auto_events.prime_latched(&game.vm);`
- `crates/wolf-game/src/auto_events.rs:158-164`：`prime_latched` 只做
  `self.latched.insert(event.id, now);`
  **不启动任何 coroutine，也不写 `self.running`**。
- `crates/wolf-game/src/auto_events.rs:192-213`：`sync` 只在 `now && !before` 的 false→true 边沿调用 `vm.start_event(event.id)`。读档后 `prime_latched` 已把 216/217 的 `latched` 设为 true，因此之后每帧都是 `now && before`，永远不会启动。
- 受影响的 HUD 事件：`216 ステータスバー表示` 重绘行动力/精力/性欲/金钱，`217 時刻表示` 重绘时钟/星期/天数（`docs/ENGINE_EVENTS.md:531-538`）。

### 1.2 复现命令 / 存档 / 帧

使用零售存档 `work/retail_backup/Save/SaveData01.sav`（90 日 21:40，HUD：行动力 75/120、精力 4/6、性欲 56%、金钱 7586）：

```bash
./target/debug/wolf-cli.exe --root release/data capture \
  --load-save work/retail_backup/Save/SaveData01.sav --event 329 --frames 1600 --auto-input \
  --out work/out/reviews/ui10_sleep_save01_end.png
```

截图 `work/out/reviews/ui10_sleep_save01_end.png`：日期已从 90 日 21:40 变成 **91 日 9:30**（217 时间文本更新），但 HUD 仍是 **行动力 75/120、精力 4/6、性欲 56%、金钱 7586**。即：时间显示被刷新，状态条没有。

内部变量验证（不依赖截图）：

```bash
./target/debug/wolf-cli.exe --root release/data dump --event 329 --frames 1600 \
  --var 2000014=75 --var 2000013=120 \
  --watch-var 2000014 --watch-var 2000013 --watch-start 226
```

输出：

- frame 0：`2000014=75 / 2000013=120`
- event 226 于 frame 76 启动；`2000014` 从 75 逐帧升到 105（frame 155 停止）
- frame 198 第二个 event 226 启动；`2000014` 从 106 升到 115（frame 263 停止）
- 1600 帧运行结束

对比截图：`work/out/reviews/save01_5f.png`（载入时 HUD）和 `ui10_sleep_save01_end.png`（睡眠后）在 y=14..28、x=431..569 的 HUD 像素逐像素一致（已用 PIL 验证）。因此 **变量已经变化、界面未重绘**。

### 1.3 变量名 / 存档字段

- 行动力：普通变量 `2000014`（当前）/ `2000013`（上限）。`SaveData01` 的 Part3 normal vars 恢复为 75/120（`savegame.rs:92-95`）。
- HUD 数字是 picture 6235 的文本 `\v[14]/\v[13]`，行动力条是 picture 6233 的 SQUARE 宽条；二者都由 CE216 绘制。
- `savegame.rs:166-174` 从 SavePart5 重建 picture 表，因此读档后 HUD 的旧数字/旧条是快照本身，CE216 不重跑就不会更新。
- `auto_events.rs` 载入日志：所有零售 SaveData01..08 都是
  `auto: 98 tracked conditional/gated event(s), 0 running []`。
  对照：不使用 `--load-save`、从 `--map SampleMapA` 开场运行 2000 帧，日志为
  `auto: 98 tracked ... 3 running [38,216,217]`
  （`work/out/reviews/ui10_map_2000f.png`、`ui10_map_answer.png`）。

### 1.4 最小修改建议

在 `savegame.rs:183` 之后，除 `prime_latched` 外恢复「状态/HUD 类绘制事件」：

1. 首选：给 `AutoEvent` 增加 `resume_on_load: bool`，对 216/217 为 true（CE216/CE217 都是一次性绘制事件，启动一次即可；不要按 `run==2` 全量恢复）；`prime_latched` 改为接收 `&mut Vm`，条件成立时执行 `vm.start_event(id)` 并把 instance 写入 `running`（不能只写 latched）。
2. 或在 `savegame::apply` 中显式对 216/217 调 `game.vm.start_event(id)`；前提是 `2000003 == 1`（开场问答后置 1），且不能在 506 眼罩条件成立时启动 506。
3. 不要把「所有 run==2 事件」都强制启动：`docs/research/M13_AUTO_EVENTS_SCOPE.md:279` 记录 506 `妹トイレ前まばたき` 是载入后必须抑制的 DLC 眼罩，需按事件白名单区分。

### 1.5 建议测试

- 单元测试（`crates/wolf-game/src/savegame.rs` 或 `auto_events.rs`）：载入 SaveData01 fixture 后断言 `game.auto_events.running_ids()` 包含 `216` 和 `217`，且不包含 `506`。
- 截图回归：载入 SaveData01 → 执行 CE329 → 1600 帧 → 断言 HUD 的行动力数字不再是 75/120，且与 `2000014/2000013` 格式化结果一致。
- 可加：`auto_events` 内测试 `prime_latched_seeds_loaded_truth_without_starting` 应改名/扩展，明确「HUD 类事件允许启动」，否则当前测试会把这个 bug 固定下来。

---

## 2. UI-11 夜晚与妹妹互动一次后妹妹从画面消失

**UI ID**：UI-11  
**结论**：已复现事件级路径（P1）。根因不是 CE332 的 `clear_night_room_hud`，而是 **CE247 聊天分支的 CE218 mode -1 删除 picture 90-98，回到菜单顶部时不重绘**。

### 2.1 根因链（file:line + dump 命令索引）

1. 载入 `work/retail_backup/Save/SaveData01.sav`，夜晚时间 21:40，SavePart5 有 66 张 picture；妹妹由 **picture 90** 显示：`Picture/stand_imouto_paja2.png`，位置 `(600,80)`；同组还有 91 `stand_blow_a`、92 `stand_eye_a`、93 `stand_mouth_a`、98 `paja03`。`CE245` 夜菜单能正常显示妹妹（`ui11_ce245_day90.png`）。
2. CE245 的「沟通」分支调用 `CommonEvent[500247,0]`（`work/wolftl2/dump/common/245_■夜コマンドメニュー.json` pc454 附近）；CE247 `┣コミュニケーション` 的聊天分支先调 `CommonEvent[500218,4,19,...]` 显示立绘，随后调 `CommonEvent[500218,4,4294967295,...]`（mode -1）。
3. `work/wolftl2/dump/common/218_■立ち絵表示.json` 的 **command list index 3/4**：
   - `idx 3`：`Picture [16777218, 90, 2, 5, 0, 98]`
   - `idx 4`：`Picture [16777218, 91, 0, 0, 0, 93]`
   
   `16777218 = 0x01000002` 是 Erase；`crates/wolf-game/src/picture.rs:1213-1238` 的长擦除形式把 `args[ERASE_SPAN]` 当第二端点（`ERASE_SPAN=3`，`picture.rs:160`），所以第一条擦除 **90..=98**，第二条擦除 **91..=93**。这正好删掉整个睡衣立绘组 90-98。
4. CE247 当前聊天分支在 `work/wolftl2/dump/common/247_┣コミュニケーション.json` 的 **list index 528** 调 `CommonEvent[500218,4,4294967295,...]`，紧接着 **list index 529** 是 `JumpLabel` → `コミュトップ`（label 在 list index 130 设置）；这两条之间没有 `CommonEvent[500325,0]`，也没有重画 picture 90 的命令。CE325 `■自宅` 才是负责重绘自宅背景和 picture 120 `Picture/imouto01..04.png` 的事件（`work/wolftl2/dump/common/325_■自宅.json` pc34-48），当前分支没有调用它。
5. 从 SampleMapA 的真实夜晚入口看：`SampleMapA` event1「妹クリック判定（夜）」page0 index42 直接 `CommonEvent[500247,0]`，之后只 erase 1210/1211，不调 CE325；支持「点击妹妹 → CE247 → 聊天分支 → 妹妹消失」的真实主线路径（此段为 dump 代码核对；完整 map 交互见 §7 限制）。

### 2.2 复现命令 / 存档 / 截图

基线（before）：

```bash
./target/debug/wolf-cli.exe --root release/data capture \
  --load-save work/retail_backup/Save/SaveData01.sav --event 245 --frames 60 \
  --out work/out/reviews/ui11_ce245_day90.png
```

截图 `work/out/reviews/ui11_ce245_day90.png`：自宅房间 + 五按钮 + 右侧妹妹（picture 90）。

直接走 CE247 聊天分支（after）：

```bash
./target/debug/wolf-cli.exe --root release/data capture \
  --load-save work/retail_backup/Save/SaveData01.sav --event 247 --frames 4000 \
  --answer 0 --answer-delay 10 \
  --out work/out/reviews/ui11_ce247_chat.png
```

实测统计：`57781 instruction(s), 112 picture op(s), 2 sound(s), 4 message(s), 2 choice(s)`；终态 live pictures 中 **没有 90/91/120**；截图 `work/out/reviews/ui11_ce247_chat.png` 显示 22:20 自宅房间、HUD 75/120、**妹妹完全消失**。对比图 `work/out/reviews/ui11_before_after.png` 同屏展示 before/after。

补充：CE245 菜单截图 `ui11_ce245_pics.png` 的 picture dump 显示 `pic 90 Picture/stand_imouto_paja2.png (600,80)`；CE247 结束后终态只有背景/房间/UI 层，无 90。`ui11_ce247_end.png`（3000 帧同分支）有相同结果，说明不是单次超时或 frame 数不足。

### 2.3 最小修改建议

- **事件数据层（最小）**：CE247 每个走 `CE218 -1` 并 `JumpLabel コミュトップ` 的返回分支，在跳转前恢复进入前的基础站立图（保存/恢复 picture 90-98 的槽位值），或调用 `CommonEvent[500325,0]` 重绘自宅与妹妹。不要只补某一分支，需覆盖所有 `CE218 -1 → JumpLabel` 返回点。
- **引擎层（若允许改引擎）**：区分「事件临时立绘」和「房间基础立绘」；`CE218 -1` 的擦除范围应只清事件临时槽（例如 800/1200），不能把基础睡衣组 90-98 当临时层；或在 `close` 事件时压栈恢复。
- **CE332 路线不要混为本 bug 的根因**：`game.rs:684-687` 的 `clear_night_room_hud` 确实会删除 120（见 NB-02），但 SaveData01 的妹妹是 90，且本路径没有调用 `start_event(332)`；不要把 UI-11 的修复错误地做成「调整 332 清理范围」。
- 若想统一处理「夜晚互动后恢复角色层」，可在 CE245/CE247 的菜单循环顶部统一重绘基础房间（CE325），但要验证 CE218 临时 CG 不会残留。

### 2.4 建议测试

- 集成/截图测试：载入 SaveData01 → `--event 245` → 进入「沟通」→ 执行一个聊天分支 → 回到 `コミュトップ`，断言 picture 表仍有 90（或 120），截图与 `ui11_ce245_day90.png` 对比。
- 事件数据单测：解析 CE247，遍历所有 `CommonEvent[500218,4,-1,...]` 后接 `JumpLabel コミュトップ` 的位置，断言每个返回点前有恢复/重绘命令（可用 dump JSON 做静态契约测试）。
- 引擎单测：`CE218 -1` 只删除临时立绘槽，不删除 90-98 基础组；用 SaveData01 part5 作为 fixture。

---

## 3. UI-09 角色说话时人名框有时消失

**UI ID**：UI-09  
**结论**：本次**未能复现**；CE264 显式名牌链路正常。全仓扫描未发现 `strings[1]` 索引 bug；另有「face → System_DB[24] 行名」兜底未实现（新 NB-01），但零售数据中未找到实际触发角色名缺失的普通 101。

### 3.1 实测

```bash
./target/debug/wolf-cli.exe --root release/data capture \
  --event 281 --frames 120 --hold-message \
  --out work/out/reviews/ui09_ce281_named.png
```

CE281 pc3：`CommonEvent[500264,12323,0,2002,1,0,0] ['','妹妹','']` → pc4 Message；截图 `work/out/reviews/ui09_ce281_named.png`：对话框左上角显示名牌「妹妹」，正文 2 行、font 20 at `[64,410]`。该典型事件**未复现** UI-09。

### 3.2 代码/数据核对

- `crates/wolf-vm/src/exec.rs:2531-2554` `string_arguments` 会丢掉编辑器保留的空槽 `stringArgs[0]`（literal 从 `cmd.strings[1+k]` 取），所以 `Host::common_event_call` 收到的 `strings` 中名字确实是 `first()`。`crates/wolf-game/src/game.rs:2261-2284` 读 `strings.first()` **不是索引 bug**；原先怀疑的 `strings[1]` 与 VM 实际传参不一致。
- 全 dump 共 **7787** 个 CE264 调用，`stringArgs` 模式全部是 `('', NAME, '')`（如妹妹 1130 次、加奈 481 次、message 39 次）；没有发现第二个非空字符串。
- 全 dump 共 **13169** 条 101 消息，`@` 前缀只有 **18** 条且全部 `@1`（物品/金钱/参数增减提示），没有角色台词依赖 `@n`。
- `game.rs:142-151` `Message.face` 只保存；`game.rs:2289-2298` 赋值；全仓没有生产渲染/名牌读取 `.face` 的路径。详见 NB-01。
- `work/wolftl2/dump/common/63_X[共]メッセージウィンドウ.json`（192 命令）由 CE63 负责画消息窗口/右下脸图（pc23-25 读 System DB 24 脸图路径，pc116-131 绘制），引擎只画文字；`Sys0` 语义为 0 不显示窗口、1 只显示窗口、>=2 显示脸图。`CE267` pc46 `SetVariable[9000000,1600000]` 改脸号；没有脚本把 Sys0 置 0/-1。

### 3.3 结论 / 建议

- UI-09 本次无法复现：**未能复现 + 已核对代码线索**。不能据此断言代码已修好；需要用户提供出现时的存档/帧，或从渲染层查「名牌存在但被消息框/图层遮挡」。
- 最小改进（见 NB-01）：若 101 有 `face` 且没有 CE264 `speaker`，用 `db.rs::name_of(Container::Sys,24,face)` 作为名牌兜底；显式 CE264 名字应优先。
- 建议测试：`common_event_call_captures_the_ce264_name` 目前用 `&["妹妹",""]` 调用，已经假定 VM 丢空槽；应新增真实原始 `['','妹妹','']` 解析测试和「普通 101 + face」兜底测试。

---

## 4. UI-06 对话框消失 / 文本显示异常（状态侧审计）

**UI ID**：UI-06  
**结论**：状态层**未能复现**；消息在 600 帧内持续存在，未发现 `frames` / `auto_advance` 提前清除。留作与渲染 review 的交叉验证项。

### 4.1 实测

```bash
./target/debug/wolf-cli.exe --root release/data capture \
  --event 281 --frames 600 --hold-message \
  --out work/out/reviews/ui06_msg_600f.png
```

600 帧后仍输出：

```
message: 2 row(s), 27/27 char(s), font 20 at [64,410], wrap 928, cursor [576,554]
stats: 1 message(s)
```

截图 `work/out/reviews/ui06_msg_600f.png` 可见对话框与两行正文。`--hold-message` 下消息没有被状态机清掉。

### 4.2 代码事实（file:line）

- `crates/wolf-game/src/game.rs:2289-2298` `show_message`：`self.messages.clear(); self.messages.push(Message { ... });` — 单窗口模型，新 101 替换旧 101。
- `crates/wolf-game/src/game.rs:2302-2306` `message_dismissed`：`self.auto_advance || self.key_bits & crate::key::OK != 0`；`auto_advance` 默认 false（`game.rs:520`），仅 wolf-cli dump/package 设置（`main.rs:1737,2818`、`wolf-package.rs:764`）；交互 `play.rs` 不设置。
- `crates/wolf-game/src/game.rs:2308-2310` `close_message` 清空整个 `messages`。
- `crates/wolf-vm/src/exec.rs:1153-1161` `WaitState::Message` 在 `host.message_dismissed()` 后调用 `host.close_message()` 并清除 wait。
- `crates/wolf-game/src/game.rs:1885` 每帧末 `set_sys(12, !messages.is_empty())`；状态侧没有提前清空。
- `crates/wolf-cli/src/play.rs:236` 点击只注入**一帧** `key::OK`；`play.rs:262` 光标使用 `messages.last().frames`。

### 4.3 结构性风险

`show_message` 的 `messages.clear()` 意味着两个协程在同一帧各发一条 101 时，前一条会被静默覆盖，且没有队列/公平性保护。当前 13169 条消息的静态 dump 未找到真实并发的两个 101 路径，所以不能把它列为已复现 bug；但如果用户报告「对话框突然换成另一句」或「完全消失」，这里是首要状态层嫌疑。

### 4.4 建议测试

- 单元测试：在 `Game` 中启动两个 event instance，让两者同一帧调用 `show_message`；定义并断言期望行为（保留第一条/排队/覆盖记录），至少暴露当前丢失行为。
- 交互回归：真实对话中持续按住/连点确认键，断言消息必须至少显示到下一次输入，不因 `auto_advance`/OK 残留瞬间跳过。
- 与渲染 review 联动：确认消失是状态层清空还是渲染层被遮挡（消息框 picture 19500 是否仍在 screen）。

---

## 5. 新发现 bug NB-01：`Message.face` 与 System_DB[24] 行名链路未接通

**严重度**：P2（潜在；当前零售数据未观察到用户可见缺失）  
**对应 UI**：UI-09 相关（角色名缺失）

### 5.1 事实

- `crates/wolf-game/src/game.rs:142-151`：
  `pub struct Message { pub text: String, pub face: Option<i32>, pub speaker: Option<String>, pub frames: u32 }`
- `crates/wolf-game/src/game.rs:2289-2298` 只把 `face` 存入 `Message`，之后没有任何生产代码读取该字段；全仓 `.face` 只剩测试构造。
- `crates/wolf-game/src/db.rs:492-506` `name_of` 注释明确：SysDatabase type 24 的 row name 就是脸名，不能从 `text()` 取；`db.rs:1057` 测试断言 `name_of(Container::Sys,24,2)==Some("妹（制服）")`。
- 游戏层 `game.rs` **没有调用 `name_of` 的任何位置**；`message_speaker()` 只读 `messages.last().speaker`（CE264 显式名字）。

### 5.2 影响

CE264 包装的角色台词有显式名字（本次 CE281 实测正常），所以零售主线暂时看不到缺失；但任何走普通 101 + `@n`（`exec.rs` 会 `set_sys(0, face)`）或 `Sys0>=2` 的消息都没有名牌兜底。若后续脚本/资料片用普通 101 给角色台词，就会出现「有脸图但无名牌」，与 UI-09 的用户描述一致，因此保留为 P2 潜在 bug。

### 5.3 最小修改建议

在 `Game::show_message` 后或 `message_speaker()` 的兜底逻辑中：

```
speaker = explicit_speaker.or_else(|| face.and_then(|f| {
    self.databases.name_of(Container::Sys, 24, f).map(str::to_owned)
}))
```

注意：旁白（face=None 且 speaker=None）必须仍返回 None，不能把默认脸名当成旁白名；CE264 显式名字优先于 DB 行名。

### 5.4 建议测试

- 单元测试：构造 `Message { face: Some(2), speaker: None }`（或调用 `show_message` 前设 `pending_speaker=None`），断言 `message_speaker()==Some("妹（制服）")`；再断言显式 `speaker=Some("加奈")` 时优先返回「加奈」。
- 数据测试：用含 System DB 的 fixture 验证 type 24 行号越界/无 .project 时返回 None，不 panic。

---

## 6. 新发现 bug NB-02：`clear_night_room_hud` 的 108-120 清理范围与槽位语义不符

**严重度**：P2（主线影响待定；直接 `--event 332` 可复现）  
**对应 UI**：UI-11 邻近（夜晚图层清理）

### 6.1 事实

- `crates/wolf-game/src/game.rs:680-687` 注释称 `108`-`120` 是 “room message window left behind when the CLI jumps straight into 332”，但：
  - 消息窗口实际是 picture **19500-19503**（`crates/wolf-game/src/message.rs:56-60` `FRAME_PICTURE: i32 = 19500`）。
  - 108/109/110/111 是 `Picture/sub_button02/03/04/05.png` 菜单子按钮；118 是 SampleMapA ev34 的 `Picture/speaker.png`；119 是 ev34 的三行 ■/□ 文本；120 是 CE325 的 `Picture/imouto01..04.png` 妹妹立绘。
- `crates/wolf-game/src/picture.rs:1148-1152` `remove_range` 是闭区间 `first..=last`；所以 `remove_range(108,120)` 会删除上述全部槽位。
- 直接复现：SaveData02 baseline `work/out/reviews/ui11_save02_base.png` 有 `pic 120 File at (388,368) ... Picture/imouto04.png`；执行
  `capture --load-save work/retail_backup/Save/SaveData02.sav --event 332 --frames 2 --out work/out/reviews/ui11_save02_ce332.png`
  后图片数 58→56，pic120 消失（只剩 `pic 1200 hscene/h_s01_pose01_paja.png`），证明 `Game::start_event(332)` 的 clear 会直接删妹妹立绘。
- **但是**：零售就寝路径 `CE328 ┣就寝前パジャマ` 自己在 command list index 704/732 执行 `Picture[16777218,108,0,0,0,120]`（同样擦除 108..=120），随后 index 711/740 才调 `CommonEvent[500332,0]`。因此主线中 CE328 已经显式清理同一范围，`clear_night_room_hud` 只是重复；本 bug 是否造成真实用户可见影响取决于是否有其它入口直接启动 332。**标记为待定，不把它当成 UI-11 的根因。**

### 6.2 最小修改建议

- 若确认 CE328 已负责清 108-120：`clear_night_room_hud` 只保留 `remove_range(1600,1621)`，不要碰 108-120，避免直接事件/调试路径误删立绘。
- 若必须保留直接进入 332 的清理：改成精确删除已知的窗口/菜单槽位（并写注释列出实际槽位），不要用 108-120 的宽范围；或把「CLI 调试路径」和「真实 CE328 路径」分开处理。

### 6.3 建议测试

- 单元测试：用带 picture 108/118/119/120 的 screen fixture 调 `Game::start_event(332)`，断言 120 保留（或按产品决定明确删除），并断言 108-111/118/119 不被误删。
- 截图回归：SaveData02 直接 `--event 332` 前后对比，确认不再无差别删除 120；真实 CE328 路径单独回归。

---

## 7. 剩余验证 / 测试限制

1. **UI-11 完整地图交互**：CLI 的 `--event 245` 保存入口没有地图层，CE245 菜单是 picture + InputKey 轮询（不是 opcode 102），`--pointer` 只能 hover 不能完成点击；因此本次用直接 `--event 247 --answer 0` 复现聊天分支，并核对了 SampleMapA event1 page0 index42 的 `CommonEvent[500247,0]` 路径。仍需在真实交互（或可注入地图输入的 debug 工具）下确认完整「点击妹妹→CE247→菜单循环」截图。
2. **UI-10 原版期望值**：内部变量 75→115 已实测；需要原版同存档睡眠截图确认 115/120 是正确目标值（当前只能确认 HUD 与内部变量不一致）。
3. **UI-06 用户帧**：状态层 600 帧保持，未能复现；需要用户提供消失时的存档/帧号，或由渲染 review 检查消息框 19500 / 字体图层是否被遮挡。
4. **UI-09 失败样本**：CE281 典型事件正常；需要用户出现时的存档/事件 id，才能验证 NB-01 是否是实际触发点。
5. **NB-02 主线影响**：CE328 已同范围擦除，需确认 `clear_night_room_hud` 是否只在 CLI/直跳路径使用，避免把重复清理当真实 bug 修。

---

## 8. 审计方法与可复现性

- 源码：`crates/wolf-game/src/game.rs`、`savegame.rs`、`auto_events.rs`、`message.rs`、`picture.rs`、`db.rs`；`crates/wolf-vm/src/exec.rs`；`crates/wolf-cli/src/play.rs`、`main.rs`。
- 事件 dump：`work/wolftl2/dump/common/`（CE63/216/217/218/245/247/325/328/332 等）。
- 真实存档：`work/retail_backup/Save/SaveData01.sav`、`SaveData02.sav`（零售备份；未使用 `work/full/save` 中被改过的存档）。
- 截图：全部位于 `work/out/reviews/`：
  - `ui10_sleep_save01_end.png`（UI-10）
  - `save01_5f.png`（SaveData01 载入基线）
  - `ui07...`（本任务未涉及）
  - `ui09_ce281_named.png`（UI-09 正常样本）
  - `ui06_msg_600f.png`（UI-06 600 帧保持）
  - `ui11_ce245_day90.png`、`ui11_ce247_chat.png`、`ui11_before_after.png`（UI-11 before/after）
  - `ui11_save02_base.png`、`ui11_save02_ce332.png`（NB-02）
- 关于 `release/data/save/system.sav`：执行 CE329 的 capture 会写该文件（365382 bytes）；本次已用 `work/out/release_save_backup/system.sav`（363523 bytes，md5 `564f3570677e10dfef2eae2f10aa69e8`）恢复并验证 md5 一致。后续复现 CE329 时请先备份，或使用只读存档夹具。
- 本报告没有修改任何源码、测试、release、manifest 或存档；所有写入均在 `work/out/reviews/`。

## 9. 一句话交接

- **UI-10**：先修 `savegame.rs:183` 的 `prime_latched` 语义，单独恢复 216/217 HUD 事件；当前所有零售存档载入后都是 `0 running`，HUD 必然停在 SavePart5 快照。
- **UI-11**：先修 CE247 的 `CE218 -1` 返回分支，保住 picture 90-98；不要把 CE332 的 `clear_night_room_hud` 当作根因，它只是重复了 CE328 的清理。
- **UI-09/UI-06**：本次状态侧均未复现；若要继续，需要用户失败样本或转渲染 review，不要仅凭代码猜修。