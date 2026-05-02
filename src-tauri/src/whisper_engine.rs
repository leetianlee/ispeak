/// Local Whisper transcription via whisper-rs (whisper.cpp bindings).
use std::path::PathBuf;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::error::{AppError, Result};
use crate::settings::WhisperModel;

/// Returns the path to the models directory inside app data.
pub fn models_dir(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("models")
}

/// Returns the full path to a model file.
pub fn model_path(app_data_dir: &PathBuf, model: &WhisperModel) -> PathBuf {
    models_dir(app_data_dir).join(model.filename())
}

/// Returns the list of models that are downloaded and ready to use.
pub fn installed_models(app_data_dir: &PathBuf) -> Vec<WhisperModel> {
    let models = [
        WhisperModel::Tiny,
        WhisperModel::Base,
        WhisperModel::Small,
        WhisperModel::Medium,
        WhisperModel::Large,
    ];
    models
        .into_iter()
        .filter(|m| model_path(app_data_dir, m).exists())
        .collect()
}

/// Transcribe 16kHz mono f32 audio samples using the selected local model.
pub fn transcribe(
    audio: &[f32],
    app_data_dir: &PathBuf,
    model: &WhisperModel,
) -> Result<String> {
    let path = model_path(app_data_dir, model);

    if !path.exists() {
        return Err(AppError::Whisper(format!(
            "Model '{}' not found. Please download it in Settings.",
            model.filename()
        )));
    }

    let path_str = path
        .to_str()
        .ok_or_else(|| AppError::Whisper("Invalid model path".to_string()))?;

    let ctx =
        WhisperContext::new_with_params(path_str, WhisperContextParameters::default())
            .map_err(|e| AppError::Whisper(format!("Failed to load model: {e}")))?;

    let mut state = ctx
        .create_state()
        .map_err(|e| AppError::Whisper(format!("Failed to create state: {e}")))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("en"));
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_suppress_non_speech_tokens(true);

    state
        .full(params, audio)
        .map_err(|e| AppError::Whisper(format!("Transcription failed: {e}")))?;

    let n_segments = state
        .full_n_segments()
        .map_err(|e| AppError::Whisper(format!("Failed to get segments: {e}")))?;

    let mut result = String::new();
    for i in 0..n_segments {
        if let Ok(text) = state.full_get_segment_text(i) {
            result.push_str(text.trim());
            result.push(' ');
        }
    }

    Ok(result.trim().to_string())
}
