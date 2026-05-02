/// Audio capture module — uses cpal for native mic access.
/// Resamples captured audio to 16kHz mono f32 for Whisper.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SupportedStreamConfig};
use rubato::{FftFixedIn, Resampler};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::error::{AppError, Result};

const TARGET_SAMPLE_RATE: u32 = 16_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrophoneDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Returns all available input devices.
pub fn list_microphones() -> Result<Vec<MicrophoneDevice>> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    let devices = host
        .input_devices()
        .map_err(|e| AppError::Audio(e.to_string()))?
        .filter_map(|d| {
            let name = d.name().ok()?;
            Some(MicrophoneDevice {
                id: name.clone(),
                name: name.clone(),
                is_default: name == default_name,
            })
        })
        .collect();

    Ok(devices)
}

/// Selects a device by name, or falls back to the system default.
fn get_device(device_id: Option<&str>) -> Result<Device> {
    let host = cpal::default_host();

    if let Some(id) = device_id {
        let device = host
            .input_devices()
            .map_err(|e| AppError::Audio(e.to_string()))?
            .find(|d| d.name().map(|n| n == id).unwrap_or(false));

        if let Some(d) = device {
            return Ok(d);
        }
    }

    host.default_input_device()
        .ok_or_else(|| AppError::Audio("No input device found".to_string()))
}

/// Captures audio until `stop_flag` is set, then returns 16kHz mono f32 samples.
pub fn record(
    device_id: Option<&str>,
    stop_flag: Arc<Mutex<bool>>,
    max_seconds: u32,
) -> Result<Vec<f32>> {
    let device = get_device(device_id)?;
    let config: SupportedStreamConfig = device
        .default_input_config()
        .map_err(|e| AppError::Audio(e.to_string()))?;

    let native_sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let sample_format = config.sample_format();

    let captured: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = captured.clone();
    let stop_clone = stop_flag.clone();
    let max_samples = (native_sample_rate * channels as u32 * max_seconds) as usize;

    let err_fn = |e| eprintln!("[iSpeak audio] stream error: {e}");

    let stream = match sample_format {
        SampleFormat::F32 => {
            let cfg = config.into();
            device
                .build_input_stream(
                    &cfg,
                    move |data: &[f32], _| {
                        let mut buf = captured_clone.lock().unwrap();
                        if buf.len() < max_samples {
                            buf.extend_from_slice(data);
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AppError::Audio(e.to_string()))?
        }
        SampleFormat::I16 => {
            let cfg = config.into();
            device
                .build_input_stream(
                    &cfg,
                    move |data: &[i16], _| {
                        let mut buf = captured_clone.lock().unwrap();
                        if buf.len() < max_samples {
                            buf.extend(data.iter().map(|s| *s as f32 / i16::MAX as f32));
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AppError::Audio(e.to_string()))?
        }
        SampleFormat::U16 => {
            let cfg = config.into();
            device
                .build_input_stream(
                    &cfg,
                    move |data: &[u16], _| {
                        let mut buf = captured_clone.lock().unwrap();
                        if buf.len() < max_samples {
                            buf.extend(
                                data.iter()
                                    .map(|s| (*s as f32 - 32768.0) / 32768.0),
                            );
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AppError::Audio(e.to_string()))?
        }
        _ => return Err(AppError::Audio("Unsupported sample format".to_string())),
    };

    stream.play().map_err(|e| AppError::Audio(e.to_string()))?;

    // Poll until stop flag is set or max duration reached
    loop {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let done = *stop_clone.lock().unwrap();
        let len = captured.lock().unwrap().len();
        if done || len >= max_samples {
            break;
        }
    }

    drop(stream);

    let raw = captured.lock().unwrap().clone();
    let mono = to_mono(&raw, channels);
    let resampled = resample(mono, native_sample_rate, TARGET_SAMPLE_RATE)?;

    Ok(resampled)
}

/// Mix multi-channel audio down to mono.
fn to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels == 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

/// Resample from `from_rate` to `to_rate` using rubato.
fn resample(samples: Vec<f32>, from_rate: u32, to_rate: u32) -> Result<Vec<f32>> {
    if from_rate == to_rate {
        return Ok(samples);
    }

    let chunk_size = 1024;
    let mut resampler = FftFixedIn::<f32>::new(
        from_rate as usize,
        to_rate as usize,
        chunk_size,
        2,
        1,
    )
    .map_err(|e| AppError::Audio(format!("Resampler init failed: {e}")))?;

    let mut output = Vec::new();
    let mut pos = 0;

    while pos < samples.len() {
        let end = (pos + chunk_size).min(samples.len());
        let mut chunk = samples[pos..end].to_vec();
        // Pad last chunk if needed
        chunk.resize(chunk_size, 0.0);

        let waves_in = vec![chunk];
        let waves_out = resampler
            .process(&waves_in, None)
            .map_err(|e| AppError::Audio(format!("Resampling failed: {e}")))?;

        output.extend_from_slice(&waves_out[0]);
        pos += chunk_size;
    }

    Ok(output)
}
