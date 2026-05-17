//! Phase 3.3b — heuristic speaker diarisation.
//!
//! Composes with Whisper's segment-level output (we already have words +
//! timestamps from `whisper_engine::transcribe_segments`). The goal here is
//! "which speaker said this segment", not text accuracy.
//!
//! Pipeline:
//!
//! 1. **Silence-bounded turn detection**: a sliding-window RMS-energy gate
//!    splits the audio into speaker "turns" wherever there's a pause >500ms.
//!    Threshold is adaptive — set to 8% of the audio's peak frame energy so
//!    quiet meetings don't get one giant turn and loud meetings don't
//!    fragment into noise.
//!
//! 2. **Per-turn features**: each turn gets three numbers averaged over its
//!    32ms frames:
//!    - mean RMS energy (volume signature)
//!    - mean ZCR (high-frequency content, correlates with voice character)
//!    - mean spectral centroid (timbre, computed via real-FFT on the 32ms frame)
//!
//! 3. **K-means clustering** on the feature space, with k from the user's
//!    settings (default 2). K-means++ init keeps the result stable across
//!    runs; we cap at 20 iterations.
//!
//! 4. **Segment-to-cluster mapping**: each Whisper segment is assigned the
//!    cluster of the turn whose time range it overlaps with most. Turns and
//!    segments don't perfectly align (Whisper may merge a brief speaker
//!    interjection into the previous segment), so this is a soft assignment
//!    by overlap fraction.
//!
//! 5. **Cluster → SpeakerLabel**: clusters are renumbered in order of first
//!    appearance so Speaker A is always whoever talks first. If k=1 (only
//!    one speaker detected), everyone gets SpeakerLabel::Other.
//!
//! Limitations (be honest with users):
//! - Two speakers with similar voices (same pitch range, same accent) cluster
//!   together. The manual relabel UI (Phase 3.3) is the escape hatch.
//! - Crosstalk = both speakers in one Whisper segment — gets a single label.
//! - The first ~5 turns matter most: k-means centroids stabilise around them.

use realfft::RealFftPlanner;

use crate::meeting::types::{Segment, SpeakerLabel};

/// Diarisation knobs. Defaults are tuned for 16k mono speech.
#[derive(Debug, Clone, Copy)]
pub struct DiariseConfig {
    pub sample_rate: u32,
    /// Frame size for feature extraction (samples). 32ms at 16k = 512.
    pub frame_size: usize,
    /// Hop between frames. 16ms at 16k = 256.
    pub hop: usize,
    /// Silence longer than this opens a new speaker turn.
    pub silence_gap_ms: u32,
    /// k for k-means. Defaults to 2 for typical 1-on-1 meetings.
    pub k: u8,
}

impl DiariseConfig {
    pub fn for_16k(k: u8) -> Self {
        Self {
            sample_rate: 16_000,
            frame_size: 512,
            hop: 256,
            silence_gap_ms: 500,
            k: k.max(1),
        }
    }
}

/// Run diarisation on a complete 16kHz mono PCM buffer plus the Whisper
/// segments produced from it. Returns updated segments with speaker labels
/// reassigned. Falls back to the input labels if diarisation can't form
/// meaningful clusters (too short, all silence, etc).
pub fn diarise(samples: &[f32], segments: Vec<Segment>, cfg: DiariseConfig) -> Vec<Segment> {
    if samples.is_empty() || segments.is_empty() {
        return segments;
    }
    let frames = extract_frames(samples, &cfg);
    if frames.is_empty() {
        return segments;
    }
    let turns = build_turns(&frames, &cfg);
    if turns.is_empty() {
        return segments;
    }
    let features: Vec<Features> = turns.iter().map(|t| t.features).collect();
    let k = (cfg.k as usize).min(turns.len()).max(1);

    let labels = if k == 1 {
        vec![0usize; turns.len()]
    } else {
        kmeans(&features, k)
    };

    // Re-order cluster ids by first appearance so Speaker A is the first
    // person who talks. Without this, k-means assigns arbitrary cluster ids.
    let labels = relabel_by_first_appearance(&labels);

    assign_to_segments(segments, &turns, &labels, k)
}

// ─── Frame-level energy / ZCR / spectral centroid ───────────────────────────

#[derive(Debug, Clone, Copy)]
struct Frame {
    /// First sample index in the original buffer.
    start_sample: usize,
    rms: f32,
    zcr: f32,
    centroid: f32,
}

