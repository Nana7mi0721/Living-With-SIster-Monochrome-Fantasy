# M8 Backlog — consolidated plan from all M7 research and tests

Status: planning note (read-only). HEAD `1d36ebe` plus the uncommitted M7 "first batch"
work in the worktree. Data root `work/all` (full 1.5 GB) and `work/full` (has
`Save/system.sav`). This document does **not** edit code or tests and is not committed;
the Lead integrates.

It supersedes the open-work parts of `docs/research/M7_BACKLOG.md` and
`docs/HANDOFF_M7.md` (both written before the M7 first batch landed; see §7
contradictions). Reference baseline `docs/BASELINE.json` (`verify.checks = 70`,
`m7_workstreams` still only lists the five pre-batch slices).

---

## 0. What "M7 first batch" already delivered (DONE — do not redo)

Tracked (committed) at/before `1d36ebe`:

| slice | artifact | evidence |
|---|---|---|
| PRNG | `crates/wolf-vm/src/rng.rs` retail MSVC LCG; `crates/wolf-vm/tests/rng_retail.rs` 11 tests | seed 1 -> 41,18467,6334,26500; `docs/research/M7_PRNG.md` |
| W1-a battle round | `crates/wolf-game/tests/m7_battle_round.rs` | CE188 -> CE143/144/192/157/158; HP copy 100/100; `docs/research/M7_BATTLE_ROUND.md` |
| W2-a H advance | `crates/wolf-game/tests/m7_hscene_advance.rs` | 14/14 opcode-123 polls key-driven; `docs/research/M7_HSCENE_ADVANCE.md` |
| W5 choice window | `crates/wolf-game/tests/m7_choice_window.rs` | `docs/research/M7_CHOICE_WINDOW.md` (H-MENU-1 resolved) |

Untracked in the worktree (delivered, awaiting Lead integration):

| slice | artifact | evidence / key result |
|---|---|---|
| W1 battle seed | `crates/wolf-game/src/battle_seed.rs`, `tests/m7_battle_seed.rs`, +17 lines each in `game.rs`/`db.rs` | `Game::seed_battle_databases()`; real attack selectable (control false -> seeded true), ce152/192/target_win true, 250 unanswered 0. `docs/research/M7_BATTLE_SEED.md` |
| W1 full settlement proof | `tests/m7_battle_full.rs` | SC1: `start_event(242)+vars.set(495,2)+self1600000=1` gives GENUINE CE244 settlement 2000350 [0,1,0,2,0], 2000352=1, 2000343=1, warns 0. `docs/research/M7_BATTLE_FULL.md` |
| W2-b H resume | `tests/m7_hscene_full.rs` (2 tests) | mid-H SaveData08 stack (~88-90 pics), late-night UI var62 walk 1->2->6->1->4->13 with one `var364=0` injection. `docs/research/M7_HSCENE_FULL.md` |
| W2 auto-events | `tests/m7_hauto_events.rs` (2 tests) | 7 run==2 H text events tracked; **hword layer always alpha 0**. `docs/research/M7_HAUTO_EVENTS.md` |
| W5 opcode 124 | `crates/wolf-vm/tests/opcode_124_long.rs` (7 tests) | interpreter complete, 1045 sites; `docs/research/M7_OPCODE_124.md` |
| W5 host state | `tests/m7_host_state.rs` (3 tests) | picture field5=0, engine 11/12=-1, 17=0; character_state=None. `docs/research/M7_HOST_STATE.md` |
| W3 day5 schedule | `tests/m7_day5_schedule.rs` | day1->day5 08:30 with 4-var CE244 win-branch injection; var160=5 var6=4 var4=8 var5=30 warns 0, 7640 frames/392530 instr. `docs/research/M7_DAY5_SCHEDULE.md` |
| W4 map edge | `tests/m7_map_edge.rs` (3 tests) + tilemap.rs doc comment | `130` -2 = target, dest map 0; Map024/031 tile-less → black; MapTree not entry. `docs/research/M7_MAP_EDGE.md` |
| W4 new-game entry | `tests/m7_new_game_entry.rs` (2 tests) | TitleMap ev2 -> CE42 pc118 -> Map003 ev1 pc28 -> SampleMapA (62,38); 92.0% capture. `docs/research/M7_NEW_GAME_ENTRY.md` |
| W4 save parts | `tests/m7_save_parts.rs` | 5-part roundtrip; fixes load→save Part4 template regression in `savegame.rs` (no research doc) |
| W5 window skin | `tests/m7_window_skin.rs` | H-WIN-1: grey 18x18 `SystemFile/WindowBase.png` nine-slice (no research doc) |

