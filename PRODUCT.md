# Product

## Register

product

## Users

Knowledge workers (developers, writers, managers) who want fast voice-to-text without leaving their current workflow. They open iSpeak mid-task, not as a destination. The app lives in the background; the interaction is a hotkey press, a few seconds of speaking, and text appearing at the cursor. Future users include meeting participants who need transcripts without switching tools.

## Product Purpose

Reduce the friction between human thought and machine text. iSpeak replaces typing with speaking for short bursts, keeping the user in flow. It runs locally by default (no cloud dependency, no accounts), processes speech in seconds, and pastes directly where the user is working. Success means the user forgets the tool exists between uses.

## Brand Personality

Quiet, fast, invisible. The interface communicates confidence through restraint. No onboarding wizards, no tooltips, no celebration animations. Status is shown; attention is not demanded.

## Anti-references

- Microsoft Teams recording UI (heavy, modal, pulls you out of context)
- Chatbot-style interfaces (conversational chrome around a simple action)
- Otter.ai (meeting-first UI with social features, transcripts as a destination)
- Bloated Electron apps with web-app aesthetics on desktop

The vision is software 3.0: deeply embedded in the workflow, not a separate surface you visit. Closer to a system utility than an application.

## Design Principles

1. **Disappear when not needed.** The best state is invisible. Recording indicator, hotkey, paste. No persistent window required.
2. **Speed is the feature.** Every interaction should feel instant. Latency in the UI is as bad as latency in the transcription.
3. **Respect the workflow.** Never pull focus, never interrupt, never require context-switching. The user's app stays in front.
4. **Earn trust through transparency.** Show what engine is running, where data goes, what permissions are needed. No magic boxes.
5. **Local by default.** Cloud is an upgrade, not a requirement. The app works offline out of the box.

## Accessibility & Inclusion

- WCAG AA contrast ratios for all text
- Keyboard-navigable settings (the app is hotkey-driven by nature)
- Respect reduced-motion preferences for the recording pulse animation
- No color-only status indicators (recording state uses text labels alongside color)
