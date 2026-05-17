import { useMemo, useState } from 'react'
import {
  defaultSpeakerLabel,
  meetingSetSpeakerName,
  MeetingTranscript,
  speakerKey,
} from '../../lib/contract'

interface UniqueSpeaker {
  key: string
  /** The first speaker reference, used to call meetingSetSpeakerName. */
  label: MeetingTranscript['segments'][number]['speaker']
  defaultName: string
}

function uniqueSpeakers(t: MeetingTranscript): UniqueSpeaker[] {
  const seen = new Map<string, UniqueSpeaker>()
  for (const seg of t.segments) {
    const k = speakerKey(seg.speaker)
    if (!seen.has(k)) {
      seen.set(k, {
        key: k,
        label: seg.speaker,
        defaultName: defaultSpeakerLabel(seg.speaker),
      })
    }
  }
  return Array.from(seen.values())
}

/**
 * Per-transcript panel that lists each distinct speaker label currently in
 * use and lets the user assign a custom name. Empty string clears.
 * `onChange` is called with the latest speaker_names map after each save so
 * the parent can refresh its rendered transcript.
 */
export function SpeakerNamesPanel({
  transcript,
  onChange,
}: {
  transcript: MeetingTranscript
  onChange: (names: Record<string, string>) => void
}) {
  const speakers = useMemo(() => uniqueSpeakers(transcript), [transcript])
  const [savingKey, setSavingKey] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  if (speakers.length === 0) return null

  const commit = async (sp: UniqueSpeaker, value: string) => {
    const trimmed = value.trim()
    const current = transcript.speaker_names[sp.key] ?? ''
    if (trimmed === current.trim()) return
    setSavingKey(sp.key)
    setError(null)
    try {
      const updated = await meetingSetSpeakerName(
        transcript.id,
        sp.label,
        trimmed || null,
      )
      onChange(updated)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setSavingKey(null)
    }
  }

  return (
    <div className="mb-3 p-3 rounded bg-slate-950/30 border border-slate-800/40">
      <div className="text-xs uppercase tracking-wider text-slate-500 mb-2">
        Speaker names
      </div>
      <div className="space-y-1.5">
        {speakers.map((sp) => {
          const current = transcript.speaker_names[sp.key] ?? ''
          return (
            <div key={sp.key} className="flex items-center gap-2 text-sm">
              <div className="text-slate-500 w-24 text-xs">{sp.defaultName}</div>
              <input
                type="text"
                defaultValue={current}
                placeholder={sp.defaultName}
                disabled={savingKey === sp.key}
                onBlur={(e) => commit(sp, e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') (e.target as HTMLInputElement).blur()
                  if (e.key === 'Escape') {
                    ;(e.target as HTMLInputElement).value = current
                    ;(e.target as HTMLInputElement).blur()
                  }
                }}
                className="flex-1 bg-slate-900/60 border border-slate-800 rounded px-2 py-1 text-slate-200 placeholder-slate-600 focus:outline-none focus:border-indigo-500"
              />
              {savingKey === sp.key && (
                <span className="text-xs text-slate-600">saving…</span>
              )}
            </div>
          )
        })}
      </div>
      {error && <div className="mt-2 text-xs text-red-400">{error}</div>}
      <div className="mt-2 text-xs text-slate-600">
        Enter saves. Empty clears back to the default label.
      </div>
    </div>
  )
}
