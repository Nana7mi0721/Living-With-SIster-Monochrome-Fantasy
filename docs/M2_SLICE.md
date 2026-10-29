# M2 纵切验收基准（开场剧情 · SampleMapA 事件 0）

> 目标读者：负责把《Living With Sister Monochrome Fantasy》的 `M2 垂直切片` 在 Rust 运行时里跑通、并判断
> "还算不算忠实还原"的人。本文是 **M2 的验收基准**：入口是哪条脚本、屏幕上该出现什么、每个数字由哪条命令产出。
> 标记沿用 docs/FORMAT_SPEC.md 的规范：**已验证** / **未验证** / **★已实测纠正**。

**与 M1 的关系**：M1 的验收对象是"启动链路 + 标题画面"，基准是 docs/M1_CLOSURE.md（标题终态与零售 PrintScreen
mean abs 0.012）。M2 的验收对象是**玩家真正开始游戏后的第一屏**——包括引擎自己画的消息窗口（101）与选择窗口（102），
这两条路径是 M1 完全没有触碰的部分。

---

## 0. 测量时点与命令索引

### 0.1 测量时点

| 项 | 值 |
|---|---|
| git HEAD | `51bd7a3` + **第二批 M2 提交**（250 写路径 / 移动插值 / 离屏裁剪 / 引擎事件 12 个 / `.project` 名字）+ **第三批**（`112` 开块 / `290` 十一种效果 / `blocks` 诊断命令 / `--answer-delay`）+ **第四批**（`69d7b9c`：暂停光标贴图 / 鼠标选项命中 / `210` 字符串实参 / **`121` 运算符表纠错** / 消息原点 410）+ **第五批**（`8825723` 存档 220/221/222 与精灵表 pattern；`ea96c1f` 290 效果实参求值） |
| 数据根 | `work/full`（23 个 .wolf 全解，4,085 文件） |
| 屏幕 | 1024x576，60 fps，tile 16 px |
| 工具链 | Rust 1.99.1 msvc；`cargo test -q --workspace` 全绿；`wolf-cli verify` 63/63 |

### 0.2 命令索引（本文所有数字的唯一来源）

| 编号 | 命令 | 产出 |
|---|---|---|
| C1 | `wolf-cli capture --map-run SampleMapA --event 0 --frames 600 --out work/out/base600.png` | 选择肢状态（§2.1） |
| C2 | `wolf-cli capture --map-run SampleMapA --event 0 --frames 400 --hold-message --out work/out/base400.png` | 消息窗口状态（§2.2） |
| C3 | `wolf-cli run --map-run SampleMapA --event 0 --frames 600` | 无渲染的纯解释执行（指令数交叉校验） |
| C4 | `wolf-cli verify` | 全量加载器回归（63 检查） |
| C5 | `cargo test -q --workspace` | 单元/集成测试 |
| C6 | `python work/_p_a103.py`（150 指令直方图） | §3.1 的 16,716 / 1,115 数字 |
| C7 | `cargo run -p wolf-render --example imgdiff -- A.png B.png` | 像素比对（M1 遗留工具） |
| C8 | `cargo run -p wolf-format --example dbdump -- work/full DataBase 16` | 数据库单元格转储 |
| C9 | `wolf-cli capture --map SampleMapA` | 地图渲染（27,648 quad）——M2 不再作为验收项，见 §5.3 |
| C10 | `wolf-cli capture --frames 716 --engine-events '' --out a.png` 与 `--engine-events 271 --out b.png` 再 `imgdiff a.png b.png` | §4.2 的常时协程 A/B |
| C11 | `wolf-cli capture --map-run SampleMapA --event 0 --frames 600 --engine-events <id>` | 单个常时协程的影响面 |
| C12 | `wolf-cli capture --map-run SampleMapA --event 0 --frames 900 --answer 3 --answer-delay 1 --out work/out/m2story900.png` | 剧情态（§2.3） |
| C13 | `wolf-cli blocks --map SampleMapA --event 0` | 块表诊断（§3.7） |
| C14 | `wolf-cli precompile` | 块栈计数器（§3.7） |
| C15 | `wolf-cli capture --load-save work/retail_backup/Save/SaveData01.sav --frames 1 --out work/out/save1_load3.png` | 存档载入态（§2.4） |

---

## 1. 纵切是什么：入口与两个稳定态

### 1.1 入口

**已验证**：M2 纵切的入口是 `MapData/SampleMapA.mps` 的**地图事件 0 第 0 页**（trigger = 1 autorun）。
它不是"某个随机地图事件"，而是本作真正的开场：第 0 页 846 槽，先调 `common event 221`（ローディング表示）画
加载画面，再进入叙述文本 + 四选项（"広阔的战场 / 广袤的森林 / 微风吹拂而过的麦田 / 有人在等我的家"）。

**已验证**：本作**没有可行走地图**。6 张已注册地图（TitleMap / SampleMapA / SampleMapA_1 / Map003 / Map024 /
Map031）的页面图形全部为空，只有未注册的引擎示例地图（Dungeon / Map002 / SampleMapB）才有角色 chip。
因此 PLAN.md 里 M2 的"角色 chip 8 向行走 + 碰撞"一项是**死重**，已从验收表移除（见 §5.3）。

### 1.2 两个稳定态（这就是 M2 要交付的两帧）

| 状态 | 帧号 | 画面内容 |
|---|---|---|
| **A 消息窗口态** | 400（`--hold-message`） | 底部灰皮消息窗 + 7 行描边 CJK 叙述 + ▼ 暂停光标 |
| **B 选择窗口态** | 600 | 中部蓝皮选择窗 + 4 个完整 CJK 选项 + ▶ 光标；底部同一段叙述（这里是**文本图片**，不是 101） |

两个状态都不是"跑一会儿截一张"的产物：A 是"事件正在等玩家按键"，B 是"事件正在等玩家选一项"——
两者都是 retail 下玩家能长时间停留的状态，因此可以用真实截图逐像素对照（§6.1）。

