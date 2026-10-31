# M10 audio residual: missing BGM/kana.mp3 and loop-point audit

Status: research note + data-driven test (shared task-72). No production decode changes.
Scope: the shipped retail asset tree under work/all (the full 1.5 GB extraction), with the
game install at E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy as
the source of truth for what actually ships.

## 1. Executive summary

1. BGM/kana.mp3 is not a lost or mis-decoded asset. It exists only as one string cell in
   BasicData/SysDatabase.dat (the type1 BGM list). No event script, map script, or other
   database references it, and no audio file named kana exists anywhere in the install or
   in the extracted tree. It is a dead registration, not a played path, so the
   "[wolf-audio] cannot play BGM/kana.mp3" warning is expected and harmless.
2. WOLF's audio runtime is already tolerant of a missing file: it logs the failure once,
   creates no voice, plays silence, and never panics. The new test pins this behaviour.
3. All 31 loop-tagged audio files are .ogg and decode with usable loop regions after the
   existing clamp (loop_end = min(LOOPSTART + LOOPLENGTH, total_frames)). Raw tags explain
   the decoded regions exactly.
4. Eighteen files carry at least one heuristic listening flag. They are candidates for a
   human ear, not proven bugs: the seam metric flags real fades/transients too, and the
   three OVERRUN rows are simply LOOPLENGTH == total frames (deliberate whole-tail clamp).
   No row is unusable, no start is out of bounds.

## 2. Missing BGM/kana.mp3

### 2.1 Search evidence

- Filesystem search of the install (find for *kana*) returned nothing. The Cosplay DLC
  folder contains only gallery images (image01..23.jpg plus thumbs and banners), no audio.
- The extracted tree work/all contains many kana-named pictures (and DLC_data kana CG) but
  zero audio file named kana.
- work/all/BGM holds 49 files (ogg and mp3); work/all/DLC_data holds 03 2NEONS.ogg,
  bgm01-04/bgm06-08.ogg, bgm09.mp3, gulp_down_water1.mp3. None is kana.mp3.
- The install Data folder ships 23 .wolf archives including BGM.wolf. A raw grep of
  BGM.wolf for kana only finds an encrypted false positive, so the archive cannot be
  string-searched directly; the authoritative check is the extraction above, which has no
  kana audio.
