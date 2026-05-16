import { useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { DropZone, ACCEPTED_EXTS } from './DropZone'
import { JobList } from './JobList'
import { TranscriptViewer } from './TranscriptViewer'
import {
  meetingEnqueueFile,
  onMeetingProgress,
  onMeetingDone,
  onMeetingError,
} from '../../lib/contract'
import { useMeetingStore } from '../../store/useMeetingStore'

export function Transcribe() {
  const [dropError, setDropError] = useState<string | null>(null)
  const { upsertProgress, removeJob, addTranscript } = useMeetingStore()

  useEffect(() => {
    const unlistens: Array<() => void> = []

    listen<{ paths: string[] }>('tauri://drag-drop', async (e) => {
      const path = e.payload?.paths?.[0]
      if (!path) return
      const lower = path.toLowerCase()
      const ext = '.' + (lower.split('.').pop() ?? '')
      if (!ACCEPTED_EXTS.includes(ext)) {
        setDropError(`Unsupported format: ${ext}`)
        return
      }
      setDropError(null)
      try {
        await meetingEnqueueFile(path)
      } catch (err) {
        setDropError(String(err))
      }
    }).then((u) => unlistens.push(u))

    onMeetingProgress((p) => upsertProgress(p)).then((u) => unlistens.push(u))
    onMeetingDone((e) => { removeJob(e.job_id); addTranscript(e.transcript) }).then((u) => unlistens.push(u))
    onMeetingError((e) => { removeJob(e.job_id); setDropError(e.reason) }).then((u) => unlistens.push(u))

    return () => unlistens.forEach((u) => u())
  }, [])

  return (
    <div className="p-6">
      <h2 className="text-lg font-medium mb-4 text-slate-200">Transcribe</h2>
      <DropZone errorMessage={dropError} />
      <JobList />
      <TranscriptViewer />
    </div>
  )
}