Note: `M7_BATTLE_FULL.md` (task-20) is the strongest W1 evidence and is **newer** than
`M7_BACKLOG.md`/`HANDOFF_M7.md`, so it overrides their "settlement unreachable" claim
(see §7).

---

## 1. Open-gap inventory (exact evidence + current blocker)

Legend: **blocker** = what physically prevents closing the gap today.

### G1 — Battle settlement is not wired into real play (W1 head gap) — **RESOLVED (W1-c)**
- Evidence: `docs/research/M8_BATTLE_SETTLE.md`; `docs/research/M7_BATTLE_FULL.md` §7;
  `docs/BASELINE.json` `battle_slice`.
- Resolution: CE242's arg0 is the acting character slot (1..3), not `2000354`
  (battle context). `battle_wiring.rs` now supplies slot 1; the genuine day-4 replay
  settles and reaches day 5 08:30 with no settlement injection
  (`m7_day4_playable`). The old bootstrap/circularity analysis below remains true for
  the shipped call graph.
- Fact: CE244 settlement logic is correct and reachable via a **direct**
  `start_event(242)` (SC1: 2000350 → [0,1,0,2,0], 2000352=1, 2000343=1). The only way
  in-data is a circular bootstrap: `2000350=1` is written only by `CE242 pc14`, and
  CE242's run condition is `2000350==1`. CE188 has no `CALL 500244`; CE239 pc795 issues
  `210[500244]` but only after 2000340/343/352 unwind flags are set.
- **Blocker**: no in-data starter for CE242; CE239 parks at pc13/pc857 waiting on
  2000343/2000350. Requires either a documented engine rising-edge hook or an accepted
  seeded path.

### G2 — CE65 command-menu DB strings unresolved
- Evidence: `docs/BASELINE.json` `battle_slice.remaining`; `docs/research/M6_BATTLE_COMMANDS.md`.
- Fact: CE65's 8 `250 CData (19,0..7,5)` menu strings are unanswered; CE152 pc64 reads
  `Data0 col20 発動アイテム要する?` which is missing (field_count=18 → stays 0).
- **Blocker**: DB cell/column not present in shipped data; needs default/fallback decision.

### G3 — type18 target rows / CE190 runtime target list
- Evidence: `docs/research/M6_BATTLE_COMMANDS.md`; `M7_BATTLE_SEED.md` boundary; `M7_BACKLOG` W1 step3.
- Fact: CData type18 row17 == 0 causes CE65 pc78↔pc140 spin; M7 seeded only type18
  row4 col0=1 (party count). CE190 builds runtime target rows; not otherwise seeded.
- **Blocker**: not yet exercised inside a real battle turn; belongs with G1 wiring.

### G4 — H text/word layer never visible
- Evidence: `docs/research/M7_HAUTO_EVENTS.md` (mechanism + line refs
  `crates/wolf-game/src/picture.rs:675` `Transition::from_command` needs non-zero
  `animate`; `picture.rs:1545` `merge_args` OPACITY; `picture.rs:890` `alpha`).
- Fact: every `hword/word_effect*` draw is immediately followed by a same-number move
  with opacity 0, interpreted as an instant teleport → alpha stays 0. ev353 still draws
  visible CG 1185 (quad 40→41) but no glyphs.
- **Blocker**: needs a picture-move semantics change (cmd=1 move + fade!=0 → timed
  transition), which is a prod edit outside the M7 tasks' write scopes.

### G5 — H forward path not end-to-end — **ADDRESSED (W2-d)**
- Evidence: `docs/research/M7_HSCENE_ADVANCE.md` §8 (stage-selected, wrapper from
  `SampleMapA ev33 → CE333` not attempted); `M7_HSCENE_FULL.md` boundary (resume, not
  forward).
- Fact: 14 polls proven individually, but the parent chain that advances `var218`/arg0
  between stages (CE372/CE323 for CE371, CE402/CE328 for CE410, CE246/CE315/CE322 for
  CE346) is not driven as one continuous playthrough from the map H button.
- **Blocker**: wrapper-chain state machine not reconstructed/tested.

### G6 — character_state / map-actor layer absent
- Evidence: `docs/research/M7_HOST_STATE.md` §"无法证明的残余" 2; `docs/research/M7_OPCODE_124.md`
  §残余; `game.rs:1936-1944` (per doc).
