//! M11 audio loop regression (shared task-88).
//!
//! Deterministic, device-free regression over every loop-tagged BGM track in the shipped
//! retail asset tree (default `work/all`, override with `WOLF_AUDIO_ROOT`). It is the
//! executable counterpart of docs/research/M11_AUDIO_ACCEPTANCE.md and
//! docs/research/M11_AUDIO_LOOP_REGRESSION.md.
//!
//! What it locks:
//! * the retail inventory: exactly 31 loop-tagged files (m11_audio_loop_audit.rs pins the same);
//! * the tag -> decoded-region contract for every track: decoded loop_start == LOOPSTART and
//!   decoded loop_end == min(LOOPSTART + LOOPLENGTH, frames);
//! * the per-track seam metrics (seam_abs / step / ratio / rms_jump / dc_delta) against values
//!   measured on the accepted build, so a decoder or asset change that silently worsens a loop
//!   (a bigger click, a louder seam step, more DC) fails instead of drifting;
//! * the per-track advisory flag set (SEAM_ABS / SEAM_RATIO / RMS_JUMP / DC_DELTA / OVERRUN /
//!   LEN_EQUALS_TRACK / SHORT_LOOP), which also fixes each track's tier (A/B/C/D);
//! * determinism: a second full decode pass must reproduce every metric exactly.
//!
//! It also prints (and best-effort writes to `work/out/m11_audio_loop_manifest.md`) a stable
//! listening manifest grouping tracks by tier with the exact human audition checklist.
//!
//! Tolerances are deliberately tight because decode is deterministic for a fixed asset set:
//! seam_abs / rms_jump / dc_delta use an absolute epsilon of 5e-4 (well below any audible
//! change but above f32/print rounding), ratio uses 0.02, and step uses 20% + 2e-5 (step is a
//! small normaliser whose absolute magnitude is tiny). To legitimately change a loop, measure
//! the new value, update this file's EXPECTED row and docs/research/M11_AUDIO_LOOP_REGRESSION.md
//! together, and state why in the commit.

const EXPECTED_LOOP_TAGGED: usize = 31;

use std::fs;
use std::path::{Path, PathBuf};

use wolf_audio::decode::{decode_file, DecodedAudio};

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

/// One accepted per-track baseline. Values are measured on the accepted build; see
/// docs/research/M11_AUDIO_LOOP_REGRESSION.md for the table and threshold rationale.
struct Expected {
    rel: &'static str,
    start: u64,
    len: Option<u64>,
    frames: usize,
    seam_abs: f32,
    step: f32,
    ratio: f32,
    rms_jump: f32,
    dc_delta: f32,
    tier: &'static str,
    flags: &'static str,
}

