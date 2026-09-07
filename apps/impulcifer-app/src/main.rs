#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Tauri 2 shell. One command (`pywebview_api`) forwards positional calls
//! from the injected bridge to `ImpulciferService`; dialogs, open path/url and
//! the title-bar theme live in `TauriHost`.

use impulcifer_service::{HostAdapter, ImpulciferService};
use serde_json::Value;
use tauri::{State, WebviewUrl, WebviewWindowBuilder};

struct TauriHost;

impl HostAdapter for TauriHost {
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

struct AppState {
    service: ImpulciferService,
}

#[tauri::command]
fn pywebview_api(state: State<'_, AppState>, method: String, args: Vec<Value>) -> Value {
    state.service.call(&method, args)
}

const BRIDGE_JS: &str = include_str!("bridge.js");

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            service: ImpulciferService::new(Box::new(TauriHost)),
        })
        .invoke_handler(tauri::generate_handler![pywebview_api])
        .setup(|app| {
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
