# 核心引擎规范与底层行为契约（docs/engine/）

本目录收归 WOLF RPG 引擎反编译规范、底层字节码指令集（Opcode）语义、二进制存档格式以及各游戏子系统的权威规格契约。

---

## 1. 核心指令集规范（Opcode Specifications）

| 文档 | 对应 Opcode | 核心职责与规范内容 |
| :--- | :---: | :--- |
| [OPCODE_SPEC.md](OPCODE_SPEC.md) | 全指令集 | **WOLF RPG 完整指令集架构总览与编码索引** |
| [OPCODE_101.md](OPCODE_101.md) | 101 | 对话消息（Message Display）与文本控制指令 |
| [OPCODE_102.md](OPCODE_102.md) | 102 | 多项分支选择（Choice Window）行为规约 |
| [OPCODE_112.md](OPCODE_112.md) | 112 | 循环与分支中断控制 |
| [OPCODE_124.md](OPCODE_124.md) | 124 | 鼠标与指针输入查询（坐标、按键状态、滚轮判定） |
| [OPCODE_210_RETURN.md](OPCODE_210_RETURN.md) | 210 | 事件调用返回与堆栈恢复 |
| [OPCODE_220_SAVE.md](OPCODE_220_SAVE.md) | 220 | 游戏存读档调用、校验和与路径处理 |
| [OPCODE_250.md](OPCODE_250.md) | 250 | 画面渲染控制与图层变换 |
| [OPCODE_290.md](OPCODE_290.md) | 290 | 数据库记录增删与查询 |

---

## 2. 数据格式与系统子系统规范

| 文档 | 涉及领域 | 核心职责与规约 |
| :--- | :---: | :--- |
| [FORMAT_SPEC.md](FORMAT_SPEC.md) | 二进制格式 | `Game.dat`、数据表、MPS 地图与 DXArchive 二进制打包格式全景 |
| [SAVE_SPEC.md](SAVE_SPEC.md) | 存档系统 | `.sav` 文件字节结构、变量库哈希、双向无损往返契约 |
| [TEXT_SPEC.md](TEXT_SPEC.md) | 文本排版 | WOLF 转义标记（`\c`、`\f`、`\v`、`\E`、`\cdb` 等）与展开器行为规范 |
| [ENGINE_EVENTS.md](ENGINE_EVENTS.md) | 事件调度 | 公共事件（Common Events）、地图事件调度状态机与并发队列 |
| [TIME_SYSTEM.md](TIME_SYSTEM.md) | 时间日程 | 游戏内时间流转、昼夜交替、作息循环与日程管理 |
| [BATTLE.md](BATTLE.md) | 战斗系统 | 回合制战斗状态机、行动调度、技能判定与结算 |
| [PARTY_DB.md](PARTY_DB.md) | 队伍数据 | 角色属性、成长曲线、队伍配置与装备数据库 |
| [AUDIO.md](AUDIO.md) | 音频系统 | BGM、BGS 与 SE 三轨独立混音、淡入淡出与音量曲线 |
| [HSCENE.md](HSCENE.md) | 特殊场景 | 差分立绘多层叠加、状态分支与场景流转契约 |
| [KEY_INPUT_SPEC.md](KEY_INPUT_SPEC.md) | 输入系统 | Opcode 123/124 按键掩码（Mask）与硬件输入对应关系实测核查 |
