# 实机 7 大顽疾深度根因诊断与修复方案（commit `c91dc88`）

> 审阅对象：最新仓库 `c91dc88` + 实机截图（存档界面异常图、读档残框图）+ 5 张原版特写参考图（含 `032_日常_菜单_道具列表.png`）。  
> 结论：**之前多轮修复都只在“表层打补丁”，而这 7 个问题背后的真正根因全部藏在底层机制里**——包括 Godot 4 `FontFile` 内存加载陷阱、`InputRouter.held_mask` 与 `PointerPressObserved` 互相吞点击、`PictureStage` 重绘清 `color_corr` + `FLASH` 无限取模循环、以及 `EventEntered` 抑制的单帧失效。以下逐项给出精确的 `文件:行` 铁证与根治方案。

---

## 问题 1：为什么字体改了这么多次，仍然不是原版字体？

### 真正根因（三重静默失败叠加）
1. **Godot 4 内存加载 `FontFile` 的致命 API 坑（`UiSkin.gd:199-203`）**：
   ```gdscript
   var ff := FontFile.new()
   ff.data = pak_bytes
   if ff.get_face_count() > 0:
       return _configure_font_file(ff, bold, strict)
   ```
   当 `Lolita.ttf` 位于 `media.pak` 中时，代码通过 `ff.data = pak_bytes` 载入。但在 Godot 4 中，直接给新建 `FontFile` 赋 `data` 后、在尚未触发字形渲染/初始化前，**`ff.get_face_count()` 经常返回 `0`**，导致 `if` 条件失败，直接跳到第 204 行回退成 `_make_system_font(bold)`（即你看到的**微软雅黑**）！
2. **发布打包脚本路径漏检且不报错（`tools/package_release.sh:163-167` & `UiSkin.gd:97-105`）**：
   `package_release.sh` 只检查 `$ROOT/work/all/Lolita.ttf`（没查 `work/all/Data/Lolita.ttf`，且在 Git Bash/Linux 下大小写敏感，遇到 `LOLITA.TTF` 会跳过），找不到时也不报错，导致打出来的发布包里可能既没有散文件 `Lolita.ttf`，进 `media.pak` 后又撞上坑 1。
3. **静默回退无任何日志，且未检查中文字形覆盖率（`UiSkin.gd:181-182,204`）**：
   加载失败没有任何 `push_warning`，F3 调试 HUD 也不显示当前生效字体；且即使 `Lolita.ttf` 成功加载，若其字库缺少部分简体字，回退链默认回退到微软雅黑，在画面上看起来依然像“没换字体”。

### 根治方案
1. **彻底放弃 `ff.data = pak_bytes` + `get_face_count()` 判定**：若从 `media.pak` 读出 `Lolita.ttf` 字节，先落盘到 `user://cache/Lolita.ttf`（或检查已存在），再用标准 **`ff.load_dynamic_font(cache_path) == OK`** 加载！
2. **启动日志 + F3 调试 HUD 强制显影**：在 `UiSkin._build_font()` 打印实际命中来源（`loose` / `pak->cache` / `SYSTEM_FALLBACK`），并抽样调用 `ff.has_char("我".unicode_at(0))`、`has_char("妹".unicode_at(0))` 打印日志，一眼就能确认到底是不是真加载了 `Lolita.ttf` 以及原版字体文件本身是否包含简体字形。
3. **`package_release.sh` 增加硬检查**：大小写不敏感查找 `Lolita.ttf` / `LOLITA.TTF`，缺字体直接报错退出而不是静默打残包。

---

## 问题 2：存档界面为什么出现新的异常显示（`save2`/`save3` 左侧大块黑影、文字变黑）？

