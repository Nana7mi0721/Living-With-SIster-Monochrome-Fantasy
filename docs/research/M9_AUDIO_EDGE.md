# M9 — Audio edge cases: malformed RIFF fmt chunk + missing assets

Status: implemented + tested. **No other crates touched, nothing committed.** Builds on
task-51 / docs/research/M9_AUDIO_SMOKE.md (358 audio files, 357 decode OK, 1 failure:
SE/（Action）跳ね.wav with a 26-byte fmt chunk; BGM/kana.mp3 referenced but absent).

## Result in one line

The one real malformed WAV now decodes via a strict, narrowly-scoped tolerant RIFF/WAVE PCM
reader; genuinely invalid/truncated files are still rejected and missing paths return a clean
AudioError. New test file: 5 tests, all green.

## The failing asset (byte-level analysis)

`work/all/SE/（Action）跳ね.wav` — 73702 bytes total, standard RIFF/WAVE + PCM data, but its
`fmt ` chunk declares **26** bytes instead of the canonical 16 (or 18 with cbSize):

| offset | field | value |
|---|---|---|
| 0 | RIFF id | "RIFF" |
| 4 | RIFF size | 73694 |
| 8 | form type | "WAVE" |
| 12 | chunk id | "fmt " |
| 16 | **fmt size** | **26** (non-standard; 10 trailing bytes after the 16 PCM fields) |
| 20 | audioFormat | 1 (PCM) |
| 22 | channels | 1 |
| 24 | sampleRate | 44100 |
| 28 | byteRate | 88200 |
| 32 | blockAlign | 2 |
| 34 | bitsPerSample | 16 |
| 36..46 | extension | 10 zero bytes |
| 46 | chunk id | "data" |
| 50 | data size | 73648 |
| 54.. | PCM data | 73648 bytes = 36824 frames = **0.835 s** |

Symphonia 0.6 rejects this at probe: `probe: malformed stream: wav: malformed fmt_pcm chunk`.
The 16 canonical PCM fields are intact, so the file is decodable by a more lenient reader.

## What changed (crates/wolf-audio/src/decode.rs only)

A single narrow fallback, invoked **only** when Symphonia fails to *probe* the container
(`is_probe_failure` = message starts with `probe:`):

1. `decode_file`: on a probe-failure it re-reads the bytes and calls
   `try_decode_wav_pcm_tolerant`; if that returns None the **original** Symphonia error is
   returned unchanged.
2. `decode_bytes`: same retry against the in-memory buffer.
3. `try_decode_wav_pcm_tolerant(bytes) -> Option<Result<DecodedAudio, AudioError>>` — a strict
   RIFF chunk walker that:
   - returns **None** (fall through to the original error) if the bytes are not "RIFF"/"WAVE",
     or if the wave is RIFF/WAVE but not integer PCM (audioFormat != 1);
   - reads the canonical 16-byte fmt fields regardless of the declared fmt size, so a 26-byte
     chunk parses;
   - **validates** channels > 0, sampleRate > 0, bit-depth in {8,16,24,32},
     blockAlign == channels * (bits/8), data length a multiple of blockAlign, and rejects a data
     chunk that runs past end-of-buffer (**truncated data**);
   - converts interleaved integer PCM (8 unsigned, 16/24/32 signed) to f32 in [-1, 1].

Design guarantee — **no weakening of rejection**: the tolerant path never runs for files
Symphonia accepts, and for files it rejects it still returns the original error unless the bytes
are *positively validated* as RIFF/WAVE integer PCM. A truncated data chunk, a missing fmt/data
chunk, an inconsistent blockAlign, a non-PCM codec, or random garbage all still yield an
`AudioError` (never a panic, never a bogus buffer).

No loop metadata is invented for WAV (plain PCM has none); `loop_start/loop_end` stay None.

## Tests (crates/wolf-audio/tests/audio_edge_cases.rs, 5 tests)

| test | asserts |
|---|---|
| `malformed_26_byte_fmt_chunk_wav_now_decodes` | decode_file of the real file => 44100 Hz, 1 ch, 36824 frames, ~0.835 s, all samples finite in [-1,1], no loop points; decode_bytes of the same bytes agrees |
| `truncated_wav_data_is_still_rejected` | first 200 bytes (RIFF+fmt+data header, data body cut) => clean AudioError describing truncated/malformed data |
| `random_garbage_bytes_are_rejected_without_panicking` | 64 zero bytes => clean AudioError |
| `missing_asset_returns_clean_error_not_panic` | decode_file(missing) => "audio io error:"; Engine::play_se(missing) => clean error, no SE queued |
| `repaired_file_mixes_to_completion_and_loops_as_bgm` | repaired wav played as SE produces non-silent output and is dropped after one pass; played as looping BGM it is still alive after a full pass |

All asset-dependent tests skip gracefully (with an eprintln note) when work/all is absent.

## Exact commands / results

```
$ cargo test -q -p wolf-audio --test audio_edge_cases -- --nocapture
running 5 tests
.....
test result: ok. 5 passed; 0 failed; ... finished in 0.00s

$ cargo test -q -p wolf-audio
test result: ok. 9 passed; 0 failed   (tests/audio.rs)
test result: ok. 5 passed; 0 failed   (tests/audio_edge_cases.rs)
test result: ok. 7 passed; 0 failed   (tests/audio_inventory.rs, ~15 s)
=> 21 passed total, 0 failed
```

The task-51 inventory (audio_inventory.rs) still passes: it tolerates zero decode failures as
well as the previously-known single failure, so repairing the asset does not regress it.

## Caveats / follow-ups (unchanged)

- The repaired `SE/（Action）跳ね.wav` now plays; the alternative (re-exporting the asset with a
  16-byte fmt chunk) is no longer required, though it would also be valid.
- Missing `BGM/kana.mp3` is still genuinely absent (game references it); it now yields a clean
  diagnostic instead of a crash. Confirming it is unused, or sourcing the file, remains a
  content decision.
- Only RIFF/WAVE **integer PCM** is tolerated. Other non-canonical containers/codecs are still
  left to Symphonia's (correct) rejection.
