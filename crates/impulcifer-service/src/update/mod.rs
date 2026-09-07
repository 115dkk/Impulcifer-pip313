#![forbid(unsafe_code)]

pub mod check;
pub mod install_kind;
mod legacy;
#[cfg(windows)]
pub mod velopack;

use crate::{HostAdapter, args, recording::request::optional_string};
use impulcifer_jobs::registry::{JobContext, JobFailure};
use impulcifer_types::ipc::{self, ErrorCode};
use install_kind::InstallKind;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const RELEASES_URL: &str =
    "https://github.com/115dkk/Impulcifer-pip313/releases/latest/download";
pub const LATEST_ENDPOINT: &str =
    "https://api.github.com/repos/115dkk/Impulcifer-pip313/releases/latest";

#[derive(Clone)]
pub struct UpdateOptions {
    pub install_kind: InstallKind,
    pub platform: String,
    pub current_version: String,
    pub latest_endpoint: String,
    pub releases_url: String,
    pub timeout: Duration,
    pub download_root: PathBuf,
    pub appimage: Option<PathBuf>,
}

impl Default for UpdateOptions {
    fn default() -> Self {
        let platform = crate::platform().to_owned();
        let executable = std::env::current_exe().unwrap_or_default();
        let appimage = std::env::var_os("APPIMAGE").map(PathBuf::from);
        Self {
            install_kind: install_kind::detect(&platform, &executable, appimage.as_deref()),
            platform,
            current_version: env!("CARGO_PKG_VERSION").into(),
            latest_endpoint: LATEST_ENDPOINT.into(),
            releases_url: RELEASES_URL.into(),
            timeout: Duration::from_secs(10),
            download_root: std::env::temp_dir().join("impulcifer_updates"),
            appimage,
        }
    }
}

impl UpdateOptions {
    fn agent(&self) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(self.timeout))
            .build()
            .new_agent()
    }
}

pub struct Request {
    pub latest_version: String,
    pub download_url: String,
}

pub fn validate(request: &Value) -> Result<Request, Value> {
    if !request.is_object() {
        return Err(args::invalid("Request must be an object."));
    }
    Ok(Request {
        latest_version: optional_string(&request["latest_version"])
            .ok_or_else(|| args::invalid("latest_version is required."))?,
        download_url: optional_string(&request["download_url"]).unwrap_or_default(),
    })
}

type Apply = Box<dyn FnOnce() -> Result<(), String> + Send>;
#[derive(Default)]
pub(crate) struct UpdateState {
    pending: Mutex<Option<Apply>>,
}

impl UpdateState {
    pub fn run(
        &self,
        request: Request,
        options: &UpdateOptions,
        host: Arc<dyn HostAdapter>,
        ctx: &JobContext,
    ) -> Result<Value, JobFailure> {
        let progress = |value: f64, message: &str| ctx.progress(value, json!({"message":message}));
        let outcome = (|| -> Result<(Value, Option<Apply>), String> {
            match options.install_kind {
                InstallKind::Tauri => {
                    progress(0.1, "update_downloading");
                    host.download_update(&request.latest_version, &progress)?;
                    progress(0.8, "update_installing");
                    Ok((
                        restart_result(),
                        Some(Box::new(move || host.apply_staged_update())),
                    ))
                }
                #[cfg(windows)]
                InstallKind::Velopack => {
                    let apply = velopack::download(options, &progress)?;
                    Ok((restart_result(), Some(apply)))
                }
                _ => {
                    legacy::execute(&request, options, &*host, &progress)?;
                    Ok((legacy_result(), None))
                }
            }
        })()
        .map_err(|message| JobFailure {
            error: failure(message)["error"].clone(),
            cancelled: false,
        })?;
        *self.pending.lock().unwrap_or_else(|p| p.into_inner()) = outcome.1;
        Ok(outcome.0)
    }

    pub fn apply(&self) -> Result<Value, Value> {
        let apply = self
            .pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .ok_or_else(|| args::invalid("No staged update to apply."))?;
        apply().map_err(failure)?;
        Ok(json!({"restarting":true}))
    }
}

fn failure(message: String) -> Value {
    ipc::error(ErrorCode::UpdateFailed, message, json!({}), true)
}

pub fn restart_result() -> Value {
    json!({
        "status_key":"update_installing", "status_default":"Applying update...",
        "title_key":"update_ready_title", "title_default":"Update Ready",
        "message_key":"update_restart_message",
        "message_default":"The application will close to apply the update.\nIt will restart automatically in a few seconds.",
        "progress":0.9, "requires_restart":true,
    })
}

pub fn legacy_result() -> Value {
    json!({
        "status_key":"update_opening_installer", "status_default":"Opening installer...",
        "title_key":"update_manual_title", "title_default":"Installer Opened",
        "message_key":"update_manual_complete",
        "message_default":"Please follow the installer prompts to complete the update.",
        "progress":1.0, "requires_restart":false,
    })
}
