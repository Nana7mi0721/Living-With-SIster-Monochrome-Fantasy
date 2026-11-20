# M4 — Audio backend (`crates/wolf-audio`)

Status: **landed** (this milestone). The crate implements a complete, device-optional audio
backend for the WOLF RPG Editor reimplementation:

* **decode** — ogg-vorbis, mp3, wav → interleaved `f32` PCM.
* **mixer** — one looping BGM voice + any number of concurrent SE voices, per-voice
  linear-interpolation resampling, volume, looping and fades; fully offline-testable.
* **engine** — a cpal output stream, or a silent null backend when no audio device exists
  (`Engine::new()` never fails and never panics).
* **SoundCmd adapter** — `Engine::apply_sound_cmd(&SoundCmd)` translates opcode 140 `Sound`
  into play/stop calls, so `Game::sound` only needs a 3-line addition.

---

## 1. Scope / M4 plan

| Step | Deliverable | State |
|------|-------------|-------|
| M4.1 | Add `symphonia` + `cpal` deps, verify offline build | done |
| M4.2 | `decode` layer (`DecodedAudio` / `AudioSource`) | done |
| M4.3 | Software `Mixer` (BGM + N SE, resample, loop, fade) | done |
| M4.4 | Loop-point research, read Vorbis `LOOPSTART`/`LOOPLENGTH` | done |
| M4.5 | `Engine` output backend (cpal / null fallback) + public API | done |
| M4.6 | `SoundCmd` adapter + `Game::sound` wiring note | done |
| M4.7 | Tests (decode, mix, clip, loop, fade, null engine) | done |
| M4.8 | This document | done |

Not in M4 (future work): streaming decode (everything is decoded fully into memory today),
3D/spatial audio, real-time BGS/ambient channel (the VM decodes BGS as `Bgm` today), and
mixing against the game's own variable-frame rendering (the mixer is fixed at 60 fps game
frames for fade-length translation).

---

## 2. Audio format inventory

The game package stores audio under `BGM/` and `SE/` (in this repo: `work/all/BGM`,
`work/all/SE`; `work/full` is a reduced test root).

* **BGM**: ~49 files, almost all `.ogg`, a few `.mp3`.
* **SE**: ~273 files, mostly `.wav` / `.ogg`.
* Whole-package extension counts: **172 mp3, 163 ogg, 23 wav**.

