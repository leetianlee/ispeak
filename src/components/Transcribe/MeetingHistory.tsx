import { useCallback, useEffect, useState } from 'react'
import {
  meetingListHistory,
  meetingDeleteHistory,
  meetingExport,
  MeetingTranscript,
} from '../../lib/contract'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { save } from '@tauri-apps/plugin-dialog'
import { writeTextFile } from '@tauri-apps/plugin-fs'

function formatDate(unixMillis: number): string {
  try {
    return new Date(unixMillis).toLocaleString()
  } catch {
    return ''
  }
}

function formatDuration(secs: number): string {
  const m = Math.floor(secs / 60)
  const s = Math.round(secs - m * 60)
  return `${m}m ${s}s`
}

function sourceLabel(t: MeetingTranscript): string {
  if (t.source.kind === 'file_import') {
    const path = t.source.value
    return path.split('/').pop() || path
  }
  return 'Live capture'
}

export function MeetingHistory() {
  const [items, setItems] = useState<MeetingTranscript[]>([])
  const [query, setQuery] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [expandedId, setExpandedId] = useState<string | null>(null)

  const refresh = useCallback(async (q: string) => {
    setLoading(true)
    setError(null)
    try {
      const rows = await meetingListHistory({ query: q.trim() || null, limit: 50 })
      setItems(rows)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refresh('')
  }, [refresh])

  useEffect(() => {
    const id = setTimeout(() => void refresh(query), 250)
    return () => clearTimeout(id)
  }, [query, refresh])

  const onDelete = async (id: string) => {
    try {
      await meetingDeleteHistory(id)
      setItems((prev) => prev.filter((t) => t.id !== id))
      if (expandedId === id) setExpandedId(null)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }

  const onCopyMd = async (id: string) => {
    const md = await meetingExport(id, 'markdown')
    await writeText(md)
  }

  const onSaveMd = async (id: string) => {
    const path = await save({
      defaultPath: 'transcript.md',
      filters: [{ name: 'Markdown', extensions: ['md'] }],
    })
    if (!path) return
    const md = await meetingExport(id, 'markdown')
    await writeTextFile(path, md)
  }

  return (
    <div className="mt-10">
      <div className="flex items-baseline gap-3 mb-3">
        <h3 className="text-sm uppercase tracking-wider text-slate-500">History</h3>
        <div className="flex-1" />
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search transcripts…"
          className="text-sm bg-slate-900/60 border border-slate-800 rounded px-3 py-1 text-slate-200 placeholder-slate-600 focus:outline-none focus:border-slate-600 w-64"
        />
      </div>

      {error && <div className="text-xs text-red-400 mb-2">{error}</div>}
      {!error && loading && items.length === 0 && (
        <div className="text-xs text-slate-500">Loading…</div>
      )}
      {!loading && items.length === 0 && (
        <div className="text-xs text-slate-500">
          {query ? `No matches for "${query}".` : 'No past meetings yet.'}
        </div>
      )}

      <div className="space-y-2">
        {items.map((t) => {
          const isOpen = expandedId === t.id
          return (
            <div
              key={t.id}
              className="rounded-md bg-slate-900/30 border border-slate-800/60"
            >
              <div className="flex items-center gap-3 p-3">
                <button
                  onClick={() => setExpandedId(isOpen ? null : t.id)}
                  className="flex-1 text-left text-sm text-slate-200 hover:text-slate-50"
                >
                  <div className="font-medium truncate">{sourceLabel(t)}</div>
                  <div className="text-xs text-slate-500 mt-0.5 flex gap-3">
                    <span>{formatDate(t.created_at)}</span>
                    <span>{formatDuration(t.duration_secs)}</span>
                    {t.partial && <span className="text-amber-400">⚠ Partial</span>}
                  </div>
                </button>
                <button
                  onClick={() => onCopyMd(t.id)}
                  className="text-xs text-slate-400 hover:text-slate-200 px-2 py-1"
                >
                  Copy
                </button>
                <button
                  onClick={() => onSaveMd(t.id)}
                  className="text-xs text-slate-400 hover:text-slate-200 px-2 py-1"
                >
                  Save
                </button>
                <button
                  onClick={() => onDelete(t.id)}
                  className="text-xs text-slate-500 hover:text-red-400 px-2 py-1"
                  title="Delete from history"
                >
                  ✕
                </button>
              </div>
              {isOpen && (
                <div className="p-3 border-t border-slate-800/60">
                  {t.summary && (
                    <div className="mb-3">
                      <div className="text-xs uppercase tracking-wider text-slate-500 mb-1">
                        Summary
                      </div>
                      <div className="text-sm text-slate-200">{t.summary}</div>
                    </div>
                  )}
                  {t.action_items.length > 0 && (
                    <div className="mb-3">
                      <div className="text-xs uppercase tracking-wider text-slate-500 mb-1">
                        Action items
                      </div>
                      <ul className="text-sm text-slate-200 list-disc list-inside space-y-0.5">
                        {t.action_items.map((item, i) => (
                          <li key={i}>{item}</li>
                        ))}
                      </ul>
                    </div>
                  )}
                  <div className="text-xs text-slate-500 mb-1 uppercase tracking-wider">
                    Transcript
                  </div>
                  <div className="text-sm text-slate-300 space-y-1 max-h-64 overflow-auto">
                    {t.segments.map((seg, i) => (
                      <div key={i}>
                        <span className="text-slate-500">
                          {seg.speaker.kind === 'you'
                            ? 'You'
                            : seg.speaker.kind === 'indexed'
                            ? `Speaker ${String.fromCharCode(65 + seg.speaker.value)}`
                            : 'Speaker'}
                          :
                        </span>{' '}
                        {seg.text}
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}
