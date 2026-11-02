# 文档总索引（docs/README.md）

> 本文件是仓库文档的唯一总入口。**新增、移动、归档文档时，必须同步更新本索引。**
> 当前里程碑：**M0–M12 已完成；M13 内容一致自动化验收三批完成（九结局/条件注入事件/DLC EP26–36/H 端到端/读档伪启动修复），并修复行动力条纵向缩放、HUD 调色板/文字基线 bug、新增 `wolf-layout` 布局导出工具（全量 874 passed / 0 failed / 14 ignored，`verify` PASS 70/0）**；H2–H5 人工验收已由用户全部通过（2026-11-02）。最新交接：[`HANDOFF_M13.md`](HANDOFF_M13.md)。当前路线图：[`../PLAN.md`](../PLAN.md)。

## 0. 先读什么

| 场景 | 阅读顺序 |
| --- | --- |
| 新会话接续 | [`NEW_SESSION_START.md`](NEW_SESSION_START.md) → [`HANDOFF_M12.md`](HANDOFF_M12.md) → 本索引 → [`../PLAN.md`](../PLAN.md) |
| 查格式 / 解码 | [`FORMAT_SPEC.md`](FORMAT_SPEC.md) → [`SAVE_SPEC.md`](SAVE_SPEC.md) → [`PARTY_DB.md`](PARTY_DB.md) |
| 查 opcode 语义 | [`OPCODE_SPEC.md`](OPCODE_SPEC.md) → 对应的 [`OPCODE_*.md`](OPCODE_SPEC.md) 专题 |
| 查战斗 / H / 地图 | [`BATTLE.md`](BATTLE.md) / [`HSCENE.md`](HSCENE.md) / [`MAP_AUDIT.md`](MAP_AUDIT.md) |
| 查某里程碑做了什么 | [`HANDOFF_M12.md`](HANDOFF_M12.md)（现行，M11 round 3 + M12 审计）→ [`HANDOFF_M11.md`](HANDOFF_M11.md)（M11 round 2，历史）→ 更早历史交接（见 §3） |
| 查研究/实测底稿 | [`research/README.md`](research/README.md) |

## 1. 状态图例

- **现行**：当前仍在维护、应优先相信的入口或路线图。
- **稳定参考**：技术规格/审计，除非实测推翻，否则长期有效。
- **历史归档**：已完成里程碑的记录，保留可追溯性，但不要当作当前指令。
- **研究记录**：一次性调查/验收底稿，按里程碑归档；结论以对应 HANDOFF/规格为准。
- **用户资料**：外部调研材料，不提交、不作为权威。

## 2. 入口与协作文档

| 文档 | 状态 | 用途 |
| --- | --- | --- |
| [`NEW_SESSION_START.md`](NEW_SESSION_START.md) | 现行 | 新会话启动提示，可直接复制给下一个 agent |
| [`HANDOFF_M13.md`](HANDOFF_M13.md) | 现行 | 最新里程碑交接（M13 内容一致首批验收；common-self 与 Text picture 两处修复、剩余清单） |
| [`HANDOFF_M12.md`](HANDOFF_M12.md) | 历史归档 | 最新里程碑交接（M11 round 3：自然帧 H、真实战斗菜单、PRNG 时钟种子、打包/帧预算；M12 零售范围审计 + round 1：战斗取消、confirm 页命中、存档逐字节 + day6 周末日程 + 30/90 天多日 soak） |
| [`../PLAN.md`](../PLAN.md) | 现行 | 当前路线图（M0–M12 汇总与剩余项） |
| [`../AGENT.md`](../AGENT.md) | 现行 | Lead 与子代理协作约定（并行、写范围、验收、清理） |

## 3. 里程碑交接（历史归档）

> 最新只保留 `HANDOFF_M12.md` 为现行。下列交接按时间倒序，仅用于追溯；不要从旧交接直接开始做事。

| 文档 | 状态 | 覆盖范围 |
| --- | --- | --- |
| [`HANDOFF_M11.md`](HANDOFF_M11.md) | 历史归档 | M11 三批（task-79..96）：战斗默认驱动、真实菜单/行走、H 深链、视觉/音频回归 |
| [`HANDOFF_M10.md`](HANDOFF_M10.md) | 历史归档 | M10 opcode 111 引用语义/compound、战斗重基线、残项收口；含 M11 第一批 §8 与 round 1 §9 |
| [`HANDOFF_M9.md`](HANDOFF_M9.md) | 历史归档 | 锚点/图片槽位、H 闭环、战斗驱动、移动与地图触发 |
| [`HANDOFF_M8.md`](HANDOFF_M8.md) | 历史归档 | 战斗结算闭环、H 前向链、地图角色层、模板存档 |
| [`HANDOFF_M7.md`](HANDOFF_M7.md) | 历史归档 | 战斗回合 / H 场景交互 / 零售 PRNG / 选择窗口第一批 |
| [`HANDOFF_M6.md`](HANDOFF_M6.md) | 历史归档 | 主线贯通与可玩日常循环 |
| [`HANDOFF_M4.md`](HANDOFF_M4.md) | 历史归档 | 音频后端 + 接线；含 M6 纵切片前史 |
| [`HANDOFF_M3.md`](HANDOFF_M3.md) | 历史归档 | M3 完整运行时关机前快照 |

## 4. 技术规格与审计（稳定参考）

### 4.1 数据格式 / 运行时基础

