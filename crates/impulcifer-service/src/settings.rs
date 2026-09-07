//! Python-compatible ~/.impulcifer/settings.json, with embedded read-only locales.
//! Loading initializes/persists language, but only set_language marks first-run
//! selection complete. Writes preserve keys belonging to other frontends.

use serde_json::{Map, Value, json};
use std::path::PathBuf;

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
    let code = code.replace('-', "_");
    let parts: Vec<_> = code.split('_').collect();
    let code = if parts.len() == 2 {
        format!("{}_{}", parts[0].to_lowercase(), parts[1].to_uppercase())
    } else {
        code
    };
    if LANGUAGES.iter().any(|(candidate, _)| *candidate == code) {
        code
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
        .or_else(platform_locale)
        .unwrap_or_default();
    let prefix = locale.split(['_', '-', '.', ':']).next().unwrap_or("");
    if prefix == "zh" {
        "zh_CN".into()
    } else {
        normalize_language(prefix)
    }
}

fn platform_locale() -> Option<String> {
    #[cfg(windows)]
    let output = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[System.Globalization.CultureInfo]::CurrentCulture.Name",
        ])
        .output()
        .ok()?;
    #[cfg(not(windows))]
    let output = std::process::Command::new("locale")
        .arg("LC_CTYPE")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn strings(language: &str) -> Map<String, Value> {
    let english = include_str!("../../../i18n/locales/en.json");
    let selected = match language {
        "ko" => include_str!("../../../i18n/locales/ko.json"),
        "fr" => include_str!("../../../i18n/locales/fr.json"),
        "de" => include_str!("../../../i18n/locales/de.json"),
        "es" => include_str!("../../../i18n/locales/es.json"),
        "ja" => include_str!("../../../i18n/locales/ja.json"),
        "zh_CN" => include_str!("../../../i18n/locales/zh_CN.json"),
        "zh_TW" => include_str!("../../../i18n/locales/zh_TW.json"),
        "ru" => include_str!("../../../i18n/locales/ru.json"),
        _ => english,
    };
    let mut merged: Map<String, Value> = serde_json::from_str(english).unwrap_or_default();
    merged.extend(serde_json::from_str::<Map<String, Value>>(selected).unwrap_or_default());
    for (key, en, ko) in EXTRA_STRINGS {
        merged
            .entry((*key).to_owned())
            .or_insert_with(|| Value::String((if language == "ko" { ko } else { en }).to_string()));
    }
    merged
}

/// Keys that only the 3.x service emits. They live here instead of
/// `i18n/locales/*.json` so the shared 2.x catalogue (and its release gate)
/// stay untouched; a catalogue entry with the same key wins if one appears.
const EXTRA_STRINGS: &[(&str, &str, &str)] = &[(
    "cli_plots_not_available_yet",
    "Plots are not available in this version yet; the plot stages were skipped.",
    "이 버전에서는 아직 플롯을 만들지 않습니다. 플롯 단계를 건너뛰었습니다.",
)];
