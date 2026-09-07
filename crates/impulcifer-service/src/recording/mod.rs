//! Recording request, playback and progress adapters for the frozen 2.x IPC.
pub mod devices;
pub mod naming;
pub mod progress;
pub mod request;
pub mod run;
pub mod sweep;

use impulcifer_jobs::registry::JobFailure;
use impulcifer_types::ipc::{self, ErrorCode};
use serde_json::{Value, json};

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct RecordingError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Value,
    pub retryable: bool,
}
impl RecordingError {
    pub fn device(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::DeviceError,
            message: message.into(),
            details: json!({}),
            retryable: true,
        }
    }
    pub fn internal(message: impl ToString) -> Self {
        Self {
            code: ErrorCode::InternalError,
            message: message.to_string(),
            details: json!({}),
            retryable: false,
        }
    }
    pub fn envelope(&self) -> Value {
        ipc::error(
            self.code,
            &self.message,
            self.details.clone(),
            self.retryable,
        )
    }
    pub fn failure(self) -> JobFailure {
        JobFailure {
            error: self.envelope()["error"].clone(),
            cancelled: false,
        }
    }
}
