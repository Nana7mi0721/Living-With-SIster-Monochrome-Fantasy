# UI 布局逆向：从脚本数据拿到每个画面的排布逻辑（不靠肉眼 / 不必 Ghidra 逆引擎）

> 状态：**已验证（HEAD 工作树）**　数据根：`work/all`　零售对照：`work/retail_backup/Save/save1..8.jpg`
> 工具：`wolf-cli` 新子命令 **`wolf-layout`**（`crates/wolf-cli/src/bin/wolf-layout.rs`）
> 关联修复：行动力条纵向铺满（`crates/wolf-game/src/picture.rs` `Picture::destination`）

## 1. 核心结论：本作的 UI 是**数据**，逆向它不需要 Ghidra

WOLF RPG Editor 的整个 UI（状态栏、时钟、消息框、选择窗）不是引擎里画死的，而是由
**公共事件脚本**用以下三条指令逐帧画出来的，参数里已经写着精确的排布：

| opcode | 指令 | 排布参数 |
| --- | --- | --- |
| `150` | Picture | `kind(type)` / 图片号 / 窗口宽高 / pattern / opacity / **x** / **y** / **scale(横)** / blend / anchor |
| `101` | Message | 文本（与位置来自所在 picture 的 150 文本层） |
| `102` | Choices | 选项文本 |

所以「零售 UI 在某个画面/事件里的详细排布逻辑」= 该事件的 `150/101/102` 指令序列。
我们**已经能静态解析** `work/all` 的 `BasicData/CommonEvent.dat` 与 `MapData/*.mps`，
把它导成表即可，无需对引擎二进制做逐像素肉眼对照。

> 姐妹项目 imouto life 对 `game.exe` 用 JDK+Ghidra，是因为它要逆的是**通用引擎**。
> 在本作里引擎是同一个 WOLF 运行时；我们只在**opcode 语义**（例如 `150` 的缩放到底是
> 单值还是分轴）上才需要"逆"，而这一层已经用「真实指令流 + 8 个零售存档 part5 + 像素 diff」
> 反推清楚（见 §5）。Ghidra 仅在需要复核某条 opcode 的引擎实现时才有价值，不是布局来源。

## 2. 工具 `wolf-layout`

```bash
source tools/env.sh >/dev/null 2>&1
cargo build -q -p wolf-cli --bin wolf-layout

# 公共事件（HUD 状态栏 = 216，时钟 = 217，行动力变动 = 226）
./target/debug/wolf-layout.exe --root work/all --event 216

# 地图事件（可指定 page）
./target/debug/wolf-layout.exe --root work/all --map Map003 --event 1 --page 0
```

输出为逐条 `150` 表：`pc / kind / cmd / #pic / boxW / boxH / opacity / x / y / scale / anch / blend / text`，
`101` 文本另起 `pc… message "…"` 行。scale 若是 magic（如 `1600012`）会原样打印，表示由运行时变量给出。

## 3. 状态栏 `CE216 ステータスバー表示` 完整排布

`wolf-layout --event 216` 实测（1024×576，**锚点 0 = 左上，3 = 右上**）：

| pc | kind | #pic | box | op | x | y | scale | anch | 内容/说明 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | file | 6000 | 1×1 | 255 | 0 | 4 | 100 | 0 | `hscene/header_bar.jpg` 顶栏底图 |
| 1 | text | 6220 | 1×1 | 255 | 190 | 12 | 100 | 0 | 哥哥（名字，`\cdb[0:12:*]`） |
| 26 | text | 6230 | 1×1 | 255 | 430 | 14 | 100 | 0 | 负伤中（受伤时切换） |
| 29 | text | 6230 | 1×1 | 255 | 430 | 14 | 100 | 0 | 行动力（正常） |
| 32 | winFile | 6231 | 150×15 | 255 | 480 | 14 | 100 | 0 | `<SQUARE><FRAME>` **行动条外框** |
| 33 | winFile | 6232 | 150×14 | 100 | 480 | 15 | 100 | 0 | `<SQUARE>` **行动条暗底**（opacity=100） |
| 34 | winFile | 6233 | 150×14 | 255 | 480 | 15 | **1600012** | 0 | `<SQUARE>` **白色填充**，scale=当前AP/上限 |
| 35 | text | 6235 | 1×1 | 255 | 630 | 16 | 100 | 0 | `\v[14]/\v[13]` → `75/120` |
| 37 | text | 6236 | 1×1 | 255 | 780 | 14 | 100 | 0 | `精力 \v[16]/\v[15]` |
| 39 | text | 6245 | 1×1 | 255 | 690 | 14 | 100 | 0 | `性欲` |
| 40 | text | 6246 | 1×1 | 255 | 760 | 14 | 100 | 3 | `\v[18]％`（anch 3 右上对齐） |
| 189 | text | 6250 | 1×1 | 255 | 860 | 14 | 100 | 0 | `所持金` |
| 190 | text | 6251 | 1×1 | 255 | 1015 | 7 | 100 | 3 | `\v[7] G`（anch 3 右上对齐） |

**行动条 = 外框(6231) + 暗底(6232, opacity100) + 白色填充(6233)** 三层叠在同一 (480,15)、
尺寸 150×14；填充的 `scale = self12`，由脚本按 `当前AP/上限 × 100` 写入（默认 SaveData01 = 62）。

## 4. 时钟 `CE217 時刻表示` 与行动力变动 `CE226`

`CE217`（anch 3 = 右上）：

