//! Android's share of the shell (ADR 0020).
//!
//! Rust reaches the platform only through the Kotlin plugin
//! `ImpulciferPlugin` (`gen/android/app/src/main/java/dev/impulcifer/app/`):
//! whether the app holds all-files access and the screen that grants it, the
//! shared storage volumes, the device and Android version, the colour of the
//! system bars, and opening a folder in the system file manager. Every call
//! blocks until the activity answers, so none may run on the activity's own
//! thread; the IPC command runs on the blocking pool and `setup` on the event
//! loop's thread, which are both fine.

use impulcifer_service::StorageRoot;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use std::path::PathBuf;
use tauri::{
    AppHandle, Manager, Wry,
    plugin::{Builder, PluginHandle, TauriPlugin},
};

/// The application id, which is also the Kotlin package of the plugin.
const PACKAGE: &str = "dev.impulcifer.app";
const PLUGIN_CLASS: &str = "ImpulciferPlugin";

/// The default test signal, the one bundled file processing reads by path
/// (`impulcifer_service::brir::estimator::DEFAULT_SIGNAL`). The segmented
/// sweeps are for playing a recording and the Harman targets are for people to
/// pick from the desktop's data folder; neither has a use in an app's private
/// folder.
pub const BUNDLED_DATA: &[(&str, &[u8])] = &[(
    "sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav",
    include_bytes!("../../../data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"),
)];

pub struct Bridge(PluginHandle<Wry>);

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new("impulcifer-android")
        .setup(|app, api| {
            let handle = api.register_android_plugin(PACKAGE, PLUGIN_CLASS)?;
            app.manage(Bridge(handle));
            Ok(())
        })
        .build()
}

/// The bridge registered by [`plugin`]. The plugin is added before `setup`
/// runs, so the state is there for every caller in this crate.
pub fn bridge(app: &AppHandle) -> tauri::State<'_, Bridge> {
    app.state::<Bridge>()
}

/// The settings file and the data folder, both inside the app's own storage.
pub fn private_paths(app: &tauri::App) -> (PathBuf, PathBuf) {
    let config = app
        .path()
        .app_config_dir()
        .expect("Android reports the app's config folder");
    let data = app
        .path()
        .app_data_dir()
        .expect("Android reports the app's data folder");
    (config.join("settings.json"), data.join("data"))
}

#[derive(Deserialize)]
struct Device {
    release: String,
    sdk: u32,
    manufacturer: String,
    model: String,
}

#[derive(Deserialize)]
struct Access {
    granted: bool,
}

#[derive(Deserialize)]
struct Opened {
    opened: bool,
}

#[derive(Deserialize)]
struct Appearance {
    dark: bool,
}

#[derive(Deserialize)]
struct Volumes {
    volumes: Vec<Volume>,
}

#[derive(Deserialize)]
struct Volume {
    path: String,
    label: String,
    removable: bool,
}

/// `Samsung SM-S948N`, without repeating a maker the model already starts with.
fn device_name(manufacturer: &str, model: &str) -> String {
    let (manufacturer, model) = (manufacturer.trim(), model.trim());
    if manufacturer.is_empty()
        || model
            .to_lowercase()
            .starts_with(&manufacturer.to_lowercase())
    {
        return model.to_owned();
    }
    let mut maker = manufacturer.chars();
    let maker = match maker.next() {
        Some(first) => first.to_uppercase().chain(maker).collect::<String>(),
        None => String::new(),
    };
    format!("{maker} {model}").trim().to_owned()
}

impl Bridge {
    fn call<T: DeserializeOwned>(&self, command: &str, payload: Value) -> Result<T, String> {
        self.0
            .run_mobile_plugin(command, payload)
            .map_err(|error| error.to_string())
    }

    /// The OS line and the device row of the system information.
    pub fn platform_info(&self) -> Map<String, Value> {
        let mut info = Map::new();
        if let Ok(device) = self.call::<Device>("device", json!({})) {
            info.insert(
                "os".into(),
                json!(format!(
                    "Android {} (API {}) {}",
                    device.release,
                    device.sdk,
                    std::env::consts::ARCH
                )),
            );
            let name = device_name(&device.manufacturer, &device.model);
            if !name.is_empty() {
                info.insert("device".into(), json!(name));
            }
        }
        info
    }

    /// A failed question counts as "not granted": the page then shows the
    /// grant screen instead of an empty folder list.
    pub fn storage_access(&self) -> bool {
        self.call::<Access>("storageAccess", json!({}))
            .is_ok_and(|access| access.granted)
    }

    pub fn request_storage_access(&self) -> Result<bool, String> {
        self.call::<Opened>("requestStorageAccess", json!({}))
            .map(|answer| answer.opened)
    }

    pub fn storage_roots(&self) -> Vec<StorageRoot> {
        self.call::<Volumes>("storageVolumes", json!({}))
            .map(|answer| {
                answer
                    .volumes
                    .into_iter()
                    .map(|volume| StorageRoot {
                        path: volume.path,
                        label: volume.label,
                        removable: volume.removable,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn open_folder(&self, path: &str) -> Result<(), String> {
        let answer = self.call::<Opened>("openFolder", json!({ "path": path }))?;
        if answer.opened {
            Ok(())
        } else {
            Err("No app on this device opens folders.".into())
        }
    }

    /// Whether the system is in its dark theme, for the page's "system" theme
    /// before the page itself can ask. An unanswered question is light, as on
    /// the desktop.
    pub fn system_is_dark(&self) -> bool {
        self.call::<Appearance>("systemAppearance", json!({}))
            .is_ok_and(|appearance| appearance.dark)
    }

    /// Paints the area behind the system bars with the page's background and
    /// picks bar icons that show on it. `theme` is the saved setting; Kotlin
    /// resolves "system".
    pub fn apply_theme(&self, theme: &str) {
        let _ = self.call::<Value>("applyTheme", json!({ "theme": theme }));
    }
}

#[cfg(test)]
mod tests {
    use super::device_name;

    #[test]
    fn device_name_does_not_repeat_the_maker() {
        assert_eq!(device_name("samsung", "SM-S948N"), "Samsung SM-S948N");
        assert_eq!(device_name("Google", "Pixel 9"), "Google Pixel 9");
        assert_eq!(device_name("OnePlus", "OnePlus 13"), "OnePlus 13");
        assert_eq!(
            device_name("", "sdk_gphone64_x86_64"),
            "sdk_gphone64_x86_64"
        );
        assert_eq!(device_name("unknown", ""), "Unknown");
    }
}
