//! Automated audio inventory + mixer regression for all BGM/SE assets (shared task-51).
//!
//! Two halves, both device-free:
//!
//! 1. Inventory - recursively decode every audio file (.ogg/.mp3/.wav) reachable from the
//!    game asset root (work/all), asserting: the file decodes, has a sane sample rate /
//!    channel count / frame count / duration, is not digital silence (peak + RMS
//!    thresholds), and - where Vorbis LOOPSTART/LOOPLENGTH tags are present - yields a
//!    usable loop region inside the file. A compact summary is printed (per-directory /
//!    per-extension counts, total duration, failures) and also every audio path referenced
//!    from the game's own data (map scripts + common events + system DB) is resolved so the
//!    set the runtime can actually ask for is covered.
//!
//! 2. Mixer regression - exercises Engine::render_offline (no audio device): BGM loop wrap,
//!    stop-with-fade, overlapping SE voices, per-voice volume scaling and clipping sanity.
//!
//! The inventory test skips itself (with a note) when work/all is absent, matching the
//! existing tests/audio.rs behaviour, so a bare checkout still passes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use wolf_audio::decode::decode_file;
use wolf_audio::{DecodedAudio, Engine};

/// Audio extensions we treat as playable BGM/SE.
const AUDIO_EXTS: &[&str] = &["ogg", "mp3", "wav"];

/// Locate the game asset root (work/all) relative to this crate; None on a bare checkout.
fn audio_root() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/all");
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

/// Recursively collect every regular file under dir with an audio extension.
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

/// Peak absolute sample and RMS across all interleaved samples.
fn peak_rms(samples: &[f32]) -> (f32, f32) {
    let mut peak = 0.0f32;
    let mut sum_sq = 0.0f64;
    for &s in samples {
        let a = s.abs();
        if a > peak {
            peak = a;
        }
        sum_sq += (s as f64) * (s as f64);
    }
    let rms = if samples.is_empty() {
        0.0
    } else {
        (sum_sq / samples.len() as f64).sqrt() as f32
    };
    (peak, rms)
}

/// Relative path with forward slashes and the top-level directory as first component.
fn rel_and_top(root: &Path, path: &Path) -> (String, String) {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let top = rel.split('/').next().unwrap_or("").to_string();
    (rel, top)
}

/// One decoded asset row.
struct Row {
    rel: String,
    top: String,
    ext: String,
    sample_rate: u32,
    channels: u16,
    frames: usize,
    duration: f32,
    peak: f32,
    rms: f32,
    loop_points: bool,
}

/// Extract candidate audio path strings embedded in raw game data (map scripts / common
/// events / databases). This is the set of filenames a SoundCmd can hand to the engine, so
/// checking them covers what the runtime will actually request.
fn scan_sound_paths(bytes: &[u8]) -> Vec<String> {
    const PREFIXES: &[&[u8]] = &[
        b"BGM/",
        b"SE/",
        b"DLC_data/",
        b"SystemFile/",
        b"SystemGraphic/",
    ];
    let mut found = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let mut advance = None;
        for p in PREFIXES {
            if bytes[i..].starts_with(p) {
                let mut j = i;
                while j < bytes.len() && bytes[j] >= 0x20 && bytes[j] != 0x7f {
                    j += 1;
                }
                if j > i {
                    if let Ok(s) = std::str::from_utf8(&bytes[i..j]) {
                        let low = s.to_ascii_lowercase();
                        if AUDIO_EXTS.iter().any(|e| low.ends_with(&format!(".{e}"))) {
                            found.push(s.to_string());
                        }
                    }
                }
                advance = Some(j.max(i + 1));
                break;
            }
        }
        i = advance.unwrap_or(i + 1);
    }
    found
}