**第三个稳定态（第三批新增）**：答完四选项之后脚本继续前进，进入**床边场景**（HUD 顶栏 + 方针/状态面板 +
房间 + 妹妹立绘），见 §2.3。它证明纵切不再"卡在开场选项"，而是真的把剧情推进到了游戏主界面。

---

## 2. 实测数字（验收用的硬指标）

### 2.1 选择窗口态（C1）

```
pictures  : 21 live -> 10 textured quad(s), 4 text picture(s)
choice    : HELD on prompt with 4 option(s) ["广阔的战场", "广袤的森林", "微风吹拂而过的麦田", "有人在等我的家"], cursor 0 at anchor (512, 192)
          : engine rect [512.0, 192.0, 242.0, 148.0], 4 row(s)
  text pic   -100 at [526,206,214,30] ▶ 广阔的战场
  text pic   -100 at [526,236,214,30]    广袤的森林
  text pic   -100 at [526,266,214,30]    微风吹拂而过的麦田
  text pic   -100 at [526,296,214,30]    有人在等我的家
text      : 4 item(s), 37 glyph(s), 4 line(s), 0 substituted
rendered  : 1024x576, 35816/589824 non-black pixel(s) = 6.1%
stats     : 29367 instruction(s), 750 picture op(s), 1 sound(s), 128 db access(es) (15 write, 11 label), 2 message(s), 1 choice(s), 0 warning(s)
```

> **口径**：22 个活图片里 19 个是第三方气泡库（event 98，由常时协程 271 启动）停在库自己的停车场
> `(-2784, 2880)` 的控件；`picture::on_screen` 判定它们碰不到屏幕，于是 10 个 quad 才是真正画出来的量。
> 早先记录里的 `3 live -> 19 quad / 200,260 px / 46,370 指令` 是**离屏裁剪之前**的数字，已作废。

- **已验证**：选择窗锚点 `(512, 192)` 来自 Sys3/Sys4；面板矩形 `[512, 192, 242, 148]`
  （宽 = 最长选项 `微风吹拂而过的麦田` 9 个全角字 × 20 px（Sys8）+ 光标 30 + 内边距 28 + 4 slack = 242）。
- **已在 M2 修正的错误**：面板宽度曾按 `CHAR_WIDTH = 16` 估算而绘制字号是 20/22，导致最长选项被折行、
  第二行覆盖下一行（`微风吹拂而` 伪影）。现在面板与字形共用 `choice::TEXT_PX`，不可能再漂移。
- **已在 M2 修正的错误**：选择窗曾被居中绘制。docs/OPCODE_102.md §4.2 明确 Sys3/Sys4 是
  **屏幕绝对像素的左上角**且**没有居中默认值**；标题事件正是靠把 Sys4 写成 1000（屏幕外）来让引擎不画窗。
- **已验证**：`text pic -100` 是引擎自己给选择窗内容分配的图片槽，脚本不参与。

### 2.2 消息窗口态（C2）

```
pictures  : 22 live -> 10 textured quad(s), 0 text picture(s)
    pic 999    WindowFile at (0, 0) alpha 0.00 window Some((100, 50)) file "<SQUARE>"
    pic 19500  WindowString at (512, 480) alpha 1.00 window Some((960, 168)) file "SystemFile/WindowBase.png"
message   : 7 row(s), 73/73 char(s), font 20 at [64,410], wrap 928, cursor at [576,554]
text      : 7 item(s), 68 glyph(s), 7 line(s), 0 substituted
rendered  : 1024x576, 161282/589824 non-black pixel(s) = 27.3%
stats     : 30619 instruction(s), 2394 picture op(s), 0 sound(s), 98 db access(es) (13 write, 9 label), 1 message(s), 0 choice(s), 0 warning(s)
```

> 这里的 `22 live` 与 §2.1 同源：19 个气泡库控件停在 `(-2784, 2880)`，被裁剪掉，真正绘制的是 10 个 quad（九宫格 9 + 暂停光标 1）。

面板九个九宫格 quad（C2 输出节选）：

```
quad pic  19500 at [32,396,3,3]     quad pic 19500 at [35,396,954,3]    quad pic 19500 at [989,396,3,3]
quad pic  19500 at [32,399,3,162]   quad pic 19500 at [35,399,954,162]  quad pic 19500 at [989,399,3,162]
quad pic  19500 at [32,561,3,3]     quad pic 19500 at [35,561,954,3]    quad pic 19500 at [989,561,3,3]
```

| 量 | 值 | 来源 | 标记 |
|---|---|---|---|
| 面板矩形 | `(32,396) 960x168` | ev63 pc90 `150 [4160, 19500, 5, self38, self39, 1, 255, self32, self33, 100, 0, 1600009]`，锚点 0x1000 = 中心 (512,480) | 已验证 |
| 面板皮肤 | `SystemFile/WindowBase.png`（灰，18x18，3 px 边） | ev63 pc25 `250 [18, 0, 24, 332288, 1600009]` → 引擎单元格 (18,0,24) | 已验证 |
| 文字原点 | `(64, 410)` | Sys1 = 2·rateX = 64，Sys2 = 17·rateY + trunc(rateY/10) = 410 | 已验证 |
| 字号 | `20` | Sys8 = `rateY*12/10 - 8`（ev48 新処理分支） | 已验证 |
| 行距 | `26` | Sys8 + Game.dat `text_line_spacing` = 20 + 6 | 已验证 |
| 折行宽度 | `928` = 面板右沿 992 − Sys1 64 | 由活着的面板矩形推出，不写死 | 已验证 |
| 暂停光标 | `(576, 554)` | Sys1+Sys5 = 64+512，Sys2+Sys6 = 410+144 | 已验证（坐标）|
| 暂停光标贴图 | `SystemFile/Text_Pause.png` 17x90 = 6 帧 x 17x15 | 每帧一个 16x10 下三角，帧内顶边 y = 3,2,1,0,1,3 上下弹跳；`message::cursor_frame` 选帧 + `atlas::sprite_from` 贴图 | 已验证（图）/ 未验证（每帧 8 个渲染帧的节奏）|
| 打字机 | Sys9 = 500 字/秒，Sys10 = 9 帧预等待 | ev48 pc117 / ev63 pc36 | 已验证（速率）/ 未验证（积累方式为浮点累加） |
| 文本来源 | `101` argc 恒 0 / strc 恒 1，全文 12,027 条 | docs/OPCODE_101.md | 已验证 |

