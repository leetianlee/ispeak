# Phase 2: AI Post-Processing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add optional AI grammar/punctuation correction after transcription, before paste, using Ollama (local) or Anthropic API (cloud).

**Architecture:** New `ai.rs` module with two async functions (`ollama_complete`, `anthropic_complete`) behind a `post_process` dispatcher. Wired into `execute_stop()` between transcription and clipboard write. AI failure is non-fatal — falls back to raw text.

**Tech Stack:** reqwest (already in Cargo.toml), Anthropic Messages API, Ollama `/api/generate` endpoint.

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `src-tauri/src/ai.rs` | Create | Ollama + Anthropic clients, `post_process` dispatcher |
| `src-tauri/src/settings.rs` | Modify | Add `ollama_model`, `ollama_base_url` fields |
| `src-tauri/src/error.rs` | Modify | Add `Ollama`, `Anthropic` error variants |
| `src-tauri/src/commands.rs` | Modify | Wire `ai::post_process` into `execute_stop` |
| `src-tauri/src/lib.rs` | Modify | Add `mod ai` declaration |
| `src/lib/contract.ts` | Modify | Remove `ai_provider`, add `ollama_model`/`ollama_base_url` |
| `src/components/SettingsPanel.tsx` | Modify | Ollama fields for Local mode, Anthropic-only for cloud |

---

### Task 1: Add Ollama settings fields to Rust backend

**Files:**
- Modify: `src-tauri/src/settings.rs:94-166`

- [ ] **Step 1: Add `ollama_model` and `ollama_base_url` to AppSettings struct**

In `src-tauri/src/settings.rs`, add two fields after `ai_mode` (line 109):

```rust
    #[serde(default)]
    pub ai_mode: AIMode,

    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,

    #[serde(default = "default_ollama_base_url")]
    pub ollama_base_url: String,

    #[serde(default)]
    pub groq_api_key: String,
```

Add the default functions after `default_true()` (line 146):

```rust
fn default_ollama_model() -> String {
    "llama3.2:3b".to_string()
}

fn default_ollama_base_url() -> String {
    "http://localhost:11434".to_string()
}
```

Update the `Default` impl (lines 148-165), adding after `ai_mode`:

```rust
            ai_mode: AIMode::default(),
            ollama_model: default_ollama_model(),
            ollama_base_url: default_ollama_base_url(),
            groq_api_key: String::new(),
```

- [ ] **Step 2: Verify it compiles**

Run from `src-tauri/`:
```bash
cargo check 2>&1 | head -5
```
Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/settings.rs
git commit -m "feat: add ollama_model and ollama_base_url to AppSettings"
```

---

### Task 2: Add AI error variants

**Files:**
- Modify: `src-tauri/src/error.rs:4-25`

- [ ] **Step 1: Add Ollama and Anthropic error variants**

In `src-tauri/src/error.rs`, add after the `Groq` variant (line 12):

```rust
    #[error("Groq API error: {0}")]
    Groq(String),

    #[error("Ollama error: {0}")]
    Ollama(String),

    #[error("Anthropic API error: {0}")]
    Anthropic(String),
```

- [ ] **Step 2: Verify it compiles**

Run from `src-tauri/`:
```bash
cargo check 2>&1 | head -5
```
Expected: `Finished` with no errors (warnings about unused variants are fine).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/error.rs
git commit -m "feat: add Ollama and Anthropic error variants"
```

---

### Task 3: Create ai.rs module

**Files:**
- Create: `src-tauri/src/ai.rs`
- Modify: `src-tauri/src/lib.rs:1-7`

- [ ] **Step 1: Create `src-tauri/src/ai.rs` with the full module**

```rust
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
```

- [ ] **Step 2: Register the module in lib.rs**

In `src-tauri/src/lib.rs`, add `mod ai;` after the existing module declarations (line 1):

```rust
mod ai;
mod audio;
mod commands;
mod error;
mod groq;
mod paste;
mod settings;
mod whisper_engine;
```

- [ ] **Step 3: Verify it compiles**

Run from `src-tauri/`:
```bash
cargo check 2>&1 | head -5
```
Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/ai.rs src-tauri/src/lib.rs
git commit -m "feat: add ai.rs module with Ollama and Anthropic clients"
```

---

### Task 4: Wire AI post-processing into execute_stop

**Files:**
- Modify: `src-tauri/src/commands.rs:186-206`

- [ ] **Step 1: Replace the raw_text passthrough with AI post-processing**

In `src-tauri/src/commands.rs`, replace lines 186-206 (from `let text = raw_text.clone();` through building the `TranscriptResult`) with:

```rust
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
```

Note: the `ai_mode` field in `TranscriptResult` now reflects the actual setting instead of hardcoded `"off"`.

- [ ] **Step 2: Add the AIMode import if not already present**

At the top of `commands.rs`, ensure the settings import includes `AIMode`:

```rust
use crate::settings::{AppSettings, RecordingMode, TranscriptionEngine, AIMode, /* ... */};
```

Check the existing import line and add `AIMode` if missing.

- [ ] **Step 3: Verify it compiles**

Run from `src-tauri/`:
```bash
cargo check 2>&1 | head -10
```
Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: wire AI post-processing into execute_stop pipeline"
```

---

### Task 5: Update contract.ts

**Files:**
- Modify: `src/lib/contract.ts:26-86`

Note: contract.ts is read-only per SPEC.md, but this change is documented in the Phase 2 design spec as an architectural decision.

- [ ] **Step 1: Remove AIProvider type and update AppSettings**

In `src/lib/contract.ts`:

1. Remove the `AIProvider` type line:
```typescript
// DELETE this line:
export type AIProvider = 'ollama' | 'anthropic' | 'openai' | 'groq'
```