/// Resolve a SoundCmd-style relative path against an asset root (root itself, then root/Data),
/// mirroring Engine::resolve.
fn resolve_ref(root: &Path, rel: &str) -> Option<PathBuf> {
    let normalised = rel.replace('\\', "/");
    let p = Path::new(&normalised);
    for cand in [p.to_path_buf(), root.join(p), root.join("Data").join(p)] {
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

#[test]
fn audio_inventory() {
    let Some(root) = audio_root() else {
        eprintln!("skipping audio_inventory: work/all not found");
        return;
    };

    let mut files = Vec::new();
    collect_audio(&root, &mut files);
    files.sort();

    // Per-directory and per-extension counts over the discovered set.
    let mut dir_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut ext_counts: BTreeMap<String, usize> = BTreeMap::new();
    for path in &files {
        let (_, top) = rel_and_top(&root, path);
        *dir_counts.entry(top).or_default() += 1;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        *ext_counts.entry(ext).or_default() += 1;
    }

    // Decode every file, collecting rows and human-readable failures.
    let mut rows: Vec<Row> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    let mut total_duration = 0.0f64;
    let mut loop_tagged = 0usize;
    let mut digital_silent: Vec<String> = Vec::new();

    for path in &files {
        let (rel, top) = rel_and_top(&root, path);
        match decode_file(path) {
            Ok(audio) => {
                let (peak, rms) = peak_rms(&audio.samples);
                let duration = audio.duration_secs();
                total_duration += duration as f64;
                if audio.has_loop_points() {
                    loop_tagged += 1;
                }
                if peak < 1e-4 && rms < 1e-5 {
                    digital_silent.push(rel.clone());
                }
                rows.push(Row {
                    rel,
                    top,
                    ext: path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase(),
                    sample_rate: audio.sample_rate,
                    channels: audio.channels,
                    frames: audio.frames(),
                    duration,
                    peak,
                    rms,
                    loop_points: audio.has_loop_points(),
                });
            }
            Err(e) => failures.push(format!("{rel}: {e}")),
        }
    }

    // ---- structural + sanity assertions over the decoded rows ------------------
    let mut sanity: Vec<String> = Vec::new();
    for r in &rows {
        if r.frames == 0 {
            sanity.push(format!("{}: zero frames", r.rel));
        }
        if r.sample_rate == 0 || r.sample_rate > 192_000 {
            sanity.push(format!("{}: implausible sample_rate {}", r.rel, r.sample_rate));
        }
        if r.channels == 0 || r.channels > 8 {
            sanity.push(format!("{}: implausible channels {}", r.rel, r.channels));
        }
        if !(r.duration.is_finite() && r.duration > 0.0 && r.duration <= 900.0) {
            sanity.push(format!("{}: implausible duration {}", r.rel, r.duration));
        }
        if r.peak < 1e-4 {
            sanity.push(format!("{}: near-silent peak {}", r.rel, r.peak));
        }
        if r.rms < 1e-5 {
            sanity.push(format!("{}: near-silent rms {}", r.rel, r.rms));
        }
        if r.loop_points && r.ext != "ogg" {
            sanity.push(format!("{}: loop tags on non-ogg", r.rel));
        }
    }
    // mp3 in this game never carries Vorbis loop tags.
    for r in &rows {
        if r.ext == "mp3" && r.loop_points {
            sanity.push(format!("{}: mp3 unexpectedly has loop points", r.rel));
        }
    }

    // ---- referenced-from-data coverage ----------------------------------------
    let mut refs: BTreeSet<String> = BTreeSet::new();
    let map_dir = root.join("MapData");
    if let Ok(entries) = fs::read_dir(&map_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("mps") {
                if let Ok(bytes) = fs::read(&p) {
                    refs.extend(scan_sound_paths(&bytes));
                }
            }
        }
    }
    for name in ["CommonEvent.dat", "SysDatabase.dat", "DataBase.dat", "CDataBase.dat"] {
        let p = root.join("BasicData").join(name);
        if let Ok(bytes) = fs::read(&p) {
            refs.extend(scan_sound_paths(&bytes));
        }
    }
    let mut ref_present = 0usize;
    let mut ref_missing: Vec<String> = Vec::new();
    for r in &refs {
        if resolve_ref(&root, r).is_some() {
            ref_present += 1;
        } else {
            ref_missing.push(r.clone());
        }
    }

    // ---- compact inventory report ---------------------------------------------
    println!(
        "\n=== audio_inventory: {} discovered, {} decoded, {} failed, {:.2} s decoded total ===",
        files.len(),
        rows.len(),
        failures.len(),
        total_duration
    );
    println!("by directory: {:?}", dir_counts);
    println!("by extension: {:?}", ext_counts);
    println!("loop-tagged files: {loop_tagged} (all ogg)");
    let mut loop_by_top: BTreeMap<&str, usize> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.loop_points) {
        *loop_by_top.entry(r.top.as_str()).or_default() += 1;
    }
    println!("loop-tagged by directory: {:?}", loop_by_top);
    println!(
        "referenced from data: {} unique ({} resolve, {} missing: {:?})",
        refs.len(),
        ref_present,
        ref_missing.len(),
        ref_missing
    );
    println!("digital-silent files: {}", digital_silent.len());

    if !rows.is_empty() {
        let min_dur = rows.iter().map(|r| r.duration).fold(f32::INFINITY, f32::min);
        let max_dur = rows.iter().map(|r| r.duration).fold(0.0f32, f32::max);
        let min_peak = rows.iter().map(|r| r.peak).fold(f32::INFINITY, f32::min);
        let min_rms = rows.iter().map(|r| r.rms).fold(f32::INFINITY, f32::min);
        let mut rates: BTreeSet<u32> = BTreeSet::new();
        let mut chans: BTreeSet<u16> = BTreeSet::new();
        for r in &rows {
            rates.insert(r.sample_rate);
            chans.insert(r.channels);
        }
        println!(
            "duration min/max {min_dur:.3}/{max_dur:.3}s, peak min {min_peak:.5}, rms min {min_rms:.6}"
        );
        println!("sample rates: {:?}, channels: {:?}", rates, chans);
    }
    if !failures.is_empty() {
        println!("\nDECODE FAILURES ({}):", failures.len());
        for f in &failures {
            println!("  {f}");
        }
    }
    if !sanity.is_empty() {
        println!("\nSANITY FAILURES ({}):", sanity.len());
        for s in &sanity {
            println!("  {s}");
        }
    }
    if !digital_silent.is_empty() {
        println!("\nDIGITAL-SILENT ({}): {:?}", digital_silent.len(), digital_silent);
    }

    // ---- hard assertions -------------------------------------------------------
    assert!(!files.is_empty(), "found no audio files under {}", root.display());
    // One shipped retail WAV is malformed: SE/（Action）跳ね.wav has a 26-byte fmt chunk
    // (PCM fmt must be 16/18/40 bytes), which symphonia correctly rejects. That is an
    // asset defect we can only report, not fix here (no production edits). Every *other*
    // file must decode cleanly, so a new regression still fails this test.
    const KNOWN_BAD_DECODE: &[&str] = &["SE/（Action）跳ね.wav"];
    let unexpected: Vec<&String> = failures
        .iter()
        .filter(|f| !KNOWN_BAD_DECODE.iter().any(|k| f.starts_with(k)))
        .collect();
    assert!(
        unexpected.is_empty(),
        "{} unexpected decode failure(s): {:?}",
        unexpected.len(),
        unexpected
    );
    assert!(sanity.is_empty(), "{} audio file(s) failed sanity checks", sanity.len());
    assert!(dir_counts.get("BGM").copied().unwrap_or(0) >= 40, "expected >=40 BGM files");
    assert!(dir_counts.get("SE").copied().unwrap_or(0) >= 250, "expected >=250 SE files");
    assert!(loop_tagged >= 20, "expected >=20 loop-tagged files, found {loop_tagged}");
    // Every audio path the game data references must resolve, except the one known missing
    // retail file (documented in docs/AUDIO.md / ASSET_AUDIT.md).
    let expected_missing: BTreeSet<String> = ["BGM/kana.mp3".to_string()].into_iter().collect();
    let got_missing: BTreeSet<String> = ref_missing.iter().cloned().collect();
    assert_eq!(
        got_missing, expected_missing,
        "unexpected missing sound references (data references audio not present in the tree)"
    );
    assert!(digital_silent.is_empty(), "{} audio file(s) are digital silence", digital_silent.len());
}

