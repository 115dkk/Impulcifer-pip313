# P04 (ASTRA): `impulcifer-jobs` registry + `impulcifer-service` non-DSP methods

You are implementing two crates of the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read first: `E:/Impulcifer/docs/rust/ARCHITECTURE.md` (sections 3.2, 3.3, 7), `E:/Impulcifer/crates/impulcifer-types/src/{ipc.rs,job.rs,config.rs}`, and the 2.x oracle `E:/Impulcifer/application/impulcifer_service.py` in full. The Rust service must return the same JSON shapes the Python service returns, because the unchanged frontend (`E:/Impulcifer/webview_ui/app.js`, read it) consumes them. Where the Python shape is ambiguous, the frontend's reading of the field decides.

Run every command in the foreground. Never use background execution.

## Scope

### `impulcifer-jobs` (`crates/impulcifer-jobs/src/registry.rs`)

```rust
pub struct JobRegistry { /* Mutex<inner> */ }
pub struct JobHandle { pub job_id: String, pub cancel: CancelToken }
pub trait JobSink: Send { fn progress(&self, progress: f64, payload: Value); fn log(&self, payload: Value); }
impl JobRegistry {
    pub fn new() -> Self;
    /// Fails with JOB_BUSY (as an ErrorCode) when a non-terminal job exists.
    pub fn start(&self, kind: JobKind, cancellable: bool, body: impl FnOnce(&JobContext) -> Result<Value, JobFailure> + Send + 'static) -> Result<JobSnapshot, ErrorCode>;
    pub fn poll(&self, job_id: &str, after_seq: u64) -> Result<PollResult, ErrorCode>;   // JOB_NOT_FOUND
    pub fn cancel(&self, job_id: &str) -> Result<JobSnapshot, ErrorCode>;               // JOB_NOT_FOUND, JOB_NOT_CANCELLABLE
    pub fn active(&self) -> Option<JobSnapshot>;
}
pub struct JobContext { pub job_id: String, pub cancel: CancelToken, /* emit helpers */ }
impl JobContext { pub fn progress(&self, fraction: f64, payload: Value); pub fn log(&self, payload: Value); pub fn check_cancelled(&self) -> Result<(), JobFailure>; }
pub struct JobFailure { pub error: Value /* the 2.x error dict */, pub cancelled: bool }
pub struct PollResult { pub job: JobSnapshot, pub events: Vec<JobEvent>, pub next_seq: u64 }
```

Semantics to reproduce from the Python service (`_start_job`, `_run_job`, `_emit`, `_append_event`, `_finish_job`, `poll_job`, `cancel_job`, `_MAX_JOB_EVENTS`, `_TERMINAL_STATES`): one active job at a time; job ids are UUID4 strings; the first event is `status {"status": "running"}`; `progress` payloads carry `progress` in 0..1 plus whatever the caller passes; `cancel` sets `cancel_requested` and appends a status event but the job only becomes `cancelled` when the body returns a cancelled failure; bodies run on a dedicated `std::thread`; the event list is bounded to 2000 with oldest dropped; `poll` returns events with `seq > after_seq`; terminal jobs remain pollable; `next_seq` is the last seq of the job (not of the page). Read the Python to confirm each of these and note any difference you find in the report.

### `impulcifer-service` (`crates/impulcifer-service/src/**`)

Implement these methods with the exact Python response shapes: `bootstrap` (version from `env!("CARGO_PKG_VERSION")` plus the fields the frontend reads: read `_bootstrap_payload` in Python and `boot()` in app.js; the `brir_defaults` come from `ProcessingConfig::default()` serialised; sweep presets from `impulcifer_types::constants`; `active_job` from the registry), `get_ui_settings`, `set_language`, `set_theme`, `set_skin`, `set_frontend` (the CTk frontend no longer exists in 3.x: accept the call, persist the value, and return the same shape), `get_system_info` (port the fields; replace Python/GIL rows with Rust toolchain/version rows but keep the keys the frontend renders, read app.js lines 1216 to 1244), `poll_job`, `cancel_job`, `resolve_recording_paths` (pure path logic: port `_resolve_recording_paths` and its helpers from Python, including the sweep spec grammar it accepts), `list_audio_devices` (through `impulcifer_audio_io::default_backend().enumerate()`, grouped exactly like the Python `list_audio_devices`: `host_apis`, `devices` with `index`, `name`, `host_api`, `max_input_channels`, `max_output_channels`, default indices), `open_path`, `open_url` (allowlist of names ported from `impulcifer_webview.py`), `select_file`, `select_directory` (through `HostAdapter`). Leave `start_recording`, `start_brir`, `start_output_recovery`, `detect_sweep`, `generate_sweep_set`, `check_for_updates`, `start_update`, `apply_pending_update` returning the current `INTERNAL_ERROR ... not implemented` envelope; they belong to later packets.

Settings persistence: mirror the Python settings file (find where `set_language`/`set_theme` persist in 2.x: `i18n/localization.py` and the service's settings helpers) and store the same keys in the same JSON file location so a 2.x install's choices carry over. Document the path resolution.

Argument handling: the frontend passes positional arguments (`args: Vec<Value>`); implement a small `Args` helper that extracts by position with defaults (`poll_job(job_id, after_seq=0)`), returning `INVALID_REQUEST` envelopes on type errors with the same message style as Python.

### Tests

- `impulcifer-jobs`: `start_rejects_second_active_job_with_job_busy`, `poll_returns_events_after_seq_and_next_seq`, `events_are_bounded_to_2000`, `cancel_marks_cancel_requested_then_cancelled_when_body_honours_token`, `cancel_on_non_cancellable_job_is_rejected`, `failed_body_produces_failed_snapshot_with_error_dict`, `terminal_jobs_remain_pollable`.
- `impulcifer-service`: one test per implemented method asserting the envelope shape against the Python shape you read (name them `ipc_<method>_shape`), plus `resolve_recording_paths_matches_python_examples` with at least six input/output pairs taken from the Python tests (`E:/Impulcifer/tests/test_application_service.py`).
- Update `E:/Impulcifer/features.toml`: set each implemented `ipc.*` entry to `implemented` with the exact test names. Leave the rest untouched.

## Allowed files

`E:/Impulcifer/crates/impulcifer-jobs/**`, `E:/Impulcifer/crates/impulcifer-service/**`, `E:/Impulcifer/features.toml` (only `ipc.*` entries you implemented). Nothing else; report gaps in `impulcifer-types` instead of editing it.

## Hard rules

- `#![forbid(unsafe_code)]` stays; no unsafe.
- Service methods never panic across the boundary: wrap bodies so any panic becomes an `INTERNAL_ERROR` envelope (`std::panic::catch_unwind` on the job thread is acceptable; document it).
- Do not change method names, envelope shapes or event names; the frontend is frozen.

## Verification (foreground, paste output)

```
cargo fmt --all -- --check
cargo clippy -p impulcifer-jobs -p impulcifer-service --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-jobs -p impulcifer-service
cargo test -p impulcifer-policy
```

## Report format

(1) files changed; (2) verification outputs verbatim; (3) a table method → Python source lines you mirrored → test name; (4) every place where Python behaviour was ambiguous and what you chose; (5) gaps in `impulcifer-types`; (6) anything undone. Do not end your turn before the commands have completed.
