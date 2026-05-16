import { create } from "zustand";
import type { AppSettings, MicrophoneDevice, RecordingState, WhisperModel } from "../lib/contract";

interface AppStore {
  // Recording state
  recordingState: RecordingState;
  setRecordingState: (state: RecordingState) => void;

  // Settings
  settings: AppSettings | null;
  setSettings: (settings: AppSettings) => void;
  updateSettingsField: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void;

  // Microphones
  microphones: MicrophoneDevice[];
  setMicrophones: (mics: MicrophoneDevice[]) => void;

  // Installed models
  installedModels: WhisperModel[];
  setInstalledModels: (models: WhisperModel[]) => void;

  // Download progress
  downloadProgress: Record<string, number>;
  setDownloadProgress: (model: string, percent: number) => void;
  clearDownloadProgress: (model: string) => void;

  // Last transcript
  lastTranscript: string | null;
  setLastTranscript: (text: string) => void;

  // Active view in settings
  activeTab: "dictate" | "transcribe" | "models" | "ai" | "about";
  setActiveTab: (tab: "dictate" | "transcribe" | "models" | "ai" | "about") => void;
}

export const useAppStore = create<AppStore>((set) => ({
  recordingState: "idle",
  setRecordingState: (state) => set({ recordingState: state }),

  settings: null,
  setSettings: (settings) => set({ settings }),
  updateSettingsField: (key, value) =>
    set((s) => ({
      settings: s.settings ? { ...s.settings, [key]: value } : null,
    })),

  microphones: [],
  setMicrophones: (microphones) => set({ microphones }),

  installedModels: [],
  setInstalledModels: (installedModels) => set({ installedModels }),

  downloadProgress: {},
  setDownloadProgress: (model, percent) =>
    set((s) => ({ downloadProgress: { ...s.downloadProgress, [model]: percent } })),
  clearDownloadProgress: (model) =>
    set((s) => {
      const next = { ...s.downloadProgress };
      delete next[model];
      return { downloadProgress: next };
    }),

  lastTranscript: null,
  setLastTranscript: (text) => set({ lastTranscript: text }),

  activeTab: "dictate",
  setActiveTab: (activeTab) => set({ activeTab }),
}));
