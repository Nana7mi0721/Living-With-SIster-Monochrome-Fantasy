# 全新状态下标题封面与 CE42 菜单图核查（2026-11-20）

> **状态**：独立只读复核完成。未修改生产代码，未提交 git；临时截图、探针与二进制均放在 `work/out/deepseek_title_graphics/`，核查结束后清理。本报告是本次唯一写入仓库的文件。
>
> 核查对象：本仓库，HEAD `1989151`；原版数据根为 `<retail-root>`。截图 `游戏示例图片/截图/` 是未跟踪用户资料，只读引用，不复制、不提交。
>
> **结论置信度**：图像选择与层序为高置信；`221` 的失败路径未在本轮直接反汇编原版 `Game.exe`，但无论“失败返回 0”还是“失败不写目标”，全新 VM 的目标变量本来就是 0，因此最终分支结论不受该差异影响。

## 0. 结论摘要

1. 全新状态下，`TitleMap.mps` 事件 2 选取正号 picture `12000 = Picture/title01.jpg`（1024×576，左上角 `(0,0)`，立即出现）。这是按确认键前的可见标题画。
2. 按确认键后，`12000` 用 30 帧淡出，同时 CE42 在负号 picture `-2` 画 `SystemGraphic/TitleGraphic.jpg`（1024×576，全屏层，30 帧淡入）。CE42 最终菜单底图不是 `title01.jpg`。
3. 全新四个关键变量都是 0：`V300=V499=V503=V550=0`。CE42 因此走 `V550<10` 的 `TitleGraphic.jpg`；`V503=0` 跳过随机演出；最终把 `V499` 写成 6。菜单期实际可见的是 `TitleGraphic.jpg` 加 CE43 的三行选项图。
4. `V499=1..5` 会覆盖初始菜单底图，改为 `TitleGraphic1..5.jpg`；`V550>=10` 只在未被 `V499=1..5` 覆盖时选 `TitleGraphic6.jpg`。`V550>=2` 才是 NG+ 继承菜单门槛，`>=10` 只是标题美术分支。
5. 选“开始”后，Map003 ev1 会清掉菜单图，再按 V300 在负号 picture `-1` 画封面；**case 顺序与 TitleMap 相反**：case1=`V300==1` → `Picture/title01.jpg`，case2=`V300==0` → `SystemGraphic/TitleGraphic.png`。因此全新 V300=0 的 Map003 过渡画 `TitleGraphic.png`。
6. 旧结论有三处需要修正：`SystemGraphic/TitleGraphic.png` 不是 WOLF 示例图（示例图是 320×240 的 `SystemFile/TitleGraphic.png`）；CE42 的 `-2` 层并非不可见（`12000` 淡出后可见）；“全新走 `title01.jpg`”只适用于确认前标题待机画面，不适用于 CE42 菜单，也不适用于 Map003 过渡。

---

## 1. 全新变量初值与 `221` 缺档语义

### 1.1 变量名与默认值

变量名来自 `work/analysis/data/_sdb_names.json` 的 SysDatabase type 14「通常変数名」：

| 变量 | 名称 | 全新值 | 在标题链中的角色 |
|---|---|---:|---|
| V300 | `タイトル2` | 0 | `TitleMap` 确认前正标题图选择；CE42 里两个分支画面相同 |
| V499 | `タイトル画面演出` | 0 | CE42 菜单底图选择/随机演出结果 |
| V503 | `温泉イベントクリア` | 0 | 仅当 V499=0 时开启 20% 随机菜单底图演出 |
| V550 | `引き継ぎポイント` | 0 | `>=10` 选 `TitleGraphic6.jpg`；游戏机制的 NG+ 门槛另为 `>=2` |

依据：

- Rust VM 的普通变量是稀疏 `Vec<i32>`，未写槽读取为 0（`crates/wolf-vm/src/vars.rs:45-76`）。
- Godot/C# 侧同样以未写/越界读取返回 0（`godot/core_cs/Variables.cs:11-16`）。
- `TitleMap` 事件 2 在比较前用 `221` 读取以下值：
  - `pc33 [2000503, path(system2), 2000503, 0]`：V503；
  - `pc52 [2000300, path(system), 2000300, 0]`：V300；
  - `pc67 [2000550, path(system), 2000550, 0]`：V550。
