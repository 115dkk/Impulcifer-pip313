//! Filesystem adapters for the file-free P08/P09/P10 BRIR pipeline.
pub mod discovery;
pub mod estimator;
pub mod inputs;
pub mod outputs;
pub mod plots;
pub mod run;
pub mod sweep_grid;
pub(crate) mod validation;

use impulcifer_dsp::DspError;
use impulcifer_io::IoError;
use impulcifer_jobs::registry::JobFailure;
use impulcifer_types::ipc::{self, ErrorCode};
use serde_json::{Map, Value, json};

#[derive(Debug, thiserror::Error)]
pub enum BrirError {
    #[error("{0}")]
    Io(#[from] IoError),
    #[error("{0}")]
    Fs(#[from] std::io::Error),
    #[error("{0}")]
    Dsp(#[from] DspError),
    #[error("{0}")]
    Missing(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("FFmpeg is required for TrueHD/MLP input")]
    FfmpegMissing,
    #[error("cancelled")]
    Cancelled,
}
impl BrirError {
    pub fn envelope(&self) -> Value {
        let code = match self {
            Self::Missing(_) => ErrorCode::FileNotFound,
            Self::Fs(e) | Self::Io(IoError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                ErrorCode::FileNotFound
            }
            Self::Unsupported(_) => ErrorCode::InvalidRequest,
            _ => ErrorCode::InternalError,
        };
        ipc::error(code, self.to_string(), json!({}), false)
    }
    pub fn failure(self) -> JobFailure {
        JobFailure {
            cancelled: matches!(self, Self::Cancelled),
            error: self.envelope()["error"].clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Catalog {
    pub strings: Map<String, Value>,
    /// Deterministic clock injection for byte-for-byte oracle tests.
    pub readme_date: Option<String>,
}
impl Catalog {
    pub fn from_strings(strings: Map<String, Value>) -> Self {
        Self {
            strings,
            readme_date: None,
        }
    }
    pub fn english() -> Self {
        Self::from_strings(
            serde_json::from_str(include_str!("../../../../i18n/locales/en.json"))
                .expect("English catalogue"),
        )
    }
    pub fn translate(&self, key: &str, args: &Value) -> String {
        let text = self.strings.get(key).and_then(Value::as_str).unwrap_or(key);
        let Some(args) = args.as_object() else {
            return text.to_owned();
        };
        // Legacy replacement is ordered by Map iteration. Braces in names or
        // replacements can create matches across replacement/literal boundaries,
        // including matches for later arguments. Keep that uncommon contract;
        // ordinary scalar interpolation builds the result in one pass below.
        if args.iter().any(|(name, value)| {
            name.contains(['{', '}'])
                || match value {
                    Value::String(value) => value.contains(['{', '}']),
                    Value::Array(_) | Value::Object(_) => true,
                    _ => false,
                }
        }) {
            return replace_interacting(text, args);
        }
        let mut result = String::with_capacity(text.len());
        let mut literal = 0;
        let mut opening = None;
        for (index, byte) in text.bytes().enumerate() {
            match byte {
                b'{' => {
                    if opening.is_some() {
                        return replace_interacting(text, args);
                    }
                    opening = Some(index);
                }
                b'}' => {
                    if let Some(start) = opening.take()
                        && let Some(value) = args.get(&text[start + 1..index])
                    {
                        result.push_str(&text[literal..start]);
                        if let Some(value) = value.as_str() {
                            result.push_str(value);
                        } else {
                            // serde_json controls numeric spelling, just as in
                            // the original Value::to_string implementation.
                            use std::fmt::Write;
                            write!(result, "{value}").expect("writing into String");
                        }
                        literal = index + 1;
                    }
                }
                _ => {}
            }
        }
        result.push_str(&text[literal..]);
        result
    }
}

fn replace_interacting(text: &str, args: &Map<String, Value>) -> String {
    let mut result = text.to_owned();
    for (name, value) in args {
        let value = value
            .as_str()
            .map(std::borrow::Cow::Borrowed)
            .unwrap_or_else(|| std::borrow::Cow::Owned(value.to_string()));
        result = result.replace(&format!("{{{name}}}"), &value);
    }
    result
}

pub trait BrirEvents {
    fn step(&mut self, key: &str, args: Value) -> Result<(), BrirError>;
    fn log(&mut self, level: &str, key: &str, args: Value);
    fn check_cancelled(&self) -> Result<(), BrirError>;
    /// Translate a catalogue key for messages that embed translated text as an
    /// argument (2.x passes `loc.get("cli_eqapo_reason_*")` into the bypass
    /// warning). The default returns the key itself.
    fn translate(&self, key: &str) -> String {
        key.to_owned()
    }
    /// The job's cancel token, so long rayon batches (plots) can consult the job
    /// at every task boundary; headless callers have none.
    fn cancel_token(&self) -> Option<impulcifer_types::audio::CancelToken> {
        None
    }
}

#[cfg(test)]
mod catalog_tests {
    use super::*;

    #[test]
    fn interpolation_matches_ordered_replacement_for_all_catalogues_and_braces() {
        let argument_sets = [
            json!({"date":"2026-09-08","fs":48000}),
            json!({"a":"", "b":"한글", "":7}),
            json!({"a":"{b}","b":"{a}","c":"}"}),
            json!({"a":"{", "b":"}", "ab":"x"}),
            json!({"a}":"x", "{b":"y", "a":"b"}),
            json!({"a":-0.0, "b":1.25e-12, "c":true, "d":null}),
            json!({"a":[1,"{b}"], "b":{"c":"x"}}),
        ];
        let mut catalog = Catalog::english();
        for (language, _) in crate::settings::LANGUAGES {
            let localized = crate::settings::catalog(language);
            for (key, text) in &localized.strings {
                if let Some(text) = text.as_str() {
                    for args in &argument_sets {
                        assert_eq!(
                            localized.translate(key, args),
                            replace_interacting(text, args.as_object().unwrap())
                        );
                    }
                }
            }
        }
        // Exhaustive small malformed/nested strings include matches created by
        // removing an inner placeholder, repeated placeholders and empty names.
        let alphabet = ["{", "}", "a", "b", "한"];
        for length in 0..=6 {
            for mut code in 0..alphabet.len().pow(length) {
                let mut text = String::new();
                for _ in 0..length {
                    text.push_str(alphabet[code % alphabet.len()]);
                    code /= alphabet.len();
                }
                catalog.strings.insert("test".into(), json!(text));
                for args in &argument_sets {
                    assert_eq!(
                        catalog.translate("test", args),
                        replace_interacting(&text, args.as_object().unwrap()),
                        "{text:?} {args}"
                    );
                }
            }
        }
        assert_eq!(
            catalog.translate("missing {a}", &json!({"a":42})),
            "missing 42"
        );
        assert_eq!(catalog.translate("{a}", &Value::Null), "{a}");
    }
}
