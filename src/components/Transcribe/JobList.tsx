import { useMeetingStore } from '../../store/useMeetingStore'
import { meetingCancel } from '../../lib/contract'

export function JobList() {
  const jobs = useMeetingStore((s) => Object.values(s.jobs))
  if (jobs.length === 0) return null
  return (
    <div className="mt-6">
      <h3 className="text-sm uppercase tracking-wider text-slate-500 mb-2">In progress</h3>
      {jobs.map((j) => {
        const pct = j.chunksTotal === 0 ? 0 : Math.round((j.chunksDone / j.chunksTotal) * 100)
        return (
          <div key={j.id} className="flex items-center gap-3 py-2 border-b border-slate-800">
            <div className="flex-1">
              <div className="text-sm text-slate-200">Job {j.id.slice(0, 8)}</div>
              <div className="h-1.5 bg-slate-800 rounded mt-1 overflow-hidden">
                <div className="h-full bg-indigo-400" style={{ width: `${pct}%` }} />
              </div>
              <div className="text-xs text-slate-500 mt-1">
                {j.state} · chunk {j.chunksDone} / {j.chunksTotal} · {pct}%
              </div>
            </div>
            <button
              onClick={() => meetingCancel(j.id)}
              className="text-xs text-slate-400 hover:text-rose-400 px-2 py-1"
            >
              Cancel
            </button>
          </div>
        )
      })}
    </div>
  )
}
