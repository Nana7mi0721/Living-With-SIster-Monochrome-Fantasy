# M11 P0 — CE157 command-registration cursor: root cause

**Task (m03389):** "Make the live battle command menu register more than one command
row... Deliver a minimal faithful fix (data/VM layer) that lets successive live menu
confirms write successive command rows."

**Verdict: the premise is false — there is no cursor bug, and no fix is warranted.**
CE157 (\`X[戦]コマンド登録\`) already advances the コマンド入力位置 cursor once per
successful confirm and writes a *distinct* row into the command list each time. Driving
the real battle entry **CE188** with real key input on the bundled retail save registers
**seven** distinct rows (row 1..7) and the cursor runs **1 → 8** (= 1 + registrations).
The "only row 1 / cursor stays 0" folklore came from two measurement mistakes in the
earlier probes, not from the engine.

Files touched by this task: \`crates/wolf-game/tests/m11_ce157_cursor.rs\` (new tests),
this document. **\`crates/wolf-vm/src/exec.rs\` was instrumented during investigation and
fully reverted** (\`git diff crates/wolf-vm/src/exec.rs\` is empty). No production source
was changed. No commit / push.

---

## 1. VM semantics used by CE157 (crates/wolf-vm/src/exec.rs, HEAD)

* **c121 assign/compound** (\`exec.rs:1676\`): \`target=arg0; a=eval(arg1); b=eval(arg2);
  bits=arg3; bin=(bits>>12)&0xF; result=binary_op(bin,a,b); current=eval(target);
  value=compound(bits&0x0FFF, current, result); store(target,value)\`.
  * \`compound\` (\`exec.rs:1440\`): \`0x100=cur+val, 0x200=cur-val, 0x300=cur*val,
    ..., 0x600=cur, 0x700=cur, 0x800=cur.abs()\` — the 0x6/0x7/0x8 kinds are **unary
    transforms of the current target value**, not of the binary result.
* **c111 compare** (\`exec.rs:1493\`): \`0:>, 1:>=, 2:==, 3:<=, 4:<, 5:!=, 6:(a&b)!=0\`;
  \`head>16\` means the low nibble is the condition **count** and bit 4 is the else flag.
* **c250 database cell** (\`exec.rs:2021\`): \`type_label=label(cmd,1)\`,
  \`field_label=label(cmd,3)\`; the physical \`(db_type,row,col)\` is resolved *by label*
  through \`databases.resolve_labels\` (game.rs:1633 / :1944). bit12 of \`flags\` = read vs
  write; \`op=(flags>>4)&7\` = assign/add/sub/mul/div/rem/max/min.
  * So a logical table name like \`基本システム用変数\` (logical 20) and
    \`コマンド処理リスト\` (logical 15) map to **physical** CData ids 18 and 14.

## 2. CE157 pc-by-pc (ALLY mode, the registration path that matters)

Mode dispatch \`self1600002\`: \`-2/-1\` = "back/clear-all" (pc8 → case pc10-28, writes
\`input_pos\` and clears), \`0\` = ally selection (pc35 → pc36-131), \`1\` = AI.

Registration (ally, pc36-131):

| pc | effect |
|----|--------|
| 38 | \`input_pos = max(cur,1)\` (c250 op6 max) — never lowers an already-advanced cursor |
| 39-40 | reset scratch \`self1600020=0\`, \`self1600019=-1\` (target) |
| 41 | branch on chosen skill code (\`self1600001\` ∈ {-2,-1,0} → item/skill submenu) |
| 80-84 | \`self1600017 = input_pos\` (c250 read) — **the row this confirm will write** |
| 85 | \`c121[1600025,1600019,0,2048]\`: bin0 → result=\`self1600019\`, kind **0x800** → \`self1600025=abs(current)=0\` (result is ignored by the unary transform) |
| 86 | \`if self1600025 >= 100\` → "immediate-execute" block pc87-105 (writes \`self1600023=100\`, \`self1600017=0\`, **does not** register a command row). **Not taken** for a normal ally confirm because pc85 always yields 0. |
| 107-120 | **registration**: write \`コマンド処理リスト\` row=\`self1600017\`, cols0..3 = \`self1600000\` (activator/slot) / \`self1600018\` / \`self1600019\` (target) / \`self1600001\` (action code) |
| 121 | \`if self1600023 == 0\` → pc123-126 |
| 124 | \`self1600017 += 1\` (c121 kind 0x100) |
| 125 | **write \`input_pos = self1600017\`** (c250 op0 assign) — the cursor advance |

So the cursor advance **does** run. The AI branch (pc136-174) advances unconditionally
(pc168-169). The task's claim that "pc93/pc100 set \`self1600023=100\` because
\`self1600019>=100\`, so pc124-125 never run" is contradicted directly by this path: pc85
the 0x800 unary transform forces \`self1600025=0\`, pc86 is not taken, pc124-125 execute.

## 3. Empirical proof — real CE188 live menu, retail save

Harness: \`Game::new; load_save(work/retail_backup/Save/SaveData01.sav); seed_battle_databases();
start_event(188); key_bits=OK on even frames\`. CE157 is driven through the genuine
\`CE188 → CE190(place) → CE191(行動内容のセット) → CE192(味方コマンド選択) → CE157\` chain.
Every time an input-position/row transition happens the probe prints it:

\`\`\`
f=46  inpos 0->1  ce157=0 ce192=0          (CE157 clear-all sets cursor to 1)
f=94  inpos 1->2  ce157=1 ce192=1   row1 col0 -1->0 col3 -1->26
f=132 inpos 2->3  ce157=2 ce192=2   row2 col0 -1->1 col3 -1->0
f=170 inpos 3->4  ce157=3 ce192=3   row3 col0 -1->2 col3 -1->0
f=208 inpos 4->5  ce157=4 ce192=4   row4 col0 -1->3 col3 -1->0
f=246 inpos 5->6  ce157=5 ce192=5   row5 col0 -1->4 col3 -1->0
f=284 inpos 6->7  ce157=6 ce192=6   row6 col0 -1->5 col3 -1->0
f=322 inpos 7->8  ce157=7 ce192=7   row7 col0 -1->6 col3 -1->0
\`\`\`

* Seven successive confirms wrote **seven distinct rows** (row 1..7, activator col0 = 0..6).
* The コマンド入力位置 advanced **exactly once per registration**: 1 → 8 = 1 + 7.
* After the seven slots are filled the CE188 stub's outer loop keeps cycling (inpos 8→9→…)
  with **no new rows** — that is the legacy \`X◆戦闘処理\` stub re-running, not a cursor
  fault. (CE188 is the old engine; the shipped path is CE239/CE242, see §5.)

Only **row 1** carries a real action code (\`26\` 蓄力, auto-selected). Rows 2..7 carry
\`col3=0\` because the seeded party has a **single** real member — the remaining ally slots
are auto / "no command" (CE192 still registers them, the cursor still advances). To get more
than one **real** action code you need a multi-member party fixture (§5), not a VM change.

## 4. The wrong-cell trap (why earlier probes saw "row 1 only, inpos 0")

\`input_pos\` was read from **physical CData type 20 row 73 col 0**, which is always 0.
The real \`基本システム用変数\` コマンド入力位置 cell is **physical CData type 18 row 73
col 0**. A full scan after a run shows \`type18 r73c0 = 2\` (later 8) while
\`type20 r73c0 = 0\`. The write at CE157 pc125 literally lands in physical type 18.

The other half of the folklore is the loop condition: the earlier multi-input probes broke
out of the frame loop **the instant the first row appeared**, so they could never observe
the second confirm. A probe that stops at the first registration will always report "one row".

## 5. Remaining, *genuine* gaps (not the cursor)

1. **More than one real action code.** The CE191 loop iterates the ally slots and the
   cursor/rows are correct, but only party slot 0 has a real command set because
   \`seed_battle_databases\` seeds a 1-member party (\`CData type18 row4 col0 = 1\`).
   Supplying a 2-member party is a *data-layer* exercise:
   * \`CData type18 row4 col0 = 2\` (party count read by CE190 pc12),
   * a second \`パーティー情報\` メンバーN entry (read by CE105 \`選択位置主人公ID取得\`),
   * a matching \`主人公ステータス\` (type 0) row with stat + command columns 47..54,
   * CE190 then places it via CE143 and CE192 offers it a real menu.
   This is fixture work, not an engine/VM defect.
2. **Live command **code** → CE242 index mapping** is the separate M9 dead-end already
   documented (tasks 63/67): the code domain {25,26,29,30,0} is disjoint from the CE242
   command index 1..8; the bridge keys off the registration **row** (1..8), which CE157
   produces correctly, so structurally the bridge has a real producer once a party of ≥2
   real members is seeded.

## 6. No fix applied (and why that is faithful)

The instruction authorised a fix *only if a real engine bug is proven*. None is: the
cursor advances, rows are distinct, physical cell is type 18. Adding a VM patch or reading a
different cell would be speculative hardcoding against a mis-measured symptom. The
deliverable is therefore the corrected root cause plus executable proof.

## 7. Tests & commands

* \`crates/wolf-game/tests/m11_ce157_cursor.rs\` — 2 tests, green:
  1. \`live_menu_registers_multiple_rows_and_advances_the_cursor\`: drives real CE188,
     asserts ≥3 distinct registration rows, activator col0==row-1, non-sentinel col3,
     \`input_pos == 1 + registrations\`, ≥ \`n\` CE192/CE157 starts, and that physical
     type 20 row 73 stays 0 (pins the wrong-cell trap).
  2. \`input_position_is_physical_type_18_row_73\`: minimal witness that type 18 row 73
     advances while type 20 row 73 stays 0.
* Run: \`WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_ce157_cursor\`
  → \`2 passed; 0 failed\`.
