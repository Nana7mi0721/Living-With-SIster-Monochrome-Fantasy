# M1 实测事实表（供 docs/M1_CLOSURE.md 引用；每行注明产出来源）

## 命令产出的原始报告（work/ 下）
- `work/_m1_closure2.py` → `work/_m1_closure2.txt` / .json：闭包（两条起点）
- `work/_m1_history.py` → .txt/.json：按 picture 号生命史（基于 `work/_m1_trace2.txt`）
- `work/_m1_assets.py` → `work/_m1_assets.txt`/.json：资产可用性
- `work/_m1_screen.py` → .txt/.json：屏幕状态重放（校准用）
- `work/_m1_waits.py` → .txt：180/102/123/175 站点清单
- `work/_m1_picsrc.py` / `_m1_picsrc_var.py` / `_m1_attrib.py` / `_m1_picvar2.py` / `_m1_selfwriters.py`：150 站点归属
- `work/_m1_trace2.txt`：当前 HEAD 的 200 帧 trace（5999 行）
- `work/out/title200.png`：capture 200 帧出图（1024x576）

## 闭包（python work/_m1_closure2.py）
从 TitleMap 地图事件 2：nodes 62 / slots 11205 / non-empty 9843 / opcode 38
从公共事件 42 单独：nodes 60 / slots 11031 / non-empty 9677 / opcode 38
PLAN.md 第 305 行称：TitleMap 事件 2 → 4 个事件 / 671 条指令 / 26 种 opcode ⇒ 三项全错（事件 4 vs 62；指令 671 vs 9843；opcode 26 vs 38）
PLAN 列的 26 项全部在闭包内（missing=[]）；闭包多出 12 项：101,107,112,122,124,176,211,220,221,222,290,1000
opcode 频次（从 m2，n / 出现事件数）：
103 2266/61 · 121 1983/54 · 401 1007/61 · 250 966/56 · 499 700/60 · 111 691/61 · 150 526/17 · 300 348/22 · 498 207/36 · 179 196/34 · 122 185/21 · 290 128/11 · 172 109/37 · 420 98/22 · 213 69/12 · 180 64/17 · 123 41/13 · 106 36/17 · 221 34/4 · 212 30/12 · 171 27/12 · 99 22/14 · 124 19/10 · 101 18/10 · 112 15/11 · 170 12/8 · 176 12/6 · 210 8/6 · 140 7/5 · 222 4/1 · 220 4/2 · 270 3/1 · 1000 2/1 · 211 2/2 · 102 1/1 · 130 1/1 · 175 1/1 · 107 1/1
从 c42 单独时：103 2217/59 · 121 1971/52 · 401 994/59 · 250 966/56 · 499 697/59 · 111 681/59 · 150 513/15 · 300 348/22 · 498 203/35 · 179 191/32 · 122 181/20 · 290 127/10 · 172 109/37 · 420 97/21 · 213 66/11 · 180 47/15 · 106 36/17 · 123 36/11 · 212 29/11 · 171 24/11 · 99 22/14 · 124 19/10 · 101 18/10 · 112 15/11 · 221 15/3 · 170 12/8

事件表（id / slots / real / calls）——来自 work/_m1_closure2.txt：
- m2 109/109 calls=[c42,c549] タイトルマップ用イベント
- c42 135/107 calls=[c14]
- c14 40/32 calls=[c128,c129,c130,c131,c132]
- c549 65/57 calls=[] forSteam[caution]
- c436 109/88 calls=[] セーブパスワード
- c49 119/93 · c50 40/36 · c51 40/36 · c52 41/37 · c54 28/25 · c55 28/25 · c56 28/25 · c59 48/40 · c60 28/23
- c64 865/766 · c65 243/214 calls=[c49,c64] · c66 27/25 · c67 42/38
- c70 686/582 calls=[c105,c122,c123,c124,c74] · c71 71/59 · c74 206/184 calls=[c70] · c75 339/291 calls=[c100]
- c88 308/286 calls=[c89] · c89 122/106 · c90 545/502 · c91 66/63 · c92 43/35 calls=[c105,c90,c91] · c93 752/684 calls=[c106,c122,c123,c124,c64] · c94 698/614 · c95 750/682 · c96 163/147 · c97 46/42 · c98 34/32 · c99 131/119 · c100 116/107 · c101 95/89 · c102 283/268
- c105 30/23 · c106 47/35 · c107 47/35 · c108 118/100 calls=[c49,c90] · c109 403/343 calls=[c101,c105,c106,c112,c113,c114,c122,c123,c124,c49,c59,c64,c65,c66,c93] · c110 212/184 · c111 131/113 calls=[c54] · c112 214/182 · c113 86/73 calls=[c106,c107] · c114 106/91 · c115 30/24 · c116 25/21 · c117 18/15 · c118 284/244 · c119 394/338 · c120 130/108 calls=[c101,c50,c51,c52,c54,c55,c56] · c121 70/61 calls=[c101,c50,c54,c70,c71]
- c122 45/43 · c123 110/96 · c124 53/49 · c128 72/66 · c129 109/99 · c130 71/64 · c131 338/293 calls=[c101,c436,c49,c60,c67,c88,c92,c94] · c132 573/475 calls=[c101,c115,c49,c95]

