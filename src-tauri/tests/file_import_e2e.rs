//! End-to-end: ingest → chunk planning, no Whisper required.
//! Uses public APIs only — exercises decode → resample → chunk planning.

use std::path::PathBuf;
use ispeak_lib::meeting::ingest::{ingest_to_pcm_file, pcm_file_samples};
use ispeak_lib::meeting::chunker::plan_chunks;

#[test]
fn ingest_then_chunk_planning_matches_audio_length() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/30s-two-tones.wav");
    let tmp = tempfile::tempdir().unwrap();
    let pcm = ingest_to_pcm_file(&fixture, tmp.path()).expect("ingest");

    let n = pcm_file_samples(&pcm).unwrap();
    let secs = n as f32 / 16_000.0;
    assert!(secs > 29.5 && secs < 30.5, "expected ~30s, got {}s", secs);

    let chunks = plan_chunks(n);
    // 30s ≤ one 30s window → one chunk
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].start_secs, 0.0);
    assert!((chunks[0].end_secs - secs).abs() < 0.01);
}
