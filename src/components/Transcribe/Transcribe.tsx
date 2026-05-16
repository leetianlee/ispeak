import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { DropZone, ACCEPTED_EXTS } from "./DropZone";
import { meetingEnqueueFile } from "../../lib/contract";

export function Transcribe() {
  const [dropError, setDropError] = useState<string | null>(null);

  useEffect(() => {
    let unlistenDrop: (() => void) | undefined;
    listen<{ paths: string[] }>("tauri://drag-drop", async (e) => {
      const path = e.payload?.paths?.[0];
      if (!path) return;
      const lower = path.toLowerCase();
      const ext = "." + (lower.split(".").pop() ?? "");
      if (!ACCEPTED_EXTS.includes(ext)) {
        setDropError(`Unsupported format: ${ext}`);
        return;
      }
      setDropError(null);
      try {
        await meetingEnqueueFile(path);
      } catch (err) {
        setDropError(String(err));
      }
    }).then((u) => (unlistenDrop = u));
    return () => unlistenDrop?.();
  }, []);

  return (
    <div className="p-6">
      <h2 className="text-lg font-medium mb-4 text-slate-200">Transcribe</h2>
      <DropZone errorMessage={dropError} />
    </div>
  );
}
