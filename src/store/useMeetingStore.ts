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
  upsertProgress: (p: MeetingProgress) => void
  removeJob: (id: string) => void
  addTranscript: (t: MeetingTranscript) => void
}

export const useMeetingStore = create<MeetingStore>((set) => ({
  jobs: {},
  transcripts: [],
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
}))
