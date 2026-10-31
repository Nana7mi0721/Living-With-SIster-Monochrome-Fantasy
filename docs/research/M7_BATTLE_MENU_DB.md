# M7 battle command-menu database: the 万能ウィンドウ labels (W1-d)

Status: **DONE — data seed plus the engine string-write fix (task-39).** The command
window now receives the real per-slot labels because CE152's own opcode-250 write path
preserves a string operand. This note records the exact cells, the event flow, the
empirical before/after and the final recipe. Companion tests:
crates/wolf-game/tests/m7_battle_menu_text.rs (game level) and
crates/wolf-vm/tests/opcode_250_string.rs (interpreter level). Seed code:
crates/wolf-game/src/battle_seed.rs.

## 1. What the command window reads

The battle command line draws through the generic 万能ウィンドウ ("versatile window")
system:

| event | name | role |
|---|---|---|
| CE152 | X[戦]戦闘コマンド一覧算出 | builds the scratch table (which commands, in which order) |
| CE65  | X[共]万能ウィンドウ選択実行 | key handling / cursor; calls CE64 to draw |
| CE64  | X[共]万能ウィンドウ描画処理 | draws the window rows |

The **visible item name** is CData type 19 (万能ウィンドウ一時DB) column 0 項目文.
CE64 reads it at pc615 (250 [19,1600043,0,331776,1600007] 項目文) into self1600007 and
renders it. CE64 does **not** read column 5 (付与文字列[後], read at pc620/pc632) for the
main label; col4/col5 are only the prefix/suffix decorations. So localising the command
line means filling type19 column 0.

CE152 is what normally fills it (110 commands):

- pc6  250 [19,-2,0,65536,1600030] — write row-count sentinel, operand self1600030
  (initialised to −1 at pc10). A value of 0 means "table cleared before refilling"
  (db.rs set_row_count, ROW_COUNT_SET).
- pc10 self1600030 = −1; pc12 self1600030 += 1 (per-slot loop counter).
- pc13 300 [0,16777218,1600000,1600030,1600019] — CALL CE13 戦闘コマンド取得:
  slot → Data type 6 row id in self1600019.
- pc21 250 [6,1600019,0,332288,1600009] コマンド名 — read Data type 6 column 0
  コマンド名 into self1600009 (stored in the self variable string bank via store_str).
- pc22 250 [19,1600032,0,327680,1600009] 項目文 — write type19 column 0 = self1600009.
- pc23..pc34 — write type19 column 1 項目コード (from Data type 6 column 2) and
  column 2 選択可能.
- pc107 self1600032 += 1; loop pc11×8.

Data type 6 戦闘コマンド (name column 0) rows: r0 普通攻击(code 0), r1 道具(-2),
r2 特殊技能(-1), r3 蓄力(26), r4 防御(25), r5 隐藏(28), r6 强斩(30), r7 反击(29). The seed's
COMMAND_SLOTS pick rows 3/4/6/7 (the four real attacks) plus row 0 for the tail slots.

## 2. What the seed writes

battle_seed::seed (reached through Game::seed_battle_databases) writes the numeric
26-cell table from M7_BATTLE_SEED **and** the string half:

    CData type 19 (万能ウィンドウ一時DB), column 0 項目文
      row 0 = 蓄力   row 1 = 防御   row 2 = 强斩   row 3 = 反击
      row 4..7 = 普通攻击          (the eight command slots, same order as COMMAND_SLOTS)

The labels are kept in battle_seed::COMMAND_MENU_TEXT and battle_seed::seed_menu_text;
the strings are the same values CE152 reads out of Data type 6 column 0, so this is a
data-faithful pre-fill, not an overlay of a different table. seed returns 26 (the
numeric-cell invariant: 1 party + 1 enemy + 8 slots + 12 stats + 1 item + 3 weapons);
the eight text cells are additional.

## 3. The engine fix (landed in task-39)

Before the fix, CE152 itself could not write the names: on an argc>=5 opcode-250 write
the interpreter built DbValue::Num(eval_num(value operand)) unconditionally
(exec.rs:2043-2045). CE152 pc22's value operand self1600009 holds a **string** (it came
from the pc21 read of a string column, stored with store_str), and eval_num on a string
slot is 0 — so type19 column 0 received the number 0 and CE64 drew blank rows. A second
shadow made it worse: set_num stored the 0 into the numeric cell map and game.rs's read
path (1696-1701) returns engine_num before engine_text, masking any pre-seeded label.

