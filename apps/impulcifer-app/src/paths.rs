#![forbid(unsafe_code)]

use std::path::PathBuf;

/// Prefer Tauri's mapped resources unless the caller supplied an explicit override.
pub fn bundled_data_dir(resource_dir: Option<PathBuf>, override_set: bool) -> Option<PathBuf> {
    if override_set {
        return None;
    }
    resource_dir
        .map(|directory| directory.join("data"))
        .filter(|directory| directory.is_dir())
}

#[cfg(test)]
mod tests {
    use super::bundled_data_dir;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Resources(PathBuf);

    impl Resources {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "impulcifer-p21-paths-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Resources {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn bundled_data_dir_uses_mapped_resources() {
        let resources = Resources::new();
        let data = resources.0.join("data");
        std::fs::create_dir(&data).unwrap();
        assert_eq!(
            bundled_data_dir(Some(resources.0.clone()), false),
            Some(data)
        );
    }

    #[test]
    fn bundled_data_dir_respects_override() {
        let resources = Resources::new();
        std::fs::create_dir(resources.0.join("data")).unwrap();
        assert_eq!(bundled_data_dir(Some(resources.0.clone()), true), None);
    }

    #[test]
    fn bundled_data_dir_requires_existing_directory() {
        let resources = Resources::new();
        assert_eq!(bundled_data_dir(None, false), None);
        assert_eq!(bundled_data_dir(Some(resources.0.clone()), false), None);
        std::fs::write(resources.0.join("data"), b"not a directory").unwrap();
        assert_eq!(bundled_data_dir(Some(resources.0.clone()), false), None);
    }
}