## 帧扫描（cargo run -q -p wolf-cli -- run --event 42 --frames N）
f / instr / peak / pictures / sounds / choices / teleports / db / effects
1 → 9 / 1 / 0 / 0 / 0 / 0 / 0 / 0
2 → 45 / 1 / 2 / 1 / 0 / 0 / 0 / 0
3 → 47 / 1 / 2 / 1 / 1 / 0 / 0 / 0
4..13 → 47 / 1 / 2 / 1 / 1 / 0 / 0 / 0（恒定 → 卡在 102 choices）
15 → 49 / 1 / 2 / 1 / 1 / 1 / 0 / 0（choice 0 被应答后 180[12] 等满 12 帧 → 130 teleport 在 f≈15 出现）
20/25 → 53 / 1 / 2 / 1 / 1 / 1 / 0 / 0
30 → 653 / 3 / 48 / 1 / 1 / 1 / 23 / 0（peak 3 协程，被调子事件开始画）
50 → 3528 / 3 / 286 / 1 / 1 / 1 / 123 / 72
100 → 15421 / 3 / 1261 / 1 / 1 / 1 / 533 / ...
200 → 39753 / 3 / 3261 / 1 / 1 / 1 / 1373 / 947
图片号首次出现的帧（grep trace）：#0 从 f=2；#12004/#12005/#12053/#12054/#12500/#12501 从 f=28；#12502 从 f=41。

## 200 帧末屏幕状态（cargo run -q -p wolf-cli -- capture --frames 200 --out work/out/title200.png）
frames: 200 (peak 3 coroutine(s))
pictures: 6 live -> 10 textured quad(s), 2 text picture(s)
atlas: 2 image(s), 14.1% of 2048x2048
quad pic 0 at [0,0,1024,576] uv [0.000,0.009,0.500,0.290]
quad pic 12500 九宫格 9 块：[855,200,3,3] [858,200,294,3] [1152,200,3,3] [855,203,3,194] [858,203,294,194] [1152,203,3,194] [855,397,3,3] [858,397,294,3] [1152,397,3,3]
text pic 12053 at [0,0,1,1] payload "0"
text pic 12501 at [715,115,1,1] 文本「欢迎来到Ver2.0DLC！/烦请注意/存档的第一栏/为自动存档所使用/第一栏已…」
font: work/full/Fonts/Lolita.ttf
text: 2 item(s), 63 glyph(s), 8 line(s), 0 substituted
rendered: 1024x576, 492345/589824 non-black = 83.5%
stats: 39753 instruction(s), 3261 picture op(s), 1 sound(s), 1373 db access(es), 0 warning(s)
diagnostics 8 条 distinct：108x 124 0x4000；1x 130 teleport map -1 at (60,34) dir 3；137x 220/221/222 [1600059,1600050,9000024,0]；137x [1600060,1600050,9000029,0]；274x 290 kind 2 for 0 frame(s) (7 args)；428x 290 kind 5 for 0 frame(s)；108x 290 kind 5 for 15 frame(s)；137x 290 kind 5 for 30 frame(s)
肉眼：背景 TitleGraphic.jpg 全屏正常；左上角中文 LOGO 正常；右侧 (855,200,300,200) 9-slice 紫色窗口正常；公告文字被裁（text picture rect 是 1x1）。

