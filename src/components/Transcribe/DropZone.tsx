import { useState } from "react";

export const ACCEPTED_EXTS = [".mp3", ".m4a", ".mp4", ".wav", ".ogg", ".flac"];

interface DropZoneProps {
  errorMessage: string | null;
}

export function DropZone({ errorMessage }: DropZoneProps) {
  const [hover, setHover] = useState(false);
  return (
    <div
      onDragOver={(e) => { e.preventDefault(); setHover(true); }}
      onDragLeave={() => setHover(false)}
      onDrop={() => setHover(false)}
      className={`rounded-lg border-2 border-dashed p-8 text-center transition-colors ${
        hover ? "border-indigo-400 bg-indigo-500/5" : "border-slate-700 bg-slate-900/30"
      }`}
    >
      <div className="text-slate-300">📥 Drop audio or video file here</div>
      <div className="text-xs text-slate-500 mt-1">{ACCEPTED_EXTS.join(" · ")}</div>
      {errorMessage && <div className="text-sm text-rose-400 mt-3">{errorMessage}</div>}
    </div>
  );
}
