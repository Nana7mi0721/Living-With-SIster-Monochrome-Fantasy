# 未验证假设清单（UNVERIFIED）

本文件登记**已经写进代码、但证据不足以称为定论**的判断。每条写清：假说、当前实现、支持证据、仍未验证的部分，以及「怎么才能证实」。

> **本文件最后整理：2026-11-03（M13 / P2 之后）**。
>
> 历史审计见 `docs/research/M7_UNVERIFIED_AUDIT.md`（截至 HEAD bcbcfba）；其后 M8–M13 与 P2 的证据已并回本文件。若两者冲突，以本文件为准。
>
> 约定：某条一旦被证实或推翻，就从本文件删除，并把实测结论写进 `docs/FORMAT_SPEC.md` / `docs/OPCODE_SPEC.md` 的对应小节。**本文件是「待实测」，那两份是「已实测」。**

## 状态总览

原清单 9 条现在的状态：

| 分组 | 条目 | 说明 |
| --- | --- | --- |
| **仍开放** | H-PIC-1、H-KEY-2、H-PRNG-1、H-ASSET-1 | 低风险视觉 / 键码 / 确定性 / 数据笔误；无发布阻断 |
| **已实现，仅剩证据或表示缺口** | H-PIC-2、H-ANC-1 | 槽位与锚点几何已落地并通过测试；仍缺「纵向缩放表示」或「5/6 锚点单独零售像素」 |
| **已解除** | H-WIN-1、H-MENU-1、H-124、opcode 111 | 已由像素对照 / 数据几何 / 回归测试锁定 |
| **清单外真残余** | CE159/CE162、CE333、character_state、PRNG 时钟、音频 | 见文末「其它真残余」 |

统计：原 9 条中 **3 条已解除**（H-WIN-1、H-MENU-1、H-124），**2 条已实现但仅剩证据/表示缺口**（H-PIC-2、H-ANC-1），**4 条仍开放**（H-PIC-1、H-KEY-2、H-PRNG-1、H-ASSET-1）。另有 opcode 111 等旧账已在后续证据中解除，列在文末「已纠正的过时否定」。

---

## 一、仍开放

### H-PIC-1 负番号图片 = 全屏图层

**假说**：`150` 的图片番号为负数时（本作实测只有 `-2`），该图片是「全屏背景图层」：忽略 x / y / scale，直接铺满 1024×576。

**实现**：`crates/wolf-game/src/picture.rs` 的 `Picture::is_screen_layer` / `Screen::quads`：负番号 → src = 纹理全图，dst = (0,0,1024,576)。

**支持证据**：
- 标题画面第一条绘制 `ev42 pc20`：`[0,-2,30,1,1,1,255,0,0,100,0] str=SystemGraphic/TitleGraphic.jpg` —— 该 JPG 本身正好是 1024×576（逻辑屏尺寸），若按普通图片解释也会铺满，所以这条其实**无法区分**两种解释。
- 本作共出现 30 次负番号绘制，全部出现在标题/读档这类「整屏换图」的位置。
- 早期 `eval_num` 把 `MagicKind::Opaque` 求值成 0，导致 -2 变成 0、本分支成了死代码；改为「Opaque 返回原值」后已在标题画面命中，并通过像素对照。

**仍未验证**：负番号的真实含义（WOLF 文档没有写）；`-1` 是否代表另一种哨兵；负番号是否允许动画/淡入。

**怎么证实**：找一个用了负番号且图片**不是** 1024×576 的场景，看游戏里是铺满还是按原尺寸画。

---

### H-KEY-2 键码模型与 `123` 掩码语义（H-KEY-1 已作废）

**结论（已实测）**：`123` 写进目标变量的是**键码**（10 決定 / 11 キャンセル / 12 サブ / 2·4·6·8 小键盘方向），不是位掩码。旧假说 H-KEY-1（`key_bits & mask`）**已作废**：那个值永远不等于脚本比较的常量，标题菜单一个分支都进不去。证据见 `docs/OPCODE_SPEC.md` §5.13（全部来自本作 `103` 调试注释：事件 108 / 65 / 118 / 510）。

**实现**：`crates/wolf-vm/src/exec.rs` 的 `Host::key_code(mask) -> i32`（默认 0），`123` 分支写它；`crates/wolf-game/src/key.rs` 维护端口内部位布局与 `code()` 映射。

