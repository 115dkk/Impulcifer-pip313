#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Tauri 2 shell. One command (`pywebview_api`) forwards positional calls
//! from the injected bridge to `ImpulciferService`; dialogs, open path/url and
//! the title-bar theme live in `TauriHost`, backed by the dialog and opener
//! plugins and the core window API.

mod dialog;
mod paths;
mod smoke;
mod updater;

use std::sync::Arc;

use impulcifer_service::{HostAdapter, ImpulciferService};
use impulcifer_types::ipc::{self, ErrorCode};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager, State, Theme, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_opener::OpenerExt;

/// Native file-dialog filters, mirroring 2.x `impulcifer_webview._FILE_DIALOG_FILTERS`.
fn dialog_filters(kind: &str) -> (&'static str, &'static [&'static str]) {
    match kind {
        "text" => ("EQ / CSV files", &["csv", "txt"]),
        "wav" => ("WAV files", &["wav"]),
        _ => ("Audio files", &["wav", "mlp", "thd", "truehd"]),
    }
}

fn file_path_to_string(path: FilePath) -> String {
    match path {
        FilePath::Path(path) => path.to_string_lossy().into_owned(),
        FilePath::Url(url) => url.to_string(),
    }
}

/// Maps the persisted UI theme name (2.x vocabulary: "dark", "light", "system")
/// to a native window theme; "system" follows the OS.
fn theme_from_name(name: &str) -> Option<Theme> {
    match name {
        "dark" => Some(Theme::Dark),
        "light" => Some(Theme::Light),
        _ => None,
    }
}

struct TauriHost {
    app: AppHandle,
    update: updater::StagedUpdate,
}

impl HostAdapter for TauriHost {
    fn download_update(
        &self,
        latest_version: &str,
        progress: &(dyn Fn(f64, &str) + Sync),
    ) -> Result<(), String> {
        self.update.download(&self.app, latest_version, progress)
    }

    fn apply_staged_update(&self) -> Result<(), String> {
        self.update.apply(&self.app)
    }

    fn select_file(&self, kind: &str) -> Option<String> {
        let (name, extensions) = dialog_filters(kind);
        self.app
            .dialog()
            .file()
            .add_filter(name, extensions)
            .add_filter("All files", &["*"])
            .blocking_pick_file()
            .map(file_path_to_string)
    }

    fn select_directory(&self) -> Option<String> {
        self.app
            .dialog()
            .file()
            .blocking_pick_folder()
            .map(file_path_to_string)
    }

    fn open_path(&self, path: &str) -> Result<(), String> {
        self.app
            .opener()
            .open_path(path, None::<&str>)
            .map_err(|err| err.to_string())
    }

    fn open_url(&self, url: &str) -> Result<(), String> {
        self.app
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|err| err.to_string())
    }

    fn apply_title_theme(&self, theme: &str) {
        if let Some(window) = self.app.get_webview_window("main") {
            let _ = window.set_theme(theme_from_name(theme));
        }
    }
}

struct AppState {
    service: Arc<ImpulciferService>,
    smoke: Option<Arc<smoke::SmokeConfig>>,
}

/// The single IPC entry point. It is `async` so Tauri runs it off the main
/// thread; the service call (which may open a blocking native dialog) runs on
/// the blocking pool. The command never rejects: failures become envelopes.
#[tauri::command]
async fn pywebview_api(
    state: State<'_, AppState>,
    method: String,
    args: Vec<Value>,
) -> Result<Value, String> {
    let service = Arc::clone(&state.service);
    let smoke = state.smoke.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        smoke::dispatch(smoke.as_deref(), &method, args, |method, args| {
            service.call(method, args)
        })
    })
    .await;
    Ok(outcome.unwrap_or_else(|err| {
        ipc::error(
            ErrorCode::InternalError,
            format!("command worker failed: {err}"),
            json!({}),
            false,
        )
    }))
}

const BRIDGE_JS: &str = include_str!("bridge.js");

fn main() {
    #[cfg(windows)]
    velopack::VelopackApp::build().run();
    let smoke = smoke::SmokeConfig::from_environment()
        .expect("invalid smoke startup configuration")
        .map(Arc::new);
    let builder = tauri::Builder::default();
    // Windows installs use only Velopack, never the Tauri updater.
    #[cfg(not(windows))]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    builder
        .plugin(dialog::NativeDialogs::new())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![pywebview_api])
        .setup(move |app| {
            let host = TauriHost {
                app: app.handle().clone(),
                update: updater::StagedUpdate::default(),
            };
            let data_dir = paths::bundled_data_dir(
                app.path().resource_dir().ok(),
                std::env::var_os("IMPULCIFER_DATA_DIR").is_some(),
            )
            .unwrap_or_else(impulcifer_service::default_data_dir);
            app.manage(AppState {
                service: Arc::new(ImpulciferService::with_dependencies(
                    Box::new(host),
                    impulcifer_service::settings::default_path(),
                    impulcifer_audio_io::default_backend(),
                    impulcifer_jobs::registry::JobRegistry::new(),
                    data_dir,
                )),
                smoke: smoke.clone(),
            });
            let mut builder =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    // The frozen HTML references sibling logo/ assets outside frontendDist.
                    // Serve the original embedded bytes at those existing URLs.
                    .on_web_resource_request(|request, response| {
                        let logo: Option<&'static [u8]> = match request.uri().path() {
                            "/logo/pulse-32.png" => Some(include_bytes!("../../../logo/pulse-32.png")),
                            "/logo/pulse-128.png" => Some(include_bytes!("../../../logo/pulse-128.png")),
                            _ => None,
                        };
                        if let Some(bytes) = logo {
                            *response.status_mut() = tauri::http::StatusCode::OK;
                            *response.body_mut() = std::borrow::Cow::Borrowed(bytes);
                            response.headers_mut().insert(tauri::http::header::CONTENT_TYPE, tauri::http::HeaderValue::from_static("image/png"));
                            response.headers_mut().remove(tauri::http::header::CONTENT_LENGTH);
                        }
                    })
                    .title("Impulcifer")
                    .inner_size(1180.0, 820.0)
                    .devtools(cfg!(debug_assertions) || smoke.is_some());
            if let Some(config) = smoke.as_deref() {
                builder = builder.initialization_script(include_str!("smoke-observer.js"));
                if let Some(directory) = &config.data_directory {
                    builder = builder.data_directory(directory.clone());
                }
                #[cfg(windows)]
                if let Some(port) = config.cdp_port {
                    builder = builder.additional_browser_args(&format!(
                        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port={port}"
                    ));
                }
            }
            builder = builder.initialization_script(BRIDGE_JS);
            if let Some(driver) = smoke.as_ref().and_then(|config| config.driver.as_ref()) {
                builder = builder.initialization_script(driver);
            }
            let window = builder.build()?;
            // Apply the persisted theme before the first frame so the native
            // title bar matches the page; 2.x did the same at startup through
            // its DWM workaround. Later `set_theme` IPC calls go through TauriHost.
            let settings = app
                .state::<AppState>()
                .service
                .call("get_ui_settings", Vec::new());
            let theme = settings["data"]["theme"].as_str().unwrap_or("dark");
            if smoke.is_some() {
                eprintln!("app smoke: settings loaded, applying theme");
            }
            let _ = window.set_theme(theme_from_name(theme));
            if smoke.is_some() {
                eprintln!("app smoke: setup completed");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Impulcifer");
}
