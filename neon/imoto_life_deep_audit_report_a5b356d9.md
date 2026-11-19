# `imoto-life`（`a5b356d9`）跨仓库深度复核审计报告

- **审计对象**：`imoto-life`（commit `a5b356d9b83c7bb15248bb1e0830a1e91e527588`）对照 `imouto-life`（commit `d8498378271db875cb9e8bca7d94e441ce84b48a`）
- **对照基线**：`imoto_life_cross_repo_audit_checklist.md`
- **环境边界**：云端沙箱无 Godot 4.7.2 .NET 运行时及零售 `work/Data` / `game_data/programs.json`，凡依赖零售指令分布或实机渲染表现的项均标注 `需数据核实` 或 `needs-runtime`，并已配套产出第一阶段只读扫描脚本 `audit_programs_static.py`。

## 一、复核总览与口径修正

对照 `a5b356d9` 实际源码，原清单中的大部分高危缺口均**确认仍存在**；同时本次深挖新发现 **5 处高价值隐患/细化点**，并对 **2 处原清单表述**作了精确修正：

| 类别 | 状态 | 核心结论 |
| --- | --- | --- |
| **R12 发布路径与向导修复** | `已合入` | `GameRoot.gd:422` 等加载器已统一改为 `AppPaths.package_root()`，确认无需重复排查 `R12-P1-01`/`R12-P2-01`。 |
| **`MapLayer.gd` 输入抢占** | `新增高危确认 (P1)` | `MapLayer.gd:53` 设置 `mouse_filter = MOUSE_FILTER_STOP`，且 `_gui_input`（`:254-259`）对地图内左键直接 `accept_event()`；而 `PictureStage` 全为 `MOUSE_FILTER_IGNORE`、`InputRouter` 仅监听 `_unhandled_input`。一旦进入卧室且 `map_view.visible = true`，地图区内的 Picture 按钮左键会被 `MapLayer` 拦截吞掉并误触发 `MoveLeader`。 |
| **`Opcode 121` 位域与 `CommonSelf`** | `深挖实锤 (P1)` | `EventVm.cs:333-399` 缺失 `0x01` 钳制、`0x02` f64、`0x04/0x08` 右值不求值、`0x10/0x20/0x40` 二阶指针；`Compound` 的 `0x600`/`0x700`（`:345-346`）为空操作而非下限/上限；`StoreNum`/`StoreStr`（`:260-310`）未实时双写 `CommonSelf`，仅在退出时 `PersistCommonSelf`（`:1319-1326`）整包覆盖。 |
| **`Opcode 130 / 140 / 180` 语义缺口** | `新增发现 (P1/P2)` | 对照 `imouto-life` BUG-35/37/39：`EventVm.cs:672-682`（`130 Teleport`）四参未 `EvalNum`；`EventVm.cs:715-718`（`140 Sound`）`volume/frames/tempo/loopPoint` 未 `EvalNum`；`EventVm.cs:847`（`180 Wait`）用 `Math.Max(1, frames)` 强制把 `<=0` 阻塞 1 帧。 |
| **`Opcode 124` `field 18`** | `口径修正` | 原清单称 `17/18` 在 `VmController.cs` 均返回 `0`；实测 `EventVm.cs:660` 已对 `field == 18` 返回 `inst.EventId + 500_000`（但尚未像 BUG-28 那样区分前台/并行事件），`10/11/12/17/20/21` 确在 `VmController.cs:550-559` 返回 `0`。 |
| **GBK / CP932 编码边界** | `架构差异澄清` | `imoto-life` 为 WOLF v3（`CommonEvent.dat` `VERSION = 0xC9`），原生采用 UTF-8。`converter/reader.py:84-89` 与 `LegacySaveImporter.cs:104-112` 均以严格 UTF-8 解码，失败时直接 `replace`。**不可照搬** `imouto-life` 的 GBK/CP932 优先启发式，应保持 UTF-8 优先，仅在 `UnicodeDecodeError` 时再尝试 CP932/GBK 回退。 |

## 二、VM 字节码、数值系统与事件调度逐项复核

