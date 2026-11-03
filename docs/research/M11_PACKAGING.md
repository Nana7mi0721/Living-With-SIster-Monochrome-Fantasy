# M11-PKG: data-root validation, packaging, frame budget, release perf

Status: implementation and black-box tests landed for task-100 (M11-PKG), then
extended by the P2 release round on 2026-11-03. The new code is a standalone
packaging binary plus reusable runtime checks. Game logic is unchanged except for
startup validation, asset-cache statistics, and the play-loop timing output.

## Deliverables

| Path | Purpose |
| --- | --- |
| crates/wolf-cli/src/bin/wolf-package.rs | Standalone validate / assemble / check / bench CLI. 1280 lines. |
| crates/wolf-cli/tests/m11_package.rs | Black-box tests that spawn the real binary and build a synthetic root. 299 lines. |
| crates/wolf-game/src/data_root.rs | Startup data-root layout check (REQUIRED_DIRS / REQUIRED_FILES), used by Game::new. 151 lines. |
| crates/wolf-game/src/frame_budget.rs | Reusable p50/p95/max frame timing around a live Game. 110 lines. |
| crates/wolf-game/src/assets.rs | Asset cache; P2 adds cache_bytes() / cache_stats(). |
| crates/wolf-cli/src/play.rs | Release play loop; P2 adds startup breakdown and per-frame perf summary. 580 lines. |
| docs/research/M11_PACKAGING.md | This document. |

The packaging binary deliberately does not touch crates/wolf-cli/src/main.rs.
The game module does not read frame_budget; callers opt in by passing a Game.

## Command line

    wolf-package validate [--root PATH]
    wolf-package assemble --root PATH --exe PATH --out DIR [--runtime FILE ...] [--font FILE ...]
    wolf-package check --package DIR
    wolf-package --check DIR
    wolf-package bench [--root PATH] [--save FILE] [--event ID] [--frames N] [--warmup N]

PATH defaults to WOLF_DATA_ROOT, then work/all. The bench defaults are 600 measured
frames after 60 warm-up frames, event 188, with auto-advance and auto-choice on.
main/usage are at crates/wolf-cli/src/bin/wolf-package.rs:47 and :57.

### validate

Required top-level directories (crates/wolf-cli/src/bin/wolf-package.rs:30):

    BasicData, MapData, SystemFile

Required files (lines 31-40):

    BasicData/Game.dat
    BasicData/CommonEvent.dat
    BasicData/CDataBase.dat
    BasicData/DataBase.dat
    BasicData/SysDatabase.dat
    BasicData/SysDataBaseBasic.dat
    BasicData/TileSetData.dat
    BasicData/MapTree.dat

run_validate is at line 188; validate_root is at line 203. It checks directory
presence and non-emptiness, file presence and non-zero size, counts the whole tree,
and lists unknown top-level names. Missing required entries or zero-byte required
entries make the command fail (non-zero exit). An extra top-level entry is reported
but is not fatal on its own. The output printer is print_validation at line 324.

Measured against the real WOLF root (release, 2026-11-03):

    wolf-package validate --root work/all

    wolf-package validate: root = work/all
      required dirs : 3/3 present
      required files: 8/8 present
      data          : 4065 files, 772716836 bytes
      extra top-level: none
    VALID: data root is complete (4065 files, 772716836 bytes)

The count is smaller than the raw directory because tree_stats skips .wolf source
archives; those are build inputs, not runtime payload.

### assemble

run_assemble is at line 362. It refuses an invalid root, refuses an output path
inside the data root, and refuses a non-empty output directory. It copies the
executable to out/wolf-cli.exe, copies each --runtime beside it (runtime DLLs are
never guessed), recursively copies the data root to out/data (skipping .wolf), copies
any --font files to out/data/Fonts/, then writes out/manifest.json. copy_tree is at
line 503; collect_manifest_entries starts at line 610. Output line:

    ASSEMBLED: <n> files, <bytes> bytes, output = <dir>

Explicit --font paths win. When omitted, resolve_fonts (line 474) mirrors the four
retail fonts from WOLF_GAME_DIR into data/Fonts/: Lolita.ttf, 07yasashi.ttf,
GenJyuuGothicXPB.ttf, MSGOTHIC.TTF. A missing font is reported but is not fatal.

Real release assembly (2026-11-03):

    WOLF_GAME_DIR='E:/SteamLibrary/steamapps/common/Living With SIster Monochrome Fantasy' \
      wolf-package assemble --root work/all --exe target/release/wolf-cli.exe --out work/out/package_p2_release

    required dirs : 3/3 present
    required files: 8/8 present
    data          : 4065 files, 772716836 bytes
    font: .../Lolita.ttf -> work/out/package_p2_release/data/Fonts/Lolita.ttf
    font: .../07yasashi.ttf -> ...
    font: .../GenJyuuGothicXPB.ttf -> ...
    font: .../MSGOTHIC.TTF -> ...
    ASSEMBLED: 4070 files, 814703960 bytes, output = work/out/package_p2_release

