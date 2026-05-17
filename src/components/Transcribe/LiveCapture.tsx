import { useEffect, useState } from 'react'
import {
  meetingStartLive,
  meetingStopLive,
  LiveSource,
} from '../../lib/contract'

function formatElapsed(secs: number): string {
  const m = Math.floor(secs / 60)
  const s = Math.floor(secs - m * 60)
  return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
}

export function LiveCapture() {
  const [isRecording, setIsRecording] = useState(false)
  const [isStopping, setIsStopping] = useState(false)
  const [activeJobId, setActiveJobId] = useState<string | null>(null)
  const [source, setSource] = useState<LiveSource>('mic_only')
  const [elapsedSecs, setElapsedSecs] = useState(0)
  const [startedAt, setStartedAt] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!isRecording || startedAt === null) return
    const id = setInterval(() => {
      setElapsedSecs(Math.floor((Date.now() - startedAt) / 1000))
    }, 500)
    return () => clearInterval(id)
  }, [isRecording, startedAt])

  const start = async () => {
    setError(null)
    try {
      const jobId = await meetingStartLive(source)
      setActiveJobId(jobId)
      setStartedAt(Date.now())
      setElapsedSecs(0)
      setIsRecording(true)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }

  const stop = async () => {
    if (!activeJobId || isStopping) return
    const jobId = activeJobId
    // Flip UI state synchronously so a second click on the button can't fire
    // meeting_stop_live again before the first call returns.
    setIsStopping(true)
    setIsRecording(false)
    setActiveJobId(null)
    setStartedAt(null)
    try {
      await meetingStopLive(jobId)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setIsStopping(false)
    }
  }

  return (
    <div className="mt-6 p-4 rounded-lg bg-slate-900/30 border border-slate-800/60">
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-2">
          <span
            className={`inline-block w-2.5 h-2.5 rounded-full ${
              isRecording ? 'bg-red-500 animate-pulse' : 'bg-slate-700'
            }`}
          />
          <span className="text-sm text-slate-300">
            {isRecording ? 'Recording' : 'Live capture'}
          </span>
        </div>

        {isRecording ? (
          <>
            <div className="text-xs text-slate-500 font-mono">
              {formatElapsed(elapsedSecs)}
            </div>
            <div className="flex-1" />
            <button
              onClick={stop}
              disabled={isStopping}
              className="text-sm bg-red-600 hover:bg-red-500 disabled:opacity-50 disabled:cursor-not-allowed text-white px-3 py-1 rounded"
            >
              {isStopping ? 'Stopping…' : 'Stop & transcribe'}
            </button>
          </>
        ) : isStopping ? (
          <>
            <div className="text-xs text-slate-500">Finalising…</div>
            <div className="flex-1" />
          </>
        ) : (
          <>
            <select
              value={source}
              onChange={(e) => setSource(e.target.value as LiveSource)}
              className="text-xs bg-slate-900/60 border border-slate-800 rounded px-2 py-1 text-slate-300"
            >
              <option value="mic_only">Microphone</option>
              <option value="system_only">System audio (3.2b — not yet wired)</option>
              <option value="mic_and_system">Mic + system (3.2b — not yet wired)</option>
            </select>
            <div className="flex-1" />
            <button
              onClick={start}
              className="text-sm bg-indigo-600 hover:bg-indigo-500 text-white px-3 py-1 rounded"
            >
              Start recording
            </button>
          </>
        )}
      </div>
      {error && (
        <div className="mt-2 text-xs text-red-400">{error}</div>
      )}
      {!isRecording && (
        <div className="mt-2 text-xs text-slate-500">
          Records from your selected microphone. On stop, the audio is transcribed,
          summarised, and saved to history.
        </div>
      )}
    </div>
  )
}
