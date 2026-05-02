# iSpeak

Local-first AI voice-to-text for macOS. Hold a hotkey, speak, release — text is pasted at your cursor.

## Prerequisites

- macOS (Apple Silicon recommended)
- Node.js 18+
- Rust (via rustup)

## First run

```bash
# 1. Install JS dependencies
npm install

# 2. Run in development mode (first run compiles Rust — takes 3–5 minutes)
npm run tauri dev
```

On first launch, go to **Models** tab and download the **Medium** model (~1.5 GB).

## Default hotkey

`⌘ ⇧ Space` — hold to record (push-to-talk), release to transcribe and paste.

Configurable in the **Dictate** tab.

## Permissions required

macOS will ask for:
- **Microphone** — to capture your voice
- **Accessibility** — to paste text into other apps

Both are required for the app to work.

## Using Groq Cloud (faster)

1. Create a free account at [console.groq.com](https://console.groq.com)
2. Generate an API key
3. In iSpeak → Dictate → Transcription Engine → select **Groq Cloud**
4. Paste your key

## Project structure

```
src/              React frontend
src-tauri/        Rust backend
src/lib/contract.ts   Interface contract (read-only)
SPEC.md           Full product specification
```

## License

MIT
