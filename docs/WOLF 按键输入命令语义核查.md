# **WOLF RPG Editor 事件命令 123 / 125 / 126 语义与键输入机制核查报告**

经核查 WOLF RPG Editor（ウディタ）官方在线帮助文档，事件命令 123、125、126 统一归属于「キー入力」命令族，分别对应「キー入力受け付け」（123，读取或等待当前按键状态）、「自動キー入力」（125，向下一帧注入虚拟键鼠操作）以及「キー入力の許可/禁止」（126，切换基本输入与输入设备的许可/禁止状态）1。在优先查证源中，官方 GitHub Pages 镜像（https\://smokingwolf.github.io/tool\_wolf\_rpg\_editor/help 及 04ev\_keyinput.html）在访问时不可达4，离线 CHM 手册亦无从获取，但官方网站主站页面 https\://silversecond.com/WolfRPGEditor/Help/04ev\_keyinput.html 完整返回了未截断的正文内容，并与官方更新日志 ReleaseLog 系列及兼容说明 02ver\_ajust.html 共同构成最高可信度依据3。  
需要率先明确的是：**官方帮助文档与官方更新日志仅面向图形界面操作，从未公开事件命令在工程数据中的数字编号（123/125/126）及参数二进制位掩码定义表**3。命令编号与英文标识映射来自 GitHub 第三方开源解析项目 WolfTL（记录 123 为玩家键输入检查/等待、124 为 SetVariableEx、125 为 AutoInput、126 为输入禁用切换）2；部分官方正文中未展开列举的数值键码（如 10/11/12 与鼠标 20–22）来自非官方社区教程站 yado.tk（标注更新至 2025/05/24、对应 Ver3.600）9。对于您提取的 123/126 二进制参数值，下文依据精确比特/字节分解结果进行结构解析10，所有缺乏官方或源码实证的比特位归属均单列于“不确定的地方”，不做猜测。

## **核心命令对照表（命令、参数、语义、原文引用与来源 URL）**

下表汇总命令族 123 / 125 / 126（并附带常被混淆为键输入的扩展变量命令 124）的界面参数、运行时语义、日文原文引用及来源可信度。

| 命令编号与名称 | 界面参数与结构 | 核心运行时语义 | 日文原文引用（附中文解释） | 来源 URL 与可信度 |
| :---- | :---- | :---- | :---- | :---- |
| 123：キー入力受け付け | 代入先変数；キー種（基本 / キーボード全キー / マウス / パッドボタン / アナログスティック / POV十字キー / 複数タッチ）；方向与按键复选、特定键判定及 5 种取得时机3 | 检查执行瞬间按下的按键或输入设备状态，向指定变量写入单个标量键码、坐标、滚轮增量、摇杆数值或帧数；无按键写入 0（无触摸写入 \-1）3 | 「このコマンドが実行された瞬間に押されているキーをチェックし、その対応した番号を変数に代入します。何もなければ0が代入されます。」（检查该命令执行瞬间按下的键，将对应编号代入变量；无按键则代入 0。）3 | 官方手册 https\://silversecond.com/WolfRPGEditor/Help/04ev\_keyinput.html（高）；命令号来源 https\://github.com/Sinflower/WolfTL（第三方工具）2 |
| 124：変数操作+（SetVariableEx，非键输入命令） | 目标变量与扩展引用项（如 \[ピクチャ\] マウス重なってる？、\[マウス\] マウスX/Y座標、\[マウス\] マウス左/中/右クリック、\[マウス\] マウスホイール変化）2 | 属于扩展变量运算，而非「キー入力」命令族；图片鼠标悬停检测（返回 1 表示光标在图片范围内）位于此命令下2 | 「マウスカーソルがそのピクチャの範囲内にあるとき、1を返します。」（鼠标光标位于该图片范围内时返回 1。）12 | 官方手册 https\://silversecond.com/WolfRPGEditor/Help/04ev\_valuenext.html（高）；命令号来源 https\://github.com/Sinflower/WolfTL（第三方工具）2 |
| 125：自動キー入力（AutoInput） | キー種（3 类：基本入力 / キーボード / マウス左・中・右クリック・座標）；键码或鼠标动作与目标坐标2 | 作为“下一帧”（延迟约 1 帧 ≈ 0.017 秒）输入自动模拟键鼠操作；「キーボード」可穿透 126 禁止，「基本入力」仍受 126 基本禁止阻塞3 | 「『次のフレーム』のキー入力として、各種キー・マウス操作を自動で行うことが出来ます。」（作为“下一帧”的键输入，可自动执行各类按键与鼠标操作。）3 | 官方手册 https\://silversecond.com/WolfRPGEditor/Help/04ev\_keyinput.html（高）；命令号来源 https\://github.com/Sinflower/WolfTL（第三方工具）2 |
| 126：キー入力の許可/禁止 | 两大分支：基本入力（决定/取消/副/↓/←/→/↑ 复选 \+ 三态状态）；入力デバイス（单键盘键/鼠标全部/手柄全部/全部设备 \+ 禁止/许可）2 | 局部修改所选键或设备的许可/禁止状态；未选对象保持不变；状态跨事件、跨地图保留，且在修复早期 Bug 后随存档保存与恢复3 | 「決定・キャンセル・サブキー上下左右、および、キーボード/マウス/パッド入力のそれぞれの許可・禁止状態を切り替えることができます。」（可切换决定/取消/副键/上下左右以及键盘/鼠标/手柄输入各自的许可・禁止状态。）3 | 官方手册 https\://silversecond.com/WolfRPGEditor/Help/04ev\_keyinput.html 与 https\://silversecond.com/WolfRPGEditor/old\_releaselog/ReleaseLog05.html（高）；命令号来源 https\://github.com/Sinflower/WolfTL（第三方工具）2 |

