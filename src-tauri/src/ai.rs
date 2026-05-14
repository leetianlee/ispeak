use crate::error::{AppError, Result};
use crate::settings::{AIMode, AppSettings};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const BASE_PROMPT: &str = "You are a dictation post-editor. The input is automatic-speech-recognition output of one speaker dictating naturally. Produce a faithful, clean written version.\n\nRules:\n1. Fix grammar, punctuation, capitalization, and sentence boundaries.\n2. Fix obvious ASR errors: homophones (their/there/they're, two/to/too, your/you're, its/it's, then/than, affect/effect), mis-segmented words (\"a lot\" not \"alot\", \"a part\" vs \"apart\"), and clear acoustic mishears when context makes the intent unambiguous. Do not invent content.\n3. Remove filler words and disfluencies: um, uh, er, ah, like (when filler), you know (when filler), and self-corrections such as \"I went to — I mean, I drove to\" → keep only the corrected version.\n4. Preserve the speaker's meaning, tone, vocabulary, and register. Do not paraphrase or summarize.\n5. Preserve proper nouns, technical terms, code identifiers, numbers, and units exactly as transcribed unless they are an obvious ASR error.\n6. If the speaker dictates punctuation verbatim (\"comma\", \"period\", \"new paragraph\", \"open quote\"), convert it to the actual mark.\n7. Output ONLY the corrected text. No preamble, no explanation, no quotes around the output, no markdown fences.\n\nExamples:\nInput: so um i was thinking that we should like maybe ship the the feature on friday and then we can iterate on it next week\nOutput: I was thinking we should ship the feature on Friday and then iterate on it next week.\n\nInput: their going to send the the report two the team but i think its already two late\nOutput: They're going to send the report to the team, but I think it's already too late.\n\nInput: open the file source slash main dot rs and add a new function called handle request\nOutput: Open the file src/main.rs and add a new function called handle_request.";

/// Build the system prompt, optionally appending a context hint based on the frontmost app.
fn build_system_prompt(app_name: Option<&str>) -> String {
    let hint = app_name.and_then(|name| {
        let lower = name.to_lowercase();
        if lower.contains("slack") || lower.contains("teams") || lower.contains("discord") {
            Some("The user is typing in a chat app. Keep the tone casual, skip formal punctuation, preserve emoji intent.")
        } else if lower.contains("mail") || lower.contains("outlook") || lower.contains("gmail") {
            Some("The user is typing an email. Use email formatting, be aware of greeting and sign-off structure.")
        } else if lower.contains("vs code") || lower.contains("visual studio code") || lower.contains("cursor") || lower.contains("zed") {
            Some("The user is typing in a code editor. Use minimal punctuation, be code-comment friendly, do not auto-capitalise.")
        } else if lower.contains("notion") || lower.contains("obsidian") {
            Some("The user is typing in a notes app. Write clean prose, preserve bullet intent.")
        } else if lower.contains("terminal") || lower.contains("iterm") || lower.contains("warp") || lower.contains("ghostty") {
            Some("The user is typing in a terminal. Be command-friendly, do not capitalise, no trailing period.")
        } else if lower == "claude" {
            Some("The user is typing a prompt for an LLM. Write clear prose, structured for LLM prompting.")
        } else {
            None
        }
    });

    match hint {
        Some(h) => format!("{BASE_PROMPT}\nContext: {h}"),
        None => BASE_PROMPT.to_string(),
    }
}

const TIMEOUT: Duration = Duration::from_secs(10);

/// Post-process transcribed text through an LLM.
/// Returns the corrected text, or an error (caller should fall back to raw_text).
pub async fn post_process(
    raw_text: &str,
    ai_mode: &AIMode,
    settings: &AppSettings,
) -> Result<String> {
    let app_name = crate::frontmost_app::get_frontmost_app_name();
    let system_prompt = build_system_prompt(app_name.as_deref());

    if system_prompt.contains("\nContext:") {
        eprintln!("[iSpeak] AI post-processing: detected app {:?}, using context hint", app_name.as_deref().unwrap_or("unknown"));
    } else {
        eprintln!("[iSpeak] AI post-processing: detected app {:?}, using default prompt", app_name.as_deref().unwrap_or("unknown"));
    }

    let result = match ai_mode {
        AIMode::Off => return Ok(raw_text.to_string()),
        AIMode::Local => {
            ollama_complete(raw_text, &settings.ollama_base_url, &settings.ollama_model, &system_prompt).await?
        }
        AIMode::CloudFast => {
            groq_chat_complete(raw_text, &settings.groq_api_key, "llama-3.3-70b-versatile", &system_prompt)
                .await?
        }
        AIMode::CloudQuality => {
            groq_chat_complete(raw_text, &settings.groq_api_key, "llama-3.3-70b-specdec", &system_prompt)
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

async fn ollama_complete(text: &str, base_url: &str, model: &str, system_prompt: &str) -> Result<String> {
    let client = Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| AppError::Ollama(e.to_string()))?;

    let url = format!("{}/api/generate", base_url.trim_end_matches('/'));

    let body = OllamaRequest {
        model,
        system: system_prompt,
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

// ─── Groq Chat ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct GroqChatRequest<'a> {
    model: &'a str,
    messages: Vec<GroqChatMessage<'a>>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize)]
struct GroqChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct GroqChatResponse {
    choices: Vec<GroqChatChoice>,
}

#[derive(Deserialize)]
struct GroqChatChoice {
    message: GroqChatChoiceMessage,
}

#[derive(Deserialize)]
struct GroqChatChoiceMessage {
    content: String,
}

async fn groq_chat_complete(text: &str, api_key: &str, model: &str, system_prompt: &str) -> Result<String> {
    if api_key.is_empty() {
        return Err(AppError::Groq("Groq API key not set".to_string()));
    }

    let client = Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| AppError::Groq(e.to_string()))?;

    let body = GroqChatRequest {
        model,
        messages: vec![
            GroqChatMessage {
                role: "system",
                content: system_prompt,
            },
            GroqChatMessage {
                role: "user",
                content: text,
            },
        ],
        max_tokens: 4096,
        // Correction task — keep output deterministic.
        temperature: 0.1,
    };

    let resp = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                AppError::Groq("Request timed out (10s)".to_string())
            } else {
                AppError::Groq(e.to_string())
            }
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Groq(format!("HTTP {status}: {body}")));
    }

    let parsed: GroqChatResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Groq(format!("Failed to parse response: {e}")))?;

    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| AppError::Groq("Empty choices in response".to_string()))
}
