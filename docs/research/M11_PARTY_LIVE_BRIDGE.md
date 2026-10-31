# M11 P0 — real two-member party fixture -> live menu -> CE242 bridge

Status: test-only proof, no production changes.
Repo: E:/Study/github program/imoto life
Test file: crates/wolf-game/tests/m11_party_live_bridge.rs
Data root used: work/all plus the bundled retail save work/retail_backup/Save/SaveData01.sav. The retail save on disk is never edited; only in-memory CData cells are patched.

## 1. Reproduction

    WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m11_party_live_bridge -- --nocapture

Result:

    running 1 test
    test result: ok. 1 passed; 0 failed; 0 ignored
    finished in 0.72s

Captured output:

    m11_party_live_bridge: registered_rows=15 real_rows=[MenuRow { row: 1, activator: 0, code: 26 }, MenuRow { row: 2, activator: 1, code: 25 }] crawl_ce190=1 ce191=1 ce192=17 ce157=17
      round=1 arg0=1 first_pc=338 pre=[5, 4, 3, 2] post=[4, 4, 3, 2]
      round=2 arg0=2 first_pc=645 pre=[4, 4, 3, 2] post=[3, 4, 3, 2]
      round=3 arg0=3 first_pc=792 pre=[3, 4, 3, 2] post=[3, 3, 3, 2]
      round=4 arg0=4 first_pc=996 pre=[3, 3, 3, 2] post=[3, 2, 3, 2]
      bridge settle=(1, 1) started_rounds=4 turn_starts=3 warnings=0

## 2. Two-member party fixture (all in-memory)

The fixture is built by build_fixture() at crates/wolf-game/tests/m11_party_live_bridge.rs:85. It first calls the production seed_battle_databases() (the 26-cell battle seed table from task-19), then patches only these cells:

    CData type 18, row 4, col 0 = 2                 party count = two members
    CData type 6,  row 0, col 2 = 13                member 2 = status row 13
    CData type 0,  row 13, cols 20/21 = 100/100     member 2 maxHP / HP
    CData type 0,  row 13, cols 22/23 = 50/50       attack / defence
    CData type 0,  row 13, cols 24..28 = 20/15/10/30/30   remaining stats
    CData type 0,  row 13, cols 9/10/4 = 20/10/10   remaining stats
    CData type 0,  row 13, cols 47..54 = [4,3,6,7,0,0,0,0]  member 2 command slots

Leader row 12 and its command slots come from seed_battle_databases(); the fixture only adds the second 主人公ステータス row and registers it in パーティー情報. The status rows are indexed by member id (type 6 row 0 col 1 = leader row 12, col 2 = member row 13), which is why CE143 can materialise two allies.

After the fixture and before the fight the test asserts:

    type18 row4 col0       = 2       (party count)
    type6  row0 col1       = 12      (member 1 remains leader row)
    type6  row0 col2       = 13      (member 2 registered)

## 3. Part A — live CE188 -> CE190 -> CE191 -> CE192 -> CE157

The test starts CE188 with real OK input (OK on every even frame, no key on odd frames), exactly as the task-89 cursor test does. The real cursor cell is physical type 18 row 73 col 0; physical type 20 row 73 is the wrong cell and stays 0.

Observed:

    CE190 starts: 1
    CE191 starts: 1
    CE192 starts: 17
    CE157 starts: 17
    registered command rows: 15 (rows 1..15)
    command-list row 0: empty (-1), so real rows start at row 1
    command-input cursor: 16 = 1 + 15 registrations
    registered row layout: row r -> col0 (activator/slot) = r-1, col3 materialised (never -1)

The two successive real confirms:

    row 1: activator 0, action code 26 (蓄力)
    row 2: activator 1, action code 25 (防御)

These are two DISTINCT rows with two DISTINCT real action codes, not the -1 empty-slot sentinel. All later registered rows (3..15) materialise col3 = 0; they are still real menu rows but do not carry one of the four real attack codes. The task-89 single-member fixture produced only row 1 real because it had one member; adding the second status row gives row 2 the real 防御 code.

The extra rows beyond row 2 are produced by the shipped CE188 stub outer loop continuing to cycle after the first two confirms; the test stops once the registration set has been stable for 80 frames. The bridge therefore deliberately uses the first four registered rows as the menu queue; the first two of those are the real-action rows above.