fn extract_frames(samples: &[f32], cfg: &DiariseConfig) -> Vec<Frame> {
    let n = samples.len();
    if n < cfg.frame_size {
        return Vec::new();
    }
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(cfg.frame_size);
    let mut input = fft.make_input_vec();
    let mut output = fft.make_output_vec();
    let mut frames = Vec::with_capacity(n / cfg.hop + 1);

    let mut start = 0usize;
    while start + cfg.frame_size <= n {
        let window = &samples[start..start + cfg.frame_size];
        let rms = rms_of(window);
        let zcr = zero_crossing_rate(window);
        input.copy_from_slice(window);
        let _ = fft.process(&mut input, &mut output);
        let centroid = spectral_centroid(&output, cfg.sample_rate, cfg.frame_size);
        frames.push(Frame {
            start_sample: start,
            rms,
            zcr,
            centroid,
        });
        start += cfg.hop;
    }
    frames
}

fn rms_of(window: &[f32]) -> f32 {
    let sum_sq: f32 = window.iter().map(|x| x * x).sum();
    (sum_sq / window.len() as f32).sqrt()
}

fn zero_crossing_rate(window: &[f32]) -> f32 {
    if window.len() < 2 {
        return 0.0;
    }
    let mut crossings = 0u32;
    for i in 1..window.len() {
        let a = window[i - 1];
        let b = window[i];
        if (a >= 0.0 && b < 0.0) || (a < 0.0 && b >= 0.0) {
            crossings += 1;
        }
    }
    crossings as f32 / (window.len() - 1) as f32
}

fn spectral_centroid(bins: &[realfft::num_complex::Complex<f32>], sample_rate: u32, frame_size: usize) -> f32 {
    // bins has frame_size/2 + 1 entries from realfft.
    let mut weighted = 0.0f32;
    let mut total = 0.0f32;
    let bin_hz = sample_rate as f32 / frame_size as f32;
    for (i, b) in bins.iter().enumerate() {
        let mag = (b.re * b.re + b.im * b.im).sqrt();
        weighted += i as f32 * bin_hz * mag;
        total += mag;
    }
    if total < 1e-8 {
        0.0
    } else {
        weighted / total
    }
}

// ─── Turn segmentation via adaptive silence gate ────────────────────────────

#[derive(Debug)]
struct Turn {
    start_secs: f32,
    end_secs: f32,
    features: Features,
}

#[derive(Debug, Clone, Copy)]
struct Features {
    rms: f32,
    zcr: f32,
    centroid: f32,
}

fn build_turns(frames: &[Frame], cfg: &DiariseConfig) -> Vec<Turn> {
    // Adaptive threshold: 8% of peak frame RMS, but never below a noise floor.
    let peak_rms = frames
        .iter()
        .map(|f| f.rms)
        .fold(0.0f32, |acc, x| acc.max(x));
    let threshold = (peak_rms * 0.08).max(1e-4);

    let frames_per_silence_gap =
        (cfg.silence_gap_ms as usize * cfg.sample_rate as usize / 1000) / cfg.hop;
    let frames_per_silence_gap = frames_per_silence_gap.max(2);

    // Walk frames, building turns. A run of frames with RMS >= threshold is a
    // turn; >= silence_gap silent frames in a row close the current turn.
    let mut turns = Vec::new();
    let mut in_turn = false;
    let mut turn_start_idx = 0usize;
    let mut silent_run = 0usize;
    let mut last_voiced_idx = 0usize;

    for (i, f) in frames.iter().enumerate() {
        let voiced = f.rms >= threshold;
        if voiced {
            silent_run = 0;
            last_voiced_idx = i;
            if !in_turn {
                in_turn = true;
                turn_start_idx = i;
            }
        } else if in_turn {
            silent_run += 1;
            if silent_run >= frames_per_silence_gap {
                turns.push(make_turn(frames, turn_start_idx, last_voiced_idx, cfg));
                in_turn = false;
                silent_run = 0;
            }
        }
    }
    if in_turn {
        turns.push(make_turn(frames, turn_start_idx, last_voiced_idx, cfg));
    }
    turns
}

fn make_turn(frames: &[Frame], start_idx: usize, end_idx: usize, cfg: &DiariseConfig) -> Turn {
    let slice = &frames[start_idx..=end_idx];
    let n = slice.len() as f32;
    let rms = slice.iter().map(|f| f.rms).sum::<f32>() / n;
    let zcr = slice.iter().map(|f| f.zcr).sum::<f32>() / n;
    let centroid = slice.iter().map(|f| f.centroid).sum::<f32>() / n;
    let start_secs = frames[start_idx].start_sample as f32 / cfg.sample_rate as f32;
    let end_sample = frames[end_idx].start_sample + cfg.frame_size;
    let end_secs = end_sample as f32 / cfg.sample_rate as f32;
    Turn {
        start_secs,
        end_secs,
        features: Features { rms, zcr, centroid },
    }
}

