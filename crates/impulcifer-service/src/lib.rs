#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! The frozen pywebview IPC surface. Host functions are injected; panics are
//! caught at the outer call boundary, including host and backend panics.

mod args;
pub mod brir;
mod paths;
pub mod recording;
mod settings;

use args::Args;
use impulcifer_jobs::registry::{JobRegistry, panic_message};
use impulcifer_types::audio::AudioBackend;
use impulcifer_types::config::ProcessingConfig;
use impulcifer_types::constants::{SPEAKER_NAMES, SWEEP_TRACK_LAYOUTS};
use impulcifer_types::ipc::{self, ErrorCode, IpcMethod};
use impulcifer_types::job::JobSnapshot;
use serde_json::{Value, json};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

pub trait HostAdapter: Send + Sync {
    fn select_file(&self, kind: &str) -> Option<String>;
    fn select_directory(&self) -> Option<String>;
    fn open_path(&self, path: &str) -> Result<(), String>;
    fn open_url(&self, url: &str) -> Result<(), String>;
    fn apply_title_theme(&self, theme: &str);
}

/// Host adapter for headless contexts (CLI, tests): dialogs return None.
pub struct NoopHost;
impl HostAdapter for NoopHost {
    fn select_file(&self, _kind: &str) -> Option<String> {
        None
    }
    fn select_directory(&self) -> Option<String> {
        None
    }
    fn open_path(&self, _path: &str) -> Result<(), String> {
        Ok(())
    }
    fn open_url(&self, _url: &str) -> Result<(), String> {
        Ok(())
    }
    fn apply_title_theme(&self, _theme: &str) {}
}

pub struct ImpulciferService {
    host: Box<dyn HostAdapter>,
    settings: Mutex<settings::Settings>,
    backend: Arc<dyn AudioBackend>,
    jobs: JobRegistry,
    data_dir: PathBuf,
}

impl ImpulciferService {
    pub fn new(host: Box<dyn HostAdapter>) -> Self {
        Self::with_dependencies(
            host,
            settings::default_path(),
            impulcifer_audio_io::default_backend(),
            JobRegistry::new(),
            default_data_dir(),
        )
    }

    /// Tests and installed shells can inject paths without mutating HOME or
    /// touching real devices. Settings are loaded lazily on the first UI call.
    pub fn with_dependencies(
        host: Box<dyn HostAdapter>,
        settings_path: PathBuf,
        backend: Box<dyn AudioBackend>,
        jobs: JobRegistry,
        data_dir: PathBuf,
    ) -> Self {
        Self {
            host,
            settings: Mutex::new(settings::Settings::new(settings_path)),
            backend: Arc::from(backend),
            jobs,
            data_dir,
        }
    }

    pub fn call(&self, method: &str, args: Vec<Value>) -> Value {
        catch_unwind(AssertUnwindSafe(|| self.dispatch(method, Args(args))))
            .unwrap_or_else(|panic| internal(panic_message(&*panic)))
    }

    fn settings(&self) -> MutexGuard<'_, settings::Settings> {
        self.settings
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn dispatch(&self, name: &str, args: Args) -> Value {
        let Some(method) = IpcMethod::from_wire_name(name) else {
            return args::invalid(format!("unknown method: {name}"));
        };
        self.execute(method, args)
            .map(ipc::ok)
            .unwrap_or_else(|error| error)
    }

