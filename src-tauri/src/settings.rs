use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingMode {
    PushToTalk,
    Toggle,
}

impl Default for RecordingMode {
    fn default() -> Self {
        RecordingMode::PushToTalk
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionEngine {
    Local,
    Groq,
}

impl Default for TranscriptionEngine {
    fn default() -> Self {
        TranscriptionEngine::Local
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AIMode {
    Off,
    Local,
    CloudFast,
    CloudQuality,
}

impl Default for AIMode {
    fn default() -> Self {
        AIMode::Off
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WhisperModel {
    Tiny,
    Base,
    Small,
    Medium,
    Large,
}

impl Default for WhisperModel {
    fn default() -> Self {
        WhisperModel::Medium
    }
}

impl WhisperModel {
    pub fn filename(&self) -> &str {
        match self {
            WhisperModel::Tiny   => "ggml-tiny.bin",
            WhisperModel::Base   => "ggml-base.bin",
            WhisperModel::Small  => "ggml-small.bin",
            WhisperModel::Medium => "ggml-medium.bin",
            WhisperModel::Large  => "ggml-large.bin",
        }
    }

    pub fn download_url(&self) -> &str {
        match self {
            WhisperModel::Tiny   => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
            WhisperModel::Base   => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
            WhisperModel::Small  => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
            WhisperModel::Medium => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
            WhisperModel::Large  => "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorPosition {
    pub x: f64,
    pub y: f64,
}

impl Default for IndicatorPosition {
    fn default() -> Self {
        IndicatorPosition { x: 40.0, y: 40.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_hotkey")]
    pub hotkey: String,

    #[serde(default)]
    pub recording_mode: RecordingMode,

    #[serde(default)]
    pub transcription_engine: TranscriptionEngine,

    #[serde(default)]
    pub whisper_model: WhisperModel,

    #[serde(default)]
    pub ai_mode: AIMode,

    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,

    #[serde(default = "default_ollama_base_url")]
    pub ollama_base_url: String,

    #[serde(default)]
    pub groq_api_key: String,

    #[serde(default)]
    pub microphone_id: Option<String>,

    #[serde(default)]
    pub indicator_position: IndicatorPosition,

    #[serde(default = "default_max_duration")]
    pub max_recording_duration_s: u32,

    #[serde(default = "default_true")]
    pub dark_mode: bool,

    /// Phase 3.3b — automatic speaker diarisation. When true, every meeting
    /// transcript gets per-segment speaker labels assigned via the heuristic
    /// in `meeting::diarise`. Disable to keep every segment labelled "Speaker".
    #[serde(default = "default_true")]
    pub auto_diarise: bool,

    /// Expected number of distinct speakers in a meeting. K-means uses this as
    /// k. 2 is the sweet spot for 1-on-1s; bump for multi-party calls.
    #[serde(default = "default_diarise_speakers")]
    pub diarise_expected_speakers: u8,
}

fn default_diarise_speakers() -> u8 {
    2
}

fn default_hotkey() -> String {
    "CommandOrControl+Shift+Space".to_string()
}

fn default_max_duration() -> u32 {
    60
}

fn default_true() -> bool {
    true
}

fn default_ollama_model() -> String {
    "llama3.2:3b".to_string()
}

fn default_ollama_base_url() -> String {
    "http://localhost:11434".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            hotkey: default_hotkey(),
            recording_mode: RecordingMode::default(),
            transcription_engine: TranscriptionEngine::default(),
            whisper_model: WhisperModel::default(),
            ai_mode: AIMode::default(),
            ollama_model: default_ollama_model(),
            ollama_base_url: default_ollama_base_url(),
            groq_api_key: String::new(),
            microphone_id: None,
            indicator_position: IndicatorPosition::default(),
            max_recording_duration_s: default_max_duration(),
            dark_mode: true,
            auto_diarise: true,
            diarise_expected_speakers: default_diarise_speakers(),
        }
    }
}

/// Mask an API key for safe display — shows last 4 chars only.
pub fn mask_key(key: &str) -> String {
    if key.len() <= 4 {
        return "*".repeat(key.len());
    }
    format!("{}...{}", &"*".repeat(8), &key[key.len() - 4..])
}
