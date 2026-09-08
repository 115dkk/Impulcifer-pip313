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
use tauri::{AppHandle, Manager, State, Theme, WebviewUrl, WebviewWindowBuilder, window::Color};
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

/// The OS colour preference, as far as the platform reports it.
fn system_mode() -> dark_light::Mode {
    dark_light::detect().unwrap_or(dark_light::Mode::Unspecified)
}

/// Resolves the saved theme name to the theme the page will actually show:
/// "system" follows the OS preference, exactly as webview_ui/app.js resolves
/// `prefers-color-scheme` (an unspecified OS answer is light, the browser default).
fn effective_theme(setting: &str, system: dark_light::Mode) -> &'static str {
    match setting {
        "dark" => "dark",
        "light" => "light",
        _ => match system {
            dark_light::Mode::Dark => "dark",
            _ => "light",
        },
    }
}

const STABLE_FEED: &str =
    "https://github.com/115dkk/Impulcifer-pip313/releases/latest/download/latest.json";
const PRERELEASE_FEED: &str =
    "https://github.com/115dkk/Impulcifer-pip313/releases/download/updater-3x-pre/latest.json";

/// Tauri updater endpoints for this build: a prerelease install reads the rolling
/// prerelease feed first (GitHub's `/releases/latest` never resolves a
/// prerelease, so alpha-to-alpha updates would be invisible), a stable install
/// reads the stable feed only.
#[cfg_attr(windows, allow(dead_code))]
fn updater_endpoints(version: &str) -> Vec<&'static str> {
    if impulcifer_service::update::check::is_prerelease(version) {
        vec![PRERELEASE_FEED, STABLE_FEED]
    } else {
        vec![STABLE_FEED]
    }
}

/// The Pulse `--bg-0` tokens of webview_ui/styles.css, painted as the window's
/// own background so the pre-load flash matches the page (2.x
/// `_WINDOW_BACKGROUNDS`). `theme` is the resolved theme (see `effective_theme`).
fn window_background(theme: &str) -> Color {
    if theme == "light" {
        Color(0xf3, 0xf5, 0xf7, 255)
    } else {
        Color(0x10, 0x12, 0x14, 255)
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
    let builder = builder.plugin(
        tauri_plugin_updater::Builder::new()
            .endpoints(
                updater_endpoints(env!("CARGO_PKG_VERSION"))
                    .iter()
                    .map(|endpoint| endpoint.parse().expect("updater endpoint URL"))
                    .collect(),
            )
            .expect("updater endpoints")
            .build(),
    );
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
            // Read the persisted theme before the window exists: the native
            // title bar and the pre-load background must match the page from
            // the first frame. 2.x applied its DWM dark-title-bar workaround
            // before show and painted the window with the Pulse background.
            let setting = app.state::<AppState>().service.call("get_ui_settings", Vec::new())
                ["data"]["theme"]
                .as_str()
                .unwrap_or("dark")
                .to_owned();
            // "system" is resolved against the OS here so the pre-load background
            // matches the page on a light OS too; the native title bar keeps
            // following the OS on its own (theme None).
            let theme = effective_theme(&setting, system_mode());
            let mut builder =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .theme(theme_from_name(&setting))
                    .background_color(window_background(theme))
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
            // The builder already applied the theme; repeat it on the live
            // window for runtimes that only honour it after creation. Later
            // `set_theme` IPC calls go through TauriHost.
            if smoke.is_some() {
                eprintln!("app smoke: settings loaded, applying theme");
            }
            let _ = window.set_theme(theme_from_name(&setting));
            if smoke.is_some() {
                eprintln!("app smoke: setup completed");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Impulcifer");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prerelease_installs_read_the_rolling_feed_first() {
        assert_eq!(
            updater_endpoints("3.0.0-alpha.0"),
            vec![PRERELEASE_FEED, STABLE_FEED]
        );
        assert_eq!(updater_endpoints("3.0.0"), vec![STABLE_FEED]);
        for endpoint in [PRERELEASE_FEED, STABLE_FEED] {
            assert!(endpoint.starts_with("https://github.com/115dkk/Impulcifer-pip313/releases/"));
            assert!(endpoint.ends_with("/latest.json"));
            let _: tauri::Url = endpoint.parse().unwrap();
        }
    }

    #[test]
    fn system_theme_resolves_to_the_os_preference() {
        use dark_light::Mode;
        assert_eq!(effective_theme("dark", Mode::Light), "dark");
        assert_eq!(effective_theme("light", Mode::Dark), "light");
        assert_eq!(effective_theme("system", Mode::Dark), "dark");
        assert_eq!(effective_theme("system", Mode::Light), "light");
        assert_eq!(effective_theme("system", Mode::Unspecified), "light");
        assert_eq!(window_background("light"), Color(0xf3, 0xf5, 0xf7, 255));
        assert_eq!(window_background("dark"), Color(0x10, 0x12, 0x14, 255));
    }
}