    fn execute(&self, method: IpcMethod, args: Args) -> Result<Value, Value> {
        match method {
            IpcMethod::Bootstrap => {
                args.count(0, 0)?;
                let mut defaults = serde_json::to_value(ProcessingConfig::default())
                    .map_err(|error| internal(error.to_string()))?;
                defaults
                    .as_object_mut()
                    .expect("config is an object")
                    .remove("dir_path");
                Ok(json!({
                    "version": env!("CARGO_PKG_VERSION"), "platform": platform(),
                    "brir_defaults": defaults,
                    "sweep": {"layouts": SWEEP_TRACK_LAYOUTS, "default_fs": paths::DEFAULT_SWEEP_FS,
                        "default_duration": paths::DEFAULT_SWEEP_DURATION, "speaker_names": SPEAKER_NAMES},
                    "capabilities": {"recording":true,"brir":true,"output_recovery":true,
                        "recording_cancel":false,"brir_cancel":true,"output_recovery_cancel":false},
                    "active_job": self.jobs.active().as_ref().map(snapshot),
                    "ui": self.settings().payload(),
                    "webview_backend": match platform() { "windows" => "edgechromium", "darwin" => "cocoa", _ => "gtk" },
                }))
            }
            IpcMethod::GetUiSettings => {
                args.count(0, 0)?;
                Ok(self.settings().payload())
            }
            IpcMethod::SetLanguage
            | IpcMethod::SetTheme
            | IpcMethod::SetSkin
            | IpcMethod::SetFrontend => {
                args.count(1, 1)?;
                let (key, choices, message) = match method {
                    IpcMethod::SetLanguage => (
                        "language",
                        settings::LANGUAGES
                            .iter()
                            .map(|(code, _)| *code)
                            .collect::<Vec<_>>(),
                        "Unsupported language code.",
                    ),
                    IpcMethod::SetTheme => (
                        "theme",
                        vec!["dark", "light", "system"],
                        "Theme must be one of: dark, light, system.",
                    ),
                    IpcMethod::SetSkin => (
                        "skin",
                        vec!["stable", "studio"],
                        "Skin must be one of: stable, studio.",
                    ),
                    _ => (
                        "frontend",
                        vec!["webview", "ctk"],
                        "Frontend must be one of: webview, ctk.",
                    ),
                };
                let value = args.string(0, key, None)?;
                if !choices.contains(&value.as_str()) {
                    return Err(ipc::error(
                        ErrorCode::InvalidRequest,
                        message,
                        json!({key: value}),
                        false,
                    ));
                }
                self.settings().set(key, &value);
                if method == IpcMethod::SetTheme {
                    self.host.apply_title_theme(&value);
                }
                if method == IpcMethod::SetLanguage {
                    Ok(self.settings().payload())
                } else {
                    Ok(json!({key: value}))
                }
            }
            IpcMethod::GetSystemInfo => {
                args.count(0, 0)?;
                let cpus = std::thread::available_parallelism().map(usize::from).ok();
                // Frozen JS still prints PYTHON and GIL labels. Preserve its keys
                // with an honest Rust value and null GIL; no translated labels invented.
                Ok(
                    json!({"version":env!("CARGO_PKG_VERSION"), "install_kind":"dev",
                    "python_version":env!("IMPULCIFER_RUSTC_VERSION"), "os":os_description(),
                    "cpu_count":cpus,"gil_enabled":null,"optimal_workers":cpus}),
                )
            }
            IpcMethod::ListAudioDevices => {
                args.count(0, 1)?;
                let filter = args.optional_string(0, "host_api")?;
                recording::devices::list_audio_devices(&*self.backend, filter.as_deref())
            }
            IpcMethod::StartRecording => {
                args.count(1, 1)?;
                let request = recording::request::validate(args.get(0))?;
                let backend = Arc::clone(&self.backend);
                let job = self
                    .jobs
                    .start(
                        impulcifer_types::job::JobKind::Recording,
                        false,
                        move |ctx| recording::run::run_recording(&request, &*backend, ctx),
                    )
                    .map_err(|code| {
                        ipc::error(
                            code,
                            "Another job is already running.",
                            json!({"job":self.jobs.active().as_ref().map(snapshot)}),
                            code == ErrorCode::JobBusy,
                        )
                    })?;
                Ok(json!({"job":snapshot(&job)}))
            }
            IpcMethod::PollJob => {
                args.count(1, 2)?;
                let id = args.job_id()?;
                let poll = self
                    .jobs
                    .poll(&id, args.after_seq()?)
                    .map_err(|code| self.job_error(code, &id))?;
                let events: Vec<_> = poll.events.iter().map(|event| json!({"seq":event.seq,"timestamp_ms":poll.timestamps_ms[&event.seq],"type":event.kind,"payload":event.payload})).collect();
                Ok(json!({"job":snapshot(&poll.job),"events":events,"next_seq":poll.next_seq}))
            }
            IpcMethod::CancelJob => {
                args.count(1, 1)?;
                let id = args.job_id()?;
                let job = self
                    .jobs
                    .cancel(&id)
                    .map_err(|code| self.job_error(code, &id))?;
                Ok(json!({"job":snapshot(&job)}))
            }
            IpcMethod::StartBrir => {
                args.count(1, 1)?;
                let request = brir::validation::validate(args.get(0))?;
                let ui = self.settings().payload();
                let catalog = brir::Catalog::from_strings(
                    ui["strings"].as_object().cloned().unwrap_or_default(),
                );
                let data = self.data_dir.clone();
                let job = self
                    .jobs
                    .start(impulcifer_types::job::JobKind::Brir, true, move |ctx| {
                        ctx.check_cancelled()?;
                        if request.config.do_equalization {
                            for (source, target) in request.sidecars {
                                ctx.check_cancelled()?;
                                std::fs::copy(source, target)
                                    .map_err(|e| brir::BrirError::Fs(e).failure())?;
                            }
                        }
                        let run = brir::run::run_with_data(&request.config, &catalog, ctx, &data)?;
                        Ok(json!({"output_path":run.output_path}))
                    })
                    .map_err(|code| {
                        ipc::error(
                            code,
                            "Another job is already running.",
                            json!({"job":self.jobs.active().as_ref().map(snapshot)}),
                            code == ErrorCode::JobBusy,
                        )
                    })?;
                Ok(json!({"job":snapshot(&job)}))
            }
            IpcMethod::DetectSweep | IpcMethod::GenerateSweepSet => {
                args.count(1, 1)?;
                let raw = paths::text(&args, 0, "dir_path")?;
                let path = std::path::Path::new(raw.as_deref().unwrap_or(""));
                if !path.is_dir() {
                    return Err(ipc::error(
                        ErrorCode::FileNotFound,
                        "Measurement directory does not exist.",
                        json!({"path":args.get(0)}),
                        false,
                    ));
                }
                if method == IpcMethod::GenerateSweepSet {
                    brir::estimator::generate_sweep_set(path).map_err(|e| e.envelope())
                } else {
                    let detection = brir::sweep_grid::detect_sweep_parameters(path)
                        .map_err(|e| e.envelope())?;
                    let sidecar = path.join("test.wav").is_file();
                    Ok(detection.map_or_else(
                        || json!({"found":false,"sidecar":sidecar}),
                        |d| d.payload(sidecar),
                    ))
                }
            }
            IpcMethod::ResolveRecordingPaths => paths::resolve(&args),
            IpcMethod::OpenPath => {
                args.count(0, 1)?;
                let raw = match args.optional_string(0, "path")? {
                    None => json!(self.data_dir.to_string_lossy()),
                    Some(path) => json!(path),
                };
                let target = raw.as_str().map(str::trim).filter(|s| !s.is_empty());
                let target = target
                    .filter(|path| std::path::Path::new(path).is_dir())
                    .ok_or_else(|| {
                        ipc::error(
                            ErrorCode::FileNotFound,
                            "Folder does not exist.",
                            json!({"path":raw}),
                            false,
                        )
                    })?;
                self.host.open_path(target).map_err(internal)?;
                Ok(json!({"path":target}))
            }
            IpcMethod::OpenUrl => {
                args.count(1, 1)?;
                let name = args.string(0, "name", None)?;
                let url = match name.as_str() {
                    "original_repo" => "https://github.com/jaakkopasanen/Impulcifer",
                    "fork_repo" => "https://github.com/115dkk/Impulcifer-pip313",
                    "report_bug" => "https://github.com/115dkk/Impulcifer-pip313/issues/new",
                    "license" => "https://github.com/115dkk/Impulcifer-pip313/blob/master/LICENSE",
                    _ => return Err(args::invalid("Unknown project link.")),
                };
                self.host.open_url(url).map_err(internal)?;
                Ok(json!({"url":url}))
            }
            IpcMethod::SelectFile => {
                args.count(0, 1)?;
                let kind = args
                    .optional_string(0, "kind")?
                    .unwrap_or_else(|| "audio".into());
                let kind = if ["audio", "text", "wav"].contains(&kind.as_str()) {
                    &kind
                } else {
                    "audio"
                };
                Ok(json!({"path":self.host.select_file(kind)}))
            }
            IpcMethod::SelectDirectory => {
                args.count(0, 0)?;
                Ok(json!({"path":self.host.select_directory()}))
            }
            _ => Err(internal(format!("{} not implemented", method.wire_name()))),
        }
    }