**★已实测纠正（三处，均与本项目早先的实现或文档冲突）**

1. **Sys2 = 410，不是 412**（曾经写反过，已推翻）。字节码是 `121 [9000002, 1600089, 10, 12544]`（12544 = 0x3100，
   复合 +`=`、二元 0x3000 = **整除**），即 `Sys2 += rateY / 10` = 408 + 2 = 410。
   早先的记录把 0x3000 当成取余（得 4）而写成 412，并一度以此否定 docs/OPCODE_101.md的 410。
   反向证据见 §3.8：CE48 pc30 的 `1024 / 32 = 32` 只能是除法。
2. **消息窗皮肤是灰色 SystemFile 版，不是蓝色 SystemGraphic 版**。两张都叫 `WindowBase.png`、都是 18x18，
   但像素完全不同：`SystemGraphic/WindowBase.png` 是深蓝渐变（零售**选择窗**用它），
   `SystemFile/WindowBase.png` 是灰皮（fill 0x1f2425 @ alpha 0xdf）。两者的边框都是 **3 px**，
   所以 `window.rs CORNER = 3`，不是文档里写的 6。
3. **面板必须走锚点 + 缩放**。早期把 `picture.x/y` 直接当左上角，960 宽的中心锚点面板被画到 x = 512..1472
   （整块出屏）；现在统一用 `Picture::destination(w,h)`，与精灵通道、消息排版共用同一条几何法则。

### 2.3 剧情态（C12，第三批新增）

```
pictures  : 69 live -> 12 textured quad(s), 33 text picture(s)
choice    : none open
sys8 font : 20 px (Sys8, used by every text path)
atlas     : 12 image(s), 30.3% of 2048x2048
rendered  : 1024x576, 546640/589824 non-black pixel(s) = 92.7%
stats     : 51297 instruction(s), 1881 picture op(s), 8 sound(s), 436 db access(es) (38 write, 31 label), 39 message(s), 5 choice(s), 0 warning(s)
```

| 量 | 值 | 说明 |
|---|---|---|
| 指令 | 51,297 | 同一命令在 `112` 修复前是 163,206,432（死循环），见 §3.7 |
| 消息 / 选择 | 39 / 5 | 开场剧情确实在推进，不是停在同一帧 |
| quad / 文本图片 | 12 / 33 | HUD 与房间由 12 张纹理拼成，面板文字全是文本图片（作者不用 101 画 UI） |
| 非黑 | 546,640（92.7%） | 全屏有内容；比未解析字号时少 1,601 px，因为 24 px 的时钟与金额不再按 20/16 px 画 |

**已验证**（转批修复）：HUD 第一版里"顶栏三段文字压叠 / 状态面板数字漏到右沿 / 标签与数字挤在一起"三处缺陷，
根因是文本图片的 `\f[n]`（字号）与 anchor 位被忽略，全部按 Sys8 = 20 px、左上角出图。
现在 `Game::text_quads()` 按片段切字号 / 颜色 / 描边走 textpic（§3.9），顶栏 `1天 18:30 周一哥哥`、`行动力[进度条] 0/60`、
`性欲 20％`、`精力 3/3`、`所持金 2409 G` 已与零售 `Save/save1.jpg` 的同一栏对齐（同为 1024x576 实机 HUD）。

---

### 2.4 存档载入态（C15，本次新增）

`wolf-cli capture --load-save <SaveDataNN.sav> --frames 1` 直接按零售存档的
SavePart5「当前画面图片表」重建屏幕，不跑任何脚本：

```
loaded    : 66 picture(s), 800 normal variable(s), playtime 16642 frame(s), choice anchor (512, 240)
pictures  : 66 live -> 17 textured quad(s), 34 text picture(s)
rendered  : 1024x576, 548416/589824 non-black pixel(s) = 93.0%
```

与零售 `Save/save1.jpg` 逐像素比较：mean abs 5.226 / <=2 83.20% / 其余区域 0.58..1.45，
差值集中在顶栏文字的抗锯齿与 JPEG 压缩（顶行 18.56），结构、数值、字号、锚点全部对齐：
`90日 21:40 周六 温柔的尖叫的哥哥` / `行动力 75/120` / `性欲 56％` / `精力 4/6` / `持有金钱 7586 G`。
立绘 picture 90 `Picture/stand_imouto_paja2.png` 按 6x2 精灵表取第 2 格，画在 `[600,80,400,650]`。

## 3. `150` 图片指令的四个语义修正（M2 的核心发现）

### 3.1 擦除是**区间**，不是单槽（★已实测纠正）

`150` 的 head 字：bits 0..3 = 动作（0 画 / 1 移动 / 2 擦除 / 3 隐藏），bits 4..6 = 类型
（0 文件 / 1 文件来自字符串变量 / 2 文本 / 3 窗口文件 / 4 窗口字符串），bits 12..14 = 锚点。

全作 550 个公共事件里 16,716 条 `150`（C6），动作分布 `{画 10,272 / 移动 3,365 / 擦除 2,835 / 隐藏 244}`；
类型分布 `{文件 14,106 / 文件来自变量 82 / 文本 2,179 / 窗口文件 258 / 窗口字符串 91}`。
2,835 条擦除按参数个数（含 head 字）：

| 形态 | 条数 | 参数形状 |
|---|---|---|
| 短形（单槽） | 1,798 | `[head, number, fade]` |
| 短尾（多余槽位，仍按单槽处理） | 85 | `[head, number, ?, ?]` |
| 长形（区间） | 952 | `[head, number, fade, fade, 0, other]` |