The fix has two symmetric halves:

1. crates/wolf-vm/src/exec.rs, opcode 250 write branch (argc>=5): choose the value kind
   from the operand instead of always numeric.

        let value_raw = arg(cmd, 4);
        let value_magic = Magic::decode(value_raw);
        if value_magic.is_string()
            || (matches!(value_magic.kind(), MagicKind::SelfVar)
                && !eval_str(value_raw, inst, vars).is_empty())
        {
            (None, Some(DbValue::Str(eval_str(value_raw, inst, vars))))
        } else {
            (None, Some(DbValue::Num(eval_num(value_raw, inst, vars))))
        }

   A MagicKind::StrVar operand (3_000_000 range) is always text. A MagicKind::SelfVar
   operand is text only when its string bank was actually filled (eval_str non-empty);
   a number-only self slot stays numeric. Ordinary MagicKind::Var operands are always
   numeric — note eval_str on a Var returns the decimal form of its number, so testing
   "eval_str non-empty" on a Var would wrongly turn v1 = 5 into Str("5"); restricting
   the test to SelfVar is what keeps that regression out (see
   crates/wolf-vm/tests/opcode_250_string.rs and exec.rs
   db_write_never_touches_the_source_variable).

2. crates/wolf-game/src/db.rs: set_num refuses a numeric write aimed at a FieldKind::Str
   column (so a stray numeric can never shadow text), and set_text routes a text write
   aimed at a FieldKind::Num column back through set_num (parsing the decimal form), so
   the two setters cannot clobber each other whichever operand kind the interpreter
   sends.

## 4. Empirical result

Interpreter level (crates/wolf-vm/tests/opcode_250_string.rs):

    cargo test -q -p wolf-vm --test opcode_250_string
    4 passed  (self-var string bank -> Str; explicit StrVar -> Str;
               ordinary Var 5 -> Num(5); number-only self slot -> Num(7))

Game level (crates/wolf-game/tests/m7_battle_menu_text.rs):

    cargo test -q -p wolf-game --test m7_battle_menu_text -- --nocapture

Observed:

- Immediately after seed_battle_databases(), CData 19 row0..7 col0 reads back
  蓄力/防御/强斩/反击/普通攻击×4 (asserted).
- Data type 6 col0 rows 3/4/6/7 read back 蓄力/防御/强斩/反击 (asserted — source is real).
- Driving the real CE188 戦闘 flow (300 frames, alternating OK) reaches CE152, CE192 and
  the target window (CE157/158), with real attacks code 26/25/30/29 selectable,
  warnings = 0, 250 unanswered = 0, peak instances >= 3.
- **CE152 writes the labels itself.** At the first frame a real attack is selectable the
  type19 column 0 rows read back ["蓄力","防御","强斩","反击","","","",""] — the four
  per-slot real-attack names, produced by CE152's own pc22 write (asserted). This is the
  behaviour the old build could not produce (it wrote blanks).
- After the whole run type19 col0 reads ["普通攻击"×8] because a later CE152 rebuild under
  the evolved command state resets the unused rows; every row is non-empty (asserted).
  The menu-build frame is the one the player sees when choosing.

Control (no seed) still reaches CE192 but leaves real attacks unselectable, so the seed is
what flips usability, and the interpreter test shows the write path is what carries text.

## 5. Same fix covers the target list (G3)

CE190 敵データリスト複写 reads the enemy name from a string column into a self string var
and copies it with 250 writes, so target labels fell over at the same exec.rs line.
The corrected write path now carries those names too; there is no additional data to seed.

## 6. Files / invariants

- crates/wolf-game/src/battle_seed.rs (additive): TextCell, COMMAND_MENU_TEXT,
  seed_menu_text; seed calls it and still reports the numeric cells (26).
- crates/wolf-game/src/battle_seed.rs / game.rs / db.rs unchanged in this task beyond the
  seed (task-41 added the 26th numeric cell).
- crates/wolf-vm/src/exec.rs: opcode-250 write branch string-kind selection.
- crates/wolf-game/src/db.rs: set_num ignores Str columns; set_text routes Num columns.
- crates/wolf-vm/tests/opcode_250_string.rs (new).
- crates/wolf-game/tests/m7_battle_menu_text.rs (updated: 26-cell invariant, CE152 label
  proof).
- m6_battle_commands / m7_battle_seed / m7_battle_round remain green.
