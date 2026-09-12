#![forbid(unsafe_code)]

use tauri_plugin_updater::UpdaterExt;

#[derive(Default)]
pub struct StagedUpdate {
    pending: std::sync::Mutex<Option<(tauri_plugin_updater::Update, Vec<u8>)>>,
}

impl StagedUpdate {
    pub fn download(
        &self,
        app: &tauri::AppHandle,
        latest_version: &str,
        progress: &(dyn Fn(f64, &str) + Sync),
    ) -> Result<(), String> {
        // Keep the entire adapter type-checked on Windows too, but never run
        // the plugin there: only Velopack handles an installed Windows app.
        if cfg!(windows) {
            return Err("not supported".into());
        }
        let (update, bytes) = tauri::async_runtime::block_on(async {
            let endpoints = crate::updater_endpoints(env!("CARGO_PKG_VERSION"))
                .iter()
                .map(|endpoint| {
                    endpoint
                        .parse::<tauri::Url>()
                        .map_err(|error| format!("updater endpoint {endpoint}: {error}"))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let updater = app
                .updater_builder()
                .endpoints(endpoints)
                .map_err(|e| e.to_string())?
                .build()
                .map_err(|e| e.to_string())?;
            let update = updater
                .check()
                .await
                .map_err(|e| e.to_string())?
                .ok_or("No update available in the Tauri release feed.")?;
            // Do not stage a different release if the static feed lags behind
            // the GitHub release that the user just confirmed. The full
            // prerelease is compared (alpha.1 is not alpha.2), since the rolling
            // feed is refreshed after the versioned release is created.
            if !impulcifer_service::update::check::same_release(&update.version, latest_version) {
                return Err(
                    "Tauri release feed version does not match the selected update.".into(),
                );
            }
            let mut downloaded = 0_u64;
            let bytes = update
                .download(
                    |count, total| {
                        downloaded = downloaded.saturating_add(count as u64);
                        if let Some(total) = total.filter(|total| *total > 0) {
                            let fraction = (downloaded as f64 / total as f64).clamp(0.0, 1.0);
                            progress(
                                0.1 + fraction * 0.7,
                                &format!("Downloading: {}%", (fraction * 100.0) as u64),
                            );
                        }
                    },
                    || {},
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok::<_, String>((update, bytes))
        })?;
        *self.pending.lock().unwrap_or_else(|p| p.into_inner()) = Some((update, bytes));
        Ok(())
    }

    pub fn apply(&self, app: &tauri::AppHandle) -> Result<(), String> {
        if cfg!(windows) {
            return Err("not supported".into());
        }
        let (update, bytes) = self
            .pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .ok_or("No staged update to apply.")?;
        update.install(bytes).map_err(|e| e.to_string())?;
        app.restart();
    }
}