// ─── K-means++ ──────────────────────────────────────────────────────────────

fn kmeans(features: &[Features], k: usize) -> Vec<usize> {
    if features.is_empty() || k == 0 {
        return Vec::new();
    }
    if k >= features.len() {
        return (0..features.len()).collect();
    }

    // Normalise each axis to [0,1] so no single feature dominates the distance.
    let points: Vec<[f32; 3]> = normalise(features);
    let centroids = kmeanspp_init(&points, k);
    let mut centroids = centroids;
    let mut assignments = vec![0usize; points.len()];

    for _ in 0..20 {
        let mut changed = false;
        for (i, p) in points.iter().enumerate() {
            let mut best = 0usize;
            let mut best_d = f32::MAX;
            for (j, c) in centroids.iter().enumerate() {
                let d = sq_dist(p, c);
                if d < best_d {
                    best_d = d;
                    best = j;
                }
            }
            if assignments[i] != best {
                assignments[i] = best;
                changed = true;
            }
        }
        if !changed {
            break;
        }
        // Recompute centroids.
        for j in 0..k {
            let mut sum = [0.0f32; 3];
            let mut count = 0usize;
            for (i, p) in points.iter().enumerate() {
                if assignments[i] == j {
                    for d in 0..3 {
                        sum[d] += p[d];
                    }
                    count += 1;
                }
            }
            if count > 0 {
                for d in 0..3 {
                    centroids[j][d] = sum[d] / count as f32;
                }
            }
        }
    }
    assignments
}

fn normalise(features: &[Features]) -> Vec<[f32; 3]> {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for f in features {
        let row = [f.rms, f.zcr, f.centroid];
        for d in 0..3 {
            if row[d] < min[d] {
                min[d] = row[d];
            }
            if row[d] > max[d] {
                max[d] = row[d];
            }
        }
    }
    let mut out = Vec::with_capacity(features.len());
    for f in features {
        let row = [f.rms, f.zcr, f.centroid];
        let mut norm = [0.0f32; 3];
        for d in 0..3 {
            let range = max[d] - min[d];
            norm[d] = if range > 1e-8 {
                (row[d] - min[d]) / range
            } else {
                0.0
            };
        }
        out.push(norm);
    }
    out
}

fn kmeanspp_init(points: &[[f32; 3]], k: usize) -> Vec<[f32; 3]> {
    // Deterministic seed so the same audio always produces the same diarisation.
    // First centroid = first point. Subsequent centroids = the point farthest
    // (squared distance) from any already-chosen centroid.
    let mut centroids = Vec::with_capacity(k);
    centroids.push(points[0]);
    while centroids.len() < k {
        let mut best_point = points[0];
        let mut best_d = -1.0f32;
        for p in points {
            let mut min_d = f32::MAX;
            for c in &centroids {
                let d = sq_dist(p, c);
                if d < min_d {
                    min_d = d;
                }
            }
            if min_d > best_d {
                best_d = min_d;
                best_point = *p;
            }
        }
        centroids.push(best_point);
    }
    centroids
}

fn sq_dist(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    let mut s = 0.0f32;
    for d in 0..3 {
        let diff = a[d] - b[d];
        s += diff * diff;
    }
    s
}

/// Renumber cluster ids so id 0 is whichever cluster appears first.
fn relabel_by_first_appearance(assignments: &[usize]) -> Vec<usize> {
    use std::collections::HashMap;
    let mut remap: HashMap<usize, usize> = HashMap::new();
    let mut next = 0usize;
    let mut out = Vec::with_capacity(assignments.len());
    for &a in assignments {
        let new_id = *remap.entry(a).or_insert_with(|| {
            let id = next;
            next += 1;
            id
        });
        out.push(new_id);
    }
    out
}

// ─── Segment-to-cluster mapping ─────────────────────────────────────────────

fn assign_to_segments(
    mut segments: Vec<Segment>,
    turns: &[Turn],
    labels: &[usize],
    k: usize,
) -> Vec<Segment> {
    if turns.is_empty() {
        return segments;
    }
    for seg in segments.iter_mut() {
        let cluster = best_overlap_cluster(seg, turns, labels);
        seg.speaker = cluster_to_label(cluster, k);
    }
    segments
}

fn best_overlap_cluster(seg: &Segment, turns: &[Turn], labels: &[usize]) -> Option<usize> {
    let mut best = None;
    let mut best_overlap = 0.0f32;
    for (i, t) in turns.iter().enumerate() {
        let overlap = (seg.end.min(t.end_secs) - seg.start.max(t.start_secs)).max(0.0);
        if overlap > best_overlap {
            best_overlap = overlap;
            best = Some(labels[i]);
        }
    }
    best
}

