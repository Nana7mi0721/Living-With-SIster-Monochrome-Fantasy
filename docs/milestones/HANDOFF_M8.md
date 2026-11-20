# M8 交接：战斗结算闭环、H 前向链、地图角色层、模板存档

> **历史证据（2026-11-14 冻结）**：Godot 阶段入口见 [`godot/README.md`](godot/README.md)；本文件不再追加“下一步”。
>
> 仓库：`E:\Study\github program\imoto life`
> 基线：本文件随 M8 集成提交入仓。以 `git log --oneline` 为准；M7 第二批集成为 `b9e0d36`，
> M8 集成从 `332b82e` 起。
> 阅读顺序：本文件（M8 最新）→ `docs/HANDOFF_M7.md` → `docs/HANDOFF_M6.md` → `docs/HANDOFF_M4.md` → `docs/HANDOFF_M3.md`。
> `docs/gpt调研/` 不是项目文件，不提交；与实测冲突时以本仓库代码/已验证文档为准。
> **本轮全部提交均为本地提交，没有 push。** `origin/master` 仍停在 `1d36ebe`。

## 1. 一句话现状

M0–M7 保持可交付；M8 已落地并机检：

- **W1-c 战斗结算闭环**：`battle_wiring.rs` 传给 CE242 的 arg0 改为**行动队员编号**（1），
  而不是战斗上下文 `2000354`。真实 day4 `CE253→CE239` 战斗现在会结算：
  `2000352=1`、`2000343=1`，CE239 返回，晚餐后经真实夜间菜单（悬停妹妹 + Decide）到
  **day5 08:30**，全程无结算变量注入。`m7_day4_playable` 的验收运行 frames=8008、warnings 0。
- **W2-d H 前向链**：`m8_hscene_forward.rs` 用真实指针连续走通 wipe→ev21→CE333 调用点、
  s01→var62 1→2、end→CE345；另以 fresh replay 到 stage 11，并重放 CE333 函数体
  （1739 条指令、重绘 pose_10/word_effect102）。兄弟包装器 CE246→CE217/CE346、
  CE323→CE372/CE371、CE328→CE402 的阻挡条件全部钉死。
- **W5-a 零售事件 RNG**：`0x6000` 现在使用 Park-Miller MINSTD（16807 / 2^31-1，
  >2e9 时继续乘、为 0 时重播种），并按反编译的归约律 `min(a,b)+draw%(abs(b-a)+1)`
  每调用消耗一次抽样；MSVC LCG 仅保留为 Game.dat 静态表/文件解密流。默认事件种子
  `EVENT_SEED=1`（确定性的时钟种子替身）。序列敏感的 M6/M7 测试已按新抽样修复。
- **W3-a 地图角色层**：`MapRuntime` 保留每个地图事件的瓦片 x/y 与每页朝向；
  `GameHost::character_state` 对地图事件与队长（`-2`）返回真实字段 0..10；英雄 `130`
  传送会放置队长（options bit0 = 半格）。
- **W4-b 模板存档**：`SaveFile::new_game_template()` 合成结构合法的 0x8C 模板；
  没有 `Save/System.sav` 的根（`work/all`）现在也能 `save_to`，并能被 fresh `load_into` 回读。
  模板的可变数据库由静态 `CDataBase` schema 播种（`Databases::template_vardb()` →
  `SaveFile::new_game_template_with_vardb()`），运行时 CData 单元格会真正写进存档
  （`m7_save_parts` 在 `work/all` 通过 `CData (0,0,5)=4242` 与文本单元格）；解析成功但
  `type_count == 0` 的 `System.sav` 用同一 schema 补全。9 个零售存档仍逐字节重建。
- **W3-b engine_state 反编译**：`game.rs` 现在回答 11/12（鼠标位移），13/17 的语义已由
  二进制钉死；`docs/research/M8_ENGINE_STATE.md`。
- **W4-c 悬空地图 id**：`db::map_path` 把空路径视为未注册，Map031 的 id 27 只记一条
  `no map registered for id 27` 诊断，不再读数据根目录。

## 2. 怎么跑 / 怎么验

```bash
cd '/e/Study/github program/imoto life'
source tools/env.sh >/dev/null 2>&1
cargo test -q --workspace            # 全绿（80 个测试二进制，0 失败）
cargo build -q -p wolf-cli
./target/debug/wolf-cli.exe verify   # 预期 PASS 70, pending 0
```

