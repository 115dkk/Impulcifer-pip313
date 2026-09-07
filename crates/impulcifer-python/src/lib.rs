#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Owned configuration boundary shared by interpreter-free tests and the wheel.
use impulcifer_types::config::{ConfigError, ProcessingConfig};
use serde_json::{Map, Value};

pub fn run_config_dict(kwargs: &Map<String, Value>) -> Result<ProcessingConfig, ConfigError> {
    ProcessingConfig::from_kwargs(kwargs)
}

#[cfg(feature = "python")]
mod module;
