#![forbid(unsafe_code)]

use impulcifer_jobs::registry::{JobFailure, JobRegistry, PollResult};
use impulcifer_types::ipc::ErrorCode;
use impulcifer_types::job::{JobKind, JobSnapshot, JobStatus};
use serde_json::json;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

fn blocked(registry: &JobRegistry, cancellable: bool, honor: bool) -> (JobSnapshot, Sender<()>) {
    let (send, receive) = mpsc::channel();
    let job = registry
        .start(JobKind::Brir, cancellable, move |context| {
            receive.recv_timeout(Duration::from_secs(10)).unwrap();
            if honor {
                context.check_cancelled()?;
            }
            Ok(json!({"done":true}))
        })
        .unwrap();
    (job, send)
}
fn finished(registry: &JobRegistry, id: &str) -> PollResult {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let poll = registry.poll(id, 0).unwrap();
        if poll.job.status.is_terminal() {
            return poll;
        }
        assert!(Instant::now() < deadline, "job failed to finish");
        std::thread::yield_now();
    }
}

#[test]
fn start_rejects_second_active_job_with_job_busy() {
    let registry = JobRegistry::new();
    let (job, send) = blocked(&registry, true, false);
    assert_eq!(job.status, JobStatus::Running);
    assert_eq!(registry.active().unwrap().job_id, job.job_id);
    assert_eq!(
        registry
            .start(JobKind::Recording, false, |_| Ok(json!({})))
            .err(),
        Some(ErrorCode::JobBusy)
    );
    send.send(()).unwrap();
    finished(&registry, &job.job_id);
}
#[test]
fn poll_returns_events_after_seq_and_next_seq() {
    let registry = JobRegistry::new();
    let job = registry
        .start(JobKind::Brir, true, |context| {
            context.progress(0.5, json!({"message":"half","key":"stage"}));
            context.log(json!({"level":"INFO","message":"hello"}));
            Ok(json!({"file":"out"}))
        })
        .unwrap();
    let poll = finished(&registry, &job.job_id);
    assert_eq!(poll.events[0].payload, json!({"status":"running"}));
    assert_eq!(
        poll.events[1].payload,
        json!({"progress":0.5,"message":"half","key":"stage"})
    );
    let after = registry.poll(&job.job_id, 2).unwrap();
    assert_eq!(
        after.events.iter().map(|e| e.seq).collect::<Vec<_>>(),
        vec![3, 4]
    );
    assert_eq!(after.next_seq, 4);
    assert_eq!(after.timestamps_ms.len(), 2);
    assert!(after.timestamps_ms.values().all(|time| *time > 0));
    assert_eq!(registry.poll(&job.job_id, 999).unwrap().next_seq, 4);
    assert!(registry.poll(&job.job_id, 999).unwrap().events.is_empty());
}
#[test]
fn events_are_bounded_to_2000() {
    let registry = JobRegistry::new();
    let job = registry
        .start(JobKind::Brir, true, |context| {
            for index in 0..2500 {
                context.log(json!({"index":index}));
            }
            Ok(json!({}))
        })
        .unwrap();
    let poll = finished(&registry, &job.job_id);
    assert_eq!(poll.next_seq, 2502);
    assert_eq!(poll.events.len(), 2000);
    assert_eq!(poll.timestamps_ms.len(), 2000);
    assert_eq!(poll.events[0].seq, 503);
    assert_eq!(poll.events[1999].seq, 2502);
}
#[test]
fn cancel_marks_cancel_requested_then_cancelled_when_body_honours_token() {
    let registry = JobRegistry::new();
    let (job, send) = blocked(&registry, true, true);
    let cancel = registry.cancel(&job.job_id).unwrap();
    assert_eq!(cancel.status, JobStatus::CancelRequested);
    assert_eq!(
        registry.active().unwrap().status,
        JobStatus::CancelRequested
    );
    assert_eq!(
        registry.poll(&job.job_id, 1).unwrap().events[0].payload,
        json!({"status":"cancel_requested"})
    );
    send.send(()).unwrap();
    let done = finished(&registry, &job.job_id);
    assert_eq!(done.job.status, JobStatus::Cancelled);
    assert!(done.job.error.is_none());
    assert!(done.job.result.is_none());
    assert!(registry.active().is_none());
}
#[test]
fn cancel_on_non_cancellable_job_is_rejected() {
    let registry = JobRegistry::new();
    let (job, send) = blocked(&registry, false, true);
    assert_eq!(
        registry.cancel(&job.job_id).err(),
        Some(ErrorCode::JobNotCancellable)
    );
    assert_eq!(registry.poll(&job.job_id, 0).unwrap().next_seq, 1);
    send.send(()).unwrap();
    finished(&registry, &job.job_id);
    assert_eq!(
        registry.cancel(&job.job_id).unwrap().status,
        JobStatus::Succeeded
    );
}
#[test]
fn failed_body_produces_failed_snapshot_with_error_dict() {
    let registry = JobRegistry::new();
    let error = json!({"code":"OUTPUT_MISSING","message":"missing","details":{"path":"out"},"retryable":false});
    let copy = error.clone();
    let job = registry
        .start(JobKind::Brir, true, move |_| {
            Err(JobFailure {
                error: copy,
                cancelled: false,
            })
        })
        .unwrap();
    let poll = finished(&registry, &job.job_id);
    assert_eq!(poll.job.status, JobStatus::Failed);
    assert_eq!(poll.job.error, Some(error));
    assert_eq!(
        poll.events.last().unwrap().payload,
        json!({"status":"failed"})
    );
}
#[test]
fn terminal_jobs_remain_pollable() {
    let registry = JobRegistry::new();
    let first = registry
        .start(JobKind::Brir, true, |_| Ok(json!({"first":true})))
        .unwrap();
    finished(&registry, &first.job_id);
    let (next, send) = blocked(&registry, false, false);
    assert_eq!(
        registry.poll(&first.job_id, 0).unwrap().job.result,
        Some(json!({"first":true}))
    );
    send.send(()).unwrap();
    finished(&registry, &next.job_id);
}
#[test]
fn terminal_eviction_matches_python_start_time_retention() {
    let registry = JobRegistry::new();
    let mut ids = Vec::new();
    for _ in 0..8 {
        let job = registry
            .start(JobKind::Brir, true, |_| Ok(json!({})))
            .unwrap();
        finished(&registry, &job.job_id);
        ids.push(job.job_id);
    }
    assert!(registry.poll(&ids[0], 0).is_ok());
    let (job, send) = blocked(&registry, true, false);
    assert_eq!(
        registry.poll(&ids[0], 0).err(),
        Some(ErrorCode::JobNotFound)
    );
    for id in &ids[1..] {
        assert!(registry.poll(id, 0).is_ok());
    }
    send.send(()).unwrap();
    finished(&registry, &job.job_id);
}
#[test]
fn repeated_cancel_emits_and_success_after_cancel_remains_success() {
    let registry = JobRegistry::new();
    let (job, send) = blocked(&registry, true, false);
    assert_eq!(registry.cancel(&job.job_id).unwrap().next_seq, 2);
    assert_eq!(registry.cancel(&job.job_id).unwrap().next_seq, 3);
    send.send(()).unwrap();
    let poll = finished(&registry, &job.job_id);
    assert_eq!(poll.job.status, JobStatus::Succeeded);
    assert_eq!(registry.cancel(&job.job_id).unwrap().next_seq, 4);
    assert_eq!(
        registry.cancel("missing").err(),
        Some(ErrorCode::JobNotFound)
    );
}
#[test]
fn panic_becomes_internal_error_and_registry_can_start_again() {
    let registry = JobRegistry::new();
    let job = registry
        .start(JobKind::Brir, true, |_| panic!("body panic"))
        .unwrap();
    let poll = finished(&registry, &job.job_id);
    assert_eq!(
        poll.job.error.unwrap(),
        json!({"code":"INTERNAL_ERROR","message":"body panic","details":{},"retryable":false})
    );
    let next = registry
        .start(JobKind::Brir, true, |_| Ok(json!({})))
        .unwrap();
    finished(&registry, &next.job_id);
}
#[test]
fn progress_is_clamped_preserves_payload_and_ids_have_uuid4_shape() {
    let registry = JobRegistry::new();
    let job = registry
        .start(JobKind::Brir, true, |context| {
            for value in [-1.0, 2.0, f64::NAN] {
                context.progress(value, json!({"progress":99,"extra":true}));
            }
            Ok(json!({}))
        })
        .unwrap();
    let parts: Vec<_> = job.job_id.split('-').collect();
    assert_eq!(
        parts.iter().map(|s| s.len()).collect::<Vec<_>>(),
        vec![8, 4, 4, 4, 12]
    );
    assert!(
        parts
            .iter()
            .all(|s| s.chars().all(|c| c.is_ascii_hexdigit()))
    );
    assert!(parts[2].starts_with('4'));
    assert!(matches!(
        parts[3].chars().next().unwrap(),
        '8' | '9' | 'a' | 'b'
    ));
    let poll = finished(&registry, &job.job_id);
    for (event, expected) in poll.events[1..4].iter().zip([0.0, 1.0, 1.0]) {
        assert_eq!(event.payload, json!({"progress":expected,"extra":true}));
    }
}