All three are covered by Symphonia with the feature set chosen in `3, so no format has to be
dropped. mp3 decodes fine (verified: `Clock-Second_Hand03-1(Loop-1).mp3` → 44100 Hz, 2ch,
352800 frames, 8.0 s), as do wav (`Cancel.wav` → 22050 Hz, mono, 3307 frames) and
ogg-vorbis (`Acoustic_High_Collar.ogg` → 44100 Hz, 2ch, 2,589,511 frames, 58.72 s).

---

## 3. Crate selection and offline dependencies

`crates/wolf-audio/Cargo.toml` uses:

```toml
symphonia = { version = "0.6.1", default-features = false, features = ["flac", "mp3", "pcm", "vorbis", "ogg", "wav"] }
cpal = "0.18.2"
```

**Why not `features = ["all"]` / default features.** Symphonia's default feature set pulls in
`symphonia-format-mkv` and `symphonia-codec-adpcm`, and the optional SIMD feature pulls
`rustfft`; `all` additionally pulls `symphonia-codec-aac`. None of those crates were present in
the offline registry cache, so the build would fail offline. The explicit feature list above is
the minimal set that covers every format the game actually uses and resolves entirely from the
cache (`bundle-flac`, `bundle-mp3`, `codec-pcm`, `codec-vorbis`, `format-ogg`,
`format-riff`, `core`, `metadata`).

**cpal platform crates.** `cpal` itself and `dasp_sample` were cached, but the first resolve
also needed the platform handler crates (`alsa`/`alsa-sys` on Linux, `coreaudio-rs`/`mach2`/
`objc2-*` on macOS). On this machine those were fetched once while the mirror was reachable
and written to `Cargo.lock`, after which:

```
cargo build -p wolf-audio --offline     # Finished in ~0.1s (lock consistent)
cargo test  -p wolf-audio --offline     # 9 passed
```

Conclusion: **the whole workspace builds and tests fully offline** with the manifest above. On a
fresh machine with an empty registry cache, one online `cargo build` is required to populate
those platform crates (they are the only non-symphonia crates that were not pre-cached).

---

## 4. Loop semantics (research conclusion)

WOLF BGM loops are **not** stored in the `.dat` databases. They come from the audio files' own
**Vorbis comments**:

* `LOOPSTART` — loop start, in **frames** (samples per channel).
* `LOOPLENGTH` — loop length, in **frames**.

Survey of the 49 BGM files (26 tagged, all `.ogg`; every mp3 BGM has **no** loop tags):

| file | total frames | LOOPSTART | LOOPLENGTH | start+len |
|------|-------------:|----------:|-----------:|----------:|
| Across_-Battle_Results-.ogg | 2,975,616 | 2,357,360 | 617,983 | 2,975,343 |
| Killer_Roll.ogg | 7,941,823 | 1,246,266 | 6,135,677 | 7,381,943 |
| Sunshine!.ogg | 7,843,968 | 358,273 | 6,417,034 | 6,775,307 |
| The_End_of_Destruction.ogg | 6,913,152 | 577,941 | 6,335,211 | 6,913,152 |
| Acoustic_High_Collar.ogg | 2,589,511 | 1,360,911 | 2,589,511 | 3,950,422 (over) |
| Our_Construction.ogg | 4,491,648 | 2,806 | 4,491,648 | 4,494,454 (over) |
| We_are_Going.ogg | 7,855,485 | 799,430 | 7,855,485 | 8,654,915 (over) |

For most files `LOOPLENGTH` is a genuine sample length and `start + length ≤ total` (often
exactly equal to the end of the file — an intro followed by a loop that runs to EOF). A handful
of files have a `LOOPLENGTH` equal to the **total frame count**, which would push
`start + length` past EOF.

**Decision (implemented in `decode.rs`):**

```
loop_start = LOOPSTART
loop_end   = min(LOOPSTART + LOOPLENGTH, total_frames)
```

i.e. treat the tags as frame offsets and **clamp the loop end to the file length**. This yields a
correct intro→loop region for the normal files and, for the anomalous ones, a sensible
"loop from `LOOPSTART` to EOF" region (e.g. `Acoustic_High_Collar.ogg` → 1,360,911…2,589,511).
A loop is considered usable only when `loop_end > loop_start`; otherwise the mixer falls back
to looping the whole file.

**Fallback loop source.** `work/sys.txt` `BGMリスト` / `BGSリスト` carry a
`ループ開始位置(ms)` column (alongside ファイル名 / 再生音量[%] / 再生周波数[%] / キー±), and
`SEリスト` carries ファイル名 / 音量 / 周波数 / メモリ解放モード. In this game the `BGMリスト`
rows leave the filename empty (`BGM番号 = -3`, "leave it to the event"), so the runtime
filenames come from opcode 140 and the loop points come from the ogg tags. The sys database's
loop-start (ms) is therefore only a **fallback** for tracks that lack tag loop points; it is not
needed for the current content and is not wired yet.

**Seamless looping.** `Mixer` keeps a fractional playback cursor. When a looping voice
advances past `loop_end` it subtracts `loop_end - loop_start` (in a `while` loop, so a very
short loop or a large buffer step cannot escape), which wraps the cursor back into the loop
region **without resetting the fractional phase** — so resampled output is sample-continuous
across the seam (verified by `bgm_loops_seamlessly`).

---

## 5. Public API

### `wolf_audio::decode`
```rust
pub struct DecodedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,          // interleaved: samples[frame * channels + channel]
    pub loop_start: Option<u64>,    // frames
    pub loop_end:   Option<u64>,    // frames (exclusive)
}
pub type AudioSource = DecodedAudio;

impl DecodedAudio {
    pub fn frames(&self) -> usize;
    pub fn duration_secs(&self) -> f32;
    pub fn has_loop_points(&self) -> bool;
    pub fn sample(&self, frame: usize, channel: usize) -> f32;
}

pub fn decode_file(path: impl AsRef<Path>) -> Result<DecodedAudio, AudioError>;
pub fn decode_bytes(bytes: Vec<u8>) -> Result<DecodedAudio, AudioError>;
```

### `wolf_audio::mixer`
```rust
pub const GAME_FPS: f32 = 60.0;

