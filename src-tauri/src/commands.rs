/// Tauri commands — implements the contract defined in src/lib/contract.ts.
/// Each function here corresponds exactly to an invoke() call in the frontend.
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_store::StoreExt;
use tokio::task::JoinHandle;

use crate::audio::{self, MicrophoneDevice};
use crate::error::{AppError, Result};
use crate::groq;
use crate::paste;
use crate::settings::{mask_key, AIMode, AppSettings, TranscriptionEngine, WhisperModel};
use crate::whisper_engine;

// ─── App state ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingState {
    Idle,
    Recording,
    Processing,
}

pub struct AppState {
    pub recording_state: Mutex<RecordingState>,
    pub stop_flag: Arc<Mutex<bool>>,
    pub app_data_dir: PathBuf,
    /// Handle to the background audio capture task, set by execute_start, consumed by execute_stop.
    pub recording_handle: Mutex<Option<JoinHandle<crate::error::Result<Vec<f32>>>>>,
}

impl AppState {
    pub fn new(app_data_dir: PathBuf) -> Self {
        AppState {
            recording_state: Mutex::new(RecordingState::Idle),
            stop_flag: Arc::new(Mutex::new(false)),
            app_data_dir,
            recording_handle: Mutex::new(None),
        }
    }
}

// ─── Result types ─────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct TranscriptResult {
    pub text: String,
    pub raw_text: String,
    pub duration_ms: u64,
    pub engine: String,
    pub ai_mode: String,
    pub timestamp: String,
}

#[derive(Serialize, Clone)]
pub struct ModelDownloadProgress {
    pub model: String,
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    pub percent: u8,
    pub complete: bool,
    pub error: Option<String>,
}

#[allow(dead_code)]
#[derive(Serialize, Clone)]
pub struct AppErrorEvent {
    pub code: String,
    pub message: String,
}

// ─── Dictation commands ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn start_recording<R: Runtime>(
    app: AppHandle<R>,
    _state: State<'_, AppState>,
) -> Result<()> {
    execute_start(app).await
}

#[tauri::command]
pub async fn stop_recording<R: Runtime>(
    app: AppHandle<R>,
    _state: State<'_, AppState>,
) -> Result<TranscriptResult> {
    execute_stop(app).await
}

#[tauri::command]
pub async fn cancel_recording<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<()> {
    *state.stop_flag.lock().unwrap() = true;
    // Wait for the background capture to finish so it doesn't leak
    let handle = state.recording_handle.lock().unwrap().take();
    if let Some(h) = handle {
        let _ = h.await;
    }
    *state.recording_state.lock().unwrap() = RecordingState::Idle;
    app.emit("recording_state_changed", "idle").ok();
    Ok(())
}

/// Shared start logic — called from both the Tauri command and the global hotkey handler.
/// Begins audio capture in a background thread immediately.
pub(crate) async fn execute_start<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    let state = app.state::<AppState>();
    let mut rs = state.recording_state.lock().unwrap();
    if *rs != RecordingState::Idle {
        return Err(AppError::Audio("Already recording".to_string()));
    }
    *rs = RecordingState::Recording;
    *state.stop_flag.lock().unwrap() = false;

    let stop_flag = state.stop_flag.clone();
    let settings = load_settings(&app);
    let mic_id = settings.microphone_id.clone();
    let max_dur = settings.max_recording_duration_s;

    // Spawn audio capture in background — it runs until stop_flag is set
    let handle = tokio::task::spawn_blocking(move || {
        audio::record(mic_id.as_deref(), stop_flag, max_dur)
    });
    *state.recording_handle.lock().unwrap() = Some(handle);

    drop(rs);
    app.emit("recording_state_changed", "recording").ok();
    Ok(())
}