| pc | #pic | x | y | anch | 内容 |
| --- | --- | --- | --- | --- | --- |
| 36/39/42 | 6200 | 140 | 7 | 3 | `\v[4]:\v[5]` → 时:分 |
| 3/8/11 | 6211 | 66 | 10 | 3 | `\v[160]` 天 |
| 48–71 | 6210 | 150 | 10 | 0 | 周一…周日（周六/周日换色） |

HUD 换色来自 SysDatabase type12 的调色板：`c[0]` 白 `[255,255,255]`、`c[1]` 蓝 `[140,175,255]`、
`c[2]` 红 `[210,130,130]`（零售 JPEG 有压缩偏移）。存档文本图把颜色号存为 `index + 14`：
`\c[0]`→0x0e、`\c[1]`→0x0f、`\c[2]`→0x10；0x0f **不是** `\c[0]` 的特例。
渲染端目标为 `Rgba8Unorm`，glyphon 必须用 `ColorMode::Web`（默认 Accurate 会把 sRGB 转 linear 而变暗）。

**文字基线**：DxLib 比 cosmic-text 的默认行盒高 2px，因此 `text.rs` 对所有文本统一 `BASELINE_NUDGE_Y=-2`。同一文本图里混排字号时（如 `\f[18]90\f[14]日`、`\f[24] \v[7] \f[14]G`）零售按**同一基线**对齐小字，`textpic.rs::baseline_offset` 让较小 run 下移 `(最大字号-本 run 字号)+1` px；HUD 实测 `日` 与 `G` 的 bbox 与零售逐像素同高（`G` 为 19..30）。

`CE226 ■行動力変動`：文本 6290 在 (590,35) 弹出 `\E+/-\cself[0]` 的 AP 增减提示。

## 5. 从布局反推出的渲染规则（正是行动力条 bug 的根因）

把 `work/retail_backup/Save/SaveData01..08.sav` 的 part5 图片表逐条比对
横向/纵向缩放（`vars7_18[9]` vs `[10]`）：**所有非等比记录都是窗口类图片，且纵向恒为 100**：

| 存档 | 图片 | 类型 | 窗口 | scaleX | scaleY |
| --- | --- | --- | --- | ---: | ---: |
| SaveData01 | 6233 | winFile `<SQUARE>` | 150×14 | 62 | 100 |
| SaveData02 | 6233 | 同上 | 150×14 | 57 | 100 |
| SaveData03/04/05/06/07 | 6233 | 同上 | 150×14 | 21/15/76/18/25 | 100 |
| SaveData08 | 6233 | 同上 | 150×14 | 62 | 100 |
| SaveData08 | 1993/1997 | `<GRADX>` | 215×4 | 60/21 | 100 |
| SaveData08 | 1996/1998 | `<SQUARE>` | 215×6 / 217×6 | −100（镜像） | 100 |

⇒ **规则：窗口类图片（windowFile/windowString，含 `<SQUARE>`/`<GRADX>`/`<CIRCLE>` 等
sysgraphic）的高度取自窗口框，不受 `150` 的百分比缩放影响；那个百分比只横向拉伸。**
文件/文本图片仍是等比缩放（cutin 250%、blackscreen 150% 等照旧两轴放大）。

**这正是用户看到的「白色没铺满条的纵向」**：端口此前把单一 `scale` 同时用于两轴，
于是 6233 被画成 93×**8.7**（150×14 都乘 62%），白色填充比外框矮一截（实测：零售 15 行全白，
修复前只有 11 行白 + 4 行灰）。修复见 `Picture::destination`：窗口类 `height_scale = 1.0`。
修复后逐列实测与零售完全一致（白 15 行、宽度边界 x≈573 处从白转灰），8 个零售存档
`load→save` 仍逐字节一致（`m12_save_full_compare` 2 passed），`Picture` 单测新增
`a_window_gauge_scales_x_only_so_the_fill_spans_the_full_bar_height`。

## 6. 复现与证据

```bash
# 布局
./target/debug/wolf-layout.exe --root work/all --event 216

# 渲染对照：零售存档 → 加载 60 帧 → 截 HUD
./target/debug/wolf-cli.exe --root work/all capture \
  --load-save work/retail_backup/Save/SaveData01.sav --frames 60 \
  --out work/out/hudfix/sd01_new.png
# 对照图（零售 save1.jpg / 修复前 / 修复后）：
#   work/out/hudfix/cmp_bar_retail_old_new.png
```

实测（`x=500` 列，y 10..34，白/灰/黑像素行数）：零售 `W=15 G=0`；修复前 `W=11 G=4`；修复后 `W=15 G=0`。

## 7. 边界与后续

- `wolf-layout` 目前导出 `150`（图片）与 `101`（消息）文本；`102` 选择、`121/250` 数据库读取等
  尚未纳入表格，但都可按同一方式扩展（它们不含"绘制位置"，位置仍来自对应的 150 文本/窗口层）。
- magic 变量（如 `1600012`）原样打印；要看当期数值可配合 `wolf-cli dump --print-self` /
  `capture --watch-var`，或直接读存档 part5。
- `<CIRCLE>` 等窗口元素在此规则下也是**横向优先、纵向恒定**（如 ev232 的 70×70 <CIRCLE>
  scale150 会画成 105×70）。这是零售存档证据支持的统一规则；若将来有零售帧显示某类 window
  确实等比放大，再按类型细分。
- 若确需 Ghidra 复核 `150` 的引擎实现（确认"单值只作用横向"），可作为交叉验证；但**布局数据
  本身不依赖它**。
