use crate::error::{AppError, Result};
use crate::settings::{AIMode, AppSettings};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const SYSTEM_PROMPT: &str = "You are a dictation assistant. The user spoke the following text aloud and it was transcribed automatically. Fix grammar, punctuation, and capitalization errors. Do not change meaning, tone, or wording beyond corrections. Output only the corrected text, nothing else.";

const TIMEOUT: Duration = Duration::from_secs(10);

/// Post-process transcribed text through an LLM.
/// Returns the corrected text, or an error (caller should fall back to raw_text).
pub async fn post_process(
    raw_text: &str,
    ai_mode: &AIMode,
    settings: &AppSettings,
) -> Result<String> {
    let result = match ai_mode {
        AIMode::Off => return Ok(raw_text.to_string()),
        AIMode::Local => {
            ollama_complete(raw_text, &settings.ollama_base_url, &settings.ollama_model).await?
        }
        AIMode::CloudFast => {
            anthropic_complete(raw_text, &settings.anthropic_api_key, "claude-haiku-4-5-20251001")
                .await?
        }
        AIMode::CloudQuality => {
            anthropic_complete(
                raw_text,
                &settings.anthropic_api_key,
                "claude-sonnet-4-5-20250514",
            )
            .await?
        }
    };

    if result.trim().is_empty() {
        return Err(AppError::Other("AI returned empty response".to_string()));
    }

    Ok(result)
}

// ─── Ollama ──────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    system: &'a str,
    prompt: &'a str,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
}

async fn ollama_complete(text: &str, base_url: &str, model: &str) -> Result<String> {
    let client = Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| AppError::Ollama(e.to_string()))?;

    let url = format!("{}/api/generate", base_url.trim_end_matches('/'));

    let body = OllamaRequest {
        model,
        system: SYSTEM_PROMPT,
        prompt: text,
        stream: false,
    };

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                AppError::Ollama("Request timed out (10s)".to_string())
            } else if e.is_connect() {
                AppError::Ollama(format!(
                    "Cannot connect to Ollama at {url}. Is it running?"
                ))
            } else {
                AppError::Ollama(e.to_string())
            }
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Ollama(format!("HTTP {status}: {body}")));
    }

    let parsed: OllamaResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Ollama(format!("Failed to parse response: {e}")))?;

    Ok(parsed.response)
}

// ─── Anthropic ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<AnthropicMessage<'a>>,
}

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

async fn anthropic_complete(text: &str, api_key: &str, model: &str) -> Result<String> {
    if api_key.is_empty() {
        return Err(AppError::Anthropic(
            "Anthropic API key not set".to_string(),
        ));
    }

    let client = Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| AppError::Anthropic(e.to_string()))?;

    let body = AnthropicRequest {
        model,
        max_tokens: 4096,
        system: SYSTEM_PROMPT,
        messages: vec![AnthropicMessage {
            role: "user",
            content: text,
        }],
    };

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                AppError::Anthropic("Request timed out (10s)".to_string())
            } else {
                AppError::Anthropic(e.to_string())
            }
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Anthropic(format!("HTTP {status}: {body}")));
    }

    let parsed: AnthropicResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Anthropic(format!("Failed to parse response: {e}")))?;

    parsed
        .content
        .into_iter()
        .next()
        .map(|c| c.text)
        .ok_or_else(|| AppError::Anthropic("Empty content in response".to_string()))
}
