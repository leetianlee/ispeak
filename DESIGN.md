---
name: iSpeak
description: Local-first voice dictation for macOS
colors:
  deep-void: "#09090e"
  dark-well: "#0f1117"
  slate-bed: "#1e2535"
  border-dim: "#2a3347"
  border-hover: "#3a4357"
  muted-steel: "#4a5568"
  subtle-fog: "#94a3b8"
  text-primary: "#e2e8f0"
  indigo-accent: "#6366f1"
  indigo-deep: "#312e81"
  recording-red: "#ef4444"
  processing-amber: "#f59e0b"
  success-green: "#10b981"
typography:
  body:
    fontFamily: "-apple-system, BlinkMacSystemFont, Inter, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "-apple-system, BlinkMacSystemFont, Inter, sans-serif"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: 1.33
    letterSpacing: "0.025em"
  section-title:
    fontFamily: "-apple-system, BlinkMacSystemFont, Inter, sans-serif"
    fontSize: "12px"
    fontWeight: 600
    lineHeight: 1.33
    letterSpacing: "0.05em"
  mono:
    fontFamily: "JetBrains Mono, Fira Code, monospace"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.5
rounded:
  sm: "6px"
  md: "8px"
  lg: "12px"
  full: "9999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "20px"
components:
  button-primary:
    backgroundColor: "{colors.indigo-accent}"
    textColor: "#ffffff"
    rounded: "{rounded.sm}"
    padding: "6px 12px"
  button-primary-hover:
    backgroundColor: "#818cf8"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.subtle-fog}"
    rounded: "{rounded.sm}"
    padding: "6px 12px"
  button-ghost-hover:
    textColor: "{colors.text-primary}"
  input-default:
    backgroundColor: "{colors.dark-well}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.sm}"
    padding: "8px 12px"
  input-focus:
    backgroundColor: "{colors.dark-well}"
    textColor: "{colors.text-primary}"
  tab-active:
    backgroundColor: "{colors.slate-bed}"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.sm}"
    padding: "6px 12px"
  tab-inactive:
    backgroundColor: "transparent"
    textColor: "{colors.muted-steel}"
    rounded: "{rounded.sm}"
    padding: "6px 12px"
  radio-card-selected:
    backgroundColor: "rgba(99, 102, 241, 0.05)"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.lg}"
    padding: "12px"
  radio-card-unselected:
    backgroundColor: "transparent"
    textColor: "{colors.text-primary}"
    rounded: "{rounded.lg}"
    padding: "12px"
  status-badge-idle:
    backgroundColor: "#1e293b"
    textColor: "{colors.subtle-fog}"
    rounded: "{rounded.full}"
    padding: "2px 8px"
  status-badge-recording:
    backgroundColor: "rgba(127, 29, 29, 0.8)"
    textColor: "{colors.recording-red}"
    rounded: "{rounded.full}"
    padding: "2px 8px"
---

# Design System: iSpeak

## 1. Overview

**Creative North Star: "The Voice Channel"**

iSpeak is a focused conduit between thought and text. The interface exists to be crossed, not inhabited. Users press a hotkey, speak, and the words appear at their cursor. The settings panel configures that channel; the UI never becomes the destination.

The visual system is dark, dense, and recessive. It borrows from system utilities (Raycast, macOS Preferences) rather than consumer apps or SaaS dashboards. Every element earns its pixel budget through function. Decoration is absent. Color is reserved for state communication (recording, processing, success) and a single indigo accent that marks interactive elements.

This system explicitly rejects chatbot-style conversational interfaces, Microsoft Teams' modal-heavy recording UI, and any pattern that pulls the user out of their active workflow. iSpeak is software 3.0: deeply embedded, never foregrounded.

**Key Characteristics:**
- Dark tonal layering (no shadows, depth through surface steps)
- Single accent color (indigo) used sparingly for interactive affordance
- State colors (red, amber, green) reserved for recording lifecycle feedback
- Compact density; 12-14px type scale; no whitespace waste
- System font stack; monospace for technical values (hotkeys, API keys, URLs)

## 2. Colors

A restrained palette of indigo-tinted darks with one accent. Color exists to communicate state, not to decorate.

