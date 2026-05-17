import { useCallback, useEffect, useState } from 'react'
import {
  meetingListHistory,
  meetingDeleteHistory,
  meetingExport,
  meetingSetTitle,
  onMeetingDone,
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
  const [editingId, setEditingId] = useState<string | null>(null)
  const [draftTitle, setDraftTitle] = useState('')

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

  // Refresh History when a new meeting completes so just-recorded transcripts
  // appear without requiring a tab switch or page reload.
  useEffect(() => {
    let unlisten: (() => void) | null = null
    let cancelled = false
    onMeetingDone(() => {
      void refresh(query)
    }).then((fn) => {
      if (cancelled) fn()
      else unlisten = fn
    })
    return () => {
      cancelled = true
      if (unlisten) unlisten()
    }
  }, [refresh, query])

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

  const startRename = (t: MeetingTranscript) => {
    setEditingId(t.id)
    setDraftTitle(t.title ?? '')
  }

  const commitRename = async () => {
    if (!editingId) return
    const id = editingId
    const next = draftTitle.trim() || null
    setEditingId(null)
    try {
      await meetingSetTitle(id, next)
      setItems((prev) =>
        prev.map((t) => (t.id === id ? { ...t, title: next } : t)),
      )
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }

  const onSaveMd = async (id: string) => {
    const t = items.find((x) => x.id === id)
    const slug = (t?.title ?? sourceLabel(t!))
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
      .slice(0, 60)
    const defaultPath = `${slug || 'transcript'}.md`
    const path = await save({
      defaultPath,
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
                <div className="flex-1 min-w-0">
                  {editingId === t.id ? (
                    <input
                      autoFocus
                      type="text"
                      value={draftTitle}
                      onChange={(e) => setDraftTitle(e.target.value)}
                      onBlur={commitRename}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') commitRename()
                        if (e.key === 'Escape') setEditingId(null)
                      }}
                      placeholder={sourceLabel(t)}
                      className="w-full bg-slate-950/60 border border-slate-700 rounded px-2 py-1 text-sm text-slate-100 focus:outline-none focus:border-indigo-500"
                    />
                  ) : (
                    <button
                      onClick={() => setExpandedId(isOpen ? null : t.id)}
                      onDoubleClick={(e) => {
                        e.stopPropagation()
                        startRename(t)
                      }}
                      className="block w-full text-left text-sm text-slate-200 hover:text-slate-50"
                      title="Double-click to rename"
                    >
                      <div className="font-medium truncate">
                        {t.title || sourceLabel(t)}
                      </div>
                      <div className="text-xs text-slate-500 mt-0.5 flex gap-3">
                        <span>{formatDate(t.created_at)}</span>
                        <span>{formatDuration(t.duration_secs)}</span>
                        {t.partial && <span className="text-amber-400">⚠ Partial</span>}
                      </div>
                    </button>
                  )}
                </div>
                {editingId !== t.id && (
                  <button
                    onClick={() => startRename(t)}
                    className="text-xs text-slate-500 hover:text-slate-300 px-2 py-1"
                    title="Rename"
                  >
                    ✎
                  </button>
                )}
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