### manifest.json

The manifest object is defined at line 532; Manifest::to_json starts at line 544.
Its shape is stable and flat:

    {
      "format": 1,
      "tool": "wolf-package",
      "data_root": "data",
      "file_count": 123,
      "total_bytes": 456,
      "files": [
        {"path": "wolf-cli.exe", "bytes": 12, "sha256": "..."},
        {"path": "data/BasicData/Game.dat", "bytes": 34, "sha256": "..."}
      ]
    }

Paths are package-relative and slash-separated. manifest.json itself is not listed
in the file list; walk_files skips it at line 625, so check does not report it as an
extra file.

### check / --check

run_check starts at line 669. For each manifest entry it checks presence, then byte
size, then SHA-256. A size difference is reported as a fast mismatch; a same-size
corruption is reported as a sha256 mismatch. It then walks the package and reports
missing, mismatched, and extra files, and validates package/data as a data root.
Success line:

    PACKAGE OK: <n> files verified, <bytes> bytes

--check DIR is accepted as an alias for check --package DIR.

**P2 bug fixed (2026-11-03):** the hand-written JSON string parser at
crates/wolf-cli/src/bin/wolf-package.rs:979 used to push every raw byte as a
Latin-1 char (`byte as char`). UTF-8 paths therefore came back as mojibake, and a
real package reported 30 Japanese/Chinese files as both MISSING and EXTRA. The
parser now accumulates raw bytes and decodes each run with std::str::from_utf8,
flushing before JSON escapes. Regression coverage:
- manifest_json_round_trips_non_ascii_paths (unit test in the binary)
- assemble_and_check_round_trip_the_manifest now includes
  data/SystemFile/くしゃくしゃ.png and asserts it survives the manifest and PACKAGE OK.

Real package check after the fix:

    wolf-package check --package work/out/package_p2_release
      manifest entries: 4070
      extra package files: none
    PACKAGE OK: 4070 files verified, 814703960 bytes

### bench

run_bench starts at line 751. It constructs Game::new(root), optionally load_save,
sets auto_advance / auto_choice / auto_choice_delay, optionally starts an event,
runs warm-up frames, then calls wolf_game::frame_budget::measure_game. Release-profile
measurement on work/all (2026-11-03):

    wolf-package bench --root work/all --save work/retail_backup/Save/SaveData01.sav --event 188 --frames 600 --warmup 60

    frame-budget: frames=600 p50_us=1.5 p95_us=2.8 max_us=86.4 mean_us=2.7 total_us=1598.5
      root=work/all save=... event=188 warmup=60 warnings=0

Frame time is well under one millisecond at p50, so the game loop is not a release
bottleneck. Numbers are machine-specific.

## Startup validation and asset-cache stats

crates/wolf-game/src/data_root.rs: REQUIRED_DIRS=[BasicData, MapData, SystemFile],
REQUIRED_FILES (same eight .dat files), DataRootError{root, problems}, and
check_layout(&Path) -> Result<(), DataRootError>. It accepts either a BasicData/
layout or a flat layout, and reports:

    data root ... is not a playable WOLF root: ...; expected BasicData/, MapData/, SystemFile/ (pass --root or set WOLF_DATA_ROOT)

Game::new calls check_layout immediately after resolving the root and maps failures
to GameError::DataRoot. This turns a wrong WOLF_DATA_ROOT into one actionable message
instead of a cascade of missing-file errors.

crates/wolf-game/src/assets.rs exposes cache_bytes() -> u64 and
cache_stats() -> (usize, u64). Game::asset_cache_stats() forwards them. play prints
the cache after exit (assets N image(s), X MiB) as part of the perf line.

## Release startup measurement

play prints a startup breakdown:

    startup: data loaded in <ms> ms
    startup: window created in <ms> ms
    startup: renderer ready in <ms> ms
    startup: assets uploaded in <ms> ms
    startup: font loaded in <ms> ms
    startup: window ready in <ms> ms
    perf: total ... ms | startup window ... ms | step p50 ... p95 ... max ... mean ... | assets N image(s), X MiB

Packaged build, warm runs (work/out/package_p2_release, RTX 5060 Laptop, Vulkan):

    data loaded  97-114 ms
    window       147-181 ms
    renderer     831-1080 ms (first run after copy 1080; warm 831-836)
    window ready 872-1124 ms (warm 872-877)

