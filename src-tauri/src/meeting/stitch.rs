//! Stitch overlapping per-chunk transcription results into a single transcript.

use crate::meeting::types::{Segment, SpeakerLabel};

/// A per-chunk result as produced by Whisper, with chunk-local timestamps.
/// `chunk_start_secs` is the chunk's offset within the original audio.
#[derive(Debug, Clone)]
pub struct ChunkResult {
    pub chunk_start_secs: f32,
    pub chunk_end_secs: f32,
    pub segments: Vec<RawSegment>,
}

#[derive(Debug, Clone)]
pub struct RawSegment {
    pub start: f32,  // seconds relative to the chunk start
    pub end: f32,
    pub text: String,
}

const OVERLAP_SECS: f32 = 2.0;

/// Merge chunk results into a single timeline. Drops segments from chunk N+1
/// whose centre falls inside chunk N's overlap region.
pub fn stitch(chunks: &[ChunkResult]) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let prev_end = if i == 0 { 0.0 } else { chunks[i - 1].chunk_end_secs };
        for raw in &chunk.segments {
            let abs_start = chunk.chunk_start_secs + raw.start;
            let abs_end = chunk.chunk_start_secs + raw.end;
            if i > 0 {
                let centre = (abs_start + abs_end) / 2.0;
                if centre < prev_end - OVERLAP_SECS / 2.0 {
                    // Earlier than (prev_end - 1s) — keep
                } else if centre < prev_end {
                    continue;
                }
            }
            out.push(Segment {
                start: abs_start,
                end: abs_end,
                speaker: SpeakerLabel::Other,
                text: raw.text.trim().to_string(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(start: f32, end: f32, text: &str) -> RawSegment {
        RawSegment { start, end, text: text.into() }
    }

    #[test]
    fn no_overlap_keeps_all_segments() {
        let chunks = vec![
            ChunkResult {
                chunk_start_secs: 0.0, chunk_end_secs: 30.0,
                segments: vec![raw(0.0, 5.0, "alpha"), raw(10.0, 15.0, "beta")],
            },
            ChunkResult {
                chunk_start_secs: 28.0, chunk_end_secs: 58.0,
                segments: vec![raw(5.0, 10.0, "gamma")],
            },
        ];
        let out = stitch(&chunks);
        assert_eq!(out.len(), 3);
        assert_eq!(out[2].text, "gamma");
        assert!((out[2].start - 33.0).abs() < 0.001);
    }

    #[test]
    fn segment_in_overlap_region_is_dropped() {
        let chunks = vec![
            ChunkResult {
                chunk_start_secs: 0.0, chunk_end_secs: 30.0,
                segments: vec![raw(28.5, 29.5, "duplicate")],
            },
            ChunkResult {
                chunk_start_secs: 28.0, chunk_end_secs: 58.0,
                segments: vec![raw(0.5, 1.5, "duplicate")],
            },
        ];
        let out = stitch(&chunks);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "duplicate");
    }

    #[test]
    fn empty_chunks_returns_empty() {
        assert!(stitch(&[]).is_empty());
    }
}
