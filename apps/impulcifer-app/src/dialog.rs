//! Keep the dialog plugin's Rust adapter, without its async window.confirm shim.
//! The frozen pywebview UI expects the browser's synchronous boolean confirm.
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
    // Deliberately retain the default (no initialization script). The upstream
    // dialog plugin uses only setup and invoke hooks, which are forwarded above.
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_confirmation_contract() {
        let plugin = NativeDialogs::<tauri::Wry>::new();
        assert_eq!(plugin.name(), "dialog");
        assert!(plugin.initialization_script_2().is_none());
    }
}
