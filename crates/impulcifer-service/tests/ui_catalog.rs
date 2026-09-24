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

/// `errorSentence` in app.js turns a service error into a catalogue sentence
/// by its fixed English message. Every message it matches must still be sent
/// by the service word for word, or the page would fall back to the English
/// text without anyone noticing.
#[test]
fn translated_error_messages_are_still_sent_by_the_service() {
    // Windows checkouts may carry CRLF line endings.
    let js = std::fs::read_to_string(ui_dir().join("app.js"))
        .unwrap()
        .replace("\r\n", "\n");
    let start = js.find("function errorSentence(").unwrap();
    let body = &js[start..start + js[start..].find("\n}\n").unwrap()];
    let messages: Vec<_> = regex::Regex::new(r#"case "([^"]+)":"#)
        .unwrap()
        .captures_iter(body)
        .map(|c| c[1].to_owned())
        .collect();
    assert!(messages.len() >= 7, "found only {messages:?}");
    let mut sources = String::new();
    let mut dirs = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                sources.push_str(&std::fs::read_to_string(&path).unwrap());
            }
        }
    }
    let stale: Vec<_> = messages
        .iter()
        .filter(|m| !sources.contains(&format!("\"{m}\"")))
        .collect();
    assert!(
        stale.is_empty(),
        "app.js translates messages the service no longer sends: {stale:?}"
    );
}

/// Keys the BRIR job logs or reports as progress (`events.log(level, "key", …)`,
/// `events.step("key", …)` and the stage-key table) must resolve in every
/// language too; the job log renders them through the same catalogue.
#[test]
fn brir_job_log_keys_resolve_in_every_language() {
    let pattern = regex::Regex::new(
        r#"(?:\.log\(\s*"[a-z]+",\s*|\.step\(\s*|=>\s*)"((?:cli|vbass)_[a-z0-9_]+)""#,
    )
    .unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/brir");
    let mut keys = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            let source = std::fs::read_to_string(&path).unwrap();
            keys.extend(pattern.captures_iter(&source).map(|c| c[1].to_owned()));
        }
    }
    assert!(
        keys.contains("cli_info_parallel_threads") && keys.len() > 20,
        "log key extraction found only {keys:?}"
    );
    for (language, _) in impulcifer_service::settings::LANGUAGES {
        let catalog = impulcifer_service::settings::catalog(language);
        let missing: Vec<_> = keys
            .iter()
            .filter(|key| !catalog.strings.contains_key(key.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "{language}: BRIR log keys missing from the merged catalogue: {missing:?}"
        );
    }
}