#[rustfmt::skip]
const EXPECTED: &[Expected] = &[
    Expected { rel: "BGM/Acoustic_High_Collar.ogg", start: 1360911, len: Some(2589511), frames: 2589511, seam_abs: 0.0195, step: 0.0010, ratio: 20.02, rms_jump: 0.0830, dc_delta: 0.0005, tier: "A", flags: "SEAM_RATIO,OVERRUN,LEN_EQUALS_TRACK" },
    Expected { rel: "BGM/Across_-Battle_Results-.ogg", start: 2357360, len: Some(617983), frames: 2975616, seam_abs: 0.0174, step: 0.0241, ratio: 0.72, rms_jump: 0.2047, dc_delta: 0.0129, tier: "C", flags: "RMS_JUMP" },
    Expected { rel: "BGM/Be_yourself.ogg", start: 1477927, len: Some(2189792), frames: 4620348, seam_abs: 0.0272, step: 0.0235, ratio: 1.16, rms_jump: 0.2885, dc_delta: 0.0001, tier: "C", flags: "RMS_JUMP" },
    Expected { rel: "BGM/Die_Letzte_Revolution.ogg", start: 811014, len: Some(2613192), frames: 3904128, seam_abs: 0.3626, step: 0.0501, ratio: 7.23, rms_jump: 0.1021, dc_delta: 0.0002, tier: "A", flags: "SEAM_ABS,RMS_JUMP" },
    Expected { rel: "BGM/InstaVista.ogg", start: 845923, len: Some(2771822), frames: 4000896, seam_abs: 0.3884, step: 0.0866, ratio: 4.49, rms_jump: 0.0337, dc_delta: 0.0200, tier: "A", flags: "SEAM_ABS" },
    Expected { rel: "BGM/Killer_Roll.ogg", start: 1246266, len: Some(6135677), frames: 7941823, seam_abs: 0.1988, step: 0.0773, ratio: 2.57, rms_jump: 0.1397, dc_delta: 0.0078, tier: "C", flags: "RMS_JUMP" },
    Expected { rel: "BGM/Migratory_Birds.ogg", start: 349294, len: Some(2753501), frames: 4696704, seam_abs: 0.1026, step: 0.0272, ratio: 3.77, rms_jump: 0.0339, dc_delta: 0.0030, tier: "D", flags: "" },
    Expected { rel: "BGM/Our_Construction.ogg", start: 2806, len: Some(4491648), frames: 4491648, seam_abs: 0.5524, step: 0.0281, ratio: 19.66, rms_jump: 0.0365, dc_delta: 0.0095, tier: "A", flags: "SEAM_ABS,SEAM_RATIO,OVERRUN,LEN_EQUALS_TRACK" },
    Expected { rel: "BGM/RAIN_&_Co.ogg", start: 3737433, len: Some(5058894), frames: 8896785, seam_abs: 0.3204, step: 0.1117, ratio: 2.87, rms_jump: 0.1809, dc_delta: 0.0096, tier: "A", flags: "SEAM_ABS,RMS_JUMP" },
    Expected { rel: "BGM/Sunshine!.ogg", start: 358273, len: Some(6417034), frames: 7843968, seam_abs: 0.2290, step: 0.0665, ratio: 3.44, rms_jump: 0.1182, dc_delta: 0.0017, tier: "A", flags: "SEAM_ABS,RMS_JUMP" },
    Expected { rel: "BGM/Tenseness.ogg", start: 142664, len: Some(4961751), frames: 5431680, seam_abs: 0.4660, step: 0.0499, ratio: 9.34, rms_jump: 0.0566, dc_delta: 0.0005, tier: "A", flags: "SEAM_ABS,SEAM_RATIO" },
    Expected { rel: "BGM/The_End_of_Destruction.ogg", start: 577941, len: Some(6335211), frames: 6913152, seam_abs: 0.2800, step: 0.0439, ratio: 6.37, rms_jump: 0.1448, dc_delta: 0.0144, tier: "A", flags: "SEAM_ABS,RMS_JUMP" },
    Expected { rel: "BGM/We_are_Going.ogg", start: 799430, len: Some(7855485), frames: 7855485, seam_abs: 0.1707, step: 0.0259, ratio: 6.59, rms_jump: 0.0769, dc_delta: 0.0136, tier: "B", flags: "OVERRUN,LEN_EQUALS_TRACK" },
    Expected { rel: "BGM/ango.ogg", start: 1485817, len: Some(8304383), frames: 17262720, seam_abs: 0.1159, step: 0.1159, ratio: 1.00, rms_jump: 0.0443, dc_delta: 0.0009, tier: "D", flags: "" },
    Expected { rel: "BGM/autumncome.ogg", start: 822589, len: Some(1693274), frames: 2515863, seam_abs: 0.1946, step: 0.0202, ratio: 9.66, rms_jump: 0.1212, dc_delta: 0.0004, tier: "A", flags: "SEAM_RATIO,RMS_JUMP" },
    Expected { rel: "BGM/centennial_loop.ogg", start: 240565, len: Some(2886521), frames: 5917423, seam_abs: 0.3019, step: 0.0294, ratio: 10.27, rms_jump: 0.0867, dc_delta: 0.0059, tier: "A", flags: "SEAM_ABS,SEAM_RATIO" },
    Expected { rel: "BGM/destiny.ogg", start: 56201, len: Some(6943141), frames: 6999342, seam_abs: 0.4619, step: 0.0407, ratio: 11.35, rms_jump: 0.0832, dc_delta: 0.0016, tier: "A", flags: "SEAM_ABS,SEAM_RATIO" },
    Expected { rel: "BGM/line.ogg", start: 2058, len: Some(6974244), frames: 7099776, seam_abs: 0.0064, step: 0.0097, ratio: 0.66, rms_jump: 0.2517, dc_delta: 0.0062, tier: "C", flags: "RMS_JUMP" },
    Expected { rel: "BGM/madoromu2.ogg", start: 709878, len: Some(3441961), frames: 7596288, seam_abs: 0.0516, step: 0.0171, ratio: 3.02, rms_jump: 0.1145, dc_delta: 0.0158, tier: "C", flags: "RMS_JUMP" },
    Expected { rel: "BGM/morning.ogg", start: 558229, len: Some(2731328), frames: 3573504, seam_abs: 0.0115, step: 0.0025, ratio: 4.57, rms_jump: 0.1274, dc_delta: 0.0001, tier: "C", flags: "RMS_JUMP" },
    Expected { rel: "BGM/nemesis_loop.ogg", start: 4061036, len: Some(3681423), frames: 8110491, seam_abs: 0.8079, step: 0.0411, ratio: 19.66, rms_jump: 0.0278, dc_delta: 0.0017, tier: "A", flags: "SEAM_ABS,SEAM_RATIO" },
    Expected { rel: "BGM/okiraku.ogg", start: 759270, len: Some(8835347), frames: 9731059, seam_abs: 0.0423, step: 0.0319, ratio: 1.33, rms_jump: 0.1136, dc_delta: 0.0037, tier: "C", flags: "RMS_JUMP" },
    Expected { rel: "BGM/p_egg_loop.ogg", start: 179398, len: Some(2332052), frames: 4662341, seam_abs: 0.0943, step: 0.0427, ratio: 2.21, rms_jump: 0.3772, dc_delta: 0.0228, tier: "C", flags: "RMS_JUMP" },
    Expected { rel: "BGM/soul_engage_loop.ogg", start: 425697, len: Some(3877316), frames: 7761171, seam_abs: 0.6304, step: 0.0572, ratio: 11.02, rms_jump: 0.1910, dc_delta: 0.0058, tier: "A", flags: "SEAM_ABS,SEAM_RATIO,RMS_JUMP" },
    Expected { rel: "BGM/straightcontrast.ogg", start: 2148554, len: Some(3686522), frames: 7966080, seam_abs: 0.4450, step: 0.0319, ratio: 13.97, rms_jump: 0.0833, dc_delta: 0.0131, tier: "A", flags: "SEAM_ABS,SEAM_RATIO" },
    Expected { rel: "BGM/waste_loop.ogg", start: 330750, len: Some(3628812), frames: 7891154, seam_abs: 0.2520, step: 0.0455, ratio: 5.54, rms_jump: 0.0645, dc_delta: 0.0027, tier: "A", flags: "SEAM_ABS" },
    Expected { rel: "BGM/windletter.ogg", start: 781072, len: Some(5438387), frames: 6590592, seam_abs: 0.1610, step: 0.0100, ratio: 16.16, rms_jump: 0.1331, dc_delta: 0.0011, tier: "A", flags: "SEAM_RATIO,RMS_JUMP" },
    Expected { rel: "DLC_data/03 2NEONS.ogg", start: 1604110, len: Some(3953345), frames: 5674070, seam_abs: 0.1949, step: 0.1011, ratio: 1.93, rms_jump: 0.1491, dc_delta: 0.0048, tier: "C", flags: "RMS_JUMP" },
    Expected { rel: "DLC_data/bgm01.ogg", start: 4569648, len: Some(4147911), frames: 8779778, seam_abs: 0.8068, step: 0.0630, ratio: 12.81, rms_jump: 0.1574, dc_delta: 0.0140, tier: "A", flags: "SEAM_ABS,SEAM_RATIO,RMS_JUMP" },
    Expected { rel: "DLC_data/bgm02.ogg", start: 3950132, len: Some(5202580), frames: 9196465, seam_abs: 0.0774, step: 0.0257, ratio: 3.02, rms_jump: 0.3704, dc_delta: 0.0110, tier: "C", flags: "RMS_JUMP" },
    Expected { rel: "DLC_data/bgm03.ogg", start: 681867, len: Some(5021630), frames: 5706642, seam_abs: 0.0247, step: 0.0089, ratio: 2.77, rms_jump: 0.0671, dc_delta: 0.0000, tier: "D", flags: "" },
];