## **A. 命令 123「キー入力受け付け」语义核查**

在命令 123 的参数设计上，官方手册 https\://silversecond.com/WolfRPGEditor/Help/04ev\_keyinput.html 最初仍保留着「・キー種 … ３種類あり、それぞれ受け付けるキーが異なります」（键种有 3 种，各自接收的键不同）的旧文描述，但同一页面紧随其后的现行规格表已列出了 7 个读取大类：基本（方向/決定/ｷｬﾝｾﾙ/ｻﾌﾞ）、キーボード全キー（100～）、マウスクリック・座標、パッドボタン（301～）、アナログスティック、POV十字キー与複数タッチ認識3。在最常用的「基本」类别下，界面不仅有方向键接收范围下拉框，还有決定キー、キャンセルキー、サブキー三个独立复选框，以及自 Ver3.50 起扩展的 5 种取得时机（「通常の押し状態を取得」「押されるまで【待つ】」「新押し時のみ取得」「離した時のみ取得」「押し続けフレームを得る」）3。因此，参数 2 在底层并非简单扁平枚举，而是**由多比特选项位与模式字段打包而成的复合整数（位掩码/位字段组合）**3。  
就查询列举的参数值而言，其精确二进制位分解为：16 \= 0x0010（bit 4）、32 \= 0x0020（bit 5）、49 \= 0x0031（bits 0, 4, 5）、144 \= 0x0090（bits 4, 7）、145 \= 0x0091（bits 0, 4, 7）、176 \= 0x00B0（bits 4, 5, 7）、177 \= 0x00B1（bits 0, 4, 5, 7）、771 \= 0x0303（bits 0, 1, 8, 9）以及 784 \= 0x0310（bits 4, 8, 9）10。由于官方手册从未公开二进制位掩码定义表3，16、32、49、145、176 等掩码中各个比特位精确对应哪几个复选框或取得模式无法从官方资料中直接证实；能够确定的是，在运行时命令 123 仅检测用户在界面上勾选/启用的那部分键位或设备状态3。  
关于返回值类型，官方手册与官方入门指南共同证实：**命令 123 返回的是单个标量数值（键码、帧数、坐标、倾角或滚轮变化量），绝对不是位集合**3。官方手册明确规定若无输入则返回 0（多点触控无触摸返回 \-1）3；官方入门指南 https\://silversecond.com/WolfRPGEditor/Guide/PRO\_005.html 进一步给出原文说明：「1つのキー入力で複数のキーを受け付けようとすると、優先順位によって上書きされてしまい、結局1つ分しか取得する事ができません。しかし、代入先変数を変更した複数のキー入力コマンドを利用すれば、同時に複数のキー入力を取得できます」（若试图用 1 条键输入接收多个按键，会因优先级覆盖而最终只能取得 1 个；使用不同代入变量的多个键输入命令即可同时获取多个输入）17。  
当多个被允许的键同时按下时，非官方社区站 http\://yado.tk/wolf/02\_ibekoma/0900\_key/（可信度为社区参考，标注对应 Ver3.600）记录了如下仲裁规则：多个方向键同按时取得“最后按下”的方向键码9；键盘全键（100～）多键同按时取得“键码数字较小”者9。值得注意的是，yado.tk 在同一页面上关于“同时启用決定(10) 与 4方向(2,4,6,8) 时谁优先”出现了内部表述冲突——其日文原文段落写为「下図のように『決定(10)＋４方向(2,4,6,8)』で設定した場合は、方向キーのキーコードが優先されます」（按图示设为决定(10)+4方向时，方向键键码优先）9，而摘要提取记录却译作决定键(10)优先9。由于官方手册正文并未写明该优先级细节，实践中应当直接采纳官方 PRO\_005.html 的拆分多变量方案以避免覆盖17。至于 mask=771 是否专门表示鼠标滚轮：对比 771（0x0303，低字节 3、次低字节 3）与 784（0x0310，低字节 16、次低字节 3）可知，两者共享 byte1 \= 3（0x0300）10，说明 771 很可能是“高字段=3”类别或模式下叠加了低位标志 3，而非专用于鼠标滚轮的孤立常数，但确切界面选项归属仍属未证实；官方文档仅记载鼠标滚轮通过选择「マウスクリック・座標」下的「ホイール回転」读取3。

## **B. 命令 126「キー入力禁止/許可」语义核查**

