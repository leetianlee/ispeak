import { useState, useRef, useEffect } from "react";
import { updateSettings } from "../lib/contract";
import { useAppStore } from "../store/useAppStore";
import { ModelDownload } from "./ModelDownload";
import { Transcribe } from "./Transcribe";

const TAB_ICONS: Record<string, React.ReactNode> = {
  dictate: (
    <svg width={12} height={12} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
      <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
      <line x1="12" x2="12" y1="19" y2="22" />
    </svg>
  ),
  models: (
    <svg width={12} height={12} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
      <polyline points="7 10 12 15 17 10" />
      <line x1="12" x2="12" y1="15" y2="3" />
    </svg>
  ),
  ai: (
    <svg width={12} height={12} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 3l1.5 4.5L18 9l-4.5 1.5L12 15l-1.5-4.5L6 9l4.5-1.5Z" />
      <path d="M18 14l.7 2.1L21 17l-2.3.9L18 20l-.7-2.1L15 17l2.3-.9Z" />
    </svg>
  ),
  transcribe: (
    <svg width={12} height={12} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z" />
      <polyline points="14 2 14 8 20 8" />
      <line x1="16" x2="8" y1="13" y2="13" />
      <line x1="16" x2="8" y1="17" y2="17" />
      <line x1="10" x2="8" y1="9" y2="9" />
    </svg>
  ),
};

const TABS = [
  { id: "dictate",    label: "Dictate" },
  { id: "transcribe", label: "Transcribe" },
  { id: "models",     label: "Models" },
  { id: "ai",         label: "AI" },
] as const;

