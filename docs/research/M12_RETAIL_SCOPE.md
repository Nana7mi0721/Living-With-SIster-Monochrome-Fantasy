# M12 零售范围审计：只实现游戏流程，不扩展引擎通用能力

> 状态：**已验证（静态数据 + 现有代码/测试）**
> 测量 HEAD：`1f19dcd`（M11 round 3 后）
> 数据根：`work/all`（23 个 .wolf 解包；MapData 9 个 .mps）
> 用途：回答“哪些是游戏零售内容，哪些只是 WOLF 引擎通用能力/示例资产/保真工具”。结论用于约束后续工作：**零售流程要补齐；引擎通用但零售不用的能力不再新增、不再当缺口。**

## 1. 一句话结论

这款游戏是**纯鼠标点触式**：注册地图没有可走角色图，没有玩家触格/事件触格，交互由 `150 Picture` + `124 SetVariableEx` 悬停/点击完成。因此下列能力虽然 WOLF 引擎具备，**不是本作零售内容**：方向键行走/碰撞/8 向/contact、ActionKey 对话、相机滚动/281/201 路线播放/远景视差、地图触发类型 3/4。

同一结论不适用于存档/读档/回标题/退出/截图存档/相册/DLC：这些都有真实调用点，必须保留。

## 2. 零售实际使用（保留，不要删）

| 功能 | 证据 | 判定 |
| --- | --- | --- |
| 220 SaveLoad / 221 LoadGame / 222 SaveGame | 220 8 条、221 16 条、222 5 条；CE131/CE435/CE436 存档与自动存档，CE94 载入窗口，TitleMap ev2 初始化 `system.sav/system2.sav` | 零售流程 |
| 174 ReturnToTitle / 175 EndGame | 174 13 条（CE246/247/305/322/323/326/359/405 + SampleMapA ev4/5/24）；175 2 条（CE38/CE42） | 零售流程 |
| 1000 ProFeature（截图） | CE131 pc223/225 存档截图；CE435 自动存档截图；CE452 DLC 剧情 `Data/SC1.jpg`；CE525/526 相册 | 零售流程 |
| 150 Picture + 124 hover | 全库 150 共 16716（CE）+ 3709（地图页）= 20425 条；124 共 579（CE）+ 466（地图页）= 1045 条；地图页交互靠二者 | 零售主输入 |
| 130 Teleport | 地图 8 站点 + CE42；真实新游戏 Map003 ev1 `[-1,62,38,1,1]` 落到 SampleMapA | 零售流程 |
| 151/160/161/162 画面转场 | 地图页各 1 组，Dungeon/SampleMapB 样本另有 | 零售流程（地图侧） |
| 战斗 / H / 数据库 / 存档结构 | M7–M11 已证的真实链路 | 零售流程 |

## 3. 引擎通用、零售不使用（不新增、不列为缺口）

| 通用能力 | 代码现状 | 为什么零售不用 |
| --- | --- | --- |
| 方向键整格/8 向行走、碰撞、contact、ActionKey | `crates/wolf-game/src/game.rs:72 WALK_STEP_FRAMES=8`；`:794 try_move_leader`、`:800 move_leader`、`:809 try_move_leader_contacts`、`:847 interact_action_key`、`:871 advance_player_input`；帧循环在 `game.rs:1372-1385` 调用 `:1384 advance_player_input()` | 注册 6 张地图 115 页 `graphic` 全空；player-touch/event-touch 页只在未注册样本 Dungeon/SampleMapB 出现；零售交互是 150 Picture + 124 hover |
| 地图触发类型 3/4（玩家触格/事件触格） | `map_runtime.rs:646 try_trigger`、`:707 start_trigger_page` | 115 页里 0 个类型 3/4 页；只有样本地图有 |
| 相机滚动 / culling / 281 ScrollScreen | `tilemap.rs:106 quads(..., camera)` 有 camera+cull；`game.rs:2197 scroll` 只记录参数 | SampleMapA 64×36、SampleMapA_1 64×36 正好 1024×576，地图铺满 viewport；281 全库仅 3 条，全在 CE225，而 CE225 无任何 210/211/300 调用者 |
| 201 SetMoveRoute 路线播放 | `game.rs:2145 move_route` 只打日志；`map_runtime.rs:789/821/1036/1055` 有移动 API | 201 全库 73 条，Map002 66、Map031 5、Map003 1；注册地图只剩 6 条 `[-2]`/`[-4]` 无形定位，没有 NPC 行走 |
| 远景视差 | 未实现；`SysDatabase` type0 遠景番号 → type13 | SampleMapA 遠景番号=2 → `Picture/bg_07_guild_a.jpg`，SampleMapA_1=1 → `cg/bg01.jpg`；type13 的 X/Y 速度全 0，且地图 3 层 tile 全铺满，远景不可见 |
| 地图触发 route / 自主移动 | `map_runtime.rs:1075 events_at`、`:1088 event_at`、`:1098 npc_at`、`:1118 contact_events_at` 基本只被测试调用 | 9 个 .mps/134 页里 133 页 `route_mode=0`，唯一 `route_mode=3` 的 TitleMap ev2 无 graphic、route_length=0 |
| 任意 opcode 面 | 主 dispatch 覆盖零售并集 50 个；implemented-not-retail 只有 opcode 0（空指令） | 没有“实现了零售不用”的 opcode 大块；多余集中在行为层，不在这 42 个 arm |

