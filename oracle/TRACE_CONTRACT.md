# Oracle Trace 契约规范（M0）

> 状态：**现行 / 冻结**
> 对应计划：`docs/godot/PLAN.md` §3.2 任务 6 与 §3.3 退出标准
> 作用：定义 Rust Oracle 与 Godot C# EventVM 对拍的唯一 trace 交换格式。

---

## 1. Trace 外层结构

Trace 文件采用标准 UTF-8 JSON（或 JSON Lines .jsonl）：

```json
{
  "schema": "wolf.trace",
  "schema_version": 1,
  "seed": 0,
  "event_id": 42,
  "event_name": "オリジナルタイトル画面Ａ",
  "total_steps": 100,
  "steps": [
    {
      "step": 1,
      "frame": 0,
      "event_id": 42,
      "pc": 0,
      "op": 150,
      "vars": {},
      "effects": [
        {
          "kind": "picture",
          "action": "draw",
          "slot": 0,
          "file": "SystemGraphic/TitleGraphic.jpg",
          "args": [30, 1, 1, 1, 255, 0, 0, 100, 0]
        }
      ]
    }
  ]
}
```

---

## 2. 字段定义

| 字段 | 类型 | 说明 |
|---|---|---|
| `step` | int | 步进序号（从 1 起递增） |
| `frame` | int | 逻辑帧号（60fps 帧数） |
| `event_id` | int | 当前执行的事件 ID（CE 或地图事件） |
| `pc` | int | 当前指令槽下标（包含空槽偏移） |
| `op` | int | 当前执行的 opcode（如 150, 210, 121, 111, 180, 401 等） |
| `vars` | object | 该步改变的变量键值对 `{ "<addr>": <new_int_value> }` |
| `effects` | array | 该步产生的可观察外部副作用列表 |

---

## 3. Side Effects 规格

每条副作用必须包含 `kind` 与 `action`：

- **picture (150)**:
  `{ "kind": "picture", "action": "draw"|"move"|"erase"|"set", "slot": <int>, "file": "<path>", "text": "<text>", "args": [...] }`
- **sound (140)**:
  `{ "kind": "sound", "action": "play"|"stop", "type": "bgm"|"bgs"|"se", "file": "<path>", "vol": <int> }`
- **choices (102/401)**:
  `{ "kind": "choice", "options": ["..."], "selected": <int> }`
- **teleport (130)**:
  `{ "kind": "teleport", "map_id": <int>, "x": <int>, "y": <int>, "direction": <int> }`
- **database (250)**:
  `{ "kind": "database", "type": <int>, "data": <int>, "item": <int>, "value": <int> }`
- **effect (290)**:
  `{ "kind": "effect", "effect_kind": <int>, "target": <int>, "args": [...] }`

---

## 4. 对拍判定规则（DoD）

1. **Step 对齐**：`event_id`、`pc`、`op` 必须 100% 逐步一致。
2. **变量对齐**：`vars` 中每个记录地址的数值必须 100% 一致（按 32 位有符号整数比较）。
3. **副作用对齐**：`effects` 序列必须语义等价（忽略空字符串与默认未设置字段）。
4. **出现分歧（divergence）时**：立即 fail-fast，报告第一个不匹配的 `step / pc / op` 与前后 5 步上下文。