### Primary
- **Indigo Accent** (#6366f1): Interactive elements, selected states, focus rings, active tab indicators. Used at full opacity on buttons and progress bars; at 5-10% opacity as selection tints on radio cards and model cards.
- **Deep Indigo** (#312e81): Gradient origin for the app icon and subtle background radial wash at the top of the window.

### Neutral
- **Deep Void** (#09090e): App background. Not pure black; carries a faint blue-violet tint.
- **Dark Well** (#0f1117): Input backgrounds, transcript cards, dropdown menus. One step above void.
- **Slate Bed** (#1e2535): Active tab fills, compact settings panels, button backgrounds. The primary "elevated surface."
- **Border Dim** (#2a3347): Default borders on inputs, cards, dividers. Subtle but present.
- **Border Hover** (#3a4357): Hover state for borders. One step brighter.
- **Muted Steel** (#4a5568): Inactive tab text, placeholder text, tertiary labels.
- **Subtle Fog** (#94a3b8): Secondary text, descriptions, metadata.
- **Text Primary** (#e2e8f0): Body text, labels, headings. Warm off-white.

### State
- **Recording Red** (#ef4444): Recording indicator dot, hero glow during capture, status badge.
- **Processing Amber** (#f59e0b): Transcribing state, download progress percentage.
- **Success Green** (#10b981): API key configured indicator dot.

### Named Rules
**The One Accent Rule.** Indigo is the only chromatic color in resting UI. Red, amber, and green appear only during state transitions (recording, processing, configured). If a new element needs color, it uses indigo or it uses a neutral.

## 3. Typography

**Body Font:** System stack (-apple-system, BlinkMacSystemFont, Inter, sans-serif)
**Mono Font:** JetBrains Mono (with Fira Code fallback)

**Character:** Native macOS feel. The system font stack ensures the app reads as a first-class desktop citizen, not a web app in a wrapper. Monospace is reserved for values the user might type or copy (hotkey strings, API keys, model names, URLs).

### Hierarchy
- **Section Title** (600, 12px, tracking 0.05em, uppercase): Section headers in settings. Muted steel color. The uppercase + tracking treatment is the only typographic flourish in the system.
- **Body** (500, 14px, line-height 1.5): Radio option labels, model names, primary descriptive text.
- **Label** (500, 12px, tracking 0.025em): Tab labels, inline field labels, button text, status badge text.
- **Caption** (400, 12px): Descriptions under radio options, metadata lines, hints. Uses subtle-fog or muted-steel color.
- **Micro** (500, 10px, tracking 0.1em, uppercase): Settings divider label, footer text. Barely visible.
- **Mono** (400, 12px): Hotkey input, API key display, Ollama URL/model fields.

### Named Rules
**The Two Size Rule.** The entire interface uses only 12px and 14px type, plus 10px for micro labels. Hierarchy is built through weight (400/500/600), color (primary/subtle/muted), case (uppercase for section titles), and tracking. Not through size proliferation.

## 4. Elevation

Flat by default. Depth is conveyed through tonal layering, not shadows.

Four surface tones step from Deep Void (#09090e) through Dark Well (#0f1117) and Slate Bed (#1e2535) to Border Dim (#2a3347). Each step adds roughly 4-6 lightness points. Borders at Border Dim provide edge definition; no surface casts a shadow.

The one exception is the DictateHero mic circle, which uses a colored box-shadow glow to communicate state: faint indigo at rest, red during recording, amber during processing. This glow is a state signal, not a decorative elevation.

A subtle radial gradient (indigo at 6% opacity) washes the top of the app background, adding atmospheric depth without introducing a shadow layer.

### Named Rules
**The No Shadow Rule.** No element uses box-shadow for structural elevation. If something needs to feel "above," it uses a brighter surface tone. The only shadows in the system are state-driven glows on the hero element.

## 5. Components

### Tabs
- **Shape:** Gently rounded (6px radius), no borders
- **Active:** Slate Bed fill, Text Primary color, 12px label with icon
- **Inactive:** Transparent, Muted Steel text, brightens to Subtle Fog on hover
- **Icons:** 12px stroke icons inline with label (mic, download arrow, sparkle), inheriting text color

### Radio Cards
- **Shape:** Rounded (12px radius), 1px border
- **Selected:** Border at indigo 50% opacity, background at indigo 5% opacity, native radio input with indigo accent
- **Unselected:** Border Dim border, transparent background, brightens border on hover
- **Content:** 14px label + 12px description below, with 12px gap

### API Key Field
- **Display mode:** Green/grey status dot, label, masked value in mono, "Replace key" or "Add key" button
- **Edit mode:** Full-width mono input with indigo border, Save button (indigo fill), Cancel (ghost), Remove (ghost, right-aligned, red on hover)
- **Save disabled** when input is empty

### Dropdown (Custom)
- **Trigger:** Dark Well background, Border Dim border, text left-aligned, chevron right-aligned with rotation on open
- **Menu:** Dark Well background, Border Dim border, shadow (black/40%), max-height 160px with custom scrollbar
- **Selected option:** Indigo text, indigo 10% background tint
- **Unselected option:** Subtle text, Slate Bed background on hover

### Status Badge
- **Shape:** Pill (full radius), 12px text
- **Idle:** Slate-800 fill, Subtle Fog text, "Ready"
- **Recording:** Red-950 fill, Recording Red text, pulsing red dot (1.5px), "Recording"
- **Processing:** Amber-950 fill, Processing Amber text, "Transcribing"

### Dictate Hero
- **Layout:** Centered column, 56px mic circle, title, hint text
- **Mic circle:** Rounded full, tinted background (indigo/red/amber by state), 1px border at matching color
- **Glow:** box-shadow that intensifies during recording (32px spread, 15% red opacity)
- **Pulse ring:** CSS animation on recording state, 1.4s ease-out infinite, scales 0.9 to 1.3 with opacity fade

### Model Cards
- **Shape:** Rounded (12px radius), 1px border
- **Active model:** Indigo border + tint (same treatment as radio cards), 1.5px indigo dot indicator
- **Actions:** "Use" button (Slate Bed fill) for installed non-active models, "Delete" (ghost, red on hover), "Download" (Slate Bed fill)
- **Progress:** 4px rounded bar, Slate Bed track, indigo fill with width transition

### Compact Settings Panel
- **Container:** Dark Well at 50% opacity, 1px Border Dim at 50% opacity, 12px radius, 12px padding
- **Layout:** InlineField rows (label left, control right) with 12px vertical gap
- **Controls:** 192px wide inputs/dropdowns, 12px type, mono for hotkey input

### Range Slider (Custom)
- **Track:** 4px height, Slate Bed color, 2px radius
- **Thumb:** 14px circle, Indigo Accent fill, 2px Dark Well border, brightens on hover
- **Focus:** 3px indigo ring at 20% opacity

## 6. Do's and Don'ts

### Do:
- **Do** use indigo exclusively for interactive affordance (buttons, selected states, focus rings, progress bars). Never as decoration.
- **Do** communicate recording state through multiple channels: color, text label, animation, and glow. Never through color alone.
- **Do** fall back gracefully when AI post-processing fails. Always paste raw text. Never block the pipeline.
- **Do** use tonal surface stepping for depth. Four tones is enough; adding more signals a layout problem.
- **Do** keep all body text at 12-14px. If something needs emphasis, change weight or color, not size.
- **Do** use the system font stack. The app should feel native to macOS, not imported from the web.

### Don't:
- **Don't** build chatbot-style conversational interfaces. iSpeak is a channel, not a conversation partner.
- **Don't** use modals for settings or confirmations. Everything happens inline in the tab panel.
- **Don't** add box shadows for elevation. Use surface tone steps or borders.
- **Don't** pull user focus with toasts, banners, or celebration animations. State changes are shown in the StatusBadge and DictateHero, nowhere else.
- **Don't** use the Teams recording UI as a reference. No floating toolbars, no participant grids, no call controls aesthetic.
- **Don't** use gradient text, glassmorphism, or side-stripe accent borders. These are structurally banned.
- **Don't** add features that require the iSpeak window to be foregrounded during use. The user's active app stays in front.
