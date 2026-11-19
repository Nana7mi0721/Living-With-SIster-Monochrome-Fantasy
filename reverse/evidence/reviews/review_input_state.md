# Review: 输入/存读档/标题状态独立审计

- 任务：task-2（revision 2），只读审查；HEAD 8c65153
- 工作目录：E:\Study\github program\imoto life
- 只读声明：未修改源码、测试、release、manifest 或存档；截图/报告只写 work/out/reviews/。

## 0. 结论

| 项 | 状态 | 根因摘要 |
|---|---|---|
| UI-01 存档页滚轮无效 | 已确认（代码 + CE131 dump；无法用 CLI 注入滚轮） | play.rs 把滚轮映射为 UP/DOWN；GameHost::key_code 忽略 mask，脚本收到键码 8/2，不是 CE131 等待的 1/-1 |
| UI-02 存档页右键不能退出 | 已确认（代码 + CE131 cancel 分支） | 右键只写 mouse_buttons bit2，未注入 key::CANCEL；CE131 需要键码 11 |
| UI-03 ESC 后标题菜单消失 | 已确认（稳定截图） | frame() 在出现 SystemFile/WindowBase 时 stop_title_menu()，存档页关闭后没有重启 event43；event42 也随即结束 |
| UI-04 普通弹窗无 hover 预选 | 已确认（两张逐字节相同截图） | handle_choice_input 的 hover 逻辑只在 parked 分支内；非 parked 弹窗从不按鼠标位置更新 choice_cursor |
| 1024x576 / DPI 坐标 | 未发现坐标偏移 | Scaler 整数倍 + 居中 offset；game_point 减 offset 再除 scale；无 DPI scale 因子 |

核心状态链：winit 事件 -> play.rs App::step（写 game.mouse/mouse_buttons/key_bits）-> Game::frame -> GameHost -> 解释器 opcode 123/124 -> 脚本事件。UI-01/02 断点在 play.rs 到 GameHost::key_code；UI-03/04 断点在 game.rs 的标题/选择处理。

## 1. UI-01 存档页滚轮无效（应左右翻页）

### 状态
已确认（代码 + dump）。CLI capture 没有滚轮注入，不能给出真实滚轮前后截图；下面给可核对的代码和事件证据。

### 证据链
1. crates/wolf-cli/src/play.rs:514-527：MouseWheel 只取 LineDelta(_, y)/PixelDelta(pos.y)；y>0 -> wheel |= key::UP，y<0 -> wheel |= key::DOWN；忽略 LineDelta.x，没有滚轮专通道。
2. play.rs:220-225 bits() 把 wheel 并入 held；play.rs:228-238 写入 game.key_bits，之后清 wheel。
3. crates/wolf-game/src/game.rs:306（Game.key_bits）-> game.rs:2122（GameHost.key_bits）-> game.rs:2708-2713：
   fn key_code(&mut self, mask: u32) -> i32 { let _ = mask; crate::key::code(self.key_bits) }
   crates/wolf-game/src/key.rs:119-147 对 UP 返回 CODE_UP=8，对 DOWN 返回 CODE_DOWN=2；没有滚轮 1/-1。
4. 事件 131 trace work/out/ce131.txt:60-66：
   - :60 pc58 op123 ints=[1600021, 771]（存档页 InputKey，目标 1600021，mask 771）
   - :61 pc59 op111 ints=[2,1600021,1,2,1600021,-1,2]（明确比较 1600021 == 1 或 == -1）
   - :63/:66 标签 ホイール上 / ホイール下
   - :300-301 左键分支跳到 ホイール上；:315-316 右键分支跳到 ホイール下
5. crates/wolf-game/src/game.rs:2819-2822 engine_state(10) 仍返回 Some(0)，注释说端口没有滚轮通道；docs/research/M8_ENGINE_STATE.md:262 把这个登记为已知缺口。它不是 CE131 的唯一原因（CE131 走 123），但同一“没有滚轮字段”会影响所有 124 field10 的脚本。

### 为什么错
滚轮上 -> key::UP -> key::code 返回 8，脚本等 1；滚轮下 -> 2，脚本等 -1。pc59 两比较全 false，滚轮落到后续方向键分支，只会上下动槽位，不会进 pc61/64 翻页标签。

