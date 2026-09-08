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
        let mut text = self
            .strings
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or(key)
            .to_owned();
        if let Some(args) = args.as_object() {
            for (name, value) in args {
                let value = value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string());
                text = text.replace(&format!("{{{name}}}"), &value);
            }
        }
        text
    }
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