/// A freshly measured track (same metric definitions as m11_audio_loop_audit.rs).
struct Measured {
    rel: String,
    start: Option<u64>,
    length: Option<u64>,
    frames: usize,
    decoded: Option<(u64, u64)>,
    seam_abs: f32,
    step: f32,
    ratio: f32,
    rms_jump: f32,
    dc_delta: f32,
    flags: Vec<&'static str>,
}

fn measure(root: &Path, path: &Path) -> Option<Measured> {
    let Ok(bytes) = fs::read(path) else {
        return None;
    };
    let start = raw_tag(&bytes, b"LOOPSTART");
    let length = raw_tag(&bytes, b"LOOPLENGTH");
    if start.is_none() && length.is_none() {
        return None;
    }
    let audio = decode_file(path).expect("loop-tagged file must decode");
    let frames = audio.frames();
    let decoded = match (audio.loop_start, audio.loop_end) {
        (Some(ls), Some(le)) => Some((ls, le)),
        _ => None,
    };

    let mut flags: Vec<&'static str> = Vec::new();
    let (mut seam, mut step, mut ratio, mut rms_jump, mut dc_delta) =
        (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);

    if let Some((ls, le)) = decoded {
        let (ls, le) = (ls as usize, le as usize);
        let loop_secs = if audio.sample_rate > 0 {
            (le - ls) as f32 / audio.sample_rate as f32
        } else {
            0.0
        };
        seam = seam_abs(&audio, ls, le);
        step = (mean_abs_step(&audio, ls, (ls + 1024).min(le))
            + mean_abs_step(&audio, le.saturating_sub(1024), le))
            * 0.5;
        ratio = if step > 0.0 { seam / step } else { 0.0 };

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
        if start.is_some_and(|v| v >= frames as u64) {
            flags.push("START_OOB");
        }
    } else {
        flags.push("NO_USABLE_LOOP");
    }

    Some(Measured {
        rel: rel(root, path),
        start,
        length,
        frames,
        decoded,
        seam_abs: seam,
        step,
        ratio,
        rms_jump,
        dc_delta,
        flags,
    })
}

