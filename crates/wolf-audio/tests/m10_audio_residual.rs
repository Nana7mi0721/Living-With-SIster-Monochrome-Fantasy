//! M10 audio residual audit (shared task-72).
//!
//! Two device-free checks over the shipped retail asset tree (default work/all,
//! overridable with WOLF_AUDIO_ROOT):
//!
//! 1. loop_tag_audit decodes every audio file that carries Vorbis LOOPSTART /
//!    LOOPLENGTH comments, prints the raw tag values, the region decode.rs keeps
//!    after clamping, and a seam-continuity metric (jump between the sample just
//!    before loop_end and the first sample at loop_start, normalised by the local
//!    mean sample-to-sample step). Rows with an overrun, a sub-second loop, or a
//!    large seam jump are flagged for manual listening.
//! 2. missing_bgm_kana proves BGM/kana.mp3 is registered only in
//!    SysDatabase.dat (never referenced by a map script or another database), is
//!    absent from the extracted tree, and that a SoundCmd for it is ignored by
//!    Engine with no panic and no BGM voice.

use std::fs;
use std::path::{Path, PathBuf};

use wolf_audio::decode::{decode_file, DecodedAudio};
use wolf_audio::Engine;
use wolf_vm::{SoundAction, SoundChannel, SoundCmd};

const AUDIO_EXTS: &[&str] = &["ogg", "mp3", "wav"];

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

/// Case-insensitive search for KEY=<digits> in the raw file. Vorbis comments are
/// ASCII even when the payload is UTF-8, so a byte scan is enough to recover the
/// un-clamped tag values decode.rs collapses into DecodedAudio::loop_start/end.
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