// ---------------------------------------------------------------------------
// Mixer / engine regression (offline, render_offline only)
// ---------------------------------------------------------------------------

/// Synthetic source at rate with the given per-frame value replicated across channels.
fn synth(values: &[f32], rate: u32, channels: u16) -> DecodedAudio {
    let mut samples = Vec::with_capacity(values.len() * channels as usize);
    for &v in values {
        for _ in 0..channels {
            samples.push(v);
        }
    }
    DecodedAudio {
        sample_rate: rate,
        channels,
        samples,
        loop_start: None,
        loop_end: None,
    }
}

fn rms_of(xs: &[f32]) -> f32 {
    if xs.is_empty() {
        return 0.0;
    }
    (xs.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / xs.len() as f64).sqrt() as f32
}

#[test]
fn engine_bgm_loop_wrap_is_sample_continuous() {
    // Ramp 0,0.1,...,0.5; loop region frames [2,4). Step == 1 (48k in / 48k out).
    let mut audio = synth(&[0.0, 0.1, 0.2, 0.3, 0.4, 0.5], 48_000, 1);
    audio.loop_start = Some(2);
    audio.loop_end = Some(4);

    let mut engine = Engine::null_with(48_000, 1);
    engine.play_bgm_audio(audio, true, 1.0);
    let out = engine.render_offline(6);
    let expected = [0.0f32, 0.1, 0.2, 0.3, 0.2, 0.3];
    for (i, (g, w)) in out.iter().zip(expected.iter()).enumerate() {
        assert!(
            (g - w).abs() < 1e-3,
            "frame {i}: got {g}, want {w} (loop wrap not continuous)"
        );
    }
    assert!(engine.bgm_playing(), "BGM must keep playing across the loop seam");
}

