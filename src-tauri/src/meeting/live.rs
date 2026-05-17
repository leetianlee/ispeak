//! Phase 3.2 — live audio capture.
//!
//! Captures audio from one or more sources during a meeting and writes a 16kHz
//! mono f32-LE PCM file that the existing chunk → Whisper → stitch pipeline can
//! consume directly.
//!
//! Architecture: each source (mic via cpal, system audio via ScreenCaptureKit)
//! runs on its own OS thread and writes RAW native-format samples to a
//! per-source temp file as they arrive. When the user stops, the recorder is
//! finalised: each raw file is read, downmixed to mono, resampled to 16kHz,
//! and (when there are multiple sources) summed sample-by-sample to produce a
//! single PCM file.
//!
//! Why disk-buffer rather than in-memory: a 1-hour meeting at 48kHz stereo f32
//! is ~1.4GB — fine on disk in tempdir, unacceptable as a Vec<f32>.
//!
//! Why per-source raw files instead of online resampling: rubato's chunked
//! resampler requires fixed-size input windows that cpal/SCStream callbacks do
//! not naturally provide. Doing the resample once at the end is far simpler and
//! has equivalent fidelity.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SupportedStreamConfig};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::meeting::ingest;

/// Which audio sources to capture.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LiveSource {
    /// Microphone only — Phase 3.2a. Always available.
    MicOnly,
    /// System audio output only (the other side of a Zoom call etc.) —
    /// Phase 3.2b. macOS-only, requires Screen Recording permission.
    SystemOnly,
    /// Both microphone and system audio, mixed.
    MicAndSystem,
}

/// Metadata captured by one source while it streamed.
pub(crate) struct SourceCaptureMeta {
    /// Raw file: f32-LE samples interleaved by channel at `sample_rate`.
    pub raw_path: PathBuf,
    pub sample_rate: u32,
    pub channels: u16,
}

/// In-flight live recording state. Held until the user stops.
pub struct LiveRecorder {
    job_id: uuid::Uuid,
    stop_flag: Arc<AtomicBool>,
    /// JoinHandles for each source's capture thread. Joined in `stop_and_finalise`.
    handles: Vec<std::thread::JoinHandle<Result<SourceCaptureMeta>>>,
    /// Where the finalised 16k PCM goes when stop is called.
    out_path: PathBuf,
    /// Wall-clock start time of capture (for reporting duration if no samples land).
    started_at: std::time::Instant,
}

impl LiveRecorder {
    /// Start capturing from the requested sources. Returns immediately; capture
    /// threads run in the background until `stop_and_finalise` is called.
    pub fn start(
        job_id: uuid::Uuid,
        source: LiveSource,
        mic_device_id: Option<String>,
        work_dir: PathBuf,
    ) -> Result<Self> {
        std::fs::create_dir_all(&work_dir)
            .map_err(|e| AppError::Meeting(format!("create live work dir: {e}")))?;
        let out_path = work_dir.join(format!("{}.pcm", job_id));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let mut handles = Vec::new();

        let wants_mic = matches!(source, LiveSource::MicOnly | LiveSource::MicAndSystem);
        let wants_system =
            matches!(source, LiveSource::SystemOnly | LiveSource::MicAndSystem);

        if wants_mic {
            let raw = work_dir.join("mic-raw.pcm");
            let flag = stop_flag.clone();
            let device_id = mic_device_id.clone();
            handles.push(std::thread::spawn(move || {
                capture_mic_to_file(device_id.as_deref(), &raw, flag)
            }));
        }

        if wants_system {
            let raw = work_dir.join("system-raw.pcm");
            let flag = stop_flag.clone();
            handles.push(std::thread::spawn(move || {
                capture_system_to_file(&raw, flag)
            }));
        }

        if handles.is_empty() {
            return Err(AppError::Meeting(
                "no audio source selected for live capture".into(),
            ));
        }

        Ok(Self {
            job_id,
            stop_flag,
            handles,
            out_path,
            started_at: std::time::Instant::now(),
        })
    }