CE143 placement is also observed directly in the database: physical type 10 rows 0 and 1 both have maxHP 100 and HP 100, proving two allies were laid down.

Warnings after the live menu: 0.

## 4. Part B — harvested rows drive real CE253 -> CE239 BOSS

Queue construction (test line 450): take the first four registered rows and map row -> command index 1..8 (row.clamp(1,8)). Result:

    queue = [1, 2, 3, 4]

Rows 1 and 2 are the two real-action rows; rows 3 and 4 are the next genuine registrations. The queue is installed with Game::set_battle_menu_command_queue (crates/wolf-game/src/game.rs:658), which delegates to BattleWiring::set_menu_command_queue (crates/wolf-game/src/battle_wiring.rs:335); next_command drains the queue front-first before falling back to the rotating schedule.

The BOSS context is real: start_engine_events(), set_battle_wiring(true), set_battle_round_loop(true), boss gauge cells 2000445=1 and 2000446=4000, then start_event(253). CE253 reaches CE239 and CE244 settles.

Observed bridge trace (one CE242 body per command round):

    round  arg0  first_pc  pre=pool[345,346,347,348]  post=pool[...]
    1      1     338       [5,4,3,2]                 [4,4,3,2]
    2      2     645       [4,4,3,2]                 [3,4,3,2]
    3      3     792       [3,4,3,2]                 [3,3,3,2]
    4      4     996       [3,3,3,2]                 [3,2,3,2]

    settle: v352 = 1, v343 = 1
    started_rounds = 4, turn_starts = 3, warnings = 0

Interpretation:

- first_pc is the first CE242 program counter above the shared prologue (pc > 143). The four values 338 / 645 / 792 / 996 are four distinct command bodies, i.e. the queue entries 1..4 each select a different CE242 body.
- arg0 is the CE242 self1600000 argument; [1,2,3,4] proves the harvested queue is consumed front-first.
- Pools 2000345 and 2000346 both drain: start [5,4,3,2] -> end [3,2,3,2], two distinct pools drained.
- Settlement writes v352 = 1 (win) and v343 = 1 (decided), and the battle warning count stays 0. The settlement values are captured on the exact frame v343 becomes 1, before any post-battle drain can reset them (the drain would otherwise print 0/0).

## 5. Line references

    crates/wolf-game/tests/m11_party_live_bridge.rs
      85-126   build_fixture(): party count, member-2 registration, status row, command slots
      128-136  open_fixture_game()
      139-156  menu_rows(): physical type 14 rows with col0 >= 0
      178-252  drive_bridge(): queue install, four-round trace, pre-drain settlement capture
      254-255  test entry point
      296      Part A live-menu frame loop
      338      assertion: at least two registered rows
      450      queue = first four harvested rows
      459      drive_bridge(&queue)
      474-476  distinct CE242 bodies assertion
      503      settlement assertion v352=1, v343=1
      506-524  captured diagnostic output

## 6. Caveats / residual

- Test-only. No production file is changed; the fixture lives entirely in the test and only writes in-memory database cells.
- The 15 registered rows are real menu output of the two-member fixture; only rows 1 and 2 carry real attack codes (26 and 25). The bridge consumes harvested row indexes, never the action code, so no code-to-index map is invented.
- The bridge's queue is a test-level injection through the existing Game::set_battle_menu_command_queue API because the shipped EVENT chain still does not call CE242 from CE239 on its own. That engine wiring gap is documented in docs/research/M11_BATTLE_MENU_BRIDGE.md and M9_BATTLE_COMMAND_INPUT.md; this test proves the consumer half works with real harvested indexes and real CE253 -> CE239 -> CE244 settlement.
- Full workspace verification on the final tree: WOLF_DATA_ROOT=work/all cargo test -q --workspace --no-fail-fast completed with 125 test-result targets, 782 passed, 0 failed, 1 ignored (including this new m11_party_live_bridge target: 1 passed). Full log: work/out/m11_party_live_bridge_workspace.log. The only stderr was a pre-existing unused-variable warning in crates/wolf-format/tests/_scratch_cdata.rs:34 (_scratch file, not part of this task and never committed).
