//! Read-only settings: invoking the CLI must not initialize or rewrite UI preferences.
use impulcifer_service::{brir::Catalog, settings};
use std::path::Path;

pub(crate) fn catalog() -> Catalog {
    catalog_at(&settings::default_path())
}

pub(crate) fn catalog_at(path: &Path) -> Catalog {
    settings::catalog(&settings::saved_language(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_settings_fall_back_to_english_without_writes() {
        let path = std::env::temp_dir()
            .join(format!(
                "impulcifer-cli-missing-settings-{}",
                std::process::id()
            ))
            .join("settings.json");
        assert!(!path.exists());
        assert_eq!(
            catalog_at(&path).translate("cli_writing_brirs", &json!({})),
            Catalog::english().translate("cli_writing_brirs", &json!({}))
        );
        assert!(
            catalog_at(&path)
                .strings
                .contains_key("cli_plots_not_available_yet"),
            "3.x-only keys come from the service overlay"
        );
        assert!(!path.exists());
    }
}