- 这三个变量在 TitleMap 中没有任何先写后读；V499 甚至没有被 TitleMap 用 `221` 读取。因此全新进程没有其他来源把它们变成非零。

### 1.2 缺档时 `221` 的行为

现有两套宿主的实现都把“文件不存在/无法解析”当成正常空槽：

- Rust：`crates/wolf-game/src/game.rs:3216-3220`，`read_save` 失败返回 `Some(0)`，VM 把 0 写入目标变量。
- C#：`godot/core_cs/GameSession.cs:650-654`，`SaveCodec.Read(path) == null` 时返回 0。

更强的稳健性论证：即使原版 `221` 在缺档时选择“保留目标变量不写”，全新 VM 的 V300/V503/V550 本来也都是 0，且 `TitleMap` 在这三个 read 前没有写过它们。因此两种语义得到的脚本分支完全相同。

**诚实边界**：本轮没有直接反汇编原版 `Game.exe` 的 `221` 失败路径。上述结论依赖：

1. 原版脚本把缺失档位当 0/不存在使用的既有证据；
2. 两套重构宿主的实现；
3. “初值 0 + 失败不写”仍为 0 的保守论证。

---

## 2. 控制流重建

### 2.1 `op111` 头字不能当运算符

`op111` 的 `arg0`：

- 低 4 位 = 条件数；
- `0x10`（bit 4）= else/结构标志；
- 每个条件占 `[var, value, operator]` 三元组；
- operator：`0 >`、`1 >=`、`2 ==`、`3 <=`、`4 <`、`5 !=`、`6 &`。

本报告涉及的 `arg0`：

- `2` = 2 个条件，无 else 标志；
- `17 = 0x11` = 1 个条件 + else 标志；
- `18 = 0x12` = 2 个条件 + else 标志；
- `19 = 0x13` = 3 个条件 + else 标志。

实现依据见 `crates/wolf-vm/src/exec.rs:2914-2954`。

### 2.2 `TitleMap.mps` 事件 2：确认前

关键路径：

| pc | 指令 | 解释 |
|---:|---|---|
| 33 | `221 [V503, save/system2.sav, V503, 0]` | 缺 system2 → V503=0 |
| 52 | `221 [V300, save/system.sav, V300, 0]` | 缺 system → V300=0 |
| 67 | `221 [V550, save/system.sav, V550, 0]` | 缺 system → V550=0 |
| 82 | `111 [2, V300==1, V300==0]` | case 1 / case 2 |
| 84 | `150 [0, 12000, 0, ..., x=0, y=0, scale=100]` + `SystemGraphic/TitleGraphic.png` | `V300==1` |
| 87 | 同一 picture 参数 + `Picture/title01.jpg` | `V300==0` |
| 90 | `150 [4096, 12001, ..., x=512, y=520]` + `SystemFile/Button01.png` | 正号按钮/提示条，中心锚定 |
| 92 | `180 [20]` | 稳定等待 20 帧 |
| 93–109 | 键位轮询无限循环 | 确认前停在这里 |
| 111–114 | SE、等 5 帧、擦 12001、以 30 帧擦 12000 | 进入菜单转场 |
| 126 | `210 [500042]` | 调 CE42 |

参数说明：

- picture `12000`：正号层，1024×576，`(0,0)`，head `0x0000` 左上锚点，**淡入 0 帧**。
- picture `12001`：正号层，606×35，中心锚点 `(512,520)`，淡入 0 帧。
- 按键后 `12001` 立即擦除；`12000` 的擦除带 **30 帧**淡出。

### 2.3 CE42：确认后菜单底图

CE42 先在 `-2` 画初始底图，再由 V499 覆盖：

