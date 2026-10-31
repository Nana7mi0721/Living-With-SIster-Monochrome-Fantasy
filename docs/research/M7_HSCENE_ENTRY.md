# M7 · W2 — H entry path: day loop → CE246 → HUD/header bar, and the CE333/CE346 gate

Status: **delivered** — `crates/wolf-game/tests/m7_hscene_entry.rs` green (3 tests, ~7.5 s).
Data root: **`work/all`** (via `WOLF_DATA_ROOT`, else default). HEAD basis: task-21/29 worktree (1d36ebe lineage).

This task connects the H overlay system to its **genuine, non-synthetic pre-entry chain** instead of
resuming a mid-H save (`SaveData08.sav`, done in task-21). No H event is started with
`start_event`/`start_map_event`; every event below is entered by ordinary map/loop execution plus
real mouse+OK input.

## Result in one line

The **real** chain day-loop → night command menu `CE245` → **身体接触** → `CE246` **is reachable headlessly by
real input, warnings 0**, and selecting a light skinship option genuinely starts **`CE217` (HUD)** and draws
**`hscene/header_bar.jpg`** (`CE216`). The actual explicit CG renderer **`CE333`** and the **`CE346`** cutin are
only called from `CE246`'s deep **性骚扰** explicit branch behind a multi-variable intimacy/satisfaction gate
that a day-2 game does not carry, and which **cannot be synthesized with a small precondition**. That gate is
the **missing production hook** (a headless-drivable way to set H-mode / accumulated intimacy). Faithful
reproduction of the full CG therefore still requires a genuine mid-H state (`SaveData08.sav`, task-21).

## Real entry chain (exact ids / pcs / gates)

```
day loop (SampleMapA ev0 opening … 学习会 day command)  →  day2 19:30 bedroom   [m6_night_menu harness]
  var295==1 night-menu flag; hover picture 120 (imouto02 @ 240,340) 8 frames + key::OK
  → CE245 '■夜コマンドメニュー' alive, rows drawn (身体接触/沟通/洗澡/泡茶/今天先休息吧)
  left-click (512,200) = 身体接触 row
  → CE245 pc454 op102[7] case0 (\cself[6]) 処理1:
        pc457 111[1,2000014,15,4]  var14<15 → 累 dialog (recurse)
        pc466 111[…2000151>=3] / pc508 111[…2000027>=2] → 硬来|算了 confirm gate
        else pc528→pc529 op210[500246,0]  ← UNGUARDED willing path   (day2 defaults var151=0,var27=0 → direct)
  → CE246 '┣スキンシップ' alive → raises op102[6] menu
```

`CE246` six-way menu (op102[6] at pc1): **[`摸摸头`,`揉肩`,`拥抱`,`亲吻`,`挠痒痒`,`性骚扰`]**
(case bodies delimited by op401 at pc2/123/226/337/451/556).

## What a real light choice reaches (positive, non-synthetic)

Selecting any of 摸摸头/揉肩/拥抱/亲吻/挠痒痒 (light branches) calls `op210 500217`/`500218`/`500228`/`500226`
etc. Empirically every light option produces the **same** started-event set containing:

```
38,41,48,63, 213,214,216,217, 224,225,236,237,241, 261,269,270,271,272,273,274   (warnings 0)
```

