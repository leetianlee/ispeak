//! PCM file → Transcript orchestrator (file-import path, no diarisation in 3.1).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{AppError, Result};
use crate::groq;
use crate::meeting::chunker::{plan_chunks, ChunkBounds};
use crate::meeting::diarise::{diarise, DiariseConfig};
use crate::meeting::ingest::{pcm_file_samples, read_pcm_window};
use crate::meeting::stitch::{stitch, ChunkResult, RawSegment};
use crate::meeting::types::{Transcript, TranscriptSource};
use crate::settings::WhisperModel;
use crate::whisper_engine;

/// Where Whisper-on-each-chunk runs.
pub enum Engine {
    Local { app_data_dir: PathBuf, model: WhisperModel },
    GroqCloud { api_key: String },
}

/// Progress callback: invoked once per chunk completion.
pub trait ProgressSink: Send + Sync {
    fn on_chunk_done(&self, chunks_done: u32, chunks_total: u32);
}

pub struct NoopProgress;
impl ProgressSink for NoopProgress {
    fn on_chunk_done(&self, _: u32, _: u32) {}
}

/// Diarisation knobs passed in from settings. `None` skips diarisation entirely
/// (every segment stays labelled as the original SpeakerLabel::Other).
#[derive(Debug, Clone, Copy)]
pub struct DiariseOpts {
    pub k: u8,
}

/// Run the file-import pipeline. Returns a complete or partial Transcript.
pub async fn run(
    pcm_path: &Path,
    source: TranscriptSource,
    engine: Engine,
    cancel: Arc<AtomicBool>,
    progress: Arc<dyn ProgressSink>,
    diarise_opts: Option<DiariseOpts>,
) -> Result<Transcript> {
    let total_samples = pcm_file_samples(pcm_path)?;
    let chunks = plan_chunks(total_samples);
    let chunks_total = chunks.len() as u32;
    let mut chunk_results: Vec<ChunkResult> = Vec::with_capacity(chunks.len());
    let mut partial = false;

    for bounds in &chunks {
        if cancel.load(Ordering::SeqCst) {
            partial = true;
            break;
        }
        match transcribe_one(pcm_path, bounds, &engine).await {
            Ok(r) => chunk_results.push(r),
            Err(e) => {
                partial = true;
                chunk_results.push(ChunkResult {
                    chunk_start_secs: bounds.start_secs,
                    chunk_end_secs: bounds.end_secs,
                    segments: vec![RawSegment {
                        start: 0.0,
                        end: bounds.end_secs - bounds.start_secs,
                        text: format!("[unintelligible {:.1}-{:.1}s: {}]",
                            bounds.start_secs, bounds.end_secs, e),
                    }],
                });
            }
        }
        progress.on_chunk_done(chunk_results.len() as u32, chunks_total);
    }

    let mut segments = stitch(&chunk_results);
    let duration_secs = total_samples as f32 / 16_000.0;

    // Phase 3.3b: heuristic diarisation. Reads the full PCM into memory, extracts
    // per-turn features, clusters with k-means, and re-labels segments. Non-fatal —
    // any error logs and leaves segments untouched.
    if let Some(opts) = diarise_opts {
        if !partial || !segments.is_empty() {
            match read_pcm_window(pcm_path, 0, total_samples) {
                Ok(samples) => {
                    let cfg = DiariseConfig::for_16k(opts.k);
                    segments = diarise(&samples, segments, cfg);
                }
                Err(e) => {
                    eprintln!("[iSpeak] diarisation skipped — read pcm failed: {e}");
                }
            }
        }
    }

    Ok(Transcript {
        id: uuid::Uuid::new_v4(),
        created_at: now_millis(),
        duration_secs,
        source,
        segments,
        summary: None,
        action_items: vec![],
        partial,
        title: None,
    })
}

async fn transcribe_one(
    pcm_path: &Path,
    bounds: &ChunkBounds,
    engine: &Engine,
) -> Result<ChunkResult> {
    let samples = read_pcm_window(pcm_path, bounds.start_sample, bounds.len_samples)?;
    let segments = match engine {
        Engine::Local { app_data_dir, model } => {
            let s = samples.clone();
            let d = app_data_dir.clone();
            let m = model.clone();
            tokio::task::spawn_blocking(move || whisper_engine::transcribe_segments(&s, &d, &m))
                .await
                .map_err(|e| AppError::Meeting(format!("whisper task join: {e}")))??
                .into_iter()
                .map(|ws| RawSegment { start: ws.start, end: ws.end, text: ws.text })
                .collect()
        }
        Engine::GroqCloud { api_key } => {
            let wav = groq::encode_wav(&samples, 16_000);
            let gsegs = groq::transcribe_verbose(wav, api_key).await?;
            gsegs.into_iter().map(|g| RawSegment { start: g.start, end: g.end, text: g.text }).collect()
        }
    };
    Ok(ChunkResult {
        chunk_start_secs: bounds.start_secs,
        chunk_end_secs: bounds.end_secs,
        segments,
    })
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_millis_is_recent() {
        let n = now_millis();
        let secs = n / 1000;
        assert!(secs > 1_700_000_000);
    }
}