pub struct Mixer { /* ... */ }
impl Mixer {
    pub fn new(out_rate: u32, out_channels: u16) -> Self;
    pub fn play_bgm(&mut self, audio: DecodedAudio, looping: bool, volume: f32);
    pub fn play_se(&mut self, audio: DecodedAudio, volume: f32);
    pub fn stop_bgm(&mut self, fade_frames: u32);   // game frames @60fps; 0 = immediate
    pub fn stop_all_se(&mut self);
    pub fn set_bgm_volume(&mut self, volume: f32);  // clamped 0..=1
    pub fn set_se_volume(&mut self, volume: f32);   // clamped 0..=1
    pub fn bgm_playing(&self) -> bool;
    pub fn se_count(&self) -> usize;
    pub fn out_rate(&self) -> u32;
    pub fn out_channels(&self) -> u16;
    pub fn render(&mut self, frames: usize) -> Vec<f32>; // len == frames * out_channels, clamped [-1,1]
}
```

### `wolf_audio::engine`
```rust
pub struct Engine { /* ... */ }
impl Engine {
    pub fn new() -> Self;                                  // current dir as asset root
    pub fn with_asset_root(root: impl Into<PathBuf>) -> Self;
    pub fn null() -> Self;                                 // 48 kHz / stereo, no device
    pub fn null_with(sample_rate: u32, channels: u16) -> Self;

    pub fn sample_rate(&self) -> u32;
    pub fn channels(&self) -> u16;
    pub fn is_silent(&self) -> bool;
    pub fn set_asset_root(&mut self, root: impl Into<PathBuf>);
    pub fn asset_root(&self) -> &Path;

    pub fn play_bgm(&mut self, file: &str, looping: bool) -> Result<(), AudioError>;
    pub fn play_bgm_with_volume(&mut self, file: &str, looping: bool, volume: f32) -> Result<(), AudioError>;
    pub fn play_bgm_audio(&mut self, audio: DecodedAudio, looping: bool, volume: f32);
    pub fn stop_bgm(&mut self, fade_frames: u32);
    pub fn set_bgm_volume(&mut self, volume: f32);

    pub fn play_se(&mut self, file: &str) -> Result<(), AudioError>;
    pub fn play_se_with_volume(&mut self, file: &str, volume: f32) -> Result<(), AudioError>;
    pub fn play_se_audio(&mut self, audio: DecodedAudio, volume: f32);
    pub fn stop_all_se(&mut self);
    pub fn set_se_volume(&mut self, volume: f32);

    pub fn bgm_playing(&self) -> bool;
    pub fn se_count(&self) -> usize;
    pub fn render_offline(&self, frames: usize) -> Vec<f32>; // device-free; used by tests

