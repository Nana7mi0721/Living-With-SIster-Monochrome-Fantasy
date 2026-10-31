//! Decoding layer: turn an audio file (or in-memory bytes) into interleaved f32 PCM.
//!
//! Uses Symphonia 0.6 with the `ogg`/`vorbis`/`mp3`/`wav`/`pcm` features enabled. Every
//! container sample format is normalised to interleaved `f32` in the range -1.0..=1.0.

use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

use crate::AudioError;

/// Decoded audio, always stored as interleaved `f32` samples.
#[derive(Clone, Debug)]
pub struct DecodedAudio {
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Number of channels (1 = mono, 2 = stereo, ...).
    pub channels: u16,
    /// Interleaved samples: `samples[frame * channels + channel]`.
    pub samples: Vec<f32>,
    /// Optional loop start, in frames (one frame = one sample per channel).
    pub loop_start: Option<u64>,
    /// Optional loop end (exclusive), in frames.
    pub loop_end: Option<u64>,
}

/// Alias so callers can talk about an "audio source" without caring about the concrete type.
pub type AudioSource = DecodedAudio;

impl DecodedAudio {
    /// Number of frames (one frame spans all channels).
    pub fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.samples.len() / self.channels as usize
        }
    }

    /// Duration in seconds.
    pub fn duration_secs(&self) -> f32 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.frames() as f32 / self.sample_rate as f32
        }
    }

    /// True when Vorbis-comment loop points were found and are usable.
    pub fn has_loop_points(&self) -> bool {
        match (self.loop_start, self.loop_end) {
            (Some(s), Some(e)) => e > s && (s as usize) < self.frames(),
            _ => false,
        }
    }

    /// Fetch one sample for `frame` / `channel`, clamped to valid bounds. Mono sources are
    /// fanned out to every output channel; extra output channels are clamped onto the last
    /// available source channel.
    pub fn sample(&self, frame: usize, channel: usize) -> f32 {
        let ch = self.channels as usize;
        if ch == 0 {
            return 0.0;
        }
        let frame = frame.min(self.frames().saturating_sub(1));
        let src_ch = if ch == 1 { 0 } else { channel.min(ch - 1) };
        self.samples[frame * ch + src_ch]
    }
}

/// Decode an audio file from disk.
///
/// Symphonia is the primary decoder. If it cannot even *probe* the container, one
/// narrow retry is attempted for RIFF/WAVE PCM files whose fmt chunk is
/// non-canonical (see try_decode_wav_pcm_tolerant); every other error is returned
/// unchanged, so genuinely invalid files are still rejected.
pub fn decode_file(path: impl AsRef<Path>) -> Result<DecodedAudio, AudioError> {
    let path = path.as_ref();
    let file = File::open(path)
        .map_err(|e| AudioError::Io(format!("open {}: {e}", path.display())))?;
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    match decode_source(Box::new(file), &hint) {
        Ok(audio) => Ok(audio),
        Err(AudioError::Unsupported(msg)) if is_probe_failure(&msg) => {
            let bytes = std::fs::read(path)
                .map_err(|e| AudioError::Io(format!("read {}: {e}", path.display())))?;
            match try_decode_wav_pcm_tolerant(&bytes) {
                Some(retry) => retry,
                None => Err(AudioError::Unsupported(msg)),
            }
        }
        Err(err) => Err(err),
    }
}

/// Decode audio from an in-memory byte buffer (format probed by content).
pub fn decode_bytes(bytes: Vec<u8>) -> Result<DecodedAudio, AudioError> {
    match decode_source(Box::new(std::io::Cursor::new(bytes.clone())), &Hint::new()) {
        Ok(audio) => Ok(audio),
        Err(AudioError::Unsupported(msg)) if is_probe_failure(&msg) => {
            match try_decode_wav_pcm_tolerant(&bytes) {
                Some(retry) => retry,
                None => Err(AudioError::Unsupported(msg)),
            }
        }
        Err(err) => Err(err),
    }
}

/// True when a decode error came from the container probe (not from decoding frames).
fn is_probe_failure(msg: &str) -> bool {
    msg.starts_with("probe:")
}