    /// Signal all capture threads to stop, join them, and produce a single
    /// 16kHz mono f32-LE PCM file at `self.out_path`. Returns the path.
    pub fn stop_and_finalise(self) -> Result<PathBuf> {
        self.stop_flag.store(true, Ordering::SeqCst);

        let mut metas: Vec<SourceCaptureMeta> = Vec::new();
        for h in self.handles {
            match h.join() {
                Ok(Ok(meta)) => metas.push(meta),
                Ok(Err(e)) => eprintln!("[iSpeak live] source failed: {e}"),
                Err(_) => eprintln!("[iSpeak live] source thread panicked"),
            }
        }
        if metas.is_empty() {
            let elapsed = self.started_at.elapsed().as_secs_f32();
            return Err(AppError::Meeting(format!(
                "live capture produced no audio (recorded for {:.1}s)",
                elapsed
            )));
        }

        // Convert each source to 16k mono f32, then sum them sample-aligned.
        let mut tracks: Vec<Vec<f32>> = Vec::with_capacity(metas.len());
        for m in &metas {
            let track = read_resample_to_16k_mono(&m.raw_path, m.sample_rate, m.channels)?;
            tracks.push(track);
        }
        let mixed = mix_tracks(tracks);

        write_pcm_f32_le(&self.out_path, &mixed)?;
        Ok(self.out_path)
    }

    pub fn job_id(&self) -> uuid::Uuid {
        self.job_id
    }

    /// Best-effort temp dir cleanup. Called after the pipeline has consumed
    /// the finalised PCM.
    pub fn cleanup(work_dir: &Path) {
        let _ = std::fs::remove_dir_all(work_dir);
    }
}

// ─── Mic capture (cpal) ─────────────────────────────────────────────────────