| 优先级 | 检查项 | `imoto-life` (`a5b356d9`) 精确代码证据 | 复核状态 | 风险与核查要点 |
| --- | --- | --- | --- | --- |
| **P0** | **`AutoEvents` 电平触发 vs 边沿触发**（BUG-25） | `godot/core_cs/AutoEvents.cs:25-40, 95-136`：除 `ResumeOnLoad = { 216, 217 }` 外均为 `now && !before` 边沿触发；`Skipped = { 43 }`，`GatedCallOnly = { 242, 243 }`。 | `确认仍存在` | 无内部 `170/179` 循环的 `RunCondition == 2` 并行事件只运行一次即失效；需用 `audit_programs_static.py` 扫描全部 `rc==2` 事件，区分线性重绘事件与 `Wait(999999)+BreakEvent` 一次性初始化事件（`需数据核实`）。 |
| **P0** | **`Opcode 176`（ContinueLoop）被误作循环头**（BUG-36） | `godot/core_cs/Program.cs:100` 将 `case 176:` 归入 `BlockKind.Loop` 压栈；`godot/core_cs/EventVm.cs:534` 注释 `170/176 unconditional loop heads`。 | `确认仍存在` | 若零售脚本中存在 `176`，`Program.FromSlots` 会多压一层无配对 `498` 的循环块，导致内层循环提前闭合或跳过后续分支（`需数据核实`）。 |
| **P1** | **`Opcode 150` 参数求值掩码误伤数据库 Magic** | `godot/core_cs/EventVm.cs:788-799`：`((uint)raw & 0x0F00_0000) != 0` 直接跳过 `EvalNum`；而 `Magic.cs:41-78` 本身仅解码 `0..9_999_999`，未解码 `1_000_000_000+` DB magic。 | `确认仍存在` | 需结合脚本确认 `programs.json` 中 `150` 参数是否命中 `0x0F00_0000` 及是否存在 `1_000_000_000+` DB magic（`需数据核实`）。 |
| **P1** | **`Opcode 121` 位域与 `CommonSelf` 实时双写**（BUG-27, 29） | `godot/core_cs/EventVm.cs:260-310, 333-399, 1319-1326`：仅实现 `0x10000` 批量与基本 `Compound`/`BinaryOp`；缺 `0x01`/`0x02`/`0x04`/`0x08`/`0x10`/`0x20`/`0x40`；`0x600`/`0x700` 返回 `current`；`CommonSelf` 仅在退出时 `PersistCommonSelf` 回写。 | `深挖实锤` | 若战斗或数值脚本用到上下限（`0x600`/`0x700`）、二阶指针（`0x10/0x20/0x40`）、实数计算（`0x02`）或同 CE 并发读写 `CSelf`，会产生静默数值偏差（`需数据核实`）。 |
| **P1** | **`Opcode 125`（AutoKeys）无 TTL 与同帧消费锁**（BUG-30） | `godot/core_cs/EventVm.cs:1093-1095` 直接覆盖 `AutoKeys`，无过期帧计数与每协程每帧消费标记。 | `确认仍存在` | 注入键可能长期粘死或被先执行的协程漏读（`需数据核实`）。 |
| **P1** | **`Opcode 151`（ChangeColor）`frames` 未求值**（BUG-39） | `godot/core_cs/EventVm.cs:1101-1107`：`int frames = slot.Params.Length > 1 ? slot.Params[1] : 0;` 未调用 `EvalNum`。 | `确认仍存在` | 首参打包 RGB 必须保持 raw，第 2 参 `frames` 若传入变量 magic 会被当作百万帧处理（`需数据核实`）。 |
| **P1** | **`Opcode 211`（Reserve）为空桩**（BUG-32） | `godot/presentation/VmController.cs:720`：`public void Reserve(int target) { }`；`EventVm.cs:975-990` 也未解析实参。 | `确认仍存在` | 若菜单、剧情或战斗依赖预约公共事件，预约将静默丢失（`需数据核实`）。 |
| **P1** | **`Opcode 124` 状态查询缺口**（BUG-28, 44, 47） | `EventVm.cs:660` 仅对 `field == 18` 返回 `inst.EventId + 500_000`（未区分前台/并行）；`VmController.cs:550-559` 仅实现 `7/8/9`，`10/11/12/17/20/21` 返回 `0`；`PictureStage.gd:200-201` `field 2/3` 返回 scale 百分比而非像素尺寸。 | `确认并修正口径` | 需按 `audit_programs_static.py` 输出的 `(sub, field)` 频次表优先补齐高频缺失字段（`需数据核实`）。 |
| **P1/P2** | **`Opcode 130 / 140 / 180` 参数求值与零帧等待**（BUG-35, 37, 39） | `EventVm.cs:672-682`（`130`）四参未 `EvalNum`；`EventVm.cs:715-718`（`140`）`volume/frames/tempo/loopPoint` 未 `EvalNum`；`EventVm.cs:847`（`180`）强制 `Math.Max(1, frames)`。 | `新增发现` | `130`/`140` 若传入变量 magic 将产生非法坐标或音量；`180 Wait(0)` 多停 1 帧可能引入微小时序偏移（`需数据核实`）。 |

## 三、表现层、渲染管线与输入架构复核