在命令 126（https\://silversecond.com/WolfRPGEditor/Help/04ev\_keyinput.html）的语义定义中，参数 1 既不是纯粹的“禁止掩码”，也不是纯粹的“允许掩码”，而是“操作对象选择 \+ 目标权限状态”的打包指令3。官方手册原文阐述了其双分支结构：「選択したキーに対し『常に受け付ける(ﾃﾞフォルト)』『移動時禁止・キー入力時だけ許可』『移動時もキー入力時も禁止』 の設定が可能です。たとえば主人公のカーソルキーによる移動を禁止したいときは、『上下左右』を選択し、『移動時禁止・キー入力時だけ許可』にすることで実現できます」（可对所选键设置“总是接受（默认）”、“移动时禁止・仅键输入时许可”或“移动与键输入时均禁止”……）3，以及「キーボードの各キー（1つずつ設定可）・マウス全部・パッド全部・全てのデバイスの入力を許可・禁止にできます」（可对键盘单键、鼠标全部、手柄全部及全部设备的输入执行许可或禁止）3。  
从您给出的数值分解来看：240 (0x000000F0, bytes \[240, 0, 0, 0\])、496 (0x000001F0, bytes \[240, 1, 0, 0\]) 与 752 (0x000002F0, bytes \[240, 2, 0, 0\]) 共享低字节 0xF0（bits 4, 5, 6, 7），仅在次低字节 byte1（bits 8, 9）上分别取 0、1、210；而 0x10000001 (268435457, bytes \[1, 0, 0, 16\])、0x10000101 (268435713, bytes \[1, 1, 0, 16\]) 与 0x10000041 (268435521, bytes \[65, 0, 0, 16\]) 均置位了高位 bit 28（0x10000000），其中 0x10000101 与 0x10000001 同样在 byte1 上表现为 1 与 0 的切换10。这与界面上「基本入力」存在 3 种状态、「入力デバイス」存在 2 种状态的语义高度契合，但比特位 0、4..7、6 及高位 0x10000000 各自精确对应哪项按键、设备（是否包含鼠标/手柄）或模式标志，官方均未披露，无法确定3。  
关于状态覆盖、清除与生命周期规则，官方文档与历史更新日志提供了清晰依据。首先，命令 126 **仅局部覆盖本次命令所选中键或设备的权限状态，既不重置未选中的项，也不存在计数叠加**。官方 FAQ https\://silversecond.com/WolfRPGEditor/Help/01FAQ.html 给出在游戏启动事件中单独禁用键盘码 283（PrintScreen） 而不影响其他键的标准做法16；社区问答（Yahoo\! 知恵袋，非官方参考）亦印证了只禁用 キャンセル 或 172 单键的局部行为，同时反映了若在并行事件中循环执行禁止指令会不断覆写外界许可操作15。其次，126 的状态**跨事件、跨地图持久保留，且在存档内序列化保存**。官方在地图 0 标题事件中禁用 283 即可在整局游戏的所有地图生效16，不会因事件结束或切换地图自动清除；而在读档方面，官方日志 https\://silversecond.com/WolfRPGEditor/old\_releaselog/ReleaseLog05.html 明确记载：「【キー入力】『キー入力の許可/禁止』の状態がセーブされていなかったバグを修正 → 入力を禁止した状態でロードすると、ロード後も禁止状態が継続されてしまう 問題が」（修复了“键输入许可/禁止”状态未被保存的 Bug → 此前会导致在禁止状态下读档后禁止状态依然延续）7。这证明现代版本中 126 状态随存档保存，读档时会还原存档当时的禁止/许可状态7。至于 126\[0\] 是否等价于清除全部禁止，以及 F12 返回标题时是否复位全部禁止，官方手册未有明文记载。  
关于 126 的作用域边界，它绝不仅仅影响命令 123 的轮询。在「基本入力」中设定的三态直接控制地图上主人公的移动与按键触发3，且当「基本入力」被禁止时，命令 125 的「基本入力」自动按键也会失效3。官方版本兼容说明 https\://silversecond.com/WolfRPGEditor/Help/02ver\_ajust.html 确认了历史修复：「【Ver3.582版 キー入力】『キー入力禁止』で方向キーを禁止しても、『選択肢』における方向キー処理が受け付けられてしまうバグを修正。→ 前の挙動に合わせて作っていた場合は、バージョン調整を『旧Ver3.582時点の挙動』以前にしてください」（修正了旧 Ver3.582 行为中禁止方向键后「選択肢」仍接收方向键的 Bug）6——这意味着在现行版本中，禁止方向键会**同步阻塞对话选项（選択肢）的光标移动**6。官方更新日志 https\://silversecond.com/WolfRPGEditor/ReleaseLog.html 与 ReleaseLog07.html 亦确认禁用 4 方向后通过手柄 POV 十字键移动的漏洞已被修复（联动系统变量 Sys134）8。此外，据社区资料 yado.tk 说明，WOLF RPG 的自带菜单、战斗与商店界面均由公共事件调用命令 123 驱动，因此同样受 126 约束9。至于禁用「マウス全部」是否影响命令 124（変数操作+）的图片悬停检测（マウス重なってる？）11 或 Sys73 文章选项的鼠标点击，官方资料未做明确说明。

## **C. 命令 125「自動キー入力」语义及与 123 / 126 的关系**

命令 125（WolfTL 命名为 AutoInput2）在官方手册 https\://silversecond.com/WolfRPGEditor/Help/04ev\_keyinput.html 中称为「自動キー入力」，其准确语义是**向“下一帧”的引擎输入管线注入虚拟按键或鼠标动作**（因此响应存在 1 帧、按帧率约 0.017 秒的延迟），而非无视一切限制直接改写目标变量，更不是静态赋值3。官方给出的典型用途包括回放系统、演示动画以及用鼠标模拟操作 RPG，并在 04ev\_keyinput.html 与 https\://silversecond.com/WolfRPGEditor/Help/04ev\_string.html 中注明了一项官方技巧：在「文字列操作」进行“キーボード入力”期间，用 125 自动输入键码 128（Enter） 可以中途强制完成输入3。  
命令 125 与 126 之间存在一项**官方着重提醒的不对称穿透规则**：当 125 选择「キーボード」类型注入物理键码时，它可以**穿透** 126 设置的禁止状态；但当 125 选择「基本入力」类型时，它**无法穿透** 126 对基本输入的禁止3。官方手册原文对此解释得极为透彻：「なお、この機能で『キーボード』を入力すると、後述する『禁止』にしたキーでも入力を受け付けられますので、ご注意下さい。プレイヤーの入力を不能にしつつ、ウディタ側だけでキー入力できるようにする目的です。ちなみに『基本入力』を禁止した場合は、『自動キー入力』による基本入力も効かなくなります」（需注意用此功能输入“键盘”时，即使是被后述命令设为“禁止”的键也会被接收，其目的是在封锁玩家输入的同时仅让编辑器侧自行按键；顺带一提，若禁止了“基本输入”，则通过“自动键输入”触发的基本输入也将失效）3。（注：早期 ReleaseLog03.html 曾简略表述为可无视禁止，后被正式手册细化修正3。）  
在参数编码体系上，125 并不与 123 完全共用同一套“キー種”参数结构：官方手册指明 125 的「キー種」仅有 3 类——「基本入力」「キーボード」「マウス左・中・右クリック・座標」，不包含 123 所支持的手柄按键（301～）、模拟摇杆、POV 十字键或多点触控读取，且包含 123 所没有的“将鼠标移动至指定坐标”输出字段3。两者底层是否局部共享基础键种 ID 尚无二进制文档可证。