**仍未解（两条）**：
1. **`arg[1]` 掩码语义** —— 一次轮询能返回六种键码（事件 108 掩码 177：先得 10/11/2，else 再得 8/4/6），所以不是「一位一键」。当前实现接受掩码但忽略它，回答「当前按下的键的键码」；对全库所有 `!= 0` / `== 10` 形式的比较都成立。若将来出现「同一次轮询要区分 Alt/方向组合」的脚本，就必须真正解开掩码。
2. **未命名的键码 13 / 14 / 20 / 100 / 300** —— 出现在比较里，本作没有注释能说明它们的来源（鼠标键？手柄？滚轮？）。目前已按「永不返回」处理（端口不会产生这些码），一旦有脚本依赖它们就会走进 else 分支。

**怎么证实**：原版实机按 F 系列的 info 键或直接在标题菜单试上下+決定，比对光标行为；或反编译 `Game.exe` 里读取 `GetKeyState` / DirectInput 的那一小段。

---

### H-PRNG-1 随机数流

**假说**：`121` 的 `0x6000` 算子 = `rand(min,max)`（闭区间）。**这一条已经证实**（1,101 处，见 `docs/OPCODE_SPEC.md` §3.2）；这里登记的是它的**实现**不确定性。

**实现（M8 W5-a）**：`121` 的 `0x6000` 随机分支已按反编译结果落地（`docs/research/M8_PRNG_RANGE.md` §1-2）。引擎在 RVA `0xaf7c0` 调用 RVA `0x35810` 的 **Park-Miller MINSTD**（`state = state*16807 % 0x7FFFFFFF`，超过 2,000,000,000 再乘一次，为 0 则重播种），取 31 位状态后用 `low + draw % (abs(b-a)+1)`（32 位回绕）归约；每次调用（含零宽区间）消耗一次抽样。`crates/wolf-vm/src/rng.rs` 新增 `next_event_draw()` / `from_event_seed()` / `set_event_seed()`，`range()` 改用该事件流与上述模运算律；M7 的 MSVC C 运行时 LCG（`next_raw()`，`state*0x343FD+0x269EC3 mod 2^32`）**只用于 Game.dat 静态表/解密流**，已被 M8 §3 证实不是事件发生器。默认事件种子为固定值 `EVENT_SEED=1`（零售用时钟自播种；零状态重播种为固定 `EVENT_RESEED=1`），以便回放确定。`cargo test -q -p wolf-vm` 全绿（unit 91 / rng_retail 15）。

**残余未验证**：零售事件发生器以时钟自播种，本移植用固定种子 → **同一存档 + 同一输入能否与原版逐位对齐仍未证**（缺一次实机运行的时钟种子）；抽样算法与归约律本身已是反编译证实。见 `docs/research/M8_PRNG_RANGE.md` §6。

**怎么证实**：在原版运行时于 RVA `0x35810` 下断点或读首次状态，取出该次运行的种子；用同一种子 + 同一存档/输入驱动本移植并逐帧对比输出。

---

### H-ASSET-1 `Picture/word_effect*` 作者笔误（已用带诊断的 fallback 兜住）

**现象**：事件 354 里 7 个 `word_effect` 引用写成 `Picture/word_effect30|36|55|56|57|58|59.png`，但全作 119 个真实文件都在 `hword/word_effect01..119.png`，`Picture/` 下没有任何 `word_effect*`（`word/` 下是同名的 30×100 占位图）。

**处理**：资产解析加了一条**刻意收窄**的回退 `Picture/word_effect* -> hword/word_effect*`（`crates/wolf-game/src/assets.rs`），命中时向诊断流吐一条 `1x asset fallback: Picture/word_effectNN.png -> hword/word_effectNN.png`，其余资产仍严格「缺文件即报」。

**仍未验证**：原作运行时是否也认这个笔误（可能 DxLib 在 `Picture/` 不存在时另有搜索路径），以及作者原本想引用的是 `hword/` 还是 `word/`。若今后确认是脚本写错，应在数据侧修掉这 7 个引用，而不是永远依赖 fallback。

**怎么证实**：对照原版实机事件 354 的文字特效是否与 `hword/` 图一致。

---

## 二、已实现，仅剩证据或表示缺口

### H-PIC-2 图片字段布局（长格式槽位下标）

**当前结论**：`150` 长格式（argc ≥ 11）的字段布局已经确定并实现：

