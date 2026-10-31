# M9 Battle Command Input (task-50)

Status: test-only deliverable, no production edits, no commit. Files:
- crates/wolf-game/tests/m8_battle_command_input.rs (new)
- docs/research/M9_BATTLE_COMMAND_INPUT.md (new)

Run:

    cargo test -q -p wolf-game --test m8_battle_command_input -- --nocapture

Latest result: 2 passed, 0 failed. Measured output:

    menu_seen=true ce65=true ce192=true ce157=true codes=[26, 25, 30, 29] selected=25 registered_row=Some(1) ce242=false ce244=false v496_peak=0 v352=0 v343=0 v350=0 warnings=0
    ce239=true ce242=true ce244_seen=false menu_line: ce191=false ce192=false ce65=false v352_peak=1 v343_peak=1 v496_peak=0 frames=1388 warnings=0

## 1. What was proven

1. The real player-facing battle command line accepts input. Starting from CE188, the
   engine reaches CE191 -> CE192 -> CE152 -> CE65 -> CE157. Driving the CE65
   万能ウィンドウ with UP/DOWN and OK selects a NON-default row: 防御 (Data type6
   row4, command code 25). CE157 registers it in the physical CData table whose editor
   label is [×┣ コマンド処理リスト] at row 1, column 3 = 発動技能 = 25.
2. That registered command is NOT consumed by CE242/CE244. In the same CE188 run,
   CE242 and CE244 never start, and 2000496 stays 0.
3. The separate CE253 -> CE239 battle wrapper DOES settle when the rebuilt engine wiring
   starts CE242: 2000352 = 1 and 2000343 = 1 at frame 1388, warnings 0. But CE239 never
   opens the CE188 menu line: CE191/CE192/CE65 never start, and 2000496 stays 0.
4. Therefore the task premise that "2000496 changes to the chosen command id and CE242
   consumes it" is factually wrong for this data. 2000496 is a Yukari/skill subchoice
   code, written by CE239 pc581 (=4) and by CE242 pc166/pc498 (=-1), read only inside the
   CE242 ユカリ branches pc148/pc159/pc480/pc491. It is not the player command register.

## 2. Scenario A: real CE188 menu input

Harness: Game::new(data_root), load SaveData01.sav, auto_advance/auto_choice = true,
auto_choice_delay = 1, game.seed_battle_databases(), start_event(188), then steer CE65 by
keypad until a command row registers; 500-frame cap. Test function:
crates/wolf-game/tests/m8_battle_command_input.rs:56.

Menu snapshot measured through CData type19 (万能ウィンドウ一時DB; col1 = 項目コード,
col2 = 選択可能[1=可]):

- First window (CE192 pc189-190, the 戦闘/逃走 menu): row0 = 战斗 code 1 selectable 1;
  row1 = 逃跑 code -1 selectable 1.
- Command window (CE152 output): row0 蓄力 code 26; row1 防御 code 25; row2 强斩 code
  30; row3 反击 code 29; row4..7 普通攻击 code 0; all selectable 1 because
  seed_battle_databases supplies inventory item 0 = 1.

The test steers the cursor to row1 防御 and presses OK. CE157 コマンド登録 then runs. Its
normal (non-AI) registration block is:

- CommonEvent script CE157 pc117: opcode 250, type 15, row self1600017,
  col0, value self1600000 -> 発動者番号.
- CE157 pc118: type 15, col1, value self1600018 -> 発動敏捷性.
- CE157 pc119: type 15, col2, value self1600019 -> 受け側番号.
- CE157 pc120: type 15, col3, value self1600001 -> 発動技能 (the chosen command id).
- CE157 pc125: type 20 row 73 col0 <- self1600017 -> 基本システム用変数 [戦闘]コマンド入力位置.

The runtime type id 15 is not the physical .dat type. Databases::resolve_labels
(crates/wolf-game/src/db.rs:811) resolves it through the editor type label
[×┣ コマンド処理リスト] and returns physical type 14, whose schema is documented at
docs/BATTLE.md:471: col0 発動者番号, col1 発動敏捷性, col2 受け側番号, col3 発動技能.
The write path reaches this resolver through Game::resolve_cell
(crates/wolf-game/src/game.rs:1455, call at :1462) before every numeric 250 write.