### 最小修法
1. play.rs 增加 wheel_dir: i32（+1 上、-1 下；同一次事件一个方向，step 后清空），保留现有 wheel: u32 给普通菜单 UP/DOWN。
2. Game 增加 wheel_dir（或 wheel_delta）传给 GameHost。
3. GameHost::key_code / key_code_auto 在 mask==771 && wheel_dir!=0 时优先返回 wheel_dir；其他 mask 保持现状。mask 771 本作只出现 1 次（docs/OPCODE_SPEC.md:723），不要全局改 key::code()，否则普通菜单上下移动会受影响。
4. 如希望 CE41/CE224 等 124 field10 也工作，让 engine_state(10) 返回同一个 wheel_dir；这不是 CE131 必要条件，但可消除 M8 已知缺口。

### 建议测试
- crates/wolf-cli/tests/m12_ui_input.rs：构造 GameHost/Game，wheel_dir=1 断言 key_code(771)==1；-1 断言 ==-1；其他 mask 仍返回方向键码。
- event131 级：启动 CE131，注入滚轮上/下，断言 1600021 收到 1/-1，且有存档 fixture 时 1600002 页码变化。
- 普通菜单回归：不改 key::code()，方向键上下仍工作。

### 复现限制
CLI 只支持 --press-frame（左键）和 --key-frame/--press-every（键码），没有滚轮注入；因此本项是代码 + dump 已核对，未能截图复现。静态参照：work/out/ui_bugs/01_load_screen_1023x576.png、work/out/reviews/probe_menu_click.png。

## 2. UI-02 存档页右键不能退出（右键=ESC）

### 状态
已确认（代码 + CE131 cancel 分支）。CLI 没有右键注入，无法做右键前后截图。

### 证据链
1. play.rs:529-543 MouseInput 把 Left/Right/Middle 写 mouse_buttons bit1/2/4；右键只设 bit2，没有 key 位。
2. play.rs:228-238 只有左键 left_click_fresh() 注入 key::OK；右键没有对应注入。
3. game.rs:2708-2721 key_code/key_code_auto 只看 Game.key_bits，从不读 Game.mouse_buttons；右键 bit2 无法变成 key::CANCEL。
4. game.rs:1735-1737 只有 pressed & key::CANCEL 才回复 ChoiceReply::Cancel；右键永远不触发。
5. CE131 cancel：work/out/ce131.txt:202-204（pc200 キャンセル -> pc201 InputKey [1600020,33] -> pc202 检查 10/11/2），:335-338（关闭画面、Wait6、清 2000302）。脚本需要键码 11。
6. 影响不只在存档页：crates/wolf-game/src/key.rs:19-20 记录事件 510 pc975 的图片注释 EXIT "长按右键"，mask 32，结果与 11 比较；当前右键同样不产生 11，该零售交互也失效。

### 最小修法
在 play.rs 的 bits()（或 step() 计算 key_bits 处）把右键按住映射为 key::CANCEL：
  let mut bits = self.wheel;
  for bit in &self.held { bits |= *bit; }
  if self.mouse_buttons & 2 != 0 { bits |= key::CANCEL; }
  bits
- 保留 Game.mouse_buttons bit2，不要改 engine_state(8) 的右键三态（game.rs:2807-2817）。
- 按住右键时 pressed 只第一帧为真；CE131 的 InputKey 是轮询，每个 held 帧都能读到 11，行为与 ESC 按住一致。
- 左键与右键同时按住时 key::code() 的 OK 优先，是可接受边界。

### 建议测试
- m12_ui_input.rs：mouse_buttons=2 时 game.key_bits & key::CANCEL != 0，key::code(game.key_bits)==11。
- 启动 CE131，右键按下 1-2 帧，断言存档页 WindowBase 消失、实例结束；再启动一次断言 1600020 收到 11。
- 反向：右键按住不应产生 key::OK，不应误确认槽位。

## 3. UI-03 ESC 退出存档后标题三行菜单消失且无法操作

### 状态
已确认（稳定截图）。