| pc | 条件 | picture / 文件 | 过渡 |
|---:|---|---|---|
| 13 | `V300==0` 或 `V300==1` | 两个 case 完全重复 | — |
| 17/20 | case 1：`V550>=10` / else | `-2`：`TitleGraphic6.jpg` / `TitleGraphic.jpg` | 30 帧淡入 |
| 27/30 | case 2：`V550>=10` / else | 同上，文件完全相同 | 30 帧淡入 |
| 35–53 | `V499==1..5` | `-2`：`TitleGraphic1..5.jpg` | 立即（0 帧） |
| 56–104 | `V499==0` | 按 V503 和随机数决定是否再覆盖 | 随机分支为 0 帧 |

要点：

- 所有 CE42 全屏底图都是负号 picture `-2`，参数为 `x=0, y=0, scale=100`；在负号全屏层语义下覆盖整个 1024×576 逻辑屏。
- `V300=0/1` 在 CE42 中没有视觉差异，只是同一段代码被编辑器生成了两个 case；差异只发生在确认前的 TitleMap `12000`。
- `V499=1..5` 会替换掉刚画的 `TitleGraphic6/TitleGraphic`，因此优先级高于 V550。
- `V499=6` 不画新底图，保留 `V550` 决定的初始底图。

### 2.4 Map003 ev1：选“开始”后的过渡

Map003 ev1 是标题菜单选择「开始」后的新游戏钩子：

| pc | 指令 | 解释 |
|---:|---|---|
| 0 | `150 [16777218, 3, 0, 0, 0, 5]` | 立即擦除菜单图片 3..5 |
| 9 | `150 [0, 4000, 30, ..., scale=110]` + `cg/blackscreen.jpg` | 画黑幕，30 帧过渡 |
| 13 | `150 [16777218, 3, 20, 0, 0, 5]` | 再以 20 帧擦除菜单图片 3..5 |
| 14 | `111 [2, V300==1, V300==0]` | case1=`V300==1`；case2=`V300==0` |
| 16 | `150 [0, -1, 0, ..., x=0, y=0, scale=100]` + `Picture/title01.jpg` | **V300==1**：负号 `-1`，立即绘制 |
| 19 | 同一 `-1` 参数 + `SystemGraphic/TitleGraphic.png` | **V300==0**：负号 `-1`，立即绘制 |
| 22 | `180 [50]` | 停 50 帧 |
| 24 | `150 [16777218, -1, 30, 0, 0, -2]` | 以 30 帧擦除 `-2..-1` 范围 |
| 28 | `130 [-1, 62, 38, 1, 1]` | 传送到 SampleMapA |

所以全新状态（V300=0）在点「开始」后的 Map003 过渡中画的是 **`SystemGraphic/TitleGraphic.png`**（负号 `-1`，0 帧淡入），不是 `title01.jpg`，也不是 CE42 的 `TitleGraphic.jpg`/`TitleGraphic6.jpg`。注意同一段还以 30 帧淡入正号 picture 4000 黑幕，所以负号 `-1` 只在黑幕完全盖住前可见。

---

## 3. 分支表

### 3.1 V300

| V300 | TitleMap 确认前 `12000` | CE42 `-2` | Map003 ev1 `-1` |
|---|---|---|---|
| 0 | `Picture/title01.jpg` | 进入 case 1；按 V550 画 `TitleGraphic.jpg` 或 `TitleGraphic6.jpg` | **`SystemGraphic/TitleGraphic.png`** |
| 1 | `SystemGraphic/TitleGraphic.png` | 进入 case 2；图像与 case 1 完全相同 | **`Picture/title01.jpg`** |
| 其他 | 不画 `12000`（只有黑底/其他已有图） | 不画初始 `-2`；随后仍可能被 V499=1..5 覆盖 | 不画 `-1` |

注意：TitleMap 是 case1=`V300==1`→`TitleGraphic.png`、case2=`V300==0`→`title01`；Map003 正好相反。`programs.json` 全量扫描中 **没有任何脚本写入 V300**。它只有 TitleMap 的 `221` 读取和两处条件读。因此 V300=1 不是正常游戏流程可累积出来的解锁；只有外部存档/补丁把 V300 置 1 才会改变这两处选图。

### 3.2 V499

