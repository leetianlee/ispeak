import { create } from 'zustand'
import { MeetingProgress, MeetingTranscript } from '../lib/contract'

interface JobView {
  id: string
  state: string
  chunksDone: number
  chunksTotal: number
}

interface MeetingStore {
  jobs: Record<string, JobView>
  transcripts: MeetingTranscript[]
  lastError: string | null
  upsertProgress: (p: MeetingProgress) => void
  removeJob: (id: string) => void
  addTranscript: (t: MeetingTranscript) => void
  setLastError: (msg: string | null) => void
}

export const useMeetingStore = create<MeetingStore>((set) => ({
  jobs: {},
  transcripts: [],
  lastError: null,
  upsertProgress: (p) =>
    set((s) => ({
      jobs: {
        ...s.jobs,
        [p.job_id]: {
          id: p.job_id,
          state: p.state,
          chunksDone: p.chunks_done,
          chunksTotal: p.chunks_total,
        },
      },
    })),
  removeJob: (id) =>
    set((s) => {
      const next = { ...s.jobs }
      delete next[id]
      return { jobs: next }
    }),
  addTranscript: (t) => set((s) => ({ transcripts: [t, ...s.transcripts] })),
  setLastError: (msg) => set({ lastError: msg }),
}))