> 口径警告：本项目早先笔记里的 `1,115 / 751 / 329 / 35` 是**更窄的扫描**（只统计"两端之间确实夹着某张被画过的图"的擦除），
> 不是全部擦除指令；上表才是 550 个公共事件的完整口径，可复现（C6）。

**判据**：长形的第 6 个参数是**第二个图片号**，两个端点构成**闭区间**，且**与书写顺序无关**（取 min/max）。
- SampleMapA 事件 9 pc1461 `150 […, 400, 0, 0, 0, 409]` 紧接 pc1462 `150 […, 410, 0, 0, 0, 509]` —— 两段相邻区间划分 UI 组。
- SampleMapA 事件 3 pc6/7/8 擦 `1200..1220`、`7000..8100`、`8101..8200`。
- 只有 4 处 A > B（Map003 事件 1 pc24 的 `(-1,-2)`，SampleMapA 事件 15 pc102/388/656 的 `(20007,20003)`）。
- **决定性证据**：ce221 pc36 `150 [16777218, 50000, 5, 5, 0, 50003]` 一条就清掉加载画面的
  `50000`（黑底）、`50001`（eyecatch）、`50002`（`\ENOW LOADING...`）、`50003`（`<SQUARE>` 进度条）。
  早期"50001/50002 永远没人擦"的谜题至此关闭：**不需要引擎在脚本结束时自动清图**。

### 3.2 动作 3 是"隐藏"，不是"重画"

ev63 pc101 `150 [3843, 19500]` 的注释写着 `フェードが1以外ならウィンドウを非表示`（淡入不足 1 就隐藏窗口），
head 低位 = 3。语义是**保留槽位/文件/矩形/位置，只是不再绘制**。早期把它当"重新创建"，于是每次隐藏都把
皮肤名、窗口尺寸、坐标清成默认值——这正是 M2 开头"面板有槽位却 0 个 quad"的直接原因。

### 3.3 移动保留锚点

`Move` 里锚点为 0 表示**不改**，不是"改回左上角"。ev63 先把窗口画在中心锚点 (512, 672)（故意停在屏幕下方 96 px，
等于"出场前藏起来"），再移到 (512, 480)。把锚点 0 当真值会把面板挪到 x = 512..1472。真正想改锚点的 56 条移动
都显式带 0x1000。

### 3.4 类型 1/4 的文件名来自**字符串变量**

`150` 的类型 1（文件来自变量）与类型 4（窗口文件来自变量）在这作里**全部 0 个字符串参数**（类型 1 共 82 条、
类型 4 共 91 条）；文件名走**最后一个整数槽**（`PICTURE_STRING_SLOT = 11`），例如 `1600009` = 事件 self 字符串 9。
`Magic::index()` 已经是扁平库下标（1,600,009 → 609），**不能再加 `program.self_base`**——那个偏移只属于
`\cself[n]` 转义。加错一次就会静默读到空串（本项目踩过）。

### 3.5 类型 3 的"系统图形"不是文件

`<SQUARE>`（252 处）、`<SQUARE><FRAME>`（16）、`<SQUARE>FRAME`（2，作者的拼写笔误，按同一形状处理）、
`<GRADX-x-y>` / `<GRADY-x-y>`（38）、`<LINE>`（7）、`<CIRCLE>`（3）、`<TRI-LEFT|RIGHT|DOWN>`（各 1）
由引擎**程序化绘制**（`crates/wolf-game/src/sysgraphic.rs`，642 行，15 个单元测试）：
- `<SQUARE>` = 不透明白色填充，颜色由图片 tone（intArgs[15..17] → `merge_args`）染；
- `<SQUARE>FRAME` = 1 px 边框 + 内部透明；
- `<GRADX-a-b>` / `<GRADY-a-b>` 的数字是**千分比 alpha 端点**，不是宽高
  （`<GRADX-999-666>` 出现在 470x8 的条上；`<GRADX-444-000>` 与 `<GRADX-000-444>` 成对出现当淡出/淡入）。
- **已验证**：全部 375 条系统图形都是类型 3（窗口类）。若不先判定 `sysgraphic::parse`，它们会被当九宫格面板
  用 `SystemGraphic/WindowBase.png` 画出来，作者画的形状永远不出现。

### 3.6 移动的尾部帧数是**插值时长**（★已实测）

`150` 的移动形态可以在尾部多带一个 `[head | 0x1000000, …, 帧数, …]`：帧数 1..=600 表示**线性插值**
（x/y/scale/opacity 四个量一起从原位滑到新位），0、缺省或无法求值的 self 变量表示**瞬移**。
全作 3,365 条移动的分布（已验证，探针 work/_probe_move_*.py）：

| 形态 | 条数 | 处理 |
|---|---|---|
| 无尾部（瞬移） | 2,817 | 直接改位 |
| 尾部 0 | 168 | 直接改位 |
| 尾部是无法求值的 self 变量 | 52 | 直接改位 |
| 尾部 1..=600 | 328（实测时长 2..16 帧） | 插值 |

**决定性成对样本**：ev63 pc93 无尾部地把消息窗停在 (512, 672)（屏幕下方 96 px，等于"出场前藏起来"），
pc94 用**同样的参数**加 `16777216, 3` 把它滑到 (512, 480)，滑完 3 帧才该出现在零售画面里。
实现：`picture::Pose` / `Transition` / `Screen::advance()`，由 `Game::frame` 每帧推进一步；
`Screen::apply` 在合并参数**之前**记下起点，合并后立刻把 x/y/scale/opacity 回滚到起点，
于是 `destination()` / `alpha()` / `quads()` 的读法完全不变（`MAX_TRANSITION_FRAMES = 600`）。

### 3.7 `112` 必须和 `111` 共用块栈（★已实测纠正，M2 空转的根因）

`112`（文字列条件分岐）在预编译阶段**必须自己开一个 `BlockKind::Condition` 块**：
它的 `401/420/499` 骨架与 `111` 一模一样，不开块就会把 `401` 挂到外层分支、把 `499` 提前关掉外层分支。
详细字节码与证据见 **docs/OPCODE_112.md**，这里只记验收用的两个事实：

