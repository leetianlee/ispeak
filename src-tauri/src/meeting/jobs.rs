//! In-process job queue with a single worker slot.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

use crate::meeting::types::{Job, JobMode, JobState, Progress};

#[derive(Debug, Clone, serde::Serialize)]
pub struct QueueSnapshot {
    pub running: Option<Job>,
    pub queued: Vec<Job>,
}

#[derive(Default)]
pub struct JobQueue {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    running: Option<RunningJob>,
    queued: VecDeque<Job>,
}

struct RunningJob {
    job: Job,
    cancel: Arc<AtomicBool>,
}

impl JobQueue {
    pub fn new() -> Self { Self::default() }

    pub fn enqueue(&self, mode: JobMode) -> Uuid {
        let id = Uuid::new_v4();
        let job = Job {
            id,
            mode,
            state: JobState::Queued,
            created_at: now_millis(),
            progress: Progress::default(),
        };
        self.inner.lock().unwrap().queued.push_back(job);
        id
    }

    pub fn start_next(&self) -> Option<(Job, Arc<AtomicBool>)> {
        let mut g = self.inner.lock().unwrap();
        if g.running.is_some() { return None; }
        let mut job = g.queued.pop_front()?;
        job.state = JobState::Transcribing;
        let cancel = Arc::new(AtomicBool::new(false));
        g.running = Some(RunningJob { job: job.clone(), cancel: cancel.clone() });
        Some((job, cancel))
    }

    pub fn cancel(&self, id: Uuid) -> bool {
        let mut g = self.inner.lock().unwrap();
        if let Some(running) = &g.running {
            if running.job.id == id {
                running.cancel.store(true, Ordering::SeqCst);
                return true;
            }
        }
        let before = g.queued.len();
        g.queued.retain(|j| j.id != id);
        before != g.queued.len()
    }

    pub fn finish_running(&self, final_state: JobState) -> Option<Job> {
        let mut g = self.inner.lock().unwrap();
        let mut r = g.running.take()?;
        r.job.state = final_state;
        Some(r.job)
    }

    pub fn update_progress(&self, id: Uuid, p: Progress) {
        let mut g = self.inner.lock().unwrap();
        if let Some(r) = g.running.as_mut() {
            if r.job.id == id { r.job.progress = p; }
        }
    }

    pub fn update_state(&self, id: Uuid, state: JobState) {
        let mut g = self.inner.lock().unwrap();
        if let Some(r) = g.running.as_mut() {
            if r.job.id == id { r.job.state = state; }
        }
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        let g = self.inner.lock().unwrap();
        QueueSnapshot {
            running: g.running.as_ref().map(|r| r.job.clone()),
            queued: g.queued.iter().cloned().collect(),
        }
    }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn file_mode() -> JobMode {
        JobMode::FileImport { path: PathBuf::from("/tmp/x.wav") }
    }

    #[test]
    fn enqueue_then_start_returns_first_job() {
        let q = JobQueue::new();
        let id1 = q.enqueue(file_mode());
        let _id2 = q.enqueue(file_mode());
        let (job, _cancel) = q.start_next().expect("a job should start");
        assert_eq!(job.id, id1);
        assert_eq!(job.state, JobState::Transcribing);
    }

    #[test]
    fn cannot_start_two_concurrent_jobs() {
        let q = JobQueue::new();
        q.enqueue(file_mode());
        q.enqueue(file_mode());
        assert!(q.start_next().is_some());
        assert!(q.start_next().is_none(), "second start_next must be None while one runs");
    }

    #[test]
    fn cancel_running_sets_flag() {
        let q = JobQueue::new();
        let id = q.enqueue(file_mode());
        let (_job, cancel) = q.start_next().unwrap();
        assert!(q.cancel(id));
        assert!(cancel.load(Ordering::SeqCst));
    }

    #[test]
    fn cancel_queued_removes_from_queue() {
        let q = JobQueue::new();
        q.enqueue(file_mode());
        let qid = q.enqueue(file_mode());
        q.start_next().unwrap();
        assert!(q.cancel(qid));
        assert_eq!(q.snapshot().queued.len(), 0);
    }

    #[test]
    fn finish_running_clears_slot() {
        let q = JobQueue::new();
        q.enqueue(file_mode());
        q.start_next().unwrap();
        let done = q.finish_running(JobState::AwaitingUserSave).unwrap();
        assert_eq!(done.state, JobState::AwaitingUserSave);
        assert!(q.snapshot().running.is_none());
    }
}