### 复现
Lead 的稳定命令（仓库根）：
  TEMP=$PWD/work/tmp TMP=$PWD/work/tmp target/debug/wolf-cli.exe --root release/data capture --boot --frames 1900 --key-frame 850:10 --key-frame 950:2 --key-frame 1050:10 --key-frame 1350:11 --out work/out/esc_after_save4.png
输出 work/out/esc_after_save4.png（1024x576，727722 bytes，sha256 6e8c5bba1f74292f7257ebf2bd0edfbc2a24c59b13d2ba872325155545419e1d）：只剩标题 CG，三行菜单不出现。

我的 review 侧复现：
  target/debug/wolf-cli.exe --root release/data capture --menu-only --frames 1100 --pointer 200,460 --press-frame 300 --key-frame 600:11 --key-frame 1000:11 --out work/out/reviews/probe_after_esc_ui03.png
work/out/reviews/probe_after_esc_ui03.png 为 1024x576；统计 24974 instr / 184 picture ops / 1 sound / 1287 db / 1 choice；画面只有标题图，没有 Picture/menu* 三行。对照 work/out/reviews/probe_menu_click.png：同一指针 200,460 在 frame 300 点击后进入存档页，说明坐标命中正常，问题发生在退出存档页之后。

### 状态机链路（为什么没恢复）
1. game.rs:713-723 start_title_menu()：只有当前 VM 没有 event43 实例时才 start_event(43)；event43 生成 Picture/menu1_b.png、menu2_b.png、menu3_b.png（work/wolftl2/dump/common/43_┗オリジナルタイトル画面Ｂ.json:34,55,76）。
2. game.rs:757-779 stop_title_menu()：停掉所有 event43 实例，并只在 slot 3..=5 文件名以 Picture/menu 开头时删除它们。
3. game.rs:1742-1762 frame() 在 VM step 之前：
   self.handle_title_menu_input();
   self.handle_choice_input();
   if self.title_menu_live() && self.screen.has_file_containing("SystemFile/WindowBase") { self.stop_title_menu(); }
   存档/读档页由 CE94 画 SystemFile/WindowBase，所以进入存档页那一帧会把 event43 和 3/4/5 三行全部清掉。
4. ESC 只触发 CE131 cancel（work/out/ce131.txt:202-204，关闭画面在 :335-338）；没有任何代码再次调用 start_title_menu()。start_title_menu() 只在 boot（game.rs:909-924）和 start_title()（game.rs:1281-1289）里被调用。
5. 事件 42 dump work/wolftl2/dump/common/42_オリジナルタイトル画面Ａ.json:913-1016：标题选项在 cpu 77-79 设 Sys3=512/Sys4=1000 并打开 Choices ["1","2","3"]；case2（继续）在 cpu80-84 Wait12 -> Teleport，case3（读取）在 cpu85-88 Wait12 -> CommonEvent [500014,1,-1]。CE14 返回后 event42 没有 LoopEnd/没有重新启动 event43，直接结束；标题图仍在，菜单协程已不存在。
6. switch_map（game.rs:1187-1223）和 start_map_event_by_name（game.rs:1235-1257）都会清屏并停标题菜单，但没有“当前在标题上下文”的标记，也没有恢复路径。

### 最小修法
在 Game 增加明确标记（如 title_menu_suspended: bool）：
1. 在 game.rs:1760 stop_title_menu() 之前设置 self.title_menu_suspended = true（只在 title_menu_live() 为真时）。
2. frame() 的 VM step 之后（建议 map_requests 处理之后、playtime_frames 自增之前）加：
   if self.title_menu_suspended && !self.screen.has_file_containing("SystemFile/WindowBase") {
       self.title_menu_suspended = false;
       if !self.title_menu_live() { self.start_title_menu(); }
   }
3. 在 switch_map / start_map_event_by_name / 新游戏入口把 title_menu_suspended = false，避免从存档页开始游戏时错误拉回 title 菜单。
4. 不要在 stop_title_menu() 里无条件重建；只有“曾在标题上下文且 WindowBase 消失”才恢复，否则每次进游戏会多出 event43。

