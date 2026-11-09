#![forbid(unsafe_code)]

use impulcifer_types::config::{ConfigError, EXTENSION_FIELD_NAMES, ProcessingConfig};
use serde_json::json;

fn message(config: &ProcessingConfig) -> String {
    match config.validate_room_options().unwrap_err() {
        ConfigError::Invalid { field, reason } => format!("invalid value for {field}: {reason}"),
    }
}

#[test]
fn itd_config_defaults_and_validation() {
    let defaults = ProcessingConfig::default();
    assert_eq!(defaults.itd_mode, "off");
    assert_eq!(defaults.head_radius, None);
    assert_eq!(EXTENSION_FIELD_NAMES.len(), 40);
    assert!(EXTENSION_FIELD_NAMES.contains(&"itd_mode"));
    assert!(EXTENSION_FIELD_NAMES.contains(&"head_radius"));

    let mut invalid_mode = defaults.clone();
    invalid_mode.itd_mode = "automatic".into();
    assert_eq!(
        message(&invalid_mode),
        "invalid value for itd_mode: must be off, symmetric or model"
    );
    let mut invalid_radius = defaults.clone();
    invalid_radius.head_radius = Some(20.0);
    assert_eq!(
        message(&invalid_radius),
        "invalid value for head_radius: must be between 5 and 15 cm"
    );

    for mode in ["off", "symmetric", "model"] {
        let config = ProcessingConfig::from_kwargs(
            json!({"itd_mode":mode,"head_radius":9.25})
                .as_object()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(config.itd_mode, mode);
        assert_eq!(config.head_radius, Some(9.25));
        config.validate_room_options().unwrap();
    }
    let unset =
        ProcessingConfig::from_kwargs(json!({"head_radius":null}).as_object().unwrap()).unwrap();
    assert_eq!(unset.head_radius, None);
}