| V499 | 行为 | 最终菜单底图 | 备注 |
|---:|---|---|---|
| 0 且 V503=0 | 不随机；BGM `White_Base.mp3`；写入 V499=6 | V550 决定的初始底图 | **全新状态** |
| 0 且 V503>=1，R=1..8 | BGM `White_Base.mp3`；写入 V499=6 | V550 决定的初始底图 | 80% |
| 0 且 V503>=1，R=9..10，R2=1..5 | BGM `nekomimi.mp3`；画 `TitleGraphic{R2}.jpg`；写入 V499=R2 | `TitleGraphic1..5.jpg` | 20% 进入，单个 4% |
| 1 | 直接画 `TitleGraphic1.jpg` | `TitleGraphic1.jpg` | 立即覆盖 |
| 2 | 直接画 `TitleGraphic2.jpg` | `TitleGraphic2.jpg` | 立即覆盖 |
| 3 | 直接画 `TitleGraphic3.jpg` | `TitleGraphic3.jpg` | 立即覆盖 |
| 4 | 直接画 `TitleGraphic4.jpg` | `TitleGraphic4.jpg` | 立即覆盖 |
| 5 | 直接画 `TitleGraphic5.jpg` | `TitleGraphic5.jpg` | 立即覆盖 |
| 6 | 不画新图 | V550 决定的初始底图 | 脚本的“正常标题图”值 |
| 其他 | 不画新图且不随机 | V550 决定的初始底图 | 数据外值 |

随机块逐指令：

```text
58: if V503 >= 1
60:   self10 = rand(1, 10)       # 121 [1600010, 1, 10, 0x6000]
61:   if self10 >= 9
63:     self10 = rand(1, 5)      # 121 [1600010, 1, 5, 0x6000]
64:     BGM/nekomimi.mp3
65:     按 self10==1/2/3 与 ==4/5 分支
67..86     画 TitleGraphic1..5，并写 V499=1..5
94:   else
95:     BGM/White_Base.mp3
96:     V499 = 6
100: else
101:   BGM/White_Base.mp3
102:   V499 = 6
```

`0x6000` 的语义是 **闭区间均匀随机** `rand(min,max)`，由 `Game.exe` 的 `121` 处理器反汇编/操作数形状确认；Rust 实现见 `crates/wolf-vm/src/exec.rs:2238-2242`、`crates/wolf-vm/src/rng.rs:249-270`。因此：

- `rand(1,10) >= 9` 的概率是 `2/10 = 20%`；
- 进入后 `rand(1,5)` 每张图的概率是 `1/5`；
- 对全新一次启动的总概率：`TitleGraphic1..5` 各 **4%**，`TitleGraphic6`/`TitleGraphic` 正常底图合计 **80%**（且最终多数写成 V499=6）。

`V499` 不参与 TitleMap 的 `221` 初始化。全新进程它是 0；CE42 运行后会把它写成 1..6。跨 `174 ReturnToTitle` 是否保留同一进程变量、还是由引擎重置，不在本次“全新安装”范围，见剩余风险。

### 3.3 V503

| V503 | 标题菜单影响 |
|---:|---|
| 0 | 不进入随机块；直接 `White_Base` + V499=6；不画 `TitleGraphic1..5` |
| >=1 | 仅当 V499==0 时，给随机演出开门；不是固定解锁某一张图 |

数据写点：CE426 会把 V503 写成 1；SampleMapA ev35 也会在满足条件时写 1。它在 CE436 里还作为「温泉イベントクリア」给 NG+ 点数加 2，但那是另一条语义，不影响标题分支。

### 3.4 V550

| V550 | CE42 初始底图 | 其他影响 |
|---:|---|---|
| <10 | `SystemGraphic/TitleGraphic.jpg` | 不影响 NG+ 菜单本身 |
| >=10 | `SystemGraphic/TitleGraphic6.jpg` | 若 V499=1..5，仍会被对应图覆盖 |
| >=2 | 图像选择不变（不看这个门槛） | SampleMapA ev0 弹出 NG+ 继承菜单 |

