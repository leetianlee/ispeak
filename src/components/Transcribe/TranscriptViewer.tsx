import { useMeetingStore } from '../../store/useMeetingStore'
import { meetingExport, MeetingTranscript } from '../../lib/contract'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { save } from '@tauri-apps/plugin-dialog'
import { writeTextFile } from '@tauri-apps/plugin-fs'

function speakerLabel(s: MeetingTranscript['segments'][number]['speaker']): string {
  switch (s.kind) {
    case 'you': return 'You'
    case 'other': return 'Speaker'
    case 'indexed': return `Speaker ${String.fromCharCode(65 + s.value)}`
  }
}

export function TranscriptViewer() {
  const transcripts = useMeetingStore((s) => s.transcripts)
  if (transcripts.length === 0) return null

  const copyMd = async (id: string) => {
    const md = await meetingExport(id, 'markdown')
    await writeText(md)
  }

  const saveMd = async (id: string) => {
    const path = await save({
      defaultPath: 'transcript.md',
      filters: [{ name: 'Markdown', extensions: ['md'] }],
    })
    if (!path) return
    const md = await meetingExport(id, 'markdown')
    await writeTextFile(path, md)
  }

  return (
    <div className="mt-8">
      <h3 className="text-sm uppercase tracking-wider text-slate-500 mb-2">Results</h3>
      {transcripts.map((t) => (
        <div key={t.id} className="mb-6 p-4 rounded-lg bg-slate-900/40 border border-slate-800">
          <div className="flex items-center gap-3 mb-3">
            <div className="text-sm text-slate-300">{Math.round(t.duration_secs)}s</div>
            {t.partial && <span className="text-xs text-amber-400">Partial</span>}
            <div className="flex-1" />
            <button
              onClick={() => copyMd(t.id)}
              className="text-xs text-slate-400 hover:text-slate-200 px-2 py-1"
            >
              Copy MD
            </button>
            <button
              onClick={() => saveMd(t.id)}
              className="text-xs text-slate-400 hover:text-slate-200 px-2 py-1"
            >
              Save .md
            </button>
          </div>
          <div className="space-y-2 text-sm">
            {t.segments.map((seg, i) => (
              <div key={i} className="flex gap-3">
                <div className="text-slate-500 w-20 text-xs pt-0.5">{speakerLabel(seg.speaker)}</div>
                <div className="text-slate-200">{seg.text}</div>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}
