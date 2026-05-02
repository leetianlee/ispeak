import { useEffect } from "react";
import { SettingsPanel } from "./components/SettingsPanel";
import {
  getSettings,
  listMicrophones,
  getInstalledModels,
  onRecordingStateChanged,
  onTranscriptReady,
  onModelDownloadProgress,
  onError,
} from "./lib/contract";
import { useAppStore } from "./store/useAppStore";

export default function App() {
  const { setRecordingState, setSettings, setMicrophones, setInstalledModels,
          setDownloadProgress, clearDownloadProgress, setLastTranscript } = useAppStore();

  // Bootstrap: load settings and devices on mount
  useEffect(() => {
    getSettings().then(setSettings).catch(console.error);
    listMicrophones().then(setMicrophones).catch(console.error);
    getInstalledModels().then((models) =>
      setInstalledModels(models as any)
    ).catch(console.error);
  }, []);

  // Subscribe to Tauri events
  useEffect(() => {
    const unlisten: Array<() => void> = [];

    onRecordingStateChanged((state) => setRecordingState(state))
      .then((u) => unlisten.push(u));

    onTranscriptReady((result) => setLastTranscript(result.text))
      .then((u) => unlisten.push(u));

    onModelDownloadProgress((progress) => {
      if (progress.complete) {
        clearDownloadProgress(progress.model);
        getInstalledModels().then((m) => setInstalledModels(m as any)).catch(console.error);
      } else {
        setDownloadProgress(progress.model, progress.percent);
      }
    }).then((u) => unlisten.push(u));

    onError((err) => {
      console.error("[iSpeak]", err.code, err.message);
    }).then((u) => unlisten.push(u));

    return () => unlisten.forEach((u) => u());
  }, []);

  return (
    <div className="h-full bg-[#09090e] text-slate-200 select-none overflow-hidden app-bg">
      <SettingsPanel />
    </div>
  );
}