/// Shared stop/transcribe logic — called from both the Tauri command and the global hotkey handler.
pub(crate) async fn execute_stop<R: Runtime>(app: AppHandle<R>) -> Result<TranscriptResult> {
    let recording_handle;
    let app_data_dir;
    {
        let state = app.state::<AppState>();
        {
            let mut rs = state.recording_state.lock().unwrap();
            if *rs != RecordingState::Recording {
                return Err(AppError::Audio("Not currently recording".to_string()));
            }
            *rs = RecordingState::Processing;
            // Signal the background audio capture to stop
            *state.stop_flag.lock().unwrap() = true;
        }
        app.emit("recording_state_changed", "processing").ok();
        recording_handle = state.recording_handle.lock().unwrap().take();
        app_data_dir = state.app_data_dir.clone();
    }

    let handle = recording_handle
        .ok_or_else(|| AppError::Audio("No active recording task".to_string()))?;

    let settings = load_settings(&app);
    let start_time = std::time::Instant::now();

    // Await the background audio capture task
    let audio_data = handle
        .await
        .map_err(|e| AppError::Audio(e.to_string()))??;

    let duration_ms = start_time.elapsed().as_millis() as u64;

    let raw_text = match settings.transcription_engine {
        TranscriptionEngine::Local => {
            let model = settings.whisper_model.clone();
            tokio::task::spawn_blocking(move || {
                whisper_engine::transcribe(&audio_data, &app_data_dir, &model)
            })
            .await
            .map_err(|e| AppError::Whisper(e.to_string()))??
        }
        TranscriptionEngine::Groq => {
            let wav = groq::encode_wav(&audio_data, 16_000);
            groq::transcribe(wav, &settings.groq_api_key).await?
        }
    };

    // AI post-processing (non-fatal — falls back to raw_text)
    let ai_mode = settings.ai_mode.clone();
    let text = if ai_mode != AIMode::Off {
        match crate::ai::post_process(&raw_text, &ai_mode, &settings).await {
            Ok(processed) => processed,
            Err(e) => {
                log::warn!("AI post-processing failed, using raw text: {e}");
                app.emit("app_error", serde_json::json!({
                    "code": "ai_post_process",
                    "message": e.to_string(),
                })).ok();
                raw_text.clone()
            }
        }
    } else {
        raw_text.clone()
    };

    let clipboard_text = text.clone();
    tokio::task::spawn_blocking(move || -> crate::error::Result<()> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| AppError::Other(format!("Clipboard error: {e}")))?;
        clipboard
            .set_text(clipboard_text)
            .map_err(|e| AppError::Other(format!("Clipboard write error: {e}")))?;
        paste::paste_to_cursor()
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))??;

    let result = TranscriptResult {
        text: text.clone(),
        raw_text,
        duration_ms,
        engine: format!("{:?}", settings.transcription_engine).to_lowercase(),
        ai_mode: format!("{:?}", ai_mode).to_lowercase(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    app.emit("transcript_ready", result.clone()).ok();
    app.emit("recording_state_changed", "idle").ok();

    *app.state::<AppState>().recording_state.lock().unwrap() = RecordingState::Idle;

    Ok(result)
}

// ─── Audio commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_microphones() -> Result<Vec<MicrophoneDevice>> {
    audio::list_microphones()
}

// ─── Model commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_installed_models(state: State<'_, AppState>) -> Vec<String> {
    whisper_engine::installed_models(&state.app_data_dir)
        .into_iter()
        .map(|m| format!("{m:?}").to_lowercase())
        .collect()
}

#[tauri::command]
pub async fn download_model<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    model: String,
) -> Result<()> {
    let whisper_model: WhisperModel = match model.as_str() {
        "tiny"   => WhisperModel::Tiny,
        "base"   => WhisperModel::Base,
        "small"  => WhisperModel::Small,
        "medium" => WhisperModel::Medium,
        "large"  => WhisperModel::Large,
        _ => return Err(AppError::Other(format!("Unknown model: {model}"))),
    };

    let models_dir = whisper_engine::models_dir(&state.app_data_dir);
    std::fs::create_dir_all(&models_dir)?;

    let url = whisper_model.download_url().to_string();
    let dest = models_dir.join(whisper_model.filename());
    let model_name = model.clone();

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::Other(format!("Download failed: {e}")))?;

    let total = response.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut file =
        tokio::fs::File::create(&dest)
            .await
            .map_err(|e| AppError::Io(e))?;

    use tokio::io::AsyncWriteExt;
    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Other(e.to_string()))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| AppError::Io(e))?;
        downloaded += chunk.len() as u64;

        let percent = if total > 0 {
            ((downloaded as f64 / total as f64) * 100.0) as u8
        } else {
            0
        };

        app.emit(
            "model_download_progress",
            ModelDownloadProgress {
                model: model_name.clone(),
                bytes_downloaded: downloaded,
                bytes_total: total,
                percent,
                complete: false,
                error: None,
            },
        )
        .ok();
    }

    app.emit(
        "model_download_progress",
        ModelDownloadProgress {
            model: model_name,
            bytes_downloaded: downloaded,
            bytes_total: total,
            percent: 100,
            complete: true,
            error: None,
        },
    )
    .ok();

    Ok(())
}