| raw 下标 | 含义 |
| --- | --- |
| `[3]` / `[4]` | 窗口宽 / 高（只对 type 3/4） |
| `[6]` | 不透明度（255 为主） |
| `[7]` / `[8]` | X / Y（大量来自变量 magic） |
| `[9]` | 统一缩放百分比（100 为主） |
| `[10]` | 混合模式（0 / -1000000 / 1250） |
| `[2]` | **FADE_IN：淡入时长**（原「含义未知」已解除） |
| `[5]` | **RESERVED：文件图的一基 sprite-sheet cell**（原「含义未知」已解除） |

**实现**：`crates/wolf-game/src/picture.rs` 的 `mod field` 常量表 + `Screen::apply`（下标已相对 `raw[2..]` 减 2）。回归：`crates/wolf-game/tests/m8_anchor_slots.rs` 4 测。

**支持证据**：
- 标题提示条 `SystemFile/Button01.png`（606×35）`ints=[4096,12001,0,1,1,1,255,512,520,100,0]`：按本布局得到 (512,520) 中心锚点 606×35，**与零售版 1024×576 截图逐像素吻合（mean abs 0.35/255，98.6% 像素差 ≤2）**；早期「`[2]`=X、`[4]`=scaleX」的布局会把同一张图画成 (30,1) 处的 1% 色块。
- 13,637 条长格式样本的取值分布全部自洽：`[6]` 以 255 为主、`[9]` 以 100 为主、`[10]` 只有三个值。
- 窗口图片 `#12500` `[0,300,200,1,255,855,200,100,0,…]` → 300×200 窗口框；文字图片 `#12501` 得到 `[3]/[4]=1,1`（自适应框哨兵）。

**横向镜像已实现**：负的单一 `scale` 解释为横向镜像（`crates/wolf-game/src/picture.rs` 用 `unsigned_abs` + 逐列 `mirrored_columns`）；回归 `crates/wolf-game/tests/m8_negative_scale.rs` 5 测。

**仍未实现 / 未验证**：**纵向缩放与纵向镜像**。存档里有独立的 Y 百分比（`vars7_18[10]`），但 `Picture` 没有独立 Y scale 字段；目前只支持统一缩放 + 横向镜像。

**怎么证实**：找一处只改 Y 缩放、不改 X 缩放的 `150` 站点，在零售版量其高度变化；同时确认 `vars7_18[10]` 在存档中的一基/零基偏移，再决定是否给 `Picture` 增加独立 Y scale。

---

### H-ANC-1 锚点位 0x2000..0x6000 的含义

**当前结论**：`150` 头字 bits 12..14 是锚点，**0..6 已全部实现**：

| code | 锚点 |
| --- | --- |
| 0 | 左上 |
| 1 | 中心（两轴） |
| 2 | 左下 |
| 3 | 右上 |
| 4 | 右下 |
| 5 | 上中 |
| 6 | 下中 |

code 7 在本作零站点，按左上回退。

**实现**：`crates/wolf-game/src/picture.rs` 的 `place()`：按锚点分别减 `w/2`、`w`、`h`、`h/2` 等。回归：`crates/wolf-game/tests/m8_anchor_slots.rs` 4 测、`crates/wolf-game/tests/m11_anchor_pixels.rs` 5 测。命中测试已改用 `Picture::destination`（不再重复实现一套中心锚点逻辑）。

**支持证据**：`0x1000` 组 4,393 条的 X 集中在 512、Y 集中在 288（正好屏幕中心）；标题提示条的像素级对照证实两轴都居中。其余 6 个 nibble 共 333 条样本（占 2.4%），X/Y 分布分别偏向 `384` / `1000` / `1024` / `800` 等值。

**仍缺证据**：**anchor 5/6 没有单独的零售像素截图**。另外 `crates/wolf-game/src/textpic.rs` 的 `anchor_origin` 只处理 0–4，5/6 回退左上；但本作没有使用 5/6 锚点的文本图片站点，因此当前无实际影响。

**怎么证实**：挑一条 `0x4000` 且 x=1024、y=570 的（事件层面能定位到具体事件），在零售版里看它是否贴右下角；再用 `0x6000` + x=800、y=576 的那条验证下中；anchor 5 同理找上中站点截图。

---

## 三、已解除

### H-WIN-1 窗口图片的皮肤与九宫格（已解除 ✅）

