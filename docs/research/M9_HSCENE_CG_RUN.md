# M9 W2 — driving the real H CG cluster from the proven `var460` / `ev33` path

Result: **the `var460` -> map event 33 path renders the H CG picture cluster on the real
retail saves, with 0 warnings.** The call chain is `ev33 -> CE333`, and `CE333` is a *leaf*
CG-drawing body — `ev33` never reaches `CE246` / `CE328` / `CE402`. Those deep entries are a
separate menu topology whose conjunctive gates are **not jointly satisfied by any single
retail save** (pinned in test 4).

Data root: `work/all`; saves `work/retail_backup/Save/SaveData01..08.sav`.
Test: `crates/wolf-game/tests/m8_hscene_cg_run.rs` (4 tests, green ~1.2 s).
Command: `WOLF_DATA_ROOT=work/all cargo test -q -p wolf-game --test m8_hscene_cg_run`.

---

## 1. What `ev33` actually drives (topology)

`ev33` page-0 has exactly one enabled condition, `var460 == 1`. It is armed legitimately by
map event 11 (the H UI dispatcher): hovering the code-35 "挿入 抜く" button (picture 444)
makes pc705 set `var61 = 35` and pc706 set `var460 = 1`. Every `op210` in `ev33` targets
`500333` (= **CE333**) — nothing else:

| node | op210 targets | role |
|---|---|---|
| `ev33` (挿入抜く) | `500333` x15 (pcs 60/103/210/224/231/245/252/254/258/264/266/269/275/282/285) | entry; draws the CG through CE333 |
| `CE333` | **(none — leaf)**; 401 `op150` picture draws | draws hscene bg / frame / pose / CG / DLC |
| `CE246` | `500333` (+ others) at **pc1983 only** | separate menu entry; calls CE333 *after* the `var157 == 13` block opened at **pc1943** (reverse direction) |
| `CE328` | incl. `500402` | separate menu entry; entry gates **pc4 `var105 == 0`**, **pc6 `var104 == 0`** |
| `CE402` | `500222/500264/500403/500407/500410/500412` | inner gates pc0 `var16 >= 1`, pc2 `var22 >= 180`, pc4 `var4 >= 21`, pc8 `var102 >= 80`, pc21 `var103 == 2` |
| `CE372` | `500371` | thin wrapper, no global conds |

So the CG cluster that `ev33` produces *is* CE333's leaf body. `CE246`/`CE328`/`CE402` are
reached from the day-time H menu (スキンシップ / パンツ etc.), not from `ev33`.

## 2. Empirical: the cluster renders on the real saves

Harness: `Game::new(work/all)` + `load_save(SaveDataNN)` + `start_engine_events()` +
`start_pages(&SampleMapA, Some(0))`; drive 60 frames; the only normalizations are
`var364 = 0` (shipped dispatcher flag), `var62 = 1` (enter the late-night H UI — the day
saves start `var62 = 0`; `SaveData08` already ships `var62 = 1`), and `var111 = var460 = 0`
(their shipped values). Then feed the real dispatcher output `var61 = 35` and run 160 frames,
collecting every picture `file` that appears on `game.screen`.

| save | `var137` (pose) | `var460` peak | ev33 ran | distinct H-CG files | warnings |
|---|---|---|---|---|---|
| `SaveData08` (mid-H) | 8 | 1 | yes | 21 (incl. `h_s10` sheet) | 0 |
| `SaveData07` (day) | 4 | 1 | yes | 21 (incl. `h_s05` sheet) | 0 |
| `SaveData04` (day) | 4 | 1 | yes | 20 (incl. `h_s05` sheet) | 0 |

Every run draws `hscene/h_bg_001.jpg`, `hscene/h_frame.png`, a `hscene/h_sNN` pose sheet
selected by `var137`, `DLC_data/pose_10.png`, `cg/bg_14_sunset_a.jpg` and
`cg/blackscreen.jpg`. `var460` is set to 1 by the dispatcher then cleared back to 0 by ev33
pc334 within the same frame, so the *peak* is 1 while the post-frame value is 0.

## 3. Deep CG gates — no single real save satisfies them

Raw variables read from each save (`load_save` restores them):