Notably **`CE217`** (the HUD clock — one of the task's named chain events `246/217/333/346`) starts, and
**`CE216`** draws **`hscene/header_bar.jpg`** — the H overlay layer genuinely appears. Live stack ≈ 45–53
pictures. This is the real, drivable "H entry" the game performs before any explicit content.

## The blocker: CE333 (CG) / CE346 (cutin) gate

The actual H CG is drawn **only** by **`CE333`** via `op150` (`h_bg_001.jpg`, `h_bg_002.png`,
`h_s10_pose03.png`, `h_s10_face_mayu/eye/kuchi`, `h_s10_ef_syasei03`, `h_s10_action_vibrator2`,
`h_frame.png`, `hword/word_effect98.png`, … across pc4..pc2500+). `CE333` common-event callers (op210 500333):

| caller | pc | args |
|---|---|---|
| **CE246** (skinship) | **pc1983** | `[500333,4,2,1,0,3]` |
| CE247 | pc819 | … |
| CE332 | pc141 | … |
| CE335 | pc3423… | … |
| CE340 | pc219… many | … |

`CE346` (cutin) is called from CE246 pc695/906/1477/1543/1598/1692/1799 `op210[500346,3,24|25,1,1]`, and from
CE315/CE322. **Every** CE246 caller of CE333/CE346 sits inside the **性骚扰** (option 5, pc557+) **deep**
submenu body, not the light branches.

`性骚扰` routing (`CE246`):
- pc557 `111[1,2000020,60,3]` — **var20<=60** → pc559 block (returns/loops; does NOT show deep list).
- else (**var20>60**, default **75**) → pc564 `111[1,2000151,1,1]` (var151>=1) → **pc572 op102[5] deep submenu**
  `[让我看内衣,让我看裸体,想看你自慰,让你看我撸管,小鸟飞飞]`.  ⇒ the var20>60 default is **required**; setting
  var20<=60 breaks the deep list.
- Deep option bodies are gated by intimacy: pc575 var21≥100, pc577/593/610/629 var22 (≥50/100/200),
  pc642/681 var18 (≥50/80), pc644 var16≥1, pc651/674 var102 (≥30/40), pc587/607 var70, pc687 op102[17]
  `看破！`, **pc695 → CE346**, **pc1983 → CE333**, pc705 op102[17] `发射`.

### Empirical matrix (all warnings 0)

| Attempt | h_bg/h_s CG? | CE333? | CE346? |
|---|---|---|---|
| 6 primary night rows auto-answered 1500f (身体接触/沟通/泡茶 → ~51 pics, 洗澡 26) | no | no | no |
| 身体接触 → each light skinship option (摸摸头…挠痒痒) | **no** (header_bar only) | no | no |
| 性骚扰 → each deep option at default vars | no | no | no |
| 性骚扰 → deep options **after fabricating** {16:1,18:120,21:200,22:200,23:10,102:120,138:100,439:1,70:1} | **no** | **no** | **no** |

Fabricating high intimacy **after** the menu is up (so CE245 willingness is unaffected) still does not open
CE333/CE346, and injecting before the click flips CE245 into the `硬来|算了` gate instead. So there is **no
single small documented precondition**: the real explicit scene depends on accumulated H-mode state
(history flags + multi-day intimacy) that is not expressible as a few `vars.set` calls in this harness.

## Conclusion / missing production hook

The production (real, non-synthetic) entry reaches **`CE246` + `CE217` + header-bar H overlay** at day2.
To render the full explicit **`CE333`** CG from a real entry, the game needs an **H-mode / intimacy state that
is ground out over many in-game days** — there is no headless-drivable hook to set it, and no minimal variable
preset reproduces it without breaking the upstream routing. **Reported as the exact blocker.** Faithful full-CG
capture remains available via the mid-H `work/retail_backup/Save/SaveData08.sav` route (task-21:
`m7_hscene_full.rs`, PNG 89.4 % non-black).

## Test file

`crates/wolf-game/tests/m7_hscene_entry.rs` (3 tests, all green):
1. `day_loop_night_menu_click_reaches_ce246_skinship` — real click on 身体接触 row raises the six-way menu
   `[摸摸头,揉肩,拥抱,亲吻,挠痒痒,性骚扰]`, event 246 alive, warnings 0, **no** CE333/CE346 and **no** h_bg/h_s CG.
2. `real_skinship_choice_starts_hud_and_header_bar` — real menu choice 摸摸头 starts **CE217** and draws
   `hscene/header_bar.jpg`; warnings 0; npics ≥ 45; still no CE333/CE346, no h_bg/h_s.
3. `deep_explicit_scene_stays_gated_without_the_intimacy_hook` — 性骚扰 deep submenu present; every deep
   option (even with fabricated intimacy) leaves CE333/CE346 unstarted and no h_bg/h_s CG, warnings 0 —
   encodes the blocker.

### Run
```
cargo test -q -p wolf-game --test m7_hscene_entry
# default data root work/all; override with WOLF_DATA_ROOT=<path relative to repo root>
```

## Harness / API notes (reused from m6)
- `Game::start_map_event_by_name("SampleMapA",0)` (game.rs:685), `frame()` (game.rs:1013),
  `screen.get(number)`, `vm.instances()`, `vm.started_events`, `set_choice_cursor`.
- Hover state: `picture_state(field 8)` → `hover()` (game.rs:1907/1426); click state via
  `mouse_state`/previous_mouse_buttons (game.rs:1463/1156); OK key → `key::code` (game.rs:1892/1899).
- Click protocol: `mouse=[x,y]` + 10 frames; `mouse_buttons=1` + 5 frames; `mouse_buttons=0`.

## Cleanup
Temp probes deleted (`crates/wolf-game/tests/_probe_entry*.rs`, `crates/wolf-format/examples/_dump246.rs`).
Permanent scratch left untouched: `crates/wolf-format/examples/opscan.rs`,
`crates/wolf-format/tests/_scratch_cdata.rs`, `crates/wolf-format/tests/_scratch_event.rs`.
