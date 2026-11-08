#![forbid(unsafe_code)]
use impulcifer_cli::{Parsed, options::EXTENSION_OPTIONS, parse};
use impulcifer_types::config::ProcessingConfig;
use serde_json::json;
fn args(s: &[&str]) -> Vec<String> {
    s.iter().map(|s| (*s).into()).collect()
}
#[test]
fn cli_room_tuning2_options_parse_and_validate() {
    assert_eq!(
        EXTENSION_OPTIONS[EXTENSION_OPTIONS.len() - 9..]
            .iter()
            .map(|option| option.dest)
            .collect::<Vec<_>>(),
        [
            "room_tuning2_dip_limit",
            "room_tuning2_low_protect",
            "room_tuning2_bands",
            "room_tuning2_lf_align",
            "room_tuning2_mode_decay",
            "room_tuning2_hf_phase",
            "room_tuning2_high_basis",
            "room_tuning2_high_transition",
            "room_tuning2_input_hp",
        ]
    );
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
        "dip_limit",
        "low_protect",
        "bands",
        "lf_align",
        "mode_decay",
        "hf_phase",
        "high_basis",
        "high_transition",
        "input_hp",
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
        "--room_tuning2_dip_limit=true",
        "--room_tuning2_low_protect=true",
        "--room_tuning2_bands=1000-3000, 20-500",
        "--room_tuning2_lf_align=true",
        "--room_tuning2_mode_decay=true",
        "--room_tuning2_hf_phase=true",
        "--room_tuning2_high_basis=lenient",
        "--room_tuning2_high_transition=4",
        "--room_tuning2_input_hp=25",
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
    assert!(config.room_tuning2_dip_limit);
    assert!(config.room_tuning2_low_protect);
    assert!(config.room_tuning2_lf_align);
    assert!(config.room_tuning2_mode_decay);
    assert!(config.room_tuning2_hf_phase);
    assert_eq!(config.room_tuning2_high_basis, "lenient");
    assert_eq!(config.room_tuning2_high_transition, 4);
    assert_eq!(config.room_tuning2_input_hp, "25");
    assert_eq!(config.room_tuning2_bands, "1000-3000, 20-500");
    assert_eq!(
        config.room_tuning2_bands(),
        vec![(20.0, 500.0), (1000.0, 3000.0)]
    );
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
        ("dip_limit", "yes"),
        ("low_protect", "yes"),
        ("lf_align", "yes"),
        ("mode_decay", "yes"),
        ("hf_phase", "yes"),
        ("high_basis", "agreement"),
        ("high_transition", "3"),
        ("input_hp", "9"),
        ("input_hp", "51"),
        ("input_hp", "NaN"),
        ("bands", "bad"),
        ("bands", "20-30,40-50,60-70,80-90,100-110"),
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
        "--room_tuning2_dip_limit=false",
        "--room_tuning2_low_protect=false",
        "--room_tuning2_lf_align=false",
        "--room_tuning2_mode_decay=false",
        "--room_tuning2_hf_phase=false",
        "--room_tuning2_high=auto",
        "--room_tuning2_mimo_low=auto",
        "--room_tuning2_high_basis=strict",
        "--room_tuning2_high_transition=1",
        "--room_tuning2_input_hp=off",
    ]))
    .unwrap() else {
        panic!()
    };
    assert_eq!(kwargs["room_tuning2_mimo"], json!(false));
    assert_eq!(kwargs["room_tuning2_dip_limit"], json!(false));
    assert_eq!(kwargs["room_tuning2_low_protect"], json!(false));
    assert_eq!(kwargs["room_tuning2_lf_align"], json!(false));
    assert_eq!(kwargs["room_tuning2_mode_decay"], json!(false));
    assert_eq!(kwargs["room_tuning2_hf_phase"], json!(false));
}
