# M9 W2 residual — unlocking the H-scene CG cluster (var460 / var111 / var157)

Result: **a legitimate, real-input path into the map H CG cluster exists.** Hovering the
"挿入 抜く" (insert-pull) button writes `var460 = 1`, which activates map event 33
("挿入抜く"); `ev33` then calls `CommonEvent` **CE333** fifteen times — the H insert-pull
animation / CG expression body. This works even on the drained mid-H save
`work/retail_backup/Save/SaveData08.sav`, because neither `var460` nor `ev33` is gated by
the energy variables `var100`/`var267`.

Separately, the arousal gauge `var111` *is* raisable to `>= 100` through real insertion
hover codes, provided the day-save energy state (`var100 >= 12`, `var267 == 0`) holds — the
exact state all seven day saves ship with. The two deeper CG gates that stay closed
(`CE246` deep body needs `var157 == 13`; `CE402` needs `var104/var105 == 0` and `var4 >= 21`)
are separate player-progress gates outside the H-UI cluster and are documented, not claimed
opened.

Data root: `work/all` (H files only live in the full unpack). Save: `SaveData08.sav`.
Test: `crates/wolf-game/tests/m8_hscene_cg_unlock.rs` (5 tests).
Run: `WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_cg_unlock`.

## Correction of the earlier "blocked" conclusion

`m8_hscene_forward.rs` (header) and `docs/research/M9_HSCENE_LOOP.md` both claimed the
forward `ev33 -> CE333` chain is unreachable because "`var460 = 1` is written only by
CE344/CE345/CE347". That audit scanned **only** `CommonEvent.dat`; `var460 = 1` is also
written by the **map events themselves**:

| Event / page | PC | Opcode-121 args | Meaning |
|---|---|---|---|
| map event 11 page0 | 706 | `[2_000_460, 1, 0, 0]` | dispatcher arms var460 on the code-35 branch |
| map event 11 page0 | 705 | `[2_000_061, 35, 0, 0]` | same branch sets var61 = 35 (挿入 抜く) |
| map event 26 page0 | 496, 862, 1119, 1319, 1353 | `[2_000_460, 1, ...]` | track-event insertion arms var460 |
| map event 28 page0 | 327, 465, 610, 929, 1356, 1715 | `[2_000_460, 1, ...]` | アナル insertion arms var460 |
| map event 33 page0 | 334 | `[2_000_460, 0, 0, 0]` | ev33 clears var460 when it finishes |

So the map dispatcher itself opens the cluster; no ejaculation required.

## The exit/entry chain that opens the cluster

1. **Every late-night H UI page draws the H buttons; the insert-pull button is picture 444.**
   Hovering it produces engine hover code 35 (dispatcher map below).
2. **Map event 11 "■深夜UI判定" page0** (conds `var62 >= 1`, `var168 == 0`, `var364 == 0`)
   runs the code-35 branch: **pc705** `var61 = 35`, **pc706** `var460 = 1` (then `var481 = 0` at pc707).
3. **Map event 33 "挿入抜く" page0** — its **only** enabled page condition is
   `var2000460 == 1` (verified: exactly one enabled slot). It activates, and its body calls
   **CE333** via `op210 500333` at pcs 60, 103, 210, 224, 231, 245, 252, 254, 258, 264, 266,
   269, 275, 282, 285 (15 calls). On completion, **pc334** `var460 = 0` clears the trigger.
4. **CE333** is the H insert-pull animation / CG expression helper. Its bath branch selects on
   `var458 == 1` (CE333 pc0); otherwise it draws the bedroom body.

CE246 / CE372 / CE402 are *separate* gated callees of other wrappers, not direct `ev33`
calls; they are reachable only through their own gates (below).

## var111 (arousal / ejaculation gauge) — reachable to >= 100 given day-save energy

Writers that **increase** `var111` (opcode 121, first arg `2_000_111`):

| Event / page | PC | Args | Effect |
|---|---|---|---|
| map event 26 | 482 | `[2_000_111, 8, 0, 256]` | +8 (gated `var267 == 0` at pc475) |
| map event 26 | 595, 789, 816, 884, 1143, 1346 | `[2_000_111, 5, 0, 256]` | +5 |
| map event 27 | 575, 758, 977, 1378, 1759, 2144; 2139 | | +10 / +5 |
| map event 28 | 156, 369, 507, 774, 1206, 1569 | `[2_000_111, 15, 0, 256]` | +15 (needs `var100 >= 12`, pc124) |
| map event 28 | 159, 372, 510, 777, 1209, 1572 | `[2_000_111, 119, 2_000_111, 4352]` | `var111 = 119 + var111` |
| CE340 / CE346 / CE348 | | | +5 / +4 / +3 |

Readers / gates:

* **射精 (pic447) is drawn only when `var111 >= 100`** — map event 9 page0 pc336 tests
  `[2, 2_000_111, 100, 1]` before drawing picture 447 at pc358. Hovering picture 447 yields
  hover code 48, which map event 25 dispatches to **CE344**.