fn decode_source(
    source: Box<dyn MediaSource>,
    hint: &Hint,
) -> Result<DecodedAudio, AudioError> {
    let mss = MediaSourceStream::new(source, Default::default());

    let mut format: Box<dyn FormatReader> = symphonia::default::get_probe()
        .probe(
            hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| AudioError::Unsupported(format!("probe: {e}")))?;

    let (loop_start, loop_length) = read_loop_tags(&mut format);

    let (track_id, params) = {
        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| AudioError::Unsupported("no audio track found".into()))?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .cloned()
            .ok_or_else(|| AudioError::Unsupported("track has no audio codec parameters".into()))?;
        (track.id, params)
    };

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| AudioError::Unsupported(format!("decoder: {e}")))?;

    let mut samples: Vec<f32> = Vec::new();
    let mut scratch: Vec<f32> = Vec::new();
    let mut sample_rate = params.sample_rate.unwrap_or(0);
    let mut channels = params.channels.as_ref().map(|c| c.count()).unwrap_or(0);

    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            // Any read error after probing is treated as end-of-stream: whatever decoded so far
            // is returned. Malformed tail data should not discard a mostly-good track.
            Err(_) => break,
        };

        if packet.track_id != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(buf) => {
                if buf.frames() == 0 {
                    continue;
                }
                let spec = buf.spec();
                if sample_rate == 0 {
                    sample_rate = spec.rate();
                }
                channels = spec.channels().count();
                scratch.resize(buf.samples_interleaved(), 0.0);
                buf.copy_to_slice_interleaved(&mut scratch);
                samples.extend_from_slice(&scratch);
            }
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(SymphoniaError::ResetRequired) => continue,
            Err(e) => return Err(AudioError::Unsupported(format!("decode: {e}"))),
        }
    }

    if samples.is_empty() || channels == 0 || sample_rate == 0 {
        return Err(AudioError::Empty("no audio frames decoded".into()));
    }

    let total_frames = samples.len() / channels;
    let (mut ls, mut le) = (None, None);
    if let Some(start) = loop_start {
        let start = start as usize;
        if start < total_frames {
            let end = loop_length
                .map(|len| start.saturating_add(len as usize))
                .unwrap_or(total_frames)
                .min(total_frames);
            if end > start {
                ls = Some(start as u64);
                le = Some(end as u64);
            }
        }
    }

    Ok(DecodedAudio {
        sample_rate,
        channels: channels as u16,
        samples,
        loop_start: ls,
        loop_end: le,
    })
}

/// Read `LOOPSTART` / `LOOPLENGTH` Vorbis-comment tags (in frames) from the container metadata.
///
/// Returns `(loop_start, loop_length)` in frames. Handles keys with irregular spacing/case,
/// integer values (the common case) and both media-level and per-track tag containers.
fn read_loop_tags(format: &mut Box<dyn FormatReader>) -> (Option<u64>, Option<u64>) {
    let mut start = None;
    let mut length = None;

    let metadata = format.metadata();
    if let Some(revision) = metadata.current() {
        for tag in &revision.media.tags {
            scan_tag(tag, &mut start, &mut length);
        }
        for per_track in &revision.per_track {
            for tag in &per_track.metadata.tags {
                scan_tag(tag, &mut start, &mut length);
            }
        }
    }
    (start, length)
}

fn scan_tag(tag: &symphonia::core::meta::Tag, start: &mut Option<u64>, length: &mut Option<u64>) {
    let key = tag.raw.key.trim().to_ascii_uppercase();
    let value = tag.raw.value.to_string();
    let value = value.trim();
    let parsed = value.parse::<u64>().ok().or_else(|| {
        value
            .split_whitespace()
            .last()
            .and_then(|token| token.parse::<u64>().ok())
    });
    match key.as_str() {
        "LOOPSTART" => *start = parsed,
        "LOOPLENGTH" => *length = parsed,
        _ => {}
    }
}

/// Canonical PCM fields read out of a WAVE `fmt ` chunk.
struct WavFmt {
    audio_format: u16,
    channels: u16,
    sample_rate: u32,
    block_align: u16,
    bits: u16,
}

