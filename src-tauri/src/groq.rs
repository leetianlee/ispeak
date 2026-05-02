/// Groq Whisper cloud transcription.
use reqwest::Client;
use serde::Deserialize;

use crate::error::{AppError, Result};

#[derive(Deserialize)]
struct GroqResponse {
    text: String,
}

/// Transcribe audio bytes via the Groq Whisper API.
/// `audio_data` should be WAV-encoded 16kHz mono audio.
pub async fn transcribe(audio_wav: Vec<u8>, api_key: &str) -> Result<String> {
    if api_key.is_empty() {
        return Err(AppError::Groq("Groq API key is not set".to_string()));
    }

    let client = Client::new();

    let part = reqwest::multipart::Part::bytes(audio_wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| AppError::Groq(e.to_string()))?;

    let form = reqwest::multipart::Form::new()
        .text("model", "whisper-large-v3-turbo")
        .text("language", "en")
        .text("response_format", "json")
        .part("file", part);

    let response = client
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {api_key}"))
        .multipart(form)
        .send()
        .await
        .map_err(|e| AppError::Groq(format!("Request failed: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Groq(format!("API error {status}: {body}")));
    }

    let result: GroqResponse = response
        .json()
        .await
        .map_err(|e| AppError::Groq(format!("Failed to parse response: {e}")))?;

    Ok(result.text.trim().to_string())
}

/// Encode f32 16kHz mono samples as a WAV file in memory.
pub fn encode_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let num_samples = samples.len() as u32;
    let byte_rate = sample_rate * 2; // 16-bit samples
    let data_size = num_samples * 2;
    let chunk_size = 36 + data_size;

    let mut wav = Vec::with_capacity(44 + data_size as usize);

    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&chunk_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    wav.extend_from_slice(&1u16.to_le_bytes());  // PCM
    wav.extend_from_slice(&1u16.to_le_bytes());  // mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

    // data chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());

    for &s in samples {
        let i16_sample = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        wav.extend_from_slice(&i16_sample.to_le_bytes());
    }

    wav
}