## 屏幕重放（python work/_m1_screen.py，独立复算 picture.rs 语义）
200 帧末存活槽 6 个：#0 #12005 #12053 #12500 #12501 #12502（与 capture 的 6 live 一致）
#0 kind0 ops=549 file=SystemGraphic/TitleGraphic.jpg x0 y0 scale100 op255 blend0 window[1,1]
#12005 kind3 ops=1 file=<TRI-RIGHT> x0 y0 window None
#12053 kind2 ops=5 text=\f[0]0 x0 y0 window[1,1]
#12500 kind4 ops=1 x855 y200 window[300,200]
#12501 kind2 ops=1 x715 y115 window[1,1]
#12502 kind1 ops=1 x855 y430 scale25 window[1,1]
重放 quads：4 个（#0 全屏；#12005/#12502 缺纹理跳过；#12053/#12501 1x1）

## 150 Picture 按号生命史（work/_m1_history.txt，3261 条全量）
#0 ops=550（Draw 2 / Move 548），kind {0:413,2:137}，file SystemGraphic/TitleGraphic.jpg ×2，几何态 2 种：{blend0,opacity255,scale100,window[1,1],x0,y0} ×2（line 7 Draw）与 {x0,y0} ×548
#12004 ops=274（Set 137 / Erase 137），kind 3，无 file/text，无几何态
#12005 ops=274（Set 137 / Draw 137），kind 3，file <TRI-RIGHT> ×137，几何 {blend0,op255,scale100,x0,y0} ×137
#12053 ops=1507（Set 137 / Draw 274 / Move 1096），kind {3:137,2:548,0:822}，text '\f[0]0' ×274，几何 2 种 {window[1,1],x0,y0} ×274 与 {x0,y0} ×1096
#12054 ops=274（Set 137 / Erase 137），kind {0:137,2:137}
#12500 ops=137 全 Draw，kind 4，几何 {blend0,op255,scale100,window[300,200],x855,y200}
#12501 ops=137 全 Draw，kind 2，text「欢迎来到Ver2.0DLC！ 烦请注意 …」（trace 经 short() 截到 40 字符），几何 {window[1,1],x715,y115}
#12502 ops=108 全 Draw，kind 1，几何 {x855,y430,scale25,window[1,1]}
合计 550+274+274+1507+274+137+137+108 = 3261 ✓
trace 中 150 引用过的资产只有 2 个：<TRI-RIGHT> ×137、SystemGraphic/TitleGraphic.jpg ×2
按 (num, action, kind) 明细：0 Draw0 ×2 / 0 Move2 ×137 / 0 Move0 ×411 / 12004 Erase3 ×137 / 12004 Set3 ×137 / 12005 Draw3 ×137 / 12005 Set3 ×137 / 12053 Draw2 ×274 / 12053 Move2 ×274 / 12053 Move0 ×822 / 12053 Set3 ×137 / 12054 Erase2 ×137 / 12054 Set0 ×137 / 12500 Draw4 ×137 / 12501 Draw2 ×137 / 12502 Draw1 ×108

## 150 站点归属（work/_m1_picsrc*.txt）
- c42 num_raw=-2 站点：slot 17/20/27/30（TitleGraphic6.jpg 与 TitleGraphic.jpg，a=[0,-2,30,1,1,1,255,0,0,100,0]）；slot 37/40/43/48/51 = TitleGraphic1..5.jpg（a=[0,-2,0,1,1,1,255,0,0,100,0]）；slot 67/71/75/81/85 又是 TitleGraphic1..5（else 分支）
- c549 独占 picture 10/11/12/13（Kagura/bg.png、Kagura.png、Disc.png、inusuku.png）
- m2 独占 12000（SystemGraphic/TitleGraphic.png slot75、Picture/title01.jpg slot77、Erase slot95）与 12001（SystemFile/Button01.png slot78）
- 12500：c94 slot 251 Draw kind4 a=[0,300,200,1,255,855,200,100,0,1600008,33554432,8,0,100,100,100]
- 12501：c94 slot 252 Draw kind2 a=[0,1,1,1,255,715,115,100,0,0,33554432,8,0,100,100,100]，text 是 7 行中文公告
- 12502：c131 slot 77/107/137/167 Erase + slot 81/111/141/171 Draw kind1 a=[0,1,1,1,255,855,430,25,0,3000005]（文件名从字符串变量 3000005 取）
- 12004/12005/12053/12054 以 num_raw=1600021/1600020 出现（c64 92 个、c93 64、c95 88、c94 69 个站点），运行时求值为 120xx
- 运行时 #12005 Draw 唯一可归属 c94 slot 651（num_raw=1600021）