- Fact: `character_state` returns None for all fields; `Game::start_pages` discards the
  parsed `Map`, `MapRuntime` tracks only page programs (no x/y/direction). So the 40
  opcode-124 `0x1000/0x1200` sites keep stale values.
- **Blocker**: no map/actor coordinate layer exists in the port (M2-scale work).

### G7 — engine_state field 17 semantics / 11/12 sentinels
- Evidence: `docs/research/M7_HOST_STATE.md` §残余 1,3; `M7_OPCODE_124.md` (5 sites
  CE265-270 as message-window guard; 11/12 each 1 site CE225).
- Fact: field17 returns 0 (probable "processing event rows"); 11/12 return -1 valid only
  while no map interaction layer exists.
- **Blocker**: needs editor/real-machine breakpoint to pin field17; 11/12 change when G6 lands.

### G8 — PRNG range reduction inferred
- Evidence: `docs/research/M7_PRNG.md` §4; `docs/UNVERIFIED.md` H-PRNG-1.
- Fact: state/constants/15-bit draw exact; `range(lo,hi)` uses multiply-high
  `(draw*(span+1))>>15`, **not** decompiled; byte-for-byte parity with retail unproven.
- **Blocker**: needs Game.exe random-op decompile / same-save replay parity.

### G9 — Visual-fidelity residuals (anchor / picture slots / window)
- Evidence: `docs/UNVERIFIED.md` H-PIC-2 ([2]/[5] slots), H-ANC-1 (6 anchor nibbles, 333
  samples treated top-left), H-WIN-1 (resolved grey skin, still needs wider-site consistency).
- Fact: `[2]/[5]` picture slots ignored (provably harmless on title); non-centre anchors
  unverified for 2.4% of draws.
- **Blocker**: needs retail screenshots / decompile; low impact, cosmetic.

### G10 — M4 audio not ear-verified
- Evidence: `docs/BASELINE.json` `m4_audio`; `M7_BACKLOG` W5 notes audio needs human ear.
- **Blocker**: requires a human listener; not machine-checkable.

### G11 — New-game template save absent on some roots
- Evidence: `docs/research/M7_NEW_GAME_ENTRY.md` §2 benign diagnostic
  `0x 220 SaveLoad: no template save`; `M7_NEW_GAME_ENTRY` reproduction on `work/all`.
- Fact: `Game::new` `base_save = read_save(root/Save/System.sav).ok()`; roots without
  `Save/System.sav` get `None` → `save_to` fails. `work/full` supplies it.
- **Blocker**: production packaging of a template save, or a synthesized base.

### G12 — Dangling map id in shipped data
- Evidence: `docs/research/M7_MAP_EDGE.md` §2/§4 (Map031 on-load teleport to map id 27,
  empty SysDatabase row, load fails `cannot read work/all\:`).
- Fact: data bug, not decoder bug; Map031 is a tile-less side map.
- **Blocker**: guard/skip in `db.rs map_path` / `switch_map` so a bad map id degrades
  gracefully instead of erroring.

---

## 2. Ranked M8 workstreams

Ranking rule: unblock the *playable* mainline first (W1), then visible content (W2),
then structural fidelity, then polish. Each item: goal / acceptance / candidate files /
likely tests / dependencies.

### W1 — Real battle completion (highest priority)
**Goal**: a real `CE188`→`CE239` battle settles without any variable pre-seed, writes
`2000352 ∈ {1,-1}` and `2000343=1`, returns to pc1106, then day4 dinner → CE329 →
**day5 with no injection**.

Sub-tasks:
- **W1-a (DONE)**: command availability (G2/G3 partial) via `seed_battle_databases()`.
- **W1-b (DONE, test-only)**: settlement logic proven reachable (`m7_battle_full` SC1) — this
  is the acceptance oracle for W1-c.
- **W1-c (DONE, M8)**: wire the battle end. The rebuilt wiring now supplies CE242's real
  arg0 — the **acting character slot** (1), not the battle context `2000354`.
  `docs/research/M8_BATTLE_SETTLE.md`; acceptance test
  `m7_day4_playable::day4_real_battle_settles_and_reaches_day5_without_injection`
  (day1→day4, genuine CE253→CE239, CE244 settles `2000352=1`/`2000343=1`, CE239 returns,
  dinner, natural night menu, **day 5 08:30**, warnings 0, no settlement injection).
  `m7_battle_playable` updated to the new state sequence.
