#![forbid(unsafe_code)]

use impulcifer_types::config::{EXTENSION_FIELD_NAMES, ProcessingConfig};
use serde_json::json;

#[test]
fn sofa_config_defaults_and_boolean_validation() {
    let defaults = ProcessingConfig::default();
    assert!(!defaults.sofa);
    assert!(!defaults.sofa_split_headphone_eq);
    assert!(EXTENSION_FIELD_NAMES.contains(&"sofa"));
    assert!(EXTENSION_FIELD_NAMES.contains(&"sofa_split_headphone_eq"));

    let config = ProcessingConfig::from_kwargs(
        json!({"sofa":true,"sofa_split_headphone_eq":true})
            .as_object()
            .unwrap(),
    )
    .unwrap();
    assert!(config.sofa);
    assert!(config.sofa_split_headphone_eq);

    for field in ["sofa", "sofa_split_headphone_eq"] {
        let error = ProcessingConfig::from_kwargs(json!({field:"true"}).as_object().unwrap())
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            format!("invalid value for {field}: expected a boolean")
        );
    }
}
