# DB01 (Daybreak): recording share mode, recovery plan IPC, Rust-edition system info

Work in `E:/Impulcifer` on the checked-out branch `claude/rust-alpha1-cleanup` (do not switch branches, do not commit, do not stash). Read `E:/Impulcifer/CLAUDE.md` (section "3.x Rust 워크스페이스"), `E:/Impulcifer/docs/rust/ARCHITECTURE.md` and `E:/Impulcifer/docs/adr/0003-3x-frontend-fork-and-overlay.md` first. Rules: `#![forbid(unsafe_code)]` everywhere (the `impulcifer-sys-win` budget is unchanged), no new runtime dependencies, no shell subprocesses (`impulcifer-policy` gate), run every command in the foreground and never in the background, never terminate a process, never touch `%LOCALAPPDATA%/Impulcifer` or any `Update.exe`.

**A second worker (ASTRA, packet A01) edits the app UI in the same tree at the same time.** Its files are `apps/impulcifer-app/ui/**`, `apps/impulcifer-app/src/*.js`, `apps/impulcifer-app/src/*.d.ts`, `apps/impulcifer-app/tests/*.cjs`, `apps/impulcifer-app/tests/ui_gallery.py`, `tests/app_smoke/**`, `crates/impulcifer-service/locales/*.json`, `docs/rust/UI.md`. Never open those for writing. Two tests belong to that worker and may fail while it works: `impulcifer-service::ui_keys_resolve_in_every_language` (`tests/ui_catalog.rs`) and `impulcifer-policy::app_scripts_opt_into_type_checking`; if only those fail, say so in the report and leave them.

This packet has three parts. The JSON shapes below are a contract with the UI worker: implement them exactly.

## Part 1. Device access preference for recording (`share_mode`)

Today `crates/impulcifer-audio-io/src/policy.rs` always tries exclusive first and falls back to shared auto-convert on `UnsupportedFormat`. The owner wants the user to be able to fix the mode: a fixed choice never falls back, and a refusal is reported as such.

### Types (`crates/impulcifer-types/src/audio.rs`)

```rust
/// The user's device-access request for a measurement session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharePreference {
    /// Exclusive first, shared auto-convert when the exclusive format is refused (the 2.x/alpha.0 policy).
    #[default]
    Auto,
    Exclusive,
    Shared,
}
impl SharePreference {
    pub fn as_str(self) -> &'static str;                 // "auto" | "exclusive" | "shared"
    pub fn parse(text: &str) -> Option<Self>;            // the same three spellings, nothing else
    pub fn fixed_mode(self) -> Option<ShareMode>;        // Auto -> None, Exclusive -> Exclusive, Shared -> SharedAutoConvert
}
```

`AudioBackend` gets one provided method: `fn selectable_share_modes(&self) -> &'static [ShareMode] { &[] }`. Empty means the backend cannot honour a fixed choice (only Auto is meaningful). `impulcifer_sys_win::WasapiBackend` returns `&[ShareMode::Exclusive, ShareMode::SharedAutoConvert]`; `CachedBackend` forwards to its inner backend; `CpalBackend` keeps the default (cpal ignores `ShareMode`, so a fixed choice there would be a lie).

### Policy and session (`crates/impulcifer-audio-io`)

- `policy.rs`: add `open_output_with_preference(backend, endpoint, spec, preference) -> Result<(Box<dyn OutputSession>, ShareMode), AudioError>` and `open_input_with_preference(...)`. `Auto` delegates to the existing `_with_policy` functions (keep them; tests and benches use them). A fixed preference calls `backend.open_output/open_input(endpoint, spec, mode)` exactly once and returns its error unchanged: no second attempt, no other mode.
- `session.rs`: `SessionRequest` gains `pub share: SharePreference` (`SessionRequest::new` sets `Auto`); `play_and_record` opens both sessions through the `_with_preference` functions with `request.share`. `Recording.output_mode` / `input_mode` keep reporting the mode actually opened.

### Request validation (`crates/impulcifer-service/src/recording/request.rs`)

- New optional wire field `share_mode` (add it to the known-field list). Missing or `null` means `Auto`. `"auto" | "exclusive" | "shared"` select the preference. Anything else (other strings, numbers, booleans) is `INVALID_REQUEST` with message `share_mode must be auto, exclusive or shared.` and `details: {"share_mode": <the raw value>}`. Validate it immediately after `mode` and before `record_dir`.
- `ValidatedRecording` gains `pub share: SharePreference`. The 136 golden cases in `tests/migration/goldens/p16_recording.json` carry no `share_mode`, so `golden_recording_validation_matches_python` must keep passing unchanged (the serialized `ValidatedRecording` gains the `share` key; if the golden compares the whole validated object, strip `share` in the test before comparing and say so in the report; do not edit the golden file).

