# M7 battle — production enemy-count seed and CE162 target selection

HEAD 1d36ebe. Scope: make the proven enemy-count cell part of the production battle
seed and pin the CE190 -> CE162 target-selection path with a dedicated test.
No engine or event code is changed; the only runtime change is one new seed cell.

## 1. Production seed change

crates/wolf-game/src/battle_seed.rs now exports:

    pub const ENEMY_COUNT: Cell = Cell { db_type: 18, row: 69, column: 0, value: 1 };

and seed_cells() pushes PARTY_COUNT then ENEMY_COUNT before the existing command
slots / stats / item / weapons. seed() therefore writes **26** numeric cells
(previously 25): 1 party + 1 enemy + 8 command slots + 12 leader stats + 1 item +
3 weapons. The eight command-menu text labels are still written by
seed_menu_text() and still do not count toward the numeric return value.

Why this cell: CData type18 row69 col0 is the enemy count read by
CE190 pc36 (250 [20,69,0,69632,1600020]) and by CE162 pc12
(250 [20,69,0,69632,1600011]). A fresh game leaves it 0, so CE190's enemy loop runs
zero times and CE162 has no target to place.

## 2. Runtime evidence (WOLF_DATA_ROOT=work/all)

Seeded run — Game::new + seed_battle_databases() + start_event(190), 600 frames with
OK on even frames:

- type18 row4 col0 = 1, type18 row69 col0 = 1 (survives CE190)
- CE144 copied a live enemy HP row: type10 row0 col2 > 0
- CE175 enemy coordinates: type18 row35 col0 = 512, row36 col0 = -2880,
  row37 col0 = 5120, row38 col0 = 3120
- warnings = 0

Then start_event(162) from that state, 300 frames: CE162 walks pc12 -> pc34 ->
pc49 -> pc50 -> pc55 -> pc57, exits, enemy index 0 selected. Its pc set includes 12
(enemy count read) and 57 (selection), and no CE162 instance remains afterwards.

Control — same production seed with only type18 row69 col0 cleared to 0:

- CE190 400 frames: row69 stays 0; type10 row0 col2 still > 0 (CE144 still copies
  one enemy row), but CE190's enemy loop is empty.
- start_event(162) 240 frames: CE162 never reaches pc57, remains active, final pc is
  49 or 50. This is the exact spin the task-31 investigation reported.

## 3. Exact event evidence

- CE190 戦闘キャラ配置 pc36: 250 [20,69,0,69632,1600020] reads the enemy count;
  pc37 reads row72 (enemy group); pc68 calls CE144 敵DB→戦闘DBコピー; pc70 calls
  CE175 敵キャラ_座標算出.
- CE162 敵対象選択実行 pc12: 250 [20,69,0,69632,1600011] loads the enemy count;
  pc15..21 builds the enemy order self1600040..46 = [5,3,1,0,2,4,6]; pc48/49 draws
  with CE179; pc50 waits on key 177; pc57 stores the selected enemy index
  (self1600010 = self1600012) and pc58 exits.
- CE175 writes type18 rows35..38 (draw coordinates/scales) and row51 from
  rows97/98 screen scale; these are the coordinate values asserted above.

## 4. What remains

The seed fixes target *generation*, not target *entry*. In a CE188-line battle the
command confirmation path runs CE157 コマンド登録 and CE192 味方コマンド選択 but never
spawns CE159 技能選択実行, whose pc247 is the only real caller of CE162. That is why a
selected real attack still returns to the command list and why direct start_event(162)
is the proof vehicle here. Remaining work is the execution-side dispatch from the
command window into CE159/CE162 (or an equivalent hook) so the seeded target list is
reachable through normal play.

Separately, turn settlement is still blocked at CE244 pc36 (2000349 == self1600010),
documented in docs/research/M7_BATTLE_TARGET_SCHEDULER.md §2.4; that is unrelated to
this seed cell.

## 5. Tests updated / added

- crates/wolf-game/src/battle_seed.rs — ENEMY_COUNT cell + updated seed count/doc.
- crates/wolf-game/tests/m7_battle_seed.rs — invariant now asserts written == 26 and
  type18 row69 col0 == 1.
- crates/wolf-game/tests/m7_battle_target.rs — 3 new tests:
  production_seed_carries_the_enemy_count_ce162_reads,
  seeded_enemy_count_lets_ce162_select_enemy_index_zero,
  without_enemy_count_ce162_spins_at_pc49_pc50.
- crates/wolf-game/tests/m7_battle_target_scheduler.rs — old "shipped seed omits
  row69" assertions updated (control = production seed then clear row69=0). Authorized
  by Lead (option A) because the production seed grew from 25 to 26 cells.

Cross-task note: crates/wolf-game/tests/m7_battle_menu_text.rs also hard-codes the old
25-cell seed return at its lines 70 and 143-145. That file is owned by task-39
(m6-multiday), which is updating the 25 -> 26 invariant; this task deliberately does
not edit it.

Verification (WOLF_DATA_ROOT=work/all):

    cargo test -q -p wolf-game --test m7_battle_seed   -> 1 passed
    cargo test -q -p wolf-game --test m7_battle_target -> 3 passed
    cargo test -q -p wolf-game --test m7_battle_target_scheduler -> 6 passed

No commit/push.