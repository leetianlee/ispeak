# Phase 3.1 — Manual smoke test

Pre-flight:
- [ ] Local Whisper model downloaded (small or larger) in Settings → Models
- [ ] Optional: Groq API key set in Settings

## File import — local Whisper

1. Run `npm run tauri dev`
2. Click the **Transcribe** tab
3. Drop a short (<2 min) real-speech wav or m4a onto the drop zone
4. Verify:
   - [ ] Job appears in *In progress* with chunk progress
   - [ ] Progress bar advances as chunks complete
   - [ ] Result card appears under *Results* when done
   - [ ] Speakers all labelled "Speaker" (no diarisation in 3.1 — expected)
   - [ ] `Copy MD` puts a Markdown transcript on the clipboard
   - [ ] `Save .md` writes a Markdown file with header + segments

## File import — Groq cloud

1. Repeat the above with Settings → Transcription engine = Groq
2. Same checks; should be noticeably faster

## Cancel during job

1. Drop a longer (>5 min) file
2. Click *Cancel* mid-progress
3. Verify:
   - [ ] Job stops within ~30s
   - [ ] Partial transcript appears with ⚠ Partial badge
   - [ ] Markdown export still works

## Unsupported format

1. Drop a `.txt` file
2. Verify:
   - [ ] Error message in the UI: "Unsupported format: .txt"
   - [ ] No job started

## Long file (>30 min)

1. Drop a 30+ min audio file
2. Verify:
   - [ ] Job completes (may take 10+ min on local)
   - [ ] Memory stays bounded (check Activity Monitor — should not exceed ~1GB)
   - [ ] PCM temp dir cleaned up after job (`ls $TMPDIR/iSpeak-jobs-*` returns nothing)