### Service (`crates/impulcifer-service/src/lib.rs`, `recording/run.rs`)

- `StartRecording`: after validation and before starting the job, if `request.share.fixed_mode()` is `Some(mode)` and `self.backend.selectable_share_modes()` does not contain `mode`, return `INVALID_REQUEST` with message `This audio backend cannot fix the device access mode; use auto.` and `details: {"kind": "share_mode_unavailable", "share_mode": "<exclusive|shared>", "backend": "<self.backend.name()>"}`.
- `run.rs`: pass `v.share` into the `SessionRequest`. When `play_and_record` fails and `v.share` is fixed, the job fails with `DEVICE_ERROR`, message `The device did not open in <exclusive|shared> mode: <AudioError text>` and `details: {"kind": "share_mode_refused", "share_mode": "<exclusive|shared>", "reason": "<AudioError text>"}`; the `phase: "error"` progress event carries the same message. Auto failures keep today's message and details.
- On success the result JSON gains `"share": {"requested": "<auto|exclusive|shared>", "output": "<exclusive|shared_auto_convert>", "input": "<exclusive|shared_auto_convert>"}` (from `Recording.output_mode` / `input_mode`, serialized with the `ShareMode` serde names). Before the `saving` phase emit one log event: `ctx.log(json!({"level": "info", "key": "recording_share_mode_opened", "message": "Output: <mode> · Input: <mode>", "share": {...same object...}}))` where `<mode>` is `exclusive` or `shared (auto-convert)`.
- `bootstrap` `capabilities` gains `"share_modes": [...]`: always `"auto"` first, then `"exclusive"` and `"shared"` when the backend's `selectable_share_modes()` contains `Exclusive` / `SharedAutoConvert` respectively. Update `ipc_bootstrap_shape` (the test fixture's fake backend has no selectable modes, so `["auto"]`).

### Tests (these exact names)

- `impulcifer-audio-io::fixed_share_preference_never_falls_back` (in `tests/session_fake.rs` or a new `tests/policy_preference.rs` using that fake): a fake that refuses `Exclusive` with `UnsupportedFormat` and counts open attempts. `Auto` gives a shared session after two attempts. `Exclusive` gives the `UnsupportedFormat` error after one attempt. `Shared` gives a shared session after one attempt with no exclusive attempt.
- `impulcifer-audio-io::session_request_carries_the_share_preference`: `play_and_record` with the fake for all three preferences; `Recording.output_mode`/`input_mode` follow the preference (and the fixed exclusive case against the refusing fake returns the error, not a recording).
- `impulcifer-service::share_mode_request_validation`: default `auto`; three valid spellings; `"EXCLUSIVE"`, `"both"`, `1`, `true` rejected with the exact message and details; an invalid `share_mode` together with a missing `record_dir` reports the `share_mode` error first.
- `impulcifer-service::share_mode_unavailable_on_backends_without_selection`: with the `ipc.rs` fake backend (no selectable modes) `start_recording` with `exclusive` and `shared` gives `INVALID_REQUEST`, `details.kind == "share_mode_unavailable"`, no job started; with a fake whose `selectable_share_modes` returns both, the same request passes that check.
- `impulcifer-service::fixed_share_mode_refusal_reports_the_mode`: a recording job (the `recording.rs` fake-backend harness) where the fake refuses exclusive: `share_mode: "exclusive"` gives job `failed`, `DEVICE_ERROR`, `details.kind == "share_mode_refused"`, `details.share_mode == "exclusive"`, and the error progress event message starts with `The device did not open in exclusive mode`; `share_mode: "auto"` gives a succeeded job, `result.share.output == "shared_auto_convert"`, and one log event with `key == "recording_share_mode_opened"` precedes the `saving` phase.
- `impulcifer-service::bootstrap_lists_selectable_share_modes`: `["auto"]` with the fake; `["auto", "exclusive", "shared"]` with a fake returning both.
- `impulcifer-sys-win::wasapi_offers_both_share_modes` (`#[cfg(windows)]` unit test): `WasapiBackend::new().selectable_share_modes()` is exactly `[Exclusive, SharedAutoConvert]`.
- `impulcifer-audio-io::virtual_cable_honours_fixed_share_preferences` (`#[cfg(windows)]`, `#[ignore]`, next to `wasapi_session_virtual_cable` in `tests/hardware.rs`): on the CABLE-A pair of `docs/rust/HARDWARE.md`, run one short session per preference (`Auto`, `Exclusive`, `Shared`) through `play_and_record`; assert `Auto` and `Shared` succeed with the expected modes; record whether `Exclusive` succeeded or which error it returned. **Run it** (`cargo test -p impulcifer-audio-io --test hardware -- --ignored virtual_cable_honours_fixed_share_preferences --nocapture`) with the machine as it is, list the audio applications that were running in the report, never terminate any process, and paste the three outcomes.

