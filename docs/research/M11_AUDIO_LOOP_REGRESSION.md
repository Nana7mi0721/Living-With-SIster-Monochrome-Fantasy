# M11 P2 — Audio loop regression thresholds and listening manifest

Shared task `task-88`. Companion to `docs/research/M11_AUDIO_ACCEPTANCE.md` (task-84 dossier) and
the executable `crates/wolf-audio/tests/m11_audio_loop_regression.rs`.
Status: **DONE / green**. Data root `work/all` (override `WOLF_AUDIO_ROOT`). No playback code change.
Human audition: **2026-11-02 完成**（28 首逐条勾选 clean；3 首 D 档免听，见 §8）。

## 1. Why this exists

The task-84 dossier is an *audit*: it measures the seam of every loop-tagged track and flags the
ones a human must audition. An audit that only *prints* can silently drift — if the decoder,
symphonia version, or an asset changes, the printed numbers move and nobody notices. This task
turns the dossier into a **regression**: every measured value is pinned to the accepted build, so
any future change that *worsens* a loop (bigger click, louder seam step, more DC, different loop
region) fails `cargo test -p wolf-audio` instead of shipping quietly.

It locks three things per track:
1. the inventory (exactly **31** loop-tagged files) and the **tag → decoded-region contract**
   (`decoded.loop_start == LOOPSTART`, `decoded.loop_end == min(LOOPSTART+LOOPLENGTH, frames)`);
2. the five seam metrics `seam_abs / step / ratio / rms_jump / dc_delta` against the accepted values;
3. the advisory flag set and hence each track's tier (A/B/C/D), plus determinism (a second full
   decode pass is bit-identical).

## 2. Metric definitions (unchanged from M11_AUDIO_ACCEPTANCE.md)

- `seam_abs` = worst per-channel `|sample(end-1) - sample(start)|`: the instantaneous waveform jump on wrap.
- `step` = mean `|sample(n)-sample(n-1)|` over both sides of the seam (2×1024-frame windows), the local step size.
- `ratio` = `seam_abs / step`: ≈1 is an ordinary step (no click), ≫1 is a click.
- `rms_jump` = `|RMS(~100 ms after start) - RMS(~100 ms before end)|`: a loudness step.
- `dc_delta` = `|mean(after) - mean(before)|`: a DC-offset step.

## 3. Tolerances and rationale

Decode is deterministic for a fixed asset set (same bytes, same symphonia), so the pins are tight.
Absolute epsilon **5e-4** sits well below anything audible yet far above f32/print rounding (observed
run-to-run difference 0.0000); it lets a genuinely fixed decoder rounding differ without flapping,
while a real seam change (≥0.001) fails.

| metric | tolerance | rationale |
|---|---|---|
| `seam_abs` | abs 5e-4 | direct click amplitude; smallest meaningful worsening is ~1e-3 |
| `rms_jump` | abs 5e-4 | loudness step; ~1e-3 audible |
| `dc_delta` | abs 5e-4 | DC offset; ~1e-3 audible |
| `ratio` | abs 0.02 | dimensionless click factor; 0.02 is far below the 8.0 "click" threshold |
| `step` | rel 20% + 2e-5 | small normaliser (1e-3..1e-1); only a sanity bound, not the click signal |
| `start`/`len`/`frames` | exact | integer container facts; any change is a real asset/decoder change |
| flags / tier | exact string | classification must not silently re-tier a track |

**Negative proof:** perturbing one pinned `seam_abs` from 0.0195 to 0.0300 makes the test fail with
`BGM/Acoustic_High_Collar.ogg seam_abs: actual 0.019535465 vs accepted 0.03 (|d|=0.0104 > 0.0005)`,
exit nonzero. Reverting restores green. The guard is real, not decorative.

## 4. Current inventory summary

- **31** loop-tagged tracks, all `.ogg`, all decode to a usable in-bounds region.
- Tiers: **A=17** (objective seam defect → must audition), **B=1**
  (metadata anomaly OVERRUN/LEN_EQUALS_TRACK), **C=10** (loudness/DC step only),
  **D=3** (no objective flag).
- Flag counts across tracks: SEAM_RATIO=11, OVERRUN=3, LEN_EQUALS_TRACK=3, RMS_JUMP=18, SEAM_ABS=14.

### 4.1 Per-metric maxima (worst three)

