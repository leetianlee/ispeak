import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { SettingsPanel } from "./components/SettingsPanel";
import {
  getSettings,
  listMicrophones,
  getInstalledModels,
  onRecordingStateChanged,
  onTranscriptReady,
  onModelDownloadProgress,
  onError,
  meetingEnqueueFile,
  onMeetingProgress,
  onMeetingDone,
  onMeetingError,
} from "./lib/contract";
import { useAppStore } from "./store/useAppStore";
import { useMeetingStore } from "./store/useMeetingStore";
import { ACCEPTED_EXTS } from "./components/Transcribe/DropZone";

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

  // Meeting event listeners — hoisted here so they survive tab switches
  useEffect(() => {
    let cancelled = false
    const handles: Array<() => void> = []

    const register = async () => {
      handles.push(await listen<{ paths: string[] }>('tauri://drag-drop', async (e) => {
        const path = e.payload?.paths?.[0]
        if (!path) return
        const lower = path.toLowerCase()
        const ext = '.' + (lower.split('.').pop() ?? '')
        if (!ACCEPTED_EXTS.includes(ext)) {
          useMeetingStore.getState().setLastError(`Unsupported format: ${ext}`)
          return
        }
        useMeetingStore.getState().setLastError(null)
        try { await meetingEnqueueFile(path) }
        catch (err) { useMeetingStore.getState().setLastError(String(err)) }
      }))

      handles.push(await onMeetingProgress((p) => {
        useMeetingStore.getState().upsertProgress(p)
      }))
      handles.push(await onMeetingDone((e) => {
        useMeetingStore.getState().removeJob(e.job_id)
        useMeetingStore.getState().addTranscript(e.transcript)
      }))
      handles.push(await onMeetingError((e) => {
        useMeetingStore.getState().removeJob(e.job_id)
        useMeetingStore.getState().setLastError(e.reason)
      }))

      if (cancelled) handles.forEach((u) => u())
    }
    register()

    return () => {
      cancelled = true
      handles.forEach((u) => u())
    }
  }, []);

  return (
    <div className="h-full bg-[#09090e] text-slate-200 select-none overflow-hidden app-bg">
      <SettingsPanel />
    </div>
  );
}