## Part 2. `plan_output_recovery`: the dry run behind the Studio recovery screen

The Studio skin will show, before the user presses Restore, what was found in the folder and which files would be created. That needs the analysis of `recovery::recover_brir_outputs` without the writes.

### Service (`crates/impulcifer-service/src/recovery.rs`)

- Split `recover_brir_outputs` into `pub fn plan_brir_outputs(dir: &Path, options: &RecoveryOptions) -> Result<RecoveryPlan, RecoveryError>` and the existing `recover_brir_outputs`, which becomes "prepare, then write". Internally keep one `prepare(...)` that returns both the public plan and the private staged outputs (`PlannedOutput` with its tracks) so the two paths cannot diverge. Nothing about the written bytes, the created/existing lists, the error codes, messages or their order may change: `golden_recovery_scenarios_match_python`, `golden_recovery_errors_match_python`, `recovery_never_overwrites_existing_outputs`, `recovery_cleans_up_partial_writes_on_failure` and `recovery_ipc_start_and_poll_round_trip` stay exactly as they are and pass.
- `RecoveryPlan` (`Serialize`, `Deserialize`, `Clone`, `Debug`):

```json
{
  "source_kind": "hrir" | "hesuvi" | "hrir+hesuvi" | "hangloose",
  "source_path": "<path>",
  "output_dir": "<path>",
  "sample_rate": 48000,
  "sample_count": 24000,
  "speakers": ["FL", "FR"],
  "existing_files": ["<path>", ...],
  "planned_files": [
    {"path": "<path>", "kind": "hrir" | "hesuvi" | "hangloose", "channels": 16, "speaker": null | "FL"}
  ],
  "hangloose_dir": null | "<path>"
}
```

  `existing_files` is the same de-duplicated list the result reports. `planned_files` is in write order; `channels` is the number of tracks that would be written (after silent-pair trimming and compaction); `speaker` is set only for Hangloose files; `hangloose_dir` is the directory Hangloose files would go to when the option is on (or where they already are), else `null`.
- `RecoveryResult` is unchanged.

### IPC (`crates/impulcifer-types/src/ipc.rs`, `crates/impulcifer-service/src/lib.rs`, `crates/impulcifer-policy/tests/gates.rs`)

- New method `IpcMethod::PlanOutputRecovery`, wire name `plan_output_recovery`, one positional argument with the same request object and the same validation as `start_output_recovery` (`recovery::validate_request`). It is synchronous and read-only: it never starts a job, never writes, and answers even while another job is running (no `JOB_BUSY`). Success: `{"ok": true, "data": <RecoveryPlan>}`. Failure: the same error object the recovery job would carry (`{"code": <RecoveryErrorCode name>, "message", "details", "retryable": false}`) wrapped in the normal `{"ok": false, "error": ...}` envelope, and the validation errors of `validate_request` unchanged.
- `IpcMethod::ALL` becomes 24 entries; `CANONICAL_IPC` in the policy gate gains `"plan_output_recovery"`.

### Tests