## **D. 键码表与位布局核查**

必须严格区分“命令 123 写入变量的运行时键码（Key Code）”与“事件命令参数内部存储的位布局（Bitmask）”。您所列举的 10=OK、11=Cancel、12=Sub、2/4/6/8=方向 属于**运行期返回给变量的键码**，而非比特掩码中的位索引3。以下按官方与社区来源可信度逐项核对键码表与位表情况：

| 输入类别 | 键码 / 返回值范围 | 精确对应内容与依据来源 | 可信度分级 |
| :---- | :---- | :---- | :---- |
| 基本入力：方向键 | 1–9（不含 5） | 官方手册明确列为小键盘排列：7(左上)、8(上)、9(右上)、4(左)、6(右)、1(左下)、2(下)、3(右下)，四方向即 2(下)、4(左)、6(右)、8(上)3 | 官方手册 04ev\_keyinput.html（最高）3 |
| 基本入力：功能键 | 10、11、12 | 官方确认基本输入含決定（默认 Enter/Space）、キャンセル（默认 Esc/Backspace/Delete）、サブ（默认 Shift），可由系统变量 52–57 重映射3；具体数字 10=决定、11=取消、12=副键 见于社区站 yado.tk9 | 键名与重映射为官方（最高）3；数字 10/11/12 为社区参考 yado.tk9 |
| 鼠标输入 | 按键 20–22；坐标为屏幕整数；滚轮为单帧变化量 | 官方确认点击状态接收左/右/中键、鼠标 X/Y 坐标、滚轮旋转（一般为 \-1～-2、0、+1～+2，部分机型可能持续输出 ±100 以上）3；社区站 yado.tk 记鼠标按键键码范围为 20–22，但未在所查正文单列三键各自对应的数字9 | 动作/滚轮/坐标为官方（最高）3；键码范围 20–22 为社区参考 yado.tk9 |
| 键盘全键 | 100～（特殊键需勾选「特定キーのみ判定」） | 官方手册正文提及 128=Enter（主/小键盘 Enter 同按后只松主 Enter 变 256）、248=半角/全角（常驻保持）、283=PrtScr、299=Home3；官方 ReleaseLog02.html 记 158=CAPS23；社区站记 132=D、172=小键盘815 | 列举键码出自官方手册与官方日志（最高）3；其余个别字母键出自社区站15 |
| 手柄按键 | 301–315（对应ボタン 1–15） | 官方 2024/01/17 对照表：301=ボタン1（Xbox A / PS □ / Switch B）、302=ボタン2（Xbox B / PS × / Switch A）、303=ボタン3（X / ○ / Y）、304=ボタン4（Y / △ / X）、305–308 对应肩键/扳机与 Back(View)/Start(Menu)、309–315 对应摇杆按下、SHARE/OPTIONS/PS/Home/截图/静音3；yado.tk 泛称 300以上9 | 官方手册 04ev\_keyinput.html（最高）3 |
| 手柄摇杆 / POV / 触控 | 摇杆 ±1000、0–3600、0–1000；POV 1–9；触控坐标/ID（无触摸 \-1） | 摇杆上下/左右 \-1000～+1000、角度 0～3600（右为 0 顺时针增）、倾角 0～1000、ZL/ZR 0～1000（受 Sys146 统一映射）；POV 十字键同 1–9；多点触控按索引 0 起读取 X/Y 与跟踪用 ID3 | 官方手册 04ev\_keyinput.html（最高）3 |

针对“是否存在一份官方「キー種」位表（1/2/4/8/16/32/64/128/256/512/0x10000000...）”的问题：经全面检索，**官方网站与说明书体系内不存在任何公开的二进制位布局表**3。

## **E. 实际游戏工程数据的位结构分析与推断边界**

虽然缺少官方位掩码定义表，但将您提供的 123 掩码频次与 126 事件位置进行精确比特和字节拆解后，仍可在不越界猜想的前提下识别出清晰的参数结构特征10。

### **命令 123 参数频率的位分解与结构特征**

