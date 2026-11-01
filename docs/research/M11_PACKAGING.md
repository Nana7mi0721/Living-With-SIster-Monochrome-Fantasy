# M11-PKG: data-root validation, packaging, frame budget

Status: implementation and black-box tests landed for task-100 (M11-PKG).
No production entry point or game logic was changed. The new code is a standalone
binary plus a reusable timing module and a black-box test file.

## Deliverables

| Path | Purpose |
| --- | --- |
| crates/wolf-cli/src/bin/wolf-package.rs | Standalone validate / assemble / check / bench CLI. 1174 lines. |
| crates/wolf-cli/tests/m11_package.rs | Black-box tests that spawn the real binary and build a synthetic root. 278 lines. |
| crates/wolf-game/src/frame_budget.rs | Reusable p50/p95/max frame timing around a live Game. 110 lines. |
| crates/wolf-game/src/lib.rs | One module export: pub mod frame_budget; (line 29). |
| docs/research/M11_PACKAGING.md | This document. |

The binary deliberately does not touch crates/wolf-cli/src/main.rs. The game
module does not read frame_budget; callers opt in by passing a Game.

## Command line

    wolf-package validate [--root PATH]
    wolf-package assemble --root PATH --exe PATH --out DIR [--runtime FILE ...]
    wolf-package check --package DIR
    wolf-package --check DIR
    wolf-package bench [--root PATH] [--save FILE] [--event ID] [--frames N] [--warmup N]

PATH defaults to WOLF_DATA_ROOT, then work/all. The bench defaults are 600 measured
frames after 60 warm-up frames, event 188, with auto-advance and auto-choice on.
The implementation starts at crates/wolf-cli/src/bin/wolf-package.rs:39 (main),
with argument parsing and the usage text at lines 39-154.

### validate

The required top-level directories are defined at
crates/wolf-cli/src/bin/wolf-package.rs:24:

    BasicData, MapData, SystemFile

The required files are defined at lines 25-34:

    BasicData/Game.dat
    BasicData/CommonEvent.dat
    BasicData/CDataBase.dat
    BasicData/DataBase.dat
    BasicData/SysDatabase.dat
    BasicData/SysDataBaseBasic.dat
    BasicData/TileSetData.dat
    BasicData/MapTree.dat

run_validate is at line 174; validate_root is at line 189. It checks directory
presence and non-emptiness, file presence and non-zero size, counts the whole tree,
and lists unknown top-level names. Missing required entries or zero-byte required
entries make the command fail (non-zero exit). An extra top-level entry is reported
but is not fatal on its own. The output printer is print_validation at line 305.

Measured against the real WOLF root:

    cargo run -q -p wolf-cli --bin wolf-package -- validate --root work/all

    wolf-package validate: root = work/all
      required dirs : 3/3 present
      required files: 8/8 present
      data          : 4087 files, 1529396038 bytes
      extra top-level: none
    VALID: data root is complete (4087 files, 1529396038 bytes)

Exit code was 0.

### assemble

run_assemble is at line 343. It refuses an invalid root, refuses an output path
inside the data root, and refuses a non-empty output directory. It copies the
executable to out/wolf-cli.exe, copies each --runtime beside it (runtime DLLs are
never guessed), recursively copies the entire data root to out/data, then writes
out/manifest.json. copy_tree is at line 436; collect_manifest_entries starts at
line 540. Output line:

    ASSEMBLED: <n> files, <bytes> bytes, output = <dir>

### manifest.json

The manifest object is defined at line 462; Manifest::to_json starts at line 474.
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
in the file list; walk_files explicitly skips it at line 572, so check does not
report it as an extra file.

### check / --check

run_check starts at line 599. For each manifest entry it checks presence, then byte
size, then SHA-256. A size difference is reported as a fast mismatch; a same-size
corruption is reported as a sha256 mismatch. It then walks the package and reports
missing, mismatched, and extra files, and validates package/data as a data root.
Success line:

    PACKAGE OK: <n> files verified, <bytes> bytes