- `impulcifer-service::recovery_plan_matches_the_written_result` (`tests/recovery.rs`): for every scenario the goldens set up, `plan_brir_outputs` returns `planned_files` whose paths equal the `created_files` of the following `recover_brir_outputs` call (same order), `existing_files` equal, `sample_rate`/`sample_count`/`speakers`/`source_kind` equal, and the directory tree (file names and bytes) is unchanged by the plan call; for the error scenarios the plan returns the same `RecoveryError` code as recovery.
- `impulcifer-service::ipc_plan_output_recovery_shape` (`tests/ipc.rs` or `tests/recovery.rs`): keys of the success data; the `NO_RECOVERY_SOURCE` error on an empty folder; the `FILE_NOT_FOUND` validation error on a missing folder; and a plan answered while a job is running (start any job with the fixture's fake, then call plan).

## Part 3. `get_system_info` for the Rust edition

The frozen 2.x shape (`python_version`, `gil_enabled`, `optimal_workers`) is replaced. New shape:

```json
{
  "version": "<CARGO_PKG_VERSION>",
  "install_kind": "velopack" | "pip" | "dev" | ...,
  "os": "<os_description()>",
  "cpu_count": 16 | null,
  "runtime": {
    "language": "Rust",
    "toolchain": "<IMPULCIFER_RUSTC_VERSION, e.g. rustc 1.9x.y>",
    "audio_backend": "<self.backend.name()>",
    "shell": "Tauri 2.11.5",
    "webview": "WebView2 152.0.4191.66"
  },
  "paths": {"data_dir": "<self.data_dir>", "settings_path": "<settings file path>"},
  "update_channel": "prerelease" | "stable"
}
```

`shell` and `webview` are present only when the host supplies them.

- `HostAdapter` gets a provided method `fn shell_info(&self) -> serde_json::Map<String, Value> { Map::new() }`; its entries are merged into `runtime` after the three fixed keys. `TauriHost` (`apps/impulcifer-app/src/main.rs`; you may edit that Rust file, not the `.js` files beside it) returns `shell: format!("Tauri {}", tauri::VERSION)` and `webview: "<engine> <version>"` using the engine label the bootstrap already reports (`edgechromium` is `WebView2`, `cocoa` is `WKWebView`, `gtk` is `WebKitGTK`) and `tauri::webview_version()` (re-exported from `tauri_runtime_wry`; returns `Result<String>`; omit the key when it errs).
- `update_channel` is `prerelease` when `update::check::is_prerelease(env!("CARGO_PKG_VERSION"))`, else `stable`.
- `settings_path` is the path the `Settings` was constructed with (add an accessor).
- Update `ipc_get_system_info_shape` to the new keys (the fixture host supplies no shell info, so `runtime` has exactly `language`, `toolchain`, `audio_backend`).

## Registry (`features.toml`): you edit it for these entries only

- New `recording.share_mode` (kind `recording`, `implemented`) with the Part 1 tests; note: "auto keeps the exclusive-then-shared policy; exclusive/shared never fall back and a refusal is reported as share_mode_refused; backends without selectable modes (cpal) reject a fixed choice as share_mode_unavailable".
- `ipc.bootstrap` tests gain `impulcifer-service::bootstrap_lists_selectable_share_modes`.
- New `ipc.plan_output_recovery` (kind `ipc`, `implemented`) with the Part 2 tests; note: "read-only dry run of output recovery for the Studio recovery screen; same validation and error codes as start_output_recovery".
- `ipc.get_system_info` note: "Rust-edition shape: runtime (language, toolchain, audio backend, shell, webview), paths, update channel; the 2.x python/GIL keys are gone".
- `recording.hardware` tests gain `impulcifer-audio-io::virtual_cable_honours_fixed_share_preferences`.

## Docs (`docs/rust/ARCHITECTURE.md` only, three places)

The IPC method count and list (section 3.2, now 24 with `plan_output_recovery`), the Windows policy sentence in section 3.4 (auto = exclusive first then shared auto-convert; a fixed `share_mode` never falls back), and one sentence in section 7 about the `get_system_info` shape. Nothing else in the docs.

## Allowed files

`crates/impulcifer-types/src/{audio.rs,ipc.rs}`, `crates/impulcifer-audio-io/src/{policy.rs,session.rs,cached_backend.rs,cpal_backend.rs,lib.rs}`, `crates/impulcifer-audio-io/tests/**` (Rust only), `crates/impulcifer-sys-win/src/lib.rs`, `crates/impulcifer-service/src/{lib.rs,recovery.rs,recording/request.rs,recording/run.rs,settings.rs}`, `crates/impulcifer-service/tests/**/*.rs`, `crates/impulcifer-policy/tests/gates.rs` (the `CANONICAL_IPC` array only), `apps/impulcifer-app/src/main.rs`, `features.toml` (the entries listed), `docs/rust/ARCHITECTURE.md` (the three places listed). Nothing else: not `Cargo.toml`/`Cargo.lock`, not `CHANGELOG.md`, not the goldens, not the Python tree, not `webview_ui/`, not `i18n/`, not any file of the ASTRA worker.

## Verification (foreground, paste the output)

```
cd E:/Impulcifer
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types -p impulcifer-audio-io -p impulcifer-sys-win
cargo test -p impulcifer-service
cargo test -p impulcifer-app --test smoke --test packaging
cargo test -p impulcifer-policy
cargo test -p impulcifer-audio-io --test hardware -- --ignored virtual_cable_honours_fixed_share_preferences --nocapture
git status --porcelain
```

## Report format

(1) the three hardware outcomes on CABLE-A (auto, exclusive, shared) and the audio applications that were running; (2) how `plan_brir_outputs` and `recover_brir_outputs` share one preparation path; (3) the exact names of every new test and whether `golden_recording_validation_matches_python` needed the `share` key stripped; (4) the pasted command output with the literal `test result:` lines; (5) `git status --porcelain`; (6) anything undone and any failure that belongs to the ASTRA worker. Do not end your turn before the commands complete.