M8 单跑：

```bash
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m7_day4_playable -- --nocapture
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_forward -- --nocapture
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_character_state -- --nocapture
WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_template_save -- --nocapture
WOLF_DATA_ROOT=work/full cargo test -q -p wolf-game --test m7_map_edge -- --nocapture
WOLF_DATA_ROOT=work/all WOLF_RETAIL_SAVE_DIR=work/retail_backup/Save cargo test -q -p wolf-game --test m7_save_parts -- --nocapture   # 8 个零售存档 + CData 单元格往返
```

数据根：`work/all`（23 个 .wolf 全解包，H 场景/`DLC_data` 只在这里）、`work/full`（带
`Save/System.sav`，存档/读档测试用）。游戏安装：`E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy`（目录名确实拼错）。
测试 cwd 是 crate 目录，相对 `WOLF_DATA_ROOT` 与 `WOLF_RETAIL_SAVE_DIR` 都会按仓库根解析。

## 3. 本批交付与证据

| 工作流 | 交付 / 证据 | 结论 |
| --- | --- | --- |
| W1-c 战斗闭环 | `332b82e`；`src/battle_wiring.rs` + `tests/m7_day4_playable.rs`；`docs/research/M8_BATTLE_SETTLE.md` | CE242 arg0 = 行动队员槽位；`wolf-cli dump --event 242 --self 600=1` 结算、`--self 600=0` 不结算；day4→day5 无注入 |
| W2-d H 前向 | `3b957a7`；`tests/m8_hscene_forward.rs`（2 测）；`docs/research/M8_HSCENE_FORWARD.md` | 连续真实指针链 + 精确阻挡门槛；warnings 0 |
| W3-a 角色层 | `f9b625f`；`src/map_runtime.rs` / `game.rs` / `tilemap.rs`；`tests/m8_character_state.rs`；`docs/research/M8_CHARACTER_STATE.md` | SampleMapA ev13 (63,35)/facing 2；新游戏 130 → (31,19)/(62,38)；跨地图不泄漏 |
| W4-b 模板存档 | `94e94e0`；`crates/wolf-save/src/lib.rs`；`tests/m8_template_save.rs`；`docs/research/M8_TEMPLATE_SAVE.md` | `work/all` 可存档并回读；9/9 零售存档仍逐字节 |
| W4-b 后续 vardb | `1f7d2be`（合并 `4b57c06`）；`db.rs::template_vardb` + `SaveFile::new_game_template_with_vardb` | 模板 vardb 由静态 CData schema 播种；`m7_save_parts` 在 `work/all` 的 CData/文本单元格往返通过 |
| 测试环境路径 | `0b7d805` | 相对 `WOLF_RETAIL_SAVE_DIR` 与 `WOLF_DATA_ROOT` 一样按仓库根解析（此前会静默 skip） |
| W4-c 悬空地图 | `5e996f0`；`db.rs`；`m7_map_edge` 4/4 | 跳过 + 单条诊断，无读错误 |
| 测试路径 | `5538bd7` | 相对 `WOLF_DATA_ROOT` 统一按仓库根解析 |
| W5-a 事件 RNG | `1fa7cf1`；`crates/wolf-vm/src/rng.rs` + `tests/rng_retail.rs`；`docs/research/M8_PRNG_RANGE.md` | Park-Miller MINSTD + 取模归约；MINSTD/归约向量、零宽消耗一次、流独立、MSVC 向量不变 |
| RNG 测试修复 | `eb83b23` | M6/M7 日循环按新抽样动态选行/等待晚间收敛，语义断言未削弱 |
| 状态文档 | `7c28480`、`4e69014` | M8_BACKLOG/BASELINE 与实测同步 |

## 4. 关键源码位置

| 位置 | 内容 |
| --- | --- |
| `crates/wolf-game/src/battle_wiring.rs` | `ACTING_PARTY_SLOT=1`；CE242 的 arg0 契约与证据 |
| `crates/wolf-game/tests/m7_day4_playable.rs` | day4 战斗结算 + 晚餐 + 自然夜间菜单 + day5 验收 |
| `crates/wolf-game/src/map_runtime.rs` | `MapActor` / `LeaderPlacement` / `place_leader` |
| `crates/wolf-game/src/game.rs` | `character_state` fields 0..10、`place_leader`、`switch_map(hero)` |
| `crates/wolf-save/src/lib.rs` | `SaveFile::new_game_template()` / `new_game_template_with_vardb()` |
| `crates/wolf-game/src/db.rs` | `Databases::template_vardb()`（静态 CData → 存档可变数据库） |
| `crates/wolf-game/src/game.rs` (`Game::new`) | System.sav 缺失时的合成模板回退 |
| `crates/wolf-game/tests/m8_hscene_forward.rs` | H 前向链与门槛 |
| `docs/research/M8_PRNG_RANGE.md` | 0x6000 反编译证据（归约律 + Park-Miller 发现） |

