#![forbid(unsafe_code)]
use impulcifer_cli::{Parsed, parse};
use impulcifer_types::config::ProcessingConfig;
use serde_json::json;
fn args(s: &[&str]) -> Vec<String> {
    s.iter().map(|s| (*s).into()).collect()
}
#[test]
fn cli_room_tuning2_options_parse_and_validate() {
    let Parsed::Kwargs(baseline) = parse(&args(&["impulcifer", "--dir_path=data/demo"])).unwrap()
    else {
        panic!()
    };
    for name in [
        "mode",
        "max_boost",
        "low",
        "high",
        "mimo",
        "fsiso",
        "mimo_low",
    ] {
        assert!(!baseline.contains_key(&format!("room_tuning2_{name}")));
    }
    let Parsed::Kwargs(kwargs) = parse(&args(&[
        "impulcifer",
        "--dir_path=data/demo",
        "--room_mode=tuning2",
        "--room_range=legacy",
        "--room_tuning2_mode=60",
        "--room_tuning2_max_boost=5",
        "--room_tuning2_low=30",
        "--room_tuning2_high=300",
        "--room_tuning2_mimo=true",
        "--room_tuning2_fsiso=120",
        "--room_tuning2_mimo_low=40",
    ]))
    .unwrap() else {
        panic!()
    };
    let config = ProcessingConfig::from_kwargs(&kwargs).unwrap();
    config.validate_room_options().unwrap();
    assert_eq!(config.room_mode, "tuning2");
    assert_eq!(config.room_tuning2_mode, "60");
    assert_eq!(config.room_tuning2_max_boost, 5.0);
    assert_eq!(config.room_tuning2_low, 30.0);
    assert_eq!(config.room_tuning2_high, "300");
    assert_eq!(kwargs["room_tuning2_mimo"], json!(true));
    assert_eq!(config.room_tuning2_fsiso, 120.0);
    assert_eq!(config.room_tuning2_mimo_low, "40");
    for mode in ["zero", "mixed", "linear", "long", "30", "200"] {
        parse(&args(&[
            "impulcifer",
            "--dir_path=data/demo",
            "--room_mode=tuning2",
            "--room_tuning2_mode",
            mode,
        ]))
        .unwrap();
    }
    for (name, value) in [
        ("mode", "29"),
        ("mode", "201"),
        ("mode", "NaN"),
        ("max_boost", "-0.1"),
        ("max_boost", "10.1"),
        ("low", "9"),
        ("low", "201"),
        ("high", "99"),
        ("high", "20001"),
        ("mimo", "yes"),
        ("fsiso", "49"),
        ("fsiso", "151"),
        ("mimo_low", "14"),
        ("mimo_low", "151"),
    ] {
        let flag = format!("--room_tuning2_{name}");
        let error =
            parse(&args(&["impulcifer", "--dir_path=data/demo", &flag, value])).unwrap_err();
        assert_eq!(error.exit_code, 2);
        assert!(
            error.message.contains(&format!("room_tuning2_{name}")),
            "{error:?}"
        );
    }
    let Parsed::Kwargs(kwargs) = parse(&args(&[
        "impulcifer",
        "--dir_path=data/demo",
        "--room_tuning2_mimo=false",
        "--room_tuning2_high=auto",
        "--room_tuning2_mimo_low=auto",
    ]))
    .unwrap() else {
        panic!()
    };
    assert_eq!(kwargs["room_tuning2_mimo"], json!(false));
}