| metric | worst three |
|---|---|
| `seam_abs` | BGM/nemesis_loop.ogg=0.8079, DLC_data/bgm01.ogg=0.8068, BGM/soul_engage_loop.ogg=0.6304 |
| `ratio` | BGM/Acoustic_High_Collar.ogg=20.0200, BGM/Our_Construction.ogg=19.6600, BGM/nemesis_loop.ogg=19.6600 |
| `rms_jump` | BGM/p_egg_loop.ogg=0.3772, DLC_data/bgm02.ogg=0.3704, BGM/Be_yourself.ogg=0.2885 |
| `dc_delta` | BGM/p_egg_loop.ogg=0.0228, BGM/InstaVista.ogg=0.0200, BGM/madoromu2.ogg=0.0158 |
| `step` | BGM/ango.ogg=0.1159, BGM/RAIN_&_Co.ogg=0.1117, DLC_data/03 2NEONS.ogg=0.1011 |
| shortest loop | BGM/Across_-Battle_Results-.ogg=14.01s, BGM/Acoustic_High_Collar.ogg=27.86s, BGM/autumncome.ogg=38.40s |

The three `SEAM_ABS`-with-`SEAM_RATIO` monsters — `nemesis_loop` (seam 0.8079, ratio 19.66),
`DLC_data/bgm01` (0.8068, 12.81) and `soul_engage_loop` (0.6304, 11.02) — are the top human-audition
targets: objective waveform discontinuity of the same order as the signal itself.

## 5. Full pinned table (accepted build)

These values are exactly the `EXPECTED` rows in `crates/wolf-audio/tests/m11_audio_loop_regression.rs`.
Region shown as LOOPSTART/LOOPLENGTH; `frames` is the decoded length.