1. **计数器与 opcode 普查逐一对上**（C14）：`conditions` = `111`(10,743) + `112`(123) = **10,866**；
   `cases` 从 18,460 涨到 **18,555**，正好等于 `401` 的全库条数；`else_cases` 3,279 = `420`、
   `cancel_cases` 153 = `421`；`orphan_cases` / `orphan_ends` / `unclosed_blocks` 全部归零。
2. **开场剧情因此才动起来**（C12）：修复前 900 帧烧 163,206,432 条指令、全黑；
   修复后 51,297 条指令、39 条消息、5 次选择、92.9% 非黑（§2.3）。

块表可以直接读出来对照（C13）：

```
block 21 Choices head 475 (102, [25]) cases [(2, 477), (3, 501), (4, 525), (5, 550), (6, 575), (7, 599), (8, 623), (9, 632), (10, 635)] else None end Some(649)
```

修复前同一块的 `cases` 混进了 `112` 的 `401`（480/483/486/489/494），`end` 落在 496；
答完选项就落进 case 体中间，绕回 `213 重置` 形成死循环。

---

### 3.8 `121` 的运算符表曾经整体错位（★已实测纠正）

`121` 的运算符字 = 高半字节二元运算 + `0x0F00` 掩码的复合运算。长期被引用的 WolfTL 表把 **`0x3000` 当取余、`0x4000` 当按位与**，本作字节码给出的却是 **`0x3000` 整除、`0x4000` 取余**：

- CE48 pc30 `121 [cs25, Sys116, 32, 0x3000]`：Sys116 = 1024（屏幕宽），1024 op 32 必须 = 32 = rateX；取余会得 0。
- CE63 pc46 `121 [Sys2, cs89, 10, 0x3100]`：Sys2 = 408 再加 24 op 10；除法得 410，与 docs/OPCODE_101.md 的独立推导一致（取余得 412）。
- CE74 pc148 `121 [cs43, cs44, 100, 0x3302]`（复合 `*=`）是百分比缩放，必须先乘后除。
- 反向证据：CE48 pc172 `self20 = self20 % 10000`、CE86 pc44/48/52 `self15 = self27 % 10000/20000/30000`。

**为什么这条排在 M2 里**：消息窗原点 Sys2 正是 `17*rateY + rateY/10`。旧表把 `/10` 算成 `%10`，于是 message.rs 一度「实测」出 412 并把这条当成对 OPCODE_101 的反驳；改正运算符后同一命令给出 **410**，与文档一致。教训：用一个未验证的运算符去「实测」另一个量，会把两个错误互相印证成实测值。

实现见 crates/wolf-vm/src/exec.rs `fn binary_op`（除零/模零保留左值），文档见 docs/OPCODE_SPEC.md §3.2。

### 3.9 文本图片的排版：`\f[n]` 是字号，锚点不是左上角（★已实现）

全库 16,716 条 `150` 里有 2,179 条文本图（type 2 = 1,147、type 4 = 59 带 `\f`），而 HUD 顶栏的所有文字都是它们。
把每条文本图按 Sys8 单一字号、左上角画出来会产生三类可见缺陷（M2 剧情态第一版就是这样）：

- `pic 6200 \f[24]18:30` 与 `pic 6210 \f[18]周一` 压叠——前者 24 px 却被当成 20 px 左对齐。
- `pic 6251 \f[24] 2409 \f[14]G` 越过 1024 右沿——它其实是 anchor 3（右上），右缘钉在 x = 1015。
- 状态面板的数字与标签挤在一起——每个 `\f` 都要换字号。

**已验证的排版律**（实现见 crates/wolf-game/src/textpic.rs，988 行 / 25 个单测）：

| 律 | 内容 |
|---|---|
| 字号 | `\f[n]` 改当前片段字号；`\f[\cself[19]]` 这类嵌套可求值 |
| 锚点 | `150` head 的 bits 12..14：0 左上 / 1 居中 / 2 左下 / 3 右上 / 4 右下；3/4 把块的右缘放在 x，1 居中，2/4 把底缘放在 y |
| 块尺寸 | 宽 = 最宽行，高 = 行数 × 字号 × 1.3（`DEFAULT_LINE_PITCH_RATIO`） |
| 右对齐 | `<R>` 把当前行贴到块右缘（`\udb[28:1:0]\n<R>\f[18]\udb[28:1:1]` 这类两行标签） |
| 颜色 | `\c[n]` 查 SysDatabase 类型 12 的 30 行调色板（`Databases::colour`） |
| 描边 | `\E` 走 renderer 的 `Outline`（四向偏移 + 本体，宽度 2 px） |

**渲染接线**：`Game::text_quads()` 把每张文本图按片段切成 `atlas::TextQuad`（新增 `font_px` / `colour` / `outline` 三个字段，0.0 表示用调用方默认 Sys8），
CLI 按 quad 的字号建 `TextItem`。离屏的文本片段同样走 `picture::on_screen` 裁掉（气泡库停在 `(8256,1008)` 的 11 个控件）。

**零售对照**：`Save/save1.jpg` 是 1024x576 的实机 HUD 截图。顶栏 `pic 6200`（anchor 3、x = 140）的墨迹落在 85..138 —— 与 Lolita.ttf 的真实 advance 56.31 px 吻合；
半角按 0.5 em 估算会给 60 px，因此 anchor 1/3/4 有最多 6% 的系统偏差，`measure_with` / `layout_with` 留了真实 advance 回调的口子（未接 cosmic-text）。

**已验证**：修复后剧情态 12 quad + 33 文本图、92.7% 非黑、0 警告；顶栏 `1天 18:30 周一哥哥`、`行动力[进度条] 0/60`、`性欲 20％`、`精力 3/3`、`所持金 2409 G` 的位置与零售 save1 同栏对齐。

---
### 3.10 普通文件图可以是**精灵表**：`[2]/[3]` 是分割列/行，pattern 是 1 基单元号（★已实测）

`150` 和存档记录对普通文件图复用同一组参数槽 `[2]/[3]/[4]`：