### 建议测试
- crates/wolf-game/tests/m12_ui_state.rs：启动 TitleMap，点击标题第二行进存档页，发送 ESC，断言 title_menu_live()==true 且 slot 3/4/5 存在 Picture/menu*_b.png。
- 同一测试断言方向键/鼠标 hover 仍能操作菜单（choice_cursor 变化，highlight 文件 _a/_b 切换）。
- 回归：从标题点“开始”进游戏后，游戏画面不能出现 Picture/menu1_b.png。

## 4. UI-04 游戏内弹窗选项不能 hover 预选

### 状态
已确认（代码 + 两张逐字节相同截图）。点击行选择能工作；缺的是鼠标移动 -> cursor 预选 -> marker 跟随。

### 证据链
1. game.rs:1663-1739 handle_choice_input：
   - let (_, anchor_y) = self.choice_anchor();
   - let parked = anchor_y >= crate::picture::SCREEN_H as i32;（SCREEN_H=576，game.rs:1675-1676）
   - hover / Sys20 / cursor=row 逻辑全部在 if parked { ... } 内（game.rs:1678-1694）。
   - 非 parked 只走 clicked 的 choice_row_at（game.rs:1697-1710）和键盘 UP/DOWN（:1713-1719），没有按鼠标位置更新 choice_cursor。
2. choice.rs:244-249 marker ▶ 的显示条件是 index == cursor；cursor 是 Game.choice_cursor（frame host 传 game.rs:1781），所以 hover 不更新 cursor 时 marker 永远 row0。
3. 截图（同一帧、只改指针 y）：
   target/debug/wolf-cli.exe --root release/data capture --menu-only --frames 300 --key-frame 100:10 --pointer 600,270 --out work/out/reviews/probe_choice_hover_first.png
   target/debug/wolf-cli.exe --root release/data capture --menu-only --frames 300 --key-frame 100:10 --pointer 600,300 --out work/out/reviews/probe_choice_hover_second.png
   两张都是 1024x576，sha256 都是 02b012e373795eca67d783d3497df6f8f434d1e8368eca995a0e03f2c29718e8（byte-identical）。选项面板位于 (512,240)，4 项：广阔的战场/广袤的森林/微风吹拂而过的麦田/有人在等我的家；marker 始终 row0。
4. 点击对照：work/out/reviews/chk_click_row0_320.png 与 chk_click_row1_320.png 的 sha256 都是 5d3acac1ade769217bf6a5df772e289ccbb2a35f2e654b07bf3cb92108566f95；两者下一帧相同（都推进了 prompt，行区分不作为本项证据，但可证明点击路径不是完全坏）。

### 最小修法
在 handle_choice_input 的 if parked 之前给非 parked 弹窗加鼠标位置命中：
  if !parked {
      if let Some(row) = self.choice_row_at(self.mouse[0], self.mouse[1]) {
          cursor = row;
          self.choice_cursor = row;
      }
  }
- 保持 parked 分支的 picture_hover(3+row)/Sys20 逻辑不变（标题三行是 picture 槽画的，不是 choice 面板本身）。
- hover 只改 cursor，不应直接 reply；确认仍只由左键/OK 触发。
- choice.rs 已按 cursor 画 marker，无需改布局；只要保证 choice_cursor 先更新。

### 建议测试
- crates/wolf-game/tests/m12_ui_choice_hover.rs：构造非 parked Choices（anchor 512,240），pointer 在 row1 的 line 中心，frame 后断言 choice_cursor==1；再移到 row0 断言回到 0。
- 渲染级断言：用 choice::draw 或 capture 比较 pointer row0/row1 的 marker 位置（本报告已给 byte-identical 反例）。
- 点击边界：pointer 在选项外不应更新 cursor，也不应误确认（见 NEW-02 待定）。

## 5. 1024x576 与鼠标坐标/Dpi 核查

