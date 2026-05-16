//! Audio file decode + resample to 16kHz mono f32 PCM.

use crate::error::{AppError, Result};
use std::path::Path;

/// Decoded PCM in source format.
pub struct DecodedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub fn decode_file(path: &Path) -> Result<DecodedAudio> {
    use symphonia::core::codecs::{audio::AudioDecoderOptions, CodecParameters};
    use symphonia::core::errors::Error as SymphoniaError;
    use symphonia::core::formats::{FormatOptions, FormatReader as _, TrackType};
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use std::fs::File;

    let file = File::open(path)
        .map_err(|e| AppError::Meeting(format!("open {path:?}: {e}")))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    // symphonia 0.6: probe() returns Box<dyn FormatReader> directly (not a ProbeResult wrapper)
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| AppError::Meeting(format!("probe: {e}")))?;

    // symphonia 0.6: default_track takes a TrackType argument
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| AppError::Meeting("no audio track".into()))?;
    let track_id = track.id;

    // codec_params is Option<CodecParameters> (enum); extract Audio variant
    let audio_params = match track.codec_params.clone() {
        Some(CodecParameters::Audio(p)) => p,
        _ => return Err(AppError::Meeting("track has no audio codec params".into())),
    };

    let sample_rate = audio_params
        .sample_rate
        .ok_or_else(|| AppError::Meeting("missing sample rate".into()))?;
    let channels = audio_params
        .channels
        .as_ref()
        .ok_or_else(|| AppError::Meeting("missing channel layout".into()))?
        .count() as u16;

    // symphonia 0.6: make_audio_decoder (not make)
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&audio_params, &AudioDecoderOptions::default())
        .map_err(|e| AppError::Meeting(format!("codec: {e}")))?;

    let mut samples: Vec<f32> = Vec::new();

    // symphonia 0.6: next_packet returns Result<Option<Packet>>; None signals end-of-stream
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(AppError::Meeting(format!("read packet: {e}"))),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                // copy_to_vec_interleaved resizes (overwrites) the vec, so use a temp
                // then extend the accumulator.
                let mut tmp: Vec<f32> = Vec::new();
                buf.copy_to_vec_interleaved(&mut tmp);
                samples.extend_from_slice(&tmp);
            }
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(AppError::Meeting(format!("decode: {e}"))),
        }
    }

    Ok(DecodedAudio { samples, sample_rate, channels })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn decodes_wav() {
        let path = fixture("30s-two-tones.wav");
        let audio = decode_file(&path).expect("decode wav");
        assert_eq!(audio.sample_rate, 44_100);
        assert_eq!(audio.channels, 2);
        // 30s * 44100 samples/sec * 2 channels = 2_646_000 interleaved samples
        // Allow small tolerance for packet padding
        let diff = (audio.samples.len() as i64 - 2_646_000_i64).abs();
        assert!(diff < 5_000, "got {} samples", audio.samples.len());
    }

    #[test]
    fn decodes_m4a() {
        let path = fixture("short.m4a");
        if !path.exists() {
            eprintln!("skipping: {path:?} not present");
            return;
        }
        let audio = decode_file(&path).expect("decode m4a");
        assert!(audio.sample_rate == 44_100 || audio.sample_rate == 48_000);
        assert!(audio.channels >= 1 && audio.channels <= 2);
        assert!(audio.samples.len() > 100_000, "expected non-trivial sample count");
    }
}