| track | LOOPSTART | LOOPLENGTH | frames | seam_abs | step | ratio | rms_jump | dc_delta | tier | flags |
|---|---:|---:|---:|---:|---:|---:|---:|---:|:--:|---|
| `BGM/Acoustic_High_Collar.ogg` | 1360911 | 2589511 | 2589511 | 0.0195 | 0.0010 | 20.02 | 0.0830 | 0.0005 | A | SEAM_RATIO,OVERRUN,LEN_EQUALS_TRACK |
| `BGM/Across_-Battle_Results-.ogg` | 2357360 | 617983 | 2975616 | 0.0174 | 0.0241 | 0.72 | 0.2047 | 0.0129 | C | RMS_JUMP |
| `BGM/Be_yourself.ogg` | 1477927 | 2189792 | 4620348 | 0.0272 | 0.0235 | 1.16 | 0.2885 | 0.0001 | C | RMS_JUMP |
| `BGM/Die_Letzte_Revolution.ogg` | 811014 | 2613192 | 3904128 | 0.3626 | 0.0501 | 7.23 | 0.1021 | 0.0002 | A | SEAM_ABS,RMS_JUMP |
| `BGM/InstaVista.ogg` | 845923 | 2771822 | 4000896 | 0.3884 | 0.0866 | 4.49 | 0.0337 | 0.0200 | A | SEAM_ABS |
| `BGM/Killer_Roll.ogg` | 1246266 | 6135677 | 7941823 | 0.1988 | 0.0773 | 2.57 | 0.1397 | 0.0078 | C | RMS_JUMP |
| `BGM/Migratory_Birds.ogg` | 349294 | 2753501 | 4696704 | 0.1026 | 0.0272 | 3.77 | 0.0339 | 0.0030 | D | — |
| `BGM/Our_Construction.ogg` | 2806 | 4491648 | 4491648 | 0.5524 | 0.0281 | 19.66 | 0.0365 | 0.0095 | A | SEAM_ABS,SEAM_RATIO,OVERRUN,LEN_EQUALS_TRACK |
| `BGM/RAIN_&_Co.ogg` | 3737433 | 5058894 | 8896785 | 0.3204 | 0.1117 | 2.87 | 0.1809 | 0.0096 | A | SEAM_ABS,RMS_JUMP |
| `BGM/Sunshine!.ogg` | 358273 | 6417034 | 7843968 | 0.2290 | 0.0665 | 3.44 | 0.1182 | 0.0017 | A | SEAM_ABS,RMS_JUMP |
| `BGM/Tenseness.ogg` | 142664 | 4961751 | 5431680 | 0.4660 | 0.0499 | 9.34 | 0.0566 | 0.0005 | A | SEAM_ABS,SEAM_RATIO |
| `BGM/The_End_of_Destruction.ogg` | 577941 | 6335211 | 6913152 | 0.2800 | 0.0439 | 6.37 | 0.1448 | 0.0144 | A | SEAM_ABS,RMS_JUMP |
| `BGM/We_are_Going.ogg` | 799430 | 7855485 | 7855485 | 0.1707 | 0.0259 | 6.59 | 0.0769 | 0.0136 | B | OVERRUN,LEN_EQUALS_TRACK |
| `BGM/ango.ogg` | 1485817 | 8304383 | 17262720 | 0.1159 | 0.1159 | 1.00 | 0.0443 | 0.0009 | D | — |
| `BGM/autumncome.ogg` | 822589 | 1693274 | 2515863 | 0.1946 | 0.0202 | 9.66 | 0.1212 | 0.0004 | A | SEAM_RATIO,RMS_JUMP |
| `BGM/centennial_loop.ogg` | 240565 | 2886521 | 5917423 | 0.3019 | 0.0294 | 10.27 | 0.0867 | 0.0059 | A | SEAM_ABS,SEAM_RATIO |
| `BGM/destiny.ogg` | 56201 | 6943141 | 6999342 | 0.4619 | 0.0407 | 11.35 | 0.0832 | 0.0016 | A | SEAM_ABS,SEAM_RATIO |
| `BGM/line.ogg` | 2058 | 6974244 | 7099776 | 0.0064 | 0.0097 | 0.66 | 0.2517 | 0.0062 | C | RMS_JUMP |
| `BGM/madoromu2.ogg` | 709878 | 3441961 | 7596288 | 0.0516 | 0.0171 | 3.02 | 0.1145 | 0.0158 | C | RMS_JUMP |
| `BGM/morning.ogg` | 558229 | 2731328 | 3573504 | 0.0115 | 0.0025 | 4.57 | 0.1274 | 0.0001 | C | RMS_JUMP |
| `BGM/nemesis_loop.ogg` | 4061036 | 3681423 | 8110491 | 0.8079 | 0.0411 | 19.66 | 0.0278 | 0.0017 | A | SEAM_ABS,SEAM_RATIO |
| `BGM/okiraku.ogg` | 759270 | 8835347 | 9731059 | 0.0423 | 0.0319 | 1.33 | 0.1136 | 0.0037 | C | RMS_JUMP |
| `BGM/p_egg_loop.ogg` | 179398 | 2332052 | 4662341 | 0.0943 | 0.0427 | 2.21 | 0.3772 | 0.0228 | C | RMS_JUMP |
| `BGM/soul_engage_loop.ogg` | 425697 | 3877316 | 7761171 | 0.6304 | 0.0572 | 11.02 | 0.1910 | 0.0058 | A | SEAM_ABS,SEAM_RATIO,RMS_JUMP |
| `BGM/straightcontrast.ogg` | 2148554 | 3686522 | 7966080 | 0.4450 | 0.0319 | 13.97 | 0.0833 | 0.0131 | A | SEAM_ABS,SEAM_RATIO |
| `BGM/waste_loop.ogg` | 330750 | 3628812 | 7891154 | 0.2520 | 0.0455 | 5.54 | 0.0645 | 0.0027 | A | SEAM_ABS |
| `BGM/windletter.ogg` | 781072 | 5438387 | 6590592 | 0.1610 | 0.0100 | 16.16 | 0.1331 | 0.0011 | A | SEAM_RATIO,RMS_JUMP |
| `DLC_data/03 2NEONS.ogg` | 1604110 | 3953345 | 5674070 | 0.1949 | 0.1011 | 1.93 | 0.1491 | 0.0048 | C | RMS_JUMP |
| `DLC_data/bgm01.ogg` | 4569648 | 4147911 | 8779778 | 0.8068 | 0.0630 | 12.81 | 0.1574 | 0.0140 | A | SEAM_ABS,SEAM_RATIO,RMS_JUMP |
| `DLC_data/bgm02.ogg` | 3950132 | 5202580 | 9196465 | 0.0774 | 0.0257 | 3.02 | 0.3704 | 0.0110 | C | RMS_JUMP |
| `DLC_data/bgm03.ogg` | 681867 | 5021630 | 5706642 | 0.0247 | 0.0089 | 2.77 | 0.0671 | 0.0000 | D | — |