| 槽 | 普通文件图 | 窗口 kind |
|---|---|---|
| `[2]` / `[3]` | 分割列数 / 行数（1x1 = 整图） | 窗口宽 / 高 |
| `[4]` | 图案编号，**1 基** | 无意义 |

决定性证据：`Picture/stand_imouto_paja2.png` 是 2400x1300 的 6x2 立绘表（每格 400x650），
零售 SaveData01 的 picture 90 记录 `vars7_18 = [600, 80, 6, 2, 2, 255, 0, -1, 1, 100, 100, 0]`；
picture 92 `stand_eye_a.png` 是 7x1 条带且记录 pattern 7——只有 1 基才不越界。

实现：`Picture.split: (u32,u32)` + `Picture.pattern_index`，`picture_frame()` 负责切格；
`ImageData::crop([x,y,w,h])` 与 `Atlas::add_asset_region(name#rect)` 只把**那一格**打包进图集，
因为 2400x1300 超过 2048 的图集页。此前剧情态没有妹妹立绘，就是因为整张 2400x1300 被 atlas 丢弃。

## 4. 数据库层与引擎协程（消息窗口的前提）

### 4.1 为什么 M2 必须先做数据库

ev63 的几何**全部来自数据库**：`250 [18, 97, 0, 69632, self88]`（座標倍率 X）、`[18, 98, 0, …]`（Y）、
`[18, 0, 24, 332288, self9]`（窗口皮肤名）。在 `Host::database` 只会返回 `None` 时，rateX/rateY 为 0，
于是 Sys1 = Sys2 = 0、折行宽度塌成 22 px（一行一个字），实测得到 68 行文本——这就是"排版全错"的根因。

现在 `crates/wolf-game/src/db.rs` 同时加载 `DataBase.dat`（40 类型）、`CDataBase.dat`（30 类型）、
`SysDatabase.dat`（28 类型），并按 `opcode 250` 的 (type, row, column) 应答：
- **已验证**：座標倍率来自 `CDataBase` 类型 18 的第 97/98 行，其列 1 里作者自己写着
  `ここの値は「自動システム初期化」処理でセットされます`（由 common event 48 = 自动系统初始化写入）。
- 引擎层同时也兜底写入 `(18,97)=(32)`、`(18,98)=(24)`、`(18,0,24)="SystemFile/WindowBase.png"`，
  即"脚本还没跑到 ev48 也能排版正确"，且刻度由**屏幕尺寸**推出：`rateX = 1024/32`、`rateY = 576/24`
  （早期误用 tile_size 16，得到 32/278 的原点）。
- 仍未应答的读取：`(0,-1,33)`、`(20,69,0)`、`(18,0,22)`（后者来自 ev48 pc158 等 8 处，不是 ev63）。

### 4.2 引擎协程

**已验证**：common event 63（`X[共]メッセージウィンドウ`）**没有任何静态调用点**，是引擎在
`101` 出现时以"常时作动"协程启动的：它写入 Sys1..Sys6、用 `150` 画出消息窗、`Sys12 = 1` 表示"消息显示中"。

现在启动的是**三组共 12 个事件**（crates/wolf-game/src/game.rs 的三个常量，验收命令 C10/C11）：

| 常量 | 事件 | 依据 |
|---|---|---|
| `ENGINE_ALWAYS` | 48, 261, 269, 270, 271, 224, 225, 213, 214, 41 | header = `parallel_process_always(3)`；顺序有意义（48 先填系统变量，261 先于 270，269 先于 270，225 发布 Sys71/72） |
| `ENGINE_AUTO_START` | 241 | `auto_start(1)`：启动一次就自己清条件变量并结束 |
| `ENGINE_IMPLICIT` | 63 | 引擎特性启动（`101` 一出现就调用），不是脚本调用 |

**已验证（A/B 实测）**：标题帧 716 上，`--engine-events ''`（一个都不启）与逐个单独启动 48/261/269/270/224/225/213/214/41 的
渲染结果 `imgdiff` 全部 `identical: true`；九个全启后与零售截图仍是 mean abs 0.012 / psnr 67.20 dB。
`271` 曾被排除过（它启动第三方气泡库 event 98，19 个控件落到 (0,0) 变成可见垃圾），现在这些控件落到库自己的停车场
`(-2784, 2880)` 并被 `picture::on_screen` 裁掉，单独启动 271 也 `identical: true`，所以恢复启用
（docs/ENGINE_EVENTS.md §10.5）。

**未验证**：343（44 个 by-id 调用点，但它读的条件变量全作没有写者）与 43（零调用点）仍未启动。

### 4.3 `210/300` 的返回值

**已验证**：19,636 个调用点里 377 个在参数状态字里置了 bit24（`0x1000000`）表示"要返回值"；
目标槽位是**被调 550 事件自己声明的** `return_value_id`（74 个事件声明了，70 个真的写），
地址是 `1,600,000 + return_value_id`（callee self group 6）。返回值目标操作数位于参数表**末尾**
（`a[2+num+str]`），不是 `2+num`——旧公式在 6 处退化成字面量 0（值被静默丢弃）、在 16 处写到无关槽位。

### 4.4 写路径（250 的 direction / op）

**已验证**（docs/OPCODE_250.md）：`flags & 0x1000` 表示**方向**，置位才是读，清零是**写**；
`container = (flags >> 8) & 3`（0 CDataBase / 1 SysDatabase / 2 DataBase）；`op = (flags >> 4) & 7`。
写路径本项目实现为**读-改-写**：操作数来自 int 实参 4（argc >= 5）或字符串实参（argc == 4），
按 op 与当前值合成后落盘，**整个写指令不回写源变量**（早期把写当读，会把脚本自己的值覆盖成 0）：

| op | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| 语义 | assign | add | sub | mul | div | rem | max | min |

