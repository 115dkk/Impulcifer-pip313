//! Python-compatible ~/.impulcifer/settings.json, with embedded read-only locales.
//! Loading initializes/persists language, but only set_language marks first-run
//! selection complete. Writes preserve keys belonging to other frontends.

use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

pub const LANGUAGES: [(&str, &str); 9] = [
    ("en", "English"),
    ("ko", "한국어"),
    ("fr", "Français"),
    ("de", "Deutsch"),
    ("es", "Español"),
    ("ja", "日本語"),
    ("zh_CN", "简体中文"),
    ("zh_TW", "繁體中文"),
    ("ru", "Русский"),
];

pub struct Settings {
    path: PathBuf,
    values: Map<String, Value>,
    initialized: bool,
}

pub fn default_path() -> PathBuf {
    // std uses USERPROFILE on Windows (then the profile API), HOME on Unix
    // (then passwd), matching Path.home rather than APPDATA/XDG directories.
    std::env::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".impulcifer")
        .join("settings.json")
}

impl Settings {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            values: Map::new(),
            initialized: false,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&mut self) {
        if self.initialized {
            return;
        }
        self.values = self.read();
        let language = match self.values.get("language") {
            Some(value) => normalize_language(value.as_str().unwrap_or("en")),
            None => detect_language(),
        };
        self.values.insert("language".into(), json!(language));
        self.initialized = true;
        self.save();
    }

    fn read(&self) -> Map<String, Value> {
        std::fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        // Python reports save errors but keeps in-memory preferences. Do the
        // same, rather than changing its successful setter envelopes.
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            if let Some(parent) = self.path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&self.path, serde_json::to_vec_pretty(&self.values)?)?;
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("Failed to save settings: {error}");
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.load();
        // Refresh unrelated persisted keys, including choices written by 2.x
        // after this service was constructed.
        self.values.extend(self.read());
        self.values.insert(key.to_owned(), json!(value));
        if key == "language" {
            self.values.insert("language_selected".into(), json!(true));
        }
        self.save();
    }

    pub fn payload(&mut self) -> Value {
        self.load();
        let language = normalize_language(
            self.values
                .get("language")
                .and_then(Value::as_str)
                .unwrap_or("en"),
        );
        json!({
            "language": language,
            "theme": self.values.get("theme").cloned().unwrap_or(json!("dark")),
            "skin": self.values.get("skin").cloned().unwrap_or(json!("stable")),
            "frontend": self.values.get("frontend").cloned().unwrap_or(json!("webview")),
            "first_run": !truthy(self.values.get("language_selected").unwrap_or(&Value::Null)),
            "languages": LANGUAGES.iter().map(|(code, name)| json!({"code":code,"name":name})).collect::<Vec<_>>(),
            "strings": strings(&language),
        })
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

fn normalize_language(code: &str) -> String {
    let code = code.replace('-', "_").to_lowercase();
    let mut parts = code.split('_');
    let language = parts.next().unwrap_or("");
    if language == "zh" {
        let subtags: Vec<_> = parts.collect();
        if subtags.contains(&"hant") || subtags.contains(&"tw") || subtags.contains(&"hk") {
            "zh_TW".into()
        } else {
            "zh_CN".into()
        }
    } else if LANGUAGES
        .iter()
        .any(|(candidate, _)| *candidate == language)
    {
        language.into()
    } else {
        "en".into()
    }
}

fn detect_language() -> String {
    // Python's ordered startswith mapping matches 'zh' before zh_TW/zh_HK;
    // preserve that first-run behavior. Explicit saved zh_TW stays zh_TW.
    let locale = ["LC_ALL", "LC_CTYPE", "LANG", "LANGUAGE"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|s| !s.is_empty()))
        .or_else(platform_locale);
    detected_language(locale.as_deref())
}

