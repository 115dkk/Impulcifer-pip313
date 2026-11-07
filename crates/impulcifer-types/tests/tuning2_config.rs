#![forbid(unsafe_code)]
use impulcifer_types::config::{EXTENSION_FIELD_NAMES, ProcessingConfig};
use serde_json::{Value, json};

#[test]
fn config_room_tuning2_defaults_and_validation() {
    assert_eq!(
        &EXTENSION_FIELD_NAMES[29..],
        &[
            "room_tuning2_dip_limit",
            "room_tuning2_low_protect",
            "room_tuning2_bands",
            "room_tuning2_lf_align",
            "room_tuning2_mode_decay",
            "room_tuning2_hf_phase",
        ]
    );
    let defaults = serde_json::to_value(ProcessingConfig::default()).unwrap();
    for (key, value) in [
        ("mode", json!("mixed")),
        ("max_boost", json!(10.0)),
        ("low", json!(20.0)),
        ("high", json!("auto")),
        ("mimo", json!(false)),
        ("fsiso", json!(150.0)),
        ("mimo_low", json!("auto")),
        ("dip_limit", json!(false)),
        ("low_protect", json!(false)),
        ("bands", json!("")),
        ("lf_align", json!(false)),
        ("mode_decay", json!(false)),
        ("hf_phase", json!(false)),
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
        for key in [
            "room_tuning2_mimo",
            "room_tuning2_dip_limit",
            "room_tuning2_low_protect",
            "room_tuning2_lf_align",
            "room_tuning2_mode_decay",
            "room_tuning2_hf_phase",
        ] {
            check(key, json!(value), true);
        }
    }
    for accepted in [
        "",
        "20-20000",
        "1000-3000, 20-500",
        "8000-9000, 1000-1500, 4000-5000, 2000-3000",
        " 1000 - 3000 ",
    ] {
        check("room_tuning2_bands", json!(accepted), true);
    }
    for rejected in [
        "20",
        "20-40-60",
        "NaN-100",
        "20-inf",
        "19.9-100",
        "20-20000.1",
        "100-100",
        "101-100",
        "20-30,40-50,60-70,80-90,100-110",
        "20-30,",
    ] {
        check("room_tuning2_bands", json!(rejected), false);
    }
    check("room_tuning2_bands", json!(100), false);
    let parsed = ProcessingConfig::from_kwargs(
        json!({"room_tuning2_bands":" 1000-3000, 20-500 "})
            .as_object()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(parsed.room_tuning2_bands, "1000-3000, 20-500");
    assert_eq!(
        parsed.room_tuning2_bands(),
        vec![(20.0, 500.0), (1000.0, 3000.0)]
    );
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