Measured cell: before the run CData type14 row1 col3 is -1 (the save template); after
selecting 防御 it becomes 25. Test assertions:
crates/wolf-game/tests/m8_battle_command_input.rs:178 (row == Some(1)) and :183
(発動技能 == DEFENSE_CODE 25). The same test asserts CE65/CE192/CE157 all ran, all four
real-attack codes [26,25,30,29] were selectable, v496 remained 0, CE242/CE244 never
started, v352/v343/v350 all 0, and warnings 0.

## 3. Scenario B: CE253/CE239 settlement without the menu

Harness: same save, seed_battle_databases(), cosmetic CData (5,16,0) label, start_engine_events(),
set_battle_wiring(true), start_event(253); OK every other frame; stop at 2000343=1 and
2000350=0, then drain 800 frames. Test function:
crates/wolf-game/tests/m8_battle_command_input.rs:217.

Measured: CE239 starts; the rebuilt wiring starts CE242; 2000352 reaches 1; 2000343 reaches
1; warnings 0. The menu events do not run: ce191=false, ce192=false, ce65=false. 2000496
never leaves 0. CE244 is called from inside CE242 (event 244 is not retained as a live
instance in this trace), so the settlement evidence is the pair 2000352 in {1,-1} and
2000343 = 1, exactly as in docs/research/M7_BATTLE_PLAYABLE.md.

## 4. Exact missing gate and call graph

The shipped event graph has two independent battle lines:

- Player menu line: CE188 -> CE191 (行動内容のセット) -> CE192 (味方コマンド選択) ->
  CE152 -> CE65 -> CE157. This line writes the chosen command to CData type14 col3 and
  never touches 2000350/CE242/CE244.
- Battle wrapper line: CE253 -> CE239 (■戦闘呼び出し). CE239 uses only opcode 210/211
  calls: pc15 500232, pc218/pc223/pc229 500231, pc457/pc491 500229, pc703 500264,
  pc795 500244, pc1098 500232. It has no opcode 300 name call at all, so it never calls
  CE188/CE191/CE192/CE65 and never calls CE242/CE243.
- CE242 is a legacy call_only subroutine with zero callers: a full CommonEvent scan for
  literal 500242 finds zero call sites. CE242 itself calls 500243 at pc207/308/470/615/
  753/966/1130/1190/1399/1432. Its pc14 writes 2000350 = 1; every other 2000350 writer is
  CE242/CE243 (set 2) or CE239/CE241/CE244 (set 0), so 2000350 can only be set to 1 by
  CE242 itself. This is the self-boot paradox documented at docs/BATTLE.md:141 and
  docs/BATTLE.md:859-864.
- CE244 is reachable directly from CE239 pc795 (500244) and from CE241/CE242/CE243. It is
  the settlement writer: 2000352 = 1 win / -1 lose, 2000343 = 1.

Therefore, in the shipped data, there is no path from a player-selected command in
CE188/CE65 to the CE242 command phase or to CE244 settlement. The only way to make
CE244 settle in this build is the rebuilt engine wiring (crates/wolf-game/src/battle_wiring.rs),
which starts CE242 on the 2000340 0->1 edge and passes acting slot 1; that is engine
integration, not data menu input. No variables are faked in either test.

## 5. Consequences and residual risk

- The correct production statement is: real menu input works and registers the selected
  command in CData type14; real settlement works through CE253/CE239 plus the rebuilt
  wiring. The two are disjoint in the shipped scripts. Bridging them would require
  engine-side wiring that maps the CData type14 command row into CE242's 2000344/2000345
  context, or a data patch adding a call from CE239/the menu line into CE242; neither is
  done here because the task is test-only and forbids production edits.
- 2000496 must not be used as the selected-command variable in any future battle work.
- Pre-existing unrelated failure observed during this study: cargo test -q -p wolf-game
  --test m7_battle_menu_text panics at its line 183 assertion (CE192 control path); it is
  not touched by this task and is left unchanged.
