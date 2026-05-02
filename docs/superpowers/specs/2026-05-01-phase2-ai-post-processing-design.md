# Phase 2: AI Post-Processing — Design Spec

> Date: 2026-05-01
> Status: Approved
> Scope: Add AI grammar/punctuation correction after transcription, before paste

---

## Goal

After Whisper transcribes spoken audio to text, optionally run it through an LLM to fix grammar, punctuation, and capitalization before pasting. The user chooses between local (Ollama) and cloud (Anthropic) processing, or turns it off entirely.

## Non-Goals

- App-context-aware prompts (detect frontmost app). Deferred to Phase 2.1.
- OpenAI integration. Only Anthropic for cloud.
- Prompt caching. Can optimize later.
- Custom vocabulary or user-defined prompt overrides.
- Streaming responses. The text is short; wait for the full response.

---

## AI Modes

| AI Mode | Provider | Model | Expected Latency |
|---|---|---|---|
| Off (default) | none | none | 0ms |
| Local | Ollama HTTP API | configurable, default `llama3.2:3b` | ~300ms |
| Cloud Fast | Anthropic Messages API | `claude-haiku-4-5-20251001` | ~400ms |
| Cloud Quality | Anthropic Messages API | `claude-sonnet-4-5-20250514` | ~800ms |

---

## Prompt

Single universal system prompt, under 200 tokens:

```
You are a dictation assistant. The user spoke the following text aloud and it was transcribed automatically. Fix grammar, punctuation, and capitalization errors. Do not change meaning, tone, or wording beyond corrections. Output only the corrected text, nothing else.
```

The user's transcribed text is sent as a single user message. The LLM response is the corrected text, used directly (no parsing needed).

---

## Data Flow

```
execute_stop()
  1. Audio capture stops
  2. Whisper/Groq transcription → raw_text
  3. If ai_mode == Off → skip to step 5
  4. ai::post_process(raw_text, ai_mode, &settings) → processed_text
     - On success: use processed_text
     - On error: log warning, fall back to raw_text (never block paste on AI failure)
  5. Clipboard write (processed_text or raw_text)
  6. Paste via Cmd+V
  7. Emit TranscriptResult { text, raw_text, ai_mode }
```

Key design decision: **AI failure is non-fatal.** If Ollama is down or the API key is invalid, the user still gets their raw transcription pasted. The error is logged and emitted via the `app_error` event, but the pipeline continues.

---

## Backend Changes

### 1. settings.rs — Add missing fields

Add to `AppSettings`:

```rust
pub ollama_model: String,     // default: "llama3.2:3b"
pub ollama_base_url: String,  // default: "http://localhost:11434"
```

Remove `ai_provider` (no longer needed with fixed mapping). The `openai_api_key` and `deepgram_api_key` fields stay in the struct for forward compatibility but are unused in Phase 2.

### 2. error.rs — Add error variants

```rust
Ollama(String),
Anthropic(String),
```

### 3. New module: ai.rs

Public interface:

```rust
pub async fn post_process(
    raw_text: &str,
    ai_mode: AIMode,
    settings: &AppSettings,
) -> Result<String, AppError>
```

Dispatches based on `ai_mode`:

- **Local** → `ollama_complete(raw_text, &settings.ollama_base_url, &settings.ollama_model)`
  - POST to `{base_url}/api/generate`
  - Body: `{ "model": model, "system": SYSTEM_PROMPT, "prompt": raw_text, "stream": false }`
  - Parse `response` field from JSON response

- **CloudFast** → `anthropic_complete(raw_text, &settings.anthropic_api_key, "claude-haiku-4-5-20251001")`
- **CloudQuality** → `anthropic_complete(raw_text, &settings.anthropic_api_key, "claude-sonnet-4-5-20250514")`
  - POST to `https://api.anthropic.com/v1/messages`
  - Headers: `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`
  - Body: `{ "model": model, "max_tokens": 4096, "system": SYSTEM_PROMPT, "messages": [{ "role": "user", "content": raw_text }] }`
  - Parse `content[0].text` from JSON response

- **Off** → unreachable (caller checks before calling)

Timeout: 10 seconds for all providers. On timeout, return error (caller falls back to raw_text).

### 4. commands.rs — Wire into execute_stop()

After transcription succeeds and before clipboard write:

```rust
let ai_mode = settings.ai_mode.clone();
let final_text = if ai_mode != AIMode::Off {
    match ai::post_process(&raw_text, ai_mode, &settings).await {
        Ok(processed) => processed,
        Err(e) => {
            // Emit error event but don't block paste
            emit_error(&app, "ai_post_process", &e.to_string());
            raw_text.clone()
        }
    }
} else {
    raw_text.clone()
};
```

### 5. lib.rs — Register ai module

Add `mod ai;` to the module declarations.

---

## Frontend Changes

### 6. SettingsPanel.tsx — AI tab updates

When **Local (Ollama)** is selected, show two additional fields below the radio group:
- Ollama model name (text input, default "llama3.2:3b")
- Ollama base URL (text input, default "http://localhost:11434")

Both use the compact `InlineField` layout, grouped in a bordered panel like the Dictate tab's compact settings.

When **Cloud Fast** or **Cloud Quality** is selected, show only the Anthropic API key field (remove the OpenAI key field).

### 7. contract.ts — Simplify

Remove `ai_provider` from `AIProvider` type and `AppSettings`. Remove `openai_api_key` from `AppSettings`. Add `ollama_model` and `ollama_base_url` if not already present.

Note: contract.ts is read-only per SPEC.md. These changes constitute an architectural decision documented in this spec.

---

## Error Handling

| Scenario | Behavior |
|---|---|
| Ollama not running | AI error event emitted, raw_text pasted |
| Ollama model not pulled | AI error event emitted, raw_text pasted |
| Invalid Anthropic API key | AI error event emitted, raw_text pasted |
| Anthropic rate limited | AI error event emitted, raw_text pasted |
| API timeout (>10s) | AI error event emitted, raw_text pasted |
| Empty AI response | Use raw_text (treat as error) |
| AI mode is Off | Skip AI entirely, no overhead |

The principle: AI post-processing is best-effort. The user's transcription always gets pasted.

---

## Testing

Manual testing checklist:
1. AI mode Off: transcription pastes raw text as before (regression check)
2. AI mode Local: start Ollama, pull llama3.2:3b, dictate — verify grammar correction
3. AI mode Local with Ollama stopped: verify raw text pastes with error logged
4. AI mode Cloud Fast: set Anthropic key, dictate — verify correction via Haiku
5. AI mode Cloud Quality: same with Sonnet
6. AI mode Cloud with bad key: verify raw text pastes with error logged
7. Settings persistence: change AI mode, restart app, verify mode persists
8. Ollama model/URL fields: verify they appear/disappear with Local mode selection

---

## Files Modified

| File | Change |
|---|---|
| `src-tauri/src/ai.rs` | New — Ollama and Anthropic clients |
| `src-tauri/src/settings.rs` | Add `ollama_model`, `ollama_base_url` fields |
| `src-tauri/src/error.rs` | Add `Ollama`, `Anthropic` variants |
| `src-tauri/src/commands.rs` | Wire `ai::post_process` into `execute_stop` |
| `src-tauri/src/lib.rs` | Add `mod ai` |
| `src/lib/contract.ts` | Remove `ai_provider`, `openai_api_key`; verify `ollama_*` fields |
| `src/components/SettingsPanel.tsx` | Ollama fields for Local mode, Anthropic-only for cloud |