**处理原则**：这些 API 已经存在且被 synthetic 测试使用，本轮不删（避免大范围回归）；但**不再扩展示例地图/通用行为，也不把它们写进剩余内容**。若以后要瘦身，优先摘掉 `game.rs:1384 advance_player_input()` 这个唯一接进零售帧循环的行走钩子，并同步改写 `crates/wolf-game/tests/m11_npc_interaction.rs` 的帧级测试；这属于代码清理，不属于游戏流程缺口。

## 4. 示例/非零售资产（不作为缺口）

| 资产 | 证据 | 判定 |
| --- | --- | --- |
| Dungeon / SampleMapB | 有行走图和 touch 页，130 目的地 map 32/未注册，不在注册地图 | WOLF 编辑器示例 |
| Map002 | 4 条 130 `[-2,2000007,1100005,0,1]` 目的 map=0=TitleMap；未注册 | WOLF 编辑器示例 |
| Map024 / Map031 | `has_tile_data==false`、0 layers；Map031 130 目标 27 未注册；无自然 130 入口 | 非当前流程；只能存档/调试直达 |
| TitleMap | **不是**死资产：`game.rs:723-742 boot()` 实测 TitleMap event2 → CE549 三 splash → 按键等待 → CE42 | boot/splash 入口，必须保留 |
| H act4/5 | 自然帧 CE402 pc208 早退到 tail pc509，CE410 acts=[1,1,2,1,3]；act4/5 只在 budget≤5 饥饿观测出现 | 无零售存档证据；不计必做 |
| 3–4 人队伍 UI | `M11_PARTY_LIVE_BRIDGE.md:31`：CData type18 row4 col0=2，真实队伍 2 人 | 数据不支持，不计缺口 |

## 5. 工具/保真项（不是游戏流程）

- PRNG 单 tick 完全重放：零售 `GetTickCount` 零状态会重读时钟，单 tick 重放不成立；`EVENT_RESEED` 足够。
- `wolf-package` 校验/组装：发布工具，不是运行流程。
- `FrameBudgetReport`：性能测量工具，不是运行流程。
- 零售时钟种子、反汇编记录：保真证据，不是玩家可见内容。

## 6. 真正的剩余（只保留零售流程/还原度）

1. **战斗完整玩家输入**：2 人真实菜单已驱动 3 个 CE242 body；取消/返回/连续确认的 UI 验收仍可补（不是 3–4 人）。
2. **点触命中框与事件格坐标一致性**：SampleMapA 6 个 confirm 页（ev11/14/18/21/22/24）与底部 UI 命中区。
3. **视觉像素**：锚点 5/6 的零售像素 diff；关键场景截图 diff。
4. **音频人工听感**：31 首 loop 的 seam/音量/延迟。
5. **存档逐段全量对照**（M10 已列剩余项）。
6. **日程/多日自然推进**（若还有未覆盖边界）。

## 7. 复现命令

```bash
# 代码锚点
rg -n "WALK_STEP_FRAMES|advance_player_input|fn scroll|fn move_route" crates/wolf-game/src/game.rs
rg -n "fn (try_trigger|try_move_leader|events_at|event_at|npc_at)" crates/wolf-game/src/map_runtime.rs

# 数据事实
# 本次审计用的临时探针 tmp_audit.rs / tmp_audit_tiles.rs 已完成任务并删除；
# 需要复核时按 docs/BASELINE.json 的 map_events/ce_opcodes/mp_opcodes 计数，
# 或临时重写一个小 example 读 work/all。
```

> 临时探针与 `work/out/audit_special.txt` 均为一次性产物，不提交。