| 123 参数值（频次） | 十六进制 | 已置位比特索引（bit 0 起） | 字节拆分 \[byte0, byte1, byte2, byte3\] | 二进制结构特征 |
| :---- | :---- | :---- | :---- | :---- |
| 145（91 次） | 0x00000091 | 0, 4, 7 | \[145, 0, 0, 0\] | 144 \| 1，最高频组合10 |
| 144（28 次） | 0x00000090 | 4, 7 | \[144, 0, 0, 0\] | 16 \| 128（较 145 仅少 bit 0）10 |
| 176（20 次） | 0x000000B0 | 4, 5, 7 | \[176, 0, 0, 0\] | 144 \| 32（在 144 上叠加 bit 5）10 |
| 16（10 次） | 0x00000010 | 4 | \[16, 0, 0, 0\] | 单个独立比特 bit 410 |
| 177（7 次） | 0x000000B1 | 0, 4, 5, 7 | \[177, 0, 0, 0\] | 176 \| 1（bits 0, 4, 5, 7 全置位）10 |
| 49（3 次） | 0x00000031 | 0, 4, 5 | \[49, 0, 0, 0\] | 1 \| 16 \| 32（不含 bit 7 的 177）10 |
| 771（1 次） | 0x00000303 | 0, 1, 8, 9 | \[3, 3, 0, 0\] | byte1 \= 3，byte0 \= 3 (bits 0, 1\)10 |
| 784（1 次） | 0x00000310 | 4, 8, 9 | \[16, 3, 0, 0\] | byte1 \= 3，byte0 \= 16 (bit 4\)10 |

在这总计 161 次调用中，除 771 外的 **160 次均置位了 bit 4（16）**，且前 6 个高频值完全由 {bit 0, bit 4, bit 5, bit 7} 这四个正交布尔位的子集组合而成；bit 7（128）在其中充当了清晰的分组开关（区分出 {144, 145, 176, 177} 与 {16, 49}）10。结合常规游戏大量使用基本按键循环（如轮询方向键并按需搭配决定/取消/副键或等待模式），这组正交位高度符合界面基本输入选项与取得模式的位标志设计3。而 771（\[3, 3, 0, 0\]）与 784（\[16, 3, 0, 0\]）在 byte1 上同为 3（bits 8, 9），仅在 byte0 上分别为 3 与 1610，说明两者同属一个 byte1 \= 3 的特定类别或子模式，而非 771 单独表示鼠标滚轮；但各个比特精确映射至哪个界面选项仍待编辑器实测确认。

### **命令 126 出现位置的成对模式与结构特征**

| 事件与位置（命令 126） | 十进制与十六进制 | 已置位比特索引 | 字节拆分 \[byte0, byte1, byte2, byte3\] | 事件流中的成对状态变化 |
| :---- | :---- | :---- | :---- | :---- |
| ev126 pc41 → pc45 | 496 (0x000001F0) → 240 (0x000000F0) | \[4,5,6,7,8\] → \[4,5,6,7\] | \[240, 1, 0, 0\] → \[240, 0, 0, 0\] | 保持 byte0 \= 0xF0，byte1 由 1 切回 010 |
| ev126 pc76 → pc80 | 496 (0x000001F0) → 240 (0x000000F0) | \[4,5,6,7,8\] → \[4,5,6,7\] | \[240, 1, 0, 0\] → \[240, 0, 0, 0\] | 保持 byte0 \= 0xF0，byte1 由 1 切回 010 |
| ev225 pc244 → pc249 | 752 (0x000002F0) → 240 (0x000000F0) | \[4,5,6,7,9\] → \[4,5,6,7\] | \[240, 2, 0, 0\] → \[240, 0, 0, 0\] | 保持 byte0 \= 0xF0，byte1 由 2 切回 010 |
| ev234 pc654 → pc656 | 268435713 (0x10000101) → 268435457 (0x10000001) | \[0,8,28\] → \[0,28\] | \[1, 1, 0, 16\] → \[1, 0, 0, 16\] | 保持 0x10000001，byte1 由 1 切回 010 |
| ev232 pc691、ev328 pc3 | 268435457 (0x10000001) | \[0, 28\] | \[1, 0, 0, 16\] | 与 ev234 pc656 同构（byte1 \= 0）10 |
| 0x10000041（提问参考） | 268435521 (0x10000041) | \[0, 6, 28\] | \[65, 0, 0, 16\] | 0x10000001 \| 0x40，byte0 增加 bit 610 |

结合事件指令间距与官方界面控件数量，上述数据呈现出两条严谨的结构性推断（注：以下属基于结构同构的工程推断，因缺乏官方位表，需以 .mps 实测最终定锤）。第一，在 {240, 496, 752} 与 {268435457, 268435713} 中，作者均在极短的 2 至 5 步操作区间内成对切换 byte1，并在区间结束时将 byte1 置回 010，同时在 ev232 末尾与 ev328 开头单独放置 byte1 \= 0 作为解锁保险——这表明 **byte1（bits 8..9）编码了目标权限状态，0 代表恢复/许可，1 与 2 代表不同程度的限制或禁止**。第二，0xF0（bits 4, 5, 6, 7）是唯一同时出现三态 byte1 ∈ {0, 1, 2} 且精确占据 4 个连续比特的操作对象；对照官方「基本入力」界面中唯一具有三态且恰好 4 键成组的「↓ / ← / → / ↑」方向键组3，可以推测作者在 ev126 中两次短暂将 4 个方向键切换至限制态 1 后立即恢复（默认基本系统中公共事件 126 也正是处理步行按键并临时锁方向的事件9），在 ev225 中则短暂切换至另一限制态 2 后立即恢复。相对地，0x10000001 与 0x10000101 仅呈现出二态 byte1 ∈ {0, 1} 且携带高位 0x1000000010，这种二态性符合「入力デバイス」（鼠标/手柄/全部设备仅含许可・禁止二态）3 或另一套双态功能开关的特征，而 0x10000041 中 byte0 \= 65（低于官方键盘键码起始线 100～3）亦更像 bit 0 \| bit 6 的设备或选项位组合而非直接键盘键码。  
综上，针对“这些值是否符合官方位表”的提问：由于**并不存在官方公开位表**3，无法逐比特比对；但这些数字呈现的三态 byte1 对 4 连位组、二态 byte1 对高位 0x10000000 以及 123 中围绕 {0, 4, 5, 7} 的正交开关模式，在结构上完全契合 WOLF RPG Editor 官方文档所描述的界面控件与状态逻辑3。