    fn job_error(&self, code: ErrorCode, id: &str) -> Value {
        let message = if code == ErrorCode::JobNotCancellable {
            let kind = self.jobs.poll(id, 0).ok().map(|poll| json!(poll.job.kind));
            format!(
                "{} jobs cannot be cancelled safely.",
                kind.as_ref().and_then(Value::as_str).unwrap_or("recording")
            )
        } else {
            "Job not found.".into()
        };
        ipc::error(code, message, json!({"job_id":id}), false)
    }
}

fn snapshot(job: &JobSnapshot) -> Value {
    json!({"job_id":job.job_id,"kind":job.kind,"status":job.status,"cancellable":job.cancellable,"result":job.result,"error":job.error})
}
fn internal(message: impl Into<String>) -> Value {
    ipc::error(ErrorCode::InternalError, message, json!({}), false)
}
fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else {
        std::env::consts::OS
    }
}
fn os_description() -> String {
    #[cfg(windows)]
    let output = std::process::Command::new("cmd.exe")
        .args(["/D", "/C", "ver"])
        .output();
    #[cfg(not(windows))]
    let output = std::process::Command::new("uname")
        .args(["-s", "-r"])
        .output();
    output
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| platform().to_owned())
}
fn default_data_dir() -> PathBuf {
    // Candidates in priority order: an explicit override, executable-adjacent
    // resources (installed builds; the Tauri bundle will place `data` there or
    // under `resources/`), then the checkout-root data directory. Only an
    // existing directory is returned so `open_path` never reports a path that
    // was merely assumed; when none exists the first candidate is returned and
    // `open_path` answers FILE_NOT_FOUND with that path.
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::var_os("IMPULCIFER_DATA_DIR") {
        candidates.push(PathBuf::from(dir));
    }
    if let Some(parent) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
    {
        candidates.push(parent.join("data"));
        candidates.push(parent.join("resources").join("data"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"));
    pick_data_dir(&candidates).unwrap_or_else(|| {
        candidates
            .first()
            .cloned()
            .unwrap_or_else(|| PathBuf::from("data"))
    })
}

/// First candidate that is an existing directory.
fn pick_data_dir(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|path| path.is_dir()).cloned()
}

#[cfg(test)]
mod data_dir_tests {
    use super::pick_data_dir;

    #[test]
    fn data_dir_picks_first_existing_candidate() {
        let unique = format!("impulcifer-data-dir-{}", std::process::id());
        let existing = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&existing).unwrap();
        let missing = existing.join("does-not-exist");
        let picked = pick_data_dir(&[missing.clone(), existing.clone()]);
        assert_eq!(picked, Some(existing.clone()));
        assert_eq!(pick_data_dir(std::slice::from_ref(&missing)), None);
        assert_eq!(pick_data_dir(&[]), None);
        std::fs::remove_dir_all(&existing).unwrap();
    }
}