| 优先级 | 检查项 | `imoto-life` (`a5b356d9`) 精确代码证据 | 复核状态 | 风险与核查要点 |
| --- | --- | --- | --- | --- |
| **P1** | **`MapLayer.gd` 左键拦截抢占 `InputRouter`** | `MapLayer.gd:53` `mouse_filter = MOUSE_FILTER_STOP`；`MapLayer.gd:248-259` 左键 `accept_event()`；`GameRoot.gd:421` 进图即设 `map_view.visible = true`；`PictureStage.gd:58,65` 全为 `MOUSE_FILTER_IGNORE`；`InputRouter.gd:53` 监听 `_unhandled_input`。 | `新增高危确认` | Godot 先派发 `_gui_input` 再派发 `_unhandled_input`。卧室阶段点击画面上任意 Picture 按钮时，`MapLayer` 会先截获左键并调用 `accept_event()`，导致 `InputRouter` 漏收点击且误触发 `MoveLeader`（`needs-runtime`）。 |
| **P1** | **`Opcode 160/161/162` 转场与 `Opcode 290` 特效未渲染** | `GameRoot.gd:357-362` `_on_transition` 与 `_on_effect` 仅调用 `status(...)` 输出日志。 | `确认仍存在` | 缺少画面冻结/淡入淡出转场、闪屏、震动与 `290` 坐标偏移，可能导致特效丢失或界面文字重叠（`needs-runtime`）。 |
| **P1** | **`PictureStage.gd` 缺淡入淡出、移动补间与动画保留** | `PictureStage.gd:82-115, 342-365`：`Draw`/`Move` 直接瞬时赋值 `position`/`modulate`，`Erase` 直接 `queue_free()`。 | `确认仍存在` | 黑屏转场与侧拉栏会瞬变闪跳，无帧级插值（`needs-runtime`）。 |
| **P1** | **`PictureStage.gd` 缺混合模式与纵向独立缩放** | `PictureStage.gd:354-364` 仅处理单轴负缩放水平镜像（`scale.x = -1`），未解析 `head` 中的 `blend`（`0..3`），也未区分独立 X/Y 缩放。 | `确认仍存在` | 加算/减算/乘算光效与阴影无法正确混合（`needs-runtime`）。 |
| **P1** | **负号与 `>=12000` 图片强制全屏拉伸** | `PictureStage.gd:12, 226, 268, 295, 347, 357` 全部将 `number < 0 or >= 12000` 强制设为 `SCREEN_SIZE`（`1024x576`）且 `position = Vector2.ZERO`。 | `确认仍存在` | 若零售脚本中存在非全屏尺寸的负号 HUD 图标或局部遮罩，将被错误拉满全屏并错位；可用 `audit_programs_static.py --assets` 实测确认（`需数据核实`）。 |
| **P2** | **对话框与音频恢复细节** | `MessageWindow.gd:58-92` 已有手绘打字机与 `@n` 头像解析，但未接入消息回顾缓冲；`AudioManager.gd:33-146` 已实现 `loop_point`/`tempo`/淡入淡出及 `snapshot/restore`，但读档 `restore` 传入 `frames=0` 且需实机核对循环点与淡出中断表现。 | `部分就绪 / needs-runtime` | 需实机验证带立绘对话遮挡、打字中取消键及读档后 BGM/BGS 循环点衔接。 |

## 四、随机性、存档安全、编码与交互体验复核

1. **实机 RNG 固定种子（P1，确认仍存在）**
   - `WolfRng.cs:8, 22, 41` 默认 `EventSeedDefault = 1`，全仓仅在测试中调用 `SetEventSeed`，`GameSession.cs` 新开局与 `GameRoot.gd` 实机启动未按系统时钟扰动种子。
2. **故障弹窗与未来确认弹窗的释放守卫（P1，部分存在缺口）**
   - `GameRoot.gd:572-575, 619-622` 在 `restart_after_fault()` / `recover_to_last_save()` 清除了 `router` 的按下状态与帧锁存，但未像 `imouto-life` `GameView.gd:410, 700-712` 那样建立物理按键松开前持续屏蔽输入的 `_modal_release_guard`，在物理鼠标左键尚未抬起的一帧内仍可能由下一帧事件穿透底层。
3. **编码边界（已澄清，不盲目移植）**
   - `converter/reader.py:84-89` 与 `LegacySaveImporter.cs:104-112` 针对 WOLF v3 原生 UTF-8 编写；建议仅在 `UnicodeDecodeError` / `DecoderFallbackException` 分支中增加 CP932 → GBK 的温和回退，保留原告警日志，切勿改为 GBK 优先以免破坏合法 UTF-8。
4. **便利热键与存档防误触（P2，确认缺失）**
   - `GameRoot.gd:462-485` 仅支持 `F5`/`F9`，尚未接入 `F11` 无边框全屏切换、`F4` 消息回顾（200 条内存环形缓冲）、前端「返回标题」确认弹窗与缩放模式切换。

## 五、本地执行入口

请先在本地仓库根目录运行配套产出的一阶段静态摸底脚本，再依据实测计数决定第二阶段修复优先级：

```bash
python3 audit_programs_static.py \
  --programs game_data/programs.json \
  --assets work/Data \
  --out-dir reports/static_audit
```

输出文件：`reports/static_audit/static_audit_summary.md`（审阅摘要）与 `reports/static_audit/static_audit_report.json`（全量站点明细）。