fn detected_language(locale: Option<&str>) -> String {
    let locale = locale.unwrap_or_default();
    // These are the first entries for each language in Python's ordered map.
    // Unlike normalization of an explicit choice, all zh prefixes select zh_CN.
    for (prefix, language) in [
        ("en", "en"),
        ("ko", "ko"),
        ("fr", "fr"),
        ("de", "de"),
        ("es", "es"),
        ("ja", "ja"),
        ("zh", "zh_CN"),
        ("ru", "ru"),
    ] {
        if locale.starts_with(prefix) {
            return language.into();
        }
    }
    "en".into()
}

fn platform_locale() -> Option<String> {
    sys_locale::get_locale()
}

/// The language saved in a settings file, normalized to the supported set
/// ("en" when the file is absent, unreadable, or names an unknown language).
/// Read-only: nothing is initialized or persisted, so the CLI can honour the
/// UI language without rewriting the user's preferences the way `load` does.
pub fn saved_language(path: &Path) -> String {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Map<String, Value>>(&bytes).ok())
        .and_then(|values| {
            values
                .get("language")
                .and_then(Value::as_str)
                .map(normalize_language)
        })
        .unwrap_or_else(|| "en".into())
}

/// The merged catalogue for a language: the 2.x English base, the 2.x locale,
/// then the 3.x overlay (`locales/en.json`, then `locales/<language>.json`).
/// Read-only.
pub fn catalog(language: &str) -> crate::brir::Catalog {
    crate::brir::Catalog::from_strings(strings(language))
}

fn strings(language: &str) -> Map<String, Value> {
    // Embedded catalogues cannot change at runtime. Parse each supported locale
    // once, but return an owned copy so callers cannot mutate the cache.
    static CATALOGS: [std::sync::OnceLock<Map<String, Value>>; 9] =
        [const { std::sync::OnceLock::new() }; 9];
    let index = LANGUAGES
        .iter()
        .position(|(code, _)| *code == language)
        .unwrap_or(0);
    CATALOGS[index]
        .get_or_init(|| parse_strings(LANGUAGES[index].0))
        .clone()
}

fn parse_strings(language: &str) -> Map<String, Value> {
    let mut merged: Map<String, Value> =
        serde_json::from_str(catalogue_2x("en")).unwrap_or_default();
    merged.extend(
        serde_json::from_str::<Map<String, Value>>(catalogue_2x(language)).unwrap_or_default(),
    );
    merged.extend(serde_json::from_str::<Map<String, Value>>(overlay_3x("en")).unwrap_or_default());
    merged.extend(
        serde_json::from_str::<Map<String, Value>>(overlay_3x(language)).unwrap_or_default(),
    );
    merged
}

/// The shared 2.x catalogue (`i18n/locales`). Never edited for 3.x work.
fn catalogue_2x(language: &str) -> &'static str {
    match language {
        "ko" => include_str!("../../../i18n/locales/ko.json"),
        "fr" => include_str!("../../../i18n/locales/fr.json"),
        "de" => include_str!("../../../i18n/locales/de.json"),
        "es" => include_str!("../../../i18n/locales/es.json"),
        "ja" => include_str!("../../../i18n/locales/ja.json"),
        "zh_CN" => include_str!("../../../i18n/locales/zh_CN.json"),
        "zh_TW" => include_str!("../../../i18n/locales/zh_TW.json"),
        "ru" => include_str!("../../../i18n/locales/ru.json"),
        _ => include_str!("../../../i18n/locales/en.json"),
    }
}

