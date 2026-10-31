//! M11 audio acceptance dossier (shared task-84).
//!
//! Device-free, deterministic audit of every loop-tagged BGM track in the shipped retail
//! asset tree (default `work/all`, override with `WOLF_AUDIO_ROOT`). The point is **not** to
//! claim the tracks loop cleanly by ear -- no agent can listen -- but to make every objective,
//! machine-checkable property explicit and reproducible, and to name exactly which items a
//! human must still audition.
//!
//! For each track carrying Vorbis `LOOPSTART` / `LOOPLENGTH` comments it reports:
//! * container/codec facts: extension, sample rate, channels, frames, duration;
//! * loop candidates: the raw tag values (frames) and the region `decode.rs` actually keeps
//!   after clamping to the decoded length;
//! * seam discontinuity metrics at the proposed loop point:
//!     - `seam_abs`: worst per-channel |sample(loop_end-1) - sample(loop_start)|,
//!       i.e. the instantaneous waveform jump when playback wraps end -> start;
//!     - `step`: local mean sample-to-sample step around both sides of the seam, used to
//!       normalise `seam_abs` into the dimensionless `ratio = seam_abs / step` (a ratio near
//!       1 means the wrap looks like an ordinary sample step; a large ratio means a click);
//!     - `rms_jump`: |RMS(window after start) - RMS(window before end)| -- a loudness step;
//!     - `dc_delta`: |mean(after) - mean(before)| -- a DC offset step at the seam.
//!
//! Assertions lock the retail inventory (exactly 31 loop-tagged files, all ogg, all with a
//! usable in-bounds region that decodes) and that every metric is finite, so a regression in
//! either the decoder or the asset set fails loudly. Tracks are flagged (SHORT_LOOP /
//! SEAM_ABS / SEAM_RATIO / RMS_JUMP / DC_DELTA / OVERRUN / LEN_EQUALS_TRACK) purely to steer
//! the human listener; flags are advisory, the assertions only pin structural facts.

use std::fs;
use std::path::{Path, PathBuf};

use wolf_audio::decode::{decode_file, DecodedAudio};

const AUDIO_EXTS: &[&str] = &["ogg", "mp3", "wav"];
const EXPECTED_LOOP_TAGGED: usize = 31;

fn audio_root() -> Option<PathBuf> {
    let root = std::env::var_os("WOLF_AUDIO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/all"));
    if root.is_dir() {
        Some(root)
    } else {
        None
    }
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn collect_audio(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            collect_audio(&path, out);
        } else if ft.is_file() && is_audio(&path) {
            out.push(path);
        }
    }
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Case-insensitive byte scan for `KEY=<digits>` in the raw file. Vorbis comments are ASCII,
/// so this recovers the *un-clamped* tag values that `decode.rs` folds into DecodedAudio.
fn raw_tag(bytes: &[u8], key: &[u8]) -> Option<u64> {
    let mut i = 0usize;
    while i + key.len() <= bytes.len() {
        if bytes[i..i + key.len()].eq_ignore_ascii_case(key) {
            let mut j = i + key.len();
            while j < bytes.len() && matches!(bytes[j], b' ' | b'\t') {
                j += 1;
            }
            if bytes.get(j) == Some(&b'=') {
                j += 1;
                while j < bytes.len() && matches!(bytes[j], b' ' | b'\t') {
                    j += 1;
                }
                let start = j;
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                if j > start {
                    return std::str::from_utf8(&bytes[start..j])
                        .ok()
                        .and_then(|text| text.parse::<u64>().ok());
                }
            }
        }
        i += 1;
    }
    None
}