**假说**：`150` 的 type 3（WindowFile）/ type 4（WindowString）会在内容下方画一块 UI 面板，皮肤来自 `SystemFile/WindowBase.png`（18×18，SysDB「×選択肢ウィンドウ画像」指定；`SystemGraphic/WindowBase.png` 是无人引用的深蓝样例），框厚 3 px，中间行/列拉伸。

**实现**：`crates/wolf-game/src/window.rs`（`nine_slice` + `SKIN` / `CORNER`），由 `Atlas::sprites` 在内容之前、同番号位置插入 9 个四边形。

**证据（已按数据/几何解除）**：
- `SystemFile/WindowBase.png` 逐像素测量：第 0/17 行列为 `0x424242` 外框，第 1 行列为 `0xf2f3f3` 高光，第 2 行列为 `0x576062` 过渡线，3..14 为 `0x1f2425` 灰底且 alpha=0xdf —— 结构上就是 3 px 框 + 拉伸体。CN 公告站点实际引用 `SystemFile/WindowBase_amania.png`，同为 18×18、3 px 九宫格。
- 本作 667 个 type 3/4 窗口站点；`#12500` 是 300×200 窗口、anchor 1（中心），真实 destination = `[705,100,300,200]`，右缘 `1005 < 1024`；`#12501` 文本 `x=715..955`（截图字形盒到 961，多 1 字体像素裕量），全在面板 `1005` 与屏幕 `1024` 内。
- M9 早期记录说「右缘 1155 溢出」是**把 (855,200) 当左上角、忽略 anchor 的算术错误**；`docs/research/M11_WINDOW_EDGE.md` 已用真实 capture 与九宫格几何纠正。
- 回归：`crates/wolf-game/tests/m8_window_skin.rs` 5 测、`crates/wolf-game/tests/m11_window_edge.rs`。

**仅外观备注（非缺陷）**：CN 补丁公告的文字图 `#12501` 不嵌套在面板 `#12500` 内，但文字实测在面板内；按脚本原样渲染，不改坐标、不做自动嵌套。type 3 的 `#12005` 框是 0×0（继承内容尺寸），没有框尺寸就无法九宫格，只画内容。

---

### H-MENU-1 选择肢（102）的可视化责任归属（已解除 ✅）

**假说**：`102` 的选项字符串由**运行时**（不是脚本）画出选择窗口；本作标题菜单的选项串是 `["1","2","3"]`，而玩家真正看到的菜单文字是脚本用图片画出来的，所以忠实移植必须**同时**做两件事：脚本图片照画 + 运行时选择窗口照画。

**实现（已确认绘制，M7 / task-18）**：`crates/wolf-game/src/choice.rs` 的 `overlay_at_sized` 画出九宫格面板/行/光标，`crates/wolf-cli/src/main.rs` 的 `build_capture_frame` 已接线（commit 7293ee8）。回归 `crates/wolf-game/tests/m7_choice_window.rs` 2 测通过；`--event 246` 截图 `work/out/m7_choice_window.png` 显示 6 个选项与 ▶ 光标。原先「尚未绘制」的记录已过时。

**仍属猜测（不影响解除）**：默认位置/边框/字体是从本作事件的实际锚点（如 `(512,192)`）反推的，不是文档值；更广的菜单仍按脚本给出的锚点绘制。

---

### H-124 变量操作（扩展）124（已确认实现，M7 / task-23；指令层已解除 ✅）

**实现（已确认）**：`crates/wolf-vm/src/exec.rs` 已按长格式 `[dst, sub, object, field]`（argc=4）/ 短格式 `[dst, sub, field]`（仅 0x3000，argc=3）完整分派，纯查询、不阻塞；对象走 `eval_num`，目标走 `store_num`；值来自 Host 的 `picture_state` / `character_state` / `engine_state`，`None` 时不写目标（保持旧值）；`0x3000 field 18` 由解释器回答（公共事件号 + 500000）。回归 `crates/wolf-vm/tests/opcode_124_long.rs` 7 测通过。

**证据**：全库（公共事件 + 地图事件）共 1045 站点；公共事件 579 条与旧记录一致：`(4,0x4000) 359`、`(3,0x3000) 174`、`(4,0x1000) 37`、`(4,0x4400) 8`、`(4,0x1200) 1`；`dst` 高 4 位全为 0（纯覆写）；未处理 sub 站点 0。详见 `docs/research/M7_OPCODE_124.md` 与 `docs/OPCODE_124.md`。