| 文档 | 状态 | 内容 |
| --- | --- | --- |
| [`FORMAT_SPEC.md`](FORMAT_SPEC.md) | 稳定参考 | 8 种二进制格式逐字节规格（Game.dat / CommonEvent / .mps / 三个数据库 / TileSetData / .wolf / .sav） |
| [`SAVE_SPEC.md`](SAVE_SPEC.md) | 稳定参考 | WOLF 存档 .sav 格式与已实现加解密 |
| [`PARTY_DB.md`](PARTY_DB.md) | 稳定参考 | CDataBase / SysDatabase / DataBase 运行时规格 |
| [`ENGINE_EVENTS.md`](ENGINE_EVENTS.md) | 稳定参考 | 引擎常驻公共事件调查 |
| [`TIME_SYSTEM.md`](TIME_SYSTEM.md) | 稳定参考 | 游戏内时间/星期/就寝/早朝 |
| [`TEXT_SPEC.md`](TEXT_SPEC.md) | 稳定参考 | 消息文本内联标记与词法契约 |
| [`AUDIO.md`](AUDIO.md) | 稳定参考 | 音频后端与接线 |
| [`TOOLING.md`](TOOLING.md) | 稳定参考 | 本机工具链清单 |
| [`ASSET_AUDIT.md`](ASSET_AUDIT.md) | 稳定参考 | 素材审计：哪些字节真正被引用 |
| [`UNVERIFIED.md`](UNVERIFIED.md) | 稳定参考 | 仍然未验证的假设清单（先看这里再声称“完成”） |

### 4.2 子系统全景

| 文档 | 状态 | 内容 |
| --- | --- | --- |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | 稳定参考（里程碑表为历史规划） | Rust 架构设计；其 M0–M6 里程碑表是 2026-10-26 规划稿，当前里程碑以 `../PLAN.md` 为准 |
| [`BATTLE.md`](BATTLE.md) | 稳定参考 | 战斗子系统全景、事件图、DB 依赖、三阶段切片 |
| [`HSCENE.md`](HSCENE.md) | 稳定参考 | H 场景子系统地图 |
| [`MAP_AUDIT.md`](MAP_AUDIT.md) | 稳定参考 | 六张注册地图结构、页面激活与可触达性 |

### 4.3 Opcode 专题

| 文档 | 状态 | 内容 |
| --- | --- | --- |
| [`OPCODE_SPEC.md`](OPCODE_SPEC.md) | 稳定参考 | 50 种 opcode 总规格（参数布局/语义/真实样例/未确认项） |
| [`OPCODE_101.md`](OPCODE_101.md) | 稳定参考 | 101 消息窗口契约 |
| [`OPCODE_102.md`](OPCODE_102.md) | 稳定参考 | 102 选择肢窗口 |
| [`OPCODE_112.md`](OPCODE_112.md) | 稳定参考 | 112 字符串条件与 111 共用块栈 |
| [`OPCODE_124.md`](OPCODE_124.md) | 稳定参考 | 124 SetVariableEx 语义 |
| [`OPCODE_210_RETURN.md`](OPCODE_210_RETURN.md) | 稳定参考 | 210/300 公共事件调用与返回值 |
| [`OPCODE_220_SAVE.md`](OPCODE_220_SAVE.md) | 稳定参考 | 220/221/222 存档三件套 |
| [`OPCODE_250.md`](OPCODE_250.md) | 稳定参考 | 250 数据库读写 |
| [`OPCODE_290.md`](OPCODE_290.md) | 稳定参考 | 290 图片效果 |

## 5. 里程碑验收基准（历史归档）

| 文档 | 状态 | 内容 |
| --- | --- | --- |
| [`M1_CLOSURE.md`](M1_CLOSURE.md) | 历史归档 | M1 标题画面闭包与像素级验收基准 |
| [`M2_SLICE.md`](M2_SLICE.md) | 历史归档 | M2 开场剧情/消息/选择窗口纵切验收基准 |

## 6. 研究记录

所有一次性调查、测试底稿与跨里程碑研究在 [`research/README.md`](research/README.md) 按 M6–M12 分组索引；M11 的 round 2/round 3、M12 零售范围审计、round 1（战斗取消/confirm 页/存档逐字节）、day6 周末日程与 30/90 天多日 soak 均已登记。外部攻略与零售脚本的内容对照见 [`research/GUIDE_CONTENT_CROSSWALK.md`](research/GUIDE_CONTENT_CROSSWALK.md)（索引/检查清单，不是规格）。研究文档不是当前指令；与 HANDOFF/规格冲突时，以现行代码与 HANDOFF 为准。

## 7. 归档

| 文档 | 内容 |
| --- | --- |
| [`archive/PLAN_2026-10-26.md`](archive/PLAN_2026-10-26.md) | 项目最初可行性研究与 M0–M5 实施规划（历史原稿，含当时的测算与风险） |
| [`archive/README.md`](archive/README.md) | 归档说明 |

## 8. 文档维护规则

1. **单一入口**：新增/移动/删除任何文档，都要更新本索引；不能只在目录里丢文件。
2. **里程碑命名**：每个里程碑结束时写 `HANDOFF_Mx.md`（现行），并把上一个交接标为历史归档。历史文件不删除、不覆盖。
3. **状态与测量时点**：技术文档开头要写清状态（已验证/未验证/历史）、测量 HEAD、数据根（`work/all` / `work/full`）与复现命令。
4. **研究文档**：放 `docs/research/`，文件名用 `Mx_主题.md` 或 `主题.md`，并登记到 `docs/research/README.md`。
5. **路线图**：`PLAN.md` 只保留当前路线与剩余项；旧规划稿进 `docs/archive/`，不在根目录保留过期版本。
6. **不提交**：`docs/gpt调研/`、`.claude/`、`work/`、临时探针/截图不解包产物，均不进版本库。