export function SettingsPanel() {
  const { settings, setSettings, microphones, activeTab, setActiveTab, recordingState, lastTranscript } = useAppStore();
  const [saving, setSaving] = useState(false);

  const save = async (patch: Record<string, unknown>) => {
    setSaving(true);
    try {
      await updateSettings(patch as any);
      // Reload settings to get masked keys
      const { getSettings } = await import("../lib/contract");
      const fresh = await getSettings();
      setSettings(fresh);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex flex-col h-full">
      {/* Title bar — draggable */}
      <div className="drag-region flex items-center justify-between px-4 py-3 titlebar-border">
        <div className="flex items-center gap-2">
          <AppIcon size={16} />
          <span className="text-sm font-semibold text-slate-100">iSpeak</span>
        </div>
        <StatusBadge state={recordingState} />
      </div>

      {/* Tabs */}
      <div className="no-drag flex gap-1 px-3 pt-3">
        {TABS.map((t) => (
          <button
            key={t.id}
            onClick={() => setActiveTab(t.id)}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
              activeTab === t.id
                ? "bg-[#1e2535] text-slate-100"
                : "text-slate-500 hover:text-slate-300"
            }`}
          >
            {TAB_ICONS[t.id]}
            {t.label}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="no-drag flex-1 overflow-y-auto px-4 py-4 space-y-5">
        {activeTab === "dictate" && settings && (
          <>
            {/* Hero status */}
            <DictateHero state={recordingState} hotkey={settings.hotkey} mode={settings.recording_mode} />

            {/* Last transcript or empty state */}
            {lastTranscript ? (
              <TranscriptCard text={lastTranscript} />
            ) : (
              <div className="text-center py-2">
                <p className="text-xs text-slate-600">No transcripts yet. Try your hotkey to get started.</p>
              </div>
            )}

            {/* Settings divider */}
            <div className="flex items-center gap-3 pt-1">
              <div className="h-px flex-1 bg-[#1e2535]" />
              <span className="text-[10px] uppercase tracking-widest text-slate-600 font-medium">Settings</span>
              <div className="h-px flex-1 bg-[#1e2535]" />
            </div>

            {/* Primary: Transcription Engine */}
            <Section title="Transcription Engine">
              <RadioGroup
                value={settings.transcription_engine}
                options={[
                  { value: "local", label: "Local (Whisper)", desc: "Runs on your Mac, private, no cost" },
                  { value: "groq", label: "Groq Cloud", desc: "Faster, requires API key" },
                ]}
                onChange={(v) => save({ transcription_engine: v })}
              />
              {settings.transcription_engine === "groq" && (
                <div className="mt-3">
                  <ApiKeyField
                    label="Groq API Key"
                    value={settings.groq_api_key}
                    placeholder="gsk_..."
                    onSave={(v) => save({ groq_api_key: v })}
                  />
                </div>
              )}
            </Section>

            {/* Standard: Recording Mode */}
            <Section title="Recording Mode">
              <RadioGroup
                value={settings.recording_mode}
                options={[
                  { value: "push_to_talk", label: "Push to talk", desc: "Hold hotkey to record, release to transcribe" },
                  { value: "toggle", label: "Toggle", desc: "Press once to start, press again to stop" },
                ]}
                onChange={(v) => save({ recording_mode: v })}
              />
            </Section>

            {/* Compact: Hotkey, Microphone, Duration */}
            <div className="space-y-3 bg-[#0f1117]/50 rounded-lg p-3 border border-[#1e2535]/50">
              <InlineField label="Hotkey">
                <input
                  className="w-48 bg-[#0f1117] border border-[#2a3347] rounded-md px-2.5 py-1.5 text-xs text-slate-200 focus:outline-none focus:border-indigo-500 font-mono"
                  value={settings.hotkey}
                  onChange={(e) => save({ hotkey: e.target.value })}
                  placeholder="CommandOrControl+Shift+Space"
                />
              </InlineField>

              <InlineField label="Microphone">
                <Dropdown
                  value={settings.microphone_id ?? ""}
                  options={[
                    { value: "", label: "System default" },
                    ...microphones.map((m) => ({
                      value: m.id,
                      label: `${m.name}${m.is_default ? " (default)" : ""}`,
                    })),
                  ]}
                  onChange={(v) => save({ microphone_id: v || null })}
                />
              </InlineField>

              <InlineField label="Max duration">
                <div className="flex items-center gap-2 w-48">
                  <input
                    type="range"
                    min={5}
                    max={300}
                    step={5}
                    value={settings.max_recording_duration_s}
                    onChange={(e) => save({ max_recording_duration_s: Number(e.target.value) })}
                    className="flex-1 custom-range"
                  />
                  <span className="text-xs text-slate-500 w-10 text-right tabular-nums">
                    {settings.max_recording_duration_s}s
                  </span>
                </div>
              </InlineField>
            </div>
          </>
        )}

        {activeTab === "transcribe" && <Transcribe />}

        {activeTab === "models" && <ModelDownload />}

        {activeTab === "ai" && settings && (
          <>
            <Section title="AI Post-Processing">
              <p className="text-xs text-slate-500 mb-3">
                Applies grammar correction and formatting after transcription.
              </p>
              <RadioGroup
                value={settings.ai_mode}
                options={[
                  { value: "off", label: "Off", desc: "Raw transcription, zero latency" },
                  { value: "local", label: "Local (Ollama)", desc: "Free, ~300ms extra, requires Ollama running" },
                  { value: "cloud_fast", label: "Cloud Fast", desc: "Groq Llama 70B, fast" },
                  { value: "cloud_quality", label: "Cloud Quality", desc: "Groq Llama 70B, quality" },
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
                  label="Groq API Key"
                  value={settings.groq_api_key}
                  placeholder="gsk_..."
                  onSave={(v) => save({ groq_api_key: v })}
                />
              </Section>
            )}

            <Section title="Speaker Diarisation">
              <p className="text-xs text-slate-500 mb-3">
                Heuristic clustering by voice features. Applied to meetings only (not dictation).
              </p>
              <InlineField label="Auto-detect speakers">
                <label className="inline-flex items-center cursor-pointer">
                  <input
                    type="checkbox"
                    className="sr-only peer"
                    checked={settings.auto_diarise}
                    onChange={(e) => save({ auto_diarise: e.target.checked })}
                  />
                  <div className="w-9 h-5 bg-[#1e2535] rounded-full peer peer-checked:bg-indigo-600 transition-colors relative">
                    <div className="absolute left-0.5 top-0.5 w-4 h-4 bg-slate-200 rounded-full transition-transform peer-checked:translate-x-4" />
                  </div>
                </label>
              </InlineField>
              {settings.auto_diarise && (
                <InlineField label="Expected speakers">
                  <input
                    type="number"
                    min={1}
                    max={8}
                    className="w-16 bg-[#0f1117] border border-[#2a3347] rounded-md px-2.5 py-1.5 text-xs text-slate-200 focus:outline-none focus:border-indigo-500"
                    value={settings.diarise_expected_speakers}
                    onChange={(e) => {
                      const n = Math.max(1, Math.min(8, parseInt(e.target.value) || 2))
                      save({ diarise_expected_speakers: n })
                    }}
                  />
                </InlineField>
              )}
            </Section>
          </>
        )}

      </div>

      {/* Footer */}
      <div className="px-4 py-2 border-t border-[#1e2535] flex items-center justify-between">
        <p className="text-[10px] text-slate-600">
          iSpeak v0.1.0 {saving && <span className="text-slate-500 ml-2">Saving…</span>}
        </p>
        <p className="text-[10px] text-slate-700">MIT licence</p>
      </div>
    </div>
  );
}

// ─── Sub-components ───────────────────────────────────────────────────────────

function hotkeyDisplay(raw: string): string {
  return raw
    .replace(/CommandOrControl/gi, "\u2318")
    .replace(/Shift/gi, "\u21E7")
    .replace(/Alt|Option/gi, "\u2325")
    .replace(/Control/gi, "\u2303")
    .replace(/Space/gi, "Space")
    .replace(/\+/g, " ");
}

function DictateHero({ state, hotkey, mode }: { state: string; hotkey: string; mode: string }) {
  const isRecording = state === "recording";
  const isProcessing = state === "processing";

  const ringColor = isRecording ? "bg-red-500/10" : isProcessing ? "bg-amber-500/10" : "bg-indigo-500/10";
  const ringBorder = isRecording ? "border-red-500/30" : isProcessing ? "border-amber-500/30" : "border-indigo-500/20";
  const iconColor = isRecording ? "text-red-400" : isProcessing ? "text-amber-400" : "text-indigo-400";

  const hint = isRecording
    ? mode === "push_to_talk" ? "Release to transcribe" : "Press hotkey to stop"
    : isProcessing
      ? "Transcribing your audio..."
      : `Press ${hotkeyDisplay(hotkey)} to start`;

  const title = isRecording ? "Listening..." : isProcessing ? "Transcribing" : "Ready to dictate";

  const glowStyle = isRecording
    ? { boxShadow: "0 0 32px 8px rgba(239, 68, 68, 0.15), 0 0 8px 2px rgba(239, 68, 68, 0.2)" }
    : isProcessing
      ? { boxShadow: "0 0 24px 6px rgba(245, 158, 11, 0.1)" }
      : { boxShadow: "0 0 24px 6px rgba(99, 102, 241, 0.08)" };

  return (
    <div className="flex flex-col items-center py-5">
      <div
        className={`relative w-14 h-14 rounded-full ${ringColor} border ${ringBorder} flex items-center justify-center mb-3 transition-shadow duration-500`}
        style={glowStyle}
      >
        {isRecording && (
          <div className="absolute inset-0 rounded-full border border-red-500/20 pulse-ring" />
        )}
        <svg width={24} height={24} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className={iconColor}>
          <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
          <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
          <line x1="12" x2="12" y1="19" y2="22" />
        </svg>
      </div>
      <p className={`text-sm font-medium ${isRecording ? "text-red-400" : isProcessing ? "text-amber-400" : "text-slate-200"}`}>
        {title}
      </p>
      <p className="text-xs text-slate-500 mt-1">{hint}</p>
    </div>
  );
}

function TranscriptCard({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  const copy = () => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <div className="bg-[#0f1117] border border-[#1e2535] rounded-lg p-3">
      <div className="flex items-center justify-between mb-1">
        <p className="text-xs text-slate-500">Last transcript</p>
        <button
          onClick={copy}
          className="text-[10px] text-slate-600 hover:text-slate-400 transition-colors"
        >
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <p className="text-sm text-slate-300 leading-relaxed">{text}</p>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <p className="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-2">{title}</p>
      {children}
    </div>
  );
}

function Dropdown({
  value,
  options,
  onChange,
}: {
  value: string;
  options: { value: string; label: string }[];
  onChange: (v: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const selected = options.find((o) => o.value === value);

  return (
    <div ref={ref} className="relative w-48">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between bg-[#0f1117] border border-[#2a3347] rounded-md px-2.5 py-1.5 text-xs text-slate-200 hover:border-[#3a4357] transition-colors text-left"
      >
        <span className="truncate">{selected?.label ?? "Select..."}</span>
        <svg width={10} height={10} viewBox="0 0 10 10" className={`text-slate-500 flex-shrink-0 ml-1 transition-transform ${open ? "rotate-180" : ""}`}>
          <path d="M2 3.5L5 6.5L8 3.5" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>
      {open && (
        <div className="absolute z-50 mt-1 w-full bg-[#0f1117] border border-[#2a3347] rounded-md shadow-lg shadow-black/40 py-0.5 max-h-40 overflow-y-auto">
          {options.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => { onChange(opt.value); setOpen(false); }}
              className={`w-full text-left px-2.5 py-1.5 text-xs transition-colors truncate ${
                opt.value === value
                  ? "text-indigo-400 bg-indigo-500/10"
                  : "text-slate-300 hover:bg-[#1e2535]"
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function InlineField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-xs text-slate-400">{label}</span>
      {children}
    </div>
  );
}

function RadioGroup({
  value,
  options,
  onChange,
}: {
  value: string;
  options: { value: string; label: string; desc: string }[];
  onChange: (v: string) => void;
}) {
  return (
    <div className="space-y-2">
      {options.map((opt) => (
        <label
          key={opt.value}
          className={`flex items-start gap-3 p-3 rounded-lg border cursor-pointer transition-colors ${
            value === opt.value
              ? "border-indigo-500/50 bg-indigo-500/5"
              : "border-[#2a3347] hover:border-[#3a4357]"
          }`}
        >
          <input
            type="radio"
            className="mt-0.5 accent-indigo-500"
            checked={value === opt.value}
            onChange={() => onChange(opt.value)}
          />
          <div>
            <p className="text-sm font-medium text-slate-200">{opt.label}</p>
            <p className="text-xs text-slate-500">{opt.desc}</p>
          </div>
        </label>
      ))}
    </div>
  );
}

function ApiKeyField({
  label,
  value,
  placeholder,
  onSave,
}: {
  label: string;
  value: string;
  placeholder: string;
  onSave: (v: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");

  const hasKey = value && value.length > 0;
  const isMasked = value.includes("...");

  return (
    <div>
      <div className="flex items-center gap-2 mb-1">
        <span className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${hasKey ? "bg-emerald-500" : "bg-slate-600"}`} />
        <p className="text-xs text-slate-500">{label}</p>
        {hasKey && !editing && (
          <span className="text-[10px] text-slate-600 font-mono ml-auto">{isMasked ? value : "configured"}</span>
        )}
      </div>

      {editing ? (
        <div className="space-y-2">
          <input
            autoFocus
            className="w-full bg-[#0f1117] border border-indigo-500/50 rounded-md px-3 py-2 text-sm text-slate-200 focus:outline-none focus:border-indigo-500 font-mono"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder={placeholder}
            onKeyDown={(e) => {
              if (e.key === "Enter" && draft.trim()) { onSave(draft.trim()); setEditing(false); }
              if (e.key === "Escape") setEditing(false);
            }}
          />
          <div className="flex gap-2">
            <button
              className="px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-40 disabled:hover:bg-indigo-600 text-white text-xs rounded-md transition-colors"
              disabled={!draft.trim()}
              onClick={() => { onSave(draft.trim()); setEditing(false); }}
            >
              Save
            </button>
            <button
              className="px-3 py-1.5 text-slate-500 hover:text-slate-300 text-xs transition-colors"
              onClick={() => setEditing(false)}
            >
              Cancel
            </button>
            {hasKey && (
              <button
                className="px-3 py-1.5 text-slate-600 hover:text-red-400 text-xs transition-colors ml-auto"
                onClick={() => { onSave(""); setEditing(false); }}
              >
                Remove
              </button>
            )}
          </div>
        </div>
      ) : (
        <button
          className="w-full flex items-center justify-center gap-1.5 bg-[#0f1117] border border-[#2a3347] hover:border-[#3a4357] rounded-md px-3 py-2 text-xs text-slate-400 hover:text-slate-200 transition-colors"
          onClick={() => { setDraft(""); setEditing(true); }}
        >
          {hasKey ? "Replace key" : "Add key"}
        </button>
      )}
    </div>
  );
}

function StatusBadge({ state }: { state: string }) {
  const map: Record<string, { label: string; className: string }> = {
    idle:       { label: "Ready",        className: "bg-slate-800 text-slate-400" },
    recording:  { label: "Recording",    className: "bg-red-950 text-red-400" },
    processing: { label: "Transcribing", className: "bg-amber-950 text-amber-400" },
  };
  const { label, className } = map[state] ?? map.idle;
  return (
    <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${className}`}>
      {state === "recording" && <span className="inline-block w-1.5 h-1.5 rounded-full bg-red-400 mr-1 animate-pulse" />}
      {label}
    </span>
  );
}

function AppIcon({ size, className }: { size: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 512 512" className={className}>
      <defs>
        <linearGradient id="app-bg" x1="0" y1="0" x2="512" y2="512" gradientUnits="userSpaceOnUse">
          <stop offset="0%" stopColor="#312e81"/>
          <stop offset="100%" stopColor="#6366f1"/>
        </linearGradient>
      </defs>
      <rect x="16" y="16" width="480" height="480" rx="112" fill="url(#app-bg)"/>
      <rect x="218" y="88" width="76" height="116" rx="38" fill="white"/>
      <rect x="224" y="248" width="64" height="176" rx="32" fill="white"/>
    </svg>
  );
}