fn close(rel: &str, name: &str, actual: f32, expected: f32, abs_tol: f32, rel_tol: f32) {
    let diff = (actual - expected).abs();
    let bound = abs_tol + rel_tol * expected.abs();
    assert!(
        diff <= bound,
        "{rel} {name}: actual {actual} vs accepted {expected} (|d|={diff} > {bound}); a loop metric moved beyond tolerance -- remeasure and update EXPECTED + docs/research/M11_AUDIO_LOOP_REGRESSION.md if the change is intentional"
    );
}

#[test]
fn m11_audio_loop_regression() {
    let Some(root) = audio_root() else {
        eprintln!("skipping m11_audio_loop_regression: audio root not found (set WOLF_AUDIO_ROOT)");
        return;
    };

    let mut files = Vec::new();
    collect_audio(&root, &mut files);
    files.sort();

    let mut rows: Vec<Measured> = Vec::new();
    for path in &files {
        if let Some(row) = measure(&root, path) {
            rows.push(row);
        }
    }
    rows.sort_by(|a, b| a.rel.cmp(&b.rel));

    // ---- inventory: same count and same names, in sorted order -----------------
    assert_eq!(
        rows.len(),
        EXPECTED.len(),
        "expected {} loop-tagged tracks, found {}; update EXPECTED and the regression doc if the retail asset set changed",
        EXPECTED.len(),
        rows.len()
    );
    assert_eq!(
        EXPECTED_LOOP_TAGGED,
        EXPECTED.len(),
        "EXPECTED table and count disagree"
    );

    for (r, e) in rows.iter().zip(EXPECTED.iter()) {
        assert_eq!(
            r.rel, e.rel,
            "track order/name drifted; expected {} got {}",
            e.rel, r.rel
        );

        // ---- tag -> decoded region contract --------------------------------
        let (ls, le) = r
            .decoded
            .unwrap_or_else(|| panic!("{} has no usable decoded loop region", r.rel));
        assert_eq!(r.start, Some(e.start), "{} LOOPSTART drifted", r.rel);
        assert_eq!(r.length, e.len, "{} LOOPLENGTH drifted", r.rel);
        assert_eq!(r.frames, e.frames, "{} decoded frame count drifted", r.rel);
        let expect_le = match e.len {
            Some(len) => e.start.saturating_add(len).min(e.frames as u64),
            None => e.frames as u64,
        };
        assert_eq!(ls, e.start, "{} decoded loop_start != LOOPSTART", r.rel);
        assert_eq!(
            le, expect_le,
            "{} decoded loop_end != min(LOOPSTART+LOOPLENGTH, frames)",
            r.rel
        );

        // ---- per-track metric pins -----------------------------------------
        close(&r.rel, "seam_abs", r.seam_abs, e.seam_abs, 5e-4, 0.0);
        close(&r.rel, "step", r.step, e.step, 2e-5, 0.20);
        close(&r.rel, "ratio", r.ratio, e.ratio, 0.02, 0.0);
        close(&r.rel, "rms_jump", r.rms_jump, e.rms_jump, 5e-4, 0.0);
        close(&r.rel, "dc_delta", r.dc_delta, e.dc_delta, 5e-4, 0.0);

        // ---- advisory flag set / tier lock ---------------------------------
        let actual_flags = r.flags.join(",");
        assert_eq!(
            actual_flags, e.flags,
            "{} flags drifted: actual [{}] accepted [{}]",
            r.rel, actual_flags, e.flags
        );
        for v in [r.seam_abs, r.step, r.ratio, r.rms_jump, r.dc_delta] {
            assert!(v.is_finite(), "{} has a non-finite metric", r.rel);
        }
    }

    // ---- determinism: a second full decode pass reproduces every metric -------
    let mut rows2: Vec<Measured> = Vec::new();
    for path in &files {
        if let Some(row) = measure(&root, path) {
            rows2.push(row);
        }
    }
    rows2.sort_by(|a, b| a.rel.cmp(&b.rel));
    assert_eq!(
        rows.len(),
        rows2.len(),
        "second pass changed the track count"
    );
    for (a, b) in rows.iter().zip(rows2.iter()) {
        assert_eq!(a.rel, b.rel, "second pass reordered tracks");
        assert_eq!(
            a.seam_abs.to_bits(),
            b.seam_abs.to_bits(),
            "{} seam_abs not deterministic",
            a.rel
        );
        assert_eq!(
            a.rms_jump.to_bits(),
            b.rms_jump.to_bits(),
            "{} rms_jump not deterministic",
            a.rel
        );
        assert_eq!(
            a.ratio.to_bits(),
            b.ratio.to_bits(),
            "{} ratio not deterministic",
            a.rel
        );
        assert_eq!(
            a.dc_delta.to_bits(),
            b.dc_delta.to_bits(),
            "{} dc_delta not deterministic",
            a.rel
        );
    }

    // ---- stable listening manifest ------------------------------------------
    let mut manifest = String::new();
    manifest.push_str("# M11 audio loop listening manifest\n\n");
    manifest.push_str(
        "Generated deterministically by crates/wolf-audio/tests/m11_audio_loop_regression.rs.\n",
    );
    manifest
        .push_str("Tier A = objective seam defect (seam_abs>=0.20 or ratio>=8) -> must audition; ");
    manifest.push_str("B = metadata anomaly (OVERRUN/LEN_EQUALS_TRACK); C = loudness/DC step (rms_jump>=0.10 or dc_delta>=0.05); ");
    manifest.push_str("D = no objective flag.\n\n");
    manifest.push_str("## Tracks by tier\n\n");
    for tier in ["A", "B", "C", "D"] {
        let group: Vec<&Expected> = EXPECTED.iter().filter(|e| e.tier == tier).collect();
        manifest.push_str(&format!("### Tier {tier} ({} track(s))\n", group.len()));
        for e in group {
            manifest.push_str(&format!(
                "- `{}`  seam_abs={:.4} step={:.4} ratio={:.2} rms_jump={:.4} dc_delta={:.4}  flags=[{}]\n",
                e.rel, e.seam_abs, e.step, e.ratio, e.rms_jump, e.dc_delta, e.flags
            ));
        }
        manifest.push('\n');
    }
    manifest.push_str("## Human audition checklist\n\n");
    manifest.push_str("1. For every Tier A track, loop it and listen for a click/pop at the wrap; note the exact track.\n");
    manifest.push_str(
        "2. For every Tier C track, listen for a loudness or DC step at the loop point.\n",
    );
    manifest.push_str("3. For every OVERRUN / LEN_EQUALS_TRACK track, confirm the loop length matches the audible musical phrase.\n");
    manifest.push_str("4. Confirm no track audible silence gap at the seam.\n");
    manifest.push_str(
        "5. Record results in docs/research/M11_AUDIO_LOOP_REGRESSION.md (human sign-off table).\n",
    );

    println!(
        "\n=== M11 audio loop regression: {} loop-tagged track(s), all pinned ===\n",
        rows.len()
    );
    println!("{manifest}");

    let out =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../work/out/m11_audio_loop_manifest.md");
    if let Some(parent) = out.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(err) = fs::write(&out, &manifest) {
        eprintln!("note: could not write {}: {err}", out.display());
    } else {
        println!("wrote {}", out.display());
    }

    // ---- tier distribution sanity (stable, independent of file order) --------
    let count = |t: &str| EXPECTED.iter().filter(|e| e.tier == t).count();
    println!(
        "tier distribution: A={} B={} C={} D={}",
        count("A"),
        count("B"),
        count("C"),
        count("D")
    );
}