结论：未发现 DPI 坐标偏移；坐标链是物理像素一致映射。
- crates/wolf-cli/src/play.rs:420：窗口创建 PhysicalSize::new(LOGICAL_W, LOGICAL_H)=1024x576 物理像素。
- crates/wolf-render/src/scaler.rs:42-56：scale=max(1,min(win_w/1024,win_h/576))；offset 居中，窗口小于逻辑尺寸时 offset 可为负（裁剪而非拉伸）。
- play.rs:502-509 CursorMoved -> game_point（play.rs:632-640）：[(x-offset_x)/scale,(y-offset_y)/scale]，floor 到整数。
- scaler.rs:95-107 单测覆盖 1024x576->1 offset0；1920x1080->1 [448,252]；2560x1440->2 [256,144]；1000x700->1 [-12,62]；3840x2160->3 [384,216]。
- 实测 --menu-only --frames 1100 --pointer 200,460 --press-frame 300 能命中标题第二行进存档页（probe_menu_click.png），1024x576 无 gross offset。
- 高 DPI：窗口按物理 1024x576 创建，Windows 150% 下视觉尺寸约 683x384（显小），但鼠标坐标仍是同窗口 1:1 物理映射；这不是“点击偏了”。若产品要求高 DPI 保持视觉大小，另开 winit ScaleFactor/逻辑尺寸任务。

## 6. 新发现 bug（尚未登记）

### NEW-01（P2，状态机/脚本终止）174 ReturnToTitle 与 175 EndGame 是空操作
- game.rs:2683-2689：
  fn return_to_title(&mut self) { self.note("174 ReturnToTitle: the title closure does not use it yet".to_owned()); }
  fn end_game(&mut self) { self.note("175 EndGame: ignored (a port never closes the window from script)".to_owned()); }
- docs/research/M12_RETAIL_SCOPE.md:19 记录 174 有 13 个零售调用点、175 有 2 个；如果某菜单/结局走 174，游戏不会回标题，只留下诊断。当前 UI-02/03 的标题路径走 Teleport/CE14，不经过 174，本次未截图复现。
- 最小建议：至少把 174 实现为停止当前 VM/地图运行时、清屏、start_title() 或等价回标题；175 向 Host 暴露“请求退出”的结果而不是静默忽略。测试：合成 opcode 174 后断言回到 TitleMap/标题菜单；175 headless 返回明确 quit 标志。
- 备注：这是未在 UI_BUG_BACKLOG 登记的功能缺口；代码注释自己承认未实现，请 Lead 决定是否纳入本轮 UI 修复或另立任务。

### NEW-02（P2，待定/未能复现）非 parked 弹窗的“选项外点击”可能确认当前 cursor
- 代码链：play.rs:236 对任意 fresh left click 注入 key::OK；handle_choice_input 在非 parked 弹窗里 clicked 且 choice_row_at 返回 None 时不 return（game.rs:1697-1710 的 if parked 才 return），后面 pressed=key_bits & !previous_keys 会包含注入的 OK，最终 game.rs:1732-1734 回复 Index(cursor)。
- 理论结果：点空白处会确认当前高亮项，而不是什么都不做。
- 复现尝试：--menu-only --frames 320 --key-frame 100:10 --pointer 10,10 --press-frame 300 与不点击版本均输出 4315 instr/213 pic/2 choice，截图像素一致；--frames 450 --press-frame 400 也仍为 2 choice。说明该 capture 时序下点击没有落在 choice 打开后的帧，未能复现；列为待定，需要合成 Game 测试或手动在弹窗打开后点空白处验证。
- 若确认，最小修法：if clicked { if let Some(row)=choice_row_at(...) { reply; return; } if parked { ...; return; } return; }，非 parked 空白点击直接吞掉，不能落到 OK 分支。
- 建议测试：构造非 parked 4 项 Choices，pointer 在面板外，注入一次 left click，断言 replies 为空且 choice_cursor 不变。

## 7. 已知缺口（不是新发现，但会影响 UI-01/02/03 的验收）