    pub fn apply_sound_cmd(&mut self, cmd: &SoundCmd);       // opcode 140 adapter
}
```

`Engine::new()` / `with_asset_root()` try `cpal::default_host().default_output_device()`;
if anything fails (no device, unsupported sample format, stream build/play error) they print

```
[wolf-audio] audio output unavailable (<reason>); using silent null backend
```

and continue on the null backend. **No error is returned and nothing panics** — headless CI and a
desktop both run the same code path, and `render_offline` still produces real mixed samples.

Missing files / undecodable files during `apply_sound_cmd` are logged exactly once per
filename (`[wolf-audio] cannot play <file>: <err>`) and otherwise ignored, so a bad asset never
aborts a frame.

### `SoundCmd` mapping (opcode 140 `Sound`)

| channel | action | engine call |
|---------|--------|-------------|
| Bgm | Play | `play_bgm_with_volume(file, looping = true, volume = volume/100)` |
| Bgm | Stop | `stop_bgm(frames as u32)` (fade length in game frames) |
| Se  | Play | `play_se_with_volume(file, volume = volume/100)` |
| Se  | Stop | `stop_all_se()` |

`SoundCmd.volume` is `intArgs[4]` (0..=100 in retail data; a few loud SE store 200 and the
mixer clamps at full scale) and is clamped/divided by 100. `SoundCmd.frames` is `intArgs[1]`:
the BGM fade-in length, the SE start delay, or the stop fade-out length. Paths are resolved
relative to the asset root, then `asset_root/Data/`, with backslashes normalised to `/` — so
both `BGM/nekomimi.mp3` and `Data\BGM\nekomimi.mp3` work.

---

## 6. Wiring into the game

> **Status: DONE (2026-...).** `crates/wolf-game/src/game.rs` now has `pub audio: Option<wolf_audio::Engine>` (defaults to `None`), `Game::enable_audio()` installs `Engine::with_asset_root(self.root)`, and `GameHost` carries `audio: Option<&mut Engine>` so `Host::sound` forwards each opcode-140 `SoundCmd` to `apply_sound_cmd` after recording it. The CLI gained `--audio` (honoured by `capture`, `capture --map`, `dump`, `play`; off by default so headless runs stay device-free). Pinned by `crates/wolf-game/tests/audio_wiring.rs`. The instructions below are kept as the original design record.

`Game::sound` lives at `crates/wolf-game/src/game.rs` and only records the command:

```rust
fn sound(&mut self, cmd: &SoundCmd) {
    self.sounds.push(cmd.clone());
    self.stats.sounds += 1;
}
```

### 6a. Minimal 3-line wiring

1. In `crates/wolf-game/Cargo.toml` add `wolf-audio = { workspace = true }` to `[dependencies]`.
2. Add one field to `struct Game` (near `pub sounds: Vec<SoundCmd>`, `game.rs:241`):

   ```rust
   /// M4 audio output. `None` keeps the pure-logic/tests path silent.
   pub audio: Option<wolf_audio::Engine>,
   ```

   and initialise it in `Game::new` (near `sounds: Vec::new(),`, `game.rs:343`) — e.g.
   `audio: None,` for tests, or, when a real run should have sound,
   `audio: Some(wolf_audio::Engine::with_asset_root(root.clone())),`.
3. Append to `fn sound` (the only behavioural change):

   ```rust
   if let Some(audio) = self.audio.as_mut() {
       audio.apply_sound_cmd(cmd);
   }
   ```

That is the entire integration: the VM already decodes opcode 140 into `SoundCmd` and calls
`Host::sound`; `apply_sound_cmd` performs play/stop with the correct volume and loop rules.
`crates/wolf-vm/src/exec.rs`, `game.rs` and `main.rs` are otherwise untouched by M4.

### 6b. Optional `--audio` CLI switch

To keep the default run silent (and tests fast / allocation-free), gate real audio behind a CLI
flag parsed in the CLI/game `main.rs`:

1. Parse `--audio` (and optionally `--asset-root <dir>`, defaulting to the package root,
   e.g. `work/all`).
2. When present, construct the game with
   `audio: Some(wolf_audio::Engine::with_asset_root(asset_root))` instead of `None`.
3. `Engine::new()` already degrades to a silent null backend if the machine has no output
   device, so `--audio` is always safe to pass (e.g. on CI or a headless box).

No change to the frame loop is required: the mixer is driven by the cpal callback thread, so
`Game::tick` only ever calls `apply_sound_cmd` (a few lock-protected mixer mutations).

---

## 7. Tests

`cargo test -p wolf-audio --offline` (all device-free):

| test | asserts |
|------|---------|
| `decode_ogg` | a real ogg decodes with rate>0, ch>0, frames>0 |
| `decode_mp3` | a real mp3 decodes with rate>0, ch>0, frames>0 |
| `decode_wav` | a real wav decodes with rate>0, ch>0, frames>0 |
| `ogg_loop_tags_are_read` | `Acoustic_High_Collar.ogg` yields usable loop points inside the file |
| `render_length_is_correct` | `render(n).len() == n * channels` |
| `two_se_voices_sum_stay_clipped` | two 0.8 voices sum and clamp to ≤ 1.0 |
| `bgm_loops_seamlessly` | a `[2,4)` loop wraps (`0,0.1,0.2,0.3,0.2,0.3`) — sample-continuous |
| `bgm_stop_with_fade_ends_silent` | a 60-frame fade reaches ~0 and stops the BGM |
| `engine_null_backend_renders_and_handles_cmds` | null engine renders, and Stop SoundCmds are safe |

Tests that need real assets locate `work/all` relative to `CARGO_MANIFEST_DIR` and
**skip themselves (with an `eprintln!` note) when it is absent**, so a bare checkout still passes.
No test requires an audio device.

---

## 8. Notes / limitations

* **Memory**: decoding is eager — a full BGM track is held as interleaved f32 (≈ 2.59M frames ×
  2ch × 4 B = ~21 MB for the longest files). Fine for a desktop M4; streaming would be the next
  step if the track set grows.
* **Fade semantics**: `SoundCmd.frames` is interpreted as a fade length in **game frames
  (60 fps)**, matching WOLF's per-frame model. `Mixer::stop_bgm` converts it to output samples.
* **BGS**: the VM currently routes BGS through the BGM channel; M4 provides a single BGM voice.
  A dedicated ambient channel is future work (the `Mixer` voice list already supports N SE, so
  it is a small addition).
* **No audio device**: fully supported — null backend, silent, tested.
