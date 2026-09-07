//! One active worker and a bounded replay journal, matching the Python service.
//! At each start, retain the seven newest terminal jobs plus the new job (the
//! Python `_MAX_TERMINAL_JOBS = 8` rule), rather than retaining jobs forever.

use std::collections::{HashMap, VecDeque};
use std::hash::{BuildHasher, RandomState};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use impulcifer_types::audio::CancelToken;
use impulcifer_types::ipc::ErrorCode;
use impulcifer_types::job::{
    JobEvent, JobEventKind, JobKind, JobSnapshot, JobStatus, MAX_JOB_EVENTS,
};
use serde_json::{Value, json};

#[derive(Clone, Default)]
pub struct JobRegistry {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    jobs: VecDeque<Job>,
    active: Option<String>,
}

struct Job {
    snapshot: JobSnapshot,
    cancel: CancelToken,
    events: VecDeque<(JobEvent, u64)>,
    worker: Option<std::thread::JoinHandle<()>>,
}

pub struct JobHandle {
    pub job_id: String,
    pub cancel: CancelToken,
}

pub trait JobSink: Send {
    fn progress(&self, progress: f64, payload: Value);
    fn log(&self, payload: Value);
}

pub struct JobContext {
    pub job_id: String,
    pub cancel: CancelToken,
    registry: JobRegistry,
}

#[derive(Debug)]
pub struct JobFailure {
    pub error: Value,
    pub cancelled: bool,
}

pub struct PollResult {
    pub job: JobSnapshot,
    pub events: Vec<JobEvent>,
    pub next_seq: u64,
    /// Side metadata because the shared JobEvent type lacks Python's timestamp.
    /// Captured under the same lock as events, so eviction cannot race the wire serializer.
    pub timestamps_ms: HashMap<u64, u64>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// RandomState supplies independently randomly seeded hashers without adding a
/// dependency. These IDs are UUID v4/variant 1, not authentication secrets.
fn uuid4() -> String {
    let high = RandomState::new().hash_one(0_u64);
    let low = RandomState::new().hash_one(1_u64);
    let mut bytes = ((u128::from(high) << 64) | u128::from(low)).to_be_bytes();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = format!("{:032x}", u128::from_be_bytes(bytes));
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

pub fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Internal panic.".to_owned())
}

impl JobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(
        &self,
        kind: JobKind,
        cancellable: bool,
        body: impl FnOnce(&JobContext) -> Result<Value, JobFailure> + Send + 'static,
    ) -> Result<JobSnapshot, ErrorCode> {
        let mut inner = lock(&self.inner);
        if inner
            .jobs
            .iter()
            .any(|job| !job.snapshot.status.is_terminal())
        {
            return Err(ErrorCode::JobBusy);
        }
        while inner.jobs.len() >= 8 {
            inner.jobs.pop_front();
        }
        let mut id = uuid4();
        while inner.jobs.iter().any(|job| job.snapshot.job_id == id) {
            id = uuid4();
        }
        let cancel = CancelToken::new();
        let mut job = Job {
            snapshot: JobSnapshot {
                job_id: id.clone(),
                kind,
                status: JobStatus::Running,
                cancellable,
                progress: 0.0,
                result: None,
                error: None,
                next_seq: 0,
            },
            cancel: cancel.clone(),
            events: VecDeque::new(),
            worker: None,
        };
        job.append(JobEventKind::Status, json!({"status": "running"}));
        let snapshot = job.snapshot.clone();
        inner.active = Some(id.clone());
        inner.jobs.push_back(job);
        let context = JobContext {
            job_id: id.clone(),
            cancel,
            registry: self.clone(),
        };
        let worker = std::thread::Builder::new()
            .name(format!("impulcifer-{}", &id[..8]))
            .spawn(move || {
                // The body never holds the journal mutex. A panic cannot strand
                // the active job or escape through the service boundary.
                let result = catch_unwind(AssertUnwindSafe(|| body(&context)))
                    .unwrap_or_else(|panic| Err(JobFailure {
                        error: json!({"code": "INTERNAL_ERROR", "message": panic_message(&*panic),
                            "details": {}, "retryable": false}), cancelled: false,
                    }));
                context.registry.finish(&context.job_id, result);
            });
        match worker {
            Ok(worker) => inner.jobs.back_mut().expect("new job").worker = Some(worker),
            Err(_) => {
                inner.jobs.pop_back();
                inner.active = None;
                return Err(ErrorCode::InternalError);
            }
        }
        Ok(snapshot)
    }

    /// Wait for a retained worker to exit, without holding the journal lock.
    /// Headless callers use this after cancellation to release resources before
    /// removing temporary inputs. Calling from the worker itself is rejected.
    pub fn join(&self, job_id: &str) -> Result<(), ErrorCode> {
        let worker = {
            let mut inner = lock(&self.inner);
            let job = inner
                .jobs
                .iter_mut()
                .find(|job| job.snapshot.job_id == job_id)
                .ok_or(ErrorCode::JobNotFound)?;
            if job
                .worker
                .as_ref()
                .is_some_and(|worker| worker.thread().id() == std::thread::current().id())
            {
                return Err(ErrorCode::InvalidRequest);
            }
            job.worker.take()
        };
        if let Some(worker) = worker {
            worker.join().map_err(|_| ErrorCode::InternalError)?;
        }
        Ok(())
    }