| save | var157 | var104 | var105 | var4 | var102 | var16 | var22 | CE246 `v157==13` | CE328 entry `104/105==0` | CE402 inner |
|---|---|---|---|---|---|---|---|---|---|---|
| 01 | 12 | 0 | 0 | 21 | 120 | - | 347 | no | yes | (v22 ok; v16 unknown) |
| 02 | 1 | 0 | 0 | 18 | 12 | - | 0 | no | yes | no |
| 03 | 10 | 0 | 0 | 22 | 52 | - | 102 | no | yes | no (v102) |
| 04 | 12 | 1 | 2 | 21 | 90 | - | 122 | no | **no** | no (v22<180) |
| 05 | 12 | 1 | 3 | 18 | 59 | - | 121 | no | **no** | no |
| 06 | 8 | 0 | 0 | 18 | 33 | - | 157 | no | yes | no |
| 07 | **13** | 0 | 0 | **19** | **59** | - | 213 | **yes** | yes | **no (v4<21, v102<80)** |
| 08 | 12 | 0 | 0 | 0 | 120 | - | 375 | no | yes | no (v4<21) |

Conclusions:
- The CE246 deep gate `var157 == 13` is carried by **only `SaveData07`**.
- `SaveData07` also clears the CE328 entry (`var104 == 0 && var105 == 0`), so it would *enter*
  CE328 and reach CE333 at CE246 pc1983 — **but** CE402's inner gate needs `var4 >= 21` and
  `var102 >= 80`; `SaveData07` ships `var4 = 19`, `var102 = 59`, so the **`CE402` CG node
  stays closed**.
- `SaveData04`, the other named save, carries `var104 = 1` / `var105 = 2`, so it **fails the
  CE328 entry** (it does satisfy `var4 >= 21`).
- Therefore the full set `{var157 == 13 & var104 == 0 & var105 == 0 & var4 >= 21 & var102 >= 80}`
  is satisfied by **no** retail save (test 4 asserts the joint set is empty).

## 4. Screenshot

The transient `ev33 -> CE333` overlay is verified by the picture-file set above; the engine
has no in-crate screenshot API usable from a unit test. The rendered H CG that CE333's body
draws is captured from `SaveData08` (which ships `var137 == 8`, i.e. the same `h_s10` pose:

```
./target/debug/wolf-cli.exe --root work/all capture --load-save work/retail_backup/Save/SaveData08.sav \
    --frames 90 --out work/out/m9_hscene_cg_run.png
```

`work/out/m9_hscene_cg_run.png`: 1024x576, **89.4% non-black**, H CG scene (h_s10 pose + inset
+ H UI), 0 warnings.

## 5. What the tests pin (`m8_hscene_cg_run.rs`)

1. `ev33_renders_the_h_cg_cluster_on_the_mid_h_save` — live `SaveData08`: `var460` peak 1,
   ev33 runs, >= 12 distinct H-CG files incl. `h_bg_001`/`h_frame`/`h_s10`, 0 warnings,
   game still on H UI after the overlay.
2. `ev33_renders_the_cg_cluster_on_savedata07_and_savedata04` — live `SaveData07` and
   `SaveData04`: same cluster + 0 warnings.
3. `ev33_calls_only_ce333_which_is_a_leaf_cg_body` — static: `ev33` op210 targets == {500333}
   (x15); `CE333` has 0 op210 / 300+ op150; `CE246` calls CE333 only at pc1983 (> pc1943 gate);
   `ev33` never targets 500246/500328/500402.
4. `deep_cg_gates_are_not_jointly_satisfied_by_any_real_save` — static gate-reader PCs
   (CE246 pc1943; CE328 pc4/pc6; CE402 pc0/2/4/8) + the 8-save table above; the joint
   `{ce246 & ce328-entry & ce402-inner}` set is empty; `SaveData07` fails on `var4`/`var102`.

## 6. Residual injections (explicit)

- `var364 = 0`: the shipped dispatcher flag (cleared in every H harness).
- `var62 = 1`: enter the late-night H UI on the day saves (they ship `var62 = 0`);
  `SaveData08` already ships `var62 = 1`.
- `var61 = 35`: the real output of hovering picture 444 (map event 11 pc705). Not an arbitrary
  write — it is the dispatcher's own code for the code-35 button.
- `var111 = var460 = 0`: their shipped values.
- Everything else (pose selection, CG files, warnings) is produced by the real bytecode.