* **CE344 needs `var111 > 99`** — pc87 `[1, 2_000_111, 99, 3]` (op 3 = `<=`, so the
  `||`-guarded block runs only when `var111 >= 100`). CE344/CE347 then spawn and reset the
  gauge (100 -> ~20).
* **EV5 pc341** `[2_000_111, 100, 0, 0]` is a *scripted* setter (a cutscene path), not the
  interaction path.

Why `SaveData08` alone never raised the gauge: it is the drained post-action snapshot
(`var100 == 0`, `var267 == 1`), so EV26 skips its `var267 == 0` block and EV28 takes the
`var100 < 12` abort branch. Every **day** save (01–07) ships `var100 = 70..75`, `var267 == 0`,
so the same real hover codes do compound the gauge; a continuous playthrough passes 100 in a
couple of insertion rounds (peak 120 measured, warnings 0).

## Deeper CG gates (documented; not claimed open)

| Gate | Location | Requirement |
|---|---|---|
| CE246 deep body | CE246 pc1943 `[17, 2_000_157, 13, 2]` | `var157 == 13` |
| CE402 (via CE328) | CE328 pc6 `[.., 2_000_104, 0, 2]`, pc4 `[.., 2_000_105, 0, 2]` | `var104 == 0` & `var105 == 0` (plus internal `var4 >= 21`) |

`var157 == 13` is carried only by a self-variable copy path (CE281 / CE328);
`SaveData07` already carries `var157 == 13`, and `SaveData04` carries `var104 = 1 / var105 = 2 /
var4 = 21`. These are player-progress flags, distinct from the H-UI cluster.

## Retail save cross-table (raw `load_save` values)

| Save | var111 | var460 | var100 | var267 | var157 | var104/105 | var4 |
|---|---|---|---|---|---|---|---|
| 01 | 0 | 0 | 70 | 0 | 12 | 0/0 | 21 |
| 02 | 0 | 0 | 70 | 0 | 1 | — | 18 |
| 03 | 0 | 0 | 70 | 0 | 10 | — | 22 |
| 04 | 0 | 0 | 75 | 0 | 12 | 1/2 | 21 |
| 05 | 0 | 0 | 70 | 0 | 12 | 1/3 | 18 |
| 06 | 0 | 0 | 70 | 0 | 8 | — | 18 |
| 07 | 0 | 0 | 70 | 0 | 13 | — | 19 |
| 08 | 0 | 0 | 0 | 1 | 12 | 0/0 | 0 |

Every save ships `var111 == 0` and `var460 == 0`; day saves 01–07 carry the energy state
(`var100 >= 12`, `var267 == 0`); `SaveData08` is the drained exception.

## What the tests pin (`crates/wolf-game/tests/m8_hscene_cg_unlock.rs`, 5 tests, deterministic)

* `insertion_hover_35_arms_var460_and_opens_event_33` — from resumed `SaveData08`
  (`var364 = 0` normalized, `var62 = 1`), feeding the real dispatcher output `var61 = 35`
  drives `var460` to 1 and activates map event 33; warnings 0.
* `without_the_insertion_hover_event_33_stays_closed` — negative control: `var61` left 0,
  `var460` stays 0, event 33 never activates; warnings 0.
* `h_cg_cluster_writers_and_readers_are_present_in_the_data` — asserts the static graph:
  ev33 single `var460 == 1` condition, ev33 `CE333` x15, ev11 pc705/pc706, EV26/EV28 var460
  and var111 writers, EV28 `var111 = 119 + var111` + `var100 >= 12` gate, EV33 pc334 clear,
  CE344 pc87 `var111 > 99`, EV5 pc341 `var111 = 100`, CE246 pc1943 `var157 == 13`,
  CE328 `var104 == 0` / `var105 == 0`.
* `retail_saves_carry_zero_gauge_and_split_on_the_energy_state` — the cross-table above.
* `var111_reaches_100_through_real_insertion_hover_codes` — real hover codes 31..45, with the
  day-save energy invariant (`var100 = 70`, `var267 = 0`) re-asserted per action, drive
  `var111` from 0 past 100 (peak >= 100, warnings 0).

## Exact inputs / residual normalizations

* Live H entry: `Game::new(work/all)` + `load_save(SaveData08.sav)` + `start_engine_events()`
  + `start_pages(&map, Some(0))` (skips opening autorun event 0).
* Only scene normalization: `var364 = 0` (asserted already 0; the dispatcher gate).
* The one injected interaction value is `var61` — the documented **dispatcher output**: the
  engine's map event 11 writes `var61 = code` from an opcode-124 hover of the corresponding H
  button (hover map: picture 444 -> code 35 = 挿入 抜く). Setting `var61` to a code is therefore
  identical to hovering that button; no scene variable is fabricated.
* Test E additionally re-asserts `var100 = 70` / `var267 = 0` between actions — the exact
  energy state the seven day saves ship — to emulate the day-to-day energy the real game resupplies.
