# M9 — Audio smoke: inventory + mixer regression (shared task-51)

Status: automated slice landed; **no production code changed, nothing committed.** The
device-free half of audio QA is now covered by one new test file; the subjective/device half is
listed under "Remaining human checks".

## What runs

- New test: `crates/wolf-audio/tests/audio_inventory.rs` (7 tests).
- `cargo test -q -p wolf-audio` -> 9 (existing `tests/audio.rs`) + 7 (new) = **16 passed, 0
  failed** (~15 s, dominated by decoding every asset).
- Focused: `cargo test -q -p wolf-audio --test audio_inventory -- --nocapture` -> 7 passed.

Asset root is `work/all` located relative to the crate; the inventory test skips (with a note)
if absent, matching the existing tests.

## Inventory (exact, full decode of work/all)

| metric | value |
|---|---|
| audio files discovered | **358** |
| decoded OK | **357** |
| decode failures | **1** (see Findings) |
| total decoded duration | **8281.43 s** (~2 h 18 m 1.4 s) |
| by directory | BGM 49, SE 273, DLC_data 10, SystemFile 13, SystemGraphic 13 |
| by extension | mp3 172, ogg 163, wav 23 |
| loop-tagged files | **31** (27 BGM + 4 DLC_data; all `.ogg`; none `.mp3`) |
| digital-silent files | **0** |
| sample rates seen | 12000, 22050, 24000, 44100, 48000 Hz |
| channel counts seen | 1, 2 |
| duration range | 0.032 s ... 391.445 s |
| min peak / min RMS (all files) | 0.02923 / 0.004245 |

The extension totals (172/163/23) match the whole-package figure in `docs/AUDIO.md` section 2
exactly, which is a completeness check: every audio file in the package was reached.

Note: `docs/AUDIO.md` section 2 calls BGM "almost all .ogg, a few .mp3"; the real split is
**20 mp3 / 29 ogg** (49 total). Its SE (~273) and whole-package extension counts were exact.

### Findings / defects surfaced

1. **`SE/（Action）跳ね.wav` fails to decode** -
   `unsupported audio: probe: malformed stream: wav: malformed fmt_pcm chunk`.
   The file is a valid RIFF/WAVE with PCM data (`audioFormat=1`, mono, 44100 Hz, 16-bit,
   blockAlign=2), but its `fmt ` chunk is **26 bytes** instead of the standard 16/18 (10
   trailing bytes after the 16 PCM fields). Symphonia 0.6 correctly rejects the non-standard fmt
   size; the 73648 data bytes follow normally. This is a retail-asset defect (or an asset a more
   tolerant parser would accept), not a decoder regression. Fixing it means re-exporting the
   asset or adding a tolerant WAV path in `decode.rs` (a **production change**, out of scope).
   The inventory test records it as a documented known-bad decode and asserts **no other** file
   fails.
2. **`BGM/kana.mp3` is referenced by the game but absent** - the only missing path among the
   185 unique audio references mined from the data (below); already known in
   `docs/ASSET_AUDIT.md`.
3. **Loop tags** - 27 of the 49 BGM and 4 of the 10 DLC tracks carry LOOPSTART/LOOPLENGTH. Every
   one yields a usable in-range loop (`loop_end > loop_start`, `loop_start < frames`) after the
   `loop_end = min(start+len, total)` clamp, and no `.mp3` carries loop tags. (Doc section 4
   estimated "26 tagged BGM"; observed 27 BGM + 4 DLC = 31.)

### SoundCmd reference coverage (what the runtime can actually ask for)

The test mines raw bytes of `work/all/MapData/*.mps` and
`work/all/BasicData/{CommonEvent,SysDatabase,DataBase,CDataBase}.dat` for
`BGM/... / SE/... / DLC_data/... / SystemFile/... / SystemGraphic/...` audio paths (the strings
opcode 140 can hand to the engine), dedupes, and resolves each like `Engine::resolve` (path,
then `asset_root/...`, then `asset_root/Data/...`):

| prefix | unique refs |
|---|---|
| BGM/ | 41 |
| SE/ | 121 |
| DLC_data/ | 10 |
| SystemFile/ | 13 |
| SystemGraphic/ | 0 |
| **total** | **185** (184 resolve, 1 missing = `BGM/kana.mp3`) |

## Mixer regression (offline `render_offline`, no device)

| test | asserts |
|---|---|
| `engine_bgm_loop_wrap_is_sample_continuous` | `[2,4)` loop wraps 0,0.1,0.2,0.3,0.2,0.3 and keeps playing |
| `engine_bgm_stop_with_fade_ends_silent` | 60-frame fade reaches ~0 and stops BGM |
| `engine_stop_bgm_zero_is_immediate` | `stop_bgm(0)` stops at once and renders silence |
| `engine_se_voices_overlap_and_sum` | two SE voices (0.2+0.3) coexist and sum to 0.5 |
| `engine_volume_scaling_halves_amplitude` | volume 1.0 vs 0.5 -> RMS ratio 2.0 |
| `engine_clipping_is_sane` | 0.8+0.8 clamps to <=1.0 and reaches full scale |
| `audio_inventory` | full asset decode + SoundCmd reference coverage (above) |

## Exact commands / results

`@
$ cargo test -q -p wolf-audio
running 0 tests ... ok. 0 passed
running 9 tests ... ok. 9 passed; 0 failed
running 7 tests ... ok. 7 passed; 0 failed   (audio_inventory, ~15 s)
running 0 tests ... ok. 0 passed
`@

## Remaining human-only checks (cannot be automated here)

- **Subjective mix / timbre** - is the BGM/SE balance pleasant, are instruments/voices clear, do
  volume levels feel right in-scene. Needs a human ear.
- **Real device output** - run with `--audio` on hardware with a working output device; confirm
  the cpal backend opens and nothing distorts. CI adds only the silent null backend.
- **Loop seam by ear** - sample-continuity is proven numerically, but confirm no audible
  click/breath at loop points on the real tracks (esp. the "over"-length files in AUDIO.md
  section 4).
- **Malformed WAV decision** - decide whether `SE/（Action）跳ね.wav` should be re-exported,
  replaced, or handled by a tolerant WAV parser (production change). Until then that one SE is
  silent.
- **Missing `BGM/kana.mp3`** - confirm it is genuinely unused / acceptable, or source the file.
- **Loop metadata correctness** - verify the 31 tagged files' LOOPSTART/LOOPLENGTH are musically
  right (the test checks they are structurally valid, not artistically correct).
- **Unreferenced assets** - `SystemGraphic/` and `SystemFile/` SE plus many `SE/`/`BGM/`
  files exist but are not referenced by the scanned data (they may be used via UI conventions or
  the `.wolf` archives); spot-check that expected cues play in-game.

---
Produced by task-51 audio inventory/regression. Files touched: this doc +
`crates/wolf-audio/tests/audio_inventory.rs`. No production edits; HEAD `bcbcfba` unchanged;
not committed.