fn mean_abs_step(audio: &DecodedAudio, from: usize, to: usize) -> f32 {
    let channels = audio.channels as usize;
    let frames = audio.frames();
    let from = from.min(frames);
    let to = to.min(frames);
    if channels == 0 || to <= from + 1 {
        return 0.0;
    }
    let mut sum = 0.0f64;
    let mut count = 0usize;
    for frame in (from + 1)..to {
        for channel in 0..channels {
            let delta = (audio.sample(frame, channel) - audio.sample(frame - 1, channel)).abs();
            sum += delta as f64;
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        (sum / count as f64) as f32
    }
}

/// Worst per-channel |sample(end-1) - sample(start)|: the waveform jump on wrap.
fn seam_abs(audio: &DecodedAudio, start: usize, end: usize) -> f32 {
    let channels = audio.channels as usize;
    if channels == 0 || end == 0 || end <= start {
        return 0.0;
    }
    let mut worst = 0.0f32;
    for channel in 0..channels {
        let delta = (audio.sample(end - 1, channel) - audio.sample(start, channel)).abs();
        worst = worst.max(delta);
    }
    worst
}

/// RMS over frames [from, to) mixing all channels.
fn window_rms(audio: &DecodedAudio, from: usize, to: usize) -> f32 {
    let channels = audio.channels as usize;
    let frames = audio.frames();
    let from = from.min(frames);
    let to = to.min(frames);
    if channels == 0 || to <= from {
        return 0.0;
    }
    let mut sum = 0.0f64;
    let mut n = 0usize;
    for frame in from..to {
        for channel in 0..channels {
            let s = audio.sample(frame, channel) as f64;
            sum += s * s;
            n += 1;
        }
    }
    (sum / n as f64).sqrt() as f32
}

/// Mean over frames [from, to) mixing all channels (DC level).
fn window_mean(audio: &DecodedAudio, from: usize, to: usize) -> f32 {
    let channels = audio.channels as usize;
    let frames = audio.frames();
    let from = from.min(frames);
    let to = to.min(frames);
    if channels == 0 || to <= from {
        return 0.0;
    }
    let mut sum = 0.0f64;
    let mut n = 0usize;
    for frame in from..to {
        for channel in 0..channels {
            sum += audio.sample(frame, channel) as f64;
            n += 1;
        }
    }
    (sum / n as f64) as f32
}

struct Row {
    rel: String,
    ext: String,
    sample_rate: u32,
    channels: u16,
    frames: usize,
    duration: f32,
    start: Option<u64>,
    length: Option<u64>,
    decoded: Option<(u64, u64)>,
    loop_secs: f32,
    seam_abs: f32,
    step: f32,
    ratio: f32,
    rms_jump: f32,
    dc_delta: f32,
    flags: Vec<&'static str>,
}

fn audit_row(root: &Path, path: &Path) -> Option<Row> {
    let Ok(bytes) = fs::read(path) else { return None };
    let start = raw_tag(&bytes, b"LOOPSTART");
    let length = raw_tag(&bytes, b"LOOPLENGTH");
    if start.is_none() && length.is_none() {
        return None;
    }
    let audio = decode_file(path).expect("loop-tagged file must decode");
    let frames = audio.frames();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let decoded = match (audio.loop_start, audio.loop_end) {
        (Some(ls), Some(le)) => Some((ls, le)),
        _ => None,
    };

    let mut flags: Vec<&'static str> = Vec::new();
    let (mut loop_secs, mut seam, mut step, mut ratio, mut rms_jump, mut dc_delta) =
        (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);

    if let Some((ls, le)) = decoded {
        let ls = ls as usize;
        let le = le as usize;
        if audio.sample_rate > 0 {
            loop_secs = (le - ls) as f32 / audio.sample_rate as f32;
        }
        seam = seam_abs(&audio, ls, le);
        step = (mean_abs_step(&audio, ls, (ls + 1024).min(le))
            + mean_abs_step(&audio, le.saturating_sub(1024), le))
            * 0.5;
        ratio = if step > 0.0 { seam / step } else { 0.0 };

        // A ~100 ms window on each side of the seam, clamped so it never crosses the region.
        let win = (audio.sample_rate as usize / 10).max(1);
        let before = window_rms(&audio, le.saturating_sub(win), le);
        let after = window_rms(&audio, ls, (ls + win).min(le));
        rms_jump = (after - before).abs();
        dc_delta = (window_mean(&audio, ls, (ls + win).min(le))
            - window_mean(&audio, le.saturating_sub(win), le))
        .abs();

        if loop_secs < 1.0 {
            flags.push("SHORT_LOOP");
        }
        if seam >= 0.20 {
            flags.push("SEAM_ABS");
        }
        if step > 0.0 && ratio >= 8.0 {
            flags.push("SEAM_RATIO");
        }
        if rms_jump >= 0.10 {
            flags.push("RMS_JUMP");
        }
        if dc_delta >= 0.05 {
            flags.push("DC_DELTA");
        }
        if let (Some(raw_start), Some(raw_len)) = (start, length) {
            let raw_end = raw_start.saturating_add(raw_len);
            if raw_end > frames as u64 {
                flags.push("OVERRUN");
            }
            if raw_len == frames as u64 {
                flags.push("LEN_EQUALS_TRACK");
            }
        }
        if start.map_or(false, |v| v >= frames as u64) {
            flags.push("START_OOB");
        }
    } else {
        flags.push("NO_USABLE_LOOP");
    }

    Some(Row {
        rel: rel(root, path),
        ext,
        sample_rate: audio.sample_rate,
        channels: audio.channels,
        frames,
        duration: audio.duration_secs(),
        start,
        length,
        decoded,
        loop_secs,
        seam_abs: seam,
        step,
        ratio,
        rms_jump,
        dc_delta,
        flags,
    })
}

#[test]
fn m11_audio_loop_audit() {
    let Some(root) = audio_root() else {
        eprintln!("skipping m11_audio_loop_audit: audio root not found (set WOLF_AUDIO_ROOT)");
        return;
    };

    let mut files = Vec::new();
    collect_audio(&root, &mut files);
    files.sort();

    let mut rows: Vec<Row> = Vec::new();
    for path in &files {
        if let Some(row) = audit_row(&root, path) {
            rows.push(row);
        }
    }
    // Deterministic presentation order, independent of filesystem iteration order.
    rows.sort_by(|a, b| a.rel.cmp(&b.rel));

    // ---- human-readable dossier (stable, one line per track) -------------------
    println!(
        "\n=== M11 audio loop audit: {} loop-tagged track(s) under {} ===",
        rows.len(),
        root.display()
    );
    println!(
        "{:>40} {:>4} {:>6} {:>2} {:>9} {:>8} {:>9} {:>9} {:>18} {:>8} {:>8} {:>7} {:>7} {:>7} {:>8}  flags",
        "file", "ext", "rate", "ch", "dur_s", "frames", "LOOPSTART", "LOOPLEN", "decoded_region",
        "loop_s", "seam_abs", "step", "ratio", "rms_jmp", "dc_delta"
    );
    for r in &rows {
        println!(
            "{:>40} {:>4} {:>6} {:>2} {:>9.2} {:>8} {:>9} {:>9} {:>18} {:>8.2} {:>8.4} {:>7.4} {:>7.2} {:>7.4} {:>8.4}  {}",
            r.rel,
            r.ext,
            r.sample_rate,
            r.channels,
            r.duration,
            r.frames,
            r.start.map_or("-".to_string(), |v| v.to_string()),
            r.length.map_or("-".to_string(), |v| v.to_string()),
            r.decoded.map(|(s, e)| format!("{s}..{e}")).unwrap_or_else(|| "none".into()),
            r.loop_secs,
            r.seam_abs,
            r.step,
            r.ratio,
            r.rms_jump,
            r.dc_delta,
            r.flags.join(","),
        );
    }

    let flagged: Vec<&Row> = rows.iter().filter(|r| !r.flags.is_empty()).collect();
    println!(
        "\nsummary: {} tracks, {} flagged for human listening",
        rows.len(),
        flagged.len()
    );
    for r in &flagged {
        println!("  LISTEN {} [{}]", r.rel, r.flags.join(","));
    }

    // ---- structural assertions (lock the shipped inventory) --------------------
    assert_eq!(
        rows.len(),
        EXPECTED_LOOP_TAGGED,
        "expected exactly {} loop-tagged tracks, found {}; if the retail asset set changed, \
         update EXPECTED_LOOP_TAGGED and docs/research/M11_AUDIO_ACCEPTANCE.md",
        EXPECTED_LOOP_TAGGED,
        rows.len()
    );
    for r in &rows {
        assert_eq!(r.ext, "ogg", "loop tags must be on ogg, got {} ({})", r.rel, r.ext);
        assert!(r.start.is_some(), "{} has LOOPLENGTH but no LOOPSTART", r.rel);
        assert!(r.decoded.is_some(), "{} has no usable decoded loop region", r.rel);
        assert!(!r.flags.contains(&"START_OOB"), "{} loop start is past end of file", r.rel);
        assert!(!r.flags.contains(&"NO_USABLE_LOOP"), "{} decoded loop is unusable", r.rel);
        assert!(r.sample_rate > 0 && r.frames > 0, "{} decoded to nothing usable", r.rel);
        for (name, v) in [
            ("seam_abs", r.seam_abs),
            ("step", r.step),
            ("ratio", r.ratio),
            ("rms_jump", r.rms_jump),
            ("dc_delta", r.dc_delta),
        ] {
            assert!(v.is_finite(), "{} {name} is not finite ({v})", r.rel);
        }
    }

    // The raw tag values must explain the clamped decoded region exactly (same contract the
    // decoder guarantees and m10_audio_residual pins): this catches an off-by-one regression.
    for r in &rows {
        let (ls, le) = r.decoded.unwrap();
        let start = r.start.unwrap();
        let expected_end = match r.length {
            Some(len) => start.saturating_add(len).min(r.frames as u64),
            None => r.frames as u64,
        };
        assert_eq!(ls, start, "{} decoded loop_start differs from LOOPSTART", r.rel);
        assert_eq!(le, expected_end, "{} decoded loop_end differs from clamped tag end", r.rel);
    }

    // Determinism: a second in-memory pass over the already-sorted rows must be identical.
    let mut rerun: Vec<(String, f32, f32, f32)> = rows
        .iter()
        .map(|r| (r.rel.clone(), r.seam_abs, r.rms_jump, r.ratio))
        .collect();
    rerun.sort_by(|a, b| a.0.cmp(&b.0));
    let first: Vec<(String, f32, f32, f32)> = rows
        .iter()
        .map(|r| (r.rel.clone(), r.seam_abs, r.rms_jump, r.ratio))
        .collect();
    assert_eq!(first, rerun, "audit metrics must be deterministic");
}