1. 读档不恢复地图/运行时（M10 已登记）：game.rs:1598-1604 load_save 只调用 savegame::load_into；savegame.rs:71-83 parse/apply/base_save，apply()（savegame.rs:86-186）只恢复 Part3 普通变量、common self、数据库、Sys3/Sys4、playtime、Part5 画面，不停止当前 VM/地图运行时、不切换地图。docs/research/M10_SAVE_COMPLETENESS.md:98-100 明确写 Part1 地图快照存在但 Game 不建模，load 后 tile map/leader/events 为空；测试 the_map_tile_layer_and_leader_are_not_part_of_this_save_path 以此断言。实测 --load-save work/all/Save/SaveData01.sav --frames 300 只得到 0 instruction/0 choice 的 Part5 画面（work/out/reviews/probe_load_save_boot.png，92.3% non-black）。这不是 UI-02/03 的根因，但修复时不要误以为 load 会自动重开地图。
2. engine_state(10) 滚轮增量恒 0（M8 已登记）：game.rs:2819-2822；docs/research/M8_ENGINE_STATE.md:262 写明 no wheel channel exists。只修 CE131 的 123 路径时，CE41/CE224 等 124 field10 滚轮仍为 0；建议与 UI-01 一起加 wheel_dir host 状态。
3. 存档页元数据文本原样显示控制码/列重叠：work/out/reviews/probe_ce131_slot1_click.png 里 slot 文本仍含 <R>、\space[0]\f[0]\-[1]\E\A-\f[18] 等标记，8 个 metadata 列在 640px 内互相压住；属 UI-08/UI-13 视觉族，不是输入 bug，本报告不重复计为新 bug。

## 8. 复现命令/证据索引

| 用途 | 命令/文件 |
|---|---|
| UI-03 稳定复现（Lead） | TEMP=$PWD/work/tmp TMP=$PWD/work/tmp target/debug/wolf-cli.exe --root release/data capture --boot --frames 1900 --key-frame 850:10 --key-frame 950:2 --key-frame 1050:10 --key-frame 1350:11 --out work/out/esc_after_save4.png |
| UI-03 review 侧 | target/debug/wolf-cli.exe --root release/data capture --menu-only --frames 1100 --pointer 200,460 --press-frame 300 --key-frame 600:11 --key-frame 1000:11 --out work/out/reviews/probe_after_esc_ui03.png |
| UI-04 hover 对照 | 见 §4 的 --pointer 600,270/600,300；sha256 02b012e3... |
| UI-04 点击对照 | work/out/reviews/chk_click_row0_320.png / chk_click_row1_320.png；sha256 5d3acac1... |
| CE131 关键 trace | work/out/ce131.txt:60-66（轮询/±1/标签）、:202-204（cancel）、:270-278（load 220）、:335-338（关闭画面） |
| 存档页静态参照 | work/out/ui_bugs/01_load_screen_1023x576.png、work/out/reviews/probe_menu_click.png |
| 读档仅画面 | target/debug/wolf-cli.exe --root work/all capture --load-save work/all/Save/SaveData01.sav --frames 300 --out work/out/reviews/probe_load_save_boot.png |
| 事件 42 分支 | work/wolftl2/dump/common/42_オリジナルタイトル画面Ａ.json:913-1016 |
| 事件 43 三行图 | work/wolftl2/dump/common/43_┗オリジナルタイトル画面Ｂ.json:34,55,76 |

## 9. 剩余验证

1. 真实窗口 UI-01：需要给 play.rs 加可注入 MouseWheel 测试（或手动滚轮）验证 CE131 页码变化；CLI 当前无滚轮注入。
2. 真实窗口 UI-02：需要右键 Pressed/Released 注入（m12_ui_input.rs）验证 CE131 离开页面、返回标题；CLI 当前无右键注入。
3. UI-03 修复后回归：需要观察三行菜单的 hover/点击/键盘是否都可操作，而不只是“图出现”。
4. NEW-02 空白点击：需要合成 Game 状态或手动在 choice 打开后点面板外；当前 capture 时序没有复现。
5. 高 DPI：100%/125%/150% Windows 缩放下手动比对 pointer 命中；本报告 scaler 推理未覆盖 OS 强制 DPI virtualization。

## 10. 一句话给 Lead
UI-01/02 的修复放在 play.rs 输入层：滚轮保留方向键位供普通菜单，同时额外产生 CE131 所需的有符号 1/-1；右键按住注入 key::CANCEL。UI-03/04 放在 game.rs：前者加“标题菜单挂起后恢复”状态，后者把非 parked 弹窗的 choice_row_at hover 移到 parked 判断外。不要把 M10 的“load 不恢复地图”当成 UI-02/03 的同一根因。
