#![forbid(unsafe_code)]

use impulcifer_cli::{Parsed, parse};
use impulcifer_types::config::ProcessingConfig;

#[test]
fn sofa_flags_parse_appear_in_help_and_reach_config() {
    let argv = [
        "impulcifer",
        "--dir_path",
        "measurements",
        "--sofa",
        "--sofa_split_headphone_eq",
    ]
    .map(String::from);
    let Parsed::Kwargs(kwargs) = parse(&argv).unwrap() else {
        panic!("expected config")
    };
    let config = ProcessingConfig::from_kwargs(&kwargs).unwrap();
    assert!(config.sofa);
    assert!(config.sofa_split_headphone_eq);

    let Parsed::Help(help) = parse(&["impulcifer".into(), "--help".into()]).unwrap() else {
        panic!("expected help")
    };
    assert!(help.contains("--sofa"));
    assert!(help.contains("--sofa_split_headphone_eq"));
    assert!(help.contains("Also save the BRIR as a SOFA file"));
    assert!(help.contains("also save the BRIR without headphone compensation"));
}