#[test]
fn engine_bgm_stop_with_fade_ends_silent() {
    let mut engine = Engine::null_with(48_000, 1);
    engine.play_bgm_audio(synth(&[0.9; 4800], 48_000, 1), true, 1.0);
    engine.stop_bgm(60); // 1 second fade at 60 fps
    let out = engine.render_offline(48_000);
    assert!(
        out.last().copied().unwrap().abs() < 1e-3,
        "last frame of a completed fade must be ~silent"
    );
    assert!(!engine.bgm_playing(), "BGM must stop once the fade completes");
}

#[test]
fn engine_stop_bgm_zero_is_immediate() {
    let mut engine = Engine::null_with(48_000, 2);
    engine.play_bgm_audio(synth(&[0.5; 100], 48_000, 2), true, 1.0);
    assert!(engine.bgm_playing());
    engine.stop_bgm(0);
    assert!(!engine.bgm_playing());
    let out = engine.render_offline(16);
    assert!(out.iter().all(|s| s.abs() < 1e-6), "immediate stop must render silence");
}

#[test]
fn engine_se_voices_overlap_and_sum() {
    let mut engine = Engine::null_with(48_000, 1);
    engine.play_se_audio(synth(&[0.2; 1024], 48_000, 1), 1.0);
    engine.play_se_audio(synth(&[0.3; 1024], 48_000, 1), 1.0);
    assert_eq!(engine.se_count(), 2, "two concurrent SE voices");
    let out = engine.render_offline(256);
    // Each frame = 0.2 + 0.3 = 0.5.
    for s in &out {
        assert!((s - 0.5).abs() < 1e-3, "overlapping SE voices should sum to 0.5, got {s}");
    }
}

#[test]
fn engine_volume_scaling_halves_amplitude() {
    // Same signal rendered at volume 1.0 vs 0.5: RMS should halve.
    let make = || synth(&[0.6; 4096], 48_000, 1);

    let mut full = Engine::null_with(48_000, 1);
    full.play_se_audio(make(), 1.0);
    let r_full = rms_of(&full.render_offline(2048));

    let mut half = Engine::null_with(48_000, 1);
    half.play_se_audio(make(), 0.5);
    let r_half = rms_of(&half.render_offline(2048));

    assert!((r_full - 0.6).abs() < 1e-3, "full-volume rms {r_full} != 0.6");
    let ratio = r_full / r_half;
    assert!(
        (ratio - 2.0).abs() < 0.02,
        "halving volume should halve rms (ratio {ratio} != 2.0)"
    );
}

#[test]
fn engine_clipping_is_sane() {
    // Two hot SE (0.8 each) sum to 1.6 pre-clamp; output must stay within [-1, 1] and reach
    // full scale (proving the clamp engages rather than the voices being dropped).
    let mut engine = Engine::null_with(48_000, 1);
    engine.play_se_audio(synth(&[0.8; 512], 48_000, 1), 1.0);
    engine.play_se_audio(synth(&[0.8; 512], 48_000, 1), 1.0);
    let out = engine.render_offline(128);
    assert!(
        out.iter().all(|s| s.abs() <= 1.0 + 1e-6),
        "mixed samples must never exceed [-1, 1]"
    );
    assert!(
        out.iter().any(|s| (*s - 1.0).abs() < 1e-6),
        "0.8 + 0.8 should clamp at full scale (+1.0)"
    );
}