- grep -R -a -l -i "BGM/kana.mp3" work/all matches exactly one file:
  ./BasicData/SysDatabase.dat. CommonEvent.dat, DataBase.dat, CDataBase.dat and every
  MapData/*.mps contain zero exact occurrences (verified byte-count in the test).
- The same-name alternative search (any audio file whose name is kana.mp3, case-insensitive)
  across the whole extracted audio set is empty.

### 2.2 Why the reference is never played

The string lives in the SysDatabase type1 BGM list (row 5), recorded in
docs/ASSET_AUDIT.md:244 as registered but never verified as reached. Runtime BGM filenames
are supplied by opcode 140 (SoundCmd) event/map commands, not by walking the BGM list.
docs/AUDIO.md:123-129 and docs/BATTLE.md:703-708 note the BGM list rows leave the filename
empty (BGM番号 = -3, filename delegated to the event), and the list's loop-start column 3
(ms) fallback is not wired. Because no opcode-140 command names BGM/kana.mp3, the file is
never opened in a real playthrough. The missing-file warning only appears if some tool or
test feeds the registered name directly to the engine.

### 2.3 WOLF missing-audio behaviour (verified)

Engine path: crates/wolf-audio/src/engine.rs:218-238 apply_sound_cmd maps a BGM Play to
play_bgm_with_volume(cmd.file, true, vol); on error it calls warn_once at :246-250, which
prints exactly:

    [wolf-audio] cannot play {file}: {err}

once per file. resolve at :252-272 tries the raw path, root/file, then root/Data/file. With
no match the load error is "Audio file not found: {file} (asset root ...)" (surfaced through
AudioError::Io as "audio io error: audio file not found: BGM/kana.mp3 (asset root ...)").
No voice is registered, bgm_playing() stays false, render_offline returns digital silence,
and nothing panics.

The new test asserts all of that: decode_file(root/BGM/kana.mp3) is Err,
Engine::resolve("BGM/kana.mp3") is None, applying the SoundCmd leaves bgm_playing() false,
and 128 rendered frames are all 0.0.

## 3. Loop-point audit of all 31 loop-tagged files

### 3.1 Method

The test scans every .ogg/.mp3/.wav for case-insensitive LOOPSTART / LOOPLENGTH comment
bytes, decodes only the tagged files, and compares raw tags against the region the decoder
keeps. Decode rule (crates/wolf-audio/src/decode.rs:200-214, tags read at :229-262):
loop_start = LOOPSTART; loop_end = min(LOOPSTART + LOOPLENGTH, total_frames), or
total_frames when no length; a loop is stored only when loop_end > loop_start and
loop_start < total_frames.

Audible-seam heuristic: seam = max over channels of |sample(loop_end - 1) - sample(loop_start)|,
i.e. the jump the mixer makes when it wraps. It is normalised by step, the mean absolute
sample-to-sample step in 1024-frame windows at the loop head and tail. Flags:
SEAM_ABS seam >= 0.20; SEAM_RATIO seam/step >= 8.0; OVERRUN LOOPSTART+LOOPLENGTH > total
frames; LEN_EQUALS_TRACK LOOPLENGTH == total frames; START_OOB LOOPSTART >= total frames;
SHORT_LOOP loop under 1.0 s. These are listening heuristics, not decode errors: a genuine
fade or transient at the boundary can raise SEAM_ABS, and the existing clamp intentionally
truncates an overrunning loop to the file end (docs/AUDIO.md:110-121).

### 3.2 Full table (all ogg, all decoded, all raw values consistent)

Columns: file | rate | ch | frames | LOOPSTART | LOOPLENGTH | decoded region | loop_s | seam | step | ratio | flags

| BGM/Acoustic_High_Collar.ogg | 44100 | 2 | 2589511 | 1360911 | 2589511 | 1360911..2589511 | 27.859 | 0.020 | 0.0010 | 20.0 | SEAM_RATIO,OVERRUN,LEN_EQUALS_TRACK |
| BGM/Across_-Battle_Results-.ogg | 44100 | 2 | 2975616 | 2357360 | 617983 | 2357360..2975343 | 14.013 | 0.017 | 0.0241 | 0.7 | - |
| BGM/Be_yourself.ogg | 44100 | 2 | 4620348 | 1477927 | 2189792 | 1477927..3667719 | 49.655 | 0.027 | 0.0235 | 1.2 | - |
| BGM/Die_Letzte_Revolution.ogg | 44100 | 2 | 3904128 | 811014 | 2613192 | 811014..3424206 | 59.256 | 0.363 | 0.0501 | 7.2 | SEAM_ABS |
| BGM/InstaVista.ogg | 44100 | 2 | 4000896 | 845923 | 2771822 | 845923..3617745 | 62.853 | 0.388 | 0.0866 | 4.5 | SEAM_ABS |
| BGM/Killer_Roll.ogg | 44100 | 2 | 7941823 | 1246266 | 6135677 | 1246266..7381943 | 139.131 | 0.199 | 0.0773 | 2.6 | - |
| BGM/Migratory_Birds.ogg | 44100 | 2 | 4696704 | 349294 | 2753501 | 349294..3102795 | 62.438 | 0.103 | 0.0272 | 3.8 | - |
| BGM/Our_Construction.ogg | 44100 | 2 | 4491648 | 2806 | 4491648 | 2806..4491648 | 101.788 | 0.552 | 0.0281 | 19.7 | SEAM_ABS,SEAM_RATIO,OVERRUN,LEN_EQUALS_TRACK |
| BGM/RAIN_&_Co.ogg | 44100 | 2 | 8896785 | 3737433 | 5058894 | 3737433..8796327 | 114.714 | 0.320 | 0.1117 | 2.9 | SEAM_ABS |
| BGM/Sunshine!.ogg | 44100 | 2 | 7843968 | 358273 | 6417034 | 358273..6775307 | 145.511 | 0.229 | 0.0665 | 3.4 | SEAM_ABS |
| BGM/Tenseness.ogg | 48000 | 2 | 5431680 | 142664 | 4961751 | 142664..5104415 | 103.370 | 0.466 | 0.0499 | 9.3 | SEAM_ABS,SEAM_RATIO |
| BGM/The_End_of_Destruction.ogg | 44100 | 2 | 6913152 | 577941 | 6335211 | 577941..6913152 | 143.656 | 0.280 | 0.0439 | 6.4 | SEAM_ABS |
| BGM/We_are_Going.ogg | 44100 | 2 | 7855485 | 799430 | 7855485 | 799430..7855485 | 160.001 | 0.171 | 0.0259 | 6.6 | OVERRUN,LEN_EQUALS_TRACK |
| BGM/ango.ogg | 44100 | 2 | 17262720 | 1485817 | 8304383 | 1485817..9790200 | 188.308 | 0.116 | 0.1159 | 1.0 | - |
| BGM/autumncome.ogg | 44100 | 2 | 2515863 | 822589 | 1693274 | 822589..2515863 | 38.396 | 0.195 | 0.0202 | 9.7 | SEAM_RATIO |
| BGM/centennial_loop.ogg | 44100 | 2 | 5917423 | 240565 | 2886521 | 240565..3127086 | 65.454 | 0.302 | 0.0294 | 10.3 | SEAM_ABS,SEAM_RATIO |
| BGM/destiny.ogg | 44100 | 2 | 6999342 | 56201 | 6943141 | 56201..6999342 | 157.441 | 0.462 | 0.0407 | 11.3 | SEAM_ABS,SEAM_RATIO |
| BGM/line.ogg | 44100 | 2 | 7099776 | 2058 | 6974244 | 2058..6976302 | 158.146 | 0.006 | 0.0097 | 0.7 | - |
| BGM/madoromu2.ogg | 44100 | 2 | 7596288 | 709878 | 3441961 | 709878..4151839 | 78.049 | 0.052 | 0.0171 | 3.0 | - |
| BGM/morning.ogg | 44100 | 2 | 3573504 | 558229 | 2731328 | 558229..3289557 | 61.935 | 0.011 | 0.0025 | 4.6 | - |
| BGM/nemesis_loop.ogg | 44100 | 2 | 8110491 | 4061036 | 3681423 | 4061036..7742459 | 83.479 | 0.808 | 0.0411 | 19.7 | SEAM_ABS,SEAM_RATIO |
| BGM/okiraku.ogg | 44100 | 2 | 9731059 | 759270 | 8835347 | 759270..9594617 | 200.348 | 0.042 | 0.0319 | 1.3 | - |
| BGM/p_egg_loop.ogg | 44100 | 2 | 4662341 | 179398 | 2332052 | 179398..2511450 | 52.881 | 0.094 | 0.0427 | 2.2 | - |
| BGM/soul_engage_loop.ogg | 44100 | 2 | 7761171 | 425697 | 3877316 | 425697..4303013 | 87.921 | 0.630 | 0.0572 | 11.0 | SEAM_ABS,SEAM_RATIO |
| BGM/straightcontrast.ogg | 48000 | 2 | 7966080 | 2148554 | 3686522 | 2148554..5835076 | 76.803 | 0.445 | 0.0319 | 14.0 | SEAM_ABS,SEAM_RATIO |
| BGM/waste_loop.ogg | 44100 | 2 | 7891154 | 330750 | 3628812 | 330750..3959562 | 82.286 | 0.252 | 0.0455 | 5.5 | SEAM_ABS |
| BGM/windletter.ogg | 44100 | 2 | 6590592 | 781072 | 5438387 | 781072..6219459 | 123.319 | 0.161 | 0.0100 | 16.2 | SEAM_RATIO |
| DLC_data/03 2NEONS.ogg | 44100 | 2 | 5674070 | 1604110 | 3953345 | 1604110..5557455 | 89.645 | 0.195 | 0.1011 | 1.9 | - |
| DLC_data/bgm01.ogg | 48000 | 2 | 8779778 | 4569648 | 4147911 | 4569648..8717559 | 86.415 | 0.807 | 0.0630 | 12.8 | SEAM_ABS,SEAM_RATIO |
| DLC_data/bgm02.ogg | 48000 | 2 | 9196465 | 3950132 | 5202580 | 3950132..9152712 | 108.387 | 0.077 | 0.0257 | 3.0 | - |
| DLC_data/bgm03.ogg | 48000 | 2 | 5706642 | 681867 | 5021630 | 681867..5703497 | 104.617 | 0.025 | 0.0089 | 2.8 | - |

### 3.3 Manual listening list (18 flagged)

Seam-dominated (highest first; seam/step ratio in parentheses):
- BGM/nemesis_loop.ogg  0.808 (19.7)
- DLC_data/bgm01.ogg    0.807 (12.8)
- BGM/soul_engage_loop.ogg 0.630 (11.0)
- BGM/Our_Construction.ogg 0.552 (19.7)
- BGM/Tenseness.ogg     0.466 (9.3)
- BGM/destiny.ogg       0.462 (11.3)
- BGM/straightcontrast.ogg 0.445 (14.0)
- BGM/InstaVista.ogg    0.388 (4.5)
- BGM/Die_Letzte_Revolution.ogg 0.363 (7.2)
- BGM/RAIN_&_Co.ogg     0.320 (2.9)
- BGM/centennial_loop.ogg 0.302 (10.3)
- BGM/The_End_of_Destruction.ogg 0.280 (6.4)
- BGM/waste_loop.ogg    0.252 (5.5)
- BGM/Sunshine!.ogg     0.229 (3.4)

Ratio-only (small absolute seam but abrupt relative to local motion):
- BGM/Acoustic_High_Collar.ogg (20.0)
- BGM/autumncome.ogg (9.7)
- BGM/windletter.ogg (16.2)

Overrun / whole-track tail (clamped, no hard failure):
- BGM/Acoustic_High_Collar.ogg: LOOPSTART 1360911 + LOOPLENGTH 2589511 > frames 2589511, LOOPLENGTH == frames.
- BGM/Our_Construction.ogg: LOOPSTART 2806 + LOOPLENGTH 4491648 > frames, LOOPLENGTH == frames.
- BGM/We_are_Going.ogg: LOOPSTART 799430 + LOOPLENGTH 7855485 > frames, LOOPLENGTH == frames.

Interpretation: the overrun rows are authored as "loop to end of file" (LOOPLENGTH set to the
whole track length); the decoder's min() clamp already handles them. The high-seam rows are
most likely musical fades or transients at the loop boundary and need an ear to confirm; they
do not indicate a decode bug. No file has an out-of-bounds loop start or an unusable loop.

## 4. How to reproduce

From the repo root:

    cargo test -q -p wolf-audio --test m10_audio_residual -- --nocapture

Observed 2025 run (HEAD 22148d8): 2 tests, 2 passed, 0 failed, finished in 16.67 s. stdout
prints the full 31-row table and the 18 flagged rows; the only stderr line is the expected
missing-file warning for BGM/kana.mp3. Set WOLF_AUDIO_ROOT to point at a different extracted
tree (the test skips gracefully when no tree is found).

## 5. Residual gaps / what was not done

- No actual listening pass was performed. The seam flags are a numeric heuristic and are
  deliberately reported as a listening list, not as confirmed defects.
- The SysDatabase BGM-list loop-start column 3 (ms) fallback remains unwired; current loop
  behaviour comes only from Vorbis LOOPSTART/LOOPLENGTH tags. If the retail engine used the
  list column for any track, that path is still unproven.
- No mp3 in the tree carries loop tags (all 31 are ogg), so mp3 loop handling is untested
  against real data.
- kana.mp3 remains genuinely absent; this note documents that it is a dead registration
  rather than silently masking the warning.
- Production decode was not modified. This task added only the test file
  crates/wolf-audio/tests/m10_audio_residual.rs and this document.