fn count_sub(haystack: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || haystack.len() < needle.len() {
        return 0;
    }
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
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

#[test]
fn loop_tag_audit() {
    let Some(root) = audio_root() else {
        eprintln!("skipping loop_tag_audit: audio root not found");
        return;
    };

    let mut files = Vec::new();
    collect_audio(&root, &mut files);
    files.sort();

    struct Row {
        rel: String,
        ext: String,
        sample_rate: u32,
        channels: u16,
        frames: usize,
        start: Option<u64>,
        length: Option<u64>,
        decoded: Option<(u64, u64)>,
        seam: f32,
        step: f32,
        flags: Vec<&'static str>,
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut decode_errors: Vec<String> = Vec::new();
    for path in &files {
        let Ok(bytes) = fs::read(path) else { continue };
        let start = raw_tag(&bytes, b"LOOPSTART");
        let length = raw_tag(&bytes, b"LOOPLENGTH");
        if start.is_none() && length.is_none() {
            continue;
        }
        let rel_name = rel(&root, path);
        let audio = match decode_file(path) {
            Ok(audio) => audio,
            Err(err) => {
                decode_errors.push(format!("{rel_name}: {err}"));
                continue;
            }
        };
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
        if let Some((ls, le)) = decoded {
            let ls = ls as usize;
            let le = le as usize;
            let loop_secs = if audio.sample_rate == 0 {
                0.0
            } else {
                (le - ls) as f32 / audio.sample_rate as f32
            };
            let seam = seam_abs(&audio, ls, le);
            let step = (mean_abs_step(&audio, ls, (ls + 1024).min(le))
                + mean_abs_step(&audio, le.saturating_sub(1024), le))
                * 0.5;
            if loop_secs < 1.0 {
                flags.push("SHORT_LOOP");
            }
            if seam >= 0.20 {
                flags.push("SEAM_ABS");
            }
            if step > 0.0 && seam / step >= 8.0 {
                flags.push("SEAM_RATIO");
            }
            if let (Some(raw_start), Some(raw_len)) = (start, length) {
                let raw_end = raw_start.saturating_add(raw_len);
                if raw_end > frames as u64 {
                    flags.push("OVERRUN");
                }
                if raw_len == frames as u64 {
                    flags.push("LEN_EQUALS_TRACK");
                }
                let _ = raw_end;
            }
            if start.is_some_and(|value| value >= frames as u64) {
                flags.push("START_OOB");
            }
            rows.push(Row {
                rel: rel_name,
                ext,
                sample_rate: audio.sample_rate,
                channels: audio.channels,
                frames,
                start,
                length,
                decoded,
                seam,
                step,
                flags,
            });
        } else {
            flags.push("NO_USABLE_LOOP");
            rows.push(Row {
                rel: rel_name,
                ext,
                sample_rate: audio.sample_rate,
                channels: audio.channels,
                frames,
                start,
                length,
                decoded: None,
                seam: 0.0,
                step: 0.0,
                flags,
            });
        }
    }

    println!(
        "\n=== m10 loop_tag_audit: {} loop-tagged audio file(s) ===",
        rows.len()
    );
    println!(
        "{:>44} {:>4} {:>6} {:>2} {:>10} {:>10} {:>10} {:>22} {:>8} {:>6} {:>6} {:>7}  flags",
        "file",
        "ext",
        "rate",
        "ch",
        "frames",
        "start",
        "length",
        "decoded_region",
        "loop_s",
        "seam",
        "step",
        "ratio"
    );
    for row in &rows {
        let region = row
            .decoded
            .map(|(ls, le)| format!("{ls}..{le}"))
            .unwrap_or_else(|| "none".to_string());
        let loop_secs = row.decoded.map_or(0.0, |(ls, le)| {
            if row.sample_rate == 0 {
                0.0
            } else {
                (le - ls) as f32 / row.sample_rate as f32
            }
        });
        let ratio = if row.step > 0.0 {
            row.seam / row.step
        } else {
            0.0
        };
        println!(
            "{:>44} {:>4} {:>6} {:>2} {:>10} {:>10} {:>10} {:>22} {:>8.3} {:>6.3} {:>6.4} {:>7.1}  {}",
            row.rel,
            row.ext,
            row.sample_rate,
            row.channels,
            row.frames,
            row.start.map_or("-".to_string(), |v| v.to_string()),
            row.length.map_or("-".to_string(), |v| v.to_string()),
            region,
            loop_secs,
            row.seam,
            row.step,
            ratio,
            row.flags.join(",")
        );
    }

    let flagged: Vec<&Row> = rows.iter().filter(|row| !row.flags.is_empty()).collect();
    println!(
        "\nloop-tagged: {} (all ext ogg expected), flagged: {}",
        rows.len(),
        flagged.len()
    );
    for row in &flagged {
        println!("  RISK {} [{}]", row.rel, row.flags.join(","));
    }
    if !decode_errors.is_empty() {
        println!("decode errors: {decode_errors:?}");
    }

    assert!(
        decode_errors.is_empty(),
        "loop-tagged files must decode: {decode_errors:?}"
    );
    assert_eq!(
        rows.len(),
        31,
        "expected the retail tree's 31 loop-tagged files"
    );
    for row in &rows {
        assert_eq!(
            row.ext, "ogg",
            "loop tags must be on ogg, got {} ({})",
            row.rel, row.ext
        );
        assert!(
            row.start.is_some(),
            "{} has LOOPLENGTH but no LOOPSTART",
            row.rel
        );
        assert!(
            row.decoded.is_some(),
            "{} has no usable decoded loop region",
            row.rel
        );
        assert!(
            !row.flags.contains(&"START_OOB"),
            "{} loop start is past end of file",
            row.rel
        );
        assert!(
            !row.flags.contains(&"NO_USABLE_LOOP"),
            "{} decoded loop is unusable",
            row.rel
        );
    }

    // The raw values must explain the clamped decoder output exactly.
    for row in &rows {
        let (ls, le) = row.decoded.unwrap();
        let start = row.start.unwrap();
        let expected_end = match row.length {
            Some(len) => start.saturating_add(len).min(row.frames as u64),
            None => row.frames as u64,
        };
        assert_eq!(
            ls, start,
            "{} decoded loop_start differs from LOOPSTART",
            row.rel
        );
        assert_eq!(
            le, expected_end,
            "{} decoded loop_end differs from clamped tag end",
            row.rel
        );
    }
}

#[test]
fn missing_bgm_kana_is_registered_but_not_referenced_by_events() {
    let Some(root) = audio_root() else {
        eprintln!("skipping missing_bgm_kana: audio root not found");
        return;
    };

    let needle = b"BGM/kana.mp3";
    let mut basic_counts: Vec<(String, usize)> = Vec::new();
    for name in [
        "CommonEvent.dat",
        "DataBase.dat",
        "CDataBase.dat",
        "SysDatabase.dat",
    ] {
        let path = root.join("BasicData").join(name);
        let count = fs::read(&path).map_or(0, |bytes| count_sub(&bytes, needle));
        basic_counts.push((name.to_string(), count));
    }
    let mut map_refs = 0usize;
    if let Ok(entries) = fs::read_dir(root.join("MapData")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("mps") {
                if let Ok(bytes) = fs::read(&path) {
                    map_refs += count_sub(&bytes, needle);
                }
            }
        }
    }

    let sys_count = basic_counts
        .iter()
        .find(|(name, _)| name == "SysDatabase.dat")
        .map(|(_, count)| *count)
        .unwrap_or(0);
    println!(
        "\n=== m10 missing BGM/kana.mp3 ===\nbasic counts: {basic_counts:?}\nmap refs: {map_refs}"
    );

    assert_eq!(
        sys_count, 1,
        "BGM/kana.mp3 should appear exactly once in SysDatabase.dat"
    );
    for (name, count) in &basic_counts {
        if name != "SysDatabase.dat" {
            assert_eq!(*count, 0, "unexpected BGM/kana.mp3 mention in {name}");
        }
    }
    assert_eq!(map_refs, 0, "no map script should reference BGM/kana.mp3");
    assert!(
        !root.join("BGM").join("kana.mp3").is_file(),
        "kana.mp3 unexpectedly exists"
    );

    let mut audio_files = Vec::new();
    collect_audio(&root, &mut audio_files);
    let same_name: Vec<String> = audio_files
        .iter()
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.eq_ignore_ascii_case("kana.mp3"))
                .unwrap_or(false)
        })
        .map(|path| rel(&root, path))
        .collect();
    assert!(
        same_name.is_empty(),
        "found a same-name kana.mp3 alternative: {same_name:?}"
    );

    assert!(decode_file(root.join("BGM").join("kana.mp3")).is_err());
    let mut engine = Engine::null();
    engine.set_asset_root(&root);
    assert!(engine.resolve("BGM/kana.mp3").is_none());
    let cmd = SoundCmd {
        channel: SoundChannel::Bgm,
        action: SoundAction::Play,
        file: "BGM/kana.mp3".to_string(),
        volume: 100,
        frames: 0,
    };
    engine.apply_sound_cmd(&cmd);
    assert!(
        !engine.bgm_playing(),
        "a missing BGM must not create a voice"
    );
    let silence = engine.render_offline(128);
    assert!(
        silence.iter().all(|sample| *sample == 0.0),
        "missing BGM must stay silent"
    );
}