2. In the `AppSettings` interface, remove `ai_provider` and `openai_api_key`, and ensure `ollama_model` and `ollama_base_url` are present:

```typescript
export interface AppSettings {
  hotkey: string
  recording_mode: RecordingMode
  transcription_engine: TranscriptionEngine
  whisper_model: WhisperModel
  ai_mode: AIMode
  ollama_model: string
  ollama_base_url: string
  groq_api_key: string
  anthropic_api_key: string
  deepgram_api_key: string
  microphone_id: string | null
  indicator_position: { x: number; y: number }
  max_recording_duration_s: number
  dark_mode: boolean
}
```

- [ ] **Step 2: Verify frontend compiles**

Run from project root:
```bash
npx tsc --noEmit 2>&1 | head -10
```
Expected: No errors. If there are errors referencing `AIProvider` or `openai_api_key`, fix them in the next task (SettingsPanel).

- [ ] **Step 3: Commit**

```bash
git add src/lib/contract.ts
git commit -m "feat: simplify contract — remove AIProvider, add ollama fields"
```

---

### Task 6: Update frontend AI settings UI

**Files:**
- Modify: `src/components/SettingsPanel.tsx`
- Modify: `src/store/useAppStore.ts` (only if it references removed fields)

- [ ] **Step 1: Update the AI tab in SettingsPanel.tsx**

Find the AI tab content section (the block inside `{activeTab === "ai" && settings && (`). Replace it entirely with:

```tsx
            <Section title="AI Post-Processing">
              <p className="text-xs text-slate-500 mb-3">
                Applies grammar correction and formatting after transcription.
              </p>
              <RadioGroup
                value={settings.ai_mode}
                options={[
                  { value: "off", label: "Off", desc: "Raw transcription, zero latency" },
                  { value: "local", label: "Local (Ollama)", desc: "Free, ~300ms extra, requires Ollama running" },
                  { value: "cloud_fast", label: "Cloud Fast", desc: "Claude Haiku, ~$0.001/call" },
                  { value: "cloud_quality", label: "Cloud Quality", desc: "Claude Sonnet, ~$0.005/call" },
                ]}
                onChange={(v) => save({ ai_mode: v })}
              />
            </Section>

            {settings.ai_mode === "local" && (
              <div className="space-y-3 bg-[#0f1117]/50 rounded-lg p-3 border border-[#1e2535]/50">
                <InlineField label="Ollama model">
                  <input
                    className="w-48 bg-[#0f1117] border border-[#2a3347] rounded-md px-2.5 py-1.5 text-xs text-slate-200 focus:outline-none focus:border-indigo-500 font-mono"
                    value={settings.ollama_model}
                    onChange={(e) => save({ ollama_model: e.target.value })}
                    placeholder="llama3.2:3b"
                  />
                </InlineField>
                <InlineField label="Ollama URL">
                  <input
                    className="w-48 bg-[#0f1117] border border-[#2a3347] rounded-md px-2.5 py-1.5 text-xs text-slate-200 focus:outline-none focus:border-indigo-500 font-mono"
                    value={settings.ollama_base_url}
                    onChange={(e) => save({ ollama_base_url: e.target.value })}
                    placeholder="http://localhost:11434"
                  />
                </InlineField>
              </div>
            )}

            {(settings.ai_mode === "cloud_fast" || settings.ai_mode === "cloud_quality") && (
              <Section title="API Key">
                <ApiKeyField
                  label="Anthropic API Key"
                  value={settings.anthropic_api_key}
                  placeholder="sk-ant-..."
                  onSave={(v) => save({ anthropic_api_key: v })}
                />
              </Section>
            )}
```

- [ ] **Step 2: Verify frontend compiles**

Run from project root:
```bash
npx tsc --noEmit 2>&1 | head -10
```
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add src/components/SettingsPanel.tsx
git commit -m "feat: update AI tab — Ollama fields for local, Anthropic-only for cloud"
```

---

### Task 7: Full build and manual test

**Files:** None (testing only)

- [ ] **Step 1: Build the app**

```bash
npm run tauri build 2>&1
```
Expected: `.app` bundle created successfully (DMG may fail, that's fine).

- [ ] **Step 2: Test AI mode Off (regression)**

1. Open the app
2. Ensure AI mode is set to Off
3. Use hotkey to dictate
4. Verify raw text is pasted correctly

- [ ] **Step 3: Test AI mode Local (Ollama)**

Prerequisites: Ollama running with `llama3.2:3b` pulled.

1. Set AI mode to Local
2. Verify Ollama model and URL fields appear
3. Use hotkey to dictate something with intentional grammar errors
4. Verify corrected text is pasted

- [ ] **Step 4: Test AI mode Local with Ollama down**

1. Stop Ollama
2. Set AI mode to Local
3. Use hotkey to dictate
4. Verify raw text is pasted (fallback) and no crash occurs

- [ ] **Step 5: Test AI mode Cloud Fast**

Prerequisites: Valid Anthropic API key.

1. Set AI mode to Cloud Fast
2. Set Anthropic API key
3. Use hotkey to dictate with grammar errors
4. Verify corrected text is pasted via Haiku

- [ ] **Step 6: Test AI mode Cloud Quality**

1. Set AI mode to Cloud Quality
2. Use hotkey to dictate
3. Verify corrected text is pasted via Sonnet

- [ ] **Step 7: Test Cloud with bad API key**

1. Set a fake API key
2. Use hotkey to dictate
3. Verify raw text is pasted (fallback) and no crash

- [ ] **Step 8: Commit final state**

```bash
git add -A
git commit -m "feat: Phase 2 — AI post-processing complete"
```