除零/取余零时**保留原值**（与原作 `1 加 → 6 夹上限 → 7 夹下限` 的用法一致）。
`The Interpreter` 侧：`DbOp { container, direction, op, type_label, field_label, source, destination }`，
其中 `destination` 只在读且 argc >= 5 时存在；游戏侧 `combine_cell(op, current, operand)` 是唯一的合成点
（`a_database_write_operator_is_the_flags_nibble` / `a_script_write_lands_in_the_cell_the_next_read_sees` 两个测试）。

**已验证**：`250` 的标签落库。`.project` 名字在 wolf-format 里（`Database::type_name/field_name/row_name`，
三个文件都精确走到 EOF），`Databases::load` 现在顺带读 `DataBase.project` / `CDataBase.project` /
`SysDatabase.project`；`resolve_labels(container, db_type, column, type_label, field_label)` 先按类型名定位表、
再在表内按列名定位列，解析不出来就退回数值对，`Sys` 容器不参与（它没有 `Database` 表）。
起因是 8 个站点（ev48 pc158、ev85 pc29、ev93 pc38、ev102 pc38、ev109 pc14、ev123 pc15、ev147 pc21、ev185 pc49）
发 `250 [18, 0, 22, ...]`、标签写 `システム設定 / 装備可能欄数`，而它们真正要的是 `DataBase` 的
**type 17, col 22 = 5**——编辑器编号把空分隔 type 14 算进去、文件不算，所以数值 id 漂了。
实测（开口 capture）：`128 db access(es) (15 write, 10 label)`，`(18, 0, 22) ... unanswered` 消失，
标题帧与零售仍一致（mean abs 0.012 / psnr 67.20 dB）。读路径先试数值对再试标签（99% 的站点数值对就是对的），
写路径直接用标签，免得在错的 type 里落下一个野格子。

**仍未回答（不是标签问题，是行不存在）**：`CData (0, 100, 0/1/4/21/48/49)` 与 `(0, -1, 33)`。
`CDataBase` type 0 是 51 列 / 17 行 / `data_id_method 1`（行 id 由运行时自动分配），行 100 是存档里运行期
建出来的角色行，新开局没有它；行不存在的读该回 0 还是不动目标变量，**原作未验证**，所以这里是留空并计数，
不做猜测。诊断行现在会把标签一起打出来（`labels "..."/"..."`），下一轮定位不必再翻字节码。

### 4.5 存档操作码：220 / 221 / 222（★已实测）

`docs/OPCODE_220_SAVE.md` 的实际参数：220 是 `argc 2 = [mode, file]`（mode 0 保存 / 1 读取）；
221 是 `argc 4 = [destination, file, address, flag]`；222 是 `argc 4 = [value, file, address, flag]`。
`file` 可以是字符串变量（路径）或数值（槽号）；`Slot(n)` 映射到 `root/Save/SaveData{n:02}.sav`。

解释器侧只有一个钩子：`Host::save_load(&SaveOp) -> Option<i32>`，`SaveOp` 带
`opcode / mode / file / destination / address / value / flag / owner`。221 的返回值由解释器写回
`destination`，220/222 返回 `None`。地址映射（本轮实测）：`2_000_000+n` 是通常変数 `n`，
`1_600_000+n` 是 owner 事件的セルフ変数 `n`，`9_000_000+29` 是游戏时间的秒数，
`9_000_000+0..12` 是 `part2.vars3_21[n+6]`。

引擎侧 `GameHost::save_load`：220 mode 1 排队 `SaveRequest::Load(path)`，`Game::frame` 在
`vm.step_frame` 返回后调用 `savegame::load_into`（清屏、按 Part5 重建图片、恢复 800 个通常变量与
`playtime`、Sys3/Sys4）；221 读文件并返回 `save_address`（缺档回 0）；222 读→`write_save_address`→
`write_save` 写回。测试：`wolf-vm save_ops_decode_slot_paths_and_store_the_221_answer`、
`wolf-game a_save_address_finds_normal_self_and_system_variables` / `a_save_write_reaches_the_next_read` /
`a_save_file_round_trips_through_the_writer`。

**仍未实现**：220 mode 0（从 Game 状态构造完整 `SaveFile` 的 part1/part2/part4/part5）只有一条诊断记录；
221 的 `flag=1` 三个站点（ev94 pc335/345/348，地址 `1600062`/`1600064`）语义未验证。

---

## 5. 与 PLAN.md 的偏差（M2 项的作废与替换）

### 5.1 作废项

| PLAN 中的 M2 项 | 现状 |
|---|---|
| 角色 chip（8 向 × 3 帧）+ 主角行走 + 碰撞 | **作废**：本作 6 张注册地图零页面图形，没有可行走地图（§1.1） |
| "SampleMapA 事件 1 → 103 个事件 / 45,364 条指令" | **作废**：事件 1 实际 113 槽 / 27 条指令（标题也写错了） |

### 5.2 实际交付并纳入验收的 M2 项

1. **地图/页面运行时**：按页面条件选页（不是走入触发），trigger 0/1/2/3，parallel 页是"结束即重启"的常驻协程。
2. **图片层**：区间擦除、隐藏、移动保留锚点、类型 1/4 的字符串变量取名、系统图形程序化绘制。
3. **选择窗口（102）**：锚点 = Sys3/Sys4 左上角，离屏则不画；取消与否由 head bit4 决定；
   面板宽度与字形共用同一字号常量。