### 真正根因
对应你发来的存档界面截图，以及 `DECISIONS.md` 中刚合入的 **`D-60`（2026-11-21）**：
- `D-60` 刚为 `PictureStage` / `PictureShapes` 的所有子节点开启了 `use_parent_material = true`，并对 `blend == 3`（`MULTIPLY` 正片叠底）套用了 `MULTIPLY_SHADER`（`PictureEffects.gd:6-14`）。
- 存档界面 `CE94` 的 `save2`/`save3` 左侧那条垂直黑影及文字变黑，直接原因是：**`CE94` 绘制的带 `blend == 3`（正片叠底）的渐变/遮罩层，或缺失/黑屏的存档缩略图槽位（`save/SaveData02.jpg`）在 `z-order` 上叠加到了卡片底框和 `save2`/`save3` 文字上**，在 `D-60` 开启 `use_parent_material` 后直接把底层白字乘成了纯黑！
- 同时，存档界面左下角未显示对应存档截图的原因也同源：`GameRoot.gd:222-223` 仅在首次打开 `12000..12599` 时抓 `_clean_scene_image`，若玩家从标题界面打开读档页、或存档时缩略图未成功写入 `save_dir/SaveDataXX.jpg`（`GameRoot.gd:654-680`），`AssetDB.resolve("save/...")` 找不到图或落到黑屏图。

### 根治方案
1. 在本机打开存档界面时，打印 `PictureStage` 中所有 `12000..12599` 存活槽位的 `(number, file, kind, blend, tone, opacity, rect)`，精确定位覆盖在 `save2`/`save3` 左侧的那个槽位号（特别是 `blend == 3` 或 `tone == [0,0,0]` 的槽位）。
2. 检查 `PictureStage._render_picture()`：当槽位 `file` 为空或为缺失的 `save/*.jpg` 缩略图时，**不要生成不透明深色 `placeholder_color` 占位盒**（`PictureStage.gd` 缺图占位框若带 `blend=3` 会把整块区域乘黑！），缺省缩略图应保持透明。
3. 验证 `save_dir` 中是否实际生成了 `SaveData02.jpg` / `SaveData03.jpg`（`needs-runtime`），确保 `GameRoot._on_pro_feature(kind=0)` 写入的路径与 `CE94` 读取的 `file` 字符串完全一致。

---

## 问题 3：光标处存档栏为什么是“闪烁”而不是稳定高亮？

### 真正根因（双源码铁证）
1. **`PictureStage.gd:985-1005` 重绘清空 `color_corr` / `effects`**：
   在 `CE94` 悬停循环中，脚本反复重绘当前高亮槽位（`150` + `290`）。看 `PictureStage.gd:1000-1005`：当 `redraws == true` 且 `same_position == true` 时，代码只保留了 `shake`/`zoom`/`breath`/`pattern`，**漏掉了 `color_corr` 和 `effects`**！结果每次重绘都把高亮清零（变暗），紧接着 `290` 又把它点亮（变亮），肉眼就是高频闪烁！
2. **`PictureEffects.gd:44` 对 `kind == 0`（单次闪光 `FLASH`）做了无限取模循环**：
   `tone_color()` 的 `else` 分支写的是 `var phase := age % duration`，且 `kind == 0` 播完后永不移除，导致单次闪光或过渡高亮变成锯齿波周期闪烁！

### 根治方案
1. 在 `PictureStage.gd:1000-1005` 的 `if same_position:` 块中，**补齐保留 `prev["color_corr"]` 与 `prev["effects"]`**（除非新 `150` 显式带色调覆盖标志）。
2. 在 `PictureEffects.gd:43-49` 修正 `kind == 0`（FLASH）：当 `age >= duration` 时停止并清理该单次特效，不再对 `age % duration` 无限循环；仅 `kind == 5`（BLINK）保持三角波循环。

---

## 问题 4：读档后卧室正中央依然残留灰黑色半透明矩形文本框

### 真正根因
看你发来的第 3 张实机截图：画面正中央那个带白边的深灰半透明空框**不是 `MessageWindow`**（`MessageWindow` 是黑色羽化阴影、无白边），而是 **`SystemFile/WindowBase.png` 画出来的 Picture 槽位（`kind=4` 窗口图）**！
- `GameSession.cs:85-86` 目前只过滤了 `12000..12599`（`SaveUiPictureFirst = 12000`, `SaveUiPictureLast = 12599`）。
- 既然读档后这个框依然出现，说明它在**存档写入瞬间被画在了 `12000..12599` 之外的槽位号上**（例如 `CE14`/`CE94`/`CE131` 弹出的“保存完了/确认”窗口槽位在 `12600+` 或 `<12000`），因此逃过了 `IsSaveUiPicture` 过滤，被永久写进了 `save_XX.json` 的 `pictures` 数组里，每次读档都会被 `RestorePictures` 原样重放出来！