**残余（host 侧，非本指令缺陷）**：`engine_state` 11/12 已按反编译实现为逐帧鼠标位移、17 已钉死为「当前公共事件号」（Host 无实例上下文，仍返回 0 中性值）；`picture_state` field 5 已返回 0；`character_state` 自 M8 W3-a 起返回真实地图事件/队长坐标（见 `docs/research/M8_CHARACTER_STATE.md`）。仅 field 13（光标下事件，本作 0 站点）仍返回 None。

---

### opcode 111 值操作数引用（已解除 ✅）

`crates/wolf-vm/src/exec.rs` 的 `let value = eval_num(arg(cmd, 2 + 3 * i), inst, vars);` 已把值操作数走 `eval_num`（commit `3bbbb12`）；`crates/wolf-vm/tests/opcode_111_semantics.rs` 5 测全过。旧记录称「仍按字面量比较、1 测 `#[ignore]`」的账目**已过时**。

---

## 四、其它真残余（原清单外）

以下是「已写代码但仍有证据缺口 / 未实现分支」的真残余，不属于原 9 条，来源为 M7/M8 已合入测试与报告（逐条依据见 `docs/research/M7_UNVERIFIED_AUDIT.md` §2）：

- **CE159/CE162 目标选择执行侧分发（开放）**：目标*生成*已修（ENEMY_COUNT 单元 CData type18 row69 col0=1；CE190 pc36 / CE162 pc12 读取；`crates/wolf-game/tests/m7_battle_target_scheduler.rs` 6 测、`crates/wolf-game/tests/m7_battle_target.rs` 3 测），但 `CE188` 指令线从未生成 `CE159`（技能選択実行），`CE162` 唯一真实调用者是 CE159 `pc247`；`crates/wolf-game/tests/m7_battle_target_flow.rs` 仅为 zz 调试探针，非绿色验收。
- **CE333 H-scene 坏结局不可达（状态缺失）**：入口 `CE246 pc1943` 需 `var157==13`（全库无字面量 13，只有 {1,2,16}），且坏结局需 `v23==0`，而零售 `v23={9,9,10,6,6,9,8,9}` 全 ≥6；强设 v23=0 触发爆炸。CE346 cutin 可达（`crates/wolf-game/tests/m7_hscene_deep.rs`），坏结局不可达（`crates/wolf-game/tests/m7_hmode_state.rs`）。
- **character_state 限制（非缺陷）**：`crates/wolf-game/tests/m8_character_state.rs` 3 测 —— 对地图事件与队长(-2) 已返回真实坐标，但无移动模型、身高 field4 恒 0、`-1` / `-3..-7` 返回 None、屏幕偏移靠推断。
- **PRNG 时钟种子回放位对齐（确定性）**：算式已反编译落地（`crates/wolf-vm/tests/rng_retail.rs` 15 测），残余 = 零售以时钟自播种、移植用固定 `EVENT_SEED=1`，同存档 + 同输入的逐位对齐未证。详见 H-PRNG-1。
- **音频**：无自动化比对，需人耳核对（M8_BACKLOG G10）。

---

## 五、已纠正的旧记录 / 过时否定（供交叉核对）

- opcode 250 写分支**已支持字符串**（`crates/wolf-vm/src/exec.rs`，`crates/wolf-vm/tests/opcode_250_string.rs` 4 测 + `crates/wolf-game/tests/m7_battle_menu_text.rs`），不再是残余。
- CE244 战斗结算**已可达**（M8 W1-c：`crates/wolf-game/src/battle_wiring.rs` `CE242` arg0=行动槽 1；`crates/wolf-game/tests/m7_day4_playable.rs` day4→day5 08:30 无注入）。
- opcode 111 值操作数引用**已落地**（详见上文「已解除」），旧记录称仍按字面量比较的账目已过时。
- H 文字 word_effect 层**已可见**（W2-c：`crates/wolf-game/src/picture.rs` `Transition::from_command` 回退 raw slot 2 淡入时长；`crates/wolf-game/tests/m7_hword_visible.rs`）。
- H 选择窗口(102) **已绘制**（见 H-MENU-1）；H-124 **指令层已解除**（见 H-124）；H-WIN-1 **已按数据/几何解除**（见 H-WIN-1）。
- day4→day5 **存在自然非战斗路径且已证**（`crates/wolf-game/tests/m7_day4_playable.rs`）。
