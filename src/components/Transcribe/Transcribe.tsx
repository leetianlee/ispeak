import { DropZone } from './DropZone'
import { JobList } from './JobList'
import { LiveCapture } from './LiveCapture'
import { TranscriptViewer } from './TranscriptViewer'
import { MeetingHistory } from './MeetingHistory'
import { useMeetingStore } from '../../store/useMeetingStore'

export function Transcribe() {
  const lastError = useMeetingStore((s) => s.lastError)

  return (
    <div className="p-6">
      <h2 className="text-lg font-medium mb-4 text-slate-200">Transcribe</h2>
      <DropZone errorMessage={lastError} />
      <LiveCapture />
      <JobList />
      <TranscriptViewer />
      <MeetingHistory />
    </div>
  )
}