### 根治方案
1. 打开你当前测试用的 `save_02.json` / `save_03.json`，在 `"pictures"` 数组里直接搜 `kind == 4` 或坐标在画面中央（约 `y=300..400, w>600`）的条目，拿到它的**精确槽位号 `number`**。
2. 将该槽位号（或存档瞬间处于 `IsSaveUiEvent` 期间创建的临时窗口槽位）纳入 `GameSession.IsSaveUiPicture()` 过滤，同时在 `LoadGameFrom` 的 `RestorePictures` 也过滤该槽位，让旧存档立刻干净。

---

## 问题 5：点击“道具”图标后，道具列表依然闪现消失；且显示位置对比原版仍有偏差

### (A) 闪现消失的真正根因（`GameRoot.gd:321,338-343`）
```gdscript
func _on_event_entered(_event_id: int, is_root: bool) -> void:
    if is_root or not _action_input_pending or router == null:
        return
    if router.action_input_held():
        router.suppress_action_until_release()
```
这条抑制逻辑在真实点击“道具”图标时**几乎必然失效**：
1. **单帧过期断点**：玩家第 N 帧点击道具图标时，`_on_action()` 置 `_action_input_pending = true`，但第 N 帧末 **`GameRoot.gd:321` 立刻把它清成 `false`**！而卧室菜单的并行事件检测到点击后，往往在第 N+1 帧才真正进入 `CE128`（道具列表）；此时 `_action_input_pending` 已经为 `false`，直接 `return`，**抑制根本没启动**！
2. **`is_root` 断点**：如果道具列表事件作为无父级事件启动（`is_root == true`），`if is_root` 直接跳过！
3. 人类一次鼠标点击按住约 80–150ms（**5–9 帧**），抑制没生效导致道具列表打开后立刻读到左键仍按着，瞬间关闭！

### (B) 位置偏差的真正根因
对比原版截图 `032_日常_菜单_道具列表.png`：原版道具列表底框位于 `x≈108, y≈148, w≈460, h≈390`。当前位置偏差由两处叠加导致：
1. **字体退回微软雅黑**，导致 `kind=2` 文本图的测量宽高与原版 `Lolita.ttf` 不同，进而通过 `anchor_position` 影响居中/定位偏移；
2. 需核对 `CE128` 绘制道具列表底框与文字槽位时的 `anchor` 与 `args[5..6]`（是否为 `kind=4` 窗口叠加 `kind=2` 文本，以及 `PictureStage.gd` 对 `kind=4` 的锚点计算是否与原版一致）。

### 根治方案
1. **彻底修复闪现消失**：修改 `GameRoot._on_event_entered`——**移除 `not _action_input_pending` 的单帧限制，并对所有前景事件生效**（只要非并行事件首次进入执行槽位时 `router.action_input_held()` 为真，就立即调用 `router.suppress_action_until_release()`，且同步把 `vm` 的 `_pointerPressFrame` 清零）。这样无论菜单事件延迟几帧进入道具列表、无论是不是子事件，只要手还没松，绝不会秒关！
2. **修复位置偏差**：先修好问题 1（真 `Lolita.ttf`），再在打开道具列表时打印 `CE128` 对应槽位的 `number, kind, anchor, args, rect` 与原版 `032_日常_菜单_道具列表.png` 像素坐标逐一核对。

---

## 问题 6：存档/读档时为什么通常需要点击很多次才能成功？

### 真正根因（`InputRouter.gd:165` + `VmController.cs:789` + `GameRoot.gd:332-334` 的致命互吞）
这是本次排查最隐蔽、最关键的底层输入 Bug：
1. **`InputRouter.gd:164-166` 漏掉了 `pointer_pressed`**：
   ```gdscript
   if allow_mouse:
       if _click_latch:
           mask |= MASK_OK
   ```
   `held_mask()` 只认一帧就会清空的 `_click_latch`，**不认持续按住的 `pointer_pressed`**！