因此 `>=10` 是装饰性标题美术门槛，`>=2` 是实际上周目继承机制门槛。旧 M13 结论的这两点成立，但“TitleGraphic6 一定出现”需要加上“V499 未在 1..5 覆盖”这一条件。

### 3.5 全新状态落在哪一支

```text
V300 = 0
V499 = 0
V503 = 0
V550 = 0
```

推导：

1. TitleMap `pc82` 走 `V300==0` case 2 → `12000 = Picture/title01.jpg`。
2. 按键后 CE42 `pc13` 走 `V300==0` case 1 → `-2 = TitleGraphic.jpg`（因 V550<10）。
3. `pc35/46` 不匹配（V499=0）。
4. `pc56` 匹配 V499==0，但 `pc58` 的 V503>=1 失败 → V499=6、`White_Base.mp3`，**不重画底图**。
5. 因此菜单底图仍是 `TitleGraphic.jpg`；没有 `TitleGraphic1..6`，也没有 `TitleGraphic.png`。
6. 若选择「开始」，Map003 `pc14` 的 `V300==0` case2 → 负号 `-1 = SystemGraphic/TitleGraphic.png`（黑幕淡入前可见）。

---

## 4. 最终可见性与像素证据

### 4.1 WOLF 层序

WOLF 官方帮助（本地快照 `work/out/yadot_picture.html`，`ピクチャ番号` 一节）明确：

> 複数のピクチャが重なった時には、大きいピクチャ番号の方が前面に表示されます。

即：

```text
负号 -2  <  正号 3/4/5  <  正号 12000  <  正号 12001
```

代码交叉验证：

- Rust `Screen` 用 `BTreeMap<i32, Picture>` 升序迭代 = 升序绘制；测试明确断言 `[-2, 0, 5, 12]` 的绘制顺序（`crates/wolf-game/src/picture.rs:1101-1106`、`:2527` 附近）。
- Godot `PictureStage._sort_band_by_number()` 同样按槽号升序重排子节点（`godot/presentation_gd/PictureStage.gd:499-514`）。

实际含义：

- 确认前：`12000` 在 `-2` 上方，但 CE42 还没运行；可见的是 `title01.jpg` + 按钮提示。
- 确认后 30 帧：`12000` 淡出、CE42 的 `-2` 淡入；`3/4/5` 菜单图已存在，但其顺序低于正在淡出的 `12000`。
- 30 帧后：`12000` 被移除，`-2` 成为最底层全屏图，`3/4/5` 叠在上面。**负层完全可见**。

### 4.2 运行期复核

用无 `Save/` 的数据根 `work/retail_dat` 跑现有 Rust 运行期：

```text
wolf-cli capture --root work/retail_dat --frames 800 \
  --out work/out/deepseek_title_graphics/fresh_title_before_confirm.png

wolf-cli capture --root work/retail_dat --frames 1000 --key-frame 700:10 \
  --out work/out/deepseek_title_graphics/fresh_menu_after_confirm.png
```

结果：

- 确认前 live 图：`-2` 不存在，`12000`（title01）+ `12001`（Button01）；与 `Picture/title01.jpg` 比，排除提示条后平均绝对差约 `0.004/255`。
- 确认后 live 图：`-2`、`3`、`4`、`5`，没有 `12000/12001`；`-2` 与 `SystemGraphic/TitleGraphic.jpg` 比，排除三行菜单后平均绝对差约 `0.005/255`。
- 这与脚本分支一致，也验证了“负层在最终屏幕上可见”。

临时运行图已清理，不在仓库中保留。

### 4.3 真实截图交叉验证

只读比较 `游戏示例图片/截图/`：

**009 `009_标题前界面.png`**

- 截图尺寸 1024×612；最佳游戏内容对齐为 `y=39..612`（1024×573，底部 3 行被窗口裁掉）。
- 与 `Picture/title01.jpg` 前 573 行：灰度相关系数 **0.9021**，平均绝对差 **13.50/255**。
- 所有其他候选均明显落后：`TitleGraphic.png` 0.3039 / 77.71；`TitleGraphic.jpg` 0.2842 / 78.88；`TitleGraphic1..6` 相关系数最高仅 0.2810。
- 该截图的窗口合成/缩放/重编码造成了非零像素差，但图像身份无歧义：确认前是 `title01.jpg`。