--check DIR is accepted as an alias for check --package DIR (lines 71-76).

### bench

run_bench starts at line 681. It constructs Game::new(root), optionally load_save,
sets auto_advance / auto_choice / auto_choice_delay, optionally starts an event,
runs warm-up frames, then calls wolf_game::frame_budget::measure_game. Measured
600-frame run (debug profile):

    cargo run -q -p wolf-cli --bin wolf-package -- bench --root work/all --save work/retail_backup/Save/SaveData01.sav --event 188 --frames 600 --warmup 60

    frame-budget: frames=600 p50_us=5.6 p95_us=7.5 max_us=102.6 mean_us=6.0 total_us=3625.1
      root=work/all save=work/retail_backup/Save/SaveData01.sav event=188 warmup=60 warnings=0

The numbers above are debug-profile and machine-specific. A release-profile run was
attempted, but release compilation exceeded the outer 120 s command budget before a
measurement was produced. Run the same command with --release to obtain opt-level
numbers; this document intentionally reports the verified debug result rather than
an unmeasured release claim.

## frame_budget module

crates/wolf-game/src/frame_budget.rs exposes:

- FrameBudgetReport at line 15 with frames, total_us, mean_us, p50_us, p95_us, max_us.
- FrameBudgetReport::from_nanos at line 33, nearest-rank percentiles.
- FrameBudgetReport::summary_line at line 52, one parse-friendly line.
- measure<F: FnMut()> at line 71.
- measure_game(game: &mut Game, frames) at line 83.

Warm-up is the caller's responsibility, so the reported frame count is exactly the
measured count. The module is exported from crates/wolf-game/src/lib.rs:29.

## Tests

    cargo test -q -p wolf-game frame_budget
    -> 2 passed, 0 failed

    cargo test -q -p wolf-cli --test m11_package -- --nocapture
    -> 4 passed, 0 failed, finished in 0.22s

The four packaging tests in crates/wolf-cli/tests/m11_package.rs are:

- validate_accepts_the_real_work_all_root (line 120): spawns validate against work/all
  and asserts 3/3 dirs, 8/8 files, no extra top-level, and VALID.
- validate_rejects_a_broken_root_and_lists_missing_and_extra (line 136): synthetic
  root missing BasicData/CommonEvent.dat plus unexpected.txt; asserts non-zero exit,
  MISSING, and EXTRA top-level.
- assemble_and_check_round_trip_the_manifest (line 155): synthetic root, synthetic
  exe and runtime DLL; asserts manifest contains sha256/bytes and both staged files;
  check succeeds; a second assembly produces a byte-identical manifest; a same-size
  first-byte corruption is reported as sha256 mismatch; deleting one packaged file
  is reported as MISSING.
- frame_budget_benchmarks_a_real_game_loop (line 242): runs bench for 60 frames on
  work/all and asserts frame-budget frames=60 and p50 <= p95 <= max.

The test helper TempDir uses work/out, not the Windows system temp directory, because
the latter produced PermissionDenied (os error 5) in this environment.

## Limitations

- The validator encodes one known retail layout: required files are looked up under
  BasicData/, with MapData/ and SystemFile/ as required directories. A different
  packaging layout would need its required-file table extended.
- Extra top-level entries are reported but are not fatal; this keeps unpacked DLC
  or extra asset folders from blocking a legitimate root. Missing or zero-byte
  required files remain fatal.
- assemble copies the entire data root (about 1.53 GB for work/all) with no
  compression, delta encoding, signing, or installer. --runtime must list every
  needed DLL explicitly.
- The manifest has only format version 1. There is no migration logic yet.
- SHA-256 is implemented locally (crates/wolf-cli/src/bin/wolf-package.rs:973) to
  avoid a new dependency; known-vector tests cover empty and abc.
- Benchmark numbers in this document are debug-profile, 600 frames, from this
  machine. They are a regression baseline, not a hardware-independent guarantee.
