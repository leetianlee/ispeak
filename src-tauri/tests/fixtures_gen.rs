//! One-off fixture generator. Run with:
//!   cargo test --test fixtures_gen -- --ignored
//! Writes `tests/fixtures/30s-two-tones.wav`.

use hound::{SampleFormat, WavSpec, WavWriter};
use std::f32::consts::TAU;
use std::path::Path;

#[test]
#[ignore]
fn generate_30s_two_tones_wav() {
    let spec = WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let path = Path::new("tests/fixtures/30s-two-tones.wav");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let mut w = WavWriter::create(path, spec).unwrap();

    let duration_secs = 30.0_f32;
    let total_samples = (spec.sample_rate as f32 * duration_secs) as u32;

    for i in 0..total_samples {
        let t = i as f32 / spec.sample_rate as f32;
        let left = (TAU * 440.0 * t).sin() * 0.5;
        let right = (TAU * 880.0 * t).sin() * 0.5;
        w.write_sample((left * i16::MAX as f32) as i16).unwrap();
        w.write_sample((right * i16::MAX as f32) as i16).unwrap();
    }
    w.finalize().unwrap();
}