#[tauri::command]
pub fn delete_model(state: State<'_, AppState>, model: String) -> Result<()> {
    let whisper_model: WhisperModel = match model.as_str() {
        "tiny"   => WhisperModel::Tiny,
        "base"   => WhisperModel::Base,
        "small"  => WhisperModel::Small,
        "medium" => WhisperModel::Medium,
        "large"  => WhisperModel::Large,
        _ => return Err(AppError::Other(format!("Unknown model: {model}"))),
    };
    let path = whisper_engine::model_path(&state.app_data_dir, &whisper_model);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

// ─── Settings commands ────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_settings<R: Runtime>(app: AppHandle<R>) -> AppSettings {
    let mut settings = load_settings(&app);
    // Mask keys before sending to frontend
    settings.groq_api_key = mask_key(&settings.groq_api_key);
    settings
}

#[tauri::command]
pub fn update_settings<R: Runtime>(
    app: AppHandle<R>,
    settings: serde_json::Value,
) -> Result<()> {
    let store = app
        .store("settings.json")
        .map_err(|e| AppError::Settings(e.to_string()))?;

    // Merge patch — only update provided keys, preserve masked keys
    if let serde_json::Value::Object(map) = &settings {
        for (key, value) in map {
            // Don't overwrite real keys with masked values
            let is_key_field = matches!(
                key.as_str(),
                "groq_api_key"
            );
            if is_key_field {
                if let Some(s) = value.as_str() {
                    // Skip if it looks like a masked value
                    if s.contains("...") || s.chars().all(|c| c == '*') {
                        continue;
                    }
                }
            }
            store.set(key.clone(), value.clone());
        }
    }

    store.save().map_err(|e| AppError::Settings(e.to_string()))?;

    // If the hotkey changed, re-register it immediately
    if let serde_json::Value::Object(ref map) = settings {
        if map.contains_key("hotkey") {
            let new_hotkey = load_settings(&app).hotkey;
            let gs = app.global_shortcut();
            let _ = gs.unregister_all();
            if let Err(e) = gs.register(new_hotkey.as_str()) {
                log::warn!("Failed to re-register hotkey '{new_hotkey}': {e}");
            }
        }
    }

    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

pub fn load_settings<R: Runtime>(app: &AppHandle<R>) -> AppSettings {
    let Ok(store) = app.store("settings.json") else {
        return AppSettings::default();
    };

    macro_rules! get {
        ($key:expr, $default:expr) => {
            store
                .get($key)
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or($default)
        };
    }

    AppSettings {
        hotkey: get!("hotkey", "CommandOrControl+Shift+Space".to_string()),
        recording_mode: get!("recording_mode", Default::default()),
        transcription_engine: get!("transcription_engine", Default::default()),
        whisper_model: get!("whisper_model", Default::default()),
        ai_mode: get!("ai_mode", Default::default()),
        ollama_model: get!("ollama_model", "llama3.2:3b".to_string()),
        ollama_base_url: get!("ollama_base_url", "http://localhost:11434".to_string()),
        groq_api_key: get!("groq_api_key", String::new()),
        microphone_id: get!("microphone_id", None),
        indicator_position: get!("indicator_position", Default::default()),
        max_recording_duration_s: get!("max_recording_duration_s", 60),
        dark_mode: get!("dark_mode", true),
    }
}