2. **`VmController.cs:789` -> `GameRoot.gd:332-334` 先手清掉了 `_click_latch`**：
   只要同一帧内**任何一个并行事件或鼠标检测先执行了 `opcode 124 EngineState(7)`**，`VmController` 就立刻发出 `PointerPressObserved` 信号，`GameRoot` 收到后马上调用 `router.clear_click_latch()` 把 `_click_latch` 清成 `false`！
3. **灾难后果**：紧接着 `CE94`/`CE131` 存档栏逻辑执行 `opcode 123`（或等 1 帧防抖后再执行 `123`）时，`held_mask()` 因为 `_click_latch` 已被清掉且不看 `pointer_pressed`，**直接返回 `0`（未按下）**！玩家只能反复狂点，直到某一次点击恰好在并行事件之前撞上 `CE94` 的 `123` 轮询帧才能成功！

### 根治方案
1. 在 `InputRouter.gd:165` 将 `if _click_latch:` 改为 **`if pointer_pressed or _click_latch:`**（前提是未处于 `_action_entry_suppressed` 抑制期，第 169 行已有 `if _action_entry_suppressed: mask &= ~MASK_OK` 兜底防止穿透！）。
2. 移除 `PointerPressObserved` 对 `_click_latch` 的暴力提前清空（因为已有 `suppress_action_until_release()` 在事件切换和模态关闭边界防穿透，不再需要靠先手 `124` 去偷杀 `123` 的点击信号）。

---

## 问题 7：对话框气泡与人名牌样式为什么仍不接近原版？（基于 4 张原版特写图的像素级改造规格）

对比你发来的 4 张原版特写图（`由衣`、`妹妹`、`加奈`、`ちょっとえっちな話をした。`）与当前 `MessageWindow.gd` / `Nameplate.gd`：

| 元素 | 原版特写实测特征 | 当前实现问题 | 精确改造方案 |
| --- | --- | --- | --- |
| **对话气泡底框** | **连续平滑高斯羽化的半透明深灰圆角矩形**，四周边缘柔滑过渡到全透明、无硬台阶；单/两行紧凑贴底（距底约 14–18px） | `MessageWindow.gd:40-42` 用 4 层离散 `StyleBoxFlat` 硬叠加，出现明显“等高线台阶” | 改用 15 行 **2D Rounded-Box SDF 羽化 Shader**（中心色 `rgba(0.10, 0.10, 0.11, 0.80)`，羽化宽度 `12–14px`，圆角 `14px`），一劳永逸还原原版雾化质感 |
| **对话文字描边** | 原版 `Lolita.ttf` 纯白字 + **2–3px 实心纯黑外描边**，在黑白漫画背景上极其锐利醒目 | `UiSkin.gd:13` `TEXT_OUTLINE_SIZE := 1` 仅 1px，描边发细发虚 | 提高对话文字描边至 **`2` 或 `3` px**（配合真 `Lolita.ttf` 加载） |
| **人名牌形状与配色** | **全圆角胶囊（pill）双层边框**：外圈 `2px` 纯黑实线 + 中层 `3px` 深灰内环 + 内芯中浅灰（`rgb(0.55, 0.55, 0.57)`）；高度约 `28px`，左右完全半圆（`corner_radius = height / 2`） | `Nameplate.gd:26-29` 是单层 `corner_radius = 12` 矩形，缺少双层胶囊立体环 | `Nameplate.gd` 改用双层胶囊绘制（外黑框 + 深灰中环 + 浅灰内芯，`corner_radius = 14`） |
| **人名牌文字与位置** | 干净纯白 `Lolita.ttf` 字（**无粗黑描边**）；位于气泡**左上角外伸偏左**（左缘比对话首字更靠左，紧贴/微压气泡上边缘） | `Nameplate.gd:66` 强加了 1px 纯黑 `draw_string_outline` 使名字发脏；位置 `bubble.x + 12` 偏右 | 移除人名牌黑描边（或仅保留极淡半透明阴影），左缘对齐调整为 `bubble.position.x - 16.0`（结合原版特写微调） |

---

## 建议立即执行的修复优先级