/// Strict, narrow fallback for RIFF/WAVE PCM files that Symphonia rejects at probe time
/// only because the `fmt ` chunk size is non-canonical (e.g. 26 bytes instead of 16/18)
/// while the canonical 16-byte PCM header fields are still present and intact.
///
/// Returns:
/// * `None` when the bytes are not a RIFF/WAVE stream, or are RIFF/WAVE but not integer
///   PCM -- the caller keeps the original Symphonia error (no weakening).
/// * `Some(Err(..))` when the stream claims to be RIFF/WAVE PCM but is malformed/truncated.
/// * `Some(Ok(..))` when a fully validated PCM stream was decoded.
fn try_decode_wav_pcm_tolerant(bytes: &[u8]) -> Option<Result<DecodedAudio, AudioError>> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }

    let mut fmt: Option<WavFmt> = None;
    // (data body start, clamped end, declared size)
    let mut data: Option<(usize, usize, usize)> = None;

    let mut pos = 12usize;
    while pos + 8 <= bytes.len() {
        let size = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        let body = pos + 8;
        match &bytes[pos..pos + 4] {
            b"fmt " => {
                if bytes.len().saturating_sub(body) < 16 {
                    return Some(Err(AudioError::Unsupported(
                        "wav: fmt chunk shorter than the canonical 16-byte PCM header".into(),
                    )));
                }
                fmt = Some(WavFmt {
                    audio_format: u16::from_le_bytes([bytes[body], bytes[body + 1]]),
                    channels: u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]),
                    sample_rate: u32::from_le_bytes([
                        bytes[body + 4],
                        bytes[body + 5],
                        bytes[body + 6],
                        bytes[body + 7],
                    ]),
                    block_align: u16::from_le_bytes([bytes[body + 12], bytes[body + 13]]),
                    bits: u16::from_le_bytes([bytes[body + 14], bytes[body + 15]]),
                });
            }
            b"data" => {
                let declared_end = body.saturating_add(size);
                let end = declared_end.min(bytes.len());
                data = Some((body, end, size));
            }
            _ => {}
        }
        // RIFF chunks are word-aligned: advance by the declared size plus pad byte.
        let advance = 8usize.saturating_add(size).saturating_add(size & 1);
        pos = pos.saturating_add(advance);
        if pos <= body {
            break;
        }
    }

    let Some(fmt) = fmt else {
        return Some(Err(AudioError::Unsupported("wav: missing fmt chunk".into())));
    };
    let Some((data_start, data_end, data_size)) = data else {
        return Some(Err(AudioError::Unsupported("wav: missing data chunk".into())));
    };

    // Only integer PCM (audioFormat == 1) is handled here. Other codecs are left to
    // Symphonia so we never fabricate a result for something we cannot decode.
    if fmt.audio_format != 1 {
        return None;
    }
    if fmt.channels == 0 || fmt.sample_rate == 0 || fmt.bits == 0 {
        return Some(Err(AudioError::Unsupported(
            "wav: zero channels / sample-rate / bit-depth".into(),
        )));
    }
    if !matches!(fmt.bits, 8 | 16 | 24 | 32) {
        return Some(Err(AudioError::Unsupported(format!(
            "wav: unsupported PCM bit depth {}",
            fmt.bits
        ))));
    }
    let bytes_per_sample = (fmt.bits / 8) as usize;
    let frame_bytes = (fmt.channels as usize) * bytes_per_sample;
    if frame_bytes == 0 || fmt.block_align as usize != frame_bytes {
        return Some(Err(AudioError::Unsupported(
            "wav: blockAlign inconsistent with channels / bit-depth".into(),
        )));
    }
    // A declared data chunk that runs past the end of the buffer is truncated data.
    if data_end < data_start.saturating_add(data_size) {
        return Some(Err(AudioError::Unsupported(
            "wav: truncated data chunk".into(),
        )));
    }
    if data_size % frame_bytes != 0 {
        return Some(Err(AudioError::Unsupported(
            "wav: data chunk length is not a multiple of blockAlign".into(),
        )));
    }
    let frames = data_size / frame_bytes;
    if frames == 0 {
        return Some(Err(AudioError::Empty(
            "wav: data chunk produced no complete frames".into(),
        )));
    }

    let mut samples = Vec::with_capacity(frames * fmt.channels as usize);
    for f in 0..frames {
        for c in 0..fmt.channels as usize {
            let off = data_start + f * frame_bytes + c * bytes_per_sample;
            samples.push(decode_pcm_sample(bytes, off, fmt.bits));
        }
    }

    Some(Ok(DecodedAudio {
        sample_rate: fmt.sample_rate,
        channels: fmt.channels,
        samples,
        loop_start: None,
        loop_end: None,
    }))
}

/// Convert one integer PCM sample at `off` to normalised f32 in -1.0..=1.0.
fn decode_pcm_sample(bytes: &[u8], off: usize, bits: u16) -> f32 {
    match bits {
        8 => (bytes[off] as f32 - 128.0) / 128.0,
        16 => i16::from_le_bytes([bytes[off], bytes[off + 1]]) as f32 / 32_768.0,
        24 => {
            let v = (bytes[off] as i32)
                | ((bytes[off + 1] as i32) << 8)
                | ((bytes[off + 2] as i32) << 16);
            ((v << 8) >> 8) as f32 / 8_388_608.0
        }
        32 => {
            i32::from_le_bytes([
                bytes[off],
                bytes[off + 1],
                bytes[off + 2],
                bytes[off + 3],
            ]) as f32
                / 2_147_483_648.0
        }
        _ => 0.0,
    }
}
