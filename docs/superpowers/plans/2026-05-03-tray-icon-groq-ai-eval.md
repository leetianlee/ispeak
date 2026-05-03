# Tray Icon + Groq AI Switch + AI Eval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a menu bar tray icon (click toggles window), switch cloud AI from Anthropic to Groq, and create an AI eval CSV template.

**Architecture:** Three independent changes. Task 1 adds the tray icon in `lib.rs`. Tasks 2-4 replace Anthropic with Groq across backend, contract, and frontend. Task 5 creates the eval CSV.

**Tech Stack:** Tauri 2 tray API, Groq OpenAI-compatible chat completions API, Rust, TypeScript/React.

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `src-tauri/src/lib.rs` | Modify | Add tray icon setup |
| `src-tauri/src/ai.rs` | Modify | Replace Anthropic client with Groq chat completions |
| `src-tauri/src/error.rs` | Modify | Remove `Anthropic` variant |
| `src-tauri/src/settings.rs` | Modify | Remove `anthropic_api_key`, `openai_api_key`, `deepgram_api_key` |
| `src-tauri/src/commands.rs` | Modify | Remove masking/loading of deleted key fields |
| `src/lib/contract.ts` | Modify | Remove `anthropic_api_key`, `deepgram_api_key` from `AppSettings` |
| `src/components/SettingsPanel.tsx` | Modify | Update AI tab labels, swap Anthropic key for Groq key |
| `docs/ai-eval.csv` | Create | Evaluation template |

---

### Task 1: Add Tray Icon

**Files:**
- Modify: `src-tauri/src/lib.rs:77-91` (inside `.setup()`)

- [ ] **Step 1: Add tray icon imports and setup in lib.rs**

Add to the top of `lib.rs`:

```rust
use tauri::tray::TrayIconBuilder;
```

Inside the `.setup(|app| { ... })` closure, after the hotkey registration block (after line 89), add:

```rust
            TrayIconBuilder::new("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("iSpeak")
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { .. } = event {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;
```

- [ ] **Step 2: Build and verify tray icon works**

Run: `npm run tauri dev`

Expected: iSpeak icon appears in macOS menu bar. Clicking it toggles the main window visibility. Verify:
1. Click tray icon → window hides
2. Click tray icon again → window shows and focuses

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: add menu bar tray icon — click toggles main window"
```

---

### Task 2: Replace Anthropic with Groq in Backend

**Files:**
- Modify: `src-tauri/src/ai.rs` (replace Anthropic structs/function with Groq chat completions)
- Modify: `src-tauri/src/error.rs:18-19` (remove Anthropic variant)

- [ ] **Step 1: Replace Anthropic code with Groq chat completions in ai.rs**

Replace the entire `// ─── Anthropic ───` section (lines 105-186) with:

```rust
// ─── Groq Chat ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct GroqChatRequest<'a> {
    model: &'a str,
    messages: Vec<GroqChatMessage<'a>>,
    max_tokens: u32,
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

async fn groq_chat_complete(text: &str, api_key: &str, model: &str) -> Result<String> {
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
                content: SYSTEM_PROMPT,
            },
            GroqChatMessage {
                role: "user",
                content: text,
            },
        ],
        max_tokens: 4096,
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
```

- [ ] **Step 2: Update post_process() to call groq_chat_complete**

Replace the `CloudFast` and `CloudQuality` match arms in `post_process()` (lines 23-34) with:

```rust
        AIMode::CloudFast => {
            groq_chat_complete(raw_text, &settings.groq_api_key, "llama-3.3-70b-versatile")
                .await?
        }
        AIMode::CloudQuality => {
            groq_chat_complete(raw_text, &settings.groq_api_key, "llama-3.3-70b-specdec")
                .await?
        }
```

- [ ] **Step 3: Remove Anthropic error variant from error.rs**

Delete these two lines from `error.rs`:

```rust
    #[error("Anthropic API error: {0}")]
    Anthropic(String),
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check` from `src-tauri/`

Expected: No errors. If there are stale references to `AppError::Anthropic`, fix them (there should be none after the ai.rs changes).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/ai.rs src-tauri/src/error.rs
git commit -m "feat: switch cloud AI post-processing from Anthropic to Groq"
```

---

### Task 3: Remove Unused API Key Fields from Settings & Commands

**Files:**
- Modify: `src-tauri/src/settings.rs:94-140` (AppSettings struct), `src-tauri/src/settings.rs:162-182` (Default impl)
- Modify: `src-tauri/src/commands.rs:352-361` (get_settings), `src-tauri/src/commands.rs:373-380` (update_settings key filter), `src-tauri/src/commands.rs:425-442` (load_settings)

- [ ] **Step 1: Remove fields from AppSettings in settings.rs**

Remove these three fields from the `AppSettings` struct:

```rust
    #[serde(default)]
    pub anthropic_api_key: String,

    #[serde(default)]
    pub openai_api_key: String,

    #[serde(default)]
    pub deepgram_api_key: String,
```

Remove the same three fields from the `Default` impl:

```rust
            anthropic_api_key: String::new(),
            openai_api_key: String::new(),
            deepgram_api_key: String::new(),
```

- [ ] **Step 2: Update get_settings in commands.rs**

In `get_settings()`, remove the three masking lines:

```rust
    settings.anthropic_api_key = mask_key(&settings.anthropic_api_key);
    settings.openai_api_key = mask_key(&settings.openai_api_key);
    settings.deepgram_api_key = mask_key(&settings.deepgram_api_key);