## **实现建议：123 与 126 的引擎语义伪代码**

为便于编写兼容运行器，下面按官方已确认的语义关系给出参考实现架构。由于尚未通过编辑器空白工程实测出完整的二进制比特映射表，代码中将未证实的比特位拆解定义为可替换的 \*\_BIT\_MAP 校准表，以防止硬编码错误假设（代码框内不附加引文标号，相关行为依据均已在前文及下文逐条核证）。

Python  
\# \=== 1\. 全局权限状态（必须随存档 Save/Load 序列化，跨事件、跨地图保留） \===  
class InputPermissionState:  
    \# 基本输入三态: 0=常に受け付ける(默认), 1=移動時禁止・キー入力時だけ許可, 2=移動時もキー入力時も禁止  
    \# 注：实际字节码中 byte1 的 1 与 2 分别对应哪种禁止级别，需由样本 .mps 校准确认  
    basic\_dir\_state \= {2: 0, 4: 0, 6: 0, 8: 0}  
    basic\_btn\_state \= {10: 0, 11: 0, 12: 0}  \# 10=決定, 11=キャンセル, 12=サブ

    \# 输入设备二态: True=許可, False=禁止  
    keyboard\_allowed \= {}   \# keycode (100+) \-\> bool (未设置默认 True)  
    mouse\_allowed \= True  
    pad\_allowed \= True

\# 待通过空白 .mps 实测填充的比特位映射表（严禁猜测硬编码）  
CALIBRATED\_126\_DIR\_BITS \= {}   \# 例: {bit\_idx: dir\_code}  
CALIBRATED\_126\_BTN\_BITS \= {}   \# 例: {bit\_idx: btn\_code}  
CALIBRATED\_123\_FILTERS  \= {}   \# 例: 解析 123 raw\_mask 为允许的 keycode 集合与取得时机

\# \=== 2\. 命令 126: キー入力の許可/禁止 \===  
def exec\_cmd\_126(perm: InputPermissionState, raw\_param1: int, extra\_keycode: int \= None):  
    \# 由实测值 \[240/496/752\] 与 \[0x10000001/0x10000101\] 观察到的结构：  
    target\_state \= (raw\_param1 \>\> 8) & 0x03       \# byte1 呈现 0/1/2 状态切换  
    is\_high\_branch \= bool(raw\_param1 & 0x10000000) \# bit 28 高位标志（具体语义待校准）

    if not is\_high\_branch:  
        \# 仅局部更新本次置位的对象，未置位对象保持现有状态不变  
        for bit\_idx, dir\_code in CALIBRATED\_126\_DIR\_BITS.items():  
            if (raw\_param1 \>\> bit\_idx) & 1:  
                perm.basic\_dir\_state\[dir\_code\] \= target\_state  
        for bit\_idx, btn\_code in CALIBRATED\_126\_BTN\_BITS.items():  
            if (raw\_param1 \>\> bit\_idx) & 1:  
                perm.basic\_btn\_state\[btn\_code\] \= target\_state  
    else:  
        \# 高位分支：依据校准后的设备/单键位表切换二态 True/False (target\_state \== 0 表示恢复允许)  
        apply\_calibrated\_device\_permission(perm, raw\_param1, target\_state, extra\_keycode)

\# \=== 3\. 运行时权限过滤判定（涵盖移动、選択肢、123 与 125） \===  
def is\_basic\_input\_allowed(perm: InputPermissionState, key\_code: int, context: str) \-\> bool:  
    \# context 取值: "MAP\_MOVE" \| "CHOICE" \| "CMD123" \| "AUTO\_BASIC"  
    \# 官方兼容规则：Ver3.582 之后修复版本中，禁止方向键亦会阻塞 "CHOICE"（選択肢）  
    state \= perm.basic\_dir\_state.get(key\_code, perm.basic\_btn\_state.get(key\_code, 0))  
    if state \== 2:      \# 移動時もキー入力時も禁止（同样阻断 125 基本自动输入）  
        return False  
    if state \== 1:      \# 移動時禁止・キー入力時だけ許可  
        return context \== "CMD123"  
    return True

\# \=== 4\. 命令 123: キー入力受け付け（恒定写入单个标量，非位集合） \===  
def exec\_cmd\_123(perm: InputPermissionState, dest\_var: int, raw\_mask: int, input\_state, game\_vars):  
    spec \= decode\_123\_spec(raw\_mask, CALIBRATED\_123\_FILTERS)  \# 提取类别、过滤键集与 5 种取得时机

    if spec.category \== "BASIC":  
        \# 采集方向(1..9)与功能键(10/11/12)，受 Sys52-57 重绑定、126 设备/基本权限及 Sys134(POV) 约束  
        \# 注意：若 126 禁止基本输入，125 注入的"基本自动输入"在此处也必须过滤掉  
        active\_codes \= poll\_basic\_with\_permissions(input\_state, perm, spec.allowed\_codes)  
        result \= resolve\_priority\_and\_timing(active\_codes, spec.acq\_mode)  \# 返回单个标量键码或 0

    elif spec.category \== "KEYBOARD\_ALL":  
        \# 采集 100+ 键码；关键穿透规则：命令 125(键盘) 注入的键可绕过 126 的键盘禁止！  
        \# 多键同按按社区记录取键码数值较小者  
        candidates \= poll\_keyboard\_100plus(input\_state, perm, allow\_cmd125\_bypass=True, specific=spec.specific\_key)  
        result \= resolve\_min\_keycode\_and\_timing(candidates, spec.acq\_mode)

    elif spec.category \== "MOUSE":  
        \# 读取点击(社区记 20-22)、坐标或滚轮变化量；受 perm.mouse\_allowed 影响（边界待实测）  
        result \= poll\_mouse\_scalar(input\_state, perm, spec.mouse\_target)

    elif spec.category \== "PAD\_BUTTON":  
        result \= poll\_pad\_301\_315(input\_state, perm) if perm.pad\_allowed else 0

    else:  
        \# アナログスティック / POV十字キー / 複数タッチ認識（无触摸返回 \-1）  
        result \= poll\_extended\_device(input\_state, perm, spec)

    \# 若取得模式为"押されるまで【待つ】"且 result \== 0，则挂起当前事件指令指针至下一帧继续检测  
    game\_vars\[dest\_var\] \= result

