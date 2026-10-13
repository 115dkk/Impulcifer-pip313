#![forbid(unsafe_code)]
use impulcifer_types::config::{EXTENSION_FIELD_NAMES, ProcessingConfig};
use serde_json::{Value, json};

#[test]
fn config_room_tuning2_defaults_and_validation() {
    let defaults = serde_json::to_value(ProcessingConfig::default()).unwrap();
    for (key, value) in [
        ("mode", json!("mixed")),
        ("max_boost", json!(10.0)),
        ("low", json!(20.0)),
        ("high", json!("auto")),
        ("mimo", json!(false)),
        ("fsiso", json!(150.0)),
        ("mimo_low", json!("auto")),
    ] {
        let key = format!("room_tuning2_{key}");
        assert_eq!(defaults[&key], value);
        assert!(EXTENSION_FIELD_NAMES.contains(&key.as_str()));
    }
    let check = |key: &str, value: Value, valid: bool| {
        let mut kwargs = json!({"room_mode":"tuning2", "room_range":"legacy"});
        kwargs[key] = value.clone();
        let result = ProcessingConfig::from_kwargs(kwargs.as_object().unwrap())
            .and_then(|c| c.validate_room_options());
        assert_eq!(result.is_ok(), valid, "{key}={value}: {result:?}");
        if let Err(e) = result {
            assert!(e.to_string().contains(key), "{e}");
        }
    };
    for mode in ["zero", "mixed", "linear", "long", "30", "200"] {
        check("room_tuning2_mode", json!(mode), true);
    }
    for (key, lo, hi) in [
        ("room_tuning2_mode", 30.0, 200.0),
        ("room_tuning2_high", 100.0, 20000.0),
        ("room_tuning2_mimo_low", 15.0, 150.0),
    ] {
        for (n, valid) in [(lo, true), (hi, true), (lo - 0.1, false), (hi + 0.1, false)] {
            check(key, json!(n), valid);
            check(key, json!(n.to_string()), valid);
        }
        for bad in ["NaN", "inf", "bad"] {
            check(key, json!(bad), false);
        }
    }
    for (key, lo, hi) in [
        ("room_tuning2_max_boost", 0.0, 10.0),
        ("room_tuning2_low", 10.0, 200.0),
        ("room_tuning2_fsiso", 50.0, 150.0),
    ] {
        for (n, valid) in [(lo, true), (hi, true), (lo - 0.1, false), (hi + 0.1, false)] {
            check(key, json!(n), valid);
        }
    }
    for key in ["room_tuning2_high", "room_tuning2_mimo_low"] {
        check(key, json!("auto"), true);
    }
    for value in [true, false] {
        check("room_tuning2_mimo", json!(value), true);
    }
    let mut config = ProcessingConfig {
        room_tuning2_low: 100.0,
        room_tuning2_high: "100".into(),
        ..Default::default()
    };
    assert!(
        config
            .validate_room_options()
            .unwrap_err()
            .to_string()
            .contains("room_tuning2_high")
    );
    config.room_tuning2_high = "101".into();
    config.validate_room_options().unwrap();
    for high in ["99", "100"] {
        config.room_tuning2_high = high.into();
        assert!(config.validate_room_options().is_err());
    }
    assert_eq!(ProcessingConfig::oracle_defaults().room_range, "legacy");
    assert_eq!(ProcessingConfig::oracle_defaults().room_mode, "eq");
}
