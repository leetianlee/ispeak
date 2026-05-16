//! Split a PCM stream into overlapping windows for chunked transcription.

const SAMPLE_RATE: usize = 16_000;
const WINDOW_SECS: f32 = 30.0;
const OVERLAP_SECS: f32 = 2.0;

/// One chunk's bounds within the source PCM file.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkBounds {
    pub index: usize,
    pub start_sample: usize,
    pub len_samples: usize,
    pub start_secs: f32,
    pub end_secs: f32,
}

pub fn plan_chunks(total_samples: usize) -> Vec<ChunkBounds> {
    let window = (WINDOW_SECS * SAMPLE_RATE as f32) as usize;
    let overlap = (OVERLAP_SECS * SAMPLE_RATE as f32) as usize;
    let step = window - overlap;
    if total_samples == 0 {
        return vec![];
    }
    let mut out = Vec::new();
    let mut start = 0_usize;
    let mut idx = 0_usize;
    loop {
        let end = (start + window).min(total_samples);
        let len = end - start;
        out.push(ChunkBounds {
            index: idx,
            start_sample: start,
            len_samples: len,
            start_secs: start as f32 / SAMPLE_RATE as f32,
            end_secs: end as f32 / SAMPLE_RATE as f32,
        });
        if end >= total_samples {
            break;
        }
        start += step;
        idx += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_produces_no_chunks() {
        assert!(plan_chunks(0).is_empty());
    }

    #[test]
    fn under_one_window_produces_one_chunk() {
        let samples = 10 * SAMPLE_RATE; // 10s
        let chunks = plan_chunks(samples);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_sample, 0);
        assert_eq!(chunks[0].len_samples, samples);
    }

    #[test]
    fn ninety_seconds_produces_four_chunks_with_overlap() {
        let samples = 90 * SAMPLE_RATE;
        let chunks = plan_chunks(samples);
        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0].start_secs, 0.0);
        assert!((chunks[1].start_secs - 28.0).abs() < 0.001);
        assert!((chunks[2].start_secs - 56.0).abs() < 0.001);
        assert!((chunks[3].start_secs - 84.0).abs() < 0.001);
        assert_eq!(chunks[3].len_samples, 6 * SAMPLE_RATE);
    }
}