- **W1-d (DONE, M7 batch-2)**: CE65 command menu strings + type18 target rows; see
  `M7_BATTLE_MENU_DB.md`, `M7_BATTLE_TARGET_SCHEDULER.md`.
- Remaining W1 polish: a multi-member party would need the per-member CE242 loop; the
  day-4 replay party power is still 132 (normal progression, not a defect).

### W2 — H-scene visibility and forward path
**Goal**: H text/word glyphs actually appear, and the scene is playable forward from the
map H button.
- **W2-a (DONE)**: opcode-123 advance 14/14.
- **W2-b (DONE, resume)**: layer stack + late-night UI var62 walk.
- **W2-c (REMAINING)**: fix `picture.rs` move semantics so `cmd=1 move` with `fade!=0`
  is a timed opacity transition (G4). Acceptance: schedule ev353 (var99=1) → a
  `hword/word_effect*` quad reaches alpha>0 for ≥N frames (assert alpha>0 + quad count
  increase), warnings 0; regression `m7_hauto_events` still green. Files:
  `crates/wolf-game/src/picture.rs`. Test `m8_hword_visible.rs`. Depends: none.
- **W2-d (DONE, M8)**: `m8_hscene_forward.rs` drives the wrapper chain with real
  pointer input. One continuous SaveData08 session: wipe picture 406 → map ev21 pc1072
  (crosses CE343 pc963 / CE333 pc986), s01 picture 400 → `var62` 1→2, end picture 294 →
  ev25 → CE345; a fresh replay s04 picture 403 → stage 11. CE333's body replays 1,739
  instructions and redraws `DLC_data/pose_10.png` + `hword/word_effect102.png`. Siblings:
  CE246→CE217 (clock 0:42→0:52 + header text), CE246 deep → CE346 (max pc 180, visible
  cutin), CE323→CE372/CE371 via the toilet menu (`var218` stays 2 by design), CE328→CE402
  blocked by `v104/v105==0` and `v4>=21`. The map H button ev33 stays gated on
  `var460==1`: the ejaculation path needs `v111>=100` (save 0) then CE344/CE347, and the
  shallow/anal/end guards (v267, v100>=12, v98>=1) are asserted exactly. warnings 0.
  Docs `M8_HSCENE_FORWARD.md`.

### W3 — Map/actor layer & host state (structural)
**Goal**: real `character_state` coordinates so the 40 opcode-124 sites see true values.
- **W3-a (DONE, M8)**: `MapRuntime` now keeps every event's tile x/y and each page's
  facing (`MapActor`, `LeaderPlacement`, `place_leader`), and `GameHost::character_state`
  answers fields 0..10 for map events and the leader (precise = tile*2, screen centre/feet,
  id). A hero `130` transfer places the leader (options bit 0 = half-tiles). Tests:
  `m8_character_state.rs` (3), `map_runtime` (3), lib 230; docs
  `M8_CHARACTER_STATE.md`. Limits: no movement model; height 0; `-1`/`-3..-7` stay None.
- **W3-b (REMAINING)**: pin `engine_state` field17 (G7) with a breakpoint/decompile;
  adjust 11/12 when W3-a lands. Files: `game.rs`. Test extend `m7_host_state`.

### W4 — Save / new-game / data robustness
- **W4-a (DONE)**: 5-part save roundtrip + Part4 template regression fix (`m7_save_parts`).
- **W4-b (DONE, M8)**: `SaveFile::new_game_template()` builds a structurally valid 0x8C
  template (no-map sentinel, 31 auto-tiles, 800 normal vars, 550 event groups, empty vardb,
  one-byte Part7); `Game::new` falls back to it when System.sav is absent and records one
  diagnostic. Test `m8_template_save.rs` on `work/all` (no Save/): save + fresh `load_into`
  restores playtime, 800 vars and the picture layer; retail saves still byte-identical.
  Docs `M8_TEMPLATE_SAVE.md`. Residuals: synthesized vardb has no types (CData cells are
  reported as `unmodelled`), and Part2/Part1 defaults are zero.
- **W4-c (DONE)**: `db::map_path` returns `None` for an empty path cell, so the dangling
  id 27 is skipped with one diagnostic instead of reading the data root. Test
  `m7_map_edge::a_dangling_map_id_is_skipped_without_a_read_error`; `m7_map_edge` 4/4 green.

