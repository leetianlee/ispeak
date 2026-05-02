import { downloadModel, deleteModel, updateSettings } from "../lib/contract";
import { useAppStore } from "../store/useAppStore";

const MODELS = [
  { id: "tiny",   label: "Tiny",   size: "75 MB",  speed: "~100ms",  quality: "Basic" },
  { id: "base",   label: "Base",   size: "142 MB", speed: "~200ms",  quality: "Moderate" },
  { id: "small",  label: "Small",  size: "466 MB", speed: "~400ms",  quality: "Good" },
  { id: "medium", label: "Medium", size: "1.5 GB", speed: "~900ms",  quality: "Very good" },
  { id: "large",  label: "Large",  size: "2.9 GB", speed: "~2000ms", quality: "Best" },
] as const;

export function ModelDownload() {
  const { installedModels, downloadProgress, settings } = useAppStore();

  const isInstalled = (id: string) =>
    installedModels.includes(id as any);

  const isActive = (id: string) =>
    settings?.whisper_model === id;

  const progress = (id: string) =>
    downloadProgress[id] ?? null;

  const handleDownload = async (id: string) => {
    try {
      await downloadModel(id as any);
    } catch (e) {
      console.error("Download failed", e);
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm(`Delete the ${id} model? You'll need to re-download it.`)) return;
    try {
      await deleteModel(id as any);
    } catch (e) {
      console.error("Delete failed", e);
    }
  };

  const handleSelect = async (id: string) => {
    try {
      await updateSettings({ whisper_model: id } as any);
      // Refresh settings to reflect the change
      const { getSettings } = await import("../lib/contract");
      const fresh = await getSettings();
      useAppStore.getState().setSettings(fresh);
    } catch (e) {
      console.error("Select failed", e);
    }
  };

  const noneInstalled = installedModels.length === 0;

  return (
    <div className="space-y-2">
      {noneInstalled ? (
        <div className="bg-indigo-500/5 border border-indigo-500/20 rounded-lg p-3 mb-3">
          <p className="text-xs text-indigo-300 font-medium mb-0.5">No models installed</p>
          <p className="text-xs text-slate-500">Download a model to use local transcription. Medium is a good starting point.</p>
        </div>
      ) : (
        <p className="text-xs text-slate-500 mb-3">
          Models are stored locally on your Mac. The medium model is recommended for a balance of speed and accuracy.
        </p>
      )}
      {MODELS.map((m) => {
        const installed = isInstalled(m.id);
        const active = isActive(m.id);
        const pct = progress(m.id);
        const downloading = pct !== null;

        return (
          <div
            key={m.id}
            className={`p-3 rounded-lg border transition-colors ${
              active
                ? "border-indigo-500/50 bg-indigo-500/5"
                : "border-[#2a3347]"
            }`}
          >
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                {active && (
                  <span className="w-1.5 h-1.5 rounded-full bg-indigo-400 flex-shrink-0" />
                )}
                <div>
                  <p className="text-sm font-medium text-slate-200">
                    {m.label}
                    {m.id === "medium" && (
                      <span className="ml-2 text-xs text-indigo-400 font-normal">recommended</span>
                    )}
                  </p>
                  <p className="text-xs text-slate-500">
                    {m.size} · {m.speed} · {m.quality}
                  </p>
                </div>
              </div>

              <div className="flex items-center gap-2">
                {downloading ? (
                  <span className="text-xs text-amber-400">{pct}%</span>
                ) : installed ? (
                  <>
                    {!active && (
                      <button
                        onClick={() => handleSelect(m.id)}
                        className="text-xs bg-[#1e2535] hover:bg-[#2a3347] text-slate-300 px-3 py-1.5 rounded-md transition-colors"
                      >
                        Use
                      </button>
                    )}
                    <button
                      onClick={() => handleDelete(m.id)}
                      className="text-xs text-slate-600 hover:text-red-400 transition-colors px-2 py-1"
                    >
                      Delete
                    </button>
                  </>
                ) : (
                  <button
                    onClick={() => handleDownload(m.id)}
                    className="text-xs bg-[#1e2535] hover:bg-[#2a3347] text-slate-300 px-3 py-1.5 rounded-md transition-colors"
                  >
                    Download
                  </button>
                )}
              </div>
            </div>

            {downloading && (
              <div className="mt-2 h-1 bg-[#1e2535] rounded-full overflow-hidden">
                <div
                  className="h-full bg-indigo-500 transition-all duration-300"
                  style={{ width: `${pct}%` }}
                />
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
