#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Tauri 2 shell. One command (`pywebview_api`) forwards positional calls
//! from the injected bridge to `ImpulciferService`; dialogs, open path/url and
//! the title-bar theme live in `TauriHost`, backed by the dialog and opener
//! plugins and the core window API.

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

struct TauriHost {
    app: AppHandle,
}

impl HostAdapter for TauriHost {
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
        let theme = match theme {
            "dark" => Some(Theme::Dark),
            "light" => Some(Theme::Light),
            _ => None,
        };
        if let Some(window) = self.app.get_webview_window("main") {
            let _ = window.set_theme(theme);
        }
    }
}

struct AppState {
    service: Arc<ImpulciferService>,
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
    let outcome = tauri::async_runtime::spawn_blocking(move || service.call(&method, args)).await;
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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![pywebview_api])
        .setup(|app| {
            let host = TauriHost {
                app: app.handle().clone(),
            };
            app.manage(AppState {
                service: Arc::new(ImpulciferService::new(Box::new(host))),
            });
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("Impulcifer")
                .inner_size(1180.0, 820.0)
                .initialization_script(BRIDGE_JS)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Impulcifer");
}