### W5 — PRNG exactness + visual fidelity polish (lowest)
- **W5-a (LAW PINNED; generator port in progress)**: Game.exe RVA 0xaf7c0 proves the
  reduction is `min(a,b) + draw % (abs(b-a)+1)` (one draw per call, wrapping 32-bit).
  A second finding: the draw is 31-bit Park-Miller MINSTD (RVA 0x35810, 16807/2^31-1,
  clock-seeded, rejection > 2e9), **not** the MSVC LCG that `WolfRng::next_raw` replays
  (that stream is the Game.dat static table/crypto). Applying the law to the 15-bit MSVC
  stream was prototyped, broke the day-loop sequences, and was reverted. Follow-up task:
  port Park-Miller into the event RNG and re-align `range` in the same change. Docs
  `M8_PRNG_RANGE.md`; residual = deterministic seed stand-in vs retail clock seed.
- **W5-b (OPTIONAL)**: anchor nibbles / picture `[2]`,`[5]` slots (G9) with retail
  screenshots. Files: `crates/wolf-game/src/picture.rs`. Low priority.
- **W5-c (OPTIONAL, human)**: M4 audio ear check (G10) — cannot be machine-gated.

---

## 3. DONE vs REMAINING (one-line ledger)

DONE in M7 (all have a test or a documented capture):
PRNG port · battle round (CE188 line) · battle seed (command availability) · real CE244
settlement reachable via SC1 · H advance 14/14 · H resume + var62 UI walk · H auto-event
scheduler · opcode124 interpreter + host fields · day5 with 4-var injection · map edge
cases · new-game entry route · save 5-part roundtrip · choice window · window skin ·
battle command-menu DB strings · H word-layer visibility · **battle settlement wired into
real play (W1-c, day4→day5 no injection)** · **map/actor placement layer (W3-a)** ·
**synthesized new-game template save (W4-b)** · **dangling-map guard (W4-c)** ·
**H forward wrapper chain (W2-d)**.

REMAINING (M8): PRNG Park-Miller generator + range law (W5-a, delegated; the reduction
law itself is pinned) · engine_state fields 11/12/17 (W3-b, delegated) · anchor/slot and
audio polish (W5-b/c).

Estimated priority order for M8: **W5-a, W3-b**, then polish.

---

## 4. How to verify the whole repo

```
cd 'E:/Study/github program/imoto life'
source tools/env.sh                      # or the repo's env helper

# 1. whole workspace tests (unit + integration; green at 1d36ebe)
cargo test -q --workspace

# 2. data/opcode audit: 70 checks, 0 pending
cargo run -q -p wolf-cli -- --root work/all verify

# 3. the playable day loop (M6)
cargo test -q -p wolf-game --test m6_multiday
cargo test -q -p wolf-game --test m6_week_loop

# 4. battle evidence
cargo test -q -p wolf-game --test m7_battle_round
cargo test -q -p wolf-game --test m7_battle_seed -- --nocapture
cargo test -q -p wolf-game --test m7_battle_full  -- --nocapture
cargo test -q -p wolf-game --test m7_day5_schedule -- --nocapture

# 5. H-scene evidence
cargo test -q -p wolf-game --test m7_hscene_advance
cargo test -q -p wolf-game --test m7_hscene_full
cargo test -q -p wolf-game --test m7_hauto_events
cargo test -q -p wolf-game --test m7_host_state

# 6. map / new-game / save evidence
cargo test -q -p wolf-game --test m7_map_edge
cargo test -q -p wolf-game --test m7_new_game_entry
cargo test -q -p wolf-game --test m7_save_parts
cargo test -q -p wolf-game --test m7_window_skin

# 7. key captures (1024x576; expect >=89% non-black where noted)
cargo run -q -p wolf-cli -- --root work/all capture --boot \
  --frames 3200 --key-frame 800:10 --answer 0 --answer-delay 2 \
  --out work/out/m7_new_game_entry.png          # 92.0%
cargo run -q -p wolf-cli -- --root work/all capture \
  --load-save work/retail_backup/Save/SaveData08.sav --frames 90 \
  --out work/out/m7_hscene_full.png             # 89.4%
```

Data roots: most M7 battle/H tests default to `work/all`; save/reload tests
(`m6_mainline`, `m7_save_parts`, `m7_window_skin`) need `work/full` (has
`Save/system.sav`). Override with `WOLF_DATA_ROOT=<dir>`.

---

