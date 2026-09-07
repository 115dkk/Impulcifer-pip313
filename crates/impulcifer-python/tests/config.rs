#![forbid(unsafe_code)]

use impulcifer_native::run_config_dict;
use impulcifer_types::config::{DecaySpec, ProcessingConfig};
use serde_json::json;

#[test]
fn run_config_dict_matches_from_kwargs() {
    for decay in [json!(0.35), json!({"FL":0.35,"FR":0.42})] {
        let kwargs = json!({
            "dir_path":"measurements", "decay":decay, "head_ms":2.0,
            "vbass":true, "unknown":{"ignored":[null,1,"value"]}
        });
        let kwargs = kwargs.as_object().unwrap();
        let actual = run_config_dict(kwargs).unwrap();
        assert_eq!(actual, ProcessingConfig::from_kwargs(kwargs).unwrap());
        assert_eq!(actual.head_ms, 2.0);
        match actual.decay.unwrap() {
            DecaySpec::Uniform(seconds) => assert_eq!(seconds, 0.35),
            DecaySpec::PerChannel(channels) => {
                assert_eq!(channels["FL"], 0.35);
                assert_eq!(channels["FR"], 0.42);
            }
        }
    }
    assert_eq!(
        run_config_dict(&Default::default()).unwrap(),
        ProcessingConfig::default()
    );
    assert!(run_config_dict(json!({"plot":"wrong type"}).as_object().unwrap()).is_err());
}
