#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Owned configuration boundary shared by interpreter-free tests and the wheel.
use impulcifer_types::config::{ConfigError, ProcessingConfig};
use serde_json::{Map, Value};

pub fn run_config_dict(kwargs: &Map<String, Value>) -> Result<ProcessingConfig, ConfigError> {
    // 2.x accepts a numeric channel balance (a dB correction) next to the
    // named modes; the service normalises that number to its string form in
    // brir/validation.rs, so do the same before the typed conversion.
    if let Some(number) = kwargs.get("channel_balance").filter(|v| v.is_number()) {
        let mut owned = kwargs.clone();
        owned.insert("channel_balance".into(), Value::String(number.to_string()));
        return ProcessingConfig::from_kwargs(&owned);
    }
    ProcessingConfig::from_kwargs(kwargs)
}

#[cfg(feature = "python")]
mod module;