    pub fn poll(&self, job_id: &str, after_seq: u64) -> Result<PollResult, ErrorCode> {
        let inner = lock(&self.inner);
        let job = inner
            .jobs
            .iter()
            .find(|job| job.snapshot.job_id == job_id)
            .ok_or(ErrorCode::JobNotFound)?;
        let selected: Vec<_> = job
            .events
            .iter()
            .filter(|(event, _)| event.seq > after_seq)
            .collect();
        Ok(PollResult {
            job: job.snapshot.clone(),
            events: selected.iter().map(|(event, _)| event.clone()).collect(),
            next_seq: job.snapshot.next_seq,
            timestamps_ms: selected
                .iter()
                .map(|(event, time)| (event.seq, *time))
                .collect(),
        })
    }

    pub fn cancel(&self, job_id: &str) -> Result<JobSnapshot, ErrorCode> {
        let mut inner = lock(&self.inner);
        let job = inner
            .jobs
            .iter_mut()
            .find(|job| job.snapshot.job_id == job_id)
            .ok_or(ErrorCode::JobNotFound)?;
        if job.snapshot.status.is_terminal() {
            return Ok(job.snapshot.clone());
        }
        if !job.snapshot.cancellable {
            return Err(ErrorCode::JobNotCancellable);
        }
        job.snapshot.status = JobStatus::CancelRequested;
        job.cancel.cancel();
        // Python emits on every request, including repeated cancellation.
        job.append(JobEventKind::Status, json!({"status": "cancel_requested"}));
        Ok(job.snapshot.clone())
    }

    pub fn active(&self) -> Option<JobSnapshot> {
        let inner = lock(&self.inner);
        inner
            .jobs
            .iter()
            .find(|job| Some(&job.snapshot.job_id) == inner.active.as_ref())
            .map(|job| job.snapshot.clone())
    }

    fn emit(&self, id: &str, kind: JobEventKind, payload: Value) {
        let mut inner = lock(&self.inner);
        if let Some(job) = inner.jobs.iter_mut().find(|job| job.snapshot.job_id == id) {
            if kind == JobEventKind::Progress {
                job.snapshot.progress = payload["progress"].as_f64().unwrap_or(0.0);
            }
            job.append(kind, payload);
        }
    }

    fn finish(&self, id: &str, result: Result<Value, JobFailure>) {
        let mut inner = lock(&self.inner);
        if let Some(job) = inner.jobs.iter_mut().find(|job| job.snapshot.job_id == id) {
            match result {
                Ok(value) => {
                    job.snapshot.status = JobStatus::Succeeded;
                    job.snapshot.result = Some(value);
                }
                Err(failure) if failure.cancelled => {
                    job.snapshot.status = JobStatus::Cancelled;
                }
                Err(failure) => {
                    job.snapshot.status = JobStatus::Failed;
                    job.snapshot.error = Some(failure.error);
                }
            }
            job.append(JobEventKind::Status, json!({"status": job.snapshot.status}));
        }
        if inner.active.as_deref() == Some(id) {
            inner.active = None;
        }
    }
}

impl Job {
    fn append(&mut self, kind: JobEventKind, payload: Value) {
        self.snapshot.next_seq += 1;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        self.events.push_back((
            JobEvent {
                seq: self.snapshot.next_seq,
                kind,
                payload,
            },
            timestamp,
        ));
        while self.events.len() > MAX_JOB_EVENTS {
            self.events.pop_front();
        }
    }
}

impl JobContext {
    pub fn progress(&self, fraction: f64, payload: Value) {
        let fraction = if fraction.is_nan() {
            1.0
        } else {
            fraction.clamp(0.0, 1.0)
        };
        let mut payload = payload.as_object().cloned().unwrap_or_default();
        payload.insert("progress".to_owned(), json!(fraction));
        self.registry
            .emit(&self.job_id, JobEventKind::Progress, Value::Object(payload));
    }
    pub fn log(&self, payload: Value) {
        self.registry.emit(&self.job_id, JobEventKind::Log, payload);
    }
    pub fn check_cancelled(&self) -> Result<(), JobFailure> {
        if self.cancel.is_cancelled() {
            Err(JobFailure {
                error: Value::Null,
                cancelled: true,
            })
        } else {
            Ok(())
        }
    }
}

impl JobSink for JobContext {
    fn progress(&self, progress: f64, payload: Value) {
        self.progress(progress, payload);
    }
    fn log(&self, payload: Value) {
        self.log(payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn join_waits_for_worker_cleanup_and_rejects_self_join() {
        let registry = JobRegistry::new();
        assert_eq!(registry.join("missing"), Err(ErrorCode::JobNotFound));
        let cleaned = Arc::new(std::sync::atomic::AtomicBool::new(false));
        struct Cleanup(Arc<std::sync::atomic::AtomicBool>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let cleanup = Cleanup(cleaned.clone());
        let job = registry
            .start(JobKind::Brir, true, move |ctx| {
                let _cleanup = cleanup;
                assert_eq!(
                    ctx.registry.join(&ctx.job_id),
                    Err(ErrorCode::InvalidRequest)
                );
                Ok(json!({}))
            })
            .unwrap();
        // Wait until the body has exercised the self-join check, then join.
        while !registry
            .poll(&job.job_id, 0)
            .unwrap()
            .job
            .status
            .is_terminal()
        {
            std::thread::yield_now();
        }
        registry.join(&job.job_id).unwrap();
        assert!(cleaned.load(std::sync::atomic::Ordering::SeqCst));
        registry.join(&job.job_id).unwrap();
    }

    #[test]
    fn poisoned_registry_lock_is_recovered() {
        let registry = JobRegistry::new();
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = registry.inner.lock().unwrap();
            panic!("poison test");
        }));
        assert!(registry.active().is_none());
        assert_eq!(
            registry.poll("missing", 0).err(),
            Some(ErrorCode::JobNotFound)
        );
    }
}
