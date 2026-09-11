#![forbid(unsafe_code)]

//! The 3.x app UI (`apps/impulcifer-app/ui`) and the merged catalogue
//! (2.x `i18n/locales` + the 3.x overlay in `crates/impulcifer-service/locales`)
//! must agree: every key the page asks for resolves in every language, so a
//! typo or a missing overlay entry cannot render as a raw key name.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn ui_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/impulcifer-app/ui")
}

/// `data-i18n="key"` in the markup and literal `t("key")` / `tOr("key", …)` calls
/// in the script. Template keys such as `webview_status_${status}` are dynamic and
/// are covered by the explicit status list below.
fn literal_keys() -> BTreeSet<String> {
    let html = std::fs::read_to_string(ui_dir().join("index.html")).unwrap();
    let js = std::fs::read_to_string(ui_dir().join("app.js")).unwrap();
    let mut keys = BTreeSet::new();
    for (haystack, prefix) in [(&html, "data-i18n=\""), (&js, "t(\""), (&js, "tOr(\"")] {
        let mut rest = haystack.as_str();
        while let Some(start) = rest.find(prefix) {
            let after = &rest[start + prefix.len()..];
            let Some(end) = after.find('"') else { break };
            let key = &after[..end];
            // `t("` must be a call of `t` itself, not the tail of another
            // identifier such as `brirDefault("` or `createElement("`.
            let bounded = rest[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
            if bounded
                && !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                keys.insert(key.to_owned());
            }
            rest = after;
        }
    }
    for status in [
        "running",
        "cancel_requested",
        "succeeded",
        "failed",
        "cancelled",
    ] {
        keys.insert(format!("webview_status_{status}"));
    }
    assert!(
        keys.len() > 100,
        "key extraction found only {} keys",
        keys.len()
    );
    keys
}

#[test]
fn ui_keys_resolve_in_every_language() {
    let keys = literal_keys();
    for (language, _) in impulcifer_service::settings::LANGUAGES {
        let catalog = impulcifer_service::settings::catalog(language);
        let missing: Vec<_> = keys
            .iter()
            .filter(|key| !catalog.strings.contains_key(key.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "{language}: UI keys missing from the merged catalogue: {missing:?}"
        );
    }
}

#[test]
fn ui_scripts_and_markup_are_present_in_the_app_crate() {
    for name in [
        "index.html",
        "app.js",
        "styles.css",
        "logo/pulse-32.png",
        "logo/pulse-128.png",
    ] {
        assert!(
            ui_dir().join(name).is_file(),
            "apps/impulcifer-app/ui/{name} is missing"
        );
    }
    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/impulcifer-app/tauri.conf.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(config["build"]["frontendDist"], "ui");
    let html = std::fs::read_to_string(ui_dir().join("index.html")).unwrap();
    assert!(
        !html.contains("../logo/"),
        "the 3.x page must not reach outside frontendDist"
    );
}
