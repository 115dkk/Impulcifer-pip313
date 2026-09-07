//! Job model mirroring 2.x `ImpulciferApplicationService`: one active job,
//! monotonically increasing per-job event sequence, bounded event history and
//! cooperative cancellation.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Maximum retained events per job (2.x `_MAX_JOB_EVENTS`).
pub const MAX_JOB_EVENTS: usize = 2000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Recording,
    Brir,
    OutputRecovery,
    Update,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Running,
    CancelRequested,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            JobStatus::Succeeded | JobStatus::Failed | JobStatus::Cancelled
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobEventKind {
    Progress,
    Log,
    Status,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JobEvent {
    pub seq: u64,
    #[serde(rename = "type")]
    pub kind: JobEventKind,
    pub payload: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JobSnapshot {
    pub job_id: String,
    pub kind: JobKind,
    pub status: JobStatus,
    pub cancellable: bool,
    pub progress: f64,
    pub result: Option<Value>,
    pub error: Option<Value>,
    pub next_seq: u64,
}