fn capture_mic_to_file(
    device_id: Option<&str>,
    raw_path: &Path,
    stop_flag: Arc<AtomicBool>,
) -> Result<SourceCaptureMeta> {
    let device = pick_input_device(device_id)?;
    let config: SupportedStreamConfig = device
        .default_input_config()
        .map_err(|e| AppError::Audio(e.to_string()))?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let sample_format = config.sample_format();

    let file = std::fs::File::create(raw_path)
        .map_err(|e| AppError::Audio(format!("create raw mic file: {e}")))?;
    let writer = Arc::new(Mutex::new(std::io::BufWriter::new(file)));
    let writer_clone = writer.clone();

    let err_fn = |e| eprintln!("[iSpeak live mic] stream error: {e}");

    let stream = match sample_format {
        SampleFormat::F32 => device
            .build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    if let Ok(mut w) = writer_clone.lock() {
                        let _ = write_f32_samples(&mut *w, data);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| AppError::Audio(e.to_string()))?,
        SampleFormat::I16 => device
            .build_input_stream(
                &config.into(),
                move |data: &[i16], _| {
                    if let Ok(mut w) = writer_clone.lock() {
                        for s in data {
                            let f = *s as f32 / i16::MAX as f32;
                            let _ = write_f32_samples(&mut *w, &[f]);
                        }
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| AppError::Audio(e.to_string()))?,
        SampleFormat::U16 => device
            .build_input_stream(
                &config.into(),
                move |data: &[u16], _| {
                    if let Ok(mut w) = writer_clone.lock() {
                        for s in data {
                            let f = (*s as f32 - 32768.0) / 32768.0;
                            let _ = write_f32_samples(&mut *w, &[f]);
                        }
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| AppError::Audio(e.to_string()))?,
        _ => return Err(AppError::Audio("Unsupported sample format".to_string())),
    };

    stream
        .play()
        .map_err(|e| AppError::Audio(format!("start mic stream: {e}")))?;

    while !stop_flag.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    drop(stream);

    if let Ok(w) = Arc::try_unwrap(writer).map(|m| m.into_inner().unwrap()) {
        use std::io::Write;
        let mut w = w;
        let _ = w.flush();
    }

    Ok(SourceCaptureMeta {
        raw_path: raw_path.to_path_buf(),
        sample_rate,
        channels,
    })
}

fn pick_input_device(device_id: Option<&str>) -> Result<Device> {
    let host = cpal::default_host();
    if let Some(id) = device_id {
        if let Some(d) = host
            .input_devices()
            .map_err(|e| AppError::Audio(e.to_string()))?
            .find(|d| d.name().map(|n| n == id).unwrap_or(false))
        {
            return Ok(d);
        }
    }
    host.default_input_device()
        .ok_or_else(|| AppError::Audio("No input device found".to_string()))
}

// ─── System audio capture (ScreenCaptureKit) ────────────────────────────────

#[cfg(target_os = "macos")]
fn capture_system_to_file(
    raw_path: &Path,
    stop_flag: Arc<AtomicBool>,
) -> Result<SourceCaptureMeta> {
    crate::meeting::live_macos::capture_system_to_file(raw_path, stop_flag)
}

#[cfg(not(target_os = "macos"))]
fn capture_system_to_file(
    _raw_path: &Path,
    _stop_flag: Arc<AtomicBool>,
) -> Result<SourceCaptureMeta> {
    Err(AppError::Meeting(
        "system audio capture is macOS-only in this build".into(),
    ))
}

// ─── Finalisation: read raw, resample, mix, write ───────────────────────────

/// Read a raw PCM file (f32-LE interleaved at `sample_rate` × `channels`),
/// downmix to mono, resample to 16kHz, return the samples.
fn read_resample_to_16k_mono(
    raw_path: &Path,
    sample_rate: u32,
    channels: u16,
) -> Result<Vec<f32>> {
    let bytes = std::fs::read(raw_path)
        .map_err(|e| AppError::Meeting(format!("read raw {}: {e}", raw_path.display())))?;
    let n = bytes.len() / 4;
    let mut interleaved: Vec<f32> = Vec::with_capacity(n);
    for c in bytes.chunks_exact(4) {
        interleaved.push(f32::from_le_bytes([c[0], c[1], c[2], c[3]]));
    }
    let mono = downmix_to_mono(&interleaved, channels as usize);
    if sample_rate == 16_000 {
        return Ok(mono);
    }
    ingest::resample(&mono, sample_rate, 16_000)
}

fn downmix_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Sample-align multiple mono tracks and sum them. Tracks may differ slightly
/// in length (capture start/stop jitter); the result is the length of the
/// longest track, with shorter tracks treated as silence past their end.
/// Output is clipped to [-1.0, 1.0].
fn mix_tracks(tracks: Vec<Vec<f32>>) -> Vec<f32> {
    if tracks.is_empty() {
        return Vec::new();
    }
    if tracks.len() == 1 {
        return tracks.into_iter().next().unwrap();
    }
    let max_len = tracks.iter().map(|t| t.len()).max().unwrap_or(0);
    let mut out = vec![0.0f32; max_len];
    for t in &tracks {
        for (i, &s) in t.iter().enumerate() {
            out[i] += s;
        }
    }
    for s in out.iter_mut() {
        *s = s.clamp(-1.0, 1.0);
    }
    out
}

fn write_pcm_f32_le(path: &Path, samples: &[f32]) -> Result<()> {
    use std::io::Write;
    let file = std::fs::File::create(path)
        .map_err(|e| AppError::Meeting(format!("create pcm {}: {e}", path.display())))?;
    let mut w = std::io::BufWriter::new(file);
    let mut buf = Vec::with_capacity(samples.len() * 4);
    for s in samples {
        buf.extend_from_slice(&s.to_le_bytes());
    }
    w.write_all(&buf)
        .map_err(|e| AppError::Meeting(format!("write pcm: {e}")))?;
    w.flush()
        .map_err(|e| AppError::Meeting(format!("flush pcm: {e}")))?;
    Ok(())
}

pub(crate) fn write_f32_samples<W: std::io::Write>(w: &mut W, samples: &[f32]) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(samples.len() * 4);
    for s in samples {
        buf.extend_from_slice(&s.to_le_bytes());
    }
    w.write_all(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_averages_channels() {
        let stereo = vec![1.0, -1.0, 0.5, -0.5];
        assert_eq!(downmix_to_mono(&stereo, 2), vec![0.0, 0.0]);
    }

    #[test]
    fn mix_clips_sum_above_one() {
        let a = vec![0.6, 0.7];
        let b = vec![0.6, 0.7];
        let m = mix_tracks(vec![a, b]);
        assert!(m.iter().all(|&s| s <= 1.0 && s >= -1.0));
    }

    #[test]
    fn mix_pads_shorter_track_with_silence() {
        let a = vec![0.5, 0.5, 0.5];
        let b = vec![0.5];
        let m = mix_tracks(vec![a, b]);
        assert_eq!(m.len(), 3);
        assert!((m[0] - 1.0).abs() < 1e-6);
        assert!((m[1] - 0.5).abs() < 1e-6);
        assert!((m[2] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn write_then_read_pcm_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.pcm");
        let samples = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        write_pcm_f32_le(&path, &samples).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), samples.len() * 4);
    }

    #[test]
    fn single_track_passes_through() {
        let t = vec![0.1, 0.2, 0.3];
        let m = mix_tracks(vec![t.clone()]);
        assert_eq!(m, t);
    }
}