## 5. Candidate test-name registry (new M8 tests, suggested)

| workstream | test file | first test name |
|---|---|---|
| W1-c | `crates/wolf-game/tests/m8_battle_settle.rs` | `a_real_battle_settles_and_returns_to_the_day_loop` |
| W1-d | `crates/wolf-game/tests/m8_battle_menu.rs` | `the_battle_command_menu_strings_and_targets_resolve` |
| W2-c | `crates/wolf-game/tests/m8_hword_visible.rs` | `a_word_effect_draw_reaches_visible_alpha` |
| W2-d | `crates/wolf-game/tests/m8_hscene_forward.rs` | `the_h_button_drives_every_stage_forward` |
| W3-a | `crates/wolf-game/tests/m8_character_state.rs` | `the_leader_reports_real_tile_and_direction` |
| W4-b | `crates/wolf-game/tests/m8_template_save.rs` | `a_root_without_a_template_can_still_save` |

---

## 6. Worktree hygiene constraints (carry over)

- Never commit the 3 permanent scratch files: `crates/wolf-format/examples/opscan.rs`,
  `crates/wolf-format/tests/_scratch_cdata.rs`, `crates/wolf-format/tests/_scratch_event.rs`.
- Never run `git clean`. Foreign untracked probe files currently present (not project
  artifacts): `crates/wolf-format/examples/_dump246.rs`, `crates/wolf-game/tests/_probe_entry*.rs`,
  `docs/gpt调研/`.
- Commit messages via `git commit -F work/_commit_*.txt`; keep `exec.rs` LF; outputs to
  `work/out`; `docs/gpt调研/` is not project-owned.

---

## 7. Contradictions / stale claims found while consolidating

1. **"CE244 settlement unreachable" is now false as a capability.** `M7_BACKLOG.md` and
   `HANDOFF_M7.md` say settlement is structurally unreachable. `M7_BATTLE_FULL.md` (task-20,
   newer) proves a **genuine** CE244 settlement via direct `start_event(242)` + minimal seed
   (`vars.set(495,2)`, `self1600000=1`). The accurate statement: `CE242 has no in-data
   starter (2000350 bootstrap)`, so it must be wired by an engine policy. M8 must plan
   against the newer fact.
2. **`M7_BACKLOG.md` recommends "first slice W1-a"** (CE152 command availability) — that is
   already DONE by `battle_seed.rs`; the real remaining W1 head is settlement wiring (W1-c).
3. **H-124 listed as a negative ("579 SetVariableEx")** in `M7_BACKLOG.md`/`HANDOFF_M7.md`;
   `M7_OPCODE_124.md` + `UNVERIFIED.md` show it is implemented (1045 sites, host field gaps
   only). Not an open parser gap.
4. **H-MENU-1 still listed as a negative** in `M7_BACKLOG.md`; it is resolved
   (`choice.rs overlay_at_sized`, `m7_choice_window`). Stale.
5. **"Non-battle day4→day5 path exists"** (`M7_BACKLOG.md` W1 note) vs empirical reality:
   day4 daytime is **forced** into the CE251→CE253→CE239 battle (clock freezes 15:00), so
   day5 is only reached today with a 4-var CE244 win-branch injection
   (`M7_DAY5_SCHEDULE.md`). Natural day5 therefore depends on W1-c, not on a separate
   schedule fix.
6. **BASELINE `m7_workstreams` is behind the worktree**: it lists only the 5 committed
   slices; the 10+ untracked M7 tests/docs are not yet reflected. Lead should refresh
   `docs/BASELINE.json` at integration (not done here — out of this task's write scope).

---

## 8. Top 5 ranked items for M8 (summary)

1. **W1-c — Wire real battle settlement** (engine rising-edge hook on 2000340 0→1 →
   CE242, or accepted SC1), acceptance: genuine CE244 settle + CE239 return + day4→day5
   with no injection.
2. **W1-d — Battle command menu DB**: CE65 `CData(19,0..7,5)` strings + type18 target rows
   so the battle menu and target list are complete.
3. **W2-c — H word-layer visibility**: `picture.rs` move+fade timed transition so
   `hword` glyphs reach alpha>0 (ev353 oracle).
4. **W2-d — H forward wrapper chain**: drive `SampleMapA ev33 → CE333` through every H
   stage with real input.
5. **W3-a — character/map-actor layer**: real `character_state` coordinates for the 40
   opcode-124 sites (largest structural item; depends on MapRuntime keeping positions).