**010 `010_标题_通关后_三行菜单.png`**

- 截图尺寸 1028×574；游戏内容为 `x=3..1027`，候选图对齐为 `TitleGraphic6.jpg` 的 `y=2..576`。
- 与 `TitleGraphic6.jpg`：灰度相关系数 **0.9698**，平均绝对差 **3.34/255**，**97.46%** 像素每通道差 ≤8。
- 排除左下三行选项区域（截图 `x=13..253, y=398..550`，即游戏内 `x=10..250`）后：平均绝对差 **0.015/255**，**99.989%** 像素每通道差 ≤8。
- 次优候选 `TitleGraphic.jpg` 仅 0.6708 / 39.81；`TitleGraphic1..5` 的相关系数最高仅 0.6526，MAE 为 39.08..43.35；`TitleGraphic.png` 0.5671 / 50.49。
- 这是像素级强匹配：该截图菜单期确实显示 `TitleGraphic6.jpg`，因此 CE42 的 `-2` 不是“不可见”，且这张截图是 `V550>=10` 的已通关状态，不是全新状态。

### 4.4 最终屏幕上到底看到什么

| 阶段 | 全屏底层 | 其上层 | 结论 |
|---|---|---|---|
| 全新启动 splash 之后、按确认前 | `Picture/title01.jpg`（`12000`） | `SystemFile/Button01.png`（`12001`） | 看到 `title01` 小屋/河谷标题画 |
| 确认瞬间起 30 帧 | `12000` 淡出，`-2 = TitleGraphic.jpg` 淡入 | CE43 菜单图 3/4/5 在两者之间 | 30 帧交叉淡化 |
| 30 帧后至选择 | `-2 = TitleGraphic.jpg` | 正号 `3/4/5 = Picture/menu1_b..menu3_b.png` | 全新菜单底图是 `TitleGraphic.jpg` |
| 已通关 `V550>=10` 的样本 | `-2 = TitleGraphic6.jpg` | 同样的三行菜单图 | 010 截图实测 |
| `V499=1..5` 的样本 | `-2 = TitleGraphic1..5.jpg` | 同样的三行菜单图 | V499 分支覆盖 |
| 选择「开始」后的 Map003 ev1 | 负号 `-1 = TitleGraphic.png`（V300=0）+ 正号黑幕淡入 | 菜单 3..5 被擦除 | 黑幕盖住前可见，随后传送 |

CE43 的三行图片位于正号 `3/4/5`，坐标 `(10,400)`、`(10,450)`、`(10,500)`，尺寸 240×52，均立即显示。CE42 原生的 `102` 选择窗口在 `Sys3=512 / Sys4=1000` 被藏到屏外；TitleMap 事后的 `Sys4=150` 不会让已打开的窗口在 CE42 返回前变成可见。因此菜单期看到的三行文字来自 CE43 图片，不改变全屏底图的结论。

---

## 5. 旧文档勘误

### 5.1 `docs/milestones/M1_CLOSURE.md`

| 旧结论 | 判定 | 说明 |
|---|---|---|
| 真实启动入口是 TitleMap ev2，不是直接 CE42 | **成立** | 本轮复核相同 |
| 确认前画面为 `Picture/title01.jpg` + `Button01`，picture 12000/12001 | **成立** | 009 截图与运行期复核均支持 |
| `V300=2000300` 由 `save/system.sav` 读取，0→title01、1→TitleGraphic.png | **仅 TitleMap 成立** | TitleMap `pc111` 的 case1/case2 确为 1→`TitleGraphic.png`、0→`title01.jpg`；Map003 `pc14` 的 case 顺序相反，不能外推 |
| “`221` 未建模时永远走 case 2 是巧合；有通关存档后可能切到 TitleGraphic.png” | **需收窄** | `221` 缺档返回 0 已有两套宿主实现；V300 在全部脚本中**没有写者**。只有外部把 V300 置 1 才会切图，正常通关流程不会自动写它 |
| “`SystemGraphic/TitleGraphic.png` 是官方默认标题图” | **需修正** | 320×240 的 WOLF 示例图实际是 `SystemFile/TitleGraphic.png`；`SystemGraphic/TitleGraphic.png` 是 1024×576 的本作同风格标题图 |
| CE42 只是“末端三选项菜单” | **不完整** | CE42 还负责选择并绘制菜单期的 `-2` 全屏封面；不是纯 UI |

