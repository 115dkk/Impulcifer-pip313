#![forbid(unsafe_code)]

use super::{Apply, UpdateOptions};
use std::sync::mpsc;
use velopack::{UpdateCheck, UpdateManager, locator::VelopackLocatorConfig, sources::HttpSource};

pub fn manager(options: &UpdateOptions) -> Result<UpdateManager, String> {
    let locator = options
        .velopack_root
        .as_ref()
        .map(|root| VelopackLocatorConfig {
            RootAppDir: root.clone(),
            UpdateExePath: root.join("Update.exe"),
            PackagesDir: root.join("packages"),
            ManifestPath: root.join("current/sq.version"),
            CurrentBinaryDir: root.join("current"),
            IsPortable: false,
        });
    UpdateManager::new(
        HttpSource::new(&options.releases_url),
        Some(velopack::UpdateOptions {
            ExplicitChannel: Some("win".into()),
            ..Default::default()
        }),
        locator,
    )
    .map_err(|e| e.to_string())
}

pub(super) fn download(
    options: &UpdateOptions,
    progress: &(dyn Fn(f64, &str) + Sync),
) -> Result<Apply, String> {
    progress(0.1, "update_downloading");
    let manager = manager(options)?;
    let UpdateCheck::UpdateAvailable(update) =
        manager.check_for_updates().map_err(|e| e.to_string())?
    else {
        return Err("No update available in the Velopack release feed.".into());
    };
    let (send, receive) = mpsc::channel();
    // The SDK uses a Sender, so drain concurrently rather than replaying all
    // progress after the blocking download. The scoped receiver always joins.
    std::thread::scope(|scope| {
        let reader = scope.spawn(move || {
            for percent in receive {
                let fraction = f64::from(percent).clamp(0.0, 100.0) / 100.0;
                progress(
                    0.1 + fraction * 0.7,
                    &format!("Downloading: {}%", (fraction * 100.0) as u64),
                );
            }
        });
        let result = manager
            .download_updates(&update, Some(send))
            .map_err(|e| e.to_string());
        reader
            .join()
            .map_err(|_| "Update progress callback panicked.".to_owned())?;
        result
    })?;
    progress(0.8, "update_installing");
    Ok(Box::new(move || {
        manager
            .apply_updates_and_restart(&*update)
            .map_err(|e| e.to_string())
    }))
}