4. **消息窗口（101）**：ev63 画框、引擎排版（原点/字号/行距/折行/打字机/▼ 光标）。
5. **数据库层**：DataBase / CDataBase / SysDatabase + opcode 250 应答 + 引擎兜底单元格。
6. **调用返回值**：377 个 bit24 调用点的目标槽位解析与回写。
7. **数据库写路径**：250 的方向/op 解码与读-改-写（8 个合成运算），写指令不回写源变量。
8. **移动插值**：328 条带时长的 `150` 移动按帧线性插值（其余 3,037 条瞬移）。
9. **常时协程**：三组共 12 个事件由引擎启动（§4.2），并用 A/B 证明对标题帧像素零影响。
10. **离屏裁剪**：碰不到屏幕的图片不再产生 quad / 皮肤包（第三方气泡库的 19 个控件）。
11. **`112` 块栈**：字符串条件分岐与 `111` 共用同一套块栈，全库孤儿计数归零（§3.7）。
12. **`290` 十一种效果**：type 0/1/2/3/4/5/7 已实现、6 全库 0 条、8/9/10 状态机就位；图片号按**闭区间** [lo, hi] 处理。
13. **诊断命令**：`wolf-cli blocks`（块表）与 `--answer-delay`（按帧作答，避免把菜单循环烧进一帧）。
14. **存档载入**：`--load-save` 按 SavePart5 重建当前画面（66 图 / 800 个通常变量 / 17 quad / 34 文本图），HUD 与零售 save1.jpg 对齐；普通文件图按 6x2 精灵表取格。
15. **存档操作码**：220 读档排队、221 读档内变量、222 写档内变量并落盘；地址映射与存档写回测试见 §4.5。

### 5.3 地图渲染不再是 M2 验收项

`wolf-cli capture --map SampleMapA` 仍能渲染 27,648 个 quad（64x36 格 × 3 层，40 事件），
但因为本作没有可行走地图，它只是**引擎能力演示**，不作为 M2 通过条件。

---

## 6. 仍未验证项（写在这里，避免下次被当成已验证）

### 6.1 没有零售截图可对照的两处

1. **消息窗口（101）的真实外观**。现有 26 张真实截图全是标题 / 开场 / 存档界面，
   没有任何一张显示 101 消息窗。因此 §2.2 的"灰皮 + 底部 960x168"是**由字节码与像素双重推导**的结果，
   不是与零售像素比对的结果。要闭掉它需要人肉在游戏里截一张带消息窗的 PrintScreen。
   **同样缺参照的一个量**：7 行 x 26 px = 182 px 高于面板 168 px，本项的第七行与 ▼ 压在底边框上。零售是分页、缩字号还是更宽的折行，尚未定量。
   （**注意**：自动化前台窗口 + 模拟按键已经被明令禁止——它会抢走用户的操作权；只能用游戏内
   PrintScreen 键，由人驱动。）
2. **Sys3/Sys4 是否真的是选择窗左上角**。docs/OPCODE_102.md §4.2 标【已验证】、字节码与标题事件
   （把 Sys4 写到 1000 让窗口出屏）都支持这个读法，但同样缺一张零售开场选择窗截图。

### 6.2 已知未实现 / 实现不完整

| 项 | 现状 |
|---|---|
| 移动插值的**缓动** | 现在统一线性 `age/duration`；原作是否有缓动曲线未验证（ev63 的 3 帧太短，看不出） |
| `TextItem::with_alpha` | **已修**：`Attrs` 改用 `Color::rgba`，文字与描边一同淡出（3 个测试，标题帧仍逐字节相同） |
| `125 AutoInput` | 只保存掩码，不真的注入按键（需要宿主） |
| `126 BanInput` | 已在 `123` 的掩码里过滤（本作 1,115 处 123 指令按位生效） |
| 常时协程 | 已启动 12 个（§4.2）；343 / 43 仍未启动 |
| `250` 的标签解析 | **已接上**（`resolve_labels`，开口实测 10 次命中，§4.4）；剩的是**行不存在**的读（CData 行 100 / 行 -1），原作语义未验证 |
| `290` 效果的绘制接线 | **已实现**：`picture.rs` 的 `Zoom/Shake/Breath/PatternFx/color_corr/shift` 全部接到 `destination()` / `tone_byte()`；`8/9/10` パターン切替已按 `pattern_now()` 选精灵表格子（159 个 wolf-game 测试）。`0` フラッシュ 的数学仍未与零售逐像素对照 |
| `290` 的实参求值 | **已修**：`exec.rs` 的 290 分支对每个 int 实参调用 `eval_num`，self 变量 magic（`1600021` 等）现在能取到图号（测试 `effect_operands_are_evaluated_before_the_host_sees_them`） |
| HUD 文本排版 | **已修**：`Game::text_quads()` 按 `\f[n]` / anchor / `<R>` / `\c[n]` / `\E` 切片出图（crates/wolf-game/src/textpic.rs，25 个单测），见 §3.9；半角 advance 仍按 0.5 em 估算，anchor 1/3/4 有 ≤6% 偏差（`measure_with` 未接 cosmic-text） |
| `290` 的图片号是**区间** | `args[2..3]` 是闭区间 [lo, hi]（官方「□～□ の欄に…」）；若按单张实现，type 4 的 7 对按钮、type 5 的整幕压暗、CE85 的「全カーソル」都会只作用一半（docs/OPCODE_290.md §0.4） |
| 字符串实参 | **已实现**：8,260 个带字符串实参的 210/300 调用点写入 CSelf5..8（docs/OPCODE_210_RETURN.md §8），零反例 |
| 存档/读档（220/221/222） | **读侧与 221/222 已落地**：`--load-save` 重建画面（Part3 通常变量 / セルフ変数 / 指文字列、Part2 时间与 Sys3/4、Part5 图片表）；221/222 经 `SaveOp` 读写并落盘，9 档 9/9 逐字节可逆。**220 mode 0 的整档保存仍未实现**（见 §4.5） |
| `270 Party`、`160/162 Transition` | 仍是记录型 no-op |

---

## 7. 一句话结论

M2 纵切**已经跑通到游戏主界面**：从 `SampleMapA` 事件 0 的第 0 页出发，经过加载画面、叙述文本、四选项选择，
答完之后脚本继续推进到床边场景（HUD + 房间 + 立绘，92.7% 非黑）。消息窗口与选择窗口都由**引擎自己的规则**
（ev63 协程 + 数据库刻度）排版，实测 0 警告；三个稳定态（消息 / 选择 / 剧情）都可复现。
促成这一切的关键修正是 `112` 必须与 `111` 共用块栈（§3.7）：它把 163,206,432 条指令的死循环变成 51,297 条。
剩下的是"真实外观校准"（缺零售截图）与"已知未实现清单"（§6.2），不影响本纵切的结构正确性。