### 5.2 `docs/research/M13_NGPLUS.md`

| 旧结论 | 判定 | 说明 |
|---|---|---|
| `V550>=2` 才触发 NG+ 继承菜单 | **成立** | SampleMapA ev0 pc438 |
| `V550>=10` 是装饰性标题美术门槛，不是继承门槛 | **成立** | CE42 pc15/25 的 >=10 分支 |
| “V550>=10 画 TitleGraphic6.jpg” | **成立但需前置** | 仅当 V499 不是 1..5；V499 覆盖优先 |
| “TitleGraphic6 是 NG+ 专属标题图” | **基本成立** | 代码条件确实是 V550>=10；但只在 CE42 菜单底图，不是确认前标题画 |
| TitleMap 的 V300/V503/V550 加载点 | **成立** | 参数顺序与目标变量已逐一核对 |

### 5.3 `docs/release/ASSET_AUDIT.md`

| 旧结论 | 判定 | 说明 |
|---|---|---|
| `TitleGraphic1..6.jpg` “被加载但不会显示，可以明确不做这条路径” | **错误** | CE42 的 `-2` 在标题图淡出后可见；010 截图像素级匹配 `TitleGraphic6.jpg` |
| “真实标题画面走 `Picture/title01.jpg`” | **仅确认前成立** | 确认后菜单底图是 `SystemGraphic/TitleGraphic.jpg` 或 `TitleGraphic1..6.jpg` |
| `TitleGraphic.png` 只作默认/未使用资产 | **错误** | 全新 TitleMap 不用它，但全新 Map003 过渡（V300=0 case2）实际画它；TitleMap/Map003 均有条件引用 |
| 把 `SystemGraphic/TitleGraphic.png` 当 WOLF 示例图 | **错误** | 见 5.1；WOLF 示例是 `SystemFile/TitleGraphic.png`（320×240） |
| “CE42 负层不可见/示例标题组无视觉作用” | **错误** | 负层只是被正号 `12000` 暂遮，30 帧淡出后成为菜单底图 |

### 5.4 三个重点短语的最终裁定

- **“TitleGraphic.png 未被使用”**：错误。全新标题待机画面不用它，但全新 Map003 过渡实际使用它；它是 V300 条件分支下的游戏美术，不是 WOLF 示例图。正常脚本无写入 V300 的路径。
- **“CE42 负层不可见”**：错误。负层是层序低，不是不可见；确认前被 `12000` 遮挡，擦除后完全可见。
- **“全新走 title01.jpg”**：仅指确认前。确认后全新状态走 `SystemGraphic/TitleGraphic.jpg`；Map003 过渡走 `TitleGraphic.png`。

---

## 6. 剩余风险与未决事项

1. **原版 `Game.exe` 的 `221` 失败路径未在本轮反汇编**。结论对其“返回 0 / 不写目标”的差异稳健，但精确原版实现仍是未直接取证项。
2. **V300 的写入者不在本作程序数据内**。不能排除启动器、外部工具、旧版补丁或人工存档把它置 1；本报告只证明正常脚本路径没有写入。
3. **V499 跨 ReturnToTitle 的生命周期未测**。全新安装无影响，因为进程初值必为 0；174 回标题是否重置普通变量可能影响同一进程的第二次菜单底图。
4. **V503>=1 的随机使用包含端点与概率**来自已确认的 `0x6000 = rand(min,max)` 语义；具体随机流/模偏差不影响“是否进入某分支”的结论。
5. **009 截图像素差**包含窗口合成、缩放或重编码，无法像 010 那样做到近乎逐像素相等；但候选间的相关性/MAE 分离极大，图像身份可以确定。
6. **路径大小写**：脚本使用 `save/system.sav`，实际安装目录是 `Save/`。Windows 原版不区分大小写；若移植端在大小写敏感文件系统上直接拼路径，可能把“有档”误判成“无档”，从而退回 0 分支。全新缺档场景本身不受影响。

