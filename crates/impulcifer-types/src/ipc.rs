//! The pywebview-compatible IPC surface: method names, error codes and the
//! `{ok, data}` / `{ok: false, error}` envelope from 2.x
//! `application/impulcifer_service.py`. Frontend code calls these by name;
//! nothing here may be renamed without an ADR.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpcMethod {
    Bootstrap,
    ListAudioDevices,
    StartRecording,
    StartBrir,
    StartOutputRecovery,
    PlanOutputRecovery,
    PollJob,
    CancelJob,
    GetUiSettings,
    SetLanguage,
    SetTheme,
    SetSkin,
    SetFrontend,
    GetSystemInfo,
    ResolveRecordingPaths,
    DetectSweep,
    GenerateSweepSet,
    OpenPath,
    CheckForUpdates,
    StartUpdate,
    ApplyPendingUpdate,
    SelectFile,
    SelectDirectory,
    OpenUrl,
}

impl IpcMethod {
    pub const ALL: [IpcMethod; 24] = [
        IpcMethod::Bootstrap,
        IpcMethod::ListAudioDevices,
        IpcMethod::StartRecording,
        IpcMethod::StartBrir,
        IpcMethod::StartOutputRecovery,
        IpcMethod::PlanOutputRecovery,
        IpcMethod::PollJob,
        IpcMethod::CancelJob,
        IpcMethod::GetUiSettings,
        IpcMethod::SetLanguage,
        IpcMethod::SetTheme,
        IpcMethod::SetSkin,
        IpcMethod::SetFrontend,
        IpcMethod::GetSystemInfo,
        IpcMethod::ResolveRecordingPaths,
        IpcMethod::DetectSweep,
        IpcMethod::GenerateSweepSet,
        IpcMethod::OpenPath,
        IpcMethod::CheckForUpdates,
        IpcMethod::StartUpdate,
        IpcMethod::ApplyPendingUpdate,
        IpcMethod::SelectFile,
        IpcMethod::SelectDirectory,
        IpcMethod::OpenUrl,
    ];

    /// The wire name used by `window.pywebview.api.<name>` (snake_case).
    pub fn wire_name(self) -> &'static str {
        match self {
            IpcMethod::Bootstrap => "bootstrap",
            IpcMethod::ListAudioDevices => "list_audio_devices",
            IpcMethod::StartRecording => "start_recording",
            IpcMethod::StartBrir => "start_brir",
            IpcMethod::StartOutputRecovery => "start_output_recovery",
            IpcMethod::PlanOutputRecovery => "plan_output_recovery",
            IpcMethod::PollJob => "poll_job",
            IpcMethod::CancelJob => "cancel_job",
            IpcMethod::GetUiSettings => "get_ui_settings",
            IpcMethod::SetLanguage => "set_language",
            IpcMethod::SetTheme => "set_theme",
            IpcMethod::SetSkin => "set_skin",
            IpcMethod::SetFrontend => "set_frontend",
            IpcMethod::GetSystemInfo => "get_system_info",
            IpcMethod::ResolveRecordingPaths => "resolve_recording_paths",
            IpcMethod::DetectSweep => "detect_sweep",
            IpcMethod::GenerateSweepSet => "generate_sweep_set",
            IpcMethod::OpenPath => "open_path",
            IpcMethod::CheckForUpdates => "check_for_updates",
            IpcMethod::StartUpdate => "start_update",
            IpcMethod::ApplyPendingUpdate => "apply_pending_update",
            IpcMethod::SelectFile => "select_file",
            IpcMethod::SelectDirectory => "select_directory",
            IpcMethod::OpenUrl => "open_url",
        }
    }

    pub fn from_wire_name(name: &str) -> Option<IpcMethod> {
        IpcMethod::ALL
            .iter()
            .copied()
            .find(|m| m.wire_name() == name)
    }
}

/// Error codes used by the 2.x service. Do not add codes without updating
/// the frontend expectations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidRequest,
    FileNotFound,
    InternalError,
    UpdateFailed,
    OutputMissing,
    JobNotFound,
    DeviceError,
    ConfirmationRequired,
    UpdateCheckFailed,
    JobNotCancellable,
    JobBusy,
}

/// `{"ok": true, "data": ...}`
pub fn ok(data: Value) -> Value {
    json!({ "ok": true, "data": data })
}

/// `{"ok": false, "error": {"code", "message", "details", "retryable"}}`
pub fn error(
    code: ErrorCode,
    message: impl Into<String>,
    details: Value,
    retryable: bool,
) -> Value {
    json!({
        "ok": false,
        "error": {
            "code": code,
            "message": message.into(),
            "details": details,
            "retryable": retryable,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_names_round_trip() {
        for method in IpcMethod::ALL {
            assert_eq!(IpcMethod::from_wire_name(method.wire_name()), Some(method));
        }
        assert_eq!(IpcMethod::ALL.len(), 24);
    }

    #[test]
    fn envelopes_match_python_shape() {
        let good = ok(json!({"x": 1}));
        assert_eq!(good["ok"], json!(true));
        assert_eq!(good["data"]["x"], json!(1));
        let bad = error(ErrorCode::JobBusy, "busy", json!({}), false);
        assert_eq!(bad["ok"], json!(false));
        assert_eq!(bad["error"]["code"], json!("JOB_BUSY"));
        assert_eq!(bad["error"]["retryable"], json!(false));
    }
}
