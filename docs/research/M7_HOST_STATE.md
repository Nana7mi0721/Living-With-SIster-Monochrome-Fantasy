# M7 主机侧 124 查询残缺字段（H-124 后续）

## 结论

opcode 124 SetVariableEx 的解释器部分在 HEAD 1d36ebe 已完整（见 docs/research/M7_OPCODE_124.md 与 crates/wolf-vm/tests/opcode_124_long.rs）。本任务补的是 Host 侧（真实 Game）对少数字段的回答：

| sub / field | 之前 | 现在 | 依据 |
|---|---|---|---|
| picture_state field 5 | 返回 None（目标变量保持旧值） | 返回 Some(0)（仅对存在的图片） | docs/OPCODE_124.md 6.6 / 9.3：本作仅 1 处，暂返回 0 |
| engine_state field 11（鼠标下 EvID） | None | Some(-1) | 移植版无地图交互层，指针下永远没有地图事件；-1 是既有 none 哨兵 |
| engine_state field 12（接触 EvID） | None | Some(-1) | 同上，无接触层 |
| engine_state field 17 | None | Some(0) | 依据未定（9.1），但 5 处均作消息窗口自增守卫，0 与旧值本为 0 一致 |
| character_state 全部字段 | None | None（未改） | 移植版没有任何地图/角色层，见下 |

没有伪造任何坐标：character_state 保持 None，理由见“无法证明的残余”。

## 代码路径（crates/wolf-game/src/game.rs）

- picture_state：game.rs:1907-1934；新增 field 5 分支在 game.rs:1926（注释 1922-1925）。
  结构：field 8/9 先于存在性检查（不存在也能答 -1/0）；随后 screen.get(number)? 拿到图片，
  0=>x、1=>y、2|3=>scale、4=>extras[0]、10=>文本是否显示完；未知字段=>None。
- character_state：game.rs:1936-1944（注释说明为何不伪造）。
- engine_state：game.rs:1946-1980；新增 11/12/17 在 game.rs:1969-1975。
  已有 0=>0、1=>-1、7/8/9=>鼠标三态、10=>0、20/21=>鼠标坐标。
- mouse_state（三态：0 抬起 / 1 持续按住 / 2 本帧刚按下）：game.rs:1463。

## 证据（docs/OPCODE_124.md）

- 6.6 覆盖表：0x3000 11/12 = 鼠标下的 EvID / 拡張EvID，少数，P1；0x4000 field 5 = 未定，本作 1 处，P2（返回 0）。
- 7.1 第 5 行：CE ev225 pc70-71 成对读 11/12，注释“マウス画像変更” => 11/12 是鼠标下事件号。
- 9.3：pic field 5 本作仅 1 处（SampleMapA#26 pc218；本机静态扫描为 CE233 obj120），无注释，暂返回 0。
- 9.1：0x3000 field 17 本作 4 处（くうきいぬ 消息窗口包），用 v/5 %100 *15000000 与 270035461 比较，只用于消息窗口自增控制；
  新版官方文档说 17 = 処理中のEv行数，但本作 index 已右移，不能直接套，需编辑器/真机断点（本机扫描计到 5 处：CE265-270）。

## 无法证明的残余（不在本切片伪造）

1. engine_state field 17 的真实语义仍未定（候选“处理中的事件行数”）。本作所有站点只把它当自增守卫，
   故返回 0 与旧行为一致；若将来出现依赖真实值的用法，需按官方定义重算。
2. character_state：移植版不存在地图/角色层。wolf-format::mps 的 MapEvent 有 x/y 瓦片坐标（mps.rs:230）与
   MapEventPage 的 direction（mps.rs:337），但 Game::start_pages（game.rs:583-592）把解析出的 Map 丢弃，
   MapRuntime（map_runtime.rs，746 行）只跟踪 page 程序、无坐标/朝向/显示；TileMap（tilemap.rs:35）也无事件坐标；
   全局没有主角/队长位置。因此“老实返回 None”是唯一不破坏跟随者/炸弹逻辑的选择。
3. engine_state 11/12 的 -1 只在“当前无地图交互层”时成立；一旦实现地图事件渲染/碰撞层，必须改为真实事件号。
   现有数据里 11/12 各 1 处（CE225）。

## 回归测试

新文件 crates/wolf-game/tests/m7_host_state.rs（3 个测试，全部经真实 Game::new 与真实 opcode 124 查询驱动，
读真实 GameHost，不是 mock）：

1. engine_state_reports_the_mouse_and_neutral_none_sentinels_through_opcode_124
   - mouse=[640,300]，mouse_buttons=1（上帧 0）；124 查 0x3000 field 20/21/17/11/12/7。
   - 断言：var600=640、601=300、602=0（17）、603=-1（11）、604=-1（12）、605=2（7 刚按下）；warnings=0。
   - 先把目标变量种成 424242，证明是“写入 0/-1”而非“保持旧值”。
2. picture_state_field_five_answers_zero_for_live_pictures_only
   - screen.insert 图片 12004（x=111,y=222,scale=150,extras=[7,8]）。
   - 124 查 field 0/1/2/4/5 + 未知字段 700 + 对不存在图片 99999 查 field 5。
   - 断言：610=111、611=222、612=150、613=7、614=0（field 5）、615/616=424242（未知字段与不存在图片均保持旧值）；warnings=0。
3. character_state_stays_none_so_no_coordinates_are_fabricated
   - 124 查 0x1000 obj=-2（队长）field 6/5。
   - 断言：620/621=424242，即 Host 返回 None、目标不被伪造坐标污染。

运行命令与结果：

~~~
cargo test -q -p wolf-game --test m7_host_state
# 3 passed; 0 failed

cargo test -q -p wolf-game --no-fail-fast
# 所有二进制全绿：225 unit + 各集成测试（含 m7_host_state 3）0 failed
~~~

说明：测试中调用 game.auto_events.set_enabled(false)，避免并行自动事件写入普通变量干扰断言。

## 影响面

改动只影响三个字段的返回值，且这些字段在全作分别只有 1/1/1/5 个站点：
- picture field 5：1 处（地图事件画图）；
- engine 11/12：各 1 处（CE225）；
- engine 17：5 处（消息窗口自增守卫，行为等价 0）。
全量 wolf-game 测试全绿，无回归。docs/UNVERIFIED.md 的 H-124 残余清单（11/12/17、picture field5）
在本任务后已过时，需 lead 决定是否更新（本任务写入范围不含该文件）。