---

## 附录 A：候选图片与哈希

均为 `work/retail_dat/` 下的只读解包资产；SHA-256 取前 16 位。

| 路径 | 尺寸 | 字节 | SHA-256 前 16 |
|---|---:|---:|---|
| `Picture/title01.jpg` | 1024×576 RGB | 280653 | `c98ea56607de9970` |
| `SystemGraphic/TitleGraphic.png` | 1024×576 RGB | 424764 | `24a096b96aab9782` |
| `SystemGraphic/TitleGraphic.jpg` | 1024×576 RGB | 358497 | `33d1a4299e0fb254` |
| `SystemGraphic/TitleGraphic1.jpg` | 1024×576 RGB | 324826 | `7f21877e4ca7ffb8` |
| `SystemGraphic/TitleGraphic2.jpg` | 1024×576 RGB | 314916 | `880d46ccf6452fe9` |
| `SystemGraphic/TitleGraphic3.jpg` | 1024×576 RGB | 304294 | `5e76d274e1d6ca9c` |
| `SystemGraphic/TitleGraphic4.jpg` | 1024×576 RGB | 310515 | `da13ccc07d199166` |
| `SystemGraphic/TitleGraphic5.jpg` | 1024×576 RGB | 303241 | `2e5604fea6401927` |
| `SystemGraphic/TitleGraphic6.jpg` | 1024×576 RGB | 337568 | `2642d1d61921c416` |
| `SystemFile/TitleGraphic.png`（WOLF 示例） | 320×240 RGBA | 61312 | `2bc2e36454b9810e` |

截图哈希：

| 截图 | 尺寸 | SHA-256 前 16 |
|---|---:|---|
| `游戏示例图片/截图/009_标题前界面.png` | 1024×612 | `86efcc84d6b98ce7` |
| `游戏示例图片/截图/010_标题_通关后_三行菜单.png` | 1028×574 | `acebe4285ca76153` |

`SystemGraphic/TitleGraphic.png` 与 `TitleGraphic.jpg` 的灰度相关系数为 0.8893、平均绝对差 12.36/255，表明二者是同一本作美术族的两个变体；它与 320×240 的 `SystemFile/TitleGraphic.png` 不是同一资产。

## 附录 B：复现要点

1. 解析 `game_data/programs.json`：
   - map id 1008 / `TitleMap.mps`，event 2；
   - common event id 42；
   - 按 `arg0 & 0x0F` 取条件数、按三元组解析 `op111`，不要把 `0x10` 当 operator。
2. 交叉检查变量：
   - `V300`：TitleMap `pc52` 读、`pc82` 选图；Map003 `pc14` 选图（case1=1→title01，case2=0→TitleGraphic.png）；程序数据无写者。
   - `V499`：仅 CE42 `pc35/46/56/68/72/76/82/86/96/102` 使用。
   - `V503`：TitleMap `pc33` 读；CE42 `pc58` 判定。
   - `V550`：TitleMap `pc67` 读；CE42 `pc15/25` 判定。
3. 层序：WOLF 官方规则“大号在前”，Rust `BTreeMap` 升序、Godot 子节点按槽号升序；负号 `-2` 在正号 `12000` 后面。
4. 截图比较时先裁窗口边框/标题栏：009 用 `y=39` 起的 1024×573，010 用 `x=3` 起的 1024×574 与候选图 `y=2` 起的 1024×574。

**最终判定**：全新确认前看 `Picture/title01.jpg`；确认后 CE42 菜单看 `SystemGraphic/TitleGraphic.jpg`；Map003 过渡看 `SystemGraphic/TitleGraphic.png`；`TitleGraphic6` 只属于 V550>=10 且未被 V499=1..5 覆盖的菜单底图；CE42 的 `-2` 是实际可见层。
