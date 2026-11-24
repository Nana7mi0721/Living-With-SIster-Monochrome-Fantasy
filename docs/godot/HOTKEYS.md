# Windows 桌面版快捷键说明

> 适用：Godot 4.7.2 .NET，Windows x64 发布包。
>
> 键位按功能分组：显示与帧率相邻，存档/读档相邻，信息浮层相邻。

## 1. 显示与帧率

| 按键 | 功能 |
| :--- | :--- |
| `F4` | 窗口尺寸循环：最小 `1024×576` → 每次放大一倍 → 下一档超出屏幕可用区域时进入全屏 → 全屏再按回最小窗口 |
| `Shift+F4` | 切换缩放过滤：Integer / Fit-Nearest / Fit-Linear |
| `F5` | 帧率循环：60 → 30 → 15 fps，并写入 `user://display_settings.cfg` |
| `Shift+F5` | 开关窗口标题栏的实时帧率信息 |
| `F11` | 全屏 ↔ 最小窗口 `1024×576` |

说明：

- “最小窗口”指游戏内容区 `1024×576`；Windows 标题栏和边框不计入该尺寸。
- `F4` 从当前窗口尺寸开始翻倍。例如 4K 屏幕上是 `1024×576 → 2048×1152 → 全屏 → 最小`；1080p 屏幕上 `2048×1152` 已超出可用区域，因此直接 `1024×576 → 全屏 → 最小`。
- `F11` 从全屏退出时固定回到最小窗口，不恢复上一次任意窗口尺寸。

## 2. 存档

| 按键 | 功能 |
| :--- | :--- |
| `F6` | 快速保存到 `user://quicksave.json` |
| `F7` | 快速读取 `user://quicksave.json` |
| `F8` | 打开存档导入 / 导出 |

`F6/F7` 是相邻的相反操作；`F8` 紧邻存档组，避免与画面快捷键混用。

## 3. 信息与导航

| 按键 | 功能 |
| :--- | :--- |
| `F2` | 游玩统计 / 里程碑浮层 |
| `F3` | 运行时调试浮层 |
| `F9` | 消息回顾（Backlog） |
| `F10` | 导出诊断快照到 `user://diagnostic_snapshot.txt` |
| `F12` | 返回标题确认 |
| `Esc` | 取消 / 关闭当前模态窗口 |

## 4. 游戏输入

| 按键 | 功能 |
| :--- | :--- |
| `Enter` / `Z` / `Space` / 鼠标左键 | 确认 / 推进对话 / 地图交互 |
| `Esc` / `X` / 鼠标右键 | 取消 / 返回 |
| 方向键 / `W A S D` | 移动 / 选择光标（`W/S` 在消息回顾中滚动） |
| 鼠标滚轮 / `PageUp` / `PageDown` | 翻页 / 滚动 |

## 5. 自定义快捷键

默认键位可用以下任一路径覆盖：

```text
--hotkeys=<file>
user://hotkeys.cfg
```

配置项名称：

```text
stats
debug
window_size
frame_rate
quick_save
quick_load
save_transfer
backlog
diag_snapshot
fullscreen
return_to_title
```

示例：

```ini
backlog=F9
frame_rate=F5
quick_save=F6
quick_load=F7
```

`Shift+F4` 与 `Shift+F5` 是对应基础键的固定组合，不需要单独配置。