## 5. 已确认边界 / 未完成

1. **PRNG（已完成，含残余）**：归约律由 RVA 0xaf7c0 钉死，抽样已移植为 31 位
   Park-Miller MINSTD（RVA 0x35810）；MSVC LCG 只用于 Game.dat 静态表/解密。残余：零售
   事件流从时钟自播种，本移植固定 `EVENT_SEED=1`，因此“同一存档+同一输入”的逐位对拍仍
   未证明（需要捕获时钟种子/生产种子钩子）。
2. **engine_state 字段（已反编译）**：11/12 是逐帧鼠标位移（已实现）；13 是光标下事件
   （本作 0 站点，返回 None）；17 是「当前公共事件号」（Host 无实例上下文，仍返回 0 中性值）。
3. **H 深链**：`ev33`（挿入抜く）必须 `var460==1`，而射精链要求 `v111>=100`（零售存档为 0）
   → CE344/CE347；本批已精确断言，不是引擎缺陷。CE333 的 bath 分支需 `v458==1`。
4. **CE328→CE402** 被 `v104/v105==0` 与 `v4>=21` 阻挡；CE323→CE372 的 `var218` 设计上在
   阶段计时器循环内保持 2。
5. **角色层限制**：无移动模型；精确坐标仅对静态放置/精确传送成立；height 恒 0；
   `-1`/`-3..-7` 仍返回 None；朝向在裸 `130` 后沿用上一放置。
6. **模板存档限制**：vardb 已由静态 CData schema 播种，CData 单元格可存档；`SysDatabase`/
   `DataBase` 单元格按设计不进存档（`write_into` 跳过）；超出静态表行高（如扩展行 `100+n`）
   且没有先写 `250` 行数的写入仍计入 `unmodelled`；Part1/Part2 为默认零值。
7. **M4 音频**：仍待人工听感验收；**像素/锚点**：H-ANC-1、H-PIC-2 低优先级残留。
8. **多队员战斗**：CE242 目前只按 1 名队员启动；多队员需要逐行动队员的循环重建。

## 6. 工程约束 / 协作陷阱

- 不提交 3 个永久 scratch：`crates/wolf-format/examples/opscan.rs`、
  `crates/wolf-format/tests/_scratch_cdata.rs`、`_scratch_event.rs`；`_m6_actor_probe.rs`
  同样是临时探测文件。`docs/gpt调研/` 是用户资料，不提交。
- 提交信息用 `git commit -F work/_commit_*.txt`；推送用 `git -c http.sslBackend=openssl push origin master`。
- `crates/wolf-vm/src/exec.rs` 实测为 LF（工作树在 Windows 上显示 CRLF，提交时按仓库配置处理）。
- 截图/临时日志写 `work/out/`；不跑 `git clean`；不杀用户/编辑器进程。
- 多 agent 共用工作树：每人只写自己声明的文件范围；集成前 Lead 检查 `git status` / `git diff`，
  并跑全量测试 + verify。
- M8 的 worktree 分支（W4-b、W3-a、W2-d、W3-b、W4-b vardb、W5-a）已全部合并到本地 master
  （合并提交 `94e94e0`、`f9b625f`、`3b957a7`、`332b82e`、`652ee9d`、`4b57c06`）；worktree 目录
  仍在磁盘上占几 GB，删除属于破坏性操作，需用户明确同意后再清理。
- 本轮遵守用户要求：只做本地提交，不 push；`origin/master` 仍是 `1d36ebe`。

## 7. 下一步建议

1. 低优先级：锚点位/图片槽位像素对照、M4 人耳验收、多队员 CE242 循环。
2. 若要逐位对拍零售：捕获一次真实运行的时钟种子，接上生产种子钩子；
   `random_a/random_b` 是 MSVC 流的编码抽样，不是种子（已修正文档）。