1. **P0（先打通交互与真字体）**：问题 6（`InputRouter` 恢复 `pointer_pressed` 到 `held_mask` + 取消 `PointerPressObserved` 误杀）→ 问题 5A（`GameRoot._on_event_entered` 去单帧限制防道具列表闪退）→ 问题 1（`UiSkin.gd` 改用 `load_dynamic_font` 缓存落盘加载 `Lolita.ttf` + 启动日志验真）。
2. **P0（消灭存档/读档视觉 Bug）**：问题 3（`PictureStage` 重绘保留 `color_corr`/`effects` + `PictureEffects` 修 `kind==0` 不无限循环）→ 问题 4（从 `save_02.json` 查出中央残框槽位号并过滤）→ 问题 2（打印 `12000..12599` 槽位定位 `save2/3` 黑影与 `blend=3` 叠层）。
3. **P1（还原原版视觉质感）**：问题 7（SDF 羽化对话框 + 双层胶囊人名牌 + 加粗 2–3px 对话描边）→ 问题 5B（修完真字体后核对道具列表坐标）。

---

## 修复闭环记录（2026-11-22，见 `D-69`）

1. **问题 1（真字体 `Lolita.ttf`）**：`UiSkin.gd` 支持大小写无关目录扫描（`work/all`、`Data`、相对解包路径）及包内字体落盘 `user://cache/Lolita.ttf` 通过 `load_dynamic_font` 加载，启动打印 `[UiSkin] font=file:Lolita.ttf (cjk=true)`，并在 `PlayOverlays.gd` F3 调试窗新增 `Font=` 行；`tools/package_release.sh` 增加大小写无关查找及缺失报错保护。
2. **问题 2（存档黑影 / 黑字 / 缩略图）**：实测定位 `save1/2/3` 左半黑影与巨型立绘系 `PictureStage.picture_state(slot, 2/3)` 返回百分比 `scale=0`（而非 WOLF 要求的像素宽高），致使 `CE94` 计算 `11500 / PicW` 时触发除零保护并放大 `save_SD_*.png` 115 倍至 `23000x23000` 遮盖存档卡片；修复 `picture_state` 字段 `2/3` 返回像素尺寸后立绘恢复正常。另接通 `IHost.FileExists` / `EventVm` `<<GET_FILE_EXIST>>` 与 `AssetDB._norm_key` 去除 `../` 前缀，右下 `12502` `saveX.jpg` 缩略图已正常显示。
3. **问题 3（存档光标闪烁）**：`PictureStage._begin_draw` 同坐标重绘保留 `color_corr` 与 `effects`；`PictureEffects.tone_color` 对 `kind != 5` 一次性闪光使用 `mini(age, duration)` 钳位。
4. **问题 4（读档中央灰框 `16000`）**：定位为 `CE101`（`X[移]メニュー時文章表示`）的 `16000..16002`（`512x384` 处 `120px` 高灰框）；在 `GameSession.IsSaveUiPicture` 补充过滤 `15000..15999` 与 `16000..16999` 瞬态窗口，且 `PictureStage.picture_snapshots()` 跳过 `erase_after` 淡出中槽位。
5. **问题 5（道具列表闪退与偏移）**：`GameRoot._on_event_entered` 移除 `is_root` 与单帧 `_action_input_pending` 限制，任何非脚本前台事件入场若动作键/左键仍按下即触发 `suppress_action_until_release()` 与 `vm.SetPointerHeld(false)`；真字体回归后文字排版恢复。
6. **问题 6（存读档点击吞键）**：移除 `GameRoot` 中 `PointerPressObserved` 提前清空 `_click_latch` 的回调，保留 `InputRouter.held_mask` 单帧 `_click_latch` 契约，避免 `124 EngineState(7)` 抢先杀掉 `123` 点击响应。
7. **问题 7（气泡 / 描边 / 人名牌）**：`MessageWindow.gd` 改用 2D Rounded-Box SDF 连续羽化 Shader；`UiSkin.TEXT_OUTLINE_SIZE` 增至 `2`；`Nameplate.gd` 升级为 `28px` 高双环胶囊（黑外框 + 亮灰 rim + 暗灰内环 + 中灰底 + 纯白无粗黑边文字），左肩微压气泡上缘。