## 6. Stable listening manifest

The test prints the manifest to stdout **and** writes
`work/out/m11_audio_loop_manifest.md` (best-effort; `work/out` is gitignored). It is stable:
derived from the pinned `EXPECTED` rows, so it does not depend on filesystem order or float jitter.
Contents: tracks grouped by tier with metric values + flags, and the human audition checklist.

## 7. Human audition checklist (the part no agent can do)

| # | check | tracks |
|---|---|---|
| 1 | Loop each **Tier A** track, listen for a click/pop at the wrap | all 17 Tier A (`nemesis_loop`, `DLC_data/bgm01`, `soul_engage_loop` first) |
| 2 | Loop each **Tier C** track, listen for a loudness or DC step | all 10 Tier C (`p_egg_loop` rms 0.3772 worst) |
| 3 | For **OVERRUN / LEN_EQUALS_TRACK**, confirm loop length matches the musical phrase | `Acoustic_High_Collar`, `Our_Construction`, `We_are_Going` |
| 4 | Confirm no audible silence gap at any seam | all 31 |
| 5 | Record verdict + date in §8 | all 31 |

## 8. Human sign-off table (fill in when auditioned)

| track | tier | listened (Y/N) | verdict (clean/click/loudness/gap) | date / by |
|---|:--:|:--:|---|---|
| `BGM/Acoustic_High_Collar.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/Across_-Battle_Results-.ogg` | C | Y | clean | 2026-11-02 / 用户 |
| `BGM/Be_yourself.ogg` | C | Y | clean | 2026-11-02 / 用户 |
| `BGM/Die_Letzte_Revolution.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/InstaVista.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/Killer_Roll.ogg` | C | Y | clean | 2026-11-02 / 用户 |
| `BGM/Migratory_Birds.ogg` | D | — | 免听 | — |
| `BGM/Our_Construction.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/RAIN_&_Co.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/Sunshine!.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/Tenseness.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/The_End_of_Destruction.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/We_are_Going.ogg` | B | Y | clean | 2026-11-02 / 用户 |
| `BGM/ango.ogg` | D | — | 免听 | — |
| `BGM/autumncome.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/centennial_loop.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/destiny.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/line.ogg` | C | Y | clean | 2026-11-02 / 用户 |
| `BGM/madoromu2.ogg` | C | Y | clean | 2026-11-02 / 用户 |
| `BGM/morning.ogg` | C | Y | clean | 2026-11-02 / 用户 |
| `BGM/nemesis_loop.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/okiraku.ogg` | C | Y | clean | 2026-11-02 / 用户 |
| `BGM/p_egg_loop.ogg` | C | Y | clean | 2026-11-02 / 用户 |
| `BGM/soul_engage_loop.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/straightcontrast.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/waste_loop.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `BGM/windletter.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `DLC_data/03 2NEONS.ogg` | C | Y | clean | 2026-11-02 / 用户 |
| `DLC_data/bgm01.ogg` | A | Y | clean | 2026-11-02 / 用户 |
| `DLC_data/bgm02.ogg` | C | Y | clean | 2026-11-02 / 用户 |
| `DLC_data/bgm03.ogg` | D | — | 免听 | — |

## 9. How to update a loop baseline (when a change is intentional)

1. Change the decoder/asset.
2. Run `WOLF_AUDIO_ROOT=<root> cargo test -p wolf-audio --test m11_audio_loop_regression -- --nocapture`;
   read the failure (it names the track, metric, actual vs accepted, and the excess).
3. If the new value is a genuine improvement or an intentional asset swap, update the matching
   `Expected { … }` row in `crates/wolf-audio/tests/m11_audio_loop_regression.rs` **and** the table
   in §5 here together, and state why in the commit message.
4. If the new value is a *worsening* (bigger click / louder seam / more DC), treat it as a defect:
   fix the asset/decoder or file it, do not move the pin.
5. Never loosen a tolerance to make a real regression pass; the tolerances in §3 exist to ignore
   sub-audible float noise only.

## 10. Verification (measured)

- `cargo test -p wolf-audio --test m11_audio_loop_regression` → **1 passed, 0 failed** (two decode
  passes, determinism asserted), ~14 s.
- `cargo test -p wolf-audio` → whole crate **green** (all binaries, 0 failed).
- Negative perturbation check fails as shown in §3; revert restores green.
