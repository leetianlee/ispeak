# Tray Icon + Groq AI Switch + AI Eval

> Date: 2026-05-03
> Status: Design

Three small changes bundled together.

---

## 1. Tray Icon

**Behaviour:** A macOS menu bar icon. Left-click toggles the main window's visibility (show/hide). No menu, no right-click behaviour.

**Icon:** Use existing `src-tauri/icons/32x32.png` loaded as a template image (macOS renders it monochrome to match the menu bar).

**Implementation:** In `lib.rs` `.setup()`, create a `TrayIconBuilder` with:
- Icon from `32x32.png` (included via `tauri::image::Image`)
- Tooltip: `"iSpeak"`
- `on_tray_icon_event`: on left-click, toggle `main` window visibility

**No other changes.** The `tray-icon` Cargo feature is already enabled.

---

## 2. Switch Cloud AI from Anthropic to Groq

The app already uses Groq for cloud transcription (Whisper). Cloud AI post-processing currently uses Anthropic (Haiku/Sonnet), requiring a separate API key. Switch to Groq's LLM API so users only need one cloud key.

### Groq LLM API

Groq exposes an OpenAI-compatible chat completions endpoint:
```
POST https://api.groq.com/openai/v1/chat/completions
Authorization: Bearer <groq_api_key>
```

### Model mapping

| AI Mode       | Groq Model                   | Rationale                          |
|---------------|------------------------------|------------------------------------|
| `cloud_fast`  | `llama-3.3-70b-versatile`   | Fast, good quality for grammar fix |
| `cloud_quality` | `llama-3.3-70b-specdec`   | Same model, speculative decoding   |

Both are the same underlying model but `specdec` may have different latency characteristics. If Groq changes available models, these are easy to swap.

### Files changed

**`ai.rs`:**
- Remove `anthropic_complete()` and all Anthropic request/response structs
- Add `groq_chat_complete(text, api_key, model)` using the chat completions endpoint
- Same system prompt, same timeout (10s)
- `CloudFast` and `CloudQuality` call `groq_chat_complete` with different model strings
- Reuse `settings.groq_api_key` (no new field)

**`settings.rs`:**
- Remove `anthropic_api_key` field
- Remove `openai_api_key` field (unused)
- Remove `deepgram_api_key` field (unused)

**`error.rs`:**
- Remove `Anthropic` error variant (Groq variant already exists)

**`contract.ts`:**
- Remove `anthropic_api_key` from `AppSettings`
- Remove `deepgram_api_key` from `AppSettings`

**`SettingsPanel.tsx`:**
- Cloud Fast description: `"Groq Llama 70B, fast"` (was "Claude Haiku")
- Cloud Quality description: `"Groq Llama 70B, quality"` (was "Claude Sonnet")
- When cloud AI is selected, show Groq API key field (contextual — only visible when `ai_mode` is `cloud_fast` or `cloud_quality`, OR when `transcription_engine` is `groq`)
- Remove the Anthropic API key section entirely

### Migration

Existing users with `anthropic_api_key` stored: the field simply becomes ignored. Serde's `#[serde(default)]` means the deserialization won't break — the old key stays in the store file but is never read. No migration code needed.

---

## 3. AI Eval Template

A CSV file at `docs/ai-eval.csv` for manual quality evaluation of AI post-processing modes.

**Columns:**
- `sample_id` — sequential number
- `audio_description` — what was spoken (context for the evaluator)
- `raw_transcription` — Whisper output with AI off
- `ollama_output` — local AI result
- `groq_fast_output` — cloud fast result
- `groq_quality_output` — cloud quality result
- `best_mode` — evaluator's pick (off/ollama/groq_fast/groq_quality)
- `notes` — free-form observations

Pre-populated with header row and 5 empty numbered rows. The evaluator fills these in by hand after real dictation sessions.
