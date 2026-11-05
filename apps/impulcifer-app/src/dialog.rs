//! Keep the dialog plugin's Rust adapter, without its async window.confirm shim.
//! The frozen pywebview UI expects the browser's synchronous boolean confirm.
//! On macOS, WKWebView has no confirm or alert panel (wry sets no UI delegate for
//! them), so `macos-dialogs.js` backs both with the plugin's `message` command;
//! ui/app.js awaits confirm, which keeps the synchronous boolean working elsewhere.
use serde_json::Value;
use tauri::{
    AppHandle, Runtime,
    ipc::Invoke,
    plugin::{Plugin, TauriPlugin},
};

pub struct NativeDialogs<R: Runtime>(TauriPlugin<R>);

impl<R: Runtime> NativeDialogs<R> {
    pub fn new() -> Self {
        Self(tauri_plugin_dialog::init())
    }
}

impl<R: Runtime> Plugin<R> for NativeDialogs<R> {
    fn name(&self) -> &'static str {
        self.0.name()
    }
    fn initialize(
        &mut self,
        app: &AppHandle<R>,
        config: Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.0.initialize(app, config)
    }
    fn extend_api(&mut self, invoke: Invoke<R>) -> bool {
        self.0.extend_api(invoke)
    }
    // The upstream script is never forwarded: it makes confirm async on every
    // platform and still invokes `plugin:dialog|confirm`, which 2.7 removed.
    // macOS gets its own script because WKWebView has no native confirm/alert.
    fn initialization_script(&self) -> Option<String> {
        cfg!(target_os = "macos").then(|| MACOS_DIALOGS_JS.to_owned())
    }
}

/// window.confirm/alert for WKWebView, backed by the plugin's `message` command.
const MACOS_DIALOGS_JS: &str = include_str!("macos-dialogs.js");

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_confirmation_contract() {
        let plugin = NativeDialogs::<tauri::Wry>::new();
        assert_eq!(plugin.name(), "dialog");
        let script = plugin.initialization_script_2();
        assert_eq!(script.is_some(), cfg!(target_os = "macos"));
        if let Some(script) = script {
            assert!(script.for_main_frame_only);
            assert_eq!(script.script, MACOS_DIALOGS_JS);
        }
    }

    #[test]
    fn macos_dialogs_use_the_message_command() {
        // The plugin has no `confirm` command since 2.7; `message` with OkCancel
        // answers "Ok" or "Cancel", as the plugin's own JS confirm() reads it.
        assert!(MACOS_DIALOGS_JS.contains(r#"invoke("plugin:dialog|message""#));
        assert!(MACOS_DIALOGS_JS.contains(r#"buttons: "OkCancel""#));
        assert!(MACOS_DIALOGS_JS.contains(r#"=== "Ok""#));
        assert!(!MACOS_DIALOGS_JS.contains("plugin:dialog|confirm"));
    }
}
