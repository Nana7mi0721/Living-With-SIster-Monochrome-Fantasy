//! Edge-case tests for the M4 audio backend.
//!
//! These cover the two retail-asset defects surfaced by the audio inventory (task-51):
//!   * SE/（Action）跳ね.wav -- a valid PCM WAV whose fmt chunk is 26 bytes instead of the
//!     canonical 16/18, which Symphonia rejects at probe time.
//!   * BGM/kana.mp3 -- referenced by the game but absent from the package.
//!
//! Everything here is device-free; asset-dependent cases skip gracefully when work/all is
//! not present (e.g. a bare CI checkout), matching the existing tests.

use std::path::{Path, PathBuf};

use wolf_audio::decode::{decode_bytes, decode_file};
use wolf_audio::mixer::Mixer;
use wolf_audio::{AudioError, Engine};

/// Root of the unpacked game data.
fn asset_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/all")
}

/// The one real WAV with a non-canonical (26-byte) fmt chunk.
fn malformed_wav_path() -> PathBuf {
    asset_root().join("SE").join("（Action）跳ね.wav")
}

fn is_clean_audio_error(err: &AudioError) -> bool {
    let text = err.to_string();
    text.starts_with("audio io error:")
        || text.starts_with("unsupported audio:")
        || text.starts_with("empty audio:")
}

#[test]
fn malformed_26_byte_fmt_chunk_wav_now_decodes() {
    let path = malformed_wav_path();
    if !path.is_file() {
        eprintln!("skipping malformed_26_byte_fmt_chunk_wav_now_decodes: work/all asset missing");
        return;
    }

    let audio = decode_file(&path).expect("tolerant WAV/PCM decode of the 26-byte fmt file");

    // The canonical 16-byte PCM fields are: PCM, mono, 44100 Hz, 16-bit, blockAlign 2.
    assert_eq!(
        audio.sample_rate, 44_100,
        "sample rate must be read from the canonical fields"
    );
    assert_eq!(audio.channels, 1, "mono file must decode as 1 channel");
    assert!(audio.frames() > 0, "repaired file must yield frames");
    // 73648 data bytes / 2 bytes-per-frame = 36824 frames = 0.835 s.
    assert_eq!(
        audio.frames(),
        36_824,
        "frame count must match the data chunk"
    );
    assert!(
        (audio.duration_secs() - 0.835).abs() < 0.01,
        "duration {} s should be ~0.835 s",
        audio.duration_secs()
    );
    assert!(!audio.samples.is_empty());
    assert!(
        audio
            .samples
            .iter()
            .all(|s| s.is_finite() && s.abs() <= 1.0),
        "all decoded samples must be finite and within [-1, 1]"
    );
    // Pure PCM WAV carries no Vorbis loop tags.
    assert!(
        !audio.has_loop_points(),
        "plain WAV must not report loop points"
    );

    // The in-memory path must decode the identical bytes the same way.
    let bytes = std::fs::read(&path).expect("read wav bytes");
    let from_mem = decode_bytes(bytes).expect("decode_bytes must also handle the repaired WAV");
    assert_eq!(from_mem.sample_rate, audio.sample_rate);
    assert_eq!(from_mem.channels, audio.channels);
    assert_eq!(from_mem.frames(), audio.frames());
}

#[test]
fn truncated_wav_data_is_still_rejected() {
    let path = malformed_wav_path();
    if !path.is_file() {
        eprintln!("skipping truncated_wav_data_is_still_rejected: work/all asset missing");
        return;
    }

    let full = std::fs::read(&path).expect("read wav bytes");
    // Keep the RIFF/WAVE header + fmt + data *header*, but cut the data body short.
    // Symphonia fails at probe (non-canonical fmt), so the tolerant reader runs and must
    // reject the truncated data chunk instead of returning a bogus buffer.
    let truncated = full[..200.min(full.len())].to_vec();
    let err = decode_bytes(truncated).expect_err("truncated wav must be rejected");
    assert!(
        is_clean_audio_error(&err),
        "expected a clean AudioError, got: {err}"
    );
    let text = err.to_string();
    assert!(
        text.contains("truncated") || text.contains("malformed") || text.contains("probe"),
        "error should describe the truncated/malformed data, got: {text}"
    );
}

#[test]
fn random_garbage_bytes_are_rejected_without_panicking() {
    // Not a RIFF/WAVE stream and not any supported container: must be a clean error.
    let err = decode_bytes(vec![0u8; 64]).expect_err("garbage must be rejected");
    assert!(
        is_clean_audio_error(&err),
        "expected a clean AudioError, got: {err}"
    );
}

#[test]
fn missing_asset_returns_clean_error_not_panic() {
    // Direct decoder path: a missing file is an Io error, never a panic.
    let missing = asset_root()
        .join("SE")
        .join("__wolf_audio_definitely_missing__.wav");
    let err = decode_file(&missing).expect_err("missing file must be an Err");
    assert!(
        is_clean_audio_error(&err),
        "expected a clean AudioError, got: {err}"
    );
    assert!(
        err.to_string().starts_with("audio io error:"),
        "missing file should be an Io error, got: {err}"
    );

    // Engine path mirrors the game's runtime: resolve() fails -> clean diagnostic, no panic.
    let mut engine = Engine::null_with(44_100, 1);
    engine.set_asset_root(asset_root());
    let se_err = engine
        .play_se("SE/__wolf_audio_definitely_missing__.wav")
        .expect_err("engine play_se on a missing path must be an Err");
    assert!(
        is_clean_audio_error(&se_err),
        "expected a clean AudioError, got: {se_err}"
    );
    assert_eq!(
        engine.se_count(),
        0,
        "no SE voice should be queued for a missing asset"
    );
}

#[test]
fn repaired_file_mixes_to_completion_and_loops_as_bgm() {
    let path = malformed_wav_path();
    if !path.is_file() {
        eprintln!(
            "skipping repaired_file_mixes_to_completion_and_loops_as_bgm: work/all asset missing"
        );
        return;
    }

    let audio = decode_file(&path).expect("decode repaired wav");
    let frames = audio.frames();
    let channels = audio.channels as usize;

    // One-shot SE: it must produce audible output and finish after one full pass.
    let mut mixer = Mixer::new(audio.sample_rate, audio.channels);
    mixer.play_se(audio.clone(), 1.0);
    assert_eq!(mixer.se_count(), 1);
    let out = mixer.render(frames);
    assert_eq!(
        out.len(),
        frames * channels,
        "render length must be frames * channels"
    );
    assert!(
        out.iter().any(|s| s.abs() > 1e-4),
        "repaired SE rendered only silence"
    );
    assert_eq!(
        mixer.se_count(),
        0,
        "a non-looping SE voice must be dropped after it ends"
    );

    // BGM loop: the same buffer played looping must still be live after a full pass.
    let mut mixer = Mixer::new(audio.sample_rate, audio.channels);
    mixer.play_bgm(audio, true, 1.0);
    let _ = mixer.render(frames + 16);
    assert!(
        mixer.bgm_playing(),
        "looping BGM must still be playing after one full pass"
    );
}