## **不确定的地方（未获可靠来源证实的事项与验证方法）**

以下 6 项细节在官方手册、更新日志及可查第三方资料中均无明确直接依据，特此单列、不做臆测：

> 1. 命令 123 参数 2 的精确比特分配：无法从官方文档证实 16、32、49、144、145、176、177、771、784 中 bit 0, 1, 4, 5, 7, 8, 9 各自对应的界面复选框、方向下拉项、取得模式或读取大类（包含 771 与鼠标滚轮是否存在关联）。  
> 2. 命令 126 参数 1 的精确比特分配与高位含义：无法证实 240 (0xF0) 中 bits 4..7 与具体按键的一一对应关系、byte1 \= 1 与 byte1 \= 2 谁对应“仅移动禁止”或“完全禁止”，以及 0x10000000（bit 28）、0x10000001（bit 0）、0x10000041（bit 6）的确切界面对应项。  
> 3. 126\[0\] 与特殊复位边界：缺乏官方说明确认执行参数为 0 的 126 命令是否清空全部禁止，也缺乏关于按下 F12 返回标题界面时引擎是否自动重置 126 禁止状态的文字记载。  
> 4. 鼠标禁用的交叉影响边界：尚无官方文档明确说明 126 禁止「マウス全部」后，是否会连带屏蔽命令 124（変数操作+）的图片悬停判定（マウス重なってる？）、124 鼠标坐标/点击读取、Sys73 对话鼠标点击，以及命令 125 的自动鼠标点击是否可穿透 126 鼠标禁止。  
> 5. 非官方键码明细与同按优先级冲突：決定=10 / キャンセル=11 / サブ=12 及 マウス=20–22 出自社区站 yado.tk 而非官方手册正文3；20、21、22 三者具体如何分配给左、右、中键在所查资料中缺失；此外 yado.tk 在“決定(10)+4方向(2,4,6,8)”同时启用时的优先级描述上存在日文（方向键优先）与英文摘要（决定键优先）的自相矛盾9。  
> 6. 命令 125 与 123 的二进制编码复用度：两者的界面类别数不同（125 仅 3 类输出，123 现含 7 类读取）3，底层枚举值是否部分共用缺乏二进制规范证实。

如需彻底消除第 1、2、4、6 项的不确定性，最可靠的核实方法是在 WOLF RPG Editor 中建立空白测试地图，分别插入只修改单个复选框或单一设备选项的命令 123 / 125 / 126，保存 .mps 后直接对比其整数字段的二进制差分，或运行空白工程实测鼠标禁用对 124 悬停与 125 点击的实际响应。

## **官方示例工程与说明书示例截图情况**

在官方在线帮助 https\://silversecond.com/WolfRPGEditor/Help/04ev\_keyinput.html 页面中，原文共嵌入了 14 张官方界面说明图片（所有 alt 属性均为空字符串），涵盖命令 123 全部 7 种读取模式的编辑器面板截图、由「ナナミ 様」调查提供的全键盘键码对照表图（以 ● 标注标准全键模式下可直接捕获的键位）、命令 125 三种自动键输入面板截图以及命令 126 基本输入/输入设备面板截图3。由于本次网页文本提取仅返回了图片位置标记（url0 及 url3–url16）而未暴露原始图片路径，且镜像站 smokingwolf.github.io 不可达3，无法在此直接附带截图直链或预览；您可在浏览器中直接打开上述 silversecond.com 官方文档查看原图。  
在官方示例工程与操作范例方面，官方并未发布专门演示 123/125/126 二进制参数的示例工程，但有以下几处直接可用的标准工程参考：官方 https\://silversecond.com/WolfRPGEditor/Help/01FAQ.html 指明了官方自带示例工程（サンプルゲーム）中地图 0 的「タイトル用イベント」可作为游戏启动后立即用「キー入力の許可/禁止」封禁键盘码 283（PrintScreen） 的范例位置16；官方入门指南 https\://silversecond.com/WolfRPGEditor/Guide/PRO\_005.html 与 https\://silversecond.com/WolfRPGEditor/Guide/PRO\_009.html 演示了如何配置命令 123 接收基本按键并利用多条指令配合局部自我变量（如 このEv:セルフ1）读取多键同时输入17；此外，官方基本系统自带的公共事件 \[126: X\[移\]歩行時\_並列キー処理\]（此为公共事件编号 126，而非事件命令编号 126）在社区参考站 http\://yado.tk/wolf/05\_kihon/0126\_hokou\_key/ 中留有逐行解析，展示了系统层如何在移动与菜单切换期间利用「キー入力禁止/許可」临时管控方向键9。

#### **引用的著作**