```

- [ ] **Step 3: Update update_settings key filter in commands.rs**

Replace the `is_key_field` match in `update_settings()` (line 377-379):

```rust
            let is_key_field = matches!(
                key.as_str(),
                "groq_api_key"
            );
```

- [ ] **Step 4: Update load_settings in commands.rs**

Remove these three lines from `load_settings()`:

```rust
        anthropic_api_key: get!("anthropic_api_key", String::new()),
        openai_api_key: get!("openai_api_key", String::new()),
        deepgram_api_key: get!("deepgram_api_key", String::new()),
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check` from `src-tauri/`

Expected: Clean compile, no errors.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/settings.rs src-tauri/src/commands.rs
git commit -m "refactor: remove unused anthropic/openai/deepgram API key fields"
```

---

### Task 4: Update Frontend Contract and UI

**Files:**
- Modify: `src/lib/contract.ts:70-72` (AppSettings interface)
- Modify: `src/components/SettingsPanel.tsx:185-234` (AI tab)

- [ ] **Step 1: Remove deleted keys from contract.ts**

Remove these two lines from the `AppSettings` interface in `contract.ts`:

```typescript
  anthropic_api_key: string
  deepgram_api_key: string
```

- [ ] **Step 2: Update AI tab radio descriptions in SettingsPanel.tsx**

Replace the AI mode radio options (lines 193-199):

```tsx
                options={[
                  { value: "off", label: "Off", desc: "Raw transcription, zero latency" },
                  { value: "local", label: "Local (Ollama)", desc: "Free, ~300ms extra, requires Ollama running" },
                  { value: "cloud_fast", label: "Cloud Fast", desc: "Groq Llama 70B, fast" },
                  { value: "cloud_quality", label: "Cloud Quality", desc: "Groq Llama 70B, quality" },
                ]}
```

- [ ] **Step 3: Replace Anthropic key section with Groq key for cloud AI**

Replace the cloud AI key section (lines 224-233) with:

```tsx
            {(settings.ai_mode === "cloud_fast" || settings.ai_mode === "cloud_quality") && !settings.groq_api_key && (
              <Section title="API Key">
                <ApiKeyField
                  label="Groq API Key"
                  value={settings.groq_api_key}
                  placeholder="gsk_..."
                  onSave={(v) => save({ groq_api_key: v })}
                />
              </Section>
            )}

            {(settings.ai_mode === "cloud_fast" || settings.ai_mode === "cloud_quality") && settings.groq_api_key && (
              <Section title="API Key">
                <ApiKeyField
                  label="Groq API Key"
                  value={settings.groq_api_key}
                  placeholder="gsk_..."
                  onSave={(v) => save({ groq_api_key: v })}
                />
              </Section>
            )}
```

Actually, simplify — the condition is the same regardless of whether the key exists (the `ApiKeyField` component handles both states). Replace lines 224-233 with:

```tsx
            {(settings.ai_mode === "cloud_fast" || settings.ai_mode === "cloud_quality") && (
              <Section title="API Key">
                <ApiKeyField
                  label="Groq API Key"
                  value={settings.groq_api_key}
                  placeholder="gsk_..."
                  onSave={(v) => save({ groq_api_key: v })}
                />
              </Section>
            )}
```

- [ ] **Step 4: Build and verify UI**

Run: `npm run tauri dev`

Expected:
1. AI tab shows "Groq Llama 70B" for cloud options (not "Claude Haiku/Sonnet")
2. Selecting Cloud Fast or Cloud Quality shows Groq API key field
3. If Groq key is already set from Dictate tab transcription, it shows as configured
4. No TypeScript errors

- [ ] **Step 5: Commit**

```bash
git add src/lib/contract.ts src/components/SettingsPanel.tsx
git commit -m "feat: update frontend — Groq for cloud AI, remove Anthropic key"
```

---

### Task 5: Create AI Eval CSV Template

**Files:**
- Create: `docs/ai-eval.csv`

- [ ] **Step 1: Create the CSV file**

```csv
sample_id,audio_description,raw_transcription,ollama_output,groq_fast_output,groq_quality_output,best_mode,notes
1,,,,,,,
2,,,,,,,
3,,,,,,,
4,,,,,,,
5,,,,,,,
```

- [ ] **Step 2: Commit**

```bash
git add docs/ai-eval.csv
git commit -m "docs: add AI eval CSV template for manual quality evaluation"
```

---

### Task 6: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update CLAUDE.md to reflect Groq switch and tray icon**

In the "Current Status" section, add under Phase 2:
- Cloud AI modes now use Groq LLM API (Llama 3.3 70B) instead of Anthropic — single API key for both transcription and AI post-processing.
- Tray icon added — click toggles main window visibility.

In "Key Technical Notes", replace the Anthropic references:
- Cloud AI uses Groq chat completions API (`/openai/v1/chat/completions`), same key as Groq transcription

In "Contract simplification" note, update:
- Cloud AI modes use Groq only (Llama 70B for fast, Llama 70B specdec for quality). No Anthropic dependency.

Remove `anthropic_api_key` mentions. Remove the "Next Steps" bullet about re-adding tray icon (done).

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md — Groq cloud AI, tray icon complete"
```
