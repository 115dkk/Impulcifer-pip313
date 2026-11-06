#![forbid(unsafe_code)]
use impulcifer_types::config::{ConfigError, EXTENSION_FIELD_NAMES, ProcessingConfig};
use serde_json::json;

fn error(field: &str, value: &str) -> String {
    let mut config = ProcessingConfig::default();
    match field {
        "missing_speakers" => config.missing_speakers = value.into(),
        "lfe_mode" => config.lfe_mode = value.into(),
        _ => unreachable!(),
    }
    match config.validate_room_options().unwrap_err() {
        ConfigError::Invalid { field, reason } => format!("invalid value for {field}: {reason}"),
    }
}

#[test]
fn output_fill_config_defaults_and_validation_messages() {
    let defaults = ProcessingConfig::default();
    assert_eq!(defaults.missing_speakers, "silent");
    assert_eq!(defaults.lfe_mode, "none");
    assert_eq!(EXTENSION_FIELD_NAMES.len(), 29);
    assert!(EXTENSION_FIELD_NAMES.contains(&"missing_speakers"));
    assert!(EXTENSION_FIELD_NAMES.contains(&"lfe_mode"));

    assert_eq!(
        error("missing_speakers", "closest"),
        "invalid value for missing_speakers: must be silent or neighbor"
    );
    assert_eq!(
        error("lfe_mode", "sub"),
        "invalid value for lfe_mode: must be none, center or all"
    );
    for value in ["silent", "neighbor"] {
        ProcessingConfig::from_kwargs(json!({"missing_speakers":value}).as_object().unwrap())
            .unwrap()
            .validate_room_options()
            .unwrap();
    }
    for value in ["none", "center", "all"] {
        ProcessingConfig::from_kwargs(json!({"lfe_mode":value}).as_object().unwrap())
            .unwrap()
            .validate_room_options()
            .unwrap();
    }
}
