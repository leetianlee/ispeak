# Meeting transcription test fixtures

These fixtures exercise the audio decode and chunking paths. They are generated
synthetically rather than recorded, so the repo doesn't ship real speech.

- `30s-two-tones.wav` — 30 seconds, 44.1kHz stereo, two distinct sine tones
  (440Hz on left channel, 880Hz on right). Used to verify decode + resample +
  chunking without depending on a real model run.
- `short.m4a` — 5 seconds of the same content re-encoded as AAC in m4a.
- `short.mp4` — 5 seconds of black video with AAC audio in mp4 container.

To regenerate, run `cargo test --test fixtures_gen -- --ignored`.