/// The 3.x overlay (`crates/impulcifer-service/locales`): keys that only the
/// 3.x service and app emit, and 3.x rewordings of 2.x keys. A key present here
/// wins over the 2.x catalogue; every overlay file carries the same key set
/// (`overlay_catalogues_share_one_key_set_and_placeholders`).
pub fn overlay_3x(language: &str) -> &'static str {
    match language {
        "ko" => include_str!("../locales/ko.json"),
        "fr" => include_str!("../locales/fr.json"),
        "de" => include_str!("../locales/de.json"),
        "es" => include_str!("../locales/es.json"),
        "ja" => include_str!("../locales/ja.json"),
        "zh_CN" => include_str!("../locales/zh_CN.json"),
        "zh_TW" => include_str!("../locales/zh_TW.json"),
        "ru" => include_str!("../locales/ru.json"),
        _ => include_str!("../locales/en.json"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(text: &str) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            let Some(len) = rest[start + 1..].find('}') else {
                break;
            };
            let name = &rest[start + 1..start + 1 + len];
            if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                out.insert(name.to_owned());
            }
            rest = &rest[start + 1 + len + 1..];
        }
        out
    }

    #[test]
    fn overlay_catalogues_share_one_key_set_and_placeholders() {
        let english: Map<String, Value> = serde_json::from_str(overlay_3x("en")).unwrap();
        assert!(!english.is_empty(), "the 3.x overlay must not be empty");
        for (language, _) in LANGUAGES {
            let overlay: Map<String, Value> = serde_json::from_str(overlay_3x(language))
                .unwrap_or_else(|e| panic!("locales/{language}.json: {e}"));
            let mut expected: Vec<_> = english.keys().collect();
            let mut actual: Vec<_> = overlay.keys().collect();
            expected.sort();
            actual.sort();
            assert_eq!(actual, expected, "locales/{language}.json key set");
            for (key, value) in &overlay {
                let text = value
                    .as_str()
                    .unwrap_or_else(|| panic!("{language}:{key} is not a string"));
                assert!(!text.trim().is_empty(), "{language}:{key} is empty");
                assert_eq!(
                    placeholders(text),
                    placeholders(english[key].as_str().unwrap()),
                    "{language}:{key} placeholders"
                );
            }
        }
    }

    #[test]
    fn overlay_keys_win_over_the_2x_catalogue() {
        let base: Map<String, Value> = serde_json::from_str(catalogue_2x("ko")).unwrap();
        let overlay: Map<String, Value> = serde_json::from_str(overlay_3x("ko")).unwrap();
        let merged = strings("ko");
        for (key, value) in &overlay {
            assert_eq!(merged.get(key), Some(value), "{key}");
        }
        for (key, value) in &base {
            if !overlay.contains_key(key) {
                assert_eq!(merged.get(key), Some(value), "{key}");
            }
        }
    }

    #[test]
    fn cached_catalogues_match_parsing_and_are_independent() {
        for (language, _) in LANGUAGES {
            let mut first = strings(language);
            assert_eq!(first, parse_strings(language));
            first.clear();
            assert_eq!(strings(language), parse_strings(language));
        }
        assert_eq!(strings("unknown"), strings("en"));
    }

    #[test]
    fn platform_locale_maps_to_a_supported_language() {
        let locale = platform_locale();
        let detected = detected_language(locale.as_deref());
        assert!(LANGUAGES.iter().any(|(code, _)| *code == detected));
        if let Some(locale) = locale {
            let normalized = normalize_language(&locale);
            assert!(LANGUAGES.iter().any(|(code, _)| *code == normalized));
        }
        assert_eq!(detected_language(None), "en");
        for (locale, normalized, detected) in [
            ("ko-KR", "ko", "ko"),
            ("zh-Hans-CN", "zh_CN", "zh_CN"),
            ("zh-Hant-TW", "zh_TW", "zh_CN"),
            ("zh-TW", "zh_TW", "zh_CN"),
            ("zh_HK", "zh_TW", "zh_CN"),
            ("pt-BR", "en", "en"),
            ("", "en", "en"),
        ] {
            assert_eq!(normalize_language(locale), normalized, "{locale}");
            assert_eq!(detected_language(Some(locale)), detected, "{locale}");
        }
        for (code, _) in LANGUAGES {
            assert_eq!(normalize_language(code), code);
        }
        // Python uses a case-sensitive startswith, not BCP-47 normalization.
        assert_eq!(detected_language(Some("KO-KR")), "en");
        assert_eq!(detected_language(Some("korean")), "ko");
    }
}