> 1. キー入力, [https\://www\.silversecond.com/WolfRPGEditor/Help/04ev\_keyinput.html](https://www.silversecond.com/WolfRPGEditor/Help/04ev_keyinput.html)  
> 2. Sinflower/WolfTL \- GitHub, [https\://github.com/Sinflower/WolfTL](https://github.com/Sinflower/WolfTL)  
> 3. [https\://silversecond.com/WolfRPGEditor/Help/04ev\_keyinput.html](https://silversecond.com/WolfRPGEditor/Help/04ev_keyinput.html)  
> 4. [https\://smokingwolf.github.io/tool\_wolf\_rpg\_editor/help](https://smokingwolf.github.io/tool_wolf_rpg_editor/help)  
> 5. [https\://smokingwolf.github.io/tool\_wolf\_rpg\_editor/help/04ev\_keyinput.html](https://smokingwolf.github.io/tool_wolf_rpg_editor/help/04ev_keyinput.html)  
> 6. Game.exe 動作バージョン調整, [https\://silversecond.com/WolfRPGEditor/Help/02ver\_ajust.html](https://silversecond.com/WolfRPGEditor/Help/02ver_ajust.html)  
> 7. WOLF RPGエディター バージョンアップ履歴, [https\://silversecond.com/WolfRPGEditor/old\_releaselog/ReleaseLog05.html](https://silversecond.com/WolfRPGEditor/old_releaselog/ReleaseLog05.html)  
> 8. WOLF RPGエディター バージョンアップ履歴 Ver3.65～, [https\://silversecond.com/WolfRPGEditor/ReleaseLog.html](https://silversecond.com/WolfRPGEditor/ReleaseLog.html)  
> 9. YADOT-WOLF キー入力, [http\://yado.tk/wolf/02\_ibekoma/0900\_key/](http://yado.tk/wolf/02_ibekoma/0900_key/)  
> 10. [unknown\_url](http://docs.google.com/unknown_url)  
> 11. YADOT-WOLF 自作メニューの基礎講座, [http\://yado.tk/wolf/07\_menu/0000\_kiso/](http://yado.tk/wolf/07_menu/0000_kiso/)  
> 12. 変数操作＋, [https\://silversecond.com/WolfRPGEditor/Help/04ev\_valuenext.html](https://silversecond.com/WolfRPGEditor/Help/04ev_valuenext.html)  
> 13. ウディタ(WolfRPGエディタ)に関して質問です。 \- 現在, [https\://detail.chiebukuro.yahoo.co.jp/qa/question\_detail/q11316736871](https://detail.chiebukuro.yahoo.co.jp/qa/question_detail/q11316736871)  
> 14. YADOT-WOLF 【使用例】キー入力, [http\://yado.tk/wolf/02\_ibekoma/0910\_rei\_key/](http://yado.tk/wolf/02_ibekoma/0910_rei_key/)  
> 15. ウディタで、主人公の移動キーを方向キーじゃない他のキーに設定, [https\://detail.chiebukuro.yahoo.co.jp/qa/question\_detail/q13227421918](https://detail.chiebukuro.yahoo.co.jp/qa/question_detail/q13227421918)  
> 16. よくある質問, [https\://silversecond.com/WolfRPGEditor/Help/01FAQ.html](https://silversecond.com/WolfRPGEditor/Help/01FAQ.html)  
> 17. 特定のキーが押されたかどうかを取得したい, [https\://silversecond.com/WolfRPGEditor/Guide/PRO\_005.html](https://silversecond.com/WolfRPGEditor/Guide/PRO_005.html)  
> 18. ウディタで一時的にメニュー画面を開くのを禁止するにはどうした, [https\://detail.chiebukuro.yahoo.co.jp/qa/question\_detail/q10148210340](https://detail.chiebukuro.yahoo.co.jp/qa/question_detail/q10148210340)  
> 19. WOLF RPGエディター バージョンアップ履歴 Ver3.50～3.649, [https\://silversecond.com/WolfRPGEditor/old\_releaselog/ReleaseLog07.html](https://silversecond.com/WolfRPGEditor/old_releaselog/ReleaseLog07.html)  
> 20. WOLF RPGエディター 更新履歴 Ver2.20～2.296まで, [https\://silversecond.com/WolfRPGEditor/old\_releaselog/ReleaseLog04.html](https://silversecond.com/WolfRPGEditor/old_releaselog/ReleaseLog04.html)  
> 21. 文字列操作, [https\://silversecond.com/WolfRPGEditor/Help/04ev\_string.html](https://silversecond.com/WolfRPGEditor/Help/04ev_string.html)  
> 22. WOLF RPGエディター 更新履歴 Ver2.00～2.10まで, [https\://silversecond.com/WolfRPGEditor/old\_releaselog/ReleaseLog03.html](https://silversecond.com/WolfRPGEditor/old_releaselog/ReleaseLog03.html)  
> 23. WOLF RPGエディター 更新履歴 Ver1.10～1.30まで, [https\://silversecond.com/WolfRPGEditor/old\_releaselog/ReleaseLog02.html](https://silversecond.com/WolfRPGEditor/old_releaselog/ReleaseLog02.html)  
> 24. 19.デバッグ＆調整 – はじめてのウディタ \- ぴぽや, [https\://pipoya.net/rpg/rpg\_step19/](https://pipoya.net/rpg/rpg_step19/)  
> 25. YADOT-WOLF コモン\[126:X\[移\]歩行時\_並列キー処理\]の解説, [http\://yado.tk/wolf/05\_kihon/0126\_hokou\_key/](http://yado.tk/wolf/05_kihon/0126_hokou_key/)  
> 26. 複数のキーが同時に押されたかどうかを取得したい, [https\://silversecond.com/WolfRPGEditor/Guide/PRO\_009.html](https://silversecond.com/WolfRPGEditor/Guide/PRO_009.html)