## 阻塞点（work/_m1_waits.txt）
180 Wait 共 64 处。c42 内只在 slot 8 [1]、slot 116 [12]、slot 122 [12]、slot 130 [12]；另有 c59 slot7 [1600001]、c64 104 [7]、c64 337 [5]、c131 55 [9]、c131 222/224/226 [16]、c132 535 [25]、c549 多处 [30]、m2 slot 80 [20]、m2 93 [5]。最大单次等待 30 帧（c549、c109 64/385）。
102 Choices 全闭包仅 1 处：c42 slot 113，a=[19]，s=["1","2","3"]。
123 InputKey 共 41 处：m2 slot 82 [1100000,784]、slot 83 [1100000,49]（标题光标）；其余 mask 常见 16/32/64（方向键）、176/177/241、771、257、512、1、33、100。Host::key_bits() 默认 0 → 非交互运行永远读不到按键。
175 EndGame 仅 1 处：c42 slot 131。
其他：210 CommonEvent 8 处；220/221/222 SaveLoad 在 trace 里 274 次；1000 ProFeature 2 处（kind=5 → WaitState::Frames((ms/16).max(1))）。

## 资产可用性（work/_m1_assets.txt + 手工 stat）
SystemGraphic/TitleGraphic.jpg  full+alldata 358497B 1024x576（trace 引用 2 次）
<TRI-RIGHT>  MISSING（内置图元，不是文件）
BGM/White_Base.mp3  full 缺 / alldata 2819135B（trace 1 次 140）
SystemGraphic/WindowBase.png  full+alldata 3122B 18x18 RGBA（9-slice 皮肤，window.rs 的 SKIN 常量）
Fonts/Lolita.ttf  full 11949844B / alldata 缺
Fonts 目录另有 07yasashi.ttf、GenJyuuGothicXPB.ttf、MSGOTHIC.TTF、PixelifySans-VariableFont_wght.ttf
m2 用到的：SystemGraphic/TitleGraphic.png 424764B、Picture/title01.jpg full 缺 alldata 280653B、SystemFile/Button01.png 13042B、Kagura/{bg,Kagura,Disc,inusuku}.png 2650/70940/87298/122843B
c549 的 Kagura/*.png 全在 work/full

## 关键结论（写进文档的判定）
1. picture 号经 decode_picture 的 number = eval_num(arg(cmd,1)) → -2 走 Opaque 分支返回 0。所以 c42 的 num_raw=-2 Draw 全部落到 picture 0；picture.rs 的 is_screen_layer()（number<0）永不触发。retail 里 -2 显然是「标题背景共用的固定槽」，落到 0 在本作视觉无害（0,0 全屏纹理 + scale100 恰好 1024x576 全屏），但属隐式行为。
2. picture.rs 字段映射被两个真实样本验证：args=[0,300,200,1,255,855,200,100,0,...] → 窗口 300x200 在 (855,200)，与 capture 九宫格一致；args=[0,-2,30,1,1,1,255,0,0,100,0] → x0 y0 scale100 op255 blend0。OPCODE_SPEC.md §5.31 槽位表（[2]=X、[4]=scaleX、[9]=blend）与实现不符。
3. text picture rect 是 1x1：merge_args 只接受 w>0&&h>0 的 window，12053/12501 写成 1x1（window.rs MIN_BOX=2 认为 ≤2 是无显式框哨兵），quads() text 分支 dst=[x,y,w,h] 拿到 1x1。
4. 尖括号（<TRI-RIGHT>/<SQUARE>/<CIRCLE>/<LINE>/<FRAME>/<GRADX-a-b>/<GRADY-a-b>/<C>/<L>/<R>）是 WOLF 内置图元，需程序化绘制，不是资产缺失。
5. -1000000（UNCHANGED）被 picture_arg 保住（0x0F000000 位非 0），坐标求值已生效。