fn cluster_to_label(cluster: Option<usize>, k: usize) -> SpeakerLabel {
    match cluster {
        // Only one cluster means we didn't actually detect distinct speakers.
        Some(_) if k <= 1 => SpeakerLabel::Other,
        Some(n) => SpeakerLabel::Indexed(n.min(255) as u8),
        None => SpeakerLabel::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sine(freq: f32, duration_secs: f32, sample_rate: u32) -> Vec<f32> {
        let n = (duration_secs * sample_rate as f32) as usize;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / sample_rate as f32;
            out.push((2.0 * std::f32::consts::PI * freq * t).sin() * 0.5);
        }
        out
    }

    fn make_silence(duration_secs: f32, sample_rate: u32) -> Vec<f32> {
        let n = (duration_secs * sample_rate as f32) as usize;
        vec![0.0; n]
    }

    fn make_segments(boundaries: &[(f32, f32)]) -> Vec<Segment> {
        boundaries
            .iter()
            .map(|&(s, e)| Segment {
                start: s,
                end: e,
                speaker: SpeakerLabel::Other,
                text: format!("seg {s:.1}-{e:.1}"),
            })
            .collect()
    }

    #[test]
    fn two_speakers_with_distinct_pitch_split_to_two_clusters() {
        let sr = 16_000;
        // 2s of 150Hz (low-pitched, "speaker 1"), 1s silence, 2s of 400Hz ("speaker 2"),
        // 1s silence, 2s of 150Hz again.
        let mut audio = Vec::new();
        audio.extend(make_sine(150.0, 2.0, sr));
        audio.extend(make_silence(1.0, sr));
        audio.extend(make_sine(400.0, 2.0, sr));
        audio.extend(make_silence(1.0, sr));
        audio.extend(make_sine(150.0, 2.0, sr));

        let segments = make_segments(&[(0.0, 2.0), (3.0, 5.0), (6.0, 8.0)]);
        let cfg = DiariseConfig::for_16k(2);
        let out = diarise(&audio, segments, cfg);
        assert_eq!(out.len(), 3);
        // First and third segments should share a speaker (both 150Hz);
        // middle should differ.
        let s0 = &out[0].speaker;
        let s1 = &out[1].speaker;
        let s2 = &out[2].speaker;
        assert_eq!(s0, s2, "150Hz turns should cluster together");
        assert_ne!(s0, s1, "400Hz turn should be its own cluster");
    }

    #[test]
    fn first_speaker_is_always_speaker_a() {
        let sr = 16_000;
        let mut audio = Vec::new();
        audio.extend(make_sine(200.0, 1.5, sr));
        audio.extend(make_silence(1.0, sr));
        audio.extend(make_sine(500.0, 1.5, sr));
        let segments = make_segments(&[(0.0, 1.5), (2.5, 4.0)]);
        let cfg = DiariseConfig::for_16k(2);
        let out = diarise(&audio, segments, cfg);
        // First speaker should be Indexed(0).
        match &out[0].speaker {
            SpeakerLabel::Indexed(n) => assert_eq!(*n, 0),
            other => panic!("expected Indexed(0), got {other:?}"),
        }
    }

    #[test]
    fn empty_input_passthrough() {
        let out = diarise(&[], Vec::new(), DiariseConfig::for_16k(2));
        assert!(out.is_empty());
    }

    #[test]
    fn k_equals_one_yields_other_labels() {
        let sr = 16_000;
        let audio = make_sine(200.0, 2.0, sr);
        let segments = make_segments(&[(0.0, 2.0)]);
        let cfg = DiariseConfig::for_16k(1);
        let out = diarise(&audio, segments, cfg);
        assert_eq!(out[0].speaker, SpeakerLabel::Other);
    }

    #[test]
    fn silence_only_input_returns_segments_unchanged() {
        let sr = 16_000;
        let audio = make_silence(3.0, sr);
        let segments = make_segments(&[(0.0, 3.0)]);
        let cfg = DiariseConfig::for_16k(2);
        let out = diarise(&audio, segments.clone(), cfg);
        // No turns detected → segments returned with their original labels.
        assert_eq!(out[0].speaker, segments[0].speaker);
    }

    #[test]
    fn rms_of_silence_is_zero() {
        let s = vec![0.0f32; 512];
        assert!((rms_of(&s) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn zcr_of_alternating_signs_is_near_one() {
        let s: Vec<f32> = (0..100).map(|i| if i % 2 == 0 { 1.0 } else { -1.0 }).collect();
        let z = zero_crossing_rate(&s);
        assert!(z > 0.95);
    }
}