Window visible timing was measured externally with EnumWindows per PID
(work/out/enum_windows.ps1): retail Game.exe shows its first window around
165-480 ms with the title "Starting up, initializing system"; our game window
"imoto-life | title screen" is visible around 251 ms. Retail keeps that title and
we could not observe its true first rendered frame, so the honest claim is: window
appearance is the same order of magnitude, while our first frame is ~0.87-0.9 s
because renderer setup waits on adapter enumeration and swapchain configure.

A temporary WOLF_PROFILE_STARTUP instrumentation measured Renderer::new as surface
~39 ms, request_adapter ~276 ms, request_device ~56 ms, surface.configure ~192 ms,
build ~1 ms (~700 ms total). It was removed; renderer.rs is clean. No low-risk
optimization was found in this round.

## frame_budget module

crates/wolf-game/src/frame_budget.rs exposes:

- FrameBudgetReport at line 15 with frames, total_us, mean_us, p50_us, p95_us, max_us.
- FrameBudgetReport::from_nanos at line 33, nearest-rank percentiles.
- FrameBudgetReport::summary_line at line 52, one parse-friendly line.
- measure<F: FnMut()> at line 71.
- measure_game(game: &mut Game, frames) at line 83.

Warm-up is the caller's responsibility, so the reported frame count is exactly the
measured count.

## Tests

    cargo test -p wolf-cli --bin wolf-package --test m11_package
    -> 3 unit tests passed; 4 packaging tests passed

Full workspace gate after the P2 fix (2026-11-03):

    WOLF_GAME_DIR=work/retail_backup WOLF_DATA_ROOT=work/all cargo test -q --workspace --no-fail-fast
    -> 146 test-result blocks, 878 passed, 0 failed, 14 ignored, EXIT 0

    ./target/debug/wolf-cli.exe verify --root work/all
    -> PASS — 70 check(s) matched baseline, 0 pending, EXIT 0

The four packaging tests in crates/wolf-cli/tests/m11_package.rs are:

- validate_accepts_the_real_work_all_root (line 126): spawns validate against work/all
  and asserts 3/3 dirs, 8/8 files, no extra top-level, and VALID.
- validate_rejects_a_broken_root_and_lists_missing_and_extra (line 142): synthetic
  root missing BasicData/CommonEvent.dat plus unexpected.txt; asserts non-zero exit,
  MISSING, and EXTRA top-level.
- assemble_and_check_round_trip_the_manifest (line 161): synthetic root including a
  non-ASCII Japanese file, synthetic exe/runtime/font; asserts manifest contains
  sha256/bytes, staged files, and data/SystemFile/くしゃくしゃ.png; check succeeds;
  a second assembly produces a byte-identical manifest; a same-size first-byte
  corruption is reported as sha256 mismatch; deleting one packaged file is reported
  as MISSING.
- frame_budget_benchmarks_a_real_game_loop (line 263): runs bench for 60 frames on
  work/all and asserts frame-budget frames=60 and p50 <= p95 <= max.

The test helper TempDir uses work/out, not the Windows system temp directory, because
the latter produced PermissionDenied (os error 5) in this environment.

## Packaged smoke test

After assembly, running the packaged executable from the package directory:

    ./wolf-cli.exe play --root data --boot --until 300

    startup: data loaded in 115 ms
    startup: window created in 177 ms
    startup: renderer ready in 1131 ms
    startup: window ready in 1179 ms
    exit: 300 frame(s), 9392 instruction(s), 10 picture op(s), 0 warning(s)
    perf: total 6244 ms | startup window 1179 ms | step p50 0.67 ms, p95 1.17 ms, max 18.18 ms, mean 0.89 ms | assets 19 image(s), 7.0 MiB

The package self-contains its fonts under data/Fonts/ and needs no WOLF_GAME_DIR.

## Limitations

- The validator encodes one known retail layout: required files are looked up under
  BasicData/, with MapData/ and SystemFile/ as required directories. A different
  packaging layout would need its required-file table extended.
- Extra top-level entries are reported but are not fatal; this keeps unpacked DLC
  or extra asset folders from blocking a legitimate root. Missing or zero-byte
  required files remain fatal.
- assemble copies the data tree with no compression, delta encoding, signing, or
  installer. --runtime must list every needed DLL explicitly.
- The manifest has only format version 1. There is no migration logic yet.
- SHA-256 is implemented locally (crates/wolf-cli/src/bin/wolf-package.rs) to avoid
  a new dependency; known-vector tests cover empty and abc.
- Startup to first frame is ~0.87-0.9 s warm on this machine, dominated by wgpu
  adapter/swapchain setup; window visibility is ~150-250 ms. This is documented as
  measured, not claimed as faster than retail.
